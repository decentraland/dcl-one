//! The CreditUsed usage index (`usage.rs`) against a JSON-RPC stub of the
//! collections chain, on a database of its own (`tests::OwnDatabase`: the
//! expiry sweep and the reconciler walk every row): the bounded first pass,
//! the resume of a fresh index from the cursor alone, the fold that debits
//! each used credit once (a released and an expired pledge too), the
//! refused debit, the replay, the manager and chain guards, the
//! read-only pass, the expiry sweep held by the index, the authorize lane
//! over a ready and a stale index and the reconciler's CreditUsed pass. The
//! old test's `sql_capture` lock count is not carried (no capture hook).

use std::sync::Arc;

use axum::http::{HeaderMap, StatusCode};
use serde_json::{Value, json};
use sqlx::PgPool;

use super::errors::ApiError;
use super::store::Credits;
use super::tests::{
    MockBroker, OwnDatabase, app, assert_reconciles, available, call, scratch_wallet, seed_wallet,
    send, signed_headers,
};
use super::{RawSettings, Settings, SharedState, State};
use crate::http::Outbound;

/// A Polygon JSON-RPC stub: `eth_chainId`, `eth_getBlockByNumber` (the
/// `finalized` tag, `latest`, a number; block `n` is `t_base + 2n`) and
/// `eth_getLogs` refusing a window wider than `limit` blocks the way
/// rpc.decentraland.org does ("range N exceeds limit of L").
struct ChainStub {
    chain_id: i64,
    head: u64,
    finalized: u64,
    limit: u64,
    t_base: i64,
    logs: Vec<Value>,
    get_logs: usize,
}

type SharedChain = Arc<std::sync::Mutex<ChainStub>>;

async fn chain_rpc(
    axum::extract::State(chain): axum::extract::State<SharedChain>,
    axum::Json(req): axum::Json<Value>,
) -> axum::Json<Value> {
    let mut c = chain.lock().unwrap();
    let hex = |n: u64| format!("0x{n:x}");
    let parse =
        |v: &Value| u64::from_str_radix(v.as_str().unwrap().trim_start_matches("0x"), 16).unwrap();
    let result = match req["method"].as_str().unwrap() {
        "eth_chainId" => json!(hex(c.chain_id as u64)),
        "eth_getBlockByNumber" => {
            let n = match req["params"][0].as_str().unwrap() {
                "finalized" => c.finalized,
                "latest" => c.head,
                _ => parse(&req["params"][0]),
            };
            json!({ "number": hex(n), "hash": format!("0x{n:064x}"), "timestamp": hex((c.t_base + 2 * n as i64) as u64) })
        }
        "eth_getLogs" => {
            c.get_logs += 1;
            let (from, to) = (
                parse(&req["params"][0]["fromBlock"]),
                parse(&req["params"][0]["toBlock"]),
            );
            if to - from + 1 > c.limit {
                return axum::Json(
                    json!({ "jsonrpc": "2.0", "id": 1, "error": { "code": -32602, "message": format!("range {} exceeds limit of {}", to - from + 1, c.limit) } }),
                );
            }
            let address = req["params"][0]["address"][0].as_str().unwrap().to_string();
            json!(
                c.logs
                    .iter()
                    .filter(|l| l["address"] == address.as_str()
                        && (from..=to).contains(&parse(&l["blockNumber"])))
                    .cloned()
                    .collect::<Vec<_>>()
            )
        }
        other => panic!("unexpected {other}"),
    };
    axum::Json(json!({ "jsonrpc": "2.0", "id": 1, "result": result }))
}

fn word(n: u128) -> String {
    format!("{n:064x}")
}

/// A CreditUsed log of `contract` at `block`: the credit's value, a far
/// expiry, `salt`, and the consumed `value`.
fn credit_used_log(
    contract: &str,
    block: u64,
    index: u64,
    sender: &str,
    salt: &str,
    credit_value: u128,
    used: u128,
) -> Value {
    json!({
        "address": contract,
        "topics": [super::usage::CREDIT_USED_TOPIC, format!("0x{:0>64}", sender.trim_start_matches("0x")), format!("0x{}", word(block as u128 * 1000 + index as u128))],
        "data": format!("0x{}{}{}{}", word(credit_value), word(4_070_908_800), salt.trim_start_matches("0x"), word(used)),
        "blockNumber": format!("0x{block:x}"),
        "logIndex": format!("0x{index:x}"),
        "transactionHash": format!("0x{:064x}", block as u128 * 100 + index as u128),
        "blockTimestamp": null,
        "removed": false
    })
}

fn salt() -> String {
    format!("0x{}", uuid::Uuid::new_v4().simple().to_string().repeat(2))
}

fn usage_settings(
    rpc: &str,
    chain_id: i64,
    contract: &str,
    budget: usize,
) -> super::usage::Settings {
    super::usage::Settings {
        enabled: true,
        rpc_url: Some(rpc.to_string()),
        chain_id,
        contract: Some(contract.to_string()),
        start_block: 1000,
        requests_per_pass: budget,
        ..super::usage::Settings::default()
    }
}

async fn authorization(
    pool: &PgPool,
    id: &str,
    addr: &str,
    usd_cents: i64,
    status: &str,
    expires: chrono::NaiveDateTime,
) {
    sqlx::query("INSERT INTO credits.authorizations (id, address, usd_cents, amount_wei, status, expires_at) VALUES ($1, $2, $3, 1, $4, $5)")
        .bind(id)
        .bind(addr)
        .bind(usd_cents)
        .bind(status)
        .bind(expires)
        .execute(pool)
        .await
        .unwrap();
}

async fn balance_is(pool: &PgPool, addr: &str, want: &str) -> bool {
    sqlx::query_scalar("SELECT available = $2::numeric FROM credits.wallets WHERE address = $1")
        .bind(addr)
        .bind(want)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn auth_status(pool: &PgPool, id: &str) -> String {
    sqlx::query_scalar("SELECT status FROM credits.authorizations WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn the_usage_index_resumes_after_a_kill_folds_each_credit_once_and_gates_the_authorize_lane()
{
    let Some(db) = OwnDatabase::create("usage").await else {
        return;
    };
    let pool = db.pool.clone();
    let chain_id: i64 = 137;
    let contract = format!("0x{}", &uuid::Uuid::new_v4().simple().to_string()[..32]) + "00000000";
    let (w1, w2) = (scratch_wallet(), scratch_wallet());
    seed_wallet(&pool, &w1.address, "0", "200").await;
    seed_wallet(&pool, &w2.address, "0", "1").await;
    let later = super::store::now() + chrono::Duration::minutes(10);
    let (s1, s2, s3, s4, s5, foreign) = (salt(), salt(), salt(), salt(), salt(), salt());
    authorization(&pool, &s1, &w1.address, 500, "authorized", later).await;
    authorization(&pool, &s2, &w1.address, 300, "released", later).await;
    authorization(&pool, &s3, &w1.address, 200, "authorized", later).await;
    authorization(&pool, &s4, &w1.address, 200, "authorized", later).await;
    authorization(&pool, &s5, &w2.address, 500, "released", later).await;
    let t_base = chrono::Utc::now().timestamp() - 20_000;
    let chain: SharedChain = Arc::new(std::sync::Mutex::new(ChainStub {
        chain_id,
        head: 6_000,
        finalized: 5_990,
        limit: 1_000,
        t_base,
        logs: vec![
            credit_used_log(
                &contract,
                1_500,
                3,
                &w1.address,
                &s1,
                10u128.pow(19),
                10u128.pow(19),
            ),
            credit_used_log(
                &contract,
                1_600,
                0,
                &w1.address,
                &foreign,
                10u128.pow(19),
                10u128.pow(18),
            ),
            credit_used_log(
                &contract,
                2_600,
                1,
                &w1.address,
                &s2,
                6 * 10u128.pow(18),
                6 * 10u128.pow(18),
            ),
            credit_used_log(
                &contract,
                3_700,
                2,
                &w1.address,
                &s4,
                4 * 10u128.pow(18),
                10u128.pow(18),
            ),
            credit_used_log(
                &contract,
                3_800,
                0,
                &w1.address,
                &s4,
                4 * 10u128.pow(18),
                3 * 10u128.pow(18),
            ),
            credit_used_log(
                &contract,
                4_900,
                5,
                &w2.address,
                &s5,
                10u128.pow(19),
                10u128.pow(19),
            ),
            credit_used_log(
                "0x000000000000000000000000000000000000dead",
                2_000,
                0,
                &w1.address,
                &s3,
                1,
                1,
            ),
        ],
        get_logs: 0,
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let rpc = format!("http://{}", listener.local_addr().unwrap());
    let stub = axum::Router::new()
        .route("/", axum::routing::post(chain_rpc))
        .with_state(chain.clone());
    tokio::spawn(async move { axum::serve(listener, stub).await.unwrap() });
    let credits = || Credits::new(pool.clone()).with_usage_index(chain_id);
    let usages = || async {
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM credits.usages WHERE chain_id = 137")
            .fetch_one(&pool)
            .await
            .unwrap()
    };

    // A bounded first pass: the endpoint refuses the 4,991-block window and
    // names its limit, then two 1,000-block windows; the budget pauses it.
    let first = super::usage::UsageIndex::new(
        credits(),
        Outbound::for_tests(),
        usage_settings(&rpc, chain_id, &contract, 3),
    );
    let out = first.run_pass(None, true).await.unwrap();
    assert!(out.paused && !out.published, "{out:?}");
    assert_eq!(
        (out.requests, out.indexed_to, out.usages_written),
        (3, 2_999, 3),
        "{out:?}"
    );
    assert_eq!(usages().await, 3);
    assert!(first.ready_at(chrono::Utc::now()).await.is_none());
    assert_eq!(
        auth_status(&pool, &s1).await,
        "authorized",
        "a paused pass folds nothing"
    );

    // The kill -9 shape: a new index; only the cursor row survives.
    drop(first);
    let index = super::usage::UsageIndex::new(
        credits(),
        Outbound::for_tests(),
        usage_settings(&rpc, chain_id, &contract, 50),
    );
    let out = index.run_pass(None, true).await.unwrap();
    assert!(!out.paused && out.published, "{out:?}");
    assert_eq!(
        (out.indexed_to, out.usages_written, out.consumed, out.failed),
        (5_990, 3, 4, 1),
        "{out:?}: three purchases and s4's second use, a remainder"
    );
    assert_eq!(
        usages().await,
        6,
        "every log once: the resumed window re-read nothing, the other manager's log is not ours"
    );
    for (salt, want) in [
        (&s1, "consumed"),
        (&s2, "consumed"),
        (&s3, "authorized"),
        (&s4, "consumed"),
        (&s5, "released"),
    ] {
        assert_eq!(auth_status(&pool, salt).await, want, "{salt}");
    }
    assert!(
        balance_is(&pool, &w1.address, "100").await,
        "200 - 50 - 30 (a released pledge used on chain) - 20 (the purchase; s4's second use is a remainder on a pre-protocol row without a rate: settled at zero): {}",
        available(&pool, &w1.address).await
    );
    let settlements: Vec<(String, String)> = sqlx::query_as("SELECT authorization_id, kind FROM credits.usage_settlements ORDER BY block_number, log_index").fetch_all(&pool).await.unwrap();
    assert_eq!(
        settlements,
        vec![
            (s1.clone(), "purchase".into()),
            (s2.clone(), "purchase".into()),
            (s4.clone(), "purchase".into()),
            (s4.clone(), "remainder".into())
        ],
        "one settlement row per log of ours; the refused debit has none"
    );
    assert!(
        balance_is(&pool, &w2.address, "1").await,
        "a 50-credit debit the 1-credit balance cannot cover moves nothing"
    );
    let spends: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM credits.ledger WHERE kind = 'spend' AND tx_ref LIKE 'credit-used:%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(spends, 3);
    assert_reconciles(&pool, &w1.address, "after the fold").await;
    assert!(
        index.ready_at(chrono::Utc::now()).await.is_some(),
        "folded at the finalized block, just now"
    );

    // An idle pass re-publishes and moves nothing.
    let out = index.run_pass(None, true).await.unwrap();
    assert!(
        out.published && out.usages_written == 0 && out.consumed == 0,
        "{out:?}"
    );
    assert!(balance_is(&pool, &w1.address, "100").await);

    // The replay (CREDITS_USAGE_REPLAY_FROM_BLOCK = the start): the lane
    // closes, the rebuild re-reads the log and arrives at the same rows.
    let replay = super::usage::UsageIndex::new(
        credits(),
        Outbound::for_tests(),
        super::usage::Settings {
            replay_from_block: Some(1000),
            ..usage_settings(&rpc, chain_id, &contract, 50)
        },
    );
    super::usage::begin_replay(&pool, &replay.settings, &contract, 1000)
        .await
        .unwrap();
    assert!(
        replay.ready_at(chrono::Utc::now()).await.is_none(),
        "a rewound index is not trusted"
    );
    let out = replay.run_pass(None, true).await.unwrap();
    assert!(
        out.published && out.usages_written == 0 && out.consumed == 0 && out.failed == 1,
        "{out:?}"
    );
    assert_eq!(usages().await, 6);
    assert!(balance_is(&pool, &w1.address, "100").await);

    // Another manager on the same chain is refused until replayed.
    let other = super::usage::UsageIndex::new(
        credits(),
        Outbound::for_tests(),
        usage_settings(
            &rpc,
            chain_id,
            "0x00000000000000000000000000000000000000aa",
            50,
        ),
    );
    let err = other.run_pass(None, true).await.unwrap_err().to_string();
    assert!(err.contains("CREDITS_USAGE_REPLAY_FROM_BLOCK"), "{err}");
    // An endpoint serving another chain is refused before any log is read.
    let wrong = super::usage::UsageIndex::new(
        credits(),
        Outbound::for_tests(),
        super::usage::Settings {
            chain_id: 80_002,
            ..usage_settings(&rpc, chain_id, &contract, 50)
        },
    );
    let err = wrong.run_pass(None, true).await.unwrap_err().to_string();
    assert!(err.contains("serves chain 137"), "{err}");
    // With OPS_WRITES_ENABLED off the pass indexes but never folds or publishes.
    chain.lock().unwrap().finalized = 6_000;
    let read_only = super::usage::UsageIndex::new(
        credits(),
        Outbound::for_tests(),
        usage_settings(&rpc, chain_id, &contract, 50),
    );
    let out = read_only.run_pass(None, false).await.unwrap();
    assert!(!out.published && out.indexed_to == 6_000, "{out:?}");

    // The expiry sweep waits for the index: s3's pledge expired an hour ago
    // by the clock, but the chain has been read only to about 2.2 h ago.
    sqlx::query("UPDATE credits.authorizations SET expires_at = $2 WHERE id = $1")
        .bind(&s3)
        .bind(super::store::now() - chrono::Duration::hours(1))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(credits().expire_stale_authorizations().await.unwrap(), 0);
    assert_eq!(
        auth_status(&pool, &s3).await,
        "authorized",
        "the index has not read past its expiry"
    );
    let blocked = credits()
        .spend(&w1.address, "90", "checkout:held", None)
        .await
        .expect_err("the held pledge still counts against the balance");
    assert!(matches!(blocked, ApiError::PaymentRequired(_)));
    chain.lock().unwrap().finalized = 9_990;
    chain.lock().unwrap().head = 10_000;
    let out = index.run_pass(None, true).await.unwrap();
    assert!(out.published, "{out:?}");
    assert_eq!(
        credits().expire_stale_authorizations().await.unwrap(),
        1,
        "folded past the expiry: now it may lapse"
    );
    assert_eq!(auth_status(&pool, &s3).await, "expired");
    assert_eq!(
        Credits::new(pool.clone())
            .expire_stale_authorizations()
            .await
            .unwrap(),
        0,
        "without an index the clock alone decides"
    );

    // The authorize lane over the ready index, then the reconciler's CreditUsed pass.
    let signer_key = "0x4c0883a69102937d6231471b5dbb6204fe512961708279f2e3e8a5d4b8e3e3e3";
    let mut settings = Settings::parse(RawSettings {
        writes_enabled: true,
        credits_onchain_authorize_enabled: true,
        credits_signer_key: Some(signer_key.into()),
        credits_manager_contract: Some(contract.clone()),
        mana_price_max_staleness_secs: 3600,
        ..RawSettings::default()
    })
    .unwrap();
    settings.usage = usage_settings(&rpc, chain_id, &contract, 50);
    let collection = "0xeede64bfaf8055492aa500846ec7c6e6a9f533d5";
    let market = super::tests::FakeMarket::default()
        .with_item(collection, "7", "2000000000000000000", true)
        .with_mana_usd("0.5");
    let state: SharedState = Arc::new(
        State::with_sources(
            pool.clone(),
            Outbound::for_tests(),
            settings,
            Arc::new(market),
            Arc::new(MockBroker::configured()),
            None,
        )
        .with_gate(Arc::new(super::settlement::FixedGate(Ok(())))),
    );
    let lane = app(state.clone());
    let (status, v) = call(
        &lane,
        &w1,
        "post",
        "/credits/credits/authorize",
        &[],
        Some(json!({ "usdPriceCents": 100, "contractAddress": collection, "itemId": "7" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["credit"]["contract"], contract.as_str());
    assert_eq!(
        v["maxCreditedValue"], "3060000000000000000",
        "1.00 USD at 0.5 USD/MANA is 2 MANA; 1.02 buffer, 1.5 headroom"
    );
    let (status, _, v) = send(
        &lane,
        "GET",
        "/credits/usage-index/status",
        HeaderMap::new(),
        None,
    )
    .await;
    assert_eq!(
        (
            status,
            v["ready"].as_bool(),
            v["cursor"]["foldedBlock"].as_i64()
        ),
        (StatusCode::OK, Some(true), Some(9_990)),
        "{v}"
    );
    let report = state.reconcile_report().await.unwrap();
    assert_eq!(report.credit_usage_index, "ready");
    let unpaid: Vec<&str> = report
        .credit_usages_reconciled
        .iter()
        .filter(|d| !d.consumed)
        .map(|d| d.credit_id.as_str())
        .collect();
    assert_eq!(
        unpaid,
        [s5.as_str()],
        "the used credit the wallet could not cover is listed, retried and still refused"
    );
    assert!(!report.ok);

    // A stale fold closes the lane again: 503 naming the source, nothing reserved.
    sqlx::query("UPDATE credits.usage_index_cursor SET folded_at = folded_at - interval '11 minutes' WHERE chain_id = 137").execute(&pool).await.unwrap();
    let (status, headers, v) = {
        let headers = signed_headers(&w1, "post", "/credits/credits/authorize").await;
        send(
            &lane,
            "POST",
            "/credits/credits/authorize",
            headers,
            Some(json!({ "usdPriceCents": 100, "contractAddress": collection, "itemId": "7" })),
        )
        .await
    };
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{v}");
    assert_eq!(
        (v["source"].as_str(), v["configure"].as_str()),
        (
            Some("credits-usage-index"),
            Some("CREDITS_USAGE_INDEX_ENABLED")
        )
    );
    assert_eq!(headers.get("retry-after").unwrap(), "60");
    let pledges: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM credits.authorizations WHERE address = $1 AND contract IS NOT NULL",
    )
    .bind(&w1.address)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pledges, 1, "the stale lane reserved nothing new");

    db.drop().await;
}

#[test]
fn the_tick_and_the_sweep_share_one_expiry_bound() {
    assert!(super::checkout::OUTBOX_TICK_SQL.contains(&super::usage::expiry_bound_sql("$2", "$5")));
}

/// `cargo test --lib credits::tests::live -- --ignored`: the CreditUsed
/// index over the last 50,000 Polygon blocks of the production manager
/// through rpc.decentraland.org/polygon (10,000-block windows), killed after
/// its first window and resumed by a fresh index: no duplicate, published.
#[tokio::test]
#[ignore = "reads rpc.decentraland.org/polygon"]
async fn live_usage_index_over_the_production_manager() {
    let Some(db) = OwnDatabase::create("usagelive").await else {
        return;
    };
    let http = Outbound::new(crate::http::HttpClientCfg::default()).unwrap();
    let rpc = "https://rpc.decentraland.org/polygon";
    let contract = "0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5";
    let probe = super::usage::Rpc::at(&http, rpc);
    let finalized = super::usage::finalized(&probe).await.unwrap();
    let settings = |budget: usize| super::usage::Settings {
        enabled: true,
        rpc_url: Some(rpc.to_string()),
        chain_id: 137,
        contract: Some(contract.to_string()),
        start_block: finalized.number - 50_000,
        requests_per_pass: budget,
        ..super::usage::Settings::default()
    };
    let credits = Credits::new(db.pool.clone()).with_usage_index(137);
    let first = super::usage::UsageIndex::new(credits.clone(), http.clone(), settings(1));
    let started = std::time::Instant::now();
    let out = first.run_pass(None, true).await.unwrap();
    eprintln!(
        "pass 1 (killed after one request): {out:?} in {:?}",
        started.elapsed()
    );
    drop(first);
    let index = super::usage::UsageIndex::new(credits, http, settings(20));
    let started = std::time::Instant::now();
    let out = index.run_pass(None, true).await.unwrap();
    let (rows, distinct): (i64, i64) = sqlx::query_as(
        "SELECT count(*), count(DISTINCT (block_number, log_index)) FROM credits.usages",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    eprintln!(
        "pass 2 (resumed): {out:?} in {:?}; {rows} CreditUsed logs, {distinct} distinct",
        started.elapsed()
    );
    assert!(out.published && rows == distinct, "{out:?}");
    assert!(index.ready_at(chrono::Utc::now()).await.is_some());
    db.drop().await;
}
