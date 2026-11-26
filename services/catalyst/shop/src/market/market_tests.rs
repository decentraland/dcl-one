//! The market slice on the scratch database: a fake subgraph on the loopback
//! feeds the collections and items mirrors (walk, budget, failure, resume,
//! the readiness gate) and the readers, then every route group answers over
//! it; the favorites, admin and signed-trade flows run over the local tables.
//! Without `CATALYST_TEST_DATABASE_URL` the database tests skip.

mod nft_listings;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

use super::tests::support::{
    MIRROR_STATE_LOCK, serve_empty_graph, state_with_db, state_without_db_settings,
};
use super::{MarketSettings, Settings, SharedState, listings, lists, market_router, mirrors};
use crate::shared::signed_fetch::test_support::{self as sf, signed_headers};

const WALLET_A: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const WALLET_B: &str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const CREATOR: &str = "0xcccccccccccccccccccccccccccccccccccccccc";
const ITEM_ROWS: usize = 1005;

#[test]
fn debug_output_never_carries_the_market_admin_bearer() {
    let market = MarketSettings {
        admin_token: Some("s3cret-bearer".into()),
        ..MarketSettings::default()
    };
    assert!(!format!("{market:?}").contains("s3cret-bearer"));
}

#[tokio::test]
async fn mounted_shop_signatures_use_the_incoming_path() {
    let Some(state) = state_with_db(Settings::for_tests()).await else {
        return;
    };
    for mount in ["/shop-api", "/marketplace-api"] {
        let app = Router::new().nest(mount, market_router(state.clone()));
        let path = format!("{mount}/v1/lists");
        for v6 in [false, true] {
            let headers = signed_headers("get", &path, "{}", v6).await;
            let (status, body) = call(
                app.clone(),
                "GET",
                &format!("{path}?limit=1"),
                headers,
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{mount}: {body}");
            assert_eq!(body["data"]["results"][0]["id"], lists::DEFAULT_LIST_ID);
        }
        for (method, signed_path) in [
            ("post", path.as_str()),
            ("get", "/other/v1/lists"),
            ("get", "/shop-api/v1/picks/stats"),
        ] {
            let mut headers = signed_headers(method, signed_path, "{}", true).await;
            headers.insert("x-original-path", signed_path.parse().unwrap());
            let (status, _) = call(app.clone(), "GET", &path, headers, None).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {signed_path}");
        }
        // Older callers sign the service-relative route; retain that contract.
        let headers = signed_headers("get", "/v1/lists", "{}", true).await;
        assert_eq!(
            call(app, "GET", &path, headers, None).await.0,
            StatusCode::OK
        );
    }
}

fn tag() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

fn contract_of(t: &str, n: u8) -> String {
    format!("0x{:0>38}{n:02}", &t[..30]).to_lowercase()
}

async fn call(
    router: Router,
    method: &str,
    uri: &str,
    headers: HeaderMap,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(uri);
    for (k, v) in headers.iter() {
        req = req.header(k, v);
    }
    let req = match body {
        Some(b) => req
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&b).unwrap()))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let resp = router.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).to_string()))
    };
    (status, value)
}

/// Whether the catalogue gate lets a descriptive read of the items through.
async fn items_ready(state: &super::MarketplaceState) -> Result<(), axum::response::Response> {
    super::catalogue::begin(
        state,
        super::catalogue::Purpose::Describe,
        &[mirrors::Mirror::Items],
        None,
        super::catalogue::Access::ReadOnly,
    )
    .await
    .map(|_| ())
}

async fn get(router: Router, uri: &str) -> (StatusCode, Value) {
    call(router, "GET", uri, HeaderMap::new(), None).await
}

/// A GET and its `Cache-Control` (empty when the response sets none).
async fn get_cached(router: Router, uri: &str) -> (Value, String) {
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "{uri}");
    let cache = resp
        .headers()
        .get(header::CACHE_CONTROL)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    (serde_json::from_slice(&bytes).unwrap(), cache)
}

fn bearer(token: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(
        header::AUTHORIZATION,
        format!("Bearer {token}").parse().unwrap(),
    );
    h
}

/// One GraphQL endpoint answering every entity the mirrors and readers ask
/// for from canned rows: `items` and `collections` by their keysets, the
/// rest by `first`/`skip`/`id_gt`. `fail_next_items_page` makes every
/// items page past the first answer 500 (through the client's retries) until cleared;
/// `hang` makes every request wait without an answer; `pause` holds the
/// next request between `arrived` and `release`.
struct FakeGraph {
    rows: std::sync::Mutex<BTreeMap<&'static str, Vec<Value>>>,
    fail_next_items_page: AtomicBool,
    hang: AtomicBool,
    pause: AtomicBool,
    arrived: tokio::sync::Notify,
    release: tokio::sync::Notify,
    requests: std::sync::Mutex<Vec<String>>,
}

impl FakeGraph {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            rows: std::sync::Mutex::new(BTreeMap::new()),
            fail_next_items_page: AtomicBool::new(false),
            hang: AtomicBool::new(false),
            pause: AtomicBool::new(false),
            arrived: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
            requests: std::sync::Mutex::new(Vec::new()),
        })
    }

    fn set(&self, entity: &'static str, rows: Vec<Value>) {
        self.rows.lock().unwrap().insert(entity, rows);
    }

    fn entity_of(query: &str) -> String {
        let after = &query[query.find('{').map(|i| i + 1).unwrap_or(0)..];
        after
            .trim_start()
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect()
    }

    fn answer(&self, query: &str, vars: &Value) -> Result<Value, ()> {
        let entity = Self::entity_of(query);
        self.requests.lock().unwrap().push(entity.clone());
        // The Graph's own bounds: a request past them is a GraphQL error
        // (HTTP 200), never a longer page.
        for (arg, max) in [("first", 1000), ("skip", 5000)] {
            if let Some(n) = vars
                .get(arg)
                .and_then(Value::as_i64)
                .filter(|n| *n < 0 || *n > max)
            {
                return Ok(
                    json!({ "errors": [{ "message": format!("The `{arg}` argument must be between 0 and {max}, but is {n}") }] }),
                );
            }
        }
        let all = self
            .rows
            .lock()
            .unwrap()
            .get(entity.as_str())
            .cloned()
            .unwrap_or_default();
        let first = vars.get("first").and_then(Value::as_i64).unwrap_or(1000) as usize;
        let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let rows: Vec<Value> = match entity.as_str() {
            "items" => {
                let from = s(vars, "idFrom");
                if !from.is_empty() && self.fail_next_items_page.load(Ordering::SeqCst) {
                    return Err(());
                }
                let mut rows: Vec<Value> = all.into_iter().filter(|r| s(r, "id") > from).collect();
                rows.sort_by_key(|a| s(a, "id"));
                rows.into_iter().take(first).collect()
            }
            "collections" => {
                let from = s(vars, "idFrom");
                let mut rows: Vec<Value> = all.into_iter().filter(|r| s(r, "id") > from).collect();
                rows.sort_by_key(|a| s(a, "id"));
                rows.into_iter().take(first).collect()
            }
            _ => {
                let where_ = vars.get("where").cloned().unwrap_or(json!({}));
                let id_gt = s(&where_, "id_gt");
                let skip = vars.get("skip").and_then(Value::as_i64).unwrap_or(0) as usize;
                fn matches(r: &Value, where_: &Value) -> bool {
                    where_.as_object().into_iter().flatten().all(|(key, want)| {
                        let field = |name: &str| match r.get(name) {
                            Some(Value::Object(o)) => {
                                o.get("address").cloned().unwrap_or(Value::Null)
                            }
                            Some(v) => v.clone(),
                            None => Value::Null,
                        };
                        let same = |have: &Value, want: &Value| {
                            have == want
                                || have
                                    .as_str()
                                    .map(|h| h.eq_ignore_ascii_case(want.as_str().unwrap_or("")))
                                    .unwrap_or(false)
                                || *have == *want
                        };
                        let number = |v: &Value| {
                            v.as_i64()
                                .map(i128::from)
                                .or_else(|| v.as_str().and_then(|s| s.parse::<i128>().ok()))
                        };
                        let bound = ["_lte", "_gte", "_lt", "_gt"]
                            .iter()
                            .find_map(|op| key.strip_suffix(op).map(|name| (name, *op)));
                        if key == "or" {
                            want.as_array().is_some_and(|branches| {
                                branches.iter().any(|branch| matches(r, branch))
                            })
                        } else if let Some(name) = key.strip_suffix("_in") {
                            want.as_array()
                                .is_some_and(|list| list.iter().any(|w| same(&field(name), w)))
                        } else if let Some((name, op)) = bound.filter(|(name, _)| *name != "id") {
                            match (number(&field(name)), number(want)) {
                                (Some(have), Some(want)) => match op {
                                    "_lte" => have <= want,
                                    "_gte" => have >= want,
                                    "_lt" => have < want,
                                    _ => have > want,
                                },
                                _ => true,
                            }
                        } else if key.contains('_') {
                            true
                        } else {
                            same(&field(key), want)
                        }
                    })
                }
                let mut rows: Vec<Value> = all
                    .into_iter()
                    .filter(|r| (id_gt.is_empty() || s(r, "id") > id_gt) && matches(r, &where_))
                    .collect();
                let ident = |after: &str| {
                    query.find(after).map(|i| {
                        query[i + after.len()..]
                            .chars()
                            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                            .collect::<String>()
                    })
                };
                let order_by = if id_gt.is_empty() {
                    ident("orderBy: ").unwrap_or_else(|| "id".to_string())
                } else {
                    "id".to_string()
                };
                let desc = ident("orderDirection: ").is_some_and(|d| d == "desc");
                rows.sort_by(|a, b| {
                    let (x, y) = (
                        a.get(&order_by).cloned().unwrap_or(Value::Null),
                        b.get(&order_by).cloned().unwrap_or(Value::Null),
                    );
                    let numeric = |v: &Value| {
                        v.as_i64()
                            .or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok()))
                    };
                    let ord = match (numeric(&x), numeric(&y)) {
                        (Some(p), Some(q)) => p.cmp(&q),
                        _ => x.to_string().cmp(&y.to_string()),
                    };
                    if desc { ord.reverse() } else { ord }
                });
                rows.into_iter().skip(skip).take(first).collect()
            }
        };
        Ok(json!({ "data": { entity: rows } }))
    }
}

async fn serve_graph(up: Arc<FakeGraph>) -> String {
    use axum::extract::State;
    use axum::routing::post;
    async fn gql(
        State(up): State<Arc<FakeGraph>>,
        axum::Json(body): axum::Json<Value>,
    ) -> axum::response::Response {
        if up.hang.load(Ordering::SeqCst) {
            std::future::pending::<()>().await;
        }
        if up.pause.swap(false, Ordering::SeqCst) {
            up.arrived.notify_one();
            up.release.notified().await;
        }
        let query = body.get("query").and_then(Value::as_str).unwrap_or("");
        let vars = body.get("variables").cloned().unwrap_or(json!({}));
        match up.answer(query, &vars) {
            Ok(v) => axum::Json(v).into_response(),
            Err(()) => (StatusCode::INTERNAL_SERVER_ERROR, "boom").into_response(),
        }
    }
    use axum::response::IntoResponse;
    let app = Router::new().route("/", post(gql)).with_state(up);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}/")
}

pub(super) async fn job_ctx(jobs: &crate::jobs::Jobs) -> crate::jobs::JobContext {
    let (tx, rx) = tokio::sync::oneshot::channel();
    jobs.spawn("ctx", move |ctx| async move {
        let _ = tx.send(ctx);
        Ok(())
    });
    rx.await.unwrap()
}

/// The matic fake carries the rows; the ethereum reader is configured and
/// empty, since a route without a network filter needs both.
fn market_settings(url: &str, eth_url: &str, writes: bool) -> Settings {
    Settings {
        matic_collections_subgraph_url: Some(url.to_string()),
        eth_collections_subgraph_url: Some(eth_url.to_string()),
        data_writes_enabled: writes,
        ..Settings::for_tests()
    }
    .with_market(MarketSettings {
        mirrors_enabled: true,
        mirror_pages_per_tick: 1,
        catalog_cache_ttl: Duration::ZERO,
        admin_token: Some("s3cr3t".to_string()),
        ..MarketSettings::default()
    })
}

/// The market settings over `url` on a database of its own with the nft
/// walk on: the test publishes the index there without touching the
/// shared one.
async fn state_over_own(
    url: &str,
    writes: bool,
) -> Option<(SharedState, super::tests::support::OwnDb)> {
    let own = super::tests::support::OwnDb::create("mirrors").await?;
    let eth_url = serve_empty_graph().await;
    let settings =
        super::owned_items::test_support::walking(market_settings(url, &eth_url, writes));
    let state = std::sync::Arc::new(super::MarketplaceState::new(
        own.pool.clone(),
        crate::http::Outbound::for_tests(),
        settings,
    ));
    state.test_allow_loopback.store(true, Ordering::Relaxed);
    Some((state, own))
}

struct Asset {
    asset_type: i32,
    contract: String,
    value: String,
}

async fn insert_trade(
    pool: &PgPool,
    kind: &str,
    signer: &str,
    sent: Asset,
    received: Asset,
) -> (String, String) {
    insert_trade_on(pool, "MATIC", 137, kind, signer, sent, received).await
}

/// [`insert_trade`] on another network (`ETHEREUM`, 1).
async fn insert_trade_on(
    pool: &PgPool,
    network: &str,
    chain_id: i64,
    kind: &str,
    signer: &str,
    sent: Asset,
    received: Asset,
) -> (String, String) {
    let id = uuid::Uuid::new_v4().to_string();
    let signature = format!("0x{}", tag()).to_lowercase();
    let hashed = listings::hashed_signature(&signature);
    sqlx::query(
        "INSERT INTO shop.trades (id, network, chain_id, signature, hashed_signature, checks, signer, type, expires_at, effective_since, contract, created_at) \
         VALUES ($1::uuid, $7, $8, $2, $3, '{\"uses\": 1}'::jsonb, $4, $5::shop.trade_type, timezone('UTC', now()) + interval '30 days', timezone('UTC', now()) - interval '1 day', $6, timezone('UTC', now()))",
    )
    .bind(&id)
    .bind(&signature)
    .bind(&hashed)
    .bind(signer)
    .bind(kind)
    .bind(listings::offchain_marketplace_v3(chain_id).unwrap().address)
    .bind(network)
    .bind(chain_id)
    .execute(pool)
    .await
    .unwrap();
    for (direction, a) in [("sent", sent), ("received", received)] {
        let asset_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO shop.trade_assets (id, trade_id, direction, asset_type, contract_address, beneficiary, extra) VALUES ($1::uuid, $2::uuid, $3::shop.asset_direction_type, $4, $5, $6, '')")
            .bind(&asset_id)
            .bind(&id)
            .bind(direction)
            .bind(a.asset_type as i16)
            .bind(&a.contract)
            .bind(signer)
            .execute(pool)
            .await
            .unwrap();
        let (table, column) = match a.asset_type {
            3 => ("trade_assets_erc721", "token_id"),
            4 => ("trade_assets_item", "item_id"),
            _ => ("trade_assets_erc20", "amount"),
        };
        let cast = if a.asset_type == 1 || a.asset_type == 2 {
            "::numeric"
        } else {
            ""
        };
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "INSERT INTO shop.{table} (asset_id, {column}) VALUES ($1::uuid, $2{cast})"
        )))
        .bind(&asset_id)
        .bind(&a.value)
        .execute(pool)
        .await
        .unwrap();
    }
    if matches!(kind, "public_item_order" | "public_nft_order") {
        crate::test_db::open_trade_verdict(pool, &id).await;
    }
    (id, hashed)
}

async fn delete_trade(pool: &PgPool, id: &str) {
    sqlx::query("DELETE FROM shop.trade_cancellations WHERE target_hashed_signature IN (SELECT hashed_signature FROM shop.trades WHERE id = $1::uuid)").bind(id).execute(pool).await.unwrap();
    sqlx::query("DELETE FROM shop.trades WHERE id = $1::uuid")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
}

async fn cleanup_mirror(pool: &PgPool, contracts: &[String]) {
    sqlx::query("DELETE FROM shop.picks WHERE split_part(item_id, '-', 1) = ANY($1)")
        .bind(contracts)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM shop.items WHERE contract_address = ANY($1)")
        .bind(contracts)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM shop.collections WHERE contract_address = ANY($1)")
        .bind(contracts)
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn upgrading_a_completed_mainnet_mirror_keeps_the_catalogue_available() {
    let url = format!("{}missing", serve_graph(FakeGraph::new()).await);
    let Some((state, own)) = state_over_own(&url, true).await else {
        return;
    };
    sqlx::query("INSERT INTO shop.mirror_state(mirror,network,source_url,last_complete_pass_at,last_complete_pass_rows,cursor_id)
                 VALUES ('items','matic',$1,timezone('UTC',now()),17,'old-key'),
                        ('collections','ethereum','https://subgraph.decentraland.org/collections-ethereum-mainnet',timezone('UTC',now()),19,'key')")
        .bind(&url).execute(&state.pool).await.unwrap();
    sqlx::raw_sql(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/schemas/migrations/0070_shop_mirror_chain.sql"
    )))
    .execute(&state.pool)
    .await
    .unwrap();
    assert!(
        super::nft_index::mirror_complete(&state.pool, super::chain_walk::ChainNet::Ethereum, 1)
            .await
    );
    sqlx::query("UPDATE shop.mirror_state SET pass_started_at=last_complete_pass_at+interval '1 second' WHERE mirror='items'")
        .execute(&state.pool).await.unwrap();
    let before = mirrors::load(
        &state.pool,
        mirrors::Mirror::Items,
        super::graph::Network::Matic,
    )
    .await
    .unwrap()
    .unwrap();
    // The first page at the custom mainnet source fails. The already served
    // copy and the keyset for its in-progress pass must survive a retry.
    let jobs = crate::jobs::Jobs::new();
    let ctx = job_ctx(&jobs).await;
    assert!(
        mirrors::run_tick(&state, mirrors::Mirror::Items, &ctx)
            .await
            .is_err()
    );
    let after = mirrors::load(
        &state.pool,
        mirrors::Mirror::Items,
        super::graph::Network::Matic,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(after.chain_id, Some(137));
    assert_eq!(after.last_complete_pass_at, before.last_complete_pass_at);
    assert_eq!(after.last_complete_pass_rows, Some(17));
    assert_eq!(after.cursor_id.as_deref(), Some("old-key"));
    assert!(items_ready(&state).await.is_ok());
    own.drop().await;
}

#[tokio::test]
async fn the_mirrors_walk_the_subgraph_and_every_market_route_answers_over_them() {
    let up = FakeGraph::new();
    let url = serve_graph(up.clone()).await;
    let Some((state, own)) = state_over_own(&url, true).await else {
        return;
    };
    let pool = state.pool.clone();
    super::trade_events::tests::publish_cursors(&pool).await;
    let t = tag();
    let (c1, c2, c_nft) = (contract_of(&t, 1), contract_of(&t, 2), contract_of(&t, 3));
    let v3_minter = listings::offchain_marketplace_v3(137).unwrap().address;

    let mut collections = vec![
        mirrors::collections::fixture(&c1, "First", CREATOR, true),
        mirrors::collections::fixture(&c2, "Second", CREATOR, true),
    ];
    for c in &mut collections {
        c["minters"] = json!([v3_minter]);
    }
    up.set("collections", collections);
    let items: Vec<Value> = (0..ITEM_ROWS)
        .map(|i| {
            let (contract, kind) = if i % 5 == 0 {
                (&c2, "emote_v1")
            } else {
                (&c1, "wearable_v2")
            };
            let mut row =
                mirrors::items::fixture(contract, &i.to_string(), kind, &format!("Item {i:04}"));
            row["updatedAt"] = json!((1_700_000_000 + i as i64 / 3).to_string());
            row
        })
        .collect();
    up.set("items", items);

    let jobs = crate::jobs::Jobs::new();
    let ctx = job_ctx(&jobs).await;
    let _mirror_rows = MIRROR_STATE_LOCK.lock().await;
    sqlx::query("DELETE FROM shop.mirror_state WHERE mirror IN ('collections', 'items')")
        .execute(&pool)
        .await
        .unwrap();

    // Before any pass the mirror-served routes answer the envelope, never an empty page.
    let (status, body) = get(market_router(state.clone()), "/v2/catalog?first=2").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["source"], "items-mirror");
    assert_eq!(body["configure"], "MARKETPLACE_MIRRORS_ENABLED");

    mirrors::run_tick(&state, mirrors::Mirror::Collections, &ctx)
        .await
        .unwrap();
    let c = mirrors::load(
        &pool,
        mirrors::Mirror::Collections,
        super::graph::Network::Matic,
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        c.last_complete_pass_at.is_some() && c.rows_upserted == 2 && c.pages_fetched == 1,
        "{c:?}"
    );

    // Tick 1: one page of 1000 (the budget), the pass stays open at the persisted keyset.
    mirrors::run_tick(&state, mirrors::Mirror::Items, &ctx)
        .await
        .unwrap();
    let c = mirrors::load(&pool, mirrors::Mirror::Items, super::graph::Network::Matic)
        .await
        .unwrap()
        .unwrap();
    assert!(
        c.in_progress()
            && c.last_complete_pass_at.is_none()
            && c.pages_fetched == 1
            && c.rows_upserted == 1000,
        "{c:?}"
    );
    assert!(
        c.cursor_id
            .as_deref()
            .is_some_and(|id| id.starts_with(&c1) || id.starts_with(&c2))
            && c.cursor_updated_at.is_none(),
        "the id keyset is persisted per page (a whole walk, no updatedAt): {c:?}"
    );
    let (status, body) = get(market_router(state.clone()), "/v2/catalog?first=2").await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "no complete pass yet: {body}"
    );

    // Tick 2 fails upstream: the error lands on the row, the cursor does not move.
    up.fail_next_items_page.store(true, Ordering::SeqCst);
    let err = mirrors::run_tick(&state, mirrors::Mirror::Items, &ctx)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("items mirror"), "{err}");
    up.fail_next_items_page.store(false, Ordering::SeqCst);
    let c = mirrors::load(&pool, mirrors::Mirror::Items, super::graph::Network::Matic)
        .await
        .unwrap()
        .unwrap();
    assert!(c.last_error.is_some() && c.pages_fetched == 1, "{c:?}");

    // Tick 3 resumes at the keyset: the short page completes the pass and only the tail was re-read.
    up.requests.lock().unwrap().clear();
    mirrors::run_tick(&state, mirrors::Mirror::Items, &ctx)
        .await
        .unwrap();
    assert_eq!(
        up.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.as_str() == "items")
            .count(),
        1
    );
    let c = mirrors::load(&pool, mirrors::Mirror::Items, super::graph::Network::Matic)
        .await
        .unwrap()
        .unwrap();
    assert!(
        !c.in_progress() && c.last_complete_pass_at.is_some() && c.last_error.is_none(),
        "{c:?}"
    );
    assert_eq!((c.pages_fetched, c.rows_upserted), (2, ITEM_ROWS as i64));
    let mirrored: i64 =
        sqlx::query_scalar("SELECT count(*) FROM shop.items WHERE contract_address = ANY($1)")
            .bind(vec![c1.clone(), c2.clone()])
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(mirrored, ITEM_ROWS as i64);
    // The resale's item: the feed joins every trade to its mirrored item.
    sqlx::query(
        "INSERT INTO shop.items SELECT (jsonb_populate_record(NULL::shop.items, to_jsonb(i) \
           || jsonb_build_object('contract_address', $2::text, 'item_id', 2, 'urn', $3::text))).* \
         FROM shop.items i WHERE i.contract_address = $1 AND i.item_id = 1",
    )
    .bind(&c1)
    .bind(&c_nft)
    .bind(format!("urn:decentraland:matic:collections-v2:{c_nft}:2"))
    .execute(&pool)
    .await
    .unwrap();

    // A native (USD-pegged) primary listing, a MANA nft resale and a bid over the mirrored items.
    let (primary_id, primary_hash) = insert_trade(
        &pool,
        "public_item_order",
        CREATOR,
        Asset {
            asset_type: 4,
            contract: c1.clone(),
            value: "1".into(),
        },
        Asset {
            asset_type: 2,
            contract: "0x2222222222222222222222222222222222222222".into(),
            value: "250000000000000000".into(),
        },
    )
    .await;
    let token_1_of_item_2 = format!(
        "{}",
        alloy_primitives::U256::from(2u64) << 216u32 | alloy_primitives::U256::from(7u64)
    );
    let (status, body) = get(
        market_router(state.clone()),
        "/v1/prices?assetType=nft&category=wearable",
    )
    .await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("nft-index")),
        "the nft leg waits for the nft index to publish"
    );
    super::owned_items::test_support::publish(
        &pool,
        &super::chain_walk::ChainNet::ALL,
        false,
        Duration::ZERO,
    )
    .await;
    sqlx::query(
        "INSERT INTO shop.nft (network, contract_address, token_id, owner, item_id, issued_id, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
         VALUES ('matic', $1, $2::numeric, $3, 2, 7, 10, 1700000000, 10, 0, 1700000000)",
    )
    .bind(&c_nft)
    .bind(&token_1_of_item_2)
    .bind(WALLET_A)
    .execute(&pool)
    .await
    .unwrap();
    let (resale_id, _) = insert_trade(
        &pool,
        "public_nft_order",
        WALLET_A,
        Asset {
            asset_type: 3,
            contract: c_nft.clone(),
            value: token_1_of_item_2,
        },
        Asset {
            asset_type: 1,
            contract: "0x2222222222222222222222222222222222222222".into(),
            value: "3000000000000000000".into(),
        },
    )
    .await;
    let (bid_id, _) = insert_trade(
        &pool,
        "bid",
        WALLET_B,
        Asset {
            asset_type: 1,
            contract: "0x2222222222222222222222222222222222222222".into(),
            value: "9000000000000000000".into(),
        },
        Asset {
            asset_type: 4,
            contract: c1.clone(),
            value: "3".into(),
        },
    )
    .await;

    let app = || market_router(state.clone());

    // The catalog, its v1 shape and the picks join.
    let (status, body) = get(
        app(),
        &format!("/v2/catalog?first=2&contractAddress={c1}&sortBy=newest"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"].as_array().unwrap().len(), 2);
    assert!(body["total"].as_i64().unwrap() >= 800, "{}", body["total"]);
    let first = &body["data"][0];
    assert_eq!(first["contractAddress"], c1);
    assert_eq!(
        first["isOnSale"], true,
        "a store-minted item with stock is on sale: {first}"
    );
    assert!(
        ["5000000000000000000", "250000000000000000"].contains(&first["price"].as_str().unwrap()),
        "the store price, or the open v3 listing's: {first}"
    );
    assert_eq!(first["network"], "MATIC");
    assert!(
        first["urn"]
            .as_str()
            .unwrap()
            .starts_with("urn:decentraland:matic:collections-v2:")
    );
    assert!(first.get("picks").is_some());
    let (status, body) = get(app(), &format!("/v2/catalog?id={c1}-1")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"][0]["id"], format!("{c1}-1"));
    assert_eq!(
        body["data"][0]["tradeId"].as_str(),
        Some(primary_id.as_str()),
        "the v3 primary listing rides the item: {}",
        body["data"][0]
    );
    let (status, body) = get(
        app(),
        &format!("/v1/catalog?first=1&contractAddress={c2}&category=emote"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body["data"][0]["data"]["emote"]["loop"].is_boolean(),
        "{}",
        body["data"][0]
    );

    // /v1/items and the credit-priced /v3/catalog/items: the credit routes need a
    // MANA spot younger than the budget (the price envelope otherwise, never a
    // constant rate); one is seeded at 0.02 and swept at the end.
    let (status, body) = get(
        app(),
        &format!("/v3/catalog/items?contractAddress={c1}&first=3&sortBy=cheapest"),
    )
    .await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("mana-price-poll")),
        "no spot, no credit price: {body}"
    );
    let _spot = crate::price::test_support::SNAPSHOTS.lock().await;
    sqlx::query("INSERT INTO economy.price_snapshots (source, mana_usd, taken_at) VALUES ('coingecko', 0.02, timezone('UTC', now()))").execute(&pool).await.unwrap();
    state.readers.clear();
    let (status, body) = get(app(), &format!("/v1/items?contractAddress={c1}&itemId=4")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (body["data"][0]["itemId"].as_str(), body["total"].as_i64()),
        (Some("4"), Some(1))
    );
    assert_eq!(body["data"][0]["data"]["wearable"]["category"], "hat");
    let (status, body) = get(
        app(),
        &format!("/v3/catalog/items?contractAddress={c1}&first=3&sortBy=cheapest"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["data"][0]["priceCredits"], 1,
        "5 MANA at the seeded 0.02 spot is one credit: {}",
        body["data"][0]
    );
    // A price no credit amount can hold (Amoy carries an item at 2^256-1 wei)
    // leaves its row in the page, priced in no credits, instead of failing the
    // whole page on the bigint cast.
    let priced = body["data"][0]["itemId"].as_str().unwrap().to_string();
    let price: String = sqlx::query_scalar(
        "SELECT price::text FROM shop.items WHERE contract_address = $1 AND item_id = $2::numeric",
    )
    .bind(c1.clone())
    .bind(&priced)
    .fetch_one(&pool)
    .await
    .unwrap();
    let set_price = |value: String| {
        sqlx::query(
            "UPDATE shop.items SET price = $1::numeric WHERE contract_address = $2 AND item_id = $3::numeric",
        )
        .bind(value)
        .bind(c1.clone())
        .bind(priced.clone())
        .execute(&pool)
    };
    set_price(
        "115792089237316195423570985008687907853269984665640564039457584007913129639935".into(),
    )
    .await
    .unwrap();
    state.readers.clear();
    let (status, body) = get(
        app(),
        &format!("/v3/catalog/items?contractAddress={c1}&first=1000"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "an absurd price fails no page: {body}"
    );
    let row = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["itemId"] == priced.as_str())
        .cloned()
        .unwrap_or_else(|| panic!("item {priced} stays in the page: {body}"));
    assert_eq!(row["priceCredits"], 0, "priced in no credits: {row}");
    set_price(price).await.unwrap();
    state.readers.clear();
    // isOnSale=false is the complement of isOnSale=true, not the whole list:
    // item 4 sold out is the one item of c1 not on sale.
    let sold_out = |available: i32| {
        sqlx::query(
            "UPDATE shop.items SET available = $1 WHERE contract_address = $2 AND item_id = '4'",
        )
        .bind(available)
        .bind(c1.clone())
        .execute(&pool)
    };
    let stock: i32 = sqlx::query_scalar(
        "SELECT available::int FROM shop.items WHERE contract_address = $1 AND item_id = '4'",
    )
    .bind(c1.clone())
    .fetch_one(&pool)
    .await
    .unwrap();
    sold_out(0).await.unwrap();
    state.readers.clear();
    let total = |body: &Value| body["total"].as_i64().unwrap();
    let (_, all) = get(
        app(),
        &format!("/v3/catalog/items?contractAddress={c1}&first=1000"),
    )
    .await;
    let (_, on) = get(
        app(),
        &format!("/v3/catalog/items?contractAddress={c1}&first=1000&isOnSale=true"),
    )
    .await;
    let (status, off) = get(
        app(),
        &format!("/v3/catalog/items?contractAddress={c1}&first=1000&isOnSale=false"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{off}");
    assert_eq!(total(&off), 1, "{off}");
    assert_eq!(off["data"][0]["itemId"], "4", "{off}");
    assert_eq!(off["data"][0]["isOnSale"], false, "{off}");
    assert_eq!(
        total(&on) + total(&off),
        total(&all),
        "on {} / all {}",
        total(&on),
        total(&all)
    );
    sold_out(stock).await.unwrap();
    state.readers.clear();

    // The collections mirror behind /v1/contracts and /v1/collections.
    let (status, body) = get(app(), "/v1/contracts?first=1000").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["address"] == c1 && c["name"] == "First"),
        "{body}"
    );
    let (status, body) = get(app(), &format!("/v1/collections?contractAddress={c2}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (
            body["data"][0]["name"].as_str(),
            body["data"][0]["chainId"].as_i64()
        ),
        (Some("Second"), Some(137))
    );

    // The shop feeds: the native listing in credits, the store mints in the unified item feed.
    let (status, body) = get(app(), &format!("/v3/catalog/shop?contractAddress={c1}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["total"], 1);
    let listing = &body["data"][0];
    assert_eq!(listing["tradeId"].as_str(), Some(primary_id.as_str()));
    assert_eq!(
        (
            listing["priceCredits"].as_i64(),
            listing["listingType"].as_str(),
            listing["gender"].as_str()
        ),
        (Some(3), Some("primary"), Some("male"))
    );
    assert!(listing["coupon"].is_null() && listing["compareAtCredits"].is_null());
    // `creator` takes a comma-separated list, any of which matches, in any case (the Shop's creators filter).
    let stranger = "0x00000000000000000000000000000000000000aa";
    for feed in ["/v3/catalog/shop?", "/v3/catalog/unified?groupBy=item&"] {
        let (_, all) = get(app(), &format!("{feed}contractAddress={c1}")).await;
        let listed = all["total"].as_i64().unwrap();
        assert!(listed > 0, "{feed}: {all}");
        for (creators, want) in [
            (
                format!("{stranger},0x{}", CREATOR[2..].to_uppercase()),
                listed,
            ),
            (stranger.to_string(), 0),
        ] {
            let (status, body) = get(
                app(),
                &format!("{feed}contractAddress={c1}&creator={creators}"),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{body}");
            assert_eq!(
                body["total"].as_i64(),
                Some(want),
                "{feed}creator={creators}: {body}"
            );
        }
    }
    // The creator's live coupon discounts the listing, but only one signed
    // against the manager paired with the marketplace the listing settles on
    // (the V3 trade here): the bigger V2-manager coupon never applies.
    let mut coupon_ids: Vec<String> = Vec::new();
    for (manager, ppm) in [
        ("0x3fd3056ee72a2a85e9392fab3a450e7736536081", 700_000),
        ("0x655fdfa91d69ea49f4ce1a8f7f7e2622c8630813", 500_000),
    ] {
        let sig = format!("0x{}", tag());
        let id: String = sqlx::query_scalar(
            "INSERT INTO shop.coupons (network, chain_id, signer, signature, hashed_signature, state_key, coupon_manager, coupon_address, checks, \
                discount_type, discount_ppm, root, collections, effective_since, expires_at) \
             VALUES ('MATIC', 137, $1, $2, $2, $2, $3, '0xc914507fe297b2dddd1232ac3a8903f1c125e794', '{\"uses\": 10}'::jsonb, 1, $4, '0x00', ARRAY[$5], \
                timezone('UTC', now()) - interval '1 day', timezone('UTC', now()) + interval '1 day') RETURNING id::text",
        )
        .bind(CREATOR)
        .bind(&sig)
        .bind(manager)
        .bind(ppm)
        .bind(&c1)
        .fetch_one(&pool)
        .await
        .unwrap();
        coupon_ids.push(id);
    }
    let (status, body) = get(
        app(),
        &format!("/v3/catalog/shop?contractAddress={c1}&discounted=true"),
    )
    .await;
    assert_eq!(
        (status, body["total"].as_i64()),
        (StatusCode::OK, Some(1)),
        "{body}"
    );
    let sale = &body["data"][0];
    assert_eq!(
        (
            sale["priceCredits"].as_i64(),
            sale["compareAtCredits"].as_i64()
        ),
        (Some(2), Some(3)),
        "half of 0.25 USD rounds up to 2 credits: {sale}"
    );
    assert_eq!(
        (
            sale["coupon"]["discount"].as_i64(),
            sale["coupon"]["couponManager"].as_str()
        ),
        (
            Some(500_000),
            Some("0x655fdfa91d69ea49f4ce1a8f7f7e2622c8630813")
        ),
        "{sale}"
    );
    assert_eq!(
        sale["coupon"]["proof"],
        json!([]),
        "a one-collection coupon needs no proof: {sale}"
    );
    assert!(
        sale["saleEndsAt"].as_i64().unwrap() > chrono::Utc::now().timestamp()
            && sale["saleUnitsLeft"].as_i64().unwrap() <= 10,
        "{sale}"
    );
    let (_, body) = get(
        app(),
        &format!("/v3/catalog/shop?contractAddress={c1}&discounted=false"),
    )
    .await;
    assert_eq!(
        body["total"], 0,
        "the discounted listing is not a full-price one: {body}"
    );
    sqlx::query("DELETE FROM shop.coupons WHERE id::text = ANY($1)")
        .bind(&coupon_ids)
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = get(
        app(),
        &format!("/v3/catalog/unified?groupBy=item&contractAddress={c1}&first=2&sortBy=cheapest"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let item = &body["data"][0];
    assert_eq!(
        (item["source"].as_str(), item["acquisition"].as_str()),
        (Some("legacy"), Some("store"))
    );
    assert!(
        item["tradeId"].is_null(),
        "a store mint has no trade: {item}"
    );
    assert_eq!(item["priceCredits"], 1);
    assert_eq!(item["manaWei"], "5000000000000000000");
    assert!(
        item["emoteLoop"].is_null() && item.get("emoteLoop").is_some(),
        "the key reaches the wire: {item}"
    );
    let (status, body) = get(
        app(),
        &format!("/v3/catalog/unified?groupBy=item&contractAddress={c1}&itemId=1&source=native"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (
            body["data"][0]["tradeId"].as_str(),
            body["data"][0]["listingCount"].as_i64()
        ),
        (Some(primary_id.as_str()), Some(1))
    );
    // A resale row (the nft order) is MANA only: no credit price, whatever its listing carries (shop::guards::catalog).
    let (status, body) = get(app(), &format!("/v3/catalog/unified?contractAddress={c_nft}&includeLegacySecondary=true&listingType=secondary")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body["total"].as_i64().unwrap() >= 1,
        "the resale listing is in the feed: {body}"
    );
    let resale = &body["data"][0];
    assert_eq!(
        (resale["listingType"].as_str(), resale["tradeId"].as_str()),
        (Some("secondary"), Some(resale_id.as_str())),
        "{resale}"
    );
    assert!(
        resale["priceCredits"].is_null() && resale.get("priceCredits").is_some(),
        "a resale row carries no credit price: {resale}"
    );
    assert_eq!(
        resale["manaWei"], "3000000000000000000",
        "its MANA price stays: {resale}"
    );
    // Unmirrored again: the order legs below read the resale without an item.
    sqlx::query("DELETE FROM shop.items WHERE contract_address = $1")
        .bind(&c_nft)
        .execute(&pool)
        .await
        .unwrap();
    // Individual items select alongside whole collections (a union), and an
    // empty collection set still shows the named items.
    let unified_total = |body: &Value| body["total"].as_i64().unwrap();
    let (status, body) = get(
        app(),
        &format!("/v3/catalog/unified?groupBy=item&items={c2}-5,{c2}-0010,junk&first=10"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let mut picked: Vec<String> = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            format!(
                "{}-{}",
                d["contractAddress"].as_str().unwrap(),
                d["itemId"].as_str().unwrap()
            )
        })
        .collect();
    picked.sort();
    assert_eq!(
        picked,
        vec![format!("{c2}-10"), format!("{c2}-5")],
        "{body}"
    );
    // The emote attributes: c2's items 5 and 10 are emotes on sale; 5 loops
    // with sound, 10 plays once with props.
    let emote = |item: &'static str, looped: bool, sound: bool, geometry: bool| {
        sqlx::query("UPDATE shop.items SET emote_loop = $1, emote_has_sound = $2, emote_has_geometry = $3 WHERE contract_address = $4 AND item_id = $5::numeric")
            .bind(looped)
            .bind(sound)
            .bind(geometry)
            .bind(c2.clone())
            .bind(item)
            .execute(&pool)
    };
    emote("5", true, true, false).await.unwrap();
    emote("10", false, false, true).await.unwrap();
    let picked_with = |filter: &'static str| {
        let app = app();
        let uri =
            format!("/v3/catalog/unified?groupBy=item&items={c2}-5,{c2}-10&first=10&{filter}");
        async move {
            let (status, body) = get(app, &uri).await;
            assert_eq!(status, StatusCode::OK, "{body}");
            let mut ids: Vec<String> = body["data"]
                .as_array()
                .unwrap()
                .iter()
                .map(|d| d["itemId"].as_str().unwrap().to_string())
                .collect();
            ids.sort();
            ids
        }
    };
    assert_eq!(picked_with("emotePlayMode=loop").await, vec!["5"]);
    assert_eq!(picked_with("emotePlayMode=simple").await, vec!["10"]);
    assert_eq!(
        picked_with("emotePlayMode=loop&emotePlayMode=simple").await,
        vec!["10", "5"],
        "both modes is no filter"
    );
    assert_eq!(picked_with("emoteHasSound=true").await, vec!["5"]);
    assert_eq!(picked_with("emoteHasGeometry=true").await, vec!["10"]);
    assert!(
        picked_with("emoteHasSound=true&emoteHasGeometry=true")
            .await
            .is_empty()
    );
    let (_, body) = get(
        app(),
        &format!("/v3/catalog/unified?groupBy=item&contractAddress=junk&items={c2}-5"),
    )
    .await;
    assert_eq!(unified_total(&body), 1, "{body}");
    let (_, body) = get(
        app(),
        "/v3/catalog/unified?groupBy=item&contractAddress=junk&items=junk",
    )
    .await;
    assert_eq!(
        unified_total(&body),
        0,
        "both named sets empty is an empty page: {body}"
    );
    let (_, only_c1) = get(
        app(),
        &format!("/v3/catalog/unified?groupBy=item&contractAddress={c1}&first=1"),
    )
    .await;
    let (_, body) = get(
        app(),
        &format!("/v3/catalog/unified?groupBy=item&contractAddress={c1}&items={c2}-5&first=1"),
    )
    .await;
    assert_eq!(unified_total(&body), unified_total(&only_c1) + 1, "{body}");
    let (status, body) = get(app(), "/v3/catalog/legacy?first=2&search=Item%200004").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["total"], 0,
        "no MANA-priced primary trade was listed: {body}"
    );
    let (status, body) = get(
        app(),
        &format!("/v3/catalog/related?contractAddress={c1}&itemId=1&first=3"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"].as_array().unwrap().len(), 3);
    assert!(
        body["data"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["itemId"] != "1" && d["category"] == "wearable")
    );
    let (status, body) = get(app(), &format!("/v3/catalog/importable?seller={CREATOR}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body["data"].as_array().unwrap().is_empty(),
        "the native listing is not a MANA listing: {body}"
    );
    let (status, body) = get(app(), "/v1/coupons").await;
    assert_eq!(
        (status, body["message"].as_str()),
        (
            StatusCode::BAD_REQUEST,
            Some("A valid signer address is required")
        ),
        "the coupon store is served: a signer is required"
    );

    // The price histogram over the on-sale items.
    let (status, body) = get(
        app(),
        &format!("/v1/prices?contractAddress={c1}&category=wearable"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body["data"]["5000000000000000000"].as_i64().unwrap() >= 800,
        "{body}"
    );
    // A memo is filed under every mirror pass it read, not only the oldest:
    // Polygon's newer pass with a new price answers the new histogram,
    // though Ethereum's older pass stays the oldest.
    let pass = |network: &'static str, minutes_ago: i32| {
        let pool = pool.clone();
        async move {
            sqlx::query(
                "UPDATE shop.mirror_state SET last_complete_pass_at = \
                   date_trunc('second', timezone('UTC', now())) - make_interval(mins => $2) \
                 WHERE network = $1",
            )
            .bind(network)
            .bind(minutes_ago)
            .execute(&pool)
            .await
            .unwrap();
        }
    };
    pass("ethereum", 3).await;
    pass("matic", 2).await;
    let prices_uri = format!("/v1/prices?contractAddress={c1}&category=wearable");
    let (_, warm) = get(app(), &prices_uri).await;
    let repriced: Vec<String> = sqlx::query_scalar(
        "UPDATE shop.items SET price = 7000000000000000000 \
         WHERE contract_address = $1 AND price = 5000000000000000000 RETURNING item_id::text",
    )
    .bind(&c1)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(!repriced.is_empty());
    pass("matic", 1).await;
    let (status, fresh) = get(app(), &prices_uri).await;
    assert_eq!(status, StatusCode::OK, "{fresh}");
    assert!(
        fresh["data"]["7000000000000000000"].as_i64().is_some(),
        "{fresh}"
    );
    assert_ne!(warm, fresh);
    sqlx::query(
        "UPDATE shop.items SET price = 5000000000000000000 \
         WHERE contract_address = $1 AND item_id::text = ANY($2)",
    )
    .bind(&c1)
    .bind(&repriced)
    .execute(&pool)
    .await
    .unwrap();

    // The readers: canned subgraph rows merged with the mirror legs.
    let now_s = chrono::Utc::now().timestamp();
    let far = (now_s + 86_400 * 30) * 1000;
    // The legacy order book is the nft index's on this walking node: the
    // order sits in `shop.legacy_order`, and its contract is a collection
    // the node serves (kept only around the order routes below).
    sqlx::query(
        "INSERT INTO shop.collections SELECT (jsonb_populate_record(NULL::shop.collections, to_jsonb(c) \
           || jsonb_build_object('contract_address', $2::text))).* FROM shop.collections c WHERE c.network = 'matic' AND c.contract_address = $1",
    )
    .bind(&c1)
    .bind(&c_nft)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO shop.legacy_order (network, marketplace_address, order_id, contract_address, token_id, seller, price, expires_at, status, \
             created_block, created_log, created_at, updated_block, updated_log, updated_at) \
         VALUES ('matic', '0x480a0f4e360e8964e68858dd231c2922f1df45ef', 'legacy-order-1', $1, 5, $2, 1000000000000000000, $3, 'open', 1, 0, $4, 1, 0, $4)",
    )
    .bind(&c_nft)
    .bind(WALLET_A)
    .bind(far)
    .bind(now_s - 100)
    .execute(&pool)
    .await
    .unwrap();
    up.set("bids", vec![json!({ "id": "legacy-bid-1", "bidAddress": "0xb1d", "nftAddress": c_nft, "tokenId": "5", "bidder": WALLET_B, "seller": WALLET_A, "price": "2000000000000000000", "fingerprint": "0x", "status": "open", "blockchainId": "bid-1", "blockNumber": "1", "expiresAt": far.to_string(), "createdAt": (now_s - 1000).to_string(), "updatedAt": (now_s - 1000).to_string() })]);
    let sale = |i: i64| json!({ "id": format!("sale-{i}"), "type": if i % 2 == 0 { "mint" } else { "order" }, "buyer": WALLET_B, "seller": if i % 2 == 0 { CREATOR } else { WALLET_A }, "price": "5000000000000000000", "timestamp": (now_s - 3600 * i).to_string(), "txHash": format!("0xtx{i}"), "searchContractAddress": c1, "searchItemId": "1", "searchTokenId": "77", "searchCategory": "wearable", "item": { "id": format!("{c1}-1"), "creator": CREATOR } });
    up.set("sales", (1..=6).map(sale).collect());
    up.set("accounts", vec![json!({ "id": WALLET_A, "address": WALLET_A, "sales": 3, "purchases": 1, "spent": "5", "earned": "30", "royalties": "0", "collections": 1, "primarySales": 2, "primarySalesEarned": "20", "uniqueCollectorsTotal": 2, "uniqueAndMythicItemsTotal": 0, "creatorsSupportedTotal": 1 }), json!({ "id": WALLET_B, "address": WALLET_B, "sales": 1, "purchases": 4, "spent": "50", "earned": "1", "royalties": "0", "collections": 0, "primarySales": 0, "primarySalesEarned": "0", "uniqueCollectorsTotal": 0, "uniqueAndMythicItemsTotal": 3, "creatorsSupportedTotal": 2 })]);
    up.set("itemsDayDatas", vec![json!({ "id": format!("19000-{c1}-1"), "sales": 2, "volume": "10", "searchWearableCategory": "hat" }), json!({ "id": format!("19001-{c1}-1"), "sales": 1, "volume": "5", "searchWearableCategory": "hat" })]);
    up.set("analyticsDayDatas", vec![json!({ "id": "19001", "date": now_s - 100, "sales": 4, "volume": "40", "creatorsEarnings": "1", "daoEarnings": "2" })]);
    let owned_urn = format!("urn:decentraland:matic:collections-v2:{c1}:4");
    // `/v1/owners` still reads the collections subgraph's holders.
    up.set("nfts", vec![
        json!({ "id": format!("{c1}-{}", "4"), "tokenId": "4", "contractAddress": c1, "category": "wearable", "itemType": "wearable_v2", "urn": owned_urn, "image": "", "issuedId": "1", "itemBlockchainId": "4", "transferredAt": "1700000000", "createdAt": "1700000000", "updatedAt": "1700000001", "soldAt": "0", "owner": { "address": WALLET_A }, "item": { "id": format!("{c1}-4"), "rarity": "epic", "price": "5000000000000000000" }, "metadata": { "wearable": { "name": "Item 0004", "description": "a hat", "category": "hat", "rarity": "epic", "bodyShapes": ["BaseMale"] }, "emote": null }, "activeOrder": null }),
    ]);
    // The nft index publishes; WALLET_A holds copy 1 of c1's item 4 there.
    super::owned_items::test_support::publish(
        &state.pool,
        &super::chain_walk::ChainNet::ALL,
        false,
        Duration::ZERO,
    )
    .await;
    let owned_token = (alloy_primitives::U256::from(4u64) << 216u32
        | alloy_primitives::U256::from(1u64))
    .to_string();
    sqlx::query(
        "INSERT INTO shop.nft (network, contract_address, token_id, owner, item_id, issued_id, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
         VALUES ('matic', $1, $2::numeric, $3, 4, 1, 10, 1700000000, 10, 0, 1700000000)",
    )
    .bind(&c1)
    .bind(&owned_token)
    .bind(WALLET_A)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO shop.usage_grants (grantee_address, urn, token_id, collection, category, unlock_at) VALUES ($1, $2, '9', $3, 'wearable', timezone('UTC', now()) + interval '2 days')")
        .bind(WALLET_A)
        .bind(format!("urn:decentraland:matic:collections-v2:{c1}:6"))
        .bind(&c1)
        .execute(&pool)
        .await
        .unwrap();
    // The nft rows above are the published fold.
    republish_nft(&pool).await;

    let (status, body) = get(
        app(),
        &format!("/v1/orders?contractAddress={c_nft}&sortBy=cheapest"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["total"], "2",
        "the legacy order and the mirrored v3 resale: {body}"
    );
    assert_eq!(body["data"][0]["id"], "legacy-order-1");
    assert_eq!(
        body["data"][1]["tradeId"].as_str(),
        Some(resale_id.as_str())
    );
    // `/v1/nfts` gives a token with no legacy order its open v3 listing,
    // but only one its current owner signed.
    let resold_token = format!(
        "{}",
        alloy_primitives::U256::from(2u64) << 216u32 | alloy_primitives::U256::from(7u64)
    );
    let wanted = |owner: &str| {
        vec![(
            "matic".to_string(),
            c_nft.to_ascii_lowercase(),
            resold_token.clone(),
            owner.to_string(),
        )]
    };
    let listed = super::orders::open_trade_orders(&state, &wanted(WALLET_A))
        .await
        .unwrap();
    let order = listed
        .get(&(
            "matic".to_string(),
            c_nft.to_ascii_lowercase(),
            resold_token.clone(),
        ))
        .expect("the owner's open listing");
    assert_eq!(
        (order.id.as_str(), order.trade_id.as_deref()),
        (resale_id.as_str(), Some(resale_id.as_str()))
    );
    assert_eq!(order.price, "3000000000000000000");
    assert!(
        super::orders::open_trade_orders(&state, &wanted(CREATOR))
            .await
            .unwrap()
            .is_empty(),
        "a listing its signer no longer owns is not the token's order"
    );
    assert_eq!(body["data"][1]["issuedId"], "7");
    let (status, body) = get(app(), &format!("/v1/orders/open-by-items?item={c_nft}-2")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["total"], "1",
        "only the token whose id encodes item 2: {body}"
    );
    sqlx::query("DELETE FROM shop.collections WHERE network = 'matic' AND contract_address = $1")
        .bind(&c_nft)
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = get(app(), "/v1/bids?sortBy=most_expensive").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let results = body["data"]["results"].as_array().unwrap();
    assert!(
        results
            .iter()
            .any(|b| b["id"] == bid_id && b["seller"] == CREATOR && b["itemId"] == "3"),
        "the mirror bid names the item's creator: {body}"
    );
    assert!(
        results
            .iter()
            .any(|b| b["id"] == "legacy-bid-1" && b["bidAddress"] == "0xb1d")
    );
    assert_eq!(results[0]["id"], bid_id, "9 MANA before 2 MANA");
    // A configured graph with rows cannot supply an unpublished sales index.
    for path in [
        "/v1/sales?first=2&type=mint".to_string(),
        "/v1/accounts?sortBy=most_spent&first=1".to_string(),
        format!("/v1/sales/summary?seller={CREATOR}"),
    ] {
        let (status, body) = get(app(), &path).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
        assert_eq!(body["source"], "sales-index");
    }
    let (status, body) = get(app(), &format!("/v1/owners?contractAddress={c1}&itemId=4")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["data"][0],
        json!({ "issuedId": "1", "ownerId": WALLET_A, "tokenId": "4" })
    );
    for path in [
        "/v1/rankings/wearables/week?first=1",
        "/v1/rankings/creators/all?first=1",
        "/v1/rankings/collectors/all?sortBy=most_sales",
        "/v1/volume/week",
        "/v1/volume/all",
    ] {
        let (status, body) = get(app(), path).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{path} {body}");
        assert_eq!(body["source"], "sales-index");
    }
    let (status, body) = get(app(), "/v1/nfts?first=1&category=wearable").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (
            body["data"][0]["nft"]["id"].as_str(),
            body["total"].as_i64()
        ),
        (Some(format!("{c1}-{owned_token}").as_str()), Some(1))
    );
    let (status, body) = get(app(), "/v1/trendings?size=3").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"][0]["id"], format!("{c1}-1"));
    let (status, body) = get(app(), "/v3/catalog/trending?first=3").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (
            body["data"][0]["itemId"].as_str(),
            body["data"][0]["trendingSales"].as_i64()
        ),
        (Some("1"), Some(6))
    );
    let (status, body) = get(app(), "/v3/catalog/suggested?first=2").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (
            body["personalized"].as_bool(),
            body["data"][0]["reason"]["kind"].as_str()
        ),
        (Some(false), Some("trending"))
    );
    let (status, body) = get(app(), "/v3/catalog/creators?days=7").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"][0]["id"], CREATOR);
    assert!(
        body["data"][0]["items"].as_i64().unwrap() >= 800 && body["data"][0]["sales"] == 6,
        "{body}"
    );

    // Every catalogue route answers on a pool of one connection: no request
    // waits on a second connection while its catalogue read holds the first.
    // Each answer, cold, is the one a cold state on a wide pool gives (a leg
    // that timed out on the pool would answer 200 with fewer rows), and each
    // request must finish well inside the pool's acquire timeout (a wait for
    // a second connection panics here rather than degrade).
    let cold = |p: sqlx::PgPool| {
        let st = Arc::new(super::MarketplaceState::new(
            p,
            crate::http::Outbound::for_tests(),
            state.settings.clone(),
        ));
        st.test_allow_loopback.store(true, Ordering::Relaxed);
        st
    };
    let single = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(20))
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap();
    let one = cold(single.clone());
    let wide = cold(pool.clone());
    let routes = [
        "/v1/catalog?first=2".to_string(),
        "/v2/catalog?first=2".to_string(),
        format!("/v1/items?contractAddress={c1}&itemId=1"),
        format!("/v3/catalog/items?contractAddress={c1}&first=3"),
        "/v1/trendings?size=3".to_string(),
        format!("/v1/prices?contractAddress={c1}&category=wearable"),
        format!("/v3/catalog/shop?contractAddress={c1}"),
        "/v3/catalog/legacy?first=2&search=Item%200004".to_string(),
        "/v3/catalog/unified?groupBy=item&first=3".to_string(),
        "/v3/catalog/trending?first=3".to_string(),
        "/v3/catalog/creators?days=7".to_string(),
        "/v3/catalog/suggested?first=2".to_string(),
        format!("/v3/catalog/related?contractAddress={c1}&itemId=1&first=3"),
        format!("/v3/catalog/importable?seller={CREATOR}"),
        "/v3/catalog/suggest?search=item".to_string(),
        format!("/v1/collections?contractAddress={c2}"),
        "/v1/contracts?first=1000".to_string(),
    ];
    let lambdas = [
        "/lambdas/collections".to_string(),
        "/lambdas/collections/wearables?textSearch=item".to_string(),
    ];
    let on = |st: &Arc<super::MarketplaceState>, uri: &str| {
        if uri.starts_with("/lambdas/") {
            super::router(st.clone())
        } else {
            market_router(st.clone())
        }
    };
    // This fixture lists no MANA-priced trade (legacy, importable) and
    // deploys no wearable entity (lambdas wearables: its one-connection
    // answer with rows is `cold_wearables_answer_the_base_leg_on_a_one_
    // connection_pool`); their bodies are still compared.
    const EMPTY_BY_FIXTURE: [&str; 3] = [
        "/v3/catalog/legacy?first=2&search=Item%200004",
        "/v3/catalog/importable?seller=0xcccccccccccccccccccccccccccccccccccccccc",
        "/lambdas/collections/wearables?textSearch=item",
    ];
    let mut empty = Vec::new();
    for uri in routes.iter().chain(&lambdas) {
        let (status, expected) = get(on(&wide, uri), uri).await;
        assert_eq!(status, StatusCode::OK, "{uri} (wide pool): {expected}");
        fn has_rows(v: &Value) -> bool {
            match v {
                Value::Array(a) => !a.is_empty(),
                // `/v1/prices` answers its buckets as a `data` map.
                Value::Object(o) => o.iter().any(|(k, v)| {
                    has_rows(v) || (k == "data" && v.as_object().is_some_and(|d| !d.is_empty()))
                }),
                _ => false,
            }
        }
        if !has_rows(&expected) && !EMPTY_BY_FIXTURE.contains(&uri.as_str()) {
            empty.push(format!("{uri}: {expected}"));
        }
        let (status, body) = tokio::time::timeout(Duration::from_secs(10), get(on(&one, uri), uri))
            .await
            .unwrap_or_else(|_| panic!("{uri} waited on the pool"));
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
        assert_eq!(body, expected, "{uri} answered otherwise on one connection");
    }
    assert!(empty.is_empty(), "routes that answered no rows: {empty:#?}");
    drop((one, wide));
    single.close().await;

    let (status, body) = get(
        app(),
        &format!(
            "/v1/users/{}/wearables",
            WALLET_A.to_uppercase().replace("0X", "0x")
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let elements = body["data"]["elements"].as_array().unwrap();
    assert_eq!(
        (
            elements.len(),
            body["data"]["totalItems"].as_i64(),
            body["data"]["page"].as_i64()
        ),
        (2, Some(2), Some(1))
    );
    assert_eq!(elements[0]["urn"], owned_urn);
    assert!(elements[0].get("status").is_none());
    assert_eq!(
        (elements[1]["status"].as_str(), elements[1]["name"].as_str()),
        (Some("leased"), Some("Item 0006")),
        "the grant overlay: {}",
        elements[1]
    );
    assert!(elements[1]["unlockAt"].as_i64().unwrap() > now_s * 1000);
    let (status, body) = get(
        app(),
        &format!("/v1/users/{WALLET_A}/wearables/grouped?orderBy=name&direction=asc"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (
            body["data"]["elements"][0]["amount"].as_str(),
            body["data"]["elements"][0]["itemType"].as_str()
        ),
        (Some("1"), Some("wearable_v2"))
    );
    let (status, body) = get(app(), &format!("/v1/users/{WALLET_A}/wearables/urn-token")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["elements"][0]["tokenId"], owned_token.as_str());
    // The NAMEs read the nft index this node walks (no subgraph asked):
    // the wallet holds none.
    let (status, body) = get(app(), &format!("/v1/users/{WALLET_A}/names")).await;
    assert_eq!(
        (status, body["data"]["total"].as_i64()),
        (StatusCode::OK, Some(0)),
        "{body}"
    );

    // A marketplace the collection does not list as a minter cannot mint: every leg drops its order.
    let set_minter = |minter: &'static str| {
        let (pool, c1) = (pool.clone(), c1.clone());
        async move {
            sqlx::query(
                "UPDATE shop.collections SET minters = ARRAY[$2] WHERE contract_address = $1",
            )
            .bind(&c1)
            .bind(minter)
            .execute(&pool)
            .await
            .unwrap();
        }
    };
    set_minter(listings::offchain_marketplace_v2(137).unwrap().address).await;
    state.readers.clear();
    let (status, body) = get(app(), &format!("/v1/items?contractAddress={c1}&itemId=1")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["data"][0]["tradeId"].is_null(), "{}", body["data"][0]);
    let (status, body) = get(app(), &format!("/v2/catalog?id={c1}-1")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["data"][0]["tradeId"].is_null(), "{}", body["data"][0]);
    let (status, body) = get(app(), &format!("/v3/catalog/shop?contractAddress={c1}")).await;
    assert_eq!(
        (status, body["total"].as_i64()),
        (StatusCode::OK, Some(0)),
        "{body}"
    );
    set_minter(listings::offchain_marketplace_v3(137).unwrap().address).await;
    state.readers.clear();
    let (status, body) = get(app(), &format!("/v1/items?contractAddress={c1}&itemId=1")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["data"][0]["tradeId"].as_str(),
        Some(primary_id.as_str()),
        "{}",
        body["data"][0]
    );

    // Warm the trending rail before the cancellation.
    state.readers.clear();
    let (status, trending) = get(app(), "/v3/catalog/trending?first=3").await;
    assert_eq!(status, StatusCode::OK, "{trending}");
    // An operator cancellation drops the listing from every leg.
    let (status, body) = call(
        app(),
        "POST",
        &format!("/v1/admin/listings/order/{primary_hash}/force-cancel"),
        bearer("s3cr3t"),
        Some(json!({ "reason": "test" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["target_hash"], primary_hash);
    let (status, body) = get(app(), &format!("/v3/catalog/shop?contractAddress={c1}")).await;
    assert_eq!(
        (status, body["total"].as_i64()),
        (StatusCode::OK, Some(0)),
        "{body}"
    );
    let (status, body) = get(app(), &format!("/v2/catalog?id={c1}-1")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["data"][0]["tradeId"].is_null(), "{}", body["data"][0]);
    // Both the item read and the warm trending rail drop the cancelled listing.
    let (_, body) = get(
        app(),
        &format!("/v3/catalog/unified?groupBy=item&contractAddress={c1}&itemId=1&source=native"),
    )
    .await;
    assert_eq!(unified_total(&body), 0, "{body}");
    let carries_primary = |body: &Value| {
        body["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["tradeId"] == primary_id.as_str())
    };
    assert!(carries_primary(&trending), "{trending}");
    let (_, body) = get(app(), "/v3/catalog/trending?first=3").await;
    assert!(!carries_primary(&body), "{body}");
    // A read that names its item never reads the creator directory; a browse read does.
    use super::readers::Ttl;
    let directory = super::search::directory_key("mirror");
    state.readers.clear();
    let (status, body) = get(
        app(),
        &format!("/v3/catalog/unified?groupBy=item&contractAddress={c1}&itemId=1&source=native"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!state.readers.holds(Ttl::M15, &directory));
    let (status, body) = get(app(), "/v3/catalog/unified?groupBy=item&first=3").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // The browse read started the directory's read in the background; it may land after the
    // answer, which waits at most 500 ms for it.
    tokio::time::timeout(Duration::from_secs(20), async {
        while !state.readers.holds(Ttl::M15, &directory) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the browse read fills the creator directory");

    // The feeds apply the profile denylist per response and require revalidation.
    let profile_entity = format!("bafyprofile{t}");
    sqlx::query(
        "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain) \
         VALUES ($1, 'v3', 'profile', $2, $3::json, now(), ARRAY[$1], now(), '[]'::json)",
    )
    .bind(CREATOR)
    .bind(&profile_entity)
    .bind(json!({ "v": { "avatars": [{ "name": "Creatorname", "avatar": { "snapshots": { "face256": "bafyface" } } }] } }).to_string())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO content.active_pointers (pointer, entity_id, entity_type) VALUES ($1, $2, 'profile')")
        .bind(CREATOR)
        .bind(&profile_entity)
        .execute(&pool)
        .await
        .unwrap();
    state.readers.clear();
    // Read the directory first, through the dropdown's own path, which has no 500 ms bound: the
    // feeds below then find it fresh, start no refresh of their own, and never depend on how
    // fast a cold read is.
    {
        // Dropped at once: an open snapshot would hold its connection to the end.
        let mut read = Some(super::catalogue::begin_unchecked(&state).await);
        super::search::creator_directory(&state, &mut read)
            .await
            .unwrap()
            .unwrap();
    }
    let feeds = [
        "/v3/catalog/unified?groupBy=item&first=12",
        "/v3/catalog/trending?first=10&days=3650",
        "/v3/catalog/creators?days=7",
    ];
    let creator_profile = |body: &Value| {
        let rows = body["data"].as_array().unwrap();
        let row = rows
            .iter()
            .find(|r| r["creator"] == CREATOR || r["id"] == CREATOR)
            .unwrap_or_else(|| panic!("a row of the creator: {body}"));
        row.get("creatorProfile")
            .or_else(|| row.get("profile"))
            .cloned()
            .unwrap_or(Value::Null)
    };
    for uri in feeds {
        let (body, cache) = get_cached(app(), uri).await;
        assert_eq!(
            creator_profile(&body)["name"],
            "Creatorname",
            "{uri}: {body}"
        );
        assert!(
            creator_profile(&body)["face"].as_str().is_some(),
            "{uri}: {body}"
        );
        assert_eq!(cache, "no-cache", "{uri}");
    }
    let listed = "https://registry.example/denylist";
    state.denylist.set_remote(listed, [profile_entity.clone()]);
    for uri in feeds {
        let (body, cache) = get_cached(app(), uri).await;
        assert_eq!(
            creator_profile(&body),
            json!({ "name": "", "face": null, "claimed": false }),
            "{uri}: {body}"
        );
        assert_eq!(cache, "no-cache", "{uri}");
    }
    state.denylist.set_remote(listed, Vec::<String>::new());
    sqlx::query("DELETE FROM content.active_pointers WHERE pointer = $1")
        .bind(CREATOR)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM content.deployments WHERE entity_id = $1")
        .bind(&profile_entity)
        .execute(&pool)
        .await
        .unwrap();
    // A feed whose directory is refreshing answers without profiles.
    // Listing pages must still be revalidated.
    // A claim still live from the feeds above could not release this hold either: its release
    // acts only on its own claim.
    state.readers.clear();
    let hold = state
        .readers
        .begin_refresh(&directory, Duration::ZERO)
        .expect("a reset frees the directory's refresh");
    for uri in [
        "/v3/catalog/trending?first=10&days=3650",
        "/v3/catalog/creators?days=7",
    ] {
        let (body, cache) = get_cached(app(), uri).await;
        assert!(
            body["data"]
                .as_array()
                .unwrap()
                .iter()
                .all(|r| r.get("creatorProfile").is_none() && r.get("profile").is_none()),
            "{uri}: {body}"
        );
        let expected = if uri.contains("/catalog/trending") {
            "no-cache"
        } else {
            "public,max-age=60,s-maxage=60"
        };
        assert_eq!(cache, expected, "{uri}");
    }
    state.readers.end_refresh(&directory, hold, true);

    sqlx::query("DELETE FROM shop.usage_grants WHERE grantee_address = $1")
        .bind(WALLET_A)
        .execute(&pool)
        .await
        .unwrap();
    for id in [primary_id, resale_id, bid_id] {
        delete_trade(&pool, &id).await;
    }
    cleanup_mirror(&pool, &[c1, c2, c_nft]).await;
    sqlx::query("DELETE FROM economy.price_snapshots WHERE source = 'coingecko' AND mana_usd = 0.02::numeric").execute(&pool).await.unwrap();
    sqlx::query("DELETE FROM shop.mirror_state WHERE mirror IN ('collections', 'items')")
        .execute(&pool)
        .await
        .unwrap();
    jobs.shutdown(Duration::from_secs(1)).await;
    own.drop().await;
}

async fn insert_list(pool: &PgPool, name: &str, owner: &str) -> String {
    sqlx::query_scalar("INSERT INTO shop.lists (name, user_address, is_private) VALUES ($1, $2, true) RETURNING id::text").bind(name).bind(owner).fetch_one(pool).await.unwrap()
}

async fn grant(pool: &PgPool, list_id: &str, permission: &str, grantee: &str) {
    sqlx::query(
        "INSERT INTO shop.list_acl (list_id, permission, grantee) VALUES ($1::uuid, $2, $3)",
    )
    .bind(list_id)
    .bind(permission)
    .bind(grantee)
    .execute(pool)
    .await
    .unwrap();
}

async fn picks_in(pool: &PgPool, list_id: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*)::int8 FROM shop.picks WHERE list_id = $1::uuid")
        .bind(list_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn opts(item_id: Option<&str>) -> lists::GetListsOptions<'_> {
    lists::GetListsOptions {
        limit: 100,
        offset: 0,
        sort_by: lists::ListSortBy::CreatedAt,
        sort_direction: lists::ListSortDirection::Desc,
        item_id,
        q: None,
    }
}

/// What the stub contract wallet answers to `isValidSignature`.
#[derive(Clone, Copy)]
enum ContractWallet {
    Accepts,
    Refuses,
    Unreachable,
    /// The client source spent its contract-wallet checks (`eip1654_budget`).
    RateLimited,
}

struct StubEip1654(ContractWallet);

#[async_trait::async_trait]
impl crate::auth::eip1654::Eip1654Validator for StubEip1654 {
    /// It asks the contract on every call, as the RPC validator does.
    fn fresh_validator(&self) -> Option<&dyn crate::auth::eip1654::Eip1654Validator> {
        Some(self)
    }

    async fn validate_signature(
        &self,
        _contract_address: &str,
        _hash: &[u8],
        _signature: &[u8],
    ) -> Result<bool, crate::auth::AuthError> {
        match self.0 {
            ContractWallet::Accepts => Ok(true),
            ContractWallet::Refuses => Ok(false),
            ContractWallet::Unreachable => Err(crate::auth::AuthError::Eip1654ValidationFailed(
                "rpc unavailable".into(),
            )),
            ContractWallet::RateLimited => Err(crate::auth::AuthError::Eip1654RateLimited),
        }
    }
}

/// A contract wallet's chain for `method path`: SIGNER (the contract), its
/// EIP-1654 ephemeral delegation, and the request payload the ephemeral key
/// signs.
async fn contract_wallet_headers(contract: &str, method: &str, path: &str) -> HeaderMap {
    contract_wallet_headers_with(contract, method, path, "{}").await
}

/// [`contract_wallet_headers`] signing `metadata`.
async fn contract_wallet_headers_with(
    contract: &str,
    method: &str,
    path: &str,
    metadata: &str,
) -> HeaderMap {
    use alloy::signers::Signer as _;
    let ephemeral = alloy::signers::local::PrivateKeySigner::random();
    let timestamp = chrono::Utc::now().timestamp_millis().to_string();
    let expiration =
        (chrono::Utc::now() + chrono::Duration::hours(1)).format("%Y-%m-%dT%H:%M:%S%.3fZ");
    let payload = crate::shared::signed_fetch::build_payload_v6(method, path, &timestamp, metadata);
    let signature = ephemeral
        .sign_message(payload.as_bytes())
        .await
        .unwrap()
        .to_string();
    let links = [
        json!({"type": "SIGNER", "payload": contract, "signature": ""}),
        json!({"type": "ECDSA_EIP_1654_EPHEMERAL",
            "payload": format!("Decentraland Login\nEphemeral address: {:#x}\nExpiration: {expiration}", ephemeral.address()),
            "signature": format!("0x{}", "11".repeat(65))}),
        json!({"type": "ECDSA_SIGNED_ENTITY", "payload": payload, "signature": signature}),
    ];
    let mut headers = HeaderMap::new();
    for (index, link) in links.iter().enumerate() {
        headers.insert(
            axum::http::HeaderName::try_from(format!("x-identity-auth-chain-{index}")).unwrap(),
            link.to_string().parse().unwrap(),
        );
    }
    headers.insert("x-identity-timestamp", timestamp.parse().unwrap());
    headers.insert("x-identity-metadata", metadata.parse().unwrap());
    headers
}

/// A contract wallet (EIP-1654) picks an item and reads its picks as an EOA
/// does, through the node's validator (`isValidSignature`), as upstream's
/// crypto middleware verified it. A contract that refuses the signature is a
/// 401, a check that cannot run (the RPC failed) is a 503 and one the client
/// source's budget refused is a 429: never the signer, on the required (POST)
/// and the optional (GET) route alike.
#[tokio::test]
async fn a_contract_wallet_picks_through_the_eip1654_validator() {
    let Some(mut state) = state_with_db(Settings {
        data_writes_enabled: true,
        ..Settings::for_tests()
    })
    .await
    else {
        return;
    };
    let pool = state.pool.clone();
    let t = tag();
    let contract = format!("0x{:0>40}", "c0de1").to_lowercase();
    let item = format!("{}-0", contract_of(&t, 7));
    mirrors::items::upsert(
        &pool,
        super::graph::Network::Matic,
        137,
        false,
        &[mirrors::items::fixture(
            &contract_of(&t, 7),
            "0",
            "wearable_v2",
            "Picked by a contract",
        )],
    )
    .await
    .unwrap();
    let path = format!("/v1/picks/{item}");
    let with = |state: &mut SharedState, wallet| {
        Arc::get_mut(state).expect("an unshared state").eip1654 =
            Some(Arc::new(StubEip1654(wallet)));
    };

    with(&mut state, ContractWallet::Accepts);
    let (status, body) = call(
        market_router(state.clone()),
        "POST",
        &path,
        contract_wallet_headers(&contract, "post", &path).await,
        Some(json!({ "pickedFor": [lists::DEFAULT_LIST_ID] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["pickedByUser"], true, "{body}");
    let (status, body) = call(
        market_router(state.clone()),
        "GET",
        &path,
        contract_wallet_headers(&contract, "get", &path).await,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["data"]["results"][0]["userAddress"], contract,
        "the caller (the contract) first: {body}"
    );

    for (wallet, code) in [
        (ContractWallet::Refuses, StatusCode::UNAUTHORIZED),
        (ContractWallet::Unreachable, StatusCode::SERVICE_UNAVAILABLE),
        (ContractWallet::RateLimited, StatusCode::TOO_MANY_REQUESTS),
    ] {
        with(&mut state, wallet);
        let (status, body) = call(
            market_router(state.clone()),
            "POST",
            &path,
            contract_wallet_headers(&contract, "post", &path).await,
            Some(json!({ "pickedFor": [lists::DEFAULT_LIST_ID] })),
        )
        .await;
        assert_eq!(status, code, "POST: {body}");
        let (status, body) = call(
            market_router(state.clone()),
            "GET",
            &path,
            contract_wallet_headers(&contract, "get", &path).await,
            None,
        )
        .await;
        assert_eq!(status, code, "GET: {body}");
    }

    // `/v1/activity` (its own refusal codes, a marketplace signer required):
    // the contract's own activity when it accepts, the 503 when the check
    // cannot run, never a 400 that blames the request.
    let activity = format!("/v1/activity?address={contract}");
    let marketplace = r#"{"signer":"dcl:marketplace"}"#;
    for (wallet, code) in [
        (ContractWallet::Accepts, StatusCode::OK),
        (ContractWallet::Refuses, StatusCode::UNAUTHORIZED),
        (ContractWallet::Unreachable, StatusCode::SERVICE_UNAVAILABLE),
        (ContractWallet::RateLimited, StatusCode::TOO_MANY_REQUESTS),
    ] {
        with(&mut state, wallet);
        let (status, body) = call(
            market_router(state.clone()),
            "GET",
            &activity,
            contract_wallet_headers_with(&contract, "get", "/v1/activity", marketplace).await,
            None,
        )
        .await;
        assert_eq!(status, code, "/v1/activity: {body}");
    }

    // Without a validator the chain is the 501 it was before.
    Arc::get_mut(&mut state).expect("an unshared state").eip1654 = None;
    let (status, _) = call(
        market_router(state.clone()),
        "POST",
        &path,
        contract_wallet_headers(&contract, "post", &path).await,
        Some(json!({ "pickedFor": [lists::DEFAULT_LIST_ID] })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
    sqlx::query("DELETE FROM shop.picks WHERE user_address = $1")
        .bind(&contract)
        .execute(&pool)
        .await
        .unwrap();
}

/// A contract that refuses every signature and counts the `isValidSignature`
/// calls it was asked.
#[derive(Default)]
struct CountingRefusal(std::sync::atomic::AtomicUsize);

#[async_trait::async_trait]
impl crate::auth::eip1654::Eip1654Validator for CountingRefusal {
    async fn validate_signature(
        &self,
        _contract_address: &str,
        _hash: &[u8],
        _signature: &[u8],
    ) -> Result<bool, crate::auth::AuthError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(false)
    }
}

/// The public reads that verify an optional contract-wallet chain spend the
/// node's RPC quota, so the node's validator (`eip1654_budget`, behind the
/// layer that names each request's client source) lets each source start only
/// the `/forward-blockchain` pre-auth budget of checks a minute: past it the
/// answer is a 429 before any `isValidSignature` call, and another source
/// keeps its own budget.
#[tokio::test]
async fn contract_wallet_checks_are_budgeted_per_source_before_any_rpc_call() {
    use crate::shared::eip1654_budget::{SourceBudgeted, scope_client_source};
    let Some(mut state) = state_with_db(Settings::for_tests()).await else {
        return;
    };
    let counter = Arc::new(CountingRefusal::default());
    Arc::get_mut(&mut state).expect("an unshared state").eip1654 =
        Some(Arc::new(SourceBudgeted::new(counter.clone())));
    let contract = format!("0x{:0>40}", "c0de2").to_lowercase();
    let path = format!("/v1/picks/{}-0", contract_of(&tag(), 8));
    let get_from = |peer: &'static str| {
        let (app, path, contract) = (
            scope_client_source(market_router(state.clone()), Vec::new()),
            path.clone(),
            contract.clone(),
        );
        async move {
            let mut req = Request::builder().uri(&path).body(Body::empty()).unwrap();
            *req.headers_mut() = contract_wallet_headers(&contract, "get", &path).await;
            req.extensions_mut()
                .insert(axum::extract::ConnectInfo::<std::net::SocketAddr>(
                    peer.parse().unwrap(),
                ));
            app.oneshot(req).await.unwrap().status()
        }
    };
    let started = std::time::Instant::now();
    let mut tripped = None;
    for sent in 0..400 {
        let before = counter.0.load(Ordering::SeqCst);
        let status = get_from("203.0.113.77:40000").await;
        if status == StatusCode::TOO_MANY_REQUESTS {
            tripped = Some((sent, before, counter.0.load(Ordering::SeqCst)));
            break;
        }
        assert_eq!(status, StatusCode::UNAUTHORIZED, "a refused contract");
    }
    let (sent, before, after) = tripped.expect("the source's budget trips");
    assert!(sent > 0, "refused contracts first");
    let admitted = crate::shared::eip1654_budget::admitted_within(started.elapsed());
    assert!(
        admitted.contains(&before),
        "the /forward-blockchain pre-auth budget of isValidSignature calls: {before}, {admitted:?}"
    );
    assert_eq!(
        before, after,
        "no isValidSignature call once the budget is spent"
    );
    assert_eq!(
        get_from("198.51.100.9:40000").await,
        StatusCode::UNAUTHORIZED,
        "another source keeps its own budget"
    );
}

/// The shared cache within its acceptance TTL after a contract wallet's
/// owner was removed: the cached answer still accepts, the contract (the
/// fresh path) refuses.
struct StaleAcceptance(StubEip1654);

#[async_trait::async_trait]
impl crate::auth::eip1654::Eip1654Validator for StaleAcceptance {
    fn fresh_validator(&self) -> Option<&dyn crate::auth::eip1654::Eip1654Validator> {
        Some(&self.0)
    }

    async fn validate_signature(
        &self,
        _contract_address: &str,
        _hash: &[u8],
        _signature: &[u8],
    ) -> Result<bool, crate::auth::AuthError> {
        Ok(true)
    }
}

/// A write asks the contract itself (`signed::fresh`), so a delegation stops
/// authorizing the moment its owner is removed; a read keeps the shared
/// cache. The optional signer of the public reads (catalog `pickedBy`, the
/// suggested feed) verifies a contract wallet too, and a check that cannot
/// run is a 503 there, never an anonymous visitor.
#[tokio::test]
async fn writes_ask_the_contract_and_optional_reads_fail_closed() {
    let Some(mut state) = state_with_db(Settings {
        data_writes_enabled: true,
        ..Settings::for_tests()
    })
    .await
    else {
        return;
    };
    let pool = state.pool.clone();
    let t = tag();
    let contract = format!("0x{:0>40}", "c0de3").to_lowercase();
    let item = format!("{}-0", contract_of(&t, 9));
    mirrors::items::upsert(
        &pool,
        super::graph::Network::Matic,
        137,
        false,
        &[mirrors::items::fixture(
            &contract_of(&t, 9),
            "0",
            "wearable_v2",
            "Stale",
        )],
    )
    .await
    .unwrap();
    Arc::get_mut(&mut state).expect("an unshared state").eip1654 = Some(Arc::new(StaleAcceptance(
        StubEip1654(ContractWallet::Refuses),
    )));
    let path = format!("/v1/picks/{item}");
    let (status, body) = call(
        market_router(state.clone()),
        "POST",
        &path,
        contract_wallet_headers(&contract, "post", &path).await,
        Some(json!({ "pickedFor": [lists::DEFAULT_LIST_ID] })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a write asks the contract: {body}"
    );
    let (status, body) = call(
        market_router(state.clone()),
        "GET",
        &path,
        contract_wallet_headers(&contract, "get", &path).await,
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a read keeps the cached answer: {body}"
    );

    use super::wire::ApiError;
    let optional = |wallet| {
        let contract = contract.clone();
        async move {
            let headers = contract_wallet_headers(&contract, "get", "/v1/catalog").await;
            super::signed::optional_signer(
                Some(&StubEip1654(wallet)),
                &headers,
                "get",
                "/v1/catalog",
            )
            .await
        }
    };
    assert_eq!(
        optional(ContractWallet::Accepts).await.unwrap(),
        Some(contract.clone())
    );
    assert!(matches!(
        optional(ContractWallet::Refuses).await,
        Err(ApiError::Http { code: 401, .. })
    ));
    assert!(matches!(
        optional(ContractWallet::Unreachable).await,
        Err(ApiError::Http { code: 503, .. })
    ));
}

#[tokio::test]
async fn favorites_lists_and_picks_keep_the_old_authorization_and_counting_rules() {
    let Some(state) = state_with_db(Settings {
        data_writes_enabled: true,
        ..Settings::for_tests()
    })
    .await
    else {
        return;
    };
    let pool = state.pool.clone();
    let t = tag();
    let (wallet_a, wallet_b) = (
        format!("0x{:0>40}", &t[..20])
            .replace('-', "a")
            .to_lowercase(),
        format!("0x{:0>40}", format!("b{}", &t[..19])).to_lowercase(),
    );
    let (wallet_a, wallet_b) = (
        format!("0xa{}", &wallet_a[3..]),
        format!("0xb{}", &wallet_b[3..]),
    );
    let item_x = format!("{}-0", contract_of(&t, 1));
    let item_y = format!("{}-0", contract_of(&t, 2));

    // A private list without ACL rows rejects a non-owner (upstream lists-authorization.spec.ts).
    let victim = insert_list(&pool, "victim private list", &wallet_a).await;
    let flagged = lists::check_non_editable_lists(&pool, std::slice::from_ref(&victim), &wallet_b)
        .await
        .unwrap();
    assert_eq!(flagged, vec![victim.clone()]);

    // The default list, own lists and edit-granted lists are editable; a view grant is not.
    let own = insert_list(&pool, "attacker own list", &wallet_b).await;
    let shared = insert_list(&pool, "victim shared list", &wallet_a).await;
    grant(&pool, &shared, "edit", &wallet_b).await;
    let public = insert_list(&pool, "everyone edits", &wallet_a).await;
    grant(&pool, &public, "edit", "*").await;
    let targets = [
        lists::DEFAULT_LIST_ID.to_string(),
        own.clone(),
        shared.clone(),
        public.clone(),
    ];
    assert!(
        lists::check_non_editable_lists(&pool, &targets, &wallet_b)
            .await
            .unwrap()
            .is_empty()
    );
    lists::pick_and_unpick_in_bulk(&pool, &item_y, &wallet_b, &targets, &[])
        .await
        .unwrap();
    for list in &targets {
        assert_eq!(
            picks_in(&pool, list).await,
            if list == lists::DEFAULT_LIST_ID {
                picks_in(&pool, list).await
            } else {
                1
            },
            "{list}"
        );
    }
    let viewable = insert_list(&pool, "view only", &wallet_a).await;
    grant(&pool, &viewable, "view", &wallet_b).await;
    assert_eq!(
        lists::check_non_editable_lists(&pool, std::slice::from_ref(&viewable), &wallet_b)
            .await
            .unwrap(),
        vec![viewable.clone()]
    );

    // A foreign pick does not move the owner's counts or preview; isItemInList is the caller's.
    lists::pick_and_unpick_in_bulk(
        &pool,
        &item_x,
        &wallet_a,
        std::slice::from_ref(&shared),
        &[],
    )
    .await
    .unwrap();
    assert_eq!(
        picks_in(&pool, &shared).await,
        2,
        "both picks physically exist in the list"
    );
    let (rows, _) = lists::get_lists_rows(&pool, &wallet_a, &opts(Some(&item_y)))
        .await
        .unwrap();
    let l = rows
        .iter()
        .find(|r| r.id == shared)
        .expect("owner sees own list");
    assert_eq!(
        (
            l.items_count,
            l.preview_of_item_ids.clone(),
            l.is_item_in_list
        ),
        (1, vec![item_x.clone()], Some(false))
    );
    let (rows, _) = lists::get_lists_rows(&pool, &wallet_a, &opts(Some(&item_x)))
        .await
        .unwrap();
    assert_eq!(
        rows.iter()
            .find(|r| r.id == shared)
            .unwrap()
            .is_item_in_list,
        Some(true)
    );
    let (rows, _) = lists::get_lists_rows(&pool, &wallet_b, &opts(None))
        .await
        .unwrap();
    assert!(
        rows.iter().all(|r| r.id != shared),
        "a grant does not list somebody else's list"
    );

    // The shared Wishlist is first, counted, and caller-scoped; isPrivate is derived from the ACL.
    let (rows, total) = lists::get_lists_rows(&pool, &wallet_a, &opts(None))
        .await
        .unwrap();
    assert!(
        total >= 2
            && rows[0].id == lists::DEFAULT_LIST_ID
            && rows[0].is_default_list
            && rows[0].user_address == lists::DEFAULT_LIST_USER_ADDRESS,
        "{rows:?}"
    );
    assert_eq!(
        rows[0].items_count, 0,
        "the shared Wishlist's itemsCount is the caller's picks"
    );
    assert!(
        rows[0].is_private,
        "no ACL rows reads private though the stored column says false"
    );
    assert!(
        !rows.iter().find(|r| r.id == public).unwrap().is_private,
        "a '*' grant reads public"
    );

    // The routes: signed picks and lists.
    let signer = sf::address();
    let (status, body) = call(
        market_router(state.clone()),
        "POST",
        &format!("/v1/picks/{item_x}"),
        signed_headers("post", &format!("/v1/picks/{item_x}"), "{}", true).await,
        Some(json!({ "pickedFor": [lists::DEFAULT_LIST_ID] })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "the item is not in the mirror: {body}"
    );
    assert_eq!(body["data"]["itemId"], item_x);
    mirrors::items::upsert(
        &pool,
        super::graph::Network::Matic,
        137,
        false,
        &[mirrors::items::fixture(
            &contract_of(&t, 1),
            "0",
            "wearable_v2",
            "Picked",
        )],
    )
    .await
    .unwrap();
    let (status, body) = call(
        market_router(state.clone()),
        "POST",
        &format!("/v1/picks/{item_x}"),
        signed_headers("post", &format!("/v1/picks/{item_x}"), "{}", true).await,
        Some(json!({ "pickedFor": [lists::DEFAULT_LIST_ID] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["pickedByUser"], true);
    let (status, body) = get(
        market_router(state.clone()),
        &format!("/v1/picks/stats?itemId={item_x}&checkingUserAddress={signer}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (
            body["data"][0]["count"].as_i64(),
            body["data"][0]["pickedByUser"].as_bool()
        ),
        (Some(2), Some(true)),
        "wallet A's pick in the shared list and the signer's in the Wishlist: {body}"
    );
    let (status, body) = call(
        market_router(state.clone()),
        "GET",
        "/v1/lists",
        signed_headers("get", "/v1/lists", "{}", true).await,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["results"][0]["id"], lists::DEFAULT_LIST_ID);
    assert_eq!(body["data"]["results"][0]["itemsCount"], 1);
    let (status, body) = call(
        market_router(state.clone()),
        "GET",
        &format!("/v1/lists/{}/picks", lists::DEFAULT_LIST_ID),
        signed_headers(
            "get",
            &format!("/v1/lists/{}/picks", lists::DEFAULT_LIST_ID),
            "{}",
            true,
        )
        .await,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body["data"]["results"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["itemId"] == item_x),
        "{body}"
    );
    let (status, _) = call(
        market_router(state.clone()),
        "DELETE",
        &format!("/v1/picks/{item_x}"),
        signed_headers("delete", &format!("/v1/picks/{item_x}"), "{}", true).await,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = call(
        market_router(state.clone()),
        "POST",
        &format!("/v1/picks/{item_x}"),
        HeaderMap::new(),
        Some(json!({})),
    )
    .await;
    assert_eq!(
        (status, body["message"].as_str()),
        (StatusCode::UNAUTHORIZED, Some("Invalid Auth Chain"))
    );

    // The write switch: the same call is a 503 with writes off, the reads stay open.
    let gated = state_without_db_settings(Settings::for_tests());
    let (status, body) = call(
        market_router(gated.clone()),
        "POST",
        &format!("/v1/picks/{item_x}"),
        HeaderMap::new(),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    let (status, _) = get(
        market_router(gated),
        "/v1/picks/stats?itemId=0x1111111111111111111111111111111111111111-0",
    )
    .await;
    assert_ne!(status, StatusCode::SERVICE_UNAVAILABLE);

    sqlx::query("DELETE FROM shop.picks WHERE user_address = ANY($1) OR item_id = ANY($2)")
        .bind(vec![wallet_a.clone(), wallet_b.clone(), signer])
        .bind(vec![item_x.clone(), item_y.clone()])
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM shop.lists WHERE user_address = ANY($1)")
        .bind(vec![wallet_a, wallet_b])
        .execute(&pool)
        .await
        .unwrap();
    cleanup_mirror(&pool, &[contract_of(&t, 1)]).await;
}

async fn seed_item_row(pool: &PgPool, contract: &str, item_id: &str, creator: &str) {
    sqlx::query(
        "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, \
             total_supply, max_supply, available, price, image, name, category, created_at, updated_at, reviewed_at, mirrored_at) \
         VALUES ('matic', lower($1), $2::numeric, 137, 'wearable_v2', $3, lower($4), 'common', 0, 10, 10, 0, '', 'Item', 'hat', 0, 0, 0, now())",
    )
    .bind(contract)
    .bind(item_id)
    .bind(format!("urn:decentraland:matic:collections-v2:{contract}:{item_id}"))
    .bind(creator)
    .execute(pool)
    .await
    .unwrap();
    // Its collection lets the v3 marketplace mint, so its item orders are served.
    sqlx::query(
        "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, is_completed, \
             is_approved, is_editable, minters, created_at, updated_at, reviewed_at, mirrored_at) \
         VALUES ('matic', lower($1), 137, 'Item fixture', lower($2), lower($2), $3, true, true, false, ARRAY[$4], 0, 0, 0, now()) \
         ON CONFLICT (network, contract_address) DO NOTHING",
    )
    .bind(contract)
    .bind(creator)
    .bind(format!("urn:decentraland:matic:collections-v2:{contract}"))
    .bind(listings::offchain_marketplace_v3(137).unwrap().address)
    .execute(pool)
    .await
    .unwrap();
}

/// The routes that read the sales reader (a per-network subgraph query) or
/// the signer's EIP-1654 call make that call with no catalogue read open:
/// with the subgraph hanging, the requests waiting on it hold no pooled
/// connection, and the other catalogue routes keep answering on a pool of two.
#[tokio::test]
async fn a_hanging_subgraph_holds_no_catalogue_connection() {
    let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
        return;
    };
    super::trade_events::tests::publish_cursors(&pool).await;
    sqlx::query(
        "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at) \
         SELECT m, n, 'test', timezone('UTC', now()) \
         FROM unnest(ARRAY['collections', 'items']) m CROSS JOIN unnest(ARRAY['ethereum', 'matic']) n",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO economy.price_snapshots (source, mana_usd, taken_at) VALUES ('coingecko', 0.02, timezone('UTC', now()))")
        .execute(&pool)
        .await
        .unwrap();
    // A subgraph that accepts and never answers.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let hanging = format!("http://{}/subgraph", listener.local_addr().unwrap());
    let held = tokio::spawn(async move {
        let mut sockets = Vec::new();
        while let Ok((socket, _)) = listener.accept().await {
            sockets.push(socket);
        }
    });
    let narrow = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(3))
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap();
    let state = Arc::new(super::MarketplaceState::new(
        narrow.clone(),
        crate::http::Outbound::for_tests(),
        Settings {
            eth_collections_subgraph_url: Some(hanging.clone()),
            matic_collections_subgraph_url: Some(hanging),
            data_writes_enabled: true,
            ..Settings::for_tests()
        },
    ));
    state.test_allow_loopback.store(true, Ordering::Relaxed);
    let waiting: Vec<_> = [
        "/v1/trendings",
        "/v1/trendings",
        "/v3/catalog/trending?first=3",
        "/v3/catalog/creators",
        "/v3/catalog/suggested?first=2",
        "/v3/catalog/suggest?search=hat",
    ]
    .into_iter()
    .map(|uri| {
        let state = state.clone();
        tokio::spawn(async move { get(market_router(state), uri).await })
    })
    .collect();
    tokio::time::sleep(Duration::from_millis(500)).await;
    for task in &waiting {
        assert!(!task.is_finished(), "a route answered without its subgraph");
    }
    for uri in [
        "/v1/collections?first=1",
        "/v1/contracts?first=1",
        "/v1/catalog?first=1",
    ] {
        let (status, body) = tokio::time::timeout(
            Duration::from_secs(10),
            get(market_router(state.clone()), uri),
        )
        .await
        .unwrap_or_else(|_| panic!("{uri} waited behind the hanging subgraph"));
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
    }
    for task in waiting {
        task.abort();
    }
    held.abort();
    drop(state);
    narrow.close().await;
    pool.close().await;
}

/// The pages of one read are read at its instant: a native listing and a
/// coupon that expire between the read's first and second OFFSET page are
/// counted and priced alike on both; a later read no longer has them, and a
/// read held at the first one's instant still does.
#[tokio::test]
async fn the_pages_of_one_read_decide_expiries_at_its_instant() {
    use super::shop::{parse_shop_filters, shop_listings};
    let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
        return;
    };
    let state = Arc::new(super::MarketplaceState::new(
        pool.clone(),
        crate::http::Outbound::for_tests(),
        Settings::for_tests(),
    ));
    super::trade_events::tests::publish_cursors(&pool).await;
    let t = tag();
    let (c1, c2) = (contract_of(&t, 1), contract_of(&t, 2));
    let mut trades = Vec::new();
    for c in [&c1, &c2] {
        seed_item_row(&pool, c, "0", CREATOR).await;
        let sent = Asset {
            asset_type: 4,
            contract: c.clone(),
            value: "0".into(),
        };
        let received = Asset {
            asset_type: 2,
            contract: "0x7ad72b9f944ea9793cf4055d88f81138cc2c63a0".into(),
            value: "250000000000000000".into(),
        };
        trades.push(
            insert_trade(&pool, "public_item_order", CREATOR, sent, received)
                .await
                .0,
        );
    }
    sqlx::query("UPDATE shop.items SET search_is_collection_approved = true")
        .execute(&pool)
        .await
        .unwrap();
    // c1's listing (first page, the newer) and c2's coupon (second page;
    // half off: a sale the card shows) end 3 s from now.
    sqlx::query("UPDATE shop.trades SET expires_at = timezone('UTC', now()) + interval '3 seconds' WHERE id = $1::uuid")
        .bind(&trades[0])
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE shop.trades SET created_at = created_at - interval '1 hour' WHERE id = $1::uuid",
    )
    .bind(&trades[1])
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO shop.coupons (network, chain_id, signer, signature, hashed_signature, state_key, coupon_manager, coupon_address, checks, \
            discount_type, discount_ppm, root, collections, effective_since, expires_at) \
            VALUES ('MATIC', 137, $1, $2, $2, $2, '0x655fdfa91d69ea49f4ce1a8f7f7e2622c8630813', '0xc914507fe297b2dddd1232ac3a8903f1c125e794', \
                '{\"uses\": 10}', 1, 500000, '0x00', ARRAY[$3], timezone('UTC', now()) - interval '1 day', timezone('UTC', now()) + interval '3 seconds')")
        .bind(CREATOR)
        .bind(format!("0x{}", tag()))
        .bind(&c2)
        .execute(&pool)
        .await
        .unwrap();
    let page = |skip: &str| {
        parse_shop_filters(&[
            ("first".to_string(), "1".to_string()),
            ("skip".to_string(), skip.to_string()),
        ])
    };
    let mut read = super::catalogue::begin_unchecked(&state).await;
    let instant = read.now;
    let (first, total) = shop_listings(&state, &mut read, &page("0")).await.unwrap();
    assert_eq!((first.len(), total), (1, 2));
    tokio::time::sleep(Duration::from_millis(3500)).await;
    let (second, total) = shop_listings(&state, &mut read, &page("1")).await.unwrap();
    assert_eq!((second.len(), total), (1, 2), "the second page of the read");
    assert_eq!(
        (first[0].trade_id.as_str(), second[0].trade_id.as_str()),
        (trades[0].as_str(), trades[1].as_str())
    );
    assert!(
        second[0].sale.coupon.is_some(),
        "the coupon holds at the read's instant"
    );
    drop(read);

    let mut later = super::catalogue::begin_unchecked(&state).await;
    let (rows, total) = shop_listings(&state, &mut later, &page("0")).await.unwrap();
    assert_eq!(total, 1);
    assert_eq!(rows[0].trade_id, trades[1]);
    assert!(rows[0].sale.coupon.is_none(), "the coupon ended");
    drop(later);

    // The read's clock decides, not the database's: a read held at the
    // first one's instant still has both.
    let mut held = super::catalogue::begin_unchecked(&state).await;
    held.now = instant;
    let all = parse_shop_filters(&[("first".to_string(), "10".to_string())]);
    let (rows, total) = shop_listings(&state, &mut held, &all).await.unwrap();
    assert_eq!(total, 2);
    assert!(
        rows[1].sale.coupon.is_some(),
        "the coupon at the read's instant"
    );
    drop(held);
    pool.close().await;
}

/// One contract and one item id on both networks: the read that names both
/// pages the two rows itself and keeps the exact total on every page, as the
/// paged read and its count do over the whole contract.
#[tokio::test]
async fn an_item_on_two_networks_keeps_its_total_on_every_page() {
    use super::items::{ItemFilters, list_items};
    let Some(state) = state_with_db(Settings::for_tests()).await else {
        return;
    };
    let pool = state.pool.clone();
    let contract = contract_of(&tag(), 3);
    for (network, chain_id, item_id, created_at) in [
        ("matic", 137, "7", 2),
        ("ethereum", 1, "7", 1),
        ("matic", 137, "8", 3),
    ] {
        sqlx::query(
            "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, \
                 total_supply, max_supply, available, price, image, name, category, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ($1, $2, $3::numeric, $4, 'wearable_v2', $5, $6, 'common', 0, 10, 10, 0, '', 'Item', 'hat', $7, 0, 0, now())",
        )
        .bind(network)
        .bind(&contract)
        .bind(item_id)
        .bind(chain_id)
        .bind(format!("urn:decentraland:{network}:collections-v2:{contract}:{item_id}"))
        .bind(CREATOR)
        .bind(created_at)
        .execute(&pool)
        .await
        .unwrap();
    }
    let read = |item_id: Option<&str>, first: i64, skip: i64| {
        let f = ItemFilters {
            contract_addresses: vec![contract.clone()],
            item_id: item_id.map(String::from),
            first: Some(first),
            skip: Some(skip),
            ..ItemFilters::default()
        };
        let state = state.clone();
        async move {
            let mut read = crate::market::catalogue::begin_unchecked(&state).await;
            let (items, total) = list_items(&state, &mut read, &f).await.unwrap();
            let chains: Vec<i64> = items
                .iter()
                .map(|i| {
                    serde_json::to_value(i).unwrap()["chainId"]
                        .as_i64()
                        .unwrap()
                })
                .collect();
            (chains, total)
        }
    };
    for (first, skip, chains) in [
        (1, 0, vec![137]),
        (1, 1, vec![1]),
        (1, 2, vec![]),
        (5, 0, vec![137, 1]),
        (0, 0, vec![]),
    ] {
        assert_eq!(
            read(Some("7"), first, skip).await,
            (chains, 2),
            "first={first} skip={skip}"
        );
    }
    assert_eq!(
        read(None, 1, 0).await,
        (vec![137], 3),
        "the whole contract: a full page, then its count"
    );
    sqlx::query("DELETE FROM shop.items WHERE contract_address = $1")
        .bind(&contract)
        .execute(&pool)
        .await
        .unwrap();
}

/// The `public_item_order` ids the shared `unified_trades` view serves now.
async fn open_item_order_ids(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "WITH {} SELECT id::text FROM unified_trades WHERE type = 'public_item_order'",
        super::catalog::unified_trades_cte("$1", "$2")
    )))
    .bind(chrono::Utc::now().naive_utc())
    .bind(None::<chrono::NaiveDateTime>)
    .fetch_all(pool)
    .await
    .unwrap()
}

fn item_order_json(signer: &str, contract: &str) -> String {
    format!(
        r#"{{ "signer": "{signer}", "signature": "0x00", "type": "public_item_order", "network": "MATIC", "chainId": 137,
             "checks": {{ "uses": 1, "expiration": 4102444800000, "effective": 0, "salt": "0x1234", "contractSignatureIndex": 0, "signerSignatureIndex": 0, "allowedRoot": "0x", "externalChecks": [] }},
             "sent": [ {{"assetType": 4, "contractAddress": "{contract}", "itemId": "1", "extra": "0x"}} ],
             "received": [ {{"assetType": 1, "contractAddress": "0xa1c57f48f0deb89f569dfbe6e2b7f46d33606fd4", "amount": "1000000000000000000", "extra": "0x", "beneficiary": "{signer}"}} ] }}"#
    )
}

/// On a network the collections index owns, a public item order's signer
/// is checked against the item's creator under the index's authorizing
/// publication, read in the statement that reads the creator.
#[tokio::test]
async fn on_an_owned_network_an_item_order_needs_an_authorizing_publication() {
    let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
        return;
    };
    use super::catalogue::owning;
    super::trade_events::tests::publish_cursors(&pool).await;
    let contract = contract_of(&tag(), 9);
    let marketplace = listings::offchain_marketplace_v3(137).unwrap();
    let mut trade = listings::test_support::parse(&item_order_json(
        &listings::test_support::signer().address().to_checksum(None),
        &contract,
    ));
    listings::test_support::sign(&mut trade, &marketplace);
    let signer = trade.signer.clone();
    seed_item_row(&pool, &contract, "1", WALLET_A).await;
    sqlx::query("UPDATE shop.items SET creator = lower($2) WHERE contract_address = $1")
        .bind(&contract)
        .bind(&signer)
        .execute(&pool)
        .await
        .unwrap();
    let unavailable = |e: listings::TradeCreationError| match e {
        listings::TradeCreationError::CatalogueUnavailable(why) => why,
        other => panic!("expected the index to refuse: {other}"),
    };

    // A creatorship transfer walked past the publication, a stale
    // publication, a rewind: no order is authorized on them.
    owning::own(&pool, "matic", 137, 100, 60).await;
    owning::walk_past(&pool, "matic", 101, &contract).await;
    let why = unavailable(
        listings::create_trade(&pool, None, &trade, &signer, 1_000)
            .await
            .unwrap_err(),
    );
    assert!(why.contains("walked"), "{why}");
    owning::own(&pool, "matic", 137, 101, 31 * 60).await;
    unavailable(
        listings::create_trade(&pool, None, &trade, &signer, 1_000)
            .await
            .unwrap_err(),
    );
    owning::rewind(&pool, "matic").await;
    unavailable(
        listings::create_trade(&pool, None, &trade, &signer, 1_000)
            .await
            .unwrap_err(),
    );

    // Published a minute ago: the creator signs it.
    owning::own(&pool, "matic", 137, 101, 60).await;
    let id = listings::create_trade(&pool, None, &trade, &signer, 1_000)
        .await
        .unwrap();
    crate::test_db::open_trade_verdict(&pool, &id).await;
    assert!(open_item_order_ids(&pool).await.contains(&id));
}

#[tokio::test]
async fn a_signed_item_order_is_stored_once_and_only_behind_the_write_switch() {
    use alloy::signers::SignerSync;
    let Some(state) = state_with_db(Settings {
        data_writes_enabled: true,
        ..Settings::for_tests()
    })
    .await
    else {
        return;
    };
    let pool = state.pool.clone();
    super::trade_events::tests::publish_cursors(&pool).await;
    let t = tag();
    let contract = contract_of(&t, 9);
    let marketplace = listings::offchain_marketplace_v3(137).unwrap();

    // The direct path: an item order needs no ownership call; the second insert is the duplicate.
    let mut trade = listings::test_support::parse(&item_order_json(
        &listings::test_support::signer().address().to_checksum(None),
        &contract,
    ));
    listings::test_support::sign(&mut trade, &marketplace);
    let signer = trade.signer.clone();
    // A primary sale is the creator's: an unmirrored item, or another
    // creator's, is refused before the insert.
    assert!(matches!(
        listings::create_trade(&pool, None, &trade, &signer, 1_000)
            .await
            .unwrap_err(),
        listings::TradeCreationError::ItemUnknown
    ));
    seed_item_row(&pool, &contract, "1", WALLET_A).await;
    assert!(matches!(
        listings::create_trade(&pool, None, &trade, &signer, 1_000)
            .await
            .unwrap_err(),
        listings::TradeCreationError::NotTheCreator
    ));
    // The rows a reader serves leave out an item order its creator did not sign.
    let foreign = listings::insert_trade(&pool, &trade, "MATIC", marketplace.address)
        .await
        .unwrap();
    assert!(!open_item_order_ids(&pool).await.contains(&foreign));
    delete_trade(&pool, &foreign).await;
    sqlx::query("UPDATE shop.items SET creator = lower($2) WHERE contract_address = $1")
        .bind(&contract)
        .bind(&signer)
        .execute(&pool)
        .await
        .unwrap();
    let id = listings::create_trade(&pool, None, &trade, &signer, 1_000)
        .await
        .unwrap();
    assert!(!open_item_order_ids(&pool).await.contains(&id));
    crate::test_db::open_trade_verdict(&pool, &id).await;
    assert!(open_item_order_ids(&pool).await.contains(&id));
    assert!(matches!(
        listings::create_trade(&pool, None, &trade, &signer, 1_000)
            .await
            .unwrap_err(),
        listings::TradeCreationError::Duplicate
    ));
    seed_item_row(&pool, &contract_of(&t, 8), "1", &sf::address()).await;
    let (kind, stored_signer, contract_col, assets): (String, String, String, i64) = sqlx::query_as(
        "SELECT t.type::text, t.signer, t.contract, (SELECT count(*) FROM shop.trade_assets a WHERE a.trade_id = t.id) FROM shop.trades t WHERE t.id = $1::uuid",
    )
    .bind(&id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (kind.as_str(), stored_signer, contract_col.as_str(), assets),
        (
            "public_item_order",
            signer.to_lowercase(),
            marketplace.address,
            2
        )
    );
    let item_id: String = sqlx::query_scalar("SELECT i.item_id FROM shop.trade_assets_item i JOIN shop.trade_assets a ON a.id = i.asset_id WHERE a.trade_id = $1::uuid").bind(&id).fetch_one(&pool).await.unwrap();
    assert_eq!(item_id, "1");
    let (status, body) = get(market_router(state.clone()), &format!("/v1/trades/{id}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["type"], "public_item_order");
    assert!(
        body["data"].get("mirroredFrom").is_none(),
        "a local trade: {}",
        body["data"]
    );

    // The route: the auth-chain wallet signs both the fetch and the EIP-712 trade.
    let wallet: alloy::signers::local::PrivateKeySigner = sf::KEY.parse().unwrap();
    let mut trade =
        listings::test_support::parse(&item_order_json(&sf::address(), &contract_of(&t, 8)));
    let hash = listings::signing_hash(&trade, &marketplace).unwrap();
    trade.signature = format!(
        "0x{}",
        hex::encode(wallet.sign_hash_sync(&hash).unwrap().as_bytes())
    );
    let body_json: Value = serde_json::from_str(
        &item_order_json(&sf::address(), &contract_of(&t, 8)).replace(
            "\"signature\": \"0x00\"",
            &format!("\"signature\": \"{}\"", trade.signature),
        ),
    )
    .unwrap();
    let metadata = r#"{"signer":"dcl:marketplace","intent":"dcl:create-trade"}"#;
    let (status, body) = call(
        market_router(state.clone()),
        "POST",
        "/v1/trades",
        signed_headers("post", "/v1/trades", metadata, true).await,
        Some(body_json.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["ok"], true);
    let created = body["data"]["id"].as_str().unwrap().to_string();
    // The 201 carries the stored trade, `received` included, as the read answers it.
    assert_eq!(
        body["data"]["received"].as_array().map(Vec::len),
        Some(1),
        "{body}"
    );
    let (status, read) = get(
        market_router(state.clone()),
        &format!("/v1/trades/{created}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{read}");
    assert_eq!(body["data"], read["data"]);
    let (status, body) = call(
        market_router(state.clone()),
        "POST",
        "/v1/trades",
        signed_headers("post", "/v1/trades", metadata, true).await,
        Some(body_json.clone()),
    )
    .await;
    assert_eq!(
        (status, body["ok"].as_bool()),
        (StatusCode::CONFLICT, Some(false)),
        "{body}"
    );
    let (status, body) = call(
        market_router(state.clone()),
        "POST",
        "/v1/trades",
        signed_headers(
            "post",
            "/v1/trades",
            r#"{"signer":"dcl:marketplace"}"#,
            true,
        )
        .await,
        Some(body_json.clone()),
    )
    .await;
    assert_eq!(
        (status, body["message"].as_str()),
        (
            StatusCode::BAD_REQUEST,
            Some("Invalid auth intent to perform this operation")
        )
    );
    let (status, body) = call(
        market_router(state.clone()),
        "POST",
        "/v1/trades",
        HeaderMap::new(),
        Some(body_json.clone()),
    )
    .await;
    assert_eq!(
        (status, body["message"].as_str()),
        (StatusCode::BAD_REQUEST, Some("Invalid auth signer"))
    );
    let mut other_signer = body_json.clone();
    other_signer["signer"] = json!(WALLET_A);
    let (status, body) = call(
        market_router(state.clone()),
        "POST",
        "/v1/trades",
        signed_headers("post", "/v1/trades", metadata, true).await,
        Some(other_signer),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "the authenticated address is not the trade signer: {body}"
    );

    // Writes off: the same request is the switch's 503 before any verification.
    let off = state_without_db_settings(Settings::for_tests());
    let (status, body) = call(
        market_router(off),
        "POST",
        "/v1/trades",
        HeaderMap::new(),
        Some(body_json),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");

    // A signature this node does not store is the upstream 404, index or not.
    let (status, body) = get(market_router(state.clone()), "/v1/trades/0xabc/accept").await;
    assert_eq!(
        (status, body["ok"].as_bool()),
        (StatusCode::NOT_FOUND, Some(false))
    );

    for id in [id, created] {
        delete_trade(&pool, &id).await;
    }
    cleanup_mirror(&pool, &[contract, contract_of(&t, 8)]).await;
}

fn item_bid_json(signer: &str, contract: &str, item_id: &str) -> String {
    format!(
        r#"{{ "signer": "{signer}", "signature": "0x00", "type": "bid", "network": "MATIC", "chainId": 137,
             "checks": {{ "uses": 1, "expiration": 4102444800000, "effective": 0, "salt": "0x1234", "contractSignatureIndex": 0, "signerSignatureIndex": 0, "allowedRoot": "0x", "externalChecks": [] }},
             "sent": [ {{"assetType": 1, "contractAddress": "0xa1c57f48f0deb89f569dfbe6e2b7f46d33606fd4", "amount": "1500000000000000000", "extra": "0x"}} ],
             "received": [ {{"assetType": 4, "contractAddress": "{contract}", "itemId": "{item_id}", "extra": "0x", "beneficiary": "{signer}"}} ] }}"#
    )
}

async fn bid_notifications(pool: &PgPool, address: &str) -> Vec<(uuid::Uuid, Value)> {
    sqlx::query_as("SELECT id, metadata FROM social.notifications WHERE address = $1 AND type = 'bid_received'")
        .bind(address)
        .fetch_all(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_local_bid_notifies_the_item_creator_once() {
    let Some(state) = state_with_db(Settings::for_tests()).await else {
        return;
    };
    let pool = state.pool.clone();
    let t = tag();
    let contract = contract_of(&t, 7);
    let creator = contract_of(&t, 6);
    sqlx::query(
        "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, \
             total_supply, max_supply, available, price, image, name, category, created_at, updated_at, reviewed_at, mirrored_at) \
         VALUES ('matic', $1, 1, 137, 'wearable_v2', $2, $3, 'legendary', 0, 10, 10, 0, 'https://img.example/1.png', 'Space Jams', 'hat', 0, 0, 0, now())",
    )
    .bind(&contract)
    .bind(format!("urn:decentraland:matic:collections-v2:{contract}:1"))
    .bind(&creator)
    .execute(&pool)
    .await
    .unwrap();
    let marketplace = listings::offchain_marketplace_v3(137).unwrap();
    let signer = listings::test_support::signer().address().to_checksum(None);
    let bid = |item: &str| {
        let mut trade = listings::test_support::parse(&item_bid_json(&signer, &contract, item));
        listings::test_support::sign(&mut trade, &marketplace);
        trade
    };

    let trade = bid("1");
    let id = listings::create_trade(&pool, Some(&state), &trade, &signer, 1_000)
        .await
        .unwrap();
    assert!(matches!(
        listings::create_trade(&pool, Some(&state), &trade, &signer, 1_000)
            .await
            .unwrap_err(),
        listings::TradeCreationError::Duplicate
    ));
    // A bid on an item no index describes is stored without a notification.
    let unknown = listings::create_trade(&pool, Some(&state), &bid("2"), &signer, 1_000)
        .await
        .unwrap();
    let rows = bid_notifications(&pool, &creator).await;
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(
        rows[0].1,
        json!({
            "image": "https://img.example/1.png",
            "seller": creator,
            "category": "wearable",
            "rarity": "legendary",
            "link": "https://decentraland.org/marketplace/account?section=bids",
            "nftName": "Space Jams",
            "price": "1500000000000000000",
            "title": "Bid Received",
            "description": "You received a bid of 1.5 MANA for this Space Jams.",
            "network": "MATIC",
        })
    );

    sqlx::query("DELETE FROM social.notifications WHERE address = $1")
        .bind(&creator)
        .execute(&pool)
        .await
        .unwrap();
    delete_trade(&pool, &id).await;
    delete_trade(&pool, &unknown).await;
    cleanup_mirror(&pool, &[contract]).await;
}

/// A bid on the bidder's own item notifies nobody, and a failing
/// notification insert leaves the bid stored.
#[tokio::test]
async fn a_bid_stands_without_its_notification() {
    let Some(state) = state_with_db(Settings::for_tests()).await else {
        return;
    };
    let pool = state.pool.clone();
    let t = tag();
    let contract = contract_of(&t, 8);
    let creator = contract_of(&t, 9);
    let signer = listings::test_support::signer().address().to_checksum(None);
    let own = signer.to_ascii_lowercase();
    for (item, by) in [("1", &creator), ("2", &own)] {
        sqlx::query(
            "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, \
                 total_supply, max_supply, available, price, image, name, category, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ('matic', $1, $2::numeric, 137, 'wearable_v2', $3, $4, 'rare', 0, 10, 10, 0, 'https://img.example/1.png', 'Hat', 'hat', 0, 0, 0, now())",
        )
        .bind(&contract)
        .bind(item)
        .bind(format!("urn:decentraland:matic:collections-v2:{contract}:{item}"))
        .bind(by)
        .execute(&pool)
        .await
        .unwrap();
    }
    let marketplace = listings::offchain_marketplace_v3(137).unwrap();
    let bid = |item: &str| {
        let mut trade = listings::test_support::parse(&item_bid_json(&signer, &contract, item));
        listings::test_support::sign(&mut trade, &marketplace);
        trade
    };

    let self_bid = listings::create_trade(&pool, Some(&state), &bid("2"), &signer, 1_000)
        .await
        .unwrap();
    assert!(
        bid_notifications(&pool, &own).await.is_empty(),
        "no self-notification"
    );

    let func = format!("fail_bid_{t}");
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE FUNCTION public.{func}() RETURNS trigger LANGUAGE plpgsql AS $f$ \
         BEGIN IF NEW.address = '{creator}' THEN RAISE EXCEPTION 'notification refused'; END IF; RETURN NEW; END $f$"
    )))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE TRIGGER {func} BEFORE INSERT ON social.notifications FOR EACH ROW EXECUTE FUNCTION public.{func}()"
    )))
    .execute(&pool)
    .await
    .unwrap();
    let stored = listings::create_trade(&pool, Some(&state), &bid("1"), &signer, 1_000).await;
    for drop in [
        format!("DROP TRIGGER {func} ON social.notifications"),
        format!("DROP FUNCTION public.{func}()"),
    ] {
        sqlx::query(sqlx::AssertSqlSafe(drop))
            .execute(&pool)
            .await
            .unwrap();
    }
    let id = stored.expect("the bid is created although its notification failed");
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM shop.trades WHERE id = $1::uuid)")
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(exists);
    assert!(bid_notifications(&pool, &creator).await.is_empty());

    delete_trade(&pool, &id).await;
    delete_trade(&pool, &self_bid).await;
    cleanup_mirror(&pool, &[contract]).await;
}

/// marketplace-server ae872a4a `validateTradeByType` refuses a second open
/// order for an item or NFT and a second open bid by the same bidder on the
/// same asset (409); a closed one no longer counts.
#[tokio::test]
async fn a_second_open_order_or_bid_on_the_same_asset_is_refused() {
    let Some(state) = state_with_db(Settings::for_tests()).await else {
        return;
    };
    let pool = state.pool.clone();
    let t = tag();
    let contract = contract_of(&t, 4);
    let marketplace = listings::offchain_marketplace_v3(137).unwrap();
    let signer = listings::test_support::signer().address().to_checksum(None);
    seed_item_row(&pool, &contract, "1", &signer).await;
    let signed = |json: String, salt: &str| {
        let mut trade = listings::test_support::parse(
            &json.replace(r#""salt": "0x1234""#, &format!(r#""salt": "{salt}""#)),
        );
        listings::test_support::sign(&mut trade, &marketplace);
        trade
    };
    let create = |trade: listings::TradeCreation| {
        let (pool, signer) = (pool.clone(), signer.clone());
        let now_ms = chrono::Utc::now().timestamp_millis();
        async move { listings::create_trade(&pool, None, &trade, &signer, now_ms).await }
    };

    let first = create(signed(item_order_json(&signer, &contract), "0x01"))
        .await
        .unwrap();
    let second = create(signed(item_order_json(&signer, &contract), "0x02")).await;
    let err = second.unwrap_err();
    assert!(
        matches!(err, listings::TradeCreationError::DuplicateOpen(_)),
        "{err}"
    );
    assert_eq!(
        err.to_string(),
        "There is already an open order for this Item"
    );
    // A first order without a verified V3 digest is left out of the
    // listings, but still counts: it may still execute on chain, or be
    // verified later, and two open orders would then be listed.
    sqlx::query("INSERT INTO shop.trade_chain_state (trade_id, network, status, folded_block, digest_failed_at) VALUES ($1::uuid, 'matic', 'open', 10, timezone('UTC', now()))
         ON CONFLICT (trade_id) DO UPDATE SET digest_failed_at=EXCLUDED.digest_failed_at")
        .bind(&first)
        .execute(&pool)
        .await
        .unwrap();
    let marked = create(signed(item_order_json(&signer, &contract), "0x03")).await;
    assert!(
        matches!(marked, Err(listings::TradeCreationError::DuplicateOpen(_))),
        "{marked:?}"
    );
    sqlx::query("DELETE FROM shop.trade_chain_state WHERE trade_id = $1::uuid")
        .bind(&first)
        .execute(&pool)
        .await
        .unwrap();
    // Once the first order is closed, the item can be listed again.
    sqlx::query("UPDATE shop.trades SET expires_at = '2000-01-01' WHERE id = $1::uuid")
        .bind(&first)
        .execute(&pool)
        .await
        .unwrap();
    let relisted = create(signed(item_order_json(&signer, &contract), "0x02"))
        .await
        .unwrap();

    let bid = create(signed(item_bid_json(&signer, &contract, "7"), "0x03"))
        .await
        .unwrap();
    let err = create(signed(item_bid_json(&signer, &contract, "7"), "0x04"))
        .await
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "There is already a bid with the same parameters"
    );
    // Another item is another asset.
    let other_bid = create(signed(item_bid_json(&signer, &contract, "8"), "0x04"))
        .await
        .unwrap();

    for id in [first, relisted, bid, other_bid] {
        delete_trade(&pool, &id).await;
    }
    cleanup_mirror(&pool, &[contract]).await;
}

/// An open NFT order blocks only its own signer's second order: another
/// wallet's still-open order (a previous owner's, never executed) does not
/// block the current owner, and a cancelled order releases the token.
#[tokio::test]
async fn an_open_nft_order_blocks_only_its_signers_second_order() {
    let Some(state) = state_with_db(Settings::for_tests()).await else {
        return;
    };
    let pool = state.pool.clone();
    let t = tag();
    let contract = contract_of(&t, 5);
    let marketplace = listings::offchain_marketplace_v3(137).unwrap();
    let signer = listings::test_support::signer().address().to_checksum(None);
    let order = |token_id: &str, salt: &str, by: &str| {
        let mut trade = listings::test_support::parse(
            &listings::test_support::trade_json_on(&signer, "0x00", 137)
                .replace(r#""salt": "0x1234""#, &format!(r#""salt": "{salt}""#)),
        );
        trade.sent[0].contract_address = contract.clone();
        trade.sent[0].token_id = Some(token_id.to_string());
        listings::test_support::sign(&mut trade, &marketplace);
        trade.signer = by.to_string();
        trade
    };
    let now = chrono::Utc::now().naive_utc();
    let check = |trade: listings::TradeCreation| {
        let pool = pool.clone();
        async move {
            let mut tx = pool.begin().await.unwrap();
            let out = listings::refuse_duplicate_open(&mut tx, &trade, "MATIC", None, now).await;
            tx.rollback().await.unwrap();
            out
        }
    };
    let mine = listings::insert_trade(
        &pool,
        &order("5", "0x31", &signer),
        "MATIC",
        marketplace.address,
    )
    .await
    .unwrap();
    let previous_owner = listings::insert_trade(
        &pool,
        &order("6", "0x32", WALLET_A),
        "MATIC",
        marketplace.address,
    )
    .await
    .unwrap();

    let err = check(order("5", "0x33", &signer)).await.unwrap_err();
    assert_eq!(
        err.to_string(),
        "There is already an open order for this NFT"
    );
    check(order("5", "0x33", WALLET_A))
        .await
        .expect("another wallet's order is not this signer's duplicate");
    check(order("6", "0x34", &signer))
        .await
        .expect("a previous owner's open order does not block the current owner");

    let hashed: String =
        sqlx::query_scalar("SELECT hashed_signature FROM shop.trades WHERE id = $1::uuid")
            .bind(&mine)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query(
        "INSERT INTO shop.trade_cancellations (target_hashed_signature, kind, signer, signature) VALUES ($1, 'signer', lower($2), '0x00')",
    )
    .bind(&hashed)
    .bind(&signer)
    .execute(&pool)
    .await
    .unwrap();
    check(order("5", "0x33", &signer))
        .await
        .expect("a cancelled order releases the token");

    for id in [mine, previous_owner] {
        delete_trade(&pool, &id).await;
    }
}

/// A token or item id stored before the create path checked it (an empty
/// string, a hex spelling) is left out of the open bids and listings, never a
/// cast error that fails the whole read.
#[tokio::test]
async fn a_stored_malformed_token_id_does_not_break_the_bid_or_listing_reads() {
    let Some(state) = state_with_db(Settings::for_tests()).await else {
        return;
    };
    let pool = state.pool.clone();
    super::trade_events::tests::publish_cursors(&pool).await;
    let t = tag();
    let contract = contract_of(&t, 3);
    let marketplace = listings::offchain_marketplace_v3(137).unwrap();
    let signer = listings::test_support::signer().address().to_checksum(None);
    let nft_bid = |token_id: &str, salt: &str| {
        let mut trade = listings::test_support::parse(
            &item_bid_json(&signer, &contract, "1")
                .replace(r#""assetType": 4"#, r#""assetType": 3"#)
                .replace(r#""itemId": "1""#, &format!(r#""tokenId": "{token_id}""#))
                .replace(r#""salt": "0x1234""#, &format!(r#""salt": "{salt}""#)),
        );
        listings::test_support::sign(&mut trade, &marketplace);
        trade
    };
    let mut ids = Vec::new();
    for (token_id, salt) in [("", "0x11"), ("0x2a", "0x12"), ("42", "0x13")] {
        ids.push(
            listings::insert_trade(
                &pool,
                &nft_bid(token_id, salt),
                "MATIC",
                marketplace.address,
            )
            .await
            .unwrap(),
        );
    }
    let mut order =
        listings::test_support::parse(&listings::test_support::trade_json_on(&signer, "0x00", 137));
    order.sent[0].contract_address = contract.clone();
    order.sent[0].token_id = Some(String::new());
    listings::test_support::sign(&mut order, &marketplace);
    ids.push(
        listings::insert_trade(&pool, &order, "MATIC", marketplace.address)
            .await
            .unwrap(),
    );

    let bids: Vec<(String, String)> =
        sqlx::query_as(sqlx::AssertSqlSafe(super::bids::test_trades_bids_sql()))
            .bind(chrono::Utc::now().naive_utc())
            .bind(None::<chrono::NaiveDateTime>)
            .fetch_all(&pool)
            .await
            .expect("one malformed row does not fail the bids read");
    let listed: Vec<&String> = bids
        .iter()
        .map(|(id, _)| id)
        .filter(|id| ids.contains(id))
        .collect();
    assert_eq!(listed, vec![&ids[2]], "{bids:?}");
    let listings: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "WITH {} SELECT id::text FROM unified_trades",
        super::catalog::unified_trades_cte("$1", "$2")
    )))
    .bind(chrono::Utc::now().naive_utc())
    .bind(None::<chrono::NaiveDateTime>)
    .fetch_all(&pool)
    .await
    .expect("one malformed row does not fail the listings read");
    assert!(!listings.contains(&ids[3]));

    for id in ids {
        delete_trade(&pool, &id).await;
    }
}

/// A read that names a contract reads only that contract's open trades; its
/// rows are those of the read over every open trade, restricted to it.
#[tokio::test]
async fn a_read_naming_a_contract_keeps_the_rows_of_the_whole_read() {
    use super::shop::{
        FeedClock, build_shop_listings_sql, build_unified_items_sql, build_unified_listings_sql,
        parse_shop_filters, parse_unified_filters,
    };
    let Some(state) = state_with_db(Settings::for_tests()).await else {
        return;
    };
    let pool = state.pool.clone();
    super::trade_events::tests::publish_cursors(&pool).await;
    let t = tag();
    let (c1, c2) = (contract_of(&t, 1), contract_of(&t, 2));
    for (c, item) in [(&c1, "0"), (&c1, "1"), (&c2, "0")] {
        seed_item_row(&pool, c, item, CREATOR).await;
    }
    sqlx::query("UPDATE shop.items SET search_is_collection_approved = true WHERE contract_address = ANY($1)")
        .bind(vec![c1.clone(), c2.clone()])
        .execute(&pool)
        .await
        .unwrap();
    let mut ids = Vec::new();
    for (c, kind, sent_type, value, received_type, amount) in [
        (&c1, "public_item_order", 4, "0", 2, "250000000000000000"),
        (&c1, "public_item_order", 4, "1", 1, "5000000000000000000"),
        (&c1, "public_nft_order", 3, "7", 2, "400000000000000000"),
        (&c1, "public_nft_order", 3, "8", 1, "6000000000000000000"),
        (&c2, "public_item_order", 4, "0", 2, "300000000000000000"),
    ] {
        let asset = |asset_type, contract: &str, value: &str| Asset {
            asset_type,
            contract: contract.to_string(),
            value: value.to_string(),
        };
        let sent = asset(sent_type, c, value);
        let received = asset(
            received_type,
            "0x7ad72b9f944ea9793cf4055d88f81138cc2c63a0",
            amount,
        );
        ids.push(insert_trade(&pool, kind, CREATOR, sent, received).await.0);
    }

    let rate = crate::types::ManaUsdRate::parse_usd_per_mana("0.5").unwrap();
    let build = |kind: &str, pairs: &[(String, String)]| match kind {
        "shop" => build_shop_listings_sql(&parse_shop_filters(pairs), FeedClock::unbounded()),
        "items" => {
            build_unified_items_sql(&parse_unified_filters(pairs), rate, FeedClock::unbounded())
        }
        _ => {
            build_unified_listings_sql(&parse_unified_filters(pairs), rate, FeedClock::unbounded())
        }
    };
    let rows = async |b: super::catalog::Builder| -> Vec<Value> {
        let sql = format!("SELECT to_jsonb(q) - 'total' FROM ({}) q", b.sql);
        sqlx::query_scalar_with(sqlx::AssertSqlSafe(sql), b.args)
            .fetch_all(&pool)
            .await
            .unwrap()
    };
    for (kind, params) in [
        ("listings", &[("includeLegacySecondary", "true")][..]),
        ("listings", &[("itemId", "0"), ("listingType", "primary")]),
        ("listings", &[("itemId", "1"), ("source", "legacy")]),
        ("items", &[("includeLegacySecondary", "true")]),
        ("shop", &[]),
    ] {
        let whole: Vec<(String, String)> = params
            .iter()
            .chain(&[("first", "1000")])
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let mut named = whole.clone();
        named.push((
            "contractAddress".to_string(),
            c1.to_uppercase().replace("0X", "0x"),
        ));
        let (named, whole) = (build(kind, &named), build(kind, &whole));
        assert!(
            named
                .sql
                .contains("sent.contract_address IN (SELECT unnest(")
                && !whole.sql.contains("unnest("),
            "{}",
            named.sql
        );
        let of_c1: Vec<Value> = rows(whole)
            .await
            .into_iter()
            .filter(|r| r["contract_address"] == c1.as_str())
            .collect();
        assert!(!of_c1.is_empty(), "{kind} {params:?}");
        assert_eq!(rows(named).await, of_c1, "{kind} {params:?}");
    }

    for id in ids {
        delete_trade(&pool, &id).await;
    }
    cleanup_mirror(&pool, &[c1, c2]).await;
}

#[tokio::test]
async fn admin_moderation_is_bearer_gated_audited_and_idempotent() {
    let Some(state) = state_with_db(
        Settings {
            data_writes_enabled: true,
            ..Settings::for_tests()
        }
        .with_market(MarketSettings {
            admin_token: Some("s3cr3t".to_string()),
            ..MarketSettings::default()
        }),
    )
    .await
    else {
        return;
    };
    let pool = state.pool.clone();
    let t = tag();
    let (id, hash) = insert_trade(
        &pool,
        "public_nft_order",
        WALLET_A,
        Asset {
            asset_type: 3,
            contract: contract_of(&t, 5),
            value: "42".into(),
        },
        Asset {
            asset_type: 1,
            contract: "0x2222222222222222222222222222222222222222".into(),
            value: "1".into(),
        },
    )
    .await;
    let app = || market_router(state.clone());

    let (status, body) = call(
        app(),
        "POST",
        &format!("/v1/admin/moderation/order/{hash}/flag"),
        HeaderMap::new(),
        Some(json!({ "severity": "hide" })),
    )
    .await;
    assert_eq!(
        (status, body["ok"].as_bool()),
        (StatusCode::FORBIDDEN, Some(false)),
        "{body}"
    );
    let (status, body) = call(
        app(),
        "POST",
        &format!("/v1/admin/moderation/order/{hash}/flag"),
        bearer("nope"),
        Some(json!({ "severity": "hide" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    let (status, body) = call(
        app(),
        "POST",
        &format!("/v1/admin/moderation/order/{hash}/flag"),
        bearer("s3cr3t"),
        Some(json!({ "severity": "hide", "reason": "spam" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({ "ok": true, "target_kind": "order", "target_hash": hash, "severity": "hide" })
    );
    let (status, body) = call(
        app(),
        "POST",
        &format!("/v1/admin/moderation/order/0x{}/flag", "9".repeat(64)),
        bearer("s3cr3t"),
        Some(json!({ "severity": "hide" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "an unknown target: {body}");
    let (status, body) = call(
        app(),
        "GET",
        "/v1/admin/moderation/flags?kind=order",
        bearer("s3cr3t"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["target_hash"] == hash && f["severity"] == "hide"),
        "{body}"
    );
    let (status, body) = call(
        app(),
        "POST",
        &format!("/v1/admin/disputes/{hash}/open"),
        bearer("s3cr3t"),
        Some(json!({ "reason": "buyer complaint" })),
    )
    .await;
    assert_eq!(
        (status, body["status"].as_str()),
        (StatusCode::OK, Some("open")),
        "{body}"
    );
    let (status, body) = call(
        app(),
        "POST",
        &format!("/v1/admin/disputes/{hash}/resolve"),
        bearer("s3cr3t"),
        Some(json!({ "status": "resolved", "resolution": "refunded" })),
    )
    .await;
    assert_eq!(
        (status, body["status"].as_str()),
        (StatusCode::OK, Some("resolved")),
        "{body}"
    );
    let (status, body) = call(
        app(),
        "GET",
        "/v1/admin/disputes?status=resolved",
        bearer("s3cr3t"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["trade_hash"] == hash),
        "{body}"
    );
    let (status, first) = call(
        app(),
        "POST",
        &format!("/v1/admin/listings/order/{hash}/force-cancel"),
        bearer("s3cr3t"),
        Some(json!({ "reason": "stolen" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert!(first.get("already_cancelled").is_none());
    let (status, again) = call(
        app(),
        "POST",
        &format!("/v1/admin/listings/order/{hash}/force-cancel"),
        bearer("s3cr3t"),
        None,
    )
    .await;
    assert_eq!(
        (status, again["already_cancelled"].as_bool()),
        (StatusCode::OK, Some(true)),
        "{again}"
    );
    assert_eq!(first["cancellation_hash"], again["cancellation_hash"]);
    let (kind, signer): (String, String) = sqlx::query_as(
        "SELECT kind, signer FROM shop.trade_cancellations WHERE target_hashed_signature = $1",
    )
    .bind(&hash)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (kind.as_str(), signer.as_str()),
        ("operator", "operator:admin-token")
    );
    let (status, body) = call(
        app(),
        "DELETE",
        &format!("/v1/admin/moderation/order/{hash}/flag"),
        bearer("s3cr3t"),
        None,
    )
    .await;
    assert_eq!(
        (status, body["removed"].as_bool()),
        (StatusCode::OK, Some(true)),
        "{body}"
    );
    let (status, body) = call(
        app(),
        "GET",
        &format!("/v1/admin/audit?target={hash}"),
        bearer("s3cr3t"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let actions: Vec<&str> = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|a| a["action"].as_str())
        .collect();
    assert!(
        actions.len() >= 5 && actions.iter().all(|a| !a.is_empty()),
        "{actions:?}"
    );

    // Writes off: the moderation writes are the switch's 503, the reads still need the bearer.
    let off = state_without_db_settings(Settings::for_tests().with_market(MarketSettings {
        admin_token: Some("s3cr3t".to_string()),
        ..MarketSettings::default()
    }));
    let (status, _) = call(
        market_router(off.clone()),
        "POST",
        &format!("/v1/admin/moderation/order/{hash}/flag"),
        bearer("s3cr3t"),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let (status, _) = call(
        market_router(off),
        "GET",
        "/v1/admin/audit",
        HeaderMap::new(),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    sqlx::query("DELETE FROM shop.admin_audit WHERE target_hash = $1")
        .bind(&hash)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM shop.disputes WHERE trade_hash = $1")
        .bind(&hash)
        .execute(&pool)
        .await
        .unwrap();
    delete_trade(&pool, &id).await;
}

#[tokio::test]
async fn every_reader_route_names_its_missing_source_instead_of_answering_empty() {
    let state = state_without_db_settings(Settings::for_tests());
    let app = || market_router(state.clone());
    for (uri, source) in [
        ("/v1/nfts?first=1", "nft-index"),
        ("/v1/orders?first=1", "collections-subgraph:matic"),
        ("/v1/bids", "collections-subgraph:matic"),
        ("/v1/sales?first=1", "sales-index"),
        (
            "/v1/sales/summary?seller=0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "sales-index",
        ),
        (
            "/v1/owners?contractAddress=0xa&itemId=1",
            "collections-subgraph:matic",
        ),
        ("/v1/accounts", "sales-index"),
        ("/v1/rankings/wearables/week", "sales-index"),
        ("/v1/volume/week", "sales-index"),
        (
            "/v1/users/0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/wearables",
            "nft-index",
        ),
        // The catalogue routes (`/v1/collections`, `/v1/contracts`,
        // `/v1/catalog`, `/v1/items`, `/v1/trendings`, `/v3/catalog/shop`,
        // the item prices)
        // read which source owns each network from the database: an index
        // that owns one answers without a subgraph
        // (`tests::without_a_subgraph_descriptive_reads_serve_the_recorded_copy`,
        // `catalogue::tests::without_a_subgraph_a_priced_read_needs_a_source_per_network`).
        ("/v1/nfts?search=hat", "nft-index"),
        ("/v1/nfts?first=1&skip=6000", "nft-index"),
        ("/v1/stats/estate/size", "nft-index"),
        ("/v1/prices?category=estate", "nft-index"),
    ] {
        let (status, body) = get(app(), uri).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{uri}: {body}");
        assert_eq!(body["source"].as_str(), Some(source), "{uri}: {body}");
        assert_eq!(body["error"], "Service Unavailable", "{uri}");
    }
    // The NAMEs: on mainnet the nft index is their source even where this
    // process does not walk it (another may publish into its database), so
    // a database that does not answer is a failed read; off mainnet without
    // the subgraph the source is missing.
    let names = "/v1/users/0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/names";
    let (status, body) = get(app(), names).await;
    assert_eq!(
        (status, body["message"].as_str()),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Some("upstream reader unavailable")
        ),
        "{body}"
    );
    let testnet = state_without_db_settings(Settings {
        eth_network: "sepolia".into(),
        ..Settings::for_tests()
    });
    let (status, body) = get(market_router(testnet), names).await;
    assert_eq!(
        (status, body["source"].as_str()),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Some("marketplace-subgraph")
        ),
        "{body}"
    );
    let (status, body) = get(app(), "/v1/coupons").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (status, body) = get(app(), "/v1/rankings/houses/week").await;
    assert_eq!(
        (status, body["ok"].as_bool()),
        (StatusCode::BAD_REQUEST, Some(false))
    );
    let (status, body) = get(app(), "/v1/owners?itemId=1").await;
    assert_eq!(
        (status, body["message"].as_str()),
        (
            StatusCode::BAD_REQUEST,
            Some("itemId and contractAddress are necessary params.")
        )
    );
}

#[tokio::test]
async fn account_inventory_pages_indexed_participants_and_excludes_sender_only_wallets() {
    const CURATOR_ONLY: &str = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    const RELAY_ONLY: &str = "0x3434343434343434343434343434343434343434";
    const PAYER: &str = "0x5656565656565656565656565656565656565656";
    const MIXED_PAYER: &str = "0x5757575757575757575757575757575757575757";
    const MIXED_PAYEE: &str = "0x9191919191919191919191919191919191919191";
    const SELLER: &str = "0x7878787878787878787878787878787878787878";
    const OTHER_TOKEN: &str = "0x9090909090909090909090909090909090909090";
    const FORMER_MEMBER: &str = "0xdddddddddddddddddddddddddddddddddddddddd";
    const FUTURE_HOLDER: &str = "0xffffffffffffffffffffffffffffffffffffffff";
    const MINT_RECIPIENT: &str = "0x1234567890123456789012345678901234567890";
    let graph = FakeGraph::new();
    let url = serve_graph(graph.clone()).await;
    let Some((mut state, own)) = state_over_own(&url, false).await else {
        return;
    };
    let indexes = &mut Arc::get_mut(&mut state).unwrap().settings.market.indexes;
    indexes.sales_enabled = true;
    indexes.collections_enabled = true;
    indexes.committee_enabled = true;
    let pool = &own.pool;
    let app = market_router(state.clone());
    let (venue, deployment_block, _) = super::trade_events::deployments(137)[0];
    let height = deployment_block as i64 + 1;
    let hash = format!("0x{:064x}", 91);
    sqlx::query(
        "INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,last_block_hash,
          full_history,folded_block,folded_at,sale_input_version,sale_verified_through,sale_covered_rewinds)
         SELECT ix,n,CASE n WHEN 'matic' THEN 137 ELSE 1 END,1,$1,$2,true,$1,timezone('UTC',now()),$3,$1,0
         FROM unnest(ARRAY['nft','sales','trade_events','collections','committee']) ix
         CROSS JOIN unnest(ARRAY['matic','ethereum']) n WHERE n='matic' OR ix IN ('nft','sales','trade_events')",
    ).bind(height).bind(&hash).bind(super::sale_history::VERSION).execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO shop.sale_publication(network,block_number,block_hash,input_version,input_epochs,
            input_revisions,late_collections,published_at)
         SELECT network,$1,$2,$3,array_agg(sale_input_epoch ORDER BY indexer),
                array_agg(sale_history_revision ORDER BY indexer),0,timezone('UTC',now())
         FROM shop.chain_cursor WHERE indexer IN ('nft','sales','trade_events') GROUP BY network",
    ).bind(height).bind(&hash).bind(super::sale_fold::PUBLICATION_VERSION).execute(pool).await.unwrap();

    // The same 700 wallets received a different protocol cut on each network.
    sqlx::query(
        "INSERT INTO shop.sale_account_total(network,currency,address,sales,purchases,spent,gross_sold,
             net_sold,unknown_proceeds,received,royalties,fees_received)
         SELECT n,CASE n WHEN 'ethereum' THEN $1 ELSE $2 END,'0x'||lpad(to_hex(i),40,'0'),
                0,0,0,0,0,0,amount,0,amount FROM generate_series(1,700) i
         CROSS JOIN unnest(ARRAY['matic','ethereum']) n
         CROSS JOIN LATERAL (SELECT i*2+(n='matic')::int AS amount) v",
    ).bind(listings::mana_token(1)).bind(listings::mana_token(137)).execute(pool).await.unwrap();
    let (status, before) = get(app.clone(), "/v1/accounts?first=10&skip=995").await;
    assert_eq!(status, StatusCode::OK, "{before}");
    assert_eq!(before["total"], 1400);
    let (volume_status, warm_volume) = get(app.clone(), "/v1/volume/all").await;
    assert_eq!(volume_status, StatusCode::OK);
    let accounts_slot = state.sales_accounts_slots.try_acquire().unwrap();
    assert_eq!(
        get(app.clone(), "/v1/accounts?skip=111").await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    // An accounts miss cannot consume the independent rankings/volume budget.
    assert_eq!(get(app.clone(), "/v1/volume/week").await.0, StatusCode::OK);
    let analytics_slots = state.sales_analytics_slots.try_acquire_many(2).unwrap();
    // Forward cursor progress and freshness updates leave an already
    // published prefix usable while the auxiliary folds catch up.
    sqlx::query(
        "UPDATE shop.chain_cursor SET last_block=last_block+1,
        updated_at=timezone('UTC',now()),folded_at=timezone('UTC',now())
        WHERE network='matic' AND indexer IN ('collections','committee')",
    )
    .execute(pool)
    .await
    .unwrap();
    assert_eq!(
        get(app.clone(), "/v1/accounts?first=10&skip=995").await,
        (StatusCode::OK, before.clone())
    );
    assert_eq!(
        get(app.clone(), "/v1/volume/all").await,
        (StatusCode::OK, warm_volume)
    );
    assert_eq!(
        get(app.clone(), "/v1/accounts?address=invalid").await.0,
        StatusCode::BAD_REQUEST
    );
    sqlx::query(
        "UPDATE shop.chain_cursor SET sale_history_revision=sale_history_revision+1
        WHERE network='matic' AND indexer='committee'",
    )
    .execute(pool)
    .await
    .unwrap();
    let (status, busy) = get(app.clone(), "/v1/accounts?first=10&skip=995").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{busy}");
    assert_eq!(busy["message"], "sales analytics is busy");
    let too_many = (0..51)
        .map(|i| format!("address=0x{i:040x}"))
        .collect::<Vec<_>>()
        .join("&");
    assert_eq!(
        get(app.clone(), &format!("/v1/accounts?{too_many}"))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    drop(analytics_slots);
    drop(accounts_slot);
    let earned: Vec<_> = before["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["earned"].as_str().unwrap())
        .collect();
    assert_eq!(
        earned,
        (995..1005)
            .map(|i| (1401 - i).to_string())
            .collect::<Vec<_>>()
    );

    sqlx::query(
        "INSERT INTO shop.nft_event(network,block_number,log_index,block_time,kind,emitter,contract_address,
                                    token_id,from_address,to_address)
         VALUES('matic',$1,1,100,'transfer',$2,$2,3,$3,$4),
               ('matic',$1+1,1,101,'transfer',$2,$2,3,$4,$5)",
    ).bind(height).bind(CREATOR).bind(WALLET_A).bind(WALLET_B).bind(FUTURE_HOLDER).execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO shop.sale_event(network,block_number,log_index,block_time,kind,emitter,contract_address,
                                     token_id,item_id,buyer,tx_hash)
         VALUES('matic',$1,8,100,'mint',$2,$2,1,1,$3,$4)",
    ).bind(height).bind(CREATOR).bind(MINT_RECIPIENT).bind(&hash).execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO shop.collection_event(network,block_number,log_index,block_time,kind,contract_address,account,previous_account)
         VALUES('matic',$1-2,1,98,'proxy_created',$2,$2,NULL),('matic',$1-1,1,99,'creatorship_transferred',$2,$3,$2)",
    ).bind(height).bind(CREATOR).bind(WALLET_B).execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO shop.committee_event(network,block_number,log_index,block_time,contract_address,member,is_member)
         VALUES('matic',$1-1,2,99,$2,$3,true),('matic',$1,2,100,$2,$3,false)",
    ).bind(height).bind(super::committee_index::CONTRACTS[0]).bind(FORMER_MEMBER).execute(pool).await.unwrap();
    // A trade with an unsupported token proves both parties. The payer
    // has no other evidence and paid for a different NFT recipient.
    let assets = json!({
        "sent":[{"assetType":"3","contractAddress":CREATOR,"value":"5","beneficiary":WALLET_A}],
        "received":[{"assetType":"1","contractAddress":OTHER_TOKEN,"value":"25","beneficiary":SELLER}]
    });
    let mixed = json!({
        "sent":[{"assetType":"3","contractAddress":CREATOR,"value":"6","beneficiary":WALLET_A}],
        "received":[
            {"assetType":"1","contractAddress":listings::mana_token(137).unwrap(),"value":"10","beneficiary":SELLER},
            {"assetType":"1","contractAddress":OTHER_TOKEN,"value":"15","beneficiary":MIXED_PAYEE}
        ]
    });
    for (log, payer, assets) in [(9, PAYER, assets), (11, MIXED_PAYER, mixed)] {
        sqlx::query(
            "INSERT INTO shop.trade_event(network,block_number,log_index,block_time,marketplace_address,
                   kind,caller,signer,assets,tx_hash,hashed_signature)
             VALUES('matic',$1,$2,100,$3,'traded',$4,$5,$6,$7,$7)",
        )
        .bind(height)
        .bind(log)
        .bind(venue)
        .bind(payer)
        .bind(SELLER)
        .bind(assets)
        .bind(&hash)
        .execute(pool)
        .await
        .unwrap();
    }
    sqlx::query("INSERT INTO shop.nft_contract(network,contract_address,walked_from_block) VALUES('matic',$1,$2)")
        .bind(CREATOR).bind(height).execute(pool).await.unwrap();
    super::sale_fold::write_rows(
        &mut pool.acquire().await.unwrap(),
        super::chain_walk::ChainNet::Matic,
        height - 1,
        height,
    )
    .await
    .unwrap();

    let (price, currency, payer): (Option<String>, Option<String>, String) = sqlx::query_as(
        "SELECT a.price::text,a.currency,m.payer FROM shop.sale_asset a
         JOIN shop.sale_payment m USING(network,block_number,log_index,ordinal)
         WHERE a.network='matic' AND a.block_number=$1 AND a.log_index=9",
    )
    .bind(height)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!((price, currency, payer.as_str()), (None, None, PAYER));
    let (status, list) = get(
        app.clone(),
        "/v1/accounts?network=MATIC&sortBy=most_collections",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert_eq!(list["total"], 709);
    assert_eq!(
        list["data"][0],
        json!({"id":CREATOR,"address":CREATOR,"sales":0,"purchases":0,
        "spent":"0","earned":"0","royalties":"0","collections":1})
    );
    for wallet in [
        WALLET_A,
        WALLET_B,
        MINT_RECIPIENT,
        FORMER_MEMBER,
        PAYER,
        MIXED_PAYER,
        MIXED_PAYEE,
        SELLER,
    ] {
        let (status, page) = get(
            app.clone(),
            &format!("/v1/accounts?network=MATIC&id={wallet}-matic"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{page}");
        assert_eq!(page["total"], 1);
        assert_eq!(
            page["data"][0],
            json!({"id":wallet,"address":wallet,"sales":0,"purchases":0,
            "spent":"0","earned":"0","royalties":"0","collections":0})
        );
    }

    // SetApproved records the flag. Only the legacy graph inventories accounts
    // created from the outer transaction sender of an approval or relayed call.
    sqlx::query(
        "INSERT INTO shop.collection_event(network,block_number,log_index,block_time,kind,contract_address,flag)
         VALUES('matic',$1,10,100,'set_approved',$2,true)",
    ).bind(height).bind(CREATOR).execute(pool).await.unwrap();
    graph.set(
        "accounts",
        [(CURATOR_ONLY, 1), (RELAY_ONLY, 0)].map(|(address, curations)| json!({
            "id":address,"address":address,"totalCurations":curations,"sales":0,"purchases":0,
            "spent":"0","earned":"0","royalties":"0","collections":0
        })).to_vec(),
    );
    let (_, after) = get(
        app.clone(),
        "/v1/accounts?network=MATIC&sortBy=most_collections",
    )
    .await;
    assert_eq!(after, list);
    sqlx::query("UPDATE shop.sale_publication SET generation=gen_random_uuid()")
        .execute(pool)
        .await
        .unwrap();
    let (status, ranked) = get(app.clone(), "/v1/accounts?first=10&skip=995").await;
    assert_eq!(status, StatusCode::OK, "{ranked}");
    assert_eq!(ranked["data"], before["data"]);
    for wallet in [CURATOR_ONLY, RELAY_ONLY] {
        for selector in ["id", "address"] {
            let (status, page) =
                get(app.clone(), &format!("/v1/accounts?{selector}={wallet}")).await;
            assert_eq!(status, StatusCode::OK, "{page}");
            assert_eq!(page, json!({"data":[],"total":0}));
        }
    }
    let (status, future) = get(
        app.clone(),
        &format!("/v1/accounts?address={FUTURE_HOLDER}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{future}");
    assert_eq!(future, json!({"data":[],"total":0}));
    let (status, empty) = get(app.clone(), "/v1/accounts?first=2&skip=1409").await;
    assert_eq!(status, StatusCode::OK, "{empty}");
    assert_eq!(empty, json!({"data":[],"total":1409}));
    assert!(graph.requests.lock().unwrap().is_empty());

    sqlx::query(
        "UPDATE shop.chain_cursor SET folded_at=NULL WHERE network='matic' AND indexer='committee'",
    )
    .execute(pool)
    .await
    .unwrap();
    let (status, missing) = get(app.clone(), "/v1/accounts").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{missing}");
    assert_eq!(missing["source"], "committee-index");
    let (status, healthy) = get(app, "/v1/accounts?network=ETHEREUM&first=1").await;
    assert_eq!(status, StatusCode::OK, "{healthy}");
    assert_eq!(healthy["total"], 700);
    assert_eq!(healthy["data"][0]["earned"], "1400");
    own.drop().await;
}

/// One inventory request opens one snapshot: the gate's. The copies are read
/// on it, a memoized answer costs no query after the gate, and the gate
/// still fails the request after the memo was filled.
#[tokio::test]
async fn an_inventory_read_reuses_its_gate_snapshot_and_gates_every_request() {
    use super::owned_items::{owned_copies, require_inventory, test_support};
    let up = FakeGraph::new();
    let url = serve_graph(up.clone()).await;
    let Some((state, own)) = state_over_own(&url, true).await else {
        return;
    };
    let pool = state.pool.clone();
    let c = contract_of(&tag(), 9);
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, total_supply, max_supply, available, price, image, name, category, body_shapes, search_is_collection_approved, created_at, updated_at, reviewed_at, mirrored_at) \
         VALUES ('matic', '{c}', 0, 137, 'wearable_v2', 'urn:decentraland:matic:collections-v2:{c}:0', '{WALLET_B}', 'rare', 1, 9, 1, 5, 'i', 'Hat', 'hat', ARRAY['BaseMale'], true, 1, 1, 1, now()); \
         INSERT INTO shop.nft (network, contract_address, token_id, owner, item_id, issued_id, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
         VALUES ('matic', '{c}', 1, '{WALLET_A}', 0, 1, 1, 1, 1, 0, 1)"
    )))
    .execute(&pool)
    .await
    .unwrap();
    test_support::publish(
        &pool,
        &super::chain_walk::ChainNet::ALL,
        true,
        Duration::ZERO,
    )
    .await;
    // Every other connection of the pool is held: a read that opened a
    // second snapshot would wait for one.
    let mut held = Vec::new();
    for _ in 1..pool.options().get_max_connections() {
        held.push(pool.acquire().await.unwrap());
    }
    for pass in ["read", "memoized"] {
        let Ok(gate) = require_inventory(&state, "wearable", super::internal_server_error).await
        else {
            panic!("{pass}: the published index passes the gate")
        };
        let copies = tokio::time::timeout(
            Duration::from_secs(10),
            owned_copies(&state, gate, WALLET_A, "wearable"),
        )
        .await
        .unwrap_or_else(|_| panic!("{pass}: the read waited for a second connection"))
        .unwrap();
        assert_eq!(copies.len(), 1, "{pass}");
    }
    drop(held);
    sqlx::query(
        "UPDATE shop.chain_cursor SET folded_block = NULL, folded_at = NULL WHERE indexer = 'nft'",
    )
    .execute(&pool)
    .await
    .unwrap();
    let Err(resp) = require_inventory(&state, "wearable", super::internal_server_error).await
    else {
        panic!("the memo does not answer for an unpublished index")
    };
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    own.drop().await;
}

/// An inventory memo is filed under the catalogue copy its items were named
/// from, read in the gate's snapshot. On a network the collections index
/// owns, a row rewritten without a republication is still served from the
/// warm memo, and a republication of the catalogue alone (an item renamed at
/// the same nft publication) misses it, in the owner's copies and in the
/// by-owner answer (`/lambdas/users/{a}/wearables`), whose key is the same.
#[tokio::test]
async fn a_warm_inventory_follows_a_catalogue_only_republication() {
    use super::catalogue::owning;
    use super::owned_items::{owned_copies, require_inventory, test_support};
    let up = FakeGraph::new();
    let url = serve_graph(up.clone()).await;
    let Some((state, own)) = state_over_own(&url, true).await else {
        return;
    };
    let pool = state.pool.clone();
    let c = contract_of(&tag(), 9);
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, total_supply, max_supply, available, price, image, name, category, body_shapes, search_is_collection_approved, created_at, updated_at, reviewed_at, mirrored_at) \
         VALUES ('matic', '{c}', 0, 137, 'wearable_v2', 'urn:decentraland:matic:collections-v2:{c}:0', '{WALLET_B}', 'rare', 1, 9, 1, 5, 'i', 'Hat', 'hat', ARRAY['BaseMale'], true, 1, 1, 1, now()); \
         INSERT INTO shop.nft (network, contract_address, token_id, owner, item_id, issued_id, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
         VALUES ('matic', '{c}', 1, '{WALLET_A}', 0, 1, 1, 1, 1, 0, 1)"
    )))
    .execute(&pool)
    .await
    .unwrap();
    test_support::publish(
        &pool,
        &super::chain_walk::ChainNet::ALL,
        true,
        Duration::ZERO,
    )
    .await;
    owning::own(&pool, "matic", 137, 100, 60).await;
    let names = |state: SharedState| async move {
        let Ok(gate) = require_inventory(&state, "wearable", super::internal_server_error).await
        else {
            panic!("the published index passes the gate")
        };
        let publications = gate.publications();
        let copies = owned_copies(&state, gate, WALLET_A, "wearable")
            .await
            .unwrap();
        (
            copies.iter().map(|c| c.name.clone()).collect::<Vec<_>>(),
            publications,
        )
    };
    let by_owner = |state: SharedState| async move {
        let (status, body) = get(
            super::router(state),
            &format!("/lambdas/users/{WALLET_A}/wearables"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body["elements"][0]["name"].as_str().map(str::to_string)
    };
    let (before, first) = names(state.clone()).await;
    assert_eq!(before, ["Hat"]);
    assert_eq!(by_owner(state.clone()).await.as_deref(), Some("Hat"));

    // Rewritten without a republication: the warm memos answer.
    sqlx::query("UPDATE shop.items SET name = 'Cap' WHERE contract_address = $1")
        .bind(&c)
        .execute(&pool)
        .await
        .unwrap();
    let (warm, again) = names(state.clone()).await;
    assert_eq!(
        (warm, again),
        (vec!["Hat".to_string()], first),
        "a memo hit"
    );
    assert_eq!(by_owner(state.clone()).await.as_deref(), Some("Hat"));

    // The catalogue alone republishes: both miss.
    owning::own(&pool, "matic", 137, 100, 60).await;
    let (after, second) = names(state.clone()).await;
    assert_eq!(
        (second.ethereum, second.matic),
        (first.ethereum, first.matic)
    );
    assert_ne!(second, first, "the catalogue copy is in the key");
    assert_eq!(after, ["Cap"]);
    assert_eq!(by_owner(state.clone()).await.as_deref(), Some("Cap"));
    own.drop().await;
}

/// Both order legs require their published sources on a walking node.
/// The graph-backed legacy reader still requires published proof for v3.
#[tokio::test]
async fn an_open_listing_stays_only_while_its_signer_holds_that_token() {
    let up = FakeGraph::new();
    let url = serve_graph(up.clone()).await;
    let Some((state, own)) = state_over_own(&url, true).await else {
        return;
    };
    let pool = state.pool.clone();
    super::trade_events::tests::publish_cursors(&pool).await;
    let t = tag();
    let c = contract_of(&t, 7);
    let unindexed = contract_of(&t, 8);
    let token = |issued: u64| {
        (alloy_primitives::U256::from(3u64) << 216u32 | alloy_primitives::U256::from(issued))
            .to_string()
    };
    let list = |contract: String, token: String| {
        let pool = pool.clone();
        async move {
            insert_trade(
                &pool,
                "public_nft_order",
                WALLET_A,
                Asset {
                    asset_type: 3,
                    contract,
                    value: token,
                },
                Asset {
                    asset_type: 1,
                    contract: "0x2222222222222222222222222222222222222222".into(),
                    value: "1000".into(),
                },
            )
            .await
            .0
        }
    };
    let held = list(c.clone(), token(1)).await;
    let sold = list(c.clone(), token(2)).await;
    let burned = list(c.clone(), token(4)).await;
    let elsewhere = list(unindexed.clone(), "5".into()).await;
    let land = insert_trade_on(
        &pool,
        "ETHEREUM",
        1,
        "public_nft_order",
        WALLET_A,
        Asset {
            asset_type: 3,
            contract: crate::land_authz::LAND_REGISTRY_MAINNET.into(),
            value: "77".into(),
        },
        Asset {
            asset_type: 1,
            contract: "0x2222222222222222222222222222222222222222".into(),
            value: "1000".into(),
        },
    )
    .await
    .0;
    for (issued, owner) in [
        (1, Some(WALLET_A)),
        (2, Some(WALLET_B)),
        (3, Some(WALLET_A)),
        (4, None),
    ] {
        sqlx::query(
            "INSERT INTO shop.nft (network, contract_address, token_id, owner, item_id, issued_id, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
             VALUES ('matic', $1, $2::numeric, $3, 3, $4, 1, 1, 1, 0, 1)",
        )
        .bind(&c)
        .bind(token(issued))
        .bind(owner)
        .bind(issued as i64)
        .execute(&pool)
        .await
        .unwrap();
    }
    let open = super::orders::OrderFilters {
        status: Some("open".into()),
        owner: Some(WALLET_A.into()),
        ..Default::default()
    };
    let ids_of = |rows: Vec<super::orders::Order>| {
        let mut ids: Vec<String> = rows.into_iter().map(|o| o.id).collect();
        ids.sort();
        ids
    };
    let unavailable = |r: Result<_, axum::response::Response>| r.err().map(|r| r.status());
    assert_eq!(
        unavailable(super::orders::list_orders(&state, &open).await),
        Some(StatusCode::SERVICE_UNAVAILABLE),
        "an unpublished book answers the envelope"
    );

    crate::test_db::publish_land_owner(
        &pool,
        crate::land_authz::LAND_REGISTRY_MAINNET,
        "77",
        WALLET_A,
    )
    .await;
    let not_walking = std::sync::Arc::new(super::MarketplaceState::new(
        pool.clone(),
        crate::http::Outbound::for_tests(),
        market_settings(&url, &url, true),
    ));
    not_walking
        .test_allow_loopback
        .store(true, Ordering::Relaxed);
    assert!(!super::owned_items::walks_here(&not_walking));
    let (rows, _) = super::orders::list_orders(&not_walking, &open)
        .await
        .unwrap();
    assert_eq!(ids_of(rows), vec![land.clone()]);

    super::owned_items::test_support::publish(
        &pool,
        &[super::chain_walk::ChainNet::Matic],
        false,
        Duration::ZERO,
    )
    .await;
    assert_eq!(
        unavailable(super::orders::list_orders(&state, &open).await),
        Some(StatusCode::SERVICE_UNAVAILABLE),
        "a book read of both networks waits for Ethereum"
    );
    let matic = super::orders::OrderFilters {
        network: Some(super::wire::Network::Matic),
        ..open.clone()
    };
    let (rows, _) = super::orders::list_orders(&state, &matic).await.unwrap();
    assert_eq!(ids_of(rows), vec![held.clone()]);
    super::owned_items::test_support::publish(
        &pool,
        &[super::chain_walk::ChainNet::Ethereum],
        false,
        Duration::ZERO,
    )
    .await;
    let mut want = vec![held.clone(), land.clone()];
    want.sort();
    for reader in [&state, &not_walking] {
        let (rows, _) = super::orders::list_orders(reader, &open).await.unwrap();
        assert_eq!(
            ids_of(rows),
            want,
            "exclude {sold}, {burned}, and {elsewhere}"
        );
    }
    sqlx::query(
        "UPDATE shop.chain_cursor SET chain_id=80002 WHERE indexer='nft' AND network='matic'",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        unavailable(super::orders::list_orders(&state, &open).await),
        Some(StatusCode::SERVICE_UNAVAILABLE),
        "another chain's publication cannot serve the local book"
    );
    let (rows, _) = super::orders::list_orders(&not_walking, &open)
        .await
        .unwrap();
    assert_eq!(
        ids_of(rows),
        vec![land],
        "other-chain ownership cannot back the v3 listing"
    );
    own.drop().await;
}

/// Both the creator and minter evidence must come from the order's chain.
#[tokio::test]
async fn an_item_listing_requires_creator_and_minter_rows_on_its_chain() {
    let up = FakeGraph::new();
    let Some((pool, state, contract)) = owned_polygon_market(&up).await else {
        return;
    };
    let (id, _) = insert_trade(
        &pool,
        "public_item_order",
        CREATOR,
        Asset {
            asset_type: 4,
            contract: contract.clone(),
            value: "0".into(),
        },
        Asset {
            asset_type: 1,
            contract: listings::mana_token(137).unwrap().into(),
            value: "1000000000000000000".into(),
        },
    )
    .await;
    let listed = async |expected: bool| {
        let uri = format!("/v3/catalog/importable?seller={CREATOR}");
        let (status, body) = get(market_router(state.clone()), &uri).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(
            body["data"][0]["oldTradeId"].as_str(),
            expected.then_some(id.as_str()),
            "{body}"
        );
    };
    listed(true).await;
    for (set, restore) in [
        (
            "UPDATE shop.items SET chain_id = 80002, testnet_verified = true WHERE contract_address = $1",
            "UPDATE shop.items SET chain_id = 137, testnet_verified = false WHERE contract_address = $1",
        ),
        (
            "UPDATE shop.collections SET chain_id = 80002, testnet_verified = true WHERE contract_address = $1",
            "UPDATE shop.collections SET chain_id = 137, testnet_verified = false WHERE contract_address = $1",
        ),
    ] {
        sqlx::query(sqlx::AssertSqlSafe(set))
            .bind(&contract)
            .execute(&pool)
            .await
            .unwrap();
        listed(false).await;
        sqlx::query(sqlx::AssertSqlSafe(restore))
            .bind(&contract)
            .execute(&pool)
            .await
            .unwrap();
        listed(true).await;
    }
    pool.close().await;
}

/// Repeat the exact same reads while their trade and ownership publications
/// change independently of the catalogue. Every leg uses the new proof.
#[tokio::test]
async fn listing_routes_stop_serving_a_trade_as_soon_as_its_proof_is_withdrawn() {
    let up = FakeGraph::new();
    let Some((pool, fixture, contract)) = owned_polygon_market(&up).await else {
        return;
    };
    let mut settings = fixture.settings.clone();
    settings.market.catalog_cache_ttl = Duration::from_secs(60);
    let state = Arc::new(super::MarketplaceState::new(
        pool.clone(),
        crate::http::Outbound::for_tests(),
        settings,
    ));
    state.test_allow_loopback.store(true, Ordering::Relaxed);
    let token = "1"; // item 0, issued 1
    let (id, hashed_signature) = insert_trade(
        &pool,
        "public_nft_order",
        WALLET_A,
        Asset {
            asset_type: 3,
            contract: contract.clone(),
            value: token.into(),
        },
        Asset {
            asset_type: 1,
            contract: "0x2222222222222222222222222222222222222222".into(),
            value: "3000000000000000000".into(),
        },
    )
    .await;
    super::owned_items::test_support::publish(
        &pool,
        &[super::chain_walk::ChainNet::Matic],
        false,
        Duration::ZERO,
    )
    .await;
    sqlx::query(
        "INSERT INTO shop.nft (network, contract_address, token_id, owner, item_id, issued_id, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
         VALUES ('matic', $1, 1, $2, 0, 1, 10, 1, 10, 0, 1)",
    )
    .bind(&contract)
    .bind(WALLET_A)
    .execute(&pool)
    .await
    .unwrap();

    let routes = [
        format!(
            "/v3/catalog/unified?contractAddress={contract}&includeLegacySecondary=true&listingType=secondary"
        ),
        "/v3/catalog/unified?includeLegacySecondary=true&listingType=secondary".into(),
        format!("/v3/catalog/importable?seller={WALLET_A}"),
        format!("/v1/orders?contractAddress={contract}"),
        format!("/v1/orders/open-by-items?item={contract}-0"),
    ];
    let assert_listed = async |expected: bool| {
        for route in &routes {
            let (status, body) = get(market_router(state.clone()), route).await;
            assert_eq!(status, StatusCode::OK, "{route}: {body}");
            let data = body["data"].as_array().expect("listing data");
            assert_eq!(data.len(), usize::from(expected), "{route}: {body}");
            if let Some(row) = data.first() {
                let listing_id = row.get("oldTradeId").unwrap_or(&row["tradeId"]);
                assert_eq!(listing_id, &id, "{route}: {body}");
            }
            if let Some(total) = body.get("total") {
                assert_eq!(
                    total.as_u64().or_else(|| total.as_str()?.parse().ok()),
                    Some(u64::from(expected)),
                    "{route}: {body}"
                );
            }
        }
        let (status, catalogue) = get(
            market_router(state.clone()),
            &format!("/v2/catalog?contractAddress={contract}"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{catalogue}");
        assert_eq!(
            catalogue["data"][0]["listings"],
            usize::from(expected),
            "{catalogue}"
        );
        let (status, nft) = get(
            market_router(state.clone()),
            &format!(
                "/v1/nfts?category=wearable&network=MATIC&contractAddress={contract}&tokenId=1"
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{nft}");
        assert_eq!(nft["data"].as_array().map(Vec::len), Some(1), "{nft}");
        assert_eq!(
            nft["data"][0]["order"]["tradeId"].as_str(),
            expected.then_some(id.as_str()),
            "{nft}"
        );
        let (status, prices) = get(
            market_router(state.clone()),
            &format!("/v1/prices?assetType=nft&category=wearable&network=MATIC&contractAddress={contract}"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{prices}");
        assert_eq!(
            prices["data"]["3000000000000000000"].as_u64(),
            expected.then_some(1),
            "{prices}"
        );
    };
    assert_listed(true).await;

    let venue = async |address: &str| {
        sqlx::query("UPDATE shop.trades SET contract = $1 WHERE id = $2::uuid")
            .bind(address)
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
    };
    venue("0x9999999999999999999999999999999999999999").await;
    assert_listed(false).await;
    venue(super::trade_events::marketplaces(super::chain_walk::ChainNet::Matic)[0].0).await;
    sqlx::query("UPDATE shop.trade_chain_state SET digest = NULL WHERE trade_id = $1::uuid")
        .bind(&id)
        .execute(&pool)
        .await
        .unwrap();
    assert_listed(true).await;
    venue(listings::offchain_marketplace_v3(137).unwrap().address).await;
    crate::test_db::open_trade_verdict(&pool, &id).await;
    sqlx::query("UPDATE shop.chain_cursor SET last_block = last_block + 10 WHERE network = 'matic' AND indexer IN ('nft', 'trade_events')")
        .execute(&pool).await.unwrap();
    assert_listed(true).await;

    sqlx::query("INSERT INTO shop.nft_contract(network, contract_address, walked_from_block, backfill_from_block) VALUES ('matic', $1, 50, 1)")
        .bind(&contract).execute(&pool).await.unwrap();
    assert_listed(false).await;
    sqlx::query("UPDATE shop.nft_contract SET backfilled_to_block = 49, refold_owed = true WHERE network='matic' AND contract_address=$1")
        .bind(&contract).execute(&pool).await.unwrap();
    assert_listed(false).await;
    sqlx::query("UPDATE shop.nft_contract SET refold_owed = false WHERE network='matic' AND contract_address=$1")
        .bind(&contract).execute(&pool).await.unwrap();
    assert_listed(true).await;

    sqlx::query(
        "INSERT INTO shop.nft_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, from_address, to_address) \
         VALUES ('matic', 101, 0, 2, 'transfer', $1, $1, 1, $2, $3)",
    )
    .bind(&contract).bind(WALLET_A).bind(WALLET_B)
    .execute(&pool).await.unwrap();
    assert_listed(false).await;
    sqlx::query("DELETE FROM shop.nft_event WHERE network = 'matic' AND contract_address = $1")
        .bind(&contract)
        .execute(&pool)
        .await
        .unwrap();
    assert_listed(true).await;

    sqlx::query(
        "INSERT INTO shop.trade_event (network, block_number, log_index, block_time, kind, marketplace_address, caller, hashed_signature, tx_hash) \
         VALUES ('matic', 11, 0, 2, 'signature_cancelled', $1, $2, $3, 'tx')",
    )
    .bind(listings::offchain_marketplace_v3(137).unwrap().address)
    .bind(WALLET_A)
    .bind("another-trade")
    .execute(&pool).await.unwrap();
    assert_listed(true).await;

    sqlx::query("UPDATE shop.trade_event SET hashed_signature = $1 WHERE network = 'matic' AND block_number = 11")
        .bind(hashed_signature).execute(&pool).await.unwrap();
    assert_listed(false).await;

    // Signer index changes only affect that signer. Contract index changes
    // affect every order at the same venue.
    sqlx::query("UPDATE shop.trade_event SET kind = 'signer_index_increased', hashed_signature = NULL, new_index = 1, caller = $1 WHERE network = 'matic' AND block_number = 11")
        .bind(WALLET_B).execute(&pool).await.unwrap();
    assert_listed(true).await;
    sqlx::query(
        "UPDATE shop.trade_event SET caller = $1 WHERE network = 'matic' AND block_number = 11",
    )
    .bind(WALLET_A)
    .execute(&pool)
    .await
    .unwrap();
    assert_listed(false).await;
    sqlx::query("UPDATE shop.trade_event SET kind = 'contract_index_increased', caller = $1 WHERE network = 'matic' AND block_number = 11")
        .bind(WALLET_B).execute(&pool).await.unwrap();
    assert_listed(false).await;

    for closed in ["cancelled", "sold"] {
        sqlx::query("UPDATE shop.trade_chain_state SET status = $1, folded_block = 11 WHERE trade_id = $2::uuid")
            .bind(closed).bind(&id).execute(&pool).await.unwrap();
        sqlx::query("UPDATE shop.chain_cursor SET folded_block = 11 WHERE network = 'matic' AND indexer = 'trade_events'")
            .execute(&pool).await.unwrap();
        assert_listed(false).await;
    }
    sqlx::query("UPDATE shop.trade_chain_state SET status = 'open', digest = NULL WHERE trade_id = $1::uuid")
        .bind(&id).execute(&pool).await.unwrap();
    assert_listed(false).await; // the fold saw the trade before its digest
    sqlx::query("DELETE FROM shop.trade_chain_state WHERE trade_id = $1::uuid")
        .bind(&id)
        .execute(&pool)
        .await
        .unwrap();
    assert_listed(false).await;
    pool.close().await;
}

/// Both orders routes keep a transferred token out during a rewind and
/// retest the surviving LAND order against its independent publication.
#[tokio::test]
async fn a_sold_listing_stays_out_through_a_rewind_and_its_refill() {
    let up = FakeGraph::new();
    let url = serve_graph(up.clone()).await;
    let Some((state, own)) = state_over_own(&url, true).await else {
        return;
    };
    let pool = state.pool.clone();
    super::trade_events::tests::publish_cursors(&pool).await;
    let c = contract_of(&tag(), 9);
    let token = (alloy_primitives::U256::from(3u64) << 216u32 | alloy_primitives::U256::from(1u64))
        .to_string();
    let price = || Asset {
        asset_type: 1,
        contract: "0x2222222222222222222222222222222222222222".into(),
        value: "1000".into(),
    };
    let sold = insert_trade(
        &pool,
        "public_nft_order",
        WALLET_A,
        Asset {
            asset_type: 3,
            contract: c.clone(),
            value: token.clone(),
        },
        price(),
    )
    .await
    .0;
    let land = insert_trade_on(
        &pool,
        "ETHEREUM",
        1,
        "public_nft_order",
        WALLET_A,
        Asset {
            asset_type: 3,
            contract: crate::land_authz::LAND_REGISTRY_MAINNET.into(),
            value: "77".into(),
        },
        price(),
    )
    .await
    .0;
    crate::test_db::publish_land_owner(
        &pool,
        crate::land_authz::LAND_REGISTRY_MAINNET,
        "77",
        WALLET_A,
    )
    .await;
    // The fold: the token belongs to WALLET_B now.
    let fold = || {
        let (pool, c, token) = (pool.clone(), c.clone(), token.clone());
        async move {
            sqlx::query(
                "INSERT INTO shop.nft (network, contract_address, token_id, owner, item_id, issued_id, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
                 VALUES ('matic', $1, $2::numeric, $3, 3, 1, 1, 1, 2, 0, 2)",
            )
            .bind(&c)
            .bind(&token)
            .bind(WALLET_B)
            .execute(&pool)
            .await
            .unwrap();
            super::owned_items::test_support::publish(
                &pool,
                &super::chain_walk::ChainNet::ALL,
                false,
                Duration::ZERO,
            )
            .await;
        }
    };
    let open = super::orders::OrderFilters {
        status: Some("open".into()),
        owner: Some(WALLET_A.into()),
        ..Default::default()
    };
    let items = vec![
        (c.clone(), "3".to_string()),
        (
            crate::land_authz::LAND_REGISTRY_MAINNET.to_string(),
            "0".to_string(),
        ),
    ];
    let withdrawn = || async {
        (
            super::orders::list_orders(&state, &open)
                .await
                .err()
                .map(|r| r.status()),
            super::orders::open_orders_by_items(&state, &items, 10)
                .await
                .err()
                .map(|r| r.status()),
        )
    };
    let routes = || async {
        let (orders, _) = super::orders::list_orders(&state, &open)
            .await
            .unwrap_or_else(|_| panic!("/v1/orders answers"));
        let by_items = super::orders::open_orders_by_items(&state, &items, 10)
            .await
            .unwrap_or_else(|_| panic!("/v1/orders/open-by-items answers"));
        let ids = |orders: Vec<super::orders::Order>| -> Vec<String> {
            let mut ids: Vec<String> = orders.into_iter().map(|o| o.id).collect();
            ids.sort();
            ids
        };
        (ids(orders), ids(by_items))
    };
    fold().await;
    let want = (vec![land.clone()], vec![land.clone()]);
    assert_eq!(routes().await, want, "published: {sold} drops out");
    // A rewind deletes the token's row and withdraws the publication.
    sqlx::query("DELETE FROM shop.nft WHERE network = 'matic' AND contract_address = $1")
        .bind(&c)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE shop.chain_cursor SET folded_block = NULL, folded_at = NULL WHERE indexer = 'nft'",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        withdrawn().await,
        (
            Some(StatusCode::SERVICE_UNAVAILABLE),
            Some(StatusCode::SERVICE_UNAVAILABLE)
        ),
        "rewound: the routes answer the envelope, never {sold} unguarded"
    );
    fold().await;
    assert_eq!(routes().await, want, "refilled: {sold} still drops out");
    own.drop().await;
}

/// The legacy order book of a walking node is `shop.legacy_order` on one
/// snapshot: only the orders of the contracts it serves (a collection, the
/// LAND and Estate registries, names), and an open order `open` only while
/// its owner source proves the seller holds the token (`shop.nft`, or
/// `land_authz.token_right` for LAND and estates): another owner, a burned
/// or dissolved token and a token with no row there all read
/// `transferred`, in the page, the status filters and the totals alike;
/// the unavailable envelope naming the
/// nft index while it is not published on a network the read names, and
/// the land-authz index's while it is not ready for a read that could
/// answer an open LAND order.
#[tokio::test]
async fn the_legacy_order_book_is_the_index_with_the_subgraph_statuses() {
    let up = FakeGraph::new();
    let url = serve_graph(up.clone()).await;
    let Some((state, own)) = state_over_own(&url, true).await else {
        return;
    };
    super::trade_events::tests::publish_cursors(&state.pool).await;
    let pool = state.pool.clone();
    let t = tag();
    let (c, stray) = (contract_of(&t, 1), contract_of(&t, 2));
    let land = crate::land_authz::LAND_REGISTRY_MAINNET.to_lowercase();
    let estate = crate::land_authz::ESTATE_REGISTRY_MAINNET.to_lowercase();
    let (m_eth, m_matic) = (
        super::nft_index::deployments::Deployments::mainnet(super::chain_walk::ChainNet::Ethereum)
            .markets[0]
            .0,
        super::nft_index::deployments::Deployments::mainnet(super::chain_walk::ChainNet::Matic)
            .markets[1]
            .0,
    );
    let token = |item: u64, issued: u64| {
        (alloy_primitives::U256::from(item) << 216u32 | alloy_primitives::U256::from(issued))
            .to_string()
    };
    sqlx::query(
        "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, is_completed, is_approved, is_editable, created_at, updated_at, reviewed_at, mirrored_at) \
         SELECT 'matic', $1, 137, 'Pirates', $2, $2, 'urn:decentraland:matic:collections-v2:' || $1, true, true, false, 1, 1, 1, timezone('UTC', now())",
    )
    .bind(&c)
    .bind(CREATOR)
    .execute(&pool)
    .await
    .unwrap();
    let far = (chrono::Utc::now().timestamp() + 86_400) * 1000;
    let order = |network: &'static str,
                 market: &'static str,
                 id: &'static str,
                 contract: String,
                 token: String,
                 seller: &'static str,
                 price: i64,
                 created: i64| {
        let pool = pool.clone();
        async move {
            sqlx::query(
                "INSERT INTO shop.legacy_order (network, marketplace_address, order_id, contract_address, token_id, seller, price, expires_at, status, \
                     created_block, created_log, created_at, updated_block, updated_log, updated_at) \
                 VALUES ($1, $2, $3, $4, $5::numeric, $6, $7, $8, 'open', $9, 0, $9, $9, 0, $9)",
            )
            .bind(network)
            .bind(market)
            .bind(id)
            .bind(contract)
            .bind(token)
            .bind(seller)
            .bind(price)
            .bind(far)
            .bind(created)
            .execute(&pool)
            .await
            .unwrap();
        }
    };
    // held: the seller still holds it; moved: the index holds it elsewhere;
    // burned: the index holds it with no owner; unheld: the index has no row
    // for it; parcel / parcel_moved / parcel_unindexed / estate_dissolved:
    // LAND and an estate, checked on the land index (the seller's, another
    // owner's, no row, no owner); stray: a contract the node does not serve.
    order(
        "matic",
        m_matic,
        "held",
        c.clone(),
        token(3, 1),
        WALLET_A,
        30,
        1_000,
    )
    .await;
    order(
        "matic",
        m_matic,
        "moved",
        c.clone(),
        token(3, 2),
        WALLET_A,
        20,
        1_001,
    )
    .await;
    order(
        "ethereum",
        m_eth,
        "parcel",
        land.clone(),
        "77".into(),
        WALLET_A,
        10,
        1_002,
    )
    .await;
    order(
        "ethereum",
        m_eth,
        "parcel_moved",
        land.clone(),
        "78".into(),
        WALLET_A,
        40,
        1_003,
    )
    .await;
    order(
        "matic",
        m_matic,
        "stray",
        stray.clone(),
        "5".into(),
        WALLET_A,
        5,
        1_004,
    )
    .await;
    order(
        "matic",
        m_matic,
        "burned",
        c.clone(),
        token(3, 4),
        WALLET_A,
        50,
        1_005,
    )
    .await;
    order(
        "matic",
        m_matic,
        "unheld",
        c.clone(),
        token(3, 5),
        WALLET_A,
        60,
        1_006,
    )
    .await;
    order(
        "ethereum",
        m_eth,
        "parcel_unindexed",
        land.clone(),
        "79".into(),
        WALLET_A,
        70,
        1_007,
    )
    .await;
    order(
        "ethereum",
        m_eth,
        "estate_dissolved",
        estate.clone(),
        "500".into(),
        WALLET_A,
        80,
        1_008,
    )
    .await;
    for (issued, owner) in [(1, Some(WALLET_A)), (2, Some(WALLET_B)), (4, None)] {
        sqlx::query(
            "INSERT INTO shop.nft (network, contract_address, token_id, owner, item_id, issued_id, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
             VALUES ('matic', $1, $2::numeric, $3, 3, $4, 1, 1, 1, 0, 1)",
        )
        .bind(&c)
        .bind(token(3, issued))
        .bind(owner)
        .bind(issued as i64)
        .execute(&pool)
        .await
        .unwrap();
    }
    let app = || market_router(state.clone());
    let ids = |body: &Value| -> Vec<(String, String)> {
        body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| {
                (
                    o["id"].as_str().unwrap().to_string(),
                    o["status"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    };
    // Not published: the envelope, never a partial page.
    let (status, body) = get(app(), "/v1/orders?network=MATIC").await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("nft-index")),
        "{body}"
    );
    super::owned_items::test_support::publish(
        &pool,
        &super::chain_walk::ChainNet::ALL,
        false,
        Duration::ZERO,
    )
    .await;
    // A book that trails the published fold (an earlier binary folded the
    // nft index past it) is not served beside that ownership.
    sqlx::query("UPDATE shop.legacy_order_cursor SET folded_through = 50 WHERE network = 'matic'")
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = get(app(), "/v1/orders?network=MATIC").await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("nft-index")),
        "{body}"
    );
    sqlx::query("UPDATE shop.legacy_order_cursor SET folded_through = 100 WHERE network = 'matic'")
        .execute(&pool)
        .await
        .unwrap();
    // Nor one with a refold queued on its network: an earlier binary
    // repaired a sale the book has folded and republished the same height
    // without folding it. Another network's queue does not withhold it.
    let queue = |network: &'static str| {
        let (pool, c) = (pool.clone(), c.clone());
        async move {
            sqlx::query(
                "INSERT INTO shop.legacy_order_refold (network, contract_address, token_id) VALUES ($1, $2, 1)",
            )
            .bind(network)
            .bind(c)
            .execute(&pool)
            .await
            .unwrap();
        }
    };
    queue("ethereum").await;
    let (status, body) = get(app(), "/v1/orders?network=MATIC").await;
    assert_eq!(status, StatusCode::OK, "another network's queue: {body}");
    queue("matic").await;
    let (status, body) = get(app(), "/v1/orders?network=MATIC").await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("nft-index")),
        "{body}"
    );
    sqlx::query("DELETE FROM shop.legacy_order_refold")
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = get(app(), "/v1/orders?network=MATIC").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // Nor one folded on a branch the walk no longer has: an earlier binary
    // republished block 100 with another hash.
    let republish = |hash: &'static str| {
        let pool = pool.clone();
        async move {
            sqlx::query(
                "UPDATE shop.chain_checkpoint SET block_hash = $1 WHERE indexer = 'nft' AND network = 'matic' AND block_number = 100",
            )
            .bind(hash)
            .execute(&pool)
            .await
            .unwrap();
        }
    };
    republish("0x1111111111111111111111111111111111111111111111111111111111111111").await;
    let (status, body) = get(app(), "/v1/orders?network=MATIC").await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("nft-index")),
        "{body}"
    );
    republish("0x0000000000000000000000000000000000000000000000000000000000000000").await;
    // Polygon alone needs no LAND index.
    let (status, body) = get(app(), "/v1/orders?network=MATIC&sortBy=cheapest").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        ids(&body),
        vec![
            ("moved".to_string(), "transferred".to_string()),
            ("held".to_string(), "open".to_string()),
            ("burned".to_string(), "transferred".to_string()),
            ("unheld".to_string(), "transferred".to_string()),
        ],
        "the stray contract is not served; a burned or unindexed token is not proof of ownership: {body}"
    );
    assert_eq!(body["total"], "4", "{body}");
    assert_eq!(
        (
            body["data"][1]["issuedId"].as_str(),
            body["data"][1]["owner"].as_str(),
            body["data"][1]["tradeId"].as_str(),
            body["data"][1]["chainId"].as_i64()
        ),
        (Some("1"), Some(WALLET_A), Some(""), Some(137))
    );
    // An open LAND order could be answered: the LAND index must be ready.
    let (status, body) = get(app(), "/v1/orders?status=open").await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("land-authz-index")),
        "{body}"
    );
    let (status, body) = get(app(), "/v1/orders?status=sold").await;
    assert_eq!(
        (status, body["total"].as_str()),
        (StatusCode::OK, Some("0")),
        "a sold order needs no LAND owner: {body}"
    );
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "INSERT INTO land_authz.index_cursor (network, land_registry, estate_registry, last_block, folded_block, folded_at, full_replay_started) \
         VALUES ('ethereum-mainnet', '{land}', '{estate}', 1, 1, timezone('UTC', now()), true); \
         INSERT INTO land_authz.token_right (token_address, token_id, x, y, owner, updated_block, updated_log) VALUES \
            ('{land}', 77, 0, 77, '{WALLET_A}', 1, 0), ('{land}', 78, 0, 78, '{WALLET_B}', 1, 1), \
            ('{estate}', 500, NULL, NULL, NULL, 1, 2)",
    )))
    .execute(&pool)
    .await
    .unwrap();
    let (status, body) = get(app(), "/v1/orders?sortBy=oldest").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        ids(&body),
        vec![
            ("held".to_string(), "open".to_string()),
            ("moved".to_string(), "transferred".to_string()),
            ("parcel".to_string(), "open".to_string()),
            ("parcel_moved".to_string(), "transferred".to_string()),
            ("burned".to_string(), "transferred".to_string()),
            ("unheld".to_string(), "transferred".to_string()),
            ("parcel_unindexed".to_string(), "transferred".to_string()),
            ("estate_dissolved".to_string(), "transferred".to_string()),
        ],
        "{body}"
    );
    assert_eq!(
        (
            body["data"][2]["network"].as_str(),
            body["data"][2]["issuedId"].clone()
        ),
        (Some("ETHEREUM"), Value::Null)
    );
    let (_, body) = get(app(), "/v1/orders?status=open").await;
    assert_eq!(
        ids(&body),
        vec![
            ("parcel".to_string(), "open".to_string()),
            ("held".to_string(), "open".to_string())
        ],
        "open, newest first, leaves out a listing whose seller lost the token: {body}"
    );
    assert_eq!(
        body["total"], "2",
        "the total counts the page's status: {body}"
    );
    let (_, body) = get(app(), "/v1/orders?status=transferred&sortBy=oldest").await;
    assert_eq!(
        ids(&body),
        vec![
            ("moved".to_string(), "transferred".to_string()),
            ("parcel_moved".to_string(), "transferred".to_string()),
            ("burned".to_string(), "transferred".to_string()),
            ("unheld".to_string(), "transferred".to_string()),
            ("parcel_unindexed".to_string(), "transferred".to_string()),
            ("estate_dissolved".to_string(), "transferred".to_string()),
        ],
        "{body}"
    );
    assert_eq!(body["total"], "6", "{body}");
    let (_, body) = get(app(), "/v1/orders?status=transferred&network=ETHEREUM").await;
    assert_eq!(
        ids(&body),
        vec![
            ("estate_dissolved".to_string(), "transferred".to_string()),
            ("parcel_unindexed".to_string(), "transferred".to_string()),
            ("parcel_moved".to_string(), "transferred".to_string()),
        ],
        "{body}"
    );
    let (_, body) = get(
        app(),
        &format!("/v1/orders?contractAddress={c}&itemId=3&status=open"),
    )
    .await;
    assert_eq!(
        ids(&body),
        vec![("held".to_string(), "open".to_string())],
        "{body}"
    );
    let (status, body) = get(app(), &format!("/v1/orders/open-by-items?item={c}-3")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        ids(&body),
        vec![("held".to_string(), "open".to_string())],
        "{body}"
    );
    own.drop().await;
}

/// The auditor's rollout case: an earlier binary's nft fold, which knows
/// nothing of the book's marker, returns a token to its order's seller at
/// 101 while the book and the published fold both still say 100. The order
/// must not read open (in the page or its total) before that fold is
/// published; a raw walk advance alone still serves the book at 100; once
/// published, the order is open.
#[tokio::test]
async fn an_earlier_writers_unpublished_transfer_never_reopens_an_order() {
    let up = FakeGraph::new();
    let url = serve_graph(up.clone()).await;
    let Some((state, own)) = state_over_own(&url, true).await else {
        return;
    };
    super::trade_events::tests::publish_cursors(&state.pool).await;
    let pool = state.pool.clone();
    let c = contract_of(&tag(), 1);
    let token = (alloy_primitives::U256::from(3u64) << 216u32 | alloy_primitives::U256::from(1u64))
        .to_string();
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, is_completed, is_approved, is_editable, created_at, updated_at, reviewed_at, mirrored_at)
         VALUES ('matic', '{c}', 137, 'Pirates', '{CREATOR}', '{CREATOR}', 'urn:decentraland:matic:collections-v2:{c}', true, true, false, 1, 1, 1, timezone('UTC', now()));
         INSERT INTO shop.nft (network, contract_address, token_id, owner, item_id, issued_id, minted_block, minted_at, transferred_block, transferred_log, transferred_at)
         VALUES ('matic', '{c}', {token}, '{WALLET_B}', 3, 1, 1, 1, 50, 0, 50);
         INSERT INTO shop.legacy_order (network, marketplace_address, order_id, contract_address, token_id, seller, price, expires_at, status,
             created_block, created_log, created_at, updated_block, updated_log, updated_at)
         VALUES ('matic', '0xmarket', 'returned', '{c}', {token}, '{WALLET_A}', 10, {far}, 'open', 60, 0, 60, 60, 0, 60)",
        far = (chrono::Utc::now().timestamp() + 86_400) * 1000,
    )))
    .execute(&pool)
    .await
    .unwrap();
    // The fixtures are the published fold at 100.
    super::owned_items::test_support::publish(
        &state.pool,
        &super::chain_walk::ChainNet::ALL,
        false,
        std::time::Duration::ZERO,
    )
    .await;
    let app = || market_router(state.clone());
    let read = |status: &'static str| {
        let router = app();
        async move {
            let (code, body) =
                get(router, &format!("/v1/orders?network=MATIC&status={status}")).await;
            (
                code,
                body["total"].as_str().map(str::to_string),
                body["source"].as_str().map(str::to_string),
            )
        }
    };
    let served = |total: &str| (StatusCode::OK, Some(total.to_string()), None);
    let refused = (
        StatusCode::SERVICE_UNAVAILABLE,
        None,
        Some("nft-index".to_string()),
    );
    assert_eq!(
        read("open").await,
        served("0"),
        "the token left its seller at 50"
    );
    assert_eq!(read("transferred").await, served("1"));
    // The walk advanced to 120 without folding: the book at 100 is served.
    sqlx::query(
        "UPDATE shop.chain_cursor SET last_block = 120 WHERE indexer = 'nft' AND network = 'matic'",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(read("open").await, served("0"));
    // The earlier binary's fold through 120 returns the token to its seller
    // at 101; nothing of it is published, and the book's marker is untouched.
    sqlx::query("UPDATE shop.nft SET owner = $1, transferred_block = 101 WHERE network = 'matic' AND contract_address = $2")
        .bind(WALLET_A)
        .bind(&c)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        read("open").await,
        refused,
        "the reopened order is not exposed"
    );
    assert_eq!(read("transferred").await, refused);
    // Published at 120 (with the book folded there): the order is open.
    sqlx::query(
        "INSERT INTO shop.chain_checkpoint (indexer, network, block_number, block_hash, block_time) \
         VALUES ('nft', 'matic', 120, '0x' || repeat('0', 64), 0) ON CONFLICT DO NOTHING",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE shop.legacy_order_cursor SET folded_through = 120 WHERE network = 'matic'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE shop.chain_cursor SET fold_through = 120, folded_block = 120, folded_at = timezone('UTC', now()) \
         WHERE indexer = 'nft' AND network = 'matic'",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        read("open").await,
        served("1"),
        "published: the order is open"
    );
    assert_eq!(read("transferred").await, served("0"));
    own.drop().await;
}

/// Publishes the nft cursors again where they stand: fixtures written after
/// a publication are pending writes (`shop.projection_write`) until one
/// commits after them.
async fn republish_nft(pool: &PgPool) {
    sqlx::query(
        "UPDATE shop.chain_cursor SET folded_at = timezone('UTC', now()) \
         WHERE indexer = 'nft' AND folded_block IS NOT NULL",
    )
    .execute(pool)
    .await
    .unwrap();
}

/// One event of a token's history, for the subgraph model below.
#[derive(Clone, Copy, Debug)]
enum TokenEvent {
    /// `OrderCreated(id)` on Marketplace `market` (0 or 1) by `seller`.
    Created(u8, usize, &'static str),
    /// `Transfer` to `to`.
    Transfer(&'static str),
    /// `OrderSuccessful(id)` on `market` to `buyer`.
    Sold(u8, usize, &'static str),
    /// `OrderCancelled(id)` on `market`.
    Cancelled(u8, usize),
}

/// The order statuses the collections graph leaves after `events`
/// (`handlers/marketplace.ts`, `marketplaceV2.ts`, `modules/nft`:
/// `cancelActiveOrder`, `updateNFTOrderProperties`, `handleTransferOrder`).
fn subgraph_statuses(events: &[TokenEvent]) -> std::collections::BTreeMap<u8, &'static str> {
    use std::collections::BTreeMap;
    let mut orders: BTreeMap<u8, (&'static str, &'static str)> = BTreeMap::new();
    // None: never set; Some(None): cleared (''); Some(Some(id)): active.
    let mut active: Option<Option<u8>> = None;
    for event in events {
        match *event {
            TokenEvent::Created(id, _, seller) => {
                if let Some(Some(old)) = active
                    && let Some(order) = orders.get_mut(&old)
                    && matches!(order.0, "open" | "transferred")
                {
                    order.0 = "cancelled";
                }
                orders.insert(id, ("open", seller));
                active = Some(Some(id));
            }
            TokenEvent::Sold(id, _, _) => {
                if let Some(order) = orders.get_mut(&id) {
                    order.0 = "sold";
                    active = Some(None);
                }
            }
            TokenEvent::Cancelled(id, _) => {
                if let Some(order) = orders.get_mut(&id) {
                    order.0 = "cancelled";
                    active = Some(None);
                }
            }
            TokenEvent::Transfer(to) => {
                if let Some(Some(id)) = active
                    && let Some(order) = orders.get_mut(&id)
                {
                    if order.0 == "open" {
                        order.0 = "transferred";
                    } else if order.0 == "transferred" && order.1 == to {
                        order.0 = "open";
                    }
                }
            }
        }
    }
    orders
        .into_iter()
        .map(|(id, (status, _))| (id, status))
        .collect()
}

/// The order book against the collections graph over the traces that
/// decide `transferred`: a token sent away and back, away for good, relisted
/// by its new owner, relisted by its seller, sold, cancelled then moved;
/// every status agrees. The one known difference is pinned: an order
/// superseded on the other Polygon Marketplace whose token is sold there
/// and comes back to its seller reads `open` here (its seller holds the
/// token and it is still executable on chain), where the graph, having
/// cleared the token's active order with the sale, keeps it `transferred`.
#[tokio::test]
async fn the_order_book_agrees_with_the_subgraph_on_transfers_away_and_back() {
    use TokenEvent::*;
    let up = FakeGraph::new();
    let url = serve_graph(up.clone()).await;
    let Some((state, own)) = state_over_own(&url, true).await else {
        return;
    };
    super::trade_events::tests::publish_cursors(&state.pool).await;
    let pool = state.pool.clone();
    let c = contract_of(&tag(), 4);
    sqlx::query(
        "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, is_completed, is_approved, is_editable, created_at, updated_at, reviewed_at, mirrored_at) \
         SELECT 'matic', $1, 137, 'Trace', $2, $2, 'urn:decentraland:matic:collections-v2:' || $1, true, true, false, 1, 1, 1, timezone('UTC', now())",
    )
    .bind(&c)
    .bind(CREATOR)
    .execute(&pool)
    .await
    .unwrap();
    super::owned_items::test_support::publish(
        &pool,
        &super::chain_walk::ChainNet::ALL,
        false,
        Duration::ZERO,
    )
    .await;
    let markets =
        super::nft_index::deployments::Deployments::mainnet(super::chain_walk::ChainNet::Matic)
            .markets;
    const S: &str = WALLET_A;
    const B: &str = WALLET_B;
    const X: &str = CREATOR;
    let traces: Vec<(&str, Vec<TokenEvent>)> = vec![
        (
            "away and back",
            vec![Created(1, 0, S), Transfer(B), Transfer(S)],
        ),
        (
            "away for good",
            vec![Created(2, 0, S), Transfer(B), Transfer(X)],
        ),
        (
            "relisted by the new owner",
            vec![Created(3, 0, S), Transfer(B), Created(4, 1, B)],
        ),
        (
            "relisted by its seller",
            vec![Created(5, 0, S), Created(6, 0, S)],
        ),
        ("sold", vec![Created(7, 1, S), Transfer(B), Sold(7, 1, B)]),
        (
            "cancelled, then away and back",
            vec![Created(8, 0, S), Cancelled(8, 0), Transfer(B), Transfer(S)],
        ),
        (
            "superseded, sold on the other Marketplace, back",
            vec![
                Created(9, 0, S),
                Created(10, 1, S),
                Transfer(X),
                Sold(9, 0, X),
                Transfer(S),
            ],
        ),
    ];
    let order_id = |n: u8| format!("0x{n:064x}");
    let mut block = 100i64;
    for (token, (_, events)) in traces.iter().enumerate() {
        let mut owner = S;
        for event in events {
            block += 1;
            let (kind, market, from, to, detail): (
                &str,
                &str,
                Option<&str>,
                Option<&str>,
                Option<String>,
            ) = match *event {
                Created(id, m, seller) => (
                    "order_created",
                    markets[m].0,
                    Some(seller),
                    None,
                    Some(order_id(id)),
                ),
                Transfer(to) => ("transfer", c.as_str(), Some(owner), Some(to), None),
                Sold(id, m, buyer) => (
                    "order_successful",
                    markets[m].0,
                    Some(owner),
                    Some(buyer),
                    Some(order_id(id)),
                ),
                Cancelled(id, m) => (
                    "order_cancelled",
                    markets[m].0,
                    Some(owner),
                    None,
                    Some(order_id(id)),
                ),
            };
            if let Transfer(to) = *event {
                owner = to;
            }
            sqlx::query(
                "INSERT INTO shop.nft_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, from_address, to_address, amount, expires_at, detail) \
                 VALUES ('matic', $1, 0, $1, $2, $3, $4, $5::numeric, $6, $7, 1000, CASE WHEN $2 = 'order_created' THEN 4000000000000 END, $8)",
            )
            .bind(block)
            .bind(kind)
            .bind(market)
            .bind(&c)
            .bind(token.to_string())
            .bind(from)
            .bind(to)
            .bind(detail)
            .execute(&pool)
            .await
            .unwrap();
        }
        sqlx::query(
            "INSERT INTO shop.nft (network, contract_address, token_id, owner, item_id, issued_id, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
             VALUES ('matic', $1, $2::numeric, $3, 0, $2::numeric, 1, 1, $4, 0, $4)",
        )
        .bind(&c)
        .bind(token.to_string())
        .bind(owner)
        .bind(block)
        .execute(&pool)
        .await
        .unwrap();
    }
    let mut tx = pool.begin().await.unwrap();
    super::legacy_orders::fold(&mut tx, super::chain_walk::ChainNet::Matic, -1)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    republish_nft(&pool).await;
    for (token, (label, events)) in traces.iter().enumerate() {
        let (status, body) = get(
            market_router(state.clone()),
            &format!("/v1/orders?contractAddress={c}&tokenId={token}&network=MATIC"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{label}: {body}");
        let ours: std::collections::BTreeMap<u8, String> = body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| {
                let id = o["id"].as_str().unwrap();
                (
                    u8::from_str_radix(id.trim_start_matches("0x"), 16).unwrap(),
                    o["status"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        let mut graph: std::collections::BTreeMap<u8, String> = subgraph_statuses(events)
            .into_iter()
            .map(|(id, status)| (id, status.to_string()))
            .collect();
        if *label == "superseded, sold on the other Marketplace, back" {
            assert_eq!(
                graph.get(&10).map(String::as_str),
                Some("transferred"),
                "{label}: the graph"
            );
            assert_eq!(
                ours.get(&10).map(String::as_str),
                Some("open"),
                "{label}: the book"
            );
            graph.insert(10, "open".to_string());
        }
        assert_eq!(ours, graph, "{label}: {body}");
    }
    own.drop().await;
}

/// The legacy order book's total counts each network up to the reader's
/// cap and sums them, as the subgraph reader did: 3,000 sold orders on each
/// network total 6,000 on a shallow page, not 5,000.
#[tokio::test]
async fn the_legacy_order_total_caps_each_network_and_sums_them() {
    let up = FakeGraph::new();
    let url = serve_graph(up.clone()).await;
    let Some((state, own)) = state_over_own(&url, true).await else {
        return;
    };
    super::trade_events::tests::publish_cursors(&state.pool).await;
    let pool = state.pool.clone();
    let c = contract_of(&tag(), 6);
    sqlx::query(
        "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, is_completed, is_approved, is_editable, created_at, updated_at, reviewed_at, mirrored_at) \
         SELECT 'matic', $1, 137, 'Counted', $2, $2, 'urn:decentraland:matic:collections-v2:' || $1, true, true, false, 1, 1, 1, timezone('UTC', now())",
    )
    .bind(&c)
    .bind(CREATOR)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO shop.legacy_order (network, marketplace_address, order_id, contract_address, token_id, seller, buyer, price, expires_at, status, \
             created_block, created_log, created_at, updated_block, updated_log, updated_at) \
         SELECT n, 'm', n || i, CASE n WHEN 'matic' THEN $1 ELSE $2 END, i, $3, $4, 1, 0, 'sold', i, 0, i, i, 0, i \
         FROM generate_series(1, 3000) i, unnest(ARRAY['matic', 'ethereum']) n",
    )
    .bind(&c)
    .bind(crate::land_authz::LAND_REGISTRY_MAINNET.to_lowercase())
    .bind(WALLET_A)
    .bind(WALLET_B)
    .execute(&pool)
    .await
    .unwrap();
    super::owned_items::test_support::publish(
        &pool,
        &super::chain_walk::ChainNet::ALL,
        false,
        Duration::ZERO,
    )
    .await;
    let app = || market_router(state.clone());
    let (status, body) = get(app(), "/v1/orders?status=sold&first=1").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["total"], "6000", "{body}");
    let (_, body) = get(app(), "/v1/orders?status=sold&first=1&network=MATIC").await;
    assert_eq!(body["total"], "3000", "{body}");
    own.drop().await;
}

/// `/v1/activity` keeps its partial 200 when an order leg's index is not
/// ready, and names each source that left a leg out in
/// `x-activity-unavailable-sources`: both while neither is ready, then the
/// land-authz index while an owner's orders (which may hold an open LAND
/// order) need it; no header once every leg answers.
#[tokio::test]
async fn a_partial_activity_names_the_unavailable_sources() {
    let up = FakeGraph::new();
    let url = serve_graph(up.clone()).await;
    let Some((state, own)) = state_over_own(&url, true).await else {
        return;
    };
    super::trade_events::tests::publish_cursors(&state.pool).await;
    let me = crate::shared::signed_fetch::test_support::address();
    let marketplace = r#"{"signer":"dcl:marketplace"}"#;
    let activity = || {
        let state = state.clone();
        let me = me.clone();
        async move {
            let headers = signed_headers("get", "/v1/activity", marketplace, true).await;
            let mut req = Request::builder().uri(format!("/v1/activity?address={me}"));
            for (k, v) in headers.iter() {
                req = req.header(k, v);
            }
            let resp = market_router(state)
                .oneshot(req.body(Body::empty()).unwrap())
                .await
                .unwrap();
            let status = resp.status();
            let named = resp
                .headers()
                .get(super::activity::UNAVAILABLE_SOURCES_HEADER)
                .map(|v| v.to_str().unwrap().to_string());
            (status, named)
        }
    };
    assert_eq!(
        activity().await,
        (
            StatusCode::OK,
            Some("land-authz-index, nft-index, sales-index".to_string())
        ),
        "neither is ready: the owner's orders stop at the LAND index, the buyer's at the nft index; \
         the transfers leg waits on the sales index, which this node does not enable"
    );
    super::owned_items::test_support::publish(
        &state.pool,
        &super::chain_walk::ChainNet::ALL,
        false,
        Duration::ZERO,
    )
    .await;
    assert_eq!(
        activity().await,
        (
            StatusCode::OK,
            Some("land-authz-index, sales-index".to_string())
        ),
        "an owner's orders may hold an open LAND order"
    );
    sqlx::query("INSERT INTO land_authz.index_cursor (network, land_registry, estate_registry, last_block, folded_block, folded_at, full_replay_started) VALUES ('ethereum-mainnet', $1, $2, 1, 1, timezone('UTC', now()), true)")
        .bind(crate::land_authz::LAND_REGISTRY_MAINNET)
        .bind(crate::land_authz::ESTATE_REGISTRY_MAINNET)
        .execute(&state.pool)
        .await
        .unwrap();
    assert_eq!(
        activity().await,
        (StatusCode::OK, Some("sales-index".to_string())),
        "every order leg answered; only the transfers leg is left out"
    );
    own.drop().await;
}

/// A contract can pin the order read even without the network filter.
#[tokio::test]
async fn order_readiness_follows_the_requested_contracts_and_legs() {
    use super::chain_walk::ChainNet;
    let graph = FakeGraph::new();
    let url = serve_graph(graph.clone()).await;
    let Some((state, own)) = state_over_own(&url, true).await else {
        return;
    };
    let pool = &own.pool;
    let contract = contract_of(&tag(), 1);
    let unknown = contract_of(&tag(), 2);
    let collection = |network: &'static str, chain: i32| {
        let contract = &contract;
        async move {
            sqlx::query(
            "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, is_completed, is_approved, is_editable, created_at, updated_at, reviewed_at, mirrored_at)
             VALUES ($1, $2, $3, 'Orders', $4, $4, $2, true, true, false, 1, 1, 1, timezone('UTC', now()))
             ON CONFLICT (network, contract_address) DO UPDATE SET chain_id = EXCLUDED.chain_id",
        )
        .bind(network)
        .bind(contract)
        .bind(chain)
        .bind(CREATOR)
        .execute(pool)
        .await
        .unwrap();
        }
    };
    collection("matic", 137).await;
    collection("ethereum", 11155111).await;
    sqlx::query(
        "INSERT INTO shop.legacy_order (network, marketplace_address, order_id, contract_address, token_id, seller, buyer, price, expires_at, status,
             created_block, created_log, created_at, updated_block, updated_log, updated_at)
         VALUES ('matic', '0xmarket', 'sold', $1, 1, $2, $3, 10, 0, 'sold', 1, 0, 1, 1, 0, 1)",
    )
    .bind(&contract)
    .bind(WALLET_A)
    .bind(WALLET_B)
    .execute(pool)
    .await
    .unwrap();
    super::owned_items::test_support::publish(pool, &[ChainNet::Matic], false, Duration::ZERO)
        .await;
    super::trade_events::tests::publish_cursors(pool).await;
    sqlx::query(
        "DELETE FROM shop.chain_cursor WHERE indexer = 'trade_events' AND network = 'ethereum'",
    )
    .execute(pool)
    .await
    .unwrap();

    let call = |uri: String| {
        let router = market_router(state.clone());
        async move { get(router, &uri).await }
    };
    let (status, body) = call(format!("/v1/orders?contractAddress={contract}&status=open")).await;
    assert_eq!(
        (status, body["total"].as_str()),
        (StatusCode::OK, Some("0")),
        "{body}"
    );
    let (status, body) = call(format!("/v1/orders/open-by-items?item={contract}-0")).await;
    assert_eq!(
        (status, body["total"].as_str()),
        (StatusCode::OK, Some("0")),
        "{body}"
    );

    for uri in [
        format!("/v1/orders?status=sold&contractAddress={unknown}"),
        format!("/v1/orders?status=sold&contractAddress={contract}&network=ETHEREUM"),
        format!("/v1/orders/open-by-items?item={contract}-0&item={unknown}-0"),
    ] {
        let (status, body) = call(uri).await;
        assert_eq!(
            (status, body["source"].as_str()),
            (StatusCode::SERVICE_UNAVAILABLE, Some("nft-index")),
            "{body}"
        );
    }

    collection("ethereum", 1).await;
    let (status, body) = call(format!("/v1/orders?status=sold&contractAddress={contract}")).await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("nft-index")),
        "{body}"
    );
    // An explicit network limits both the snapshot and the returned rows.
    sqlx::query(
        "DELETE FROM shop.chain_cursor WHERE indexer = 'trade_events' AND network = 'matic'",
    )
    .execute(pool)
    .await
    .unwrap();
    for (suffix, count) in [
        ("status=sold".to_string(), "1"),
        ("status=cancelled".to_string(), "0"),
        ("status=transferred".to_string(), "0"),
        (format!("buyer={WALLET_B}"), "0"),
    ] {
        let (status, body) = call(format!(
            "/v1/orders?contractAddress={contract}&network=MATIC&{suffix}"
        ))
        .await;
        assert_eq!(
            (status, body["total"].as_str()),
            (StatusCode::OK, Some(count)),
            "{suffix}: {body}"
        );
    }
    let (status, body) = call(format!(
        "/v1/orders?contractAddress={contract}&network=MATIC&status=open"
    ))
    .await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("trade-events-index")),
        "{body}"
    );
    own.drop().await;
}

/// Scope follows the live trade while the rewind removes its backing NFT.
#[tokio::test]
async fn a_trade_keeps_its_network_in_scope_after_its_nft_row_is_rewound() {
    use super::chain_walk::ChainNet;
    let url = serve_graph(FakeGraph::new()).await;
    let Some((state, own)) = state_over_own(&url, true).await else {
        return;
    };
    let pool = &own.pool;
    let contract = contract_of(&tag(), 1);
    sqlx::query(
        "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, is_completed, is_approved, is_editable, created_at, updated_at, reviewed_at, mirrored_at)
         VALUES ('matic', $1, 137, 'Orders', $2, $2, $1, true, true, false, 1, 1, 1, timezone('UTC', now()))",
    )
    .bind(&contract)
    .bind(CREATOR)
    .execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO shop.nft (network, contract_address, token_id, owner, minted_block, minted_at, transferred_block, transferred_log, transferred_at)
         VALUES ('ethereum', $1, 1, $2, 1, 1, 1, 0, 1)",
    )
    .bind(&contract)
    .bind(WALLET_A)
    .execute(pool).await.unwrap();
    super::owned_items::test_support::publish(pool, &ChainNet::ALL, false, Duration::ZERO).await;
    super::trade_events::tests::publish_cursors(pool).await;
    let (listing, _) = insert_trade_on(
        pool,
        "ETHEREUM",
        1,
        "public_nft_order",
        WALLET_A,
        Asset {
            asset_type: 3,
            contract: contract.clone(),
            value: "1".into(),
        },
        Asset {
            asset_type: 1,
            contract: "0x2222222222222222222222222222222222222222".into(),
            value: "1000".into(),
        },
    )
    .await;
    sqlx::query("UPDATE shop.trades SET mirrored_from = 'https://trades.example', mirrored_at = timezone('UTC', now()) WHERE id=$1::uuid")
        .bind(&listing).execute(pool).await.unwrap();
    let read = || async {
        get(
            market_router(state.clone()),
            &format!("/v1/orders?contractAddress={contract}&status=open"),
        )
        .await
    };
    let (status, body) = read().await;
    assert_eq!(
        (status, body["data"][0]["id"].as_str()),
        (StatusCode::OK, Some(listing.as_str())),
        "{body}"
    );
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("DELETE FROM shop.nft WHERE network='ethereum'")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("UPDATE shop.chain_cursor SET folded_block=NULL, folded_at=NULL WHERE indexer='nft' AND network='ethereum'")
        .execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    let (status, body) = read().await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("nft-index")),
        "{body}"
    );
    super::owned_items::test_support::publish(pool, &[ChainNet::Ethereum], false, Duration::ZERO)
        .await;
    let (status, body) = read().await;
    assert_eq!(
        (status, body["total"].as_str()),
        (StatusCode::OK, Some("0")),
        "{body}"
    );
    own.drop().await;
}

/// Awaiting the graph leaves the pool free even when it has only one connection.
#[tokio::test]
async fn graph_orders_release_the_backed_listing_snapshot_before_the_graph_call() {
    let graph = FakeGraph::new();
    let url = serve_graph(graph.clone()).await;
    let Some(db) = super::tests::support::OwnDb::create("order_graph_snapshot").await else {
        return;
    };
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with(db.pool.connect_options().as_ref().clone())
        .await
        .unwrap();
    let state = Arc::new(super::MarketplaceState::new(
        pool.clone(),
        crate::http::Outbound::for_tests(),
        market_settings(&url, &url, true),
    ));
    state.test_allow_loopback.store(true, Ordering::Relaxed);
    super::trade_events::tests::publish_cursors(&pool).await;
    graph.pause.store(true, Ordering::SeqCst);
    let route = tokio::spawn(get(
        market_router(state.clone()),
        "/v1/orders?network=MATIC",
    ));
    tokio::time::timeout(Duration::from_secs(3), graph.arrived.notified())
        .await
        .unwrap();
    let available = tokio::time::timeout(Duration::from_secs(3), pool.acquire()).await;
    graph.release.notify_one();
    drop(available.unwrap().unwrap());
    assert_eq!(route.await.unwrap().0, StatusCode::OK);
    sqlx::query("DELETE FROM shop.chain_cursor WHERE indexer = 'trade_events'")
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = get(
        market_router(state),
        "/v1/orders?network=MATIC&status=cancelled",
    )
    .await;
    assert_eq!(
        (status, body["total"].as_str()),
        (StatusCode::OK, Some("0")),
        "{body}"
    );
    pool.close().await;
    db.drop().await;
}

async fn publish_trades_mirror_for_timeout_test(state: &mut SharedState, pool: &PgPool) {
    let upstream = "https://example.invalid/v1/trades";
    Arc::get_mut(state)
        .unwrap()
        .settings
        .trades_sync_upstream_url = Some(upstream.into());
    sqlx::query(
        "INSERT INTO shop.trades_mirror (id, upstream_url, completed_at)
         VALUES (1, $1, timezone('UTC', now()))",
    )
    .bind(upstream)
    .execute(pool)
    .await
    .unwrap();
}

/// Hold the marker long enough for PostgreSQL to cancel the route's read.
async fn assert_trades_readiness_timeout(
    pool: &PgPool,
    app: Router,
    uri: &str,
    lock_sql: &'static str,
) {
    let (status, body) = get(app.clone(), uri).await;
    assert_eq!(status, StatusCode::OK, "{uri}: {body}");

    let mut lock = pool.begin().await.unwrap();
    sqlx::query(lock_sql).execute(&mut *lock).await.unwrap();
    let response = tokio::time::timeout(
        super::nft_reads::READ_STATEMENT_TIMEOUT + Duration::from_secs(5),
        get(app.clone(), uri),
    )
    .await;
    lock.rollback().await.unwrap();

    let (status, body) = response.expect("PostgreSQL cancelled the blocked marker read");
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{uri}: {body}");
    let message = format!(
        "the read ran past its {} s statement bound: narrow the filters or the page window",
        super::nft_reads::READ_STATEMENT_TIMEOUT.as_secs()
    );
    assert_eq!(
        body["message"].as_str(),
        Some(message.as_str()),
        "{uri}: {body}"
    );

    let (status, body) = get(app, uri).await;
    assert_eq!(status, StatusCode::OK, "{uri}: {body}");
}

#[tokio::test]
async fn the_order_book_preserves_a_trades_mirror_timeout() {
    let url = serve_empty_graph().await;
    let Some((mut state, own)) = state_over_own(&url, true).await else {
        return;
    };
    let pool = &own.pool;
    publish_trades_mirror_for_timeout_test(&mut state, pool).await;
    super::owned_items::test_support::publish(
        pool,
        &[super::chain_walk::ChainNet::Matic],
        false,
        Duration::ZERO,
    )
    .await;
    super::trade_events::tests::publish_cursors(pool).await;
    assert_trades_readiness_timeout(
        pool,
        market_router(state),
        "/v1/orders?network=MATIC&status=open",
        "LOCK TABLE shop.trades_mirror IN ACCESS EXCLUSIVE MODE",
    )
    .await;
    own.drop().await;
}

#[tokio::test]
async fn the_graph_order_path_preserves_a_trade_events_marker_timeout() {
    let Some(own) = super::tests::support::OwnDb::create("order_marker_timeout").await else {
        return;
    };
    let url = serve_empty_graph().await;
    let state = Arc::new(super::MarketplaceState::new(
        own.pool.clone(),
        crate::http::Outbound::for_tests(),
        market_settings(&url, &url, true),
    ));
    state.test_allow_loopback.store(true, Ordering::Relaxed);
    assert!(!super::owned_items::walks_here(&state));
    super::trade_events::tests::publish_cursors(&own.pool).await;
    let app = market_router(state);
    assert_trades_readiness_timeout(
        &own.pool,
        app.clone(),
        "/v1/orders?network=MATIC&status=open",
        "LOCK TABLE shop.chain_cursor IN ACCESS EXCLUSIVE MODE",
    )
    .await;
    sqlx::query("ALTER TABLE shop.chain_cursor RENAME TO unavailable_chain_cursor")
        .execute(&own.pool)
        .await
        .unwrap();
    for uri in [
        "/v1/orders?network=MATIC&status=open",
        "/v1/bids?network=MATIC",
    ] {
        let (status, body) = get(app.clone(), uri).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{uri}: {body}");
        assert_eq!(body["source"], "trade-events-index", "{uri}: {body}");
    }
    own.drop().await;
}

#[tokio::test]
async fn open_orders_batch_preserves_each_contracts_cheapest_window() {
    let graph = FakeGraph::new();
    let url = serve_graph(graph.clone()).await;
    let Some((state, own)) = state_over_own(&url, true).await else {
        return;
    };
    let pool = &own.pool;
    let fixture = tag();
    let matic = contract_of(&fixture, 1);
    let ethereum = contract_of(&fixture, 2);
    let token = |item: u64, issued: u64| {
        ((alloy_primitives::U256::from(item) << 216u32) + alloy_primitives::U256::from(issued))
            .to_string()
    };
    for (network, chain, contract) in [("matic", 137, &matic), ("ethereum", 1, &ethereum)] {
        sqlx::query(
            "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner,
                urn, is_completed, is_approved, is_editable, created_at, updated_at, reviewed_at, mirrored_at)
             VALUES ($1, $2, $3, 'Orders', $4, $4, $2, true, true, false, 1, 1, 1, timezone('UTC', now()))",
        )
        .bind(network).bind(contract).bind(chain).bind(CREATOR).execute(pool).await.unwrap();
    }
    for (network, contract, item, issued, id, price) in [
        ("matic", &matic, 2, 1, "unrequested", "1"),
        ("matic", &matic, 3, 2, "legacy", "2"),
        ("matic", &matic, 3, 3, "v3-token", "20"),
        ("ethereum", &ethereum, 3, 1, "eth-cheap", "1"),
        (
            "ethereum",
            &ethereum,
            3,
            2,
            "eth-wide",
            "340282366920938463463374607431768211456",
        ),
    ] {
        let token_id = token(item, issued);
        sqlx::query(
            "INSERT INTO shop.nft (network, contract_address, token_id, owner, minted_block, minted_at,
                                    transferred_block, transferred_log, transferred_at)
             VALUES ($1, $2, $3::numeric, $4, 1, 1, 1, 0, 1)",
        )
        .bind(network).bind(contract).bind(&token_id).bind(WALLET_A).execute(pool).await.unwrap();
        if id == "v3-token" {
            continue;
        }
        sqlx::query(
            "INSERT INTO shop.legacy_order (network, marketplace_address, order_id, contract_address,
                token_id, seller, price, expires_at, status, created_block, created_log, created_at,
                updated_block, updated_log, updated_at)
             VALUES ($1, '0xmarket', $2, $3, $4::numeric, $5, $6::numeric, 4102444800000, 'open', 1, 0, 1, 1, 0, 1)",
        )
        .bind(network).bind(id).bind(contract).bind(token_id).bind(WALLET_A).bind(price)
        .execute(pool).await.unwrap();
    }
    let (trade, _) = insert_trade(
        pool,
        "public_nft_order",
        WALLET_A,
        Asset {
            asset_type: 3,
            contract: matic.clone(),
            value: token(3, 3),
        },
        Asset {
            asset_type: 1,
            contract: CREATOR.into(),
            value: "2".into(),
        },
    )
    .await;
    super::owned_items::test_support::publish(
        pool,
        &super::chain_walk::ChainNet::ALL,
        false,
        Duration::ZERO,
    )
    .await;
    super::trade_events::tests::publish_cursors(pool).await;
    republish_nft(pool).await;

    let app = market_router(state.clone());
    for (cap, expected) in [
        (2, vec![trade.as_str(), "eth-cheap", "eth-wide"]),
        (3, vec![trade.as_str(), "eth-cheap", "eth-wide", "legacy"]),
    ] {
        let (status, body) = get(app.clone(), &format!(
            "/v1/orders/open-by-items?item={matic}-3&item={matic}-3&item={ethereum}-3&perContract={cap}"
        )).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let mut ids: Vec<&str> = body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|order| order["id"].as_str().unwrap())
            .collect();
        ids.sort_unstable();
        let mut expected = expected;
        expected.sort_unstable();
        assert_eq!(ids, expected, "{body}");
    }
    let (status, body) = get(
        app,
        &format!("/v1/orders/open-by-items?item={matic}-2&item={matic}-3&perContract=2"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["total"], "2");
    assert_eq!(body["data"][0]["id"], "unrequested");
    assert_eq!(body["data"][1]["id"], trade);

    // The full decimal price matters both at the cutoff and when both legs fit.
    sqlx::query(
        "UPDATE shop.trade_assets_erc20 SET amount = 340282366920938463463374607431768211456
         WHERE asset_id IN (SELECT id FROM shop.trade_assets WHERE trade_id=$1::uuid)",
    )
    .bind(&trade)
    .execute(pool)
    .await
    .unwrap();
    let app = market_router(state.clone());
    for (cap, expected) in [
        (1, vec!["unrequested"]),
        (3, vec!["unrequested", "legacy", trade.as_str()]),
    ] {
        let (status, body) = get(
            app.clone(),
            &format!("/v1/orders/open-by-items?item={matic}-2&item={matic}-3&perContract={cap}"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let ids: Vec<&str> = body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|order| order["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, expected, "{body}");
    }
    // The graph-backed path merges the same local v3 candidate with the
    // legacy page from the configured subgraph.
    graph.set(
        "orders",
        vec![json!({
            "id": "graph-legacy", "marketplaceAddress": "0xmarket", "nftAddress": matic,
            "tokenId": token(3, 2), "owner": WALLET_A, "price": "1", "status": "open",
            "expiresAt": "4102444800000", "createdAt": "1700000000", "updatedAt": "1700000000",
        })],
    );
    let graph_settings = market_settings(&url, &serve_empty_graph().await, true);
    let graph_state = Arc::new(super::MarketplaceState::new(
        pool.clone(),
        crate::http::Outbound::for_tests(),
        graph_settings,
    ));
    graph_state
        .test_allow_loopback
        .store(true, Ordering::Relaxed);
    assert!(!super::owned_items::walks_here(&graph_state));
    let (status, body) = get(
        market_router(graph_state),
        &format!("/v1/orders/open-by-items?item={matic}-3&perContract=2"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["total"], "2", "{body}");
    assert_eq!(body["data"][0]["id"], "graph-legacy", "{body}");
    assert_eq!(body["data"][1]["id"], trade, "{body}");
    own.drop().await;
}

#[tokio::test]
async fn open_orders_batch_keeps_the_newest_v3_cap_per_contract() {
    let url = serve_empty_graph().await;
    let Some((state, own)) = state_over_own(&url, true).await else {
        return;
    };
    let pool = &own.pool;
    let fixture = tag();
    let primary = contract_of(&fixture, 1);
    let secondary = contract_of(&fixture, 2);
    let contracts = [primary.clone(), secondary.clone()];
    sqlx::query(
        "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner,
            urn, is_completed, is_approved, is_editable, created_at, updated_at, reviewed_at, mirrored_at)
         SELECT 'matic', c, 137, 'Orders', $2, $2, c, true, true, false, 1, 1, 1, timezone('UTC', now())
         FROM unnest($1::text[]) c",
    )
    .bind(contracts.as_slice()).bind(CREATOR).execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO shop.nft (network, contract_address, token_id, owner, minted_block, minted_at,
            transferred_block, transferred_log, transferred_at)
         SELECT 'matic', c, 1, $2, 1, 1, 1, 0, 1 FROM unnest($1::text[]) c",
    )
    .bind(contracts.as_slice())
    .bind(WALLET_A)
    .execute(pool)
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query(
        "CREATE TEMP TABLE newest_cap_fixture ON COMMIT DROP AS
         SELECT g, gen_random_uuid() AS trade, gen_random_uuid() AS sent, gen_random_uuid() AS received,
            CASE WHEN g = 1001 THEN $2::text ELSE $1::text END AS contract, $3::text AS owner
         FROM generate_series(0, 1001) g",
    )
    .bind(&primary).bind(&secondary).bind(WALLET_A).execute(&mut *tx).await.unwrap();
    sqlx::raw_sql(
        "INSERT INTO shop.trades (id, network, chain_id, signature, hashed_signature, checks, signer,
                                   type, expires_at, effective_since, contract, created_at)
         SELECT trade, 'MATIC', 137, trade::text, trade::text, '{\"uses\":1}', owner, 'public_nft_order',
                timezone('UTC', now()) + interval '1 day', timezone('UTC', now()) - interval '1 day',
                '0xe38ef22abe871513555cba89adfe45ab4f548ada',
                timezone('UTC', now()) - CASE g WHEN 0 THEN interval '2 minutes'
                                                WHEN 1001 THEN interval '1 minute' ELSE interval '1 second' END
         FROM newest_cap_fixture;
         INSERT INTO shop.trade_assets (id, trade_id, direction, asset_type, contract_address, beneficiary)
         SELECT sent, trade, 'sent'::shop.asset_direction_type, 3, contract, owner FROM newest_cap_fixture
         UNION ALL
         SELECT received, trade, 'received'::shop.asset_direction_type, 1, '0x2222222222222222222222222222222222222222', owner
         FROM newest_cap_fixture;
         INSERT INTO shop.trade_assets_erc721 (asset_id, token_id) SELECT sent, '1' FROM newest_cap_fixture;
         INSERT INTO shop.trade_assets_erc20 (asset_id, amount)
         SELECT received, CASE g WHEN 0 THEN 1 WHEN 1001 THEN 2 ELSE 10 END FROM newest_cap_fixture;
         INSERT INTO shop.trade_chain_state (trade_id, network, status, folded_block, digest)
         SELECT trade, 'matic', 'open', 0, trade::text FROM newest_cap_fixture;",
    )
    .execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    super::owned_items::test_support::publish(
        pool,
        &[super::chain_walk::ChainNet::Matic],
        false,
        Duration::ZERO,
    )
    .await;
    super::trade_events::tests::publish_cursors(pool).await;
    republish_nft(pool).await;

    let (status, body) = get(
        market_router(state),
        &format!("/v1/orders/open-by-items?item={primary}-0&item={secondary}-0&perContract=1"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["total"], "2");
    let prices: BTreeMap<_, _> = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|order| {
            (
                order["contractAddress"].as_str().unwrap(),
                order["price"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        prices[primary.as_str()],
        "10",
        "the older cheap order was outside the newest 1,000"
    );
    assert_eq!(
        prices[secondary.as_str()],
        "2",
        "the other contract keeps its own window"
    );
    own.drop().await;
}

#[tokio::test]
async fn order_publication_counters_name_the_route_of_each_check() {
    let url = serve_empty_graph().await;
    let Some((state, own)) = state_over_own(&url, true).await else {
        return;
    };
    let pool = &own.pool;
    let contract = contract_of(&tag(), 1);
    sqlx::query(
        "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner,
             urn, is_completed, is_approved, is_editable, created_at, updated_at, reviewed_at, mirrored_at)
         VALUES ('matic', $1, 137, 'Orders', $2, $2, $1, true, true, false, 1, 1, 1, timezone('UTC', now()))",
    ).bind(&contract).bind(CREATOR).execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO shop.legacy_order (network, marketplace_address, order_id, contract_address, token_id,
            seller, buyer, price, expires_at, status, created_block, created_log, created_at,
            updated_block, updated_log, updated_at)
         SELECT 'matic', '0xmarket', g::text, $1, g, $2, $3, 1, 4102444800000, 'sold', 1, 0, g, 1, 0, g
         FROM generate_series(1, 101) g",
    ).bind(&contract).bind(WALLET_A).bind(WALLET_B).execute(pool).await.unwrap();
    super::owned_items::test_support::publish(
        pool,
        &super::chain_walk::ChainNet::ALL,
        false,
        Duration::ZERO,
    )
    .await;
    super::trade_events::tests::publish_cursors(pool).await;
    republish_nft(pool).await;
    sqlx::query(
        "INSERT INTO land_authz.index_cursor (network, land_registry, estate_registry,
            last_block, folded_block, folded_at, full_replay_started)
         VALUES ('ethereum-mainnet', $1, $2, 1, 1, timezone('UTC', now()), true)",
    )
    .bind(crate::land_authz::LAND_REGISTRY_MAINNET)
    .bind(crate::land_authz::ESTATE_REGISTRY_MAINNET)
    .execute(pool)
    .await
    .unwrap();

    let recorder = crate::comms::metrics::prometheus_builder()
        .unwrap()
        .build_recorder();
    let _recording = dcl_metrics::set_default_local_recorder(&recorder);
    let app = market_router(state.clone());
    let uri = "/v1/orders?network=MATIC&status=sold";
    assert_eq!(get(app.clone(), uri).await.0, StatusCode::OK);
    sqlx::query(
        "UPDATE shop.legacy_order SET updated_at=200 WHERE network='matic' AND order_id='1'",
    )
    .execute(pool)
    .await
    .unwrap();
    assert_eq!(
        get(app.clone(), uri).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    republish_nft(pool).await;
    assert_eq!(
        get(app, &format!("/v1/orders/open-by-items?item={contract}-0"))
            .await
            .0,
        StatusCode::OK
    );
    let (_, _, unavailable) = super::activity::user_activity(&state, WALLET_A, None, None).await;
    assert!(!unavailable.contains("orders"), "{unavailable:?}");

    let rendered = recorder.handle().render();
    let check = |metric: &str, route: &str, count: &str| {
        let network = "network=\"matic\"";
        let route = format!("route=\"{route}\"");
        assert!(
            rendered.lines().any(|line| line.starts_with(metric)
                && line.contains(network)
                && line.contains(&route)
                && line.ends_with(count)),
            "{metric} {route}: {rendered}"
        );
    };
    check(
        "catalyst_shop_order_book_fold_read_checks_total",
        "orders",
        " 2",
    );
    check(
        "catalyst_shop_order_book_unpublished_fold_reads_total",
        "orders",
        " 1",
    );
    check(
        "catalyst_shop_order_book_fold_read_checks_total",
        "open_by_items",
        " 1",
    );
    // Two seller pages and the empty buyer page each check this network.
    check(
        "catalyst_shop_order_book_fold_read_checks_total",
        "activity",
        " 3",
    );
    own.drop().await;
}

/// A book read pins the trades readiness and the backed listings leg
/// together. Quarantine is visible to the next read.
#[tokio::test]
async fn the_book_and_backed_listings_share_their_read_snapshot() {
    use super::chain_walk::ChainNet;
    let url = serve_empty_graph().await;
    let Some((state, own)) = state_over_own(&url, true).await else {
        return;
    };
    let pool = own.pool.clone();
    super::trade_events::tests::publish_cursors(&pool).await;
    let contract = contract_of(&tag(), 1);
    let (listing, _) = insert_trade(
        &pool,
        "public_nft_order",
        WALLET_A,
        Asset {
            asset_type: 3,
            contract: contract.clone(),
            value: "1".into(),
        },
        Asset {
            asset_type: 1,
            contract: "0x2222222222222222222222222222222222222222".into(),
            value: "1000".into(),
        },
    )
    .await;
    sqlx::query(
        "INSERT INTO shop.nft (network, contract_address, token_id, owner, minted_block, minted_at, transferred_block, transferred_log, transferred_at)
         VALUES ('matic', $1, 1, $2, 1, 1, 1, 0, 1)",
    )
    .bind(&contract)
    .bind(WALLET_A)
    .execute(&pool)
    .await
    .unwrap();
    super::owned_items::test_support::publish(&pool, &[ChainNet::Matic], false, Duration::ZERO)
        .await;
    let mut read = match super::orders::book_snapshot(
        &state,
        Some(super::wire::Network::Matic),
        &[],
        false,
        true,
        super::orders::BookRoute::Orders,
    )
    .await
    {
        Ok(snapshot) => snapshot,
        Err(r) => panic!("the trades mirror is ready: {}", r.status()),
    };
    sqlx::query(
        "UPDATE shop.chain_cursor SET quarantined_at = timezone('UTC', now()), quarantine_reason = 'test' \
         WHERE indexer = 'trade_events' AND network = 'matic'",
    )
    .execute(&pool)
    .await
    .unwrap();
    let rows = super::orders::read_trades(
        &mut read.tx,
        &state,
        &super::orders::OrderFilters::default(),
        read.now,
    )
    .await
    .unwrap();
    read.tx.rollback().await.unwrap();
    assert_eq!(
        rows.into_iter().map(|k| k.order.id).collect::<Vec<_>>(),
        vec![listing],
        "the rows are the checked snapshot's"
    );
    let (status, body) = get(market_router(state.clone()), "/v1/orders?network=MATIC").await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("trade-events-index")),
        "{body}"
    );
    own.drop().await;
}

/// On the Sepolia/Amoy pair a walking node reads its orders from the book,
/// never 1/137 by default and never from the subgraph: before publication
/// `/v1/orders`, open-by-items and the activity orders leg name the nft
/// index; after it they serve only the collections the nft index walks there
/// (of that chain and verified on it), in the page and the total, with the
/// served chains as `chainId`; and no request reaches the order graph. A
/// publication bound to the mainnets does not serve it.
#[tokio::test]
async fn the_order_book_reads_the_served_testnet_pair() {
    use super::chain_walk::ChainNet;
    let Some(own) = super::tests::support::OwnDb::create("testnet_orders").await else {
        return;
    };
    let up = FakeGraph::new();
    let url = serve_graph(up.clone()).await;
    let mut settings = super::owned_items::test_support::walking(market_settings(&url, &url, true));
    settings.eth_network = "sepolia".into();
    settings.market.indexes.chain_id_ethereum = ChainNet::Ethereum.testnet_chain_id();
    settings.market.indexes.chain_id_matic = ChainNet::Matic.testnet_chain_id();
    let state = Arc::new(super::MarketplaceState::new(
        own.pool.clone(),
        crate::http::Outbound::for_tests(),
        settings,
    ));
    state.test_allow_loopback.store(true, Ordering::Relaxed);
    assert!(
        super::owned_items::walks_here(&state),
        "a Sepolia/Amoy walker reads the book"
    );
    let pool = own.pool.clone();
    super::trade_events::tests::publish_cursors(&pool).await;
    let app = || market_router(state.clone());
    let t = tag();
    let (verified, unverified, retained) =
        (contract_of(&t, 1), contract_of(&t, 2), contract_of(&t, 3));
    // Not published: each route names the nft index.
    for uri in [
        "/v1/orders?status=sold".to_string(),
        format!("/v1/orders/open-by-items?item={verified}-3"),
    ] {
        let (status, body) = get(app(), &uri).await;
        assert_eq!(
            (status, body["source"].as_str()),
            (StatusCode::SERVICE_UNAVAILABLE, Some("nft-index")),
            "{uri}: {body}"
        );
    }
    // The signed activity read: its status and the sources it names.
    let me = crate::shared::signed_fetch::test_support::address();
    let activity = || {
        let (state, me) = (state.clone(), me.clone());
        async move {
            let headers = signed_headers(
                "get",
                "/v1/activity",
                r#"{"signer":"dcl:marketplace"}"#,
                true,
            )
            .await;
            let mut req = Request::builder().uri(format!("/v1/activity?address={me}"));
            for (k, v) in headers.iter() {
                req = req.header(k, v);
            }
            let resp = market_router(state)
                .oneshot(req.body(Body::empty()).unwrap())
                .await
                .unwrap();
            let named = resp
                .headers()
                .get(super::activity::UNAVAILABLE_SOURCES_HEADER)
                .map(|v| v.to_str().unwrap().to_string());
            (resp.status(), named)
        }
    };
    let (status, named) = activity().await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        named.as_deref().is_some_and(|v| v.contains("nft-index")),
        "{named:?}"
    );
    let publish = |testnet: bool| {
        let pool = pool.clone();
        async move {
            for net in ChainNet::ALL {
                let chain = if testnet {
                    net.testnet_chain_id()
                } else {
                    net.chain_id()
                };
                sqlx::query(
                    "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, last_block_hash, fold_through, folded_block, folded_at, full_history, chain_id) \
                     VALUES ('nft', $1, 1, 100, '0x' || repeat('0', 64), 100, 100, timezone('UTC', now()), true, $2) \
                     ON CONFLICT (indexer, network) DO UPDATE SET chain_id = EXCLUDED.chain_id",
                )
                .bind(net.db_name())
                .bind(chain)
                .execute(&pool)
                .await
                .unwrap();
                sqlx::query(
                    "INSERT INTO shop.chain_checkpoint (indexer, network, block_number, block_hash, block_time) VALUES ('nft', $1, 100, '0x' || repeat('0', 64), 0) ON CONFLICT DO NOTHING",
                )
                .bind(net.db_name())
                .execute(&pool)
                .await
                .unwrap();
                sqlx::query(
                    "INSERT INTO shop.legacy_order_cursor (network, folded_through, through_hash) VALUES ($1, 100, '0x' || repeat('0', 64)) ON CONFLICT DO NOTHING",
                )
                .bind(net.db_name())
                .execute(&pool)
                .await
                .unwrap();
            }
        }
    };
    publish(true).await;
    // A retained mainnet row stays out even when marked verified.
    for (contract, chain, checked) in [
        (&verified, 80_002, true),
        (&unverified, 80_002, false),
        (&retained, 137, true),
    ] {
        sqlx::query(
            "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, is_completed, is_approved, is_editable, created_at, updated_at, reviewed_at, mirrored_at, testnet_verified) \
             SELECT 'matic', $1, $2, 'Pirates', $3, $3, 'urn:decentraland:amoy:collections-v2:' || $1, true, true, false, 1, 1, 1, timezone('UTC', now()), $4",
        )
        .bind(contract)
        .bind(chain)
        .bind(CREATOR)
        .bind(checked)
        .execute(&pool)
        .await
        .unwrap();
    }
    let sepolia_land = super::nft_index::deployments::Deployments::for_chain(
        ChainNet::Ethereum,
        ChainNet::Ethereum.testnet_chain_id(),
    )
    .and_then(|d| d.land)
    .unwrap()
    .land_registry
    .to_lowercase();
    let token = (alloy_primitives::U256::from(3u64) << 216u32 | alloy_primitives::U256::from(1u64))
        .to_string();
    for (network, id, contract, token) in [
        ("matic", "verified", &verified, token.as_str()),
        ("matic", "unverified", &unverified, token.as_str()),
        ("matic", "retained", &retained, token.as_str()),
        ("ethereum", "parcel", &sepolia_land, "77"),
    ] {
        sqlx::query(
            "INSERT INTO shop.legacy_order (network, marketplace_address, order_id, contract_address, token_id, seller, buyer, price, expires_at, status, \
                 created_block, created_log, created_at, updated_block, updated_log, updated_at) \
             VALUES ($1, '0xmarket', $2, $3, $4::numeric, $5, $6, 10, 0, 'sold', 1, 0, 1, 1, 0, 1)",
        )
        .bind(network)
        .bind(id)
        .bind(contract)
        .bind(token)
        .bind(WALLET_A)
        .bind(WALLET_B)
        .execute(&pool)
        .await
        .unwrap();
    }
    republish_nft(&pool).await;
    let read = |uri: String| {
        let router = app();
        async move {
            let (status, body) = get(router, &uri).await;
            assert_eq!(status, StatusCode::OK, "{uri}: {body}");
            let mut page: Vec<(String, i64)> = body["data"]
                .as_array()
                .unwrap()
                .iter()
                .map(|o| {
                    (
                        o["id"].as_str().unwrap().to_string(),
                        o["chainId"].as_i64().unwrap(),
                    )
                })
                .collect();
            page.sort();
            (page, body["total"].as_str().map(str::to_string))
        }
    };
    assert_eq!(
        read("/v1/orders?status=sold".into()).await,
        (
            vec![
                ("parcel".to_string(), 11_155_111),
                ("verified".to_string(), 80_002),
            ],
            Some("2".to_string())
        ),
        "the walked catalogue and the served registries, on the served chains"
    );
    for (contract, expected) in [(&verified, 1), (&unverified, 0), (&retained, 0)] {
        let (page, total) = read(format!("/v1/orders?status=sold&itemId={contract}-3")).await;
        assert_eq!(
            (page.len(), total),
            (expected, Some(expected.to_string())),
            "{contract}"
        );
    }
    let (status, body) = get(
        app(),
        &format!("/v1/orders/open-by-items?item={verified}-3"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (uncatalogued, _) = insert_trade_on(
        &pool,
        "ETHEREUM",
        11_155_111,
        "public_nft_order",
        WALLET_A,
        Asset {
            asset_type: 3,
            contract: verified.clone(),
            value: token.clone(),
        },
        Asset {
            asset_type: 1,
            contract: "0x2222222222222222222222222222222222222222".into(),
            value: "1000".into(),
        },
    )
    .await;
    sqlx::query("UPDATE shop.chain_cursor SET folded_block=NULL, folded_at=NULL WHERE indexer='nft' AND network='ethereum'")
        .execute(&pool).await.unwrap();
    for uri in [
        format!("/v1/orders?status=open&contractAddress={verified}"),
        format!("/v1/orders/open-by-items?item={verified}-3"),
    ] {
        let (status, body) = get(app(), &uri).await;
        assert_eq!(
            (status, body["total"].as_str()),
            (StatusCode::OK, Some("0")),
            "{uri}: {body}"
        );
    }
    delete_trade(&pool, &uncatalogued).await;
    sqlx::query("UPDATE shop.chain_cursor SET folded_block=100, folded_at=timezone('UTC',now()) WHERE indexer='nft' AND network='ethereum'")
        .execute(&pool).await.unwrap();
    // An Ethereum item filter reads the v1 wearable's item, which must be
    // of the served chain and verified there too, as the collection is.
    let v1 = contract_of(&t, 4);
    let urn = "urn:decentraland:sepolia:collections-v1:halloween";
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, is_completed, is_approved, is_editable, created_at, updated_at, reviewed_at, mirrored_at, testnet_verified)
         VALUES ('ethereum', '{v1}', 11155111, 'Halloween', '{CREATOR}', '{CREATOR}', '{urn}', true, true, false, 1, 1, 1, timezone('UTC', now()), true);
         INSERT INTO shop.nft (network, contract_address, token_id, owner, wearable_id, minted_block, minted_at, transferred_block, transferred_log, transferred_at)
         VALUES ('ethereum', '{v1}', 9, '{WALLET_A}', 'hat', 1, 1, 1, 0, 1);
         INSERT INTO shop.legacy_order (network, marketplace_address, order_id, contract_address, token_id, seller, buyer, price, expires_at, status,
             created_block, created_log, created_at, updated_block, updated_log, updated_at)
         VALUES ('ethereum', '0xmarket', 'hat', '{v1}', 9, '{WALLET_A}', '{WALLET_B}', 10, 0, 'sold', 1, 0, 1, 1, 0, 1)"
    )))
    .execute(&pool)
    .await
    .unwrap();
    republish_nft(&pool).await;
    for (chain, checked, expected) in [(11_155_111, false, 0), (1, true, 0), (11_155_111, true, 1)]
    {
        sqlx::query(
            "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, total_supply, max_supply, available, \
                 price, name, category, created_at, updated_at, reviewed_at, mirrored_at, testnet_verified) \
             VALUES ('ethereum', $1, 5, $2, 'wearable_v1', $3 || ':hat', $4, 'common', 1, 1, 0, 0, 'Hat', 'hat', 1, 1, 1, timezone('UTC', now()), $5) \
             ON CONFLICT (network, contract_address, item_id) DO UPDATE SET chain_id = EXCLUDED.chain_id, testnet_verified = EXCLUDED.testnet_verified",
        )
        .bind(&v1)
        .bind(chain)
        .bind(urn)
        .bind(CREATOR)
        .bind(checked)
        .execute(&pool)
        .await
        .unwrap();
        let (page, total) = read(format!("/v1/orders?status=sold&itemId={v1}-5")).await;
        assert_eq!(
            (page.len(), total),
            (expected, Some(expected.to_string())),
            "an item of chain {chain}, verified {checked}"
        );
    }
    let (status, named) = activity().await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        named.as_deref().is_none_or(|v| !v.contains("nft-index")),
        "the activity orders leg answers from the book: {named:?}"
    );
    assert!(
        !up.requests.lock().unwrap().iter().any(|e| e == "orders"),
        "no order read asks the subgraph: {:?}",
        up.requests.lock().unwrap()
    );
    // A v3 listing carries its trade's chain, which the leg keeps to the
    // served one, never the network's default.
    let listing = sqlx::query(
        "SELECT 'listing' AS id, NULL::text AS marketplace_address, '0xc' AS nft_address, '5' AS token_id, '10' AS price, \
                '0xo' AS owner, NULL::text AS payee, 0::bigint AS created_at, 0::bigint AS expires_at, 'MATIC' AS network, \
                80002::bigint AS chain_id, '5' AS issued_id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        super::orders::trade_row_order(&listing).chain_id,
        super::wire::ChainId::MaticAmoy
    );
    // Publications bound to the mainnets do not serve this node.
    publish(false).await;
    let (status, body) = get(app(), "/v1/orders?status=sold").await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("nft-index")),
        "{body}"
    );
    own.drop().await;
}

#[tokio::test]
async fn shop_feed_totals_survive_exact_and_past_end_offsets() {
    use super::shop::*;
    use sqlx::Row;
    let Some(db) = crate::test_db::OwnDatabase::create("shop_page_totals").await else {
        return;
    };
    super::trade_events::tests::publish_cursors(&db.pool).await;
    // Prices, names, timestamps, relevance and discount each induce a different order.
    let fixture = tag();
    for (id, name, price, created) in [
        (1, "Hat", 10, 2),
        (2, "Hat Blue", 30, 3),
        (3, "A Hat", 20, 1),
    ] {
        let contract = contract_of(&fixture, id);
        seed_item_row(&db.pool, &contract, &id.to_string(), CREATOR).await;
        sqlx::query("UPDATE shop.items SET name = $2, search_is_collection_approved = true WHERE network = 'matic' AND contract_address = $1")
            .bind(&contract).bind(name).execute(&db.pool).await.unwrap();
        if id == 3 {
            sqlx::query("INSERT INTO shop.coupons (network, chain_id, signer, signature, hashed_signature, state_key, coupon_manager, coupon_address, checks,
                discount_type, discount_ppm, root, collections, effective_since, expires_at)
                VALUES ('MATIC', 137, $1, $2, $2, $2, '0x655fdfa91d69ea49f4ce1a8f7f7e2622c8630813', '0xc914507fe297b2dddd1232ac3a8903f1c125e794',
                    '{\"uses\": 10}', 1, 100000, '0x00', ARRAY[$3], now() - interval '1 day', now() + interval '1 day')")
                .bind(CREATOR).bind(format!("0x{}", tag())).bind(&contract).execute(&db.pool).await.unwrap();
        }
        for asset_type in [1, 2] {
            let (trade, _) = insert_trade(
                &db.pool,
                "public_item_order",
                CREATOR,
                Asset {
                    asset_type: 4,
                    contract: contract.clone(),
                    value: id.to_string(),
                },
                Asset {
                    asset_type,
                    contract: contract.clone(),
                    value: format!("{price}000000000000000000"),
                },
            )
            .await;
            sqlx::query("UPDATE shop.trades SET created_at = timestamp '2026-01-01' + $2 * interval '1 day' WHERE id = $1::uuid")
                .bind(trade).bind(f64::from(created)).execute(&db.pool).await.unwrap();
        }
    }
    let rate = crate::types::ManaUsdRate::parse_usd_per_mana("1").unwrap();
    for feed in ["shop", "legacy", "unified listings", "unified items"] {
        for (sort, expected) in [
            (ShopSortBy::Newest, ["2", "1", "3"]),
            (ShopSortBy::Cheapest, ["1", "3", "2"]),
            (ShopSortBy::MostExpensive, ["2", "3", "1"]),
            (ShopSortBy::Name, ["3", "1", "2"]),
            (ShopSortBy::Discount, ["3", "2", "1"]),
            (ShopSortBy::Relevance, ["1", "2", "3"]),
        ] {
            // The legacy catalog only supports date, price and name ordering.
            let expected = if feed == "legacy"
                && matches!(sort, ShopSortBy::Discount | ShopSortBy::Relevance)
            {
                ["2", "1", "3"]
            } else {
                expected
            };
            for skip in [0, 2, 3, 1000] {
                let base = ShopCatalogFilters {
                    first: Some(2),
                    skip: Some(skip),
                    search: Some("Hat".into()),
                    sort_by: Some(sort),
                    ..Default::default()
                };
                let unified = UnifiedCatalogFilters {
                    base: base.clone(),
                    source: Some(UnifiedSource::Native),
                    ..Default::default()
                };
                let b = match feed {
                    "shop" => build_shop_listings_sql(&base, FeedClock::unbounded()),
                    "legacy" => build_legacy_listings_sql(
                        &LegacyCatalogFilters {
                            first: base.first,
                            skip: base.skip,
                            search: base.search,
                            sort_by: base.sort_by,
                            ..Default::default()
                        },
                        FeedClock::unbounded(),
                    ),
                    "unified listings" => {
                        build_unified_listings_sql(&unified, rate, FeedClock::unbounded())
                    }
                    "unified items" => {
                        build_unified_items_sql(&unified, rate, FeedClock::unbounded())
                    }
                    _ => unreachable!(),
                };
                let rows = sqlx::query_with(sqlx::AssertSqlSafe(b.sql), b.args)
                    .fetch_all(&db.pool)
                    .await
                    .unwrap();
                let label = format!("{feed} {sort:?} skip={skip}");
                assert!(!rows.is_empty(), "{label}: missing total");
                for row in &rows {
                    assert_eq!(row.get::<i64, _>("total"), 3, "{label}");
                }
                if skip >= 3 {
                    assert_eq!(rows.len(), 1, "{label}");
                    assert_eq!(
                        rows[0].get::<Option<bool>, _>("page_present"),
                        None,
                        "{label}"
                    );
                } else {
                    let ids: Vec<&str> = rows
                        .iter()
                        .map(|row| {
                            assert_eq!(
                                row.get::<Option<bool>, _>("page_present"),
                                Some(true),
                                "{label}"
                            );
                            row.get("item_id")
                        })
                        .collect();
                    let start = skip as usize;
                    assert_eq!(ids, expected[start..(start + 2).min(3)], "{label}");
                }
            }
        }
    }
    db.drop().await;
}

/// With Polygon taken by the collections index, the mirror skips it (no
/// page read, no row written) and still walks Ethereum. A hand-back returns
/// Polygon to the mirror: strict reads wait until its next complete passes,
/// which leave only the mirror's rows.
#[tokio::test]
async fn the_mirror_walks_ethereum_while_the_index_owns_polygon_then_takes_polygon_back() {
    use super::chain_walk::ChainNet;
    use super::collections_projection;
    let (matic, eth) = (FakeGraph::new(), FakeGraph::new());
    let (matic_url, eth_url) = (
        serve_graph(matic.clone()).await,
        serve_graph(eth.clone()).await,
    );
    let Some(own) = super::tests::support::OwnDb::create("collections-takeover").await else {
        return;
    };
    let pool = own.pool.clone();
    let state = Arc::new(super::MarketplaceState::new(
        pool.clone(),
        crate::http::Outbound::for_tests(),
        market_settings(&matic_url, &eth_url, false),
    ));
    state.test_allow_loopback.store(true, Ordering::Relaxed);
    let t = tag();
    let (mirrored, native, ethereum) = (contract_of(&t, 1), contract_of(&t, 2), contract_of(&t, 3));
    matic.set(
        "collections",
        vec![mirrors::collections::fixture(
            &mirrored, "Mirrored", CREATOR, true,
        )],
    );
    matic.set(
        "items",
        vec![mirrors::items::fixture(
            &mirrored,
            "0",
            "wearable_v2",
            "Hat",
        )],
    );
    eth.set(
        "collections",
        vec![mirrors::collections::fixture(
            &ethereum, "Ethereum", CREATOR, true,
        )],
    );
    eth.set(
        "items",
        vec![mirrors::items::fixture(
            &ethereum,
            "0",
            "wearable_v2",
            "Mask",
        )],
    );

    // The index has taken Polygon: a row the mirror never had stands there.
    mirrors::collections::upsert(
        &pool,
        super::graph::Network::Matic,
        137,
        false,
        &[mirrors::collections::fixture(
            &native, "Native", CREATOR, true,
        )],
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO shop.collection_projection (network, owned_at, projected_through, generation) \
         VALUES ('matic', timezone('UTC', now()), 100, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let jobs = crate::jobs::Jobs::new();
    let ctx = job_ctx(&jobs).await;
    let tick = || async {
        for mirror in [mirrors::Mirror::Collections, mirrors::Mirror::Items] {
            mirrors::run_tick(&state, mirror, &ctx).await.unwrap();
        }
    };
    let contracts = |network: &'static str| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, String>(
                "SELECT contract_address FROM shop.collections WHERE network = $1 ORDER BY contract_address",
            )
            .bind(network)
            .fetch_all(&pool)
            .await
            .unwrap()
        }
    };
    tick().await;
    assert!(
        matic.requests.lock().unwrap().is_empty(),
        "Polygon's pages are not read"
    );
    assert_eq!(contracts("matic").await, vec![native.clone()]);
    assert_eq!(contracts("ethereum").await, vec![ethereum.clone()]);
    for mirror in [mirrors::Mirror::Collections, mirrors::Mirror::Items] {
        let pass = mirrors::load(&pool, mirror, super::graph::Network::Ethereum)
            .await
            .unwrap()
            .unwrap();
        assert!(pass.last_complete_pass_at.is_some(), "{pass:?}");
    }

    assert!(
        collections_projection::hand_back(&pool, ChainNet::Matic)
            .await
            .unwrap()
    );
    assert_eq!(contracts("matic").await, Vec::<String>::new());
    let err = items_ready(&state).await.unwrap_err();
    assert_eq!(
        err.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "no Polygon pass yet"
    );
    let (owned, generation): (bool, i64) = sqlx::query_as(
        "SELECT owned_at IS NOT NULL, generation FROM shop.collection_projection WHERE network = 'matic'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((owned, generation), (false, 2));

    tick().await;
    assert!(items_ready(&state).await.is_ok());
    assert_eq!(
        contracts("matic").await,
        vec![mirrored.clone()],
        "the mirror's rows alone"
    );
    let (minters, allowances): (Vec<String>, Vec<String>) = sqlx::query_as(
        "SELECT minters, minter_allowances::text[] FROM shop.items WHERE network = 'matic'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(minters.is_empty() && allowances.is_empty());

    // A hand-back repeated after its commit went unconfirmed changes nothing.
    let passes = || async {
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM shop.mirror_state WHERE network = 'matic' AND last_complete_pass_at IS NOT NULL",
        )
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    assert_eq!(passes().await, 2);
    assert!(
        !collections_projection::hand_back(&pool, ChainNet::Matic)
            .await
            .unwrap()
    );
    assert_eq!(contracts("matic").await, vec![mirrored.clone()]);
    assert_eq!(passes().await, 2);
    let generation: i64 = sqlx::query_scalar(
        "SELECT generation FROM shop.collection_projection WHERE network = 'matic'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(generation, 2);
    own.drop().await;
}

/// The collections index owns Polygon, published at block 100.
async fn own_polygon(pool: &PgPool) {
    sqlx::query(
        "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, last_block_hash, \
                full_history, chain_id, folded_block, folded_at) \
         VALUES ('collections', 'matic', 1, 100, $1, true, 137, 100, timezone('UTC', now()))",
    )
    .bind("0xabababababababababababababababababababababababababababababababab")
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO shop.collection_projection (network, owned_at, projected_through, pending, generation) \
         VALUES ('matic', timezone('UTC', now()), 100, 0, 1)",
    )
    .execute(pool)
    .await
    .unwrap();
}

/// Both mirrors' complete passes on `network` now, proven on `chain`.
async fn mirror_passes(pool: &PgPool, network: &str, chain: Option<i64>) {
    sqlx::query(
        "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at, chain_id, marks_item_rows) \
         SELECT m, $1, 'test', timezone('UTC', now()), $2, m = 'items' AND $2 IN (11155111, 80002) \
         FROM unnest(ARRAY['collections', 'items']) m \
         ON CONFLICT (mirror, network) DO UPDATE SET last_complete_pass_at = EXCLUDED.last_complete_pass_at, \
             chain_id = EXCLUDED.chain_id, marks_item_rows = EXCLUDED.marks_item_rows",
    )
    .bind(network)
    .bind(chain)
    .execute(pool)
    .await
    .unwrap();
}

/// The contracts of a `/v1/collections` answer, sorted.
fn collection_contracts(body: &Value) -> Vec<String> {
    let mut out: Vec<String> = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["contractAddress"].as_str().unwrap().to_string())
        .collect();
    out.sort();
    out
}

/// A read returns rows only of networks the gate checked: a network left by
/// an older configuration (no subgraph for it now) still has its mirror
/// pass bound to this node's chain checked, whether the index owns the
/// other network or a mirror serves it. A testnet reader never reads a
/// projection bound to Polygon mainnet.
#[tokio::test]
async fn the_catalogue_reads_only_networks_and_chains_it_checked() {
    let Some(own) = super::tests::support::OwnDb::create("collections-chains").await else {
        return;
    };
    let pool = own.pool.clone();
    let t = tag();
    let (polygon, ethereum) = (contract_of(&t, 1), contract_of(&t, 2));
    for (network, chain, contract) in [
        (super::graph::Network::Matic, 137, &polygon),
        (super::graph::Network::Ethereum, 1, &ethereum),
    ] {
        mirrors::collections::upsert(
            &pool,
            network,
            chain,
            false,
            &[mirrors::collections::fixture(contract, "C", CREATOR, true)],
        )
        .await
        .unwrap();
    }
    let mut both = vec![polygon.clone(), ethereum.clone()];
    both.sort();
    let state_with = |settings: Settings| {
        let state = Arc::new(super::MarketplaceState::new(
            pool.clone(),
            crate::http::Outbound::for_tests(),
            settings,
        ));
        move || market_router(state.clone())
    };
    let only_polygon = Settings {
        eth_collections_subgraph_url: None,
        ..market_settings("http://127.0.0.1:9", "http://127.0.0.1:9", false)
    };

    // The index owns Polygon, the only network with a subgraph; Ethereum's
    // rows were left by an older configuration.
    own_polygon(&pool).await;
    let app = state_with(only_polygon.clone());
    mirror_passes(&pool, "ethereum", Some(11155111)).await;
    let (code, body) = get(app(), "/v1/collections").await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["source"], "collections-mirror");
    mirror_passes(&pool, "ethereum", Some(1)).await;
    let (code, body) = get(app(), "/v1/collections").await;
    assert_eq!(code, StatusCode::OK, "{body}");
    assert_eq!(collection_contracts(&body), both);

    // Nothing owned: an explicit read of the network without a subgraph,
    // beside Polygon's pass.
    sqlx::query("UPDATE shop.collection_projection SET owned_at = NULL")
        .execute(&pool)
        .await
        .unwrap();
    mirror_passes(&pool, "matic", Some(137)).await;
    mirror_passes(&pool, "ethereum", Some(11155111)).await;
    let (code, body) = get(app(), "/v1/collections?network=ETHEREUM").await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    mirror_passes(&pool, "ethereum", Some(1)).await;
    let (code, body) = get(app(), "/v1/collections?network=ETHEREUM").await;
    assert_eq!(code, StatusCode::OK, "{body}");
    assert_eq!(collection_contracts(&body), vec![ethereum.clone()]);

    // A testnet reader: Polygon owned again, its projection bound to 137.
    sqlx::query("UPDATE shop.collection_projection SET owned_at = timezone('UTC', now())")
        .execute(&pool)
        .await
        .unwrap();
    let amoy = state_with(Settings {
        eth_network: "sepolia".into(),
        ..market_settings("http://127.0.0.1:9", "http://127.0.0.1:9", false)
    });
    let (code, body) = get(amoy(), "/v1/collections?network=MATIC").await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["source"], "collections-index");
    own.drop().await;
}

/// The lambdas catalogue never stands in a subgraph for a network the index
/// owns: rewound, it answers the index's envelope; published beside an
/// Ethereum mirror with no pass, the mirror's; neither asks the graph. With
/// nothing owned and the mirrors refusing, the legacy graph path answers.
#[tokio::test]
async fn lambdas_never_read_a_subgraph_for_a_network_the_index_owns() {
    let (matic, eth) = (FakeGraph::new(), FakeGraph::new());
    let (matic_url, eth_url) = (
        serve_graph(matic.clone()).await,
        serve_graph(eth.clone()).await,
    );
    let Some(own) = super::tests::support::OwnDb::create("collections-lambdas").await else {
        return;
    };
    let pool = own.pool.clone();
    let state = Arc::new(super::MarketplaceState::new(
        pool.clone(),
        crate::http::Outbound::for_tests(),
        market_settings(&matic_url, &eth_url, false),
    ));
    state.test_allow_loopback.store(true, Ordering::Relaxed);
    let app = || super::router(state.clone());
    let routes = [
        "/lambdas/collections/wearables?textSearch=hat",
        "/lambdas/collections/emotes?textSearch=hat",
    ];
    let asked = || matic.requests.lock().unwrap().len() + eth.requests.lock().unwrap().len();

    own_polygon(&pool).await;
    mirror_passes(&pool, "ethereum", Some(1)).await;
    mirror_passes(&pool, "matic", Some(137)).await;
    sqlx::query("UPDATE shop.chain_cursor SET folded_block = NULL, folded_at = NULL WHERE indexer = 'collections'")
        .execute(&pool)
        .await
        .unwrap();
    for route in routes {
        let (code, body) = get(app(), route).await;
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{route}: {body}");
        assert_eq!(body["source"], "collections-index", "{route}");
    }
    assert_eq!(asked(), 0, "no graph read for an owned network");

    sqlx::query(
        "UPDATE shop.chain_cursor SET folded_block = 100, folded_at = timezone('UTC', now()) \
         WHERE indexer = 'collections'",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("DELETE FROM shop.mirror_state WHERE network = 'ethereum'")
        .execute(&pool)
        .await
        .unwrap();
    for route in routes {
        let (code, body) = get(app(), route).await;
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{route}: {body}");
        assert_eq!(body["source"], "items-mirror", "{route}");
    }
    assert_eq!(
        asked(),
        0,
        "a healthy owned network is never replaced by the graph"
    );

    // Nothing owned, the mirrors refusing: the legacy graph path.
    sqlx::query("UPDATE shop.collection_projection SET owned_at = NULL")
        .execute(&pool)
        .await
        .unwrap();
    let (code, body) = get(app(), routes[0]).await;
    assert_eq!(code, StatusCode::OK, "{body}");
    assert!(asked() > 0, "the graph answered for the mirrors");
    own.drop().await;
}

/// A catalogue whose Polygon the collections index owns, published now at
/// block 100: one approved Polygon item, both mirrors' passes, local trades,
/// a MANA price, and both subgraph readers on `up` (one sale of the item).
async fn owned_polygon_market(up: &Arc<FakeGraph>) -> Option<(PgPool, SharedState, String)> {
    let pool = crate::db::scratch::isolated_pool(4).await?;
    let url = serve_graph(up.clone()).await;
    let state = Arc::new(super::MarketplaceState::new(
        pool.clone(),
        crate::http::Outbound::for_tests(),
        market_settings(&url, &url, true),
    ));
    state.test_allow_loopback.store(true, Ordering::Relaxed);
    super::trade_events::tests::publish_cursors(&pool).await;
    let polygon = contract_of(&tag(), 1);
    seed_item_row(&pool, &polygon, "0", CREATOR).await;
    up.set(
        "sales",
        vec![json!({
            "id": "sale-1", "searchItemId": "0", "searchContractAddress": polygon,
            "price": "1000000000000000000", "timestamp": "1", "type": "mint",
            "seller": CREATOR, "buyer": WALLET_A,
            "item": { "id": format!("{polygon}-0"), "creator": CREATOR }
        })],
    );
    // On sale in the store, so the trending rail has its row.
    sqlx::query(
        "UPDATE shop.items SET search_is_collection_approved = true, search_is_store_minter = true, \
                price = 1000000000000000000",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at) \
         SELECT m, n, 'test', timezone('UTC', now()) \
         FROM unnest(ARRAY['collections', 'items']) m CROSS JOIN unnest(ARRAY['ethereum', 'matic']) n",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO economy.price_snapshots (source, mana_usd, taken_at) VALUES ('coingecko', 0.25, timezone('UTC', now()))")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, last_block_hash, \
                full_history, chain_id, folded_block, folded_at) \
         VALUES ('collections', 'matic', 1, 100, '0xabababababababababababababababababababababababababababababababab', true, 137, 100, timezone('UTC', now()))",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO shop.collection_projection (network, owned_at, projected_through, pending, generation) \
         VALUES ('matic', timezone('UTC', now()), 100, 0, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    Some((pool, state, polygon))
}

/// A warm sales memo can supply ranking inputs even when the graph stalls.
/// An unpublished catalogue refuses before asking the stalled sales reader.
#[tokio::test]
async fn a_stalled_sales_read_never_masks_the_gate() {
    use super::readers::Ttl;
    let up = FakeGraph::new();
    let Some((pool, state, _)) = owned_polygon_market(&up).await else {
        return;
    };
    let within = |uri: &'static str| {
        let app = market_router(state.clone());
        async move {
            tokio::time::timeout(Duration::from_secs(3), get(app, uri))
                .await
                .unwrap_or_else(|_| panic!("{uri} waited on the stalled subgraph"))
        }
    };
    let trend = "/v3/catalog/trending?first=3";
    let (status, page) = within(trend).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(page["data"].as_array().map(Vec::len), Some(1), "{page}");
    up.hang.store(true, Ordering::SeqCst);
    let (status, body) = within(trend).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"], page["data"]);
    state.readers.forget_all(Ttl::M10);
    sqlx::query("UPDATE shop.chain_cursor SET folded_block = NULL, folded_at = NULL WHERE indexer = 'collections'")
        .execute(&pool)
        .await
        .unwrap();
    for uri in [
        "/v1/trendings",
        "/v3/catalog/trending?first=3",
        "/v3/catalog/creators",
        "/v3/catalog/suggested?first=2",
        "/v3/catalog/suggest?search=item",
        "/v1/catalog?first=1",
    ] {
        let (status, body) = within(uri).await;
        assert_eq!(
            (status, body["source"].as_str()),
            (StatusCode::SERVICE_UNAVAILABLE, Some("collections-index")),
            "{uri}: {body}"
        );
    }
    drop(state);
    pool.close().await;
}

/// Requests that wait on another's memo fetch hold no connection. A lock on
/// a table only the fetch reads (the creator directory's deployments, read by
/// the creator search and the search suggest) holds the
/// leader; four identical cold requests wait on it, and another catalogue
/// route still gets a connection on a pool of two. Once the lock goes, every
/// waiter answers. A burst of eight identical cold `/v1/catalog` pages (whose
/// query reads only tables the gate reads too, so no lock can hold it alone)
/// all answer on the same pool.
#[tokio::test]
async fn memo_waiters_hold_no_connection() {
    let up = FakeGraph::new();
    let Some((pool, wide, _)) = owned_polygon_market(&up).await else {
        return;
    };
    let mut settings = wide.settings.clone();
    settings.market.catalog_cache_ttl = Duration::from_secs(60);
    let narrow = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(2))
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap();
    let state = Arc::new(super::MarketplaceState::new(
        narrow.clone(),
        crate::http::Outbound::for_tests(),
        settings,
    ));
    state.test_allow_loopback.store(true, Ordering::Relaxed);
    for (table, uri) in [
        (
            "content.deployments",
            "/v3/catalog/creators/search?search=cc",
        ),
        ("content.deployments", "/v3/catalog/suggest?search=item"),
    ] {
        state.readers.clear();
        state.catalog_cache.invalidate();
        let mut lock = pool.begin().await.unwrap();
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "LOCK TABLE {table} IN ACCESS EXCLUSIVE MODE"
        )))
        .execute(&mut *lock)
        .await
        .unwrap();
        let waiting: Vec<_> = (0..4)
            .map(|_| {
                let state = state.clone();
                tokio::spawn(async move { get(market_router(state), uri).await })
            })
            .collect();
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(
            waiting.iter().all(|t| !t.is_finished()),
            "{uri}: the fetch waits on the lock"
        );
        let (status, body) = tokio::time::timeout(
            Duration::from_secs(10),
            get(market_router(state.clone()), "/v1/collections?first=1"),
        )
        .await
        .unwrap_or_else(|_| panic!("/v1/collections waited behind {uri}'s memo"));
        assert_eq!(status, StatusCode::OK, "beside {uri}: {body}");
        lock.rollback().await.unwrap();
        let mut bodies = Vec::new();
        for task in waiting {
            let (status, body) = task.await.unwrap();
            assert_eq!(status, StatusCode::OK, "{uri}: {body}");
            bodies.push(body);
        }
        assert!(bodies.windows(2).all(|w| w[0] == w[1]), "{uri}: one page");
    }
    state.catalog_cache.invalidate();
    let burst = futures::future::join_all(
        (0..8).map(|_| get(market_router(state.clone()), "/v1/catalog?first=2")),
    )
    .await;
    for (status, body) in &burst {
        assert_eq!(*status, StatusCode::OK, "{body}");
        assert_eq!(body, &burst[0].1, "one page for the burst");
    }
    drop((state, wide));
    narrow.close().await;
    pool.close().await;
}

/// A suggested page is read on one snapshot. A full personal page answers
/// from its first read without the sales reader; one that falls back to the
/// trending rail rebuilds its personal rows on the read it opens after the
/// sales call, so a publication committed meanwhile never mixes into it.
#[tokio::test]
async fn a_suggested_page_never_mixes_two_publications() {
    let up = FakeGraph::new();
    let Some((pool, state, c)) = owned_polygon_market(&up).await else {
        return;
    };
    for i in 1..=4 {
        seed_item_row(&pool, &c, &i.to_string(), CREATOR).await;
    }
    sqlx::query(
        "UPDATE shop.items SET name = 'before', search_is_collection_approved = true, \
                search_is_store_minter = true, price = 1000000000000000000",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO shop.item_neighbors_meta (id, built_at, algorithm, wallets, items, rows_written) VALUES (1, now(), 'test', 10, 5, 4)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO shop.item_neighbors (network, contract_address, item_id, neighbor_network, \
                neighbor_contract_address, neighbor_item_id, sim, support, rank) \
         SELECT 'matic', $1, 0, 'matic', $1, g, 0.9, 5, g FROM generate_series(1, 4) g",
    )
    .bind(&c)
    .execute(&pool)
    .await
    .unwrap();
    let asked = || up.requests.lock().unwrap().len();

    // Four personal rows fill a page of four: no sales call.
    let before = asked();
    let (status, body) = get(
        market_router(state.clone()),
        &format!("/v3/catalog/suggested?first=4&seeds={c}-0"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"].as_array().map(Vec::len), Some(4), "{body}");
    assert_eq!(asked(), before, "a full personal page reads no sales");

    // A page of five falls back to the trending rail; the catalogue is
    // republished while its sales call is held.
    up.pause.store(true, Ordering::SeqCst);
    let uri = format!("/v3/catalog/suggested?first=5&seeds={c}-0");
    let task = {
        let state = state.clone();
        tokio::spawn(async move { get(market_router(state), &uri).await })
    };
    tokio::time::timeout(Duration::from_secs(15), up.arrived.notified())
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("UPDATE shop.items SET name = 'after', price = 2000000000000000000")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE shop.collection_projection SET generation = generation + 1 WHERE network = 'matic'",
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    up.release.notify_one();
    let (status, body) = task.await.unwrap();
    assert_eq!(status, StatusCode::OK, "{body}");
    let rows: Vec<(Value, Value)> = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["name"].clone(), r["reason"]["kind"].clone()))
        .collect();
    assert_eq!(rows.len(), 5, "{body}");
    assert!(
        rows.iter().all(|(name, _)| name == "after"),
        "one publication: {rows:?}"
    );
    assert!(
        rows.iter().any(|(_, kind)| kind == "trending")
            && rows.iter().any(|(_, kind)| kind != "trending"),
        "{rows:?}"
    );
    assert_eq!(body["personalized"], true);
    drop(state);
    pool.close().await;
}

/// A page is stamped with the copy it was read from: a trending page read on
/// the snapshot opened after its sales call carries that publication's
/// `as_of`, not the one the discarded preflight read saw.
#[tokio::test]
async fn a_page_is_stamped_with_the_copy_it_was_read_from() {
    let up = FakeGraph::new();
    let Some((pool, state, _)) = owned_polygon_market(&up).await else {
        return;
    };
    let old: chrono::NaiveDateTime = sqlx::query_scalar(
        "UPDATE shop.chain_cursor SET folded_at = date_trunc('second', timezone('UTC', now())) - interval '10 minutes' \
         WHERE network = 'matic' AND indexer = 'collections' RETURNING folded_at",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    up.pause.store(true, Ordering::SeqCst);
    let task = {
        let state = state.clone();
        tokio::spawn(async move { get(market_router(state), "/v3/catalog/trending?first=3").await })
    };
    tokio::time::timeout(Duration::from_secs(15), up.arrived.notified())
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("UPDATE shop.items SET name = 'after'")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE shop.collection_projection SET generation = generation + 1 WHERE network = 'matic'",
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query("UPDATE shop.chain_cursor SET folded_at = timezone('UTC', now()) WHERE network = 'matic' AND indexer = 'collections'")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    up.release.notify_one();
    let (status, body) = task.await.unwrap();
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"][0]["name"], "after", "{body}");
    // The final read's copies: Ethereum's items pass and Polygon's new
    // publication; the oldest stamps the page.
    let expected: chrono::NaiveDateTime = sqlx::query_scalar(
        "SELECT least( \
           (SELECT last_complete_pass_at FROM shop.mirror_state WHERE mirror = 'items' AND network = 'ethereum'), \
           (SELECT folded_at FROM shop.chain_cursor WHERE indexer = 'collections' AND network = 'matic'))",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let stamp = chrono::DateTime::parse_from_rfc3339(body["as_of"].as_str().unwrap())
        .unwrap()
        .timestamp_millis();
    assert_ne!(
        stamp,
        old.and_utc().timestamp_millis(),
        "not the preflight's copy"
    );
    assert_eq!(stamp, expected.and_utc().timestamp_millis(), "{body}");
    drop(state);
    pool.close().await;
}

/// The shop's priced and descriptive routes and the import check follow an
/// owned network's publication: all answer a fresh one; a publication older
/// than the money window refuses the priced and authorizing routes (503
/// `collections-index`) while the descriptive ones answer; a rewind refuses
/// them all. (The lambdas catalogue's owned-network case is
/// `lambdas_never_read_a_subgraph_for_a_network_the_index_owns`; credits,
/// campaign tags and the favourites job go through the same gate.) The creator directory is filed
/// under the sources that serve each network, so a hand-back files it anew.
#[tokio::test]
async fn every_gated_route_follows_an_owned_networks_publication() {
    use super::readers::Ttl;
    let up = FakeGraph::new();
    let Some((pool, state, polygon)) = owned_polygon_market(&up).await else {
        return;
    };
    let app = || market_router(state.clone());
    let priced = [
        "/v1/catalog?first=1".to_string(),
        "/v2/catalog?first=1".to_string(),
        "/v1/items?first=1".to_string(),
        "/v3/catalog/items?first=1".to_string(),
        format!("/v1/prices?contractAddress={polygon}&category=wearable"),
        "/v3/catalog/shop".to_string(),
        "/v3/catalog/legacy".to_string(),
        "/v3/catalog/unified?groupBy=item".to_string(),
        format!("/v3/catalog/importable?seller={CREATOR}"),
        "/v3/catalog/suggest?search=item".to_string(),
        "/v1/trendings".to_string(),
        "/v3/catalog/trending?first=3".to_string(),
        "/v3/catalog/suggested?first=2".to_string(),
        "/v3/catalog/creators".to_string(),
    ];
    let descriptive = [
        "/v1/collections".to_string(),
        "/v1/contracts?first=10".to_string(),
        "/v3/catalog/creators/search?search=cc".to_string(),
    ];
    let import = || async {
        let path = crate::import::CHECK_PATH;
        call(
            app(),
            "POST",
            path,
            signed_headers("post", path, "{}", true).await,
            Some(json!({ "chainId": 137, "contractAddress": polygon, "itemId": "0" })),
        )
        .await
    };
    let index = (
        StatusCode::SERVICE_UNAVAILABLE,
        Some("collections-index".to_string()),
    );
    let refused =
        |status: StatusCode, body: &Value| (status, body["source"].as_str().map(str::to_string));
    for uri in priced.iter().chain(&descriptive) {
        let (status, body) = get(app(), uri).await;
        assert_eq!(status, StatusCode::OK, "a fresh publication: {uri}: {body}");
    }
    let (status, body) = import().await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a fresh publication: import: {body}"
    );

    sqlx::query("UPDATE shop.chain_cursor SET folded_at = timezone('UTC', now()) - interval '2 hours' WHERE indexer = 'collections'")
        .execute(&pool)
        .await
        .unwrap();
    for uri in &priced {
        let (status, body) = get(app(), uri).await;
        assert_eq!(
            refused(status, &body),
            index,
            "an old publication: {uri}: {body}"
        );
    }
    let (status, body) = import().await;
    assert_eq!(
        refused(status, &body),
        index,
        "an old publication: import: {body}"
    );
    for uri in &descriptive {
        let (status, body) = get(app(), uri).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "a description of an old publication: {uri}: {body}"
        );
    }

    sqlx::query("UPDATE shop.chain_cursor SET folded_block = NULL, folded_at = NULL WHERE indexer = 'collections'")
        .execute(&pool)
        .await
        .unwrap();
    for uri in priced.iter().chain(&descriptive) {
        let (status, body) = get(app(), uri).await;
        assert_eq!(refused(status, &body), index, "a rewind: {uri}: {body}");
    }
    let (status, body) = import().await;
    assert_eq!(refused(status, &body), index, "a rewind: import: {body}");

    // The creator directory: filed under the index serving Polygon, then
    // anew under the mirrors once Polygon is handed back.
    sqlx::query("UPDATE shop.chain_cursor SET folded_block = 100, folded_at = timezone('UTC', now()) WHERE indexer = 'collections'")
        .execute(&pool)
        .await
        .unwrap();
    let filed = |sources: &'static str| {
        let state = state.clone();
        async move {
            let key = super::search::directory_key(sources);
            tokio::time::timeout(Duration::from_secs(20), async {
                while !state.readers.holds(Ttl::M15, &key) {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            })
            .await
            .is_ok()
        }
    };
    let (status, body) = get(app(), "/v3/catalog/unified?groupBy=item&first=3").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(filed("mirror|index:matic").await);
    assert!(
        !state
            .readers
            .holds(Ttl::M15, &super::search::directory_key("mirror"))
    );
    assert!(
        super::collections_projection::hand_back(&pool, super::chain_walk::ChainNet::Matic)
            .await
            .unwrap()
    );
    sqlx::query(
        "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at) \
         SELECT m, 'matic', 'test', timezone('UTC', now()) FROM unnest(ARRAY['collections', 'items']) m \
         ON CONFLICT (mirror, network) DO UPDATE SET last_complete_pass_at = EXCLUDED.last_complete_pass_at",
    )
    .execute(&pool)
    .await
    .unwrap();
    let (status, body) = get(app(), "/v3/catalog/unified?groupBy=item&first=3").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        filed("mirror").await,
        "an ownership change files the directory anew"
    );
    drop(state);
    pool.close().await;
}

/// A catalogue route answers owned Polygon from the index's publication and
/// Ethereum from the mirror in one read: a rewind on Polygon makes it answer
/// the index's envelope, its republication answers again, and after a
/// hand-back it follows the mirror, which has no Polygon pass yet.
#[tokio::test]
async fn a_route_reads_owned_polygon_beside_mirrored_ethereum() {
    let Some(own) = super::tests::support::OwnDb::create("collections-readers").await else {
        return;
    };
    let pool = own.pool.clone();
    let state = Arc::new(super::MarketplaceState::new(
        pool.clone(),
        crate::http::Outbound::for_tests(),
        market_settings("http://127.0.0.1:9", "http://127.0.0.1:9", false),
    ));
    let t = tag();
    let (polygon, ethereum) = (contract_of(&t, 1), contract_of(&t, 2));
    for (network, chain, contract) in [
        (super::graph::Network::Matic, 137, &polygon),
        (super::graph::Network::Ethereum, 1, &ethereum),
    ] {
        mirrors::collections::upsert(
            &pool,
            network,
            chain,
            false,
            &[mirrors::collections::fixture(contract, "C", CREATOR, true)],
        )
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at) \
         SELECT m, n, 'test', timezone('UTC', now()) \
         FROM unnest(ARRAY['collections', 'items']) m CROSS JOIN unnest(ARRAY['ethereum', 'matic']) n",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, last_block_hash, \
                full_history, chain_id, folded_block, folded_at) \
         VALUES ('collections', 'matic', 1, 100, '0xabababababababababababababababababababababababababababababababab', true, 137, 100, timezone('UTC', now()))",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO shop.collection_projection (network, owned_at, projected_through, pending, generation) \
         VALUES ('matic', timezone('UTC', now()), 100, 0, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let app = || market_router(state.clone());
    let contracts = |body: &Value| -> Vec<String> {
        let mut out: Vec<String> = body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["contractAddress"].as_str().unwrap().to_string())
            .collect();
        out.sort();
        out
    };
    let mut both = vec![polygon.clone(), ethereum.clone()];
    both.sort();

    let (code, body) = get(app(), "/v1/collections").await;
    assert_eq!(code, StatusCode::OK, "{body}");
    assert_eq!(contracts(&body), both);

    sqlx::query("UPDATE shop.chain_cursor SET folded_block = NULL, folded_at = NULL WHERE indexer = 'collections'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE shop.collection_projection SET projected_through = NULL WHERE network = 'matic'",
    )
    .execute(&pool)
    .await
    .unwrap();
    let (code, body) = get(app(), "/v1/collections").await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["source"], "collections-index");
    let (code, _) = get(app(), "/v1/collections?network=ETHEREUM").await;
    assert_eq!(
        code,
        StatusCode::OK,
        "a read of Ethereum alone does not wait on Polygon's index"
    );

    sqlx::query(
        "UPDATE shop.chain_cursor SET folded_block = 100, folded_at = timezone('UTC', now()) \
         WHERE indexer = 'collections'",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE shop.collection_projection SET projected_through = 100 WHERE network = 'matic'",
    )
    .execute(&pool)
    .await
    .unwrap();
    let (code, body) = get(app(), "/v1/collections").await;
    assert_eq!(code, StatusCode::OK, "{body}");
    assert_eq!(contracts(&body), both);

    assert!(
        super::collections_projection::hand_back(&pool, super::chain_walk::ChainNet::Matic)
            .await
            .unwrap()
    );
    let (code, body) = get(app(), "/v1/collections").await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["source"], "collections-mirror");
    own.drop().await;
}

#[tokio::test]
async fn land_pages_read_the_published_catalogue_and_enrich_the_same_snapshot() {
    use crate::land_authz::LAND_MAINNET;
    let Some(db) = super::tests::support::OwnDb::create("land_nft_pages").await else {
        return;
    };
    let names = super::nft_index::DCL_REGISTRAR;
    let graph = FakeGraph::new();
    let url = serve_graph(graph.clone()).await;
    let app = |network: &str, walker: bool, trades_upstream: Option<String>| {
        let mut settings = Settings::for_tests();
        settings.eth_network = network.into();
        settings.market.indexes.nft_enabled = walker;
        settings.trades_sync_upstream_url = trades_upstream;
        settings.marketplace_subgraph_url = Some(url.clone());
        let state = super::MarketplaceState::new(
            db.pool.clone(),
            crate::http::Outbound::for_tests(),
            settings,
        );
        state.test_allow_loopback.store(true, Ordering::Relaxed);
        market_router(Arc::new(state))
    };
    let walker = app("mainnet", true, None);
    let land = LAND_MAINNET.land_registry;
    let estate = LAND_MAINNET.estate_registry;
    let parcel = (alloy_primitives::U256::from((-102_i128) as u128) << 128_usize)
        | alloy_primitives::U256::from((-10_i128) as u128);
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "INSERT INTO land_authz.index_cursor(network,land_registry,estate_registry,last_block,folded_block,folded_at,full_replay_started)
         VALUES ('ethereum-mainnet','{land}','{estate}',20,20,timezone('UTC',now()),true);
         INSERT INTO shop.chain_cursor(indexer,network,start_block,last_block,folded_block,folded_at,full_history,chain_id)
         VALUES ('nft','ethereum',0,20,20,timezone('UTC',now()),true,1), ('nft','matic',0,20,20,timezone('UTC',now()),true,137), ('rentals','ethereum',0,20,20,timezone('UTC',now()),true,1);
         INSERT INTO shop.mirror_state(mirror,network,source_url,last_complete_pass_at)
         VALUES ('items','ethereum','test',now()),('items','matic','test',now());
         INSERT INTO land_authz.token_right(token_address,token_id,owner,x,y,updated_block,updated_log)
         VALUES ('{land}',{parcel},'{WALLET_A}',-102,-10,10,0),('{estate}',42,'{WALLET_A}',NULL,NULL,11,1);
         INSERT INTO land_authz.estate_land VALUES({parcel},42,11,2);
         INSERT INTO shop.nft(network,contract_address,token_id,owner,ens_subdomain,minted_block,minted_at,transferred_block,transferred_log,transferred_at)
         VALUES('ethereum','{names}',9,'{WALLET_A}','alice',1,1,1,0,1);
         INSERT INTO shop.land_data VALUES('{estate}',42,'North estate','Two words',12,1);
         INSERT INTO shop.nft_order(network,contract_address,token_id,order_id,marketplace_address,status,seller,price,expires_at,created_at,updated_at,updated_block,updated_log)
         VALUES('ethereum','{estate}',42,'order-42','{CREATOR}','open','{WALLET_A}',25,4102444800000,1,2,13,0)"
    ))).execute(&db.pool).await.unwrap();
    let rental = crate::rentals::index::tests::seed_listing(
        &db.pool, estate, "42", ["0"; 3], "0x42", WALLET_A, CREATOR,
    )
    .await;
    let (code, result) = get(
        walker.clone(),
        &format!("/v1/nfts?contractAddress={estate}&tokenId=42"),
    )
    .await;
    assert_eq!(code, StatusCode::OK, "{result}");
    assert_eq!(result["total"], 1);
    let item = &result["data"][0];
    assert_eq!(item["nft"]["name"], "North estate");
    assert_eq!(item["nft"]["owner"], WALLET_A);
    assert_eq!(item["nft"]["data"]["estate"]["size"], 1);
    assert_eq!(
        item["nft"]["data"]["estate"]["parcels"],
        json!([{"x":-102,"y":-10}])
    );
    assert_eq!(item["order"]["id"], "order-42");
    assert_eq!(item["rental"]["id"], rental.to_string());
    let (code, result) = get(walker.clone(), "/v1/nfts?isLand=true&isOnRent=true").await;
    assert_eq!(code, StatusCode::OK, "{result}");
    assert_eq!(result["total"], 1);
    assert_eq!(result["data"][0]["nft"]["tokenId"], "42");
    let (code, result) = get(
        walker.clone(),
        "/v1/nfts?category=parcel&minDistanceToPlaza=9&maxDistanceToPlaza=9",
    )
    .await;
    assert_eq!(code, StatusCode::OK, "{result}");
    assert_eq!(result["data"][0]["nft"]["tokenId"], parcel.to_string());
    let (code, result) = get(
        walker.clone(),
        &format!("/v1/nfts?owner={WALLET_A}&contractAddress={estate}&contractAddress={names}"),
    )
    .await;
    assert_eq!(code, StatusCode::OK, "{result}");
    assert_eq!(result["total"], 2);
    assert!(
        result["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["nft"]["name"] == "alice")
    );
    assert!(
        result["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["nft"]["name"] == "North estate")
    );
    for skip in [0, 1, 5001] {
        let (code, result) = get(
            walker.clone(),
            &format!("/v1/nfts?owner={WALLET_A}&first=1&skip={skip}"),
        )
        .await;
        assert_eq!(code, StatusCode::OK, "{result}");
        assert_eq!(result["total"], 3);
        assert_eq!(
            result["data"].as_array().unwrap().len(),
            usize::from(skip < 2)
        );
    }

    let unavailable_trades = app("mainnet", true, Some(url.clone()));
    for query in [
        "isOnSale=true".to_owned(),
        format!("contractAddress={land}&tokenId={parcel}"),
    ] {
        let (code, result) = get(unavailable_trades.clone(), &format!("/v1/nfts?{query}")).await;
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{result}");
        assert_eq!(result["source"], "trades-mirror");
    }
    let (code, result) = get(unavailable_trades, "/v1/nfts?first=24").await;
    assert_eq!(code, StatusCode::OK, "{result}");
    assert_eq!(result["total"], 3);
    // The node's own trades are readable (no upstream, writes on), but the
    // trade-events verdicts are not published: both readers of a page's v3
    // orders on the LAND snapshot answer that index's envelope instead of
    // dropping the orders; once it publishes on both networks they serve.
    let local_trades = {
        let mut settings = Settings::for_tests();
        settings.eth_network = "mainnet".into();
        settings.market.indexes.nft_enabled = true;
        settings.data_writes_enabled = true;
        settings.marketplace_subgraph_url = Some(url.clone());
        let state = super::MarketplaceState::new(
            db.pool.clone(),
            crate::http::Outbound::for_tests(),
            settings,
        );
        state.test_allow_loopback.store(true, Ordering::Relaxed);
        market_router(Arc::new(state))
    };
    let order_reads = [
        "isOnSale=true".to_owned(),
        format!("contractAddress={land}&tokenId={parcel}"),
    ];
    for query in &order_reads {
        let (code, result) = get(local_trades.clone(), &format!("/v1/nfts?{query}")).await;
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{query}: {result}");
        assert_eq!(result["source"], "trade-events-index", "{query}");
    }
    super::trade_events::tests::publish_cursors(&db.pool).await;
    for query in &order_reads {
        let (code, result) = get(local_trades.clone(), &format!("/v1/nfts?{query}")).await;
        assert_eq!(code, StatusCode::OK, "{query}: {result}");
    }
    let mixed_contracts = format!("/v1/nfts?contractAddress={estate}&contractAddress={WALLET_B}");
    let (code, result) = get(walker.clone(), &mixed_contracts).await;
    assert_eq!(code, StatusCode::OK, "{result}");
    assert_eq!(result["total"], 1);
    sqlx::query("UPDATE shop.chain_cursor SET folded_block=NULL WHERE network='matic'")
        .execute(&db.pool)
        .await
        .unwrap();
    let (code, result) = get(walker.clone(), &mixed_contracts).await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{result}");
    assert_eq!(result["source"], "nft-index");
    sqlx::query("UPDATE shop.chain_cursor SET folded_block=last_block WHERE network='matic'")
        .execute(&db.pool)
        .await
        .unwrap();

    // A missing rental table must not manufacture an empty rental result.
    sqlx::query("ALTER TABLE signatures.rental_periods RENAME TO held_periods")
        .execute(&db.pool)
        .await
        .unwrap();
    for query in ["category=estate", "category=estate&isOnRent=true"] {
        let (code, _) = get(walker.clone(), &format!("/v1/nfts?{query}")).await;
        assert_eq!(code, StatusCode::INTERNAL_SERVER_ERROR);
    }
    sqlx::query("ALTER TABLE signatures.held_periods RENAME TO rental_periods")
        .execute(&db.pool)
        .await
        .unwrap();

    // Hold the final rental lookup after the LAND/NFT snapshot is established.
    let mut writer = db.pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE signatures.rentals IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *writer)
        .await
        .unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *writer)
        .await
        .unwrap();
    let request = tokio::spawn(get(walker.clone(), "/v1/nfts?category=estate"));
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let waiting: bool = sqlx::query_scalar("SELECT EXISTS(SELECT FROM pg_stat_activity WHERE datname=current_database() AND $1=ANY(pg_blocking_pids(pid)) AND query LIKE '%FROM signatures.rentals%')")
                .bind(pid).fetch_one(&db.pool).await.unwrap();
            if waiting { break; }
            tokio::task::yield_now().await;
        }
    }).await.expect("page reached the rental enrichment");
    sqlx::query("UPDATE signatures.rentals SET status='cancelled' WHERE id=$1")
        .bind(rental)
        .execute(&mut *writer)
        .await
        .unwrap();
    sqlx::query("UPDATE land_authz.index_cursor SET folded_block=NULL")
        .execute(&mut *writer)
        .await
        .unwrap();
    writer.commit().await.unwrap();
    let (code, result) = request.await.unwrap();
    assert_eq!(code, StatusCode::OK, "{result}");
    assert_eq!(result["data"][0]["rental"]["id"], rental.to_string());
    for query in [
        format!("owner={WALLET_A}"),
        "first=24".into(),
        "category=estate&isOnRent=true".into(),
    ] {
        let (code, result) = get(walker.clone(), &format!("/v1/nfts?{query}")).await;
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{result}");
        assert_eq!(result["source"], "land-authz-index");
    }
    sqlx::query("UPDATE land_authz.index_cursor SET folded_block=last_block")
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE shop.chain_cursor SET folded_block=NULL WHERE network='ethereum'")
        .execute(&db.pool)
        .await
        .unwrap();
    let (code, result) = get(walker.clone(), "/v1/nfts?category=parcel").await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{result}");
    assert_eq!(result["source"], "nft-index");
    assert!(graph.requests.lock().unwrap().is_empty());

    for (network, enabled) in [("mainnet", false), ("sepolia", true)] {
        let (code, result) = get(app(network, enabled, None), "/v1/nfts?category=parcel").await;
        assert_eq!(code, StatusCode::OK, "{result}");
        assert_eq!(result["total"], 0);
        assert!(!std::mem::take(&mut *graph.requests.lock().unwrap()).is_empty());
    }
    drop(walker);
    db.drop().await;
}

#[tokio::test]
async fn testnet_collection_and_name_pages_bind_the_mirrored_rows_and_registrar() {
    use super::nft_index::Catalogue;
    use crate::chain_index::names::Purpose;

    let Some(db) = super::tests::support::OwnDb::create("testnet_catalogue").await else {
        return;
    };
    let mut settings = Settings::for_tests();
    settings.eth_network = "sepolia".into();
    settings.market.indexes.nft_enabled = true;
    settings.market.indexes.chain_id_ethereum = 11_155_111;
    settings.market.indexes.chain_id_matic = 80_002;
    let state = Arc::new(super::MarketplaceState::new(
        db.pool.clone(),
        crate::http::Outbound::for_tests(),
        settings,
    ));
    let app = market_router(state);
    let registrar = Catalogue::Testnet.registrar();
    let wearable = "/v1/nfts?category=wearable&network=MATIC";
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,folded_block,folded_at,full_history)
      VALUES('nft','ethereum',11155111,1,101,100,now(),true),('nft','matic',80002,1,101,100,now(),true);
      INSERT INTO shop.collections(network,contract_address,chain_id,name,creator,owner,urn,is_completed,is_approved,is_editable,created_at,updated_at,reviewed_at,mirrored_at,testnet_verified)
      VALUES('matic','{WALLET_B}',80002,'Amoy collection','{WALLET_A}','{WALLET_A}','urn:decentraland:amoy:collections-v2:{WALLET_B}',true,true,false,1,1,1,now(),true);
      INSERT INTO shop.items(network,contract_address,item_id,chain_id,item_type,urn,creator,rarity,total_supply,max_supply,available,price,image,name,category,body_shapes,search_is_collection_approved,created_at,updated_at,reviewed_at,mirrored_at,testnet_verified)
      SELECT 'matic','{WALLET_B}',n,80002,'wearable_v2','urn:decentraland:amoy:collections-v2:{WALLET_B}:'||n,'{WALLET_A}','rare',1,1,0,5,'img','Hat','hat',ARRAY['BaseMale'],true,1,1,1,now(),n=1 FROM generate_series(1,2)n;
      INSERT INTO shop.nft(network,contract_address,token_id,owner,item_id,issued_id,minted_block,minted_at,transferred_block,transferred_log,transferred_at)
      SELECT 'matic','{WALLET_B}',n,'{WALLET_A}',n,n,1,1,1,0,1 FROM generate_series(1,2)n;
      INSERT INTO shop.nft(network,contract_address,token_id,owner,ens_subdomain,minted_block,minted_at,transferred_block,transferred_log,transferred_at)
      VALUES ('ethereum','{registrar}',8,'{WALLET_A}','testname',1,1,1,0,1)"))).execute(&db.pool).await.unwrap();
    let (code, out) = get(app.clone(), wearable).await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{out}");
    assert_eq!(out["source"], "items-mirror");
    sqlx::query("INSERT INTO shop.mirror_state(mirror,network,source_url,chain_id,last_complete_pass_at,marks_item_rows) VALUES('items','matic','graph',80002,now(),true)").execute(&db.pool).await.unwrap();
    let (code, out) = get(app.clone(), wearable).await;
    assert_eq!(code, StatusCode::OK, "{out}");
    assert_eq!(out["total"], 1, "{out}");
    assert_eq!(out["data"][0]["nft"]["chainId"], 80002);
    let (code, out) = get(
        app,
        &format!("/v1/nfts?contractAddress={registrar}&tokenId=8"),
    )
    .await;
    assert_eq!(code, StatusCode::OK, "{out}");
    assert_eq!(out["data"][0]["nft"]["chainId"], 11155111, "{out}");
    {
        let mut conn = db.pool.acquire().await.unwrap();
        assert!(
            super::names::check(
                Catalogue::Testnet,
                &mut conn,
                Purpose::Authorize,
                chrono::Utc::now()
            )
            .await
            .is_ok()
        );
        sqlx::query("INSERT INTO shop.nft_event(network,block_number,log_index,block_time,kind,emitter,contract_address,token_id)
            VALUES('ethereum',101,0,1,'transfer',$1,$1,8)").bind(registrar).execute(&mut *conn).await.unwrap();
        assert!(
            super::names::check(
                Catalogue::Testnet,
                &mut conn,
                Purpose::Authorize,
                chrono::Utc::now()
            )
            .await
            .is_err()
        );
    }
    db.drop().await;
}

#[tokio::test]
async fn sepolia_owner_pages_require_the_matching_publications_and_keep_the_wire_owner() {
    use crate::land_authz::LAND_SEPOLIA;
    let venue = crate::rentals::index::RentalsChain::Sepolia.venue();
    let Some(db) = super::tests::support::OwnDb::create("sepolia_local_inventory").await else {
        return;
    };
    let mut settings = Settings::for_tests();
    settings.eth_network = "sepolia".into();
    settings.market.indexes.nft_enabled = true;
    settings.market.indexes.chain_id_ethereum = 11_155_111;
    settings.market.indexes.chain_id_matic = 80_002;
    let graph = FakeGraph::new();
    settings.marketplace_subgraph_url = Some(serve_graph(graph.clone()).await);
    let state = Arc::new(super::MarketplaceState::new(
        db.pool.clone(),
        crate::http::Outbound::for_tests(),
        settings,
    ));
    let app = market_router(state);
    let land = LAND_SEPOLIA.land_registry;
    let estate = LAND_SEPOLIA.estate_registry;
    let url = format!("/v1/nfts?category=parcel&owner={WALLET_A}");
    let run = |sql: String| async {
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
            .execute(&db.pool)
            .await
            .unwrap();
    };
    run(format!("INSERT INTO land_authz.index_cursor(network,land_registry,estate_registry,last_block,folded_block,folded_at,full_replay_started)
      VALUES ('ethereum-sepolia','{land}','{estate}',35,35,timezone('UTC',now()),true);
      INSERT INTO land_authz.token_right(token_address,token_id,owner,x,y,updated_block,updated_log)
      VALUES ('{land}',13,'{venue}',0,13,12,0);
      INSERT INTO land_authz.authz_event(token_address,token_id,kind,block_number,log_index,block_time,account)
      VALUES ('{land}',13,'transfer',12,0,100,'{venue}');
      INSERT INTO shop.chain_cursor(indexer,network,start_block,last_block,folded_block,folded_at,full_history,chain_id)
      VALUES ('nft','ethereum',0,39,35,timezone('UTC',now()),true,1),
             ('rentals','ethereum',0,40,40,timezone('UTC',now()),true,1);
      INSERT INTO shop.rental_event(network,rentals_address,kind,contract_address,token_id,block_number,log_index,block_time,signer,tenant,rental_days,price_per_day,is_extension,signature,tx_hash)
      VALUES ('sepolia','{venue}','rented','{land}',13,12,1,100,'{WALLET_A}','{WALLET_B}',1,1,false,'0x01','0x02')")).await;
    let request = || get(app.clone(), &url);
    let (status, page) = request().await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{page}");
    assert_eq!(page["source"], "nft-index");
    run("UPDATE shop.chain_cursor SET chain_id=11155111 WHERE indexer='nft'".into()).await;
    for seed in ["", "INSERT INTO shop.chain_cursor(indexer,network,start_block,last_block,folded_block,folded_at,full_history,chain_id)
           VALUES ('rentals_sepolia','ethereum',0,40,30,timezone('UTC',now()),true,11155111)"] {
        if !seed.is_empty() {run(seed.into()).await;}
        let (status,page)=request().await;
        assert_eq!(status,StatusCode::SERVICE_UNAVAILABLE,"{page}");
        assert_eq!(page["source"],"rentals-index");
    }
    run("INSERT INTO shop.worker_progress(worker,scope,position,ceiling) VALUES ('rental_listing_generation',11155111,0,0)".into()).await;
    let (status, page) = request().await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(page["total"], 1);
    let nft = &page["data"][0]["nft"];
    assert_eq!(nft["owner"], venue);
    assert_eq!(nft["chainId"], 11155111);
    let (status, empty) = get(app, "/v1/nfts?category=parcel&network=MATIC").await;
    assert_eq!(status, StatusCode::OK, "{empty}");
    assert_eq!(empty["total"], 0);
    assert!(graph.requests.lock().unwrap().is_empty());
    db.drop().await;
}

/// Cancelling an optional V3 statement leaves the parcel page's required
/// rental read usable on the same snapshot.
#[tokio::test]
async fn a_parcel_list_keeps_rentals_when_optional_orders_time_out() {
    use crate::land_authz::LAND_MAINNET;
    use crate::rentals::index::RENTALS_MAINNET;
    let graph = FakeGraph::new();
    let url = serve_graph(graph).await;
    let Some((state, db)) = state_over_own(&url, true).await else {
        return;
    };
    let pool = &db.pool;
    let land = LAND_MAINNET.land_registry;
    for token in ["1", "2"] {
        crate::test_db::publish_land_owner(pool, land, token, WALLET_A).await;
    }
    sqlx::query("UPDATE land_authz.token_right SET x=0,y=token_id::int WHERE token_address=$1")
        .bind(land)
        .execute(pool)
        .await
        .unwrap();
    super::owned_items::test_support::publish(
        pool,
        &[super::chain_walk::ChainNet::Ethereum],
        false,
        Duration::ZERO,
    )
    .await;
    super::trade_events::tests::publish_cursors(pool).await;
    sqlx::query(
        "INSERT INTO shop.nft_order(network,contract_address,token_id,order_id,marketplace_address,status,seller,price,expires_at,created_at,updated_at,updated_block,updated_log)
         VALUES('ethereum',$1,1,'legacy-1',$2,'open',$3,25,4102444800000,1,2,13,0)",
    )
    .bind(land).bind(CREATOR).bind(WALLET_A).execute(pool).await.unwrap();
    let rental = crate::rentals::index::tests::seed_listing(
        pool,
        land,
        "2",
        ["0"; 3],
        "0x42",
        WALLET_A,
        RENTALS_MAINNET,
    )
    .await
    .to_string();

    let app = market_router(state);
    let uri = "/v1/nfts?category=parcel&first=2";
    let check = |status, body: &Value| {
        assert_eq!(status, StatusCode::OK, "{body}");
        let rows = body["data"].as_array().unwrap();
        assert_eq!(rows.len(), 2, "{body}");
        assert!(
            rows.iter().any(|r| r["order"]["id"] == "legacy-1"),
            "{body}"
        );
        assert!(rows.iter().any(|r| r["rental"]["id"] == rental), "{body}");
    };
    let (status, before) = get(app.clone(), uri).await;
    check(status, &before);
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE shop.trades IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *lock)
        .await
        .unwrap();
    let started = std::time::Instant::now();
    let (status, after) = get(app, uri).await;
    lock.rollback().await.unwrap();
    assert!(started.elapsed() >= super::nft_reads::READ_STATEMENT_TIMEOUT);
    check(status, &after);
    db.drop().await;
}

#[tokio::test]
async fn owner_pages_include_only_current_unclaimed_land_before_pagination() {
    use crate::land_authz::LAND_MAINNET;
    use crate::rentals::index::RENTALS_MAINNET as VENUE;

    let Some(db) = super::tests::support::OwnDb::create("owner_rented_land").await else {
        return;
    };
    let mut settings = Settings::for_tests();
    settings.eth_network = "mainnet".into();
    settings.market.indexes.nft_enabled = true;
    let state = Arc::new(super::MarketplaceState::new(
        db.pool.clone(),
        crate::http::Outbound::for_tests(),
        settings,
    ));
    let app = market_router(state.clone());
    let run = |sql: String| async {
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
            .execute(&db.pool)
            .await
            .unwrap();
    };
    let land = LAND_MAINNET.land_registry;
    let estate = LAND_MAINNET.estate_registry;
    let owner_url = format!("/v1/nfts?owner={WALLET_A}");
    run(format!(
        "INSERT INTO land_authz.index_cursor(network,land_registry,estate_registry,last_block,folded_block,folded_at,full_replay_started)
         VALUES ('ethereum-mainnet','{land}','{estate}',40,40,timezone('UTC',now()),true);
         INSERT INTO shop.chain_cursor(indexer,network,start_block,last_block,folded_block,folded_at,full_history,chain_id)
         VALUES ('nft','ethereum',0,40,40,timezone('UTC',now()),true,1),
                ('nft','matic',0,40,40,timezone('UTC',now()),true,137),
                ('rentals','ethereum',0,38,30,timezone('UTC',now()),true,1);
         INSERT INTO shop.mirror_state(mirror,network,source_url,last_complete_pass_at)
         VALUES ('items','ethereum','test',now()),('items','matic','test',now());
         INSERT INTO land_authz.token_right(token_address,token_id,owner,x,y,updated_block,updated_log)
         VALUES ('{land}',1,'{WALLET_A}',0,1,10,0),('{land}',2,'{VENUE}',0,2,12,0),('{estate}',3,'{VENUE}',NULL,NULL,15,1);
         INSERT INTO land_authz.estate_land VALUES (801,3,14,1);
         INSERT INTO land_authz.authz_event(token_address,token_id,kind,block_number,log_index,block_time,account)
         VALUES ('{land}',1,'transfer',10,0,100,'{WALLET_A}'),('{land}',2,'transfer',12,0,120,'{VENUE}'),
                ('{estate}',3,'transfer',15,1,150,'{VENUE}');"
    )).await;
    let rent = |contract: &str, id: i32, block: i64, log: i32, signer: &str, extension: bool| {
        format!(
        "INSERT INTO shop.rental_event(network,rentals_address,kind,contract_address,token_id,block_number,log_index,block_time,signer,tenant,rental_days,price_per_day,is_extension,signature,tx_hash)
         VALUES ('ethereum','{VENUE}','rented','{contract}',{id},{block},{log},100,'{signer}','{WALLET_B}',1,1,{extension},'0x01','0x02')"
    )
    };
    run(rent(land, 2, 12, 1, WALLET_A, false)).await;
    run(rent(land, 2, 14, 0, WALLET_A, true)).await;
    run(rent(estate, 3, 15, 2, WALLET_A, false)).await;
    // Rentals has walked to 38 but published through 30. The owner page
    // stays available and the event at 35 cannot change its published answer.
    run(rent(land, 2, 35, 0, WALLET_B, false)).await;
    let mut tokens = std::collections::BTreeMap::new();
    for skip in 0..3 {
        let (code, result) = get(app.clone(), &format!("{owner_url}&first=1&skip={skip}")).await;
        assert_eq!(code, StatusCode::OK, "{result}");
        assert_eq!(result["total"], 3);
        let nft = &result["data"][0]["nft"];
        tokens.insert(
            nft["tokenId"].as_str().unwrap().to_owned(),
            nft["owner"].clone(),
        );
    }
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens["1"], WALLET_A);
    assert_eq!(tokens["2"], VENUE);
    assert_eq!(tokens["3"], VENUE);
    let listing = crate::rentals::index::tests::seed_listing(
        &db.pool, land, "2", ["0"; 3], "0xabb1", WALLET_A, VENUE,
    )
    .await;
    sqlx::query("UPDATE signatures.rentals SET status='executed' WHERE id=$1")
        .bind(listing)
        .execute(&db.pool)
        .await
        .unwrap();
    for (status, total) in [("executed", 1), ("open", 0)] {
        let (code, result) = get(
            app.clone(),
            &format!("{owner_url}&category=parcel&isOnRent=true&rentalStatus={status}"),
        )
        .await;
        assert_eq!(code, StatusCode::OK, "{result}");
        assert_eq!(result["total"], total);
        if total == 1 {
            assert_eq!(result["data"][0]["nft"]["tokenId"], "2");
            assert_eq!(result["data"][0]["rental"]["id"], listing.to_string());
        }
    }
    let claim = |block: i64, log: i32| {
        format!(
        "INSERT INTO shop.rental_event(network,rentals_address,kind,contract_address,token_id,block_number,log_index,block_time,tx_hash)
         VALUES ('ethereum','{VENUE}','claimed','{land}',2,{block},{log},200,'0x03')"
    )
    };
    run(claim(20, 0)).await;
    let (code, result) = get(app.clone(), &owner_url).await;
    assert_eq!(code, StatusCode::OK, "{result}");
    assert_eq!(result["total"], 2, "a claim removes the parcel");
    let transfer = |block: i64, log: i32, owner: &str| {
        format!(
        "INSERT INTO land_authz.authz_event(token_address,token_id,kind,block_number,log_index,block_time,account)
         VALUES ('{land}',2,'transfer',{block},{log},200,'{owner}');
         UPDATE land_authz.token_right SET owner='{owner}',updated_block={block},updated_log={log} WHERE token_address='{land}' AND token_id=2"
    )
    };
    // LAND catches up with the claim: the parcel returns through direct ownership.
    run(transfer(20, 1, WALLET_A)).await;
    let (code, result) = get(app.clone(), &owner_url).await;
    assert_eq!(code, StatusCode::OK, "{result}");
    assert_eq!(result["total"], 3);
    run(transfer(20, 2, VENUE)).await;
    run(rent(land, 2, 20, 3, WALLET_B, false)).await;
    let (code, result) = get(
        app.clone(),
        &format!("/v1/nfts?owner={WALLET_B}&category=parcel"),
    )
    .await;
    assert_eq!(code, StatusCode::OK, "{result}");
    assert_eq!(result["total"], 1);
    assert_eq!(result["data"][0]["nft"]["tokenId"], "2");

    // The new arrival has no published lessor yet. An owner with no rent
    // history cannot safely be answered either.
    run(format!(
        "INSERT INTO land_authz.authz_event(token_address,token_id,kind,block_number,log_index,block_time,account)
         VALUES ('{land}',2,'transfer',36,1,360,'{VENUE}');
         UPDATE land_authz.token_right SET updated_block=36,updated_log=1 WHERE token_address='{land}' AND token_id=2"
    )).await;
    for owner in [
        WALLET_A,
        WALLET_B,
        "0xcccccccccccccccccccccccccccccccccccccccc",
    ] {
        let (code, result) = get(app.clone(), &format!("/v1/nfts?owner={owner}")).await;
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{result}");
        assert_eq!(result["source"], "rentals-index");
    }
    run(
        "UPDATE shop.chain_cursor SET last_block=40,folded_block=40 WHERE indexer='rentals'".into(),
    )
    .await;
    // The latest rent for B preceded the new arrival, even with a rent
    // earlier in the same block. A later log reestablishes custody.
    run(rent(land, 2, 36, 0, WALLET_B, false)).await;
    let url_b = format!("/v1/nfts?owner={WALLET_B}&category=parcel");
    let (code, result) = get(app.clone(), &url_b).await;
    assert_eq!(code, StatusCode::OK, "{result}");
    assert_eq!(result["total"], 0);
    run(rent(land, 2, 36, 2, WALLET_B, false)).await;
    let (code, result) = get(app.clone(), &url_b).await;
    assert_eq!(code, StatusCode::OK, "{result}");
    assert_eq!(result["total"], 1);
    run(format!("INSERT INTO shop.rental_event(network,rentals_address,kind,contract_address,token_id,block_number,log_index,block_time,tx_hash)
        VALUES ('ethereum','{VENUE}','claimed','{estate}',3,38,1,380,'0x04')")).await;
    let (code, result) = get(app.clone(), &format!("{owner_url}&category=estate")).await;
    assert_eq!(code, StatusCode::OK, "{result}");
    assert_eq!(result["total"], 0);
    run("UPDATE shop.chain_cursor SET folded_block=NULL WHERE indexer='rentals'".into()).await;
    let (code, result) = get(app.clone(), &owner_url).await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{result}");
    assert_eq!(result["source"], "rentals-index");
    let (code, result) = get(app.clone(), "/v1/nfts?category=parcel").await;
    assert_eq!(code, StatusCode::OK, "{result}");
    // The switch controls the writer. Without a cursor the reader cannot
    // establish LAND custody, while a wearable-only page still has all its sources.
    assert!(
        super::chain_jobs::expected_cursors(
            &state,
            super::chain_walk::ChainNet::Ethereum,
            Some(false)
        )
        .iter()
        .all(|(index, _)| index.indexer() != "rentals")
    );
    run("DELETE FROM shop.chain_cursor WHERE indexer='rentals'".into()).await;
    let (code, result) = get(app.clone(), &owner_url).await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{result}");
    assert_eq!(result["source"], "rentals-index");
    let (code, result) = get(app.clone(), &format!("{owner_url}&category=wearable")).await;
    assert_eq!(code, StatusCode::OK, "{result}");
    db.drop().await;
}

async fn publish_bid_collections(pool: &PgPool, chain: i64) {
    sqlx::query(
        "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at, chain_id) \
         VALUES ('collections', 'matic', 'test', timezone('UTC', now()), $1) \
         ON CONFLICT (mirror, network) DO UPDATE SET last_complete_pass_at = EXCLUDED.last_complete_pass_at, \
             chain_id = EXCLUDED.chain_id",
    )
    .bind(chain)
    .execute(pool)
    .await
    .unwrap();
}

/// A walking node with the sales index on, over its own database: Polygon's
/// legacy bids come from `shop.nft_bid`.
async fn state_with_the_bid_book(url: &str) -> Option<(SharedState, super::tests::support::OwnDb)> {
    let own = super::tests::support::OwnDb::create("bid_book").await?;
    publish_bid_collections(&own.pool, 137).await;
    let eth_url = serve_empty_graph().await;
    let settings = super::owned_items::test_support::walking(market_settings(url, &eth_url, true));
    let market = MarketSettings {
        indexes: super::chain_walk::IndexSettings {
            sales_enabled: true,
            ..settings.market.indexes.clone()
        },
        ..settings.market.clone()
    };
    let state = Arc::new(super::MarketplaceState::new(
        own.pool.clone(),
        crate::http::Outbound::for_tests(),
        settings.with_market(market),
    ));
    state.test_allow_loopback.store(true, Ordering::Relaxed);
    Some((state, own))
}

#[tokio::test]
async fn the_bid_book_preserves_trades_readiness_timeouts() {
    let url = serve_empty_graph().await;
    let Some((mut state, own)) = state_with_the_bid_book(&url).await else {
        return;
    };
    let pool = &own.pool;
    publish_trades_mirror_for_timeout_test(&mut state, pool).await;
    super::trade_events::tests::publish_cursors(pool).await;
    super::owned_items::test_support::publish(
        pool,
        &[super::chain_walk::ChainNet::Matic],
        false,
        Duration::ZERO,
    )
    .await;
    sqlx::query(
        "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, fold_through,
                                        folded_block, folded_at, full_history, chain_id)
         VALUES ('sales', 'matic', 1, 100, 100, 100, timezone('UTC', now()), true, 137)",
    )
    .execute(pool)
    .await
    .unwrap();

    assert!(super::bids::polygon_book_here(&state));
    let app = market_router(state);
    for lock in [
        "LOCK TABLE shop.trades_mirror IN ACCESS EXCLUSIVE MODE",
        "LOCK TABLE shop.chain_cursor IN ACCESS EXCLUSIVE MODE",
    ] {
        assert_trades_readiness_timeout(pool, app.clone(), "/v1/bids?network=MATIC", lock).await;
    }
    own.drop().await;
}

/// Polygon's legacy bids on a node that walks the nft and sales indexes come
/// from `shop.nft_bid` with the subgraph's wire shape and statuses (a bid
/// another acceptance dropped reads `cancelled`), behind the sales index's
/// and the nft index's envelopes. A bid the fold left without a seller is
/// served with the token's owner at the bid once the nft walk's log reaches
/// its block and holds the contract's whole history; until then any read
/// whose scope holds it fails closed, even with both cursors fresh and the
/// sales one ahead, and a read whose filters leave it out answers. Bids on
/// an asset outside the walked catalogue are out of every scope: they
/// neither fail a read nor appear in a page or a total.
#[tokio::test]
async fn polygon_bids_come_from_the_bid_book_and_never_without_their_seller() {
    let up = FakeGraph::new();
    let url = serve_graph(up.clone()).await;
    let Some((state, own)) = state_with_the_bid_book(&url).await else {
        return;
    };
    super::trade_events::tests::publish_cursors(&state.pool).await;
    assert!(super::bids::polygon_book_here(&state));
    let pool = state.pool.clone();
    let c = contract_of(&tag(), 5);
    const CARL: &str = "0xdddddddddddddddddddddddddddddddddddddddd";
    let catalogue = |contract: String| {
        let pool = pool.clone();
        async move {
            sqlx::query(
                "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, is_completed, is_approved, is_editable, created_at, updated_at, reviewed_at, mirrored_at) \
                 SELECT 'matic', $1, 137, 'Pirates', $2, $2, 'urn:decentraland:matic:collections-v2:' || $1, true, true, false, 1, 1, 1, timezone('UTC', now())",
            )
            .bind(contract)
            .bind(CREATOR)
            .execute(&pool)
            .await
            .unwrap();
        }
    };
    catalogue(c.clone()).await;
    // Bids on an ERC721 the catalogue does not hold, which the bid contract
    // takes and the nft walk never reads: one the fold left without a
    // seller, below the nft walk, and an acceptance with its seller.
    let stray = contract_of(&tag(), 6);
    for (bidder, seller, status, block) in [
        (CARL, None, "open", 20i64),
        (WALLET_B, Some(WALLET_A), "accepted", 21),
    ] {
        sqlx::query(
            "INSERT INTO shop.nft_bid (network, contract_address, token_id, bidder, bid_id, seller, price, expires_at, status, \
                 created_block, created_log_index, created_at, updated_block, updated_log_index, updated_at) \
             VALUES ('matic', $1, 1, $2, '0x' || lpad(to_hex($5), 64, '0'), $3, 1000, $6, $4, $5, 0, $5, $5, 0, $5)",
        )
        .bind(&stray)
        .bind(bidder)
        .bind(seller)
        .bind(status)
        .bind(block)
        .bind(chrono::Utc::now().timestamp() + 86_400)
        .execute(&pool)
        .await
        .unwrap();
    }
    let now_s = chrono::Utc::now().timestamp();
    let bid = |token: i64,
               bidder: &'static str,
               seller: Option<&'static str>,
               status: &'static str,
               price: i64,
               block: i64,
               expires: i64| {
        let (pool, c) = (pool.clone(), c.clone());
        async move {
            sqlx::query(
                "INSERT INTO shop.nft_bid (network, contract_address, token_id, bidder, bid_id, seller, price, expires_at, status, \
                     created_block, created_log_index, created_at, updated_block, updated_log_index, updated_at) \
                 VALUES ('matic', $1, $2, $3, '0x' || lpad(to_hex($2 * 100 + $6), 64, '0'), $4, $5, $7, $8, $6, 0, $6, $6 + 1, 0, $6 + 1)",
            )
            .bind(&c)
            .bind(token)
            .bind(bidder)
            .bind(seller)
            .bind(price)
            .bind(block)
            .bind(expires)
            .bind(status)
            .execute(&pool)
            .await
            .unwrap();
        }
    };
    let later = now_s + 86_400;
    bid(1, WALLET_B, Some(WALLET_A), "open", 100, 10, later).await;
    bid(2, WALLET_B, Some(WALLET_A), "accepted", 200, 11, later).await;
    bid(3, WALLET_B, Some(WALLET_A), "cancelled", 300, 12, later).await;
    bid(4, WALLET_B, Some(WALLET_A), "invalidated", 400, 13, later).await;
    bid(5, WALLET_B, Some(WALLET_A), "open", 500, 14, now_s - 10).await;
    // The fold met these before the nft walk reached them.
    bid(6, CARL, None, "open", 600, 50, later).await;
    bid(7, CARL, None, "open", 700, 150, later).await;
    for (token, block) in [(6, 40), (7, 140)] {
        sqlx::query(
            "INSERT INTO shop.nft_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, from_address, to_address) \
             VALUES ('matic', $1, 0, $1, 'transfer', $2, $2, $3, '0x0000000000000000000000000000000000000000', $4)",
        )
        .bind(block)
        .bind(&c)
        .bind(token)
        .bind(CREATOR)
        .execute(&pool)
        .await
        .unwrap();
    }
    let set_cursor = |indexer: &'static str, last: i64| {
        let pool = pool.clone();
        async move {
            sqlx::query(
                "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, last_block_hash, fold_through, folded_block, folded_at, full_history, chain_id) \
                 VALUES ($1, 'matic', 1, $2, '0x' || repeat('0', 64), $2, $2, timezone('UTC', now()), true, 137) \
                 ON CONFLICT (indexer, network) DO UPDATE SET last_block = $2, fold_through = $2, folded_block = $2, folded_at = EXCLUDED.folded_at, full_history = true, chain_id = 137",
            )
            .bind(indexer)
            .bind(last)
            .execute(&pool)
            .await
            .unwrap();
        }
    };
    let app = || market_router(state.clone());
    let ids = |body: &Value| -> Vec<(String, String, String)> {
        body["data"]["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| {
                (
                    b["tokenId"].as_str().unwrap().to_string(),
                    b["status"].as_str().unwrap().to_string(),
                    b["seller"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    };
    let polygon = "/v1/bids?network=MATIC&sortBy=most_expensive";
    let (status, body) = get(app(), polygon).await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("sales-index")),
        "{body}"
    );
    set_cursor("sales", 200).await;
    let (status, body) = get(app(), polygon).await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("nft-index")),
        "{body}"
    );
    // Both fresh, the sales walk ahead: bid 7 (block 150) has no seller yet.
    set_cursor("nft", 100).await;
    let (status, body) = get(app(), polygon).await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("nft-index")),
        "an unresolved seller in scope fails the read closed: {body}"
    );
    let (status, body) = get(app(), &format!("{polygon}&seller={CREATOR}")).await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "a seller filter cannot tell whether bid 7 is its: {body}"
    );
    let (status, body) = get(app(), &format!("{polygon}&bidder={WALLET_B}")).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "bid 7 is out of this scope, as is the stray acceptance: {body}"
    );
    assert_eq!(
        ids(&body),
        vec![
            ("4".into(), "cancelled".into(), WALLET_A.into()),
            ("3".into(), "cancelled".into(), WALLET_A.into()),
            ("2".into(), "sold".into(), WALLET_A.into()),
            ("1".into(), "open".into(), WALLET_A.into()),
        ],
        "the expired bid is left out: {body}"
    );
    assert_eq!(body["data"]["total"], 4, "{body}");
    let first = &body["data"]["results"][3];
    assert_eq!(
        (
            first["id"].as_str().map(str::to_string),
            first["bidAddress"].as_str(),
            first["blockchainId"].as_str().map(str::to_string),
            first["blockNumber"].as_str(),
            first["fingerprint"].as_str(),
            first["price"].as_str(),
            first["createdAt"].as_i64(),
            first["updatedAt"].as_i64(),
            first["expiresAt"].as_i64(),
            first["chainId"].as_i64(),
        ),
        (
            Some(format!("{c}-1-{WALLET_B}")),
            Some(super::sales_index::BID_V2_MATIC),
            Some(format!("0x{:064x}", 110)),
            Some("11"),
            Some(""),
            Some("100"),
            Some(10_000),
            Some(11_000),
            Some(later * 1000),
            Some(137),
        ),
        "{body}"
    );
    // The nft walk reaches block 150: bid 7's seller is the owner at its block.
    // The stray bid without a seller does not fail the read.
    set_cursor("nft", 200).await;
    let (status, body) = get(app(), polygon).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        ids(&body)[..2].to_vec(),
        vec![
            ("7".into(), "open".into(), CREATOR.into()),
            ("6".into(), "open".into(), CREATOR.into()),
        ],
        "{body}"
    );
    let (_, body) = get(app(), &format!("{polygon}&seller={CREATOR}&status=open")).await;
    assert_eq!(body["data"]["total"], 2, "{body}");
    let (_, body) = get(app(), "/v1/bids?network=MATIC&status=cancelled").await;
    assert_eq!(
        body["data"]["total"], 2,
        "a dropped bid counts as cancelled: {body}"
    );
    let (_, body) = get(app(), "/v1/bids?network=MATIC&status=sold").await;
    assert_eq!(
        ids(&body),
        vec![("2".into(), "sold".into(), WALLET_A.into())],
        "the stray acceptance is neither listed nor counted: {body}"
    );
    assert_eq!(body["data"]["total"], 1, "{body}");
    sqlx::query(
        "INSERT INTO shop.collection_projection (network, owned_at, projected_through) \
         VALUES ('matic', timezone('UTC', now()), 200)",
    )
    .execute(&pool)
    .await
    .unwrap();
    set_cursor("collections", 200).await;
    let sold = "/v1/bids?network=MATIC&status=sold";
    let (status, before_hand_back) = get(app(), sold).await;
    assert_eq!(status, StatusCode::OK, "{before_hand_back}");
    assert_eq!(before_hand_back["data"]["total"], 1, "{before_hand_back}");
    assert!(
        super::collections_projection::hand_back(&pool, super::chain_walk::ChainNet::Matic)
            .await
            .unwrap()
    );
    let (status, body) = get(app(), sold).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["source"], "collections-mirror", "{body}");
    // Repopulating the catalogue is insufficient before the mirror publishes.
    catalogue(c.clone()).await;
    let (status, body) = get(app(), sold).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["source"], "collections-mirror", "{body}");
    publish_bid_collections(&pool, 137).await;
    let (status, after_hand_back) = get(app(), sold).await;
    assert_eq!(status, StatusCode::OK, "{after_hand_back}");
    assert_eq!(after_hand_back, before_hand_back);

    // An nft rewind the sales fold has not rechecked yet: the stored seller
    // of an open bid created past it is set aside and read again from the
    // rewound log, as the next fold will; an acceptance's seller stands.
    bid(8, WALLET_B, Some(WALLET_A), "open", 800, 60, later).await;
    bid(9, WALLET_B, Some(WALLET_A), "accepted", 900, 61, later).await;
    set_cursor("sales", 200).await;
    sqlx::query(
        "INSERT INTO shop.nft_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, from_address, to_address) \
         VALUES ('matic', 55, 0, 55, 'transfer', $1, $1, 8, '0x0000000000000000000000000000000000000000', $2)",
    )
    .bind(&c)
    .bind(CREATOR)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO shop.bid_seller_recheck (network, from_block) VALUES ('matic', 50)")
        .execute(&pool)
        .await
        .unwrap();
    for (token, seller) in [(8, CREATOR), (9, WALLET_A)] {
        let (status, body) = get(
            app(),
            &format!("/v1/bids?network=MATIC&contractAddress={c}&tokenId={token}"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(
            body["data"]["results"][0]["seller"], seller,
            "{token}: {body}"
        );
    }
    for (seller, total) in [(CREATOR, 1), (WALLET_A, 0)] {
        let (status, body) = get(
            app(),
            &format!("/v1/bids?network=MATIC&contractAddress={c}&tokenId=8&seller={seller}"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["data"]["total"], total, "{body}");
    }
    sqlx::query("DELETE FROM shop.nft_bid WHERE network = 'matic' AND token_id IN (8, 9)")
        .execute(&pool)
        .await
        .unwrap();
    set_cursor("sales", 200).await;
    // Ethereum's legacy bids still read the collections subgraph.
    let (status, body) = get(app(), "/v1/bids?network=ETHEREUM").await;
    assert_eq!(
        (status, body["data"]["total"].as_i64()),
        (StatusCode::OK, Some(0)),
        "{body}"
    );
    let (status, body) = get(app(), "/v1/bids").await;
    assert_eq!(
        (status, body["data"]["total"].as_i64()),
        (StatusCode::OK, Some(6)),
        "{body}"
    );
    // The book is read only at the sales walk's published fold. A walk that
    // advanced without folding leaves it at the published 200, still read; a
    // fold committed through 210 whose publication stalls (the walk reads
    // the head first) is not read until it is published.
    sqlx::query("UPDATE shop.chain_cursor SET last_block = 210 WHERE indexer = 'sales' AND network = 'matic'")
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = get(app(), sold).await;
    assert_eq!(status, StatusCode::OK, "the walk advanced unfolded: {body}");
    assert_eq!(body["data"]["total"], 1, "{body}");
    sqlx::query(
        "INSERT INTO shop.sale_event (network, block_number, log_index, block_time, kind, emitter, contract_address, \
             token_id, bid_id, buyer, seller, price, expires_at, tx_hash) \
         VALUES ('matic', 10, 0, 10, 'bid_created', $1, $2, 1, $3, $4, NULL, 100, $6, '0x' || repeat('1', 64)), \
                ('matic', 210, 0, 210, 'bid_accepted', $1, $2, 1, $3, $4, $5, 100, NULL, '0x' || repeat('2', 64))",
    )
    .bind(super::sales_index::BID_V2_MATIC)
    .bind(&c)
    .bind(format!("0x{:064x}", 110))
    .bind(WALLET_B)
    .bind(WALLET_A)
    .bind(later)
    .execute(&pool)
    .await
    .unwrap();
    let ix = super::sales_index::SalesIndex::for_chain(137).unwrap();
    let owner = super::chain_walk::test_owner(&pool, &ix, &state.settings.market.indexes).await;
    super::sales_index::fold(&pool, Some(200), &owner)
        .await
        .unwrap();
    // Simulate a delayed head RPC by stopping before the publication steps.
    let (status, body) = get(app(), sold).await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("sales-index")),
        "a fold through 210 committed while 200 is published: {body}"
    );
    super::chain_walk::record_fold(&pool, &owner, 210)
        .await
        .unwrap();
    assert!(
        super::chain_walk::mark_folded(&pool, &ix, &owner, 210, chrono::Utc::now())
            .await
            .unwrap()
    );
    let (status, body) = get(app(), sold).await;
    assert_eq!(status, StatusCode::OK, "published at 210: {body}");
    assert_eq!(
        ids(&body),
        vec![
            ("2".into(), "sold".into(), WALLET_A.into()),
            ("1".into(), "sold".into(), WALLET_A.into()),
        ],
        "{body}"
    );
    assert_eq!(body["data"]["total"], 2, "{body}");
    // A contract added to the walk late, its backfill done through block 40
    // of the 99 below its walk: the log holds a transfer at 35 but not yet
    // the one at 60, so the owner before a bid at 70 is not known.
    let late = contract_of(&tag(), 7);
    catalogue(late.clone()).await;
    sqlx::query(
        "INSERT INTO shop.nft_contract (network, contract_address, walked_from_block, backfill_from_block, backfilled_to_block) \
         VALUES ('matic', $1, 100, 10, 40)",
    )
    .bind(&late)
    .execute(&pool)
    .await
    .unwrap();
    let transfer = |block: i64, to: &'static str| {
        let (pool, late) = (pool.clone(), late.clone());
        async move {
            sqlx::query(
                "INSERT INTO shop.nft_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, from_address, to_address) \
                 VALUES ('matic', $1, 0, $1, 'transfer', $2, $2, 1, '0x0000000000000000000000000000000000000000', $3)",
            )
            .bind(block)
            .bind(late)
            .bind(to)
            .execute(&pool)
            .await
            .unwrap();
        }
    };
    transfer(35, WALLET_A).await;
    sqlx::query(
        "INSERT INTO shop.nft_bid (network, contract_address, token_id, bidder, bid_id, seller, price, expires_at, status, \
             created_block, created_log_index, created_at, updated_block, updated_log_index, updated_at) \
         VALUES ('matic', $1, 1, $2, '0x' || lpad(to_hex(70), 64, '0'), NULL, 1000, $3, 'open', 70, 0, 70, 70, 0, 70)",
    )
    .bind(&late)
    .bind(CARL)
    .bind(later)
    .execute(&pool)
    .await
    .unwrap();
    set_cursor("sales", 210).await;
    let of_late =
        |seller: &str| format!("/v1/bids?network=MATIC&contractAddress={late}&seller={seller}");
    let (status, body) = get(app(), &of_late(WALLET_A)).await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("nft-index")),
        "the transfer at 35 is not the owner before the bid while the backfill is owed: {body}"
    );
    transfer(60, CREATOR).await;
    sqlx::query("UPDATE shop.nft_contract SET backfilled_to_block = 99 WHERE network = 'matic' AND contract_address = $1")
        .bind(&late)
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = get(app(), &of_late(WALLET_A)).await;
    assert_eq!(
        (status, body["data"]["total"].as_i64()),
        (StatusCode::OK, Some(0)),
        "{body}"
    );
    let (status, body) = get(app(), &of_late(CREATOR)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        ids(&body),
        vec![("1".into(), "open".into(), CREATOR.into())],
        "{body}"
    );
    // An old sales writer running after an upgraded writer commits the
    // same bid fold. The schema records its writes as pending. Its raw walk
    // leaves the existing publication readable until it changes the book.
    sqlx::query(
        "UPDATE shop.chain_cursor SET last_block = 220 WHERE indexer = 'sales' AND network = 'matic'",
    )
    .execute(&pool)
    .await
    .unwrap();
    let (status, body) = get(app(), sold).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    sqlx::query(
        "INSERT INTO shop.sale_event (network, block_number, log_index, block_time, kind, emitter, contract_address, \
             token_id, bid_id, buyer, price, expires_at, tx_hash) \
         VALUES ('matic', 50, 0, 50, 'bid_created', $1, $2, 6, $3, $4, 600, $5, '0x' || repeat('3', 64)), \
                ('matic', 220, 0, 220, 'bid_cancelled', $1, $2, 6, $3, $4, NULL, NULL, '0x' || repeat('4', 64))",
    )
    .bind(super::sales_index::BID_V2_MATIC)
    .bind(&c)
    .bind(format!("0x{:064x}", 650))
    .bind(CARL)
    .bind(later)
    .execute(&pool)
    .await
    .unwrap();
    let legacy_owner =
        super::chain_walk::test_owner(&pool, &ix, &state.settings.market.indexes).await;
    let mut tx = pool.begin().await.unwrap();
    super::sales_index::fold_in(&mut tx, Some(210), &legacy_owner)
        .await
        .unwrap();
    legacy_owner.check(&mut tx).await.unwrap();
    tx.commit().await.unwrap();
    let cancelled =
        format!("/v1/bids?network=MATIC&contractAddress={c}&tokenId=6&status=cancelled");
    let (status, body) = get(app(), &cancelled).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["source"], "sales-index", "{body}");

    super::chain_walk::record_fold(&pool, &legacy_owner, 220)
        .await
        .unwrap();
    assert!(
        super::chain_walk::mark_folded(&pool, &ix, &legacy_owner, 220, chrono::Utc::now())
            .await
            .unwrap()
    );
    let (status, body) = get(app(), &cancelled).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        ids(&body),
        vec![("6".into(), "cancelled".into(), CREATOR.into())]
    );
    assert_eq!(body["data"]["total"], 1, "{body}");

    let (status, body) = get(app(), &format!("{sold}&sortBy=recently_updated&limit=1")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["results"][0]["tokenId"], "1", "{body}");
    assert_eq!(body["data"]["total"], 2, "{body}");

    // ERC721BidV2 price and expiry are full-width on chain. The SQL window
    // and final merge must rank the same bid first for every requested limit.
    const WIDE_PRICE: &str = "340282366920938463463374607431768211456";
    sqlx::query(
        "UPDATE shop.nft_bid SET price = $2::numeric, expires_at = $3 \
         WHERE network = 'matic' AND contract_address = $1 AND token_id = 1",
    )
    .bind(&c)
    .bind(WIDE_PRICE)
    .bind(i64::MAX)
    .execute(&pool)
    .await
    .unwrap();
    // A fold can write a bid at the same height as the current publication.
    // A matching height is insufficient until the writer republishes it.
    let (status, body) = get(app(), sold).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["source"], "sales-index", "{body}");
    assert!(
        super::chain_walk::mark_folded(&pool, &ix, &legacy_owner, 220, chrono::Utc::now())
            .await
            .unwrap()
    );
    for limit in [1, 2] {
        let url = format!("/v1/bids?network=MATIC&status=sold&sortBy=most_expensive&limit={limit}");
        let (status, body) = get(app(), &url).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["data"]["total"], 2, "{body}");
        let results = body["data"]["results"].as_array().unwrap();
        assert_eq!(results.len(), limit);
        assert_eq!(results[0]["tokenId"], "1", "{body}");
        assert_eq!(results[0]["price"], WIDE_PRICE, "{body}");
        assert_eq!(results[0]["expiresAt"], i64::MAX, "{body}");
    }
    sqlx::raw_sql(
        "ALTER TABLE shop.bid_seller_recheck RENAME TO saved_bid_seller_recheck;
         CREATE FUNCTION shop.bid_snapshot_error() RETURNS bigint LANGUAGE plpgsql AS $$
         BEGIN RAISE EXCEPTION 'private fixture detail' USING ERRCODE = '57014'; END;
         $$;
         CREATE VIEW shop.bid_seller_recheck AS
           SELECT 'matic'::text AS network, shop.bid_snapshot_error() AS from_block;",
    )
    .execute(&pool)
    .await
    .unwrap();
    let (status, body) = get(app(), sold).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    let message = body["message"].as_str().unwrap();
    assert!(message.contains("statement bound"), "{body}");
    assert!(!message.contains("private fixture detail"), "{body}");

    sqlx::raw_sql(
        "CREATE OR REPLACE FUNCTION shop.bid_snapshot_error() RETURNS bigint LANGUAGE plpgsql AS $$
         BEGIN RAISE EXCEPTION 'private fixture detail'; END;
         $$;",
    )
    .execute(&pool)
    .await
    .unwrap();
    let (status, body) = get(app(), sold).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    assert_eq!(body["message"], "database error", "{body}");

    own.drop().await;
}

/// On the Sepolia/Amoy pair a walking node with the sales index serves
/// Polygon's legacy bids from the book, never from the subgraph: before
/// publication the read names the sales index; after it, only bids on the
/// collections the nft index walks on Amoy (of that chain and verified on
/// it), with Amoy's `chainId` and bid contract.
#[tokio::test]
async fn amoy_bids_come_from_the_bid_book_of_the_served_catalogue() {
    use super::chain_walk::ChainNet;
    let up = FakeGraph::new();
    let url = serve_graph(up.clone()).await;
    let Some(own) = super::tests::support::OwnDb::create("amoy_bids").await else {
        return;
    };
    publish_bid_collections(&own.pool, 80_002).await;
    let mut settings = super::owned_items::test_support::walking(market_settings(&url, &url, true));
    settings.eth_network = "sepolia".into();
    settings.market.indexes.chain_id_ethereum = ChainNet::Ethereum.testnet_chain_id();
    settings.market.indexes.chain_id_matic = ChainNet::Matic.testnet_chain_id();
    settings.market.indexes.sales_enabled = true;
    let state = Arc::new(super::MarketplaceState::new(
        own.pool.clone(),
        crate::http::Outbound::for_tests(),
        settings,
    ));
    state.test_allow_loopback.store(true, Ordering::Relaxed);
    assert!(super::bids::polygon_book_here(&state));
    let pool = own.pool.clone();
    super::trade_events::tests::publish_cursors(&pool).await;
    let app = || market_router(state.clone());
    let uri = "/v1/bids?network=MATIC";
    let (status, body) = get(app(), uri).await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("sales-index")),
        "{body}"
    );
    for indexer in ["sales", "nft"] {
        sqlx::query(
            "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, last_block_hash, fold_through, folded_block, folded_at, full_history, chain_id) \
             VALUES ($1, 'matic', 1, 200, '0x' || repeat('0', 64), 200, 200, timezone('UTC', now()), true, 80002)",
        )
        .bind(indexer)
        .execute(&pool)
        .await
        .unwrap();
    }
    let t = tag();
    let (verified, unverified) = (contract_of(&t, 1), contract_of(&t, 2));
    for (contract, checked, bid) in [(&verified, true, 10i64), (&unverified, false, 11)] {
        sqlx::query(
            "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, is_completed, is_approved, is_editable, created_at, updated_at, reviewed_at, mirrored_at, testnet_verified) \
             SELECT 'matic', $1, 80002, 'Pirates', $2, $2, 'urn:decentraland:amoy:collections-v2:' || $1, true, true, false, 1, 1, 1, timezone('UTC', now()), $3",
        )
        .bind(contract)
        .bind(CREATOR)
        .bind(checked)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO shop.nft_bid (network, contract_address, token_id, bidder, bid_id, seller, price, expires_at, status, \
                 created_block, created_log_index, created_at, updated_block, updated_log_index, updated_at) \
             VALUES ('matic', $1, 1, $2, '0x' || lpad(to_hex($5), 64, '0'), $3, 100, $4, 'open', 10, 0, 10, 10, 0, 10)",
        )
        .bind(contract)
        .bind(WALLET_B)
        .bind(WALLET_A)
        .bind(chrono::Utc::now().timestamp() + 86_400)
        .bind(bid)
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "UPDATE shop.chain_cursor SET folded_at = timezone('UTC', now()) \
         WHERE indexer = 'sales' AND network = 'matic'",
    )
    .execute(&pool)
    .await
    .unwrap();
    let (status, body) = get(app(), uri).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let results = body["data"]["results"].as_array().unwrap();
    assert_eq!(
        results
            .iter()
            .map(|b| (
                b["contractAddress"].as_str().unwrap().to_string(),
                b["chainId"].as_i64(),
                b["bidAddress"].as_str().map(str::to_string),
            ))
            .collect::<Vec<_>>(),
        vec![(
            verified.clone(),
            Some(80_002),
            super::sales_index::SalesIndex::for_chain(80_002).map(|i| i.bid_contract().to_string()),
        )],
        "only the verified Amoy collection, on Amoy: {body}"
    );
    assert_eq!(body["data"]["total"], 1, "{body}");
    assert!(
        !up.requests.lock().unwrap().iter().any(|e| e == "bids"),
        "no Polygon bid read asks the subgraph: {:?}",
        up.requests.lock().unwrap()
    );
    // Ethereum's legacy bids still come from the subgraph here: labelled
    // with the chain the node serves (Sepolia), never the alias's default.
    let far = (chrono::Utc::now().timestamp() + 86_400) * 1000;
    up.set(
        "bids",
        vec![json!({
            "id": "0xparcel-1-0xbidder", "bidAddress": "0xbid", "nftAddress": "0xparcel", "tokenId": "1",
            "bidder": WALLET_B, "seller": WALLET_A, "price": "5", "status": "open", "blockchainId": "0x01",
            "blockNumber": "1", "expiresAt": far.to_string(), "createdAt": "1", "updatedAt": "1"
        })],
    );
    let (status, body) = get(app(), "/v1/bids").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let mut chains: Vec<(String, Option<i64>)> = body["data"]["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| {
            (
                b["network"].as_str().unwrap().to_string(),
                b["chainId"].as_i64(),
            )
        })
        .collect();
    chains.sort();
    assert_eq!(
        chains,
        vec![
            ("ETHEREUM".to_string(), Some(11_155_111)),
            ("MATIC".to_string(), Some(80_002)),
        ],
        "{body}"
    );
    own.drop().await;
}

#[tokio::test]
async fn mirrored_land_and_estate_bids_use_the_land_owner_without_an_nft_publication() {
    let graph = serve_empty_graph().await;
    let Some((state, own)) = state_with_the_bid_book(&graph).await else {
        return;
    };
    let pool = &own.pool;
    use crate::land_authz::LAND_MAINNET;

    super::trade_events::tests::publish_cursors(pool).await;
    sqlx::query(
        "INSERT INTO land_authz.index_cursor (network, land_registry, estate_registry, last_block, \
                                             folded_block, folded_at, full_replay_started) \
         VALUES ($1, $2, $3, 100, 100, timezone('UTC', now()), true)",
    )
    .bind(LAND_MAINNET.network)
    .bind(LAND_MAINNET.land_registry)
    .bind(LAND_MAINNET.estate_registry)
    .execute(pool)
    .await
    .unwrap();
    let graph_backed = Arc::new(super::MarketplaceState::new(
        pool.clone(),
        crate::http::Outbound::for_tests(),
        market_settings(&graph, &graph, true),
    ));
    graph_backed
        .test_allow_loopback
        .store(true, Ordering::Relaxed);
    assert!(!super::owned_items::walks_here(&graph_backed));
    let apps = [market_router(state), market_router(graph_backed)];
    for (contract, token, owner, x, y) in [
        (
            LAND_MAINNET.land_registry,
            77i64,
            WALLET_A,
            Some(0),
            Some(77),
        ),
        (LAND_MAINNET.estate_registry, 500, WALLET_B, None, None),
    ] {
        sqlx::query(
            "INSERT INTO land_authz.token_right (token_address, token_id, x, y, owner, updated_block, updated_log) \
             VALUES ($1, $2::bigint, $3, $4, $5, 100, 0)",
        )
        .bind(contract)
        .bind(token)
        .bind(x)
        .bind(y)
        .bind(owner)
        .execute(pool)
        .await
        .unwrap();
        // A row under the LAND registry must not displace the LAND owner.
        sqlx::query(
            "INSERT INTO shop.nft (network, contract_address, token_id, owner, minted_block, minted_at, \
                                   transferred_block, transferred_log, transferred_at) \
             VALUES ('ethereum', $1, $2::bigint, $3, 10, 10, 100, 0, 100)",
        )
        .bind(contract)
        .bind(token)
        .bind(CREATOR)
        .execute(pool)
        .await
        .unwrap();
        let (trade, _) = insert_trade_on(
            pool,
            "ETHEREUM",
            1,
            "bid",
            CREATOR,
            Asset {
                asset_type: 1,
                contract: "0x2222222222222222222222222222222222222222".into(),
                value: "100".into(),
            },
            Asset {
                asset_type: 3,
                contract: contract.into(),
                value: token.to_string(),
            },
        )
        .await;
        let url = format!("/v1/bids?network=ETHEREUM&contractAddress={contract}&seller={owner}");
        sqlx::query("UPDATE land_authz.index_cursor SET folded_at = timezone('UTC', now()) - interval '2 hours'")
            .execute(pool)
            .await
            .unwrap();
        for app in &apps {
            let (status, body) = get(app.clone(), &url).await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
            assert_eq!(body["source"], "land-authz-index", "{body}");
        }
        sqlx::query("UPDATE land_authz.index_cursor SET folded_at = timezone('UTC', now())")
            .execute(pool)
            .await
            .unwrap();
        for app in &apps {
            let (status, body) = get(app.clone(), &url).await;
            assert_eq!(status, StatusCode::OK, "{body}");
            assert_eq!(body["data"]["total"], 1, "{body}");
            assert_eq!(body["data"]["results"][0]["tradeId"], trade, "{body}");
            assert_eq!(body["data"]["results"][0]["seller"], owner, "{body}");
        }
    }

    let land = LAND_MAINNET.land_registry;
    let url =
        format!("/v1/bids?network=ETHEREUM&contractAddress={land}&itemId=0&seller={WALLET_A}");
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE shop.projection_write IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *lock)
        .await
        .unwrap();
    let (status, body) = get(apps[0].clone(), &url).await;
    lock.rollback().await.unwrap();
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["total"], 1, "{body}");
    assert_eq!(body["data"]["results"][0]["seller"], WALLET_A, "{body}");

    let mut lock = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE land_authz.index_cursor IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *lock)
        .await
        .unwrap();
    let (status, body) = get(apps[1].clone(), &url).await;
    lock.rollback().await.unwrap();
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["source"], "land-authz-index", "{body}");

    // A graph-backed reader can be configured for a different LAND chain
    // from the chain its local trade mirror serves.
    let sepolia = crate::land_authz::LAND_SEPOLIA;
    sqlx::query(
        "UPDATE land_authz.index_cursor SET network = $1, land_registry = $2, estate_registry = $3, \
             folded_at = timezone('UTC', now())",
    )
    .bind(sepolia.network)
    .bind(sepolia.land_registry)
    .bind(sepolia.estate_registry)
    .execute(pool)
    .await
    .unwrap();
    let wrong_land_settings = Settings {
        eth_network: "sepolia".into(),
        eth_collections_subgraph_url: Some(graph),
        data_writes_enabled: true,
        ..Settings::for_tests()
    };
    wrong_land_settings.check_writer_pair().unwrap();
    let wrong_land = Arc::new(super::MarketplaceState::new(
        pool.clone(),
        crate::http::Outbound::for_tests(),
        wrong_land_settings,
    ));
    wrong_land
        .test_allow_loopback
        .store(true, Ordering::Relaxed);
    assert!(!super::owned_items::walks_here(&wrong_land));
    let app = market_router(wrong_land);
    let (status, body) = get(app.clone(), &url).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["source"], "land-authz-index", "{body}");
    let (status, body) = get(app, &format!("{url}&tokenId=78")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["total"], 0, "{body}");
    own.drop().await;
}

#[tokio::test]
async fn an_ethereum_nft_bid_read_skips_the_land_cursor() {
    let graph = serve_empty_graph().await;
    let Some((state, own)) = state_with_the_bid_book(&graph).await else {
        return;
    };
    let pool = &own.pool;
    super::trade_events::tests::publish_cursors(pool).await;
    let contract = contract_of(&tag(), 1);
    sqlx::query(
        "INSERT INTO shop.nft (network, contract_address, token_id, owner, minted_block, minted_at, \
                               transferred_block, transferred_log, transferred_at) \
         VALUES ('ethereum', $1, 1, $2, 10, 10, 100, 0, 100)",
    )
    .bind(&contract)
    .bind(WALLET_A)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, \
                                      fold_through, folded_block, folded_at, full_history, chain_id) \
         VALUES ('nft', 'ethereum', 1, 100, 100, 100, timezone('UTC', now()), true, 1)",
    )
    .execute(pool)
    .await
    .unwrap();
    let (trade, _) = insert_trade_on(
        pool,
        "ETHEREUM",
        1,
        "bid",
        CREATOR,
        Asset {
            asset_type: 1,
            contract: "0x2222222222222222222222222222222222222222".into(),
            value: "100".into(),
        },
        Asset {
            asset_type: 3,
            contract: contract.clone(),
            value: "1".into(),
        },
    )
    .await;
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE land_authz.index_cursor IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *lock)
        .await
        .unwrap();
    let (status, body) = get(
        market_router(state),
        &format!("/v1/bids?network=ETHEREUM&contractAddress={contract}&seller={WALLET_A}"),
    )
    .await;
    lock.rollback().await.unwrap();
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["total"], 1, "{body}");
    assert_eq!(body["data"]["results"][0]["tradeId"], trade, "{body}");
    assert_eq!(body["data"]["results"][0]["seller"], WALLET_A, "{body}");
    own.drop().await;
}

#[tokio::test]
async fn a_mirrored_token_bid_waits_for_its_sellers_nft_publication() {
    let graph = serve_empty_graph().await;
    let Some((state, own)) = state_with_the_bid_book(&graph).await else {
        return;
    };
    let pool = &own.pool;
    super::trade_events::tests::publish_cursors(pool).await;
    let c = contract_of(&tag(), 1);
    sqlx::query(
        "INSERT INTO shop.nft (network, contract_address, token_id, owner, minted_block, minted_at, \
                               transferred_block, transferred_log, transferred_at) \
         VALUES ('matic', $1, 1, $2, 10, 10, 100, 0, 100)",
    )
    .bind(&c)
    .bind(WALLET_A)
    .execute(pool)
    .await
    .unwrap();
    for indexer in ["nft", "sales"] {
        sqlx::query(
            "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, last_block_hash, \
                                          fold_through, folded_block, folded_at, full_history, chain_id) \
             VALUES ($1, 'matic', 1, 200, '0x' || repeat('0', 64), 200, 200, timezone('UTC', now()), true, 137)",
        )
        .bind(indexer)
        .execute(pool)
        .await
        .unwrap();
    }
    let (trade, _) = insert_trade(
        pool,
        "bid",
        CREATOR,
        Asset {
            asset_type: 1,
            contract: "0x2222222222222222222222222222222222222222".into(),
            value: "100".into(),
        },
        Asset {
            asset_type: 3,
            contract: c.clone(),
            value: "1".into(),
        },
    )
    .await;
    let url = |seller| format!("/v1/bids?network=MATIC&contractAddress={c}&seller={seller}");
    let app = market_router(state);
    let (status, body) = get(app.clone(), &url(WALLET_A)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["total"], 1, "{body}");
    assert_eq!(body["data"]["results"][0]["tradeId"], trade, "{body}");
    let (status, body) = get(app.clone(), &url(WALLET_B)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["total"], 0, "{body}");

    sqlx::query(
        "UPDATE shop.chain_cursor SET last_block = 201 WHERE indexer = 'nft' AND network = 'matic'",
    )
    .execute(pool)
    .await
    .unwrap();
    let (status, body) = get(app.clone(), &url(WALLET_A)).await;
    assert_eq!(status, StatusCode::OK, "raw advance: {body}");
    assert_eq!(body["data"]["total"], 1, "{body}");
    let publish = || {
        sqlx::query(
            "UPDATE shop.chain_cursor SET fold_through = last_block, folded_block = last_block, \
                 folded_at = timezone('UTC', now()) WHERE indexer = 'nft' AND network = 'matic'",
        )
        .execute(pool)
    };
    publish().await.unwrap();
    sqlx::query("UPDATE shop.nft SET owner = $2 WHERE network = 'matic' AND contract_address = $1 AND token_id = 1")
        .bind(&c)
        .bind(WALLET_B)
        .execute(pool)
        .await
        .unwrap();

    // Both seller scopes wait for the NFT publication, even when the
    // pending owner would remove the only bid from the previous seller.
    let recorder = dcl_metrics::Registry::new(&[super::bids::FAMILIES])
        .unwrap()
        .builder(dcl_metrics::PrometheusBuilder::new())
        .unwrap()
        .build_recorder();
    let handle = recorder.handle();
    let recording = dcl_metrics::set_default_local_recorder(&recorder);
    for seller in [WALLET_A, WALLET_B] {
        let (status, body) = get(app.clone(), &url(seller)).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
        assert_eq!(body["source"], "nft-index", "{body}");
    }
    drop(recording);
    let metrics = handle.render();
    assert!(
        metrics.contains("catalyst_shop_bid_nft_unavailable_reads_total{network=\"matic\"} 2"),
        "{metrics}"
    );
    publish().await.unwrap();
    let (status, body) = get(app.clone(), &url(WALLET_A)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["total"], 0, "{body}");
    let (status, body) = get(app.clone(), &url(WALLET_B)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["total"], 1, "{body}");
    assert_eq!(body["data"]["results"][0]["tradeId"], trade, "{body}");
    assert_eq!(body["data"]["results"][0]["seller"], WALLET_B, "{body}");
    // Token 1 derives item 0. The itemId filter skips the legacy book,
    // but it still matches this ERC721 bid and needs its seller's NFT fold.
    let item = format!("{}&itemId=0", url(WALLET_B));
    sqlx::query(
        "UPDATE shop.chain_cursor SET folded_at = timezone('UTC', now()) - interval '2 hours' \
         WHERE indexer = 'nft' AND network = 'matic'",
    )
    .execute(pool)
    .await
    .unwrap();
    let (status, body) = get(app.clone(), &item).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["source"], "nft-index", "{body}");

    sqlx::query(
        "UPDATE shop.chain_cursor SET chain_id = 80002 WHERE indexer = 'nft' AND network = 'matic'",
    )
    .execute(pool)
    .await
    .unwrap();
    publish().await.unwrap();
    let (status, body) = get(app.clone(), &item).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["source"], "nft-index", "{body}");
    sqlx::query(
        "UPDATE shop.chain_cursor SET chain_id = 137 WHERE indexer = 'nft' AND network = 'matic'",
    )
    .execute(pool)
    .await
    .unwrap();
    publish().await.unwrap();
    let (status, body) = get(app, &item).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["total"], 1, "{body}");
    assert_eq!(body["data"]["results"][0]["seller"], WALLET_B, "{body}");

    own.drop().await;
}

#[tokio::test]
async fn item_only_bids_on_a_walker_do_not_require_an_ethereum_graph() {
    let Some(own) = super::tests::support::OwnDb::create("item_only_bids").await else {
        return;
    };
    let url = serve_empty_graph().await;
    let mut settings = super::owned_items::test_support::walking(market_settings(&url, &url, true));
    settings.eth_collections_subgraph_url = None;
    settings.market.indexes.sales_enabled = true;
    let state = Arc::new(super::MarketplaceState::new(
        own.pool.clone(),
        crate::http::Outbound::for_tests(),
        settings,
    ));
    state.test_allow_loopback.store(true, Ordering::Relaxed);
    assert!(super::bids::polygon_book_here(&state));
    super::trade_events::tests::publish_cursors(&own.pool).await;
    let (trade, _) = insert_trade(
        &own.pool,
        "bid",
        WALLET_B,
        Asset {
            asset_type: 1,
            contract: "0x2222222222222222222222222222222222222222".into(),
            value: "9000000000000000000".into(),
        },
        Asset {
            asset_type: 4,
            contract: contract_of(&tag(), 2),
            value: "3".into(),
        },
    )
    .await;
    let (status, body) = get(market_router(state), "/v1/bids?itemId=3").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["total"], 1, "{body}");
    assert_eq!(body["data"]["results"][0]["tradeId"], trade, "{body}");
    own.drop().await;
}

/// A mirror bid on an item takes its seller (the item's creator) only from
/// an item of the trade's chain, verified there on a testnet: an item row of
/// another chain under the same network, contract and id names no seller.
/// On a node that does not walk the indexes the readiness and the rows are
/// one short snapshot read beside the subgraph.
#[tokio::test]
async fn an_item_bid_takes_its_creator_from_an_item_of_its_chain() {
    let Some(own) = super::tests::support::OwnDb::create("item_bid_creator").await else {
        return;
    };
    let url = serve_empty_graph().await;
    let state = Arc::new(super::MarketplaceState::new(
        own.pool.clone(),
        crate::http::Outbound::for_tests(),
        market_settings(&url, &url, true),
    ));
    state.test_allow_loopback.store(true, Ordering::Relaxed);
    assert!(!super::owned_items::walks_here(&state));
    let pool = own.pool.clone();
    super::trade_events::tests::publish_cursors(&pool).await;
    let c = contract_of(&tag(), 1);
    insert_trade(
        &pool,
        "bid",
        WALLET_B,
        Asset {
            asset_type: 1,
            contract: "0x2222222222222222222222222222222222222222".into(),
            value: "9000000000000000000".into(),
        },
        Asset {
            asset_type: 4,
            contract: c.clone(),
            value: "3".into(),
        },
    )
    .await;
    let mut sellers = Vec::new();
    for chain in [80_002, 137] {
        sqlx::query(
            "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, total_supply, max_supply, available, \
                 price, name, category, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ('matic', $1, 3, $2, 'wearable_v2', 'urn:decentraland:matic:collections-v2:' || $1 || ':3', $3, 'common', 1, 1, 0, 0, 'Hat', 'hat', 1, 1, 1, timezone('UTC', now())) \
             ON CONFLICT (network, contract_address, item_id) DO UPDATE SET chain_id = EXCLUDED.chain_id",
        )
        .bind(&c)
        .bind(chain)
        .bind(CREATOR)
        .execute(&pool)
        .await
        .unwrap();
        let (status, body) = get(
            market_router(state.clone()),
            &format!("/v1/bids?bidder={WALLET_B}"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        sellers.push(
            body["data"]["results"][0]["seller"]
                .as_str()
                .map(str::to_string),
        );
    }
    assert_eq!(
        sellers,
        vec![Some(String::new()), Some(CREATOR.to_string())],
        "an Amoy item row names no seller for a chain-137 bid; the chain-137 row does"
    );
    own.drop().await;
}

#[tokio::test]
async fn an_amoy_item_bid_requires_a_verified_amoy_creator() {
    // The served chain pair is process-wide. Install the testnet pair in
    // an isolated test process so concurrent mainnet tests keep their pair.
    const CHILD: &str = "CATALYST_AMOY_ITEM_BID_TEST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                concat!(
                    module_path!(),
                    "::an_amoy_item_bid_requires_a_verified_amoy_creator"
                )
                .trim_start_matches(concat!(env!("CARGO_CRATE_NAME"), "::")),
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stdout).contains("1 passed"));
        return;
    }
    let Some(own) = super::tests::support::OwnDb::create("amoy_item_bid").await else {
        return;
    };
    let url = serve_empty_graph().await;
    let mut settings = market_settings(&url, &url, true);
    settings.eth_network = "sepolia".into();
    settings.market.indexes.chain_id_ethereum = 11_155_111;
    settings.market.indexes.chain_id_matic = 80_002;
    super::catalog::install_served_chains(&settings.market.indexes);
    let state = Arc::new(super::MarketplaceState::new(
        own.pool.clone(),
        crate::http::Outbound::for_tests(),
        settings,
    ));
    state.test_allow_loopback.store(true, Ordering::Relaxed);
    assert!(!super::owned_items::walks_here(&state));

    super::trade_events::tests::publish_cursors(&own.pool).await;
    sqlx::query(
        "UPDATE shop.chain_cursor SET chain_id = CASE network WHEN 'ethereum' THEN 11155111 ELSE 80002 END, \
             sepolia_trade_v1 = network = 'ethereum' \
         WHERE indexer = 'trade_events'",
    )
    .execute(&own.pool)
    .await
    .unwrap();
    let contract = contract_of(&tag(), 1);
    let (trade, _) = insert_trade_on(
        &own.pool,
        "MATIC",
        80_002,
        "bid",
        WALLET_B,
        Asset {
            asset_type: 1,
            contract: "0x2222222222222222222222222222222222222222".into(),
            value: "9000000000000000000".into(),
        },
        Asset {
            asset_type: 4,
            contract: contract.clone(),
            value: "3".into(),
        },
    )
    .await;
    let url = format!("/v1/bids?network=MATIC&bidder={WALLET_B}");
    // This adversarial row carries a verification flag on the wrong chain.
    // Each negative fixture isolates one half of the seller join.
    for (chain, verified, seller, total) in [
        (137, true, "", 0),
        (80_002, false, "", 0),
        (80_002, true, CREATOR, 1),
    ] {
        sqlx::query(
            "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, total_supply, max_supply, available, \
                 price, name, category, created_at, updated_at, reviewed_at, mirrored_at, testnet_verified) \
             VALUES ('matic', $1, 3, $2, 'wearable_v2', 'urn:decentraland:amoy:collections-v2:' || $1 || ':3', $3, \
                 'common', 1, 1, 0, 0, 'Hat', 'hat', 1, 1, 1, timezone('UTC', now()), $4) \
             ON CONFLICT (network, contract_address, item_id) DO UPDATE SET chain_id = EXCLUDED.chain_id, \
                 testnet_verified = EXCLUDED.testnet_verified",
        )
        .bind(&contract)
        .bind(chain)
        .bind(CREATOR)
        .bind(verified)
        .execute(&own.pool)
        .await
        .unwrap();
        let (status, body) = get(market_router(state.clone()), &url).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["data"]["results"][0]["tradeId"], trade, "{body}");
        assert_eq!(body["data"]["results"][0]["chainId"], 80_002, "{body}");
        assert_eq!(body["data"]["results"][0]["seller"], seller, "{body}");

        let (status, body) = get(
            market_router(state.clone()),
            &format!("{url}&seller={CREATOR}"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["data"]["total"], total, "{body}");
        assert_eq!(body["data"]["results"].as_array().unwrap().len(), total);
    }
    own.drop().await;
}

#[tokio::test]
async fn a_graph_backed_bids_page_releases_its_local_snapshot_before_the_graph() {
    let Some(own) = super::tests::support::OwnDb::create("graph_bid_snapshot").await else {
        return;
    };
    let graph = FakeGraph::new();
    let url = serve_graph(graph.clone()).await;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with(own.pool.connect_options().as_ref().clone())
        .await
        .unwrap();
    let state = Arc::new(super::MarketplaceState::new(
        pool.clone(),
        crate::http::Outbound::for_tests(),
        market_settings(&url, &url, true),
    ));
    state.test_allow_loopback.store(true, Ordering::Relaxed);
    assert!(!super::owned_items::walks_here(&state));
    super::trade_events::tests::publish_cursors(&pool).await;
    let (trade, _) = insert_trade(
        &pool,
        "bid",
        WALLET_B,
        Asset {
            asset_type: 1,
            contract: "0x2222222222222222222222222222222222222222".into(),
            value: "9000000000000000000".into(),
        },
        Asset {
            asset_type: 4,
            contract: contract_of(&tag(), 2),
            value: "3".into(),
        },
    )
    .await;

    let uri = "/v1/bids?network=MATIC";
    let app = market_router(state);
    graph.pause.store(true, Ordering::SeqCst);
    let request = tokio::spawn(get(app.clone(), uri));
    tokio::time::timeout(Duration::from_secs(5), graph.arrived.notified())
        .await
        .expect("the page reached the legacy bids subgraph");
    let quarantine = tokio::time::timeout(
        Duration::from_secs(5),
        sqlx::query(
            "UPDATE shop.chain_cursor SET quarantined_at = timezone('UTC', now()), quarantine_reason = 'test' \
             WHERE indexer = 'trade_events' AND network = 'matic'",
        )
        .execute(&pool),
    )
    .await;
    graph.release.notify_one();
    quarantine
        .expect("the only pool connection was released before awaiting the subgraph")
        .unwrap();

    let (status, body) = request.await.unwrap();
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["total"], 1, "{body}");
    assert_eq!(body["data"]["results"][0]["tradeId"], trade, "{body}");

    let (status, body) = get(app, uri).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["source"], "trade-events-index", "{body}");
    pool.close().await;
    own.drop().await;
}

/// The bids of a node that walks the indexes read one snapshot: the trades
/// mirror's readiness and its bids are read on it, so a trade-events rewind
/// that lands between them neither drops nor keeps a bid against what the
/// check saw; the next read answers the envelope.
#[tokio::test]
async fn the_bids_legs_read_the_snapshot_their_readiness_checked() {
    let url = serve_empty_graph().await;
    let Some((state, own)) = state_with_the_bid_book(&url).await else {
        return;
    };
    super::trade_events::tests::publish_cursors(&state.pool).await;
    let pool = state.pool.clone();
    // The token-bid leg can rely on an empty, published NFT projection.
    sqlx::query(
        "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, \
                                      fold_through, folded_block, folded_at, full_history, chain_id) \
         VALUES ('nft', 'matic', 1, 200, 200, 200, timezone('UTC', now()), true, 137)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let c = contract_of(&tag(), 8);
    let (trade, _) = insert_trade(
        &pool,
        "bid",
        WALLET_B,
        Asset {
            asset_type: 1,
            contract: "0x2222222222222222222222222222222222222222".into(),
            value: "9000000000000000000".into(),
        },
        Asset {
            asset_type: 3,
            contract: c.clone(),
            value: "1".into(),
        },
    )
    .await;
    let f = super::bids::parse_filters(&[("network".to_string(), "MATIC".to_string())]).unwrap();
    let (mut tx, bounds, now) = match super::bids::bids_snapshot(&state, false).await {
        Ok(snapshot) => snapshot,
        Err(r) => panic!("the trades mirror is ready: {}", r.status()),
    };
    assert!(bounds.is_none());
    sqlx::query(
        "UPDATE shop.chain_cursor SET quarantined_at = timezone('UTC', now()), quarantine_reason = 'test' \
         WHERE indexer = 'trade_events' AND network = 'matic'",
    )
    .execute(&pool)
    .await
    .unwrap();
    let rows = super::bids::trades_rows(&mut tx, &state, &f, now, None)
        .await
        .unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(
        rows.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(),
        vec![trade.as_str()],
        "the rows are the checked snapshot's"
    );
    let (status, body) = get(market_router(state.clone()), "/v1/bids?network=MATIC").await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("trade-events-index")),
        "{body}"
    );
    own.drop().await;
}

/// A shallow NAME page reads the index on a walker whose Ethereum latest
/// sales are admitted, whatever its sort: the graph ranks the two names one
/// way by its own `soldAt`, the ledger the other, and each token shows one
/// soldAt across sorts. A failed admission check is the page's error with
/// no graph request; confirmed not admitted, the page reads the graph.
#[tokio::test]
async fn a_names_page_by_latest_sale_reads_the_admitted_index() {
    let graph = FakeGraph::new();
    let url = serve_graph(graph.clone()).await;
    let Some((mut state, own)) = state_over_own(&url, false).await else {
        return;
    };
    let settings = &mut Arc::get_mut(&mut state).unwrap().settings;
    settings.market.indexes.sales_enabled = true;
    settings.marketplace_subgraph_url = Some(url.clone());
    // A configured trades mirror that has not completed a pass.
    settings.trades_sync_upstream_url = Some("https://marketplace-api.example/v1/trades".into());
    let pool = &own.pool;
    let registrar = super::nft_index::DCL_REGISTRAR;
    let owner = "0x00000000000000000000000000000000000000aa";
    let name = |token: &str, label: &str, sold: &str| {
        json!({ "id": format!("{registrar}-{token}"), "tokenId": token, "contractAddress": registrar,
                "category": "ens", "image": "", "createdAt": "1", "updatedAt": "1", "soldAt": sold,
                "name": label, "owner": { "address": owner }, "ens": { "subdomain": label },
                "activeOrder": null })
    };
    // The graph sold alpha last; the ledger sold beta last.
    graph.set(
        "nfts",
        vec![name("1001", "alpha", "2000"), name("1002", "beta", "1000")],
    );
    let (_, deployment, _) = super::trade_events::deployments(1)[0];
    let height = deployment as i64 + 1;
    sqlx::query(
        "INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,last_block_hash,
          full_history,fold_through,folded_block,folded_at,sale_input_version,sale_verified_through,sale_covered_rewinds)
         SELECT ix,'ethereum',1,1,$1,$2,true,$1,$1,timezone('UTC',now()),$3,$1,0
         FROM unnest(ARRAY['nft','sales','trade_events']) ix",
    )
    .bind(height)
    .bind(format!("0x{:064x}", 91))
    .bind(super::sale_history::VERSION)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO shop.sale_publication(network,block_number,block_hash,input_version,input_epochs,
            input_revisions,late_collections,published_at,last_sale_through,generation,last_sale_generation)
         SELECT network,$1,$2,$3,array_agg(sale_input_epoch ORDER BY indexer),
                array_agg(sale_history_revision ORDER BY indexer),0,timezone('UTC',now()),$1,
                '00000000-0000-4000-8000-000000000001','00000000-0000-4000-8000-000000000001'
         FROM shop.chain_cursor WHERE indexer IN ('nft','sales','trade_events') GROUP BY network",
    )
    .bind(height)
    .bind(format!("0x{:064x}", 91))
    .bind(super::sale_fold::PUBLICATION_VERSION)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO shop.nft (network, contract_address, token_id, owner, ens_subdomain, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
         VALUES ('ethereum', $1, 1001, $2, 'alpha', 1, 1, 1, 0, 1), ('ethereum', $1, 1002, $2, 'beta', 2, 2, 2, 0, 2); \
         ",
    )
    .bind(registrar)
    .bind(owner)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO shop.nft_last_sale (network, contract_address, token_id, sold_at, block_number) \
         VALUES ('ethereum', $1, 1001, 500, 1), ('ethereum', $1, 1002, 3000, 2)",
    )
    .bind(registrar)
    .execute(pool)
    .await
    .unwrap();
    let app = market_router(state.clone());
    let sold_at = |body: &Value, token: &str| {
        body["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["nft"]["tokenId"] == token)
            .and_then(|r| r["nft"]["soldAt"].as_i64())
    };
    let graph_reads = || {
        graph
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|e| *e == "nfts")
            .count()
    };
    let by_sale = "/v1/nfts?category=ens&sortBy=recently_sold&first=2";
    let browse = "/v1/nfts?category=ens&first=2";
    // Admitted: every shallow NAME page reads the index, so a token shows
    // the same soldAt whatever the sort.
    let (status, sorted) = get(app.clone(), by_sale).await;
    assert_eq!(status, StatusCode::OK, "{sorted}");
    assert_eq!(sorted["data"][0]["nft"]["tokenId"], "1002", "{sorted}");
    let (status, browsed) = get(app.clone(), browse).await;
    assert_eq!(status, StatusCode::OK, "{browsed}");
    for token in ["1001", "1002"] {
        assert_eq!(sold_at(&sorted, token), sold_at(&browsed, token), "{token}");
    }
    assert_eq!(sold_at(&browsed, "1002"), Some(3_000_000));
    assert_eq!(graph_reads(), 0);

    // An admitted page keeps the index path's rules: an order filter waits
    // for the trades mirror's pass, and the `first + skip` window is capped.
    let (status, body) = get(app.clone(), "/v1/nfts?category=ens&isOnSale=true&first=2").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["source"], "trades-mirror", "{body}");
    let (status, body) = get(app.clone(), "/v1/nfts?category=ens&first=1&skip=99999").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = get(app.clone(), "/v1/nfts?category=ens&first=1&skip=100000").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(graph_reads(), 0);

    // One clock: a page admitted at its request's clock keeps the latest
    // sales for its values and order, though a later clock finds the
    // publication past its freshness deadline (the sales inputs folded 31
    // minutes ago, the names index 29).
    let real = chrono::Utc::now();
    for (indexers, ago) in [(vec!["sales", "trade_events"], 31), (vec!["nft"], 29)] {
        sqlx::query(
            "UPDATE shop.chain_cursor SET folded_at = $1 WHERE network = 'ethereum' AND indexer = ANY($2)",
        )
        .bind((real - chrono::Duration::minutes(ago)).naive_utc())
        .bind(indexers)
        .execute(pool)
        .await
        .unwrap();
    }
    sqlx::query("UPDATE shop.sale_publication SET published_at = $1")
        .bind((real - chrono::Duration::minutes(31)).naive_utc())
        .execute(pool)
        .await
        .unwrap();
    let filters =
        super::nfts::parse_filters(&[("category".to_string(), "ens".to_string())]).unwrap();
    let (rows, _, _) = super::nft_reads::names_page_if_admitted(
        &state,
        &filters,
        2,
        0,
        real - chrono::Duration::minutes(2),
    )
    .await
    .unwrap()
    .expect("admitted at the request's clock");
    let beta = rows.iter().find(|r| r.nft.token_id == "1002").unwrap();
    assert_eq!(beta.nft.sold_at, 3_000_000);
    sqlx::query("UPDATE shop.chain_cursor SET folded_at = timezone('UTC', now()) WHERE network = 'ethereum'")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("UPDATE shop.sale_publication SET published_at = timezone('UTC', now())")
        .execute(pool)
        .await
        .unwrap();

    // An admission check that cannot run is the page's error, never the graph.
    sqlx::query("ALTER TABLE shop.sale_publication RENAME TO sale_publication_hidden")
        .execute(pool)
        .await
        .unwrap();
    let (status, body) = get(app.clone(), by_sale).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    assert_eq!(graph_reads(), 0, "no graph request on a failed check");
    sqlx::query("ALTER TABLE shop.sale_publication_hidden RENAME TO sale_publication")
        .execute(pool)
        .await
        .unwrap();

    // Confirmed not admitted: the graph, as before.
    sqlx::query("UPDATE shop.sale_publication SET last_sale_through = NULL")
        .execute(pool)
        .await
        .unwrap();
    let (status, body) = get(app, by_sale).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"][0]["nft"]["tokenId"], "1001", "{body}");
    assert_eq!(sold_at(&body, "1001"), Some(2_000_000));
    assert!(graph_reads() > 0);
    own.drop().await;
}
