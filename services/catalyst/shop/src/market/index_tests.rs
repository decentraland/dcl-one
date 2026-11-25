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

tokio::task_local! {
    /// The shadow a test's walks offer their accepted windows to.
    static TEST_SHADOW: Option<Arc<crate::chain_index::portal::shadow::Shadow>>;
    /// The portal a test's walks read the windows it admits from.
    static TEST_PORTAL: Option<Arc<crate::chain_index::portal::admit::Admission>>;
}

/// The shadow of the running test, if it set one.
fn test_shadow() -> Option<Arc<crate::chain_index::portal::shadow::Shadow>> {
    TEST_SHADOW.try_with(Clone::clone).ok().flatten()
}

/// The portal of the running test, if it set one.
fn test_portal() -> Option<Arc<crate::chain_index::portal::admit::Admission>> {
    TEST_PORTAL.try_with(Clone::clone).ok().flatten()
}

/// A Polygon shadow over a portal that accepts connections and never
/// answers, its worker running until the returned sender is dropped, and
/// how many connections the portal took.
async fn stalled_shadow() -> (
    Arc<crate::chain_index::portal::shadow::Shadow>,
    tokio::sync::oneshot::Sender<()>,
    Arc<AtomicUsize>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let accepted = Arc::new(AtomicUsize::new(0));
    let count = accepted.clone();
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((sock, _)) = listener.accept().await {
            count.fetch_add(1, Ordering::SeqCst);
            held.push(sock);
        }
    });
    let reader =
        crate::chain_index::portal::PortalReader::new(&origin, "polygon-mainnet", None, 100.0)
            .unwrap();
    let (shadow, worker) = crate::chain_index::portal::shadow::channel(
        reader,
        "matic",
        crate::chain_index::portal::AddressHistory::Unknown,
    );
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(worker.run(async move {
        let _ = stopped.await;
    }));
    (shadow, stop, accepted)
}

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
    /// Logs without their `blockHash`, as an endpoint that omits it.
    strip_block_hash: std::sync::atomic::AtomicBool,
    /// `eth_chainId` answers an error.
    fail_chain_id: std::sync::atomic::AtomicBool,
    /// Text the failing reads echo in their error message (a gateway that
    /// repeats the request it got).
    echo: Mutex<String>,
    batches: AtomicUsize,
    drop_in_batch: Mutex<Vec<u64>>,
    /// The `eth_blockNumber` calls still answered; the ones after are
    /// refused (an endpoint that fails in the middle of a pass).
    head_reads_left: std::sync::atomic::AtomicI64,
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
    /// Count in `watched` every `eth_getLogs` whose first topics include all
    /// of these (only a shared read mixes two projections' topics).
    watch_topics: Mutex<Vec<String>>,
    watched: AtomicUsize,
    /// A fork from this block on: every block at or past it has another hash
    /// (salted), as every descendant of a reorged block does.
    fork_from: Mutex<Option<(u64, u8)>>,
    /// A fork applied right after a given header read is answered.
    fork_after: Mutex<Option<ForkAfter>>,
    /// Header reads below this block wait for this gate (counted in
    /// `probes_waiting`), or fail while `fail_probes` is set: a recovery's
    /// probes, apart from the tip's reads.
    probe_gate: Mutex<Option<(u64, Arc<tokio::sync::Semaphore>)>>,
    probes_waiting: AtomicUsize,
    fail_probes_below: Mutex<Option<u64>>,
    /// Header reads below this block take this long.
    probe_delay: Mutex<Option<(u64, std::time::Duration)>>,
    /// What a fork that only lasts one log read restores once that read
    /// was answered: the logs and the fork before it.
    revert_after_logs: Mutex<Option<Restore>>,
    /// Once the header at the first block was read this many times, header
    /// reads below the third wait for the gate (as `probe_gate`).
    gate_after: Mutex<Option<GateAfter>>,
    /// Header reads of exactly this block wait for this gate (counted in
    /// `probes_waiting`): one projection's probe, not another's.
    gate_at: Mutex<Option<(u64, Arc<tokio::sync::Semaphore>)>>,
    /// Requests that carried an `x-api-key` header (a portal's credential).
    api_keys: AtomicUsize,
}

/// The logs and the fork a fork lasting one log read restores.
type Restore = (Vec<Value>, Option<(u64, u8)>);

/// The header block, the reads of it left, the block below which reads
/// then wait, and their gate.
type GateAfter = (u64, usize, u64, Arc<tokio::sync::Semaphore>);

/// A fork the fake chain applies once the `reads`-th read of the header at
/// `header` was answered: blocks from `from` on take hashes salted `salt`,
/// and the logs from `from` on are replaced by `logs`.
struct ForkAfter {
    header: u64,
    reads: usize,
    from: u64,
    salt: u8,
    logs: Vec<Value>,
    /// The fork lasts one log read: the chain flips back once it answered.
    one_read: bool,
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
            strip_block_hash: std::sync::atomic::AtomicBool::new(false),
            fail_chain_id: std::sync::atomic::AtomicBool::new(false),
            echo: Mutex::new(String::new()),
            batches: AtomicUsize::new(0),
            drop_in_batch: Mutex::new(Vec::new()),
            head_reads_left: std::sync::atomic::AtomicI64::new(i64::MAX),
            calls: Mutex::new(Vec::new()),
            chain_id: std::sync::atomic::AtomicU64::new(137),
            fail_headers: std::sync::atomic::AtomicBool::new(false),
            refuse_topics: Mutex::new(Vec::new()),
            refused: AtomicUsize::new(0),
            refuse_from: Mutex::new(None),
            watch_topics: Mutex::new(Vec::new()),
            watched: AtomicUsize::new(0),
            fork_from: Mutex::new(None),
            fork_after: Mutex::new(None),
            probe_gate: Mutex::new(None),
            probes_waiting: AtomicUsize::new(0),
            fail_probes_below: Mutex::new(None),
            probe_delay: Mutex::new(None),
            revert_after_logs: Mutex::new(None),
            gate_after: Mutex::new(None),
            gate_at: Mutex::new(None),
            api_keys: AtomicUsize::new(0),
        })
    }

    fn hash_of(&self, block: u64) -> String {
        if let Some(hash) = self.forks.lock().get(&block) {
            return hash.clone();
        }
        match *self.fork_from.lock() {
            Some((from, salt)) if block >= from => format!("0x{salt:02x}{block:062x}"),
            _ => format!("0x{:064x}", block),
        }
    }

    /// The chain reorganizes from `from` on: every block from there has
    /// another hash, and the logs from there are `logs`.
    fn fork(&self, from: u64, salt: u8, logs: Vec<crate::chain_index::rpc::RpcLog>) {
        self.fork_with(from, salt, logs.iter().map(log_json).collect());
    }

    fn fork_with(&self, from: u64, salt: u8, logs: Vec<Value>) {
        *self.fork_from.lock() = Some((from, salt));
        let mut held = self.logs.lock();
        held.retain(|l| hex_u64(&l["blockNumber"]) < from);
        held.extend(logs);
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
    headers: axum::http::HeaderMap,
    axum::Json(body): axum::Json<Value>,
) -> axum::response::Response {
    chain.requests.fetch_add(1, Ordering::SeqCst);
    if headers.contains_key("x-api-key") {
        chain.api_keys.fetch_add(1, Ordering::SeqCst);
    }
    let tag = match &body {
        Value::Array(calls) => format!(
            "batch [{}]",
            calls.iter().map(call_tag).collect::<Vec<_>>().join(", ")
        ),
        one => call_tag(one),
    };
    chain.calls.lock().push(tag);
    if let Value::Object(_) = &body
        && body["method"] == "eth_getBlockByNumber"
        && body["params"][0] != "finalized"
    {
        let n = hex_u64(&body["params"][0]);
        let delay = *chain.probe_delay.lock();
        if let Some((below, wait)) = delay
            && n < below
        {
            tokio::time::sleep(wait).await;
        }
        let exact = chain.gate_at.lock().clone();
        if let Some((block, gate)) = exact
            && n == block
        {
            chain.probes_waiting.fetch_add(1, Ordering::SeqCst);
            gate.acquire().await.unwrap().forget();
        }
        let gate = chain.probe_gate.lock().clone();
        if let Some((below, gate)) = gate
            && n < below
        {
            chain.probes_waiting.fetch_add(1, Ordering::SeqCst);
            gate.acquire().await.unwrap().forget();
        }
        if chain
            .fail_probes_below
            .lock()
            .is_some_and(|below| n < below)
        {
            return axum::Json(json!({ "jsonrpc": "2.0", "id": body["id"], "error": { "code": -32000, "message": "header unavailable" } })).into_response();
        }
    }
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
        one => {
            let answered = answer(&chain, &one);
            if one["method"] == "eth_getLogs"
                && let Some((logs, fork)) = chain.revert_after_logs.lock().take()
            {
                *chain.logs.lock() = logs;
                *chain.fork_from.lock() = fork;
            }
            axum::Json(answered).into_response()
        }
    }
}

fn answer(chain: &FakeChain, body: &Value) -> Value {
    let id = body["id"].clone();
    let reply = |result: Value| json!({ "jsonrpc": "2.0", "id": id, "result": result });
    let refuse = |message: String| json!({ "jsonrpc": "2.0", "id": body["id"], "error": { "code": -32602, "message": message } });
    match body["method"].as_str() {
        Some("eth_blockNumber") => {
            if chain.head_reads_left.fetch_sub(1, Ordering::SeqCst) <= 0 {
                return refuse("the head is unavailable".into());
            }
            reply(json!(format!("0x{:x}", *chain.head.lock())))
        }
        Some("eth_chainId") if chain.fail_chain_id.load(Ordering::SeqCst) => {
            json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32000, "message": format!("upstream refused {}", chain.echo.lock()) } })
        }
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
                    return json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32000, "message": format!("header unavailable {}", chain.echo.lock()) } });
                }
                hex_u64(&body["params"][0])
            };
            let answered = reply(
                json!({ "number": format!("0x{n:x}"), "hash": chain.hash_of(n), "timestamp": format!("0x{:x}", time_of(n)) }),
            );
            let mut gating = chain.gate_after.lock();
            if let Some(g) = gating.as_mut()
                && body["params"][0] != "finalized"
                && g.0 == n
            {
                g.1 -= 1;
                if g.1 == 0 {
                    let (_, _, below, gate) = gating.take().unwrap();
                    *chain.probe_gate.lock() = Some((below, gate));
                }
            }
            drop(gating);
            let mut trigger = chain.fork_after.lock();
            if let Some(t) = trigger.as_mut()
                && body["params"][0] != "finalized"
                && t.header == n
            {
                t.reads -= 1;
                if t.reads == 0 {
                    let t = trigger.take().unwrap();
                    if t.one_read {
                        let saved = (chain.logs.lock().clone(), *chain.fork_from.lock());
                        *chain.revert_after_logs.lock() = Some(saved);
                    }
                    chain.fork_with(t.from, t.salt, t.logs);
                }
            }
            answered
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
            let watch = chain.watch_topics.lock().clone();
            if !watch.is_empty()
                && topics
                    .as_ref()
                    .is_some_and(|t| watch.iter().all(|w| t.contains(&w.to_ascii_lowercase())))
            {
                chain.watched.fetch_add(1, Ordering::SeqCst);
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
                // Each log names its block's hash, as an endpoint's do.
                .map(|mut l| {
                    if !chain.strip_block_hash.load(Ordering::SeqCst) {
                        let block = hex_u64(&l["blockNumber"]);
                        l["blockHash"] = json!(chain.hash_of(block));
                    }
                    l
                })
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
    let (shadow, portal) = (test_shadow(), test_portal());
    let source = crate::chain_index::portal::shadow::Shadowed::new(
        crate::chain_index::portal::admit::AdmittedPortal::new(
            chain_walk::RpcSource::new(Rpc::at(&state.http, url)),
            portal.as_deref(),
        ),
        shadow.as_deref(),
    );
    // With a portal the pass checks the chain through its source first, as
    // a job's pass does (and counts it): the portal admits only that chain.
    let checked = if portal.is_some() {
        super::ensure_chain(&source, ix.chain_id()).await?;
        1
    } else {
        0
    };
    let owner = chain_walk::test_owner(&state.pool, ix, &state.settings.market.indexes).await;
    let mut out = chain_walk::run_pass(
        &state.pool,
        &source,
        ix,
        &state.settings.market.indexes,
        &owner,
        None,
        &|_| {},
    )
    .await?;
    out.requests += checked;
    Ok(out)
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

    // A stale publish is not trusted, and the listings fail closed: no
    // trade of the network is listed, the sold one never again as open,
    // and no bid of it is reported.
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
        open_ids.is_empty(),
        "a stale index lists nothing of its network: {open_ids:?}"
    );
    let bids: Vec<(String, String)> =
        sqlx::query_as(sqlx::AssertSqlSafe(super::bids::test_trades_bids_sql()))
            .bind(chrono::Utc::now().naive_utc())
            .bind(None::<chrono::NaiveDateTime>)
            .fetch_all(&db.pool)
            .await
            .unwrap();
    assert!(bids.is_empty(), "{bids:?}");
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

/// A V3 trade on `net`'s mainnet signed by the test wallet (`salt` makes it
/// distinct), stored as `POST /v1/trades` stores it: with its digest.
async fn stored_v3_trade(pool: &PgPool, net: ChainNet, salt: &str) -> (String, String) {
    let chain_id = net.chain_id();
    let v3 = listings::offchain_marketplace_v3(chain_id).unwrap();
    let wallet = listings::test_support::signer().address().to_checksum(None);
    let mut trade = listings::test_support::parse(&listings::test_support::trade_json_on(
        &wallet, "0x00", chain_id,
    ));
    trade.checks.salt = salt.to_string();
    trade.network = net.db_name().to_ascii_uppercase();
    listings::test_support::sign(&mut trade, &v3);
    let digest = format!("{:#x}", listings::signing_hash(&trade, &v3).unwrap());
    let network = trade.network.clone();
    let id = match listings::insert_trade(pool, &trade, &network, v3.address).await {
        Ok(id) => id,
        Err(e) => panic!("the fixture trade is stored: {e}"),
    };
    (id, digest)
}

async fn chain_state(pool: &PgPool, id: &str) -> (Option<String>, String, bool) {
    sqlx::query_as(
        "SELECT digest, status, digest_failed_at IS NOT NULL FROM shop.trade_chain_state WHERE trade_id = $1::uuid",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn listed_ids(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "WITH {} SELECT id::text FROM unified_trades",
        super::catalog::unified_trades_cte("$1", "$2")
    )))
    .bind(chrono::Utc::now().naive_utc())
    .bind(None::<chrono::NaiveDateTime>)
    .fetch_all(pool)
    .await
    .unwrap()
}

/// On both walks, a V3 cancellation applies on the digest the trade stored
/// at creation even when the row can no longer be rebuilt into the signed
/// struct; a stored digest the signature does not recover the signer over
/// is not used (the rebuild is); a V3 trade with no verified digest is
/// marked, left out of the listings and read `unverifiable` by the MANA
/// facts; and a row the fold already holds without a digest (an older
/// failure, or one from before failures were marked) gets its digest on a
/// later fold, past the backoff, with the cancellation already in the log
/// applied, without a rebuild.
#[tokio::test]
async fn a_v3_cancellation_applies_on_the_stored_digest_and_a_trade_without_one_is_not_listed() {
    for net in [ChainNet::Matic, ChainNet::Ethereum] {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let chain_id = net.chain_id();
        let v3 = listings::offchain_marketplace_v3(chain_id).unwrap();
        let v3_block = trade_events::deployments(chain_id)
            .iter()
            .find(|(_, _, v3)| *v3)
            .unwrap()
            .1;
        let signer = listings::test_support::signer()
            .address()
            .to_string()
            .to_ascii_lowercase();
        // kept: its checks no longer parse, its stored digest keys the cancellation.
        let kept = stored_v3_trade(&db.pool, net, "0x1235").await;
        // bogus: a stored digest the signature does not recover over; the row rebuilds.
        let bogus = stored_v3_trade(&db.pool, net, "0x1236").await;
        // late: no stored digest and a row that does not rebuild, until an import brings it.
        let late = stored_v3_trade(&db.pool, net, "0x1237").await;
        // open: nothing closes it.
        let open = stored_v3_trade(&db.pool, net, "0x1238").await;
        sqlx::query("UPDATE shop.trades SET checks = '{}'::jsonb WHERE id = ANY($1::uuid[])")
            .bind(vec![kept.0.clone(), late.0.clone()])
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("UPDATE shop.trades SET trade_digest = NULL WHERE id = $1::uuid")
            .bind(&late.0)
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("UPDATE shop.trades SET trade_digest = $2 WHERE id = $1::uuid")
            .bind(&bogus.0)
            .bind(format!("0x{}", "11".repeat(32)))
            .execute(&db.pool)
            .await
            .unwrap();
        let head = v3_block + 200;
        let chain = FakeChain::new(
            head,
            vec![
                cancelled(v3_block + 10, v3.address, &signer, &kept.1),
                cancelled(v3_block + 11, v3.address, &signer, &bogus.1),
                cancelled(v3_block + 12, v3.address, &signer, &late.1),
            ],
            1_000_000_000,
            10,
        );
        chain.chain_id.store(chain_id as u64, Ordering::SeqCst);
        let url = serve(chain).await;
        let st = state_over(
            db.pool.clone(),
            &url,
            IndexSettings {
                window_blocks: 20_000_000,
                requests_per_pass: 50,
                ..index_settings(50, 20_000_000)
            },
            true,
        );
        let ix = TradeEventsIndex::mainnet(net);
        let mut last = PassOutcome::default();
        for _ in 0..10 {
            last = pass(&st, &url, &ix).await;
            if last.published {
                break;
            }
        }
        assert!(last.published, "{net:?}: {last:?}");
        assert_eq!(
            chain_state(&db.pool, &kept.0).await,
            (Some(kept.1.clone()), "cancelled".to_string(), false),
            "{net:?}: the stored digest keys the cancellation"
        );
        assert_eq!(
            chain_state(&db.pool, &bogus.0).await,
            (Some(bogus.1.clone()), "cancelled".to_string(), false),
            "{net:?}: the rebuilt digest, not the one that does not recover the signer"
        );
        assert_eq!(
            chain_state(&db.pool, &late.0).await,
            (None, "open".to_string(), true),
            "{net:?}: no verified digest: marked"
        );
        assert_eq!(
            listed_ids(&db.pool).await,
            vec![open.0.clone()],
            "{net:?}: neither the cancelled trades nor the unverifiable one is listed"
        );
        let verdict = crate::mana::facts::trade_verdict(&db.pool, net, chain_id, &late.0)
            .await
            .unwrap();
        assert_eq!(
            verdict,
            (Some(("unverifiable".to_string(), 0)), true),
            "{net:?}"
        );

        // The import brings the late trade's digest; within the backoff the
        // fold does not try again, past it the digest is verified and the
        // cancellation already in the log applies.
        sqlx::query("UPDATE shop.trades SET trade_digest = $2 WHERE id = $1::uuid")
            .bind(&late.0)
            .bind(&late.1)
            .execute(&db.pool)
            .await
            .unwrap();
        let owner = chain_walk::test_owner(&db.pool, &ix, &st.settings.market.indexes).await;
        trade_events::fold(
            &db.pool,
            net,
            chain_id,
            Some(head as i64),
            head as i64,
            &owner,
        )
        .await
        .unwrap();
        assert_eq!(
            chain_state(&db.pool, &late.0).await,
            (None, "open".to_string(), true),
            "{net:?}: within the backoff"
        );
        sqlx::query("UPDATE shop.trade_chain_state SET digest_failed_at = digest_failed_at - interval '11 minutes' WHERE trade_id = $1::uuid")
            .bind(&late.0)
            .execute(&db.pool)
            .await
            .unwrap();
        trade_events::fold(
            &db.pool,
            net,
            chain_id,
            Some(head as i64),
            head as i64,
            &owner,
        )
        .await
        .unwrap();
        assert_eq!(
            chain_state(&db.pool, &late.0).await,
            (Some(late.1.clone()), "cancelled".to_string(), false),
            "{net:?}: past the backoff"
        );

        // A row held without a digest and never marked (as an older binary
        // left it) is settled on the next fold.
        sqlx::query("UPDATE shop.trade_chain_state SET digest = NULL, status = 'open', digest_failed_at = NULL WHERE trade_id = $1::uuid")
            .bind(&kept.0)
            .execute(&db.pool)
            .await
            .unwrap();
        trade_events::fold(
            &db.pool,
            net,
            chain_id,
            Some(head as i64),
            head as i64,
            &owner,
        )
        .await
        .unwrap();
        assert_eq!(
            chain_state(&db.pool, &kept.0).await,
            (Some(kept.1.clone()), "cancelled".to_string(), false),
            "{net:?}: an unmarked row without a digest is retried at once"
        );
        db.drop().await;
    }
}

/// `POST /v1/trades` stores a V3 trade's cancellation digest, the hash the
/// signature verified over; a V2 trade, which cancels by signature, none.
#[tokio::test]
async fn a_stored_trade_keeps_its_v3_digest_and_a_v2_one_none() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let (v3_id, digest) = stored_v3_trade(&db.pool, ChainNet::Matic, "0x1240").await;
    let v2 = listings::offchain_marketplace_v2(137).unwrap();
    let v2_trade = listings::test_support::signed_trade_on(137, &v2);
    let v2_id = listings::insert_trade(&db.pool, &v2_trade, "MATIC", v2.address)
        .await
        .unwrap_or_else(|e| panic!("stored: {e}"));
    let stored = |id: String| {
        let pool = db.pool.clone();
        async move {
            sqlx::query_scalar::<_, Option<String>>(
                "SELECT trade_digest FROM shop.trades WHERE id = $1::uuid",
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };
    assert_eq!(stored(v3_id).await, Some(digest));
    assert_eq!(stored(v2_id).await, None);
    db.drop().await;
}

/// The `trades::ready` envelope as `(status, source, message)`.
async fn listing_gate(st: &MarketplaceState) -> (StatusCode, Option<String>, String) {
    match super::trades::ready(st).await {
        Ok(_) => (StatusCode::OK, None, String::new()),
        Err(response) => {
            let status = response.status();
            let body: Value = serde_json::from_slice(
                &axum::body::to_bytes(response.into_body(), 1 << 16)
                    .await
                    .unwrap(),
            )
            .unwrap_or(Value::Null);
            (
                status,
                body["source"].as_str().map(str::to_string),
                body["message"].as_str().unwrap_or_default().to_string(),
            )
        }
    }
}

/// The routes that list, price or report the status of trades fail closed
/// until the trade-events index is current on both networks: their gate
/// (`trades::ready`) answers the envelope naming the index and the networks
/// behind, and the open-listing read drops a network's trades as soon as
/// its cursor falls behind, in the same statement. The stored records of
/// `/v1/trades`, which carry no status, are served on the mirror's pass
/// alone, whatever the cursors say.
#[tokio::test]
async fn the_listing_routes_fail_closed_until_the_trade_events_index_is_current_on_both_networks() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let st = state_over(
        db.pool.clone(),
        "http://127.0.0.1:9/",
        index_settings(1, 1),
        true,
    );
    let app = super::market_router(st.clone());
    let (id, _) = seed_trade(
        &db.pool,
        "public_nft_order",
        ALICE,
        "0x00000000000000000000000000000000000000aa",
        1,
        0,
        (3, "0x00000000000000000000000000000000000000c1", "1"),
        (1, ZERO, "1"),
    )
    .await;
    let records_served = || async {
        for path in ["/v1/trades?first=1".to_string(), format!("/v1/trades/{id}")] {
            let (code, body) = get_json(&app, &path).await;
            assert_eq!(code, StatusCode::OK, "{path}: {body}");
        }
    };
    let (code, source, message) = listing_gate(&st).await;
    assert_eq!(
        (code, source.as_deref()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("trade-events-index")),
        "{message}"
    );
    assert!(message.contains("ethereum and matic"), "{message}");
    assert!(
        listed_ids(&db.pool).await.is_empty(),
        "no cursor, no listing"
    );
    records_served().await;

    // Polygon current, Ethereum not: still refused, naming Ethereum.
    sqlx::query("INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, fold_through, folded_block, folded_at, full_history, chain_id) VALUES ('trade_events', 'matic', 0, 10, 10, 10, timezone('UTC', now()), true, 137)")
        .execute(&db.pool)
        .await
        .unwrap();
    let (code, _, message) = listing_gate(&st).await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{message}");
    assert!(message.contains("fold on ethereum within"), "{message}");
    assert_eq!(
        listed_ids(&db.pool).await,
        vec![id.clone()],
        "the read itself asks only the trade's network"
    );

    trade_events::tests::publish_cursors(&db.pool).await;
    assert_eq!(listing_gate(&st).await.0, StatusCode::OK);

    // A pass in progress (its windows advanced `last_block`, its fold has not
    // published yet) keeps the previous publish: the routes serve, the trade
    // stays listed, and a sold one stays closed.
    let (sold, _) = seed_trade(
        &db.pool,
        "public_nft_order",
        ALICE,
        "0x00000000000000000000000000000000000000aa",
        1,
        0,
        (3, "0x00000000000000000000000000000000000000c1", "2"),
        (1, ZERO, "1"),
    )
    .await;
    sqlx::query("INSERT INTO shop.trade_chain_state (trade_id, network, executions, status, folded_block) VALUES ($1::uuid, 'matic', 1, 'sold', 10)")
        .bind(&sold)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE shop.chain_cursor SET last_block = last_block + 500 WHERE indexer = 'trade_events'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    assert_eq!(listing_gate(&st).await.0, StatusCode::OK, "mid-pass");
    assert_eq!(listed_ids(&db.pool).await, vec![id.clone()], "mid-pass");

    // The walker's clock a few minutes ahead of the reader's still counts;
    // past the skew allowance it does not.
    sqlx::query("UPDATE shop.chain_cursor SET folded_at = timezone('UTC', now()) + interval '5 minutes' WHERE indexer = 'trade_events'")
        .execute(&db.pool)
        .await
        .unwrap();
    assert_eq!(listing_gate(&st).await.0, StatusCode::OK, "within the skew");
    assert_eq!(listed_ids(&db.pool).await, vec![id.clone()]);
    sqlx::query("UPDATE shop.chain_cursor SET folded_at = timezone('UTC', now()) + interval '11 minutes' WHERE indexer = 'trade_events'")
        .execute(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        listing_gate(&st).await.0,
        StatusCode::SERVICE_UNAVAILABLE,
        "past the skew"
    );
    assert!(listed_ids(&db.pool).await.is_empty());
    trade_events::tests::publish_cursors(&db.pool).await;

    // The publish is withdrawn after the gate (a rewind, a divergence): the
    // statement drops the trade, and the next gate refuses; the records stay
    // served.
    sqlx::query("UPDATE shop.chain_cursor SET folded_block = NULL WHERE indexer = 'trade_events' AND network = 'matic'")
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(listed_ids(&db.pool).await.is_empty());
    assert_eq!(listing_gate(&st).await.0, StatusCode::SERVICE_UNAVAILABLE);
    records_served().await;
    db.drop().await;
}

/// `/v1/bids` reports a bid only while its network's trade-events index is
/// current, and never a V3 bid whose cancellation digest could not be
/// established: its cancellation would go unseen. A pass in progress keeps
/// the previous publish: a sold bid stays `sold`, never `open`.
#[tokio::test]
async fn an_unverifiable_v3_bid_is_not_reported() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let mana = "0xa1c57f48f0deb89f569dfbe6e2b7f46d33606fd4";
    let item = "0x00000000000000000000000000000000000000c1";
    let bid = |token: &'static str| {
        let pool = db.pool.clone();
        async move {
            seed_trade(
                &pool,
                "bid",
                ALICE,
                "0x00000000000000000000000000000000000000aa",
                1,
                0,
                (1, mana, "1000"),
                (3, item, token),
            )
            .await
            .0
        }
    };
    let (kept, unverifiable, sold) = (bid("1").await, bid("2").await, bid("3").await);
    sqlx::query("INSERT INTO shop.trade_chain_state (trade_id, network, status, folded_block, digest_failed_at) VALUES ($1::uuid, 'matic', 'open', 10, timezone('UTC', now()))")
        .bind(&unverifiable)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO shop.trade_chain_state (trade_id, network, executions, status, folded_block) VALUES ($1::uuid, 'matic', 1, 'sold', 10)")
        .bind(&sold)
        .execute(&db.pool)
        .await
        .unwrap();
    let reported = || async {
        let mut rows: Vec<(String, String)> = sqlx::query_as::<_, (String, String)>(
            sqlx::AssertSqlSafe(super::bids::test_trades_bids_sql()),
        )
        .bind(chrono::Utc::now().naive_utc())
        .bind(None::<chrono::NaiveDateTime>)
        .fetch_all(&db.pool)
        .await
        .unwrap();
        rows.sort();
        rows
    };
    assert!(reported().await.is_empty(), "no cursor, no bid");
    trade_events::tests::publish_cursors(&db.pool).await;
    let mut want = vec![
        (kept.clone(), "open".to_string()),
        (sold.clone(), "sold".to_string()),
    ];
    want.sort();
    assert_eq!(reported().await, want);
    sqlx::query(
        "UPDATE shop.chain_cursor SET last_block = last_block + 500 WHERE indexer = 'trade_events'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    assert_eq!(reported().await, want, "mid-pass");
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
    // The items mirror's pass, which the owned-items reads need.
    sqlx::query(
        "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at) \
         VALUES ('items', $1, 'https://items.example', timezone('UTC', now())) ON CONFLICT (mirror, network) DO NOTHING",
    )
    .bind(network)
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

fn estate_document(
    block: u64,
    log: u64,
    token: u64,
    create: bool,
    data: &str,
) -> crate::chain_index::rpc::RpcLog {
    let estate = crate::land_authz::ESTATE_REGISTRY_MAINNET;
    let id = topic_word(U256::from(token));
    let topics = if create {
        vec![
            crate::land_authz::events::TOPIC_CREATE_ESTATE.into(),
            topic_addr(ALICE),
            id,
        ]
    } else {
        vec![
            nft_index::TOPIC_LAND_UPDATE.into(),
            id,
            topic_addr(ALICE),
            topic_addr(ALICE),
        ]
    };
    log_at(
        estate,
        topics,
        (data.to_string(),).abi_encode_params(),
        block,
        log,
    )
}

#[tokio::test]
async fn a_testnet_graph_page_proves_its_block_even_when_the_page_is_empty() {
    let chain = FakeChain::new(100, vec![], 100, 100);
    chain.chain_id.store(80_002, Ordering::SeqCst);
    let rpc = serve(chain.clone()).await;
    let good = chain.hash_of(90);
    let meta = Arc::new(Mutex::new(json!({"number":90,"hash":"wrong"})));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let graph = format!("http://{}/", listener.local_addr().unwrap());
    let app = Router::new().route("/", post(|State(meta): State<Arc<Mutex<Value>>>, axum::Json(body): axum::Json<Value>| async move {
        let meta = meta.lock().clone();
        let block = if body["variables"]["number"] == 90 || body["variables"]["block"].is_object() {
            meta.clone()
        } else if meta.is_null() {
            Value::Null
        } else {
            json!({"number":90+ChainNet::Matic.confirmations()})
        };
        axum::Json(json!({"data":{"collections":[],"_meta":{"block":block}}}))
    })).with_state(meta.clone());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut state = super::tests::support::state_without_db();
    state.index.allow_loopback_for_tests();
    let settings = &mut Arc::get_mut(&mut state).unwrap().settings;
    settings.market.indexes.nft_enabled = false;
    settings.market.indexes.chain_id_matic = ChainNet::Matic.testnet_chain_id();
    settings.market.trade_rpc_urls = RpcEndpoints::from_env(Some(&format!("80002={rpc}")));
    let prove = || super::mirrors::catalogue_pass_hash(&state, ChainNet::Matic, &graph);
    assert!(
        prove()
            .await
            .unwrap_err()
            .contains("differs from the configured chain")
    );
    *meta.lock() = Value::Null;
    assert!(prove().await.unwrap_err().contains("no block number"));
    *meta.lock() = json!({"number":90});
    assert!(prove().await.unwrap_err().contains("no block hash"));
    *meta.lock() = json!({"number":90,"hash":good});
    chain.fail_chain_id.store(true, Ordering::SeqCst);
    assert!(prove().await.is_err());
    chain.fail_chain_id.store(false, Ordering::SeqCst);
    let pinned = prove().await.unwrap();
    let calls = chain.requests.load(Ordering::SeqCst);
    for after in [None, Some("next".to_owned())] {
        let page = super::mirrors::read_catalogue_page(
            &state,
            &graph,
            "{collections{id}}",
            after,
            Some(pinned.clone()),
        )
        .await
        .unwrap();
        assert_eq!(page["collections"], json!([]));
    }
    assert_eq!(chain.requests.load(Ordering::SeqCst), calls);
    chain.chain_id.store(137, Ordering::SeqCst);
    assert!(prove().await.unwrap_err().contains("serves chain 137"));
}

#[tokio::test]
async fn a_split_mirror_resumes_the_pinned_graph_block_without_another_rpc_read() {
    use super::{
        graph::Network,
        mirrors::{self, Mirror},
    };
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let net = ChainNet::Matic;
    let chain = FakeChain::new(200, vec![], 100, 100);
    chain
        .chain_id
        .store(net.testnet_chain_id() as u64, Ordering::SeqCst);
    let rpc = serve(chain.clone()).await;
    let hash = chain.hash_of(100);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let graph = format!("http://{}/", listener.local_addr().unwrap());
    let app = Router::new()
        .route(
            "/",
            post(
                |State(hash): State<String>, axum::Json(body): axum::Json<Value>| async move {
                    let vars = &body["variables"];
                    let doc = body["query"].as_str().unwrap();
                    let data = if vars.get("idFrom").is_some() {
                        assert_eq!(vars["block"]["hash"], hash);
                        assert!(doc.contains("collections(block: $block"));
                        let rows = if vars["idFrom"] == "" {
                            (1..=mirrors::PAGE_SIZE)
                                .map(|i| json!({"id":format!("0x{i:040x}")}))
                                .collect::<Vec<_>>()
                        } else {
                            Vec::new()
                        };
                        json!({"_meta":{"block":{"hash":hash}},"collections":rows})
                    } else if vars["number"] == 100 {
                        json!({"_meta":{"block":{"number":100,"hash":hash}}})
                    } else {
                        json!({"_meta":{"block":{"number":100+ChainNet::Matic.confirmations()}}})
                    };
                    axum::Json(json!({"data":data}))
                },
            ),
        )
        .with_state(hash.clone());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let state = || {
        let mut config = Settings::for_tests();
        config.matic_collections_subgraph_url = Some(graph.clone());
        config.eth_collections_subgraph_url = None;
        config.market.indexes.nft_enabled = false;
        config.market.indexes.chain_id_matic = net.testnet_chain_id();
        config.market.mirror_pages_per_tick = 1;
        config.market.trade_rpc_urls = RpcEndpoints::from_env(Some(&format!("80002={rpc}")));
        let state =
            MarketplaceState::new(db.pool.clone(), crate::http::Outbound::for_tests(), config);
        state.index.allow_loopback_for_tests();
        state
    };
    let jobs = crate::jobs::Jobs::new();
    let ctx = super::market_tests::job_ctx(&jobs).await;
    mirrors::run_tick(&state(), Mirror::Collections, &ctx)
        .await
        .unwrap();
    let first = mirrors::load(&db.pool, Mirror::Collections, Network::Matic)
        .await
        .unwrap()
        .unwrap();
    assert!(first.in_progress());
    assert_eq!(first.pass_block_hash.as_deref(), Some(hash.as_str()));
    assert_eq!(first.chain_id, Some(net.testnet_chain_id()));
    let calls = chain.requests.load(Ordering::SeqCst);
    assert_eq!(calls, 2);
    chain.fail_chain_id.store(true, Ordering::SeqCst);
    mirrors::run_tick(&state(), Mirror::Collections, &ctx)
        .await
        .unwrap();
    assert_eq!(chain.requests.load(Ordering::SeqCst), calls);
    assert!(nft_index::mirror_complete(&db.pool, net, net.testnet_chain_id()).await);
    let mut disconnected = state();
    disconnected.settings.market.trade_rpc_urls = RpcEndpoints::from_env(None);
    assert!(
        mirrors::run_tick(&disconnected, Mirror::Collections, &ctx)
            .await
            .is_err()
    );
    assert!(nft_index::mirror_complete(&db.pool, net, net.testnet_chain_id()).await);
    db.drop().await;
}

#[tokio::test]
async fn an_unbound_mirror_keyset_is_adopted_only_for_the_same_mainnet_source() {
    use super::{
        graph::Network,
        mirrors::{self, Mirror},
    };
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    sqlx::query("INSERT INTO shop.mirror_state (mirror,network,source_url,pass_started_at,cursor_id) VALUES ('items','matic','source',timezone('UTC',now()),'next')")
        .execute(&db.pool).await.unwrap();
    let read =
        |url, chain| mirrors::cursor_for(&db.pool, Mirror::Items, Network::Matic, url, chain);
    assert_eq!(
        read("source", Some(137))
            .await
            .unwrap()
            .cursor_id
            .as_deref(),
        Some("next")
    );
    assert!(
        read("source", Some(80002))
            .await
            .unwrap()
            .cursor_id
            .is_none()
    );
    assert!(
        read("different", Some(137))
            .await
            .unwrap()
            .cursor_id
            .is_none()
    );
    sqlx::query("UPDATE shop.mirror_state SET chain_id=80002")
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(read("source", Some(137)).await.is_err());
    assert!(read("source", None).await.is_err());
    db.drop().await;
}

#[tokio::test]
async fn catalogue_startup_refuses_another_pair_and_allows_independent_rentals() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let main = IndexSettings::default();
    let test = IndexSettings {
        chain_id_ethereum: ChainNet::Ethereum.testnet_chain_id(),
        chain_id_matic: ChainNet::Matic.testnet_chain_id(),
        ..main.clone()
    };
    sqlx::query("INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,full_history)
      VALUES ('rentals_sepolia','ethereum',11155111,1,0,true),('nft','matic',137,1,0,true)")
        .execute(&db.pool).await.unwrap();
    assert!(
        test.check_catalogue_db(&db.pool, true)
            .await
            .unwrap_err()
            .to_string()
            .contains("nft/matic")
    );
    sqlx::query("DELETE FROM shop.chain_cursor WHERE indexer='nft'")
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO shop.mirror_state(mirror,network,source_url,chain_id) VALUES ('items','matic','test',137)").execute(&db.pool).await.unwrap();
    assert!(
        test.check_catalogue_db(&db.pool, true)
            .await
            .unwrap_err()
            .to_string()
            .contains("items/matic")
    );
    main.check_catalogue_db(&db.pool, true).await.unwrap();
    sqlx::query("DELETE FROM shop.mirror_state")
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(test.check_catalogue_db(&db.pool, false).await.is_err());
    main.check_catalogue_db(&db.pool, false).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn the_rpc_fallback_is_bound_to_the_configured_catalogue_pair() {
    let mut state = super::tests::support::state_without_db();
    let settings = &mut Arc::get_mut(&mut state).unwrap().settings;
    settings.eth_network = "sepolia".into();
    settings.market.indexes.chain_id_ethereum = 11_155_111;
    settings.market.indexes.chain_id_matic = 80_002;
    settings.market.trade_rpc_urls = RpcEndpoints::from_env(None);
    settings.eth_rpc_url = Some("https://sepolia.example".into());
    settings.polygon_rpc_url = Some("https://amoy.example".into());
    assert_eq!(
        state.catalogue_rpc_url(ChainNet::Ethereum).as_deref(),
        Some("https://sepolia.example")
    );
    assert_eq!(
        state.catalogue_rpc_url(ChainNet::Matic).as_deref(),
        Some("https://amoy.example")
    );
    Arc::get_mut(&mut state).unwrap().settings.eth_network = "mainnet".into();
    assert!(state.catalogue_rpc_url(ChainNet::Ethereum).is_none());
    assert!(state.catalogue_rpc_url(ChainNet::Matic).is_none());
}

#[tokio::test]
async fn testnet_nft_walks_publish_only_their_catalogue_and_chain() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let settings = IndexSettings {
        chain_id_ethereum: 11_155_111,
        chain_id_matic: 80_002,
        ..index_settings(120, 100)
    };
    for (net, chain_id, addr, first) in [
        (
            ChainNet::Ethereum,
            11_155_111,
            "0x7518456ae93eb98f3e64571b689c626616bb7f30",
            3_831_219,
        ),
        (
            ChainNet::Matic,
            80_002,
            "0x0c8ad1f6aadf89d2eb19f01a100a6143108fe2b0",
            5_706_656,
        ),
    ] {
        let ix = NftIndex::for_chain(net, chain_id).unwrap();
        assert_eq!(ix.start_block(), first);
        sqlx::query("INSERT INTO shop.mirror_state (mirror,network,source_url,chain_id,last_complete_pass_at) VALUES ('collections',$1,'source',$2,timezone('UTC',now()))")
            .bind(net.db_name()).bind(net.chain_id()).execute(&db.pool).await.unwrap();
        assert!(!nft_index::mirror_complete(&db.pool, net, chain_id).await);
        sqlx::query(
            "UPDATE shop.mirror_state SET chain_id=$2 WHERE network=$1 AND mirror='collections'",
        )
        .bind(net.db_name())
        .bind(chain_id)
        .execute(&db.pool)
        .await
        .unwrap();
        assert!(nft_index::mirror_complete(&db.pool, net, chain_id).await);
        let at = first + 25;
        let token = U256::from(12);
        let other = "0x00000000000000000000000000000000000000e2";
        seed_collection(
            &db.pool,
            net.db_name(),
            other,
            first,
            "unused",
            "Other chain",
        )
        .await;
        // A graph-only writer can have stamped this chain ID before an RPC
        // was attached. A later proven empty pass cannot admit that address.
        sqlx::query(
            "UPDATE shop.collections SET chain_id=$1 WHERE network=$2 AND contract_address=$3",
        )
        .bind(chain_id as i32)
        .bind(net.db_name())
        .bind(other)
        .execute(&db.pool)
        .await
        .unwrap();
        let mut logs = vec![
            transfer(addr, ZERO, BOB, token, at, 2),
            transfer(other, ZERO, CARL, token, at, 3),
            transfer(nft_index::DCL_REGISTRAR, ZERO, CARL, token, at, 4),
        ];
        if net == ChainNet::Ethereum {
            for (pos, create, data) in [(0, true, "0,\"named\",\"desc\""), (1, false, "0")] {
                let mut doc = estate_document(at, pos, 12, create, data);
                doc.address = crate::land_authz::ESTATE_REGISTRY_SEPOLIA.into();
                logs.push(doc);
            }
        }
        let chain = FakeChain::new(at + net.confirmations(), logs, 100, 100);
        let url = serve(chain.clone()).await;
        let st = state_over(db.pool.clone(), &url, settings.clone(), false);
        chain
            .chain_id
            .store(net.chain_id() as u64, Ordering::SeqCst);
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
        let error = run().await.unwrap_err();
        assert!(
            error.downcast_ref::<chain_walk::WrongChain>().is_some(),
            "{error:#}"
        );
        chain.chain_id.store(chain_id as u64, Ordering::SeqCst);
        assert!(run().await.unwrap().published);
        let c = chain_walk::cursor(&db.pool, INDEXER_NFT, net)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            (c.chain_id, c.folded_block),
            (Some(chain_id), Some(at as i64))
        );
        assert_eq!(
            owner_of(&db.pool, net.db_name(), addr, token).await,
            Some(Some(BOB.to_string()))
        );
        assert_eq!(owner_of(&db.pool, net.db_name(), other, token).await, None);
        assert_eq!(
            owner_of(&db.pool, net.db_name(), nft_index::DCL_REGISTRAR, token).await,
            None
        );
        if net == ChainNet::Ethereum {
            let fields: (String, String, i32) = sqlx::query_as("SELECT name, description, updated_log FROM shop.land_data WHERE token_address=$1 AND token_id=12")
                .bind(crate::land_authz::ESTATE_REGISTRY_SEPOLIA).fetch_one(&db.pool).await.unwrap();
            assert_eq!(fields, ("named".into(), "desc".into(), 0));
            let repairs: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM shop.worker_progress WHERE worker='nft_estate_creations'",
            )
            .fetch_one(&db.pool)
            .await
            .unwrap();
            assert_eq!(repairs, 0);
        }
        chain
            .chain_id
            .store(net.chain_id() as u64, Ordering::SeqCst);
        assert!(
            pass(&st, &url, &NftIndex::new(net))
                .await
                .quarantined
                .is_some()
        );
        assert_eq!(
            owner_of(&db.pool, net.db_name(), addr, token).await,
            Some(Some(BOB.to_string()))
        );
    }
    db.drop().await;
}

#[tokio::test]
async fn an_advanced_nft_cursor_repairs_estate_creations_after_publication_and_resumes() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let first = 6_236_547;
    let last = first + 5;
    let net = ChainNet::Ethereum;
    let renamed = estate_document(first + 4, 1, 2, false, r#"0,"Later","Update""#);
    let chain = FakeChain::new(
        first - 1 + net.confirmations(),
        vec![renamed, estate_document(last, 2, 2, false, "0")],
        2_000_000,
        100,
    );
    chain.chain_id.store(1, Ordering::SeqCst);
    let url = serve(chain.clone()).await;
    let original = state_over(db.pool.clone(), &url, index_settings(100, 2_000_000), false);
    let index = NftIndex::new(net);
    assert!(publish_nft(&original, &url, &index).await.published);
    *chain.head.lock() = last + net.confirmations();
    assert!(publish_nft(&original, &url, &index).await.published);
    // Model the pre-upgrade database: it has the Update and a published
    // cursor, but never requested the two historical creations.
    sqlx::query("DELETE FROM shop.worker_progress WHERE worker='nft_estate_creations'")
        .execute(&db.pool)
        .await
        .unwrap();
    // The old parser accepted "0" and erased the preceding valid Update.
    sqlx::query("UPDATE shop.land_data SET name=NULL, description=NULL, updated_block=$1, updated_log=2 WHERE token_id=2")
        .bind(last as i64).execute(&db.pool).await.unwrap();
    let mut undecodable = log_json(&estate_document(first + 1, 4, 9, true, "0,Ignored,"));
    undecodable["data"] = serde_json::Value::String("0x01".into());
    chain.logs.lock().extend([
        log_json(&estate_document(
            first,
            0,
            1,
            true,
            r#"0,"Original","Desc""#,
        )),
        undecodable,
        log_json(&estate_document(
            first + 2,
            0,
            2,
            true,
            r#"0,"Older","Desc""#,
        )),
    ]);
    let small = state_over(db.pool.clone(), &url, index_settings(20, 2), false);
    let marker = || {
        sqlx::query_as::<_, (i64, i64)>(
            "SELECT position, ceiling FROM shop.worker_progress
             WHERE worker = 'nft_estate_creations' AND scope = 1",
        )
        .fetch_one(&db.pool)
    };
    let names = || {
        sqlx::query_as::<_, (String, String)>(
            "SELECT token_id::text, coalesce(name, '') FROM shop.land_data ORDER BY token_id",
        )
        .fetch_all(&db.pool)
    };
    let old = vec![(String::from("2"), String::new())];
    assert_eq!(names().await.unwrap(), old);

    // The forward walk spends this pass's log budget. It captures the old
    // cursor before advancing and publishes the next block immediately.
    let busy = state_over(db.pool.clone(), &url, index_settings(1, 2), false);
    *chain.head.lock() += 1;
    assert!(pass(&busy, &url, &NftIndex::new(net)).await.published);
    assert_eq!(marker().await.unwrap(), (first as i64 - 1, last as i64));
    // Pause the archive at its historical upper bound and observe the new
    // publish from a separate connection before the read can finish.
    let published_at = || {
        sqlx::query_scalar::<_, chrono::NaiveDateTime>(
        "SELECT folded_at FROM shop.chain_cursor WHERE indexer='nft' AND network='ethereum' AND folded_block=last_block"
    ).fetch_one(&db.pool)
    };
    let before = published_at().await.unwrap();
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    *chain.gate_at.lock() = Some((first + 1, gate.clone()));
    chain.reorg_on_logs.store(true, Ordering::SeqCst);
    let restarted = NftIndex::new(net);
    let (out, during) = tokio::join!(pass(&small, &url, &restarted), async {
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            while chain.probes_waiting.load(Ordering::SeqCst) == 0 {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("archive reached the gated header");
        let during = published_at().await.unwrap();
        *chain.gate_at.lock() = None;
        gate.add_permits(1);
        during
    });
    assert!(out.published && during > before);
    assert_eq!(published_at().await.unwrap(), during);
    assert_eq!(marker().await.unwrap(), (first as i64 - 1, last as i64));
    chain.forks.lock().remove(&(first + 1));

    // The RPC briefly serves another branch for just the historical log
    // read. Both endpoint headers look stable again by the closing bracket.
    *chain.fork_after.lock() = Some(ForkAfter {
        header: first + 1,
        reads: 1,
        from: first,
        salt: 0xcf,
        logs: vec![log_json(&estate_document(first, 0, 1, true, "0,Orphan,"))],
        one_read: true,
    });
    assert!(pass(&small, &url, &NftIndex::new(net)).await.published);
    assert_eq!(marker().await.unwrap(), (first as i64 - 1, last as i64));

    // This first window spans the first two event blocks. Its request budget
    // stops before the next creation block; the next owner resumes there.
    let dense = state_over(db.pool.clone(), &url, index_settings(2, 6), false);
    for (position, state) in [(first + 1, &dense), (first + 3, &small), (last, &small)] {
        let out = pass(state, &url, &NftIndex::new(net)).await;
        assert!(out.published, "{out:?}");
        assert_eq!(marker().await.unwrap(), (position as i64, last as i64));
        if position < last {
            assert_eq!(names().await.unwrap(), old);
        }
    }
    assert_eq!(
        names().await.unwrap(),
        [
            ("1".into(), "Original".into()),
            ("2".into(), "Later".into())
        ]
    );
    let restored: (String, i64, i32) = sqlx::query_as(
        "SELECT description, updated_block, updated_log FROM shop.land_data WHERE token_id=2",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(restored, (String::from("Update"), (first + 4) as i64, 1));
    let reads = chain.get_logs.load(Ordering::SeqCst);
    assert!(pass(&small, &url, &NftIndex::new(net)).await.published);
    assert_eq!(chain.get_logs.load(Ordering::SeqCst), reads);

    chain.fork(first, 7, vec![]);
    assert!(
        publish_nft(&small, &url, &NftIndex::new(net))
            .await
            .published
    );
    assert!(names().await.unwrap().is_empty());
    assert_eq!(
        marker().await.unwrap(),
        (first as i64 - 1, first as i64 - 1)
    );
    db.drop().await;
}

#[tokio::test]
async fn estate_repair_reconciles_legacy_rows_and_recovers_an_old_writer_interval() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let net = ChainNet::Ethereum;
    let ix = NftIndex::new(net);
    let first = 6_236_547;
    let h = first + 4;
    let estate = crate::land_authz::ESTATE_REGISTRY_MAINNET;
    let land = crate::land_authz::LAND_REGISTRY_MAINNET;
    let mut parcel = estate_document(first, 1, 7, false, r#"0,"Parcel","Kept""#);
    parcel.address = land.into();
    let chain = FakeChain::new(
        h + net.confirmations(),
        vec![
            parcel,
            estate_document(first, 2, 1, true, "bad"),
            estate_document(first + 1, 0, 1, false, r#"0 ,"Legacy","Desc""#),
            estate_document(first + 1, 1, 2, true, r#"0,"","""#),
            estate_document(first + 2, 0, 3, true, "0,Orphaned,"),
        ],
        2_000_000,
        100,
    );
    chain.chain_id.store(1, Ordering::SeqCst);
    let url = serve(chain.clone()).await;
    let st = state_over(db.pool.clone(), &url, index_settings(100, 2_000_000), false);
    assert!(publish_nft(&st, &url, &ix).await.published);
    // State of a legacy writer that accepted the whitespace before the comma
    // and never captured CreateEstate. Keep its parcel row through the repair.
    sqlx::query("DELETE FROM shop.worker_progress WHERE worker='nft_estate_creations'")
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM shop.nft_event WHERE kind='land_data' AND to_address IS NOT NULL")
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM shop.land_data WHERE token_address=$1")
        .bind(estate)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO shop.land_data VALUES ($1,1,'Legacy','Desc',$2,0)")
        .bind(estate)
        .bind((first + 1) as i64)
        .execute(&db.pool)
        .await
        .unwrap();
    // A retained recovery point lands on the last archived block while
    // reconciliation is still owed. The legacy row has no orphaned event.
    let partial = state_over(db.pool.clone(), &url, index_settings(100, 2), false);
    assert!(pass(&partial, &url, &ix).await.published);
    let retained = first + 1;
    let progress: (i64, i64) = sqlx::query_as(
        "SELECT position,ceiling FROM shop.worker_progress WHERE worker='nft_estate_creations'",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(progress, (retained as i64, h as i64));
    let retained_hash = chain.hash_of(retained);
    sqlx::query(
        "INSERT INTO shop.chain_checkpoint VALUES ('nft','ethereum',$1,$2,$3)
        ON CONFLICT (indexer,network,block_number) DO NOTHING",
    )
    .bind(retained as i64)
    .bind(&retained_hash)
    .bind(time_of(retained))
    .execute(&db.pool)
    .await
    .unwrap();
    let owner = chain_walk::test_owner(&db.pool, &ix, &IndexSettings::default()).await;
    let mut tx = db.pool.begin().await.unwrap();
    ix.rewind(&mut tx, retained as i64).await.unwrap();
    chain_walk::rewind(
        &mut tx,
        ix.indexer(),
        net,
        (h as i64, Some(&chain.hash_of(h))),
        retained as i64,
        Some((&retained_hash, time_of(retained))),
    )
    .await
    .unwrap();
    owner.check(&mut tx).await.unwrap();
    tx.commit().await.unwrap();
    let shortened: (i64, i64) = sqlx::query_as(
        "SELECT position,ceiling FROM shop.worker_progress WHERE worker='nft_estate_creations' AND scope=1"
    ).fetch_one(&db.pool).await.unwrap();
    assert_eq!(shortened, (retained as i64, retained as i64));
    let reads_before = chain.get_logs.load(Ordering::SeqCst);
    *chain.head.lock() = retained + net.confirmations();
    assert!(pass(&st, &url, &ix).await.published);
    assert_eq!(
        chain.get_logs.load(Ordering::SeqCst),
        reads_before,
        "an accounted rewind keeps the retained archive proof"
    );
    let rows = || {
        sqlx::query_as::<_,(String,String,Option<String>,Option<String>,i64,i32)>(
        "SELECT token_address,token_id::text,name,description,updated_block,updated_log FROM shop.land_data ORDER BY token_id,token_address"
    ).fetch_all(&db.pool)
    };
    let repaired = rows().await.unwrap();
    assert_eq!(
        repaired,
        [
            (
                estate.into(),
                "2".into(),
                Some(String::new()),
                Some(String::new()),
                (first + 1) as i64,
                1
            ),
            (
                land.into(),
                "7".into(),
                Some("Parcel".into()),
                Some("Kept".into()),
                first as i64,
                1
            ),
        ]
    );
    let owner = chain_walk::test_owner(&db.pool, &ix, &IndexSettings::default()).await;
    sqlx::query("TRUNCATE shop.land_data")
        .execute(&db.pool)
        .await
        .unwrap();
    nft_index::fold(&db.pool, net, None, &owner).await.unwrap();
    assert_eq!(rows().await.unwrap(), repaired);

    *chain.head.lock() = h + net.confirmations();
    assert!(publish_nft(&st, &url, &ix).await.published);
    // Stop the new writer at H. The old writer commits the Updates and its
    // cursor through J, without changing our existing, completed marker.
    let owner = chain_walk::test_owner(&db.pool, &ix, &IndexSettings::default()).await;
    let j = h + 3;
    let rename = estate_document(j, 0, 4, false, r#"0,"Later","Update""#);
    chain.logs.lock().extend([
        log_json(&estate_document(h + 1, 0, 3, true, "0,Recovered,")),
        log_json(&estate_document(h + 2, 0, 4, true, "0,Earlier,")),
        log_json(&rename),
    ]);
    *chain.head.lock() = j + net.confirmations();
    let mut tx = db.pool.begin().await.unwrap();
    ix.write(&mut tx, &[rename], &HashMap::from([(j as i64, time_of(j))]))
        .await
        .unwrap();
    chain_walk::advance(
        &mut tx,
        ix.indexer(),
        net,
        (h as i64, Some(&chain.hash_of(h))),
        &crate::chain_index::rpc::BlockHeader {
            number: j,
            hash: chain.hash_of(j),
            timestamp: time_of(j),
        },
    )
    .await
    .unwrap();
    owner.check(&mut tx).await.unwrap();
    tx.commit().await.unwrap();
    // Restart caught up at J: no marker reset or manual replay.
    assert!(pass(&st, &url, &NftIndex::new(net)).await.published);
    let names: Vec<(String,String)> = sqlx::query_as(
        "SELECT token_id::text,name FROM shop.land_data WHERE token_address=$1 AND token_id IN (3,4) ORDER BY token_id"
    ).bind(estate).fetch_all(&db.pool).await.unwrap();
    assert_eq!(
        names,
        [
            ("3".into(), "Recovered".into()),
            ("4".into(), "Later".into())
        ]
    );
    // An old writer resets the whole projection and advances over the
    // same chain. Its cursor ends at exactly the marker's former height and hash.
    let before_rebuild = rows().await.unwrap();
    let mut tx = db.pool.begin().await.unwrap();
    sqlx::query("UPDATE shop.chain_cursor SET rebuild_requested_at=timezone('UTC',now()) WHERE indexer='nft' AND network='ethereum'")
        .execute(&mut *tx).await.unwrap();
    let start: i64 = sqlx::query_scalar("UPDATE shop.chain_cursor SET last_block=start_block-1,last_block_hash=NULL,
        folded_block=NULL,folded_at=NULL,fold_through=NULL,rebuild_requested_at=NULL WHERE indexer='nft' AND network='ethereum' RETURNING last_block")
        .fetch_one(&mut *tx).await.unwrap();
    let cleared: bool = sqlx::query_scalar("SELECT NOT EXISTS(SELECT FROM shop.worker_progress WHERE worker='nft_estate_creations' AND scope=1)")
        .fetch_one(&mut *tx).await.unwrap();
    assert!(
        cleared,
        "the rebuild reset invalidates the marker before the replay"
    );
    sqlx::query("DELETE FROM shop.chain_checkpoint WHERE indexer='nft' AND network='ethereum'")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("DELETE FROM shop.nft_event WHERE network='ethereum' AND kind='land_data' AND to_address IS NOT NULL")
        .execute(&mut *tx).await.unwrap();
    sqlx::query("DELETE FROM shop.land_data WHERE token_address=$1")
        .bind(estate)
        .execute(&mut *tx)
        .await
        .unwrap();
    chain_walk::advance(
        &mut tx,
        ix.indexer(),
        net,
        (start, None),
        &crate::chain_index::rpc::BlockHeader {
            number: j,
            hash: chain.hash_of(j),
            timestamp: time_of(j),
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_ne!(rows().await.unwrap(), before_rebuild);
    assert!(pass(&st, &url, &NftIndex::new(net)).await.published);
    assert_eq!(rows().await.unwrap(), before_rebuild);
    db.drop().await;
}

#[tokio::test]
async fn estate_repair_recovers_after_a_legacy_bounded_rewind_to_the_same_tip() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let net = ChainNet::Ethereum;
    let ix = NftIndex::new(net);
    let k = 6_236_548;
    let h = k + 3;
    let rename = estate_document(h, 0, 2, false, "0,Renamed,");
    let chain = FakeChain::new(
        h + net.confirmations(),
        vec![
            estate_document(k, 0, 1, true, "0,Kept,"),
            estate_document(k + 1, 0, 2, true, "0,Created,"),
            estate_document(k + 2, 0, 3, true, "0,Recovered,"),
            rename.clone(),
        ],
        2_000_000,
        100,
    );
    chain.chain_id.store(1, Ordering::SeqCst);
    let url = serve(chain.clone()).await;
    let st = state_over(db.pool.clone(), &url, index_settings(100, 2_000_000), false);
    assert!(publish_nft(&st, &url, &ix).await.published);
    let rows = || {
        sqlx::query_as::<_, (String, Option<String>, Option<String>, i64, i32)>(
            "SELECT token_id::text, name, description, updated_block, updated_log
             FROM shop.land_data ORDER BY token_id",
        )
        .fetch_all(&db.pool)
    };
    let clean = rows().await.unwrap();
    assert_eq!(clean.len(), 3);
    sqlx::query("DELETE FROM shop.worker_progress WHERE worker='nft_estate_creations' AND scope=1")
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(pass(&st, &url, &ix).await.published);
    let marker = || {
        sqlx::query_scalar::<_, Value>(
            "SELECT to_jsonb(p) FROM shop.worker_progress p
             WHERE worker='nft_estate_creations' AND scope=1",
        )
        .fetch_one(&db.pool)
    };
    let completed = marker().await.unwrap();
    assert_eq!(completed["position"], h);
    assert_eq!(completed["ceiling"], h);

    // The old writer's projection cleanup and cursor rewind commit together.
    // Neither the old cleanup nor its forward filter knows about creations.
    let owner = chain_walk::test_owner(&db.pool, &ix, &IndexSettings::default()).await;
    let mut tx = db.pool.begin().await.unwrap();
    sqlx::query(
        "INSERT INTO shop.chain_checkpoint VALUES ('nft','ethereum',$1,$2,$3)
        ON CONFLICT (indexer,network,block_number) DO NOTHING",
    )
    .bind(k as i64)
    .bind(chain.hash_of(k))
    .bind(time_of(k))
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query("WITH gone AS (DELETE FROM shop.nft_event WHERE network='ethereum' AND block_number>$1 RETURNING contract_address,token_id)
        DELETE FROM shop.land_data d USING gone WHERE (d.token_address,d.token_id)=(gone.contract_address,gone.token_id)")
        .bind(k as i64).execute(&mut *tx).await.unwrap();
    chain_walk::rewind(
        &mut tx,
        ix.indexer(),
        net,
        (h as i64, Some(&chain.hash_of(h))),
        k as i64,
        Some((&chain.hash_of(k), time_of(k))),
    )
    .await
    .unwrap();
    owner.check(&mut tx).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        marker().await.unwrap(),
        completed,
        "a bounded rewind does not fire the rebuild trigger"
    );

    let mut tx = db.pool.begin().await.unwrap();
    ix.write(&mut tx, &[rename], &HashMap::from([(h as i64, time_of(h))]))
        .await
        .unwrap();
    chain_walk::advance(
        &mut tx,
        ix.indexer(),
        net,
        (k as i64, Some(&chain.hash_of(k))),
        &crate::chain_index::rpc::BlockHeader {
            number: h,
            hash: chain.hash_of(h),
            timestamp: time_of(h),
        },
    )
    .await
    .unwrap();
    owner.check(&mut tx).await.unwrap();
    tx.commit().await.unwrap();
    nft_index::fold(&db.pool, net, None, &owner).await.unwrap();
    assert_eq!(marker().await.unwrap(), completed);
    assert_eq!(rows().await.unwrap().len(), 2);
    let reads = chain.get_logs.load(Ordering::SeqCst);
    assert!(pass(&st, &url, &NftIndex::new(net)).await.published);
    assert_eq!(rows().await.unwrap(), clean);
    assert!(
        chain.get_logs.load(Ordering::SeqCst) > reads,
        "the same-tip restart read historical creations"
    );
    // With no retained checkpoint a normal reorg can reach the same
    // hashless start as a rebuild. Its rewind count distinguishes it.
    let owner = chain_walk::test_owner(&db.pool, &ix, &IndexSettings::default()).await;
    let mut tx = db.pool.begin().await.unwrap();
    let start: i64 = sqlx::query_scalar(
        "SELECT start_block-1 FROM shop.chain_cursor
        WHERE indexer='nft' AND network='ethereum'",
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    ix.rewind(&mut tx, start).await.unwrap();
    chain_walk::rewind(
        &mut tx,
        ix.indexer(),
        net,
        (h as i64, Some(&chain.hash_of(h))),
        start,
        None,
    )
    .await
    .unwrap();
    owner.check(&mut tx).await.unwrap();
    tx.commit().await.unwrap();
    let shortened = marker().await.unwrap();
    assert_eq!(shortened["position"], start.max(0));
    assert_eq!(shortened["key_position"][1], "2");
    db.drop().await;
}

#[tokio::test]
async fn a_malformed_parcel_update_clears_the_fields_and_advances_the_position() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let net = ChainNet::Ethereum;
    let ix = NftIndex::new(net);
    let owner = chain_walk::test_owner(&db.pool, &ix, &IndexSettings::default()).await;
    let fields = || {
        sqlx::query_as::<_,(Option<String>,Option<String>,i64,i32)>(
        "SELECT name,description,updated_block,updated_log FROM shop.land_data WHERE token_id=17"
    ).fetch_one(&db.pool)
    };
    for (block, log, raw, since, name, desc) in [
        (
            10,
            1,
            r#"0,"Parcel","Desc""#,
            None,
            Some("Parcel".to_owned()),
            Some("Desc".to_owned()),
        ),
        (11, 2, "bad", Some(10), None, None),
    ] {
        let mut update = estate_document(block, log, 17, false, raw);
        update.address = crate::land_authz::LAND_REGISTRY_MAINNET.into();
        ix.write(
            &mut db.pool.acquire().await.unwrap(),
            &[update],
            &Default::default(),
        )
        .await
        .unwrap();
        nft_index::fold(&db.pool, net, since, &owner).await.unwrap();
        assert_eq!(
            fields().await.unwrap(),
            (name, desc, block as i64, log as i32)
        );
    }
    sqlx::query("TRUNCATE shop.land_data")
        .execute(&db.pool)
        .await
        .unwrap();
    nft_index::fold(&db.pool, net, None, &owner).await.unwrap();
    assert_eq!(fields().await.unwrap(), (None, None, 11, 2));
    db.drop().await;
}

/// After a reorg rewind and the refold it owes, `land_data` equals a
/// rebuild of the remaining estate documents, including the creation data.
#[tokio::test]
async fn estate_updates_preserve_the_last_valid_document_across_folds_and_rewinds() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let index = NftIndex::new(ChainNet::Ethereum);
    let owner = chain_walk::test_owner(&db.pool, &index, &IndexSettings::default()).await;
    let fold = |since| nft_index::fold(&db.pool, ChainNet::Ethereum, since, &owner);
    let rows = || {
        sqlx::query_as::<_, (String, Option<String>, Option<String>, i64, i32)>(
            "SELECT token_id::text, name, description, updated_block, updated_log
             FROM shop.land_data ORDER BY token_id",
        )
        .fetch_all(&db.pool)
    };
    let mut conn = db.pool.acquire().await.unwrap();
    index
        .write(
            &mut conn,
            &[
                estate_document(100, 1, 1, true, r#"0,"Named","Desc""#),
                estate_document(100, 2, 2, true, r#"0,"Second","Estate""#),
            ],
            &Default::default(),
        )
        .await
        .unwrap();
    fold(None).await.unwrap();

    index
        .write(
            &mut conn,
            &[
                estate_document(200, 3, 1, false, "0"),
                estate_document(201, 4, 2, false, r#"0,"","""#),
            ],
            &Default::default(),
        )
        .await
        .unwrap();
    fold(Some(100)).await.unwrap();
    let expected = vec![
        (
            "1".into(),
            Some("Named".into()),
            Some("Desc".into()),
            100,
            1,
        ),
        ("2".into(), Some(String::new()), Some(String::new()), 201, 4),
    ];
    assert_eq!(rows().await.unwrap(), expected);
    sqlx::query("TRUNCATE shop.land_data")
        .execute(&mut *conn)
        .await
        .unwrap();
    fold(None).await.unwrap();
    assert_eq!(rows().await.unwrap(), expected);

    index
        .write(
            &mut conn,
            &[
                estate_document(210, 5, 1, false, r#"0,"Renamed","New description""#),
                estate_document(211, 6, 3, true, "0,Late estate,"),
            ],
            &Default::default(),
        )
        .await
        .unwrap();
    fold(Some(201)).await.unwrap();
    assert_eq!(
        rows().await.unwrap(),
        vec![
            (
                "1".into(),
                Some("Renamed".into()),
                Some("New description".into()),
                210,
                5
            ),
            expected[1].clone(),
            ("3".into(), Some("Late estate".into()), None, 211, 6),
        ]
    );

    index.rewind(&mut conn, 201).await.unwrap();
    drop(conn);
    fold(None).await.unwrap();
    assert_eq!(rows().await.unwrap(), expected);
    db.drop().await;
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
    trade_events::tests::publish_cursors(&db.pool).await;
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
    // The ownership readers over the walked index: the emotes (Polygon
    // only) list CARL's dance, and a profile naming BOB's hat and ALICE's
    // parrot resolves each to its holder's token.
    let (code, body) = get_json(
        &super::router(st.clone()),
        &format!("/lambdas/users/{CARL}/emotes"),
    )
    .await;
    assert_eq!(
        (
            code,
            body["elements"][0]["urn"].as_str(),
            body["elements"][0]["name"].as_str()
        ),
        (
            StatusCode::OK,
            Some("urn:decentraland:matic:collections-v2:0xc2:0"),
            Some("Moonwalk")
        ),
        "{body}"
    );
    let hat = "urn:decentraland:matic:collections-v2:0x00000000000000000000000000000000000000c1:0";
    let parrot =
        "urn:decentraland:matic:collections-v2:0x00000000000000000000000000000000000000c1:1";
    let requested: std::collections::HashSet<(String, String)> = [
        (BOB.to_string(), hat.to_string()),
        (ALICE.to_string(), hat.to_string()),
        (ALICE.to_string(), parrot.to_string()),
    ]
    .into_iter()
    .collect();
    let owned =
        super::owned_items::owned_tokens(&db.pool, &requested, super::owned_items::Rule::Complete)
            .await
            .unwrap();
    assert_eq!(owned[BOB][hat], vec![t1.to_string()]);
    assert_eq!(owned[ALICE][parrot], vec![t2.to_string()]);
    assert!(!owned[ALICE].contains_key(hat), "ALICE sold the hat to BOB");

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
    // The chain reorganizes from s+1,880: the transfer to BOB is gone and
    // one to CARL replaces it. One window walked the whole history, so the
    // only recovery point is the cursor's own: the rewind goes to the start.
    chain.fork(
        s + 1_880,
        0xee,
        vec![transfer(c1, ALICE, CARL, t1, s + 1_880, 0)],
    );
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
/// backfill lane (bracketed by the cursor block) and a reorg recovered at the
/// newest recovery point still on the chain. Pinned so that a change to how
/// the walk reads the chain shows as a diff of requests, not only of
/// outcomes.
#[tokio::test]
async fn the_walk_sends_the_pinned_requests_in_order() {
    pinned_requests_in_order().await;
}

/// The same requests and outcomes with a shadow whose portal never answers:
/// the walk offers its accepted windows and never waits on them.
#[tokio::test]
async fn the_pinned_requests_hold_with_a_stalled_shadow() {
    let (shadow, stop, accepted) = stalled_shadow().await;
    if TEST_SHADOW
        .scope(Some(shadow), pinned_requests_in_order())
        .await
    {
        assert!(
            accepted.load(Ordering::SeqCst) >= 1,
            "an accepted window reached the shadow's portal"
        );
    }
    drop(stop);
}

/// Runs the pinned walk; false when no test database is configured.
async fn pinned_requests_in_order() -> bool {
    let Some(db) = ScratchDb::create().await else {
        return false;
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
    // The chain reorganizes from s+25,000 on: the recovery compares the
    // recovery points below the cursor newest first (s+29,936 is past the
    // fork) and rewinds to s+19,999, the newest still on the chain.
    chain.fork(s + 25_000, 0xee, vec![]);
    outcomes.push(tag(&pass(&st, &url, &NftIndex::new(ChainNet::Matic)).await));
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
    true
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
    "requests 9 logs 0 rows 0 to +19999 paused true published false rewound None",
    "requests 12 logs 1 rows 1 to +29936 paused false published true rewound None",
    "requests 5 logs 0 rows 0 to +29936 paused false published true rewound None",
    "requests 11 logs 0 rows 0 to +30036 paused false published false rewound None",
    "requests 9 logs 0 rows 0 to +30036 paused false published false rewound None",
    "requests 8 logs 0 rows 0 to +30036 paused false published false rewound None",
    "requests 8 logs 0 rows 0 to +30036 paused false published true rewound None",
    "requests 3 logs 0 rows 0 to +30036 paused false published false rewound Some(19999)",
    "requests 10 logs 1 rows 1 to +29999 paused true published false rewound None",
    "requests 7 logs 0 rows 0 to +29999 paused true published false rewound None",
    "requests 6 logs 0 rows 0 to +29999 paused true published false rewound None",
    "requests 11 logs 0 rows 0 to +30036 paused false published true rewound None",
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
    "header s+9999",
    "header s+19999",
    "header s+19999",
    "head",
    "finalized",
    "header s+29936",
    "logs s+20000..s+29936 x3",
    "logs s+20000..s+29936 x2",
    "logs s+20000..s+29936 x1",
    "batch [header s+20100]",
    "header s+19999",
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
    "header s+29936",
    "header s+30036",
    "head",
    "finalized",
    "header s+30036",
    "head",
    "finalized",
    "header s+30036",
    "logs s+0..s+9999 x1",
    "batch [header s+1500]",
    "header s+30036",
    "head",
    "finalized",
    "header s+30036",
    "head",
    "finalized",
    "header s+30036",
    "logs s+10000..s+19999 x1",
    "header s+30036",
    "head",
    "finalized",
    "header s+30036",
    "head",
    "finalized",
    "header s+30036",
    "logs s+20000..s+29936 x1",
    "header s+30036",
    "head",
    "finalized",
    "header s+30036",
    "header s+29936",
    "header s+19999",
    "header s+19999",
    "head",
    "finalized",
    "header s+29999",
    "logs s+20000..s+29999 x4",
    "logs s+20000..s+29999 x2",
    "logs s+20000..s+29999 x2",
    "batch [header s+20100]",
    "header s+19999",
    "header s+29999",
    "header s+29999",
    "head",
    "finalized",
    "header s+29999",
    "logs s+0..s+9999 x1",
    "batch [header s+1500]",
    "header s+29999",
    "header s+29999",
    "head",
    "finalized",
    "header s+29999",
    "logs s+10000..s+19999 x1",
    "header s+29999",
    "header s+29999",
    "head",
    "finalized",
    "header s+30036",
    "logs s+30000..s+30036 x4",
    "logs s+30000..s+30036 x2",
    "logs s+30000..s+30036 x2",
    "header s+29999",
    "header s+30036",
    "head",
    "finalized",
];

/// A rewind deletes the log and the fold back to the start; the refill then
/// takes several passes. Profile ownership (the relaxed rule, no clock)
/// fails on every one of them instead of answering the half-walked table,
/// which would strip items their owners hold: the publish marker stays
/// cleared until a pass reaches the head, folds and publishes.
#[tokio::test]
async fn a_rewind_fails_profile_ownership_until_the_refill_publishes() {
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
        s + 3_000,
        vec![transfer(c1, ZERO, ALICE, t1, s + 100, 0)],
        1_000_000,
        100,
    );
    let url = serve(chain.clone()).await;
    // Four 200-block windows a pass: the refill of 3,000 blocks is several passes.
    let st = state_over(db.pool.clone(), &url, index_settings(4, 200), false);
    assert!(
        publish_nft(&st, &url, &NftIndex::new(ChainNet::Matic))
            .await
            .published
    );
    let urn = format!("urn:decentraland:matic:collections-v2:{c1}:0");
    let requested: std::collections::HashSet<(String, String)> =
        [(ALICE.to_string(), urn.clone())].into_iter().collect();
    let read = || {
        super::owned_items::owned_tokens(&db.pool, &requested, super::owned_items::Rule::Complete)
    };
    assert_eq!(read().await.unwrap()[ALICE][&urn], vec![t1.to_string()]);

    // The chain reorganizes from just past the start block, the transfer
    // kept: no recovery point below the cursor is still on the chain, so the
    // rewind goes back to the start and the refill walks the whole history
    // again on the new branch.
    chain.fork(s + 1, 0xee, vec![transfer(c1, ZERO, ALICE, t1, s + 100, 0)]);
    let rewound = pass(&st, &url, &NftIndex::new(ChainNet::Matic)).await;
    assert!(rewound.reorg_rewound_to.is_some(), "{rewound:?}");
    let mut refill_passes = 0;
    loop {
        let c = chain_walk::cursor(&db.pool, INDEXER_NFT, ChainNet::Matic)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(c.folded_block, None, "no publish mid-refill: {c:?}");
        assert!(
            matches!(
                read().await,
                Err(super::owned_items::ReadError::NotReady(..))
            ),
            "pass {refill_passes}: a profile read fails, never an empty inventory"
        );
        let outcome = pass(&st, &url, &NftIndex::new(ChainNet::Matic)).await;
        refill_passes += 1;
        if outcome.published {
            break;
        }
        assert!(refill_passes < 40, "the refill never published");
    }
    assert!(
        refill_passes > 2,
        "the refill spans passes: {refill_passes}"
    );
    assert_eq!(read().await.unwrap()[ALICE][&urn], vec![t1.to_string()]);
    db.drop().await;
}

/// A pass that takes in a late collection folds while that collection's
/// backfill is still owed. When the head read after that fold fails, the
/// pass ends before its own marker update: the fold's transaction has
/// already cleared the marker, so profile ownership fails instead of
/// answering the collection's tokens transferred since the walk took it in
/// without its earlier ones. The next passes backfill and publish.
#[tokio::test]
async fn a_fold_that_owes_a_backfill_clears_the_marker_when_the_pass_fails_after_it() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let s = nft_index::start_block(ChainNet::Matic);
    let c1 = "0x00000000000000000000000000000000000000c1";
    let late = "0x00000000000000000000000000000000000000c3";
    seed_collection(
        &db.pool,
        "matic",
        c1,
        s + 10,
        "urn:decentraland:matic:collections-v2:0xc1",
        "Pirates",
    )
    .await;
    let (t1, early, recent) = (v2_token(0, 1), v2_token(0, 7), v2_token(0, 8));
    let head = s + 2_000;
    let chain = FakeChain::new(
        head,
        vec![
            transfer(c1, ZERO, ALICE, t1, s + 100, 0),
            transfer(late, ZERO, ALICE, early, s + 500, 0),
            transfer(late, ZERO, ALICE, recent, head + 10, 0),
        ],
        1_000_000,
        100,
    );
    let url = serve(chain.clone()).await;
    let st = state_over(db.pool.clone(), &url, index_settings(64, 1_000), false);
    let ix = NftIndex::new(ChainNet::Matic);
    assert!(publish_nft(&st, &url, &ix).await.published);
    let c1_urn = format!("urn:decentraland:matic:collections-v2:{c1}:0");
    let late_urn = format!("urn:decentraland:matic:collections-v2:{late}:0");
    let requested: std::collections::HashSet<(String, String)> = [
        (ALICE.to_string(), c1_urn.clone()),
        (ALICE.to_string(), late_urn.clone()),
    ]
    .into_iter()
    .collect();
    let read = || {
        super::owned_items::owned_tokens(&db.pool, &requested, super::owned_items::Rule::Complete)
    };
    assert_eq!(read().await.unwrap()[ALICE][&c1_urn], vec![t1.to_string()]);
    let cursor = || async {
        chain_walk::cursor(&db.pool, INDEXER_NFT, ChainNet::Matic)
            .await
            .unwrap()
            .unwrap()
    };
    let published = cursor().await;
    assert!(published.folded_block.is_some(), "{published:?}");

    // The mirror learns the late collection after the walk passed its
    // creation; the pass reads the head once, then the endpoint fails.
    seed_collection(
        &db.pool,
        "matic",
        late,
        s + 400,
        "urn:decentraland:matic:collections-v2:0xc3",
        "Late",
    )
    .await;
    *chain.head.lock() = head + 100;
    chain.head_reads_left.store(1, Ordering::SeqCst);
    let source = chain_walk::RpcSource::new(Rpc::at(&st.http, &url));
    let owner = chain_walk::test_owner(&db.pool, &ix, &st.settings.market.indexes).await;
    let failed = chain_walk::run_pass(
        &db.pool,
        &source,
        &ix,
        &st.settings.market.indexes,
        &owner,
        None,
        &|_| {},
    )
    .await
    .unwrap_err();
    assert!(
        format!("{failed:#}").contains("eth_blockNumber after the fold"),
        "{failed:#}"
    );
    chain.head_reads_left.store(i64::MAX, Ordering::SeqCst);
    let c = cursor().await;
    assert!(
        c.last_block > published.last_block && c.fold_through == Some(c.last_block),
        "the pass walked and folded before it failed: {c:?}"
    );
    assert_eq!(
        owner_of(&db.pool, "matic", late, recent).await,
        Some(Some(ALICE.to_string())),
        "the fold committed the late collection's recent token"
    );
    assert_eq!(owner_of(&db.pool, "matic", late, early).await, None);
    let owed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM shop.nft_contract WHERE contract_address = $1 AND backfill_from_block IS NOT NULL AND backfilled_to_block IS NULL",
    )
    .bind(late)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(owed, 1, "the backfill is still owed");
    assert_eq!(c.folded_block, None, "the fold cleared the marker: {c:?}");
    assert!(
        matches!(
            read().await,
            Err(super::owned_items::ReadError::NotReady(..))
        ),
        "a profile read fails under a fold that owes a backfill"
    );

    assert!(publish_nft(&st, &url, &ix).await.published);
    let owned = read().await.unwrap();
    assert_eq!(owned[ALICE][&c1_urn], vec![t1.to_string()]);
    assert_eq!(
        owned[ALICE][&late_urn],
        vec![recent.to_string(), early.to_string()]
    );
    db.drop().await;
}

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
        format!("('ethereum', 202, 1, 202, 'land_data', '{land}', '{land}', 7, NULL, NULL, NULL, NULL, 'bad')"),
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
    assert_eq!(
        counts.land_data, 4,
        "LAND 1 and 7, Estate 1 and the far token"
    );
    let incremental = rows().await;
    assert_eq!(
        incremental,
        vec![
            (estate.into(), "1".into(), Some("E-new".into()), 151, 0),
            (land.into(), "1".into(), Some("A-last".into()), 200, 3),
            (land.into(), "7".into(), None, 202, 1),
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
    .bind(None::<&str>)
    .bind(crate::land_authz::ESTATE_REGISTRY_MAINNET)
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
    trade_events::tests::publish_cursors(&db.pool).await;
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
                crate::land_authz::events::TOPIC_CREATE_ESTATE.into(),
                topic_addr(CARL),
                topic_word(U256::from(500u32)),
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
    // A v1 token's wearable id is its Issue log's, not a tokenURI.
    let (wearable, issued): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT wearable_id, issued_id::text FROM shop.nft WHERE network = 'ethereum' AND contract_address = $1 AND token_id = 0",
    )
    .bind(v1)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        (wearable.as_deref(), issued.as_deref()),
        (Some("jester_feet"), Some("1"))
    );
    sqlx::query("INSERT INTO shop.chain_cursor(indexer,network,start_block,last_block,folded_block,folded_at,full_history,chain_id)
        SELECT 'rentals',network,start_block,last_block,folded_block,folded_at,true,chain_id
        FROM shop.chain_cursor WHERE indexer='nft' AND network='ethereum'")
        .execute(&db.pool).await.unwrap();
    // A read naming no category also answers the owner's names and LAND,
    // their indexes being published.
    let (code, body) = get_json(&app, &format!("/v1/nfts?owner={ALICE}&network=ETHEREUM")).await;
    let mut held: Vec<(String, String)> = body["data"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .map(|r| {
                    (
                        r["nft"]["category"].as_str().unwrap_or("").to_string(),
                        r["nft"]["name"].as_str().unwrap_or("").to_string(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    held.sort();
    assert_eq!(
        (code, held),
        (
            StatusCode::OK,
            vec![
                ("estate".to_string(), "Fashion Street".to_string()),
                ("wearable".to_string(), "Jester Feet".to_string())
            ]
        ),
        "{body}"
    );
    let (code, body) = get_json(&app, &format!("/v1/nfts?owner={BOB}&network=ETHEREUM")).await;
    assert_eq!(
        (
            code,
            body["data"][0]["nft"]["name"].as_str(),
            body["total"].as_i64()
        ),
        (StatusCode::OK, Some("PixelGolf"), Some(1)),
        "{body}"
    );
    // A collection filter keeps the collections alone.
    let (code, body) = get_json(
        &app,
        &format!("/v1/nfts?owner={ALICE}&network=ETHEREUM&category=wearable"),
    )
    .await;
    assert_eq!(
        (code, body["total"].as_i64()),
        (StatusCode::OK, Some(1)),
        "{body}"
    );
    // The wardrobe reads both networks: Polygon not published yet is the
    // envelope naming it; published, ALICE's v1 copy is listed by the URN
    // its Issue log names.
    let lambdas = super::router(st.clone());
    let wardrobe = format!("/lambdas/users/{ALICE}/wearables");
    let (code, body) = get_json(&lambdas, &wardrobe).await;
    assert_eq!(
        (code, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("nft-index")),
        "{body}"
    );
    assert!(
        body["message"].as_str().unwrap().contains("matic"),
        "{body}"
    );
    super::owned_items::test_support::publish(
        &db.pool,
        &[ChainNet::Matic],
        true,
        std::time::Duration::ZERO,
    )
    .await;
    let (code, body) = get_json(&lambdas, &wardrobe).await;
    assert_eq!(
        (
            code,
            body["elements"][0]["urn"].as_str(),
            body["elements"][0]["individualData"][0]["tokenId"].as_str(),
            body["totalAmount"].as_i64()
        ),
        (
            StatusCode::OK,
            Some("urn:decentraland:ethereum:collections-v1:halloween_2019:jester_feet"),
            Some("0"),
            Some(1)
        ),
        "{body}"
    );
    let requested = [(
        ALICE.to_string(),
        "urn:decentraland:ethereum:collections-v1:halloween_2019:jester_feet".to_string(),
    )]
    .into_iter()
    .collect();
    let owned =
        super::owned_items::owned_tokens(&db.pool, &requested, super::owned_items::Rule::Complete)
            .await
            .unwrap();
    assert_eq!(
        owned[ALICE]["urn:decentraland:ethereum:collections-v1:halloween_2019:jester_feet"],
        vec!["0".to_string()],
        "the profile reader resolves the v1 URN by the Issue log's wearable id"
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
    // The newest recovery point below the fork, which the rewind keeps.
    let kept: i64 = sqlx::query_scalar(
        "SELECT max(block_number) FROM shop.chain_checkpoint WHERE indexer = 'sales' AND network = 'matic' AND block_number < $1",
    )
    .bind((b + 880) as i64)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert!(
        kept > cursor.start_block - 1,
        "a recovery point below the fork"
    );
    // The chain reorganizes from b+880: the acceptance is gone and CARL
    // cancels instead.
    chain.fork(
        b + 880,
        0xee,
        vec![sales_index::tests::bid_cancelled_log(
            c1,
            t1,
            CARL,
            0x22,
            b + 880,
            0,
        )],
    );
    let rewound = pass(&st, &url, &SalesIndex).await;
    assert_eq!(rewound.reorg_rewound_to, Some(kept), "{rewound:?}");
    assert!(
        !sales_index::ready(&db.pool, chrono::Utc::now()).await,
        "a rewind unpublishes"
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

/// A source recording every window the walk accepts.
struct Recording<'a> {
    inner: chain_walk::RpcSource<'a>,
    offers: Mutex<Vec<(u64, u64, String)>>,
}

#[async_trait::async_trait]
impl chain_walk::LogSource for Recording<'_> {
    async fn chain_id(&self) -> Result<chain_walk::Read<u64>, crate::chain_index::rpc::RpcError> {
        self.inner.chain_id().await
    }
    async fn head(&self) -> Result<chain_walk::Read<u64>, crate::chain_index::rpc::RpcError> {
        self.inner.head().await
    }
    async fn finalized(
        &self,
    ) -> Result<
        chain_walk::Read<crate::chain_index::rpc::BlockHeader>,
        crate::chain_index::rpc::RpcError,
    > {
        self.inner.finalized().await
    }
    async fn header(
        &self,
        number: u64,
    ) -> Result<
        chain_walk::Read<crate::chain_index::rpc::BlockHeader>,
        crate::chain_index::rpc::RpcError,
    > {
        self.inner.header(number).await
    }
    async fn window(
        &self,
        from: u64,
        to: u64,
        groups: &[chain_walk::LogGroup],
        chunk: &mut usize,
    ) -> Result<chain_walk::Window, crate::chain_index::rpc::RpcError> {
        self.inner.window(from, to, groups, chunk).await
    }
    async fn block_times(
        &self,
        logs: &[crate::chain_index::rpc::RpcLog],
    ) -> Result<chain_walk::Read<HashMap<i64, i64>>, crate::chain_index::rpc::RpcError> {
        self.inner.block_times(logs).await
    }
    fn accepted(&self, read: &crate::chain_index::portal::compare::AcceptedView<'_>) {
        self.offers
            .lock()
            .push((read.from, read.reached, read.reached_hash.to_string()));
    }
}

/// Only a window whose bracket held is offered: a fork while its logs are
/// read offers nothing, and a clean pass offers each committed window once,
/// contiguous, the last one ending at the cursor's block and hash.
#[tokio::test]
async fn only_a_window_whose_bracket_held_is_offered() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let chain = FakeChain::new(100, vec![], 1000, 1000);
    let url = serve(chain.clone()).await;
    let st = state_over(db.pool.clone(), &url, index_settings(10, 30), false);
    let ix = super::mana_transfers::ManaTransfersIndex {
        net: ChainNet::Matic,
    };
    let source = Recording {
        inner: chain_walk::RpcSource::new(Rpc::at(&st.http, &url)),
        offers: Mutex::new(Vec::new()),
    };
    let owner = chain_walk::test_owner(&db.pool, &ix, &st.settings.market.indexes).await;
    chain.reorg_on_logs.store(true, Ordering::SeqCst);
    let pass = chain_walk::run_pass(
        &db.pool,
        &source,
        &ix,
        &st.settings.market.indexes,
        &owner,
        None,
        &|_| {},
    )
    .await;
    assert!(pass.is_err(), "the bracket caught the fork");
    assert!(
        source.offers.lock().is_empty(),
        "a rejected bracket offers nothing"
    );
    let out = chain_walk::run_pass(
        &db.pool,
        &source,
        &ix,
        &st.settings.market.indexes,
        &owner,
        None,
        &|_| {},
    )
    .await
    .unwrap();
    let offers = source.offers.lock().clone();
    assert!(offers.len() > 1, "{offers:?}");
    for pair in offers.windows(2) {
        assert_eq!(pair[1].0, pair[0].1 + 1, "contiguous: {offers:?}");
    }
    let cur = chain_walk::cursor(&db.pool, chain_walk::INDEXER_MANA, ChainNet::Matic)
        .await
        .unwrap()
        .unwrap();
    let last = offers.last().unwrap();
    assert_eq!(
        (last.1 as i64, Some(last.2.clone())),
        (cur.last_block, cur.last_block_hash)
    );
    assert_eq!(out.indexed_to, cur.last_block);
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

/// `catalyst index` walks rentals on each configured Ethereum chain,
/// unless switched off.
#[tokio::test]
async fn the_rentals_index_attaches_behind_its_switch_and_chain_endpoints() {
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
    let expected = |state: &SharedState, enabled| {
        super::chain_jobs::expected_cursors(state, ChainNet::Ethereum, enabled)
            .into_iter()
            .map(|(index, url)| (index.indexer(), url.is_some()))
            .collect::<Vec<_>>()
    };
    assert!(expected(&no_rpc, None).is_empty());
    assert_eq!(expected(&no_rpc, Some(true)), [("rentals", false)]);
    assert!(
        super::chain_jobs::projections(&with_rpc, ChainNet::Matic, None)
            .iter()
            .all(|p| !p.lease.starts_with("rentals-index")),
        "the rentals venue is mainnet's"
    );
    let mut with_sepolia = with_rpc;
    Arc::get_mut(&mut with_sepolia)
        .unwrap()
        .settings
        .market
        .trade_rpc_urls = RpcEndpoints::from_env(Some(
        "1=https://rpc.example.org/,11155111=https://sepolia.example.org/",
    ));
    assert_eq!(
        expected(&with_sepolia, None),
        [("rentals", true), ("rentals_sepolia", true)]
    );
    assert!(expected(&with_sepolia, Some(false)).is_empty());
    let both = super::chain_jobs::projections(&with_sepolia, ChainNet::Ethereum, None);
    let sepolia = both
        .iter()
        .find(|projection| projection.lease == "rentals-index-sepolia-ethereum")
        .unwrap();
    assert_eq!(sepolia.url, "https://sepolia.example.org/");
    let index = (sepolia.make)();
    assert_eq!(index.indexer(), "rentals_sepolia");
    assert_eq!(index.chain_id(), 11_155_111);
    assert_eq!(index.start_block(), 3_831_254);
    let mainnet = both
        .iter()
        .find(|projection| projection.lease == "rentals-index-ethereum")
        .unwrap();
    let longest_retry = mainnet.cfg.wait_after_failure(u32::MAX);
    assert!(longest_retry <= std::time::Duration::from_secs(30));
    assert!(longest_retry < chain_walk::READY_WINDOW.to_std().unwrap());
    assert!(
        super::chain_jobs::projections(&with_sepolia, ChainNet::Ethereum, Some(false))
            .iter()
            .all(|projection| !projection.lease.starts_with("rentals-index"))
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
        /// While `alone_armed`, `walks_alone` waits for `alone_gate` once the
        /// projection's cursor stands at `alone_from` or past it (counting in
        /// `alone_held`): a read before that commit answers.
        alone_gate: Option<Arc<tokio::sync::Semaphore>>,
        alone_armed: Arc<std::sync::atomic::AtomicBool>,
        alone_from: Arc<std::sync::atomic::AtomicI64>,
        alone_held: Arc<AtomicUsize>,
        /// Once set, `walks_alone` answers yes.
        alone_answer: Arc<std::sync::atomic::AtomicBool>,
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
            if let Some(gate) = &self.alone_gate
                && self.alone_armed.load(Ordering::SeqCst)
            {
                let at = chain_walk::cursor(pool, self.indexer(), self.net())
                    .await?
                    .map_or(-1, |c| c.last_block);
                if at >= self.alone_from.load(Ordering::SeqCst) {
                    self.alone_held.fetch_add(1, Ordering::SeqCst);
                    gate.acquire().await.unwrap().forget();
                }
            }
            if self.alone_answer.load(Ordering::SeqCst) {
                return Ok(true);
            }
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
            shadow: None,
            portal: test_portal(),
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
        windows_once(false).await;
    }

    /// The same shared walk with a portal whose coverage lists every address
    /// both read: the same rows as the two RPC walks alone, and no
    /// eth_getLogs.
    #[tokio::test]
    async fn a_shared_walk_from_the_portal_writes_the_rows_of_two_rpc_walks() {
        windows_once(true).await;
    }

    /// The same requests and rows with a shadow whose portal never answers:
    /// the walks alone and the shared walk offer their accepted windows and
    /// never wait on them.
    #[tokio::test]
    async fn the_shared_trace_holds_with_a_stalled_shadow() {
        let (shadow, stop, accepted) = super::stalled_shadow().await;
        if super::TEST_SHADOW
            .scope(Some(shadow), windows_once(false))
            .await
        {
            assert!(accepted.load(Ordering::SeqCst) >= 1);
        }
        drop(stop);
    }

    /// Runs the comparison, the shared walk reading from a portal that covers
    /// every address when `portal`; false when no test database is
    /// configured.
    async fn windows_once(portal: bool) -> bool {
        let (Some(alone), Some(together)) = (ScratchDb::create().await, ScratchDb::create().await)
        else {
            return false;
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
        let admission = if portal {
            // Each address once, as a coverage answer lists it.
            let mut every = std::collections::BTreeMap::new();
            let nft = NftIndex::new(ChainNet::Matic);
            for ix in [&nft as &dyn ChainIndex, &SalesIndex] {
                let anchor = (s + 498, time_of(s + 498));
                for g in ix
                    .groups(&together.pool, s + 499, s + 1_498, anchor)
                    .await
                    .unwrap()
                {
                    every.extend(g.addresses.into_iter().map(|a| (a.to_ascii_lowercase(), 0)));
                }
            }
            let (origin, _) = super::compare_sources_tests::covering_portal(
                chain.clone(),
                every.into_iter().collect(),
            )
            .await;
            Some(Arc::new(crate::chain_index::portal::admit::Admission::new(
                crate::chain_index::portal::PortalReader::new(
                    &origin,
                    "polygon-mainnet",
                    None,
                    1_000.0,
                )
                .unwrap(),
                "matic",
                Duration::from_secs(5),
            )))
        } else {
            super::test_portal()
        };
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
            super::test_shadow().as_deref(),
            admission.as_deref(),
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
        if portal {
            assert!(
                together_calls.iter().all(|c| !c.starts_with("logs")),
                "{together_calls:#?}"
            );
            for table in PROJECTION_TABLES {
                assert_eq!(
                    table_rows(&together.pool, table).await,
                    table_rows(&alone.pool, table).await,
                    "{table}"
                );
            }
            alone.drop().await;
            together.drop().await;
            return true;
        }
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
            // The checkpoint both extend, read in the window's bracket.
            format!("header {}", s + 498),
            format!("header {}", s + 1_498),
            // Each close reads the heads again after its fold.
            "head".into(),
            "finalized".into(),
            "head".into(),
            "finalized".into(),
        ]
        .into();
        assert_eq!(together_calls, expected, "{together_calls:#?}");
        assert_eq!(sent, 9, "the shared walk's own requests");
        assert_eq!(alone_calls.len(), 19, "{alone_calls:#?}");
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
        true
    }

    /// The touch each member's first walk under its generation makes runs
    /// beside its companions', under the member deadline: with one member's
    /// cursor row locked while its touch is due, that member leaves the walk
    /// at the deadline, and its companion walks its window and commits while
    /// the lock is still held.
    #[tokio::test]
    async fn a_locked_member_touch_holds_back_no_companion() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let chain = FakeChain::new(100, vec![], 10_000, 1_000);
        chain.finalized_lag.store(2, Ordering::SeqCst);
        let url = serve(chain.clone()).await;
        let settings = IndexSettings {
            window_blocks: 100,
            requests_per_pass: 100,
            ..walk_settings()
        };
        let st = state_over(db.pool.clone(), &url, settings.clone(), false);
        let mana: Arc<dyn ChainIndex> =
            Arc::new(super::super::mana_transfers::ManaTransfersIndex {
                net: ChainNet::Matic,
            });
        let trades: Arc<dyn ChainIndex> = Arc::new(TradeEventsIndex {
            net: ChainNet::Matic,
            chain_id: 137,
        });
        for ix in [&mana, &trades] {
            assert!(publish(&st, &url, ix.as_ref()).await.published);
        }
        let at = chain_group::Checkpoint::of(
            &cursor(&db.pool, chain_walk::INDEXER_MANA, ChainNet::Matic)
                .await
                .unwrap(),
        )
        .unwrap();
        *chain.head.lock() = 200;
        // Fresh holdings, claimed before the lock: each member's first walk
        // under its new generation owes its touch.
        let members: Vec<chain_group::Member> = [trades.clone(), mana.clone()]
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
        for m in &members {
            m.holdings
                .owner(&db.pool, None, m.index.as_ref(), &settings)
                .await
                .unwrap();
        }
        let mut lock = db.pool.begin().await.unwrap();
        sqlx::query("SELECT 1 FROM shop.chain_cursor WHERE indexer = 'trade_events' AND network = 'matic' FOR UPDATE")
            .execute(&mut *lock)
            .await
            .unwrap();
        let counted = AtomicUsize::new(0);
        let started = std::time::Instant::now();
        let ends = tokio::time::timeout(
            Duration::from_secs(5),
            chain_group::walk_together(
                &db.pool,
                &st.http,
                &url,
                &settings,
                members,
                &at,
                Duration::from_millis(800),
                &counted,
                None,
                super::test_portal().as_deref(),
            ),
        )
        .await
        .expect("the walk ends while the lock is held");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "{:?}",
            started.elapsed()
        );
        let mut trades_end = None;
        for (slot, end) in ends {
            match (slot, end) {
                (0, end) => trades_end = Some(end),
                (1, chain_group::MemberEnd::Walked { out, .. }) => {
                    assert_eq!(out.indexed_to, 198, "{out:?}")
                }
                (slot, end) => panic!("member {slot}: {}", describe(&end)),
            }
        }
        assert!(
            matches!(
                trades_end,
                Some(chain_group::MemberEnd::Failed { tainted: true, .. })
            ),
            "the locked member left at the deadline"
        );
        // The companion committed while the lock is still held.
        assert_eq!(last_block(&db.pool, chain_walk::INDEXER_MANA).await, 198);
        assert_eq!(last_block(&db.pool, INDEXER_TRADE_EVENTS).await, 98);
        lock.rollback().await.unwrap();
        db.drop().await;
    }

    fn describe(end: &chain_group::MemberEnd) -> String {
        match end {
            chain_group::MemberEnd::Failed { error, tainted } => {
                format!("failed ({tainted}): {error:#}")
            }
            chain_group::MemberEnd::Walked { .. } => "walked".into(),
            _ => "another end".into(),
        }
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
        // Only sales' lease is meant to go, and on `second`, whose short
        // keepalive notices it. The walk and nft's lease are on `first`: a
        // lease statement that outlasts 200 ms on a loaded runner would end
        // nft's acquisition too, and sales would walk the window alone.
        let first = Jobs::with_leases(
            db.pool.clone(),
            LeaseSettings {
                keepalive: Duration::from_secs(30),
                ..lease_settings(&namespace, Duration::from_millis(100))
            },
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
        // As in `a_commit_past_the_deadline_recovers_from_wherever_the_cursor_is`:
        // on a short cadence one projection's close can outlast the other's
        // interval on a slow machine, and sales then walks the new window
        // alone. On this one each stands idle when the other comes due.
        for p in &mut projections {
            p.cfg = PeriodicCfg::every(Duration::from_secs(1))
                .with_error_backoff(Duration::from_millis(400));
        }
        let sup = supervisor(&st, ChainNet::Matic, projections);
        let stats = sup.stats.clone();
        chain_jobs::spawn_with(&first, sup, Some(leases));
        for indexer in [INDEXER_NFT, INDEXER_SALES] {
            eventually("each walked alone to the head", || async {
                published(&db.pool, indexer, ChainNet::Matic).await
            })
            .await;
        }
        let walks = stats.walks.load(Ordering::SeqCst);
        eventually("nft and sales walk together", || async {
            stats.walks.load(Ordering::SeqCst) > walks
        })
        .await;
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
        // Only a shared walk holds a member to its deadline: sales walking
        // the window alone would wait in its commit for good. On a short
        // cadence, one projection's close can outlast the other's interval
        // on a slow machine, and the other then walks alone. On this one,
        // each stands idle when the other comes due, and the two walk each
        // window together.
        for p in &mut projections {
            p.cfg = PeriodicCfg::every(Duration::from_secs(1))
                .with_error_backoff(Duration::from_millis(400));
        }
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
            let walks = stats.walks.load(Ordering::SeqCst);
            eventually("nft and sales walk together again", || async {
                stats.walks.load(Ordering::SeqCst) > walks
            })
            .await;
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
                    None,
                    super::test_portal().as_deref(),
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
            None,
            super::test_portal().as_deref(),
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

    /// The walks-alone hint after a shared walk is each member's own: nft's
    /// read at the walk's opening answers, the one after it committed (in
    /// its close) is held, and sales walks, closes and publishes meanwhile.
    #[tokio::test]
    async fn a_held_walks_alone_read_holds_back_no_companion() {
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
            alone_gate: Some(Arc::new(tokio::sync::Semaphore::new(0))),
            ..Hooked::default()
        };
        let mut projections = nft_and_sales(&url);
        projections[0].make = {
            let (gate, armed, from, held) = (
                hooks.alone_gate.clone(),
                hooks.alone_armed.clone(),
                hooks.alone_from.clone(),
                hooks.alone_held.clone(),
            );
            Arc::new(move || {
                Arc::new(Hooked {
                    inner: Some(Arc::new(NftIndex::new(ChainNet::Matic))),
                    alone_gate: gate.clone(),
                    alone_armed: armed.clone(),
                    alone_from: from.clone(),
                    alone_held: held.clone(),
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
        // Reads before nft commits the new window answer (the walk's opening);
        // the one after it, in nft's close, waits.
        let target = (s + 1_498) as i64;
        hooks.alone_from.store(target, Ordering::SeqCst);
        hooks.alone_armed.store(true, Ordering::SeqCst);
        add_sale(&chain, c1, v2_token(0, 2), s + 700);
        *chain.head.lock() = s + 1_500;
        eventually("nft's read after its close is held", || async {
            hooks.alone_held.load(Ordering::SeqCst) >= 1
        })
        .await;
        eventually("sales walks, closes and publishes meanwhile", || async {
            last_block(&db.pool, INDEXER_SALES).await == target
                && published(&db.pool, INDEXER_SALES, ChainNet::Matic).await
        })
        .await;
        assert_eq!(
            last_block(&db.pool, INDEXER_NFT).await,
            target,
            "the walk was shared"
        );
        hooks.alone_armed.store(false, Ordering::SeqCst);
        hooks.alone_gate.as_ref().unwrap().add_permits(100);
        assert!(jobs.shutdown(Duration::from_secs(5)).await.is_empty());
        db.drop().await;
    }

    /// A member a shared walk pauses at its budget reads its walks-alone
    /// hint in a task of its own. A and C, which share an address, walk
    /// together from 50 (a read that mixes their topics shows it) and pause
    /// at 70; their hints are held while B walks on to the target, closes
    /// and publishes. Then A's hint answers yes and C's no, and no shared
    /// walk starts until both pause again at 90: A walks alone. Had A's
    /// answer been lost, the two would gather into a walk that A leaves at
    /// its opening, but only when both come due on the same wake-up, which
    /// is usual, not certain; the answer itself is pinned by
    /// `chain_jobs::tests::a_paused_members_hint_decides_whether_it_walks_alone_next`.
    #[tokio::test]
    async fn a_held_hint_after_a_budget_pause_holds_back_no_companion() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let (a1, b1, c1, shared) = (
            "0x00000000000000000000000000000000000000a1",
            "0x00000000000000000000000000000000000000b1",
            "0x00000000000000000000000000000000000000c1",
            "0x00000000000000000000000000000000000000ab",
        );
        let (topic_a, topic_b, topic_c) = (
            super::super::mana_transfers::TRANSFER,
            "0x00000000000000000000000000000000000000000000000000000000000000b1",
            "0x00000000000000000000000000000000000000000000000000000000000000c1",
        );
        let chain = FakeChain::new(52, vec![], 1_000, 1_000);
        chain.finalized_lag.store(2, Ordering::SeqCst);
        *chain.watch_topics.lock() = vec![topic_a.to_string(), topic_c.to_string()];
        let url = serve(chain.clone()).await;
        // A and C owe two requests a window, B one; three a pass: from 50 to
        // 98 in windows of 20, B walks all three, A and C pause after the first.
        let settings = IndexSettings {
            address_chunk: 1,
            window_blocks: 20,
            requests_per_pass: 3,
            ..walk_settings()
        };
        let st = state_over(db.pool.clone(), &url, settings, false);
        let gate = Arc::new(tokio::sync::Semaphore::new(0));
        let (armed, from, held) = (
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
            Arc::new(std::sync::atomic::AtomicI64::new(i64::MAX)),
            Arc::new(AtomicUsize::new(0)),
        );
        let (a_answer, c_answer) = (
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
        let (fold_gate, fold_armed, folding) = (
            Arc::new(tokio::sync::Semaphore::new(0)),
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
            Arc::new(AtomicUsize::new(0)),
        );
        let hooked = |inner: Arc<dyn ChainIndex>,
                      groups: Vec<chain_walk::LogGroup>,
                      answer: Option<Arc<std::sync::atomic::AtomicBool>>|
         -> MakeIndex {
            let (gate, armed, from, held) =
                (gate.clone(), armed.clone(), from.clone(), held.clone());
            let (fold_gate, fold_armed, folding) =
                (fold_gate.clone(), fold_armed.clone(), folding.clone());
            Arc::new(move || {
                let mut h = Hooked {
                    inner: Some(inner.clone()),
                    groups_override: Some(groups.clone()),
                    armed_fold_gate: Some(fold_gate.clone()),
                    armed: fold_armed.clone(),
                    entered: folding.clone(),
                    ..Hooked::default()
                };
                if let Some(answer) = &answer {
                    h.alone_gate = Some(gate.clone());
                    h.alone_armed = armed.clone();
                    h.alone_from = from.clone();
                    h.alone_held = held.clone();
                    h.alone_answer = answer.clone();
                }
                Arc::new(h) as Arc<dyn ChainIndex>
            })
        };
        let group = |addresses: &[&str], topic: &str| {
            vec![chain_walk::LogGroup {
                addresses: addresses.iter().map(|a| a.to_string()).collect(),
                topics: json!([[topic]]),
            }]
        };
        let a = hooked(
            Arc::new(super::super::mana_transfers::ManaTransfersIndex {
                net: ChainNet::Matic,
            }),
            group(&[a1, shared], topic_a),
            Some(a_answer.clone()),
        );
        let b = hooked(
            Arc::new(TradeEventsIndex {
                net: ChainNet::Matic,
                chain_id: 137,
            }),
            group(&[b1], topic_b),
            None,
        );
        let c = hooked(
            Arc::new(super::super::committee_index::CommitteeIndex),
            group(&[c1, shared], topic_c),
            Some(c_answer.clone()),
        );
        // Half a second between turns: far more than the three take to
        // settle once let out of their closes together (below).
        let paced = |job: &str, make: MakeIndex| Projection {
            cfg: PeriodicCfg::every(Duration::from_millis(500))
                .with_error_backoff(Duration::from_millis(400)),
            ..projection(job, ChainNet::Matic, &url, make)
        };
        let sup = supervisor(
            &st,
            ChainNet::Matic,
            vec![
                paced("trade-events-index", b),
                paced("committee-index", c),
                paced("mana-transfers-index", a),
            ],
        );
        let stats = sup.stats.clone();
        let jobs = Jobs::new().with_first_pass_now(true);
        chain_jobs::spawn(&jobs, sup);
        let (mana, committee) = (chain_walk::INDEXER_MANA, chain_walk::INDEXER_COMMITTEE);
        for indexer in [mana, INDEXER_TRADE_EVENTS, committee] {
            eventually("each walked to 50", || async {
                last_block(&db.pool, indexer).await == 50
                    && published(&db.pool, indexer, ChainNet::Matic).await
            })
            .await;
        }
        // Hold the three in their closes at once, so no walk is in flight;
        // move the head; then let them out together. They settle within
        // moments of each other, well inside the 500 ms before the first is
        // due again, so the walk from 50 gathers all three.
        fold_armed.store(true, Ordering::SeqCst);
        eventually("the three held in their closes at once", || async {
            folding.load(Ordering::SeqCst) >= 3
        })
        .await;
        // The reads after the closes and at the shared walk's opening
        // (cursors at 50) answer; the ones after the pause at 70 wait.
        from.store(70, Ordering::SeqCst);
        armed.store(true, Ordering::SeqCst);
        let mixed = chain.watched.load(Ordering::SeqCst);
        *chain.head.lock() = 100;
        fold_armed.store(false, Ordering::SeqCst);
        fold_gate.add_permits(3);
        eventually("A's and C's hints after their pause are held", || async {
            held.load(Ordering::SeqCst) >= 2
        })
        .await;
        for indexer in [mana, committee] {
            assert_eq!(
                last_block(&db.pool, indexer).await,
                70,
                "{indexer} paused after one window"
            );
        }
        assert!(
            chain.watched.load(Ordering::SeqCst) > mixed,
            "the walk from 50 was shared"
        );
        eventually(
            "B walks to the target, closes and publishes meanwhile",
            || async {
                last_block(&db.pool, INDEXER_TRADE_EVENTS).await == 98
                    && published(&db.pool, INDEXER_TRADE_EVENTS, ChainNet::Matic).await
            },
        )
        .await;
        // A's hint answers yes, C's no. The reads after the next pause, at
        // 90, wait: until then, no shared walk starts.
        from.store(90, Ordering::SeqCst);
        a_answer.store(true, Ordering::SeqCst);
        let (walks, mixed) = (
            stats.walks.load(Ordering::SeqCst),
            chain.watched.load(Ordering::SeqCst),
        );
        gate.add_permits(2);
        eventually(
            "A's and C's hints after their pause at 90 are held",
            || async { held.load(Ordering::SeqCst) >= 4 },
        )
        .await;
        for indexer in [mana, committee] {
            assert_eq!(
                last_block(&db.pool, indexer).await,
                90,
                "{indexer} paused after one more window"
            );
        }
        assert_eq!(
            stats.walks.load(Ordering::SeqCst),
            walks,
            "A walked alone and C had no companion: no shared walk started"
        );
        assert_eq!(
            chain.watched.load(Ordering::SeqCst),
            mixed,
            "no read mixed A's and C's topics again"
        );
        armed.store(false, Ordering::SeqCst);
        gate.add_permits(100);
        for indexer in [mana, committee] {
            eventually("A and C go on to the target", || async {
                last_block(&db.pool, indexer).await == 98
            })
            .await;
        }
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

    /// Two projections, MANA and trade events, walked together from 0 in
    /// windows of 100 and published at 998, their closes held on a shared
    /// gate while armed; half a second between turns, so the three settle
    /// together once their closes are let out.
    struct Paired {
        chain: Arc<FakeChain>,
        jobs: Jobs,
        stats: Arc<chain_jobs::SharedStats>,
        fold_gate: Arc<tokio::sync::Semaphore>,
        fold_armed: Arc<std::sync::atomic::AtomicBool>,
        folding: Arc<AtomicUsize>,
    }

    async fn paired(
        db: &ScratchDb,
        logs: Vec<crate::chain_index::rpc::RpcLog>,
        deadline: Duration,
    ) -> Paired {
        let chain = FakeChain::new(1_000, logs, 10_000, 1_000);
        chain.finalized_lag.store(2, Ordering::SeqCst);
        let url = serve(chain.clone()).await;
        let settings = IndexSettings {
            window_blocks: 100,
            requests_per_pass: 100,
            ..walk_settings()
        };
        let st = state_over(db.pool.clone(), &url, settings, false);
        let (fold_gate, fold_armed, folding) = (
            Arc::new(tokio::sync::Semaphore::new(0)),
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
            Arc::new(AtomicUsize::new(0)),
        );
        let hooked = |inner: Arc<dyn ChainIndex>| -> MakeIndex {
            let (fold_gate, fold_armed, folding) =
                (fold_gate.clone(), fold_armed.clone(), folding.clone());
            Arc::new(move || {
                Arc::new(Hooked {
                    inner: Some(inner.clone()),
                    armed_fold_gate: Some(fold_gate.clone()),
                    armed: fold_armed.clone(),
                    entered: folding.clone(),
                    ..Hooked::default()
                }) as Arc<dyn ChainIndex>
            })
        };
        let paced = |job: &str, make: MakeIndex| Projection {
            cfg: PeriodicCfg::every(Duration::from_millis(500))
                .with_error_backoff(Duration::from_millis(400)),
            ..projection(job, ChainNet::Matic, &url, make)
        };
        let mut sup = supervisor(
            &st,
            ChainNet::Matic,
            vec![
                paced(
                    "trade-events-index",
                    hooked(Arc::new(TradeEventsIndex {
                        net: ChainNet::Matic,
                        chain_id: 137,
                    })),
                ),
                paced(
                    "mana-transfers-index",
                    hooked(Arc::new(super::super::mana_transfers::ManaTransfersIndex {
                        net: ChainNet::Matic,
                    })),
                ),
            ],
        );
        sup.member_deadline = deadline;
        let stats = sup.stats.clone();
        let jobs = Jobs::new().with_first_pass_now(true);
        chain_jobs::spawn(&jobs, sup);
        for indexer in [INDEXER_TRADE_EVENTS, chain_walk::INDEXER_MANA] {
            eventually("each published at 998", || async {
                last_block(&db.pool, indexer).await == 998
                    && published(&db.pool, indexer, ChainNet::Matic).await
            })
            .await;
        }
        Paired {
            chain,
            jobs,
            stats,
            fold_gate,
            fold_armed,
            folding,
        }
    }

    impl Paired {
        /// Holds both in their closes at once (no walk in flight), runs
        /// `meanwhile`, then lets them out together.
        async fn between_turns(&self, meanwhile: impl FnOnce(&FakeChain)) {
            self.folding.store(0, Ordering::SeqCst);
            self.fold_armed.store(true, Ordering::SeqCst);
            eventually("both held in their closes", || async {
                self.folding.load(Ordering::SeqCst) >= 2
            })
            .await;
            meanwhile(&self.chain);
            self.fold_armed.store(false, Ordering::SeqCst);
            self.fold_gate.add_permits(2);
        }
    }

    async fn rewinds(pool: &PgPool, indexer: &str) -> i32 {
        sqlx::query_scalar(
            "SELECT reorg_rewinds FROM shop.chain_cursor WHERE indexer = $1 AND network = 'matic'",
        )
        .bind(indexer)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    fn mana_at(block: u64, amount: u64) -> crate::chain_index::rpc::RpcLog {
        let mana = super::super::mana_transfers::contract(ChainNet::Matic);
        crate::chain_index::rpc::RpcLog {
            address: mana,
            topics: vec![
                super::super::mana_transfers::TRANSFER.into(),
                format!("0x{:0>64}", &ALICE[2..]),
                format!("0x{:0>64}", &BOB[2..]),
            ],
            data: format!("0x{amount:064x}"),
            block_number: format!("0x{block:x}"),
            log_index: "0x0".into(),
            block_timestamp: Some(format!("0x{:x}", time_of(block))),
            block_hash: None,
            transaction_hash: Some(format!("0x{:064x}", block * 1_000 + amount)),
        }
    }

    /// A shared window whose bracket stands on the new branch while the
    /// checkpoint both members extend is not on it: evidence about both
    /// cursors, not a failed read. Nothing is committed; the shared walk
    /// itself withdraws each one's publication and leaves each to recover
    /// in a pass alone (no fallback): while those passes wait on their first
    /// read, readers of both already fall back and nothing moved. Then both
    /// rewind below the fork, and MANA ends with the new branch's transfer.
    #[tokio::test]
    async fn a_shared_window_extending_a_checkpoint_on_another_branch_recovers_each_alone() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let p = paired(&db, vec![mana_at(960, 1)], Duration::from_secs(60)).await;
        let gate = Arc::new(tokio::sync::Semaphore::new(0));
        p.between_turns(|chain| {
            *chain.head.lock() = 1_100;
            // The fork comes once the shared walk's own check at 998 answered.
            *chain.fork_after.lock() = Some(ForkAfter {
                header: 998,
                reads: 1,
                from: 950,
                salt: 0xee,
                logs: vec![log_json(&mana_at(970, 2))],
                one_read: false,
            });
            // After the walk's second read at 998 (the window's predecessor),
            // every read at or below 998 waits: the passes alone that follow
            // wait on their own check, before anything of theirs withdraws.
            *chain.gate_after.lock() = Some((998, 2, 999, gate.clone()));
        })
        .await;
        eventually("both passes alone wait on their first read", || async {
            p.chain.probes_waiting.load(Ordering::SeqCst) >= 2
        })
        .await;
        for indexer in [INDEXER_TRADE_EVENTS, chain_walk::INDEXER_MANA] {
            assert!(
                !published(&db.pool, indexer, ChainNet::Matic).await,
                "{indexer} withdrawn"
            );
            assert_eq!(
                last_block(&db.pool, indexer).await,
                998,
                "{indexer} not moved"
            );
        }
        assert_eq!(p.stats.fallbacks.load(Ordering::SeqCst), 0, "no fallback");
        gate.add_permits(100);
        for indexer in [INDEXER_TRADE_EVENTS, chain_walk::INDEXER_MANA] {
            eventually("each rewound once and published again", || async {
                rewinds(&db.pool, indexer).await == 1
                    && last_block(&db.pool, indexer).await == 1_098
                    && published(&db.pool, indexer, ChainNet::Matic).await
            })
            .await;
        }
        let blocks: Vec<i64> =
            sqlx::query_scalar("SELECT block_number FROM shop.mana_transfer ORDER BY 1")
                .fetch_all(&db.pool)
                .await
                .unwrap();
        assert_eq!(
            blocks,
            vec![970],
            "the new branch's transfer, not the old one's"
        );
        assert!(p.jobs.shutdown(Duration::from_secs(5)).await.is_empty());
        db.drop().await;
    }

    /// A shared walk whose checkpoint is on another branch leaves the search
    /// to each member's pass alone, which runs under no member deadline: its
    /// slow older-header reads, each well within its own time but all of
    /// them past the 300 ms a member's step may take, still end in one
    /// search. Each recovery point is read once per projection, apart from
    /// the reads at the tip.
    #[tokio::test]
    async fn a_slow_search_after_a_shared_opening_runs_once_alone() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let p = paired(&db, vec![], Duration::from_millis(300)).await;
        let mark = p.chain.calls.lock().len();
        p.between_turns(|chain| {
            chain.fork(550, 0xee, vec![]);
            *chain.probe_delay.lock() = Some((998, Duration::from_millis(150)));
        })
        .await;
        // Read the moment both rewound: each one's next walk (which reads
        // those heights again as window ends) waits half a second after it.
        let mut calls = Vec::new();
        for _ in 0..2_000 {
            let done = rewinds(&db.pool, INDEXER_TRADE_EVENTS).await == 1
                && rewinds(&db.pool, chain_walk::INDEXER_MANA).await == 1;
            if done {
                calls = p.chain.calls.lock()[mark..].to_vec();
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(!calls.is_empty(), "both rewound");
        for indexer in [INDEXER_TRADE_EVENTS, chain_walk::INDEXER_MANA] {
            let c = cursor(&db.pool, indexer, ChainNet::Matic).await.unwrap();
            assert!(c.last_block >= 499, "{indexer} at {}", c.last_block);
        }
        for block in [899, 799, 699, 599, 499] {
            let reads = calls
                .iter()
                .filter(|c| **c == format!("header {block}"))
                .count();
            assert_eq!(reads, 2, "header {block}: once per projection");
        }
        assert!(!calls.contains(&"header 399".to_string()));
        assert!(p.jobs.shutdown(Duration::from_secs(5)).await.is_empty());
        db.drop().await;
    }
}

/// The bounded recovery from a reorg past the confirmation depth: the
/// recovery points each advance records, their search newest first, the
/// publication withdrawn before any older header is read, and the bracket
/// that makes a committed point one to recover at.
mod recovery {
    use super::super::chain_walk::{INDEXER_MANA, cursor};
    use super::super::mana_transfers::{self, ManaTransfersIndex};
    use super::*;
    use crate::chain_index::rpc::RpcLog;
    use std::time::Duration;

    const MANA: ManaTransfersIndex = ManaTransfersIndex {
        net: ChainNet::Matic,
    };

    /// A MANA transfer at `block`; `amount` tells two apart at one block.
    fn mana_log(block: u64, from: &str, to: &str, amount: u64) -> RpcLog {
        RpcLog {
            address: mana_transfers::contract(ChainNet::Matic),
            topics: vec![
                mana_transfers::TRANSFER.into(),
                format!("0x{:0>64}", &from[2..]),
                format!("0x{:0>64}", &to[2..]),
            ],
            data: format!("0x{amount:064x}"),
            block_number: format!("0x{block:x}"),
            log_index: "0x0".into(),
            block_timestamp: Some(format!("0x{:x}", time_of(block))),
            block_hash: None,
            transaction_hash: Some(format!("0x{:064x}", block * 1_000 + amount)),
        }
    }

    async fn rows(pool: &PgPool, table: &str) -> Vec<String> {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT (to_jsonb(t) - 'updated_at' - 'created_at' - 'id' - 'indexed_at' - 'mirrored_at' - 'added_at')::text \
             FROM {table} t ORDER BY 1"
        )))
        .fetch_all(pool)
        .await
        .unwrap()
    }

    async fn points(pool: &PgPool, indexer: &str) -> Vec<i64> {
        sqlx::query_scalar(
            "SELECT block_number FROM shop.chain_checkpoint WHERE indexer = $1 AND network = 'matic' ORDER BY 1",
        )
        .bind(indexer)
        .fetch_all(pool)
        .await
        .unwrap()
    }

    async fn at(pool: &PgPool, indexer: &str) -> (i64, Option<String>) {
        let c = cursor(pool, indexer, ChainNet::Matic)
            .await
            .unwrap()
            .unwrap();
        (c.last_block, c.last_block_hash)
    }

    async fn ready(pool: &PgPool, indexer: &str) -> bool {
        chain_walk::ready_cursor(pool, indexer, ChainNet::Matic, chrono::Utc::now())
            .await
            .is_some()
    }

    /// A chain of `head` blocks, finalized two behind, carrying `logs`.
    async fn chain_of(head: u64, logs: Vec<RpcLog>, max_span: u64) -> (Arc<FakeChain>, String) {
        let chain = FakeChain::new(head, logs, max_span, 1_000);
        chain.finalized_lag.store(2, Ordering::SeqCst);
        let url = serve(chain.clone()).await;
        (chain, url)
    }

    /// What one walk of `logs` alone leaves in the MANA table: the
    /// projection a recovery must end at.
    async fn clean_mana(head: u64, logs: Vec<RpcLog>) -> Vec<String> {
        let db = ScratchDb::create().await.unwrap();
        let (_, url) = chain_of(head, logs, 10_000).await;
        let st = state_over(db.pool.clone(), &url, index_settings(100, 100), false);
        assert!(publish(&st, &url, &MANA).await.published);
        let r = rows(&db.pool, "shop.mana_transfer").await;
        db.drop().await;
        r
    }

    fn calls_since(chain: &FakeChain, mark: usize) -> Vec<String> {
        chain.calls.lock()[mark..].to_vec()
    }

    fn headers(blocks: impl IntoIterator<Item = u64>) -> Vec<String> {
        blocks.into_iter().map(|b| format!("header {b}")).collect()
    }

    /// Every window end of a walk from 0 to 998 in windows of 100.
    fn hundreds() -> Vec<i64> {
        (0..9).map(|k| 99 + 100 * k).chain([998]).collect()
    }

    /// A reorg past the confirmation depth rewinds to the newest recovery
    /// point still on the chain, found by comparing the points below the
    /// cursor newest first: the rows below it stay, its hash and time are
    /// the cursor's, the points past it go, and the projection ends as one
    /// walk of the new branch leaves it.
    #[tokio::test]
    async fn a_reorg_rewinds_to_the_newest_recovery_point_still_on_the_chain() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let kept = vec![mana_log(120, ALICE, BOB, 1), mana_log(480, BOB, CARL, 2)];
        let old = [
            kept.clone(),
            vec![mana_log(560, ALICE, BOB, 3), mana_log(900, CARL, ALICE, 4)],
        ]
        .concat();
        let new = vec![mana_log(570, BOB, ALICE, 5), mana_log(910, ALICE, CARL, 6)];
        let (chain, url) = chain_of(1_000, old, 10_000).await;
        let st = state_over(db.pool.clone(), &url, index_settings(100, 100), false);
        assert!(publish(&st, &url, &MANA).await.published);
        assert_eq!(points(&db.pool, INDEXER_MANA).await, hundreds());
        let below = rows(&db.pool, "shop.mana_transfer").await[..2].to_vec();
        chain.fork(550, 0xee, new.clone());
        let mark = chain.calls.lock().len();
        let rewound = pass(&st, &url, &MANA).await;
        assert_eq!(rewound.reorg_rewound_to, Some(499), "{rewound:?}");
        // The tip's check, then the points below it, newest first, until
        // one is on the chain.
        assert_eq!(
            calls_since(&chain, mark),
            headers([998, 899, 799, 699, 599, 499])
        );
        assert_eq!(
            at(&db.pool, INDEXER_MANA).await,
            (499, Some(chain.hash_of(499)))
        );
        let c = cursor(&db.pool, INDEXER_MANA, ChainNet::Matic)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(c.last_block_time, Some(time_of(499)));
        assert_eq!(points(&db.pool, INDEXER_MANA).await, hundreds()[..5]);
        assert!(!ready(&db.pool, INDEXER_MANA).await, "a rewind unpublishes");
        assert_eq!(rows(&db.pool, "shop.mana_transfer").await, below);
        assert!(publish(&st, &url, &MANA).await.published);
        assert_eq!(
            rows(&db.pool, "shop.mana_transfer").await,
            clean_mana(1_000, [kept, new].concat()).await
        );
        db.drop().await;
    }

    /// A fork below every recovery point: each is compared once, none is
    /// on the chain, and the rewind goes to the start, dropping them all.
    #[tokio::test]
    async fn a_fork_below_every_recovery_point_rewinds_to_the_start() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let (chain, url) = chain_of(1_000, vec![mana_log(30, ALICE, BOB, 1)], 10_000).await;
        let st = state_over(db.pool.clone(), &url, index_settings(100, 100), false);
        assert!(publish(&st, &url, &MANA).await.published);
        let new = vec![mana_log(60, BOB, ALICE, 2)];
        chain.fork(50, 0xee, new.clone());
        let mark = chain.calls.lock().len();
        let rewound = pass(&st, &url, &MANA).await;
        assert_eq!(rewound.reorg_rewound_to, Some(-1), "{rewound:?}");
        assert_eq!(
            calls_since(&chain, mark),
            headers([998, 899, 799, 699, 599, 499, 399, 299, 199, 99])
        );
        assert!(points(&db.pool, INDEXER_MANA).await.is_empty());
        assert_eq!(at(&db.pool, INDEXER_MANA).await, (-1, None));
        assert!(publish(&st, &url, &MANA).await.published);
        assert_eq!(
            rows(&db.pool, "shop.mana_transfer").await,
            clean_mana(1_000, [vec![mana_log(30, ALICE, BOB, 1)], new].concat()).await
        );
        db.drop().await;
    }

    /// A cursor with no recovery points (as an upgraded database's is until
    /// its next advance) rewinds to the start after the tip's check alone.
    #[tokio::test]
    async fn a_cursor_with_no_recovery_points_rewinds_to_the_start() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let (chain, url) = chain_of(1_000, vec![mana_log(700, ALICE, BOB, 1)], 10_000).await;
        let st = state_over(db.pool.clone(), &url, index_settings(100, 100), false);
        assert!(publish(&st, &url, &MANA).await.published);
        sqlx::query("DELETE FROM shop.chain_checkpoint")
            .execute(&db.pool)
            .await
            .unwrap();
        chain.fork(550, 0xee, vec![]);
        let mark = chain.calls.lock().len();
        let rewound = pass(&st, &url, &MANA).await;
        assert_eq!(rewound.reorg_rewound_to, Some(-1), "{rewound:?}");
        assert_eq!(calls_since(&chain, mark), headers([998]));
        db.drop().await;
    }

    /// Each projection keeps its newest 64 points, and an operator's rebuild
    /// drops them all with the history.
    #[tokio::test]
    async fn the_newest_recovery_points_are_kept_and_a_rebuild_drops_them() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let (_, url) = chain_of(1_000, vec![], 10_000).await;
        let st = state_over(db.pool.clone(), &url, index_settings(1_000, 10), false);
        assert!(publish(&st, &url, &MANA).await.published);
        let kept: Vec<i64> = (0..63).map(|k| 369 + 10 * k).chain([998]).collect();
        assert_eq!(points(&db.pool, INDEXER_MANA).await, kept);
        sqlx::query("UPDATE shop.chain_cursor SET rebuild_requested_at = timezone('UTC', now()) WHERE indexer = 'mana_transfers'")
            .execute(&db.pool)
            .await
            .unwrap();
        assert!(pass(&st, &url, &MANA).await.rebuilt);
        assert!(points(&db.pool, INDEXER_MANA).await.is_empty());
        db.drop().await;
    }

    /// A writer that lost the projection records no recovery point,
    /// withdraws nothing, rewinds nothing, and a rebuild it runs drops none.
    #[tokio::test]
    async fn a_writer_that_lost_the_projection_cannot_record_withdraw_or_recover() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let (chain, url) = chain_of(1_000, vec![mana_log(700, ALICE, BOB, 1)], 10_000).await;
        let st = state_over(db.pool.clone(), &url, index_settings(100, 100), false);
        assert!(publish(&st, &url, &MANA).await.published);
        let settings = &st.settings.market.indexes;
        let stale = chain_walk::test_owner(&db.pool, &MANA, settings).await;
        let _current = chain_walk::test_owner(&db.pool, &MANA, settings).await;
        let cur = cursor(&db.pool, INDEXER_MANA, ChainNet::Matic)
            .await
            .unwrap()
            .unwrap();
        let checkpoint = (cur.last_block, cur.last_block_hash.as_deref());
        *chain.head.lock() = 1_100;
        let header = crate::chain_index::rpc::BlockHeader {
            number: 1_050,
            hash: chain.hash_of(1_050),
            timestamp: time_of(1_050),
        };
        assert!(
            chain_walk::commit_window(
                &db.pool,
                &MANA,
                &stale,
                checkpoint,
                &header,
                &[],
                &HashMap::new()
            )
            .await
            .is_err()
        );
        assert_eq!(points(&db.pool, INDEXER_MANA).await, hundreds());
        assert!(
            chain_walk::withdraw(&db.pool, &stale, checkpoint)
                .await
                .is_err()
        );
        assert!(ready(&db.pool, INDEXER_MANA).await, "still published");
        chain.fork(550, 0xee, vec![]);
        let source = chain_walk::RpcSource::new(Rpc::at(&st.http, &url));
        let mut out = PassOutcome::default();
        assert!(
            chain_walk::recover(&db.pool, &source, &MANA, &stale, &cur, &mut out)
                .await
                .is_err()
        );
        assert_eq!(
            at(&db.pool, INDEXER_MANA).await,
            (998, Some(format!("0x{:064x}", 998)))
        );
        assert_eq!(points(&db.pool, INDEXER_MANA).await, hundreds());
        assert_eq!(rows(&db.pool, "shop.mana_transfer").await.len(), 1);
        sqlx::query("UPDATE shop.chain_cursor SET rebuild_requested_at = timezone('UTC', now()) WHERE indexer = 'mana_transfers'")
            .execute(&db.pool)
            .await
            .unwrap();
        assert!(
            chain_walk::run_pass(&db.pool, &source, &MANA, settings, &stale, None, &|_| {})
                .await
                .is_err()
        );
        assert_eq!(points(&db.pool, INDEXER_MANA).await, hundreds());
        db.drop().await;
    }

    /// Projections whose windows ended at different blocks recover at
    /// different points, each at its own newest one below the fork.
    #[tokio::test]
    async fn each_projection_recovers_at_its_own_newest_point() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let (chain, url) = chain_of(1_000, vec![], 10_000).await;
        let by_hundreds = state_over(
            db.pool.clone(),
            &url,
            IndexSettings {
                start_block_matic: Some(0),
                ..index_settings(100, 100)
            },
            false,
        );
        let by_150 = state_over(
            db.pool.clone(),
            &url,
            IndexSettings {
                start_block_matic: Some(0),
                ..index_settings(100, 150)
            },
            false,
        );
        let trades = TradeEventsIndex {
            net: ChainNet::Matic,
            chain_id: 137,
        };
        assert!(publish(&by_hundreds, &url, &MANA).await.published);
        assert!(publish(&by_150, &url, &trades).await.published);
        chain.fork(520, 0xee, vec![]);
        assert_eq!(
            pass(&by_hundreds, &url, &MANA).await.reorg_rewound_to,
            Some(499)
        );
        assert_eq!(
            pass(&by_150, &url, &trades).await.reorg_rewound_to,
            Some(449)
        );
        db.drop().await;
    }

    /// A fork right after a window's bracket closed, at a window that ended
    /// short of its target (the endpoint narrowed it): the end's hash was
    /// read inside the bracket, so the old branch's logs are committed with
    /// the old branch's hash, the next pass sees the divergence, and the
    /// recovery reads the new branch's different transfer.
    #[tokio::test]
    async fn a_fork_after_a_short_window_is_recovered_with_the_new_branch() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let old = vec![mana_log(700, ALICE, BOB, 1), mana_log(1_030, ALICE, BOB, 2)];
        let new = vec![mana_log(1_030, BOB, CARL, 3)];
        let (chain, url) = chain_of(1_000, old, 60).await;
        // One window a pass, so the pass ends with the window that forks.
        let st = state_over(db.pool.clone(), &url, index_settings(1, 100), false);
        assert!(publish(&st, &url, &MANA).await.published);
        // The next window is [999, 1098], narrowed to [999, 1058]; the fork
        // comes once its second read at 1098 (the bracket's close) answered.
        *chain.head.lock() = 1_100;
        *chain.fork_after.lock() = Some(ForkAfter {
            header: 1_098,
            reads: 2,
            from: 1_020,
            salt: 0xee,
            logs: new.iter().map(log_json).collect(),
            one_read: false,
        });
        let walked = pass(&st, &url, &MANA).await;
        assert_eq!(walked.indexed_to, 1_058, "{walked:?}");
        assert!(publish(&st, &url, &MANA).await.published);
        assert_eq!(
            rows(&db.pool, "shop.mana_transfer").await,
            clean_mana(1_100, [vec![mana_log(700, ALICE, BOB, 1)], new].concat()).await
        );
        db.drop().await;
    }

    /// A fork between two windows of one pass: the second window's bracket
    /// is stable on the new branch, but the checkpoint it extends is not on
    /// it. Nothing is committed, the publication is withdrawn and the pass
    /// recovers; the projection ends with the new branch's transfer.
    #[tokio::test]
    async fn a_fork_between_two_windows_of_a_pass_is_recovered() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let old = vec![mana_log(700, ALICE, BOB, 1), mana_log(1_030, ALICE, BOB, 2)];
        let new = vec![mana_log(1_030, BOB, CARL, 3)];
        let (chain, url) = chain_of(1_000, old, 60).await;
        let st = state_over(db.pool.clone(), &url, index_settings(100, 100), false);
        assert!(publish(&st, &url, &MANA).await.published);
        *chain.head.lock() = 1_100;
        *chain.fork_after.lock() = Some(ForkAfter {
            header: 1_098,
            reads: 2,
            from: 1_020,
            salt: 0xee,
            logs: new.iter().map(log_json).collect(),
            one_read: false,
        });
        let recovered = pass(&st, &url, &MANA).await;
        assert_eq!(recovered.reorg_rewound_to, Some(998), "{recovered:?}");
        assert!(publish(&st, &url, &MANA).await.published);
        assert_eq!(
            rows(&db.pool, "shop.mana_transfer").await,
            clean_mana(1_100, [vec![mana_log(700, ALICE, BOB, 1)], new].concat()).await
        );
        db.drop().await;
    }

    /// The first window after a published pass finds the checkpoint it
    /// extends on another branch: its publication is withdrawn before the
    /// search reads any older header, so while that read waits, readers fall
    /// back and nothing durable moved.
    #[tokio::test]
    async fn a_first_window_that_diverges_withdraws_before_its_search() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let (chain, url) = chain_of(1_000, vec![mana_log(700, ALICE, BOB, 1)], 10_000).await;
        let st = state_over(db.pool.clone(), &url, index_settings(100, 100), false);
        assert!(publish(&st, &url, &MANA).await.published);
        assert!(ready(&db.pool, INDEXER_MANA).await);
        // The fork lands right after the pass's own check at 998 answered.
        *chain.head.lock() = 1_100;
        *chain.fork_after.lock() = Some(ForkAfter {
            header: 998,
            reads: 1,
            from: 950,
            salt: 0xee,
            logs: vec![],
            one_read: false,
        });
        let gate = Arc::new(tokio::sync::Semaphore::new(0));
        *chain.probe_gate.lock() = Some((998, gate.clone()));
        let (st2, url2) = (st.clone(), url.clone());
        let walk = tokio::spawn(async move { pass(&st2, &url2, &MANA).await });
        for _ in 0..200 {
            if chain.probes_waiting.load(Ordering::SeqCst) > 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(chain.probes_waiting.load(Ordering::SeqCst), 1);
        assert!(!ready(&db.pool, INDEXER_MANA).await, "withdrawn first");
        assert_eq!(
            at(&db.pool, INDEXER_MANA).await,
            (998, Some(format!("0x{:064x}", 998)))
        );
        assert_eq!(points(&db.pool, INDEXER_MANA).await, hundreds());
        assert_eq!(rows(&db.pool, "shop.mana_transfer").await.len(), 1);
        gate.add_permits(10);
        assert_eq!(walk.await.unwrap().reorg_rewound_to, Some(899));
        db.drop().await;
    }

    /// A pass that finds its cursor block on another branch withdraws the
    /// publication before it reads any older header: while that read waits,
    /// readers fall back and nothing durable moved.
    #[tokio::test]
    async fn a_divergence_at_the_cursor_withdraws_before_its_search() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let (chain, url) = chain_of(1_000, vec![mana_log(700, ALICE, BOB, 1)], 10_000).await;
        let st = state_over(db.pool.clone(), &url, index_settings(100, 100), false);
        assert!(publish(&st, &url, &MANA).await.published);
        let advanced = cursor(&db.pool, INDEXER_MANA, ChainNet::Matic)
            .await
            .unwrap()
            .unwrap()
            .updated_at;
        chain.fork(550, 0xee, vec![]);
        let gate = Arc::new(tokio::sync::Semaphore::new(0));
        *chain.probe_gate.lock() = Some((998, gate.clone()));
        let (st2, url2) = (st.clone(), url.clone());
        let walk = tokio::spawn(async move { pass(&st2, &url2, &MANA).await });
        for _ in 0..200 {
            if chain.probes_waiting.load(Ordering::SeqCst) > 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(chain.probes_waiting.load(Ordering::SeqCst), 1);
        assert!(!ready(&db.pool, INDEXER_MANA).await, "withdrawn first");
        assert_eq!(
            at(&db.pool, INDEXER_MANA).await,
            (998, Some(format!("0x{:064x}", 998)))
        );
        assert_eq!(points(&db.pool, INDEXER_MANA).await, hundreds());
        assert_eq!(rows(&db.pool, "shop.mana_transfer").await.len(), 1);
        let c = cursor(&db.pool, INDEXER_MANA, ChainNet::Matic)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(c.updated_at, advanced, "a withdrawal is no progress");
        gate.add_permits(10);
        assert_eq!(walk.await.unwrap().reorg_rewound_to, Some(499));
        db.drop().await;
    }

    const C1: &str = "0x00000000000000000000000000000000000000c1";

    /// One token of a collection minted to ALICE at s+100 and sold to BOB at
    /// s+2,300, walked in windows of 500 to s+2,936 and published (points
    /// every 500 blocks), with a producer of notifications reading the nft
    /// index standing at the cursor.
    async fn nft_history(db: &ScratchDb) -> (Arc<FakeChain>, String, SharedState, u64) {
        let s = nft_index::start_block(ChainNet::Matic);
        seed_collection(
            &db.pool,
            "matic",
            C1,
            s + 10,
            "urn:decentraland:matic:collections-v2:0xc1",
            "Pirates",
        )
        .await;
        let t1 = v2_token(0, 1);
        let chain = FakeChain::new(
            s + 3_000,
            vec![
                transfer(C1, ZERO, ALICE, t1, s + 100, 0),
                transfer(C1, ALICE, BOB, t1, s + 2_300, 0),
            ],
            1_000_000,
            100,
        );
        let url = serve(chain.clone()).await;
        let st = state_over(db.pool.clone(), &url, index_settings(100, 500), false);
        assert!(
            publish_nft(&st, &url, &NftIndex::new(ChainNet::Matic))
                .await
                .published
        );
        sqlx::query(
            "INSERT INTO shop.notification_cursor (producer, block_number, log_index, seed_block, seed_log_index) \
             VALUES ('royalties_earned', $1, 0, 0, 0)",
        )
        .bind((s + 2_936) as i64)
        .execute(&db.pool)
        .await
        .unwrap();
        (chain, url, st, s)
    }

    async fn producer(pool: &PgPool) -> (i64, i32) {
        sqlx::query_as(
            "SELECT block_number, log_index FROM shop.notification_cursor WHERE producer = 'royalties_earned'",
        )
        .fetch_one(pool)
        .await
        .unwrap()
    }

    /// A search whose older header read fails rewinds nothing: the height,
    /// the rows, the recovery points and the notification cursors stay, and
    /// readers already fell back. The next pass searches again, rewinds to
    /// the newest point on the chain and moves the producers reading the
    /// index back to just after it; the refold follows the new branch.
    #[tokio::test]
    async fn a_failed_search_rewinds_nothing_and_the_next_replays_from_its_point() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let (chain, url, st, s) = nft_history(&db).await;
        let nft = NftIndex::new(ChainNet::Matic);
        let t1 = v2_token(0, 1);
        let points_before = points(&db.pool, INDEXER_NFT).await;
        assert_eq!(
            points_before,
            [499, 999, 1_499, 1_999, 2_499, 2_936].map(|b| (s + b) as i64)
        );
        let before = (
            at(&db.pool, INDEXER_NFT).await,
            rows(&db.pool, "shop.nft_event").await,
            producer(&db.pool).await,
        );
        chain.fork(
            s + 2_200,
            0xee,
            vec![transfer(C1, ALICE, CARL, t1, s + 2_250, 0)],
        );
        *chain.fail_probes_below.lock() = Some(s + 2_936);
        assert!(try_pass(&st, &url, &nft).await.is_err());
        assert!(
            !ready(&db.pool, INDEXER_NFT).await,
            "withdrawn before the search"
        );
        assert_eq!(points(&db.pool, INDEXER_NFT).await, points_before);
        assert_eq!(
            (
                at(&db.pool, INDEXER_NFT).await,
                rows(&db.pool, "shop.nft_event").await,
                producer(&db.pool).await,
            ),
            before
        );
        *chain.fail_probes_below.lock() = None;
        let rewound = pass(&st, &url, &nft).await;
        assert_eq!(
            rewound.reorg_rewound_to,
            Some((s + 1_999) as i64),
            "{rewound:?}"
        );
        assert_eq!(producer(&db.pool).await, ((s + 1_999) as i64, i32::MAX));
        assert!(publish_nft(&st, &url, &nft).await.published);
        assert_eq!(
            owner_of(&db.pool, "matic", C1, t1).await,
            Some(Some(CARL.to_string()))
        );
        db.drop().await;
    }

    /// A backfill lane read while the chain briefly stands on another
    /// branch (it flips back right after the read): the lane's bracket on
    /// the cursor block changes, so it writes nothing, and the next pass
    /// backfills the branch the cursor is on. The late collection's token
    /// ends as one walk of that branch leaves it.
    #[tokio::test]
    async fn a_lane_read_on_a_passing_branch_writes_nothing() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let s = nft_index::start_block(ChainNet::Matic);
        let late = "0x00000000000000000000000000000000000000c3";
        let t7 = v2_token(0, 7);
        seed_collection(
            &db.pool,
            "matic",
            C1,
            s + 10,
            "urn:decentraland:matic:collections-v2:0xc1",
            "Pirates",
        )
        .await;
        let chain = FakeChain::new(
            s + 3_000,
            vec![
                transfer(C1, ZERO, ALICE, v2_token(0, 1), s + 100, 0),
                transfer(late, ZERO, ALICE, t7, s + 1_500, 0),
            ],
            1_000_000,
            100,
        );
        let url = serve(chain.clone()).await;
        let st = state_over(db.pool.clone(), &url, index_settings(100, 500), false);
        let nft = NftIndex::new(ChainNet::Matic);
        assert!(publish_nft(&st, &url, &nft).await.published);
        // The mirror learns the late collection; the next pass takes it in,
        // owing a backfill below the cursor.
        seed_collection(
            &db.pool,
            "matic",
            late,
            s + 1_000,
            "urn:decentraland:matic:collections-v2:0xc3",
            "Late",
        )
        .await;
        *chain.head.lock() = s + 3_100;
        let taken = pass(&st, &url, &nft).await;
        let cursor_block = taken.indexed_to as u64;
        // The lane's pass: right after its own check at the cursor answered,
        // the chain stands on a branch where the late token went to CARL, for
        // exactly one log read.
        *chain.fork_after.lock() = Some(ForkAfter {
            header: cursor_block,
            reads: 1,
            from: s + 1_200,
            salt: 0xee,
            logs: vec![log_json(&transfer(late, ZERO, CARL, t7, s + 1_500, 0))],
            one_read: true,
        });
        let err = try_pass(&st, &url, &nft).await.unwrap_err();
        assert!(
            format!("{err:#}").contains("chain changed while backfilling"),
            "{err:#}"
        );
        assert!(publish_nft(&st, &url, &nft).await.published);
        assert_eq!(
            owner_of(&db.pool, "matic", late, t7).await,
            Some(Some(ALICE.to_string()))
        );
        db.drop().await;
    }

    /// A fresh bootstrap holds the recovery points' table with its keys; a
    /// database from before it (its cursor populated) gains it from the
    /// schema's object pass with the cursor untouched, and a second run
    /// changes nothing.
    #[tokio::test]
    async fn the_recovery_points_table_arrives_by_bootstrap_and_by_upgrade() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let keys: Vec<(String, Option<String>)> = sqlx::query_as(
            "SELECT contype::text, confrelid::regclass::text FROM pg_constraint \
             WHERE conrelid = 'shop.chain_checkpoint'::regclass AND contype IN ('p', 'f') ORDER BY 1",
        )
        .fetch_all(&db.pool)
        .await
        .unwrap();
        assert_eq!(
            keys,
            vec![
                ("f".to_string(), Some("shop.chain_cursor".to_string())),
                ("p".to_string(), Some("-".to_string())),
            ]
        );
        let (chain, url) = chain_of(1_000, vec![mana_log(700, ALICE, BOB, 1)], 10_000).await;
        let st = state_over(db.pool.clone(), &url, index_settings(100, 100), false);
        assert!(publish(&st, &url, &MANA).await.published);
        let cursor_before = rows(&db.pool, "shop.chain_cursor").await;
        sqlx::query("DROP TABLE shop.chain_checkpoint")
            .execute(&db.pool)
            .await
            .unwrap();
        let done = crate::db::schema_migrate::migrate(
            &db.pool,
            crate::db::SCHEMA_SQL,
            crate::db::MIGRATIONS_SQL,
        )
        .await
        .unwrap();
        assert!(done.steps.is_empty(), "{done:?}");
        assert_eq!(done.created.len(), 1, "{done:?}");
        assert!(
            format!("{:?}", done.created[0]).contains("chain_checkpoint"),
            "{done:?}"
        );
        assert_eq!(rows(&db.pool, "shop.chain_cursor").await, cursor_before);
        assert!(
            crate::db::schema_migrate::migrate(
                &db.pool,
                crate::db::SCHEMA_SQL,
                crate::db::MIGRATIONS_SQL
            )
            .await
            .unwrap()
            .is_empty()
        );
        // The next advance records its point in the table it gained.
        *chain.head.lock() = 1_100;
        assert!(publish(&st, &url, &MANA).await.published);
        assert_eq!(points(&db.pool, INDEXER_MANA).await, vec![1_098]);
        db.drop().await;
    }

    /// A backfill lane whose bracket stands still on a branch the cursor's
    /// hash is not on: a divergence, found before the lane writes a row. The
    /// lane's own pass withdraws the publication and recovers below the
    /// fork, even though the lane spent the pass's budget.
    #[tokio::test]
    async fn a_lane_that_finds_the_cursor_on_another_branch_recovers() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let s = nft_index::start_block(ChainNet::Matic);
        let late = "0x00000000000000000000000000000000000000c3";
        let t7 = v2_token(0, 7);
        seed_collection(
            &db.pool,
            "matic",
            C1,
            s + 10,
            "urn:decentraland:matic:collections-v2:0xc1",
            "Pirates",
        )
        .await;
        let chain = FakeChain::new(
            s + 3_000,
            vec![
                transfer(C1, ZERO, ALICE, v2_token(0, 1), s + 100, 0),
                transfer(late, ZERO, ALICE, t7, s + 1_500, 0),
            ],
            1_000_000,
            100,
        );
        let url = serve(chain.clone()).await;
        // Three requests a pass: a lane window (its bracket, its read and its
        // times) spends a whole pass.
        let st = state_over(db.pool.clone(), &url, index_settings(3, 500), false);
        let nft = NftIndex::new(ChainNet::Matic);
        assert!(publish_nft(&st, &url, &nft).await.published);
        seed_collection(
            &db.pool,
            "matic",
            late,
            s + 1_000,
            "urn:decentraland:matic:collections-v2:0xc3",
            "Late",
        )
        .await;
        *chain.head.lock() = s + 3_100;
        let taken = pass(&st, &url, &nft).await;
        let cursor_block = taken.indexed_to as u64;
        *chain.fork_after.lock() = Some(ForkAfter {
            header: cursor_block,
            reads: 1,
            from: s + 1_200,
            salt: 0xee,
            logs: vec![log_json(&transfer(late, ZERO, CARL, t7, s + 1_500, 0))],
            one_read: false,
        });
        let lane = pass(&st, &url, &nft).await;
        assert_eq!(lane.reorg_rewound_to, Some((s + 999) as i64), "{lane:?}");
        let carl: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM shop.nft_event WHERE contract_address = $1 AND to_address = $2",
        )
        .bind(late)
        .bind(CARL.to_ascii_lowercase())
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert_eq!(carl, 0, "nothing written from the lane's read");
        assert!(publish_nft(&st, &url, &nft).await.published);
        assert_eq!(
            owner_of(&db.pool, "matic", late, t7).await,
            Some(Some(CARL.to_string()))
        );
        db.drop().await;
    }

    /// The sales fold infers a bid's seller from the nft walk's transfers
    /// only while the nft cursor is not diverged, read in the same statement:
    /// while the nft recovery's search waits, sales recovers on its own and
    /// refolds the bid without reading the orphaned transfer to ALICE, and
    /// once the nft walk recovers with the replacement transfer to CARL, the
    /// next fold sets CARL without any further sale event.
    #[tokio::test]
    async fn a_diverged_nft_cursor_lends_no_seller_to_the_sales_fold() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let s = nft_index::start_block(ChainNet::Matic);
        let b = sales_index::BID_V2_MATIC_START_BLOCK + 100;
        let t1 = v2_token(0, 1);
        seed_collection(
            &db.pool,
            "matic",
            C1,
            s + 10,
            "urn:decentraland:matic:collections-v2:0xc1",
            "Pirates",
        )
        .await;
        let bid = || {
            sales_index::tests::bid_created_log(C1, t1, DAVE, 0x22, 8, 4_000_000_000, b + 400, 0)
        };
        let chain = FakeChain::new(
            b + 1_000,
            vec![
                transfer(C1, ZERO, BOB, t1, s + 100, 0),
                transfer(C1, BOB, ALICE, t1, b + 300, 0),
                bid(),
            ],
            10_000_000,
            100,
        );
        let url = serve(chain.clone()).await;
        // Different windows, so the two recover at different points: nft at
        // s+6,999,999, sales at s+5,999,999.
        let by_million = state_over(db.pool.clone(), &url, index_settings(100, 1_000_000), false);
        let by_1_5m = state_over(db.pool.clone(), &url, index_settings(100, 1_500_000), false);
        let nft = NftIndex::new(ChainNet::Matic);
        assert!(publish_nft(&by_million, &url, &nft).await.published);
        assert!(publish(&by_1_5m, &url, &SalesIndex).await.published);
        let seller = async || bids_of(&db.pool).await[0].3.clone();
        assert_eq!(seller().await, Some(ALICE.to_string()));
        // The chain reorganizes from b+200: BOB sold to CARL, not ALICE; the
        // bid stands on both branches.
        chain.fork(
            b + 200,
            0xee,
            vec![transfer(C1, BOB, CARL, t1, b + 250, 0), bid()],
        );
        let gate = Arc::new(tokio::sync::Semaphore::new(0));
        *chain.gate_at.lock() = Some((s + 6_999_999, gate.clone()));
        let (st, u) = (by_million.clone(), url.clone());
        let nft_recovery =
            tokio::spawn(async move { try_pass(&st, &u, &NftIndex::new(ChainNet::Matic)).await });
        for _ in 0..200 {
            if chain.probes_waiting.load(Ordering::SeqCst) > 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(
            chain.probes_waiting.load(Ordering::SeqCst),
            1,
            "nft's search waits"
        );
        let c = cursor(&db.pool, INDEXER_NFT, ChainNet::Matic)
            .await
            .unwrap()
            .unwrap();
        assert!(c.diverged_at.is_some(), "the nft cursor is marked diverged");
        // Sales recovers on its own and refolds the bid meanwhile.
        let recovered = pass(&by_1_5m, &url, &SalesIndex).await;
        assert_eq!(
            recovered.reorg_rewound_to,
            Some((s + 5_999_999) as i64),
            "{recovered:?}"
        );
        assert!(publish(&by_1_5m, &url, &SalesIndex).await.published);
        assert_eq!(seller().await, None, "no seller from an orphaned transfer");
        // The nft walk recovers with the transfer to CARL; the next sales
        // fold sets the seller, with no further sale event.
        gate.add_permits(10);
        let nft_rewound = nft_recovery.await.unwrap().unwrap();
        assert_eq!(nft_rewound.reorg_rewound_to, Some((s + 6_999_999) as i64));
        let c = cursor(&db.pool, INDEXER_NFT, ChainNet::Matic)
            .await
            .unwrap()
            .unwrap();
        assert!(c.diverged_at.is_none(), "the rewind clears the mark");
        assert!(publish_nft(&by_million, &url, &nft).await.published);
        assert!(publish(&by_1_5m, &url, &SalesIndex).await.published);
        assert_eq!(seller().await, Some(CARL.to_string()));
        db.drop().await;
    }

    /// A divergence that passes (the chain flips back before any search
    /// rewinds) leaves the cursor as it was; the mark goes with the next
    /// pass's check of the cursor block, whether that pass then closes with
    /// nothing to walk or pauses after a window.
    #[tokio::test]
    async fn a_passing_divergence_is_unmarked_once_the_cursor_is_proven() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let (chain, url) = chain_of(1_000, vec![mana_log(700, ALICE, BOB, 1)], 10_000).await;
        let st = state_over(db.pool.clone(), &url, index_settings(100, 100), false);
        assert!(publish(&st, &url, &MANA).await.published);
        let diverged = async || {
            cursor(&db.pool, INDEXER_MANA, ChainNet::Matic)
                .await
                .unwrap()
                .unwrap()
                .diverged_at
                .is_some()
        };
        let flicker = async || {
            *chain.fork_from.lock() = Some((550, 0xee));
            *chain.fail_probes_below.lock() = Some(998);
            assert!(try_pass(&st, &url, &MANA).await.is_err());
            assert!(diverged().await && !ready(&db.pool, INDEXER_MANA).await);
            *chain.fork_from.lock() = None;
            *chain.fail_probes_below.lock() = None;
        };
        flicker().await;
        let closed = pass(&st, &url, &MANA).await;
        assert!(
            closed.published && closed.reorg_rewound_to.is_none(),
            "{closed:?}"
        );
        assert!(!diverged().await, "the next check clears the mark");
        flicker().await;
        *chain.head.lock() = 1_300;
        let paused = pass(
            &state_over(db.pool.clone(), &url, index_settings(1, 100), false),
            &url,
            &MANA,
        )
        .await;
        assert!(paused.paused && paused.indexed_to == 1_098, "{paused:?}");
        assert!(!diverged().await, "and again");
        db.drop().await;
    }

    /// A sales fold that ran before the nft walk saw the fork inferred a
    /// seller from the old branch's transfer (ALICE). The nft walk's rewind
    /// leaves a recheck, so the next sales fold, with no new sale event,
    /// infers again: CARL, the replacement transfer's buyer. A seller a sale
    /// event names (the acceptance's) stays as it is.
    #[tokio::test]
    async fn an_nft_rewind_has_the_sales_fold_infer_its_sellers_again() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let s = nft_index::start_block(ChainNet::Matic);
        let b = sales_index::BID_V2_MATIC_START_BLOCK + 100;
        let (t1, t2) = (v2_token(0, 1), v2_token(0, 2));
        seed_collection(
            &db.pool,
            "matic",
            C1,
            s + 10,
            "urn:decentraland:matic:collections-v2:0xc1",
            "Pirates",
        )
        .await;
        // DAVE's open bid on t1, and his bid on t2 accepted naming ALICE.
        let bids = || {
            vec![
                sales_index::tests::bid_created_log(
                    C1,
                    t1,
                    DAVE,
                    0x22,
                    8,
                    4_000_000_000,
                    b + 400,
                    0,
                ),
                sales_index::tests::bid_created_log(
                    C1,
                    t2,
                    DAVE,
                    0x23,
                    8,
                    4_000_000_000,
                    b + 410,
                    0,
                ),
                sales_index::tests::bid_accepted_log(C1, t2, DAVE, ALICE, 0x23, 8, b + 420, 0),
            ]
        };
        let chain = FakeChain::new(
            b + 1_000,
            [
                vec![
                    transfer(C1, ZERO, BOB, t1, s + 100, 0),
                    transfer(C1, ZERO, BOB, t2, s + 100, 1),
                    transfer(C1, BOB, ALICE, t1, b + 300, 0),
                ],
                bids(),
            ]
            .concat(),
            10_000_000,
            100,
        );
        let url = serve(chain.clone()).await;
        let by_million = state_over(db.pool.clone(), &url, index_settings(100, 1_000_000), false);
        let by_1_5m = state_over(db.pool.clone(), &url, index_settings(100, 1_500_000), false);
        let nft = NftIndex::new(ChainNet::Matic);
        assert!(publish_nft(&by_million, &url, &nft).await.published);
        assert!(publish(&by_1_5m, &url, &SalesIndex).await.published);
        let sellers = async || {
            bids_of(&db.pool)
                .await
                .into_iter()
                .map(|(token, _, _, seller, _, _)| (token, seller))
                .collect::<Vec<_>>()
        };
        let both = |one: &str, two: &str| {
            vec![
                (t1.to_string(), Some(one.to_string())),
                (t2.to_string(), Some(two.to_string())),
            ]
        };
        assert_eq!(sellers().await, both(ALICE, ALICE));
        // The chain reorganizes from b+200: BOB sold t1 to CARL, not ALICE;
        // the bids stand on both branches.
        chain.fork(
            b + 200,
            0xee,
            [vec![transfer(C1, BOB, CARL, t1, b + 250, 0)], bids()].concat(),
        );
        // Sales recovers and folds first, while the nft walk has not looked:
        // its cursor still stands, unmarked, past the bid.
        let recovered = pass(&by_1_5m, &url, &SalesIndex).await;
        assert_eq!(
            recovered.reorg_rewound_to,
            Some((s + 5_999_999) as i64),
            "{recovered:?}"
        );
        assert!(publish(&by_1_5m, &url, &SalesIndex).await.published);
        assert_eq!(
            sellers().await,
            both(ALICE, ALICE),
            "inferred from the old branch"
        );
        // The nft walk recovers and walks the new branch.
        let nft_rewound = pass(&by_million, &url, &nft).await;
        assert_eq!(nft_rewound.reorg_rewound_to, Some((s + 6_999_999) as i64));
        let rechecks: Vec<i64> =
            sqlx::query_scalar("SELECT from_block FROM shop.bid_seller_recheck")
                .fetch_all(&db.pool)
                .await
                .unwrap();
        assert_eq!(rechecks, vec![(s + 6_999_999) as i64]);
        assert!(publish_nft(&by_million, &url, &nft).await.published);
        // The next sales fold, with no new sale event.
        let sale_events: i64 = sqlx::query_scalar("SELECT count(*) FROM shop.sale_event")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert!(publish(&by_1_5m, &url, &SalesIndex).await.published);
        assert_eq!(sellers().await, both(CARL, ALICE));
        let after: i64 = sqlx::query_scalar("SELECT count(*) FROM shop.sale_event")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(after, sale_events, "no new sale event");
        let left: i64 = sqlx::query_scalar("SELECT count(*) FROM shop.bid_seller_recheck")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(left, 0, "the fold deleted the recheck it read");
        db.drop().await;
    }

    /// Points an older binary left past the cursor (its rewind drops none)
    /// belong to no proven prefix: the next advance drops every point past
    /// the checkpoint it extends, so they neither crowd the newest 64 nor
    /// become recovery points.
    #[tokio::test]
    async fn an_advance_drops_the_points_past_the_checkpoint_it_extends() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let (chain, url) = chain_of(1_000, vec![mana_log(700, ALICE, BOB, 1)], 10_000).await;
        let st = state_over(db.pool.clone(), &url, index_settings(100, 100), false);
        assert!(publish(&st, &url, &MANA).await.published);
        // An older binary's full rewind: the cursor back at the start with no
        // hash, every point left behind, and one more at a height no window
        // of this walk ends at.
        sqlx::query(
            "UPDATE shop.chain_cursor SET last_block = -1, last_block_hash = NULL, last_block_time = NULL, \
                 fold_through = NULL, folded_block = NULL, folded_at = NULL WHERE indexer = 'mana_transfers'",
        )
        .execute(&db.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO shop.chain_checkpoint (indexer, network, block_number, block_hash, block_time) \
             VALUES ('mana_transfers', 'matic', 650, $1, 0)",
        )
        .bind(format!("0x{}", "ff".repeat(32)))
        .execute(&db.pool)
        .await
        .unwrap();
        assert!(publish(&st, &url, &MANA).await.published);
        let held: Vec<(i64, String)> = sqlx::query_as(
            "SELECT block_number, block_hash FROM shop.chain_checkpoint WHERE indexer = 'mana_transfers' ORDER BY 1",
        )
        .fetch_all(&db.pool)
        .await
        .unwrap();
        let fresh: Vec<(i64, String)> = hundreds()
            .into_iter()
            .map(|b| (b, chain.hash_of(b as u64)))
            .collect();
        assert_eq!(held, fresh);
        db.drop().await;
    }

    /// A cursor that never publishes (its walk starts past the projection's
    /// start) is unmarked by the next pass that finds its block on its
    /// stored hash again, though that pass's close publishes nothing.
    #[tokio::test]
    async fn a_cursor_that_never_publishes_is_unmarked_when_proven_again() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let (chain, url) = chain_of(1_000, vec![mana_log(700, ALICE, BOB, 1)], 10_000).await;
        let st = state_over(
            db.pool.clone(),
            &url,
            IndexSettings {
                start_block_matic: Some(10),
                ..index_settings(100, 100)
            },
            false,
        );
        let walked = pass(&st, &url, &MANA).await;
        assert_eq!(walked.indexed_to, 998, "{walked:?}");
        assert!(!walked.published, "a partial history never publishes");
        *chain.fork_from.lock() = Some((550, 0xee));
        *chain.fail_probes_below.lock() = Some(998);
        assert!(try_pass(&st, &url, &MANA).await.is_err());
        let marked = |c: Option<chain_walk::Cursor>| c.unwrap().diverged_at.is_some();
        assert!(marked(
            cursor(&db.pool, INDEXER_MANA, ChainNet::Matic)
                .await
                .unwrap()
        ));
        *chain.fork_from.lock() = None;
        *chain.fail_probes_below.lock() = None;
        let proven = pass(&st, &url, &MANA).await;
        assert!(
            proven.reorg_rewound_to.is_none() && !proven.published,
            "{proven:?}"
        );
        assert!(!marked(
            cursor(&db.pool, INDEXER_MANA, ChainNet::Matic)
                .await
                .unwrap()
        ));
        db.drop().await;
    }

    /// A lane pass reads the cursor block's header once a lane window, plus
    /// once to open the first: each window's closing read opens the next.
    #[tokio::test]
    async fn a_lane_pass_reads_the_cursor_block_once_a_window() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let s = nft_index::start_block(ChainNet::Matic);
        let late = "0x00000000000000000000000000000000000000c3";
        seed_collection(
            &db.pool,
            "matic",
            C1,
            s + 10,
            "urn:decentraland:matic:collections-v2:0xc1",
            "Pirates",
        )
        .await;
        let chain = FakeChain::new(
            s + 31_100,
            vec![
                transfer(C1, ZERO, ALICE, v2_token(0, 1), s + 100, 0),
                transfer(late, ZERO, ALICE, v2_token(0, 7), s + 1_500, 0),
            ],
            1_000_000,
            100,
        );
        let url = serve(chain.clone()).await;
        let st = state_over(db.pool.clone(), &url, index_settings(100, 10_000), false);
        let nft = NftIndex::new(ChainNet::Matic);
        assert!(publish_nft(&st, &url, &nft).await.published);
        seed_collection(
            &db.pool,
            "matic",
            late,
            s + 1_000,
            "urn:decentraland:matic:collections-v2:0xc3",
            "Late",
        )
        .await;
        *chain.head.lock() = s + 31_200;
        let taken = pass(&st, &url, &nft).await;
        let cursor_block = taken.indexed_to;
        let (from, walked): (i64, i64) = sqlx::query_as(
            "SELECT backfill_from_block, walked_from_block FROM shop.nft_contract WHERE contract_address = $1",
        )
        .bind(late)
        .fetch_one(&db.pool)
        .await
        .unwrap();
        let windows = (walked - from + 9_999) / 10_000;
        assert!(windows >= 3, "{from}..{walked}");
        let mark = chain.calls.lock().len();
        assert!(publish_nft(&st, &url, &nft).await.published);
        let reads = calls_since(&chain, mark)
            .iter()
            .filter(|c| **c == format!("header {cursor_block}"))
            .count() as i64;
        // The pass's own check, the lane's opening read, one read a window.
        assert_eq!(reads, 2 + windows);
        db.drop().await;
    }
}

/// The first pass's touch of a started walk is a fenced write. A job that
/// lost the projection at the same height and hash leaves an aged cursor as
/// it is (its pass fails fenced, and it stays untouched, to try again), and
/// the job that took the projection still touches it.
#[tokio::test]
async fn a_started_walk_touch_is_fenced_by_its_owner() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let chain = FakeChain::new(100, vec![], 10_000, 1_000);
    chain.finalized_lag.store(2, Ordering::SeqCst);
    let url = serve(chain.clone()).await;
    let st = state_over(db.pool.clone(), &url, index_settings(100, 10_000), false);
    let mana = super::mana_transfers::ManaTransfersIndex {
        net: ChainNet::Matic,
    };
    assert!(publish(&st, &url, &mana).await.published);
    sqlx::query(
        "UPDATE shop.chain_cursor SET updated_at = timezone('UTC', now()) - interval '5 hours' \
         WHERE indexer = 'mana_transfers'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let updated_at = async || {
        chain_walk::cursor(&db.pool, chain_walk::INDEXER_MANA, ChainNet::Matic)
            .await
            .unwrap()
            .unwrap()
            .updated_at
    };
    let aged = updated_at().await;
    let settings = &st.settings.market.indexes;
    let stale = chain_walk::test_owner(&db.pool, &mana, settings).await;
    let current = chain_walk::test_owner(&db.pool, &mana, settings).await;
    let source = chain_walk::RpcSource::new(Rpc::at(&st.http, &url));
    let err = chain_walk::run_pass(&db.pool, &source, &mana, settings, &stale, None, &|_| {})
        .await
        .unwrap_err();
    assert!(
        err.chain()
            .any(|c| c.downcast_ref::<chain_walk::Fenced>().is_some()),
        "{err:#}"
    );
    assert_eq!(
        updated_at().await,
        aged,
        "the stale job's touch changed nothing"
    );
    assert!(!stale.started.done(), "a fenced touch is not marked done");
    // The current job, at a checkpoint the cursor no longer stands on.
    let c = chain_walk::cursor(&db.pool, chain_walk::INDEXER_MANA, ChainNet::Matic)
        .await
        .unwrap()
        .unwrap();
    assert!(
        chain_walk::touch_started(
            &db.pool,
            &current,
            (c.last_block - 1, c.last_block_hash.as_deref())
        )
        .await
        .is_err()
    );
    assert_eq!(
        updated_at().await,
        aged,
        "a touch at a moved checkpoint changed nothing"
    );
    chain_walk::run_pass(&db.pool, &source, &mana, settings, &current, None, &|_| {})
        .await
        .unwrap();
    assert!(updated_at().await > aged, "the current job touched it");
    assert!(current.started.done());
    db.drop().await;
}

/// `catalyst shop compare-sources` over a fake chain and a fake portal that
/// serves the same chain's logs and headers.
mod compare_sources_tests {
    use super::*;
    use crate::chain_index::portal::admit::{Admission, AdmittedPortal};
    use crate::chain_index::portal::{AddressHistory, PortalReader};
    use crate::market::compare_sources::{self, Request};
    use axum::routing::get;

    /// What the fake portal does differently from its chain.
    #[derive(Default)]
    struct Quirks {
        /// At most this many blocks a page (an early end).
        page_blocks: Option<u64>,
        /// Logs it does not serve, by (block, log index).
        drop: Vec<(u64, u64)>,
        /// Its heads answer 503.
        heads_down: bool,
        /// After this many pages, the RPC's headers start failing.
        rpc_fails_after_pages: Option<usize>,
        pages: usize,
        /// The addresses its coverage answer lists, with their first block:
        /// it then answers `/contracts` up to its finalized block, and names
        /// [`REVISION`] on every page; without it, `/contracts` is a 404.
        coverage: Option<Vec<(String, u64)>>,
    }

    /// The one manifest the fake lake serves from.
    const REVISION: &str = "3333333333333333333333333333333333333333333333333333333333333333";

    struct Portal {
        chain: Arc<FakeChain>,
        quirks: Mutex<Quirks>,
        /// Every request it answered.
        hits: Arc<AtomicUsize>,
        /// The coverage reads and pages that carried an `x-api-key`.
        keyed: Arc<AtomicUsize>,
    }

    fn finalized(chain: &FakeChain) -> u64 {
        chain
            .head
            .lock()
            .saturating_sub(chain.finalized_lag.load(Ordering::SeqCst))
    }

    fn header(chain: &FakeChain, n: u64) -> Value {
        json!({"number": n, "hash": chain.hash_of(n), "parentHash": chain.hash_of(n.saturating_sub(1)), "timestamp": time_of(n)})
    }

    async fn stream(
        State(p): State<Arc<Portal>>,
        headers: axum::http::HeaderMap,
        body: String,
    ) -> axum::response::Response {
        p.hits.fetch_add(1, Ordering::SeqCst);
        if headers.contains_key("x-api-key") {
            p.keyed.fetch_add(1, Ordering::SeqCst);
        }
        let q: Value = serde_json::from_str(&body).unwrap();
        let from = q["fromBlock"].as_u64().unwrap();
        let fin = finalized(&p.chain);
        if from > fin {
            return (
                StatusCode::NO_CONTENT,
                [("x-sqd-finalized-head-number", fin.to_string())],
            )
                .into_response();
        }
        let mut quirks = p.quirks.lock();
        quirks.pages += 1;
        if quirks
            .rpc_fails_after_pages
            .is_some_and(|n| quirks.pages >= n)
        {
            p.chain.fail_headers.store(true, Ordering::SeqCst);
        }
        let mut to = q["toBlock"].as_u64().unwrap().min(fin);
        if let Some(n) = quirks.page_blocks {
            to = to.min(from + n - 1);
        }
        let wanted = |l: &Value| {
            q["logs"].as_array().unwrap().iter().any(|r| {
                let address = l["address"].as_str().unwrap().to_ascii_lowercase();
                r["address"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|a| a == &json!(address))
                    && (0..4).all(|i| match r.get(format!("topic{i}")) {
                        None => true,
                        Some(alts) => l["topics"].get(i).is_some_and(|t| {
                            alts.as_array()
                                .unwrap()
                                .contains(&json!(t.as_str().unwrap().to_ascii_lowercase()))
                        }),
                    })
            })
        };
        let mut blocks: std::collections::BTreeMap<u64, Vec<Value>> = Default::default();
        for l in p.chain.logs.lock().iter() {
            let (b, i) = (hex_u64(&l["blockNumber"]), hex_u64(&l["logIndex"]));
            if b < from || b > to || quirks.drop.contains(&(b, i)) || !wanted(l) {
                continue;
            }
            blocks.entry(b).or_default().push(json!({
                "logIndex": i, "transactionIndex": 0, "address": l["address"], "topics": l["topics"],
                "data": l["data"], "transactionHash": l["transactionHash"],
            }));
        }
        blocks.entry(to).or_default();
        let body: String = blocks
            .into_iter()
            .map(|(b, logs)| {
                let mut line = json!({"header": header(&p.chain, b)});
                if !logs.is_empty() {
                    line["logs"] = json!(logs);
                }
                format!("{line}\n")
            })
            .collect();
        let mut response = ([("content-type", "application/x-ndjson")], body).into_response();
        if quirks.coverage.is_some() {
            response.headers_mut().insert(
                "x-lake-coverage-revision",
                axum::http::HeaderValue::from_static(REVISION),
            );
        }
        response
    }

    /// A portal over `chain` whose coverage lists `every` address, and how
    /// many requests it answered.
    pub(super) async fn covering_portal(
        chain: Arc<FakeChain>,
        every: Vec<(String, u64)>,
    ) -> (String, Arc<AtomicUsize>) {
        let portal = Arc::new(Portal {
            chain,
            quirks: Mutex::new(Quirks {
                coverage: Some(every),
                ..Default::default()
            }),
            hits: Arc::default(),
            keyed: Arc::default(),
        });
        let hits = portal.hits.clone();
        (portal_at(portal).await, hits)
    }

    /// The same, and how many of its requests carried a key.
    async fn keyed_portal(
        chain: Arc<FakeChain>,
        every: Vec<(String, u64)>,
    ) -> (String, Arc<AtomicUsize>, Arc<AtomicUsize>) {
        let portal = Arc::new(Portal {
            chain,
            quirks: Mutex::new(Quirks {
                coverage: Some(every),
                ..Default::default()
            }),
            hits: Arc::default(),
            keyed: Arc::default(),
        });
        let (hits, keyed) = (portal.hits.clone(), portal.keyed.clone());
        (portal_at(portal).await, hits, keyed)
    }

    async fn contracts(
        State(p): State<Arc<Portal>>,
        headers: axum::http::HeaderMap,
    ) -> axum::response::Response {
        p.hits.fetch_add(1, Ordering::SeqCst);
        if headers.contains_key("x-api-key") {
            p.keyed.fetch_add(1, Ordering::SeqCst);
        }
        let Some(coverage) = p.quirks.lock().coverage.clone() else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let fin = finalized(&p.chain);
        axum::Json(json!({
            "schemaVersion": 1, "dataset": "polygon-mainnet", "chainId": 137,
            "coverageRevision": REVISION,
            "finalized": {"number": fin, "hash": p.chain.hash_of(fin)},
            "contracts": coverage.iter().map(|(a, f)| json!({"address": a, "from": f})).collect::<Vec<_>>(),
        }))
        .into_response()
    }

    async fn heads(State(p): State<Arc<Portal>>, uri: axum::http::Uri) -> axum::response::Response {
        p.hits.fetch_add(1, Ordering::SeqCst);
        if p.quirks.lock().heads_down {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
        let n = if uri.path().ends_with("/finalized-head") {
            finalized(&p.chain)
        } else {
            *p.chain.head.lock()
        };
        axum::Json(json!({"number": n, "hash": p.chain.hash_of(n)})).into_response()
    }

    async fn serve_portal(chain: Arc<FakeChain>, quirks: Quirks) -> String {
        portal_at(Arc::new(Portal {
            chain,
            quirks: Mutex::new(quirks),
            hits: Arc::default(),
            keyed: Arc::default(),
        }))
        .await
    }

    async fn portal_at(portal: Arc<Portal>) -> String {
        let mut app = Router::new();
        for ds in ["polygon-mainnet", "ethereum-mainnet"] {
            app = app
                .route(&format!("/datasets/{ds}/finalized-stream"), post(stream))
                .route(&format!("/datasets/{ds}/head"), get(heads))
                .route(&format!("/datasets/{ds}/finalized-head"), get(heads))
                .route(&format!("/datasets/{ds}/contracts"), get(contracts));
        }
        let app = app.with_state(portal);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}")
    }

    fn mana_log(block: u64, index: u64, amount: u64) -> crate::chain_index::rpc::RpcLog {
        crate::chain_index::rpc::RpcLog {
            address: super::super::mana_transfers::contract(ChainNet::Matic),
            topics: vec![
                super::super::mana_transfers::TRANSFER.into(),
                format!("0x{:0>64}", &ALICE[2..]),
                format!("0x{:0>64}", &BOB[2..]),
            ],
            data: format!("0x{amount:064x}"),
            block_number: format!("0x{block:x}"),
            log_index: format!("0x{index:x}"),
            block_timestamp: Some(format!("0x{:x}", time_of(block))),
            block_hash: None,
            transaction_hash: Some(format!("0x{:064x}", block * 1_000 + index)),
        }
    }

    async fn compare(
        db: &ScratchDb,
        rpc: &str,
        portal: &str,
        from: u64,
        to: u64,
    ) -> compare_sources::Report {
        compare_on(db, rpc, portal, "polygon-mainnet", from, to).await
    }

    async fn compare_on(
        db: &ScratchDb,
        rpc: &str,
        portal: &str,
        dataset: &str,
        from: u64,
        to: u64,
    ) -> compare_sources::Report {
        compare_with(db, rpc, portal, dataset, from, to, 100).await
    }

    async fn compare_with(
        db: &ScratchDb,
        rpc: &str,
        portal: &str,
        dataset: &str,
        from: u64,
        to: u64,
        window: u64,
    ) -> compare_sources::Report {
        compare_sources::run(
            &db.pool,
            &crate::http::Outbound::for_tests(),
            Request {
                net: ChainNet::Matic,
                indexer: "mana_transfers".into(),
                from,
                to,
                window,
                rpc_url: rpc.to_string(),
                reader: PortalReader::new(portal, dataset, None, 1_000.0).unwrap(),
                history: AddressHistory::Unknown,
            },
        )
        .await
    }

    fn chain() -> Arc<FakeChain> {
        let chain = FakeChain::new(
            1_000,
            vec![
                mana_log(120, 0, 1),
                mana_log(250, 3, 2),
                mana_log(399, 1, 3),
            ],
            1_000,
            1_000,
        );
        chain.finalized_lag.store(10, Ordering::SeqCst);
        chain
    }

    #[tokio::test]
    async fn equal_sources_compare_the_whole_range() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let chain = chain();
        let (rpc, portal) = (
            serve(chain.clone()).await,
            serve_portal(chain, Quirks::default()).await,
        );
        let r = compare(&db, &rpc, &portal, 101, 400).await;
        assert_eq!((r.outcome, r.exit_code()), ("equal", 0), "{r:#?}");
        assert_eq!(r.compared, [(101, 200), (201, 300), (301, 400)]);
        assert!(r.uncovered.is_empty());
        assert_eq!(r.totals.missing + r.totals.extra + r.totals.mismatch, 0);
        // The fixture's three logs, one per window, on both sides.
        assert_eq!((r.totals.rpc_logs, r.totals.portal_logs), (3, 3));
        assert_eq!(
            r.windows
                .iter()
                .map(|w| (w.outcome.rpc_logs, w.outcome.portal_logs))
                .collect::<Vec<_>>(),
            [(1, 1), (1, 1), (1, 1)]
        );
        assert!(r.sends.rpc_requests > 0 && r.sends.portal.pages >= 3);
        assert_eq!(r.windows[0].filter_fingerprint.len(), 64);
        db.drop().await;
    }

    /// The portal stops early each page: the RPC is read again to where it
    /// stopped, and the next window starts after that block.
    #[tokio::test]
    async fn an_early_portal_end_is_compared_where_both_agree() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let chain = chain();
        let rpc = serve(chain.clone()).await;
        let portal = serve_portal(
            chain,
            Quirks {
                page_blocks: Some(30),
                ..Default::default()
            },
        )
        .await;
        let r = compare(&db, &rpc, &portal, 101, 400).await;
        assert_eq!(r.outcome, "equal", "{r:#?}");
        let mut next = 101;
        for (a, b) in &r.compared {
            assert_eq!(*a, next, "contiguous");
            next = b + 1;
        }
        assert_eq!(next, 401, "the whole range");
        db.drop().await;
    }

    #[tokio::test]
    async fn a_missing_log_makes_the_run_different() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let chain = chain();
        let rpc = serve(chain.clone()).await;
        let portal = serve_portal(
            chain,
            Quirks {
                drop: vec![(250, 3)],
                ..Default::default()
            },
        )
        .await;
        let r = compare(&db, &rpc, &portal, 101, 400).await;
        assert_eq!((r.outcome, r.exit_code()), ("different", 1));
        assert_eq!(r.totals.missing, 1);
        assert_eq!((r.totals.rpc_logs, r.totals.portal_logs), (3, 2));
        assert!(r.different && !r.incomplete);
        db.drop().await;
    }

    /// A range past the finalized head is reported uncovered; an empty
    /// intersection is incomplete, never equal.
    #[tokio::test]
    async fn uncovered_blocks_make_the_run_incomplete() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let chain = chain();
        let (rpc, portal) = (
            serve(chain.clone()).await,
            serve_portal(chain, Quirks::default()).await,
        );
        let r = compare(&db, &rpc, &portal, 901, 1_200).await;
        assert_eq!((r.outcome, r.exit_code()), ("incomplete", 3));
        assert_eq!(r.compared, [(901, 990)]);
        assert_eq!((r.uncovered[0].from, r.uncovered[0].to), (991, 1_200));
        let r = compare(&db, &rpc, &portal, 995, 1_200).await;
        assert_eq!((r.outcome, r.exit_code()), ("incomplete", 3));
        assert!(r.compared.is_empty());
        assert_eq!((r.uncovered[0].from, r.uncovered[0].to), (995, 1_200));
        db.drop().await;
    }

    /// The projection, the dataset and the RPC must name one chain: a
    /// matic run against an Ethereum dataset over an RPC serving Ethereum is
    /// refused before any window, though dataset and RPC agree.
    #[tokio::test]
    async fn a_run_on_the_wrong_network_is_refused() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let chain = chain();
        chain.chain_id.store(1, Ordering::SeqCst);
        let (rpc, portal) = (
            serve(chain.clone()).await,
            serve_portal(chain.clone(), Quirks::default()).await,
        );
        let r = compare_on(&db, &rpc, &portal, "ethereum-mainnet", 101, 400).await;
        assert_eq!((r.outcome, r.exit_code()), ("refused", 4), "{r:#?}");
        assert!(r.refused.is_some() && r.compared.is_empty());
        assert_eq!(r.sends.portal.pages, 0, "no window was read");
        // The dataset right but the RPC on another chain: refused too.
        let r = compare(&db, &rpc, &portal, 101, 400).await;
        assert_eq!((r.outcome, r.exit_code()), ("refused", 4));
        db.drop().await;
    }

    /// An outage at admission or a read failing mid-run still reports: the
    /// windows already compared, the sends, the rest uncovered; exit 3.
    #[tokio::test]
    async fn a_read_failure_is_reported_not_raised() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let chain = chain();
        let rpc = serve(chain.clone()).await;
        let down = serve_portal(
            chain.clone(),
            Quirks {
                heads_down: true,
                ..Default::default()
            },
        )
        .await;
        let r = compare(&db, &rpc, &down, 101, 400).await;
        assert_eq!((r.outcome, r.exit_code()), ("incomplete", 3), "{r:#?}");
        assert!(
            r.uncovered[0].reason.starts_with("portal_unavailable"),
            "{r:#?}"
        );
        assert_eq!((r.uncovered[0].from, r.uncovered[0].to), (101, 400));
        assert!(r.sends.portal.admission > 0);
        let failing = serve_portal(
            chain.clone(),
            Quirks {
                rpc_fails_after_pages: Some(1),
                ..Default::default()
            },
        )
        .await;
        let r = compare(&db, &rpc, &failing, 101, 400).await;
        chain.fail_headers.store(false, Ordering::SeqCst);
        assert_eq!((r.outcome, r.exit_code()), ("incomplete", 3), "{r:#?}");
        assert_eq!(
            r.compared,
            [(101, 200)],
            "the window before the failure is kept"
        );
        let last = r.uncovered.last().unwrap();
        assert_eq!((last.from, last.to), (201, 400));
        assert!(last.reason.starts_with("read_failed"), "{last:?}");
        db.drop().await;
    }

    /// A compared field one side lacked leaves the run incomplete, never
    /// equal; a difference besides makes it different and incomplete.
    #[tokio::test]
    async fn an_unavailable_field_leaves_the_run_incomplete() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let chain = chain();
        chain.strip_block_hash.store(true, Ordering::SeqCst);
        let rpc = serve(chain.clone()).await;
        let portal = serve_portal(chain.clone(), Quirks::default()).await;
        let r = compare(&db, &rpc, &portal, 101, 400).await;
        assert_eq!((r.outcome, r.exit_code()), ("incomplete", 3), "{r:#?}");
        assert!(
            r.totals
                .unavailable
                .get("block_hash")
                .is_some_and(|n| *n > 0)
        );
        let dropping = serve_portal(
            chain,
            Quirks {
                drop: vec![(250, 3)],
                ..Default::default()
            },
        )
        .await;
        let r = compare(&db, &rpc, &dropping, 101, 400).await;
        assert_eq!((r.outcome, r.exit_code()), ("different", 1));
        assert!(r.different && r.incomplete);
        db.drop().await;
    }

    /// An RPC that echoes a credential in its errors never puts it in the
    /// report, whether it fails at admission or after a compared window; the
    /// coverage and the sends are still reported.
    #[tokio::test]
    async fn an_rpc_error_never_reaches_the_report() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let chain = chain();
        *chain.echo.lock() = "https://rpc.example/sk-example-rpc".into();
        let rpc = serve(chain.clone()).await;
        let portal = serve_portal(chain.clone(), Quirks::default()).await;
        chain.fail_chain_id.store(true, Ordering::SeqCst);
        let r = compare(&db, &rpc, &portal, 101, 400).await;
        chain.fail_chain_id.store(false, Ordering::SeqCst);
        let json = serde_json::to_string(&r).unwrap();
        assert!(!json.contains("sk-example-rpc"), "{json}");
        assert_eq!((r.outcome, r.exit_code()), ("incomplete", 3));
        assert_eq!(
            r.uncovered[0].reason,
            "rpc_unavailable: eth_chainId: rpc_error"
        );
        assert!(r.sends.rpc_requests >= 1);
        let failing = serve_portal(
            chain.clone(),
            Quirks {
                rpc_fails_after_pages: Some(1),
                ..Default::default()
            },
        )
        .await;
        let r = compare(&db, &rpc, &failing, 101, 400).await;
        chain.fail_headers.store(false, Ordering::SeqCst);
        let json = serde_json::to_string(&r).unwrap();
        assert!(!json.contains("sk-example-rpc"), "{json}");
        assert_eq!(r.compared, [(101, 200)]);
        let last = r.uncovered.last().unwrap();
        assert_eq!(
            last.reason, "read_failed: rpc window [201, 300]: rpc_error",
            "{last:?}"
        );
        assert!(r.sends.rpc_requests > 0 && r.sends.portal.pages > 0);
        db.drop().await;
    }

    /// The command as the binary runs it, for an index whose filters read
    /// the database, against a database that cannot be reached: a report of
    /// the whole range uncovered, exit 3, and not one chain read.
    #[tokio::test]
    async fn an_unreachable_database_is_reported_before_any_chain_read() {
        let chain = chain();
        let (rpc, portal) = (
            serve(chain.clone()).await,
            serve_portal(chain.clone(), Quirks::default()).await,
        );
        let reader = PortalReader::new(&portal, "polygon-mainnet", None, 1_000.0).unwrap();
        let r = compare_sources::command(
            "postgres://nobody@127.0.0.1:1/unreachable",
            &crate::http::Outbound::for_tests(),
            Request {
                net: ChainNet::Matic,
                indexer: "nft".into(),
                from: 101,
                to: 400,
                window: 100,
                rpc_url: rpc,
                reader,
                history: AddressHistory::Unknown,
            },
        )
        .await;
        assert_eq!((r.outcome, r.exit_code()), ("incomplete", 3), "{r:#?}");
        assert_eq!(
            r.uncovered[0].reason,
            "database_unavailable: probe: database_error"
        );
        assert_eq!((r.uncovered[0].from, r.uncovered[0].to), (101, 400));
        assert_eq!(r.sends.rpc_requests, 0);
        assert_eq!(r.sends.portal, Default::default());
        assert_eq!(chain.requests.load(Ordering::SeqCst), 0);
        // A URL that does not parse is refused, never echoed.
        let reader = PortalReader::new(&portal, "polygon-mainnet", None, 1_000.0).unwrap();
        let r = compare_sources::command(
            "not a url with sk-example-db",
            &crate::http::Outbound::for_tests(),
            Request {
                net: ChainNet::Matic,
                indexer: "nft".into(),
                from: 101,
                to: 400,
                window: 100,
                rpc_url: "http://127.0.0.1:1".into(),
                reader,
                history: AddressHistory::Unknown,
            },
        )
        .await;
        assert_eq!((r.outcome, r.exit_code()), ("refused", 4));
        assert!(!serde_json::to_string(&r).unwrap().contains("sk-example-db"));
    }

    /// The widest window is clamped to the range, never overflowing.
    #[tokio::test]
    async fn the_widest_window_is_clamped_to_the_range() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let chain = chain();
        let (rpc, portal) = (
            serve(chain.clone()).await,
            serve_portal(chain, Quirks::default()).await,
        );
        let r = compare_with(&db, &rpc, &portal, "polygon-mainnet", 101, 400, u64::MAX).await;
        assert_eq!((r.outcome, r.exit_code()), ("equal", 0), "{r:#?}");
        assert_eq!(r.compared, [(101, 400)]);
        db.drop().await;
    }

    /// A mismatch known before any read is refused before the database is
    /// probed: down or not, `--net matic` against an Ethereum dataset is a
    /// refusal, not an incomplete run.
    #[tokio::test]
    async fn a_known_mismatch_is_refused_before_the_database_is_probed() {
        let chain = chain();
        let (rpc, portal) = (
            serve(chain.clone()).await,
            serve_portal(chain.clone(), Quirks::default()).await,
        );
        let reader = PortalReader::new(&portal, "ethereum-mainnet", None, 1_000.0).unwrap();
        let r = compare_sources::command(
            "postgres://nobody@127.0.0.1:1/unreachable",
            &crate::http::Outbound::for_tests(),
            Request {
                net: ChainNet::Matic,
                indexer: "nft".into(),
                from: 101,
                to: 400,
                window: 100,
                rpc_url: rpc,
                reader,
                history: AddressHistory::Unknown,
            },
        )
        .await;
        assert_eq!((r.outcome, r.exit_code()), ("refused", 4), "{r:#?}");
        assert_eq!(chain.requests.load(Ordering::SeqCst), 0);
    }

    /// The database the filters come from must be the network's: a cursor
    /// bound to a testnet is refused; a bound mainnet one is reported.
    #[tokio::test]
    async fn the_database_cursor_must_name_the_networks_chain() {
        let Some(db) = ScratchDb::create().await else {
            return;
        };
        let chain = chain();
        let (rpc, portal) = (
            serve(chain.clone()).await,
            serve_portal(chain.clone(), Quirks::default()).await,
        );
        sqlx::query(
            "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, full_history, chain_id) \
             VALUES ('mana_transfers', 'matic', 1, 1, true, 80002)",
        )
        .execute(&db.pool)
        .await
        .unwrap();
        let r = compare(&db, &rpc, &portal, 101, 400).await;
        assert_eq!((r.outcome, r.exit_code()), ("refused", 4), "{r:#?}");
        assert_eq!(r.sends.portal.pages, 0);
        sqlx::query("UPDATE shop.chain_cursor SET chain_id = 137 WHERE indexer = 'mana_transfers'")
            .execute(&db.pool)
            .await
            .unwrap();
        let r = compare(&db, &rpc, &portal, 101, 400).await;
        assert_eq!((r.outcome, r.database_chain), ("equal", Some(137)));
        db.drop().await;
    }

    /// The nft walk of seven collections, one address a request (seven
    /// chunks a window) under a budget of ten, then a collection learned
    /// late and backfilled by its lane: read from the RPC, or with a portal
    /// whose coverage lists every address. Returns each pass's outcome
    /// without its requests (and whether the requests it reported are the
    /// ones the RPC saw), the events written, and the eth_getLogs sent.
    async fn nft_walk(portal: bool) -> Option<(Vec<String>, Vec<(i64, i64, String)>, usize)> {
        let db = ScratchDb::create().await?;
        let s = nft_index::start_block(ChainNet::Matic);
        let contracts: Vec<String> = (1..=7).map(|i| format!("0x{:040x}", 0xd0 + i)).collect();
        for (i, c) in contracts.iter().enumerate() {
            let urn = format!("urn:decentraland:matic:collections-v2:{c}");
            seed_collection(&db.pool, "matic", c, s + 100, &urn, &format!("C{i}")).await;
        }
        let late = "0x00000000000000000000000000000000000000e8";
        let head = s + 30_000;
        let chain = FakeChain::new(
            head,
            vec![
                transfer(&contracts[0], ZERO, ALICE, v2_token(0, 1), s + 200, 0),
                transfer(&contracts[3], ZERO, BOB, v2_token(0, 1), s + 5_000, 0),
                transfer(&contracts[6], ZERO, CARL, v2_token(0, 1), s + 15_000, 1),
                transfer(late, ZERO, ALICE, v2_token(0, 7), s + 1_500, 0),
            ],
            10_000,
            1_000,
        );
        let url = serve(chain.clone()).await;
        let settings = IndexSettings {
            address_chunk: 1,
            ..index_settings(10, 10_000)
        };
        let st = state_over(db.pool.clone(), &url, settings.clone(), false);
        let admission = if portal {
            // Every address the walk reads: the collections, the late one
            // and the index's fixed contracts.
            let mut every: Vec<(String, u64)> = contracts.iter().map(|c| (c.clone(), 0)).collect();
            every.push((late.to_string(), 0));
            every.extend(
                nft_index::fixed_contracts(ChainNet::Matic)
                    .into_iter()
                    .map(|(a, _)| (a.to_ascii_lowercase(), 0)),
            );
            let (origin, hits, keyed) = keyed_portal(chain.clone(), every).await;
            let key = crate::chain_index::portal::Secret::new("sk-example123");
            Some((
                Arc::new(Admission::new(
                    PortalReader::new(&origin, "polygon-mainnet", key, 1_000.0).unwrap(),
                    "matic",
                    std::time::Duration::from_secs(5),
                )),
                hits,
                keyed,
            ))
        } else {
            None
        };
        let walk = async {
            let mut outcomes = Vec::new();
            let until_published = async |outcomes: &mut Vec<String>| {
                for _ in 0..40 {
                    let before = chain.requests.load(Ordering::SeqCst);
                    let o = pass(&st, &url, &NftIndex::new(ChainNet::Matic)).await;
                    let sent = chain.requests.load(Ordering::SeqCst) - before;
                    outcomes.push(format!(
                        "logs {} rows {} to +{} paused {} published {} reported {}",
                        o.logs,
                        o.rows_written,
                        o.indexed_to - s as i64,
                        o.paused,
                        o.published,
                        sent == o.requests
                    ));
                    if o.published {
                        return;
                    }
                }
                panic!("never published: {outcomes:?}");
            };
            until_published(&mut outcomes).await;
            seed_collection(
                &db.pool,
                "matic",
                late,
                s + 1_000,
                "urn:decentraland:matic:collections-v2:0xe8",
                "Late",
            )
            .await;
            *chain.head.lock() = head + 100;
            until_published(&mut outcomes).await;
            outcomes
        };
        let outcomes = TEST_PORTAL
            .scope(admission.as_ref().map(|(a, ..)| a.clone()), walk)
            .await;
        if let Some((admission, hits, keyed)) = &admission {
            // The key went to the portal and never to the RPC.
            assert!(keyed.load(Ordering::SeqCst) > 0);
            assert_eq!(chain.api_keys.load(Ordering::SeqCst), 0);
            // A carried fallback with nothing left reads nothing from either
            // source: it pauses before its first window.
            *chain.head.lock() = head + 20_000;
            let source = AdmittedPortal::new(
                chain_walk::RpcSource::new(Rpc::at(&st.http, &url)),
                Some(admission.as_ref()),
            );
            let (logs, asked) = (
                chain.get_logs.load(Ordering::SeqCst),
                hits.load(Ordering::SeqCst),
            );
            let out = chain_walk::job_pass_with(
                &db.pool,
                &source,
                &NftIndex::new(ChainNet::Matic),
                &settings,
                &Arc::default(),
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
            assert_eq!(chain.get_logs.load(Ordering::SeqCst), logs);
            assert_eq!(hits.load(Ordering::SeqCst), asked, "no portal request");
        }
        let events: Vec<(i64, i64, String)> = sqlx::query_as(
            "SELECT block_number, log_index::bigint, contract_address FROM shop.nft_event \
             WHERE network = 'matic' ORDER BY block_number, log_index",
        )
        .fetch_all(&db.pool)
        .await
        .unwrap();
        let get_logs = chain.get_logs.load(Ordering::SeqCst);
        db.drop().await;
        Some((outcomes, events, get_logs))
    }

    /// With a portal that covers every address, the walk pauses where the
    /// RPC walk pauses (seven chunks a window, a budget of ten), its lane
    /// backfills the late collection, it writes the same events, it reports
    /// the RPC requests it sent, and it sends no eth_getLogs.
    #[tokio::test]
    async fn a_portal_walk_pauses_and_indexes_where_the_rpc_walk_does() {
        let Some((rpc_outcomes, rpc_events, rpc_get_logs)) = nft_walk(false).await else {
            return;
        };
        let (outcomes, events, get_logs) = nft_walk(true).await.unwrap();
        assert_eq!(outcomes, rpc_outcomes);
        assert!(
            rpc_outcomes.iter().any(|o| o.contains("paused true")),
            "{rpc_outcomes:?}"
        );
        assert!(rpc_outcomes.iter().all(|o| o.ends_with("reported true")));
        assert_eq!(events, rpc_events);
        assert!(
            events
                .iter()
                .any(|(_, _, c)| c == "0x00000000000000000000000000000000000000e8")
        );
        assert!(rpc_get_logs > 0);
        assert_eq!(get_logs, 0, "every window and lane read from the portal");
    }

    /// A source whose windows charge `.1` times the requests they send: a
    /// portal's logical charge, apart from its sends.
    struct Charging<S>(S, usize);

    #[async_trait::async_trait]
    impl<S: chain_walk::LogSource> chain_walk::LogSource for Charging<S> {
        async fn chain_id(
            &self,
        ) -> Result<chain_walk::Read<u64>, crate::chain_index::rpc::RpcError> {
            self.0.chain_id().await
        }
        async fn head(&self) -> Result<chain_walk::Read<u64>, crate::chain_index::rpc::RpcError> {
            self.0.head().await
        }
        async fn finalized(
            &self,
        ) -> Result<
            chain_walk::Read<crate::chain_index::rpc::BlockHeader>,
            crate::chain_index::rpc::RpcError,
        > {
            self.0.finalized().await
        }
        async fn header(
            &self,
            number: u64,
        ) -> Result<
            chain_walk::Read<crate::chain_index::rpc::BlockHeader>,
            crate::chain_index::rpc::RpcError,
        > {
            self.0.header(number).await
        }
        async fn window(
            &self,
            from: u64,
            to: u64,
            groups: &[chain_walk::LogGroup],
            chunk: &mut usize,
        ) -> Result<chain_walk::Window, crate::chain_index::rpc::RpcError> {
            let mut w = self.0.window(from, to, groups, chunk).await?;
            w.charge = w.requests * self.1;
            Ok(w)
        }
        async fn block_times(
            &self,
            logs: &[crate::chain_index::rpc::RpcLog],
        ) -> Result<chain_walk::Read<HashMap<i64, i64>>, crate::chain_index::rpc::RpcError>
        {
            self.0.block_times(logs).await
        }
    }

    /// A lane pays its windows' charge, never less than their sends, and
    /// reports their sends: with each window charging three, a budget of
    /// four backfills one window a pass where one charging what it sends
    /// backfills two, as does one charging nothing; and each pass reports the
    /// requests the RPC saw.
    #[tokio::test]
    async fn a_lane_pays_its_windows_charge_and_reports_their_sends() {
        let mut reached = Vec::new();
        for charging in [None, Some(3), Some(0)] {
            let Some(db) = ScratchDb::create().await else {
                return;
            };
            let s = nft_index::start_block(ChainNet::Matic);
            let c1 = "0x00000000000000000000000000000000000000c1";
            let late = "0x00000000000000000000000000000000000000c3";
            seed_collection(
                &db.pool,
                "matic",
                c1,
                s + 100,
                "urn:decentraland:matic:collections-v2:0xc1",
                "Pirates",
            )
            .await;
            let chain = FakeChain::new(
                s + 30_000,
                vec![
                    transfer(c1, ZERO, ALICE, v2_token(0, 1), s + 200, 0),
                    transfer(late, ZERO, ALICE, v2_token(0, 7), s + 1_500, 0),
                ],
                10_000,
                1_000,
            );
            let url = serve(chain.clone()).await;
            let st = state_over(db.pool.clone(), &url, index_settings(4, 10_000), false);
            for _ in 0..40 {
                if pass(&st, &url, &NftIndex::new(ChainNet::Matic))
                    .await
                    .published
                {
                    break;
                }
            }
            seed_collection(
                &db.pool,
                "matic",
                late,
                s + 1_000,
                "urn:decentraland:matic:collections-v2:0xc3",
                "Late",
            )
            .await;
            // The pass that walks the new blocks learns the late
            // collection; the next one runs its lane.
            *chain.head.lock() = s + 30_100;
            pass(&st, &url, &NftIndex::new(ChainNet::Matic)).await;
            let ix = NftIndex::new(ChainNet::Matic);
            let rpc = chain_walk::RpcSource::new(Rpc::at(&st.http, &url));
            let owner = chain_walk::test_owner(&st.pool, &ix, &st.settings.market.indexes).await;
            let before = chain.requests.load(Ordering::SeqCst);
            let out = if let Some(factor) = charging {
                let source = Charging(rpc, factor);
                chain_walk::run_pass(
                    &st.pool,
                    &source,
                    &ix,
                    &st.settings.market.indexes,
                    &owner,
                    None,
                    &|_| {},
                )
                .await
                .unwrap()
            } else {
                chain_walk::run_pass(
                    &st.pool,
                    &rpc,
                    &ix,
                    &st.settings.market.indexes,
                    &owner,
                    None,
                    &|_| {},
                )
                .await
                .unwrap()
            };
            assert_eq!(
                out.requests,
                chain.requests.load(Ordering::SeqCst) - before,
                "the pass reports the requests the RPC saw"
            );
            let to: Option<i64> = sqlx::query_scalar(
                "SELECT backfilled_to_block FROM shop.nft_contract WHERE contract_address = $1",
            )
            .bind(late)
            .fetch_one(&db.pool)
            .await
            .unwrap();
            reached.push(to.map(|b| b - s as i64));
            db.drop().await;
        }
        assert_eq!(
            reached,
            [Some(19_999), Some(9_999), Some(19_999)],
            "two windows on the RPC's charge, one on three times it, two on none"
        );
    }
}
