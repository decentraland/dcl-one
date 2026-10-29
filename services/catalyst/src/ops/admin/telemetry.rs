//! `/admin/api/telemetry/*`: the issue workflow state, the read-only SQL
//! console and the eight `/dash/admin/*` operator routes, as calls into
//! `ops::telemetry`. The writes answer the telemetry console's own 503
//! while `OPS_WRITES_ENABLED` is off (its gate lets `/dash/sql` through);
//! the handlers get a [`TelemetryAdmin`] minted from the telemetry bearer
//! this node holds, so they write their own `ops.admin_audit` row as
//! before, with the console wallet as `operator` in the detail.

#![allow(clippy::result_large_err)]

use axum::Json;
use axum::extract::{Query, State};
use axum::http::Method;
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use super::SharedState;
use super::auth::AdminSession;
use super::console::{
    Call, body_as, configured, finish, not_configured, present, query_from_obj, refused,
    service_headers, token_missing,
};
use crate::ops::telemetry::admin::{self as tadmin, ActorQuery, TelemetryAdmin};
use crate::ops::telemetry::{self, dashboard};

const DOMAIN: &str = "telemetry";

/// Why the telemetry console's own gate refuses `method` on `path`, if it does.
fn gated(
    st: &telemetry::SharedState,
    method: &Method,
    path: &str,
) -> Option<crate::shared::writes::Refusal> {
    if *method == Method::GET || telemetry::OPEN_WHILE_DISABLED.contains(&path) {
        return None;
    }
    crate::shared::writes::ops_refusal(st.settings.writes_enabled)
}

/// The telemetry state, the write gate and the minted credential, or the
/// console's answer for whichever is missing.
async fn admin_for(
    state: &SharedState,
    session: &AdminSession,
    method: Method,
    path: &str,
) -> Result<(telemetry::SharedState, TelemetryAdmin), Response> {
    let Some(st) = state.domains.telemetry.clone() else {
        return Err(not_configured(DOMAIN));
    };
    if let Some(r) = gated(&st, &method, path) {
        return Err(refused(telemetry::writes_refused_response(&r)).await);
    }
    let Some(token) = configured(st.settings.admin_token.as_deref()) else {
        return Err(token_missing(DOMAIN));
    };
    match present::<TelemetryAdmin, _>(&st, service_headers(Some(token), session.address())).await {
        Ok(admin) => Ok((st, admin)),
        Err(rejection) => Err(super::console::envelope(rejection.into_response()).await),
    }
}

/// The old console form sends `resolve`/`ignore`/`unresolve`; the route
/// stores `resolved`/`ignored`/`unresolved`. Blank optional fields are left
/// out (absent = keep), as the form means.
fn issue_state_body(body: &Value) -> Value {
    let mut out = super::console::body_without(body, &[]).unwrap_or_else(|| json!({}));
    if let Some(status) = out.get("status").and_then(Value::as_str) {
        let mapped = match status {
            "resolve" => "resolved",
            "ignore" => "ignored",
            "unresolve" => "unresolved",
            other => other,
        };
        out["status"] = json!(mapped);
    }
    out
}

pub async fn issue_state(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let (st, admin) = match admin_for(&state, &session, Method::POST, "/dash/issue/state").await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let fingerprint = super::console::str_field(&body, "fingerprint").unwrap_or_default();
    let typed = match body_as::<dashboard::IssueStateBody>(issue_state_body(&body)) {
        Ok(b) => b,
        Err(resp) => return resp,
    };
    let answer = dashboard::set_issue_state(admin, State(st), Json(typed))
        .await
        .into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "telemetry.issue-state")
            .target(fingerprint)
            .detail(body),
        answer,
    )
    .await
}

/// `POST /admin/api/telemetry/sql`: a read (the console gate lets it
/// through); the telemetry reader role and its `READ ONLY` transaction are
/// the module's.
pub async fn sql(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let Some(st) = state.domains.telemetry.clone() else {
        return not_configured(DOMAIN);
    };
    let typed = match body_as::<dashboard::SqlBody>(body.clone()) {
        Ok(b) => b,
        Err(resp) => return resp,
    };
    let answer = dashboard::sql_query(State(st), Json(typed))
        .await
        .into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "telemetry.sql").detail(body),
        answer,
    )
    .await
}

/// The operator's label rides in `x-catalyrst-admin` (the minted headers);
/// `?actor=` stays empty.
fn operator_headers(session: &AdminSession) -> axum::http::HeaderMap {
    service_headers(None, session.address())
}

macro_rules! telemetry_admin_post {
    ($name:ident, $leaf:literal, $action:literal, $body:ty, $handler:path) => {
        pub async fn $name(
            session: AdminSession,
            State(state): State<SharedState>,
            Json(body): Json<Value>,
        ) -> Response {
            let (st, admin) = match admin_for(
                &state,
                &session,
                Method::POST,
                concat!("/dash/admin/", $leaf),
            )
            .await
            {
                Ok(v) => v,
                Err(resp) => return resp,
            };
            let typed = match body_as::<$body>(body.clone()) {
                Ok(b) => b,
                Err(resp) => return resp,
            };
            let answer = $handler(
                admin,
                State(st),
                operator_headers(&session),
                Query(ActorQuery::default()),
                Json(typed),
            )
            .await
            .into_response();
            finish(
                &state,
                Call::new(&session, DOMAIN, $action).detail(body),
                answer,
            )
            .await
        }
    };
}

telemetry_admin_post!(
    purge,
    "purge",
    "telemetry.purge",
    tadmin::PurgeBody,
    tadmin::purge
);
telemetry_admin_post!(
    quota,
    "quota",
    "telemetry.quota",
    tadmin::QuotaBody,
    tadmin::quota
);
telemetry_admin_post!(
    bulk_delete,
    "bulk-delete",
    "telemetry.bulk-delete",
    tadmin::BulkFilter,
    tadmin::bulk_delete
);
telemetry_admin_post!(
    export,
    "export",
    "telemetry.export",
    tadmin::ExportBody,
    tadmin::export
);
telemetry_admin_post!(
    regroup,
    "regroup",
    "telemetry.regroup",
    tadmin::RegroupBody,
    tadmin::regroup
);
telemetry_admin_post!(
    release,
    "release",
    "telemetry.release",
    tadmin::ReleaseBody,
    tadmin::release
);

/// `POST /admin/api/telemetry/ingest`: with `enabled` the runtime toggle (a
/// write), without it the ingest report (a read).
pub async fn ingest(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let toggle = body.get("enabled").is_some_and(|v| !v.is_null() && v != "");
    let method = if toggle { Method::POST } else { Method::GET };
    let (st, admin) = match admin_for(&state, &session, method, "/dash/admin/ingest").await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let answer = if toggle {
        let Some(enabled) = super::console::bool_field(&body, "enabled") else {
            return super::console::bad_body("enabled must be true or false");
        };
        let typed = match body_as::<tadmin::IngestBody>(json!({ "enabled": enabled })) {
            Ok(b) => b,
            Err(resp) => return resp,
        };
        tadmin::ingest_toggle(
            admin,
            State(st),
            operator_headers(&session),
            Query(ActorQuery::default()),
            Json(typed),
        )
        .await
        .into_response()
    } else {
        tadmin::ingest_status(admin, State(st))
            .await
            .into_response()
    };
    finish(
        &state,
        Call::new(&session, DOMAIN, "telemetry.ingest").detail(body),
        answer,
    )
    .await
}

/// `POST /admin/api/telemetry/audit`: the telemetry audit page, its filters
/// taken from the body as the old proxy's query string.
pub async fn audit(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let (st, admin) = match admin_for(&state, &session, Method::GET, "/dash/admin/audit").await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let q = match query_from_obj::<tadmin::AuditQuery>(&body, &["fingerprint", "action", "limit"]) {
        Ok(q) => q,
        Err(resp) => return resp,
    };
    let answer = tadmin::audit_list(admin, State(st), Query(q))
        .await
        .into_response();
    finish(
        &state,
        Call::new(&session, DOMAIN, "telemetry.audit").detail(body),
        answer,
    )
    .await
}
