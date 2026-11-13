-- Temporary, isolated player-storage command replay. Production owns acceptance;
-- fixed namespace bindings prohibit guessed remaps after a deployment changes.
CREATE TABLE migration_sync.player_streams (
    name text PRIMARY KEY CHECK (name ~ '^[A-Za-z0-9_-]{1,64}$'),
    singleton boolean NOT NULL DEFAULT true UNIQUE CHECK (singleton),
    generation uuid NOT NULL CHECK (generation<>'00000000-0000-0000-0000-000000000000'),
    baseline bigint NOT NULL CHECK (baseline>=0),
    captured bigint NOT NULL CHECK (captured>=baseline),
    applied bigint NOT NULL CHECK (applied>=baseline AND applied<=captured),
    paused boolean NOT NULL DEFAULT true,
    observed_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (name,generation)
);
CREATE TABLE migration_sync.player_scopes (
    stream text NOT NULL REFERENCES migration_sync.player_streams(name),
    world_name text NOT NULL CHECK (world_name=lower(world_name) AND world_name LIKE '%.eth'),
    place_id uuid NOT NULL CHECK (place_id<>'00000000-0000-0000-0000-000000000000'),
    scene_id text NOT NULL CHECK (length(scene_id) BETWEEN 1 AND 256),
    PRIMARY KEY (stream,world_name,place_id)
);
CREATE TABLE migration_sync.player_inbox (
    stream text NOT NULL,
    generation uuid NOT NULL,
    position bigint NOT NULL CHECK (position>0),
    command jsonb,
    unclassified boolean NOT NULL,
    status text NOT NULL DEFAULT 'pending' CHECK (status IN('pending','blocked','applied')),
    error_code text,
    PRIMARY KEY (stream,generation,position),
    FOREIGN KEY (stream,generation) REFERENCES migration_sync.player_streams(name,generation),
    CHECK ((status='blocked')=(error_code IS NOT NULL))
);
CREATE TABLE migration_sync.player_receipts (
    stream text NOT NULL,
    generation uuid NOT NULL,
    position bigint NOT NULL,
    command jsonb NOT NULL,
    completed_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (stream,generation,position),
    FOREIGN KEY (stream,generation,position) REFERENCES migration_sync.player_inbox(stream,generation,position)
);
