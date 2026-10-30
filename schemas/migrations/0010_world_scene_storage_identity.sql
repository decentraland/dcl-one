-- World storage moves from one scope per world ('world') to one per world
-- scene (worlds.world_scene_identity, schemas/20_worlds.sql). The object pass
-- that follows creates that table and seeds one identity per scene as
-- md5(world_name || '/' || entity_id)::uuid; this step moves each world's
-- 'world' rows to the identity of the scene the world resolves to (its
-- newest, the one the authoritative host runs and the /about lists first).
-- The rows cannot be told apart by scene, and no other scene gains rows it
-- could not read before. A world with no scene keeps its rows under 'world',
-- parked: no request resolves to them. Runtime quota excludes this bucket
-- once the world has an active scene identity; the rows remain untouched.
-- Keep separate UPDATE statements: the additive migration validator deliberately
-- refuses top-level WITH statements, including data-modifying CTEs.
UPDATE worlds.world_storage t SET scene_namespace = md5(s.world_name || '/' || s.entity_id)::uuid::text FROM (SELECT DISTINCT ON (world_name) world_name, entity_id FROM worlds.world_scenes WHERE cardinality(parcels) > 0 ORDER BY world_name, created_at DESC, entity_id) s WHERE t.realm = s.world_name AND t.scene_namespace = 'world';
UPDATE worlds.player_storage t SET scene_namespace = md5(s.world_name || '/' || s.entity_id)::uuid::text FROM (SELECT DISTINCT ON (world_name) world_name, entity_id FROM worlds.world_scenes WHERE cardinality(parcels) > 0 ORDER BY world_name, created_at DESC, entity_id) s WHERE t.realm = s.world_name AND t.scene_namespace = 'world';
UPDATE worlds.env_variables t SET scene_namespace = md5(s.world_name || '/' || s.entity_id)::uuid::text FROM (SELECT DISTINCT ON (world_name) world_name, entity_id FROM worlds.world_scenes WHERE cardinality(parcels) > 0 ORDER BY world_name, created_at DESC, entity_id) s WHERE t.realm = s.world_name AND t.scene_namespace = 'world';
