use super::*;

#[tokio::test]
async fn sales_trade_replay_completes_old_payment_data_only_for_the_same_execution() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let mut conn = db.pool.acquire().await.unwrap();
    let net = ChainNet::Matic;
    let market = trade_events::marketplaces(net)[1].0;
    let log = trade_events::tests::traded_log(100, 1, market, ALICE, BOB, "0x01", 1, false);
    let execution = trade_events::decode_log(&log, 123).unwrap().unwrap();
    let mut old = execution.clone();
    old.assets.as_mut().unwrap()["received"][0]
        .as_object_mut()
        .unwrap()
        .remove("extra");
    assert_eq!(
        trade_events::insert_events(&mut conn, net, &[old])
            .await
            .unwrap(),
        1
    );
    let mut fork = execution.clone();
    fork.tx_hash = format!("0x{:064x}", 55);
    assert_eq!(
        trade_events::insert_events(&mut conn, net, &[fork])
            .await
            .unwrap(),
        0
    );
    for expected in [1, 0] {
        assert_eq!(
            trade_events::insert_events(&mut conn, net, std::slice::from_ref(&execution))
                .await
                .unwrap(),
            expected
        );
    }
    let assets: Value = sqlx::query_scalar("SELECT assets FROM shop.trade_event")
        .fetch_one(&mut *conn)
        .await
        .unwrap();
    assert_eq!(Some(assets), execution.assets);
    drop(conn);
    db.drop().await;
}

#[tokio::test]
async fn ethereum_sales_preserve_name_fees_and_bids_across_replay_and_rewind() {
    use crate::market::ethereum_sales::{
        ERC721_BID, EthereumSalesIndex, NAME_CONTROLLER, TOPIC_NAME_BOUGHT,
    };

    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let mut conn = db.pool.acquire().await.unwrap();
    let price = U256::from(100u8).pow(U256::from(10u8));
    let name = log_at(
        NAME_CONTROLLER,
        vec![TOPIC_NAME_BOUGHT.into(), topic_addr(ALICE), topic_addr(BOB)],
        (price, "alice").abi_encode_params(),
        100,
        3,
    );
    let mut bid =
        sales_index::tests::bid_accepted_log(ALICE, U256::from(4), BOB, CARL, 0x11, 400, 100, 4);
    bid.address = ERC721_BID.into();
    let logs = [name, bid];
    let times = HashMap::from([(100, 1_700_000_000)]);
    let index = EthereumSalesIndex;
    assert_eq!(index.write(&mut conn, &logs, &times).await.unwrap(), 2);
    assert_eq!(index.write(&mut conn, &logs, &times).await.unwrap(), 0);
    let row: (String, String, String, String) =
        sqlx::query_as("SELECT name, caller, beneficiary, price::text FROM shop.name_purchase")
            .fetch_one(&mut *conn)
            .await
            .unwrap();
    assert_eq!(
        row,
        ("alice".into(), ALICE.into(), BOB.into(), price.to_string())
    );
    sqlx::query("UPDATE shop.sale_event SET bid_fee = NULL")
        .execute(&mut *conn)
        .await
        .unwrap();
    assert_eq!(index.write(&mut conn, &logs, &times).await.unwrap(), 1);
    let fee: String =
        sqlx::query_scalar("SELECT bid_fee::text FROM shop.sale_event WHERE network = 'ethereum'")
            .fetch_one(&mut *conn)
            .await
            .unwrap();
    assert_eq!(fee, "10");
    index.rewind(&mut conn, 99).await.unwrap();
    let remaining: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM shop.name_purchase) + (SELECT count(*) FROM shop.sale_event)",
    )
    .fetch_one(&mut *conn)
    .await
    .unwrap();
    assert_eq!(remaining, 0);
    drop(conn);
    db.drop().await;
}

#[tokio::test]
async fn ethereum_sales_walk_withdraws_name_fees_on_a_reorg_and_publishes_the_refill() {
    use crate::market::ethereum_sales::{EthereumSalesIndex, NAME_CONTROLLER, TOPIC_NAME_BOUGHT};
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let ix = EthereumSalesIndex;
    let start = ix.start_block();
    let block = start + 20;
    let purchase = |amount: u64| {
        log_at(
            NAME_CONTROLLER,
            vec![TOPIC_NAME_BOUGHT.into(), topic_addr(ALICE), topic_addr(BOB)],
            (U256::from(amount), "alice").abi_encode_params(),
            block,
            1,
        )
    };
    let chain = FakeChain::new(block + 16, vec![purchase(100)], 1000, 10);
    chain.chain_id.store(1, Ordering::SeqCst);
    let url = serve(chain.clone()).await;
    let st = state_over(db.pool.clone(), &url, index_settings(100, 10), false);
    assert!(publish(&st, &url, &ix).await.published);
    assert_eq!(
        sales_index::status(&db.pool).await["ethereum"]["ready"],
        true
    );
    chain.fork(block, 0xee, vec![purchase(200)]);
    assert!(pass(&st, &url, &ix).await.reorg_rewound_to.is_some());
    assert_eq!(
        sales_index::status(&db.pool).await["ethereum"]["ready"],
        false
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM shop.name_purchase")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert!(publish(&st, &url, &ix).await.published);
    let fees: Vec<String> = sqlx::query_scalar("SELECT price::text FROM shop.name_purchase")
        .fetch_all(&db.pool)
        .await
        .unwrap();
    assert_eq!(fees, ["200"]);
    db.drop().await;
}

#[tokio::test]
async fn sales_accounting_publishes_one_common_prefix_and_rechecks_inputs_after_folding() {
    use crate::market::{ethereum_sales, sale_fold};
    use chrono::{Duration, Utc};
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let net = ChainNet::Ethereum;
    let block = nft_index::start_block(net) + 60;
    let chain = FakeChain::new(block + 14, vec![], 1000, 10);
    chain.chain_id.store(1, Ordering::SeqCst);
    let url = serve(chain.clone()).await;
    let st = state_over(db.pool.clone(), &url, index_settings(100, 10), false);
    let rpc = Rpc::at(&st.http, &url);
    for (name, tip) in [
        ("nft", block + 1),
        ("sales", block),
        ("trade_events", block + 2),
    ] {
        sqlx::query("INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,last_block_hash,
              full_history,folded_block,folded_at,sale_input_version,sale_verified_through,sale_covered_rewinds)
            VALUES ($1,'ethereum',1,1,$2,$3,true,$2,timezone('UTC',now()),1,$2,0)")
            .bind(name).bind(tip as i64).bind(chain.hash_of(tip)).execute(&db.pool).await.unwrap();
    }
    let price: U256 = U256::from(1u8) << 190;
    let log = log_at(
        ethereum_sales::BURNING_NAME_CONTROLLER,
        vec![
            ethereum_sales::TOPIC_NAME_BOUGHT.into(),
            topic_addr(ALICE),
            topic_addr(BOB),
        ],
        (price, "alice").abi_encode_params(),
        block,
        8,
    );
    let mut conn = db.pool.acquire().await.unwrap();
    ethereum_sales::EthereumSalesIndex
        .write(
            &mut conn,
            &[log],
            &HashMap::from([(block as i64, 1700000000)]),
        )
        .await
        .unwrap();
    drop(conn);
    sale_fold::pass(&db.pool, &rpc, net).await.unwrap();
    let fees: (String, String, i64) =
        sqlx::query_as("SELECT name_fees::text,burn_fees::text,sales FROM shop.sale_analytics_day")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(fees, (price.to_string(), price.to_string(), 0));
    let p: (i64,String,String,String,i64) = sqlx::query_as(
        "SELECT m.block_number,m.block_hash,p.gross::text,p.payer,(SELECT count(*) FROM shop.sale_asset)
         FROM shop.sale_publication m JOIN shop.sale_payment p USING (network)")
        .fetch_one(&db.pool).await.unwrap();
    assert_eq!(
        p,
        (
            block as i64,
            chain.hash_of(block),
            price.to_string(),
            ALICE.into(),
            0
        )
    );
    assert!(
        sale_fold::snapshot(&db.pool, &[net], Utc::now())
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        sale_fold::snapshot(&db.pool, &ChainNet::ALL, Utc::now())
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        sale_fold::snapshot(&db.pool, &[net], Utc::now() + Duration::minutes(31))
            .await
            .unwrap()
            .is_none()
    );
    // The walk can advance between folds while the recent common prefix stands.
    sqlx::query("UPDATE shop.chain_cursor SET last_block=$1,last_block_hash=$2,sale_verified_through=$1 WHERE indexer='nft'")
        .bind(block as i64+5).bind(chain.hash_of(block+5)).execute(&db.pool).await.unwrap();
    assert!(
        sale_fold::snapshot(&db.pool, &[net], Utc::now())
            .await
            .unwrap()
            .is_some()
    );
    // Hold the second branch check after rows are staged and before input locks.
    // Exercise both a same-height repair and a rewind by an independent owner.
    for rewind in [false, true] {
        let warm = sale_fold::snapshot(&db.pool, &[net], Utc::now())
            .await
            .unwrap()
            .unwrap();
        let token = warm.generations.clone();
        assert!(warm.expires_at > Utc::now());
        drop(warm);
        let gate = Arc::new(tokio::sync::Semaphore::new(0));
        *chain.gate_at.lock() = Some((block, gate.clone()));
        let waiting = chain.probes_waiting.load(Ordering::SeqCst);
        let (pool, http, endpoint) = (db.pool.clone(), st.http.clone(), url.clone());
        let task =
            tokio::spawn(
                async move { sale_fold::pass(&pool, &Rpc::at(&http, &endpoint), net).await },
            );
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while chain.probes_waiting.load(Ordering::SeqCst) == waiting {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        if rewind {
            sqlx::query("UPDATE shop.chain_cursor SET last_block=last_block-10,folded_block=NULL,folded_at=NULL,
                         sale_history_revision=sale_history_revision+1,reorg_rewinds=reorg_rewinds+1 WHERE indexer='nft'")
                .execute(&db.pool).await.unwrap();
        } else {
            sqlx::query("UPDATE shop.chain_cursor SET sale_history_revision=sale_history_revision+1 WHERE indexer='nft'")
                .execute(&db.pool).await.unwrap();
        }
        *chain.gate_at.lock() = None;
        gate.add_permits(1);
        let error = task.await.unwrap().unwrap_err().to_string();
        assert!(
            error.contains(if rewind {
                "withdrawn"
            } else {
                "history changed"
            }),
            "{error}"
        );
        assert!(
            sale_fold::snapshot(&db.pool, &[net], Utc::now())
                .await
                .unwrap()
                .is_none()
        );
        if !rewind {
            sale_fold::pass(&db.pool, &rpc, net).await.unwrap();
            assert_ne!(
                sale_fold::snapshot(&db.pool, &[net], Utc::now())
                    .await
                    .unwrap()
                    .unwrap()
                    .generations,
                token
            );
        }
    }
    db.drop().await;
}

#[tokio::test]
async fn sales_accounting_replays_fees_in_log_order_and_removes_empty_reorg_groups() {
    use crate::market::{sale_fold, sale_terms};
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let net = ChainNet::Ethereum;
    let venue = nft_index::legacy_marketplaces(net)[0].0;
    let block = nft_index::start_block(net) + 15;
    let chain = FakeChain::new(block + 17, Vec::new(), 1_000, 100);
    chain.chain_id.store(1, Ordering::SeqCst);
    let url = serve(chain.clone()).await;
    let state = state_over(db.pool.clone(), &url, index_settings(100, 10), false);
    let rpc = Rpc::at(&state.http, &url);
    let token = listings::mana_token(net.chain_id()).unwrap();
    let sale_price: U256 = U256::from(1u8) << 190;
    let tx_hash = format!("0x{:064x}", 73);
    let asset = "0x1234000000000000000000000000000000000000";
    let market_event = |seller: &str, token_id: u64, log| {
        log_at(
            venue,
            vec![
                nft_index::TOPIC_ORDER_SUCCESSFUL.into(),
                topic_word(U256::from(token_id)),
                topic_addr(seller),
                topic_addr(BOB),
            ],
            (
                FixedBytes::<32>::repeat_byte(5),
                Address::from_str(asset).unwrap(),
                sale_price,
            )
                .abi_encode_params(),
            block,
            log,
        )
    };
    let fee_change = |rate: u64, index: u64| {
        log_at(
            venue,
            vec![format!(
                "{:#x}",
                keccak256("ChangedOwnerCutPerMillion(uint256)")
            )],
            U256::from(rate).abi_encode(),
            block,
            index,
        )
    };
    let transfer = |to: &str, amount: U256, index: u64| {
        log_at(
            token,
            vec![
                nft_index::TOPIC_TRANSFER.into(),
                topic_addr(BOB),
                topic_addr(to),
            ],
            amount.abi_encode(),
            block,
            index,
        )
    };
    for (fork, seller, id, time) in [(false, ALICE, 1, 86400_i64), (true, CARL, 2, 172800)] {
        if fork {
            chain.fork(block, 0xba, vec![]);
        }
        if fork {
            // An earlier execution survives even when two cursor revisions arrive before the fold.
            sqlx::query("INSERT INTO shop.sale_execution(network,block_number,log_index,tx_hash,venue,kind,block_time,receipt_read)
                VALUES('ethereum',$1,0,$2,$3,'order',0,true)")
                .bind(block as i64-1).bind(format!("0x{:064x}",5)).bind(venue).execute(&db.pool).await.unwrap();
            sqlx::query("UPDATE shop.chain_cursor SET sale_dirty_from=$1,sale_history_revision=sale_history_revision+1")
                .bind(block as i64+2).execute(&db.pool).await.unwrap();
        }
        let hash = chain.hash_of(block);
        let mut dbtx = db.pool.begin().await.unwrap();
        if fork {
            sqlx::query("DELETE FROM shop.nft_event")
                .execute(&mut *dbtx)
                .await
                .unwrap();
            sqlx::query("DELETE FROM shop.sale_fee_change")
                .execute(&mut *dbtx)
                .await
                .unwrap();
        }
        for name in ["nft", "sales", "trade_events"] {
            sqlx::query("INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,last_block_hash,
                full_history,folded_block,folded_at,sale_input_version,sale_verified_through,sale_covered_rewinds)
             VALUES ($1,'ethereum',1,1,$2,$3,true,$2,timezone('UTC',now()),1,$2,0)
             ON CONFLICT(indexer,network) DO UPDATE SET last_block_hash=excluded.last_block_hash,
                sale_dirty_from=least(chain_cursor.sale_dirty_from,excluded.last_block),sale_history_revision=chain_cursor.sale_history_revision+1")
                .bind(name).bind(block as i64).bind(&hash).execute(&mut *dbtx).await.unwrap();
        }
        sqlx::query("INSERT INTO shop.nft(network,contract_address,token_id,item_id,minted_block,minted_at,transferred_block,transferred_log,transferred_at)
          VALUES('ethereum',$1,$2::bigint,$2::bigint,1,1,1,0,1)")
            .bind(asset).bind(id as i64).execute(&mut *dbtx).await.unwrap();
        let fee1 = sale_price / U256::from(100u8);
        let fee2 = sale_price / U256::from(50u8);
        let mut logs = vec![
            fee_change(10_000, 0),
            transfer(DAVE, fee1, 1),
            transfer(seller, sale_price - fee1, 2),
            market_event(seller, id, 3),
        ];
        if !fork {
            logs.extend([
                fee_change(20_000, 4),
                transfer(DAVE, fee2, 5),
                transfer(seller, sale_price - fee2, 6),
                market_event(seller, id, 7),
            ]);
        }
        let mut receipt = Vec::new();
        for log in &mut logs {
            log.transaction_hash = Some(tx_hash.clone());
            let mut raw = log_json(log);
            raw["blockHash"] = json!(hash);
            receipt.push(raw);
            if log.topics[0] == nft_index::TOPIC_ORDER_SUCCESSFUL {
                let e = nft_index::decode_log(log, time).unwrap().unwrap();
                nft_index::insert_events(&mut dbtx, net, &[e])
                    .await
                    .unwrap();
            }
        }
        sale_terms::write(&mut dbtx, net, &logs).await.unwrap();
        dbtx.commit().await.unwrap();
        chain.receipts.lock().insert(
            tx_hash.clone(),
            json!({"status":"0x1","blockNumber":format!("0x{block:x}"),
            "blockHash":hash,"transactionHash":tx_hash,"logs":receipt}),
        );
        sale_fold::pass(&db.pool, &rpc, net).await.unwrap();
        if fork {
            assert!(
                sqlx::query_scalar::<_, bool>(
                    "SELECT receipt_read FROM shop.sale_execution WHERE block_number=$1"
                )
                .bind(block as i64 - 1)
                .fetch_one(&db.pool)
                .await
                .unwrap()
            );
            let firsts: Vec<(String, i64)> =
                sqlx::query_as("SELECT seller,first_day FROM shop.sale_first_purchase")
                    .fetch_all(&db.pool)
                    .await
                    .unwrap();
            assert_eq!(firsts, [(seller.to_string(), time)]);
        }
        let n = if fork { 1 } else { 2 };
        let fees = if fork { fee1 } else { fee1 + fee2 };
        let expected_volume = (sale_price * U256::from(n as u64)).to_string();
        let days:Vec<(i64,i64,String,String,i64)>=sqlx::query_as("SELECT day,sales,volume::text,venue_fees::text,unknown_splits FROM shop.sale_analytics_day")
            .fetch_all(&db.pool).await.unwrap();
        assert_eq!(
            days,
            [(time, n, expected_volume.clone(), fees.to_string(), 0)]
        );
        let items: Vec<(i64, String, String)> =
            sqlx::query_as("SELECT day,item_id::text,volume::text FROM shop.sale_item_day")
                .fetch_all(&db.pool)
                .await
                .unwrap();
        assert_eq!(items, [(time, id.to_string(), expected_volume.clone())]);
        let totals: Vec<(String, i64, String)> = sqlx::query_as(
            "SELECT address,sales,gross_sold::text FROM shop.sale_account_total WHERE sales>0",
        )
        .fetch_all(&db.pool)
        .await
        .unwrap();
        assert_eq!(totals, [(seller.into(), n, expected_volume)]);
        let per_fill: Vec<String> = sqlx::query_scalar(
            "SELECT amount::text FROM shop.sale_payout WHERE role='venue_fee' ORDER BY log_index",
        )
        .fetch_all(&db.pool)
        .await
        .unwrap();
        assert_eq!(
            per_fill,
            if fork {
                vec![fee1.to_string()]
            } else {
                vec![fee1.to_string(), fee2.to_string()]
            }
        );
    }
    db.drop().await;
}

#[tokio::test]
async fn sales_history_repair_resumes_at_the_next_window_and_discovers_store_issues_from_bought() {
    use crate::market::sale_history;
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let s = SalesIndex.start_block();
    let b = s + 10_020;
    let tip = b + 15;
    let hash = format!("0x{:064x}", 317);
    let mut one = sales_index::tests::issue_log(
        CARL,
        ALICE,
        v2_token(4, 1),
        4,
        sales_index::STORE_MATIC,
        b,
        0,
    );
    let mut two = sales_index::tests::issue_log(
        CARL,
        ALICE,
        v2_token(4, 2),
        4,
        sales_index::STORE_MATIC,
        b,
        2,
    );
    one.transaction_hash = Some(hash.clone());
    two.transaction_hash = Some(hash.clone());
    let buy1 = sales_index::tests::bought_log(&[(CARL, 4, 25, ALICE)], &hash, b, 1);
    let buy2 = sales_index::tests::bought_log(&[(CARL, 4, 63, ALICE)], &hash, b, 3);
    let mut malformed = two.clone();
    malformed.log_index = "0x4".into();
    malformed.data = "0x12".into();
    let orphan = sales_index::tests::bought_log(&[(CARL, 5, 17, BOB)], &hash, b, 5);
    let chain = FakeChain::new(
        tip + 64,
        vec![one, buy1, two, buy2, malformed, orphan],
        100_000,
        100,
    );
    let url = serve(chain.clone()).await;
    let mut settings = index_settings(100, 100_000);
    settings.sales_enabled = true;
    let st = state_over(db.pool.clone(), &url, settings.clone(), false);
    let anchor = chain.hash_of(tip);
    sqlx::query("INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,last_block_hash,folded_block,folded_at,full_history)
        VALUES ('sales','matic',137,$1,$2,$3,$2,timezone('UTC',now()),true)")
        .bind(s as i64).bind(tip as i64).bind(&anchor).execute(&db.pool).await.unwrap();
    let source = chain_walk::RpcSource::new(Rpc::at(&st.http, &url));
    assert_eq!(
        sale_history::pending(&db.pool, &SalesIndex).await.unwrap(),
        Some(s as i64)
    );
    for (expected, calls) in [(Some((s + 10_000) as i64), 0), (None, 3)] {
        let owner = chain_walk::test_owner(&db.pool, &SalesIndex, &settings).await;
        sale_history::repair(
            &db.pool,
            &source,
            &SalesIndex,
            &owner,
            Some((tip, &anchor)),
            100,
            &mut 100,
        )
        .await
        .unwrap();
        assert_eq!(
            sale_history::pending(&db.pool, &SalesIndex).await.unwrap(),
            expected
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM shop.store_sale_line")
                .fetch_one(&db.pool)
                .await
                .unwrap(),
            calls
        );
    }
    let pairs: Vec<(i32, i32, String)> = sqlx::query_as(
        "SELECT bought_log,issue_log,price::text FROM shop.store_sale_line WHERE issue_log IS NOT NULL ORDER BY bought_log",
    )
    .fetch_all(&db.pool)
    .await
    .unwrap();
    assert_eq!(pairs, [(1, 0, "25".into()), (3, 2, "63".into())]);
    assert!(sqlx::query_scalar::<_,bool>("SELECT issue_checked AND issue_log IS NULL FROM shop.store_sale_line WHERE bought_log=5")
        .fetch_one(&db.pool).await.unwrap());
    // A newly owed line at this height invalidates the old semantic revision,
    // even though the cursor never moved.
    sqlx::query("UPDATE shop.store_sale_line SET issue_log=NULL WHERE bought_log=3")
        .execute(&db.pool)
        .await
        .unwrap();
    let before: i64 = sqlx::query_scalar("SELECT sale_history_revision FROM shop.chain_cursor")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        sale_history::pending(&db.pool, &SalesIndex).await.unwrap(),
        Some(b as i64)
    );
    let owner = chain_walk::test_owner(&db.pool, &SalesIndex, &settings).await;
    sale_history::repair(
        &db.pool,
        &source,
        &SalesIndex,
        &owner,
        Some((tip, &anchor)),
        100,
        &mut 100,
    )
    .await
    .unwrap();
    let after: (i64, i64) =
        sqlx::query_as("SELECT sale_history_revision,last_block FROM shop.chain_cursor")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(after, (before + 1, tip as i64));
    // Prove the neighbouring independent walks on the same branch.
    for name in ["nft", "trade_events"] {
        sqlx::query("INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,last_block_hash,
            full_history,folded_block,folded_at,sale_input_version,sale_verified_through,sale_covered_rewinds)
            VALUES($1,'matic',137,1,$2,$3,true,$2,timezone('UTC',now()),1,$2,0)")
            .bind(name).bind(tip as i64).bind(&anchor).execute(&db.pool).await.unwrap();
    }
    *chain.head.lock() = tip + 5;
    let rpc = Rpc::at(&st.http, &url);
    crate::market::sale_fold::pass(&db.pool, &rpc, ChainNet::Matic)
        .await
        .unwrap();
    assert!(
        crate::market::sale_fold::snapshot(&db.pool, &[ChainNet::Matic], chrono::Utc::now())
            .await
            .unwrap()
            .is_some()
    );
    let canonical: (i64, i64, String) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM shop.sale_execution),
        (SELECT count(*) FROM shop.sale_asset),sum(gross)::text FROM shop.sale_payment",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(canonical, (3, 2, "105".into()));
    assert_eq!(
        sale_history::pending(&db.pool, &SalesIndex).await.unwrap(),
        None
    );
    db.drop().await;
}

#[tokio::test]
async fn sales_accounting_bundles_preserve_order_and_gross_once_per_currency() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let net = ChainNet::Ethereum;
    let b = nft_index::start_block(net) + 20;
    let chain = FakeChain::new(b + 12, vec![], 1000, 100);
    chain.chain_id.store(1, Ordering::SeqCst);
    let url = serve(chain.clone()).await;
    let st = state_over(db.pool.clone(), &url, index_settings(100, 10), false);
    let rpc = Rpc::at(&st.http, &url);
    for name in ["nft", "sales", "trade_events"] {
        sqlx::query("INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,last_block_hash,
          full_history,folded_block,folded_at,sale_input_version,sale_verified_through,sale_covered_rewinds)
          VALUES($1,'ethereum',1,1,$2,$3,true,$2,timezone('UTC',now()),1,$2,0)")
          .bind(name).bind(b as i64).bind(chain.hash_of(b)).execute(&db.pool).await.unwrap();
    }
    let venue = trade_events::marketplaces(net)[0].0;
    let mana = listings::mana_token(net.chain_id()).unwrap();
    let asset = |ty: &str, contract: &str, value: &str, beneficiary: &str| json!({"assetType":ty,"contractAddress":contract,"value":value,"beneficiary":beneficiary,"extra":"0x"});
    let mut log = trade_events::tests::traded_log(b, 8, venue, BOB, ALICE, "0x01", 1, false);
    let hash = format!("0x{:064x}", 431);
    log.transaction_hash = Some(hash.clone());
    let mut event = trade_events::decode_log(&log, 86400).unwrap().unwrap();
    event.signer = Some(ALICE.into());
    event.caller = BOB.into();
    event.assets = Some(
        json!({"sent":[asset("3",DAVE,"12",CARL),asset("3",DAVE,"13",BOB)],
        "received":[asset("1",mana,"115",DAVE),asset("2",mana,"8",ALICE),asset("1",CARL,"99",ALICE),asset("2",DAVE,"10",ALICE)],"uses":"1"}),
    );
    let mut c = db.pool.acquire().await.unwrap();
    trade_events::insert_events(&mut c, net, &[event])
        .await
        .unwrap();
    drop(c);
    crate::market::sale_fold::pass(&db.pool, &rpc, net)
        .await
        .unwrap();
    let rows:Vec<(String,String,String,String,Option<String>)>=sqlx::query_as("SELECT public_id,seller,buyer,token_id::text,price::text FROM shop.sale_asset ORDER BY ordinal")
        .fetch_all(&db.pool).await.unwrap();
    assert_eq!(
        rows,
        [
            (
                format!("ethereum-{hash}-0-0"),
                ALICE.into(),
                CARL.into(),
                "12".into(),
                None
            ),
            (
                format!("ethereum-{hash}-0-1"),
                ALICE.into(),
                BOB.into(),
                "13".into(),
                None
            )
        ]
    );
    let amounts:Vec<(String,String,i64,i64,i64)>=sqlx::query_as(
        "SELECT currency,volume::text,sales,unallocated_executions,unknown_splits FROM shop.sale_analytics_day ORDER BY currency")
        .fetch_all(&db.pool).await.unwrap();
    assert_eq!(
        amounts,
        [
            (mana.into(), "123".into(), 0, 1, 2),
            (CARL.into(), "99".into(), 0, 1, 1),
            ("usd".into(), "10".into(), 0, 1, 1)
        ]
    );
    let legs: Vec<(String, String)> =
        sqlx::query_as("SELECT payer,beneficiary FROM shop.sale_payment ORDER BY ordinal")
            .fetch_all(&db.pool)
            .await
            .unwrap();
    assert_eq!(
        legs,
        [
            (BOB.into(), DAVE.into()),
            (BOB.into(), ALICE.into()),
            (BOB.into(), ALICE.into()),
            (BOB.into(), ALICE.into())
        ]
    );
    db.drop().await;
}
