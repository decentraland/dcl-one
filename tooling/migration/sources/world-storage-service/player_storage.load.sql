-- Upstream scopes rows by (world_name, place_id): world_name is the
-- lowercase world, or "main" for any Genesis City realm; place_id the Places
-- UUID the request resolved (the nil UUID where the 1775047654810 backfill
-- found none). The node scopes by (realm, scene_namespace):
--   world:   the Places UUID itself, the identity world_scene_place binds;
--            nil -> 'world', the parked legacy bucket.
--   Genesis: the canonical base parcel of that place while it is an active
--            explore.place row; else 'legacy:<place_id>', retained but
--            unreachable (a retired or unknown place has no parcel scope).
-- Rows that collapse onto one target key keep the newest; a newer target row
-- (written by the node) is never overwritten.
-- Player addresses are lowercased; one that is not an address is skipped.
INSERT INTO worlds.player_storage AS t
    (realm, scene_namespace, player_address, key, value, value_size, created_at, updated_at)
SELECT DISTINCT ON (m.realm, m.scene_namespace, m.player, m.key)
       m.realm, m.scene_namespace, m.player, m.key, m.value, greatest(coalesce(m.value_size, 0), 0),
       m.created_at AT TIME ZONE 'UTC', m.updated_at AT TIME ZONE 'UTC'
FROM (
    SELECT x.*, lower(x.player_address) AS player FROM (
    SELECT s.*,
           CASE WHEN lower(s.world_name) LIKE '%.eth' THEN lower(s.world_name) ELSE 'main' END AS realm,
           CASE
               WHEN lower(s.world_name) LIKE '%.eth' THEN
                   CASE WHEN s.place_id = '00000000-0000-0000-0000-000000000000' THEN 'world'
                        ELSE s.place_id::text END
               ELSE coalesce(
                   (SELECT (split_part(p.base_position, ',', 1)::numeric)::text || ','
                               || (split_part(p.base_position, ',', 2)::numeric)::text
                    FROM explore.place p
                    WHERE p.id = s.place_id::text AND NOT p.world AND NOT p.disabled
                      AND p.base_position ~ '^-?[0-9]{1,9},-?[0-9]{1,9}$'),
                   'legacy:' || s.place_id::text) END AS scene_namespace
    FROM stage s
    ) x
) m
WHERE m.player ~ '^0x[0-9a-f]{40}$'
ORDER BY m.realm, m.scene_namespace, m.player, m.key, m.updated_at DESC
ON CONFLICT (realm, scene_namespace, player_address, key) DO UPDATE SET
    value = EXCLUDED.value,
    value_size = EXCLUDED.value_size,
    created_at = EXCLUDED.created_at,
    updated_at = EXCLUDED.updated_at
WHERE t.updated_at <= EXCLUDED.updated_at;
