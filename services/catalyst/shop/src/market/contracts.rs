//! `/lambdas/contracts/servers`: the catalyst registry read from the
//! `Catalyst` contract over `ETH_RPC_URL` (`catalystCount`, `catalystIds(i)`,
//! `catalystById(id)`); `/lambdas/contracts/pois` (the POI list contract over
//! the polygon endpoint, `RPC_ENDPOINT_POLYGON` or `TRADE_RPC_URLS` 137) and
//! `/lambdas/contracts/denylisted-names` (the name denylist contract over
//! `ETH_RPC_URL`): `size()`, then `get(i)` per entry. Each is cached for six
//! hours with a one-minute back-off after a failed refresh (stale is still
//! served); without its endpoint a list answers empty, as the old handlers
//! did. The old handler batched the calls through Multicall3 and fell back
//! to one `eth_call` per entry; v2 keeps only the per-entry path (16
//! concurrent), a few hundred calls every six hours. The
//! `/lambdas/third-party-integrations` sibling is `third_party`.

use std::time::{Duration, Instant};

use alloy_primitives::Address;
use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use parking_lot::Mutex;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::http::{RetryDecision, is_transient_status, read_body_capped, retry_with_backoff};
use crate::shared::cache::TtlMap;

use super::{MarketplaceState, SharedState};

struct ContractAddrs {
    catalyst: &'static str,
    name_denylist: &'static str,
    poi: &'static str,
}

fn contracts_for(network: &str) -> ContractAddrs {
    match network {
        "sepolia" => ContractAddrs {
            catalyst: "0x9b5091588a4bae0a5ea54a35af3c31f57a68ed37",
            name_denylist: "0x6082b0b10b0fe9040652e35acbf3a22fe6764f27",
            poi: "0x7a0fad6854de8df1245da952cd3ae7f6893154c1",
        },
        _ => ContractAddrs {
            catalyst: "0x4a2f10076101650f40342885b99b6b101d83c486",
            name_denylist: "0x0c4c90a4f29872a2e9ef4c4be3d419792bca9a36",
            poi: "0xFEC09d5C192aaf7Ec7E2C89Cc8D3224138391B2E",
        },
    }
}

const SEL_CATALYST_COUNT: &str = "18becc10";
const SEL_CATALYST_IDS: &str = "7b9b4f2c";
const SEL_CATALYST_BY_ID: &str = "c9038ce9";
const SEL_SIZE: &str = "949d225d";
const SEL_GET: &str = "9507d39a";

const CACHE_TTL: Duration = Duration::from_secs(6 * 60 * 60);
const RETRY_AFTER: Duration = Duration::from_secs(60);
const ETH_CALL_CONCURRENCY: usize = 16;
const RPC_BODY_CAP: usize = 1024 * 1024;
const RPC_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Deserialize)]
struct RpcResponse {
    result: Option<Value>,
    error: Option<RpcError>,
}

#[derive(Deserialize)]
struct RpcError {
    code: i64,
    message: String,
}

async fn eth_call(
    state: &MarketplaceState,
    rpc_url: &str,
    to: &str,
    data_hex: &str,
) -> Result<String, String> {
    if !state.upstream_allowed(rpc_url) {
        return Err("ETH_RPC_URL is not a publicly routable http(s) endpoint".to_string());
    }
    let client = &state.http;
    let req = json!({
        "jsonrpc": "2.0",
        "method": "eth_call",
        "params": [ { "to": to, "data": format!("0x{data_hex}") }, "latest" ],
        "id": 1
    });
    let answer = retry_with_backoff("eth_call", 3, 500, || {
        let req = req.clone();
        async move {
            let resp = match client
                .post(rpc_url)
                .timeout(RPC_TIMEOUT)
                .json(&req)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) if e.is_connect() || e.is_timeout() => {
                    return Ok(RetryDecision::Retry {
                        after: None,
                        rate_limited: false,
                    });
                }
                Err(e) => return Err(anyhow::anyhow!("RPC request failed: {e}")),
            };
            let status = resp.status();
            if is_transient_status(status) {
                return Ok(RetryDecision::Retry {
                    after: None,
                    rate_limited: status == reqwest::StatusCode::TOO_MANY_REQUESTS,
                });
            }
            if !status.is_success() {
                return Err(anyhow::anyhow!("RPC returned HTTP {status}"));
            }
            let bytes = read_body_capped(resp, RPC_BODY_CAP).await?;
            let body: RpcResponse = serde_json::from_slice(&bytes)
                .map_err(|e| anyhow::anyhow!("failed to parse RPC response: {e}"))?;
            Ok(RetryDecision::Done(body))
        }
    })
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "RPC unavailable after retries".to_string())?;
    if let Some(err) = answer.error {
        return Err(format!("RPC error {}: {}", err.code, err.message));
    }
    match answer.result {
        Some(Value::String(s)) => Ok(s),
        Some(other) => Err(format!("unexpected RPC result type: {other}")),
        None => Err("RPC returned null result with no error".into()),
    }
}

fn strip0x(s: &str) -> &str {
    s.strip_prefix("0x").unwrap_or(s)
}

fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    let b = s.as_bytes();
    if b.len() >= 2 && b[0] == b'0' && (b[1] == b'x' || b[1] == b'X') {
        return Err(format!("invalid hex char: {}", b[1] as char));
    }
    hex::decode(s).map_err(|e| e.to_string())
}

fn encode_uint256(n: u64) -> String {
    format!("{n:064x}")
}

fn to_checksum_address(addr: &str) -> String {
    match hex_decode(strip0x(addr)) {
        Ok(bytes) if bytes.len() == 20 => Address::from_slice(&bytes).to_checksum(None),
        _ => format!("0x{}", strip0x(addr).to_ascii_lowercase()),
    }
}

fn decode_uint_word(hex_word: &str) -> Result<u64, String> {
    let h = strip0x(hex_word);
    if h.len() < 64 {
        return Err(format!("word too short: {h}"));
    }
    let tail = &h[64 - 16..64];
    u64::from_str_radix(tail, 16).map_err(|e| format!("bad uint word: {e}"))
}

fn decode_string_at(bytes: &[u8], head_word_idx: usize) -> Result<String, String> {
    let head_off = head_word_idx * 32;
    if bytes.len() < head_off + 32 {
        return Err("truncated head".into());
    }
    let offset = be_word_to_usize(&bytes[head_off..head_off + 32])?;
    if bytes.len() < offset + 32 {
        return Err("truncated string length".into());
    }
    let len = be_word_to_usize(&bytes[offset..offset + 32])?;
    let start = offset + 32;
    if bytes.len() < start + len {
        return Err("truncated string data".into());
    }
    String::from_utf8(bytes[start..start + len].to_vec())
        .map_err(|e| format!("string not utf8: {e}"))
}

fn be_word_to_usize(word: &[u8]) -> Result<usize, String> {
    if word.len() != 32 {
        return Err("word not 32 bytes".into());
    }
    let mut v: usize = 0;
    for &b in &word[24..32] {
        v = (v << 8) | b as usize;
    }
    Ok(v)
}

/// One `eth_call` per calldata, `ETH_CALL_CONCURRENCY` at a time; any failure fails all.
async fn calls(
    state: &MarketplaceState,
    rpc: &str,
    contract: &str,
    calldatas: Vec<String>,
) -> Result<Vec<Vec<u8>>, String> {
    use futures::stream::{self, StreamExt, TryStreamExt};
    stream::iter(calldatas)
        .map(|calldata| async move {
            let hex = eth_call(state, rpc, contract, &calldata).await?;
            hex_decode(strip0x(&hex)).map_err(|e| format!("bad hex: {e}"))
        })
        .buffered(ETH_CALL_CONCURRENCY)
        .try_collect()
        .await
}

/// `ids` are the 64-hex-char catalyst ids, `records` the matching `catalystById` returns.
fn servers_from_records(ids: &[String], records: Vec<Vec<u8>>) -> Result<Vec<Value>, String> {
    let mut out = Vec::with_capacity(ids.len());
    for (id, rec) in ids.iter().zip(records) {
        if rec.len() < 96 {
            return Err(format!(
                "catalystById return too short: 0x{}",
                hex::encode(&rec)
            ));
        }
        let owner = to_checksum_address(&hex::encode(&rec[44..64]));
        let domain = decode_string_at(&rec, 2)?;
        if domain.starts_with("http://") {
            continue;
        }
        let mut address = domain.clone();
        if !address.starts_with("https://") {
            address = format!("https://{address}");
        }
        out.push(json!({
            "baseUrl": address.trim(),
            "owner": owner,
            "id": format!("0x{id}"),
        }));
    }
    Ok(out)
}

async fn fetch_servers(
    state: &MarketplaceState,
    rpc: &str,
    network: &str,
) -> Result<Vec<Value>, String> {
    let c = contracts_for(network);
    let count_hex = eth_call(state, rpc, c.catalyst, SEL_CATALYST_COUNT).await?;
    let count = decode_uint_word(&count_hex)?;

    let id_calls = (0..count)
        .map(|i| format!("{SEL_CATALYST_IDS}{}", encode_uint256(i)))
        .collect();
    let id_words = calls(state, rpc, c.catalyst, id_calls).await?;
    let mut ids = Vec::with_capacity(id_words.len());
    for word in &id_words {
        if word.len() < 32 {
            return Err(format!("bad catalystIds return: 0x{}", hex::encode(word)));
        }
        ids.push(hex::encode(&word[..32]));
    }

    let record_calls = ids
        .iter()
        .map(|id| format!("{SEL_CATALYST_BY_ID}{id}"))
        .collect();
    let records = calls(state, rpc, c.catalyst, record_calls).await?;
    servers_from_records(&ids, records)
}

/// A strings list contract: `size()`, then `get(i)` for every entry.
async fn fetch_list(
    state: &MarketplaceState,
    rpc: &str,
    contract: &str,
) -> Result<Vec<String>, String> {
    let size_hex = eth_call(state, rpc, contract, SEL_SIZE).await?;
    let size = decode_uint_word(&size_hex)?;
    let get_calls = (0..size)
        .map(|i| format!("{SEL_GET}{}", encode_uint256(i)))
        .collect();
    calls(state, rpc, contract, get_calls)
        .await?
        .iter()
        .map(|bytes| decode_string_at(bytes, 0))
        .collect()
}

/// A 6 h cell whose failed refreshes are not retried for `RETRY_AFTER`; the
/// last good value keeps being served meanwhile.
pub struct Slot {
    value: TtlMap<(), Value>,
    stale: Mutex<Option<Value>>,
    failed_at: Mutex<Option<Instant>>,
}

impl Default for Slot {
    fn default() -> Self {
        Self {
            value: TtlMap::new(CACHE_TTL),
            stale: Mutex::new(None),
            failed_at: Mutex::new(None),
        }
    }
}

impl Slot {
    async fn get<F, Fut>(&self, what: &str, fetch: F) -> Result<Value, String>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<Value, String>>,
    {
        let fetched = self
            .value
            .get_or_fetch((), || async {
                let recent = self
                    .failed_at
                    .lock()
                    .is_some_and(|at| at.elapsed() < RETRY_AFTER);
                if recent {
                    return Err("upstream failed recently; backing off".to_string());
                }
                let fetched = fetch().await;
                *self.failed_at.lock() = fetched.is_err().then(Instant::now);
                if let Ok(v) = &fetched {
                    *self.stale.lock() = Some(v.clone());
                }
                fetched
            })
            .await;
        match fetched {
            Ok(v) => Ok(v),
            Err(e) => match self.stale.lock().clone() {
                Some(stale) => {
                    tracing::warn!(error = %e, "contracts/{what} refresh failed; serving the stale answer");
                    Ok(stale)
                }
                None => Err(e),
            },
        }
    }
}

#[derive(Default)]
pub struct Caches {
    servers: Slot,
    pois: Slot,
    denylisted_names: Slot,
}

pub async fn contracts_servers(State(state): State<SharedState>) -> impl IntoResponse {
    let Some(rpc) = state.settings.eth_rpc_url.clone() else {
        tracing::debug!("ETH_RPC_URL is unset; /lambdas/contracts/servers answers empty");
        return Json(json!([]));
    };
    let network = state.settings.eth_network.clone();
    let st = state.clone();
    match state
        .contracts
        .servers
        .get("servers", || async move {
            fetch_servers(&st, &rpc, &network).await.map(Value::Array)
        })
        .await
    {
        Ok(v) => Json(v),
        Err(e) => {
            tracing::error!("contracts/servers failed: {e}");
            Json(json!([]))
        }
    }
}

/// `GET /lambdas/contracts/pois`: the POI contract's list, over the polygon endpoint.
pub async fn contracts_pois(State(state): State<SharedState>) -> impl IntoResponse {
    let Some(rpc) = state
        .settings
        .polygon_rpc_url
        .clone()
        .or_else(|| state.chain_rpc_url(super::chain_walk::ChainNet::Matic))
    else {
        tracing::debug!("RPC_ENDPOINT_POLYGON is unset; /lambdas/contracts/pois answers empty");
        return Json(json!([]));
    };
    let contract = contracts_for(&state.settings.eth_network).poi.to_string();
    let st = state.clone();
    let fetch = || async move {
        fetch_list(&st, &rpc, &contract)
            .await
            .map(|l| Value::Array(l.into_iter().map(Value::String).collect()))
    };
    match state.contracts.pois.get("pois", fetch).await {
        Ok(v) => Json(v),
        Err(e) => {
            tracing::error!("contracts/pois failed: {e}");
            Json(json!([]))
        }
    }
}

/// `GET /lambdas/contracts/denylisted-names`: the name denylist contract's list, over `ETH_RPC_URL`.
pub async fn contracts_denylisted_names(State(state): State<SharedState>) -> impl IntoResponse {
    let Some(rpc) = state.settings.eth_rpc_url.clone() else {
        tracing::debug!("ETH_RPC_URL is unset; /lambdas/contracts/denylisted-names answers empty");
        return Json(json!([]));
    };
    let contract = contracts_for(&state.settings.eth_network)
        .name_denylist
        .to_string();
    let st = state.clone();
    let fetch = || async move {
        fetch_list(&st, &rpc, &contract)
            .await
            .map(|l| Value::Array(l.into_iter().map(Value::String).collect()))
    };
    match state
        .contracts
        .denylisted_names
        .get("denylisted-names", fetch)
        .await
    {
        Ok(v) => Json(v),
        Err(e) => {
            tracing::error!("contracts/denylisted-names failed: {e}");
            Json(json!([]))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_decode_rejects_residual_0x_prefix() {
        assert_eq!(hex_decode("0xdead"), Err("invalid hex char: x".to_string()));
        assert_eq!(hex_decode("0Xdead"), Err("invalid hex char: X".to_string()));
        assert_eq!(hex_decode("dead"), Ok(vec![0xde, 0xad]));
    }

    #[test]
    fn eip55_checksum() {
        assert_eq!(
            to_checksum_address("0x75e1d32289679dfcb2f01fbc0e043b3d7f9cd443"),
            "0x75e1d32289679dfcB2F01fBc0e043B3d7F9Cd443"
        );
        assert_eq!(
            to_checksum_address("5aaeb6053f3e94c9b9a09f33669435e7ef1beaed"),
            "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed"
        );
    }

    #[test]
    fn the_list_contracts_are_the_old_ones() {
        let main = contracts_for("mainnet");
        assert_eq!(main.poi, "0xFEC09d5C192aaf7Ec7E2C89Cc8D3224138391B2E");
        assert_eq!(
            main.name_denylist,
            "0x0c4c90a4f29872a2e9ef4c4be3d419792bca9a36"
        );
        assert_eq!(
            contracts_for("sepolia").poi,
            "0x7a0fad6854de8df1245da952cd3ae7f6893154c1"
        );
        assert_eq!((SEL_SIZE, SEL_GET), ("949d225d", "9507d39a"));
    }

    #[tokio::test]
    async fn a_failed_refresh_backs_off_and_serves_the_stale_answer() {
        let slot = Slot::default();
        assert!(
            slot.get("t", || async { Err::<Value, _>("down".to_string()) })
                .await
                .is_err()
        );
        assert_eq!(
            slot.get("t", || async { Ok(json!(["x"])) })
                .await
                .unwrap_err(),
            "upstream failed recently; backing off"
        );
        *slot.failed_at.lock() = None;
        assert_eq!(
            slot.get("t", || async { Ok(json!(["x"])) }).await.unwrap(),
            json!(["x"])
        );
        slot.value.clear();
        *slot.failed_at.lock() = None;
        assert_eq!(
            slot.get("t", || async { Err::<Value, _>("down".to_string()) })
                .await
                .unwrap(),
            json!(["x"]),
            "stale after a failure"
        );
    }

    #[test]
    fn decode_string_simple() {
        let hexed = "0000000000000000000000000000000000000000000000000000000000000020\
                     0000000000000000000000000000000000000000000000000000000000000005\
                     68656c6c6f000000000000000000000000000000000000000000000000000000";
        let bytes = hex_decode(hexed).unwrap();
        assert_eq!(decode_string_at(&bytes, 0).unwrap(), "hello");
    }

    #[test]
    fn servers_from_records_keeps_https_and_skips_http() {
        fn record(domain: &str) -> Vec<u8> {
            let mut rec = vec![0u8; 96];
            rec[44..64]
                .copy_from_slice(&hex_decode("75e1d32289679dfcb2f01fbc0e043b3d7f9cd443").unwrap());
            rec[64..96].copy_from_slice(&hex_decode(&encode_uint256(96)).unwrap());
            rec.extend_from_slice(&hex_decode(&encode_uint256(domain.len() as u64)).unwrap());
            rec.extend_from_slice(domain.as_bytes());
            rec.resize(rec.len().div_ceil(32) * 32, 0);
            rec
        }
        let ids: Vec<String> = ["11", "22", "33"].iter().map(|s| s.repeat(32)).collect();
        let out = servers_from_records(
            &ids,
            vec![
                record("peer.example "),
                record("http://plain.example"),
                record("https://secure.example"),
            ],
        )
        .unwrap();
        assert_eq!(out.len(), 2, "http:// entries are dropped");
        assert_eq!(out[0]["baseUrl"], "https://peer.example");
        assert_eq!(
            out[0]["owner"],
            "0x75e1d32289679dfcB2F01fBc0e043B3d7F9Cd443"
        );
        assert_eq!(out[0]["id"], format!("0x{}", "11".repeat(32)));
        assert_eq!(out[1]["baseUrl"], "https://secure.example");
        assert_eq!(out[1]["id"], format!("0x{}", "33".repeat(32)));
        assert!(servers_from_records(&ids[..1], vec![vec![0u8; 40]]).is_err());
    }

    #[test]
    fn decode_uint_word_works() {
        assert_eq!(
            decode_uint_word("0x0000000000000000000000000000000000000000000000000000000000000008")
                .unwrap(),
            8
        );
        assert!(decode_uint_word("0x1234").is_err());
    }

    #[tokio::test]
    async fn a_private_rpc_endpoint_is_refused_before_any_call() {
        let state = crate::market::tests::support::state_without_db();
        let err = eth_call(&state, "http://127.0.0.1:8545", "0x0", SEL_CATALYST_COUNT)
            .await
            .unwrap_err();
        assert!(err.contains("not a publicly routable"), "{err}");
    }

    #[tokio::test]
    async fn the_cache_serves_the_stale_answer_while_a_refresh_backs_off() {
        let caches = Caches::default();
        let first = caches
            .servers
            .get("servers", || async { Ok(json!(["a"])) })
            .await
            .unwrap();
        assert_eq!(first, json!(["a"]));
        // A fresh cell never refetches inside its TTL.
        let again = caches
            .servers
            .get("servers", || async { Err("boom".to_string()) })
            .await
            .unwrap();
        assert_eq!(again, json!(["a"]));

        let cold = Caches::default();
        assert_eq!(
            cold.servers
                .get("servers", || async { Err("boom".to_string()) })
                .await
                .unwrap_err(),
            "boom"
        );
        let err = cold
            .servers
            .get("servers", || async { Ok(json!(["late"])) })
            .await
            .unwrap_err();
        assert!(
            err.contains("backing off"),
            "a failure within RETRY_AFTER is not retried: {err}"
        );
    }
}
