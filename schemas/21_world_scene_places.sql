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

-- Exact displaced bindings let legacy stream notices recover their admin
-- namespace after retirement or redeployment. Distinct incarnations remain
-- distinct evidence: a reader must refuse an ambiguous entity, never guess.
-- No FK: these recovery facts survive deletion of the source identity too.
CREATE TABLE explore.world_scene_binding (
    world_name text NOT NULL,
    entity_id text NOT NULL,
    identity uuid NOT NULL,
    CONSTRAINT world_scene_binding_pkey PRIMARY KEY (world_name, entity_id, identity)
);

CREATE FUNCTION explore.retain_world_scene_binding() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.active AND (TG_OP = 'DELETE' OR NOT NEW.active
        OR (OLD.world_name, OLD.entity_id, OLD.identity)
           IS DISTINCT FROM (NEW.world_name, NEW.entity_id, NEW.identity)) THEN
        INSERT INTO explore.world_scene_binding (world_name, entity_id, identity)
        VALUES (OLD.world_name, OLD.entity_id, OLD.identity)
        ON CONFLICT DO NOTHING;
    END IF;
    RETURN NULL;
END;
$$;
CREATE TRIGGER world_scene_binding_retained AFTER UPDATE OR DELETE ON explore.world_scene_place
FOR EACH ROW EXECUTE FUNCTION explore.retain_world_scene_binding();

-- Quota excludes retired UUID namespaces while accepting arbitrary legacy text.
CREATE INDEX world_scene_place_retired_identity_idx
    ON explore.world_scene_place (world_name, (identity::text)) WHERE NOT active;

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

-- Preserve exact current admin namespaces before the final scene retirement.
-- Worlds serializes scene changes under its world row lock. Catalogue rows
-- retain their fetch time; this is local evidence, not a mirror refresh.
CREATE FUNCTION explore.capture_world_removal_admin_keys() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE keys text[];
BEGIN
    IF OLD.active AND NOT NEW.active AND NOT EXISTS (
        SELECT 1 FROM worlds.world_scenes WHERE world_name=OLD.world_name
    ) THEN
        SELECT array_agg(identity::text ORDER BY identity) INTO keys
        FROM explore.world_scene_place WHERE world_name=OLD.world_name AND active;
        UPDATE explore.place SET removal_admin_keys=ARRAY(
            SELECT DISTINCT unnest(removal_admin_keys || keys) ORDER BY 1
        ) WHERE world AND lower(world_name)=OLD.world_name;
        UPDATE explore.place_world_local SET removal_admin_keys=ARRAY(
            SELECT DISTINCT unnest(removal_admin_keys || keys) ORDER BY 1
        ) WHERE world AND lower(world_name)=OLD.world_name;
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER world_removal_admin_keys BEFORE UPDATE OF active ON explore.world_scene_place
FOR EACH ROW EXECUTE FUNCTION explore.capture_world_removal_admin_keys();

-- An imported identity can precede its backing scene. Only a real scene
-- insertion starts a new live catalogue lifetime; queued events stay frozen.
CREATE FUNCTION explore.clear_world_removal_admin_keys() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    -- Keep the identity-before-catalogue lock order used by publication and ranking.
    PERFORM identity FROM explore.world_scene_place
    WHERE world_name=NEW.world_name AND active ORDER BY identity FOR UPDATE;
    UPDATE explore.place SET removal_admin_keys='{}'
    WHERE world AND lower(world_name)=NEW.world_name AND cardinality(removal_admin_keys)>0;
    UPDATE explore.place_world_local SET removal_admin_keys='{}'
    WHERE world AND lower(world_name)=NEW.world_name AND cardinality(removal_admin_keys)>0;
    RETURN NEW;
END;
$$;
CREATE TRIGGER world_scene_clear_removal_admin_keys AFTER INSERT ON worlds.world_scenes
FOR EACH ROW EXECUTE FUNCTION explore.clear_world_removal_admin_keys();
