-- Temporary, bounded projection of production owner/quota outcomes. Payloads
-- hold only the two mapped facts, never the full source rows or job machinery.
CREATE TABLE migration_sync.worlds_streams (
    name text PRIMARY KEY CHECK (name ~ '^[A-Za-z0-9_-]{1,64}$'),
    singleton boolean NOT NULL DEFAULT true UNIQUE CHECK (singleton),
    generation uuid NOT NULL CHECK (generation <> '00000000-0000-0000-0000-000000000000'),
    baseline_position bigint NOT NULL CHECK (baseline_position >= 0),
    captured_position bigint NOT NULL,
    applied_position bigint NOT NULL,
    paused boolean NOT NULL DEFAULT true,
    error_code text,
    last_contact_at timestamptz NOT NULL DEFAULT now(),
    CHECK (captured_position >= applied_position AND applied_position >= baseline_position),
    UNIQUE (name,generation)
);
CREATE TABLE migration_sync.worlds_scope (
    stream text NOT NULL REFERENCES migration_sync.worlds_streams(name),
    world_name text NOT NULL CHECK (world_name = lower(world_name) AND world_name <> ''),
    PRIMARY KEY (stream,world_name)
);
CREATE TABLE migration_sync.worlds_wallet_scope (
    stream text NOT NULL REFERENCES migration_sync.worlds_streams(name),
    wallet text NOT NULL CHECK (wallet ~ '^0x[0-9a-f]{40}$'),
    PRIMARY KEY (stream,wallet)
);
CREATE TABLE migration_sync.worlds_inbox (
    stream text NOT NULL,
    generation uuid NOT NULL,
    position bigint NOT NULL CHECK (position > 0),
    changes jsonb NOT NULL CHECK (jsonb_typeof(changes)='array'),
    applied_at timestamptz,
    PRIMARY KEY (stream,generation,position),
    FOREIGN KEY (stream,generation) REFERENCES migration_sync.worlds_streams(name,generation)
);
