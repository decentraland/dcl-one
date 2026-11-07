-- Deployment and streaming grants with their parcel scopes (canonical "x,y";
-- a parcel that is not two integers is dropped). Every upstream parcel edit
-- bumps the grant's updated_at. Access grants ride on the world step.
SELECT g.world_name, g.permission, g.address,
       ARRAY(
           SELECT DISTINCT (btrim(split_part(pp.parcel, ',', 1))::numeric)::text || ','
                           || (btrim(split_part(pp.parcel, ',', 2))::numeric)::text
           FROM world_permission_parcels pp
           WHERE pp.permission_id = g.id
             AND pp.parcel ~ '^ *[-+]?[0-9]{1,9} *, *[-+]?[0-9]{1,9} *$'
       ) AS parcels,
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
WHERE g.rn = 1
