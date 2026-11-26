use super::*;

#[tokio::test]
async fn shallow_nft_sale_filters_use_published_land_and_names_before_paging() {
    use super::super::chain_walk::ChainNet;
    use crate::land_authz::LAND_MAINNET;

    let graph = FakeGraph::new();
    let url = serve_graph(graph.clone()).await;
    let Some((state, db)) = state_over_own(&url, true).await else {
        return;
    };
    let pool = &db.pool;
    let land = LAND_MAINNET.land_registry;
    let estate = LAND_MAINNET.estate_registry;
    let names = super::super::nft_index::DCL_REGISTRAR;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "INSERT INTO land_authz.index_cursor(network,land_registry,estate_registry,last_block,folded_block,folded_at,full_replay_started)
         VALUES ('ethereum-mainnet','{land}','{estate}',20,20,timezone('UTC',now()),true);
         INSERT INTO land_authz.token_right(token_address,token_id,owner,x,y,updated_block,updated_log)
         VALUES ('{land}',77,'{WALLET_A}',0,77,10,0),('{land}',78,'{WALLET_A}',0,78,10,1),('{land}',79,'{WALLET_A}',0,79,10,2);
         INSERT INTO shop.nft(network,contract_address,token_id,owner,ens_subdomain,minted_block,minted_at,transferred_block,transferred_log,transferred_at)
         VALUES ('ethereum','{names}',9,'{WALLET_A}','alpha',1,1,1,0,1),
                ('ethereum','{names}',10,'{WALLET_A}','beta',2,2,2,0,2),
                ('ethereum','{names}',11,'{WALLET_A}','gamma',3,3,3,0,3);
         INSERT INTO shop.nft_order(network,contract_address,token_id,order_id,marketplace_address,status,seller,price,expires_at,created_at,updated_at,updated_block,updated_log)
         VALUES ('ethereum','{land}',79,'legacy-parcel','{CREATOR}','open','{WALLET_A}',25,4102444800000,1,2,13,0),
                ('ethereum','{names}',11,'legacy-name','{CREATOR}','open','{WALLET_A}',25,4102444800000,1,2,13,0)"
    )))
    .execute(pool)
    .await
    .unwrap();
    super::super::owned_items::test_support::publish(
        pool,
        &[ChainNet::Ethereum],
        false,
        Duration::ZERO,
    )
    .await;
    super::super::trade_events::tests::publish_cursors(pool).await;
    sqlx::query(
        "UPDATE shop.chain_cursor SET folded_block=NULL,folded_at=NULL WHERE network='matic' AND indexer='trade_events'",
    )
    .execute(pool)
    .await
    .unwrap();
    let mut trades = BTreeMap::new();
    for (contract, token) in [(land, "77"), (land, "79"), (names, "9"), (names, "11")] {
        let (id, _) = insert_trade_on(
            pool,
            "ETHEREUM",
            1,
            "public_nft_order",
            WALLET_A,
            Asset {
                asset_type: 3,
                contract: contract.to_owned(),
                value: token.into(),
            },
            Asset {
                asset_type: 1,
                contract: listings::mana_token(1).unwrap().to_owned(),
                value: "10".into(),
            },
        )
        .await;
        trades.insert(token, id);
    }
    let app = |walker: bool| {
        let mut settings = state.settings.clone();
        settings.market.indexes.nft_enabled = walker;
        settings.marketplace_subgraph_url = Some(url.clone());
        let state = Arc::new(super::super::MarketplaceState::new(
            pool.clone(),
            crate::http::Outbound::for_tests(),
            settings,
        ));
        state.test_allow_loopback.store(true, Ordering::Relaxed);
        market_router(state)
    };
    for (category, listed, overlap, unlisted) in
        [("parcel", "77", "79", "78"), ("ens", "9", "11", "10")]
    {
        for walks in [true, false] {
            let path = format!("/v1/nfts?category={category}");
            let mut tokens = Vec::new();
            for skip in [0, 1] {
                let (code, page) = get(
                    app(walks),
                    &format!("{path}&isOnSale=true&first=1&skip={skip}"),
                )
                .await;
                assert_eq!(code, StatusCode::OK, "{page}");
                assert_eq!(page["total"], 2);
                let row = &page["data"][0];
                let token = row["nft"]["tokenId"].as_str().unwrap();
                if token == listed {
                    assert_eq!(row["order"]["tradeId"], trades[token]);
                } else {
                    assert_eq!(token, overlap);
                    assert!(row["order"]["id"].as_str().unwrap().starts_with("legacy-"));
                }
                tokens.push(token.to_owned());
            }
            tokens.sort();
            let mut expected_tokens = [listed, overlap];
            expected_tokens.sort();
            assert_eq!(tokens, expected_tokens);
            let (code, page) = get(app(walks), &format!("{path}&isOnSale=false&first=1")).await;
            assert_eq!(code, StatusCode::OK, "{page}");
            assert_eq!(page["total"], 1);
            assert_eq!(page["data"][0]["nft"]["tokenId"], unlisted);
        }
    }
    sqlx::query("UPDATE shop.chain_cursor SET folded_block=NULL,folded_at=NULL WHERE network='ethereum' AND indexer='trade_events'")
        .execute(pool).await.unwrap();
    let (code, page) = get(app(false), "/v1/nfts?category=ens&isOnSale=false&first=1").await;
    assert_eq!(
        (code, page["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("trade-events-index"))
    );
    sqlx::query("UPDATE shop.chain_cursor SET folded_block=NULL,folded_at=NULL WHERE network='ethereum' AND indexer='nft'")
        .execute(pool).await.unwrap();
    let (code, page) = get(
        app(false),
        &format!("/v1/nfts?contractAddress={names}&isOnSale=true&first=1"),
    )
    .await;
    assert_eq!(
        (code, page["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("nft-index"))
    );
    assert!(
        !graph
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|entity| entity == "nfts")
    );
    db.drop().await;
}

#[tokio::test]
async fn graph_orders_filter_both_expiry_units_before_pages_and_counts() {
    let graph = FakeGraph::new();
    let url = serve_graph(graph.clone()).await;
    let eth = serve_empty_graph().await;
    let Some(db) = super::super::tests::support::OwnDb::create("expiry_graph_orders").await else {
        return;
    };
    let settings = market_settings(&url, &eth, true);
    let state = Arc::new(super::super::MarketplaceState::new(
        db.pool.clone(),
        crate::http::Outbound::for_tests(),
        settings,
    ));
    state.test_allow_loopback.store(true, Ordering::Relaxed);
    super::super::trade_events::tests::publish_cursors(&db.pool).await;
    let contract = contract_of(&tag(), 1);
    let now = chrono::Utc::now().timestamp();
    graph.set(
        "orders",
        [(1, now - 600), (2, (now - 600) * 1000), (3, now + 600), (4, (now + 600) * 1000)]
            .into_iter()
            .map(|(number, raw)| {
                json!({
                    "id": format!("order-{number}"), "nftAddress": contract,
                    "marketplaceAddress": CREATOR, "tokenId": number.to_string(),
                    "owner": WALLET_A, "price": number.to_string(), "status": "open",
                    "expiresAt": raw.to_string(), "createdAt": number.to_string(), "updatedAt": number.to_string()
                })
            })
            .collect(),
    );
    let app = || market_router(state.clone());
    for (skip, id) in [(0, "order-3"), (1, "order-4")] {
        let (code, page) = get(
            app(),
            &format!("/v1/orders?network=MATIC&contractAddress={contract}&status=open&sortBy=cheapest&first=1&skip={skip}"),
        ).await;
        assert_eq!(code, StatusCode::OK, "{page}");
        assert_eq!(page["total"], "2");
        assert_eq!(page["data"][0]["id"], id);
        assert_eq!(page["data"][0]["expiresAt"], (now + 600) * 1000);
    }
    let (code, page) = get(
        app(),
        &format!("/v1/orders/open-by-items?item={contract}-0&perContract=2"),
    )
    .await;
    assert_eq!(code, StatusCode::OK, "{page}");
    assert_eq!(page["total"], "2");
    assert_eq!(page["data"][0]["id"], "order-3");
    assert_eq!(page["data"][1]["id"], "order-4");
    db.drop().await;
}
