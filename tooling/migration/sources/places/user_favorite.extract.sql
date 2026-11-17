-- Favorites of an imported place, or of any upstream world (keyed by its
-- lowercase name, the id the node's world rows use): a favorite of a place
-- that is not imported (overwritten or undeployed scenes, world-scene place
-- UUIDs from before upstream keyed worlds by name) would match nothing.
SELECT "user", entity_id, user_activity, created_at
FROM (
    SELECT lower(f."user"::text) AS "user",
           f.entity_id::text AS entity_id,
           f.user_activity::double precision AS user_activity,
           f.created_at AT TIME ZONE 'UTC' AS created_at,
           row_number() OVER (PARTITION BY lower(f."user"::text), f.entity_id::text
                              ORDER BY f.created_at) AS pick
    FROM user_favorites f
    WHERE lower(f."user"::text) ~ '^0x[0-9a-f]{40}$'
      AND (EXISTS (SELECT 1 FROM places p
                   WHERE p.id = f.entity_id AND p.world IS FALSE
                     AND (p.disabled IS FALSE OR p.disabled_reason IN ('opt_out', 'moderation')))
           OR EXISTS (SELECT 1 FROM worlds w WHERE w.id = f.entity_id::text))
) f
WHERE pick = 1
