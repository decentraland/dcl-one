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

-- The api.decentraland.org /v2/tiles grid (about 92k tiles, 38 MB of JSON),
-- gzip-compressed as one row so a restart renders map.png before the first
-- pass; last_updated_at is the upstream /v2/tiles/info stamp the grid carries.
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
-- the row. It holds an ephemeral private key: never copied, logged or kept
-- past its pickup or expiry.
CREATE TABLE explore_api.auth_identities (
    identity_id text PRIMARY KEY,
    identity json NOT NULL,
    ip_address text NOT NULL,
    source text,
    is_mobile boolean NOT NULL,
    created_at timestamp without time zone NOT NULL,
    expiration timestamp without time zone NOT NULL
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
