-- Upstream's catalog becomes the node's: definitions and tiers keep their
-- upstream ids, and the seed's rows for the same ids take upstream's content.
-- A badge whose only tier sits in the `root` slot is a single badge (no tier
-- rows on the node; its target is 1). Asset URLs follow upstream's layout
-- (<badge>[/<tier slot>]/{2d,3d}/...) under the host the node rewrites to its
-- public asset base; the art itself is mirrored into BADGES_ASSETS_DIR.
CREATE TEMP TABLE catalog_badge ON COMMIT DROP AS
SELECT s.id, s.name, s.category, s.description, s.visible, s.tiers,
       NOT EXISTS (SELECT 1 FROM jsonb_array_elements(s.tiers) t WHERE t ->> 'asset_slot' = 'root') AS is_tier
FROM stage s;

CREATE TEMP TABLE catalog_assets ON COMMIT DROP AS
SELECT dir, jsonb_build_object(
           '2d', jsonb_build_object('normal', base || '/2d/normal.png'),
           '3d', jsonb_build_object('normal', base || '/3d/normal.png', 'hrm', base || '/3d/hrm.png',
                                    'basecolor', base || '/3d/basecolor.png')) AS assets
FROM (SELECT dir, 'https://badges.decentraland.org/assets/' || dir AS base
      FROM (SELECT id AS dir FROM catalog_badge
            UNION
            SELECT b.id || '/' || (t ->> 'asset_slot')
            FROM catalog_badge b CROSS JOIN LATERAL jsonb_array_elements(b.tiers) t
            WHERE b.is_tier) d) x;

DO $hidden$
DECLARE n integer;
BEGIN
  SELECT count(*) INTO n FROM catalog_badge WHERE NOT visible;
  IF n > 0 THEN
    RAISE NOTICE 'badges/catalog: skipped % unpublished or archived badges (their progress is skipped too)', n;
  END IF;
END $hidden$;

INSERT INTO social.badge_definitions AS d (id, name, description, category, is_tier, assets)
SELECT b.id, b.name, b.description, b.category, b.is_tier, a.assets
FROM catalog_badge b
JOIN catalog_assets a ON a.dir = b.id
WHERE b.visible
ON CONFLICT (id) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    category = EXCLUDED.category,
    is_tier = EXCLUDED.is_tier,
    assets = EXCLUDED.assets
WHERE (d.name, d.description, d.category, d.is_tier, d.assets)
      IS DISTINCT FROM (EXCLUDED.name, EXCLUDED.description, EXCLUDED.category, EXCLUDED.is_tier, EXCLUDED.assets);

-- A badge's tiers are exactly upstream's: the seed's other tiers of the same
-- badge (and any tier rows of a single badge) go. Earned tiers reference the
-- badge, not the tier row, so nothing a player holds is deleted.
DELETE FROM social.badge_tiers t
USING catalog_badge b
WHERE t.badge_id = b.id AND b.visible
  AND (NOT b.is_tier OR NOT EXISTS (SELECT 1 FROM jsonb_array_elements(b.tiers) u
                                    WHERE u ->> 'tier_id' = t.tier_id));

INSERT INTO social.badge_tiers AS t (badge_id, tier_id, tier_name, description, assets, criteria_steps, ordinal)
SELECT b.id, u ->> 'tier_id', u ->> 'name', u ->> 'description', a.assets,
       (u ->> 'steps')::integer, (u ->> 'position')::integer - 1
FROM catalog_badge b
CROSS JOIN LATERAL jsonb_array_elements(b.tiers) u
JOIN catalog_assets a ON a.dir = b.id || '/' || (u ->> 'asset_slot')
WHERE b.visible AND b.is_tier
ON CONFLICT (badge_id, tier_id) DO UPDATE SET
    tier_name = EXCLUDED.tier_name,
    description = EXCLUDED.description,
    assets = EXCLUDED.assets,
    criteria_steps = EXCLUDED.criteria_steps,
    ordinal = EXCLUDED.ordinal
WHERE (t.tier_name, t.description, t.assets, t.criteria_steps, t.ordinal)
      IS DISTINCT FROM (EXCLUDED.tier_name, EXCLUDED.description, EXCLUDED.assets, EXCLUDED.criteria_steps,
                        EXCLUDED.ordinal);

-- The seed's placeholders for badges upstream names differently
-- (decentraland_citizen = dclcitizen, walkabout = walkabout_wanderer) would
-- list twice; they go once upstream's catalog is in, unless a player holds
-- them on the node.
DO $placeholders$
DECLARE kept text;
BEGIN
  IF EXISTS (SELECT 1 FROM catalog_badge WHERE id IN ('dclcitizen', 'walkabout_wanderer') AND visible) THEN
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
END $placeholders$;

DROP TABLE catalog_assets;
DROP TABLE catalog_badge;
