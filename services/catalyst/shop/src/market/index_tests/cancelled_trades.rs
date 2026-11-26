use super::*;
use crate::market::market_router;
use crate::shared::signed_fetch::test_support::{address, signed_headers};
use tower::ServiceExt;

const PATH: &str = "/v1/cancelled-trades";
const QUERY: &str = "/v1/cancelled-trades?reason=contract_signature_index_bump";

async fn listing(pool: &PgPool, signer: &str, venue: &str, token: i64, created: i64) -> uuid::Uuid {
    let id = uuid::Uuid::new_v4();
    let key = format!("0x{:064x}", id.as_u128());
    sqlx::query("INSERT INTO shop.trades(id,network,chain_id,signature,hashed_signature,checks,signer,
        type,expires_at,effective_since,created_at,contract)
        VALUES($1,'MATIC',137,$2,$2,'{\"signerSignatureIndex\":0,\"contractSignatureIndex\":0,\"uses\":1}',
            $3,'public_nft_order',timezone('UTC',now())+interval '1 hour',
            to_timestamp($4) AT TIME ZONE 'UTC',to_timestamp($4) AT TIME ZONE 'UTC',$5)")
        .bind(id).bind(key).bind(signer).bind(created as f64).bind(venue)
        .execute(pool).await.unwrap();
    sqlx::query(
        "WITH asset AS (
        INSERT INTO shop.trade_assets(trade_id,direction,asset_type,contract_address)
        VALUES($1,'sent',3,$2) RETURNING id)
        INSERT INTO shop.trade_assets_erc721(asset_id,token_id)
        SELECT id,$3 FROM asset",
    )
    .bind(id)
    .bind(CARL)
    .bind(token.to_string())
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "WITH price AS (
        INSERT INTO shop.trade_assets(trade_id,direction,asset_type,contract_address)
        VALUES($1,'received',1,$2) RETURNING id)
        INSERT INTO shop.trade_assets_erc20(asset_id,amount) SELECT id,1000 FROM price",
    )
    .bind(id)
    .bind(listings::mana_token(137))
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn signed_get(app: &axum::Router, path: &str) -> (StatusCode, Value) {
    let headers = signed_headers("get", PATH, r#"{"signer":"dcl:marketplace"}"#, true).await;
    let mut request = axum::http::Request::builder()
        .uri(path)
        .body(axum::body::Body::empty())
        .unwrap();
    *request.headers_mut() = headers;
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn cancelled_trades_prove_the_bump_against_the_signed_wallet_and_published_history() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let pool = &db.pool;
    let signer = address();
    let venue = listings::offchain_marketplace_v2(137).unwrap().address;
    let other_venue = listings::offchain_marketplace_v3(137).unwrap().address;
    let state = state_over(
        pool.clone(),
        "http://127.0.0.1",
        index_settings(100, 20),
        true,
    );
    let app = market_router(state);
    sqlx::query(
        "INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,
        full_history,folded_block,folded_at)
        SELECT ix,n,c,1,12,true,10,timezone('UTC',now())
        FROM unnest(ARRAY['nft','trade_events']) ix
        CROSS JOIN (VALUES ('ethereum',1),('matic',137)) networks(n,c)",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO shop.mirror_state(mirror,network,source_url,last_complete_pass_at)
        SELECT 'items',net,'test',timezone('UTC',now()) FROM unnest(ARRAY['ethereum','matic']) net",
    )
    .execute(pool)
    .await
    .unwrap();
    seed_item(
        pool,
        "matic",
        CARL,
        1,
        "cancelled-hat",
        "Hat",
        "wearable_v2",
        "hat",
    )
    .await;
    sqlx::query(
        "INSERT INTO shop.nft(network,contract_address,token_id,owner,item_id,minted_block,
        minted_at,transferred_block,transferred_log,transferred_at)
        SELECT 'matic',$1,token,CASE WHEN token=5 THEN $3 ELSE $2 END,1,1,1,1,1,1
        FROM generate_series(1,7) token",
    )
    .bind(CARL)
    .bind(&signer)
    .bind(BOB)
    .execute(pool)
    .await
    .unwrap();
    let kept = listing(pool, &signer, venue, 1, 101).await;
    let individually_cancelled = listing(pool, &signer, venue, 2, 102).await;
    let executed = listing(pool, &signer, venue, 3, 103).await;
    let cancelled_later = listing(pool, &signer, venue, 4, 104).await;
    listing(pool, &signer, venue, 5, 105).await; // no longer owned
    listing(pool, &signer, venue, 6, 106).await;
    listing(pool, &signer, other_venue, 6, 107).await; // re-created
    listing(pool, BOB, venue, 7, 108).await; // another wallet
    for (log, kind, id) in [
        (
            2,
            trade_events::KIND_SIGNATURE_CANCELLED,
            individually_cancelled,
        ),
        (3, trade_events::KIND_TRADED, executed),
        (6, trade_events::KIND_SIGNATURE_CANCELLED, cancelled_later),
    ] {
        sqlx::query(
            "INSERT INTO shop.trade_event(network,block_number,log_index,block_time,
            marketplace_address, kind, caller, signer, hashed_signature, tx_hash)
            SELECT 'matic',10,$1,200,$2,$3,signer,signer,hashed_signature,'0x'||repeat('1',64)
            FROM shop.trades WHERE id=$4",
        )
        .bind(log)
        .bind(venue)
        .bind(kind)
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO shop.trade_event(network,block_number,log_index,block_time,
        marketplace_address, kind, caller, new_index,tx_hash)
        VALUES ('matic',10,5,200,$1,'contract_index_increased',$1,1,'0x'||repeat('2',64)),
               ('matic',11,0,201,$2,'contract_index_increased',$2,1,'0x'||repeat('3',64))",
    )
    .bind(venue)
    .bind(other_venue)
    .execute(pool)
    .await
    .unwrap();
    assert_eq!(get_json(&app, QUERY).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(signed_get(&app, PATH).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(
        signed_get(&app, &format!("{QUERY}&first=0")).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        signed_get(&app, &format!("{QUERY}&type=invalid")).await.0,
        StatusCode::BAD_REQUEST
    );
    let (status, body) = signed_get(&app, &format!("{QUERY}&type=public_nft_order&first=1")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "data":[{"id":cancelled_later,"type":"public_nft_order","network":"MATIC","chainId":137,
                "contract":venue,"reason":"contract_signature_index_bump","createdAt":104000,
                "expiresAt":body["data"][0]["expiresAt"],"cancelledAt":200000,
                "asset":{"contractAddress":CARL,"tokenId":"4","itemId":null,"name":"Hat","image":"https://img/cancelled-hat"},
                "price":{"assetType":1,"amount":"1000"}}],"total":2
        })
    );
    let (status, second) = signed_get(&app, &format!("{QUERY}&first=1&skip=1")).await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert_eq!(second["data"][0]["id"], json!(kept));
    assert_eq!(second["total"], 2);
    assert_eq!(
        signed_get(&app, &format!("{QUERY}&skip=2")).await,
        (StatusCode::OK, json!({"data":[],"total":2}))
    );
    assert_eq!(
        signed_get(&app, &format!("{QUERY}&type=bid")).await,
        (StatusCode::OK, json!({"data":[],"total":0}))
    );
    let ethereum_venue = listings::offchain_marketplace_v2(1).unwrap().address;
    let land = crate::land_authz::LAND_MAINNET;
    let land_trade = listing(pool, &signer, venue, 1, 109).await;
    sqlx::query("UPDATE shop.trades SET network='ETHEREUM',chain_id=1,contract=$2 WHERE id=$1")
        .bind(land_trade)
        .bind(ethereum_venue)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE shop.trade_assets SET contract_address=$2 WHERE trade_id=$1 AND asset_type=3",
    )
    .bind(land_trade)
    .bind(land.land_registry)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO shop.trade_event(network,block_number,log_index,block_time,
        marketplace_address,kind,caller,new_index,tx_hash)
        VALUES ('ethereum',10,5,200,$1,'contract_index_increased',$1,1,'0x'||repeat('4',64))",
    )
    .bind(ethereum_venue)
    .execute(pool)
    .await
    .unwrap();
    let (status, missing_land) = signed_get(&app, QUERY).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{missing_land}");
    assert_eq!(missing_land["source"], "land-authz-index");
    sqlx::query(
        "INSERT INTO land_authz.index_cursor
        (network,land_registry,estate_registry,last_block,folded_block,folded_at)
        VALUES ($1,$2,$3,10,10,timezone('UTC',now()))",
    )
    .bind(land.network)
    .bind(land.land_registry)
    .bind(land.estate_registry)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO land_authz.token_right(token_address,token_id,x,y,owner,updated_block,updated_log)
        VALUES ($1,1,0,1,$2,1,0)")
        .bind(land.land_registry).bind(&signer).execute(pool).await.unwrap();
    let (status, ready_land) = signed_get(&app, &format!("{QUERY}&first=1")).await;
    assert_eq!(status, StatusCode::OK, "{ready_land}");
    assert_eq!(ready_land["total"], 3);
    assert_eq!(ready_land["data"][0]["id"], json!(land_trade));
    assert_eq!(
        ready_land["data"][0]["asset"]["contractAddress"],
        land.land_registry
    );
    sqlx::query(
        "UPDATE shop.chain_cursor SET chain_id=80002
        WHERE network='matic' AND indexer='trade_events'",
    )
    .execute(pool)
    .await
    .unwrap();
    let (status, wrong_chain) = signed_get(&app, QUERY).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{wrong_chain}");
    assert_eq!(wrong_chain["source"], "trade-events-index");
    sqlx::query(
        "UPDATE shop.chain_cursor SET chain_id=137,folded_block=NULL,folded_at=NULL
        WHERE network='matic' AND indexer='trade_events'",
    )
    .execute(pool)
    .await
    .unwrap();
    let (status, withdrawn) = signed_get(&app, QUERY).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{withdrawn}");
    assert_eq!(withdrawn["source"], "trade-events-index");
    db.drop().await;
}
