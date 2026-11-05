//! `POST /lambdas/crypto/validate-signature`: verifies an auth chain against
//! its signed message (or timestamp) with the shared verifier and the ONE
//! EIP-1654 validator (`ETH_RPC_URL`); without one, a contract-wallet link is
//! refused as unverifiable rather than checked against a production RPC.
//!
//! lamb2's schema-validator middleware runs first: a Content-Type other than
//! JSON answers 415, a body that does not parse 400 `{ ok: false, message }`
//! with V8's `JSON.parse` text, and a body that breaks
//! `validateSignatureBodySchema` (an object with only `timestamp`,
//! `signedMessage` and the required `AuthChain.schema`) 400 `{ ok: false,
//! message: "Invalid JSON body", data }` with ajv's first error. The shapes
//! are the production peer's (`captures/lambdas-profiles/LP-27-*.json`); a
//! link error the captures do not pin answers `data: []`.
//!
//! The verdict follows `@dcl/crypto` where the captures pin it
//! (`LP-28-*.json`): a lone SIGNER is valid when its address is the signed
//! message, and the final-authority and bad-ephemeral-signature messages
//! carry its text; other failures keep the shared verifier's.

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::auth::auth_chain::split_ephemeral_payload;
use crate::auth::{
    AuthError, AuthLink, AuthLinkType, decode_hex_0x, recover_address, verify_auth_chain_async,
};

use super::{SharedState, bad_request};

const MAX_AUTH_CHAIN_LENGTH: usize = 10;

#[derive(Debug, Deserialize)]
struct ValidateSignatureBody {
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default, rename = "signedMessage")]
    signed_message: Option<String>,
    #[serde(default, rename = "authChain")]
    auth_chain: Option<Vec<AuthLink>>,
}

const BODY_KEYS: [&str; 3] = ["timestamp", "signedMessage", "authChain"];

/// `AuthLinkType`'s values, in `@dcl/schemas` order (`LP-27-bad-link-type.json`).
const LINK_TYPES: [&str; 5] = [
    "SIGNER",
    "ECDSA_EPHEMERAL",
    "ECDSA_SIGNED_ENTITY",
    "ECDSA_EIP_1654_EPHEMERAL",
    "ECDSA_EIP_1654_SIGNED_ENTITY",
];

/// ajv's first error for the top-level schema, in ajv's object shape.
fn ajv_error(path: &str, schema_path: &str, keyword: &str, params: Value, message: &str) -> Value {
    json!({
        "instancePath": path,
        "schemaPath": schema_path,
        "keyword": keyword,
        "params": params,
        "message": message,
    })
}

/// The middleware's verdict on a parsed body: `Err(data)` when it breaks the schema.
fn check_body_schema(body: &Value) -> Result<ValidateSignatureBody, Vec<Value>> {
    let Some(object) = body.as_object() else {
        return Err(vec![ajv_error(
            "",
            "#/type",
            "type",
            json!({ "type": "object" }),
            "must be object",
        )]);
    };
    if !object.contains_key("authChain") {
        return Err(vec![ajv_error(
            "",
            "#/required",
            "required",
            json!({ "missingProperty": "authChain" }),
            "must have required property 'authChain'",
        )]);
    }
    if let Some(extra) = object.keys().find(|k| !BODY_KEYS.contains(&k.as_str())) {
        return Err(vec![ajv_error(
            "",
            "#/additionalProperties",
            "additionalProperties",
            json!({ "additionalProperty": extra }),
            "must NOT have additional properties",
        )]);
    }
    for key in ["timestamp", "signedMessage"] {
        if object.get(key).is_some_and(|v| !v.is_string()) {
            return Err(vec![ajv_error(
                &format!("/{key}"),
                &format!("#/properties/{key}/type"),
                "type",
                json!({ "type": "string" }),
                "must be string",
            )]);
        }
    }
    check_auth_chain_schema(&object["authChain"])?;
    serde_json::from_value(body.clone()).map_err(|_| Vec::new())
}

/// `AuthChain.schema` for the errors the captures pin: an empty chain
/// (`minItems`) and a link whose `type` is a string outside the enum.
fn check_auth_chain_schema(chain: &Value) -> Result<(), Vec<Value>> {
    let Some(links) = chain.as_array() else {
        return Ok(());
    };
    if links.is_empty() {
        return Err(vec![ajv_error(
            "/authChain",
            "#/properties/authChain/minItems",
            "minItems",
            json!({ "limit": 1 }),
            "must NOT have fewer than 1 items",
        )]);
    }
    for (index, link) in links.iter().enumerate() {
        let link_type = link.get("type").and_then(Value::as_str);
        let payload_ok = link.get("payload").is_some_and(Value::is_string);
        if let Some(link_type) = link_type
            && payload_ok
            && !LINK_TYPES.contains(&link_type)
        {
            return Err(vec![ajv_error(
                &format!("/authChain/{index}/type"),
                "#/properties/authChain/items/properties/type/enum",
                "enum",
                json!({ "allowedValues": LINK_TYPES }),
                "must be equal to one of the allowed values",
            )]);
        }
        if serde_json::from_value::<AuthLink>(link.clone()).is_err() {
            return Err(Vec::new());
        }
    }
    Ok(())
}

/// The middleware's own envelope: `data` only for a schema violation.
fn validator_error(status: StatusCode, message: &str, data: Option<Vec<Value>>) -> Response {
    let mut body = json!({ "ok": false, "message": message });
    if let Some(data) = data {
        body["data"] = Value::Array(data);
    }
    (status, Json(body)).into_response()
}

/// `application/json`, with or without parameters (`LP-27-text-plain.json`,
/// `LP-27-no-content-type.json`).
fn is_json_content_type(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|essence| essence.trim().eq_ignore_ascii_case("application/json"))
}

/// V8's `JSON.parse` message for the shapes the captures pin: an empty body
/// (`LP-27-empty-body.json`) and an object whose first token is neither a
/// key nor `}` (`LP-27-not-json.json`); serde's text for the rest.
fn json_parse_message(body: &[u8], err: &serde_json::Error) -> String {
    const WS: [char; 4] = [' ', '\t', '\n', '\r'];
    let text = String::from_utf8_lossy(body);
    let rest = text.trim_start_matches(WS);
    if rest.is_empty() {
        return "Unexpected end of JSON input".to_string();
    }
    if let Some(after_brace) = rest.strip_prefix('{') {
        let next = after_brace.trim_start_matches(WS);
        if !next.starts_with('"') && !next.starts_with('}') {
            let position = text.len() - next.len();
            let line_start = text[..position].rfind('\n').map_or(0, |i| i + 1);
            let line = text[..position].matches('\n').count() + 1;
            let column = position - line_start + 1;
            return format!(
                "Expected property name or '}}' in JSON at position {position} (line {line} column {column})"
            );
        }
    }
    err.to_string()
}

#[derive(Debug, PartialEq)]
struct PreparedValidation {
    chain: Vec<AuthLink>,
    final_authority: String,
    owner: String,
}

fn prepare_validation(
    auth_chain: Option<Vec<AuthLink>>,
    signed_message: Option<String>,
    timestamp: Option<String>,
) -> Result<PreparedValidation, String> {
    let chain = auth_chain.ok_or_else(|| "Invalid request. Missing 'authChain'".to_string())?;
    if chain.is_empty() || chain.len() > MAX_AUTH_CHAIN_LENGTH {
        return Err(format!(
            "'authChain' length must be between 1 and {MAX_AUTH_CHAIN_LENGTH}"
        ));
    }
    let final_authority = signed_message.or(timestamp).unwrap_or_default();
    if final_authority.is_empty() {
        return Err("Expected 'signedMessage' property to be set".to_string());
    }
    let owner = chain.first().map(|l| l.payload.clone()).unwrap_or_default();
    Ok(PreparedValidation {
        chain,
        final_authority,
        owner,
    })
}

/// `@dcl/crypto`'s text for a final authority that is not the signed message.
fn final_authority_message(expected: &str, current: &str) -> String {
    format!("ERROR: Invalid final authority. Expected: {expected}. Current {current}.")
}

/// The first link after the SIGNER whose signature does not recover: the
/// link a [`AuthError::RecoveryFailed`] came from, since the verifier stops
/// at the first failing link.
fn unrecoverable_link(chain: &[AuthLink]) -> Option<&AuthLink> {
    chain.iter().skip(1).find(|link| {
        let signature = link.signature.as_deref().unwrap_or_default();
        match link.link_type {
            AuthLinkType::SIGNER => false,
            AuthLinkType::EcdsaEphemeral => {
                split_ephemeral_payload(&link.payload).is_ok_and(|(message, _, _)| {
                    recover_address(message.as_bytes(), signature).is_err()
                })
            }
            AuthLinkType::EcdsaSignedEntity => {
                recover_address(link.payload.as_bytes(), signature).is_err()
            }
            AuthLinkType::EcdsaEip1654Ephemeral | AuthLinkType::EcdsaEip1654SignedEntity => {
                decode_hex_0x(signature).is_err()
            }
        }
    })
}

/// The verifier's error in `@dcl/crypto`'s words where a capture pins them.
fn verdict_message(chain: &[AuthLink], err: &AuthError) -> String {
    match err {
        AuthError::FinalAuthorityMismatch { expected, actual } => {
            final_authority_message(expected, actual)
        }
        AuthError::RecoveryFailed(_)
            if unrecoverable_link(chain)
                .is_some_and(|l| l.link_type == AuthLinkType::EcdsaEphemeral) =>
        {
            "ERROR. Link type: ECDSA_EPHEMERAL. Invalid ethereum signature.".to_string()
        }
        other => other.to_string(),
    }
}

async fn verdict(state: &SharedState, prepared: &PreparedValidation) -> Result<(), String> {
    // `@dcl/crypto` takes a lone SIGNER's address as the final authority;
    // the shared verifier refuses an unsigned chain.
    if let [signer] = prepared.chain.as_slice()
        && signer.link_type == AuthLinkType::SIGNER
    {
        if signer.payload == prepared.final_authority {
            return Ok(());
        }
        return Err(final_authority_message(
            &prepared.final_authority,
            &signer.payload,
        ));
    }
    let now_ms = chrono::Utc::now().timestamp_millis();
    verify_auth_chain_async(
        &prepared.chain,
        &prepared.final_authority,
        Some(now_ms),
        state.eip1654.as_deref(),
    )
    .await
    .map_err(|e| verdict_message(&prepared.chain, &e))
}

pub async fn validate_signature(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !is_json_content_type(&headers) {
        return validator_error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "Content-Type must be application/json",
            None,
        );
    }
    let parsed: Value = match serde_json::from_slice(&body) {
        Ok(b) => b,
        Err(e) => {
            return validator_error(
                StatusCode::BAD_REQUEST,
                &json_parse_message(&body, &e),
                None,
            );
        }
    };
    let parsed = match check_body_schema(&parsed) {
        Ok(b) => b,
        Err(data) => {
            return validator_error(StatusCode::BAD_REQUEST, "Invalid JSON body", Some(data));
        }
    };
    let prepared =
        match prepare_validation(parsed.auth_chain, parsed.signed_message, parsed.timestamp) {
            Ok(p) => p,
            Err(msg) => return bad_request(&msg),
        };
    let body = match verdict(&state, &prepared).await {
        Ok(()) => json!({ "valid": true, "ownerAddress": prepared.owner }),
        Err(error) => json!({ "valid": false, "error": error }),
    };
    (StatusCode::OK, Json(body)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signer_link(addr: &str) -> AuthLink {
        AuthLink {
            link_type: AuthLinkType::SIGNER,
            payload: addr.to_string(),
            signature: None,
        }
    }

    #[test]
    fn prepare_rejects_missing_or_bad_input() {
        assert_eq!(
            prepare_validation(None, Some("msg".into()), None).unwrap_err(),
            "Invalid request. Missing 'authChain'"
        );
        assert_eq!(
            prepare_validation(Some(vec![]), Some("msg".into()), None).unwrap_err(),
            "'authChain' length must be between 1 and 10"
        );
        let chain: Vec<AuthLink> = (0..=MAX_AUTH_CHAIN_LENGTH)
            .map(|i| signer_link(&format!("0x{i}")))
            .collect();
        assert_eq!(
            prepare_validation(Some(chain), Some("msg".into()), None).unwrap_err(),
            "'authChain' length must be between 1 and 10"
        );
        let chain: Vec<AuthLink> = (0..MAX_AUTH_CHAIN_LENGTH)
            .map(|i| signer_link(&format!("0x{i}")))
            .collect();
        assert!(prepare_validation(Some(chain), Some("msg".into()), None).is_ok());
        assert_eq!(
            prepare_validation(Some(vec![signer_link("0xabc")]), None, None).unwrap_err(),
            "Expected 'signedMessage' property to be set"
        );
        assert_eq!(
            prepare_validation(Some(vec![signer_link("0xabc")]), Some(String::new()), None)
                .unwrap_err(),
            "Expected 'signedMessage' property to be set"
        );
    }

    #[test]
    fn prepare_prefers_signed_message_over_timestamp() {
        let p = prepare_validation(
            Some(vec![signer_link("0xabc")]),
            Some("the-message".into()),
            Some("12345".into()),
        )
        .unwrap();
        assert_eq!(p.final_authority, "the-message");
        assert_eq!(p.owner, "0xabc");
        let p = prepare_validation(Some(vec![signer_link("0xABC")]), None, Some("12345".into()))
            .unwrap();
        assert_eq!(p.final_authority, "12345");
        assert_eq!(p.owner, "0xABC");
    }

    async fn post_raw(content_type: Option<&str>, body: &str) -> (StatusCode, Value) {
        let state = super::super::tests::support::state_without_db();
        let mut headers = HeaderMap::new();
        if let Some(content_type) = content_type {
            headers.insert(header::CONTENT_TYPE, content_type.parse().unwrap());
        }
        let resp = validate_signature(State(state), headers, Bytes::from(body.to_string())).await;
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    async fn post(body: Value) -> (StatusCode, Value) {
        post_raw(Some("application/json"), &body.to_string()).await
    }

    fn json_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
        headers
    }

    const OWNER: &str = "0x0000000000000000000000000000000000000001";

    fn one_signer() -> Value {
        json!([{ "type": "SIGNER", "payload": OWNER, "signature": "" }])
    }

    /// `LP-27-text-plain.json`, `LP-27-no-content-type.json`: 415 before the body is read.
    #[tokio::test]
    async fn a_non_json_content_type_is_415() {
        let body = json!({ "authChain": one_signer(), "timestamp": "1" }).to_string();
        for content_type in [Some("text/plain"), None] {
            let (status, v) = post_raw(content_type, &body).await;
            assert_eq!(
                status,
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "{content_type:?}"
            );
            assert_eq!(
                v,
                json!({ "ok": false, "message": "Content-Type must be application/json" })
            );
        }
        let (status, _) = post_raw(Some("application/json; charset=utf-8"), &body).await;
        assert_eq!(status, StatusCode::OK, "past the content-type check");
    }

    /// `LP-27-empty-body.json`, `LP-27-not-json.json`: V8's parse messages.
    #[tokio::test]
    async fn an_unparsable_body_answers_the_parse_message() {
        let (status, v) = post_raw(Some("application/json"), "").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            v,
            json!({ "ok": false, "message": "Unexpected end of JSON input" })
        );
        let (status, v) = post_raw(Some("application/json"), "{").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            v,
            json!({ "ok": false, "message": "Expected property name or '}' in JSON at position 1 (line 1 column 2)" })
        );
        let (_, v) = post_raw(Some("application/json"), "\n { 1").await;
        assert_eq!(
            v["message"],
            "Expected property name or '}' in JSON at position 4 (line 2 column 4)"
        );
    }

    /// lamb2's schema-validator middleware, with the `data` the
    /// production peer answers (`LP-27-*.json`).
    #[tokio::test]
    async fn schema_violations_answer_the_validator_envelope() {
        let chain = one_signer();
        let cases = [
            (
                json!({}),
                json!({"instancePath":"","schemaPath":"#/required","keyword":"required","params":{"missingProperty":"authChain"},"message":"must have required property 'authChain'"}),
            ),
            (
                json!({ "authChain": chain, "timestamp": "1", "foo": 1 }),
                json!({"instancePath":"","schemaPath":"#/additionalProperties","keyword":"additionalProperties","params":{"additionalProperty":"foo"},"message":"must NOT have additional properties"}),
            ),
            (
                json!({ "authChain": chain, "timestamp": 1 }),
                json!({"instancePath":"/timestamp","schemaPath":"#/properties/timestamp/type","keyword":"type","params":{"type":"string"},"message":"must be string"}),
            ),
            (
                json!({ "authChain": [], "timestamp": "1" }),
                json!({"instancePath":"/authChain","schemaPath":"#/properties/authChain/minItems","keyword":"minItems","params":{"limit":1},"message":"must NOT have fewer than 1 items"}),
            ),
            (
                json!({ "authChain": [{ "type": "NOPE", "payload": OWNER, "signature": "" }], "timestamp": "1" }),
                json!({"instancePath":"/authChain/0/type","schemaPath":"#/properties/authChain/items/properties/type/enum","keyword":"enum","params":{"allowedValues":["SIGNER","ECDSA_EPHEMERAL","ECDSA_SIGNED_ENTITY","ECDSA_EIP_1654_EPHEMERAL","ECDSA_EIP_1654_SIGNED_ENTITY"]},"message":"must be equal to one of the allowed values"}),
            ),
            (
                json!([]),
                json!({"instancePath":"","schemaPath":"#/type","keyword":"type","params":{"type":"object"},"message":"must be object"}),
            ),
        ];
        for (body, error) in cases {
            let (status, v) = post(body.clone()).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
            assert_eq!(
                v,
                json!({ "ok": false, "message": "Invalid JSON body", "data": [error] }),
                "{body}"
            );
        }
        // A link error no capture pins: the envelope with no `data` entry.
        let (status, v) =
            post(json!({ "signedMessage": "m", "authChain": [{ "type": "SIGNER" }] })).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            v,
            json!({ "ok": false, "message": "Invalid JSON body", "data": [] })
        );
        // `LP-27-no-signed-message.json`: past the schema, the handler's own 400.
        let (status, v) = post(json!({ "authChain": chain })).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            v,
            json!({ "error": "Bad request", "message": "Expected 'signedMessage' property to be set" })
        );
    }

    /// `LP-28-single-signer-match.json`, `LP-28-single-signer-other.json`.
    #[tokio::test]
    async fn a_lone_signer_is_its_own_final_authority() {
        let (status, v) = post(json!({ "authChain": one_signer(), "signedMessage": OWNER })).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v, json!({ "valid": true, "ownerAddress": OWNER }));
        let (status, v) =
            post(json!({ "authChain": one_signer(), "signedMessage": "hello" })).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            v,
            json!({ "valid": false, "error": format!("ERROR: Invalid final authority. Expected: hello. Current {OWNER}.") })
        );
    }

    /// `LP-28-bad-signature.json`.
    #[tokio::test]
    async fn a_bad_ephemeral_signature_carries_the_crypto_message() {
        let body = json!({
            "authChain": [
                { "type": "SIGNER", "payload": OWNER, "signature": "" },
                {
                    "type": "ECDSA_EPHEMERAL",
                    "payload": "Decentraland Login\nEphemeral address: 0x0000000000000000000000000000000000000002\nExpiration: 2099-01-01T00:00:00.000Z",
                    "signature": "0xdeadbeef"
                },
                { "type": "ECDSA_SIGNED_ENTITY", "payload": "hello", "signature": "0xdeadbeef" }
            ],
            "signedMessage": "hello"
        });
        let (status, v) = post(body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            v,
            json!({ "valid": false, "error": "ERROR. Link type: ECDSA_EPHEMERAL. Invalid ethereum signature." })
        );
    }

    #[tokio::test]
    async fn a_signed_chain_validates_and_a_forged_one_does_not() {
        use alloy_signer::SignerSync;
        let signer = alloy_signer_local::PrivateKeySigner::random();
        let addr = format!("{:#x}", signer.address());
        let message = "validate me";
        let sig = signer.sign_message_sync(message.as_bytes()).unwrap();
        let chain = vec![
            signer_link(&addr),
            AuthLink {
                link_type: AuthLinkType::EcdsaSignedEntity,
                payload: message.into(),
                signature: Some(format!("0x{}", hex::encode(sig.as_bytes()))),
            },
        ];
        let state = super::super::tests::support::state_without_db();
        let body = Bytes::from(json!({ "signedMessage": message, "authChain": chain }).to_string());
        let resp = validate_signature(State(state.clone()), json_headers(), body).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let v: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(v["valid"], true, "{v}");
        assert_eq!(v["ownerAddress"], addr);
        let body = Bytes::from(
            json!({ "signedMessage": "another message", "authChain": chain }).to_string(),
        );
        let resp = validate_signature(State(state), json_headers(), body).await;
        let v: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(v["valid"], false, "{v}");
        assert_eq!(
            v["error"],
            "ERROR: Invalid final authority. Expected: another message. Current validate me."
        );
    }
}
