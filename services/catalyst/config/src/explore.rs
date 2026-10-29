//! The explore plane's settings: mirrors, the boot surface, imposters and the AB CDN.

use clap::Args;

/// Explore: the places, world-catalogue and events mirrors and the explorer boot surface.
#[derive(Args, Clone, Debug)]
pub struct ExploreConfig {
    // explore: the places, world-catalogue and events mirrors (the old
    // catalyrst-places / catalyrst-events names).
    /// Mirror the place catalogue from `PLACES_UPSTREAM_URL`.
    #[arg(long, env = "PLACES_MIRROR_UPSTREAM", default_value_t = false, action = clap::ArgAction::Set)]
    pub places_mirror_upstream: bool,

    /// Explicit upstream places API (the mirror posture's: Decentraland's);
    /// refused if private, loopback or this node, and empty with the mirror on.
    #[arg(long, env = "PLACES_UPSTREAM_URL", default_value = "")]
    pub places_upstream_url: String,

    #[arg(long, env = "PLACES_MIRROR_INTERVAL_SECS", default_value_t = 3600)]
    pub places_mirror_interval_secs: u64,

    /// Derive the whole place catalogue from this node's active scene
    /// deployments (the old production mode); without `PLACES_MIRROR_UPSTREAM`
    /// only. Every node derives its own local scenes either way.
    #[arg(long, env = "PLACES_DERIVE_FROM_CONTENT", default_value_t = false, action = clap::ArgAction::Set)]
    pub places_derive_from_content: bool,

    /// One derive pass per this many seconds (`places-derive`).
    #[arg(long, env = "PLACES_DERIVE_INTERVAL_SECS", default_value_t = 300)]
    pub places_derive_interval_secs: u64,

    /// Mirror the world catalogue (the places API's `/api/worlds` list) from `WORLDS_UPSTREAM_URL`.
    #[arg(long, env = "WORLDS_MIRROR_UPSTREAM", default_value_t = false, action = clap::ArgAction::Set)]
    pub worlds_mirror_upstream: bool,

    /// The places API the world list mirrors from (the mirror posture's).
    #[arg(long, env = "WORLDS_UPSTREAM_URL", default_value = "")]
    pub worlds_upstream_url: String,

    #[arg(long, env = "WORLDS_MIRROR_INTERVAL_SECS", default_value_t = 3600)]
    pub worlds_mirror_interval_secs: u64,

    /// Mirror the event catalogue from `EVENTS_UPSTREAM_URL`.
    #[arg(long, env = "EVENTS_MIRROR_UPSTREAM", default_value_t = false, action = clap::ArgAction::Set)]
    pub events_mirror_upstream: bool,

    /// The events API the catalogue mirrors from (the mirror posture's).
    #[arg(long, env = "EVENTS_UPSTREAM_URL", default_value = "")]
    pub events_upstream_url: String,

    #[arg(long, env = "EVENTS_MIRROR_INTERVAL_SECS", default_value_t = 3600)]
    pub events_mirror_interval_secs: u64,

    /// The events site `/events/sitemap*.xml` points at.
    #[arg(
        long,
        env = "EVENTS_BASE_URL",
        default_value = "https://events.decentraland.org"
    )]
    pub events_base_url: String,

    /// Bearer for `GET /api/events/moderation` and the admin-only event
    /// filters; unset refuses them.
    #[arg(long, env = "CATALYRST_EVENTS_ADMIN_TOKEN", hide_env_values = true)]
    pub catalyrst_events_admin_token: Option<String>,

    /// The places admin bearer (the reports queue, POIs, moderation,
    /// featured, rankings; also PLACES_ADMIN_AUTH_TOKEN_FILE); unset refuses it.
    #[arg(long, env = "PLACES_ADMIN_AUTH_TOKEN", hide_env_values = true)]
    pub places_admin_auth_token: Option<String>,

    /// The data team's automated-ranking bearer (also DATA_TEAM_AUTH_TOKEN_FILE).
    #[arg(long, env = "DATA_TEAM_AUTH_TOKEN", hide_env_values = true)]
    pub data_team_auth_token: Option<String>,

    /// Wallets whose signed fetch may moderate places (highlight, rating,
    /// disable), comma-separated.
    #[arg(long, env = "PLACES_ADMIN_ADDRESSES", value_delimiter = ',')]
    pub places_admin_addresses: Vec<String>,

    // explore/api: the explorer boot surface (realm provider, asset-bundle
    // registry reader, dcl-lists, denylist, feature flags, atlas map).
    /// Run the four explorer mirrors (lists, denylist, feature flags, tile
    /// grid) against the `EXPLORER_*_SOURCE_URL` sources; off, the routes
    /// serve what the database holds and the registry reader still reads
    /// `EXPLORER_REGISTRY_SOURCE_URL` on demand (when set).
    #[arg(long, env = "EXPLORER_MIRRORS_ENABLED", default_value_t = false, action = clap::ArgAction::Set)]
    pub explorer_mirrors_enabled: bool,

    /// The bounded on-disk cache behind `/ab-cdn/*` (`explore::api::ab_cdn`);
    /// a dev `serve` points it at a writable directory.
    #[arg(
        long,
        env = "AB_CDN_CACHE_DIR",
        default_value = "/var/lib/catalyst/ab-cdn"
    )]
    pub ab_cdn_cache_dir: String,

    #[arg(long, env = "AB_CDN_CACHE_MAX_BYTES", default_value_t = 21_474_836_480)]
    pub ab_cdn_cache_max_bytes: u64,

    /// The explorer mirrors' sources (the mirror posture's: Decentraland's);
    /// an empty one runs no mirror of it.
    #[arg(long, env = "EXPLORER_LISTS_SOURCE_URL", default_value = "")]
    pub explorer_lists_source_url: String,

    #[arg(long, env = "EXPLORER_DENYLIST_SOURCE_URL", default_value = "")]
    pub explorer_denylist_source_url: String,

    #[arg(long, env = "EXPLORER_FEATURE_FLAGS_SOURCE_URL", default_value = "")]
    pub explorer_feature_flags_source_url: String,

    /// The asset-bundle registry `/asset-bundle-registry/entities/*` and
    /// `/profiles*` read through; empty reads none (no bundle state, empty
    /// answers).
    #[arg(long, env = "EXPLORER_REGISTRY_SOURCE_URL", default_value = "")]
    pub explorer_registry_source_url: String,

    #[arg(long, env = "EXPLORER_MAP_SOURCE_URL", default_value = "")]
    pub explorer_map_source_url: String,

    /// Floor between two downloads of the 38 MB tile grid.
    #[arg(long, env = "EXPLORER_MAP_MIN_REFRESH_SECS", default_value_t = 3600)]
    pub explorer_map_min_refresh_secs: u64,

    /// The render directory `/satellite/{z}/{x}/{y}` is painted from
    /// (`manifest.json`, `<cx>/<cy>.png`, `<cx>/<cy>.t.png`), mirrored into
    /// the blob store with the explorer mirrors (the old satelliteMirror);
    /// unset or blank, the satellite mirror does not run.
    #[arg(long, env = "SATELLITE_MIRROR_UPSTREAM_URL")]
    pub satellite_mirror_upstream_url: Option<String>,

    #[arg(long, env = "SATELLITE_MIRROR_INTERVAL_SECS", default_value_t = 21_600)]
    pub satellite_mirror_interval_secs: u64,

    /// The render rig may be a private (tailnet) address: the satellite pass reads that one configured upstream past the SSRF
    /// guard (the relay's CATALYRST_RPC_ALLOW_PRIVATE_UPSTREAM rule).
    #[arg(long, env = "SATELLITE_MIRROR_ALLOW_PRIVATE_UPSTREAM", default_value_t = false, action = clap::ArgAction::Set)]
    pub satellite_mirror_allow_private_upstream: bool,

    /// Decoded region renders held in memory, in MiB (the old map service's budget).
    #[arg(long, env = "SATELLITE_SOURCE_BUDGET_MB", default_value_t = 256)]
    pub satellite_source_budget_mb: usize,

    /// Encoded satellite tiles held in memory.
    #[arg(long, env = "SATELLITE_OUTPUT_ENTRIES", default_value_t = 4096)]
    pub satellite_output_entries: usize,

    /// The genesis manifest served at `/world-manifest.json`, mirrored with
    /// the explorer mirrors (the old worldManifestMirror; the mirror
    /// posture's); empty runs no manifest mirror.
    #[arg(long, env = "WORLD_MANIFEST_UPSTREAM_URL", default_value = "")]
    pub world_manifest_upstream_url: String,

    #[arg(
        long,
        env = "WORLD_MANIFEST_MIRROR_INTERVAL_SECS",
        default_value_t = 86_400
    )]
    pub world_manifest_mirror_interval_secs: u64,

    /// The event poster bucket `/poster[-vertical]/{file}` reads through
    /// while `EVENTS_MIRROR_UPSTREAM` is on (the old `@upstream-poster`; the
    /// mirror posture's); empty reads none.
    #[arg(long, env = "EVENTS_POSTER_UPSTREAM_URL", default_value = "")]
    pub events_poster_upstream_url: String,

    /// Where the OpenSea resolver sends what it cannot answer (the old
    /// `@dcl_opensea` location); empty (the default) keeps its own 404.
    #[arg(long, env = "OPENSEA_FALLBACK_URL", default_value = "")]
    pub opensea_fallback_url: String,

    /// The CryptoKitties API the OpenSea resolver asks for that contract,
    /// beside an Ethereum RPC upstream only (the mirror posture's); empty
    /// skips that leg.
    #[arg(long, env = "OPENSEA_CRYPTOKITTIES_API", default_value = "")]
    pub opensea_cryptokitties_api: String,

    /// The IPFS gateways (comma-separated, in order) the OpenSea resolver
    /// reads token metadata through (the mirror posture's); empty reads no
    /// `ipfs://` metadata.
    #[arg(long, env = "OPENSEA_IPFS_GATEWAYS", default_value = "")]
    pub opensea_ipfs_gateways: String,

    /// The realm document's `comms.adapter` (the deploy tree sets
    /// `fixed-adapter:archipelago:<ws>://<domain>/ws` while Pulse and NATS run).
    #[arg(long, env = "COMMS_ADAPTER", default_value = "offline:offline")]
    pub comms_adapter: String,

    /// The adapter both `comms` blocks advertise: the realm document's
    /// (`offline:offline` when unset) and `/about`'s (`content::about_comms`,
    /// left out when unset). The deploy tree sets `archipelago:<ws>://<domain>/ws`.
    #[arg(long, env = "COMMS_FIXED_ADAPTER")]
    pub comms_fixed_adapter: Option<String>,

    /// The realm document's `bff.publicUrl`.
    #[arg(long, env = "BFF_URL", default_value = "/bff")]
    pub bff_url: String,
}

/// The imposter store and its bakes.
#[derive(Args, Clone, Debug)]
pub struct ImpostersConfig {
    // explore/imposters (the old catalyrst-bvimposters): the zip supply's
    // bounded directory beside the ONE store, filled by this node's own bakes.
    #[arg(
        long,
        env = "BVIMPOSTERS_STORE_ROOT",
        default_value = "/var/lib/catalyst/bvimposters"
    )]
    pub bvimposters_store_root: String,

    #[arg(
        long,
        env = "BVIMPOSTERS_STORE_MAX_BYTES",
        default_value_t = 21_474_836_480
    )]
    pub bvimposters_store_max_bytes: u64,

    #[arg(
        long,
        env = "BVIMPOSTERS_MEM_CACHE_BYTES",
        default_value_t = 268_435_456
    )]
    pub bvimposters_mem_cache_bytes: u64,

    /// Bake a missed tile with the engine's `impost` renderer (off: the store serves only
    /// what was seeded).
    #[arg(long, env = "BVIMPOSTERS_BAKE_ENABLED", default_value_t = false, action = clap::ArgAction::Set)]
    pub bvimposters_bake_enabled: bool,

    /// A command prefix giving the renderer a display (e.g. `xvfb-run -a`).
    #[arg(long, env = "BVIMPOSTERS_BAKE_WRAPPER")]
    pub bvimposters_bake_wrapper: Option<String>,

    /// The renderer binary; unset, `impost` from PATH.
    #[arg(long, env = "BVIMPOSTERS_IMPOST_BIN")]
    pub bvimposters_impost_bin: Option<String>,

    /// The realm the renderer loads (this node, e.g. http://127.0.0.1:5210); required
    /// with BVIMPOSTERS_BAKE_ENABLED.
    #[arg(long, env = "BVIMPOSTERS_IMPOST_SERVER")]
    pub bvimposters_impost_server: Option<String>,

    /// The catalyst root the renderer reads content from (the engine appends
    /// `/content/`); unset, the realm's own.
    #[arg(long, env = "BVIMPOSTERS_IMPOST_CONTENT_SERVER")]
    pub bvimposters_impost_content_server: Option<String>,

    /// The SDK6 adaption layer bundle the renderer runs legacy scenes with
    /// (`/renderer-artifacts/sdk6-adaption-layer/main/index.min.js` of the
    /// web client artifact); unset, an SDK6 scene bakes empty.
    #[arg(long, env = "BVIMPOSTERS_IMPOST_SDK6_ADAPTION_LAYER")]
    pub bvimposters_impost_sdk6_adaption_layer: Option<String>,

    /// The bake queue's depth, clamped to 1..=2.
    #[arg(long, env = "BVIMPOSTERS_BAKE_QUEUE_DEPTH", default_value_t = 1)]
    pub bvimposters_bake_queue_depth: u64,

    /// A bake's hard timeout; its process group is killed on expiry.
    #[arg(long, env = "BVIMPOSTERS_BAKE_TIMEOUT_SECS", default_value_t = 1800)]
    pub bvimposters_bake_timeout_secs: u64,

    /// Consecutive bake failures before a tile is quarantined.
    #[arg(long, env = "BVIMPOSTERS_BAKE_MAX_FAILURES", default_value_t = 3)]
    pub bvimposters_bake_max_failures: u64,

    #[arg(
        long,
        env = "BVIMPOSTERS_BAKE_QUARANTINE_SECS",
        default_value_t = 86_400
    )]
    pub bvimposters_bake_quarantine_secs: u64,
}

/// The asset-bundle CDN read-through and the map metadata links.
#[derive(Args, Clone, Debug)]
pub struct AbCdnConfig {
    // explore/api: the asset-bundle CDN read-through cache and the map
    // metadata links (the old catalyrst-ab-cdn bundle and catalyrst-map).
    /// The upstream asset-bundle CDN `/ab-cdn` reads through; unset or empty,
    /// none (every object comes from this node's cache and abgen).
    #[arg(long, env = "ABGEN_UPSTREAM_AB_CDN")]
    pub abgen_upstream_ab_cdn: Option<String>,

    /// Off, every /ab-cdn object answers 404 (the cache directory and budget
    /// are `AB_CDN_CACHE_DIR` / `AB_CDN_CACHE_MAX_BYTES` under explore/api above).
    #[arg(long, env = "AB_CDN_ENABLED", default_value_t = true, action = clap::ArgAction::Set)]
    pub ab_cdn_enabled: bool,

    /// What the LAND/Estate metadata `image` points at; unset, `<PUBLIC_URL>/api/v1`.
    #[arg(long, env = "MAP_IMAGE_BASE_URL")]
    pub map_image_base_url: Option<String>,

    /// The marketplace the metadata's `external_url` links to; unset yields no link.
    #[arg(long, env = "MAP_EXTERNAL_BASE_URL")]
    pub map_external_base_url: Option<String>,
}
