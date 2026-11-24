-- Catalyst v2: explore.api domain: the explorer-api surface unity-explorer hits
-- at boot (realm-provider, dcl-lists, config denylist, feature flags, the
-- api.decentraland.org tile grid; the asset-bundle registry reads are a cached
-- upstream reader with no table). Derived from
-- catalyrst/crates/catalyrst-places/migrations/0001_lists.sql (lists) and the
-- file/asset-backed state of catalyrst-explorer-api and catalyrst-map.
--
-- Mirrored rows are the upstream's; the local overlay lives in the *_local
-- tables and is never touched by a mirror pass, so a mirror can neither
-- overwrite nor delete what an operator set here. Every mirror records its
-- passes in mirror_passes: a row whose completed_at is at or after started_at
-- is a complete pass, and the cursor is what the next pass resumes from.

CREATE SCHEMA explore_api;

CREATE TABLE explore_api.mirror_passes (
    mirror text PRIMARY KEY,
    passes bigint NOT NULL DEFAULT 0,
    items bigint NOT NULL DEFAULT 0,
    cursor text,
    started_at timestamp without time zone,
    completed_at timestamp without time zone,
    last_error text
);

-- dcl-lists: POST /pois and POST /banned-names, one row per value.
CREATE TABLE explore_api.lists (
    list text NOT NULL,
    value text NOT NULL,
    first_seen_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    PRIMARY KEY (list, value)
);

-- present = true adds a value the upstream lacks; false hides a mirrored one.
CREATE TABLE explore_api.lists_local (
    list text NOT NULL,
    value text NOT NULL,
    present boolean NOT NULL,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    PRIMARY KEY (list, value)
);

-- config.decentraland.org/denylist.json users[].wallet; keyed lowercase, served as received.
CREATE TABLE explore_api.denylist_users (
    wallet text PRIMARY KEY,
    wallet_raw text NOT NULL,
    first_seen_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now())
);

CREATE TABLE explore_api.denylist_users_local (
    wallet text PRIMARY KEY,
    present boolean NOT NULL,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now())
);

-- feature-flags.decentraland.org/<app>.json as received (flags + variants).
CREATE TABLE explore_api.feature_flags (
    app text PRIMARY KEY,
    document jsonb NOT NULL,
    etag text,
    fetched_at timestamp without time zone NOT NULL
);

CREATE TABLE explore_api.feature_flags_local (
    app text NOT NULL,
    name text NOT NULL,
    value boolean NOT NULL,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    PRIMARY KEY (app, name)
);

-- The explorer map's tile grid (about 92k tiles, 37 MB of JSON), built from
-- the land-authz and marketplace indexes (DATABASE.md "Map grid build
-- workload") and gzip-compressed as one row so a restart renders map.png
-- before the first pass; the row is the grid's revision: last_updated_at
-- is the /v2/tiles/info stamp, moved only when the body changes; etag is the
-- hex SHA-256 of the stored body (a row the replaced mirror wrote holds the
-- upstream's ETag); fetched_at is the build time.
CREATE TABLE explore_api.map_grid (
    id smallint PRIMARY KEY CHECK (id = 1),
    last_updated_at bigint NOT NULL,
    etag text,
    tile_count integer NOT NULL,
    body_gzip bytea NOT NULL,
    fetched_at timestamp without time zone NOT NULL
);

-- /auth-api, the sign-in relay (DATABASE.md "Auth API relay state workload"):
-- short-lived rows every API replica reads and writes, so a request created
-- on one replica is recovered, answered and polled on any other. A request
-- lives 5 minutes plus a 5-minute grace in which it answers 410; status
-- moves pending -> signed (the outcome is stored) -> fulfilled (delivered
-- once: the outcome, params and challenge are cleared).
CREATE TABLE explore_api.auth_requests (
    request_id text PRIMARY KEY,
    method text NOT NULL,
    params json NOT NULL,
    sender text,
    code smallint NOT NULL CHECK (code BETWEEN 0 AND 99),
    challenge text NOT NULL,
    created_at timestamp without time zone NOT NULL,
    expiration timestamp without time zone NOT NULL,
    requires_validation boolean NOT NULL DEFAULT false,
    status text NOT NULL CHECK (status IN ('pending', 'signed', 'fulfilled')),
    outcome json,
    -- The creating network's key, for the per-network pending cap.
    source text,
    CHECK ((status = 'signed') = (outcome IS NOT NULL))
);

CREATE INDEX auth_requests_expiration_idx ON explore_api.auth_requests (expiration);

-- A signed identity parked for one pickup within an hour; the pickup deletes
-- the row. It holds an ephemeral private key, never logged: like every row it
-- reaches the WAL, the replicas and the backups (DATABASE.md, "Auth API relay
-- state workload").
CREATE TABLE explore_api.auth_identities (
    identity_id text PRIMARY KEY,
    identity json NOT NULL,
    ip_address text NOT NULL,
    source text,
    is_mobile boolean NOT NULL,
    created_at timestamp without time zone NOT NULL,
    expiration timestamp without time zone NOT NULL,
    -- The deep-link client's request UUID (lowercase), derived from the
    -- verifier it presents on pickup; NULL when the create named none.
    auth_request_id text
);

CREATE INDEX auth_identities_expiration_idx ON explore_api.auth_identities (expiration);

-- Why an identity is gone, kept 14 days for the 404 bodies: NULL while it is
-- parked or once it was evicted (swept unread or revoked by an operator).
CREATE TABLE explore_api.auth_identity_tombstones (
    identity_id text PRIMARY KEY,
    created_at timestamp without time zone NOT NULL,
    deletion_reason text CHECK (deletion_reason IN ('consumed', 'expired', 'ip_mismatch'))
);

CREATE INDEX auth_identity_tombstones_created_idx ON explore_api.auth_identity_tombstones (created_at);

CREATE TABLE explore_api.magic_deletion_attempts (
    token_key bytea PRIMARY KEY,
    address_key bytea NOT NULL,
    network_key bytea,
    requested_at timestamptz NOT NULL DEFAULT now(),
    status text NOT NULL DEFAULT 'requested'
        CHECK (status IN ('requested', 'processed', 'unprocessed', 'rate_limited', 'upstream_error'))
);

CREATE INDEX magic_deletion_attempts_requested_idx ON explore_api.magic_deletion_attempts (requested_at);

-- POST /onboarding/checkpoint: one row per (user, checkpoint 1-7), the
-- auth server's onboarding_checkpoints. reached_at is the first `reached`
-- and never moves; completed_at is set once (by `completed`, or by reaching
-- the next checkpoint). id is the insertion order: the order of
-- GET /onboarding/pending-nudges and the tie-break of a wallet's resolution.
-- Rows older than the retention (DATABASE.md, "Onboarding checkpoints
-- workload") are deleted by the writers in small batches.
CREATE TABLE explore_api.onboarding_checkpoints (
    id bigint PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    user_id text NOT NULL CHECK (char_length(user_id) BETWEEN 1 AND 255),
    checkpoint smallint NOT NULL CHECK (checkpoint BETWEEN 1 AND 7),
    email text CHECK (char_length(email) <= 255),
    wallet text CHECK (char_length(wallet) <= 255 AND wallet = lower(wallet)),
    reached_at timestamp with time zone NOT NULL,
    completed_at timestamp with time zone,
    metadata jsonb CHECK (jsonb_typeof(metadata) = 'object'),
    CONSTRAINT onboarding_checkpoints_user_checkpoint_key UNIQUE (user_id, checkpoint)
);

-- A wallet-identified checkpoint resolves to the lowest checkpoint (then the
-- oldest row) bound to that wallet with an email.
CREATE INDEX onboarding_checkpoints_wallet_idx
    ON explore_api.onboarding_checkpoints (wallet, checkpoint, id)
    WHERE wallet IS NOT NULL AND email IS NOT NULL;

-- The nudge candidates: open rows with an email, by age.
CREATE INDEX onboarding_checkpoints_pending_idx
    ON explore_api.onboarding_checkpoints (reached_at)
    WHERE email IS NOT NULL AND completed_at IS NULL;

-- Sites' phone pairing sessions (/auth/pair, /internal/pair), shared by every
-- Sites replica: written and read only by apps/sites
-- (src/data/lib/auth/pair-store.server.ts). A row lives five minutes; the
-- desktop's poll that releases a completed row deletes it, and every create
-- first deletes the expired rows. The poll token is kept as its sha256.
CREATE TABLE explore_api.pair_sessions (
    id text PRIMARY KEY CHECK (id ~ '^[A-Za-z0-9_-]{20,64}$'),
    poll_token_sha256 bytea NOT NULL CHECK (length(poll_token_sha256) = 32),
    ephemeral text NOT NULL,
    expiration text NOT NULL,
    message text NOT NULL,
    created_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL,
    completed_signer text,
    completed_signature text,
    CHECK ((completed_signer IS NULL) = (completed_signature IS NULL))
);
