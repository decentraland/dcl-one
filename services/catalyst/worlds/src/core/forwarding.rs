//! The worlds lane of upstream forwarding (`WORLDS_FORWARD_UPSTREAM_URL`;
//! the engine is `content::forward`): a local publish is queued in its
//! transaction (`store::deploy_scene`) and sent, unchanged, to the upstream
//! worlds content server's `POST /entities`. The operator reads the records
//! under `/admin/forwarding` and requeues one at
//! `/admin/forwarding/{entity_id}/retry`, behind `CATALYRST_WORLDS_ADMIN_TOKEN`.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use serde_json::Value;

use crate::content::forward::{self, Lane, ListQuery, Requeued};

use super::SharedState;
use super::auth::authorize_admin;
use super::http::ApiError;

/// `worlds.forward_outbox` to `<upstream>/entities`. The source is the scene
/// while this node still hosts it as its own publish (a replaced or
/// undeployed scene has no row; a mirrored one has a `mirror_source`). A
/// worlds server takes an entity for five minutes from its signature.
pub const LANE: Lane = Lane {
    job: "worlds-forward",
    table: "worlds.forward_outbox",
    deploy_path: "/entities",
    available_path: "/available-content",
    source_sql: "SELECT s.deployment_auth_chain FROM worlds.forward_outbox o \
                 JOIN worlds.world_scenes s ON s.world_name = o.world_name AND s.entity_id = o.entity_id \
                 WHERE o.id = $1 AND s.mirror_source IS NULL",
    world_column: Some("world_name"),
    max_age_ms: super::deploy::ENTITY_TTL_MS,
    max_ahead_ms: super::deploy::MAX_DEPLOYMENT_FUTURE_SKEW_MS,
};

/// `GET /admin/forwarding/{entity_id}`: one publish's forwarding record.
pub async fn get(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(entity_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    authorize_admin(state.settings.admin_token.as_deref(), &headers)?;
    forward::status_of(&state.pool, &LANE, &entity_id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format!("No forwarding record for {entity_id}.")))
}

/// `GET /admin/forwarding?status=&before=&limit=`: the records, newest
/// first, `next` the cursor of the following page.
pub async fn list(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    authorize_admin(state.settings.admin_token.as_deref(), &headers)?;
    let status = q.status().map_err(ApiError::bad_request)?;
    Ok(Json(
        forward::list(&state.pool, &LANE, status, q.before, q.limit).await?,
    ))
}

/// `POST /admin/forwarding/{entity_id}/retry`: `Forwarder::requeue`; 409 for
/// a record in another state or an entity past the upstream's window, 503
/// while `WORLDS_FORWARD_UPSTREAM_URL` is unset.
pub async fn retry(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(entity_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    authorize_admin(state.settings.admin_token.as_deref(), &headers)?;
    let forwarder = state.forwarder.as_ref().ok_or_else(|| {
        ApiError::service_unavailable("Forwarding is off (WORLDS_FORWARD_UPSTREAM_URL is unset).")
    })?;
    match forwarder.requeue(&entity_id).await? {
        Requeued::Queued(record) => Ok(Json(record)),
        Requeued::Unknown => Err(ApiError::not_found(format!(
            "No forwarding record for {entity_id}."
        ))),
        Requeued::Kept(status) => Err(ApiError::conflict(Requeued::kept_message(&status))),
        Requeued::TooOld(reason) => Err(ApiError::conflict(reason)),
    }
}
