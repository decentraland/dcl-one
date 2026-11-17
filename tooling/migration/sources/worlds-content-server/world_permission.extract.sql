-- Deployment and streaming grants with their parcel scopes (canonical "x,y";
-- unrepresentable restrictions fail the load instead of widening a grant). Every upstream parcel edit
-- bumps the grant's updated_at. Access grants ride on the world step.
SELECT g.world_name, g.permission, g.address,
       scopes.parcels, scopes.invalid_parcels,
       g.created_at, g.updated_at
FROM (
    SELECT id,
           lower(world_name) AS world_name,
           permission_type::text AS permission,
           lower(address) AS address,
           created_at AT TIME ZONE 'UTC' AS created_at,
           updated_at AT TIME ZONE 'UTC' AS updated_at,
           row_number() OVER (PARTITION BY lower(world_name), permission_type, lower(address)
                              ORDER BY updated_at DESC, id) AS rn
    FROM world_permissions
    WHERE permission_type IN ('deployment', 'streaming')
      AND lower(address) ~ '^0x[0-9a-f]{40}$'
) g
CROSS JOIN LATERAL (
    SELECT COALESCE(array_agg(DISTINCT canonical) FILTER (WHERE canonical IS NOT NULL), '{}') AS parcels,
           count(*) FILTER (WHERE canonical IS NULL)::integer AS invalid_parcels
    FROM (
        SELECT CASE WHEN x BETWEEN -2147483648 AND 2147483647
                          AND y BETWEEN -2147483648 AND 2147483647
                    THEN x::integer::text || ',' || y::integer::text END AS canonical
        FROM (
            SELECT CASE WHEN pp.parcel ~ '^ *[-+]?[0-9]{1,10} *, *[-+]?[0-9]{1,10} *$'
                        THEN btrim(split_part(pp.parcel, ',', 1))::numeric END AS x,
                   CASE WHEN pp.parcel ~ '^ *[-+]?[0-9]{1,10} *, *[-+]?[0-9]{1,10} *$'
                        THEN btrim(split_part(pp.parcel, ',', 2))::numeric END AS y
            FROM world_permission_parcels pp
            WHERE pp.permission_id = g.id
        ) coordinates
    ) canonical_scopes
) scopes
WHERE g.rn = 1
