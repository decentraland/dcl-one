//! The ws-connector's settings. The old executable read a TOML file plus the
//! environment; v2 keeps the environment names that mapped 1:1 (`Settings::
//! from_env`) and takes the content URL and the pool from the process it runs
//! in (the content settings, the ONE pool). LiveKit is the gatekeeper's: it
//! removes a kicked wallet's participant (`gatekeeper::admission::island_kick`).

use std::env;

/// Connected-wallet moderation cadence; statistics have no peer expiry.
#[derive(Clone, Debug)]
pub struct ClusterConfig {
    pub ban_sweep_interval_ms: u64,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            ban_sweep_interval_ms: 30_000,
        }
    }
}

pub const DEFAULT_MAX_SOCKETS_PER_IP: u32 = 64;

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub max_sockets: u32,
    /// `WS_MAX_SOCKETS_PER_IP`: sockets one client address may hold, pending handshakes
    /// included; 0 is unlimited.
    pub max_sockets_per_ip: u32,
    /// `HTTP_TRUSTED_PROXY_CIDRS`: proxies whose `X-Real-IP` names the client.
    pub trusted_proxy_cidrs: Vec<ipnet::IpNet>,
    /// `ISLAND_CHANGED_DEDUP_MS`: the window inside which the same room handed to the same socket
    /// twice is the client's own assignment arriving again through the re-announce path; it
    /// already holds a token for it.
    pub island_changed_dedup_ms: f64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            max_sockets: 3000,
            max_sockets_per_ip: DEFAULT_MAX_SOCKETS_PER_IP,
            trusted_proxy_cidrs: Vec::new(),
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
    /// `BAN_SWEEP_INTERVAL_MS`, normalized to a positive millisecond timer delay by `Config`.
    pub ban_sweep_interval_ms: u64,
    /// WS_MAX_SOCKETS; zero is not an unlimited mode.
    pub max_sockets: u32,
    /// `WS_MAX_SOCKETS_PER_IP`; zero is unlimited.
    pub max_sockets_per_ip: u32,
    /// `HTTP_TRUSTED_PROXY_CIDRS`.
    pub trusted_proxy_cidrs: Vec<ipnet::IpNet>,
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
            max_sockets_per_ip: raw.max_sockets_per_ip,
            trusted_proxy_cidrs: raw.trusted_proxy_cidrs,
            island_changed_dedup_ms: raw.island_changed_dedup_ms,
        };
        Self {
            cluster: ClusterConfig {
                ban_sweep_interval_ms: raw.ban_sweep_interval_ms,
            },
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
    /// `DENY_LIST_URL`, `BAN_SWEEP_INTERVAL_MS`, `ISLAND_CHANGED_DEDUP_MS` and
    /// `COMMIT_HASH`; LiveKit and the broker (`NATS_URL`) are the gatekeeper's.
    pub fn from_env() -> anyhow::Result<Self> {
        let mut cluster = ClusterConfig::default();
        if let Ok(value) = env::var("BAN_SWEEP_INTERVAL_MS") {
            cluster.ban_sweep_interval_ms =
                catalyst_config::parse_ban_sweep_interval_ms(&value).map_err(anyhow::Error::msg)?;
        }
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
            cluster,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ban_sweep_resolve_keeps_milliseconds_from_process_config() {
        let process = catalyst_config::config::Config::parse_env_for_tests([
            ("DATABASE_URL", "postgres://localhost/test"),
            ("BAN_SWEEP_INTERVAL_MS", "1250ms"),
        ])
        .unwrap();
        let settings = Settings::resolve(RawSettings {
            ban_sweep_interval_ms: process.worlds.ban_sweep_interval_ms,
            max_sockets: process.worlds.ws_max_sockets,
            max_sockets_per_ip: process.worlds.ws_max_sockets_per_ip,
            trusted_proxy_cidrs: process.node.http_trusted_proxy_cidrs,
            handshake_timeout_ms: process.worlds.handshake_timeout,
            deny_list_url: process.worlds.deny_list(),
            island_changed_dedup_ms: process.worlds.island_changed_dedup_ms,
            commit_hash: process.node.commit_hash,
        });
        assert_eq!(settings.cluster.ban_sweep_interval_ms, 1250);
    }

    #[test]
    fn ban_sweep_environment_parser_keeps_defaults_delays_and_errors() {
        const CHILD: &str = "CATALYST_TEST_BAN_SWEEP_ENV";
        if let Ok(expected) = env::var(CHILD) {
            let result = Settings::from_env();
            if expected == "error" {
                let error = result.unwrap_err().to_string();
                assert!(error.contains("BAN_SWEEP_INTERVAL_MS"), "{error}");
            } else {
                assert_eq!(
                    result.unwrap().cluster.ban_sweep_interval_ms,
                    expected.parse::<u64>().unwrap()
                );
            }
            return;
        }
        for (value, expected) in [
            (None, "30000"),
            (Some("1250ms"), "1250"),
            (Some("12.9"), "12"),
            (Some("-1"), "1"),
            (Some("Infinity"), "1"),
            (Some(""), "error"),
            (Some("NaN"), "error"),
        ] {
            let mut child = std::process::Command::new(std::env::current_exe().unwrap());
            child.args([
                "--exact", "archipelago::config::tests::ban_sweep_environment_parser_keeps_defaults_delays_and_errors",
                "--nocapture",
            ]).env_clear().env(CHILD, expected);
            if let Some(value) = value {
                child.env("BAN_SWEEP_INTERVAL_MS", value);
            }
            let result = child.output().unwrap();
            assert!(
                result.status.success(),
                "{value:?}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(String::from_utf8_lossy(&result.stdout).contains("1 passed"));
        }
    }
}
