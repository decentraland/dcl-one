-- Binds each imported scene the places source left unbound (its exact
-- places.deployment_id binding wins and is never touched here), as the deploy
-- transaction would (explore::places::scene_identity): a retired identity of
-- the same world (the places source writes every upstream world-scene place
-- UUID, retired when it holds no deployment) whose parcels overlap this scene
-- and no other local scene of the world is reactivated, so the scene keeps its
-- upstream UUID and storage. Otherwise (no such identity, ambiguous overlap)
-- md5(world/entity), the node's legacy derivation. A scene already bound keeps
-- its identity, so a re-run changes nothing.

WITH scene AS (
    SELECT ws.world_name, ws.entity_id, ws.parcels
    FROM stage st
    JOIN worlds.world_scenes ws
      ON ws.world_name = st.world_name AND ws.entity_id = st.entity_id AND ws.mirror_source IS NULL
    WHERE cardinality(ws.parcels) > 0
      AND NOT EXISTS (SELECT 1 FROM explore.world_scene_place p
                      WHERE p.active AND p.world_name = ws.world_name AND p.entity_id = ws.entity_id)
), place AS (
    SELECT p.world_name, p.identity, p.parcels AS positions
    FROM explore.world_scene_place p
    WHERE NOT p.active
      AND p.world_name IN (SELECT world_name FROM scene)
), candidate AS (
    SELECT sc.world_name, sc.entity_id, pl.identity
    FROM scene sc
    JOIN place pl ON pl.world_name = sc.world_name AND pl.positions && sc.parcels
    WHERE (SELECT count(*) FROM worlds.world_scenes o
           WHERE o.world_name = pl.world_name AND o.mirror_source IS NULL
             AND o.parcels && pl.positions) = 1
      AND NOT EXISTS (SELECT 1 FROM explore.world_scene_place p
                      WHERE p.identity = pl.identity AND (p.active OR p.world_name <> sc.world_name))
), chosen AS (
    SELECT world_name, entity_id, min(identity::text)::uuid AS identity
    FROM candidate
    GROUP BY world_name, entity_id
    HAVING count(*) = 1
)
INSERT INTO explore.world_scene_place AS p (world_name, identity, entity_id, parcels, active)
SELECT sc.world_name,
       coalesce(c.identity, md5(sc.world_name || '/' || sc.entity_id)::uuid),
       sc.entity_id, sc.parcels, true
FROM scene sc
LEFT JOIN chosen c ON c.world_name = sc.world_name AND c.entity_id = sc.entity_id
ON CONFLICT (identity) DO UPDATE SET
    entity_id = EXCLUDED.entity_id,
    parcels = EXCLUDED.parcels,
    active = true,
    updated_at = timezone('UTC', now())
WHERE p.world_name = EXCLUDED.world_name AND NOT p.active;
