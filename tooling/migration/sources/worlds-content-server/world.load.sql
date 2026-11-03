-- Imported worlds are local (mirror_source NULL). Columns upstream lacks
-- (blocked_since, realm_name_override, preview_wearable_urns) are untouched.
INSERT INTO worlds.worlds AS w (
    name, owner, access_type, access_secret_hash, access_nft, spawn_coordinates,
    title, description, content_rating, skybox_time, categories, single_player,
    show_in_places, thumbnail_hash, settings_version, mirror_source, created_at, updated_at)
SELECT
    s.name, s.owner, s.access_type, s.access_secret_hash, s.access_nft, s.spawn_coordinates,
    s.title, s.description,
    CASE WHEN s.content_rating IN ('RP', 'E', 'T', 'A', 'R') THEN s.content_rating END,
    s.skybox_time,
    CASE WHEN cardinality(s.categories) > 20 THEN s.categories[1:20] ELSE s.categories END,
    coalesce(s.single_player, false),
    coalesce(s.show_in_places, true),
    s.thumbnail_hash,
    greatest(coalesce(s.settings_version, 0), 0),
    NULL,
    s.created_at AT TIME ZONE 'UTC',
    s.updated_at AT TIME ZONE 'UTC'
FROM stage s
ON CONFLICT (name) DO UPDATE SET
    owner = EXCLUDED.owner,
    access_type = EXCLUDED.access_type,
    access_secret_hash = EXCLUDED.access_secret_hash,
    access_nft = EXCLUDED.access_nft,
    spawn_coordinates = EXCLUDED.spawn_coordinates,
    title = EXCLUDED.title,
    description = EXCLUDED.description,
    content_rating = EXCLUDED.content_rating,
    skybox_time = EXCLUDED.skybox_time,
    categories = EXCLUDED.categories,
    single_player = EXCLUDED.single_player,
    show_in_places = EXCLUDED.show_in_places,
    thumbnail_hash = EXCLUDED.thumbnail_hash,
    settings_version = greatest(w.settings_version, EXCLUDED.settings_version),
    mirror_source = NULL,
    created_at = EXCLUDED.created_at,
    updated_at = EXCLUDED.updated_at;

-- The allow-list wallets are `access` grants; the row carries the whole list,
-- so the world's set is replaced (with their parcel rows, by cascade).
DELETE FROM worlds.world_permissions p
USING stage s
WHERE p.world_name = s.name AND p.permission = 'access'
  AND NOT (p.address = ANY (s.access_wallets));

INSERT INTO worlds.world_permissions (world_name, permission, address, created_at, updated_at)
SELECT s.name, 'access', a, s.updated_at AT TIME ZONE 'UTC', s.updated_at AT TIME ZONE 'UTC'
FROM stage s CROSS JOIN LATERAL unnest(s.access_wallets) a
ON CONFLICT (world_name, permission, address) DO NOTHING;

DELETE FROM worlds.world_access_communities c
USING stage s
WHERE c.world_name = s.name AND NOT (c.community_id::text = ANY (s.access_communities));

INSERT INTO worlds.world_access_communities (world_name, community_id, created_at)
SELECT s.name, c::uuid, s.updated_at AT TIME ZONE 'UTC'
FROM stage s CROSS JOIN LATERAL unnest(s.access_communities) c
ON CONFLICT (world_name, community_id) DO NOTHING;
