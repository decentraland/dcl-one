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

    /// https JSON-RPC endpoint the EIP-1654 (contract wallet) signature check
    /// of local deployments calls; peer sync does not use it. Plaintext
    /// http:// is refused at startup.
    #[arg(long, env = "ETH_RPC_URL")]
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
    // authoritative node keeps its history (an unpublish there restores the
    // scene a local deployment superseded).
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
}

impl ContentConfig {
    /// `ETH_RPC_URL` trimmed, `None` when unset or blank. A plaintext endpoint is refused:
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

/// The content validator: the L2 subgraphs and the access policy of local deploys.
#[derive(Args, Clone, Debug)]
pub struct ValidatorConfig {
    // content validator (src/content/validator): the L2 subgraphs a
    // third-party wearable or emote deploy is proven against, and the access
    // policy of local deploys (the old catalyrst-server names).
    /// Third Party Registry subgraph on L2 (approved merkle roots); public https, set with BLOCKS_L2_SUBGRAPH_URL.
    #[arg(
        long,
        env = "THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL",
        hide_env_values = true
    )]
    pub third_party_registry_l2_subgraph_url: Option<String>,

    /// L2 blocks subgraph (the block a deploy's timestamp falls in); public https.
    #[arg(long, env = "BLOCKS_L2_SUBGRAPH_URL", hide_env_values = true)]
    pub blocks_l2_subgraph_url: Option<String>,

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
        assert!(
            c.validator.third_party_registry_l2_subgraph_url.is_none()
                && c.validator.blocks_l2_subgraph_url.is_none()
        );
        assert!(
            !c.validator.ignore_blockchain_access_checks
                && c.validator.additional_decentraland_address.is_none(),
            "local deploys run every access check"
        );
    }
}
