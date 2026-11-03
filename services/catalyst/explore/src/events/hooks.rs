//! Optional Slack moderation adapter. Authoring captures an outbox intent;
//! a bounded leased worker sends outside the transaction. HTTP retry is at
//! least once (Slack has no idempotency contract), with a six-attempt ceiling.

use std::time::Duration;

use chrono::{NaiveDateTime, Utc};
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

const RETENTION_DAYS: i32 = 30;
const CLEANUP_LIMIT: i64 = 1000;

fn flag(raw: &Value, key: &str) -> bool {
    raw.get(key).and_then(Value::as_bool).unwrap_or(false)
}

pub(super) async fn record(
    tx: &mut Transaction<'_, Postgres>,
    event_id: &str,
    before: &Value,
    after: &Value,
    local: bool,
) -> Result<(), sqlx::Error> {
    let newly_deleted = (flag(after, "deleted_by_user") || flag(after, "deleted_by_admin"))
        && !flag(before, "deleted_by_user")
        && !flag(before, "deleted_by_admin");
    let kind = if newly_deleted {
        if !flag(after, "deleted_by_admin") {
            return Ok(());
        }
        "deleted"
    } else if before.is_null() && local {
        "submitted"
    } else if flag(after, "approved") && !flag(before, "approved") {
        "approved"
    } else if flag(after, "rejected") && !flag(before, "rejected") {
        "rejected"
    } else if local
        && [
            "name",
            "description",
            "image",
            "image_vertical",
            "start_at",
            "duration",
            "url",
            "world",
            "server",
            "x",
            "y",
            "categories",
            "schedules",
            "recurrent_dates",
            "community_id",
            "featured_item",
        ]
        .iter()
        .any(|key| before.get(*key) != after.get(*key))
    {
        "edited"
    } else {
        return Ok(());
    };
    let mut event = serde_json::Map::new();
    for key in ["name", "user_name", "rejection_reason", "deleted_reason"] {
        if let Some(value) = after.get(key).and_then(Value::as_str) {
            event.insert(
                key.into(),
                json!(value.chars().take(500).collect::<String>()),
            );
        }
    }
    // Use the same database clock for capture and activation, in the author's
    // transaction. The first leader tick cannot discard already queued work.
    sqlx::query("INSERT INTO explore.event_moderation_hook_activation(activated_at) VALUES(clock_timestamp() AT TIME ZONE 'UTC') ON CONFLICT DO NOTHING")
        .execute(&mut **tx).await?;
    // A concurrent author may have won activation while this INSERT waited.
    // Read that committed cutoff before sampling capture time, and floor the
    // sample against it so even a backward wall-clock adjustment loses no intent.
    let at: NaiveDateTime = sqlx::query_scalar("SELECT GREATEST(clock_timestamp() AT TIME ZONE 'UTC',activated_at) FROM explore.event_moderation_hook_activation WHERE singleton")
        .fetch_one(&mut **tx).await?;
    let creator = after
        .get("user")
        .and_then(Value::as_str)
        .unwrap_or(event_id)
        .to_ascii_lowercase();
    if kind == "edited" || kind == "submitted" {
        // Serialize per creator across different event row locks. Administrative
        // transitions never wait behind this throttle or lose their alert.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 7421))")
            .bind(&creator)
            .execute(&mut **tx)
            .await?;
        if kind == "edited" {
            let updated = sqlx::query("UPDATE explore.event_moderation_hooks SET event=$2 WHERE event_id=$1 AND kind='edited' AND completed_at IS NULL AND attempts=0")
                .bind(event_id).bind(Value::Object(event.clone())).execute(&mut **tx).await?.rows_affected();
            if updated > 0 {
                return Ok(());
            }
        }
    }
    sqlx::query("INSERT INTO explore.event_moderation_hooks (id,event_id,kind,event,creator_address,edit_bucket,created_at,next_attempt_at) \
        SELECT $1,$2,$3,$4,$5,CASE WHEN $3='edited' THEN date_trunc('minute',$6::timestamp) END,$6,$6 \
        WHERE ($3 NOT IN ('submitted','edited') OR (SELECT count(*) FROM explore.event_moderation_hooks \
            WHERE creator_address=$5 AND kind IN ('submitted','edited') AND created_at>$6-interval '1 minute')<2) \
        AND ($3<>'edited' OR NOT EXISTS (SELECT 1 FROM explore.event_moderation_hooks h \
            WHERE h.event_id=$2 AND h.kind='edited' AND h.edit_bucket>=date_trunc('minute',$6::timestamp)-interval '1 minute' \
              AND h.created_at>$6-interval '60 seconds')) ON CONFLICT DO NOTHING")
        .bind(Uuid::new_v4()).bind(event_id).bind(kind).bind(Value::Object(event))
        .bind(creator).bind(at).execute(&mut **tx).await?;
    Ok(())
}

fn message(event_id: &str, kind: &str, event: &Value, base: &str) -> Value {
    let route = if kind == "submitted" {
        format!(
            "/whats-on/admin/pending-events?id={}",
            urlencoding::encode(event_id)
        )
    } else {
        format!("/whats-on/{}", urlencoding::encode(event_id))
    };
    // Use plain_text for user content: names and reasons cannot inject Slack
    // mentions, links or mrkdwn. Only the fixed action/link uses mrkdwn.
    let text = ["name", "user_name", "rejection_reason", "deleted_reason"]
        .iter()
        .filter_map(|key| event.get(*key).and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    json!({"blocks":[
        {"type":"section","text":{"type":"mrkdwn","text":format!("Event {kind}: <{}{route}|Review event>",base.trim_end_matches('/'))}},
        {"type":"section","text":{"type":"plain_text","text":if text.is_empty(){"Decentraland event".to_string()}else{text}}}
    ]})
}

// Never remove deliverable retries. Expired terminal rows and history outside
// the activation boundary can be collected even after delivery is disabled.
async fn cleanup(pool: &PgPool, now: NaiveDateTime) -> anyhow::Result<u64> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL statement_timeout = '500ms'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL lock_timeout = '100ms'")
        .execute(&mut *tx)
        .await?;
    let removed = sqlx::query(
        "DELETE FROM explore.event_moderation_hooks h WHERE h.id IN ( \
            SELECT id FROM explore.event_moderation_hooks \
            WHERE created_at < $1 - $2::int4 * interval '1 day' \
              AND (completed_at < $1 - $2::int4 * interval '1 day' \
                OR (attempts >= 6 AND next_attempt_at < $1 - $2::int4 * interval '1 day') \
                OR created_at < COALESCE((SELECT activated_at FROM explore.event_moderation_hook_activation WHERE singleton),$1)) \
            ORDER BY created_at,id LIMIT $3 FOR UPDATE SKIP LOCKED)",
    ).bind(now).bind(RETENTION_DAYS).bind(CLEANUP_LIMIT).execute(&mut *tx).await?.rows_affected();
    tx.commit().await?;
    Ok(removed)
}

pub(super) async fn run_once(
    pool: &PgPool,
    client: &crate::http::Outbound,
    endpoint: Option<&str>,
    base: &str,
    now: NaiveDateTime,
) -> anyhow::Result<u64> {
    match tokio::time::timeout(Duration::from_secs(1), cleanup(pool, now)).await {
        Ok(Ok(_)) => {}
        Ok(Err(error)) => tracing::warn!(%error, "event moderation hook cleanup deferred"),
        Err(_) => tracing::warn!("event moderation hook cleanup exceeded budget"),
    }
    let Some(endpoint) = endpoint.filter(|endpoint| !endpoint.trim().is_empty()) else {
        return Ok(0);
    };
    if !sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM explore.event_moderation_hook_activation)",
    )
    .fetch_one(pool)
    .await?
    {
        return Ok(0);
    }
    let mut sent = 0;
    for _ in 0..10 {
        let claim = Uuid::new_v4();
        let mut tx = pool.begin().await?;
        sqlx::query("SET LOCAL statement_timeout='2s'")
            .execute(&mut *tx)
            .await?;
        sqlx::query("SET LOCAL lock_timeout='100ms'")
            .execute(&mut *tx)
            .await?;
        let row:Option<(Uuid,String,String,Value,i32)>=sqlx::query_as(
            "WITH due AS (SELECT id FROM explore.event_moderation_hooks \
             WHERE completed_at IS NULL AND attempts<6 AND next_attempt_at<=$1 \
               AND created_at >= (SELECT activated_at FROM explore.event_moderation_hook_activation WHERE singleton) \
             ORDER BY CASE WHEN kind='edited' THEN 1 ELSE 0 END,next_attempt_at,id LIMIT 1 FOR UPDATE SKIP LOCKED) \
             UPDATE explore.event_moderation_hooks h SET claim=$2,attempts=attempts+1,next_attempt_at=$1+interval '1 minute' \
             FROM due WHERE h.id=due.id RETURNING h.id,h.event_id,h.kind,h.event,h.attempts")
            .bind(now).bind(claim).fetch_optional(&mut *tx).await?;
        tx.commit().await?;
        let Some((id, event_id, kind, event, attempts)) = row else {
            break;
        };
        let success = client
            .post(endpoint)
            .timeout(Duration::from_secs(5))
            .json(&message(&event_id, &kind, &event, base))
            .send()
            .await
            .is_ok_and(|response| response.status().is_success());
        sqlx::query("UPDATE explore.event_moderation_hooks SET completed_at=CASE WHEN $3 THEN $4 ELSE NULL END,next_attempt_at=$4 + $5::int4*interval '1 second',claim=NULL WHERE id=$1 AND claim=$2")
            .bind(id).bind(claim).bind(success).bind(now).bind(30_i32 * (1 << attempts.min(6))).execute(pool).await?;
        if success {
            sent += 1
        } else {
            tracing::warn!(%id,attempts,"event moderation hook delivery failed");
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
        "event-moderation-hook",
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
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[tokio::test]
    async fn concurrent_first_captures_and_backward_clock_samples_never_precede_activation() {
        let _guard = TEST_LOCK.lock().await;
        let Some(pool) = crate::db::scratch::pool(6).await else {
            return;
        };
        sqlx::query("DELETE FROM explore.event_moderation_hook_activation")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("CREATE FUNCTION explore.test_hook_capture_barrier() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF current_setting('application_name')='hook-capture-race' THEN PERFORM pg_advisory_xact_lock(742119); END IF; RETURN NEW; END $$").execute(&pool).await.unwrap();
        sqlx::query("CREATE TRIGGER test_hook_capture_barrier BEFORE INSERT ON explore.event_moderation_hook_activation FOR EACH ROW EXECUTE FUNCTION explore.test_hook_capture_barrier()").execute(&pool).await.unwrap();
        let prefix = Uuid::new_v4().to_string();
        let mut held = pool.acquire().await.unwrap();
        sqlx::query("SELECT pg_advisory_lock(742119)")
            .execute(&mut *held)
            .await
            .unwrap();
        let (ready, waiting) = tokio::sync::oneshot::channel();
        let clone = pool.clone();
        let slow_id = format!("{prefix}-slow");
        let slow = tokio::spawn(async move {
            let mut tx = clone.begin().await.unwrap();
            sqlx::query("SET LOCAL application_name='hook-capture-race'")
                .execute(&mut *tx)
                .await
                .unwrap();
            let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
                .fetch_one(&mut *tx)
                .await
                .unwrap();
            ready.send(pid).unwrap();
            record(
                &mut tx,
                &slow_id,
                &Value::Null,
                &json!({"name":"Slow first capture"}),
                true,
            )
            .await
            .unwrap();
            tx.commit().await.unwrap();
        });
        let pid = waiting.await.unwrap();
        tokio::time::timeout(Duration::from_secs(3),async {
            loop {
                let blocked:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid=$1 AND wait_event='advisory')")
                    .bind(pid).fetch_one(&pool).await.unwrap();
                if blocked {break;}
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        record(
            &mut tx,
            &format!("{prefix}-fast"),
            &Value::Null,
            &json!({"name":"Concurrent activation winner"}),
            true,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        sqlx::query("SELECT pg_advisory_unlock(742119)")
            .execute(&mut *held)
            .await
            .unwrap();
        slow.await.unwrap();
        let concurrent_eligible:bool=sqlx::query_scalar("SELECT bool_and(h.created_at>=a.activated_at) FROM explore.event_moderation_hooks h CROSS JOIN explore.event_moderation_hook_activation a WHERE h.event_id LIKE $1")
            .bind(format!("{prefix}-%")).fetch_one(&pool).await.unwrap();
        // A future cutoff simulates the wall clock stepping backward after activation.
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("UPDATE explore.event_moderation_hook_activation SET activated_at=(clock_timestamp() AT TIME ZONE 'UTC')+interval '1 minute'")
            .execute(&mut *tx).await.unwrap();
        record(
            &mut tx,
            &format!("{prefix}-clock"),
            &Value::Null,
            &json!({"name":"Backward clock capture"}),
            true,
        )
        .await
        .unwrap();
        let clock_eligible:bool=sqlx::query_scalar("SELECT h.created_at=a.activated_at FROM explore.event_moderation_hooks h CROSS JOIN explore.event_moderation_hook_activation a WHERE h.event_id=$1")
            .bind(format!("{prefix}-clock")).fetch_one(&mut *tx).await.unwrap();
        tx.rollback().await.unwrap();
        sqlx::query(
            "DROP TRIGGER test_hook_capture_barrier ON explore.event_moderation_hook_activation",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("DROP FUNCTION explore.test_hook_capture_barrier()")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM explore.event_moderation_hooks WHERE event_id LIKE $1")
            .bind(format!("{prefix}-%"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM explore.event_moderation_hook_activation")
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            concurrent_eligible,
            "a delayed first writer must not fall behind another writer's activation"
        );
        assert!(
            clock_eligible,
            "capture timestamps must never fall behind the durable cutoff"
        );
    }

    #[tokio::test]
    async fn capture_activates_with_the_database_clock_before_the_first_worker_tick() {
        let _guard = TEST_LOCK.lock().await;
        let Some(pool) = crate::db::scratch::pool(6).await else {
            return;
        };
        sqlx::query("DELETE FROM explore.event_moderation_hook_activation")
            .execute(&pool)
            .await
            .unwrap();
        let id = Uuid::new_v4().to_string();
        let mut tx = pool.begin().await.unwrap();
        record(
            &mut tx,
            &id,
            &Value::Null,
            &json!({"name":"First captured alert"}),
            true,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        let (created,activated):(NaiveDateTime,NaiveDateTime)=sqlx::query_as("SELECT h.created_at,a.activated_at FROM explore.event_moderation_hooks h CROSS JOIN explore.event_moderation_hook_activation a WHERE h.event_id=$1")
            .bind(&id).fetch_one(&pool).await.unwrap();
        assert!(created >= activated);
        run_once(
            &pool,
            &crate::http::Outbound::for_tests(),
            Some("http://127.0.0.1:1/unreachable"),
            "https://example.org",
            Utc::now().naive_utc() + chrono::Duration::minutes(1),
        )
        .await
        .unwrap();
        let (attempts, completed): (i32, Option<NaiveDateTime>) = sqlx::query_as(
            "SELECT attempts,completed_at FROM explore.event_moderation_hooks WHERE event_id=$1",
        )
        .bind(&id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            attempts, 1,
            "first captured intent is not discarded on leader activation"
        );
        assert_eq!(completed, None);
        sqlx::query("DELETE FROM explore.event_moderation_hooks WHERE event_id=$1")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM explore.event_moderation_hook_activation")
            .execute(&pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn moderation_alerts_are_delivered_before_an_edit_backlog() {
        let _guard = TEST_LOCK.lock().await;
        let Some(pool) = crate::db::scratch::pool(6).await else {
            return;
        };
        let captured = Arc::new(tokio::sync::Mutex::new(Vec::<Value>::new()));
        let copy = captured.clone();
        let app = axum::Router::new().route(
            "/hook",
            axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
                let copy = copy.clone();
                async move {
                    copy.lock().await.push(body);
                    axum::http::StatusCode::NO_CONTENT
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/hook", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let prefix = Uuid::new_v4().to_string();
        let mut tx = pool.begin().await.unwrap();
        for index in 0..12 {
            record(
                &mut tx,
                &format!("{prefix}-{index}"),
                &json!({"name":"Before"}),
                &json!({"name":"Edited"}),
                true,
            )
            .await
            .unwrap();
        }
        record(
            &mut tx,
            &format!("{prefix}-submitted"),
            &Value::Null,
            &json!({"name":"New submission"}),
            true,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(
            run_once(
                &pool,
                &crate::http::Outbound::for_tests(),
                Some(&endpoint),
                "https://example.org",
                Utc::now().naive_utc() + chrono::Duration::seconds(1)
            )
            .await
            .unwrap(),
            10
        );
        let payloads = captured.lock().await;
        assert!(
            payloads[0]["blocks"][0]["text"]["text"]
                .as_str()
                .unwrap()
                .starts_with("Event submitted:")
        );
        assert_eq!(payloads[0]["blocks"][1]["text"]["text"], "New submission");
        sqlx::query("DELETE FROM explore.event_moderation_hooks WHERE event_id LIKE $1")
            .bind(format!("{prefix}-%"))
            .execute(&pool)
            .await
            .unwrap();
        server.abort();
    }

    #[tokio::test]
    async fn creator_throttle_and_edit_coalescing_bound_non_admin_alerts() {
        let Some(pool) = crate::db::scratch::pool(6).await else {
            return;
        };
        let actor = Uuid::new_v4().to_string();
        let mut tx = pool.begin().await.unwrap();
        for i in 0..5 {
            record(
                &mut tx,
                &format!("{actor}-{i}"),
                &Value::Null,
                &json!({"name":"Submission","user":actor}),
                true,
            )
            .await
            .unwrap();
        }
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM explore.event_moderation_hooks WHERE creator_address=$1",
        )
        .bind(&actor)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        assert_eq!(
            count, 2,
            "a creator cannot flood the queue with distinct events"
        );
        let id = format!("{actor}-admin");
        record(
            &mut tx,
            &id,
            &json!({"approved":false}),
            &json!({"name":"Moderated","user":actor,"approved":true}),
            true,
        )
        .await
        .unwrap();
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM explore.event_moderation_hooks WHERE creator_address=$1",
        )
        .bind(&actor)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        assert_eq!(count, 3, "operator transitions bypass user throttles");
        let other = Uuid::new_v4().to_string();
        let before = json!({"name":"Before","user":other});
        record(
            &mut tx,
            &other,
            &before,
            &json!({"name":"Edit one","user":other}),
            true,
        )
        .await
        .unwrap();
        record(
            &mut tx,
            &other,
            &before,
            &json!({"name":"Edit two","user":other}),
            true,
        )
        .await
        .unwrap();
        let names: Vec<String> = sqlx::query_scalar(
            "SELECT event->>'name' FROM explore.event_moderation_hooks WHERE event_id=$1",
        )
        .bind(&other)
        .fetch_all(&mut *tx)
        .await
        .unwrap();
        assert_eq!(
            names,
            vec!["Edit two"],
            "an unsent edit retains the latest public labels"
        );
        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn disabled_hook_capture_keeps_the_optional_outbox_empty() {
        let Some(pool) = crate::db::scratch::pool(6).await else {
            return;
        };
        let mut tx = pool.begin().await.unwrap();
        let prefix = Uuid::new_v4().to_string();
        let mut settings = crate::Settings::for_tests();
        for (index, (writes, endpoint)) in [
            (false, Some("http://127.0.0.1/hook")),
            (true, None),
            (true, Some("  ")),
            (true, Some("http://127.0.0.1/hook")),
        ]
        .into_iter()
        .enumerate()
        {
            settings.operator_writes_enabled = writes;
            settings.events_slack_webhook = endpoint.map(str::to_owned);
            super::super::notifications::record_transition(
                &mut tx,
                &format!("{prefix}-{index}"),
                None,
                &json!({"name":"Public event"}),
                true,
                &settings,
            )
            .await
            .unwrap();
        }
        let captured: Vec<String> = sqlx::query_scalar(
            "SELECT event_id FROM explore.event_moderation_hooks WHERE event_id LIKE $1",
        )
        .bind(format!("{prefix}-%"))
        .fetch_all(&mut *tx)
        .await
        .unwrap();
        assert_eq!(captured, vec![format!("{prefix}-3")]);
        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn disabled_delivery_cleans_expired_history_in_bounded_unlocked_batches_and_retains_retries()
     {
        let _guard = TEST_LOCK.lock().await;
        let Some(pool) = crate::db::scratch::pool(6).await else {
            return;
        };
        let now = Utc::now().naive_utc();
        let prefix = Uuid::new_v4().to_string();
        // This integration test owns the activation singleton while holding TEST_LOCK.
        sqlx::query("DELETE FROM explore.event_moderation_hook_activation")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO explore.event_moderation_hook_activation(activated_at) VALUES($1-interval '50 days')").bind(now).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO explore.event_moderation_hooks(id,event_id,kind,event,created_at,completed_at) SELECT gen_random_uuid(),$1||'-completed','approved','{}',$2-interval '40 days',$2-interval '31 days' FROM generate_series(1,$3)")
            .bind(&prefix).bind(now).bind(CLEANUP_LIMIT+5).execute(&pool).await.unwrap();
        for (suffix, age, completed_age, attempts, retry_age) in [
            ("before-activation", 60, None, 0, 60),
            ("exhausted", 40, None, 6, 31),
            ("retry", 40, None, 2, 31),
            ("recent-completion", 40, Some(1), 1, 1),
            ("recent-edit", 0, Some(0), 1, 0),
            ("locked", 70, Some(31), 1, 31),
        ] {
            sqlx::query("INSERT INTO explore.event_moderation_hooks(id,event_id,kind,event,created_at,completed_at,attempts,next_attempt_at) VALUES($1,$2,'edited','{}',$3-$4::int4*interval '1 day',CASE WHEN $5::int4 IS NOT NULL THEN $3-$5::int4*interval '1 day' END,$6,$3-$7::int4*interval '1 day')")
                .bind(Uuid::new_v4()).bind(format!("{prefix}-{suffix}")).bind(now).bind(age).bind(completed_age).bind(attempts).bind(retry_age).execute(&pool).await.unwrap();
        }
        let mut held = pool.begin().await.unwrap();
        sqlx::query("SELECT id FROM explore.event_moderation_hooks WHERE event_id=$1 FOR UPDATE")
            .bind(format!("{prefix}-locked"))
            .fetch_one(&mut *held)
            .await
            .unwrap();
        let before: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM explore.event_moderation_hooks WHERE event_id LIKE $1",
        )
        .bind(format!("{prefix}-%"))
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            run_once(
                &pool,
                &crate::http::Outbound::for_tests(),
                None,
                "https://example.org",
                now
            )
            .await
            .unwrap(),
            0
        );
        let after: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM explore.event_moderation_hooks WHERE event_id LIKE $1",
        )
        .bind(format!("{prefix}-%"))
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(before - after, CLEANUP_LIMIT);
        assert_eq!(cleanup(&pool, now).await.unwrap(), 7);
        let retained:Vec<String> = sqlx::query_scalar("SELECT event_id FROM explore.event_moderation_hooks WHERE event_id LIKE $1 ORDER BY event_id").bind(format!("{prefix}-%")).fetch_all(&pool).await.unwrap();
        assert_eq!(
            retained,
            ["locked", "recent-completion", "recent-edit", "retry"]
                .map(|suffix| format!("{prefix}-{suffix}"))
        );
        held.rollback().await.unwrap();
        assert_eq!(cleanup(&pool, now).await.unwrap(), 1);
        sqlx::query("DELETE FROM explore.event_moderation_hooks WHERE event_id LIKE $1")
            .bind(format!("{prefix}-%"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM explore.event_moderation_hook_activation")
            .execute(&pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn edit_throttle_covers_the_rolling_minute_across_bucket_boundaries() {
        let Some(pool) = crate::db::scratch::pool(6).await else {
            return;
        };
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("INSERT INTO explore.event_moderation_hooks (id,event_id,kind,event,edit_bucket,created_at) \
            VALUES ($1,'rolling-hook','edited','{}',date_trunc('minute',now() AT TIME ZONE 'UTC')-interval '1 minute', \
                (now() AT TIME ZONE 'UTC')-interval '30 seconds')")
            .bind(Uuid::new_v4()).execute(&mut *tx).await.unwrap();
        let before = json!({"name":"Before"});
        let after = json!({"name":"After"});
        record(&mut tx, "rolling-hook", &before, &after, true)
            .await
            .unwrap();
        let one: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM explore.event_moderation_hooks WHERE event_id='rolling-hook'",
        )
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        assert_eq!(
            one, 1,
            "last bucket still suppresses an edit within 60 seconds"
        );
        sqlx::query("UPDATE explore.event_moderation_hooks SET edit_bucket=edit_bucket-interval '1 minute',created_at=created_at-interval '1 minute' WHERE event_id='rolling-hook'")
            .execute(&mut *tx).await.unwrap();
        record(&mut tx, "rolling-hook", &before, &after, true)
            .await
            .unwrap();
        let two: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM explore.event_moderation_hooks WHERE event_id='rolling-hook'",
        )
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        assert_eq!(
            two, 1,
            "an unsent edit is coalesced even after the throttle window"
        );
        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn moderation_hooks_skip_disabled_history_retry_boundedly_and_keep_private_fields_out() {
        let _guard = TEST_LOCK.lock().await;
        let Some(pool) = crate::db::scratch::pool(6).await else {
            return;
        };
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
        let now = chrono::DateTime::parse_from_rfc3339("2099-08-01T18:00:00Z")
            .unwrap()
            .naive_utc();
        let client = crate::http::Outbound::for_tests();
        assert_eq!(
            run_once(&pool, &client, Some(&endpoint), "https://example.org", now)
                .await
                .unwrap(),
            0
        );
        let mut tx = pool.begin().await.unwrap();
        let raw = json!({"name":"Public @channel <attack>","contact":"secret contact","details":"secret details","user_name":"Host"});
        record(&mut tx, "hook-event", &Value::Null, &raw, true)
            .await
            .unwrap();
        let mut edited = raw.clone();
        edited["name"] = json!("New public name");
        record(&mut tx, "hook-event", &raw, &edited, true)
            .await
            .unwrap();
        record(&mut tx, "hook-event", &raw, &edited, true)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        sqlx::query("UPDATE explore.event_moderation_hooks SET created_at=$1,next_attempt_at=$1 WHERE event_id='hook-event'")
            .bind(now+chrono::Duration::seconds(1)).execute(&pool).await.unwrap();
        let queued: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM explore.event_moderation_hooks WHERE event_id='hook-event'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(queued, 2, "owner edits are throttled once per event/minute");
        // Any pre-existing disabled backlog remains behind the activation cutoff.
        sqlx::query("INSERT INTO explore.event_moderation_hooks (id,event_id,kind,event,created_at) VALUES ($1,'old-hook','submitted','{}',(SELECT activated_at-interval '1 day' FROM explore.event_moderation_hook_activation WHERE singleton))")
            .bind(Uuid::new_v4()).execute(&pool).await.unwrap();
        assert_eq!(
            run_once(
                &pool,
                &client,
                Some(&endpoint),
                "https://example.org",
                now + chrono::Duration::seconds(1)
            )
            .await
            .unwrap(),
            1
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "only two current records, including failed first delivery"
        );
        assert_eq!(
            run_once(
                &pool,
                &client,
                Some(&endpoint),
                "https://example.org",
                now + chrono::Duration::seconds(2)
            )
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
                now + chrono::Duration::minutes(2)
            )
            .await
            .unwrap(),
            1
        );
        for payload in payloads.lock().await.iter() {
            assert!(!payload.to_string().contains("secret"));
            assert_eq!(payload["blocks"][1]["text"]["type"], "plain_text");
        }
        // Exhaustion is durable; the worker never retries a sixth failed attempt.
        sqlx::query("UPDATE explore.event_moderation_hooks SET completed_at=NULL,attempts=5,next_attempt_at=$1 WHERE event_id='hook-event'")
            .bind(now).execute(&pool).await.unwrap();
        let exhausted = run_once(
            &pool,
            &client,
            Some("http://127.0.0.1:1/unreachable"),
            "https://example.org",
            now + chrono::Duration::minutes(3),
        )
        .await
        .unwrap();
        assert_eq!(exhausted, 0);
        let pending:i64=sqlx::query_scalar("SELECT count(*) FROM explore.event_moderation_hooks WHERE event_id='hook-event' AND completed_at IS NULL").fetch_one(&pool).await.unwrap();
        assert_eq!(
            pending, 2,
            "exhausted failures are distinct from successful completion"
        );
        let mut tx = pool.begin().await.unwrap();
        let mut state = raw.clone();
        record(&mut tx, "hook-lifecycle", &Value::Null, &state, true)
            .await
            .unwrap();
        for (key, value) in [
            ("approved", true),
            ("rejected", true),
            ("deleted_by_admin", true),
        ] {
            let before = state.clone();
            state[key] = json!(value);
            record(&mut tx, "hook-lifecycle", &before, &state, true)
                .await
                .unwrap();
        }
        let kinds:Vec<String>=sqlx::query_scalar("SELECT kind FROM explore.event_moderation_hooks WHERE event_id='hook-lifecycle' ORDER BY kind")
            .fetch_all(&mut *tx).await.unwrap();
        assert_eq!(kinds, vec!["approved", "deleted", "rejected", "submitted"]);
        tx.rollback().await.unwrap();
        let rolled_back: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM explore.event_moderation_hooks WHERE event_id='hook-lifecycle'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            rolled_back, 0,
            "integration intents share authoring transaction rollback"
        );
        server.abort();
    }
}
