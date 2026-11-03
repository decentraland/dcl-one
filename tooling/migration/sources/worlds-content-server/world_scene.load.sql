-- Retire the identities of the scenes this batch deletes (before deleting
-- them), as the node's deploy/undeploy transaction does. Only those: an
-- identity the places source bound to a scene a later batch imports stays.
UPDATE explore.world_scene_place i
SET active = false, entity_id = NULL, updated_at = timezone('UTC', now())
FROM worlds.world_scenes t
WHERE i.active AND i.world_name = t.world_name AND i.entity_id = t.entity_id
  AND (
    (t.mirror_source IS NULL
     AND EXISTS (SELECT 1 FROM stage s
                 WHERE NOT s.deployed AND s.world_name = t.world_name AND s.entity_id = t.entity_id))
    OR (t.mirror_source IS NOT NULL
        AND EXISTS (SELECT 1 FROM stage s WHERE s.deployed AND s.world_name = t.world_name)));

-- An upstream undeploy removes the local row (never a mirrored one).
DELETE FROM worlds.world_scenes t
USING stage s
WHERE NOT s.deployed AND t.world_name = s.world_name AND t.entity_id = s.entity_id
  AND t.mirror_source IS NULL;

-- A world with an imported local scene drops its mirrored scenes, as a local
-- publish does: the mirror never refreshes a world holding a local scene.
DELETE FROM worlds.world_scenes t
USING (SELECT DISTINCT world_name FROM stage WHERE deployed) s
WHERE t.world_name = s.world_name AND t.mirror_source IS NOT NULL;

INSERT INTO worlds.world_scenes AS t
    (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size,
     mirror_source, created_at, updated_at)
SELECT s.world_name, s.entity_id, s.deployment_auth_chain, s.entity, s.deployer,
       coalesce(s.parcels, '{}'), greatest(coalesce(s.size, 0), 0), NULL,
       s.created_at AT TIME ZONE 'UTC', s.updated_at AT TIME ZONE 'UTC'
FROM stage s
JOIN worlds.worlds w ON w.name = s.world_name
WHERE s.deployed
ON CONFLICT (world_name, entity_id) DO UPDATE SET
    deployment_auth_chain = EXCLUDED.deployment_auth_chain,
    entity = EXCLUDED.entity,
    deployer = EXCLUDED.deployer,
    parcels = EXCLUDED.parcels,
    size = EXCLUDED.size,
    mirror_source = NULL,
    created_at = EXCLUDED.created_at,
    updated_at = EXCLUDED.updated_at;
