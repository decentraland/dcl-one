//! The player report intake (upstream's `report-user` service, at
//! `/api/reports/players`), the node area `serve` nests at `/report-user`:
//! a presign mints the evidence slots, each file is `PUT` to the path the
//! plan named, then the report is filed. Every request is the reporter's
//! own signed fetch over the route path (the nest-stripped path the client
//! signs, or the request path). The moderator token, or a listed moderator
//! wallet, reads the queue and the evidence. Rows: `social.player_reports`
//! and `social.player_report_evidence` (`schemas/40_social.sql`; the slots
//! come first, the report row at filing, so no FK between them); bytes: the
//! shared hash-addressed store. Limits: `DATABASE.md`, "Player report intake
//! workload".

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, OriginalUri, Path, Query, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use catalyst_storage::Storage;
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{PgConnection, Row};
use uuid::Uuid;

use super::auth::{
    ModeratorCredentials, ModeratorMode, authorize_moderator, verify_wallet_signed_fetch,
};
use super::errors::{ApiError, forbidden};
use super::{CommsState, SharedState};
use crate::shared::signed_fetch::SignedPath;
use crate::shared::{cors, nul_guard};

/// The client's limits (`apps/sites/src/data/lib/catalyst/landings/report.ts`).
pub const MAX_FILES: usize = 5;
pub const MAX_FILE_BYTES: usize = 5 * 1024 * 1024;
/// Reports one wallet may file in a rolling hour (`player_reports_reporter_idx`).
pub const REPORTS_PER_HOUR: i64 = 5;
/// Unfiled slots one wallet may hold, counted over the last
/// `PENDING_SLOT_WINDOW_HOURS` (`player_report_evidence_reporter_idx`).
pub const PENDING_SLOTS_PER_REPORTER: i64 = 25;
pub const PENDING_SLOT_WINDOW_HOURS: i32 = 24;
const TEXT_MAX: usize = 500;
const JSON_BODY_LIMIT: usize = 64 * 1024;
const REASONS: [&str; 5] = [
    "scam_phishing",
    "illegal_content",
    "harassment",
    "cheating",
    "impersonation",
];
/// The form's accepted evidence types.
const ACCEPTED_TYPES: [&str; 7] = [
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "video/mp4",
    "video/webm",
    "application/pdf",
];

pub struct ReportsState {
    pub comms: SharedState,
    pub storage: Arc<Storage>,
}

type Shared = Arc<ReportsState>;

/// The area's router, for `serve` to nest at `/report-user`.
pub fn router(comms: SharedState, storage: Arc<Storage>) -> Router {
    let state = Arc::new(ReportsState { comms, storage });
    Router::new()
        .route(
            "/api/reports/players/presign",
            post(presign).layer(DefaultBodyLimit::max(JSON_BODY_LIMIT)),
        )
        .route(
            "/api/reports/players",
            post(create).layer(DefaultBodyLimit::max(JSON_BODY_LIMIT)),
        )
        .route(
            "/api/reports/players/{id}/evidence/{key}",
            put(upload).layer(DefaultBodyLimit::max(MAX_FILE_BYTES + 1)),
        )
        .route("/api/reports", get(list))
        .route("/api/reports/{id}/evidence/{key}", get(evidence))
        .layer(axum::middleware::from_fn(nul_guard::nul_guard_middleware))
        .layer(axum::middleware::from_fn(cors::cors_middleware))
        .with_state(state)
}

fn upload_path(id: Uuid, key: &str) -> String {
    format!("/api/reports/players/{id}/evidence/{key}")
}

/// The key's filename part: ASCII letters, digits, `.`, `_` and `-`, at most 100 chars.
fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .take(100)
        .collect();
    let cleaned = cleaned.trim_matches('.').to_string();
    if cleaned.is_empty() {
        "file".to_string()
    } else {
        cleaned
    }
}

fn parse_body<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, ApiError> {
    serde_json::from_slice(bytes).map_err(|e| ApiError::bad_request(format!("Invalid body: {e}")))
}

fn parse_report_id(raw: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(raw).map_err(|_| ApiError::bad_request("Invalid reportId"))
}

fn is_wallet(raw: &str) -> bool {
    raw.len() == 42 && raw.starts_with("0x") && raw[2..].chars().all(|c| c.is_ascii_hexdigit())
}

fn moderator_creds(state: &CommsState) -> ModeratorCredentials<'_> {
    ModeratorCredentials {
        token: state.settings.moderator_token.as_deref(),
        addresses: &state.settings.moderator_addresses,
    }
}

async fn reporter_of(
    state: &CommsState,
    headers: &HeaderMap,
    method: &str,
    original: &Uri,
    local: &Uri,
) -> Result<String, ApiError> {
    let verified = verify_wallet_signed_fetch(
        headers,
        method,
        &SignedPath::from_uris(original, local),
        state.eip1654(),
    )
    .await?;
    Ok(verified.signer.as_str().to_lowercase())
}

fn rfc3339(ts: NaiveDateTime) -> String {
    DateTime::<Utc>::from_naive_utc_and_offset(ts, Utc).to_rfc3339()
}

/// Serializes one reporter's quota checks against its own concurrent requests.
async fn lock_reporter(conn: &mut PgConnection, reporter: &str) -> Result<(), ApiError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("player-report:{reporter}"))
        .execute(conn)
        .await?;
    Ok(())
}

/// The hourly limit: reports this wallet filed in the last hour.
async fn check_hourly_limit(conn: &mut PgConnection, reporter: &str) -> Result<(), ApiError> {
    let filed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM social.player_reports \
         WHERE reporter_address = $1 \
           AND created_at > timezone('UTC', now()) - interval '1 hour'",
    )
    .bind(reporter)
    .fetch_one(conn)
    .await?;
    if filed >= REPORTS_PER_HOUR {
        return Err(ApiError::http(
            429,
            format!("At most {REPORTS_PER_HOUR} reports per hour"),
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PresignFile {
    filename: String,
    content_type: String,
    file_size: i64,
}

#[derive(Deserialize)]
struct PresignBody {
    files: Vec<PresignFile>,
}

/// `POST /api/reports/players/presign` `{files:[{filename, contentType,
/// fileSize}]}`: one upload slot per file under a fresh report id, for the
/// signer, `{reportId, files:[{key, uploadPath}]}` (`uploadPath` relative to
/// the mount). The report row itself comes at filing.
async fn presign(
    State(state): State<Shared>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let reporter = reporter_of(&state.comms, &headers, "post", &original, &uri).await?;
    let body: PresignBody = parse_body(&body)?;
    if body.files.is_empty() || body.files.len() > MAX_FILES {
        return Err(ApiError::bad_request(format!(
            "files must hold between 1 and {MAX_FILES} entries"
        )));
    }
    let mut slots = Vec::with_capacity(body.files.len());
    for (position, file) in body.files.iter().enumerate() {
        if file.filename.trim().is_empty() || file.filename.chars().count() > 255 {
            return Err(ApiError::bad_request(
                "filename must be 1 to 255 characters",
            ));
        }
        if !ACCEPTED_TYPES.contains(&file.content_type.as_str()) {
            return Err(ApiError::bad_request(format!(
                "contentType {} is not accepted",
                file.content_type
            )));
        }
        if file.file_size < 1 || file.file_size > MAX_FILE_BYTES as i64 {
            return Err(ApiError::bad_request(format!(
                "fileSize must be between 1 and {MAX_FILE_BYTES} bytes"
            )));
        }
        let key = format!("{position}-{}", sanitize_filename(&file.filename));
        slots.push((key, file));
    }

    let id = Uuid::new_v4();
    let mut tx = state.comms.pool.begin().await?;
    lock_reporter(&mut tx, &reporter).await?;
    check_hourly_limit(&mut tx, &reporter).await?;
    let pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM social.player_report_evidence e \
         WHERE e.reporter_address = $1 \
           AND e.created_at > timezone('UTC', now()) - $2::int * interval '1 hour' \
           AND NOT EXISTS (SELECT 1 FROM social.player_reports r WHERE r.id = e.report_id)",
    )
    .bind(&reporter)
    .bind(PENDING_SLOT_WINDOW_HOURS)
    .fetch_one(&mut *tx)
    .await?;
    if pending + slots.len() as i64 > PENDING_SLOTS_PER_REPORTER {
        return Err(ApiError::http(
            429,
            format!("At most {PENDING_SLOTS_PER_REPORTER} unsubmitted evidence files at a time"),
        ));
    }
    for (key, file) in &slots {
        sqlx::query(
            "INSERT INTO social.player_report_evidence \
             (report_id, evidence_key, reporter_address, filename, content_type, declared_size) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(id)
        .bind(key)
        .bind(&reporter)
        .bind(file.filename.trim())
        .bind(&file.content_type)
        .bind(file.file_size)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    let files: Vec<Value> = slots
        .iter()
        .map(|(key, _)| json!({ "key": key, "uploadPath": upload_path(id, key) }))
        .collect();
    Ok(Json(json!({ "reportId": id.to_string(), "files": files })))
}

/// `PUT /api/reports/players/{id}/evidence/{key}`: the file of one presigned
/// slot, by the slot's signer while the report is not filed (a repeat
/// replaces the bytes); the bytes go to the shared store under their
/// content hash.
async fn upload(
    State(state): State<Shared>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
    Path((id, key)): Path<(String, String)>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let reporter = reporter_of(&state.comms, &headers, "put", &original, &uri).await?;
    let id = parse_report_id(&id)?;
    let row = sqlx::query(
        "SELECT e.reporter_address, e.content_type, e.declared_size, \
                EXISTS (SELECT 1 FROM social.player_reports r WHERE r.id = e.report_id) AS filed \
         FROM social.player_report_evidence e \
         WHERE e.report_id = $1 AND e.evidence_key = $2",
    )
    .bind(id)
    .bind(&key)
    .fetch_optional(&state.comms.pool)
    .await?
    .ok_or_else(|| ApiError::not_found("Unknown report or evidence key"))?;
    if row.get::<String, _>("reporter_address") != reporter {
        return Err(forbidden("The report belongs to another wallet"));
    }
    if row.get::<bool, _>("filed") {
        return Err(ApiError::http(409, "The report is already submitted"));
    }
    let declared: String = row.get("content_type");
    let sent = headers
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| {
            v.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
        })
        .unwrap_or_default();
    if sent != declared {
        return Err(ApiError::bad_request(format!(
            "content-type must be {declared}"
        )));
    }
    let declared_size: i64 = row.get("declared_size");
    if body.len() as i64 != declared_size {
        return Err(ApiError::bad_request(format!(
            "the file must hold the {declared_size} bytes the presign declared"
        )));
    }

    let hash = crate::auth::hash_bytes_v1(&body);
    state.storage.store(&hash, body).await.map_err(|e| {
        tracing::error!(error = %e, "storing report evidence");
        ApiError::internal("Internal Server Error")
    })?;
    sqlx::query(
        "UPDATE social.player_report_evidence \
         SET content_hash = $3, uploaded_at = timezone('UTC', now()) \
         WHERE report_id = $1 AND evidence_key = $2",
    )
    .bind(id)
    .bind(&key)
    .bind(&hash)
    .execute(&state.comms.pool)
    .await?;
    Ok(Json(json!({ "ok": true, "key": key })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateBody {
    report_id: String,
    reported_address: String,
    reason: String,
    description: String,
    #[serde(default)]
    additional_comments: Option<String>,
    #[serde(default)]
    confirm_accuracy: bool,
    evidence_keys: Vec<String>,
}

/// `POST /api/reports/players`: files the presigned report once, by the
/// slots' signer, naming every presigned (and uploaded) evidence key;
/// `{data:{id, evidenceKeys}}`.
async fn create(
    State(state): State<Shared>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let reporter = reporter_of(&state.comms, &headers, "post", &original, &uri).await?;
    let body: CreateBody = parse_body(&body)?;
    let id = parse_report_id(&body.report_id)?;
    let reported = body.reported_address.trim().to_lowercase();
    if !is_wallet(&reported) {
        return Err(ApiError::bad_request(
            "reportedAddress must be a wallet address",
        ));
    }
    if !REASONS.contains(&body.reason.as_str()) {
        return Err(ApiError::bad_request(
            "reason is not one of the report reasons",
        ));
    }
    let description = body.description.trim();
    if description.is_empty() || description.chars().count() > TEXT_MAX {
        return Err(ApiError::bad_request(format!(
            "description must be 1 to {TEXT_MAX} characters"
        )));
    }
    let comments = body
        .additional_comments
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty());
    if comments.is_some_and(|c| c.chars().count() > TEXT_MAX) {
        return Err(ApiError::bad_request(format!(
            "additionalComments must be at most {TEXT_MAX} characters"
        )));
    }
    if !body.confirm_accuracy {
        return Err(ApiError::bad_request("confirmAccuracy must be true"));
    }

    let mut tx = state.comms.pool.begin().await?;
    let rows = sqlx::query(
        "SELECT evidence_key, reporter_address, content_hash IS NOT NULL AS uploaded \
         FROM social.player_report_evidence WHERE report_id = $1 \
         ORDER BY evidence_key FOR UPDATE",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    if rows.is_empty() {
        return Err(ApiError::not_found("Unknown reportId"));
    }
    if rows
        .iter()
        .any(|r| r.get::<String, _>("reporter_address") != reporter)
    {
        return Err(forbidden("The report belongs to another wallet"));
    }
    let filed: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM social.player_reports WHERE id = $1)")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
    if filed {
        return Err(ApiError::http(409, "The report is already submitted"));
    }
    let keys: Vec<String> = rows.iter().map(|r| r.get("evidence_key")).collect();
    let mut named = body.evidence_keys.clone();
    named.sort();
    named.dedup();
    if named != keys {
        return Err(ApiError::bad_request(
            "evidenceKeys must name every presigned file and nothing else",
        ));
    }
    if let Some(missing) = rows.iter().find(|r| !r.get::<bool, _>("uploaded")) {
        return Err(ApiError::bad_request(format!(
            "evidence {} was not uploaded",
            missing.get::<String, _>("evidence_key")
        )));
    }
    lock_reporter(&mut tx, &reporter).await?;
    check_hourly_limit(&mut tx, &reporter).await?;
    sqlx::query(
        "INSERT INTO social.player_reports \
         (id, reporter_address, reported_address, reason, description, additional_comments, \
          evidence_keys, status) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'open')",
    )
    .bind(id)
    .bind(&reporter)
    .bind(&reported)
    .bind(&body.reason)
    .bind(description)
    .bind(comments)
    .bind(&keys)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(
        json!({ "data": { "id": id.to_string(), "evidenceKeys": keys } }),
    ))
}

#[derive(Deserialize, Default)]
struct ListQuery {
    limit: Option<i64>,
    offset: Option<i64>,
    reported: Option<String>,
}

/// `GET /api/reports?limit=&offset=&reported=`: the moderator queue, filed
/// reports newest first with their evidence, `{data:[…], total}`.
async fn list(
    State(state): State<Shared>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    authorize_moderator(
        &moderator_creds(&state.comms),
        &headers,
        "get",
        &SignedPath::from_uris(&original, &uri),
        ModeratorMode::Read,
        None,
        state.comms.eip1654(),
    )
    .await?;
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let offset = q.offset.unwrap_or(0).max(0);
    let reported = q
        .reported
        .as_deref()
        .map(|r| r.trim().to_lowercase())
        .filter(|r| !r.is_empty());
    if reported.as_deref().is_some_and(|r| !is_wallet(r)) {
        return Err(ApiError::bad_request("reported must be a wallet address"));
    }
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM social.player_reports \
         WHERE ($1::text IS NULL OR reported_address = $1)",
    )
    .bind(&reported)
    .fetch_one(&state.comms.pool)
    .await?;
    let reports = sqlx::query(
        "SELECT id, reporter_address, reported_address, reason, description, \
                additional_comments, evidence_keys, status, created_at \
         FROM social.player_reports \
         WHERE ($1::text IS NULL OR reported_address = $1) \
         ORDER BY created_at DESC, id LIMIT $2 OFFSET $3",
    )
    .bind(&reported)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.comms.pool)
    .await?;
    let ids: Vec<Uuid> = reports.iter().map(|r| r.get("id")).collect();
    let evidence = sqlx::query(
        "SELECT report_id, evidence_key, filename, content_type, declared_size, content_hash, \
                uploaded_at \
         FROM social.player_report_evidence WHERE report_id = ANY($1)",
    )
    .bind(&ids)
    .fetch_all(&state.comms.pool)
    .await?;
    let data: Vec<Value> = reports
        .iter()
        .map(|r| {
            let id: Uuid = r.get("id");
            let keys: Vec<String> = r.get("evidence_keys");
            let files: Vec<Value> = keys
                .iter()
                .filter_map(|key| {
                    evidence.iter().find(|e| {
                        e.get::<Uuid, _>("report_id") == id
                            && e.get::<String, _>("evidence_key") == *key
                    })
                })
                .map(|e| {
                    json!({
                        "key": e.get::<String, _>("evidence_key"),
                        "filename": e.get::<String, _>("filename"),
                        "contentType": e.get::<String, _>("content_type"),
                        "declaredSize": e.get::<i64, _>("declared_size"),
                        "contentHash": e.get::<Option<String>, _>("content_hash"),
                        "uploadedAt": e.get::<Option<NaiveDateTime>, _>("uploaded_at").map(rfc3339),
                    })
                })
                .collect();
            json!({
                "id": id.to_string(),
                "reporter": r.get::<String, _>("reporter_address"),
                "reportedAddress": r.get::<String, _>("reported_address"),
                "reason": r.get::<String, _>("reason"),
                "description": r.get::<String, _>("description"),
                "additionalComments": r.get::<Option<String>, _>("additional_comments"),
                "status": r.get::<String, _>("status"),
                "createdAt": rfc3339(r.get("created_at")),
                "evidence": files,
            })
        })
        .collect();
    Ok(Json(json!({ "data": data, "total": total })))
}

/// `GET /api/reports/{id}/evidence/{key}`: one uploaded file, for a moderator.
async fn evidence(
    State(state): State<Shared>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
    Path((id, key)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    authorize_moderator(
        &moderator_creds(&state.comms),
        &headers,
        "get",
        &SignedPath::from_uris(&original, &uri),
        ModeratorMode::Read,
        None,
        state.comms.eip1654(),
    )
    .await?;
    let id = parse_report_id(&id)?;
    let row = sqlx::query(
        "SELECT content_type, content_hash FROM social.player_report_evidence \
         WHERE report_id = $1 AND evidence_key = $2 AND content_hash IS NOT NULL",
    )
    .bind(id)
    .bind(&key)
    .fetch_optional(&state.comms.pool)
    .await?
    .ok_or_else(|| ApiError::not_found("Unknown or unuploaded evidence"))?;
    let hash: String = row.get("content_hash");
    let bytes = state
        .storage
        .retrieve(&hash)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "reading report evidence");
            ApiError::internal("Internal Server Error")
        })?
        .ok_or_else(|| ApiError::not_found("Evidence bytes are gone"))?;
    let content_type: String = row.get("content_type");
    Ok((
        StatusCode::OK,
        [
            (CONTENT_TYPE, content_type),
            (CACHE_CONTROL, "private, no-store".to_string()),
        ],
        bytes,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::Router;
    use axum::body::Body;
    use axum::http::{HeaderMap, Request, StatusCode};
    use serde_json::{Value, json};
    use tower::ServiceExt;
    use uuid::Uuid;

    use super::super::auth::test_support::{OUTSIDER_KEY, address_of, random_key, signed};
    use super::super::{CommsState, Settings};
    use super::*;
    use crate::http::Outbound;
    use crate::interfaces::test_support::{StubDirectory, StubPolicy, StubScenes};

    const MOUNT: &str = "/report-user";
    const PRESIGN: &str = "/api/reports/players/presign";
    const CREATE: &str = "/api/reports/players";
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n-not-really-a-png-";

    async fn app() -> Option<(Router, std::path::PathBuf)> {
        let pool = crate::db::scratch::pool(4).await?;
        let dir = std::env::temp_dir().join(format!(
            "catalyst-player-reports-{}",
            Uuid::new_v4().simple()
        ));
        let storage = Arc::new(
            Storage::from_config(&dir)
                .await
                .expect("a scratch blob store"),
        );
        let comms = Arc::new(CommsState::new(
            pool,
            Outbound::for_tests(),
            Settings {
                moderator_token: Some("moderator-token".into()),
                ..Settings::for_tests()
            },
            None,
            None,
            Arc::new(StubPolicy::default()),
            Arc::new(StubScenes::default()),
            Arc::new(crate::presence::Presence::new()),
            Arc::new(StubDirectory::default()),
        ));
        Some((Router::new().nest(MOUNT, router(comms, storage)), dir))
    }

    /// A request at the mount; `signed_at_mount` signs it, as the client
    /// does, over the path it is sent to.
    async fn call(
        app: &Router,
        method: &str,
        local: &str,
        headers: HeaderMap,
        content_type: &str,
        body: Vec<u8>,
    ) -> (StatusCode, Value, Vec<u8>) {
        let mut request = Request::builder()
            .method(method.to_uppercase().as_str())
            .uri(format!("{MOUNT}{local}"))
            .header("content-type", content_type);
        request.headers_mut().unwrap().extend(headers);
        let response = app
            .clone()
            .oneshot(request.body(Body::from(body)).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), MAX_FILE_BYTES + 1024)
            .await
            .unwrap()
            .to_vec();
        let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, value, bytes)
    }

    async fn signed_at_mount(key: &str, method: &str, local: &str) -> HeaderMap {
        signed(key, method, &format!("{MOUNT}{local}"), "{}").await
    }

    async fn presign_one(app: &Router, key: &str) -> (String, String, String) {
        let body = json!({ "files": [{ "filename": "shot one.png", "contentType": "image/png", "fileSize": PNG.len() }] });
        let headers = signed_at_mount(key, "post", PRESIGN).await;
        let (status, value, _) = call(
            app,
            "post",
            PRESIGN,
            headers,
            "application/json",
            body.to_string().into_bytes(),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{value}");
        let report_id = value["reportId"].as_str().unwrap().to_string();
        let slot = &value["files"][0];
        (
            report_id,
            slot["key"].as_str().unwrap().to_string(),
            slot["uploadPath"].as_str().unwrap().to_string(),
        )
    }

    /// Presigns, uploads and files one report; the filing's status and body.
    async fn file_one(app: &Router, key: &str, reported: &str) -> (StatusCode, Value) {
        let (report_id, slot, upload_path) = presign_one(app, key).await;
        let headers = signed_at_mount(key, "put", &upload_path).await;
        let (status, value, _) =
            call(app, "put", &upload_path, headers, "image/png", PNG.to_vec()).await;
        assert_eq!(status, StatusCode::OK, "{value}");
        let headers = signed_at_mount(key, "post", CREATE).await;
        let (status, value, _) = call(
            app,
            "post",
            CREATE,
            headers,
            "application/json",
            create_body(&report_id, reported, &[&slot]),
        )
        .await;
        (status, value)
    }

    fn create_body(report_id: &str, reported: &str, keys: &[&str]) -> Vec<u8> {
        json!({
            "reportId": report_id,
            "reportedAddress": reported,
            "reason": "harassment",
            "description": "  Followed me around shouting.  ",
            "additionalComments": "",
            "confirmAccuracy": true,
            "evidenceKeys": keys,
        })
        .to_string()
        .into_bytes()
    }

    fn bearer(token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", format!("Bearer {token}").parse().unwrap());
        headers
    }

    fn fresh_wallet() -> String {
        format!("0x{}", &Uuid::new_v4().simple().to_string().repeat(2)[..40])
    }

    #[tokio::test]
    async fn presign_upload_create_then_moderator_reads() {
        let Some((app, dir)) = app().await else {
            eprintln!("SKIP: player reports need CATALYST_TEST_DATABASE_URL");
            return;
        };
        let reporter_key = random_key();
        let (report_id, key, upload_path) = presign_one(&app, &reporter_key).await;
        assert_eq!(key, "0-shot_one.png");
        assert_eq!(upload_path, upload_path_of(&report_id, &key));

        // The bytes must be what the presign declared.
        let headers = signed_at_mount(&reporter_key, "put", &upload_path).await;
        let (status, _, _) = call(
            &app,
            "put",
            &upload_path,
            headers,
            "image/png",
            PNG[1..].to_vec(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let headers = signed_at_mount(&reporter_key, "put", &upload_path).await;
        let (status, value, _) = call(
            &app,
            "put",
            &upload_path,
            headers,
            "image/png",
            PNG.to_vec(),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{value}");

        let reported = fresh_wallet();
        let headers = signed_at_mount(&reporter_key, "post", CREATE).await;
        let (status, value, _) = call(
            &app,
            "post",
            CREATE,
            headers,
            "application/json",
            create_body(
                &report_id,
                &reported.to_uppercase().replace("0X", "0x"),
                &[&key],
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{value}");
        assert_eq!(value["data"]["id"], report_id);
        assert_eq!(value["data"]["evidenceKeys"], json!([key]));

        // A second filing, or another upload, of a filed report is refused.
        let headers = signed_at_mount(&reporter_key, "post", CREATE).await;
        let (status, _, _) = call(
            &app,
            "post",
            CREATE,
            headers,
            "application/json",
            create_body(&report_id, &reported, &[&key]),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        let headers = signed_at_mount(&reporter_key, "put", &upload_path).await;
        let (status, _, _) = call(
            &app,
            "put",
            &upload_path,
            headers,
            "image/png",
            PNG.to_vec(),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);

        // The moderator token reads the queue; nobody else does.
        let list_path = format!("/api/reports?reported={reported}");
        let (status, _, _) = call(&app, "get", &list_path, HeaderMap::new(), "", vec![]).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, value, _) = call(
            &app,
            "get",
            &list_path,
            bearer("moderator-token"),
            "",
            vec![],
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{value}");
        assert_eq!(value["total"], 1);
        let row = &value["data"][0];
        assert_eq!(row["id"], report_id);
        assert_eq!(row["reporter"], address_of(&reporter_key).to_lowercase());
        assert_eq!(row["reportedAddress"], reported);
        assert_eq!(row["reason"], "harassment");
        assert_eq!(row["description"], "Followed me around shouting.");
        assert_eq!(row["additionalComments"], Value::Null);
        assert_eq!(row["status"], "open");
        assert_eq!(row["evidence"][0]["key"], key);
        assert_eq!(row["evidence"][0]["contentType"], "image/png");
        assert_eq!(row["evidence"][0]["declaredSize"], PNG.len());
        assert!(row["evidence"][0]["contentHash"].is_string());

        let evidence_path = format!("/api/reports/{report_id}/evidence/{key}");
        let (status, _, bytes) = call(
            &app,
            "get",
            &evidence_path,
            bearer("moderator-token"),
            "",
            vec![],
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(bytes, PNG);
        let (status, _, _) = call(&app, "get", &evidence_path, HeaderMap::new(), "", vec![]).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn another_signer_cannot_upload_or_file_a_presign() {
        let Some((app, dir)) = app().await else {
            eprintln!("SKIP: player reports need CATALYST_TEST_DATABASE_URL");
            return;
        };
        let reporter_key = random_key();
        let (report_id, key, upload_path) = presign_one(&app, &reporter_key).await;

        let headers = signed_at_mount(OUTSIDER_KEY, "put", &upload_path).await;
        let (status, _, _) = call(
            &app,
            "put",
            &upload_path,
            headers,
            "image/png",
            PNG.to_vec(),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        let reported = "0x1111111111111111111111111111111111111111";
        let headers = signed_at_mount(OUTSIDER_KEY, "post", CREATE).await;
        let (status, _, _) = call(
            &app,
            "post",
            CREATE,
            headers,
            "application/json",
            create_body(&report_id, reported, &[&key]),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        // The signer itself cannot file before every upload landed, nor with
        // a key the presign never named, nor under an unknown report id.
        let headers = signed_at_mount(&reporter_key, "post", CREATE).await;
        let (status, _, _) = call(
            &app,
            "post",
            CREATE,
            headers,
            "application/json",
            create_body(&report_id, reported, &[&key]),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let headers = signed_at_mount(&reporter_key, "post", CREATE).await;
        let (status, _, _) = call(
            &app,
            "post",
            CREATE,
            headers,
            "application/json",
            create_body(&report_id, reported, &[&key, "9-other.png"]),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let headers = signed_at_mount(&reporter_key, "post", CREATE).await;
        let (status, _, _) = call(
            &app,
            "post",
            CREATE,
            headers,
            "application/json",
            create_body(&Uuid::new_v4().to_string(), reported, &[&key]),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        // Unsigned requests (no auth chain) never reach the rows.
        let (status, _, _) = call(
            &app,
            "post",
            PRESIGN,
            HeaderMap::new(),
            "application/json",
            b"{\"files\":[]}".to_vec(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn a_wallet_files_at_most_five_reports_an_hour() {
        let Some((app, dir)) = app().await else {
            eprintln!("SKIP: player reports need CATALYST_TEST_DATABASE_URL");
            return;
        };
        let reporter_key = random_key();
        let reported = fresh_wallet();
        // Slots presigned ahead of the limit are refused at filing time.
        let (spare_id, spare_key, spare_upload) = presign_one(&app, &reporter_key).await;
        let headers = signed_at_mount(&reporter_key, "put", &spare_upload).await;
        let (status, value, _) = call(
            &app,
            "put",
            &spare_upload,
            headers,
            "image/png",
            PNG.to_vec(),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{value}");
        for _ in 0..REPORTS_PER_HOUR {
            let (status, value) = file_one(&app, &reporter_key, &reported).await;
            assert_eq!(status, StatusCode::OK, "{value}");
        }
        let headers = signed_at_mount(&reporter_key, "post", CREATE).await;
        let (status, _, _) = call(
            &app,
            "post",
            CREATE,
            headers,
            "application/json",
            create_body(&spare_id, &reported, &[&spare_key]),
        )
        .await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        // A new presign is refused early, with the same status.
        let body = json!({ "files": [{ "filename": "late.png", "contentType": "image/png", "fileSize": PNG.len() }] });
        let headers = signed_at_mount(&reporter_key, "post", PRESIGN).await;
        let (status, _, _) = call(
            &app,
            "post",
            PRESIGN,
            headers,
            "application/json",
            body.to_string().into_bytes(),
        )
        .await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        // The limit is per wallet.
        let (status, value) = file_one(&app, &random_key(), &reported).await;
        assert_eq!(status, StatusCode::OK, "{value}");
        let list_path = format!("/api/reports?reported={reported}");
        let (status, value, _) = call(
            &app,
            "get",
            &list_path,
            bearer("moderator-token"),
            "",
            vec![],
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{value}");
        assert_eq!(value["total"], REPORTS_PER_HOUR + 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    fn upload_path_of(report_id: &str, key: &str) -> String {
        format!("/api/reports/players/{report_id}/evidence/{key}")
    }

    #[test]
    fn filenames_are_sanitized() {
        assert_eq!(sanitize_filename("  ../evil name.PNG "), "_evil_name.PNG");
        assert_eq!(sanitize_filename("..."), "file");
        assert_eq!(sanitize_filename("ünïcode.jpg"), "_n_code.jpg");
    }
}
