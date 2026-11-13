-- The DEPLOYED scenes; the load reads their imported rows in the target.
SELECT lower(world_name) AS world_name, entity_id::text AS entity_id,
       max(updated_at) AT TIME ZONE 'UTC' AS updated_at
FROM world_scenes
WHERE status = 'DEPLOYED'
GROUP BY lower(world_name), entity_id
