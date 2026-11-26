use super::*;
use crate::market::{market_router, sale_fold, sale_history};

#[tokio::test]
async fn royalty_pages_use_proven_resale_payouts_and_keep_totals_outside_the_page() {
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
        let state = state_over(pool.clone(), &url, settings.clone(), false);
        sqlx::query("INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,last_block_hash,
            full_history,folded_block,folded_at,sale_input_version,sale_verified_through,sale_covered_rewinds)
            SELECT ix,$1,$2,1,$3,$4,true,$3,timezone('UTC',now()),$5,$3,0
            FROM unnest(ARRAY['nft','sales','trade_events']) ix")
            .bind(net.db_name()).bind(net.chain_id()).bind(height as i64)
            .bind(chain.hash_of(height)).bind(sale_history::VERSION).execute(pool).await.unwrap();
        sale_fold::pass(pool, &Rpc::at(&state.http, &url), net)
            .await
            .unwrap();
    }
    let state = state_over(pool.clone(), "http://127.0.0.1", settings, false);
    let app = market_router(state);
    sqlx::query(
        "INSERT INTO shop.mirror_state(mirror,network,source_url,last_complete_pass_at)
        VALUES ('items','ethereum','test',timezone('UTC',now())),
               ('items','matic','test',timezone('UTC',now())),
               ('collections','ethereum','test',timezone('UTC',now()))",
    )
    .execute(pool)
    .await
    .unwrap();
    seed_collection(pool, "ethereum", DAVE, 1, "royalty-v1", "v1").await;
    seed_item(
        pool,
        "ethereum",
        DAVE,
        9,
        "royalty-v1:hat",
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
        "royalty-v2",
        "Cap",
        "wearable_v2",
        "hat",
    )
    .await;
    sqlx::query(
        "INSERT INTO shop.nft(network,contract_address,token_id,owner,wearable_id,
        minted_block,minted_at,transferred_block,transferred_log,transferred_at)
        VALUES ('ethereum',$1,7,$2,'hat',1,1,1,1,1)",
    )
    .bind(DAVE)
    .bind(ALICE)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("WITH samples(network,log_index,token_id,item_id,currency,venue_kind,asset_kind,time,unpublished) AS (
        VALUES ('ethereum',1,7,NULL,$4::text,'order','order',10,0),
               ('matic',2,8,1,$5,'trade','bid',20,0),
               ('matic',3,9,1,$5,'order','order',30,0),
               ('matic',4,10,1,$5,'store','mint',40,0),
               ('matic',5,11,1,$5,'order','order',50,1)
    ), executions AS (
        INSERT INTO shop.sale_execution(network,block_number,log_index,tx_hash,venue,kind,block_time)
        SELECT s.network,p.block_number+s.unpublished,s.log_index,'0x'||lpad(to_hex(s.log_index),64,'0'),
            $1,s.venue_kind,s.time FROM samples s JOIN shop.sale_publication p USING(network)
        RETURNING network,block_number,log_index,tx_hash
    ), assets AS (
        INSERT INTO shop.sale_asset(network,block_number,log_index,ordinal,public_id,beneficiary,
            kind,contract_address,token_id,item_id,seller,buyer,price,currency)
        SELECT e.network,e.block_number,e.log_index,0,'royalty-'||e.log_index,$3,s.asset_kind,$1,
            s.token_id,s.item_id,$2,$3,1000,s.currency FROM executions e JOIN samples s USING(network,log_index)
        RETURNING network,block_number,log_index,ordinal,price,currency
    ), payments AS (
        INSERT INTO shop.sale_payment(network,block_number,log_index,ordinal,currency,gross,settled,beneficiary)
        SELECT network,block_number,log_index,ordinal,currency,price,true,$2 FROM assets
        RETURNING network,block_number,log_index,ordinal
    )
    INSERT INTO shop.sale_payout(network,block_number,log_index,payment_ordinal,ordinal,role,recipient,amount)
    SELECT network,block_number,log_index,ordinal,0,'royalty',$6,CASE WHEN log_index=3 THEN 50 ELSE 25 END
    FROM payments WHERE log_index IN (2,3,4,5)
    UNION ALL
    SELECT network,block_number,log_index,ordinal,1,'royalty',$1,7 FROM payments WHERE log_index=2
    UNION ALL
    SELECT network,block_number,log_index,ordinal,1,'royalty',$6,3 FROM payments WHERE log_index=3")
        .bind(DAVE).bind(ALICE).bind(BOB).bind(crate::market::listings::mana_token(1))
        .bind(crate::market::listings::mana_token(137)).bind(CARL)
        .execute(pool).await.unwrap();
    sqlx::query(
        "WITH execution AS (
            INSERT INTO shop.sale_execution(network,block_number,log_index,tx_hash,venue,kind,block_time)
            SELECT network,block_number,6,'0x'||lpad('6',64,'0'),$1,'trade',60
            FROM shop.sale_publication WHERE network='matic'
            RETURNING network,block_number,log_index
        )
        INSERT INTO shop.sale_asset(network,block_number,log_index,ordinal,public_id,beneficiary,
            kind,contract_address,token_id,item_id,seller,buyer)
        SELECT network,block_number,log_index,ordinal,'unpriced-'||ordinal,$2,
            'order',$1,ordinal+20,1,$3,$2 FROM execution CROSS JOIN generate_series(0,1) ordinal",
    )
    .bind(DAVE)
    .bind(BOB)
    .bind(ALICE)
    .execute(pool)
    .await
    .unwrap();
    let base = format!("/v1/sales/royalties?creator={CARL}");
    let (status, page) = get_json(&app, &format!("{base}&first=1")).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(
        page,
        json!({"data":[{
        "id":"royalty-3","timestamp":30000,"contractAddress":DAVE,"itemId":"1","tokenId":"9",
        "priceWei":"1000","royaltyWei":"53","collector":CARL,"buyer":BOB,"seller":ALICE,
        "network":"matic"
    }],"total":3,"royaltiesWei":"85"})
    );
    let (status, split) = get_json(&app, &format!("{base}&from=20000&to=20000")).await;
    assert_eq!(status, StatusCode::OK, "{split}");
    assert_eq!(split["data"][0]["id"], "royalty-2");
    assert_eq!(split["data"][0]["royaltyWei"], "32");
    assert_eq!(split["data"][0]["collector"], Value::Null);
    assert_eq!(split["royaltiesWei"], "32");
    assert_eq!(split["total"], 1);
    let (status, old) = get_json(&app, &format!("{base}&to=10000")).await;
    assert_eq!(status, StatusCode::OK, "{old}");
    assert_eq!(old["data"][0]["itemId"], "9");
    assert_eq!(old["data"][0]["royaltyWei"], "0");
    assert_eq!(old["data"][0]["collector"], Value::Null);
    assert_eq!(old["total"], 1);
    assert_eq!(
        get_json(&app, &format!("{base}&from=10001&skip=200")).await,
        (
            StatusCode::OK,
            json!({"data":[],"total":2,"royaltiesWei":"85"})
        )
    );
    assert_eq!(
        get_json(&app, &format!("/v1/sales/royalties?creator={ALICE}")).await,
        (
            StatusCode::OK,
            json!({"data":[],"total":0,"royaltiesWei":"0"})
        )
    );
    for query in [
        "from=-1",
        "to=8640000000000001",
        "from=2&to=1",
        "first=0",
        "skip=0.2",
    ] {
        assert_eq!(
            get_json(&app, &format!("{base}&{query}")).await.0,
            StatusCode::BAD_REQUEST,
            "{query}"
        );
    }
    sqlx::query("UPDATE shop.sale_payment SET settled=false WHERE log_index=2")
        .execute(pool)
        .await
        .unwrap();
    let (status, unresolved) = get_json(&app, &format!("{base}&first=1")).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{unresolved}");
    assert_eq!(unresolved["source"], "sales-index");
    sqlx::query("UPDATE shop.sale_publication SET published_at=NULL WHERE network='matic'")
        .execute(pool)
        .await
        .unwrap();
    let (status, withdrawn) = get_json(&app, &base).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{withdrawn}");
    assert_eq!(withdrawn["source"], "sales-index");
    db.drop().await;
}
