//! `POST /v1/newsletter`: builder-server's newsletter opt-in. Anyone may
//! subscribe an email, as there; what the node adds: the source address's
//! budget is spent before the body is read, the email is checked and
//! lowercased, each email and the node have a budget too ([`limits`]), and
//! the forward to the newsletter service goes through the client that
//! follows no redirect, so its key never reaches another host. The node
//! keeps no subscription: the email is never stored, logged nor written
//! into an answer.

use axum::Json;
use axum::extract::{Request, State};
use serde::{Deserialize, Serialize};

use crate::SharedState;
use crate::http::ApiError;
use crate::limits;
use crate::newsletter::is_valid_email;

/// The largest body the route reads.
pub const MAX_BODY: usize = 4 * 1024;
/// The longest `source` the route takes.
pub const MAX_SOURCE: usize = 100;

/// The bare `{ok:true}` success body (no `data`, as upstream answers).
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "builder/"))]
pub struct NewsletterSubscribeOut {
    pub ok: bool,
}

/// The subscription body: an email and where it came from; other keys are
/// ignored.
#[derive(Debug, Default, Deserialize)]
pub struct NewsletterBody {
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

/// Subscribes the body's email: 400 for a missing or malformed email or
/// source, 413 for a body over [`MAX_BODY`], 429 over a budget, 502 when the
/// newsletter service doesn't answer 2xx; `{ok:true}` otherwise, also
/// without the service's settings, as builder-server answers then.
pub async fn post_newsletter(
    State(state): State<SharedState>,
    request: Request,
) -> Result<Json<NewsletterSubscribeOut>, ApiError> {
    if let Some(source) = limits::source(request.headers()) {
        limits::spend_newsletter_source(&state, &source)?;
    }
    let body = axum::body::to_bytes(request.into_body(), MAX_BODY)
        .await
        .map_err(|_| ApiError::too_large("Request body too large"))?;
    let body: NewsletterBody = if body.is_empty() {
        NewsletterBody::default()
    } else {
        serde_json::from_slice(&body).map_err(|_| ApiError::bad_request("Invalid request body"))?
    };
    let email = body.email.unwrap_or_default().trim().to_ascii_lowercase();
    if email.is_empty() {
        return Err(ApiError::bad_request("email is required"));
    }
    if !is_valid_email(&email) {
        return Err(ApiError::bad_request("invalid email address"));
    }
    let source = body.source.unwrap_or_else(|| "Builder".to_string());
    if source.chars().count() > MAX_SOURCE || source.chars().any(char::is_control) {
        return Err(ApiError::bad_request("invalid source"));
    }
    limits::spend_newsletter_email(&state, &email)?;

    crate::newsletter::forward(&state, &email, &source).await?;

    Ok(Json(NewsletterSubscribeOut { ok: true }))
}

#[cfg(test)]
mod tests {
    use super::NewsletterSubscribeOut;
    use serde_json::json;

    #[test]
    fn success_body_is_exactly_ok_true_without_data_field() {
        let v = serde_json::to_value(NewsletterSubscribeOut { ok: true }).unwrap();
        assert_eq!(v, json!({ "ok": true }));
        assert!(v.get("data").is_none());
    }
}
