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
         VALUES ('ethereum','{land}',79,'legacy-parcel','{CREATOR}','open','{WALLET_A}',25,4102444800000,EXTRACT(EPOCH FROM now())::bigint,2,13,0),
                ('ethereum','{names}',11,'legacy-name','{CREATOR}','open','{WALLET_A}',25,4102444800000,EXTRACT(EPOCH FROM now())::bigint,2,13,0)"
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
    sqlx::query("UPDATE shop.trades SET created_at=timezone('UTC',now())-interval '1 day'")
        .execute(pool)
        .await
        .unwrap();
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
            for (sort, token, price) in [
                ("recently_listed", overlap, "25"),
                ("cheapest", listed, "10"),
            ] {
                let (code, page) = get(
                    app(walks),
                    &format!("{path}&isOnSale=true&sortBy={sort}&first=1"),
                )
                .await;
                assert_eq!(code, StatusCode::OK, "{page}");
                assert_eq!(page["total"], 2);
                assert_eq!(page["data"][0]["nft"]["tokenId"], token);
                assert_eq!(page["data"][0]["order"]["price"], price);
            }
            let (code, page) =
                get(app(walks), &format!("{path}&maxPrice=0.00000000000000002")).await;
            assert_eq!(code, StatusCode::OK, "{page}");
            assert_eq!(page["total"], 1);
            assert_eq!(page["data"][0]["nft"]["tokenId"], listed);
        }
    }
    for walks in [true, false] {
        for filter in [
            "category=parcel".to_owned(),
            "category=estate".to_owned(),
            "category=ens".to_owned(),
            format!("contractAddress={land}"),
            format!("contractAddress={estate}"),
            format!("contractAddress={names}"),
        ] {
            let (code, page) = get(
                app(walks),
                &format!("/v1/nfts?network=MATIC&{filter}&isOnSale=true"),
            )
            .await;
            assert_eq!(code, StatusCode::OK, "{page}");
            assert_eq!(page["total"], 0);
        }
    }
    for walker in [true, false] {
        let mut settings = state.settings.clone();
        settings.eth_network = "sepolia".into();
        settings.market.indexes.nft_enabled = walker;
        settings.market.indexes.chain_id_ethereum = 11_155_111;
        settings.market.indexes.chain_id_matic = 80_002;
        let testnet = market_router(Arc::new(super::super::MarketplaceState::new(
            pool.clone(),
            crate::http::Outbound::for_tests(),
            settings,
        )));
        let sepolia = crate::land_authz::LAND_SEPOLIA;
        for address in [
            sepolia.land_registry,
            sepolia.estate_registry,
            super::super::nft_index::Catalogue::Testnet.registrar(),
        ] {
            let (code, page) = get(
                testnet.clone(),
                &format!("/v1/nfts?network=MATIC&contractAddress={address}&isOnSale=true"),
            )
            .await;
            assert_eq!(code, StatusCode::OK, "{page}");
            assert_eq!(page["total"], 0);
        }
        for category in ["parcel", "estate", "ens"] {
            let (code, page) = get(
                testnet.clone(),
                &format!("/v1/nfts?network=MATIC&category={category}&isOnSale=true"),
            )
            .await;
            assert_eq!(code, StatusCode::OK, "{page}");
            assert_eq!(page["total"], 0);
        }
    }
    let parcel_filter = "/v1/nfts?category=parcel&isOnSale=true";
    let name_filter = "/v1/nfts?category=ens&isOnSale=true";
    let publish_eth = || {
        super::super::owned_items::test_support::publish(
            pool,
            &[ChainNet::Ethereum],
            false,
            Duration::ZERO,
        )
    };
    // An upgraded database gains order proof at the first publication,
    // including when the writer publishes at the same height.
    sqlx::query(
        "DELETE FROM shop.projection_write WHERE network='ethereum' AND projection='nft_order'",
    )
    .execute(pool)
    .await
    .unwrap();
    let (code, page) = get(app(false), parcel_filter).await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{page}");
    assert_eq!(page["source"], "nft-index");
    publish_eth().await;
    sqlx::query(
        "UPDATE shop.nft_order SET price=76, updated_block=101 WHERE order_id='legacy-parcel'",
    )
    .execute(pool)
    .await
    .unwrap();
    let (code, page) = get(app(false), parcel_filter).await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{page}");
    assert_eq!(page["source"], "nft-index");
    sqlx::query("UPDATE shop.nft_order SET updated_block=100 WHERE order_id='legacy-parcel'")
        .execute(pool)
        .await
        .unwrap();
    publish_eth().await;
    let (code, page) = get(app(false), parcel_filter).await;
    assert_eq!(code, StatusCode::OK, "{page}");
    assert_eq!(page["total"], 2);
    assert!(
        page["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| { row["order"]["id"] == "legacy-parcel" && row["order"]["price"] == "76" }),
        "{page}"
    );

    sqlx::query("UPDATE shop.nft SET owner=$1 WHERE network='ethereum' AND contract_address=$2 AND token_id=9")
        .bind(CREATOR).bind(names).execute(pool).await.unwrap();
    let (code, page) = get(app(false), name_filter).await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{page}");
    assert_eq!(page["source"], "nft-index");
    let (code, page) = get(app(false), parcel_filter).await;
    assert_eq!(code, StatusCode::OK, "{page}");
    sqlx::query("UPDATE shop.nft SET owner=$1 WHERE network='ethereum' AND contract_address=$2 AND token_id=9")
        .bind(WALLET_A).bind(names).execute(pool).await.unwrap();
    publish_eth().await;
    let (code, page) = get(app(false), name_filter).await;
    assert_eq!(code, StatusCode::OK, "{page}");
    assert_eq!(page["total"], 2);

    let (second, _) = insert_trade_on(
        pool,
        "ETHEREUM",
        1,
        "public_nft_order",
        WALLET_A,
        Asset {
            asset_type: 3,
            contract: names.into(),
            value: "9".into(),
        },
        Asset {
            asset_type: 1,
            contract: listings::mana_token(1).unwrap().into(),
            value: "10".into(),
        },
    )
    .await;
    let mut ids = [trades["9"].clone(), second];
    ids.sort();
    for (id, stamp, price) in [
        (&ids[0], "2026-01-01 00:00:00.0001", "123"),
        (&ids[1], "2026-01-01 00:00:00.0002", "10"),
    ] {
        sqlx::query("UPDATE shop.trades SET created_at=$1::text::timestamp WHERE id=$2::uuid")
            .bind(stamp)
            .bind(id)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("UPDATE shop.trade_assets_erc20 v SET amount=$1::numeric FROM shop.trade_assets a WHERE v.asset_id=a.id AND a.trade_id=$2::uuid")
            .bind(price).bind(id).execute(pool).await.unwrap();
    }
    let capped = format!("{name_filter}&maxPrice=0.000000000000000011");
    let (code, page) = get(app(false), &capped).await;
    assert_eq!(code, StatusCode::OK, "{page}");
    assert_eq!(page["total"], 0);
    sqlx::query("UPDATE shop.trade_assets_erc20 v SET amount=10 FROM shop.trade_assets a WHERE v.asset_id=a.id AND a.trade_id=$1::uuid")
        .bind(&ids[0]).execute(pool).await.unwrap();
    let (code, page) = get(app(false), &capped).await;
    assert_eq!(code, StatusCode::OK, "{page}");
    assert_eq!(page["total"], 1);
    assert_eq!(page["data"][0]["order"]["tradeId"], ids[0]);
    assert_eq!(page["data"][0]["order"]["price"], "10");
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
    // An unscoped non-walker still lists eligible assets with optional LAND
    // withdrawn. The explicit LAND path itself must report that source.
    super::super::owned_items::test_support::publish(
        pool,
        &[ChainNet::Ethereum],
        true,
        Duration::ZERO,
    )
    .await;
    super::super::trade_events::tests::publish_cursors(pool).await;
    sqlx::query("UPDATE land_authz.index_cursor SET folded_block=NULL,folded_at=NULL")
        .execute(pool)
        .await
        .unwrap();
    let (code, page) = get(app(false), "/v1/nfts?network=ETHEREUM&isOnSale=true").await;
    assert_eq!(code, StatusCode::OK, "{page}");
    assert_eq!(page["total"], 2);
    assert!(
        page["data"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["nft"]["category"] == "ens")
    );
    let (code, page) = get(app(false), "/v1/nfts?category=parcel&isOnSale=true").await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{page}");
    assert_eq!(page["source"], "land-authz-index");

    // The same Polygon filter drops the unrelated Ethereum registrar but
    // keeps an eligible collection in a mixed contract list.
    let polygon = contract_of(&tag(), 1);
    seed_item_row(pool, &polygon, "1", CREATOR).await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "INSERT INTO shop.nft (network,contract_address,token_id,owner,item_id,
          minted_block,minted_at,transferred_block,transferred_log,transferred_at)
         VALUES ('matic','{polygon}',1,'{WALLET_A}',1,1,1,1,0,1);
         INSERT INTO shop.nft_order (network,contract_address,token_id,order_id,
           marketplace_address,status,seller,price,expires_at,created_at,updated_at,updated_block,updated_log)
         VALUES ('matic','{polygon}',1,'polygon-1','{CREATOR}','open','{WALLET_A}',30,4102444800000,1,1,1,0)"
    ))).execute(pool).await.unwrap();
    super::super::owned_items::test_support::publish(
        pool,
        &[ChainNet::Matic],
        true,
        Duration::ZERO,
    )
    .await;
    let (code, page) = get(
        app(false),
        &format!(
            "/v1/nfts?network=MATIC&contractAddress={names}&contractAddress={polygon}&isOnSale=true"
        ),
    )
    .await;
    assert_eq!(code, StatusCode::OK, "{page}");
    assert_eq!(page["total"], 1);
    assert_eq!(page["data"][0]["nft"]["contractAddress"], polygon);

    // Commit LAND's publication and a NAME price together when the old
    // reader would return its last probe connection before opening the page.
    const PUBLISH: &str = "BEGIN;
         UPDATE land_authz.index_cursor SET folded_block=20,folded_at=timezone('UTC',now());
         UPDATE shop.nft_order SET price=26 WHERE order_id='legacy-name';
         UPDATE shop.chain_cursor SET folded_at=timezone('UTC',now()) WHERE indexer='nft' AND network='ethereum';
         COMMIT";
    let released = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let armed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let gated_pool = sqlx::postgres::PgPoolOptions::new()
        .min_connections(1)
        .max_connections(1)
        .after_release({
            let released = released.clone();
            let armed = armed.clone();
            let writer = pool.clone();
            move |_, _| {
                let (released, armed, writer) = (released.clone(), armed.clone(), writer.clone());
                Box::pin(async move {
                    if armed.load(Ordering::SeqCst) && released.fetch_add(1, Ordering::SeqCst) == 2
                    {
                        sqlx::raw_sql(PUBLISH).execute(&writer).await?;
                    }
                    Ok(true)
                })
            }
        })
        .connect_with(pool.connect_options().as_ref().clone())
        .await
        .unwrap();
    let mut reader = state.settings.clone();
    reader.market.indexes.nft_enabled = false;
    let mixed = market_router(Arc::new(super::super::MarketplaceState::new(
        gated_pool.clone(),
        crate::http::Outbound::for_tests(),
        reader,
    )));
    let endpoint = "/v1/nfts?network=ETHEREUM&isOnSale=true";
    let name_price = |page: Value| {
        let order = page["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["order"]["id"] == "legacy-name")
            .unwrap();
        (
            page["total"].as_u64().unwrap(),
            order["order"]["price"].as_str().unwrap().to_owned(),
        )
    };
    let (code, before) = get(mixed.clone(), endpoint).await;
    assert_eq!(code, StatusCode::OK, "{before}");
    let mut idle = gated_pool.acquire().await.unwrap();
    idle.return_to_pool().await;
    drop(idle);
    armed.store(true, Ordering::SeqCst);
    let (code, during) = get(mixed.clone(), endpoint).await;
    assert_eq!(code, StatusCode::OK, "{during}");
    armed.store(false, Ordering::SeqCst);
    sqlx::raw_sql(PUBLISH).execute(pool).await.unwrap();
    let (code, after) = get(mixed, endpoint).await;
    assert_eq!(code, StatusCode::OK, "{after}");
    let before = name_price(before);
    let after = name_price(after);
    let during = name_price(during);
    assert_eq!(before, (2, "25".into()));
    assert_eq!(after, (4, "26".into()));
    assert!(
        during == before || during == after,
        "mixed snapshot: {during:?}"
    );
    gated_pool.close().await;

    sqlx::query(
        "ALTER TABLE land_authz.index_cursor RENAME COLUMN updated_at TO hidden_updated_at",
    )
    .execute(pool)
    .await
    .unwrap();
    let (code, page) = get(app(false), endpoint).await;
    assert_eq!(code, StatusCode::OK, "{page}");
    assert_eq!(name_price(page), (2, "26".into()));
    sqlx::query(
        "ALTER TABLE land_authz.index_cursor RENAME COLUMN hidden_updated_at TO updated_at",
    )
    .execute(pool)
    .await
    .unwrap();

    // The configured trades mirror's SQL failure keeps its diagnostics in
    // the server log and returns the generic database error to the caller.
    let mut settings = state.settings.clone();
    settings.trades_sync_upstream_url = Some(url.clone());
    let mirrored = market_router(Arc::new(super::super::MarketplaceState::new(
        pool.clone(),
        crate::http::Outbound::for_tests(),
        settings,
    )));
    sqlx::query("ALTER TABLE shop.trades_mirror RENAME TO hidden_trades_mirror")
        .execute(pool)
        .await
        .unwrap();
    let (code, page) = get(mirrored, "/v1/nfts?category=ens&isOnSale=true").await;
    assert_eq!(code, StatusCode::INTERNAL_SERVER_ERROR, "{page}");
    assert!(!page.to_string().contains("shop.trades_mirror"), "{page}");
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
