//! `/auth-api`: the explorer and web sign-in relay. A client posts a signing
//! request, a wallet page recovers it by id and posts the outcome, and the
//! requester polls for it; a signed identity is parked for a device to
//! pick up once. The state lives in `explore_api.auth_requests`,
//! `auth_identities` and `auth_identity_tombstones` with a short TTL (5 min
//! requests, 1 h identities), so every API replica behind the gateway serves
//! the same requests (DATABASE.md "Auth API relay state workload").

use crate::shared::routing::{erase, get, post};
use axum::Extension;
use axum::body::Body;
use axum::extract::{ConnectInfo, DefaultBodyLimit, Path, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use chrono::{DateTime, Duration, NaiveDateTime, SubsecRound, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::api::Shared as AppState;
use crate::api::internal_error;
use crate::auth::AuthChain;
use crate::shared::client_ip::{
    client_ip, is_internal, is_trusted_proxy, normalize_ip, single_real_ip, source_key,
};

mod identities;
mod validation;

use identities::{create_identity, get_identity};
pub use identities::{sweep_expired_identities, sweep_identity_tombstones};
use validation::{validate_auth_chain, validate_request_message};

const REQUEST_TTL_SECONDS: i64 = 300;
/// An expired request still answers 410 for this long before the sweep drops it.
const REQUEST_EXPIRED_GRACE_SECONDS: i64 = 300;
const MAX_PENDING_REQUESTS: usize = 10_000;
const MAX_REQUESTS_PER_SOURCE: usize = 100;
const DEFAULT_BODY_SIZE_BYTES: usize = 16 * 1024;
const IDENTITY_SWEEP_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);
const CLIENT_HEADER: &str = "x-dcl-client";
const CORRELATION_HEADER: &str = "x-correlation-id";

fn client_tag(headers: &HeaderMap) -> &str {
    headers
        .get(CLIENT_HEADER)
        .and_then(|value| value.to_str().ok())
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 80
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._/-".contains(&byte))
        })
        .unwrap_or("unknown")
}

async fn record_request_telemetry(req: Request, next: Next) -> Response {
    let client = client_tag(req.headers()).to_owned();
    let correlation_id = req
        .headers()
        .get(CORRELATION_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| Uuid::parse_str(value).ok())
        .unwrap_or_else(Uuid::new_v4)
        .to_string();
    let method = req.method().clone();
    let path = req.uri().path().to_owned();
    let mut response = next.run(req).await;
    let status = response.status();
    let response_class = format!("{}xx", status.as_u16() / 100);
    if let Ok(value) = HeaderValue::from_str(&correlation_id) {
        response.headers_mut().insert(CORRELATION_HEADER, value);
    }
    response.headers_mut().insert(
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static(CORRELATION_HEADER),
    );
    tracing::info!(
        target: "catalyst::auth_api_request",
        %client,
        %correlation_id,
        %method,
        %path,
        status = status.as_u16(),
        %response_class,
        "auth-api request"
    );
    response
}

/// Prunes expired requests and identities and aged tombstones on an
/// interval instead of on every POST: one periodic job on every replica.
/// Each pass is a set of idempotent `DELETE`s by expiry, so replicas
/// sweeping the same rows at once delete each row once.
pub fn spawn_identity_sweeper(state: &AppState, jobs: &crate::jobs::Jobs) {
    let state = std::sync::Arc::downgrade(state);
    jobs.spawn_periodic(
        "auth-api-identity-sweep",
        crate::jobs::PeriodicCfg::every(IDENTITY_SWEEP_INTERVAL).after_first_interval(),
        move |_ctx| {
            let state = state.clone();
            async move {
                if let Some(state) = state.upgrade() {
                    let now = Utc::now();
                    let mut conn = state.pool.acquire().await?;
                    sweep_expired_identities(&mut conn, now).await?;
                    sweep_identity_tombstones(&mut conn, now).await?;
                    sweep_expired_requests(&mut conn, now).await?;
                }
                Ok(())
            }
        },
    );
}

/// Serializes request admission across replicas: the cap check and its
/// insert run in one transaction holding this lock.
const REQUEST_ADMISSION_LOCK: &str = "explore_api.auth_requests:admission";

/// The instant a handler works with, at the database's microsecond
/// precision, so the expiration a response carries is the one stored.
fn now_micros() -> DateTime<Utc> {
    Utc::now().trunc_subsecs(6)
}

fn naive(at: DateTime<Utc>) -> NaiveDateTime {
    at.naive_utc()
}

#[derive(Debug, Clone)]
pub struct ChallengeRecord {
    pub request_id: String,
    pub method: String,
    pub params: Vec<Value>,
    pub sender: Option<String>,
    pub code: u32,
    pub challenge: String,
    pub created_at: DateTime<Utc>,
    pub expiration: DateTime<Utc>,
    pub requires_validation: bool,
    pub status: ChallengeStatus,
    /// The creating network's [`request_source`] key, for the pending cap.
    pub source: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChallengeStatus {
    Pending,
    /// The outcome is stored (`auth_requests.outcome`) until it is delivered.
    Signed,
    Fulfilled,
}

impl ChallengeStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Signed => "signed",
            Self::Fulfilled => "fulfilled",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "signed" => Self::Signed,
            "fulfilled" => Self::Fulfilled,
            _ => Self::Pending,
        }
    }
}

#[derive(sqlx::FromRow)]
struct RequestRow {
    request_id: String,
    method: String,
    params: String,
    sender: Option<String>,
    code: i16,
    challenge: String,
    created_at: NaiveDateTime,
    expiration: NaiveDateTime,
    requires_validation: bool,
    status: String,
    source: Option<String>,
}

impl RequestRow {
    fn into_record(self) -> ChallengeRecord {
        ChallengeRecord {
            request_id: self.request_id,
            method: self.method,
            params: serde_json::from_str(&self.params).unwrap_or_default(),
            sender: self.sender,
            code: u32::try_from(self.code).unwrap_or_default(),
            challenge: self.challenge,
            created_at: self.created_at.and_utc(),
            expiration: self.expiration.and_utc(),
            requires_validation: self.requires_validation,
            status: ChallengeStatus::parse(&self.status),
            source: self.source,
        }
    }
}

macro_rules! request_columns {
    () => {
        "request_id, method, params::text AS params, sender, code, challenge, created_at, \
         expiration, requires_validation, status, source"
    };
}

/// Stores a new pending or fulfilled request (the create handler, inside its
/// admission transaction; tests seed rows with it). A signed request only
/// comes from [`submit_outcome`], which stores its outcome.
async fn insert_request(conn: &mut PgConnection, record: &ChallengeRecord) -> sqlx::Result<()> {
    let params = serde_json::to_string(&record.params).unwrap_or_else(|_| "[]".into());
    sqlx::query(
        "INSERT INTO explore_api.auth_requests (request_id, method, params, sender, code, \
         challenge, created_at, expiration, requires_validation, status, source) \
         VALUES ($1, $2, $3::json, $4, $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(&record.request_id)
    .bind(&record.method)
    .bind(params)
    .bind(&record.sender)
    .bind(i16::try_from(record.code).unwrap_or_default())
    .bind(&record.challenge)
    .bind(naive(record.created_at))
    .bind(naive(record.expiration))
    .bind(record.requires_validation)
    .bind(record.status.as_str())
    .bind(&record.source)
    .execute(conn)
    .await
    .map(drop)
}

/// A request as stored, or why there is none to work with: an expired
/// request is deleted on the read that finds it and answers 410 once.
enum Loaded {
    Missing,
    Expired,
    Found(ChallengeRecord),
}

async fn load_request(state: &AppState, id: &str, now: DateTime<Utc>) -> sqlx::Result<Loaded> {
    let row: Option<RequestRow> = sqlx::query_as(concat!(
        "SELECT ",
        request_columns!(),
        " FROM explore_api.auth_requests WHERE request_id = $1"
    ))
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;
    let Some(record) = row.map(RequestRow::into_record) else {
        return Ok(Loaded::Missing);
    };
    if record.expiration >= now {
        return Ok(Loaded::Found(record));
    }
    sqlx::query("DELETE FROM explore_api.auth_requests WHERE request_id = $1 AND expiration < $2")
        .bind(id)
        .bind(naive(now))
        .execute(&state.pool)
        .await?;
    Ok(Loaded::Expired)
}

#[derive(Debug, Deserialize)]
pub struct CreateRequestBody {
    pub method: String,
    #[serde(default)]
    pub params: Option<Vec<Value>>,
    #[serde(rename = "authChain", default)]
    pub auth_chain: Option<AuthChain>,
    #[serde(flatten)]
    pub unknown_fields: Map<String, Value>,
}

#[derive(Debug, Serialize)]
pub struct CreateRequestResponse {
    #[serde(rename = "requestId")]
    pub request_id: String,
    pub expiration: DateTime<Utc>,
    pub code: u32,
    pub challenge: String,
}

#[derive(Debug, Serialize)]
pub struct RecoverResponse {
    pub expiration: DateTime<Utc>,
    pub code: u32,
    pub method: String,
    pub params: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender: Option<String>,
    pub challenge: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OutcomeError {
    pub code: i32,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HttpOutcomeMessage {
    pub sender: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<OutcomeError>,
    #[serde(rename = "authChain", default, skip_serializing_if = "Option::is_none")]
    pub auth_chain: Option<AuthChain>,
}

#[derive(Debug, Serialize, Clone)]
pub struct OutcomeResponseMessage {
    #[serde(rename = "requestId")]
    pub request_id: String,
    pub sender: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<OutcomeError>,
    #[serde(rename = "authChain", skip_serializing_if = "Option::is_none")]
    pub auth_chain: Option<AuthChain>,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct EvictedBody {
    pub error: String,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ValidationStatusResponse {
    #[serde(rename = "requiresValidation")]
    pub requires_validation: bool,
}

#[derive(Debug, Serialize)]
pub struct LiveResponse {
    pub timestamp: i64,
}

/// The sign-in relay under the gateway's `/auth-api` prefix (the old
/// `rewrite ^/auth-api/?(.*)$ /auth/$1`): `/auth-api/requests`,
/// `/auth-api/v2/requests/{id}[/outcome,/validation]`, `/auth-api/identities`.
/// The old bearer-guarded `/admin/auth/*` routes are the console's
/// `explorer-api/{challenges-list,challenge-get,challenge-revoke,
/// identities-list,identity-revoke}` calls (`ops::admin::explore`).
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/health/live", get(health_live))
        .route("/health", get(health_live))
        .route("/requests", post(create_request))
        .route(
            "/requests/{id}",
            get(get_request_outcome).post(erase(post_request_outcome_legacy)),
        )
        .route("/v2/requests/{id}", get(get_request_v2))
        .route("/v2/requests/{id}/outcome", post(post_request_outcome))
        .route(
            "/v2/requests/{id}/validation",
            post(post_request_validation).get(erase(get_request_validation)),
        )
        .route("/identities", post(create_identity))
        .route("/identities/{id}", get(get_identity))
        .layer(middleware::from_fn(cors_preflight))
        .layer(middleware::from_fn(enforce_body_cap))
        .layer(middleware::from_fn(record_request_telemetry))
        .layer(DefaultBodyLimit::max(DEFAULT_BODY_SIZE_BYTES))
}

async fn cors_preflight(req: Request, next: Next) -> Response {
    if req.method() == Method::OPTIONS {
        return Response::builder()
            .status(StatusCode::NO_CONTENT)
            .header(header::ACCESS_CONTROL_ALLOW_METHODS, "GET, POST, OPTIONS")
            .header(
                header::ACCESS_CONTROL_ALLOW_HEADERS,
                "content-type, x-dcl-client, x-correlation-id, x-identity-verifier",
            )
            .header(header::ACCESS_CONTROL_EXPOSE_HEADERS, CORRELATION_HEADER)
            .header(header::ACCESS_CONTROL_MAX_AGE, "3600")
            .body(Body::empty())
            .expect("static preflight response");
    }
    next.run(req).await
}

async fn enforce_body_cap(req: Request, next: Next) -> Response {
    let declared = req
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty())
        .and_then(|v| v.parse::<usize>().ok());
    if declared.is_some_and(|len| len > DEFAULT_BODY_SIZE_BYTES) {
        return payload_too_large();
    }
    let res = next.run(req).await;
    if res.status() == StatusCode::PAYLOAD_TOO_LARGE {
        return payload_too_large();
    }
    res
}

fn challenge_view(record: &ChallengeRecord) -> Value {
    json!({
        "requestId": record.request_id,
        "method": record.method,
        "sender": record.sender,
        "code": record.code,
        "createdAt": record.created_at,
        "expiration": record.expiration,
        "requiresValidation": record.requires_validation,
        "status": record.status.as_str(),
    })
}

fn admin_db_error(err: sqlx::Error) -> (StatusCode, Value) {
    tracing::error!(%err, "auth-api console call");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        json!({ "error": "auth-api state unavailable" }),
    )
}

/// The console's `explorer-api/challenges-list`.
pub async fn admin_list_challenges(state: &AppState) -> (StatusCode, Value) {
    let rows: Vec<RequestRow> = match sqlx::query_as(concat!(
        "SELECT ",
        request_columns!(),
        " FROM explore_api.auth_requests ORDER BY created_at, request_id"
    ))
    .fetch_all(&state.pool)
    .await
    {
        Ok(rows) => rows,
        Err(err) => return admin_db_error(err),
    };
    let items: Vec<Value> = rows
        .into_iter()
        .map(|row| challenge_view(&row.into_record()))
        .collect();
    (
        StatusCode::OK,
        json!({ "count": items.len(), "challenges": items }),
    )
}

/// The console's `explorer-api/challenge-get`.
pub async fn admin_get_challenge(state: &AppState, id: &str) -> (StatusCode, Value) {
    let row: Option<RequestRow> = match sqlx::query_as(concat!(
        "SELECT ",
        request_columns!(),
        " FROM explore_api.auth_requests WHERE request_id = $1"
    ))
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    {
        Ok(row) => row,
        Err(err) => return admin_db_error(err),
    };
    match row {
        Some(row) => (StatusCode::OK, challenge_view(&row.into_record())),
        None => (
            StatusCode::NOT_FOUND,
            json!({ "error": "challenge_not_found", "requestId": id }),
        ),
    }
}

/// The console's `explorer-api/challenge-revoke`.
pub async fn admin_revoke_challenge(state: &AppState, id: &str) -> (StatusCode, Value) {
    let removed = match sqlx::query("DELETE FROM explore_api.auth_requests WHERE request_id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
    {
        Ok(done) => done.rows_affected() > 0,
        Err(err) => return admin_db_error(err),
    };
    let status = if removed {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    };
    (
        status,
        json!({ "ok": removed, "requestId": id, "revoked": removed }),
    )
}

/// The console's `explorer-api/identities-list`.
pub async fn admin_list_identities(state: &AppState) -> (StatusCode, Value) {
    let rows: Vec<(String, String, bool, NaiveDateTime, NaiveDateTime)> = match sqlx::query_as(
        "SELECT identity_id, ip_address, is_mobile, created_at, expiration \
         FROM explore_api.auth_identities ORDER BY created_at, identity_id",
    )
    .fetch_all(&state.pool)
    .await
    {
        Ok(rows) => rows,
        Err(err) => return admin_db_error(err),
    };
    let items: Vec<Value> = rows
        .into_iter()
        .map(
            |(identity_id, ip_address, is_mobile, created_at, expiration)| {
                json!({
                    "identityId": identity_id,
                    "ipAddress": ip_address,
                    "isMobile": is_mobile,
                    "createdAt": created_at.and_utc(),
                    "expiration": expiration.and_utc(),
                })
            },
        )
        .collect();
    (
        StatusCode::OK,
        json!({ "count": items.len(), "identities": items }),
    )
}

/// The console's `explorer-api/identity-revoke`: the identity goes, its
/// tombstone stays without a reason, so a pickup answers "evicted".
pub async fn admin_revoke_identity(state: &AppState, id: &str) -> (StatusCode, Value) {
    let removed =
        match sqlx::query("DELETE FROM explore_api.auth_identities WHERE identity_id = $1")
            .bind(id)
            .execute(&state.pool)
            .await
        {
            Ok(done) => done.rows_affected() > 0,
            Err(err) => return admin_db_error(err),
        };
    let status = if removed {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    };
    (
        status,
        json!({ "ok": removed, "identityId": id, "revoked": removed }),
    )
}

async fn health_live() -> impl IntoResponse {
    Json(LiveResponse {
        timestamp: Utc::now().timestamp_millis(),
    })
}

/// Drops the requests past their expiry and its 410 grace window.
pub async fn sweep_expired_requests(
    conn: &mut PgConnection,
    now: DateTime<Utc>,
) -> sqlx::Result<u64> {
    let horizon = now - Duration::seconds(REQUEST_EXPIRED_GRACE_SECONDS);
    sqlx::query("DELETE FROM explore_api.auth_requests WHERE expiration < $1")
        .bind(naive(horizon))
        .execute(conn)
        .await
        .map(|done| done.rows_affected())
}

/// The network a request comes from, for the pending caps, IPv6 by its /64:
/// a trusted proxy's `X-Real-IP` (the proxy itself when it sends none), or a
/// public peer. An untrusted private peer is most likely an unlisted gateway
/// whose address would put every client in one bucket, so it, like a request
/// without a socket, has no source and only the global cap applies. The
/// request chain is not bound to the request, so the caps never key on the
/// signer.
pub(super) fn request_source(
    headers: &HeaderMap,
    peer: Option<std::net::SocketAddr>,
    trusted_proxies: &[ipnet::IpNet],
) -> Option<String> {
    request_source_with(headers, peer, trusted_proxies, &UNTRUSTED_PEER_WARNED)
}

fn request_source_with(
    headers: &HeaderMap,
    peer: Option<std::net::SocketAddr>,
    trusted_proxies: &[ipnet::IpNet],
    warned: &std::sync::atomic::AtomicBool,
) -> Option<String> {
    let peer = peer?.ip().to_canonical();
    let ip = if is_trusted_proxy(peer, trusted_proxies) {
        single_real_ip(headers).unwrap_or(peer)
    } else if is_internal(peer) {
        warn_untrusted_internal_peer_once(peer, warned);
        return None;
    } else {
        peer
    };
    Some(source_key(&ip.to_string()))
}

/// Once per process, as content warns for deploys.
static UNTRUSTED_PEER_WARNED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Once per process, for any untrusted private or loopback peer: a proxy
/// missing from the setting, or clients that all arrive from private
/// addresses. `true` when this call logged.
fn warn_untrusted_internal_peer_once(
    peer: std::net::IpAddr,
    warned: &std::sync::atomic::AtomicBool,
) -> bool {
    use std::sync::atomic::Ordering;
    if warned.load(Ordering::Relaxed) || warned.swap(true, Ordering::Relaxed) {
        return false;
    }
    tracing::warn!(
        %peer,
        "/auth-api: a private or loopback peer is not in HTTP_TRUSTED_PROXY_CIDRS, so \
         sign-ins from it get no per-network cap; list the backend proxy networks there"
    );
    true
}

/// Takes the admission lock of `key` for the rest of the transaction: the
/// creates of every replica pass their cap check and insert one at a time.
pub(super) async fn admission_lock(conn: &mut PgConnection, key: &str) -> sqlx::Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(key)
        .execute(conn)
        .await
        .map(drop)
}

/// Rows held, unexpired requests whose outcome has not been delivered, and
/// those of them from `source`, in one pass over the table.
async fn request_counts(
    conn: &mut PgConnection,
    source: Option<&str>,
    now: DateTime<Utc>,
) -> sqlx::Result<(i64, i64, i64)> {
    sqlx::query_as(
        "SELECT count(*), \
                count(*) FILTER (WHERE expiration >= $1 AND status <> 'fulfilled'), \
                count(*) FILTER (WHERE expiration >= $1 AND status <> 'fulfilled' \
                                   AND source = $2) \
         FROM explore_api.auth_requests",
    )
    .bind(naive(now))
    .bind(source)
    .fetch_one(conn)
    .await
}

/// Refuses a new request once the relay or its network holds too many
/// unexpired requests whose outcome has not been delivered. Runs inside the
/// create's transaction, after [`admission_lock`].
async fn request_capacity_refusal(
    conn: &mut PgConnection,
    source: Option<&str>,
    now: DateTime<Utc>,
) -> sqlx::Result<Option<Response>> {
    let max = MAX_PENDING_REQUESTS as i64;
    let mut counts = request_counts(conn, source, now).await?;
    if counts.0 >= max {
        sweep_expired_requests(conn, now).await?;
        // Delivered records only answer 410; under pressure the oldest go,
        // just enough to admit one more.
        let (len, ..) = request_counts(conn, source, now).await?;
        if len >= max {
            sqlx::query(
                "DELETE FROM explore_api.auth_requests WHERE request_id IN ( \
                     SELECT request_id FROM explore_api.auth_requests \
                     WHERE status = 'fulfilled' ORDER BY created_at LIMIT $1)",
            )
            .bind(len + 1 - max)
            .execute(&mut *conn)
            .await?;
        }
        counts = request_counts(conn, source, now).await?;
    }
    // The caps bound requests in flight, not how many are created: a sender
    // can still free its own slot by posting and fetching an outcome.
    let (_, live, from_source) = counts;
    if live >= max {
        return Ok(Some(
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorBody {
                    error: "Too many pending requests, retry shortly".into(),
                }),
            )
                .into_response(),
        ));
    }
    Ok(
        (source.is_some() && from_source >= MAX_REQUESTS_PER_SOURCE as i64)
            .then(too_many_from_network),
    )
}

fn too_many_from_network() -> Response {
    (
        StatusCode::TOO_MANY_REQUESTS,
        Json(ErrorBody {
            error: "Too many pending requests from this network, try again later".into(),
        }),
    )
        .into_response()
}

async fn create_request(
    State(state): State<AppState>,
    peer: Option<Extension<ConnectInfo<std::net::SocketAddr>>>,
    headers: HeaderMap,
    Json(body): Json<CreateRequestBody>,
) -> Response {
    let validated = match validate_request_message(&body) {
        Ok(validated) => validated,
        Err(msg) => return bad_request(&msg),
    };
    let sender = match validate_auth_chain(validated.auth_chain) {
        Ok(verified) => verified.sender.to_lowercase(),
        Err(msg) => return bad_request(&msg),
    };
    let now = now_micros();
    let source = request_source(
        &headers,
        peer.map(|Extension(ConnectInfo(p))| p),
        &state.settings.trusted_proxy_cidrs,
    );

    let request_id = Uuid::new_v4().to_string();
    let expiration = now + Duration::seconds(REQUEST_TTL_SECONDS);
    let code = random_pairing_code();
    let challenge = build_challenge(&request_id, code, &now, &body.method, validated.params);

    let record = ChallengeRecord {
        request_id: request_id.clone(),
        method: body.method.clone(),
        params: validated.params.to_vec(),
        sender: Some(sender),
        code,
        challenge: challenge.clone(),
        created_at: now,
        expiration,
        requires_validation: false,
        status: ChallengeStatus::Pending,
        source,
    };
    match admit_request(&state, &record, now).await {
        Ok(None) => {}
        Ok(Some(refusal)) => return refusal,
        Err(err) => return internal_error("auth-api: storing the request", err),
    }

    (
        StatusCode::CREATED,
        Json(CreateRequestResponse {
            request_id,
            expiration,
            code,
            challenge,
        }),
    )
        .into_response()
}

/// The caps checked and the request stored in one transaction under the
/// admission lock, so concurrent creates on any replica cannot overshoot.
async fn admit_request(
    state: &AppState,
    record: &ChallengeRecord,
    now: DateTime<Utc>,
) -> sqlx::Result<Option<Response>> {
    // A network over its cap is refused before the admission lock: a flood
    // from one network does not queue on the lock every create takes, holding
    // pool connections. Under the lock the count is taken again, exactly.
    if let Some(source) = record.source.as_deref() {
        let from_source: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM explore_api.auth_requests \
             WHERE source = $1 AND expiration >= $2 AND status <> 'fulfilled'",
        )
        .bind(source)
        .bind(naive(now))
        .fetch_one(&state.pool)
        .await?;
        if from_source >= MAX_REQUESTS_PER_SOURCE as i64 {
            return Ok(Some(too_many_from_network()));
        }
    }
    let mut tx = state.pool.begin().await?;
    admission_lock(&mut tx, REQUEST_ADMISSION_LOCK).await?;
    if let Some(refusal) = request_capacity_refusal(&mut tx, record.source.as_deref(), now).await? {
        return Ok(Some(refusal));
    }
    insert_request(&mut tx, record).await?;
    tx.commit().await?;
    Ok(None)
}

fn expired(id: &str) -> Response {
    (
        StatusCode::GONE,
        Json(ErrorBody {
            error: format!("Request with id \"{}\" has expired", id),
        }),
    )
        .into_response()
}

/// The answer for a request that is not there to work with, or the record.
async fn found_request(
    state: &AppState,
    id: &str,
    now: DateTime<Utc>,
) -> Result<ChallengeRecord, Box<Response>> {
    match load_request(state, id, now).await {
        Ok(Loaded::Found(record)) => Ok(record),
        Ok(Loaded::Missing) => Err(Box::new(not_found(id))),
        Ok(Loaded::Expired) => Err(Box::new(expired(id))),
        Err(err) => Err(Box::new(internal_error(
            "auth-api: reading the request",
            err,
        ))),
    }
}

async fn get_request_v2(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let record = match found_request(&state, &id, now_micros()).await {
        Ok(record) => record,
        Err(answer) => return *answer,
    };
    if record.status == ChallengeStatus::Fulfilled {
        return gone_already_fulfilled(&id);
    }
    let body = RecoverResponse {
        expiration: record.expiration,
        code: record.code,
        method: record.method,
        params: record.params,
        sender: record.sender,
        challenge: record.challenge,
    };
    (StatusCode::OK, Json(body)).into_response()
}

async fn get_request_outcome(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let record = match found_request(&state, &id, now_micros()).await {
        Ok(record) => record,
        Err(answer) => return *answer,
    };
    match record.status {
        ChallengeStatus::Fulfilled => return gone_already_fulfilled(&id),
        ChallengeStatus::Pending => return StatusCode::NO_CONTENT.into_response(),
        ChallengeStatus::Signed => {}
    }
    // Delivered once: the poller whose update moves signed to fulfilled gets
    // the outcome; the record then frees its slot and keeps only the 410.
    // A concurrent poller waits on the row lock, finds it no longer signed
    // and returns nothing.
    let delivered: sqlx::Result<Option<String>> = sqlx::query_scalar(
        "UPDATE explore_api.auth_requests AS r \
         SET status = 'fulfilled', outcome = NULL, params = '[]', challenge = '' \
         FROM (SELECT request_id, outcome FROM explore_api.auth_requests \
               WHERE request_id = $1 AND status = 'signed' FOR UPDATE) AS signed \
         WHERE r.request_id = signed.request_id \
         RETURNING signed.outcome::text",
    )
    .bind(&id)
    .fetch_optional(&state.pool)
    .await;
    match delivered {
        Ok(Some(outcome)) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            outcome,
        )
            .into_response(),
        // Another poller took it, or it was revoked meanwhile.
        Ok(None) => match found_request(&state, &id, now_micros()).await {
            Ok(record) if record.status == ChallengeStatus::Pending => {
                StatusCode::NO_CONTENT.into_response()
            }
            Ok(_) => gone_already_fulfilled(&id),
            Err(answer) => *answer,
        },
        Err(err) => internal_error("auth-api: delivering the outcome", err),
    }
}

async fn post_request_outcome(
    state: State<AppState>,
    path: Path<String>,
    body: Json<HttpOutcomeMessage>,
) -> Response {
    submit_outcome(state, path, body).await
}

async fn post_request_outcome_legacy(
    state: State<AppState>,
    path: Path<String>,
    body: Json<HttpOutcomeMessage>,
) -> Response {
    submit_outcome(state, path, body).await
}

fn already_answered(id: &str, status: ChallengeStatus) -> Response {
    if status == ChallengeStatus::Fulfilled {
        return gone_already_fulfilled(id);
    }
    bad_request(&format!(
        "Request with id \"{}\" already has a response",
        id
    ))
}

async fn submit_outcome(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<HttpOutcomeMessage>,
) -> Response {
    if let Err(msg) = validate_outcome_body(&body) {
        return bad_request(&msg);
    }

    if let Some(chain) = body.auth_chain.as_ref()
        && let Err(msg) = validate_auth_chain(chain)
    {
        return bad_request(&msg);
    }

    let now = now_micros();
    let record = match found_request(&state, &id, now).await {
        Ok(record) => record,
        Err(answer) => return *answer,
    };
    if record.status != ChallengeStatus::Pending {
        return already_answered(&id, record.status);
    }

    let outcome = OutcomeResponseMessage {
        request_id: id.clone(),
        sender: body.sender.to_lowercase(),
        result: body.result,
        error: body.error,
        auth_chain: body.auth_chain,
    };
    let outcome = match serde_json::to_string(&outcome) {
        Ok(outcome) => outcome,
        Err(err) => return internal_error("auth-api: encoding the outcome", err),
    };
    // Only a pending request takes an outcome: of two wallets answering at
    // once, the second finds it signed.
    let stored = sqlx::query(
        "UPDATE explore_api.auth_requests SET status = 'signed', outcome = $2::json \
         WHERE request_id = $1 AND status = 'pending'",
    )
    .bind(&id)
    .bind(outcome)
    .execute(&state.pool)
    .await;
    match stored {
        Ok(done) if done.rows_affected() > 0 => StatusCode::OK.into_response(),
        Ok(_) => match found_request(&state, &id, now).await {
            Ok(record) => already_answered(&id, record.status),
            Err(answer) => *answer,
        },
        Err(err) => internal_error("auth-api: storing the outcome", err),
    }
}

async fn post_request_validation(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let record = match found_request(&state, &id, now_micros()).await {
        Ok(record) => record,
        Err(answer) => return *answer,
    };
    if record.status == ChallengeStatus::Fulfilled {
        return gone_already_fulfilled(&id);
    }
    let marked = sqlx::query(
        "UPDATE explore_api.auth_requests SET requires_validation = true \
         WHERE request_id = $1 AND status <> 'fulfilled'",
    )
    .bind(&id)
    .execute(&state.pool)
    .await;
    match marked {
        Ok(done) if done.rows_affected() > 0 => StatusCode::NO_CONTENT.into_response(),
        Ok(_) => match found_request(&state, &id, now_micros()).await {
            Ok(_) => gone_already_fulfilled(&id),
            Err(answer) => *answer,
        },
        Err(err) => internal_error("auth-api: marking the request", err),
    }
}

async fn get_request_validation(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let record = match found_request(&state, &id, now_micros()).await {
        Ok(record) => record,
        Err(answer) => return *answer,
    };
    if record.status == ChallengeStatus::Fulfilled {
        return gone_already_fulfilled(&id);
    }
    (
        StatusCode::OK,
        Json(ValidationStatusResponse {
            requires_validation: record.requires_validation,
        }),
    )
        .into_response()
}

fn random_pairing_code() -> u32 {
    loop {
        let bytes = *Uuid::new_v4().as_bytes();
        for (i, byte) in bytes.into_iter().enumerate() {
            if i == 6 || i == 8 {
                continue;
            }
            if byte < 200 {
                return u32::from(byte % 100);
            }
        }
    }
}

fn build_challenge(
    request_id: &str,
    code: u32,
    now: &DateTime<Utc>,
    method: &str,
    params: &[Value],
) -> String {
    let params_blob = serde_json::to_string(params).unwrap_or_else(|_| "[]".into());
    format!(
        "Decentraland Login\nRequest: {}\nCode: {}\nMethod: {}\nParams: {}\nIssued: {}",
        request_id,
        code,
        method,
        params_blob,
        now.to_rfc3339()
    )
}

fn not_found(id: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorBody {
            error: format!("Request with id \"{}\" not found", id),
        }),
    )
        .into_response()
}

fn gone_already_fulfilled(id: &str) -> Response {
    (
        StatusCode::GONE,
        Json(ErrorBody {
            error: format!("Request with id \"{}\" has already been fulfilled", id),
        }),
    )
        .into_response()
}

fn bad_request(msg: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorBody {
            error: msg.to_string(),
        }),
    )
        .into_response()
}

fn payload_too_large() -> Response {
    Response::builder()
        .status(StatusCode::PAYLOAD_TOO_LARGE)
        .header(header::CONNECTION, "close")
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Body::from("Payload Too Large"))
        .expect("static payload-too-large response")
}

fn validate_outcome_body(body: &HttpOutcomeMessage) -> Result<(), String> {
    if !crate::auth::is_eth_address(&body.sender) {
        return Err("sender must be a valid address".into());
    }
    match (body.result.is_some(), body.error.is_some()) {
        (true, false) | (false, true) => Ok(()),
        (false, false) => Err("Outcome must include either a result or an error".into()),
        (true, true) => Err("Outcome must include either a result or an error, not both".into()),
    }
}

fn is_valid_uuid(s: &str) -> bool {
    s.len() == 36
        && Uuid::parse_str(s).is_ok_and(|u| {
            u.get_version_num() == 4 && matches!(u.get_variant(), uuid::Variant::RFC4122)
        })
}

fn ips_match(stored: &str, request: &str) -> bool {
    if stored.is_empty() || request.is_empty() {
        return false;
    }
    let a = normalize_ip(stored);
    let b = normalize_ip(request);
    if a == b {
        return true;
    }
    let octets_a: Vec<&str> = a.split('.').collect();
    let octets_b: Vec<&str> = b.split('.').collect();
    if octets_a.len() == 4
        && octets_b.len() == 4
        && octets_a.iter().all(|o| o.parse::<u8>().is_ok())
        && octets_b.iter().all(|o| o.parse::<u8>().is_ok())
    {
        return octets_a[..3] == octets_b[..3];
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderName;

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(
                HeaderName::from_bytes(name.as_bytes()).unwrap(),
                value.parse().unwrap(),
            );
        }
        map
    }

    #[test]
    fn client_tag_accepts_versioned_known_clients_and_marks_everything_else_unknown() {
        assert_eq!(
            client_tag(&headers(&[(CLIENT_HEADER, "dcl-web/1")])),
            "dcl-web/1"
        );
        assert_eq!(
            client_tag(&headers(&[(CLIENT_HEADER, "dcl-native/0.5.0")])),
            "dcl-native/0.5.0"
        );
        assert_eq!(client_tag(&headers(&[])), "unknown");
        assert_eq!(
            client_tag(&headers(&[(CLIENT_HEADER, "spaces are invalid")])),
            "unknown"
        );
    }

    #[tokio::test]
    async fn request_telemetry_returns_the_supplied_or_generated_correlation_id() {
        use tower::ServiceExt;

        let app = Router::new()
            .route("/", get(|| async { StatusCode::NO_CONTENT }))
            .layer(middleware::from_fn(record_request_telemetry));
        let supplied = "4ef97fa3-9dcf-4630-9bc4-5f742706e87a";
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header(CLIENT_HEADER, "dcl-web/1")
                    .header(CORRELATION_HEADER, supplied)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.headers()[CORRELATION_HEADER], supplied);
        assert_eq!(
            response.headers()[header::ACCESS_CONTROL_EXPOSE_HEADERS],
            CORRELATION_HEADER
        );

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let generated = response.headers()[CORRELATION_HEADER].to_str().unwrap();
        assert!(Uuid::parse_str(generated).is_ok());
    }

    #[test]
    fn pairing_code_within_range() {
        for _ in 0..1000 {
            assert!(random_pairing_code() < 100);
        }
    }

    #[test]
    fn pairing_code_independent_of_challenge_timestamp() {
        let now = Utc::now();
        let stamp_code = now.timestamp_subsec_nanos() % 100;
        let codes: std::collections::HashSet<u32> =
            (0..128).map(|_| random_pairing_code()).collect();
        assert!(codes.len() > 1);
        assert!(codes.iter().any(|c| *c != stamp_code));
        let challenge = build_challenge("req-1", 42, &now, "personal_sign", &[]);
        assert!(challenge.contains("Code: 42"));
    }

    #[test]
    fn request_ttl_matches_the_upstream_default() {
        assert_eq!(REQUEST_TTL_SECONDS, 300);
    }

    fn outcome(
        sender: &str,
        result: Option<Value>,
        error: Option<OutcomeError>,
    ) -> HttpOutcomeMessage {
        HttpOutcomeMessage {
            sender: sender.into(),
            result,
            error,
            auth_chain: None,
        }
    }

    #[test]
    fn outcome_sender_must_be_an_eth_address() {
        let sig = Some(Value::from("0xdeadbeef"));
        assert!(
            validate_outcome_body(&outcome(
                "0x1234567890123456789012345678901234567890",
                sig.clone(),
                None
            ))
            .is_ok()
        );
        for bad in [
            "",
            "not-an-address",
            "0x123",
            "1234567890123456789012345678901234567890",
            "0x123456789012345678901234567890123456789z",
        ] {
            assert_eq!(
                validate_outcome_body(&outcome(bad, sig.clone(), None)).unwrap_err(),
                "sender must be a valid address"
            );
        }
    }

    #[test]
    fn uuid_accepts_hyphenated_v4_in_either_case() {
        assert!(is_valid_uuid("8c545b52-c08e-4c39-a854-46e2b5b03c48"));
        assert!(is_valid_uuid("8C545B52-C08E-4C39-A854-46E2B5B03C48"));
    }

    #[test]
    fn uuid_rejects_non_v4_versions_and_nil() {
        assert!(!is_valid_uuid("c232ab00-9414-11ec-b909-0242ac120002"));
        assert!(!is_valid_uuid("00000000-0000-0000-0000-000000000000"));
    }

    #[test]
    fn uuid_rejects_non_hyphenated_encodings() {
        assert!(!is_valid_uuid("8c545b52c08e4c39a85446e2b5b03c48"));
        assert!(!is_valid_uuid("{8c545b52-c08e-4c39-a854-46e2b5b03c48}"));
        assert!(!is_valid_uuid(
            "urn:uuid:8c545b52-c08e-4c39-a854-46e2b5b03c48"
        ));
    }

    #[test]
    fn uuid_rejects_non_rfc4122_variant_nibbles() {
        assert!(!is_valid_uuid("8c545b52-c08e-4c39-c854-46e2b5b03c48"));
        assert!(!is_valid_uuid("8c545b52-c08e-4c39-7854-46e2b5b03c48"));
    }

    #[test]
    fn outcome_requires_exactly_one_of_result_or_error() {
        let addr = "0x1234567890123456789012345678901234567890";
        let err = OutcomeError {
            code: 4001,
            message: "User rejected the request".into(),
            data: None,
        };
        assert!(validate_outcome_body(&outcome(addr, Some(Value::from("0xsig")), None)).is_ok());
        assert!(validate_outcome_body(&outcome(addr, None, Some(err.clone()))).is_ok());
        assert_eq!(
            validate_outcome_body(&outcome(addr, None, None)).unwrap_err(),
            "Outcome must include either a result or an error"
        );
        assert_eq!(
            validate_outcome_body(&outcome(addr, Some(Value::from("0xsig")), Some(err)))
                .unwrap_err(),
            "Outcome must include either a result or an error, not both"
        );
    }

    async fn router() -> Router {
        routes().with_state(crate::api::tests_support::state_without_db().await)
    }

    use crate::api::tests_support::{ScratchDb, scratch_database, state_on_pool};

    /// A scratch database and a replica's state over it; `None` (the test
    /// skips) without `CATALYST_TEST_DATABASE_URL`.
    async fn replica() -> Option<(ScratchDb, AppState)> {
        let db = scratch_database().await?;
        let state = state_on_pool(db.pool.clone()).await;
        Some((db, state))
    }

    /// A second replica: its own state and pool on the same database.
    async fn other_replica(db: &ScratchDb) -> AppState {
        state_on_pool(db.second_pool().await).await
    }

    fn app(state: &AppState) -> Router {
        Router::new().nest("/auth-api", routes().with_state(state.clone()))
    }

    fn json_request(method: &str, uri: &str, body: Vec<u8>) -> Request {
        Request::builder()
            .method(method)
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::CONTENT_LENGTH, body.len())
            .header("x-real-ip", "203.0.113.9")
            .body(Body::from(body))
            .unwrap()
    }

    fn json_post(uri: &str, body: Vec<u8>) -> Request {
        json_request("POST", uri, body)
    }

    async fn assert_upstream_payload_too_large(res: Response, uri: &str) {
        assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE, "{uri}");
        assert_eq!(
            res.headers()
                .get(header::CONNECTION)
                .and_then(|v| v.to_str().ok()),
            Some("close"),
            "{uri}"
        );
        let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024)
            .await
            .unwrap();
        assert_eq!(bytes.as_ref(), b"Payload Too Large", "{uri}");
    }

    fn padded_request_body(padding: usize) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "method": "eth_sendTransaction",
            "params": [format!("0x{}", "ab".repeat(padding))],
        }))
        .unwrap()
    }

    fn delegated(ephemeral: &str) -> AuthChain {
        use alloy_signer::SignerSync;
        let signer = crate::shared::signed_fetch::test_support::wallet();
        let message = format!(
            "Decentraland Login\nEphemeral address: {ephemeral}\nExpiration: 2100-01-01T00:00:00.000Z"
        );
        let signature = format!(
            "0x{}",
            hex::encode(
                signer
                    .sign_message_sync(message.as_bytes())
                    .unwrap()
                    .as_bytes()
            )
        );
        vec![
            crate::auth::AuthLink {
                link_type: crate::auth::AuthLinkType::SIGNER,
                payload: crate::shared::signed_fetch::test_support::address(),
                signature: None,
            },
            crate::auth::AuthLink {
                link_type: crate::auth::AuthLinkType::EcdsaEphemeral,
                payload: message,
                signature: Some(signature),
            },
        ]
    }

    async fn call(app: &Router, req: Request) -> (StatusCode, Value) {
        use tower::ServiceExt;
        let res = app.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    fn get_req(uri: &str) -> Request {
        Request::builder()
            .uri(uri)
            .header("x-real-ip", "203.0.113.9")
            .body(Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn a_sign_in_request_is_recovered_answered_and_fulfilled_once() {
        let Some((db, state)) = replica().await else {
            return;
        };
        let app = app(&state);
        let ephemeral = alloy_signer_local::PrivateKeySigner::random();
        let chain = delegated(&format!("{:#x}", ephemeral.address()));
        let body = serde_json::to_vec(
            &json!({ "method": "personal_sign", "params": ["hello"], "authChain": chain }),
        )
        .unwrap();
        let (status, created) = call(&app, json_post("/auth-api/requests", body)).await;
        assert_eq!(status, StatusCode::CREATED, "{created}");
        let id = created["requestId"].as_str().unwrap().to_string();
        assert!(
            created["challenge"]
                .as_str()
                .unwrap()
                .contains(&format!("Request: {id}"))
        );

        let (status, recovered) = call(&app, get_req(&format!("/auth-api/v2/requests/{id}"))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(recovered["method"], "personal_sign");
        assert_eq!(
            recovered["sender"],
            crate::shared::signed_fetch::test_support::address()
        );
        let (status, _) = call(&app, get_req(&format!("/auth-api/requests/{id}"))).await;
        assert_eq!(status, StatusCode::NO_CONTENT, "pending");

        let (status, _) = call(
            &app,
            json_post(
                &format!("/auth-api/v2/requests/{id}/validation"),
                Vec::new(),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (_, v) = call(
            &app,
            get_req(&format!("/auth-api/v2/requests/{id}/validation")),
        )
        .await;
        assert_eq!(v["requiresValidation"], true);

        let outcome = serde_json::to_vec(
            &json!({ "sender": "0x1234567890123456789012345678901234567890", "result": "0xsig" }),
        )
        .unwrap();
        let (status, _) = call(
            &app,
            json_post(
                &format!("/auth-api/v2/requests/{id}/outcome"),
                outcome.clone(),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, again) = call(
            &app,
            json_post(&format!("/auth-api/v2/requests/{id}/outcome"), outcome),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{again}");
        let (status, answered) = call(&app, get_req(&format!("/auth-api/requests/{id}"))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(answered["result"], "0xsig");
        assert_eq!(answered["requestId"], id);
        let (status, gone) = call(&app, get_req(&format!("/auth-api/requests/{id}"))).await;
        assert_eq!(status, StatusCode::GONE);
        assert_eq!(
            gone["error"],
            format!("Request with id \"{id}\" has already been fulfilled")
        );
        let (status, _) = call(
            &app,
            get_req("/auth-api/requests/8c545b52-c08e-4c39-a854-46e2b5b03c48"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        db.teardown().await;
    }

    #[tokio::test]
    async fn an_identity_is_parked_by_its_owner_and_picked_up_once_from_the_same_network() {
        let Some((db, state)) = replica().await else {
            return;
        };
        let app = app(&state);
        let ephemeral = alloy_signer_local::PrivateKeySigner::random();
        let eph_address = format!("{:#x}", ephemeral.address());
        let identity = json!({
            "ephemeralIdentity": { "address": eph_address, "privateKey": format!("0x{}", hex::encode(ephemeral.to_bytes())), "publicKey": "0x04" },
            "expiration": "2100-01-01T00:00:00.000Z",
            "authChain": delegated(&eph_address),
        });
        let body = serde_json::to_vec(&json!({ "identity": identity })).unwrap();
        let unsigned = call(&app, json_post("/auth-api/identities", body.clone())).await;
        assert_eq!(unsigned.0, StatusCode::UNAUTHORIZED, "{}", unsigned.1);

        let headers = crate::shared::signed_fetch::test_support::signed_headers(
            "post",
            "/auth-api/identities",
            "{}",
            true,
        )
        .await;
        let mut req = json_post("/auth-api/identities", body);
        req.headers_mut().extend(headers);
        let (status, created) = call(&app, req).await;
        assert_eq!(status, StatusCode::CREATED, "{created}");
        let id = created["identityId"].as_str().unwrap().to_string();
        assert_eq!(admin_list_identities(&state).await.1["count"], 1);

        let other_network = Request::builder()
            .uri(format!("/auth-api/identities/{id}"))
            .header("x-real-ip", "198.51.100.7")
            .body(Body::empty())
            .unwrap();
        let (status, err) = call(&app, other_network).await;
        assert_eq!(
            (status, err["error"].as_str()),
            (StatusCode::FORBIDDEN, Some("IP address mismatch"))
        );
        let (status, err) = call(&app, get_req(&format!("/auth-api/identities/{id}"))).await;
        assert_eq!(
            (status, err["error"].as_str()),
            (
                StatusCode::NOT_FOUND,
                Some("Identity was deleted due to IP mismatch")
            )
        );
        db.teardown().await;
    }

    fn from_peer(mut req: Request, peer: &str) -> Request {
        req.extensions_mut()
            .insert(ConnectInfo(peer.parse::<std::net::SocketAddr>().unwrap()));
        req
    }

    fn record(id: &str, source: &str, expiration: DateTime<Utc>) -> ChallengeRecord {
        ChallengeRecord {
            request_id: id.into(),
            method: "personal_sign".into(),
            params: Vec::new(),
            sender: Some("0x1234567890123456789012345678901234567890".into()),
            code: 1,
            challenge: String::new(),
            created_at: Utc::now(),
            expiration,
            requires_validation: false,
            status: ChallengeStatus::Pending,
            source: Some(source.into()),
        }
    }

    #[tokio::test]
    async fn a_network_is_capped_on_pending_requests_and_a_replayed_chain_locks_no_one_out() {
        let Some((db, state)) = replica().await else {
            return;
        };
        let app = app(&state);
        let ephemeral = alloy_signer_local::PrivateKeySigner::random();
        let body = serde_json::to_vec(&json!({ "method": "personal_sign", "params": ["hi"], "authChain": delegated(&format!("{:#x}", ephemeral.address())) })).unwrap();
        let post = |peer: &str| from_peer(json_post("/auth-api/requests", body.clone()), peer);
        for _ in 0..MAX_REQUESTS_PER_SOURCE {
            let (status, created) = call(&app, post("203.0.113.5:4000")).await;
            assert_eq!(status, StatusCode::CREATED, "{created}");
        }
        let (status, _) = call(&app, post("203.0.113.5:4001")).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        let (status, _) = call(&app, post("198.51.100.7:4000")).await;
        assert_eq!(
            status,
            StatusCode::CREATED,
            "the same chain from another network is not refused"
        );
        let (status, _) = call(&app, json_post("/auth-api/requests", body.clone())).await;
        assert_eq!(
            status,
            StatusCode::CREATED,
            "a request without a source has no per-network cap"
        );
        for _ in 0..MAX_REQUESTS_PER_SOURCE {
            let (status, _) = call(&app, post("10.0.0.9:4000")).await;
            assert_eq!(
                status,
                StatusCode::CREATED,
                "an unlisted private gateway does not put every client in one bucket"
            );
        }

        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT request_id FROM explore_api.auth_requests WHERE source = '203.0.113.5'",
        )
        .fetch_all(&state.pool)
        .await
        .unwrap();
        let outcome = serde_json::to_vec(
            &json!({ "sender": "0x1234567890123456789012345678901234567890", "error": { "code": 1, "message": "no" } }),
        )
        .unwrap();
        for id in &ids {
            let (status, _) = call(
                &app,
                json_post(
                    &format!("/auth-api/v2/requests/{id}/outcome"),
                    outcome.clone(),
                ),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
        }
        let (status, _) = call(&app, post("203.0.113.5:4002")).await;
        assert_eq!(
            status,
            StatusCode::TOO_MANY_REQUESTS,
            "an answer posted to the request does not free its slot"
        );
        for id in &ids {
            let (status, _) = call(&app, get_req(&format!("/auth-api/requests/{id}"))).await;
            assert_eq!(status, StatusCode::OK, "the requester picks up its outcome");
        }
        let (status, _) = call(&app, post("203.0.113.5:4003")).await;
        assert_eq!(
            status,
            StatusCode::CREATED,
            "a delivered outcome frees the slot"
        );
        let (status, _) = call(&app, get_req(&format!("/auth-api/requests/{}", ids[0]))).await;
        assert_eq!(
            status,
            StatusCode::GONE,
            "a delivered request still answers 410"
        );
        db.teardown().await;
    }

    #[test]
    fn an_untrusted_private_peer_without_x_real_ip_warns() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let peer = |s: &str| Some(s.parse::<std::net::SocketAddr>().unwrap());
        let warned = AtomicBool::new(false);
        let public = request_source_with(&HeaderMap::new(), peer("203.0.113.5:1"), &[], &warned);
        assert_eq!(public.as_deref(), Some("203.0.113.5"));
        assert!(
            !warned.load(Ordering::Relaxed),
            "a public peer is not warned about"
        );
        let private = request_source_with(&HeaderMap::new(), peer("172.17.0.1:1"), &[], &warned);
        assert_eq!(private, None);
        assert!(warned.load(Ordering::Relaxed));
    }

    #[test]
    fn the_untrusted_peer_warning_is_logged_once() {
        let warned = std::sync::atomic::AtomicBool::new(false);
        let peer: std::net::IpAddr = "10.0.0.5".parse().unwrap();
        assert!(warn_untrusted_internal_peer_once(peer, &warned));
        assert!(!warn_untrusted_internal_peer_once(peer, &warned));
    }

    #[test]
    fn request_sources_group_ipv6_by_64_and_trust_only_listed_proxies() {
        let none = HeaderMap::new();
        let peer = |s: &str| Some(s.parse::<std::net::SocketAddr>().unwrap());
        assert_eq!(
            request_source(&none, peer("[2001:db8:1:2::9]:1"), &[]),
            request_source(&none, peer("[2001:db8:1:2:abcd::1]:1"), &[])
        );
        assert_ne!(
            request_source(&none, peer("[2001:db8:1:2::9]:1"), &[]),
            request_source(&none, peer("[2001:db8:1:3::9]:1"), &[])
        );
        let spoofed = headers(&[("x-real-ip", "198.51.100.7")]);
        assert_eq!(
            request_source(&spoofed, peer("203.0.113.5:1"), &[]).as_deref(),
            Some("203.0.113.5"),
            "a public untrusted peer's header is ignored"
        );
        let proxies = ["10.0.0.0/8".parse().unwrap()];
        assert_eq!(
            request_source(&spoofed, peer("10.0.0.5:1"), &proxies).as_deref(),
            Some("198.51.100.7")
        );
        assert_eq!(
            request_source(&none, peer("10.0.0.5:1"), &proxies).as_deref(),
            Some("10.0.0.5"),
            "a trusted proxy without X-Real-IP is its own source"
        );
        assert_eq!(
            request_source(&spoofed, peer("10.0.0.5:1"), &[]),
            None,
            "an unlisted private gateway gives no source"
        );
        assert_eq!(request_source(&spoofed, None, &proxies), None);
    }

    /// `n` pending requests `r0..`, each from its own network, expiring at
    /// `expiration`, created a second apart (`r0` oldest), in one statement.
    async fn seed_requests(
        state: &AppState,
        n: usize,
        status: ChallengeStatus,
        expiration: DateTime<Utc>,
        now: DateTime<Utc>,
    ) {
        sqlx::query(
            "INSERT INTO explore_api.auth_requests (request_id, method, params, sender, code, \
             challenge, created_at, expiration, status, source) \
             SELECT 'r' || i, 'personal_sign', '[]', NULL, 1, '', \
                    $3 - make_interval(secs => $1 - i), $2, $4, 's' || i \
             FROM generate_series(0, $1 - 1) AS i",
        )
        .bind(n as i32)
        .bind(naive(expiration))
        .bind(naive(now))
        .bind(status.as_str())
        .execute(&state.pool)
        .await
        .unwrap();
    }

    async fn refusal(
        state: &AppState,
        source: Option<&str>,
        now: DateTime<Utc>,
    ) -> Option<StatusCode> {
        let mut tx = state.pool.begin().await.unwrap();
        admission_lock(&mut tx, REQUEST_ADMISSION_LOCK)
            .await
            .unwrap();
        let refused = request_capacity_refusal(&mut tx, source, now)
            .await
            .unwrap()
            .map(|r| r.status());
        tx.commit().await.unwrap();
        refused
    }

    async fn request_ids(state: &AppState) -> Vec<String> {
        sqlx::query_scalar("SELECT request_id FROM explore_api.auth_requests ORDER BY request_id")
            .fetch_all(&state.pool)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn the_global_cap_counts_only_unexpired_requests() {
        let Some((db, state)) = replica().await else {
            return;
        };
        let now = now_micros();
        let in_grace = now - Duration::seconds(1);
        seed_requests(
            &state,
            MAX_PENDING_REQUESTS,
            ChallengeStatus::Pending,
            in_grace,
            now,
        )
        .await;
        assert_eq!(
            refusal(&state, Some("new"), now).await,
            None,
            "expired requests in their grace window hold no slot"
        );
        let live = now + Duration::seconds(REQUEST_TTL_SECONDS);
        sqlx::query("UPDATE explore_api.auth_requests SET expiration = $1")
            .bind(naive(live))
            .execute(&state.pool)
            .await
            .unwrap();
        assert_eq!(
            refusal(&state, None, now).await,
            Some(StatusCode::SERVICE_UNAVAILABLE)
        );
        db.teardown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn concurrent_creates_on_two_replicas_never_overshoot_a_network_cap() {
        let Some((db, state)) = replica().await else {
            return;
        };
        let replicas = [app(&state), app(&other_replica(&db).await)];
        let now = now_micros();
        let mut conn = state.pool.acquire().await.unwrap();
        for i in 0..MAX_REQUESTS_PER_SOURCE - 3 {
            let id = format!("r{i}");
            insert_request(
                &mut conn,
                &record(
                    &id,
                    "203.0.113.5",
                    now + Duration::seconds(REQUEST_TTL_SECONDS),
                ),
            )
            .await
            .unwrap();
        }
        drop(conn);
        let ephemeral = alloy_signer_local::PrivateKeySigner::random();
        let body = serde_json::to_vec(&json!({ "method": "personal_sign", "params": ["hi"], "authChain": delegated(&format!("{:#x}", ephemeral.address())) })).unwrap();
        let calls: Vec<_> = (0..32)
            .map(|i| {
                let app = replicas[i % 2].clone();
                let req = from_peer(
                    json_post("/auth-api/requests", body.clone()),
                    &format!("203.0.113.5:{}", 5000 + i),
                );
                tokio::spawn(async move { call(&app, req).await.0 })
            })
            .collect();
        let mut created = 0;
        for call in calls {
            let status = call.await.unwrap();
            if status == StatusCode::CREATED {
                created += 1;
            } else {
                assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
            }
        }
        assert_eq!(created, 3, "only the free slots are taken");
        let held: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM explore_api.auth_requests WHERE source = '203.0.113.5'",
        )
        .fetch_one(&state.pool)
        .await
        .unwrap();
        assert_eq!(held, MAX_REQUESTS_PER_SOURCE as i64);
        db.teardown().await;
    }

    #[tokio::test]
    async fn delivered_requests_go_first_when_the_relay_is_full() {
        let Some((db, state)) = replica().await else {
            return;
        };
        let now = now_micros();
        let live = now + Duration::seconds(REQUEST_TTL_SECONDS);
        seed_requests(
            &state,
            MAX_PENDING_REQUESTS,
            ChallengeStatus::Fulfilled,
            live,
            now,
        )
        .await;
        assert_eq!(refusal(&state, Some("new"), now).await, None);
        let ids = request_ids(&state).await;
        assert_eq!(
            ids.len(),
            MAX_PENDING_REQUESTS - 1,
            "only enough delivered records go to admit one more"
        );
        assert!(
            !ids.iter().any(|id| id == "r0"),
            "the oldest delivered record goes first"
        );
        assert!(ids.iter().any(|id| id == "r1"));
        db.teardown().await;
    }

    #[tokio::test]
    async fn the_sweep_keeps_requests_through_their_grace_window() {
        let Some((db, state)) = replica().await else {
            return;
        };
        let now = now_micros();
        let mut conn = state.pool.acquire().await.unwrap();
        for (id, expiration) in [
            (
                "old",
                now - Duration::seconds(REQUEST_EXPIRED_GRACE_SECONDS + 1),
            ),
            ("grace", now - Duration::seconds(1)),
            ("live", now + Duration::seconds(60)),
        ] {
            insert_request(&mut conn, &record(id, "", expiration))
                .await
                .unwrap();
        }
        assert_eq!(sweep_expired_requests(&mut conn, now).await.unwrap(), 1);
        assert_eq!(
            sweep_expired_requests(&mut conn, now).await.unwrap(),
            0,
            "a second sweep, as another replica runs it, finds nothing"
        );
        drop(conn);
        assert_eq!(request_ids(&state).await, ["grace", "live"]);
        db.teardown().await;
    }

    #[tokio::test]
    async fn a_network_is_capped_on_pending_identities_but_a_signer_is_not() {
        let Some((db, state)) = replica().await else {
            return;
        };
        let app = app(&state);
        let post = || async {
            let ephemeral = alloy_signer_local::PrivateKeySigner::random();
            let eph_address = format!("{:#x}", ephemeral.address());
            let identity = json!({
                "ephemeralIdentity": { "address": eph_address, "privateKey": format!("0x{}", hex::encode(ephemeral.to_bytes())), "publicKey": "0x04" },
                "expiration": "2100-01-01T00:00:00.000Z",
                "authChain": delegated(&eph_address),
            });
            let body = serde_json::to_vec(&json!({ "identity": identity })).unwrap();
            let headers = crate::shared::signed_fetch::test_support::signed_headers(
                "post",
                "/auth-api/identities",
                "{}",
                true,
            )
            .await;
            let mut req = json_post("/auth-api/identities", body);
            req.headers_mut().extend(headers);
            req
        };
        for _ in 0..8 {
            let (status, body) = call(&app, from_peer(post().await, "198.51.100.7:1")).await;
            assert_eq!(
                status,
                StatusCode::CREATED,
                "one signer is not capped: {body}"
            );
        }
        let now = now_micros();
        let mut conn = state.pool.acquire().await.unwrap();
        for i in 0..identities::MAX_IDENTITIES_PER_SOURCE {
            identities::insert_identity(
                &mut conn,
                &identities::IdentityRecord {
                    identity_id: format!("i{i}"),
                    identity: Value::Null,
                    ip_address: String::new(),
                    source: Some("203.0.113.5".into()),
                    is_mobile: false,
                    created_at: now,
                    expiration: now + Duration::seconds(3600),
                    auth_request_id: None,
                },
            )
            .await
            .unwrap();
        }
        drop(conn);
        let (status, _) = call(&app, from_peer(post().await, "203.0.113.5:1")).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        let (status, _) = call(&app, from_peer(post().await, "198.51.100.7:2")).await;
        assert_eq!(status, StatusCode::CREATED);
        db.teardown().await;
    }

    #[tokio::test]
    async fn the_console_calls_list_and_revoke_pending_requests() {
        let Some((db, state)) = replica().await else {
            return;
        };
        let app = app(&state);
        let ephemeral = alloy_signer_local::PrivateKeySigner::random();
        let body = serde_json::to_vec(&json!({ "method": "eth_signTypedData_v4", "params": [], "authChain": delegated(&format!("{:#x}", ephemeral.address())) })).unwrap();
        let (_, created) = call(&app, json_post("/auth-api/requests", body)).await;
        let id = created["requestId"].as_str().unwrap().to_string();
        assert_eq!(admin_list_challenges(&state).await.1["count"], 1);
        assert_eq!(
            admin_get_challenge(&state, &id).await.1["status"],
            "pending"
        );
        assert_eq!(admin_revoke_challenge(&state, &id).await.0, StatusCode::OK);
        assert_eq!(
            admin_revoke_challenge(&state, &id).await.0,
            StatusCode::NOT_FOUND
        );
        let (status, _) = call(&app, get_req(&format!("/auth-api/v2/requests/{id}"))).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        db.teardown().await;
    }

    fn request_body() -> Vec<u8> {
        let ephemeral = alloy_signer_local::PrivateKeySigner::random();
        let chain = delegated(&format!("{:#x}", ephemeral.address()));
        serde_json::to_vec(
            &json!({ "method": "personal_sign", "params": ["hello"], "authChain": chain }),
        )
        .unwrap()
    }

    async fn identity_post() -> Request {
        identity_post_with(json!({})).await
    }

    /// An identity create whose body also carries `extra`'s fields.
    async fn identity_post_with(extra: Value) -> Request {
        let ephemeral = alloy_signer_local::PrivateKeySigner::random();
        let eph_address = format!("{:#x}", ephemeral.address());
        let identity = json!({
            "ephemeralIdentity": { "address": eph_address, "privateKey": format!("0x{}", hex::encode(ephemeral.to_bytes())), "publicKey": "0x04" },
            "expiration": "2100-01-01T00:00:00.000Z",
            "authChain": delegated(&eph_address),
        });
        let mut body = json!({ "identity": identity });
        body.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        let body = serde_json::to_vec(&body).unwrap();
        let headers = crate::shared::signed_fetch::test_support::signed_headers(
            "post",
            "/auth-api/identities",
            "{}",
            true,
        )
        .await;
        let mut req = json_post("/auth-api/identities", body);
        req.headers_mut().extend(headers);
        req
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_request_created_on_one_replica_is_recovered_answered_and_polled_on_another() {
        let Some((db, state)) = replica().await else {
            return;
        };
        let (a, b) = (app(&state), app(&other_replica(&db).await));
        let (status, created) = call(&a, json_post("/auth-api/requests", request_body())).await;
        assert_eq!(status, StatusCode::CREATED, "{created}");
        let id = created["requestId"].as_str().unwrap().to_string();

        let (status, recovered) = call(&b, get_req(&format!("/auth-api/v2/requests/{id}"))).await;
        assert_eq!(status, StatusCode::OK, "{recovered}");
        assert_eq!(recovered["code"], created["code"]);
        assert_eq!(recovered["challenge"], created["challenge"]);
        assert_eq!(recovered["expiration"], created["expiration"]);
        assert_eq!(recovered["params"], json!(["hello"]));
        let (status, _) = call(&a, get_req(&format!("/auth-api/requests/{id}"))).await;
        assert_eq!(
            status,
            StatusCode::NO_CONTENT,
            "pending on the other replica"
        );

        let validation = format!("/auth-api/v2/requests/{id}/validation");
        let (status, _) = call(&b, json_post(&validation, Vec::new())).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (_, v) = call(&a, get_req(&validation)).await;
        assert_eq!(v["requiresValidation"], true);

        let outcome = serde_json::to_vec(
            &json!({ "sender": "0x1234567890123456789012345678901234567890", "result": "0xsig" }),
        )
        .unwrap();
        let path = format!("/auth-api/v2/requests/{id}/outcome");
        let (status, _) = call(&b, json_post(&path, outcome.clone())).await;
        assert_eq!(status, StatusCode::OK);
        let (status, again) = call(&a, json_post(&path, outcome)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{again}");
        assert_eq!(
            again["error"],
            format!("Request with id \"{id}\" already has a response")
        );

        let polls: Vec<_> = [a.clone(), b.clone()]
            .into_iter()
            .map(|app| {
                let req = get_req(&format!("/auth-api/requests/{id}"));
                tokio::spawn(async move { call(&app, req).await })
            })
            .collect();
        let mut answers = Vec::new();
        for poll in polls {
            answers.push(poll.await.unwrap());
        }
        answers.sort_by_key(|(status, _)| status.as_u16());
        assert_eq!(answers[0].0, StatusCode::OK, "one poller gets the outcome");
        assert_eq!(answers[0].1["result"], "0xsig");
        assert_eq!(answers[0].1["requestId"], id);
        assert_eq!(
            answers[0].1["sender"],
            "0x1234567890123456789012345678901234567890"
        );
        assert_eq!(
            answers[1].0,
            StatusCode::GONE,
            "the other finds it delivered"
        );
        assert_eq!(
            answers[1].1["error"],
            format!("Request with id \"{id}\" has already been fulfilled")
        );
        let (status, _) = call(&b, get_req(&format!("/auth-api/v2/requests/{id}"))).await;
        assert_eq!(status, StatusCode::GONE);
        let kept: (String, String, Option<String>) = sqlx::query_as(
            "SELECT params::text, challenge, outcome::text FROM explore_api.auth_requests \
             WHERE request_id = $1",
        )
        .bind(&id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
        assert_eq!(
            kept,
            ("[]".into(), String::new(), None),
            "a delivered request keeps nothing but its 410"
        );
        db.teardown().await;
    }

    #[tokio::test]
    async fn an_expired_request_answers_410_once_then_404_on_any_replica() {
        let Some((db, state)) = replica().await else {
            return;
        };
        let (a, b) = (app(&state), app(&other_replica(&db).await));
        let now = now_micros();
        let mut conn = state.pool.acquire().await.unwrap();
        insert_request(
            &mut conn,
            &record("expired", "203.0.113.5", now - Duration::seconds(1)),
        )
        .await
        .unwrap();
        drop(conn);
        let (status, body) = call(&b, get_req("/auth-api/v2/requests/expired")).await;
        assert_eq!(status, StatusCode::GONE);
        assert_eq!(body["error"], "Request with id \"expired\" has expired");
        let (status, _) = call(&a, get_req("/auth-api/requests/expired")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        db.teardown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn an_identity_parked_on_one_replica_is_picked_up_once_on_another() {
        let Some((db, state)) = replica().await else {
            return;
        };
        let (a, b) = (app(&state), app(&other_replica(&db).await));
        let (status, created) = call(&a, identity_post().await).await;
        assert_eq!(status, StatusCode::CREATED, "{created}");
        let id = created["identityId"].as_str().unwrap().to_string();
        let (status, picked) = call(&b, get_req(&format!("/auth-api/identities/{id}"))).await;
        assert_eq!(status, StatusCode::OK, "{picked}");
        assert_eq!(picked["identity"]["expiration"], "2100-01-01T00:00:00.000Z");
        let (status, again) = call(&a, get_req(&format!("/auth-api/identities/{id}"))).await;
        assert_eq!(
            (status, again["error"].as_str()),
            (StatusCode::NOT_FOUND, Some("Identity was already consumed"))
        );

        let (_, created) = call(&b, identity_post().await).await;
        let id = created["identityId"].as_str().unwrap().to_string();
        let pickups: Vec<_> = [a.clone(), b.clone(), a.clone(), b.clone()]
            .into_iter()
            .map(|app| {
                let req = get_req(&format!("/auth-api/identities/{id}"));
                tokio::spawn(async move { call(&app, req).await })
            })
            .collect();
        let mut answers = Vec::new();
        for pickup in pickups {
            answers.push(pickup.await.unwrap());
        }
        let delivered = answers
            .iter()
            .filter(|(status, _)| *status == StatusCode::OK)
            .count();
        assert_eq!(delivered, 1, "{answers:?}");
        for (status, body) in answers.iter().filter(|(s, _)| *s != StatusCode::OK) {
            assert_eq!(
                (*status, body["error"].as_str()),
                (StatusCode::NOT_FOUND, Some("Identity was already consumed"))
            );
        }
        db.teardown().await;
    }

    /// A cross-origin client may send its verifier on pickup.
    #[tokio::test]
    async fn the_preflight_allows_the_identity_verifier_header() {
        use tower::ServiceExt;
        let res = Router::new()
            .nest(
                "/auth-api",
                routes().with_state(crate::api::tests_support::state_without_db().await),
            )
            .oneshot(
                Request::builder()
                    .method("OPTIONS")
                    .uri("/auth-api/identities/0f007385-b6f9-44b7-aeb2-748605afe1a9")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);
        let allowed = res.headers()[header::ACCESS_CONTROL_ALLOW_HEADERS]
            .to_str()
            .unwrap();
        assert!(
            allowed.split(", ").any(|h| h == "x-identity-verifier"),
            "{allowed}"
        );
    }

    /// An identity bound to a deep-link request is served to the verifier it
    /// was derived from, from any network; a pickup with a wrong verifier or
    /// none, from the same IP, another network or a mobile create, is refused
    /// (401) and leaves it parked.
    #[tokio::test]
    async fn a_bound_identity_is_served_only_to_its_verifier() {
        let Some(db) = scratch_database().await else {
            return;
        };
        const VERIFIER: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
        const OTHER: &str = "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";
        let pickup = |id: &str, ip: &str, verifier: Option<&str>| {
            let mut req = Request::builder()
                .uri(format!("/auth-api/identities/{id}"))
                .header("x-real-ip", ip);
            if let Some(verifier) = verifier {
                req = req.header("x-identity-verifier", verifier);
            }
            req.body(Body::empty()).unwrap()
        };
        let bound = json!({ "authRequestId": "0F007385-B6F9-44B7-AEB2-748605AFE1A9" });

        let state = state_on_pool(db.pool.clone()).await;
        let main = app(&state);
        let (status, body) = call(
            &main,
            identity_post_with(json!({ "authRequestId": "not-a-uuid" })).await,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

        let (status, created) = call(&main, identity_post_with(bound.clone()).await).await;
        assert_eq!(status, StatusCode::CREATED, "{created}");
        let id = created["identityId"].as_str().unwrap().to_string();
        let (status, body) = call(&main, pickup(&id, "203.0.113.9", Some(OTHER))).await;
        assert_eq!(
            (status, body["error"].as_str()),
            (StatusCode::UNAUTHORIZED, Some("Invalid identity verifier"))
        );
        for ip in ["203.0.113.9", "198.51.100.7"] {
            let (status, body) = call(&main, pickup(&id, ip, None)).await;
            assert_eq!(
                (status, body["error"].as_str()),
                (StatusCode::UNAUTHORIZED, Some("Invalid identity verifier")),
                "{ip}"
            );
        }
        let (status, body) = call(&main, pickup(&id, "198.51.100.7", Some(VERIFIER))).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let (status, body) = call(&main, pickup(&id, "203.0.113.9", Some(VERIFIER))).await;
        assert_eq!(
            (status, body["error"].as_str()),
            (StatusCode::NOT_FOUND, Some("Identity was already consumed"))
        );

        // An identity with no binding keeps the IP check, verifier or not.
        let (_, created) = call(&main, identity_post().await).await;
        let id = created["identityId"].as_str().unwrap().to_string();
        let (status, _) = call(&main, pickup(&id, "198.51.100.7", Some(VERIFIER))).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, body) = call(&main, pickup(&id, "203.0.113.9", None)).await;
        assert_eq!(
            (status, body["error"].as_str()),
            (
                StatusCode::NOT_FOUND,
                Some("Identity was deleted due to IP mismatch")
            )
        );

        // A mobile create skips the IP check but not the verifier.
        let mut mobile = bound.clone();
        mobile["isMobile"] = json!(true);
        let (_, created) = call(&main, identity_post_with(mobile).await).await;
        let id = created["identityId"].as_str().unwrap().to_string();
        let (status, _) = call(&main, pickup(&id, "198.51.100.7", None)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, body) = call(&main, pickup(&id, "198.51.100.7", Some(VERIFIER))).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        db.teardown().await;
    }

    #[tokio::test]
    async fn expired_identities_and_aged_tombstones_are_swept() {
        let Some((db, state)) = replica().await else {
            return;
        };
        let (a, b) = (app(&state), app(&other_replica(&db).await));
        let now = now_micros();
        let ids = [
            "8c545b52-c08e-4c39-a854-46e2b5b03c48",
            "9d545b52-c08e-4c39-a854-46e2b5b03c48",
        ];
        let mut conn = state.pool.acquire().await.unwrap();
        for id in ids {
            identities::insert_identity(
                &mut conn,
                &identities::IdentityRecord {
                    identity_id: id.into(),
                    identity: json!({}),
                    ip_address: "203.0.113.9".into(),
                    source: None,
                    is_mobile: false,
                    created_at: now - Duration::seconds(3601),
                    expiration: now - Duration::seconds(1),
                    auth_request_id: None,
                },
            )
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO explore_api.auth_identity_tombstones (identity_id, created_at) \
                 VALUES ($1, $2)",
            )
            .bind(id)
            .bind(naive(now - Duration::seconds(3601)))
            .execute(&mut *conn)
            .await
            .unwrap();
        }
        let (status, body) = call(&a, get_req(&format!("/auth-api/identities/{}", ids[0]))).await;
        assert_eq!(
            (status, body["error"].as_str()),
            (StatusCode::GONE, Some("Identity has expired")),
            "an expired identity read before the sweep"
        );
        let (status, body) = call(&b, get_req(&format!("/auth-api/identities/{}", ids[0]))).await;
        assert_eq!(
            (status, body["error"].as_str()),
            (StatusCode::NOT_FOUND, Some("Identity has expired"))
        );

        assert_eq!(sweep_expired_identities(&mut conn, now).await.unwrap(), 1);
        assert_eq!(sweep_expired_identities(&mut conn, now).await.unwrap(), 0);
        let (status, body) = call(&b, get_req(&format!("/auth-api/identities/{}", ids[1]))).await;
        assert_eq!(
            (status, body["error"].as_str()),
            (StatusCode::NOT_FOUND, Some("Identity was evicted"))
        );
        assert_eq!(
            body["createdAt"],
            json!(now - Duration::seconds(3601)),
            "the tombstone keeps its creation"
        );

        let later = now + Duration::days(14);
        assert_eq!(
            sweep_identity_tombstones(&mut conn, later).await.unwrap(),
            2
        );
        assert_eq!(
            sweep_identity_tombstones(&mut conn, later).await.unwrap(),
            0
        );
        drop(conn);
        let (status, body) = call(&a, get_req(&format!("/auth-api/identities/{}", ids[1]))).await;
        assert_eq!(
            (status, body["error"].as_str()),
            (StatusCode::NOT_FOUND, Some("Identity not found"))
        );
        db.teardown().await;
    }

    #[test]
    fn body_cap_matches_the_upstream_shared_route_cap() {
        assert_eq!(DEFAULT_BODY_SIZE_BYTES, 16 * 1024);
    }

    #[tokio::test]
    async fn oversized_bodies_are_refused_on_every_posting_route() {
        use tower::ServiceExt;

        let app = router().await;
        let oversized = padded_request_body(DEFAULT_BODY_SIZE_BYTES);
        assert!(oversized.len() > DEFAULT_BODY_SIZE_BYTES);

        for uri in [
            "/requests",
            "/requests/8c545b52-c08e-4c39-a854-46e2b5b03c48",
            "/v2/requests/8c545b52-c08e-4c39-a854-46e2b5b03c48/outcome",
            "/v2/requests/8c545b52-c08e-4c39-a854-46e2b5b03c48/validation",
            "/identities",
        ] {
            let res = app
                .clone()
                .oneshot(json_post(uri, oversized.clone()))
                .await
                .unwrap();
            assert_upstream_payload_too_large(res, uri).await;
        }
    }

    #[tokio::test]
    async fn an_oversized_body_is_refused_on_a_reading_free_get_route() {
        use tower::ServiceExt;

        let app = router().await;
        let oversized = padded_request_body(DEFAULT_BODY_SIZE_BYTES);
        let uri = "/requests/8c545b52-c08e-4c39-a854-46e2b5b03c48";

        let res = app
            .oneshot(json_request("GET", uri, oversized))
            .await
            .unwrap();
        assert_upstream_payload_too_large(res, uri).await;
    }

    #[tokio::test]
    async fn an_undeclared_oversized_body_is_refused_with_the_same_envelope() {
        use tower::ServiceExt;

        let app = router().await;
        let uri = "/requests";
        let req = Request::builder()
            .method("POST")
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/json")
            .header("x-real-ip", "203.0.113.9")
            .body(Body::from(padded_request_body(DEFAULT_BODY_SIZE_BYTES)))
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_upstream_payload_too_large(res, uri).await;
    }

    #[tokio::test]
    async fn a_body_of_exactly_the_cap_is_not_refused() {
        use tower::ServiceExt;

        let app = router().await;
        let at_cap = vec![b' '; DEFAULT_BODY_SIZE_BYTES];

        let res = app.oneshot(json_post("/requests", at_cap)).await.unwrap();
        assert_ne!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn bodies_under_the_cap_reach_the_handler() {
        use tower::ServiceExt;

        let app = router().await;
        let accepted = padded_request_body(1024);
        assert!(accepted.len() < DEFAULT_BODY_SIZE_BYTES);

        let res = app.oneshot(json_post("/requests", accepted)).await.unwrap();
        assert_ne!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }
}
