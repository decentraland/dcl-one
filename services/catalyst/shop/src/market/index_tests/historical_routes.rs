use super::*;
use crate::land_authz::{ESTATE_REGISTRY_MAINNET, LAND_MAINNET, LAND_REGISTRY_MAINNET};
use crate::market::market_router;

const TOKEN: &str = "historical-fixture-bearer-for-tests";
const PROVIDER: &str = "urn:decentraland:matic:collections-thirdparty:test";

async fn post(
    app: &Router,
    path: &str,
    authorization: Option<&str>,
    body: Value,
) -> (StatusCode, axum::http::HeaderMap, Value) {
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use tower::ServiceExt;
    let mut req = Request::post(path).header("content-type", "application/json");
    if let Some(value) = authorization {
        req = req.header("authorization", value);
    }
    let response = app
        .clone()
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let (parts, body) = response.into_parts();
    let body = to_bytes(body, 1024 * 1024).await.unwrap();
    (
        parts.status,
        parts.headers,
        serde_json::from_slice(&body).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn historical_routes_read_only_the_published_prefix() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let chain = FakeChain::new(150, vec![], 1_000, 12);
    let url = serve(chain.clone()).await;
    let mut state = state_over(db.pool.clone(), &url, index_settings(100, 10), false);
    let (status, _, _) = post(
        &market_router(state.clone()),
        "/v1/historical/head",
        None,
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let settings = &mut Arc::get_mut(&mut state).unwrap().settings;
    settings.historical_admin_token = Some(TOKEN.to_string());
    settings.eth_network = "ethereum".to_string();
    let mut testnet = state_over(db.pool.clone(), &url, index_settings(100, 10), false);
    let settings = &mut Arc::get_mut(&mut testnet).unwrap().settings;
    settings.historical_admin_token = Some(TOKEN.to_string());
    settings.eth_network = "sepolia".to_string();
    let auth = format!("Bearer {TOKEN}");
    let (status, _, _) = post(
        &market_router(testnet),
        "/v1/historical/head",
        Some(&auth),
        json!({"squid":"marketplace","chain":"ethereum"}),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(!state.settings.data_writes_enabled);
    let app = market_router(state);
    let bearer = format!("Bearer {TOKEN}");

    sqlx::query(
        "INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,full_history,folded_block,folded_at)
         VALUES ('nft','ethereum',1,1,140,true,120,timezone('UTC',now())),
                ('rentals','ethereum',1,1,140,true,120,timezone('UTC',now())),
                ('third_party_registry','matic',137,1,140,true,120,timezone('UTC',now()))",
    ).execute(&db.pool).await.unwrap();
    sqlx::query(
        "INSERT INTO shop.chain_checkpoint(indexer,network,block_number,block_hash,block_time)
         SELECT indexer,network,120,$1,2440 FROM shop.chain_cursor",
    )
    .bind(chain.hash_of(120))
    .execute(&db.pool)
    .await
    .unwrap();

    // The LAND publisher records the hash on its update, then the walk
    // advances without changing the published height or its identity.
    sqlx::query(
        "INSERT INTO land_authz.index_cursor
         (network,land_registry,estate_registry,last_block,last_block_hash,full_replay_started,history_timestamps_ready)
         VALUES ($1,$2,$3,120,$4,true,true)",
    ).bind(LAND_MAINNET.network).bind(LAND_REGISTRY_MAINNET).bind(ESTATE_REGISTRY_MAINNET)
     .bind(chain.hash_of(120)).execute(&db.pool).await.unwrap();
    sqlx::query(
        "UPDATE land_authz.index_cursor SET folded_block=120,folded_at=timezone('UTC',now())",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query("UPDATE land_authz.index_cursor SET last_block=140,last_block_hash=$1")
        .bind(chain.hash_of(140))
        .execute(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT folded_block_hash FROM land_authz.index_cursor")
            .fetch_one(&db.pool)
            .await
            .unwrap(),
        chain.hash_of(120),
    );

    sqlx::query(
        "INSERT INTO land_authz.authz_event(block_number,log_index,block_time,token_address,kind,token_id,estate_id,account)
         VALUES (80,0,1,$1,'transfer',2,NULL,$2),
                (80,1,1,$1,'transfer',10,NULL,$2),
                (91,0,1,$1,'add_land',10,2,NULL),
                (91,1,1,$1,'add_land',2,2,NULL),
                (111,0,1,$1,'remove_land',10,2,NULL),
                (111,1,1,$1,'add_land',10,10,NULL),
                (130,0,1,$1,'transfer',2,NULL,$3),
                (130,1,1,$1,'remove_land',2,2,NULL)",
    ).bind(ESTATE_REGISTRY_MAINNET).bind(ALICE).bind(BOB).execute(&db.pool).await.unwrap();
    sqlx::query(
        "INSERT INTO shop.rental_event(network,block_number,log_index,block_time,rentals_address,
                kind,contract_address,token_id,signer,tenant,rental_days,price_per_day,signature,tx_hash)
         VALUES ('ethereum',90,0,1,$1,'rented',$2,2,$3,$4,1,1,'0x12','0xa'),
                ('ethereum',105,0,1,$1,'claimed',$2,2,NULL,NULL,NULL,NULL,NULL,'0xb'),
                ('ethereum',115,0,1,$1,'rented',$2,2,$4,$3,1,1,'0x14','0xc')",
    ).bind(crate::rentals::index::RentalsChain::Mainnet.venue()).bind(ESTATE_REGISTRY_MAINNET)
     .bind(ALICE).bind(BOB).execute(&db.pool).await.unwrap();
    sqlx::query(
        "INSERT INTO shop.third_party_registry_event(block_number,log_index,block_time,provider_id,state,is_review)
         VALUES(80,0,1,$1,'{\"root\":null,\"approved\":true}',false),
               (85,0,1,$1,'{\"root\":null,\"approved\":true}',true),
               (95,0,1,$1,'{\"root\":\"0xabc\",\"approved\":true}',true),
               (110,0,1,$1,'{\"root\":\"0xabc\",\"approved\":false}',true),
               (123,0,1,$1,'{\"root\":\"0xdef\",\"approved\":true}',NULL)",
    ).bind(PROVIDER).execute(&db.pool).await.unwrap();

    let historical = |path: &str| format!("/v1/historical/{path}");
    let (status, _, _) = post(&app, &historical("head"), None, json!({})).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, unavailable) = post(
        &app,
        &historical("head"),
        Some(&bearer),
        json!({"squid":"marketplace","chain":"ethereum"}),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{unavailable}");
    assert_eq!(unavailable["source"], "collections-index");
    sqlx::query(
        "INSERT INTO shop.mirror_state(mirror,network,source_url,last_complete_pass_at,chain_id,pass_started_at,pass_block_hash,marks_item_rows)
         SELECT mirror,'ethereum','test',timezone('UTC',now()),1,timezone('UTC',now())-interval '1 minute',$1,mirror='items'
         FROM unnest(ARRAY['collections','items']) mirror",
    )
    .bind(chain.hash_of(80))
    .execute(&db.pool)
    .await
    .unwrap();
    let (status, _, old_inventory) = post(
        &app,
        &historical("head"),
        Some(&bearer),
        json!({"squid":"marketplace","chain":"ethereum"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{old_inventory}");
    assert_eq!(old_inventory["data"]["number"], 80);
    assert_eq!(old_inventory["data"]["hash"], chain.hash_of(80));
    chain
        .by_number_parents
        .lock()
        .insert(95, format!("0xbb{:062x}", 94));
    let (status, _, split_inventory) = post(
        &app,
        &historical("head"),
        Some(&bearer),
        json!({"squid":"marketplace","chain":"ethereum"}),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{split_inventory}");
    assert_eq!(split_inventory["source"], "ethereum-publication");
    chain.by_number_parents.lock().remove(&95);
    sqlx::query("UPDATE shop.mirror_state SET pass_block_hash=$1 WHERE network='ethereum' AND mirror IN ('collections','items')")
        .bind(chain.hash_of(120)).execute(&db.pool).await.unwrap();
    sqlx::query("INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,full_history,folded_block,folded_at)
         VALUES ('nft','matic',137,1,140,true,120,timezone('UTC',now())),
                ('collections','matic',137,1,140,true,120,timezone('UTC',now()))")
        .execute(&db.pool).await.unwrap();
    sqlx::query(
        "INSERT INTO shop.chain_checkpoint(indexer,network,block_number,block_hash,block_time)
         VALUES ('nft','matic',120,$1,2440),('collections','matic',120,$1,2440)",
    )
    .bind(chain.hash_of(120))
    .execute(&db.pool)
    .await
    .unwrap();
    let polygon = json!({"squid":"marketplace","chain":"polygon"});
    let (status, _, unavailable) =
        post(&app, &historical("head"), Some(&bearer), polygon.clone()).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{unavailable}");
    assert_eq!(unavailable["source"], "collections-index");
    sqlx::query(
        "INSERT INTO shop.collection_projection(network,owned_at,projected_through)
         VALUES ('matic',timezone('UTC',now()),120)",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let (status, _, polygon_head) = post(&app, &historical("head"), Some(&bearer), polygon).await;
    assert_eq!(status, StatusCode::OK, "{polygon_head}");

    let (status, headers, vesting) = post(
        &app,
        &historical("head"),
        Some(&bearer),
        json!({"squid":"registry","chain":"ethereum","processor":"vesting"}),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{vesting}");
    assert_eq!(headers["retry-after"], "30");
    assert_eq!(vesting["code"], "unavailable");
    assert_eq!(vesting["source"], "registry-vesting-index");

    sqlx::query("DELETE FROM shop.chain_checkpoint WHERE indexer='third_party_registry'")
        .execute(&db.pool)
        .await
        .unwrap();
    let (status, _, registry) = post(
        &app,
        &historical("head"),
        Some(&bearer),
        json!({"squid":"registry","chain":"polygon"}),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{registry}");
    assert_eq!(registry["source"], "third-party-registry-index");

    let (status, _, head) = post(
        &app,
        &historical("head"),
        Some(&bearer),
        json!({"squid":"marketplace","chain":"ethereum"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{head}");
    assert_eq!(
        head,
        json!({"ok":true, "data":{"number":120,"hash":chain.hash_of(120),"timestamp":2440}})
    );

    let (status, _, nfts) = post(&app, &historical("nfts"), Some(&bearer),
        json!({"block":120,"category":"estate","owners":[ALICE],"idGt":format!("{ESTATE_REGISTRY_MAINNET}-10")})).await;
    assert_eq!(status, StatusCode::OK, "{nfts}");
    assert_eq!(
        nfts["data"],
        json!([{"contractAddress":ESTATE_REGISTRY_MAINNET,"tokenId":"2","category":"estate",
        "owner":ALICE,"itemType":null,"searchWearableRarity":null,"searchEstateSize":1}])
    );
    let (status, _, estates) = post(
        &app,
        &historical("estates"),
        Some(&bearer),
        json!({"block":120,"tokenIds":["10","2","999"],"withParcels":true}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{estates}");
    assert_eq!(
        estates["data"],
        json!([
            {"tokenId":"10","size":1,"parcels":[{"tokenId":"10","x":0,"y":10}]},
            {"tokenId":"2","size":1,"parcels":[{"tokenId":"2","x":0,"y":2}]}
        ])
    );
    for (block, count) in [(100, 1), (105, 0), (120, 0)] {
        let (status, _, rentals) = post(
            &app,
            &historical("rental-assets"),
            Some(&bearer),
            json!({"block":block,"lessors":[ALICE]}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{rentals}");
        assert_eq!(rentals["data"].as_array().unwrap().len(), count);
    }

    for (block, expected) in [
        (80, json!([])),
        (85, json!([{"id":PROVIDER,"root":"","isApproved":false}])),
        (
            100,
            json!([{"id":PROVIDER,"root":"0xabc","isApproved":true}]),
        ),
        (
            120,
            json!([{"id":PROVIDER,"root":"0xabc","isApproved":false}]),
        ),
    ] {
        let (status, _, parties) = post(
            &app,
            &historical("third-parties"),
            Some(&bearer),
            json!({"block":block,"ids":[PROVIDER]}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{parties}");
        assert_eq!(parties["data"], expected);
    }
    let (status, headers, too_recent) = post(
        &app,
        &historical("third-parties"),
        Some(&bearer),
        json!({"block":121,"ids":[PROVIDER]}),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(headers["retry-after"], "60");
    assert_eq!(too_recent["code"], "block-too-recent");

    sqlx::query("UPDATE shop.third_party_registry_event SET is_review=NULL WHERE block_number=85")
        .execute(&db.pool)
        .await
        .unwrap();
    let (status, _, incomplete) = post(
        &app,
        &historical("third-parties"),
        Some(&bearer),
        json!({"block":120,"ids":[PROVIDER]}),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(incomplete["code"], "unavailable");
    sqlx::query("UPDATE land_authz.index_cursor SET folded_block_hash=$1")
        .bind(chain.hash_of(121))
        .execute(&db.pool)
        .await
        .unwrap();
    for squid in ["marketplace", "registry"] {
        let (status, _, fork) = post(
            &app,
            &historical("head"),
            Some(&bearer),
            json!({"squid":squid,"chain":"ethereum"}),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{fork}");
        assert_eq!(fork["source"], "ethereum-publication");
    }
    let (status, _, fork) = post(&app, &historical("nfts"), Some(&bearer),
        json!({"block":110,"owners":[ALICE],"contractAddresses":[ESTATE_REGISTRY_MAINNET,crate::market::nft_index::DCL_REGISTRAR]})).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{fork}");
    assert_eq!(fork["source"], "ethereum-publication");

    sqlx::query("UPDATE land_authz.index_cursor SET folded_block=119,last_block_hash=$1")
        .bind(chain.hash_of(119))
        .execute(&db.pool)
        .await
        .unwrap();
    let (status, _, lower) = post(
        &app,
        &historical("head"),
        Some(&bearer),
        json!({"squid":"marketplace","chain":"ethereum"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{lower}");
    assert_eq!(lower["data"]["number"], 119);
    assert_eq!(lower["data"]["hash"], chain.hash_of(119));

    let original = chain.hash_of(120);
    let fork_high = format!("0xbb{:062x}", 120);
    chain.forks.lock().insert(120, fork_high.clone());
    chain
        .by_hash_parents
        .lock()
        .insert(120, format!("0xbb{:062x}", 119));
    sqlx::query("UPDATE shop.chain_checkpoint SET block_hash=$1 WHERE network='ethereum' AND block_number=120")
        .bind(&fork_high).execute(&db.pool).await.unwrap();
    for squid in ["marketplace", "registry"] {
        let (status, _, fork) = post(
            &app,
            &historical("head"),
            Some(&bearer),
            json!({"squid":squid,"chain":"ethereum"}),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{fork}");
        assert_eq!(fork["source"], "ethereum-publication");
    }
    chain.by_hash_parents.lock().remove(&120);
    chain.forks.lock().remove(&120);
    sqlx::query("UPDATE shop.chain_checkpoint SET block_hash=$1 WHERE network='ethereum' AND block_number=120")
        .bind(original).execute(&db.pool).await.unwrap();

    seed_collection(&db.pool, "ethereum", CARL, 1, "collection", "Collection").await;
    sqlx::query(
        "UPDATE shop.collections SET created_at=$2 WHERE network='ethereum' AND contract_address=$1",
    )
    .bind(CARL).bind(time_of(91))
    .execute(&db.pool)
    .await
    .unwrap();
    seed_item(
        &db.pool,
        "ethereum",
        CARL,
        42,
        "collection:fixture",
        "Wearable",
        "wearable_v1",
        "hat",
    )
    .await;
    sqlx::query(
        "UPDATE shop.mirror_state SET chain_id=1 WHERE mirror='items' AND network='ethereum'",
    )
    .execute(&db.pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO shop.nft_contract(network,contract_address,walked_from_block,backfill_from_block,backfilled_to_block)
         VALUES('ethereum',$1,125,50,89)").bind(CARL).execute(&db.pool).await.unwrap();
    let wardrobe = json!({"block":90,"owners":[ALICE],"contractAddresses":[CARL]});
    let (status, _, gap) = post(&app, &historical("nfts"), Some(&bearer), wardrobe.clone()).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{gap}");
    assert_eq!(gap["source"], "nft-index");
    let (status, _, gap) = post(
        &app,
        &historical("head"),
        Some(&bearer),
        json!({"squid":"marketplace","chain":"ethereum"}),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{gap}");
    assert_eq!(gap["source"], "nft-index");
    sqlx::query("UPDATE shop.nft_contract SET backfilled_to_block=90 WHERE contract_address=$1")
        .bind(CARL)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO shop.nft_event(network,block_number,log_index,block_time,kind,emitter,contract_address,token_id,to_address,detail)
         VALUES('ethereum',88,0,1,'transfer',$1,$1,42,$2,NULL),
               ('ethereum',88,1,1,'issue_v1',$1,$1,42,NULL,'fixture')")
        .bind(CARL).bind(ALICE).execute(&db.pool).await.unwrap();
    let (status, _, native) =
        post(&app, &historical("nfts"), Some(&bearer), wardrobe.clone()).await;
    assert_eq!(status, StatusCode::OK, "{native}");
    assert_eq!(native["data"][0]["category"], "wearable");
    sqlx::query("UPDATE shop.items SET chain_id=11155111,item_type='emote_v1' WHERE network='ethereum' AND contract_address=$1")
        .bind(CARL).execute(&db.pool).await.unwrap();
    let (status, _, foreign) = post(&app, &historical("nfts"), Some(&bearer), wardrobe).await;
    assert_eq!(status, StatusCode::OK, "{foreign}");
    assert_eq!(foreign["data"], json!([]));

    let zero = "0x0000000000000000000000000000000000000000";
    sqlx::query("INSERT INTO land_authz.authz_event(block_number,log_index,block_time,token_address,kind,token_id,account)
         VALUES (112,0,1,$1,'transfer',7,$2)").bind(LAND_REGISTRY_MAINNET).bind(zero)
         .execute(&db.pool).await.unwrap();
    let (status, _, burned) = post(
        &app,
        &historical("nfts"),
        Some(&bearer),
        json!({"block":119,"category":"parcel","owners":[zero]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{burned}");
    assert_eq!(burned["data"][0]["owner"], zero);

    sqlx::query("UPDATE land_authz.index_cursor SET folded_block=NULL,folded_at=NULL")
        .execute(&db.pool)
        .await
        .unwrap();
    let (status, _, unpublished) = post(
        &app,
        &historical("estates"),
        Some(&bearer),
        json!({"block":100,"tokenIds":["2"]}),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(unpublished["code"], "unavailable");
    db.drop().await;
}
