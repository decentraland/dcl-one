-- Temporary migration-worker state. One source owner serializes complete
-- captured transactions; target facts and the applied position commit together.
-- Full envelopes are retained for replay conflict detection, not application data.
CREATE SCHEMA migration_sync;

CREATE TABLE migration_sync.streams (
    name text PRIMARY KEY CHECK (name ~ '^[A-Za-z0-9_-]{1,64}$'),
    source text NOT NULL UNIQUE CHECK (source = 'places'),
    generation uuid NOT NULL CHECK (generation <> '00000000-0000-0000-0000-000000000000'::uuid),
    baseline_lsn pg_lsn NOT NULL,
    applied_lsn pg_lsn NOT NULL,
    paused boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (name, generation),
    CHECK (applied_lsn >= baseline_lsn)
);

CREATE TABLE migration_sync.inbox (
    stream text NOT NULL,
    generation uuid NOT NULL,
    commit_lsn pg_lsn NOT NULL,
    previous_lsn pg_lsn NOT NULL,
    payload jsonb NOT NULL CHECK (jsonb_typeof(payload) = 'object'),
    status text NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'applied', 'blocked')),
    error_code text,
    received_at timestamptz NOT NULL DEFAULT now(),
    applied_at timestamptz,
    PRIMARY KEY (stream, generation, commit_lsn),
    UNIQUE (stream, generation, previous_lsn),
    FOREIGN KEY (stream, generation) REFERENCES migration_sync.streams (name, generation),
    CHECK (commit_lsn > previous_lsn),
    CHECK ((status = 'blocked') = (error_code IS NOT NULL)),
    CHECK ((status = 'applied') = (applied_at IS NOT NULL))
);
CREATE INDEX migration_sync_inbox_status_idx ON migration_sync.inbox (stream, status, commit_lsn);

-- Keep deletion/version history even after the favorite fact is removed.
-- There is no FK to explore: a deleted place must not erase its tombstone.
CREATE TABLE migration_sync.favorite_versions (
    stream text NOT NULL,
    generation uuid NOT NULL,
    "user" text NOT NULL CHECK ("user" ~ '^0x[0-9a-f]{40}$'),
    entity_id text NOT NULL,
    commit_lsn pg_lsn NOT NULL,
    deleted boolean NOT NULL,
    PRIMARY KEY (stream, generation, "user", entity_id),
    FOREIGN KEY (stream, generation, commit_lsn)
        REFERENCES migration_sync.inbox (stream, generation, commit_lsn)
);

-- The follower writes this receipt in the native mutation transaction. The
-- coordinator reads it; captured command JSON alone never proves execution.
CREATE TABLE migration_sync.command_receipts (
    stream text NOT NULL,
    generation uuid NOT NULL,
    operation_id uuid NOT NULL,
    commit_lsn pg_lsn NOT NULL,
    command jsonb NOT NULL CHECK (jsonb_typeof(command) = 'object'),
    state jsonb NOT NULL,
    completed_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (stream, generation, operation_id),
    UNIQUE (stream, generation, commit_lsn),
    FOREIGN KEY (stream, generation, commit_lsn)
        REFERENCES migration_sync.inbox (stream, generation, commit_lsn)
);
