//! Optional Slack moderation adapter. Authoring captures an outbox intent;
//! a bounded leased worker sends outside the transaction. HTTP retry is at
//! least once (Slack has no idempotency contract), with a six-attempt ceiling.

use std::time::Duration;

use chrono::{NaiveDateTime, Utc};
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

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
    // The caller holds the event row lock. Check the actual last minute as
    // well as the unique bucket, so crossing a minute boundary cannot send
    // two edit notices seconds apart. The bucket prefix bounds this probe.
    sqlx::query("WITH stamp AS (SELECT clock_timestamp() AT TIME ZONE 'UTC' AS at) \
        INSERT INTO explore.event_moderation_hooks (id,event_id,kind,event,edit_bucket,created_at) \
        SELECT $1,$2,$3,$4,CASE WHEN $3='edited' THEN date_trunc('minute',at) END,at FROM stamp \
        WHERE $3<>'edited' OR NOT EXISTS (SELECT 1 FROM explore.event_moderation_hooks h \
            WHERE h.event_id=$2 AND h.kind='edited' AND h.edit_bucket >= date_trunc('minute',at)-interval '1 minute' \
              AND h.created_at > at-interval '60 seconds') ON CONFLICT DO NOTHING")
        .bind(Uuid::new_v4()).bind(event_id).bind(kind).bind(Value::Object(event)).execute(&mut **tx).await?;
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

pub(super) async fn run_once(
    pool: &PgPool,
    client: &crate::http::Outbound,
    endpoint: &str,
    base: &str,
    now: NaiveDateTime,
) -> anyhow::Result<u64> {
    let activated=sqlx::query("INSERT INTO explore.event_moderation_hook_activation (activated_at) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(now).execute(pool).await?.rows_affected()>0;
    if activated {
        return Ok(0);
    }
    let mut sent = 0;
    for _ in 0..10 {
        let claim = Uuid::new_v4();
        let row:Option<(Uuid,String,String,Value,i32)>=sqlx::query_as(
            "WITH due AS (SELECT id FROM explore.event_moderation_hooks \
             WHERE completed_at IS NULL AND attempts<6 AND next_attempt_at<=$1 \
               AND created_at >= (SELECT activated_at FROM explore.event_moderation_hook_activation WHERE singleton) \
             ORDER BY next_attempt_at,id LIMIT 1 FOR UPDATE SKIP LOCKED) \
             UPDATE explore.event_moderation_hooks h SET claim=$2,attempts=attempts+1,next_attempt_at=$1+interval '1 minute' \
             FROM due WHERE h.id=due.id RETURNING h.id,h.event_id,h.kind,h.event,h.attempts")
            .bind(now).bind(claim).fetch_optional(pool).await?;
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
        sqlx::query("UPDATE explore.event_moderation_hooks SET completed_at=CASE WHEN $3 OR attempts>=6 THEN $4 ELSE NULL END,next_attempt_at=$4 + $5::int4*interval '1 second',claim=NULL WHERE id=$1 AND claim=$2")
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
    endpoint: String,
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
        assert_eq!(two, 2, "a new edit can notify after 60 seconds");
        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn moderation_hooks_skip_disabled_history_retry_boundedly_and_keep_private_fields_out() {
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
            run_once(&pool, &client, &endpoint, "https://example.org", now)
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
        sqlx::query("INSERT INTO explore.event_moderation_hooks (id,event_id,kind,event,created_at) VALUES ($1,'old-hook','submitted','{}',$2)")
            .bind(Uuid::new_v4()).bind(now-chrono::Duration::days(1)).execute(&pool).await.unwrap();
        assert_eq!(
            run_once(
                &pool,
                &client,
                &endpoint,
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
                &endpoint,
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
                &endpoint,
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
            "http://127.0.0.1:1/unreachable",
            "https://example.org",
            now + chrono::Duration::minutes(3),
        )
        .await
        .unwrap();
        assert_eq!(exhausted, 0);
        let pending:i64=sqlx::query_scalar("SELECT count(*) FROM explore.event_moderation_hooks WHERE event_id='hook-event' AND completed_at IS NULL").fetch_one(&pool).await.unwrap();
        assert_eq!(pending, 0);
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
