-- Identities keep their upstream UUID. A binding another identity already
-- holds on the node (world_scene_place_deployment_idx) stays with it: this
-- row lands retired. A re-run moves an imported identity only while the
-- node has not touched it since (its updated_at is not newer than upstream's).
INSERT INTO explore.world_scene_place AS p
    (world_name, identity, entity_id, parcels, active, created_at, updated_at)
SELECT s.world_name, s.identity,
       CASE WHEN bind.ok THEN s.entity_id END,
       s.parcels, bind.ok,
       s.created_at AT TIME ZONE 'UTC', s.updated_at AT TIME ZONE 'UTC'
FROM stage s
CROSS JOIN LATERAL (
    SELECT s.active AND NOT EXISTS (
        SELECT 1 FROM explore.world_scene_place o
        WHERE o.active AND o.world_name = s.world_name AND o.entity_id = s.entity_id
          AND o.identity <> s.identity
    ) AS ok
) bind
ON CONFLICT (identity) DO UPDATE SET
    entity_id = EXCLUDED.entity_id,
    parcels = EXCLUDED.parcels,
    active = EXCLUDED.active,
    updated_at = EXCLUDED.updated_at
WHERE p.world_name = EXCLUDED.world_name
  AND p.updated_at <= EXCLUDED.updated_at
  AND (p.entity_id, p.parcels, p.active) IS DISTINCT FROM (EXCLUDED.entity_id, EXCLUDED.parcels, EXCLUDED.active);
