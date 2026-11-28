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
use axum::http::header;
use serde::{Deserialize, Serialize};

use crate::SharedState;
use crate::http::ApiError;
use crate::limits;

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

#[derive(Debug, Default, Deserialize)]
pub struct NewsletterBody {
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Serialize)]
struct Subscription<'a> {
    email: &'a str,
    reactivate_existing: bool,
    send_welcome_email: bool,
    utm_source: &'a str,
    utm_medium: &'a str,
}

pub fn is_valid_email(email: &str) -> bool {
    if email.len() > 254 {
        return false;
    }
    if email.chars().any(|c| {
        c.is_whitespace() || c.is_control() || c == ',' || c == ';' || c == '<' || c == '>'
    }) {
        return false;
    }
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    if local.is_empty() || local.len() > 64 || domain.contains('@') {
        return false;
    }
    let labels: Vec<&str> = domain.split('.').collect();
    if labels.len() < 2 {
        return false;
    }
    labels.iter().all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    })
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
        .map_err(|_| ApiError::http(413, "Request body too large"))?;
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

    if let Some(forward) = &state.settings.newsletter {
        let target = format!(
            "{}/publications/{}/subscriptions",
            forward.url.trim_end_matches('/'),
            forward.publication_id
        );
        let answer = state
            .guarded
            .post(&target)
            .header(header::AUTHORIZATION, format!("Bearer {}", forward.api_key))
            .header(header::ACCEPT, "application/json")
            .json(&Subscription {
                email: &email,
                reactivate_existing: true,
                send_welcome_email: false,
                utm_source: &source,
                utm_medium: "organic",
            })
            .send()
            .await;
        let refused = match answer {
            Ok(answer) if answer.status().is_success() => None,
            Ok(answer) => Some(answer.status().to_string()),
            Err(_) => Some("unreachable".to_string()),
        };
        if let Some(why) = refused {
            tracing::warn!(answer = %why, "the newsletter service did not take a subscription");
            return Err(ApiError::http(
                502,
                "The newsletter service did not take the subscription",
            ));
        }
    }

    Ok(Json(NewsletterSubscribeOut { ok: true }))
}

#[cfg(test)]
mod tests {
    use super::{NewsletterSubscribeOut, is_valid_email};
    use serde_json::json;

    #[test]
    fn success_body_is_exactly_ok_true_without_data_field() {
        let v = serde_json::to_value(NewsletterSubscribeOut { ok: true }).unwrap();
        assert_eq!(v, json!({ "ok": true }));
        assert!(v.get("data").is_none());
    }

    #[test]
    fn accepts_normal_addresses() {
        for good in [
            "a@b.co",
            "user@example.com",
            "first.last+tag@sub.example.org",
            "UPPER@EXAMPLE.COM",
            "x_y-z@ex-ample.io",
        ] {
            assert!(is_valid_email(good), "expected valid: {good}");
        }
    }

    #[test]
    fn rejects_malformed_addresses() {
        for bad in [
            "",
            "plainaddress",
            "@example.com",
            "user@",
            "user@localhost",
            "user@@example.com",
            "user@.com",
            "user@example.",
            "user@-example.com",
            "user name@example.com",
            "user@exam ple.com",
            "user@example.com\n",
            "<user@example.com>",
            "a@b,c.com",
        ] {
            assert!(!is_valid_email(bad), "expected invalid: {bad}");
        }
    }

    #[test]
    fn rejects_oversized_addresses() {
        let long_local = format!("{}@example.com", "a".repeat(65));
        assert!(!is_valid_email(&long_local));
        let long_total = format!("a@{}.com", "b".repeat(260));
        assert!(!is_valid_email(&long_total));
    }
}
