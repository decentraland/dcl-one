use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::api::internal_error;
use crate::auth::{AuthChain, AuthError};
use crate::shared::signed_fetch::{self, PayloadPolicy, Signer};

use super::contract_checks::{Budgeted, Offline};
use super::validation::{self, ChainRefusal, needs_contract_check, validate_identity_chain};
use super::{
    ErrorBody, EvictedBody, Pending, admission_lock, as_validator, bad_request, budgeted,
    chain_refused, client_ip, ips_match, is_valid_uuid, naive, network_at_cap, normalize_ip,
    now_micros, throttled_if_spent,
};
use crate::api::Shared as AppState;

const IDENTITY_TTL_SECONDS: i64 = 3600;
const IDENTITY_STATUS_TTL_SECONDS: i64 = 14 * 24 * 3600;
const MAX_PENDING_IDENTITIES: usize = 10_000;
pub(super) const MAX_IDENTITIES_PER_SOURCE: usize = 100;
const MAX_IDENTITY_TOMBSTONES: usize = 50_000;
const IDENTITY_SIGNED_PATH: &str = "/identities";
/// Serializes identity admission across replicas (see `admission_lock`).
const IDENTITY_ADMISSION_LOCK: &str = "explore_api.auth_identities:admission";
/// The secret a deep-link client presents on pickup; its `authRequestId` is
/// derived from it.
const IDENTITY_VERIFIER_HEADER: &str = "x-identity-verifier";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeletionReason {
    Consumed,
    Expired,
    IpMismatch,
}

impl DeletionReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::Consumed => "consumed",
            Self::Expired => "expired",
            Self::IpMismatch => "ip_mismatch",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct IdentityRecord {
    pub identity_id: String,
    pub identity: Value,
    pub ip_address: String,
    /// The creating network's `request_source` key, for the pending cap.
    pub source: Option<String>,
    pub is_mobile: bool,
    pub created_at: DateTime<Utc>,
    pub expiration: DateTime<Utc>,
    /// The deep-link request UUID the identity is bound to, lowercase.
    pub auth_request_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct IdentityRequestBody {
    pub identity: Value,
    #[serde(rename = "isMobile", default)]
    pub is_mobile: Option<bool>,
    #[serde(rename = "authRequestId", default)]
    pub auth_request_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StoredIdentity {
    #[serde(rename = "ephemeralIdentity")]
    ephemeral_identity: EphemeralIdentity,
    #[serde(rename = "authChain")]
    auth_chain: AuthChain,
}

#[derive(Debug, Deserialize)]
struct EphemeralIdentity {
    address: String,
    #[serde(rename = "privateKey")]
    private_key: String,
}

/// The stored identity's fields the handler never reads still have to be well-formed: an
/// AuthIdentity without a parseable `expiration` or an `ephemeralIdentity.publicKey` is refused.
fn identity_shape_error(identity: &Value) -> Option<String> {
    match identity.get("expiration") {
        None => return Some("missing field `expiration`".into()),
        Some(value) => {
            if let Err(err) = serde_json::from_value::<DateTime<Utc>>(value.clone()) {
                return Some(format!("expiration: {err}"));
            }
        }
    }
    match identity["ephemeralIdentity"].get("publicKey") {
        None => Some("missing field `publicKey`".into()),
        Some(value) if !value.is_string() => Some("publicKey: expected a string".into()),
        Some(_) => None,
    }
}

#[derive(Debug, Serialize)]
pub struct IdentityResponse {
    #[serde(rename = "identityId")]
    pub identity_id: String,
    pub expiration: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct IdentityIdValidationResponse {
    pub identity: Value,
}

pub(super) async fn create_identity(
    State(state): State<AppState>,
    peer: Option<axum::Extension<axum::extract::ConnectInfo<std::net::SocketAddr>>>,
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
    headers: HeaderMap,
    Json(body): Json<IdentityRequestBody>,
) -> Response {
    let source = super::request_source(
        &headers,
        peer.map(|axum::Extension(axum::extract::ConnectInfo(p))| p),
        &state.settings.trusted_proxy_cidrs,
    );
    // Every check that needs no RPC first, contract-wallet links taken as signed.
    let header_offline = Offline::default();
    let request_signer =
        match verify_identity_post(&headers, uri.path(), Some(&header_offline)).await {
            Ok(signer) => signer,
            Err(refusal) => return *refusal,
        };
    if body.identity.is_null() {
        return bad_request("AuthIdentity is required in request body");
    }
    if body
        .auth_request_id
        .as_deref()
        .is_some_and(|id| !is_valid_uuid(id))
    {
        return bad_request("authRequestId must be a UUID v4");
    }
    let parsed: StoredIdentity = match serde_json::from_value(body.identity.clone()) {
        Ok(parsed) => parsed,
        Err(err) => return bad_request(&format!("Invalid AuthIdentity: {err}")),
    };
    if let Some(err) = identity_shape_error(&body.identity) {
        return bad_request(&format!("Invalid AuthIdentity: {err}"));
    }
    // Contract-wallet links are taken as signed until every other check passed.
    let chain = &parsed.auth_chain;
    let (owner, final_authority) =
        match validate_identity_chain(chain, Some(&Offline::default())).await {
            Ok(authorities) => authorities,
            Err(refusal) => return chain_refused(refusal),
        };
    if !parsed
        .ephemeral_identity
        .address
        .eq_ignore_ascii_case(&final_authority)
    {
        return forbidden("Ephemeral wallet address does not match auth chain final authority");
    }
    if !request_signer.as_str().eq_ignore_ascii_case(&owner) {
        return forbidden("Request sender does not match identity owner");
    }
    let derives_address = parsed
        .ephemeral_identity
        .private_key
        .trim()
        .parse::<alloy_signer_local::PrivateKeySigner>()
        .is_ok_and(|wallet| {
            format!("{:#x}", wallet.address())
                .eq_ignore_ascii_case(&parsed.ephemeral_identity.address)
        });
    if !derives_address {
        return forbidden("Ephemeral private key does not match the provided address");
    }
    let budget = budgeted(&state, source.as_deref());
    // Without a validator no contract call is made: the 503 below needs no count.
    if budget.is_some() && (header_offline.asked() || needs_contract_check(chain)) {
        // A spent budget costs no database count; a network at its cap no contract call.
        if let Err(spent) = state
            .contract_checks
            .has_room(source.as_deref(), std::time::Instant::now())
        {
            return chain_refused(ChainRefusal::Throttled(spent));
        }
        match network_at_cap(&state, Pending::Identities, source.as_deref(), now_micros()).await {
            Ok(false) => {}
            Ok(true) => return too_many_identities_from_network(),
            Err(err) => return internal_error("auth-api: counting the network's identities", err),
        }
    }
    if header_offline.asked()
        && let Err(refusal) =
            verify_identity_post(&headers, uri.path(), as_validator(&budget)).await
    {
        return match budget.as_ref().and_then(Budgeted::spent) {
            Some(spent) => chain_refused(ChainRefusal::Throttled(spent)),
            None => *refusal,
        };
    }
    if needs_contract_check(chain)
        && let Err(refusal) = validate_identity_chain(chain, as_validator(&budget)).await
    {
        return chain_refused(throttled_if_spent(&budget, refusal));
    }

    let identity_id = Uuid::new_v4().to_string();
    let now = now_micros();
    let expiration = now + Duration::seconds(IDENTITY_TTL_SECONDS);
    let record = IdentityRecord {
        identity_id: identity_id.clone(),
        identity: body.identity,
        ip_address: client_ip(&headers),
        source: source.clone(),
        is_mobile: body.is_mobile.unwrap_or(false),
        created_at: now,
        expiration,
        auth_request_id: body.auth_request_id.map(|id| id.to_lowercase()),
    };
    match admit_identity(&state, &record, now).await {
        Ok(None) => {}
        Ok(Some(refusal)) => return refusal,
        Err(err) => return internal_error("auth-api: storing the identity", err),
    }
    if let Some(budget) = budget {
        budget.refund();
    }
    (
        StatusCode::CREATED,
        Json(IdentityResponse {
            identity_id,
            expiration,
        }),
    )
        .into_response()
}

/// Rows held, unexpired identities, and those of them from `source`.
async fn identity_counts(
    conn: &mut PgConnection,
    source: Option<&str>,
    now: DateTime<Utc>,
) -> sqlx::Result<(i64, i64, i64)> {
    sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE expiration >= $1), \
                count(*) FILTER (WHERE expiration >= $1 AND source = $2) \
         FROM explore_api.auth_identities",
    )
    .bind(naive(now))
    .bind(source)
    .fetch_one(conn)
    .await
}

fn too_many_identities_from_network() -> Response {
    (
        StatusCode::TOO_MANY_REQUESTS,
        Json(ErrorBody {
            error: "Too many pending sign-ins from this network, try again later".into(),
        }),
    )
        .into_response()
}

/// The caps checked and the identity (and, while fewer than
/// `MAX_IDENTITY_TOMBSTONES` are kept, its tombstone) stored in one
/// transaction under the admission lock.
async fn admit_identity(
    state: &AppState,
    record: &IdentityRecord,
    now: DateTime<Utc>,
) -> sqlx::Result<Option<Response>> {
    let mut tx = state.pool.begin().await?;
    admission_lock(&mut tx, IDENTITY_ADMISSION_LOCK).await?;
    let source = record.source.as_deref();
    let mut counts = identity_counts(&mut tx, source, now).await?;
    if counts.0 >= MAX_PENDING_IDENTITIES as i64 {
        sweep_expired_identities(&mut tx, now).await?;
        sweep_identity_tombstones(&mut tx, now).await?;
        counts = identity_counts(&mut tx, source, now).await?;
    }
    let (_, live, from_source) = counts;
    if live >= MAX_PENDING_IDENTITIES as i64 {
        return Ok(Some(
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorBody {
                    error: "Too many pending identities, retry shortly".into(),
                }),
            )
                .into_response(),
        ));
    }
    if source.is_some() && from_source >= MAX_IDENTITIES_PER_SOURCE as i64 {
        return Ok(Some(too_many_identities_from_network()));
    }
    insert_identity(&mut tx, record).await?;
    sqlx::query(
        "INSERT INTO explore_api.auth_identity_tombstones (identity_id, created_at) \
         SELECT $1, $2 \
         WHERE (SELECT count(*) FROM explore_api.auth_identity_tombstones) < $3",
    )
    .bind(&record.identity_id)
    .bind(naive(record.created_at))
    .bind(MAX_IDENTITY_TOMBSTONES as i64)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(None)
}

/// Stores a parked identity (the create handler; tests seed rows with it).
pub(super) async fn insert_identity(
    conn: &mut PgConnection,
    record: &IdentityRecord,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO explore_api.auth_identities (identity_id, identity, ip_address, source, \
         is_mobile, created_at, expiration, auth_request_id) \
         VALUES ($1, $2::json, $3, $4, $5, $6, $7, $8)",
    )
    .bind(&record.identity_id)
    .bind(record.identity.to_string())
    .bind(&record.ip_address)
    .bind(&record.source)
    .bind(record.is_mobile)
    .bind(naive(record.created_at))
    .bind(naive(record.expiration))
    .bind(&record.auth_request_id)
    .execute(conn)
    .await
    .map(drop)
}

/// The signed POST of the identity's owner over `/identities` (the gateway
/// path `/auth-api/identities` counts: the old gateway sent it as
/// `x-original-path`, which this handler sets from the request's own path when
/// the client did not); a chain a scene signed on a player's behalf is refused.
async fn verify_identity_post(
    headers: &HeaderMap,
    public_path: &str,
    validator: Option<&dyn crate::auth::eip1654::Eip1654Validator>,
) -> Result<Signer, Box<Response>> {
    let mut headers = headers.clone();
    if !headers.contains_key(signed_fetch::ORIGINAL_PATH_HEADER)
        && let Ok(v) = public_path.parse()
    {
        headers.insert(signed_fetch::ORIGINAL_PATH_HEADER, v);
    }
    signed_fetch::require_signer(
        &headers,
        "post",
        IDENTITY_SIGNED_PATH,
        PayloadPolicy::V6Only,
        validator,
    )
    .await
    .map_err(|err| {
        // A contract wallet no validator could decide: logged and answered as on
        // the other relay routes.
        let undecided = match &err {
            signed_fetch::SignedFetchError::EipNotImplemented => {
                Some(AuthError::Eip1654NotImplemented)
            }
            signed_fetch::SignedFetchError::CatalystUnavailable(detail) => {
                Some(AuthError::Eip1654ValidationFailed(detail.clone()))
            }
            _ => None,
        };
        if let Some(undecided) = undecided {
            return Box::new(chain_refused(validation::refusal(undecided)));
        }
        let (_, message) = err.http_status_and_message();
        Box::new(
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": message,
                    "message": "This endpoint requires a signed fetch request. See ADR-44.",
                })),
            )
                .into_response(),
        )
    })
}

/// Drops the identities past their expiry; their tombstones stay, without
/// a reason, so a late pickup answers "evicted".
pub async fn sweep_expired_identities(
    conn: &mut PgConnection,
    now: DateTime<Utc>,
) -> sqlx::Result<u64> {
    sqlx::query("DELETE FROM explore_api.auth_identities WHERE expiration < $1")
        .bind(naive(now))
        .execute(conn)
        .await
        .map(|done| done.rows_affected())
}

/// Drops the tombstones older than their 14 days.
pub async fn sweep_identity_tombstones(
    conn: &mut PgConnection,
    now: DateTime<Utc>,
) -> sqlx::Result<u64> {
    let horizon = now - Duration::seconds(IDENTITY_STATUS_TTL_SECONDS);
    sqlx::query("DELETE FROM explore_api.auth_identity_tombstones WHERE created_at < $1")
        .bind(naive(horizon))
        .execute(conn)
        .await
        .map(|done| done.rows_affected())
}

pub(super) async fn get_identity(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if !is_valid_uuid(&id) {
        return bad_request("Invalid identity format");
    }
    match pick_up_identity(&state, &headers, &id).await {
        Ok(answer) => answer,
        Err(err) => internal_error("auth-api: picking up the identity", err),
    }
}

/// The `authRequestId` a deep-link client derives from its verifier: the
/// first 16 bytes of SHA-256 over the verifier (base64url of 32 bytes) with
/// the UUID v4 version and variant bits set. `None` for a malformed verifier.
fn derive_auth_request_id(verifier: &str) -> Option<String> {
    if verifier.len() != 43
        || !verifier
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return None;
    }
    let digest = <sha2::Sha256 as sha2::Digest>::digest(verifier.as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Some(Uuid::from_bytes(bytes).hyphenated().to_string())
}

/// How a pickup's verifier relates to the identity's binding.
#[derive(Debug, PartialEq, Eq)]
enum Verification {
    /// The verifier matches: the IP check is skipped.
    Verified,
    /// No binding: the IP check applies.
    Unbound,
    /// A bound identity with a wrong, malformed or missing verifier.
    Refused,
}

fn verify_pickup(auth_request_id: Option<&str>, verifier: Option<&str>) -> Verification {
    let Some(bound) = auth_request_id else {
        return Verification::Unbound;
    };
    match verifier.and_then(derive_auth_request_id) {
        Some(derived) if derived == bound => Verification::Verified,
        _ => Verification::Refused,
    }
}

fn error_response(status: StatusCode, error: &str) -> Response {
    (
        status,
        Json(ErrorBody {
            error: error.into(),
        }),
    )
        .into_response()
}

/// Consumes the identity once. The row is locked while the pickup is
/// decided, and deleted with its tombstone's reason in the same transaction
/// when it is served, expired or refused for its IP, so a concurrent pickup
/// on any replica waits on the row, finds it gone and reads the reason. A
/// bound identity is served only to its verifier, from any network; a pickup
/// without it (401) keeps the identity for its own client.
async fn pick_up_identity(
    state: &AppState,
    headers: &HeaderMap,
    id: &str,
) -> sqlx::Result<Response> {
    let now = now_micros();
    let mut tx = state.pool.begin().await?;
    type Parked = (String, String, bool, NaiveDateTime, Option<String>);
    let parked: Option<Parked> = sqlx::query_as(
        "SELECT identity::text, ip_address, is_mobile, expiration, auth_request_id \
         FROM explore_api.auth_identities WHERE identity_id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((identity, ip_address, is_mobile, expiration, auth_request_id)) = parked else {
        tx.commit().await?;
        return identity_status_response(state, id).await;
    };
    let identity: Value = serde_json::from_str(&identity).unwrap_or(Value::Null);
    let signer = identity_owner(&identity).unwrap_or_default();

    let request_ip = client_ip(headers);
    let ip_mismatch =
        !ip_address.is_empty() && !request_ip.is_empty() && !ips_match(&ip_address, &request_ip);
    let verifier = headers
        .get(IDENTITY_VERIFIER_HEADER)
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty());
    let (reason, answer) = if expiration.and_utc() < now {
        (
            DeletionReason::Expired,
            error_response(StatusCode::GONE, "Identity has expired"),
        )
    } else {
        let verification = verify_pickup(auth_request_id.as_deref(), verifier);
        if verification == Verification::Refused {
            tracing::warn!(%signer, "identity pickup with a missing or wrong verifier; identity kept");
            tx.commit().await?;
            return Ok(error_response(
                StatusCode::UNAUTHORIZED,
                "Invalid identity verifier",
            ));
        }
        if verification == Verification::Unbound && !is_mobile && ip_mismatch {
            (
                DeletionReason::IpMismatch,
                error_response(StatusCode::FORBIDDEN, "IP address mismatch"),
            )
        } else {
            if !is_mobile
                && !ip_mismatch
                && !ip_address.is_empty()
                && !request_ip.is_empty()
                && normalize_ip(&ip_address) != normalize_ip(&request_ip)
            {
                tracing::warn!(
                    stored = %ip_address,
                    request = %request_ip,
                    %signer,
                    "identity IP matched only by /24 prefix"
                );
            }
            (
                DeletionReason::Consumed,
                (
                    StatusCode::OK,
                    Json(IdentityIdValidationResponse { identity }),
                )
                    .into_response(),
            )
        }
    };
    sqlx::query("DELETE FROM explore_api.auth_identities WHERE identity_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    // The create stored the tombstone while fewer than the cap were kept: the
    // pickup sets its reason. One that has none (the table was full at the
    // create) gets one only under the same cap, so pickups never grow the
    // table past it.
    let updated = sqlx::query(
        "UPDATE explore_api.auth_identity_tombstones SET deletion_reason = $2 \
         WHERE identity_id = $1",
    )
    .bind(id)
    .bind(reason.as_str())
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() == 0 {
        sqlx::query(
            "INSERT INTO explore_api.auth_identity_tombstones (identity_id, created_at, deletion_reason) \
             SELECT $1, $2, $3 \
             WHERE (SELECT count(*) FROM explore_api.auth_identity_tombstones) < $4 \
             ON CONFLICT (identity_id) DO UPDATE SET deletion_reason = EXCLUDED.deletion_reason",
        )
        .bind(id)
        .bind(naive(now))
        .bind(reason.as_str())
        .bind(MAX_IDENTITY_TOMBSTONES as i64)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(answer)
}

async fn identity_status_response(state: &AppState, id: &str) -> sqlx::Result<Response> {
    let status: Option<(NaiveDateTime, Option<String>)> = sqlx::query_as(
        "SELECT created_at, deletion_reason FROM explore_api.auth_identity_tombstones \
         WHERE identity_id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;
    let Some((created_at, reason)) = status else {
        return Ok((
            StatusCode::NOT_FOUND,
            Json(ErrorBody {
                error: "Identity not found".into(),
            }),
        )
            .into_response());
    };
    let error = match reason.as_deref() {
        Some("consumed") => "Identity was already consumed",
        Some("expired") => "Identity has expired",
        Some("ip_mismatch") => "Identity was deleted due to IP mismatch",
        _ => {
            return Ok((
                StatusCode::NOT_FOUND,
                Json(EvictedBody {
                    error: "Identity was evicted".into(),
                    created_at: created_at.and_utc(),
                }),
            )
                .into_response());
        }
    };
    Ok((
        StatusCode::NOT_FOUND,
        Json(ErrorBody {
            error: error.into(),
        }),
    )
        .into_response())
}

fn identity_owner(identity: &Value) -> Option<String> {
    identity
        .get("authChain")
        .and_then(|c| c.as_array())
        .and_then(|links| links.first())
        .and_then(|link| link.get("payload"))
        .and_then(|p| p.as_str())
        .map(|s| s.to_lowercase())
}

fn forbidden(msg: &str) -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(ErrorBody {
            error: msg.to_string(),
        }),
    )
        .into_response()
}

#[cfg(test)]
mod verifier_tests {
    use super::*;

    const VERIFIER: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    const REQUEST_ID: &str = "0f007385-b6f9-44b7-aeb2-748605afe1a9";

    #[test]
    fn the_request_id_is_derived_from_the_verifier_as_clients_derive_it() {
        assert_eq!(
            derive_auth_request_id(VERIFIER).as_deref(),
            Some(REQUEST_ID)
        );
        for malformed in [
            "",
            &VERIFIER[1..],
            &format!("{VERIFIER}A"),
            &VERIFIER.replace('A', "+"),
        ] {
            assert_eq!(derive_auth_request_id(malformed), None, "{malformed}");
        }
    }

    #[test]
    fn a_bound_pickup_needs_the_matching_verifier() {
        use Verification::*;
        let other = "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";
        assert_eq!(verify_pickup(None, None), Unbound);
        assert_eq!(verify_pickup(None, Some(other)), Unbound);
        assert_eq!(verify_pickup(Some(REQUEST_ID), Some(VERIFIER)), Verified);
        assert_eq!(verify_pickup(Some(REQUEST_ID), Some(other)), Refused);
        assert_eq!(verify_pickup(Some(REQUEST_ID), Some("short")), Refused);
        assert_eq!(verify_pickup(Some(REQUEST_ID), None), Refused);
    }
}
