//! The WebSocket auth frame: the first binary frame a client sends is
//! `[MessageType::Auth]` + the signed-fetch header bag as one JSON object
//! (`x-identity-auth-chain-N`, `x-identity-timestamp`, `x-identity-metadata`),
//! signed over `get:/ws/<scene>:<timestamp>:<metadata>`. The verified SIGNER
//! is the client's identity; its address is what the permission core admits
//! and what the scene sees.
//!
//! Both payload shapes the old host accepted are kept: the crypto-middleware
//! 6.x one (method and path folded, the metadata bytes bound to the
//! signature) first, the pre-6.0 fully lowercased one only when the 6.x
//! verdict was an invalid signature and the two differ. Two things are
//! stricter than the old host, matching the other v2 sockets: a
//! non-numeric timestamp is refused instead of skipping the freshness
//! window, and a frame the explorer signed on a scene's behalf
//! (`signer: decentraland-kernel-scene`) is refused before the signature is
//! checked. The frame's metadata is handed back verified: a shared-secret
//! world reads the presented `secret` from it.

use std::collections::HashMap;

use serde_json::Value;
use thiserror::Error;

use crate::auth::AuthError as ChainError;
use crate::auth::eip1654::Eip1654Validator;
use crate::auth::signed_fetch::{
    LegacyPayload, SignatureRefusal, SignedRequest, SkewError, check_symmetric_skew,
    parse_metadata, verify_signature,
};
use crate::shared::identity::Address;
use crate::shared::rpc::handshake::{extract_from_object, is_refused_signer};
use crate::shared::signed_fetch::{AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER};

pub use crate::shared::signed_fetch::FIVE_MINUTES;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("auth frame is not valid JSON: {0}")]
    BadJson(String),
    #[error("auth chain malformed: {0}")]
    MalformedChain(String),
    #[error("auth chain has fewer than 2 links")]
    InsufficientLinks,
    #[error("signature expired (signed_at={signed_at}, now={now}, window={window_secs}s)")]
    Expired {
        signed_at: i64,
        now: i64,
        window_secs: i64,
    },
    #[error("signature rejected: {0}")]
    InvalidSignature(String),
    #[error("requests from scenes are not allowed")]
    RefusedSigner,
    #[error("EIP-1654 chains need an RPC validator")]
    EipNotImplemented,
    #[error("signature validation unavailable: {0}")]
    ValidatorUnavailable(String),
}

#[derive(Debug, Clone)]
pub struct Authenticated {
    pub signer: Address,
    /// The signed `x-identity-metadata`, parsed (`{}` when absent).
    pub metadata: Value,
}

impl Authenticated {
    /// The `secret` field of the signed metadata: what a shared-secret world
    /// checks. Bound to the signature, so it cannot be replayed onto another
    /// identity.
    pub fn presented_secret(&self) -> Option<&str> {
        self.metadata
            .get("secret")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
    }
}

/// Verifies `frame_json` for `method` over the first of `paths` that
/// verifies (only an invalid-signature verdict tries the next), at `now`
/// seconds.
pub async fn verify_auth_frame(
    frame_json: &[u8],
    method: &str,
    paths: &[String],
    now: i64,
    validator: Option<&dyn Eip1654Validator>,
) -> Result<Authenticated, AuthError> {
    let headers: HashMap<String, String> =
        serde_json::from_slice(frame_json).map_err(|e| AuthError::BadJson(e.to_string()))?;
    let obj: serde_json::Map<String, Value> = headers
        .into_iter()
        .map(|(k, v)| (k.to_ascii_lowercase(), Value::String(v)))
        .collect();

    let chain = extract_from_object(&obj).map_err(|e| match e {
        crate::shared::rpc::handshake::HandshakeError::InsufficientLinks => {
            AuthError::InsufficientLinks
        }
        other => AuthError::MalformedChain(other.to_string()),
    })?;
    let signer = Address::parse(&chain[0].payload)
        .map_err(|e| AuthError::MalformedChain(format!("SIGNER payload is not an address: {e}")))?;

    // An absent timestamp is 0 (expired); an empty one is not a number.
    let ts = obj
        .get(AUTH_TIMESTAMP_HEADER)
        .and_then(|v| v.as_str())
        .unwrap_or("0");
    let metadata_raw = obj
        .get(AUTH_METADATA_HEADER)
        .and_then(|v| v.as_str())
        .unwrap_or("{}");
    check_symmetric_skew(ts, now, FIVE_MINUTES).map_err(|e| match e {
        SkewError::InvalidTimestamp(ts) => {
            AuthError::MalformedChain(format!("invalid timestamp: {ts:?}"))
        }
        SkewError::Expired {
            signed_at,
            now,
            window_secs,
        } => AuthError::Expired {
            signed_at,
            now,
            window_secs,
        },
    })?;
    let metadata = parse_metadata(metadata_raw)
        .ok_or_else(|| AuthError::MalformedChain("invalid chain metadata".into()))?;
    if is_refused_signer(&metadata) {
        return Err(AuthError::RefusedSigner);
    }

    let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
    let request = SignedRequest {
        method,
        paths: &paths,
        timestamp: ts,
        metadata: metadata_raw,
    };
    verify_signature(&chain, &request, now, LegacyPayload::Accepted, validator)
        .await
        .map_err(|refusal| match refusal {
            SignatureRefusal::Invalid(Some(e)) | SignatureRefusal::Settled(e) => chain_error(e),
            // `Accepted` never refuses on the legacy guard.
            SignatureRefusal::Invalid(None) | SignatureRefusal::LegacyKeysNotCanonical => {
                AuthError::InvalidSignature("no path".into())
            }
        })?;
    Ok(Authenticated { signer, metadata })
}

fn chain_error(e: ChainError) -> AuthError {
    match e {
        ChainError::MalformedChain(d) | ChainError::InvalidEphemeralPayload(d) => {
            AuthError::MalformedChain(d)
        }
        ChainError::MissingSignature { .. } => AuthError::MalformedChain(e.to_string()),
        ChainError::EphemeralExpired {
            expiration_ms,
            now_ms,
        } => AuthError::Expired {
            signed_at: expiration_ms / 1000,
            now: now_ms / 1000,
            window_secs: 0,
        },
        ChainError::Eip1654NotImplemented => AuthError::EipNotImplemented,
        ChainError::Eip1654ValidationFailed(d) => AuthError::ValidatorUnavailable(d),
        other => AuthError::InvalidSignature(format!("{other:?}")),
    }
}

/// The paths a `/ws/{scene}` frame may be signed over, tried in order: a
/// proxy-forwarded `x-original-path` when it ends with the route (the old
/// `signed_fetch_path` rule: the header is client-controlled, so any other
/// value is ignored), the path this node served the socket at (the nested
/// `/scene-state/ws/{scene}`), and the bare route.
pub fn candidate_paths(
    headers: &axum::http::HeaderMap,
    served_path: &str,
    route: &str,
) -> Vec<String> {
    let mut paths = Vec::with_capacity(3);
    if let Some(original) = headers
        .get(crate::shared::signed_fetch::ORIGINAL_PATH_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|raw| raw.split('?').next().unwrap_or(raw))
        .filter(|stripped| stripped.ends_with(route))
    {
        paths.push(original.to_string());
    }
    for p in [served_path, route] {
        if !paths.iter().any(|q| q == p) {
            paths.push(p.to_string());
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::signed_fetch::build_legacy_payload;
    use axum::http::{HeaderMap, HeaderValue};

    async fn verify_one(body: &[u8], path: &str, now: i64) -> Result<Authenticated, AuthError> {
        verify_auth_frame(body, "GET", &[path.to_string()], now, None).await
    }

    #[tokio::test]
    async fn rejects_non_json() {
        let err = verify_one(b"not json", "/ws/x", 0).await.unwrap_err();
        assert!(
            matches!(err, AuthError::BadJson(_)),
            "expected BadJson, got {err:?}"
        );
    }

    #[tokio::test]
    async fn rejects_chain_with_one_link() {
        let body = serde_json::json!({
            "x-identity-auth-chain-0": "{\"type\":\"SIGNER\",\"payload\":\"0xabc\",\"signature\":\"\"}"
        })
        .to_string();
        let err = verify_one(body.as_bytes(), "/ws/x", 0).await.unwrap_err();
        assert!(
            matches!(err, AuthError::InsufficientLinks),
            "expected InsufficientLinks, got {err:?}"
        );
    }

    #[tokio::test]
    async fn rejects_signer_link_at_non_zero_index() {
        let body = serde_json::json!({
            "x-identity-auth-chain-0": "{\"type\":\"ECDSA_EPHEMERAL\",\"payload\":\"0xabc\",\"signature\":\"0xsig\"}",
            "x-identity-auth-chain-1": "{\"type\":\"SIGNER\",\"payload\":\"0xabc\",\"signature\":\"\"}"
        })
        .to_string();
        let err = verify_one(body.as_bytes(), "/ws/x", 0).await.unwrap_err();
        assert!(
            matches!(err, AuthError::MalformedChain(_)),
            "expected MalformedChain, got {err:?}"
        );
    }

    #[tokio::test]
    async fn rejects_non_signer_first_link() {
        let body = serde_json::json!({
            "x-identity-auth-chain-0": "{\"type\":\"ECDSA_EPHEMERAL\",\"payload\":\"0xabc\",\"signature\":\"0xsig\"}",
            "x-identity-auth-chain-1": "{\"type\":\"ECDSA_SIGNED_ENTITY\",\"payload\":\"payload\",\"signature\":\"0xsig\"}"
        })
        .to_string();
        let err = verify_one(body.as_bytes(), "/ws/x", 0).await.unwrap_err();
        assert!(
            matches!(err, AuthError::MalformedChain(_)),
            "expected MalformedChain, got {err:?}"
        );
    }

    #[tokio::test]
    async fn rejects_missing_signature_on_non_signer_link() {
        let body = serde_json::json!({
            "x-identity-auth-chain-0": "{\"type\":\"SIGNER\",\"payload\":\"0xabc\",\"signature\":\"\"}",
            "x-identity-auth-chain-1": "{\"type\":\"ECDSA_SIGNED_ENTITY\",\"payload\":\"payload\"}"
        })
        .to_string();
        let err = verify_one(body.as_bytes(), "/ws/x", 0).await.unwrap_err();
        assert!(
            matches!(err, AuthError::MalformedChain(_)),
            "expected MalformedChain, got {err:?}"
        );
    }

    #[test]
    fn payload_is_lowercased() {
        assert_eq!(
            build_legacy_payload("GET", "/ws/MyWorld.dcl.eth", "123", "{}"),
            "get:/ws/myworld.dcl.eth:123:{}"
        );
    }

    #[test]
    fn candidate_paths_take_a_suffix_matching_original_path_first() {
        let mut headers = HeaderMap::new();
        assert_eq!(
            candidate_paths(&headers, "/scene-state/ws/w.dcl.eth", "/ws/w.dcl.eth"),
            vec!["/scene-state/ws/w.dcl.eth", "/ws/w.dcl.eth"]
        );
        headers.insert(
            "x-original-path",
            HeaderValue::from_static("/edge/ws/w.dcl.eth?x=1"),
        );
        assert_eq!(
            candidate_paths(&headers, "/scene-state/ws/w.dcl.eth", "/ws/w.dcl.eth"),
            vec![
                "/edge/ws/w.dcl.eth",
                "/scene-state/ws/w.dcl.eth",
                "/ws/w.dcl.eth"
            ]
        );
        headers.insert(
            "x-original-path",
            HeaderValue::from_static("/ws/other.dcl.eth"),
        );
        assert_eq!(
            candidate_paths(&headers, "/ws/w.dcl.eth", "/ws/w.dcl.eth"),
            vec!["/ws/w.dcl.eth"]
        );
    }
}
