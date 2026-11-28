//! Route parity for the trending readers. `fixtures/trending/trending-goldens.json`
//! holds what `/v1/trendings`, `/v3/catalog/trending`, `/v3/catalog/creators`,
//! `/v3/catalog/suggested` and `/v3/catalog/suggest` answered when their
//! signal came from the collections subgraphs' `sales`: recorded on the
//! subgraph-backed readers over the catalogue [`seed_catalogue`] writes, with
//! one stub subgraph per network serving the captured `sales` fixtures under
//! the query's `timestamp_gt` and `first`, as the subgraph does. The routes
//! must answer the same from the sales history, with every subgraph URL
//! unset.
//!
//! The fixtures are shifted by whole days to today, so each sale keeps its
//! time of day and the windows (from midnight UTC) take the same sales
//! whenever the test runs.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use axum::body::Body;
use axum::http::Request;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

use super::chain_walk::ChainNet;
use super::{MarketplaceState, Settings, listings, market_router};

/// The routes the goldens hold, each read once in this order.
pub(super) const ROUTES: [&str; 8] = [
    "/v1/trendings",
    "/v1/trendings?size=40",
    "/v3/catalog/trending?first=50",
    "/v3/catalog/trending?first=50&days=1",
    "/v3/catalog/creators?days=30",
    "/v3/catalog/creators?days=365&first=50",
    "/v3/catalog/suggested?first=12",
    "/v3/catalog/suggest?search=fixture&collections=10&items=3",
];

/// One captured `sales` row, shifted to today.
#[derive(Clone, Debug)]
pub(super) struct FixtureSale {
    pub net: ChainNet,
    pub id: u64,
    pub time: i64,
    pub kind: String,
    pub contract: String,
    pub item: String,
    pub price: String,
    pub creator: String,
}

/// Both fixtures' sales in the subgraph's order (newest first, a tie by id
/// descending), Ethereum's first, shifted by the whole days from the
/// Polygon capture's day to today.
pub(super) fn fixture_sales() -> Vec<FixtureSale> {
    let matic: Value =
        serde_json::from_str(include_str!("fixtures/trending/subgraph-sales-matic.json")).unwrap();
    let captured = matic["block"]["timestamp"].as_i64().unwrap();
    let today = chrono::Utc::now().timestamp().div_euclid(86_400);
    let shift = (today - captured.div_euclid(86_400)) * 86_400;
    let ethereum: Value = serde_json::from_str(include_str!(
        "fixtures/trending/subgraph-sales-ethereum.json"
    ))
    .unwrap();
    [(ChainNet::Ethereum, ethereum), (ChainNet::Matic, matic)]
        .into_iter()
        .flat_map(|(net, v)| {
            v["sales"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| {
                    let text = |k: &str| s[k].as_str().unwrap().to_string();
                    FixtureSale {
                        net,
                        id: text("id").parse().unwrap(),
                        time: text("timestamp").parse::<i64>().unwrap() + shift,
                        kind: text("type"),
                        contract: text("searchContractAddress"),
                        item: text("searchItemId"),
                        price: text("price"),
                        creator: text("creator"),
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The catalogue the routes read: every fixture item, approved, on sale in
/// the store at a price of its own, in a collection named after its
/// contract; ten more items per creator in a filler collection, so every
/// creator passes the creators route's catalogue floor; both mirrors' passes,
/// a MANA price and published trades.
pub(super) async fn seed_catalogue(pool: &PgPool) {
    let sales = fixture_sales();
    let mut items: BTreeMap<(ChainNet, String, String), String> = BTreeMap::new();
    for s in &sales {
        items
            .entry((s.net, s.contract.clone(), s.item.clone()))
            .or_insert_with(|| s.creator.clone());
    }
    let creators: BTreeSet<String> = items.values().cloned().collect();
    let mut rows: Vec<(ChainNet, String, String, String)> = items
        .into_iter()
        .map(|((net, contract, item), creator)| (net, contract, item, creator))
        .collect();
    for creator in &creators {
        let filler = format!("0xf1{}", &creator[4..]);
        for item in 0..10 {
            rows.push((
                ChainNet::Matic,
                filler.clone(),
                item.to_string(),
                creator.clone(),
            ));
        }
    }
    let store = super::sales_index::STORE_MATIC;
    let v3 = listings::offchain_marketplace_v3(137).unwrap().address;
    for (net, contract, item, creator) in &rows {
        let filler = contract.starts_with("0xf1");
        let price = format!(
            "{}000000000000000000",
            item.parse::<u64>().unwrap_or(0) % 7 + 1
        );
        sqlx::query(
            "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, \
                 total_supply, max_supply, available, price, image, name, category, body_shapes, \
                 search_is_collection_approved, search_is_store_minter, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ($1, $2, $3::text::numeric, $4, $5, $6, $7, 'common', 0, 1000, 1000, $8::text::numeric, '', $9, \
                 'hat', ARRAY['BaseMale'], true, true, 1, 1, 1, timezone('UTC', now()))",
        )
        .bind(net.db_name())
        .bind(contract)
        .bind(item)
        .bind(net.chain_id())
        .bind(if *net == ChainNet::Ethereum { "wearable_v1" } else { "wearable_v2" })
        .bind(format!("urn:decentraland:{}:collections-v2:{contract}:{item}", net.db_name()))
        .bind(creator)
        .bind(&price)
        .bind(format!("Item {}-{item}", &contract[2..8]))
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, is_completed, \
                 is_approved, is_editable, minters, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ($1, $2, $3, $4, $5, $5, $6, true, true, false, ARRAY[$7, $8], 1, 1, 1, timezone('UTC', now())) \
             ON CONFLICT (network, contract_address) DO NOTHING",
        )
        .bind(net.db_name())
        .bind(contract)
        .bind(net.chain_id())
        .bind(if filler { format!("Filler {}", &contract[2..8]) } else { format!("Fixture {}", &contract[2..8]) })
        .bind(creator)
        .bind(format!("urn:decentraland:{}:collections-v2:{contract}", net.db_name()))
        .bind(store)
        .bind(v3)
        .execute(pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at) \
         SELECT m, n, 'test', timezone('UTC', now()) \
         FROM unnest(ARRAY['collections', 'items']) m CROSS JOIN unnest(ARRAY['ethereum', 'matic']) n",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO economy.price_snapshots (source, mana_usd, taken_at) \
         VALUES ('coingecko', 0.25, timezone('UTC', now()))",
    )
    .execute(pool)
    .await
    .unwrap();
}

/// A state over `pool` with `settings` that may call loopback stubs.
pub(super) fn state_with(pool: &PgPool, settings: Settings) -> Arc<MarketplaceState> {
    let state = Arc::new(MarketplaceState::new(
        pool.clone(),
        crate::http::Outbound::for_tests(),
        settings,
    ));
    state
        .test_allow_loopback
        .store(true, std::sync::atomic::Ordering::Relaxed);
    state
}

/// Each route's status and body, with the read's stamp (`as_of`,
/// `stale_after_secs`), which follows the clock, left out.
pub(super) async fn answers(state: &Arc<MarketplaceState>) -> Value {
    let mut out = serde_json::Map::new();
    for uri in ROUTES {
        let response = market_router(state.clone())
            .oneshot(Request::get(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let mut body: Value = serde_json::from_slice(&bytes).unwrap();
        if let Some(map) = body.as_object_mut() {
            map.remove("as_of");
            map.remove("stale_after_secs");
        }
        out.insert(
            uri.to_string(),
            json!({ "status": status.as_u16(), "body": body }),
        );
    }
    Value::Object(out)
}

/// The routes answer from the sales history, with every subgraph URL unset
/// and the collections index serving the catalogue, what the
/// subgraph-backed readers answered on the same catalogue and sales.
#[tokio::test]
async fn the_trending_readers_answer_the_subgraph_goldens() {
    use super::sale_fold::test_support::{Sale, insert, publish};
    let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
        return;
    };
    seed_catalogue(&pool).await;
    // Each timestamp its block; the subgraph's counter id, which grows in
    // log order, its log.
    let sales = fixture_sales();
    let mut top = 0;
    for net in ChainNet::ALL {
        let mut by_time: BTreeMap<i64, Vec<&FixtureSale>> = BTreeMap::new();
        for s in sales.iter().filter(|s| s.net == net) {
            by_time.entry(s.time).or_default().push(s);
        }
        for (block, (time, mut rows)) in (1i64..).zip(by_time) {
            rows.sort_by_key(|s| s.id);
            for (log, s) in (0i32..).zip(rows) {
                let (execution, kind) = match s.kind.as_str() {
                    "mint" => ("store", "mint"),
                    "bid" => ("bid", "bid"),
                    _ => ("order", "order"),
                };
                insert(
                    &pool,
                    &Sale {
                        net,
                        block,
                        log,
                        ordinal: 0,
                        time,
                        execution,
                        kind,
                        contract: s.contract.clone(),
                        item_id: Some(s.item.clone()),
                        token_id: Some(s.id.to_string()),
                        price: s.price.clone(),
                        currency: None,
                        buyer: "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
                        seller: (execution != "store")
                            .then(|| "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into()),
                    },
                )
                .await;
            }
            top = top.max(block);
        }
    }
    for net in ChainNet::ALL {
        assert_eq!(publish(&pool, net, top).await, top);
    }
    super::trade_events::tests::publish_cursors(&pool).await;
    // With no subgraph, the collections index serves both networks'
    // catalogue: the same rows, published.
    for net in ChainNet::ALL {
        super::catalogue::owning::own(&pool, net.db_name(), net.chain_id(), top, 0).await;
    }
    let mut settings = Settings::for_tests();
    settings.data_writes_enabled = true;
    settings.market.indexes.sales_enabled = true;
    settings.market.indexes.collections_enabled = true;
    settings.market.indexes.collections_owned = ChainNet::ALL.to_vec();
    assert!(settings.eth_collections_subgraph_url.is_none());
    assert!(settings.matic_collections_subgraph_url.is_none());
    assert!(settings.marketplace_subgraph_url.is_none());
    let got = answers(&state_with(&pool, settings)).await;
    let goldens: Value =
        serde_json::from_str(include_str!("fixtures/trending/trending-goldens.json")).unwrap();
    for uri in ROUTES {
        assert_eq!(got[uri]["status"], 200, "{uri}: {}", got[uri]);
        assert_eq!(got[uri], goldens["routes"][uri], "{uri}");
    }
    pool.close().await;
}
