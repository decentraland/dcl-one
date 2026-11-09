BEGIN;
-- Copy active and retained scene bans to stable world/base keys, then remove
-- every mapped legacy alias. Existing keys keep the newest author/time.
-- Genesis and unmapped rows stay. Stop old replicas before migration: old
-- readers cannot enforce stable keys. Rollback needs the matching backup.
WITH mapping AS (
        SELECT ws.entity_id, ws.world_name || ':' || base.canonical AS ban_key
          FROM (SELECT world_name, entity_id, parcels, entity FROM worlds.world_scenes
                UNION ALL SELECT world_name, entity_id, parcels, entity FROM worlds.retired_world_scenes) ws
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

), candidates AS (
    SELECT m.ban_key, b.banned_address, b.banned_by, b.banned_at
      FROM social.scene_bans b JOIN mapping m ON m.entity_id = b.place_id
), copied AS (
    INSERT INTO social.scene_bans (place_id, banned_address, banned_by, banned_at)
    SELECT DISTINCT ON (ban_key, banned_address) ban_key, banned_address, banned_by, banned_at
      FROM candidates ORDER BY ban_key, banned_address, banned_at DESC, banned_by
    ON CONFLICT (place_id, banned_address) DO UPDATE
        SET banned_by = EXCLUDED.banned_by, banned_at = EXCLUDED.banned_at
        WHERE EXCLUDED.banned_at > scene_bans.banned_at
    RETURNING place_id
)
DELETE FROM social.scene_bans b USING mapping m
 WHERE b.place_id = m.entity_id AND b.place_id <> m.ban_key;

COMMIT;
