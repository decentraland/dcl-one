//! The forward of a newsletter subscription to the newsletter service
//! (`NEWSLETTER_SERVICE_URL`, publication `NEWSLETTER_PUBLICATION_ID`, bearer
//! `NEWSLETTER_SERVICE_API_KEY`) through the client that follows no redirect,
//! and the email rule it and the terms acceptances share.

use axum::http::header;
use serde::Serialize;

use crate::State;
use crate::http::ApiError;

#[derive(Serialize)]
struct Subscription<'a> {
    email: &'a str,
    reactivate_existing: bool,
    send_welcome_email: bool,
    utm_source: &'a str,
    utm_medium: &'a str,
}

/// Whether `email` is an address a subscription or a terms acceptance
/// takes: at most 254 characters, one `@`, a dotted domain of plain labels.
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

/// Forwards `email`'s subscription from `source`; 502 when the service
/// doesn't answer 2xx, nothing without the service's settings.
pub async fn forward(state: &State, email: &str, source: &str) -> Result<(), ApiError> {
    let Some(forward) = &state.settings.newsletter else {
        return Ok(());
    };
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
            email,
            reactivate_existing: true,
            send_welcome_email: false,
            utm_source: source,
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_valid_email;

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
