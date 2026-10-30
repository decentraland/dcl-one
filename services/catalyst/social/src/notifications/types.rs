//! The wire shapes of notifications.decentraland.org and the
//! `@dcl/schemas` SubscriptionDetails rules (`ts-bindings` were generated
//! from these; the field names are the contract).

use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use uuid::Uuid;

#[derive(Debug, Serialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "notifications/")
)]
pub struct NotificationItem {
    #[serde(with = "uuid_str")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub id: Uuid,
    #[serde(rename = "type")]
    pub kind: String,
    pub address: String,
    /// Epoch ms, as a string (the explorer parses it with `Number`).
    #[serde(with = "i64_str")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub timestamp: i64,
    pub read: bool,
    pub created_at: String,
    pub updated_at: String,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
    pub metadata: Json,
}

mod uuid_str {
    use uuid::Uuid;

    pub(super) fn serialize<S>(value: &Uuid, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }
}

mod i64_str {
    pub(super) fn serialize<S>(value: &i64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "notifications/")
)]
pub struct NotificationsListResponse {
    pub notifications: Vec<NotificationItem>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "notifications/")
)]
pub struct MarkReadResponse {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub updated: u64,
}

#[derive(Debug, Serialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "notifications/")
)]
pub struct OptOutResponse {
    pub ok: bool,
}

#[derive(Debug, Serialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "notifications/")
)]
pub struct CommunityOptOutStatus {
    pub scope: String,
    #[serde(rename = "scopeId")]
    pub scope_id: String,
    #[serde(rename = "optedOut")]
    pub opted_out: bool,
}

#[derive(Debug, Serialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "notifications/")
)]
pub struct BroadcastResponse {
    pub ok: bool,
    #[serde(rename = "broadcastId")]
    pub broadcast_id: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub recipients: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SubscriptionDetails {
    #[serde(default)]
    pub ignore_all_email: bool,
    #[serde(default)]
    pub ignore_all_in_app: bool,
    #[serde(default)]
    pub message_type: Json,
}

pub const NOTIFICATION_TYPES: &[&str] = &[
    "land_transfer_received",
    "badge_granted",
    "bid_accepted",
    "bid_received",
    "events_started",
    "friend_first_wear",
    "events_starts_soon",
    "event_created",
    "event_approved",
    "event_rejected",
    "event_deleted",
    "governance_announcement",
    "governance_authored_proposal_finished",
    "governance_coauthor_requested",
    "governance_cliff_ended",
    "governance_new_comment_on_project_update",
    "governance_new_comment_on_proposal",
    "governance_proposal_enacted",
    "governance_voting_ended_voter",
    "governance_pitch_passed",
    "governance_tender_passed",
    "governance_whale_vote",
    "governance_voted_on_behalf",
    "item_sold",
    "item_published",
    "rental_ended",
    "rental_started",
    "reward_assignment",
    "reward_campaign_out_of_funds",
    "reward_campaign_gas_price_higher_than_expected",
    "reward_campaign_out_of_stock",
    "reward_delayed",
    "reward_in_progress",
    "royalties_earned",
    "social_service_friendship_request",
    "social_service_friendship_accepted",
    "worlds_access_restored",
    "worlds_access_restricted",
    "worlds_missing_resources",
    "worlds_permission_granted",
    "worlds_permission_revoked",
    "credits_goal_completed",
    "credits_on_demand_granted",
    "streaming_key_reset",
    "streaming_key_revoke",
    "streaming_key_expired",
    "streaming_time_exceeded",
    "streaming_place_updated",
    "credits_reminder_complete_goals",
    "credits_reminder_claim_credits",
    "credits_reminder_usage",
    "credits_reminder_usage_24_hours",
    "credits_reminder_do_not_miss_out",
    "credits_new_season_reminder",
    "referral_invited_users_accepted",
    "referral_new_tier_reached",
    "community_deleted",
    "community_deleted_content_violation",
    "community_renamed",
    "community_member_banned",
    "community_member_removed",
    "community_request_to_join_received",
    "community_request_to_join_accepted",
    "community_invite_received",
    "community_ownership_transferred",
    "community_post_added",
    "community_voice_chat_started",
    "user_banned_from_scene",
    "user_unbanned_from_scene",
    "transfer_received",
    "tip_received",
    "banned",
    "ban_warning",
    "ban_lifted",
];

pub fn is_known_type(kind: &str) -> bool {
    NOTIFICATION_TYPES.contains(&kind)
}

pub fn validate_subscription_details(body: &Json) -> Result<(), String> {
    let obj = body
        .as_object()
        .ok_or_else(|| "must be an object".to_string())?;

    for key in obj.keys() {
        if key != "ignore_all_email" && key != "ignore_all_in_app" && key != "message_type" {
            return Err(format!("unexpected property {}", key));
        }
    }

    for field in ["ignore_all_email", "ignore_all_in_app"] {
        match obj.get(field) {
            Some(Json::Bool(_)) => {}
            Some(_) => return Err(format!("{} must be a boolean", field)),
            None => return Err(format!("{} is required", field)),
        }
    }

    let mt = obj
        .get("message_type")
        .ok_or_else(|| "message_type is required".to_string())?
        .as_object()
        .ok_or_else(|| "message_type must be an object".to_string())?;

    for key in mt.keys() {
        if !NOTIFICATION_TYPES.contains(&key.as_str()) {
            return Err(format!("unexpected message_type property {}", key));
        }
    }

    for ty in NOTIFICATION_TYPES {
        let Some(entry) = mt.get(*ty) else {
            if *ty == "friend_first_wear" || *ty == "land_transfer_received" {
                continue;
            }
            return Err(format!("message_type.{} is required", ty));
        };
        let channel = entry
            .as_object()
            .ok_or_else(|| format!("message_type.{} must be an object", ty))?;
        for chan_field in ["email", "in_app"] {
            match channel.get(chan_field) {
                Some(Json::Bool(_)) => {}
                Some(_) => {
                    return Err(format!(
                        "message_type.{}.{} must be a boolean",
                        ty, chan_field
                    ));
                }
                None => return Err(format!("message_type.{}.{} is required", ty, chan_field)),
            }
        }
    }

    Ok(())
}

pub fn default_message_type() -> Json {
    let mut map = serde_json::Map::with_capacity(NOTIFICATION_TYPES.len());
    for ty in NOTIFICATION_TYPES {
        map.insert(
            (*ty).to_string(),
            serde_json::json!({ "email": true, "in_app": true }),
        );
    }
    Json::Object(map)
}

/// The stored document completed to the full shape: every known type
/// present, absent flags read as true, absent ignore_* as false.
pub fn normalize_details(stored: &Json) -> Json {
    let obj = stored.as_object();
    let ignore_all_email = obj
        .and_then(|m| m.get("ignore_all_email"))
        .and_then(Json::as_bool)
        .unwrap_or(false);
    let ignore_all_in_app = obj
        .and_then(|m| m.get("ignore_all_in_app"))
        .and_then(Json::as_bool)
        .unwrap_or(false);
    let stored_mt = obj
        .and_then(|m| m.get("message_type"))
        .and_then(Json::as_object);

    let mut message_type = serde_json::Map::with_capacity(NOTIFICATION_TYPES.len());
    for ty in NOTIFICATION_TYPES {
        let entry = stored_mt.and_then(|m| m.get(*ty)).and_then(Json::as_object);
        let email = entry
            .and_then(|e| e.get("email"))
            .and_then(Json::as_bool)
            .unwrap_or(true);
        let in_app = entry
            .and_then(|e| e.get("in_app"))
            .and_then(Json::as_bool)
            .unwrap_or(true);
        message_type.insert(
            (*ty).to_string(),
            serde_json::json!({ "email": email, "in_app": in_app }),
        );
    }

    serde_json::json!({
        "ignore_all_email": ignore_all_email,
        "ignore_all_in_app": ignore_all_in_app,
        "message_type": Json::Object(message_type),
    })
}

impl Default for SubscriptionDetails {
    fn default() -> Self {
        Self {
            ignore_all_email: false,
            ignore_all_in_app: false,
            message_type: default_message_type(),
        }
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "notifications/", rename_all = "camelCase")
)]
pub struct Subscription {
    pub address: String,
    pub email: Option<String>,
    #[serde(rename = "unconfirmedEmail")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub unconfirmed_email: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
    pub details: Json,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn full_details() -> Json {
        json!({ "ignore_all_email": false, "ignore_all_in_app": true, "message_type": default_message_type() })
    }

    #[test]
    fn details_validation_requires_every_type_but_the_two_optional_ones() {
        assert!(validate_subscription_details(&full_details()).is_ok());
        let mut partial = full_details();
        partial["message_type"]
            .as_object_mut()
            .unwrap()
            .remove("friend_first_wear");
        partial["message_type"]
            .as_object_mut()
            .unwrap()
            .remove("land_transfer_received");
        assert!(validate_subscription_details(&partial).is_ok());
        partial["message_type"]
            .as_object_mut()
            .unwrap()
            .remove("bid_received");
        assert_eq!(
            validate_subscription_details(&partial).unwrap_err(),
            "message_type.bid_received is required"
        );

        let mut extra = full_details();
        extra["extra"] = json!(1);
        assert_eq!(
            validate_subscription_details(&extra).unwrap_err(),
            "unexpected property extra"
        );
        let mut bad_flag = full_details();
        bad_flag["ignore_all_email"] = json!("no");
        assert_eq!(
            validate_subscription_details(&bad_flag).unwrap_err(),
            "ignore_all_email must be a boolean"
        );
        let mut unknown = full_details();
        unknown["message_type"]["made_up"] = json!({"email": true, "in_app": true});
        assert_eq!(
            validate_subscription_details(&unknown).unwrap_err(),
            "unexpected message_type property made_up"
        );
        let mut chan = full_details();
        chan["message_type"]["bid_received"] = json!({"email": true});
        assert_eq!(
            validate_subscription_details(&chan).unwrap_err(),
            "message_type.bid_received.in_app is required"
        );
        assert_eq!(
            validate_subscription_details(&json!([])).unwrap_err(),
            "must be an object"
        );
    }

    #[test]
    fn normalize_fills_the_full_shape_with_permissive_defaults() {
        let n = normalize_details(&json!({}));
        assert_eq!(n["ignore_all_email"], json!(false));
        assert_eq!(n["ignore_all_in_app"], json!(false));
        assert_eq!(
            n["message_type"].as_object().unwrap().len(),
            NOTIFICATION_TYPES.len()
        );
        assert_eq!(
            n["message_type"]["bid_received"],
            json!({"email": true, "in_app": true})
        );

        let n = normalize_details(
            &json!({"ignore_all_email": true, "message_type": {"bid_received": {"email": false}}}),
        );
        assert_eq!(n["ignore_all_email"], json!(true));
        assert_eq!(
            n["message_type"]["bid_received"],
            json!({"email": false, "in_app": true})
        );
        assert_eq!(
            n["message_type"]["item_sold"],
            json!({"email": true, "in_app": true})
        );
    }

    #[test]
    fn timestamp_is_serialized_as_a_string() {
        let item = NotificationItem {
            id: Uuid::nil(),
            kind: "bid_received".into(),
            address: "0xa".into(),
            timestamp: 1700000000123,
            read: false,
            created_at: "x".into(),
            updated_at: "y".into(),
            metadata: json!({}),
        };
        let v = serde_json::to_value(&item).unwrap();
        assert_eq!(v["timestamp"], json!("1700000000123"));
        assert_eq!(v["id"], json!("00000000-0000-0000-0000-000000000000"));
        assert_eq!(v["type"], json!("bid_received"));
    }
}
