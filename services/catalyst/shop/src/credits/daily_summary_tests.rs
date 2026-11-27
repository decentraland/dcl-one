//! The credits daily summary (`daily_summary.rs`): the hour and the day's
//! claim, the figures of a closed UTC day read from rows on both sides of
//! its midnights, a chain that cannot be read, and the message.

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
    // A replica past the EXISTS check inserts nothing either.
    let payload = figures(&credits, day(20), at(21, 9, 0, 0)).await.unwrap();
    let mut conn = db.pool.acquire().await.unwrap();
    super::alerts::record(&mut conn, DAILY_SUMMARY, "2026-11-20", payload)
        .await
        .unwrap();
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
    let (a1, a2, b0, b1, c1) = (salt(), salt(), salt(), salt(), salt());
    for (auth, who) in [(&a1, &a), (&a2, &a), (&b0, &b), (&b1, &b), (&c1, &c)] {
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
    for (intent, cents, when) in [
        ("pi_in", 500, at(20, 8, 0, 0)),
        ("pi_after", 900, at(21, 0, 0, 0)),
    ] {
        sqlx::query("INSERT INTO credits.payment_reversals (payment_intent, refunded_cents, updated_at) VALUES ($1, $2, $3)")
            .bind(intent)
            .bind(cents as i64)
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
            "outflow7dWei": "9100000000000000000",
            "integrity": { "expired": 1, "reversals": 1, "reversedCents": 500 },
        })
    );
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
        "integrity": { "expired": 1, "reversals": 1, "reversedCents": 500 },
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
            ":bank: *Unspent*  $2.70 in credits sold and not yet spent",
            ":fuelpump: *Float now*  CreditsManager 1,000 MANA ($250 · 2,500 credits)",
            ":shield: *Coverage*  78.1× the $3.20 owed in unspent credits · ~100 days at the 7-day pace (10 MANA/day)",
            "",
            "1 reservation expired unused · :warning: 1 refund/chargeback ($5.00)",
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
        "integrity": { "expired": 0, "reversals": 0, "reversedCents": 0 },
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
            ":bank: *Unspent*  $0.00 in credits sold and not yet spent",
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
async fn rpc(broken: bool) -> String {
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
                    word(1_700_000_000),
                    word(1_700_000_000),
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
/// round as `evm::oracle` does, and reports a failing endpoint as
/// unavailable rather than failing the summary.
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
    };
    let up = reader(rpc(false).await);
    assert_eq!(up.float_wei().await, mana(1_000));
    let quarter = U256::from(25u64) * U256::from(10u64).pow(U256::from(16u64));
    assert_eq!(up.mana_usd_18().await, Reading::Value(quarter));

    let down = reader(rpc(true).await);
    assert_eq!(down.float_wei().await, Reading::Unavailable);
    assert_eq!(down.mana_usd_18().await, Reading::Unavailable);

    let no_feed = super::daily_summary::ChainReader {
        aggregator: None,
        ..reader(rpc(false).await)
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
