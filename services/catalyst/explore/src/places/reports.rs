//! Place reports: `POST /api/report` (signed fetch) answers the URL the
//! client uploads its report document to and records the report; the
//! client's `PUT` of that URL uses an expiring upload capability, so an
//! existing presigned-upload client needs no second wallet signature. Only a
//! digest is retained; the capability is single-use and revoked on resolution. The queue (`GET
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
use sha2::{Digest, Sha256};
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
        signed_url: r
            .get::<String, _>("signed_url")
            .split('?')
            .next()
            .unwrap_or_default()
            .to_owned(),
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
        .filter(|host| {
            !host
                .chars()
                .any(|c| c.is_whitespace() || "/\\?#@".contains(c))
        })
        .unwrap_or("127.0.0.1")
        .to_string();
    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .filter(|scheme| matches!(*scheme, "http" | "https"))
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
    let token = format!("{:032x}", rand::random::<u128>());
    let signed_url = format!(
        "{}{}/upload/{filename}",
        request_base_url(&headers),
        public_path(&headers, uri.path())
    );
    sqlx::query("INSERT INTO explore.place_reports (entity_id, reporter, signed_url, filename, payload, created_at, upload_token_hash) VALUES ($1, $2, $3, $4, $5, $6, $7)")
        .bind(entity_id.as_deref())
        .bind(&reporter)
        .bind(&signed_url)
        .bind(&filename)
        .bind(&payload)
        .bind(now.naive_utc())
        .bind(Sha256::digest(token.as_bytes()).to_vec())
        .execute(&state.pool)
        .await?;
    Ok(Json(
        json!({ "ok": true, "data": { "signed_url": format!("{signed_url}?token={token}") } }),
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

// S3 Expires is seconds: preserve the existing service's 60 * 1000 limit.
const UPLOAD_TTL_SECONDS: f64 = 60_000.0;

#[derive(Debug, Deserialize, Default)]
pub struct UploadQuery {
    token: Option<String>,
}

async fn upload_by_capability(
    pool: &sqlx::PgPool,
    filename: &str,
    token: &str,
    payload: &Value,
) -> Result<u64, sqlx::Error> {
    if token.len() != 32 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Ok(0);
    }
    // Only dedicated hashes authorize anonymous PUT, once and while open. Older
    // signed-fetch reports and federation reports cannot be uploaded this way.
    Ok(sqlx::query(
        "UPDATE explore.place_reports SET payload = $3, upload_token_hash = NULL WHERE filename = $1 \
         AND upload_token_hash = $2 AND status = 'open' \
         AND created_at >= timezone('UTC', clock_timestamp()) - make_interval(secs => $4)",
    )
    .bind(filename)
    .bind(Sha256::digest(token.as_bytes()).to_vec())
    .bind(payload)
    .bind(UPLOAD_TTL_SECONDS)
    .execute(pool)
    .await?
    .rows_affected())
}

/// Accept the issued capability, or the original reporter's signed fetch.
pub async fn put_report_upload(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(filename): Path<String>,
    Query(upload): Query<UploadQuery>,
    body: Option<Json<Value>>,
) -> Result<Json<Value>, ApiError> {
    state.user_writes()?;
    let payload = body.map(|Json(v)| v).unwrap_or_else(|| json!({}));
    if let Some(token) = upload.token {
        if upload_by_capability(&state.pool, &filename, &token, &payload).await? != 0 {
            return Ok(Json(json!({ "ok": true })));
        }
        // A stale capability must not override valid reporter authentication.
    }
    let reporter = state.signer(&headers, method.as_str(), uri.path()).await?;
    let updated = sqlx::query(
        "UPDATE explore.place_reports SET payload = $3, upload_token_hash = NULL WHERE filename = $1 AND reporter = $2 AND status = 'open'",
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
        "UPDATE explore.place_reports SET status = $2, upload_token_hash = CASE WHEN $2 <> 'open' THEN NULL ELSE upload_token_hash END, resolution = COALESCE($3, resolution),
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
    async fn request(
        app: &axum::Router,
        method: &str,
        uri: &str,
        signed: bool,
        payload: Value,
    ) -> (axum::http::StatusCode, Value) {
        use axum::body::{Body, to_bytes};
        use axum::http::Request;
        use tower::ServiceExt;
        let mut req = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json");
        if signed {
            for (key, value) in crate::shared::signed_fetch::test_support::signed_headers(
                &method.to_lowercase(),
                uri.split('?').next().unwrap(),
                "{}",
                true,
            )
            .await
            .iter()
            {
                req = req.header(key, value);
            }
        }
        let response = app
            .clone()
            .oneshot(req.body(Body::from(payload.to_string())).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
        (status, serde_json::from_slice(&body).unwrap())
    }

    #[tokio::test]
    async fn upload_authority_is_private_single_use_open_and_independent_of_signature() {
        use axum::http::StatusCode;
        let pool = crate::db::scratch::pool(4)
            .await
            .expect("report upload regressions require CATALYST_TEST_DATABASE_URL");
        let settings = crate::Settings {
            user_writes_enabled: true,
            ..crate::Settings::for_tests()
        };
        let app = crate::router(std::sync::Arc::new(crate::ExploreState::new(
            pool.clone(),
            settings,
            None,
        )));
        for scenario in [
            "unsigned",
            "signed",
            "invalid",
            "expired",
            "resolved",
            "concurrent",
            "legacy",
        ] {
            let (status, body) = request(
                &app,
                "POST",
                "/api/report",
                true,
                json!({"scenario": scenario}),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{body}");
            let issued = url::Url::parse(body["data"]["signed_url"].as_str().unwrap()).unwrap();
            let token = issued
                .query_pairs()
                .find(|(key, _)| key == "token")
                .unwrap()
                .1
                .into_owned();
            let path = issued.path();
            let filename = path.rsplit('/').next().unwrap();
            let full = format!("{path}?token={token}");
            let row = sqlx::query("SELECT signed_url, upload_token_hash FROM explore.place_reports WHERE filename = $1").bind(filename).fetch_one(&pool).await.unwrap();
            assert!(!row.get::<String, _>("signed_url").contains('?'));
            assert_eq!(
                row.get::<Vec<u8>, _>("upload_token_hash"),
                Sha256::digest(token.as_bytes()).to_vec()
            );
            let (reports, _) = list_reports(
                &pool,
                &ReportQuery {
                    status: None,
                    entity_id: None,
                    limit: None,
                    offset: None,
                },
            )
            .await
            .unwrap();
            assert!(!serde_json::to_string(&reports).unwrap().contains(&token));
            let payload = json!({"reason": scenario});
            assert_eq!(
                request(
                    &app,
                    "PUT",
                    &format!("{path}?token=bad"),
                    false,
                    payload.clone()
                )
                .await
                .0,
                StatusCode::UNAUTHORIZED
            );
            assert_eq!(
                upload_by_capability(
                    &pool,
                    filename,
                    "00000000000000000000000000000000",
                    &payload
                )
                .await
                .unwrap(),
                0
            );
            assert_eq!(
                upload_by_capability(&pool, "another-report", &token, &payload)
                    .await
                    .unwrap(),
                0
            );
            if scenario == "signed" {
                // A valid signature from a different wallet cannot replace the evidence.
                sqlx::query("UPDATE explore.place_reports SET reporter = 'another-wallet' WHERE filename = $1").bind(filename).execute(&pool).await.unwrap();
                assert_eq!(
                    request(&app, "PUT", path, true, payload.clone()).await.0,
                    StatusCode::NOT_FOUND
                );
                sqlx::query("UPDATE explore.place_reports SET reporter = $2 WHERE filename = $1")
                    .bind(filename)
                    .bind(crate::shared::signed_fetch::test_support::address())
                    .execute(&pool)
                    .await
                    .unwrap();
            }
            if scenario == "expired" {
                sqlx::query("UPDATE explore.place_reports SET created_at = timezone('UTC', now()) - interval '1 day' WHERE filename = $1").bind(filename).execute(&pool).await.unwrap();
                assert_eq!(
                    request(&app, "PUT", &full, false, payload.clone()).await.0,
                    StatusCode::UNAUTHORIZED
                );
            }
            if scenario == "legacy" {
                sqlx::query("UPDATE explore.place_reports SET upload_token_hash = NULL, signed_url = $2 WHERE filename = $1").bind(filename).bind(issued.as_str()).execute(&pool).await.unwrap();
                let (rows, _) = list_reports(
                    &pool,
                    &ReportQuery {
                        status: None,
                        entity_id: None,
                        limit: None,
                        offset: None,
                    },
                )
                .await
                .unwrap();
                assert!(!serde_json::to_string(&rows).unwrap().contains(&token));
                assert_eq!(
                    request(&app, "PUT", &full, false, payload.clone()).await.0,
                    StatusCode::UNAUTHORIZED
                );
            }
            if scenario == "resolved" {
                let id =
                    sqlx::query_scalar("SELECT id FROM explore.place_reports WHERE filename = $1")
                        .bind(filename)
                        .fetch_one(&pool)
                        .await
                        .unwrap();
                resolve_report(
                    &pool,
                    id,
                    &ReportPatch {
                        status: "resolved".into(),
                        resolution: None,
                        notes: None,
                        resolved_by: None,
                    },
                )
                .await
                .unwrap();
                assert_eq!(
                    request(&app, "PUT", &full, false, payload.clone()).await.0,
                    StatusCode::UNAUTHORIZED
                );
                assert_eq!(
                    request(&app, "PUT", path, true, payload.clone()).await.0,
                    StatusCode::NOT_FOUND
                );
                resolve_report(
                    &pool,
                    id,
                    &ReportPatch {
                        status: "open".into(),
                        resolution: None,
                        notes: None,
                        resolved_by: None,
                    },
                )
                .await
                .unwrap();
                assert_eq!(
                    request(&app, "PUT", &full, false, payload.clone()).await.0,
                    StatusCode::UNAUTHORIZED
                );
                continue;
            }
            if scenario == "concurrent" {
                let (first, second) = tokio::join!(
                    request(&app, "PUT", &full, false, payload.clone()),
                    request(&app, "PUT", &full, false, payload.clone())
                );
                assert!(matches!(
                    (first.0, second.0),
                    (StatusCode::OK, StatusCode::UNAUTHORIZED)
                        | (StatusCode::UNAUTHORIZED, StatusCode::OK)
                ));
            } else {
                let upload = match scenario {
                    "signed" => path.to_owned(),
                    "invalid" => format!("{path}?token=bad"),
                    _ => full.clone(),
                };
                assert_eq!(
                    request(
                        &app,
                        "PUT",
                        &upload,
                        scenario != "unsigned",
                        payload.clone()
                    )
                    .await
                    .0,
                    StatusCode::OK,
                    "{scenario}"
                );
            }
            assert_eq!(
                request(&app, "PUT", &full, false, json!({"reason":"overwrite"}))
                    .await
                    .0,
                StatusCode::UNAUTHORIZED
            );
            let stored: Value =
                sqlx::query_scalar("SELECT payload FROM explore.place_reports WHERE filename = $1")
                    .bind(filename)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            assert_eq!(stored, payload, "{scenario}");
        }
    }
    #[test]
    fn forwarded_host_cannot_insert_query_or_credentials_into_upload_urls() {
        let mut headers = HeaderMap::new();
        for host in [
            "node.example?token=injected",
            "node.example/path",
            "user@node.example",
            "node.example#fragment",
        ] {
            headers.insert("x-forwarded-host", host.parse().unwrap());
            assert_eq!(request_base_url(&headers), "http://127.0.0.1");
        }
    }

    #[tokio::test]
    async fn capability_migration_scrubs_legacy_tokens_without_granting_authority() {
        let pool = crate::db::scratch::pool(2)
            .await
            .expect("report migration regression requires CATALYST_TEST_DATABASE_URL");
        // Recreate the previous column shape on this disposable test database.
        sqlx::query("ALTER TABLE explore.place_reports DROP COLUMN upload_token_hash")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(r#"INSERT INTO explore.place_reports (filename, reporter, signed_url, payload) VALUES ('legacy', 'reporter', 'https://node.example/upload/legacy?token=secret', '{"reason":"kept"}')"#).execute(&pool).await.unwrap();
        sqlx::query("DELETE FROM meta.schema_steps WHERE step = '0016_report_upload_capabilities'")
            .execute(&pool)
            .await
            .unwrap();
        let applied = crate::db::schema_migrate::migrate(
            &pool,
            crate::db::SCHEMA_SQL,
            crate::db::MIGRATIONS_SQL,
        )
        .await
        .unwrap();
        assert_eq!(applied.steps, vec!["0016_report_upload_capabilities"]);
        assert!(
            crate::db::schema_migrate::migrate(
                &pool,
                crate::db::SCHEMA_SQL,
                crate::db::MIGRATIONS_SQL
            )
            .await
            .unwrap()
            .is_empty()
        );
        let row = sqlx::query("SELECT signed_url, upload_token_hash, payload FROM explore.place_reports WHERE filename = 'legacy'").fetch_one(&pool).await.unwrap();
        assert_eq!(
            row.get::<String, _>("signed_url"),
            "https://node.example/upload/legacy"
        );
        assert!(row.get::<Option<Vec<u8>>, _>("upload_token_hash").is_none());
        assert_eq!(row.get::<Value, _>("payload"), json!({"reason":"kept"}));
    }
}
