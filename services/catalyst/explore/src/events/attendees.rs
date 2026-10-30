//! `GET /api/events/{id}/attendees`: the local RSVPs (`explore.event_attendance_local`)
//! ahead of the mirrored `latest_attendees`, deduplicated case-insensitively.
//! The going/cancel writes (`events::writes`) upsert explicit local state,
//! including a cancellation tombstone that overrides the mirrored preview.
//! row and answer the list in the same transaction.

use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;

use super::EventAttendeeRecord;
use crate::shared::errors::ApiError;

const LOCAL_LIMIT: i64 = 500;

/// The local RSVPs and the mirrored event ride in one row: `$1` is the event id.
fn list_sql() -> String {
    format!(
        "SELECT a.signers, a.names, a.times, e.raw, e.fetched_at, \
          ARRAY(SELECT lower(ua.address) FROM jsonb_array_elements_text(CASE WHEN jsonb_typeof(e.raw->'latest_attendees')='array' THEN e.raw->'latest_attendees' ELSE '[]' END) ua(address) \
                WHERE EXISTS (SELECT 1 FROM explore.event_attendance_local cancelled WHERE cancelled.event_id=$1 AND cancelled.signer=lower(ua.address) AND cancelled.action='cancel')) \
         FROM (SELECT coalesce(array_agg(x.signer ORDER BY x.signed_at DESC), '{{}}') AS signers, \
                      coalesce(array_agg(x.user_name ORDER BY x.signed_at DESC), '{{}}') AS names, \
                      coalesce(array_agg(x.signed_at ORDER BY x.signed_at DESC), '{{}}') AS times \
               FROM (SELECT signer, signed_payload->>'user_name' AS user_name, signed_at \
                     FROM explore.event_attendance_local WHERE event_id = $1 AND action = 'going' \
                     ORDER BY signed_at DESC LIMIT {LOCAL_LIMIT}) x) a \
         LEFT JOIN explore.event e ON e.id = $1"
    )
}

type ListRow = (
    Vec<String>,
    Vec<Option<String>>,
    Vec<NaiveDateTime>,
    Option<Value>,
    Option<NaiveDateTime>,
    Vec<String>,
);

fn to_records(event_id: &str, row: ListRow) -> Vec<EventAttendeeRecord> {
    let (signers, names, times, raw, fetched_at, cancelled) = row;
    let mut seen: std::collections::HashSet<String> = cancelled
        .into_iter()
        .map(|address| address.to_ascii_lowercase())
        .collect();
    let mut out: Vec<EventAttendeeRecord> = Vec::new();
    for (i, signer) in signers.into_iter().enumerate() {
        if seen.insert(signer.to_lowercase()) {
            out.push(EventAttendeeRecord {
                event_id: event_id.to_string(),
                user: signer,
                user_name: names.get(i).cloned().flatten(),
                created_at: times.get(i).map(|t| t.and_utc()).unwrap_or_else(Utc::now),
            });
        }
    }
    if let Some(raw) = raw {
        let cached_ts = raw
            .get("updated_at")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&Utc))
            .or(fetched_at.map(|t| t.and_utc()))
            .unwrap_or_else(Utc::now);
        if let Some(arr) = raw.get("latest_attendees").and_then(|v| v.as_array()) {
            for addr in arr.iter().filter_map(Value::as_str) {
                if seen.insert(addr.to_lowercase()) {
                    out.push(EventAttendeeRecord {
                        event_id: event_id.to_string(),
                        user: addr.to_string(),
                        user_name: None,
                        created_at: cached_ts,
                    });
                }
            }
        }
    }
    out.sort_by_key(|b| std::cmp::Reverse(b.created_at));
    out.truncate(LOCAL_LIMIT as usize);
    out
}

pub async fn list_for_event(
    pool: &PgPool,
    event_id: &str,
) -> Result<Vec<EventAttendeeRecord>, ApiError> {
    let row: ListRow = sqlx::query_as(sqlx::AssertSqlSafe(list_sql()))
        .bind(event_id)
        .fetch_one(pool)
        .await?;
    Ok(to_records(event_id, row))
}

/// The signer is going (`user_name` from the request's metadata): the row
/// upserted and the list read in one transaction.
pub async fn rsvp_going(
    pool: &PgPool,
    event_id: &str,
    signer: &str,
    user_name: Option<&str>,
) -> Result<Vec<EventAttendeeRecord>, ApiError> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO explore.event_attendance_local (event_id, signer, signed_payload, action, signed_at) \
         VALUES ($1, $2, $3, 'going', $4) \
         ON CONFLICT (event_id, signer) DO UPDATE SET signed_payload = EXCLUDED.signed_payload, action = 'going', signed_at = EXCLUDED.signed_at",
    )
    .bind(event_id)
    .bind(signer.to_lowercase())
    .bind(serde_json::json!({ "user_name": user_name }))
    .bind(Utc::now().naive_utc())
    .execute(&mut *tx)
    .await?;
    let row: ListRow = sqlx::query_as(sqlx::AssertSqlSafe(list_sql()))
        .bind(event_id)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(to_records(event_id, row))
}

/// The cancellation marker survives mirror refreshes and overrides its preview.
pub async fn rsvp_cancel(
    pool: &PgPool,
    event_id: &str,
    signer: &str,
) -> Result<Vec<EventAttendeeRecord>, ApiError> {
    let mut tx = pool.begin().await?;
    sqlx::query("INSERT INTO explore.event_attendance_local (event_id, signer, action) VALUES ($1,$2,'cancel') ON CONFLICT (event_id,signer) DO UPDATE SET action='cancel',signed_at=timezone('UTC',now())")
        .bind(event_id)
        .bind(signer.to_lowercase())
        .execute(&mut *tx)
        .await?;
    let row: ListRow = sqlx::query_as(sqlx::AssertSqlSafe(list_sql()))
        .bind(event_id)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(to_records(event_id, row))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn local_rsvps_lead_and_upstream_attendees_dedupe_case_insensitively() {
        let now = Utc::now().naive_utc();
        let row: ListRow = (
            vec!["0xaaa".into(), "0xbbb".into()],
            vec![Some("A".into()), None],
            vec![now, now - chrono::Duration::minutes(1)],
            Some(json!({
                "updated_at": "2026-08-16T04:00:00.019Z",
                "latest_attendees": ["0xAAA", "0xccc"]
            })),
            Some(now),
            Vec::new(),
        );
        let out = to_records("e1", row);
        assert_eq!(
            out.iter().map(|a| a.user.as_str()).collect::<Vec<_>>(),
            vec!["0xaaa", "0xbbb", "0xccc"]
        );
        assert_eq!(out[0].user_name.as_deref(), Some("A"));
        assert_eq!(
            out[2]
                .created_at
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "2026-08-16T04:00:00.019Z"
        );
        assert!(to_records("e2", (vec![], vec![], vec![], None, None, vec![])).is_empty());
    }
}
