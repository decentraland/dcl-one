//! Durable event notification intents. Explore captures transitions in the
//! event's own transaction; social drains the queue through its shared
//! notification store. A mirror refresh never enqueues an authoring action.

use chrono::Utc;
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

fn flag(raw: &Value, key: &str) -> bool {
    raw.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn text<'a>(raw: &'a Value, key: &str) -> Option<&'a str> {
    raw.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
}

/// Only the fields needed to address and render a notice enter its durable
/// payload. Private organizer details, recurrence arrays and future fields
/// cannot accidentally become part of the notification record. `x`, `y` and
/// `server` build the jump-in link (`Notifications/index.ts:30-39`).
pub fn snapshot(event: &Value) -> Value {
    let mut result = serde_json::Map::new();
    for key in [
        "id",
        "name",
        "image",
        "description",
        "duration",
        "user",
        "community_id",
        "rejection_reason",
        "deleted_reason",
        "x",
        "y",
        "server",
    ] {
        if let Some(value) = event.get(key) {
            result.insert(key.into(), value.clone());
        }
    }
    Value::Object(result)
}

/// `buildEventLink` (`Notifications/index.ts:30-39`): `JUMP_IN_SITE_URL/events`
/// with `id`, `position=x,y` and, when the event names a server, `realm`
/// (URLSearchParams encoding). The reminder, start and approval notices link
/// here (`:48,85,205`).
pub fn event_link(jump_in_site_url: &str, event_id: &str, event: &Value) -> String {
    let x = event.get("x").and_then(Value::as_i64).unwrap_or(0);
    let y = event.get("y").and_then(Value::as_i64).unwrap_or(0);
    let mut link = format!(
        "{}/events?id={}&position={}",
        jump_in_site_url.trim_end_matches('/'),
        urlencoding::encode(event_id),
        urlencoding::encode(&format!("{x},{y}"))
    );
    if let Some(server) = text(event, "server") {
        link.push_str("&realm=");
        link.push_str(&urlencoding::encode(server));
    }
    link
}

/// Capture host state changes and approved community associations. Pending
/// creation sends no user notice, matching the original moderation workflow.
/// A local community association is required: mirrored metadata alone never
/// authorizes notifying this node's community members.
pub async fn record_transition(
    tx: &mut Transaction<'_, Postgres>,
    event_id: &str,
    before: Option<&Value>,
    after: &Value,
    local: bool,
    settings: &crate::Settings,
) -> Result<(), sqlx::Error> {
    let empty = Value::Null;
    let before = before.unwrap_or(&empty);
    let deleted = flag(after, "deleted_by_user") || flag(after, "deleted_by_admin");
    let previously_deleted = flag(before, "deleted_by_user") || flag(before, "deleted_by_admin");
    let mut kinds = Vec::new();
    if !deleted {
        if flag(after, "approved") && !flag(before, "approved") {
            kinds.push("event_approved");
        }
        if flag(after, "rejected") && !flag(before, "rejected") {
            kinds.push("event_rejected");
        }
        if local
            && flag(after, "approved")
            && text(after, "community_id").is_some()
            && (!flag(before, "approved")
                || text(before, "community_id") != text(after, "community_id"))
        {
            kinds.push("event_created");
        }
    } else if !previously_deleted && flag(after, "deleted_by_admin") {
        kinds.push("event_deleted");
    }
    let snapshot = snapshot(after);
    for kind in kinds {
        sqlx::query(
            "INSERT INTO social.event_notification_intents (id,event_id,kind,event,local_event,occurred_at) \
             VALUES ($1,$2,$3,$4,$5,$6)",
        ).bind(Uuid::new_v4()).bind(event_id).bind(kind).bind(&snapshot).bind(local)
            .bind(Utc::now().naive_utc()).execute(&mut **tx).await?;
    }
    if slack_notices_enabled(settings) {
        super::hooks::record(tx, event_id, before, after, local).await?;
    }
    Ok(())
}

/// The Slack moderation channel is on: operator writes and a webhook.
pub fn slack_notices_enabled(settings: &crate::Settings) -> bool {
    settings.operator_writes_enabled
        && settings
            .events_slack_webhook
            .as_deref()
            .is_some_and(|endpoint| !endpoint.trim().is_empty())
}

/// The reminder job's Slack notice (`Event/cron.ts:76`, `Slack/utils.ts:222-257`):
/// an occurrence whose "starts in an hour" notices went out, with how many.
/// Social calls this in the transaction that completes the reminder intent;
/// the caller checks the channel switch (it has no explore settings). Nothing
/// is queued for zero notices, as upstream (`Slack/utils.ts:226-228`).
pub async fn record_upcoming_notice(
    tx: &mut Transaction<'_, Postgres>,
    event_id: &str,
    event: &Value,
    notifications: i64,
) -> Result<(), sqlx::Error> {
    if notifications <= 0 {
        return Ok(());
    }
    super::hooks::record_upcoming(tx, event_id, event, notifications).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn notice_snapshots_exclude_private_and_unneeded_fields_by_default() {
        let payload = snapshot(
            &json!({"name":"Public", "duration":3600000, "contact":"private@example.org", "details":"secret", "recurrent_dates":["2099-01-01"], "future_private_field":"secret", "x":1, "y":-2, "server":"w.dcl.eth"}),
        );
        assert_eq!(
            payload,
            json!({"name":"Public","duration":3600000,"x":1,"y":-2,"server":"w.dcl.eth"})
        );
    }

    /// `Notifications/index.ts:30-39`: `id`, `position` and the optional `realm`.
    #[test]
    fn event_links_follow_build_event_link() {
        assert_eq!(
            event_link(
                "https://decentraland.org/jump",
                "ev-1",
                &json!({"x":10,"y":-20})
            ),
            "https://decentraland.org/jump/events?id=ev-1&position=10%2C-20"
        );
        assert_eq!(
            event_link(
                "https://jump.example.org/",
                "ev 2",
                &json!({"x":0,"y":0,"server":"world.dcl.eth&position=1,2"})
            ),
            "https://jump.example.org/events?id=ev%202&position=0%2C0&realm=world.dcl.eth%26position%3D1%2C2"
        );
        assert_eq!(
            event_link(
                "https://decentraland.org/jump",
                "ev-3",
                &json!({"server":""})
            ),
            "https://decentraland.org/jump/events?id=ev-3&position=0%2C0"
        );
    }
}
