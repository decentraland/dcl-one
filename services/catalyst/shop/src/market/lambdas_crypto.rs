//! `POST /lambdas/crypto/validate-signature`: verifies an auth chain against
//! its signed message (or timestamp) with the shared verifier and the ONE
//! EIP-1654 validator (`ETH_RPC_URL`); without one, a contract-wallet link is
//! refused as unverifiable rather than checked against a production RPC.
//!
//! A body that breaks lamb2's `validateSignatureBodySchema` (an object with
//! only `timestamp`, `signedMessage` and the required `authChain`) answers
//! its schema-validator middleware's 400 `{ ok: false, message: "Invalid JSON
//! body", data }`. `data` carries ajv's error for the top-level keywords of
//! that schema; `AuthChain.schema` itself is not vendored, so a link that
//! breaks it answers `data: []` until a capture pins ajv's text.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::auth::{AuthLink, verify_auth_chain_async};

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
    serde_json::from_value(body.clone()).map_err(|_| Vec::new())
}

fn invalid_body(data: Vec<Value>) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({ "ok": false, "message": "Invalid JSON body", "data": data })),
    )
        .into_response()
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

pub async fn validate_signature(State(state): State<SharedState>, body: Bytes) -> Response {
    let parsed: Value = match serde_json::from_slice(&body) {
        Ok(b) => b,
        Err(e) => return bad_request(&format!("Invalid JSON body: {e}")),
    };
    let parsed = match check_body_schema(&parsed) {
        Ok(b) => b,
        Err(data) => return invalid_body(data),
    };
    let prepared =
        match prepare_validation(parsed.auth_chain, parsed.signed_message, parsed.timestamp) {
            Ok(p) => p,
            Err(msg) => return bad_request(&msg),
        };
    let now_ms = chrono::Utc::now().timestamp_millis();
    let result = verify_auth_chain_async(
        &prepared.chain,
        &prepared.final_authority,
        Some(now_ms),
        state.eip1654.as_deref(),
    )
    .await;
    let body = match result {
        Ok(()) => json!({ "valid": true, "ownerAddress": prepared.owner }),
        Err(e) => json!({ "valid": false, "error": e.to_string() }),
    };
    (StatusCode::OK, Json(body)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthLinkType;

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

    async fn post(body: Value) -> (StatusCode, Value) {
        let state = super::super::tests::support::state_without_db();
        let resp = validate_signature(State(state), Bytes::from(body.to_string())).await;
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    /// lamb2's schema-validator middleware: `{ ok: false, message: "Invalid
    /// JSON body", data }` for a missing `authChain`, an unknown key or a
    /// non-string `timestamp`/`signedMessage`.
    #[tokio::test]
    async fn schema_violations_answer_the_validator_envelope() {
        let chain = json!([{ "type": "SIGNER", "payload": "0x0000000000000000000000000000000000000001", "signature": "" }]);
        for (body, keyword) in [
            (json!({ "signedMessage": "m" }), "required"),
            (
                json!({ "signedMessage": "m", "authChain": chain, "x": 1 }),
                "additionalProperties",
            ),
            (json!({ "timestamp": 1, "authChain": chain }), "type"),
            (json!([]), "type"),
        ] {
            let (status, v) = post(body.clone()).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
            assert_eq!(v["ok"], false, "{v}");
            assert_eq!(v["message"], "Invalid JSON body", "{v}");
            assert_eq!(v["data"][0]["keyword"], keyword, "{body}: {v}");
        }
        let (status, v) = post(
            json!({ "signedMessage": "m", "authChain": [{ "type": "NOPE", "payload": "x" }] }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            v,
            json!({ "ok": false, "message": "Invalid JSON body", "data": [] })
        );
        let (status, v) = post(json!({ "signedMessage": "m", "authChain": [] })).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(v["message"], "'authChain' length must be between 1 and 10");
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
        let resp = validate_signature(State(state.clone()), body).await;
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
        let resp = validate_signature(State(state), body).await;
        let v: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(v["valid"], false, "{v}");
        assert!(v["error"].as_str().is_some());
    }
}
