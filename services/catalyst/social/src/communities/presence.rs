//! Comms presence for the `onlyOnline` member filter, read from the
//! archipelago stats `/peers` endpoint (the same source upstream's
//! archipelagoStats adapter polls). Presence is advisory: every failure, and
//! an unconfigured source, degrades to the empty set, so members render
//! offline rather than the request failing. Both outcomes are cached for ten
//! seconds, so a dead source costs at most one bounded probe per window.
//! `catalyst-comms` runs in this process, below social in the layers, so the
//! set can become a local read.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Deserialize;

const PEERS_CACHE_TTL: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Deserialize)]
struct PeersResponse {
    #[serde(default)]
    peers: Vec<PeerEntry>,
}

/// Upstream maps `peer.id`; `address` is the fallback because
/// catalyrst-archipelago serves both with equal value.
#[derive(Deserialize)]
struct PeerEntry {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    address: Option<String>,
}

pub struct Presence {
    client: reqwest::Client,
    /// `ARCHIPELAGO_STATS_URL`; `None` reports every member offline.
    base_url: Option<String>,
    cache: Mutex<Option<(Instant, Vec<String>)>>,
}

impl Presence {
    /// `client` is the process's raw client (the URL is operator
    /// configuration, typically a loopback service, not user input).
    pub fn new(client: reqwest::Client, base_url: Option<String>) -> Self {
        Self {
            client,
            base_url: base_url
                .map(|u| u.trim_end_matches('/').to_string())
                .filter(|u| !u.is_empty()),
            cache: Mutex::new(None),
        }
    }

    pub fn is_configured(&self) -> bool {
        self.base_url.is_some()
    }

    /// Lowercased, deduplicated. Never errors.
    pub async fn connected_peers(&self) -> Vec<String> {
        let Some(base) = &self.base_url else {
            return Vec::new();
        };
        if let Some(cached) = self.cache_get() {
            return cached;
        }
        let peers = match self.fetch(base).await {
            Ok(peers) => peers,
            Err(e) => {
                tracing::warn!(error = %e, url = %base, "archipelago peers unavailable; reporting all members offline");
                Vec::new()
            }
        };
        self.cache_put(peers.clone());
        peers
    }

    fn cache_get(&self) -> Option<Vec<String>> {
        let cache = self.cache.lock().expect("peers cache poisoned");
        cache
            .as_ref()
            .filter(|(at, _)| at.elapsed() < PEERS_CACHE_TTL)
            .map(|(_, peers)| peers.clone())
    }

    fn cache_put(&self, peers: Vec<String>) {
        *self.cache.lock().expect("peers cache poisoned") = Some((Instant::now(), peers));
    }

    async fn fetch(&self, base: &str) -> anyhow::Result<Vec<String>> {
        let resp = self
            .client
            .get(format!("{base}/peers"))
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            anyhow::bail!("archipelago stats returned status {}", status.as_u16());
        }
        let body: PeersResponse = resp.json().await?;
        Ok(peer_addresses(body))
    }
}

fn peer_addresses(resp: PeersResponse) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for peer in resp.peers {
        let Some(addr) = peer.id.or(peer.address) else {
            continue;
        };
        let addr = addr.to_lowercase();
        if addr.is_empty() || !seen.insert(addr.clone()) {
            continue;
        }
        out.push(addr);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn parse(json: &str) -> Vec<String> {
        peer_addresses(serde_json::from_str(json).unwrap())
    }

    #[test]
    fn parses_ids_lowercased_and_deduplicated() {
        let json = r#"{"ok":true,"peers":[
            {"id":"0xAB","address":"0xAB"},
            {"id":"0xab"},
            {"address":"0xCD"},
            {"lastPing":1},
            {"id":""}
        ]}"#;
        assert_eq!(parse(json), vec!["0xab".to_string(), "0xcd".to_string()]);
        assert!(parse(r#"{"ok":true}"#).is_empty());
    }

    async fn serve(status: u16, body: &'static str, hits: Arc<AtomicUsize>) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = axum::Router::new().route(
            "/peers",
            axum::routing::get(move || {
                hits.fetch_add(1, Ordering::SeqCst);
                async move {
                    axum::response::Response::builder()
                        .status(status)
                        .header("content-type", "application/json")
                        .body(axum::body::Body::from(body))
                        .unwrap()
                }
            }),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn returns_connected_addresses_and_serves_repeats_from_cache() {
        let hits = Arc::new(AtomicUsize::new(0));
        let base = serve(
            200,
            r#"{"ok":true,"peers":[{"id":"0xAA"},{"id":"0xbb"}]}"#,
            hits.clone(),
        )
        .await;
        let presence = Presence::new(reqwest::Client::new(), Some(base));
        let peers = presence.connected_peers().await;
        assert_eq!(peers, vec!["0xaa".to_string(), "0xbb".to_string()]);
        assert_eq!(presence.connected_peers().await, peers);
        assert_eq!(
            hits.load(Ordering::SeqCst),
            1,
            "second read is cache-served"
        );
    }

    #[tokio::test]
    async fn upstream_error_unreachable_and_unconfigured_yield_empty_not_error() {
        let hits = Arc::new(AtomicUsize::new(0));
        let base = serve(500, "boom", hits.clone()).await;
        let presence = Presence::new(reqwest::Client::new(), Some(base));
        assert!(presence.connected_peers().await.is_empty());
        presence.connected_peers().await;
        assert_eq!(hits.load(Ordering::SeqCst), 1, "failure is cached too");

        let presence = Presence::new(
            reqwest::Client::new(),
            Some("http://127.0.0.1:1".to_string()),
        );
        assert!(presence.connected_peers().await.is_empty());
        let presence = Presence::new(reqwest::Client::new(), None);
        assert!(!presence.is_configured());
        assert!(presence.connected_peers().await.is_empty());
    }
}
