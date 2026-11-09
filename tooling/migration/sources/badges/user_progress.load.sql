-- Badge and tier ids are upstream's (the catalog step imported every badge,
-- archived ones included); a row whose badge or tier is unknown is skipped.
--
-- Rows of one wallet under two spellings (and the node's own row) merge:
-- the most steps, the earliest completed_at, every achieved tier at its
-- earliest completed_at, the highest-ordinal last tier. The progress
-- document is the source's: the one of the spelling with the most steps
-- (then the latest updated_at, then the lowest address), each of its
-- top-level arrays replaced by the distinct union of that key's arrays
-- across the spellings. On the node it replaces the stored document (no
-- node writer keeps one yet); steps_done never goes down.
--
-- progress comes with the badge catalog schema; on a schema without it a
-- non-empty batch stops instead of dropping the documents.
DO $progress$
DECLARE
  lacking text;
  n bigint;
BEGIN
  SELECT string_agg(c, ', ' ORDER BY c) INTO lacking
  FROM (VALUES ('user_badge_progress', 'progress'), ('badge_definitions', 'criteria_steps')) v(t, c)
  WHERE NOT EXISTS (SELECT 1 FROM information_schema.columns
                    WHERE table_schema = 'social' AND table_name = v.t AND column_name = v.c);
  IF lacking IS NOT NULL THEN
    IF EXISTS (SELECT 1 FROM stage) THEN
      RAISE EXCEPTION 'badges/user_progress: the badge tables lack %; apply the badge catalog schema first', lacking;
    END IF;
    RETURN;
  END IF;

  SELECT count(*) INTO n FROM stage st
  WHERE NOT EXISTS (SELECT 1 FROM social.badge_definitions d WHERE d.id = st.badge_id);
  IF n > 0 THEN
    RAISE NOTICE 'badges/user_progress: skipped % rows of badges not in the catalog', n;
  END IF;

  CREATE TEMP TABLE progress_row ON COMMIT DROP AS
  SELECT st.*, d.is_tier, d.criteria_steps
  FROM stage st
  JOIN social.badge_definitions d ON d.id = st.badge_id;

  CREATE TEMP TABLE progress_src ON COMMIT DROP AS
  SELECT r.address, r.badge_id, bool_or(r.is_tier) AS is_tier, max(r.criteria_steps) AS criteria_steps,
         max(r.steps) AS steps, min(r.completed_at) AS completed_at, max(r.updated_at) AS updated_at,
         (array_agg(r.progress ORDER BY r.steps DESC NULLS LAST, r.updated_at DESC, r.source_address))[1] AS progress,
         jsonb_agg(r.progress) AS documents, count(*) AS spellings
  FROM progress_row r
  GROUP BY r.address, r.badge_id;

  UPDATE progress_src p
     SET progress = p.progress || COALESCE((
           SELECT jsonb_object_agg(k.key, (
                    SELECT COALESCE(jsonb_agg(DISTINCT e), '[]'::jsonb)
                    FROM jsonb_array_elements(p.documents) d
                    CROSS JOIN LATERAL jsonb_array_elements(
                        CASE WHEN jsonb_typeof(d -> k.key) = 'array' THEN d -> k.key ELSE '[]'::jsonb END) e))
           FROM jsonb_each(p.progress) k
           WHERE jsonb_typeof(k.value) = 'array'), '{}'::jsonb)
   WHERE p.spellings > 1 AND jsonb_typeof(p.progress) = 'object';

  CREATE TEMP TABLE progress_tier ON COMMIT DROP AS
  SELECT r.address, r.badge_id, t.tier_id, t.ordinal,
         min(COALESCE(timestamptz 'epoch' + (e ->> 'completed_at')::numeric * interval '1 millisecond',
                      r.completed_at, r.updated_at)) AS completed_at
  FROM progress_row r
  CROSS JOIN LATERAL jsonb_array_elements(
      CASE WHEN jsonb_typeof(r.achieved_tiers) = 'array' THEN r.achieved_tiers ELSE '[]'::jsonb END) e
  JOIN social.badge_tiers t ON t.badge_id = r.badge_id AND t.tier_id = e ->> 'tier_id'
  WHERE r.is_tier
  GROUP BY r.address, r.badge_id, t.tier_id, t.ordinal;

  INSERT INTO social.user_achieved_tiers (address, badge_id, tier_id, completed_at)
  SELECT address, badge_id, tier_id, completed_at FROM progress_tier
  ON CONFLICT (address, badge_id, tier_id) DO UPDATE
     SET completed_at = LEAST(social.user_achieved_tiers.completed_at, EXCLUDED.completed_at);

  -- steps_done is the document's `steps`, for single badges too (a partial
  -- open_for_business keeps its 1 of 2); a completed row without one counts
  -- as its target.
  INSERT INTO social.user_badge_progress AS p
      (address, badge_id, steps_done, progress, completed_at, last_completed_tier_id, updated_at)
  SELECT s.address, s.badge_id,
         LEAST(GREATEST(COALESCE(s.steps, CASE WHEN s.completed_at IS NOT NULL
                                               THEN COALESCE(s.criteria_steps, 1) ELSE 0 END), 0),
               2147483647)::integer,
         s.progress,
         s.completed_at,
         (SELECT t.tier_id FROM progress_tier t
           WHERE t.address = s.address AND t.badge_id = s.badge_id
           ORDER BY t.ordinal DESC LIMIT 1),
         s.updated_at
  FROM progress_src s
  ON CONFLICT (address, badge_id) DO UPDATE
     SET steps_done = GREATEST(p.steps_done, EXCLUDED.steps_done),
         progress = EXCLUDED.progress,
         completed_at = LEAST(p.completed_at, EXCLUDED.completed_at),
         last_completed_tier_id = COALESCE(
             (SELECT t.tier_id FROM social.badge_tiers t
               WHERE t.badge_id = EXCLUDED.badge_id
                 AND t.tier_id IN (p.last_completed_tier_id, EXCLUDED.last_completed_tier_id)
               ORDER BY t.ordinal DESC LIMIT 1),
             p.last_completed_tier_id),
         updated_at = GREATEST(p.updated_at, EXCLUDED.updated_at);

  DROP TABLE progress_tier;
  DROP TABLE progress_src;
  DROP TABLE progress_row;
END $progress$;
