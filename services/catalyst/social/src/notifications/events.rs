//! Event notices over local RSVP consent and managed, local community
//! associations. A rolling one-hour catch-up uses retained occurrence intents
//! as durable scan progress. Explore captures moderation changes
//! atomically with the event. Dispatch pages commit their recipient cursor
//! with notification/email-outbox inserts, so crashes and retries are safe.

use std::time::Duration;

use chrono::{Duration as Span, NaiveDateTime, Utc};
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::store::{NewNotification, insert_event_notifications};
use crate::identity::Address;

const PAGE: i64 = 100;
const CATCH_UP_SECONDS: i64 = 3600;
const PASS_BUDGET: Duration = Duration::from_secs(5);
const SCAN_BUDGET: Duration = Duration::from_secs(3);
const CLEANUP_BUDGET: Duration = Duration::from_millis(500);
const RETENTION_DAYS: i64 = 30;

#[derive(sqlx::FromRow)]
struct Occurrence {
    event_id: String,
    event: Value,
    local_event: bool,
    starts_at: NaiveDateTime,
    due_at: NaiveDateTime,
}

#[derive(sqlx::FromRow)]
struct Intent {
    id: Uuid,
    event_id: String,
    kind: String,
    event: Value,
    local_event: bool,
    occurrence_start: Option<NaiveDateTime>,
    occurred_at: NaiveDateTime,
    recipient_after: String,
}

#[derive(sqlx::FromRow)]
struct Recipient {
    address: String,
    community: bool,
    community_name: Option<String>,
}

fn occurrence_id(event_id: &str, starts_at: NaiveDateTime, kind: &str) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("event-occurrence:{event_id}:{starts_at}:{kind}").as_bytes(),
    )
}

// finish_at is the final series finish, unlike next_finish_at which can remain
// stale until the next catalogue refresh. Bound live candidates before expanding
// recurrence, then keyset-order the unqueued occurrences. The unique queue key
// makes concurrent scans safe and avoids losing late approvals behind a cursor.
const SCAN_SQL: &str = "
    SELECT DISTINCT e.id AS event_id,e.raw - 'recurrent_dates' AS event,e.origin='local' AS local_event,
           d.starts_at,d.starts_at - $2::int4 * interval '1 second' AS due_at
    FROM explore.event e CROSS JOIN LATERAL (
        SELECT CASE WHEN pg_input_is_valid(value, 'timestamp with time zone')
               THEN value::timestamptz AT TIME ZONE 'UTC' END AS starts_at
        FROM jsonb_array_elements_text(CASE WHEN jsonb_typeof(e.raw->'recurrent_dates')='array'
             AND e.raw->'recurrent_dates' <> '[]'::jsonb THEN e.raw->'recurrent_dates'
             ELSE jsonb_build_array(e.raw->'start_at') END)
    ) d
    WHERE e.approved AND e.finish_at > $3
      AND e.start_at <= $3 + $2::int4 * interval '1 second'
      AND COALESCE(e.raw->>'rejected','false') <> 'true'
      AND COALESCE(e.raw->>'deleted_by_user','false') <> 'true'
      AND COALESCE(e.raw->>'deleted_by_admin','false') <> 'true'
      AND (EXISTS (SELECT 1 FROM explore.event_attendance_local a
                   WHERE a.event_id=e.id AND a.action='going')
           OR ($1='events_started' AND e.origin='local' AND EXISTS (
               SELECT 1 FROM social.communities c JOIN social.community_members m ON m.community_id=c.id
               WHERE c.id=CASE WHEN pg_input_is_valid(e.community_id,'uuid') THEN e.community_id::uuid END
                 AND c.origin='local' AND c.active AND NOT c.suspended
                 AND NOT EXISTS (SELECT 1 FROM social.subscription_opt_outs o
                     WHERE o.address=m.member_address AND o.scope='community' AND CASE WHEN pg_input_is_valid(o.scope_id,'uuid') THEN o.scope_id::uuid END=c.id))))
      AND d.starts_at - $2::int4 * interval '1 second' >= $4
      AND d.starts_at - $2::int4 * interval '1 second' <= $3
      AND CASE WHEN $1='events_starts_soon' THEN d.starts_at > $3
               ELSE d.starts_at + e.duration_ms * interval '1 millisecond' > $3 END
      AND NOT EXISTS (SELECT 1 FROM social.event_notification_intents q
                      WHERE q.event_id=e.id AND q.occurrence_start=d.starts_at AND q.kind=$1)
    ORDER BY due_at,event_id LIMIT $5";

async fn scan_kind(
    pool: &PgPool,
    kind: &str,
    ahead: i32,
    now: NaiveDateTime,
) -> anyhow::Result<u64> {
    let mut tx = timed_transaction(pool).await?;
    sqlx::query("SET LOCAL statement_timeout = '2s'")
        .execute(&mut *tx)
        .await?;
    let rows: Vec<Occurrence> = sqlx::query_as(SCAN_SQL)
        .bind(kind)
        .bind(ahead)
        .bind(now)
        .bind(now - Span::seconds(CATCH_UP_SECONDS))
        .bind(PAGE)
        .fetch_all(&mut *tx)
        .await?;
    let ids: Vec<Uuid> = rows
        .iter()
        .map(|row| occurrence_id(&row.event_id, row.starts_at, kind))
        .collect();
    let event_ids: Vec<&str> = rows.iter().map(|row| row.event_id.as_str()).collect();
    let events: Vec<Value> = rows.iter().map(|row| row.event.clone()).collect();
    let local: Vec<bool> = rows.iter().map(|row| row.local_event).collect();
    let starts: Vec<NaiveDateTime> = rows.iter().map(|row| row.starts_at).collect();
    let due: Vec<NaiveDateTime> = rows.iter().map(|row| row.due_at).collect();
    sqlx::query(
        "INSERT INTO social.event_notification_intents (id,event_id,event,local_event,occurrence_start,occurred_at,kind) \
         SELECT n.*,$7 FROM UNNEST($1::uuid[],$2::text[],$3::jsonb[],$4::boolean[],$5::timestamp[],$6::timestamp[]) n \
         ON CONFLICT DO NOTHING",
    ).bind(ids).bind(event_ids).bind(events).bind(local).bind(starts).bind(due).bind(kind).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(rows.len() as u64)
}

fn text<'a>(event: &'a Value, field: &str) -> &'a str {
    event.get(field).and_then(Value::as_str).unwrap_or_default()
}

async fn recipients(
    tx: &mut Transaction<'_, Postgres>,
    intent: &Intent,
) -> anyhow::Result<Vec<Recipient>> {
    let community_id = Uuid::parse_str(text(&intent.event, "community_id")).ok();
    // A community scope opt-out affects the community audience. An explicit
    // RSVP remains independent consent and can still receive a generic notice.
    let rows = sqlx::query_as(
        "WITH candidates AS ( \
            SELECT $3::text AS address,false AS community,NULL::text AS community_name \
            WHERE $2 IN ('event_approved','event_rejected','event_deleted') AND $3 > $6 \
            UNION ALL \
            (SELECT a.signer,false,NULL FROM explore.event_attendance_local a JOIN explore.event e ON e.id=a.event_id \
            WHERE a.event_id=$1 AND a.signer > $6 AND a.action='going' AND $2 IN ('events_starts_soon','events_started') \
              AND e.approved AND COALESCE(e.raw->>'rejected','false') <> 'true' \
              AND COALESCE(e.raw->>'deleted_by_user','false') <> 'true' AND COALESCE(e.raw->>'deleted_by_admin','false') <> 'true' \
            ORDER BY a.signer LIMIT $7) \
            UNION ALL \
            (SELECT m.member_address,true,c.name FROM social.community_members m \
            JOIN social.communities c ON c.id=m.community_id \
            JOIN explore.event e ON e.id=$1 AND CASE WHEN pg_input_is_valid(e.community_id,'uuid') THEN e.community_id::uuid END=$5::uuid AND e.origin='local' \
            WHERE $4 AND $2 IN ('event_created','events_started') AND c.id=$5::uuid AND m.member_address > $6 \
              AND c.origin='local' AND c.active AND NOT c.suspended AND e.approved \
              AND COALESCE(e.raw->>'rejected','false') <> 'true' \
              AND COALESCE(e.raw->>'deleted_by_user','false') <> 'true' AND COALESCE(e.raw->>'deleted_by_admin','false') <> 'true' \
              AND NOT EXISTS (SELECT 1 FROM social.subscription_opt_outs o \
                  WHERE o.address=m.member_address AND o.scope='community' AND CASE WHEN pg_input_is_valid(o.scope_id,'uuid') THEN o.scope_id::uuid END=$5::uuid) \
            ORDER BY m.member_address LIMIT $7) \
        ) SELECT address,bool_or(community) AS community,max(community_name) AS community_name \
          FROM candidates WHERE address > $6 GROUP BY address ORDER BY address LIMIT $7",
    ).bind(&intent.event_id).bind(&intent.kind).bind(text(&intent.event,"user"))
        .bind(intent.local_event).bind(community_id).bind(&intent.recipient_after)
        .bind(PAGE).fetch_all(&mut **tx).await?;
    Ok(rows)
}

fn metadata(intent: &Intent, recipient: &Recipient, base: &str) -> Value {
    let name = text(&intent.event, "name");
    let description =
        crate::explore::sanitize::sanitize_event_description(text(&intent.event, "description"));
    let link = format!(
        "{}/whats-on/{}",
        base.trim_end_matches('/'),
        urlencoding::encode(&intent.event_id)
    );
    let (title, description) = match intent.kind.as_str() {
        "events_starts_soon" => (
            "Event starts soon".to_string(),
            format!("The event {name} starts soon."),
        ),
        "events_started" if recipient.community => (
            "Community Event starting".into(),
            format!(
                "A {} event is about to start.",
                recipient.community_name.as_deref().unwrap_or("community")
            ),
        ),
        "events_started" => (
            "Event started".into(),
            format!("The event {name} has begun!"),
        ),
        "event_created" => (
            "Community Event Added".into(),
            format!(
                "The {} Community has added a new event.",
                recipient.community_name.as_deref().unwrap_or("community")
            ),
        ),
        _ => (name.to_string(), description),
    };
    let mut metadata = json!({"eventId":intent.event_id,"title":title,"description":description,"name":name,"image":text(&intent.event,"image"),"link":link,"occurredAt":intent.occurred_at.and_utc().to_rfc3339()});
    if let Some(start) = intent.occurrence_start {
        let duration = intent
            .event
            .get("duration")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .max(0);
        metadata["startsAt"] = json!(start.and_utc().to_rfc3339());
        metadata["endsAt"] = json!(
            Span::try_milliseconds(duration)
                .and_then(|duration| start.checked_add_signed(duration))
                .map(|time| time.and_utc().to_rfc3339())
        );
    }
    if recipient.community {
        metadata["communityId"] = json!(text(&intent.event, "community_id"));
        metadata["communityName"] = json!(recipient.community_name);
    }
    if intent.kind == "event_rejected" {
        metadata["reason"] = json!(text(&intent.event, "rejection_reason"));
    }
    if intent.kind == "event_deleted" {
        metadata["reason"] = json!(text(&intent.event, "deleted_reason"));
    }
    metadata
}

/// PostgreSQL cancels blocked or slow statements even if the Rust future is
/// dropped. SET LOCAL is transaction-scoped and never leaks into pooled users.
async fn timed_transaction(pool: &PgPool) -> Result<Transaction<'_, Postgres>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL statement_timeout = '500ms'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL lock_timeout = '100ms'")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}

fn timed_out(error: &anyhow::Error) -> bool {
    matches!(error.downcast_ref::<sqlx::Error>(), Some(sqlx::Error::Database(db))
        if matches!(db.code().as_deref(), Some("57014" | "55P03")))
}

async fn dispatch_page(
    pool: &PgPool,
    base: &str,
    now: NaiveDateTime,
) -> anyhow::Result<Option<u64>> {
    // Commit fair rotation separately from delivery. If a later page times out
    // or rolls back, another intent gets the next turn instead of starvation.
    let mut claim = timed_transaction(pool).await?;
    let id: Option<Uuid> = sqlx::query_scalar(
        "UPDATE social.event_notification_intents SET dispatch_at=timezone('UTC', clock_timestamp())
         WHERE id=(SELECT id FROM social.event_notification_intents WHERE processed_at IS NULL
                   ORDER BY dispatch_at,id LIMIT 1 FOR UPDATE SKIP LOCKED) RETURNING id",
    ).fetch_optional(&mut *claim).await?;
    claim.commit().await?;
    let Some(id) = id else { return Ok(None) };
    let mut tx = timed_transaction(pool).await?;
    let intent: Option<Intent> = sqlx::query_as(
        "SELECT id,event_id,kind,event,local_event,occurrence_start,occurred_at,recipient_after
         FROM social.event_notification_intents WHERE id=$1 AND processed_at IS NULL FOR UPDATE SKIP LOCKED",
    ).bind(id).fetch_optional(&mut *tx).await?;
    let Some(mut intent) = intent else {
        tx.commit().await?;
        return Ok(Some(0));
    };
    // A queued occurrence may have been moved or removed since its snapshot.
    // Revalidate the current event under a shared row lock before delivering;
    // a reschedule's old intent is consumed, while its new occurrence is scanned.
    let mut obsolete = false;
    if let Some(start) = intent.occurrence_start {
        let current: Option<(Value, bool)> = sqlx::query_as(
            "SELECT e.raw,e.origin='local' FROM explore.event e
             WHERE e.id=$1 AND e.approved
               AND COALESCE(e.raw->>'rejected','false') <> 'true'
               AND COALESCE(e.raw->>'deleted_by_user','false') <> 'true'
               AND COALESCE(e.raw->>'deleted_by_admin','false') <> 'true'
               AND EXISTS (SELECT 1 FROM jsonb_array_elements_text(
                   CASE WHEN jsonb_typeof(e.raw->'recurrent_dates')='array'
                        AND e.raw->'recurrent_dates' <> '[]'::jsonb THEN e.raw->'recurrent_dates'
                        ELSE jsonb_build_array(e.raw->'start_at') END) d(value)
                   WHERE CASE WHEN pg_input_is_valid(value,'timestamp with time zone')
                         THEN value::timestamptz AT TIME ZONE 'UTC' END=$2)
             FOR SHARE",
        )
        .bind(&intent.event_id)
        .bind(start)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some((event, local)) = current {
            intent.event = event;
            intent.local_event = local;
        } else {
            obsolete = true;
        }
    }
    let delivered_at: NaiveDateTime =
        sqlx::query_scalar("SELECT timezone('UTC', clock_timestamp())")
            .fetch_one(&mut *tx)
            .await?;
    // Recheck expiry after lock waits and scans, not only at pass start. The
    // supplied horizon can be ahead of the wall clock for deterministic replay.
    let checked_at = now.max(delivered_at);
    let suppression = suppression_reason(&intent, checked_at, obsolete);
    let recipients = if suppression.is_some() {
        Vec::new()
    } else {
        recipients(&mut tx, &intent).await?
    };
    let batch: Vec<NewNotification> = recipients
        .iter()
        .filter_map(|recipient| {
            let address: Address = recipient.address.parse().ok()?;
            Some(NewNotification {
                id: Uuid::new_v5(&intent.id, address.as_str().as_bytes()),
                address,
                kind: intent.kind.clone(),
                metadata: metadata(&intent, recipient, base),
                occurred_at: delivered_at,
            })
        })
        .collect();
    let inserted = insert_event_notifications(&mut *tx, &batch).await?;
    let finished = recipients.len() < PAGE as usize;
    sqlx::query("UPDATE social.event_notification_intents SET recipient_after=$2,processed_at=CASE WHEN $3 THEN $4 ELSE NULL END,dispatch_at=timezone('UTC', clock_timestamp()) WHERE id=$1")
        .bind(intent.id).bind(recipients.last().map(|row| row.address.as_str()).unwrap_or(&intent.recipient_after))
        .bind(finished).bind(checked_at).execute(&mut *tx).await?;
    tx.commit().await?;
    if let Some(reason) = suppression {
        tracing::warn!(intent_id=%intent.id,event_id=%intent.event_id,kind=%intent.kind,reason,suppressed_intents=1,"event notification intent suppressed");
    }
    Ok(Some(inserted))
}

fn suppression_reason(intent: &Intent, now: NaiveDateTime, obsolete: bool) -> Option<&'static str> {
    if obsolete {
        Some("obsolete_occurrence")
    } else if stale_occurrence(intent, now) {
        Some(if intent.kind == "events_started" {
            "event_ended"
        } else {
            "reminder_expired"
        })
    } else {
        None
    }
}

fn stale_occurrence(intent: &Intent, now: NaiveDateTime) -> bool {
    let Some(start) = intent.occurrence_start else {
        return false;
    };
    if intent.kind == "events_starts_soon" {
        return start <= now;
    }
    let end = intent
        .event
        .get("duration")
        .and_then(Value::as_i64)
        .and_then(Span::try_milliseconds)
        .and_then(|duration| start.checked_add_signed(duration));
    intent.kind == "events_started" && end.is_none_or(|end| end <= now)
}

async fn cleanup(pool: &PgPool, now: NaiveDateTime) -> anyhow::Result<()> {
    let mut tx = timed_transaction(pool).await?;
    // Pending work is never purged. Completed keys outlive the one-hour scan
    // window by 30 days, so cleanup cannot make an eligible occurrence new again.
    sqlx::query(
        "DELETE FROM social.event_notification_intents WHERE id IN (
        SELECT id FROM social.event_notification_intents
        WHERE processed_at < $1 AND occurred_at < $1
        ORDER BY processed_at,id LIMIT 1000 FOR UPDATE SKIP LOCKED)",
    )
    .bind(now - Span::days(RETENTION_DAYS))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

async fn scan_with_budget(
    pool: &PgPool,
    kind: &str,
    ahead: i32,
    now: NaiveDateTime,
) -> anyhow::Result<()> {
    let scan = async {
        while scan_kind(pool, kind, ahead, now).await? == PAGE as u64 {}
        Ok::<_, anyhow::Error>(())
    };
    tokio::time::timeout(SCAN_BUDGET, scan)
        .await
        .map_err(|_| anyhow::anyhow!("{kind} occurrence scan exceeded its budget"))?
        .map_err(|error| error.context(format!("{kind} occurrence scan failed")))
}

/// Cleanup always receives a turn before scanning/dispatch. Independent scan
/// budgets prevent one occurrence kind starving the other. Committed rotation
/// survives a timed-out page. Scan failures surface after dispatch; maintenance
/// errors are logged without blocking delivery.
pub async fn run_once(pool: &PgPool, base: &str, now: NaiveDateTime) -> anyhow::Result<u64> {
    match tokio::time::timeout(CLEANUP_BUDGET, cleanup(pool, now)).await {
        Ok(Err(error)) => {
            tracing::warn!(%error, "event intent cleanup deferred")
        }
        Ok(Ok(())) => {}
        Err(_) => {}
    }
    let (reminders, starts) = tokio::join!(
        scan_with_budget(pool, "events_starts_soon", 3600, now),
        scan_with_budget(pool, "events_started", 0, now),
    );
    let deadline = tokio::time::Instant::now() + PASS_BUDGET;
    let mut emitted = 0;
    loop {
        match tokio::time::timeout_at(deadline, dispatch_page(pool, base, now)).await {
            Ok(Err(error)) if timed_out(&error) => {
                tracing::warn!(%error, "event notification page deferred")
            }
            Ok(result) => match result? {
                Some(inserted) => emitted += inserted,
                None => break,
            },
            Err(_) => break,
        }
    }
    reminders?;
    starts?;
    Ok(emitted)
}

pub fn spawn(pool: PgPool, base: String, jobs: &crate::jobs::Jobs) {
    jobs.spawn_periodic_leased(
        "event-notifications",
        crate::jobs::PeriodicCfg::every(Duration::from_secs(60))
            .with_error_backoff(Duration::from_secs(300)),
        move |ctx| {
            let (pool, base) = (pool.clone(), base.clone());
            async move {
                let emitted = run_once(&pool, &base, Utc::now().naive_utc()).await?;
                if emitted > 0 {
                    ctx.progress(format!("{emitted} event notices emitted"));
                }
                Ok(())
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::explore::events::moderation::{self, AdminPatch, CreateEvent};

    const HOST: &str = "0x1111111111111111111111111111111111111111";
    const RSVP: &str = "0x2222222222222222222222222222222222222222";
    const DUAL: &str = "0x3333333333333333333333333333333333333333";
    const EMAIL: &str = "0x4444444444444444444444444444444444444444";
    const OPTED: &str = "0x5555555555555555555555555555555555555555";
    const PREVIEW: &str = "0x6666666666666666666666666666666666666666";

    async fn count(pool: &PgPool, event: &str, kind: &str, address: &str) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM social.notifications WHERE metadata->>'eventId'=$1 AND type=$2 AND address=$3")
            .bind(event).bind(kind).bind(address).fetch_one(pool).await.unwrap()
    }

    #[tokio::test]
    async fn event_notices_recover_without_duplicates_respect_consent_and_separate_channels() {
        let Some(pool) = crate::db::scratch::pool(6).await else {
            return;
        };
        let community = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO social.communities (id,name,owner_address) VALUES ($1,'Calendar Club',$2)",
        )
        .bind(community)
        .bind(HOST)
        .execute(&pool)
        .await
        .unwrap();
        for member in [HOST, DUAL, EMAIL, OPTED] {
            sqlx::query("INSERT INTO social.community_members (community_id,member_address,role) VALUES ($1,$2,'member')")
                .bind(community).bind(member).execute(&pool).await.unwrap();
        }
        sqlx::query("INSERT INTO social.subscriptions (address,email,details) VALUES ($1,'email@example.com','{\"ignore_all_in_app\":true}'::jsonb),($2,'dual@example.com','{\"ignore_all_email\":true}'::jsonb) ON CONFLICT (address) DO UPDATE SET email=EXCLUDED.email,details=EXCLUDED.details")
            .bind(EMAIL).bind(DUAL).execute(&pool).await.unwrap();
        for member in [DUAL, OPTED] {
            sqlx::query("INSERT INTO social.subscription_opt_outs (address,scope,scope_id) VALUES ($1,'community',$2)")
                .bind(member).bind(community.to_string()).execute(&pool).await.unwrap();
        }
        let body: CreateEvent = serde_json::from_value(json!({"name":"Calendar event","start_at":"2099-06-01T18:00:00Z","duration":3600000,"x":0,"y":0,"community_id":community.to_string(),"recurrent":true,"recurrent_frequency":"DAILY","recurrent_count":3})).unwrap();
        let event = moderation::create_as(&pool, &body, HOST, HOST)
            .await
            .unwrap();
        let pending: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM social.event_notification_intents WHERE event_id=$1",
        )
        .bind(&event)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            pending, 0,
            "pending creation does not announce an unapproved event"
        );
        for _ in 0..2 {
            moderation::moderate(
                &pool,
                &event,
                &AdminPatch {
                    approved: Some(true),
                    ..Default::default()
                },
                "moderator",
            )
            .await
            .unwrap();
        }
        for member in [RSVP, DUAL] {
            crate::explore::events::attendees::rsvp_going(&pool, &event, member, None)
                .await
                .unwrap();
        }
        sqlx::query(
            "UPDATE explore.event SET raw=jsonb_set(raw,'{latest_attendees}',$2) WHERE id=$1",
        )
        .bind(&event)
        .bind(json!([PREVIEW]))
        .execute(&pool)
        .await
        .unwrap();
        let base = "https://example.org";
        let t0 = chrono::DateTime::parse_from_rfc3339("2099-06-01T16:00:00Z")
            .unwrap()
            .naive_utc();
        let historical = Uuid::new_v4().to_string();
        crate::explore::events::mirror::upsert_event(&pool,&json!({
            "id":historical,"name":"Historical","start_at":"2099-05-31T18:00:00Z","finish_at":"2099-05-31T19:00:00Z",
            "duration":3600000,"approved":true,"recurrent_dates":["2099-05-31T18:00:00Z"]
        })).await.unwrap();
        crate::explore::events::attendees::rsvp_going(&pool, &historical, RSVP, None)
            .await
            .unwrap();
        run_once(&pool, base, t0).await.unwrap();
        assert_eq!(
            count(&pool, &historical, "events_started", RSVP).await,
            0,
            "first boot never replays historical occurrences"
        );
        assert_eq!(count(&pool, &event, "event_approved", HOST).await, 1);
        assert_eq!(count(&pool, &event, "event_created", HOST).await, 1);
        assert_eq!(count(&pool, &event, "event_created", OPTED).await, 0);
        assert_eq!(count(&pool, &event, "event_created", DUAL).await, 0);
        let (visible,email_state): (bool,String) = sqlx::query_as("SELECT n.in_app,d.state FROM social.notifications n JOIN social.notification_deliveries d ON d.notification_id=n.id WHERE n.metadata->>'eventId'=$1 AND n.type='event_created' AND n.address=$2")
            .bind(&event).bind(EMAIL).fetch_one(&pool).await.unwrap();
        assert!(!visible);
        assert_eq!(
            email_state, "pending",
            "in-app opt-out preserves opted-in email"
        );
        let store = super::super::NotificationsStore::new(pool.clone());
        assert!(
            store
                .list(&EMAIL.parse().unwrap(), 100, None, false)
                .await
                .unwrap()
                .iter()
                .all(|notice| notice.metadata["eventId"] != event)
        );

        let soon = t0 + Span::hours(1) + Span::seconds(1);
        let (first, second) =
            tokio::join!(run_once(&pool, base, soon), run_once(&pool, base, soon));
        first.unwrap();
        second.unwrap();
        assert_eq!(count(&pool, &event, "events_starts_soon", RSVP).await, 1);
        assert_eq!(count(&pool, &event, "events_starts_soon", DUAL).await, 1);
        assert_eq!(
            count(&pool, &event, "events_starts_soon", EMAIL).await,
            0,
            "community membership alone does not subscribe to reminders"
        );
        assert_eq!(
            count(&pool, &event, "events_starts_soon", PREVIEW).await,
            0,
            "upstream preview is not local consent"
        );
        let started = t0 + Span::hours(2) + Span::seconds(1);
        run_once(&pool, base, started).await.unwrap();
        run_once(&pool, base, started).await.unwrap();
        assert_eq!(
            count(&pool, &event, "events_started", DUAL).await,
            1,
            "overlapping RSVP/member audience is deduplicated"
        );
        assert_eq!(count(&pool, &event, "events_started", OPTED).await, 0);
        let (state,visible,metadata): (String,bool,Value) = sqlx::query_as("SELECT d.state,n.in_app,n.metadata FROM social.notifications n JOIN social.notification_deliveries d ON d.notification_id=n.id WHERE n.metadata->>'eventId'=$1 AND n.type='events_started' AND n.address=$2")
            .bind(&event).bind(DUAL).fetch_one(&pool).await.unwrap();
        assert_eq!(state, "skipped");
        assert!(visible, "email opt-out preserves in-app delivery");
        assert!(
            metadata.get("communityId").is_none(),
            "scope opt-out falls back to explicit RSVP notice"
        );
        assert_eq!(
            metadata["endsAt"], "2099-06-01T19:00:00+00:00",
            "occurrence duration, not series finish"
        );
        assert_eq!(metadata["link"], format!("{base}/whats-on/{event}"));
        run_once(&pool, base, soon + Span::days(1)).await.unwrap();
        assert_eq!(
            count(&pool, &event, "events_starts_soon", RSVP).await,
            2,
            "each occurrence has a distinct idempotency key"
        );

        moderation::moderate(
            &pool,
            &event,
            &AdminPatch {
                rejected: Some(true),
                reason: Some("Reason".into()),
                ..Default::default()
            },
            "moderator",
        )
        .await
        .unwrap();
        run_once(&pool, base, soon + Span::days(1)).await.unwrap();
        assert_eq!(count(&pool, &event, "event_rejected", HOST).await, 1);

        let queued: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM social.event_notification_intents WHERE event_id=$1",
        )
        .bind(&event)
        .fetch_one(&pool)
        .await
        .unwrap();
        let mut tx = pool.begin().await.unwrap();
        crate::explore::events::notifications::record_transition(
            &mut tx,
            &event,
            None,
            &json!({"approved":true,"user":HOST}),
            true,
        )
        .await
        .unwrap();
        tx.rollback().await.unwrap();
        let after: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM social.event_notification_intents WHERE event_id=$1",
        )
        .bind(&event)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            queued, after,
            "rolled-back event transitions leave no delivery intent"
        );

        // A high-degree audience advances in bounded pages. A restart or
        // concurrent worker resumes after the committed recipient key.
        let body: CreateEvent = serde_json::from_value(json!({"name":"Paged","start_at":"2100-01-01T12:00:00Z","duration":3600000,"x":0,"y":0})).unwrap();
        let paged = moderation::create_as(&pool, &body, HOST, HOST)
            .await
            .unwrap();
        sqlx::query("UPDATE explore.event SET approved=true,raw=raw || '{\"approved\":true}'::jsonb WHERE id=$1")
            .bind(&paged).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO explore.event_attendance_local (event_id,signer,action) SELECT $1,'0x'||lpad(to_hex(n),40,'0'),'going' FROM generate_series(1000,1104) n")
            .bind(&paged).execute(&pool).await.unwrap();
        let intent_id = Uuid::new_v4();
        let paged_start = chrono::DateTime::parse_from_rfc3339("2100-01-01T12:00:00Z")
            .unwrap()
            .naive_utc();
        sqlx::query("INSERT INTO social.event_notification_intents (id,event_id,kind,event,local_event,occurrence_start,occurred_at) SELECT $1,id,'events_started',raw,true,$3,$3 FROM explore.event WHERE id=$2")
            .bind(intent_id).bind(&paged).bind(paged_start).execute(&pool).await.unwrap();
        dispatch_page(&pool, base, paged_start).await.unwrap();
        let pending: bool = sqlx::query_scalar("SELECT processed_at IS NULL AND recipient_after <> '' FROM social.event_notification_intents WHERE id=$1")
            .bind(intent_id).fetch_one(&pool).await.unwrap();
        assert!(pending, "a full page persists its resume key");
        let (left, right) = tokio::join!(
            dispatch_page(&pool, base, paged_start),
            dispatch_page(&pool, base, paged_start)
        );
        left.unwrap();
        right.unwrap();
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM social.notifications WHERE metadata->>'eventId'=$1 AND type='events_started'")
            .bind(&paged).fetch_one(&pool).await.unwrap();
        assert_eq!(count, 105);
        let finished: bool = sqlx::query_scalar(
            "SELECT processed_at IS NOT NULL FROM social.event_notification_intents WHERE id=$1",
        )
        .bind(intent_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(finished);
        catch_up_pages_live_audiences_and_retention(&pool).await;
        bounded_work_preserves_fairness_cleanup_and_polling(&pool).await;
    }

    // The worker deliberately returns when its wall-clock budget expires.
    // Assert eventual completion and durable progress across bounded passes,
    // rather than requiring a particular runner to drain the queue in 5 seconds.
    async fn drain_until_complete(
        pool: &PgPool,
        now: NaiveDateTime,
        event_pattern: &str,
        expected_intents: usize,
    ) {
        let mut previous = std::collections::HashMap::<Uuid, (String, bool)>::new();
        for pass in 0..=32 {
            let progress: Vec<(Uuid, String, bool)> = sqlx::query_as(
                "SELECT id,recipient_after,processed_at IS NOT NULL
                 FROM social.event_notification_intents WHERE event_id LIKE $1",
            )
            .bind(event_pattern)
            .fetch_all(pool)
            .await
            .unwrap();
            for (id, cursor, completed) in &progress {
                if let Some((old_cursor, old_completed)) = previous.get(id) {
                    assert!(
                        cursor >= old_cursor,
                        "a committed recipient cursor must not regress"
                    );
                    assert!(
                        !old_completed || *completed,
                        "completed intents must stay completed"
                    );
                }
                previous.insert(*id, (cursor.clone(), *completed));
            }
            if progress.len() == expected_intents && progress.iter().all(|(_, _, done)| *done) {
                return;
            }
            if pass == 32 {
                break;
            }
            run_once(pool, "https://example.org", now).await.unwrap();
        }
        panic!("event notification queue did not complete within 32 resumable passes");
    }

    async fn catch_up_pages_live_audiences_and_retention(pool: &PgPool) {
        let now = chrono::DateTime::parse_from_rfc3339("2101-06-01T18:00:00Z")
            .unwrap()
            .naive_utc();
        let prefix = Uuid::new_v4().to_string();
        let address = format!("0x{}", Uuid::new_v4().simple())
            .chars()
            .chain("00000000".chars())
            .collect::<String>();
        // 125 simultaneous starts with local consent, 1,000 live catalogue
        // rows without local consent, and 1,000 expired rows with consent.
        sqlx::query("INSERT INTO explore.event (id,name,approved,start_at,finish_at,next_finish_at,duration_ms,raw)
            SELECT $1 || ':' || n,'Catchup',true,$2,$2 + interval '1 hour',$2 + interval '1 hour',3600000,
                jsonb_build_object('name','Catchup','approved',true,'start_at',$2::timestamp,'duration',3600000,'user',$3)
            FROM generate_series(1,1125) n")
            .bind(&prefix).bind(now-Span::minutes(10)).bind(&address).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO explore.event (id,name,approved,start_at,finish_at,next_finish_at,duration_ms,raw)
            SELECT $1 || ':' || n,'Expired',true,$2,$2 + interval '1 hour',$2 + interval '1 hour',3600000,
                jsonb_build_object('name','Expired','approved',true,'start_at',$2::timestamp,'duration',3600000,'user',$3)
            FROM generate_series(1126,2125) n")
            .bind(&prefix).bind(now-Span::hours(3)).bind(&address).execute(pool).await.unwrap();
        sqlx::query(
            "INSERT INTO explore.event_attendance_local(event_id,signer,action)
            SELECT $1 || ':' || n,$2,'going' FROM generate_series(1,2125) n WHERE n<=125 OR n>1125",
        )
        .bind(&prefix)
        .bind(&address)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query("ANALYZE explore.event")
            .execute(pool)
            .await
            .unwrap();
        let plan: Value = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) {SCAN_SQL}"
        )))
        .bind("events_started")
        .bind(0_i32)
        .bind(now)
        .bind(now - Span::hours(1))
        .bind(PAGE)
        .fetch_one(pool)
        .await
        .unwrap();
        eprintln!(
            "event scan smoke: 2125 rows, 125 eligible; execution_ms={}, plan={}",
            plan[0]["Execution Time"], plan[0]["Plan"]
        );
        let before = std::time::Instant::now();
        drain_until_complete(pool, now, &format!("{prefix}:%"), 125).await;
        let notices: i64=sqlx::query_scalar("SELECT count(*) FROM social.notifications WHERE metadata->>'eventId' LIKE $1 AND type='events_started'")
            .bind(format!("{prefix}:%")).fetch_one(pool).await.unwrap();
        assert_eq!(
            notices, 125,
            "all due-time ties across scan pages drain exactly once"
        );
        let queued: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM social.event_notification_intents WHERE event_id LIKE $1",
        )
        .bind(format!("{prefix}:%"))
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(
            queued, 125,
            "expired and no-audience catalogue rows never enter the queue"
        );
        eprintln!(
            "event queue smoke: drained 125 recipients in {:?}",
            before.elapsed()
        );

        // A large community resumes after a partially committed page and an
        // additional short-budget pass. Runner speed must not determine correctness.
        let large_community = Uuid::new_v4();
        let large_event = format!("{prefix}:large");
        sqlx::query("INSERT INTO social.communities(id,name,owner_address) VALUES($1,'Large',$2)")
            .bind(large_community)
            .bind(&address)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO social.community_members(community_id,member_address,role)
            SELECT $1,'0x'||lpad(to_hex(n),40,'0'),'member' FROM generate_series(20000,30004) n",
        )
        .bind(large_community)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO explore.event(id,name,approved,origin,community_id,start_at,finish_at,duration_ms,raw)
            VALUES($1,'Large',true,'local',$2,$3,$3+interval '1 hour',3600000,
            jsonb_build_object('name','Large','approved',true,'community_id',$2::text,'start_at',$3::timestamp,'duration',3600000,'user',$4::text))")
            .bind(&large_event).bind(large_community.to_string()).bind(now+Span::hours(2)).bind(&address).execute(pool).await.unwrap();
        let large_intent = Uuid::new_v4();
        sqlx::query("INSERT INTO social.event_notification_intents(id,event_id,kind,event,local_event,occurred_at)
            SELECT $1,id,'event_created',raw,true,$3 FROM explore.event WHERE id=$2")
            .bind(large_intent).bind(&large_event).bind(now).execute(pool).await.unwrap();
        let large_before = std::time::Instant::now();
        dispatch_page(pool, "https://example.org", now)
            .await
            .unwrap();
        let (cursor, completed): (String, bool) = sqlx::query_as(
            "SELECT recipient_after,processed_at IS NOT NULL FROM social.event_notification_intents WHERE id=$1",
        ).bind(large_intent).fetch_one(pool).await.unwrap();
        assert!(
            !cursor.is_empty() && !completed,
            "one page must persist partial progress"
        );
        if let Ok(result) = tokio::time::timeout(
            Duration::from_millis(1),
            run_once(pool, "https://example.org", now),
        )
        .await
        {
            result.unwrap();
        }
        drain_until_complete(pool, now, &large_event, 1).await;
        let large_count:i64=sqlx::query_scalar("SELECT count(*) FROM social.notifications WHERE metadata->>'eventId'=$1 AND type='event_created'")
            .bind(&large_event).fetch_one(pool).await.unwrap();
        assert_eq!(large_count, 10005);
        let distinct_recipients: i64 = sqlx::query_scalar(
            "SELECT count(DISTINCT address) FROM social.notifications WHERE metadata->>'eventId'=$1 AND type='event_created'",
        ).bind(&large_event).fetch_one(pool).await.unwrap();
        assert_eq!(
            distinct_recipients, large_count,
            "resuming never duplicates recipient delivery"
        );
        eprintln!(
            "event queue smoke: drained 10005-member community in {:?}",
            large_before.elapsed()
        );

        // An approval or new mirror arriving after the scan has passed its due
        // time still gets a live started notice, and a late reminder is caught.
        for (suffix, start) in [
            ("late-start", now - Span::minutes(5)),
            ("late-reminder", now + Span::minutes(20)),
        ] {
            let id = format!("{prefix}:{suffix}");
            let body = json!({"id":id,"name":"Late","approved":false,"start_at":start.and_utc().to_rfc3339(),"finish_at":(start+Span::hours(1)).and_utc().to_rfc3339(),"duration":3600000});
            crate::explore::events::mirror::upsert_event(pool, &body)
                .await
                .unwrap();
            crate::explore::events::attendees::rsvp_going(pool, &id, &address, None)
                .await
                .unwrap();
            run_once(pool, "https://example.org", now).await.unwrap();
            sqlx::query("UPDATE explore.event SET approved=true,raw=raw || '{\"approved\":true}'::jsonb WHERE id=$1").bind(&id).execute(pool).await.unwrap();
            run_once(pool, "https://example.org", now).await.unwrap();
            let kind = if suffix == "late-start" {
                "events_started"
            } else {
                "events_starts_soon"
            };
            assert_eq!(count(pool, &id, kind, &address).await, 1);
        }
        // Moving an already-scanned future event earlier creates a fresh due
        // occurrence, without depending on a monotonically advancing cursor.
        let moved = format!("{prefix}:moved");
        let start = now + Span::hours(3);
        let mut body = json!({"id":moved,"name":"Moved","approved":true,"start_at":start.and_utc().to_rfc3339(),"finish_at":(start+Span::hours(1)).and_utc().to_rfc3339(),"duration":3600000});
        crate::explore::events::mirror::upsert_event(pool, &body)
            .await
            .unwrap();
        crate::explore::events::attendees::rsvp_going(pool, &moved, &address, None)
            .await
            .unwrap();
        run_once(pool, "https://example.org", now).await.unwrap();
        let old_intent = Uuid::new_v4();
        sqlx::query("INSERT INTO social.event_notification_intents(id,event_id,kind,event,local_event,occurrence_start,occurred_at) VALUES($1,$2,'events_starts_soon',$3,false,$4,$5)")
            .bind(old_intent).bind(&moved).bind(&body).bind(start).bind(now).execute(pool).await.unwrap();
        body["start_at"] = json!((now + Span::minutes(30)).and_utc().to_rfc3339());
        body["finish_at"] = json!((now + Span::minutes(90)).and_utc().to_rfc3339());
        crate::explore::events::mirror::upsert_event(pool, &body)
            .await
            .unwrap();
        run_once(pool, "https://example.org", now).await.unwrap();
        assert_eq!(count(pool, &moved, "events_starts_soon", &address).await, 1);

        // A queued started notice can become stale during an outage, and is
        // consumed without producing either an in-app row or email delivery.
        let stale_id = Uuid::new_v4();
        let stale_event = format!("{prefix}:1126");
        sqlx::query("INSERT INTO social.event_notification_intents(id,event_id,kind,event,local_event,occurrence_start,occurred_at)
            SELECT $1,id,'events_started',raw,false,$3,$3 FROM explore.event WHERE id=$2")
            .bind(stale_id).bind(&stale_event).bind(now-Span::hours(3)).execute(pool).await.unwrap();
        run_once(pool, "https://example.org", now).await.unwrap();
        assert_eq!(
            count(pool, &stale_event, "events_started", &address).await,
            0
        );
        let finished: bool = sqlx::query_scalar(
            "SELECT processed_at IS NOT NULL FROM social.event_notification_intents WHERE id=$1",
        )
        .bind(stale_id)
        .fetch_one(pool)
        .await
        .unwrap();
        assert!(finished);
        let recent = Uuid::new_v4();
        let old = Uuid::new_v4();
        let pending = Uuid::new_v4();
        for (id, processed, occurred) in [
            (recent, Some(now), now - Span::days(40)),
            (old, Some(now - Span::days(40)), now - Span::days(40)),
            (pending, None, now - Span::days(40)),
        ] {
            sqlx::query("INSERT INTO social.event_notification_intents(id,event_id,kind,event,local_event,occurred_at,processed_at) VALUES($1,'retention','event_approved','{}',false,$2,$3)")
                .bind(id).bind(occurred).bind(processed).execute(pool).await.unwrap();
        }
        cleanup(pool, now).await.unwrap();
        let retained: Vec<Uuid> =
            sqlx::query_scalar("SELECT id FROM social.event_notification_intents WHERE id=ANY($1)")
                .bind(vec![recent, old, pending])
                .fetch_all(pool)
                .await
                .unwrap();
        assert!(retained.contains(&recent) && retained.contains(&pending));
        assert!(!retained.contains(&old));
        run_once(pool, "https://example.org", now + Span::days(40))
            .await
            .unwrap();
        assert_eq!(
            count(pool, &stale_event, "events_started", &address).await,
            0,
            "retention never makes old occurrences eligible again"
        );
    }

    async fn bounded_work_preserves_fairness_cleanup_and_polling(pool: &PgPool) {
        let now = chrono::DateTime::from_timestamp_micros(Utc::now().timestamp_micros())
            .unwrap()
            .naive_utc();
        let address = format!("0x{}00000000", Uuid::new_v4().simple());
        let prefix = Uuid::new_v4().simple().to_string();
        async fn add_event(pool: &PgPool, id: &str, address: &str, start: NaiveDateTime) {
            let raw = json!({"id":id,"name":"Fair event","approved":true,"start_at":start.and_utc().to_rfc3339(),"finish_at":(start+Span::hours(1)).and_utc().to_rfc3339(),"duration":3600000,"user":address});
            crate::explore::events::mirror::upsert_event(pool, &raw)
                .await
                .unwrap();
            crate::explore::events::attendees::rsvp_going(pool, id, address, None)
                .await
                .unwrap();
        }
        let slow = format!("{prefix}-slow");
        let fast = format!("{prefix}-fast");
        let start = now - Span::minutes(10);
        add_event(pool, &slow, &address, start).await;
        let slow_intent = occurrence_id(&slow, start, "events_started");
        sqlx::query("INSERT INTO social.event_notification_intents(id,event_id,kind,event,local_event,occurrence_start,occurred_at,dispatch_at)
            SELECT $1,id,'events_started',raw,false,$3,$3,TIMESTAMP '1970-01-01' FROM explore.event WHERE id=$2")
            .bind(slow_intent).bind(&slow).bind(start).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO social.event_notification_intents(id,event_id,kind,event,local_event,occurred_at,dispatch_at)
            VALUES($1,$2,'event_approved',$3,false,$4,TIMESTAMP '1970-01-02')")
            .bind(Uuid::new_v4()).bind(&fast).bind(json!({"name":"Fast","user":address})).bind(now).execute(pool).await.unwrap();
        let mut held = pool.begin().await.unwrap();
        sqlx::query("SELECT id FROM explore.event WHERE id=$1 FOR UPDATE")
            .bind(&slow)
            .execute(&mut *held)
            .await
            .unwrap();
        let error = dispatch_page(pool, "https://example.org", now)
            .await
            .unwrap_err();
        assert!(
            timed_out(&error),
            "PostgreSQL must cancel the blocked page: {error}"
        );
        let rotated:bool=sqlx::query_scalar("SELECT dispatch_at > TIMESTAMP '1970-01-02' AND recipient_after='' AND processed_at IS NULL FROM social.event_notification_intents WHERE id=$1")
            .bind(slow_intent).fetch_one(pool).await.unwrap();
        assert!(
            rotated,
            "claim rotation commits even when the delivery transaction rolls back"
        );
        dispatch_page(pool, "https://example.org", now)
            .await
            .unwrap();
        assert_eq!(
            count(pool, &fast, "event_approved", &address).await,
            1,
            "a blocked page cannot monopolize the queue"
        );
        let expired = Uuid::new_v4();
        sqlx::query("INSERT INTO social.event_notification_intents(id,event_id,kind,event,local_event,occurred_at,processed_at)
            VALUES($1,'expired','event_approved','{}',false,$2,$2)")
            .bind(expired).bind(now-Span::days(40)).execute(pool).await.unwrap();
        run_once(pool, "https://example.org", now).await.unwrap();
        let retained: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM social.event_notification_intents WHERE id=$1)",
        )
        .bind(expired)
        .fetch_one(pool)
        .await
        .unwrap();
        assert!(
            !retained,
            "cleanup runs even while a blocked pending intent exhausts the dispatch budget"
        );
        held.rollback().await.unwrap();
        let expired_now = now + Span::hours(2);
        let intent: Intent = sqlx::query_as("SELECT id,event_id,kind,event,local_event,occurrence_start,occurred_at,recipient_after FROM social.event_notification_intents WHERE id=$1")
            .bind(slow_intent).fetch_one(pool).await.unwrap();
        assert_eq!(
            suppression_reason(&intent, expired_now, false),
            Some("event_ended")
        );
        drain_until_complete(pool, expired_now, &slow, 1).await;
        assert_eq!(
            count(pool, &slow, "events_started", &address).await,
            0,
            "contention followed by expiry records suppression rather than delivering stale work"
        );

        let baseline: String = sqlx::query_scalar("SHOW statement_timeout")
            .fetch_one(pool)
            .await
            .unwrap();
        let mut tx = timed_transaction(pool).await.unwrap();
        let error = sqlx::query("SELECT pg_sleep(2)")
            .execute(&mut *tx)
            .await
            .unwrap_err();
        assert_eq!(
            error
                .as_database_error()
                .and_then(|db| db.code())
                .as_deref(),
            Some("57014")
        );
        tx.rollback().await.unwrap();
        let restored: String = sqlx::query_scalar("SHOW statement_timeout")
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(
            restored, baseline,
            "server timeouts do not leak into pooled callers"
        );

        // Delay only reminder inserts on this fixture. The independent start
        // scanner still commits while PostgreSQL cancels the slow reminder SQL.
        let reminder = format!("{prefix}-reminder");
        let started = format!("{prefix}-started");
        add_event(pool, &reminder, &address, now + Span::minutes(10)).await;
        add_event(pool, &started, &address, now - Span::minutes(10)).await;
        let function = format!("delay_reminder_{prefix}");
        let trigger = format!("delay_reminder_trigger_{prefix}");
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE FUNCTION social.{function}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.event_id='{reminder}' AND NEW.kind='events_starts_soon' THEN PERFORM pg_sleep(4); END IF; RETURN NEW; END $$")))
            .execute(pool).await.unwrap();
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE TRIGGER {trigger} BEFORE INSERT ON social.event_notification_intents FOR EACH ROW EXECUTE FUNCTION social.{function}()")))
            .execute(pool).await.unwrap();
        let result = run_once(pool, "https://example.org", now).await;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP TRIGGER {trigger} ON social.event_notification_intents"
        )))
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP FUNCTION social.{function}()"
        )))
        .execute(pool)
        .await
        .unwrap();
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("events_starts_soon occurrence scan")
        );
        assert_eq!(count(pool, &started, "events_started", &address).await, 1);
        assert_eq!(
            count(pool, &reminder, "events_starts_soon", &address).await,
            0
        );
        let from = (now - Span::minutes(5)).and_utc().timestamp_millis();
        run_once(pool, "https://example.org", now).await.unwrap();
        let store = super::super::NotificationsStore::new(pool.clone());
        let fresh = store
            .list(&address.parse().unwrap(), 100, Some(from), false)
            .await
            .unwrap();
        let reminder_notice = fresh
            .iter()
            .find(|item| item.metadata["eventId"] == reminder)
            .expect("incremental polling must see a newly delivered catch-up reminder");
        assert_eq!(reminder_notice.metadata["title"], "Event starts soon");
        assert_eq!(
            reminder_notice.metadata["occurredAt"],
            (now - Span::minutes(50)).and_utc().to_rfc3339()
        );

        // Community opt-outs use one canonical UUID representation even if a
        // legacy event stores its association with uppercase hex digits.
        let community = Uuid::new_v4();
        let community_event = format!("{prefix}-community");
        sqlx::query(
            "INSERT INTO social.communities(id,name,owner_address) VALUES($1,'Canonical',$2)",
        )
        .bind(community)
        .bind(&address)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO social.community_members(community_id,member_address,role) VALUES($1,$2,'member')").bind(community).bind(&address).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO social.subscription_opt_outs(address,scope,scope_id) VALUES($1,'community',$2)").bind(&address).bind(community.to_string().to_uppercase()).execute(pool).await.unwrap();
        let upper = community.to_string().to_uppercase();
        sqlx::query("INSERT INTO explore.event(id,name,approved,origin,community_id,start_at,finish_at,duration_ms,raw)
            VALUES($1,'Canonical',true,'local',$2,$3,$3+interval '1 hour',3600000,
            jsonb_build_object('name','Canonical','approved',true,'community_id',$2::text,'start_at',$3::timestamp,'duration',3600000))")
            .bind(&community_event).bind(&upper).bind(now-Span::minutes(5)).execute(pool).await.unwrap();
        scan_kind(pool, "events_started", 0, now).await.unwrap();
        let queued: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM social.event_notification_intents WHERE event_id=$1)",
        )
        .bind(&community_event)
        .fetch_one(pool)
        .await
        .unwrap();
        assert!(
            !queued,
            "canonical opt-out prevents noncanonical event IDs creating empty work"
        );
        // An already queued notice must also honor a legacy uppercase opt-out.
        sqlx::query("INSERT INTO social.event_notification_intents(id,event_id,kind,event,local_event,occurred_at) SELECT $1,id,'event_created',raw,true,$3 FROM explore.event WHERE id=$2")
            .bind(Uuid::new_v4()).bind(&community_event).bind(now).execute(pool).await.unwrap();
        drain_until_complete(pool, now, &community_event, 1).await;
        assert_eq!(
            count(pool, &community_event, "event_created", &address).await,
            0
        );
        let store = super::super::NotificationsStore::new(pool.clone());
        let parsed_address = address.parse().unwrap();
        assert!(
            store
                .is_opted_out(&parsed_address, "community", &community.to_string())
                .await
                .unwrap()
        );
        assert!(
            store
                .delete_opt_out(&parsed_address, "community", &community.to_string())
                .await
                .unwrap()
        );
        store
            .create_opt_out(
                &parsed_address,
                "community",
                &community.to_string().to_uppercase(),
            )
            .await
            .unwrap();
        let stored: String = sqlx::query_scalar("SELECT scope_id FROM social.subscription_opt_outs WHERE address=$1 AND scope='community'")
            .bind(&address).fetch_one(pool).await.unwrap();
        assert_eq!(stored, community.to_string());
        assert!(
            store
                .delete_opt_out(
                    &parsed_address,
                    "community",
                    &community.to_string().to_uppercase()
                )
                .await
                .unwrap()
        );
        // Ordinary maintenance failures must not block already queued delivery.
        let cleanup_event = format!("{prefix}-cleanup-error");
        let delivery_event = format!("{prefix}-cleanup-delivery");
        sqlx::query("INSERT INTO social.event_notification_intents(id,event_id,kind,event,local_event,occurred_at,processed_at) VALUES($1,$2,'event_approved','{}',false,$3,$3)")
            .bind(Uuid::new_v4()).bind(&cleanup_event).bind(now-Span::days(40)).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO social.event_notification_intents(id,event_id,kind,event,local_event,occurred_at) VALUES($1,$2,'event_approved',$3,false,$4)")
            .bind(Uuid::new_v4()).bind(&delivery_event).bind(json!({"name":"Cleanup delivery","user":address})).bind(now).execute(pool).await.unwrap();
        let function = format!("fail_cleanup_{prefix}");
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE FUNCTION social.{function}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF OLD.event_id='{cleanup_event}' THEN RAISE EXCEPTION 'test cleanup failure'; END IF; RETURN OLD; END $$"))).execute(pool).await.unwrap();
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE TRIGGER {function} BEFORE DELETE ON social.event_notification_intents FOR EACH ROW EXECUTE FUNCTION social.{function}()"))).execute(pool).await.unwrap();
        let result = run_once(pool, "https://example.org", now).await;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP TRIGGER {function} ON social.event_notification_intents"
        )))
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP FUNCTION social.{function}()"
        )))
        .execute(pool)
        .await
        .unwrap();
        result.unwrap();
        assert_eq!(
            count(pool, &delivery_event, "event_approved", &address).await,
            1
        );
        let retained: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM social.event_notification_intents WHERE event_id=$1)",
        )
        .bind(&cleanup_event)
        .fetch_one(pool)
        .await
        .unwrap();
        assert!(
            retained,
            "failed cleanup rolls back without preventing delivery"
        );
        // A pass-start horizon must not permit a notice that expires during work.
        let expired_reminder = format!("{prefix}-expired-during-pass");
        let event_start = now - Span::minutes(1);
        add_event(pool, &expired_reminder, &address, event_start).await;
        sqlx::query("INSERT INTO social.event_notification_intents(id,event_id,kind,event,local_event,occurrence_start,occurred_at) SELECT $1,id,'events_starts_soon',raw,false,$3,$3-interval '1 hour' FROM explore.event WHERE id=$2")
            .bind(Uuid::new_v4()).bind(&expired_reminder).bind(event_start).execute(pool).await.unwrap();
        drain_until_complete(pool, now - Span::minutes(2), &expired_reminder, 1).await;
        assert_eq!(
            count(pool, &expired_reminder, "events_starts_soon", &address).await,
            0
        );
    }
}
