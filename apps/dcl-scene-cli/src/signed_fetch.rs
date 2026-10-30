//! Signed fetch (`x-identity-*` headers): an auth chain over
//! `method:path:timestamp:metadata`, verified with the catalyst's own
//! `dcl_auth::signed_fetch`. Both payload shapes verify (crypto-middleware
//! 6.x, which folds only method and path, and the older fully lowercased one),
//! over the route path and a proxy's `x-original-path` when it ends with the
//! route. Contract-wallet (EIP-1654) links are asked `isValidSignature` over
//! the JSON-RPC at `ETH_RPC_URL` (v1's `RPC_ENDPOINT_ETH` also read), as the
//! catalyst's write path asks it; with neither set they fail closed.

use axum::http::HeaderMap;
use dcl_auth::eip1654::Eip1654Validator;
use dcl_auth::rpc_validator::RpcEip1654Validator;
use dcl_auth::signed_fetch::{
    ChainShapeError, LegacyPayload, SignatureRefusal, SignedPath, SignedRequest, SkewError,
    UnsignedLinks, candidate_paths, check_symmetric_skew, extract_auth_chain, header_str,
    parse_metadata, timestamp_or_zero, verify_signature,
};
use dcl_auth::{AuthError, MAX_AUTH_CHAIN_LINKS};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;

pub use dcl_auth::signed_fetch::{
    AUTH_CHAIN_HEADER_PREFIX, AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER,
};

pub const ETH_RPC_URL_ENV: &str = "ETH_RPC_URL";
const LEGACY_ETH_RPC_URL_ENV: &str = "RPC_ENDPOINT_ETH";
/// An explorer waits on the signed fetch; a stalled RPC must not hold it.
const EIP1654_RPC_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, thiserror::Error)]
pub enum SignedFetchError {
    #[error("Invalid Auth Chain: {0}")]
    MalformedChain(String),
    #[error("Invalid timestamp: {0}")]
    InvalidTimestamp(String),
    #[error("Expired signature")]
    Expired,
    #[error("Invalid signature: {0}")]
    InvalidSignature(String),
    #[error(
        "Invalid signature: a contract-wallet (EIP-1654) link needs an Ethereum RPC; set {ETH_RPC_URL_ENV}"
    )]
    Eip1654Unconfigured,
}

impl SignedFetchError {
    /// A malformed request answers 400 with its detail, a refused one 401.
    pub fn http_status_and_message(&self) -> (u16, String) {
        match self {
            Self::MalformedChain(_) | Self::InvalidTimestamp(_) => (400, self.to_string()),
            Self::Expired | Self::InvalidSignature(_) | Self::Eip1654Unconfigured => {
                (401, format!("Unauthenticated: {self}"))
            }
        }
    }
}

fn shape_error(e: ChainShapeError) -> SignedFetchError {
    SignedFetchError::MalformedChain(match e {
        ChainShapeError::Unparseable { index, detail } => format!("link {index}: {detail}"),
        ChainShapeError::SignerNotFirst(_) | ChainShapeError::FirstNotSigner => {
            "the SIGNER link must come first, and only first".into()
        }
        ChainShapeError::MissingSignature(index) => format!("missing signature on link {index}"),
        ChainShapeError::TooLong => format!("exceeds max length of {MAX_AUTH_CHAIN_LINKS}"),
        ChainShapeError::InsufficientLinks => "fewer than two links".into(),
    })
}

/// The RPC-backed validator for contract-wallet links, from `ETH_RPC_URL`
/// (else `RPC_ENDPOINT_ETH`); `None` when neither is set.
pub fn default_eip1654_validator() -> Option<&'static RpcEip1654Validator> {
    static V: OnceLock<Option<RpcEip1654Validator>> = OnceLock::new();
    V.get_or_init(|| {
        let url = [ETH_RPC_URL_ENV, LEGACY_ETH_RPC_URL_ENV]
            .iter()
            .filter_map(|k| std::env::var(k).ok())
            .map(|v| v.trim().to_string())
            .find(|v| !v.is_empty())?;
        Some(RpcEip1654Validator::with_timeout(
            url,
            reqwest::Client::new(),
            EIP1654_RPC_TIMEOUT,
        ))
    })
    .as_ref()
}

/// One request's contract answers: every payload shape re-walks the same
/// contract-wallet links, and the RPC is asked each question once.
struct PerRequest<'a> {
    inner: &'a dyn Eip1654Validator,
    answers: Mutex<HashMap<ContractQuestion, bool>>,
}

/// Contract, hash, signature.
type ContractQuestion = (String, Vec<u8>, Vec<u8>);

#[async_trait::async_trait]
impl Eip1654Validator for PerRequest<'_> {
    async fn validate_signature(
        &self,
        contract_address: &str,
        hash: &[u8],
        signature: &[u8],
    ) -> Result<bool, AuthError> {
        let key = (
            contract_address.to_lowercase(),
            hash.to_vec(),
            signature.to_vec(),
        );
        if let Some(answer) = self.answers.lock().get(&key) {
            return Ok(*answer);
        }
        let answer = self
            .inner
            .validate_signature(contract_address, hash, signature)
            .await?;
        self.answers.lock().insert(key, answer);
        Ok(answer)
    }
}

/// The verified signer (lowercase) of a signed fetch to `method path`, and
/// its metadata object. An absent timestamp reads as 0 and so as expired.
pub async fn verify_signed_fetch_meta(
    headers: &HeaderMap,
    method: &str,
    path: &str,
    tolerance_secs: i64,
) -> Result<(String, serde_json::Value), SignedFetchError> {
    let validator = default_eip1654_validator().map(|v| v as &dyn Eip1654Validator);
    verify_signed_fetch_meta_with(headers, method, path, tolerance_secs, validator).await
}

/// [`verify_signed_fetch_meta`] with an explicit EIP-1654 validator; `None`
/// refuses contract-wallet links as [`SignedFetchError::Eip1654Unconfigured`].
pub async fn verify_signed_fetch_meta_with(
    headers: &HeaderMap,
    method: &str,
    path: &str,
    tolerance_secs: i64,
    eip1654: Option<&dyn Eip1654Validator>,
) -> Result<(String, serde_json::Value), SignedFetchError> {
    // A link without a signature is left to the verifier, which refuses it
    // as an invalid signature (401), not a malformed chain.
    let chain = extract_auth_chain(headers, UnsignedLinks::LeftToVerify).map_err(shape_error)?;
    let ts = timestamp_or_zero(header_str(headers, AUTH_TIMESTAMP_HEADER));
    let now = chrono::Utc::now().timestamp();
    check_symmetric_skew(ts, now, tolerance_secs).map_err(|e| match e {
        SkewError::InvalidTimestamp(ts) => SignedFetchError::InvalidTimestamp(ts),
        SkewError::Expired { .. } => SignedFetchError::Expired,
    })?;
    let metadata_raw = header_str(headers, AUTH_METADATA_HEADER).unwrap_or("{}");
    let metadata = parse_metadata(metadata_raw).ok_or_else(|| {
        SignedFetchError::MalformedChain(format!("invalid chain metadata: {metadata_raw}"))
    })?;
    let memo = eip1654.map(|inner| PerRequest {
        inner,
        answers: Mutex::new(HashMap::new()),
    });
    let validator = memo.as_ref().map(|m| m as &dyn Eip1654Validator);
    let signed_path = SignedPath::local(path);
    let request = SignedRequest {
        method,
        paths: &candidate_paths(headers, &signed_path),
        timestamp: ts,
        metadata: metadata_raw,
    };
    verify_signature(&chain, &request, now, LegacyPayload::Accepted, validator)
        .await
        .map_err(|refusal| match refusal {
            SignatureRefusal::Settled(AuthError::Eip1654NotImplemented) => {
                SignedFetchError::Eip1654Unconfigured
            }
            SignatureRefusal::Invalid(Some(e)) | SignatureRefusal::Settled(e) => {
                SignedFetchError::InvalidSignature(e.to_string())
            }
            SignatureRefusal::Invalid(None) | SignatureRefusal::LegacyKeysNotCanonical => {
                SignedFetchError::InvalidSignature(String::new())
            }
        })?;
    Ok((chain[0].payload.to_lowercase(), metadata))
}

/// [`verify_signed_fetch_meta`] without the metadata.
pub async fn verify_signed_fetch(
    headers: &HeaderMap,
    method: &str,
    path: &str,
    tolerance_secs: i64,
) -> Result<String, SignedFetchError> {
    verify_signed_fetch_meta(headers, method, path, tolerance_secs)
        .await
        .map(|(signer, _)| signer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    use dcl_auth::Wallet;
    use serde_json::{Value, json};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const CONTRACT: &str = "0x5555555555555555555555555555555555555555";
    const MAGIC: &str = "0x1626ba7e00000000000000000000000000000000000000000000000000000000";

    /// A contract-wallet signed fetch: SIGNER (the contract), an
    /// ECDSA_EIP_1654_EPHEMERAL delegation the contract vouches for, and the
    /// ephemeral key's signature over the request.
    fn contract_wallet_headers(method: &str, path: &str) -> HeaderMap {
        let ephemeral = Wallet::random().unwrap();
        let delegation = format!(
            "Decentraland Login\nEphemeral address: {}\nExpiration: 2099-01-01T00:00:00.000Z",
            ephemeral.address()
        );
        let ts = chrono::Utc::now().timestamp_millis().to_string();
        let payload = format!("{method}:{path}:{ts}:{{}}");
        let links = [
            json!({ "type": "SIGNER", "payload": CONTRACT, "signature": "" }),
            json!({ "type": "ECDSA_EIP_1654_EPHEMERAL", "payload": delegation, "signature": format!("0x{}", "ab".repeat(65)) }),
            json!({ "type": "ECDSA_SIGNED_ENTITY", "payload": payload, "signature": ephemeral.sign_message(payload.as_bytes()).unwrap() }),
        ];
        let mut headers = HeaderMap::new();
        for (i, link) in links.iter().enumerate() {
            headers.insert(
                axum::http::HeaderName::try_from(format!("{AUTH_CHAIN_HEADER_PREFIX}{i}")).unwrap(),
                HeaderValue::from_str(&link.to_string()).unwrap(),
            );
        }
        headers.insert(AUTH_TIMESTAMP_HEADER, HeaderValue::from_str(&ts).unwrap());
        headers.insert(AUTH_METADATA_HEADER, HeaderValue::from_static("{}"));
        headers
    }

    /// A JSON-RPC node answering every `eth_call` with `answer`, counting calls.
    async fn mock_rpc(answer: Value) -> (String, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = calls.clone();
        let app = axum::Router::new().route(
            "/",
            axum::routing::post(move |axum::Json(req): axum::Json<Value>| {
                let answer = answer.clone();
                let seen = seen.clone();
                async move {
                    assert_eq!(req["method"], "eth_call");
                    assert_eq!(req["params"][0]["to"], CONTRACT);
                    assert!(
                        req["params"][0]["data"]
                            .as_str()
                            .unwrap()
                            .starts_with("0x1626ba7e")
                    );
                    seen.fetch_add(1, Ordering::SeqCst);
                    let mut body = json!({ "jsonrpc": "2.0", "id": req["id"] });
                    body.as_object_mut()
                        .unwrap()
                        .extend(answer.as_object().unwrap().clone());
                    axum::Json(body)
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (url, calls)
    }

    async fn verify_against(answer: Value) -> (Result<String, SignedFetchError>, usize) {
        let (url, calls) = mock_rpc(answer).await;
        let rpc = RpcEip1654Validator::new(url, reqwest::Client::new());
        let headers = contract_wallet_headers("post", "/signed-login");
        let result =
            verify_signed_fetch_meta_with(&headers, "post", "/signed-login", 300, Some(&rpc))
                .await
                .map(|(signer, _)| signer);
        (result, calls.load(Ordering::SeqCst))
    }

    #[tokio::test]
    async fn a_contract_answering_the_magic_value_signs_for_itself() {
        let (result, calls) = verify_against(json!({ "result": MAGIC })).await;
        assert_eq!(result.unwrap(), CONTRACT);
        assert_eq!(calls, 1, "the raw keccak hash is asked first and answered");
    }

    #[tokio::test]
    async fn any_other_answer_is_a_rejection() {
        let (result, calls) =
            verify_against(json!({ "result": format!("0x{}", "ff".repeat(32)) })).await;
        let err = result.unwrap_err();
        assert!(
            matches!(err, SignedFetchError::InvalidSignature(ref m) if m.contains("rejected the signature")),
            "{err}"
        );
        assert_eq!(err.http_status_and_message().0, 401);
        assert_eq!(
            calls, 2,
            "raw then personal-message hash, each asked once across payload shapes"
        );
    }

    #[tokio::test]
    async fn a_reverting_contract_is_a_rejection() {
        let (result, calls) =
            verify_against(json!({ "error": { "code": -32000, "message": "execution reverted" } }))
                .await;
        let err = result.unwrap_err();
        assert!(
            matches!(err, SignedFetchError::InvalidSignature(ref m) if m.contains("rejected the signature")),
            "{err}"
        );
        assert_eq!(err.http_status_and_message().0, 401);
        assert_eq!(calls, 2, "each hash is asked once across payload shapes");
    }

    #[tokio::test]
    async fn an_rpc_error_refuses_without_retrying_every_payload() {
        let (result, calls) =
            verify_against(json!({ "error": { "code": -32603, "message": "header not found" } }))
                .await;
        let err = result.unwrap_err();
        assert!(
            matches!(err, SignedFetchError::InvalidSignature(ref m) if m.contains("RPC error -32603: header not found")),
            "{err}"
        );
        assert_eq!(err.http_status_and_message().0, 401);
        assert_eq!(
            calls, 2,
            "both hashes are asked once, not again per payload"
        );
    }

    #[tokio::test]
    async fn without_an_rpc_contract_wallets_fail_closed() {
        let headers = contract_wallet_headers("post", "/signed-login");
        let err = verify_signed_fetch_meta_with(&headers, "post", "/signed-login", 300, None)
            .await
            .unwrap_err();
        assert!(matches!(err, SignedFetchError::Eip1654Unconfigured));
        let (status, message) = err.http_status_and_message();
        assert_eq!(status, 401);
        assert!(message.contains(ETH_RPC_URL_ENV), "{message}");
    }
}
