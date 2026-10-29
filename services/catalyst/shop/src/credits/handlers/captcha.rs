//! `GET /captcha` invalidates the wallet's open challenges and inserts one
//! (120 s TTL, the PNG rendered from the stored answer); `POST /captcha`
//! consumes the newest open one and compares within tolerance, then the
//! hCaptcha token when a provider is configured. The GET writes the
//! challenge, so it waits for the credits switch and the restore stamps
//! like the POST (the gate in front covers only the mutating methods).
//! The program grants no
//! Credits for a claim any more (`credits_granted` is 0; the seasons were
//! removed upstream); the route reports whether the wallet is blocked.

use axum::Json;
use axum::extract::{OriginalUri, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use chrono::Duration;
use sqlx::Row;

use super::signer_from;
use crate::credits::SharedState;
use crate::credits::captcha::{CLAIM_TOLERANCE, answer_for_seed, render_png};
use crate::credits::dto::{ClaimCreditsBody, ClaimCreditsResponse};
use crate::credits::errors::ApiError;
use crate::credits::store::now;

const CAPTCHA_TTL_SECS: i64 = 120;

pub async fn generate(
    State(state): State<SharedState>,
    original: OriginalUri,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let signer = signer_from(&state, &headers, "get", "/captcha", &original).await?;
    if let Some(refusal) = crate::shared::writes::ops_refusal(state.settings.writes_enabled) {
        return Ok(crate::credits::writes::refused_response(&refusal));
    }

    let now = now();
    let expires_at = now + Duration::seconds(CAPTCHA_TTL_SECS);
    let seed = (now.and_utc().timestamp_millis() as u64)
        ^ signer
            .as_str()
            .bytes()
            .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
    let answer = answer_for_seed(seed);

    sqlx::query(
        "WITH invalidated AS ( \
             UPDATE credits.captcha_challenges SET consumed_at = $4 \
             WHERE address = $1 AND consumed_at IS NULL \
         ) \
         INSERT INTO credits.captcha_challenges (address, answer_x, expires_at) \
         VALUES ($1, $2, $3)",
    )
    .bind(signer.as_str())
    .bind(answer)
    .bind(expires_at)
    .bind(now)
    .execute(&state.credits.pool)
    .await?;

    let png = render_png(answer);
    Ok((StatusCode::OK, [(header::CONTENT_TYPE, "image/png")], png).into_response())
}

pub fn slider_solved(answer: f64, submitted: f64) -> bool {
    (answer - submitted).abs() <= CLAIM_TOLERANCE
}

fn rejected() -> Response {
    Json(ClaimCreditsResponse {
        ok: false,
        credits_granted: 0.0,
        is_blocked_for_claiming: false,
    })
    .into_response()
}

pub async fn claim(
    State(state): State<SharedState>,
    original: OriginalUri,
    headers: HeaderMap,
    body: Option<Json<ClaimCreditsBody>>,
) -> Result<Response, ApiError> {
    let signer = signer_from(&state, &headers, "post", "/captcha", &original).await?;
    let Json(claim) = body.ok_or_else(|| ApiError::bad_request("missing JSON body { x }"))?;

    let now = now();
    let row = sqlx::query(
        "UPDATE credits.captcha_challenges SET consumed_at = $2 \
         WHERE id = ( \
             SELECT id FROM credits.captcha_challenges \
             WHERE address = $1 AND consumed_at IS NULL AND expires_at > $2 \
             ORDER BY id DESC LIMIT 1 \
         ) \
         AND consumed_at IS NULL \
         RETURNING answer_x::float8 AS answer_x, \
                   (SELECT is_blocked_for_claiming FROM credits.wallets WHERE address = $1) AS blocked",
    )
    .bind(signer.as_str())
    .bind(now)
    .fetch_optional(&state.credits.pool)
    .await?;

    let (answer, blocked) = row
        .map(|r| {
            (
                r.get::<f64, _>("answer_x"),
                r.get::<Option<bool>, _>("blocked").unwrap_or(false),
            )
        })
        .ok_or_else(|| ApiError::bad_request("no active captcha challenge"))?;

    if !slider_solved(answer, claim.x) {
        return Ok(rejected());
    }

    if let Some(provider) = &state.captcha {
        let Some(token) = claim.token.as_deref().filter(|t| !t.is_empty()) else {
            return Ok(rejected());
        };
        if !provider.verify(token, None).await? {
            return Ok(rejected());
        }
    }

    Ok(Json(ClaimCreditsResponse {
        ok: !blocked,
        credits_granted: 0.0,
        is_blocked_for_claiming: blocked,
    })
    .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slider_within_tolerance_passes() {
        assert!(slider_solved(50.0, 50.0));
        assert!(slider_solved(50.0, 50.0 + CLAIM_TOLERANCE));
        assert!(slider_solved(50.0, 50.0 - CLAIM_TOLERANCE));
    }

    #[test]
    fn slider_outside_tolerance_fails() {
        assert!(!slider_solved(50.0, 50.0 + CLAIM_TOLERANCE + 0.1));
        assert!(!slider_solved(50.0, 0.0));
        assert!(!slider_solved(0.0, 100.0));
    }
}
