//! The console's JSON API. Every answer is the old envelope
//! `{ok, status, body}`; every mutation lands one `ops.admin_audit` row
//! (`actor` = the session wallet, `domain` = `content`, `action` = the old
//! action name without its prefix).

use axum::Json;
use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};

use super::auth::AdminSession;
use super::{SharedState, audit};
use crate::content::SyncState;
use crate::content::sync::orchestrator::SyncControl;
use crate::jobs::JobState;
use crate::shared::writes::{Refusal, WRITES_DISABLED_MESSAGE, ops_refusal};

const DOMAIN: &str = "content";

#[derive(serde::Serialize)]
pub(crate) struct AdminEnvelope {
    pub ok: bool,
    pub status: u16,
    pub body: Value,
}

impl AdminEnvelope {
    pub(crate) fn success(body: Value) -> Response {
        (
            StatusCode::OK,
            Json(AdminEnvelope {
                ok: true,
                status: 200,
                body,
            }),
        )
            .into_response()
    }

    pub(crate) fn error(status: StatusCode, body: Value) -> Response {
        (
            status,
            Json(AdminEnvelope {
                ok: false,
                status: status.as_u16(),
                body,
            }),
        )
            .into_response()
    }
}

/// The console's own writes under `OPS_WRITES_ENABLED` and the restore
/// stamps behind it: `None` lets the write through.
pub(crate) fn ops_refused(writes_enabled: bool) -> Option<Response> {
    ops_refusal(writes_enabled).map(|r| writes_refused(&r))
}

/// The 503 for `refusal`, in the envelope with a `Retry-After` (the social
/// gate's shape): the switch's message while `OPS_WRITES_ENABLED` is off,
/// the restore stamp that holds it and its age while it is on.
fn writes_refused(refusal: &Refusal) -> Response {
    let message = refusal.message("operator-plane", WRITES_DISABLED_MESSAGE);
    let mut resp = AdminEnvelope::error(
        StatusCode::SERVICE_UNAVAILABLE,
        json!({ "error": "writes-disabled", "message": message }),
    );
    resp.headers_mut().insert(
        header::RETRY_AFTER,
        header::HeaderValue::from_static("3600"),
    );
    resp
}

fn internal(e: impl std::fmt::Display) -> Response {
    AdminEnvelope::error(
        StatusCode::INTERNAL_SERVER_ERROR,
        json!({ "error": e.to_string() }),
    )
}

/// A sync control, applied and audited after the effect (process state, not
/// SQL). Without a sync worker the answer is the old backend's 501.
async fn control(
    state: &SharedState,
    session: &AdminSession,
    action: &str,
    apply: fn(&SyncControl) -> Value,
) -> Response {
    let entry = audit::Entry::new(session.address(), DOMAIN, action);
    match &state.sync_control {
        Some(control) => {
            let body = apply(control);
            audit::record_after(&state.pool, &entry).await;
            AdminEnvelope::success(body)
        }
        None => {
            let message = format!("{} not supported by this backend", action.replace('.', " "));
            audit::record_after(
                &state.pool,
                &entry.detail(json!({ "error": message })).result("error"),
            )
            .await;
            AdminEnvelope::error(StatusCode::NOT_IMPLEMENTED, json!({ "error": message }))
        }
    }
}

pub async fn sync_pause(session: AdminSession, State(state): State<SharedState>) -> Response {
    control(&state, &session, "sync.pause", |c| {
        c.pause();
        json!({ "control": "paused" })
    })
    .await
}

pub async fn sync_resume(session: AdminSession, State(state): State<SharedState>) -> Response {
    control(&state, &session, "sync.resume", |c| {
        c.resume();
        json!({ "control": "run" })
    })
    .await
}

pub async fn sync_force(session: AdminSession, State(state): State<SharedState>) -> Response {
    control(&state, &session, "sync.force", |c| {
        c.force();
        json!({ "forced": true, "control": "run" })
    })
    .await
}

pub async fn flush_cache(session: AdminSession, State(state): State<SharedState>) -> Response {
    if let Some(refused) = ops_refused(state.settings.writes_enabled) {
        return refused;
    }
    let entries = state.content.deployments_cache.len();
    state.content.deployments_cache.clear();
    audit::record_after(
        &state.pool,
        &audit::Entry::new(session.address(), DOMAIN, "flush-cache")
            .detail(json!({ "entries": entries })),
    )
    .await;
    AdminEnvelope::success(json!({ "cleared": true, "entries": entries }))
}

#[derive(Deserialize)]
pub struct EntityIdReq {
    #[serde(default, alias = "entityId")]
    pub id: String,
}

fn valid_content_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 512 && s.chars().all(|c| !c.is_control() && !c.is_whitespace())
}

/// Re-arms one failed deployment for the retry worker: its deadline moves to
/// the epoch and its attempt count to zero, so the next `retry-failed` cycle
/// (at most `retry_delay_ms`, 15 min by default, after this call) picks it
/// first instead of evicting a record that had reached the cap. The old live
/// node answered 501 here; a synchronous re-deploy stays with the worker, so
/// one code path attempts entities. Same transaction as the audit row.
pub async fn failed_retry(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(req): Json<EntityIdReq>,
) -> Response {
    if let Some(refused) = ops_refused(state.settings.writes_enabled) {
        return refused;
    }
    if !valid_content_id(&req.id) {
        return AdminEnvelope::error(
            StatusCode::BAD_REQUEST,
            json!({ "error": "invalid-entity-id" }),
        );
    }
    let outcome: Result<Response, sqlx::Error> = async {
        let mut tx = state.pool.begin().await?;
        let rearmed = sqlx::query(
            "UPDATE content.failed_deployments
             SET next_retry_at = '1970-01-01 00:00:00'::timestamp, retry_count = 0
             WHERE entity_id = $1",
        )
        .bind(&req.id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        let entry = audit::Entry::new(session.address(), DOMAIN, "failed-deployments.retry")
            .target(Some(&req.id))
            .detail(json!({ "entityId": req.id, "rearmed": rearmed == 1 }))
            .result(if rearmed == 1 { "ok" } else { "not-found" });
        audit::record(&mut *tx, &entry).await?;
        tx.commit().await?;
        Ok(if rearmed == 1 {
            AdminEnvelope::success(
                json!({ "entityId": req.id, "rearmed": true, "worker": "retry-failed" }),
            )
        } else {
            AdminEnvelope::error(
                StatusCode::NOT_FOUND,
                json!({ "error": "not-found", "entityId": req.id }),
            )
        })
    }
    .await;
    outcome.unwrap_or_else(internal)
}

/// Purges one failed deployment, or every one when `id` is empty (the old
/// `clear` contract). Same transaction as the audit row.
pub async fn failed_clear(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(req): Json<EntityIdReq>,
) -> Response {
    if let Some(refused) = ops_refused(state.settings.writes_enabled) {
        return refused;
    }
    let all = req.id.is_empty();
    if !all && !valid_content_id(&req.id) {
        return AdminEnvelope::error(
            StatusCode::BAD_REQUEST,
            json!({ "error": "invalid-entity-id" }),
        );
    }
    let outcome: Result<Response, sqlx::Error> = async {
        let mut tx = state.pool.begin().await?;
        let removed = if all {
            sqlx::query("DELETE FROM content.failed_deployments").execute(&mut *tx).await?.rows_affected()
        } else {
            sqlx::query("DELETE FROM content.failed_deployments WHERE entity_id = $1").bind(&req.id).execute(&mut *tx).await?.rows_affected()
        };
        let scope = if all { "all" } else { "one" };
        let entry = audit::Entry::new(session.address(), DOMAIN, "failed-deployments.clear")
            .target((!all).then_some(req.id.as_str()))
            .detail(json!({ "scope": scope, "entityId": (!all).then_some(&req.id), "removed": removed }));
        audit::record(&mut *tx, &entry).await?;
        tx.commit().await?;
        Ok(AdminEnvelope::success(json!({ "removed": removed, "scope": scope })))
    }
    .await;
    outcome.unwrap_or_else(internal)
}

/// `GET /admin/api/jobs`: the job registry as it stands (process state).
pub async fn jobs(_session: AdminSession, State(state): State<SharedState>) -> Response {
    let jobs: Vec<Value> = state
        .jobs
        .report()
        .into_iter()
        .map(|j| {
            json!({
                "name": j.name,
                "state": match j.state { JobState::Running => "running", JobState::Standby => "standby", JobState::StoodDown => "stood-down", JobState::Finished => "finished", JobState::Failed => "failed" },
                "uptimeSecs": j.started_at.elapsed().as_secs(),
                "progress": j.progress,
                "lastError": j.last_error,
                "passes": j.passes,
            })
        })
        .collect();
    AdminEnvelope::success(json!({ "jobs": jobs }))
}

/// `GET /admin/api/cache`: the in-process caches the console can flush or
/// should know about.
pub async fn cache(_session: AdminSession, State(state): State<SharedState>) -> Response {
    AdminEnvelope::success(json!({
        "deployments": { "entries": state.content.deployments_cache.len() },
        "denylist": { "ids": state.content.denylist.list().len() },
    }))
}

#[derive(sqlx::FromRow)]
struct PeerCursor {
    server_url: String,
    cursor_ms: i64,
    updated_at: chrono::NaiveDateTime,
}

#[derive(sqlx::FromRow)]
struct FailedCounts {
    total: i64,
    due_now: i64,
    waiting: i64,
    without_auth_chain: i64,
}

/// `GET /admin/api/sync`: the orchestrator's state and control, the
/// per-peer cursors and the failed-deployment counts (`now` is a parameter;
/// the counts scan the bounded retry backlog, not a history table).
pub async fn sync(_session: AdminSession, State(state): State<SharedState>) -> Response {
    let status = &state.content.sync;
    let (sync_state, ready_types) = match status.state() {
        SyncState::PartiallySynced { ready_types } => ("PartiallySynced", Some(ready_types)),
        SyncState::Bootstrapping => ("Bootstrapping", None),
        SyncState::Syncing => ("Syncing", None),
        SyncState::Failed => ("Failed", None),
    };
    let control = match &state.sync_control {
        None => "unavailable",
        Some(c) if c.is_paused() => "paused",
        Some(_) => "run",
    };
    let now = chrono::Utc::now().naive_utc();
    let outcome: Result<Response, sqlx::Error> = async {
        let peers = sqlx::query_as::<_, PeerCursor>(
            "SELECT server_url, cursor_ms, updated_at FROM content.server_sync_cursors ORDER BY server_url",
        )
        .fetch_all(&state.pool)
        .await?;
        let failed = sqlx::query_as::<_, FailedCounts>(
            "SELECT count(*)::bigint AS total,
                    count(*) FILTER (WHERE next_retry_at <= $1
                        AND CASE WHEN json_typeof(auth_chain) = 'array' THEN json_array_length(auth_chain) > 0 ELSE false END)::bigint AS due_now,
                    count(*) FILTER (WHERE next_retry_at > $1)::bigint AS waiting,
                    count(*) FILTER (WHERE NOT CASE WHEN json_typeof(auth_chain) = 'array' THEN json_array_length(auth_chain) > 0 ELSE false END)::bigint AS without_auth_chain
             FROM content.failed_deployments",
        )
        .bind(now)
        .fetch_one(&state.pool)
        .await?;
        Ok(AdminEnvelope::success(json!({
            "state": sync_state,
            "readyTypes": ready_types,
            "frontierMs": status.sync_frontier_ms(),
            "heartbeatMs": status.heartbeat_ms(),
            "control": control,
            "peers": peers.iter().map(|p| json!({ "server": p.server_url, "cursorMs": p.cursor_ms, "updatedAt": p.updated_at })).collect::<Vec<_>>(),
            "failedDeployments": {
                "total": failed.total,
                "dueNow": failed.due_now,
                "waiting": failed.waiting,
                "withoutAuthChain": failed.without_auth_chain,
            },
            "writesEnabled": state.settings.writes_enabled,
        })))
    }
    .await;
    outcome.unwrap_or_else(internal)
}

async fn rows_json(pool: &sqlx::PgPool, sql: &'static str) -> sqlx::Result<Vec<Value>> {
    sqlx::query_scalar::<_, Value>(sql).fetch_all(pool).await
}

/// `GET /admin/api/mirrors`: every mirror's pass/cursor row as the module
/// that owns it keeps it (one small table each; the whole row, so a new
/// column shows up without a console change).
pub async fn mirrors(_session: AdminSession, State(state): State<SharedState>) -> Response {
    let pool = &state.pool;
    let outcome: Result<Response, sqlx::Error> = async {
        Ok(AdminEnvelope::success(json!({
            "explore": rows_json(pool, "SELECT to_jsonb(t) FROM explore.mirror_state t ORDER BY mirror").await?,
            "explorerApi": rows_json(pool, "SELECT to_jsonb(t) FROM explore_api.mirror_passes t ORDER BY mirror").await?,
            "worlds": rows_json(pool, "SELECT to_jsonb(t) FROM worlds.mirror_passes t ORDER BY mirror").await?,
            "marketplace": rows_json(pool, "SELECT to_jsonb(t) FROM shop.mirror_state t ORDER BY mirror, network").await?,
            "landAuthz": rows_json(pool, "SELECT to_jsonb(t) FROM land_authz.index_cursor t ORDER BY network").await?,
        })))
    }
    .await;
    outcome.unwrap_or_else(internal)
}

/// `GET /admin/api/audit`: newest first; `nextBefore` is the id to pass as
/// `before` for the following page, absent on the last one.
pub async fn audit_list(
    _session: AdminSession,
    State(state): State<SharedState>,
    Query(filter): Query<audit::Filter>,
) -> Response {
    match audit::list(&state.pool, &filter).await {
        Ok(rows) => {
            let limit = filter
                .limit
                .unwrap_or(audit::DEFAULT_PAGE)
                .clamp(1, audit::MAX_PAGE) as usize;
            let next_before = (rows.len() == limit)
                .then(|| rows.last().map(|r| r.id))
                .flatten();
            AdminEnvelope::success(json!({ "rows": rows, "nextBefore": next_before }))
        }
        Err(e) => internal(e),
    }
}
