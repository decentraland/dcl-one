//! `DELETE /scenes/{coord}` (root and `/content`): a LAND holder takes down
//! the scene published against this node at that parcel.
//!
//! Authorization is the old route's: a signed fetch over `delete` and the
//! request's own path (the 6.x payload, then the pre-6.0 folded one behind
//! the canonical `signer` metadata key, which ui3 and dcl-scene-cli still
//! mint; a scene runtime's `decentraland-kernel-scene` signer refused), a
//! 5-minute window, and a deploy-granting leg for the signer on every parcel
//! of the scene the request takes down, not just the requested one
//! (`land_authz::can_deploy_batch`, the rule `POST /entities` applies to the
//! scene itself). Only a locally published scene can go ([`land_publish`]);
//! what a peer serves is not this node's to withdraw, and the freed parcels
//! are left empty. Answers: 200 `{entityId, unpublished: true, parcels}`,
//! 400 for a malformed chain or coordinate, 401 for a chain that does not
//! verify, 403 without LAND rights on any of those parcels, 404 with no
//! local scene there (or no longer: a concurrent unpublish took it down),
//! 409 when a concurrent deploy replaced the scene on the parcel, 503
//! when LAND rights cannot be decided; the [`super::writes`] gates first.
//!
//! [`land_publish`]: super::land_publish

use axum::Json;
use axum::extract::{OriginalUri, Path, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use super::SharedState;
use super::errors::{AppError, AppResult};
use super::land_publish::{UnpublishError, local_scene_at, unpublish_local_scene};
use super::writes;
use crate::shared::signed_fetch::{PayloadPolicy, SignedFetchError, require_signer};

fn parse_parcel(coord: &str) -> Option<(i32, i32)> {
    let (x, y) = crate::auth::parse_pointer(coord)?;
    Some((x.try_into().ok()?, y.try_into().ok()?))
}

fn map_auth_error(e: SignedFetchError) -> AppError {
    let (status, message) = e.http_status_and_message();
    match status {
        400 => AppError::InvalidRequest(message),
        503 => AppError::ServiceUnavailable(message),
        _ => AppError::Unauthorized(message),
    }
}

pub async fn unpublish_scene(
    State(state): State<SharedState>,
    Path(coord): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> AppResult<Response> {
    if let Some(refused) = writes::refuse(&state) {
        return Ok(refused);
    }
    let Some(deployer) = state.deployer().cloned() else {
        return Ok(writes::disabled_response());
    };
    let signer = require_signer(
        &headers,
        "delete",
        uri.path(),
        PayloadPolicy::V6ThenLegacy,
        deployer.eip1654(),
    )
    .await
    .map_err(map_auth_error)?
    .into_string();

    let (x, y) = parse_parcel(&coord).ok_or_else(|| {
        AppError::InvalidRequest(format!(
            "Scene pointers should only contain two integers separated by a comma, \
             for example (10,10) or (120,-45). Invalid pointer: {coord}"
        ))
    })?;

    let land = deployer.land_authz().ok_or_else(|| {
        AppError::ServiceUnavailable(
            "LAND ownership checks are unavailable on this node".to_string(),
        )
    })?;
    let pointer = format!("{x},{y}");
    let scene = local_scene_at(&state.pool, &pointer)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    // Every parcel of the scene the unpublish takes down; the requested one alone when none.
    let parcels: Vec<(i32, i32)> = match &scene {
        Some(scene) => scene
            .pointers
            .iter()
            .map(|p| {
                parse_parcel(p).ok_or_else(|| {
                    AppError::Internal(format!(
                        "scene {} has a non-parcel pointer {p}",
                        scene.entity_id
                    ))
                })
            })
            .collect::<Result<_, _>>()?,
        None => vec![(x, y)],
    };
    let decisions = crate::land_authz::can_deploy_batch(land, &parcels, &signer)
        .await
        .map_err(|e| AppError::ServiceUnavailable(format!("parcel access check failed: {e}")))?;
    let denied: Vec<String> = parcels
        .iter()
        .zip(&decisions)
        .filter(|(_, d)| !d.verdict.allows())
        .map(|((px, py), _)| {
            format!(
                "The provided Eth Address does not have access to the following parcel: ({px},{py})"
            )
        })
        .collect();
    if !denied.is_empty() {
        return Err(AppError::Forbidden(denied.join("; ")));
    }
    let Some(scene) = scene else {
        return Err(AppError::NotFound(format!(
            "No locally published scene at {x},{y}."
        )));
    };

    match unpublish_local_scene(&state.pool, &pointer, &scene.entity_id).await {
        Ok(outcome) => {
            state.deployments_cache.clear();
            tracing::info!(
                entity_id = %outcome.entity_id,
                signer = %signer,
                pointer = %pointer,
                parcels = outcome.parcels.len(),
                "DELETE /scenes - local publish tombstoned"
            );
            Ok(Json(json!({
                "entityId": outcome.entity_id,
                "unpublished": true,
                "parcels": outcome.parcels,
            }))
            .into_response())
        }
        Err(UnpublishError::NotLocal(_)) => Err(AppError::NotFound(format!(
            "No locally published scene at {x},{y}."
        ))),
        Err(UnpublishError::Conflict) => Err(AppError::Conflict(
            "A concurrent deployment changed the parcel; please retry.".to_string(),
        )),
        Err(UnpublishError::Unavailable(e)) => Err(AppError::ServiceUnavailable(e)),
        Err(UnpublishError::Db(e)) => Err(AppError::Internal(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_parcel_accepts_signed_coords() {
        assert_eq!(parse_parcel("52,-52"), Some((52, -52)));
        assert_eq!(parse_parcel(" -10 , 20 "), Some((-10, 20)));
    }

    #[test]
    fn parse_parcel_rejects_garbage() {
        assert_eq!(parse_parcel("52"), None);
        assert_eq!(parse_parcel("a,b"), None);
        assert_eq!(parse_parcel("1,2,3"), None);
        assert_eq!(parse_parcel(""), None);
        assert_eq!(
            parse_parcel("3000000000,0"),
            None,
            "out of the i32 range the LAND index keys by"
        );
    }

    #[test]
    fn signed_fetch_refusals_keep_the_old_statuses() {
        use axum::http::StatusCode;
        let status = |e: SignedFetchError| map_auth_error(e).into_response().status();
        assert_eq!(
            status(SignedFetchError::InsufficientLinks),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            status(SignedFetchError::MalformedChain { detail: "x".into() }),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            status(SignedFetchError::InvalidTimestamp("x".into())),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            status(SignedFetchError::InvalidSignature("x".into())),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            status(SignedFetchError::Expired {
                signed_at: 0,
                now: 1,
                window_secs: 300
            }),
            StatusCode::UNAUTHORIZED
        );
    }
}
