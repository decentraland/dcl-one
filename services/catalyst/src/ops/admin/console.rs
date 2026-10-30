//! The console's calls into the other domains: what the old live node did
//! by proxying `/admin/api/<family>/<verb>` over loopback HTTP to each
//! service's admin route, done here as a call into the module that owns the
//! route, in this process (the old `catalyrst-live`).
//!
//! Every call runs the same four steps:
//!
//! 1. [`AdminSession`](super::auth::AdminSession) (the SIWE cookie, same
//!    origin, the live allowlist): no session, the old 403.
//! 2. The domain's own write switch, decided by the domain's own gate
//!    predicate on the route-local path the old proxy hit
//!    (`social::writes::is_gated`, `credits::writes::is_gated`, ...); a
//!    refused write answers the domain's own 503 body inside the envelope
//!    with its `Retry-After` and records nothing (it never reached the
//!    module, the `flush-cache` rule).
//! 3. The call: the domain's operation where it takes its actor as an
//!    argument (the credits ledger records the console wallet), otherwise
//!    the domain's handler function itself, presented with the credential
//!    the domain's own check expects: the bearer this node configured for
//!    that domain, exactly what the old proxy sent, plus the
//!    `x-catalyrst-admin` operator header the price and telemetry routes
//!    record. The handler's validation, response shape and own audit run
//!    unchanged; a domain whose bearer is unset answers the old
//!    `<domain>-admin-token-not-configured` 403 without being called.
//! 4. The answer wrapped in the old envelope `{ok, status, body}` with the
//!    domain's status, and one `ops.admin_audit` row: `actor` the session
//!    wallet, `domain` the owning module, `action` the old console action
//!    name, `result` `ok` or `error`, `detail` the request as received plus
//!    the domain's status (never a bearer: the credential is the node's,
//!    not the request's). The row is written after the call
//!    ([`audit::record_after`]): the module commits its own write, as the
//!    proxied service did.
//!
//! The asset-bundle registry's queues and denylist (`/admin/api/create/*`)
//! are the exception: abgen is a prebuilt external runtime with no source
//! in this tree (the old `catalyrst-ab-cdn`), so those calls go
//! to it over loopback HTTP as before ([`super::create`]).

#![allow(clippy::result_large_err)]

use std::sync::Arc;

use axum::Json;
use axum::body::to_bytes;
use axum::extract::{FromRequestParts, Query};
use axum::http::{HeaderMap, HeaderValue, Request, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::api::AdminEnvelope;
use super::auth::AdminSession;
use super::{SharedState, audit};

pub use super::create::Abgen;

/// The largest domain answer the console relays (a telemetry export or a
/// CRDT dump are the big ones); a larger one answers 502 instead.
pub(crate) const MAX_ANSWER_BYTES: usize = 32 << 20;

/// The module states the console calls into, cloned from `serve` before
/// they are mounted. `None` is a module this node does not run (the scene
/// host without `SCENE_HOST_ENABLED`, a test harness): its calls answer the
/// old unconfigured-service envelope.
#[derive(Clone, Default)]
pub struct Domains {
    pub telemetry: Option<crate::ops::telemetry::SharedState>,
    pub credits: Option<crate::shop::credits::SharedState>,
    pub price: Option<crate::price::SharedState>,
    pub comms: Option<crate::comms::gatekeeper::SharedState>,
    pub social_rpc: Option<Arc<crate::social::rpc::State>>,
    pub communities: Option<crate::social::communities::SharedState>,
    pub notifications: Option<crate::social::notifications::SharedState>,
    pub badges: Option<crate::social::badges::SharedState>,
    pub worlds: Option<crate::worlds::core::SharedState>,
    pub scene_host: Option<crate::worlds::scene_state::SharedState>,
    pub camera_reel: Option<crate::camera_reel::SharedState>,
    pub builder: Option<crate::builder::SharedState>,
    pub rpc: Option<crate::ops::rpc::Shared>,
    /// The explorer-api overlays (POIs, feature flags, the denylist).
    pub explorer_api: Option<crate::explore::api::Shared>,
    /// Places, worlds and events: the operator moderation.
    pub explore: Option<crate::explore::SharedState>,
    pub abgen: Option<Abgen>,
    /// `SOCIAL_WRITES_ENABLED`: the social routers take it as an argument
    /// rather than carrying it in their states.
    pub social_writes_enabled: bool,
    /// `WORLDS_WRITES_ENABLED`, for the same reason.
    pub worlds_writes_enabled: bool,
}

/// One console call as the audit row records it.
pub(crate) struct Call<'a> {
    pub actor: &'a str,
    pub domain: &'static str,
    pub action: &'static str,
    pub target: Option<String>,
    pub detail: Value,
}

impl<'a> Call<'a> {
    pub fn new(session: &'a AdminSession, domain: &'static str, action: &'static str) -> Self {
        Self {
            actor: session.address(),
            domain,
            action,
            target: None,
            detail: json!({}),
        }
    }

    pub fn target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }

    pub fn detail(mut self, detail: Value) -> Self {
        self.detail = detail;
        self
    }
}

/// The domain's answer in the console envelope, unaudited: the status and
/// `Retry-After` carried over, the body parsed as JSON (a non-JSON body is
/// kept as a string, as the old proxy did).
pub(crate) async fn envelope(answer: Response) -> Response {
    let (parts, body) = answer.into_parts();
    let Ok(bytes) = to_bytes(body, MAX_ANSWER_BYTES).await else {
        return AdminEnvelope::error(
            StatusCode::BAD_GATEWAY,
            json!({ "error": "answer-too-large", "maxBytes": MAX_ANSWER_BYTES }),
        );
    };
    let downstream = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()))
    };
    let status = parts.status;
    // The envelope is a body: a bodiless status (204 from an unban) travels
    // as 200 with the module's status inside.
    let carrier = if matches!(
        status,
        StatusCode::NO_CONTENT | StatusCode::RESET_CONTENT | StatusCode::NOT_MODIFIED
    ) {
        StatusCode::OK
    } else {
        status
    };
    let mut resp = (
        carrier,
        Json(AdminEnvelope {
            ok: status.is_success(),
            status: status.as_u16(),
            body: downstream,
        }),
    )
        .into_response();
    if let Some(retry) = parts.headers.get(header::RETRY_AFTER) {
        resp.headers_mut()
            .insert(header::RETRY_AFTER, retry.clone());
    }
    resp
}

/// [`envelope`] plus the call's audit row.
pub(crate) async fn finish(state: &SharedState, call: Call<'_>, answer: Response) -> Response {
    let status = answer.status();
    let resp = envelope(answer).await;
    let mut detail = call.detail;
    if let Some(obj) = detail.as_object_mut() {
        obj.insert("status".into(), json!(status.as_u16()));
    }
    let result = if status.is_success() { "ok" } else { "error" };
    let entry = audit::Entry::new(call.actor, call.domain, call.action)
        .target(call.target.as_deref())
        .detail(detail)
        .result(result);
    audit::record_after(&state.pool, &entry).await;
    resp
}

/// A write the domain's switch refuses: the domain's own 503, enveloped,
/// no audit row.
pub(crate) async fn refused(disabled: Response) -> Response {
    envelope(disabled).await
}

/// The old answer for a service the console has no URL for, here a module
/// this process does not run.
pub(crate) fn not_configured(service: &str) -> Response {
    AdminEnvelope::error(
        StatusCode::BAD_GATEWAY,
        json!({ "error": "not-configured", "service": service }),
    )
}

/// The old answer when the node holds no credential for the domain.
pub(crate) fn token_missing(key: &str) -> Response {
    credential_missing(&format!("{key}-admin-token-not-configured"))
}

/// The same 403 under the error name the old route used verbatim.
pub(crate) fn credential_missing(error: &str) -> Response {
    AdminEnvelope::error(StatusCode::FORBIDDEN, json!({ "error": error }))
}

pub(crate) fn bad_segment(what: &str) -> Response {
    AdminEnvelope::error(
        StatusCode::BAD_REQUEST,
        json!({ "error": format!("invalid-{what}") }),
    )
}

pub(crate) fn bad_body(detail: impl std::fmt::Display) -> Response {
    AdminEnvelope::error(
        StatusCode::BAD_REQUEST,
        json!({ "error": "invalid-body", "detail": detail.to_string() }),
    )
}

/// The configured credential, or `None` for an unset/blank one.
pub(crate) fn configured(token: Option<&str>) -> Option<&str> {
    token.map(str::trim).filter(|t| !t.is_empty())
}

/// The headers the old proxy sent: the domain's bearer, the operator's
/// wallet (`x-catalyrst-admin`, a checked address) and a JSON content type.
pub(crate) fn service_headers(token: Option<&str>, operator: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    if let Some(token) = token
        && let Ok(v) = HeaderValue::from_str(&format!("Bearer {token}"))
    {
        h.insert(header::AUTHORIZATION, v);
    }
    if super::auth::valid_eth_address(operator)
        && let Ok(v) = HeaderValue::from_str(operator)
    {
        h.insert("x-catalyrst-admin", v);
    }
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    h
}

/// Runs an extractor-typed credential check (`RequireAdmin`,
/// `TelemetryAdmin`) against `headers`, the way the router would.
pub(crate) async fn present<T, S>(state: &S, headers: HeaderMap) -> Result<T, T::Rejection>
where
    T: FromRequestParts<S>,
    S: Send + Sync,
{
    let mut parts = Request::new(()).into_parts().0;
    parts.headers = headers;
    T::from_request_parts(&mut parts, state).await
}

/// The same segment rule the old console applied before building a path.
pub(crate) fn valid_path_segment(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 256
        && s != "."
        && s != ".."
        && !s.contains("..")
        && s.chars().all(|c| {
            !c.is_control() && !c.is_whitespace() && !matches!(c, '/' | '\\' | '?' | '#' | '%')
        })
}

/// A lowercased, checked `0x` address from the body, or the 400.
pub(crate) fn address_field(body: &Value, key: &str, what: &str) -> Result<String, Response> {
    let raw = str_field(body, key).unwrap_or_default();
    if super::auth::valid_eth_address(raw.trim()) {
        Ok(raw.trim().to_lowercase())
    } else {
        Err(bad_segment(what))
    }
}

/// A path segment from the body, or the 400.
pub(crate) fn segment_field(body: &Value, key: &str, what: &str) -> Result<String, Response> {
    let raw = str_field(body, key).unwrap_or_default();
    if valid_path_segment(&raw) {
        Ok(raw)
    } else {
        Err(bad_segment(what))
    }
}

pub(crate) fn str_field(body: &Value, key: &str) -> Option<String> {
    body.get(key).and_then(Value::as_str).map(str::to_string)
}

/// A form's `"true"`/`"false"` or a JSON bool.
pub(crate) fn bool_field(body: &Value, key: &str) -> Option<bool> {
    match body.get(key)? {
        Value::Bool(b) => Some(*b),
        Value::String(s) => match s.trim() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

/// The body without `drop` and without the empty strings a console form
/// sends for a field left blank; `None` when nothing is left (the old
/// `body_without`).
pub(crate) fn body_without(body: &Value, drop: &[&str]) -> Option<Value> {
    let obj = body.as_object()?;
    let kept: serde_json::Map<String, Value> = obj
        .iter()
        .filter(|(k, v)| {
            !drop.contains(&k.as_str()) && !matches!(v, Value::String(s) if s.trim().is_empty())
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    (!kept.is_empty()).then_some(Value::Object(kept))
}

/// The query the old proxy built from `keys` of the body (non-empty
/// strings, numbers, booleans), parsed into the domain's query type the way
/// axum parses a request's, so a form's `"10"` reaches an integer field.
pub(crate) fn query_from_obj<T: DeserializeOwned>(
    body: &Value,
    keys: &[&str],
) -> Result<T, Response> {
    let mut qs = url::form_urlencoded::Serializer::new(String::new());
    if let Some(obj) = body.as_object() {
        for k in keys {
            match obj.get(*k) {
                Some(Value::String(s)) if !s.trim().is_empty() => {
                    qs.append_pair(k, s.trim());
                }
                Some(Value::Number(n)) => {
                    qs.append_pair(k, &n.to_string());
                }
                Some(Value::Bool(b)) => {
                    qs.append_pair(k, if *b { "true" } else { "false" });
                }
                _ => {}
            }
        }
    }
    let uri: axum::http::Uri = format!("/?{}", qs.finish()).parse().map_err(bad_body)?;
    Query::<T>::try_from_uri(&uri)
        .map(|Query(q)| q)
        .map_err(bad_body)
}

/// The body as the domain's JSON type, or the 400.
pub(crate) fn body_as<T: DeserializeOwned>(body: Value) -> Result<T, Response> {
    serde_json::from_value(body).map_err(bad_body)
}

/// The console's module calls at the old paths, every one `POST` (the
/// old console's forms post JSON; the reads take their filters in the body).
pub(super) fn routes() -> axum::Router<SharedState> {
    use axum::routing::post;
    axum::Router::new()
        .route(
            "/admin/api/telemetry/issue-state",
            post(super::telemetry::issue_state),
        )
        .route("/admin/api/telemetry/sql", post(super::telemetry::sql))
        .route("/admin/api/telemetry/purge", post(super::telemetry::purge))
        .route(
            "/admin/api/telemetry/ingest",
            post(super::telemetry::ingest),
        )
        .route("/admin/api/telemetry/quota", post(super::telemetry::quota))
        .route(
            "/admin/api/telemetry/bulk-delete",
            post(super::telemetry::bulk_delete),
        )
        .route(
            "/admin/api/telemetry/export",
            post(super::telemetry::export),
        )
        .route("/admin/api/telemetry/audit", post(super::telemetry::audit))
        .route(
            "/admin/api/telemetry/regroup",
            post(super::telemetry::regroup),
        )
        .route(
            "/admin/api/telemetry/release",
            post(super::telemetry::release),
        )
        .route(
            "/admin/api/credits/grant",
            post(super::economy::credits_grant),
        )
        .route(
            "/admin/api/credits/revoke",
            post(super::economy::credits_revoke),
        )
        .route(
            "/admin/api/credits/user-block",
            post(super::economy::credits_user_block),
        )
        .route(
            "/admin/api/price/override-set",
            post(super::economy::price_override_set),
        )
        .route(
            "/admin/api/price/override-delete",
            post(super::economy::price_override_delete),
        )
        .route("/admin/api/social/user-ban", post(super::social::user_ban))
        .route(
            "/admin/api/social/user-unban",
            post(super::social::user_unban),
        )
        .route(
            "/admin/api/social/user-warning",
            post(super::social::user_warning),
        )
        .route(
            "/admin/api/social-rpc/presence",
            post(super::social::rpc_presence),
        )
        .route(
            "/admin/api/social-rpc/voice-calls",
            post(super::social::rpc_voice_calls),
        )
        .route(
            "/admin/api/social-rpc/friendships",
            post(super::social::rpc_friendships),
        )
        .route(
            "/admin/api/social-rpc/disconnect",
            post(super::social::rpc_disconnect),
        )
        .route(
            "/admin/api/social-rpc/force-presence",
            post(super::social::rpc_force_presence),
        )
        .route(
            "/admin/api/social-rpc/reset-settings",
            post(super::social::rpc_reset_settings),
        )
        .route(
            "/admin/api/communities/list",
            post(super::social::communities_list),
        )
        .route(
            "/admin/api/communities/suspend",
            post(super::social::communities_suspend),
        )
        .route(
            "/admin/api/communities/unsuspend",
            post(super::social::communities_unsuspend),
        )
        .route(
            "/admin/api/notifications/broadcast",
            post(super::social::notifications_broadcast),
        )
        .route("/admin/api/badges/grant", post(super::social::badges_grant))
        .route(
            "/admin/api/badges/revoke",
            post(super::social::badges_revoke),
        )
        .route("/admin/api/worlds/list", post(super::worlds::list))
        .route("/admin/api/worlds/detail", post(super::worlds::detail))
        .route("/admin/api/worlds/enable", post(super::worlds::enable))
        .route("/admin/api/worlds/disable", post(super::worlds::disable))
        .route(
            "/admin/api/worlds/ban-status",
            post(super::worlds::ban_status),
        )
        .route(
            "/admin/api/worlds/blocked-list",
            post(super::worlds::blocked_list),
        )
        .route(
            "/admin/api/worlds/blocked-add",
            post(super::worlds::blocked_add),
        )
        .route(
            "/admin/api/worlds/blocked-remove",
            post(super::worlds::blocked_remove),
        )
        .route(
            "/admin/api/worlds/access-log",
            post(super::worlds::access_log),
        )
        .route("/admin/api/scene/reload", post(super::worlds::scene_reload))
        .route(
            "/admin/api/scene-state/crdt",
            post(super::worlds::scene_crdt),
        )
        .route(
            "/admin/api/scene-state/kick-all",
            post(super::worlds::scene_kick_all),
        )
        .route(
            "/admin/api/scene-state/reset",
            post(super::worlds::scene_reset),
        )
        .route(
            "/admin/api/camera-reel/image-delete",
            post(super::create::reel_image_delete),
        )
        .route(
            "/admin/api/camera-reel/image-review",
            post(super::create::reel_image_review),
        )
        .route(
            "/admin/api/builder/item-status",
            post(super::create::builder_item_status),
        )
        .route(
            "/admin/api/builder/items-status",
            post(super::create::builder_items_status),
        )
        .route(
            "/admin/api/create/registry-reingest",
            post(super::create::registry_reingest),
        )
        .route(
            "/admin/api/create/flush-ab-cache",
            post(super::create::flush_ab_cache),
        )
        .route(
            "/admin/api/create/queues-retry",
            post(super::create::queues_retry),
        )
        .route(
            "/admin/api/create/queues-pause",
            post(super::create::queues_pause),
        )
        .route(
            "/admin/api/create/queues-resume",
            post(super::create::queues_resume),
        )
        .route(
            "/admin/api/create/queues-status",
            post(super::create::queues_status),
        )
        .route(
            "/admin/api/create/denylist-add",
            post(super::create::denylist_add),
        )
        .route(
            "/admin/api/create/denylist-remove",
            post(super::create::denylist_remove),
        )
        .route("/admin/api/rpc/config", post(super::rpc::config))
        .route(
            "/admin/api/rpc/methods-list",
            post(super::rpc::methods_list),
        )
        .route("/admin/api/rpc/methods-add", post(super::rpc::methods_add))
        .route(
            "/admin/api/rpc/methods-remove",
            post(super::rpc::methods_remove),
        )
        .route(
            "/admin/api/rpc/methods-reset",
            post(super::rpc::methods_reset),
        )
        .route(
            "/admin/api/rpc/networks-list",
            post(super::rpc::networks_list),
        )
        .route(
            "/admin/api/rpc/networks-set",
            post(super::rpc::networks_set),
        )
        .route(
            "/admin/api/rpc/networks-delete",
            post(super::rpc::networks_delete),
        )
        .route(
            "/admin/api/places/pois-list",
            post(super::explore::pois_list),
        )
        .route(
            "/admin/api/places/poi-create",
            post(super::explore::poi_create),
        )
        .route(
            "/admin/api/places/poi-update",
            post(super::explore::poi_update),
        )
        .route(
            "/admin/api/places/poi-delete",
            post(super::explore::poi_delete),
        )
        .route(
            "/admin/api/explorer-api/flags-toggle",
            post(super::explore::flags_toggle),
        )
        .route(
            "/admin/api/explorer-api/flags-clear",
            post(super::explore::flags_clear),
        )
        .route(
            "/admin/api/explorer-api/flags-reload",
            post(super::explore::flags_reload),
        )
        .route(
            "/admin/api/explorer-api/blocklist-add",
            post(super::explore::blocklist_add),
        )
        .route(
            "/admin/api/explorer-api/blocklist-remove",
            post(super::explore::blocklist_remove),
        )
        .route(
            "/admin/api/explorer-api/blocklist-reload",
            post(super::explore::blocklist_reload),
        )
        .route(
            "/admin/api/explorer-api/challenges-list",
            post(super::explore::challenges_list),
        )
        .route(
            "/admin/api/explorer-api/challenge-get",
            post(super::explore::challenge_get),
        )
        .route(
            "/admin/api/explorer-api/challenge-revoke",
            post(super::explore::challenge_revoke),
        )
        .route(
            "/admin/api/explorer-api/identities-list",
            post(super::explore::identities_list),
        )
        .route(
            "/admin/api/explorer-api/identity-revoke",
            post(super::explore::identity_revoke),
        )
        .route(
            "/admin/api/explorer-api/config-list",
            post(super::explore::config_list),
        )
        .route(
            "/admin/api/explorer-api/config-get",
            post(super::explore::config_get),
        )
        .route(
            "/admin/api/explorer-api/config-set",
            post(super::explore::config_set),
        )
        .route(
            "/admin/api/explorer-api/config-delete",
            post(super::explore::config_delete),
        )
        .route(
            "/admin/api/places/reports",
            post(super::explore::places_reports),
        )
        .route(
            "/admin/api/places/report-resolve",
            post(super::explore::places_report_resolve),
        )
        .route(
            "/admin/api/places/place-disable",
            post(super::explore::place_disable),
        )
        .route(
            "/admin/api/places/place-highlight",
            post(super::explore::place_highlight),
        )
        .route(
            "/admin/api/places/place-rating",
            post(super::explore::place_rating),
        )
        .route(
            "/admin/api/places/world-highlight",
            post(super::explore::world_highlight),
        )
        .route(
            "/admin/api/places/world-rating",
            post(super::explore::world_rating),
        )
        .route(
            "/admin/api/events/moderate",
            post(super::explore::event_moderate),
        )
        .route(
            "/admin/api/events/create",
            post(super::explore::event_create),
        )
}

/// Every path [`routes`] mounts, for the gating tests.
pub const CONSOLE_ROUTES: &[&str] = &[
    "/admin/api/telemetry/issue-state",
    "/admin/api/telemetry/sql",
    "/admin/api/telemetry/purge",
    "/admin/api/telemetry/ingest",
    "/admin/api/telemetry/quota",
    "/admin/api/telemetry/bulk-delete",
    "/admin/api/telemetry/export",
    "/admin/api/telemetry/audit",
    "/admin/api/telemetry/regroup",
    "/admin/api/telemetry/release",
    "/admin/api/credits/grant",
    "/admin/api/credits/revoke",
    "/admin/api/credits/user-block",
    "/admin/api/price/override-set",
    "/admin/api/price/override-delete",
    "/admin/api/social/user-ban",
    "/admin/api/social/user-unban",
    "/admin/api/social/user-warning",
    "/admin/api/social-rpc/presence",
    "/admin/api/social-rpc/voice-calls",
    "/admin/api/social-rpc/friendships",
    "/admin/api/social-rpc/disconnect",
    "/admin/api/social-rpc/force-presence",
    "/admin/api/social-rpc/reset-settings",
    "/admin/api/communities/list",
    "/admin/api/communities/suspend",
    "/admin/api/communities/unsuspend",
    "/admin/api/notifications/broadcast",
    "/admin/api/badges/grant",
    "/admin/api/badges/revoke",
    "/admin/api/worlds/list",
    "/admin/api/worlds/detail",
    "/admin/api/worlds/enable",
    "/admin/api/worlds/disable",
    "/admin/api/worlds/ban-status",
    "/admin/api/worlds/blocked-list",
    "/admin/api/worlds/blocked-add",
    "/admin/api/worlds/blocked-remove",
    "/admin/api/worlds/access-log",
    "/admin/api/scene/reload",
    "/admin/api/scene-state/crdt",
    "/admin/api/scene-state/kick-all",
    "/admin/api/scene-state/reset",
    "/admin/api/camera-reel/image-delete",
    "/admin/api/camera-reel/image-review",
    "/admin/api/builder/item-status",
    "/admin/api/builder/items-status",
    "/admin/api/create/registry-reingest",
    "/admin/api/create/flush-ab-cache",
    "/admin/api/create/queues-retry",
    "/admin/api/create/queues-pause",
    "/admin/api/create/queues-resume",
    "/admin/api/create/queues-status",
    "/admin/api/create/denylist-add",
    "/admin/api/create/denylist-remove",
    "/admin/api/rpc/config",
    "/admin/api/rpc/methods-list",
    "/admin/api/rpc/methods-add",
    "/admin/api/rpc/methods-remove",
    "/admin/api/rpc/methods-reset",
    "/admin/api/rpc/networks-list",
    "/admin/api/rpc/networks-set",
    "/admin/api/rpc/networks-delete",
    "/admin/api/places/pois-list",
    "/admin/api/places/poi-create",
    "/admin/api/places/poi-update",
    "/admin/api/places/poi-delete",
    "/admin/api/explorer-api/flags-toggle",
    "/admin/api/explorer-api/flags-clear",
    "/admin/api/explorer-api/flags-reload",
    "/admin/api/explorer-api/blocklist-add",
    "/admin/api/explorer-api/blocklist-remove",
    "/admin/api/explorer-api/blocklist-reload",
    "/admin/api/explorer-api/challenges-list",
    "/admin/api/explorer-api/challenge-get",
    "/admin/api/explorer-api/challenge-revoke",
    "/admin/api/explorer-api/identities-list",
    "/admin/api/explorer-api/identity-revoke",
    "/admin/api/explorer-api/config-list",
    "/admin/api/explorer-api/config-get",
    "/admin/api/explorer-api/config-set",
    "/admin/api/explorer-api/config-delete",
    "/admin/api/places/reports",
    "/admin/api/places/report-resolve",
    "/admin/api/places/place-disable",
    "/admin/api/places/place-highlight",
    "/admin/api/places/place-rating",
    "/admin/api/places/world-highlight",
    "/admin/api/places/world-rating",
    "/admin/api/events/moderate",
    "/admin/api/events/create",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Deserialize, Debug, PartialEq)]
    struct Page {
        limit: Option<i64>,
        status: Option<String>,
        on: Option<bool>,
    }

    #[test]
    fn the_query_keeps_the_named_non_empty_fields_and_parses_form_strings() {
        let body =
            json!({ "limit": "10", "status": " open ", "on": true, "other": "x", "empty": "" });
        let q: Page = query_from_obj(&body, &["limit", "status", "on", "empty"]).unwrap();
        assert_eq!(
            q,
            Page {
                limit: Some(10),
                status: Some("open".into()),
                on: Some(true)
            }
        );
        let q: Page = query_from_obj(&json!({ "limit": 5 }), &["limit"]).unwrap();
        assert_eq!(q.limit, Some(5));
        assert!(query_from_obj::<Page>(&json!({ "limit": "ten" }), &["limit"]).is_err());
    }

    #[test]
    fn the_old_segment_and_body_rules_hold() {
        for bad in ["", ".", "..", "a/b", "a..b", "a b", "a?b", "a%2f", "x#y"] {
            assert!(!valid_path_segment(bad), "{bad:?}");
        }
        assert!(valid_path_segment("bafy-ok_1.2"));
        assert_eq!(
            body_without(&json!({ "id": "x", "reason": "r", "blank": " " }), &["id"]),
            Some(json!({ "reason": "r" }))
        );
        assert_eq!(body_without(&json!({ "id": "x" }), &["id"]), None);
        assert_eq!(bool_field(&json!({ "b": "false" }), "b"), Some(false));
        assert_eq!(bool_field(&json!({ "b": true }), "b"), Some(true));
        assert_eq!(bool_field(&json!({ "b": "maybe" }), "b"), None);
    }

    #[test]
    fn the_service_headers_carry_the_bearer_and_a_checked_operator_only() {
        let h = service_headers(Some("tok"), "0x00000000000000000000000000000000000000aa");
        assert_eq!(h.get(header::AUTHORIZATION).unwrap(), "Bearer tok");
        assert_eq!(
            h.get("x-catalyrst-admin").unwrap(),
            "0x00000000000000000000000000000000000000aa"
        );
        let h = service_headers(None, "not-an-address");
        assert!(h.get(header::AUTHORIZATION).is_none() && h.get("x-catalyrst-admin").is_none());
        assert_eq!(configured(Some("  ")), None);
        assert_eq!(configured(Some(" t ")), Some("t"));
    }

    #[tokio::test]
    async fn the_envelope_keeps_the_domain_status_body_and_retry_after() {
        let answer = (
            StatusCode::SERVICE_UNAVAILABLE,
            [(header::RETRY_AFTER, "60")],
            Json(json!({ "message": "off" })),
        )
            .into_response();
        let resp = envelope(answer).await;
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(resp.headers().get(header::RETRY_AFTER).unwrap(), "60");
        let body: Value =
            serde_json::from_slice(&to_bytes(resp.into_body(), 1 << 16).await.unwrap()).unwrap();
        assert_eq!(
            body,
            json!({ "ok": false, "status": 503, "body": { "message": "off" } })
        );
        let resp = envelope(StatusCode::NO_CONTENT.into_response()).await;
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "a 204 carries its envelope as 200"
        );
        let body: Value =
            serde_json::from_slice(&to_bytes(resp.into_body(), 1 << 16).await.unwrap()).unwrap();
        assert_eq!(body, json!({ "ok": true, "status": 204, "body": null }));
        let resp = envelope((StatusCode::OK, "plain").into_response()).await;
        let body: Value =
            serde_json::from_slice(&to_bytes(resp.into_body(), 1 << 16).await.unwrap()).unwrap();
        assert_eq!(body, json!({ "ok": true, "status": 200, "body": "plain" }));
    }
}
