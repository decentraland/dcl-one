//! The worlds content server's signed-fetch verifier, shared by the worlds
//! routes (`worlds::core::auth` re-exports it) and the world comms mints:
//! [`require_verified`] runs the 60-second window, a per-route list of
//! metadata keys that switches the folded-payload fallback on (empty = the
//! 6.x payload only), the same scene-signer gate, and answers its refusals
//! in the worlds envelope ([`map_auth_error_api`]). The steps it runs (the
//! chain, the timestamp window, the metadata parse, the one path rule) are
//! `shared::signed_fetch`'s.

use axum::http::HeaderMap;
use serde_json::{Map, Value};

use crate::auth::AuthChain;
use crate::auth::eip1654::Eip1654Validator;
use crate::shared::identity::Address;
use crate::shared::signed_fetch::{
    AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER, SignedFetchError, SignedPath,
    build_legacy_payload, build_payload_v6, candidate_paths, check_symmetric_skew,
    extract_auth_chain, header_str, is_refused_signer, map_auth_error, parse_metadata,
    truncate_detail,
};
use crate::worlds_envelope::ApiError;

/// Upstream's `setupRouter` passes no `expiration` to `wellKnownComponents`, so every
/// route on this surface runs on crypto-middleware's `DEFAULT_EXPIRATION` of `1000 * 60`
/// (core-libs/libs/crypto-middleware/src/types.ts). The world-storage surface already
/// reads the same default.
pub const SIGNED_FETCH_EXPIRATION_SECS: i64 = 60;

/// The explorer comms handshakes. The js explorers still sign the folded payload with
/// camelCase metadata while bevy-explorer mints the 6.x payload, so the fallback stays
/// on behind exactly the keys upstream pins. The comms adapter checks signer and intent
/// after verification; re-spelled keys are refused on the same requests upstream refuses. Empty
/// the list once every explorer signs the 6.x payload.
pub const EXPLORER_METADATA_KEYS: &[&str] = &["signer", "intent", "secret"];

#[derive(Debug, Clone)]
pub struct VerifiedAuth {
    pub signer: Address,
    pub metadata: Value,
}

impl VerifiedAuth {
    pub fn secret(&self) -> Option<String> {
        self.metadata
            .get("secret")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }
}

fn objects_to_inspect<'a>(value: &'a Value, out: &mut Vec<&'a Map<String, Value>>) {
    match value {
        Value::Array(items) => items.iter().for_each(|item| objects_to_inspect(item, out)),
        Value::Object(object) => out.push(object),
        _ => {}
    }
}

/// The first declared key whose delivered spelling differs (keys only; values
/// such as `secret` are case-sensitive and travel verbatim).
fn first_respelled_key<'k>(metadata: &Value, canonical_keys: &[&'k str]) -> Option<&'k str> {
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
                    return Some(declared_path);
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
    None
}

async fn validate_over_paths(
    chain: &AuthChain,
    payloads: &[String],
    now: i64,
    validator: Option<&dyn Eip1654Validator>,
) -> Result<Address, SignedFetchError> {
    let mut verdict = None;
    for payload in payloads {
        match crate::auth::verify_auth_chain_async(chain, payload, Some(now * 1000), validator)
            .await
        {
            Ok(()) => {
                return Address::parse(&chain[0].payload).map_err(|e| {
                    SignedFetchError::MalformedChain {
                        detail: format!("signer: {e}"),
                    }
                });
            }
            Err(e) => match map_auth_error(e) {
                e @ SignedFetchError::InvalidSignature(_) => verdict = Some(e),
                other => return Err(other),
            },
        }
    }
    Err(verdict.unwrap_or_else(|| SignedFetchError::InvalidSignature("no payload".into())))
}

/// `canonical_metadata_keys` is the switch for the folded-payload fallback: a route
/// naming no keys verifies the 6.x payload only. The owner-only routes (scene delete,
/// undeploy, settings, per-address permissions) name none, as upstream keeps them
/// strict; the gate still answers there. Stage order as upstream `verify()`:
/// chain, timestamp (`Number(raw || '0')`, so an absent header expires like an
/// empty one), metadata parse, the scene gate, then the signature.
/// `signed_path` names the route as mounted, so a nested request verifies a
/// signature over its gateway-forwarded bare path (`SignedPath::from_uris`).
pub async fn require_verified(
    headers: &HeaderMap,
    method: &str,
    signed_path: &SignedPath,
    canonical_metadata_keys: &[&str],
    validator: Option<&dyn Eip1654Validator>,
) -> Result<VerifiedAuth, SignedFetchError> {
    let chain = extract_auth_chain(headers)?;
    let ts = header_str(headers, AUTH_TIMESTAMP_HEADER)
        .filter(|raw| !raw.is_empty())
        .unwrap_or("0")
        .to_string();
    let now = chrono::Utc::now().timestamp();
    check_symmetric_skew(&ts, now, SIGNED_FETCH_EXPIRATION_SECS)?;

    let raw_metadata = header_str(headers, AUTH_METADATA_HEADER)
        .unwrap_or("{}")
        .to_string();
    let metadata = parse_metadata(&raw_metadata)?;
    if is_refused_signer(&metadata) {
        return Err(SignedFetchError::MalformedChain {
            detail: format!(
                "invalid metadata content: \"{}\"",
                truncate_detail(&raw_metadata)
            ),
        });
    }

    let paths = candidate_paths(headers, signed_path);
    let v6: Vec<String> = paths
        .iter()
        .map(|p| build_payload_v6(method, p, &ts, &raw_metadata))
        .collect();
    let signer = match validate_over_paths(&chain, &v6, now, validator).await {
        Err(SignedFetchError::InvalidSignature(detail)) if !canonical_metadata_keys.is_empty() => {
            if let Some(key) = first_respelled_key(&metadata, canonical_metadata_keys) {
                return Err(SignedFetchError::MalformedChain {
                    detail: format!("non-canonical metadata key: expected \"{key}\""),
                });
            }
            let legacy: Vec<String> = paths
                .iter()
                .map(|p| build_legacy_payload(method, p, &ts, &raw_metadata))
                .collect();
            if legacy == v6 {
                return Err(SignedFetchError::InvalidSignature(detail));
            }
            validate_over_paths(&chain, &legacy, now, validator).await?
        }
        settled => settled?,
    };
    Ok(VerifiedAuth { signer, metadata })
}

/// The old `map_auth_error`: a malformed request is a 400 with its detail,
/// every other refusal a 401 (503 when the contract-wallet check could not run).
pub fn map_auth_error_api(e: SignedFetchError) -> ApiError {
    let (status, message) = e.http_status_and_message();
    ApiError::http(status, message)
}
