//! Friendship notifications: the rows notifications-workers writes for the
//! `FRIENDSHIP_REQUEST` and `FRIENDSHIP_ACCEPTED` events social-service-ea
//! publishes, produced here in process.
//!
//! The event follows upstream's `src/logic/notifications.ts`
//! (`decentraland/social-service-ea` at `b6b2cef`): sent for a request or an
//! accept only, keyed by the action's id, naming both parties and, for a
//! request, its message. The row follows the processor's event parser
//! (`processor/src/logic/event-parser.ts` of
//! `decentraland/notifications-workers` at `0f113fb`): one row for the
//! receiver (the party that did not act). Its id is a v5 uuid over (type,
//! recipient, action id), so a retried write is the same row.

use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::store::{NewNotification, insert_notifications};
use crate::Address;

const FRIENDSHIP_NS: Uuid = Uuid::from_bytes([
    0x2f, 0x41, 0x8a, 0x06, 0x7c, 0x3b, 0x4e, 0x52, 0x9d, 0x17, 0x60, 0xe4, 0xb8, 0x0d, 0x35, 0xa9,
]);

pub const REQUEST: &str = "social_service_friendship_request";
pub const ACCEPTED: &str = "social_service_friendship_accepted";

/// One party of the event as the processor stores it.
pub struct Party<'a> {
    pub address: &'a str,
    pub name: &'a str,
    pub profile_image_url: &'a str,
    pub has_claimed_name: bool,
}

impl Party<'_> {
    fn json(&self) -> Value {
        json!({
            "address": self.address,
            "name": self.name,
            "profileImageUrl": self.profile_image_url,
            "hasClaimedName": self.has_claimed_name,
        })
    }
}

/// The receiver's row for the action `request_id`: `kind` is [`REQUEST`]
/// (with its message, when there is one) or [`ACCEPTED`] (never a message).
/// Returns how many rows were new.
pub async fn publish(
    pool: &PgPool,
    kind: &'static str,
    request_id: Uuid,
    sender: &Party<'_>,
    receiver: &Party<'_>,
    message: Option<&str>,
) -> sqlx::Result<u64> {
    let Ok(recipient) = Address::parse(receiver.address) else {
        return Ok(0);
    };
    let mut metadata = json!({
        "requestId": request_id,
        "sender": sender.json(),
        "receiver": receiver.json(),
    });
    if let Some(message) = message.filter(|m| kind == REQUEST && !m.is_empty()) {
        metadata["message"] = json!(message);
    }
    let identity = serde_json::to_vec(&(kind, recipient.as_str(), request_id.to_string()))
        .expect("the identity holds only strings");
    let row = NewNotification {
        id: Uuid::new_v5(&FRIENDSHIP_NS, &identity),
        ..NewNotification::now(recipient, kind, metadata)
    };
    insert_notifications(pool, std::slice::from_ref(&row)).await
}
