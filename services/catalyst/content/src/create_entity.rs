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
//! Answers: 200 `{creationTimestamp}`, the stored `local_timestamp`; an
//! entity already deployed answers its stored one before any signature,
//! TTL or rate-limit check, as upstream's idempotent deploy does. 400
//! `{errors}` for a refused deployment and `{error}` for a malformed form,
//! 413 `{error}` for one past a form limit (upstream's multipart limits and
//! texts, the total capped at [`MAX_DEPLOYMENT_BYTES`]); 503 `{errors}` with
//! `Retry-After` when this node could not decide (a source unset or down,
//! the store unreadable), its cause logged, not answered. The gates of
//! [`super::writes`] come before any of it (404 read-only, 503 disabled),
//! then the 503 with `Retry-After: 60` while the node is bootstrapping.
//! Before the body is read, a body that does not fit the in-memory upload
//! budget answers 503 and a client address past its daily quota 429, both
//! with `Retry-After` ([`super::upload_admission`], per process); a body
//! that is not a form, or declares more than a form can hold, then answers
//! 400 or 413 uncounted. The quota counts every other admitted request, a
//! form refused for its limits included, and gives back only the ones the
//! deployer could not decide for its own state (its 503). A contract-wallet
//! signature this node cannot check (503) and an unsynced copy refusing the
//! entity's type (503) still count: the client chose them.
//! Ahead of the bootstrapping gate, every request (partial batches
//! included) counts toward upstream's per-client burst limit, 200 per
//! 60 s by default, a 429 `{"error":"Too many requests"}` with
//! `Retry-After` past it; the gateway's coarser `limit_req zone=deploy`
//! (`infra/gateway/gateway.py`) sits above it. The per-pointer TTLs are the
//! deployer's.
//!
//! A form with `partial=true` is a batch of a partial upload
//! ([`super::partial_deployments`]): 202 `{missing}` until the upload is
//! complete, then 200. `?partial=true` declares it before the body, so the
//! quota is not counted on admission but once the form is read (the upload
//! once, when created, and a batch refused with a 400 before joining it);
//! the query flag on a form without the field is a 400. Once the fields
//! before the first file are read, a batch from a source that spent its
//! quota and not resuming its signer's own upload, or from an account over
//! its byte rate, is turned away (429) before any file is buffered.

use std::collections::{BTreeMap, HashSet};

use axum::Json;
use axum::extract::multipart::{MultipartError, MultipartRejection};
use axum::extract::{Multipart, RawQuery, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use bytes::{Bytes, BytesMut};
use serde_json::{Value, json};

use super::SharedState;
use super::errors::{AppError, AppResult};
use super::partial_deployments;
use super::write_deployer::DeployFailure;
use super::writes;
use crate::auth::MAX_AUTH_CHAIN_LINKS;

/// Upstream's `MAX_UPLOAD_FILE_COUNT` and `MAX_UPLOAD_FILE_SIZE` defaults.
pub const MAX_DEPLOY_FILES: usize = 3000;
pub const MAX_DEPLOY_FILE_BYTES: usize = 100 * 1024 * 1024;

/// The bytes of every file and field value a form may carry (upstream's
/// `MAX_UPLOAD_TOTAL_SIZE`, 2 GiB there): the proxy's `client_max_body_size
/// 200m` on the same route, and the old `MAX_DEPLOYMENT_SIZE_BYTES` default.
pub const MAX_DEPLOYMENT_BYTES: usize = 200 * 1024 * 1024;

/// The most framing a part adds (upstream `MULTIPART_PART_FRAMING_BYTES`):
/// a 16 KiB header block plus a boundary line and CRLFs.
const MULTIPART_PART_FRAMING_BYTES: usize = 16 * 1024 + 128;

/// The largest body a form within the limits can have, framing included
/// (upstream `maxMultipartBodySize`); the route's body limit.
pub fn max_deployment_body_bytes(max_fields: usize) -> usize {
    MAX_DEPLOYMENT_BYTES + (MAX_DEPLOY_FILES + max_fields + 1) * MULTIPART_PART_FRAMING_BYTES
}

fn invalid(msg: impl Into<String>) -> AppError {
    AppError::InvalidRequest(msg.into())
}

fn too_large(msg: impl Into<String>) -> AppError {
    AppError::PayloadTooLarge(msg.into())
}

/// Upstream's 413 for a body past [`max_deployment_body_bytes`].
fn body_too_large(max_fields: usize) -> AppError {
    too_large(format!(
        "The request body is too large. The maximum allowed total upload size is {MAX_DEPLOYMENT_BYTES} bytes plus multipart framing ({} bytes in all).",
        max_deployment_body_bytes(max_fields)
    ))
}

/// A body that is not a `multipart/form-data` form.
const NOT_MULTIPART_MESSAGE: &str = "Invalid request: expected a multipart/form-data body";

/// A read error of the form: past the route's body limit, upstream's 413;
/// any other parser failure its fixed 400, the parser's text logged.
fn read_error(e: MultipartError, max_fields: usize) -> AppError {
    if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
        return body_too_large(max_fields);
    }
    tracing::debug!(error = %e, "POST /entities: unreadable multipart form");
    invalid("Invalid multipart/form-data request")
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

/// A form as read, or turned away before its files were buffered.
enum Form {
    Read(BTreeMap<String, String>, Vec<(String, Bytes)>),
    Refused(Response),
}

/// Reads the whole form with upstream's multipart limits and answers
/// (`multipart.ts`): text fields by name, at most `max_fields` of up to
/// `max_field_bytes` each, and file parts in order with their field names,
/// each up to [`MAX_DEPLOY_FILE_BYTES`], at most [`MAX_DEPLOY_FILES`] of
/// them, all values together up to [`MAX_DEPLOYMENT_BYTES`]; a limit passed
/// is a 413, a name sent twice a 400. `before_files` sees the fields read
/// before the first file part (the clients send theirs first) and may
/// refuse the form there.
async fn read_form(
    multipart: &mut Multipart,
    max_fields: usize,
    max_field_bytes: usize,
    before_files: impl AsyncFnOnce(&BTreeMap<String, String>) -> Option<Response>,
) -> AppResult<Form> {
    let mut fields: BTreeMap<String, String> = BTreeMap::new();
    let mut files: Vec<(String, Bytes)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut field_count = 0usize;
    let mut total_bytes = 0usize;
    let mut before_files = Some(before_files);
    let duplicate = |name: &str| invalid(format!("Duplicate form field '{name}'"));
    let mut count_total = |bytes: usize| {
        total_bytes = total_bytes.saturating_add(bytes);
        if total_bytes > MAX_DEPLOYMENT_BYTES {
            return Err(too_large(format!(
                "The request body is too large. The maximum allowed total upload size is {MAX_DEPLOYMENT_BYTES} bytes."
            )));
        }
        Ok(())
    };

    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| read_error(e, max_fields))?
    {
        let name = field.name().unwrap_or("").to_string();

        if is_file_part(field.file_name(), field.content_type()) {
            if let Some(gate) = before_files.take()
                && let Some(refused) = gate(&fields).await
            {
                return Ok(Form::Refused(refused));
            }
            if files.len() >= MAX_DEPLOY_FILES {
                return Err(too_large(format!(
                    "Too many files in the request. The maximum allowed is {MAX_DEPLOY_FILES}."
                )));
            }
            if !seen.insert(name.clone()) {
                return Err(duplicate(&name));
            }
            // Upstream prints busboy's file name, `undefined` for a part
            // without one (the Explorer names its parts by hash only).
            let file_name = field.file_name().unwrap_or("undefined").to_string();
            let mut buf = BytesMut::new();
            while let Some(chunk) = field.chunk().await.map_err(|e| read_error(e, max_fields))? {
                if buf.len().saturating_add(chunk.len()) > MAX_DEPLOY_FILE_BYTES {
                    return Err(too_large(format!(
                        "File '{file_name}' is too large. The maximum allowed size per file is {MAX_DEPLOY_FILE_BYTES} bytes."
                    )));
                }
                count_total(chunk.len())?;
                buf.extend_from_slice(&chunk);
            }
            files.push((name, buf.freeze()));
        } else {
            if field_count >= max_fields {
                return Err(too_large(format!(
                    "Too many form fields in the request. The maximum allowed is {max_fields}."
                )));
            }
            field_count += 1;
            let mut buf = Vec::new();
            while let Some(chunk) = field.chunk().await.map_err(|e| read_error(e, max_fields))? {
                if buf.len().saturating_add(chunk.len()) > max_field_bytes {
                    return Err(too_large(format!(
                        "Field '{name}' is too large. The maximum allowed size per field is {max_field_bytes} bytes."
                    )));
                }
                buf.extend_from_slice(&chunk);
            }
            if !seen.insert(name.clone()) {
                return Err(duplicate(&name));
            }
            count_total(buf.len())?;
            // busboy decodes a value as UTF-8 with replacement characters.
            fields.insert(name, String::from_utf8_lossy(&buf).into_owned());
        }
    }
    Ok(Form::Read(fields, files))
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
    let unavailable = !matches!(failure, DeployFailure::Rejected(_));
    let (status, errors) = match failure {
        DeployFailure::Rejected(errors) => (StatusCode::BAD_REQUEST, errors),
        DeployFailure::Unavailable(errors) | DeployFailure::Unverifiable(errors) => {
            (StatusCode::SERVICE_UNAVAILABLE, errors)
        }
    };
    let response = (status, Json(json!({ "errors": errors }))).into_response();
    if unavailable {
        return with_retry_after(
            response,
            header::HeaderValue::from_static(writes::UNAVAILABLE_RETRY_AFTER_SECS),
        );
    }
    response
}

/// `response` with a `Retry-After` of `value`.
fn with_retry_after(mut response: Response, value: header::HeaderValue) -> Response {
    response.headers_mut().insert(header::RETRY_AFTER, value);
    response
}

/// 429 with `Retry-After` for a client past its burst limit or daily quota.
fn quota_exceeded_response(retry_after_secs: u64) -> Response {
    with_retry_after(
        super::errors::json_error(StatusCode::TOO_MANY_REQUESTS, "Too many requests"),
        header::HeaderValue::from(retry_after_secs),
    )
}

/// The bytes a body can occupy: its declared length, else the body cap.
fn declared_body_bytes(headers: &HeaderMap) -> u64 {
    declared_length(headers)
        .unwrap_or(u64::MAX)
        .min(MAX_DEPLOYMENT_BYTES as u64)
}

/// The declared `Content-Length`, if any.
fn declared_length(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok())
}

/// Any other method on `/entities`: 405 with `Allow: POST`, or, read-only,
/// the not-found of upstream's `READ_ONLY` node, which has no such route.
pub async fn other_method(State(state): State<SharedState>) -> Response {
    if state.is_read_only() {
        return writes::read_only_response();
    }
    (
        StatusCode::METHOD_NOT_ALLOWED,
        [(header::ALLOW, header::HeaderValue::from_static("POST"))],
    )
        .into_response()
}

pub async fn create_entity(
    State(state): State<SharedState>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    multipart: Result<Multipart, MultipartRejection>,
) -> AppResult<Response> {
    let request_started = std::time::Instant::now();
    if let Some(refused) = writes::refuse(&state) {
        return Ok(refused);
    }
    let Some(deployer) = state.deployer().cloned() else {
        return Ok(writes::disabled_response());
    };
    let client_ip = crate::shared::client_ip::client_ip(&headers);
    // Upstream's burst limit: every request counts, none is given back.
    if let Err(retry_after) = state.deploy_burst.try_consume(&client_ip) {
        return Ok(quota_exceeded_response(retry_after));
    }
    if !state.sync.takes_some_deploys() {
        return Ok(writes::bootstrapping_response());
    }

    let Some(_reservation) = state
        .upload_budget
        .try_reserve(declared_body_bytes(&headers))
    else {
        return Ok(with_retry_after(
            super::errors::json_error(
                StatusCode::SERVICE_UNAVAILABLE,
                super::upload_admission::BUDGET_EXHAUSTED_MESSAGE,
            ),
            header::HeaderValue::from_static(writes::UNAVAILABLE_RETRY_AFTER_SECS),
        ));
    };
    // A declared partial batch is counted once its form is read
    // ([`partial_deployments`]): a source that spent its quota may still
    // resume its own uploads.
    let declared_partial = partial_deployments::is_declared(query.as_deref());
    // Upstream's parser refuses these before its quota counts the request,
    // and after its pre-body check of a spent quota.
    let max_fields = state.settings.max_upload_field_count;
    let multipart = match multipart {
        Err(_) => Err(invalid(NOT_MULTIPART_MESSAGE)),
        Ok(_)
            if declared_length(&headers)
                .is_some_and(|n| n > max_deployment_body_bytes(max_fields) as u64) =>
        {
            Err(body_too_large(max_fields))
        }
        Ok(multipart) => Ok(multipart),
    };
    let multipart = match multipart {
        Ok(multipart) => multipart,
        Err(refusal) => {
            if !declared_partial && let Some(retry_after) = state.deploy_quota.exhausted(&client_ip)
            {
                return Ok(quota_exceeded_response(retry_after));
            }
            return Err(refusal);
        }
    };
    let charge = if declared_partial {
        None
    } else {
        match state.deploy_quota.try_consume(&client_ip) {
            Ok(charge) => charge,
            Err(retry_after) => return Ok(quota_exceeded_response(retry_after)),
        }
    };

    let request = Request {
        headers: &headers,
        client_ip: &client_ip,
        declared_partial,
        // Undeclared, the source was admitted under its quota above.
        quota_exhausted: if declared_partial {
            state.deploy_quota.exhausted(&client_ip)
        } else {
            None
        },
        started: request_started,
    };
    Ok(
        match deploy_form(&state, deployer, &request, multipart).await {
            Ok(Answer::Decided(response)) => response,
            Ok(Answer::Undecided(response)) | Ok(Answer::Partial(response)) => {
                if let Some(charge) = charge {
                    charge.refund();
                }
                response
            }
            Err(e) => e.into_response(),
        },
    )
}

/// The 200 of a deployed entity: its stored `local_timestamp` in ms.
fn created_response(creation_timestamp: i64) -> Response {
    (
        StatusCode::OK,
        Json(json!({ "creationTimestamp": creation_timestamp })),
    )
        .into_response()
}

/// What `deploy_form` needs of the request besides its body.
struct Request<'a> {
    headers: &'a HeaderMap,
    client_ip: &'a str,
    declared_partial: bool,
    /// The daily quota's Retry-After when the source had spent it on arrival.
    quota_exhausted: Option<u64>,
    started: std::time::Instant,
}

/// An admitted deploy's answer: [`Answer::Undecided`] is the deployer's
/// 503 for this node's own state, which the client's quota gets back;
/// [`Answer::Partial`] a partial batch, which counted itself.
enum Answer {
    Decided(Response),
    Undecided(Response),
    Partial(Response),
}

/// Reads an admitted form and deploys it.
async fn deploy_form(
    state: &SharedState,
    deployer: std::sync::Arc<super::write_deployer::WriteDeployer>,
    request: &Request<'_>,
    mut multipart: Multipart,
) -> AppResult<Answer> {
    let headers = request.headers;
    let request_started = request.started;
    let settings = &state.settings;
    let is_partial = |fields: &BTreeMap<String, String>| {
        fields.get("partial").map(String::as_str) == Some("true")
    };
    let form = read_form(
        &mut multipart,
        settings.max_upload_field_count,
        settings.max_upload_field_bytes,
        async |fields: &BTreeMap<String, String>| {
            if !request.declared_partial && !is_partial(fields) {
                return None;
            }
            let entity_id = fields.get("entityId")?;
            let chain = auth_chain_of(fields, entity_id).ok()?;
            let signer = chain.get(0)?.get("payload")?.as_str()?;
            partial_deployments::admit_before_files(
                &deployer,
                &settings.partial_uploads,
                entity_id,
                signer,
                request.quota_exhausted,
            )
            .await
        },
    )
    .await?;
    let (fields, files) = match form {
        Form::Read(fields, files) => (fields, files),
        Form::Refused(response) => return Ok(Answer::Partial(response)),
    };
    if is_partial(&fields) {
        let answer = partial_batch(state, &deployer, request, fields, files).await;
        return Ok(Answer::Partial(
            answer.unwrap_or_else(IntoResponse::into_response),
        ));
    }
    let entity_id = fields
        .get("entityId")
        .ok_or_else(|| invalid("Missing entityId field"))?
        .clone();
    // Otherwise the query flag would carry a regular deploy past the quota.
    if request.declared_partial {
        return Err(invalid(
            "The 'partial=true' query parameter requires the 'partial=true' form field",
        ));
    }
    let auth_chain = auth_chain_of(&fields, &entity_id)?;
    // Deploys are idempotent: a replay answers the stored timestamp with no
    // signature, TTL or rate-limit check (its chain may have expired since).
    // A failed lookup goes on to the deploy, whose write answers a deployed
    // entity the same way.
    match deployer.deployed_timestamp(&entity_id).await {
        Ok(Some(creation_timestamp)) => {
            tracing::info!(entity_id = %entity_id, "POST /entities - Entity already deployed");
            return Ok(Answer::Decided(created_response(creation_timestamp)));
        }
        Ok(None) => {}
        Err(e) => {
            tracing::warn!(entity_id = %entity_id, error = %e, "POST /entities - replay lookup failed");
        }
    }
    let files: Vec<Bytes> = files.into_iter().map(|(_, bytes)| bytes).collect();
    if !state.sync.takes_deploys() {
        let Some(entity_type) = entity_type_of(&files, &entity_id) else {
            return Ok(Answer::Decided(deployment_failure_response(
                DeployFailure::Rejected(vec![
                    "The entity file was not part of the uploaded files or has no type."
                        .to_string(),
                ]),
            )));
        };
        if !state.sync.takes_deploys_of(&entity_type) {
            return Ok(Answer::Decided(writes::bootstrapping_response()));
        }
    }

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
            Ok(Answer::Decided(created_response(creation_timestamp)))
        }
        Err(failure) => {
            tracing::warn!(entity_id = %entity_id, errors = ?failure.errors(), "POST /entities - Deployment failed");
            let undecided = matches!(failure, DeployFailure::Unavailable(_));
            let response = deployment_failure_response(failure);
            Ok(if undecided {
                Answer::Undecided(response)
            } else {
                Answer::Decided(response)
            })
        }
    }
}

/// A batch of a partial upload. As upstream's `withDeploymentQuota`, the
/// daily quota counts the upload once, when its first batch creates it, and
/// a batch refused with a 400 before joining its upload (a missing
/// `entityId` or a bad chain included); its 429s, 503s and replays are not
/// counted. A spent source may only resume the signer's own upload.
async fn partial_batch(
    state: &SharedState,
    deployer: &super::write_deployer::WriteDeployer,
    request: &Request<'_>,
    fields: BTreeMap<String, String>,
    files: Vec<(String, Bytes)>,
) -> AppResult<Response> {
    if !state.sync.takes_deploys_of("scene") {
        return Ok(writes::bootstrapping_response());
    }
    // Counted for good: the charge is never refunded.
    let count = || {
        let _ = state.deploy_quota.try_consume(request.client_ip);
    };
    let Some(entity_id) = fields.get("entityId").cloned() else {
        count();
        return Err(invalid("Missing entityId field"));
    };
    let auth_chain = match auth_chain_of(&fields, &entity_id) {
        Ok(chain) => chain,
        Err(e) => {
            count();
            return Err(e);
        }
    };
    tracing::info!(
        entity_id = %entity_id,
        file_count = files.len(),
        "POST /entities - Partial deployment batch"
    );
    let batch = partial_deployments::Batch {
        entity_id,
        auth_chain,
        files,
        requested_at_ms: chrono::Utc::now().timestamp_millis()
            - request.started.elapsed().as_millis() as i64,
    };
    let mut admission = partial_deployments::Admission::default();
    let response = partial_deployments::handle(
        deployer,
        &state.settings.partial_uploads,
        batch,
        request.quota_exhausted,
        &mut admission,
    )
    .await;
    if admission.created || (response.status() == StatusCode::BAD_REQUEST && !admission.admitted) {
        count();
    }
    if response.status() == StatusCode::OK {
        state.deployments_cache.clear();
    }
    Ok(response)
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
