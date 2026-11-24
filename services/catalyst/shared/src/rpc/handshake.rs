//! The WebSocket handshake envelope: the first frame a client sends is the
//! signed-request header bag encoded as one JSON object
//! (`x-identity-auth-chain-N`, `x-identity-timestamp`, `x-identity-metadata`).
//! [`frame_fields`] reads it as upstream's `parseAuthChainHeaders` does and
//! [`frame_parts`] hands the fields to `dcl_auth::request::verify_request`
//! for the socket's own signed path. The verified SIGNER address becomes the
//! connection's identity; nothing else authenticates a procedure call.

use serde_json::{Map, Value};
use thiserror::Error;

use crate::auth::request::RequestParts;
use crate::auth::signed_fetch::{ChainShapeError, UnsignedLinks, extract_auth_chain_with};
use crate::auth::{AuthChain, MAX_AUTH_CHAIN_LINKS};
use crate::signed_fetch::{AUTH_CHAIN_HEADER_PREFIX, ORIGINAL_PATH_HEADER, SignedPath};

pub use crate::auth::signed_fetch::is_refused_signer;

const DETAIL_MAX_CHARS: usize = 64;

#[derive(Debug, Error)]
pub enum HandshakeError {
    #[error("invalid auth-chain envelope: not a JSON object")]
    EnvelopeNotObject,
    #[error("invalid auth-chain envelope: header values must be strings")]
    HeaderNotString,
    #[error("invalid auth-chain link {index}: {detail}")]
    MalformedChain { index: usize, detail: String },
    #[error("auth-chain shorter than 2 links")]
    InsufficientLinks,
}

fn truncate_detail(value: &str) -> String {
    if value.chars().count() > DETAIL_MAX_CHARS {
        let head: String = value.chars().take(DETAIL_MAX_CHARS).collect();
        format!("{head}...")
    } else {
        value.to_string()
    }
}

/// A header read under its exact lowercase name, as `verify()` in
/// `@dcl/crypto-middleware` reads the frame: a re-cased key is never read, so
/// it cannot shadow the lowercase one.
fn obj_str<'a>(obj: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    obj.get(key).and_then(Value::as_str)
}

/// The first frame as its identity fields ([`identity_headers`]).
pub fn frame_fields(frame_json: &str) -> Result<Map<String, Value>, HandshakeError> {
    let value: Value =
        serde_json::from_str(frame_json).map_err(|e| HandshakeError::MalformedChain {
            index: 0,
            detail: format!("frame not JSON: {e}"),
        })?;
    match value {
        Value::Object(obj) => identity_headers(obj),
        _ => Err(HandshakeError::EnvelopeNotObject),
    }
}

/// The frame's fields as a request to `method path`. The signed path is the
/// socket's own: an `x-original-path` in the frame is never read.
pub fn frame_parts(fields: &Map<String, Value>, method: &str, path: SignedPath) -> RequestParts {
    RequestParts::from_fields(method, path, |name| {
        (name != ORIGINAL_PATH_HEADER)
            .then(|| obj_str(fields, name).map(str::to_owned))
            .flatten()
    })
}

const IDENTITY_PREFIX: &str = "x-identity-";

/// Every `x-identity-*` field, in any case, must be a string or an array of
/// strings before anything is verified (upstream's `parseAuthChainHeaders` in
/// `ws-handler.ts`); other fields are left alone. An array reads as its first
/// element and an empty one as absent, as `firstOf` in `@dcl/crypto-middleware`
/// reads a repeated header.
///
/// The chain links as `verify()` reads them: an empty link ends the chain,
/// and the raw value at index [`MAX_AUTH_CHAIN_LINKS`] is tested before
/// `firstOf`, so any array there, an empty one included, or a non-empty
/// string is one link too many.
fn identity_headers(mut obj: Map<String, Value>) -> Result<Map<String, Value>, HandshakeError> {
    let past_cap = format!("{AUTH_CHAIN_HEADER_PREFIX}{MAX_AUTH_CHAIN_LINKS}");
    let too_long = match obj.get(&past_cap) {
        Some(Value::Array(_)) => true,
        Some(Value::String(link)) => !link.is_empty(),
        _ => false,
    };
    for (key, value) in obj.iter_mut() {
        if !key
            .get(..IDENTITY_PREFIX.len())
            .is_some_and(|p| p.eq_ignore_ascii_case(IDENTITY_PREFIX))
        {
            continue;
        }
        match value {
            Value::String(_) => {}
            Value::Array(items) if items.iter().all(Value::is_string) => {
                *value = items.first().cloned().unwrap_or(Value::Null);
            }
            _ => return Err(HandshakeError::HeaderNotString),
        }
        if key.starts_with(AUTH_CHAIN_HEADER_PREFIX) && value.as_str() == Some("") {
            *value = Value::Null;
        }
    }
    if too_long {
        return Err(HandshakeError::MalformedChain {
            index: MAX_AUTH_CHAIN_LINKS,
            detail: format!("exceeds max length {MAX_AUTH_CHAIN_LINKS}"),
        });
    }
    Ok(obj)
}

pub fn extract_from_object(obj: &Map<String, Value>) -> Result<AuthChain, HandshakeError> {
    extract_auth_chain_with(|name| obj_str(obj, name), UnsignedLinks::Malformed).map_err(|e| {
        let (index, detail) = match e {
            ChainShapeError::InsufficientLinks => return HandshakeError::InsufficientLinks,
            ChainShapeError::Unparseable { index, detail } => (index, truncate_detail(&detail)),
            ChainShapeError::SignerNotFirst(index) => {
                (index, "SIGNER link at non-zero index".into())
            }
            ChainShapeError::FirstNotSigner => (0, "first link must be SIGNER".into()),
            ChainShapeError::MissingSignature(index) => (index, "missing signature".into()),
            ChainShapeError::TooLong => (
                MAX_AUTH_CHAIN_LINKS,
                format!("exceeds max length {MAX_AUTH_CHAIN_LINKS}"),
            ),
        };
        HandshakeError::MalformedChain { index, detail }
    })
}

#[cfg(any(test, feature = "test-support"))]
pub mod test_support {
    use std::sync::LazyLock;

    use alloy::signers::{Signer as _, local::PrivateKeySigner};

    use crate::signed_fetch::build_payload_v6;

    /// The run's wallets, made fresh per process so no key is written out.
    pub static ROOT_WALLET: LazyLock<PrivateKeySigner> = LazyLock::new(PrivateKeySigner::random);
    pub static EPHEMERAL_WALLET: LazyLock<PrivateKeySigner> =
        LazyLock::new(PrivateKeySigner::random);
    /// The ephemeral link's expiry for frames that only reach this node.
    pub const FAR_EXPIRATION: &str = "2099-01-01T00:00:00.000Z";

    /// A three-link chain (SIGNER, ECDSA_EPHEMERAL, ECDSA_SIGNED_ENTITY) as the
    /// explorer sends it: `signed_payload` is what the ephemeral key signs,
    /// `delivered_metadata` what the frame carries (they differ only in the
    /// re-cased-key attack shape). Returns `(lowercase signer, frame json)`.
    pub async fn frame_with(
        root: &PrivateKeySigner,
        ts_ms: i64,
        signed_payload: &str,
        delivered_metadata: &str,
    ) -> (String, String) {
        frame_signed(
            root,
            &EPHEMERAL_WALLET,
            FAR_EXPIRATION,
            ts_ms,
            signed_payload,
            delivered_metadata,
        )
        .await
    }

    /// [`frame_with`] with the ephemeral wallet the caller holds, valid until
    /// `expiration` (an ISO timestamp).
    pub async fn frame_signed(
        root: &PrivateKeySigner,
        ephemeral: &PrivateKeySigner,
        expiration: &str,
        ts_ms: i64,
        signed_payload: &str,
        delivered_metadata: &str,
    ) -> (String, String) {
        let root_address = format!("{:#x}", root.address());
        let ephemeral_address = format!("{:#x}", ephemeral.address());
        let ephemeral_payload = format!(
            "Decentraland Login\nEphemeral address: {ephemeral_address}\nExpiration: {expiration}"
        );
        let ephemeral_sig = root
            .sign_message(ephemeral_payload.as_bytes())
            .await
            .unwrap();
        let entity_sig = ephemeral
            .sign_message(signed_payload.as_bytes())
            .await
            .unwrap();
        let frame = serde_json::json!({
            "x-identity-auth-chain-0": serde_json::json!({"type": "SIGNER", "payload": root_address, "signature": ""}).to_string(),
            "x-identity-auth-chain-1": serde_json::json!({"type": "ECDSA_EPHEMERAL", "payload": ephemeral_payload, "signature": ephemeral_sig.to_string()}).to_string(),
            "x-identity-auth-chain-2": serde_json::json!({"type": "ECDSA_SIGNED_ENTITY", "payload": signed_payload, "signature": entity_sig.to_string()}).to_string(),
            "x-identity-timestamp": ts_ms.to_string(),
            "x-identity-metadata": delivered_metadata
        });
        (root_address.to_lowercase(), frame.to_string())
    }

    /// A frame signed by `root` for `get <path>` at `ts_ms` with `{}` metadata.
    pub async fn frame_for(root: &PrivateKeySigner, path: &str, ts_ms: i64) -> (String, String) {
        let payload = build_payload_v6("get", path, &ts_ms.to_string(), "{}");
        frame_with(root, ts_ms, &payload, "{}").await
    }

    pub async fn frame(path: &str, ts_ms: i64) -> (String, String) {
        frame_for(&ROOT_WALLET, path, ts_ms).await
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;
    use crate::auth::request::{
        RequestAuthError, VerifiedRequest, VerifyOptions, reject_if_signer, verify_request_at,
    };
    use crate::signed_fetch::{SCENE_SIGNER, build_legacy_payload, build_payload_v6};
    use serde_json::json;

    const NOW_MS: i64 = 1_700_000_000_000;

    #[derive(Debug)]
    enum Refused {
        Envelope(HandshakeError),
        Request(RequestAuthError),
    }

    async fn verify(frame: &str, method: &str, path: &str) -> Result<VerifiedRequest, Refused> {
        let fields = frame_fields(frame).map_err(Refused::Envelope)?;
        let options = VerifyOptions::builder()
            .metadata_validator(reject_if_signer(&[SCENE_SIGNER]).unwrap())
            .build()
            .unwrap();
        let parts = frame_parts(&fields, method, SignedPath::local(path));
        verify_request_at(&parts, &options, NOW_MS)
            .await
            .map(Option::unwrap)
            .map_err(Refused::Request)
    }

    fn status(result: Result<VerifiedRequest, Refused>) -> u16 {
        match result {
            Err(Refused::Request(e)) => e.status(),
            other => panic!("expected a verifier refusal: {other:?}"),
        }
    }

    async fn edited(edit: impl FnOnce(&mut Map<String, Value>)) -> String {
        let (_, signed) = frame("/", NOW_MS).await;
        let mut obj: Map<String, Value> = serde_json::from_str(&signed).unwrap();
        edit(&mut obj);
        Value::Object(obj).to_string()
    }

    #[tokio::test]
    async fn a_frame_verifies_for_its_method_and_path_only() {
        let (signer, signed) = frame("/", NOW_MS).await;
        assert_eq!(verify(&signed, "get", "/").await.unwrap().signer, signer);
        assert_eq!(status(verify(&signed, "post", "/").await), 401);
        assert_eq!(status(verify(&signed, "get", "/social-rpc").await), 401);
    }

    #[tokio::test]
    async fn an_original_path_in_the_frame_is_never_a_candidate() {
        let (_, signed) = frame("/v1/friends/", NOW_MS).await;
        let mut obj: Map<String, Value> = serde_json::from_str(&signed).unwrap();
        obj.insert(ORIGINAL_PATH_HEADER.into(), json!("/v1/friends/"));
        let fields = frame_fields(&Value::Object(obj).to_string()).unwrap();
        let parts = frame_parts(&fields, "get", SignedPath::local("/"));
        assert_eq!(parts.original_path, None);
        assert_eq!(parts.candidate_paths(), vec!["/"]);
    }

    #[tokio::test]
    async fn a_timestamp_outside_sixty_seconds_is_expired() {
        let (_, stale) = frame("/", NOW_MS - 61_000).await;
        assert_eq!(status(verify(&stale, "get", "/").await), 401);
        let (_, fresh) = frame("/", NOW_MS - 59_000).await;
        assert!(verify(&fresh, "get", "/").await.is_ok());
    }

    #[tokio::test]
    async fn a_malformed_envelope_is_refused_before_the_verifier() {
        assert!(matches!(
            verify("not json", "get", "/").await,
            Err(Refused::Envelope(HandshakeError::MalformedChain { .. }))
        ));
        assert!(matches!(
            verify("[]", "get", "/").await,
            Err(Refused::Envelope(HandshakeError::EnvelopeNotObject))
        ));
        let short = json!({
            "x-identity-auth-chain-0": "{\"type\":\"SIGNER\",\"payload\":\"0xabc\",\"signature\":\"\"}",
            "x-identity-timestamp": "0",
            "x-identity-metadata": "{}"
        })
        .to_string();
        assert_eq!(status(verify(&short, "get", "/").await), 400);
        let no_timestamp = edited(|obj| {
            obj.remove("x-identity-timestamp");
        })
        .await;
        assert_eq!(status(verify(&no_timestamp, "get", "/").await), 401);
    }

    /// Upstream's `parseAuthChainHeaders`: an `x-identity-*` field in any case
    /// that is neither a string nor an array of strings is refused before
    /// anything is verified.
    #[tokio::test]
    async fn an_identity_field_that_is_not_a_string_is_refused_up_front() {
        for (key, value) in [
            ("x-identity-timestamp", json!(NOW_MS)),
            ("x-identity-metadata", json!({})),
            ("x-identity-metadata", Value::Null),
            ("x-identity-auth-chain-0", json!({ "type": "SIGNER" })),
            ("x-identity-auth-chain-1", json!(["{}", 1])),
            ("X-Identity-Anything", json!(1)),
        ] {
            let frame = edited(|obj| {
                obj.insert(key.to_string(), value.clone());
            })
            .await;
            assert!(
                matches!(
                    verify(&frame, "get", "/").await,
                    Err(Refused::Envelope(HandshakeError::HeaderNotString))
                ),
                "{key} {value}"
            );
        }
        let other = edited(|obj| {
            obj.insert("version".into(), json!({ "nested": [1] }));
        })
        .await;
        assert!(verify(&other, "get", "/").await.is_ok());
    }

    /// `firstOf`: an array of strings reads as its first element, and an empty
    /// one as absent.
    #[tokio::test]
    async fn an_array_of_strings_reads_as_its_first_element() {
        let (signer, _) = frame("/", NOW_MS).await;
        let wrapped = edited(|obj| {
            for value in obj.values_mut() {
                *value = json!([value.clone(), "ignored"]);
            }
        })
        .await;
        assert_eq!(verify(&wrapped, "get", "/").await.unwrap().signer, signer);
        // An empty metadata array is absent, signed as nothing: a chain signed
        // over `{}` does not verify without it.
        let empty_metadata = edited(|obj| {
            obj.insert("x-identity-metadata".into(), json!([]));
        })
        .await;
        assert_eq!(status(verify(&empty_metadata, "get", "/").await), 401);
    }

    /// `createPayload` joins an absent metadata as empty: the payload ends
    /// with `:`, and a chain signed over `{}` does not verify without it.
    #[tokio::test]
    async fn an_absent_metadata_is_signed_as_nothing() {
        let ts = NOW_MS.to_string();
        let absent = async |signed_metadata: &str| {
            let payload = build_payload_v6("get", "/", &ts, signed_metadata);
            let (signer, frame) = frame_with(&ROOT_WALLET, NOW_MS, &payload, "{}").await;
            let mut obj: Map<String, Value> = serde_json::from_str(&frame).unwrap();
            obj.remove("x-identity-metadata");
            (signer, Value::Object(obj).to_string())
        };
        let (signer, frame) = absent("").await;
        assert_eq!(verify(&frame, "get", "/").await.unwrap().signer, signer);
        let (_, frame) = absent("{}").await;
        assert_eq!(status(verify(&frame, "get", "/").await), 401);
    }

    /// The raw value at index 10 is tested before `firstOf`: any array there,
    /// an empty one included, or a non-empty string is too long; an empty
    /// string there is not, and an empty link anywhere ends the chain.
    #[tokio::test]
    async fn a_link_at_the_cap_is_too_long_whatever_its_value() {
        let (signer, _) = frame("/", NOW_MS).await;
        for value in [json!([]), json!([""]), json!(["{}"]), json!("x")] {
            let frame = edited(|obj| {
                obj.insert("x-identity-auth-chain-10".into(), value.clone());
            })
            .await;
            assert!(
                matches!(
                    verify(&frame, "get", "/").await,
                    Err(Refused::Envelope(HandshakeError::MalformedChain {
                        index: 10,
                        ..
                    }))
                ),
                "{value}"
            );
        }
        for (key, value) in [
            ("x-identity-auth-chain-10", json!("")),
            ("x-identity-auth-chain-3", json!("")),
            ("x-identity-auth-chain-3", json!([""])),
        ] {
            let frame = edited(|obj| {
                obj.insert(key.into(), value.clone());
                obj.insert("x-identity-auth-chain-4".into(), json!("not a link"));
            })
            .await;
            let got = verify(&frame, "get", "/")
                .await
                .unwrap_or_else(|e| panic!("{key} {value}: {e:?}"));
            assert_eq!(got.signer, signer);
        }
    }

    /// A re-cased duplicate passes the type check and is never read.
    #[tokio::test]
    async fn a_re_cased_identity_header_is_never_read() {
        let (signer, _) = frame("/", NOW_MS).await;
        for (key, value) in [
            ("X-Identity-Timestamp", json!("0")),
            (
                "X-Identity-Metadata",
                json!(r#"{"signer":"decentraland-kernel-scene"}"#),
            ),
            ("X-Identity-Auth-Chain-0", json!("not a link")),
        ] {
            let frame = edited(|obj| {
                obj.insert(key.to_string(), value.clone());
            })
            .await;
            let got = verify(&frame, "get", "/")
                .await
                .unwrap_or_else(|e| panic!("{key} {value}: {e:?}"));
            assert_eq!(got.signer, signer);
        }
    }

    #[tokio::test]
    async fn a_scene_signed_frame_is_refused_as_malformed() {
        let ts = NOW_MS.to_string();
        for delivered in [
            r#"{"signer":"decentraland-kernel-scene"}"#,
            r#"{"Signer":"decentraland-kernel-scene"}"#,
        ] {
            let signed = build_payload_v6("get", "/", &ts, delivered);
            let (_, frame) = frame_with(&ROOT_WALLET, NOW_MS, &signed, delivered).await;
            assert_eq!(status(verify(&frame, "get", "/").await), 400, "{delivered}");
        }
    }

    #[tokio::test]
    async fn the_socket_takes_the_current_payload_only() {
        let ts = NOW_MS.to_string();
        let cased = r#"{"signer":"dcl:explorer","sceneId":"bafkreiAbC123"}"#;
        let v6 = build_payload_v6("get", "/", &ts, cased);
        let (_, frame) = frame_with(&ROOT_WALLET, NOW_MS, &v6, cased).await;
        assert!(verify(&frame, "get", "/").await.is_ok());
        let legacy = build_legacy_payload("get", "/", &ts, cased);
        let (_, frame) = frame_with(&ROOT_WALLET, NOW_MS, &legacy, cased).await;
        assert_eq!(status(verify(&frame, "get", "/").await), 401);
    }

    #[tokio::test]
    async fn unparseable_metadata_is_refused_as_malformed() {
        let ts = NOW_MS.to_string();
        for raw in ["not json", "[]", "42"] {
            let signed = build_payload_v6("get", "/", &ts, raw);
            let (_, frame) = frame_with(&ROOT_WALLET, NOW_MS, &signed, raw).await;
            assert_eq!(status(verify(&frame, "get", "/").await), 400, "{raw}");
        }
    }
}
