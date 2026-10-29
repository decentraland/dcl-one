//! `POST /entities`: a world publish. The multipart form is read under the
//! shared upload budgets, the entity validated the way the old service did
//! (scene type, timestamp window, `.dcl.eth` name, canonical parcels and
//! bounds, uploaded files, world size cap, navmap thumbnail, the auth chain
//! over the entity id through `crate::auth` with the shared EIP-1654
//! validator), the name checked against the DAO deny list, and the publish
//! authorized by `worlds::policy::Policy::can_deploy` (the NAME/personal
//! owner replaces every overlapping scene; a deployment grantee exactly the
//! overlapping scenes its grant covers). The files go to the ONE blob store
//! and the scene row lands in `worlds.world_scenes` in one transaction
//! (`store::deploy_scene`).
//!
//! Why not the content write path's statement: `content.deployments` /
//! `content.active_pointers` key active scenes by parcel, which is Genesis
//! City's authority; every world has its own `0,0`, so world scenes keep
//! the `world_scenes` relation (stage 4) and share the write path's
//! ingredients (hashing, chain verification, the blob store), not its rows.

use std::collections::HashMap;
use std::time::Duration;

use axum::Json;
use axum::extract::{Multipart, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use serde::Serialize;
use serde_json::Value;

use crate::auth::{AuthChain, hash_bytes_v1, is_canonical_cid};
use crate::comms::world_identity::{DCL_ETH_SUFFIX, ParcelSet, WorldName};
use crate::policy::Deploy;
use crate::shared::identity::{Address, AuthorityNotEstablished};

use super::deploy_form::{self, DeployForm};
use super::deploy_validate::{
    MAX_ENTITY_FILE_SIZE_BYTES, canon_pointer_set, entity_file_too_large_error,
    extract_auth_chain_from_fields, is_canonical_parcel_set, validate_navmap_thumbnail,
    validate_parcel_in_bounds,
};
use super::store::{self, SceneDeployment};
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct DeploySuccess {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub creation_timestamp: i64,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct DeployErrors {
    pub errors: Vec<String>,
}

fn declared_length_exceeds_limit(declared_len: u64) -> bool {
    declared_len > MAX_UPLOAD_WIRE_SIZE_BYTES as u64
}

fn err_response(status: StatusCode, messages: Vec<String>) -> Response {
    (status, Json(DeployErrors { errors: messages })).into_response()
}

pub(super) fn err_one(message: impl Into<String>) -> Response {
    err_response(StatusCode::BAD_REQUEST, vec![message.into()])
}

fn forbidden(message: impl Into<String>) -> Response {
    err_response(StatusCode::FORBIDDEN, vec![message.into()])
}

fn internal(message: impl Into<String>) -> Response {
    err_response(StatusCode::INTERNAL_SERVER_ERROR, vec![message.into()])
}

fn unavailable(message: impl Into<String>) -> Response {
    err_response(StatusCode::SERVICE_UNAVAILABLE, vec![message.into()])
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

    let Some(_slot) = upload_limits::try_acquire_upload_slot(limits.max_concurrent_uploads) else {
        tracing::warn!(
            active = upload_limits::active_uploads(),
            max = limits.max_concurrent_uploads,
            "POST /entities shed: concurrent-upload cap exceeded"
        );
        return upload_limits::shed_response(upload_limits::CONCURRENCY_SHED_MESSAGE);
    };

    let mut bytes_lease = upload_limits::reserve_in_flight();
    let mut files_lease = upload_limits::reserve_in_flight_files();

    let form = match tokio::time::timeout(
        Duration::from_millis(limits.multipart_upload_timeout_ms),
        deploy_form::read_deploy_form(
            multipart,
            &mut bytes_lease,
            &mut files_lease,
            limits.max_in_flight_upload_bytes,
            limits.max_in_flight_upload_files,
        ),
    )
    .await
    {
        Ok(Ok(form)) => form,
        Ok(Err(resp)) => return *resp,
        Err(_) => {
            tracing::warn!(
                timeout_ms = limits.multipart_upload_timeout_ms,
                "POST /entities: multipart upload timed out"
            );
            return upload_limits::timeout_response(upload_limits::MULTIPART_TIMEOUT_MESSAGE);
        }
    };

    let timeout_ms = limits.deployment_processing_timeout_ms;
    match tokio::time::timeout(
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
    }
}

fn present_truthy(v: &Value, key: &str) -> bool {
    match v.get(key) {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => true,
    }
}

/// The deployment window, both directions. A timestamp dated forward is refused for the
/// same reason a stale one is: the signature covers the entity id, so a deployment the
/// clock has not reached yet would stay replayable until it does.
fn entity_timestamp_error(entity: &Value, now_ms: i64) -> Option<String> {
    let Some(ts) = entity.get("timestamp").and_then(|v| v.as_i64()) else {
        return Some("The entity is missing a valid timestamp".to_string());
    };
    let ttl = now_ms.saturating_sub(ts);
    if ttl > ENTITY_TTL_MS {
        return Some(format!(
            "The request is not authorized to deploy: the entity timestamp is too old (older than {}s)",
            ENTITY_TTL_MS / 1000
        ));
    }
    if ttl < -MAX_DEPLOYMENT_FUTURE_SKEW_MS {
        return Some(format!(
            "The request is not authorized to deploy: the entity timestamp is too far in the future (more than {}s)",
            MAX_DEPLOYMENT_FUTURE_SKEW_MS / 1000
        ));
    }
    None
}

async fn deploy_entity_inner(
    state: &SharedState,
    headers: &HeaderMap,
    form: DeployForm,
) -> Response {
    let DeployForm { fields, files } = form;

    let entity_id = match fields.get("entityId") {
        Some(id) if !id.is_empty() => id.clone(),
        _ => return err_one("Missing entityId field"),
    };

    let auth_chain_value = match extract_auth_chain_from_fields(&fields) {
        Ok(v) => v,
        Err(e) => return err_one(e),
    };

    let mut by_hash: HashMap<String, Bytes> = HashMap::new();
    for blob in &files {
        by_hash
            .entry(hash_bytes_v1(blob))
            .or_insert_with(|| blob.clone());
    }

    let Some(entity_bytes) = by_hash.get(&entity_id).cloned() else {
        return err_one(format!(
            "The entity file was not uploaded, or its hash does not match the entityId ({entity_id})"
        ));
    };

    if entity_bytes.len() > MAX_ENTITY_FILE_SIZE_BYTES {
        return err_one(entity_file_too_large_error());
    }

    let entity: Value = match serde_json::from_slice(&entity_bytes) {
        Ok(v) => v,
        Err(e) => return err_one(format!("The entity file is not valid JSON: {e}")),
    };

    let mut errors: Vec<String> = Vec::new();

    let entity_type = entity.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if entity_type != "scene" {
        errors.push(format!(
            "Only scene entities can be deployed to a World (got type \"{entity_type}\")"
        ));
    }

    let now_ms = chrono::Utc::now().timestamp_millis();
    if let Some(error) = entity_timestamp_error(&entity, now_ms) {
        errors.push(error);
    }

    let raw_world_name = entity
        .get("metadata")
        .and_then(|m| m.get("worldConfiguration"))
        .and_then(|w| w.get("name"))
        .and_then(|n| n.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    let mut world: Option<WorldName> = None;
    match raw_world_name {
        None => errors.push(
            "The metadata.worldConfiguration.name is required to deploy a scene to a World"
                .to_string(),
        ),
        Some(name) => match WorldName::parse(name) {
            Ok(parsed) if parsed.key().ends_with(DCL_ETH_SUFFIX) => world = Some(parsed),
            _ => errors.push(format!(
                "Only .dcl.eth world names are supported for publishing (got \"{name}\")"
            )),
        },
    }

    if let Some(name) = raw_world_name
        && !state.names.check_name_deny_list(&state.pool, name).await
    {
        errors.push(format!(
            "Deployment failed: World \"{name}\" can not be deployed because the name is in the name deny list managed by Decentraland DAO."
        ));
    }

    if let Some(wc) = entity
        .get("metadata")
        .and_then(|m| m.get("worldConfiguration"))
    {
        if present_truthy(wc, "dclName") {
            errors.push("`dclName` in scene.json was renamed to `name`. Please update your scene.json accordingly.".to_string());
        }
        if present_truthy(wc, "minimapVisible") {
            errors.push(
                "`minimapVisible` in scene.json is deprecated in favor of `{ miniMapConfig: { visible } }`. Please update your scene.json accordingly."
                    .to_string(),
            );
        }
        if present_truthy(wc, "skybox") {
            errors.push(
                "`skybox` in scene.json is deprecated in favor of `{ \"skyboxConfig\": { \"fixedTime\": 36000 }}`. Please update your scene.json accordingly."
                    .to_string(),
            );
        }
    }

    let scene_meta = entity.get("metadata").and_then(|m| m.get("scene"));
    let raw_pointers = entity.get("pointers").and_then(|v| v.as_array());
    let raw_parcels = scene_meta
        .and_then(|s| s.get("parcels"))
        .and_then(|v| v.as_array());
    let raw_base = scene_meta
        .and_then(|s| s.get("base"))
        .and_then(|b| b.as_str());

    let pointers = raw_pointers
        .map(|a| canon_pointer_set(a))
        .unwrap_or_default();
    let scene_parcels = raw_parcels
        .map(|a| canon_pointer_set(a))
        .unwrap_or_default();

    let pointers_canonical = raw_pointers
        .map(|a| is_canonical_parcel_set(a))
        .unwrap_or(false);
    let parcels_canonical = raw_parcels
        .map(|a| is_canonical_parcel_set(a))
        .unwrap_or(false);

    if !pointers_canonical {
        errors.push(
            "The entity pointers must be a unique set of canonical parcel coordinates".to_string(),
        );
    }
    if !parcels_canonical {
        errors.push(
            "The scene parcels must be a unique set of canonical parcel coordinates".to_string(),
        );
    } else {
        let base_included = raw_base
            .map(|base| raw_parcels.is_some_and(|a| a.iter().any(|p| p.as_str() == Some(base))))
            .unwrap_or(false);
        if !base_included {
            let listed = raw_parcels
                .map(|a| {
                    a.iter()
                        .filter_map(|p| p.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            errors.push(format!(
                "The scene base parcel [{}] must be included in the scene parcels [{}].",
                raw_base.unwrap_or(""),
                listed
            ));
        }
    }

    if pointers_canonical && parcels_canonical && pointers != scene_parcels {
        errors.push("The entity pointers do not match metadata.scene.parcels".to_string());
    }

    for parcel in &pointers {
        if let Err(msg) = validate_parcel_in_bounds(parcel) {
            errors.push(msg);
        }
    }

    let mut total_content_size: i64 = 0;
    match entity.get("content") {
        Some(Value::Array(items)) => {
            for item in items {
                let file = item.get("file").and_then(|v| v.as_str()).unwrap_or("");
                let hash = item.get("hash").and_then(|v| v.as_str()).unwrap_or("");
                if hash.is_empty() {
                    errors.push(format!("Content entry \"{file}\" is missing a hash"));
                    continue;
                }
                match by_hash.get(hash) {
                    Some(blob) => {
                        total_content_size = total_content_size.saturating_add(blob.len() as i64)
                    }
                    None => {
                        let stored_len = if is_canonical_cid(hash) {
                            match state.storage.file_info(hash).await {
                                Ok(info) => info.map(|i| i.size as i64),
                                Err(e) => {
                                    tracing::warn!(hash, error = %e, "deploy: could not stat an already-stored file");
                                    None
                                }
                            }
                        } else {
                            None
                        };
                        match stored_len {
                            Some(size) => total_content_size = total_content_size.saturating_add(size),
                            None => errors.push(format!("The file {file} ({hash}) was not uploaded or its hash does not match its content")),
                        }
                    }
                }
            }
        }
        Some(Value::Null) | None => {}
        Some(_) => errors.push("The entity content must be an array".to_string()),
    }

    let max_world_size = world
        .as_ref()
        .map_or(crate::policy::MAX_WORLD_SIZE_BYTES, |w| {
            state.policy.world_size_cap(w)
        });
    if total_content_size > max_world_size {
        errors.push(format!(
            "The deployment exceeds the maximum world size of {max_world_size} bytes"
        ));
    }

    validate_navmap_thumbnail(&entity, &mut errors);

    let signer: Option<Address> =
        match serde_json::from_value::<AuthChain>(auth_chain_value.clone()) {
            Ok(chain) => match crate::auth::verify_auth_chain_async(
                &chain,
                &entity_id,
                Some(now_ms),
                state.eip1654.as_deref(),
            )
            .await
            {
                Ok(()) => match chain.first().map(|link| Address::parse(&link.payload)) {
                    Some(Ok(address)) => Some(address),
                    Some(Err(e)) => {
                        errors.push(format!("The auth chain signer is not an address: {e}"));
                        None
                    }
                    None => {
                        errors.push("The auth chain is empty".to_string());
                        None
                    }
                },
                Err(crate::auth::AuthError::Eip1654ValidationFailed(e)) => {
                    return unavailable(format!(
                        "The auth chain could not be verified right now: {e}"
                    ));
                }
                Err(e) => {
                    errors.push(format!("The auth chain is invalid: {e}"));
                    None
                }
            },
            Err(e) => {
                errors.push(format!("The auth chain is malformed: {e}"));
                None
            }
        };

    if !errors.is_empty() {
        return err_response(StatusCode::BAD_REQUEST, errors);
    }

    let Some(signer) = signer else {
        return err_one("Could not recover the signer from the auth chain");
    };
    let Some(world) = world else {
        return err_one("Missing world name");
    };
    let parcels = match ParcelSet::parse(&pointers) {
        Ok(p) => p,
        Err(e) => return err_one(e.to_string()),
    };

    let replacement = match state.policy.can_deploy(&world, &signer, &parcels).await {
        Ok(decision) => decision,
        Err(AuthorityNotEstablished::RefusedLacksAuthority { detail }) => {
            tracing::info!(world = %world, signer = %signer.as_str(), "deploy denied: {detail}");
            return forbidden(detail);
        }
        Err(undetermined) => {
            tracing::warn!(world = %world, signer = %signer.as_str(), error = %undetermined, "deploy denied: authority undetermined (fail-closed)");
            return forbidden(
                "Not authorized: could not verify deployment permissions (deploy denied)",
            );
        }
    };
    match store::is_world_disabled(&state.pool, &world).await {
        Ok(false) => {}
        Ok(true) => return forbidden(store::world_disabled(&world).public_message()),
        Err(e) => {
            tracing::error!(error = ?e, world = %world, "deploy failed: disabled check");
            return internal("Failed to persist deployment");
        }
    }
    let name_owner = match &replacement {
        Deploy::Owner => Some(&signer),
        Deploy::Scoped(_) => None,
    };

    let mut blobs_to_store: Vec<(String, Bytes)> = vec![(entity_id.clone(), entity_bytes.clone())];
    if let Some(Value::Array(items)) = entity.get("content") {
        for item in items {
            if let Some(hash) = item.get("hash").and_then(|v| v.as_str())
                && let Some(blob) = by_hash.get(hash)
            {
                blobs_to_store.push((hash.to_string(), blob.clone()));
            }
        }
    }
    for (hash, bytes) in blobs_to_store {
        if let Err(e) = state.storage.store(&hash, bytes).await {
            tracing::error!(error = %e, hash = %hash, "deploy failed: could not store blob");
            return internal("Failed to persist deployment content");
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
            size: total_content_size,
            replacement: &replacement,
            forward_to: state.forwarder.as_deref().map(|f| f.target()),
        },
    )
    .await;
    if let Err(e) = persisted {
        if e.is_conflict() {
            tracing::info!(world = %world, entity_id = %entity_id, "deploy conflict: overlapping-scene set changed under a scoped authorization");
            return e.into_response();
        }
        if e.is_forbidden() || e.is_bad_request() {
            let reason = e.public_message();
            tracing::info!(world = %world, entity_id = %entity_id, reason = %reason, "deploy refused");
            return if e.is_forbidden() {
                forbidden(reason)
            } else {
                err_one(reason)
            };
        }
        tracing::error!(error = ?e, world = %world, entity_id = %entity_id, "deploy failed: DB tx error");
        return internal("Failed to persist deployment");
    }

    tracing::info!(
        entity_id = %entity_id,
        signer = %signer.as_str(),
        world = %world,
        authz = match replacement { Deploy::Owner => "name-ownership", Deploy::Scoped(_) => "acl" },
        file_count = files.len(),
        content_size = total_content_size,
        user_agent = headers.get("user-agent").and_then(|v| v.to_str().ok()).unwrap_or("unknown"),
        forwarded = state.forwarder.is_some(),
        "POST /entities - deployed (validated + authorized + persisted)"
    );
    if let Some(forwarder) = &state.forwarder {
        forwarder.kick();
    }

    (
        StatusCode::OK,
        Json(DeploySuccess {
            creation_timestamp: now_ms,
            message: format!(
                "Deployment {entity_id} was successful, world {} is now available.",
                world.key()
            ),
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
    fn a_timestamp_past_the_forwards_bound_is_refused() {
        let error = entity_timestamp_error(
            &json!({ "timestamp": NOW_MS + MAX_DEPLOYMENT_FUTURE_SKEW_MS + 1 }),
            NOW_MS,
        )
        .expect("a deployment the clock has not reached must not be accepted");
        assert!(error.contains("too far in the future"), "{error}");
        assert!(error.contains("900s"), "{error}");
    }

    #[test]
    fn a_stale_timestamp_is_still_refused() {
        let error = entity_timestamp_error(&json!({ "timestamp": NOW_MS - 300_001 }), NOW_MS)
            .expect("a deployment older than the TTL must not be accepted");
        assert!(error.contains("too old"), "{error}");
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
}
