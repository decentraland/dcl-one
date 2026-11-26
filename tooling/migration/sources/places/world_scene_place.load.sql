-- Identity changes also retain exact displaced bindings through the schema
-- trigger; replay preserves that recovery evidence without duplicating facts.
-- Identities keep their upstream UUID. A binding another identity already
-- holds on the node (world_scene_place_deployment_idx) stays with it: this
-- row lands retired unless it would conflict with an exact mirrored scene
-- awaiting verification. A re-run moves an imported identity only while the
-- node has not touched it since (its updated_at is not newer than upstream's).
-- Serialize against mirror claims before checking UUID ownership.
SELECT pg_advisory_xact_lock_shared(hashtextextended('explore.scene_interaction_import', 0));
SET LOCAL explore.scene_identity_import = 'on';

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
WHERE NOT EXISTS (
    SELECT 1 FROM explore.world_scene_public_id claim
    WHERE claim.public_id = s.identity AND claim.identity <> s.identity
)
AND NOT (
    s.active AND EXISTS (
        SELECT 1 FROM explore.world_scene_place p
        JOIN worlds.world_scenes ws ON ws.world_name = p.world_name AND ws.entity_id = p.entity_id
        JOIN worlds.worlds w ON w.name = p.world_name AND w.mirror_source = ws.mirror_source
        WHERE p.active AND p.world_name = s.world_name AND p.entity_id = s.entity_id
          AND p.identity <> s.identity
    )
)
ON CONFLICT (identity) DO UPDATE SET
    entity_id = EXCLUDED.entity_id,
    parcels = EXCLUDED.parcels,
    active = EXCLUDED.active,
    updated_at = EXCLUDED.updated_at
WHERE p.world_name = EXCLUDED.world_name
  AND p.updated_at <= EXCLUDED.updated_at
  AND (p.entity_id, p.parcels, p.active) IS DISTINCT FROM (EXCLUDED.entity_id, EXCLUDED.parcels, EXCLUDED.active);
