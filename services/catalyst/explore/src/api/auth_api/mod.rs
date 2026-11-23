//! `/auth-api`: the explorer and web sign-in relay. A client posts a signing
//! request, a wallet page recovers it by id and posts the outcome, and the
//! requester polls for it; a signed identity is parked for a device to
//! pick up once. The state lives in `explore_api.auth_requests`,
//! `auth_identities` and `auth_identity_tombstones` with a short TTL (5 min
//! requests, 1 h identities), so every API replica behind the gateway serves
//! the same requests (DATABASE.md "Auth API relay state workload").

use crate::shared::routing::{delete, erase, get, post};
use axum::Extension;
use axum::body::Body;
use axum::extract::{ConnectInfo, DefaultBodyLimit, FromRequest, Path, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
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

mod accounts;
mod contract_checks;
mod identities;
mod validation;

pub(crate) use accounts::MagicDeletion;
pub use contract_checks::ContractChecks;
use contract_checks::{Budgeted, Offline, Spent};

use identities::{create_identity, get_identity};
pub use identities::{sweep_expired_identities, sweep_identity_tombstones};
use validation::{
    ChainRefusal, needs_contract_check, validate_auth_chain, validate_request_message,
};

const REQUEST_TTL_SECONDS: i64 = 300;
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
                    if state.settings.magic_deletion.is_some() {
                        accounts::sweep_attempts(&mut conn).await?;
                    }
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

async fn load_request(
    state: &AppState,
    id: &str,
    now: DateTime<Utc>,
) -> sqlx::Result<Option<ChallengeRecord>> {
    let row: Option<RequestRow> = sqlx::query_as(concat!(
        "SELECT ",
        request_columns!(),
        " FROM explore_api.auth_requests WHERE request_id = $1 AND expiration >= $2"
    ))
    .bind(id)
    .bind(naive(now))
    .fetch_optional(&state.pool)
    .await?;
    Ok(row.map(RequestRow::into_record))
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
    /// `Some(Value::Null)` when the body carries `"result": null`; `None` when the key is absent.
    #[serde(
        default,
        deserialize_with = "present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<OutcomeError>,
    #[serde(rename = "authChain", default, skip_serializing_if = "Option::is_none")]
    pub auth_chain: Option<AuthChain>,
}

fn present_value<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
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
        .route("/accounts", delete(accounts::delete_account))
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
        .layer(middleware::from_fn(enforce_body_cap))
        .layer(middleware::from_fn(record_request_telemetry))
        .layer(middleware::from_fn(crate::shared::cors::decentraland_cors))
        .layer(DefaultBodyLimit::max(DEFAULT_BODY_SIZE_BYTES))
}

struct RelayJson<T>(T);

impl<S, T> FromRequest<S> for RelayJson<T>
where
    S: Send + Sync,
    T: serde::de::DeserializeOwned,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        Json::<T>::from_request(req, state)
            .await
            .map(|Json(value)| Self(value))
            .map_err(|error| {
                if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
                    payload_too_large()
                } else {
                    bad_request("Invalid request body")
                }
            })
    }
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

/// Drops requests past their expiry.
pub async fn sweep_expired_requests(
    conn: &mut PgConnection,
    now: DateTime<Utc>,
) -> sqlx::Result<u64> {
    sqlx::query("DELETE FROM explore_api.auth_requests WHERE expiration < $1")
        .bind(naive(now))
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
    RelayJson(body): RelayJson<CreateRequestBody>,
) -> Response {
    let validated = match validate_request_message(&body) {
        Ok(validated) => validated,
        Err(msg) => return bad_request(&msg),
    };
    let now = now_micros();
    let source = request_source(
        &headers,
        peer.map(|Extension(ConnectInfo(p))| p),
        &state.settings.trusted_proxy_cidrs,
    );
    let chain = validated.auth_chain;
    let offline = match validate_auth_chain(chain, Some(&Offline::default())).await {
        Ok(verified) => verified,
        Err(refusal) => return chain_refused(refusal),
    };
    let budget = budgeted(&state, source.as_deref());
    // A network with no contract-wallet budget left costs no database count.
    if needs_contract_check(chain)
        && budget.is_some()
        && let Err(spent) = state
            .contract_checks
            .has_room(source.as_deref(), std::time::Instant::now())
    {
        return chain_refused(ChainRefusal::Throttled(spent));
    }
    // Before the RPC and the admission lock.
    // A network at its cap neither reaches the RPC nor queues on the lock every
    // create takes, holding pool connections.
    match network_at_cap(&state, Pending::Requests, source.as_deref(), now).await {
        Ok(false) => {}
        Ok(true) => return too_many_from_network(),
        Err(err) => return internal_error("auth-api: counting the network's requests", err),
    }
    let sender = if needs_contract_check(chain) {
        match validate_auth_chain(chain, as_validator(&budget)).await {
            Ok(verified) => verified.sender,
            Err(refusal) => return chain_refused(throttled_if_spent(&budget, refusal)),
        }
    } else {
        offline.sender
    }
    .to_lowercase();

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
        source: source.clone(),
    };
    match admit_request(&state, &record, now).await {
        Ok(None) => {}
        Ok(Some(refusal)) => return refusal,
        Err(err) => return internal_error("auth-api: storing the request", err),
    }
    if let Some(budget) = budget {
        budget.refund();
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

/// The relay's tables with a per-network cap of live rows.
#[derive(Clone, Copy)]
enum Pending {
    Requests,
    Identities,
}

impl Pending {
    /// A network's live rows, by `source` and the expiry bound.
    fn count_sql(self) -> &'static str {
        match self {
            Pending::Requests => {
                "SELECT count(*) FROM explore_api.auth_requests \
                 WHERE source = $1 AND expiration >= $2 AND status <> 'fulfilled'"
            }
            Pending::Identities => {
                "SELECT count(*) FROM explore_api.auth_identities \
                 WHERE source = $1 AND expiration >= $2"
            }
        }
    }

    fn cap(self) -> usize {
        match self {
            Pending::Requests => MAX_REQUESTS_PER_SOURCE,
            Pending::Identities => identities::MAX_IDENTITIES_PER_SOURCE,
        }
    }
}

/// Whether `source` holds its cap of live rows in `table`, counted without
/// the admission lock; under the lock the count is taken again, exactly. No
/// source has no per-network cap.
async fn network_at_cap(
    state: &AppState,
    table: Pending,
    source: Option<&str>,
    now: DateTime<Utc>,
) -> sqlx::Result<bool> {
    let Some(source) = source else {
        return Ok(false);
    };
    let from_source: i64 = sqlx::query_scalar(table.count_sql())
        .bind(source)
        .bind(naive(now))
        .fetch_one(&state.pool)
        .await?;
    Ok(from_source >= table.cap() as i64)
}

/// The caps checked and the request stored in one transaction under the
/// admission lock, so concurrent creates on any replica cannot overshoot.
async fn admit_request(
    state: &AppState,
    record: &ChallengeRecord,
    now: DateTime<Utc>,
) -> sqlx::Result<Option<Response>> {
    // The caller already refused a network over its cap (`network_at_cap`);
    // under the lock the count is taken again, exactly.
    let mut tx = state.pool.begin().await?;
    admission_lock(&mut tx, REQUEST_ADMISSION_LOCK).await?;
    if let Some(refusal) = request_capacity_refusal(&mut tx, record.source.as_deref(), now).await? {
        return Ok(Some(refusal));
    }
    insert_request(&mut tx, record).await?;
    tx.commit().await?;
    Ok(None)
}

/// The answer for a request that is not there to work with, or the record.
async fn found_request(
    state: &AppState,
    id: &str,
    now: DateTime<Utc>,
) -> Result<ChallengeRecord, Box<Response>> {
    match load_request(state, id, now).await {
        Ok(Some(record)) => Ok(record),
        Ok(None) => Err(Box::new(not_found(id))),
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
               WHERE request_id = $1 AND status = 'signed' AND expiration >= $2 FOR UPDATE) AS signed \
         WHERE r.request_id = signed.request_id \
         RETURNING signed.outcome::text",
    )
    .bind(&id)
    .bind(naive(now_micros()))
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
    peer: Option<Extension<ConnectInfo<std::net::SocketAddr>>>,
    headers: HeaderMap,
    path: Path<String>,
    body: RelayJson<HttpOutcomeMessage>,
) -> Response {
    submit_outcome(state, peer, headers, path, body).await
}

async fn post_request_outcome_legacy(
    state: State<AppState>,
    peer: Option<Extension<ConnectInfo<std::net::SocketAddr>>>,
    headers: HeaderMap,
    path: Path<String>,
    body: RelayJson<HttpOutcomeMessage>,
) -> Response {
    submit_outcome(state, peer, headers, path, body).await
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
    peer: Option<Extension<ConnectInfo<std::net::SocketAddr>>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    RelayJson(body): RelayJson<HttpOutcomeMessage>,
) -> Response {
    if let Err(msg) = validate_outcome_body(&body) {
        return bad_request(&msg);
    }

    // The pending request first: a chain for an unknown id reaches no RPC.
    let now = now_micros();
    let record = match found_request(&state, &id, now).await {
        Ok(record) => record,
        Err(answer) => return *answer,
    };
    if record.status != ChallengeStatus::Pending {
        return already_answered(&id, record.status);
    }

    let source = request_source(
        &headers,
        peer.map(|Extension(ConnectInfo(p))| p),
        &state.settings.trusted_proxy_cidrs,
    );
    let budget = budgeted(&state, source.as_deref());
    if let Some(chain) = body.auth_chain.as_ref() {
        if let Err(refusal) = validate_auth_chain(chain, Some(&Offline::default())).await {
            return chain_refused(refusal);
        }
        if needs_contract_check(chain)
            && let Err(refusal) = validate_auth_chain(chain, as_validator(&budget)).await
        {
            return chain_refused(throttled_if_spent(&budget, refusal));
        }
    }

    let outcome = OutcomeResponseMessage {
        request_id: id.clone(),
        sender: body.sender,
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
         WHERE request_id = $1 AND status = 'pending' AND expiration >= $3",
    )
    .bind(&id)
    .bind(outcome)
    .bind(naive(now_micros()))
    .execute(&state.pool)
    .await;
    match stored {
        Ok(done) if done.rows_affected() > 0 => {
            if let Some(budget) = budget {
                budget.refund();
            }
            StatusCode::OK.into_response()
        }
        Ok(_) => match found_request(&state, &id, now_micros()).await {
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
         WHERE request_id = $1 AND status <> 'fulfilled' AND expiration >= $2",
    )
    .bind(&id)
    .bind(naive(now_micros()))
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

/// The state's validator with its calls charged to `source`'s budget of
/// contract-wallet checks; `None` without a validator.
fn budgeted<'a>(state: &'a AppState, source: Option<&'a str>) -> Option<Budgeted<'a>> {
    state
        .eip1654_validator()
        .map(|v| Budgeted::new(v, &state.contract_checks, source))
}

fn as_validator<'a>(
    budget: &'a Option<Budgeted<'_>>,
) -> Option<&'a dyn crate::auth::eip1654::Eip1654Validator> {
    budget
        .as_ref()
        .map(|b| b as &dyn crate::auth::eip1654::Eip1654Validator)
}

/// `refusal`, or a 429 when a call was refused because a budget was spent.
fn throttled_if_spent(budget: &Option<Budgeted<'_>>, refusal: ChainRefusal) -> ChainRefusal {
    match budget.as_ref().and_then(Budgeted::spent) {
        Some(spent) => ChainRefusal::Throttled(spent),
        None => refusal,
    }
}

fn chain_refused(refusal: ChainRefusal) -> Response {
    match refusal {
        ChainRefusal::Invalid(msg) => bad_request(&msg),
        ChainRefusal::Throttled(spent) => (
            StatusCode::TOO_MANY_REQUESTS,
            Json(ErrorBody {
                error: match spent {
                    Spent::Network => {
                        "Too many contract wallet signature checks from this network, try again later"
                    }
                    Spent::Replica => {
                        "Too many contract wallet signature checks on this node, try again later"
                    }
                }
                .into(),
            }),
        )
            .into_response(),
        ChainRefusal::Unavailable => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorBody {
                error: "Contract wallet signature verification unavailable".into(),
            }),
        )
            .into_response(),
    }
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
            .layer(middleware::from_fn(record_request_telemetry))
            .layer(middleware::from_fn(crate::shared::cors::decentraland_cors));
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

    #[test]
    fn a_present_null_result_is_kept_apart_from_a_missing_one() {
        let addr = "0x1234567890123456789012345678901234567890";
        let body: HttpOutcomeMessage =
            serde_json::from_value(json!({ "sender": addr, "result": null })).unwrap();
        assert_eq!(body.result, Some(Value::Null));
        assert!(validate_outcome_body(&body).is_ok());
        let missing: HttpOutcomeMessage =
            serde_json::from_value(json!({ "sender": addr })).unwrap();
        assert!(missing.result.is_none());
    }

    #[tokio::test]
    async fn an_outcome_without_result_or_error_is_rejected() {
        let (status, body) = call(
            &router().await,
            json_post(
                "/v2/requests/8c545b52-c08e-4c39-a854-46e2b5b03c48/outcome",
                serde_json::to_vec(
                    &json!({ "sender": "0x1234567890123456789012345678901234567890" }),
                )
                .unwrap(),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(
            body["error"],
            "Outcome must include either a result or an error"
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

    fn contract_request(signature: &str) -> Vec<u8> {
        let ephemeral = alloy_signer_local::PrivateKeySigner::random();
        let chain =
            validation::contract_wallet::chain(&format!("{:#x}", ephemeral.address()), signature);
        serde_json::to_vec(
            &json!({ "method": "personal_sign", "params": ["hi"], "authChain": chain }),
        )
        .unwrap()
    }

    fn contract_outcome(signature: &str) -> Vec<u8> {
        let ephemeral = alloy_signer_local::PrivateKeySigner::random();
        serde_json::to_vec(&json!({
            "sender": validation::contract_wallet::CONTRACT,
            "result": "0xsig",
            "authChain": validation::contract_wallet::chain(
                &format!("{:#x}", ephemeral.address()),
                signature,
            ),
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn contract_wallet_chains_need_the_validator() {
        use validation::contract_wallet::{SIGNATURE, Wallet};
        let state = crate::api::tests_support::state_without_db().await;
        let app = Router::new().nest("/auth-api", routes().with_state(state.clone()));
        let (status, body) = call(
            &app,
            json_post("/auth-api/requests", contract_request(SIGNATURE)),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
        assert_eq!(
            body["error"],
            "Contract wallet signature verification unavailable"
        );

        assert!(state.attach_eip1654(Some(std::sync::Arc::new(Wallet::default()))));
        let (status, body) = call(
            &app,
            json_post("/auth-api/requests", contract_request("0xbad0")),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }

    /// An identity post from the contract wallet: its signed-fetch chain's
    /// contract link signed `header_sig`, its identity chain's `identity_sig`.
    fn contract_identity_post(header_sig: &str, identity_sig: &str, peer: Option<&str>) -> Request {
        use crate::shared::signed_fetch as sf;
        use alloy_signer::SignerSync;
        use validation::contract_wallet::chain;
        let ephemeral = alloy_signer_local::PrivateKeySigner::random();
        let eph_address = format!("{:#x}", ephemeral.address());
        let identity = json!({
            "ephemeralIdentity": { "address": eph_address, "privateKey": format!("0x{}", hex::encode(ephemeral.to_bytes())), "publicKey": "0x04" },
            "expiration": "2100-01-01T00:00:00.000Z",
            "authChain": chain(&eph_address, identity_sig),
        });
        let mut req = json_post(
            "/auth-api/identities",
            serde_json::to_vec(&json!({ "identity": identity })).unwrap(),
        );
        if let Some(peer) = peer {
            req = from_peer(req, peer);
        }
        let timestamp = Utc::now().timestamp_millis().to_string();
        let payload = sf::build_payload_v6("post", "/auth-api/identities", &timestamp, "{}");
        let signature = ephemeral.sign_message_sync(payload.as_bytes()).unwrap();
        let mut links = chain(&eph_address, header_sig);
        links.push(crate::auth::AuthLink {
            link_type: crate::auth::AuthLinkType::EcdsaSignedEntity,
            payload,
            signature: Some(signature.to_string()),
        });
        let headers = req.headers_mut();
        headers.insert(sf::AUTH_TIMESTAMP_HEADER, timestamp.parse().unwrap());
        headers.insert(sf::AUTH_METADATA_HEADER, "{}".parse().unwrap());
        for (i, link) in links.iter().enumerate() {
            headers.insert(
                axum::http::HeaderName::try_from(format!("{}{i}", sf::AUTH_CHAIN_HEADER_PREFIX))
                    .unwrap(),
                serde_json::to_string(link).unwrap().parse().unwrap(),
            );
        }
        req
    }

    #[tokio::test]
    async fn a_contract_wallet_identity_is_checked_without_the_rpc_until_its_owner_matches() {
        use validation::contract_wallet::{SIGNATURE, Unreachable, Wallet, chain};
        let state = crate::api::tests_support::state_without_db().await;
        let app = Router::new().nest("/auth-api", routes().with_state(state.clone()));
        let (status, err) = call(&app, contract_identity_post(SIGNATURE, SIGNATURE, None)).await;
        assert_eq!(
            (status, err["error"].as_str()),
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Some("Contract wallet signature verification unavailable")
            ),
            "no validator"
        );

        let unreachable = crate::api::tests_support::state_without_db().await;
        assert!(unreachable.attach_eip1654(Some(std::sync::Arc::new(Unreachable))));
        let (status, err) = call(
            &Router::new().nest("/auth-api", routes().with_state(unreachable)),
            contract_identity_post(SIGNATURE, SIGNATURE, None),
        )
        .await;
        assert_eq!(
            (status, err["error"].as_str()),
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Some("Contract wallet signature verification unavailable")
            ),
            "an RPC that cannot answer the signed fetch's contract link"
        );

        let wallet = std::sync::Arc::new(Wallet::default());
        assert!(state.attach_eip1654(Some(wallet.clone())));
        let ephemeral = alloy_signer_local::PrivateKeySigner::random();
        let eph_address = format!("{:#x}", ephemeral.address());
        let identity = json!({
            "ephemeralIdentity": { "address": eph_address, "privateKey": format!("0x{}", hex::encode(ephemeral.to_bytes())), "publicKey": "0x04" },
            "expiration": "2100-01-01T00:00:00.000Z",
            "authChain": chain(&eph_address, SIGNATURE),
        });
        let mut req = json_post(
            "/auth-api/identities",
            serde_json::to_vec(&json!({ "identity": identity })).unwrap(),
        );
        req.headers_mut().extend(
            crate::shared::signed_fetch::test_support::signed_headers(
                "post",
                "/auth-api/identities",
                "{}",
                true,
            )
            .await,
        );
        let (status, err) = call(&app, req).await;
        assert_eq!(
            (status, err["error"].as_str()),
            (
                StatusCode::FORBIDDEN,
                Some("Request sender does not match identity owner")
            )
        );
        assert_eq!(wallet.calls(), 0, "a 403 costs no contract call");

        let mut forged = contract_identity_post(SIGNATURE, SIGNATURE, None);
        let ts = forged.headers()[crate::shared::signed_fetch::AUTH_TIMESTAMP_HEADER]
            .to_str()
            .unwrap()
            .parse::<i64>()
            .unwrap();
        forged.headers_mut().insert(
            crate::shared::signed_fetch::AUTH_TIMESTAMP_HEADER,
            (ts + 1).to_string().parse().unwrap(),
        );
        let (status, err) = call(&app, forged).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{err}");
        assert_eq!(
            wallet.calls(),
            0,
            "a forged request chain costs no contract call"
        );

        let (status, err) = call(&app, contract_identity_post(SIGNATURE, "0xbad0", None)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{err}");
        assert!(wallet.calls() > 0);
    }

    #[tokio::test]
    async fn a_network_that_spends_its_contract_checks_reaches_no_validator() {
        use validation::contract_wallet::{SIGNATURE, Wallet};
        let Some((db, state)) = replica().await else {
            return;
        };
        let wallet = std::sync::Arc::new(Wallet::default());
        assert!(state.attach_eip1654(Some(wallet.clone())));
        let app = app(&state);
        let mut refused = 0;
        let throttled = loop {
            let (status, err) = call(
                &app,
                contract_identity_post("0xbad0", SIGNATURE, Some("203.0.113.5:1")),
            )
            .await;
            if status != StatusCode::UNAUTHORIZED {
                break (status, err);
            }
            refused += 1;
            assert!(refused <= contract_checks::PER_SOURCE, "never throttled");
        };
        assert_eq!(
            (throttled.0, throttled.1["error"].as_str()),
            (
                StatusCode::TOO_MANY_REQUESTS,
                Some(
                    "Too many contract wallet signature checks from this network, try again later"
                )
            ),
            "the signed-fetch chain is charged too"
        );
        assert_eq!(
            wallet.calls() as u32,
            contract_checks::PER_SOURCE,
            "each call is charged"
        );
        let asked = wallet.calls();
        let (status, _) = call(
            &app,
            contract_identity_post("0xbad0", SIGNATURE, Some("203.0.113.5:2")),
        )
        .await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            wallet.calls(),
            asked,
            "a spent network reaches no validator"
        );
        let (status, _) = call(
            &app,
            contract_identity_post("0xbad0", SIGNATURE, Some("198.51.100.7:1")),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "another network is still asked"
        );
        assert!(wallet.calls() > asked);
        db.teardown().await;
    }

    #[tokio::test]
    async fn a_network_with_no_budget_left_is_refused_before_any_count() {
        use validation::contract_wallet::{SIGNATURE, Wallet};
        // No database behind this state: a count would answer 500.
        let state = crate::api::tests_support::state_without_db().await;
        let wallet = std::sync::Arc::new(Wallet::default());
        assert!(state.attach_eip1654(Some(wallet.clone())));
        for _ in 0..contract_checks::PER_SOURCE {
            assert!(
                state
                    .contract_checks
                    .charge(Some("203.0.113.5"), std::time::Instant::now())
                    .is_ok()
            );
        }
        let app = Router::new().nest("/auth-api", routes().with_state(state));
        let spent =
            Some("Too many contract wallet signature checks from this network, try again later");
        let (status, err) = call(
            &app,
            from_peer(
                json_post("/auth-api/requests", contract_request(SIGNATURE)),
                "203.0.113.5:1",
            ),
        )
        .await;
        assert_eq!(
            (status, err["error"].as_str()),
            (StatusCode::TOO_MANY_REQUESTS, spent)
        );
        let (status, err) = call(
            &app,
            contract_identity_post(SIGNATURE, SIGNATURE, Some("203.0.113.5:1")),
        )
        .await;
        assert_eq!(
            (status, err["error"].as_str()),
            (StatusCode::TOO_MANY_REQUESTS, spent)
        );
        assert_eq!(wallet.calls(), 0);
    }

    #[tokio::test]
    async fn a_network_at_its_identity_cap_spends_no_contract_call() {
        use validation::contract_wallet::{SIGNATURE, Wallet};
        let Some((db, state)) = replica().await else {
            return;
        };
        let wallet = std::sync::Arc::new(Wallet::default());
        assert!(state.attach_eip1654(Some(wallet.clone())));
        let app = app(&state);
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
        let (status, err) = call(
            &app,
            contract_identity_post(SIGNATURE, SIGNATURE, Some("203.0.113.5:1")),
        )
        .await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{err}");
        assert_eq!(wallet.calls(), 0);
        db.teardown().await;
    }

    #[tokio::test]
    async fn a_contract_wallet_request_is_created_with_the_contract_as_sender() {
        use validation::contract_wallet::{CONTRACT, SIGNATURE, Wallet};
        let Some((db, state)) = replica().await else {
            return;
        };
        let wallet = std::sync::Arc::new(Wallet::default());
        assert!(state.attach_eip1654(Some(wallet.clone())));
        let app = app(&state);
        let (status, created) = call(
            &app,
            json_post("/auth-api/requests", contract_request(SIGNATURE)),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{created}");
        let id = created["requestId"].as_str().unwrap();
        let (status, recovered) = call(&app, get_req(&format!("/auth-api/v2/requests/{id}"))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(recovered["sender"], CONTRACT);

        let asked = wallet.calls();
        let (status, _) = call(
            &app,
            json_post(
                "/auth-api/v2/requests/8c545b52-c08e-4c39-a854-46e2b5b03c48/outcome",
                contract_outcome(SIGNATURE),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(wallet.calls(), asked, "an unknown id reaches no validator");

        let without_validator = self::app(&other_replica(&db).await);
        let (status, body) = call(
            &without_validator,
            json_post(
                &format!("/auth-api/v2/requests/{id}/outcome"),
                contract_outcome(SIGNATURE),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
        let (status, body) = call(
            &app,
            json_post(
                &format!("/auth-api/v2/requests/{id}/outcome"),
                contract_outcome("0xbad0"),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        let (status, body) = call(
            &app,
            json_post(
                &format!("/auth-api/v2/requests/{id}/outcome"),
                contract_outcome(SIGNATURE),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        db.teardown().await;
    }

    #[tokio::test]
    async fn a_spent_budget_says_whose_it_was() {
        for (spent, says) in [
            (
                Spent::Network,
                "Too many contract wallet signature checks from this network, try again later",
            ),
            (
                Spent::Replica,
                "Too many contract wallet signature checks on this node, try again later",
            ),
        ] {
            let res = chain_refused(ChainRefusal::Throttled(spent));
            assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
            let body = axum::body::to_bytes(res.into_body(), 1024).await.unwrap();
            assert_eq!(
                serde_json::from_slice::<Value>(&body).unwrap()["error"],
                says
            );
        }
    }

    #[tokio::test]
    async fn a_malformed_chain_is_refused_before_the_network_count() {
        let state = crate::api::tests_support::state_without_db().await;
        let app = Router::new().nest("/auth-api", routes().with_state(state));
        let mut chain = delegated("0x00000000000000000000000000000000000000e1");
        chain[1].signature = Some(format!("0x{}", "11".repeat(65)));
        let body = serde_json::to_vec(
            &json!({ "method": "personal_sign", "params": ["hi"], "authChain": chain }),
        )
        .unwrap();
        let (status, err) = call(
            &app,
            from_peer(json_post("/auth-api/requests", body), "203.0.113.5:1"),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "no database behind this state: {err}"
        );
    }

    #[tokio::test]
    async fn a_network_that_spends_its_refused_contract_checks_posts_no_outcome() {
        use validation::contract_wallet::{SIGNATURE, Wallet};
        let Some((db, state)) = replica().await else {
            return;
        };
        let wallet = std::sync::Arc::new(Wallet::default());
        assert!(state.attach_eip1654(Some(wallet.clone())));
        let app = app(&state);
        let (status, created) = call(
            &app,
            from_peer(
                json_post("/auth-api/requests", contract_request(SIGNATURE)),
                "198.51.100.7:1",
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{created}");
        let outcome = format!(
            "/auth-api/v2/requests/{}/outcome",
            created["requestId"].as_str().unwrap()
        );
        let post = |signature: &str, peer: &str| {
            from_peer(json_post(&outcome, contract_outcome(signature)), peer)
        };
        let before = wallet.calls();
        let mut refused = 0;
        while wallet.calls() - before < contract_checks::PER_SOURCE as usize {
            let (status, body) = call(&app, post("0xbad0", "203.0.113.5:1")).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
            refused += 1;
            assert!(refused <= contract_checks::PER_SOURCE);
        }
        let asked = wallet.calls();
        let (status, body) = call(&app, post(SIGNATURE, "203.0.113.5:2")).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
        assert_eq!(
            wallet.calls(),
            asked,
            "a spent network reaches no validator"
        );
        let (status, body) = call(&app, post(SIGNATURE, "198.51.100.7:2")).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "another network still answers: {body}"
        );
        db.teardown().await;
    }

    #[tokio::test]
    async fn a_null_result_is_delivered_as_null() {
        let Some((_db, state)) = replica().await else {
            return;
        };
        let app = app(&state);
        let ephemeral = alloy_signer_local::PrivateKeySigner::random();
        let chain = delegated(&format!("{:#x}", ephemeral.address()));
        let body = serde_json::to_vec(
            &json!({ "method": "eth_sendTransaction", "params": [{}], "authChain": chain }),
        )
        .unwrap();
        let (status, created) = call(&app, json_post("/auth-api/requests", body)).await;
        assert_eq!(status, StatusCode::CREATED, "{created}");
        let id = created["requestId"].as_str().unwrap().to_string();

        let outcome = serde_json::to_vec(
            &json!({ "sender": "0x1234567890123456789012345678901234567890", "result": null }),
        )
        .unwrap();
        let (status, body) = call(
            &app,
            json_post(&format!("/auth-api/v2/requests/{id}/outcome"), outcome),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let (status, answered) = call(&app, get_req(&format!("/auth-api/requests/{id}"))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(answered.get("result"), Some(&Value::Null), "{answered}");
        assert!(answered.get("error").is_none());
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

        let sender = "0xabcdefABCDEFabcdefABCDEFabcdefABCDEFabcd";
        let outcome = serde_json::to_vec(&json!({ "sender": sender, "result": "0xsig" })).unwrap();
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
        assert_eq!(answered["sender"], sender);
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
        let wallet = std::sync::Arc::new(validation::contract_wallet::Wallet::default());
        assert!(state.attach_eip1654(Some(wallet.clone())));
        let (status, _) = call(
            &app,
            from_peer(
                json_post(
                    "/auth-api/requests",
                    contract_request(validation::contract_wallet::SIGNATURE),
                ),
                "203.0.113.5:4001",
            ),
        )
        .await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            wallet.calls(),
            0,
            "a network over its cap reaches no validator"
        );
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
        seed_requests(
            &state,
            MAX_PENDING_REQUESTS,
            ChallengeStatus::Pending,
            now - Duration::seconds(1),
            now,
        )
        .await;
        assert_eq!(
            refusal(&state, Some("new"), now).await,
            None,
            "expired requests hold no slot"
        );
        seed_requests(
            &state,
            MAX_PENDING_REQUESTS,
            ChallengeStatus::Pending,
            now + Duration::seconds(REQUEST_TTL_SECONDS),
            now,
        )
        .await;
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
    async fn the_sweep_removes_expired_requests() {
        let Some((db, state)) = replica().await else {
            return;
        };
        let now = now_micros();
        let mut conn = state.pool.acquire().await.unwrap();
        for (id, expiration) in [
            ("old", now - Duration::seconds(300)),
            ("just-expired", now - Duration::seconds(1)),
            ("live", now + Duration::seconds(60)),
        ] {
            insert_request(&mut conn, &record(id, "", expiration))
                .await
                .unwrap();
        }
        assert_eq!(sweep_expired_requests(&mut conn, now).await.unwrap(), 2);
        assert_eq!(
            sweep_expired_requests(&mut conn, now).await.unwrap(),
            0,
            "a second sweep, as another replica runs it, finds nothing"
        );
        drop(conn);
        assert_eq!(request_ids(&state).await, ["live"]);
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
    async fn an_expired_request_answers_404_on_any_replica() {
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
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "Request with id \"expired\" not found");
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

    #[tokio::test]
    async fn auth_routes_use_the_allowlist_for_preflight_and_responses() {
        use tower::ServiceExt;
        let state = crate::api::tests_support::state_without_db().await;
        let app = crate::api::router(state);
        for (origin, allowed) in [
            ("https://decentraland.org", true),
            ("http://localhost:3000", true),
            ("https://decentraland.org.example.com", false),
            ("null", false),
        ] {
            let preflight = Request::builder()
                .method("OPTIONS")
                .uri("/auth-api/identities/0f007385-b6f9-44b7-aeb2-748605afe1a9")
                .header(header::ORIGIN, origin)
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                .header(
                    header::ACCESS_CONTROL_REQUEST_HEADERS,
                    "x-identity-verifier",
                )
                .body(Body::empty())
                .unwrap();
            let response = app.clone().oneshot(preflight).await.unwrap();
            assert_eq!(response.status(), StatusCode::NO_CONTENT);
            assert_eq!(
                response.headers().get(header::ACCESS_CONTROL_ALLOW_HEADERS),
                allowed
                    .then(|| HeaderValue::from_static("x-identity-verifier"))
                    .as_ref(),
            );
            let request = Request::builder()
                .uri("/auth-api/health")
                .header(header::ORIGIN, origin)
                .body(Body::empty())
                .unwrap();
            for response in [response, app.clone().oneshot(request).await.unwrap()] {
                assert_eq!(
                    response.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN),
                    allowed.then(|| HeaderValue::from_static(origin)).as_ref(),
                );
            }
        }
    }

    #[tokio::test]
    async fn invalid_json_bodies_use_the_relay_error_envelope() {
        let state = crate::api::tests_support::state_without_db().await;
        let app = app(&state);
        for uri in [
            "/auth-api/requests",
            "/auth-api/requests/test",
            "/auth-api/v2/requests/test/outcome",
            "/auth-api/identities",
        ] {
            for request in [
                json_post(uri, b"{".to_vec()),
                json_post(uri, b"[]".to_vec()),
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .body(Body::from("{}"))
                    .unwrap(),
            ] {
                let (status, body) = call(&app, request).await;
                assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
                assert_eq!(body, json!({ "error": "Invalid request body" }));
            }
        }
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
