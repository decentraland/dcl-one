-- Raw sightings precede catalogue deduplication. A resumed page replaces its
-- own sightings; only a complete trusted walk can publish observations.
CREATE TABLE explore.genesis_place_page (
    pass_started_at timestamp without time zone NOT NULL,
    page_offset bigint NOT NULL CHECK (page_offset >= 0),
    page_rows bigint NOT NULL CHECK (page_rows >= 0),
    reported_total bigint,
    PRIMARY KEY (pass_started_at, page_offset)
);

CREATE TABLE explore.genesis_place_sighting (
    pass_started_at timestamp without time zone NOT NULL,
    page_offset bigint NOT NULL,
    ordinal bigint NOT NULL,
    public_id text NOT NULL,
    base_position text,
    deployment_id text,
    PRIMARY KEY (pass_started_at, page_offset, ordinal),
    FOREIGN KEY (pass_started_at, page_offset)
        REFERENCES explore.genesis_place_page (pass_started_at, page_offset) ON DELETE CASCADE
);

CREATE TABLE explore.genesis_place_observed (
    public_id text PRIMARY KEY,
    base_position text NOT NULL,
    deployment_id text NOT NULL,
    UNIQUE (base_position, deployment_id)
);

-- Retain public ownership after its proof expires so another place cannot
-- take over historical interactions. Catalogue rows have their own lifecycle.
CREATE TABLE explore.genesis_place_claim (
    public_id text PRIMARY KEY,
    local_id text NOT NULL CHECK (local_id <> public_id)
);
CREATE INDEX genesis_place_claim_local_idx ON explore.genesis_place_claim (local_id);

-- Operator decisions follow the reserved pair even while discovery reverts
-- to separate catalogue rows. Interaction facts still use the live proof.
CREATE FUNCTION explore.genesis_curation_key(input_id text) RETURNS text
LANGUAGE sql STABLE STRICT AS $$
    SELECT COALESCE((SELECT local_id FROM explore.genesis_place_claim
                     WHERE public_id = input_id), input_id);
$$;

CREATE FUNCTION explore.project_genesis_place_curation(local_ids text[], include_public boolean) RETURNS void
LANGUAGE sql AS $$
    UPDATE explore.place p SET
        disabled = COALESCE(m.disabled, p.disabled),
        highlighted = COALESCE(m.highlighted, p.highlighted),
        content_rating = COALESCE(m.content_rating, p.content_rating),
        raw = p.raw || COALESCE(r.overlay, '{}'::jsonb) ||
            CASE m.disabled WHEN true THEN jsonb_build_object(
                'disabled_at', to_char(m.disabled_at, 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"'),
                'disabled_reason', m.reason)
            WHEN false THEN '{"disabled_at":null,"disabled_reason":null}'::jsonb
            ELSE '{}'::jsonb END
    FROM (SELECT DISTINCT local_id AS id FROM explore.genesis_place_claim WHERE local_id = ANY(local_ids)) target
    LEFT JOIN explore.place_moderation m ON m.place_id = target.id AND m.set_by <> 'migration:places'
    LEFT JOIN explore.place_ranking r ON r.place_id = target.id AND r.set_by <> 'migration:places'
    WHERE (m.place_id IS NOT NULL OR r.place_id IS NOT NULL)
        AND (p.id = target.id OR (include_public AND p.id IN (
            SELECT public_id FROM explore.genesis_place_claim WHERE local_id = target.id)));
$$;

-- This runs after the World insert guard, which took the per-UUID lock
-- or verified that the Places import holds the shared claim lock.
CREATE FUNCTION explore.guard_world_scene_genesis_reservation() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM explore.genesis_place_claim
        WHERE public_id = NEW.identity::text OR local_id = NEW.identity::text
    ) THEN
        RAISE unique_violation USING MESSAGE = 'world scene identity is reserved by a Genesis place';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER world_scene_identity_reserved_genesis BEFORE INSERT ON explore.world_scene_place
FOR EACH ROW EXECUTE FUNCTION explore.guard_world_scene_genesis_reservation();

CREATE INDEX place_local_genesis_deployment_idx
    ON explore.place (base_position, (raw->>'deployment_id'))
    WHERE origin = 'local' AND NOT world AND raw->>'source' = 'content';

CREATE VIEW explore.genesis_place_candidate AS
SELECT l.id AS local_id, o.public_id, o.deployment_id
FROM explore.genesis_place_observed o
JOIN explore.place p ON p.id = o.public_id AND p.origin = 'upstream' AND NOT p.world
    AND p.base_position = o.base_position AND p.raw->>'deployment_id' = o.deployment_id
    AND p.raw->>'world' = 'false' AND p.world_name IS NULL
    AND COALESCE(p.raw->>'disabled', 'false') = 'false'
JOIN content.active_pointers a ON a.pointer = o.base_position AND a.entity_id = o.deployment_id
    AND a.entity_type = 'scene'
JOIN content.local_entities e ON e.entity_id = a.entity_id AND e.tombstoned_at IS NULL
JOIN explore.place l ON l.origin = 'local' AND NOT l.world AND l.raw->>'source' = 'content'
    AND l.base_position = o.base_position AND l.raw->>'deployment_id' = o.deployment_id
    AND l.id <> p.id
WHERE EXISTS (
    SELECT 1 FROM explore.mirror_state
    WHERE mirror = 'places' AND source_url = 'https://places.decentraland.org'
)
AND NOT EXISTS (
    SELECT 1 FROM explore.place other
    WHERE other.origin = 'local' AND NOT other.world AND other.raw->>'source' = 'content'
        AND other.base_position = o.base_position AND other.raw->>'deployment_id' = o.deployment_id
        AND other.id <> l.id
);

CREATE VIEW explore.genesis_place_alias AS
SELECT c.local_id, c.public_id, c.deployment_id
FROM explore.genesis_place_candidate c
JOIN explore.genesis_place_claim a ON a.public_id = c.public_id AND a.local_id = c.local_id;

-- A World reservation retains the durable interaction key on expiry. Genesis
-- redirects only while current evidence says the two catalogue rows agree.
CREATE FUNCTION explore.scene_interaction_entity_id(input_id text) RETURNS text
LANGUAGE plpgsql STABLE STRICT AS $$
DECLARE
    normalized text := regexp_replace(input_id, '^urn:uuid:', '', 'i');
BEGIN
    IF NOT pg_input_is_valid(normalized, 'uuid') THEN
        RETURN input_id;
    END IF;
    RETURN COALESCE((
        SELECT identity::text FROM explore.world_scene_public_id
        WHERE public_id = normalized::uuid
    ), (
        SELECT local_id FROM explore.genesis_place_alias WHERE public_id = normalized::uuid::text
    ), input_id);
END;
$$;
