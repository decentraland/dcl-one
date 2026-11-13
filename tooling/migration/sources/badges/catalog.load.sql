-- Upstream's catalog becomes the node's: definitions and tiers keep their
-- upstream ids, and the seed's rows for the same ids take upstream's content.
-- Every badge is imported, published or not, archived or not, with its
-- published_at, archived_at and created_at. A badge whose only tier sits in
-- the `root` slot is a single badge: no tier rows on the node, its root
-- tier's steps in criteria_steps. Assets are upstream's paths relative to
-- its CDN_HOST (<badge>[/<tier slot>]/{2d,3d}/...), which the node resolves
-- against its public asset base; a tiered badge has none of its own, as
-- upstream. `migrate.py blobs badges` mirrors the art they name. A presence
-- badge upstream refuses to serve (tiered, or its steps differ from its
-- configuration's) is imported unpublished. A badge whose definition or
-- tiers no longer match what this load last wrote was edited on the node
-- and is kept (reported).
--
-- The publication columns and criteria_steps come with the badge catalog
-- schema; on a schema without them a non-empty batch stops instead of
-- importing a catalog that loses them.
DO $catalog$
DECLARE
  lacking text;
  n bigint;
  kept text;
  refused_id text;
  edited_ids text;
BEGIN
  SELECT string_agg(c, ', ' ORDER BY c) INTO lacking
  FROM unnest(ARRAY['criteria_steps', 'published_at', 'archived_at', 'created_at']) c
  WHERE NOT EXISTS (SELECT 1 FROM information_schema.columns
                    WHERE table_schema = 'social' AND table_name = 'badge_definitions' AND column_name = c);
  IF lacking IS NOT NULL THEN
    IF EXISTS (SELECT 1 FROM stage) THEN
      RAISE EXCEPTION 'badges/catalog: social.badge_definitions lacks %; apply the badge catalog schema first', lacking;
    END IF;
    RETURN;
  END IF;

  CREATE TEMP TABLE catalog_badge ON COMMIT DROP AS
  SELECT s.id, s.name, s.category, s.description, s.published_at, s.archived_at, s.created_at, s.tiers,
         r.steps AS root_steps, r.steps IS NULL AS is_tier,
         -- Upstream's catalog reader leaves out a presence badge that is not a
         -- single badge asking for its configuration's steps; the ids are its
         -- BADGE_EVALUATORS with an observer, tiers or an import of its own.
         s.presence AND s.id <> ALL (ARRAY['dclcitizen', 'epic_ensemble', 'regally_rare', 'legendary_look',
                                           'mythic_model', 'unique_unicorn', 'exotic_elegance', 'open_for_business',
                                           'traveler', 'profile_pro', 'emotionista', 'fashionista',
                                           'event_enthusiast', 'moves_master', 'social_butterfly',
                                           'vertical_voyager', 'walkabout_wanderer', 'land_architect',
                                           'emote_creator', 'wearable_designer', 'referral'])
           AND (jsonb_array_length(s.tiers) <> 1 OR r.steps IS NULL
                OR r.steps IS DISTINCT FROM COALESCE(s.presence_steps, r.steps)) AS refused
  FROM stage s
  LEFT JOIN LATERAL (SELECT (t ->> 'steps')::integer AS steps FROM jsonb_array_elements(s.tiers) t
                     WHERE t ->> 'asset_slot' = 'root' LIMIT 1) r ON true;

  -- What a badge holds on the node: its definition and tiers.
  CREATE TEMP VIEW catalog_fingerprint AS
  SELECT d.id, md5(row(d.name, d.description, d.category, d.is_tier, d.assets, d.criteria_steps, d.published_at,
                       d.archived_at, d.created_at,
                       (SELECT jsonb_agg(jsonb_build_array(t.tier_id, t.tier_name, t.description, t.assets,
                                                           t.criteria_steps, t.ordinal) ORDER BY t.tier_id)
                        FROM social.badge_tiers t WHERE t.badge_id = d.id))::text) AS fingerprint
  FROM social.badge_definitions d;

  ALTER TABLE catalog_badge ADD COLUMN edited boolean NOT NULL DEFAULT false;
  UPDATE catalog_badge b SET edited = true
  FROM catalog_fingerprint f JOIN dcl_migration.badge_catalog_loaded l ON l.id = f.id
  WHERE f.id = b.id AND f.fingerprint <> l.fingerprint;
  SELECT string_agg(id, ', ' ORDER BY id) INTO edited_ids FROM catalog_badge WHERE edited;
  IF edited_ids IS NOT NULL THEN
    RAISE NOTICE 'badges/catalog: kept badge(s) % as edited on the node', edited_ids;
  END IF;

  FOR refused_id IN SELECT id FROM catalog_badge WHERE refused AND published_at IS NOT NULL ORDER BY id LOOP
    RAISE NOTICE 'badges/catalog: badge % not published: its presence configuration does not match a single badge '
                 'with the same steps', refused_id;
  END LOOP;

  CREATE TEMP TABLE catalog_tier ON COMMIT DROP AS
  SELECT b.id AS badge_id, u ->> 'tier_id' AS tier_id, u ->> 'name' AS tier_name, u ->> 'description' AS description,
         (u ->> 'steps')::integer AS criteria_steps, (u ->> 'position')::integer - 1 AS ordinal,
         u ->> 'asset_slot' AS asset_slot
  FROM catalog_badge b
  CROSS JOIN LATERAL jsonb_array_elements(b.tiers) u
  WHERE b.is_tier AND NOT b.edited;

  -- Upstream's CDN_HOST layout (legacy-assets.ts): a single badge's art sits
  -- in its own directory, a tiered badge's in one directory per tier slot.
  CREATE TEMP TABLE catalog_assets ON COMMIT DROP AS
  SELECT dir, jsonb_build_object(
             '2d', jsonb_build_object('normal', dir || '/2d/normal.png'),
             '3d', jsonb_build_object('normal', dir || '/3d/normal.png', 'hrm', dir || '/3d/hrm.png',
                                      'basecolor', dir || '/3d/basecolor.png')) AS assets
  FROM (SELECT id AS dir FROM catalog_badge WHERE NOT is_tier
        UNION
        SELECT badge_id || '/' || asset_slot FROM catalog_tier) d;

  INSERT INTO social.badge_definitions AS d
      (id, name, description, category, is_tier, assets, criteria_steps, published_at, archived_at, created_at)
  SELECT b.id, b.name, b.description, b.category, b.is_tier,
         COALESCE(a.assets, '{}'::jsonb),
         b.root_steps, CASE WHEN NOT b.refused THEN b.published_at AT TIME ZONE 'UTC' END,
         b.archived_at AT TIME ZONE 'UTC',
         b.created_at AT TIME ZONE 'UTC'
  FROM catalog_badge b
  LEFT JOIN catalog_assets a ON a.dir = b.id AND NOT b.is_tier
  WHERE NOT b.edited
  ON CONFLICT (id) DO UPDATE SET
      name = EXCLUDED.name,
      description = EXCLUDED.description,
      category = EXCLUDED.category,
      is_tier = EXCLUDED.is_tier,
      assets = EXCLUDED.assets,
      criteria_steps = EXCLUDED.criteria_steps,
      published_at = EXCLUDED.published_at,
      archived_at = EXCLUDED.archived_at,
      created_at = EXCLUDED.created_at
  WHERE (d.name, d.description, d.category, d.is_tier, d.assets, d.criteria_steps, d.published_at, d.archived_at,
         d.created_at)
        IS DISTINCT FROM (EXCLUDED.name, EXCLUDED.description, EXCLUDED.category, EXCLUDED.is_tier, EXCLUDED.assets,
                          EXCLUDED.criteria_steps, EXCLUDED.published_at, EXCLUDED.archived_at, EXCLUDED.created_at);
  GET DIAGNOSTICS n = ROW_COUNT;
  RAISE NOTICE 'badges/catalog: upserted % badges (% not published or archived)', n,
    (SELECT count(*) FROM catalog_badge WHERE published_at IS NULL OR archived_at IS NOT NULL OR refused);

  -- A badge's tiers are exactly upstream's: the seed's other tiers of the
  -- badge, any tier rows of a single badge, and rows whose position moved
  -- (re-inserted below, so no two tiers of a badge share an ordinal) go.
  -- Earned tiers reference the badge, not the tier row, so nothing a player
  -- holds is deleted.
  DELETE FROM social.badge_tiers t
  USING catalog_badge b
  WHERE t.badge_id = b.id AND NOT b.edited
    AND (NOT b.is_tier OR NOT EXISTS (SELECT 1 FROM catalog_tier u
                                      WHERE u.badge_id = t.badge_id AND u.tier_id = t.tier_id
                                        AND u.ordinal = t.ordinal));

  INSERT INTO social.badge_tiers AS t (badge_id, tier_id, tier_name, description, assets, criteria_steps, ordinal)
  SELECT u.badge_id, u.tier_id, u.tier_name, u.description, a.assets, u.criteria_steps, u.ordinal
  FROM catalog_tier u
  JOIN catalog_assets a ON a.dir = u.badge_id || '/' || u.asset_slot
  ON CONFLICT (badge_id, tier_id) DO UPDATE SET
      tier_name = EXCLUDED.tier_name,
      description = EXCLUDED.description,
      assets = EXCLUDED.assets,
      criteria_steps = EXCLUDED.criteria_steps
  WHERE (t.tier_name, t.description, t.assets, t.criteria_steps)
        IS DISTINCT FROM (EXCLUDED.tier_name, EXCLUDED.description, EXCLUDED.assets, EXCLUDED.criteria_steps);

  -- The seed's placeholders for badges upstream names differently
  -- (decentraland_citizen = dclcitizen, walkabout = walkabout_wanderer) would
  -- list twice; they go once upstream's catalog is in, unless a player holds
  -- them on the node.
  IF EXISTS (SELECT 1 FROM catalog_badge WHERE id IN ('dclcitizen', 'walkabout_wanderer')) THEN
    DELETE FROM social.badge_definitions d
    WHERE d.id IN ('decentraland_citizen', 'walkabout')
      AND NOT EXISTS (SELECT 1 FROM social.user_badge_progress p WHERE p.badge_id = d.id)
      AND NOT EXISTS (SELECT 1 FROM social.user_achieved_tiers p WHERE p.badge_id = d.id);
    SELECT string_agg(id, ', ') INTO kept FROM social.badge_definitions
    WHERE id IN ('decentraland_citizen', 'walkabout');
    IF kept IS NOT NULL THEN
      RAISE NOTICE 'badges/catalog: kept seed placeholder(s) % because players hold them on the node', kept;
    END IF;
  END IF;

  INSERT INTO dcl_migration.badge_catalog_loaded (id, fingerprint)
  SELECT f.id, f.fingerprint FROM catalog_fingerprint f JOIN catalog_badge b ON b.id = f.id AND NOT b.edited
  ON CONFLICT (id) DO UPDATE SET fingerprint = EXCLUDED.fingerprint;

  DROP VIEW catalog_fingerprint;
  DROP TABLE catalog_assets;
  DROP TABLE catalog_tier;
  DROP TABLE catalog_badge;
END $catalog$;
