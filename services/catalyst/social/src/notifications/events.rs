//! Event notices over local RSVP consent and managed, local community
//! associations. The first lifecycle scan starts now; subsequent passes use
//! a durable (due time, event id) cursor. Explore captures moderation changes
//! atomically with the event. Dispatch pages commit their recipient cursor
//! with notification/email-outbox inserts, so crashes and retries are safe.

use std::time::Duration;

use chrono::{Duration as Span, NaiveDateTime, Utc};
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::store::{NewNotification, insert_notifications};
use crate::identity::Address;

const PAGE: i64 = 100;
const DISPATCH_PAGES: usize = 10;

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

async fn scan_kind(
    pool: &PgPool,
    kind: &str,
    ahead: i32,
    now: NaiveDateTime,
) -> anyhow::Result<u64> {
    let mut tx = pool.begin().await?;
    let initialized = sqlx::query("INSERT INTO social.event_notification_cursor (kind,due_at) VALUES ($1,$2) ON CONFLICT (kind) DO NOTHING")
        .bind(kind).bind(now).execute(&mut *tx).await?.rows_affected() > 0;
    if initialized {
        tx.commit().await?;
        return Ok(0);
    }
    let (since, event_id): (NaiveDateTime, String) = sqlx::query_as(
        "SELECT due_at,event_id FROM social.event_notification_cursor WHERE kind=$1 FOR UPDATE",
    )
    .bind(kind)
    .fetch_one(&mut *tx)
    .await?;
    if now < since {
        tx.commit().await?;
        return Ok(0);
    }
    // The guarded cast ignores malformed upstream recurrence strings rather
    // than letting one bad mirror row stall every other event's notices.
    let rows: Vec<Occurrence> = sqlx::query_as(
        "SELECT DISTINCT e.id AS event_id,e.raw - 'recurrent_dates' AS event,e.origin='local' AS local_event, \
                d.starts_at,d.starts_at - $3::int4 * interval '1 second' AS due_at \
         FROM explore.event e CROSS JOIN LATERAL ( \
             SELECT CASE WHEN pg_input_is_valid(value, 'timestamp with time zone') \
                    THEN value::timestamptz AT TIME ZONE 'UTC' END AS starts_at \
             FROM jsonb_array_elements_text(CASE WHEN jsonb_typeof(e.raw->'recurrent_dates')='array' \
                  AND e.raw->'recurrent_dates' <> '[]'::jsonb THEN e.raw->'recurrent_dates' \
                  ELSE jsonb_build_array(e.raw->'start_at') END) \
         ) d \
         WHERE e.approved AND COALESCE(e.raw->>'rejected','false') <> 'true' \
           AND COALESCE(e.raw->>'deleted_by_user','false') <> 'true' \
           AND COALESCE(e.raw->>'deleted_by_admin','false') <> 'true' \
           AND (d.starts_at - $3::int4 * interval '1 second',e.id) > ($1,$2) \
           AND d.starts_at - $3::int4 * interval '1 second' <= $4 \
         ORDER BY due_at,event_id LIMIT $5",
    ).bind(since).bind(&event_id).bind(ahead).bind(now).bind(PAGE).fetch_all(&mut *tx).await?;
    let ids: Vec<Uuid> = rows
        .iter()
        .map(|row| {
            crate::explore::events::notifications::occurrence_id(&row.event_id, row.starts_at, kind)
        })
        .collect();
    let event_ids: Vec<&str> = rows.iter().map(|row| row.event_id.as_str()).collect();
    let events: Vec<Value> = rows
        .iter()
        .map(|row| crate::explore::events::notifications::snapshot(&row.event))
        .collect();
    let local: Vec<bool> = rows.iter().map(|row| row.local_event).collect();
    let starts: Vec<NaiveDateTime> = rows.iter().map(|row| row.starts_at).collect();
    let due: Vec<NaiveDateTime> = rows.iter().map(|row| row.due_at).collect();
    sqlx::query(
        "INSERT INTO explore.event_notification_intents (id,event_id,event,local_event,occurrence_start,occurred_at,kind) \
         SELECT n.*,$7 FROM UNNEST($1::uuid[],$2::text[],$3::jsonb[],$4::boolean[],$5::timestamp[],$6::timestamp[]) n \
         ON CONFLICT (id) DO NOTHING",
    ).bind(ids).bind(event_ids).bind(events).bind(local).bind(starts).bind(due).bind(kind).execute(&mut *tx).await?;
    let (next_time, next_id) = if rows.len() < PAGE as usize {
        let id = rows
            .last()
            .filter(|last| last.due_at == now)
            .map(|last| last.event_id.clone())
            .unwrap_or_else(|| {
                if since == now {
                    event_id
                } else {
                    String::new()
                }
            });
        (now, id)
    } else {
        let last = rows.last().expect("a full page has a last row");
        (last.due_at, last.event_id.clone())
    };
    sqlx::query("UPDATE social.event_notification_cursor SET due_at=$2,event_id=$3 WHERE kind=$1")
        .bind(kind)
        .bind(next_time)
        .bind(next_id)
        .execute(&mut *tx)
        .await?;
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
    // A community scope opt-out affects the community audience. An explicit
    // RSVP remains independent consent and can still receive a generic notice.
    let rows = sqlx::query_as(
        "WITH candidates AS ( \
            SELECT $3::text AS address,false AS community,NULL::text AS community_name \
            WHERE $2 IN ('event_approved','event_rejected','event_deleted') \
            UNION ALL \
            SELECT a.signer,false,NULL FROM explore.event_attendance_local a JOIN explore.event e ON e.id=a.event_id \
            WHERE a.event_id=$1 AND a.action='going' AND $2 IN ('events_starts_soon','events_started') \
              AND e.approved AND COALESCE(e.raw->>'rejected','false') <> 'true' \
              AND COALESCE(e.raw->>'deleted_by_user','false') <> 'true' AND COALESCE(e.raw->>'deleted_by_admin','false') <> 'true' \
            UNION ALL \
            SELECT m.member_address,true,c.name FROM social.community_members m \
            JOIN social.communities c ON c.id=m.community_id \
            JOIN explore.event e ON e.id=$1 AND e.community_id=c.id::text AND e.origin='local' \
            WHERE $4 AND $2 IN ('event_created','events_started') AND c.id::text=$5 \
              AND c.origin='local' AND c.active AND NOT c.suspended AND e.approved \
              AND COALESCE(e.raw->>'rejected','false') <> 'true' \
              AND COALESCE(e.raw->>'deleted_by_user','false') <> 'true' AND COALESCE(e.raw->>'deleted_by_admin','false') <> 'true' \
              AND NOT EXISTS (SELECT 1 FROM social.subscription_opt_outs o \
                  WHERE o.address=m.member_address AND o.scope='community' AND o.scope_id=$5) \
        ) SELECT address,bool_or(community) AS community,max(community_name) AS community_name \
          FROM candidates WHERE address > $6 GROUP BY address ORDER BY address LIMIT $7",
    ).bind(&intent.event_id).bind(&intent.kind).bind(text(&intent.event,"user"))
        .bind(intent.local_event).bind(text(&intent.event,"community_id")).bind(&intent.recipient_after)
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
            "Event starts in an hour".to_string(),
            format!("The event {name} starts in an hour."),
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
    let mut metadata = json!({"eventId":intent.event_id,"title":title,"description":description,"name":name,"image":text(&intent.event,"image"),"link":link});
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

async fn dispatch_page(
    pool: &PgPool,
    base: &str,
    now: NaiveDateTime,
) -> anyhow::Result<Option<u64>> {
    let mut tx = pool.begin().await?;
    let intent: Option<Intent> = sqlx::query_as(
        "SELECT id,event_id,kind,event,local_event,occurrence_start,occurred_at,recipient_after \
         FROM explore.event_notification_intents WHERE processed_at IS NULL ORDER BY dispatch_at,id LIMIT 1 FOR UPDATE SKIP LOCKED",
    ).fetch_optional(&mut *tx).await?;
    let Some(intent) = intent else {
        tx.commit().await?;
        return Ok(None);
    };
    let stale_reminder = intent.kind == "events_starts_soon"
        && intent.occurrence_start.is_some_and(|start| start <= now);
    let recipients = if stale_reminder {
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
                occurred_at: intent.occurred_at,
            })
        })
        .collect();
    let inserted = insert_notifications(&mut *tx, &batch).await?;
    let finished = recipients.len() < PAGE as usize;
    sqlx::query("UPDATE explore.event_notification_intents SET recipient_after=$2,processed_at=CASE WHEN $3 THEN $4 ELSE NULL END,dispatch_at=$4 WHERE id=$1")
        .bind(intent.id).bind(recipients.last().map(|row| row.address.as_str()).unwrap_or(&intent.recipient_after))
        .bind(finished).bind(now).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Some(inserted))
}

/// One bounded pass. Clock input is explicit for rollover/restart tests.
pub async fn run_once(pool: &PgPool, base: &str, now: NaiveDateTime) -> anyhow::Result<u64> {
    scan_kind(pool, "events_starts_soon", 3600, now).await?;
    scan_kind(pool, "events_started", 0, now).await?;
    super::event_ended::run_once(pool, now).await?;
    let mut emitted = 0;
    for _ in 0..DISPATCH_PAGES {
        let Some(inserted) = dispatch_page(pool, base, now).await? else {
            break;
        };
        emitted += inserted;
    }
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
        let _guard = super::super::event_ended::TEST_LOCK.lock().await;
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
            "SELECT count(*) FROM explore.event_notification_intents WHERE event_id=$1",
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
            "SELECT count(*) FROM explore.event_notification_intents WHERE event_id=$1",
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
            "SELECT count(*) FROM explore.event_notification_intents WHERE event_id=$1",
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
        sqlx::query("INSERT INTO explore.event_notification_intents (id,event_id,kind,event,local_event,occurrence_start,occurred_at) SELECT $1,id,'events_started',raw,true,$3,$3 FROM explore.event WHERE id=$2")
            .bind(intent_id).bind(&paged).bind(t0).execute(&pool).await.unwrap();
        dispatch_page(&pool, base, t0).await.unwrap();
        let pending: bool = sqlx::query_scalar("SELECT processed_at IS NULL AND recipient_after <> '' FROM explore.event_notification_intents WHERE id=$1")
            .bind(intent_id).fetch_one(&pool).await.unwrap();
        assert!(pending, "a full page persists its resume key");
        let (left, right) = tokio::join!(
            dispatch_page(&pool, base, t0),
            dispatch_page(&pool, base, t0)
        );
        left.unwrap();
        right.unwrap();
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM social.notifications WHERE metadata->>'eventId'=$1 AND type='events_started'")
            .bind(&paged).fetch_one(&pool).await.unwrap();
        assert_eq!(count, 105);
        let finished: bool = sqlx::query_scalar(
            "SELECT processed_at IS NOT NULL FROM explore.event_notification_intents WHERE id=$1",
        )
        .bind(intent_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(finished);
    }
}
