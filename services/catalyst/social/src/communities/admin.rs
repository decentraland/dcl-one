//! Operator routes: the bearer-gated admin listing and suspension (403 when
//! no token is configured or it does not match), and the global-moderator
//! listing of every community (signed fetch + the allow-list).

use axum::Json;
use axum::extract::{OriginalUri, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;

use crate::CommunityId;

use super::http::{CommError, Paginated, err_json, get_first, get_pagination_params};
use super::{SharedState, queries, signed_path};

fn require_admin(state: &SharedState, headers: &HeaderMap) -> Result<String, Response> {
    if state
        .settings
        .admin_token
        .as_deref()
        .filter(|s| !s.is_empty())
        .is_none()
    {
        return Err(err_json(
            StatusCode::FORBIDDEN,
            "admin controls disabled (COMMUNITIES_ADMIN_TOKEN unset)",
        ));
    }
    if !state.admin_bearer(headers) {
        return Err(err_json(
            StatusCode::FORBIDDEN,
            "admin bearer token required",
        ));
    }
    Ok("admin-token".to_string())
}

#[derive(Debug, Default, Deserialize)]
pub struct SuspendBody {
    #[serde(default)]
    pub reason: Option<String>,
}

pub async fn suspend_community(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id_str): Path<String>,
    body: Option<Json<SuspendBody>>,
) -> Response {
    let actor = match require_admin(&state, &headers) {
        Ok(a) => a,
        Err(e) => return e,
    };
    set_suspension(
        &state,
        &id_str,
        true,
        &actor,
        body.and_then(|Json(b)| b.reason),
    )
    .await
}

pub async fn unsuspend_community(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id_str): Path<String>,
) -> Response {
    let actor = match require_admin(&state, &headers) {
        Ok(a) => a,
        Err(e) => return e,
    };
    set_suspension(&state, &id_str, false, &actor, None).await
}

async fn set_suspension(
    state: &SharedState,
    id_str: &str,
    suspended: bool,
    actor: &str,
    reason: Option<String>,
) -> Response {
    let Ok(id) = CommunityId::parse(id_str) else {
        return err_json(StatusCode::BAD_REQUEST, "invalid community id");
    };
    match queries::set_suspended(&state.pool, id, suspended, actor, reason.as_deref()).await {
        Ok(true) => {
            if suspended {
                state.community_closed_to_worlds(id);
            }
            (
                StatusCode::OK,
                Json(json!({ "ok": true, "id": id.as_uuid(), "suspended": suspended })),
            )
                .into_response()
        }
        Ok(false) => err_json(
            StatusCode::NOT_FOUND,
            format!("Community not found: {id_str}"),
        ),
        Err(e) => {
            tracing::error!(error = %e, community_id = %id, "admin set_suspended failed");
            err_json(StatusCode::INTERNAL_SERVER_ERROR, "database error")
        }
    }
}

pub async fn list_communities(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    if let Err(e) = require_admin(&state, &headers) {
        return e;
    }
    let pagination = get_pagination_params(&pairs);
    let status = get_first(&pairs, "status").unwrap_or_else(|| "all".to_string());
    let owner = get_first(&pairs, "owner");
    let search = get_first(&pairs, "search");
    match queries::admin_list(
        &state.pool,
        &pagination,
        &status,
        owner.as_deref(),
        search.as_deref(),
    )
    .await
    {
        Ok((results, total)) => (
            StatusCode::OK,
            Json(json!({ "data": Paginated::new(results, total, &pagination) })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "admin list_communities failed");
            err_json(StatusCode::INTERNAL_SERVER_ERROR, "database error")
        }
    }
}

pub async fn get_moderation_communities(
    State(state): State<SharedState>,
    headers: HeaderMap,
    original: OriginalUri,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Result<Json<serde_json::Value>, CommError> {
    let signer = state
        .require_signer(
            &headers,
            "get",
            &signed_path(&original, "/v1/moderation/communities"),
        )
        .await?;
    if !state.is_global_moderator(&signer) {
        return Err(CommError::MessageOnly(
            StatusCode::FORBIDDEN,
            "Access denied. Global moderator privileges required.".into(),
        ));
    }
    let pagination = get_pagination_params(&pairs);
    let search = get_first(&pairs, "search");
    let (rows, total) =
        queries::moderation_all(&state.pool, search.as_deref(), &pagination).await?;
    Ok(Json(
        json!({ "data": Paginated::new(rows, total, &pagination) }),
    ))
}
