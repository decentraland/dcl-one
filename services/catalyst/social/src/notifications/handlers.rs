//! The routes. Every wallet-scoped route verifies the signed-fetch chain
//! over the path the client requested (`OriginalUri`, so the router works
//! nested under `/notifications` or at a vhost root; a proxy that rewrites
//! the path forwards the public one as `x-original-path`, and a public path
//! that ends with the route, `/notifications` on notifications.<domain>,
//! verifies as the route; `SignedPath::from_uris`); the broadcast is
//! the bearer-gated admin route.

use std::sync::Arc;

use axum::Json;
use axum::extract::{OriginalUri, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::IntoResponse;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use super::admin::RequireAdmin;
use super::email::{self, CODE_LEN};
use super::errors::{ApiError, NotificationsError};
use super::store::SetEmailOutcome;
use super::types::{
    BroadcastResponse, CommunityOptOutStatus, MarkReadResponse, NotificationsListResponse,
    OptOutResponse, Subscription, is_known_type, normalize_details, validate_subscription_details,
};
use crate::identity::Address;
use crate::shared::signed_fetch;

type AppState = Arc<super::State>;

const DEFAULT_LIMIT: i64 = 50;
const MAX_LIMIT: i64 = 100;
const MAX_ADDRESSES: usize = 10_000;
const SCOPE_COMMUNITY: &str = "community";

/// The verified signer, or the signed-fetch refusal as the envelope (400
/// malformed, 401 unauthenticated, 503 when a contract wallet cannot be
/// asked; the old Rust port collapsed all three to 401). The legacy payload
/// stays accepted behind the canonical-`signer` guard, as the old crate's
/// `CANONICAL_METADATA_KEYS = ["signer"]` declared.
async fn signer(
    state: &AppState,
    headers: &HeaderMap,
    method: &str,
    uri: &OriginalUri,
    local: &Uri,
) -> Result<Address, NotificationsError> {
    signed_fetch::require_signer_at(
        headers,
        method,
        &signed_fetch::SignedPath::from_uris(uri, local),
        signed_fetch::PayloadPolicy::V6ThenLegacy,
        state.eip1654.as_deref(),
    )
    .await
    .map(|s| Address::from_signer(&s))
    .map_err(|e| {
        tracing::warn!(error = %e, "signed-fetch rejected");
        let (status, error) = e.http_status_and_message();
        NotificationsError::SignedFetch { status, error }
    })
}

#[derive(Debug, Default, Deserialize)]
pub struct ListQuery {
    pub limit: Option<i64>,
    pub from: Option<i64>,
    #[serde(rename = "onlyUnread")]
    pub only_unread: Option<bool>,
}

pub async fn get_notifications(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: OriginalUri,
    local: Uri,
    Query(q): Query<ListQuery>,
) -> Result<Json<NotificationsListResponse>, NotificationsError> {
    let signer = signer(&state, &headers, "get", &uri, &local).await?;
    let limit = match q.limit {
        Some(n) if n > 0 && n <= MAX_LIMIT => n,
        Some(n) if n > MAX_LIMIT => MAX_LIMIT,
        _ => DEFAULT_LIMIT,
    };
    let only_unread = q.only_unread.unwrap_or(false);
    let items = state
        .store
        .list_and_touch(&signer, limit, q.from, only_unread)
        .await?;
    Ok(Json(NotificationsListResponse {
        notifications: items,
    }))
}

#[derive(Debug, Deserialize)]
pub struct ReadBody {
    #[serde(rename = "notificationIds")]
    pub notification_ids: Vec<String>,
}

pub async fn put_read(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: OriginalUri,
    local: Uri,
    Json(body): Json<ReadBody>,
) -> Result<impl IntoResponse, NotificationsError> {
    let signer = signer(&state, &headers, "put", &uri, &local).await?;
    let ids: Vec<Uuid> = body
        .notification_ids
        .iter()
        .map(|s| Uuid::parse_str(s))
        .collect::<Result<_, _>>()
        .map_err(|_| ApiError::bad_request("invalid notification id"))?;
    let updated = state.store.mark_read(&signer, &ids).await?;
    Ok(Json(MarkReadResponse { updated }))
}

pub async fn get_subscription(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: OriginalUri,
    local: Uri,
) -> Result<Json<Subscription>, NotificationsError> {
    let signer = signer(&state, &headers, "get", &uri, &local).await?;
    let sub = match state.store.get_subscription(&signer).await? {
        Some(sub) => sub,
        None => Subscription {
            address: signer.into_string(),
            email: None,
            unconfirmed_email: None,
            details: normalize_details(&serde_json::json!({})),
        },
    };
    Ok(Json(sub))
}

pub async fn put_subscription(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: OriginalUri,
    local: Uri,
    Json(details): Json<JsonValue>,
) -> Result<Json<Subscription>, NotificationsError> {
    let signer = signer(&state, &headers, "put", &uri, &local).await?;
    validate_subscription_details(&details).map_err(ApiError::bad_request)?;
    let sub = state
        .store
        .put_subscription_details(&signer, &details)
        .await?;
    Ok(Json(sub))
}

#[derive(Debug, Deserialize)]
pub struct SetEmailBody {
    pub email: String,
    #[serde(rename = "isCreditsWorkflow", default)]
    pub is_credits_workflow: bool,
}

pub async fn put_set_email(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: OriginalUri,
    local: Uri,
    Json(body): Json<SetEmailBody>,
) -> Result<impl IntoResponse, NotificationsError> {
    let signer = signer(&state, &headers, "put", &uri, &local).await?;
    let email = body.email.trim();
    if !email.is_empty() && (!email.contains('@') || email.starts_with('@') || email.ends_with('@'))
    {
        return Err(ApiError::bad_request("Invalid email").into());
    }
    if !email.is_empty() && state.email.is_domain_blacklisted(email) {
        return Err(ApiError::bad_request("Email domain not allowed").into());
    }
    match state
        .store
        .set_email(&signer, email, body.is_credits_workflow)
        .await?
    {
        SetEmailOutcome::NoEmailSent => {}
        SetEmailOutcome::SendConfirmation { source, code } => {
            state
                .email
                .send_confirmation(source, email, signer.as_str(), &code)
                .await?;
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct ConfirmEmailBody {
    #[serde(default)]
    pub address: Option<String>,
    pub code: String,
    #[serde(rename = "turnstileToken", default)]
    pub turnstile_token: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

/// Unsigned by design (the link is opened from a mail client); the code and
/// the optional Turnstile challenge are the credential.
pub async fn put_confirm_email(
    State(state): State<AppState>,
    Json(body): Json<ConfirmEmailBody>,
) -> Result<impl IntoResponse, NotificationsError> {
    let address = body
        .address
        .as_deref()
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .ok_or_else(|| ApiError::bad_request("Missing address"))?;
    let address = Address::parse(address).map_err(|_| ApiError::bad_request("Invalid address"))?;
    let code = body.code.trim();
    if code.len() != CODE_LEN {
        return Err(ApiError::bad_request("Invalid confirmation code").into());
    }
    let verified = email::verify_turnstile(
        &state.http,
        state.email.turnstile_secret(),
        body.turnstile_token.as_deref(),
    )
    .await?;
    if !verified {
        return Err(ApiError::unauthorized("Turnstile verification failed").into());
    }
    match state.store.confirm_email(&address, code).await? {
        Some(_) => Ok(StatusCode::NO_CONTENT),
        None => Err(ApiError::bad_request("Invalid confirmation code").into()),
    }
}

pub async fn get_community_opt_out(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: OriginalUri,
    local: Uri,
    Path(community_id): Path<String>,
) -> Result<Json<CommunityOptOutStatus>, NotificationsError> {
    let signer = signer(&state, &headers, "get", &uri, &local).await?;
    let opted_out = state
        .store
        .is_opted_out(&signer, SCOPE_COMMUNITY, &community_id)
        .await?;
    Ok(Json(CommunityOptOutStatus {
        scope: SCOPE_COMMUNITY.to_string(),
        scope_id: community_id,
        opted_out,
    }))
}

pub async fn delete_community_opt_out(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: OriginalUri,
    local: Uri,
    Path(community_id): Path<String>,
) -> Result<impl IntoResponse, NotificationsError> {
    let signer = signer(&state, &headers, "delete", &uri, &local).await?;
    state
        .store
        .delete_opt_out(&signer, SCOPE_COMMUNITY, &community_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct OptOutBody {
    pub scope: String,
    #[serde(rename = "scopeId")]
    pub scope_id: String,
}

pub async fn post_opt_out(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: OriginalUri,
    local: Uri,
    Json(body): Json<OptOutBody>,
) -> Result<impl IntoResponse, NotificationsError> {
    let signer = signer(&state, &headers, "post", &uri, &local).await?;
    if body.scope != SCOPE_COMMUNITY {
        return Err(ApiError::bad_request("unsupported opt-out scope").into());
    }
    if body.scope_id.trim().is_empty() {
        return Err(ApiError::bad_request("missing scopeId").into());
    }
    state
        .store
        .create_opt_out(&signer, &body.scope, &body.scope_id)
        .await?;
    Ok((StatusCode::CREATED, Json(OptOutResponse { ok: true })))
}

const MAX_BROADCAST_ID_CHARS: usize = 128;

#[derive(Debug, Deserialize)]
pub struct BroadcastBody {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub metadata: JsonValue,
    #[serde(default)]
    pub addresses: Option<Vec<String>>,
    /// The idempotency key of this broadcast (`notifications_broadcast_uidx`):
    /// a retried POST with the same id reaches no recipient twice. Absent, the
    /// server mints a uuid per request, as the old admin route did.
    #[serde(rename = "broadcastId", default)]
    pub broadcast_id: Option<String>,
}

pub async fn post_broadcast(
    admin: RequireAdmin,
    State(state): State<AppState>,
    Json(body): Json<BroadcastBody>,
) -> Result<impl IntoResponse, NotificationsError> {
    if !is_known_type(&body.kind) {
        return Err(
            ApiError::bad_request(format!("unknown notification type: {}", body.kind)).into(),
        );
    }
    if !body.metadata.is_null() && !body.metadata.is_object() {
        return Err(ApiError::bad_request("metadata must be an object").into());
    }
    let metadata = if body.metadata.is_null() {
        serde_json::json!({})
    } else {
        body.metadata
    };
    let addresses: Option<Vec<Address>> = match body.addresses {
        Some(list) => {
            if list.is_empty() {
                return Err(ApiError::bad_request(
                    "addresses must be non-empty when provided (omit it to broadcast to all)",
                )
                .into());
            }
            if list.len() > MAX_ADDRESSES {
                return Err(ApiError::bad_request(format!(
                    "too many addresses: {} (max {})",
                    list.len(),
                    MAX_ADDRESSES
                ))
                .into());
            }
            let mut parsed = Vec::with_capacity(list.len());
            for a in &list {
                parsed.push(
                    Address::parse(a)
                        .map_err(|_| ApiError::bad_request(format!("invalid address: {}", a)))?,
                );
            }
            Some(parsed)
        }
        None => None,
    };
    let broadcast_id = match body.broadcast_id.as_deref().map(str::trim) {
        None => Uuid::new_v4().to_string(),
        Some(id) if !id.is_empty() && id.chars().count() <= MAX_BROADCAST_ID_CHARS => {
            id.to_string()
        }
        Some(_) => {
            return Err(ApiError::bad_request(format!(
                "broadcastId must be 1 to {MAX_BROADCAST_ID_CHARS} characters"
            ))
            .into());
        }
    };
    let inserted = state
        .store
        .broadcast(&broadcast_id, &body.kind, &metadata, addresses.as_deref())
        .await?;
    tracing::info!(actor = %admin.audit_actor_description(), kind = %body.kind, recipients = inserted, "broadcast");
    Ok((
        StatusCode::OK,
        Json(BroadcastResponse {
            ok: true,
            broadcast_id,
            kind: body.kind,
            recipients: inserted,
        }),
    ))
}
