-- ── scope (identical in the three world-storage-service loads) ──
-- Upstream scopes rows by (world_name, place_id): world_name is the
-- lowercase world, or "main" for any Genesis City realm; place_id the Places
-- UUID the request resolved (the nil UUID where the 1775047654810 backfill
-- found none). The node scopes by (realm, scene_namespace):
--   world:   the Places UUID itself, the identity world_scene_place binds;
--            nil -> 'world', the parked legacy bucket.
--   Genesis: the effective base parcel of the active place's scene, the rule
--            the node's resolver applies (ParcelSet::effective_base): the
--            declared base when it is one of the scene's parcels, else its
--            least parcel by (x, y). The scene is the active content
--            deployment on the place's parcels declaring the place's base,
--            else the place's own positions; a disabled or unknown place
--            maps to 'legacy:<place_id>', retained but unreachable.

-- A parcel as the node's Parcel::parse reads it ([x, y], i32), else NULL.
CREATE OR REPLACE FUNCTION pg_temp.wss_parcel(raw text) RETURNS int[]
LANGUAGE sql IMMUTABLE AS $f$
  SELECT ARRAY[m[1]::numeric::int, m[2]::numeric::int]
  FROM regexp_match(raw, '^\s*(-?[0-9]+)\s*,\s*(-?[0-9]+)\s*$') m
  WHERE m[1]::numeric BETWEEN -2147483648 AND 2147483647
    AND m[2]::numeric BETWEEN -2147483648 AND 2147483647
$f$;

-- ParcelSet::effective_base, canonical "x,y"; NULL for an empty footprint.
CREATE OR REPLACE FUNCTION pg_temp.wss_effective_base(parcels text[], declared text) RETURNS text
LANGUAGE sql IMMUTABLE AS $f$
  WITH f AS (SELECT DISTINCT pg_temp.wss_parcel(p) AS c FROM unnest(parcels) p)
  SELECT b[1] || ',' || b[2]
  FROM (SELECT coalesce(
          (SELECT f.c FROM f WHERE f.c = pg_temp.wss_parcel(declared)),
          (SELECT f.c FROM f WHERE f.c IS NOT NULL ORDER BY f.c[1], f.c[2] LIMIT 1)) AS b) x
$f$;

CREATE TEMP TABLE wss_scope ON COMMIT DROP AS
SELECT s.world_name, s.place_id,
       CASE WHEN lower(s.world_name) LIKE '%.eth' THEN lower(s.world_name) ELSE 'main' END AS realm,
       CASE WHEN lower(s.world_name) LIKE '%.eth' THEN
                 CASE WHEN s.place_id = '00000000-0000-0000-0000-000000000000' THEN 'world'
                      ELSE s.place_id::text END
            ELSE coalesce(g.base, 'legacy:' || s.place_id::text) END AS scene_namespace
FROM (SELECT DISTINCT world_name, place_id FROM stage) s
LEFT JOIN LATERAL (
    SELECT coalesce(
               (SELECT pg_temp.wss_effective_base(d.entity_pointers, d.entity_metadata #>> '{scene,base}')
                FROM content.active_pointers ap
                JOIN content.deployments d ON d.entity_id = ap.entity_id
                WHERE ap.pointer = ANY (fp.canonical)
                  AND d.entity_type = 'scene' AND d.deleter_deployment IS NULL
                  AND pg_temp.wss_parcel(d.entity_metadata #>> '{scene,base}') = pg_temp.wss_parcel(p.base_position)
                ORDER BY d.entity_timestamp DESC, d.entity_id
                LIMIT 1),
               pg_temp.wss_effective_base(fp.parcels, p.base_position)) AS base
    FROM explore.place p
    CROSS JOIN LATERAL (
        SELECT pos.parcels,
               ARRAY(SELECT c[1] || ',' || c[2]
                     FROM (SELECT pg_temp.wss_parcel(x) AS c FROM unnest(pos.parcels) x) q
                     WHERE c IS NOT NULL) AS canonical
        FROM (SELECT CASE WHEN jsonb_typeof(p.raw -> 'positions') = 'array'
                               AND jsonb_array_length(p.raw -> 'positions') > 0
                          THEN ARRAY(SELECT jsonb_array_elements_text(p.raw -> 'positions'))
                          ELSE ARRAY[p.base_position] END AS parcels) pos
    ) fp
    WHERE p.id = s.place_id::text AND NOT p.world AND NOT p.disabled
) g ON lower(s.world_name) NOT LIKE '%.eth';
-- ── end scope ──

-- Rows that collapse onto one target key keep the newest; a newer target row
-- (written by the node) is never overwritten. value_enc is the upstream
-- ciphertext, unchanged (see source.toml notes). Every upstream key whose
-- target row this batch wrote is recorded with the updated_at it holds, for
-- env_variable_reconcile.
WITH written AS (
    INSERT INTO worlds.env_variables AS t
        (realm, scene_namespace, key, value_enc, value_size, created_at, updated_at)
    SELECT DISTINCT ON (sc.realm, sc.scene_namespace, s.key)
           sc.realm, sc.scene_namespace, s.key, s.value_enc, greatest(coalesce(s.value_size, 0), 0),
           s.created_at AT TIME ZONE 'UTC', s.updated_at AT TIME ZONE 'UTC'
    FROM stage s JOIN wss_scope sc USING (world_name, place_id)
    ORDER BY sc.realm, sc.scene_namespace, s.key, s.updated_at DESC
    ON CONFLICT (realm, scene_namespace, key) DO UPDATE SET
        value_enc = EXCLUDED.value_enc,
        value_size = EXCLUDED.value_size,
        created_at = EXCLUDED.created_at,
        updated_at = EXCLUDED.updated_at
    WHERE t.updated_at <= EXCLUDED.updated_at
    RETURNING t.realm, t.scene_namespace, t.key, t.updated_at
)
INSERT INTO dcl_migration.wss_env_variable_loaded AS l
    (world_name, place_id, key, realm, scene_namespace, updated_at)
SELECT s.world_name, s.place_id, s.key, w.realm, w.scene_namespace, w.updated_at
FROM stage s JOIN wss_scope sc USING (world_name, place_id)
JOIN written w ON w.realm = sc.realm AND w.scene_namespace = sc.scene_namespace AND w.key = s.key
ON CONFLICT (world_name, place_id, key) DO UPDATE SET
    realm = EXCLUDED.realm, scene_namespace = EXCLUDED.scene_namespace, updated_at = EXCLUDED.updated_at;

DROP TABLE wss_scope;
