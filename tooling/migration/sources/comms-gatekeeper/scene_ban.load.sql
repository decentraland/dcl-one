-- Resolve the upstream Places UUID through the imported active catalogue.
-- A missing or ambiguous identity must stop cutover, never silently drop a ban/grant.
CREATE TEMP TABLE migration_scene_ban_identity ON COMMIT DROP AS
WITH candidates AS (
    SELECT DISTINCT s.place_id AS source_id, d.entity_id AS target_id
    FROM stage s
    JOIN explore.place p ON p.id = s.place_id AND NOT p.world
    JOIN content.deployments d ON d.entity_id = p.raw->>'deployment_id'
        AND d.entity_type = 'scene' AND d.deleter_deployment IS NULL
    WHERE EXISTS (SELECT 1 FROM content.active_pointers a WHERE a.entity_id = d.entity_id)
    UNION
    SELECT DISTINCT s.place_id, ws.world_name || ':' || base.canonical
    FROM stage s
    JOIN explore.world_scene_place i ON i.identity::text = lower(s.place_id) AND i.active
    JOIN worlds.world_scenes ws ON ws.world_name = i.world_name AND ws.entity_id = i.entity_id
         CROSS JOIN LATERAL (
           WITH normalized AS (
             -- Mirror Parcel::parse for both sources. Strip leading zeros
             -- before bounding casts: arbitrarily long zero padding is valid,
             -- whereas an oversized nonzero magnitude must never overflow.
             SELECT declared,
                    regexp_replace(m[1], '^(-?)0*([0-9])', '\1\2') AS x,
                    regexp_replace(m[2], '^(-?)0*([0-9])', '\1\2') AS y
               FROM (
                 SELECT p AS pointer, false AS declared FROM unnest(ws.parcels) p
                 UNION ALL
                 SELECT ws.entity #>> '{metadata,scene,base}', true
               ) inputs,
                    -- Rust str::trim uses Unicode White_Space, independent of the database locale.
                    -- PostgreSQL \s excludes characters such as NBSP on some locales.
                    regexp_match(pointer, U&'^[\0009-\000D\0020\0085\00A0\1680\2000-\200A\2028\2029\202F\205F\3000]*(-?[0-9]+)[\0009-\000D\0020\0085\00A0\1680\2000-\200A\2028\2029\202F\205F\3000]*,[\0009-\000D\0020\0085\00A0\1680\2000-\200A\2028\2029\202F\205F\3000]*(-?[0-9]+)[\0009-\000D\0020\0085\00A0\1680\2000-\200A\2028\2029\202F\205F\3000]*$') m
           ), bounded AS (
             SELECT declared,
                    CASE WHEN length(x) <= 11 THEN x::bigint END AS x,
                    CASE WHEN length(y) <= 11 THEN y::bigint END AS y
               FROM normalized
           ), coordinates AS (
             SELECT declared, x, y, x::text || ',' || y::text AS canonical
               FROM bounded
              WHERE x BETWEEN -2147483648 AND 2147483647
                AND y BETWEEN -2147483648 AND 2147483647
           )
           SELECT f.canonical
             FROM coordinates f
            WHERE NOT f.declared
            ORDER BY f.canonical IS DISTINCT FROM (
                       SELECT canonical FROM coordinates WHERE declared),
                     f.x, f.y
            LIMIT 1
         ) base
)
SELECT source_id, min(target_id) AS target_id, count(*) AS candidates
FROM candidates GROUP BY source_id;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM stage s LEFT JOIN migration_scene_ban_identity m ON m.source_id=s.place_id
               WHERE m.target_id IS NULL OR m.candidates <> 1) THEN
        RAISE EXCEPTION 'unresolved or ambiguous comms scene_ban Places identity; import the active catalogue and repair source identities before cutover';
    END IF;
END $$;

INSERT INTO social.scene_bans (place_id, banned_address, banned_by, banned_at)
SELECT DISTINCT ON (m.target_id, banned_address) m.target_id, banned_address, banned_by, banned_at AT TIME ZONE 'UTC'
FROM stage s JOIN migration_scene_ban_identity m ON m.source_id=s.place_id
ORDER BY m.target_id, banned_address, banned_at, s.place_id, banned_by
ON CONFLICT (place_id, banned_address) DO UPDATE
SET banned_by = EXCLUDED.banned_by,
    banned_at = EXCLUDED.banned_at;

DROP TABLE migration_scene_ban_identity;
