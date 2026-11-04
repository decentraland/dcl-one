//! Referral progress (upstream social-service-ea `/v1/referral-progress`).
//! The invited user is always the signed-fetch signer: POST records the
//! attribution (first referrer wins), PATCH marks the signer as signed up,
//! GET reports the signer's own stats as a referrer. `rewardImages` stays
//! empty; nothing here grants tiers or uploads reward art. `POST
//! /v1/referral-email` stores the contact email of a referrer at the IRL-swag
//! tier, checked as upstream's `setReferralEmail` checks it.

use axum::Json;
use axum::extract::{Extension, OriginalUri, State};
use axum::http::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};

use crate::Address;

use super::http::CommError;
use super::{SharedState, signed_path};

#[derive(Serialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "communities/")
)]
pub struct ReferralRewardImage {
    pub tier: i32,
    pub url: String,
}

#[derive(Serialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "communities/")
)]
pub struct ReferralProgressStats {
    #[serde(rename = "invitedUsersAccepted")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub invited_users_accepted: i64,
    #[serde(rename = "invitedUsersAcceptedViewed")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub invited_users_accepted_viewed: i64,
    #[serde(rename = "rewardImages")]
    pub reward_images: Vec<ReferralRewardImage>,
}

#[derive(Deserialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "communities/")
)]
pub struct CreateReferralBody {
    pub referrer: String,
}

/// GET reports the stats and marks them viewed; the mark is a social write,
/// skipped while the social switch is off or held (the stats still answer).
pub async fn get_referral_progress(
    State(state): State<SharedState>,
    headers: HeaderMap,
    switch: Option<Extension<crate::comms::writes::Switch>>,
    original: OriginalUri,
) -> Result<Json<ReferralProgressStats>, CommError> {
    let referrer = state
        .require_signer(
            &headers,
            "get",
            &signed_path(&original, "/v1/referral-progress"),
        )
        .await?;
    let accepted: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM social.referral_progress WHERE referrer = $1 AND status = 'tier_granted'")
        .bind(&referrer)
        .fetch_one(&state.pool)
        .await?;
    let viewed: i64 = sqlx::query_scalar(
        "SELECT invites_accepted_viewed FROM social.referral_progress_viewed WHERE referrer = $1",
    )
    .bind(&referrer)
    .fetch_optional(&state.pool)
    .await?
    .unwrap_or(0);
    if crate::comms::writes::side_refusal(switch).is_none() {
        sqlx::query(
            "INSERT INTO social.referral_progress_viewed (referrer, invites_accepted_viewed) \
             VALUES ($1, $2) \
             ON CONFLICT (referrer) DO UPDATE \
             SET invites_accepted_viewed = EXCLUDED.invites_accepted_viewed, updated_at = timezone('UTC', now())",
        )
        .bind(&referrer)
        .bind(accepted)
        .execute(&state.pool)
        .await?;
    }
    Ok(Json(ReferralProgressStats {
        invited_users_accepted: accepted,
        invited_users_accepted_viewed: viewed,
        reward_images: Vec::new(),
    }))
}

/// A same-referrer duplicate is an idempotent no-op (204) so client retries
/// converge; a different referrer is a conflict, attribution is first-wins.
fn resolve_existing(
    existing: &str,
    referrer: &Address,
    invited: &Address,
) -> Result<StatusCode, CommError> {
    if existing == referrer.as_str() {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(CommError::bad_request(format!(
            "Referral progress already exists for the invited user: {invited}"
        )))
    }
}

pub async fn create_referral(
    State(state): State<SharedState>,
    headers: HeaderMap,
    original: OriginalUri,
    Json(body): Json<CreateReferralBody>,
) -> Result<StatusCode, CommError> {
    let invited = state
        .require_signer(
            &headers,
            "post",
            &signed_path(&original, "/v1/referral-progress"),
        )
        .await?;
    let referrer =
        Address::parse(&body.referrer).map_err(|_| CommError::bad_request("Invalid referrer"))?;
    if referrer == invited {
        return Err(CommError::bad_request(format!(
            "User cannot refer themselves: {invited}"
        )));
    }
    let existing: Option<String> =
        sqlx::query_scalar("SELECT referrer FROM social.referral_progress WHERE invited_user = $1")
            .bind(&invited)
            .fetch_optional(&state.pool)
            .await?;
    if let Some(existing) = existing {
        return resolve_existing(&existing, &referrer, &invited);
    }
    let inserted = sqlx::query(
        "INSERT INTO social.referral_progress (invited_user, referrer, status) VALUES ($1, $2, 'pending') \
         ON CONFLICT (invited_user) DO NOTHING",
    )
    .bind(&invited)
    .bind(&referrer)
    .execute(&state.pool)
    .await?
    .rows_affected();
    if inserted == 0 {
        let stored: Option<String> = sqlx::query_scalar(
            "SELECT referrer FROM social.referral_progress WHERE invited_user = $1",
        )
        .bind(&invited)
        .fetch_optional(&state.pool)
        .await?;
        return match stored {
            Some(stored) => resolve_existing(&stored, &referrer, &invited),
            None => Err(CommError::Internal),
        };
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn update_referral_signed_up(
    State(state): State<SharedState>,
    headers: HeaderMap,
    original: OriginalUri,
) -> Result<StatusCode, CommError> {
    let invited = state
        .require_signer(
            &headers,
            "patch",
            &signed_path(&original, "/v1/referral-progress"),
        )
        .await?;
    let status: Option<String> =
        sqlx::query_scalar("SELECT status FROM social.referral_progress WHERE invited_user = $1")
            .bind(&invited)
            .fetch_optional(&state.pool)
            .await?;
    match status.as_deref() {
        None => Err(CommError::not_found(format!(
            "Referral progress not found for user: {invited}"
        ))),
        Some(current) if current != "pending" => Err(CommError::bad_request(format!(
            "Invalid referral status: {current}. Expected: pending"
        ))),
        Some(_) => {
            sqlx::query("UPDATE social.referral_progress SET status = 'signed_up', updated_at = timezone('UTC', now()) WHERE invited_user = $1")
                .bind(&invited)
                .execute(&state.pool)
                .await?;
            Ok(StatusCode::NO_CONTENT)
        }
    }
}

/// Upstream's `TIERS_IRL_SWAG`: the accepted invites that unlock the email.
const TIERS_IRL_SWAG: i64 = 100;

/// `setReferralEmail`'s refusal of characters usable in markup or script
/// (`logic/referral/referral.ts` at `b6b2cef`), case-insensitive.
fn has_dangerous_chars(email: &str) -> bool {
    email.chars().any(|c| "<>\"'`&;(){}[]\\".contains(c))
        || [
            "script",
            "javascript",
            "vbscript",
            "onload",
            "onerror",
            "onclick",
        ]
        .iter()
        .any(|word| email.to_lowercase().contains(word))
}

/// `@dcl/schemas` `Email.validate`: the JSON-schema `email` format as
/// ajv-formats checks it (`local@label(.label)*`, each label alphanumeric
/// with inner hyphens, at most 63 characters).
fn is_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    let local_ok = !local.is_empty()
        && local
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".!#$%&'*+/=?^_`{|}~-".contains(c));
    let label_ok = |label: &str| {
        let bytes = label.as_bytes();
        !bytes.is_empty()
            && bytes.len() <= 63
            && bytes
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || *b == b'-')
            && bytes[0] != b'-'
            && bytes[bytes.len() - 1] != b'-'
    };
    local_ok && domain.split('.').all(label_ok)
}

/// The checks of upstream `setReferralEmail` that need no stored row, in its
/// order; the email lower-cased and trimmed.
fn validated_email(raw: Option<&str>) -> Result<String, CommError> {
    let email = raw.map(str::trim).unwrap_or_default();
    if email.is_empty() {
        return Err(CommError::bad_request("Email is required"));
    }
    let email = email.to_lowercase();
    if email.chars().count() > 254 {
        return Err(CommError::bad_request("Email is too long"));
    }
    if has_dangerous_chars(&email) {
        return Err(CommError::bad_request("Email contains invalid characters"));
    }
    if !is_email(&email) {
        return Err(CommError::bad_request("Invalid email format"));
    }
    Ok(email)
}

/// `POST /v1/referral-email`: a referrer with 100 accepted invites leaves a
/// contact email, at most once per 24 hours. Upstream also mails it to its
/// marketing team; this node keeps the row for its operator.
pub async fn add_referral_email(
    State(state): State<SharedState>,
    headers: HeaderMap,
    original: OriginalUri,
    body: axum::body::Bytes,
) -> Result<StatusCode, CommError> {
    let referrer = state
        .require_signer(
            &headers,
            "post",
            &signed_path(&original, "/v1/referral-email"),
        )
        .await?;
    let accepted: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM social.referral_progress WHERE referrer = $1 AND status = 'tier_granted'")
        .bind(&referrer)
        .fetch_one(&state.pool)
        .await?;
    if accepted < TIERS_IRL_SWAG {
        return Err(CommError::bad_request(format!(
            "You must have at least {TIERS_IRL_SWAG} accepted invites to set an email"
        )));
    }
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
    let email = validated_email(body.get("email").and_then(|v| v.as_str()))?;
    let inserted = sqlx::query(
        "INSERT INTO social.referral_emails (referrer, email) \
         SELECT $1, $2 WHERE NOT EXISTS ( \
           SELECT 1 FROM social.referral_emails \
           WHERE referrer = $1 AND updated_at > timezone('UTC', now()) - interval '24 hours')",
    )
    .bind(&referrer)
    .bind(&email)
    .execute(&state.pool)
    .await?
    .rows_affected();
    if inserted == 0 {
        return Err(CommError::bad_request(format!(
            "Email can only be updated once per day. Last update was less than 24 hours ago for user: {referrer}"
        )));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_same_referrer_repeat_is_idempotent_and_a_different_one_conflicts() {
        let referrer = Address::parse("0x00000000000000000000000000000000000000a1").unwrap();
        let other = Address::parse("0x00000000000000000000000000000000000000a2").unwrap();
        let invited = Address::parse("0x00000000000000000000000000000000000000b1").unwrap();
        assert_eq!(
            resolve_existing(referrer.as_str(), &referrer, &invited).unwrap(),
            StatusCode::NO_CONTENT
        );
        let err = resolve_existing(other.as_str(), &referrer, &invited).unwrap_err();
        assert!(matches!(err, CommError::BadRequest(m) if m.contains("already exists")));
    }

    #[test]
    fn referral_emails_are_checked_in_upstreams_order() {
        let message = |raw: Option<&str>| match validated_email(raw) {
            Ok(email) => email,
            Err(CommError::BadRequest(m)) => m,
            Err(other) => panic!("{other:?}"),
        };
        assert_eq!(message(None), "Email is required");
        assert_eq!(message(Some("   ")), "Email is required");
        assert_eq!(
            message(Some(&format!("{}@example.com", "a".repeat(250)))),
            "Email is too long"
        );
        assert_eq!(
            message(Some("<b>@example.com")),
            "Email contains invalid characters"
        );
        assert_eq!(
            message(Some("javascript@example.com")),
            "Email contains invalid characters"
        );
        assert_eq!(message(Some("not-an-email")), "Invalid email format");
        assert_eq!(message(Some("a@-bad.com")), "Invalid email format");
        assert_eq!(
            message(Some(" Jane.Doe@Example.com ")),
            "jane.doe@example.com"
        );
    }
}
