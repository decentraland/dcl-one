-- Upstream's catalog becomes the node's: definitions and tiers keep their
-- upstream ids, and the seed's rows for the same ids take upstream's content.
-- Every badge is imported, published or not, archived or not, with its
-- published_at, archived_at and created_at. A badge whose only tier sits in
-- the `root` slot is a single badge: no tier rows on the node, its root
-- tier's steps in criteria_steps. Asset URLs are upstream's (CDN_HOST +
-- <badge>[/<tier slot>]/{2d,3d}/...); a tiered badge has none of its own,
-- as upstream. `migrate.py blobs badges` mirrors the art they name.
--
-- The publication columns and criteria_steps come with the badge catalog
-- schema; on a schema without them a non-empty batch stops instead of
-- importing a catalog that loses them.
DO $catalog$
DECLARE
  lacking text;
  n bigint;
  kept text;
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
         r.steps AS root_steps, r.steps IS NULL AS is_tier
  FROM stage s
  LEFT JOIN LATERAL (SELECT (t ->> 'steps')::integer AS steps FROM jsonb_array_elements(s.tiers) t
                     WHERE t ->> 'asset_slot' = 'root' LIMIT 1) r ON true;

  CREATE TEMP TABLE catalog_tier ON COMMIT DROP AS
  SELECT b.id AS badge_id, u ->> 'tier_id' AS tier_id, u ->> 'name' AS tier_name, u ->> 'description' AS description,
         (u ->> 'steps')::integer AS criteria_steps, (u ->> 'position')::integer - 1 AS ordinal,
         u ->> 'asset_slot' AS asset_slot
  FROM catalog_badge b
  CROSS JOIN LATERAL jsonb_array_elements(b.tiers) u
  WHERE b.is_tier;

  -- Upstream's CDN_HOST layout (legacy-assets.ts): a single badge's art sits
  -- in its own directory, a tiered badge's in one directory per tier slot.
  CREATE TEMP TABLE catalog_assets ON COMMIT DROP AS
  SELECT dir, jsonb_build_object(
             '2d', jsonb_build_object('normal', base || '/2d/normal.png'),
             '3d', jsonb_build_object('normal', base || '/3d/normal.png', 'hrm', base || '/3d/hrm.png',
                                      'basecolor', base || '/3d/basecolor.png')) AS assets
  FROM (SELECT dir, 'https://assets-cdn.decentraland.org/' || dir AS base
        FROM (SELECT id AS dir FROM catalog_badge WHERE NOT is_tier
              UNION
              SELECT badge_id || '/' || asset_slot FROM catalog_tier) d) x;

  INSERT INTO social.badge_definitions AS d
      (id, name, description, category, is_tier, assets, criteria_steps, published_at, archived_at, created_at)
  SELECT b.id, b.name, b.description, b.category, b.is_tier,
         COALESCE(a.assets, '{}'::jsonb),
         b.root_steps, b.published_at AT TIME ZONE 'UTC', b.archived_at AT TIME ZONE 'UTC',
         b.created_at AT TIME ZONE 'UTC'
  FROM catalog_badge b
  LEFT JOIN catalog_assets a ON a.dir = b.id AND NOT b.is_tier
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
    (SELECT count(*) FROM catalog_badge WHERE published_at IS NULL OR archived_at IS NOT NULL);

  -- A badge's tiers are exactly upstream's: the seed's other tiers of the
  -- badge, any tier rows of a single badge, and rows whose position moved
  -- (re-inserted below, so no two tiers of a badge share an ordinal) go.
  -- Earned tiers reference the badge, not the tier row, so nothing a player
  -- holds is deleted.
  DELETE FROM social.badge_tiers t
  USING catalog_badge b
  WHERE t.badge_id = b.id
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

  DROP TABLE catalog_assets;
  DROP TABLE catalog_tier;
  DROP TABLE catalog_badge;
END $catalog$;
