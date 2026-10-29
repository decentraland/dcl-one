//! The admin bearer and the operator routes. `CATALYRST_TELEMETRY_ADMIN_TOKEN`
//! unset answers 403 "admin disabled" to every gated request (fail closed);
//! a bearer is compared in constant time; the console's GET reads also
//! accept the 30-day login session (an HMAC under the token of its expiry,
//! `login::session_valid`; never the token itself). [`TelemetryAdmin`] is the gate as an
//! extractor: the only way a handler obtains one is `from_request_parts`
//! (private field, no other constructor), so an operator route that forgets
//! it does not compile into an open route. Every mutation writes its
//! `ops.admin_audit` row in the same statement ([`audited`]), actor
//! `token:CATALYRST_TELEMETRY_ADMIN_TOKEN` (the verified credential), the
//! console's `x-catalyrst-admin` / `?actor=` label in `detail.operator`
//! (client-supplied, never the actor). The two bulk deletes (`purge`,
//! `bulk-delete`) run as the retention job's 5,000-row `ctid` batches under
//! `TELEMETRY_PURGE_MAX_BATCHES` ([`super::purge::delete_in_batches`]) and
//! write their audit row after the last batch with `{deleted, batches,
//! exhausted}`; an exhausted call is repeated by the operator, every batch
//! before it already committed.

use std::sync::atomic::Ordering;

use axum::Json;
use axum::extract::{FromRequestParts, Query, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{ADMIN_TOKEN_ENV, SharedState, db_err, purge};
use crate::auth::admin_bearer::bearer_token;

pub const AUDIT_DOMAIN: &str = "telemetry";

type AdminResult = Result<Json<Value>, (StatusCode, String)>;

fn bad(msg: &str) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, msg.to_string())
}

fn token_ok(expected: Option<&str>, presented: Option<&str>) -> bool {
    crate::auth::admin_bearer::check_token(expected, presented).is_ok()
}

fn cookie_token(headers: &HeaderMap) -> Option<String> {
    let cookies = headers.get("cookie")?.to_str().ok()?;
    cookies.split(';').find_map(|pair| {
        let (name, value) = pair.trim().split_once('=')?;
        (name == super::login::COOKIE_NAME && !value.is_empty()).then(|| value.to_string())
    })
}

fn disabled() -> (StatusCode, String) {
    (
        StatusCode::FORBIDDEN,
        format!("admin disabled ({ADMIN_TOKEN_ENV} unset)"),
    )
}

fn invalid() -> (StatusCode, String) {
    (StatusCode::FORBIDDEN, "invalid admin bearer".into())
}

pub(crate) fn token_matches(state: &SharedState, presented: &str) -> bool {
    token_ok(state.settings.admin_token.as_deref(), Some(presented))
}

/// Bearer or login session: the console's reads and pages.
pub(crate) fn authorize_read(
    state: &SharedState,
    headers: &HeaderMap,
) -> Result<(), (StatusCode, String)> {
    let Some(expected) = state.settings.admin_token.as_deref() else {
        return Err(disabled());
    };
    if let Some(bearer) = bearer_token(headers) {
        return if token_ok(Some(expected), Some(bearer)) {
            Ok(())
        } else {
            Err(invalid())
        };
    }
    match cookie_token(headers) {
        Some(session)
            if super::login::session_valid(expected, &session, super::login::now_unix()) =>
        {
            Ok(())
        }
        _ => Err(invalid()),
    }
}

/// Bearer only: every write.
pub(crate) fn authorize(
    state: &SharedState,
    headers: &HeaderMap,
) -> Result<(), (StatusCode, String)> {
    if state.settings.admin_token.is_none() {
        return Err(disabled());
    }
    if token_ok(state.settings.admin_token.as_deref(), bearer_token(headers)) {
        Ok(())
    } else {
        Err(invalid())
    }
}

/// Proof that the request presented the configured admin bearer; carries
/// the audit actor. Constructed by `from_request_parts` only.
pub struct TelemetryAdmin {
    actor: String,
}

impl TelemetryAdmin {
    /// The server-verified audit actor: `token:CATALYRST_TELEMETRY_ADMIN_TOKEN`.
    pub fn actor(&self) -> &str {
        &self.actor
    }
}

impl FromRequestParts<SharedState> for TelemetryAdmin {
    type Rejection = (StatusCode, String);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &SharedState,
    ) -> Result<Self, Self::Rejection> {
        authorize(state, &parts.headers)?;
        Ok(Self {
            actor: format!("token:{ADMIN_TOKEN_ENV}"),
        })
    }
}

#[derive(Deserialize, Default)]
pub struct ActorQuery {
    #[serde(default)]
    actor: Option<String>,
}

fn clean_actor(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.chars().take(100).collect())
    }
}

/// The console operator's label (`x-catalyrst-admin`, then `?actor=`), kept
/// in the audit detail; `loopback` when neither is present.
pub(crate) fn operator_of(headers: &HeaderMap, q: &ActorQuery) -> String {
    headers
        .get("x-catalyrst-admin")
        .and_then(|v| v.to_str().ok())
        .and_then(clean_actor)
        .or_else(|| q.actor.as_deref().and_then(clean_actor))
        .unwrap_or_else(|| "loopback".to_string())
}

fn with_operator(mut detail: Value, operator: &str) -> Value {
    if let Some(obj) = detail.as_object_mut() {
        obj.insert("operator".into(), json!(operator));
    }
    detail
}

/// One statement for `write` plus its audit row; bind actor, action, target,
/// detail after the write's `binds`.
pub(crate) fn audited(write: &str, binds: usize) -> String {
    format!(
        "WITH w AS ({write} RETURNING 1) \
         INSERT INTO ops.admin_audit (actor, domain, action, target, detail) \
         VALUES (${}, '{AUDIT_DOMAIN}', ${}, ${}, ${})",
        binds + 1,
        binds + 2,
        binds + 3,
        binds + 4
    )
}

/// The audit row of a batched delete, written once after the last batch
/// (the batches are their own transactions); the detail carries the report.
async fn audit_batched_delete(
    pool: &sqlx::PgPool,
    actor: &str,
    action: &str,
    target: Option<&str>,
    detail: Value,
    report: &purge::BatchReport,
) -> Result<(), sqlx::Error> {
    let mut detail = detail;
    if let Some(obj) = detail.as_object_mut() {
        obj.insert("deleted".into(), json!(report.deleted));
        obj.insert("batches".into(), json!(report.batches));
        obj.insert("exhausted".into(), json!(report.exhausted));
    }
    sqlx::query("INSERT INTO ops.admin_audit (actor, domain, action, target, detail) VALUES ($1, $2, $3, $4, $5)")
        .bind(actor)
        .bind(AUDIT_DOMAIN)
        .bind(action)
        .bind(target)
        .bind(detail)
        .execute(pool)
        .await
        .map(|_| ())
}

fn batch_answer(report: &purge::BatchReport) -> Json<Value> {
    Json(
        json!({ "ok": true, "deleted": report.deleted, "batches": report.batches, "exhausted": report.exhausted }),
    )
}

fn now_utc() -> chrono::NaiveDateTime {
    chrono::Utc::now().naive_utc()
}

#[derive(Deserialize)]
pub struct PurgeBody {
    older_than_days: i64,
    source: Option<String>,
    project: Option<String>,
}

pub async fn purge(
    admin: TelemetryAdmin,
    State(st): State<SharedState>,
    headers: HeaderMap,
    Query(aq): Query<ActorQuery>,
    Json(b): Json<PurgeBody>,
) -> AdminResult {
    if b.older_than_days < 1 {
        return Err(bad("older_than_days must be >= 1"));
    }
    let operator = operator_of(&headers, &aq);
    let cutoff = now_utc() - chrono::Duration::days(b.older_than_days);
    let source = b.source.clone().filter(|s| !s.is_empty());
    let project = b.project.clone().filter(|s| !s.is_empty());
    let report = purge::delete_in_batches(&st.pool, st.settings.purge_max_batches, |batch| {
        sqlx::query(PURGE_BATCH_SQL)
            .bind(cutoff)
            .bind(source.clone())
            .bind(project.clone())
            .bind(batch)
    })
    .await
    .map_err(|e| db_err("telemetry admin", e))?;
    audit_batched_delete(
        &st.pool,
        admin.actor(),
        "purge",
        project.as_deref(),
        with_operator(json!({ "older_than_days": b.older_than_days, "source": b.source, "project": b.project }), &operator),
        &report,
    )
    .await
    .map_err(|e| db_err("telemetry admin", e))?;
    Ok(batch_answer(&report))
}

/// One purge batch: the retention job's shape with the operator's filters.
const PURGE_BATCH_SQL: &str = "DELETE FROM telemetry.events WHERE ctid = ANY (ARRAY( \
       SELECT ctid FROM telemetry.events \
       WHERE received_at < $1 AND ($2::text IS NULL OR source = $2) AND ($3::text IS NULL OR project = $3) \
       ORDER BY received_at LIMIT $4))";

#[derive(Deserialize)]
pub struct IngestBody {
    enabled: bool,
}

/// `GET /dash/admin/ingest`: the runtime toggle, the admission bounds and
/// the refused/dropped/written counters since boot.
pub async fn ingest_status(_admin: TelemetryAdmin, State(st): State<SharedState>) -> AdminResult {
    Ok(Json(st.ingest_counters()))
}

pub async fn ingest_toggle(
    admin: TelemetryAdmin,
    State(st): State<SharedState>,
    headers: HeaderMap,
    Query(aq): Query<ActorQuery>,
    Json(b): Json<IngestBody>,
) -> AdminResult {
    let operator = operator_of(&headers, &aq);
    sqlx::query(sqlx::AssertSqlSafe(audited(
        "UPDATE telemetry.ingest_settings SET enabled = $1, updated_at = timezone('UTC', now()) WHERE id = 1",
        1,
    )))
    .bind(b.enabled)
    .bind(admin.actor())
    .bind("ingest_toggle")
    .bind("ingest_settings")
    .bind(with_operator(json!({ "enabled": b.enabled }), &operator))
    .execute(&st.pool)
    .await
    .map_err(|e| db_err("telemetry admin", e))?;
    st.ingest.enabled.store(b.enabled, Ordering::Relaxed);
    Ok(Json(json!({ "ok": true, "enabled": b.enabled })))
}

#[derive(Deserialize)]
pub struct QuotaBody {
    project: String,
    daily_limit: Option<i64>,
}

pub async fn quota(
    admin: TelemetryAdmin,
    State(st): State<SharedState>,
    headers: HeaderMap,
    Query(aq): Query<ActorQuery>,
    Json(b): Json<QuotaBody>,
) -> AdminResult {
    if b.project.is_empty() {
        return Err(bad("project required"));
    }
    let operator = operator_of(&headers, &aq);
    let detail = with_operator(
        json!({ "project": b.project, "daily_limit": b.daily_limit }),
        &operator,
    );
    match b.daily_limit {
        Some(limit) => {
            if limit < 0 {
                return Err(bad("daily_limit must be >= 0"));
            }
            sqlx::query(sqlx::AssertSqlSafe(audited(
                "INSERT INTO telemetry.project_quotas (project, daily_limit, updated_at) VALUES ($1, $2, timezone('UTC', now())) \
                 ON CONFLICT (project) DO UPDATE SET daily_limit = $2, updated_at = timezone('UTC', now())",
                2,
            )))
            .bind(&b.project)
            .bind(limit)
            .bind(admin.actor())
            .bind("quota")
            .bind(&b.project)
            .bind(detail)
            .execute(&st.pool)
            .await
            .map_err(|e| db_err("telemetry admin", e))?;
            st.ingest
                .quotas
                .write()
                .unwrap()
                .insert(b.project.clone(), limit);
        }
        None => {
            sqlx::query(sqlx::AssertSqlSafe(audited(
                "DELETE FROM telemetry.project_quotas WHERE project = $1",
                1,
            )))
            .bind(&b.project)
            .bind(admin.actor())
            .bind("quota")
            .bind(&b.project)
            .bind(detail)
            .execute(&st.pool)
            .await
            .map_err(|e| db_err("telemetry admin", e))?;
            st.ingest.quotas.write().unwrap().remove(&b.project);
        }
    }
    Ok(Json(
        json!({ "ok": true, "project": b.project, "daily_limit": b.daily_limit }),
    ))
}

#[derive(Deserialize)]
pub struct BulkFilter {
    source: Option<String>,
    project: Option<String>,
    fingerprint: Option<String>,
    /// An ISO timestamp (UTC); rows received before it.
    before: Option<String>,
    level: Option<String>,
}

impl BulkFilter {
    fn require_some(&self) -> Result<(), (StatusCode, String)> {
        if self.binds().iter().all(Option::is_none) {
            return Err(bad(
                "refusing an unfiltered bulk operation; specify at least one of source/project/fingerprint/before/level",
            ));
        }
        Ok(())
    }

    fn binds(&self) -> [Option<&str>; 5] {
        [
            self.source.as_deref().filter(|s| !s.is_empty()),
            self.project.as_deref().filter(|s| !s.is_empty()),
            self.fingerprint.as_deref().filter(|s| !s.is_empty()),
            self.before.as_deref().filter(|s| !s.is_empty()),
            self.level.as_deref().filter(|s| !s.is_empty()),
        ]
    }

    fn target(&self) -> Option<&str> {
        self.fingerprint
            .as_deref()
            .filter(|s| !s.is_empty())
            .or(self.project.as_deref().filter(|s| !s.is_empty()))
    }

    fn detail(&self) -> Value {
        json!({ "source": self.source, "project": self.project, "fingerprint": self.fingerprint, "before": self.before, "level": self.level })
    }
}

/// `before` is parsed by PostgreSQL as a UTC `timestamp` (a trailing `Z` or
/// offset is dropped by the cast, which is the session's clock anyway).
const BULK_WHERE: &str = "($1::text IS NULL OR source = $1) \
     AND ($2::text IS NULL OR project = $2) \
     AND ($3::text IS NULL OR fingerprint = $3) \
     AND ($4::text IS NULL OR received_at < ($4::timestamptz AT TIME ZONE 'UTC')) \
     AND ($5::text IS NULL OR body->>'level' = $5)";

pub async fn bulk_delete(
    admin: TelemetryAdmin,
    State(st): State<SharedState>,
    headers: HeaderMap,
    Query(aq): Query<ActorQuery>,
    Json(f): Json<BulkFilter>,
) -> AdminResult {
    f.require_some()?;
    let sql = format!(
        "DELETE FROM telemetry.events WHERE ctid = ANY (ARRAY(SELECT ctid FROM telemetry.events WHERE {BULK_WHERE} ORDER BY id LIMIT $6))"
    );
    let owned: Vec<Option<String>> = f.binds().iter().map(|b| b.map(str::to_string)).collect();
    let operator = operator_of(&headers, &aq);
    let report = purge::delete_in_batches(&st.pool, st.settings.purge_max_batches, |batch| {
        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql.clone()));
        for b in &owned {
            q = q.bind(b.clone());
        }
        q.bind(batch)
    })
    .await
    .map_err(|e| (StatusCode::BAD_REQUEST, format!("db error: {e}")))?;
    audit_batched_delete(
        &st.pool,
        admin.actor(),
        "bulk_delete",
        f.target(),
        with_operator(f.detail(), &operator),
        &report,
    )
    .await
    .map_err(|e| db_err("telemetry admin", e))?;
    Ok(batch_answer(&report))
}

#[derive(Deserialize)]
pub struct ExportBody {
    #[serde(flatten)]
    filter: BulkFilter,
    limit: Option<i64>,
}

pub async fn export(
    admin: TelemetryAdmin,
    State(st): State<SharedState>,
    headers: HeaderMap,
    Query(aq): Query<ActorQuery>,
    Json(b): Json<ExportBody>,
) -> AdminResult {
    let limit = b.limit.unwrap_or(100).clamp(1, 10_000);

    let sql = format!(
        "WITH r AS ( \
           SELECT to_jsonb(t) - 'ord' AS row, t.ord FROM ( \
             SELECT id, source, project, event_kind, fingerprint, \
               to_char(received_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS received_at, \
               body, received_at AS ord \
             FROM telemetry.events WHERE {BULK_WHERE} \
             ORDER BY telemetry.events.received_at DESC LIMIT {limit} \
           ) t \
         ), a AS ( \
           INSERT INTO ops.admin_audit (actor, domain, action, target, detail) \
           SELECT $6, '{AUDIT_DOMAIN}', $7, $8, $9::jsonb || jsonb_build_object('count', (SELECT count(*) FROM r)) \
         ) \
         SELECT row FROM r ORDER BY ord DESC"
    );
    let [b1, b2, b3, b4, b5] = b.filter.binds();
    let operator = operator_of(&headers, &aq);
    let rows: Vec<Value> = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(b1)
        .bind(b2)
        .bind(b3)
        .bind(b4)
        .bind(b5)
        .bind(admin.actor())
        .bind("export")
        .bind(b.filter.target())
        .bind(with_operator(b.filter.detail(), &operator))
        .fetch_all(&st.pool)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("db error: {e}")))?;
    let truncated = rows.len() as i64 >= limit;
    Ok(Json(
        json!({ "ok": true, "count": rows.len(), "truncated": truncated, "events": rows }),
    ))
}

#[derive(Deserialize)]
pub struct AuditQuery {
    fingerprint: Option<String>,
    action: Option<String>,
    #[serde(default = "d_audit_limit")]
    limit: i64,
}
fn d_audit_limit() -> i64 {
    200
}

/// `ops.admin_audit` rows of this domain, newest first; `?fingerprint=`
/// matches the target or the detail's fingerprint (the regroup's canonical).
pub async fn audit_list(
    _admin: TelemetryAdmin,
    State(st): State<SharedState>,
    Query(q): Query<AuditQuery>,
) -> AdminResult {
    let limit = q.limit.clamp(1, 1000);
    let sql = format!(
        "SELECT id, to_char(at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS at, actor, action, target, detail \
         FROM ops.admin_audit \
         WHERE domain = '{AUDIT_DOMAIN}' \
           AND ($1::text IS NULL OR target = $1 OR detail->>'fingerprint' = $1) \
           AND ($2::text IS NULL OR action = $2) \
         ORDER BY at DESC, id DESC LIMIT {limit}"
    );
    let rows = sqlx::query_as::<_, (i64, String, String, String, Option<String>, Value)>(
        sqlx::AssertSqlSafe(sql),
    )
    .bind(q.fingerprint.as_deref().filter(|s| !s.is_empty()))
    .bind(q.action.as_deref().filter(|s| !s.is_empty()))
    .fetch_all(&st.pool)
    .await
    .map_err(|e| db_err("telemetry admin", e))?;
    let items: Vec<Value> = rows
        .into_iter()
        .map(|(id, at, actor, action, target, detail)| {
            json!({ "id": id, "at": at, "actor": actor, "action": action, "target": target, "detail": detail })
        })
        .collect();
    Ok(Json(
        json!({ "ok": true, "count": items.len(), "items": items }),
    ))
}

#[derive(Deserialize)]
pub struct RegroupBody {
    sources: Vec<String>,
    canonical: String,
}

pub async fn regroup(
    admin: TelemetryAdmin,
    State(st): State<SharedState>,
    headers: HeaderMap,
    Query(aq): Query<ActorQuery>,
    Json(b): Json<RegroupBody>,
) -> AdminResult {
    if b.canonical.is_empty() {
        return Err(bad("canonical required"));
    }
    let sources: Vec<String> = b
        .sources
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != &b.canonical)
        .collect();
    if sources.is_empty() {
        return Err(bad(
            "at least one source fingerprint (distinct from canonical) required",
        ));
    }
    let merged = sources.len() as i64;
    let mut distinct = sources.clone();
    distinct.sort();
    distinct.dedup();
    let operator = operator_of(&headers, &aq);
    sqlx::query(sqlx::AssertSqlSafe(audited(
        "INSERT INTO telemetry.issue_merges (source_fingerprint, canonical_fingerprint, merged_at) \
         SELECT s, $2, timezone('UTC', now()) FROM unnest($1::text[]) AS s \
         ON CONFLICT (source_fingerprint) \
         DO UPDATE SET canonical_fingerprint = $2, merged_at = timezone('UTC', now())",
        2,
    )))
    .bind(&distinct)
    .bind(&b.canonical)
    .bind(admin.actor())
    .bind("regroup")
    .bind(&b.canonical)
    .bind(with_operator(
        json!({ "fingerprint": b.canonical, "canonical": b.canonical, "sources": sources, "merged": merged }),
        &operator,
    ))
    .execute(&st.pool)
    .await
    .map_err(|e| db_err("telemetry admin", e))?;
    Ok(Json(
        json!({ "ok": true, "canonical": b.canonical, "merged": merged }),
    ))
}

#[derive(Deserialize)]
pub struct ReleaseBody {
    release: String,
    state: String,
    note: Option<String>,
}

pub async fn release(
    admin: TelemetryAdmin,
    State(st): State<SharedState>,
    headers: HeaderMap,
    Query(aq): Query<ActorQuery>,
    Json(b): Json<ReleaseBody>,
) -> AdminResult {
    if b.release.is_empty() {
        return Err(bad("release required"));
    }
    if !matches!(b.state.as_str(), "active" | "archived" | "broken") {
        return Err(bad("state must be active|archived|broken"));
    }
    let operator = operator_of(&headers, &aq);
    sqlx::query(sqlx::AssertSqlSafe(audited(
        "INSERT INTO telemetry.release_state (release, state, note, updated_at) VALUES ($1, $2, $3, timezone('UTC', now())) \
         ON CONFLICT (release) DO UPDATE SET state = $2, note = $3, updated_at = timezone('UTC', now())",
        3,
    )))
    .bind(&b.release)
    .bind(&b.state)
    .bind(b.note.as_deref().filter(|s| !s.is_empty()))
    .bind(admin.actor())
    .bind("release")
    .bind(&b.release)
    .bind(with_operator(json!({ "release": b.release, "state": b.state, "note": b.note }), &operator))
    .execute(&st.pool)
    .await
    .map_err(|e| db_err("telemetry admin", e))?;
    Ok(Json(
        json!({ "ok": true, "release": b.release, "state": b.state }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fails_closed_when_token_unset() {
        assert!(!token_ok(None, Some("anything")));
        assert!(!token_ok(None, None));
        assert!(
            !token_ok(Some(""), Some("")),
            "an empty configured token accepted an empty bearer"
        );
    }

    #[test]
    fn requires_matching_bearer() {
        assert!(token_ok(Some("tok"), Some("tok")));
        assert!(!token_ok(Some("tok"), Some("nope")));
        assert!(!token_ok(Some("tok"), None));
    }

    #[test]
    fn cookie_token_parses_session_cookie() {
        let mut h = HeaderMap::new();
        h.insert(
            "cookie",
            "other=1; telemetry_admin=tok123; more=2".parse().unwrap(),
        );
        assert_eq!(cookie_token(&h).as_deref(), Some("tok123"));
        let mut empty_val = HeaderMap::new();
        empty_val.insert("cookie", "telemetry_admin=".parse().unwrap());
        assert_eq!(cookie_token(&empty_val), None);
        assert_eq!(cookie_token(&HeaderMap::new()), None);
    }

    #[test]
    fn actor_prefers_console_header_over_query() {
        let mut h = HeaderMap::new();
        h.insert("x-catalyrst-admin", "alice".parse().unwrap());
        let q = ActorQuery {
            actor: Some("mallory".into()),
        };
        assert_eq!(operator_of(&h, &q), "alice");
    }

    #[test]
    fn actor_falls_back_to_query_when_header_absent() {
        let h = HeaderMap::new();
        let q = ActorQuery {
            actor: Some("bob".into()),
        };
        assert_eq!(operator_of(&h, &q), "bob");
    }

    #[test]
    fn actor_defaults_to_loopback() {
        let h = HeaderMap::new();
        let q = ActorQuery::default();
        assert_eq!(operator_of(&h, &q), "loopback");

        let mut h2 = HeaderMap::new();
        h2.insert("x-catalyrst-admin", "   ".parse().unwrap());
        let q2 = ActorQuery {
            actor: Some("  ".into()),
        };
        assert_eq!(operator_of(&h2, &q2), "loopback");
    }

    #[test]
    fn actor_label_truncates_to_100_chars() {
        let mut h = HeaderMap::new();
        let long: String = "x".repeat(250);
        h.insert("x-catalyrst-admin", long.parse().unwrap());
        let q = ActorQuery::default();
        assert_eq!(operator_of(&h, &q).chars().count(), 100);
    }

    #[test]
    fn audited_binds_the_audit_columns_after_the_write() {
        let sql = audited("DELETE FROM telemetry.flag_overrides WHERE flag = $1", 1);
        assert!(
            sql.contains("INSERT INTO ops.admin_audit (actor, domain, action, target, detail)")
        );
        assert!(sql.ends_with("VALUES ($2, 'telemetry', $3, $4, $5)"));
        assert!(
            PURGE_BATCH_SQL.contains("ctid = ANY (ARRAY(") && PURGE_BATCH_SQL.contains("LIMIT $4"),
            "the operator purge is the job's batch shape"
        );
    }

    #[test]
    fn an_unfiltered_bulk_filter_is_refused() {
        let f = BulkFilter {
            source: Some(String::new()),
            project: None,
            fingerprint: None,
            before: None,
            level: None,
        };
        assert_eq!(
            f.require_some().err().map(|e| e.0),
            Some(StatusCode::BAD_REQUEST)
        );
        let f = BulkFilter {
            source: None,
            project: Some("p".into()),
            fingerprint: Some("fp".into()),
            before: None,
            level: None,
        };
        assert!(f.require_some().is_ok());
        assert_eq!(
            f.target(),
            Some("fp"),
            "the fingerprint is the audit target before the project"
        );
    }
}
