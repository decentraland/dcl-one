// from: credits-server@d9c6a261fb7acf3beb926e10f03d1aed33e9771b:src/controllers/handlers/get-authorized-charge.ts, src/controllers/routes.ts (chargeLookupRateLimit), src/logic/rate-limit.ts
//! `GET /credits/authorize/{salt}` (canonical
//! `GET /shop-api/v1/credit-authorizations/{salt}`): what the signer's
//! credit under `salt` debits its balance ([`crate::credits::charge`]).
//! Signed fetch; it answers only for the signer, so another wallet's salt is
//! the same 404 as an unknown one. Each wallet may ask
//! [`LOOKUPS_PER_MINUTE`] times a minute, as credits-server's charge-lookup
//! limiter allows.

use std::sync::LazyLock;
use std::time::Duration;

use axum::Json;
use axum::extract::{OriginalUri, Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::json;

use super::signer_from;
use crate::credits::SharedState;
use crate::credits::errors::ApiError;
use crate::shared::limits::{RateLimitDecision, RateLimiter};

/// Lookups one wallet may make a minute (credits-server's
/// `USD_CHARGE_LOOKUP_RATE_LIMIT` default).
pub const LOOKUPS_PER_MINUTE: u32 = 120;

static LOOKUPS: LazyLock<RateLimiter> =
    LazyLock::new(|| RateLimiter::new(LOOKUPS_PER_MINUTE, Duration::from_secs(60)));

/// The largest integer a JSON number carries exactly.
const MAX_EXACT_JSON_INT: i64 = (1 << 53) - 1;

/// A bytes32 hex string, either case.
fn is_salt(text: &str) -> bool {
    text.strip_prefix("0x")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
}

pub async fn authorized_charge(
    State(state): State<SharedState>,
    Path(salt): Path<String>,
    original: OriginalUri,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let path = format!("/credits/authorize/{salt}");
    let signer = signer_from(&state, &headers, "get", &path, &original).await?;
    if LOOKUPS.check(signer.as_str()) == RateLimitDecision::Deny {
        return Ok(too_many_lookups());
    }
    if !is_salt(&salt) {
        return Err(ApiError::bad_request(
            "\"salt\" must be a 32-byte hex string",
        ));
    }
    let salt = salt.to_ascii_lowercase();
    let charge = state
        .credits
        .authorized_charge(signer.as_str(), &salt)
        .await?
        .ok_or_else(|| ApiError::not_found("No authorization for that credit"))?;
    // A JSON number past 2^53 is rounded on the way out, and a caller comparing
    // it against a price would compare against an approximation.
    if !(0..=MAX_EXACT_JSON_INT).contains(&charge.usd_cents)
        || !(0..=MAX_EXACT_JSON_INT).contains(&charge.lines)
    {
        return Err(ApiError::Internal(
            "the authorized charge cannot be represented exactly".into(),
        ));
    }
    Ok(Json(json!({
        "salt": salt,
        "usdCents": charge.usd_cents,
        "lines": charge.lines,
        "status": charge.status,
    }))
    .into_response())
}

/// credits-server's limiter answer: 429 with `Retry-After` and the seconds in
/// the body. The bucket refills a lookup every half second.
fn too_many_lookups() -> Response {
    (
        StatusCode::TOO_MANY_REQUESTS,
        [(header::RETRY_AFTER, "1")],
        Json(json!({
            "error": "Too many requests. Slow down and try again shortly.",
            "retryAfterSeconds": 1,
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_salt_is_a_bytes32_hex_string() {
        assert!(is_salt(&format!("0x{}", "ab".repeat(32))));
        assert!(is_salt(&format!("0x{}", "AB".repeat(32))));
        for bad in [
            "".to_string(),
            "ab".repeat(32),
            format!("0x{}", "ab".repeat(31)),
            format!("0x{}", "ab".repeat(33)),
            format!("0x{}zz", "ab".repeat(31)),
            "batch".to_string(),
        ] {
            assert!(!is_salt(&bad), "{bad}");
        }
    }
}
