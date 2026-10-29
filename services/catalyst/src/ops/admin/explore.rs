//! `/admin/api/places/poi-*` and `/admin/api/explorer-api/{flags,blocklist}-*`:
//! the operator overlays `explore::api` keeps over its mirrors, edited as
//! module calls.
//!
//! - Points of interest: the old places service kept a POI table the
//!   `dcl-lists` `/pois` route served; v2 serves that list from the mirror
//!   plus `explore_api.lists_local` (`explore::api::lists`), so a POI create
//!   or enable adds the parcel (`present = true`), a disable or delete hides it
//!   (`present = false`), and the served set is the module's own
//!   `lists::values`. A POI is its parcel (`x,y`): the wire carries nothing
//!   else, so the old row's title, description and entity id have nowhere to
//!   be served and are not kept.
//! - Feature flags: a toggle is an operator override in
//!   `explore_api.feature_flags_local` (`feature_flags::set_local_flag`) that
//!   every mirror pass re-applies; `flags-clear` drops it so the upstream value
//!   shows again; `flags-reload` rebuilds the served document from the
//!   mirrored row and the overrides (the old service re-read its file).
//! - The blocklist: the old explorer-api's blocklist was the served
//!   `denylist.json`; here the mirrored denylist plus
//!   `explore_api.denylist_users_local` (`denylist::set_local`): add puts a
//!   wallet on it, remove hides it (mirrored or local), reload rebuilds the
//!   served snapshot.
//!
//! - Place reports (`places/{reports,report-resolve}`): the reports page and
//!   the settling of one report (`explore::places::reports`), the calls the
//!   old console proxied to `/api/reports[/{id}]` with the places token.
//! - Place and world moderation (`places/{place-disable,place-highlight,
//!   place-rating,world-highlight,world-rating}`): the decision recorded in
//!   `explore.place_moderation` and projected onto the row
//!   (`explore::places::moderation`), which the places mirror re-applies.
//! - Event moderation and creation (`events/{moderate,create}`): the old admin
//!   patch rules, and the old admin create (pending approval, created by
//!   `admin`), each with its `events_local` audit
//!   (`explore::events::moderation`). On a mirrored event the decision is
//!   recorded in `explore.event_moderation` and projected onto the row, which
//!   the events mirror re-applies; a patch of its name or description answers
//!   409 naming the upstream, whose fields every pass writes back.
//!
//! The overrides are local authority an operator owns, so the writes are
//! behind `OPS_WRITES_ENABLED` (the console's own switch); the listings and
//! the reloads (a rebuild of in-memory state from rows already stored) are
//! open. The explorer-api `config-*` (the runtime key/value map, process
//! state no route reads) and `challenges`/`identities` (the `/auth-api`
//! sign-in relay's pending requests and parked identities, process state)
//! are module calls too; like the relay's allowlist edits they change no
//! user-authoritative row, so they are audited and not gated by
//! `OPS_WRITES_ENABLED`.

#![allow(clippy::result_large_err)]

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use super::SharedState;
use super::api::ops_refused;
use super::auth::AdminSession;
use super::console::{
    Call, address_field, body_without, bool_field, finish, not_configured, query_from_obj,
    str_field, valid_path_segment,
};
use crate::explore::api::{denylist, feature_flags, lists};
use crate::explore::events::moderation::{self as event_moderation, AdminPatch, CreateEvent};
use crate::explore::places::moderation::{self as place_moderation, Decision, Target};
use crate::explore::places::reports::{self, ReportPatch, ReportQuery};

const DOMAIN: &str = "explore";
const POIS: &str = "pois";

fn api_for(state: &SharedState) -> Result<crate::explore::api::Shared, Response> {
    state
        .domains
        .explorer_api
        .clone()
        .ok_or_else(|| not_configured("explorer-api"))
}

fn internal(e: impl std::fmt::Display) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": e.to_string() })),
    )
        .into_response()
}

/// A parcel `x,y` (two integers), normalized without spaces; the dcl-lists
/// `/pois` wire is exactly that string.
fn parcel(raw: &str) -> Option<String> {
    let (x, y) = raw.trim().split_once(',')?;
    let (x, y) = (x.trim().parse::<i32>().ok()?, y.trim().parse::<i32>().ok()?);
    ((-1024..=1024).contains(&x) && (-1024..=1024).contains(&y)).then(|| format!("{x},{y}"))
}

fn position(body: &Value) -> Result<String, Response> {
    str_field(body, "position")
        .and_then(|p| parcel(&p))
        .ok_or_else(|| super::console::bad_segment("position"))
}

/// `POST /admin/api/places/pois-list`: the served POI set.
pub async fn pois_list(session: AdminSession, State(state): State<SharedState>) -> Response {
    let api = match api_for(&state) {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    let answer = match lists::values(&api.pool, POIS).await {
        Ok(values) => Json(json!({ "ok": true, "data": values })).into_response(),
        Err(e) => internal(e),
    };
    finish(
        &state,
        Call::new(&session, DOMAIN, "places.pois.list"),
        answer,
    )
    .await
}

async fn set_poi(
    state: &SharedState,
    session: &AdminSession,
    action: &'static str,
    pos: String,
    present: bool,
    detail: Value,
) -> Response {
    let api = match api_for(state) {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    if let Some(refused) = ops_refused(state.settings.writes_enabled) {
        return refused;
    }
    let answer = match lists::set_local(&api.pool, POIS, &pos, present).await {
        Ok(()) => Json(json!({ "ok": true, "data": { "position": pos, "present": present } }))
            .into_response(),
        Err(e) => internal(e),
    };
    finish(
        state,
        Call::new(session, DOMAIN, action)
            .target(pos)
            .detail(detail),
        answer,
    )
    .await
}

/// `POST /admin/api/places/poi-create` `{position}`: served from now on.
pub async fn poi_create(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let pos = match position(&body) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    set_poi(
        &state,
        &session,
        "places.poi.create",
        pos.clone(),
        true,
        json!({ "position": pos }),
    )
    .await
}

/// `POST /admin/api/places/poi-update` `{position, enabled}`.
pub async fn poi_update(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let pos = match position(&body) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    let Some(enabled) = bool_field(&body, "enabled") else {
        return super::console::bad_body("enabled must be true or false");
    };
    set_poi(
        &state,
        &session,
        "places.poi.update",
        pos.clone(),
        enabled,
        json!({ "position": pos, "enabled": enabled }),
    )
    .await
}

/// `POST /admin/api/places/poi-delete` `{position}`: hidden, mirrored or not.
pub async fn poi_delete(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let pos = match position(&body) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    set_poi(
        &state,
        &session,
        "places.poi.delete",
        pos.clone(),
        false,
        json!({ "position": pos }),
    )
    .await
}

fn flag_name(body: &Value) -> Result<String, Response> {
    let name = str_field(body, "name")
        .map(|n| n.trim().to_string())
        .unwrap_or_default();
    if name.is_empty()
        || name.len() > 200
        || name.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        return Err(super::console::bad_segment("flag-name"));
    }
    Ok(name)
}

async fn reload_flags(api: &crate::explore::api::Shared) -> sqlx::Result<()> {
    api.flags
        .reload(&api.pool, &api.settings.feature_flags_app)
        .await
}

/// `POST /admin/api/explorer-api/flags-toggle` `{name, value?}` (`value`,
/// or the console form's `enabled`, defaults to true as the old route's did).
pub async fn flags_toggle(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let name = match flag_name(&body) {
        Ok(n) => n,
        Err(resp) => return resp,
    };
    let api = match api_for(&state) {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    if let Some(refused) = ops_refused(state.settings.writes_enabled) {
        return refused;
    }
    let value = bool_field(&body, "value")
        .or_else(|| bool_field(&body, "enabled"))
        .unwrap_or(true);
    let app = api.settings.feature_flags_app.clone();
    let answer = match async {
        feature_flags::set_local_flag(&api.pool, &app, &name, value).await?;
        reload_flags(&api).await
    }
    .await
    {
        Ok(()) => Json(json!({ "ok": true, "name": name, "value": value })).into_response(),
        Err(e) => internal(e),
    };
    finish(
        &state,
        Call::new(&session, DOMAIN, "explorer-api.flags.toggle")
            .target(name.clone())
            .detail(json!({ "app": app, "name": name, "value": value })),
        answer,
    )
    .await
}

/// `POST /admin/api/explorer-api/flags-clear` `{name}`: the override goes,
/// the upstream value shows again.
pub async fn flags_clear(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let name = match flag_name(&body) {
        Ok(n) => n,
        Err(resp) => return resp,
    };
    let api = match api_for(&state) {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    if let Some(refused) = ops_refused(state.settings.writes_enabled) {
        return refused;
    }
    let app = api.settings.feature_flags_app.clone();
    let answer = match async {
        feature_flags::clear_local_flag(&api.pool, &app, &name).await?;
        reload_flags(&api).await
    }
    .await
    {
        Ok(()) => Json(json!({ "ok": true, "name": name, "cleared": true })).into_response(),
        Err(e) => internal(e),
    };
    finish(
        &state,
        Call::new(&session, DOMAIN, "explorer-api.flags.clear")
            .target(name.clone())
            .detail(json!({ "app": app, "name": name })),
        answer,
    )
    .await
}

/// `POST /admin/api/explorer-api/flags-reload`: the served document rebuilt
/// from the mirrored row and the overrides.
pub async fn flags_reload(session: AdminSession, State(state): State<SharedState>) -> Response {
    let api = match api_for(&state) {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    let answer = match reload_flags(&api).await {
        Ok(()) => {
            let flags = api
                .flags
                .document()
                .get("flags")
                .and_then(Value::as_object)
                .map_or(0, |f| f.len());
            Json(json!({ "ok": true, "app": api.settings.feature_flags_app, "flags": flags }))
                .into_response()
        }
        Err(e) => internal(e),
    };
    finish(
        &state,
        Call::new(&session, DOMAIN, "explorer-api.flags.reload"),
        answer,
    )
    .await
}

async fn set_blocked(
    state: &SharedState,
    session: &AdminSession,
    action: &'static str,
    body: &Value,
    present: bool,
) -> Response {
    let wallet = match address_field(body, "wallet", "wallet") {
        Ok(w) => w,
        Err(resp) => return resp,
    };
    let api = match api_for(state) {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    if let Some(refused) = ops_refused(state.settings.writes_enabled) {
        return refused;
    }
    let answer = match async {
        let before = api.denylist.contains(&wallet);
        denylist::set_local(&api.pool, &wallet, present).await?;
        api.denylist.reload(&api.pool).await?;
        Ok::<_, sqlx::Error>(before)
    }
    .await
    {
        Ok(before) => {
            let count = api.denylist.list().users.len();
            let changed = before != present;
            let mut answer = serde_json::Map::new();
            answer.insert("ok".into(), json!(true));
            answer.insert("wallet".into(), json!(wallet));
            answer.insert(
                if present { "added" } else { "removed" }.into(),
                json!(changed),
            );
            answer.insert("count".into(), json!(count));
            Json(Value::Object(answer)).into_response()
        }
        Err(e) => internal(e),
    };
    finish(
        state,
        Call::new(session, DOMAIN, action)
            .target(wallet.clone())
            .detail(json!({ "wallet": wallet })),
        answer,
    )
    .await
}

/// `POST /admin/api/explorer-api/blocklist-add` `{wallet}`.
pub async fn blocklist_add(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    set_blocked(&state, &session, "explorer-api.blocklist.add", &body, true).await
}

/// `POST /admin/api/explorer-api/blocklist-remove` `{wallet}`.
pub async fn blocklist_remove(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    set_blocked(
        &state,
        &session,
        "explorer-api.blocklist.remove",
        &body,
        false,
    )
    .await
}

/// `POST /admin/api/explorer-api/blocklist-reload`.
pub async fn blocklist_reload(session: AdminSession, State(state): State<SharedState>) -> Response {
    let api = match api_for(&state) {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    let answer = match api.denylist.reload(&api.pool).await {
        Ok(()) => {
            Json(json!({ "ok": true, "count": api.denylist.list().users.len() })).into_response()
        }
        Err(e) => internal(e),
    };
    finish(
        &state,
        Call::new(&session, DOMAIN, "explorer-api.blocklist.reload"),
        answer,
    )
    .await
}

fn explore_for(state: &SharedState) -> Result<crate::explore::SharedState, Response> {
    let explore = state
        .domains
        .explore
        .clone()
        .ok_or_else(|| not_configured("explore"))?;
    if let Some(refused) = ops_refused(state.settings.writes_enabled) {
        return Err(refused);
    }
    Ok(explore)
}

fn row_id(body: &Value, key: &str, what: &str) -> Result<String, Response> {
    super::console::segment_field(body, key, what)
}

/// One place or world decision: the overlay and its projection, then the row
/// as the read routes serve it (the disable keeps the old `{id, disabled}`).
async fn moderate_place(
    state: &SharedState,
    session: &AdminSession,
    action: &'static str,
    key: &str,
    decision: Result<Decision, Response>,
    body: &Value,
) -> Response {
    let world = key == "world_id";
    let id = match row_id(body, key, if world { "world-id" } else { "place-id" }) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let decision = match decision {
        Ok(d) => d,
        Err(resp) => return resp,
    };
    let explore = match explore_for(state) {
        Ok(e) => e,
        Err(resp) => return resp,
    };
    let target = if world {
        Target::World(&id)
    } else {
        Target::Place(&id)
    };
    let answer =
        match place_moderation::apply(&explore.pool, target.clone(), &decision, session.address())
            .await
        {
            Ok(None) => crate::shared::errors::ApiError::not_found(format!(
                "Not found {} \"{id}\"",
                if world { "world" } else { "place" }
            ))
            .into_response(),
            Ok(Some(row)) => match &decision {
                Decision::Disabled { disabled, .. } => {
                    Json(json!({ "ok": true, "data": { "id": row, "disabled": disabled } }))
                        .into_response()
                }
                _ => match place_moderation::answer(&explore.places, &target, &row).await {
                    Ok(place) => Json(json!({ "ok": true, "data": place })).into_response(),
                    Err(e) => e.into_response(),
                },
            },
            Err(e) => e.into_response(),
        };
    let detail = super::console::body_without(body, &[]).unwrap_or_else(|| json!({}));
    finish(
        state,
        Call::new(session, DOMAIN, action).target(id).detail(detail),
        answer,
    )
    .await
}

fn rating(body: &Value) -> Result<Decision, Response> {
    let raw = str_field(body, "content_rating")
        .or_else(|| str_field(body, "rating"))
        .unwrap_or_default();
    Decision::rating(&raw).map_err(IntoResponse::into_response)
}

fn highlighted(body: &Value) -> Result<Decision, Response> {
    bool_field(body, "highlighted")
        .map(Decision::Highlighted)
        .ok_or_else(|| {
            crate::shared::errors::ApiError::bad_request(
                "Invalid highlight body. Expected { highlighted: boolean }.",
            )
            .into_response()
        })
}

/// `POST /admin/api/places/place-disable` `{place_id, disabled?, reason?}`
/// (`disabled` defaults to true, as the old route's did).
pub async fn place_disable(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let decision = Ok(Decision::Disabled {
        disabled: bool_field(&body, "disabled").unwrap_or(true),
        reason: str_field(&body, "reason").filter(|r| !r.trim().is_empty()),
    });
    moderate_place(
        &state,
        &session,
        "places.place.disable",
        "place_id",
        decision,
        &body,
    )
    .await
}

/// `POST /admin/api/places/reports` `{status?, entity_id?, limit?, offset?}`:
/// the reports page, newest first (a read: open with writes off).
pub async fn places_reports(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let Some(explore) = state.domains.explore.clone() else {
        return not_configured("explore");
    };
    let q: ReportQuery = match query_from_obj(&body, &["status", "entity_id", "limit", "offset"]) {
        Ok(q) => q,
        Err(resp) => return resp,
    };
    let answer = match reports::list_reports(&explore.pool, &q).await {
        Ok((rows, total)) => {
            Json(crate::shared::errors::ApiDataTotal::ok(rows, total)).into_response()
        }
        Err(e) => internal(e),
    };
    finish(
        &state,
        Call::new(&session, DOMAIN, "places.reports.list").detail(body),
        answer,
    )
    .await
}

/// `POST /admin/api/places/report-resolve` `{id, status, resolution?, notes?,
/// resolved_by?}`: the old `PATCH /api/reports/{id}` as a module call.
pub async fn places_report_resolve(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let Some(explore) = state.domains.explore.clone() else {
        return not_configured("explore");
    };
    let raw = str_field(&body, "id").unwrap_or_default();
    let Some(id) = valid_path_segment(&raw)
        .then(|| raw.parse::<i64>().ok())
        .flatten()
    else {
        return super::console::bad_segment("report-id");
    };
    let extra = body_without(&body, &["id"]).unwrap_or_else(|| json!({}));
    let patch: ReportPatch = match serde_json::from_value(extra.clone()) {
        Ok(p) => p,
        Err(e) => return super::console::bad_body(e),
    };
    if let Some(refused) = ops_refused(state.settings.writes_enabled) {
        return refused;
    }
    let answer = match reports::resolve_report(&explore.pool, id, &patch).await {
        Ok(row) => Json(crate::shared::errors::ApiData::ok(row)).into_response(),
        Err(e) => e.into_response(),
    };
    finish(
        &state,
        Call::new(&session, DOMAIN, "places.report.resolve")
            .target(raw)
            .detail(extra),
        answer,
    )
    .await
}

/// `POST /admin/api/places/place-highlight` `{place_id, highlighted}`.
pub async fn place_highlight(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let decision = highlighted(&body);
    moderate_place(
        &state,
        &session,
        "places.place.highlight",
        "place_id",
        decision,
        &body,
    )
    .await
}

/// `POST /admin/api/places/place-rating` `{place_id, content_rating}`.
pub async fn place_rating(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let decision = rating(&body);
    moderate_place(
        &state,
        &session,
        "places.place.rating",
        "place_id",
        decision,
        &body,
    )
    .await
}

/// `POST /admin/api/places/world-highlight` `{world_id, highlighted}` (the
/// catalogue id or the world name).
pub async fn world_highlight(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let decision = highlighted(&body);
    moderate_place(
        &state,
        &session,
        "places.world.highlight",
        "world_id",
        decision,
        &body,
    )
    .await
}

/// `POST /admin/api/places/world-rating` `{world_id, content_rating}`.
pub async fn world_rating(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let decision = rating(&body);
    moderate_place(
        &state,
        &session,
        "places.world.rating",
        "world_id",
        decision,
        &body,
    )
    .await
}

/// `POST /admin/api/events/moderate` `{event_id, action?, approved?,
/// rejected?, highlighted?, trending?, name?, description?, reason?}`: the old
/// admin patch, the operator its actor; the answer is the event as served.
pub async fn event_moderate(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let id = match row_id(&body, "event_id", "event-id") {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let mut patch_body =
        super::console::body_without(&body, &["event_id"]).unwrap_or_else(|| json!({}));
    for key in ["approved", "rejected", "highlighted", "trending"] {
        if let Some(b) = bool_field(&patch_body, key) {
            patch_body[key] = json!(b);
        }
    }
    let patch = match super::console::body_as::<AdminPatch>(patch_body.clone()) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    let explore = match explore_for(&state) {
        Ok(e) => e,
        Err(resp) => return resp,
    };
    let answer =
        match event_moderation::moderate(&explore.pool, &id, &patch, session.address()).await {
            Ok(false) => {
                crate::shared::errors::ApiError::not_found(format!("Not found event \"{id}\""))
                    .into_response()
            }
            Ok(true) => match explore.events.get(&id).await {
                Ok(event) => Json(json!({ "ok": true, "data": event })).into_response(),
                Err(e) => e.into_response(),
            },
            Err(e) => e.into_response(),
        };
    finish(
        &state,
        Call::new(&session, DOMAIN, "events.moderate")
            .target(id)
            .detail(patch_body),
        answer,
    )
    .await
}

/// `POST /admin/api/events/create`: the old admin create body (unknown
/// fields refused; a console form's numeric and boolean strings are read as
/// such); the answer is the event as served, pending approval.
pub async fn event_create(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let mut create_body = super::console::body_without(&body, &[]).unwrap_or_else(|| json!({}));
    if let Some(obj) = create_body.as_object_mut() {
        for key in [
            "x",
            "y",
            "duration",
            "recurrent_setpos",
            "recurrent_monthday",
            "recurrent_weekday_mask",
            "recurrent_month_mask",
            "recurrent_interval",
            "recurrent_count",
        ] {
            if let Some(n) = obj
                .get(key)
                .and_then(Value::as_str)
                .and_then(|v| v.trim().parse::<i64>().ok())
            {
                obj.insert(key.into(), json!(n));
            }
        }
        for key in ["world", "all_day", "recurrent"] {
            if let Some(b) = bool_field(&Value::Object(obj.clone()), key) {
                obj.insert(key.into(), json!(b));
            }
        }
        for key in ["categories", "schedules"] {
            if let Some(list) = obj.get(key).and_then(Value::as_str).map(|v| {
                v.split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(|s| json!(s))
                    .collect::<Vec<_>>()
            }) {
                obj.insert(key.into(), Value::Array(list));
            }
        }
    }
    let typed = match super::console::body_as::<CreateEvent>(create_body.clone()) {
        Ok(b) => b,
        Err(resp) => return resp,
    };
    let explore = match explore_for(&state) {
        Ok(e) => e,
        Err(resp) => return resp,
    };
    let (answer, id) =
        match event_moderation::create(&explore.pool, &typed, session.address()).await {
            Ok(id) => match explore.events.get(&id).await {
                Ok(event) => (
                    Json(json!({ "ok": true, "data": event })).into_response(),
                    Some(id),
                ),
                Err(e) => (e.into_response(), Some(id)),
            },
            Err(e) => (e.into_response(), None),
        };
    let mut call = Call::new(&session, DOMAIN, "events.create").detail(create_body);
    if let Some(id) = id {
        call = call.target(id);
    }
    finish(&state, call, answer).await
}

fn id_field(body: &Value) -> Result<String, Response> {
    str_field(body, "id")
        .filter(|id| !id.is_empty() && id.len() <= 64)
        .ok_or_else(|| super::console::bad_segment("id"))
}

fn key_field(body: &Value) -> Result<String, Response> {
    str_field(body, "key")
        .filter(|k| {
            !k.is_empty()
                && k.len() <= 128
                && k.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
        })
        .ok_or_else(|| super::console::bad_segment("config-key"))
}

fn with_status((status, body): (StatusCode, Value)) -> Response {
    (status, Json(body)).into_response()
}

/// `POST /admin/api/explorer-api/challenges-list`: the pending sign-in requests.
pub async fn challenges_list(session: AdminSession, State(state): State<SharedState>) -> Response {
    let api = match api_for(&state) {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    let answer = Json(crate::explore::api::auth_api::admin_list_challenges(&api)).into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "explorer-api.challenges.list"),
        answer,
    )
    .await
}

/// `POST /admin/api/explorer-api/challenge-get` `{id}`.
pub async fn challenge_get(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let (api, id) = match (api_for(&state), id_field(&body)) {
        (Ok(a), Ok(id)) => (a, id),
        (Err(resp), _) | (_, Err(resp)) => return resp,
    };
    let answer = with_status(crate::explore::api::auth_api::admin_get_challenge(
        &api, &id,
    ));
    finish(
        &state,
        Call::new(&session, DOMAIN, "explorer-api.challenge.get")
            .target(id.clone())
            .detail(json!({ "id": id })),
        answer,
    )
    .await
}

/// `POST /admin/api/explorer-api/challenge-revoke` `{id}`.
pub async fn challenge_revoke(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let (api, id) = match (api_for(&state), id_field(&body)) {
        (Ok(a), Ok(id)) => (a, id),
        (Err(resp), _) | (_, Err(resp)) => return resp,
    };
    let answer = with_status(crate::explore::api::auth_api::admin_revoke_challenge(
        &api, &id,
    ));
    finish(
        &state,
        Call::new(&session, DOMAIN, "explorer-api.challenge.revoke")
            .target(id.clone())
            .detail(json!({ "id": id })),
        answer,
    )
    .await
}

/// `POST /admin/api/explorer-api/identities-list`: the parked identities.
pub async fn identities_list(session: AdminSession, State(state): State<SharedState>) -> Response {
    let api = match api_for(&state) {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    let answer = Json(crate::explore::api::auth_api::admin_list_identities(&api)).into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "explorer-api.identities.list"),
        answer,
    )
    .await
}

/// `POST /admin/api/explorer-api/identity-revoke` `{id}`.
pub async fn identity_revoke(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let (api, id) = match (api_for(&state), id_field(&body)) {
        (Ok(a), Ok(id)) => (a, id),
        (Err(resp), _) | (_, Err(resp)) => return resp,
    };
    let answer = with_status(crate::explore::api::auth_api::admin_revoke_identity(
        &api, &id,
    ));
    finish(
        &state,
        Call::new(&session, DOMAIN, "explorer-api.identity.revoke")
            .target(id.clone())
            .detail(json!({ "id": id })),
        answer,
    )
    .await
}

/// `POST /admin/api/explorer-api/config-list`.
pub async fn config_list(session: AdminSession, State(state): State<SharedState>) -> Response {
    let api = match api_for(&state) {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    let answer = Json(json!({ "config": api.runtime_config.snapshot() })).into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "explorer-api.config.list"),
        answer,
    )
    .await
}

/// `POST /admin/api/explorer-api/config-get` `{key}`.
pub async fn config_get(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let (api, key) = match (api_for(&state), key_field(&body)) {
        (Ok(a), Ok(k)) => (a, k),
        (Err(resp), _) | (_, Err(resp)) => return resp,
    };
    let answer = match api.runtime_config.get(&key) {
        Some(value) => Json(json!({ "key": key, "value": value })).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "not_found", "key": key })),
        )
            .into_response(),
    };
    finish(
        &state,
        Call::new(&session, DOMAIN, "explorer-api.config.get")
            .target(key.clone())
            .detail(json!({ "key": key })),
        answer,
    )
    .await
}

/// `POST /admin/api/explorer-api/config-set` `{key, value}`.
pub async fn config_set(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let (api, key) = match (api_for(&state), key_field(&body)) {
        (Ok(a), Ok(k)) => (a, k),
        (Err(resp), _) | (_, Err(resp)) => return resp,
    };
    let Some(value) = body.get("value").cloned() else {
        return super::console::bad_segment("value");
    };
    api.runtime_config.set(key.clone(), value.clone());
    let answer = Json(json!({ "ok": true, "key": key, "value": value })).into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "explorer-api.config.set")
            .target(key.clone())
            .detail(json!({ "value": value })),
        answer,
    )
    .await
}

/// `POST /admin/api/explorer-api/config-delete` `{key}`.
pub async fn config_delete(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let (api, key) = match (api_for(&state), key_field(&body)) {
        (Ok(a), Ok(k)) => (a, k),
        (Err(resp), _) | (_, Err(resp)) => return resp,
    };
    let removed = api.runtime_config.remove(&key).is_some();
    let answer = Json(json!({ "ok": true, "key": key, "removed": removed })).into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "explorer-api.config.delete")
            .target(key.clone())
            .detail(json!({ "key": key })),
        answer,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::parcel;

    #[test]
    fn a_poi_is_a_parcel() {
        assert_eq!(parcel(" 10, -5 ").as_deref(), Some("10,-5"));
        assert_eq!(parcel("-150,150").as_deref(), Some("-150,150"));
        for bad in ["", "10", "a,b", "1,2,3", "5000,1", "1.5,2"] {
            assert_eq!(parcel(bad), None, "{bad:?}");
        }
    }
}
