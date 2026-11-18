-- Likes and dislikes of an imported place or of any upstream world, as in
-- user_favorite.extract.sql.
SELECT "user", entity_id, "like", user_activity, created_at, updated_at
FROM (
    SELECT lower(l."user"::text) AS "user",
           l.entity_id::text AS entity_id,
           l."like",
           l.user_activity::double precision AS user_activity,
           l.created_at AT TIME ZONE 'UTC' AS created_at,
           l.updated_at AT TIME ZONE 'UTC' AS updated_at,
           row_number() OVER (PARTITION BY lower(l."user"::text), l.entity_id::text
                              ORDER BY l.updated_at DESC) AS pick
    FROM user_likes l
    WHERE lower(l."user"::text) ~ '^0x[0-9a-f]{40}$'
      AND (EXISTS (SELECT 1 FROM places p
                   WHERE p.id = l.entity_id AND p.world IS FALSE
                     AND (p.disabled IS FALSE OR p.disabled_reason IN ('opt_out', 'moderation')))
           OR EXISTS (SELECT 1 FROM worlds w WHERE w.id = l.entity_id::text))
) l
WHERE pick = 1
