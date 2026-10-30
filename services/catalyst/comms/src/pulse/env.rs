//! The `catalyst pulse` process: the `PULSE_*` environment contract of the
//! old executable, read exactly as it read it (unset and blank mean the
//! default, a typo fails startup), and the server assembled from it.

use std::env::VarError;
use std::sync::Arc;

use super::PulseServer;
use super::cluster::feed::{ClusterFeedPublisher, NoopClusterFeedPublisher};
use super::cluster::nats::{
    DEFAULT_CHANNEL_CAPACITY, DEFAULT_COMMIT_HASH, DEFAULT_DISCOVERY_INTERVAL_MS,
    DEFAULT_SERVER_NAME, NatsClusterFeed, NatsFeedOptions,
};
use super::cluster::{
    ClusterOptions, ClusterTracker, DEFAULT_CLUSTERS_ENABLED, DEFAULT_DWELL_PASSES,
    DEFAULT_ID_PREFIX, DEFAULT_PASS_INTERVAL_MS, DEFAULT_SESSION_RETENTION_PASSES,
};
use super::handshake::MAX_TIMESTAMP_SKEW_MS;
use super::hardening::{
    DEFAULT_DISCRETE_BURST, DEFAULT_DISCRETE_RATE_PER_SEC, DEFAULT_INPUT_BURST,
    DEFAULT_INPUT_MAX_HZ, DisconnectReason, GameplayRateLimiter, HandshakeReplayPolicy,
    IpWhitelist,
};
use super::interest::{SpatialAreaOfInterest, SpatialAreaOfInterestOptions};
use super::server::{ENET_CAPACITY, WT_CAPACITY};
use super::transport::webtransport::WtConfig;
use super::transport::webtransport::config::{
    DEFAULT_MAX_DATAGRAM_BYTES, DEFAULT_MAX_MESSAGE_BYTES, DEFAULT_SERVICE_TIMEOUT_MS,
};

const DEFAULT_BIND: &str = "0.0.0.0:9000";
const DEFAULT_WT_BIND: &str = "0.0.0.0:7743";
pub const DEFAULT_LOG_FILTER: &str = "catalyst=info";
const DEFAULT_REPLAY_STATE_PATH: &str = "data/pulse-replay.tsv";

const BOOL_TRUE: &[&str] = &["1", "true", "yes", "on"];
const BOOL_FALSE: &[&str] = &["0", "false", "no", "off"];

/// Literal pairs on purpose: this table is the process's env contract, and the defaults quoted
/// here are pinned to the constants by `tests::env_docs_defaults_track_the_constants`, so a
/// lockstep bump of a constant fails the build until the prose follows.
pub const ENV_DOCS: &[(&str, &str)] = &[
    (
        "PULSE_LEGACY_HANDSHAKE",
        "obsolete authentication request, validation only: strict lowercase true/false; absent or blank keeps legacy public authentication; accepts true, rejects false as unsupported",
    ),
    (
        "PULSE_V4_ENABLED",
        "obsolete authentication request, validation only: strict lowercase true/false; absent or blank keeps public V4 authentication unavailable; accepts false, rejects true as unavailable",
    ),
    (
        "COMMS_MODERATOR_TOKEN",
        "required moderator bearer for the Pulse ban poll",
    ),
    (
        "PULSE_BANS_URL",
        "required HTTPS moderator GET /bans endpoint (loopback HTTP permitted)",
    ),
    (
        "PULSE_BANS_POLL_INTERVAL_SECONDS",
        "positive ban poll interval (default 30)",
    ),
    ("PULSE_BIND", "ENet/UDP bind address (default 0.0.0.0:9000)"),
    (
        "PULSE_METRICS_BIND",
        "prometheus /metrics bind address (default 127.0.0.1:5005)",
    ),
    (
        "PULSE_INPUT_MAX_HZ",
        "gameplay input rate cap in Hz (default 20)",
    ),
    (
        "PULSE_INPUT_BURST",
        "gameplay input burst allowance (default 16)",
    ),
    (
        "PULSE_DISCRETE_RATE_PER_SEC",
        "discrete-action rate cap per second (default 20)",
    ),
    (
        "PULSE_DISCRETE_BURST",
        "discrete-action burst allowance (default 16)",
    ),
    (
        "PULSE_SCENE_LISTENER_MAX_PARCELS",
        "scene-listener AoI budget in parcels: sum of rect areas over every announced realm plus 4 per realm (default 4096)",
    ),
    (
        "PULSE_IP_LIMITER_ENABLED",
        "enforce concurrent source-IP connection caps; still counts when disabled (default false)",
    ),
    (
        "PULSE_IP_LIMITER_MAX_CONCURRENCY",
        "concurrent players per source IP across ENet and WebTransport, including pending authentication; 0 means unlimited (default 10)",
    ),
    (
        "PULSE_IP_LIMITER_SCENE_LISTENER_MAX_CONCURRENCY",
        "concurrent validated listeners per source IP across both transports; 0 means unlimited (default 2)",
    ),
    (
        "PULSE_FLAGS_URL",
        "explicit full pulse.json URL; HTTPS (loopback HTTP permitted), unset disables remote configuration; initial fetch 5s, request timeout 10s, poll 60s, body at most 1MiB; failures retain last good settings, removed keys restore local settings",
    ),
    (
        "PULSE_IP_LIMITER_WHITELIST",
        "comma-separated exact source IPs of trusted infrastructure, IPv4 or IPv6 (a dotted entry also matches the v4-mapped peer), no CIDR; exempts connection caps and PULSE_SCENE_LISTENER_MAX_PARCELS on handshake or update; still counted, auth/bans/pre-auth/global transport bounds and other AoI rules apply; an entry that is not an IP fails startup (default unset)",
    ),
    (
        "PULSE_AOI_TIER0_RADIUS",
        "player AoI tier-0 radius in world units: peers this close get full-detail updates (default 30)",
    ),
    (
        "PULSE_AOI_TIER1_RADIUS",
        "player AoI tier-1 radius in world units: peers between tier-0 and this get reduced-detail updates (default 60)",
    ),
    (
        "PULSE_AOI_MAX_RADIUS",
        "player AoI cutoff in world units: peers between tier-1 and this get position-only updates, beyond it they are invisible; at most 800 (default 200)",
    ),
    (
        "PULSE_WT_ENABLED",
        "enables the WebTransport front door: 1/true/yes/on or 0/false/no/off, case-insensitive, unset or blank is off, anything else fails startup (default off)",
    ),
    (
        "PULSE_WT_BIND",
        "WebTransport bind address (default 0.0.0.0:7743)",
    ),
    (
        "PULSE_WT_CERT_PEM",
        "inline TLS certificate PEM for WebTransport (takes precedence over PULSE_WT_CERT_PATH)",
    ),
    (
        "PULSE_WT_CERT_PATH",
        "path to the WebTransport TLS certificate PEM",
    ),
    (
        "PULSE_WT_KEY_PEM",
        "inline TLS key PEM for WebTransport (takes precedence over PULSE_WT_KEY_PATH)",
    ),
    ("PULSE_WT_KEY_PATH", "path to the WebTransport TLS key PEM"),
    (
        "PULSE_WT_MAX_DATAGRAM_BYTES",
        "max WebTransport datagram size in bytes (default 1200)",
    ),
    (
        "PULSE_WT_MAX_MESSAGE_BYTES",
        "max WebTransport message size in bytes (default 4096)",
    ),
    (
        "PULSE_CLUSTERS_ENABLED",
        "derive peer clusters every pass: 1/true/yes/on or 0/false/no/off, case-insensitive, blank or unset keeps the default, anything else fails startup (default true)",
    ),
    (
        "PULSE_CLUSTERS_PASS_INTERVAL_MS",
        "milliseconds between clustering passes; not a bound on broker delivery or client convergence (default 1000)",
    ),
    (
        "PULSE_CLUSTERS_DWELL_PASSES",
        "consecutive passes that must agree on a new assignment before it is published; 1 disables the debounce (default 3)",
    ),
    (
        "PULSE_CLUSTERS_ID_PREFIX",
        "prefix of every minted cluster id, so two Pulse instances on one broker never mint the same one (default C)",
    ),
    (
        "PULSE_CLUSTERS_SESSION_RETENTION_PASSES",
        "passes a departed wallet's last published assignment is retained so a new session can name it as displaced; 0 disables the annotation (default 300)",
    ),
    (
        "PULSE_NATS_URL",
        "broker URL for the cluster feed; blank or unset leaves the tracker in stats-only mode, deriving clusters and reporting metrics while publishing nothing (default unset)",
    ),
    (
        "NATS_URL",
        "fallback broker URL, read only when PULSE_NATS_URL is blank or unset (default unset)",
    ),
    (
        "PULSE_NATS_SERVER_NAME",
        "name this server announces on the engine.discovery heartbeat, and the connection name the broker shows on /connz (default pulse)",
    ),
    (
        "COMMIT_HASH",
        "commit of the deployed build, announced on the engine.discovery heartbeat so consumers can tell two deployments of one version apart (default unknown)",
    ),
    (
        "PULSE_NATS_DISCOVERY_INTERVAL_MS",
        "milliseconds between engine.discovery heartbeats (default 10000)",
    ),
    (
        "PULSE_NATS_CHANNEL_CAPACITY",
        "distinct peers that may hold an undelivered assignment at once; past it the longest-admitted is evicted and counted on pulse_nats_dropped_total (default 1024)",
    ),
    (
        "PULSE_REPLAY_STATE_PATH",
        "durable consumed-handshake journal; startup fails closed if it cannot be opened or parsed (default data/pulse-replay.tsv)",
    ),
    (
        "ARCHIPELAGO_CONTROL_V4_AUDIENCE",
        "shared audience for configured legacy ownership and its NATS control-position bridge; must match PULSE_V4_AUDIENCE when both are set (default unset)",
    ),
    (
        "PULSE_V4_AUDIENCE",
        "compatibility audience input for the internal control-position bridge; does not enable public V4 authentication (default unset)",
    ),
    (
        "NODE_POSTURE",
        "island or journey holds PULSE_ROOM_AUTHORITY_URL, PULSE_NATS_URL and NATS_URL to loopback, \
         .localhost and .invalid names and PUBLIC_URL's host (default unset: no check)",
    ),
    (
        "PUBLIC_URL",
        "this node's own address, the one outside host island and journey let those URLs name (default unset)",
    ),
    ("RUST_LOG", "tracing filter (default catalyst=info)"),
];

/// The URLs `NODE_POSTURE` holds to loopback and this node's own host.
const POSTURE_CHECKED: [&str; 5] = [
    "PUBLIC_URL",
    "PULSE_ROOM_AUTHORITY_URL",
    "PULSE_BANS_URL",
    "PULSE_NATS_URL",
    "NATS_URL",
];

/// Under `NODE_POSTURE=island` or `journey` the room authority and the broker
/// are loopback or this node's own, as serve's own URLs are
/// (`catalyst_config::posture::check_values`).
fn refuse_outside_hosts(env: &catalyst_config::posture::Lookup) -> anyhow::Result<()> {
    let Some(posture) = catalyst_config::posture::from_env(env)? else {
        return Ok(());
    };
    catalyst_config::posture::check_values(
        posture,
        POSTURE_CHECKED
            .into_iter()
            .filter_map(|key| env(key).map(|value| (key, value))),
    )
}

/// Reject historical requests for an authentication mode this process cannot serve.
/// These values validate the shipped posture; they never configure server capabilities.
fn validate_auth_requests(
    legacy: Result<String, VarError>,
    v4: Result<String, VarError>,
) -> anyhow::Result<()> {
    if !parse_or(
        "PULSE_LEGACY_HANDSHAKE",
        env_value("PULSE_LEGACY_HANDSHAKE", legacy)?,
        true,
    )? {
        anyhow::bail!(
            "PULSE_LEGACY_HANDSHAKE=false is unsupported; Pulse supports only legacy public authentication"
        );
    }
    if parse_or(
        "PULSE_V4_ENABLED",
        env_value("PULSE_V4_ENABLED", v4)?,
        false,
    )? {
        anyhow::bail!(
            "PULSE_V4_ENABLED=true is unavailable; Pulse does not provide public authentication via V4"
        );
    }
    Ok(())
}

/// `catalyst pulse`: tracing, the Prometheus exporter, and the server assembled from the
/// environment; returns when the loop ends.
pub async fn run_from_env() -> anyhow::Result<()> {
    validate_auth_requests(
        std::env::var("PULSE_LEGACY_HANDSHAKE"),
        std::env::var("PULSE_V4_ENABLED"),
    )?;
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(DEFAULT_LOG_FILTER));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();
    refuse_outside_hosts(&|key| std::env::var(key).ok())?;

    let ban_poller = super::bans::BanPoller::from_lookup(&|key| std::env::var(key).ok())?;
    let metrics_bind = super::metrics::metrics_bind_from_env()?;
    crate::metrics::install_prometheus_exporter(metrics_bind)?;
    super::metrics::register_resync_seq_gap_series();
    super::metrics::nats_connected(false);
    tracing::info!(%metrics_bind, "pulse /metrics listening");
    let bind = std::env::var("PULSE_BIND")
        .unwrap_or_else(|_| DEFAULT_BIND.to_string())
        .parse()?;
    let wt = webtransport_config_from_env()?;
    let aoi = aoi_options_from_env()?;
    tracing::info!(
        tier0 = aoi.tier0_radius,
        tier1 = aoi.tier1_radius,
        max = aoi.max_radius,
        "player AoI radii"
    );
    let mut server = PulseServer::new();
    server.ban_snapshots = Some(ban_poller.spawn());
    let replay_state_path = std::env::var("PULSE_REPLAY_STATE_PATH")
        .unwrap_or_else(|_| DEFAULT_REPLAY_STATE_PATH.to_string());
    server.replay_policy = HandshakeReplayPolicy::durable(
        true,
        MAX_TIMESTAMP_SKEW_MS,
        ENET_CAPACITY + WT_CAPACITY,
        &replay_state_path,
        chrono::Utc::now().timestamp_millis(),
    )
    .map_err(|_| anyhow::anyhow!("cannot open the Pulse replay journal"))?;
    tracing::info!("durable Pulse handshake replay protection enabled");
    server.gameplay_limiter = GameplayRateLimiter::new(
        env_or("PULSE_INPUT_MAX_HZ", DEFAULT_INPUT_MAX_HZ)?,
        env_or("PULSE_INPUT_BURST", DEFAULT_INPUT_BURST)?,
        env_or("PULSE_DISCRETE_RATE_PER_SEC", DEFAULT_DISCRETE_RATE_PER_SEC)?,
        env_or("PULSE_DISCRETE_BURST", DEFAULT_DISCRETE_BURST)?,
    );
    server.aoi = SpatialAreaOfInterest::new(aoi);
    let fallback = limiter_options_from_lookup(&|key| std::env::var(key).ok())?;
    server.apply_ip_limiter_options(fallback.clone());
    if let Some(url) = env_value("PULSE_FLAGS_URL", std::env::var("PULSE_FLAGS_URL"))? {
        let (initial, receiver) = super::flags::FlagsPoller::new(&url, fallback)?
            .start()
            .await;
        server.apply_ip_limiter_options(initial);
        server.flag_snapshots = Some(receiver);
    }
    let control_audience = control_audience_from(
        env_value(
            "ARCHIPELAGO_CONTROL_V4_AUDIENCE",
            std::env::var("ARCHIPELAGO_CONTROL_V4_AUDIENCE"),
        )?,
        env_value("PULSE_V4_AUDIENCE", std::env::var("PULSE_V4_AUDIENCE"))?,
    )?;
    server.clusters = clusters_from_env(control_audience.as_deref())?;
    server.run_with_webtransport(bind, 50, wt).await
}

fn limiter_options_from_lookup(
    env: &dyn Fn(&str) -> Option<String>,
) -> anyhow::Result<super::ip_limiter::IpLimiterOptions> {
    let cap = |key: &str, default: usize| -> anyhow::Result<usize> {
        let Some(raw) = env(key).filter(|raw| !raw.trim().is_empty()) else {
            return Ok(default);
        };
        raw.trim()
            .parse::<i32>()
            .ok()
            .filter(|n| *n >= 0)
            .map(|n| n as usize)
            .ok_or_else(|| anyhow::anyhow!("{key} must be an integer from 0 to 2147483647"))
    };
    Ok(super::ip_limiter::IpLimiterOptions {
        enabled: parse_bool_or(
            "PULSE_IP_LIMITER_ENABLED",
            env("PULSE_IP_LIMITER_ENABLED"),
            false,
        )?,
        max_concurrency: cap("PULSE_IP_LIMITER_MAX_CONCURRENCY", 10)?,
        scene_listener_max_concurrency: cap("PULSE_IP_LIMITER_SCENE_LISTENER_MAX_CONCURRENCY", 2)?,
        whitelist: ip_whitelist_from(env("PULSE_IP_LIMITER_WHITELIST"))?,
    })
}

fn ip_whitelist_from(raw: Option<String>) -> anyhow::Result<IpWhitelist> {
    IpWhitelist::parse(raw.as_deref().unwrap_or_default())
        .map_err(|e| anyhow::anyhow!("PULSE_IP_LIMITER_WHITELIST {e}"))
}

fn control_audience_from(
    shared: Option<String>,
    compatibility: Option<String>,
) -> anyhow::Result<Option<String>> {
    let normalize = |raw: Option<String>| -> anyhow::Result<Option<String>> {
        let value = raw.unwrap_or_default().trim().to_owned();
        if value.len() > 256 {
            anyhow::bail!("control position audience must be at most 256 bytes");
        }
        Ok((!value.is_empty()).then_some(value))
    };
    let shared = normalize(shared)?;
    let compatibility = normalize(compatibility)?;
    if matches!((&shared, &compatibility), (Some(a), Some(b)) if a != b) {
        anyhow::bail!("ARCHIPELAGO_CONTROL_V4_AUDIENCE and PULSE_V4_AUDIENCE must match");
    }
    Ok(shared.or(compatibility))
}

/// Player AoI radii from the environment, upstream's defaults where unset. A radius that fails to
/// parse or an ordering that empties a tier is a startup error, because the fallback would change
/// what every player sees, silently.
fn aoi_options_from_env() -> anyhow::Result<SpatialAreaOfInterestOptions> {
    let defaults = SpatialAreaOfInterestOptions::default();
    let options = SpatialAreaOfInterestOptions {
        tier0_radius: env_or("PULSE_AOI_TIER0_RADIUS", defaults.tier0_radius)?,
        tier1_radius: env_or("PULSE_AOI_TIER1_RADIUS", defaults.tier1_radius)?,
        max_radius: env_or("PULSE_AOI_MAX_RADIUS", defaults.max_radius)?,
    };
    options.validate()?;
    Ok(options)
}

/// Build the WebTransport config from the environment, or `None` when disabled. WebTransport is
/// off unless `PULSE_WT_ENABLED` is truthy; when on, it needs a certificate + key (inline PEM or
/// a file path) -- a browser cannot reach a raw ENet/UDP socket, so this is the browser front door.
fn webtransport_config_from_env() -> anyhow::Result<Option<WtConfig>> {
    if !env_bool("PULSE_WT_ENABLED")? {
        return Ok(None);
    }

    let bind_addr = std::env::var("PULSE_WT_BIND")
        .unwrap_or_else(|_| DEFAULT_WT_BIND.to_string())
        .parse()
        .map_err(|e| anyhow::anyhow!("PULSE_WT_BIND: {e}"))?;

    let cert_pem = read_pem("PULSE_WT_CERT_PEM", "PULSE_WT_CERT_PATH")?
        .ok_or_else(|| anyhow::anyhow!("PULSE_WT_ENABLED but no PULSE_WT_CERT_PEM/PATH set"))?;
    let key_pem = read_pem("PULSE_WT_KEY_PEM", "PULSE_WT_KEY_PATH")?
        .ok_or_else(|| anyhow::anyhow!("PULSE_WT_ENABLED but no PULSE_WT_KEY_PEM/PATH set"))?;

    Ok(Some(WtConfig {
        bind_addr,
        cert_pem,
        key_pem,
        slot_base: ENET_CAPACITY as u32,
        slot_capacity: WT_CAPACITY,
        max_datagram_bytes: env_or("PULSE_WT_MAX_DATAGRAM_BYTES", DEFAULT_MAX_DATAGRAM_BYTES)?,
        max_message_bytes: env_or("PULSE_WT_MAX_MESSAGE_BYTES", DEFAULT_MAX_MESSAGE_BYTES)?,
        service_timeout_ms: DEFAULT_SERVICE_TIMEOUT_MS,
        server_full_reason: DisconnectReason::ServerFull.code(),
    }))
}

/// The cluster tracker, or `None` when clustering is off entirely -- which leaves every cluster
/// path out of the server loop rather than running a tracker that publishes nothing. Publishing
/// nothing is the separate, and far more common, stats-only mode: clustering on with no broker URL.
fn clusters_from_env(control_audience: Option<&str>) -> anyhow::Result<Option<ClusterTracker>> {
    if !env_bool_or("PULSE_CLUSTERS_ENABLED", DEFAULT_CLUSTERS_ENABLED)? {
        tracing::info!("peer clustering disabled");
        return Ok(None);
    }
    let pass_interval_ms = env_or("PULSE_CLUSTERS_PASS_INTERVAL_MS", DEFAULT_PASS_INTERVAL_MS)?;
    if pass_interval_ms == 0 {
        tracing::info!("peer clustering disabled: PULSE_CLUSTERS_PASS_INTERVAL_MS is not positive");
        return Ok(None);
    }
    let options = ClusterOptions {
        enabled: true,
        pass_interval_ms,
        dwell_passes: env_or("PULSE_CLUSTERS_DWELL_PASSES", DEFAULT_DWELL_PASSES)?,
        id_prefix: env_or("PULSE_CLUSTERS_ID_PREFIX", DEFAULT_ID_PREFIX.to_string())?,
        session_retention_passes: env_or(
            "PULSE_CLUSTERS_SESSION_RETENTION_PASSES",
            DEFAULT_SESSION_RETENTION_PASSES,
        )?,
    };
    tracing::info!(
        pass_interval_ms = options.pass_interval_ms,
        dwell_passes = options.dwell_passes,
        id_prefix = %options.id_prefix,
        "peer clustering enabled"
    );
    Ok(Some(ClusterTracker::new(
        options,
        ENET_CAPACITY + WT_CAPACITY,
        cluster_feed_from_env(control_audience)?,
    )))
}

/// The deployed build's commit, which is what a discovery consumer tells deployments apart by.
/// Unset or blank advertises `unknown`, which is upstream's fallback -- the crate version is
/// deliberately not used, because every build of one version shares it.
fn commit_hash_from_env() -> String {
    match std::env::var("COMMIT_HASH") {
        Ok(hash) if !hash.trim().is_empty() => hash.trim().to_string(),
        _ => DEFAULT_COMMIT_HASH.to_string(),
    }
}

/// The broker URL, `PULSE_NATS_URL` first and the shared `NATS_URL` after it, so a host that
/// already exports one broker for its services needs no Pulse-specific copy of it.
fn nats_url_from_env() -> anyhow::Result<Option<String>> {
    for key in ["PULSE_NATS_URL", "NATS_URL"] {
        let value = env_value(key, std::env::var(key))?;
        match value.as_deref().map(str::trim) {
            Some(url) if !url.is_empty() => return Ok(Some(url.to_string())),
            _ => continue,
        }
    }
    Ok(None)
}

fn cluster_feed_from_env(
    control_audience: Option<&str>,
) -> anyhow::Result<Arc<dyn ClusterFeedPublisher>> {
    let Some(url) = nats_url_from_env()? else {
        tracing::info!("cluster feed in stats-only mode: no broker URL set");
        return Ok(Arc::new(NoopClusterFeedPublisher));
    };
    let options = NatsFeedOptions {
        url,
        server_name: env_or("PULSE_NATS_SERVER_NAME", DEFAULT_SERVER_NAME.to_string())?,
        commit_hash: commit_hash_from_env(),
        discovery_interval_ms: env_or(
            "PULSE_NATS_DISCOVERY_INTERVAL_MS",
            DEFAULT_DISCOVERY_INTERVAL_MS,
        )?,
        capacity: env_or("PULSE_NATS_CHANNEL_CAPACITY", DEFAULT_CHANNEL_CAPACITY)?,
    };
    if options.capacity == 0 {
        tracing::warn!(
            "PULSE_NATS_CHANNEL_CAPACITY is not positive: each assignment evicts the previous one, \
             so almost everything is lost -- watch pulse_nats_dropped_total"
        );
    }
    if options.discovery_interval_ms == 0 {
        tracing::warn!(
            "PULSE_NATS_DISCOVERY_INTERVAL_MS is not positive: assignments and topology still \
             publish, the service is not advertised on engine.discovery"
        );
    }
    tracing::info!(server_name = %options.server_name, "cluster feed publishing");
    let feed = NatsClusterFeed::spawn(options);
    if let Some(audience) = control_audience {
        feed.enable_control_positions(audience)
            .map_err(anyhow::Error::msg)?;
        tracing::info!(%audience, "control position clustering enabled");
    }
    Ok(Arc::new(feed))
}

fn env_bool(key: &str) -> anyhow::Result<bool> {
    parse_bool(key, env_value(key, std::env::var(key))?)
}

fn env_bool_or(key: &str, default: bool) -> anyhow::Result<bool> {
    parse_bool_or(key, env_value(key, std::env::var(key))?, default)
}

/// Unset and blank mean off; the documented spellings match trimmed and case-insensitively;
/// anything else is a startup error, because reading a typo as "off" would close the browser
/// front door silently.
fn parse_bool(key: &str, raw: Option<String>) -> anyhow::Result<bool> {
    parse_bool_or(key, raw, false)
}

/// As `parse_bool`, for a setting whose unset state is on: unset and blank both take `default`,
/// which keeps a blanked-out env template reading as the shipped behaviour rather than silently
/// turning a feature off.
fn parse_bool_or(key: &str, raw: Option<String>, default: bool) -> anyhow::Result<bool> {
    let Some(raw) = raw else {
        return Ok(default);
    };
    let value = raw.trim().to_ascii_lowercase();
    if value.is_empty() {
        Ok(default)
    } else if BOOL_FALSE.contains(&value.as_str()) {
        Ok(false)
    } else if BOOL_TRUE.contains(&value.as_str()) {
        Ok(true)
    } else {
        anyhow::bail!(
            "{key} `{}`: expected one of {} or {}",
            raw.trim(),
            BOOL_TRUE.join("/"),
            BOOL_FALSE.join("/")
        )
    }
}

fn env_or<T>(key: &str, default: T) -> anyhow::Result<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    parse_or(key, env_value(key, std::env::var(key))?, default)
}

/// Unset is the default; a value that is not UTF-8 fails startup naming the key, because reading
/// it as unset would swap an operator's explicit setting for the default without a trace.
fn env_value(key: &str, raw: Result<String, VarError>) -> anyhow::Result<Option<String>> {
    match raw {
        Ok(value) => Ok(Some(value)),
        Err(VarError::NotPresent) => Ok(None),
        Err(VarError::NotUnicode(value)) => {
            anyhow::bail!("{key} `{}`: not valid UTF-8", value.to_string_lossy())
        }
    }
}

/// Unset and blank both mean the default; anything else must parse.
fn parse_or<T>(key: &str, raw: Option<String>, default: T) -> anyhow::Result<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match raw.as_deref().map(str::trim) {
        Some(value) if !value.is_empty() => value
            .parse()
            .map_err(|e| anyhow::anyhow!("{key} `{value}`: {e}")),
        _ => Ok(default),
    }
}

/// Read a PEM blob from an inline env var (takes precedence) or a file path.
fn read_pem(inline_key: &str, path_key: &str) -> anyhow::Result<Option<String>> {
    if let Ok(pem) = std::env::var(inline_key)
        && !pem.trim().is_empty()
    {
        return Ok(Some(pem));
    }
    if let Ok(path) = std::env::var(path_key)
        && !path.trim().is_empty()
    {
        return Ok(Some(
            std::fs::read_to_string(&path)
                .map_err(|e| anyhow::anyhow!("reading {path_key}={path}: {e}"))?,
        ));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn auth_raw(raw: Option<&str>) -> Result<String, VarError> {
        raw.map(str::to_owned).ok_or(VarError::NotPresent)
    }

    #[test]
    fn obsolete_auth_settings_accept_only_the_shipped_posture() {
        for legacy in [None, Some(""), Some(" \t "), Some("true"), Some(" true ")] {
            for v4 in [None, Some(""), Some(" \t "), Some("false"), Some(" false ")] {
                validate_auth_requests(auth_raw(legacy), auth_raw(v4)).unwrap();
            }
        }
        for (legacy, v4, key) in [
            (Some("false"), Some("false"), "PULSE_LEGACY_HANDSHAKE"),
            (Some("true"), Some("true"), "PULSE_V4_ENABLED"),
            (Some("false"), Some("true"), "PULSE_LEGACY_HANDSHAKE"),
            (Some(" false "), None, "PULSE_LEGACY_HANDSHAKE"),
            (None, Some(" true "), "PULSE_V4_ENABLED"),
        ] {
            let error = validate_auth_requests(auth_raw(legacy), auth_raw(v4))
                .unwrap_err()
                .to_string();
            assert!(error.starts_with(key), "{error}");
            assert!(error.contains("public authentication"), "{error}");
        }
    }

    #[test]
    fn obsolete_auth_settings_reuse_strict_typed_boolean_errors() {
        for key in ["PULSE_LEGACY_HANDSHAKE", "PULSE_V4_ENABLED"] {
            for bad in [
                "TRUE",
                "FALSE",
                "True",
                "False",
                "1",
                "0",
                "yes",
                "no",
                "on",
                "off",
                "maybe",
                "true false",
            ] {
                let value = format!(" {bad} ");
                let (legacy, v4) = if key == "PULSE_LEGACY_HANDSHAKE" {
                    (auth_raw(Some(&value)), auth_raw(None))
                } else {
                    (auth_raw(None), auth_raw(Some(&value)))
                };
                let error = validate_auth_requests(legacy, v4).unwrap_err().to_string();
                let expected = parse_or(key, Some(value), false).unwrap_err().to_string();
                assert_eq!(error, expected, "{key}={bad}");
                assert!(error.starts_with(&format!("{key} `{bad}`")), "{error}");
            }
        }
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn obsolete_auth_settings_reject_non_unicode_without_treating_it_as_unset() {
        #[cfg(unix)]
        let raw = {
            use std::os::unix::ffi::OsStringExt;
            std::ffi::OsString::from_vec(vec![0xff])
        };
        #[cfg(windows)]
        let raw = {
            use std::os::windows::ffi::OsStringExt;
            std::ffi::OsString::from_wide(&[0xd800])
        };
        for key in ["PULSE_LEGACY_HANDSHAKE", "PULSE_V4_ENABLED"] {
            let invalid = Err(VarError::NotUnicode(raw.clone()));
            let (legacy, v4) = if key == "PULSE_LEGACY_HANDSHAKE" {
                (invalid, auth_raw(None))
            } else {
                (auth_raw(None), invalid)
            };
            let error = validate_auth_requests(legacy, v4).unwrap_err().to_string();
            assert!(error.starts_with(key), "{error}");
            assert!(error.ends_with("not valid UTF-8"), "{error}");
        }
    }

    #[test]
    fn obsolete_auth_sentinels_are_documented_as_validation_not_switches() {
        for (key, allowed, rejected) in [
            ("PULSE_LEGACY_HANDSHAKE", "true", "false"),
            ("PULSE_V4_ENABLED", "false", "true"),
        ] {
            let doc = doc_for(key);
            for text in [
                "validation only",
                "strict lowercase true/false",
                "absent or blank",
                &format!("accepts {allowed}"),
                &format!("rejects {rejected}"),
            ] {
                assert!(doc.contains(text), "{key}: missing {text:?}: {doc}");
            }
        }
    }

    #[test]
    fn obsolete_auth_requests_fail_before_startup_side_effects() {
        const TEST: &str =
            "pulse::env::tests::obsolete_auth_requests_fail_before_startup_side_effects";
        const CHILD: &str = "CATALYST_TEST_PULSE_AUTH_STARTUP_ERROR";
        if let Ok(expected) = std::env::var(CHILD) {
            // Isolate configuration from the existing Windows debug startup-stack limit.
            // No production stack/resource setting changes are made by this fixture.
            let error = std::thread::Builder::new()
                .stack_size(16 * 1024 * 1024)
                .spawn(|| {
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap()
                        .block_on(run_from_env())
                        .unwrap_err()
                        .to_string()
                })
                .unwrap()
                .join()
                .unwrap();
            assert!(
                error.starts_with(&expected),
                "expected {expected:?}, got {error:?}"
            );
            return;
        }
        let run =
            |legacy: Option<&std::ffi::OsStr>, v4: Option<&std::ffi::OsStr>, expected: &str| {
                let mut command = std::process::Command::new(std::env::current_exe().unwrap());
                command
                    .env_clear()
                    .args(["--exact", TEST, "--nocapture"])
                    .env(CHILD, expected)
                    .env("PULSE_BIND", "invalid native bind")
                    .env("PULSE_METRICS_BIND", "invalid metrics bind")
                    .env("PULSE_FLAGS_URL", "invalid remote URL")
                    .env("PULSE_WT_ENABLED", "true")
                    .env("PULSE_WT_CERT_PATH", "missing-test-certificate")
                    .env("PULSE_REPLAY_STATE_PATH", "must-not-create-replay-journal")
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped());
                if let Some(raw) = legacy {
                    command.env("PULSE_LEGACY_HANDSHAKE", raw);
                }
                if let Some(raw) = v4 {
                    command.env("PULSE_V4_ENABLED", raw);
                }
                let mut child = command.spawn().unwrap();
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
                while child.try_wait().unwrap().is_none() {
                    if std::time::Instant::now() >= deadline {
                        child.kill().unwrap();
                        child.wait().unwrap();
                        panic!("Pulse auth startup child exceeded 10s: {legacy:?}/{v4:?}");
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                let output = child.wait_with_output().unwrap();
                assert!(
                    output.status.success(),
                    "{legacy:?}/{v4:?}: {}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            };
        let next = "COMMS_MODERATOR_TOKEN is required for Pulse bans";
        for (legacy, v4, expected) in [
            (None, None, next),
            (Some("true"), Some("false"), next),
            (Some("  "), Some("\t"), next),
            (Some(" true "), Some(" false "), next),
            (
                Some("false"),
                None,
                "PULSE_LEGACY_HANDSHAKE=false is unsupported",
            ),
            (None, Some("true"), "PULSE_V4_ENABLED=true is unavailable"),
            (Some("TRUE"), None, "PULSE_LEGACY_HANDSHAKE `TRUE`"),
            (None, Some("1"), "PULSE_V4_ENABLED `1`"),
        ] {
            run(
                legacy.map(std::ffi::OsStr::new),
                v4.map(std::ffi::OsStr::new),
                expected,
            );
        }
        #[cfg(any(unix, windows))]
        {
            #[cfg(unix)]
            let raw = {
                use std::os::unix::ffi::OsStringExt;
                std::ffi::OsString::from_vec(vec![0xff])
            };
            #[cfg(windows)]
            let raw = {
                use std::os::windows::ffi::OsStringExt;
                std::ffi::OsString::from_wide(&[0xd800])
            };
            run(Some(&raw), None, "PULSE_LEGACY_HANDSHAKE `");
            run(None, Some(&raw), "PULSE_V4_ENABLED `");
        }
    }

    #[test]
    fn control_position_audience_is_independent_of_public_auth_and_conflicts_fail_closed() {
        assert_eq!(control_audience_from(None, None).unwrap(), None);
        assert_eq!(
            control_audience_from(Some("  ".into()), None).unwrap(),
            None
        );
        for (shared, compatibility) in [
            (Some("realm".into()), None),
            (None, Some("realm".into())),
            (Some(" realm ".into()), Some("realm".into())),
        ] {
            assert_eq!(
                control_audience_from(shared, compatibility)
                    .unwrap()
                    .as_deref(),
                Some("realm")
            );
        }
        assert!(control_audience_from(Some("one".into()), Some("two".into())).is_err());
        assert!(control_audience_from(Some("a".repeat(257)), None).is_err());
        assert!(control_audience_from(None, Some("a".repeat(257))).is_err());
    }
    use crate::pulse::hardening::{
        DEFAULT_SCENE_LISTENER_MAX_PARCELS, SCENE_LISTENER_REALM_BUDGET_COST,
    };
    use crate::pulse::interest::{
        AOI_MAX_RADIUS_CEILING, DEFAULT_AOI_MAX_RADIUS, DEFAULT_AOI_TIER0_RADIUS,
        DEFAULT_AOI_TIER1_RADIUS,
    };
    use crate::pulse::metrics::DEFAULT_METRICS_BIND;

    fn doc_for(key: &str) -> &'static str {
        ENV_DOCS
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, doc)| *doc)
            .unwrap_or_else(|| panic!("{key} is read by this module but missing from ENV_DOCS"))
    }

    #[test]
    fn env_docs_defaults_track_the_constants() {
        let pinned = [
            ("PULSE_BIND", DEFAULT_BIND.to_string()),
            ("PULSE_METRICS_BIND", DEFAULT_METRICS_BIND.to_string()),
            ("PULSE_INPUT_MAX_HZ", DEFAULT_INPUT_MAX_HZ.to_string()),
            ("PULSE_INPUT_BURST", DEFAULT_INPUT_BURST.to_string()),
            (
                "PULSE_DISCRETE_RATE_PER_SEC",
                DEFAULT_DISCRETE_RATE_PER_SEC.to_string(),
            ),
            ("PULSE_DISCRETE_BURST", DEFAULT_DISCRETE_BURST.to_string()),
            (
                "PULSE_SCENE_LISTENER_MAX_PARCELS",
                DEFAULT_SCENE_LISTENER_MAX_PARCELS.to_string(),
            ),
            (
                "PULSE_AOI_TIER0_RADIUS",
                DEFAULT_AOI_TIER0_RADIUS.to_string(),
            ),
            (
                "PULSE_AOI_TIER1_RADIUS",
                DEFAULT_AOI_TIER1_RADIUS.to_string(),
            ),
            ("PULSE_AOI_MAX_RADIUS", DEFAULT_AOI_MAX_RADIUS.to_string()),
            ("PULSE_WT_BIND", DEFAULT_WT_BIND.to_string()),
            (
                "PULSE_CLUSTERS_ENABLED",
                DEFAULT_CLUSTERS_ENABLED.to_string(),
            ),
            (
                "PULSE_CLUSTERS_PASS_INTERVAL_MS",
                DEFAULT_PASS_INTERVAL_MS.to_string(),
            ),
            (
                "PULSE_CLUSTERS_DWELL_PASSES",
                DEFAULT_DWELL_PASSES.to_string(),
            ),
            ("PULSE_CLUSTERS_ID_PREFIX", DEFAULT_ID_PREFIX.to_string()),
            (
                "PULSE_CLUSTERS_SESSION_RETENTION_PASSES",
                DEFAULT_SESSION_RETENTION_PASSES.to_string(),
            ),
            ("PULSE_NATS_URL", "unset".to_string()),
            ("NATS_URL", "unset".to_string()),
            (
                "PULSE_WT_MAX_DATAGRAM_BYTES",
                DEFAULT_MAX_DATAGRAM_BYTES.to_string(),
            ),
            ("RUST_LOG", DEFAULT_LOG_FILTER.to_string()),
        ];
        for (key, default) in &pinned {
            let doc = doc_for(key);
            let suffix = format!("(default {default})");
            assert!(
                doc.ends_with(&suffix),
                "{key}: `{doc}` must end with `{suffix}`"
            );
        }
        assert!(
            doc_for("PULSE_WT_MAX_MESSAGE_BYTES")
                .ends_with(&format!("(default {DEFAULT_MAX_MESSAGE_BYTES})"))
        );
        for (key, default) in [
            ("COMMIT_HASH", DEFAULT_COMMIT_HASH.to_string()),
            ("PULSE_NATS_SERVER_NAME", DEFAULT_SERVER_NAME.to_string()),
            (
                "PULSE_NATS_DISCOVERY_INTERVAL_MS",
                DEFAULT_DISCOVERY_INTERVAL_MS.to_string(),
            ),
            (
                "PULSE_NATS_CHANNEL_CAPACITY",
                DEFAULT_CHANNEL_CAPACITY.to_string(),
            ),
        ] {
            let doc = doc_for(key);
            let suffix = format!("(default {default})");
            assert!(
                doc.ends_with(&suffix),
                "{key}: `{doc}` must end with `{suffix}`"
            );
        }
        assert!(
            doc_for("PULSE_SCENE_LISTENER_MAX_PARCELS").contains(&format!(
                "plus {SCENE_LISTENER_REALM_BUDGET_COST} per realm"
            )),
            "the per-realm budget cost is documented next to the parcel budget"
        );
        assert!(
            doc_for("PULSE_AOI_MAX_RADIUS").contains(&format!("at most {AOI_MAX_RADIUS_CEILING}")),
            "the radius ceiling is documented next to the default"
        );
        let wt = doc_for("PULSE_WT_ENABLED");
        assert!(
            wt.contains(&BOOL_TRUE.join("/")) && wt.contains(&BOOL_FALSE.join("/")),
            "the accepted boolean spellings are documented: `{wt}`"
        );
    }

    #[test]
    fn ip_whitelist_is_unset_by_default_and_a_typo_fails_startup() {
        assert!(ip_whitelist_from(None).unwrap().is_empty());
        assert!(ip_whitelist_from(Some("  ".into())).unwrap().is_empty());
        let list = ip_whitelist_from(Some("10.0.0.7, ::1".into())).unwrap();
        assert!(list.contains("::ffff:10.0.0.7") && list.contains("::1"));
        let err = ip_whitelist_from(Some("10.0.0.7,10.0.0.0/8".into())).unwrap_err();
        assert!(
            err.to_string()
                .starts_with("PULSE_IP_LIMITER_WHITELIST `10.0.0.0/8`: "),
            "{err}"
        );
        let doc = doc_for("PULSE_IP_LIMITER_WHITELIST");
        assert!(doc.ends_with("(default unset)"), "{doc}");
        assert!(doc.contains("PULSE_SCENE_LISTENER_MAX_PARCELS"), "{doc}");
    }

    #[test]
    fn local_limiter_settings_keep_baseline_defaults_and_validate_caps() {
        let defaults = limiter_options_from_lookup(&|_| None).unwrap();
        assert!(!defaults.enabled);
        assert_eq!(defaults.max_concurrency, 10);
        assert_eq!(defaults.scene_listener_max_concurrency, 2);
        assert!(defaults.whitelist.is_empty());
        let configured = limiter_options_from_lookup(&|key| match key {
            "PULSE_IP_LIMITER_ENABLED" => Some("yes".into()),
            "PULSE_IP_LIMITER_MAX_CONCURRENCY" => Some("0".into()),
            "PULSE_IP_LIMITER_SCENE_LISTENER_MAX_CONCURRENCY" => Some("3".into()),
            "PULSE_IP_LIMITER_WHITELIST" => Some("::ffff:192.0.2.1".into()),
            _ => None,
        })
        .unwrap();
        assert!(configured.enabled);
        assert_eq!(configured.max_concurrency, 0);
        assert_eq!(configured.scene_listener_max_concurrency, 3);
        assert!(configured.whitelist.contains("192.0.2.1"));
        for bad in ["-1", "ten", "1.5", "2147483648"] {
            assert!(
                limiter_options_from_lookup(
                    &|key| (key == "PULSE_IP_LIMITER_MAX_CONCURRENCY").then(|| bad.into())
                )
                .is_err()
            );
        }
    }

    fn lookup(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
        let map: std::collections::HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |key| map.get(key).cloned()
    }

    #[test]
    fn island_and_journey_hold_the_room_authority_and_broker_to_loopback() {
        for posture in ["island", "journey"] {
            for (key, value) in [
                (
                    "PULSE_ROOM_AUTHORITY_URL",
                    "https://authority.example.org/internal/pulse/room-authority/v1",
                ),
                ("PULSE_NATS_URL", "nats://broker.example.org:4222"),
                ("NATS_URL", "broker.example.org:4222"),
            ] {
                let env = lookup(&[("NODE_POSTURE", posture), (key, value)]);
                let err = refuse_outside_hosts(&env).unwrap_err().to_string();
                assert!(err.contains(&format!("{key} names ")), "{err}");
            }
            let env = lookup(&[
                ("NODE_POSTURE", posture),
                ("PUBLIC_URL", "https://node.example.org"),
                (
                    "PULSE_ROOM_AUTHORITY_URL",
                    "https://node.example.org/internal/pulse/room-authority/v1",
                ),
                ("PULSE_NATS_URL", "nats://127.0.0.1:4222"),
            ]);
            refuse_outside_hosts(&env).unwrap();
        }
        let outside = ("PULSE_NATS_URL", "nats://broker.example.org:4222");
        refuse_outside_hosts(&lookup(&[("NODE_POSTURE", "mirror"), outside])).unwrap();
        refuse_outside_hosts(&lookup(&[outside])).unwrap();
    }

    #[test]
    fn env_docs_keys_are_unique() {
        let mut keys: Vec<&str> = ENV_DOCS.iter().map(|(k, _)| *k).collect();
        keys.sort_unstable();
        let before = keys.len();
        keys.dedup();
        assert_eq!(before, keys.len(), "duplicate ENV_DOCS key");
    }

    #[test]
    fn parse_bool_is_strict_case_insensitive_and_off_when_unset() {
        let key = "PULSE_WT_ENABLED";
        assert!(!parse_bool(key, None).unwrap());
        assert!(!parse_bool(key, Some("   ".into())).unwrap());
        for on in ["1", "true", "TRUE", " Yes ", "On", "yES"] {
            assert!(parse_bool(key, Some(on.into())).unwrap(), "{on:?}");
        }
        for off in ["0", "false", "FALSE", " No ", "Off", "fAlSe"] {
            assert!(!parse_bool(key, Some(off.into())).unwrap(), "{off:?}");
        }
        for bad in ["maybe", "2", "enabled", "t", "y", "-1", "true false"] {
            let err = parse_bool(key, Some(format!(" {bad} "))).unwrap_err();
            assert_eq!(
                err.to_string(),
                format!(
                    "PULSE_WT_ENABLED `{bad}`: expected one of 1/true/yes/on or 0/false/no/off"
                ),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn parse_or_defaults_on_unset_or_blank_and_rejects_garbage() {
        assert_eq!(parse_or("K", None, 30.0f32).unwrap(), 30.0);
        assert_eq!(parse_or("K", Some("  ".into()), 30.0f32).unwrap(), 30.0);
        assert_eq!(parse_or("K", Some(" 75.5 ".into()), 30.0f32).unwrap(), 75.5);
        assert_eq!(parse_or("K", Some("7".into()), 1u32).unwrap(), 7);
        let err = parse_or("PULSE_AOI_MAX_RADIUS", Some("far".into()), 200.0f32).unwrap_err();
        assert!(
            err.to_string().starts_with("PULSE_AOI_MAX_RADIUS `far`"),
            "{err}"
        );
        for garbage in ["lots", "-1", "16.5", "0x10"] {
            let err = parse_or("PULSE_INPUT_BURST", Some(garbage.into()), 16u32).unwrap_err();
            assert!(
                err.to_string()
                    .starts_with(&format!("PULSE_INPUT_BURST `{garbage}`")),
                "{err}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn env_value_distinguishes_unset_from_not_unicode() {
        use std::os::unix::ffi::OsStringExt;
        assert_eq!(
            env_value("K", Ok("7".into())).unwrap(),
            Some("7".to_string())
        );
        assert_eq!(env_value("K", Err(VarError::NotPresent)).unwrap(), None);
        let raw = std::ffi::OsString::from_vec(vec![b'o', b'n', 0xff]);
        let err = env_value("PULSE_WT_ENABLED", Err(VarError::NotUnicode(raw))).unwrap_err();
        assert_eq!(
            err.to_string(),
            "PULSE_WT_ENABLED `on\u{fffd}`: not valid UTF-8"
        );
    }
}
