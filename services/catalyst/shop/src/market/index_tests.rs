//! The marketplace chain indexes on a scratch database per test (created on
//! the `CATALYST_TEST_DATABASE_URL` cluster with the real schema, dropped at
//! the end; skipped without the variable): the walk against a fake chain
//! (window and address limits, the request budget, a kill between passes,
//! the confirmation depth and the publish rule, a reorg rewind, a late
//! collection's backfill), the folds (trade verdicts, nft owners, legacy
//! orders, names, LAND data), and the routes that read them before and
//! after readiness (the envelope, then the answer), plus the neighbours
//! build and the coupon reads, writes and state refresh.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use alloy_primitives::{Address, FixedBytes, U256, keccak256};
use alloy_sol_types::SolValue;
use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use axum::routing::post;
use parking_lot::Mutex;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use tower::ServiceExt;

use super::chain_walk::{
    self, ChainIndex, ChainNet, INDEXER_NFT, INDEXER_SALES, INDEXER_TRADE_EVENTS, IndexSettings,
    PassOutcome,
};
use super::listings::{self, RpcEndpoints};
use super::nft_index::{self, NftIndex};
use super::sales_index::{self, SalesIndex};
use super::trade_events::{self, TradeEventsIndex};
use super::{MarketSettings, MarketplaceState, Settings, SharedState};
use crate::chain_index::rpc::Rpc;

struct ScratchDb {
    name: String,
    pool: PgPool,
    admin: PgPool,
}

impl ScratchDb {
    async fn create() -> Option<Self> {
        let Ok(base) = std::env::var("CATALYST_TEST_DATABASE_URL") else {
            eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
            return None;
        };
        let opts: PgConnectOptions = base
            .parse()
            .expect("CATALYST_TEST_DATABASE_URL must be a postgres DSN");
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect_with(opts.clone().database("postgres"))
            .await
            .unwrap();
        let name = format!(
            "catalyst_z_mkt_{}",
            &uuid::Uuid::new_v4().simple().to_string()[..12]
        );
        crate::db::scratch::create_database_with_schema(&admin, &name).await;
        let mut url = url::Url::parse(&base).unwrap();
        url.set_path(&format!("/{name}"));
        let pool = crate::db::connect_pool(
            url.as_str(),
            &crate::db::PoolSettings {
                max_connections: 4,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        Some(Self { name, pool, admin })
    }

    async fn drop(self) {
        self.pool.close().await;
        let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE \"{}\" WITH (FORCE)",
            self.name
        )))
        .execute(&self.admin)
        .await;
    }
}

/// A JSON-RPC chain with a fixed log set, the two public backends' limits
/// (a window wider than `max_span` refused with "exceeds limit of N", more
/// than `max_addresses` refused without a range) and a block hash per block
/// that a test can move (a reorg).
struct FakeChain {
    head: Mutex<u64>,
    finalized_lag: std::sync::atomic::AtomicU64,
    finalized_available: std::sync::atomic::AtomicBool,
    reorg_on_logs: std::sync::atomic::AtomicBool,
    logs: Mutex<Vec<Value>>,
    max_span: u64,
    max_addresses: usize,
    forks: Mutex<HashMap<u64, String>>,
    requests: AtomicUsize,
    get_logs: AtomicUsize,
    /// Answer eth_getLogs without `blockTimestamp` (the Polygon backends that do).
    strip_timestamps: std::sync::atomic::AtomicBool,
    batches: AtomicUsize,
    drop_in_batch: Mutex<Vec<u64>>,
    /// Every request in arrival order, as [`call_tag`] names it.
    calls: Mutex<Vec<String>>,
    /// What `eth_chainId` answers (137 unless a test moves it).
    chain_id: std::sync::atomic::AtomicU64,
    /// Refuse every header read by number (a backend that does not answer).
    fail_headers: std::sync::atomic::AtomicBool,
    /// Refuse every `eth_getLogs` whose first topics include all of these
    /// (a union of two projections' topics), when there are any.
    refuse_topics: Mutex<Vec<String>>,
    /// The `eth_getLogs` refused for `refuse_topics` or `refuse_from`.
    refused: AtomicUsize,
    /// Refuse every `eth_getLogs` from this block on that asks for this
    /// address.
    refuse_from: Mutex<Option<(u64, String)>>,
}

impl FakeChain {
    fn new(
        head: u64,
        logs: Vec<crate::chain_index::rpc::RpcLog>,
        max_span: u64,
        max_addresses: usize,
    ) -> Arc<Self> {
        Arc::new(Self {
            head: Mutex::new(head),
            finalized_lag: std::sync::atomic::AtomicU64::new(64),
            finalized_available: std::sync::atomic::AtomicBool::new(true),
            reorg_on_logs: std::sync::atomic::AtomicBool::new(false),
            logs: Mutex::new(logs.iter().map(log_json).collect()),
            max_span,
            max_addresses,
            forks: Mutex::new(HashMap::new()),
            requests: AtomicUsize::new(0),
            get_logs: AtomicUsize::new(0),
            strip_timestamps: std::sync::atomic::AtomicBool::new(false),
            batches: AtomicUsize::new(0),
            drop_in_batch: Mutex::new(Vec::new()),
            calls: Mutex::new(Vec::new()),
            chain_id: std::sync::atomic::AtomicU64::new(137),
            fail_headers: std::sync::atomic::AtomicBool::new(false),
            refuse_topics: Mutex::new(Vec::new()),
            refused: AtomicUsize::new(0),
            refuse_from: Mutex::new(None),
        })
    }

    fn hash_of(&self, block: u64) -> String {
        self.forks
            .lock()
            .get(&block)
            .cloned()
            .unwrap_or_else(|| format!("0x{:064x}", block))
    }
}

fn log_json(l: &crate::chain_index::rpc::RpcLog) -> Value {
    json!({
        "address": l.address, "topics": l.topics, "data": l.data, "blockNumber": l.block_number, "logIndex": l.log_index,
        "blockTimestamp": l.block_timestamp, "transactionHash": l.transaction_hash.clone().unwrap_or_else(|| format!("0x{:064x}", 7)),
    })
}

fn hex_u64(v: &Value) -> u64 {
    u64::from_str_radix(v.as_str().unwrap().trim_start_matches("0x"), 16).unwrap()
}

/// The fake chain's block time: two seconds a block from 1,600,000,000.
fn time_of(block: u64) -> i64 {
    1_600_000_000 + 2 * block as i64
}

/// One request as the request-order test pins it: the method and the
/// parameters that decide what it reads.
fn call_tag(body: &Value) -> String {
    let p = &body["params"];
    match body["method"].as_str().unwrap_or_default() {
        "eth_blockNumber" => "head".into(),
        "eth_chainId" => "chain id".into(),
        "eth_call" => "call".into(),
        "eth_getBlockByNumber" if p[0] == "finalized" => "finalized".into(),
        "eth_getBlockByNumber" => format!("header {}", hex_u64(&p[0])),
        "eth_getLogs" => {
            let f = &p[0];
            let addresses = match &f["address"] {
                Value::Array(a) => a.len(),
                Value::String(_) => 1,
                _ => 0,
            };
            format!(
                "logs {}..{} x{addresses}",
                hex_u64(&f["fromBlock"]),
                hex_u64(&f["toBlock"])
            )
        }
        other => other.to_string(),
    }
}

async fn rpc(
    State(chain): State<Arc<FakeChain>>,
    axum::Json(body): axum::Json<Value>,
) -> axum::response::Response {
    chain.requests.fetch_add(1, Ordering::SeqCst);
    let tag = match &body {
        Value::Array(calls) => format!(
            "batch [{}]",
            calls.iter().map(call_tag).collect::<Vec<_>>().join(", ")
        ),
        one => call_tag(one),
    };
    chain.calls.lock().push(tag);
    match body {
        Value::Array(calls) => {
            chain.batches.fetch_add(1, Ordering::SeqCst);
            // A batch: every element answered, except the one a flaky
            // backend drops (the client asks it again alone).
            let answers: Vec<Value> = calls
                .iter()
                .filter(|c| {
                    !chain
                        .drop_in_batch
                        .lock()
                        .contains(&hex_u64(&c["params"][0]))
                })
                .map(|c| answer(&chain, c))
                .collect();
            axum::Json(Value::Array(answers)).into_response()
        }
        one => axum::Json(answer(&chain, &one)).into_response(),
    }
}

fn answer(chain: &FakeChain, body: &Value) -> Value {
    let id = body["id"].clone();
    let reply = |result: Value| json!({ "jsonrpc": "2.0", "id": id, "result": result });
    let refuse = |message: String| json!({ "jsonrpc": "2.0", "id": body["id"], "error": { "code": -32602, "message": message } });
    match body["method"].as_str() {
        Some("eth_blockNumber") => reply(json!(format!("0x{:x}", *chain.head.lock()))),
        Some("eth_chainId") => reply(json!(format!(
            "0x{:x}",
            chain.chain_id.load(Ordering::SeqCst)
        ))),
        // `royaltiesCutPerMillion()` of any venue: the pinned 2.5 %.
        Some("eth_call") if body["params"][0]["data"] == sales_index::ROYALTIES_CUT_SELECTOR => {
            reply(json!(format!("0x{:064x}", 25_000)))
        }
        Some("eth_getBlockByNumber") => {
            let n = if body["params"][0] == "finalized" {
                if !chain.finalized_available.load(Ordering::SeqCst) {
                    return reply(Value::Null);
                }
                chain
                    .head
                    .lock()
                    .saturating_sub(chain.finalized_lag.load(Ordering::SeqCst))
            } else {
                if chain.fail_headers.load(Ordering::SeqCst) {
                    return json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32000, "message": "header unavailable" } });
                }
                hex_u64(&body["params"][0])
            };
            reply(
                json!({ "number": format!("0x{n:x}"), "hash": chain.hash_of(n), "timestamp": format!("0x{:x}", time_of(n)) }),
            )
        }
        Some("eth_getLogs") => {
            chain.get_logs.fetch_add(1, Ordering::SeqCst);
            let f = &body["params"][0];
            let (from, to) = (hex_u64(&f["fromBlock"]), hex_u64(&f["toBlock"]));
            if chain.reorg_on_logs.swap(false, Ordering::SeqCst) {
                chain
                    .forks
                    .lock()
                    .insert(to, format!("0x{}", "ee".repeat(32)));
            }
            if to - from + 1 > chain.max_span {
                return refuse(format!(
                    "range {} exceeds limit of {}",
                    to - from + 1,
                    chain.max_span
                ));
            }
            let addresses: Vec<String> = match &f["address"] {
                Value::String(a) => vec![a.to_ascii_lowercase()],
                Value::Array(a) => a
                    .iter()
                    .map(|v| v.as_str().unwrap().to_ascii_lowercase())
                    .collect(),
                _ => vec![],
            };
            if addresses.len() > chain.max_addresses {
                return refuse("exceed max addresses or topics per search position".into());
            }
            // The first topics, as `eth_getLogs` reads them: a string is one
            // topic, a list any of them, none (or null) any topic at all.
            let topics: Option<Vec<String>> = match &f["topics"][0] {
                Value::String(t) => Some(vec![t.to_ascii_lowercase()]),
                Value::Array(a) => Some(
                    a.iter()
                        .map(|t| t.as_str().unwrap().to_ascii_lowercase())
                        .collect(),
                ),
                _ => None,
            };
            if let Some((block, address)) = chain.refuse_from.lock().clone()
                && from >= block
                && addresses.contains(&address.to_ascii_lowercase())
            {
                chain.refused.fetch_add(1, Ordering::SeqCst);
                return json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32000, "message": "internal error: this address is refused by the test" } });
            }
            let refused = chain.refuse_topics.lock().clone();
            if !refused.is_empty()
                && topics
                    .as_ref()
                    .is_some_and(|t| refused.iter().all(|r| t.contains(&r.to_ascii_lowercase())))
            {
                chain.refused.fetch_add(1, Ordering::SeqCst);
                return json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32000, "message": "internal error: this filter is refused by the test" } });
            }
            let logs = chain.logs.lock();
            let hits: Vec<Value> = logs
                .iter()
                .filter(|l| (from..=to).contains(&hex_u64(&l["blockNumber"])))
                .filter(|l| {
                    addresses.contains(&l["address"].as_str().unwrap().to_ascii_lowercase())
                })
                .filter(|l| {
                    topics.as_ref().is_none_or(|t| {
                        t.contains(&l["topics"][0].as_str().unwrap().to_ascii_lowercase())
                    })
                })
                .cloned()
                .collect();
            if chain.strip_timestamps.load(Ordering::SeqCst) {
                let stripped: Vec<Value> = hits
                    .into_iter()
                    .map(|mut l| {
                        l.as_object_mut().unwrap().remove("blockTimestamp");
                        l
                    })
                    .collect();
                return reply(Value::Array(stripped));
            }
            reply(Value::Array(hits))
        }
        other => {
            json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": format!("unexpected method {other:?}") } })
        }
    }
}

async fn serve(chain: Arc<FakeChain>) -> String {
    let app = Router::new().route("/", post(rpc)).with_state(chain);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}/")
}

fn index_settings(budget: usize, window: u64) -> IndexSettings {
    IndexSettings {
        nft_enabled: true,
        trade_events_enabled: true,
        requests_per_pass: budget,
        window_blocks: window,
        ..IndexSettings::default()
    }
}

fn state_over(pool: PgPool, url: &str, indexes: IndexSettings, writes: bool) -> SharedState {
    let market = MarketSettings {
        trade_rpc_urls: RpcEndpoints::from_env(Some(&format!("137={url},1={url}"))),
        ..MarketSettings::default()
    }
    .with_indexes(indexes);
    let settings = Settings {
        data_writes_enabled: writes,
        matic_collections_subgraph_url: Some("https://subgraph.example/matic".into()),
        ..Settings::for_tests()
    }
    .with_market(market);
    Arc::new(MarketplaceState::new(
        pool,
        crate::http::Outbound::for_tests(),
        settings,
    ))
}

/// `st` with its trades mirrored from an upstream (no pass recorded yet).
fn trades_from_upstream(mut st: SharedState) -> SharedState {
    Arc::get_mut(&mut st)
        .expect("an unshared state")
        .settings
        .trades_sync_upstream_url = Some("https://marketplace-api.example/v1/trades".into());
    st
}

/// The trades mirror's last pass completed `ago` before now.
async fn trades_pass_ended(st: &MarketplaceState, ago: std::time::Duration) {
    sqlx::query(
        "INSERT INTO shop.trades_mirror (id, upstream_url, pass, completed_at) \
         VALUES (1, $1, 1, timezone('UTC', now()) - make_interval(secs => $2)) \
         ON CONFLICT (id) DO UPDATE SET upstream_url = EXCLUDED.upstream_url, completed_at = EXCLUDED.completed_at",
    )
    .bind(st.settings.trades_sync_upstream_url.as_deref())
    .bind(ago.as_secs_f64())
    .execute(&st.pool)
    .await
    .unwrap();
    st.trades_ready.clear();
}

async fn pass(state: &MarketplaceState, url: &str, ix: &dyn ChainIndex) -> PassOutcome {
    try_pass(state, url, ix).await.unwrap()
}

/// One pass as a process without leases runs it: the projection claimed on
/// the pool, then the walk.
async fn try_pass(
    state: &MarketplaceState,
    url: &str,
    ix: &dyn ChainIndex,
) -> anyhow::Result<PassOutcome> {
    let source = chain_walk::RpcSource::new(Rpc::at(&state.http, url));
    let owner = chain_walk::test_owner(&state.pool, ix, &state.settings.market.indexes).await;
    chain_walk::run_pass(
        &state.pool,
        &source,
        ix,
        &state.settings.market.indexes,
        &owner,
        None,
        &|_| {},
    )
    .await
}

/// Whether `e` is a write refused because its job no longer owns the projection.
fn fenced(e: &anyhow::Error) -> bool {
    e.chain()
        .any(|c| c.downcast_ref::<chain_walk::Fenced>().is_some())
}

async fn get_json(app: &Router, path: &str) -> (StatusCode, Value) {
    let resp = app
        .clone()
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 22)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn topic_addr(a: &str) -> String {
    format!("0x{:0>64}", a.trim_start_matches("0x"))
}

fn topic_word(v: U256) -> String {
    format!("0x{v:064x}")
}

fn log_at(
    address: &str,
    topics: Vec<String>,
    data: Vec<u8>,
    block: u64,
    index: u64,
) -> crate::chain_index::rpc::RpcLog {
    crate::chain_index::rpc::RpcLog {
        address: address.to_string(),
        topics,
        data: format!("0x{}", hex::encode(data)),
        block_number: format!("0x{block:x}"),
        log_index: format!("0x{index:x}"),
        block_timestamp: Some(format!("0x{:x}", time_of(block))),
        block_hash: None,
        transaction_hash: Some(format!("0x{:064x}", block * 100 + index)),
    }
}

const ALICE: &str = "0xaaaa000000000000000000000000000000000001";
const BOB: &str = "0xbbbb000000000000000000000000000000000002";
const CARL: &str = "0xcccc000000000000000000000000000000000003";
const DAVE: &str = "0xdddd000000000000000000000000000000000004";
const ZERO: &str = "0x0000000000000000000000000000000000000000";

/// A stored trade with one sent and one received asset; returns (id, hashed signature).
#[allow(
    clippy::too_many_arguments,
    reason = "fixture: one argument per seeded trade field"
)]
async fn seed_trade(
    pool: &PgPool,
    kind: &str,
    signer: &str,
    contract: &str,
    uses: u64,
    signer_index: u64,
    sent: (i32, &str, &str),
    received: (i32, &str, &str),
) -> (String, String) {
    let signature = format!(
        "0x{}1b",
        hex::encode(uuid::Uuid::new_v4().as_bytes()).repeat(4)
    );
    let hashed = listings::hashed_signature(&signature);
    let id: String = sqlx::query_scalar(
        "INSERT INTO shop.trades (network, chain_id, signature, hashed_signature, checks, signer, type, expires_at, effective_since, contract) \
         VALUES ('MATIC', 137, $1, $2, $3, $4, $5::shop.trade_type, timezone('UTC', now()) + interval '30 days', timezone('UTC', now()) - interval '1 day', $6) \
         RETURNING id::text",
    )
    .bind(&signature)
    .bind(&hashed)
    .bind(json!({ "uses": uses, "expiration": 4_000_000_000_000i64, "effective": 0, "salt": "0x01", "contractSignatureIndex": 0, "signerSignatureIndex": signer_index, "allowedRoot": "0x", "externalChecks": [] }))
    .bind(signer)
    .bind(kind)
    .bind(contract)
    .fetch_one(pool)
    .await
    .unwrap();
    for (direction, (asset_type, addr, value)) in [("sent", sent), ("received", received)] {
        let asset: String = sqlx::query_scalar(
            "INSERT INTO shop.trade_assets (trade_id, direction, asset_type, contract_address, beneficiary, extra) \
             VALUES ($1::uuid, $2::shop.asset_direction_type, $3, $4, $5, '0x') RETURNING id::text",
        )
        .bind(&id)
        .bind(direction)
        .bind(asset_type as i16)
        .bind(addr)
        .bind(if direction == "received" { Some(signer) } else { None })
        .fetch_one(pool)
        .await
        .unwrap();
        let (table, column) = match asset_type {
            1 | 2 => ("trade_assets_erc20", "amount"),
            3 => ("trade_assets_erc721", "token_id"),
            _ => ("trade_assets_item", "item_id"),
        };
        let cast = if column == "amount" { "::numeric" } else { "" };
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "INSERT INTO shop.{table} (asset_id, {column}) VALUES ($1::uuid, $2{cast})"
        )))
        .bind(&asset)
        .bind(value)
        .execute(pool)
        .await
        .unwrap();
    }
    (id, hashed)
}

fn traded(
    block: u64,
    index: u64,
    market: &str,
    caller: &str,
    signer: &str,
    hashed: &str,
) -> crate::chain_index::rpc::RpcLog {
    let mut log =
        trade_events::tests::traded_log(block, index, market, caller, signer, "0x01", 1, false);
    log.topics[2] = hashed.to_string();
    log.block_timestamp = Some(format!("0x{:x}", time_of(block)));
    log
}

fn cancelled(
    block: u64,
    market: &str,
    caller: &str,
    hashed: &str,
) -> crate::chain_index::rpc::RpcLog {
    log_at(
        market,
        vec![
            trade_events::TOPIC_SIGNATURE_CANCELLED.into(),
            topic_addr(caller),
            hashed.to_string(),
        ],
        vec![],
        block,
        0,
    )
}

fn signer_index(
    block: u64,
    market: &str,
    signer: &str,
    value: u64,
) -> crate::chain_index::rpc::RpcLog {
    log_at(
        market,
        vec![
            trade_events::TOPIC_SIGNER_INDEX_INCREASED.into(),
            topic_addr(signer),
            topic_word(U256::from(value)),
        ],
        vec![],
        block,
        0,
    )
}

async fn status_of(pool: &PgPool, id: &str) -> Option<String> {
    sqlx::query_scalar("SELECT status FROM shop.trade_chain_state WHERE trade_id = $1::uuid")
        .bind(id)
        .fetch_optional(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn the_trade_events_walk_refuses_an_endpoint_of_another_chain() {
    let url = serve(FakeChain::new(1_000, vec![], 10_000, 1_000)).await;
    let http = crate::http::Outbound::for_tests();
    let source = chain_walk::RpcSource::new(Rpc::at(&http, &url));
    super::ensure_chain(&source, 137).await.unwrap();
    let refused = super::ensure_chain(&source, 80_002)
        .await
        .unwrap_err()
        .to_string();
    assert!(refused.contains("serves chain 137, not 80002"), "{refused}");
}

#[tokio::test]
async fn trade_events_walk_under_a_budget_resume_after_a_kill_fold_the_verdicts_and_answer_accept()
{
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let market = trade_events::marketplaces(ChainNet::Matic)[1].0;
    let start = TradeEventsIndex::mainnet(ChainNet::Matic).start_block();
    let item = "0x00000000000000000000000000000000000000c1";
    let mana = "0xa1c57f48f0deb89f569dfbe6e2b7f46d33606fd4";
    let sold = seed_trade(
        &db.pool,
        "public_item_order",
        ALICE,
        market,
        1,
        0,
        (4, item, "0"),
        (1, mana, "1000"),
    )
    .await;
    let by_signer = seed_trade(
        &db.pool,
        "public_item_order",
        ALICE,
        market,
        1,
        0,
        (4, item, "1"),
        (1, mana, "1000"),
    )
    .await;
    let by_stranger = seed_trade(
        &db.pool,
        "public_item_order",
        ALICE,
        market,
        1,
        0,
        (4, item, "2"),
        (1, mana, "1000"),
    )
    .await;
    let index_moved = seed_trade(
        &db.pool,
        "public_item_order",
        BOB,
        market,
        1,
        0,
        (4, item, "3"),
        (1, mana, "1000"),
    )
    .await;
    let open = seed_trade(
        &db.pool,
        "public_item_order",
        ALICE,
        market,
        2,
        0,
        (4, item, "4"),
        (1, mana, "1000"),
    )
    .await;
    let bid = seed_trade(
        &db.pool,
        "bid",
        CARL,
        market,
        1,
        0,
        (1, mana, "5"),
        (4, item, "0"),
    )
    .await;
    sqlx::query(
        "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, total_supply, max_supply, available, price, image, name, category, created_at, updated_at, reviewed_at, mirrored_at) \
         VALUES ('matic', $1, 0, 137, 'wearable_v2', 'urn:decentraland:matic:collections-v2:0xc1:0', $2, 'epic', 1, 10, 9, 1000, 'https://img/0', 'Hat', 'hat', 1, 1, 1, timezone('UTC', now()))",
    )
    .bind(item)
    .bind(ALICE)
    .execute(&db.pool)
    .await
    .unwrap();
    // Each item order is its creator's primary sale, so every verdict is read.
    for (item_id, creator) in [(1, ALICE), (2, ALICE), (3, BOB), (4, ALICE)] {
        sqlx::query(
            "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, total_supply, max_supply, available, price, image, name, category, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ('matic', $1, $2::int4, 137, 'wearable_v2', 'urn:decentraland:matic:collections-v2:0xc1:' || $2::int4::text, $3, 'epic', 1, 10, 9, 1000, 'https://img/0', 'Hat', 'hat', 1, 1, 1, timezone('UTC', now()))",
        )
        .bind(item)
        .bind(item_id)
        .bind(creator)
        .execute(&db.pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, is_completed, is_approved, is_editable, minters, created_at, updated_at, reviewed_at, mirrored_at) \
         VALUES ('matic', $1, 137, 'Hats', $2, $2, 'urn:decentraland:matic:collections-v2:0xc1', true, true, false, ARRAY[$3], 1, 1, 1, timezone('UTC', now()))",
    )
    .bind(item)
    .bind(ALICE)
    .bind(market)
    .execute(&db.pool)
    .await
    .unwrap();
    let s = start;
    let logs = vec![
        traded(s + 100, 0, market, CARL, ALICE, &sold.1),
        traded(s + 150, 1, market, BOB, ALICE, &open.1),
        cancelled(s + 2_000, market, ALICE, &by_signer.1),
        cancelled(s + 3_000, market, CARL, &by_stranger.1),
        signer_index(s + 4_000, market, BOB, 1),
        traded(s + 5_000, 0, market, ALICE, CARL, &bid.1),
    ];
    let head = s + 6_000 + 64;
    let chain = FakeChain::new(head, logs, 1_000, 10);
    let url = serve(chain.clone()).await;

    // Pass 1, budget 3: the refused 10,000-block window narrows to 1,000, a
    // few windows land, the cursor is durable, nothing is published and the
    // accept route answers the envelope.
    let st = state_over(db.pool.clone(), &url, index_settings(4, 10_000), false);
    let app = super::market_router(st.clone());
    let first = pass(&st, &url, &TradeEventsIndex::mainnet(ChainNet::Matic)).await;
    assert!(first.paused && !first.published, "{first:?}");
    assert!(first.indexed_to >= s as i64 && first.indexed_to < head as i64);
    let (code, body) = get_json(&app, &format!("/v1/trades/{}/accept", sold.1)).await;
    assert_eq!(
        (code, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("trade-events-index")),
        "{body}"
    );
    assert_eq!(
        get_json(&app, &format!("/v1/trades/0x{}/accept", "ab".repeat(32)))
            .await
            .0,
        StatusCode::NOT_FOUND,
        "an unknown trade is the 404 even before readiness"
    );

    // A kill: a new state and index resume from the durable cursor.
    let st = state_over(db.pool.clone(), &url, index_settings(4, 10_000), false);
    let mut last = PassOutcome::default();
    for _ in 0..20 {
        last = pass(&st, &url, &TradeEventsIndex::mainnet(ChainNet::Matic)).await;
        if last.published {
            break;
        }
    }
    assert!(last.published, "{last:?}");
    let (rows, distinct): (i64, i64) = sqlx::query_as(
        "SELECT count(*), count(DISTINCT (block_number, log_index)) FROM shop.trade_event",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!((rows, distinct), (6, 6), "every log once across the kill");
    assert_eq!(
        last.indexed_to,
        (head - 64) as i64,
        "the walk stays the confirmation depth behind the head"
    );

    assert_eq!(status_of(&db.pool, &sold.0).await.as_deref(), Some("sold"));
    assert_eq!(
        status_of(&db.pool, &by_signer.0).await.as_deref(),
        Some("cancelled")
    );
    assert_eq!(
        status_of(&db.pool, &by_stranger.0).await.as_deref(),
        Some("open"),
        "only the signer's cancellation cancels"
    );
    assert_eq!(
        status_of(&db.pool, &index_moved.0).await.as_deref(),
        Some("cancelled"),
        "the signer's index moved past the signed one"
    );
    assert_eq!(
        status_of(&db.pool, &open.0).await.as_deref(),
        Some("open"),
        "one of two uses"
    );
    assert_eq!(status_of(&db.pool, &bid.0).await.as_deref(), Some("sold"));

    // The routes over the published index.
    let app = super::market_router(st.clone());
    let (code, body) = get_json(
        &app,
        &format!("/v1/trades/{}/accept?timestamp=1234&caller={BOB}", sold.1),
    )
    .await;
    assert_eq!(code, StatusCode::OK, "{body}");
    assert_eq!(
        (
            body["ok"].as_bool(),
            body["data"]["subType"].as_str(),
            body["data"]["timestamp"].as_i64()
        ),
        (Some(true), Some("item-sold"), Some(1234))
    );
    assert_eq!(body["data"]["metadata"]["nftName"], "Hat");
    assert_eq!(body["data"]["metadata"]["buyer"], BOB);
    let (code, body) = get_json(&app, &format!("/v1/trades/{}/accept", by_stranger.1)).await;
    assert_eq!(
        (code, body["ok"].as_bool()),
        (StatusCode::NOT_FOUND, Some(false)),
        "a trade the chain never executed: {body}"
    );
    let open_ids: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "WITH {} SELECT id::text FROM unified_trades",
        super::catalog::unified_trades_cte("$1", "$2")
    )))
    .bind(chrono::Utc::now().naive_utc())
    .bind(None::<chrono::NaiveDateTime>)
    .fetch_all(&db.pool)
    .await
    .unwrap();
    assert!(
        open_ids.contains(&open.0) && open_ids.contains(&by_stranger.0),
        "{open_ids:?}"
    );
    assert!(
        !open_ids.contains(&sold.0)
            && !open_ids.contains(&by_signer.0)
            && !open_ids.contains(&index_moved.0),
        "{open_ids:?}"
    );
    let bids: Vec<(String, String)> =
        sqlx::query_as(sqlx::AssertSqlSafe(super::bids::test_trades_bids_sql()))
            .bind(chrono::Utc::now().naive_utc())
            .bind(None::<chrono::NaiveDateTime>)
            .fetch_all(&db.pool)
            .await
            .unwrap()
            .into_iter()
            .collect();
    assert!(
        bids.contains(&(bid.0.clone(), "sold".to_string())),
        "{bids:?}"
    );

    // A stale publish is not trusted: the verdicts fall back to open.
    sqlx::query("UPDATE shop.chain_cursor SET folded_at = folded_at - interval '31 minutes' WHERE indexer = 'trade_events'").execute(&db.pool).await.unwrap();
    let open_ids: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "WITH {} SELECT id::text FROM unified_trades",
        super::catalog::unified_trades_cte("$1", "$2")
    )))
    .bind(chrono::Utc::now().naive_utc())
    .bind(None::<chrono::NaiveDateTime>)
    .fetch_all(&db.pool)
    .await
    .unwrap();
    assert!(
        open_ids.contains(&sold.0) && open_ids.contains(&index_moved.0),
        "an unpublished verdict is ignored: {open_ids:?}"
    );
    db.drop().await;
}

#[tokio::test]
async fn a_v3_cancellation_names_the_digest_the_fold_recomputes_from_the_stored_trade() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let v3 = listings::offchain_marketplace_v3(137).unwrap();
    let mut trade = listings::test_support::signed_trade_on(137, &v3);
    listings::test_support::sign(&mut trade, &v3);
    let signer = listings::test_support::signer()
        .address()
        .to_string()
        .to_ascii_lowercase();
    let digest = format!("{:#x}", listings::signing_hash(&trade, &v3).unwrap());
    let id = match listings::insert_trade(&db.pool, &trade, "MATIC", v3.address).await {
        Ok(id) => id,
        Err(e) => panic!("the fixture trade is stored: {e}"),
    };
    let start = TradeEventsIndex::mainnet(ChainNet::Matic).start_block();
    let head = 93_600_000u64.max(start + 200);
    let chain = FakeChain::new(
        head,
        vec![cancelled(93_590_000, v3.address, &signer, &digest)],
        1_000_000_000,
        10,
    );
    let url = serve(chain).await;
    let st = state_over(
        db.pool.clone(),
        &url,
        IndexSettings {
            window_blocks: 20_000_000,
            requests_per_pass: 50,
            ..index_settings(50, 20_000_000)
        },
        false,
    );
    let mut last = PassOutcome::default();
    for _ in 0..10 {
        last = pass(&st, &url, &TradeEventsIndex::mainnet(ChainNet::Matic)).await;
        if last.published {
            break;
        }
    }
    assert!(last.published, "{last:?}");
    let (stored_digest, status): (Option<String>, String) = sqlx::query_as(
        "SELECT digest, status FROM shop.trade_chain_state WHERE trade_id = $1::uuid",
    )
    .bind(&id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(stored_digest.as_deref(), Some(digest.as_str()));
    assert_eq!(status, "cancelled");

    // A derived state that lost its digest: a rewind keeps it, a rebuild
    // recomputes it from the stored trade and the cancellation applies again.
    sqlx::query("UPDATE shop.trade_chain_state SET digest = NULL, status = 'open' WHERE trade_id = $1::uuid")
        .bind(&id)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE shop.chain_cursor SET rebuild_requested_at = timezone('UTC', now()), folded_block = NULL, folded_at = NULL, owner_generation = owner_generation + 1 WHERE indexer = 'trade_events' AND network = 'matic'")
        .execute(&db.pool)
        .await
        .unwrap();
    let rebuilt = pass(&st, &url, &TradeEventsIndex::mainnet(ChainNet::Matic)).await;
    assert!(rebuilt.rebuilt, "{rebuilt:?}");
    let mut last = PassOutcome::default();
    for _ in 0..10 {
        last = pass(&st, &url, &TradeEventsIndex::mainnet(ChainNet::Matic)).await;
        if last.published {
            break;
        }
    }
    assert!(last.published, "{last:?}");
    let again: (Option<String>, String) = sqlx::query_as(
        "SELECT digest, status FROM shop.trade_chain_state WHERE trade_id = $1::uuid",
    )
    .bind(&id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        again,
        (Some(digest.clone()), "cancelled".to_string()),
        "the rebuild matches a clean walk"
    );
    db.drop().await;
}

fn transfer(
    contract: &str,
    from: &str,
    to: &str,
    token: U256,
    block: u64,
    index: u64,
) -> crate::chain_index::rpc::RpcLog {
    log_at(
        contract,
        vec![
            nft_index::TOPIC_TRANSFER.into(),
            topic_addr(from),
            topic_addr(to),
            topic_word(token),
        ],
        vec![],
        block,
        index,
    )
}

fn v2_token(item: u64, issued: u64) -> U256 {
    (U256::from(item) << 216) | U256::from(issued)
}

async fn seed_collection(
    pool: &PgPool,
    network: &str,
    contract: &str,
    created_block: u64,
    urn: &str,
    name: &str,
) {
    sqlx::query(
        "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, is_completed, is_approved, is_editable, created_at, updated_at, reviewed_at, mirrored_at) \
         VALUES ($1, $2, $3, $4, $5, $5, $6, true, true, false, $7, $7, $7, timezone('UTC', now()))",
    )
    .bind(network)
    .bind(contract)
    .bind(if network == "matic" { 137 } else { 1 })
    .bind(name)
    .bind(CARL)
    .bind(urn)
    .bind(time_of(created_block))
    .execute(pool)
    .await
    .unwrap();
}

#[allow(
    clippy::too_many_arguments,
    reason = "fixture: one argument per seeded shop.items column"
)]
async fn seed_item(
    pool: &PgPool,
    network: &str,
    contract: &str,
    item: u64,
    urn: &str,
    name: &str,
    item_type: &str,
    category: &str,
) {
    sqlx::query(
        "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, total_supply, max_supply, available, price, image, name, category, body_shapes, search_is_collection_approved, created_at, updated_at, reviewed_at, mirrored_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'rare', 1, 100, 99, 10, 'https://img/' || $6, $8, $9, ARRAY['BaseMale'], true, 1, 1, 1, timezone('UTC', now()))",
    )
    .bind(network)
    .bind(contract)
    .bind(item as i64)
    .bind(if network == "matic" { 137 } else { 1 })
    .bind(item_type)
    .bind(urn)
    .bind(CARL)
    .bind(name)
    .bind(category)
    .execute(pool)
    .await
    .unwrap();
}

async fn owner_of(
    pool: &PgPool,
    network: &str,
    contract: &str,
    token: U256,
) -> Option<Option<String>> {
    sqlx::query_scalar("SELECT owner FROM shop.nft WHERE network = $1 AND contract_address = $2 AND token_id = $3::numeric")
        .bind(network)
        .bind(contract)
        .bind(token.to_string())
        .fetch_optional(pool)
        .await
        .unwrap()
}

async fn publish_nft(st: &MarketplaceState, url: &str, ix: &NftIndex) -> PassOutcome {
    publish(st, url, ix).await
}

async fn publish(st: &MarketplaceState, url: &str, ix: &dyn ChainIndex) -> PassOutcome {
    let mut last = PassOutcome::default();
    for _ in 0..40 {
        last = pass(st, url, ix).await;
        if last.published {
            break;
        }
    }
    last
}

#[tokio::test]
async fn the_nft_walk_folds_owners_orders_and_resumes_a_kill_backfills_a_late_collection_and_serves_the_deep_shapes()
 {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let s = nft_index::start_block(ChainNet::Matic);
    let c1 = "0x00000000000000000000000000000000000000c1";
    let c2 = "0x00000000000000000000000000000000000000c2";
    let late = "0x00000000000000000000000000000000000000c3";
    let market = nft_index::legacy_marketplaces(ChainNet::Matic)[0].0;
    seed_collection(
        &db.pool,
        "matic",
        c1,
        s + 100,
        "urn:decentraland:matic:collections-v2:0xc1",
        "Pirates",
    )
    .await;
    seed_collection(
        &db.pool,
        "matic",
        c2,
        s + 20_000,
        "urn:decentraland:matic:collections-v2:0xc2",
        "Dances",
    )
    .await;
    seed_item(
        &db.pool,
        "matic",
        c1,
        0,
        "urn:decentraland:matic:collections-v2:0xc1:0",
        "Pirate Hat",
        "wearable_v2",
        "hat",
    )
    .await;
    seed_item(
        &db.pool,
        "matic",
        c1,
        1,
        "urn:decentraland:matic:collections-v2:0xc1:1",
        "Parrot",
        "wearable_v2",
        "earring",
    )
    .await;
    seed_item(
        &db.pool,
        "matic",
        c2,
        0,
        "urn:decentraland:matic:collections-v2:0xc2:0",
        "Moonwalk",
        "emote_v1",
        "dance",
    )
    .await;
    let t1 = v2_token(0, 1);
    let t2 = v2_token(1, 1);
    let t3 = v2_token(0, 1);
    let order_id = FixedBytes::<32>::repeat_byte(0x42);
    let order_data = (
        order_id,
        Address::from_str(c1).unwrap(),
        U256::from(5_000u64),
        U256::from(4_000_000_000_000u64),
    )
        .abi_encode_params();
    let logs = vec![
        transfer(c1, ZERO, ALICE, t1, s + 200, 0),
        transfer(c1, ALICE, BOB, t1, s + 300, 0),
        transfer(c1, ZERO, ALICE, t2, s + 310, 0),
        log_at(
            market,
            vec![
                nft_index::TOPIC_ORDER_CREATED.into(),
                topic_word(t1),
                topic_addr(BOB),
            ],
            order_data.clone(),
            s + 400,
            1,
        ),
        transfer(c2, ZERO, CARL, t3, s + 20_100, 0),
        transfer(late, ZERO, ALICE, v2_token(0, 7), s + 1_500, 0),
    ];
    let head = s + 30_000;
    let chain = FakeChain::new(head, logs, 10_000, 2);
    let url = serve(chain.clone()).await;
    let st = trades_from_upstream(state_over(
        db.pool.clone(),
        &url,
        index_settings(3, 10_000),
        false,
    ));
    trades_pass_ended(&st, std::time::Duration::ZERO).await;
    let app = super::market_router(st.clone());
    let (code, body) = get_json(
        &app,
        "/marketplace-api/v1/nfts?search=pirate&network=MATIC"
            .trim_start_matches("/marketplace-api"),
    )
    .await;
    assert_eq!(
        (code, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("nft-index")),
        "before readiness the envelope: {body}"
    );

    // Pass 1 under a budget of 3; then a kill (new index state) and the rest.
    let first = pass(&st, &url, &NftIndex::new(ChainNet::Matic)).await;
    assert!(first.paused && !first.published, "{first:?}");
    let last = publish_nft(&st, &url, &NftIndex::new(ChainNet::Matic)).await;
    assert!(last.published, "{last:?}");
    let (rows, distinct): (i64, i64) = sqlx::query_as(
        "SELECT count(*), count(DISTINCT (network, block_number, log_index)) FROM shop.nft_event",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        (rows, distinct),
        (5, 5),
        "c3 is not in the mirror: its log is not walked"
    );
    assert_eq!(
        owner_of(&db.pool, "matic", c1, t1).await,
        Some(Some(BOB.to_string()))
    );
    let (item, issued, minted): (Option<String>, Option<String>, i64) =
        sqlx::query_as("SELECT item_id::text, issued_id::text, minted_at FROM shop.nft WHERE contract_address = $1 AND token_id = $2::numeric").bind(c1).bind(t2.to_string()).fetch_one(&db.pool).await.unwrap();
    assert_eq!(
        (item.as_deref(), issued.as_deref(), minted),
        (Some("1"), Some("1"), time_of(s + 310))
    );
    let (status, seller, price): (String, String, String) = sqlx::query_as("SELECT status, seller, price::text FROM shop.nft_order WHERE contract_address = $1 AND token_id = $2::numeric").bind(c1).bind(t1.to_string()).fetch_one(&db.pool).await.unwrap();
    assert_eq!(
        (status.as_str(), seller.as_str(), price.as_str()),
        ("open", BOB, "5000")
    );
    let walked: i64 =
        sqlx::query_scalar("SELECT count(*) FROM shop.nft_contract WHERE network = 'matic'")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(walked, 2);
    assert!(
        chain.get_logs.load(Ordering::SeqCst) > 3,
        "the 2-address limit split the list"
    );

    // The index-served shapes.
    let (code, body) = get_json(&app, "/v1/nfts?search=hat&network=MATIC").await;
    assert_eq!(code, StatusCode::OK, "{body}");
    assert_eq!(body["total"], 1, "{body}");
    assert_eq!(body["data"][0]["nft"]["name"], "Pirate Hat");
    assert_eq!(body["data"][0]["nft"]["owner"], BOB);
    assert_eq!(body["data"][0]["nft"]["itemId"], "0");
    assert_eq!(
        body["data"][0]["order"]["price"], "5000",
        "the legacy order of the owner: {body}"
    );
    assert_eq!(
        body["data"][0]["nft"]["activeOrderId"],
        format!("0x{}", "42".repeat(32))
    );
    let (code, body) = get_json(&app, "/v1/nfts?search=pirates&network=MATIC").await;
    assert_eq!(
        (code, body["total"].as_i64()),
        (StatusCode::OK, Some(2)),
        "the collection name matches both of its items: {body}"
    );
    let (code, body) = get_json(&app, "/v1/nfts?search=pirates&network=MATIC&isOnSale=true").await;
    assert_eq!(
        (code, body["total"].as_i64()),
        (StatusCode::OK, Some(1)),
        "only the listed one is on sale: {body}"
    );
    let (code, body) = get_json(&app, "/v1/nfts?first=10&skip=6000&network=MATIC").await;
    assert_eq!(
        (
            code,
            body["total"].as_i64(),
            body["data"].as_array().map(Vec::len)
        ),
        (StatusCode::OK, Some(3), Some(0)),
        "a page past the reader window reads the index: {body}"
    );
    let (code, body) = get_json(
        &app,
        "/v1/prices?assetType=nft&category=wearable&network=MATIC",
    )
    .await;
    assert_eq!(
        (code, body["data"].clone()),
        (StatusCode::OK, json!({ "5000": 1 })),
        "{body}"
    );

    // A late collection: the mirror learns c3 after the walk passed its
    // creation; the backfill lane reads it before the network republishes.
    seed_collection(
        &db.pool,
        "matic",
        late,
        s + 1_000,
        "urn:decentraland:matic:collections-v2:0xc3",
        "Late",
    )
    .await;
    *chain.head.lock() = head + 100;
    let last = publish_nft(&st, &url, &NftIndex::new(ChainNet::Matic)).await;
    assert!(last.published, "{last:?}");
    let (from, backfill, done, owed): (i64, Option<i64>, Option<i64>, bool) =
        sqlx::query_as("SELECT walked_from_block, backfill_from_block, backfilled_to_block, refold_owed FROM shop.nft_contract WHERE contract_address = $1").bind(late).fetch_one(&db.pool).await.unwrap();
    assert!(
        backfill.is_some_and(|b| b <= (s + 1_000) as i64) && done == Some(from - 1) && !owed,
        "{from} {backfill:?} {done:?} {owed}"
    );
    assert_eq!(
        owner_of(&db.pool, "matic", late, v2_token(0, 7)).await,
        Some(Some(ALICE.to_string())),
        "the backfilled token is folded"
    );
    db.drop().await;
}

#[tokio::test]
async fn a_reorg_past_the_confirmation_depth_rewinds_and_the_refold_follows_the_new_chain() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let s = nft_index::start_block(ChainNet::Matic);
    let c1 = "0x00000000000000000000000000000000000000c1";
    seed_collection(
        &db.pool,
        "matic",
        c1,
        s + 10,
        "urn:decentraland:matic:collections-v2:0xc1",
        "Pirates",
    )
    .await;
    let t1 = v2_token(0, 1);
    let head = s + 2_000;
    let chain = FakeChain::new(
        head,
        vec![
            transfer(c1, ZERO, ALICE, t1, s + 100, 0),
            transfer(c1, ALICE, BOB, t1, s + 1_900, 0),
        ],
        1_000_000,
        100,
    );
    let url = serve(chain.clone()).await;
    let st = state_over(db.pool.clone(), &url, index_settings(50, 100_000), false);
    assert!(
        publish_nft(&st, &url, &NftIndex::new(ChainNet::Matic))
            .await
            .published
    );
    assert_eq!(
        owner_of(&db.pool, "matic", c1, t1).await,
        Some(Some(BOB.to_string()))
    );
    let cursor = chain_walk::cursor(&db.pool, INDEXER_NFT, ChainNet::Matic)
        .await
        .unwrap()
        .unwrap();
    // The chain reorganizes: the block the cursor stands on has another
    // hash, the transfer to BOB is gone and one to CARL replaces it.
    chain
        .forks
        .lock()
        .insert(cursor.last_block as u64, format!("0x{}", "ee".repeat(32)));
    {
        let mut logs = chain.logs.lock();
        logs.retain(|l| l["blockNumber"] != json!(format!("0x{:x}", s + 1_900)));
        logs.push(log_json(&transfer(c1, ALICE, CARL, t1, s + 1_880, 0)));
    }
    let rewound = pass(&st, &url, &NftIndex::new(ChainNet::Matic)).await;
    assert_eq!(
        rewound.reorg_rewound_to,
        Some(cursor.start_block - 1),
        "{rewound:?}"
    );
    assert!(
        chain_walk::ready_cursor(&db.pool, INDEXER_NFT, ChainNet::Matic, chrono::Utc::now())
            .await
            .is_none(),
        "a rewind unpublishes"
    );
    chain.forks.lock().clear();
    assert!(
        publish_nft(&st, &url, &NftIndex::new(ChainNet::Matic))
            .await
            .published
    );
    assert_eq!(
        owner_of(&db.pool, "matic", c1, t1).await,
        Some(Some(CARL.to_string()))
    );
    let orphans: i64 =
        sqlx::query_scalar("SELECT count(*) FROM shop.nft_event WHERE block_number = $1")
            .bind((s + 1_900) as i64)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(orphans, 0);
    db.drop().await;
}

/// The requests the walk sends, in order, for one fixed history: budgeted
/// passes, the endpoint's address limit halving the chunk, headers for logs
/// without a timestamp, a pass with nothing new, a late collection's
/// backfill lane and a reorg rewind. Pinned so that a change to how the walk
/// reads the chain shows as a diff of requests, not only of outcomes.
#[tokio::test]
async fn the_walk_sends_the_pinned_requests_in_order() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let s = nft_index::start_block(ChainNet::Matic);
    let c1 = "0x00000000000000000000000000000000000000c1";
    let c2 = "0x00000000000000000000000000000000000000c2";
    let late = "0x00000000000000000000000000000000000000c3";
    for (contract, created, urn, name) in [
        (
            c1,
            s + 100,
            "urn:decentraland:matic:collections-v2:0xc1",
            "Pirates",
        ),
        (
            c2,
            s + 20_000,
            "urn:decentraland:matic:collections-v2:0xc2",
            "Dances",
        ),
    ] {
        seed_collection(&db.pool, "matic", contract, created, urn, name).await;
    }
    let head = s + 30_000;
    let chain = FakeChain::new(
        head,
        vec![
            transfer(c1, ZERO, ALICE, v2_token(0, 1), s + 200, 0),
            transfer(c1, ALICE, BOB, v2_token(0, 1), s + 300, 0),
            transfer(c2, ZERO, CARL, v2_token(0, 1), s + 20_100, 0),
            transfer(late, ZERO, ALICE, v2_token(0, 7), s + 1_500, 0),
        ],
        10_000,
        2,
    );
    chain.strip_timestamps.store(true, Ordering::SeqCst);
    let url = serve(chain.clone()).await;
    let st = state_over(
        db.pool.clone(),
        &url,
        IndexSettings {
            address_chunk: 4,
            ..index_settings(3, 10_000)
        },
        false,
    );
    let tag = |o: &PassOutcome| {
        format!(
            "requests {} logs {} rows {} to +{} paused {} published {} rewound {:?}",
            o.requests,
            o.logs,
            o.rows_written,
            o.indexed_to - s as i64,
            o.paused,
            o.published,
            o.reorg_rewound_to.map(|b| b - s as i64)
        )
    };
    let mut outcomes = Vec::new();
    let until_published = async |outcomes: &mut Vec<String>| {
        for _ in 0..40 {
            let o = pass(&st, &url, &NftIndex::new(ChainNet::Matic)).await;
            outcomes.push(tag(&o));
            if o.published {
                return;
            }
        }
        panic!("never published: {outcomes:?}");
    };
    until_published(&mut outcomes).await;
    // Nothing new: the cursor check, the heads and the publish refresh.
    outcomes.push(tag(&pass(&st, &url, &NftIndex::new(ChainNet::Matic)).await));
    // The mirror learns c3 after the walk passed its creation.
    seed_collection(
        &db.pool,
        "matic",
        late,
        s + 1_000,
        "urn:decentraland:matic:collections-v2:0xc3",
        "Late",
    )
    .await;
    *chain.head.lock() = head + 100;
    until_published(&mut outcomes).await;
    // The block the cursor stands on changes hash.
    let cursor = chain_walk::cursor(&db.pool, INDEXER_NFT, ChainNet::Matic)
        .await
        .unwrap()
        .unwrap();
    chain
        .forks
        .lock()
        .insert(cursor.last_block as u64, format!("0x{}", "ee".repeat(32)));
    outcomes.push(tag(&pass(&st, &url, &NftIndex::new(ChainNet::Matic)).await));
    chain.forks.lock().clear();
    until_published(&mut outcomes).await;
    let calls: Vec<String> = chain
        .calls
        .lock()
        .iter()
        .map(|c| from_start(c, s))
        .collect();
    assert_eq!(outcomes, PINNED_OUTCOMES);
    assert_eq!(calls, PINNED_CALLS);
    db.drop().await;
}

/// `tag` with every block number at or past `s` written as `s+N`.
fn from_start(tag: &str, s: u64) -> String {
    let mut out = String::new();
    let mut digits = String::new();
    for ch in tag.chars().chain(std::iter::once('\0')) {
        if ch.is_ascii_digit() {
            digits.push(ch);
            continue;
        }
        if !digits.is_empty() {
            match digits.parse::<u64>() {
                Ok(n) if n >= s => out.push_str(&format!("s+{}", n - s)),
                _ => out.push_str(&digits),
            }
            digits.clear();
        }
        if ch != '\0' {
            out.push(ch);
        }
    }
    out
}

const PINNED_OUTCOMES: &[&str] = &[
    "requests 9 logs 2 rows 2 to +9999 paused true published false rewound None",
    "requests 8 logs 0 rows 0 to +19999 paused true published false rewound None",
    "requests 11 logs 1 rows 1 to +29936 paused false published true rewound None",
    "requests 5 logs 0 rows 0 to +29936 paused false published true rewound None",
    "requests 10 logs 0 rows 0 to +30036 paused false published false rewound None",
    "requests 8 logs 0 rows 0 to +30036 paused false published false rewound None",
    "requests 6 logs 0 rows 0 to +30036 paused false published true rewound None",
    "requests 1 logs 0 rows 0 to +30036 paused false published false rewound Some(-1)",
    "requests 9 logs 3 rows 3 to +9999 paused true published false rewound None",
    "requests 8 logs 0 rows 0 to +19999 paused true published false rewound None",
    "requests 9 logs 1 rows 1 to +29999 paused true published false rewound None",
    "requests 10 logs 0 rows 0 to +30036 paused false published true rewound None",
];

const PINNED_CALLS: &[&str] = &[
    "head",
    "finalized",
    "header 15201999",
    "header s+9999",
    "logs s+0..s+9999 x3",
    "logs s+0..s+9999 x2",
    "logs s+0..s+9999 x1",
    "batch [header s+200, header s+300]",
    "header s+9999",
    "header s+9999",
    "head",
    "finalized",
    "header s+19999",
    "logs s+10000..s+19999 x3",
    "logs s+10000..s+19999 x2",
    "logs s+10000..s+19999 x1",
    "header s+19999",
    "header s+19999",
    "head",
    "finalized",
    "header s+29936",
    "logs s+20000..s+29936 x3",
    "logs s+20000..s+29936 x2",
    "logs s+20000..s+29936 x1",
    "batch [header s+20100]",
    "header s+29936",
    "head",
    "finalized",
    "header s+29936",
    "head",
    "finalized",
    "head",
    "finalized",
    "header s+29936",
    "head",
    "finalized",
    "header s+30036",
    "logs s+29937..s+30036 x4",
    "logs s+29937..s+30036 x2",
    "logs s+29937..s+30036 x2",
    "header s+30036",
    "head",
    "finalized",
    "header s+30036",
    "head",
    "finalized",
    "logs s+0..s+9999 x1",
    "batch [header s+1500]",
    "logs s+10000..s+19999 x1",
    "head",
    "finalized",
    "header s+30036",
    "head",
    "finalized",
    "logs s+20000..s+29936 x1",
    "head",
    "finalized",
    "header s+30036",
    "head",
    "finalized",
    "header 15201999",
    "header s+9999",
    "logs s+0..s+9999 x4",
    "logs s+0..s+9999 x2",
    "logs s+0..s+9999 x2",
    "batch [header s+200, header s+300, header s+1500]",
    "header s+9999",
    "header s+9999",
    "head",
    "finalized",
    "header s+19999",
    "logs s+10000..s+19999 x4",
    "logs s+10000..s+19999 x2",
    "logs s+10000..s+19999 x2",
    "header s+19999",
    "header s+19999",
    "head",
    "finalized",
    "header s+29999",
    "logs s+20000..s+29999 x4",
    "logs s+20000..s+29999 x2",
    "logs s+20000..s+29999 x2",
    "batch [header s+20100]",
    "header s+29999",
    "header s+29999",
    "head",
    "finalized",
    "header s+30036",
    "logs s+30000..s+30036 x4",
    "logs s+30000..s+30036 x2",
    "logs s+30000..s+30036 x2",
    "header s+30036",
    "head",
    "finalized",
];

/// After a reorg rewind and the refold it owes, `land_data` equals a
/// truncate-and-rebuild of the remaining log: a token whose only Update was
/// reorged out loses its row, a token with an older Update falls back to it.
#[tokio::test]
async fn a_rewind_leaves_land_data_as_a_rebuild_of_the_remaining_log() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let land = crate::land_authz::LAND_REGISTRY_MAINNET;
    let update = |block: i64, token: i64, name: &str| {
        format!(
            "('ethereum', {block}, 0, {block}, 'land_data', '{land}', '{land}', {token}, NULL, NULL, NULL, NULL, '0,\"{name}\",\"\"')"
        )
    };
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "INSERT INTO shop.nft_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, from_address, to_address, amount, expires_at, detail) VALUES {}, {}, {}",
        update(100, 1, "A-old"),
        update(200, 1, "A-new"),
        update(201, 2, "B-only-late"),
    )))
    .execute(&db.pool)
    .await
    .unwrap();
    nft_index::fold(
        &db.pool,
        ChainNet::Ethereum,
        None,
        &chain_walk::test_owner(
            &db.pool,
            &NftIndex::new(ChainNet::Ethereum),
            &IndexSettings::default(),
        )
        .await,
    )
    .await
    .unwrap();
    let rows = |pool: sqlx::PgPool| async move {
        sqlx::query_as::<_, (String, Option<String>, i64)>(
            "SELECT token_id::text, name, updated_block FROM shop.land_data ORDER BY token_id",
        )
        .fetch_all(&pool)
        .await
        .unwrap()
    };
    assert_eq!(
        rows(db.pool.clone()).await,
        vec![
            ("1".into(), Some("A-new".into()), 200),
            ("2".into(), Some("B-only-late".into()), 201)
        ]
    );

    let mut conn = db.pool.acquire().await.unwrap();
    NftIndex::new(ChainNet::Ethereum)
        .rewind(&mut conn, 150)
        .await
        .unwrap();
    drop(conn);
    nft_index::fold(
        &db.pool,
        ChainNet::Ethereum,
        None,
        &chain_walk::test_owner(
            &db.pool,
            &NftIndex::new(ChainNet::Ethereum),
            &IndexSettings::default(),
        )
        .await,
    )
    .await
    .unwrap();
    let after_rewind = rows(db.pool.clone()).await;
    assert_eq!(
        after_rewind,
        vec![("1".into(), Some("A-old".into()), 100)],
        "token 2 had no Update left at or before the cursor"
    );

    sqlx::query("TRUNCATE shop.land_data")
        .execute(&db.pool)
        .await
        .unwrap();
    nft_index::fold(
        &db.pool,
        ChainNet::Ethereum,
        None,
        &chain_walk::test_owner(
            &db.pool,
            &NftIndex::new(ChainNet::Ethereum),
            &IndexSettings::default(),
        )
        .await,
    )
    .await
    .unwrap();
    assert_eq!(
        rows(db.pool.clone()).await,
        after_rewind,
        "the rewind's result is the truncate-and-rebuild's"
    );
    db.drop().await;
}

/// A fold past `since` leaves `land_data` exactly as a truncate-and-rebuild would.
#[tokio::test]
async fn a_land_data_fold_past_since_equals_a_rebuild() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let land = crate::land_authz::LAND_REGISTRY_MAINNET;
    let estate = crate::land_authz::ESTATE_REGISTRY_MAINNET;
    let far = "115792089237316195423570985008687907853269984665640564039457584007913129639935";
    let update = |block: i64, log: i32, contract: &str, token: &str, name: &str| {
        format!(
            "('ethereum', {block}, {log}, {block}, 'land_data', '{contract}', '{contract}', {token}, NULL, NULL, NULL, NULL, '0,\"{name}\",\"\"')"
        )
    };
    let insert = |rows: Vec<String>| {
        let pool = db.pool.clone();
        async move {
            sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                "INSERT INTO shop.nft_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, from_address, to_address, amount, expires_at, detail) VALUES {}",
                rows.join(", ")
            )))
            .execute(&pool)
            .await
            .unwrap();
        }
    };
    let rows = || {
        let pool = db.pool.clone();
        async move {
            sqlx::query_as::<_, (String, String, Option<String>, i64, i32)>(
                "SELECT token_address, token_id::text, name, updated_block, updated_log FROM shop.land_data ORDER BY token_address, land_data.token_id",
            )
            .fetch_all(&pool)
            .await
            .unwrap()
        }
    };

    insert(vec![
        update(100, 0, land, "1", "A-old"),
        update(150, 0, estate, "1", "E-old"),
        update(120, 0, land, "7", "G-old"),
        update(150, 1, land, "9", "Edge"),
    ])
    .await;
    nft_index::fold(
        &db.pool,
        ChainNet::Ethereum,
        None,
        &chain_walk::test_owner(
            &db.pool,
            &NftIndex::new(ChainNet::Ethereum),
            &IndexSettings::default(),
        )
        .await,
    )
    .await
    .unwrap();

    insert(vec![
        update(200, 0, land, "1", "A-first"),
        update(200, 3, land, "1", "A-last"),
        update(151, 0, estate, "1", "E-new"),
        update(202, 0, land, far, "Far"),
        format!(
            "('ethereum', 203, 0, 203, 'transfer', '{land}', '{land}', 7, '{ALICE}', '{BOB}', NULL, NULL, NULL)"
        ),
    ])
    .await;
    let counts = nft_index::fold(
        &db.pool,
        ChainNet::Ethereum,
        Some(150),
        &chain_walk::test_owner(
            &db.pool,
            &NftIndex::new(ChainNet::Ethereum),
            &IndexSettings::default(),
        )
        .await,
    )
    .await
    .unwrap();
    assert_eq!(counts.land_data, 3, "LAND 1, Estate 1 and the far token");
    let incremental = rows().await;
    assert_eq!(
        incremental,
        vec![
            (estate.into(), "1".into(), Some("E-new".into()), 151, 0),
            (land.into(), "1".into(), Some("A-last".into()), 200, 3),
            (land.into(), "7".into(), Some("G-old".into()), 120, 0),
            (land.into(), "9".into(), Some("Edge".into()), 150, 1),
            (land.into(), far.into(), Some("Far".into()), 202, 0),
        ]
    );

    sqlx::query("TRUNCATE shop.land_data")
        .execute(&db.pool)
        .await
        .unwrap();
    nft_index::fold(
        &db.pool,
        ChainNet::Ethereum,
        None,
        &chain_walk::test_owner(
            &db.pool,
            &NftIndex::new(ChainNet::Ethereum),
            &IndexSettings::default(),
        )
        .await,
    )
    .await
    .unwrap();
    assert_eq!(
        rows().await,
        incremental,
        "the incremental fold's result is the rebuild's"
    );
    db.drop().await;
}

/// Pins `LAND_DATA_SQL`'s shape with seq scans off: one bounded index scan, not which index nor the fold's `SET LOCAL`.
#[tokio::test]
async fn the_land_data_read_is_one_bounded_index_scan() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let mut conn = db.pool.acquire().await.unwrap();
    sqlx::query("SET enable_seqscan = off")
        .execute(&mut *conn)
        .await
        .unwrap();
    let lines: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "EXPLAIN {}",
        nft_index::LAND_DATA_SQL
    )))
    .bind(23_900_000_i64)
    .fetch_all(&mut *conn)
    .await
    .unwrap();
    let plan = lines.join("\n");
    assert!(
        plan.lines()
            .any(|l| l.contains("Index Cond") && l.contains("block_number > "))
            && plan.matches("on nft_event ").count() == 1
            && !plan.contains("Seq Scan"),
        "{plan}"
    );
    sqlx::query("RESET enable_seqscan")
        .execute(&mut *conn)
        .await
        .unwrap();
    drop(conn);
    db.drop().await;
}

#[tokio::test]
async fn a_reindexed_order_only_gains_its_missing_transaction_hash() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let order = |to: &str, tx_hash: Option<&str>| nft_index::NftEvent {
        block_number: 100,
        log_index: 2,
        block_time: 1,
        kind: "order_successful",
        emitter: "0xmarket".into(),
        contract_address: "0xnft".into(),
        token_id: "7".into(),
        from_address: None,
        to_address: Some(to.into()),
        amount: Some("5".into()),
        expires_at: None,
        detail: None,
        tx_hash: tx_hash.map(str::to_string),
    };
    let (a, b) = (
        format!("0x{}", "a".repeat(64)),
        format!("0x{}", "b".repeat(64)),
    );
    let mut conn = db.pool.acquire().await.unwrap();
    let mut write = async |row| {
        nft_index::insert_events(&mut conn, ChainNet::Matic, &[row])
            .await
            .unwrap()
    };
    assert_eq!(write(order("0xbuyer", None)).await, 1);
    assert_eq!(write(order("0xbuyer", None)).await, 0, "nothing to fill");
    assert_eq!(
        write(order("0xother", Some(&a))).await,
        1,
        "the hash is filled"
    );
    assert_eq!(write(order("0xbuyer", Some(&a))).await, 0, "already filled");
    assert_eq!(
        write(order("0xbuyer", Some(&b))).await,
        0,
        "a kept hash stays"
    );
    drop(conn);
    let kept: (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT to_address, tx_hash FROM shop.nft_event WHERE network = 'matic' AND block_number = 100",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(kept, (Some("0xbuyer".into()), Some(a)));
    db.drop().await;
}

#[tokio::test]
async fn ethereum_names_v1_wearables_land_data_and_the_land_routes() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    // Past the registrar's deployment (9,412,979), the walk's first
    // fixed contracts, so every log below is in the address set.
    let s = 9_420_000u64;
    let v1 = "0x00000000000000000000000000000000000000e1";
    let market = nft_index::legacy_marketplaces(ChainNet::Ethereum)[0].0;
    let (land, estate) = (
        crate::land_authz::LAND_REGISTRY_MAINNET,
        crate::land_authz::ESTATE_REGISTRY_MAINNET,
    );
    seed_collection(
        &db.pool,
        "ethereum",
        v1,
        s + 10,
        "urn:decentraland:ethereum:collections-v1:halloween_2019",
        "Halloween",
    )
    .await;
    seed_item(
        &db.pool,
        "ethereum",
        v1,
        6,
        "urn:decentraland:ethereum:collections-v1:halloween_2019:jester_feet",
        "Jester Feet",
        "wearable_v1",
        "feet",
    )
    .await;
    let label = keccak256("pixelgolf");
    let name_token = U256::from_be_bytes(label.0);
    let parcel = U256::from_str("12590447576074723148144860474975423823927").unwrap();
    let order = (
        FixedBytes::<32>::repeat_byte(0x07),
        Address::from_str(land).unwrap(),
        U256::from(9_000u64),
        U256::from(4_000_000_000u64),
    )
        .abi_encode_params();
    let reg = nft_index::DCL_REGISTRAR;
    // PixelGolf on sale for 5,000 wei, under the parcel's 9,000: the NAME browse's price range.
    let name_order = (
        FixedBytes::<32>::repeat_byte(0x08),
        Address::from_str(reg).unwrap(),
        U256::from(5_000u64),
        U256::from(4_000_000_000u64),
    )
        .abi_encode_params();
    let logs = vec![
        transfer(v1, ZERO, ALICE, U256::from(0u8), s + 100, 0),
        log_at(
            v1,
            vec![
                nft_index::TOPIC_ISSUE_V1.into(),
                topic_addr(ALICE),
                topic_word(U256::ZERO),
                format!("{:#x}", keccak256("jester_feet")),
            ],
            ("jester_feet".to_string(), U256::from(1u8)).abi_encode_params(),
            s + 100,
            1,
        ),
        transfer(reg, ZERO, BOB, name_token, s + 200, 0),
        log_at(
            reg,
            vec![
                nft_index::TOPIC_NAME_REGISTERED.into(),
                topic_addr(BOB),
                topic_addr(BOB),
                format!("{label:#x}"),
            ],
            ("PixelGolf".to_string(), U256::from(1u8)).abi_encode_params(),
            s + 200,
            1,
        ),
        log_at(
            land,
            vec![
                nft_index::TOPIC_LAND_UPDATE.into(),
                topic_word(parcel),
                topic_addr(CARL),
                topic_addr(CARL),
            ],
            ("0,\"Sea View\",\"by the road\"".to_string(),).abi_encode_params(),
            s + 300,
            0,
        ),
        log_at(
            estate,
            vec![
                nft_index::TOPIC_LAND_UPDATE.into(),
                topic_word(U256::from(500u32)),
                topic_addr(CARL),
                topic_addr(CARL),
            ],
            ("0,\"Fashion Street\",\"\"".to_string(),).abi_encode_params(),
            s + 301,
            0,
        ),
        log_at(
            market,
            vec![
                nft_index::TOPIC_ORDER_CREATED.into(),
                topic_word(parcel),
                topic_addr(CARL),
            ],
            order,
            s + 400,
            0,
        ),
        log_at(
            market,
            vec![
                nft_index::TOPIC_ORDER_CREATED.into(),
                topic_word(name_token),
                topic_addr(BOB),
            ],
            name_order,
            s + 401,
            0,
        ),
    ];
    let head = s + 1_000;
    let chain = FakeChain::new(head, logs, 1_000_000, 100);
    let url = serve(chain).await;
    let st = trades_from_upstream(state_over(
        db.pool.clone(),
        &url,
        index_settings(60, 500_000),
        false,
    ));
    trades_pass_ended(&st, std::time::Duration::ZERO).await;
    assert!(
        publish_nft(&st, &url, &NftIndex::new(ChainNet::Ethereum))
            .await
            .published
    );
    // The LAND facts are land_authz's: a published cursor and its rows.
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "INSERT INTO land_authz.index_cursor (network, land_registry, estate_registry, last_block, folded_block, folded_at, full_replay_started) \
         VALUES ('ethereum-mainnet', '{land}', '{estate}', 10, 10, timezone('UTC', now()), true); \
         INSERT INTO land_authz.token_right (token_address, token_id, x, y, owner, updated_block, updated_log) VALUES \
            ('{land}', {parcel}, 37, 55, '{CARL}', 1, 0), ('{estate}', 500, NULL, NULL, '{ALICE}', 1, 1), ('{land}', 1, 0, 1, '{ESTATE_OWNER}', 1, 2), ('{land}', 2, 0, 2, '{ESTATE_OWNER}', 1, 3); \
         INSERT INTO land_authz.estate_land VALUES (1, 500, 1, 0), (2, 500, 1, 1); \
         INSERT INTO land_authz.authz_event (block_number, log_index, block_time, token_address, kind, token_id, account) VALUES \
            (1, 0, 1600000000, '{land}', 'transfer', {parcel}, '{CARL}'), (1, 1, 1600000100, '{estate}', 'transfer', 500, '{ALICE}')",
        ESTATE_OWNER = estate,
    )))
    .execute(&db.pool)
    .await
    .unwrap();
    let app = super::market_router(st.clone());
    let (code, body) = get_json(&app, "/v1/nfts?category=ens&search=pixel").await;
    assert_eq!(code, StatusCode::OK, "{body}");
    assert_eq!(
        (
            body["data"][0]["nft"]["name"].as_str(),
            body["data"][0]["nft"]["data"]["ens"]["subdomain"].as_str(),
            body["data"][0]["nft"]["owner"].as_str()
        ),
        (Some("PixelGolf"), Some("PixelGolf"), Some(BOB))
    );
    // The NAME browse's price range is in wei and bounds the open listing; `cheapest` sorts by it.
    // A search reads this index (without one the subgraph serves, its `where` checked in nfts.rs).
    for (range, listed) in [
        ("minPrice=5000&maxPrice=5000", true),
        ("minPrice=1&maxPrice=9000", true),
        ("minPrice=5001", false),
        ("maxPrice=4999", false),
    ] {
        let (code, body) = get_json(
            &app,
            &format!("/v1/nfts?category=ens&search=pixel&isOnSale=true&sortBy=cheapest&{range}"),
        )
        .await;
        assert_eq!(code, StatusCode::OK, "{range}: {body}");
        let names: Vec<_> = body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["nft"]["name"].as_str())
            .collect();
        assert_eq!(
            names,
            if listed {
                vec![Some("PixelGolf")]
            } else {
                vec![]
            },
            "{range}: {body}"
        );
        if listed {
            assert_eq!(body["data"][0]["order"]["price"], "5000", "{range}: {body}");
        }
    }
    let (code, body) = get_json(&app, "/v1/nfts?category=ens&isOnSale=true&minPrice=abc").await;
    assert_eq!(code, StatusCode::BAD_REQUEST, "{body}");
    let (code, body) = get_json(&app, "/v1/nfts?search=jester&network=ETHEREUM").await;
    assert_eq!(
        (
            code,
            body["data"][0]["nft"]["name"].as_str(),
            body["data"][0]["nft"]["issuedId"].as_str()
        ),
        (StatusCode::OK, Some("Jester Feet"), Some("1")),
        "{body}"
    );
    let (code, body) = get_json(&app, "/v1/nfts?category=parcel&search=37,55").await;
    assert_eq!(code, StatusCode::OK, "{body}");
    assert_eq!(
        (
            body["data"][0]["nft"]["name"].as_str(),
            body["data"][0]["nft"]["data"]["parcel"]["x"].as_str()
        ),
        (Some("Sea View"), Some("37"))
    );
    assert_eq!(
        body["data"][0]["order"]["price"], "9000",
        "the LAND's legacy order, its seller the owner"
    );
    assert_eq!(
        body["data"][0]["order"]["expiresAt"], 4_000_000_000_000i64,
        "seconds scaled to ms"
    );
    let (code, body) = get_json(&app, "/v1/nfts?category=estate&search=fashion").await;
    assert_eq!(
        (
            code,
            body["data"][0]["nft"]["data"]["estate"]["size"].as_i64()
        ),
        (StatusCode::OK, Some(2)),
        "{body}"
    );
    assert_eq!(
        body["data"][0]["nft"]["data"]["estate"]["parcels"],
        json!([{ "x": 0, "y": 1 }, { "x": 0, "y": 2 }])
    );
    let (code, body) = get_json(&app, "/v1/prices?category=parcel").await;
    assert_eq!(
        (code, body["data"].clone()),
        (StatusCode::OK, json!({ "9000": 1 })),
        "{body}"
    );
    let (code, body) = get_json(&app, "/v1/stats/estate/size").await;
    assert_eq!(
        (code, body["data"].clone()),
        (StatusCode::OK, json!({ "2": 1 })),
        "{body}"
    );
    // The geography filters read the squid's static LAND map (none of these
    // parcels is in it: distance -1, no road), with the squid's semantics:
    // `<= max` keeps a -1, `>= min` and `adjacentToRoad` drop it.
    let (code, body) = get_json(
        &app,
        "/v1/nfts?category=parcel&search=37,55&adjacentToRoad=true",
    )
    .await;
    assert_eq!(
        (code, body["total"].as_i64()),
        (StatusCode::OK, Some(0)),
        "{body}"
    );
    let (code, body) = get_json(
        &app,
        "/v1/nfts?category=parcel&search=37,55&maxDistanceToPlaza=3",
    )
    .await;
    assert_eq!(
        (code, body["data"][0]["nft"]["name"].as_str()),
        (StatusCode::OK, Some("Sea View")),
        "{body}"
    );
    let (code, body) = get_json(
        &app,
        "/v1/nfts?category=parcel&search=37,55&minDistanceToPlaza=1",
    )
    .await;
    assert_eq!(
        (code, body["total"].as_i64()),
        (StatusCode::OK, Some(0)),
        "{body}"
    );
    let (code, body) = get_json(&app, "/v1/stats/estate/size?adjacentToRoad=true").await;
    assert_eq!(
        (code, body["data"].clone()),
        (StatusCode::OK, json!({})),
        "{body}"
    );
    let (code, body) = get_json(&app, "/v1/prices?category=parcel&maxDistanceToPlaza=3").await;
    assert_eq!(
        (code, body["data"].clone()),
        (StatusCode::OK, json!({ "9000": 1 })),
        "{body}"
    );
    // The trades mirror past its budget: every read over the order legs
    // answers its envelope (the prices memo above included); the rest serve.
    trades_pass_ended(
        &st,
        super::trades::max_age(st.settings.trades_sync_interval)
            + std::time::Duration::from_secs(60),
    )
    .await;
    for uri in [
        "/v1/prices?category=parcel",
        "/v1/prices?category=estate",
        "/v1/prices?category=ens",
        "/v1/nfts?category=parcel&search=37,55&isOnSale=true",
        "/v1/nfts?category=parcel&isOnSale=true&skip=30000",
        "/v1/stats/estate/size?isOnSale=true",
    ] {
        let (code, body) = get_json(&app, uri).await;
        assert_eq!(
            (code, body["source"].as_str()),
            (StatusCode::SERVICE_UNAVAILABLE, Some("trades-mirror")),
            "{uri}: {body}"
        );
    }
    let (code, body) = get_json(&app, "/v1/nfts?category=parcel&search=37,55").await;
    assert_eq!(
        (code, body["data"][0]["nft"]["name"].as_str()),
        (StatusCode::OK, Some("Sea View")),
        "no order filter, no trades read: {body}"
    );
    let (code, body) = get_json(&app, "/v1/stats/estate/size").await;
    assert_eq!(
        (code, body["data"].clone()),
        (StatusCode::OK, json!({ "2": 1 })),
        "{body}"
    );
    // land_authz unpublished: the LAND shapes answer its envelope.
    sqlx::query("UPDATE land_authz.index_cursor SET folded_at = NULL")
        .execute(&db.pool)
        .await
        .unwrap();
    let (code, body) = get_json(&app, "/v1/stats/estate/size").await;
    assert_eq!(
        (code, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("land-authz-index")),
        "{body}"
    );
    db.drop().await;
}

#[tokio::test]
async fn block_times_missing_from_the_logs_come_from_batched_headers() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let s = nft_index::start_block(ChainNet::Matic);
    let c1 = "0x00000000000000000000000000000000000000c1";
    seed_collection(
        &db.pool,
        "matic",
        c1,
        s + 10,
        "urn:decentraland:matic:collections-v2:0xc1",
        "Pirates",
    )
    .await;
    let logs: Vec<_> = (0..5u64)
        .map(|i| transfer(c1, ZERO, ALICE, v2_token(0, i + 1), s + 100 + i * 7, 0))
        .collect();
    let chain = FakeChain::new(s + 1_000, logs, 1_000_000, 100);
    chain.strip_timestamps.store(true, Ordering::SeqCst);
    chain.drop_in_batch.lock().push(s + 114);
    let url = serve(chain.clone()).await;
    let st = state_over(db.pool.clone(), &url, index_settings(50, 100_000), false);
    assert!(
        publish_nft(&st, &url, &NftIndex::new(ChainNet::Matic))
            .await
            .published
    );
    let times: Vec<(i64, i64)> =
        sqlx::query_as("SELECT block_number, block_time FROM shop.nft_event ORDER BY block_number")
            .fetch_all(&db.pool)
            .await
            .unwrap();
    assert_eq!(times.len(), 5);
    assert!(
        times.iter().all(|(b, t)| *t == time_of(*b as u64)),
        "{times:?}"
    );
    assert!(
        chain.batches.load(Ordering::SeqCst) >= 1,
        "the five headers came in a batch"
    );
    db.drop().await;
}

#[tokio::test]
async fn a_bounded_window_start_is_never_published() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let s = nft_index::start_block(ChainNet::Matic);
    seed_collection(
        &db.pool,
        "matic",
        "0x00000000000000000000000000000000000000c1",
        s + 10,
        "urn:decentraland:matic:collections-v2:0xc1",
        "Old",
    )
    .await;
    let chain = FakeChain::new(s + 5_000, vec![], 1_000_000, 100);
    let url = serve(chain).await;
    let st = state_over(
        db.pool.clone(),
        &url,
        IndexSettings {
            start_block_matic: Some(s + 4_000),
            ..index_settings(50, 100_000)
        },
        false,
    );
    let out = pass(&st, &url, &NftIndex::new(ChainNet::Matic)).await;
    assert!(out.folded && !out.published, "{out:?}");
    let c = chain_walk::cursor(&db.pool, INDEXER_NFT, ChainNet::Matic)
        .await
        .unwrap()
        .unwrap();
    assert!(!c.full_history && c.start_block == (s + 4_000) as i64);
    assert_eq!(
        c.fold_through,
        Some(c.last_block),
        "the fold is recorded without a publish"
    );
    // Collections older than a bounded window owe no backfill: the window
    // is the walk.
    let owed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM shop.nft_contract WHERE backfill_from_block IS NOT NULL",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    let walked: i64 = sqlx::query_scalar("SELECT count(*) FROM shop.nft_contract")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!((walked, owed), (1, 0));
    db.drop().await;
}

#[tokio::test]
async fn the_neighbours_build_from_holdings_and_personalise_the_rail() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let c = "0x00000000000000000000000000000000000000c1";
    seed_collection(
        &db.pool,
        "matic",
        c,
        1,
        "urn:decentraland:matic:collections-v2:0xc1",
        "Pirates",
    )
    .await;
    for i in 0..3u64 {
        seed_item(
            &db.pool,
            "matic",
            c,
            i,
            &format!("urn:decentraland:matic:collections-v2:0xc1:{i}"),
            &format!("Item {i}"),
            "wearable_v2",
            "hat",
        )
        .await;
    }
    // Four wallets hold items 0 and 1; two hold 0 and 2.
    let mut rows = Vec::new();
    for w in 0..6u64 {
        let wallet = format!("0x{:040x}", 0x1000 + w);
        let other = if w < 4 { 1 } else { 2 };
        for item in [0u64, other] {
            rows.push((wallet.clone(), item, v2_token(item, w + 1)));
        }
    }
    for (wallet, item, token) in &rows {
        sqlx::query(
            "INSERT INTO shop.nft (network, contract_address, token_id, owner, item_id, issued_id, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
             VALUES ('matic', $1, $2::numeric, $3, $4, 1, 1, 1, 1, 0, 1)",
        )
        .bind(c)
        .bind(token.to_string())
        .bind(wallet)
        .bind(*item as i64)
        .execute(&db.pool)
        .await
        .unwrap();
    }
    let built = super::neighbours::rebuild_from(&db.pool, &["matic".to_string()], Some(1), None)
        .await
        .unwrap();
    assert_eq!((built.wallets, built.items), (6, 3), "{built:?}");
    let n = super::neighbours::neighbours_of(
        &db.pool,
        &[("matic".into(), c.into(), "0".into())],
        &[],
        10,
    )
    .await
    .unwrap();
    assert_eq!(
        n.first().map(|s| s.id.as_str()),
        Some(format!("{c}-1").as_str()),
        "four co-owners beat two: {n:?}"
    );
    // A seed's weight scales what it contributes (an equipped item outranks a holding).
    let weighted = super::neighbours::weighted_neighbours_of(
        &db.pool,
        &[("matic".into(), c.into(), "0".into(), 1.5)],
        &[],
        10,
    )
    .await
    .unwrap();
    assert_eq!(
        weighted
            .iter()
            .map(|s| (&s.id, &s.seed))
            .collect::<Vec<_>>(),
        n.iter().map(|s| (&s.id, &s.seed)).collect::<Vec<_>>()
    );
    for (w, u) in weighted.iter().zip(&n) {
        assert!((w.score - 1.5 * u.score).abs() < 1e-6, "{w:?} vs {u:?}");
    }
    let held = super::neighbours::held_items(&db.pool, &format!("0x{:040x}", 0x1000), 10)
        .await
        .unwrap();
    assert_eq!(held.len(), 2);
    db.drop().await;
}

#[tokio::test]
async fn the_coupon_routes_read_the_store_and_refuse_writes_while_disabled() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let st = state_over(
        db.pool.clone(),
        "http://127.0.0.1:9/",
        IndexSettings::default(),
        false,
    );
    let app = super::market_router(st.clone());
    assert_eq!(
        get_json(&app, "/v1/coupons").await.0,
        StatusCode::BAD_REQUEST,
        "a signer is required"
    );
    let (code, body) = get_json(&app, &format!("/v1/coupons?signer={ALICE}")).await;
    assert_eq!(
        (code, body),
        (StatusCode::OK, json!({ "ok": true, "data": [] }))
    );
    assert_eq!(
        get_json(&app, "/v1/coupons/not-a-uuid").await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        get_json(&app, &format!("/v1/coupons/{}", uuid::Uuid::new_v4()))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/coupons")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "DATA_WRITES_ENABLED=false refuses the write"
    );
    db.drop().await;
}

struct FakeManager {
    uses: i64,
    reads: AtomicUsize,
}

#[async_trait::async_trait]
impl crate::coupons::CouponChainReader for FakeManager {
    async fn read_coupon_allowed(
        &self,
        _chain: i64,
        _manager: &str,
        _coupon: &str,
    ) -> Result<bool, crate::coupons::ChainReadError> {
        Ok(true)
    }
    async fn read_indexes(
        &self,
        _chain: i64,
        _manager: &str,
        _signer: &str,
    ) -> Result<crate::coupons::types::CouponChainIndexes, crate::coupons::ChainReadError> {
        Ok(crate::coupons::types::CouponChainIndexes {
            contract_signature_index: 0,
            signer_signature_index: 0,
        })
    }
    async fn read_state(
        &self,
        _chain: i64,
        _manager: &str,
        _keys: &[String],
    ) -> Result<crate::coupons::types::CouponChainState, crate::coupons::ChainReadError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(crate::coupons::types::CouponChainState {
            uses: self.uses,
            cancelled: false,
        })
    }
}

#[tokio::test]
async fn a_coupon_is_stored_once_for_its_collections_creator_and_its_state_refreshes() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let (coupon, signer) = crate::coupons::component_tests::fixture_for_db();
    let collection = coupon.collections[0].clone();
    sqlx::query(
        "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, is_completed, is_approved, is_editable, created_at, updated_at, reviewed_at, mirrored_at) \
         VALUES ('matic', $1, 137, 'Mine', $2, $2, 'urn:x', true, true, false, 1, 1, 1, timezone('UTC', now()))",
    )
    .bind(&collection)
    .bind(&signer)
    .execute(&db.pool)
    .await
    .unwrap();
    let manager = Arc::new(FakeManager {
        uses: 0,
        reads: AtomicUsize::new(0),
    });
    let component = crate::coupons::CouponsComponent::new(db.pool.clone(), manager.clone());
    let now = crate::coupons::component_tests::NOW_FOR_DB;
    let (coupon2, _) = crate::coupons::component_tests::fixture_for_db();
    let stored = component
        .add_coupon(coupon, &signer, now)
        .await
        .unwrap_or_else(|e| panic!("stored: {e}"));
    assert_eq!(stored.signer, signer);
    assert!(matches!(
        component.add_coupon(coupon2, &signer, now).await,
        Err(crate::coupons::CouponError::Duplicate)
    ));
    let listed = component
        .get_coupons_by_signer(&signer, crate::coupons::CouponPagination::default())
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    // The state reader: a used-up coupon reads exhausted after a refresh.
    let exhausted = Arc::new(FakeManager {
        uses: 10,
        reads: AtomicUsize::new(0),
    });
    sqlx::query("UPDATE shop.coupons SET effective_since = timezone('UTC', now()) - interval '1 hour', expires_at = timezone('UTC', now()) + interval '1 day'").execute(&db.pool).await.unwrap();
    let refreshed = crate::coupons::CouponsComponent::new(db.pool.clone(), exhausted.clone())
        .refresh_state()
        .await
        .unwrap();
    assert_eq!(refreshed, 1);
    let one = component.get_coupon(&stored.id).await.unwrap();
    assert_eq!(
        (one.state.map(|s| s.uses), one.status),
        (Some(10), crate::coupons::CouponStatus::Exhausted)
    );
    db.drop().await;
}

/// One bid row per bidder and token after the walk: `(token, bidder, bid_id, seller, price, status)`.
async fn bids_of(pool: &PgPool) -> Vec<(String, String, String, Option<String>, String, String)> {
    sqlx::query_as(
        "SELECT token_id::text, bidder, bid_id, seller, price::text, status FROM shop.nft_bid \
         WHERE network = 'matic' ORDER BY token_id, bidder",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

fn bid_id(byte: u8) -> String {
    format!("0x{}", format!("{byte:02x}").repeat(32))
}

#[tokio::test]
async fn the_sales_walk_keeps_store_mints_folds_the_bids_and_checks_the_venues() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let s = nft_index::start_block(ChainNet::Matic);
    let b = sales_index::BID_V2_MATIC_START_BLOCK + 100;
    let c1 = "0x00000000000000000000000000000000000000c1";
    seed_collection(
        &db.pool,
        "matic",
        c1,
        s + 10,
        "urn:decentraland:matic:collections-v2:0xc1",
        "Pirates",
    )
    .await;
    let (t1, t2) = (v2_token(0, 1), v2_token(0, 2));
    let (c9, t9) = ("0x00000000000000000000000000000000000000c9", v2_token(0, 9));
    let chain = FakeChain::new(
        b + 1_000,
        vec![
            // The store sells t1 to ALICE; the creator mints t2 to CARL.
            sales_index::tests::issue_log(c1, ALICE, t1, 0, sales_index::STORE_MATIC, s + 100, 0),
            transfer(c1, ZERO, ALICE, t1, s + 100, 1),
            sales_index::tests::issue_log(c1, CARL, t2, 0, CARL, s + 100, 2),
            transfer(c1, ZERO, CARL, t2, s + 100, 3),
            // The store's Bought prices its mint (same transaction).
            sales_index::tests::bought_log(
                &[(c1, 0, 3_000_000_000_000_000_000, ALICE)],
                &format!("0x{:064x}", (s + 100) * 100),
                s + 100,
                4,
            ),
            // BOB, CARL and DAVE bid on t1; ALICE accepts CARL's, which
            // drops DAVE's without an event; BOB cancels.
            sales_index::tests::bid_created_log(c1, t1, BOB, 0x11, 7, 1_700_000_000, b, 0),
            sales_index::tests::bid_created_log(c1, t1, CARL, 0x22, 8, 1_700_000_000, b + 1, 0),
            sales_index::tests::bid_created_log(c1, t1, DAVE, 0x55, 9, 1_700_000_000, b + 1, 1),
            sales_index::tests::bid_accepted_log(c1, t1, CARL, ALICE, 0x22, 8, b + 2, 0),
            transfer(c1, ALICE, CARL, t1, b + 2, 1),
            sales_index::tests::bid_cancelled_log(c1, t1, BOB, 0x11, b + 3, 0),
            // BOB bids twice on t2: the second replaces the first.
            sales_index::tests::bid_created_log(c1, t2, BOB, 0x33, 1, 1_700_000_000, b + 4, 0),
            sales_index::tests::bid_created_log(c1, t2, BOB, 0x44, 2, 1_700_000_100, b + 5, 0),
            // BOB bids on a token of a collection the nft walk never sees.
            sales_index::tests::bid_created_log(c9, t9, BOB, 0x66, 3, 1_700_000_000, b + 6, 0),
        ],
        1_000_000,
        100,
    );
    let url = serve(chain.clone()).await;
    let st = state_over(db.pool.clone(), &url, index_settings(100, 1_000_000), false);
    // The sales walk runs ahead of the nft walk: the bids fold without the
    // owners; the next fold after the nft walk fills exactly the sellers
    // it reached, and the one it never reaches is not revisited.
    assert!(publish(&st, &url, &SalesIndex).await.published);
    assert_eq!(
        bids_of(&db.pool)
            .await
            .iter()
            .filter(|row| row.3.is_none())
            .count(),
        4,
        "every bid but the accepted one waits for its seller"
    );
    assert!(
        publish_nft(&st, &url, &NftIndex::new(ChainNet::Matic))
            .await
            .published
    );
    let folded = chain_walk::cursor(&db.pool, INDEXER_SALES, ChainNet::Matic)
        .await
        .unwrap()
        .unwrap()
        .fold_through
        .unwrap();
    assert_eq!(
        sales_index::fold(
            &db.pool,
            Some(folded),
            &chain_walk::test_owner(&db.pool, &SalesIndex, &IndexSettings::default()).await
        )
        .await
        .unwrap(),
        3,
        "the sellers the nft walk reached are filled"
    );
    assert!(publish(&st, &url, &SalesIndex).await.published);
    assert!(sales_index::ready(&db.pool, chrono::Utc::now()).await);
    let status = sales_index::status(&db.pool).await;
    assert_eq!(status["matic"]["ready"], true, "{status}");

    let kinds: Vec<(String, i64)> = sqlx::query_as(
        "SELECT kind, count(*) FROM shop.sale_event WHERE network = 'matic' GROUP BY kind ORDER BY kind",
    )
    .fetch_all(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        kinds,
        vec![
            ("bid_accepted".to_string(), 1),
            ("bid_cancelled".to_string(), 1),
            ("bid_created".to_string(), 6),
            ("mint".to_string(), 1),
        ],
        "the creator's own Issue is not a sale"
    );
    let (buyer, item, tx, price): (String, String, String, Option<String>) = sqlx::query_as(
        "SELECT buyer, item_id::text, tx_hash, price::text FROM shop.sale_event WHERE network = 'matic' AND kind = 'mint'",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        (buyer.as_str(), item.as_str(), tx, price.as_deref()),
        (
            ALICE,
            "0",
            format!("0x{:064x}", (s + 100) * 100),
            Some("3000000000000000000")
        )
    );
    assert_eq!(
        bids_of(&db.pool).await,
        vec![
            (
                t1.to_string(),
                BOB.into(),
                bid_id(0x11),
                Some(ALICE.into()),
                "7".into(),
                "cancelled".into()
            ),
            (
                t1.to_string(),
                CARL.into(),
                bid_id(0x22),
                Some(ALICE.into()),
                "8".into(),
                "accepted".into()
            ),
            (
                t1.to_string(),
                DAVE.into(),
                bid_id(0x55),
                Some(ALICE.into()),
                "9".into(),
                "invalidated".into()
            ),
            (
                t2.to_string(),
                BOB.into(),
                bid_id(0x44),
                Some(CARL.into()),
                "2".into(),
                "open".into()
            ),
            (
                t9.to_string(),
                BOB.into(),
                bid_id(0x66),
                None,
                "3".into(),
                "open".into()
            ),
        ],
        "the seller is the owner when the bid was placed; a re-bid replaces the bidder's row; an acceptance invalidates the token's other open bids"
    );
    sqlx::query(
        "UPDATE shop.nft_bid SET status = 'cancelled' WHERE network = 'matic' AND bid_id = $1",
    )
    .bind(bid_id(0x66))
    .execute(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        sales_index::fold(
            &db.pool,
            Some(folded),
            &chain_walk::test_owner(&db.pool, &SalesIndex, &IndexSettings::default()).await
        )
        .await
        .unwrap(),
        0
    );
    let untouched: String = sqlx::query_scalar(
        "SELECT status FROM shop.nft_bid WHERE network = 'matic' AND bid_id = $1",
    )
    .bind(bid_id(0x66))
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        untouched, "cancelled",
        "a seller-less bid on a token the nft walk has no transfer for is not revisited"
    );
    // A database folded before the invalidation rule existed keeps DAVE's
    // bid open: the next incremental fold revisits it without a cursor reset.
    sqlx::query("UPDATE shop.nft_bid SET status = 'open' WHERE network = 'matic' AND bid_id = $1")
        .bind(bid_id(0x55))
        .execute(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        sales_index::fold(
            &db.pool,
            Some(folded),
            &chain_walk::test_owner(&db.pool, &SalesIndex, &IndexSettings::default()).await
        )
        .await
        .unwrap(),
        1
    );
    let healed: String = sqlx::query_scalar(
        "SELECT status FROM shop.nft_bid WHERE network = 'matic' AND bid_id = $1",
    )
    .bind(bid_id(0x55))
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        healed, "invalidated",
        "an open bid with a later acceptance is invalidated whatever fold saw the acceptance"
    );
    let (expires, created, updated): (i64, i64, i64) = sqlx::query_as(
        "SELECT expires_at, created_block, updated_block FROM shop.nft_bid WHERE network = 'matic' AND bid_id = $1",
    )
    .bind(bid_id(0x22))
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        (expires, created, updated),
        (1_700_000_000, (b + 1) as i64, (b + 2) as i64)
    );

    // The venue check: both charging venues answer the pinned cut; a changed
    // seed row is reported as a difference (and left as it is).
    let rpc = Rpc::at(&st.http, &url);
    let checks = sales_index::check_venues(&rpc, &db.pool).await.unwrap();
    assert_eq!(checks.len(), 2, "{checks:?}");
    assert!(
        checks.iter().all(|c| c.pinned == 25_000 && c.matches()),
        "{checks:?}"
    );
    sqlx::query(
        "UPDATE shop.sale_venue SET royalties_cut_per_million = 30000 WHERE kind = 'bid_v2'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let checks = sales_index::check_venues(&rpc, &db.pool).await.unwrap();
    let bid_v2 = checks.iter().find(|c| c.kind == "bid_v2").unwrap();
    assert!(
        !bid_v2.matches() && bid_v2.live == Some(U256::from(25_000)),
        "{checks:?}"
    );
    let pinned: i32 = sqlx::query_scalar(
        "SELECT royalties_cut_per_million FROM shop.sale_venue WHERE kind = 'bid_v2'",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(pinned, 30_000);
    db.drop().await;
}

#[tokio::test]
async fn a_bid_past_the_nft_walk_takes_no_seller_until_the_walk_passes_it() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let c1 = "0x00000000000000000000000000000000000000c1";
    let t1 = v2_token(0, 1).to_string();
    // The nft walk has indexed the mint to ALICE but not her transfer to
    // CARL; DAVE bids after the transfer.
    sqlx::query(
        "INSERT INTO shop.nft_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, from_address, to_address) \
         VALUES ('matic', 100, 0, 100, 'transfer', $1, $1, $2::numeric, $3, $4), ('matic', 110, 0, 110, 'transfer', $1, $1, $2::numeric, $4, $5)",
    )
    .bind(c1)
    .bind(&t1)
    .bind(ZERO)
    .bind(ALICE)
    .bind(CARL)
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, full_history, chain_id) VALUES ('nft', 'matic', 1, 105, true, 137)")
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO shop.sale_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, bid_id, buyer, price, expires_at, tx_hash) \
         VALUES ('matic', 120, 0, 120, 'bid_created', $1, $2, $3::numeric, $4, $5, 9, 1700000000, $6)",
    )
    .bind(sales_index::BID_V2_MATIC)
    .bind(c1)
    .bind(&t1)
    .bind(bid_id(0x77))
    .bind(DAVE)
    .bind(format!("0x{}", "7".repeat(64)))
    .execute(&db.pool)
    .await
    .unwrap();
    let seller = |pool: PgPool| async move {
        sqlx::query_scalar::<_, Option<String>>(
            "SELECT seller FROM shop.nft_bid WHERE network = 'matic' AND bid_id = $1",
        )
        .bind(bid_id(0x77))
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    assert_eq!(
        sales_index::fold(
            &db.pool,
            None,
            &chain_walk::test_owner(&db.pool, &SalesIndex, &IndexSettings::default()).await
        )
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        seller(db.pool.clone()).await,
        None,
        "the owner at the bid is past the nft walk: no seller, not the last owner it knows"
    );
    assert_eq!(
        sales_index::fold(
            &db.pool,
            Some(120),
            &chain_walk::test_owner(&db.pool, &SalesIndex, &IndexSettings::default()).await
        )
        .await
        .unwrap(),
        0,
        "not revisited while the walk is short of the bid"
    );
    sqlx::query(
        "UPDATE shop.chain_cursor SET last_block = 125 WHERE indexer = 'nft' AND network = 'matic'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    // The contract is still owed a backfill below its walked range: the
    // transfers before the bid may be missing, so the seller still waits.
    sqlx::query(
        "INSERT INTO shop.nft_contract (network, contract_address, walked_from_block, backfill_from_block) VALUES ('matic', $1, 120, 50)",
    )
    .bind(c1)
    .execute(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        sales_index::fold(
            &db.pool,
            Some(120),
            &chain_walk::test_owner(&db.pool, &SalesIndex, &IndexSettings::default()).await
        )
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        seller(db.pool.clone()).await,
        None,
        "a backfill owed hides the owner"
    );
    sqlx::query(
        "UPDATE shop.nft_contract SET backfilled_to_block = 119 WHERE network = 'matic' AND contract_address = $1",
    )
    .bind(c1)
    .execute(&db.pool)
    .await
    .unwrap();
    // The nft walk's history set apart: its progress counts for nothing, so
    // the seller still waits; once it is bound again the seller follows.
    sqlx::query("UPDATE shop.chain_cursor SET quarantined_at = timezone('UTC', now()), quarantine_reason = 'test' WHERE indexer = 'nft'")
        .execute(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        sales_index::fold(
            &db.pool,
            Some(120),
            &chain_walk::test_owner(&db.pool, &SalesIndex, &IndexSettings::default()).await
        )
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        seller(db.pool.clone()).await,
        None,
        "an nft history in quarantine hides the owner"
    );
    sqlx::query("UPDATE shop.chain_cursor SET quarantined_at = NULL, quarantine_reason = NULL WHERE indexer = 'nft'")
        .execute(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        sales_index::fold(
            &db.pool,
            Some(120),
            &chain_walk::test_owner(&db.pool, &SalesIndex, &IndexSettings::default()).await
        )
        .await
        .unwrap(),
        1
    );
    assert_eq!(seller(db.pool.clone()).await.as_deref(), Some(CARL));
    db.drop().await;
}

#[tokio::test]
async fn a_sales_reorg_rewinds_and_the_refold_follows_the_new_chain() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let s = nft_index::start_block(ChainNet::Matic);
    let b = sales_index::BID_V2_MATIC_START_BLOCK + 100;
    let c1 = "0x00000000000000000000000000000000000000c1";
    seed_collection(
        &db.pool,
        "matic",
        c1,
        s + 10,
        "urn:decentraland:matic:collections-v2:0xc1",
        "Pirates",
    )
    .await;
    let t1 = v2_token(0, 1);
    let chain = FakeChain::new(
        b + 1_000,
        vec![
            transfer(c1, ZERO, ALICE, t1, s + 100, 0),
            sales_index::tests::bid_created_log(c1, t1, CARL, 0x22, 8, 1_700_000_000, b, 0),
            sales_index::tests::bid_accepted_log(c1, t1, CARL, ALICE, 0x22, 8, b + 900, 0),
        ],
        1_000_000,
        100,
    );
    let url = serve(chain.clone()).await;
    let st = state_over(db.pool.clone(), &url, index_settings(100, 1_000_000), false);
    assert!(
        publish_nft(&st, &url, &NftIndex::new(ChainNet::Matic))
            .await
            .published
    );
    assert!(publish(&st, &url, &SalesIndex).await.published);
    assert_eq!(
        bids_of(&db.pool).await,
        vec![(
            t1.to_string(),
            CARL.into(),
            bid_id(0x22),
            Some(ALICE.into()),
            "8".into(),
            "accepted".into()
        )]
    );
    let cursor = chain_walk::cursor(&db.pool, INDEXER_SALES, ChainNet::Matic)
        .await
        .unwrap()
        .unwrap();
    // The chain reorganizes: the acceptance is gone and CARL cancels instead.
    chain
        .forks
        .lock()
        .insert(cursor.last_block as u64, format!("0x{}", "ee".repeat(32)));
    {
        let mut logs = chain.logs.lock();
        logs.retain(|l| l["blockNumber"] != json!(format!("0x{:x}", b + 900)));
        logs.push(log_json(&sales_index::tests::bid_cancelled_log(
            c1,
            t1,
            CARL,
            0x22,
            b + 880,
            0,
        )));
    }
    let rewound = pass(&st, &url, &SalesIndex).await;
    assert_eq!(
        rewound.reorg_rewound_to,
        Some(cursor.start_block - 1),
        "{rewound:?}"
    );
    assert!(
        !sales_index::ready(&db.pool, chrono::Utc::now()).await,
        "a rewind unpublishes"
    );
    chain.forks.lock().clear();
    assert!(publish(&st, &url, &SalesIndex).await.published);
    assert_eq!(
        bids_of(&db.pool).await,
        vec![(
            t1.to_string(),
            CARL.into(),
            bid_id(0x22),
            Some(ALICE.into()),
            "8".into(),
            "cancelled".into()
        )]
    );
    let orphans: i64 =
        sqlx::query_scalar("SELECT count(*) FROM shop.sale_event WHERE block_number = $1")
            .bind((b + 900) as i64)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(orphans, 0);
    db.drop().await;
}

#[test]
fn the_readiness_rule_is_shared_by_every_reader() {
    assert_eq!(chain_walk::READY_WINDOW, crate::land_authz::READY_WINDOW);
    assert_eq!(INDEXER_TRADE_EVENTS, "trade_events");
}

#[tokio::test]
async fn mana_sync_atomic_replay_and_reorg() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    use super::mana_transfers::{self, ManaTransfersIndex};
    let mana = mana_transfers::contract(ChainNet::Matic);
    let event = |block: u64, index: u64, from: &str, to: &str, amount: U256| {
        crate::chain_index::rpc::RpcLog {
            address: mana.clone(),
            topics: vec![
                mana_transfers::TRANSFER.into(),
                format!("0x{:0>64}", &from[2..]),
                format!("0x{:0>64}", &to[2..]),
            ],
            data: format!("0x{amount:064x}"),
            block_number: format!("0x{block:x}"),
            log_index: format!("0x{index:x}"),
            block_timestamp: Some(format!("0x{:x}", time_of(block))),
            block_hash: None,
            transaction_hash: Some(format!("0x{:064x}", block)),
        }
    };
    let chain = FakeChain::new(
        100,
        vec![
            event(90, 0, ALICE, BOB, U256::MAX),
            event(91, 1, ALICE, ALICE, U256::ZERO),
        ],
        10_000,
        1000,
    );
    chain.finalized_lag.store(2, Ordering::SeqCst);
    let url = serve(chain.clone()).await;
    let st = state_over(db.pool.clone(), &url, index_settings(100, 10_000), false);
    let app = super::market_router(st.clone());
    let path = format!("/v1/mana/transfers?network=matic&address={ALICE}&limit=1");
    assert_eq!(
        get_json(&app, &path).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    let ix = ManaTransfersIndex {
        net: ChainNet::Matic,
    };
    let first = pass(&st, &url, &ix).await;
    assert_eq!(
        first.indexed_to, 98,
        "uses finalized head, not fixed 64 blocks"
    );
    assert!(first.published);
    let c = chain_walk::cursor(&db.pool, chain_walk::INDEXER_MANA, ChainNet::Matic)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(c.last_block_hash, Some(chain.hash_of(98)));
    let (code, body) = get_json(&app, &path).await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(body["data"].as_array().unwrap().len(), 1);
    assert_eq!(body["data"][0]["amount"], "0");
    let next = format!("{path}&beforeBlock=91&beforeLog=1");
    let (_, page) = get_json(&app, &next).await;
    assert_eq!(page["data"][0]["amount"], U256::MAX.to_string());
    assert_eq!(page["next"], Value::Null);
    let replay = pass(&st, &url, &ix).await;
    assert_eq!(replay.rows_written, 0);
    let mut tx = db.pool.begin().await.unwrap();
    assert!(
        chain_walk::advance(
            &mut tx,
            chain_walk::INDEXER_MANA,
            ChainNet::Matic,
            (97, Some(chain.hash_of(97).as_str())),
            &crate::chain_index::rpc::BlockHeader {
                number: 99,
                hash: chain.hash_of(99),
                timestamp: time_of(99)
            }
        )
        .await
        .is_err()
    );
    tx.rollback().await.unwrap();
    chain
        .forks
        .lock()
        .insert(98, format!("0x{}", "ee".repeat(32)));
    chain.logs.lock().clear();
    let rewound = pass(&st, &url, &ix).await;
    assert_eq!(rewound.reorg_rewound_to, Some(-1));
    assert_eq!(
        get_json(&app, &path).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM shop.mana_transfer")
            .fetch_one(&db.pool)
            .await
            .unwrap(),
        0
    );
    assert!(pass(&st, &url, &ix).await.published);
    db.drop().await;
}

#[tokio::test]
async fn mana_sync_failed_window_rolls_back_and_legacy_cursor_replays() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    use super::mana_transfers::{self, ManaTransfersIndex};
    let bad = crate::chain_index::rpc::RpcLog {
        address: mana_transfers::contract(ChainNet::Matic),
        topics: vec![mana_transfers::TRANSFER.into()],
        data: "0x00".into(),
        block_number: "0x1".into(),
        log_index: "0x0".into(),
        block_timestamp: Some("0x1".into()),
        block_hash: None,
        transaction_hash: None,
    };
    let chain = FakeChain::new(100, vec![bad], 1000, 1000);
    let url = serve(chain.clone()).await;
    let st = state_over(db.pool.clone(), &url, index_settings(100, 1000), false);
    let ix = ManaTransfersIndex {
        net: ChainNet::Matic,
    };
    assert!(
        chain_walk::run_pass(
            &db.pool,
            &chain_walk::RpcSource::new(Rpc::at(&st.http, &url)),
            &ix,
            &st.settings.market.indexes,
            &chain_walk::test_owner(&db.pool, &ix, &st.settings.market.indexes).await,
            None,
            &|_| {}
        )
        .await
        .is_err()
    );
    let c = chain_walk::cursor(&db.pool, chain_walk::INDEXER_MANA, ChainNet::Matic)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(c.last_block, -1);
    chain.logs.lock().clear();
    assert!(pass(&st, &url, &ix).await.published);
    sqlx::query("UPDATE shop.chain_cursor SET last_block_hash=NULL WHERE indexer='mana_transfers'")
        .execute(&db.pool)
        .await
        .unwrap();
    assert_eq!(pass(&st, &url, &ix).await.reorg_rewound_to, Some(-1));
    db.drop().await;
}

#[tokio::test]
async fn mana_sync_migration_preserves_existing_cursors() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    sqlx::query("INSERT INTO shop.chain_cursor(indexer,network,start_block,last_block,full_history,chain_id) VALUES ('nft','matic',1,123,true,137)")
        .execute(&db.pool).await.unwrap();
    sqlx::raw_sql("ALTER TABLE shop.chain_cursor DROP CONSTRAINT chain_cursor_indexer_known; ALTER TABLE shop.chain_cursor ADD CONSTRAINT chain_cursor_indexer_known CHECK(indexer IN ('nft','trade_events','rentals')); DELETE FROM meta.schema_steps WHERE step='0006_chain_cursor_mana_transfers';")
        .execute(&db.pool).await.unwrap();
    crate::db::schema_migrate::migrate(&db.pool, crate::db::SCHEMA_SQL, crate::db::MIGRATIONS_SQL)
        .await
        .unwrap();
    assert_eq!(
        chain_walk::cursor(&db.pool, INDEXER_NFT, ChainNet::Matic)
            .await
            .unwrap()
            .unwrap()
            .last_block,
        123
    );
    chain_walk::test_owner(
        &db.pool,
        &super::mana_transfers::ManaTransfersIndex {
            net: ChainNet::Matic,
        },
        &IndexSettings::default(),
    )
    .await;
    db.drop().await;
}

#[tokio::test]
async fn mana_sync_refuses_missing_finality_and_a_fork_during_fetch() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let chain = FakeChain::new(100, vec![], 1000, 1000);
    let url = serve(chain.clone()).await;
    let st = state_over(db.pool.clone(), &url, index_settings(1, 1000), false);
    let ix = super::mana_transfers::ManaTransfersIndex {
        net: ChainNet::Matic,
    };
    chain.finalized_available.store(false, Ordering::SeqCst);
    assert!(
        chain_walk::run_pass(
            &db.pool,
            &chain_walk::RpcSource::new(Rpc::at(&st.http, &url)),
            &ix,
            &st.settings.market.indexes,
            &chain_walk::test_owner(&db.pool, &ix, &st.settings.market.indexes).await,
            None,
            &|_| {}
        )
        .await
        .is_err()
    );
    chain.finalized_available.store(true, Ordering::SeqCst);
    chain.reorg_on_logs.store(true, Ordering::SeqCst);
    assert!(
        chain_walk::run_pass(
            &db.pool,
            &chain_walk::RpcSource::new(Rpc::at(&st.http, &url)),
            &ix,
            &st.settings.market.indexes,
            &chain_walk::test_owner(&db.pool, &ix, &st.settings.market.indexes).await,
            None,
            &|_| {}
        )
        .await
        .is_err()
    );
    assert_eq!(
        chain_walk::cursor(&db.pool, chain_walk::INDEXER_MANA, ChainNet::Matic)
            .await
            .unwrap()
            .unwrap()
            .last_block,
        -1
    );
    assert!(
        pass(&st, &url, &ix).await.published,
        "even a one-request budget must make progress"
    );
    db.drop().await;
}

#[tokio::test]
async fn mana_sync_crash_after_window_keeps_the_hash() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let chain = FakeChain::new(100, vec![], 1000, 1000);
    let url = serve(chain.clone()).await;
    let st = state_over(db.pool.clone(), &url, index_settings(100, 1000), false);
    let runner = st.clone();
    let endpoint = url.clone();
    let crashed = tokio::spawn(async move {
        chain_walk::run_pass(
            &runner.pool,
            &chain_walk::RpcSource::new(Rpc::at(&runner.http, &endpoint)),
            &super::mana_transfers::ManaTransfersIndex {
                net: ChainNet::Matic,
            },
            &runner.settings.market.indexes,
            &chain_walk::test_owner(
                &runner.pool,
                &super::mana_transfers::ManaTransfersIndex {
                    net: ChainNet::Matic,
                },
                &runner.settings.market.indexes,
            )
            .await,
            None,
            &|_| panic!("injected stop after window commit"),
        )
        .await
    })
    .await;
    assert!(crashed.unwrap_err().is_panic());
    let c = chain_walk::cursor(&db.pool, chain_walk::INDEXER_MANA, ChainNet::Matic)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(c.last_block, 36);
    assert_eq!(
        c.last_block_hash,
        Some(chain.hash_of(36)),
        "committed window must survive a crash with its reorg anchor"
    );
    assert_eq!(c.folded_block, None);
    assert!(
        pass(
            &st,
            &url,
            &super::mana_transfers::ManaTransfersIndex {
                net: ChainNet::Matic
            }
        )
        .await
        .published
    );
    db.drop().await;
}

#[tokio::test]
async fn mana_sync_stop_and_regressed_finality_clear_publication() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let chain = FakeChain::new(100, vec![], 1000, 1000);
    let url = serve(chain.clone()).await;
    let st = state_over(db.pool.clone(), &url, index_settings(100, 1000), false);
    let ix = super::mana_transfers::ManaTransfersIndex {
        net: ChainNet::Matic,
    };
    let mut settings = st.settings.market.indexes.clone();
    settings.stop_block_matic = Some(36);
    let outcome = chain_walk::run_pass(
        &db.pool,
        &chain_walk::RpcSource::new(Rpc::at(&st.http, &url)),
        &ix,
        &settings,
        &chain_walk::test_owner(&db.pool, &ix, &settings).await,
        None,
        &|_| {
            *chain.head.lock() = 101;
        },
    )
    .await
    .unwrap();
    assert!(!outcome.published, "stop became historical while folding");
    assert!(pass(&st, &url, &ix).await.published);
    *chain.head.lock() = 90;
    assert!(
        !pass(&st, &url, &ix).await.published,
        "never publish beyond a regressed safe head"
    );
    let c = chain_walk::cursor(&db.pool, chain_walk::INDEXER_MANA, ChainNet::Matic)
        .await
        .unwrap()
        .unwrap();
    assert!(!chain_walk::is_ready(&c, chrono::Utc::now()));
    db.drop().await;
}

#[tokio::test]
async fn api_jobs_never_attach_market_indexing_even_when_enabled() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let mut settings = index_settings(100, 1000);
    settings.mana_transfers_enabled = true;
    settings.sales_enabled = true;
    settings.committee_enabled = true;
    let mut state = state_over(db.pool.clone(), "https://rpc.example.org/", settings, false);
    let inner = Arc::get_mut(&mut state).unwrap();
    inner.settings.market.mirrors_enabled = true;
    inner.settings.trades_sync_upstream_url = Some("https://market.example.org/".into());
    let api = crate::jobs::Jobs::new().with_fleet_jobs(false);
    state.spawn_api_jobs(&api);
    let names: Vec<_> = api.report().into_iter().map(|j| j.name).collect();
    for forbidden in [
        "chain-walk-ethereum",
        "chain-walk-matic",
        "collections-mirror",
        "items-mirror",
        "trades-mirror",
        "item-neighbours",
    ] {
        assert!(
            !names.iter().any(|n| n == forbidden),
            "API attached {forbidden}: {names:?}"
        );
    }
    let worker = crate::jobs::Jobs::new().with_fleet_jobs(false);
    state.spawn_index_jobs(&worker, None);
    let names: Vec<_> = worker.report().into_iter().map(|j| j.name).collect();
    for required in [
        "chain-walk-ethereum",
        "chain-walk-matic",
        "collections-mirror",
        "items-mirror",
        "trades-mirror",
        "item-neighbours",
    ] {
        assert!(
            names.iter().any(|n| n == required),
            "worker omitted {required}: {names:?}"
        );
    }
    assert!(
        api.shutdown(std::time::Duration::from_secs(5))
            .await
            .is_empty()
    );
    assert!(
        worker
            .shutdown(std::time::Duration::from_secs(5))
            .await
            .is_empty()
    );
    db.drop().await;
}

/// `catalyst index` walks `rentals-index-ethereum` whenever a chain-1
/// endpoint is configured, unless switched off.
#[tokio::test]
async fn the_rentals_index_attaches_behind_its_switch_and_a_chain_one_endpoint() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let attached = |state: &SharedState, enabled: Option<bool>| {
        super::chain_jobs::projections(state, ChainNet::Ethereum, enabled)
            .iter()
            .any(|p| p.lease == "rentals-index-ethereum")
    };
    let with_rpc = state_over(
        db.pool.clone(),
        "https://rpc.example.org/",
        IndexSettings::default(),
        false,
    );
    let mut no_rpc = state_over(
        db.pool.clone(),
        "https://rpc.example.org/",
        IndexSettings::default(),
        false,
    );
    Arc::get_mut(&mut no_rpc)
        .unwrap()
        .settings
        .market
        .trade_rpc_urls = RpcEndpoints::from_env(None);
    assert!(!attached(&with_rpc, Some(false)));
    assert!(attached(&with_rpc, Some(true)) && attached(&with_rpc, None));
    assert!(!attached(&no_rpc, Some(true)) && !attached(&no_rpc, None));
    assert!(
        super::chain_jobs::projections(&with_rpc, ChainNet::Matic, None)
            .iter()
            .all(|p| !p.lease.starts_with("rentals-index")),
        "the rentals venue is mainnet's"
    );
    db.drop().await;
}

/// The rentals walk over a fake chain feeds the notifications: a rent before
/// the first notification pass is history; a later rent notifies its start,
/// and its end once the published block's time passes it, with the
/// events-notifier metadata.
#[tokio::test]
async fn the_rentals_walk_feeds_rental_started_and_rental_ended() {
    use crate::market::transfer_notices::Pass;
    use crate::rentals::index::{RENTALS_MAINNET, RentalsIndex, tests::rented_log};
    use crate::rentals::notices::{ended_id, run_ended, run_started, started_id};
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let base = "https://decentraland.org/marketplace";
    let land = crate::land_authz::LAND_REGISTRY_MAINNET.to_ascii_lowercase();
    let lessor = "0x00000000000000000000000000000000000000a1";
    let tenant = "0x00000000000000000000000000000000000000b0";
    let day_blocks = 86_400 / 2;
    let rent = |token: U256, days: u64, block: u64| {
        let mut l = rented_log(
            &land,
            token,
            lessor,
            tenant,
            days,
            U256::from(1),
            &[block as u8; 65],
            block,
            0,
        );
        l.block_timestamp = Some(format!("0x{:x}", time_of(block)));
        l
    };
    // Parcel 1,2.
    let parcel = (U256::from(1u64) << 128) | U256::from(2u64);
    let chain = FakeChain::new(
        1_000 + 2 * day_blocks,
        vec![rent(U256::from(7u64), 1, 1_100)],
        10_000_000,
        10,
    );
    let url = serve(chain.clone()).await;
    let st = state_over(db.pool.clone(), &url, index_settings(10, 1_000_000), false);
    let ix = RentalsIndex::at(RENTALS_MAINNET, 1_000);
    assert!(pass(&st, &url, &ix).await.published);
    assert_eq!(
        run_started(&st.pool, base).await.unwrap(),
        Pass::Emitted(0),
        "no past rent"
    );
    assert_eq!(
        run_ended(&st.pool, base).await.unwrap(),
        Pass::Emitted(0),
        "no past end"
    );

    let rented_at = 1_000 + 2 * day_blocks + 100;
    chain
        .logs
        .lock()
        .push(log_json(&rent(parcel, 1, rented_at)));
    *chain.head.lock() = rented_at + 1_000;
    assert!(pass(&st, &url, &ix).await.published);
    assert_eq!(run_started(&st.pool, base).await.unwrap(), Pass::Emitted(1));
    assert_eq!(
        run_ended(&st.pool, base).await.unwrap(),
        Pass::Emitted(0),
        "still running"
    );

    *chain.head.lock() = rented_at + day_blocks + 1_000;
    assert!(pass(&st, &url, &ix).await.published);
    assert_eq!(run_ended(&st.pool, base).await.unwrap(), Pass::Emitted(1));
    assert_eq!(
        run_started(&st.pool, base).await.unwrap(),
        Pass::Emitted(0),
        "nothing twice"
    );

    let rows: Vec<(uuid::Uuid, String, String, Value)> = sqlx::query_as(
        "SELECT id, address, type, metadata FROM social.notifications ORDER BY occurred_at",
    )
    .fetch_all(&st.pool)
    .await
    .unwrap();
    let key: String = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT {} FROM shop.rental_event e WHERE e.kind = 'rented' AND e.block_number = $1",
        crate::market::notice_cursor::tx_log_key("rental_event", "e", "coalesce(e.signature, '')")
    )))
    .bind(rented_at as i64)
    .fetch_one(&st.pool)
    .await
    .unwrap();
    let (started, ended) = (time_of(rented_at), time_of(rented_at) + 86_400);
    let token = parcel.to_string();
    let link = format!("{base}/contracts/{land}/tokens/{token}/manage");
    let meta = |title: &str, description: String| {
        json!({
            "address": lessor, "contract": land, "lessor": lessor, "tenant": tenant,
            "operator": tenant, "startedAt": started.to_string(), "endedAt": ended.to_string(), "tokenId": token,
            "link": link, "title": title, "land": "1,2", "description": description,
        })
    };
    assert_eq!(
        rows,
        vec![
            (
                started_id("ethereum", &key),
                lessor.to_string(),
                "rental_started".to_string(),
                meta(
                    "LAND Rented",
                    format!("Your LAND at 1,2 was rented by {tenant}.")
                )
            ),
            (
                ended_id("ethereum", &key),
                lessor.to_string(),
                "rental_ended".to_string(),
                meta(
                    "Rent Period Ending",
                    "The rent of your LAND at 1,2 has ended.".to_string()
                )
            ),
        ]
    );
    db.drop().await;
}

/// The rewind path of `run_pass` (plan, delete, apply) moves a producer's
/// cursor back: the gift that replaces a reorged one is announced once the
/// index republishes.
#[tokio::test]
async fn a_reorg_rewind_replays_the_notification_producers() {
    use super::transfer_notices::{self, Pass};
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let s = nft_index::start_block(ChainNet::Matic);
    let c1 = "0x00000000000000000000000000000000000000c1";
    seed_collection(
        &db.pool,
        "matic",
        c1,
        s + 10,
        "urn:decentraland:matic:collections-v2:0xc1",
        "Pirates",
    )
    .await;
    let t1 = v2_token(0, 1);
    let chain = FakeChain::new(
        s + 1_500,
        vec![transfer(c1, ZERO, ALICE, t1, s + 100, 0)],
        1_000_000,
        100,
    );
    let url = serve(chain.clone()).await;
    let st = state_over(db.pool.clone(), &url, index_settings(50, 100_000), false);
    let ix = NftIndex::new(ChainNet::Matic);
    assert!(publish_nft(&st, &url, &ix).await.published);
    transfer_notices::tests::publish(
        &db.pool,
        INDEXER_TRADE_EVENTS,
        "matic",
        (s + 1_000_000) as i64,
        chrono::Utc::now().timestamp(),
    )
    .await;
    let notified = || async {
        sqlx::query_scalar::<_, String>(
            "SELECT address FROM social.notifications WHERE type = 'transfer_received' ORDER BY address",
        )
        .fetch_all(&db.pool)
        .await
        .unwrap()
    };
    assert_eq!(
        transfer_notices::run_once(&db.pool, 137).await.unwrap(),
        Pass::Emitted(0),
        "seeded at the mint"
    );

    chain
        .logs
        .lock()
        .push(log_json(&transfer(c1, ALICE, BOB, t1, s + 1_900, 0)));
    *chain.head.lock() = s + 2_000;
    assert!(publish_nft(&st, &url, &ix).await.published);
    assert_eq!(
        transfer_notices::run_once(&db.pool, 137).await.unwrap(),
        Pass::Emitted(1)
    );

    let cursor = chain_walk::cursor(&db.pool, INDEXER_NFT, ChainNet::Matic)
        .await
        .unwrap()
        .unwrap();
    chain
        .forks
        .lock()
        .insert(cursor.last_block as u64, format!("0x{}", "ee".repeat(32)));
    {
        let mut logs = chain.logs.lock();
        logs.retain(|l| l["blockNumber"] != json!(format!("0x{:x}", s + 1_900)));
        logs.push(log_json(&transfer(c1, ALICE, CARL, t1, s + 1_880, 0)));
    }
    assert!(pass(&st, &url, &ix).await.reorg_rewound_to.is_some());
    chain.forks.lock().clear();
    assert!(publish_nft(&st, &url, &ix).await.published);
    assert_eq!(
        transfer_notices::run_once(&db.pool, 137).await.unwrap(),
        Pass::Emitted(1),
        "the replacing gift is read again below the old cursor"
    );
    assert_eq!(notified().await, vec![BOB.to_string(), CARL.to_string()]);
    db.drop().await;
}

#[tokio::test]
async fn committee_index_keeps_member_set_logs_of_both_contracts() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    use super::committee_index::{CONTRACTS, CommitteeIndex, MEMBER_SET, START_BLOCK};
    let event =
        |block: u64, contract: &str, member: &str, set: bool| crate::chain_index::rpc::RpcLog {
            address: contract.into(),
            topics: vec![MEMBER_SET.into(), format!("0x{:0>64}", &member[2..])],
            data: format!("0x{:064x}", u8::from(set)),
            block_number: format!("0x{block:x}"),
            log_index: "0x0".into(),
            block_timestamp: Some(format!("0x{:x}", time_of(block))),
            block_hash: None,
            transaction_hash: Some(format!("0x{block:064x}")),
        };
    let s = START_BLOCK;
    let chain = FakeChain::new(
        s + 100,
        vec![
            event(s + 1, CONTRACTS[0], ALICE, true),
            event(s + 2, CONTRACTS[1], ALICE, false),
            event(s + 3, CONTRACTS[1], BOB, true),
        ],
        10_000,
        1000,
    );
    chain.finalized_lag.store(2, Ordering::SeqCst);
    let url = serve(chain.clone()).await;
    let st = state_over(db.pool.clone(), &url, index_settings(100, 10_000), false);
    let out = pass(&st, &url, &CommitteeIndex).await;
    assert!(out.published, "{out:?}");
    let rows: Vec<(String, String, bool)> = sqlx::query_as(
        "SELECT contract_address, member, is_member FROM shop.committee_event ORDER BY block_number",
    )
    .fetch_all(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        rows,
        vec![
            (CONTRACTS[0].to_string(), ALICE.to_string(), true),
            (CONTRACTS[1].to_string(), ALICE.to_string(), false),
            (CONTRACTS[1].to_string(), BOB.to_string(), true),
        ]
    );
    let count = || async {
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM shop.committee_event")
            .fetch_one(&db.pool)
            .await
            .unwrap()
    };

    // A walk over blocks it already holds writes nothing new.
    sqlx::query(
        "UPDATE shop.chain_cursor SET last_block = start_block - 1, last_block_hash = NULL, \
         last_block_time = NULL, fold_through = NULL, folded_block = NULL, folded_at = NULL WHERE indexer = 'committee'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let again = pass(&st, &url, &CommitteeIndex).await;
    assert!(again.published, "{again:?}");
    assert_eq!(again.rows_written, 0, "{again:?}");
    assert_eq!(count().await, 3);

    // A reorg that drops BOB's MemberSet: the rewind deletes the rows past
    // its target and unpublishes the index, and the next pass keeps only
    // the logs the chain still has.
    let cursor = chain_walk::cursor(&db.pool, chain_walk::INDEXER_COMMITTEE, ChainNet::Matic)
        .await
        .unwrap()
        .unwrap();
    chain
        .forks
        .lock()
        .insert(cursor.last_block as u64, format!("0x{}", "ee".repeat(32)));
    chain
        .logs
        .lock()
        .retain(|l| l["blockNumber"] != json!(format!("0x{:x}", s + 3)));
    let rewound = pass(&st, &url, &CommitteeIndex).await;
    assert!(rewound.reorg_rewound_to.is_some(), "{rewound:?}");
    assert!(!rewound.published, "{rewound:?}");
    assert_eq!(count().await, 0, "the rewind deletes the reorged rows");
    let unpublished: bool = sqlx::query_scalar(
        "SELECT folded_block IS NULL AND folded_at IS NULL FROM shop.chain_cursor WHERE indexer = 'committee'",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert!(unpublished, "the rewind clears the published block");
    chain.forks.lock().clear();
    let replayed = pass(&st, &url, &CommitteeIndex).await;
    assert!(replayed.published, "{replayed:?}");
    let members: Vec<String> =
        sqlx::query_scalar("SELECT member FROM shop.committee_event ORDER BY block_number")
            .fetch_all(&db.pool)
            .await
            .unwrap();
    assert_eq!(members, vec![ALICE.to_string(), ALICE.to_string()]);

    // The walk always rewinds below its start, so the partial case is
    // checked on the index directly: rows at or below the target survive.
    let mut conn = db.pool.acquire().await.unwrap();
    CommitteeIndex
        .rewind(&mut conn, (s + 1) as i64)
        .await
        .unwrap();
    drop(conn);
    let kept: Vec<(i64, bool)> =
        sqlx::query_as("SELECT block_number, is_member FROM shop.committee_event")
            .fetch_all(&db.pool)
            .await
            .unwrap();
    assert_eq!(kept, vec![((s + 1) as i64, true)]);
    db.drop().await;
}

/// A quiet committee stays published: a pass that finds no new MemberSet
/// still refreshes `folded_at`, so builder keeps reading non-admin members
/// past the readiness window.
#[tokio::test]
async fn a_committee_pass_without_new_logs_refreshes_the_publish() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    use super::committee_index::{CONTRACTS, CommitteeIndex, MEMBER_SET, START_BLOCK};
    let s = START_BLOCK;
    let chain = FakeChain::new(
        s + 100,
        vec![crate::chain_index::rpc::RpcLog {
            address: CONTRACTS[1].into(),
            topics: vec![MEMBER_SET.into(), format!("0x{:0>64}", &ALICE[2..])],
            data: format!("0x{:064x}", 1),
            block_number: format!("0x{:x}", s + 1),
            log_index: "0x0".into(),
            block_timestamp: Some(format!("0x{:x}", time_of(s + 1))),
            block_hash: None,
            transaction_hash: Some(format!("0x{:064x}", s + 1)),
        }],
        10_000,
        1000,
    );
    chain.finalized_lag.store(2, Ordering::SeqCst);
    let url = serve(chain.clone()).await;
    let st = state_over(db.pool.clone(), &url, index_settings(100, 10_000), false);
    assert!(pass(&st, &url, &CommitteeIndex).await.published);
    let age_out = || async {
        sqlx::query(
            "UPDATE shop.chain_cursor SET folded_at = folded_at - interval '2 hours' WHERE indexer = 'committee'",
        )
        .execute(&db.pool)
        .await
        .unwrap();
    };
    let ready = || async {
        let cursor = chain_walk::cursor(&db.pool, chain_walk::INDEXER_COMMITTEE, ChainNet::Matic)
            .await
            .unwrap()
            .unwrap();
        (
            chain_walk::is_ready(&cursor, chrono::Utc::now()),
            cursor.folded_block,
        )
    };

    // The head has not moved.
    age_out().await;
    assert!(!ready().await.0, "aged out");
    let quiet = pass(&st, &url, &CommitteeIndex).await;
    assert!(quiet.published, "{quiet:?}");
    assert_eq!(quiet.rows_written, 0, "{quiet:?}");
    let (is_ready, folded) = ready().await;
    assert!(is_ready, "the same head republishes");

    // New blocks, none with a MemberSet.
    age_out().await;
    *chain.head.lock() = s + 200;
    let quiet = pass(&st, &url, &CommitteeIndex).await;
    assert!(quiet.published, "{quiet:?}");
    assert_eq!(quiet.rows_written, 0, "{quiet:?}");
    let (is_ready, advanced) = ready().await;
    assert!(is_ready, "empty blocks republish");
    assert!(advanced > folded, "{advanced:?} > {folded:?}");
    db.drop().await;
}

/// The cursor columns a fenced writer must leave as they were.
async fn checkpoint(
    pool: &PgPool,
    indexer: &str,
    net: ChainNet,
) -> (i64, Option<String>, Option<i64>, Option<i64>, Option<i64>) {
    let c = chain_walk::cursor(pool, indexer, net)
        .await
        .unwrap()
        .unwrap();
    (
        c.last_block,
        c.last_block_hash,
        c.fold_through,
        c.folded_block,
        c.chain_id,
    )
}

/// The decisive fencing test: once B takes the projection at the same block
/// and hash, A commits nothing. That covers a window, a reorg rewind, a fold
/// whose writes were already made, the fold record, the publish and its
/// withdrawal, a trade-events digest batch and the nft backfill lane.
#[tokio::test]
async fn a_job_that_lost_its_projection_commits_nothing() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let s = nft_index::start_block(ChainNet::Matic);
    let c1 = "0x00000000000000000000000000000000000000c1";
    seed_collection(
        &db.pool,
        "matic",
        c1,
        s + 10,
        "urn:decentraland:matic:collections-v2:0xc1",
        "Pirates",
    )
    .await;
    let t1 = v2_token(0, 1);
    let head = s + 2_000;
    let chain = FakeChain::new(
        head,
        vec![transfer(c1, ZERO, ALICE, t1, s + 100, 0)],
        1_000_000,
        100,
    );
    let url = serve(chain.clone()).await;
    let st = state_over(db.pool.clone(), &url, index_settings(50, 100_000), false);
    let settings = st.settings.market.indexes.clone();
    let ix = NftIndex::new(ChainNet::Matic);
    let source = chain_walk::RpcSource::new(Rpc::at(&st.http, &url));
    let a = chain_walk::test_owner(&db.pool, &ix, &settings).await;
    let mut published = false;
    for _ in 0..10 {
        let o = chain_walk::run_pass(&db.pool, &source, &ix, &settings, &a, None, &|_| {})
            .await
            .unwrap();
        if o.published {
            published = true;
            break;
        }
    }
    assert!(published, "A walks and publishes");
    let before = checkpoint(&db.pool, INDEXER_NFT, ChainNet::Matic).await;
    let events = || async {
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM shop.nft_event")
            .fetch_one(&db.pool)
            .await
            .unwrap()
    };
    let owners = || async {
        sqlx::query_scalar::<_, String>("SELECT string_agg(contract_address || ':' || token_id || ':' || coalesce(owner, ''), ',' ORDER BY contract_address, token_id) FROM shop.nft")
            .fetch_one(&db.pool)
            .await
            .unwrap()
    };
    let (event_count, owned) = (events().await, owners().await);

    // B takes the projection at the same block and hash.
    let b = chain_walk::test_owner(&db.pool, &ix, &settings).await;
    assert!(b.generation > a.generation);

    // A window: new blocks carry a transfer.
    chain
        .logs
        .lock()
        .push(log_json(&transfer(c1, ALICE, BOB, t1, s + 2_050, 0)));
    *chain.head.lock() = head + 200;
    let err = chain_walk::run_pass(&db.pool, &source, &ix, &settings, &a, None, &|_| {})
        .await
        .unwrap_err();
    assert!(fenced(&err), "{err:#}");
    assert_eq!(events().await, event_count, "no window committed");

    // A reorg rewind: the cursor block has another hash.
    chain
        .forks
        .lock()
        .insert(before.0 as u64, format!("0x{}", "ee".repeat(32)));
    let err = chain_walk::run_pass(&db.pool, &source, &ix, &settings, &a, None, &|_| {})
        .await
        .unwrap_err();
    assert!(fenced(&err), "{err:#}");
    assert_eq!(events().await, event_count, "no rewind committed");
    chain.forks.lock().clear();

    // A fold whose writes reach the fence after B took the projection.
    sqlx::query("UPDATE shop.nft SET owner = NULL")
        .execute(&db.pool)
        .await
        .unwrap();
    let cleared = owners().await;
    let err = nft_index::fold(&db.pool, ChainNet::Matic, None, &a)
        .await
        .unwrap_err();
    assert!(fenced(&err), "{err:#}");
    assert_eq!(
        owners().await,
        cleared,
        "the fold's tentative writes rolled back"
    );
    sqlx::query("UPDATE shop.nft SET owner = $1")
        .bind(ALICE)
        .execute(&db.pool)
        .await
        .unwrap();
    let _ = owned;

    // The fold record, the publish and its withdrawal.
    for err in [
        chain_walk::record_fold(&db.pool, &a, before.0)
            .await
            .unwrap_err(),
        chain_walk::mark_folded(&db.pool, &a, before.0, chrono::Utc::now())
            .await
            .unwrap_err(),
        chain_walk::unpublish(&db.pool, &a, before.0)
            .await
            .unwrap_err(),
    ] {
        assert!(fenced(&err), "{err:#}");
    }
    assert_eq!(
        checkpoint(&db.pool, INDEXER_NFT, ChainNet::Matic).await,
        before
    );

    // A trade-events digest batch.
    let te = TradeEventsIndex {
        net: ChainNet::Matic,
        chain_id: 137,
    };
    let te_a = chain_walk::test_owner(&db.pool, &te, &settings).await;
    let _te_b = chain_walk::test_owner(&db.pool, &te, &settings).await;
    let v3 = trade_events::deployments(137)
        .iter()
        .find(|(_, _, v3)| *v3)
        .expect("a V3 marketplace on Polygon")
        .0;
    seed_trade(
        &db.pool,
        "public_nft_order",
        ALICE,
        v3,
        1,
        0,
        (3, c1, "1"),
        (1, ZERO, "1"),
    )
    .await;
    let err = trade_events::fold(&db.pool, ChainNet::Matic, 137, None, 0, &te_a)
        .await
        .unwrap_err();
    assert!(fenced(&err), "{err:#}");
    let states: i64 = sqlx::query_scalar("SELECT count(*) FROM shop.trade_chain_state")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(states, 0, "no digest batch committed");

    // The nft backfill lane.
    let lane = "0x00000000000000000000000000000000000000c9";
    sqlx::query("INSERT INTO shop.nft_contract (network, contract_address, walked_from_block, backfill_from_block) VALUES ('matic', $1, $2, $3)")
        .bind(lane)
        .bind((s + 1_500) as i64)
        .bind((s + 10) as i64)
        .execute(&db.pool)
        .await
        .unwrap();
    chain.logs.lock().push(log_json(&transfer(
        lane,
        ZERO,
        CARL,
        v2_token(0, 2),
        s + 50,
        0,
    )));
    let err = chain_walk::run_pass(&db.pool, &source, &ix, &settings, &a, None, &|_| {})
        .await
        .unwrap_err();
    assert!(fenced(&err), "{err:#}");
    let progress: Option<i64> = sqlx::query_scalar(
        "SELECT backfilled_to_block FROM shop.nft_contract WHERE contract_address = $1",
    )
    .bind(lane)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        (progress, events().await),
        (None, event_count),
        "no lane progress committed"
    );

    // B walks on.
    let o = chain_walk::run_pass(&db.pool, &source, &ix, &settings, &b, None, &|_| {})
        .await
        .unwrap();
    assert!(o.indexed_to > before.0, "{o:?}");
    db.drop().await;
}

/// A cursor from before the chain binding is adopted when its last block's
/// hash proves its history on the chain, and set apart when it does not. A
/// source that does not answer decides nothing. Revalidation keeps the
/// history; a rebuild starts the projection over on its own network only and
/// converges to the same rows.
#[tokio::test]
async fn a_legacy_cursor_is_adopted_by_proof_or_set_apart() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let s = nft_index::start_block(ChainNet::Matic);
    let c1 = "0x00000000000000000000000000000000000000c1";
    seed_collection(
        &db.pool,
        "matic",
        c1,
        s + 10,
        "urn:decentraland:matic:collections-v2:0xc1",
        "Pirates",
    )
    .await;
    let t1 = v2_token(0, 1);
    let head = s + 2_000;
    let chain = FakeChain::new(
        head,
        vec![
            transfer(c1, ZERO, ALICE, t1, s + 100, 0),
            transfer(c1, ALICE, BOB, t1, s + 900, 0),
        ],
        1_000_000,
        100,
    );
    let url = serve(chain.clone()).await;
    let st = state_over(db.pool.clone(), &url, index_settings(50, 100_000), false);
    let ix = NftIndex::new(ChainNet::Matic);
    assert!(publish_nft(&st, &url, &ix).await.published);
    let ready = || async {
        chain_walk::ready_cursor(&db.pool, INDEXER_NFT, ChainNet::Matic, chrono::Utc::now())
            .await
            .is_some()
    };
    let cursor = || async {
        chain_walk::cursor(&db.pool, INDEXER_NFT, ChainNet::Matic)
            .await
            .unwrap()
            .unwrap()
    };
    let set = |sql: &'static str| {
        let pool = db.pool.clone();
        async move { sqlx::query(sql).execute(&pool).await.unwrap() }
    };
    assert!(ready().await);
    let walked = cursor().await;
    let events: i64 = sqlx::query_scalar("SELECT count(*) FROM shop.nft_event")
        .fetch_one(&db.pool)
        .await
        .unwrap();

    // Published but unbound (a row from before the binding): not ready.
    set("UPDATE shop.chain_cursor SET chain_id = NULL WHERE indexer = 'nft'").await;
    assert!(!ready().await, "an unadopted legacy cursor is not served");
    // A backend that does not answer: the pass fails and decides nothing.
    chain.fail_headers.store(true, Ordering::SeqCst);
    assert!(try_pass(&st, &url, &ix).await.is_err());
    let c = cursor().await;
    assert_eq!((c.chain_id, c.quarantine_reason), (None, None));
    chain.fail_headers.store(false, Ordering::SeqCst);
    // The proof, on a pass with nothing new: adopted and published again.
    let o = pass(&st, &url, &ix).await;
    assert!(o.published && o.quarantined.is_none(), "{o:?}");
    assert_eq!(cursor().await.chain_id, Some(137));
    assert!(ready().await);

    // Bound to another chain: set apart; revalidation keeps the history.
    set("UPDATE shop.chain_cursor SET chain_id = 80002 WHERE indexer = 'nft'").await;
    let o = pass(&st, &url, &ix).await;
    assert!(
        o.quarantined
            .as_deref()
            .is_some_and(|w| w.contains("80002")),
        "{o:?}"
    );
    assert!(!ready().await);
    set("UPDATE shop.chain_cursor SET quarantined_at = NULL, quarantine_reason = NULL, chain_id = NULL, owner_generation = owner_generation + 1 WHERE indexer = 'nft' AND network = 'matic' AND quarantined_at IS NOT NULL").await;
    let o = pass(&st, &url, &ix).await;
    assert!(o.quarantined.is_none() && o.published, "{o:?}");
    let kept: i64 = sqlx::query_scalar("SELECT count(*) FROM shop.nft_event")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(kept, events, "revalidation keeps the history");

    // A stored hash the chain does not have: set apart, unpublished, not walked.
    set("UPDATE shop.chain_cursor SET chain_id = NULL, last_block_hash = '0xabababababababababababababababababababababababababababababababab' WHERE indexer = 'nft'").await;
    let o = pass(&st, &url, &ix).await;
    assert!(o.quarantined.is_some(), "{o:?}");
    *chain.head.lock() = head + 100;
    let o2 = pass(&st, &url, &ix).await;
    assert!(
        o2.quarantined.is_some() && !o2.published && o2.indexed_to == o.indexed_to,
        "{o2:?}"
    );
    assert!(cursor().await.folded_block.is_none() && !ready().await);

    // A rebuild of that unproven history is refused and keeps its rows and
    // the notification producers' position.
    sqlx::query("INSERT INTO shop.notification_cursor (producer, block_number, log_index, seed_block, seed_log_index) VALUES ('transfer_received', $1, 0, 0, 0)")
        .bind((s + 500) as i64)
        .execute(&db.pool)
        .await
        .unwrap();
    let matic_events = || async {
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM shop.nft_event WHERE network = 'matic'")
            .fetch_one(&db.pool)
            .await
            .unwrap()
    };
    let producer = || async {
        sqlx::query_scalar::<_, i64>("SELECT block_number FROM shop.notification_cursor WHERE producer = 'transfer_received'")
            .fetch_one(&db.pool)
            .await
            .unwrap()
    };
    let unproven = matic_events().await;
    set("UPDATE shop.chain_cursor SET rebuild_requested_at = timezone('UTC', now()), folded_block = NULL, folded_at = NULL, owner_generation = owner_generation + 1 WHERE indexer = 'nft' AND network = 'matic'").await;
    let o = pass(&st, &url, &ix).await;
    assert!(
        o.quarantined
            .as_deref()
            .is_some_and(|w| w.starts_with("rebuild refused") && w.contains("stored")),
        "{o:?}"
    );
    assert_eq!(
        matic_events().await,
        unproven,
        "the unproven history is kept"
    );
    assert_eq!(
        producer().await,
        (s + 500) as i64,
        "the producers keep their position"
    );
    assert!(cursor().await.rebuild_requested_at.is_none());

    // Proven again (the hash restored and revalidated), a rebuild of the
    // bound history runs: the other network's rows stay, the old owner is
    // fenced, and the chain stays bound.
    sqlx::query("UPDATE shop.chain_cursor SET last_block_hash = $1 WHERE indexer = 'nft'")
        .bind(chain.hash_of(cursor().await.last_block as u64))
        .execute(&db.pool)
        .await
        .unwrap();
    set("UPDATE shop.chain_cursor SET quarantined_at = NULL, quarantine_reason = NULL, chain_id = NULL, owner_generation = owner_generation + 1 WHERE indexer = 'nft' AND network = 'matic' AND quarantined_at IS NOT NULL").await;
    assert!(pass(&st, &url, &ix).await.quarantined.is_none());
    assert_eq!(cursor().await.chain_id, Some(137));
    sqlx::query("INSERT INTO shop.nft_event (network, block_number, log_index, kind, emitter, contract_address, token_id, from_address, to_address, block_time) VALUES ('ethereum', 1, 0, 'transfer', $1, $1, 1, $2, $3, 1)")
        .bind(c1)
        .bind(ZERO)
        .bind(ALICE)
        .execute(&db.pool)
        .await
        .unwrap();
    let old = chain_walk::test_owner(&db.pool, &ix, &st.settings.market.indexes).await;
    set("UPDATE shop.chain_cursor SET rebuild_requested_at = timezone('UTC', now()), folded_block = NULL, folded_at = NULL, owner_generation = owner_generation + 1 WHERE indexer = 'nft' AND network = 'matic'").await;
    assert!(fenced(
        &chain_walk::record_fold(&db.pool, &old, walked.last_block)
            .await
            .unwrap_err()
    ));
    let o = pass(&st, &url, &ix).await;
    assert!(o.rebuilt, "{o:?}");
    let c = cursor().await;
    assert_eq!(
        (
            c.last_block,
            c.chain_id,
            c.quarantine_reason.as_deref(),
            c.rebuild_requested_at
        ),
        (c.start_block - 1, Some(137), None, None),
        "the rebuilt cursor stays bound to the chain it proved"
    );
    let matic: i64 =
        sqlx::query_scalar("SELECT count(*) FROM shop.nft_event WHERE network = 'matic'")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    let ethereum: i64 =
        sqlx::query_scalar("SELECT count(*) FROM shop.nft_event WHERE network = 'ethereum'")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!((matic, ethereum), (0, 1));
    assert!(publish_nft(&st, &url, &ix).await.published);
    assert_eq!(cursor().await.chain_id, Some(137));
    assert_eq!(
        owner_of(&db.pool, "matic", c1, t1).await,
        Some(Some(BOB.to_string())),
        "the rebuild converges"
    );
    db.drop().await;
}

/// A job pass checks the source's chain before anything else: an endpoint
/// that serves another chain fails the pass and decides nothing. A write
/// fenced by an operator's generation raise drops the holding, and the next
/// pass claims the projection again.
#[tokio::test]
async fn a_job_pass_refuses_another_chain_and_claims_again_after_a_fence() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let chain = FakeChain::new(100, vec![], 1000, 1000);
    chain.finalized_lag.store(2, Ordering::SeqCst);
    let url = serve(chain.clone()).await;
    let st = state_over(db.pool.clone(), &url, index_settings(100, 1000), false);
    let settings = st.settings.market.indexes.clone();
    let ix = super::mana_transfers::ManaTransfersIndex {
        net: ChainNet::Matic,
    };
    let source = chain_walk::RpcSource::new(Rpc::at(&st.http, &url));
    let holdings = chain_walk::Holdings::default();
    let run = || {
        chain_walk::job_pass(
            &db.pool,
            &source,
            &ix,
            &settings,
            &holdings,
            None,
            None,
            &|_| {},
        )
    };
    let first = run().await.unwrap();
    assert!(first.published, "{first:?}");
    let claimed = chain_walk::cursor(&db.pool, chain_walk::INDEXER_MANA, ChainNet::Matic)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claimed.chain_id, Some(137));

    chain.chain_id.store(80_002, Ordering::SeqCst);
    let err = run().await.unwrap_err();
    assert!(
        err.chain()
            .any(|c| c.downcast_ref::<chain_walk::WrongChain>().is_some()),
        "a typed error, so a two-network job skips only this network: {err:#}"
    );
    assert!(
        err.to_string().contains("serves chain 80002, not 137"),
        "{err}"
    );
    let after = chain_walk::cursor(&db.pool, chain_walk::INDEXER_MANA, ChainNet::Matic)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            after.chain_id,
            after.quarantine_reason,
            after.owner_generation,
            after.last_block
        ),
        (
            Some(137),
            None,
            claimed.owner_generation,
            claimed.last_block
        ),
        "a wrong endpoint decides nothing about the data"
    );
    chain.chain_id.store(137, Ordering::SeqCst);

    sqlx::query("UPDATE shop.chain_cursor SET owner_generation = owner_generation + 1 WHERE indexer = 'mana_transfers'")
        .execute(&db.pool)
        .await
        .unwrap();
    *chain.head.lock() = 110;
    let err = run().await.unwrap_err();
    assert!(fenced(&err), "{err:#}");
    let again = run().await.unwrap();
    assert!(again.indexed_to > claimed.last_block, "{again:?}");
    let c = chain_walk::cursor(&db.pool, chain_walk::INDEXER_MANA, ChainNet::Matic)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(c.owner_generation, claimed.owner_generation + 2);
    db.drop().await;
}

/// The verdict SQL of the listings lends a trade-events verdict only from a
/// cursor bound to the chain this process serves (mainnet in the tests):
/// one bound to another chain is ignored before any walk runs.
#[tokio::test]
async fn trade_verdicts_come_only_from_the_served_chain() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let c1 = "0x00000000000000000000000000000000000000c1";
    let (id, _) = seed_trade(
        &db.pool,
        "public_nft_order",
        ALICE,
        "0x00000000000000000000000000000000000000aa",
        1,
        0,
        (3, c1, "1"),
        (1, ZERO, "1"),
    )
    .await;
    sqlx::query("INSERT INTO shop.trade_chain_state (trade_id, network, status, folded_block) VALUES ($1::uuid, 'matic', 'cancelled', 10)")
        .bind(&id)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, fold_through, folded_block, folded_at, full_history, chain_id) VALUES ('trade_events', 'matic', 0, 10, 10, 10, timezone('UTC', now()), true, 80002)")
        .execute(&db.pool)
        .await
        .unwrap();
    let verdict = || async {
        let sql = format!(
            "SELECT {} AS closed, {} AS status FROM shop.trades t WHERE t.id = $2::uuid",
            trade_events::closed_on_chain_sql("t", "$1"),
            trade_events::chain_status_sql("t", "$1")
        );
        sqlx::query_as::<_, (bool, Option<String>)>(sqlx::AssertSqlSafe(sql))
            .bind(chrono::Utc::now().naive_utc())
            .bind(&id)
            .fetch_one(&db.pool)
            .await
            .unwrap()
    };
    assert_eq!(
        verdict().await,
        (false, None),
        "a cursor bound to another chain lends nothing"
    );
    sqlx::query("UPDATE shop.chain_cursor SET chain_id = 137 WHERE indexer = 'trade_events'")
        .execute(&db.pool)
        .await
        .unwrap();
    assert_eq!(verdict().await, (true, Some("cancelled".to_string())));
    db.drop().await;
}

/// A rebuild keeps the projection's chain and start block: asked on a
/// cursor whose history is from another chain, it is refused, the cursor is
/// set apart and nothing is deleted.
#[tokio::test]
async fn a_rebuild_across_chains_is_refused() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let polygon = TradeEventsIndex::mainnet(ChainNet::Matic);
    let amoy = TradeEventsIndex {
        net: ChainNet::Matic,
        chain_id: 80_002,
    };
    assert_ne!(polygon.start_block(), amoy.start_block());
    let settings = IndexSettings::default();
    chain_walk::test_owner(&db.pool, &polygon, &settings).await;
    sqlx::query("UPDATE shop.chain_cursor SET last_block = start_block + 10, last_block_hash = $1 WHERE indexer = 'trade_events'")
        .bind(format!("0x{}", "12".repeat(32)))
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO shop.trade_event (network, block_number, log_index, block_time, marketplace_address, kind, caller, hashed_signature, signer, assets, tx_hash) VALUES ('matic', 1, 0, 1, '0xm', 'traded', $1, '0xsig', $1, '{}'::jsonb, $2)")
        .bind(ALICE)
        .bind(format!("0x{}", "3".repeat(64)))
        .execute(&db.pool)
        .await
        .unwrap();
    let before = chain_walk::cursor(&db.pool, INDEXER_TRADE_EVENTS, ChainNet::Matic)
        .await
        .unwrap()
        .unwrap();
    sqlx::query("UPDATE shop.chain_cursor SET rebuild_requested_at = timezone('UTC', now()), owner_generation = owner_generation + 1 WHERE indexer = 'trade_events'")
        .execute(&db.pool)
        .await
        .unwrap();
    let chain = FakeChain::new(1_000, vec![], 10_000, 1_000);
    chain.chain_id.store(80_002, Ordering::SeqCst);
    let url = serve(chain).await;
    let http = crate::http::Outbound::for_tests();
    let source = chain_walk::RpcSource::new(Rpc::at(&http, &url));
    let owner = chain_walk::test_owner(&db.pool, &amoy, &settings).await;
    let o = chain_walk::run_pass(&db.pool, &source, &amoy, &settings, &owner, None, &|_| {})
        .await
        .unwrap();
    assert!(
        o.quarantined
            .as_deref()
            .is_some_and(|w| w.starts_with("rebuild refused")),
        "{o:?}"
    );
    let after = chain_walk::cursor(&db.pool, INDEXER_TRADE_EVENTS, ChainNet::Matic)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            after.last_block,
            after.start_block,
            after.chain_id,
            after.rebuild_requested_at.is_none()
        ),
        (before.last_block, before.start_block, before.chain_id, true)
    );
    let kept: i64 = sqlx::query_scalar("SELECT count(*) FROM shop.trade_event")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(kept, 1, "nothing is deleted");
    db.drop().await;
}

/// A rebuild keeps the chain it proved: a node that starts on another chain
/// after the cleanup, before the replay, finds the cursor bound to the old
/// one and sets it apart instead of walking an empty history unproven.
#[tokio::test]
async fn a_rebuilt_cursor_keeps_its_chain_against_a_switch_before_the_replay() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let polygon = TradeEventsIndex::mainnet(ChainNet::Matic);
    let amoy = TradeEventsIndex {
        net: ChainNet::Matic,
        chain_id: 80_002,
    };
    let settings = IndexSettings::default();
    let chain = FakeChain::new(polygon.start_block() + 1_000, vec![], 1_000_000, 1_000);
    let url = serve(chain.clone()).await;
    let http = crate::http::Outbound::for_tests();
    let source = chain_walk::RpcSource::new(Rpc::at(&http, &url));
    let owner = chain_walk::test_owner(&db.pool, &polygon, &settings).await;
    let at = polygon.start_block() + 10;
    sqlx::query("UPDATE shop.chain_cursor SET last_block = $1, last_block_hash = $2, rebuild_requested_at = timezone('UTC', now()) WHERE indexer = 'trade_events'")
        .bind(at as i64)
        .bind(chain.hash_of(at))
        .execute(&db.pool)
        .await
        .unwrap();
    let o = chain_walk::run_pass(
        &db.pool,
        &source,
        &polygon,
        &settings,
        &owner,
        None,
        &|_| {},
    )
    .await
    .unwrap();
    assert!(o.rebuilt, "{o:?}");
    let c = chain_walk::cursor(&db.pool, INDEXER_TRADE_EVENTS, ChainNet::Matic)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (c.last_block, c.chain_id),
        (polygon.start_block() as i64 - 1, Some(137))
    );

    chain.chain_id.store(80_002, Ordering::SeqCst);
    let switched = chain_walk::test_owner(&db.pool, &amoy, &settings).await;
    let o = chain_walk::run_pass(
        &db.pool,
        &source,
        &amoy,
        &settings,
        &switched,
        None,
        &|_| {},
    )
    .await
    .unwrap();
    assert!(
        o.quarantined
            .as_deref()
            .is_some_and(|w| w.contains("bound to chain 137")),
        "{o:?}"
    );
    db.drop().await;
}

/// The chain index supervisor (`chain_jobs`): one projection per lease, each
/// on its own cadence, its attempt interrupted when its lease is lost.
mod supervisor {
    use super::*;
    use crate::jobs::{JobState, Jobs, LeaseSettings, PeriodicCfg};
    use crate::market::chain_group;
    use crate::market::chain_jobs::{self, MakeIndex, Projection, Supervisor};
    use std::time::Duration;

    /// A real index with hooks: a fold that waits for `fold_gate` (counting
    /// the entries, and the waits dropped before the gate opened), a
    /// `groups` that panics, and an `abandon` that panics too.
    #[derive(Default)]
    struct Hooked {
        inner: Option<Arc<dyn ChainIndex>>,
        fold_gate: Option<Arc<tokio::sync::Semaphore>>,
        entered: Arc<AtomicUsize>,
        dropped: Arc<AtomicUsize>,
        panics: Option<Arc<AtomicUsize>>,
        abandon_panics: bool,
        /// While set, `write` waits for `write_gate` (counting the entries
        /// in `write_entered`) and `groups` for `groups_gate`.
        armed: Arc<std::sync::atomic::AtomicBool>,
        write_gate: Option<Arc<tokio::sync::Semaphore>>,
        write_entered: Arc<AtomicUsize>,
        groups_gate: Option<Arc<tokio::sync::Semaphore>>,
        /// The next writes that fail.
        write_fails: Arc<AtomicUsize>,
        /// While `armed`, `fold` waits for this gate.
        armed_fold_gate: Option<Arc<tokio::sync::Semaphore>>,
        /// The filters `groups` answers instead of the index's own.
        groups_override: Option<Vec<chain_walk::LogGroup>>,
        /// While `commit_armed`, each window's commit returns only once this
        /// gate opens (the commit itself has landed), counting in
        /// `write_entered`.
        committed_gate: Option<Arc<tokio::sync::Semaphore>>,
        commit_armed: Arc<std::sync::atomic::AtomicBool>,
    }

    impl Hooked {
        fn inner(&self) -> &dyn ChainIndex {
            self.inner.as_deref().expect("a hooked index wraps one")
        }
    }

    /// Counts a wait dropped before it ended.
    struct DroppedWait(Arc<AtomicUsize>);

    impl Drop for DroppedWait {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[async_trait::async_trait]
    impl ChainIndex for Hooked {
        fn indexer(&self) -> &'static str {
            self.inner().indexer()
        }
        fn net(&self) -> ChainNet {
            self.inner().net()
        }
        fn chain_id(&self) -> i64 {
            self.inner().chain_id()
        }
        fn start_block(&self) -> u64 {
            self.inner().start_block()
        }
        async fn groups(
            &self,
            pool: &PgPool,
            from: u64,
            to: u64,
            anchor: (u64, i64),
        ) -> anyhow::Result<Vec<chain_walk::LogGroup>> {
            if let Some(panics) = &self.panics {
                panics.fetch_add(1, Ordering::SeqCst);
                panic!("a hooked index panicked");
            }
            if let Some(gate) = &self.groups_gate
                && self.armed.load(Ordering::SeqCst)
            {
                gate.acquire().await.unwrap().forget();
            }
            if let Some(groups) = &self.groups_override {
                return Ok(groups.clone());
            }
            self.inner().groups(pool, from, to, anchor).await
        }
        async fn write(
            &self,
            conn: &mut sqlx::PgConnection,
            logs: &[crate::chain_index::rpc::RpcLog],
            times: &HashMap<i64, i64>,
        ) -> anyhow::Result<usize> {
            if self
                .write_fails
                .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                .is_ok()
            {
                anyhow::bail!("a hooked write failed");
            }
            if let Some(gate) = &self.write_gate
                && self.armed.load(Ordering::SeqCst)
            {
                self.write_entered.fetch_add(1, Ordering::SeqCst);
                gate.acquire().await.unwrap().forget();
            }
            self.inner().write(conn, logs, times).await
        }
        async fn rewind(&self, conn: &mut sqlx::PgConnection, to: i64) -> anyhow::Result<()> {
            self.inner().rewind(conn, to).await
        }
        async fn clear(&self, conn: &mut sqlx::PgConnection, to: i64) -> anyhow::Result<()> {
            self.inner().clear(conn, to).await
        }
        async fn fold(
            &self,
            pool: &PgPool,
            since: Option<i64>,
            upto: i64,
            owner: &chain_walk::Owner,
        ) -> anyhow::Result<bool> {
            if let Some(gate) = &self.fold_gate {
                self.entered.fetch_add(1, Ordering::SeqCst);
                let wait = DroppedWait(self.dropped.clone());
                gate.acquire().await.unwrap().forget();
                std::mem::forget(wait);
            }
            if let Some(gate) = &self.armed_fold_gate
                && self.armed.load(Ordering::SeqCst)
            {
                self.entered.fetch_add(1, Ordering::SeqCst);
                gate.acquire().await.unwrap().forget();
            }
            self.inner().fold(pool, since, upto, owner).await
        }
        fn abandon(&self) {
            assert!(!self.abandon_panics, "a hooked index's cleanup panicked");
            self.inner().abandon()
        }
        fn begin_pass(&self) {
            self.inner().begin_pass()
        }
        async fn walks_alone(&self, pool: &PgPool) -> anyhow::Result<bool> {
            self.inner().walks_alone(pool).await
        }
        async fn window_committed(&self) {
            if let Some(gate) = &self.committed_gate
                && self.commit_armed.load(Ordering::SeqCst)
            {
                self.write_entered.fetch_add(1, Ordering::SeqCst);
                gate.acquire().await.unwrap().forget();
            }
        }
    }

    /// The market test settings, every Polygon projection walked from block
    /// 0 (the fake chains end at block 100).
    fn walk_settings() -> IndexSettings {
        IndexSettings {
            start_block_matic: Some(0),
            ..index_settings(100, 1000)
        }
    }

    fn supervisor(st: &SharedState, net: ChainNet, projections: Vec<Projection>) -> Supervisor {
        Supervisor {
            name: chain_jobs::job_name(net),
            pool: st.pool.clone(),
            http: st.http.clone(),
            settings: st.settings.market.indexes.clone(),
            projections,
            member_deadline: Duration::from_secs(60),
            stats: Arc::default(),
        }
    }

    fn projection(job: &str, net: ChainNet, url: &str, make: MakeIndex) -> Projection {
        Projection {
            lease: chain_jobs::lease_name(job, net),
            url: url.to_string(),
            cfg: PeriodicCfg::every(Duration::from_millis(50))
                .with_error_backoff(Duration::from_millis(400)),
            make,
            gate: None,
        }
    }

    fn mana(net: ChainNet) -> MakeIndex {
        Arc::new(move || Arc::new(super::super::mana_transfers::ManaTransfersIndex { net }))
    }

    fn lease_settings(namespace: &str, poll: Duration) -> LeaseSettings {
        LeaseSettings {
            namespace: namespace.to_string(),
            poll,
            keepalive: Duration::from_millis(200),
        }
    }

    async fn eventually<F, Fut>(what: &str, mut check: F)
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while !check().await {
            assert!(
                std::time::Instant::now() < deadline,
                "timed out waiting: {what}"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    async fn cursor(pool: &PgPool, indexer: &str, net: ChainNet) -> Option<chain_walk::Cursor> {
        chain_walk::cursor(pool, indexer, net).await.unwrap()
    }

    async fn published(pool: &PgPool, indexer: &str, net: ChainNet) -> bool {
        cursor(pool, indexer, net)
            .await
            .is_some_and(|c| c.folded_block == Some(c.last_block) && c.last_block >= 0)
    }

    /// A URL whose connections are refused.
    async fn refusing_endpoint() -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        format!("http://{addr}/")
    }

    /// A URL that accepts connections and never answers.
    async fn hanging_endpoint() -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let mut held = Vec::new();
            while let Ok((socket, _)) = listener.accept().await {
                held.push(socket);
            }
        });
        format!("http://{addr}/")
    }

    /// The advisory locks held in this database: the leases taken.
    async fn advisory_locks(pool: &PgPool) -> i64 {
        sqlx::query_scalar(
            "SELECT count(*) FROM pg_locks WHERE locktype = 'advisory' \
             AND database = (SELECT oid FROM pg_database WHERE datname = current_database())",
        )
        .fetch_one(pool)
        .await
        .unwrap()
    }

    /// A projection keeps walking on its own cadence while, on the same
    /// network, one fails on a refusing endpoint, one waits on an endpoint
    /// that never answers and one panics, whose cleanup would panic too: the
    /// panicking one is retried with a fresh index, every finished attempt
    /// counts as a pass, and stopping the process interrupts the one still
    /// waiting.
    #[tokio::test]
    async fn a_projection_keeps_its_turns_while_others_fail_hang_or_panic() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let mana_chain = FakeChain::new(100, vec![], 1000, 1000);
        mana_chain.finalized_lag.store(2, Ordering::SeqCst);
        let mana_url = serve(mana_chain.clone()).await;
        let sales_chain = FakeChain::new(100, vec![], 1000, 1000);
        sales_chain.finalized_lag.store(2, Ordering::SeqCst);
        let sales_url = serve(sales_chain.clone()).await;
        let st = state_over(db.pool.clone(), &mana_url, walk_settings(), false);
        let (panics, made) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
        let sales: MakeIndex = {
            let (panics, made) = (panics.clone(), made.clone());
            Arc::new(move || {
                made.fetch_add(1, Ordering::SeqCst);
                Arc::new(Hooked {
                    inner: Some(Arc::new(super::super::sales_index::SalesIndex)),
                    panics: Some(panics.clone()),
                    abandon_panics: true,
                    ..Hooked::default()
                }) as Arc<dyn ChainIndex>
            })
        };
        let projections = vec![
            projection(
                "mana-transfers-index",
                ChainNet::Matic,
                &mana_url,
                mana(ChainNet::Matic),
            ),
            projection(
                "trade-events-index",
                ChainNet::Matic,
                &refusing_endpoint().await,
                Arc::new(|| {
                    Arc::new(TradeEventsIndex {
                        net: ChainNet::Matic,
                        chain_id: 137,
                    })
                }),
            ),
            projection(
                "committee-index",
                ChainNet::Matic,
                &hanging_endpoint().await,
                Arc::new(|| Arc::new(super::super::committee_index::CommitteeIndex)),
            ),
            projection("sales-index", ChainNet::Matic, &sales_url, sales),
        ];
        let namespace = format!("supervisor-turns-{}", std::process::id());
        let jobs = Jobs::with_leases(
            db.pool.clone(),
            lease_settings(&namespace, Duration::from_millis(100)),
        )
        .with_first_pass_now(true);
        chain_jobs::spawn(&jobs, supervisor(&st, ChainNet::Matic, projections));
        eventually("mana published", || {
            published(&db.pool, chain_walk::INDEXER_MANA, ChainNet::Matic)
        })
        .await;
        eventually("the panicking projection retried", || {
            let panics = panics.clone();
            async move { panics.load(Ordering::SeqCst) >= 2 }
        })
        .await;
        let before = mana_chain.requests.load(Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(600)).await;
        let after = mana_chain.requests.load(Ordering::SeqCst);
        assert!(
            after > before + 10,
            "mana keeps its 50 ms cadence while the others fail, hang or panic: {before} -> {after}"
        );
        eventually("a third panic", || {
            let panics = panics.clone();
            async move { panics.load(Ordering::SeqCst) >= 3 }
        })
        .await;
        let panicked = panics.load(Ordering::SeqCst);
        assert!(
            made.load(Ordering::SeqCst) >= panicked,
            "each retry after a panic runs on a fresh index: {} indexes for {panicked} panics",
            made.load(Ordering::SeqCst)
        );
        assert!(
            cursor(&db.pool, chain_walk::INDEXER_TRADE_EVENTS, ChainNet::Matic)
                .await
                .is_none(),
            "a refusing endpoint claims nothing"
        );
        let report = jobs.report();
        let job = report
            .iter()
            .find(|j| j.name == "chain-walk-matic")
            .unwrap();
        assert!(
            !matches!(job.state, JobState::Finished | JobState::Failed),
            "a panicking attempt never ends the supervisor"
        );
        assert!(
            job.passes >= 4,
            "every finished attempt counts: {}",
            job.passes
        );
        assert!(
            jobs.shutdown(Duration::from_secs(5)).await.is_empty(),
            "stopping interrupts the attempt waiting on the silent endpoint"
        );
        db.drop().await;
    }

    /// The process's leases with a gate per name before the request, names
    /// whose request panics, and every name asked for, in order.
    struct GatedLeases {
        inner: Arc<crate::jobs::Leases>,
        gates: HashMap<String, Arc<tokio::sync::Semaphore>>,
        panics: Vec<String>,
        asked: Arc<Mutex<Vec<String>>>,
        /// When each panicking request started.
        panicked_at: Arc<Mutex<Vec<std::time::Instant>>>,
    }

    #[async_trait::async_trait]
    impl chain_jobs::TakeLease for GatedLeases {
        async fn try_acquire(&self, name: &str) -> sqlx::Result<Option<crate::jobs::LeaseGuard>> {
            self.asked.lock().push(name.to_string());
            if self.panics.iter().any(|n| n == name) {
                self.panicked_at.lock().push(std::time::Instant::now());
                panic!("a lease request panicked");
            }
            if let Some(gate) = self.gates.get(name) {
                gate.acquire().await.unwrap().forget();
            }
            self.inner.try_acquire(name).await
        }
        fn standby_wait(&self, name: &str) -> Duration {
            self.inner.settings().standby_wait(name)
        }
    }

    fn gated(
        jobs: &Jobs,
        gated: &[&str],
    ) -> (
        Arc<GatedLeases>,
        Arc<tokio::sync::Semaphore>,
        Arc<Mutex<Vec<String>>>,
    ) {
        let gate = Arc::new(tokio::sync::Semaphore::new(0));
        let asked = Arc::new(Mutex::new(Vec::new()));
        let leases = Arc::new(GatedLeases {
            inner: jobs.leases().expect("a process with leases").clone(),
            gates: gated
                .iter()
                .map(|n| (n.to_string(), gate.clone()))
                .collect(),
            panics: Vec::new(),
            asked: asked.clone(),
            panicked_at: Arc::default(),
        });
        (leases, gate, asked)
    }

    /// A lease request that panics fails like one: the supervisor keeps
    /// running, asks again only after the lease poll, and the other
    /// projection keeps walking.
    #[tokio::test]
    async fn a_panicking_lease_request_is_asked_again_after_the_poll() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let chain = FakeChain::new(100, vec![], 1000, 1000);
        chain.finalized_lag.store(2, Ordering::SeqCst);
        let url = serve(chain.clone()).await;
        let st = state_over(db.pool.clone(), &url, walk_settings(), false);
        let namespace = format!("supervisor-request-panic-{}", std::process::id());
        let jobs = Jobs::with_leases(
            db.pool.clone(),
            lease_settings(&namespace, Duration::from_millis(200)),
        )
        .with_first_pass_now(true);
        let panicked_at = Arc::new(Mutex::new(Vec::new()));
        let leases = Arc::new(GatedLeases {
            inner: jobs.leases().unwrap().clone(),
            gates: HashMap::new(),
            panics: vec!["committee-index-matic".to_string()],
            asked: Arc::default(),
            panicked_at: panicked_at.clone(),
        });
        chain_jobs::spawn_with(
            &jobs,
            supervisor(
                &st,
                ChainNet::Matic,
                vec![
                    projection(
                        "mana-transfers-index",
                        ChainNet::Matic,
                        &url,
                        mana(ChainNet::Matic),
                    ),
                    projection(
                        "committee-index",
                        ChainNet::Matic,
                        &url,
                        Arc::new(|| Arc::new(super::super::committee_index::CommitteeIndex)),
                    ),
                ],
            ),
            Some(leases),
        );
        eventually("mana published", || {
            published(&db.pool, chain_walk::INDEXER_MANA, ChainNet::Matic)
        })
        .await;
        eventually("three panicking requests", || {
            let panicked_at = panicked_at.clone();
            async move { panicked_at.lock().len() >= 3 }
        })
        .await;
        let starts = panicked_at.lock().clone();
        for pair in starts.windows(2) {
            let gap = pair[1] - pair[0];
            assert!(
                gap >= Duration::from_millis(200),
                "asked again after the 200 ms poll, not in a loop: {gap:?}"
            );
        }
        let report = jobs.report();
        let job = report
            .iter()
            .find(|j| j.name == "chain-walk-matic")
            .unwrap();
        assert!(!matches!(job.state, JobState::Finished | JobState::Failed));
        assert!(
            cursor(&db.pool, chain_walk::INDEXER_COMMITTEE, ChainNet::Matic)
                .await
                .is_none()
        );
        assert!(jobs.shutdown(Duration::from_secs(5)).await.is_empty());
        db.drop().await;
    }

    /// A supervisor whose only lease request is still in flight has not
    /// been told another process holds anything, so it does not report
    /// standby; it does once a request comes back without the lease.
    #[tokio::test]
    async fn a_lease_request_in_flight_is_not_standby() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let chain = FakeChain::new(100, vec![], 1000, 1000);
        chain.finalized_lag.store(2, Ordering::SeqCst);
        let url = serve(chain.clone()).await;
        let st = state_over(db.pool.clone(), &url, walk_settings(), false);
        let namespace = format!("supervisor-in-flight-{}", std::process::id());
        let elsewhere = Arc::new(crate::jobs::Leases::new(
            db.pool.clone(),
            lease_settings(&namespace, Duration::from_millis(100)),
        ));
        let held = elsewhere
            .try_acquire("mana-transfers-index-matic")
            .await
            .unwrap()
            .expect("another process holds mana's lease");
        let jobs = Jobs::with_leases(
            db.pool.clone(),
            lease_settings(&namespace, Duration::from_millis(100)),
        )
        .with_first_pass_now(true);
        let (leases, gate, asked) = gated(&jobs, &["mana-transfers-index-matic"]);
        chain_jobs::spawn_with(
            &jobs,
            supervisor(
                &st,
                ChainNet::Matic,
                vec![projection(
                    "mana-transfers-index",
                    ChainNet::Matic,
                    &url,
                    mana(ChainNet::Matic),
                )],
            ),
            Some(leases),
        );
        let state = || {
            jobs.report()
                .into_iter()
                .find(|j| j.name == "chain-walk-matic")
                .unwrap()
                .state
        };
        eventually("the request in flight", || {
            let asked = asked.clone();
            async move { !asked.lock().is_empty() }
        })
        .await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_ne!(state(), JobState::Standby, "no answer yet");
        gate.add_permits(1);
        eventually(
            "standby once the request comes back without the lease",
            || async { state() == JobState::Standby },
        )
        .await;
        gate.add_permits(100);
        held.release().await;
        assert!(jobs.shutdown(Duration::from_secs(5)).await.is_empty());
        db.drop().await;
    }

    /// The rows of the projections' tables, without the columns a walk
    /// stamps with the time it ran.
    async fn table_rows(pool: &PgPool, table: &str) -> Vec<String> {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT (to_jsonb(t) - 'updated_at' - 'created_at' - 'id' - 'indexed_at' - 'mirrored_at' - 'added_at')::text \
             FROM {table} t ORDER BY 1"
        )))
        .fetch_all(pool)
        .await
        .unwrap()
    }

    const PROJECTION_TABLES: [&str; 6] = [
        "shop.nft",
        "shop.nft_event",
        "shop.nft_contract",
        "shop.nft_order",
        "shop.nft_bid",
        "shop.sale_event",
    ];

    /// The nft and sales walks standing at one checkpoint read their next
    /// window once: the same rows as two walks alone, from fewer requests, in
    /// the pinned order (the finalized header serves as the window's before).
    #[tokio::test]
    async fn two_projections_at_one_checkpoint_read_their_windows_once() {
        let (Some(alone), Some(together)) = (ScratchDb::create().await, ScratchDb::create().await)
        else {
            return;
        };
        let s = nft_index::start_block(ChainNet::Matic);
        let c1 = "0x00000000000000000000000000000000000000c1";
        for db in [&alone, &together] {
            seed_collection(
                &db.pool,
                "matic",
                c1,
                s + 10,
                "urn:decentraland:matic:collections-v2:0xc1",
                "Pirates",
            )
            .await;
        }
        let (t1, t2) = (v2_token(0, 1), v2_token(0, 2));
        let chain = FakeChain::new(
            s + 500,
            vec![
                sales_index::tests::issue_log(
                    c1,
                    ALICE,
                    t1,
                    0,
                    sales_index::STORE_MATIC,
                    s + 100,
                    0,
                ),
                transfer(c1, ZERO, ALICE, t1, s + 100, 1),
            ],
            1_000_000,
            100,
        );
        chain.finalized_lag.store(2, Ordering::SeqCst);
        let url = serve(chain.clone()).await;
        let settings = index_settings(100, 1_000_000);
        let (st_alone, st_together) = (
            state_over(alone.pool.clone(), &url, settings.clone(), false),
            state_over(together.pool.clone(), &url, settings.clone(), false),
        );
        // Both walk alone to the head first, in both databases.
        for st in [&st_alone, &st_together] {
            assert!(
                publish(st, &url, &NftIndex::new(ChainNet::Matic))
                    .await
                    .published
            );
            assert!(publish(st, &url, &SalesIndex).await.published);
        }
        // The next segment: CARL mints t2 and sells it to BOB.
        {
            let mut logs = chain.logs.lock();
            for l in [
                sales_index::tests::issue_log(c1, CARL, t2, 0, CARL, s + 700, 0),
                transfer(c1, ZERO, CARL, t2, s + 700, 1),
                transfer(c1, CARL, BOB, t2, s + 900, 0),
            ] {
                logs.push(log_json(&l));
            }
        }
        *chain.head.lock() = s + 1_500;
        let mark = chain.calls.lock().len();
        let nft = NftIndex::new(ChainNet::Matic);
        pass(&st_alone, &url, &nft).await;
        pass(&st_alone, &url, &SalesIndex).await;
        let alone_calls = chain.calls.lock()[mark..].to_vec();

        let at = chain_group::Checkpoint::of(
            &chain_walk::cursor(&together.pool, INDEXER_NFT, ChainNet::Matic)
                .await
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        let mark = chain.calls.lock().len();
        let members: Vec<chain_group::Member> = vec![
            Arc::new(NftIndex::new(ChainNet::Matic)) as Arc<dyn ChainIndex>,
            Arc::new(SalesIndex),
        ]
        .into_iter()
        .enumerate()
        .map(|(slot, index)| chain_group::Member {
            slot,
            index,
            holdings: Arc::new(chain_walk::Holdings::default()),
            lease: None,
            gate: None,
        })
        .collect();
        let indexes: Vec<_> = members
            .iter()
            .map(|m| (m.index.clone(), m.holdings.clone()))
            .collect();
        let counted = AtomicUsize::new(0);
        let ends = chain_group::walk_together(
            &together.pool,
            &st_together.http,
            &url,
            &settings,
            members,
            &at,
            Duration::from_secs(60),
            &counted,
        )
        .await;
        let sent = counted.load(Ordering::SeqCst);
        for (slot, end) in ends {
            let chain_group::MemberEnd::Walked {
                mut out,
                close: Some(fold_since),
                ..
            } = end
            else {
                panic!("member {slot} did not walk to the target");
            };
            let (index, holdings) = &indexes[slot];
            let owner = holdings
                .owner(&together.pool, None, index.as_ref(), &settings)
                .await
                .unwrap();
            let source = chain_walk::RpcSource::new(Rpc::at(&st_together.http, &url));
            chain_walk::close(
                &together.pool,
                &source,
                index.as_ref(),
                &settings,
                &owner,
                fold_since,
                &mut out,
            )
            .await
            .unwrap();
            assert!(out.published, "{out:?}");
        }
        let together_calls = chain.calls.lock()[mark..].to_vec();
        let logs = format!("logs {}..{} x1", s + 499, s + 1_498);
        let expected: Vec<String> = [
            "chain id".to_string(),
            // The checkpoint's header, once for both (the reorg check).
            format!("header {}", s + 498),
            "head".into(),
            "finalized".into(),
            // The window's before is the finalized header: one read for
            // both, each address once.
            logs.clone(),
            logs.clone(),
            logs,
            format!("header {}", s + 1_498),
            // Each close reads the heads again after its fold.
            "head".into(),
            "finalized".into(),
            "head".into(),
            "finalized".into(),
        ]
        .into();
        assert_eq!(together_calls, expected, "{together_calls:#?}");
        assert_eq!(sent, 8, "the shared walk's own requests");
        assert_eq!(alone_calls.len(), 17, "{alone_calls:#?}");
        for table in PROJECTION_TABLES {
            assert_eq!(
                table_rows(&together.pool, table).await,
                table_rows(&alone.pool, table).await,
                "{table}"
            );
        }
        assert!(together_calls.len() < alone_calls.len());
        alone.drop().await;
        together.drop().await;
    }

    /// The nft and sales projections of `url` for a supervisor, ungated.
    fn nft_and_sales(url: &str) -> Vec<Projection> {
        vec![
            projection(
                "nft-index",
                ChainNet::Matic,
                url,
                Arc::new(|| Arc::new(NftIndex::new(ChainNet::Matic))),
            ),
            projection(
                "sales-index",
                ChainNet::Matic,
                url,
                Arc::new(|| Arc::new(SalesIndex)),
            ),
        ]
    }

    async fn last_block(pool: &PgPool, indexer: &str) -> i64 {
        cursor(pool, indexer, ChainNet::Matic)
            .await
            .map_or(-1, |c| c.last_block)
    }

    /// When the shared read always fails but each projection's own filters
    /// work, every projection still advances, on a budget of one request a
    /// pass: each falls back alone with what it had left, and a fallback
    /// that has nothing left walks alone again on a whole budget.
    #[tokio::test]
    async fn a_shared_read_that_always_fails_still_lets_every_projection_advance() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let s = nft_index::start_block(ChainNet::Matic);
        let c1 = "0x00000000000000000000000000000000000000c1";
        seed_collection(
            &db.pool,
            "matic",
            c1,
            s + 10,
            "urn:decentraland:matic:collections-v2:0xc1",
            "Pirates",
        )
        .await;
        let chain = FakeChain::new(
            s + 500,
            vec![
                sales_index::tests::issue_log(
                    c1,
                    ALICE,
                    v2_token(0, 1),
                    0,
                    sales_index::STORE_MATIC,
                    s + 100,
                    0,
                ),
                transfer(c1, ZERO, ALICE, v2_token(0, 1), s + 100, 1),
            ],
            1_000_000,
            100,
        );
        chain.finalized_lag.store(2, Ordering::SeqCst);
        // The union of the two first topics on the collection is refused;
        // each projection's own read is not.
        *chain.refuse_topics.lock() = vec![
            nft_index::TOPIC_TRANSFER.to_string(),
            sales_index::TOPIC_ISSUE.to_string(),
        ];
        let url = serve(chain.clone()).await;
        let settings = IndexSettings {
            requests_per_pass: 1,
            ..index_settings(100, 1_000_000)
        };
        let st = state_over(db.pool.clone(), &url, settings, false);
        let jobs = Jobs::new().with_first_pass_now(true);
        chain_jobs::spawn(&jobs, supervisor(&st, ChainNet::Matic, nft_and_sales(&url)));
        for indexer in [INDEXER_NFT, INDEXER_SALES] {
            eventually("each walked alone to the head", || async {
                published(&db.pool, indexer, ChainNet::Matic).await
            })
            .await;
        }
        for round in 1..=3u64 {
            let block = s + 500 + round * 1_000;
            {
                let mut logs = chain.logs.lock();
                let token = v2_token(0, round + 1);
                for l in [
                    sales_index::tests::issue_log(c1, CARL, token, 0, CARL, block - 300, 0),
                    transfer(c1, ZERO, CARL, token, block - 300, 1),
                ] {
                    logs.push(log_json(&l));
                }
            }
            *chain.head.lock() = block;
            let target = (block - 2) as i64;
            for indexer in [INDEXER_NFT, INDEXER_SALES] {
                eventually("every cursor advances", || async {
                    last_block(&db.pool, indexer).await >= target
                })
                .await;
            }
        }
        assert!(
            chain.refused.load(Ordering::SeqCst) > 0,
            "the shared read was tried"
        );
        assert!(jobs.shutdown(Duration::from_secs(5)).await.is_empty());
        db.drop().await;
    }

    /// Leases taken from two processes' sessions: the names in `apart`
    /// from the second, so one of them can be lost alone.
    struct SplitLeases {
        first: Arc<crate::jobs::Leases>,
        second: Arc<crate::jobs::Leases>,
        apart: Vec<String>,
    }

    #[async_trait::async_trait]
    impl chain_jobs::TakeLease for SplitLeases {
        async fn try_acquire(&self, name: &str) -> sqlx::Result<Option<crate::jobs::LeaseGuard>> {
            if self.apart.iter().any(|n| n == name) {
                self.second.try_acquire(name).await
            } else {
                self.first.try_acquire(name).await
            }
        }
        fn standby_wait(&self, name: &str) -> Duration {
            self.first.settings().standby_wait(name)
        }
    }

    /// Ends the session that holds the lease `name`.
    async fn lose_lease(pool: &PgPool, name: &str) {
        let ended: Vec<bool> = sqlx::query_scalar(
            "SELECT pg_terminate_backend(pid) FROM pg_locks WHERE locktype = 'advisory' AND objsubid = 2 \
             AND objid::bigint = (hashtext($1)::bigint & 4294967295) \
             AND database = (SELECT oid FROM pg_database WHERE datname = current_database())",
        )
        .bind(name)
        .fetch_all(pool)
        .await
        .unwrap();
        assert_eq!(ended, vec![true], "one session held {name}");
    }

    async fn sale_events(pool: &PgPool) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM shop.sale_event")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    /// A store sale of `token` to BOB at `block`: its Issue, its mint and
    /// the store's Bought, in one transaction.
    fn add_sale(chain: &FakeChain, c1: &str, token: U256, block: u64) {
        let mut logs = chain.logs.lock();
        for l in [
            sales_index::tests::issue_log(c1, BOB, token, 0, sales_index::STORE_MATIC, block, 0),
            transfer(c1, ZERO, BOB, token, block, 1),
            sales_index::tests::bought_log(
                &[(c1, 0, 1_000_000_000_000_000_000, BOB)],
                &format!("0x{:064x}", block * 100),
                block,
                2,
            ),
        ] {
            logs.push(log_json(&l));
        }
    }

    fn add_mint(chain: &FakeChain, c1: &str, token: U256, block: u64) {
        let mut logs = chain.logs.lock();
        for l in [
            sales_index::tests::issue_log(c1, CARL, token, 0, CARL, block, 0),
            transfer(c1, ZERO, CARL, token, block, 1),
        ] {
            logs.push(log_json(&l));
        }
    }

    fn hooked_sales(hooks: &Hooked) -> MakeIndex {
        let (armed, gate, entered) = (
            hooks.armed.clone(),
            hooks.write_gate.clone(),
            hooks.write_entered.clone(),
        );
        Arc::new(move || {
            Arc::new(Hooked {
                inner: Some(Arc::new(SalesIndex)),
                armed: armed.clone(),
                write_gate: gate.clone(),
                write_entered: entered.clone(),
                ..Hooked::default()
            }) as Arc<dyn ChainIndex>
        })
    }

    /// A shared walk loses sales' lease alone while sales waits in its
    /// commit, after nft committed the window: the walk stops, nft keeps the
    /// window it committed and sales its last one, and both go on from
    /// there to the same rows.
    #[tokio::test]
    async fn a_lease_lost_between_two_commits_leaves_each_at_its_own_durable_point() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let s = nft_index::start_block(ChainNet::Matic);
        let c1 = "0x00000000000000000000000000000000000000c1";
        seed_collection(
            &db.pool,
            "matic",
            c1,
            s + 10,
            "urn:decentraland:matic:collections-v2:0xc1",
            "Pirates",
        )
        .await;
        let chain = FakeChain::new(s + 500, vec![], 1_000_000, 100);
        chain.finalized_lag.store(2, Ordering::SeqCst);
        add_mint(&chain, c1, v2_token(0, 1), s + 100);
        let url = serve(chain.clone()).await;
        let st = state_over(db.pool.clone(), &url, index_settings(100, 1_000_000), false);
        let namespace = format!("shared-lease-loss-{}", std::process::id());
        let first = Jobs::with_leases(
            db.pool.clone(),
            lease_settings(&namespace, Duration::from_millis(100)),
        )
        .with_first_pass_now(true);
        let second = Jobs::with_leases(
            db.pool.clone(),
            lease_settings(&namespace, Duration::from_millis(100)),
        );
        let hooks = Hooked {
            write_gate: Some(Arc::new(tokio::sync::Semaphore::new(0))),
            ..Hooked::default()
        };
        let leases = Arc::new(SplitLeases {
            first: first.leases().unwrap().clone(),
            second: second.leases().unwrap().clone(),
            apart: vec!["sales-index-matic".into()],
        });
        let mut projections = nft_and_sales(&url);
        projections[1].make = hooked_sales(&hooks);
        chain_jobs::spawn_with(
            &first,
            supervisor(&st, ChainNet::Matic, projections),
            Some(leases),
        );
        for indexer in [INDEXER_NFT, INDEXER_SALES] {
            eventually("each walked alone to the head", || async {
                published(&db.pool, indexer, ChainNet::Matic).await
            })
            .await;
        }
        let before = cursor(&db.pool, INDEXER_SALES, ChainNet::Matic)
            .await
            .unwrap()
            .last_block;
        let sales_before = sale_events(&db.pool).await;
        hooks.armed.store(true, Ordering::SeqCst);
        add_sale(&chain, c1, v2_token(0, 2), s + 700);
        *chain.head.lock() = s + 1_500;
        let target = (s + 1_498) as i64;
        eventually("sales waits in its commit", || async {
            hooks.write_entered.load(Ordering::SeqCst) >= 1
        })
        .await;
        let progress = first
            .report()
            .into_iter()
            .find(|j| j.name == "chain-walk-matic")
            .and_then(|j| j.progress)
            .unwrap_or_default();
        assert!(progress.contains("walking together"), "{progress}");
        assert_eq!(
            last_block(&db.pool, INDEXER_NFT).await,
            target,
            "nft committed the window first"
        );
        lose_lease(&db.pool, "sales-index-matic").await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert_eq!(
            last_block(&db.pool, INDEXER_SALES).await,
            before,
            "sales' commit never landed"
        );
        assert_eq!(sale_events(&db.pool).await, sales_before);
        hooks.armed.store(false, Ordering::SeqCst);
        hooks.write_gate.as_ref().unwrap().add_permits(100);
        eventually("sales goes on from its own checkpoint", || async {
            last_block(&db.pool, INDEXER_SALES).await == target
                && published(&db.pool, INDEXER_SALES, ChainNet::Matic).await
        })
        .await;
        assert_eq!(
            sale_events(&db.pool).await,
            sales_before + 1,
            "the window's sale, once"
        );
        assert!(first.shutdown(Duration::from_secs(5)).await.is_empty());
        assert!(second.shutdown(Duration::from_secs(5)).await.is_empty());
        db.drop().await;
    }

    /// A member stuck in its own filters leaves the shared walk at the
    /// deadline, and the others go on: mana reaches two new heads while
    /// trade events is stuck, and trade events catches up once released.
    #[tokio::test]
    async fn a_member_stuck_in_its_filters_holds_back_no_other() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let chain = FakeChain::new(100, vec![], 1000, 1000);
        chain.finalized_lag.store(2, Ordering::SeqCst);
        let url = serve(chain.clone()).await;
        let st = state_over(db.pool.clone(), &url, walk_settings(), false);
        let hooks = Hooked {
            groups_gate: Some(Arc::new(tokio::sync::Semaphore::new(0))),
            ..Hooked::default()
        };
        let stuck: MakeIndex = {
            let (armed, gate) = (hooks.armed.clone(), hooks.groups_gate.clone());
            Arc::new(move || {
                Arc::new(Hooked {
                    inner: Some(Arc::new(TradeEventsIndex {
                        net: ChainNet::Matic,
                        chain_id: 137,
                    })),
                    armed: armed.clone(),
                    groups_gate: gate.clone(),
                    ..Hooked::default()
                }) as Arc<dyn ChainIndex>
            })
        };
        let mut sup = supervisor(
            &st,
            ChainNet::Matic,
            vec![
                projection(
                    "mana-transfers-index",
                    ChainNet::Matic,
                    &url,
                    mana(ChainNet::Matic),
                ),
                projection("trade-events-index", ChainNet::Matic, &url, stuck),
            ],
        );
        sup.member_deadline = Duration::from_millis(300);
        let jobs = Jobs::new().with_first_pass_now(true);
        chain_jobs::spawn(&jobs, sup);
        for indexer in [chain_walk::INDEXER_MANA, INDEXER_TRADE_EVENTS] {
            eventually("each walked alone to the head", || async {
                published(&db.pool, indexer, ChainNet::Matic).await
            })
            .await;
        }
        hooks.armed.store(true, Ordering::SeqCst);
        for head in [1_100u64, 2_100] {
            *chain.head.lock() = head;
            eventually("mana reaches the new head", || async {
                last_block(&db.pool, chain_walk::INDEXER_MANA).await == (head - 2) as i64
            })
            .await;
        }
        assert_eq!(
            last_block(&db.pool, INDEXER_TRADE_EVENTS).await,
            98,
            "trade events is stuck"
        );
        hooks.armed.store(false, Ordering::SeqCst);
        hooks.groups_gate.as_ref().unwrap().add_permits(100);
        eventually("trade events catches up", || async {
            last_block(&db.pool, INDEXER_TRADE_EVENTS).await == 2_098
        })
        .await;
        assert!(jobs.shutdown(Duration::from_secs(5)).await.is_empty());
        db.drop().await;
    }

    /// A commit whose return overruns the member deadline leaves its outcome
    /// unknown to the walk: the member leaves with a fresh index and reads its
    /// cursor again. Whether the commit was lost (held before it, the cursor
    /// at the window's start) or landed (its return held after it, the cursor
    /// at the window's end), the next attempt converges to the same sale,
    /// cursor and publication as a clean walk: the window's sale exactly once.
    #[tokio::test]
    async fn a_commit_past_the_deadline_recovers_from_wherever_the_cursor_is() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let s = nft_index::start_block(ChainNet::Matic);
        let c1 = "0x00000000000000000000000000000000000000c1";
        seed_collection(
            &db.pool,
            "matic",
            c1,
            s + 10,
            "urn:decentraland:matic:collections-v2:0xc1",
            "Pirates",
        )
        .await;
        let chain = FakeChain::new(s + 500, vec![], 1_000_000, 100);
        chain.finalized_lag.store(2, Ordering::SeqCst);
        add_mint(&chain, c1, v2_token(0, 1), s + 100);
        let url = serve(chain.clone()).await;
        let st = state_over(db.pool.clone(), &url, index_settings(100, 1_000_000), false);
        let hooks = Hooked {
            write_gate: Some(Arc::new(tokio::sync::Semaphore::new(0))),
            committed_gate: Some(Arc::new(tokio::sync::Semaphore::new(0))),
            ..Hooked::default()
        };
        let mut projections = nft_and_sales(&url);
        projections[1].make = {
            let (armed, commit_armed, write_gate, committed_gate, entered) = (
                hooks.armed.clone(),
                hooks.commit_armed.clone(),
                hooks.write_gate.clone(),
                hooks.committed_gate.clone(),
                hooks.write_entered.clone(),
            );
            Arc::new(move || {
                Arc::new(Hooked {
                    inner: Some(Arc::new(SalesIndex)),
                    armed: armed.clone(),
                    commit_armed: commit_armed.clone(),
                    write_gate: write_gate.clone(),
                    committed_gate: committed_gate.clone(),
                    write_entered: entered.clone(),
                    ..Hooked::default()
                }) as Arc<dyn ChainIndex>
            })
        };
        let mut sup = supervisor(&st, ChainNet::Matic, projections);
        sup.member_deadline = Duration::from_millis(300);
        let stats = sup.stats.clone();
        let jobs = Jobs::new().with_first_pass_now(true);
        chain_jobs::spawn(&jobs, sup);
        for indexer in [INDEXER_NFT, INDEXER_SALES] {
            eventually("each walked alone to the head", || async {
                published(&db.pool, indexer, ChainNet::Matic).await
            })
            .await;
        }
        for (round, landed) in [(1u64, false), (2, true)] {
            let start = last_block(&db.pool, INDEXER_SALES).await;
            let sales = sale_events(&db.pool).await;
            let entered = hooks.write_entered.load(Ordering::SeqCst);
            let cut_short = stats.cut_short.load(Ordering::SeqCst);
            let flag = if landed {
                &hooks.commit_armed
            } else {
                &hooks.armed
            };
            flag.store(true, Ordering::SeqCst);
            add_sale(
                &chain,
                c1,
                v2_token(0, round + 1),
                s + 500 + round * 1_000 - 300,
            );
            *chain.head.lock() = s + 500 + round * 1_000;
            let target = (s + 498 + round * 1_000) as i64;
            eventually("sales is held in its commit", || async {
                hooks.write_entered.load(Ordering::SeqCst) > entered
                    && last_block(&db.pool, INDEXER_NFT).await == target
            })
            .await;
            eventually("the supervisor took sales' commit as cut short", || async {
                stats.cut_short.load(Ordering::SeqCst) > cut_short
            })
            .await;
            if landed {
                assert_eq!(
                    last_block(&db.pool, INDEXER_SALES).await,
                    target,
                    "the commit landed"
                );
                assert_eq!(sale_events(&db.pool).await, sales + 1);
            } else {
                assert_eq!(
                    last_block(&db.pool, INDEXER_SALES).await,
                    start,
                    "the commit was lost"
                );
                assert_eq!(sale_events(&db.pool).await, sales);
            }
            flag.store(false, Ordering::SeqCst);
            let gate = if landed {
                &hooks.committed_gate
            } else {
                &hooks.write_gate
            };
            gate.as_ref().unwrap().add_permits(100);
            eventually("sales converges at the target, published", || async {
                last_block(&db.pool, INDEXER_SALES).await == target
                    && published(&db.pool, INDEXER_SALES, ChainNet::Matic).await
            })
            .await;
            assert_eq!(
                sale_events(&db.pool).await,
                sales + 1,
                "the window's sale, once"
            );
            let gate = gate.as_ref().unwrap();
            gate.forget_permits(gate.available_permits());
        }
        assert!(jobs.shutdown(Duration::from_secs(5)).await.is_empty());
        db.drop().await;
    }

    /// A projection whose cursor moved under the checkpoint the supervisor
    /// kept leaves the next shared walk once, from where it really stands,
    /// and both advance without regrouping over and over.
    #[tokio::test]
    async fn a_checkpoint_moved_underneath_regroups_once_and_both_advance() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let chain = FakeChain::new(100, vec![], 1000, 1000);
        chain.finalized_lag.store(2, Ordering::SeqCst);
        let url = serve(chain.clone()).await;
        let st = state_over(db.pool.clone(), &url, walk_settings(), false);
        let every = |p: Projection| Projection {
            cfg: PeriodicCfg::every(Duration::from_millis(200)),
            ..p
        };
        let trade_events: MakeIndex = Arc::new(|| {
            Arc::new(TradeEventsIndex {
                net: ChainNet::Matic,
                chain_id: 137,
            })
        });
        let jobs = Jobs::new().with_first_pass_now(true);
        chain_jobs::spawn(
            &jobs,
            supervisor(
                &st,
                ChainNet::Matic,
                vec![
                    every(projection(
                        "mana-transfers-index",
                        ChainNet::Matic,
                        &url,
                        mana(ChainNet::Matic),
                    )),
                    every(projection(
                        "trade-events-index",
                        ChainNet::Matic,
                        &url,
                        trade_events,
                    )),
                ],
            ),
        );
        for indexer in [chain_walk::INDEXER_MANA, INDEXER_TRADE_EVENTS] {
            eventually("each walked alone to the head", || async {
                published(&db.pool, indexer, ChainNet::Matic).await
            })
            .await;
        }
        // Trade events' cursor goes back to block 50, a checkpoint the chain
        // still proves, under the supervisor's cached 98.
        sqlx::query(
            "UPDATE shop.chain_cursor SET last_block = 50, last_block_hash = $1, last_block_time = NULL, \
             fold_through = NULL, folded_block = NULL, folded_at = NULL \
             WHERE indexer = 'trade_events' AND network = 'matic'",
        )
        .bind(chain.hash_of(50))
        .execute(&db.pool)
        .await
        .unwrap();
        *chain.head.lock() = 200;
        let moved = std::time::Instant::now();
        for indexer in [chain_walk::INDEXER_MANA, INDEXER_TRADE_EVENTS] {
            eventually("both reach the new head", || async {
                last_block(&db.pool, indexer).await == 198
            })
            .await;
        }
        assert!(
            moved.elapsed() < Duration::from_secs(5),
            "within a few dispatches: {:?}",
            moved.elapsed()
        );
        let before = chain.requests.load(Ordering::SeqCst);
        tokio::time::sleep(Duration::from_secs(2)).await;
        let spent = chain.requests.load(Ordering::SeqCst) - before;
        assert!(
            spent < 300,
            "no regroup loop: {spent} requests in ten intervals"
        );
        assert!(jobs.shutdown(Duration::from_secs(5)).await.is_empty());
        db.drop().await;
    }

    /// A collection the mirror adds between two shared walks is taken in by
    /// the next one: the same rows as the walks alone.
    #[tokio::test]
    async fn a_collection_added_between_shared_walks_is_taken_in() {
        let (Some(alone), Some(together)) = (ScratchDb::create().await, ScratchDb::create().await)
        else {
            return;
        };
        let s = nft_index::start_block(ChainNet::Matic);
        let (c1, c2) = (
            "0x00000000000000000000000000000000000000c1",
            "0x00000000000000000000000000000000000000c2",
        );
        for db in [&alone, &together] {
            seed_collection(
                &db.pool,
                "matic",
                c1,
                s + 10,
                "urn:decentraland:matic:collections-v2:0xc1",
                "Pirates",
            )
            .await;
        }
        let chain = FakeChain::new(s + 500, vec![], 1_000_000, 100);
        chain.finalized_lag.store(2, Ordering::SeqCst);
        add_mint(&chain, c1, v2_token(0, 1), s + 100);
        let url = serve(chain.clone()).await;
        let settings = index_settings(100, 1_000_000);
        let (st_alone, st_together) = (
            state_over(alone.pool.clone(), &url, settings.clone(), false),
            state_over(together.pool.clone(), &url, settings.clone(), false),
        );
        let shared: Vec<(Arc<dyn ChainIndex>, Arc<chain_walk::Holdings>)> = vec![
            (Arc::new(NftIndex::new(ChainNet::Matic)), Arc::default()),
            (Arc::new(SalesIndex), Arc::default()),
        ];
        let solo_nft = NftIndex::new(ChainNet::Matic);
        for st in [&st_alone, &st_together] {
            assert!(publish(st, &url, &solo_nft).await.published);
            assert!(publish(st, &url, &SalesIndex).await.published);
        }
        let walk_together = |head: u64| {
            let (pool, http, settings, url, shared) = (
                together.pool.clone(),
                st_together.http.clone(),
                settings.clone(),
                url.clone(),
                shared.clone(),
            );
            async move {
                let at = chain_group::Checkpoint::of(
                    &chain_walk::cursor(&pool, INDEXER_NFT, ChainNet::Matic)
                        .await
                        .unwrap()
                        .unwrap(),
                )
                .unwrap();
                let members = shared
                    .iter()
                    .enumerate()
                    .map(|(slot, (index, holdings))| chain_group::Member {
                        slot,
                        index: index.clone(),
                        holdings: holdings.clone(),
                        lease: None,
                        gate: None,
                    })
                    .collect();
                let ends = chain_group::walk_together(
                    &pool,
                    &http,
                    &url,
                    &settings,
                    members,
                    &at,
                    Duration::from_secs(60),
                    &AtomicUsize::new(0),
                )
                .await;
                for (slot, end) in ends {
                    let chain_group::MemberEnd::Walked {
                        mut out,
                        close: Some(since),
                        ..
                    } = end
                    else {
                        panic!("member {slot} did not reach {head}");
                    };
                    let (index, holdings) = &shared[slot];
                    let owner = holdings
                        .owner(&pool, None, index.as_ref(), &settings)
                        .await
                        .unwrap();
                    let source = chain_walk::RpcSource::new(Rpc::at(&http, &url));
                    chain_walk::close(
                        &pool,
                        &source,
                        index.as_ref(),
                        &settings,
                        &owner,
                        since,
                        &mut out,
                    )
                    .await
                    .unwrap();
                }
            }
        };
        // A first shared walk, then the mirror adds c2 with a mint in the
        // next segment, then a second shared walk. The walks alone cover the
        // same windows: a pass at each head.
        add_mint(&chain, c1, v2_token(0, 2), s + 700);
        *chain.head.lock() = s + 1_500;
        walk_together(s + 1_500).await;
        pass(&st_alone, &url, &solo_nft).await;
        pass(&st_alone, &url, &SalesIndex).await;
        for db in [&alone, &together] {
            seed_collection(
                &db.pool,
                "matic",
                c2,
                s + 1_600,
                "urn:decentraland:matic:collections-v2:0xc2",
                "Dances",
            )
            .await;
        }
        add_mint(&chain, c2, v2_token(0, 1), s + 1_700);
        *chain.head.lock() = s + 2_500;
        walk_together(s + 2_500).await;
        pass(&st_alone, &url, &solo_nft).await;
        pass(&st_alone, &url, &SalesIndex).await;
        for table in PROJECTION_TABLES {
            assert_eq!(
                table_rows(&together.pool, table).await,
                table_rows(&alone.pool, table).await,
                "{table}"
            );
        }
        let c2_events: i64 =
            sqlx::query_scalar("SELECT count(*) FROM shop.nft_event WHERE contract_address = $1")
                .bind(c2)
                .fetch_one(&together.pool)
                .await
                .unwrap();
        assert!(c2_events > 0, "the second shared walk read c2");
        alone.drop().await;
        together.drop().await;
    }

    /// While nft fails, sales walks on and folds its bid with the seller
    /// pending; once nft recovers, a later sales fold fills the seller with
    /// no new sales event.
    #[tokio::test]
    async fn sales_fills_a_seller_once_nft_recovers() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let s = nft_index::start_block(ChainNet::Matic);
        let b = sales_index::BID_V2_MATIC_START_BLOCK + 100;
        let c1 = "0x00000000000000000000000000000000000000c1";
        seed_collection(
            &db.pool,
            "matic",
            c1,
            s + 10,
            "urn:decentraland:matic:collections-v2:0xc1",
            "Pirates",
        )
        .await;
        let chain = FakeChain::new(s + 500, vec![], 1_000_000, 100);
        chain.finalized_lag.store(2, Ordering::SeqCst);
        add_mint(&chain, c1, v2_token(0, 9), s + 100);
        let url = serve(chain.clone()).await;
        let st = state_over(db.pool.clone(), &url, index_settings(100, 1_000_000), false);
        let fails = Arc::new(AtomicUsize::new(0));
        let mut projections = nft_and_sales(&url);
        projections[0].make = {
            let fails = fails.clone();
            Arc::new(move || {
                Arc::new(Hooked {
                    inner: Some(Arc::new(NftIndex::new(ChainNet::Matic))),
                    write_fails: fails.clone(),
                    ..Hooked::default()
                }) as Arc<dyn ChainIndex>
            })
        };
        let jobs = Jobs::new().with_first_pass_now(true);
        chain_jobs::spawn(&jobs, supervisor(&st, ChainNet::Matic, projections));
        for indexer in [INDEXER_NFT, INDEXER_SALES] {
            eventually("each walked alone to the head", || async {
                published(&db.pool, indexer, ChainNet::Matic).await
            })
            .await;
        }
        // ALICE gets t1; BOB bids on it. nft fails its writes meanwhile.
        fails.store(usize::MAX, Ordering::SeqCst);
        let t1 = v2_token(0, 1);
        {
            let mut logs = chain.logs.lock();
            for l in [
                sales_index::tests::issue_log(c1, ALICE, t1, 0, CARL, b - 10, 0),
                transfer(c1, ZERO, ALICE, t1, b - 10, 1),
                sales_index::tests::bid_created_log(c1, t1, BOB, 0x11, 7, 1_700_000_000, b, 0),
            ] {
                logs.push(log_json(&l));
            }
        }
        *chain.head.lock() = b + 1_000;
        eventually("sales folds the bid", || async {
            bids_of(&db.pool).await.len() == 1
        })
        .await;
        assert_eq!(
            bids_of(&db.pool).await[0].3,
            None,
            "the seller waits for nft"
        );
        let events = sale_events(&db.pool).await;
        fails.store(0, Ordering::SeqCst);
        eventually("a later sales fold fills the seller", || async {
            bids_of(&db.pool).await[0].3.as_deref() == Some(ALICE)
        })
        .await;
        assert_eq!(sale_events(&db.pool).await, events, "no new sales event");
        assert!(jobs.shutdown(Duration::from_secs(5)).await.is_empty());
        db.drop().await;
    }

    /// A slow close holds back only its own projection: nft walks two more
    /// heads while sales' fold after a shared walk is held.
    #[tokio::test]
    async fn a_slow_close_holds_back_only_its_projection() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let s = nft_index::start_block(ChainNet::Matic);
        let c1 = "0x00000000000000000000000000000000000000c1";
        seed_collection(
            &db.pool,
            "matic",
            c1,
            s + 10,
            "urn:decentraland:matic:collections-v2:0xc1",
            "Pirates",
        )
        .await;
        let chain = FakeChain::new(s + 500, vec![], 1_000_000, 100);
        chain.finalized_lag.store(2, Ordering::SeqCst);
        add_mint(&chain, c1, v2_token(0, 1), s + 100);
        let url = serve(chain.clone()).await;
        let st = state_over(db.pool.clone(), &url, index_settings(100, 1_000_000), false);
        let hooks = Hooked {
            armed_fold_gate: Some(Arc::new(tokio::sync::Semaphore::new(0))),
            ..Hooked::default()
        };
        let mut projections = nft_and_sales(&url);
        projections[1].make = {
            let (armed, gate, entered) = (
                hooks.armed.clone(),
                hooks.armed_fold_gate.clone(),
                hooks.entered.clone(),
            );
            Arc::new(move || {
                Arc::new(Hooked {
                    inner: Some(Arc::new(SalesIndex)),
                    armed: armed.clone(),
                    armed_fold_gate: gate.clone(),
                    entered: entered.clone(),
                    ..Hooked::default()
                }) as Arc<dyn ChainIndex>
            })
        };
        let jobs = Jobs::new().with_first_pass_now(true);
        chain_jobs::spawn(&jobs, supervisor(&st, ChainNet::Matic, projections));
        for indexer in [INDEXER_NFT, INDEXER_SALES] {
            eventually("each walked alone to the head", || async {
                published(&db.pool, indexer, ChainNet::Matic).await
            })
            .await;
        }
        hooks.armed.store(true, Ordering::SeqCst);
        *chain.head.lock() = s + 1_500;
        eventually("sales' close is held", || async {
            hooks.entered.load(Ordering::SeqCst) >= 1
        })
        .await;
        let held = cursor(&db.pool, INDEXER_SALES, ChainNet::Matic)
            .await
            .unwrap()
            .folded_block;
        for head in [s + 2_500, s + 3_500] {
            *chain.head.lock() = head;
            eventually("nft walks on", || async {
                last_block(&db.pool, INDEXER_NFT).await == (head - 2) as i64
                    && published(&db.pool, INDEXER_NFT, ChainNet::Matic).await
            })
            .await;
        }
        assert_eq!(
            cursor(&db.pool, INDEXER_SALES, ChainNet::Matic)
                .await
                .unwrap()
                .folded_block,
            held,
            "sales stays in its close"
        );
        hooks.armed.store(false, Ordering::SeqCst);
        hooks.armed_fold_gate.as_ref().unwrap().add_permits(100);
        eventually("sales closes and catches up", || async {
            last_block(&db.pool, INDEXER_SALES).await == (s + 3_498) as i64
                && published(&db.pool, INDEXER_SALES, ChainNet::Matic).await
        })
        .await;
        assert!(jobs.shutdown(Duration::from_secs(5)).await.is_empty());
        db.drop().await;
    }

    /// An nft walk that owes a backfill walks alone, with its lanes, while
    /// sales goes on.
    #[tokio::test]
    async fn an_nft_walk_owing_a_backfill_walks_alone_with_its_lanes() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let s = nft_index::start_block(ChainNet::Matic);
        let (c1, late) = (
            "0x00000000000000000000000000000000000000c1",
            "0x00000000000000000000000000000000000000c9",
        );
        seed_collection(
            &db.pool,
            "matic",
            c1,
            s + 10,
            "urn:decentraland:matic:collections-v2:0xc1",
            "Pirates",
        )
        .await;
        let chain = FakeChain::new(s + 500, vec![], 1_000_000, 100);
        chain.finalized_lag.store(2, Ordering::SeqCst);
        add_mint(&chain, c1, v2_token(0, 1), s + 100);
        let url = serve(chain.clone()).await;
        let st = state_over(db.pool.clone(), &url, index_settings(100, 1_000_000), false);
        let jobs = Jobs::new().with_first_pass_now(true);
        chain_jobs::spawn(&jobs, supervisor(&st, ChainNet::Matic, nft_and_sales(&url)));
        for indexer in [INDEXER_NFT, INDEXER_SALES] {
            eventually("each walked alone to the head", || async {
                published(&db.pool, indexer, ChainNet::Matic).await
            })
            .await;
        }
        // A contract the nft walk took in late owes its history before.
        sqlx::query(
            "INSERT INTO shop.nft_contract (network, contract_address, walked_from_block, backfill_from_block) \
             VALUES ('matic', $1, $2, $3)",
        )
        .bind(late)
        .bind((s + 400) as i64)
        .bind((s + 10) as i64)
        .execute(&db.pool)
        .await
        .unwrap();
        *chain.head.lock() = s + 1_500;
        eventually("the backfill ran", || async {
            sqlx::query_scalar::<_, Option<i64>>(
                "SELECT backfilled_to_block FROM shop.nft_contract WHERE contract_address = $1",
            )
            .bind(late)
            .fetch_one(&db.pool)
            .await
            .unwrap()
                == Some((s + 399) as i64)
        })
        .await;
        for indexer in [INDEXER_NFT, INDEXER_SALES] {
            eventually("both reach the head", || async {
                last_block(&db.pool, indexer).await == (s + 1_498) as i64
            })
            .await;
        }
        assert!(jobs.shutdown(Duration::from_secs(5)).await.is_empty());
        db.drop().await;
    }

    /// Two windows committed, then the third window's second request
    /// refused: each member stays at the second window and falls back with
    /// what its own filters left it, the walk's count is every request the
    /// chain saw (the refused one too), and a fallback with nothing left
    /// reads nothing, while one with something left walks on alone.
    #[tokio::test]
    async fn a_late_shared_read_failure_keeps_each_budget_and_the_count() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let (a1, a2, b1) = (
            "0x00000000000000000000000000000000000000a1",
            "0x00000000000000000000000000000000000000a2",
            "0x00000000000000000000000000000000000000b1",
        );
        let chain = FakeChain::new(52, vec![], 1_000, 1_000);
        chain.finalized_lag.store(2, Ordering::SeqCst);
        let url = serve(chain.clone()).await;
        let settings = IndexSettings {
            address_chunk: 1,
            window_blocks: 10,
            requests_per_pass: 6,
            ..walk_settings()
        };
        let st = state_over(db.pool.clone(), &url, settings.clone(), false);
        // A reads two addresses (two requests a window at chunk 1), B one.
        let a: Arc<dyn ChainIndex> = Arc::new(Hooked {
            inner: Some(Arc::new(super::super::mana_transfers::ManaTransfersIndex {
                net: ChainNet::Matic,
            })),
            groups_override: Some(vec![chain_walk::LogGroup {
                addresses: vec![a1.into(), a2.into()],
                topics: json!([[super::super::mana_transfers::TRANSFER]]),
            }]),
            ..Hooked::default()
        });
        let b: Arc<dyn ChainIndex> = Arc::new(Hooked {
            inner: Some(Arc::new(TradeEventsIndex {
                net: ChainNet::Matic,
                chain_id: 137,
            })),
            groups_override: Some(vec![chain_walk::LogGroup {
                addresses: vec![b1.into()],
                topics: json!([[
                    "0x00000000000000000000000000000000000000000000000000000000000000b1"
                ]]),
            }]),
            ..Hooked::default()
        });
        for ix in [&a, &b] {
            assert!(publish(&st, &url, ix.as_ref()).await.published);
        }
        assert_eq!(last_block(&db.pool, chain_walk::INDEXER_MANA).await, 50);
        *chain.head.lock() = 100;
        *chain.refuse_from.lock() = Some((71, a2.into()));
        let at = chain_group::Checkpoint::of(
            &chain_walk::cursor(&db.pool, chain_walk::INDEXER_MANA, ChainNet::Matic)
                .await
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        let holdings: Vec<Arc<chain_walk::Holdings>> = vec![Arc::default(), Arc::default()];
        let members = [&a, &b]
            .into_iter()
            .enumerate()
            .map(|(slot, index)| chain_group::Member {
                slot,
                index: index.clone(),
                holdings: holdings[slot].clone(),
                lease: None,
                gate: None,
            })
            .collect();
        let seen = chain.requests.load(Ordering::SeqCst);
        let counted = AtomicUsize::new(0);
        let ends = chain_group::walk_together(
            &db.pool,
            &st.http,
            &url,
            &settings,
            members,
            &at,
            Duration::from_secs(60),
            &counted,
        )
        .await;
        assert_eq!(
            counted.load(Ordering::SeqCst),
            chain.requests.load(Ordering::SeqCst) - seen,
            "the walk counts every request the chain saw"
        );
        // The client tries a refused read again a few times; the count above
        // holds every attempt.
        assert!(chain.refused.load(Ordering::SeqCst) >= 1);
        let budgets: Vec<_> = ends
            .iter()
            .map(|(slot, end)| match end {
                chain_group::MemberEnd::Fallback { budget } => (*slot, *budget),
                _ => panic!("member {slot} did not fall back"),
            })
            .collect();
        assert_eq!(
            budgets,
            vec![
                (
                    0,
                    chain_walk::Budget {
                        remaining: 0,
                        first_window_used: true
                    }
                ),
                (
                    1,
                    chain_walk::Budget {
                        remaining: 3,
                        first_window_used: true
                    }
                ),
            ],
            "three windows paid at two and one requests each"
        );
        for indexer in [chain_walk::INDEXER_MANA, INDEXER_TRADE_EVENTS] {
            assert_eq!(
                last_block(&db.pool, indexer).await,
                70,
                "the second window, committed"
            );
        }
        // A falls back with nothing left: it reads no logs at all.
        let source = chain_walk::RpcSource::new(Rpc::at(&st.http, &url));
        let logs = chain.get_logs.load(Ordering::SeqCst);
        let out = chain_walk::job_pass_with(
            &db.pool,
            &source,
            a.as_ref(),
            &settings,
            &holdings[0],
            None,
            chain_walk::Budget {
                remaining: 0,
                first_window_used: true,
            },
            None,
            &|_| {},
        )
        .await
        .unwrap();
        assert!(out.paused, "{out:?}");
        assert_eq!(
            chain.get_logs.load(Ordering::SeqCst),
            logs,
            "no eth_getLogs on an empty budget"
        );
        assert_eq!(last_block(&db.pool, chain_walk::INDEXER_MANA).await, 70);
        // Nothing left is nothing, even for a window that costs one request.
        let out = chain_walk::job_pass_with(
            &db.pool,
            &source,
            b.as_ref(),
            &settings,
            &holdings[1],
            None,
            chain_walk::Budget {
                remaining: 0,
                first_window_used: true,
            },
            None,
            &|_| {},
        )
        .await
        .unwrap();
        assert!(out.paused, "{out:?}");
        assert_eq!(
            chain.get_logs.load(Ordering::SeqCst),
            logs,
            "no eth_getLogs on an empty budget"
        );
        // B falls back with three: its own reads are not refused.
        let out = chain_walk::job_pass_with(
            &db.pool,
            &source,
            b.as_ref(),
            &settings,
            &holdings[1],
            None,
            chain_walk::Budget {
                remaining: 3,
                first_window_used: true,
            },
            None,
            &|_| {},
        )
        .await
        .unwrap();
        assert_eq!(out.indexed_to, 98, "{out:?}");
        db.drop().await;
    }

    /// The members' own steps run side by side: two gates held up cost the
    /// healthy member one deadline (2 s), not two, and a gate that panics as
    /// it is called fails its member alone.
    #[tokio::test]
    async fn held_and_panicking_gates_cost_the_others_one_deadline() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let chain = FakeChain::new(100, vec![], 1000, 1000);
        chain.finalized_lag.store(2, Ordering::SeqCst);
        let url = serve(chain.clone()).await;
        let st = state_over(db.pool.clone(), &url, walk_settings(), false);
        let armed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let held: chain_jobs::Gate = {
            let armed = armed.clone();
            Arc::new(move || {
                let armed = armed.clone();
                Box::pin(async move {
                    if armed.load(Ordering::SeqCst) {
                        std::future::pending::<()>().await;
                    }
                    Ok(None)
                })
            })
        };
        let panicking: chain_jobs::Gate = {
            let armed = armed.clone();
            Arc::new(move || {
                assert!(
                    !armed.load(Ordering::SeqCst),
                    "a gate panicked as it was called"
                );
                Box::pin(async { Ok(None) })
            })
        };
        let with_gate = |p: Projection, gate: &chain_jobs::Gate| Projection {
            gate: Some(gate.clone()),
            ..p
        };
        let mut sup = supervisor(
            &st,
            ChainNet::Matic,
            vec![
                projection(
                    "mana-transfers-index",
                    ChainNet::Matic,
                    &url,
                    mana(ChainNet::Matic),
                ),
                with_gate(
                    projection(
                        "trade-events-index",
                        ChainNet::Matic,
                        &url,
                        Arc::new(|| {
                            Arc::new(TradeEventsIndex {
                                net: ChainNet::Matic,
                                chain_id: 137,
                            })
                        }),
                    ),
                    &held,
                ),
                with_gate(
                    projection(
                        "committee-index",
                        ChainNet::Matic,
                        &url,
                        Arc::new(|| Arc::new(super::super::committee_index::CommitteeIndex)),
                    ),
                    &held,
                ),
                with_gate(
                    projection(
                        "sales-index",
                        ChainNet::Matic,
                        &url,
                        Arc::new(|| Arc::new(SalesIndex)),
                    ),
                    &panicking,
                ),
            ],
        );
        sup.member_deadline = Duration::from_secs(2);
        let jobs = Jobs::new().with_first_pass_now(true);
        chain_jobs::spawn(&jobs, sup);
        for indexer in [
            chain_walk::INDEXER_MANA,
            INDEXER_TRADE_EVENTS,
            chain_walk::INDEXER_COMMITTEE,
            INDEXER_SALES,
        ] {
            eventually("each walked alone to the head", || async {
                published(&db.pool, indexer, ChainNet::Matic).await
            })
            .await;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
        armed.store(true, Ordering::SeqCst);
        *chain.head.lock() = 1_100;
        let moved = std::time::Instant::now();
        eventually("mana reaches the new head", || async {
            last_block(&db.pool, chain_walk::INDEXER_MANA).await == 1_098
        })
        .await;
        assert!(
            moved.elapsed() < Duration::from_millis(3_500),
            "one deadline for both held gates: {:?}",
            moved.elapsed()
        );
        let report = jobs.report();
        let job = report
            .iter()
            .find(|j| j.name == "chain-walk-matic")
            .unwrap();
        assert!(!matches!(job.state, JobState::Finished | JobState::Failed));
        armed.store(false, Ordering::SeqCst);
        assert!(jobs.shutdown(Duration::from_secs(5)).await.is_empty());
        db.drop().await;
    }

    /// A lease request held back holds back no other projection: mana
    /// publishes twice more, from its own chain, while trade events still
    /// waits for its lease, and walks once it has it.
    #[tokio::test]
    async fn a_slow_lease_request_holds_back_no_other_projection() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let mana_chain = FakeChain::new(100, vec![], 1000, 1000);
        mana_chain.finalized_lag.store(2, Ordering::SeqCst);
        let mana_url = serve(mana_chain.clone()).await;
        let trades_chain = FakeChain::new(100, vec![], 1000, 1000);
        trades_chain.finalized_lag.store(2, Ordering::SeqCst);
        let trades_url = serve(trades_chain.clone()).await;
        let st = state_over(db.pool.clone(), &mana_url, walk_settings(), false);
        let namespace = format!("supervisor-slow-lease-{}", std::process::id());
        let jobs = Jobs::with_leases(
            db.pool.clone(),
            lease_settings(&namespace, Duration::from_millis(100)),
        )
        .with_first_pass_now(true);
        let (leases, gate, asked) = gated(&jobs, &["trade-events-index-matic"]);
        let trade_events: MakeIndex = Arc::new(|| {
            Arc::new(TradeEventsIndex {
                net: ChainNet::Matic,
                chain_id: 137,
            })
        });
        chain_jobs::spawn_with(
            &jobs,
            supervisor(
                &st,
                ChainNet::Matic,
                vec![
                    projection(
                        "mana-transfers-index",
                        ChainNet::Matic,
                        &mana_url,
                        mana(ChainNet::Matic),
                    ),
                    projection(
                        "trade-events-index",
                        ChainNet::Matic,
                        &trades_url,
                        trade_events,
                    ),
                ],
            ),
            Some(leases),
        );
        eventually("mana published", || {
            published(&db.pool, chain_walk::INDEXER_MANA, ChainNet::Matic)
        })
        .await;
        let folded_at = || async {
            cursor(&db.pool, chain_walk::INDEXER_MANA, ChainNet::Matic)
                .await
                .and_then(|c| c.folded_at)
        };
        let first = folded_at().await;
        eventually("a second mana pass", || async { folded_at().await > first }).await;
        let second = folded_at().await;
        eventually("a third mana pass", || async { folded_at().await > second }).await;
        assert_eq!(
            trades_chain.requests.load(Ordering::SeqCst),
            0,
            "trade events still waits for its lease"
        );
        assert_eq!(
            asked
                .lock()
                .iter()
                .filter(|n| *n == "trade-events-index-matic")
                .count(),
            1,
            "one request, still in flight"
        );
        gate.add_permits(1);
        eventually("trade events walks once it has its lease", || {
            published(&db.pool, chain_walk::INDEXER_TRADE_EVENTS, ChainNet::Matic)
        })
        .await;
        assert!(jobs.shutdown(Duration::from_secs(5)).await.is_empty());
        db.drop().await;
    }

    /// Stopping lets a lease request in flight run to its end, lets go of
    /// the lease it took, and asks for no lease again: committee, whose lease
    /// another process holds, stops asking once the stop starts. A
    /// supervisor that holds no lease reports standby.
    #[tokio::test]
    async fn stopping_lets_a_lease_request_finish_and_asks_for_no_other() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let chain = FakeChain::new(100, vec![], 1000, 1000);
        chain.finalized_lag.store(2, Ordering::SeqCst);
        let url = serve(chain.clone()).await;
        let st = state_over(db.pool.clone(), &url, walk_settings(), false);
        let namespace = format!("supervisor-stop-{}", std::process::id());
        let elsewhere = Arc::new(crate::jobs::Leases::new(
            db.pool.clone(),
            lease_settings(&namespace, Duration::from_millis(100)),
        ));
        let committee_lease = elsewhere
            .try_acquire("committee-index-matic")
            .await
            .unwrap()
            .expect("another process holds committee's lease");
        let jobs = Arc::new(Jobs::with_leases(
            db.pool.clone(),
            lease_settings(&namespace, Duration::from_millis(100)),
        ));
        let (leases, gate, asked) = gated(&jobs, &["mana-transfers-index-matic"]);
        chain_jobs::spawn_with(
            &jobs,
            supervisor(
                &st,
                ChainNet::Matic,
                vec![
                    projection(
                        "mana-transfers-index",
                        ChainNet::Matic,
                        &url,
                        mana(ChainNet::Matic),
                    ),
                    projection(
                        "committee-index",
                        ChainNet::Matic,
                        &url,
                        Arc::new(|| Arc::new(super::super::committee_index::CommitteeIndex)),
                    ),
                ],
            ),
            Some(leases),
        );
        let committee_asks = || {
            asked
                .lock()
                .iter()
                .filter(|n| *n == "committee-index-matic")
                .count()
        };
        eventually("committee asked again after its standby wait", || async {
            committee_asks() >= 2
        })
        .await;
        let report = jobs.report();
        let job = report
            .iter()
            .find(|j| j.name == "chain-walk-matic")
            .unwrap();
        assert_eq!(job.state, JobState::Standby, "it holds no lease");

        let stopping = tokio::spawn({
            let jobs = jobs.clone();
            async move { jobs.shutdown(Duration::from_secs(10)).await }
        });
        // A request already in flight when the stop starts still counts.
        tokio::time::sleep(Duration::from_millis(300)).await;
        let at_stop = committee_asks();
        tokio::time::sleep(Duration::from_millis(1200)).await;
        assert!(
            !stopping.is_finished(),
            "the request in flight runs to its end"
        );
        assert_eq!(
            committee_asks(),
            at_stop,
            "no lease asked for after the stop"
        );
        gate.add_permits(1);
        assert!(stopping.await.unwrap().is_empty());
        assert_eq!(
            asked
                .lock()
                .iter()
                .filter(|n| *n == "mana-transfers-index-matic")
                .count(),
            1
        );
        assert_eq!(
            chain.requests.load(Ordering::SeqCst),
            0,
            "no attempt after the stop"
        );
        committee_lease.release().await;
        assert_eq!(
            advisory_locks(&db.pool).await,
            0,
            "the lease mana took was let go"
        );
        db.drop().await;
    }

    /// A gate that says wait ends a run of failures, as a pass that ran
    /// does: the next failure backs off from the start, one interval (200
    /// ms), not the fourth doubling (1.6 s).
    #[tokio::test]
    async fn a_waiting_gate_resets_the_backoff() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let amoy = FakeChain::new(100, vec![], 1000, 1000);
        amoy.chain_id.store(80_002, Ordering::SeqCst);
        let url = serve(amoy.clone()).await;
        let st = state_over(db.pool.clone(), &url, walk_settings(), false);
        let calls: Arc<Mutex<Vec<std::time::Instant>>> = Arc::default();
        let gate: chain_jobs::Gate = {
            let calls = calls.clone();
            Arc::new(move || {
                let calls = calls.clone();
                Box::pin(async move {
                    let n = {
                        let mut calls = calls.lock();
                        calls.push(std::time::Instant::now());
                        calls.len()
                    };
                    match n {
                        1..=3 => Err(anyhow::anyhow!("the venue check failed")),
                        4 => Ok(Some("waiting for the mirror".to_string())),
                        // Then the walk fails on another chain.
                        _ => Ok(None),
                    }
                })
            })
        };
        let projection = Projection {
            cfg: PeriodicCfg::every(Duration::from_millis(200))
                .with_error_backoff(Duration::from_secs(10)),
            gate: Some(gate),
            ..projection(
                "mana-transfers-index",
                ChainNet::Matic,
                &url,
                mana(ChainNet::Matic),
            )
        };
        let jobs = Jobs::new().with_first_pass_now(true);
        chain_jobs::spawn(&jobs, supervisor(&st, ChainNet::Matic, vec![projection]));
        eventually("a sixth gate call", || {
            let calls = calls.clone();
            async move { calls.lock().len() >= 6 }
        })
        .await;
        let calls = calls.lock().clone();
        let after_failure = calls[5] - calls[4];
        assert!(
            after_failure < Duration::from_secs(1),
            "one 200 ms interval after the first failure past the wait, not the 1.6 s of a fourth: {after_failure:?}"
        );
        assert!(jobs.shutdown(Duration::from_secs(5)).await.is_empty());
        db.drop().await;
    }

    /// An idle projection whose lease is lost stands by at once, without
    /// waiting out its backoff, and the next acquisition starts with none:
    /// its next attempt comes after the lease poll, not after the 10 s
    /// backoff of the failure before.
    #[tokio::test]
    async fn a_lost_lease_resets_an_idle_projections_backoff() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let amoy = FakeChain::new(100, vec![], 1000, 1000);
        amoy.chain_id.store(80_002, Ordering::SeqCst);
        let url = serve(amoy.clone()).await;
        let st = state_over(db.pool.clone(), &url, walk_settings(), false);
        let namespace = format!("supervisor-backoff-{}", std::process::id());
        let jobs = Jobs::with_leases(
            db.pool.clone(),
            lease_settings(&namespace, Duration::from_millis(100)),
        )
        .with_first_pass_now(true);
        let failing = Projection {
            cfg: PeriodicCfg::every(Duration::from_secs(10))
                .with_error_backoff(Duration::from_secs(60)),
            ..projection(
                "mana-transfers-index",
                ChainNet::Matic,
                &url,
                mana(ChainNet::Matic),
            )
        };
        chain_jobs::spawn(&jobs, supervisor(&st, ChainNet::Matic, vec![failing]));
        eventually("the first attempt failed on another chain", || {
            let amoy = amoy.clone();
            async move { amoy.requests.load(Ordering::SeqCst) == 1 }
        })
        .await;
        sqlx::query(
            "SELECT pg_terminate_backend(pid) FROM pg_locks WHERE locktype = 'advisory' \
             AND database = (SELECT oid FROM pg_database WHERE datname = current_database())",
        )
        .execute(&db.pool)
        .await
        .unwrap();
        let lost_at = std::time::Instant::now();
        eventually("a new acquisition attempts again", || {
            let amoy = amoy.clone();
            async move { amoy.requests.load(Ordering::SeqCst) >= 2 }
        })
        .await;
        assert!(
            lost_at.elapsed() < Duration::from_secs(5),
            "after the lease poll, not the backoff: {:?}",
            lost_at.elapsed()
        );
        assert!(jobs.shutdown(Duration::from_secs(5)).await.is_empty());
        db.drop().await;
    }

    /// Losing the lease interrupts an attempt blocked in its fold, before the
    /// fold commits and with no successor yet; another process then takes
    /// the lease, claims the projection at the same block and hash, and
    /// publishes it, and opening the old fold's gate changes nothing.
    #[tokio::test]
    async fn a_lost_lease_interrupts_a_blocked_fold_and_a_successor_takes_over() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let polygon = FakeChain::new(100, vec![], 1000, 1000);
        polygon.finalized_lag.store(2, Ordering::SeqCst);
        let url = serve(polygon.clone()).await;
        let st = state_over(db.pool.clone(), &url, walk_settings(), false);
        let gate = Arc::new(tokio::sync::Semaphore::new(0));
        let (entered, dropped) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
        let hooked: MakeIndex = {
            let (gate, entered, dropped) = (gate.clone(), entered.clone(), dropped.clone());
            Arc::new(move || {
                Arc::new(Hooked {
                    inner: Some(Arc::new(super::super::mana_transfers::ManaTransfersIndex {
                        net: ChainNet::Matic,
                    })),
                    fold_gate: Some(gate.clone()),
                    entered: entered.clone(),
                    dropped: dropped.clone(),
                    ..Hooked::default()
                }) as Arc<dyn ChainIndex>
            })
        };
        let namespace = format!("supervisor-handover-{}", std::process::id());
        // A stands by long after a loss, so the successor takes the lease.
        let a = Jobs::with_leases(
            db.pool.clone(),
            lease_settings(&namespace, Duration::from_secs(60)),
        )
        .with_first_pass_now(true);
        chain_jobs::spawn(
            &a,
            supervisor(
                &st,
                ChainNet::Matic,
                vec![projection(
                    "mana-transfers-index",
                    ChainNet::Matic,
                    &url,
                    hooked,
                )],
            ),
        );
        eventually("A's fold entered", || {
            let entered = entered.clone();
            async move { entered.load(Ordering::SeqCst) == 1 }
        })
        .await;
        let walked = cursor(&db.pool, chain_walk::INDEXER_MANA, ChainNet::Matic)
            .await
            .unwrap();
        assert_eq!(walked.last_block, 98, "the walk committed before the fold");
        assert_eq!((walked.fold_through, walked.folded_block), (None, None));

        sqlx::query(
            "SELECT pg_terminate_backend(pid) FROM pg_locks WHERE locktype = 'advisory' \
             AND database = (SELECT oid FROM pg_database WHERE datname = current_database())",
        )
        .execute(&db.pool)
        .await
        .unwrap();
        eventually("A's blocked fold dropped", || {
            let dropped = dropped.clone();
            async move { dropped.load(Ordering::SeqCst) == 1 }
        })
        .await;
        let after_loss = cursor(&db.pool, chain_walk::INDEXER_MANA, ChainNet::Matic)
            .await
            .unwrap();
        assert_eq!(
            (
                after_loss.fold_through,
                after_loss.folded_block,
                after_loss.owner_generation
            ),
            (None, None, walked.owner_generation),
            "the interrupted attempt committed nothing more, and no one has claimed yet"
        );

        let b = Jobs::with_leases(
            db.pool.clone(),
            lease_settings(&namespace, Duration::from_millis(100)),
        )
        .with_first_pass_now(true);
        chain_jobs::spawn(
            &b,
            supervisor(
                &st,
                ChainNet::Matic,
                vec![projection(
                    "mana-transfers-index",
                    ChainNet::Matic,
                    &url,
                    mana(ChainNet::Matic),
                )],
            ),
        );
        eventually("B published", || {
            published(&db.pool, chain_walk::INDEXER_MANA, ChainNet::Matic)
        })
        .await;
        let by_b = cursor(&db.pool, chain_walk::INDEXER_MANA, ChainNet::Matic)
            .await
            .unwrap();
        assert_eq!(
            by_b.owner_generation,
            walked.owner_generation + 1,
            "B claimed once"
        );
        assert_eq!(by_b.last_block, 98, "the same block and hash");

        gate.add_permits(10);
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(
            entered.load(Ordering::SeqCst),
            1,
            "A stood by: no new attempt"
        );
        let last = cursor(&db.pool, chain_walk::INDEXER_MANA, ChainNet::Matic)
            .await
            .unwrap();
        assert_eq!(
            last.owner_generation, by_b.owner_generation,
            "A's old attempt is gone"
        );
        assert!(a.shutdown(Duration::from_secs(5)).await.is_empty());
        assert!(b.shutdown(Duration::from_secs(5)).await.is_empty());
        db.drop().await;
    }

    /// Two processes with different projections enabled on one network:
    /// the projection both have is walked by one writer, claimed once, and
    /// the one only the second has walks there.
    #[tokio::test]
    async fn processes_with_different_projections_share_a_network() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let polygon = FakeChain::new(100, vec![], 1000, 1000);
        polygon.finalized_lag.store(2, Ordering::SeqCst);
        let url = serve(polygon.clone()).await;
        let st = state_over(db.pool.clone(), &url, walk_settings(), false);
        let namespace = format!("supervisor-capabilities-{}", std::process::id());
        let trade_events: MakeIndex = Arc::new(|| {
            Arc::new(TradeEventsIndex {
                net: ChainNet::Matic,
                chain_id: 137,
            })
        });
        let first = Jobs::with_leases(
            db.pool.clone(),
            lease_settings(&namespace, Duration::from_millis(100)),
        )
        .with_first_pass_now(true);
        chain_jobs::spawn(
            &first,
            supervisor(
                &st,
                ChainNet::Matic,
                vec![projection(
                    "mana-transfers-index",
                    ChainNet::Matic,
                    &url,
                    mana(ChainNet::Matic),
                )],
            ),
        );
        eventually("the first process walks mana", || {
            published(&db.pool, chain_walk::INDEXER_MANA, ChainNet::Matic)
        })
        .await;
        let second = Jobs::with_leases(
            db.pool.clone(),
            lease_settings(&namespace, Duration::from_millis(100)),
        )
        .with_first_pass_now(true);
        chain_jobs::spawn(
            &second,
            supervisor(
                &st,
                ChainNet::Matic,
                vec![
                    projection(
                        "mana-transfers-index",
                        ChainNet::Matic,
                        &url,
                        mana(ChainNet::Matic),
                    ),
                    projection("trade-events-index", ChainNet::Matic, &url, trade_events),
                ],
            ),
        );
        eventually("trade events published on the second process", || {
            published(&db.pool, chain_walk::INDEXER_TRADE_EVENTS, ChainNet::Matic)
        })
        .await;
        eventually("mana published", || {
            published(&db.pool, chain_walk::INDEXER_MANA, ChainNet::Matic)
        })
        .await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        let mana = cursor(&db.pool, chain_walk::INDEXER_MANA, ChainNet::Matic)
            .await
            .unwrap();
        assert_eq!(
            mana.owner_generation, 1,
            "one writer, claimed once: {mana:?}"
        );
        assert!(first.shutdown(Duration::from_secs(5)).await.is_empty());
        assert!(second.shutdown(Duration::from_secs(5)).await.is_empty());
        db.drop().await;
    }

    /// Both networks walk under their own supervisors; when the process
    /// stops, it lets its leases go and another one takes both over.
    #[tokio::test]
    async fn two_networks_walk_and_hand_over_between_processes() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let chain = FakeChain::new(100, vec![], 1000, 1000);
        chain.finalized_lag.store(2, Ordering::SeqCst);
        let polygon_url = serve(chain.clone()).await;
        let ethereum = FakeChain::new(100, vec![], 1000, 1000);
        ethereum.chain_id.store(1, Ordering::SeqCst);
        let ethereum_url = serve(ethereum.clone()).await;
        let st = state_over(db.pool.clone(), &polygon_url, walk_settings(), false);
        let namespace = format!("supervisor-networks-{}", std::process::id());
        let start = |jobs: &Jobs| {
            for (net, url) in [
                (ChainNet::Matic, &polygon_url),
                (ChainNet::Ethereum, &ethereum_url),
            ] {
                chain_jobs::spawn(
                    jobs,
                    supervisor(
                        &st,
                        net,
                        vec![projection("mana-transfers-index", net, url, mana(net))],
                    ),
                );
            }
        };
        let first = Jobs::with_leases(
            db.pool.clone(),
            lease_settings(&namespace, Duration::from_millis(100)),
        )
        .with_first_pass_now(true);
        start(&first);
        for net in ChainNet::ALL {
            eventually("both networks published", || {
                published(&db.pool, chain_walk::INDEXER_MANA, net)
            })
            .await;
        }
        assert!(first.shutdown(Duration::from_secs(5)).await.is_empty());
        assert_eq!(
            advisory_locks(&db.pool).await,
            0,
            "a stopped process lets its leases go"
        );

        let second = Jobs::with_leases(
            db.pool.clone(),
            lease_settings(&namespace, Duration::from_millis(100)),
        )
        .with_first_pass_now(true);
        start(&second);
        for net in ChainNet::ALL {
            let pool = &db.pool;
            eventually("the second process claimed both", || async move {
                cursor(pool, chain_walk::INDEXER_MANA, net)
                    .await
                    .is_some_and(|c| {
                        c.owner_generation == 2 && c.folded_block == Some(c.last_block)
                    })
            })
            .await;
        }
        let ethereum_cursor = cursor(&db.pool, chain_walk::INDEXER_MANA, ChainNet::Ethereum)
            .await
            .unwrap();
        assert_eq!(ethereum_cursor.chain_id, Some(1));
        assert!(second.shutdown(Duration::from_secs(5)).await.is_empty());
        db.drop().await;
    }

    /// A process that runs no fleet jobs takes no lease, reads no chain and
    /// claims nothing, with leases or without them; with leases, another
    /// process can take the name.
    #[tokio::test]
    async fn a_process_without_fleet_jobs_takes_no_lease_and_reads_no_chain() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let polygon = FakeChain::new(100, vec![], 1000, 1000);
        let url = serve(polygon.clone()).await;
        let st = state_over(db.pool.clone(), &url, walk_settings(), false);
        let namespace = format!("supervisor-fleet-off-{}", std::process::id());
        let with_leases = Jobs::with_leases(
            db.pool.clone(),
            lease_settings(&namespace, Duration::from_millis(50)),
        )
        .with_fleet_jobs(false)
        .with_first_pass_now(true);
        let without_leases = Jobs::new().with_fleet_jobs(false).with_first_pass_now(true);
        for jobs in [&with_leases, &without_leases] {
            chain_jobs::spawn(
                jobs,
                supervisor(
                    &st,
                    ChainNet::Matic,
                    vec![projection(
                        "mana-transfers-index",
                        ChainNet::Matic,
                        &url,
                        mana(ChainNet::Matic),
                    )],
                ),
            );
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert_eq!(polygon.requests.load(Ordering::SeqCst), 0, "no chain read");
        assert!(
            cursor(&db.pool, chain_walk::INDEXER_MANA, ChainNet::Matic)
                .await
                .is_none(),
            "nothing claimed"
        );
        assert_eq!(advisory_locks(&db.pool).await, 0, "no lease taken");
        for jobs in [&with_leases, &without_leases] {
            let report = jobs.report();
            let job = report
                .iter()
                .find(|j| j.name == "chain-walk-matic")
                .unwrap();
            assert_eq!(job.state, JobState::StoodDown);
        }
        let other = Arc::new(crate::jobs::Leases::new(
            db.pool.clone(),
            lease_settings(&namespace, Duration::from_millis(50)),
        ));
        let guard = other
            .try_acquire("mana-transfers-index-matic")
            .await
            .unwrap()
            .expect("another process takes the name");
        guard.release().await;
        assert!(
            with_leases
                .shutdown(Duration::from_secs(5))
                .await
                .is_empty()
        );
        assert!(
            without_leases
                .shutdown(Duration::from_secs(5))
                .await
                .is_empty()
        );
        db.drop().await;
    }
}
