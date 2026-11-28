//! `/v1/owners` from the nft index on a node with every subgraph URL unset,
//! against the subgraph reader, over the subgraphs' own answers for one
//! Polygon (v2) and one Ethereum (v1) item
//! (`tests/fixtures/shop/owners-subgraph.json`).

use super::super::chain_walk::ChainNet;
use super::*;

fn fixture() -> Value {
    let path = format!(
        "{}/services/catalyst/tests/fixtures/shop/owners-subgraph.json",
        env!("DCL_ONE_ROOT")
    );
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

fn text(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap().to_string()
}

fn router_over(pool: &PgPool, settings: Settings) -> Router {
    let state = super::super::MarketplaceState::new(
        pool.clone(),
        crate::http::Outbound::for_tests(),
        settings,
    );
    state.test_allow_loopback.store(true, Ordering::Relaxed);
    market_router(Arc::new(state))
}

/// The collections, items and copies `fx` describes, in the catalogue and
/// the nft index, with what a page must leave out around them: on Polygon a
/// neighbouring item's copies on both sides of the item's token range and a
/// burned copy of the item; on Ethereum the collection's other wearables,
/// minted between the item's copies.
async fn seed(pool: &PgPool, fx: &Value) {
    for it in fx["items"].as_array().unwrap() {
        let network = text(it, "network");
        let contract = text(it, "contractAddress");
        let chain = if network == "matic" { 137 } else { 1 };
        let (item_type, wearable) = if network == "matic" {
            ("wearable_v2", None)
        } else {
            let urn = text(it, "urn");
            let wearable = urn.rsplit_once(':').unwrap().1.to_string();
            ("wearable_v1", Some(wearable))
        };
        sqlx::query(
            "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, is_completed, is_approved, is_editable, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ($1, $2, $3, 'Owners', $4, $4, $5, true, true, false, 1, 1, 1, now())",
        )
        .bind(&network)
        .bind(&contract)
        .bind(chain)
        .bind(CREATOR)
        .bind(text(it, "collectionUrn"))
        .execute(pool)
        .await
        .unwrap();
        let item: i64 = text(it, "itemId").parse().unwrap();
        let sibling_urn = match &wearable {
            Some(_) => format!("{}:sibling_wearable", text(it, "collectionUrn")),
            None => format!("{}:{}", text(it, "collectionUrn"), item + 1),
        };
        for (id, urn) in [(item, text(it, "urn")), (item + 1, sibling_urn)] {
            sqlx::query(
                "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, total_supply, max_supply, available, price, image, name, category, body_shapes, search_is_collection_approved, created_at, updated_at, reviewed_at, mirrored_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, 'mythic', 10, 10, 0, 0, 'i', 'Item', 'hat', ARRAY['BaseMale'], true, 1, 1, 1, now())",
            )
            .bind(&network)
            .bind(&contract)
            .bind(id)
            .bind(chain)
            .bind(item_type)
            .bind(urn)
            .bind(CREATOR)
            .execute(pool)
            .await
            .unwrap();
        }
        let insert = |token: String,
                      owner: Option<String>,
                      item_id: Option<i64>,
                      issued: i64,
                      wearable: Option<String>| {
            let (network, contract) = (network.clone(), contract.clone());
            async move {
                sqlx::query(
                    "INSERT INTO shop.nft (network, contract_address, token_id, owner, item_id, issued_id, wearable_id, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
                     VALUES ($1, $2, $3::numeric, $4, $5, $6, $7, 10, 1700000000, 10, 0, 1700000000)",
                )
                .bind(network)
                .bind(contract)
                .bind(token)
                .bind(owner)
                .bind(item_id)
                .bind(issued)
                .bind(wearable)
                .execute(pool)
                .await
                .unwrap();
            }
        };
        for copy in it["copies"].as_array().unwrap() {
            let issued: i64 = text(copy, "issuedId").parse().unwrap();
            insert(
                text(copy, "tokenId"),
                Some(text(copy, "ownerId")),
                wearable.is_none().then_some(item),
                issued,
                wearable.clone(),
            )
            .await;
        }
        match &wearable {
            None => {
                let token = |item: i64, issued: i64| {
                    ((alloy_primitives::U256::from(item as u64) << 216usize)
                        | alloy_primitives::U256::from(issued as u64))
                    .to_string()
                };
                // The next copy of the item, burned.
                insert(token(item, 11), None, Some(item), 11, None).await;
                for (other, issued) in [(item - 1, 1), (item - 1, 99), (item + 1, 1), (item + 1, 2)]
                {
                    insert(
                        token(other, issued),
                        Some(WALLET_B.into()),
                        Some(other),
                        issued,
                        None,
                    )
                    .await;
                }
            }
            Some(_) => {
                for (k, copy) in it["copies"].as_array().unwrap().iter().enumerate() {
                    let token: u64 = text(copy, "tokenId").parse().unwrap();
                    insert(
                        (token + 1).to_string(),
                        Some(WALLET_B.into()),
                        None,
                        k as i64 + 1,
                        Some("sibling_wearable".into()),
                    )
                    .await;
                }
            }
        }
    }
}

/// The subgraph reader's rows of `fx`'s item on `network`.
fn graph_nfts(fx: &Value, network: &str) -> Vec<Value> {
    let it = fx["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|it| it["network"] == network)
        .unwrap();
    it["copies"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            json!({
                "id": format!("{}-{}", text(it, "contractAddress"), text(c, "tokenId")),
                "contractAddress": it["contractAddress"],
                "itemBlockchainId": it["itemId"],
                "issuedId": c["issuedId"],
                "tokenId": c["tokenId"],
                "owner": { "address": c["ownerId"] },
            })
        })
        .collect()
}

/// With every subgraph URL unset, a node that walks the nft index answers
/// the pages the subgraphs answered for the same item (order, window and
/// total), as the subgraph reader does over those copies; the item's
/// neighbours, its burned copy and its collection's other wearables stay
/// out.
#[tokio::test]
async fn owners_from_the_nft_index_answer_the_subgraph_pages() {
    let Some(own) = super::super::tests::support::OwnDb::create("owners_parity").await else {
        return;
    };
    let pool = own.pool.clone();
    let fx = fixture();
    seed(&pool, &fx).await;
    super::super::owned_items::test_support::publish(&pool, &ChainNet::ALL, true, Duration::ZERO)
        .await;
    let (eth, matic) = (FakeGraph::new(), FakeGraph::new());
    eth.set("nfts", graph_nfts(&fx, "ethereum"));
    matic.set("nfts", graph_nfts(&fx, "matic"));
    let reader = router_over(
        &pool,
        Settings {
            eth_collections_subgraph_url: Some(serve_graph(eth).await),
            matic_collections_subgraph_url: Some(serve_graph(matic).await),
            ..Settings::for_tests()
        },
    );
    let unset = Settings::for_tests();
    assert!(unset.eth_collections_subgraph_url.is_none());
    assert!(unset.matic_collections_subgraph_url.is_none());
    assert!(unset.marketplace_subgraph_url.is_none());
    let walker = router_over(
        &pool,
        super::super::owned_items::test_support::walking(unset),
    );
    let mut compared = 0;
    for it in fx["items"].as_array().unwrap() {
        for page in it["pages"].as_array().unwrap() {
            let uri = format!(
                "/v1/owners?contractAddress={}&itemId={}&orderDirection={}&first={}&skip={}",
                text(it, "contractAddress")
                    .to_uppercase()
                    .replacen("0X", "0x", 1),
                text(it, "itemId"),
                text(page, "orderDirection"),
                page["first"],
                page["skip"],
            );
            let want = json!({ "data": page["data"], "total": 10 });
            for (side, app) in [("index", walker.clone()), ("subgraph", reader.clone())] {
                let (status, body) = get(app, &uri).await;
                assert_eq!(status, StatusCode::OK, "{side} {uri}: {body}");
                assert_eq!(body, want, "{side} {uri}");
            }
            compared += 1;
        }
        // The default order is the subgraph reader's, descending.
        let uri = format!(
            "/v1/owners?contractAddress={}&itemId={}&first=3",
            text(it, "contractAddress"),
            text(it, "itemId")
        );
        let (status, body) = get(walker.clone(), &uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
        let issued: Vec<&str> = body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["issuedId"].as_str().unwrap())
            .collect();
        assert_eq!(issued, ["10", "9", "8"], "{uri}");
        // A page past the last copy keeps the item's total (the subgraph
        // reader answers 0 there: it skips a network whose page is empty).
        let (status, body) = get(walker.clone(), &format!("{uri}&skip=10")).await;
        assert_eq!(
            (status, body),
            (StatusCode::OK, json!({ "data": [], "total": 10 })),
            "{uri}"
        );
    }
    assert_eq!(compared, 10);
    own.drop().await;
}

/// The index read answers only on a published, fresh nft fold whose
/// catalogue names the item, and refuses a malformed item id or window;
/// an item the catalogue does not name has no holders.
#[tokio::test]
async fn owners_from_the_nft_index_wait_for_a_published_fresh_fold() {
    let Some(own) = super::super::tests::support::OwnDb::create("owners_ready").await else {
        return;
    };
    let pool = own.pool.clone();
    let fx = fixture();
    seed(&pool, &fx).await;
    let walker = router_over(
        &pool,
        super::super::owned_items::test_support::walking(Settings::for_tests()),
    );
    let polygon = &fx["items"][0];
    let uri = format!(
        "/v1/owners?contractAddress={}&itemId={}",
        text(polygon, "contractAddress"),
        text(polygon, "itemId")
    );
    let source = |body: &Value| body["source"].as_str().map(str::to_string);
    // Not published yet.
    let (status, body) = get(walker.clone(), &uri).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(source(&body).as_deref(), Some("nft-index"), "{body}");
    // Published, but the items mirror never finished a pass.
    super::super::owned_items::test_support::publish(&pool, &ChainNet::ALL, false, Duration::ZERO)
        .await;
    let (status, body) = get(walker.clone(), &uri).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(source(&body).as_deref(), Some("items-mirror"), "{body}");
    super::super::owned_items::test_support::publish(&pool, &ChainNet::ALL, true, Duration::ZERO)
        .await;
    let (status, body) = get(walker.clone(), &uri).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["total"], 10);
    // A fold write no publication has acknowledged yet.
    sqlx::query("UPDATE shop.nft SET owner = $1 WHERE network = 'ethereum' AND owner IS NOT NULL")
        .bind(WALLET_A)
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = get(walker.clone(), &uri).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(source(&body).as_deref(), Some("nft-index"), "{body}");
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("ethereum nft index holds a fold it has not published yet"),
        "{body}"
    );
    super::super::owned_items::test_support::publish(&pool, &ChainNet::ALL, true, Duration::ZERO)
        .await;
    let (status, body) = get(walker.clone(), &uri).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // A publication older than the freshness window.
    super::super::owned_items::test_support::publish(
        &pool,
        &[ChainNet::Matic],
        true,
        Duration::from_secs(24 * 3600),
    )
    .await;
    let (status, body) = get(walker.clone(), &uri).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(source(&body).as_deref(), Some("nft-index"), "{body}");
    super::super::owned_items::test_support::publish(&pool, &ChainNet::ALL, true, Duration::ZERO)
        .await;
    for (query, total) in [
        // An item the catalogue does not name, and one no token id encodes.
        ("itemId=77", 0),
        ("itemId=1099511627776", 0),
        // Another contract.
        (
            "itemId=3&contractAddress=0x00000000000000000000000000000000000000c9",
            0,
        ),
    ] {
        let uri = if query.contains("contractAddress") {
            format!("/v1/owners?{query}")
        } else {
            format!(
                "/v1/owners?contractAddress={}&{query}",
                text(polygon, "contractAddress")
            )
        };
        let (status, body) = get(walker.clone(), &uri).await;
        assert_eq!(
            (status, body),
            (StatusCode::OK, json!({ "data": [], "total": total })),
            "{uri}"
        );
    }
    for (query, message) in [
        ("itemId=three", "itemId must be a decimal item id"),
        (
            "itemId=3&first=100&skip=24901",
            "first + skip may not exceed 25000",
        ),
    ] {
        let uri = format!(
            "/v1/owners?contractAddress={}&{query}",
            text(polygon, "contractAddress")
        );
        let (status, body) = get(walker.clone(), &uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {body}");
        assert_eq!(body["message"], message, "{uri}");
    }
    own.drop().await;
}
