-- Catalyst v2: worlds domain: the world catalogue mirror, the world
-- scenes/files mirror, the worlds read routes (/world/{name}/about,
-- /world/{name}/scenes, /index, /worlds/status), and (stage 5) the permission
-- core, world settings, world/player/env storage and the access log behind
-- the local publish, permission, settings, comms and storage routes.
--
-- Derived from catalyrst/crates/catalyrst-worlds/migrations 0001 (worlds,
-- world_scenes, world_permissions, blocked), 0002 (world_access_log), 0003
-- (world_permission_parcels), 0004 (lower(name) indexes), 0007..0009
-- (world_storage, player_storage, env_variables and their place-aware size
-- indexes, lowercase world names), 0010 (settings_version), 0011
-- (world_scenes.updated_at), 0012 (realm_name_override), 0013
-- (preview_wearable_urns), collapsed to the current shape.
--
-- Workload (measured 2026-09-24 against worlds-content-server.decentraland.org):
-- the upstream catalogue lists 1,667 worlds and 2,517 scenes; GET /index is one
-- 4.3 MB document per catalogue pass. A scene entity is a few KB of JSON and
-- references tens to hundreds of hash-addressed files (MBs) that go to the
-- shared blob store, not here. Reads are one world at a time by name
-- (/about, /scenes), plus the index scan and two counts for /status. Writes
-- are one bounded transaction per mirrored world and one per catalogue pass.
--
-- Authority. Mirrored rows are distinguishable from local publishes by
-- `mirror_source` (the upstream URL) being NOT NULL; a local publish writes
-- NULL. The mirror never touches a world that holds a local scene, and only
-- retires scenes it wrote itself; it never writes owner, access, permissions
-- or storage rows, so a mirrored world grants nobody deploy, stream or host
-- authority here. The zero-address `deployer` on mirrored scenes is kept for
-- the wire (the old /scenes shape and the old marker).
--
-- Conventions: world names are stored lowercase (CHECKed; every old local
-- write path inserted lower($1) and the catalogue lists lowercase names) and
-- read with name = $key, the lowercase key every reader binds
-- (src/worlds/identity.rs::WorldName; a raw input is bound as lower($1)), so
-- the PKs serve every lookup and no lower() expression index exists; wallet
-- addresses are lowercase 0x + 40
-- hex (social::Address); parcels are canonical "x,y"; every timestamp column
-- is `timestamp without time zone` holding UTC (the pool pins TimeZone=UTC);
-- the current time is a bind parameter, never an index predicate. No FK
-- crosses a schema: the scene admins and bans a world scene is moderated by
-- are `social.scene_admins`, keyed by the scene entity id, and
-- `social.scene_bans`, keyed by `<world>:<base parcel>` so a ban survives a
-- redeploy (src/worlds/policy.rs reads them; comms owns their writes).
--
-- The federation listing mirror of 0005/0006 (peers' public /worlds listings,
-- a different protocol from the upstream /index catalogue) is the
-- `worlds-federation` section at the end: worlds.federation_worlds and
-- worlds.federation_peer_status, renamed because worlds.remote_worlds here is
-- the catalogue. Not carried over: the old `worlds.size` column (unused after
-- upstream 0016; a world's size is sum(world_scenes.size)).
--
-- Domain agents append ONLY inside the `-- ==== section: ... ====` blocks at
-- the end (scene host, storage routes, worlds federation), after re-reading
-- this file.

CREATE SCHEMA worlds;

-- ── worlds and scenes ──────────────────────────────────────────────────────

-- One row per world this node knows: mirrored (mirror_source) or published
-- here. `owner` is the wallet the last local publish resolved as the NAME
-- owner (personal world: the address itself; else the name authority), the
-- second step of the owner lookup (src/worlds/policy.rs, "owner"); it is
-- never written by the mirror and never by a permission edit alone (those
-- create the row with the signer as owner only after the signer proved
-- ownership). The access setting is three typed columns (the old JSONB
-- document): the kind, the bcrypt hash for shared-secret, the NFT urn for
-- nft-ownership; an allow-list's wallets are world_permissions rows with
-- permission = 'access' and its communities world_access_communities rows.
-- `settings_version` is bumped by every settings-affecting write (deploy
-- refresh, PUT /settings, access edits, mirror ingest) under the row lock so
-- a settings consumer can refuse an out-of-order update.
CREATE TABLE worlds.worlds (
    name text NOT NULL,
    owner text,
    access_type text NOT NULL DEFAULT 'unrestricted',
    access_secret_hash text,
    access_nft text,
    blocked_since timestamp without time zone,
    spawn_coordinates text,
    title text,
    description text,
    content_rating text,
    skybox_time integer,
    categories text[],
    single_player boolean NOT NULL DEFAULT false,
    show_in_places boolean NOT NULL DEFAULT true,
    thumbnail_hash text,
    preview_wearable_urns text[],
    settings_version bigint NOT NULL DEFAULT 0,
    realm_name_override text,
    mirror_source text,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT worlds_pkey PRIMARY KEY (name),
    CONSTRAINT worlds_name_lowercase CHECK (name = lower(name) AND btrim(name) <> ''),
    CONSTRAINT worlds_owner_lowercase_address CHECK (owner IS NULL OR owner ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT worlds_access_type_check
        CHECK (access_type IN ('unrestricted', 'shared-secret', 'nft-ownership', 'allow-list')),
    CONSTRAINT worlds_access_secret_check
        CHECK ((access_type = 'shared-secret') = (access_secret_hash IS NOT NULL)),
    CONSTRAINT worlds_access_nft_check
        CHECK ((access_type = 'nft-ownership') = (access_nft IS NOT NULL)),
    CONSTRAINT worlds_content_rating_check
        CHECK (content_rating IS NULL OR content_rating IN ('RP', 'E', 'T', 'A', 'R')),
    CONSTRAINT worlds_categories_check
        CHECK (categories IS NULL OR cardinality(categories) <= 20),
    CONSTRAINT worlds_preview_wearables_check
        CHECK (preview_wearable_urns IS NULL OR cardinality(preview_wearable_urns) <= 8),
    CONSTRAINT worlds_settings_version_nonneg CHECK (settings_version >= 0)
);

-- /world/{name}/about, /world/{name}/scenes, the mirror, the permission core
-- and the room resolver look worlds up by the lowercase key and read one
-- row: the PK is the one-world-per-name invariant and the one index (the
-- stage-4 readers' lower(name) expression index went with their predicate,
-- 2026-09-25).

CREATE TABLE worlds.world_scenes (
    world_name text NOT NULL,
    entity_id text NOT NULL,
    deployment_auth_chain json NOT NULL,
    entity jsonb NOT NULL,
    deployer text NOT NULL,
    parcels text[] NOT NULL,
    size bigint NOT NULL,
    mirror_source text,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT world_scenes_pkey PRIMARY KEY (world_name, entity_id),
    CONSTRAINT world_scenes_size_nonneg CHECK (size >= 0),
    CONSTRAINT fk_world_name FOREIGN KEY (world_name)
        REFERENCES worlds.worlds (name) ON DELETE CASCADE
);

-- The about/scenes lookup and its newest-first order; also the room
-- resolver's "active scene of a world" (ORDER BY created_at DESC LIMIT 1) and
-- the deploy path's overlap probe (`parcels && $2` under the same prefix).
-- Keyed on the stored lowercase column (every reader binds the key), so the
-- PK prefix and this index are the two shapes one predicate uses.
CREATE INDEX world_scenes_world_name_created_idx
    ON worlds.world_scenes (world_name, created_at DESC);

-- A scene's storage collaborators: the lowercased string entries of its
-- `metadata.logsPermissions` array, from the raw JSON text (an entry spelled
-- with escapes never matches; no stored json/jsonb value makes it fail, so it can index json
-- columns that may hold `\u0000`). The storage routes' grant
-- (`storage::has_logs_access`) and `GET /collaborator` compare against it.
CREATE FUNCTION worlds.logs_permissions(text) RETURNS text[]
    LANGUAGE sql IMMUTABLE PARALLEL SAFE AS $$
    SELECT ARRAY(
        SELECT lower(btrim(e::text, '"'))
        FROM json_array_elements(CASE WHEN json_typeof($1::json) = 'array' THEN $1::json ELSE '[]'::json END) e
        WHERE json_typeof(e) = 'string')
$$;

-- `GET /collaborator`: the local world scenes listing the signer.
CREATE INDEX world_scenes_logs_permissions_idx ON worlds.world_scenes
    USING gin (worlds.logs_permissions((entity #> '{metadata,logsPermissions}')::text))
    WHERE mirror_source IS NULL;

-- `GET /collaborator`: the live Genesis scenes listing the signer (partial on
-- the predicate the query repeats; overwritten rows leave the index).
CREATE INDEX deployments_scene_logs_permissions_idx ON content.deployments
    USING gin (worlds.logs_permissions((entity_metadata -> 'logsPermissions')::text))
    WHERE entity_type = 'scene' AND deleter_deployment IS NULL;

-- ── the permission core ────────────────────────────────────────────────────

-- An allow-list grant on a world: `deployment` (may publish), `streaming`
-- (may stream), `access` (may enter an allow-list world). Owners are not
-- rows: ownership is resolved from the name authority (policy.rs). A grant
-- is WORLD-WIDE when it has no world_permission_parcels row and
-- PARCEL-SCOPED when it has one or more; granting world-wide clears the
-- parcel rows (the old upstream rule). Keyed on the world's lowercase name
-- and the lowercase address, the spellings every old statement compared
-- with lower(); the FK follows the world's lifecycle.
CREATE TABLE worlds.world_permissions (
    world_name text NOT NULL,
    permission text NOT NULL,
    address text NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT world_permissions_pkey PRIMARY KEY (world_name, permission, address),
    CONSTRAINT world_permissions_permission_check
        CHECK (permission IN ('deployment', 'streaming', 'access')),
    CONSTRAINT world_permissions_address_lowercase CHECK (address ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT world_permissions_world_lowercase CHECK (world_name = lower(world_name)),
    CONSTRAINT fk_world_permissions_world FOREIGN KEY (world_name)
        REFERENCES worlds.worlds (name) ON DELETE CASCADE
);

-- The parcels a grant is scoped to, canonical "x,y". Cascades with the
-- grant; the PK is the membership probe of the coverage predicate
-- (policy.rs::has_permission_covering) and of the parcel listing.
CREATE TABLE worlds.world_permission_parcels (
    world_name text NOT NULL,
    permission text NOT NULL,
    address text NOT NULL,
    parcel text NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT world_permission_parcels_pkey PRIMARY KEY (world_name, permission, address, parcel),
    CONSTRAINT world_permission_parcels_canonical CHECK (parcel ~ '^-?(0|[1-9][0-9]*),-?(0|[1-9][0-9]*)$'),
    CONSTRAINT fk_world_permission_parcels_grant FOREIGN KEY (world_name, permission, address)
        REFERENCES worlds.world_permissions (world_name, permission, address) ON DELETE CASCADE
);

-- The communities an allow-list world admits (the old access.communities).
-- No FK to social.communities (different lifecycle); reconciled on read.
CREATE TABLE worlds.world_access_communities (
    world_name text NOT NULL,
    community_id uuid NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT world_access_communities_pkey PRIMARY KEY (world_name, community_id),
    CONSTRAINT world_access_communities_world_lowercase CHECK (world_name = lower(world_name)),
    CONSTRAINT fk_world_access_communities_world FOREIGN KEY (world_name)
        REFERENCES worlds.worlds (name) ON DELETE CASCADE
);
-- The worlds listing a community, read on every community leave, kick, ban,
-- suspension and delete (core::worlds_listing_community).
CREATE INDEX world_access_communities_community_idx
    ON worlds.world_access_communities (community_id);

-- Wallets an operator blocked on this node (the old `blocked` table: the
-- over-storage / platform block the admin routes manage). A blocked wallet
-- is refused every world room join (policy.rs, "blocked"; the old comms
-- mint's wallet_blocked probe); the old deploy path did not consult it and
-- neither does can_deploy.
CREATE TABLE worlds.blocked_wallets (
    wallet text NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT blocked_wallets_pkey PRIMARY KEY (wallet),
    CONSTRAINT blocked_wallets_lowercase_address CHECK (wallet ~ '^0x[0-9a-f]{40}$')
);

-- ── access log ─────────────────────────────────────────────────────────────

-- Who entered/left which world room, from the LiveKit webhook's
-- participant_joined/left events (the old world_access_log). Append-only,
-- batched inserts; read by the bearer-gated admin listing.
CREATE TABLE worlds.world_access_log (
    id bigint GENERATED ALWAYS AS IDENTITY,
    world_name text NOT NULL,
    address text NOT NULL,
    action text NOT NULL,
    room text NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT world_access_log_pkey PRIMARY KEY (id),
    CONSTRAINT world_access_log_action_check CHECK (action IN ('join', 'leave')),
    CONSTRAINT world_access_log_world_lowercase CHECK (world_name = lower(world_name)),
    CONSTRAINT world_access_log_address_lowercase CHECK (address = lower(address))
);

-- The admin listing `WHERE ($1 IS NULL OR world_name = $1) AND ($2 IS NULL OR
-- address = $2) ORDER BY created_at DESC, id DESC LIMIT/OFFSET`: one index per
-- filter arm and one for the unfiltered walk (the old three, on the stored
-- lowercase columns instead of lower() expressions).
CREATE INDEX world_access_log_world_idx ON worlds.world_access_log (world_name, created_at DESC, id DESC);
CREATE INDEX world_access_log_address_idx ON worlds.world_access_log (address, created_at DESC, id DESC);
CREATE INDEX world_access_log_created_idx ON worlds.world_access_log (created_at DESC, id DESC);

-- ── world, player and env storage ──────────────────────────────────────────

-- Three key/value namespaces a running scene reads and writes through the
-- scoped storage API (the old catalyrst-world-storage tables), scoped by
-- `(realm, scene_namespace)` (src/worlds/identity.rs::SceneNamespace):
--   realm           the world's lowercase name for a world scene, 'main' for
--                   any shared realm (Genesis City; the old GENESIS_CITY_REALM)
--   scene_namespace for a world scene its active explore.world_scene_place
--                   (below; the old world-storage keyed rows on the Places
--                   id, one place per world scene); for a Genesis scene the
--                   canonical base parcel of its active deployment (the LAND
--                   place the old Places id stood for; a moved base is a new
--                   place, as it was). 'world' marks rows written before
--                   world scenes were scoped apart that the backfill could
--                   not attribute (a world with no scene then): parked, never
--                   resolved; excluded from quota once this world has an
--                   active identity
-- and NOT the deployment's content hash: a redeploy of the scene keeps its
-- rows, and an undeploy does not touch them (no FK to world_scenes; a
-- world's rows survive its world row too). Only the explicit clear routes
-- (DELETE /values, /players, /env, owner/deployer authorized) remove them.
-- `value_size` is the serialized value's byte length, kept beside the value
-- so the quota aggregate never reads the (TOASTed) jsonb. env values are
-- AES-256-GCM ciphertext; plaintext never reaches the database.
CREATE TABLE worlds.world_storage (
    realm text NOT NULL,
    scene_namespace text NOT NULL,
    key text NOT NULL,
    value jsonb NOT NULL,
    value_size integer NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT world_storage_pkey PRIMARY KEY (realm, scene_namespace, key),
    CONSTRAINT world_storage_realm_lowercase CHECK (realm = lower(realm) AND realm <> ''),
    CONSTRAINT world_storage_key_len CHECK (octet_length(key) BETWEEN 1 AND 1020),
    CONSTRAINT world_storage_size_nonneg CHECK (value_size >= 0)
);

CREATE TABLE worlds.player_storage (
    realm text NOT NULL,
    scene_namespace text NOT NULL,
    player_address text NOT NULL,
    key text NOT NULL,
    value jsonb NOT NULL,
    value_size integer NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT player_storage_pkey PRIMARY KEY (realm, scene_namespace, player_address, key),
    CONSTRAINT player_storage_realm_lowercase CHECK (realm = lower(realm) AND realm <> ''),
    CONSTRAINT player_storage_player_lowercase_address CHECK (player_address ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT player_storage_key_len CHECK (octet_length(key) BETWEEN 1 AND 1020),
    CONSTRAINT player_storage_size_nonneg CHECK (value_size >= 0)
);

CREATE TABLE worlds.env_variables (
    realm text NOT NULL,
    scene_namespace text NOT NULL,
    key text NOT NULL,
    value_enc bytea NOT NULL,
    value_size integer NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT env_variables_pkey PRIMARY KEY (realm, scene_namespace, key),
    CONSTRAINT env_variables_realm_lowercase CHECK (realm = lower(realm) AND realm <> ''),
    CONSTRAINT env_variables_key_len CHECK (octet_length(key) BETWEEN 1 AND 1020),
    CONSTRAINT env_variables_size_nonneg CHECK (value_size >= 0)
);

-- Legacy storage-identity bootstrap retained for databases upgrading through
-- migration0010. No runtime writer or resolver uses this table. The shared
-- Places authority in21_world_scene_places.sql copies its UUIDs once, including
-- retired scopes, and associates active identities with exact deployments.
-- Do not remove/reseed this compatibility source or change migration0010:
-- already-applied migration digests and existing storage namespaces must survive.
CREATE TABLE worlds.world_scene_identity (
    world_name text NOT NULL,
    identity uuid NOT NULL,
    parcels text[] NOT NULL,
    active boolean NOT NULL DEFAULT true,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT world_scene_identity_pkey PRIMARY KEY (identity),
    CONSTRAINT world_scene_identity_world_lowercase CHECK (world_name = lower(world_name) AND world_name <> ''),
    CONSTRAINT world_scene_identity_parcels_nonempty CHECK (cardinality(parcels) > 0)
);

-- Legacy index retained for layout compatibility with earlier 0010 binaries.
-- Current overlap/resolution/quota queries use explore.world_scene_place;
-- no runtime workload is served by this frozen upgrade-source index.
CREATE INDEX world_scene_identity_world_active_idx
    ON worlds.world_scene_identity (world_name) WHERE active;

-- A database that held scenes before this table: one identity per scene,
-- derived from (world, entity id) so schemas/migrations/0010 moves the
-- world's 'world' rows to the same id before this runs.
INSERT INTO worlds.world_scene_identity (world_name, identity, parcels)
SELECT world_name, md5(world_name || '/' || entity_id)::uuid, parcels
FROM worlds.world_scenes WHERE cardinality(parcels) > 0;

-- The per-write quota aggregate, taken under pg_advisory_xact_lock of the
-- namespace scope: `SELECT COALESCE(MAX(value_size) FILTER (WHERE
-- scene_namespace = $2 AND key = $3), 0), COALESCE(SUM(value_size), 0) FROM
-- <table> WHERE realm = $1 [AND scene_namespace = $2]` (a world's quota spans
-- non-retired scenes plus unknown legacy scopes, excluding parked 'world'
-- rows once that world has an active identity; a shared realm's is per
-- scene), and the /usage routes. A NOT EXISTS anti-join excludes recorded
-- inactive explore.world_scene_place rows without deleting their retained bytes.
-- value_size in the leaf keeps them index-only; the PK cannot (it lacks the
-- size). The player index leads with the player so a player's world-wide
-- total is one range. Inherited from upstream 1782950400000 and unmeasured
-- on v2.
CREATE INDEX world_storage_size_idx
    ON worlds.world_storage (realm, scene_namespace) INCLUDE (key, value_size);
CREATE INDEX player_storage_size_idx
    ON worlds.player_storage (realm, player_address, scene_namespace) INCLUDE (key, value_size);
CREATE INDEX env_variables_size_idx
    ON worlds.env_variables (realm, scene_namespace) INCLUDE (key, value_size);

-- ── mirrors ────────────────────────────────────────────────────────────────

-- The catalogue: one row per world the upstream /index listed, with the scene
-- ids it advertised, so the scenes mirror can tell an unchanged world from the
-- catalogue alone. `unlisted_at` is set by a complete catalogue pass that no
-- longer lists the world; the scenes pass then retires its mirrored scenes.
CREATE TABLE worlds.remote_worlds (
    world_name text NOT NULL,
    source text NOT NULL,
    scene_ids text[] NOT NULL,
    newest_timestamp_ms bigint,
    first_seen_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    listed_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    unlisted_at timestamp without time zone,
    mirrored_at timestamp without time zone,
    last_error text,
    CONSTRAINT remote_worlds_pkey PRIMARY KEY (world_name),
    CONSTRAINT remote_worlds_name_lowercase CHECK (world_name = lower(world_name))
);

-- One row per mirror ('catalogue', 'scenes'). `cursor` is the last world name
-- the scenes pass completed (NULL between passes), persisted after every
-- batch so a restart resumes there; `pass_completed_at` with a NULL cursor is
-- the complete-pass proof, `stats` its counts.
CREATE TABLE worlds.mirror_passes (
    mirror text NOT NULL,
    source text NOT NULL,
    pass_number bigint NOT NULL DEFAULT 0,
    pass_started_at timestamp without time zone,
    pass_completed_at timestamp without time zone,
    last_attempt_at timestamp without time zone,
    cursor text,
    stats jsonb NOT NULL DEFAULT '{}'::jsonb,
    last_error text,
    CONSTRAINT mirror_passes_pkey PRIMARY KEY (mirror),
    CONSTRAINT mirror_passes_known_mirror CHECK (mirror IN ('catalogue', 'scenes'))
);

-- ══ Domain-agent sections: append inside the matching block only ═══════════

-- ==== section: scene-host ====
-- Left to the authoritative scene host agent. The old catalyrst-scene-state
-- persisted nothing: on restart it re-resolved the world's active scene
-- (/world/{name}/about scenesUrn[0], now rooms.rs::resolve), reloaded the
-- files from the blob store and minted a fresh storage delegation; live
-- CRDT/tick state and client entity ranges stay in the runtime; player state
-- is player_storage above. A host lease (fencing epoch per (realm,
-- scene_namespace), for a host running as its own process or twice) is the
-- one relation the restart contract may need.
-- Landed 2026-09-25 (src/worlds/scene_state): no relation. One in-process
-- host; a restart re-resolves through rooms.rs (world_active_scene,
-- genesis_scene, genesis_scene_at), reloads game.js/main.crdt from the blob
-- store and re-mints the delegation (SCENE_HOST_AUTHORITATIVE_KEY in-process,
-- the old deploy-signer envelope, or the external minter); the
-- scene-host-sweep job re-reads the same rows to reload a redeployed scene
-- and re-admit every socket. scene_host_leases stays NOT created until a
-- second host instance exists.
-- ==== end section: scene-host ====

-- ==== section: storage-routes ====
-- Left to the storage routes agent: nothing expected. A storage_usage
-- projection replacing the three size indexes is a measured decision.
-- ==== end section: storage-routes ====

-- ==== section: worlds-federation ====
-- The worlds federation read mirror (src/worlds/fed): what admitted peers say
-- on their own public /worlds listings, held under the peer that said it and
-- served only on /federation/worlds/* qualified by peerId. Migrations 0005
-- (remote_worlds, remote_peer_status) and 0006 (de-admission tombstone),
-- renamed: v2's worlds.remote_worlds is the stage-4 catalogue.
--
-- Authority: none. A peer's row is a content claim, never an ownership or
-- permission fact: there is no owner, access, permissions, blocked_since,
-- deployer or auth-chain column, so the `owner` a peer prints on every entry
-- has nowhere to land (the poller's deserialiser has no field for it either).
-- No FK to worlds.worlds in either direction: an ACL row cannot attach to a
-- mirrored-only name (world_permissions FKs worlds(name), which the mirror
-- never writes), a peer's listing cannot constrain or cascade into the local
-- tables, and the same name held locally and claimed by a peer is two
-- independent rows. The room resolver, the permission core, /index and
-- /about never read this table, so a mirrored world grants nobody host
-- authority and never shadows a local publish.
--
-- Workload: a poll replaces one peer's
-- rows in one transaction (DELETE ... WHERE peer_id = $1 AND hidden_since IS
-- NULL, then 500-row upserts); the public listing pages ORDER BY peer_id,
-- world_name, filtered to the admitted set; the boot sweep deletes the rows
-- of peers no longer admitted. Peers: a handful; rows per peer: the
-- production listing is 1.6k, capped at WORLDS_FED_MAX_WORLDS_PER_PEER (10k).
-- `observed_at` is our clock at the poll (the per-row staleness signal),
-- `last_deployed_at` the peer's text, parsed and never read by a branch.
-- `hidden_since` is the local operator veto: the poller's DELETE spares it
-- and its UPDATE arm never names it, so a peer cannot un-hide itself by
-- re-listing, and the sweep spares vetoed rows so a re-admission does not
-- republish a vetoed world. Every timestamp is UTC without time zone and the
-- current time is a bind parameter.
CREATE TABLE worlds.federation_worlds (
    peer_id text NOT NULL,
    world_name text NOT NULL,
    title text,
    description text,
    content_rating text,
    categories text[],
    thumbnail_hash text,
    deployed_scenes bigint NOT NULL DEFAULT 0,
    last_deployed_at timestamp without time zone,
    observed_at timestamp without time zone NOT NULL,
    hidden_since timestamp without time zone,
    CONSTRAINT federation_worlds_pkey PRIMARY KEY (peer_id, world_name),
    CONSTRAINT federation_worlds_peer_id_lowercase CHECK (peer_id = lower(peer_id) AND peer_id <> ''),
    CONSTRAINT federation_worlds_name_lowercase CHECK (world_name = lower(world_name) AND world_name <> ''),
    CONSTRAINT federation_worlds_deployed_scenes_nonneg CHECK (deployed_scenes >= 0)
);

-- One row per peer this node ever attempted, admitted or since removed. The
-- poller writes last_attempt_at on every poll and last_success_at only on a
-- complete one, so an unreachable peer reads as STALE (attempt > success,
-- last_error set), never as EMPTY; a peer that never answered has no
-- last_success_at at all (hasEverSucceeded: false on the wire). The boot
-- sweep records a de-admission here, bounded, when it deletes the peer's
-- unbounded per-world rows: deadmitted_at (cleared on re-admission) and the
-- cumulative deadmitted_worlds_deleted. Read whole (ORDER BY peer_id) by the
-- two public routes; the old partial index on deadmitted_at answered no
-- statement and is not created.
CREATE TABLE worlds.federation_peer_status (
    peer_id text NOT NULL,
    last_attempt_at timestamp without time zone,
    last_success_at timestamp without time zone,
    last_error text,
    worlds_observed bigint NOT NULL DEFAULT 0,
    entries_skipped bigint NOT NULL DEFAULT 0,
    truncated boolean NOT NULL DEFAULT false,
    deadmitted_at timestamp without time zone,
    deadmitted_worlds_deleted bigint NOT NULL DEFAULT 0,
    CONSTRAINT federation_peer_status_pkey PRIMARY KEY (peer_id),
    CONSTRAINT federation_peer_status_id_lowercase CHECK (peer_id = lower(peer_id) AND peer_id <> ''),
    CONSTRAINT federation_peer_status_counts_nonneg
        CHECK (worlds_observed >= 0 AND entries_skipped >= 0 AND deadmitted_worlds_deleted >= 0)
);
-- ==== end section: worlds-federation ====

-- ==== section: forwarding ====
-- The worlds lane of the upstream forwarding outbox (content's
-- src/content/forward.rs; content.forward_outbox is the content lane): one
-- row per local world publish this node sends on, unchanged, to
-- WORLDS_FORWARD_UPSTREAM_URL. `store::deploy_scene` upserts it in the
-- publish transaction, so a committed scene has its row and a refused publish
-- never does; a re-publish of the same entity replaces its row. The leased
-- `worlds-forward` worker claims one due row at a time, sends after the claim
-- committed and records the outcome; `attempts` fences a late finish.
-- `world_name` is the scene's world and the worker's join key. No FK to
-- worlds.world_scenes: a replaced or undeployed scene deletes its row there
-- while this row stays the forwarding record, and the worker reads the scene
-- missing (or turned into a mirror row) as superseded and never sends it,
-- since a worlds upstream applies the last publish it receives and an old
-- scene sent late would roll the world back. Another target's open rows are
-- closed `failed` after 20 minutes (the upstream's 5-minute age window plus
-- the 15 minutes a timestamp may lead). Terminal rows are deleted 30 days
-- after their last update.
CREATE TABLE worlds.forward_outbox (
    id bigint GENERATED ALWAYS AS IDENTITY,
    entity_id text NOT NULL,
    world_name text NOT NULL,
    target text NOT NULL,
    status text NOT NULL DEFAULT 'queued',
    attempts integer NOT NULL DEFAULT 0,
    next_attempt_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    last_error text,
    upstream_status smallint,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT forward_outbox_pkey PRIMARY KEY (id),
    CONSTRAINT forward_outbox_entity_id_key UNIQUE (entity_id),
    CONSTRAINT forward_outbox_world_name_lowercase CHECK (world_name = lower(world_name) AND world_name <> ''),
    CONSTRAINT forward_outbox_status_check
        CHECK (status IN ('queued', 'sending', 'accepted', 'rejected', 'failed', 'superseded')),
    CONSTRAINT forward_outbox_attempts_check CHECK (attempts >= 0),
    CONSTRAINT forward_outbox_upstream_status_check CHECK (upstream_status BETWEEN 100 AND 599)
);

-- The worker's claim, as content.forward_outbox_due_idx.
CREATE INDEX forward_outbox_due_idx ON worlds.forward_outbox USING btree (target, next_attempt_at, id)
    WHERE status IN ('queued', 'sending');
-- ==== end section: forwarding ====

-- ==== section: replaced-scenes ====
-- A world scene a deploy replaced, kept seven days so its scene comms still
-- mint for the players in its room (the old service resolved undeployed
-- scenes for scene comms; worlds::rooms::replaced_world_scene_base). The
-- replacing deploy writes these rows in its own transaction from the rows it
-- deletes and purges its world's expired ones; a world's removal cascades. The resolver
-- derives the base from `parcels` and the declared base as for an active
-- scene, and never treats a row here as an active deployment.
CREATE TABLE worlds.replaced_world_scenes (
    world_name text NOT NULL,
    entity_id text NOT NULL,
    parcels text[] NOT NULL,
    declared_base text,
    replaced_at timestamp without time zone NOT NULL,
    CONSTRAINT replaced_world_scenes_pkey PRIMARY KEY (world_name, entity_id),
    CONSTRAINT replaced_world_scenes_world_fk FOREIGN KEY (world_name)
        REFERENCES worlds.worlds (name) ON DELETE CASCADE
);

-- The deploy's purge of its world's expired rows.
CREATE INDEX replaced_world_scenes_replaced_at_idx
    ON worlds.replaced_world_scenes (world_name, replaced_at);
-- ==== end section: replaced-scenes ====
