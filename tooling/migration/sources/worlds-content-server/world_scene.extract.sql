-- Every scene row, DEPLOYED or not: an UNDEPLOYED row only carries the delete
-- (no payload). One row per (lowercase world, entity): DEPLOYED wins.
SELECT
    lower(s.world_name) AS world_name,
    s.entity_id::text AS entity_id,
    s.status = 'DEPLOYED' AS deployed,
    CASE WHEN s.status = 'DEPLOYED' THEN s.deployment_auth_chain END AS deployment_auth_chain,
    CASE WHEN s.status = 'DEPLOYED' THEN s.entity END AS entity,
    lower(s.deployer) AS deployer,
    s.parcels,
    s.size,
    s.created_at AT TIME ZONE 'UTC' AS created_at,
    s.updated_at AT TIME ZONE 'UTC' AS updated_at
FROM (
    SELECT world_scenes.*,
           row_number() OVER (PARTITION BY lower(world_name), entity_id
                              ORDER BY (status = 'DEPLOYED') DESC, (world_name = lower(world_name)) DESC,
                                       updated_at DESC) AS rn
    FROM world_scenes
) s
WHERE s.rn = 1
