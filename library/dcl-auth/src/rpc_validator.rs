//! EIP-1271/EIP-1654 `isValidSignature(bytes32,bytes)` over JSON-RPC
//! `eth_call`. The validator borrows the process-wide reqwest client (the
//! raw half: `ETH_RPC_URL` is the operator's exact https endpoint) and
//! bounds each call with its own timeout, so a stalled RPC never holds a
//! deployment open; the request and answer shapes are the shared
//! `http::jsonrpc` ones. Transport errors are reported without the URL: it may
//! carry an API key, and the error text reaches logs and failure records.
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

    async fn eth_call(&self, to: &str, data: &[u8]) -> Result<String, EthCallError> {
        let failed = |m: String| EthCallError::Failed(AuthError::Eip1654ValidationFailed(m));
        let call_data = format!("0x{}", hex::encode(data));
        let body = jsonrpc::request(
            1,
            "eth_call",
            serde_json::json!([{ "to": to, "data": call_data }, "latest"]),
        );
        let resp = self
            .client
            .post(&self.rpc_url)
            .timeout(self.timeout)
            .json(&body)
            .send()
            .await
            .map_err(|e| failed(format!("RPC request failed: {}", e.without_url())))?;
        let status = resp.status().as_u16();
        let bytes = read_body_capped(resp, jsonrpc::DEFAULT_BODY_CAP)
            .await
            .map_err(|e| failed(format!("Failed to read RPC response: {e}")))?;
        let result = jsonrpc::parse_envelope(status, &bytes)
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
            })?;
        match result {
            Value::String(s) => Ok(s),
            Value::Null => Err(failed("RPC returned null result with no error".into())),
            other => Err(failed(format!("Unexpected RPC result type: {other}"))),
        }
    }
}

#[async_trait]
impl Eip1654Validator for RpcEip1654Validator {
    async fn validate_signature(
        &self,
        contract_address: &str,
        hash: &[u8],
        signature: &[u8],
    ) -> Result<bool, AuthError> {
        if hash.len() != 32 {
            return Err(AuthError::Eip1654ValidationFailed(format!(
                "hash must be 32 bytes, got {}",
                hash.len()
            )));
        }

        let calldata = encode_is_valid_signature(hash, signature);
        match self.eth_call(contract_address, &calldata).await {
            Ok(result_hex) => Ok(is_magic_return(&result_hex)),
            Err(EthCallError::Reverted) => Ok(false),
            Err(EthCallError::Failed(e)) => Err(e),
        }
    }
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
