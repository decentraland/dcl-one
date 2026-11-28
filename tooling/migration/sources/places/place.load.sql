-- A world's image may be the old worlds server's `<base>/contents/<sha256 hex>`
-- of an uploaded thumbnail; the node stores that file under its CID
-- (`migrate.py blobs worlds-content-server` records key -> CID), so the URL
-- names the CID. A key the copy has not recorded yet is left as is.
UPDATE stage s
SET raw = jsonb_set(s.raw, '{image}', to_jsonb(left(s.raw->>'image', -64) || b.hash))
FROM dcl_migration.blob_inventory b
WHERE s.world_name IS NOT NULL
  AND s.raw->>'image' ~ '/contents/[0-9a-f]{64}$'
  AND b.source = 'worlds-content-server'
  AND b.object_key = right(s.raw->>'image', 64);

DO $images$ DECLARE n bigint; BEGIN
  SELECT count(*) INTO n FROM stage s
   WHERE s.world_name IS NOT NULL AND s.raw->>'image' ~ '/contents/[0-9a-f]{64}$'
     AND NOT EXISTS (SELECT 1 FROM dcl_migration.blob_missing m
                      WHERE m.source = 'worlds-content-server' AND m.object_key = right(s.raw->>'image', 64));
  IF n > 0 THEN RAISE NOTICE
    'places/place: % world images name a sha256 thumbnail key the worlds blob copy has not recorded; run `migrate.py blobs worlds-content-server`, then this step again', n;
  ELSE NULL; END IF; END $images$;

-- Operator decisions first, as overlays the places mirror and derive re-apply
-- (places::moderation, places::ranking). Rows this migration wrote carry
-- set_by = 'migration:places'; a decision made on the node (any other set_by)
-- is never overwritten, and a migration overlay whose upstream decision is gone
-- is removed.
SELECT pg_advisory_xact_lock_shared(hashtextextended('explore.scene_interaction_import', 0));
SELECT pg_advisory_xact_lock(hashtextextended('explore.place_curation', 0));
ALTER TABLE stage ADD COLUMN interaction_id text, ADD COLUMN curation_winner boolean DEFAULT false;
UPDATE stage SET interaction_id = explore.genesis_curation_key(explore.scene_interaction_entity_id(id));

-- Select once for both overlays and their deletions. Historical aliases can
-- resolve to the same key, but the currently verified UUID wins if present.
WITH preferred AS (
    SELECT DISTINCT ON (s.interaction_id) s.id
    FROM stage s
    LEFT JOIN explore.world_scene_public_id b
      ON b.public_id = CASE WHEN pg_input_is_valid(s.id, 'uuid') THEN s.id::uuid END
     AND b.identity::text = s.interaction_id AND b.entity_id IS NOT NULL
    LEFT JOIN explore.world_scene_place p
      ON p.identity = b.identity AND p.active AND p.world_name = b.world_name AND p.entity_id = b.entity_id
    LEFT JOIN worlds.world_scenes d
      ON d.world_name = p.world_name AND d.entity_id = p.entity_id AND d.mirror_source = b.source
    LEFT JOIN worlds.worlds w ON w.name = d.world_name AND w.mirror_source = b.source
    LEFT JOIN explore.genesis_place_alias g ON g.public_id = s.id AND g.local_id = s.interaction_id
    ORDER BY s.interaction_id, (w.name IS NOT NULL OR g.public_id IS NOT NULL) DESC, s.id
)
UPDATE stage s SET curation_winner = true FROM preferred WHERE s.id = preferred.id;

DELETE FROM explore.place_moderation m
USING stage s
WHERE s.curation_winner AND m.place_id = s.interaction_id AND m.set_by = 'migration:places'
  AND NOT (s.mod_disabled OR s.highlighted OR s.mod_rating IS NOT NULL);

INSERT INTO explore.place_moderation AS m
    (place_id, disabled, highlighted, content_rating, reason, set_by, updated_at)
SELECT interaction_id,
       CASE WHEN mod_disabled THEN true END,
       CASE WHEN highlighted THEN true END,
       mod_rating,
       CASE WHEN mod_disabled THEN 'moderation' END,
       'migration:places',
       timezone('UTC', now())
FROM stage
WHERE curation_winner AND (mod_disabled OR highlighted OR mod_rating IS NOT NULL)
ON CONFLICT (place_id) DO UPDATE SET
    disabled = EXCLUDED.disabled,
    highlighted = EXCLUDED.highlighted,
    content_rating = EXCLUDED.content_rating,
    reason = EXCLUDED.reason,
    updated_at = EXCLUDED.updated_at
WHERE m.set_by = 'migration:places'
  AND (m.disabled, m.highlighted, m.content_rating, m.reason)
      IS DISTINCT FROM (EXCLUDED.disabled, EXCLUDED.highlighted, EXCLUDED.content_rating, EXCLUDED.reason);

DELETE FROM explore.place_ranking r
USING stage s
WHERE s.curation_winner AND r.place_id = s.interaction_id AND r.set_by = 'migration:places' AND s.ranking_overlay IS NULL;

INSERT INTO explore.place_ranking AS r (place_id, overlay, set_by, updated_at)
SELECT interaction_id, ranking_overlay, 'migration:places', timezone('UTC', now())
FROM stage
WHERE curation_winner AND ranking_overlay IS NOT NULL
ON CONFLICT (place_id) DO UPDATE SET
    overlay = EXCLUDED.overlay,
    updated_at = EXCLUDED.updated_at
WHERE r.set_by = 'migration:places' AND r.overlay IS DISTINCT FROM EXCLUDED.overlay;

SELECT explore.project_genesis_place_curation(
    ARRAY(SELECT DISTINCT interaction_id FROM stage WHERE curation_winner), true
);

-- The catalogue row, local authority: counters start at 0 because the facts
-- (user_favorites, user_likes) are imported and counted in
-- place_interaction_counts. A world or id this node already publishes in the
-- local leg is left to it, and a row derived from this node's content
-- (raw.source = 'content') is never overwritten, as the mirror does.
INSERT INTO explore.place AS p
    (id, base_position, title, description, creator_address, content_rating,
     categories, likes, dislikes, favorites, deployed_at, disabled, highlighted,
     raw, fetched_at, origin, removal_admin_keys)
SELECT s.id, s.base_position, s.title, s.description, s.creator_address,
       COALESCE(m.content_rating, s.content_rating),
       COALESCE(s.categories, '{}'), 0, 0, 0,
       s.deployed_at AT TIME ZONE 'UTC',
       COALESCE(m.disabled, s.disabled), COALESCE(m.highlighted, s.highlighted),
       s.raw || COALESCE(r.overlay, '{}'::jsonb),
       timezone('UTC', now()), 'local', '{}'::text[]
FROM stage s
LEFT JOIN explore.place_moderation m ON m.place_id = s.interaction_id
LEFT JOIN explore.place_ranking r ON r.place_id = s.interaction_id
WHERE (s.world_name IS NULL OR s.id = lower(s.world_name))
  AND NOT EXISTS (
    SELECT 1 FROM explore.place_world_local l
    WHERE l.id = s.id
       OR (s.world_name IS NOT NULL AND l.world IS TRUE AND lower(l.world_name) = lower(s.world_name)))
ON CONFLICT (id) DO UPDATE SET
    base_position   = EXCLUDED.base_position,
    title           = EXCLUDED.title,
    description     = EXCLUDED.description,
    creator_address = EXCLUDED.creator_address,
    content_rating  = EXCLUDED.content_rating,
    categories      = EXCLUDED.categories,
    likes           = 0,
    dislikes        = 0,
    favorites       = 0,
    deployed_at     = EXCLUDED.deployed_at,
    disabled        = EXCLUDED.disabled,
    highlighted     = EXCLUDED.highlighted,
    raw             = EXCLUDED.raw,
    fetched_at      = EXCLUDED.fetched_at,
    origin          = 'local'
WHERE p.raw->>'source' IS DISTINCT FROM 'content'
  AND NOT EXISTS (
    SELECT 1 FROM explore.genesis_place_observed trusted WHERE trusted.public_id = p.id
  )
  AND (p.base_position, p.title, p.description, p.creator_address, p.content_rating,
       p.categories, p.likes, p.dislikes, p.favorites, p.deployed_at, p.disabled,
       p.highlighted, p.raw, p.origin)
      IS DISTINCT FROM
      (EXCLUDED.base_position, EXCLUDED.title, EXCLUDED.description, EXCLUDED.creator_address,
       EXCLUDED.content_rating, EXCLUDED.categories, 0, 0, 0, EXCLUDED.deployed_at,
       EXCLUDED.disabled, EXCLUDED.highlighted, EXCLUDED.raw, 'local');
