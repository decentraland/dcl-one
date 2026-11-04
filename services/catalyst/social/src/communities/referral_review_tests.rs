use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::mpsc;

/// A drainer intentionally scans every pending grant. Each drainer fixture
/// therefore needs its own database so it cannot issue another test's rewards.
struct RewardDatabase {
    pool: sqlx::PgPool,
    admin: sqlx::PgPool,
    name: String,
}

async fn reward_database() -> Option<RewardDatabase> {
    let Ok(url) = std::env::var("CATALYST_TEST_DATABASE_URL") else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return None;
    };
    let base: sqlx::postgres::PgConnectOptions = url.parse().expect("PostgreSQL test URL");
    let admin = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with(base.clone().database("postgres"))
        .await
        .unwrap();
    let name = format!("catalyst_test_referral_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
        .execute(&admin)
        .await
        .unwrap();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(12)
        .connect_with(base.database(&name).options([("TimeZone", "UTC")]))
        .await
        .unwrap();
    crate::db::apply_schema(&pool).await.unwrap();
    Some(RewardDatabase { pool, admin, name })
}

impl RewardDatabase {
    async fn teardown(self) {
        self.pool.close().await;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE \"{}\" WITH (FORCE)",
            self.name
        )))
        .execute(&self.admin)
        .await
        .unwrap();
        self.admin.close().await;
    }
}

fn wallet() -> Address {
    Address::parse(&format!("0x{}00000000", uuid::Uuid::new_v4().simple())).unwrap()
}
async fn repeatable_read_writers(pool: &sqlx::PgPool) -> sqlx::PgPool {
    let writers = sqlx::postgres::PgPoolOptions::new()
        .max_connections(12)
        .after_connect(|connection, _| {
            Box::pin(async move {
                sqlx::query(
                    "SET SESSION CHARACTERISTICS AS TRANSACTION ISOLATION LEVEL REPEATABLE READ",
                )
                .execute(connection)
                .await?;
                Ok(())
            })
        })
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap();
    let isolation: String = sqlx::query_scalar("SHOW default_transaction_isolation")
        .fetch_one(&writers)
        .await
        .unwrap();
    assert_eq!(isolation, "repeatable read");
    writers
}
async fn advisory_key(pool: &sqlx::PgPool, name: &str) -> i64 {
    sqlx::query_scalar("SELECT hashtextextended($1,0)")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap()
}
async fn wait_for_locks(pool: &sqlx::PgPool, keys: &[i64], expected: i64) {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let count:i64=sqlx::query_scalar("SELECT count(*) FROM pg_locks WHERE locktype='advisory' AND NOT granted AND ((classid::bigint << 32) | objid::bigint)=ANY($1)").bind(keys).fetch_one(pool).await.unwrap();
            if count == expected { break; }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    }).await.expect("concurrent operations reached their transaction reservations");
}

#[tokio::test]
async fn concurrent_ip_creates_enforce_the_cap_and_keep_rejected_retry_receipts() {
    let Some(pool) = crate::db::scratch::pool(12).await else {
        return;
    };
    let referral = Arc::new(Referral::new(
        repeatable_read_writers(&pool).await,
        Outbound::for_tests(),
        Settings {
            suspicious_window_ms: 0.0,
            ..Settings::for_tests()
        },
    ));
    let referrer = wallet();
    let invited = (0..8).map(|_| wallet()).collect::<Vec<_>>();
    let gate = advisory_key(&pool, &uuid::Uuid::new_v4().to_string()).await;
    let quota = advisory_key(&pool, &format!("social-referral-ip:{referrer}:192.0.2.1")).await;
    let function = format!("referral_ip_gate_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE FUNCTION social.{function}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.referrer='{referrer}' THEN PERFORM pg_advisory_xact_lock({gate}); END IF; RETURN NEW; END $$"))).execute(&pool).await.unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE TRIGGER {function} BEFORE INSERT ON social.referral_progress FOR EACH ROW EXECUTE FUNCTION social.{function}()"))).execute(&pool).await.unwrap();
    let mut held = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(gate)
        .execute(&mut *held)
        .await
        .unwrap();
    let mut tasks = tokio::task::JoinSet::new();
    for invited in invited.iter().cloned() {
        let referral = referral.clone();
        let referrer = referrer.clone();
        tasks.spawn(async move { referral.create(&referrer, &invited, "192.0.2.1").await });
    }
    wait_for_locks(&pool, &[gate, quota], 8).await;
    held.commit().await.unwrap();
    let mut accepted = 0;
    while let Some(result) = tasks.join_next().await {
        accepted += usize::from(result.unwrap().is_ok());
    }
    assert_eq!(accepted, 2);
    let statuses:Vec<(String,String)>=sqlx::query_as("SELECT invited_user,status FROM social.referral_progress WHERE referrer=$1 ORDER BY invited_user").bind(&referrer).fetch_all(&pool).await.unwrap();
    assert_eq!(statuses.len(), 8);
    assert_eq!(statuses.iter().filter(|row| row.1 == "pending").count(), 2);
    assert_eq!(
        statuses
            .iter()
            .filter(|row| row.1 == "rejected_ip_match")
            .count(),
        6
    );
    for (invited, status) in &statuses {
        if status == "rejected_ip_match" {
            referral
                .create(&referrer, &Address::parse(invited).unwrap(), "192.0.2.1")
                .await
                .unwrap();
        }
    }
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP TRIGGER {function} ON social.referral_progress"
    )))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP FUNCTION social.{function}()"
    )))
    .execute(&pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn concurrent_referrer_grants_observe_distinct_totals_and_release_sql_before_rewards() {
    let Some(pool) = crate::db::scratch::pool(12).await else {
        return;
    };
    let referrer = wallet();
    let invited = (0..8).map(|_| wallet()).collect::<Vec<_>>();
    let calls = Arc::new(AtomicUsize::new(0));
    let requests = calls.clone();
    let observed_pool = pool.clone();
    let observed_referrer = referrer.clone();
    let app=axum::Router::new().route("/api/rewards",axum::routing::post(move || {let calls=requests.clone();let pool=observed_pool.clone();let referrer=observed_referrer.clone();async move {
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                let available:bool=sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended($1,0))").bind(format!("social-referral-tier:{referrer}")).fetch_one(&pool).await.unwrap();
                if available { break; }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        }).await.expect("no transaction reservation spans the external reward request");
        calls.fetch_add(1,Ordering::SeqCst);
        Json(json!({"data":[{"image":"https://reward.example.org/tier.png","rarity":"rare"}]}))
    }}));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let referral = Arc::new(Referral::new(
        repeatable_read_writers(&pool).await,
        Outbound::for_tests(),
        Settings {
            min_login_days: 0.0,
            max_ip_matches: 100.0,
            suspicious_window_ms: 0.0,
            reward_server_url: Some(url),
            ..Settings::for_tests()
        },
    ));
    sqlx::query("INSERT INTO social.referral_progress(referrer,invited_user,status,tier_granted) SELECT $1,'0x'||replace(gen_random_uuid()::text,'-','')||'00000000','tier_granted',true FROM generate_series(1,4)").bind(&referrer).execute(&pool).await.unwrap();
    for invited in &invited {
        referral
            .create(&referrer, invited, "192.0.2.1")
            .await
            .unwrap();
    }
    let key = advisory_key(&pool, &format!("social-referral-tier:{referrer}")).await;
    let mut held = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(key)
        .execute(&mut *held)
        .await
        .unwrap();
    let mut tasks = tokio::task::JoinSet::new();
    for invited in invited {
        let referral = referral.clone();
        tasks.spawn(async move { referral.finalize_referral(invited.as_str()).await });
    }
    wait_for_locks(&pool, &[key], 8).await;
    let before: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM social.referral_progress WHERE referrer=$1 AND tier_granted",
    )
    .bind(&referrer)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(before, 4);
    held.commit().await.unwrap();
    while let Some(result) = tasks.join_next().await {
        result.unwrap().unwrap();
    }
    let totals:Vec<i64>=sqlx::query_scalar("SELECT (metadata->>'invitedUsers')::bigint FROM social.notifications WHERE address=$1 AND type='referral_invited_users_accepted' ORDER BY (metadata->>'invitedUsers')::bigint").bind(&referrer).fetch_all(&pool).await.unwrap();
    assert_eq!(totals, (5..=12).collect::<Vec<_>>());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    let images: Vec<i32> = sqlx::query_scalar(
        "SELECT tier FROM social.referral_reward_images WHERE referrer=$1 ORDER BY tier",
    )
    .bind(&referrer)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(images, [5, 10]);
    server.abort();
}

fn source_failure(scenario: &str) -> Value {
    let source: Value = serde_json::from_str(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/conformance/social/domain-boundary-source-cases.json"
    )))
    .unwrap();
    source["failures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["scenario"] == scenario)
        .unwrap()
        .clone()
}

#[tokio::test]
async fn independent_tier_image_and_sns_work_survive_the_other_operations_early_failure() {
    for image_first in [false, true] {
        let Some(pool) = crate::db::scratch::pool(8).await else {
            return;
        };
        let referrer = wallet();
        let invited = wallet();
        let source = source_failure(if image_first {
            "image-failure-first"
        } else {
            "tier-sns-failure"
        });
        let (entered, mut entries) = mpsc::unbounded_channel();
        let release = Arc::new(tokio::sync::Notify::new());
        let gate = release.clone();
        let seen = Arc::new(AtomicUsize::new(0));
        let completed = seen.clone();
        let app=axum::Router::new().route("/api/rewards",axum::routing::post(||async {Json(json!({"data":[{"image":"https://reward.example.org/tier.png","rarity":"rare"}]}))}))
            .fallback(move |body:axum::body::Bytes| {let entered=entered.clone();let release=gate.clone();let seen=completed.clone();async move {
                let fields=url::form_urlencoded::parse(&body).into_owned().collect::<std::collections::HashMap<_,_>>();let event:Value=serde_json::from_str(&fields["Message"]).unwrap();
                if event["subType"]=="referral-new-tier-reached" {entered.send(()).unwrap();release.notified().await;if !image_first{return (StatusCode::BAD_REQUEST,[("content-type","text/xml")],"<ErrorResponse><Error><Type>Sender</Type><Code>InvalidParameter</Code><Message>fixture SNS failure</Message></Error><RequestId>fixture</RequestId></ErrorResponse>");} seen.fetch_add(1,Ordering::SeqCst);}
                (StatusCode::OK,[("content-type","text/xml")],"<PublishResponse xmlns=\"http://sns.amazonaws.com/doc/2010-03-31/\"><PublishResult><MessageId>fixture</MessageId></PublishResult><ResponseMetadata><RequestId>fixture</RequestId></ResponseMetadata></PublishResponse>")
            }});
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let referral = Arc::new(Referral::new(
            pool.clone(),
            Outbound::for_tests(),
            Settings {
                min_login_days: 0.0,
                reward_server_url: Some(url.clone()),
                suspicious_window_ms: 0.0,
                ..Settings::for_tests()
            },
        ));
        referral.set_publisher(crate::legacy_events::LegacyEvents::test_publisher(&url));
        sqlx::query("INSERT INTO social.referral_progress(referrer,invited_user,status,tier_granted) SELECT $1,'0x'||replace(gen_random_uuid()::text,'-','')||'00000000','tier_granted',true FROM generate_series(1,4)").bind(&referrer).execute(&pool).await.unwrap();
        referral
            .create(&referrer, &invited, "192.0.2.1")
            .await
            .unwrap();
        let lock = advisory_key(&pool, &uuid::Uuid::new_v4().to_string()).await;
        let mut held = pool.begin().await.unwrap();
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(lock)
            .execute(&mut *held)
            .await
            .unwrap();
        let function = format!("tier_image_gate_{}", uuid::Uuid::new_v4().simple());
        let operation = if image_first {
            "RAISE EXCEPTION 'fixture image failure';".into()
        } else {
            format!("PERFORM pg_advisory_xact_lock({lock});")
        };
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE FUNCTION social.{function}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.referrer='{referrer}' THEN {operation} END IF; RETURN NEW; END $$"))).execute(&pool).await.unwrap();
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE TRIGGER {function} BEFORE INSERT ON social.referral_reward_images FOR EACH ROW EXECUTE FUNCTION social.{function}()"))).execute(&pool).await.unwrap();
        let running = referral.clone();
        let user = invited.clone();
        let task = tokio::spawn(async move { running.finalize_referral(user.as_str()).await });
        tokio::time::timeout(std::time::Duration::from_secs(5), entries.recv())
            .await
            .expect("tier notification reaches the fixture")
            .unwrap();
        if !image_first {
            wait_for_locks(&pool, &[lock], 1).await;
            release.notify_one();
        }
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(2), task)
                .await
                .unwrap()
                .unwrap()
                .is_err(),
            "source Promise.all returns the first failure before unrelated work is released"
        );
        let state: (String, bool) = sqlx::query_as(
            "SELECT status,tier_granted FROM social.referral_progress WHERE invited_user=$1",
        )
        .bind(&invited)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            json!({"status":state.0,"tier_granted":state.1}),
            source["state"]
        );
        let images: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM social.referral_reward_images WHERE referrer=$1",
        )
        .bind(&referrer)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(images, 0);
        held.commit().await.unwrap();
        if image_first {
            release.notify_one();
        }
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                let images: i64 = sqlx::query_scalar(
                    "SELECT count(*) FROM social.referral_reward_images WHERE referrer=$1",
                )
                .bind(&referrer)
                .fetch_one(&pool)
                .await
                .unwrap();
                let notices: i64 = sqlx::query_scalar("SELECT count(*) FROM social.notifications WHERE address=$1 AND type='referral_new_tier_reached'")
                    .bind(&referrer).fetch_one(&pool).await.unwrap();
                if (image_first && seen.load(Ordering::SeqCst) == 1 && notices == 1)
                    || (!image_first && images == 1)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP TRIGGER {function} ON social.referral_reward_images"
        )))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP FUNCTION social.{function}()"
        )))
        .execute(&pool)
        .await
        .unwrap();
        if image_first {
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                loop {
                    referral.finalize_referral(invited.as_str()).await.unwrap();
                    let pending: Option<i32> = sqlx::query_scalar(
                        "SELECT reward_tier FROM social.referral_progress WHERE invited_user=$1",
                    )
                    .bind(&invited)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
                    if pending.is_none() {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            assert_eq!(
                seen.load(Ordering::SeqCst),
                1,
                "retrying the failed image must not resend its successful notification"
            );
        }
        server.abort();
    }
}

/// A restore hold blocks both new provider claims and confirmed downstream
/// retries. Refreshing the stamps resumes the same durable work.
#[tokio::test]
async fn reward_drain_respects_restore_holds_before_claims_and_follow_ups() {
    use crate::shared::writes::restore::{
        Stamp,
        lab::{HOUR, StampLab},
        scoped,
    };
    let Some(database) = reward_database().await else {
        return;
    };
    let pool = database.pool.clone();
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let app = axum::Router::new().route(
        "/api/rewards",
        axum::routing::post(move || {
            let observed = observed.clone();
            async move {
                observed.fetch_add(1, Ordering::SeqCst);
                Json(json!({"data":[{"image":"https://reward.example.org/new.png"}]}))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let referral = Referral::new(
        pool.clone(),
        Outbound::for_tests(),
        Settings {
            reward_server_url: Some(url),
            ..Settings::for_tests()
        },
    );
    for confirmed in [false, true] {
        sqlx::query("INSERT INTO social.referral_progress(referrer,invited_user,status,tier_granted,reward_tier,reward_requested_at,reward_response) VALUES($1,$2,'tier_granted',true,5,CASE WHEN $3 THEN timezone('UTC',now()) END,$4)")
            .bind(wallet()).bind(wallet()).bind(confirmed)
            .bind(confirmed.then(|| json!({"image":"https://reward.example.org/confirmed.png"})))
            .execute(&pool).await.unwrap();
    }
    let before: Vec<Value> = sqlx::query_scalar(
        "SELECT to_jsonb(r) FROM social.referral_progress r ORDER BY invited_user",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    for kind in ["missing", "stale", "no_blobs"] {
        let lab = if kind == "missing" {
            StampLab::new()
        } else {
            StampLab::fresh()
        };
        match kind {
            "stale" => lab.advance(30 * HOUR),
            "no_blobs" => {
                let mut stamp: Value = serde_json::from_slice(
                    &std::fs::read(lab.dir.join(Stamp::Backup.file())).unwrap(),
                )
                .unwrap();
                stamp["blobs"]["included"] = json!(false);
                lab.write_json(Stamp::Backup, stamp);
            }
            _ => {}
        }
        scoped(lab.stamps.clone(), async {
            assert!(crate::shared::writes::refusal(true).is_some(), "{kind}");
            referral.drain_pending_rewards().await.unwrap();
        })
        .await;
        let after: Vec<Value> = sqlx::query_scalar(
            "SELECT to_jsonb(r) FROM social.referral_progress r ORDER BY invited_user",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            after, before,
            "{kind}: no claim, retry timestamp or receipt mutation"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0, "{kind}: no provider POST");
        let effects: (i64, i64) = sqlx::query_as(
            "SELECT (SELECT count(*) FROM social.notifications), (SELECT count(*) FROM social.referral_reward_images)",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(effects, (0, 0), "{kind}: no downstream effects");
    }
    let fresh = StampLab::fresh();
    scoped(fresh.stamps.clone(), referral.drain_pending_rewards())
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let completed: (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM social.referral_progress WHERE reward_tier IS NOT NULL), (SELECT count(*) FROM social.notifications), (SELECT count(*) FROM social.referral_reward_images)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(completed, (0, 2, 2));
    drop(referral);
    server.abort();
    database.teardown().await;
}

/// Persist the provider response before tier SNS and resume it after a restart.
/// Concurrent logins and drainers must never repeat the provider POST.
#[tokio::test]
async fn confirmed_reward_survives_notification_failure_restart_and_concurrent_drain() {
    let Some(database) = reward_database().await else {
        return;
    };
    let pool = database.pool.clone();
    let (referrer, invited) = (wallet(), wallet());
    let sends = Arc::new(AtomicUsize::new(0));
    let rewards = Arc::new(AtomicUsize::new(0));
    let observed_sends = sends.clone();
    let observed_rewards = rewards.clone();
    let app = axum::Router::new()
        .route("/api/rewards", axum::routing::post(move || {
            let calls = observed_rewards.clone();
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Json(json!({"data":[{"image":"https://reward.example.org/tier.png","rarity":"rare"}]}))
            }
        }))
        .fallback(move || {
            let calls = observed_sends.clone();
            async move {
                if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                    (StatusCode::BAD_REQUEST, [("content-type", "text/xml")], "<ErrorResponse><Error><Type>Sender</Type><Code>InvalidParameter</Code><Message>fixture SNS failure</Message></Error><RequestId>fixture</RequestId></ErrorResponse>")
                } else {
                    (StatusCode::OK, [("content-type", "text/xml")], "<PublishResponse xmlns=\"http://sns.amazonaws.com/doc/2010-03-31/\"><PublishResult><MessageId>fixture</MessageId></PublishResult><ResponseMetadata><RequestId>fixture</RequestId></ResponseMetadata></PublishResponse>")
                }
            }
        });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let settings = Settings {
        reward_server_url: Some(url.clone()),
        ..Settings::for_tests()
    };
    sqlx::query("INSERT INTO social.referral_progress(referrer,invited_user,status,tier_granted,reward_tier) VALUES($1,$2,'tier_granted',true,5)")
        .bind(&referrer).bind(&invited).execute(&pool).await.unwrap();
    let before = Referral::new(pool.clone(), Outbound::for_tests(), settings.clone());
    before.set_publisher(crate::legacy_events::LegacyEvents::test_publisher(&url));
    assert!(before.finalize_referral(invited.as_str()).await.is_err());
    let stored: Value = sqlx::query_scalar(
        "SELECT reward_response FROM social.referral_progress WHERE invited_user=$1",
    )
    .bind(&invited)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored["rarity"], "rare");
    drop(before);
    let after = Arc::new(Referral::new(pool.clone(), Outbound::for_tests(), settings));
    after.set_publisher(crate::legacy_events::LegacyEvents::test_publisher(&url));
    // Retry using only the durable worker, without a new login event. Allow
    // the previous independent image effect to release its guard first.
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            sqlx::query(
                "UPDATE social.referral_progress SET reward_retry_at=NULL WHERE invited_user=$1",
            )
            .bind(&invited)
            .execute(&pool)
            .await
            .unwrap();
            let mut tasks = tokio::task::JoinSet::new();
            for _ in 0..8 {
                let referral = after.clone();
                tasks.spawn(async move { referral.drain_pending_rewards().await });
            }
            while let Some(task) = tasks.join_next().await {
                task.unwrap().unwrap();
            }
            let pending: Option<i32> = sqlx::query_scalar(
                "SELECT reward_tier FROM social.referral_progress WHERE invited_user=$1",
            )
            .bind(&invited)
            .fetch_one(&pool)
            .await
            .unwrap();
            if pending.is_none() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    after.finalize_referral(invited.as_str()).await.unwrap();
    assert_eq!(rewards.load(Ordering::SeqCst), 1);
    assert_eq!(sends.load(Ordering::SeqCst), 2);
    let images: i64 =
        sqlx::query_scalar("SELECT count(*) FROM social.referral_reward_images WHERE referrer=$1")
            .bind(&referrer)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(images, 1);
    server.abort();
    database.teardown().await;
}

#[tokio::test]
async fn ambiguous_reward_responses_are_not_reissued_after_restart_or_login() {
    for failure in ["http", "malformed", "missing_image"] {
        let Some(database) = reward_database().await else {
            return;
        };
        let pool = database.pool.clone();
        let (referrer, invited) = (wallet(), wallet());
        let rewards = Arc::new(AtomicUsize::new(0));
        let calls = rewards.clone();
        let app = axum::Router::new().route(
            "/api/rewards",
            axum::routing::post(move || {
                let calls = calls.clone();
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    // The provider can have granted before either failure reply.
                    match failure {
                        "http" => (StatusCode::INTERNAL_SERVER_ERROR, "unconfirmed response"),
                        "malformed" => (StatusCode::OK, "unconfirmed response"),
                        _ => (StatusCode::OK, r#"{"data":[{}]}"#),
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let settings = Settings {
            reward_server_url: Some(url),
            ..Settings::for_tests()
        };
        sqlx::query("INSERT INTO social.referral_progress(referrer,invited_user,status,tier_granted,reward_tier) VALUES($1,$2,'tier_granted',true,5)")
            .bind(&referrer).bind(&invited).execute(&pool).await.unwrap();
        let before = Referral::new(pool.clone(), Outbound::for_tests(), settings.clone());
        assert!(before.finalize_referral(invited.as_str()).await.is_err());
        drop(before);
        let after = Referral::new(pool.clone(), Outbound::for_tests(), settings);
        sqlx::query(
            "UPDATE social.referral_progress SET reward_retry_at=NULL WHERE invited_user=$1",
        )
        .bind(&invited)
        .execute(&pool)
        .await
        .unwrap();
        after.drain_pending_rewards().await.unwrap();
        assert!(after.finalize_referral(invited.as_str()).await.is_err());
        assert_eq!(rewards.load(Ordering::SeqCst), 1);
        let state: (Option<i32>, bool, bool) = sqlx::query_as("SELECT reward_tier,reward_requested_at IS NOT NULL,reward_response IS NULL FROM social.referral_progress WHERE invited_user=$1")
            .bind(&invited).fetch_one(&pool).await.unwrap();
        assert_eq!(state, (Some(5), true, true));
        server.abort();
        database.teardown().await;
    }
}

/// The production shared client does not impose a total deadline. The reward
/// adapter must bound reading the response body and retain an ambiguous claim.
#[tokio::test]
async fn a_reward_body_timeout_is_quarantined_without_reposting() {
    use futures::StreamExt;
    use std::time::Duration;
    let Some(database) = reward_database().await else {
        return;
    };
    let pool = database.pool.clone();
    let (referrer, invited) = (wallet(), wallet());
    let rewards = Arc::new(AtomicUsize::new(0));
    let calls = rewards.clone();
    let (started, mut starts) = mpsc::unbounded_channel();
    let app = axum::Router::new().route(
        "/api/rewards",
        axum::routing::post(move || {
            let calls = calls.clone();
            let started = started.clone();
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                let prefix = futures::stream::once(async move {
                    started.send(()).unwrap();
                    Ok::<_, std::io::Error>(axum::body::Bytes::from_static(b"{\"data\":["))
                });
                axum::body::Body::from_stream(prefix.chain(futures::stream::pending()))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let settings = Settings {
        reward_server_url: Some(url),
        ..Settings::for_tests()
    };
    sqlx::query("INSERT INTO social.referral_progress(referrer,invited_user,status,tier_granted,reward_tier) VALUES($1,$2,'tier_granted',true,5)")
        .bind(&referrer).bind(&invited).execute(&pool).await.unwrap();
    // Remove the test client's own 30-second timeout, so this exercises the
    // adapter's deadline, not a default that serve deliberately does not use.
    let http = Outbound::for_tests().provider_profile(None, None).unwrap();
    let before = Referral::new(pool.clone(), http.clone(), settings.clone());
    let user = invited.clone();
    let task = tokio::spawn(async move { before.finalize_referral(user.as_str()).await });
    tokio::time::timeout(Duration::from_secs(5), starts.recv())
        .await
        .unwrap()
        .unwrap();
    for _ in 0..128 {
        tokio::task::yield_now().await;
    }
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(31)).await;
    for _ in 0..128 {
        tokio::task::yield_now().await;
    }
    let finished = task.is_finished();
    tokio::time::resume();
    assert!(
        finished,
        "the adapter must bound reading an unfinished body"
    );
    assert!(task.await.unwrap().is_err());
    let after = Referral::new(pool.clone(), http, settings);
    sqlx::query("UPDATE social.referral_progress SET reward_retry_at=NULL WHERE invited_user=$1")
        .bind(&invited)
        .execute(&pool)
        .await
        .unwrap();
    after.drain_pending_rewards().await.unwrap();
    assert!(after.finalize_referral(invited.as_str()).await.is_err());
    assert_eq!(rewards.load(Ordering::SeqCst), 1);
    let state: (Option<i32>, bool, bool) = sqlx::query_as("SELECT reward_tier,reward_requested_at IS NOT NULL,reward_response IS NULL FROM social.referral_progress WHERE invited_user=$1")
        .bind(&invited).fetch_one(&pool).await.unwrap();
    assert_eq!(state, (Some(5), true, true));
    server.abort();
    database.teardown().await;
}

#[tokio::test]
async fn cancelled_inflight_reward_preserves_its_claim_for_reconciliation() {
    let Some(database) = reward_database().await else {
        return;
    };
    let pool = database.pool.clone();
    let (referrer, invited) = (wallet(), wallet());
    let entered = Arc::new(tokio::sync::Notify::new());
    let gate = entered.clone();
    let rewards = Arc::new(AtomicUsize::new(0));
    let observed = rewards.clone();
    let app = axum::Router::new().route(
        "/api/rewards",
        axum::routing::post(move || {
            let entered = gate.clone();
            let observed = observed.clone();
            async move {
                observed.fetch_add(1, Ordering::SeqCst);
                entered.notify_one();
                std::future::pending::<()>().await;
                StatusCode::OK
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let settings = Settings {
        reward_server_url: Some(url),
        ..Settings::for_tests()
    };
    sqlx::query("INSERT INTO social.referral_progress(referrer,invited_user,status,tier_granted,reward_tier) VALUES($1,$2,'tier_granted',true,5)")
        .bind(&referrer).bind(&invited).execute(&pool).await.unwrap();
    let before = Referral::new(pool.clone(), Outbound::for_tests(), settings.clone());
    let user = invited.clone();
    let task = tokio::spawn(async move { before.finalize_referral(user.as_str()).await });
    tokio::time::timeout(std::time::Duration::from_secs(5), entered.notified())
        .await
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    let after = Referral::new(pool.clone(), Outbound::for_tests(), settings);
    sqlx::query("UPDATE social.referral_progress SET reward_retry_at=NULL WHERE invited_user=$1")
        .bind(&invited)
        .execute(&pool)
        .await
        .unwrap();
    after.drain_pending_rewards().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while after.finalize_referral(invited.as_str()).await.is_ok() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(rewards.load(Ordering::SeqCst), 1);
    server.abort();
    database.teardown().await;
}

/// The first finalize fails as the source does (state and first effects), but
/// the grant keeps its reward tier pending. A request with an uncertain
/// provider outcome requires reconciliation before a retry.
#[tokio::test]
async fn a_failed_tier_reward_is_retried_only_after_its_provider_outcome_is_reconciled() {
    for reward_failure in [false, true] {
        let Some(pool) = crate::db::scratch::pool(6).await else {
            return;
        };
        let referrer = wallet();
        let invited = wallet();
        let sends = Arc::new(AtomicUsize::new(0));
        let rewards = Arc::new(AtomicUsize::new(0));
        let observed_sends = sends.clone();
        let observed_rewards = rewards.clone();
        let app=axum::Router::new().route("/api/rewards",axum::routing::post(move || {let calls=observed_rewards.clone();async move {
            let call=calls.fetch_add(1,Ordering::SeqCst);
            if reward_failure && call==0 {(StatusCode::BAD_REQUEST,Json(json!({})))} else {(StatusCode::OK,Json(json!({"data":[{"image":"https://reward.example.org/tier.png","rarity":"rare"}]})))}
        }}))
        .fallback(move || {let calls=observed_sends.clone();async move {let call=calls.fetch_add(1,Ordering::SeqCst);if !reward_failure && call==0 {(StatusCode::BAD_REQUEST,[("content-type","text/xml")],"<ErrorResponse><Error><Type>Sender</Type><Code>InvalidParameter</Code><Message>fixture SNS failure</Message></Error><RequestId>fixture</RequestId></ErrorResponse>")}else{(StatusCode::OK,[("content-type","text/xml")],"<PublishResponse xmlns=\"http://sns.amazonaws.com/doc/2010-03-31/\"><PublishResult><MessageId>fixture</MessageId></PublishResult><ResponseMetadata><RequestId>fixture</RequestId></ResponseMetadata></PublishResponse>")}}});
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let referral = Referral::new(
            pool.clone(),
            Outbound::for_tests(),
            Settings {
                min_login_days: 0.0,
                reward_server_url: Some(url.clone()),
                suspicious_window_ms: 0.0,
                ..Settings::for_tests()
            },
        );
        referral.set_publisher(crate::legacy_events::LegacyEvents::test_publisher(&url));
        sqlx::query("INSERT INTO social.referral_progress(referrer,invited_user,status,tier_granted) SELECT $1,'0x'||replace(gen_random_uuid()::text,'-','')||'00000000','tier_granted',true FROM generate_series(1,4)").bind(&referrer).execute(&pool).await.unwrap();
        referral
            .create(&referrer, &invited, "192.0.2.1")
            .await
            .unwrap();
        let source = source_failure(if reward_failure {
            "reward-failure"
        } else {
            "first-sns-failure"
        });
        let progress = |pool: sqlx::PgPool, invited: Address| async move {
            sqlx::query_as::<_, (String, bool, Option<i32>)>(
                "SELECT status,tier_granted,reward_tier FROM social.referral_progress WHERE invited_user=$1",
            )
            .bind(&invited)
            .fetch_one(&pool)
            .await
            .unwrap()
        };
        let images = |pool: sqlx::PgPool, referrer: Address| async move {
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM social.referral_reward_images WHERE referrer=$1 AND tier=5",
            )
            .bind(&referrer)
            .fetch_one(&pool)
            .await
            .unwrap()
        };

        assert!(referral.finalize_referral(invited.as_str()).await.is_err());
        let (status, granted, pending) = progress(pool.clone(), invited.clone()).await;
        assert_eq!(
            json!({"status":status,"tier_granted":granted}),
            source["state"]
        );
        assert_eq!(pending, Some(5), "the failed reward stays pending");
        assert_eq!(sends.load(Ordering::SeqCst) as u64, source["sns_calls"]);
        assert_eq!(rewards.load(Ordering::SeqCst), usize::from(reward_failure));
        assert_eq!(images(pool.clone(), referrer.clone()).await, 0);

        if reward_failure {
            assert!(referral.finalize_referral(invited.as_str()).await.is_err());
            assert_eq!(rewards.load(Ordering::SeqCst), 1);
            // An operator has confirmed that the fixture did not issue a
            // reward; only that reconciliation permits a second POST.
            sqlx::query("UPDATE social.referral_progress SET reward_requested_at=NULL WHERE invited_user=$1")
                .bind(&invited).execute(&pool).await.unwrap();
        }
        // Retry only the outstanding reward after configuration or outcome recovery.
        referral.finalize_referral(invited.as_str()).await.unwrap();
        assert_eq!(progress(pool.clone(), invited.clone()).await.2, None);
        assert_eq!(
            rewards.load(Ordering::SeqCst),
            1 + usize::from(reward_failure)
        );
        assert_eq!(
            sends.load(Ordering::SeqCst),
            2,
            "accepted once, tier reached once"
        );
        assert_eq!(images(pool.clone(), referrer.clone()).await, 1);

        referral.finalize_referral(invited.as_str()).await.unwrap();
        assert_eq!(
            rewards.load(Ordering::SeqCst),
            1 + usize::from(reward_failure)
        );
        assert_eq!(
            sends.load(Ordering::SeqCst),
            2,
            "a finished grant sends nothing"
        );
        server.abort();
    }
}

/// A paid reward whose tier notification fails is not paid again: the next
/// finalize retries only the notification, and the image is stored once.
#[tokio::test]
async fn a_failed_tier_notification_is_retried_without_paying_the_reward_again() {
    let Some(pool) = crate::db::scratch::pool(6).await else {
        return;
    };
    let (referrer, invited) = (wallet(), wallet());
    let rewards = Arc::new(AtomicUsize::new(0));
    let tier_notices = Arc::new(AtomicUsize::new(0));
    let (observed_rewards, observed_notices) = (rewards.clone(), tier_notices.clone());
    let app=axum::Router::new().route("/api/rewards",axum::routing::post(move || {let calls=observed_rewards.clone();async move {calls.fetch_add(1,Ordering::SeqCst);Json(json!({"data":[{"image":"https://reward.example.org/tier.png","rarity":"rare"}]}))}}))
        .fallback(move |body:axum::body::Bytes| {let notices=observed_notices.clone();async move {
            let fields=url::form_urlencoded::parse(&body).into_owned().collect::<std::collections::HashMap<_,_>>();
            let event:Value=serde_json::from_str(&fields["Message"]).unwrap();
            if event["subType"]=="referral-new-tier-reached" && notices.fetch_add(1,Ordering::SeqCst)==0 {
                return (StatusCode::BAD_REQUEST,[("content-type","text/xml")],"<ErrorResponse><Error><Type>Sender</Type><Code>InvalidParameter</Code><Message>fixture SNS failure</Message></Error><RequestId>fixture</RequestId></ErrorResponse>");
            }
            (StatusCode::OK,[("content-type","text/xml")],"<PublishResponse xmlns=\"http://sns.amazonaws.com/doc/2010-03-31/\"><PublishResult><MessageId>fixture</MessageId></PublishResult><ResponseMetadata><RequestId>fixture</RequestId></ResponseMetadata></PublishResponse>")
        }});
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let referral = Referral::new(
        pool.clone(),
        Outbound::for_tests(),
        Settings {
            min_login_days: 0.0,
            reward_server_url: Some(url.clone()),
            suspicious_window_ms: 0.0,
            ..Settings::for_tests()
        },
    );
    referral.set_publisher(crate::legacy_events::LegacyEvents::test_publisher(&url));
    sqlx::query("INSERT INTO social.referral_progress(referrer,invited_user,status,tier_granted) SELECT $1,'0x'||replace(gen_random_uuid()::text,'-','')||'00000000','tier_granted',true FROM generate_series(1,4)").bind(&referrer).execute(&pool).await.unwrap();
    referral
        .create(&referrer, &invited, "192.0.2.1")
        .await
        .unwrap();
    let state = |pool: sqlx::PgPool, invited: Address| async move {
        sqlx::query_as::<_, (Option<i32>, Option<Value>)>(
            "SELECT reward_tier,reward_paid FROM social.referral_progress WHERE invited_user=$1",
        )
        .bind(&invited)
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    let images = |pool: sqlx::PgPool, referrer: Address| async move {
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM social.referral_reward_images WHERE referrer=$1 AND tier=5",
        )
        .bind(&referrer)
        .fetch_one(&pool)
        .await
        .unwrap()
    };

    assert!(referral.finalize_referral(invited.as_str()).await.is_err());
    assert_eq!(rewards.load(Ordering::SeqCst), 1);
    assert_eq!(tier_notices.load(Ordering::SeqCst), 1);
    let (owed, paid) = state(pool.clone(), invited.clone()).await;
    assert_eq!(
        owed,
        Some(5),
        "the grant remains pending until follow-ups finish"
    );
    let paid = paid.expect("its follow-ups are pending");
    assert_eq!(
        (paid["tier"].clone(), paid["notified"].clone()),
        (json!(5), json!(false))
    );
    // The image work started with the notification and survives its failure.
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while images(pool.clone(), referrer.clone()).await != 1 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the paid reward's image is stored");

    referral.finalize_referral(invited.as_str()).await.unwrap();
    assert_eq!(
        rewards.load(Ordering::SeqCst),
        1,
        "the reward is not requested again"
    );
    assert_eq!(
        tier_notices.load(Ordering::SeqCst),
        2,
        "the notification is retried"
    );
    assert_eq!(state(pool.clone(), invited.clone()).await, (None, None));
    assert_eq!(images(pool.clone(), referrer.clone()).await, 1);

    referral.finalize_referral(invited.as_str()).await.unwrap();
    assert_eq!(
        (
            rewards.load(Ordering::SeqCst),
            tier_notices.load(Ordering::SeqCst)
        ),
        (1, 2)
    );
    server.abort();
}

/// A tier without a configured campaign key posts nothing; the reward stays
/// pending until the key is configured.
#[tokio::test]
async fn an_unset_campaign_key_leaves_the_reward_pending_without_a_request() {
    let Some(pool) = crate::db::scratch::pool(6).await else {
        return;
    };
    let (referrer, invited) = (wallet(), wallet());
    let rewards = Arc::new(AtomicUsize::new(0));
    let observed = rewards.clone();
    let app=axum::Router::new().route("/api/rewards",axum::routing::post(move || {let calls=observed.clone();async move {calls.fetch_add(1,Ordering::SeqCst);Json(json!({"data":[{"image":"https://reward.example.org/tier.png","rarity":"rare"}]}))}}));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let settings = Settings {
        min_login_days: 0.0,
        reward_server_url: Some(url.clone()),
        reward_keys: Default::default(),
        suspicious_window_ms: 0.0,
        ..Settings::for_tests()
    };
    sqlx::query("INSERT INTO social.referral_progress(referrer,invited_user,status,tier_granted) SELECT $1,'0x'||replace(gen_random_uuid()::text,'-','')||'00000000','tier_granted',true FROM generate_series(1,4)").bind(&referrer).execute(&pool).await.unwrap();
    let unset = Referral::new(pool.clone(), Outbound::for_tests(), settings.clone());
    unset
        .create(&referrer, &invited, "192.0.2.1")
        .await
        .unwrap();
    assert!(unset.finalize_referral(invited.as_str()).await.is_err());
    assert_eq!(
        rewards.load(Ordering::SeqCst),
        0,
        "no empty campaign_key request"
    );
    let claimed: bool = sqlx::query_scalar("SELECT reward_requested_at IS NOT NULL FROM social.referral_progress WHERE invited_user=$1")
        .bind(&invited).fetch_one(&pool).await.unwrap();
    assert!(
        !claimed,
        "missing configuration must not consume the provider claim"
    );
    let configured = Arc::new(Referral::new(
        pool.clone(),
        Outbound::for_tests(),
        Settings {
            reward_keys: Settings::for_tests().reward_keys,
            ..settings
        },
    ));
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..12 {
        let referral = configured.clone();
        let invited = invited.clone();
        tasks.spawn(async move { referral.finalize_referral(invited.as_str()).await });
    }
    while let Some(task) = tasks.join_next().await {
        task.unwrap().unwrap();
    }
    assert_eq!(rewards.load(Ordering::SeqCst), 1);
    let pending: Option<i32> = sqlx::query_scalar(
        "SELECT reward_tier FROM social.referral_progress WHERE invited_user=$1",
    )
    .bind(&invited)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pending, None);
    server.abort();
}

#[tokio::test]
async fn historical_ipv6_aliases_and_mapped_ipv4_keep_their_quota_and_new_writers_share_the_canonical_lock()
 {
    let Some(pool) = crate::db::scratch::pool(12).await else {
        return;
    };
    let referral = Arc::new(Referral::new(
        repeatable_read_writers(&pool).await,
        Outbound::for_tests(),
        Settings {
            max_ip_matches: 3.0,
            suspicious_window_ms: 0.0,
            ..Settings::for_tests()
        },
    ));
    for (historical, canonical, variants) in [
        (
            ["2001:0DB8:0:0:0:0:0:1", "2001:db8::1"],
            "2001:db8::1",
            ["2001:0db8::0001", "2001:db8:0:0::1"],
        ),
        (
            ["::ffff:c000:201", "::ffff:192.0.2.1"],
            "192.0.2.1",
            ["192.0.2.1", "::FFFF:192.0.2.1"],
        ),
    ] {
        let referrer = wallet();
        for ip in historical.into_iter().chain(["invalid historical IP"]) {
            sqlx::query("INSERT INTO social.referral_progress(referrer,invited_user,invited_user_ip,status) VALUES($1,$2,$3,'pending')")
                .bind(&referrer).bind(wallet()).bind(ip).execute(&pool).await.unwrap();
        }
        let key = advisory_key(&pool, &format!("social-referral-ip:{referrer}:{canonical}")).await;
        let mut held = pool.begin().await.unwrap();
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(key)
            .execute(&mut *held)
            .await
            .unwrap();
        let invited = [wallet(), wallet()];
        let mut tasks = tokio::task::JoinSet::new();
        for (user, ip) in invited.iter().cloned().zip(variants) {
            let referral = referral.clone();
            let referrer = referrer.clone();
            tasks.spawn(async move { referral.create(&referrer, &user, ip).await });
        }
        wait_for_locks(&pool, &[key], 2).await;
        held.commit().await.unwrap();
        let mut accepted = 0;
        while let Some(result) = tasks.join_next().await {
            accepted += usize::from(result.unwrap().is_ok());
        }
        assert_eq!(
            accepted, 1,
            "two historical aliases already consume two of the three slots"
        );
        let rows: Vec<(String, String)> = sqlx::query_as("SELECT invited_user_ip,status FROM social.referral_progress WHERE invited_user=ANY($1)")
            .bind(invited.iter().map(ToString::to_string).collect::<Vec<_>>()).fetch_all(&pool).await.unwrap();
        assert!(rows.iter().all(|(ip, _)| ip == canonical));
        assert_eq!(
            rows.iter()
                .filter(|(_, status)| status == "pending")
                .count(),
            1
        );
        assert_eq!(
            rows.iter()
                .filter(|(_, status)| status == "rejected_ip_match")
                .count(),
            1
        );
        let retained: i64 = sqlx::query_scalar("SELECT count(*) FROM social.referral_progress WHERE referrer=$1 AND invited_user_ip='invalid historical IP'")
            .bind(&referrer).fetch_one(&pool).await.unwrap();
        assert_eq!(retained, 1);
    }
    let referrer = wallet();
    assert!(referral.create(&referrer, &wallet(), "").await.is_err());
    assert!(
        referral
            .create(&referrer, &wallet(), "not-an-IP")
            .await
            .is_err()
    );
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM social.referral_progress WHERE referrer=$1")
            .bind(&referrer)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
}
