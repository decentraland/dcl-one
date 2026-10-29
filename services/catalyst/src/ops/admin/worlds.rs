//! `/admin/api/worlds/*`, `/admin/api/scene/reload` and
//! `/admin/api/scene-state/*`: the worlds operator routes
//! (`worlds::core::admin`, bearer `CATALYRST_WORLDS_ADMIN_TOKEN`) and the
//! authoritative scene host's (`worlds::scene_state`: the debugging reload
//! with `SCENE_HOST_DEBUGGING_SECRET` in its body, the CRDT dump, kick-all
//! and reset with the scene host's admin bearer), each the module's own
//! handler with the credential the node configured for it.
//!
//! The world enable/disable and the wallet block list are worlds writes:
//! `worlds::writes::is_gated` on the old path, so `WORLDS_WRITES_ENABLED`
//! refuses them as the worlds router does. The scene host's routes are not
//! user authority and were never wrapped (`worlds::writes`), so neither are
//! these.

#![allow(clippy::result_large_err)]

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::Method;
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use super::SharedState;
use super::auth::AdminSession;
use super::console::{
    Call, address_field, configured, finish, not_configured, query_from_obj, refused,
    segment_field, service_headers, str_field, token_missing,
};
use crate::worlds::core::admin as wadmin;

const DOMAIN: &str = "worlds";

async fn worlds_for(
    state: &SharedState,
    session: &AdminSession,
    method: Method,
    path: &str,
) -> Result<(crate::worlds::core::SharedState, axum::http::HeaderMap), Response> {
    let Some(ws) = state.domains.worlds.clone() else {
        return Err(not_configured(DOMAIN));
    };
    if crate::worlds::writes::is_gated(&method, path)
        && let Some(r) = crate::shared::writes::refusal(state.domains.worlds_writes_enabled)
    {
        return Err(refused(crate::worlds::writes::refused_response(&r)).await);
    }
    let Some(token) = configured(ws.settings.admin_token.as_deref()) else {
        return Err(token_missing(DOMAIN));
    };
    Ok((ws.clone(), service_headers(Some(token), session.address())))
}

/// `POST /admin/api/worlds/list` `{limit?, offset?}`.
pub async fn list(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let (ws, headers) = match worlds_for(&state, &session, Method::GET, "/admin/worlds").await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let q = match query_from_obj::<wadmin::PageQuery>(&body, &["limit", "offset"]) {
        Ok(q) => q,
        Err(resp) => return resp,
    };
    let answer = wadmin::list_worlds(State(ws), headers, Query(q))
        .await
        .into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "worlds.list").detail(body),
        answer,
    )
    .await
}

fn world_name(body: &Value) -> Result<String, Response> {
    segment_field(body, "world_name", "world-name")
}

/// `POST /admin/api/worlds/detail` `{world_name}`.
pub async fn detail(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let name = match world_name(&body) {
        Ok(n) => n,
        Err(resp) => return resp,
    };
    let (ws, headers) = match worlds_for(
        &state,
        &session,
        Method::GET,
        &format!("/admin/worlds/{name}"),
    )
    .await
    {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let answer = wadmin::world_detail(State(ws), headers, Path(name.clone()))
        .await
        .into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "worlds.detail")
            .target(name.clone())
            .detail(json!({ "world_name": name })),
        answer,
    )
    .await
}

/// `POST /admin/api/worlds/enable` `{world_name}`.
pub async fn enable(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let name = match world_name(&body) {
        Ok(n) => n,
        Err(resp) => return resp,
    };
    let (ws, headers) = match worlds_for(
        &state,
        &session,
        Method::POST,
        &format!("/admin/worlds/{name}/enable"),
    )
    .await
    {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let answer = wadmin::enable_world(State(ws), headers, Path(name.clone()))
        .await
        .into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "worlds.enable")
            .target(name.clone())
            .detail(json!({ "world_name": name })),
        answer,
    )
    .await
}

/// `POST /admin/api/worlds/disable` `{world_name}`.
pub async fn disable(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let name = match world_name(&body) {
        Ok(n) => n,
        Err(resp) => return resp,
    };
    let (ws, headers) = match worlds_for(
        &state,
        &session,
        Method::POST,
        &format!("/admin/worlds/{name}/disable"),
    )
    .await
    {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let answer = wadmin::disable_world(State(ws), headers, Path(name.clone()))
        .await
        .into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "worlds.disable")
            .target(name.clone())
            .detail(json!({ "world_name": name })),
        answer,
    )
    .await
}

/// `POST /admin/api/worlds/ban-status` `{world_name, address, parcel?}`.
pub async fn ban_status(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let name = match world_name(&body) {
        Ok(n) => n,
        Err(resp) => return resp,
    };
    let (ws, headers) = match worlds_for(
        &state,
        &session,
        Method::GET,
        &format!("/admin/worlds/{name}/ban-status"),
    )
    .await
    {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let q = match query_from_obj::<wadmin::BanStatusQuery>(&body, &["address", "parcel"]) {
        Ok(q) => q,
        Err(resp) => return resp,
    };
    let answer = wadmin::world_ban_status(State(ws), headers, Path(name.clone()), Query(q))
        .await
        .into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "worlds.ban-status")
            .target(name)
            .detail(body),
        answer,
    )
    .await
}

/// `POST /admin/api/worlds/blocked-list`.
pub async fn blocked_list(session: AdminSession, State(state): State<SharedState>) -> Response {
    let (ws, headers) = match worlds_for(&state, &session, Method::GET, "/admin/blocked").await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let answer = wadmin::list_blocked(State(ws), headers)
        .await
        .into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "worlds.blocked.list"),
        answer,
    )
    .await
}

/// `POST /admin/api/worlds/blocked-add` `{wallet}`.
pub async fn blocked_add(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let wallet = match address_field(&body, "wallet", "wallet") {
        Ok(w) => w,
        Err(resp) => return resp,
    };
    let (ws, headers) = match worlds_for(
        &state,
        &session,
        Method::POST,
        &format!("/admin/blocked/{wallet}"),
    )
    .await
    {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let answer = wadmin::block_wallet(State(ws), headers, Path(wallet.clone()))
        .await
        .into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "worlds.blocked.add")
            .target(wallet.clone())
            .detail(json!({ "wallet": wallet })),
        answer,
    )
    .await
}

/// `POST /admin/api/worlds/blocked-remove` `{wallet}`.
pub async fn blocked_remove(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let wallet = match address_field(&body, "wallet", "wallet") {
        Ok(w) => w,
        Err(resp) => return resp,
    };
    let (ws, headers) = match worlds_for(
        &state,
        &session,
        Method::DELETE,
        &format!("/admin/blocked/{wallet}"),
    )
    .await
    {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let answer = wadmin::unblock_wallet(State(ws), headers, Path(wallet.clone()))
        .await
        .into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "worlds.blocked.remove")
            .target(wallet.clone())
            .detail(json!({ "wallet": wallet })),
        answer,
    )
    .await
}

/// `POST /admin/api/worlds/access-log` `{world?, address?, limit?, offset?}`.
pub async fn access_log(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let (ws, headers) = match worlds_for(&state, &session, Method::GET, "/admin/access-log").await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let q = match query_from_obj::<wadmin::AccessLogQuery>(
        &body,
        &["world", "address", "limit", "offset"],
    ) {
        Ok(q) => q,
        Err(resp) => return resp,
    };
    let answer = wadmin::access_log(State(ws), headers, Query(q))
        .await
        .into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "worlds.access-log").detail(body),
        answer,
    )
    .await
}

fn valid_scene_name(s: &str) -> bool {
    !s.is_empty() && s.len() <= 256 && s.chars().all(|c| !c.is_control() && !c.is_whitespace())
}

/// `POST /admin/api/scene/reload` `{name}` (or `sceneId`): the host's
/// debugging reload, its secret put in the body by the node, never logged.
pub async fn scene_reload(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let Some(host) = state.domains.scene_host.clone() else {
        return not_configured("scene-state");
    };
    let Some(secret) = configured(host.settings.debugging_secret.as_deref()) else {
        return super::console::credential_missing("debugging-secret-not-configured");
    };
    let Some(name) = str_field(&body, "name")
        .or_else(|| str_field(&body, "sceneId"))
        .map(|n| n.trim().to_string())
        .filter(|n| valid_scene_name(n))
    else {
        return super::api::AdminEnvelope::error(
            axum::http::StatusCode::BAD_REQUEST,
            json!({ "error": "missing-or-invalid-scene-name" }),
        );
    };
    let payload = Bytes::from(json!({ "secret": secret, "name": name }).to_string());
    let answer = crate::worlds::scene_state::handlers::reload(State(host), payload)
        .await
        .into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "scene.reload")
            .target(name.clone())
            .detail(json!({ "name": name })),
        answer,
    )
    .await
}

fn scene_host_for(
    state: &SharedState,
    session: &AdminSession,
) -> Result<
    (
        crate::worlds::scene_state::SharedState,
        axum::http::HeaderMap,
    ),
    Response,
> {
    let Some(host) = state.domains.scene_host.clone() else {
        return Err(not_configured("scene-state"));
    };
    let Some(token) = configured(host.settings.admin_token.as_deref()) else {
        return Err(token_missing("scene-state"));
    };
    let headers = service_headers(Some(token), session.address());
    Ok((host, headers))
}

/// `POST /admin/api/scene-state/crdt` `{scene, snapshotHex?}`.
pub async fn scene_crdt(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let scene = match segment_field(&body, "scene", "scene") {
        Ok(s) => s,
        Err(resp) => return resp,
    };
    let (host, headers) = match scene_host_for(&state, &session) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let q = match query_from_obj::<crate::worlds::scene_state::admin::InspectQuery>(
        &body,
        &["snapshotHex"],
    ) {
        Ok(q) => q,
        Err(resp) => return resp,
    };
    let answer = crate::worlds::scene_state::admin::inspect_crdt(
        State(host),
        headers,
        Path(scene.clone()),
        Query(q),
    )
    .await;
    finish(
        &state,
        Call::new(&session, DOMAIN, "scene-state.crdt")
            .target(scene.clone())
            .detail(json!({ "scene": scene })),
        answer,
    )
    .await
}

/// `POST /admin/api/scene-state/kick-all` `{scene}`.
pub async fn scene_kick_all(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let scene = match segment_field(&body, "scene", "scene") {
        Ok(s) => s,
        Err(resp) => return resp,
    };
    let (host, headers) = match scene_host_for(&state, &session) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let answer =
        crate::worlds::scene_state::admin::kick_all(State(host), headers, Path(scene.clone()))
            .await;
    finish(
        &state,
        Call::new(&session, DOMAIN, "scene-state.kick-all")
            .target(scene.clone())
            .detail(json!({ "scene": scene })),
        answer,
    )
    .await
}

/// `POST /admin/api/scene-state/reset` `{scene}`.
pub async fn scene_reset(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let scene = match segment_field(&body, "scene", "scene") {
        Ok(s) => s,
        Err(resp) => return resp,
    };
    let (host, headers) = match scene_host_for(&state, &session) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let answer =
        crate::worlds::scene_state::admin::reset_state(State(host), headers, Path(scene.clone()))
            .await;
    finish(
        &state,
        Call::new(&session, DOMAIN, "scene-state.reset")
            .target(scene.clone())
            .detail(json!({ "scene": scene })),
        answer,
    )
    .await
}
