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
