//! Durable event notification intents. Explore captures transitions in the
//! event's own transaction; social drains the queue through its shared
//! notification store. A mirror refresh never enqueues an authoring action.

use chrono::{NaiveDateTime, Utc};
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
    for kind in kinds {
        sqlx::query(
            "INSERT INTO explore.event_notification_intents (id,event_id,kind,event,local_event,occurred_at) \
             VALUES ($1,$2,$3,$4,$5,$6)",
        ).bind(Uuid::new_v4()).bind(event_id).bind(kind).bind(after).bind(local)
            .bind(Utc::now().naive_utc()).execute(&mut **tx).await?;
    }
    Ok(())
}

/// Stable identity for an occurrence's reminder/start dispatch, including
/// the occurrence instant so recurring events can notify again each time.
pub fn occurrence_id(event_id: &str, starts_at: NaiveDateTime, kind: &str) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("event-occurrence:{event_id}:{starts_at}:{kind}").as_bytes(),
    )
}
