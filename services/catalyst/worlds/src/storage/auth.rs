//! The signed-fetch credential of the storage routes (ADR-44): an
//! `x-identity-*` auth chain over `method:path:timestamp:metadata`, verified
//! through `crate::auth` (the one chain verifier) as world-storage-service's
//! `@dcl/crypto-middleware` 6.3 verifies it: the timestamp read as
//! `Number(header)` and held to 60 000 ms either side of now, the metadata
//! parsed as `JSON.parse` does, the scene-signed request
//! (`{"signer": "decentraland-kernel-scene"}`) refused before any crypto
//! runs, the 6.x payload first (an absent metadata header signs as the empty
//! string) and the legacy folded one only when every key this surface
//! authorizes on is delivered in its declared spelling. The status codes and
//! message texts are upstream's, pinned by the tests. The timestamp rule is
//! this surface's own: the shared `dcl_auth::signed_fetch` window also serves
//! services ported from other upstreams.

use axum::http::HeaderMap;

use crate::auth::eip1654::Eip1654Validator;
use crate::auth::{AuthChain, AuthError};
use crate::shared::identity::Address;
use crate::shared::signed_fetch::{
    self, AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER, SCENE_SIGNER, SignedFetchError,
    build_legacy_payload, build_payload_v6, header_str,
};

use super::js::{JsObject, JsValue, js_trim, number_from_str, number_to_string, safe};

/// crypto-middleware's `DEFAULT_EXPIRATION`.
pub const EXPIRATION_MS: f64 = 60_000.0;

/// The metadata fields this surface authorizes on or derives the scene
/// context from, in upstream's order (the first refused key names the
/// error). Only these are pinned to their declared spelling on the legacy
/// payload, where folding leaves key casing outside the signature; a field a
/// handler starts reading belongs here too.
pub const CANONICAL_METADATA_KEYS: &[&str] = &[
    "signer",
    "realmName",
    "realm.serverName",
    "parcel",
    "sceneId",
];

/// The delivered metadata object, read field by field as upstream reads it:
/// a field of an unexpected type affects only the reads of that field.
#[derive(Debug, Clone, Default)]
pub struct SceneAuthMetadata(JsObject);

impl SceneAuthMetadata {
    /// `JSON.parse` of a metadata object.
    pub fn parse(raw: &str) -> Option<Self> {
        match JsValue::parse(raw)? {
            JsValue::Object(o) => Some(Self(o)),
            _ => None,
        }
    }

    pub fn get(&self, field: &str) -> Option<&JsValue> {
        self.0.get(field)
    }

    /// `realm?.serverName || realmName`: the first truthy value, any type.
    pub fn realm(&self) -> Option<&JsValue> {
        let server_name = match self.get("realm") {
            Some(JsValue::Object(realm)) => realm.get("serverName"),
            _ => None,
        };
        server_name
            .filter(|v| v.truthy())
            .or_else(|| self.get("realmName").filter(|v| v.truthy()))
    }

    /// `parcel || undefined`.
    pub fn parcel(&self) -> Option<&JsValue> {
        self.get("parcel").filter(|v| v.truthy())
    }

    /// `sceneId` when it is a string.
    pub fn scene_id(&self) -> Option<&str> {
        self.get("sceneId").and_then(JsValue::as_str)
    }

    /// `sceneId` as the delegation compares it: `""` (never a claim's) when
    /// absent, not a string, or holding a lone surrogate no claim can carry.
    pub fn claim_scene_id(&self) -> &str {
        match self.get("sceneId") {
            Some(JsValue::String(s)) if s.is_well_formed() => s,
            _ => "",
        }
    }
}

/// world-storage's wire mapping over the chain errors: upstream status codes
/// and message text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthChainError {
    MalformedChain {
        detail: String,
    },
    InsufficientLinks,
    /// The timestamp header as received.
    InvalidTimestamp(String),
    /// The whole message: expired, or too far in the future.
    Expired(String),
    /// The whole message: unparseable metadata or a respelled legacy key.
    InvalidMetadata(String),
    InvalidSignature(String),
    EipNotImplemented,
    CatalystUnavailable(String),
    /// The whole `Invalid metadata content: …` message.
    SceneSignerRejected(String),
}

impl AuthChainError {
    pub fn status_code(&self) -> u16 {
        match self {
            Self::MalformedChain { .. }
            | Self::InsufficientLinks
            | Self::InvalidTimestamp(_)
            | Self::InvalidMetadata(_)
            | Self::SceneSignerRejected(_) => 400,
            Self::Expired(_) | Self::InvalidSignature(_) => 401,
            Self::EipNotImplemented | Self::CatalystUnavailable(_) => 503,
        }
    }

    pub fn raw_message(&self) -> String {
        match self {
            Self::MalformedChain { detail } => format!("Invalid chain format: {detail}"),
            Self::InsufficientLinks => "Invalid Auth Chain".to_string(),
            Self::InvalidTimestamp(value) => format!("Invalid chain timestamp: {}", safe(value)),
            Self::Expired(message)
            | Self::InvalidMetadata(message)
            | Self::SceneSignerRejected(message) => message.clone(),
            Self::InvalidSignature(detail) => format!("Invalid signature: {detail}"),
            Self::EipNotImplemented => "EIP-1654 chains need an RPC validator".to_string(),
            Self::CatalystUnavailable(detail) => format!("Error connecting to catalyst: {detail}"),
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
            } => Self::Expired(format!(
                "Expired signature: signature timestamp: {signed_at}, timestamp expiration: {}, local timestamp: {now}",
                signed_at + window_secs
            )),
            SignedFetchError::InvalidSignature(d) => Self::InvalidSignature(d),
            SignedFetchError::EipNotImplemented => Self::EipNotImplemented,
            SignedFetchError::CatalystUnavailable(d) => Self::CatalystUnavailable(d),
        }
    }
}

/// `@dcl/crypto` reports a malformed or expired ephemeral link as a failed
/// validation, which the middleware answers 401 `Invalid signature: …`.
fn map_auth_error(err: AuthError) -> AuthChainError {
    match err {
        AuthError::MalformedChain(d) => AuthChainError::MalformedChain { detail: d },
        AuthError::MissingSignature { .. } => AuthChainError::MalformedChain {
            detail: err.to_string(),
        },
        AuthError::RecoveryFailed(d) => AuthChainError::InvalidSignature(d),
        AuthError::SignerMismatch { .. }
        | AuthError::FinalAuthorityMismatch { .. }
        | AuthError::EphemeralExpired { .. }
        | AuthError::InvalidEphemeralPayload(_)
        | AuthError::Eip1654Rejected { .. } => AuthChainError::InvalidSignature(err.to_string()),
        AuthError::Eip1654NotImplemented => AuthChainError::EipNotImplemented,
        AuthError::Eip1654ValidationFailed(d) => AuthChainError::CatalystUnavailable(d),
    }
}

/// `verifyTimestamp` + `verifyExpiration`: `Number(raw || '0')`, refused
/// when a present value is not finite, then held to [`EXPIRATION_MS`] on
/// either side of `now_ms` (an absent or empty header is 0 and expires).
pub fn check_freshness(raw: Option<&str>, now_ms: i64) -> Result<f64, AuthChainError> {
    let raw = raw.unwrap_or("");
    let timestamp = if raw.is_empty() {
        0.0
    } else {
        number_from_str(raw)
    };
    if !timestamp.is_finite() {
        return Err(AuthChainError::InvalidTimestamp(raw.to_string()));
    }
    let now = now_ms as f64;
    if timestamp + EXPIRATION_MS < now {
        return Err(AuthChainError::Expired(format!(
            "Expired signature: signature timestamp: {}, timestamp expiration: {}, local timestamp: {now_ms}",
            number_to_string(timestamp),
            number_to_string(timestamp + EXPIRATION_MS)
        )));
    }
    if timestamp > now + EXPIRATION_MS {
        return Err(AuthChainError::Expired(format!(
            "Signature timestamp is too far in the future: signature timestamp: {}, local timestamp: {now_ms}",
            number_to_string(timestamp)
        )));
    }
    Ok(timestamp)
}

pub struct VerifiedRequest {
    pub signer: Address,
    pub metadata: SceneAuthMetadata,
}

fn is_canonical(value: &str) -> bool {
    value == js_trim(value).to_lowercase()
}

/// `rejectIfSigner('decentraland-kernel-scene')`: a key that folds to
/// `signer` without being spelled so, a non-string or non-canonical value,
/// and the scene signer are refused; no `signer` passes.
fn scene_signer_permits(metadata: &JsObject) -> bool {
    if metadata
        .keys()
        .any(|k| k != "signer" && k.to_lowercase() == "signer")
    {
        return false;
    }
    match metadata.get("signer") {
        None => true,
        Some(JsValue::String(v)) => is_canonical(v) && v != SCENE_SIGNER,
        Some(_) => false,
    }
}

/// `verifyMetadata`: `JSON.parse(raw ?? '{}')`, a JSON `null` read as an
/// empty object, anything else that is not an object refused.
fn metadata_object(raw: Option<&str>) -> Result<JsObject, AuthChainError> {
    match JsValue::parse(raw.unwrap_or("{}")) {
        Some(JsValue::Null) => Ok(JsObject::default()),
        Some(JsValue::Object(o)) => Ok(o),
        _ => Err(AuthChainError::InvalidMetadata(format!(
            "Invalid chain metadata: \"{}\"",
            safe(raw.unwrap_or(""))
        ))),
    }
}

fn objects_to_inspect<'a>(value: &'a JsValue, out: &mut Vec<&'a JsObject>) {
    match value {
        JsValue::Array(items) => items.iter().for_each(|item| objects_to_inspect(item, out)),
        JsValue::Object(object) => out.push(object),
        _ => {}
    }
}

/// `assertLegacyMetadataKeys`: the legacy payload folds the metadata, so key
/// casing sits outside the signature; every declared key (dotted paths
/// descend into objects and arrays) must be delivered in exactly its
/// declared spelling, once.
pub fn assert_legacy_metadata_keys(
    metadata: &JsObject,
    canonical_keys: &[&str],
) -> Result<(), AuthChainError> {
    let root = JsValue::Object(metadata.clone());
    for declared_path in canonical_keys {
        let mut containers: Vec<&JsValue> = vec![&root];
        for segment in declared_path.split('.') {
            let folded = segment.to_lowercase();
            let mut objects: Vec<&JsObject> = Vec::new();
            for container in &containers {
                objects_to_inspect(container, &mut objects);
            }
            let mut next: Vec<&JsValue> = Vec::new();
            for object in objects {
                let delivered: Vec<&str> = object
                    .keys()
                    .filter(|key| key.to_lowercase() == folded)
                    .collect();
                match delivered.as_slice() {
                    [] => continue,
                    [only] if *only == segment => {}
                    [only] => {
                        return Err(AuthChainError::InvalidMetadata(format!(
                            "Invalid chain metadata: expected \"{}\", got \"{}\"",
                            safe(segment),
                            safe(only)
                        )));
                    }
                    many => {
                        return Err(AuthChainError::InvalidMetadata(format!(
                            "Invalid chain metadata: \"{}\" delivered under {} spellings",
                            safe(segment),
                            many.len()
                        )));
                    }
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
    Ok(())
}

/// The chain's final authority. The walk reads the signed payload only at its
/// last step, the final-authority comparison, so one walk (and its EIP-1654
/// contract calls) answers every candidate payload; a failure before that step
/// is the same for every candidate.
async fn chain_authority(
    chain: &AuthChain,
    probe: &str,
    now_ms: i64,
    validator: Option<&dyn Eip1654Validator>,
) -> Result<String, AuthChainError> {
    match crate::auth::verify_auth_chain_async(chain, probe, Some(now_ms), validator).await {
        Ok(()) => Ok(probe.to_string()),
        Err(AuthError::FinalAuthorityMismatch { actual, .. }) => Ok(actual),
        Err(e) => Err(map_auth_error(e)),
    }
}

/// Upstream tries the candidates in order and answers the last one's refusal.
fn match_payloads(authority: &str, payloads: &[String]) -> Result<(), AuthChainError> {
    if payloads.iter().any(|p| p == authority) {
        return Ok(());
    }
    Err(match payloads.last() {
        Some(last) => map_auth_error(AuthError::FinalAuthorityMismatch {
            expected: last.clone(),
            actual: authority.to_string(),
        }),
        None => AuthChainError::InvalidSignature("no payload verified".to_string()),
    })
}

/// Stage order as upstream's `verify()`: chain, timestamp, freshness,
/// metadata parse, the scene gate, then the signature: 6.x over every
/// candidate path, the legacy shape only behind the key guard. `paths` are
/// the candidates in order (`mod.rs::signed_paths`).
pub async fn verify_request(
    headers: &HeaderMap,
    method: &str,
    paths: &[String],
    eip1654_validator: Option<&dyn Eip1654Validator>,
) -> Result<VerifiedRequest, AuthChainError> {
    let chain = signed_fetch::extract_auth_chain(headers)?;
    let timestamp_raw = header_str(headers, AUTH_TIMESTAMP_HEADER);
    let now_ms = chrono::Utc::now().timestamp_millis();
    check_freshness(timestamp_raw, now_ms)?;
    let metadata_raw = header_str(headers, AUTH_METADATA_HEADER);
    let metadata = metadata_object(metadata_raw)?;
    if !scene_signer_permits(&metadata) {
        return Err(AuthChainError::SceneSignerRejected(format!(
            "Invalid metadata content: {}",
            safe(&JsValue::Object(metadata).stringify())
        )));
    }

    // The payload joins the raw headers: an absent one is the empty string.
    let ts = timestamp_raw.unwrap_or("").to_lowercase();
    let signed_metadata = metadata_raw.unwrap_or("");
    let v6: Vec<String> = paths
        .iter()
        .map(|p| build_payload_v6(method, p, &ts, signed_metadata))
        .collect();
    let probe = v6.first().map_or("", String::as_str);
    let authority = chain_authority(&chain, probe, now_ms, eip1654_validator).await;
    let v6_verdict = match &authority {
        Ok(authority) => match_payloads(authority, &v6),
        Err(err) => Err(err.clone()),
    };
    match v6_verdict {
        Ok(()) => {}
        Err(err @ AuthChainError::InvalidSignature(_)) => {
            assert_legacy_metadata_keys(&metadata, CANONICAL_METADATA_KEYS)?;
            let legacy: Vec<String> = paths
                .iter()
                .map(|p| build_legacy_payload(method, p, &ts, signed_metadata))
                .collect();
            if legacy == v6 {
                return Err(err);
            }
            // A chain failure answers the legacy attempt the same way.
            match_payloads(&authority?, &legacy)?;
        }
        Err(err) => return Err(err),
    }
    let signer = Address::parse(&chain[0].payload).map_err(|_| AuthChainError::MalformedChain {
        detail: "SIGNER link is not an address".to_string(),
    })?;

    Ok(VerifiedRequest {
        signer,
        metadata: SceneAuthMetadata(metadata),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW_MS: i64 = 1_700_000_000_000;

    fn obj(raw: &str) -> JsObject {
        match JsValue::parse(raw).unwrap() {
            JsValue::Object(o) => o,
            other => panic!("not an object: {other:?}"),
        }
    }

    fn at(offset_ms: i64) -> String {
        (NOW_MS + offset_ms).to_string()
    }

    #[test]
    fn freshness_is_sixty_seconds_either_side_in_milliseconds() {
        for offset in [0, -60_000, 60_000, -59_999, 59_999] {
            assert!(
                check_freshness(Some(&at(offset)), NOW_MS).is_ok(),
                "{offset}"
            );
        }
        assert_eq!(
            check_freshness(Some(&at(-60_001)), NOW_MS).unwrap_err(),
            AuthChainError::Expired(
                "Expired signature: signature timestamp: 1699999939999, timestamp expiration: 1699999999999, local timestamp: 1700000000000".into()
            )
        );
        assert_eq!(
            check_freshness(Some(&at(60_001)), NOW_MS).unwrap_err(),
            AuthChainError::Expired(
                "Signature timestamp is too far in the future: signature timestamp: 1700000060001, local timestamp: 1700000000000".into()
            )
        );
        // Not truncated to seconds: 999 ms past the window is already expired.
        assert!(check_freshness(Some(&at(-60_999)), NOW_MS).is_err());
    }

    /// `Number(raw)`: any finite numeric literal is a timestamp.
    #[test]
    fn freshness_reads_the_timestamp_as_javascript_number() {
        for raw in [
            "1.7e12",
            "1700000000000.0",
            " 1700000000000",
            "1700000000000\n",
            "+1700000000000",
            "1700000000000.5",
        ] {
            assert!(check_freshness(Some(raw), NOW_MS).is_ok(), "{raw:?}");
        }
        assert_eq!(
            check_freshness(Some("1699999939999.5"), NOW_MS).unwrap_err(),
            AuthChainError::Expired(
                "Expired signature: signature timestamp: 1699999939999.5, timestamp expiration: 1699999999999.5, local timestamp: 1700000000000".into()
            )
        );
        for raw in ["not-a-number", "Infinity", "NaN", "1e400", "12px"] {
            assert_eq!(
                check_freshness(Some(raw), NOW_MS).unwrap_err(),
                AuthChainError::InvalidTimestamp(raw.to_string()),
                "{raw:?}"
            );
        }
        // Absent, empty and blank read as 0, which is long expired.
        for raw in [None, Some(""), Some("   ")] {
            assert!(matches!(
                check_freshness(raw, NOW_MS).unwrap_err(),
                AuthChainError::Expired(_)
            ));
        }
        let long = "x".repeat(100);
        assert_eq!(
            AuthChainError::InvalidTimestamp(long).raw_message(),
            format!("Invalid chain timestamp: {}…", "x".repeat(64))
        );
    }

    #[test]
    fn status_code_maps_to_upstream_request_error_codes() {
        let cases = [
            (
                AuthChainError::MalformedChain {
                    detail: "bad json".into(),
                },
                400,
            ),
            (AuthChainError::InsufficientLinks, 400),
            (AuthChainError::InvalidTimestamp("abc".into()), 400),
            (AuthChainError::InvalidMetadata("m".into()), 400),
            (AuthChainError::SceneSignerRejected("m".into()), 400),
            (AuthChainError::Expired("m".into()), 401),
            (AuthChainError::InvalidSignature("nope".into()), 401),
            (AuthChainError::EipNotImplemented, 503),
            (AuthChainError::CatalystUnavailable("rpc down".into()), 503),
        ];
        for (err, status) in cases {
            assert_eq!(err.status_code(), status, "{err:?}");
        }
    }

    #[test]
    fn scene_signer_is_rejected_in_every_spelling_and_padding() {
        for refused in [
            r#"{"signer":"decentraland-kernel-scene"}"#,
            r#"{"signer":"Decentraland-Kernel-Scene"}"#,
            r#"{"signer":" decentraland-kernel-scene"}"#,
            r#"{"signer":"decentraland-kernel-scene "}"#,
            r#"{"signer":"\tdecentraland-kernel-scene"}"#,
            r#"{"signer":"0xAbC"}"#,
            r#"{"signer":42}"#,
            r#"{"Signer":"x","signer":"dcl:authoritative-server"}"#,
        ] {
            assert!(!scene_signer_permits(&obj(refused)), "{refused}");
        }
        for permitted in [
            r#"{"signer":"dcl:authoritative-server"}"#,
            r#"{"signer":"0xabc"}"#,
            r#"{"realmName":"some.dcl.eth"}"#,
            "{}",
        ] {
            assert!(scene_signer_permits(&obj(permitted)), "{permitted}");
        }
    }

    /// Upstream refuses this delivery at the metadata-parse stage with a 400,
    /// so the gate and the legacy key guard never run over a coerced empty object.
    #[test]
    fn non_object_metadata_is_refused_before_the_gate() {
        for raw in [
            "not json",
            "[1,2]",
            "\"decentraland-kernel-scene\"",
            "5",
            "",
        ] {
            let err = metadata_object(Some(raw)).unwrap_err();
            assert_eq!(err.status_code(), 400);
            assert_eq!(
                err.raw_message(),
                format!("Invalid chain metadata: \"{raw}\"")
            );
        }
        assert_eq!(metadata_object(Some("null")).unwrap(), JsObject::default());
        assert_eq!(metadata_object(None).unwrap(), JsObject::default());
        let raw = "x".repeat(200);
        assert_eq!(
            metadata_object(Some(&raw)).unwrap_err().raw_message(),
            format!("Invalid chain metadata: \"{}…\"", "x".repeat(64))
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
    fn legacy_fallback_refuses_respelled_authorized_keys_with_upstream_text() {
        for (respelled, message) in [
            (
                r#"{"Signer":"dcl:authoritative-server"}"#,
                r#"Invalid chain metadata: expected "signer", got "Signer""#,
            ),
            (
                r#"{"RealmName":"some.dcl.eth"}"#,
                r#"Invalid chain metadata: expected "realmName", got "RealmName""#,
            ),
            (
                r#"{"realm":{"ServerName":"some.dcl.eth"}}"#,
                r#"Invalid chain metadata: expected "serverName", got "ServerName""#,
            ),
            (
                r#"{"SceneId":"bafkrei"}"#,
                r#"Invalid chain metadata: expected "sceneId", got "SceneId""#,
            ),
            (
                r#"{"Parcel":"0,0"}"#,
                r#"Invalid chain metadata: expected "parcel", got "Parcel""#,
            ),
            (
                r#"{"sceneId":"a","sceneid":"b"}"#,
                r#"Invalid chain metadata: "sceneId" delivered under 2 spellings"#,
            ),
            // Upstream's order: parcel is checked before sceneId.
            (
                r#"{"SceneId":"a","Parcel":"0,0"}"#,
                r#"Invalid chain metadata: expected "parcel", got "Parcel""#,
            ),
        ] {
            let err =
                assert_legacy_metadata_keys(&obj(respelled), CANONICAL_METADATA_KEYS).unwrap_err();
            assert_eq!(err.status_code(), 400);
            assert_eq!(err.raw_message(), message, "{respelled}");
        }
    }

    #[test]
    fn legacy_fallback_accepts_the_metadata_our_scene_runtime_signs() {
        let metadata = obj(
            r#"{"origin":"catalyrst-scene-state://","signer":"dcl:authoritative-server","isGuest":false,
                "realmName":"some.dcl.eth","realm":{"serverName":"some.dcl.eth"},"sceneId":"bafkreiabc","parcel":"0,0"}"#,
        );
        assert!(assert_legacy_metadata_keys(&metadata, CANONICAL_METADATA_KEYS).is_ok());
        assert!(
            assert_legacy_metadata_keys(&obj(r#"{"Other":1}"#), CANONICAL_METADATA_KEYS).is_ok(),
            "undeclared keys are free"
        );
    }

    #[test]
    fn chain_failures_answer_as_crypto_middleware_does() {
        let mapped = map_auth_error(AuthError::Eip1654ValidationFailed("RPC timeout".into()));
        assert!(matches!(mapped, AuthChainError::CatalystUnavailable(_)));
        assert_eq!(mapped.status_code(), 503);
        assert_eq!(
            map_auth_error(AuthError::Eip1654NotImplemented).status_code(),
            503
        );
        for err in [
            AuthError::RecoveryFailed("x".into()),
            AuthError::InvalidEphemeralPayload("bad".into()),
            AuthError::EphemeralExpired {
                expiration_ms: 1,
                now_ms: 2,
            },
        ] {
            let mapped = map_auth_error(err);
            assert!(
                matches!(mapped, AuthChainError::InvalidSignature(_)),
                "{mapped:?}"
            );
            assert_eq!(mapped.status_code(), 401);
        }
    }

    #[test]
    fn metadata_fields_read_one_by_one() {
        let m = SceneAuthMetadata::parse(
            r#"{"realm":{"serverName":""},"realmName":"x.dcl.eth","parcel":5,"sceneId":7}"#,
        )
        .unwrap();
        assert_eq!(m.realm(), Some(&JsValue::String("x.dcl.eth".into())));
        assert_eq!(m.parcel(), Some(&JsValue::Number(5.0)));
        assert_eq!(m.scene_id(), None);
        let m = SceneAuthMetadata::parse(
            r#"{"realm":"x","realmName":0,"parcel":"1,2","sceneId":"b","sceneId":"c"}"#,
        )
        .unwrap();
        assert_eq!(m.realm(), None);
        assert_eq!(m.parcel(), Some(&JsValue::String("1,2".into())));
        assert_eq!(m.scene_id(), Some("c"), "JSON.parse keeps the last value");
    }

    #[test]
    fn a_lone_surrogate_scene_id_never_matches_a_claim() {
        let m = SceneAuthMetadata::parse(r#"{"sceneId":"\ud800"}"#).unwrap();
        assert_eq!(m.claim_scene_id(), "");
        let m = SceneAuthMetadata::parse(r#"{"sceneId":"bafy"}"#).unwrap();
        assert_eq!(m.claim_scene_id(), "bafy");
    }

    /// A contract wallet answering every `isValidSignature` with `valid`.
    struct CountingValidator {
        valid: bool,
        calls: std::sync::atomic::AtomicUsize,
    }

    #[async_trait::async_trait]
    impl Eip1654Validator for CountingValidator {
        async fn validate_signature(
            &self,
            _contract_address: &str,
            _hash: &[u8],
            _signature: &[u8],
        ) -> Result<bool, AuthError> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(self.valid)
        }
    }

    const CONTRACT: &str = "0x00000000000000000000000000000000000c0de1";
    // Mixed case, so the legacy (all-lowercase) payload differs from the 6.x one.
    const METADATA: &str = r#"{"sceneId":"bafkreiABC"}"#;

    fn paths() -> Vec<String> {
        ["/world-storage/values/k", "/values/k", "/values/k/"]
            .map(String::from)
            .to_vec()
    }

    /// SIGNER (contract) -> EIP-1654 ephemeral -> the request payload the
    /// ephemeral key signs; `expires_in_ms` places the link's expiration.
    fn contract_wallet_headers(signed: impl Fn(&str) -> String, expires_in_ms: i64) -> HeaderMap {
        let ephemeral = crate::auth::wallet::Wallet::random().unwrap();
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let expiration = (chrono::Utc::now() + chrono::Duration::milliseconds(expires_in_ms))
            .format("%Y-%m-%dT%H:%M:%S%.3fZ");
        let payload = signed(&timestamp);
        let links = [
            serde_json::json!({"type": "SIGNER", "payload": CONTRACT, "signature": ""}),
            serde_json::json!({"type": "ECDSA_EIP_1654_EPHEMERAL",
                "payload": format!("Decentraland Login\nEphemeral address: {}\nExpiration: {expiration}", ephemeral.address()),
                "signature": format!("0x{}", "11".repeat(65))}),
            serde_json::json!({"type": "ECDSA_SIGNED_ENTITY", "payload": payload,
                "signature": ephemeral.sign_message(payload.as_bytes()).unwrap()}),
        ];
        let mut headers = HeaderMap::new();
        for (index, link) in links.iter().enumerate() {
            headers.insert(
                axum::http::HeaderName::try_from(format!("x-identity-auth-chain-{index}")).unwrap(),
                link.to_string().parse().unwrap(),
            );
        }
        headers.insert(AUTH_TIMESTAMP_HEADER, timestamp.parse().unwrap());
        headers.insert(AUTH_METADATA_HEADER, METADATA.parse().unwrap());
        headers
    }

    async fn verify_counting(
        headers: &HeaderMap,
        valid: bool,
    ) -> (Result<VerifiedRequest, AuthChainError>, usize) {
        let validator = CountingValidator {
            valid,
            calls: Default::default(),
        };
        let result = verify_request(headers, "get", &paths(), Some(&validator)).await;
        (result, validator.calls.into_inner())
    }

    #[tokio::test]
    async fn a_rejected_contract_link_is_asked_once_across_every_candidate() {
        let headers = contract_wallet_headers(
            |ts| build_payload_v6("get", "/values/k", ts, METADATA),
            3_600_000,
        );
        let (result, calls) = verify_counting(&headers, false).await;
        // One verify_eip1654: the raw hash, then the EIP-191 hash.
        assert_eq!(calls, 2);
        let Err(err) = result else {
            panic!("a rejected contract link verified")
        };
        assert_eq!(
            err,
            map_auth_error(AuthError::Eip1654Rejected {
                contract: CONTRACT.to_string()
            })
        );
    }

    #[tokio::test]
    async fn an_expired_contract_link_asks_no_contract_and_answers_once() {
        let headers = contract_wallet_headers(
            |ts| build_payload_v6("get", "/values/k", ts, METADATA),
            -1_000,
        );
        let (result, calls) = verify_counting(&headers, true).await;
        assert_eq!(calls, 0);
        let Err(err @ AuthChainError::InvalidSignature(_)) = result else {
            panic!("an expired link verified")
        };
        assert!(
            err.raw_message().contains("Ephemeral key expired"),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn an_accepted_contract_link_is_asked_once_whichever_candidate_matches() {
        for (signed, accepted) in [
            (
                build_payload_v6 as fn(&str, &str, &str, &str) -> String,
                "/values/k/",
            ),
            (build_legacy_payload, "/values/k/"),
            (build_payload_v6, "/elsewhere"),
        ] {
            let headers =
                contract_wallet_headers(|ts| signed("get", accepted, ts, METADATA), 3_600_000);
            let ts = headers[AUTH_TIMESTAMP_HEADER].to_str().unwrap().to_string();
            let (result, calls) = verify_counting(&headers, true).await;
            assert_eq!(calls, 1, "{accepted}");
            if accepted == "/elsewhere" {
                // Upstream's refusal names the last candidate it tried.
                let Err(err) = result else {
                    panic!("a payload no candidate signs verified")
                };
                assert_eq!(
                    err,
                    map_auth_error(AuthError::FinalAuthorityMismatch {
                        expected: build_legacy_payload("get", "/values/k/", &ts, METADATA),
                        actual: signed("get", accepted, &ts, METADATA),
                    })
                );
            } else {
                assert_eq!(result.unwrap().signer.to_string(), CONTRACT);
            }
        }
    }
}
