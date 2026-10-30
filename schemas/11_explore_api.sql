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
