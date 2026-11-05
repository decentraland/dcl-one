//! The content plane's settings: sync, the validator and the /about comms block.

use clap::Args;

/// Content: peer sync and its failed-deployment retry, snapshot generation, the history
/// prune and the chain endpoint local deploys verify contract-wallet signatures against.
#[derive(Args, Clone, Debug)]
pub struct ContentConfig {
    /// Comma-separated peer content URLs (`https://host/content`). Plaintext
    /// http:// is accepted for loopback hosts only. Empty: no sync configured.
    #[arg(long, env = "SYNC_SOURCE", default_value = "http://127.0.0.1:5140")]
    pub sync_source: String,

    /// Comma-separated subset of scene,wearable,emote,store,outfits,profile;
    /// empty syncs every type. Snapshots are marked processed for this subset
    /// only, so widening it later needs an explicit resync.
    #[arg(long, env = "SYNC_ENTITY_TYPES", default_value = "")]
    pub sync_entity_types: String,

    /// Bootstrap window start, ms since the epoch: snapshots that end before
    /// it are skipped and pointer changes start there. 0 (the default) is a
    /// full bootstrap; a recent value bounds a development or demo node.
    #[arg(long, env = "SYNC_FROM_TIMESTAMP_MS", default_value_t = 0)]
    pub sync_from_timestamp_ms: i64,

    /// Non-profile types first, then profiles; /about reports the ready types.
    #[arg(long, env = "PHASED_SYNC", default_value_t = true, action = clap::ArgAction::Set)]
    pub phased_sync: bool,

    /// https L1 JSON-RPC endpoint: the EIP-1654 (contract wallet) signature
    /// check and the L1 deploy access checks (LAND, names, v1 items) of local
    /// deployments; the L2 checks use RPC_ENDPOINT_POLYGON. Plaintext http://
    /// is refused at startup. Unset or empty:
    /// https://rpc.decentraland.org/<mainnet|sepolia>?project=catalyst-content
    /// by ETH_NETWORK, except under NODE_POSTURE island and journey. May come
    /// from ETH_RPC_URL_FILE; an empty file fails startup.
    #[arg(long, env = "ETH_RPC_URL", hide_env_values = true)]
    pub eth_rpc_url: Option<String>,

    /// Content files downloaded at once across every peer.
    #[arg(long, env = "CONCURRENT_SYNC_DOWNLOADS", default_value_t = 200)]
    pub concurrent_sync_downloads: usize,

    /// Idle HTTP connections kept per peer host.
    #[arg(long, env = "CONNECTIONS_MAX_IDLE", default_value_t = 25)]
    pub connections_max_idle: usize,

    /// Snapshots deployed at once per bootstrap pass.
    #[arg(long, env = "SYNC_SNAPSHOT_CONCURRENCY", default_value_t = 4)]
    pub sync_snapshot_concurrency: usize,

    /// Non-scene entities of one snapshot handed to the deployer at once.
    #[arg(long, env = "SYNC_NONSCENE_CONCURRENCY", default_value_t = 64)]
    pub sync_nonscene_concurrency: usize,

    /// Scenes of one snapshot handed to the deployer at once.
    #[arg(long, env = "SYNC_SCENE_CONCURRENCY", default_value_t = 1)]
    pub sync_scene_concurrency: usize,

    /// Deployment batches flushed to the database at once.
    #[arg(long, env = "SYNC_FLUSH_CONCURRENCY", default_value_t = 8)]
    pub sync_flush_concurrency: usize,

    /// Bytes of entity and content file bodies held in memory across every
    /// in-flight sync download; a single larger file proceeds alone.
    #[arg(long, env = "SYNC_INFLIGHT_BYTES_MAX", default_value_t = 1024 * 1024 * 1024)]
    pub sync_inflight_bytes_max: u64,

    /// Synced entities buffered ahead of the database flush before the
    /// producers wait.
    #[arg(long, env = "SYNC_FLUSH_BACKLOG_MAX", default_value_t = 4096)]
    pub sync_flush_backlog_max: usize,

    /// Profiles older than this many days are not synced; 0 syncs every profile, which a node
    /// that must answer for every active pointer of the network (a cutover twin) needs.
    #[arg(long, env = "SYNC_PROFILE_MAX_AGE_DAYS", default_value_t = 365)]
    pub sync_profile_max_age_days: u32,

    /// Bloom filter capacity for the already-deployed probe.
    #[arg(
        long,
        env = "BLOOM_FILTER_EXPECTED_ELEMENTS",
        default_value_t = 10_000_000
    )]
    pub bloom_filter_expected_elements: usize,

    /// Run the failed-deployments retry loop beside the sync worker.
    #[arg(long, env = "RETRY_FAILED_ENABLED", default_value_t = true, action = clap::ArgAction::Set)]
    pub retry_failed_enabled: bool,

    /// Attempts a failed entity gets before its record is dropped.
    #[arg(long, env = "MAX_FAILED_DEPLOYMENT_RETRIES", default_value_t = 10)]
    pub max_failed_deployment_retries: u32,

    /// Due failed entities re-attempted at once per retry cycle.
    #[arg(long, env = "SYNC_RETRY_CONCURRENCY", default_value_t = 10)]
    pub sync_retry_concurrency: usize,

    /// Failed-deployment records older than this are pruned daily.
    #[arg(long, env = "RETRY_FAILED_PRUNE_TTL_DAYS", default_value_t = 7)]
    pub retry_failed_prune_ttl_days: i64,

    /// Hours between snapshot generation passes (`content::snapshot_gen`:
    /// the time-windowed files `/snapshots` lists, what lets another
    /// catalyst name this node in its SYNC_SOURCE); 0 turns generation off.
    #[arg(long, env = "SNAPSHOT_GENERATION_INTERVAL_HOURS", default_value_t = 6)]
    pub snapshot_generation_interval_hours: u64,

    // content history prune (src/content/history_prune.rs): a sync-only node
    // drops the deployments no pointer is served by any more (superseded or
    // unpublished), their content_files and the blobs nothing else names.
    // Off by default, and never beside CONTENT_WRITES_ENABLED: an
    // authoritative node keeps its history.
    #[arg(long, env = "CONTENT_HISTORY_PRUNE_ENABLED", default_value_t = false, action = clap::ArgAction::Set)]
    pub content_history_prune_enabled: bool,

    /// Hours a row stays after it left active_pointers before the prune
    /// takes it; covers what a restart's pointer-changes replay can re-offer.
    #[arg(long, env = "CONTENT_HISTORY_PRUNE_GRACE_HOURS", default_value_t = 24)]
    pub content_history_prune_grace_hours: u64,

    /// Hours between prune passes.
    #[arg(
        long,
        env = "CONTENT_HISTORY_PRUNE_INTERVAL_HOURS",
        default_value_t = 6
    )]
    pub content_history_prune_interval_hours: u64,

    /// Deployments deleted per transaction; a pass loops, with a pause in
    /// between, until a batch comes back short.
    #[arg(long, env = "CONTENT_HISTORY_PRUNE_BATCH", default_value_t = 2000)]
    pub content_history_prune_batch: usize,

    /// A read that misses (an entity by pointer or id, a content file, a
    /// profile, a mirrored world's scenes) asks the SYNC_SOURCE peers, in
    /// order, and keeps a verified copy as a synced deployment
    /// (`content::read_through`). Off: a miss answers what it always did.
    #[arg(long, env = "READ_THROUGH_ENABLED", default_value_t = false, action = clap::ArgAction::Set)]
    pub read_through_enabled: bool,

    /// Non-file fields a `POST /entities` form may carry: `entityId` and the
    /// `type`/`payload`/`signature` of up to ten auth-chain links, and one spare.
    #[arg(long, env = "MAX_UPLOAD_FIELD_COUNT", default_value_t = 2 + 3 * 10)]
    pub max_upload_field_count: usize,

    /// Bytes one non-file `POST /entities` field may hold.
    #[arg(long, env = "MAX_UPLOAD_FIELD_SIZE", default_value_t = 32 * 1024)]
    pub max_upload_field_size: usize,

    /// Bytes of `POST /entities` bodies held in memory at once across every
    /// request, each holding at least 16 MiB; never below one maximum-size
    /// deployment. A request that does not fit answers 503 unread.
    #[arg(
        long,
        env = "MAX_IN_MEMORY_DEPLOYMENT_BYTES",
        default_value_t = 2 * 1024 * 1024 * 1024
    )]
    pub max_in_memory_deployment_bytes: u64,

    /// `POST /entities` requests one client address (IPv6 by /64) may make per
    /// 24 hours, counted on admission and given back when the deployer answers 503
    /// for this node's own state (not for an unchecked contract-wallet signature);
    /// 0 turns the quota off. Kept per process: N upstreams admit N times it,
    /// and a restart resets it.
    #[arg(long, env = "POST_ENTITIES_DAILY_QUOTA_MAX", default_value_t = 300)]
    pub post_entities_daily_quota_max: u32,

    /// `POST /entities` requests one client address (IPv6 by /64) may make per
    /// `POST_ENTITIES_RATE_LIMIT_WINDOW_SECONDS`, every request counted, before
    /// a 429 with `Retry-After`; upstream's burst limit. 0 turns it off. Kept
    /// per process, like the daily quota.
    #[arg(long, env = "POST_ENTITIES_RATE_LIMIT_MAX", default_value_t = 200)]
    pub post_entities_rate_limit_max: u32,

    /// The burst limit's fixed window, in seconds.
    #[arg(
        long,
        env = "POST_ENTITIES_RATE_LIMIT_WINDOW_SECONDS",
        default_value_t = 60
    )]
    pub post_entities_rate_limit_window_seconds: u64,

    // Worlds' partial uploads also read these (not the cleanup interval or pointer cap) as their own budget.
    /// Milliseconds a partial (multi-request) upload lives from its first
    /// batch; batches do not extend it.
    #[arg(long, env = "PENDING_DEPLOYMENT_TTL", default_value_t = 60 * 60 * 1000)]
    pub pending_deployment_ttl_ms: u64,

    /// Milliseconds between two runs of the expired partial-upload cleanup.
    #[arg(long, env = "PENDING_DEPLOYMENTS_CLEANUP_INTERVAL", default_value_t = 5 * 60 * 1000)]
    pub pending_deployments_cleanup_interval_ms: u64,

    /// Partial uploads one account may hold, expired ones awaiting cleanup included.
    #[arg(
        long,
        env = "MAX_PENDING_DEPLOYMENTS_PER_DEPLOYER",
        default_value_t = 10
    )]
    pub max_pending_deployments_per_deployer: u32,

    /// Live partial uploads holding any one parcel across accounts; one
    /// account gets all but one of them (at least 1).
    #[arg(long, env = "MAX_PENDING_UPLOADS_PER_POINTER", default_value_t = 4)]
    pub max_pending_uploads_per_pointer: u32,

    /// Bytes partial uploads hold per account (all but content a published
    /// deployment already keeps), expired uploads included until cleanup.
    #[arg(long, env = "MAX_PENDING_BYTES_PER_DEPLOYER", default_value_t = 1024 * 1024 * 1024)]
    pub max_pending_bytes_per_deployer: u64,

    /// Bytes partial uploads hold on this server.
    #[arg(long, env = "MAX_PENDING_BYTES", default_value_t = 50 * 1024 * 1024 * 1024)]
    pub max_pending_bytes: u64,

    /// Partial-upload batch bytes one account may send per minute; at least
    /// a batch of the entity file (10 MiB) and one 100 MiB file, or startup
    /// fails, as upstream's.
    #[arg(
        long,
        env = "MAX_PARTIAL_UPLOAD_BYTES_PER_MINUTE",
        default_value_t = 512 * 1024 * 1024,
        value_parser = clap::value_parser!(u64).range(MIN_PARTIAL_UPLOAD_BYTES_PER_MINUTE..)
    )]
    pub max_partial_upload_bytes_per_minute: u64,

    /// `POST /entities` bodies one client address (IPv6 by /64) may have in
    /// flight at once on this process, before a 429 with `Retry-After: 5`.
    /// Upstream's catalyst and worlds-content-server read the same variable,
    /// so the worlds multipart uploads take it too.
    #[arg(
        long,
        env = "MAX_CONCURRENT_UPLOADS_PER_SOURCE",
        default_value_t = 4,
        value_parser = clap::value_parser!(u32).range(1..)
    )]
    pub max_concurrent_uploads_per_source: u32,

    /// Bytes of `POST /entities` bodies (declared, else the 200 MiB cap) one
    /// client address may have in flight; unset, one maximum-size upload.
    /// Below that startup fails. The worlds uploads read it too; unset, they
    /// default to one 350 MiB world deploy.
    #[arg(
        long,
        env = "MAX_IN_FLIGHT_UPLOAD_BYTES_PER_SOURCE",
        value_parser = clap::value_parser!(u64).range(MAX_UPLOAD_TOTAL_BYTES..)
    )]
    pub max_in_flight_upload_bytes_per_source: Option<u64>,

    /// How old an entity's timestamp may be when deployed (a duration such
    /// as `20m`, or milliseconds).
    #[arg(long, env = "REQUEST_TTL_BACKWARDS", default_value = "20m", value_parser = ms_duration)]
    pub request_ttl_backwards_ms: u64,

    /// The per-pointer deploy rate limit of every entity type without its
    /// own setting below: a pointer deployed within the window (a duration)
    /// is refused, as is any deploy while more than `_MAX` pointers are
    /// held. Unset, each type keeps this node's built-in window.
    #[arg(long, env = "DEPLOYMENTS_DEFAULT_RATE_LIMIT_TTL", value_parser = ms_duration)]
    pub deployments_default_rate_limit_ttl_ms: Option<u64>,
    #[arg(long, env = "DEPLOYMENTS_DEFAULT_RATE_LIMIT_MAX")]
    pub deployments_default_rate_limit_max: Option<u64>,

    /// Per-type overrides of the deploy rate limit (`DEPLOYMENT_RATE_LIMIT_TTL_<TYPE>`, a
    /// duration, and `DEPLOYMENT_RATE_LIMIT_MAX_<TYPE>`).
    #[arg(long, env = "DEPLOYMENT_RATE_LIMIT_TTL_SCENE", value_parser = ms_duration)]
    pub deployment_rate_limit_ttl_scene_ms: Option<u64>,
    #[arg(long, env = "DEPLOYMENT_RATE_LIMIT_TTL_PROFILE", value_parser = ms_duration)]
    pub deployment_rate_limit_ttl_profile_ms: Option<u64>,
    #[arg(long, env = "DEPLOYMENT_RATE_LIMIT_TTL_WEARABLE", value_parser = ms_duration)]
    pub deployment_rate_limit_ttl_wearable_ms: Option<u64>,
    #[arg(long, env = "DEPLOYMENT_RATE_LIMIT_TTL_STORE", value_parser = ms_duration)]
    pub deployment_rate_limit_ttl_store_ms: Option<u64>,
    #[arg(long, env = "DEPLOYMENT_RATE_LIMIT_TTL_EMOTE", value_parser = ms_duration)]
    pub deployment_rate_limit_ttl_emote_ms: Option<u64>,
    #[arg(long, env = "DEPLOYMENT_RATE_LIMIT_TTL_OUTFITS", value_parser = ms_duration)]
    pub deployment_rate_limit_ttl_outfits_ms: Option<u64>,
    #[arg(long, env = "DEPLOYMENT_RATE_LIMIT_MAX_SCENE")]
    pub deployment_rate_limit_max_scene: Option<u64>,
    #[arg(long, env = "DEPLOYMENT_RATE_LIMIT_MAX_PROFILE")]
    pub deployment_rate_limit_max_profile: Option<u64>,
    #[arg(long, env = "DEPLOYMENT_RATE_LIMIT_MAX_WEARABLE")]
    pub deployment_rate_limit_max_wearable: Option<u64>,
    #[arg(long, env = "DEPLOYMENT_RATE_LIMIT_MAX_STORE")]
    pub deployment_rate_limit_max_store: Option<u64>,
    #[arg(long, env = "DEPLOYMENT_RATE_LIMIT_MAX_EMOTE")]
    pub deployment_rate_limit_max_emote: Option<u64>,
    #[arg(long, env = "DEPLOYMENT_RATE_LIMIT_MAX_OUTFITS")]
    pub deployment_rate_limit_max_outfits: Option<u64>,
}

/// The total of every value one `POST /entities` form may carry (the
/// content crate's `MAX_DEPLOYMENT_BYTES`).
const MAX_UPLOAD_TOTAL_BYTES: u64 = 200 * 1024 * 1024;

/// A partial batch of the entity file (10 MiB) and one maximum-size
/// (100 MiB) file.
const MIN_PARTIAL_UPLOAD_BYTES_PER_MINUTE: u64 = (10 + 100) * 1024 * 1024;

/// A duration as the `ms` package reads one: a number with an optional
/// unit (`ms`, `s`, `m`, `h`, `d`, `w`, `y` and their long forms), bare
/// milliseconds without one.
fn ms_duration(raw: &str) -> Result<u64, String> {
    let text = raw.trim().to_ascii_lowercase();
    let split = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let value: f64 = number
        .parse()
        .map_err(|_| format!("expected a duration such as 20m or 1200000, got {raw:?}"))?;
    let unit_ms = match unit.trim() {
        "" | "ms" | "msec" | "msecs" | "millisecond" | "milliseconds" => 1.0,
        "s" | "sec" | "secs" | "second" | "seconds" => 1e3,
        "m" | "min" | "mins" | "minute" | "minutes" => 60e3,
        "h" | "hr" | "hrs" | "hour" | "hours" => 3_600e3,
        "d" | "day" | "days" => 86_400e3,
        "w" | "week" | "weeks" => 604_800e3,
        "y" | "yr" | "yrs" | "year" | "years" => 31_557_600e3,
        other => return Err(format!("unknown duration unit {other:?} in {raw:?}")),
    };
    Ok((value * unit_ms).round() as u64)
}

impl ContentConfig {
    /// `ETH_RPC_URL` trimmed, `None` when unset or blank (after
    /// `Config::default_rpc_urls`, only under island and journey). A plaintext endpoint is refused:
    /// the contract's answer decides who may deploy, so it must come over TLS. The error names
    /// the host only; the URL commonly carries an API key.
    pub fn eth_rpc_url(&self) -> anyhow::Result<Option<String>> {
        let Some(raw) = self
            .eth_rpc_url
            .as_deref()
            .map(str::trim)
            .filter(|u| !u.is_empty())
        else {
            return Ok(None);
        };
        let parsed = url::Url::parse(raw)
            .map_err(|e| anyhow::anyhow!("ETH_RPC_URL is not a valid URL: {e}"))?;
        if parsed.scheme() != "https" {
            anyhow::bail!(
                "ETH_RPC_URL is {}:// to {}; EIP-1654 signature validation requires a trusted \
                 TLS (https://) endpoint. Refusing to start",
                parsed.scheme(),
                parsed.host_str().unwrap_or("?")
            );
        }
        Ok(Some(raw.to_string()))
    }
}

/// The content validator: the access policy of local deploys.
#[derive(Args, Clone, Debug)]
pub struct ValidatorConfig {
    // content validator (src/content/validator): the access policy of local
    // deploys (the old catalyrst-server names); its chain checks read
    // ETH_RPC_URL and RPC_ENDPOINT_POLYGON.
    /// Third Party Registry subgraph on L2 (the approved providers the shop lists); public https.
    #[arg(
        long,
        env = "THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL",
        hide_env_values = true
    )]
    pub third_party_registry_l2_subgraph_url: Option<String>,

    /// Skip the chain-derived access checks of local deploys (LAND, names, items, collections, third party); the pointer-ownership rule still runs.
    #[arg(long, env = "IGNORE_BLOCKCHAIN_ACCESS_CHECKS", default_value_t = false, action = clap::ArgAction::Set)]
    pub ignore_blockchain_access_checks: bool,

    /// A second address with the Decentraland address's deploy rights.
    #[arg(long, env = "ADDITIONAL_DECENTRALAND_ADDRESS")]
    pub additional_decentraland_address: Option<String>,
}

/// The /about comms block (`content::about_comms`) and the v4 discovery it advertises.
#[derive(Args, Clone, Debug)]
pub struct AboutCommsConfig {
    // content::about_comms: the /about comms block, advertised when
    // COMMS_FIXED_ADAPTER (above) is set; healthy while archipelago's core
    // status is (the clustering engine's engine.discovery heartbeat).
    #[arg(long, env = "COMMS_PROTOCOL", default_value = "v3")]
    pub comms_protocol: String,

    #[arg(long, env = "COMMS_VERSION")]
    pub comms_version: Option<String>,

    #[arg(long, env = "COMMS_COMMIT_HASH")]
    pub comms_commit_hash: Option<String>,

    /// /about's acceptingUsers turns false at this many core users; unset, no cap.
    #[arg(long, env = "MAX_USERS")]
    pub max_users: Option<u64>,

    /// Probe the advertised adapter (a LiveKit or gatekeeper URL; an
    /// archipelago adapter has nothing to probe) and advertise offline:offline
    /// after three failed probes.
    #[arg(long, env = "COMMS_OFFLINE_WHEN_UNREACHABLE", default_value_t = true, action = clap::ArgAction::Set)]
    pub comms_offline_when_unreachable: bool,

    /// The v4 discovery /about advertises under comms.v4 (wss, or ws on loopback).
    #[arg(long, env = "COMMS_V4_CONTROL_URL")]
    pub comms_v4_control_url: Option<String>,

    #[arg(long, env = "COMMS_V4_CONTROL_AUDIENCE")]
    pub comms_v4_control_audience: Option<String>,

    /// host:port of Pulse's ENet listener.
    #[arg(long, env = "COMMS_V4_PULSE_NATIVE_ENDPOINT")]
    pub comms_v4_pulse_native_endpoint: Option<String>,

    #[arg(long, env = "COMMS_V4_PULSE_WEBTRANSPORT_URL")]
    pub comms_v4_pulse_webtransport_url: Option<String>,

    /// Hex SHA-256 of Pulse's self-signed WebTransport certificate; advertised only
    /// beside a loopback COMMS_V4_PULSE_WEBTRANSPORT_URL, for serverCertificateHashes.
    #[arg(long, env = "COMMS_V4_PULSE_CERTIFICATE_HASH")]
    pub comms_v4_pulse_certificate_hash: Option<String>,

    #[arg(long, env = "COMMS_V4_PULSE_AUDIENCE")]
    pub comms_v4_pulse_audience: Option<String>,

    #[arg(long, env = "COMMS_V4_ISLAND_REFRESH_URL")]
    pub comms_v4_island_refresh_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use crate::config::Config;

    fn parse(args: &[&str]) -> Result<Config, clap::Error> {
        Config::parse_for_tests(args)
    }

    #[test]
    fn eth_rpc_url_is_optional_must_be_https_and_never_echoed() {
        let cfg = |url: &str| {
            parse(&[
                "x",
                "--database-url",
                "postgres:///catalyst",
                "--eth-rpc-url",
                url,
            ])
            .unwrap()
        };
        assert_eq!(cfg("  ").content.eth_rpc_url().unwrap(), None);
        assert_eq!(
            cfg(" https://rpc.example/v3/SECRET ")
                .content
                .eth_rpc_url()
                .unwrap()
                .as_deref(),
            Some("https://rpc.example/v3/SECRET")
        );
        let err = cfg("http://rpc.example/v3/SECRET")
            .content
            .eth_rpc_url()
            .unwrap_err()
            .to_string();
        assert!(err.contains("http:// to rpc.example"), "{err}");
        assert!(
            !err.contains("SECRET"),
            "the refusal must not print the URL: {err}"
        );
        assert!(cfg("not a url").content.eth_rpc_url().is_err());
    }

    #[test]
    fn write_path_limits_read_upstreams_names_and_refuse_what_cannot_work() {
        let base = ["x", "--database-url", "postgres:///catalyst"];
        let cli = parse(&base).unwrap();
        assert_eq!(cli.content.request_ttl_backwards_ms, 20 * 60 * 1000);
        assert_eq!(cli.content.max_concurrent_uploads_per_source, 4);
        assert_eq!(cli.content.max_in_flight_upload_bytes_per_source, None);
        assert_eq!(cli.content.deployments_default_rate_limit_ttl_ms, None);
        let with = |extra: &[&str]| parse(&[&base[..], extra].concat());
        let cli = with(&[
            "--request-ttl-backwards-ms",
            "90s",
            "--deployments-default-rate-limit-ttl-ms",
            "1m",
            "--deployment-rate-limit-ttl-profile-ms",
            "1500",
        ])
        .unwrap();
        assert_eq!(cli.content.request_ttl_backwards_ms, 90_000);
        assert_eq!(
            cli.content.deployments_default_rate_limit_ttl_ms,
            Some(60_000)
        );
        assert_eq!(cli.content.deployment_rate_limit_ttl_profile_ms, Some(1500));
        assert!(with(&["--request-ttl-backwards-ms", "soon"]).is_err());
        // A rate below one entity file and one maximal file, a source share
        // below one maximal upload, or no upload at all refuse to start.
        assert!(with(&["--max-partial-upload-bytes-per-minute", "1048576"]).is_err());
        assert!(with(&["--max-in-flight-upload-bytes-per-source", "1048576"]).is_err());
        assert!(with(&["--max-concurrent-uploads-per-source", "0"]).is_err());
    }

    #[test]
    fn sync_defaults_are_the_old_binary_values() {
        let cli = parse(&["x", "--database-url", "postgres:///catalyst"]).unwrap();
        let c = cli;
        assert_eq!(c.content.sync_source, "http://127.0.0.1:5140");
        assert_eq!(c.content.sync_entity_types, "");
        assert!(c.content.phased_sync);
        assert_eq!(c.content.sync_from_timestamp_ms, 0);
        assert!(c.content.eth_rpc_url.is_none());
        assert_eq!(c.content.concurrent_sync_downloads, 200);
        assert_eq!(c.content.connections_max_idle, 25);
        assert_eq!(c.content.sync_snapshot_concurrency, 4);
        assert_eq!(c.content.sync_nonscene_concurrency, 64);
        assert_eq!(c.content.sync_scene_concurrency, 1);
        assert_eq!(c.content.sync_flush_concurrency, 8);
        assert_eq!(c.content.sync_inflight_bytes_max, 1024 * 1024 * 1024);
        assert_eq!(c.content.sync_flush_backlog_max, 4096);
        assert_eq!(c.content.bloom_filter_expected_elements, 10_000_000);
        assert_eq!(c.content.sync_profile_max_age_days, 365);
        assert!(!c.content.read_through_enabled);
        assert_eq!(
            (
                c.content.max_upload_field_count,
                c.content.max_upload_field_size,
                c.content.max_in_memory_deployment_bytes,
                c.content.post_entities_daily_quota_max,
                c.content.post_entities_rate_limit_max,
                c.content.post_entities_rate_limit_window_seconds
            ),
            (32, 32 * 1024, 2 * 1024 * 1024 * 1024, 300, 200, 60)
        );
        assert!(c.content.retry_failed_enabled);
        assert_eq!(c.content.max_failed_deployment_retries, 10);
        assert_eq!(c.content.sync_retry_concurrency, 10);
        assert_eq!(c.content.retry_failed_prune_ttl_days, 7);
        assert_eq!(c.content.snapshot_generation_interval_hours, 6);
        assert_eq!(
            (
                c.content.content_history_prune_enabled,
                c.content.content_history_prune_grace_hours,
                c.content.content_history_prune_interval_hours,
                c.content.content_history_prune_batch
            ),
            (false, 24, 6, 2000),
            "the history prune is opt-in, with a day of grace, six-hourly passes and batches of 2000"
        );

        let cli = parse(&[
            "x",
            "--database-url",
            "postgres:///catalyst",
            "--sync-enabled",
            "--phased-sync",
            "false",
            "--retry-failed-enabled",
            "false",
            "--sync-entity-types",
            "store,outfits",
        ])
        .unwrap();
        assert!(!cli.content.phased_sync);
        assert!(!cli.content.retry_failed_enabled);
        assert_eq!(cli.content.sync_entity_types, "store,outfits");
    }

    #[test]
    fn data_plane_defaults_are_the_old_binaries_values_and_writes_are_off() {
        let cli = parse(&["x", "--database-url", "postgres:///catalyst"]).unwrap();
        let c = cli;
        assert!(c.validator.third_party_registry_l2_subgraph_url.is_none());
        assert!(
            !c.validator.ignore_blockchain_access_checks
                && c.validator.additional_decentraland_address.is_none(),
            "local deploys run every access check"
        );
    }
}
