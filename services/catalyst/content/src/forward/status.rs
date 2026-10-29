//! The operator's view of one lane: `GET .../forwarding`, `GET .../forwarding/{id}`,
//! `POST .../forwarding/{id}/retry`.

use super::*;

#[derive(sqlx::FromRow)]
pub(super) struct StatusRow {
    id: i64,
    entity_id: String,
    world_name: Option<String>,
    target: String,
    status: String,
    attempts: i32,
    next_attempt_at: NaiveDateTime,
    last_error: Option<String>,
    upstream_status: Option<i16>,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
}

fn rfc3339(t: NaiveDateTime) -> String {
    t.and_utc()
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

impl StatusRow {
    pub(super) fn to_json(&self) -> Value {
        let open = matches!(self.status.as_str(), "queued" | "sending");
        let mut out = json!({
            "entityId": self.entity_id,
            "target": origin(&self.target),
            "status": self.status,
            "attempts": self.attempts,
            "nextAttemptAt": open.then(|| rfc3339(self.next_attempt_at)),
            "lastError": self.last_error,
            "upstreamStatus": self.upstream_status,
            "createdAt": rfc3339(self.created_at),
            "updatedAt": rfc3339(self.updated_at),
        });
        if let Some(world) = &self.world_name {
            out["worldName"] = json!(world);
        }
        out
    }
}

pub(super) fn status_columns(lane: &Lane) -> String {
    format!(
        "id, entity_id, {} AS world_name, target, status, attempts, next_attempt_at, \
         last_error, upstream_status, created_at, updated_at",
        lane.world_column.unwrap_or("NULL::text")
    )
}

/// One entity's forwarding record.
pub async fn status_of(pool: &PgPool, lane: &Lane, entity_id: &str) -> sqlx::Result<Option<Value>> {
    let sql = format!(
        "SELECT {} FROM {} WHERE entity_id = $1",
        status_columns(lane),
        lane.table
    );
    let row: Option<StatusRow> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(entity_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.to_json()))
}

/// `?status=&before=&limit=`: newest first, `before` the previous page's `next`.
#[derive(Debug, Default, Deserialize)]
pub struct ListQuery {
    pub status: Option<String>,
    pub before: Option<i64>,
    pub limit: Option<i64>,
}

impl ListQuery {
    /// The status filter, refused when it names no status.
    pub fn status(&self) -> Result<Option<&'static str>, String> {
        match self.status.as_deref().filter(|s| !s.is_empty()) {
            None => Ok(None),
            Some(s) => STATUSES
                .iter()
                .find(|known| **known == s)
                .map(|known| Some(*known))
                .ok_or_else(|| format!("status must be one of {}", STATUSES.join(", "))),
        }
    }
}

/// A page of a lane's records: `{forwards: [...], next}`, `next` the cursor
/// of the following page (null on the last).
pub async fn list(
    pool: &PgPool,
    lane: &Lane,
    status: Option<&str>,
    before: Option<i64>,
    limit: Option<i64>,
) -> sqlx::Result<Value> {
    let limit = limit.unwrap_or(LIST_DEFAULT).clamp(1, LIST_MAX);
    let sql = format!(
        "SELECT {} FROM {} WHERE ($1::text IS NULL OR status = $1) AND ($2::bigint IS NULL OR id < $2) \
         ORDER BY id DESC LIMIT $3",
        status_columns(lane),
        lane.table
    );
    let rows: Vec<StatusRow> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(status)
        .bind(before)
        .bind(limit)
        .fetch_all(pool)
        .await?;
    let next = (rows.len() as i64 == limit)
        .then(|| rows.last().map(|r| r.id))
        .flatten();
    Ok(json!({
        "forwards": rows.iter().map(StatusRow::to_json).collect::<Vec<_>>(),
        "next": next,
    }))
}

// ── the content lane's routes (`CATALYRST_CONTENT_ADMIN_TOKEN`) ─────────────

fn content_admin(state: &SharedState, headers: &HeaderMap) -> Result<(), Response> {
    crate::auth::admin_bearer::check(state.settings.admin_token.get(), headers).map_err(|refusal| {
        json_error(
            StatusCode::from_u16(refusal.http_status()).unwrap_or(StatusCode::FORBIDDEN),
            "Not authorized",
        )
    })
}

fn internal(e: sqlx::Error) -> Response {
    tracing::error!(error = %e, "forwarding status query failed");
    json_error(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")
}

/// `GET /content/forwarding/{entityId}`: one entity's forwarding record.
pub async fn get_forward(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(entity_id): Path<String>,
) -> Response {
    if let Err(refused) = content_admin(&state, &headers) {
        return refused;
    }
    match status_of(&state.pool, &CONTENT_LANE, &entity_id).await {
        Ok(Some(record)) => Json(record).into_response(),
        Ok(None) => json_error(
            StatusCode::NOT_FOUND,
            "No forwarding record for this entity",
        ),
        Err(e) => internal(e),
    }
}

/// `GET /content/forwarding?status=&before=&limit=`: the records, newest first.
pub async fn list_forwards(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(q): Query<ListQuery>,
) -> Response {
    if let Err(refused) = content_admin(&state, &headers) {
        return refused;
    }
    let status = match q.status() {
        Ok(status) => status,
        Err(e) => return json_error(StatusCode::BAD_REQUEST, &e),
    };
    match list(&state.pool, &CONTENT_LANE, status, q.before, q.limit).await {
        Ok(page) => Json(page).into_response(),
        Err(e) => internal(e),
    }
}

/// `POST /content/forwarding/{entityId}/retry`: [`Forwarder::requeue`]; 409
/// for a record in another state or an entity past the upstream's window,
/// 503 while `CONTENT_FORWARD_UPSTREAM_URL` is unset or content writes are
/// off or held (the worlds twin sits behind the worlds gate).
pub async fn retry_forward(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(entity_id): Path<String>,
) -> Response {
    if let Err(refused) = content_admin(&state, &headers) {
        return refused;
    }
    if let Some(refused) = crate::writes::refuse_switch(&state) {
        return refused;
    }
    let Some(forwarder) = state.forwarder() else {
        return json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Forwarding is off (CONTENT_FORWARD_UPSTREAM_URL is unset)",
        );
    };
    match forwarder.requeue(&entity_id).await {
        Ok(Requeued::Queued(record)) => Json(record).into_response(),
        Ok(Requeued::Unknown) => json_error(
            StatusCode::NOT_FOUND,
            "No forwarding record for this entity",
        ),
        Ok(Requeued::Kept(status)) => {
            json_error(StatusCode::CONFLICT, &Requeued::kept_message(&status))
        }
        Ok(Requeued::TooOld(reason)) => json_error(StatusCode::CONFLICT, &reason),
        Err(e) => internal(e),
    }
}
