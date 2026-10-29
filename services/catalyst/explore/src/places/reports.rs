//! Place reports: `POST /api/report` (signed fetch) answers the URL the
//! client uploads its report document to and records the report; the
//! client's `PUT` of that URL (`/api/report/upload/{filename}`, signed by
//! the same wallet) stores the document; the operator queue (`GET
//! /api/reports`, `PATCH /api/reports/{id}`, the admin bearer) lists and
//! resolves them. The old service's default was an S3 presigned PUT; the
//! document is small JSON, so v2 keeps it in `explore.place_reports.payload`
//! (the old "database" upload mode) instead of a second store.

use axum::Json;
use axum::extract::{OriginalUri, Path, Query, State};
use axum::http::{HeaderMap, Method};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;

use super::admin::RequireAdmin;
use crate::SharedState;
use crate::shared::errors::{ApiData, ApiDataTotal, ApiError};

#[derive(Debug, Clone, Serialize)]
pub struct ReportRow {
    pub id: i64,
    pub entity_id: Option<String>,
    pub reporter: String,
    pub signed_url: String,
    pub filename: String,
    pub payload: Value,
    pub status: String,
    pub resolution: Option<String>,
    pub moderator_notes: Option<String>,
    pub resolved_by: Option<String>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

const REPORT_COLUMNS: &str = "id, entity_id, reporter, signed_url, filename, payload, status, resolution, moderator_notes, resolved_by, resolved_at, created_at";

fn row_to_report(r: sqlx::postgres::PgRow) -> ReportRow {
    ReportRow {
        id: r.get("id"),
        entity_id: r.get("entity_id"),
        reporter: r.get("reporter"),
        signed_url: r.get("signed_url"),
        filename: r.get("filename"),
        payload: r.get("payload"),
        status: r.get("status"),
        resolution: r.get("resolution"),
        moderator_notes: r.get("moderator_notes"),
        resolved_by: r.get("resolved_by"),
        resolved_at: r
            .get::<Option<NaiveDateTime>, _>("resolved_at")
            .map(|t| t.and_utc()),
        created_at: r.get::<NaiveDateTime, _>("created_at").and_utc(),
    }
}

/// The upstream key shape: the reporter's last 8 hex chars, the second in
/// hex, then 128 random bits, so a key is never derivable from a public
/// address and a time.
fn report_filename(address: &str, now_seconds: i64) -> String {
    let chars: Vec<char> = address.chars().collect();
    let user_hash: String = chars[chars.len().saturating_sub(8)..]
        .iter()
        .collect::<String>()
        .to_lowercase();
    format!(
        "{user_hash}{now_seconds:x}{:032x}.json",
        rand::random::<u128>()
    )
}

fn request_base_url(headers: &HeaderMap) -> String {
    let host = headers
        .get("x-forwarded-host")
        .or_else(|| headers.get("host"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("127.0.0.1")
        .to_string();
    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .unwrap_or_else(|| {
            if host.starts_with("127.0.0.1") || host.starts_with("localhost") {
                "http".into()
            } else {
                "https".into()
            }
        });
    format!("{scheme}://{host}")
}

/// The public path of this route: a prefix-stripping proxy's
/// `x-original-path` when it ends with the route, else the route.
fn public_path<'a>(headers: &'a HeaderMap, route: &'a str) -> &'a str {
    headers
        .get("x-original-path")
        .and_then(|v| v.to_str().ok())
        .map(|p| p.split('?').next().unwrap_or(p))
        .filter(|p| p.starts_with('/') && !p.starts_with("//") && p.ends_with(route))
        .unwrap_or(route)
}

/// `POST /api/report`: `{ entity_id | place_id, ... }` -> `{ signed_url }`.
pub async fn post_report(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Option<Json<Value>>,
) -> Result<Json<Value>, ApiError> {
    state.user_writes()?;
    if let Some(Json(env)) = body.as_ref().filter(|Json(v)| catalyst_fed::is_envelope(v)) {
        return fed_report(&state, &headers, env.clone()).await;
    }
    let reporter = state.signer(&headers, method.as_str(), uri.path()).await?;
    let payload = body.map(|Json(v)| v).unwrap_or_else(|| json!({}));
    let entity_id = payload
        .get("entity_id")
        .and_then(Value::as_str)
        .or_else(|| payload.get("place_id").and_then(Value::as_str))
        .map(str::to_string);
    let now = Utc::now();
    let filename = report_filename(&reporter, now.timestamp());
    let signed_url = format!(
        "{}{}/upload/{filename}",
        request_base_url(&headers),
        public_path(&headers, uri.path())
    );
    sqlx::query("INSERT INTO explore.place_reports (entity_id, reporter, signed_url, filename, payload, created_at) VALUES ($1, $2, $3, $4, $5, $6)")
        .bind(entity_id.as_deref())
        .bind(&reporter)
        .bind(&signed_url)
        .bind(&filename)
        .bind(&payload)
        .bind(now.naive_utc())
        .execute(&state.pool)
        .await?;
    Ok(Json(
        json!({ "ok": true, "data": { "signed_url": signed_url } }),
    ))
}

/// A `PlaceReport` envelope: logged and queued (its digest is the report's
/// key; a replayed digest is not queued twice) in one transaction.
async fn fed_report(
    state: &SharedState,
    headers: &HeaderMap,
    body: Value,
) -> Result<Json<Value>, ApiError> {
    use crate::fed;
    let (signed, signer) =
        fed::preflight::<fed::PlaceReport>(state, headers, body, fed::Scope::Places).await?;
    let place_id = signed.message.place_id.clone();
    if state
        .places
        .find_by_id_for(&place_id, None)
        .await?
        .is_none()
    {
        return Err(ApiError::not_found(format!(
            "Not found place \"{place_id}\""
        )));
    }
    let hash = signed.signature_hash();
    let reason = serde_json::to_value(signed.message.reason).unwrap_or_else(|_| json!("other"));
    let logged =
        json!({ "place_id": place_id, "reason": reason, "signed_at": signed.message.signed_at });
    let mut tx = state.pool.begin().await?;
    fed::record_action(
        &mut tx,
        &fed::Action {
            scope: fed::Scope::Places,
            signature_hash: &hash,
            signer: &signer,
            subject_id: Some(&place_id),
            action_type: "report",
            payload: &logged,
            signed_at: signed.signed_at,
        },
    )
    .await?;
    sqlx::query("INSERT INTO explore.place_reports (entity_id, reporter, signed_url, filename, payload, created_at) VALUES ($1, $2, '', $3, $4, $5) ON CONFLICT (filename) DO NOTHING")
        .bind(&place_id)
        .bind(&signer)
        .bind(&hash)
        .bind(json!({ "entity_id": place_id, "reason": reason, "signed": true, "signature_hash": hash }))
        .bind(Utc::now().naive_utc())
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(
        json!({ "ok": true, "signature_hash": hash, "data": { "place_id": place_id } }),
    ))
}

/// `PUT /api/report/upload/{filename}`: the report document, by its reporter only.
pub async fn put_report_upload(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(filename): Path<String>,
    body: Option<Json<Value>>,
) -> Result<Json<Value>, ApiError> {
    state.user_writes()?;
    let reporter = state.signer(&headers, method.as_str(), uri.path()).await?;
    let payload = body.map(|Json(v)| v).unwrap_or_else(|| json!({}));
    let updated = sqlx::query(
        "UPDATE explore.place_reports SET payload = $3 WHERE filename = $1 AND reporter = $2",
    )
    .bind(&filename)
    .bind(&reporter)
    .bind(&payload)
    .execute(&state.pool)
    .await?
    .rows_affected();
    if updated == 0 {
        return Err(ApiError::not_found("report not found"));
    }
    Ok(Json(json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
pub struct ReportQuery {
    pub status: Option<String>,
    pub entity_id: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// `GET /api/reports?status=&entity_id=&limit=&offset=`: newest first.
pub async fn get_reports(
    State(state): State<SharedState>,
    _admin: RequireAdmin,
    Query(q): Query<ReportQuery>,
) -> Result<Json<ApiDataTotal<ReportRow>>, ApiError> {
    let (rows, total) = list_reports(&state.pool, &q).await?;
    Ok(Json(ApiDataTotal::ok(rows, total)))
}

/// The reports page and its total (the route's and the operator
/// console's `places/reports` call).
pub async fn list_reports(
    pool: &sqlx::PgPool,
    q: &ReportQuery,
) -> Result<(Vec<ReportRow>, i64), sqlx::Error> {
    let status = q.status.as_deref().filter(|s| !s.is_empty());
    let entity_id = q.entity_id.as_deref().filter(|s| !s.is_empty());
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let offset = q.offset.unwrap_or(0).max(0);
    let sql = format!(
        "SELECT {REPORT_COLUMNS}, count(*) OVER () AS total_count FROM explore.place_reports
         WHERE ($1::text IS NULL OR status = $1) AND ($2::text IS NULL OR entity_id = $2)
         ORDER BY created_at DESC, id DESC LIMIT $3 OFFSET $4"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(status)
        .bind(entity_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
    let total = match rows.first() {
        Some(r) => r.get::<i64, _>("total_count"),
        None if offset > 0 => {
            sqlx::query_scalar("SELECT count(*) FROM explore.place_reports WHERE ($1::text IS NULL OR status = $1) AND ($2::text IS NULL OR entity_id = $2)")
                .bind(status)
                .bind(entity_id)
                .fetch_one(pool)
                .await?
        }
        None => 0,
    };
    Ok((rows.into_iter().map(row_to_report).collect(), total))
}

#[derive(Debug, Deserialize)]
pub struct ReportPatch {
    pub status: String,
    pub resolution: Option<String>,
    pub notes: Option<String>,
    pub resolved_by: Option<String>,
}

pub const REPORT_STATUSES: [&str; 4] = ["open", "resolved", "dismissed", "actioned"];

/// `PATCH /api/reports/{id}` `{ status, resolution?, notes?, resolved_by? }`.
pub async fn patch_report(
    State(state): State<SharedState>,
    _admin: RequireAdmin,
    Path(id): Path<i64>,
    Json(body): Json<ReportPatch>,
) -> Result<Json<ApiData<ReportRow>>, ApiError> {
    state.operator_writes()?;
    Ok(Json(ApiData::ok(
        resolve_report(&state.pool, id, &body).await?,
    )))
}

/// Settles one report (the route's and the operator console's
/// `places/report-resolve` call); the caller holds the write switch.
pub async fn resolve_report(
    pool: &sqlx::PgPool,
    id: i64,
    body: &ReportPatch,
) -> Result<ReportRow, ApiError> {
    let status = body.status.trim();
    if !REPORT_STATUSES.contains(&status) {
        return Err(ApiError::bad_request(
            "status must be one of: open, resolved, dismissed, actioned",
        ));
    }
    let sql = format!(
        "UPDATE explore.place_reports SET status = $2, resolution = COALESCE($3, resolution),
             moderator_notes = COALESCE($4, moderator_notes), resolved_by = $5,
             resolved_at = CASE WHEN $2 <> 'open' THEN $6 ELSE NULL END
         WHERE id = $1 RETURNING {REPORT_COLUMNS}"
    );
    let row = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(status)
        .bind(body.resolution.as_deref())
        .bind(body.notes.as_deref())
        .bind(body.resolved_by.as_deref())
        .bind(Utc::now().naive_utc())
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found(format!("Report {id} not found")))?;
    Ok(row_to_report(row))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_keeps_upstream_prefix() {
        let f = report_filename("0x1234567890ABCDEF1234567890abcdef12345678", 0x6500_0000);
        assert!(f.starts_with("1234567865000000"), "{f}");
        assert!(f.ends_with(".json"), "{f}");
    }

    #[test]
    fn filename_is_not_derivable_from_a_public_address_and_second() {
        let addr = "0x1234567890ABCDEF1234567890abcdef12345678";
        let (first, second) = (
            report_filename(addr, 0x6500_0000),
            report_filename(addr, 0x6500_0000),
        );
        assert_ne!(
            first, second,
            "same reporter and second must not mint the same report key"
        );
        assert!(first.len() >= "1234567865000000".len() + 32 + ".json".len());
    }

    #[test]
    fn the_upload_url_keeps_the_public_prefix_a_proxy_forwarded() {
        let mut headers = HeaderMap::new();
        headers.insert("host", "node.example".parse().unwrap());
        assert_eq!(public_path(&headers, "/api/report"), "/api/report");
        headers.insert("x-original-path", "/places/api/report?x=1".parse().unwrap());
        assert_eq!(public_path(&headers, "/api/report"), "/places/api/report");
        headers.insert("x-original-path", "/elsewhere".parse().unwrap());
        assert_eq!(public_path(&headers, "/api/report"), "/api/report");
        assert_eq!(request_base_url(&headers), "https://node.example");
    }
}
