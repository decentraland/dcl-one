//! The five `WORLDS_FED_*` keys, parsed once at boot rather than at the point of use,
//! which is a background poller: a bad value discovered there degrades a running process
//! instead of refusing to start one. Zero is rejected explicitly -- a zero poll interval
//! is a busy loop against a peer, and a zero byte or row cap is a mirror that silently
//! stores nothing.
//!
//! `own_urls` is new in v2: this node's own URLs (`PUBLIC_URL`, `CONTENT_URL`, the bind
//! address, the same list `serve` hands the other mirrors), so a peer file naming this
//! node is refused as a mirror loop at admission.

use std::path::PathBuf;

use anyhow::{Result, anyhow};

/// 5 minutes. Slow enough that a peer never sees us as load, fast enough that a
/// world published on a peer shows up within one coffee.
pub const DEFAULT_POLL_INTERVAL_SECS: u64 = 300;
/// 4 MiB, ~2x head-room over the largest listing known to exist (the production corpus
/// `worlds-content-server/index`, 1552 worlds, is ~2.1 MB). Enforced by a streaming
/// byte counter *before* parse, never after.
pub const DEFAULT_MAX_RESPONSE_BYTES: u64 = 4 * 1024 * 1024;
/// 10000 rows per peer. Same corpus is 1552.
pub const DEFAULT_MAX_WORLDS_PER_PEER: u64 = 10_000;

/// `peers_file: None` is the *only* representation of "federation was never requested",
/// never conflated with "the file loaded and yielded nothing" -- that distinction is
/// carried by [`super::peers::WorldsFederationPeers`].
#[derive(Debug, Clone)]
pub struct WorldsFedConfig {
    /// `WORLDS_FED_PEERS_FILE`. `None` when unset or empty.
    pub peers_file: Option<PathBuf>,
    /// `WORLDS_FED_POLL_INTERVAL_SECS`, always >= 1.
    pub poll_interval_secs: u64,
    /// `WORLDS_FED_MAX_RESPONSE_BYTES`, always >= 1.
    pub max_response_bytes: u64,
    /// `WORLDS_FED_MAX_WORLDS_PER_PEER`, always >= 1.
    pub max_worlds_per_peer: u64,
    /// `WORLDS_FED_ALLOW_INSECURE_LOOPBACK_PEERS`. Permits a `http://127.0.0.1:*` peer
    /// with no pinned root, and nothing else. Named in its own warning line every time
    /// it fires (the `LIVEKIT_ALLOW_DEV_CREDS` precedent).
    pub allow_insecure_loopback_peers: bool,
    /// This node's own URLs; a `worlds_url` on one of their origins is a loop.
    pub own_urls: Vec<String>,
}

impl Default for WorldsFedConfig {
    fn default() -> Self {
        Self {
            peers_file: None,
            poll_interval_secs: DEFAULT_POLL_INTERVAL_SECS,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
            max_worlds_per_peer: DEFAULT_MAX_WORLDS_PER_PEER,
            allow_insecure_loopback_peers: false,
            own_urls: Vec::new(),
        }
    }
}

fn positive(name: &str, value: u64) -> Result<u64> {
    if value == 0 {
        return Err(anyhow!("{name} must be a positive integer, got {value}"));
    }
    Ok(value)
}

fn env_u64(name: &str, default: u64) -> Result<u64> {
    match std::env::var(name) {
        Ok(raw) if !raw.trim().is_empty() => raw
            .trim()
            .parse::<u64>()
            .map_err(|e| anyhow!("{name} must be a positive integer, got {raw:?}: {e}")),
        _ => Ok(default),
    }
}

fn env_bool(name: &str, default: bool) -> bool {
    match std::env::var(name) {
        Ok(raw) => matches!(
            raw.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => default,
    }
}

impl WorldsFedConfig {
    /// The checked constructor `serve` calls with the clap values (or
    /// [`Self::from_env`] with the raw environment).
    pub fn new(
        peers_file: Option<PathBuf>,
        poll_interval_secs: u64,
        max_response_bytes: u64,
        max_worlds_per_peer: u64,
        allow_insecure_loopback_peers: bool,
    ) -> Result<Self> {
        Ok(Self {
            peers_file: peers_file.filter(|p| !p.as_os_str().is_empty()),
            poll_interval_secs: positive("WORLDS_FED_POLL_INTERVAL_SECS", poll_interval_secs)?,
            max_response_bytes: positive("WORLDS_FED_MAX_RESPONSE_BYTES", max_response_bytes)?,
            max_worlds_per_peer: positive("WORLDS_FED_MAX_WORLDS_PER_PEER", max_worlds_per_peer)?,
            allow_insecure_loopback_peers,
            own_urls: Vec::new(),
        })
    }

    pub fn from_env() -> Result<Self> {
        let peers_file = std::env::var("WORLDS_FED_PEERS_FILE")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .map(PathBuf::from);
        Self::new(
            peers_file,
            env_u64("WORLDS_FED_POLL_INTERVAL_SECS", DEFAULT_POLL_INTERVAL_SECS)?,
            env_u64("WORLDS_FED_MAX_RESPONSE_BYTES", DEFAULT_MAX_RESPONSE_BYTES)?,
            env_u64(
                "WORLDS_FED_MAX_WORLDS_PER_PEER",
                DEFAULT_MAX_WORLDS_PER_PEER,
            )?,
            env_bool("WORLDS_FED_ALLOW_INSECURE_LOOPBACK_PEERS", false),
        )
    }

    /// The node's own URLs for the loop check (`PUBLIC_URL`, `CONTENT_URL`, the bind address).
    pub fn with_own_urls(mut self, own_urls: Vec<String>) -> Self {
        self.own_urls = own_urls;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_is_refused_for_every_cap_with_the_crate_message_shape() {
        for key in [
            "WORLDS_FED_POLL_INTERVAL_SECS",
            "WORLDS_FED_MAX_RESPONSE_BYTES",
            "WORLDS_FED_MAX_WORLDS_PER_PEER",
        ] {
            let err = positive(key, 0).unwrap_err();
            assert_eq!(
                err.to_string(),
                format!("{key} must be a positive integer, got 0")
            );
            assert_eq!(positive(key, 1).unwrap(), 1);
        }
        assert!(WorldsFedConfig::new(None, 0, 1, 1, false).is_err());
        assert!(WorldsFedConfig::new(None, 1, 0, 1, false).is_err());
        assert!(WorldsFedConfig::new(None, 1, 1, 0, false).is_err());
        let empty_path = WorldsFedConfig::new(Some(PathBuf::new()), 1, 1, 1, false).unwrap();
        assert!(
            empty_path.peers_file.is_none(),
            "an empty path is unset, not a file named ''"
        );
    }

    #[test]
    fn defaults_are_the_documented_ones() {
        let d = WorldsFedConfig::default();
        assert!(d.peers_file.is_none(), "federation is off unless asked for");
        assert!(!d.allow_insecure_loopback_peers);
        assert_eq!(d.poll_interval_secs, 300);
        assert_eq!(d.max_response_bytes, 4 * 1024 * 1024);
        assert_eq!(d.max_worlds_per_peer, 10_000);
        assert!(d.own_urls.is_empty());
    }
}
