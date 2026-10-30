//! The signed-fetch credential of the storage routes (ADR-44): an
//! `x-identity-*` auth chain over `method:path:timestamp:metadata`, verified
//! through `crate::auth` (the one chain verifier) with the old world-storage
//! posture: a 60 s freshness window, the scene-signed request
//! (`{"signer": "decentraland-kernel-scene"}`) refused before any crypto
//! runs, the crypto-middleware 6.x payload first and the legacy folded one
//! only when every key this surface authorizes on is delivered in its
//! declared spelling. The status codes and message texts are upstream
//! worlds-content-server's, pinned by the tests.

use axum::http::HeaderMap;
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::auth::eip1654::Eip1654Validator;
use crate::auth::{AuthChain, AuthError};
use crate::shared::identity::Address;
use crate::shared::signed_fetch::field_is_canonical;
use crate::shared::signed_fetch::{
    self, AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER, SCENE_SIGNER, SignedFetchError,
    build_legacy_payload, build_payload_v6, check_symmetric_skew, header_str,
};

pub const ONE_MINUTE: i64 = 60;

/// The metadata fields this surface authorizes on or derives the scene
/// context from. Only these are pinned to their declared spelling on the
/// legacy payload, where folding leaves key casing outside the signature; a
/// field a handler starts reading belongs here too.
pub const CANONICAL_METADATA_KEYS: &[&str] = &[
    "signer",
    "realmName",
    "realm.serverName",
    "sceneId",
    "parcel",
];

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SceneAuthMetadata {
    #[serde(default)]
    pub realm: Option<RealmField>,
    #[serde(rename = "realmName", default)]
    pub realm_name: Option<String>,
    #[serde(default)]
    pub parcel: Option<String>,
    #[serde(rename = "sceneId", default)]
    pub scene_id: Option<String>,
    #[serde(default)]
    pub signer: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RealmField {
    #[serde(rename = "serverName", default)]
    pub server_name: Option<String>,
}

/// world-storage's wire mapping over the chain errors: exact upstream
/// worlds-content-server status codes and message text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthChainError {
    MalformedChain {
        detail: String,
    },
    InsufficientLinks,
    InvalidTimestamp(String),
    Expired {
        signed_at: i64,
        now: i64,
        window_secs: i64,
    },
    InvalidSignature(String),
    EipNotImplemented,
    CatalystUnavailable(String),
    SceneSignerRejected,
}

impl AuthChainError {
    pub fn status_code(&self) -> u16 {
        match self {
            Self::MalformedChain { .. }
            | Self::InsufficientLinks
            | Self::InvalidTimestamp(_)
            | Self::SceneSignerRejected => 400,
            Self::Expired { .. } | Self::InvalidSignature(_) => 401,
            Self::EipNotImplemented | Self::CatalystUnavailable(_) => 503,
        }
    }

    pub fn raw_message(&self) -> String {
        match self {
            Self::MalformedChain { detail } => format!("Invalid chain format: {detail}"),
            Self::InsufficientLinks => "Invalid Auth Chain".to_string(),
            Self::InvalidTimestamp(value) => format!("Invalid chain timestamp: {value}"),
            Self::Expired {
                signed_at,
                now,
                window_secs,
            } => format!(
                "Expired signature: signature timestamp: {signed_at}, timestamp expiration: {}, local timestamp: {now}",
                signed_at + window_secs
            ),
            Self::InvalidSignature(detail) => format!("Invalid signature: {detail}"),
            Self::EipNotImplemented => "EIP-1654 chains need an RPC validator".to_string(),
            Self::CatalystUnavailable(detail) => format!("Error connecting to catalyst: {detail}"),
            Self::SceneSignerRejected => "Invalid metadata".to_string(),
        }
    }
}

impl From<SignedFetchError> for AuthChainError {
    fn from(e: SignedFetchError) -> Self {
        match e {
            SignedFetchError::MalformedChain { detail } => Self::MalformedChain { detail },
            SignedFetchError::InsufficientLinks => Self::InsufficientLinks,
            SignedFetchError::InvalidTimestamp(v) => Self::InvalidTimestamp(v),
            SignedFetchError::Expired {
                signed_at,
                now,
                window_secs,
            } => Self::Expired {
                signed_at,
                now,
                window_secs,
            },
            SignedFetchError::InvalidSignature(d) => Self::InvalidSignature(d),
            SignedFetchError::EipNotImplemented => Self::EipNotImplemented,
            SignedFetchError::CatalystUnavailable(d) => Self::CatalystUnavailable(d),
        }
    }
}

fn map_auth_error(err: AuthError) -> AuthChainError {
    match err {
        AuthError::MalformedChain(d) => AuthChainError::MalformedChain { detail: d },
        AuthError::MissingSignature { .. } => AuthChainError::MalformedChain {
            detail: err.to_string(),
        },
        AuthError::RecoveryFailed(d) => AuthChainError::InvalidSignature(d),
        AuthError::SignerMismatch { .. } | AuthError::FinalAuthorityMismatch { .. } => {
            AuthChainError::InvalidSignature(err.to_string())
        }
        AuthError::EphemeralExpired {
            expiration_ms,
            now_ms,
        } => AuthChainError::Expired {
            signed_at: expiration_ms / 1000,
            now: now_ms / 1000,
            window_secs: 0,
        },
        AuthError::InvalidEphemeralPayload(d) => AuthChainError::MalformedChain { detail: d },
        AuthError::Eip1654NotImplemented => AuthChainError::EipNotImplemented,
        AuthError::Eip1654Rejected { .. } => AuthChainError::InvalidSignature(err.to_string()),
        AuthError::Eip1654ValidationFailed(d) => AuthChainError::CatalystUnavailable(d),
    }
}

/// Fail closed, like the shared signed-fetch path: a timestamp that is not
/// plain integer milliseconds is rejected, not silently skipped. The one
/// lenient value (an absent or empty header, which upstream reads as zero)
/// is applied where the header is read.
pub fn check_freshness(
    timestamp: &str,
    expiration_secs: i64,
    now: i64,
) -> Result<(), AuthChainError> {
    check_symmetric_skew(timestamp, now, expiration_secs).map_err(AuthChainError::from)
}

pub async fn validate_signature(
    chain: &AuthChain,
    payload: &str,
    timestamp: &str,
    expiration_secs: i64,
    now: i64,
    eip1654_validator: Option<&dyn Eip1654Validator>,
) -> Result<Address, AuthChainError> {
    check_freshness(timestamp, expiration_secs, now)?;
    crate::auth::verify_auth_chain_async(chain, payload, Some(now * 1000), eip1654_validator)
        .await
        .map_err(map_auth_error)?;
    Address::parse(&chain[0].payload).map_err(|_| AuthChainError::MalformedChain {
        detail: "SIGNER link is not an address".to_string(),
    })
}

pub struct VerifiedRequest {
    pub signer: Address,
    pub metadata: SceneAuthMetadata,
}

fn own_string<'a>(metadata: &'a Value, field: &str) -> Option<&'a str> {
    metadata.as_object()?.get(field)?.as_str()
}

/// A scene-signed request must never reach this surface. The gate refuses a
/// `signer` that is not already canonical rather than folding before
/// comparing: padding survives the signature, so a folded comparison would
/// read ` decentraland-kernel-scene` as "not a scene" and serve it as an
/// ordinary user-signed request.
fn scene_signer_permits(metadata: &Value) -> bool {
    field_is_canonical(metadata, "signer") && own_string(metadata, "signer") != Some(SCENE_SIGNER)
}

/// Upstream's `verifyMetadata` refuses an unparseable or non-object metadata
/// header outright, before any signature work; only an explicit JSON `null`
/// degrades to an empty object.
fn metadata_object(raw: &str) -> Result<Value, AuthChainError> {
    let refused = || AuthChainError::MalformedChain {
        detail: format!("invalid chain metadata: \"{}\"", truncate_detail(raw)),
    };
    match serde_json::from_str::<Value>(raw) {
        Ok(Value::Null) => Ok(Value::Object(Map::new())),
        Ok(value @ Value::Object(_)) => Ok(value),
        _ => Err(refused()),
    }
}

fn truncate_detail(value: &str) -> String {
    const DETAIL_MAX_CHARS: usize = 64;
    if value.chars().count() > DETAIL_MAX_CHARS {
        let head: String = value.chars().take(DETAIL_MAX_CHARS).collect();
        format!("{head}...")
    } else {
        value.to_string()
    }
}

fn objects_to_inspect<'a>(value: &'a Value, out: &mut Vec<&'a Map<String, Value>>) {
    match value {
        Value::Array(items) => items.iter().for_each(|item| objects_to_inspect(item, out)),
        Value::Object(object) => out.push(object),
        _ => {}
    }
}

/// The legacy payload folds the metadata, so key casing sits outside the
/// signature: every declared key (dotted paths descend into objects and
/// arrays) must be delivered in exactly its declared spelling, once.
pub fn legacy_metadata_keys_are_canonical(metadata: &Value, canonical_keys: &[&str]) -> bool {
    for declared_path in canonical_keys {
        let mut containers: Vec<&Value> = vec![metadata];
        for segment in declared_path.split('.') {
            let folded = segment.to_lowercase();
            let mut objects: Vec<&Map<String, Value>> = Vec::new();
            for container in &containers {
                objects_to_inspect(container, &mut objects);
            }
            let mut next: Vec<&Value> = Vec::new();
            for object in objects {
                let delivered: Vec<&String> = object
                    .keys()
                    .filter(|key| key.to_lowercase() == folded)
                    .collect();
                if delivered.is_empty() {
                    continue;
                }
                if delivered.len() > 1 || delivered[0].as_str() != segment {
                    return false;
                }
                if let Some(value) = object.get(segment) {
                    next.push(value);
                }
            }
            if next.is_empty() {
                break;
            }
            containers = next;
        }
    }
    true
}

/// The one verdict a path attempt may move past is a signature comparison
/// failure; every other failure is deterministic in the path.
async fn validate_over_paths(
    chain: &AuthChain,
    payloads: &[String],
    ts: &str,
    now: i64,
    validator: Option<&dyn Eip1654Validator>,
) -> Result<Address, AuthChainError> {
    let mut last = AuthChainError::InvalidSignature("no payload verified".to_string());
    for payload in payloads {
        match validate_signature(chain, payload, ts, ONE_MINUTE, now, validator).await {
            Ok(signer) => return Ok(signer),
            Err(e @ AuthChainError::InvalidSignature(_)) => last = e,
            Err(other) => return Err(other),
        }
    }
    Err(last)
}

/// Stage order as upstream's `verify()`: chain, freshness, metadata parse,
/// the scene gate, then the signature: 6.x over every candidate path, the
/// legacy shape only behind the key guard. `paths` are the candidates in
/// order (`mod.rs::signed_paths`).
pub async fn verify_request(
    headers: &HeaderMap,
    method: &str,
    paths: &[String],
    eip1654_validator: Option<&dyn Eip1654Validator>,
) -> Result<VerifiedRequest, AuthChainError> {
    let chain = signed_fetch::extract_auth_chain(headers)?;
    let ts = header_str(headers, AUTH_TIMESTAMP_HEADER)
        .filter(|raw| !raw.is_empty())
        .unwrap_or("0")
        .to_string();
    let now = chrono::Utc::now().timestamp();
    check_freshness(&ts, ONE_MINUTE, now)?;
    let metadata_raw = header_str(headers, AUTH_METADATA_HEADER)
        .unwrap_or("{}")
        .to_string();

    let metadata_value = metadata_object(&metadata_raw)?;
    if !scene_signer_permits(&metadata_value) {
        return Err(AuthChainError::SceneSignerRejected);
    }

    let v6: Vec<String> = paths
        .iter()
        .map(|p| build_payload_v6(method, p, &ts, &metadata_raw))
        .collect();
    let signer = match validate_over_paths(&chain, &v6, &ts, now, eip1654_validator).await {
        Ok(signer) => signer,
        Err(AuthChainError::InvalidSignature(_)) => {
            if !legacy_metadata_keys_are_canonical(&metadata_value, CANONICAL_METADATA_KEYS) {
                return Err(AuthChainError::MalformedChain {
                    detail: "legacy payload metadata key is not in its declared spelling"
                        .to_string(),
                });
            }
            let legacy: Vec<String> = paths
                .iter()
                .map(|p| build_legacy_payload(method, p, &ts, &metadata_raw))
                .collect();
            if legacy == v6 {
                return Err(AuthChainError::InvalidSignature(
                    "signature does not verify over the request".to_string(),
                ));
            }
            validate_over_paths(&chain, &legacy, &ts, now, eip1654_validator).await?
        }
        Err(err) => return Err(err),
    };

    let metadata: SceneAuthMetadata = serde_json::from_str(&metadata_raw).unwrap_or_default();
    Ok(VerifiedRequest { signer, metadata })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const NOW: i64 = 1_700_000_000;

    fn ts_ms(secs_ago: i64) -> String {
        ((NOW - secs_ago) * 1000).to_string()
    }

    #[test]
    fn freshness_uses_one_minute_window() {
        assert_eq!(ONE_MINUTE, 60);
        assert!(check_freshness(&ts_ms(59), ONE_MINUTE, NOW).is_ok());
        assert!(check_freshness(&ts_ms(60), ONE_MINUTE, NOW).is_ok());
        assert!(matches!(
            check_freshness(&ts_ms(61), ONE_MINUTE, NOW).unwrap_err(),
            AuthChainError::Expired { .. }
        ));
        assert!(matches!(
            check_freshness(&ts_ms(4 * 60), ONE_MINUTE, NOW).unwrap_err(),
            AuthChainError::Expired { .. }
        ));
        assert!(check_freshness(&ts_ms(-60), ONE_MINUTE, NOW).is_ok());
        assert!(matches!(
            check_freshness(&ts_ms(-61), ONE_MINUTE, NOW).unwrap_err(),
            AuthChainError::Expired { .. }
        ));
        // Symmetric: a signature dated far in the future must not stay replayable forever.
        assert!(matches!(
            check_freshness(&ts_ms(-10_000), ONE_MINUTE, NOW).unwrap_err(),
            AuthChainError::Expired { .. }
        ));
    }

    /// The window used to be skipped whenever the timestamp did not parse,
    /// minting a credential that never expired.
    #[test]
    fn freshness_rejects_timestamps_that_are_not_plain_integer_milliseconds() {
        for raw in [
            "",
            "not-a-number",
            "1.7e12",
            "1700000000000.0",
            "Infinity",
            "NaN",
            " 1700000000000",
        ] {
            let err = check_freshness(raw, ONE_MINUTE, NOW).unwrap_err();
            assert!(
                matches!(&err, AuthChainError::InvalidTimestamp(value) if value == raw),
                "timestamp {raw:?} produced {err:?}"
            );
        }
    }

    #[test]
    fn status_code_maps_to_upstream_request_error_codes() {
        assert_eq!(
            AuthChainError::MalformedChain {
                detail: "bad json".into()
            }
            .status_code(),
            400
        );
        assert_eq!(AuthChainError::InsufficientLinks.status_code(), 400);
        assert_eq!(
            AuthChainError::InvalidTimestamp("abc".into()).status_code(),
            400
        );
        assert_eq!(AuthChainError::SceneSignerRejected.status_code(), 400);
        assert_eq!(
            AuthChainError::Expired {
                signed_at: 0,
                now: 100,
                window_secs: 60
            }
            .status_code(),
            401
        );
        assert_eq!(
            AuthChainError::InvalidSignature("nope".into()).status_code(),
            401
        );
        assert_eq!(AuthChainError::EipNotImplemented.status_code(), 503);
        assert_eq!(
            AuthChainError::CatalystUnavailable("rpc down".into()).status_code(),
            503
        );
    }

    #[test]
    fn raw_message_mirrors_upstream_error_text() {
        assert_eq!(
            AuthChainError::MalformedChain {
                detail: "unexpected token".into()
            }
            .raw_message(),
            "Invalid chain format: unexpected token"
        );
        assert_eq!(
            AuthChainError::InsufficientLinks.raw_message(),
            "Invalid Auth Chain"
        );
        assert_eq!(
            AuthChainError::InvalidTimestamp("xyz".into()).raw_message(),
            "Invalid chain timestamp: xyz"
        );
        assert_eq!(
            AuthChainError::SceneSignerRejected.raw_message(),
            "Invalid metadata"
        );
        assert!(
            AuthChainError::InvalidSignature("recovery failed".into())
                .raw_message()
                .starts_with("Invalid signature: ")
        );
        assert_eq!(
            AuthChainError::Expired {
                signed_at: 0,
                now: 100,
                window_secs: 60
            }
            .raw_message(),
            "Expired signature: signature timestamp: 0, timestamp expiration: 60, local timestamp: 100"
        );
    }

    #[test]
    fn scene_signer_is_rejected_in_every_spelling_and_padding() {
        assert!(!scene_signer_permits(
            &json!({ "signer": "decentraland-kernel-scene" })
        ));
        // A re-cased spelling is refused as non-canonical, one layer earlier and without any crypto.
        assert!(!scene_signer_permits(
            &json!({ "signer": "Decentraland-Kernel-Scene" })
        ));
        assert!(!scene_signer_permits(
            &json!({ "signer": "DECENTRALAND-KERNEL-SCENE" })
        ));
        // Padding is signature-bound, so no folded comparison can catch it.
        assert!(!scene_signer_permits(
            &json!({ "signer": " decentraland-kernel-scene" })
        ));
        assert!(!scene_signer_permits(
            &json!({ "signer": "decentraland-kernel-scene " })
        ));
        assert!(!scene_signer_permits(
            &json!({ "signer": "\tdecentraland-kernel-scene" })
        ));
        // Non-canonical signers are refused even when they are not a scene.
        assert!(!scene_signer_permits(&json!({ "signer": "0xAbC" })));
        assert!(!scene_signer_permits(&json!({ "signer": 42 })));
        assert!(!scene_signer_permits(
            &json!({ "Signer": "x", "signer": "dcl:authoritative-server" })
        ));
        // Canonical non-scene signers and absence pass.
        assert!(scene_signer_permits(
            &json!({ "signer": "dcl:authoritative-server" })
        ));
        assert!(scene_signer_permits(&json!({ "signer": "0xabc" })));
        assert!(scene_signer_permits(
            &json!({ "realmName": "some.dcl.eth" })
        ));
        assert!(scene_signer_permits(&json!({})));
    }

    /// Upstream refuses this delivery at the metadata-parse stage with a 400,
    /// so the gate and the legacy key guard never run over a coerced empty object.
    #[test]
    fn non_object_metadata_is_refused_before_the_gate() {
        for raw in ["not json", "[1,2]", "\"decentraland-kernel-scene\"", "5"] {
            let err = metadata_object(raw).unwrap_err();
            assert_eq!(err.status_code(), 400);
            assert_eq!(
                err.raw_message(),
                format!("Invalid chain format: invalid chain metadata: \"{raw}\"")
            );
        }
        let value = metadata_object("null").expect("null metadata must degrade");
        assert_eq!(value, json!({}));
        assert!(scene_signer_permits(&value));
        let raw = "x".repeat(200);
        let AuthChainError::MalformedChain { detail } = metadata_object(&raw).unwrap_err() else {
            panic!()
        };
        assert!(
            detail.contains("...") && detail.len() < raw.len(),
            "{detail}"
        );
    }

    /// The 6.x payload binds the metadata bytes: only method and path fold.
    #[test]
    fn v6_payload_keeps_metadata_casing() {
        let metadata = r#"{"realmName":"Some.dcl.eth","sceneId":"bafkreiABC"}"#;
        assert_eq!(
            build_payload_v6("PUT", "/Values/K", "123", metadata),
            format!("put:/values/k:123:{metadata}")
        );
        assert_ne!(
            build_payload_v6("PUT", "/Values/K", "123", metadata),
            build_legacy_payload("PUT", "/Values/K", "123", metadata)
        );
    }

    #[test]
    fn legacy_fallback_refuses_respelled_authorized_keys() {
        for respelled in [
            json!({ "Signer": "dcl:authoritative-server" }),
            json!({ "RealmName": "some.dcl.eth" }),
            json!({ "realm": { "ServerName": "some.dcl.eth" } }),
            json!({ "SceneId": "bafkrei" }),
            json!({ "Parcel": "0,0" }),
            json!({ "sceneId": "a", "sceneid": "b" }),
        ] {
            assert!(
                !legacy_metadata_keys_are_canonical(&respelled, CANONICAL_METADATA_KEYS),
                "{respelled} must not be accepted on the legacy payload"
            );
        }
    }

    #[test]
    fn legacy_fallback_accepts_the_metadata_our_scene_runtime_signs() {
        let metadata = json!({
            "origin": "catalyrst-scene-state://",
            "signer": "dcl:authoritative-server",
            "isGuest": false,
            "realmName": "some.dcl.eth",
            "realm": { "serverName": "some.dcl.eth" },
            "sceneId": "bafkreiabc",
            "parcel": "0,0",
        });
        assert!(legacy_metadata_keys_are_canonical(
            &metadata,
            CANONICAL_METADATA_KEYS
        ));
        assert!(
            legacy_metadata_keys_are_canonical(&json!({ "Other": 1 }), CANONICAL_METADATA_KEYS),
            "undeclared keys are free"
        );
    }

    #[test]
    fn rpc_validation_failure_is_catalyst_unavailable_503() {
        let mapped = map_auth_error(AuthError::Eip1654ValidationFailed("RPC timeout".into()));
        assert!(matches!(mapped, AuthChainError::CatalystUnavailable(_)));
        assert_eq!(mapped.status_code(), 503);
        assert_eq!(
            map_auth_error(AuthError::Eip1654NotImplemented).status_code(),
            503
        );
        assert_eq!(
            map_auth_error(AuthError::RecoveryFailed("x".into())).status_code(),
            401
        );
    }
}
