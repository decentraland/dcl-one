//! The market slice on the scratch database: a fake subgraph on the loopback
//! feeds the collections and items mirrors (walk, budget, failure, resume,
//! the readiness gate) and the readers, then every route group answers over
//! it; the favorites, admin and signed-trade flows run over the local tables.
//! Without `CATALYST_TEST_DATABASE_URL` the database tests skip.

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

async fn get(router: Router, uri: &str) -> (StatusCode, Value) {
    call(router, "GET", uri, HeaderMap::new(), None).await
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
/// items page past the first answer 500 (through the client's retries) until cleared.
struct FakeGraph {
    rows: std::sync::Mutex<BTreeMap<&'static str, Vec<Value>>>,
    fail_next_items_page: AtomicBool,
    requests: std::sync::Mutex<Vec<String>>,
}

impl FakeGraph {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            rows: std::sync::Mutex::new(BTreeMap::new()),
            fail_next_items_page: AtomicBool::new(false),
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
                let matches = |r: &Value| {
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
                        if let Some(name) = key.strip_suffix("_in") {
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
                };
                let mut rows: Vec<Value> = all
                    .into_iter()
                    .filter(|r| (id_gt.is_empty() || s(r, "id") > id_gt) && matches(r))
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

async fn job_ctx(jobs: &crate::jobs::Jobs) -> crate::jobs::JobContext {
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

async fn state_over(url: &str, writes: bool) -> Option<SharedState> {
    let eth_url = serve_empty_graph().await;
    let state = state_with_db(market_settings(url, &eth_url, writes)).await?;
    state.test_allow_loopback.store(true, Ordering::Relaxed);
    Some(state)
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
    let id = uuid::Uuid::new_v4().to_string();
    let signature = format!("0x{}", tag()).to_lowercase();
    let hashed = listings::hashed_signature(&signature);
    sqlx::query(
        "INSERT INTO shop.trades (id, network, chain_id, signature, hashed_signature, checks, signer, type, expires_at, effective_since, contract, created_at) \
         VALUES ($1::uuid, 'MATIC', 137, $2, $3, '{\"uses\": 1}'::jsonb, $4, $5::shop.trade_type, timezone('UTC', now()) + interval '30 days', timezone('UTC', now()) - interval '1 day', $6, timezone('UTC', now()))",
    )
    .bind(&id)
    .bind(&signature)
    .bind(&hashed)
    .bind(signer)
    .bind(kind)
    .bind(listings::offchain_marketplace_v3(137).unwrap().address)
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
async fn the_mirrors_walk_the_subgraph_and_every_market_route_answers_over_them() {
    let up = FakeGraph::new();
    let url = serve_graph(up.clone()).await;
    let Some(state) = state_over(&url, true).await else {
        return;
    };
    let pool = state.pool.clone();
    let t = tag();
    let (c1, c2, c_nft) = (contract_of(&t, 1), contract_of(&t, 2), contract_of(&t, 3));
    let v3_minter = listings::offchain_marketplace_v2(137).unwrap().address;

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
    state.mirror_ready.clear();

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
    let (status, body) = get(app(), "/v1/prices?assetType=nft&category=wearable").await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("nft-index")),
        "the nft leg waits for the nft index to publish"
    );

    // The readers: canned subgraph rows merged with the mirror legs.
    let now_s = chrono::Utc::now().timestamp();
    let far = (now_s + 86_400 * 30) * 1000;
    up.set("orders", vec![json!({ "id": "legacy-order-1", "marketplaceAddress": "0x8e5660b4ab70168b5a6feea0e0315cb49c8cd539", "nftAddress": c_nft, "tokenId": "5", "owner": WALLET_A, "buyer": null, "price": "1000000000000000000", "status": "open", "expiresAt": far.to_string(), "createdAt": (now_s - 100).to_string(), "updatedAt": (now_s - 100).to_string(), "nft": { "issuedId": "5" } })]);
    up.set("bids", vec![json!({ "id": "legacy-bid-1", "bidAddress": "0xb1d", "nftAddress": c_nft, "tokenId": "5", "bidder": WALLET_B, "seller": WALLET_A, "price": "2000000000000000000", "fingerprint": "0x", "status": "open", "blockchainId": "bid-1", "blockNumber": "1", "expiresAt": far.to_string(), "createdAt": (now_s - 1000).to_string(), "updatedAt": (now_s - 1000).to_string() })]);
    let sale = |i: i64| json!({ "id": format!("sale-{i}"), "type": if i % 2 == 0 { "mint" } else { "order" }, "buyer": WALLET_B, "seller": if i % 2 == 0 { CREATOR } else { WALLET_A }, "price": "5000000000000000000", "timestamp": (now_s - 3600 * i).to_string(), "txHash": format!("0xtx{i}"), "searchContractAddress": c1, "searchItemId": "1", "searchTokenId": "77", "searchCategory": "wearable", "item": { "id": format!("{c1}-1"), "creator": CREATOR } });
    up.set("sales", (1..=6).map(sale).collect());
    up.set("accounts", vec![json!({ "id": WALLET_A, "address": WALLET_A, "sales": 3, "purchases": 1, "spent": "5", "earned": "30", "royalties": "0", "collections": 1, "primarySales": 2, "primarySalesEarned": "20", "uniqueCollectorsTotal": 2, "uniqueAndMythicItemsTotal": 0, "creatorsSupportedTotal": 1 }), json!({ "id": WALLET_B, "address": WALLET_B, "sales": 1, "purchases": 4, "spent": "50", "earned": "1", "royalties": "0", "collections": 0, "primarySales": 0, "primarySalesEarned": "0", "uniqueCollectorsTotal": 0, "uniqueAndMythicItemsTotal": 3, "creatorsSupportedTotal": 2 })]);
    up.set("itemsDayDatas", vec![json!({ "id": format!("19000-{c1}-1"), "sales": 2, "volume": "10", "searchWearableCategory": "hat" }), json!({ "id": format!("19001-{c1}-1"), "sales": 1, "volume": "5", "searchWearableCategory": "hat" })]);
    up.set("analyticsDayDatas", vec![json!({ "id": "19001", "date": now_s - 100, "sales": 4, "volume": "40", "creatorsEarnings": "1", "daoEarnings": "2" })]);
    let owned_urn = format!("urn:decentraland:matic:collections-v2:{c1}:4");
    up.set("nfts", vec![
        json!({ "id": format!("{c1}-{}", "4"), "tokenId": "4", "contractAddress": c1, "category": "wearable", "itemType": "wearable_v2", "urn": owned_urn, "image": "", "issuedId": "1", "itemBlockchainId": "4", "transferredAt": "1700000000", "createdAt": "1700000000", "updatedAt": "1700000001", "soldAt": "0", "owner": { "address": WALLET_A }, "item": { "id": format!("{c1}-4"), "rarity": "epic", "price": "5000000000000000000" }, "metadata": { "wearable": { "name": "Item 0004", "description": "a hat", "category": "hat", "rarity": "epic", "bodyShapes": ["BaseMale"] }, "emote": null }, "activeOrder": null }),
    ]);
    sqlx::query("INSERT INTO shop.usage_grants (grantee_address, urn, token_id, collection, category, unlock_at) VALUES ($1, $2, '9', $3, 'wearable', timezone('UTC', now()) + interval '2 days')")
        .bind(WALLET_A)
        .bind(format!("urn:decentraland:matic:collections-v2:{c1}:6"))
        .bind(&c1)
        .execute(&pool)
        .await
        .unwrap();

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
    assert_eq!(body["data"][1]["issuedId"], "7");
    let (status, body) = get(app(), &format!("/v1/orders/open-by-items?item={c_nft}-2")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["total"], "1",
        "only the token whose id encodes item 2: {body}"
    );
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
    let (status, body) = get(app(), "/v1/sales?first=2&type=mint").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (
            body["data"].as_array().unwrap().len(),
            body["total"].as_i64()
        ),
        (2, Some(3))
    );
    assert_eq!(body["data"][0]["type"], "mint");
    let (status, body) = get(app(), &format!("/v1/sales/summary?seller={CREATOR}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (
            body["data"]["total"].as_i64(),
            body["data"]["mints"].as_i64(),
            body["data"]["royalties"]["resales"].as_i64()
        ),
        (Some(3), Some(3), Some(3)),
        "{body}"
    );
    let (status, body) = get(app(), "/v1/accounts?sortBy=most_spent&first=1").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (body["data"][0]["address"].as_str(), body["total"].as_i64()),
        (Some(WALLET_B), Some(2))
    );
    let (status, body) = get(app(), &format!("/v1/owners?contractAddress={c1}&itemId=4")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["data"][0],
        json!({ "issuedId": "1", "ownerId": WALLET_A, "tokenId": "4" })
    );
    let (status, body) = get(app(), "/v1/rankings/wearables/week?first=1").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["data"][0],
        json!({ "id": format!("{c1}-1"), "sales": 3, "volume": "15" })
    );
    let (status, body) = get(app(), "/v1/volume/week").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["data"],
        json!({ "sales": 4, "volume": "40", "creatorsEarnings": "1", "daoEarnings": "2" })
    );
    // All time: the lifetime totals of the same subgraph entities.
    let (status, body) = get(app(), "/v1/volume/all").await;
    assert_eq!(
        (status, &body["data"]),
        (
            StatusCode::OK,
            &json!({ "sales": 4, "volume": "40", "creatorsEarnings": "1", "daoEarnings": "2" })
        ),
        "{body}"
    );
    let (status, body) = get(app(), "/v1/rankings/creators/all?first=1").await;
    assert_eq!(
        (status, &body["data"]),
        (
            StatusCode::OK,
            &json!([{ "id": WALLET_A, "sales": 3, "earned": "30", "uniqueCollectors": 2, "collections": 1 }])
        ),
        "{body}"
    );
    let (status, body) = get(app(), "/v1/rankings/collectors/all?sortBy=most_sales").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["data"][0],
        json!({ "id": WALLET_B, "purchases": 4, "spent": "50", "uniqueAndMythicItems": 3, "creatorsSupported": 2 })
    );
    assert_eq!(
        get(app(), "/v1/rankings/wearables/all?first=2").await.0,
        StatusCode::OK
    );
    let (status, body) = get(app(), "/v1/nfts?first=1&category=wearable").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (
            body["data"][0]["nft"]["id"].as_str(),
            body["total"].as_i64()
        ),
        (Some(format!("{c1}-4").as_str()), Some(1))
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
    assert_eq!(body["data"]["elements"][0]["tokenId"], "4");
    let (status, body) = get(app(), &format!("/v1/users/{WALLET_A}/names")).await;
    assert_eq!(
        (status, body["source"].as_str()),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Some("marketplace-subgraph")
        ),
        "{body}"
    );

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

fn item_order_json(signer: &str, contract: &str) -> String {
    format!(
        r#"{{ "signer": "{signer}", "signature": "0x00", "type": "public_item_order", "network": "MATIC", "chainId": 137,
             "checks": {{ "uses": 1, "expiration": 4102444800000, "effective": 0, "salt": "0x1234", "contractSignatureIndex": 0, "signerSignatureIndex": 0, "allowedRoot": "0x", "externalChecks": [] }},
             "sent": [ {{"assetType": 4, "contractAddress": "{contract}", "itemId": "1", "extra": "0x"}} ],
             "received": [ {{"assetType": 1, "contractAddress": "0x2222222222222222222222222222222222222222", "amount": "1000000000000000000", "extra": "0x", "beneficiary": "{signer}"}} ] }}"#
    )
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
    let id = listings::create_trade(&pool, None, &trade, &signer, 1_000)
        .await
        .unwrap();
    assert!(matches!(
        listings::create_trade(&pool, None, &trade, &signer, 1_000)
            .await
            .unwrap_err(),
        listings::TradeCreationError::Duplicate
    ));
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
        ("/v1/nfts?first=1", "collections-subgraph:matic"),
        ("/v1/orders?first=1", "collections-subgraph:matic"),
        ("/v1/bids", "collections-subgraph:matic"),
        ("/v1/sales?first=1", "collections-subgraph:matic"),
        (
            "/v1/sales/summary?seller=0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "collections-subgraph:matic",
        ),
        (
            "/v1/owners?contractAddress=0xa&itemId=1",
            "collections-subgraph:matic",
        ),
        ("/v1/accounts", "collections-subgraph:matic"),
        ("/v1/rankings/wearables/week", "collections-subgraph:matic"),
        ("/v1/volume/week", "collections-subgraph:matic"),
        ("/v1/trendings", "collections-subgraph:matic"),
        (
            "/v1/users/0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/wearables",
            "collections-subgraph:matic",
        ),
        (
            "/v1/users/0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/names",
            "marketplace-subgraph",
        ),
        ("/v1/catalog", "collections-subgraph:matic"),
        ("/v1/items", "collections-subgraph:matic"),
        // `/v1/collections` and `/v1/contracts` serve the copy their tables
        // record, so they need the database
        // (`tests::without_a_subgraph_descriptive_reads_serve_the_recorded_copy`).
        ("/v3/catalog/shop", "collections-subgraph:matic"),
        ("/v1/prices?category=wearable", "collections-subgraph:matic"),
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

/// A page past the graph's 5,000 skip is read by keyset on the sort field
/// (the fake enforces the graph's bounds and honours the range filters):
/// 7,300 sales in threes per timestamp, newest first, so the continuation
/// crosses ties; the total counts past the window; a window past the deep
/// bound is a 400.
#[tokio::test]
async fn deep_pages_past_the_graph_window_read_by_keyset() {
    let matic = FakeGraph::new();
    let sale = |i: i64| json!({ "id": format!("m-{i:05}"), "type": "order", "buyer": WALLET_A, "seller": WALLET_B, "price": "1", "timestamp": (100_000 - i / 3).to_string(), "txHash": "0x", "searchContractAddress": "0x1", "searchCategory": "wearable" });
    matic.set("sales", (0..7_300).map(sale).collect());
    let eth_url = serve_empty_graph().await;
    let state = state_without_db_settings(Settings {
        matic_collections_subgraph_url: Some(serve_graph(matic.clone()).await),
        eth_collections_subgraph_url: Some(eth_url),
        ..Settings::for_tests()
    });
    state.test_allow_loopback.store(true, Ordering::Relaxed);
    let app = || market_router(state.clone());
    for (first, skip) in [(5, 5_998), (5, 7_000), (100, 7_250)] {
        let (status, body) = get(
            app(),
            &format!("/v1/sales?network=MATIC&first={first}&skip={skip}"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "skip={skip}: {body}");
        let ids: Vec<&str> = body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["id"].as_str().unwrap())
            .collect();
        let want: Vec<String> = (skip..(skip + first).min(7_300))
            .map(|i| format!("m-{i:05}"))
            .collect();
        assert_eq!(ids, want, "skip={skip}");
        assert_eq!(
            body["total"], 7_300,
            "a deep page counts past the graph's window"
        );
    }
    let (status, body) = get(app(), "/v1/sales?first=5&skip=7000").await;
    assert_eq!(
        status,
        StatusCode::OK,
        "both networks merge past the window too: {body}"
    );
    assert_eq!(body["data"][0]["id"], "m-07000");
    let (status, _) = get(app(), "/v1/sales?first=10&skip=24995").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "past the deep bound");
}

/// Pages merged across the two networks need each network's top
/// `first + skip` rows, and The Graph refuses a `first` above 1000: those
/// windows are read in pages it accepts (the fake enforces its bounds), so a
/// deep merged page answers the right rows instead of the 503 (`/v1/nfts`)
/// or the silently empty page (`/v1/sales`, `/v1/accounts`) of a
/// `first: first + skip` request or a window clamped to 1000.
#[tokio::test]
async fn deep_merged_pages_read_the_graph_in_pages_it_accepts() {
    let matic = FakeGraph::new();
    let eth = FakeGraph::new();
    let nft = |net: &str, i: i64| json!({ "id": format!("{net}-{i:05}"), "tokenId": (i * 2 + if net == "matic" { 1 } else { 0 }).to_string(), "contractAddress": "0x0000000000000000000000000000000000000001", "category": "wearable", "owner": { "address": WALLET_A }, "createdAt": (i * 2 + if net == "matic" { 1 } else { 0 }).to_string(), "updatedAt": "1", "name": format!("n{i}") });
    let sale = |net: &str, i: i64| json!({ "id": format!("{net}-{i:05}"), "type": "order", "buyer": WALLET_A, "seller": WALLET_B, "price": "1", "timestamp": (i * 2 + if net == "matic" { 1 } else { 0 }).to_string(), "txHash": "0x", "searchContractAddress": "0x1", "searchCategory": "wearable" });
    let account = |net: &str, i: i64| json!({ "id": format!("{net}-{i:05}"), "address": format!("0x{:040x}", i * 2 + if net == "matic" { 1 } else { 0 }), "sales": 0, "purchases": 0, "spent": "0", "earned": (i * 2 + if net == "matic" { 1 } else { 0 }).to_string(), "royalties": "0", "collections": 0 });
    for (graph, net) in [(&matic, "matic"), (&eth, "eth")] {
        graph.set("nfts", (0..3100).map(|i| nft(net, i)).collect());
        graph.set("sales", (0..3100).map(|i| sale(net, i)).collect());
        graph.set("accounts", (0..700).map(|i| account(net, i)).collect());
    }
    let (matic_url, eth_url) = (
        serve_graph(matic.clone()).await,
        serve_graph(eth.clone()).await,
    );
    let state = state_without_db_settings(Settings {
        matic_collections_subgraph_url: Some(matic_url),
        eth_collections_subgraph_url: Some(eth_url),
        ..Settings::for_tests()
    });
    state.test_allow_loopback.store(true, Ordering::Relaxed);
    let app = || market_router(state.clone());
    // Both networks interleave by createdAt/timestamp (eth even, matic odd),
    // newest first: rank r of the merge is 6199 - r.
    let rank = |r: i64| -> String {
        let v = 6199 - r;
        format!("{}-{:05}", if v % 2 == 1 { "matic" } else { "eth" }, v / 2)
    };
    for (first, skip) in [(5, 990), (5, 996), (5, 4990), (5, 5000), (100, 5000)] {
        let (status, body) = get(app(), &format!("/v1/nfts?first={first}&skip={skip}")).await;
        assert_eq!(status, StatusCode::OK, "nfts skip={skip}: {body}");
        let tokens: Vec<&str> = body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["nft"]["tokenId"].as_str().unwrap())
            .collect();
        let want_tokens: Vec<String> = (skip..skip + first)
            .map(|r| (6199 - r).to_string())
            .collect();
        assert_eq!(tokens, want_tokens, "nfts skip={skip}");
        let want: Vec<String> = (skip..skip + first).map(rank).collect();
        let (status, body) = get(app(), &format!("/v1/sales?first={first}&skip={skip}")).await;
        assert_eq!(status, StatusCode::OK, "sales skip={skip}: {body}");
        let ids: Vec<&str> = body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, want, "sales skip={skip}");
    }
    let (status, body) = get(app(), "/v1/nfts?first=3&skip=1000&network=MATIC").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let tokens: Vec<&str> = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["nft"]["tokenId"].as_str().unwrap())
        .collect();
    assert_eq!(
        tokens,
        vec!["4199", "4197", "4195"],
        "one network pages itself: matic's rows 1000..1003, newest first"
    );
    let (status, body) = get(app(), "/v1/accounts?first=10&skip=995").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let earned: Vec<&str> = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["earned"].as_str().unwrap())
        .collect();
    let want: Vec<String> = (995..1005).map(|r| (1399 - r).to_string()).collect();
    assert_eq!(
        earned, want,
        "1,400 accounts across both networks, skip 995: ten rows past the 1000-row window, not an empty page"
    );
}
