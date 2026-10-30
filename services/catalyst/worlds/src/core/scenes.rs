//! Unpublishing: `DELETE /world/{name}/scenes/{coord}` removes the scenes
//! standing on a parcel (the owner any of them, a deployment grantee exactly
//! the ones whose whole footprint its grant covers) and
//! `DELETE /entities/{name}` every scene of the world (the owner or a
//! world-wide deployer).

use axum::Json;
use axum::extract::{OriginalUri, Path, State};
use axum::http::{HeaderMap, StatusCode, Uri};
use serde_json::{Value, json};

use crate::comms::world_identity::{Parcel, ParcelSet};
use crate::policy::Permission;
use crate::shared::identity::{AuthorityNotEstablished, PermissionDecision};
use crate::shared::signed_fetch::SignedPath;

use super::SharedState;
use super::auth::{map_auth_error_api, require_verified};
use super::http::ApiError;
use super::permissions::parse_world;
use super::store;

/// DELETE /world/{world_name}/scenes/{scene_coord}
pub async fn delete_scene(
    State(state): State<SharedState>,
    Path((world_name, scene_coord)): Path<(String, String)>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
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
    let parcel = Parcel::parse(&scene_coord).map_err(|e| ApiError::bad_request(e.to_string()))?;

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
                return Err(ApiError::forbidden(format!(
                    "Your wallet can not unpublish scenes from \"{}\".",
                    world.display()
                )));
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

    let removed = store::undeploy_scene(&state.pool, &world, parcel, authorized.as_deref()).await?;
    if removed == 0 {
        return Err(ApiError::not_found(format!(
            "No scene is published at {parcel} in \"{}\".",
            world.display()
        )));
    }
    Ok(StatusCode::OK)
}

/// DELETE /entities/{world_name}
pub async fn undeploy_world(
    State(state): State<SharedState>,
    Path(world_name): Path<String>,
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
        return Err(ApiError::forbidden(format!(
            "You must have world-wide deployment permission to undeploy \"{}\".",
            world.display()
        )));
    }

    let removed = store::undeploy_world(&state.pool, &world).await?;
    tracing::info!(world = %world, signer = %auth.signer.as_str(), removed, "world undeployed");
    Ok(Json(json!({})))
}
