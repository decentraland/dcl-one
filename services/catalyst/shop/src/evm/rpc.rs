//! One JSON-RPC round trip over the process's guarded client
//! (`crate::http::Outbound`): the endpoint is an operator-configured URL that
//! may carry a gateway key in its path, so it is never echoed in an error or
//! a log line, and every request passes the SSRF guard and the body cap.

use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::http::{Outbound, SendError, read_body_capped};

/// Receipts of a busy block carry hundreds of logs; four MiB covers them.
const RPC_BODY_CAP: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcError {
    /// The node could not answer (guard refusal, transport, HTTP status,
    /// malformed body): a property of the endpoint, never of a contract.
    Transport(String),
    /// The node answered with a JSON-RPC error object.
    Response { code: i64, message: String },
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RpcError::Transport(m) => write!(f, "rpc unavailable: {m}"),
            RpcError::Response { code, message } => write!(f, "rpc error {code}: {message}"),
        }
    }
}

impl RpcError {
    /// Whether the error is the target reverting rather than the node
    /// failing: EIP-1474 code 3, or a node-specific error naming the revert.
    pub fn is_execution_revert(&self) -> bool {
        match self {
            RpcError::Response { code: 3, .. } => true,
            RpcError::Response { message, .. } => {
                let m = message.to_ascii_lowercase();
                m.contains("execution reverted") || m.contains("revert")
            }
            RpcError::Transport(_) => false,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            RpcError::Transport(m) | RpcError::Response { message: m, .. } => m,
        }
    }
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<JsonRpcErrorBody>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcErrorBody {
    #[serde(default)]
    code: i64,
    #[serde(default)]
    message: String,
}

#[derive(Clone)]
pub struct Rpc {
    http: Outbound,
    url: String,
    timeout: Duration,
}

impl std::fmt::Debug for Rpc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Rpc")
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl Rpc {
    pub fn new(http: Outbound, url: &str, timeout: Duration) -> Self {
        Self {
            http,
            url: url.trim().to_string(),
            timeout,
        }
    }

    pub async fn call(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
        let resp = self
            .http
            .post(&self.url)
            .timeout(self.timeout)
            .json(&body)
            .send()
            .await
            .map_err(|e| match e {
                SendError::Refused(m) => {
                    RpcError::Transport(format!("{method}: endpoint refused: {m}"))
                }
                SendError::Http(e) => RpcError::Transport(format!("{method}: {e}")),
            })?;
        let status = resp.status();
        if !status.is_success() {
            return Err(RpcError::Transport(format!(
                "{method}: rpc responded with status {}",
                status.as_u16()
            )));
        }
        let bytes = read_body_capped(resp, RPC_BODY_CAP)
            .await
            .map_err(|e| RpcError::Transport(format!("{method}: {e}")))?;
        let parsed: JsonRpcResponse = serde_json::from_slice(&bytes)
            .map_err(|e| RpcError::Transport(format!("{method}: malformed rpc body: {e}")))?;
        if let Some(err) = parsed.error {
            return Err(RpcError::Response {
                code: err.code,
                message: err.message,
            });
        }
        Ok(parsed.result.unwrap_or(Value::Null))
    }

    /// A call whose result is a `0x` hex string (quantities and data).
    pub async fn call_hex(&self, method: &str, params: Value) -> Result<String, RpcError> {
        match self.call(method, params).await? {
            Value::String(s) => Ok(s),
            Value::Null => Err(RpcError::Transport(format!("{method} returned no result"))),
            other => Err(RpcError::Transport(format!(
                "{method} returned a non-string result: {other}"
            ))),
        }
    }
}

pub fn decode_hex(hex_str: &str) -> Option<Vec<u8>> {
    let stripped = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    if stripped.is_empty() {
        return Some(Vec::new());
    }
    hex::decode(stripped).ok()
}

pub fn parse_quantity_u128(hex_str: &str) -> Result<u128, RpcError> {
    let h = hex_str.trim().trim_start_matches("0x");
    if h.is_empty() {
        return Ok(0);
    }
    u128::from_str_radix(h, 16)
        .map_err(|e| RpcError::Transport(format!("bad quantity {hex_str}: {e}")))
}

pub fn parse_quantity_u64(hex_str: &str) -> Result<u64, RpcError> {
    let v = parse_quantity_u128(hex_str)?;
    u64::try_from(v)
        .map_err(|_| RpcError::Transport(format!("quantity {hex_str} does not fit u64")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revert_is_distinguished_from_transport() {
        assert!(
            RpcError::Response {
                code: 3,
                message: "execution reverted".into()
            }
            .is_execution_revert()
        );
        assert!(
            RpcError::Response {
                code: -32000,
                message: "execution reverted: bad".into()
            }
            .is_execution_revert()
        );
        assert!(
            !RpcError::Response {
                code: -32000,
                message: "header not found".into()
            }
            .is_execution_revert()
        );
        assert!(
            !RpcError::Response {
                code: 429,
                message: "rate limited".into()
            }
            .is_execution_revert()
        );
        assert!(
            !RpcError::Transport("execution reverted".into()).is_execution_revert(),
            "a transport failure is never a revert"
        );
    }

    #[test]
    fn empty_code_decodes_to_no_bytes() {
        assert_eq!(decode_hex("0x"), Some(Vec::new()));
        assert_eq!(decode_hex(""), Some(Vec::new()));
        assert_eq!(decode_hex("0x0102"), Some(vec![0x01, 0x02]));
        assert_eq!(decode_hex("0xzz"), None);
    }

    #[test]
    fn quantities_parse_with_and_without_padding() {
        assert_eq!(parse_quantity_u128("0x89").unwrap(), 137);
        assert_eq!(parse_quantity_u64("0x0").unwrap(), 0);
        assert_eq!(parse_quantity_u64("0x").unwrap(), 0);
        assert!(parse_quantity_u64("0xzz").is_err());
        assert!(parse_quantity_u64(&format!("0x{}", "f".repeat(20))).is_err());
    }

    #[tokio::test]
    async fn the_guard_refuses_a_private_endpoint_before_any_byte_leaves() {
        let rpc = Rpc::new(
            Outbound::new(crate::http::HttpClientCfg::default()).unwrap(),
            "http://127.0.0.1:9/",
            Duration::from_secs(1),
        );
        let err = rpc.call("eth_chainId", json!([])).await.unwrap_err();
        assert!(
            matches!(err, RpcError::Transport(ref m) if m.contains("refused")),
            "{err}"
        );
    }

    #[test]
    fn debug_never_prints_the_endpoint() {
        let rpc = Rpc::new(
            Outbound::for_tests(),
            "https://rpc.example/v3/SECRET",
            Duration::from_secs(1),
        );
        assert!(!format!("{rpc:?}").contains("SECRET"));
    }
}
