-- Catalyst v2: presence domain: the user-count history archive (one snapshot
-- per collector pass: archipelago peers, islands and hot scenes, per-scene
-- occupancy, per-world membership, a per-day rollup). Distinct from live
-- presence (comms' voice presence, the worlds presence map), which follows
-- the transport and is never persisted.
--
-- Derived from catalyrst/crates/catalyrst-presence/migrations/0001_presence.sql
-- with every timestamp `timestamp without time zone` UTC, bigint identities,
-- the address arrays kept as jsonb (read as a unit, never filtered by member)
-- with their count checked, and the five indexes no statement read dropped.
-- The collector writes a pass in one transaction behind DATA_WRITES_ENABLED.

CREATE SCHEMA presence;

CREATE TABLE presence.snapshots (
    id bigint PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    taken_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    peers_count integer NOT NULL DEFAULT 0,
    islands_count integer NOT NULL DEFAULT 0,
    hot_scenes_count integer NOT NULL DEFAULT 0,
    scenes_polled integer NOT NULL DEFAULT 0,
    scene_users_total integer NOT NULL DEFAULT 0,
    worlds_polled integer NOT NULL DEFAULT 0,
    active_worlds integer NOT NULL DEFAULT 0,
    world_users_total integer NOT NULL DEFAULT 0,
    worlds_live_total integer
);

-- current: ORDER BY taken_at DESC LIMIT 1 (and the latest-snapshot sub-select of the two current lists).
CREATE INDEX snapshots_taken_at_idx ON presence.snapshots (taken_at DESC);

CREATE TABLE presence.peer_snapshots (
    snapshot_id bigint NOT NULL REFERENCES presence.snapshots(id) ON DELETE CASCADE,
    address varchar(42) NOT NULL,
    parcel_x integer,
    parcel_y integer,
    position_x double precision,
    position_y double precision,
    position_z double precision,
    last_ping bigint,
    PRIMARY KEY (snapshot_id, address)
);

CREATE TABLE presence.island_snapshots (
    snapshot_id bigint NOT NULL REFERENCES presence.snapshots(id) ON DELETE CASCADE,
    island_id text NOT NULL,
    peer_count integer,
    max_peers integer,
    center_x double precision,
    center_y double precision,
    center_z double precision,
    radius double precision,
    PRIMARY KEY (snapshot_id, island_id)
);

CREATE TABLE presence.hot_scene_snapshots (
    snapshot_id bigint NOT NULL REFERENCES presence.snapshots(id) ON DELETE CASCADE,
    scene_id text NOT NULL,
    name text,
    base_x integer,
    base_y integer,
    users_count integer,
    parcel_count integer,
    creator text,
    description text,
    PRIMARY KEY (snapshot_id, scene_id)
);

-- Per-scene occupancy: `pointer` is the base parcel "x,y", `realm` the
-- genesis realm; `addresses` the lowercase wallets in the scene.
CREATE TABLE presence.scene_occupancy (
    snapshot_id bigint NOT NULL REFERENCES presence.snapshots(id) ON DELETE CASCADE,
    taken_at timestamp without time zone NOT NULL,
    pointer text NOT NULL,
    scene_name text,
    realm text NOT NULL DEFAULT 'main',
    addresses jsonb NOT NULL,
    count integer NOT NULL,
    PRIMARY KEY (snapshot_id, pointer, realm),
    CONSTRAINT scene_occupancy_count_matches CHECK (count = jsonb_array_length(addresses))
);

-- scene_history: WHERE pointer = $1 ORDER BY taken_at DESC LIMIT $2.
CREATE INDEX scene_occupancy_pointer_idx ON presence.scene_occupancy (pointer, taken_at DESC);

CREATE TABLE presence.world_membership (
    snapshot_id bigint NOT NULL REFERENCES presence.snapshots(id) ON DELETE CASCADE,
    taken_at timestamp without time zone NOT NULL,
    world_name text NOT NULL,
    addresses jsonb NOT NULL,
    count integer NOT NULL,
    live_users integer,
    PRIMARY KEY (snapshot_id, world_name),
    CONSTRAINT world_membership_count_matches CHECK (count = jsonb_array_length(addresses))
);

-- world_history: WHERE world_name = $1 ORDER BY taken_at DESC LIMIT $2.
CREATE INDEX world_membership_world_idx ON presence.world_membership (world_name, taken_at DESC);

-- The per-UTC-day rollup the collector upserts after each pass; derived,
-- rebuilt by re-aggregating the day's snapshots.
CREATE TABLE presence.daily_stats (
    date date PRIMARY KEY,
    snapshots_taken integer NOT NULL DEFAULT 0,
    peak_peers integer NOT NULL DEFAULT 0,
    avg_peers double precision NOT NULL DEFAULT 0,
    peak_hot_scene_users integer NOT NULL DEFAULT 0,
    peak_scene_users integer NOT NULL DEFAULT 0,
    peak_world_users integer NOT NULL DEFAULT 0
);
