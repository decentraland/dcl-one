//! `POST /entities` (root and `/content`): the local deploy. A
//! `multipart/form-data` body carries the `entityId` field, the auth chain
//! (an `authChain` JSON field or the `authChain[i][type|payload|signature]`
//! fields, checked as `@dcl/schemas`' `AuthChain.validate` checks it) and
//! the files, the entity file among them. The write deployer
//! ([`super::write_deployer::WriteDeployer::deploy_entity`]) hashes the
//! uploads, refuses a denylisted id, checks the signature, then upstream's
//! server checks, runs the content validator (the LAND leg through
//! `land_authz`) and persists the whole deployment in ONE call of the
//! hash-named deploy function, `content.local_entities` provenance included
//! for a local scene.
//!
//! Answers: 200 `{creationTimestamp}`, the stored `local_timestamp`; an
//! entity already deployed answers its stored one once the form carries its
//! entity file, which names nothing denylisted, and a chain that signs the id
//! at the entity's timestamp, and otherwise the error a fresh deploy of it
//! gets, before any TTL, rate-limit or access check. Upstream answers a
//! replay right after the chain's shape check, unsigned and without the
//! entity file; this node does not. 400
//! `{errors}` for a refused deployment and `{error}` for a malformed form,
//! 413 `{error}` for one past a form limit (upstream's `MAX_UPLOAD_*`
//! variables, defaults and texts: 3000 files of up to 100 MiB, 2 GiB in
//! all), 408 `{error}` for a
//! body still arriving at the upload timeout (`MULTIPART_UPLOAD_TIMEOUT_MS`,
//! upstream's text with what arrived); 503 `{errors}` with `Retry-After`
//! only when this node could not decide for its own state, its cause
//! logged, not answered ([`deployment_failure_response`] has the mapping).
//! The gates of
//! [`super::writes`] come before any of it (404 read-only, 503 disabled),
//! then the 503 with `Retry-After: 60` while the node is bootstrapping.
//!
//! The files are spooled as upstream spools them: each file part is
//! streamed, hashed and checked against the limits into its own file under
//! the request's directory in `UPLOAD_SPOOL_FOLDER`
//! (`<STORAGE_ROOT_FOLDER>/tmp/content-uploads` by default), removed when the
//! request ends, by success, refusal, error or disconnect; a crashed
//! process's directories go at the next start. The spool hashes each part
//! (CIDv0 and CIDv1) as it writes it; the entity file is then read onto the
//! heap, and the deployer reads any other part only when a validation
//! inspects its bytes (a thumbnail), with ordinary I/O whose failure is the
//! spool's 503, and stores each one from its spool file. Two budgets admit
//! a request before its body, as upstream's: the spool budget
//! (`MAX_IN_FLIGHT_UPLOAD_BYTES`, never below one maximal form) admits it
//! when its declared payload (none for a chunked body) fits beside what is
//! reserved, holds its directory's [`SPOOL_FILE_OVERHEAD_BYTES`] and grows
//! with what it writes, so a slow or short body never holds bytes it has
//! not sent. That 16 KiB is what bounds the bodies in flight, beside each
//! source's slots (`MAX_CONCURRENT_UPLOADS_PER_SOURCE`) and the upload
//! timeout: the default 4 GiB holds 262,144 idle bodies, 65,536 sources at
//! 4 each, where upstream's 16 MiB minimum fills with 256. The memory budget
//! (`MAX_IN_MEMORY_DEPLOYMENT_BYTES`, never below [`max_heap_share`]) admits
//! it when a fixed 16 MiB for its fields and parsing fits, holding nothing
//! while the body arrives, and once the form is read reserves that share,
//! its entity file and the largest file its validation reads. A budget that
//! does not fit, or a spool the disk refuses (full, failing), answers 503
//! with `Retry-After`.
//! Before the body is read, in upstream's order: a client address past its
//! daily quota 429, a body that does not fit the in-memory upload budget
//! 503, and a client with its uploads in flight, or without room in its byte
//! share for the declared body, 429; the share then grows with the bytes
//! received, a 429 when it runs out
//! ([`super::upload_admission`], per process), each with `Retry-After`; a
//! body that is not a form, or declares more than a form can hold, then
//! answers 400 or 413. The quota counts a request once its form is read,
//! as upstream's `withDeploymentQuota`, and gives back only the ones the
//! deployer could not decide for its own state (its 503), which upstream
//! keeps. A contract-wallet signature this node cannot check (503) and an
//! unsynced copy refusing the entity's type (503) still count: the client
//! chose them.
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
//! quota is not checked on admission but once the form is read (the upload
//! counted once, when created, a 400 before joining it, and a replay refused
//! or asking a contract wallet); the bare query flag is a 400. Once the fields
//! before the first file are read, a batch from a source that spent its
//! quota and not resuming its signer's own upload, or from an account over
//! its byte rate, is turned away (429) before any file is buffered.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use axum::Json;
use axum::extract::multipart::MultipartError;
use axum::extract::{FromRequest, Multipart, RawQuery, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use futures::StreamExt;
use serde_json::{Value, json};

use super::errors::{AppError, AppResult};
use super::partial_deployments;
use super::upload_admission::{Reservation, SourceLease};
use super::write_deployer::{DeployFailure, Upload};
use super::writes;
use super::{Settings, SharedState};
use crate::auth::MAX_AUTH_CHAIN_LINKS;
use crate::storage::upload_spool::{SpooledFile, UploadSpool};

/// What each spooled file is charged on the spool budget beside its bytes
/// (upstream `SPOOL_FILE_OVERHEAD_BYTES`), so the budget bounds inodes too.
/// A request's directory is charged the same from admission, all it holds
/// before its first bytes arrive.
pub const SPOOL_FILE_OVERHEAD_BYTES: u64 = 16 * 1024;

/// Upstream's answer when the spool cannot be written (full or failing disk).
pub const SPOOL_UNAVAILABLE_MESSAGE: &str =
    "The server cannot store uploads right now, please retry shortly.";

/// The label of this route's spool metrics and cleanup counter.
const SPOOL_ROUTE: &str = "content-entities";

/// The spool's default place: `<STORAGE_ROOT_FOLDER>/tmp/content-uploads`,
/// on the blob store's filesystem.
pub fn default_spool_root(storage: &crate::storage::Storage) -> std::path::PathBuf {
    storage.scratch_dir().join("content-uploads")
}

/// The spool bytes one maximal form takes: every value plus the charge of
/// each file and of its directory; the spool budget never sits below it.
pub fn max_spool_footprint(settings: &Settings) -> u64 {
    (settings.max_upload_total_bytes as u64).saturating_add(
        (settings.max_upload_file_count as u64 + 1).saturating_mul(SPOOL_FILE_OVERHEAD_BYTES),
    )
}

/// The most memory one request's reservation grows to ([`uploads_of`]): the
/// fixed share for its fields and parsing, its entity file and the largest
/// file its validation reads, each up to `MAX_UPLOAD_FILE_SIZE`. The memory
/// budget never sits below it, so a form within the limits always fits a
/// node with no other deploy in flight.
pub fn max_heap_share(settings: &Settings) -> u64 {
    super::upload_admission::MIN_UPLOAD_RESERVATION_BYTES
        .saturating_add((settings.max_upload_file_bytes as u64).saturating_mul(2))
}

/// The most framing a part adds (upstream `MULTIPART_PART_FRAMING_BYTES`):
/// a 16 KiB header block plus a boundary line and CRLFs.
const MULTIPART_PART_FRAMING_BYTES: usize = 16 * 1024 + 128;

/// The largest body a form within the `MAX_UPLOAD_*` limits can have,
/// framing included (upstream `maxMultipartBodySize`); the route's body
/// limit. The gateway's body cap on the route (`CONTENT_DEPLOY_CAP` in
/// `infra/gateway/gateway.py`) sits just above it at the defaults; a gateway
/// test reads this formula and the defaults.
pub fn max_deployment_body_bytes(settings: &Settings) -> usize {
    multipart_body_limit(
        settings.max_upload_total_bytes,
        settings.max_upload_file_count,
        settings.max_upload_field_count,
    )
}

/// The gateway's body cap on this route as shipped (`CONTENT_DEPLOY_CAP`,
/// 2096 MiB, in `infra/gateway/gateway.py`; a gateway test pins the two).
pub const SHIPPED_GATEWAY_DEPLOY_CAP_BYTES: usize = 2096 * 1024 * 1024;

/// The startup warning for `MAX_UPLOAD_*` values whose route limit
/// ([`max_deployment_body_bytes`]) is past the shipped gateway cap: a body
/// between the two meets nginx's HTML 413 instead of this route's JSON one
/// unless the gateway's cap is raised to match.
pub fn gateway_cap_warning(settings: &Settings) -> Option<String> {
    let limit = max_deployment_body_bytes(settings);
    (limit > SHIPPED_GATEWAY_DEPLOY_CAP_BYTES).then(|| {
        format!(
            "MAX_UPLOAD_TOTAL_SIZE, MAX_UPLOAD_FILE_COUNT and MAX_UPLOAD_FIELD_COUNT allow \
             POST /entities bodies of {limit} bytes, past the shipped gateway cap \
             (CONTENT_DEPLOY_CAP, {SHIPPED_GATEWAY_DEPLOY_CAP_BYTES} bytes): larger bodies \
             meet the gateway's HTML 413 unless its cap is raised to match"
        )
    })
}

fn multipart_body_limit(max_total: usize, max_files: usize, max_fields: usize) -> usize {
    max_total.saturating_add(
        (max_files.saturating_add(max_fields).saturating_add(1))
            .saturating_mul(MULTIPART_PART_FRAMING_BYTES),
    )
}

fn invalid(msg: impl Into<String>) -> AppError {
    AppError::InvalidRequest(msg.into())
}

fn too_large(msg: impl Into<String>) -> AppError {
    AppError::PayloadTooLarge(msg.into())
}

/// Upstream's 413 for a body past [`max_deployment_body_bytes`].
fn body_too_large(settings: &Settings) -> AppError {
    too_large(format!(
        "The request body is too large. The maximum allowed total upload size is {} bytes plus multipart framing ({} bytes in all).",
        settings.max_upload_total_bytes,
        max_deployment_body_bytes(settings)
    ))
}

/// A form without the `entityId` field.
const MISSING_ENTITY_ID_MESSAGE: &str = "Missing required field: entityId";

/// A body that is not a `multipart/form-data` form.
const NOT_MULTIPART_MESSAGE: &str = "Invalid request: expected a multipart/form-data body";

/// A read error of the form: past the route's body limit, upstream's 413;
/// any other parser failure its fixed 400, the parser's text logged.
fn read_error(e: MultipartError, settings: &Settings) -> AppError {
    if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
        return body_too_large(settings);
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

    let Some(biggest_index) = auth_chain_field_max_index(fields.keys()) else {
        return Ok(None);
    };
    if biggest_index >= MAX_AUTH_CHAIN_LINKS as u64 {
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

/// The largest link index the indexed form fields name: the first
/// `authChain[<digits>]` in each field name (`/authChain\[(\d+)\]/`),
/// saturating; `None` when no name carries one.
pub fn auth_chain_field_max_index<'a>(keys: impl IntoIterator<Item = &'a String>) -> Option<u64> {
    keys.into_iter()
        .filter_map(|key| auth_chain_field_index(key))
        .max()
}

fn auth_chain_field_index(key: &str) -> Option<u64> {
    const OPEN: &str = "authChain[";
    key.match_indices(OPEN).find_map(|(at, _)| {
        let rest = &key[at + OPEN.len()..];
        let digits = &rest[..rest.bytes().take_while(u8::is_ascii_digit).count()];
        (!digits.is_empty() && rest[digits.len()..].starts_with(']'))
            .then(|| digits.parse::<u64>().unwrap_or(u64::MAX))
    })
}

fn indexed_field<'a>(
    fields: &'a BTreeMap<String, String>,
    i: u64,
    part: &str,
) -> Result<&'a String, String> {
    fields
        .get(&format!("authChain[{i}][{part}]"))
        .ok_or_else(|| format!("Missing auth chain element at index {i}"))
}

fn too_long_message() -> String {
    format!("Auth chain is too long; the maximum allowed is {MAX_AUTH_CHAIN_LINKS} elements")
}

/// The `type` values `@dcl/schemas` admits in an auth link.
const AUTH_LINK_TYPES: [&str; 5] = [
    "SIGNER",
    "ECDSA_EPHEMERAL",
    "ECDSA_SIGNED_ENTITY",
    "ECDSA_EIP_1654_EPHEMERAL",
    "ECDSA_EIP_1654_SIGNED_ENTITY",
];

/// `AuthChain.validate` of `@dcl/schemas`: at least one link, each an
/// object with a known `type` and a string `payload`; `signature` a string
/// or null, required on every link but a SIGNER, whose signature is absent,
/// null or empty.
fn is_valid_auth_chain(chain: &Value) -> bool {
    let Some(links) = chain.as_array().filter(|links| !links.is_empty()) else {
        return false;
    };
    links.iter().all(|link| {
        let Some(link) = link.as_object() else {
            return false;
        };
        let Some(link_type) = link
            .get("type")
            .and_then(Value::as_str)
            .filter(|t| AUTH_LINK_TYPES.contains(t))
        else {
            return false;
        };
        let signer = link_type == "SIGNER";
        link.get("payload").is_some_and(Value::is_string)
            && match link.get("signature") {
                None => signer,
                Some(Value::Null) => true,
                Some(Value::String(signature)) => !signer || signature.is_empty(),
                Some(_) => false,
            }
    })
}

/// The chain of a deploy form, as upstream's handler takes it: a form
/// without one is upstream's `No auth chain can be derivated` (sic), and a
/// chain `AuthChain.validate` refuses its `Invalid auth chain`.
fn auth_chain_of(fields: &BTreeMap<String, String>) -> AppResult<Value> {
    let chain = extract_auth_chain_from_fields(fields)
        .map_err(invalid)?
        .ok_or_else(|| invalid("No auth chain can be derivated"))?;
    if !is_valid_auth_chain(&chain) {
        return Err(invalid("Invalid auth chain"));
    }
    Ok(chain)
}

/// A part is a file when it names one or carries a non-text body: the Explorer
/// sends each file as a bare `application/octet-stream` part named by its hash.
fn is_file_part(file_name: Option<&str>, content_type: Option<&str>) -> bool {
    file_name.is_some()
        || content_type.is_some_and(|t| !t.trim_start().to_ascii_lowercase().starts_with("text/"))
}

/// A form as read, or turned away before or while its files were spooled.
enum Form {
    Read(BTreeMap<String, String>, Vec<SpooledFile>),
    Refused(Response),
}

/// What a request spools into: its directory, and its share of the spool
/// budget and of its source's allowance, both grown as its bytes arrive.
struct Spooling<'a, 'b, 'c> {
    spool: &'a mut UploadSpool,
    disk: &'a mut Reservation<'b>,
    source: Option<&'a mut SourceLease<'c>>,
}

/// Reads the whole form with upstream's multipart limits and answers
/// (`multipart.ts`): text fields by name in memory, at most
/// `MAX_UPLOAD_FIELD_COUNT` of up to `MAX_UPLOAD_FIELD_SIZE` each, and file
/// parts in order with their field names, streamed and hashed into the
/// request's spool, each up to `MAX_UPLOAD_FILE_SIZE`, at most
/// `MAX_UPLOAD_FILE_COUNT` of them, all values together up to
/// `MAX_UPLOAD_TOTAL_SIZE`; a limit passed is a 413 as the bytes arrive, a
/// name sent twice a 400. The request's spool reservation grows with what
/// it wrote (each file and the directory charged [`SPOOL_FILE_OVERHEAD_BYTES`] more); past the
/// budget, or on a write the disk refuses (full, failing), the form is
/// refused with a 503 and `Retry-After`. `before_files` sees the fields read
/// before the first file part (the clients send theirs first) and may
/// refuse the form there.
async fn read_form(
    multipart: &mut Multipart,
    settings: &Settings,
    spooling: Spooling<'_, '_, '_>,
    before_files: impl AsyncFnOnce(&BTreeMap<String, String>) -> Option<Response>,
) -> AppResult<Form> {
    let max_fields = settings.max_upload_field_count;
    let max_field_bytes = settings.max_upload_field_bytes;
    let max_files = settings.max_upload_file_count;
    let max_file_bytes = settings.max_upload_file_bytes;
    let max_total = settings.max_upload_total_bytes;
    let Spooling {
        spool,
        disk,
        mut source,
    } = spooling;
    let mut fields: BTreeMap<String, String> = BTreeMap::new();
    let mut files: Vec<SpooledFile> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut field_count = 0usize;
    let mut total_bytes = 0usize;
    let mut before_files = Some(before_files);
    let duplicate = |name: &str| invalid(format!("Duplicate form field '{name}'"));
    let over_total = |total: usize| {
        (total > max_total).then(|| {
            too_large(format!(
                "The request body is too large. The maximum allowed total upload size is {max_total} bytes."
            ))
        })
    };

    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| read_error(e, settings))?
    {
        let name = field.name().unwrap_or("").to_string();

        if is_file_part(field.file_name(), field.content_type()) {
            if let Some(gate) = before_files.take()
                && let Some(refused) = gate(&fields).await
            {
                return Ok(Form::Refused(refused));
            }
            if files.len() >= max_files {
                return Err(too_large(format!(
                    "Too many files in the request. The maximum allowed is {max_files}."
                )));
            }
            if !seen.insert(name.clone()) {
                return Err(duplicate(&name));
            }
            // Upstream prints busboy's file name, `undefined` for a part
            // without one (the Explorer names its parts by hash only).
            let file_name = field.file_name().unwrap_or("undefined").to_string();
            let mut part = match spool.part().await {
                Ok(part) => part,
                Err(e) => return Ok(Form::Refused(spool_unavailable(&e))),
            };
            // The files before it, this one and the request's directory.
            let charged = (files.len() as u64 + 2) * SPOOL_FILE_OVERHEAD_BYTES;
            while let Some(chunk) = field.chunk().await.map_err(|e| read_error(e, settings))? {
                if part.len().saturating_add(chunk.len()) > max_file_bytes {
                    return Err(too_large(format!(
                        "File '{file_name}' is too large. The maximum allowed size per file is {max_file_bytes} bytes."
                    )));
                }
                total_bytes = total_bytes.saturating_add(chunk.len());
                if let Some(e) = over_total(total_bytes) {
                    return Err(e);
                }
                // Checked after the limits: an oversize body gets the final
                // 413, never the retryable 503.
                if !disk.grow(total_bytes as u64 + charged) {
                    return Ok(Form::Refused(budget_exhausted_after(total_bytes as u64)));
                }
                if let Some(source) = source.as_deref_mut()
                    && !source.grow(total_bytes as u64)
                {
                    return Ok(Form::Refused(source_busy()));
                }
                if let Err(e) = part.write(&chunk).await {
                    return Ok(Form::Refused(spool_unavailable(&e)));
                }
            }
            match part.finish(name).await {
                Ok(spooled) => files.push(spooled),
                Err(e) => return Ok(Form::Refused(spool_unavailable(&e))),
            }
        } else {
            if field_count >= max_fields {
                return Err(too_large(format!(
                    "Too many form fields in the request. The maximum allowed is {max_fields}."
                )));
            }
            field_count += 1;
            let mut buf = Vec::new();
            while let Some(chunk) = field.chunk().await.map_err(|e| read_error(e, settings))? {
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
            total_bytes = total_bytes.saturating_add(buf.len());
            if let Some(e) = over_total(total_bytes) {
                return Err(e);
            }
            // busboy decodes a value as UTF-8 with replacement characters.
            fields.insert(name, String::from_utf8_lossy(&buf).into_owned());
        }
    }
    Ok(Form::Read(fields, files))
}

/// Upstream's 503 for a spool the disk refuses (full, failing): the cause
/// is logged, the client told to retry.
fn spool_unavailable(e: &std::io::Error) -> Response {
    tracing::error!(error = %e, "POST /entities: the upload spool could not be written");
    metrics::counter!("upload_spool_failures_total", "route" => SPOOL_ROUTE).increment(1);
    with_retry_after(
        super::errors::json_error(StatusCode::SERVICE_UNAVAILABLE, SPOOL_UNAVAILABLE_MESSAGE),
        header::HeaderValue::from_static(writes::UNAVAILABLE_RETRY_AFTER_SECS),
    )
}

/// [`budget_exhausted`] for a request whose body had begun to arrive:
/// the bytes it sent, which a retrying client sends again, are recorded in
/// `upload_budget_refused_received_bytes`.
fn budget_exhausted_after(received: u64) -> Response {
    metrics::histogram!("upload_budget_refused_received_bytes", "route" => SPOOL_ROUTE)
        .record(received as f64);
    budget_exhausted()
}

/// The 503 of a full upload budget, memory or spool.
fn budget_exhausted() -> Response {
    with_retry_after(
        super::errors::json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            super::upload_admission::BUDGET_EXHAUSTED_MESSAGE,
        ),
        header::HeaderValue::from_static(writes::UNAVAILABLE_RETRY_AFTER_SECS),
    )
}

/// The form's files as the deployer takes them, by field name: the entity
/// file (the first part hashing to `entityId`) read onto the heap and
/// returned beside them, every other part from its spool file. `memory`,
/// empty while the body arrived, is reserved the fixed share, then the
/// entity file before the read and, once it parses, the largest part its
/// validation reads ([`crate::validator::rules::inspected_hashes`]).
/// A grow that does not fit is the budget's 503, a failed read the spool's.
async fn uploads_of(
    fields: &BTreeMap<String, String>,
    parts: Vec<SpooledFile>,
    memory: &mut Reservation<'_>,
) -> Result<(Vec<(String, Upload)>, Option<Bytes>), Response> {
    let entity_at = fields
        .get("entityId")
        .and_then(|id| parts.iter().position(|part| part.names(id)));
    let received: u64 = parts.iter().map(|part| part.len() as u64).sum();
    if !memory.grow(super::upload_admission::MIN_UPLOAD_RESERVATION_BYTES) {
        return Err(budget_exhausted_after(received));
    }
    let mut entity = None;
    if let Some(at) = entity_at {
        let part = &parts[at];
        if !memory.grow(memory.bytes() + part.len() as u64) {
            return Err(budget_exhausted_after(received));
        }
        let bytes = Bytes::from(part.read().await.map_err(|e| spool_unavailable(&e))?);
        let inspected = serde_json::from_slice::<crate::entity::Entity>(&bytes)
            .map(|parsed| {
                let hashes = crate::validator::rules::inspected_hashes(&parsed);
                parts
                    .iter()
                    .filter(|part| hashes.iter().any(|hash| part.names(hash)))
                    .map(|part| part.len() as u64)
                    .max()
                    .unwrap_or(0)
            })
            .unwrap_or(0);
        if !memory.grow(memory.bytes() + inspected) {
            return Err(budget_exhausted_after(received));
        }
        entity = Some(bytes);
    }
    let uploads = parts
        .into_iter()
        .enumerate()
        .map(|(at, part)| match &entity {
            Some(bytes) if Some(at) == entity_at => (part.name, Upload::Bytes(bytes.clone())),
            _ => (part.name.clone(), Upload::Spooled(part)),
        })
        .collect();
    Ok((uploads, entity))
}

/// The `type` of the uploaded entity file, for the unsynced copy's per-type
/// gate; the deployer checks the same file in full.
fn entity_type_of(entity: Option<&Bytes>) -> Option<String> {
    entity_file_of(entity).and_then(|v| v.get("type")?.as_str().map(str::to_string))
}

/// The entity file ([`uploads_of`]: the part hashing to the id), parsed.
fn entity_file_of(entity: Option<&Bytes>) -> Option<Value> {
    entity.and_then(|bytes| serde_json::from_slice::<Value>(bytes).ok())
}

/// Whether an undecided deploy's quota charge is given back: only when its
/// access check fits one Checker batch and asks only Decentraland's own
/// contracts (LAND, estates, the NAME registrar). A larger request, or a
/// contract the entity names (a collection, a worn item's NFT), can drive the
/// check past its deadline or into a non-revert failure.
fn is_refundable(entity: Option<&Bytes>) -> bool {
    entity_file_of(entity).is_some_and(|entity| {
        !asks_named_contracts(&entity)
            && access_calls_of(&entity) <= crate::validator::onchain::CALLS_PER_BATCH
    })
}

/// Whether the access check calls contracts the entity names: a wearable's or
/// emote's collection, or an on-chain item a profile or outfit wears.
fn asks_named_contracts(entity: &Value) -> bool {
    if matches!(
        entity.get("type").and_then(Value::as_str),
        Some("wearable" | "emote")
    ) {
        return true;
    }
    let on_chain = |urn: &Value| {
        let urn = urn.as_str().or_else(|| urn.get("urn")?.as_str());
        urn.is_some_and(|u| {
            crate::validator::onchain::items::item_ref(u)
                .on_l1()
                .is_some()
        })
    };
    let worn = |list: Option<&Value>| {
        list.and_then(Value::as_array)
            .is_some_and(|items| items.iter().any(on_chain))
    };
    let Some(metadata) = entity.get("metadata") else {
        return false;
    };
    let avatars = metadata.get("avatars").and_then(Value::as_array);
    let outfits = metadata.get("outfits").and_then(Value::as_array);
    avatars.into_iter().flatten().any(|a| {
        let a = a.get("avatar");
        worn(a.and_then(|a| a.get("wearables"))) || worn(a.and_then(|a| a.get("emotes")))
    }) || outfits
        .into_iter()
        .flatten()
        .any(|o| worn(o.get("outfit").and_then(|o| o.get("wearables"))))
}

/// An upper bound on the on-chain sub-calls the access check asks for `entity`:
/// its pointers (LAND parcels), a wearable's or emote's content hashes, and the
/// names and items its profiles and outfits claim.
fn access_calls_of(entity: &Value) -> usize {
    let len = |v: Option<&Value>| v.and_then(Value::as_array).map_or(0, Vec::len);
    let mut calls = len(entity.get("pointers"));
    if matches!(
        entity.get("type").and_then(Value::as_str),
        Some("wearable" | "emote")
    ) {
        calls += len(entity.get("content"));
    }
    let Some(metadata) = entity.get("metadata") else {
        return calls;
    };
    for avatar in metadata
        .get("avatars")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        calls += usize::from(avatar.get("hasClaimedName") == Some(&Value::Bool(true)));
        let worn = avatar.get("avatar");
        calls += len(worn.and_then(|a| a.get("wearables")));
        calls += len(worn.and_then(|a| a.get("emotes")));
    }
    for outfit in metadata
        .get("outfits")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        calls += len(outfit.get("outfit").and_then(|o| o.get("wearables")));
    }
    calls + len(metadata.get("namesForExtraSlots"))
}

/// The answer to a refused or undecided deploy. A failure that follows
/// from the request is a 400 `{errors}`, as upstream answers it; only a
/// failure of this node's side is a 503 `{errors}` with `Retry-After`:
///
/// | failure | answer |
/// |---|---|
/// | the entity, its files, signature, rights, pointers, TTLs, rate limits, metadata the database cannot store (a NUL), more items than one access check asks about | 400 |
/// | a contract-wallet (EIP-1654) contract answering no, or reverting | 400 |
/// | the database or content store failing, a store collection racing the deploy | 503 |
/// | an access-check RPC unreachable or failing, or past the access-check deadline | 503 |
/// | a source unset: no `ETH_RPC_URL` for a contract-wallet signature | 503 |
/// | a read form not decided within `DEPLOYMENT_PROCESSING_TIMEOUT_MS` | 503 |
///
/// Upstream answers a failed access-check RPC with the 400 of a refused
/// right; this node answers 503, so a client retries what the chain did not
/// refuse.
pub fn deployment_failure_response(failure: DeployFailure) -> Response {
    let unavailable = !matches!(failure, DeployFailure::Rejected(_, _));
    let (status, errors) = match failure {
        DeployFailure::Rejected(_, errors) => (StatusCode::BAD_REQUEST, errors),
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

/// The 503 of a read form not decided within `DEPLOYMENT_PROCESSING_TIMEOUT_MS`
/// (worlds' text). Dropping the deploy there is safe: a client disconnect
/// drops it at the same points, its guards release on drop, stored blobs are
/// content-addressed and the write is one statement, so it either committed
/// (a retry then answers the stored timestamp) or did not.
fn processing_timed_out(timeout: Duration) -> Response {
    tracing::warn!(
        timeout_ms = timeout.as_millis() as u64,
        "POST /entities: deployment processing timed out"
    );
    deployment_failure_response(DeployFailure::Unavailable(vec![format!(
        "Deployment processing exceeded the {}ms deadline.",
        timeout.as_millis()
    )]))
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

/// The bytes a body may grow to, which its source's share must have room
/// for on admission: its declared length, else `MAX_UPLOAD_TOTAL_SIZE`.
fn declared_body_bytes(headers: &HeaderMap, settings: &Settings) -> u64 {
    declared_length(headers)
        .unwrap_or(u64::MAX)
        .min(settings.max_upload_total_bytes as u64)
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

/// The client the `POST /entities` limits count. Behind the gateway the
/// socket peer is a proxy in `HTTP_TRUSTED_PROXY_CIDRS`, and its single
/// `X-Real-IP` names the client (the proxy itself when missing); any other
/// peer is counted by its own address, since the route is reachable without
/// the gateway and a forwarding header from it is the client's own choice.
/// Without a socket (in-process callers and tests) the forwarded address is
/// read as before.
fn deploy_client(
    headers: &HeaderMap,
    peer: Option<std::net::IpAddr>,
    trusted_proxies: &[ipnet::IpNet],
) -> String {
    use crate::shared::client_ip::{client_ip, is_trusted_proxy, single_real_ip};
    let Some(peer) = peer.map(|p| p.to_canonical()) else {
        return client_ip(headers);
    };
    if is_trusted_proxy(peer, trusted_proxies) {
        single_real_ip(headers).unwrap_or(peer).to_string()
    } else {
        if looks_like_an_untrusted_proxy(headers, peer) {
            warn_untrusted_proxy_once(peer);
        }
        peer.to_string()
    }
}

/// An untrusted private or loopback peer that forwards `X-Real-IP`: most
/// likely the gateway or another backend proxy missing from
/// `HTTP_TRUSTED_PROXY_CIDRS`, so every deployer behind it shares its limits.
/// A direct client on a public address, or a local one sending no header, is
/// not one.
fn looks_like_an_untrusted_proxy(headers: &HeaderMap, peer: std::net::IpAddr) -> bool {
    use crate::shared::client_ip::{is_internal, single_real_ip};
    // A header equal to the peer is the server's own pin, not a proxy's.
    is_internal(peer)
        && headers.contains_key("x-real-ip")
        && single_real_ip(headers) != Some(peer.to_canonical())
}

/// Once per process: a node without the setting cannot tell its proxy from a
/// client, so it says so instead of refusing to start.
fn warn_untrusted_proxy_once(peer: std::net::IpAddr) {
    static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !WARNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        tracing::warn!(
            %peer,
            "POST /entities: a private or loopback peer forwards X-Real-IP but is not in \
             HTTP_TRUSTED_PROXY_CIDRS, so its clients share one deploy limit and daily quota; \
             list the backend proxy networks there"
        );
    }
}

pub async fn create_entity(
    State(state): State<SharedState>,
    RawQuery(query): RawQuery,
    connect: Option<axum::Extension<axum::extract::ConnectInfo<std::net::SocketAddr>>>,
    headers: HeaderMap,
    request: axum::extract::Request,
) -> AppResult<Response> {
    let request_started = std::time::Instant::now();
    if let Some(refused) = writes::refuse(&state) {
        return Ok(refused);
    }
    let Some(deployer) = state.deployer().cloned() else {
        return Ok(writes::disabled_response());
    };
    let client_ip = deploy_client(
        &headers,
        connect.map(|axum::Extension(axum::extract::ConnectInfo(addr))| addr.ip()),
        &state.settings.trusted_proxy_cidrs,
    );
    // Upstream's burst limit: every request counts, none is given back.
    if let Err(retry_after) = state.deploy_burst.try_consume(&client_ip) {
        return Ok(quota_exceeded_response(retry_after));
    }
    if !state.sync.takes_some_deploys() {
        return Ok(writes::bootstrapping_response());
    }
    // A declared partial batch is checked against the daily quota once its
    // form is read ([`partial_deployments`]): a source that spent its quota
    // may still resume its own uploads. Any other request of a spent source
    // is turned away here, unread and uncounted.
    let declared_partial = partial_deployments::is_declared(query.as_deref());
    if !declared_partial && let Some(retry_after) = state.deploy_quota.exhausted(&client_ip) {
        return Ok(quota_exceeded_response(retry_after));
    }

    // Admission checks fit and holds at most a directory's charge; each
    // reservation grows with what is real, so a slow or short body holds
    // only what it sent. Memory: its fixed share (fields, parsing) must fit;
    // it is reserved with the real sizes once the form is read
    // ([`uploads_of`]), as upstream's memory admission only checks `fits`.
    // Disk: the declared payload must fit (none for a chunked body); the
    // reservation holds the request directory's charge and grows with what
    // the spool holds. Source: the declared body must fit its
    // share; the lease grows with the bytes received.
    let Some(mut memory) = state
        .upload_budget
        .try_admit(super::upload_admission::MIN_UPLOAD_RESERVATION_BYTES, 0)
    else {
        return Ok(budget_exhausted());
    };
    let declared_payload = declared_length(&headers)
        .unwrap_or(0)
        .min(state.settings.max_upload_total_bytes as u64);
    let Some(mut disk) = state
        .spool_budget
        .try_admit(declared_payload, SPOOL_FILE_OVERHEAD_BYTES)
    else {
        return Ok(budget_exhausted());
    };
    let body_bytes = declared_body_bytes(&headers, &state.settings);
    let Ok(mut source_lease) = state.source_uploads.try_acquire(&client_ip, body_bytes) else {
        return Ok(source_busy());
    };
    let received = Arc::new(AtomicU64::new(0));
    let multipart = Multipart::from_request(counting_body(request, Arc::clone(&received)), &())
        .await
        .map_err(|_| invalid(NOT_MULTIPART_MESSAGE))?;
    if declared_length(&headers)
        .is_some_and(|n| n > max_deployment_body_bytes(&state.settings) as u64)
    {
        return Err(body_too_large(&state.settings));
    }

    let request = Request {
        headers: &headers,
        client_ip: &client_ip,
        declared_partial,
        // Undeclared, a spent source was turned away above.
        quota_exhausted: if declared_partial {
            state.deploy_quota.exhausted(&client_ip)
        } else {
            None
        },
        started: request_started,
        received: &received,
    };
    deploy_form(
        &state,
        deployer,
        &request,
        multipart,
        &mut memory,
        &mut disk,
        source_lease.as_mut(),
    )
    .await
}

/// Upstream's 429 for a source past its uploads or bytes in flight.
fn source_busy() -> Response {
    with_retry_after(
        super::errors::json_error(
            StatusCode::TOO_MANY_REQUESTS,
            super::upload_admission::SOURCE_BUSY_MESSAGE,
        ),
        header::HeaderValue::from(super::upload_admission::SOURCE_BUSY_RETRY_AFTER_SECS),
    )
}

/// `request` with its body's bytes added to `received` as they arrive.
fn counting_body(
    request: axum::extract::Request,
    received: Arc<AtomicU64>,
) -> axum::extract::Request {
    request.map(|body| {
        axum::body::Body::from_stream(body.into_data_stream().inspect(move |chunk| {
            if let Ok(chunk) = chunk {
                received.fetch_add(chunk.len() as u64, Ordering::Relaxed);
            }
        }))
    })
}

/// Upstream's 408 for a body that did not arrive within `timeout`, with
/// what did arrive (`uploadTimedOutMessage`).
fn upload_timed_out(timeout: Duration, received: u64, declared: Option<u64>) -> Response {
    let timeout_ms = timeout.as_millis() as f64;
    let received_text = match declared {
        Some(declared) => format!("{received} of {declared}"),
        None => received.to_string(),
    };
    let bytes_per_second = if timeout_ms > 0.0 {
        received as f64 * 1000.0 / timeout_ms
    } else {
        0.0
    };
    super::errors::json_error(
        StatusCode::REQUEST_TIMEOUT,
        &format!(
            "The upload did not finish within {} s: received {received_text} bytes ({} KiB/s). Retry on a faster connection or send smaller batches.",
            js_fixed(timeout_ms / 1000.0, 3),
            js_fixed(bytes_per_second / 1024.0, 2),
        ),
    )
}

/// `String(Number(value.toFixed(digits)))`: the value rounded to `digits`
/// decimals, without trailing zeros.
fn js_fixed(value: f64, digits: usize) -> String {
    let fixed = format!("{value:.digits$}");
    if fixed.contains('.') {
        fixed
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    } else {
        fixed
    }
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
    /// The body's bytes read so far.
    received: &'a AtomicU64,
}

/// An admitted deploy's answer: [`Answer::Undecided`] is the deployer's
/// 503 for this node's own state on a request small enough not to have
/// caused it, which the client's quota gets back.
enum Answer {
    Decided(Response),
    Undecided(Response),
}

/// Reads an admitted form, within the upload timeout, and deploys it.
async fn deploy_form(
    state: &SharedState,
    deployer: std::sync::Arc<super::write_deployer::WriteDeployer>,
    request: &Request<'_>,
    mut multipart: Multipart,
    memory: &mut Reservation<'_>,
    disk: &mut Reservation<'_>,
    source: Option<&mut SourceLease<'_>>,
) -> AppResult<Response> {
    let settings = &state.settings;
    // Removed, with every file in it, when this request ends however it
    // ends, after the deploy stored what it keeps.
    let mut spool = match UploadSpool::create(&state.spool_root, SPOOL_ROUTE, true).await {
        Ok(spool) => spool,
        Err(e) => return Ok(spool_unavailable(&e)),
    };
    let is_partial = |fields: &BTreeMap<String, String>| {
        fields.get("partial").map(String::as_str) == Some("true")
    };
    let read = read_form(
        &mut multipart,
        settings,
        Spooling {
            spool: &mut spool,
            disk,
            source,
        },
        async |fields: &BTreeMap<String, String>| {
            if !request.declared_partial && !is_partial(fields) {
                return None;
            }
            let entity_id = fields.get("entityId")?;
            let chain = auth_chain_of(fields).ok()?;
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
    );
    let timeout = settings.multipart_upload_timeout;
    let Ok(form) = tokio::time::timeout(timeout, read).await else {
        return Ok(upload_timed_out(
            timeout,
            request.received.load(Ordering::Relaxed),
            declared_length(request.headers),
        ));
    };
    let (fields, parts) = match form? {
        Form::Read(fields, parts) => (fields, parts),
        Form::Refused(response) => return Ok(response),
    };
    let (files, entity) = match uploads_of(&fields, parts, memory).await {
        Ok(read) => read,
        Err(refused) => return Ok(refused),
    };
    if is_partial(&fields) {
        let answer = partial_batch(state, &deployer, request, fields, files).await;
        return Ok(answer.unwrap_or_else(IntoResponse::into_response));
    }
    // Otherwise the query flag would carry a regular deploy past the quota.
    if request.declared_partial {
        return Err(invalid(
            "The 'partial=true' query parameter requires the 'partial=true' form field",
        ));
    }
    // Counted once the form is read, as upstream's `withDeploymentQuota`:
    // a form refused by the parser or its limits is not.
    let charge = match state.deploy_quota.try_consume(request.client_ip) {
        Ok(charge) => charge,
        Err(retry_after) => return Ok(quota_exceeded_response(retry_after)),
    };
    let uploads = files.into_iter().map(|(_, upload)| upload).collect();
    let deploy = deploy_regular(state, &deployer, request, fields, uploads, entity);
    let Ok(answer) = tokio::time::timeout(settings.deployment_processing_timeout, deploy).await
    else {
        // Counted: the client may have chosen what stalled (its contract
        // wallet's RPC), and the deploy may yet have committed.
        return Ok(processing_timed_out(settings.deployment_processing_timeout));
    };
    Ok(match answer {
        Ok(Answer::Decided(response)) => response,
        Ok(Answer::Undecided(response)) => {
            if let Some(charge) = charge {
                charge.refund();
            }
            response
        }
        Err(e) => e.into_response(),
    })
}

/// A regular (single-request) deploy of a read form.
async fn deploy_regular(
    state: &SharedState,
    deployer: &super::write_deployer::WriteDeployer,
    request: &Request<'_>,
    fields: BTreeMap<String, String>,
    uploads: Vec<Upload>,
    entity: Option<Bytes>,
) -> AppResult<Answer> {
    let headers = request.headers;
    let request_started = request.started;
    let entity_id = fields
        .get("entityId")
        .ok_or_else(|| invalid(MISSING_ENTITY_ID_MESSAGE))?
        .clone();
    let auth_chain = auth_chain_of(&fields)?;
    let failed = |failure: DeployFailure, refundable: bool| {
        tracing::warn!(entity_id = %entity_id, errors = ?failure.errors(), "POST /entities - Deployment failed");
        let undecided = refundable && matches!(failure, DeployFailure::Unavailable(_));
        let response = deployment_failure_response(failure);
        if undecided {
            Answer::Undecided(response)
        } else {
            Answer::Decided(response)
        }
    };
    // Deploys are idempotent: a replay answers the stored timestamp once it
    // carries the entity file, names nothing denylisted and carries a chain
    // that signs the id at the entity's timestamp, with the errors a fresh
    // deploy gets; the TTL, rate-limit and access checks are not run again.
    // A failed lookup goes on to the deploy, whose write answers a deployed
    // entity the same way.
    match deployer.deployed_timestamp(&entity_id).await {
        Ok(Some(creation_timestamp)) => {
            if let Err(failure) = deployer
                .verify_replay(entity.as_ref(), &uploads, &entity_id, &auth_chain)
                .await
            {
                // Only a 503 is refundable; the check parses the entity file again.
                let refundable = matches!(failure, DeployFailure::Unavailable(_))
                    && is_refundable(entity.as_ref());
                return Ok(failed(failure, refundable));
            }
            tracing::info!(entity_id = %entity_id, "POST /entities - Entity already deployed");
            return Ok(Answer::Decided(created_response(creation_timestamp)));
        }
        Ok(None) => {}
        Err(e) => {
            tracing::warn!(entity_id = %entity_id, error = %e, "POST /entities - replay lookup failed");
        }
    }
    if !state.sync.takes_deploys() {
        let Some(entity_type) = entity_type_of(entity.as_ref()) else {
            return Ok(Answer::Decided(deployment_failure_response(
                DeployFailure::Rejected(
                    crate::write_deployer::RejectionKind::Validation,
                    vec![
                        "The entity file was not part of the uploaded files or has no type."
                            .to_string(),
                    ],
                ),
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
        file_count = uploads.len(),
        "POST /entities - Deploying entity (multipart)"
    );

    let refundable = is_refundable(entity.as_ref());
    let multipart_us = request_started.elapsed().as_micros() as u64;
    let deploy_started = std::time::Instant::now();
    match deployer
        .deploy_entity(uploads, &entity_id, auth_chain, "LOCAL")
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
        Err(failure) => Ok(failed(failure, refundable)),
    }
}

/// A batch of a partial upload. As upstream's `withDeploymentQuota`, the
/// daily quota counts the upload once, when its first batch creates it, and
/// a batch refused with a 400 before joining its upload (a missing
/// `entityId` or a bad chain included), and a replay of a deployed entity
/// that is not answered its timestamp or that asked a contract wallet; its
/// other 429s and 503s, its resumes and its other replays are not counted.
/// A spent source may only resume the signer's own upload or replay a
/// deployed entity not signed by a contract wallet.
async fn partial_batch(
    state: &SharedState,
    deployer: &super::write_deployer::WriteDeployer,
    request: &Request<'_>,
    fields: BTreeMap<String, String>,
    files: Vec<(String, Upload)>,
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
        return Err(invalid(MISSING_ENTITY_ID_MESSAGE));
    };
    let auth_chain = match auth_chain_of(&fields) {
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
    let timeout = state.settings.deployment_processing_timeout;
    let response = tokio::time::timeout(
        timeout,
        partial_deployments::handle(
            deployer,
            &state.settings.partial_uploads,
            batch,
            request.quota_exhausted,
            &mut admission,
        ),
    )
    .await
    .unwrap_or_else(|_| processing_timed_out(timeout));
    if admission.created
        || admission.counted_replay
        || (response.status() == StatusCode::BAD_REQUEST && !admission.admitted)
    {
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

    /// The route's read of a spooled form: the entity file comes onto the
    /// heap, the reservation grows by it and by the thumbnail its validation
    /// reads (never by the other parts), a budget without that room is the
    /// budget's 503 and an entity part the disk cannot give back the
    /// spool's.
    #[tokio::test]
    async fn the_entity_file_and_the_inspected_thumbnail_are_charged_to_memory() {
        let dir =
            std::env::temp_dir().join(format!("catalyst-uploads-of-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let entity = serde_json::to_vec(&json!({
            "version": "v3", "type": "wearable", "pointers": ["urn:x"], "timestamp": 1,
            "content": [{"file": "thumbnail.png", "hash": "bafythumb"}],
            "metadata": {"thumbnail": "thumbnail.png"}
        }))
        .unwrap();
        let part = |name: &str, hash: &str, bytes: &[u8]| {
            std::fs::write(dir.join(name), bytes).unwrap();
            SpooledFile {
                name: name.to_string(),
                hash: hash.to_string(),
                hash_v0: Some(format!("Qm{hash}")),
                len: bytes.len(),
                path: dir.join(name),
            }
        };
        let parts = vec![
            part("bafyentity", "bafyentity", &entity),
            part("bafythumb", "bafythumb", &[7; 3000]),
            part("bafymodel", "bafymodel", &[9; 50_000]),
        ];
        let fields = fields(&[("entityId", "bafyentity")]);
        let min = super::super::upload_admission::MIN_UPLOAD_RESERVATION_BYTES;
        let most = min + entity.len() as u64 + 3000;

        let budget = super::super::upload_admission::UploadBudget::new(most);
        let mut memory = budget.try_reserve(0).unwrap();
        let (uploads, read) = uploads_of(&fields, parts.clone(), &mut memory)
            .await
            .unwrap();
        assert_eq!(read.as_deref(), Some(&entity[..]));
        assert_eq!(memory.bytes(), most, "the model part is not charged");
        assert!(matches!(&uploads[0].1, Upload::Bytes(b) if b[..] == entity[..]));
        assert!(
            uploads[1..]
                .iter()
                .all(|(_, upload)| matches!(upload, Upload::Spooled(_)))
        );
        drop(memory);

        let small = super::super::upload_admission::UploadBudget::new(most - 1);
        let mut memory = small.try_reserve(0).unwrap();
        let recorder = metrics_exporter_prometheus::PrometheusBuilder::new().build_recorder();
        let guard = metrics::set_default_local_recorder(&recorder);
        let refused = uploads_of(&fields, parts.clone(), &mut memory)
            .await
            .unwrap_err();
        drop(guard);
        assert_eq!(refused.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(refused.headers().contains_key(header::RETRY_AFTER));
        let received = entity.len() + 3000 + 50_000;
        assert!(
            recorder.handle().render().contains(&format!(
                "upload_budget_refused_received_bytes_sum{{route=\"content-entities\"}} {received}"
            )),
            "the form's bytes are recorded with the refusal"
        );
        drop(memory);

        let mut gone = parts;
        gone[0].path = dir.join("removed");
        let mut memory = budget.try_reserve(0).unwrap();
        let unreadable = uploads_of(&fields, gone, &mut memory).await.unwrap_err();
        assert_eq!(unreadable.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(unreadable.headers().contains_key(header::RETRY_AFTER));
        let body = axum::body::to_bytes(unreadable.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(
            String::from_utf8_lossy(&body).contains(SPOOL_UNAVAILABLE_MESSAGE),
            "{body:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn limits_past_the_shipped_gateway_cap_are_warned_about() {
        let defaults = Settings::for_tests();
        assert!(max_deployment_body_bytes(&defaults) <= SHIPPED_GATEWAY_DEPLOY_CAP_BYTES);
        assert_eq!(gateway_cap_warning(&defaults), None);
        for raised in [
            Settings {
                max_upload_total_bytes: defaults.max_upload_total_bytes + 64 * 1024 * 1024,
                ..Settings::for_tests()
            },
            Settings {
                max_upload_file_count: defaults.max_upload_file_count * 2,
                ..Settings::for_tests()
            },
        ] {
            let warning = gateway_cap_warning(&raised).expect("past the gateway cap");
            assert!(warning.contains("CONTENT_DEPLOY_CAP"), "{warning}");
        }
    }

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
    fn a_private_peer_forwarding_x_real_ip_looks_like_an_untrusted_proxy() {
        let mut forwarded = HeaderMap::new();
        forwarded.insert("x-real-ip", header::HeaderValue::from_static("203.0.113.9"));
        for peer in [
            "10.1.2.3",
            "172.18.0.4",
            "192.168.1.2",
            "127.0.0.1",
            "100.64.0.1",
            "169.254.1.1",
            "::1",
            "fd00::4",
            "fe80::1",
            "::ffff:172.18.0.4",
        ] {
            let peer = peer.parse().unwrap();
            assert!(looks_like_an_untrusted_proxy(&forwarded, peer), "{peer}");
            assert!(
                !looks_like_an_untrusted_proxy(&HeaderMap::new(), peer),
                "no header: {peer}"
            );
        }
        for peer in ["198.51.100.7", "100.128.0.1", "2001:db8::1"] {
            let peer = peer.parse().unwrap();
            assert!(!looks_like_an_untrusted_proxy(&forwarded, peer), "{peer}");
        }
        let mut pinned = HeaderMap::new();
        pinned.insert("x-real-ip", header::HeaderValue::from_static("172.18.0.4"));
        assert!(
            !looks_like_an_untrusted_proxy(&pinned, "172.18.0.4".parse().unwrap()),
            "the server's own pin"
        );
    }

    #[test]
    fn access_calls_count_parcels_names_and_items() {
        let scene = serde_json::json!({"type": "scene", "pointers": ["0,0", "0,1"]});
        assert_eq!(access_calls_of(&scene), 2);
        let profile = serde_json::json!({
            "type": "profile",
            "pointers": ["0xabc"],
            "metadata": {"avatars": [
                {"hasClaimedName": true, "avatar": {"wearables": ["a", "b"], "emotes": [{"urn": "c"}]}},
                {"hasClaimedName": false, "avatar": {"wearables": []}}
            ]}
        });
        assert_eq!(access_calls_of(&profile), 5);
        let outfits = serde_json::json!({
            "type": "outfits",
            "pointers": ["0xabc:outfits"],
            "metadata": {"outfits": [{"outfit": {"wearables": ["a", "b"]}}], "namesForExtraSlots": ["n"]}
        });
        assert_eq!(access_calls_of(&outfits), 4);
        let wearable =
            serde_json::json!({"type": "wearable", "pointers": ["urn"], "content": [{}, {}]});
        assert_eq!(access_calls_of(&wearable), 3);
    }

    #[test]
    fn checks_on_contracts_the_entity_names_are_never_refunded() {
        let wearing = |urn: &str| {
            serde_json::json!({
                "type": "profile",
                "pointers": ["0xabc"],
                "metadata": {"avatars": [{"avatar": {"wearables": [urn], "emotes": []}}]}
            })
        };
        assert!(!asks_named_contracts(&wearing(
            "urn:decentraland:off-chain:base-avatars:eyebrows_00"
        )));
        assert!(asks_named_contracts(&wearing(
            "urn:decentraland:matic:collections-v2:0x0002000000000000000000000000000000000001:0:1"
        )));
        let emote = serde_json::json!({"type": "emote", "pointers": ["urn"]});
        assert!(asks_named_contracts(&emote));
        let scene = serde_json::json!({"type": "scene", "pointers": ["0,0"]});
        assert!(!asks_named_contracts(&scene));
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

    /// `/authChain\[(\d+)\]/` anywhere in a field name, digits only.
    #[test]
    fn indexed_auth_chain_keys_are_scanned_as_upstream() {
        let mut f = link_fields(1);
        f.insert("authChain[+1][type]".into(), "SIGNER".into());
        let chain = extract_auth_chain_from_fields(&f).unwrap().unwrap();
        assert_eq!(chain.as_array().unwrap().len(), 1);

        let mut f = link_fields(1);
        f.insert("xauthChain[2][payload]".into(), "0x".into());
        assert_eq!(
            extract_auth_chain_from_fields(&f).unwrap_err(),
            "Missing auth chain element at index 1"
        );

        let mut f = link_fields(1);
        f.insert("authChain[x]authChain[1][type]".into(), "SIGNER".into());
        assert_eq!(
            extract_auth_chain_from_fields(&f).unwrap_err(),
            "Missing auth chain element at index 1"
        );

        let mut f = link_fields(1);
        f.insert(
            "authChain[99999999999999999999][type]".into(),
            "SIGNER".into(),
        );
        assert_eq!(
            extract_auth_chain_from_fields(&f).unwrap_err(),
            too_long_message()
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
    fn a_chain_is_taken_as_auth_chain_validate_takes_it() {
        let chain_of = |chain: &str| auth_chain_of(&fields(&[("authChain", chain)]));
        let refused = |chain: &str| chain_of(chain).unwrap_err().to_string();
        for bad in [
            "[]",
            "[null]",
            r#"[{"payload":"0x1"}]"#,
            r#"[{"type":"SIGNER"}]"#,
            r#"[{"type":"SIGNER","payload":1}]"#,
            r#"[{"type":"OTHER","payload":"0x1","signature":""}]"#,
            r#"[{"type":"SIGNER","payload":"0x1","signature":"0xsig"}]"#,
            r#"[{"type":"SIGNER","payload":"0x1"},{"type":"ECDSA_SIGNED_ENTITY","payload":"bafy"}]"#,
            r#"[{"type":"SIGNER","payload":"0x1"},{"type":"ECDSA_SIGNED_ENTITY","payload":"bafy","signature":1}]"#,
        ] {
            assert_eq!(refused(bad), "Invalid auth chain", "{bad}");
        }
        for good in [
            r#"[{"type":"SIGNER","payload":"0x1"}]"#,
            r#"[{"type":"SIGNER","payload":"0x1","signature":null}]"#,
            r#"[{"type":"SIGNER","payload":"0x1","signature":""},{"type":"ECDSA_EPHEMERAL","payload":"p","signature":"0xs"},{"type":"ECDSA_SIGNED_ENTITY","payload":"bafy","signature":null}]"#,
            r#"[{"type":"SIGNER","payload":"0x1","signature":""},{"type":"ECDSA_EIP_1654_SIGNED_ENTITY","payload":"bafy","signature":"0xs","extra":1}]"#,
        ] {
            assert!(chain_of(good).is_ok(), "{good}");
        }
        // The legacy pair is no chain, as upstream's handler takes it.
        for form in [
            fields(&[("ethAddress", "0xabc"), ("signature", "0xsig")]),
            fields(&[("entityId", "bafy")]),
        ] {
            assert_eq!(
                auth_chain_of(&form).unwrap_err().to_string(),
                "No auth chain can be derivated"
            );
        }
    }

    #[test]
    fn an_upload_timeout_names_what_arrived_as_upstream_does() {
        assert_eq!(js_fixed(300.0, 3), "300");
        assert_eq!(js_fixed(0.05, 3), "0.05");
        assert_eq!(js_fixed(1.5, 2), "1.5");
        assert_eq!(js_fixed(0.0, 2), "0");
    }

    #[tokio::test]
    async fn a_failure_keeps_its_kind_and_an_undecidable_one_carries_retry_after() {
        use crate::write_deployer::RejectionKind;
        for kind in [
            RejectionKind::Validation,
            RejectionKind::PointerContention,
            RejectionKind::RateLimited,
            RejectionKind::Superseded,
        ] {
            let rejected =
                deployment_failure_response(DeployFailure::Rejected(kind, vec!["no".into()]));
            assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
            assert!(rejected.headers().get(header::RETRY_AFTER).is_none());
            let body = axum::body::to_bytes(rejected.into_body(), 4096)
                .await
                .unwrap();
            assert_eq!(
                serde_json::from_slice::<Value>(&body).unwrap(),
                json!({ "errors": ["no"] })
            );
        }
        // This node's side only: a source down or unset, the store, a contract-wallet
        // signature it could not ask about.
        for failure in [
            DeployFailure::Unavailable(vec!["down".into()]),
            DeployFailure::Unverifiable(vec!["The signature is invalid. rpc".into()]),
        ] {
            let unavailable = deployment_failure_response(failure);
            assert_eq!(unavailable.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(unavailable.headers()[header::RETRY_AFTER], "30");
        }
    }
}
