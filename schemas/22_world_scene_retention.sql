-- Retired world scenes: the scenes a publish replaced or an undeploy removed,
-- kept for SCENE_EVICTION_TTL_MS (default 7 days) like worlds-content-server's
-- UNDEPLOYED rows. Worlds owns this relation.
--
-- Workload: one row per replaced/undeployed local scene (the catalogue holds
-- ~2.5k active scenes; retirements are a few per publish, a few hundred a
-- week expected, unmeasured). Rows are a few KB (the entity document). Reads:
-- the blob collector (every row, per run), the comms resolver's fallback by
-- (world_name, entity_id) PK when the entity is no longer active, and the
-- eviction sweep by retired_at.
--
-- Invariants:
-- - A row is moved here in the same transaction (under the world row lock)
--   that deletes it from worlds.world_scenes; a re-publish of the same entity
--   deletes its row here, so a (world, entity) is active or retired, never both.
-- - Active-scene readers never read this table; only the collector (content
--   stays referenced until eviction) and the comms room resolver do.
-- - No place identity: a retired scene resolves with no storage scope.
-- - The FK follows the world's lifecycle (a deleted world drops its history).
CREATE TABLE worlds.retired_world_scenes (
    world_name text NOT NULL,
    entity_id text NOT NULL,
    entity jsonb NOT NULL,
    parcels text[] NOT NULL,
    mirror_source text,
    retired_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT retired_world_scenes_pkey PRIMARY KEY (world_name, entity_id),
    CONSTRAINT fk_retired_world_scenes_world FOREIGN KEY (world_name)
        REFERENCES worlds.worlds (name) ON DELETE CASCADE
);

-- The eviction sweep: `DELETE ... WHERE retired_at < $1`.
CREATE INDEX retired_world_scenes_retired_at_idx
    ON worlds.retired_world_scenes (retired_at);
