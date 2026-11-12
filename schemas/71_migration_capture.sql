-- Capture progress differs from application progress: source input is durable
-- before native duplicate execution or transformed application finishes.
CREATE TABLE migration_sync.capture_sources (
    stream text PRIMARY KEY,
    generation uuid NOT NULL,
    adapter text NOT NULL DEFAULT 'places_outbox_v1' CHECK (adapter IN ('places_outbox_v1','places_hybrid_commands_v1')),
    captured_position bigint NOT NULL CHECK (captured_position >= 0),
    observed_position bigint NOT NULL CHECK (observed_position >= captured_position),
    last_contact_at timestamptz NOT NULL,
    FOREIGN KEY (stream,generation) REFERENCES migration_sync.streams(name,generation)
);

-- Immutable approved catalogue boundary for follower reads and reconciliation.
CREATE TABLE migration_sync.capture_scope (
    stream text NOT NULL,
    generation uuid NOT NULL,
    entity_id uuid NOT NULL,
    PRIMARY KEY (stream, generation, entity_id),
    FOREIGN KEY (stream, generation) REFERENCES migration_sync.streams(name, generation)
);
