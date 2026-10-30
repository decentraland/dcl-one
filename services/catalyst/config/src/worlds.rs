//! The worlds plane's settings: the content mirror, hosting and federation.

use clap::Args;

/// Worlds: the worlds content server mirror and what every world loads.
#[derive(Args, Clone, Debug)]
pub struct WorldsContentConfig {
    // worlds: the worlds content server mirror (scenes and files). The
    // `WORLDS_CONTENT_MIRROR_*` family names the worlds-content-server
    // upstream; `WORLDS_MIRROR_UPSTREAM` above is the places API world list.
    #[arg(long, env = "WORLDS_CONTENT_MIRROR_ENABLED", default_value_t = false, action = clap::ArgAction::Set)]
    pub worlds_content_mirror_enabled: bool,

    /// Explicit upstream worlds content server: https:// to a public host,
    /// never this node (the mirror posture's); empty refuses the mirror. With
    /// READ_THROUGH_ENABLED it is also where a world not held, and its files,
    /// are read through from, mirror pass or not.
    #[arg(long, env = "WORLDS_CONTENT_MIRROR_SOURCE", default_value = "")]
    pub worlds_content_mirror_source: String,

    /// Pause between passes; the v1 stack's timer ran every six hours.
    #[arg(
        long,
        env = "WORLDS_CONTENT_MIRROR_INTERVAL_SECS",
        default_value_t = 21600
    )]
    pub worlds_content_mirror_interval_secs: u64,

    /// Worlds mirrored at once (one batch = the cursor granularity).
    #[arg(long, env = "WORLDS_CONTENT_MIRROR_CONCURRENCY", default_value_t = 4)]
    pub worlds_content_mirror_concurrency: usize,

    /// Files fetched at once per scene.
    #[arg(
        long,
        env = "WORLDS_CONTENT_MIRROR_BLOB_CONCURRENCY",
        default_value_t = 8
    )]
    pub worlds_content_mirror_blob_concurrency: usize,

    /// Bounded run: only the first N catalogue names in name order; 0 = all.
    #[arg(long, env = "WORLDS_CONTENT_MIRROR_LIMIT", default_value_t = 0)]
    pub worlds_content_mirror_limit: usize,

    /// Comma-separated world names to mirror; empty = the whole catalogue.
    #[arg(long, env = "WORLDS_CONTENT_MIRROR_NAMES", default_value = "")]
    pub worlds_content_mirror_names: String,

    /// A world read through from WORLDS_CONTENT_MIRROR_SOURCE whose scene was
    /// signed by someone other than the NAME holder is accepted when the
    /// upstream lists the signer on the world's deployment allow-list and names
    /// the owner this node's NAME authority names. The grant is the upstream's
    /// word (its signed grant is not published). Off: only the holder's scenes.
    #[arg(long, env = "WORLDS_READ_THROUGH_TRUST_UPSTREAM_GRANTS", default_value_t = false, action = clap::ArgAction::Set)]
    pub worlds_read_through_trust_upstream_grants: bool,

    /// Strip `.dcl.eth` from the realm name of worlds published here.
    #[arg(long, env = "WORLDS_REALM_NAME_STRIP_ENS", default_value_t = true, action = clap::ArgAction::Set)]
    pub worlds_realm_name_strip_ens: bool,

    /// Whitespace-separated scene URNs every world loads.
    #[arg(long, env = "GLOBAL_SCENES_URN")]
    pub global_scenes_urn: Option<String>,

    /// The estate minimap a world's /about falls back to; empty is this
    /// node's own `<PUBLIC_URL>/api/v1/estatemap.png`.
    #[arg(long, env = "MAP_ESTATE_VIEW_URL", default_value = "")]
    pub map_estate_view_url: String,

    /// Smart-wearable URNs `/world/{name}/preview-wearables` answers for a
    /// world that selected none (comma, space or newline separated).
    #[arg(long, env = "PREVIEW_WEARABLE_URNS")]
    pub preview_wearable_urns: Option<String>,
}

/// Worlds: the write surface, world and player storage, the scene host and archipelago.
#[derive(Args, Clone, Debug)]
pub struct WorldsConfig {
    // worlds: the stage-5 worlds bundle (the old catalyrst-worlds write
    // surface, catalyrst-world-storage, catalyrst-scene-state, archipelago's
    // ws-connector and the worlds federation mirror). A world publish, a
    // permission or settings edit and a storage value are local authority
    // peer sync does not back up, so every worlds write answers 503 until a
    // backup restore has been demonstrated (DATABASE.md, "Bootstrap, schema versions and
    // local writes"); the reads, the admission mints and the sockets serve.
    #[arg(long, env = "WORLDS_WRITES_ENABLED", default_value_t = false, action = clap::ArgAction::Set)]
    pub worlds_writes_enabled: bool,

    /// A worlds content server base URL every local world publish is sent on
    /// to, unchanged, at `<url>/entities` once it committed here (the content
    /// lane's engine and FORWARD_* knobs). Unset forwards nothing. A public
    /// http(s) origin with no path that is not this node, checked at startup.
    #[arg(long, env = "WORLDS_FORWARD_UPSTREAM_URL")]
    pub worlds_forward_upstream_url: Option<String>,

    /// Bearer of the worlds `/admin/*` routes and the federation refresh/veto
    /// routes (or CATALYRST_WORLDS_ADMIN_TOKEN_FILE); unset refuses them.
    #[arg(long, env = "CATALYRST_WORLDS_ADMIN_TOKEN", hide_env_values = true)]
    pub catalyrst_worlds_admin_token: Option<String>,

    /// Personal test worlds (`<address>.dcl.eth`) this realm hosts at once; 0 disables them.
    #[arg(long, env = "WORLDS_PERSONAL_WORLDS_MAX", default_value_t = 0)]
    pub worlds_personal_worlds_max: u64,

    /// Size cap of a personal world, bytes (1..=300 MiB).
    #[arg(
        long,
        env = "WORLDS_PERSONAL_WORLD_MAX_SIZE_BYTES",
        default_value_t = 52_428_800
    )]
    pub worlds_personal_world_max_size_bytes: u64,

    /// Pause between passes rewriting the stored owner of the DCL-NAME worlds
    /// published here from the name authority (upstream's 12 h); 0 = off.
    #[arg(
        long,
        env = "WORLDS_OWNER_RECONCILE_INTERVAL_SECS",
        default_value_t = 43_200
    )]
    pub worlds_owner_reconcile_interval_secs: u64,

    /// Deploy payload bytes buffered at once across every upload (>= one 350 MiB deploy).
    #[arg(
        long,
        env = "MAX_IN_FLIGHT_UPLOAD_BYTES",
        default_value_t = 4_294_967_296
    )]
    pub max_in_flight_upload_bytes: u64,

    #[arg(long, env = "MAX_CONCURRENT_UPLOADS", default_value_t = 40)]
    pub max_concurrent_uploads: u64,

    #[arg(long, env = "MAX_IN_FLIGHT_UPLOAD_FILES", default_value_t = 40_000)]
    pub max_in_flight_upload_files: u64,

    #[arg(long, env = "MULTIPART_UPLOAD_TIMEOUT_MS", default_value_t = 300_000)]
    pub multipart_upload_timeout_ms: u64,

    #[arg(
        long,
        env = "DEPLOYMENT_PROCESSING_TIMEOUT_MS",
        default_value_t = 300_000
    )]
    pub deployment_processing_timeout_ms: u64,

    /// 64 hex characters (or ENCRYPTION_KEY_FILE): the AES-256-GCM key of the
    /// env values; unset, the `/env` routes answer 503 and the rest serve.
    #[arg(long, env = "ENCRYPTION_KEY", hide_env_values = true)]
    pub encryption_key: Option<String>,

    /// Comma-separated wallets authorized on every storage route.
    #[arg(long, env = "AUTHORIZED_ADDRESSES", default_value = "")]
    pub authorized_addresses: String,

    /// The per-process storage memo, disabled by default on every node.
    /// Enable only with a single storage-serving process; no cross-process invalidation exists.
    #[arg(long, env = "STORAGE_CACHE_ENABLED", action = clap::ArgAction::Set)]
    pub storage_cache_enabled: Option<bool>,

    #[arg(long, env = "STORAGE_CACHE_TTL_SECONDS", default_value_t = 60)]
    pub storage_cache_ttl_seconds: u64,

    #[arg(long, env = "STORAGE_CACHE_MAX", default_value_t = 8_000)]
    pub storage_cache_max: u64,

    #[arg(long, env = "STORAGE_CACHE_MAX_VALUE_BYTES", default_value_t = 32_768)]
    pub storage_cache_max_value_bytes: u64,

    #[arg(
        long,
        env = "ENV_STORAGE_MAX_VALUE_SIZE_BYTES",
        default_value_t = 10_240
    )]
    pub env_storage_max_value_size_bytes: i64,

    #[arg(
        long,
        env = "ENV_STORAGE_MAX_TOTAL_SIZE_BYTES",
        default_value_t = 262_144
    )]
    pub env_storage_max_total_size_bytes: i64,

    #[arg(
        long,
        env = "WORLD_STORAGE_MAX_VALUE_SIZE_BYTES",
        default_value_t = 524_288
    )]
    pub world_storage_max_value_size_bytes: i64,

    #[arg(
        long,
        env = "WORLD_STORAGE_MAX_TOTAL_SIZE_BYTES",
        default_value_t = 10_485_760
    )]
    pub world_storage_max_total_size_bytes: i64,

    #[arg(
        long,
        env = "PLAYER_STORAGE_MAX_VALUE_SIZE_BYTES",
        default_value_t = 102_400
    )]
    pub player_storage_max_value_size_bytes: i64,

    #[arg(
        long,
        env = "PLAYER_STORAGE_MAX_TOTAL_SIZE_BYTES",
        default_value_t = 1_048_576
    )]
    pub player_storage_max_total_size_bytes: i64,

    /// `POST /world-storage/exports` running at once per scene, per process.
    #[arg(long, env = "EXPORT_MAX_CONCURRENT_PER_SCENE", default_value_t = 1)]
    pub export_max_concurrent_per_scene: u64,

    /// Exports running at once in this process (each holds a pool connection).
    #[arg(long, env = "EXPORT_MAX_CONCURRENT_PER_POD", default_value_t = 5)]
    pub export_max_concurrent_per_pod: u64,

    /// Seconds after an export starts before the same scene may start another.
    #[arg(long, env = "EXPORT_SCENE_COOLDOWN_SECONDS", default_value_t = 10)]
    pub export_scene_cooldown_seconds: u64,

    #[arg(long, env = "EXPORT_TIMEOUT_SECONDS", default_value_t = 120)]
    pub export_timeout_seconds: u64,

    /// Cap on an export's raw (uncompressed) bytes.
    #[arg(long, env = "EXPORT_MAX_SIZE_BYTES", default_value_t = 524_288_000)]
    pub export_max_size_bytes: u64,

    // worlds/scene_state: the authoritative SDK7 scene host (the old
    // catalyrst-scene-state; V8 isolates in this process).
    /// Construct and mount the scene host under /scene-state.
    #[arg(long, env = "SCENE_HOST_ENABLED", default_value_t = false, action = clap::ArgAction::Set)]
    pub scene_host_enabled: bool,

    /// A local `game.js` served as `localScene` (development).
    #[arg(long, env = "SCENE_HOST_LOCAL_SCENE_PATH")]
    pub scene_host_local_scene_path: Option<String>,

    /// Secret of POST /scene-state/debugging/reload (or _FILE); unset refuses it.
    #[arg(long, env = "SCENE_HOST_DEBUGGING_SECRET", hide_env_values = true)]
    pub scene_host_debugging_secret: Option<String>,

    /// Bearer of /scene-state/admin/scene/* (or _FILE); falls back to the debugging secret.
    #[arg(long, env = "SCENE_HOST_ADMIN_TOKEN", hide_env_values = true)]
    pub scene_host_admin_token: Option<String>,

    /// Seconds a socket gets to send its Auth frame.
    #[arg(long, env = "SCENE_HOST_AUTH_TIMEOUT_SECS", default_value_t = 5)]
    pub scene_host_auth_timeout_secs: u64,

    /// Run every scene as a plain CRDT relay (no V8).
    #[arg(long, env = "SCENE_HOST_DISABLE_JS_RUNTIME", default_value_t = false, action = clap::ArgAction::Set)]
    pub scene_host_disable_js_runtime: bool,

    #[arg(long, env = "JS_HEAP_LIMIT_MB", default_value_t = 384)]
    pub js_heap_limit_mb: usize,

    #[arg(long, env = "JS_TICK_BUDGET_MS", default_value_t = 250)]
    pub js_tick_budget_ms: u64,

    #[arg(long, env = "JS_SHUTDOWN_JOIN_MS", default_value_t = 2000)]
    pub js_shutdown_join_ms: u64,

    #[arg(long, env = "JS_UPDATE_FAILURE_CAP", default_value_t = 30)]
    pub js_update_failure_cap: usize,

    #[arg(long, env = "CLIENT_OUTBOUND_MAX", default_value_t = 1024)]
    pub client_outbound_max: usize,

    #[arg(long, env = "CLIENT_INBOUND_MAX", default_value_t = 1024)]
    pub client_inbound_max: usize,

    #[arg(long, env = "CRDT_MAX_COMPONENTS", default_value_t = 100_000)]
    pub crdt_max_components: usize,

    #[arg(long, env = "WS_MAX_FRAME_BYTES", default_value_t = 2_097_152)]
    pub ws_max_frame_bytes: usize,

    /// The world-storage origin, the ONLY origin a scene's `~system/SignedFetch`
    /// may reach (this node's `<PUBLIC_URL>/world-storage`); unset disables scene storage.
    #[arg(long, env = "SCENE_HOST_STORAGE_URL")]
    pub scene_host_storage_url: Option<String>,

    /// Admit an `http://` loopback storage origin (development).
    #[arg(long, env = "SCENE_HOST_STORAGE_ALLOW_HTTP", default_value_t = false, action = clap::ArgAction::Set)]
    pub scene_host_storage_allow_http: bool,

    /// The authoritative server's private key (or _FILE): storage
    /// delegations are minted in-process; its address is AUTHORITATIVE_SERVER_ADDRESS.
    #[arg(long, env = "SCENE_HOST_AUTHORITATIVE_KEY", hide_env_values = true)]
    pub scene_host_authoritative_key: Option<String>,

    #[arg(long, env = "SCENE_HOST_DELEGATION_TTL_SECS", default_value_t = 3600)]
    pub scene_host_delegation_ttl_secs: u64,

    /// The external delegation minter when the key stays outside this process.
    #[arg(long, env = "SCENE_HOST_DELEGATION_MINTER_URL")]
    pub scene_host_delegation_minter_url: Option<String>,

    #[arg(
        long,
        env = "SCENE_HOST_DELEGATION_MINTER_TOKEN",
        hide_env_values = true
    )]
    pub scene_host_delegation_minter_token: Option<String>,

    /// A pre-minted base64 v1 delegation envelope (development; never renewed).
    #[arg(long, env = "SCENE_HOST_STORAGE_DELEGATION", hide_env_values = true)]
    pub scene_host_storage_delegation: Option<String>,

    #[arg(
        long,
        env = "SIGNED_FETCH_MAX_RESPONSE_BYTES",
        default_value_t = 2_097_152
    )]
    pub signed_fetch_max_response_bytes: usize,

    #[arg(long, env = "SIGNED_FETCH_MAX_BODY_BYTES", default_value_t = 1_048_576)]
    pub signed_fetch_max_body_bytes: usize,

    #[arg(long, env = "SIGNED_FETCH_MAX_IN_FLIGHT", default_value_t = 8)]
    pub signed_fetch_max_in_flight: usize,

    #[arg(long, env = "SIGNED_FETCH_TIMEOUT_MS", default_value_t = 10_000)]
    pub signed_fetch_timeout_ms: u64,

    /// Hosted scenes at once (one V8 isolate and up to JS_HEAP_LIMIT_MB each).
    #[arg(long, env = "SCENE_HOST_MAX_SCENES", default_value_t = 64)]
    pub scene_host_max_scenes: usize,

    /// Load an unhosted scene with a local active deployment on first contact.
    #[arg(long, env = "SCENE_HOST_AUTOLOAD", default_value_t = true, action = clap::ArgAction::Set)]
    pub scene_host_autoload: bool,

    /// The re-admission and redeploy sweep interval; 0 disables it.
    #[arg(long, env = "SCENE_HOST_SWEEP_INTERVAL_SECS", default_value_t = 30)]
    pub scene_host_sweep_interval_secs: u64,

    // worlds/archipelago: the ws-connector (the old catalyrst-archipelago
    // names); LiveKit comes from the comms settings, the pool is the ONE pool.
    /// Process-wide connector socket cap, including pending handshakes.
    #[arg(long, env = "WS_MAX_SOCKETS", default_value_t = 3000, value_parser = clap::value_parser!(u32).range(1..))]
    pub ws_max_sockets: u32,

    /// Handshake deadline, ms; 0 keeps the default.
    #[arg(long, env = "HANDSHAKE_TIMEOUT", default_value_t = 60_000)]
    pub handshake_timeout: u64,

    /// The wallet denylist document; unset disarms the external denylist.
    #[arg(long, env = "DENY_LIST_URL")]
    pub deny_list_url: Option<String>,

    /// The NATS broker `serve` (comms' one link) and `catalyst pulse` meet on; unset, no island is ever delivered.
    #[arg(long, env = "NATS_URL")]
    pub nats_url: Option<String>,

    #[arg(long, env = "ISLAND_CHANGED_DEDUP_MS", default_value_t = 10_000)]
    pub island_changed_dedup_ms: u64,

    #[arg(long, env = "CLUSTER_TAKEOVER_RETRY_DELAY_MS", default_value_t = 100)]
    pub cluster_takeover_retry_delay_ms: u64,

    #[arg(long, env = "CLUSTER_DRAIN_TIMEOUT_MS", default_value_t = 5_000)]
    pub cluster_drain_timeout_ms: u64,

    /// Island JWT lifetime, capped at 60 s (the takeover quarantine waits it out).
    #[arg(long, env = "CLUSTER_ISLAND_TOKEN_TTL_SECONDS", default_value_t = 60)]
    pub cluster_island_token_ttl_seconds: u64,

    /// Wait out old island JWTs when a same-room owner is replaced; disable
    /// only with LiveKit Cloud's cutoff-based token revocation.
    #[arg(long, env = "CLUSTER_SELF_HOSTED_TOKEN_QUARANTINE", default_value_t = true, action = clap::ArgAction::Set)]
    pub cluster_self_hosted_token_quarantine: bool,

    #[arg(long, env = "CLUSTER_PEER_STATE_MAX", default_value_t = 20_000)]
    pub cluster_peer_state_max: usize,

    #[arg(long, env = "CLUSTER_PEER_STATE_TTL_MS", default_value_t = 3_600_000)]
    pub cluster_peer_state_ttl_ms: u64,

    /// The world comms mints refuse a world holding this many participants.
    #[arg(long, env = "MAX_USERS_PER_WORLD", default_value_t = 100)]
    pub max_users_per_world: i64,

    /// The deployment audience clients sign for /ws/v4; unset leaves the v4 control authority unavailable.
    #[arg(long, env = "ARCHIPELAGO_CONTROL_V4_AUDIENCE")]
    pub archipelago_control_v4_audience: Option<String>,
}

/// The worlds federation: the peer file and its caps.
#[derive(Args, Clone, Debug)]
pub struct WorldsFedConfig {
    // worlds/fed: the worlds federation peer file and its caps.
    /// The federation-peers.toml; unset turns federation off (routes 503, nothing polled).
    #[arg(long, env = "WORLDS_FED_PEERS_FILE")]
    pub worlds_fed_peers_file: Option<std::path::PathBuf>,

    #[arg(long, env = "WORLDS_FED_POLL_INTERVAL_SECS", default_value_t = 300)]
    pub worlds_fed_poll_interval_secs: u64,

    #[arg(long, env = "WORLDS_FED_MAX_RESPONSE_BYTES", default_value_t = 4 * 1024 * 1024)]
    pub worlds_fed_max_response_bytes: u64,

    #[arg(long, env = "WORLDS_FED_MAX_WORLDS_PER_PEER", default_value_t = 10_000)]
    pub worlds_fed_max_worlds_per_peer: u64,

    #[arg(long, env = "WORLDS_FED_ALLOW_INSECURE_LOOPBACK_PEERS", default_value_t = false, action = clap::ArgAction::Set)]
    pub worlds_fed_allow_insecure_loopback_peers: bool,
}

#[cfg(test)]
mod tests {
    use crate::config::Config;

    fn parse(args: &[&str]) -> Result<Config, clap::Error> {
        Config::parse_for_tests(args)
    }

    #[test]
    fn storage_cache_flag_accepts_explicit_boolean_values() {
        for (value, expected) in [("true", true), ("false", false)] {
            let config = parse(&[
                "catalyst",
                "--database-url",
                "postgres://localhost/test",
                "--storage-cache-enabled",
                value,
            ])
            .unwrap();
            assert_eq!(config.worlds.storage_cache_enabled, Some(expected));
        }
        assert!(
            parse(&[
                "catalyst",
                "--database-url",
                "postgres://localhost/test",
                "--storage-cache-enabled",
                "off"
            ])
            .is_err()
        );
    }

    #[test]
    fn worlds_defaults_are_the_old_binaries_values_and_writes_are_off() {
        let cli = parse(&["x", "--database-url", "postgres:///catalyst"]).unwrap();
        let c = cli;
        assert!(
            !c.worlds.worlds_writes_enabled,
            "worlds writes are opt-in; a restore must be demonstrated first"
        );
        assert!(
            c.worlds.worlds_forward_upstream_url.is_none(),
            "nothing is forwarded until an operator names an upstream"
        );
        assert!(c.worlds.catalyrst_worlds_admin_token.is_none());
        assert_eq!(
            (
                c.worlds.worlds_personal_worlds_max,
                c.worlds.worlds_personal_world_max_size_bytes
            ),
            (0, 52_428_800)
        );
        assert_eq!(c.worlds.worlds_owner_reconcile_interval_secs, 43_200);
        assert_eq!(
            (
                c.worlds.max_in_flight_upload_bytes,
                c.worlds.max_concurrent_uploads,
                c.worlds.max_in_flight_upload_files,
                c.worlds.multipart_upload_timeout_ms,
                c.worlds.deployment_processing_timeout_ms
            ),
            (4_294_967_296, 40, 40_000, 300_000, 300_000)
        );
        assert!(c.worlds.encryption_key.is_none());
        assert_eq!(c.worlds.authorized_addresses, "");
        assert_eq!(c.worlds.storage_cache_enabled, None);
        assert_eq!(
            (
                c.worlds.storage_cache_ttl_seconds,
                c.worlds.storage_cache_max,
                c.worlds.storage_cache_max_value_bytes
            ),
            (60, 8_000, 32_768)
        );
        assert_eq!(
            (
                c.worlds.env_storage_max_value_size_bytes,
                c.worlds.env_storage_max_total_size_bytes
            ),
            (10_240, 262_144)
        );
        assert_eq!(
            (
                c.worlds.world_storage_max_value_size_bytes,
                c.worlds.world_storage_max_total_size_bytes
            ),
            (524_288, 10_485_760)
        );
        assert_eq!(
            (
                c.worlds.player_storage_max_value_size_bytes,
                c.worlds.player_storage_max_total_size_bytes
            ),
            (102_400, 1_048_576)
        );
        assert_eq!(
            (
                c.worlds.export_max_concurrent_per_scene,
                c.worlds.export_max_concurrent_per_pod,
                c.worlds.export_scene_cooldown_seconds,
                c.worlds.export_timeout_seconds,
                c.worlds.export_max_size_bytes
            ),
            (1, 5, 10, 120, 524_288_000)
        );
        assert!(
            !c.worlds.scene_host_enabled
                && !c.worlds.scene_host_disable_js_runtime
                && !c.worlds.scene_host_storage_allow_http
                && c.worlds.scene_host_autoload
        );
        assert!(
            c.worlds.scene_host_local_scene_path.is_none()
                && c.worlds.scene_host_debugging_secret.is_none()
                && c.worlds.scene_host_admin_token.is_none()
        );
        assert!(
            c.worlds.scene_host_storage_url.is_none()
                && c.worlds.scene_host_authoritative_key.is_none()
                && c.worlds.scene_host_storage_delegation.is_none()
        );
        assert!(
            c.worlds.scene_host_delegation_minter_url.is_none()
                && c.worlds.scene_host_delegation_minter_token.is_none()
        );
        assert_eq!(
            (
                c.worlds.scene_host_auth_timeout_secs,
                c.worlds.scene_host_delegation_ttl_secs,
                c.worlds.scene_host_max_scenes,
                c.worlds.scene_host_sweep_interval_secs
            ),
            (5, 3600, 64, 30)
        );
        assert_eq!(
            (
                c.worlds.js_heap_limit_mb,
                c.worlds.js_tick_budget_ms,
                c.worlds.js_shutdown_join_ms,
                c.worlds.js_update_failure_cap
            ),
            (384, 250, 2000, 30)
        );
        assert_eq!(
            (
                c.worlds.client_outbound_max,
                c.worlds.client_inbound_max,
                c.worlds.crdt_max_components,
                c.worlds.ws_max_frame_bytes
            ),
            (1024, 1024, 100_000, 2_097_152)
        );
        assert_eq!(
            (
                c.worlds.signed_fetch_max_response_bytes,
                c.worlds.signed_fetch_max_body_bytes,
                c.worlds.signed_fetch_max_in_flight,
                c.worlds.signed_fetch_timeout_ms
            ),
            (2_097_152, 1_048_576, 8, 10_000)
        );
        assert_eq!(
            (c.worlds.handshake_timeout, c.worlds.island_changed_dedup_ms),
            (60_000, 10_000)
        );
        assert!(
            c.worlds.deny_list_url.is_none()
                && c.worlds.nats_url.is_none()
                && c.worlds.archipelago_control_v4_audience.is_none()
        );
        assert!(
            c.worlds_fed.worlds_fed_peers_file.is_none()
                && !c.worlds_fed.worlds_fed_allow_insecure_loopback_peers
        );
        assert_eq!(
            (
                c.worlds_fed.worlds_fed_poll_interval_secs,
                c.worlds_fed.worlds_fed_max_response_bytes,
                c.worlds_fed.worlds_fed_max_worlds_per_peer
            ),
            (300, 4 * 1024 * 1024, 10_000)
        );

        let cli = parse(&[
            "x",
            "--database-url",
            "postgres:///catalyst",
            "--worlds-writes-enabled",
            "true",
            "--scene-host-enabled",
            "true",
            "--worlds-personal-worlds-max",
            "3",
            "--nats-url",
            "nats://127.0.0.1:4222",
        ])
        .unwrap();
        assert!(cli.worlds.worlds_writes_enabled && cli.worlds.scene_host_enabled);
        assert_eq!(cli.worlds.worlds_personal_worlds_max, 3);
        assert_eq!(
            cli.worlds.nats_url.as_deref(),
            Some("nats://127.0.0.1:4222")
        );
    }
}
