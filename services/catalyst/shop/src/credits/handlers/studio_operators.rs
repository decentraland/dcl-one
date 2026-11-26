// from: credits-server@d9c6a261fb7acf3beb926e10f03d1aed33e9771b:src/controllers/handlers/studio-operators.ts, src/controllers/routes.ts (the operator routes and studioGrantRateLimit)
//! What a studio's operators use: the studios their wallet acts for, each
//! one's budget and grants, and granting to players out of that budget
//! (`studio_grants.rs`). The Shop's studio page calls these.
//!
//! Signed fetch only, with no admin gate: who may act is the studio's own
//! operator list (`credits.studio_operators`), checked on every request. A
//! wallet that does not act for a studio is answered as if the studio did not
//! exist, so it cannot learn which studios exist. Operators see what their
//! studio was given and gave, never who changed its budget.

use std::sync::LazyLock;
use std::time::Duration;

use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{OriginalUri, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::{Value as JsonValue, json};

use super::signer_from;
use super::studios::{PageQuery, json_object};
use crate::credits::SharedState;
use crate::credits::studio_grants::{
    OperatorStudio, StudioGrantRequest, parse_annotation, parse_grant_credits, parse_reason,
};
use crate::credits::studios::{
    StudioError, USD_CENTS_PER_CREDIT, parse_idempotency_key, parse_studio_id,
};
use crate::shared::limits::{RateLimitDecision, RateLimiter};
use crate::shared::routing::{get, post};

/// Grants one operator wallet may ask for in a window (credits-server's
/// `STUDIO_GRANT_RATE_LIMIT` and `STUDIO_GRANT_RATE_WINDOW_SECONDS` defaults).
/// Each grant notifies its player, so this paces a flood of small grants; the
/// budget is what bounds the money.
pub const GRANTS_PER_WINDOW: u32 = 60;
pub const GRANT_WINDOW: Duration = Duration::from_secs(60);

static GRANTS: LazyLock<RateLimiter> =
    LazyLock::new(|| RateLimiter::new(GRANTS_PER_WINDOW, GRANT_WINDOW));

/// The same paths in the credits router and the canonical `/shop-api` one.
pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/operator/studios", get(list_operator_studios))
        .route("/operator/studios/{studio_id}", get(get_operator_studio))
        .route(
            "/operator/studios/{studio_id}/grants",
            post(grant_studio_credits),
        )
}

/// The signing wallet, lowercase, of a request to `route`.
async fn operator_of(
    state: &SharedState,
    headers: &HeaderMap,
    method: &str,
    route: &str,
    original: &OriginalUri,
) -> Result<String, StudioError> {
    signer_from(state, headers, method, route, original)
        .await
        .map(|signer| signer.as_str().to_string())
        .map_err(StudioError::Refused)
}

/// The per-grant ceiling the routes report; `null` while none is configured.
fn grant_limits(state: &SharedState) -> JsonValue {
    json!({ "maxGrantCents": state.settings.studio_grant_max_cents })
}

/// `GET /operator/studios`: the studios the signing wallet acts for (empty
/// when none) and the per-grant ceiling.
async fn list_operator_studios(
    State(state): State<SharedState>,
    original: OriginalUri,
    headers: HeaderMap,
) -> Result<Json<JsonValue>, StudioError> {
    let operator = operator_of(&state, &headers, "get", "/operator/studios", &original).await?;
    let studios: Vec<OperatorStudio> = state
        .credits
        .operator_studios(&operator)
        .await?
        .into_iter()
        .map(OperatorStudio::from)
        .collect();
    Ok(Json(
        json!({ "studios": studios, "grantLimits": grant_limits(&state) }),
    ))
}

/// `GET /operator/studios/{studioId}`: one of the signing wallet's studios,
/// a page of its grants (newest first) and the per-grant ceiling. Query:
/// `limit` (default 50, at most 200) and `offset`.
async fn get_operator_studio(
    State(state): State<SharedState>,
    Path(studio_id): Path<String>,
    Query(page): Query<PageQuery>,
    original: OriginalUri,
    headers: HeaderMap,
) -> Result<Json<JsonValue>, StudioError> {
    let route = format!("/operator/studios/{studio_id}");
    let operator = operator_of(&state, &headers, "get", &route, &original).await?;
    let studio_id = parse_studio_id(&studio_id)?;
    let (limit, offset) = page.page();
    let (studio, grants, total) = state
        .credits
        .operator_studio(&studio_id, &operator, limit, offset)
        .await?
        .ok_or(StudioError::NotFound)?;
    Ok(Json(json!({
        "studio": OperatorStudio::from(studio),
        "grants": grants,
        "page": { "limit": limit, "offset": offset, "total": total },
        "grantLimits": grant_limits(&state),
    })))
}

/// credits-server's answer past the rate limit.
fn too_many_grants() -> Response {
    let wait = GRANT_WINDOW.as_secs();
    (
        StatusCode::TOO_MANY_REQUESTS,
        [(header::RETRY_AFTER, wait.to_string())],
        Json(json!({
            "error": "Too many requests. Slow down and try again shortly.",
            "retryAfterSeconds": wait,
        })),
    )
        .into_response()
}

/// `POST /operator/studios/{studioId}/grants`: an operator gives a player
/// Credits out of the studio's budget. Body: `{ address, credits, reason,
/// annotation?, idempotencyKey }`; the signing operator is the granter.
///
/// 201 with the new credit, or 200 with `replayed: true` and the credit the
/// same key already granted.
async fn grant_studio_credits(
    State(state): State<SharedState>,
    Path(studio_id): Path<String>,
    original: OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, StudioError> {
    let route = format!("/operator/studios/{studio_id}/grants");
    let operator = operator_of(&state, &headers, "post", &route, &original).await?;
    if GRANTS.check(&operator) == RateLimitDecision::Deny {
        return Ok(too_many_grants());
    }
    let studio_id = parse_studio_id(&studio_id)?;
    let body = json_object(&body)?;
    let recipient = body
        .get("address")
        .and_then(JsonValue::as_str)
        .map(str::to_ascii_lowercase)
        .filter(|a| crate::types::is_hex_address(a))
        .ok_or_else(|| StudioError::Invalid("\"address\" must be a wallet address".into()))?;
    let credits = parse_grant_credits(body.get("credits"))?;
    let reason = parse_reason(body.get("reason"))?;
    let annotation = parse_annotation(body.get("annotation"))?;
    let idempotency_key = parse_idempotency_key(body.get("idempotencyKey"))?;
    let usd_cents = credits * USD_CENTS_PER_CREDIT;
    let req = StudioGrantRequest {
        studio_id: &studio_id,
        recipient: &recipient,
        granter: &operator,
        usd_cents,
        reason: &reason,
        annotation: annotation.as_deref(),
        idempotency_key: &idempotency_key,
    };

    // Who may act comes first: nothing about a studio, not even a past grant,
    // is told to a wallet that does not act for it.
    if !state
        .credits
        .is_studio_operator(&studio_id, &operator)
        .await?
    {
        return Err(StudioError::NotFound);
    }
    // A grant already made is answered next: nothing below can undo it.
    let grant = match state.credits.studio_grant_replay(req).await? {
        Some(replayed) => replayed,
        None => {
            let max_cents = state
                .settings
                .studio_grant_max_cents
                .ok_or(StudioError::GrantsNotConfigured)?;
            if usd_cents > max_cents {
                return Err(StudioError::GrantLimitExceeded {
                    max_cents,
                    requested_cents: usd_cents,
                });
            }
            state.credits.grant_from_studio(req).await?
        }
    };
    tracing::info!(
        studio_id = %studio_id,
        studio_name = %grant.studio_name,
        recipient = %recipient,
        operator = %operator,
        usd_cents,
        balance_cents = grant.balance_cents,
        credit_id = %grant.credit_id,
        replayed = grant.replayed,
        "studio grant"
    );
    let status = if grant.replayed {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    Ok((
        status,
        Json(json!({
            "creditId": grant.credit_id,
            "studioId": studio_id,
            "address": recipient,
            "denomination": "USD",
            "credits": credits,
            "usdCents": usd_cents,
            "balanceCents": grant.balance_cents,
            "replayed": grant.replayed,
        })),
    )
        .into_response())
}
