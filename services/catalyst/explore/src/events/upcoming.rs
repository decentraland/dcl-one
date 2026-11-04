//! Optional aggregate summaries for native occurrences starting within an hour.
//! Rolling scans recover late eligibility; unique retained keys deduplicate work.
//! HTTP delivery is at least once because Slack provides no idempotency contract.

use std::time::Duration;

use chrono::{NaiveDateTime, Utc};
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

const RETENTION_DAYS: i32 = 30;
const CLEANUP_LIMIT: i64 = 1000;

async fn timed_transaction(pool: &PgPool) -> Result<Transaction<'_, Postgres>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL statement_timeout='2s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL lock_timeout='100ms'")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}

// Reuse the public event list's finite, validated recurrence dates. Native
// nonrecurring events use their typed UTC start when no series dates exist.
fn occurrences_sql() -> String {
    format!(
        "SELECT dates.starts_at AT TIME ZONE 'UTC' AS starts_at FROM ({}) dates \
        UNION SELECT e.start_at WHERE e.start_at IS NOT NULL AND isfinite(e.start_at) \
        AND CASE WHEN jsonb_typeof(e.raw->'recurrent_dates')='array' \
            THEN jsonb_array_length(e.raw->'recurrent_dates')=0 ELSE true END",
        super::VALID_RECURRENCE_DATES_SQL
    )
}

async fn scan(pool: &PgPool, now: NaiveDateTime) -> anyhow::Result<u64> {
    let mut tx = timed_transaction(pool).await?;
    // Typed bounds use event_notifications_live_idx before expanding the series.
    // NOT EXISTS is before LIMIT, so a second tick reaches the next 100 rows,
    // including when every occurrence shares the same start time.
    let sql = format!(
        "INSERT INTO explore.event_upcoming_hooks (event_id,occurrence_start,name,next_attempt_at) \
        SELECT DISTINCT e.id,d.starts_at,'',$1 FROM explore.event e \
        CROSS JOIN LATERAL ({}) d \
        WHERE e.origin='local' AND e.approved AND e.finish_at>$1 \
          AND e.start_at<=$1+interval '1 hour' \
          AND COALESCE(e.raw->>'rejected','false')<>'true' \
          AND COALESCE(e.raw->>'deleted_by_user','false')<>'true' \
          AND COALESCE(e.raw->>'deleted_by_admin','false')<>'true' \
          AND d.starts_at>$1 AND d.starts_at<=$1+interval '1 hour' \
          AND EXISTS (SELECT 1 FROM explore.event_attendance_local a WHERE a.event_id=e.id AND a.action='going') \
          AND NOT EXISTS (SELECT 1 FROM explore.event_upcoming_hooks h WHERE h.event_id=e.id AND h.occurrence_start=d.starts_at) \
        ORDER BY d.starts_at,e.id LIMIT 100 ON CONFLICT DO NOTHING",
        occurrences_sql()
    );
    let inserted = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(now)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    tx.commit().await?;
    Ok(inserted)
}

async fn cleanup(pool: &PgPool, now: NaiveDateTime) -> anyhow::Result<u64> {
    let mut tx = timed_transaction(pool).await?;
    sqlx::query("SET LOCAL statement_timeout='500ms'")
        .execute(&mut *tx)
        .await?;
    // Expired occurrences cannot be delivered, whether completed, dead-lettered,
    // or abandoned during downtime. Retain all of them for thirty days.
    let removed = sqlx::query(
        "DELETE FROM explore.event_upcoming_hooks h USING ( \
        SELECT event_id,occurrence_start FROM explore.event_upcoming_hooks \
        WHERE occurrence_start<$1-$2::int4*interval '1 day' \
        ORDER BY occurrence_start,event_id LIMIT $3 FOR UPDATE SKIP LOCKED) old \
        WHERE h.event_id=old.event_id AND h.occurrence_start=old.occurrence_start",
    )
    .bind(now)
    .bind(RETENTION_DAYS)
    .bind(CLEANUP_LIMIT)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    tx.commit().await?;
    Ok(removed)
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
    endpoint: Option<&str>,
    base: &str,
    now: NaiveDateTime,
) -> anyhow::Result<u64> {
    // Keep the supplied wall clock testable while advancing it through SQL
    // waits and earlier webhook calls in this batch.
    let started = tokio::time::Instant::now();
    let current_time =
        || chrono::Duration::from_std(started.elapsed()).map(|elapsed| now + elapsed);
    match tokio::time::timeout(Duration::from_secs(1), cleanup(pool, now)).await {
        Ok(Ok(_)) => {}
        Ok(Err(error)) => tracing::warn!(%error, "upcoming hook cleanup deferred"),
        Err(_) => tracing::warn!("upcoming hook cleanup exceeded budget"),
    }
    let Some(endpoint) = endpoint.filter(|value| !value.trim().is_empty()) else {
        return Ok(0);
    };
    // A slow scan must not prevent already queued retries from being delivered.
    match tokio::time::timeout(Duration::from_secs(3), scan(pool, now)).await {
        Ok(Ok(_)) => {}
        Ok(Err(error)) => tracing::warn!(%error, "upcoming hook scan deferred"),
        Err(_) => tracing::warn!("upcoming hook scan exceeded budget"),
    }
    let mut sent = 0;
    for _ in 0..10 {
        let claim = Uuid::new_v4();
        let mut tx = timed_transaction(pool).await?;
        let row:Option<(String,NaiveDateTime)>=sqlx::query_as(
            "WITH due AS (SELECT event_id,occurrence_start FROM explore.event_upcoming_hooks \
             WHERE completed_at IS NULL AND attempts<6 AND next_attempt_at<=$1 AND occurrence_start>$1 AND occurrence_start<=$1+interval '1 hour' \
             ORDER BY next_attempt_at,event_id,occurrence_start LIMIT 1 FOR UPDATE SKIP LOCKED) \
             UPDATE explore.event_upcoming_hooks h SET claim=$2,attempts=attempts+1,next_attempt_at=$1+interval '1 minute' \
             FROM due WHERE h.event_id=due.event_id AND h.occurrence_start=due.occurrence_start \
             RETURNING h.event_id,h.occurrence_start")
            .bind(current_time()?).bind(claim).fetch_optional(&mut *tx).await?;
        let Some((event_id, start)) = row else {
            break;
        };
        let sql = format!(
            "SELECT left(e.name,500),(SELECT count(*) FROM explore.event_attendance_local a \
            WHERE a.event_id=e.id AND a.action='going') FROM explore.event e \
            WHERE e.id=$1 AND e.origin='local' AND e.approved AND $2>$3 \
               AND COALESCE(e.raw->>'rejected','false')<>'true' \
               AND COALESCE(e.raw->>'deleted_by_user','false')<>'true' \
               AND COALESCE(e.raw->>'deleted_by_admin','false')<>'true' \
               AND EXISTS (SELECT 1 FROM ({}) d WHERE d.starts_at=$2)",
            occurrences_sql()
        );
        let current: Option<(String, i64)> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(&event_id)
            .bind(start)
            .bind(current_time()?)
            .fetch_optional(&mut *tx)
            .await?;
        tx.commit().await?;
        let expired = start <= current_time()?;
        let eligible = !expired && current.as_ref().is_some_and(|(_, count)| *count > 0);
        let success = if eligible && let Some((name, count)) = current {
            client
                .post(endpoint)
                .timeout(Duration::from_secs(5))
                .json(&message(&event_id, &name, count, base))
                .send()
                .await
                .is_ok_and(|response| response.status().is_success())
        } else {
            false
        };
        let mut tx = timed_transaction(pool).await?;
        if eligible || expired {
            // Only delivery marks completion. Exhausted or expired unfinished
            // rows remain observable until retention removes them.
            sqlx::query("UPDATE explore.event_upcoming_hooks SET completed_at=CASE WHEN $4 THEN $5 ELSE NULL END,next_attempt_at=$5+(30 * (1 << attempts)) * interval '1 second',claim=NULL WHERE event_id=$1 AND occurrence_start=$2 AND claim=$3")
                .bind(&event_id).bind(start).bind(claim).bind(success).bind(current_time()?).execute(&mut *tx).await?;
        } else {
            // A temporary loss of approval or RSVPs is not delivery. A future
            // scan can capture this key again if it becomes eligible in time.
            sqlx::query("DELETE FROM explore.event_upcoming_hooks WHERE event_id=$1 AND occurrence_start=$2 AND claim=$3")
                .bind(&event_id).bind(start).bind(claim).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        if success {
            sent += 1
        } else if eligible {
            tracing::warn!(event_id, "upcoming event hook delivery failed");
        }
    }
    Ok(sent)
}

pub(crate) fn spawn(
    pool: PgPool,
    client: crate::http::Outbound,
    endpoint: Option<String>,
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
                run_once(
                    &pool,
                    &client,
                    endpoint.as_deref(),
                    &base,
                    Utc::now().naive_utc(),
                )
                .await?;
                Ok(())
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[tokio::test]
    async fn upcoming_summaries_recover_per_occurrence_and_recheck_current_consent_without_private_payloads()
     {
        let _guard = TEST_LOCK.lock().await;
        let Some(pool) = crate::db::scratch::pool(6).await else {
            return;
        };
        let now = chrono::DateTime::parse_from_rfc3339("2099-09-01T16:00:00Z")
            .unwrap()
            .naive_utc();
        let start = now + chrono::Duration::hours(2);
        let next = start + chrono::Duration::days(1);
        let raw = json!({"start_at":start.and_utc().to_rfc3339(),"recurrent_dates":[start.and_utc().to_rfc3339(),next.and_utc().to_rfc3339()],"latest_attendees":["private-preview"],"total_attendees":100,"contact":"private contact","details":"private details"});
        for (id, origin) in [
            ("upcoming-native", "local"),
            ("upcoming-preview-only", "local"),
            ("upcoming-mirror", "upstream"),
        ] {
            sqlx::query("INSERT INTO explore.event (id,name,approved,origin,raw,start_at,finish_at) VALUES ($1,'Public upcoming @channel',true,$2,$3,$4,$5)")
                .bind(id).bind(origin).bind(&raw).bind(start).bind(next+chrono::Duration::hours(1)).execute(&pool).await.unwrap();
        }
        sqlx::query("INSERT INTO explore.event_attendance_local (event_id,signer,action) VALUES ('upcoming-native','private-going','going'),('upcoming-native','private-cancel','cancel'),('upcoming-mirror','private-going','going')")
            .execute(&pool).await.unwrap();
        let due = start - chrono::Duration::hours(1);
        let (one, two) = tokio::join!(scan(&pool, due), scan(&pool, due));
        let mut inserted = 0;
        for outcome in [one, two] {
            match outcome {
                Ok(count) => inserted += count,
                Err(error) => {
                    let Some(sqlx::Error::Database(database)) = error.downcast_ref::<sqlx::Error>()
                    else {
                        panic!("unexpected scan failure: {error:#}");
                    };
                    assert_eq!(database.code().as_deref(), Some("55P03"), "{error:#}");
                }
            }
        }
        assert_eq!(
            inserted, 1,
            "concurrent scans enqueue one native occurrence"
        );
        assert_eq!(
            scan(&pool, due).await.unwrap(),
            0,
            "a tick after bounded contention deduplicates the committed intent"
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
            run_once(&pool, &client, Some(&endpoint), "https://example.org", due)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            run_once(
                &pool,
                &client,
                Some(&endpoint),
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
        // Fail only the INSERT scan. The already queued retry must still send.
        let scan_failure = seed(&pool, due, 30, true, true).await;
        let fail_sql = format!(
            "CREATE FUNCTION explore.test_upcoming_scan_failure() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.event_id='{}' THEN RAISE EXCEPTION 'scan fixture fault'; END IF; RETURN NEW; END $$; CREATE TRIGGER test_upcoming_scan_failure BEFORE INSERT ON explore.event_upcoming_hooks FOR EACH ROW EXECUTE FUNCTION explore.test_upcoming_scan_failure();",
            scan_failure
        );
        sqlx::raw_sql(sqlx::AssertSqlSafe(fail_sql))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "UPDATE explore.event SET name='Renamed before retry' WHERE id='upcoming-native'",
        )
        .execute(&pool)
        .await
        .unwrap();
        assert_eq!(
            run_once(
                &pool,
                &client,
                Some(&endpoint),
                "https://example.org",
                due + chrono::Duration::minutes(2)
            )
            .await
            .unwrap(),
            1
        );
        sqlx::raw_sql("DROP TRIGGER test_upcoming_scan_failure ON explore.event_upcoming_hooks; DROP FUNCTION explore.test_upcoming_scan_failure();").execute(&pool).await.unwrap();
        sqlx::query("DELETE FROM explore.event WHERE id=$1")
            .bind(&scan_failure)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            run_once(
                &pool,
                &client,
                Some(&endpoint),
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
        assert!(
            payloads.last().unwrap()["blocks"][1]["text"]["text"]
                .as_str()
                .unwrap()
                .contains("Renamed before retry")
        );
        drop(payloads);
        let tomorrow = due + chrono::Duration::days(1);
        assert_eq!(scan(&pool, tomorrow).await.unwrap(), 1);
        sqlx::query("UPDATE explore.event_attendance_local SET action='cancel' WHERE event_id='upcoming-native'").execute(&pool).await.unwrap();
        assert_eq!(
            run_once(
                &pool,
                &client,
                Some(&endpoint),
                "https://example.org",
                tomorrow
            )
            .await
            .unwrap(),
            0,
            "cancelled RSVPs suppress a queued operational summary"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        server.abort();
    }
    async fn seed(
        pool: &PgPool,
        now: NaiveDateTime,
        minutes: i64,
        approved: bool,
        going: bool,
    ) -> String {
        let id = Uuid::new_v4().to_string();
        let start = now + chrono::Duration::minutes(minutes);
        sqlx::query("INSERT INTO explore.event(id,name,approved,origin,raw,start_at,finish_at) VALUES($1,'Current name', $2,'local',$3,$4,$5)")
            .bind(&id).bind(approved).bind(json!({"start_at":start.and_utc().to_rfc3339(),"recurrent_dates":[start.and_utc().to_rfc3339()]}))
            .bind(start).bind(start+chrono::Duration::hours(1)).execute(pool).await.unwrap();
        if going {
            rsvp(pool, &id).await;
        }
        id
    }

    async fn rsvp(pool: &PgPool, id: &str) {
        sqlx::query("INSERT INTO explore.event_attendance_local(event_id,signer,action) VALUES($1,'going-wallet','going')")
            .bind(id).execute(pool).await.unwrap();
    }

    fn clock(date: &str) -> NaiveDateTime {
        chrono::DateTime::parse_from_rfc3339(date)
            .unwrap()
            .naive_utc()
    }

    #[tokio::test]
    async fn rolling_window_includes_late_eligibility_and_skips_existing_history_and_invalid_dates()
    {
        let _guard = TEST_LOCK.lock().await;
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let now = clock("2055-01-01T12:00:00Z");
        seed(&pool, now, -5, true, true).await;
        seed(&pool, now, 120, true, true).await;
        let approval = seed(&pool, now, 30, false, true).await;
        let going = seed(&pool, now, 40, true, false).await;
        let invalid = seed(&pool, now, 45, true, true).await;
        sqlx::query("UPDATE explore.event SET raw=jsonb_set(raw,'{recurrent_dates}','[\"infinity\",\"-infinity\",\"bad\"]') WHERE id=$1")
            .bind(&invalid).execute(&pool).await.unwrap();
        assert_eq!(
            scan(&pool, now).await.unwrap(),
            0,
            "first activation skips existing past events"
        );
        sqlx::query("UPDATE explore.event SET approved=true WHERE id=$1")
            .bind(&approval)
            .execute(&pool)
            .await
            .unwrap();
        rsvp(&pool, &going).await;
        assert_eq!(
            scan(&pool, now + chrono::Duration::minutes(10))
                .await
                .unwrap(),
            2
        );
        assert_eq!(
            scan(&pool, now + chrono::Duration::minutes(11))
                .await
                .unwrap(),
            0
        );
        let recent = seed(&pool, now, 15, true, true).await;
        assert_eq!(
            scan(&pool, now + chrono::Duration::minutes(12))
                .await
                .unwrap(),
            1,
            "new events less than an hour ahead are included"
        );
        let start = now + chrono::Duration::minutes(25);
        sqlx::query("UPDATE explore.event SET start_at=$2,finish_at=$2+interval '1 hour',raw=$3 WHERE id=$1")
            .bind(&recent).bind(start).bind(json!({"start_at":start.and_utc().to_rfc3339(),"recurrent_dates":[]}))
            .execute(&pool).await.unwrap();
        assert_eq!(
            scan(&pool, now + chrono::Duration::minutes(20))
                .await
                .unwrap(),
            1,
            "rescheduled occurrences use their new identity"
        );
        assert_eq!(
            scan(&pool, now + chrono::Duration::days(1)).await.unwrap(),
            0,
            "downtime never replays expired occurrences"
        );
    }

    #[tokio::test]
    async fn tied_due_times_page_past_retained_keys_without_losing_late_additions() {
        let _guard = TEST_LOCK.lock().await;
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let now = clock("2056-01-01T12:00:00Z");
        for _ in 0..205 {
            seed(&pool, now, 30, true, true).await;
        }
        assert_eq!(scan(&pool, now).await.unwrap(), 100);
        assert_eq!(scan(&pool, now).await.unwrap(), 100);
        assert_eq!(scan(&pool, now).await.unwrap(), 5);
        seed(&pool, now, 30, true, true).await;
        assert_eq!(scan(&pool, now).await.unwrap(), 1);
        assert_eq!(scan(&pool, now).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn a_slow_webhook_does_not_dispatch_a_later_occurrence_after_it_starts() {
        let _guard = TEST_LOCK.lock().await;
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let now = clock("2059-01-01T12:00:00Z");
        let first = seed(&pool, now, 40, true, true).await;
        let expires = seed(&pool, now, 1, true, true).await;
        let start = now + chrono::Duration::seconds(1);
        sqlx::query("UPDATE explore.event SET start_at=$2,finish_at=$2+interval '1 hour',raw=$3 WHERE id=$1")
            .bind(&expires).bind(start).bind(json!({"start_at":start.and_utc().to_rfc3339(),"recurrent_dates":[]}))
            .execute(&pool).await.unwrap();
        assert_eq!(scan(&pool, now).await.unwrap(), 2);
        // Claim order is deliberate: the slow request must precede the
        // occurrence that will expire while its response is held.
        sqlx::query("UPDATE explore.event_upcoming_hooks SET next_attempt_at=$2-interval '1 second' WHERE event_id=$1")
            .bind(&first).bind(now).execute(&pool).await.unwrap();
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let calls = Arc::new(AtomicUsize::new(0));
        let (request_entered, request_release, request_calls) =
            (entered.clone(), release.clone(), calls.clone());
        let app = axum::Router::new().route(
            "/hook",
            axum::routing::post(move || {
                let (entered, release, calls) = (
                    request_entered.clone(),
                    request_release.clone(),
                    request_calls.clone(),
                );
                async move {
                    if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                        entered.notify_one();
                        release.notified().await;
                    }
                    axum::http::StatusCode::OK
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/hook", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let worker_pool = pool.clone();
        let worker = tokio::spawn(async move {
            run_once(
                &worker_pool,
                &crate::http::Outbound::for_tests(),
                Some(&endpoint),
                "https://example.org",
                now,
            )
            .await
            .unwrap()
        });
        tokio::time::timeout(Duration::from_secs(2), entered.notified())
            .await
            .unwrap();
        // Pause only after the PostgreSQL work has reached the local HTTP
        // fixture, then resume before any further database I/O.
        tokio::time::pause();
        tokio::time::advance(Duration::from_secs(2)).await;
        tokio::time::resume();
        release.notify_one();
        assert_eq!(worker.await.unwrap(), 1);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "the expired occurrence is never dispatched"
        );
        let state: (i32, Option<NaiveDateTime>) = sqlx::query_as(
            "SELECT attempts,completed_at FROM explore.event_upcoming_hooks WHERE event_id=$1",
        )
        .bind(&expires)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            state,
            (0, None),
            "expired unclaimed work remains observable as a dead letter"
        );
        server.abort();
    }

    #[tokio::test]
    async fn temporarily_ineligible_occurrences_recover_without_resending_delivered_summaries() {
        let _guard = TEST_LOCK.lock().await;
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let now = clock("2058-01-01T12:00:00Z");
        let cancelled = seed(&pool, now, 40, true, true).await;
        let unapproved = seed(&pool, now, 45, true, true).await;
        assert_eq!(scan(&pool, now).await.unwrap(), 2);
        sqlx::query("UPDATE explore.event_attendance_local SET action='cancel' WHERE event_id=$1")
            .bind(&cancelled)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE explore.event SET approved=false WHERE id=$1")
            .bind(&unapproved)
            .execute(&pool)
            .await
            .unwrap();

        let calls = Arc::new(AtomicUsize::new(0));
        let captured = calls.clone();
        let app = axum::Router::new().route(
            "/hook",
            axum::routing::post(move || {
                let calls = captured.clone();
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    axum::http::StatusCode::OK
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/hook", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = crate::http::Outbound::for_tests();
        assert_eq!(
            run_once(&pool, &client, Some(&endpoint), "https://example.org", now)
                .await
                .unwrap(),
            0
        );
        let queued: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM explore.event_upcoming_hooks WHERE event_id=ANY($1)",
        )
        .bind(vec![&cancelled, &unapproved])
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            queued, 0,
            "suppression must not leave a permanently deduplicated completion"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        sqlx::query("UPDATE explore.event_attendance_local SET action='going' WHERE event_id=$1")
            .bind(&cancelled)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE explore.event SET approved=true WHERE id=$1")
            .bind(&unapproved)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            run_once(
                &pool,
                &client,
                Some(&endpoint),
                "https://example.org",
                now + chrono::Duration::minutes(1)
            )
            .await
            .unwrap(),
            2
        );
        assert_eq!(
            run_once(
                &pool,
                &client,
                Some(&endpoint),
                "https://example.org",
                now + chrono::Duration::minutes(2)
            )
            .await
            .unwrap(),
            0
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "delivered keys still suppress repeated ticks"
        );
        server.abort();
    }

    #[tokio::test]
    async fn failures_remain_dead_letters_and_disabled_delivery_still_cleans_expired_history() {
        let _guard = TEST_LOCK.lock().await;
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let now = clock("2057-01-01T12:00:00Z");
        let failed = seed(&pool, now, 55, true, true).await;
        let expired = seed(&pool, now, 20, true, true).await;
        let rejected = seed(&pool, now, 40, true, true).await;
        let deleted = seed(&pool, now, 45, true, true).await;
        assert_eq!(scan(&pool, now).await.unwrap(), 4);
        sqlx::query("UPDATE explore.event SET raw=raw||'{\"rejected\":true}' WHERE id=$1")
            .bind(&rejected)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE explore.event SET raw=raw||'{\"deleted_by_admin\":true}' WHERE id=$1")
            .bind(&deleted)
            .execute(&pool)
            .await
            .unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let captured = calls.clone();
        let app = axum::Router::new().route(
            "/hook",
            axum::routing::post(move || {
                let calls = captured.clone();
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    axum::http::StatusCode::SERVICE_UNAVAILABLE
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/hook", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = crate::http::Outbound::for_tests();
        let mut at = now;
        for _ in 0..6 {
            assert_eq!(
                run_once(&pool, &client, Some(&endpoint), "https://example.org", at)
                    .await
                    .unwrap(),
                0
            );
            at = sqlx::query_scalar(
                "SELECT next_attempt_at FROM explore.event_upcoming_hooks WHERE event_id=$1",
            )
            .bind(&failed)
            .fetch_one(&pool)
            .await
            .unwrap();
            // Each item settles against its own advancing clock. Move beyond
            // the tick's small per-item offset so both failures are due.
            at += chrono::Duration::seconds(1);
        }
        assert_eq!(
            calls.load(Ordering::SeqCst),
            11,
            "rejected and deleted events send nothing"
        );
        let state: (i32, Option<NaiveDateTime>) = sqlx::query_as(
            "SELECT attempts,completed_at FROM explore.event_upcoming_hooks WHERE event_id=$1",
        )
        .bind(&failed)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(state, (6, None));
        let state: (i32, Option<NaiveDateTime>) = sqlx::query_as(
            "SELECT attempts,completed_at FROM explore.event_upcoming_hooks WHERE event_id=$1",
        )
        .bind(&expired)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(state, (5, None), "a deadline can precede the sixth attempt");
        let dead_letters: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM explore.event_upcoming_hooks WHERE event_id=ANY($1) \
            AND completed_at IS NULL AND (attempts>=6 OR occurrence_start<=$2)",
        )
        .bind(vec![&failed, &expired])
        .bind(at)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            dead_letters, 2,
            "the operational dead-letter predicate includes expired unfinished work"
        );
        run_once(&pool, &client, Some(&endpoint), "https://example.org", at)
            .await
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 11);
        // Opt-out runs bounded retention, including pending rows abandoned during downtime.
        let old = now - chrono::Duration::days(31);
        sqlx::query("INSERT INTO explore.event_upcoming_hooks(event_id,occurrence_start,name) SELECT 'expired-'||n,$1,'legacy' FROM generate_series(1,1005) n")
            .bind(old).execute(&pool).await.unwrap();
        assert_eq!(cleanup(&pool, now).await.unwrap(), 1000);
        let disabled = seed(&pool, now, 50, true, true).await;
        run_once(&pool, &client, Some("   "), "https://example.org", now)
            .await
            .unwrap();
        assert!(
            !sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM explore.event_upcoming_hooks WHERE event_id=$1)"
            )
            .bind(&disabled)
            .fetch_one(&pool)
            .await
            .unwrap(),
            "blank endpoints do not scan new work"
        );
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM explore.event_upcoming_hooks WHERE event_id LIKE 'expired-%'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 0);
        assert!(
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM explore.event_upcoming_hooks WHERE event_id=$1)"
            )
            .bind(&failed)
            .fetch_one(&pool)
            .await
            .unwrap(),
            "retention keeps recent dead letters"
        );
        server.abort();
    }
}
