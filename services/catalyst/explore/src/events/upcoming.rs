//! Optional aggregate operational summaries, one hour before a native event.
//! The queue contains no RSVP identities; current explicit consent is counted
//! again before delivery. First activation skips historical occurrences.

use std::time::Duration;

use chrono::{NaiveDateTime, Utc};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct Occurrence {
    event_id: String,
    starts_at: NaiveDateTime,
    due_at: NaiveDateTime,
    name: String,
}

async fn scan(pool: &PgPool, now: NaiveDateTime) -> anyhow::Result<u64> {
    let mut tx = pool.begin().await?;
    let initialized=sqlx::query("INSERT INTO explore.event_upcoming_hook_cursor (due_at) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(now).execute(&mut *tx).await?.rows_affected()>0;
    if initialized {
        tx.commit().await?;
        return Ok(0);
    }
    let (since,after_id,after_start):(NaiveDateTime,String,NaiveDateTime)=sqlx::query_as(
        "SELECT due_at,event_id,occurrence_start FROM explore.event_upcoming_hook_cursor WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx).await?;
    if now < since {
        tx.commit().await?;
        return Ok(0);
    }
    let rows:Vec<Occurrence>=sqlx::query_as(
        "SELECT DISTINCT e.id AS event_id,left(e.name,500) AS name,d.starts_at,d.starts_at-interval '1 hour' AS due_at \
         FROM explore.event e CROSS JOIN LATERAL ( \
            SELECT CASE WHEN pg_input_is_valid(value,'timestamp with time zone') \
                THEN value::timestamptz AT TIME ZONE 'UTC' END AS starts_at \
            FROM jsonb_array_elements_text(CASE WHEN jsonb_typeof(e.raw->'recurrent_dates')='array' \
                AND e.raw->'recurrent_dates'<>'[]'::jsonb THEN e.raw->'recurrent_dates' \
                ELSE jsonb_build_array(e.raw->'start_at') END) \
         ) d WHERE e.origin='local' AND e.approved \
            AND COALESCE(e.raw->>'rejected','false')<>'true' \
            AND COALESCE(e.raw->>'deleted_by_user','false')<>'true' \
            AND COALESCE(e.raw->>'deleted_by_admin','false')<>'true' \
            AND EXISTS (SELECT 1 FROM explore.event_attendance_local a WHERE a.event_id=e.id AND a.action='going') \
            AND (d.starts_at-interval '1 hour',e.id,d.starts_at)>($1,$2,$3) \
            AND d.starts_at-interval '1 hour'<=$4 \
         ORDER BY due_at,event_id,starts_at LIMIT 100")
        .bind(since).bind(&after_id).bind(after_start).bind(now).fetch_all(&mut *tx).await?;
    let event_ids: Vec<&str> = rows.iter().map(|row| row.event_id.as_str()).collect();
    let starts: Vec<NaiveDateTime> = rows.iter().map(|row| row.starts_at).collect();
    let names: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();
    let inserted=sqlx::query("INSERT INTO explore.event_upcoming_hooks (event_id,occurrence_start,name,next_attempt_at) SELECT n.*,$4 FROM UNNEST($1::text[],$2::timestamp[],$3::text[]) n ON CONFLICT DO NOTHING")
        .bind(event_ids).bind(starts).bind(names).bind(now).execute(&mut *tx).await?.rows_affected();
    if let Some(last) = rows.last() {
        sqlx::query("UPDATE explore.event_upcoming_hook_cursor SET due_at=$1,event_id=$2,occurrence_start=$3 WHERE singleton")
            .bind(last.due_at).bind(&last.event_id).bind(last.starts_at).execute(&mut *tx).await?;
    }
    if rows.len() < 100 && rows.last().is_none_or(|last| last.due_at < now) {
        sqlx::query("UPDATE explore.event_upcoming_hook_cursor SET due_at=$1,event_id='',occurrence_start='0001-01-01' WHERE singleton")
            .bind(now).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(inserted)
}

fn message(event_id: &str, name: &str, count: i64, base: &str) -> Value {
    json!({"blocks":[
        {"type":"section","text":{"type":"mrkdwn","text":format!("Event starts soon: <{}/whats-on/{}|View event>",base.trim_end_matches('/'),urlencoding::encode(event_id))}},
        {"type":"section","text":{"type":"plain_text","text":format!("Name: {name}\nEligible local RSVP reminders: {count}")}}
    ]})
}

pub(super) async fn run_once(
    pool: &PgPool,
    client: &crate::http::Outbound,
    endpoint: &str,
    base: &str,
    now: NaiveDateTime,
) -> anyhow::Result<u64> {
    scan(pool, now).await?;
    let mut sent = 0;
    for _ in 0..10 {
        let claim = Uuid::new_v4();
        let row:Option<(String,NaiveDateTime,String)>=sqlx::query_as(
            "WITH due AS (SELECT event_id,occurrence_start FROM explore.event_upcoming_hooks \
             WHERE completed_at IS NULL AND attempts<6 AND next_attempt_at<=$1 \
             ORDER BY next_attempt_at,event_id,occurrence_start LIMIT 1 FOR UPDATE SKIP LOCKED) \
             UPDATE explore.event_upcoming_hooks h SET claim=$2,attempts=attempts+1,next_attempt_at=$1+interval '1 minute' \
             FROM due WHERE h.event_id=due.event_id AND h.occurrence_start=due.occurrence_start \
             RETURNING h.event_id,h.occurrence_start,h.name")
            .bind(now).bind(claim).fetch_optional(pool).await?;
        let Some((event_id, start, name)) = row else {
            break;
        };
        let count:i64=sqlx::query_scalar(
            "SELECT count(*) FROM explore.event_attendance_local a JOIN explore.event e ON e.id=a.event_id \
             WHERE e.id=$1 AND a.action='going' AND e.origin='local' AND e.approved AND $2>$3 \
               AND COALESCE(e.raw->>'rejected','false')<>'true' \
               AND COALESCE(e.raw->>'deleted_by_user','false')<>'true' \
               AND COALESCE(e.raw->>'deleted_by_admin','false')<>'true' \
               AND EXISTS (SELECT 1 FROM jsonb_array_elements_text(CASE WHEN jsonb_typeof(e.raw->'recurrent_dates')='array' \
                 AND e.raw->'recurrent_dates'<>'[]'::jsonb THEN e.raw->'recurrent_dates' ELSE jsonb_build_array(e.raw->'start_at') END) d(value) \
                 WHERE pg_input_is_valid(value,'timestamp with time zone') AND \
                   CASE WHEN pg_input_is_valid(value,'timestamp with time zone') THEN value::timestamptz AT TIME ZONE 'UTC' END=$2)")
            .bind(&event_id).bind(start).bind(now).fetch_one(pool).await?;
        let success = count > 0
            && client
                .post(endpoint)
                .timeout(Duration::from_secs(5))
                .json(&message(&event_id, &name, count, base))
                .send()
                .await
                .is_ok_and(|response| response.status().is_success());
        sqlx::query("UPDATE explore.event_upcoming_hooks SET completed_at=CASE WHEN $4 OR attempts>=6 THEN $5 ELSE NULL END,next_attempt_at=$5+(30 * (1 << attempts)) * interval '1 second',claim=NULL WHERE event_id=$1 AND occurrence_start=$2 AND claim=$3")
            .bind(&event_id).bind(start).bind(claim).bind(success || count==0).bind(now).execute(pool).await?;
        if success {
            sent += 1
        } else if count > 0 {
            tracing::warn!(event_id, "upcoming event hook delivery failed");
        }
    }
    Ok(sent)
}

pub(crate) fn spawn(
    pool: PgPool,
    client: crate::http::Outbound,
    endpoint: String,
    base: String,
    jobs: &crate::jobs::Jobs,
) {
    jobs.spawn_periodic_leased(
        "event-upcoming-hook",
        crate::jobs::PeriodicCfg::every(Duration::from_secs(60)),
        move |_| {
            let (pool, client, endpoint, base) =
                (pool.clone(), client.clone(), endpoint.clone(), base.clone());
            async move {
                run_once(&pool, &client, &endpoint, &base, Utc::now().naive_utc()).await?;
                Ok(())
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[tokio::test]
    async fn upcoming_summaries_recover_per_occurrence_and_recheck_current_consent_without_private_payloads()
     {
        let Some(pool) = crate::db::scratch::pool(6).await else {
            return;
        };
        let now = chrono::DateTime::parse_from_rfc3339("2099-09-01T16:00:00Z")
            .unwrap()
            .naive_utc();
        assert_eq!(
            scan(&pool, now).await.unwrap(),
            0,
            "first activation skips historical catalogue work"
        );
        let start = now + chrono::Duration::hours(2);
        let next = start + chrono::Duration::days(1);
        let raw = json!({"start_at":start.and_utc().to_rfc3339(),"recurrent_dates":[start.and_utc().to_rfc3339(),next.and_utc().to_rfc3339()],"latest_attendees":["private-preview"],"total_attendees":100,"contact":"private contact","details":"private details"});
        for (id, origin) in [
            ("upcoming-native", "local"),
            ("upcoming-preview-only", "local"),
            ("upcoming-mirror", "upstream"),
        ] {
            sqlx::query("INSERT INTO explore.event (id,name,approved,origin,raw) VALUES ($1,'Public upcoming @channel',true,$2,$3)")
                .bind(id).bind(origin).bind(&raw).execute(&pool).await.unwrap();
        }
        sqlx::query("INSERT INTO explore.event_attendance_local (event_id,signer,action) VALUES ('upcoming-native','private-going','going'),('upcoming-native','private-cancel','cancel'),('upcoming-mirror','private-going','going')")
            .execute(&pool).await.unwrap();
        let due = start - chrono::Duration::hours(1);
        let (one, two) = tokio::join!(scan(&pool, due), scan(&pool, due));
        assert_eq!(
            one.unwrap() + two.unwrap(),
            1,
            "concurrent scans enqueue one native occurrence"
        );
        let calls = Arc::new(AtomicUsize::new(0));
        let payloads = Arc::new(tokio::sync::Mutex::new(Vec::<Value>::new()));
        let (captured_calls, captured_payloads) = (calls.clone(), payloads.clone());
        let app = axum::Router::new().route(
            "/hook",
            axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
                let (calls, payloads) = (captured_calls.clone(), captured_payloads.clone());
                async move {
                    payloads.lock().await.push(body);
                    if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                        axum::http::StatusCode::SERVICE_UNAVAILABLE
                    } else {
                        axum::http::StatusCode::OK
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/hook", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = crate::http::Outbound::for_tests();
        assert_eq!(
            run_once(&pool, &client, &endpoint, "https://example.org", due)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            run_once(
                &pool,
                &client,
                &endpoint,
                "https://example.org",
                due + chrono::Duration::seconds(1)
            )
            .await
            .unwrap(),
            0
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "failure waits for backoff rather than immediate retry"
        );
        assert_eq!(
            run_once(
                &pool,
                &client,
                &endpoint,
                "https://example.org",
                due + chrono::Duration::minutes(2)
            )
            .await
            .unwrap(),
            1
        );
        assert_eq!(
            run_once(
                &pool,
                &client,
                &endpoint,
                "https://example.org",
                due + chrono::Duration::minutes(3)
            )
            .await
            .unwrap(),
            0
        );
        let payloads = payloads.lock().await;
        for body in payloads.iter() {
            assert!(!body.to_string().contains("private"));
            assert_eq!(body["blocks"][1]["text"]["type"], "plain_text");
            assert!(
                body["blocks"][1]["text"]["text"]
                    .as_str()
                    .unwrap()
                    .contains("reminders: 1")
            );
        }
        drop(payloads);
        let tomorrow = due + chrono::Duration::days(1);
        assert_eq!(scan(&pool, tomorrow).await.unwrap(), 1);
        sqlx::query("UPDATE explore.event_attendance_local SET action='cancel' WHERE event_id='upcoming-native'").execute(&pool).await.unwrap();
        assert_eq!(
            run_once(&pool, &client, &endpoint, "https://example.org", tomorrow)
                .await
                .unwrap(),
            0,
            "cancelled RSVPs suppress a queued operational summary"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        server.abort();
    }
}
