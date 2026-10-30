-- Shared world-scene place identity. Places owns this relation; worlds calls
-- explore::places::scene_identity in the deployment transaction under the
-- world row lock. Discovery and storage resolve this exact (world, entity)
-- association, never an inferred overlap on the read path.
-- UUIDs survive single-overlap redeploys. Retired UUIDs and their storage stay;
-- no FK to world_scenes/worlds: deleting a deployment must not delete history.
CREATE TABLE explore.world_scene_place (
    world_name text NOT NULL,
    identity uuid NOT NULL,
    entity_id text,
    parcels text[] NOT NULL,
    active boolean NOT NULL DEFAULT true,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT world_scene_place_pkey PRIMARY KEY (identity),
    CONSTRAINT world_scene_place_world_lowercase CHECK (world_name = lower(world_name) AND world_name <> ''),
    CONSTRAINT world_scene_place_parcels_nonempty CHECK (cardinality(parcels) > 0),
    CONSTRAINT world_scene_place_deployment CHECK (active = (entity_id IS NOT NULL))
);

-- One place per active deployment, and the exact resolver/discovery lookup.
-- No FK: inactive records retain UUIDs after their deployment is deleted.
CREATE UNIQUE INDEX world_scene_place_deployment_idx
    ON explore.world_scene_place (world_name, entity_id) WHERE active;
-- The unique index's world prefix also bounds overlap probes, orphan sweeps
-- and the parked-scope quota check to the active scenes in one world.

-- Upgrade copy, NOT a recovery/rebuild source: this executes whenever the
-- shared table is newly created, not once for the lifetime of the database.
-- After cutover the legacy table is stale. Never drop/recreate the shared table
-- or recover it by running schema migrate; restore authoritative shared-place
-- rows together with deployments/storage from backup/PITR (docs/readiness.md).
-- Object-pass seed: runs only when the shared table is created, after schema20
-- bootstraps the old identity table on a pre-0010 database. Copy every UUID,
-- including retired scopes, so storage keys and quota exclusions do not change.
-- Match the complete footprint in the same world; ambiguous bindings abort the
-- upgrade rather than assigning another scene's data. Missing deployments
-- retire their existing UUID. Subsequent migrations never replay this copy.
INSERT INTO explore.world_scene_place
    (world_name, identity, entity_id, parcels, active, created_at, updated_at)
SELECT world_name, identity, entity_id, parcels, entity_id IS NOT NULL, created_at, updated_at
FROM (
    SELECT i.*, CASE WHEN i.active THEN (
        SELECT s.entity_id FROM worlds.world_scenes s
        WHERE s.world_name = i.world_name
          AND s.parcels @> i.parcels AND s.parcels <@ i.parcels
    ) END AS entity_id
    FROM worlds.world_scene_identity i
) existing;
