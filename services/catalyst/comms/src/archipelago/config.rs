//! The ws-connector's settings. The old executable read a TOML file plus the
//! environment; v2 keeps the environment names that mapped 1:1 (`Settings::
//! from_env`) and takes the content URL and the pool from the process it runs
//! in (the content settings, the ONE pool). LiveKit is the gatekeeper's: it
//! removes a kicked wallet's participant (`gatekeeper::admission::island_kick`).

use std::env;

/// Connected-wallet moderation cadence; statistics have no peer expiry.
#[derive(Clone, Debug)]
pub struct ClusterConfig {
    pub ban_sweep_interval_secs: u64,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            ban_sweep_interval_secs: 30,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub max_sockets: u32,
    /// `ISLAND_CHANGED_DEDUP_MS`: the window inside which the same room handed to the same socket
    /// twice is the client's own assignment arriving again through the re-announce path; it
    /// already holds a token for it.
    pub island_changed_dedup_ms: f64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            max_sockets: 3000,
            island_changed_dedup_ms: 10_000.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AuthConfig {
    pub challenge_ttl_secs: u64,
    pub signature_max_age_secs: u64,
    /// A socket that opens and never speaks holds a connection and a challenge slot for nothing.
    /// Restarted after every stage, so a slow signer is not punished for the wallet's own latency.
    pub handshake_timeout_ms: u64,
    pub deny_list_url: Option<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            challenge_ttl_secs: 120,
            signature_max_age_secs: 300,
            handshake_timeout_ms: 60_000,
            deny_list_url: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub cluster: ClusterConfig,
    pub server: ServerConfig,
    pub auth: AuthConfig,
    pub commit_hash: String,
}

/// The settings as `Config` carries them, under the old names;
/// [`Settings::resolve`] checks them.
#[derive(Clone, Debug)]
pub struct RawSettings {
    /// WS_MAX_SOCKETS; zero is not an unlimited mode.
    pub max_sockets: u32,
    /// `HANDSHAKE_TIMEOUT` in ms; 0 keeps the default.
    pub handshake_timeout_ms: u64,
    /// `DENY_LIST_URL`.
    pub deny_list_url: Option<String>,
    /// `ISLAND_CHANGED_DEDUP_MS`.
    pub island_changed_dedup_ms: f64,
    /// `COMMIT_HASH`.
    pub commit_hash: Option<String>,
}

impl Settings {
    /// The checked settings from `Config`'s values.
    pub fn resolve(raw: RawSettings) -> Self {
        let mut auth = AuthConfig::default();
        if raw.handshake_timeout_ms > 0 {
            auth.handshake_timeout_ms = raw.handshake_timeout_ms;
        }
        auth.deny_list_url = raw
            .deny_list_url
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
        let server = ServerConfig {
            max_sockets: raw.max_sockets.max(1),
            island_changed_dedup_ms: raw.island_changed_dedup_ms,
        };
        Self {
            cluster: ClusterConfig::default(),
            server,
            auth,
            commit_hash: raw.commit_hash.unwrap_or_default(),
        }
    }

    /// Settings for tests: sockets still require signed challenges; no deny list.
    pub fn for_tests() -> Self {
        Self {
            cluster: ClusterConfig::default(),
            server: ServerConfig::default(),
            auth: AuthConfig::default(),
            commit_hash: "deadbeef".into(),
        }
    }

    /// The connector environment names: `HANDSHAKE_TIMEOUT`,
    /// `DENY_LIST_URL`, `ISLAND_CHANGED_DEDUP_MS` and
    /// `COMMIT_HASH`; LiveKit and the broker (`NATS_URL`) are the gatekeeper's.
    pub fn from_env() -> anyhow::Result<Self> {
        let mut auth = AuthConfig::default();
        if let Ok(v) = env::var("HANDSHAKE_TIMEOUT") {
            match v.trim().parse::<u64>() {
                Ok(0) => {}
                Ok(ms) => auth.handshake_timeout_ms = ms,
                Err(_) => {
                    return Err(anyhow::anyhow!(
                        "HANDSHAKE_TIMEOUT must be a whole number of milliseconds, got {v:?}"
                    ));
                }
            }
        }
        auth.deny_list_url = optional_endpoint("DENY_LIST_URL");
        let mut server = ServerConfig::default();
        if let Ok(v) = env::var("WS_MAX_SOCKETS") {
            server.max_sockets = v
                .trim()
                .parse::<u32>()
                .ok()
                .filter(|n| *n > 0)
                .ok_or_else(|| anyhow::anyhow!("WS_MAX_SOCKETS must be a positive integer"))?;
        }
        if let Ok(v) = env::var("ISLAND_CHANGED_DEDUP_MS") {
            server.island_changed_dedup_ms =
                catalyst_config::parse_island_changed_dedup_ms(&v).map_err(anyhow::Error::msg)?;
        }
        Ok(Self {
            cluster: ClusterConfig::default(),
            server,
            auth,
            commit_hash: env::var("COMMIT_HASH").unwrap_or_default(),
        })
    }
}

fn optional_endpoint(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}
