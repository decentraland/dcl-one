//! Unpublishing: `DELETE /world/{name}/scenes/{coord}` removes the scenes
//! standing on a parcel (the owner any of them, a deployment grantee exactly
//! the ones whose whole footprint its grant covers) and
//! `DELETE /entities/{name}` every scene of the world (the owner or a
//! world-wide deployer). Statuses and bodies follow the old routes: 401 for a
//! refused parcel undeploy, 400 for a refused world undeploy, and a 200 body
//! whether or not anything was standing there.

use axum::Json;
use axum::extract::{OriginalUri, Path, State};
use axum::http::{HeaderMap, Uri, header};
use axum::response::IntoResponse;
use serde_json::{Value, json};

use crate::comms::world_identity::{Parcel, ParcelSet, WorldName};
use crate::policy::Permission;
use crate::shared::identity::{AuthorityNotEstablished, PermissionDecision};
use crate::shared::signed_fetch::SignedPath;

use super::SharedState;
use super::auth::{map_auth_error_api, require_verified};
use super::http::ApiError;
use super::permissions::parse_world;
use super::store;

/// Freed space may lift the stored owner's quota block (best effort, off the request).
fn recheck_owner_quota(state: &SharedState, world: &WorldName) {
    let (state, world) = (state.clone(), world.clone());
    tokio::spawn(async move { recheck_owner_quota_now(&state, &world).await });
}

async fn recheck_owner_quota_now(state: &SharedState, world: &WorldName) {
    match state.policy.world(world).await {
        Ok(Some(row)) => {
            if let Some(owner) = row.owner.filter(|_| row.quota_blocked_since.is_some()) {
                state
                    .unblock_if_under_quota(&owner, chrono::Utc::now().naive_utc())
                    .await;
            }
        }
        Ok(None) => {}
        Err(error) => tracing::warn!(%world, %error, "quota recheck after undeploy skipped"),
    }
}

/// DELETE /world/{world_name}/scenes/{scene_coord}
pub async fn delete_scene(
    State(state): State<SharedState>,
    Path((world_name, scene_coord)): Path<(String, String)>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let world = parse_world(&world_name)?;
    let auth = require_verified(
        &headers,
        "delete",
        &SignedPath::from_uris(&original, &uri),
        &[],
        state.eip1654.as_deref(),
    )
    .await
    .map_err(map_auth_error_api)?;
    let parcel = Parcel::parse_strict(&scene_coord).map_err(|_| {
        ApiError::bad_request(format!(
            "Invalid coordinate format: {scene_coord}. Expected format: x,y (e.g., 0,0 or -1,2)"
        ))
    })?;

    let authorized = match state.policy.is_owner(&world, &auth.signer).await {
        PermissionDecision::Allowed => None,
        PermissionDecision::Refused(AuthorityNotEstablished::RefusedLacksAuthority { .. }) => {
            let footprint: ParcelSet = std::iter::once(parcel).collect();
            let overlapping = state.policy.overlapping_scenes(&world, &footprint).await?;
            let mut required = footprint;
            for (_, parcels) in &overlapping {
                required.extend(parcels.iter().copied());
            }
            let allowed = state
                .policy
                .has_permission_covering(&world, Permission::Deployment, &auth.signer, &required)
                .await?;
            if !allowed {
                return Err(ApiError::unauthorized(
                    "Unauthorized. You do not have permission to undeploy scenes in this world.",
                ));
            }
            Some(
                overlapping
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect::<Vec<_>>(),
            )
        }
        PermissionDecision::Refused(undetermined) => return Err(undetermined.into()),
    };

    store::undeploy_scene(&state.pool, &world, parcel, authorized.as_deref()).await?;
    recheck_owner_quota(&state, &world);
    Ok(Json(json!({
        "message": format!("Scene at parcel {scene_coord} undeployed successfully")
    })))
}

/// DELETE /entities/{world_name}. The 200 is the string `{}` as `text/plain`, as upstream's
/// handler answers it (`body: '{}'`).
pub async fn undeploy_world(
    State(state): State<SharedState>,
    Path(world_name): Path<String>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    let world = parse_world(&world_name)?;
    let auth = require_verified(
        &headers,
        "delete",
        &SignedPath::from_uris(&original, &uri),
        &[],
        state.eip1654.as_deref(),
    )
    .await
    .map_err(map_auth_error_api)?;

    let authorized = match state.policy.is_owner(&world, &auth.signer).await {
        PermissionDecision::Allowed => true,
        PermissionDecision::Refused(AuthorityNotEstablished::RefusedLacksAuthority { .. }) => {
            state
                .policy
                .has_world_wide_permission(&world, Permission::Deployment, &auth.signer)
                .await?
        }
        PermissionDecision::Refused(undetermined) => return Err(undetermined.into()),
    };
    if !authorized {
        return Err(ApiError::bad_request(
            "Invalid request. You must have world-wide deployment permission to undeploy the entire world.",
        ));
    }

    let removed = store::undeploy_world(&state.pool, &world).await?;
    recheck_owner_quota(&state, &world);
    tracing::info!(world = %world, signer = %auth.signer.as_str(), removed, "world undeployed");
    Ok(([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], "{}"))
}
