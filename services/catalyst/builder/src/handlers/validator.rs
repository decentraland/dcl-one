//! The wearable validator's routes: `POST /v1/validator/claims`,
//! `POST /v1/validator/claims/:validationId/lease` and builder-server's
//! `POST /v1/collections/:id/validation-result`.
//!
//! Each call passes, in order: the per-source limit (60 a minute across the
//! three, 429), the route's body cap (413), the signature
//! (`shared::validator_signature` over the path the validator requested, the
//! secret from `WEARABLE_VALIDATOR_CALLBACK_SECRET`), the `builder`
//! flag, and the seen-signature record that refuses a replay; every
//! refusal among the last three is the same 401 `Unauthorized`, and none of
//! them reads the database before the signature verifies. Then the body is
//! parsed.

use axum::Json;
use axum::extract::{FromRequestParts, OriginalUri, Path, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use chrono::DateTime;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::SharedState;
use crate::auth::{SignedPath, signed_path};
use crate::http::{ApiData, ApiError, iso};
use crate::shared::limits::RateLimitDecision;
use crate::shared::signed_fetch::{candidate_paths, header_str};
use crate::shared::validator_signature::{
    SIGNATURE_HEADER, TIMESTAMP_HEADER, WINDOW_MS, timestamp_ms, verify,
};
use crate::validator::{self, claims};

/// The claim and lease bodies' cap.
pub const SMALL_BODY: usize = 4 << 10;
/// The verdict body's cap.
pub const VERDICT_BODY: usize = 5 << 20;

/// A validator call within its source's limit: its headers, method and the
/// path it was requested at.
pub struct Called {
    headers: HeaderMap,
    method: String,
    path: SignedPath,
}

impl FromRequestParts<SharedState> for Called {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &SharedState) -> Result<Self, ApiError> {
        if let Some(source) = crate::limits::source(&parts.headers)
            && state.validator_calls.check(&source) == RateLimitDecision::Deny
        {
            return Err(ApiError::http(
                429,
                "Too many requests, try again in a minute",
            ));
        }
        let original = parts
            .extensions
            .get::<OriginalUri>()
            .map_or(&parts.uri, |original| &original.0);
        Ok(Self {
            path: signed_path(original, &parts.uri),
            headers: parts.headers.clone(),
            method: parts.method.as_str().to_string(),
        })
    }
}

fn unauthorized() -> ApiError {
    ApiError::unauthorized("Unauthorized")
}

/// A validator call whose signature verified, fresh and never seen before:
/// made only here, the proof `access::validator_verdict` takes.
#[derive(Debug)]
pub struct Verified(());

async fn authenticate(
    state: &SharedState,
    called: &Called,
    body: &[u8],
) -> Result<Verified, ApiError> {
    let secret = state
        .settings
        .validator_secret
        .as_ref()
        .ok_or_else(unauthorized)?;
    let timestamp = header_str(&called.headers, TIMESTAMP_HEADER).ok_or_else(unauthorized)?;
    let signature = header_str(&called.headers, SIGNATURE_HEADER).ok_or_else(unauthorized)?;
    let mac = verify(
        secret.bytes(),
        timestamp,
        signature,
        &called.method,
        &candidate_paths(&called.headers, &called.path),
        body,
        (state.now)().timestamp_millis(),
    )
    .ok_or_else(unauthorized)?;
    let mut conn = state.items.pool().acquire().await?;
    if !validator::enabled(&mut conn, state).await? {
        return Err(unauthorized());
    }
    let expires = timestamp_ms(timestamp)
        .and_then(|sent| DateTime::from_timestamp_millis(sent + WINDOW_MS))
        .ok_or_else(unauthorized)?;
    let fresh = sqlx::query(
        "INSERT INTO builder.validator_seen_signatures (signature, expires_at) VALUES ($1, $2) \
         ON CONFLICT (signature) DO NOTHING",
    )
    .bind(&mac)
    .bind(expires.naive_utc())
    .execute(&mut *conn)
    .await?
    .rows_affected()
        == 1;
    if !fresh {
        tracing::info!(path = %called.path.request, "builder validation: a signature seen before is refused");
        return Err(unauthorized());
    }
    Ok(Verified(()))
}

fn invalid_body() -> ApiError {
    ApiError::bad_request("Invalid request body")
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClaimBody {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct LeaseBody {
    claim_id: Uuid,
}

/// `POST /v1/validator/claims` (body `{}`): the oldest claimable request's
/// manifest, or 204 when there is none.
pub async fn claim(
    State(state): State<SharedState>,
    called: Called,
    body: Bytes,
) -> Result<Response, ApiError> {
    authenticate(&state, &called, &body).await?;
    serde_json::from_slice::<ClaimBody>(&body).map_err(|_| invalid_body())?;
    Ok(match claims::claim(&state).await? {
        Some(claimed) => Json(ApiData::ok(claimed)).into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    })
}

/// `POST /v1/validator/claims/:validationId/lease` (body `{"claimId"}`):
/// the lease's new end, or 409 when the claim is not current.
pub async fn lease(
    State(state): State<SharedState>,
    called: Called,
    Path(validation): Path<String>,
    body: Bytes,
) -> Result<Json<ApiData<serde_json::Value>>, ApiError> {
    authenticate(&state, &called, &body).await?;
    let LeaseBody { claim_id } = serde_json::from_slice(&body).map_err(|_| invalid_body())?;
    let until = match Uuid::parse_str(&validation) {
        Ok(validation) => claims::extend(&state, validation, claim_id).await?,
        Err(_) => None,
    };
    match until {
        Some(until) => Ok(Json(ApiData::ok(json!({ "leaseUntil": iso(&until) })))),
        None => Err(ApiError::conflict("The claim is not current")),
    }
}

/// `POST /v1/collections/:id/validation-result`: 204 once the verdict is
/// taken, whether or not its claim was current.
pub async fn verdict(
    State(state): State<SharedState>,
    called: Called,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<StatusCode, ApiError> {
    let verified = authenticate(&state, &called, &body).await?;
    claims::settle(&state, &id, claims::Verdict::parse(&body)?, &verified).await?;
    Ok(StatusCode::NO_CONTENT)
}
