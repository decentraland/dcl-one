use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::mpsc;

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
        entries.recv().await.unwrap();
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
        referral.finalize_referral(invited.as_str()).await.unwrap();
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
        server.abort();
    }
}

#[tokio::test]
async fn committed_grant_remains_a_retry_receipt_after_initial_sns_or_reward_failure() {
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
        let app=axum::Router::new().route("/api/rewards",axum::routing::post(move || {let calls=observed_rewards.clone();async move {calls.fetch_add(1,Ordering::SeqCst);StatusCode::BAD_REQUEST}}))
        .fallback(move || {let calls=observed_sends.clone();async move {calls.fetch_add(1,Ordering::SeqCst);if !reward_failure {(StatusCode::BAD_REQUEST,[("content-type","text/xml")],"<ErrorResponse><Error><Type>Sender</Type><Code>InvalidParameter</Code><Message>fixture SNS failure</Message></Error><RequestId>fixture</RequestId></ErrorResponse>")}else{(StatusCode::OK,[("content-type","text/xml")],"<PublishResponse xmlns=\"http://sns.amazonaws.com/doc/2010-03-31/\"><PublishResult><MessageId>fixture</MessageId></PublishResult><ResponseMetadata><RequestId>fixture</RequestId></ResponseMetadata></PublishResponse>")}}});
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
        assert!(referral.finalize_referral(invited.as_str()).await.is_err());
        referral.finalize_referral(invited.as_str()).await.unwrap();
        let source = source_failure(if reward_failure {
            "reward-failure"
        } else {
            "first-sns-failure"
        });
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
        assert_eq!(sends.load(Ordering::SeqCst) as u64, source["sns_calls"]);
        assert_eq!(rewards.load(Ordering::SeqCst), usize::from(reward_failure));
        server.abort();
    }
}
