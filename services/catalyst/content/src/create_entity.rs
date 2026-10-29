//! `POST /entities` (root and `/content`): the local deploy. A
//! `multipart/form-data` body carries the `entityId` field, the auth chain
//! (an `authChain` JSON field, `authChain[i][type|payload|signature]`
//! fields, or the legacy `ethAddress` + `signature` pair) and the files,
//! the entity file among them. The write deployer
//! ([`super::write_deployer::WriteDeployer::deploy_entity`]) hashes the
//! uploads, verifies the chain (the shared EIP-1654 validator for contract
//! wallets), refuses a denylisted id, runs the content validator (the LAND
//! leg through `land_authz`) and persists the whole deployment in ONE call
//! of the hash-named deploy function, `content.local_entities` provenance
//! included for a local scene.
//!
//! Answers: 200 `{creationTimestamp}`; 400 `{errors}` for a refused
//! deployment and `{error}` for a malformed form; 503 `{errors}` with
//! `Retry-After` when this node could not decide (a source unset or down,
//! the store unreadable); the gates of [`super::writes`] before any of it
//! (503 disabled, 403 read-only) and the old 503 while the node is
//! bootstrapping. The per-client request budget is the proxy's
//! (`limit_req zone=catdeploy` on `= /content/entities`,
//! `deploy/nix/nginx.nix`); the per-pointer TTLs are the deployer's.

use std::collections::BTreeMap;

use axum::Json;
use axum::extract::{Multipart, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use bytes::{Bytes, BytesMut};
use serde_json::{Value, json};

use super::SharedState;
use super::errors::{AppError, AppResult};
use super::write_deployer::DeployFailure;
use super::writes;
use crate::auth::MAX_AUTH_CHAIN_LINKS;

pub const MAX_DEPLOY_FILES: usize = 1000;
pub const MAX_DEPLOY_FILE_BYTES: usize = 50 * 1024 * 1024;

/// The request body cap: the proxy's `client_max_body_size 200m` on the
/// same route, and the old `MAX_DEPLOYMENT_SIZE_BYTES` default.
pub const MAX_DEPLOYMENT_BYTES: usize = 200 * 1024 * 1024;

fn invalid(msg: impl Into<String>) -> AppError {
    AppError::InvalidRequest(msg.into())
}

/// The chain as the form spells it: an `authChain` JSON array, or the
/// indexed `authChain[i][type|payload|signature]` fields; `None` when
/// neither is present.
pub fn extract_auth_chain_from_fields(
    fields: &BTreeMap<String, String>,
) -> Result<Option<Value>, String> {
    if let Some(chain_str) = fields.get("authChain") {
        let chain: Value =
            serde_json::from_str(chain_str).map_err(|_| "Invalid auth chain".to_string())?;
        let arr = chain
            .as_array()
            .ok_or_else(|| "Invalid auth chain".to_string())?;
        if arr.len() > MAX_AUTH_CHAIN_LINKS {
            return Err(too_long_message());
        }
        return Ok(Some(chain));
    }

    let mut biggest_index: i64 = -1;
    for key in fields.keys() {
        if let Some(rest) = key.strip_prefix("authChain[")
            && let Some(idx_str) = rest.split(']').next()
            && let Ok(idx) = idx_str.parse::<i64>()
            && idx > biggest_index
        {
            biggest_index = idx;
        }
    }

    if biggest_index == -1 {
        return Ok(None);
    }
    if biggest_index >= MAX_AUTH_CHAIN_LINKS as i64 {
        return Err(too_long_message());
    }

    let mut chain = Vec::new();
    for i in 0..=biggest_index {
        let payload = indexed_field(fields, i, "payload")?;
        let signature = indexed_field(fields, i, "signature")?;
        let link_type = indexed_field(fields, i, "type")?;
        chain.push(json!({
            "type": link_type,
            "payload": payload,
            "signature": signature,
        }));
    }
    Ok(Some(Value::Array(chain)))
}

fn indexed_field<'a>(
    fields: &'a BTreeMap<String, String>,
    i: i64,
    part: &str,
) -> Result<&'a String, String> {
    fields
        .get(&format!("authChain[{i}][{part}]"))
        .ok_or_else(|| format!("Missing auth chain element at index {i}"))
}

fn too_long_message() -> String {
    format!("Auth chain is too long; the maximum allowed is {MAX_AUTH_CHAIN_LINKS} elements")
}

/// The chain of a deploy form: the explicit one (every link with a string
/// `type` and `payload`), else the legacy `ethAddress` + `signature` pair
/// as a SIGNER + ECDSA_SIGNED_ENTITY chain.
fn auth_chain_of(fields: &BTreeMap<String, String>, entity_id: &str) -> AppResult<Value> {
    if let Some(chain) = extract_auth_chain_from_fields(fields).map_err(invalid)? {
        if let Some(arr) = chain.as_array() {
            for link in arr {
                if link.get("type").and_then(|v| v.as_str()).is_none() {
                    return Err(invalid(
                        "invalid auth chain format: each link must have a \"type\" field",
                    ));
                }
                if link.get("payload").and_then(|v| v.as_str()).is_none() {
                    return Err(invalid(
                        "invalid auth chain format: each link must have a \"payload\" field",
                    ));
                }
            }
        }
        return Ok(chain);
    }
    match (fields.get("ethAddress"), fields.get("signature")) {
        (Some(addr), Some(sig)) => Ok(json!([
            { "type": "SIGNER", "payload": addr, "signature": Value::Null },
            { "type": "ECDSA_SIGNED_ENTITY", "payload": entity_id, "signature": sig }
        ])),
        _ => Err(invalid("No auth chain can be derived")),
    }
}

/// A part is a file when it names one or carries a non-text body: the Explorer
/// sends each file as a bare `application/octet-stream` part named by its hash.
fn is_file_part(file_name: Option<&str>, content_type: Option<&str>) -> bool {
    file_name.is_some()
        || content_type.is_some_and(|t| !t.trim_start().to_ascii_lowercase().starts_with("text/"))
}

/// Reads the whole form: text fields by name, file parts in order, each
/// under [`MAX_DEPLOY_FILE_BYTES`], at most [`MAX_DEPLOY_FILES`] of them.
async fn read_form(multipart: &mut Multipart) -> AppResult<(BTreeMap<String, String>, Vec<Bytes>)> {
    let mut fields: BTreeMap<String, String> = BTreeMap::new();
    let mut files: Vec<Bytes> = Vec::new();

    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| invalid(format!("Failed to read multipart field: {e}")))?
    {
        let name = field.name().unwrap_or("").to_string();

        if is_file_part(field.file_name(), field.content_type()) {
            if files.len() >= MAX_DEPLOY_FILES {
                return Err(invalid(format!(
                    "deployment exceeds maximum of {MAX_DEPLOY_FILES} files"
                )));
            }
            let mut buf = BytesMut::new();
            while let Some(chunk) = field
                .chunk()
                .await
                .map_err(|e| invalid(format!("Failed to read file data: {e}")))?
            {
                if buf.len().saturating_add(chunk.len()) > MAX_DEPLOY_FILE_BYTES {
                    return Err(invalid(format!(
                        "file {name} exceeds {MAX_DEPLOY_FILE_BYTES} bytes"
                    )));
                }
                buf.extend_from_slice(&chunk);
            }
            files.push(buf.freeze());
        } else {
            let value = field
                .text()
                .await
                .map_err(|e| invalid(format!("Failed to read field value: {e}")))?;
            fields.insert(name, value);
        }
    }
    Ok((fields, files))
}

/// The `type` of the uploaded entity file (the JSON upload hashing to
/// `entity_id`), for the unsynced copy's per-type gate; the deployer checks
/// the same file in full.
fn entity_type_of(files: &[Bytes], entity_id: &str) -> Option<String> {
    files
        .iter()
        .filter(|f| f.first() == Some(&b'{'))
        .find(|f| crate::auth::verify_hash(f, entity_id))
        .and_then(|f| serde_json::from_slice::<Value>(f).ok())
        .and_then(|v| v.get("type")?.as_str().map(str::to_string))
}

pub fn deployment_failure_response(failure: DeployFailure) -> Response {
    let unavailable = matches!(failure, DeployFailure::Unavailable(_));
    let (status, errors) = match failure {
        DeployFailure::Rejected(errors) => (StatusCode::BAD_REQUEST, errors),
        DeployFailure::Unavailable(errors) => (StatusCode::SERVICE_UNAVAILABLE, errors),
    };
    let mut response = (status, Json(json!({ "errors": errors }))).into_response();
    if unavailable {
        response.headers_mut().insert(
            header::RETRY_AFTER,
            header::HeaderValue::from_static(writes::UNAVAILABLE_RETRY_AFTER_SECS),
        );
    }
    response
}

pub async fn create_entity(
    State(state): State<SharedState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> AppResult<Response> {
    let request_started = std::time::Instant::now();
    if let Some(refused) = writes::refuse(&state) {
        return Ok(refused);
    }
    let Some(deployer) = state.deployer().cloned() else {
        return Ok(writes::disabled_response());
    };
    let bootstrapping = || {
        Err(AppError::ServiceUnavailable(
            writes::BOOTSTRAPPING_MESSAGE.to_string(),
        ))
    };
    if !state.sync.takes_some_deploys() {
        return bootstrapping();
    }

    let (fields, files) = read_form(&mut multipart).await?;
    let entity_id = fields
        .get("entityId")
        .ok_or_else(|| invalid("Missing entityId field"))?
        .clone();
    if !state.sync.takes_deploys()
        && !entity_type_of(&files, &entity_id).is_some_and(|t| state.sync.takes_deploys_of(&t))
    {
        return bootstrapping();
    }
    let auth_chain = auth_chain_of(&fields, &entity_id)?;

    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");
    tracing::info!(
        entity_id = %entity_id,
        user_agent = %user_agent,
        file_count = files.len(),
        "POST /entities - Deploying entity (multipart)"
    );

    let multipart_us = request_started.elapsed().as_micros() as u64;
    let deploy_started = std::time::Instant::now();
    match deployer
        .deploy_entity(files, &entity_id, auth_chain, "LOCAL")
        .await
    {
        Ok(creation_timestamp) => {
            tracing::info!(entity_id = %entity_id, "POST /entities - Deployment successful");
            state.deployments_cache.clear();
            tracing::debug!(
                target: "catalyst_perf",
                phase = "publish",
                entity_id,
                multipart_us,
                deploy_us = deploy_started.elapsed().as_micros() as u64,
                elapsed_us = request_started.elapsed().as_micros() as u64
            );
            Ok((
                StatusCode::OK,
                Json(json!({ "creationTimestamp": creation_timestamp })),
            )
                .into_response())
        }
        Err(failure) => {
            tracing::warn!(entity_id = %entity_id, errors = ?failure.errors(), "POST /entities - Deployment failed");
            Ok(deployment_failure_response(failure))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    fn link_fields(count: usize) -> BTreeMap<String, String> {
        let mut f = BTreeMap::new();
        for i in 0..count {
            f.insert(format!("authChain[{i}][type]"), "SIGNER".to_string());
            f.insert(format!("authChain[{i}][payload]"), format!("0xp{i}"));
            f.insert(format!("authChain[{i}][signature]"), format!("0xs{i}"));
        }
        f
    }

    #[test]
    fn a_part_is_a_file_when_named_as_one_or_not_text() {
        assert!(is_file_part(Some("profile.json"), None));
        assert!(is_file_part(Some("face.png"), Some("image/png")));
        assert!(is_file_part(None, Some("application/octet-stream")));
        assert!(!is_file_part(None, None));
        assert!(!is_file_part(None, Some("text/plain; charset=utf-8")));
        assert!(!is_file_part(None, Some("Text/Plain")));
    }

    #[test]
    fn extract_auth_chain_from_fields_parses_indexed_keys() {
        let f = fields(&[
            ("authChain[0][type]", "SIGNER"),
            ("authChain[0][payload]", "0xabc"),
            ("authChain[0][signature]", ""),
            ("authChain[1][type]", "ECDSA_SIGNED_ENTITY"),
            ("authChain[1][payload]", "QmEntity"),
            ("authChain[1][signature]", "0xdeadbeef"),
        ]);
        let out = extract_auth_chain_from_fields(&f).expect("parser ok");
        let arr = out.expect("Some(array)");
        let arr = arr.as_array().expect("is array");
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["type"], "SIGNER");
        assert_eq!(arr[0]["payload"], "0xabc");
        assert_eq!(arr[0]["signature"], "");
        assert_eq!(arr[1]["type"], "ECDSA_SIGNED_ENTITY");
        assert_eq!(arr[1]["payload"], "QmEntity");
        assert_eq!(arr[1]["signature"], "0xdeadbeef");
    }

    #[test]
    fn extract_auth_chain_rejects_mixed_keys() {
        let f = fields(&[
            ("authChain[0][type]", "SIGNER"),
            ("authChain[ABC][type]", "SIGNER"),
            ("authChain[ABC][payload]", "0xabc"),
            ("authChain[ABC][signature]", ""),
        ]);
        let err = extract_auth_chain_from_fields(&f)
            .expect_err("missing payload/signature at index 0 must error");
        assert!(
            err.to_lowercase().contains("missing auth chain element"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn extract_auth_chain_caps_at_max_links() {
        let err = extract_auth_chain_from_fields(&link_fields(MAX_AUTH_CHAIN_LINKS + 1))
            .expect_err("index loop must reject more than the max allowed links");
        assert!(
            err.contains("Auth chain is too long"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn extract_auth_chain_json_caps_at_max_links() {
        let links: Vec<Value> = (0..=MAX_AUTH_CHAIN_LINKS)
            .map(|i| json!({ "type": "SIGNER", "payload": format!("0x{i}"), "signature": "" }))
            .collect();
        let chain = serde_json::to_string(&Value::Array(links)).unwrap();
        let f = fields(&[("authChain", chain.as_str())]);
        let err = extract_auth_chain_from_fields(&f)
            .expect_err("json array longer than the max must be rejected");
        assert!(
            err.contains("Auth chain is too long"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn extract_auth_chain_empty_returns_none() {
        let f = fields(&[("entityId", "Qm123"), ("someOtherField", "x")]);
        assert!(
            extract_auth_chain_from_fields(&f)
                .expect("parser ok")
                .is_none()
        );
    }

    #[test]
    fn extract_auth_chain_from_json_string_works() {
        let f = fields(&[(
            "authChain",
            r#"[{"type":"SIGNER","payload":"0x1","signature":""}]"#,
        )]);
        let arr = extract_auth_chain_from_fields(&f)
            .expect("parser ok")
            .expect("Some(array)");
        let arr = arr.as_array().expect("is array");
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["type"], "SIGNER");
    }

    #[test]
    fn extract_auth_chain_rejects_invalid_json() {
        for bad in ["not-json", r#"{"a":1}"#] {
            assert_eq!(
                extract_auth_chain_from_fields(&fields(&[("authChain", bad)])),
                Err("Invalid auth chain".to_string())
            );
        }
    }

    #[test]
    fn a_chain_without_a_link_type_or_payload_is_malformed_and_the_legacy_pair_is_a_chain() {
        let err =
            auth_chain_of(&fields(&[("authChain", r#"[{"payload":"0x1"}]"#)]), "bafy").unwrap_err();
        assert_eq!(
            err.to_string(),
            "invalid auth chain format: each link must have a \"type\" field"
        );
        let err =
            auth_chain_of(&fields(&[("authChain", r#"[{"type":"SIGNER"}]"#)]), "bafy").unwrap_err();
        assert_eq!(
            err.to_string(),
            "invalid auth chain format: each link must have a \"payload\" field"
        );
        let chain = auth_chain_of(
            &fields(&[("ethAddress", "0xabc"), ("signature", "0xsig")]),
            "bafy",
        )
        .unwrap();
        assert_eq!(
            chain,
            json!([
                { "type": "SIGNER", "payload": "0xabc", "signature": null },
                { "type": "ECDSA_SIGNED_ENTITY", "payload": "bafy", "signature": "0xsig" }
            ])
        );
        assert_eq!(
            auth_chain_of(&fields(&[("ethAddress", "0xabc")]), "bafy")
                .unwrap_err()
                .to_string(),
            "No auth chain can be derived"
        );
    }

    #[tokio::test]
    async fn a_failure_keeps_its_kind_and_an_undecidable_one_carries_retry_after() {
        let rejected = deployment_failure_response(DeployFailure::Rejected(vec!["no".into()]));
        assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
        assert!(rejected.headers().get(header::RETRY_AFTER).is_none());
        let body = axum::body::to_bytes(rejected.into_body(), 4096)
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            json!({ "errors": ["no"] })
        );
        let unavailable =
            deployment_failure_response(DeployFailure::Unavailable(vec!["down".into()]));
        assert_eq!(unavailable.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(unavailable.headers()[header::RETRY_AFTER], "30");
    }
}
