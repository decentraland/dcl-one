//! The credits alerts (`alerts.rs`), each on a database of its own (the
//! sweep and the delivery job walk every row): the settlement's three
//! alarms written in its transaction and once per case, the expiry sweep's
//! two in the tick and the sweep alike, and the delivery job's sent, retried,
//! skipped and given-up rows.

use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use chrono::Timelike;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::alerts::{self, Delivery, PassOutcome};
use super::store::{Credits, now};
use super::tests::{FakeMarket, MockBroker, OwnDatabase, scratch_wallet, seed_wallet};
use super::usage_tests::{
    auth_status, authorization, one_used_credit_index, salt, used_credit_index,
};
use super::{RawSettings, Settings, State};
use crate::http::Outbound;

const SHOP: &str = "https://shop.example";

/// The `(dedupe_key, payload, status)` of every alert of `kind`, oldest first.
async fn alerts_of(pool: &PgPool, kind: &str) -> Vec<(String, Value, String)> {
    sqlx::query_as::<_, (String, sqlx::types::Json<Value>, String)>(
        "SELECT dedupe_key, payload, status FROM credits.alerts WHERE kind = $1 ORDER BY id",
    )
    .bind(kind)
    .fetch_all(pool)
    .await
    .unwrap()
    .into_iter()
    .map(|(key, payload, status)| (key, payload.0, status))
    .collect()
}

/// A credit used on chain after its hold expired settles (the debit is
/// re-applied) and queues one alert, which a second pass does not repeat.
#[tokio::test]
async fn a_credit_used_after_its_hold_expired_is_alerted_once() {
    let Some(db) = OwnDatabase::create("alerts_recovered").await else {
        return;
    };
    let pool = db.pool.clone();
    let wallet = scratch_wallet();
    seed_wallet(&pool, &wallet.address, "0", "100").await;
    let pledge = salt();
    let earlier = now() - chrono::Duration::minutes(10);
    authorization(&pool, &pledge, &wallet.address, 600, "expired", earlier).await;
    let contract = format!("0x{}", "b".repeat(40));
    line(&pool, &pledge, 0, &contract, 7).await;
    line(&pool, &pledge, 1, &contract, 7).await;
    let index = one_used_credit_index(&pool, &wallet.address, &pledge).await;

    let out = index.run_pass(None, true).await.unwrap();
    assert!(out.consumed == 1 && out.failed == 0, "{out:?}");
    index.run_pass(None, true).await.unwrap();
    // The feed's line, written in the same settlement, once.
    let settled = alerts_of(&pool, alerts::PURCHASE_SETTLED).await;
    assert_eq!(settled.len(), 1, "{settled:?}");
    let (key, payload, _) = &settled[0];
    assert_eq!(key, &pledge);
    assert_eq!(payload["recovered"], true);
    assert_eq!(payload["chainId"], 137);
    assert_eq!(payload["manaWei"], "10000000000000000000");
    assert_eq!(payload["lines"], 2);
    assert_eq!(
        payload["items"],
        json!([{ "contractAddress": contract, "itemId": "7" }, { "contractAddress": contract, "itemId": "7" }])
    );
    let recovered = alerts_of(&pool, alerts::RECOVERED_CONSUMPTION).await;
    assert_eq!(recovered.len(), 1, "{recovered:?}");
    let (key, payload, status) = &recovered[0];
    assert_eq!(key, &pledge);
    assert_eq!(status, "pending");
    assert_eq!(payload["status"], "expired");
    assert_eq!(payload["debitedCredits"], "60");
    assert_eq!(payload["address"], wallet.address.as_str());
    assert!(
        alerts_of(&pool, alerts::UNPAID_CONSUMPTION)
            .await
            .is_empty()
    );
    db.drop().await;
}

/// A used credit its wallet cannot cover debits nothing, keeps its
/// authorization and queues one alert however often the fold retries it;
/// the rolled-back settlement queues no recovered alert.
#[tokio::test]
async fn a_used_credit_its_wallet_cannot_cover_is_alerted_once() {
    let Some(db) = OwnDatabase::create("alerts_unpaid").await else {
        return;
    };
    let pool = db.pool.clone();
    let wallet = scratch_wallet();
    seed_wallet(&pool, &wallet.address, "0", "10").await;
    let pledge = salt();
    let earlier = now() - chrono::Duration::minutes(10);
    authorization(&pool, &pledge, &wallet.address, 600, "expired", earlier).await;
    let index = one_used_credit_index(&pool, &wallet.address, &pledge).await;

    let out = index.run_pass(None, true).await.unwrap();
    assert!(out.consumed == 0 && out.failed == 1, "{out:?}");
    // The reconciler's full fold retries the debit, and fails it again.
    let contract: String =
        sqlx::query_scalar("SELECT contract FROM credits.usages WHERE salt = $1")
            .bind(&pledge)
            .fetch_one(&pool)
            .await
            .unwrap();
    let credits = Credits::new(pool.clone());
    for _ in 0..2 {
        let diffs = super::usage::fold(&credits, 137, &contract, -1, 2_000)
            .await
            .unwrap();
        assert!(
            !diffs.is_empty() && diffs.iter().all(|d| !d.consumed),
            "{diffs:?}"
        );
    }
    assert_eq!(auth_status(&pool, &pledge).await, "expired");
    let unpaid = alerts_of(&pool, alerts::UNPAID_CONSUMPTION).await;
    assert_eq!(unpaid.len(), 1, "{unpaid:?}");
    let (key, payload, _) = &unpaid[0];
    assert_eq!(key, "137:1500:0");
    assert_eq!(payload["salt"], pledge.as_str());
    assert_eq!(payload["debitedCredits"], "60");
    assert!(
        alerts_of(&pool, alerts::RECOVERED_CONSUMPTION)
            .await
            .is_empty()
    );
    db.drop().await;
}

/// The cap alerts of one live credit signed for `cap_wei` and used for 10
/// MANA, on a database of its own; `None` without PostgreSQL.
async fn cap_alerts(name: &str, cap_wei: u128) -> Option<Vec<(String, Value, String)>> {
    let db = OwnDatabase::create(name).await?;
    let pool = db.pool.clone();
    let wallet = scratch_wallet();
    seed_wallet(&pool, &wallet.address, "0", "100").await;
    let pledge = salt();
    sqlx::query(
        "INSERT INTO credits.authorizations (id, address, usd_cents, amount_wei, status, expires_at) \
         VALUES ($1, $2, 600, $3::numeric, 'authorized', $4)",
    )
    .bind(&pledge)
    .bind(&wallet.address)
    .bind(cap_wei.to_string())
    .bind(now() + chrono::Duration::minutes(10))
    .execute(&pool)
    .await
    .unwrap();
    let index = used_credit_index(&pool, &wallet.address, &pledge, 10u128.pow(19)).await;
    let out = index.run_pass(None, true).await.unwrap();
    assert!(out.consumed == 1, "{out:?}");
    assert!(
        alerts_of(&pool, alerts::RECOVERED_CONSUMPTION)
            .await
            .is_empty(),
        "a live credit's use is no recovery"
    );
    let alerts = alerts_of(&pool, alerts::CAP_NEAR_LIMIT).await;
    db.drop().await;
    Some(alerts)
}

/// Consumption at 99.5% of the cap or more is alerted, rounded toward the
/// alert; one wei of cap more and it is not.
#[tokio::test]
async fn a_credit_consumed_from_its_cap_share_on_is_alerted() {
    let at = 10u128.pow(23) / alerts::CAP_NEAR_LIMIT_BPS as u128;
    let Some(near) = cap_alerts("alerts_cap_near", at).await else {
        return;
    };
    assert_eq!(near.len(), 1, "{near:?}");
    assert_eq!(near[0].1["consumedWei"], "10000000000000000000");
    assert_eq!(near[0].1["capWei"], at.to_string());
    let below = cap_alerts("alerts_cap_below", at + 1).await.unwrap();
    assert!(below.is_empty(), "{below:?}");
}

async fn submit(pool: &PgPool, id: &str, tx_hash: &str) {
    sqlx::query("UPDATE credits.authorizations SET submitted_tx_hash = $2 WHERE id = $1")
        .bind(id)
        .bind(tx_hash)
        .execute(pool)
        .await
        .unwrap();
}

/// The sweep alerts a submitted credit that expired with no use indexed,
/// only where the CreditUsed index bounds it: not one never submitted, not
/// one with a use indexed, not one whose transaction went through by another
/// credit of the same wallet (another wallet's credit in it does not count),
/// and once.
#[tokio::test]
async fn an_expired_credit_the_shop_submitted_is_alerted() {
    let Some(db) = OwnDatabase::create("alerts_submitted").await else {
        return;
    };
    let pool = db.pool.clone();
    let (wallet, other) = (scratch_wallet().address, scratch_wallet().address);
    let earlier = now() - chrono::Duration::minutes(10);
    let later = now() + chrono::Duration::minutes(10);
    // Without the index the sweep runs on the clock: nothing is alerted.
    let unindexed = salt();
    authorization(&pool, &unindexed, &wallet, 500, "authorized", earlier).await;
    submit(&pool, &unindexed, &salt()).await;
    let clock_only = Credits::new(pool.clone());
    assert_eq!(clock_only.expire_stale_authorizations().await.unwrap(), 1);
    assert!(
        alerts_of(&pool, alerts::SUBMITTED_UNCONSUMED)
            .await
            .is_empty()
    );
    // With it, the sweep expires what the index read past.
    sqlx::query(
        "INSERT INTO credits.usage_index_cursor (chain_id, contract, last_block, folded_block, folded_block_time) \
         VALUES (137, $1, 100, 100, $2)",
    )
    .bind(format!("0x{}", "c".repeat(40)))
    .bind(now() + chrono::Duration::hours(1))
    .execute(&pool)
    .await
    .unwrap();
    let credits = Credits::new(pool.clone()).with_usage_index(137);
    let [alone, unsent, sibling_used, sibling, other_wallets, indexed] =
        std::array::from_fn::<_, 6, _>(|_| salt());
    let [tx_alone, tx_shared, tx_indexed] = std::array::from_fn::<_, 3, _>(|_| salt());
    authorization(&pool, &alone, &wallet, 500, "authorized", earlier).await;
    submit(&pool, &alone, &tx_alone).await;
    authorization(&pool, &unsent, &wallet, 500, "authorized", earlier).await;
    authorization(&pool, &sibling_used, &wallet, 500, "consumed", later).await;
    submit(&pool, &sibling_used, &tx_shared).await;
    authorization(&pool, &sibling, &wallet, 500, "authorized", earlier).await;
    submit(&pool, &sibling, &tx_shared).await;
    authorization(&pool, &other_wallets, &other, 500, "retired", earlier).await;
    submit(&pool, &other_wallets, &tx_shared).await;
    authorization(&pool, &indexed, &wallet, 500, "authorized", earlier).await;
    submit(&pool, &indexed, &tx_indexed).await;
    sqlx::query(
        "INSERT INTO credits.usages (chain_id, block_number, log_index, contract, tx_hash, sender, credit_id, salt, credit_value, credit_expires_at, value_wei) \
         VALUES (137, 1, 0, $1, $2, $3, $4, $5, 1, 1, 1)",
    )
    .bind(format!("0x{}", "c".repeat(40)))
    .bind(&tx_indexed)
    .bind(&wallet)
    .bind(salt())
    .bind(&indexed)
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(credits.expire_stale_authorizations().await.unwrap(), 5);
    assert_eq!(credits.expire_stale_authorizations().await.unwrap(), 0);
    let submitted = alerts_of(&pool, alerts::SUBMITTED_UNCONSUMED).await;
    let mut keys: Vec<&str> = submitted.iter().map(|(key, _, _)| key.as_str()).collect();
    keys.sort_unstable();
    let mut expected = [alone.as_str(), other_wallets.as_str()];
    expected.sort_unstable();
    assert_eq!(keys, expected, "{submitted:?}");
    let (_, payload, _) = submitted.iter().find(|(key, _, _)| key == &alone).unwrap();
    assert_eq!(payload["txHash"], tx_alone.as_str());
    assert_eq!(payload["usdCents"], 500);
    assert_eq!(payload["address"], wallet.as_str());
    assert_eq!(auth_status(&pool, &other_wallets).await, "released");
    db.drop().await;
}

/// A primary-mint line of `authorization` for item `item` of `contract`.
async fn line(pool: &PgPool, authorization: &str, position: i32, contract: &str, item: i64) {
    sqlx::query(
        "INSERT INTO credits.authorization_lines (authorization_id, position, quoted_cents, charged_cents, sale_kind, asset_kind, asset_chain_id, asset_contract, asset_item_id) \
         VALUES ($1, $2, 300, 300, 'primary_mint', 'wearable', 137, $3, $4::numeric)",
    )
    .bind(authorization)
    .bind(position)
    .bind(contract)
    .bind(item)
    .execute(pool)
    .await
    .unwrap();
}

/// An authorization of `wallet` expired at `expires`, created `age` ago,
/// for item `item` of `contract`.
async fn attempt(
    pool: &PgPool,
    wallet: &str,
    contract: &str,
    item: i64,
    status: &str,
    age: chrono::Duration,
) -> String {
    let id = salt();
    sqlx::query(
        "INSERT INTO credits.authorizations (id, address, usd_cents, amount_wei, status, expires_at, created_at) \
         VALUES ($1, $2, 500, 1, $3, $4, $5)",
    )
    .bind(&id)
    .bind(wallet)
    .bind(status)
    .bind(now() - chrono::Duration::minutes(1))
    .bind(now() - age)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO credits.authorization_lines (authorization_id, position, quoted_cents, charged_cents, sale_kind, asset_kind, asset_chain_id, asset_contract, asset_item_id) \
         VALUES ($1, 0, 500, 500, 'primary_mint', 'wearable', 137, $2, $3::numeric)",
    )
    .bind(&id)
    .bind(contract)
    .bind(item)
    .execute(pool)
    .await
    .unwrap();
    id
}

/// The tick alerts a wallet whose purchases of one item expired three times
/// within half an hour of the newest: an older attempt, another item and
/// another wallet do not count, and attempts swept long after they were made
/// still do; a later expired attempt alerts again, a tick with none does not.
#[tokio::test]
async fn a_wallet_whose_purchases_of_one_item_keep_expiring_is_alerted() {
    let Some(db) = OwnDatabase::create("alerts_repeated").await else {
        return;
    };
    let pool = db.pool.clone();
    let state = State::with_sources(
        pool.clone(),
        Outbound::for_tests(),
        Settings::for_tests(),
        Arc::new(FakeMarket::default()),
        Arc::new(MockBroker::configured()),
        None,
    );
    let worker = state.outbox_worker();
    let (wallet, other) = (scratch_wallet().address, scratch_wallet().address);
    let contract = format!("0x{}", "a".repeat(40));
    let minutes = chrono::Duration::minutes;
    attempt(&pool, &wallet, &contract, 7, "expired", minutes(40)).await;
    attempt(&pool, &wallet, &contract, 7, "expired", minutes(20)).await;
    attempt(&pool, &wallet, &contract, 7, "released", minutes(10)).await;
    attempt(&pool, &wallet, &contract, 7, "authorized", minutes(5)).await;
    attempt(&pool, &wallet, &contract, 8, "authorized", minutes(5)).await;
    attempt(&pool, &other, &contract, 7, "expired", minutes(10)).await;
    attempt(&pool, &other, &contract, 7, "authorized", minutes(5)).await;
    // A long TTL and a late sweep: all three attempts are older than half an
    // hour when swept, yet created within 25 minutes of one another.
    let slow = scratch_wallet().address;
    attempt(&pool, &slow, &contract, 9, "expired", minutes(60)).await;
    attempt(&pool, &slow, &contract, 9, "released", minutes(45)).await;
    let slow_newest = attempt(&pool, &slow, &contract, 9, "authorized", minutes(35)).await;

    worker.run_once().await.unwrap();
    let of = async |address: &str| -> Vec<(String, Value, String)> {
        alerts_of(&pool, alerts::REPEATED_EXPIRY)
            .await
            .into_iter()
            .filter(|(_, payload, _)| payload["address"] == address)
            .collect()
    };
    let repeated = of(&wallet).await;
    assert_eq!(repeated.len(), 1, "{repeated:?}");
    let (_, payload, _) = &repeated[0];
    assert_eq!(payload["item"], format!("{contract}:7"));
    assert_eq!(payload["attempts"], 3);
    let slowly = of(&slow).await;
    assert_eq!(
        slowly.len(),
        1,
        "anchored on the attempts, not the sweep: {slowly:?}"
    );
    assert_eq!(slowly[0].1["attempts"], 3);
    assert_eq!(
        slowly[0].0,
        format!("{slow}:{contract}:9:{slow_newest}"),
        "keyed by the newest attempt"
    );
    assert!(of(&other).await.is_empty());

    worker.run_once().await.unwrap();
    assert_eq!(of(&wallet).await.len(), 1);
    attempt(&pool, &wallet, &contract, 7, "authorized", minutes(1)).await;
    worker.run_once().await.unwrap();
    let repeated = of(&wallet).await;
    assert_eq!(repeated.len(), 2, "{repeated:?}");
    assert_eq!(repeated[1].1["attempts"], 4);
    db.drop().await;
}

/// A webhook stub answering `status` with `headers`, keeping each body it
/// got, and counting the requests a redirect to `/elsewhere` would bring.
struct Hook {
    status: Mutex<StatusCode>,
    headers: Mutex<Vec<(&'static str, String)>>,
    bodies: Mutex<Vec<Value>>,
    elsewhere: std::sync::atomic::AtomicUsize,
}

async fn webhook(status: StatusCode) -> (String, Arc<Hook>) {
    let hook = Arc::new(Hook {
        status: Mutex::new(status),
        headers: Mutex::default(),
        bodies: Mutex::default(),
        elsewhere: std::sync::atomic::AtomicUsize::new(0),
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/hook", listener.local_addr().unwrap());
    let app = axum::Router::new()
        .route(
            "/hook",
            axum::routing::post(
                |axum::extract::State(hook): axum::extract::State<Arc<Hook>>,
                 axum::Json(body): axum::Json<Value>| async move {
                    hook.bodies.lock().unwrap().push(body);
                    let mut headers = axum::http::HeaderMap::new();
                    for (name, value) in hook.headers.lock().unwrap().iter() {
                        headers.insert(*name, value.parse().unwrap());
                    }
                    (*hook.status.lock().unwrap(), headers)
                },
            ),
        )
        .route(
            "/elsewhere",
            axum::routing::any(
                |axum::extract::State(hook): axum::extract::State<Arc<Hook>>| async move {
                    hook.elsewhere
                        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    StatusCode::OK
                },
            ),
        )
        .with_state(hook.clone());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, hook)
}

async fn queue_one(pool: &PgPool) {
    let mut conn = pool.acquire().await.unwrap();
    for usd_cents in [500, 900] {
        alerts::record(
            &mut conn,
            alerts::SUBMITTED_UNCONSUMED,
            "0xabc",
            json!({ "salt": "0xabc", "address": "0xdef", "usdCents": usd_cents, "txHash": "0x01" }),
        )
        .await
        .unwrap();
    }
}

async fn alert_row(pool: &PgPool) -> (String, i32, Option<String>, chrono::NaiveDateTime) {
    sqlx::query_as(
        "SELECT status, attempts, last_error, next_attempt_at FROM credits.alerts WHERE dedupe_key = '0xabc'",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

/// An alert refused by Slack waits out its backoff and is posted again; it
/// ends `sent` once Slack accepted it, and is never posted after that. A
/// case queued twice is one alert.
#[tokio::test]
async fn an_alert_is_retried_until_slack_accepts_it() {
    let Some(db) = OwnDatabase::create("alerts_delivery").await else {
        return;
    };
    let credits = Credits::new(db.pool.clone());
    let (url, hook) = webhook(StatusCode::INTERNAL_SERVER_ERROR).await;
    let delivery = Delivery::new(Some(url), SHOP, Outbound::for_tests());
    queue_one(&db.pool).await;
    let t0 = now() + chrono::Duration::seconds(1);
    let seconds = chrono::Duration::seconds;

    let out = delivery.run_pass(&credits, t0).await.unwrap();
    assert_eq!(
        out,
        PassOutcome {
            retried: 1,
            ..PassOutcome::default()
        }
    );
    let (status, attempts, error, next) = alert_row(&db.pool).await;
    assert_eq!((status.as_str(), attempts), ("pending", 1));
    assert_eq!(
        error.as_deref(),
        Some("Slack answered 500 Internal Server Error")
    );
    assert!(
        next > t0 + seconds(29) && next <= t0 + seconds(30),
        "{next}"
    );
    let early = delivery.run_pass(&credits, t0 + seconds(29)).await.unwrap();
    assert_eq!(early, PassOutcome::default(), "not before its backoff");

    *hook.status.lock().unwrap() = StatusCode::OK;
    let out = delivery.run_pass(&credits, t0 + seconds(30)).await.unwrap();
    assert_eq!(
        out,
        PassOutcome {
            sent: 1,
            ..PassOutcome::default()
        }
    );
    assert_eq!(alert_row(&db.pool).await.0, "sent");
    let later = delivery
        .run_pass(&credits, t0 + seconds(3600))
        .await
        .unwrap();
    assert_eq!(later, PassOutcome::default());
    let bodies = hook.bodies.lock().unwrap().clone();
    assert_eq!(bodies.len(), 2);
    for body in bodies {
        let text = body["text"].as_str().unwrap();
        assert!(
            text.contains("0xabc")
                && text.contains("$5.00")
                && text.contains("expired with no use indexed"),
            "{text}"
        );
    }
    db.drop().await;
}

/// With no webhook an alert is logged at error and ends `skipped`.
#[tokio::test]
async fn with_no_webhook_an_alert_ends_skipped() {
    let Some(db) = OwnDatabase::create("alerts_unset").await else {
        return;
    };
    let credits = Credits::new(db.pool.clone());
    let delivery = Delivery::new(None, SHOP, Outbound::for_tests());
    queue_one(&db.pool).await;
    let t0 = now() + chrono::Duration::seconds(1);
    let out = delivery.run_pass(&credits, t0).await.unwrap();
    assert_eq!(
        out,
        PassOutcome {
            skipped: 1,
            ..PassOutcome::default()
        }
    );
    assert_eq!(alert_row(&db.pool).await.0, "skipped");
    assert_eq!(
        delivery.run_pass(&credits, t0).await.unwrap(),
        PassOutcome::default()
    );
    db.drop().await;
}

/// An alert still pending a day after it was queued is posted once all the
/// same (no writer may have delivered meanwhile), and given up only once that
/// post failed too.
#[tokio::test]
async fn an_alert_undelivered_for_a_day_is_given_up_after_one_post() {
    let Some(db) = OwnDatabase::create("alerts_stale").await else {
        return;
    };
    let credits = Credits::new(db.pool.clone());
    let (url, hook) = webhook(StatusCode::BAD_GATEWAY).await;
    let delivery = Delivery::new(Some(url), SHOP, Outbound::for_tests());
    queue_one(&db.pool).await;
    let late = now() + alerts::MAX_AGE + chrono::Duration::seconds(1);
    let out = delivery.run_pass(&credits, late).await.unwrap();
    assert_eq!(
        out,
        PassOutcome {
            retried: 1,
            ..PassOutcome::default()
        }
    );
    let out = delivery
        .run_pass(&credits, late + chrono::Duration::seconds(31))
        .await
        .unwrap();
    assert_eq!(
        out,
        PassOutcome {
            failed: 1,
            ..PassOutcome::default()
        }
    );
    assert_eq!(alert_row(&db.pool).await.0, "failed");
    assert_eq!(hook.bodies.lock().unwrap().len(), 1, "posted once");
    db.drop().await;
}

/// A redirect is not followed: it is a refusal to retry, and its target
/// hears nothing.
#[tokio::test]
async fn a_redirect_is_not_followed() {
    let Some(db) = OwnDatabase::create("alerts_redirect").await else {
        return;
    };
    let credits = Credits::new(db.pool.clone());
    let (url, hook) = webhook(StatusCode::FOUND).await;
    hook.headers
        .lock()
        .unwrap()
        .push(("location", url.replace("/hook", "/elsewhere")));
    // Serve's client follows two redirects; the post must not.
    let follows = Outbound::new(crate::http::HttpClientCfg::serve(4))
        .unwrap()
        .allowing_private_hosts();
    let delivery = Delivery::new(Some(url), SHOP, follows);
    queue_one(&db.pool).await;
    let out = delivery
        .run_pass(&credits, now() + chrono::Duration::seconds(1))
        .await
        .unwrap();
    assert_eq!(
        out,
        PassOutcome {
            retried: 1,
            ..PassOutcome::default()
        }
    );
    assert_eq!(
        alert_row(&db.pool).await.2.as_deref(),
        Some("Slack answered 302 Found")
    );
    assert_eq!(hook.elsewhere.load(std::sync::atomic::Ordering::SeqCst), 0);
    db.drop().await;
}

/// A webhook that refuses the connection is retried, and the reason kept
/// never names it (its path is the credential).
#[tokio::test]
async fn a_refused_connection_is_retried_without_naming_the_webhook() {
    let Some(db) = OwnDatabase::create("alerts_refused").await else {
        return;
    };
    let credits = Credits::new(db.pool.clone());
    let closed = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "http://{}/services/secret-path",
        closed.local_addr().unwrap()
    );
    drop(closed);
    let delivery = Delivery::new(Some(url), SHOP, Outbound::for_tests());
    queue_one(&db.pool).await;
    let out = delivery
        .run_pass(&credits, now() + chrono::Duration::seconds(1))
        .await
        .unwrap();
    assert_eq!(
        out,
        PassOutcome {
            retried: 1,
            ..PassOutcome::default()
        }
    );
    let error = alert_row(&db.pool).await.2.unwrap();
    assert_eq!(error, "the webhook did not accept a connection");
    assert!(!error.contains("secret-path"));
    db.drop().await;
}

/// Slack's 429 holds the alert for its `Retry-After`, and the rest of the
/// pass with it. A held alert still gets its first post after the age limit.
#[tokio::test]
async fn slack_rate_limiting_holds_the_rest_of_the_pass() {
    let Some(db) = OwnDatabase::create("alerts_rate_limited").await else {
        return;
    };
    let credits = Credits::new(db.pool.clone());
    let (url, hook) = webhook(StatusCode::TOO_MANY_REQUESTS).await;
    hook.headers
        .lock()
        .unwrap()
        .push(("retry-after", "120".into()));
    let delivery = Delivery::new(Some(url), SHOP, Outbound::for_tests());
    queue_one(&db.pool).await;
    let mut conn = db.pool.acquire().await.unwrap();
    alerts::record(
        &mut conn,
        alerts::SUBMITTED_UNCONSUMED,
        "0xdef",
        json!({ "salt": "0xdef", "address": "0xdef", "usdCents": 100, "txHash": "0x02" }),
    )
    .await
    .unwrap();
    drop(conn);
    let t0 = now().with_nanosecond(0).unwrap() + chrono::Duration::hours(1);
    let out = delivery.run_pass(&credits, t0).await.unwrap();
    assert_eq!(
        out,
        PassOutcome {
            retried: 2,
            ..PassOutcome::default()
        }
    );
    assert_eq!(
        hook.bodies.lock().unwrap().len(),
        1,
        "the second is held, not posted"
    );
    let rows: Vec<(String, i32, chrono::NaiveDateTime)> = sqlx::query_as(
        "SELECT last_error, attempts, next_attempt_at FROM credits.alerts ORDER BY id",
    )
    .fetch_all(&db.pool)
    .await
    .unwrap();
    let next = t0 + chrono::Duration::seconds(120);
    assert_eq!(
        rows,
        vec![
            ("Slack answered 429 Too Many Requests".into(), 1, next),
            ("held while Slack rate limits".into(), 0, next),
        ]
    );

    *hook.status.lock().unwrap() = StatusCode::BAD_GATEWAY;
    let late = t0 + alerts::MAX_AGE;
    let out = delivery.run_pass(&credits, late).await.unwrap();
    assert_eq!(
        out,
        PassOutcome {
            retried: 1,
            failed: 1,
            ..PassOutcome::default()
        }
    );
    {
        let bodies = hook.bodies.lock().unwrap();
        assert_eq!(bodies.len(), 2);
        assert!(bodies[1]["text"].as_str().unwrap().contains("0xdef"));
    }
    let out = delivery
        .run_pass(&credits, late + chrono::Duration::seconds(30))
        .await
        .unwrap();
    assert_eq!(
        out,
        PassOutcome {
            failed: 1,
            ..PassOutcome::default()
        }
    );
    assert_eq!(hook.bodies.lock().unwrap().len(), 2);
    db.drop().await;
}

#[tokio::test]
async fn slack_retry_after_is_capped_at_thirty_minutes() {
    let Some(db) = OwnDatabase::create("alerts_retry_after_cap").await else {
        return;
    };
    let credits = Credits::new(db.pool.clone());
    let (url, hook) = webhook(StatusCode::TOO_MANY_REQUESTS).await;
    hook.headers
        .lock()
        .unwrap()
        .push(("retry-after", "86400".into()));
    let delivery = Delivery::new(Some(url), SHOP, Outbound::for_tests());
    queue_one(&db.pool).await;

    let t0 = now().with_nanosecond(0).unwrap() + chrono::Duration::hours(1);
    delivery.run_pass(&credits, t0).await.unwrap();
    assert_eq!(
        alert_row(&db.pool).await.3,
        t0 + chrono::Duration::minutes(30)
    );
    db.drop().await;
}

/// The webhook is a public https endpoint, checked at startup; blank is
/// unset.
#[test]
fn the_alerts_webhook_must_be_a_public_https_endpoint() {
    let parse = |url: &str| {
        Settings::parse(RawSettings {
            writes_enabled: true,
            require_purchase_intent: false,
            mana_price_max_staleness_secs: 3600,
            credits_usd_pegged_feed_max_age_secs: 3_600,
            alerts_slack_webhook: Some(url.into()),
            ..RawSettings::default()
        })
    };
    for refused in [
        "http://hooks.slack.com/services/example",
        "https://127.0.0.1/hook",
        "https://localhost/hook",
        "hooks.slack.com/services/example",
    ] {
        let error = parse(refused).err().expect(refused).to_string();
        assert!(error.contains("CREDITS_ALERTS_SLACK_WEBHOOK"), "{error}");
    }
    let accepted = parse(" https://hooks.slack.com/services/example ").unwrap();
    assert_eq!(
        accepted.alerts_slack_webhook.as_deref(),
        Some("https://hooks.slack.com/services/example")
    );
    assert!(parse("  ").unwrap().alerts_slack_webhook.is_none());
}

/// The settled purchases of a pass share one message, the other alerts one
/// each; a message's alerts share its outcome, a refused one retrying them
/// all.
#[tokio::test]
async fn settled_purchases_share_a_message_and_its_outcome() {
    let Some(db) = OwnDatabase::create("alerts_feed_batch").await else {
        return;
    };
    let credits = Credits::new(db.pool.clone());
    let (url, hook) = webhook(StatusCode::OK).await;
    let delivery = Delivery::new(Some(url), SHOP, Outbound::for_tests());
    let mut conn = db.pool.acquire().await.unwrap();
    let settled = |n: i64| json!({ "address": format!("0x{n:040x}"), "usdCents": 100 * n, "manaWei": "1", "lines": 1, "items": [] });
    for n in 1..=3 {
        alerts::record(
            &mut conn,
            alerts::PURCHASE_SETTLED,
            &format!("s{n}"),
            settled(n),
        )
        .await
        .unwrap();
    }
    alerts::record(
        &mut conn,
        alerts::PACK_CREDITED,
        "pack:p1",
        json!({ "address": "0x02", "credits": "50", "chargedCents": 500, "packId": "pack_50", "quantity": 1 }),
    )
    .await
    .unwrap();
    let t0 = now() + chrono::Duration::seconds(1);
    let out = delivery.run_pass(&credits, t0).await.unwrap();
    assert_eq!(
        out,
        PassOutcome {
            sent: 4,
            ..PassOutcome::default()
        }
    );
    let bodies: Vec<String> = hook
        .bodies
        .lock()
        .unwrap()
        .iter()
        .map(|b| b["text"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(bodies.len(), 2, "{bodies:?}");
    assert!(
        bodies[0]
            .starts_with(":shopping_bags: *3 purchases* — 3 items · 60 credits ($6.00) · 3 buyers"),
        "{}",
        bodies[0]
    );
    assert_eq!(bodies[0].lines().count(), 4);
    assert!(
        bodies[1].starts_with(":coin: *Credits purchased* — 50 credits ($5.00)"),
        "{}",
        bodies[1]
    );

    *hook.status.lock().unwrap() = StatusCode::SERVICE_UNAVAILABLE;
    for n in 4..=5 {
        alerts::record(
            &mut conn,
            alerts::PURCHASE_SETTLED,
            &format!("s{n}"),
            settled(n),
        )
        .await
        .unwrap();
    }
    let out = delivery
        .run_pass(&credits, t0 + chrono::Duration::seconds(1))
        .await
        .unwrap();
    assert_eq!(
        out,
        PassOutcome {
            retried: 2,
            ..PassOutcome::default()
        }
    );
    let pending: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT status, last_error FROM credits.alerts WHERE dedupe_key IN ('s4', 's5') ORDER BY id",
    )
    .fetch_all(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        pending,
        vec![
            (
                "pending".to_string(),
                Some("Slack answered 503 Service Unavailable".to_string())
            );
            2
        ]
    );
    assert_eq!(hook.bodies.lock().unwrap().len(), 3, "one post for both");
    drop(conn);
    db.drop().await;
}
