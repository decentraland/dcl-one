use super::*;
use crate::land_authz::LAND_REGISTRY_MAINNET;
use crate::market::{market_router, sale_fold, sale_history};

#[tokio::test]
async fn top_owners_and_item_picks_use_local_rows() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let pool = &db.pool;
    let mut settings = index_settings(100, 20);
    settings.sales_enabled = true;
    for net in ChainNet::ALL {
        let height = nft_index::deployments::Deployments::mainnet(net).start + 20;
        let chain = FakeChain::new(height + net.confirmations(), vec![], 1000, 100);
        chain
            .chain_id
            .store(net.chain_id() as u64, Ordering::SeqCst);
        let url = serve(chain.clone()).await;
        let st = state_over(pool.clone(), &url, settings.clone(), false);
        sqlx::query("INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,last_block_hash,
            full_history,folded_block,folded_at,sale_input_version,sale_verified_through,sale_covered_rewinds)
            SELECT ix,$1,$2,1,$3,$4,true,$3,timezone('UTC',now()),$5,$3,0
            FROM unnest(ARRAY['nft','sales','trade_events']) ix")
            .bind(net.db_name()).bind(net.chain_id()).bind(height as i64)
            .bind(chain.hash_of(height)).bind(sale_history::VERSION).execute(pool).await.unwrap();
        sale_fold::pass(pool, &Rpc::at(&st.http, &url), net)
            .await
            .unwrap();
    }
    let state = state_over(pool.clone(), "http://127.0.0.1", settings, true);
    let app = market_router(state.clone());
    sqlx::query(
        "INSERT INTO shop.mirror_state(mirror,network,source_url,last_complete_pass_at)
        VALUES ('items','ethereum','test',timezone('UTC',now())),
               ('items','matic','test',timezone('UTC',now())),
               ('collections','ethereum','test',timezone('UTC',now()))",
    )
    .execute(pool)
    .await
    .unwrap();
    seed_collection(pool, "ethereum", DAVE, 1, "v1-collection", "v1").await;
    seed_item(
        pool,
        "ethereum",
        DAVE,
        3,
        "v1-collection:hat",
        "Old hat",
        "wearable_v1",
        "hat",
    )
    .await;
    seed_item(
        pool,
        "matic",
        DAVE,
        1,
        "v2-first",
        "Cap",
        "wearable_v2",
        "hat",
    )
    .await;
    seed_item(
        pool,
        "matic",
        DAVE,
        2,
        "v2-second",
        "Visor",
        "wearable_v2",
        "hat",
    )
    .await;
    sqlx::query(
        "INSERT INTO shop.nft(network,contract_address,token_id,owner,item_id,
        wearable_id,minted_block,minted_at,transferred_block,transferred_log,transferred_at)
        VALUES ('ethereum',$1,101,$2,NULL,'hat',1,6,2,0,40),
               ('ethereum',$1,102,NULL,NULL,'hat',1,6,2,0,50),
               ('matic',$3,201,$2,1,NULL,1,6,2,0,25),
               ('matic',$3,202,$2,1,NULL,1,32,1,1,0),
               ('matic',$3,203,$4,2,NULL,1,6,2,0,60),
               ('matic',$3,204,$5,2,NULL,1,6,2,0,70),
               ('matic',$3,205,$6,2,NULL,1,6,2,0,80)",
    )
    .bind(DAVE)
    .bind(BOB)
    .bind(DAVE)
    .bind(ALICE)
    .bind(CARL)
    .bind(nft_index::ZERO_ADDRESS)
    .execute(pool)
    .await
    .unwrap();
    // Distinguish chain copies even though the contract address is shared.
    // The Ethereum purchase of the burned copy still counts for its buyer.
    sqlx::query("WITH purchases(network,log_index,token_id,item_id,buyer,price,currency,unpublished) AS (
        VALUES ('ethereum',1,102,NULL,$2::text,4::numeric,$4::text,0),
               ('matic',2,201,1,$2,2,$5,0), ('matic',3,203,2,$3,11,$5,0),
               ('matic',4,201,1,$2,1000,$5,1)
    ), executions AS (
        INSERT INTO shop.sale_execution(network,block_number,log_index,tx_hash,venue,kind,block_time)
        SELECT p.network, m.block_number+p.unpublished,p.log_index,'0x'||lpad(to_hex(p.log_index),64,'0'),
          $1,'order',1
        FROM purchases p JOIN shop.sale_publication m USING(network)
        RETURNING network,block_number,log_index,tx_hash
    )
    INSERT INTO shop.sale_asset(network,block_number,log_index,ordinal,public_id,beneficiary,
        kind,contract_address,token_id,item_id,seller,buyer,price,currency)
    SELECT e.network,e.block_number,e.log_index,0,e.tx_hash,p.buyer,'order',$1,p.token_id,p.item_id,$6,
        p.buyer,p.price,p.currency FROM executions e JOIN purchases p USING(network,log_index)")
        .bind(DAVE).bind(BOB).bind(ALICE)
        .bind(crate::market::listings::mana_token(1))
        .bind(crate::market::listings::mana_token(137)).bind(CARL)
        .execute(pool).await.unwrap();
    sqlx::query("UPDATE shop.chain_cursor SET last_block=last_block+1 WHERE indexer='nft'")
        .execute(pool)
        .await
        .unwrap();
    let base = format!("/v1/owners/top?creator={CARL}");
    let (status, body) = get_json(&app, &base).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({"data":[
        {"address":BOB,"nfts":3,"items":2,"collections":1,"lastAcquiredAt":40000,"spentWei":"6"},
        {"address":ALICE,"nfts":1,"items":1,"collections":1,"lastAcquiredAt":60000,"spentWei":"11"}
    ],"total":2})
    );
    for (sort, direction, expected) in [
        ("items", "desc", BOB),
        ("collections", "asc", BOB),
        ("recent", "desc", ALICE),
        ("spent", "desc", ALICE),
        ("spent", "asc", BOB),
    ] {
        let path = format!("{base}&sortBy={sort}&orderDirection={direction}&first=1");
        let (status, body) = get_json(&app, &path).await;
        assert_eq!(status, StatusCode::OK, "{path}: {body}");
        assert_eq!(body["data"].as_array().unwrap().len(), 1, "{body}");
        assert_eq!(body["data"][0]["address"], expected, "{path}: {body}");
        assert_eq!(body["total"], 2);
    }
    let (status, empty) = get_json(&app, &format!("{base}&skip=2")).await;
    assert_eq!(status, StatusCode::OK, "{empty}");
    assert_eq!(empty, json!({"data":[],"total":2}));
    for path in [
        "/v1/owners/top?creator=invalid".to_string(),
        format!("{base}&sortBy=invalid"),
        format!("{base}&orderDirection=invalid"),
        format!("{base}&skip=1000000"),
    ] {
        assert_eq!(
            get_json(&app, &path).await.0,
            StatusCode::BAD_REQUEST,
            "{path}"
        );
    }
    let analytics = state.sales_analytics_slots.try_acquire_many(2).unwrap();
    assert_eq!(
        get_json(&app, &format!("{base}&orderDirection=asc"))
            .await
            .0,
        StatusCode::OK,
        "a cold top-owners page does not take the shared sales permits"
    );
    drop(analytics);
    let owner_slot = state.top_owners_slots.try_acquire().unwrap();
    assert_eq!(get_json(&app, &base).await, (StatusCode::OK, body));
    let (status, busy) = get_json(&app, &format!("/v1/owners/top?creator={ALICE}")).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{busy}");
    drop(owner_slot);
    sqlx::query("DELETE FROM shop.mirror_state WHERE mirror='items' AND network='matic'")
        .execute(pool)
        .await
        .unwrap();
    let (status, missing_items) = get_json(&app, &base).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{missing_items}");
    assert_eq!(missing_items["source"], "items-mirror");
    sqlx::query(
        "INSERT INTO shop.mirror_state(mirror,network,source_url,last_complete_pass_at)
        VALUES ('items','matic','test',timezone('UTC',now()))",
    )
    .execute(pool)
    .await
    .unwrap();
    let (status, warm_again) = get_json(&app, &base).await;
    assert_eq!(status, StatusCode::OK, "{warm_again}");
    sqlx::query("UPDATE shop.nft SET owner=$1 WHERE network='matic' AND contract_address=$2 AND token_id=202")
        .bind(ALICE).bind(DAVE).execute(pool).await.unwrap();
    sqlx::query(
        "UPDATE shop.chain_cursor SET folded_block=last_block,folded_at=timezone('UTC',now()),
        sale_verified_through=last_block WHERE network='matic' AND indexer='nft'",
    )
    .execute(pool)
    .await
    .unwrap();
    let (status, newer_nft) = get_json(&app, &base).await;
    assert_eq!(status, StatusCode::OK, "{newer_nft}");
    assert_eq!(newer_nft["data"][0]["address"], ALICE);
    assert_eq!(newer_nft["data"][0]["nfts"], 2);
    assert_eq!(newer_nft["data"][1]["address"], BOB);
    assert_eq!(newer_nft["data"][1]["nfts"], 2);
    let item = format!("{DAVE}-1");
    sqlx::query(
        "WITH lists AS (
        INSERT INTO shop.lists(name,user_address,is_private)
        VALUES ('one',$2,true),('two',$2,true),('three',$3,true) RETURNING id,user_address
    )
    INSERT INTO shop.picks(item_id,user_address,list_id) SELECT $1,user_address,id FROM lists",
    )
    .bind(&item)
    .bind(ALICE)
    .bind(BOB)
    .execute(pool)
    .await
    .unwrap();
    let path = format!("/v1/items?contractAddress={DAVE}&network=MATIC&sortBy=name&first=1");
    let (status, picked) = get_json(&app, &path).await;
    assert_eq!(status, StatusCode::OK, "{picked}");
    assert_eq!(picked["data"][0]["id"], item);
    assert_eq!(picked["data"][0]["picks"], json!({"count":2}));
    let (status, unpicked) = get_json(&app, &format!("{path}&skip=1")).await;
    assert_eq!(status, StatusCode::OK, "{unpicked}");
    assert_eq!(unpicked["data"][0]["id"], format!("{DAVE}-2"));
    assert_eq!(unpicked["data"][0]["picks"], json!({"count":0}));
    assert_eq!(unpicked["total"], 2);
    sqlx::query("ALTER TABLE shop.picks RENAME TO picks_unavailable")
        .execute(pool)
        .await
        .unwrap();
    let (status, unavailable_picks) = get_json(&app, &path).await;
    assert_eq!(
        status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "{unavailable_picks}"
    );
    sqlx::query("INSERT INTO shop.chain_cursor
        (indexer,network,chain_id,start_block,last_block,last_block_hash,full_history,folded_block,folded_at)
        SELECT 'collections',network,chain_id,start_block,last_block,last_block_hash,true,folded_block,folded_at
        FROM shop.chain_cursor WHERE indexer='nft' AND network='matic'")
        .execute(pool).await.unwrap();
    sqlx::query("INSERT INTO shop.collection_projection(network,owned_at,projected_through,generation)
        SELECT network,timezone('UTC',now()),block_number,1 FROM shop.sale_publication WHERE network='matic'")
        .execute(pool).await.unwrap();
    let (status, native) = get_json(&app, &base).await;
    assert_eq!(status, StatusCode::OK, "{native}");
    assert_eq!(native["data"][1]["nfts"], 2);
    sqlx::query("UPDATE shop.items SET creator=$1 WHERE network='matic' AND contract_address=$2 AND item_id=1")
        .bind(ALICE).bind(DAVE).execute(pool).await.unwrap();
    sqlx::query(
        "UPDATE shop.collection_projection
        SET generation=generation+1,projected_through=projected_through+1 WHERE network='matic'",
    )
    .execute(pool)
    .await
    .unwrap();
    let (status, newer_items) = get_json(&app, &base).await;
    assert_eq!(status, StatusCode::OK, "{newer_items}");
    assert_eq!(newer_items["data"][1]["address"], BOB);
    assert_eq!(newer_items["data"][1]["nfts"], 1);
    sqlx::query(
        "UPDATE shop.chain_cursor SET folded_at=NULL,folded_block=NULL
        WHERE indexer='nft' AND network='ethereum'",
    )
    .execute(pool)
    .await
    .unwrap();
    let (status, withdrawn) = get_json(&app, &base).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{withdrawn}");
    assert_eq!(withdrawn["source"], "sales-index");
    db.drop().await;
}

#[tokio::test]
async fn sales_page_reads_one_published_network_and_refuses_a_withdrawn_warm_page() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let net = ChainNet::Ethereum;
    let block = nft_index::deployments::Deployments::mainnet(net).start + 75;
    let chain = FakeChain::new(block + 14, vec![], 1000, 10);
    chain.chain_id.store(1, Ordering::SeqCst);
    let url = serve(chain.clone()).await;
    let mut settings = index_settings(100, 10);
    settings.sales_enabled = true;
    settings.collections_enabled = true;
    let state = state_over(db.pool.clone(), &url, settings, false);
    let app = market_router(state.clone());
    sqlx::query(
        "INSERT INTO shop.chain_cursor
            (indexer,network,chain_id,start_block,last_block,last_block_hash,full_history,
             folded_block,folded_at,sale_input_version,sale_verified_through,sale_covered_rewinds)
         SELECT indexer,'ethereum',1,1,$1,$2,true,$1,timezone('UTC',now()),$3,$1,0
         FROM unnest(ARRAY['nft','sales','trade_events']) AS indexer",
    )
    .bind(block as i64)
    .bind(chain.hash_of(block))
    .bind(sale_history::VERSION)
    .execute(&db.pool)
    .await
    .unwrap();
    let price = (U256::from(1) << 190usize).to_string();
    // The same transaction buys two parcels at the same price and timestamp.
    sqlx::query(
        "INSERT INTO shop.nft_event
            (network,block_number,log_index,block_time,kind,emitter,contract_address,
             token_id,from_address,to_address,amount,tx_hash)
         SELECT 'ethereum',$1,i,1700000000,'order_successful',$2,$3,i,$4,$5,$6::text::numeric,$7
         FROM generate_series(1,2) AS i",
    )
    .bind(block as i64)
    .bind(nft_index::deployments::Deployments::mainnet(net).markets[0].0)
    .bind(LAND_REGISTRY_MAINNET)
    .bind(ALICE)
    .bind(BOB)
    .bind(&price)
    .bind(format!("0x{:064x}", 73))
    .execute(&db.pool)
    .await
    .unwrap();
    sale_fold::pass(&db.pool, &Rpc::at(&state.http, &url), net)
        .await
        .unwrap();
    let mut snapshot = sale_fold::snapshot(&db.pool, &[net], chrono::Utc::now())
        .await
        .unwrap()
        .unwrap();
    let mut testnet = state_over(
        db.pool.clone(),
        &url,
        state.settings.market.indexes.clone(),
        false,
    );
    Arc::get_mut(&mut testnet).unwrap().settings.eth_network = "sepolia".into();
    let (code, disabled) = get_json(&market_router(testnet), "/v1/sales?network=ETHEREUM").await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{disabled}");
    assert_eq!(disabled["message"], "the mainnet sales index is disabled");
    let timeout: String = sqlx::query_scalar("SHOW statement_timeout")
        .fetch_one(&mut *snapshot.transaction)
        .await
        .unwrap();
    assert_eq!(
        timeout,
        format!(
            "{}s",
            crate::market::nft_reads::READ_STATEMENT_TIMEOUT.as_secs()
        )
    );
    drop(snapshot);
    let path =
        "/v1/sales?network=ETHEREUM&category=parcel&type=order&sortBy=most_expensive&first=1";
    let (status, first) = get_json(&app, path).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let (status, second) = get_json(&app, &format!("{path}&skip=1")).await;
    assert_eq!(status, StatusCode::OK, "{second}");
    for page in [&first, &second] {
        assert_eq!(page["total"], 2);
        let rows = page["data"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["price"], price);
        assert_eq!(rows[0]["buyer"], BOB);
        assert_eq!(rows[0]["seller"], ALICE);
        assert!(!rows[0]["id"].as_str().unwrap().is_empty());
    }
    assert_eq!(
        first["data"][0],
        json!({"id":first["data"][0]["id"],"itemId":null,
        "contractAddress":LAND_REGISTRY_MAINNET,"buyer":BOB,"chainId":1,"network":"ETHEREUM",
        "price":price,"seller":ALICE,"timestamp":1700000000000i64,"tokenId":"1",
        "txHash":format!("0x{:064x}",73),"type":"order"})
    );
    let exact = format!(
        "/v1/sales?network=ETHEREUM&buyer={BOB}&contractAddress={LAND_REGISTRY_MAINNET}&tokenId=1&from=1700000000000&to=1700000000000&minPrice={price}&maxPrice={price}"
    );
    let (status, filtered) = get_json(&app, &exact).await;
    assert_eq!(status, StatusCode::OK, "{filtered}");
    assert_eq!(filtered["data"], first["data"]);
    for filter in [
        format!("buyer={ALICE}"),
        "itemId=7".into(),
        "tokenId=3".into(),
        "from=1700000000001".into(),
        "to=1699999999999".into(),
        "maxPrice=1".into(),
        "category=estate".into(),
    ] {
        let (status, filtered) =
            get_json(&app, &format!("/v1/sales?network=ETHEREUM&{filter}")).await;
        assert_eq!(status, StatusCode::OK, "{filter}: {filtered}");
        assert_eq!(filtered["total"], 0, "{filter}");
    }
    assert_eq!(
        get_json(&app, "/v1/sales?network=ETHEREUM&minPrice=abc")
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_ne!(first["data"][0]["tokenId"], second["data"][0]["tokenId"]);
    assert_ne!(first["data"][0]["id"], second["data"][0]["id"]);
    assert_eq!(get_json(&app, path).await.1, first);
    assert_eq!(
        get_json(&app, "/v1/sales").await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );

    let polygon = nft_index::deployments::Deployments::mainnet(ChainNet::Matic).start + 20;
    let polygon_chain = FakeChain::new(polygon + 5, vec![], 1000, 10);
    let polygon_url = serve(polygon_chain.clone()).await;
    sqlx::query("INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,last_block_hash,
        full_history,folded_block,folded_at,sale_input_version,sale_verified_through,sale_covered_rewinds)
        SELECT ix,'matic',137,1,$1,$2,true,$1,timezone('UTC',now()),$3,$1,0 FROM unnest(ARRAY['nft','sales','trade_events']) ix")
        .bind(polygon as i64).bind(polygon_chain.hash_of(polygon)).bind(sale_history::VERSION).execute(&db.pool).await.unwrap();
    sale_fold::pass(
        &db.pool,
        &Rpc::at(&state.http, &polygon_url),
        ChainNet::Matic,
    )
    .await
    .unwrap();
    let (activity, _) = crate::market::activity::user_activity(&state, ALICE, None, None).await;
    assert_eq!(
        activity
            .iter()
            .filter(|event| event.event_type == "sale_seller")
            .count(),
        1,
        "two sales in one transaction make one seller activity"
    );
    sqlx::query("INSERT INTO shop.mirror_state(mirror,network,source_url,last_complete_pass_at)
        SELECT mirror,net,'test',timezone('UTC',now()) FROM unnest(ARRAY['items','collections']) mirror CROSS JOIN unnest(ARRAY['ethereum','matic']) net")
        .execute(&db.pool).await.unwrap();
    let (code, missing) = get_json(&app, "/v1/volume/all").await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{missing}");
    // An indirect fee collector and the store both earn protocol fees.
    // Venue and NAME burns remain in the burn bucket. The original gross
    // still includes the venue burn and never includes NAME registration.
    sqlx::query(
        "UPDATE shop.sale_analytics_day SET unknown_splits=0, venue_fees=11,
        store_fees=3,creator_earnings=7,name_fees=150,burn_fees=120 WHERE network='ethereum'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO shop.sale_execution(network,block_number,log_index,tx_hash,venue,kind,block_time,receipt_read)
        SELECT 'ethereum',$1,i,$2,$3,'name_fee',1700000000,true FROM generate_series(3,4) i")
        .bind(block as i64).bind(format!("0x{:064x}",80)).bind(ALICE).execute(&db.pool).await.unwrap();
    let mana = crate::market::listings::mana_token(1).unwrap();
    sqlx::query("INSERT INTO shop.sale_payment(network,block_number,log_index,ordinal,currency,gross,consideration,settled)
        SELECT 'ethereum',$1,i,0,$2,CASE WHEN i=3 THEN 50 ELSE 100 END,true,true FROM generate_series(3,4) i")
        .bind(block as i64).bind(mana).execute(&db.pool).await.unwrap();
    sqlx::query("INSERT INTO shop.sale_payout(network,block_number,log_index,payment_ordinal,ordinal,role,recipient,amount)
        SELECT network,block_number,log_index,0,0,CASE WHEN log_index=3 THEN 'name_fee' ELSE 'burn' END,$1,gross
        FROM shop.sale_payment WHERE log_index IN (3,4)").bind(CARL).execute(&db.pool).await.unwrap();
    // The same day already has a published NAME fee. A later execution
    // and its payout can only contribute after its own block is published.
    sqlx::query("WITH e AS (INSERT INTO shop.sale_execution(network,block_number,log_index,tx_hash,venue,kind,block_time,receipt_read)
        VALUES('ethereum',$1,5,$2,$3,'name_fee',1700000010,true) RETURNING network,block_number,log_index),
        p AS (INSERT INTO shop.sale_payment(network,block_number,log_index,ordinal,currency,gross,consideration,settled)
        SELECT e.*,0,$4,30,true,true FROM e RETURNING network,block_number,log_index)
        INSERT INTO shop.sale_payout(network,block_number,log_index,payment_ordinal,ordinal,role,recipient,amount)
        SELECT p.*,0,0,'name_fee',$3,30 FROM p")
        .bind(block as i64+1).bind(format!("0x{:064x}",81)).bind(CARL).bind(mana)
        .execute(&db.pool).await.unwrap();
    let (code, volume) = get_json(&app, "/v1/volume/all").await;
    assert_eq!(code, StatusCode::OK, "{volume}");
    assert_eq!(
        volume["data"],
        json!({"sales":2,"volume":(U256::from(1)<<191usize).to_string(),
        "creatorsEarnings":"7","daoEarnings":"64"})
    );
    let (code, week) = get_json(&app, "/v1/volume/week").await;
    assert_eq!(code, StatusCode::OK, "{week}");
    assert_eq!(
        week["data"],
        json!({"sales":0,"volume":"0","creatorsEarnings":"0","daoEarnings":"0"})
    );
    let generation = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT generation FROM shop.sale_publication WHERE network='matic'",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    // Exercise the real fold, with an ordinary tip advance while the old
    // publication is cached. A repair of the same height must miss the cache.
    let later = polygon + 1;
    sqlx::query(
        "UPDATE shop.chain_cursor SET last_block=$1,last_block_hash=$2,folded_block=$1,
        folded_at=timezone('UTC',now()),sale_verified_through=$1 WHERE network='matic'",
    )
    .bind(later as i64)
    .bind(polygon_chain.hash_of(later))
    .execute(&db.pool)
    .await
    .unwrap();
    let polygon_rpc = Rpc::at(&state.http, &polygon_url);
    sale_fold::pass(&db.pool, &polygon_rpc, ChainNet::Matic)
        .await
        .unwrap();
    let publication: (i64, uuid::Uuid) = sqlx::query_as(
        "SELECT block_number,generation FROM shop.sale_publication WHERE network='matic'",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(publication, (later as i64, generation));
    let slots = state.sales_analytics_slots.try_acquire_many(2).unwrap();
    assert_eq!(
        get_json(&app, "/v1/volume/all").await,
        (StatusCode::OK, volume.clone())
    );
    sqlx::query(
        "UPDATE shop.chain_cursor SET sale_history_revision=sale_history_revision+1,
        sale_dirty_from=$1 WHERE network='matic' AND indexer='sales'",
    )
    .bind(later as i64)
    .execute(&db.pool)
    .await
    .unwrap();
    let (code, gap) = get_json(&app, "/v1/volume/all").await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{gap}");
    let history_repaired = sale_fold::pass(&db.pool, &polygon_rpc, ChainNet::Matic).await;
    history_repaired.unwrap();
    let updated = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT generation FROM shop.sale_publication WHERE network='matic'",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_ne!(updated, generation);
    let (code, busy) = get_json(&app, "/v1/volume/all").await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{busy}");
    assert_eq!(busy["message"], "sales analytics is busy");
    drop(slots);
    sqlx::query("UPDATE shop.sale_publication SET published_at=NULL WHERE network='matic'")
        .execute(&db.pool)
        .await
        .unwrap();
    sale_fold::pass(&db.pool, &polygon_rpc, ChainNet::Matic)
        .await
        .unwrap();
    let restored = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT generation FROM shop.sale_publication WHERE network='matic' AND published_at IS NOT NULL",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_ne!(restored, updated);
    // Both item categories share an address and have equal sales. Their public
    // rankings split by item type and honor the rarity and UTC-day filters.
    seed_item(
        &db.pool,
        "matic",
        CARL,
        1,
        "wearable-a",
        "Cap",
        "wearable_v2",
        "hat",
    )
    .await;
    seed_item(
        &db.pool, "matic", CARL, 2, "emote-b", "Wave", "emote_v1", "dance",
    )
    .await;
    seed_item(
        &db.pool,
        "matic",
        CARL,
        3,
        "wearable-c",
        "Top",
        "wearable_v2",
        "upper_body",
    )
    .await;
    let day = chrono::Utc::now().timestamp() / 86400 * 86400;
    sqlx::query(
        "INSERT INTO shop.sale_item_day(network,day,currency,contract_address,item_id,sales,volume)
        VALUES('matic',$1,$2,$3,1,2,100),('matic',$1,$2,$3,2,3,300),('matic',$1,$2,$3,3,4,25),
              ('matic',$1-864000,$2,$3,1,2,150)",
    )
    .bind(day)
    .bind(crate::market::listings::mana_token(137))
    .bind(CARL)
    .execute(&db.pool)
    .await
    .unwrap();
    for (url, expected) in [
        (
            "/v1/rankings/wearables/week?first=1",
            json!([{"id":format!("{CARL}-1"),"sales":2,"volume":"100"}]),
        ),
        (
            "/v1/rankings/wearables/all?first=1&sortBy=most_sales",
            json!([{"id":format!("{CARL}-1"),"sales":4,"volume":"250"}]),
        ),
        (
            "/v1/rankings/wearables/week?first=1&sortBy=most_sales",
            json!([{"id":format!("{CARL}-3"),"sales":4,"volume":"25"}]),
        ),
        (
            "/v1/rankings/emotes/month?rarity=rare",
            json!([{"id":format!("{CARL}-2"),"sales":3,"volume":"300"}]),
        ),
        ("/v1/rankings/emotes/month?rarity=mythic", json!([])),
    ] {
        let (code, resp) = get_json(&app, url).await;
        assert_eq!(code, StatusCode::OK, "{url} {resp}");
        assert_eq!(resp["data"], expected, "{url}");
    }
    // A native catalogue takes precedence over a previously complete
    // mirror. Its publication is checked even for a warm rank result.
    let ranked_path = "/v1/rankings/emotes/all";
    assert_eq!(get_json(&app, ranked_path).await.0, StatusCode::OK);
    sqlx::query(
        "INSERT INTO shop.collection_projection(network,owned_at,projected_through)
        VALUES('matic',timezone('UTC',now()),NULL)",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let (code, pending) = get_json(&app, ranked_path).await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{pending}");
    assert_eq!(pending["source"], "collections-index");
    sqlx::query("INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,last_block_hash,full_history,folded_block,folded_at)
        SELECT 'collections',network,chain_id,start_block,last_block,last_block_hash,true,folded_block,folded_at
        FROM shop.chain_cursor WHERE indexer='nft' AND network='matic'").execute(&db.pool).await.unwrap();
    sqlx::query("UPDATE shop.collection_projection SET projected_through=$1 WHERE network='matic'")
        .bind(later as i64)
        .execute(&db.pool)
        .await
        .unwrap();
    let ranked = get_json(&app, ranked_path).await;
    assert_eq!(ranked.0, StatusCode::OK);
    let slots = state.sales_analytics_slots.try_acquire_many(2).unwrap();
    sqlx::query(
        "UPDATE shop.collection_projection SET generation=generation+1 WHERE network='matic'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    assert_eq!(get_json(&app, ranked_path).await, ranked);
    sqlx::query(
        "UPDATE shop.chain_cursor SET sale_history_revision=sale_history_revision+1
        WHERE network='matic' AND indexer='collections'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let (code, busy) = get_json(&app, ranked_path).await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{busy}");
    assert_eq!(busy["message"], "sales analytics is busy");
    drop(slots);
    sqlx::query("DELETE FROM shop.collection_projection WHERE network='matic'")
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM shop.chain_cursor WHERE indexer='collections' AND network='matic'")
        .execute(&db.pool)
        .await
        .unwrap();
    let (code, collectors) = get_json(&app, "/v1/rankings/collectors/all?sortBy=most_sales").await;
    assert_eq!(code, StatusCode::OK, "{collectors}");
    assert_eq!(
        collectors["data"],
        json!([{"id":BOB,"purchases":2,
       "spent":(U256::from(1)<<191usize).to_string(),"creatorsSupported":1,"uniqueAndMythicItems":0}])
    );
    let (code, inactive) = get_json(&app, "/v1/rankings/collectors/week").await;
    assert_eq!(code, StatusCode::OK, "{inactive}");
    assert_eq!(inactive["data"], json!([]));
    seed_collection(
        &db.pool,
        "matic",
        CARL,
        1,
        "collection-c",
        "Store collection",
    )
    .await;
    let (code, empty_store) = get_json(&app, &format!("/v1/stores/{CARL}/stats?from=1&to=2")).await;
    assert_eq!(code, StatusCode::OK, "{empty_store}");
    assert_eq!(empty_store["data"]["currency"], "MANA");
    assert_eq!(empty_store["data"]["summary"]["total"], 0);
    let item = &empty_store["data"]["collections"][0]["items"][0];
    assert_eq!(item["sold"], 0, "{empty_store}");
    assert_eq!(item["minted"], "1", "{empty_store}");
    let (code, empty) = get_json(&app, &format!("/v1/proceeds?seller={CARL}")).await;
    assert_eq!(code, StatusCode::OK, "{empty}");
    assert_eq!(empty, json!({"data":[],"total":0}));
    let (code, empty) = get_json(&app, &format!("/v1/proceeds/summary?seller={CARL}")).await;
    assert_eq!(code, StatusCode::OK, "{empty}");
    assert_eq!(empty["data"]["netWei"], "0");
    assert_eq!(empty["data"]["settlement"], "mana");
    assert_eq!(empty["data"]["treasuryRouting"], false);
    let summary_path = format!("/v1/sales/summary?seller={ALICE}");
    let (code, summary) = get_json(&app, &summary_path).await;
    assert_eq!(code, StatusCode::OK, "{summary}");
    assert_eq!(summary["data"]["total"], 2);
    assert_eq!(summary["data"]["resales"], 2);
    let gross = (U256::from(1) << 191usize).to_string();
    assert_eq!(summary["data"]["earnedWei"], gross);
    let (code, store) = get_json(&app, &format!("/v1/stores/{ALICE}/stats")).await;
    assert_eq!(code, StatusCode::OK, "{store}");
    assert_eq!(store["data"]["summary"], summary["data"]);
    assert_eq!(summary["data"]["earnedUsd"], "0.000000");
    assert_eq!(summary["data"]["unpricedSales"], 2);
    sqlx::query(
        "INSERT INTO shop.mana_usd_daily(day,usd,round_id)
         VALUES ('2023-11-14',0.125,42),('2023-11-15',NULL,43)",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let rates_path = "/v1/rates/mana-usd?from=1699920000001&to=1700092799999";
    assert_eq!(
        get_json(&app, rates_path).await,
        (
            StatusCode::OK,
            json!({"data":[{"day":"2023-11-14","usd":"0.125"}]})
        )
    );
    for invalid in [
        "?from=1",
        "?from=2&to=1",
        "?from=0&to=9000000000000",
        "?from=-1&to=1",
    ] {
        assert_eq!(
            get_json(&app, &format!("/v1/rates/mana-usd{invalid}"))
                .await
                .0,
            StatusCode::BAD_REQUEST,
        );
    }
    let (code, priced) = get_json(&app, &summary_path).await;
    assert_eq!(code, StatusCode::OK, "{priced}");
    assert_eq!(
        priced["data"]["earnedUsd"],
        "392318858461667547739736838950479151006.397215"
    );
    assert_eq!(priced["data"]["unpricedSales"], 0);
    assert_eq!(
        priced["data"]["byCollection"][0]["earnedUsd"],
        "392318858461667547739736838950479151006.397215"
    );
    assert_eq!(priced["data"]["byCollection"][0]["unpricedSales"], 0);
    let proceeds_path = format!("/v1/proceeds?seller={ALICE}&network=ETHEREUM");
    assert_eq!(
        get_json(&app, &proceeds_path).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    sqlx::query("INSERT INTO shop.sale_payout(network,block_number,log_index,payment_ordinal,ordinal,role,recipient,amount)
         SELECT network,block_number,log_index,p.ordinal,v.n,v.role,v.recipient,v.amount FROM shop.sale_payment p
         CROSS JOIN LATERAL (VALUES(0,'seller',$1::text,p.gross-15),(1,'venue_fee',$2::text,10),(2,'royalty',$3::text,5)) v(n,role,recipient,amount) WHERE p.log_index IN (1,2)")
         .bind(ALICE).bind(CARL).bind(DAVE).execute(&db.pool).await.unwrap();
    sqlx::query("UPDATE shop.sale_payment SET settled=true")
        .execute(&db.pool)
        .await
        .unwrap();
    sale_fold::rollups(&mut db.pool.acquire().await.unwrap(), net, &[1699920000])
        .await
        .unwrap();
    let (code, proceeds) = get_json(&app, &proceeds_path).await;
    assert_eq!(code, StatusCode::OK, "{proceeds}");
    assert_eq!(proceeds["total"], 2);
    assert_eq!(proceeds["data"][0]["grossWei"], price);
    assert_eq!(proceeds["data"][0]["feesWei"], "10");
    assert_eq!(proceeds["data"][0]["royaltiesWei"], "5");
    assert_eq!(
        proceeds["data"][0]["netWei"],
        ((U256::from(1) << 190usize) - U256::from(15)).to_string()
    );
    let (code, paid) = get_json(
        &app,
        &format!("/v1/proceeds/summary?seller={ALICE}&network=ETHEREUM"),
    )
    .await;
    assert_eq!(code, StatusCode::OK, "{paid}");
    assert_eq!(paid["data"]["grossWei"], gross);
    assert_eq!(paid["data"]["feesWei"], "20");
    assert_eq!(paid["data"]["royaltiesWei"], "10");
    assert_eq!(paid["data"]["royaltiesEarnedWei"], "0");
    let seller_net = ((U256::from(1) << 191usize) - U256::from(30)).to_string();
    for (address, sales, purchases, spent, earned) in [
        (ALICE, 2, 0, "0", seller_net.as_str()),
        (BOB, 0, 2, gross.as_str(), "0"),
    ] {
        let (code, page) =
            get_json(&app, &format!("/v1/accounts?network=ETHEREUM&id={address}")).await;
        assert_eq!(code, StatusCode::OK, "{page}");
        assert_eq!(
            page,
            json!({"total":1,"data":[{"id":address,"address":address,"sales":sales,"purchases":purchases,
            "spent":spent,"earned":earned,"royalties":"0","collections":0}]})
        );
    }
    let creator_path = "/v1/rankings/creators/all?sortBy=most_volume";
    let (code, unavailable) = get_json(&app, creator_path).await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{unavailable}");
    assert_eq!(unavailable["source"], "collections-index");

    sqlx::query("INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,last_block_hash,full_history,folded_block,folded_at)
         SELECT 'collections',network,chain_id,start_block,last_block,last_block_hash,true,folded_block,folded_at
         FROM shop.chain_cursor WHERE indexer='nft' AND network='matic'").execute(&db.pool).await.unwrap();
    sqlx::query("INSERT INTO shop.collection_event(network,block_number,log_index,block_time,kind,contract_address,account,previous_account)
        VALUES('matic',$1-5,1,1,'proxy_created',$2,$3,NULL),('matic',$1-1,1,2,'creatorship_transferred',$2,$3,$2)")
        .bind(polygon as i64).bind(CARL).bind(DAVE).execute(&db.pool).await.unwrap();
    sqlx::query("INSERT INTO shop.sale_execution(network,block_number,log_index,tx_hash,venue,kind,block_time,receipt_read)
        VALUES('matic',$1,5,$2,$3,'store',$4,true)").bind(polygon as i64-2).bind(format!("0x{:064x}",93)).bind(CARL)
        .bind(day).execute(&db.pool).await.unwrap();
    let mana = crate::market::listings::mana_token(137);
    sqlx::query("INSERT INTO shop.sale_asset(network,block_number,log_index,ordinal,public_id,beneficiary,kind,contract_address,token_id,item_id,seller,buyer,price,currency)
        VALUES('matic',$1,5,0,'polygon-mint',$2,'mint',$3,12,1,$3,$2,90,$4)").bind(polygon as i64-2).bind(BOB)
        .bind(CARL).bind(mana).execute(&db.pool).await.unwrap();
    sqlx::query("INSERT INTO shop.sale_payment(network,block_number,log_index,ordinal,currency,gross,settled)
        VALUES('matic',$1,5,0,$2,90,true)").bind(polygon as i64-2).bind(mana).execute(&db.pool).await.unwrap();
    sqlx::query("INSERT INTO shop.sale_payout(network,block_number,log_index,payment_ordinal,ordinal,role,recipient,amount)
        VALUES('matic',$1,5,0,0,'seller',$2,80),('matic',$1,5,0,1,'store_fee',$2,7),('matic',$1,5,0,2,'royalty',$3,3)").bind(polygon as i64-2)
        .bind(CARL).bind(DAVE).execute(&db.pool).await.unwrap();
    sqlx::query(
        "INSERT INTO shop.sale_account_total(network,currency,address,sales,purchases,spent,gross_sold,
         net_sold,unknown_proceeds,received,royalties,fees_received)
         VALUES ('matic',$1,$2,1,0,0,90,80,0,87,0,7),('matic',$1,$3,0,0,0,0,0,0,3,3,0)",
    )
    .bind(mana)
    .bind(CARL)
    .bind(DAVE)
    .execute(&db.pool)
    .await
    .unwrap();
    for (from, sold, earned) in [(day * 1000, 1, "90"), (day * 1000 + 1, 0, "0")] {
        let (code, stats) = get_json(&app, &format!("/v1/stores/{CARL}/stats?from={from}")).await;
        assert_eq!(code, StatusCode::OK, "{stats}");
        let collection = &stats["data"]["collections"][0];
        assert_eq!(collection["sold"], sold);
        assert_eq!(collection["earnedWei"], earned);
        let item = collection["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["itemId"] == "1")
            .unwrap();
        assert_eq!(item["sold"], sold);
        assert_eq!(item["soldLifetime"], 1);
        assert_eq!(item["earnedWei"], earned);
    }
    for (path, earned) in [
        (creator_path, "87"),
        ("/v1/rankings/creators/week?sortBy=most_sales", "80"),
    ] {
        let (code, creators) = get_json(&app, path).await;
        assert_eq!(code, StatusCode::OK, "{creators}");
        assert_eq!(
            creators["data"],
            json!([{"id":CARL,"sales":1,"earned":earned,"uniqueCollectors":1,"collections":1}]),
            "{path}"
        );
    }
    // Two explicitly priced store lines share a receipt. Each payout belongs
    // to the corresponding asset ordinal.
    sqlx::query("INSERT INTO shop.sale_asset(network,block_number,log_index,ordinal,public_id,beneficiary,kind,contract_address,token_id,item_id,seller,buyer,price,currency)
        SELECT network,block_number,log_index,1,'polygon-mint-2',beneficiary,kind,contract_address,13,1,seller,buyer,41,currency
        FROM shop.sale_asset WHERE public_id='polygon-mint'").execute(&db.pool).await.unwrap();
    sqlx::query("WITH extra AS (
        INSERT INTO shop.sale_payment(network,block_number,log_index,ordinal,currency,gross,settled)
        SELECT network,block_number,log_index,1,currency,41,true FROM shop.sale_asset WHERE public_id='polygon-mint-2'
        RETURNING network,block_number,log_index,ordinal)
        INSERT INTO shop.sale_payout(network,block_number,log_index,payment_ordinal,ordinal,role,recipient,amount)
        SELECT network,block_number,log_index,extra.ordinal,v.n,v.role,$1,v.amount FROM extra
        CROSS JOIN (VALUES(0,'seller',40),(1,'store_fee',1)) v(n,role,amount)")
        .bind(CARL).execute(&db.pool).await.unwrap();
    let (code, lines) = get_json(&app, &format!("/v1/proceeds?seller={CARL}&network=MATIC")).await;
    assert_eq!(code, StatusCode::OK, "{lines}");
    assert_eq!(lines["total"], 2);
    let net_amounts: Vec<_> = lines["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| {
            (
                a["grossWei"].as_str().unwrap(),
                a["netWei"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(net_amounts, [("90", "80"), ("41", "40")]);
    // Only CARL is a candidate for this unresolved store seller. A different
    // wallet, even on the same network, keeps its valid empty response.
    sqlx::query("WITH e AS (INSERT INTO shop.sale_execution(network,block_number,log_index,tx_hash,venue,kind,block_time,receipt_read)
        VALUES('matic',$1,10,$2,$3,'store',$4,true) RETURNING network,block_number,log_index),
        a AS (INSERT INTO shop.sale_asset(network,block_number,log_index,ordinal,public_id,beneficiary,kind,contract_address,token_id,item_id,seller,buyer,price,currency)
        SELECT e.*,0,'uncertain-store-line',$5,'mint',$6,1,99,NULL,$5,100,$7 FROM e)
        INSERT INTO shop.sale_payment(network,block_number,log_index,ordinal,currency,beneficiary,gross,settled)
        SELECT e.*,0,$7,$3,100,false FROM e")
        .bind(polygon as i64).bind(format!("0x{:064x}",94)).bind(CARL).bind(day+10).bind(BOB).bind(DAVE).bind(mana)
        .execute(&db.pool).await.unwrap();
    for endpoint in [
        "/v1/sales/summary",
        "/v1/proceeds",
        "/v1/proceeds/summary",
        "/v1/sales",
    ] {
        for (seller, expected) in [
            (CARL, StatusCode::SERVICE_UNAVAILABLE),
            (ALICE, StatusCode::OK),
        ] {
            let (status, body) =
                get_json(&app, &format!("{endpoint}?seller={seller}&network=MATIC")).await;
            assert_eq!(status, expected, "{endpoint} {seller} {body}");
        }
    }
    assert_eq!(
        get_json(&app, "/v1/sales?network=MATIC&first=1").await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    sqlx::query(
        "UPDATE shop.sale_payment SET beneficiary=NULL WHERE network='matic' AND log_index=10",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        get_json(&app, &format!("/v1/proceeds?seller={CARL}&network=MATIC"))
            .await
            .0,
        StatusCode::OK
    );
    // Fold a v1 order from its chain events. Its nullable ledger item ID
    // stays empty, so the creator lookup has to follow Issue's wearable ID.
    let v1_block = block as i64 + 1;
    seed_collection(
        &db.pool,
        "ethereum",
        CARL,
        block,
        "v1-collection",
        "Vintage",
    )
    .await;
    seed_item(
        &db.pool,
        "ethereum",
        CARL,
        9,
        "v1-collection:wearable",
        "Vintage hat",
        "wearable_v1",
        "hat",
    )
    .await;
    let complete_ethereum_items =
        "UPDATE shop.mirror_state SET last_complete_pass_at=timezone('UTC',now()),
         pass_started_at=(SELECT min(mirrored_at) FROM shop.items WHERE network='ethereum')
         WHERE network='ethereum' AND mirror='items'";
    sqlx::query(complete_ethereum_items)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE shop.collections SET creator=$1 WHERE network='ethereum' AND contract_address=$2",
    )
    .bind(BOB)
    .bind(CARL)
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO shop.nft_contract(network,contract_address,walked_from_block)
        VALUES('ethereum',$1,$2)",
    )
    .bind(CARL)
    .bind(block as i64)
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO shop.nft_event(network,block_number,log_index,block_time,kind,emitter,
         contract_address,token_id,from_address,to_address,amount,detail,tx_hash)
         VALUES('ethereum',$1,0,$2,'issue_v1',$3,$3,42,NULL,$4,42,'wearable',NULL),
               ('ethereum',$1,1,$2,'order_successful',$5,$3,42,$4,$6,1000,NULL,$7)",
    )
    .bind(v1_block)
    .bind(1700000001i64)
    .bind(CARL)
    .bind(ALICE)
    .bind(nft_index::deployments::Deployments::mainnet(net).markets[0].0)
    .bind(BOB)
    .bind(format!("0x{:064x}", 96))
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE shop.chain_cursor SET last_block=$1,last_block_hash=$2,
         folded_block=$1,folded_at=timezone('UTC',now()),sale_verified_through=$1
         WHERE network='ethereum'",
    )
    .bind(v1_block)
    .bind(chain.hash_of(v1_block as u64))
    .execute(&db.pool)
    .await
    .unwrap();
    sale_fold::pass(&db.pool, &Rpc::at(&state.http, &url), net)
        .await
        .unwrap();
    let item_id: Option<String> = sqlx::query_scalar(
        "SELECT item_id::text FROM shop.sale_asset WHERE network='ethereum' AND block_number=$1 AND log_index=1"
    ).bind(v1_block).fetch_one(&db.pool).await.unwrap();
    assert_eq!(item_id, None);
    sqlx::query("UPDATE shop.sale_payment SET settled=true WHERE network='ethereum' AND block_number=$1 AND log_index=1")
        .bind(v1_block).execute(&db.pool).await.unwrap();
    sqlx::query("INSERT INTO shop.sale_payout(network,block_number,log_index,payment_ordinal,ordinal,role,recipient,amount)
        VALUES('ethereum',$1,1,0,0,'seller',$2,995),('ethereum',$1,1,0,1,'royalty',$3,5)")
        .bind(v1_block).bind(ALICE).bind(DAVE).execute(&db.pool).await.unwrap();
    sqlx::query("UPDATE shop.mirror_state SET last_complete_pass_at=NULL WHERE network='ethereum' AND mirror='collections'")
        .execute(&db.pool).await.unwrap();
    let (code, incomplete) = get_json(&app, &format!("/v1/sales/summary?seller={CARL}")).await;
    assert_eq!(code, StatusCode::OK, "{incomplete}");
    assert_eq!(incomplete["data"]["royalties"]["royaltiesWei"], Value::Null);
    assert!(incomplete["data"]["earnedUsd"].is_string());
    // The pre-0095 writer updates the active pass and the completion
    // timestamp. The database derives the matching completed start.
    let complete_ethereum_collections =
        "UPDATE shop.mirror_state SET last_complete_pass_at=timezone('UTC',now())
         WHERE network='ethereum' AND mirror='collections'";
    let begin_ethereum_collections =
        "UPDATE shop.mirror_state SET pass_started_at=timezone('UTC',now())
         WHERE network='ethereum' AND mirror='collections'";
    // An empty completed pass cannot prove a collection arriving later.
    sqlx::query("DELETE FROM shop.collections WHERE network='ethereum' AND contract_address=$1")
        .bind(CARL)
        .execute(&db.pool)
        .await
        .unwrap();
    let (code, incomplete) = get_json(&app, &format!("/v1/sales/summary?seller={CARL}")).await;
    assert_eq!(code, StatusCode::OK, "{incomplete}");
    assert_eq!(incomplete["data"]["royalties"]["royaltiesWei"], Value::Null);
    assert!(incomplete["data"]["earnedUsd"].is_string());
    sqlx::query(
        "UPDATE shop.mirror_state SET pass_started_at=timezone('UTC',now())-interval '2 minutes',
         last_complete_pass_at=timezone('UTC',now())-interval '1 minute'
         WHERE network='ethereum' AND mirror='collections'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(begin_ethereum_collections)
        .execute(&db.pool)
        .await
        .unwrap();
    seed_collection(
        &db.pool,
        "ethereum",
        CARL,
        block,
        "v1-collection",
        "Vintage",
    )
    .await;
    sqlx::query(
        "UPDATE shop.collections SET creator=$1 WHERE network='ethereum' AND contract_address=$2",
    )
    .bind(BOB)
    .bind(CARL)
    .execute(&db.pool)
    .await
    .unwrap();
    let (code, incomplete) = get_json(&app, &format!("/v1/sales/summary?seller={CARL}")).await;
    assert_eq!(code, StatusCode::OK, "{incomplete}");
    assert_eq!(incomplete["data"]["royalties"]["royaltiesWei"], Value::Null);
    // An older writer starts a replacement before the object pass installs
    // the attribution tables. Installing them cannot certify the abandoned row.
    sqlx::raw_sql(
        "DROP TRIGGER publish_v1_royalties_on_completion ON shop.mirror_state;
         DROP TABLE shop.v1_royalty_collection, shop.v1_royalty_item;",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(begin_ethereum_collections)
        .execute(&db.pool)
        .await
        .unwrap();
    let upgrade = crate::db::schema_migrate::migrate(
        &db.pool,
        crate::db::SCHEMA_SQL,
        crate::db::MIGRATIONS_SQL,
    )
    .await
    .unwrap();
    assert!(upgrade.steps.is_empty(), "{upgrade:?}");
    assert!(
        upgrade
            .created
            .iter()
            .any(|object| format!("{object:?}").contains("v1_royalty_collection")),
        "{upgrade:?}"
    );
    sqlx::query(complete_ethereum_items)
        .execute(&db.pool)
        .await
        .unwrap();
    // The old writer finishes without reading this collection.
    sqlx::query(complete_ethereum_collections)
        .execute(&db.pool)
        .await
        .unwrap();
    let (code, incomplete) = get_json(&app, &format!("/v1/sales/summary?seller={CARL}")).await;
    assert_eq!(code, StatusCode::OK, "{incomplete}");
    assert_eq!(incomplete["data"]["royalties"]["royaltiesWei"], Value::Null);
    // Refreshing the row during a pass that completes proves the amount.
    sqlx::query(begin_ethereum_collections)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE shop.collections SET mirrored_at=timezone('UTC',now())
         WHERE network='ethereum' AND contract_address=$1",
    )
    .bind(CARL)
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(complete_ethereum_collections)
        .execute(&db.pool)
        .await
        .unwrap();
    let (code, complete_copy) = get_json(&app, &format!("/v1/sales/summary?seller={CARL}")).await;
    assert_eq!(code, StatusCode::OK, "{complete_copy}");
    assert_eq!(complete_copy["data"]["royalties"]["royaltiesWei"], "5");

    // Later full sweeps rewrite these rows before publishing. Both their
    // unchanged refresh and an unpublished attribution change leave the
    // previous royalty amount, resale count and volume available.
    sqlx::query(begin_ethereum_collections)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE shop.mirror_state SET pass_started_at=timezone('UTC',now())
         WHERE network='ethereum' AND mirror='items'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE shop.items SET mirrored_at=timezone('UTC',now())
         WHERE network='ethereum' AND contract_address=$1",
    )
    .bind(CARL)
    .execute(&db.pool)
    .await
    .unwrap();
    let catalogue_chain =
        "UPDATE shop.collections SET creator=$1,chain_id=$2,mirrored_at=timezone('UTC',now())
        WHERE network='ethereum' AND contract_address=$3";
    sqlx::query(catalogue_chain)
        .bind(BOB)
        .bind(1)
        .bind(CARL)
        .execute(&db.pool)
        .await
        .unwrap();
    let (code, refreshing) = get_json(&app, &format!("/v1/sales/summary?seller={CARL}")).await;
    assert_eq!(code, StatusCode::OK, "{refreshing}");
    assert_eq!(
        refreshing["data"]["royalties"],
        complete_copy["data"]["royalties"]
    );
    sqlx::query(
        "UPDATE shop.items SET creator=$1,mirrored_at=timezone('UTC',now())
         WHERE network='ethereum' AND contract_address=$2",
    )
    .bind(DAVE)
    .bind(CARL)
    .execute(&db.pool)
    .await
    .unwrap();
    let (code, refreshing) = get_json(&app, &format!("/v1/sales/summary?seller={CARL}")).await;
    assert_eq!(code, StatusCode::OK, "{refreshing}");
    assert_eq!(
        refreshing["data"]["royalties"],
        complete_copy["data"]["royalties"]
    );
    sqlx::query(
        "UPDATE shop.items SET creator=$1,mirrored_at=timezone('UTC',now())
         WHERE network='ethereum' AND contract_address=$1",
    )
    .bind(CARL)
    .execute(&db.pool)
    .await
    .unwrap();

    // Publishing a replacement pass still cannot prove a row on another chain.
    sqlx::query(catalogue_chain)
        .bind(BOB)
        .bind(11155111)
        .bind(CARL)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query(complete_ethereum_collections)
        .execute(&db.pool)
        .await
        .unwrap();
    let (code, incomplete) = get_json(&app, &format!("/v1/sales/summary?seller={CARL}")).await;
    assert_eq!(code, StatusCode::OK, "{incomplete}");
    assert_eq!(incomplete["data"]["royalties"]["royaltiesWei"], Value::Null);
    assert!(incomplete["data"]["earnedUsd"].is_string());
    sqlx::query(begin_ethereum_collections)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query(catalogue_chain)
        .bind(BOB)
        .bind(1)
        .bind(CARL)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query(complete_ethereum_collections)
        .execute(&db.pool)
        .await
        .unwrap();
    // With the catalogue published, a missing Issue still leaves attribution
    // unresolved for an item creator who does not own the collection.
    let move_issue = "UPDATE shop.nft_event SET block_number=$1
        WHERE network='ethereum' AND kind='issue_v1' AND contract_address=$2";
    sqlx::query(move_issue)
        .bind(v1_block + 1)
        .bind(CARL)
        .execute(&db.pool)
        .await
        .unwrap();
    let (code, incomplete) = get_json(&app, &format!("/v1/sales/summary?seller={CARL}")).await;
    assert_eq!(code, StatusCode::OK, "{incomplete}");
    assert_eq!(incomplete["data"]["royalties"]["royaltiesWei"], Value::Null);
    assert!(incomplete["data"]["earnedUsd"].is_string());
    sqlx::query(move_issue)
        .bind(v1_block)
        .bind(CARL)
        .execute(&db.pool)
        .await
        .unwrap();
    let (code, creator_summary) = get_json(&app, &format!("/v1/sales/summary?seller={CARL}")).await;
    assert_eq!(code, StatusCode::OK, "{creator_summary}");
    assert_eq!(creator_summary["data"]["royalties"]["royaltiesWei"], "5");
    assert_eq!(creator_summary["data"]["royalties"]["resales"], 1);
    for mirror in ["collections", "items"] {
        sqlx::query("DELETE FROM shop.mirror_state WHERE network='ethereum' AND mirror=$1")
            .bind(mirror)
            .execute(&db.pool)
            .await
            .unwrap();
        let proof_rows: i64 = sqlx::query_scalar(
            "SELECT CASE $1 WHEN 'items' THEN (SELECT count(*) FROM shop.v1_royalty_item)
             ELSE (SELECT count(*) FROM shop.v1_royalty_collection) END",
        )
        .bind(mirror)
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert_eq!(proof_rows, 0, "{mirror}");
        let (code, withdrawn) = get_json(&app, &format!("/v1/sales/summary?seller={CARL}")).await;
        if mirror == "items" {
            // Items readiness also gates the existing sales summary.
            assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{withdrawn}");
        } else {
            assert_eq!(code, StatusCode::OK, "{withdrawn}");
            assert_eq!(withdrawn["data"]["royalties"]["royaltiesWei"], Value::Null);
        }
        sqlx::query(
            "INSERT INTO shop.mirror_state(mirror,network,source_url,chain_id,pass_started_at,last_complete_pass_at)
             SELECT $1,'ethereum','https://copy.example',1,
               CASE $1 WHEN 'items' THEN (SELECT min(mirrored_at) FROM shop.items WHERE network='ethereum')
               ELSE (SELECT min(mirrored_at) FROM shop.collections WHERE network='ethereum') END,
               timezone('UTC',now())",
        )
        .bind(mirror)
        .execute(&db.pool)
        .await
        .unwrap();
        let (code, restored) = get_json(&app, &format!("/v1/sales/summary?seller={CARL}")).await;
        assert_eq!(code, StatusCode::OK, "{restored}");
        assert_eq!(restored["data"]["royalties"]["royaltiesWei"], "5");
    }
    let set_seller = "UPDATE shop.sale_asset SET seller=$2 WHERE
        network='ethereum' AND block_number=$1 AND log_index=1";
    sqlx::query(set_seller)
        .bind(v1_block)
        .bind(CARL)
        .execute(&db.pool)
        .await
        .unwrap();
    let (code, self_sale) = get_json(&app, &format!("/v1/sales/summary?seller={CARL}")).await;
    assert_eq!(code, StatusCode::OK, "{self_sale}");
    assert_eq!(self_sale["data"]["royalties"]["resales"], 1);
    assert_eq!(self_sale["data"]["royalties"]["royaltiesWei"], "0");
    sqlx::query(set_seller)
        .bind(v1_block)
        .bind(ALICE)
        .execute(&db.pool)
        .await
        .unwrap();
    seed_item(
        &db.pool,
        "ethereum",
        CARL,
        10,
        "v1-collection:other",
        "Other wearable",
        "wearable_v1",
        "hat",
    )
    .await;
    sqlx::query("UPDATE shop.items SET creator=$1 WHERE network='ethereum' AND contract_address=$2 AND item_id=10")
        .bind(DAVE).bind(CARL).execute(&db.pool).await.unwrap();
    sqlx::query(complete_ethereum_items)
        .execute(&db.pool)
        .await
        .unwrap();
    let set_issue = "UPDATE shop.nft_event SET detail=$1 WHERE
        network='ethereum' AND block_number=$2 AND kind='issue_v1' AND contract_address=$3";
    sqlx::query(set_issue)
        .bind("other")
        .bind(v1_block)
        .bind(CARL)
        .execute(&db.pool)
        .await
        .unwrap();
    let (code, other_creator) = get_json(&app, &format!("/v1/sales/summary?seller={CARL}")).await;
    assert_eq!(code, StatusCode::OK, "{other_creator}");
    assert_eq!(other_creator["data"]["royalties"]["royaltiesWei"], "0");
    assert_eq!(other_creator["data"]["royalties"]["resales"], 0);
    sqlx::query(set_issue)
        .bind("wearable")
        .bind(v1_block)
        .bind(CARL)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE shop.items SET chain_id=11155111 WHERE network='ethereum' AND contract_address=$1 AND item_id=9")
        .bind(CARL).execute(&db.pool).await.unwrap();
    sqlx::query(complete_ethereum_items)
        .execute(&db.pool)
        .await
        .unwrap();
    seed_item(
        &db.pool,
        "matic",
        DAVE,
        58,
        "foreign-item",
        "Foreign",
        "wearable_v2",
        "hat",
    )
    .await;
    sqlx::query("UPDATE shop.items SET chain_id=80002 WHERE network='matic' AND contract_address=$1 AND item_id=58")
        .bind(DAVE).execute(&db.pool).await.unwrap();
    sqlx::query("WITH e AS (INSERT INTO shop.sale_execution(network,block_number,log_index,tx_hash,venue,kind,block_time,receipt_read)
        VALUES('matic',$1,21,$2,$3,'order',$4,true) RETURNING network,block_number,log_index),
        a AS (INSERT INTO shop.sale_asset(network,block_number,log_index,ordinal,public_id,beneficiary,kind,contract_address,token_id,item_id,seller,buyer,price,currency)
        SELECT e.*,0,'foreign-item-resale',$5,'order',$3,1,58,$6,$5,100,$7 FROM e),
        p AS (INSERT INTO shop.sale_payment(network,block_number,log_index,ordinal,currency,gross,settled)
        SELECT e.*,0,$7,100,true FROM e RETURNING network,block_number,log_index)
        INSERT INTO shop.sale_payout(network,block_number,log_index,payment_ordinal,ordinal,role,recipient,amount)
        SELECT p.*,0,0,'royalty',$8,5 FROM p")
        .bind(polygon as i64).bind(format!("0x{:064x}",95)).bind(DAVE).bind(day).bind(BOB).bind(ALICE).bind(mana).bind(CARL)
        .execute(&db.pool).await.unwrap();
    let (code, foreign) = get_json(&app, &format!("/v1/sales/summary?seller={CARL}")).await;
    assert_eq!(code, StatusCode::OK, "{foreign}");
    assert_eq!(foreign["data"]["royalties"]["royaltiesWei"], "0");
    assert_eq!(foreign["data"]["royalties"]["resales"], 0);
    sqlx::query("UPDATE shop.items SET chain_id=1 WHERE network='ethereum' AND contract_address=$1 AND item_id=9")
        .bind(CARL).execute(&db.pool).await.unwrap();
    sqlx::query(complete_ethereum_items)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE shop.sale_payment SET settled=false WHERE network='ethereum' AND block_number=$1 AND log_index=1",
    )
    .bind(v1_block)
    .execute(&db.pool)
    .await
    .unwrap();
    let (status, pending) = get_json(&app, &format!("/v1/sales/summary?seller={CARL}")).await;
    assert_eq!(status, StatusCode::OK, "{pending}");
    assert_eq!(pending["data"]["royalties"]["royaltiesWei"], Value::Null);
    assert_eq!(pending["data"]["royalties"]["resales"], 1);
    let (status, store) = get_json(&app, &format!("/v1/stores/{CARL}/stats")).await;
    assert_eq!(status, StatusCode::OK, "{store}");
    assert_eq!(
        store["data"]["summary"]["royalties"]["royaltiesWei"],
        Value::Null
    );
    assert!(store["data"]["summary"]["earnedUsd"].is_string());
    sqlx::query(
        "DELETE FROM shop.sale_payout WHERE network='ethereum' AND block_number=$1 AND log_index=1 AND role='royalty'",
    )
    .bind(v1_block)
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE shop.sale_payment SET settled=true WHERE network='ethereum' AND block_number=$1 AND log_index=1",
    )
    .bind(v1_block)
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query("UPDATE shop.sale_payout SET amount=amount+5 WHERE network='ethereum' AND block_number=$1 AND log_index=1 AND role='seller'")
        .bind(v1_block).execute(&db.pool).await.unwrap();
    let (code, zero) = get_json(&app, &format!("/v1/sales/summary?seller={CARL}")).await;
    assert_eq!(code, StatusCode::OK, "{zero}");
    assert_eq!(zero["data"]["royalties"]["royaltiesWei"], "0");
    assert_eq!(zero["data"]["royalties"]["resales"], 1);
    // A missing payment row cannot turn a sale with unknown proceeds into
    // an empty page or a smaller seller total.
    sqlx::query("DELETE FROM shop.sale_payment WHERE network='ethereum' AND log_index=1")
        .execute(&db.pool)
        .await
        .unwrap();
    for path in [
        proceeds_path.clone(),
        format!("/v1/proceeds/summary?seller={ALICE}&network=ETHEREUM"),
    ] {
        let (code, body) = get_json(&app, &path).await;
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{path}: {body}");
        assert_eq!(body["source"], "sales-index");
    }
    sqlx::query(
        "UPDATE shop.chain_cursor SET folded_at=NULL WHERE network='ethereum' AND indexer='sales'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        get_json(&app, "/v1/volume/all").await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    let (status, body) = get_json(&app, path).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    let (activity, _) = crate::market::activity::user_activity(&state, ALICE, None, None).await;
    assert!(
        activity
            .iter()
            .all(|event| event.event_type != "sale_seller")
    );
    db.drop().await;
}

#[tokio::test]
async fn deep_sales_pages_keep_equal_timestamps_in_id_order_across_networks() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let mut settings = index_settings(100, 20);
    settings.sales_enabled = true;
    let mut expected = Vec::new();
    for net in ChainNet::ALL {
        let height = nft_index::deployments::Deployments::mainnet(net).start + 20;
        let chain = FakeChain::new(height + net.confirmations(), vec![], 1000, 100);
        chain
            .chain_id
            .store(net.chain_id() as u64, Ordering::SeqCst);
        let url = serve(chain.clone()).await;
        let st = state_over(db.pool.clone(), &url, settings.clone(), false);
        sqlx::query("INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,last_block_hash,
            full_history,folded_block,folded_at,sale_input_version,sale_verified_through,sale_covered_rewinds)
            SELECT ix,$1,$2,1,$3,$4,true,$3,timezone('UTC',now()),$5,$3,0
            FROM unnest(ARRAY['nft','sales','trade_events']) ix")
            .bind(net.db_name()).bind(net.chain_id()).bind(height as i64).bind(chain.hash_of(height)).bind(sale_history::VERSION).execute(&db.pool).await.unwrap();
        sale_fold::pass(&db.pool, &Rpc::at(&st.http, &url), net)
            .await
            .unwrap();
        let count: i32 = if net == ChainNet::Ethereum {
            25_001
        } else {
            7_300
        };
        sqlx::query("WITH executions AS (INSERT INTO shop.sale_execution(network,block_number,log_index,tx_hash,venue,kind,block_time)
            SELECT $1,$2,i,'0x'||lpad(to_hex(i),64,'0'),$3,'order',100000-i/3 FROM generate_series(0,$7::int-1) i
            RETURNING network,block_number,log_index,tx_hash)
            INSERT INTO shop.sale_asset(network,block_number,log_index,ordinal,public_id,beneficiary,kind,contract_address,token_id,seller,buyer,price,currency)
            SELECT network,block_number,log_index,0,network||'-'||tx_hash||'-0-0',$4,'order',$3,log_index,$5,$4,1,$6 FROM executions")
            .bind(net.db_name()).bind(height as i64).bind(CARL).bind(ALICE).bind(BOB)
            .bind(crate::market::listings::mana_token(net.chain_id())).bind(count).execute(&db.pool).await.unwrap();
        sqlx::query("INSERT INTO shop.sale_analytics_day(network,day,currency,sales,volume,name_fees,creator_earnings,venue_fees,store_fees,burn_fees,unknown_splits,unallocated_executions)
            SELECT e.network,e.block_time/86400*86400,a.currency,count(*),sum(price),0,0,0,0,0,0,0
            FROM shop.sale_execution e JOIN shop.sale_asset a USING(network,block_number,log_index) WHERE e.network=$1 GROUP BY 1,2,3")
            .bind(net.db_name()).execute(&db.pool).await.unwrap();
        expected.extend((0..count).map(|i| (i / 3, format!("{}-0x{i:064x}-0-0", net.db_name()))));
    }
    sqlx::query("ANALYZE shop.sale_execution")
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("ANALYZE shop.sale_asset")
        .execute(&db.pool)
        .await
        .unwrap();
    let state = state_over(db.pool.clone(), "http://127.0.0.1", settings, false);
    let router = market_router(state.clone());
    expected.sort();
    for (nets, first, skip) in [
        ("MATIC", 5, 5998),
        ("MATIC", 5, 7000),
        ("MATIC", 100, 7250),
        ("", 5, 997),
        ("", 100, 7000),
    ] {
        let mut path = format!("/v1/sales?first={first}&skip={skip}");
        if !nets.is_empty() {
            path.push_str("&network=");
            path.push_str(nets);
        }
        let (status, body) = get_json(&router, &path).await;
        assert_eq!(status, StatusCode::OK, "{path} {body}");
        let selected: Vec<_> = expected
            .iter()
            .map(|(_, id)| id.as_str())
            .filter(|id| nets.is_empty() || id.starts_with("matic-"))
            .collect();
        assert_eq!(body["total"], selected.len());
        let ids: Vec<_> = body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["id"].as_str().unwrap())
            .collect();
        assert_eq!(
            ids,
            selected
                .into_iter()
                .skip(skip)
                .take(first)
                .collect::<Vec<_>>(),
            "{path}"
        );
    }
    assert_eq!(
        get_json(&router, "/v1/sales?first=10&skip=24995").await.0,
        StatusCode::BAD_REQUEST
    );
    for (skip, total) in [(0, 5_000), (5_500, 25_000)] {
        let (code, page) = get_json(
            &router,
            &format!("/v1/sales?network=ETHEREUM&type=order&first=5&skip={skip}"),
        )
        .await;
        assert_eq!(code, StatusCode::OK, "{page}");
        assert_eq!(page["total"], total);
        assert_eq!(page["data"].as_array().unwrap().len(), 5);
    }
    assert_eq!(
        get_json(&router, "/v1/sales?tokenId=1e2").await.0,
        StatusCode::BAD_REQUEST
    );
    // Two dashboards with two windows each can acquire permits together.
    // Holding all four demonstrates what the endpoints do after the queue timeout.
    let (one, two, three, four) = tokio::try_join!(
        crate::market::sale_reads::seller_slot(&state),
        crate::market::sale_reads::seller_slot(&state),
        crate::market::sale_reads::seller_slot(&state),
        crate::market::sale_reads::seller_slot(&state),
    )
    .unwrap();
    let waiting = crate::market::sale_reads::seller_slot(&state);
    tokio::pin!(waiting);
    tokio::select! {
        _ = &mut waiting => panic!("the request should wait for a seller permit"),
        _ = tokio::time::sleep(std::time::Duration::from_millis(20)) => {}
    }
    drop(one);
    let one = waiting.await.unwrap();
    for path in [
        format!("/v1/sales/summary?seller={BOB}"),
        format!("/v1/proceeds?seller={BOB}"),
        format!("/v1/proceeds/summary?seller={BOB}"),
        format!("/v1/stores/{BOB}/stats"),
    ] {
        let (code, busy) = get_json(&router, &path).await;
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{path} {busy}");
        assert_eq!(busy["message"], "seller sales queries are busy");
    }
    let (bought, sold) = crate::market::sale_reads::activity_sales(&state, BOB, 100).await;
    assert!(bought.is_empty() && sold.is_empty());
    drop((one, two, three, four));
    // Aggregates cover every published sale, including a seller beyond the
    // list's 25,000-row paging window on a single network.
    sqlx::query(
        "INSERT INTO shop.mirror_state(mirror,network,source_url,last_complete_pass_at)
        SELECT 'items',net,'test',timezone('UTC',now()) FROM unnest(ARRAY['ethereum','matic']) net",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query("WITH payments AS (
        INSERT INTO shop.sale_payment(network,block_number,log_index,ordinal,currency,gross,settled)
        SELECT network,block_number,log_index,ordinal,currency,price,true FROM shop.sale_asset
        RETURNING network,block_number,log_index,ordinal,gross)
        INSERT INTO shop.sale_payout(network,block_number,log_index,payment_ordinal,ordinal,role,recipient,amount)
        SELECT network,block_number,log_index,ordinal,0,'seller',$1,gross FROM payments")
        .bind(BOB).execute(&db.pool).await.unwrap();
    // Bulk seeding above runs before autovacuum can collect normal planner statistics.
    for analyze in ["ANALYZE shop.sale_payment", "ANALYZE shop.sale_payout"] {
        sqlx::query(analyze).execute(&db.pool).await.unwrap();
    }
    for (skip, total) in [(0, 5_000), (5_500, 25_000)] {
        let (code, page) = get_json(
            &router,
            &format!("/v1/proceeds?seller={BOB}&network=ETHEREUM&first=5&skip={skip}"),
        )
        .await;
        assert_eq!(code, StatusCode::OK, "{page}");
        assert_eq!(page["total"], total);
        assert_eq!(page["data"].as_array().unwrap().len(), 5);
    }
    for (route, count_field, wei_field) in [
        ("/v1/sales/summary", "total", "earnedWei"),
        ("/v1/proceeds/summary", "sales", "netWei"),
    ] {
        let (code, page) = get_json(&router, &format!("{route}?seller={BOB}")).await;
        assert_eq!(code, StatusCode::OK, "{route}: {page}");
        assert_eq!(page["data"][count_field], expected.len());
        assert_eq!(page["data"][wei_field], expected.len().to_string());
    }
    db.drop().await;
}

#[tokio::test]
async fn volume_repairs_unallocated_payments_and_counts_only_priced_store_lines() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let mut settings = index_settings(100, 20);
    settings.sales_enabled = true;
    let mut polygon_url = String::new();
    for net in ChainNet::ALL {
        let height = nft_index::deployments::Deployments::mainnet(net).start + 20;
        let chain = FakeChain::new(height + net.confirmations(), vec![], 1000, 100);
        chain
            .chain_id
            .store(net.chain_id() as u64, Ordering::SeqCst);
        let url = serve(chain.clone()).await;
        let st = state_over(db.pool.clone(), &url, settings.clone(), false);
        sqlx::query("INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,last_block_hash,
            full_history,folded_block,folded_at,sale_input_version,sale_verified_through,sale_covered_rewinds)
            SELECT ix,$1,$2,1,$3,$4,true,$3,timezone('UTC',now()),$5,$3,0 FROM unnest(ARRAY['nft','sales','trade_events']) ix")
            .bind(net.db_name()).bind(net.chain_id()).bind(height as i64).bind(chain.hash_of(height)).bind(sale_history::VERSION)
            .execute(&db.pool).await.unwrap();
        sale_fold::pass(&db.pool, &Rpc::at(&st.http, &url), net)
            .await
            .unwrap();
        if net == ChainNet::Matic {
            polygon_url = url;
        }
    }
    let net = ChainNet::Matic;
    let block = nft_index::deployments::Deployments::mainnet(net).start as i64 + 20;
    let mana = crate::market::listings::mana_token(net.chain_id()).unwrap();
    let st = state_over(db.pool.clone(), &polygon_url, settings, false);
    let rpc = Rpc::at(&st.http, &polygon_url);
    let app = market_router(st.clone());
    // Version 1 counted the consideration and royalties of this unpriced bid.
    sqlx::query("WITH e AS (INSERT INTO shop.sale_execution(network,block_number,log_index,tx_hash,venue,kind,block_time,receipt_read)
        VALUES('matic',$1,1,$2,$3,'bid',86400,true) RETURNING network,block_number,log_index),
        p AS (INSERT INTO shop.sale_payment(network,block_number,log_index,ordinal,currency,gross,consideration,settled)
        SELECT network,block_number,log_index,0,$4,240,true,true FROM e RETURNING network,block_number,log_index)
        INSERT INTO shop.sale_payout(network,block_number,log_index,payment_ordinal,ordinal,role,recipient,amount)
        SELECT network,block_number,log_index,0,0,'royalty',$3,240 FROM p")
        .bind(block).bind(format!("0x{:064x}",81)).bind(CARL).bind(mana).execute(&db.pool).await.unwrap();
    // A bundle price has no per-NFT allocation. Even a partial legacy price
    // on one of its two asset rows cannot establish an independent sale.
    sqlx::query("WITH e AS (INSERT INTO shop.sale_execution(network,block_number,log_index,tx_hash,venue,kind,block_time,receipt_read)
        VALUES('matic',$1,3,$2,$3,'trade',86400,true) RETURNING network,block_number,log_index),
        a AS (INSERT INTO shop.sale_asset(network,block_number,log_index,ordinal,public_id,beneficiary,kind,contract_address,token_id,seller,buyer,price,currency)
        SELECT e.*,v.n,'bundled-'||v.n,$4,'order',$3,v.n,$3,$4,v.amount,CASE WHEN v.amount IS NOT NULL THEN $5 END
        FROM e CROSS JOIN (VALUES (0,120),(1,NULL::int)) v(n,amount))
        INSERT INTO shop.sale_payment(network,block_number,log_index,ordinal,currency,gross,consideration,settled)
        SELECT e.*,0,$5,120,true,false FROM e")
        .bind(block).bind(format!("0x{:064x}",83)).bind(CARL).bind(BOB).bind(mana).execute(&db.pool).await.unwrap();
    sqlx::query(
        "INSERT INTO shop.sale_analytics_day(network,day,currency,sales,volume,name_fees,creator_earnings,
        venue_fees,store_fees,burn_fees,unknown_splits,unallocated_executions)
        VALUES('matic',86400,$1,1,360,0,240,0,0,0,1,0)",
    )
    .bind(mana)
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query("UPDATE shop.sale_publication SET input_version=2 WHERE network='matic'")
        .execute(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        get_json(&app, "/v1/volume/all").await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    sale_fold::pass(&db.pool, &rpc, net).await.unwrap();
    let (code, volume) = get_json(&app, "/v1/volume/all").await;
    assert_eq!(code, StatusCode::OK, "{volume}");
    assert_eq!(
        volume["data"],
        json!({"sales":0,"volume":"0","creatorsEarnings":"0","daoEarnings":"0"})
    );

    // One receipt includes a priced store line and a second payment whose NFT
    // has no proven Issue. Neither the second gross nor its fee is allocated.
    sqlx::query("INSERT INTO shop.sale_execution(network,block_number,log_index,tx_hash,venue,kind,block_time,receipt_read)
        VALUES('matic',$1,2,$2,$3,'store',86400,true)")
        .bind(block).bind(format!("0x{:064x}",82)).bind(CARL).execute(&db.pool).await.unwrap();
    sqlx::query("INSERT INTO shop.sale_asset(network,block_number,log_index,ordinal,public_id,beneficiary,kind,contract_address,token_id,item_id,seller,buyer,price,currency)
        VALUES('matic',$1,2,1,'priced-store-line',$2,'mint',$3,12,1,$3,$2,60,$4)")
        .bind(block).bind(BOB).bind(CARL).bind(mana).execute(&db.pool).await.unwrap();
    sqlx::query("INSERT INTO shop.sale_payment(network,block_number,log_index,ordinal,currency,gross,consideration,settled)
        VALUES('matic',$1,2,0,$2,250,true,true),('matic',$1,2,1,$2,60,true,true)").bind(block).bind(mana).execute(&db.pool).await.unwrap();
    sqlx::query("INSERT INTO shop.sale_payout(network,block_number,log_index,payment_ordinal,ordinal,role,recipient,amount)
        VALUES('matic',$1,2,0,0,'store_fee',$2,250),('matic',$1,2,1,0,'seller',$2,45),
              ('matic',$1,2,1,1,'royalty',$3,5),('matic',$1,2,1,2,'store_fee',$2,10)")
        .bind(block).bind(CARL).bind(DAVE).execute(&db.pool).await.unwrap();
    sqlx::query("UPDATE shop.sale_publication SET input_version=2 WHERE network='matic'")
        .execute(&db.pool)
        .await
        .unwrap();
    sale_fold::pass(&db.pool, &rpc, net).await.unwrap();
    let (code, volume) = get_json(&app, "/v1/volume/all").await;
    assert_eq!(code, StatusCode::OK, "{volume}");
    assert_eq!(
        volume["data"],
        json!({"sales":1,"volume":"60","creatorsEarnings":"50","daoEarnings":"10"})
    );
    let (code, sales) = get_json(&app, "/v1/sales?network=MATIC").await;
    assert_eq!(code, StatusCode::OK, "{sales}");
    assert_eq!(sales["total"], 1);
    assert_eq!(sales["data"][0]["price"], "60");
    db.drop().await;
}

/// `shop.nft_last_sale` follows the published ledger through the real fold:
/// a token's latest public sale, admitted to reads once the table reaches
/// the publication. A history change that reverts the window holding a
/// token's latest sale falls back to its earlier sale and drops a token
/// sold only there, never keeping the reverted one. A publication from
/// before the table (no `last_sale_through`) is refilled by the next pass.
/// A writer from before the table that replaces or corrects the published
/// history rotates the generation and withdraws the table until a current
/// writer rebuilds it.
#[tokio::test]
async fn the_latest_sale_follows_the_published_ledger_across_a_rewind() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let net = ChainNet::Ethereum;
    let block = nft_index::deployments::Deployments::mainnet(net).start + 75;
    let chain = FakeChain::new(block + 14, vec![], 1000, 10);
    chain.chain_id.store(1, Ordering::SeqCst);
    let url = serve(chain.clone()).await;
    let mut settings = index_settings(100, 10);
    settings.sales_enabled = true;
    let state = state_over(db.pool.clone(), &url, settings, false);
    let rpc = Rpc::at(&state.http, &url);
    sqlx::query(
        "INSERT INTO shop.chain_cursor
            (indexer,network,chain_id,start_block,last_block,last_block_hash,full_history,
             folded_block,folded_at,sale_input_version,sale_verified_through,sale_covered_rewinds)
         SELECT indexer,'ethereum',1,1,$1,$2,true,$1,timezone('UTC',now()),$3,$1,0
         FROM unnest(ARRAY['nft','sales','trade_events']) AS indexer",
    )
    .bind(block as i64)
    .bind(chain.hash_of(block))
    .bind(sale_history::VERSION)
    .execute(&db.pool)
    .await
    .unwrap();
    // Parcel 1 sells at `block - 10` and again at `block`; parcel 2 only at `block`.
    let market = nft_index::deployments::Deployments::mainnet(net).markets[0].0;
    for (at, log, token, time, tx) in [
        (block as i64 - 10, 1, 1, 1_700_000_000i64, 81),
        (block as i64, 1, 1, 1_700_000_500, 82),
        (block as i64, 2, 2, 1_700_000_500, 83),
    ] {
        sqlx::query(
            "INSERT INTO shop.nft_event
                (network,block_number,log_index,block_time,kind,emitter,contract_address,
                 token_id,from_address,to_address,amount,tx_hash)
             VALUES ('ethereum',$1,$2,$3,'order_successful',$4,$5,$6,$7,$8,1000,$9)",
        )
        .bind(at)
        .bind(log)
        .bind(time)
        .bind(market)
        .bind(LAND_REGISTRY_MAINNET)
        .bind(token as i64)
        .bind(ALICE)
        .bind(BOB)
        .bind(format!("0x{tx:064x}"))
        .execute(&db.pool)
        .await
        .unwrap();
    }
    let rows = || async {
        sqlx::query_as::<_, (String, i64, i64)>(
            "SELECT token_id::text, sold_at, block_number FROM shop.nft_last_sale
             WHERE network = 'ethereum' ORDER BY token_id",
        )
        .fetch_all(&db.pool)
        .await
        .unwrap()
    };
    let admitted = || async {
        let mut conn = db.pool.acquire().await.unwrap();
        sale_fold::last_sale_networks(&mut conn, &ChainNet::ALL, chrono::Utc::now())
            .await
            .unwrap()
    };
    sale_fold::pass(&db.pool, &rpc, net).await.unwrap();
    let b = block as i64;
    assert_eq!(
        rows().await,
        vec![
            ("1".into(), 1_700_000_500, b),
            ("2".into(), 1_700_000_500, b)
        ]
    );
    assert_eq!(admitted().await, vec![ChainNet::Ethereum]);

    // The walk drops the window holding both latest sales and reports it.
    sqlx::query("DELETE FROM shop.nft_event WHERE network = 'ethereum' AND block_number = $1")
        .bind(b)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE shop.chain_cursor SET sale_history_revision = sale_history_revision + 1,
         sale_dirty_from = $1 WHERE network = 'ethereum' AND indexer = 'nft'",
    )
    .bind(b)
    .execute(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        admitted().await,
        Vec::<ChainNet>::new(),
        "a changed input withdraws it"
    );
    sale_fold::pass(&db.pool, &rpc, net).await.unwrap();
    assert_eq!(rows().await, vec![("1".into(), 1_700_000_000, b - 10)]);
    assert_eq!(admitted().await, vec![ChainNet::Ethereum]);

    // A publication from before the table: the next pass refills it.
    sqlx::query(
        "UPDATE shop.sale_publication SET last_sale_through = NULL WHERE network = 'ethereum'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query("DELETE FROM shop.nft_last_sale")
        .execute(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        admitted().await,
        Vec::<ChainNet>::new(),
        "an unfilled table is not read"
    );
    sale_fold::pass(&db.pool, &rpc, net).await.unwrap();
    assert_eq!(rows().await, vec![("1".into(), 1_700_000_000, b - 10)]);
    let through: Option<i64> = sqlx::query_scalar(
        "SELECT last_sale_through FROM shop.sale_publication WHERE network = 'ethereum'",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(through, Some(b));
    assert_eq!(admitted().await, vec![ChainNet::Ethereum]);

    // A writer from before step 0078 replaces the history: parcel 1's only
    // sale leaves the ledger and it republishes the new input revision,
    // keeping the table's columns as they were. The table is not read, and
    // the next pass of this writer rebuilds it from the ledger.
    sqlx::query("DELETE FROM shop.nft_event WHERE network = 'ethereum' AND block_number = $1")
        .bind(b - 10)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM shop.sale_execution WHERE network = 'ethereum' AND block_number > $1")
        .bind(b - 11)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE shop.chain_cursor SET sale_history_revision = sale_history_revision + 1
         WHERE network = 'ethereum' AND indexer = 'nft'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE shop.sale_publication p SET published_at = timezone('UTC', now()), generation = gen_random_uuid(),
             input_revisions = (SELECT array_agg(c.sale_history_revision ORDER BY c.indexer) FROM shop.chain_cursor c
                                WHERE c.network = p.network AND c.indexer IN ('nft', 'sales', 'trade_events'))
         WHERE network = 'ethereum'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let valid = sale_fold::snapshot(&db.pool, &[net], chrono::Utc::now())
        .await
        .unwrap()
        .is_some();
    assert!(valid, "the old writer's publication is valid");
    assert_eq!(rows().await, vec![("1".into(), 1_700_000_000, b - 10)]);
    assert_eq!(
        admitted().await,
        Vec::<ChainNet>::new(),
        "a table built from another history is not read"
    );
    sale_fold::pass(&db.pool, &rpc, net).await.unwrap();
    assert_eq!(rows().await, vec![]);
    assert_eq!(admitted().await, vec![ChainNet::Ethereum]);

    // Clearing the coverage alone removes a row the ledger does not hold.
    sqlx::query(
        "INSERT INTO shop.nft_last_sale (network, contract_address, token_id, sold_at, block_number)
         VALUES ('ethereum', $1, 99, 1, 1)",
    )
    .bind(LAND_REGISTRY_MAINNET)
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE shop.sale_publication SET last_sale_through = NULL WHERE network = 'ethereum'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    sale_fold::pass(&db.pool, &rpc, net).await.unwrap();
    assert_eq!(rows().await, vec![]);
    assert_eq!(admitted().await, vec![ChainNet::Ethereum]);

    // A writer from before step 0078 corrects a published sale in the
    // covered prefix (a retried receipt names parcel 7's sale at `b - 5`) and
    // rotates the generation, at the same height and history. The table is
    // not read until this writer's next pass takes the sale.
    sqlx::query(
        "INSERT INTO shop.sale_execution (network, block_number, log_index, tx_hash, venue, kind, block_time)
         VALUES ('ethereum', $1, 3, $2, $3, 'order', 1700000200)",
    )
    .bind(b - 5)
    .bind(format!("0x{:064x}", 85))
    .bind(market)
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO shop.sale_asset (network, block_number, log_index, ordinal, public_id, beneficiary, kind,
             contract_address, token_id, seller, buyer, price, currency)
         VALUES ('ethereum', $1, 3, 0, 'repaired-7', $2, 'order', $3, 7, $4, $2, 1000, $5)",
    )
    .bind(b - 5)
    .bind(BOB)
    .bind(LAND_REGISTRY_MAINNET)
    .bind(ALICE)
    .bind(crate::market::listings::mana_token(1))
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE shop.sale_publication SET generation = gen_random_uuid(), published_at = timezone('UTC', now())
         WHERE network = 'ethereum'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        admitted().await,
        Vec::<ChainNet>::new(),
        "a correction without the table withdraws it"
    );
    sale_fold::pass(&db.pool, &rpc, net).await.unwrap();
    assert_eq!(rows().await, vec![("7".into(), 1_700_000_200, b - 5)]);
    assert_eq!(admitted().await, vec![ChainNet::Ethereum]);
    db.drop().await;
}

#[tokio::test]
async fn creator_rankings_attribute_delegated_mints_at_the_execution_position() {
    use crate::market::collections_index::{CollectionsIndex, Seed, reads_token, write_seeds};

    const FIRST_COLLECTION: &str = "0x1010101010101010101010101010101010101010";
    const OTHER_COLLECTION: &str = "0x2020202020202020202020202020202020202020";
    const STORE_COLLECTION: &str = "0x3030303030303030303030303030303030303030";
    const OLD_COLLECTOR: &str = "0x4040404040404040404040404040404040404040";
    const WALK_AHEAD_COLLECTION: &str = "0x5050505050505050505050505050505050505050";
    const CORRECTED_CREATOR: &str = "0xfffffffffffffffffffffffffffffffffffffffe";
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let mut settings = index_settings(100, 20);
    settings.sales_enabled = true;
    settings.collections_enabled = true;
    let state = state_over(db.pool.clone(), "http://127.0.0.1", settings, false);
    let app = market_router(state);
    let eth = nft_index::deployments::Deployments::mainnet(ChainNet::Ethereum).start as i64 + 100;
    let polygon = nft_index::deployments::Deployments::mainnet(ChainNet::Matic).start as i64 + 100;
    for (net, height) in [(ChainNet::Ethereum, eth), (ChainNet::Matic, polygon)] {
        let hash = format!("0x{height:064x}");
        sqlx::query(
            "INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,last_block_hash,
                full_history,folded_block,folded_at,sale_input_version,sale_verified_through,sale_covered_rewinds)
             SELECT ix,$1,$2,1,$3,$4,true,$3,timezone('UTC',now()),$5,$3,0
             FROM unnest(ARRAY['nft','sales','trade_events','collections']) ix",
        )
        .bind(net.db_name())
        .bind(net.chain_id())
        .bind(height)
        .bind(&hash)
        .bind(sale_history::VERSION)
        .execute(&db.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO shop.sale_publication(network,block_number,block_hash,input_version,input_epochs,
                input_revisions,late_collections,published_at)
             SELECT $1,$2,$3,$4,array_agg(sale_input_epoch ORDER BY indexer),
                    array_agg(sale_history_revision ORDER BY indexer),0,timezone('UTC',now())
             FROM shop.chain_cursor WHERE network=$1 AND indexer IN ('nft','sales','trade_events')",
        )
        .bind(net.db_name())
        .bind(height)
        .bind(&hash)
        .bind(sale_fold::PUBLICATION_VERSION)
        .execute(&db.pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO shop.mirror_state(mirror,network,source_url,last_complete_pass_at,chain_id,pass_block_hash,marks_item_rows)
         SELECT m,n,'test',timezone('UTC',now()),CASE WHEN n='ethereum' THEN 1 ELSE 137 END,
                '0x'||repeat('a',64),m='items' FROM unnest(ARRAY['items','collections']) m
         CROSS JOIN unnest(ARRAY['ethereum','matic']) n",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let day = chrono::Utc::now().timestamp() / 86400 * 86400;
    let older = day - 10 * 86400;
    let recent = day - 3600;
    sqlx::query(
        "INSERT INTO shop.collection_event(network,block_number,log_index,block_time,kind,
            contract_address,account,previous_account)
         VALUES('matic',$1-2,1,$2,'proxy_created',$3,$3,NULL),
               ('matic',$1-2,20,$2,'creatorship_transferred',$3,$4,$5),
               ('matic',$1-18,1,$6-1,'proxy_created',$7,$7,NULL),
               ('matic',$1-17,1,$6,'proxy_created',$8,$8,NULL),
               ('matic',$1-2,40,$2,'creatorship_transferred',$8,$9,$4)",
    )
    .bind(polygon)
    .bind(recent)
    .bind(FIRST_COLLECTION)
    .bind(BOB)
    .bind(ALICE)
    .bind(older)
    .bind(OTHER_COLLECTION)
    .bind(STORE_COLLECTION)
    .bind(CARL)
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO shop.collection_seed(network,contract_address,name,symbol,creator,is_approved,
            is_editable,read_block,read_hash,verified)
         VALUES('matic',$1,'Existing','E',$2,true,false,$3,$4,true)",
    )
    .bind(OTHER_COLLECTION)
    .bind(ALICE)
    .bind(polygon - 1)
    .bind(format!("0x{:064x}", polygon - 1))
    .execute(&db.pool)
    .await
    .unwrap();

    seed_item(
        &db.pool,
        "ethereum",
        FIRST_COLLECTION,
        1,
        "ethereum-delegated-item",
        "Ethereum item",
        "wearable_v1",
        "hat",
    )
    .await;
    sqlx::query(
        "UPDATE shop.items SET creator=$1 WHERE network='ethereum' AND contract_address=$2",
    )
    .bind(ALICE)
    .bind(FIRST_COLLECTION)
    .execute(&db.pool)
    .await
    .unwrap();

    // CARL signed the mints for others. Store seller is proved at Issue,
    // earlier than the BOB-to-CARL transfer at log 40 and the Bought at 45.
    for (net, block, log, at, venue, kind, contract, seller, buyer, amount, settled) in [
        (
            ChainNet::Matic,
            polygon - 15,
            7,
            older,
            "trade",
            "mint",
            OTHER_COLLECTION,
            CARL,
            OLD_COLLECTOR,
            15,
            false,
        ),
        (
            ChainNet::Matic,
            polygon - 2,
            10,
            recent,
            "trade",
            "mint",
            FIRST_COLLECTION,
            CARL,
            DAVE,
            30,
            true,
        ),
        (
            ChainNet::Matic,
            polygon - 2,
            11,
            recent,
            "trade",
            "mint",
            FIRST_COLLECTION,
            CARL,
            DAVE,
            40,
            true,
        ),
        (
            ChainNet::Matic,
            polygon - 2,
            30,
            recent,
            "trade",
            "mint",
            FIRST_COLLECTION,
            CARL,
            DAVE,
            200,
            true,
        ),
        (
            ChainNet::Matic,
            polygon - 2,
            45,
            recent,
            "store",
            "mint",
            STORE_COLLECTION,
            BOB,
            DAVE,
            50,
            true,
        ),
        (
            ChainNet::Matic,
            polygon - 1,
            1,
            recent + 1,
            "trade",
            "mint",
            OTHER_COLLECTION,
            CARL,
            DAVE,
            50,
            true,
        ),
        (
            ChainNet::Ethereum,
            eth - 1,
            1,
            recent,
            "order",
            "order",
            LAND_REGISTRY_MAINNET,
            BOB,
            DAVE,
            100,
            true,
        ),
        (
            ChainNet::Ethereum,
            eth - 1,
            2,
            recent,
            "trade",
            "mint",
            FIRST_COLLECTION,
            CARL,
            DAVE,
            10,
            true,
        ),
    ] {
        sqlx::query(
            "WITH e AS (
                INSERT INTO shop.sale_execution(network,block_number,log_index,tx_hash,venue,kind,block_time,receipt_read)
                VALUES($1,$2,$3,$10,$4,$5,$6,true) RETURNING network,block_number,log_index
             ), asset AS (
                INSERT INTO shop.sale_asset(network,block_number,log_index,ordinal,public_id,beneficiary,kind,
                    contract_address,token_id,item_id,seller,buyer,price,currency)
                SELECT e.*,0,$1||'-'||$10,$8,$11,$7,$3,CASE WHEN $11='mint' THEN 1 END,$9,$8,$12,$13 FROM e
             ), payment AS (
                INSERT INTO shop.sale_payment(network,block_number,log_index,ordinal,currency,gross,settled)
                SELECT e.*,0,$13,$12,$14 FROM e RETURNING network,block_number,log_index,ordinal,gross,settled
             )
             INSERT INTO shop.sale_payout(network,block_number,log_index,payment_ordinal,ordinal,role,recipient,amount)
             SELECT network,block_number,log_index,ordinal,0,'seller',$9,gross FROM payment WHERE settled",
        )
        .bind(net.db_name())
        .bind(block)
        .bind(log)
        .bind(contract)
        .bind(venue)
        .bind(at)
        .bind(contract)
        .bind(buyer)
        .bind(seller)
        .bind(format!("0x{:064x}", block * 100 + i64::from(log)))
        .bind(kind)
        .bind(amount as i64)
        .bind(crate::market::listings::mana_token(net.chain_id()))
        .bind(settled)
        .execute(&db.pool)
        .await
        .unwrap();
    }

    let week = "/v1/rankings/creators/week";
    let count_path = format!("{week}?sortBy=most_sales");
    let alice = json!({"id":ALICE,"sales":4,"earned":"130","uniqueCollectors":2,"collections":3});
    let bob = json!({"id":BOB,"sales":2,"earned":"250","uniqueCollectors":1,"collections":2});
    let (code, counts) = get_json(&app, &count_path).await;
    assert_eq!(code, StatusCode::OK, "{counts}");
    assert_eq!(counts["data"], json!([alice, bob]));
    for path in [week.to_owned(), format!("{week}?sortBy=most_volume")] {
        let (code, volume) = get_json(&app, &path).await;
        assert_eq!(code, StatusCode::OK, "{volume}");
        assert_eq!(volume["data"], json!([bob, alice]));
    }

    // Replace the proof without changing the completion time. An Ethereum
    // mint still requires its item on the served chain.
    for (chain_id, proof, expected) in [(11155111, 'b', false), (1, 'c', true)] {
        sqlx::query("UPDATE shop.items SET chain_id=$1 WHERE network='ethereum'")
            .bind(chain_id)
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query(
            "UPDATE shop.mirror_state SET pass_block_hash=$1 WHERE network='ethereum' AND mirror='items'",
        )
        .bind(format!("0x{}", proof.to_string().repeat(64)))
        .execute(&db.pool)
        .await
        .unwrap();
        let (code, result) = get_json(&app, &count_path).await;
        if expected {
            assert_eq!((code, result), (StatusCode::OK, counts.clone()));
        } else {
            assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{result}");
            assert_eq!(result["source"], "collections-index");
        }
    }

    let (code, month) = get_json(&app, "/v1/rankings/creators/month").await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{month}");
    assert_eq!(month["source"], "sales-index");

    // The incomplete payment is outside the week, but its creator is
    // required for the cumulative relationships.
    sqlx::query(
        "UPDATE shop.sale_asset SET contract_address=$1 WHERE network='matic' AND block_number=$2",
    )
    .bind(CORRECTED_CREATOR)
    .bind(polygon - 15)
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE shop.sale_publication SET generation=gen_random_uuid() WHERE network='matic'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let (code, missing) = get_json(&app, &count_path).await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{missing}");
    assert_eq!(missing["source"], "collections-index");
    sqlx::query(
        "UPDATE shop.sale_asset SET contract_address=$1 WHERE network='matic' AND block_number=$2",
    )
    .bind(OTHER_COLLECTION)
    .bind(polygon - 15)
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "WITH p AS (UPDATE shop.sale_payment SET settled=true WHERE network='matic' AND block_number=$1
                    RETURNING network,block_number,log_index,ordinal,gross)
         INSERT INTO shop.sale_payout(network,block_number,log_index,payment_ordinal,ordinal,role,recipient,amount)
         SELECT network,block_number,log_index,ordinal,0,'seller',$2,gross FROM p",
    )
    .bind(polygon-15)
    .bind(CARL)
    .execute(&db.pool)
    .await
    .unwrap();
    {
        let mut conn = db.pool.acquire().await.unwrap();
        sale_fold::rollups(&mut conn, ChainNet::Matic, &[older, day - 86400])
            .await
            .unwrap();
        sale_fold::rollups(&mut conn, ChainNet::Ethereum, &[day - 86400])
            .await
            .unwrap();
    }

    let all = "/v1/rankings/creators/all";
    let alice_lifetime =
        json!({"id":ALICE,"sales":0,"earned":"0","uniqueCollectors":0,"collections":1});
    let bob_lifetime =
        json!({"id":BOB,"sales":1,"earned":"50","uniqueCollectors":1,"collections":2});
    for (path, expected) in [
        (all.to_owned(), json!([bob_lifetime])),
        (
            format!("{all}?sortBy=most_sales"),
            json!([alice_lifetime, bob_lifetime]),
        ),
        (
            format!("{all}?sortBy=most_volume"),
            json!([bob_lifetime, alice_lifetime]),
        ),
    ] {
        let (code, page) = get_json(&app, &path).await;
        assert_eq!(code, StatusCode::OK, "{page}");
        assert_eq!(page["data"], expected, "{path}");
    }

    let (code, warm) = get_json(&app, &count_path).await;
    assert_eq!((code, warm), (StatusCode::OK, counts.clone()));
    sqlx::query(
        "UPDATE shop.mirror_state SET last_complete_pass_at=timezone('UTC',now())+interval '1 day'
         WHERE network='ethereum' AND mirror='items'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let (code, future) = get_json(&app, &count_path).await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{future}");
    assert_eq!(future["source"], "items-mirror");
    sqlx::query(
        "UPDATE shop.mirror_state SET last_complete_pass_at=timezone('UTC',now())-interval '1 day'
         WHERE network='ethereum' AND mirror='items'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let (code, stale) = get_json(&app, &count_path).await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{stale}");
    assert_eq!(stale["source"], "items-mirror");
    sqlx::query(
        "UPDATE shop.mirror_state SET last_complete_pass_at=timezone('UTC',now())+interval '30 seconds'
         WHERE network='ethereum' AND mirror='items'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        get_json(&app, &count_path).await,
        (StatusCode::OK, counts.clone())
    );
    sqlx::query(
        "UPDATE shop.mirror_state SET last_complete_pass_at=NULL WHERE network='matic' AND mirror='collections'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let (code, withdrawn) = get_json(&app, &count_path).await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{withdrawn}");
    assert_eq!(withdrawn["source"], "collections-mirror");
    sqlx::query(
        "UPDATE shop.mirror_state SET last_complete_pass_at=timezone('UTC',now())-interval '1 day'
         WHERE network='matic' AND mirror='collections'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let (code, stale) = get_json(&app, &count_path).await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{stale}");
    assert_eq!(stale["source"], "collections-mirror");
    sqlx::query(
        "UPDATE shop.mirror_state SET last_complete_pass_at=timezone('UTC',now())
         WHERE network='matic' AND mirror='collections'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    assert_eq!(get_json(&app, &count_path).await, (StatusCode::OK, counts));
    sqlx::query(
        "UPDATE shop.collection_event SET previous_account=$1 WHERE network='matic' AND block_number=$2 AND log_index=20",
    )
    .bind(CORRECTED_CREATOR)
    .bind(polygon-2)
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE shop.chain_cursor SET sale_history_revision=sale_history_revision+1 WHERE network='matic' AND indexer='collections'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let (code, corrected) = get_json(&app, &count_path).await;
    assert_eq!(code, StatusCode::OK, "{corrected}");
    let corrected_creator = json!({"id":CORRECTED_CREATOR,"sales":2,"earned":"70","uniqueCollectors":1,"collections":1});
    let remaining_alice =
        json!({"id":ALICE,"sales":2,"earned":"60","uniqueCollectors":2,"collections":2});
    assert_eq!(
        corrected["data"],
        json!([remaining_alice, bob, corrected_creator])
    );

    let index = CollectionsIndex::new();
    let lifetime = format!("{all}?sortBy=most_sales");
    let lifetime_page = get_json(&app, &lifetime).await;
    assert_eq!(lifetime_page.0, StatusCode::OK);
    for (retained_through, retains_seed) in [(polygon, true), (polygon - 2, false)] {
        let mut tx = db.pool.begin().await.unwrap();
        index.rewind(&mut tx, retained_through).await.unwrap();
        // The collections log through the sales prefix has been republished
        // after the rewind. A seed above the retained block still needs a read.
        sqlx::query(
            "UPDATE shop.chain_cursor SET reorg_rewinds=reorg_rewinds+1,
                sale_history_revision=sale_history_revision+1, folded_at=timezone('UTC',now())
             WHERE network='matic' AND indexer='collections'",
        )
        .execute(&mut *tx)
        .await
        .unwrap();
        tx.commit().await.unwrap();
        let (code, page) = get_json(&app, &count_path).await;
        if retains_seed {
            assert_eq!((code, page), (StatusCode::OK, corrected.clone()));
            assert_eq!(get_json(&app, &lifetime).await, lifetime_page);
        } else {
            assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{page}");
            assert_eq!(page["source"], "collections-index");
        }
    }
    // A metadata update after both mints remains within the published
    // prefix. With no creatorship transfers, it can prove both positions.
    sqlx::query(
        "INSERT INTO shop.sale_execution(network,block_number,log_index,tx_hash,venue,kind,block_time,receipt_read)
         VALUES('matic',$1,1,$2,'order','order',$3,true)",
    )
    .bind(polygon)
    .bind(format!("0x{:064x}", polygon * 100 + 1))
    .bind(recent + 3)
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO shop.collections(network,contract_address,chain_id,name,creator,owner,urn,
            is_completed,is_approved,is_editable,created_at,updated_at,reviewed_at,mirrored_at)
         VALUES('matic',$1,137,'Existing',$2,$2,'urn:decentraland:matic:collections-v2:creator-rank',
            true,true,false,$3,$4,$3,timezone('UTC',now()))",
    )
    .bind(OTHER_COLLECTION)
    .bind(ALICE)
    .bind(older - 1)
    .bind(recent + 2)
    .execute(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        get_json(&app, &count_path).await,
        (StatusCode::OK, corrected.clone())
    );
    sqlx::query("DELETE FROM shop.collections WHERE network='matic' AND contract_address=$1")
        .bind(OTHER_COLLECTION)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE shop.mirror_state SET last_complete_pass_at=timezone('UTC',now())
         WHERE network='matic' AND mirror='collections'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let token = reads_token(&db.pool, ChainNet::Matic).await.unwrap();
    assert_eq!(
        write_seeds(
            &db.pool,
            ChainNet::Matic,
            &[Seed {
                contract_address: OTHER_COLLECTION.into(),
                name: "Existing".into(),
                symbol: "E".into(),
                creator: ALICE.into(),
                is_approved: true,
                is_editable: false,
                read_block: polygon,
            }],
            &format!("0x{polygon:064x}"),
            token,
        )
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        get_json(&app, &count_path).await,
        (StatusCode::OK, corrected)
    );
    assert_eq!(get_json(&app, &lifetime).await, lifetime_page);

    sqlx::query(
        "UPDATE shop.chain_cursor SET last_block=$1,last_block_hash=$2,last_block_time=$3,
                sale_history_revision=sale_history_revision+1
         WHERE network='matic' AND indexer='collections'",
    )
    .bind(polygon + 20)
    .bind(format!("0x{:064x}", polygon + 20))
    .bind(recent + 30)
    .execute(&db.pool)
    .await
    .unwrap();
    assert_eq!(get_json(&app, &lifetime).await, lifetime_page);
    sqlx::query(
        "WITH events AS (
            INSERT INTO shop.collection_event(network,block_number,log_index,block_time,
                kind,contract_address,account,previous_account)
            VALUES('matic',$1-10,1,$2,'proxy_created',$3,$3,NULL),
                  ('matic',$1+1,1,$4,'creatorship_transferred',$3,$5,$6)
         )
         INSERT INTO shop.collections(network,contract_address,chain_id,name,creator,owner,urn,
            is_completed,is_approved,is_editable,created_at,updated_at,reviewed_at,mirrored_at)
         VALUES('matic',$3,137,'Walk ahead',$5,$5,'urn:decentraland:matic:collections-v2:walk-ahead',
            true,true,false,$2,$4,$2,timezone('UTC',now()))",
    )
    .bind(polygon)
    .bind(recent - 10)
    .bind(WALK_AHEAD_COLLECTION)
    .bind(recent + 4)
    .bind(BOB)
    .bind(ALICE)
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE shop.mirror_state SET last_complete_pass_at=timezone('UTC',now())
         WHERE network='matic' AND mirror='collections'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let (code, ahead) = get_json(&app, &lifetime).await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{ahead}");
    assert_eq!(ahead["source"], "collections-index");

    sqlx::query(
        "UPDATE shop.chain_cursor SET folded_block=last_block,folded_at=timezone('UTC',now())
         WHERE network='matic' AND indexer='collections'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let (code, recovered) = get_json(&app, &lifetime).await;
    assert_eq!(code, StatusCode::OK, "{recovered}");
    let founder = recovered["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == ALICE)
        .unwrap();
    assert_eq!(founder["collections"], 2);
    db.drop().await;
}
