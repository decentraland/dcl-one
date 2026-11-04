//! `POST /entities`: a world publish. The multipart form is read under the
//! shared upload budgets, then checked in the old service's order, the
//! first failure answering `{error, message}` with "Deployment failed: …":
//! the auth chain over the entity id (through `crate::auth` with the shared
//! EIP-1654 validator), the entity and scene schemas, the scene type, the
//! file count, the timestamp window, the `.dcl.eth` name, deprecated
//! config, parcel bounds and canonical pointers, the world's parcel cap
//! (`Policy::world_limits`), the DAO deny list, the publish authorized by
//! `worlds::policy::Policy::can_deploy` (the NAME/personal owner replaces
//! every overlapping scene; a deployment grantee exactly the overlapping
//! scenes its grant covers), the minimap, skybox and navmap files, the
//! uploaded files (none extra, none missing) and the world size cap. The
//! files go to the ONE blob store and the scene row lands in
//! `worlds.world_scenes` in one transaction (`store::deploy_scene`).
//!
//! Why not the content write path's statement: `content.deployments` /
//! `content.active_pointers` key active scenes by parcel, which is Genesis
//! City's authority; every world has its own `0,0`, so world scenes keep
//! the `world_scenes` relation (stage 4) and share the write path's
//! ingredients (hashing, chain verification, the blob store), not its rows.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::{Multipart, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::Value;

use crate::auth::AuthChain;
use crate::comms::world_identity::{DCL_ETH_SUFFIX, ParcelSet, WorldName};
use crate::policy::Deploy;
use crate::shared::identity::{Address, AuthorityNotEstablished};

use super::deploy_form::{self, DeployForm};
use super::deploy_validate::{
    MAX_ENTITY_FILE_SIZE_BYTES, canon_pointer_set, entity_file_too_large_error,
    entity_schema_error, extract_auth_chain_from_fields, is_canonical_parcel_set,
    missing_minimap_images, missing_skybox_textures, validate_navmap_thumbnail,
    validate_parcel_in_bounds,
};
use super::metrics as worlds_metrics;
use super::store::{self, SceneDeployment};
use super::upload_limits::DeployRejection;
use super::upload_spool::{self, SpooledFile};
use super::{SharedState, upload_limits};

/// Payload cap for a deployment: every file chunk and field value, multipart framing excluded.
pub const MAX_UPLOAD_SIZE_BYTES: usize = 350 * 1024 * 1024;

/// Wire-size cap for the whole multipart body; enforced on the Content-Length precheck and as this route's axum body limit.
pub const MAX_UPLOAD_WIRE_SIZE_BYTES: usize = MAX_UPLOAD_SIZE_BYTES + 10 * 1024 * 1024;

const _: () = assert!(MAX_UPLOAD_WIRE_SIZE_BYTES >= MAX_UPLOAD_SIZE_BYTES);

pub(super) const ENTITY_TTL_MS: i64 = 300_000;

/// Reject a deployment dated more than this far in the future: a garbage or clock-skewed
/// timestamp must not let an entity stay deployable/replayable indefinitely. Kept aligned
/// with the Catalyst request TTL forwards guard.
pub(super) const MAX_DEPLOYMENT_FUTURE_SKEW_MS: i64 = 15 * 60 * 1000;

/// `MAX_FILE_COUNT`'s default.
pub const DEFAULT_MAX_FILE_COUNT: usize = 10_000;

pub(super) const TOO_MANY_FILES_MESSAGE: &str = "The multipart request has too many files.";
pub(super) const FILE_TOO_LARGE_MESSAGE: &str =
    "An uploaded file exceeds the maximum allowed size.";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct DeploySuccess {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub creation_timestamp: i64,
    pub message: String,
}

fn declared_length_exceeds_limit(declared_len: u64) -> bool {
    declared_len > MAX_UPLOAD_WIRE_SIZE_BYTES as u64
}

/// The old service's `{error, message}` answer of a refused deploy.
fn rejection(status: StatusCode, message: impl Into<String>) -> Response {
    let error = match status {
        StatusCode::BAD_REQUEST => "Bad request",
        other => other.canonical_reason().unwrap_or("Error"),
    };
    (
        status,
        Json(DeployRejection {
            error: error.to_string(),
            message: message.into(),
        }),
    )
        .into_response()
}

/// Size and count limits of the multipart body answer 413, as the old service.
fn is_upload_limit(message: &str) -> bool {
    [
        upload_limits::PAYLOAD_TOO_LARGE_MESSAGE,
        upload_limits::TOO_MANY_PARTS_MESSAGE,
        TOO_MANY_FILES_MESSAGE,
        FILE_TOO_LARGE_MESSAGE,
    ]
    .contains(&message)
        || message.starts_with("The multipart request has too many fields")
}

/// A request refused before validation: 400, or 413 for a body limit.
pub(super) fn err_one(message: impl Into<String>) -> Response {
    let message = message.into();
    let status = if is_upload_limit(&message) {
        StatusCode::PAYLOAD_TOO_LARGE
    } else {
        StatusCode::BAD_REQUEST
    };
    rejection(status, message)
}

/// A refused publish: its status and message.
struct Refusal(StatusCode, String);

impl IntoResponse for Refusal {
    fn into_response(self) -> Response {
        rejection(self.0, self.1)
    }
}

/// A failed validation, worded as the old service.
fn failed(message: impl std::fmt::Display) -> Refusal {
    Refusal(
        StatusCode::BAD_REQUEST,
        format!("Deployment failed: {message}"),
    )
}

fn forbidden(message: impl Into<String>) -> Refusal {
    Refusal(StatusCode::FORBIDDEN, message.into())
}

fn internal(message: impl Into<String>) -> Refusal {
    Refusal(StatusCode::INTERNAL_SERVER_ERROR, message.into())
}

fn unavailable(message: impl Into<String>) -> Refusal {
    Refusal(StatusCode::SERVICE_UNAVAILABLE, message.into())
}

pub async fn deploy_entity(
    State(state): State<SharedState>,
    headers: HeaderMap,
    multipart: Multipart,
) -> Response {
    let limits = &state.settings.upload_limits;
    let declared: Option<u64> = match upload_limits::declared_content_length(&headers) {
        upload_limits::DeclaredContentLength::Invalid => {
            return err_one(upload_limits::INVALID_CONTENT_LENGTH_MESSAGE);
        }
        upload_limits::DeclaredContentLength::Absent => None,
        upload_limits::DeclaredContentLength::Bytes(n) => Some(n),
    };
    if let Some(len) = declared
        && declared_length_exceeds_limit(len)
    {
        return err_one(upload_limits::PAYLOAD_TOO_LARGE_MESSAGE);
    }

    let _source = match upload_limits::admit_source(
        &headers,
        limits,
        worlds_metrics::ENTITIES_ROUTE,
        MAX_UPLOAD_SIZE_BYTES as u64,
    ) {
        Ok(lease) => lease,
        Err(_) => return upload_limits::source_limit_response(),
    };

    let Some(_slot) = upload_limits::try_acquire_upload_slot(limits.max_concurrent_uploads) else {
        worlds_metrics::upload_rejected(worlds_metrics::ENTITIES_ROUTE, "concurrency");
        tracing::warn!(
            active = upload_limits::active_uploads(),
            max = limits.max_concurrent_uploads,
            "POST /entities shed: concurrent-upload cap exceeded"
        );
        return upload_limits::shed_response(upload_limits::CONCURRENCY_SHED_MESSAGE);
    };

    let mut bytes_lease = upload_limits::reserve_in_flight();
    let mut files_lease = upload_limits::reserve_in_flight_files();
    let spool_root = upload_spool::spool_root(&state.storage);
    let receiving = Instant::now();
    let received = |outcome: &'static str, bytes: u64| {
        worlds_metrics::upload_received(
            worlds_metrics::ENTITIES_ROUTE,
            declared.is_some(),
            outcome,
            receiving.elapsed(),
            bytes,
        )
    };

    let form = match tokio::time::timeout(
        Duration::from_millis(limits.multipart_upload_timeout_ms),
        deploy_form::read_deploy_form(
            multipart,
            &mut bytes_lease,
            &mut files_lease,
            limits.max_in_flight_upload_bytes,
            limits.max_in_flight_upload_files,
            &spool_root,
        ),
    )
    .await
    {
        Ok(Ok(form)) => {
            received("completed", bytes_lease.reserved());
            form
        }
        Ok(Err(resp)) => {
            received("rejected", bytes_lease.reserved());
            return *resp;
        }
        Err(_) => {
            received("timeout", bytes_lease.reserved());
            worlds_metrics::upload_rejected(worlds_metrics::ENTITIES_ROUTE, "timeout");
            tracing::warn!(
                timeout_ms = limits.multipart_upload_timeout_ms,
                "POST /entities: multipart upload timed out"
            );
            return upload_limits::timeout_response(upload_limits::MULTIPART_TIMEOUT_MESSAGE);
        }
    };

    let timeout_ms = limits.deployment_processing_timeout_ms;
    let processing = Instant::now();
    let resp = match tokio::time::timeout(
        Duration::from_millis(timeout_ms),
        deploy_entity_inner(&state, &headers, form),
    )
    .await
    {
        Ok(resp) => resp,
        Err(_) => {
            tracing::warn!(
                timeout_ms,
                "POST /entities: deployment processing timed out"
            );
            upload_limits::timeout_response(&format!(
                "Deployment processing exceeded the {timeout_ms}ms deadline."
            ))
        }
    };
    worlds_metrics::deployment_processed(resp.status(), processing.elapsed());
    resp
}

fn present_truthy(v: &Value, key: &str) -> bool {
    match v.get(key) {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => true,
    }
}

/// JavaScript's `Number` text of `ms / 1000`.
fn secs(ms: i64) -> String {
    (ms as f64 / 1000.0).to_string()
}

/// The deployment window, both directions. A timestamp dated forward is refused for the
/// same reason a stale one is: the signature covers the entity id, so a deployment the
/// clock has not reached yet would stay replayable until it does.
fn entity_timestamp_error(entity: &Value, now_ms: i64) -> Option<String> {
    let Some(ts) = entity.get("timestamp").and_then(|v| v.as_i64()) else {
        return Some("The entity is missing a valid timestamp".to_string());
    };
    let ttl = now_ms.saturating_sub(ts);
    if ttl < -MAX_DEPLOYMENT_FUTURE_SKEW_MS {
        return Some(format!(
            "Deployment was created {} secs in the future. Max allowed: {} secs.",
            secs(ttl.saturating_abs()),
            secs(MAX_DEPLOYMENT_FUTURE_SKEW_MS)
        ));
    }
    if ttl > ENTITY_TTL_MS {
        return Some(format!(
            "Deployment was created {} secs ago. Max allowed: {} secs.",
            secs(ttl),
            secs(ENTITY_TTL_MS)
        ));
    }
    None
}

/// `encodeURIComponent`.
fn encode_uri_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The old `buildSceneDeploymentMessage`: the parcels and the play link.
fn deployment_message(base_url: &str, world_name: &str, parcels: &[String]) -> String {
    let world_url = format!("{base_url}/world/{world_name}");
    let position = parcels.first().map_or("0,0", String::as_str);
    format!(
        "Your scene was deployed to World \"{world_name}\" at parcels: {}!\nAccess world: https://play.decentraland.org/?realm={}&position={}",
        parcels.join(", "),
        encode_uri_component(&world_url),
        encode_uri_component(position)
    )
}

fn raw_strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect()
}

/// The first-seen distinct values, as a JavaScript `Set` keeps them.
fn distinct(values: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .iter()
        .filter(|v| seen.insert(v.as_str()))
        .cloned()
        .collect()
}

/// `createValidateScenePointers`: `scene.parcels` holds its base, and both
/// it and the pointers are the same unique canonical parcel set.
fn scene_pointers_error(entity: &Value) -> Option<String> {
    const NOT_CANONICAL: &str =
        "Scene pointers and parcels must be unique canonical parcel coordinates.";
    let scene = entity.pointer("/metadata/scene");
    let base = scene.and_then(|s| s.get("base"));
    let parcels = scene
        .and_then(|s| s.get("parcels"))
        .and_then(Value::as_array);
    if let (Some(base), Some(parcels)) = (base, parcels)
        && !parcels.contains(base)
    {
        let listed: Vec<String> = parcels
            .iter()
            .map(|p| p.as_str().map_or_else(|| p.to_string(), str::to_string))
            .collect();
        return Some(format!(
            "The scene base parcel [{}] must be included in the scene parcels [{}].",
            base.as_str()
                .map_or_else(|| base.to_string(), str::to_string),
            listed.join(", ")
        ));
    }
    let canonical = |v: Option<&Vec<Value>>| v.is_some_and(|a| is_canonical_parcel_set(a));
    let base_ok = base
        .and_then(Value::as_str)
        .is_some_and(|b| is_canonical_parcel_set(&[Value::String(b.to_string())]));
    let pointers = entity.get("pointers").and_then(Value::as_array);
    if !base_ok || !canonical(parcels) || !canonical(pointers) {
        return Some(NOT_CANONICAL.to_string());
    }
    let pointers = distinct(&raw_strings(entity.get("pointers")));
    let parcels = distinct(&raw_strings(scene.and_then(|s| s.get("parcels"))));
    let same = pointers.len() == parcels.len() && pointers.iter().all(|p| parcels.contains(p));
    (!same).then(|| {
        format!(
            "The scene pointers [{}] must match the scene parcels [{}].",
            pointers.join(", "),
            parcels.join(", ")
        )
    })
}

/// The deprecated `worldConfiguration` keys, first one found.
fn deprecated_config_error(entity: &Value) -> Option<&'static str> {
    let wc = entity.pointer("/metadata/worldConfiguration")?;
    if present_truthy(wc, "dclName") {
        return Some(
            "`dclName` in scene.json was renamed to `name`. Please update your scene.json accordingly.",
        );
    }
    if present_truthy(wc, "minimapVisible") {
        return Some(
            "`minimapVisible` in scene.json is deprecated in favor of `{ miniMapConfig: { visible } }`. Please update your scene.json accordingly.",
        );
    }
    if present_truthy(wc, "skybox") {
        return Some(
            "`skybox` in scene.json is deprecated in favor of `{ \"skyboxConfig\": { \"fixedTime\": 36000 }}`. Please update your scene.json accordingly.",
        );
    }
    None
}

/// The uploaded files, each once in upload order, keyed by their CIDv1.
struct Uploads {
    order: Vec<String>,
    by_hash: HashMap<String, SpooledFile>,
}

impl Uploads {
    fn new(files: &[SpooledFile]) -> Self {
        let mut order = Vec::new();
        let mut by_hash = HashMap::new();
        for blob in files {
            if let std::collections::hash_map::Entry::Vacant(slot) =
                by_hash.entry(blob.hash.clone())
            {
                order.push(slot.key().clone());
                slot.insert(blob.clone());
            }
        }
        Self { order, by_hash }
    }
}

/// The signer of a chain over `entity_id`, or the refusal.
async fn authenticate(
    state: &SharedState,
    entity_id: &str,
    chain: &Value,
    now_ms: i64,
) -> Result<Address, Refusal> {
    let chain: AuthChain = serde_json::from_value(chain.clone())
        .map_err(|e| failed(format!("The auth chain is malformed: {e}")))?;
    let signer = chain
        .first()
        .ok_or_else(|| failed("The auth chain is empty"))?;
    let signer = Address::parse(&signer.payload)
        .map_err(|_| failed(format!("Invalid signer: {}", signer.payload)))?;
    match crate::auth::verify_auth_chain_async(
        &chain,
        entity_id,
        Some(now_ms),
        state.eip1654.as_deref(),
    )
    .await
    {
        Ok(()) => Ok(signer),
        Err(crate::auth::AuthError::Eip1654ValidationFailed(e)) => Err(unavailable(format!(
            "The auth chain could not be verified right now: {e}"
        ))),
        Err(e) => Err(failed(format!("The auth chain is invalid: {e}"))),
    }
}

/// What a validated publish carries to the store.
struct Validated {
    world: WorldName,
    world_name: String,
    parcels: ParcelSet,
    replacement: Deploy,
    size: i64,
}

async fn validate(
    state: &SharedState,
    entity: &Value,
    signer: &Address,
    entity_id: &str,
    uploads: &Uploads,
    now_ms: i64,
) -> Result<Validated, Refusal> {
    if let Some(e) = entity_schema_error(entity) {
        return Err(failed(e));
    }
    let entity_type = entity["type"].as_str().unwrap_or_default();
    if entity_type != "scene" {
        return Err(failed(format!(
            "Entity type {entity_type} is not supported."
        )));
    }
    let content = entity["content"].as_array().map_or(&[][..], Vec::as_slice);
    let max_files = state.settings.max_file_count;
    if content.len() > max_files {
        return Err(failed(format!(
            "The deployment has too many files. The maximum allowed is {max_files} but the deployment has {}.",
            content.len()
        )));
    }
    if let Some(e) = entity_timestamp_error(entity, now_ms) {
        return Err(failed(e));
    }

    let metadata = entity.get("metadata").unwrap_or(&Value::Null);
    crate::content::validator::validate_scene_metadata(metadata).map_err(failed)?;
    let world_name = metadata
        .pointer("/worldConfiguration/name")
        .and_then(Value::as_str)
        .filter(|n| !n.is_empty())
        .ok_or_else(|| {
            failed("scene.json needs to specify a worldConfiguration section with a valid name inside.")
        })?;
    let world = match WorldName::parse(world_name) {
        Ok(parsed) if parsed.key().ends_with(DCL_ETH_SUFFIX) => parsed,
        Ok(parsed) if parsed.is_ens_world() && state.policy.ens_enabled() => parsed,
        _ if state.policy.ens_enabled() => {
            return Err(failed(format!(
                "Only .dcl.eth and .eth world names are supported for publishing (got \"{world_name}\")"
            )));
        }
        _ => {
            return Err(failed(format!(
                "Only .dcl.eth world names are supported for publishing (got \"{world_name}\")"
            )));
        }
    };
    if let Some(e) = deprecated_config_error(entity) {
        return Err(failed(e));
    }

    let pointers = canon_pointer_set(entity["pointers"].as_array().map_or(&[][..], Vec::as_slice));
    let bounds: Vec<String> = pointers
        .iter()
        .filter_map(|p| validate_parcel_in_bounds(p).err())
        .collect();
    if !bounds.is_empty() {
        return Err(failed(bounds.join(", ")));
    }
    if let Some(e) = scene_pointers_error(entity) {
        return Err(failed(e));
    }
    let parcels = ParcelSet::parse(&pointers).map_err(|e| failed(e.to_string()))?;
    let limits = state.policy.world_limits(&world).await.map_err(|e| {
        tracing::error!(error = %e, world = %world, "deploy failed: world limits");
        internal("Failed to read the world limits")
    })?;
    if pointers.len() > limits.max_parcels as usize {
        return Err(failed(format!(
            "Max allowed scene dimensions is {} parcels.",
            limits.max_parcels
        )));
    }

    if !state
        .names
        .check_name_deny_list(&state.pool, world_name)
        .await
    {
        return Err(failed(format!(
            "Deployment failed: World \"{world_name}\" can not be deployed because the name is in the name deny list managed by Decentraland DAO."
        )));
    }
    let replacement = match state.policy.can_deploy(&world, signer, &parcels).await {
        Ok(decision) => decision,
        Err(AuthorityNotEstablished::RefusedLacksAuthority { detail }) => {
            tracing::info!(world = %world, signer = %signer.as_str(), "deploy denied: {detail}");
            // A realm-level refusal (personal worlds, no name authority) keeps its own reason.
            return Err(if detail.starts_with("Not authorized:") {
                failed(detail)
            } else {
                failed(format!(
                    "Deployment failed: Your wallet has no permission to publish this scene because it does not have permission to deploy under \"{world_name}\". Check scene.json to select a name that either you own or you were given permission to deploy."
                ))
            });
        }
        Err(undetermined) => {
            tracing::warn!(world = %world, signer = %signer.as_str(), error = %undetermined, "deploy denied: authority undetermined (fail-closed)");
            return Err(forbidden(
                "Not authorized: could not verify deployment permissions (deploy denied)",
            ));
        }
    };
    match store::is_world_disabled(&state.pool, &world).await {
        Ok(false) => {}
        Ok(true) => return Err(forbidden(store::world_disabled(&world).public_message())),
        Err(e) => {
            tracing::error!(error = ?e, world = %world, "deploy failed: disabled check");
            return Err(internal("Failed to persist deployment"));
        }
    }

    let minimap = missing_minimap_images(entity);
    if !minimap.is_empty() {
        return Err(failed(minimap.join(", ")));
    }
    let skybox = missing_skybox_textures(entity);
    if !skybox.is_empty() {
        return Err(failed(skybox.join(", ")));
    }
    let mut thumbnail = Vec::new();
    validate_navmap_thumbnail(entity, &mut thumbnail);
    if let Some(e) = thumbnail.into_iter().next() {
        return Err(failed(e));
    }

    let referenced: HashSet<&str> = content
        .iter()
        .filter_map(|item| item["hash"].as_str())
        .collect();
    let mut file_errors: Vec<String> = uploads
        .order
        .iter()
        .filter(|hash| hash.as_str() != entity_id && !referenced.contains(hash.as_str()))
        .map(|hash| format!("Extra file detected {hash}"))
        .collect();
    let mut sizes: HashMap<&str, i64> = HashMap::new();
    for item in content {
        let (file, hash) = (
            item["file"].as_str().unwrap_or_default(),
            item["hash"].as_str().unwrap_or_default(),
        );
        if sizes.contains_key(hash) {
            continue;
        }
        let size = match uploads.by_hash.get(hash) {
            Some(blob) => Some(blob.len() as i64),
            None => match state.storage.file_info(hash).await {
                Ok(info) => info.map(|i| i.size as i64),
                Err(e) => {
                    tracing::warn!(hash, error = %e, "deploy: could not stat an already-stored file");
                    None
                }
            },
        };
        match size {
            Some(size) => {
                sizes.insert(hash, size);
            }
            None => file_errors.push(format!(
                "The file {hash} ({file}) is neither present in the storage or in the provided entity"
            )),
        }
    }
    if !file_errors.is_empty() {
        return Err(failed(file_errors.join(", ")));
    }
    let size: i64 = sizes.values().copied().fold(0, i64::saturating_add);
    if size > limits.max_size_bytes {
        let max = limits.max_size_bytes;
        return Err(failed(format!(
            "The deployment is too big. The maximum total size allowed is {max} bytes for scenes. You can upload up to {max} bytes but you tried to upload {size}."
        )));
    }

    Ok(Validated {
        world,
        world_name: world_name.to_string(),
        parcels,
        replacement,
        size,
    })
}

async fn deploy_entity_inner(
    state: &SharedState,
    headers: &HeaderMap,
    form: DeployForm,
) -> Response {
    // `_spool` keeps the spooled parts on disk until this deploy has stored them.
    let DeployForm {
        fields,
        files,
        spool: _spool,
    } = form;

    let entity_id = match fields.get("entityId") {
        Some(id) if !id.is_empty() => id.clone(),
        _ => return err_one("Missing entityId field"),
    };

    let auth_chain_value = match extract_auth_chain_from_fields(&fields) {
        Ok(v) => v,
        Err(e) => return err_one(e),
    };

    let now_ms = chrono::Utc::now().timestamp_millis();
    // The signer is established before the entity is read, as the old handler did.
    let signer = match authenticate(state, &entity_id, &auth_chain_value, now_ms).await {
        Ok(signer) => signer,
        Err(refusal) => return refusal.into_response(),
    };

    let uploads = Uploads::new(&files);
    let Some(entity_file) = uploads.by_hash.get(&entity_id) else {
        return err_one(format!(
            "Entity file \"{entity_id}\" is missing from the request."
        ));
    };
    if entity_file.len() > MAX_ENTITY_FILE_SIZE_BYTES {
        return err_one(entity_file_too_large_error());
    }
    let entity_bytes = match entity_file.read().await {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::error!(error = %e, "deploy failed: could not read the spooled entity file");
            return internal("Failed to read the uploaded entity file").into_response();
        }
    };
    let entity: Value = match serde_json::from_slice(&entity_bytes) {
        Ok(v) => v,
        Err(_) => return err_one("The entity file is not valid JSON."),
    };

    let Validated {
        world,
        world_name,
        parcels,
        replacement,
        size,
    } = match validate(state, &entity, &signer, &entity_id, &uploads, now_ms).await {
        Ok(v) => v,
        Err(refusal) => return refusal.into_response(),
    };
    // Every publish refreshes worlds.owner to the live NAME holder.
    let name_owner = match &replacement {
        Deploy::Owner => signer.clone(),
        Deploy::Scoped(_) => match state.policy.live_name_owner(&world).await {
            Ok(Some(owner)) => owner,
            Ok(None) => {
                return err_one(format!(
                    "Cannot deploy scene \"{entity_id}\" to world \"{}\": owner address could not be resolved.",
                    world.display()
                ));
            }
            Err(e) => {
                tracing::warn!(world = %world, error = %e, "deploy denied: NAME owner undetermined");
                return unavailable(
                    "Deployment failed: the world's owner could not be resolved right now",
                )
                .into_response();
            }
        },
    };
    let name_owner = Some(&name_owner);

    // Extra files are refused in validation, so every upload is referenced.
    for (hash, blob) in &uploads.by_hash {
        if let Err(e) = state.storage.store_file(hash, &blob.path).await {
            tracing::error!(error = %e, hash = %hash, "deploy failed: could not store blob");
            return internal("Failed to persist deployment content").into_response();
        }
    }

    let persisted = store::deploy_scene(
        &state.pool,
        &state.storage,
        SceneDeployment {
            world: &world,
            name_owner,
            entity_id: &entity_id,
            deployer: &signer,
            deployment_auth_chain: &auth_chain_value,
            entity: &entity,
            parcels: &parcels,
            size,
            replacement: &replacement,
            forward_to: state.forwarder.as_deref().map(|f| f.target()),
        },
    )
    .await;
    let replaced_owner = persisted.as_ref().ok().cloned().flatten();
    if let Err(e) = persisted {
        let reason = e.public_message();
        if e.is_conflict() {
            tracing::info!(world = %world, entity_id = %entity_id, "deploy conflict: overlapping-scene set changed under a scoped authorization");
            return rejection(StatusCode::CONFLICT, reason);
        }
        if e.is_forbidden() || e.is_bad_request() {
            tracing::info!(world = %world, entity_id = %entity_id, reason = %reason, "deploy refused");
            return if e.is_forbidden() {
                forbidden(reason).into_response()
            } else {
                err_one(reason)
            };
        }
        tracing::error!(error = ?e, world = %world, entity_id = %entity_id, "deploy failed: DB tx error");
        return internal("Failed to persist deployment").into_response();
    }
    // A new holder's publish moved worlds.owner: re-decide the previous one now.
    if let (Some(previous), Some(owner)) = (replaced_owner, name_owner) {
        state.policy.note_owner(&world, Some(owner));
        state.revoke_unless_admitted(&world, &previous).await;
    }

    tracing::info!(
        entity_id = %entity_id,
        signer = %signer.as_str(),
        world = %world,
        authz = match replacement { Deploy::Owner => "name-ownership", Deploy::Scoped(_) => "acl" },
        file_count = files.len(),
        content_size = size,
        user_agent = headers.get("user-agent").and_then(|v| v.to_str().ok()).unwrap_or("unknown"),
        forwarded = state.forwarder.is_some(),
        "POST /entities - deployed (validated + authorized + persisted)"
    );
    worlds_metrics::world_deployed(world.key());
    if let Some(forwarder) = &state.forwarder {
        forwarder.kick();
    }

    let scene_parcels = raw_strings(entity.pointer("/metadata/scene/parcels"));
    (
        StatusCode::OK,
        Json(DeploySuccess {
            creation_timestamp: now_ms,
            message: deployment_message(&state.settings.public_url, &world_name, &scene_parcels),
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const NOW_MS: i64 = 1_700_000_000_000;

    #[test]
    fn upload_precheck_compares_against_the_wire_cap() {
        assert_eq!(MAX_UPLOAD_WIRE_SIZE_BYTES, 360 * 1024 * 1024);
        assert!(!declared_length_exceeds_limit(
            MAX_UPLOAD_SIZE_BYTES as u64 + 1
        ));
        assert!(!declared_length_exceeds_limit(
            MAX_UPLOAD_WIRE_SIZE_BYTES as u64
        ));
        assert!(declared_length_exceeds_limit(
            MAX_UPLOAD_WIRE_SIZE_BYTES as u64 + 1
        ));
        assert!(declared_length_exceeds_limit(u64::MAX));
        assert!(!declared_length_exceeds_limit(0));
        assert!(!declared_length_exceeds_limit(1024));
    }

    #[test]
    fn a_timestamp_inside_the_window_is_accepted_in_both_directions() {
        for ts in [
            NOW_MS,
            NOW_MS - 299_000,
            NOW_MS + MAX_DEPLOYMENT_FUTURE_SKEW_MS,
        ] {
            assert_eq!(
                entity_timestamp_error(&json!({ "timestamp": ts }), NOW_MS),
                None,
                "{ts} is inside the deployment window"
            );
        }
    }

    #[test]
    fn a_timestamp_past_the_forwards_bound_is_refused_in_the_old_words() {
        let error = entity_timestamp_error(
            &json!({ "timestamp": NOW_MS + MAX_DEPLOYMENT_FUTURE_SKEW_MS + 1 }),
            NOW_MS,
        )
        .expect("a deployment the clock has not reached must not be accepted");
        assert_eq!(
            error,
            "Deployment was created 900.001 secs in the future. Max allowed: 900 secs."
        );
    }

    #[test]
    fn a_stale_timestamp_is_refused_in_the_old_words() {
        let error = entity_timestamp_error(&json!({ "timestamp": NOW_MS - 301_000 }), NOW_MS)
            .expect("a deployment older than the TTL must not be accepted");
        assert_eq!(
            error,
            "Deployment was created 301 secs ago. Max allowed: 300 secs."
        );
    }

    #[test]
    fn a_missing_or_unreadable_timestamp_is_refused() {
        for entity in [json!({}), json!({ "timestamp": "1700000000000" })] {
            assert_eq!(
                entity_timestamp_error(&entity, NOW_MS).as_deref(),
                Some("The entity is missing a valid timestamp")
            );
        }
    }

    #[test]
    fn the_success_message_is_the_old_services() {
        assert_eq!(
            deployment_message(
                "https://peer.example",
                "Boedo.dcl.eth",
                &["1,-2".to_string(), "1,-1".to_string()]
            ),
            "Your scene was deployed to World \"Boedo.dcl.eth\" at parcels: 1,-2, 1,-1!\n\
             Access world: https://play.decentraland.org/?realm=https%3A%2F%2Fpeer.example%2Fworld%2FBoedo.dcl.eth&position=1%2C-2"
        );
        assert!(deployment_message("https://p", "a.dcl.eth", &[]).ends_with("&position=0%2C0"));
    }

    async fn body(resp: Response) -> (u16, Value) {
        let status = resp.status().as_u16();
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[tokio::test]
    async fn refusals_answer_the_old_envelope_and_body_limits_413() {
        assert_eq!(
            body(failed("x").into_response()).await,
            (
                400,
                json!({ "error": "Bad request", "message": "Deployment failed: x" })
            )
        );
        for limit in [
            upload_limits::PAYLOAD_TOO_LARGE_MESSAGE,
            upload_limits::TOO_MANY_PARTS_MESSAGE,
            upload_limits::TOO_MANY_FIELDS_MESSAGE,
            TOO_MANY_FILES_MESSAGE,
            FILE_TOO_LARGE_MESSAGE,
        ] {
            assert_eq!(
                body(err_one(limit)).await,
                (
                    413,
                    json!({ "error": "Payload Too Large", "message": limit })
                )
            );
        }
        assert_eq!(body(err_one("Missing entityId field")).await.0, 400);
        assert_eq!(
            body(rejection(StatusCode::CONFLICT, "c")).await.1["error"],
            "Conflict"
        );
    }

    fn scene(pointers: Value, base: &str, parcels: Value) -> Value {
        json!({ "pointers": pointers, "metadata": { "scene": { "base": base, "parcels": parcels } } })
    }

    #[test]
    fn scene_pointers_are_checked_in_the_old_words() {
        assert_eq!(
            scene_pointers_error(&scene(json!(["0,0"]), "0,0", json!(["0,0"]))),
            None
        );
        assert_eq!(
            scene_pointers_error(&scene(json!(["0,0"]), "1,1", json!(["0,0"]))).as_deref(),
            Some("The scene base parcel [1,1] must be included in the scene parcels [0,0].")
        );
        assert_eq!(
            scene_pointers_error(&scene(json!(["00,0"]), "0,0", json!(["0,0"]))).as_deref(),
            Some("Scene pointers and parcels must be unique canonical parcel coordinates.")
        );
        assert_eq!(
            scene_pointers_error(&scene(json!(["0,1"]), "0,0", json!(["0,0"]))).as_deref(),
            Some("The scene pointers [0,1] must match the scene parcels [0,0].")
        );
    }

    #[test]
    fn deprecated_config_reports_the_first_key() {
        let entity = json!({ "metadata": { "worldConfiguration": { "minimapVisible": true, "skybox": 1 } } });
        assert!(
            deprecated_config_error(&entity)
                .unwrap()
                .starts_with("`minimapVisible`")
        );
        assert_eq!(deprecated_config_error(&json!({ "metadata": {} })), None);
    }

    #[test]
    fn uploads_keep_the_first_copy_in_upload_order() {
        let spooled = |hash: &str| SpooledFile {
            hash: hash.to_string(),
            len: 1,
            path: std::path::PathBuf::from(hash),
        };
        let (a, b) = (spooled("a"), spooled("b"));
        let uploads = Uploads::new(&[b.clone(), a.clone(), b.clone()]);
        assert_eq!(uploads.order, vec!["b".to_string(), "a".to_string()]);
        assert_eq!(uploads.by_hash.len(), 2);
    }
}
