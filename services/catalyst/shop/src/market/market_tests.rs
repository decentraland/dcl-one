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
    assert!(
        mirrors::require_ready(&state, mirrors::Mirror::Items)
            .await
            .is_ok()
    );
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
    let (status, body) = get(app(), "/v1/prices?assetType=nft&category=wearable").await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("nft-index")),
        "the nft leg waits for the nft index to publish"
    );

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

    // The trending rail's memo is filled right before the cancellation below, so the few reads in
    // between are all that separate it from the check that it still answers: its 30 s cannot lapse
    // over the rest of this test on a slow runner.
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
    // An item read stays fresh; the trending rail answers from its 30 s memo until it lapses.
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
    assert_eq!(body["data"], trending["data"]);
    state.readers.clear();
    let (_, body) = get(app(), "/v3/catalog/trending?first=3").await;
    assert!(!carries_primary(&body), "{body}");
    // A read that names its item never reads the creator directory; a browse read does.
    use super::readers::Ttl;
    use super::search::CREATOR_DIRECTORY_KEY;
    state.readers.clear();
    let (status, body) = get(
        app(),
        &format!("/v3/catalog/unified?groupBy=item&contractAddress={c1}&itemId=1&source=native"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!state.readers.holds(Ttl::M15, CREATOR_DIRECTORY_KEY));
    let (status, body) = get(app(), "/v3/catalog/unified?groupBy=item&first=3").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // The browse read started the directory's read in the background; it may land after the
    // answer, which waits at most 500 ms for it.
    tokio::time::timeout(Duration::from_secs(20), async {
        while !state.readers.holds(Ttl::M15, CREATOR_DIRECTORY_KEY) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the browse read fills the creator directory");

    // The feeds carry their creators' profiles per response, never memoized with the page: a
    // profile denylisted after the pages were memoized leaves the very next answer, and every
    // response that carries a profile is sent no-cache.
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
    super::search::creator_directory(&state).await.unwrap();
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
    // A feed without the creator directory (here, its refresh held by another) answers without
    // profiles under a brief cache, never the hour these feeds had, so a CDN does not keep an
    // answer whose cards each ask /lambdas/profiles.
    // A claim still live from the feeds above could not release this hold either: its release
    // acts only on its own claim.
    state.readers.clear();
    let hold = state
        .readers
        .begin_refresh(CREATOR_DIRECTORY_KEY, Duration::ZERO)
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
        assert_eq!(cache, "public,max-age=60,s-maxage=60", "{uri}");
    }
    state.readers.end_refresh(CREATOR_DIRECTORY_KEY, hold, true);

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
/// 401, and a check that cannot run (the RPC failed) is a 503: never the
/// signer, on the required (POST) and the optional (GET) route alike.
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
/// node's RPC quota per request, so each client source may start only the
/// `/forward-blockchain` pre-auth budget of checks a minute: past it the
/// answer is a 429 before any `isValidSignature` call, and another source
/// keeps its own budget.
#[tokio::test]
async fn contract_wallet_checks_are_admitted_per_source_before_any_rpc_call() {
    let Some(mut state) = state_with_db(Settings::for_tests()).await else {
        return;
    };
    let counter = Arc::new(CountingRefusal::default());
    Arc::get_mut(&mut state).expect("an unshared state").eip1654 = Some(counter.clone());
    let contract = format!("0x{:0>40}", "c0de2").to_lowercase();
    let path = format!("/v1/picks/{}-0", contract_of(&tag(), 8));
    let get_from = |peer: &'static str| {
        let (app, path, contract) = (market_router(state.clone()), path.clone(), contract.clone());
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
    let mut tripped = None;
    for sent in 0..200 {
        let before = counter.0.load(Ordering::SeqCst);
        let status = get_from("203.0.113.77:40000").await;
        if status == StatusCode::TOO_MANY_REQUESTS {
            tripped = Some((sent, before, counter.0.load(Ordering::SeqCst)));
            break;
        }
        assert_eq!(status, StatusCode::UNAUTHORIZED, "a refused contract");
    }
    let (sent, before, after) = tripped.expect("the source's budget trips");
    assert!(
        (super::tips::PREAUTH_PER_MINUTE as usize..=super::tips::PREAUTH_PER_MINUTE as usize + 5)
            .contains(&sent),
        "the /forward-blockchain pre-auth budget: {sent}"
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
            let (items, total) = list_items(&state, &f).await.unwrap();
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
    sqlx::query("INSERT INTO shop.trade_chain_state (trade_id, network, status, folded_block, digest_failed_at) VALUES ($1::uuid, 'matic', 'open', 10, timezone('UTC', now()))")
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
        build_shop_listings_sql, build_unified_items_sql, build_unified_listings_sql,
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
        "shop" => build_shop_listings_sql(&parse_shop_filters(pairs), None),
        "items" => build_unified_items_sql(&parse_unified_filters(pairs), rate, None),
        _ => build_unified_listings_sql(&parse_unified_filters(pairs), rate, None),
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
            "nft-index",
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
/// deep merged page answers the right rows instead of the silently empty
/// page (`/v1/sales`, `/v1/accounts`) of a
/// `first: first + skip` request or a window clamped to 1000.
#[tokio::test]
async fn deep_merged_pages_read_the_graph_in_pages_it_accepts() {
    let matic = FakeGraph::new();
    let eth = FakeGraph::new();
    let sale = |net: &str, i: i64| json!({ "id": format!("{net}-{i:05}"), "type": "order", "buyer": WALLET_A, "seller": WALLET_B, "price": "1", "timestamp": (i * 2 + if net == "matic" { 1 } else { 0 }).to_string(), "txHash": "0x", "searchContractAddress": "0x1", "searchCategory": "wearable" });
    let account = |net: &str, i: i64| json!({ "id": format!("{net}-{i:05}"), "address": format!("0x{:040x}", i * 2 + if net == "matic" { 1 } else { 0 }), "sales": 0, "purchases": 0, "spent": "0", "earned": (i * 2 + if net == "matic" { 1 } else { 0 }).to_string(), "royalties": "0", "collections": 0 });
    for (graph, net) in [(&matic, "matic"), (&eth, "eth")] {
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

/// The open listings' owner guard is an exact-token check on the nft index:
/// a listing whose signer still holds that token stays, one whose token
/// another address holds (even while the signer holds another copy of the
/// item) or nobody holds (burned) drops out, and a token the index does not
/// hold (LAND, an estate, a collection outside the mirror) passes. Until the
/// index is published on every network the read names, the leg answers the
/// unavailable envelope, never unguarded or partial; a node that does not
/// walk the index has no guard and serves every open listing, whatever rows
/// its `shop.nft` holds.
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
    use super::orders::OwnerCheck;
    let ids_of = |rows: Vec<super::orders::Keyed>| {
        let mut ids: Vec<String> = rows.into_iter().map(|k| k.order.id).collect();
        ids.sort();
        ids
    };
    let unavailable = |r: Result<
        (Vec<super::orders::Keyed>, OwnerCheck),
        axum::response::Response,
    >| { r.err().map(|resp| resp.status()) };
    assert_eq!(
        unavailable(super::orders::trades_leg(&state, &open).await),
        Some(StatusCode::SERVICE_UNAVAILABLE),
        "an unpublished index answers the envelope"
    );
    let not_walking = std::sync::Arc::new(super::MarketplaceState::new(
        pool.clone(),
        crate::http::Outbound::for_tests(),
        market_settings(&url, &url, true),
    ));
    assert!(!super::owned_items::walks_here(&not_walking));
    let (rows, check) = super::orders::trades_leg(&not_walking, &open)
        .await
        .unwrap();
    assert_eq!(check, OwnerCheck::Unchecked);
    let ids = ids_of(rows);
    let mut want = vec![
        held.clone(),
        sold.clone(),
        burned.clone(),
        elsewhere.clone(),
        land.clone(),
    ];
    want.sort();
    assert_eq!(
        ids, want,
        "a node that does not walk the index serves the leg without the guard"
    );
    // The leg waits for the networks the read names only.
    super::owned_items::test_support::publish(
        &pool,
        &[super::chain_walk::ChainNet::Matic],
        false,
        Duration::ZERO,
    )
    .await;
    assert_eq!(
        unavailable(super::orders::trades_leg(&state, &open).await),
        Some(StatusCode::SERVICE_UNAVAILABLE),
        "a read of both networks waits for Ethereum"
    );
    let matic = super::orders::OrderFilters {
        network: Some(super::wire::Network::Matic),
        ..open.clone()
    };
    let mut polygon = vec![held.clone(), elsewhere.clone()];
    polygon.sort();
    let mut both = vec![held, elsewhere, land];
    both.sort();
    for (label, filters, nets, want) in [
        (
            "Polygon alone",
            &matic,
            &[super::chain_walk::ChainNet::Matic][..],
            &polygon,
        ),
        ("both", &open, &super::chain_walk::ChainNet::ALL[..], &both),
    ] {
        super::owned_items::test_support::publish(&pool, nets, false, Duration::ZERO).await;
        let (rows, check) = super::orders::trades_leg(&state, filters).await.unwrap();
        assert_eq!(check, OwnerCheck::Checked, "{label}");
        assert_eq!(
            &ids_of(rows),
            want,
            "{label}: the sold ({sold}) and burned ({burned}) tokens' listings drop out"
        );
    }
    own.drop().await;
}

/// A listing the guard drops (its signer sold the token) stays out on both
/// order routes once the index is published; while a rewind has deleted
/// the token's row and withdrawn the index both routes answer the
/// unavailable envelope instead of serving it unguarded, and after the
/// refill publishes it again it still drops out. The LAND registry's
/// listing, which the guard never filters, is served whenever the routes
/// answer.
#[tokio::test]
async fn a_sold_listing_stays_out_through_a_rewind_and_its_refill() {
    use super::orders::OwnerCheck;
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
    // The LAND index is ready: the open LAND orders the routes may answer
    // are checked against it.
    sqlx::query("INSERT INTO land_authz.index_cursor (network, land_registry, estate_registry, last_block, folded_block, folded_at, full_replay_started) VALUES ('ethereum-mainnet', $1, $2, 1, 1, timezone('UTC', now()), true)")
        .bind(crate::land_authz::LAND_REGISTRY_MAINNET)
        .bind(crate::land_authz::ESTATE_REGISTRY_MAINNET)
        .execute(&pool)
        .await
        .unwrap();
    let withdrawn = || async {
        (
            super::orders::list_orders_checked(&state, &open)
                .await
                .err()
                .map(|r| r.status()),
            super::orders::open_orders_by_items_checked(&state, &items, 10)
                .await
                .err()
                .map(|r| r.status()),
        )
    };
    let routes = || async {
        let (orders, _, listed) = super::orders::list_orders_checked(&state, &open)
            .await
            .unwrap_or_else(|_| panic!("/v1/orders answers"));
        let (by_items, by_items_check) =
            super::orders::open_orders_by_items_checked(&state, &items, 10)
                .await
                .unwrap_or_else(|_| panic!("/v1/orders/open-by-items answers"));
        let ids = |orders: Vec<super::orders::Order>| -> Vec<String> {
            let mut ids: Vec<String> = orders.into_iter().map(|o| o.id).collect();
            ids.sort();
            ids
        };
        ((ids(orders), listed), (ids(by_items), by_items_check))
    };
    fold().await;
    let want = |check| ((vec![land.clone()], check), (vec![land.clone()], check));
    assert_eq!(
        routes().await,
        want(OwnerCheck::Checked),
        "published: {sold} drops out"
    );
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
    assert_eq!(
        routes().await,
        want(OwnerCheck::Checked),
        "refilled: {sold} still drops out"
    );
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
            Some("land-authz-index, nft-index".to_string())
        ),
        "neither is ready: the owner's orders stop at the LAND index, the buyer's at the nft index"
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
        (StatusCode::OK, Some("land-authz-index".to_string())),
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
        (StatusCode::OK, None),
        "every leg answered"
    );
    own.drop().await;
}

/// The order book's readiness reads the chains the node serves, never
/// 1/137 by default: on the Sepolia/Amoy pair its snapshot takes nft
/// publications bound to that pair (with the book caught up on them) and
/// refuses ones bound to the mainnets. The routes reach this snapshot once
/// the shared walk predicate counts a testnet walker; until then such a node
/// keeps the subgraph reader.
#[tokio::test]
async fn the_order_book_readiness_reads_the_served_testnet_pair() {
    use super::chain_walk::ChainNet;
    let Some(own) = super::tests::support::OwnDb::create("testnet_readiness").await else {
        return;
    };
    let url = serve_empty_graph().await;
    let mut settings = super::owned_items::test_support::walking(market_settings(&url, &url, true));
    settings.eth_network = "sepolia".into();
    settings.market.indexes.chain_id_ethereum = ChainNet::Ethereum.testnet_chain_id();
    settings.market.indexes.chain_id_matic = ChainNet::Matic.testnet_chain_id();
    let state = super::MarketplaceState::new(
        own.pool.clone(),
        crate::http::Outbound::for_tests(),
        settings,
    );
    let pool = own.pool.clone();
    super::trade_events::tests::publish_cursors(&pool).await;
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
    let snapshot = super::orders::book_snapshot(&state, &ChainNet::ALL, false, true).await;
    let held = match snapshot {
        Ok((tx, _)) => tx,
        Err(r) => panic!(
            "the testnet publications are the served chains' ones: {}",
            r.status()
        ),
    };
    // The snapshot holds a connection of the test pool until released.
    held.rollback().await.unwrap();
    publish(false).await;
    let snapshot = super::orders::book_snapshot(&state, &ChainNet::ALL, false, true).await;
    assert_eq!(
        snapshot.err().map(|r| r.status()),
        Some(StatusCode::SERVICE_UNAVAILABLE),
        "a mainnet-bound publication does not serve a testnet node"
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
                    "shop" => build_shop_listings_sql(&base, None),
                    "legacy" => build_legacy_listings_sql(
                        &LegacyCatalogFilters {
                            first: base.first,
                            skip: base.skip,
                            search: base.search,
                            sort_by: base.sort_by,
                            ..Default::default()
                        },
                        None,
                    ),
                    "unified listings" => build_unified_listings_sql(&unified, rate, None),
                    "unified items" => build_unified_items_sql(&unified, rate, None),
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
