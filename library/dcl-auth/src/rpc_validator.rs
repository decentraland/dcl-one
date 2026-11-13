//! EIP-1271/EIP-1654 `isValidSignature(bytes32,bytes)` over JSON-RPC
//! `eth_call`. The validator borrows the process-wide reqwest client (the
//! raw half: `ETH_RPC_URL` is the operator's exact https endpoint) and
//! bounds each call with its own timeout, so a stalled RPC never holds a
//! deployment open; the request and answer shapes are the shared
//! `http::jsonrpc` ones. Transport errors are reported without the URL: it may
//! carry an API key, and the error text reaches logs and failure records.
use std::collections::{HashMap, HashSet};
use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;

use super::eip1654::Eip1654Validator;
use super::error::AuthError;
use dcl_http::{jsonrpc, read_body_capped};

pub const DEFAULT_RPC_TIMEOUT: Duration = Duration::from_secs(30);

const IS_VALID_SIGNATURE_SELECTOR: [u8; 4] = [0x16, 0x26, 0xba, 0x7e];

const EIP1271_MAGIC_VALUE: [u8; 4] = [0x16, 0x26, 0xba, 0x7e];

pub fn encode_is_valid_signature(hash: &[u8], signature: &[u8]) -> Vec<u8> {
    assert!(hash.len() == 32, "hash must be 32 bytes");

    let sig_len = signature.len();
    let padded_sig_len = sig_len.div_ceil(32) * 32;

    let mut buf = Vec::with_capacity(4 + 32 + 32 + 32 + padded_sig_len);

    buf.extend_from_slice(&IS_VALID_SIGNATURE_SELECTOR);

    buf.extend_from_slice(hash);

    let mut offset = [0u8; 32];
    offset[31] = 0x40;
    buf.extend_from_slice(&offset);

    let mut len_word = [0u8; 32];
    let len_bytes = (sig_len as u64).to_be_bytes();
    len_word[24..32].copy_from_slice(&len_bytes);
    buf.extend_from_slice(&len_word);

    buf.extend_from_slice(signature);
    buf.resize(buf.len() + padded_sig_len - sig_len, 0);

    buf
}

pub fn is_magic_return(hex_response: &str) -> bool {
    let hex = hex_response.strip_prefix("0x").unwrap_or(hex_response);
    if hex.len() < 8 {
        return false;
    }
    let Ok(first4) = hex::decode(&hex[..8]) else {
        return false;
    };
    first4.as_slice() == EIP1271_MAGIC_VALUE
}

pub struct RpcEip1654Validator {
    rpc_url: String,
    client: Client,
    timeout: Duration,
}

impl RpcEip1654Validator {
    pub fn new(rpc_url: impl Into<String>, client: Client) -> Self {
        Self::with_timeout(rpc_url, client, DEFAULT_RPC_TIMEOUT)
    }

    pub fn with_timeout(rpc_url: impl Into<String>, client: Client, timeout: Duration) -> Self {
        Self {
            rpc_url: rpc_url.into(),
            client,
            timeout,
        }
    }

    async fn eth_call(&self, to: &str, data: &[u8], block: &str) -> Result<String, EthCallError> {
        let call_data = format!("0x{}", hex::encode(data));
        match self
            .rpc(
                "eth_call",
                serde_json::json!([{ "to": to, "data": call_data }, block]),
            )
            .await?
        {
            Value::String(s) => Ok(s),
            Value::Null => Err(failed("RPC returned null result with no error".into())),
            other => Err(failed(format!("Unexpected RPC result type: {other}"))),
        }
    }

    async fn is_valid_signature(
        &self,
        contract_address: &str,
        hash: &[u8],
        signature: &[u8],
        block: &str,
    ) -> Result<bool, AuthError> {
        if hash.len() != 32 {
            return Err(AuthError::Eip1654ValidationFailed(format!(
                "hash must be 32 bytes, got {}",
                hash.len()
            )));
        }

        let calldata = encode_is_valid_signature(hash, signature);
        match self.eth_call(contract_address, &calldata, block).await {
            Ok(result_hex) => Ok(is_magic_return(&result_hex)),
            Err(EthCallError::Reverted) => Ok(false),
            Err(EthCallError::Failed(e)) => Err(e),
        }
    }

    async fn rpc(&self, method: &str, params: Value) -> Result<Value, EthCallError> {
        let body = jsonrpc::request(1, method, params);
        let resp = self
            .client
            .post(&self.rpc_url)
            .timeout(self.timeout)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                #[cfg(test)]
                tests::observe_rpc_send_error(method, &e);
                failed(format!("RPC request failed: {}", e.without_url()))
            })?;
        let status = resp.status().as_u16();
        let bytes = read_body_capped(resp, jsonrpc::DEFAULT_BODY_CAP)
            .await
            .map_err(|e| failed(format!("Failed to read RPC response: {e}")))?;
        jsonrpc::parse_envelope(status, &bytes)
            .and_then(jsonrpc::result_of)
            .map_err(|e| match e {
                jsonrpc::RpcError::Status(status) => failed(format!("RPC returned HTTP {status}")),
                jsonrpc::RpcError::Malformed(m) => {
                    failed(format!("Failed to parse RPC response: {m}"))
                }
                jsonrpc::RpcError::Response { code, message } if is_revert(code, &message) => {
                    EthCallError::Reverted
                }
                jsonrpc::RpcError::Response { code, message } => {
                    failed(format!("RPC error {code}: {message}"))
                }
                other => failed(other.to_string()),
            })
    }
}

fn failed(m: String) -> EthCallError {
    EthCallError::Failed(AuthError::Eip1654ValidationFailed(m))
}

#[async_trait]
impl Eip1654Validator for RpcEip1654Validator {
    fn fresh_validator(&self) -> Option<&dyn Eip1654Validator> {
        Some(self)
    }

    async fn validate_signature(
        &self,
        contract_address: &str,
        hash: &[u8],
        signature: &[u8],
    ) -> Result<bool, AuthError> {
        self.is_valid_signature(contract_address, hash, signature, "latest")
            .await
    }

    /// An RPC that cannot answer for `block` (not an archive node) fails the call:
    /// undecidable, never a refusal.
    async fn validate_signature_at_block(
        &self,
        contract_address: &str,
        hash: &[u8],
        signature: &[u8],
        block: u64,
    ) -> Result<bool, AuthError> {
        self.is_valid_signature(contract_address, hash, signature, &format!("0x{block:x}"))
            .await
    }

    async fn verification_time_block(&self, started_ms: i64) -> Result<u64, AuthError> {
        VerificationBlocks::new(self).select(started_ms).await
    }
}

#[derive(Clone, Copy)]
struct TimedBlock {
    number: f64,
    timestamp: f64,
}

/// Each contract link owns a lookup and its cache. Head timestamp and height are
/// separate observations; a later head shortcut samples the height again.
struct VerificationBlocks<'a> {
    rpc: &'a RpcEip1654Validator,
    latest: Option<TimedBlock>,
    saved: HashMap<u64, TimedBlock>,
    checked: HashSet<u64>,
}

impl<'a> VerificationBlocks<'a> {
    fn new(rpc: &'a RpcEip1654Validator) -> Self {
        Self {
            rpc,
            latest: None,
            saved: HashMap::new(),
            checked: HashSet::new(),
        }
    }

    async fn request(&self, method: &str, params: Value) -> Result<Value, AuthError> {
        self.rpc.rpc(method, params).await.map_err(|e| match e {
            EthCallError::Failed(e) => e,
            EthCallError::Reverted => invalid_block("block lookup reverted"),
        })
    }

    async fn head_number(&self) -> Result<f64, AuthError> {
        block_quantity(
            &self
                .request("eth_blockNumber", serde_json::json!([]))
                .await?,
        )
    }

    async fn block(&mut self, number: Option<f64>) -> Result<TimedBlock, AuthError> {
        // Yield even for cache hits: refinement can otherwise run without polling I/O.
        tokio::task::yield_now().await;
        let key = number.map(block_cache_key);
        if let Some(number) = number {
            if let Some(block) = self.saved.get(&block_cache_key(number)) {
                return Ok(*block);
            }
            if let Some(latest) = self.latest
                && latest.number <= number
            {
                return Ok(latest);
            }
        } else if let Some(latest) = self.latest {
            return Ok(latest);
        }
        // A zero-over-zero refinement produces NaN. Numeric formatting and the
        // returned numeric block both coerce that value to zero, while the cache
        // and visited-candidate keys retain NaN separately from numeric zero.
        let coerced = number.map(|n| if n.is_nan() { 0.0 } else { n });
        let tag = match coerced {
            Some(n) => format!("0x{:x}", block_integer(n)?),
            None => "latest".into(),
        };
        let answer = self
            .request("eth_getBlockByNumber", serde_json::json!([tag, false]))
            .await?;
        let timestamp = block_quantity(&answer["timestamp"])?;
        let number = match coerced {
            Some(number) => number,
            None => self.head_number().await?,
        };
        let block = TimedBlock { number, timestamp };
        if let Some(key) = key {
            self.saved.insert(key, block);
        } else {
            self.latest = Some(block);
        }
        Ok(block)
    }

    async fn select(&mut self, started_ms: i64) -> Result<u64, AuthError> {
        let target = started_ms as f64 / 1000.0;
        let lookup_now = chrono::Utc::now().timestamp_millis() as f64 / 1000.0;
        self.select_at(target, lookup_now).await
    }

    async fn select_at(&mut self, target: f64, lookup_now: f64) -> Result<u64, AuthError> {
        let latest = self.block(None).await?;
        let first = self.block(Some(1.0)).await?;
        let mut block_time = (latest.timestamp - first.timestamp) / latest.number - 1.0;
        if target < first.timestamp {
            return Ok(1);
        }
        if target >= lookup_now || target > latest.timestamp {
            return block_integer(self.head_number().await?);
        }
        let predicted = ((target - first.timestamp / block_time) / 1000.0).ceil();
        let mut current = self.block(Some(predicted)).await?;
        loop {
            if current.timestamp < target {
                let next = self.block(Some(current.number + 1.0)).await?;
                if next.timestamp >= target {
                    return block_integer(current.number);
                }
            }
            let difference = target - current.timestamp;
            let mut skip = (difference / block_time).ceil();
            if skip == 0.0 {
                skip = if difference < 0.0 { -1.0 } else { 1.0 };
            }
            let next_number = loop {
                tokio::task::yield_now().await;
                let next = current.number + skip;
                let key = block_cache_key(next);
                if self.checked.insert(key) {
                    break next;
                }
                let following = skip + if skip < 0.0 { 1.0 } else { -1.0 };
                // No new candidate or external observation can be reached from
                // an unchanged skip, or the visited zero/minus-one oscillation.
                // These are deterministic recursive failures, not work budgets.
                if block_cache_key(following) == block_cache_key(skip)
                    || (skip == 0.0
                        && self
                            .checked
                            .contains(&block_cache_key(current.number - 1.0)))
                    || (skip == -1.0 && self.checked.contains(&block_cache_key(current.number)))
                {
                    return Err(invalid_block(
                        "block refinement cannot reach an unvisited candidate",
                    ));
                }
                skip = following;
            };
            let next = self.block(Some(next_number)).await?;
            block_time =
                ((current.timestamp - next.timestamp) / (current.number - next.number)).abs();
            current = next;
        }
    }
}

fn block_cache_key(number: f64) -> u64 {
    if number.is_nan() {
        f64::NAN.to_bits()
    } else if number == 0.0 {
        0
    } else {
        number.to_bits()
    }
}

fn invalid_block(message: &str) -> AuthError {
    AuthError::Eip1654ValidationFailed(message.into())
}

fn block_integer(number: f64) -> Result<u64, AuthError> {
    if !number.is_finite() || number < 0.0 || number.fract() != 0.0 || number >= u64::MAX as f64 {
        return Err(invalid_block(
            "block number is outside the supported integer range",
        ));
    }
    Ok(number as u64)
}

fn block_quantity(value: &Value) -> Result<f64, AuthError> {
    let text = value
        .as_str()
        .and_then(|s| s.strip_prefix("0x"))
        .ok_or_else(|| invalid_block("block lookup returned no hex quantity"))?;
    let number = u64::from_str_radix(text, 16)
        .map_err(|_| invalid_block("block lookup returned an invalid quantity"))?;
    Ok(number as f64)
}

/// Why `eth_call` gave no result: the contract reverted, which is its answer that the
/// signature is not valid for that hash, or the call itself failed.
enum EthCallError {
    Reverted,
    Failed(AuthError),
}

/// A JSON-RPC error that reports the call's own revert: code 3 with revert data, or the
/// `execution reverted` text nodes send without it.
fn is_revert(code: i64, message: &str) -> bool {
    code == 3 || message.to_ascii_lowercase().contains("revert")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    tokio::task_local! {
        static RPC_WORKLOAD_DIAGNOSTICS: Arc<WorkloadDiagnostics>;
    }

    #[derive(Default)]
    struct WorkloadDiagnostics {
        phase: Value,
        records: Mutex<Vec<Value>>,
        dropped: AtomicUsize,
    }

    impl WorkloadDiagnostics {
        fn observe(&self, method: &str, error: &reqwest::Error) {
            let mut records = self.records.lock().unwrap();
            if records.len() == 8 {
                self.dropped.fetch_add(1, Ordering::SeqCst);
                return;
            }
            let mut record = rpc_send_error_diagnostic(method, error);
            record["phase"] = self.phase.clone();
            records.push(record.clone());
            drop(records);
            println!("RPC_WORKLOAD_SEND_ERROR {record}");
        }
    }

    pub(super) fn observe_rpc_send_error(method: &str, error: &reqwest::Error) {
        let _ = RPC_WORKLOAD_DIAGNOSTICS.try_with(|diagnostics| diagnostics.observe(method, error));
    }

    fn diagnostic_io_causes(error: &(dyn std::error::Error + 'static)) -> Vec<Value> {
        let mut io = Vec::new();
        let mut source = Some(error);
        for _ in 0..16 {
            let Some(error) = source else { break };
            if let Some(error) = error.downcast_ref::<std::io::Error>() {
                io.push(serde_json::json!({
                    "kind": format!("{:?}", error.kind()),
                    "raw_os_error": error.raw_os_error(),
                }));
            }
            source = error.source();
        }
        io
    }

    fn rpc_send_error_diagnostic(method: &str, error: &reqwest::Error) -> Value {
        let method = match method {
            "eth_call" | "eth_getBlockByNumber" | "eth_blockNumber" => method,
            _ => "other",
        };
        serde_json::json!({
            "method": method,
            "is_connect": error.is_connect(),
            "is_timeout": error.is_timeout(),
            "is_request": error.is_request(),
            "io": diagnostic_io_causes(error),
        })
    }

    fn inherited_nofile() -> Value {
        #[cfg(unix)]
        {
            let output = std::process::Command::new("sh")
                .args(["-c", "ulimit -Sn; ulimit -Hn"])
                .output();
            if let Ok(output) = output
                && output.status.success()
                && let Ok(text) = std::str::from_utf8(&output.stdout)
            {
                let limits = text.lines().map(str::trim).collect::<Vec<_>>();
                if limits.len() == 2
                    && limits
                        .iter()
                        .all(|limit| *limit == "unlimited" || limit.parse::<u64>().is_ok())
                {
                    return serde_json::json!({"soft": limits[0], "hard": limits[1]});
                }
            }
            serde_json::json!({"status": "unavailable"})
        }
        #[cfg(not(unix))]
        serde_json::json!({"status": "inapplicable", "os": std::env::consts::OS})
    }

    fn descriptor_count() -> Value {
        #[cfg(unix)]
        {
            let path = if cfg!(target_os = "linux") {
                "/proc/self/fd"
            } else {
                "/dev/fd"
            };
            if let Ok(mut entries) = std::fs::read_dir(path) {
                let count = entries.try_fold(0usize, |count, entry| entry.map(|_| count + 1));
                if let Ok(count) = count {
                    return serde_json::json!({"count_including_directory_reader": count});
                }
            }
            serde_json::json!({"status": "unavailable"})
        }
        #[cfg(not(unix))]
        serde_json::json!({"status": "inapplicable", "os": std::env::consts::OS})
    }

    #[tokio::test]
    async fn workload_transport_diagnostics_keep_io_causes_without_endpoint_data() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let reservation = Arc::new(tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap());
        let endpoint = reservation.local_addr().unwrap();
        let competitor = tokio::net::TcpSocket::new_v4().unwrap();
        assert!(
            competitor.bind(endpoint).is_err(),
            "the failing endpoint must stay reserved throughout both sends"
        );
        let listener = reservation.clone();
        let task = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut header = [0; 5];
                stream.read_exact(&mut header).await.unwrap();
                assert_eq!(header[0], 22, "expected a TLS handshake record");
                let mut hello = vec![0; u16::from_be_bytes([header[3], header[4]]) as usize];
                stream.read_exact(&mut hello).await.unwrap();
                // A fatal TLS access_denied alert fails the real handshake without a
                // certificate or a platform-dependent non-listening connect timeout.
                stream.write_all(&[21, 3, 3, 0, 2, 2, 49]).await.unwrap();
                stream.shutdown().await.unwrap();
            }
        });
        let url = format!("https://{endpoint}/SECRET?token=PRIVATE");
        let error = Client::new()
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .unwrap_err();
        assert!(error.url().is_some());
        let record = rpc_send_error_diagnostic("eth_call", &error);
        assert_eq!(record["method"], "eth_call");
        assert_eq!(record["is_connect"], true);
        assert_eq!(record["is_request"], true);
        assert_eq!(record["is_timeout"], false);
        assert!(!record["io"].as_array().unwrap().is_empty());
        let text = record.to_string();
        for secret in ["SECRET", "PRIVATE", "127.0.0.1", "http", "token"] {
            assert!(!text.contains(secret), "diagnostic exposed endpoint data");
        }
        assert_eq!(rpc_send_error_diagnostic(&url, &error)["method"], "other");

        #[derive(Debug)]
        struct NestedIo(std::io::Error);
        impl std::fmt::Display for NestedIo {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("SECRET endpoint and PRIVATE input")
            }
        }
        impl std::error::Error for NestedIo {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                Some(&self.0)
            }
        }
        let nested = NestedIo(std::io::Error::from_raw_os_error(24));
        let io = diagnostic_io_causes(&nested);
        assert_eq!(io.len(), 1);
        assert_eq!(io[0]["raw_os_error"], 24);
        assert!(io[0]["kind"].is_string());
        assert!(!serde_json::to_string(&io).unwrap().contains("SECRET"));

        let diagnostics = Arc::new(WorkloadDiagnostics {
            phase: serde_json::json!({"test": "redaction_regression"}),
            ..Default::default()
        });
        let rpc = RpcEip1654Validator::with_timeout(&url, Client::new(), Duration::from_secs(5));
        let result = RPC_WORKLOAD_DIAGNOSTICS
            .scope(
                diagnostics.clone(),
                rpc.rpc("eth_call", serde_json::json!([])),
            )
            .await;
        let Err(EthCallError::Failed(sanitized)) = result else {
            panic!("expected the installed sanitized transport error");
        };
        assert!(sanitized.to_string().contains("RPC request failed:"));
        assert!(!sanitized.to_string().contains("SECRET"));
        assert!(!sanitized.to_string().contains("127.0.0.1"));
        assert_eq!(diagnostics.records.lock().unwrap().len(), 1);
        assert_eq!(diagnostics.records.lock().unwrap()[0]["is_connect"], true);
        assert_eq!(diagnostics.records.lock().unwrap()[0]["is_request"], true);
        assert_eq!(diagnostics.records.lock().unwrap()[0]["is_timeout"], false);
        assert!(
            !diagnostics.records.lock().unwrap()[0]["io"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            diagnostics.records.lock().unwrap()[0]["phase"]["test"],
            "redaction_regression"
        );
        observe_rpc_send_error("eth_call", &error);
        assert_eq!(diagnostics.records.lock().unwrap().len(), 1);
        RPC_WORKLOAD_DIAGNOSTICS
            .scope(diagnostics.clone(), async {
                for _ in 0..10 {
                    observe_rpc_send_error("eth_call", &error);
                }
            })
            .await;
        assert_eq!(diagnostics.records.lock().unwrap().len(), 8);
        assert_eq!(diagnostics.dropped.load(Ordering::SeqCst), 3);
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap();
        drop(reservation);
    }

    async fn scripted_rpc(
        answers: Vec<Value>,
    ) -> (
        RpcEip1654Validator,
        Arc<Mutex<Vec<Value>>>,
        tokio::task::JoinHandle<()>,
    ) {
        scripted_rpc_held(answers, None).await
    }

    async fn scripted_rpc_held(
        answers: Vec<Value>,
        hold: Option<(
            usize,
            Arc<tokio::sync::Semaphore>,
            Arc<tokio::sync::Semaphore>,
        )>,
    ) -> (
        RpcEip1654Validator,
        Arc<Mutex<Vec<Value>>>,
        tokio::task::JoinHandle<()>,
    ) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let recorded = calls.clone();
        let app = axum::Router::new().route("/", axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
            let calls = recorded.clone();
            let answers = answers.clone();
                    let hold=hold.clone();
            async move {
                        let index={let mut calls=calls.lock().unwrap();let index=calls.len();calls.push(body.clone());index};
                        if let Some((at,entered,release))=hold && at==index {entered.add_permits(1);release.acquire().await.unwrap().forget();}
                let answer = answers.get(index).cloned().unwrap_or(serde_json::json!({"error":{"code":-1,"message":"unexpected extra request"}}));
                let mut answer = answer;
                answer["jsonrpc"] = "2.0".into();
                answer["id"] = body["id"].clone();
                axum::Json(answer)
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (RpcEip1654Validator::new(url, Client::new()), calls, task)
    }

    fn result(value: Value) -> Value {
        serde_json::json!({"result":value})
    }
    fn block_answer(timestamp: u64) -> Value {
        result(serde_json::json!({"number":"0x5a","timestamp":format!("0x{timestamp:x}")}))
    }
    fn refusal() -> Value {
        result(serde_json::json!(format!("0x{}", "0".repeat(64))))
    }
    fn magic() -> Value {
        result(serde_json::json!(format!("0x1626ba7e{}", "0".repeat(56))))
    }
    fn rpc_error() -> Value {
        serde_json::json!({"error":{"code":-32000,"message":"controlled failure"}})
    }

    #[tokio::test]
    async fn verification_selector_preserves_order_shortcuts_equality_and_fractional_time() {
        // Literal request traces captured independently at the JSON-RPC boundary.
        for (target, clock, head, heights, tags, selected) in [
            (
                115.0,
                200.0,
                110,
                vec![10, 11],
                vec!["latest", "N", "0x1", "N"],
                11,
            ),
            (19.0, 200.0, 110, vec![10], vec!["latest", "N", "0x1"], 1),
            (
                55.0,
                200.0,
                110,
                vec![10],
                vec!["latest", "N", "0x1", "0x2", "0x6", "0x5", "0x4"],
                4,
            ),
            (
                60.0,
                200.0,
                110,
                vec![10],
                vec!["latest", "N", "0x1", "0x2", "0x6", "0x5", "0x4"],
                4,
            ),
            (
                60.001,
                200.0,
                110,
                vec![10],
                vec!["latest", "N", "0x1", "0x2", "0x7", "0x6", "0x5"],
                5,
            ),
            (
                110.0,
                200.0,
                110,
                vec![10],
                vec!["latest", "N", "0x1", "0x2", "0x0", "0x9"],
                9,
            ),
            (
                115.0,
                115.0,
                130,
                vec![10, 11],
                vec!["latest", "N", "0x1", "N"],
                11,
            ),
            (
                20.0,
                200.0,
                110,
                vec![10],
                vec!["latest", "N", "0x1", "0x2", "0x0"],
                0,
            ),
        ] {
            let mut heights = heights.into_iter();
            let answers = tags
                .iter()
                .map(|tag| {
                    if *tag == "N" {
                        result(serde_json::json!(format!(
                            "0x{:x}",
                            heights.next().unwrap()
                        )))
                    } else {
                        block_answer(if *tag == "latest" {
                            head
                        } else {
                            10 * u64::from_str_radix(&tag[2..], 16).unwrap() + 10
                        })
                    }
                })
                .collect();
            let (rpc, calls, task) = scripted_rpc(answers).await;
            let mut selector = VerificationBlocks::new(&rpc);
            assert_eq!(
                selector.select_at(target, clock).await.unwrap(),
                selected,
                "target {target}"
            );
            let calls = calls.lock().unwrap();
            assert_eq!(calls.len(), tags.len(), "target {target}");
            for (call, tag) in calls.iter().zip(tags) {
                if tag == "N" {
                    assert_eq!(call["method"], "eth_blockNumber");
                    assert_eq!(call["params"], serde_json::json!([]));
                } else {
                    assert_eq!(call["method"], "eth_getBlockByNumber");
                    assert_eq!(call["params"], serde_json::json!([tag, false]));
                }
            }
            println!(
                "SELECTOR target={target} block={selected} calls={} saved={} checked={} entry_payload_bytes={}",
                calls.len(),
                selector.saved.len() + 1,
                selector.checked.len(),
                (selector.saved.len() + 1) * std::mem::size_of::<TimedBlock>()
                    + selector.checked.len() * 8
            );
            println!(
                "SELECTOR_RPC_TRACE {}",
                serde_json::json!({"target":target,"clock":clock,"head":head,"selected":selected,"calls":*calls})
            );
            task.abort();
        }
    }

    #[tokio::test]
    async fn complete_latest_pair_precedes_lookup_and_complete_history_pair() {
        use crate::eip1654::verify_eip1654_at_verification_time;
        let message = b"dcl-controlled-fallback";
        let contract = "0x0000000000000000000000000000000000000001";
        for (fixture, (latest, historical, accepted, expected_calls)) in [
            (vec![magic()], vec![], true, 1),
            (vec![rpc_error(), magic()], vec![], true, 2),
            (vec![refusal(), refusal()], vec![magic()], true, 7),
            (
                vec![rpc_error(), rpc_error()],
                vec![rpc_error(), magic()],
                true,
                8,
            ),
            (
                vec![refusal(), rpc_error()],
                vec![refusal(), rpc_error()],
                false,
                8,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let latest_count = latest.len();
            let mut answers = latest;
            if !historical.is_empty() {
                answers.extend([
                    block_answer(110),
                    result(serde_json::json!("0xa")),
                    block_answer(20),
                    result(serde_json::json!("0xb")),
                ]);
                answers.extend(historical);
            }
            let (rpc, calls, task) = scripted_rpc(answers).await;
            assert_eq!(
                matches!(
                    verify_eip1654_at_verification_time(
                        &rpc,
                        contract,
                        message,
                        &[0x12, 0x34],
                        115000
                    )
                    .await,
                    Ok(true)
                ),
                accepted
            );
            let calls = calls.lock().unwrap();
            assert_eq!(calls.len(), expected_calls);
            for (index, call) in calls
                .iter()
                .enumerate()
                .filter(|(_, c)| c["method"] == "eth_call")
            {
                let raw = index == 0 || index == latest_count + 4;
                let hash = if raw {
                    alloy_primitives::keccak256(message)
                } else {
                    alloy_primitives::eip191_hash_message(message)
                };
                // ABI layout asserted independently of encode_is_valid_signature.
                let data = format!(
                    "0x1626ba7e{}{}{}1234{}",
                    hex::encode(hash),
                    format!("{:064x}", 64),
                    format!("{:064x}", 2),
                    "0".repeat(60)
                );
                let block = if index < latest_count {
                    "latest"
                } else {
                    "0xb"
                };
                assert_eq!(
                    call["params"],
                    serde_json::json!([{"to":contract,"data":data},block])
                );
            }
            println!(
                "MESSAGE_RPC_TRACE {}",
                serde_json::json!({"fixture":fixture,"accepted":accepted,"calls":*calls})
            );
            task.abort();
        }
    }

    #[tokio::test]
    async fn selector_errors_stop_before_historical_signature_and_ordinary_calls_stay_latest() {
        use crate::eip1654::{verify_eip1654, verify_eip1654_at_verification_time};
        for failure_at in 0..4 {
            let mut answers = vec![
                refusal(),
                rpc_error(),
                block_answer(110),
                result(serde_json::json!("0xa")),
                block_answer(20),
                result(serde_json::json!("0xb")),
            ];
            answers[2 + failure_at] = rpc_error();
            answers.truncate(3 + failure_at);
            let (rpc, calls, task) = scripted_rpc(answers).await;
            assert!(
                verify_eip1654_at_verification_time(&rpc, "0xabc", b"message", &[1], 115000)
                    .await
                    .is_err()
            );
            assert_eq!(calls.lock().unwrap().len(), 3 + failure_at);
            task.abort();
        }
        let (rpc, calls, task) = scripted_rpc(vec![refusal(), refusal(), magic()]).await;
        assert!(
            !verify_eip1654(&rpc, "0xabc", b"message", &[1])
                .await
                .unwrap()
        );
        assert_eq!(calls.lock().unwrap().len(), 2);
        task.abort();
    }

    fn historical_chain(links: usize, payload: &str) -> crate::AuthChain {
        let mut chain = vec![
            serde_json::json!({"type":"SIGNER","payload":"0x0000000000000000000000000000000000000001","signature":""}),
        ];
        for n in 1..links {
            chain.push(serde_json::json!({"type":"ECDSA_EIP_1654_EPHEMERAL","payload":format!("Decentraland Login\nEphemeral address: 0x{:040x}\nExpiration: 2099-12-31T00:00:00.000Z",n+1),"signature":"0x1234"}));
        }
        chain.push(serde_json::json!({"type":"ECDSA_EIP_1654_SIGNED_ENTITY","payload":payload,"signature":"0x1234"}));
        serde_json::from_value(serde_json::json!(chain)).unwrap()
    }

    #[tokio::test]
    async fn every_contract_link_uses_same_start_time_and_its_own_fresh_lookup() {
        use crate::verify::verify_auth_chain_at_verification_time;
        for links in 1..=9 {
            // Interior lookup, then raw historical refusal and prefixed historical success.
            let per_link = vec![
                refusal(),
                rpc_error(),
                block_answer(110),
                result(serde_json::json!("0xa")),
                block_answer(20),
                block_answer(30),
                block_answer(70),
                block_answer(60),
                block_answer(50),
                rpc_error(),
                magic(),
            ];
            let (rpc, calls, task) =
                scripted_rpc((0..links).flat_map(|_| per_link.iter().cloned()).collect()).await;
            let started = std::time::Instant::now();
            verify_auth_chain_at_verification_time(
                &historical_chain(links, "challenge"),
                "challenge",
                55000,
                Some(&rpc),
            )
            .await
            .unwrap();
            let elapsed = started.elapsed();
            let calls = calls.lock().unwrap();
            assert_eq!(calls.len(), 11 * links);
            for (index, link_calls) in calls.chunks_exact(11).enumerate() {
                assert_eq!(link_calls[0]["params"][1], "latest");
                assert_eq!(
                    link_calls[0]["params"][0]["to"],
                    format!("0x{:040x}", index + 1)
                );
                assert_eq!(
                    link_calls[2]["params"],
                    serde_json::json!(["latest", false])
                );
                assert_eq!(link_calls[8]["params"], serde_json::json!(["0x4", false]));
                assert_eq!(link_calls[9]["params"][1], "0x4");
                assert_eq!(link_calls[10]["params"][1], "0x4");
            }
            println!(
                "FALLBACK links={links} calls={} signature_calls={} selector_calls={} elapsed_us={} sequential_per_handshake=true",
                calls.len(),
                4 * links,
                7 * links,
                elapsed.as_micros()
            );
            task.abort();
        }
    }

    #[tokio::test]
    async fn fresh_fallback_bypasses_nested_success_cache_without_changing_ordinary_callers() {
        use crate::verify::{verify_auth_chain_async, verify_auth_chain_at_verification_time};
        use crate::{Eip1654Validator, ValidationCache};
        let answers = vec![
            magic(),
            refusal(),
            rpc_error(),
            block_answer(110),
            result(serde_json::json!("0xa")),
            block_answer(20),
            result(serde_json::json!("0xb")),
            refusal(),
            refusal(),
        ];
        let (rpc, calls, task) = scripted_rpc(answers).await;
        let cache = ValidationCache::new(Arc::new(ValidationCache::new(Arc::new(rpc))));
        let chain = historical_chain(1, "reused-delegation");
        verify_auth_chain_async(&chain, "reused-delegation", Some(115000), Some(&cache))
            .await
            .unwrap();
        assert!(
            verify_auth_chain_at_verification_time(
                &chain,
                "reused-delegation",
                115000,
                cache.fresh_validator()
            )
            .await
            .is_err()
        );
        assert_eq!(calls.lock().unwrap().len(), 9);
        verify_auth_chain_async(&chain, "reused-delegation", Some(115000), Some(&cache))
            .await
            .unwrap();
        assert_eq!(
            calls.lock().unwrap().len(),
            9,
            "ordinary positive cache remains in effect"
        );
        task.abort();
    }

    #[tokio::test]
    async fn late_bad_link_wrong_payload_and_malformed_signature_never_accept_partial_history() {
        use crate::verify::verify_auth_chain_at_verification_time;
        let mut answers = vec![
            refusal(),
            refusal(),
            block_answer(110),
            result(serde_json::json!("0xa")),
            block_answer(20),
            result(serde_json::json!("0xb")),
            magic(),
        ];
        answers.extend([
            refusal(),
            refusal(),
            block_answer(110),
            result(serde_json::json!("0xa")),
            block_answer(20),
            result(serde_json::json!("0xb")),
            refusal(),
            refusal(),
        ]);
        let (rpc, calls, task) = scripted_rpc(answers).await;
        assert!(
            verify_auth_chain_at_verification_time(
                &historical_chain(2, "challenge"),
                "challenge",
                115000,
                Some(&rpc)
            )
            .await
            .is_err()
        );
        assert_eq!(calls.lock().unwrap().len(), 15);
        task.abort();
        let (rpc, calls, task) = scripted_rpc(vec![magic()]).await;
        let mut chain = historical_chain(1, "challenge");
        chain[1].signature = Some("0xZZ".into());
        assert!(
            verify_auth_chain_at_verification_time(&chain, "challenge", 115000, Some(&rpc))
                .await
                .is_err()
        );
        assert!(calls.lock().unwrap().is_empty());
        chain[1].signature = Some("0x1234".into());
        assert!(
            verify_auth_chain_at_verification_time(&chain, "different", 115000, Some(&rpc))
                .await
                .is_err()
        );
        assert_eq!(calls.lock().unwrap().len(), 1);
        task.abort();
    }

    #[tokio::test]
    async fn repeated_nan_candidate_rejects_the_pure_recursive_failure() {
        use alloy::signers::{SignerSync, local::PrivateKeySigner};
        let (rpc, calls, task) = scripted_rpc(vec![
            block_answer(110),
            result(serde_json::json!("0xa")),
            block_answer(20),
            block_answer(30),
            block_answer(110),
        ])
        .await;
        let mut selector = VerificationBlocks::new(&rpc);
        let mut future = Box::pin(selector.select_at(110.0, 200.0));
        let wallet = PrivateKeySigner::random();
        let chain:crate::AuthChain=serde_json::from_value(serde_json::json!([
            {"type":"SIGNER","payload":format!("{:#x}",wallet.address()),"signature":""},
            {"type":"ECDSA_SIGNED_ENTITY","payload":"challenge","signature":wallet.sign_message_sync(b"challenge").unwrap().to_string()}
        ])).unwrap();
        let start = std::time::Instant::now();
        crate::verify_auth_chain_async(&chain, "challenge", Some(110000), None)
            .await
            .unwrap();
        let eoa_us = start.elapsed().as_micros();
        assert!(
            matches!(tokio::time::timeout(Duration::from_secs(1),&mut future).await.unwrap(),Err(AuthError::Eip1654ValidationFailed(message)) if message.contains("unvisited candidate"))
        );
        let dropped = std::time::Instant::now();
        drop(future);
        let drop_us = dropped.elapsed().as_micros();
        assert_eq!(calls.lock().unwrap().len(), 5);
        println!(
            "PURE_RECURSION_REJECT eoa_us={eoa_us} drop_us={drop_us} saved={} checked={} calls=5",
            selector.saved.len() + 1,
            selector.checked.len()
        );
        task.abort();
    }

    #[tokio::test]
    async fn successful_cached_refinement_yields_and_can_be_dropped() {
        use alloy::signers::{SignerSync, local::PrivateKeySigner};
        let (rpc, calls, task) = scripted_rpc(Vec::new()).await;
        let prefilled = || {
            let mut selector = VerificationBlocks::new(&rpc);
            selector.latest = Some(TimedBlock {
                number: 10.0,
                timestamp: 110.0,
            });
            for (number, timestamp) in [(1, 20.0), (2, 30.0), (0, 10.0), (9, 100.0)] {
                selector.saved.insert(
                    block_cache_key(number as f64),
                    TimedBlock {
                        number: number as f64,
                        timestamp,
                    },
                );
            }
            // NaN has a distinct cache key even though its fetched block is zero.
            selector.saved.insert(
                block_cache_key(f64::NAN),
                TimedBlock {
                    number: 0.0,
                    timestamp: 10.0,
                },
            );
            selector
        };
        let mut successful = prefilled();
        assert_eq!(successful.select_at(110.0, 200.0).await.unwrap(), 9);
        let wallet = PrivateKeySigner::random();
        let chain:crate::AuthChain=serde_json::from_value(serde_json::json!([
            {"type":"SIGNER","payload":format!("{:#x}",wallet.address()),"signature":""},
            {"type":"ECDSA_SIGNED_ENTITY","payload":"challenge","signature":wallet.sign_message_sync(b"challenge").unwrap().to_string()}
        ])).unwrap();
        let mut observed_refinement = false;
        // Search suspension checkpoints rather than assuming an exact poll count.
        // This bounds the fixture search, not the selector's work or acceptance.
        for poll_budget in 1..=64 {
            let mut selector = prefilled();
            let mut future = Box::pin(selector.select_at(110.0, 200.0));
            let mut completed = false;
            for _ in 0..poll_budget {
                if let std::task::Poll::Ready(result) = futures::poll!(&mut future) {
                    assert_eq!(result.unwrap(), 9);
                    completed = true;
                    break;
                }
            }
            if completed {
                continue;
            }
            let started = std::time::Instant::now();
            crate::verify_auth_chain_async(&chain, "challenge", Some(110000), None)
                .await
                .unwrap();
            let eoa_us = started.elapsed().as_micros();
            let dropped = std::time::Instant::now();
            drop(future);
            let drop_us = dropped.elapsed().as_micros();
            if selector.checked.is_empty() {
                continue;
            }
            observed_refinement = true;
            println!(
                "CACHED_REFINEMENT_CHECKPOINT poll_budget={poll_budget} checked={} saved={} eoa_us={eoa_us} drop_us={drop_us} successful_block=9",
                selector.checked.len(),
                selector.saved.len() + 1,
            );
            break;
        }
        assert!(
            observed_refinement,
            "no suspension after checked-candidate refinement progress"
        );
        assert!(
            calls.lock().unwrap().is_empty(),
            "prefilled refinement emitted RPC"
        );
        task.abort();
    }

    #[tokio::test]
    async fn refinement_read_cancellation_drops_work_without_a_following_rpc() {
        let entered = Arc::new(tokio::sync::Semaphore::new(0));
        let release = Arc::new(tokio::sync::Semaphore::new(0));
        let (rpc, calls, task) = scripted_rpc_held(
            vec![
                block_answer(110),
                result(serde_json::json!("0xa")),
                block_answer(20),
                block_answer(30),
            ],
            Some((3, entered.clone(), release.clone())),
        )
        .await;
        let mut selector = VerificationBlocks::new(&rpc);
        let mut future = Box::pin(selector.select_at(55.0, 200.0));
        tokio::select! {result=&mut future=>panic!("refinement must hold: {result:?}"),permit=entered.acquire()=>permit.unwrap().forget()};
        let started = std::time::Instant::now();
        // Shared walker work for an unrelated EOA chain remains runnable.
        use alloy::signers::{SignerSync, local::PrivateKeySigner};
        let wallet = PrivateKeySigner::random();
        let chain:crate::AuthChain=serde_json::from_value(serde_json::json!([
            {"type":"SIGNER","payload":format!("{:#x}",wallet.address()),"signature":""},
            {"type":"ECDSA_SIGNED_ENTITY","payload":"challenge","signature":wallet.sign_message_sync(b"challenge").unwrap().to_string()}
        ])).unwrap();
        crate::verify_auth_chain_async(&chain, "challenge", Some(55000), None)
            .await
            .unwrap();
        let eoa_us = started.elapsed().as_micros();
        let dropped = std::time::Instant::now();
        drop(future);
        let drop_us = dropped.elapsed().as_micros();
        release.add_permits(1);
        tokio::time::sleep(Duration::from_millis(15)).await;
        assert_eq!(calls.lock().unwrap().len(), 4);
        println!("REFINEMENT_CANCEL eoa_us={eoa_us} drop_us={drop_us} calls=4");
        task.abort();
    }

    #[tokio::test]
    async fn controlled_rpc_workload_measures_latest_and_fallback_at_the_verification_envelope() {
        use crate::verify::verify_auth_chain_at_verification_time;
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        println!(
            "RPC_WORKLOAD_ENV {}",
            serde_json::json!({
                "os": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
                "runtime": "current_thread",
                "client": "reqwest_default",
                "rpc_timeout_ms": DEFAULT_RPC_TIMEOUT.as_millis(),
                "phase_timeout_ms": 90_000,
                "nofile": inherited_nofile(),
                "accept_error_observation": "unavailable_no_scoped_subscriber",
            })
        );
        let fallback = Arc::new(AtomicBool::new(false));
        let calls = Arc::new(AtomicUsize::new(0));
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let (mode, count, running, maximum) = (
            fallback.clone(),
            calls.clone(),
            active.clone(),
            peak.clone(),
        );
        let app = axum::Router::new().route(
            "/",
            axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
                let (mode, count, running, maximum) = (
                    mode.clone(),
                    count.clone(),
                    running.clone(),
                    maximum.clone(),
                );
                async move {
                    struct Active(Arc<AtomicUsize>);
                    impl Drop for Active {
                        fn drop(&mut self) {
                            self.0.fetch_sub(1, Ordering::SeqCst);
                        }
                    }
                    let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                    let _guard = Active(running);
                    maximum.fetch_max(now, Ordering::SeqCst);
                    count.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(1)).await;
                    let answer = match body["method"].as_str().unwrap() {
                        "eth_call"
                            if mode.load(Ordering::SeqCst) && body["params"][1] == "latest" =>
                        {
                            refusal()
                        }
                        "eth_call" => magic(),
                        "eth_getBlockByNumber" => block_answer(20),
                        "eth_blockNumber" => result(serde_json::json!("0xb")),
                        method => panic!("unexpected method {method}"),
                    };
                    let mut answer = answer;
                    answer["id"] = body["id"].clone();
                    answer["jsonrpc"] = "2.0".into();
                    axum::Json(answer)
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let rpc = RpcEip1654Validator::new(
            format!("http://{}/", listener.local_addr().unwrap()),
            Client::new(),
        );
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        for (links, concurrency) in (1..=9).map(|k| (k, 32)).chain(std::iter::once((9, 3000))) {
            for historical in [false, true] {
                fallback.store(historical, Ordering::SeqCst);
                calls.store(0, Ordering::SeqCst);
                peak.store(0, Ordering::SeqCst);
                let chain = historical_chain(links, "challenge");
                let diagnostics = Arc::new(WorkloadDiagnostics {
                    phase: serde_json::json!({
                        "test": "verification_envelope",
                        "links": links,
                        "concurrent_verifications": concurrency,
                        "fallback": historical,
                    }),
                    ..Default::default()
                });
                println!(
                    "RPC_WORKLOAD_BEGIN {}",
                    serde_json::json!({
                        "links": links,
                        "concurrent_verifications": concurrency,
                        "fallback": historical,
                        "provider_delay_ms": 1,
                        "descriptors": descriptor_count(),
                    })
                );
                let wall = std::time::Instant::now();
                let mut latencies = RPC_WORKLOAD_DIAGNOSTICS.scope(diagnostics.clone(), tokio::time::timeout(
                    Duration::from_secs(90),
                    futures::future::join_all((0..concurrency).map(|verification| {
                        let rpc = &rpc;
                        let chain = &chain;
                        let diagnostics = &diagnostics;
                        let (calls, active, peak) = (&calls, &active, &peak);
                        async move {
                            let started = std::time::Instant::now();
                            verify_auth_chain_at_verification_time(
                                chain,
                                "challenge",
                                115000,
                                Some(rpc),
                            )
                            .await
                            .unwrap_or_else(|_| panic!(
                                "RPC_WORKLOAD_FAILURE links={links} concurrent_verifications={concurrency} fallback={historical} verification={verification} calls={} active={} peak={} diagnostics={} dropped_diagnostics={}",
                                calls.load(Ordering::SeqCst),
                                active.load(Ordering::SeqCst),
                                peak.load(Ordering::SeqCst),
                                diagnostics.records.lock().unwrap().len(),
                                diagnostics.dropped.load(Ordering::SeqCst),
                            ));
                            started.elapsed().as_micros()
                        }
                    })),
                ))
                .await
                .unwrap();
                latencies.sort_unstable();
                let total = calls.load(Ordering::SeqCst);
                assert_eq!(total, concurrency * links * if historical { 7 } else { 1 });
                assert_eq!(active.load(Ordering::SeqCst), 0);
                assert!(peak.load(Ordering::SeqCst) <= concurrency);
                println!(
                    "RPC_WORKLOAD links={links} concurrent_verifications={concurrency} fallback={historical} calls={total} peak_requests={} p50_us={} p95_us={} p99_us={} wall_ms={} provider_delay_ms=1",
                    peak.load(Ordering::SeqCst),
                    latencies[concurrency / 2],
                    latencies[concurrency * 95 / 100],
                    latencies[concurrency * 99 / 100],
                    wall.elapsed().as_millis()
                );
                println!(
                    "RPC_WORKLOAD_DIAGNOSTICS links={links} concurrent_verifications={concurrency} fallback={historical} captured={} dropped={}",
                    diagnostics.records.lock().unwrap().len(),
                    diagnostics.dropped.load(Ordering::SeqCst),
                );
            }
        }
        task.abort();
    }

    #[tokio::test]
    async fn selector_degenerate_and_provider_number_cases_fail_closed_or_preserve_selection() {
        for (target, answers, expected, count) in [
            (
                22.0,
                vec![
                    block_answer(25),
                    result(serde_json::json!("0xa")),
                    block_answer(20),
                    block_answer(30),
                ],
                Some(1),
                4,
            ),
            (
                60.0,
                vec![
                    block_answer(110),
                    result(serde_json::json!("0xa")),
                    block_answer(20),
                    block_answer(30),
                    block_answer(70),
                    block_answer(50),
                ],
                Some(5),
                6,
            ),
            (
                25.0,
                vec![
                    block_answer(30),
                    result(serde_json::json!("0xa")),
                    block_answer(20),
                ],
                None,
                3,
            ),
            (
                115.0,
                vec![
                    block_answer(110),
                    result(serde_json::json!("0x20000000000001")),
                    block_answer(20),
                    result(serde_json::json!("0x20000000000001")),
                ],
                Some(9_007_199_254_740_992),
                4,
            ),
            (115.0, vec![result(Value::Null)], None, 1),
            (
                115.0,
                vec![
                    block_answer(110),
                    result(serde_json::json!("0x10000000000000000")),
                ],
                None,
                2,
            ),
        ] {
            let (rpc, calls, task) = scripted_rpc(answers).await;
            let selected = VerificationBlocks::new(&rpc).select_at(target, 200.0).await;
            assert_eq!(selected.ok(), expected);
            assert_eq!(calls.lock().unwrap().len(), count);
            task.abort();
        }
    }

    #[test]
    fn encode_roundtrip() {
        let hash = [0xABu8; 32];
        let sig = vec![0x01, 0x02, 0x03];

        let encoded = encode_is_valid_signature(&hash, &sig);

        assert_eq!(encoded.len(), 4 + 32 + 32 + 32 + 32);

        assert_eq!(&encoded[0..4], &IS_VALID_SIGNATURE_SELECTOR);

        assert_eq!(&encoded[4..36], &hash);

        assert_eq!(encoded[35 + 32], 0x40);

        assert_eq!(encoded[4 + 32 + 32 + 31], 3);

        assert_eq!(&encoded[4 + 32 + 32 + 32..4 + 32 + 32 + 32 + 3], &[1, 2, 3]);

        assert!(encoded[4 + 32 + 32 + 32 + 3..].iter().all(|&b| b == 0));
    }

    #[test]
    fn encode_exact_32_byte_sig() {
        let hash = [0x00u8; 32];
        let sig = [0xFFu8; 32];

        let encoded = encode_is_valid_signature(&hash, &sig);

        assert_eq!(encoded.len(), 132);
    }

    #[test]
    fn encode_65_byte_sig() {
        let hash = [0x11u8; 32];
        let sig = [0xAAu8; 65];

        let encoded = encode_is_valid_signature(&hash, &sig);

        assert_eq!(encoded.len(), 4 + 32 + 32 + 32 + 96);

        assert_eq!(encoded[4 + 32 + 32 + 31], 65);
    }

    #[test]
    fn magic_value_detected() {
        let hex = "0x1626ba7e00000000000000000000000000000000000000000000000000000000";
        assert!(is_magic_return(hex));

        let hex = "1626ba7e00000000000000000000000000000000000000000000000000000000";
        assert!(is_magic_return(hex));
    }

    #[test]
    fn wrong_magic_rejected() {
        let hex = "0xffffffff00000000000000000000000000000000000000000000000000000000";
        assert!(!is_magic_return(hex));

        assert!(!is_magic_return("0x1626ba"));

        assert!(!is_magic_return("0x"));
    }

    #[test]
    fn zero_return_rejected() {
        let hex = "0x0000000000000000000000000000000000000000000000000000000000000000";
        assert!(!is_magic_return(hex));
    }

    #[tokio::test]
    async fn a_transport_failure_never_echoes_the_endpoint() {
        let v = RpcEip1654Validator::with_timeout(
            "https://127.0.0.1:9/v3/SECRET-API-KEY",
            Client::new(),
            Duration::from_secs(5),
        );
        let err = v
            .validate_signature("0xabc", &[0u8; 32], &[0u8; 65])
            .await
            .unwrap_err();
        let text = err.to_string();
        assert!(text.contains("RPC request failed"), "{text}");
        assert!(!text.contains("SECRET"), "{text}");
        assert!(!text.contains("127.0.0.1"), "{text}");
    }

    /// A JSON-RPC node on loopback that answers `isValidSignature` with the magic value for
    /// `accepted`, reverts for every other hash, and fails with HTTP 500 when `accepted` is
    /// `None`.
    async fn fake_wallet(accepted: Option<[u8; 32]>) -> String {
        use axum::{Json, Router, http::StatusCode, response::IntoResponse, routing::post};
        let app = Router::new().route(
            "/",
            post(move |Json(body): Json<Value>| async move {
                let Some(accepted) = accepted else {
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                };
                let data = body["params"][0]["data"].as_str().unwrap_or_default();
                let answer = if data.contains(&hex::encode(accepted)) {
                    serde_json::json!({ "result": format!("0x1626ba7e{}", "0".repeat(56)) })
                } else {
                    serde_json::json!({ "error": { "code": 3, "message": "execution reverted" } })
                };
                let mut answer = answer;
                answer["jsonrpc"] = "2.0".into();
                answer["id"] = 1.into();
                Json(answer).into_response()
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        url
    }

    /// A contract that reverts for a hash it was not signed over is answering "not valid"
    /// for that hash; the EIP-191 hash is still asked, as upstream does.
    #[tokio::test]
    async fn a_revert_is_a_rejection_and_the_other_hash_is_still_asked() {
        let message = b"Decentraland Login";
        let prefixed = alloy_primitives::eip191_hash_message(message).0;
        let v = RpcEip1654Validator::new(fake_wallet(Some(prefixed)).await, Client::new());
        let raw = alloy_primitives::keccak256(message);
        assert!(
            !v.validate_signature("0xabc", raw.as_slice(), &[0u8; 65])
                .await
                .unwrap()
        );
        assert!(
            super::super::eip1654::verify_eip1654(&v, "0xabc", message, &[0u8; 65])
                .await
                .unwrap()
        );
    }

    /// An RPC that fails is an error for the caller to retry, never an acceptance.
    #[tokio::test]
    async fn a_failing_rpc_never_accepts() {
        let v = RpcEip1654Validator::new(fake_wallet(None).await, Client::new());
        let err = super::super::eip1654::verify_eip1654(&v, "0xabc", b"m", &[0u8; 65])
            .await
            .unwrap_err();
        assert!(
            matches!(err, AuthError::Eip1654ValidationFailed(_)),
            "{err}"
        );
    }

    /// A chain whose head is block 999 and whose wallet accepted `accepted` only up to block
    /// `valid_until`. `archive: false` answers state reads for past blocks the way a pruned
    /// node does.
    async fn fake_chain(accepted: [u8; 32], valid_until: u64, archive: bool) -> String {
        use axum::{Json, Router, routing::post};
        const LATEST: u64 = 999;
        let app = Router::new().route(
            "/",
            post(move |Json(body): Json<Value>| async move {
                assert_eq!(body["method"], "eth_call");
                let tag = body["params"][1].as_str().unwrap();
                let n = if tag == "latest" {
                    LATEST
                } else {
                    u64::from_str_radix(tag.trim_start_matches("0x"), 16).unwrap()
                };
                let data = body["params"][0]["data"].as_str().unwrap_or_default();
                let mut answer = if !archive && n != LATEST {
                    serde_json::json!({ "error": { "code": -32000, "message": "missing trie node" } })
                } else if n <= valid_until && data.contains(&hex::encode(accepted)) {
                    serde_json::json!({ "result": format!("0x1626ba7e{}", "0".repeat(56)) })
                } else {
                    serde_json::json!({ "error": { "code": 3, "message": "execution reverted" } })
                };
                answer["jsonrpc"] = "2.0".into();
                answer["id"] = 1.into();
                Json(answer)
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        url
    }

    #[tokio::test]
    async fn a_historical_signature_is_asked_at_its_block() {
        let hash = [0x42u8; 32];
        let v = RpcEip1654Validator::new(fake_chain(hash, 500, true).await, Client::new());
        let sig = [0u8; 65];

        assert!(!v.validate_signature("0xabc", &hash, &sig).await.unwrap());
        assert!(
            v.validate_signature_at_block("0xabc", &hash, &sig, 500)
                .await
                .unwrap()
        );
        assert!(
            !v.validate_signature_at_block("0xabc", &hash, &sig, 501)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn a_node_without_the_historical_state_is_undecidable_not_a_refusal() {
        let hash = [0x42u8; 32];
        let v = RpcEip1654Validator::new(fake_chain(hash, 999, false).await, Client::new());
        let err = v
            .validate_signature_at_block("0xabc", &hash, &[0u8; 65], 500)
            .await
            .unwrap_err();
        assert!(
            matches!(err, AuthError::Eip1654ValidationFailed(_)),
            "{err}"
        );
    }

    #[tokio::test]
    async fn short_hash_is_refused_before_any_rpc() {
        let v = RpcEip1654Validator::new("http://127.0.0.1:9/", Client::new());
        let err = v
            .validate_signature("0xabc", &[0u8; 31], &[0u8; 65])
            .await
            .unwrap_err();
        assert!(matches!(err, AuthError::Eip1654ValidationFailed(m) if m.contains("32 bytes")));
    }
}
