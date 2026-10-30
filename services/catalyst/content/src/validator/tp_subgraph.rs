//! The two L2 subgraphs a third-party deploy is checked against: the blocks
//! subgraph (`BLOCKS_L2_SUBGRAPH_URL`: the block a deploy's timestamp falls
//! in) and the Third Party Registry (`THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL`:
//! the approved third party's merkle root at that block). Every request goes
//! through the process's ONE [`Outbound`] client (the SSRF guard, the DNS pin,
//! redirects re-checked per hop) with a capped body; both URLs are checked at
//! startup to be public https endpoints that are not this node
//! ([`super::Settings::parse`]) and never appear in a log line or an error.
//!
//! What changed: the old reader built its own reqwest client, panicked at
//! startup on a plaintext URL, and answered `None` both for "the registry
//! has no approved root" and for "the request failed", so an outage became
//! the depositor's rejection. Here a transport or upstream failure is an
//! `Err` (the validator answers `Unavailable`) and only a confirmed answer
//! is `Ok`. The squid root source (`THIRD_PARTY_ROOT_SOURCE=squid` and the
//! `third-party-root-refresh` job that fed `squid_marketplace.third_party`)
//! is gone with the squid: the registry root at the deploy's block is the
//! reference validator's source.

use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::Mutex;
use tracing::debug;

use crate::http::{Outbound, read_body_capped};

/// Bounds the third-party-root memo; `(tp_id, block)` pairs are few in practice,
/// so this cap is only a safety valve -- on overflow the whole map is cleared
/// and entries are re-fetched.
const TP_ROOT_CACHE_CAP: usize = 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
/// A blocks or registry answer is a few hundred bytes.
const BODY_CAP: usize = 1024 * 1024;

type RootCache = Mutex<HashMap<(String, u64), Option<[u8; 32]>>>;

pub struct TpSubgraph {
    http: Outbound,
    blocks_l2_url: String,
    tpr_url: String,
    /// A finalized L2 block is immutable, so a fetched (or confirmed-absent)
    /// root for a `(tp_id, block)` pair is safe to cache forever. Transport
    /// failures are NOT inserted (see `third_party_root_cached`), so a transient
    /// subgraph outage can never pin a false rejection.
    root_cache: RootCache,
}

impl TpSubgraph {
    pub fn new(http: Outbound, blocks_l2_url: String, tpr_url: String) -> Self {
        Self {
            http,
            blocks_l2_url,
            tpr_url,
            root_cache: Mutex::new(HashMap::new()),
        }
    }

    /// The `data` of one GraphQL answer; the error text never carries the URL.
    async fn graphql(&self, url: &str, query: &str, variables: Value) -> Result<Value, String> {
        let resp = self
            .http
            .post(url)
            .timeout(REQUEST_TIMEOUT)
            .json(&json!({ "query": query, "variables": variables }))
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(format!("HTTP {status}"));
        }
        let bytes = read_body_capped(resp, BODY_CAP)
            .await
            .map_err(|e| format!("body: {e}"))?;
        let body: Value =
            serde_json::from_slice(&bytes).map_err(|e| format!("unparseable answer: {e}"))?;
        if let Some(errors) = body
            .get("errors")
            .and_then(Value::as_array)
            .filter(|e| !e.is_empty())
        {
            return Err(format!("errors: {}", Value::Array(errors.clone())));
        }
        body.get("data")
            .filter(|d| !d.is_null())
            .cloned()
            .ok_or_else(|| "answer without data".to_string())
    }

    /// The last L2 block at or before the deploy (the window reaches 8 s past
    /// the timestamp and 5 minutes before it). `Ok(None)`: the subgraph
    /// answered with no block in the window (it has not indexed that far).
    /// Three attempts with doubling backoff and full jitter; `Err` when every
    /// attempt failed on the transport or upstream side.
    pub async fn block_for_timestamp(&self, timestamp_ms: i64) -> Result<Option<u64>, String> {
        let timestamp_sec = (timestamp_ms as f64 / 1000.0).ceil() as i64 + 8;
        let timestamp_5min = (timestamp_sec - 60 * 5 - 7).max(0);
        let query = r#"query getBlockForTimestampRange($timestamp: Int!, $timestamp5Min: Int!) {
            max: blocks(where: {timestamp_gte: $timestamp5Min, timestamp_lte: $timestamp}, first: 1, orderBy: timestamp, orderDirection: desc) { number }
            min: blocks(where: {timestamp_gte: $timestamp5Min, timestamp_lte: $timestamp}, first: 1, orderBy: timestamp, orderDirection: asc) { number }
        }"#;
        let variables = json!({ "timestamp": timestamp_sec, "timestamp5Min": timestamp_5min });

        let mut last = Ok(None);
        for attempt in 1..=BLOCK_FETCH_MAX_RETRIES {
            last = self
                .graphql(&self.blocks_l2_url, query, variables.clone())
                .await
                .map(|data| parse_max_block(&data));
            if matches!(last, Ok(Some(_))) {
                return last;
            }
            if attempt < BLOCK_FETCH_MAX_RETRIES {
                let base = block_fetch_backoff_base(attempt);
                let backoff = base + block_fetch_jitter(base);
                debug!(
                    attempt,
                    backoff_ms = backoff,
                    "retrying L2 block-for-timestamp lookup"
                );
                tokio::time::sleep(Duration::from_millis(backoff)).await;
            }
        }
        debug!("L2 block-for-timestamp lookup exhausted retries");
        last
    }

    /// The approved third party's merkle root at `block`; `Ok(None)` when the
    /// registry has no approved third party with that id there (or its root
    /// does not decode).
    pub async fn third_party_root(
        &self,
        third_party_id: &str,
        block: u64,
    ) -> Result<Option<[u8; 32]>, String> {
        third_party_root_cached(&self.root_cache, third_party_id, block, || async {
            let query = r#"query MerkleRoot($id: ID!, $block: Int!) {
                thirdParties(where: { id: $id, isApproved: true }, block: { number: $block }, first: 1) { root }
            }"#;
            let data = self
                .graphql(&self.tpr_url, query, json!({ "id": third_party_id, "block": block }))
                .await?;
            Ok(data
                .get("thirdParties")
                .and_then(|t| t.as_array())
                .and_then(|a| a.first())
                .and_then(|tp| tp.get("root"))
                .and_then(|r| r.as_str())
                .and_then(super::merkle::decode_hash32))
        })
        .await
    }
}

fn parse_max_block(data: &Value) -> Option<u64> {
    let num = data
        .get("max")
        .and_then(|m| m.as_array())
        .and_then(|a| a.first())
        .and_then(|b| b.get("number"))?;
    parse_u64(num)
}

/// The `fetch` closure returns `Err` for a transport failure (NOT cached, so a
/// transient outage never pins a false rejection) and `Ok(root_opt)` for a
/// confirmed answer (cached -- the block is finalized, so the answer is
/// immutable).
async fn third_party_root_cached<F, Fut>(
    cache: &RootCache,
    third_party_id: &str,
    block: u64,
    fetch: F,
) -> Result<Option<[u8; 32]>, String>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<Option<[u8; 32]>, String>>,
{
    let key = (third_party_id.to_string(), block);
    if let Some(cached) = cache.lock().await.get(&key) {
        return Ok(*cached);
    }
    let result = fetch().await?;
    let mut guard = cache.lock().await;
    if guard.len() >= TP_ROOT_CACHE_CAP {
        guard.clear();
    }
    guard.insert(key, result);
    Ok(result)
}

const BLOCK_FETCH_MAX_RETRIES: u32 = 3;
const BLOCK_FETCH_BASE_DELAY_MS: u64 = 100;

fn block_fetch_backoff_base(attempt: u32) -> u64 {
    BLOCK_FETCH_BASE_DELAY_MS * (1u64 << (attempt - 1))
}

fn block_fetch_jitter(base: u64) -> u64 {
    if base == 0 {
        return 0;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    nanos % base
}

fn parse_u64(v: &Value) -> Option<u64> {
    match v {
        Value::String(s) => s.parse().ok(),
        Value::Number(n) => n.as_u64(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn tp_root_fetched_once_per_block() {
        let cache: RootCache = Mutex::new(HashMap::new());
        let fetches = AtomicUsize::new(0);
        let root = [7u8; 32];

        for _ in 0..10 {
            let got = third_party_root_cached(&cache, "urn:tp:foo", 42, || async {
                fetches.fetch_add(1, Ordering::SeqCst);
                Ok(Some(root))
            })
            .await;
            assert_eq!(got, Ok(Some(root)));
        }
        assert_eq!(
            fetches.load(Ordering::SeqCst),
            1,
            "the finalized-block root must be fetched at most once"
        );
    }

    #[tokio::test]
    async fn tp_root_transport_failure_not_cached() {
        let cache: RootCache = Mutex::new(HashMap::new());
        let fetches = AtomicUsize::new(0);
        let root = [9u8; 32];

        let a = third_party_root_cached(&cache, "urn:tp:bar", 7, || async {
            fetches.fetch_add(1, Ordering::SeqCst);
            Err("down".to_string())
        })
        .await;
        assert_eq!(a, Err("down".to_string()));

        let b = third_party_root_cached(&cache, "urn:tp:bar", 7, || async {
            fetches.fetch_add(1, Ordering::SeqCst);
            Ok(Some(root))
        })
        .await;
        assert_eq!(b, Ok(Some(root)));

        let c = third_party_root_cached(&cache, "urn:tp:bar", 7, || async {
            fetches.fetch_add(1, Ordering::SeqCst);
            Ok(Some([0u8; 32]))
        })
        .await;
        assert_eq!(c, Ok(Some(root)));

        assert_eq!(
            fetches.load(Ordering::SeqCst),
            2,
            "only the failed attempt and the first success fetch; the cached hit must not"
        );
    }

    #[tokio::test]
    async fn a_confirmed_absent_root_is_cached_too() {
        let cache: RootCache = Mutex::new(HashMap::new());
        let fetches = AtomicUsize::new(0);
        for _ in 0..3 {
            let got = third_party_root_cached(&cache, "urn:tp:none", 1, || async {
                fetches.fetch_add(1, Ordering::SeqCst);
                Ok(None)
            })
            .await;
            assert_eq!(got, Ok(None));
        }
        assert_eq!(fetches.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn retry_budget_is_bounded() {
        assert_eq!(BLOCK_FETCH_MAX_RETRIES, 3);
        const {
            assert!(
                BLOCK_FETCH_MAX_RETRIES > 1,
                "there must be at least one retry after the initial attempt"
            )
        };
        assert_eq!(BLOCK_FETCH_MAX_RETRIES - 1, 2);
    }

    #[test]
    fn backoff_base_doubles_each_attempt() {
        assert_eq!(block_fetch_backoff_base(1), 100);
        assert_eq!(block_fetch_backoff_base(2), 200);
        assert_eq!(block_fetch_backoff_base(3), 400);
        for attempt in 1..BLOCK_FETCH_MAX_RETRIES {
            assert!(block_fetch_backoff_base(attempt) < block_fetch_backoff_base(attempt + 1));
        }
    }

    #[test]
    fn jitter_stays_within_full_jitter_bounds() {
        assert_eq!(block_fetch_jitter(0), 0);

        for attempt in 1..BLOCK_FETCH_MAX_RETRIES {
            let base = block_fetch_backoff_base(attempt);
            for _ in 0..1000 {
                let jitter = block_fetch_jitter(base);
                assert!(jitter < base, "jitter {jitter} must be < base {base}");
                let total = base + jitter;
                assert!(total >= base && total < 2 * base);
            }
        }
    }

    #[test]
    fn the_max_block_is_read_from_either_number_form() {
        assert_eq!(
            parse_max_block(&json!({ "max": [{ "number": "123" }], "min": [] })),
            Some(123)
        );
        assert_eq!(
            parse_max_block(&json!({ "max": [{ "number": 7 }] })),
            Some(7)
        );
        assert_eq!(parse_max_block(&json!({ "max": [] })), None);
    }
}
