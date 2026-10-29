//! `/admin/api/{social,social-rpc,communities,notifications,badges}/*`: the
//! platform moderation of `comms::gatekeeper` (ban, unban, warn), the social
//! RPC operator routes (`social::rpc::admin`), the community suspension and
//! listing, the notification broadcast and the badge grant/revoke, each the
//! module's own handler presented with the bearer this node configured for
//! it (`MODERATOR_TOKEN`, `CATALYRST_SOCIAL_RPC_ADMIN_TOKEN`,
//! `COMMUNITIES_ADMIN_TOKEN`, `CATALYRST_NOTIFICATIONS_ADMIN_TOKEN`,
//! `CATALYRST_BADGES_ADMIN_TOKEN`). The platform moderation writes name the
//! console wallet as the moderator (`?moderator=`, the bearer path of
//! `comms::gatekeeper::auth::authorize_moderator`).
//!
//! Writes go through `social::writes::is_gated` on the route-local path the
//! old proxy hit, so `SOCIAL_WRITES_ENABLED` refuses exactly what the social
//! routers refuse: the RPC `disconnect` and `force-presence` stay open (they
//! change no durable state), `reset-settings` and every moderation,
//! suspension, broadcast and badge write answer the social 503.

#![allow(clippy::result_large_err)]

use axum::Json;
use axum::body::Bytes;
use axum::extract::{OriginalUri, Path, Query, State};
use axum::http::Method;
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use super::SharedState;
use super::auth::AdminSession;
use super::console::{
    Call, address_field, body_as, body_without, configured, credential_missing, envelope, finish,
    not_configured, present, query_from_obj, refused, segment_field, service_headers, str_field,
    token_missing,
};
use crate::comms::gatekeeper::moderation::handlers::{self as comms, ModeratorQuery};
use crate::social::notifications::admin::RequireAdmin;
use crate::social::rpc::admin as rpc_admin;
use crate::social::writes;

/// The path the comms route is mounted at, for a handler called in process:
/// the console's call carries the moderator bearer, so the path is never
/// signed over, but the handler is the gatekeeper's and takes its own URI.
fn mounted(route: &str) -> OriginalUri {
    OriginalUri(
        format!("/comms-gatekeeper{route}")
            .parse()
            .expect("a mounted comms path"),
    )
}

const DOMAIN: &str = "social";

async fn gate(state: &SharedState, method: &Method, path: &str) -> Result<(), Response> {
    if writes::is_gated(method, path)
        && let Some(r) = crate::shared::writes::refusal(state.domains.social_writes_enabled)
    {
        return Err(refused(writes::refused_response(&r)).await);
    }
    Ok(())
}

async fn comms_for(
    state: &SharedState,
    session: &AdminSession,
    method: Method,
    path: &str,
) -> Result<(crate::comms::gatekeeper::SharedState, axum::http::HeaderMap), Response> {
    let Some(cs) = state.domains.comms.clone() else {
        return Err(not_configured(DOMAIN));
    };
    gate(state, &method, path).await?;
    let Some(token) = configured(cs.settings.moderator_token.as_deref()) else {
        return Err(credential_missing("comms-moderator-token-not-configured"));
    };
    Ok((cs.clone(), service_headers(Some(token), session.address())))
}

fn moderator(session: &AdminSession) -> Query<ModeratorQuery> {
    Query(ModeratorQuery {
        moderator: Some(session.address().to_string()),
    })
}

/// `POST /admin/api/social/user-ban` `{address, reason, duration?, customMessage?}`.
pub async fn user_ban(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let address = match address_field(&body, "address", "address") {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    let path = format!("/users/{address}/bans");
    let (cs, headers) = match comms_for(&state, &session, Method::POST, &path).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let rest = body_without(&body, &["address"]).unwrap_or_else(|| json!({}));
    let answer = comms::post_user_ban(
        State(cs),
        mounted(&format!("/users/{address}/bans")),
        headers,
        Path(address.clone()),
        moderator(&session),
        Bytes::from(rest.to_string()),
    )
    .await
    .into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "social.user-ban")
            .target(address)
            .detail(rest),
        answer,
    )
    .await
}

/// `POST /admin/api/social/user-unban` `{address}`.
pub async fn user_unban(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let address = match address_field(&body, "address", "address") {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    let path = format!("/users/{address}/bans");
    let (cs, headers) = match comms_for(&state, &session, Method::DELETE, &path).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let answer = comms::delete_user_ban(
        State(cs),
        mounted(&format!("/users/{address}/bans")),
        headers,
        Path(address.clone()),
        moderator(&session),
    )
    .await
    .into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "social.user-unban")
            .target(address)
            .detail(json!({})),
        answer,
    )
    .await
}

/// `POST /admin/api/social/user-warning` `{address, reason}`.
pub async fn user_warning(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let address = match address_field(&body, "address", "address") {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    let path = format!("/users/{address}/warnings");
    let (cs, headers) = match comms_for(&state, &session, Method::POST, &path).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let rest = body_without(&body, &["address"]).unwrap_or_else(|| json!({}));
    let answer = comms::post_user_warning(
        State(cs),
        mounted(&format!("/users/{address}/warnings")),
        headers,
        Path(address.clone()),
        moderator(&session),
        Bytes::from(rest.to_string()),
    )
    .await
    .into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "social.user-warning")
            .target(address)
            .detail(rest),
        answer,
    )
    .await
}

async fn rpc_for(
    state: &SharedState,
    session: &AdminSession,
    method: Method,
    leaf: &str,
) -> Result<
    (
        std::sync::Arc<crate::social::rpc::State>,
        axum::http::HeaderMap,
    ),
    Response,
> {
    let Some(rs) = state.domains.social_rpc.clone() else {
        return Err(not_configured("social-rpc"));
    };
    gate(state, &method, &format!("/admin/social/{leaf}")).await?;
    let Some(token) = configured(rs.settings.admin_token.as_deref()) else {
        return Err(token_missing("social-rpc"));
    };
    Ok((rs.clone(), service_headers(Some(token), session.address())))
}

/// `POST /admin/api/social-rpc/presence`: the connected wallets.
pub async fn rpc_presence(session: AdminSession, State(state): State<SharedState>) -> Response {
    let (rs, headers) = match rpc_for(&state, &session, Method::GET, "presence").await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let answer = rpc_admin::get_presence(State(rs), headers)
        .await
        .into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "social-rpc.presence"),
        answer,
    )
    .await
}

/// `POST /admin/api/social-rpc/voice-calls` `{limit?}`.
pub async fn rpc_voice_calls(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let (rs, headers) = match rpc_for(&state, &session, Method::GET, "voice-calls").await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let q = match query_from_obj::<rpc_admin::LimitQuery>(&body, &["limit"]) {
        Ok(q) => q,
        Err(resp) => return resp,
    };
    let answer = rpc_admin::get_voice_calls(State(rs), headers, Query(q))
        .await
        .into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "social-rpc.voice-calls").detail(body),
        answer,
    )
    .await
}

/// `POST /admin/api/social-rpc/friendships` `{address, limit?, offset?}`.
pub async fn rpc_friendships(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let address = match address_field(&body, "address", "address") {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    let (rs, headers) = match rpc_for(&state, &session, Method::GET, "friendships").await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let q = match query_from_obj::<rpc_admin::GraphQuery>(&body, &["limit", "offset"]) {
        Ok(q) => q,
        Err(resp) => return resp,
    };
    let answer = rpc_admin::get_friendships(State(rs), headers, Path(address.clone()), Query(q))
        .await
        .into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "social-rpc.friendships")
            .target(address)
            .detail(body),
        answer,
    )
    .await
}

/// `POST /admin/api/social-rpc/disconnect` `{address}`: open while social
/// writes are off (a socket kick, no durable row).
pub async fn rpc_disconnect(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let address = match address_field(&body, "address", "address") {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    let (rs, headers) = match rpc_for(&state, &session, Method::POST, "disconnect").await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let typed = match body_as::<rpc_admin::AddressBody>(json!({ "address": address })) {
        Ok(b) => b,
        Err(resp) => return resp,
    };
    let answer = rpc_admin::post_disconnect(State(rs), headers, Json(typed))
        .await
        .into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "social-rpc.disconnect")
            .target(address)
            .detail(body),
        answer,
    )
    .await
}

/// `POST /admin/api/social-rpc/force-presence` `{address, status}` (the old
/// form's `presence` field is accepted for `status`): open while social
/// writes are off (a broadcast frame, no durable row).
pub async fn rpc_force_presence(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let address = match address_field(&body, "address", "address") {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    let (rs, headers) = match rpc_for(&state, &session, Method::POST, "force-presence").await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let status = str_field(&body, "status")
        .or_else(|| str_field(&body, "presence"))
        .unwrap_or_default();
    let typed = match body_as::<rpc_admin::ForcePresenceBody>(
        json!({ "address": address, "status": status }),
    ) {
        Ok(b) => b,
        Err(resp) => return resp,
    };
    let answer = rpc_admin::post_force_presence(State(rs), headers, Json(typed))
        .await
        .into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "social-rpc.force-presence")
            .target(address)
            .detail(body),
        answer,
    )
    .await
}

/// `POST /admin/api/social-rpc/reset-settings` `{address}`: a durable write,
/// gated.
pub async fn rpc_reset_settings(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let address = match address_field(&body, "address", "address") {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    let (rs, headers) = match rpc_for(&state, &session, Method::POST, "reset-settings").await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let typed = match body_as::<rpc_admin::AddressBody>(json!({ "address": address })) {
        Ok(b) => b,
        Err(resp) => return resp,
    };
    let answer = rpc_admin::post_reset_settings(State(rs), headers, Json(typed))
        .await
        .into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "social-rpc.reset-settings")
            .target(address)
            .detail(body),
        answer,
    )
    .await
}

async fn communities_for(
    state: &SharedState,
    session: &AdminSession,
    method: Method,
    path: &str,
) -> Result<
    (
        crate::social::communities::SharedState,
        axum::http::HeaderMap,
    ),
    Response,
> {
    let Some(cs) = state.domains.communities.clone() else {
        return Err(not_configured(DOMAIN));
    };
    gate(state, &method, path).await?;
    let Some(token) = configured(cs.settings.admin_token.as_deref()) else {
        return Err(token_missing("communities"));
    };
    Ok((cs.clone(), service_headers(Some(token), session.address())))
}

/// `POST /admin/api/communities/list` `{status?, owner?, search?, limit?, offset?}`.
pub async fn communities_list(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let (cs, headers) =
        match communities_for(&state, &session, Method::GET, "/v1/admin/communities").await {
            Ok(v) => v,
            Err(resp) => return resp,
        };
    let pairs = match query_from_obj::<Vec<(String, String)>>(
        &body,
        &["status", "owner", "search", "limit", "offset"],
    ) {
        Ok(q) => q,
        Err(resp) => return resp,
    };
    let answer =
        crate::social::communities::admin::list_communities(State(cs), headers, Query(pairs)).await;
    finish(
        &state,
        Call::new(&session, DOMAIN, "communities.list").detail(body),
        answer,
    )
    .await
}

/// `POST /admin/api/communities/suspend` `{id, reason?}`.
pub async fn communities_suspend(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let id = match segment_field(&body, "id", "community-id") {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let (cs, headers) = match communities_for(
        &state,
        &session,
        Method::POST,
        &format!("/v1/admin/communities/{id}/suspend"),
    )
    .await
    {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let rest = body_without(&body, &["id"]);
    let reason = match rest
        .clone()
        .map(body_as::<crate::social::communities::admin::SuspendBody>)
        .transpose()
    {
        Ok(r) => r.map(Json),
        Err(resp) => return resp,
    };
    let answer = crate::social::communities::admin::suspend_community(
        State(cs),
        headers,
        Path(id.clone()),
        reason,
    )
    .await;
    finish(
        &state,
        Call::new(&session, DOMAIN, "communities.suspend")
            .target(id)
            .detail(rest.unwrap_or_else(|| json!({}))),
        answer,
    )
    .await
}

/// `POST /admin/api/communities/unsuspend` `{id}`.
pub async fn communities_unsuspend(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let id = match segment_field(&body, "id", "community-id") {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let (cs, headers) = match communities_for(
        &state,
        &session,
        Method::POST,
        &format!("/v1/admin/communities/{id}/unsuspend"),
    )
    .await
    {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let answer = crate::social::communities::admin::unsuspend_community(
        State(cs),
        headers,
        Path(id.clone()),
    )
    .await;
    finish(
        &state,
        Call::new(&session, DOMAIN, "communities.unsuspend")
            .target(id.clone())
            .detail(json!({ "id": id })),
        answer,
    )
    .await
}

/// `POST /admin/api/notifications/broadcast` `{type, metadata?, addresses?,
/// broadcastId?}`; `metadata` may arrive as a JSON string (a console
/// textarea) and `addresses` as a comma-separated string.
pub async fn notifications_broadcast(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let Some(ns) = state.domains.notifications.clone() else {
        return not_configured(DOMAIN);
    };
    if let Err(resp) = gate(&state, &Method::POST, "/notifications/broadcast").await {
        return resp;
    }
    let Some(token) = configured(ns.settings.admin_token.as_deref()) else {
        return token_missing("notifications");
    };
    let admin = match present::<RequireAdmin, _>(
        &ns,
        service_headers(Some(token), session.address()),
    )
    .await
    {
        Ok(a) => a,
        Err(rejection) => return envelope(rejection.into_response()).await,
    };
    let mut typed_body = body_without(&body, &[]).unwrap_or_else(|| json!({}));
    if let Some(raw) = typed_body.get("metadata").and_then(Value::as_str) {
        match serde_json::from_str::<Value>(raw) {
            Ok(v) => typed_body["metadata"] = v,
            Err(e) => return super::console::bad_body(format!("metadata is not JSON: {e}")),
        }
    }
    if let Some(raw) = typed_body.get("addresses").and_then(Value::as_str) {
        let list: Vec<Value> = raw
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| json!(s))
            .collect();
        typed_body["addresses"] = Value::Array(list);
    }
    let typed = match body_as::<crate::social::notifications::handlers::BroadcastBody>(typed_body) {
        Ok(b) => b,
        Err(resp) => return resp,
    };
    let answer =
        crate::social::notifications::handlers::post_broadcast(admin, State(ns), Json(typed))
            .await
            .into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "notifications.broadcast").detail(body),
        answer,
    )
    .await
}

async fn badges_for(
    state: &SharedState,
    session: &AdminSession,
    method: Method,
    path: &str,
) -> Result<(crate::social::badges::SharedState, RequireAdmin), Response> {
    let Some(bs) = state.domains.badges.clone() else {
        return Err(not_configured(DOMAIN));
    };
    gate(state, &method, path).await?;
    let Some(token) = configured(bs.settings.admin_token.as_deref()) else {
        return Err(token_missing("badges"));
    };
    match present::<RequireAdmin, _>(&bs, service_headers(Some(token), session.address())).await {
        Ok(admin) => Ok((bs.clone(), admin)),
        Err(rejection) => Err(envelope(rejection.into_response()).await),
    }
}

fn badge_target(body: &Value) -> Result<(String, String), Response> {
    Ok((
        address_field(body, "address", "address")?,
        segment_field(body, "badge_id", "badge-id")?,
    ))
}

/// `POST /admin/api/badges/grant` `{address, badge_id, tierId?}`.
pub async fn badges_grant(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let (address, badge_id) = match badge_target(&body) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let (bs, admin) = match badges_for(
        &state,
        &session,
        Method::POST,
        &format!("/users/{address}/badges/{badge_id}"),
    )
    .await
    {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let rest = body_without(&body, &["address", "badge_id"]);
    let grant = match rest
        .clone()
        .map(body_as::<crate::social::badges::handlers::GrantBody>)
        .transpose()
    {
        Ok(g) => g.map(Json),
        Err(resp) => return resp,
    };
    let answer = crate::social::badges::handlers::grant_user_badge(
        admin,
        State(bs),
        Path((address.clone(), badge_id.clone())),
        grant,
    )
    .await
    .into_response();
    let detail = json!({ "address": address, "badge_id": badge_id, "body": rest });
    finish(
        &state,
        Call::new(&session, DOMAIN, "badges.grant")
            .target(badge_id)
            .detail(detail),
        answer,
    )
    .await
}

/// `POST /admin/api/badges/revoke` `{address, badge_id}`.
pub async fn badges_revoke(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let (address, badge_id) = match badge_target(&body) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let (bs, admin) = match badges_for(
        &state,
        &session,
        Method::DELETE,
        &format!("/users/{address}/badges/{badge_id}"),
    )
    .await
    {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let answer = crate::social::badges::handlers::revoke_user_badge(
        admin,
        State(bs),
        Path((address.clone(), badge_id.clone())),
    )
    .await
    .into_response();
    let detail = json!({ "address": address, "badge_id": badge_id });
    finish(
        &state,
        Call::new(&session, DOMAIN, "badges.revoke")
            .target(badge_id)
            .detail(detail),
        answer,
    )
    .await
}
