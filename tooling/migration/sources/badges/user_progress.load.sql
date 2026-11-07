-- Badge and tier ids are upstream's (the catalog step imported them); a row
-- whose badge is not in the catalog (unpublished or archived upstream) or
-- whose tier is unknown is skipped. Merges keep the furthest progress so
-- node-side grants survive re-runs.
WITH src AS (
    SELECT DISTINCT ON (st.address, d.id)
           st.address, d.id AS badge_id, d.is_tier, st.steps,
           st.achieved_tiers, st.completed_at, st.updated_at
    FROM stage st
    JOIN social.badge_definitions d ON d.id = st.badge_id
    ORDER BY st.address, d.id, st.updated_at DESC, st.source_address
), tiers AS (
    SELECT s.address, s.badge_id, t.tier_id, t.ordinal,
           min(COALESCE(timestamptz 'epoch' + (e ->> 'completed_at')::numeric * interval '1 millisecond',
                        s.completed_at, s.updated_at)) AS completed_at
    FROM src s
    CROSS JOIN LATERAL jsonb_array_elements(
        CASE WHEN jsonb_typeof(s.achieved_tiers) = 'array' THEN s.achieved_tiers ELSE '[]'::jsonb END) e
    JOIN social.badge_tiers t ON t.badge_id = s.badge_id AND t.tier_id = e ->> 'tier_id'
    WHERE s.is_tier
    GROUP BY s.address, s.badge_id, t.tier_id, t.ordinal
), tier_rows AS (
    INSERT INTO social.user_achieved_tiers (address, badge_id, tier_id, completed_at)
    SELECT address, badge_id, tier_id, completed_at FROM tiers
    ON CONFLICT (address, badge_id, tier_id) DO UPDATE
       SET completed_at = LEAST(social.user_achieved_tiers.completed_at, EXCLUDED.completed_at)
)
INSERT INTO social.user_badge_progress
    (address, badge_id, steps_done, completed_at, last_completed_tier_id, updated_at)
SELECT s.address, s.badge_id,
       CASE WHEN s.is_tier THEN LEAST(GREATEST(COALESCE(s.steps, 0), 0), 2147483647)::integer
            ELSE (s.completed_at IS NOT NULL)::integer END,
       s.completed_at,
       (SELECT t.tier_id FROM tiers t
         WHERE t.address = s.address AND t.badge_id = s.badge_id
         ORDER BY t.ordinal DESC LIMIT 1),
       s.updated_at
FROM src s
ON CONFLICT (address, badge_id) DO UPDATE
   SET steps_done = GREATEST(social.user_badge_progress.steps_done, EXCLUDED.steps_done),
       completed_at = LEAST(social.user_badge_progress.completed_at, EXCLUDED.completed_at),
       last_completed_tier_id = COALESCE(
           (SELECT t.tier_id FROM social.badge_tiers t
             WHERE t.badge_id = EXCLUDED.badge_id
               AND t.tier_id IN (social.user_badge_progress.last_completed_tier_id,
                                 EXCLUDED.last_completed_tier_id)
             ORDER BY t.ordinal DESC LIMIT 1),
           social.user_badge_progress.last_completed_tier_id),
       updated_at = GREATEST(social.user_badge_progress.updated_at, EXCLUDED.updated_at)
