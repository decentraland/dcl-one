//! The credits daily summary (`daily_summary.rs`): the hour and the day's
//! claim, the figures of a closed UTC day read from rows on both sides of
//! its midnights, a chain that cannot be read, and the message.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    reason = "test fixtures use small literal values"
)]

use std::sync::Arc;

use alloy_primitives::U256;
use async_trait::async_trait;
use chrono::{NaiveDate, NaiveDateTime};
use serde_json::{Value, json};
use sqlx::PgPool;

use super::daily_summary::{
    ChainFigures, DAILY_SUMMARY, DailySummary, NoChain, PassOutcome, Reading, figures, message,
};
use super::store::Credits;
use super::tests::{OwnDatabase, seed_wallet};
use super::usage_tests::{authorization, salt};

struct Chain {
    float: Reading,
    rate: Reading,
}

#[async_trait]
impl ChainFigures for Chain {
    async fn float_wei(&self) -> Reading {
        self.float
    }
    async fn mana_usd_18(&self) -> Reading {
        self.rate
    }
}

fn at(day: u32, h: u32, m: u32, s: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 11, day)
        .unwrap()
        .and_hms_opt(h, m, s)
        .unwrap()
}

fn mana(n: u64) -> Reading {
    Reading::Value(U256::from(n) * U256::from(10u64).pow(U256::from(18u64)))
}

async fn summaries(pool: &PgPool) -> Vec<(String, Value)> {
    sqlx::query_as::<_, (String, sqlx::types::Json<Value>)>(
        "SELECT dedupe_key, payload FROM credits.alerts WHERE kind = $1 ORDER BY id",
    )
    .bind(DAILY_SUMMARY)
    .fetch_all(pool)
    .await
    .unwrap()
    .into_iter()
    .map(|(key, payload)| (key, payload.0))
    .collect()
}

/// Nothing before the hour; from it, yesterday is queued once, whatever
/// the passes after it; a missed day is not posted, the next pass reports
/// its own yesterday.
#[tokio::test]
async fn the_day_is_queued_once_from_the_hour() {
    let Some(db) = OwnDatabase::create("daily_summary_claim").await else {
        return;
    };
    let credits = Credits::new(db.pool.clone());
    let summary = DailySummary {
        hour_utc: 9,
        chain: Arc::new(NoChain),
    };
    let day = |d: u32| NaiveDate::from_ymd_opt(2026, 11, d).unwrap();

    let early = summary.run_pass(&credits, at(21, 8, 59, 59)).await.unwrap();
    assert_eq!(early, PassOutcome::BeforeTheHour);
    assert!(summaries(&db.pool).await.is_empty());

    let first = summary.run_pass(&credits, at(21, 9, 0, 0)).await.unwrap();
    assert_eq!(first, PassOutcome::Queued(day(20)));
    let again = summary.run_pass(&credits, at(21, 23, 59, 0)).await.unwrap();
    assert_eq!(again, PassOutcome::AlreadyQueued(day(20)));
    // A replica past the EXISTS check inserts nothing either, and knows it.
    let payload = figures(&credits, day(20), at(21, 9, 0, 0)).await.unwrap();
    let mut conn = db.pool.acquire().await.unwrap();
    let added = super::alerts::queue(&mut conn, DAILY_SUMMARY, "2026-11-20", payload)
        .await
        .unwrap();
    assert!(!added);
    drop(conn);
    let rows = summaries(&db.pool).await;
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].0, "2026-11-20");
    assert_eq!(rows[0].1["day"], "2026-11-20");
    assert_eq!(rows[0].1["floatWei"], Value::Null, "no chain, no float");

    // The 22nd's window was missed: the 23rd reports the 22nd only.
    let later = summary.run_pass(&credits, at(23, 15, 0, 0)).await.unwrap();
    assert_eq!(later, PassOutcome::Queued(day(22)));
    let keys: Vec<String> = summaries(&db.pool).await.into_iter().map(|r| r.0).collect();
    assert_eq!(keys, ["2026-11-20", "2026-11-22"]);
}

async fn purchase(
    pool: &PgPool,
    method: &str,
    status: &str,
    credits: &str,
    cents: i64,
    created: NaiveDateTime,
) {
    sqlx::query(
        "INSERT INTO credits.purchases (address, sku, credits, amount_cents, currency, method, status, created_at) \
         VALUES ($1, 'pack', $2::numeric, $3, 'usd', $4, $5, $6)",
    )
    .bind(format!("0x{}", "1".repeat(40)))
    .bind(credits)
    .bind(cents)
    .bind(method)
    .bind(status)
    .bind(created)
    .execute(pool)
    .await
    .unwrap();
}

/// One CreditUsed log and its settlement: `kind` of authorization `auth` by
/// `address`, in transaction `tx` (one byte repeated), at `when`.
#[allow(
    clippy::too_many_arguments,
    reason = "one argument per settlement column the test varies"
)]
async fn settled(
    pool: &PgPool,
    log: i32,
    auth: &str,
    address: &str,
    kind: &str,
    wei: &str,
    debited: &str,
    tx: char,
    when: NaiveDateTime,
) {
    sqlx::query(
        "INSERT INTO credits.usages (chain_id, block_number, log_index, contract, tx_hash, block_time, sender, credit_id, salt, credit_value, credit_expires_at, value_wei) \
         VALUES (137, 1, $1, $2, $3, $4, $5, $6, $6, 1, 1, $7::numeric)",
    )
    .bind(log)
    .bind(format!("0x{}", "c".repeat(40)))
    .bind(format!("0x{}", tx.to_string().repeat(64)))
    .bind(when)
    .bind(address)
    .bind(salt())
    .bind(wei)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO credits.usage_settlements (chain_id, block_number, log_index, authorization_id, address, kind, value_wei, debited_credits, tx_ref, settled_at) \
         VALUES (137, 1, $1, $2, $3, $4, $5::numeric, $6::numeric, 'ref-' || $1, $7)",
    )
    .bind(log)
    .bind(auth)
    .bind(address)
    .bind(kind)
    .bind(wei)
    .bind(debited)
    .bind(when)
    .execute(pool)
    .await
    .unwrap();
}

async fn lines(pool: &PgPool, auth: &str, n: i32) {
    for position in 0..n {
        sqlx::query(
            "INSERT INTO credits.authorization_lines (authorization_id, position, quoted_cents, charged_cents, sale_kind, asset_kind, asset_chain_id, asset_contract, asset_item_id) \
             VALUES ($1, $2, 300, 300, 'primary_mint', 'wearable', 137, $3, 1)",
        )
        .bind(auth)
        .bind(position)
        .bind(format!("0x{}", "b".repeat(40)))
        .execute(pool)
        .await
        .unwrap();
    }
}

async fn expired_on(pool: &PgPool, created: NaiveDateTime) {
    sqlx::query(
        "INSERT INTO credits.authorizations (id, address, usd_cents, amount_wei, status, expires_at, created_at) \
         VALUES ($1, $2, 100, 1, 'expired', $3 + interval '1 hour', $3)",
    )
    .bind(salt())
    .bind(format!("0x{}", "1".repeat(40)))
    .bind(created)
    .execute(pool)
    .await
    .unwrap();
}

/// Each figure counts the rows of `[00:00, 24:00)` UTC of the 20th and none
/// of the last second before it or the first one after.
#[tokio::test]
async fn the_figures_read_the_closed_day() {
    let Some(db) = OwnDatabase::create("daily_summary_figures").await else {
        return;
    };
    let pool = db.pool.clone();
    let credits = Credits::new(pool.clone());
    let (a, b, c) = (
        format!("0x{}", "a".repeat(40)),
        format!("0x{}", "b".repeat(40)),
        format!("0x{}", "c".repeat(40)),
    );

    purchase(&pool, "card", "paid", "999", 9_990, at(19, 23, 59, 59)).await;
    purchase(&pool, "card", "paid", "60", 600, at(20, 0, 0, 0)).await;
    purchase(&pool, "card", "paid", "40", 400, at(20, 23, 59, 59)).await;
    purchase(&pool, "card", "paid", "999", 9_990, at(21, 0, 0, 0)).await;
    purchase(&pool, "mana", "paid", "999", 9_990, at(20, 12, 0, 0)).await;
    purchase(&pool, "card", "abandoned", "10", 100, at(20, 12, 0, 0)).await;
    purchase(&pool, "card", "failed", "10", 100, at(20, 12, 0, 0)).await;
    purchase(&pool, "card", "pending", "10", 100, at(20, 12, 0, 0)).await;
    purchase(&pool, "card", "refunded", "10", 100, at(20, 12, 0, 0)).await;

    let later = at(30, 0, 0, 0);
    let d = format!("0x{}", "d".repeat(40));
    let (a1, a2, b0, b1, c1, d1) = (salt(), salt(), salt(), salt(), salt(), salt());
    for (auth, who) in [
        (&a1, &a),
        (&a2, &a),
        (&b0, &b),
        (&b1, &b),
        (&c1, &c),
        (&d1, &d),
    ] {
        authorization(&pool, auth, who, 300, "consumed", later).await;
    }
    lines(&pool, &a1, 2).await;
    lines(&pool, &a2, 1).await;
    lines(&pool, &b0, 5).await;
    lines(&pool, &b1, 1).await;
    lines(&pool, &c1, 5).await;
    // a1 and a2 are one cart: one transaction. b0 makes b a returning buyer,
    // and its remainder that day adds credits and MANA, not items.
    settled(
        &pool,
        0,
        &a1,
        &a,
        "purchase",
        "5000000000000000000",
        "6",
        '1',
        at(20, 10, 0, 0),
    )
    .await;
    settled(
        &pool,
        1,
        &a2,
        &a,
        "purchase",
        "2000000000000000000",
        "3",
        '1',
        at(20, 11, 0, 0),
    )
    .await;
    settled(
        &pool,
        2,
        &b0,
        &b,
        "remainder",
        "100000000000000000",
        "1",
        '3',
        at(20, 11, 30, 0),
    )
    .await;
    settled(
        &pool,
        3,
        &b0,
        &b,
        "purchase",
        "1000000000000000000",
        "3",
        '4',
        at(19, 23, 59, 59),
    )
    .await;
    settled(
        &pool,
        4,
        &b1,
        &b,
        "purchase",
        "1000000000000000000",
        "3",
        '2',
        at(20, 12, 0, 0),
    )
    .await;
    settled(
        &pool,
        5,
        &c1,
        &c,
        "purchase",
        "9000000000000000000",
        "9",
        '5',
        at(21, 0, 0, 0),
    )
    .await;
    // a1's cart took a second credit: another log, the same items.
    settled(
        &pool,
        8,
        &a1,
        &a,
        "purchase",
        "500000000000000000",
        "1",
        '1',
        at(20, 10, 0, 0),
    )
    .await;
    // A single item keeps its line on the authorization: one item.
    settled(
        &pool,
        7,
        &d1,
        &d,
        "purchase",
        "1000000000000000000",
        "2",
        '7',
        at(20, 13, 0, 0),
    )
    .await;
    // Outside the seven days that end with the 20th.
    settled(
        &pool,
        6,
        &c1,
        &c,
        "remainder",
        "7000000000000000000",
        "7",
        '6',
        at(13, 23, 59, 59),
    )
    .await;

    seed_wallet(&pool, &a, "5", "20").await;
    seed_wallet(&pool, &b, "0", "7").await;

    expired_on(&pool, at(19, 23, 59, 59)).await;
    expired_on(&pool, at(20, 5, 0, 0)).await;
    // A dispute records no amount: it is counted apart from the refunds. A
    // refund followed by a dispute keeps both on its row, and counts in both.
    for (intent, cents, disputed, when) in [
        ("pi_in", 500, false, at(20, 8, 0, 0)),
        ("pi_disputed", 0, true, at(20, 9, 0, 0)),
        ("pi_both", 300, true, at(20, 10, 0, 0)),
        ("pi_after", 900, false, at(21, 0, 0, 0)),
    ] {
        sqlx::query("INSERT INTO credits.payment_reversals (payment_intent, refunded_cents, disputed, updated_at) VALUES ($1, $2, $3, $4)")
            .bind(intent)
            .bind(cents as i64)
            .bind(disputed)
            .bind(when)
            .execute(&pool)
            .await
            .unwrap();
    }

    let day = NaiveDate::from_ymd_opt(2026, 11, 20).unwrap();
    let got = figures(&credits, day, at(21, 9, 0, 0)).await.unwrap();
    assert_eq!(
        got,
        json!({
            "day": "2026-11-20",
            "packs": { "count": 2, "credits": "100", "chargedCents": 1000 },
            "purchases": {
                "items": 5,
                "checkouts": 3,
                "credits": "16",
                "buyers": 3,
                "newBuyers": 2,
                "manaWei": "9600000000000000000",
            },
            "checkout": { "credited": 2, "abandoned": 1, "failed": 1, "pending": 1 },
            "unspentCredits": "27",
            "owed": { "credits": "32", "manaWei": "0" },
            "outflow7dWei": "10600000000000000000",
            "integrity": { "expired": 1, "refunds": 2, "refundedCents": 800, "chargebacks": 2 },
        })
    );
}

/// A claimed goal grant owes its amount less its own usages: those of its
/// salt on its chain, contract and credit, by its wallet, as the wallet's
/// balance reads them. Another credit's usage that reuses the salt takes
/// nothing off.
#[tokio::test]
async fn a_goal_grant_owes_what_its_own_usages_left() {
    let Some(db) = OwnDatabase::create("daily_summary_goal_grant").await else {
        return;
    };
    let pool = db.pool.clone();
    let credits = Credits::new(pool.clone());
    let wallet = format!("0x{}", "e".repeat(40));
    let grant = format!("0x{}", "5".repeat(64));
    let credit = format!("0x{}", "6".repeat(64));
    let contract = format!("0x{}", "c".repeat(40));
    for statement in [
        "INSERT INTO credits.seasons (id, name, start_date, end_date, max_mana_wei) VALUES (1, 'One', '2026-11-01', '2026-12-31', 0)",
        "INSERT INTO credits.season_goals (season_id, goal_id, title, thumbnail, total_steps, reward_wei, start_week, end_week) VALUES (1, 'take-photo', 'Photo', 'p.png', 1, 0, 1, 20)",
        "INSERT INTO credits.wallets (address) VALUES ($1)",
        "INSERT INTO credits.goal_progress (address, season_id, week_id, goal_id, title, thumbnail, total_steps, reward_wei, completed_steps, completed_at) VALUES ($1, 1, 1, 'take-photo', 'Photo', 'p.png', 1, 0, 1, '2026-11-02')",
    ] {
        sqlx::query(statement)
            .bind(&wallet)
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query(
        "INSERT INTO credits.goal_grants (address, season_id, week_id, goal_id, salt, chain_id, contract, credit_hash, amount_wei, signature, created_at, expires_at, claimed_at) \
         VALUES ($1, 1, 1, 'take-photo', $2, 137, $3, $4, 100, '0xsig', '2026-11-02', '2026-11-30', '2026-11-02')",
    )
    .bind(&wallet)
    .bind(&grant)
    .bind(&contract)
    .bind(&credit)
    .execute(&pool)
    .await
    .unwrap();
    let used = async |log: i32,
                      chain: i64,
                      contract: &str,
                      credit: &str,
                      sender: &str,
                      wei: i64| {
        sqlx::query(
            "INSERT INTO credits.usages (chain_id, block_number, log_index, contract, tx_hash, block_time, sender, credit_id, salt, credit_value, credit_expires_at, value_wei) \
             VALUES ($1, 1, $2, $3, $4, '2026-11-03', $5, $6, $7, 1, 1, $8)",
        )
        .bind(chain)
        .bind(log)
        .bind(contract)
        .bind(format!("0x{}", "f".repeat(64)))
        .bind(sender)
        .bind(credit)
        .bind(&grant)
        .bind(wei)
        .execute(&pool)
        .await
        .unwrap();
    };
    let owed = async || {
        let day = NaiveDate::from_ymd_opt(2026, 11, 20).unwrap();
        figures(&credits, day, at(21, 9, 0, 0)).await.unwrap()["owed"]["manaWei"].clone()
    };
    let other = format!("0x{}", "9".repeat(40));
    let other_credit = format!("0x{}", "8".repeat(64));
    // Each differs from the grant in one of the four.
    used(0, 1, &contract, &credit, &wallet, 100).await;
    used(1, 137, &other, &credit, &wallet, 100).await;
    used(2, 137, &contract, &other_credit, &wallet, 100).await;
    used(3, 137, &contract, &credit, &other, 100).await;
    assert_eq!(owed().await, json!("100"), "other credits' usages");
    used(4, 137, &contract, &credit, &wallet, 40).await;
    assert_eq!(owed().await, json!("60"), "its own partial usage");
}

/// A chain read that fails says "unavailable" in its line, and the day is
/// queued all the same; the rate is read only when there is a float.
#[tokio::test]
async fn an_unreadable_chain_still_queues_the_summary() {
    let Some(db) = OwnDatabase::create("daily_summary_chain").await else {
        return;
    };
    let credits = Credits::new(db.pool.clone());
    let pass = |float, rate, hour_utc| DailySummary {
        hour_utc,
        chain: Arc::new(Chain { float, rate }),
    };

    pass(Reading::Unavailable, mana(1), 9)
        .run_pass(&credits, at(21, 9, 0, 0))
        .await
        .unwrap();
    pass(mana(1_000), Reading::Unavailable, 9)
        .run_pass(&credits, at(22, 9, 0, 0))
        .await
        .unwrap();
    let rows = summaries(&db.pool).await;
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].1["floatWei"], "unavailable");
    assert_eq!(rows[0].1["manaUsd18"], Value::Null);
    assert!(message(&rows[0].1).contains(":fuelpump: *Float now*  unavailable"));
    assert_eq!(rows[1].1["floatWei"], "1000000000000000000000");
    assert_eq!(rows[1].1["manaUsd18"], "unavailable");
    let text = message(&rows[1].1);
    assert!(text.contains("CreditsManager 1,000 MANA\n"), "{text}");
    assert!(text.contains("Coverage unpriced"), "{text}");
    // The delivery job renders the row by its kind.
    assert_eq!(
        super::alerts::render(DAILY_SUMMARY, &rows[1].1, "https://shop.example"),
        text
    );
}

fn fixture() -> Value {
    json!({
        "day": "2026-11-20",
        "packs": { "count": 2, "credits": "100", "chargedCents": 1500 },
        "purchases": {
            "items": 4,
            "checkouts": 2,
            "credits": "13",
            "buyers": 2,
            "newBuyers": 1,
            "manaWei": "8100000000000000000",
        },
        "checkout": { "credited": 2, "abandoned": 1, "failed": 1, "pending": 1 },
        "unspentCredits": "27",
        "owed": { "credits": "32", "manaWei": "0" },
        "outflow7dWei": "70000000000000000000",
        "integrity": { "expired": 1, "refunds": 1, "refundedCents": 500, "chargebacks": 1 },
        "floatWei": "1000000000000000000000",
        "manaUsd18": "250000000000000000",
    })
}

#[test]
fn the_message_spells_out_the_day() {
    assert_eq!(
        message(&fixture()),
        [
            ":bar_chart: *Shop — 2026-11-20 (UTC)*",
            ":credit_card: *Credits sold*  2 packs · 100 credits",
            "      $15.00 charged  −$1.04 Stripe (est.)  −$10.00 credits issued  =  *$3.96* ours (26.4%)",
            ":shopping_bags: *Items bought*  4 items in 2 checkouts · 13 credits ($1.30) · AOV $0.65",
            ":busts_in_silhouette: *Buyers*  2 unique, 1 new · 8.1 MANA moved ($1.30)",
            ":receipt: *Card checkout*  2 credited · 1 abandoned · 1 declined → 50% completed · 1 still open",
            ":bank: *Unspent*  $2.70 in paid credits (sold or granted) not yet spent",
            ":fuelpump: *Float now*  CreditsManager 1,000 MANA ($250 · 2,500 credits)",
            ":shield: *Coverage*  78.1× the $3.20 owed in unspent credits · ~100 days at the 7-day pace (10 MANA/day)",
            "",
            "1 reservation expired unused · :warning: 1 refund ($5.00) · :warning: 1 chargeback",
        ]
        .join("\n")
    );
}

#[test]
fn a_quiet_day_has_no_money_line_and_no_warning() {
    let quiet = json!({
        "day": "2026-11-20",
        "packs": { "count": 0, "credits": "0", "chargedCents": 0 },
        "purchases": { "items": 0, "checkouts": 0, "credits": "0", "buyers": 0, "newBuyers": 0, "manaWei": "0" },
        "checkout": { "credited": 0, "abandoned": 0, "failed": 0, "pending": 0 },
        "unspentCredits": "0",
        "owed": { "credits": "0", "manaWei": "0" },
        "outflow7dWei": "0",
        "integrity": { "expired": 0, "refunds": 0, "refundedCents": 0, "chargebacks": 0 },
        "floatWei": null,
        "manaUsd18": null,
    });
    assert_eq!(
        message(&quiet),
        [
            ":bar_chart: *Shop — 2026-11-20 (UTC)*",
            ":credit_card: *Credits sold*  0 packs · 0 credits",
            ":shopping_bags: *Items bought*  0 items",
            ":busts_in_silhouette: *Buyers*  0 unique, 0 new · 0 MANA moved",
            ":receipt: *Card checkout*  0 credited · 0 abandoned · 0 declined",
            ":bank: *Unspent*  $0.00 in paid credits (sold or granted) not yet spent",
        ]
        .join("\n")
    );
}

#[test]
fn a_short_float_is_warned() {
    let mut low = fixture();
    low["floatWei"] = json!("100000000000000000000");
    low["owed"]["credits"] = json!("20000");
    low["owed"]["manaWei"] = json!("40000000000000000000");
    low["outflow7dWei"] = json!("140000000000000000000");
    let text = message(&low);
    // $25 against $2,000 of credits and 40 MANA ($10) owed.
    assert!(text.contains("0.01× the $2,010.00 owed"), "{text}");
    assert!(
        text.contains(":warning: Float covers only 0.01× the unspent credits"),
        "{text}"
    );
    assert!(
        text.contains(":warning: Float runs out in ~5 days at the 7-day pace"),
        "{text}"
    );
}

/// A JSON-RPC endpoint that answers `eth_call` by selector, or with an error
/// to every call when `broken`.
async fn rpc(broken: bool, updated_at: i64) -> String {
    use axum::Router;
    use axum::routing::post;
    let word = |n: u128| format!("{n:064x}");
    let app = Router::new().route(
        "/",
        post(move |axum::Json(req): axum::Json<Value>| async move {
            if broken {
                return axum::Json(json!({ "jsonrpc": "2.0", "id": req["id"], "error": { "code": -32000, "message": "execution reverted" } }));
            }
            let data = req["params"][0]["data"].as_str().unwrap_or("");
            let result = match &data[..10.min(data.len())] {
                // balanceOf: 1,000 MANA.
                "0x70a08231" => format!("0x{}", word(1_000 * 10u128.pow(18))),
                // decimals: 8.
                "0x313ce567" => format!("0x{}", word(8)),
                // latestRoundData: $0.25 with 8 decimals.
                "0xfeaf968c" => format!(
                    "0x{}{}{}{}{}",
                    word(1),
                    word(25_000_000),
                    word(updated_at as u128),
                    word(updated_at as u128),
                    word(1)
                ),
                _ => "0x".to_string(),
            };
            axum::Json(json!({ "jsonrpc": "2.0", "id": req["id"], "result": result }))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}

/// The reader decodes the CreditsManager's balance and the aggregator's
/// round as `evm::oracle` does, and reports a failing endpoint, or a round
/// older than its freshness, as unavailable rather than failing the
/// summary. A stale round leaves the balance read.
#[tokio::test]
async fn the_chain_reader_reads_the_float_and_the_rate() {
    let reader = |url: String| super::daily_summary::ChainReader {
        chain: Arc::new(crate::evm::chain::ChainComponent::new(
            crate::http::Outbound::for_tests(),
            &url,
            std::time::Duration::from_secs(5),
        )),
        credits_manager: alloy_primitives::Address::repeat_byte(0x11),
        mana: alloy_primitives::Address::repeat_byte(0x22),
        aggregator: Some(alloy_primitives::Address::repeat_byte(0x33)),
        max_age_secs: 900,
    };
    let fresh = chrono::Utc::now().timestamp() - 60;
    let up = reader(rpc(false, fresh).await);
    assert_eq!(up.float_wei().await, mana(1_000));
    let quarter = U256::from(25u64) * U256::from(10u64).pow(U256::from(16u64));
    assert_eq!(up.mana_usd_18().await, Reading::Value(quarter));

    let stale = reader(rpc(false, fresh - 900).await);
    assert_eq!(stale.float_wei().await, mana(1_000));
    assert_eq!(stale.mana_usd_18().await, Reading::Unavailable);

    let down = reader(rpc(true, fresh).await);
    assert_eq!(down.float_wei().await, Reading::Unavailable);
    assert_eq!(down.mana_usd_18().await, Reading::Unavailable);

    let no_feed = super::daily_summary::ChainReader {
        aggregator: None,
        ..reader(rpc(false, fresh).await)
    };
    assert_eq!(no_feed.mana_usd_18().await, Reading::NotConfigured);
}

#[test]
fn the_hour_is_an_hour_of_the_day() {
    let hour = |h| {
        super::Settings::parse(super::RawSettings {
            writes_enabled: true,
            require_purchase_intent: false,
            mana_price_max_staleness_secs: 3600,
            daily_summary_hour_utc: h,
            ..super::RawSettings::default()
        })
    };
    assert_eq!(hour(23).unwrap().daily_summary_hour_utc, 23);
    assert!(hour(24).is_err());
    assert_eq!(
        super::Settings::for_tests().daily_summary_hour_utc,
        super::daily_summary::DEFAULT_HOUR_UTC
    );
}

/// Reads that fail are an error until [`LAST_CHANCE`] before the end of the
/// reporting day, so the pass is retried; from then on the day is queued
/// without its figures, and says so.
#[tokio::test]
async fn a_day_whose_reads_keep_failing_is_queued_without_figures() {
    let Some(db) = OwnDatabase::create("daily_summary_failing").await else {
        return;
    };
    let credits = Credits::new(db.pool.clone());
    let summary = DailySummary {
        hour_utc: 9,
        chain: Arc::new(NoChain),
    };
    sqlx::query("ALTER TABLE credits.payment_reversals RENAME TO payment_reversals_gone")
        .execute(&db.pool)
        .await
        .unwrap();

    assert!(
        summary
            .run_pass(&credits, at(21, 22, 54, 59))
            .await
            .is_err()
    );
    assert!(summaries(&db.pool).await.is_empty());

    let last = summary.run_pass(&credits, at(21, 22, 55, 0)).await.unwrap();
    assert_eq!(
        last,
        PassOutcome::Queued(NaiveDate::from_ymd_opt(2026, 11, 20).unwrap())
    );
    let rows = summaries(&db.pool).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].1["figuresUnavailable"], true);
    let text = message(&rows[0].1);
    assert!(
        text.starts_with(
            ":bar_chart: *Shop — 2026-11-20 (UTC)*\n:warning: The day's figures could not be read"
        ),
        "{text}"
    );
}

/// The cutoff is judged when the reads fail, not when the pass began: a
/// pass that starts just before it and fails just after it queues the day,
/// since a retry an hour later could fall on the next day.
#[tokio::test]
async fn a_pass_that_fails_past_the_cutoff_queues_the_day() {
    let Some(db) = OwnDatabase::create("daily_summary_cutoff").await else {
        return;
    };
    let credits = Credits::new(db.pool.clone());
    let summary = DailySummary {
        hour_utc: 9,
        chain: Arc::new(NoChain),
    };
    sqlx::query("ALTER TABLE credits.payment_reversals RENAME TO payment_reversals_gone")
        .execute(&db.pool)
        .await
        .unwrap();
    let reads = std::sync::atomic::AtomicU32::new(0);
    let clock = || {
        if reads.fetch_add(1, std::sync::atomic::Ordering::Relaxed) == 0 {
            at(21, 22, 54, 59)
        } else {
            at(21, 22, 55, 9)
        }
    };
    let outcome = summary.run_pass_on(&credits, clock).await.unwrap();
    assert_eq!(
        outcome,
        PassOutcome::Queued(NaiveDate::from_ymd_opt(2026, 11, 20).unwrap())
    );
    let rows = summaries(&db.pool).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, "2026-11-20");
    assert_eq!(rows[0].1["figuresUnavailable"], true);
}

/// A chain whose balance read lets another pass queue the day first.
struct Racing {
    pool: PgPool,
}

#[async_trait]
impl ChainFigures for Racing {
    async fn float_wei(&self) -> Reading {
        let mut conn = self.pool.acquire().await.unwrap();
        super::alerts::record(
            &mut conn,
            DAILY_SUMMARY,
            "2026-11-20",
            json!({ "day": "2026-11-20" }),
        )
        .await
        .unwrap();
        Reading::NotConfigured
    }
    async fn mana_usd_18(&self) -> Reading {
        Reading::NotConfigured
    }
}

/// A pass that another queued the day ahead of, after its check, adds
/// nothing and says the day was already queued.
#[tokio::test]
async fn a_pass_that_loses_the_race_reports_the_day_queued() {
    let Some(db) = OwnDatabase::create("daily_summary_race").await else {
        return;
    };
    let credits = Credits::new(db.pool.clone());
    let summary = DailySummary {
        hour_utc: 9,
        chain: Arc::new(Racing {
            pool: db.pool.clone(),
        }),
    };
    let outcome = summary.run_pass(&credits, at(21, 9, 0, 0)).await.unwrap();
    assert_eq!(
        outcome,
        PassOutcome::AlreadyQueued(NaiveDate::from_ymd_opt(2026, 11, 20).unwrap())
    );
    assert_eq!(summaries(&db.pool).await.len(), 1);
}
