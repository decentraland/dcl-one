use super::*;

#[tokio::test]
async fn sales_replay_backfills_only_the_transaction_hash_on_a_transfer() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let mut conn = db.pool.acquire().await.unwrap();
    let log = nft_index::tests::transfer_log(CARL, ALICE, BOB, U256::from(12), 100, 2);
    let mut row = nft_index::decode_log(&log, 0).unwrap().unwrap();
    row.tx_hash = None;
    nft_index::insert_events(&mut conn, ChainNet::Matic, std::slice::from_ref(&row))
        .await
        .unwrap();
    row.tx_hash = Some(format!("0x{:064x}", 2));
    row.block_time = 200;
    row.to_address = Some(DAVE.into());
    for count in [1, 0] {
        assert_eq!(
            nft_index::insert_events(&mut conn, ChainNet::Matic, std::slice::from_ref(&row))
                .await
                .unwrap(),
            count
        );
    }
    let kept: (String, i64, String) =
        sqlx::query_as("SELECT tx_hash,block_time,to_address FROM shop.nft_event")
            .fetch_one(&mut *conn)
            .await
            .unwrap();
    assert_eq!(kept, (row.tx_hash.unwrap(), 0, BOB.into()));
    drop(conn);
    db.drop().await;
}

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
        sales_index::tests::bid_accepted_log(ALICE, U256::from(4), BOB, CARL, 0x11, 20, 100, 4);
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
    assert_eq!(fee, "0");
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
    let earlier = block as i64 - 10;
    sqlx::query("INSERT INTO shop.sale_execution(network,block_number,log_index,tx_hash,venue,kind,block_time,receipt_read)
        VALUES('ethereum',$1,1,$2,$3,'order',86400,true)").bind(earlier).bind(format!("0x{:064x}",97)).bind(ALICE).execute(&db.pool).await.unwrap();
    sqlx::query("INSERT INTO shop.nft_contract(network,contract_address,walked_from_block,backfill_from_block,backfilled_to_block)
        VALUES('ethereum',$1,$2,$2::bigint-2,$2::bigint-1)").bind(BOB).bind(block as i64).execute(&db.pool).await.unwrap();
    assert!(
        sale_fold::snapshot(&db.pool, &[net], Utc::now())
            .await
            .unwrap()
            .is_none()
    );
    sale_fold::pass(&db.pool, &rpc, net).await.unwrap();
    assert!(
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT FROM shop.sale_execution WHERE block_number=$1)"
        )
        .bind(earlier)
        .fetch_one(&db.pool)
        .await
        .unwrap()
    );
    // A second late arrival must preserve a confirmed receipt between the
    // old discovery boundary and the new one.
    let between = block as i64 - 2;
    sqlx::query("INSERT INTO shop.sale_execution(network,block_number,log_index,tx_hash,venue,kind,block_time,receipt_read)
        VALUES('ethereum',$1,2,$2,$3,'order',86400,true)").bind(between).bind(format!("0x{:064x}",98)).bind(ALICE).execute(&db.pool).await.unwrap();
    sqlx::query("INSERT INTO shop.nft_contract(network,contract_address,walked_from_block,backfill_from_block,backfilled_to_block)
        VALUES('ethereum',$1,$2,$2::bigint-1,$2::bigint-1)").bind(DAVE).bind(block as i64).execute(&db.pool).await.unwrap();
    sale_fold::pass(&db.pool, &rpc, net).await.unwrap();
    assert!(
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT FROM shop.sale_execution WHERE block_number=$1 AND receipt_read)"
        )
        .bind(between)
        .fetch_one(&db.pool)
        .await
        .unwrap()
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
    sqlx::query("INSERT INTO shop.nft_contract(network,contract_address,walked_from_block) VALUES($1,$2,$3)")
        .bind(net.db_name()).bind(asset).bind(block as i64).execute(&db.pool).await.unwrap();
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
    // The canonical receipt can be complete while payment attribution is not.
    // Two identical transfer sequences for one venue event have no unique split.
    let ambiguous_hash = format!("0x{:064x}", 77);
    let mut order = market_event(CARL, 2, 15);
    order.transaction_hash = Some(ambiguous_hash.clone());
    let fee = sale_price / U256::from(100u8);
    let mut c = db.pool.acquire().await.unwrap();
    nft_index::insert_events(
        &mut c,
        net,
        &[nft_index::decode_log(&order, 172800).unwrap().unwrap()],
    )
    .await
    .unwrap();
    drop(c);
    let receipt: Vec<_> = [
        transfer(DAVE, fee, 10),
        transfer(CARL, sale_price - fee, 11),
        transfer(DAVE, fee, 12),
        transfer(CARL, sale_price - fee, 13),
        order,
    ]
    .iter()
    .map(|log| {
        let mut v = log_json(log);
        v["blockHash"] = json!(chain.hash_of(block));
        v["transactionHash"] = json!(ambiguous_hash);
        v
    })
    .collect();
    chain.receipts.lock().insert(
        ambiguous_hash.clone(),
        json!({"status":"0x1","blockNumber":format!("0x{block:x}"),
        "blockHash":chain.hash_of(block),"transactionHash":ambiguous_hash,"logs":receipt}),
    );
    sqlx::query("UPDATE shop.chain_cursor SET sale_dirty_from=$1, sale_history_revision=sale_history_revision+1 WHERE indexer='nft'")
        .bind(block as i64).execute(&db.pool).await.unwrap();
    sale_fold::pass(&db.pool, &rpc, net).await.unwrap();
    let observed:(bool,bool,i64)=sqlx::query_as("SELECT receipt_read,settled,(SELECT count(*) FROM shop.sale_payout WHERE log_index=15)
        FROM shop.sale_execution JOIN shop.sale_payment USING(network,block_number,log_index) WHERE tx_hash=$1")
        .bind(ambiguous_hash).fetch_one(&db.pool).await.unwrap();
    assert_eq!(observed, (true, false, 0));
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
    seed_collection(
        &db.pool,
        "matic",
        CARL,
        s,
        "factory-collection",
        "Factory collection",
    )
    .await;
    // A catalogue entry created after the purchase cannot attest to its issuer.
    seed_collection(
        &db.pool,
        "matic",
        DAVE,
        b + 1,
        "future-collection",
        "Future collection",
    )
    .await;
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

    let mut malformed = two.clone();
    malformed.log_index = "0x4".into();
    malformed.data = "0x12".into();
    let orphan = sales_index::tests::bought_log(&[(CARL, 5, 17, BOB)], &hash, b, 5);
    // An ordinary catalogue exclusion in both a mixed and a standalone purchase.
    let mixed =
        sales_index::tests::bought_log(&[(CARL, 4, 63, ALICE), (DAVE, 8, 13, ALICE)], &hash, b, 3);
    let outside = sales_index::tests::bought_log(&[(DAVE, 9, 14, BOB)], &hash, b, 7);
    let changed = log_at(
        CARL,
        vec![
            format!(
                "{:#x}",
                alloy_primitives::keccak256("CreatorshipTransferred(address,address)")
            ),
            topic_addr(BOB),
            topic_addr(DAVE),
        ],
        vec![],
        b,
        6,
    );
    let chain = FakeChain::new(
        tip + 64,
        vec![one, buy1, two, mixed, malformed, orphan, changed, outside],
        100_000,
        100,
    );
    chain.strip_timestamps.store(true, Ordering::SeqCst);
    chain
        .creators_at_hash
        .lock()
        .insert((CARL.into(), chain.hash_of(b)), DAVE.into());
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
    // Multiple address chunks share one archive budget, even if a later
    // chunk would ordinarily issue another request.
    let before_reads = chain.get_logs.load(Ordering::SeqCst);
    let groups = [chain_walk::LogGroup {
        addresses: vec![CARL.into(), DAVE.into()],
        topics: json!([[sales_index::TOPIC_ISSUE]]),
    }];
    let budget_error = chain_walk::LogSource::archive_window(&source, s, s, &groups, &mut 1, 1)
        .await
        .unwrap_err();
    assert!(matches!(
        budget_error,
        crate::chain_index::rpc::RpcError::Budget { requests: 1, .. }
    ));
    assert_eq!(chain.get_logs.load(Ordering::SeqCst) - before_reads, 1);
    let owner = chain_walk::test_owner(&db.pool, &SalesIndex, &settings).await;
    let charged = sale_history::repair(
        &db.pool,
        &source,
        &SalesIndex,
        &owner,
        Some((tip, &anchor)),
        2,
        &mut 100,
    )
    .await
    .unwrap();
    assert_eq!(charged, chain_walk::Spent::default());
    for (expected, calls) in [(Some((s + 10_000) as i64), 0), (None, 5)] {
        let owner = chain_walk::test_owner(&db.pool, &SalesIndex, &settings).await;
        sale_history::repair(
            &db.pool,
            &source,
            &SalesIndex,
            &owner,
            Some((tip, &anchor)),
            150,
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
    sqlx::query("UPDATE shop.store_sale_line SET issue_log=NULL WHERE bought_log=3 AND ordinal=0")
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
            .is_none(),
        "a complete factory walk must distinguish a late collection from an unrelated issuer"
    );
    sqlx::query("INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,last_block_hash,folded_block,folded_at,full_history)
        VALUES('collections','matic',137,1,$1,$2,$1,timezone('UTC',now()),true)")
        .bind(tip as i64).bind(&anchor).execute(&db.pool).await.unwrap();
    crate::market::sale_fold::pass(&db.pool, &rpc, ChainNet::Matic)
        .await
        .unwrap();
    assert!(
        crate::market::sale_fold::snapshot(&db.pool, &[ChainNet::Matic], chrono::Utc::now())
            .await
            .unwrap()
            .is_some()
    );
    let generation: uuid::Uuid = sqlx::query_scalar("SELECT generation FROM shop.sale_publication")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    let proof: i64 = sqlx::query_scalar(
        "SELECT sale_verified_through FROM shop.chain_cursor WHERE indexer='collections'",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(proof, b as i64);
    let net = ChainNet::Matic;
    for (old, target, ready) in [(tip, b + 1, true), (b + 1, b - 1, false)] {
        let mut tx = db.pool.begin().await.unwrap();
        chain_walk::rewind(
            &mut tx,
            "collections",
            net,
            (old as i64, Some(&chain.hash_of(old))),
            target as i64,
            Some((&chain.hash_of(target), time_of(target))),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        // A tip rewind withdraws the factory cursor. Only a rewind reaching
        // the excluded purchase withdraws the sales proof.
        assert_eq!(
            crate::market::sale_fold::snapshot(&db.pool, &[net], chrono::Utc::now())
                .await
                .unwrap()
                .is_some(),
            ready
        );
        crate::market::sale_fold::pass(&db.pool, &rpc, net)
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, uuid::Uuid>("SELECT generation FROM shop.sale_publication")
                .fetch_one(&db.pool)
                .await
                .unwrap(),
            generation
        );
    }
    sqlx::query(
        "UPDATE shop.chain_cursor SET last_block=$1,last_block_hash=$2,
         folded_block=$1,folded_at=timezone('UTC',now()) WHERE indexer='collections'",
    )
    .bind(tip as i64)
    .bind(&anchor)
    .execute(&db.pool)
    .await
    .unwrap();
    crate::market::sale_fold::pass(&db.pool, &rpc, net)
        .await
        .unwrap();
    assert!(
        crate::market::sale_fold::snapshot(&db.pool, &[net], chrono::Utc::now())
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        sqlx::query_scalar::<_, uuid::Uuid>("SELECT generation FROM shop.sale_publication")
            .fetch_one(&db.pool)
            .await
            .unwrap(),
        generation,
        "revalidation preserves settled history"
    );
    let canonical: (i64, i64, String) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM shop.sale_execution),
        (SELECT count(*) FROM shop.sale_asset),sum(gross)::text FROM shop.sale_payment",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(canonical, (3, 2, "105".into()));
    let excluded: (bool, bool, bool, bool, String) = sqlx::query_as(
        "SELECT EXISTS(SELECT FROM shop.sale_execution WHERE log_index=7),
            EXISTS(SELECT FROM shop.sale_item_day WHERE contract_address=$1),
            EXISTS(SELECT FROM shop.sale_first_purchase WHERE contract_address=$1),
            EXISTS(SELECT FROM shop.sale_account_day WHERE address=$1),
            (SELECT sum(volume)::text FROM shop.sale_analytics_day)",
    )
    .bind(DAVE)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(excluded, (false, false, false, false, "105".into()));

    let sellers: Vec<String> =
        sqlx::query_scalar("SELECT seller FROM shop.sale_asset ORDER BY log_index")
            .fetch_all(&db.pool)
            .await
            .unwrap();
    assert_eq!(
        sellers,
        [BOB, BOB],
        "both mints preceded the same-block change to DAVE"
    );
    assert_eq!(
        sale_history::pending(&db.pool, &SalesIndex).await.unwrap(),
        None
    );
    // Historical discovery behind that attested prefix reopens the line.
    sqlx::query(
        "UPDATE shop.collections SET created_at=$2 WHERE network='matic' AND contract_address=$1",
    )
    .bind(DAVE)
    .bind(time_of(b))
    .execute(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        sale_history::pending(&db.pool, &SalesIndex).await.unwrap(),
        Some(b as i64)
    );
    assert!(
        crate::market::sale_fold::snapshot(&db.pool, &[net], chrono::Utc::now())
            .await
            .unwrap()
            .is_none()
    );
    db.drop().await;
}

#[tokio::test]
async fn sales_accounting_keeps_unallocated_evidence_out_of_public_totals() {
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
    sqlx::query("INSERT INTO shop.nft_contract(network,contract_address,walked_from_block) VALUES($1,$2,$3)")
        .bind(net.db_name()).bind(DAVE).bind(b as i64).execute(&db.pool).await.unwrap();
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
    // Source rows remain available to the primary indexes even when their
    // NFT contract is outside the economic catalogue.
    let mut outside = event.clone();
    outside.log_index = 9;
    outside.tx_hash = format!("0x{:064x}", 432);
    outside.assets = Some(json!({"sent":[asset("4", CARL, "1", BOB)],
        "received":[asset("1",mana,"500",ALICE)],"uses":"1"}));
    let mut mixed = outside.clone();
    mixed.log_index = 12;
    mixed.tx_hash = format!("0x{:064x}", 433);
    mixed.assets = Some(
        json!({"sent":[asset("3",DAVE,"2",BOB),asset("3",CARL,"4",BOB)],
        "received":[asset("1",mana,"255",ALICE)],"uses":"1"}),
    );
    let mut bundled = mixed.clone();
    bundled.log_index = 13;
    bundled.tx_hash = format!("0x{:064x}", 434);
    bundled.assets.as_mut().unwrap()["sent"][1]["contractAddress"] = json!(DAVE);
    sqlx::query("INSERT INTO shop.nft_event(network,block_number,log_index,block_time,kind,emitter,contract_address,token_id,from_address,to_address,amount,tx_hash)
        VALUES('ethereum',$1,10,86400,'order_successful',$2,$3,1,$3,$4,500,$5)")
        .bind(b as i64).bind(nft_index::legacy_marketplaces(net)[0].0).bind(CARL).bind(BOB).bind(&outside.tx_hash)
        .execute(&mut *c).await.unwrap();
    sqlx::query("INSERT INTO shop.sale_event(network,block_number,log_index,block_time,kind,emitter,contract_address,token_id,bid_id,buyer,seller,price,tx_hash)
        VALUES('ethereum',$1,11,86400,'bid_accepted',$2,$3,1,$5,$4,$3,500,$5)")
        .bind(b as i64).bind(crate::market::ethereum_sales::ERC721_BID).bind(CARL).bind(BOB).bind(&outside.tx_hash)
        .execute(&mut *c).await.unwrap();
    trade_events::insert_events(&mut c, net, &[event, outside, mixed, bundled])
        .await
        .unwrap();
    drop(c);
    crate::market::sale_fold::pass(&db.pool, &rpc, net)
        .await
        .unwrap();
    let executions: Vec<i32> =
        sqlx::query_scalar("SELECT log_index FROM shop.sale_execution ORDER BY log_index")
            .fetch_all(&db.pool)
            .await
            .unwrap();
    assert_eq!(
        executions,
        [8, 13],
        "only catalogue-only executions are normalized"
    );
    let counts: (i64,i64,i64,i64,i64,i64) = sqlx::query_as("SELECT
         (SELECT count(*) FROM shop.trade_event), (SELECT count(*) FROM shop.sale_asset WHERE price>0),
         (SELECT count(*) FROM shop.sale_payment),(SELECT count(*) FROM shop.sale_analytics_day),
         (SELECT count(*) FROM shop.sale_account_day),(SELECT count(*) FROM shop.sale_item_day)")
        .fetch_one(&db.pool).await.unwrap();
    assert_eq!(counts, (4, 0, 5, 0, 0, 0));
    sqlx::query(
        "INSERT INTO shop.mirror_state(mirror,network,source_url,last_complete_pass_at)
        VALUES('items','ethereum','test',timezone('UTC',now()))",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let mut settings = index_settings(100, 10);
    settings.sales_enabled = true;
    let app = crate::market::market_router(state_over(db.pool.clone(), &url, settings, false));
    let (code, page) = get_json(&app, "/v1/sales?network=ETHEREUM").await;
    assert_eq!(code, StatusCode::OK, "{page}");
    assert_eq!(page["total"], 0);
    let (code, proceeds) = get_json(
        &app,
        &format!("/v1/proceeds/summary?seller={ALICE}&network=ETHEREUM"),
    )
    .await;
    assert_eq!(code, StatusCode::OK, "{proceeds}");
    assert_eq!(proceeds["data"]["sales"], 0);
    assert_eq!(proceeds["data"]["netWei"], "0");

    // A single catalogue NFT has an exact price when several payments in
    // the same currency cross from the opposite side.
    let mut bid = trade_events::decode_log(&log, 86401).unwrap().unwrap();
    bid.block_number = (b + 1) as i64;
    bid.log_index = 2;
    bid.tx_hash = format!("0x{:064x}", 435);
    bid.signer = Some(BOB.into());
    bid.caller = ALICE.into();
    bid.assets = Some(
        json!({"sent":[asset("1",mana,"10",DAVE),asset("1",mana,"15",CARL)],
        "received":[asset("3",DAVE,"99",BOB)],"uses":"1"}),
    );
    let mut c = db.pool.acquire().await.unwrap();
    trade_events::insert_events(&mut c, net, &[bid])
        .await
        .unwrap();
    drop(c);
    *chain.head.lock() = b + 13;
    sqlx::query("UPDATE shop.chain_cursor SET last_block=$1,last_block_hash=$2,folded_block=$1,sale_verified_through=$1")
        .bind((b+1) as i64).bind(chain.hash_of(b+1)).execute(&db.pool).await.unwrap();
    crate::market::sale_fold::pass(&db.pool, &rpc, net)
        .await
        .unwrap();
    let (code, page) = get_json(
        &app,
        &format!("/v1/sales?network=ETHEREUM&type=bid&seller={DAVE}"),
    )
    .await;
    assert_eq!(code, StatusCode::OK, "{page}");
    assert_eq!(page["total"], 1);
    assert_eq!(page["data"][0]["price"], "25");
    assert_eq!(page["data"][0]["buyer"], BOB);
    let endpoint = format!("/v1/proceeds?seller={DAVE}&network=ETHEREUM");
    assert_eq!(
        get_json(&app, &endpoint).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    sqlx::query("WITH paid AS (UPDATE shop.sale_payment SET settled=true WHERE network='ethereum' AND block_number=$1
        RETURNING network,block_number,log_index,ordinal,gross)
        INSERT INTO shop.sale_payout(network,block_number,log_index,payment_ordinal,ordinal,role,recipient,amount)
        SELECT network,block_number,log_index,ordinal,0,'seller',CASE WHEN ordinal=0 THEN $2 ELSE $3 END,gross FROM paid")
        .bind((b+1) as i64).bind(DAVE).bind(CARL).execute(&db.pool).await.unwrap();
    let (code, proceeds) = get_json(&app, &endpoint).await;
    assert_eq!(code, StatusCode::OK, "{proceeds}");
    assert_eq!(proceeds["total"], 1);
    assert_eq!(proceeds["data"][0]["netWei"], "25");
    db.drop().await;
}

#[tokio::test]
async fn sales_delayed_receipt_corrects_checkout_buyer_and_first_purchase_day() {
    use crate::market::sale_fold;
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    const CHECKOUT: &str = "0xed038688ecf1193f8d9717eb3930f0bf0d745cb4";
    let net = ChainNet::Matic;
    let b = nft_index::start_block(net) + 12;
    let chain = FakeChain::new(b + 5, vec![], 1000, 100);
    let url = serve(chain.clone()).await;
    let st = state_over(db.pool.clone(), &url, index_settings(100, 10), false);
    let rpc = Rpc::at(&st.http, &url);
    for name in ["nft", "sales", "trade_events"] {
        sqlx::query("INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,last_block_hash,
            full_history,folded_block,folded_at,sale_input_version,sale_verified_through,sale_covered_rewinds)
            VALUES($1,'matic',137,1,$2,$3,true,$2,timezone('UTC',now()),1,$2,0)")
            .bind(name).bind(b as i64+1).bind(chain.hash_of(b+1)).execute(&db.pool).await.unwrap();
    }
    let venue = trade_events::marketplaces(net)[0].0;
    let mana = listings::mana_token(net.chain_id()).unwrap();
    sqlx::query(
        "INSERT INTO shop.sale_fee_change(network,venue,field,block_number,log_index,value)
        VALUES('matic',$1,'collector',1,0,$2),('matic',$1,'fee_ppm',1,1,'50000')",
    )
    .bind(venue)
    .bind(DAVE)
    .execute(&db.pool)
    .await
    .unwrap();
    let split = format!(
        "0x{}",
        hex::encode((true, U256::ZERO, Vec::<Address>::new()).abi_encode_params())
    );
    sqlx::query("INSERT INTO shop.nft_contract(network,contract_address,walked_from_block) VALUES($1,$2,$3)")
        .bind(net.db_name()).bind(DAVE).bind((b-2) as i64).execute(&db.pool).await.unwrap();
    let tx_hash = |block: u64| format!("0x{block:064x}");
    let traded = |block| {
        let mut log =
            trade_events::tests::traded_log(block, 9, venue, BOB, ALICE, "0x01", 1, false);
        log.transaction_hash = Some(tx_hash(block));
        log
    };
    for (block, day) in [(b, 86400), (b + 1, 172800)] {
        let mut e = trade_events::decode_log(&traded(block), day)
            .unwrap()
            .unwrap();
        e.signer = Some(ALICE.into());
        e.caller = BOB.into();
        e.assets = Some(
            json!({"sent":[{"assetType":"4","contractAddress":DAVE,"value":"3","beneficiary":CHECKOUT,"extra":"0x"}],
            "received":[{"assetType":"2","contractAddress":mana,"value":"500","beneficiary":ALICE,"extra":split}]}),
        );
        let mut c = db.pool.acquire().await.unwrap();
        trade_events::insert_events(&mut c, net, &[e])
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO shop.nft_event(network,block_number,log_index,block_time,kind,emitter,contract_address,token_id,from_address,to_address)
        VALUES('matic',$1,2,86400,'transfer',$2,$2,123,$3,$5),
              ('matic',$1,10,86400,'transfer',$2,$2,123,$3,$4),
              ('matic',$1,12,86400,'transfer',$2,$2,123,$4,$5)")
        .bind(b as i64).bind(DAVE).bind(CHECKOUT).bind(CARL).bind(BOB).execute(&db.pool).await.unwrap();
    let barter = b - 1;
    let mut gift = trade_events::decode_log(&traded(barter), 86400)
        .unwrap()
        .unwrap();
    gift.signer = Some(ALICE.into());
    gift.caller = BOB.into();
    gift.assets = Some(
        json!({"sent":[{"assetType":"4","contractAddress":DAVE,"value":"3","beneficiary":CARL,"extra":"0x"}],
        "received":[{"assetType":"3","contractAddress":DAVE,"value":"24","beneficiary":ALICE,"extra":"0x"}]}),
    );
    let mut c = db.pool.acquire().await.unwrap();
    let swap_block = barter - 1;
    let mut swap = trade_events::decode_log(&traded(swap_block), 86400)
        .unwrap()
        .unwrap();
    swap.signer = Some(ALICE.into());
    swap.caller = BOB.into();
    swap.assets = Some(json!({
        "sent":[{"assetType":"3","contractAddress":DAVE,"value":"11","beneficiary":BOB,"extra":"0x"}],
        "received":[{"assetType":"3","contractAddress":DAVE,"value":"24","beneficiary":ALICE,"extra":"0x"}]
    }));
    trade_events::insert_events(&mut c, net, &[gift, swap])
        .await
        .unwrap();
    drop(c);
    let barter_logs: Vec<_> = [
        sales_index::tests::issue_log(DAVE, CARL, U256::from(122), 3, venue, barter, 3),
        traded(barter),
    ]
    .iter()
    .map(|l| {
        let mut v = log_json(l);
        v["blockHash"] = json!(chain.hash_of(barter));
        v["transactionHash"] = json!(tx_hash(barter));
        v
    })
    .collect();
    chain.receipts.lock().insert(tx_hash(barter),json!({"status":"0x1","blockNumber":format!("0x{barter:x}"),"blockHash":chain.hash_of(barter),"transactionHash":tx_hash(barter),"logs":barter_logs}));
    let firsts = || {
        sqlx::query_as::<_, (String, i64)>(
            "SELECT buyer,first_day FROM shop.sale_first_purchase ORDER BY first_day",
        )
        .fetch_all(&db.pool)
    };
    sale_fold::pass(&db.pool, &rpc, net).await.unwrap();
    assert_eq!(firsts().await.unwrap(), [(CHECKOUT.into(), 86400)]);
    let barter_fact: (bool, String, i64) = sqlx::query_as(
        "SELECT e.receipt_read,a.token_id::text,
        (SELECT count(*) FROM shop.sale_payment WHERE block_number=$1)
        FROM shop.sale_execution e JOIN shop.sale_asset a USING(network,block_number,log_index)
        WHERE e.block_number=$1 AND a.kind='mint'",
    )
    .bind(barter as i64)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(barter_fact, (true, "122".into(), 0));
    // The swap has no receipt in the RPC fixture. Its event already supplied
    // both token IDs, so the first fold completes it without a receipt request.
    let swap: (bool, bool, Vec<String>) = sqlx::query_as(
        "SELECT e.receipt_read,e.receipt_attempted_at IS NULL,
                ARRAY(SELECT a.token_id::text FROM shop.sale_asset a
                      WHERE (a.network,a.block_number,a.log_index)=(e.network,e.block_number,e.log_index)
                      ORDER BY a.ordinal)
         FROM shop.sale_execution e WHERE e.network='matic' AND e.block_number=$1",
    )
    .bind(swap_block as i64)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(swap, (true, true, vec!["11".into(), "24".into()]));
    // Older folds can leave the same swap queued.
    sqlx::query("UPDATE shop.sale_execution SET receipt_read=false WHERE network='matic' AND block_number=$1")
        .bind(swap_block as i64).execute(&db.pool).await.unwrap();
    let issue = sales_index::tests::issue_log(DAVE, CHECKOUT, U256::from(123), 3, venue, b, 3);
    let mut malformed = issue.clone();
    malformed.address = BOB.into();
    malformed.data = "0x01".into();
    malformed.log_index = "0x1".into();
    let transfer = |to: &str, amount: u64, index| {
        log_at(
            mana,
            vec![
                nft_index::TOPIC_TRANSFER.into(),
                topic_addr(BOB),
                topic_addr(to),
            ],
            U256::from(amount).abi_encode(),
            b,
            index,
        )
    };
    let receipt = [
        malformed,
        issue,
        transfer(DAVE, 25, 5),
        transfer(ALICE, 475, 6),
        traded(b),
    ]
    .iter()
    .map(|l| {
        let mut v = log_json(l);
        v["blockHash"] = json!(chain.hash_of(b));
        v["transactionHash"] = json!(tx_hash(b));
        v
    })
    .collect::<Vec<_>>();
    chain.receipts.lock().insert(
        tx_hash(b),
        json!({"status":"0x1","transactionHash":tx_hash(b),
        "blockNumber":format!("0x{b:x}"),"blockHash":chain.hash_of(b),"logs":receipt}),
    );
    sqlx::query("UPDATE shop.sale_execution SET receipt_attempted_at=timezone('UTC',now())-interval '6 minutes' WHERE block_number=$1")
        .bind(b as i64).execute(&db.pool).await.unwrap();
    sale_fold::pass(&db.pool, &rpc, net).await.unwrap();
    assert_eq!(
        firsts().await.unwrap(),
        [(CARL.into(), 86400), (CHECKOUT.into(), 172800)]
    );
    assert!(
        sqlx::query_scalar::<_, bool>(
            "SELECT receipt_read AND receipt_attempted_at IS NULL FROM shop.sale_execution
             WHERE network='matic' AND block_number=$1",
        )
        .bind(swap_block as i64)
        .fetch_one(&db.pool)
        .await
        .unwrap()
    );
    let paid:(bool,String,String,String)=sqlx::query_as("SELECT p.settled,p.gross::text,a.token_id::text,a.beneficiary FROM shop.sale_asset a
        JOIN shop.sale_payment p USING(network,block_number,log_index,ordinal) WHERE block_number=$1")
        .bind(b as i64).fetch_one(&db.pool).await.unwrap();
    assert_eq!(paid, (true, "500".into(), "123".into(), CHECKOUT.into()));
    db.drop().await;
}

#[tokio::test]
async fn incomplete_sales_evidence_keeps_the_primary_walk_live_and_the_proof_at_the_gap() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let net = ChainNet::Matic;
    let b = sales_index::BID_V2_MATIC_START_BLOCK + 150;
    let mut bought =
        sales_index::tests::bought_log(&[(CARL, 4, 25, ALICE)], &format!("0x{:064x}", 317), b, 1);
    bought.data.clear();
    let mut incomplete_bid =
        sales_index::tests::bid_accepted_log(CARL, U256::from(4), BOB, ALICE, 1, 400, b, 2);
    incomplete_bid.data.clear();
    let good_bid =
        sales_index::tests::bid_accepted_log(CARL, U256::from(5), BOB, ALICE, 2, 800, b, 3);
    let order = log_at(
        nft_index::legacy_marketplaces(net)[0].0,
        vec![
            nft_index::TOPIC_ORDER_SUCCESSFUL.into(),
            topic_word(U256::from(7)),
            topic_addr(ALICE),
            topic_addr(BOB),
        ],
        (
            FixedBytes::<32>::repeat_byte(5),
            Address::from_str(CARL).unwrap(),
            U256::from(400),
        )
            .abi_encode_params(),
        b,
        4,
    );
    let mut partial_order = order.clone();
    partial_order.topics[2] = "0x".into();
    let chain = FakeChain::new(
        b + 64,
        vec![bought, incomplete_bid, good_bid, partial_order],
        1000,
        100,
    );
    let url = serve(chain.clone()).await;
    let disabled = state_over(db.pool.clone(), &url, index_settings(100, 100), false);
    let mut settings = index_settings(100, 100);
    settings.sales_enabled = true;
    let enabled = state_over(db.pool.clone(), &url, settings, false);
    let nft = NftIndex::new(net);
    for ix in [&SalesIndex as &dyn ChainIndex, &nft] {
        sqlx::query("INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,last_block_hash,full_history,folded_block,folded_at,sale_input_version,sale_verified_through,sale_covered_rewinds)
            VALUES($1,'matic',137,$2,$3,$4,true,$3,timezone('UTC',now()),1,$3,0)")
            .bind(ix.indexer()).bind(ix.start_block() as i64).bind(b as i64-1).bind(chain.hash_of(b-1)).execute(&db.pool).await.unwrap();
    }
    // A Polygon sale with its fee evidence missing must still allow a valid
    // bid in the same window to reach the existing notice and badge readers.
    assert!(
        try_pass(&disabled, &url, &SalesIndex)
            .await
            .unwrap()
            .published
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM shop.sale_event WHERE kind='bid_accepted'"
        )
        .fetch_one(&db.pool)
        .await
        .unwrap(),
        1
    );
    // Start with an unverified old window, then advance live again. A failing
    // sales replay cannot keep either projection at its previous checkpoint.
    *chain.head.lock() += 1;
    for ix in [&SalesIndex as &dyn ChainIndex, &nft] {
        let out = try_pass(&enabled, &url, ix).await.unwrap();
        assert_eq!(out.indexed_to, b as i64 + 1);
        let row:(i64,Option<i64>, Option<i64>)=sqlx::query_as("SELECT last_block,folded_block,sale_verified_through FROM shop.chain_cursor WHERE network='matic' AND indexer=$1")
            .bind(ix.indexer()).fetch_one(&db.pool).await.unwrap();
        assert_eq!(
            row,
            (b as i64 + 1, Some(b as i64 + 1), Some(b as i64 - 1)),
            "{}",
            ix.indexer()
        );
    }
    // Once the endpoint supplies a complete window, the repair can certify
    // the gap at the same tip.
    assert_eq!(
        sqlx::query_scalar::<_, Option<String>>(
            "SELECT from_address FROM shop.nft_event WHERE kind='order_successful'"
        )
        .fetch_one(&db.pool)
        .await
        .unwrap(),
        None
    );
    chain.logs.lock().retain(|log| log["logIndex"] == "0x3");
    chain.logs.lock().push(log_json(&order));
    for ix in [&SalesIndex as &dyn ChainIndex, &nft] {
        try_pass(&enabled, &url, ix).await.unwrap();
        assert_eq!(sqlx::query_scalar::<_,Option<i64>>("SELECT sale_verified_through FROM shop.chain_cursor WHERE network='matic' AND indexer=$1")
            .bind(ix.indexer()).fetch_one(&db.pool).await.unwrap(),Some(b as i64+1));
    }
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT from_address FROM shop.nft_event WHERE kind='order_successful'"
        )
        .fetch_one(&db.pool)
        .await
        .unwrap(),
        ALICE
    );
    db.drop().await;
}

#[tokio::test]
async fn capped_sales_advance_preserves_the_published_prefix_deadline() {
    use crate::market::{ethereum_sales, sale_fold};
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let net = ChainNet::Ethereum;
    let first = nft_index::start_block(net) + 10;
    let chain = FakeChain::new(first + 12, vec![], 1000, 100);
    chain.chain_id.store(1, Ordering::SeqCst);
    let url = serve(chain.clone()).await;
    let st = state_over(db.pool.clone(), &url, index_settings(100, 100), false);
    let rpc = Rpc::at(&st.http, &url);
    sqlx::query("INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,last_block_hash,
        full_history,folded_block,folded_at,sale_input_version,sale_verified_through,sale_covered_rewinds)
        SELECT ix,'ethereum',1,1,$1,$2,true,$1,timezone('UTC',now()),$3,$1,0
        FROM unnest(ARRAY['nft','sales','trade_events']) ix")
        .bind(first as i64).bind(chain.hash_of(first)).bind(crate::market::sale_history::VERSION).execute(&db.pool).await.unwrap();
    sale_fold::pass(&db.pool, &rpc, net).await.unwrap();
    let deadline = sale_fold::snapshot(&db.pool, &[net], chrono::Utc::now())
        .await
        .unwrap()
        .unwrap()
        .expires_at;
    let tip = first + 300;
    *chain.head.lock() = tip + 12;
    sqlx::query(
        "UPDATE shop.chain_cursor SET last_block=$1,last_block_hash=$2,folded_block=$1,
        sale_verified_through=$1,folded_at=timezone('UTC',now())",
    )
    .bind(tip as i64)
    .bind(chain.hash_of(tip))
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO shop.name_purchase(venue,block_number,log_index,block_time,caller,beneficiary,price,name,tx_hash)
        SELECT $1,$2::bigint+i,1,172800,$3,$3,1,'name-'||i,'0x'||lpad(to_hex(i),64,'0') FROM generate_series(1,97) i")
        .bind(ethereum_sales::BURNING_NAME_CONTROLLER).bind(first as i64).bind(ALICE).execute(&db.pool).await.unwrap();
    let old_stamp: chrono::NaiveDateTime =
        sqlx::query_scalar("SELECT published_at FROM shop.sale_publication")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    sale_fold::pass(&db.pool, &rpc, net).await.unwrap();
    let (height, stamp): (i64, chrono::NaiveDateTime) =
        sqlx::query_as("SELECT block_number,published_at FROM shop.sale_publication")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!((height, stamp), ((first + 96) as i64, old_stamp));
    assert!(
        sale_fold::snapshot(&db.pool, &[net], chrono::Utc::now())
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        sale_fold::snapshot(&db.pool, &[net], deadline + chrono::Duration::minutes(1))
            .await
            .unwrap()
            .is_none()
    );
    sale_fold::pass(&db.pool, &rpc, net).await.unwrap();
    assert!(
        sale_fold::snapshot(&db.pool, &[net], chrono::Utc::now())
            .await
            .unwrap()
            .unwrap()
            .expires_at
            > deadline
    );
    db.drop().await;
}

#[tokio::test]
async fn a_pending_sales_replay_leaves_budget_for_ethereum_estate_repair() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let tip = 6_236_559;
    let net = ChainNet::Ethereum;
    let chain = FakeChain::new(tip + net.confirmations(), vec![], 2_000_000, 100);
    chain.chain_id.store(1, Ordering::SeqCst);
    let url = serve(chain.clone()).await;
    let old = state_over(db.pool.clone(), &url, index_settings(100, 2_000_000), false);
    let nft = NftIndex::new(net);
    assert!(publish_nft(&old, &url, &nft).await.published);
    sqlx::query("DELETE FROM shop.worker_progress WHERE worker='nft_estate_creations'")
        .execute(&db.pool)
        .await
        .unwrap();
    let mut settings = index_settings(20, 2);
    settings.sales_enabled = true;
    let current = state_over(db.pool.clone(), &url, settings, false);
    assert!(pass(&current, &url, &NftIndex::new(net)).await.published);
    let progress: (i64, i64) = sqlx::query_as(
        "SELECT c.sale_verified_through,p.position FROM shop.chain_cursor c
        JOIN shop.worker_progress p ON p.worker='nft_estate_creations' AND p.scope=1
        WHERE c.indexer='nft' AND c.network='ethereum'",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert!(progress.0 >= nft_index::start_block(net) as i64 && progress.0 < (tip as i64));
    assert!(
        progress.1 >= 6_236_547 && progress.1 < tip as i64,
        "{progress:?}"
    );
    db.drop().await;
}

#[tokio::test]
async fn archive_repair_carries_a_narrowed_range_across_passes() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let first = SalesIndex.start_block();
    let tip = first + 500;
    let chain = FakeChain::new(tip + 64, vec![], 1, 100);
    chain.omit_range_hint.store(true, Ordering::SeqCst);
    let url = serve(chain.clone()).await;
    let settings = index_settings(100, 100_000);
    let st = state_over(db.pool.clone(), &url, settings.clone(), false);
    let hash = chain.hash_of(tip);
    sqlx::query("INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,last_block_hash,full_history)
        VALUES('sales','matic',137,$1,$2,$3,true)")
        .bind(first as i64).bind(tip as i64).bind(&hash).execute(&db.pool).await.unwrap();
    let source = chain_walk::RpcSource::new(Rpc::at(&st.http, &url));
    let mut previous = tip as i64;
    for _ in 0..8 {
        let owner = chain_walk::test_owner(&db.pool, &SalesIndex, &settings).await;
        let spent = crate::market::sale_history::repair(
            &db.pool,
            &source,
            &SalesIndex,
            &owner,
            Some((tip, &hash)),
            7,
            &mut 100,
        )
        .await
        .unwrap();
        assert!(spent.charge <= 7);
        let (hint, through): (Option<i64>, Option<i64>) =
            sqlx::query_as("SELECT sale_repair_upper,sale_verified_through FROM shop.chain_cursor")
                .fetch_one(&db.pool)
                .await
                .unwrap();
        if through.is_some() {
            assert!(previous < tip as i64);
            assert_eq!((hint, through), (None, Some(first as i64)));
            db.drop().await;
            return;
        }
        let hint = hint.expect("next pass retains a smaller archive interval");
        assert!(hint < previous);
        previous = hint;
    }
    panic!("a one-block window must make progress on this budget");
}

#[tokio::test]
async fn trade_verdict_survives_an_incomplete_sales_window_and_its_repair() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let net = ChainNet::Matic;
    let ix = TradeEventsIndex::mainnet(net);
    let (venue, start, _) = trade_events::marketplaces(net)[0];
    let block = start + 100;
    let (id, signature) = seed_trade(
        &db.pool,
        "public_item_order",
        ALICE,
        venue,
        1,
        0,
        (4, CARL, "1"),
        (1, listings::mana_token(137).unwrap(), "100"),
    )
    .await;
    let log = traded(block, 1, venue, BOB, ALICE, &signature);
    let chain = FakeChain::new(block + 64, vec![log], 1000, 100);
    let tx_hash = format!("0x{:064x}", 79);
    chain.logs.lock()[0]["transactionHash"] = Value::Null;
    let url = serve(chain.clone()).await;
    let mut settings = index_settings(100, 1000);
    settings.sales_enabled = true;
    let st = state_over(db.pool.clone(), &url, settings, false);
    sqlx::query("INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,last_block_hash,
        full_history,folded_block,folded_at,sale_input_version,sale_verified_through,sale_covered_rewinds)
        VALUES('trade_events','matic',137,$1,$2,$3,true,$2,timezone('UTC',now()),1,$2,0)")
        .bind(ix.start_block() as i64).bind(block as i64-1).bind(chain.hash_of(block-1)).execute(&db.pool).await.unwrap();
    for repaired in [false, true] {
        if repaired {
            chain.logs.lock()[0]["transactionHash"] = json!(tx_hash);
        }
        try_pass(&st, &url, &ix).await.unwrap();
        let (status,executions,hash,through):(String,i64,String,i64)=sqlx::query_as(
            "SELECT t.status,t.executions::bigint,e.tx_hash,c.sale_verified_through FROM shop.trade_chain_state t
             CROSS JOIN shop.trade_event e CROSS JOIN shop.chain_cursor c WHERE t.trade_id::text=$1 AND c.indexer='trade_events'")
            .bind(&id).fetch_one(&db.pool).await.unwrap();
        assert_eq!((status.as_str(), executions), ("sold", 1));
        assert_eq!(hash, if repaired { tx_hash.as_str() } else { "" });
        assert_eq!(through >= block as i64, repaired);
    }
    db.drop().await;
}

#[tokio::test]
async fn a_store_purchase_and_order_in_one_receipt_settle_after_a_failed_batch() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let net = ChainNet::Matic;
    let (order_venue, deployed) = nft_index::legacy_marketplaces(net)[0];
    let block = deployed.max(SalesIndex.start_block()) + 100;
    let chain = FakeChain::new(block + 5, vec![], 1000, 100);
    let url = serve(chain.clone()).await;
    let st = state_over(db.pool.clone(), &url, index_settings(100, 100), false);
    let rpc = Rpc::at(&st.http, &url);
    let hash = format!("0x{:064x}", 82);
    let mana = listings::mana_token(137).unwrap();
    let store = sales_index::STORE_MATIC;
    seed_collection(
        &db.pool,
        "matic",
        CARL,
        block - 10,
        "store-collection",
        "Store collection",
    )
    .await;
    chain
        .creators_at_hash
        .lock()
        .insert((CARL.into(), chain.hash_of(block)), ALICE.into());
    for ix in ["nft", "sales", "trade_events"] {
        sqlx::query("INSERT INTO shop.chain_cursor(indexer,network,chain_id,start_block,last_block,last_block_hash,
            full_history,folded_block,folded_at,sale_input_version,sale_verified_through,sale_covered_rewinds)
            VALUES($1,'matic',137,1,$2,$3,true,$2,timezone('UTC',now()),1,$2,0)")
            .bind(ix).bind(block as i64).bind(chain.hash_of(block)).execute(&db.pool).await.unwrap();
    }
    sqlx::query("INSERT INTO shop.sale_event(network,block_number,log_index,block_time,kind,emitter,contract_address,token_id,item_id,buyer,tx_hash)
         VALUES('matic',$1,1,$2,'mint',$3,$3,1,0,$4,$5)").bind(block as i64).bind(time_of(block)).bind(CARL).bind(BOB).bind(&hash).execute(&db.pool).await.unwrap();
    sqlx::query("INSERT INTO shop.store_sale_line(block_number,bought_log,ordinal,block_time,tx_hash,contract_address,item_id,beneficiary,price,issue_log)
         VALUES($1,2,0,$2,$3,$4,0,$5,250,1)").bind(block as i64).bind(time_of(block)).bind(&hash).bind(CARL).bind(BOB).execute(&db.pool).await.unwrap();
    sqlx::query("INSERT INTO shop.nft_event(network,block_number,log_index,block_time,kind,emitter,contract_address,token_id,from_address,to_address,amount,tx_hash)
        VALUES('matic',$1,4,$2,'order_successful',$3,$4,9,$5,$6,75,$7)").bind(block as i64).bind(time_of(block)).bind(order_venue).bind(CARL).bind(DAVE).bind(BOB).bind(&hash).execute(&db.pool).await.unwrap();
    sqlx::query(
        "INSERT INTO shop.sale_fee_change(network,venue,field,block_number,log_index,value)
        VALUES('matic',$1,'fee_ppm',1,0,'0')",
    )
    .bind(store)
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO shop.nft_contract(network,contract_address,walked_from_block) VALUES($1,$2,$3)")
        .bind(net.db_name()).bind(CARL).bind(block as i64).execute(&db.pool).await.unwrap();
    let movement = |to, amount: u64, index| {
        log_at(
            mana,
            vec![
                nft_index::TOPIC_TRANSFER.into(),
                topic_addr(BOB),
                topic_addr(to),
            ],
            U256::from(amount).abi_encode(),
            block,
            index,
        )
    };
    let logs = [
        movement(ALICE, 250, 0),
        sales_index::tests::issue_log(CARL, BOB, U256::from(1), 0, store, block, 1),
        sales_index::tests::bought_log(&[(CARL, 0, 250, BOB)], &hash, block, 2),
        movement(DAVE, 75, 3),
        log_at(
            order_venue,
            vec![
                nft_index::TOPIC_ORDER_SUCCESSFUL.into(),
                topic_word(U256::from(9)),
                topic_addr(DAVE),
                topic_addr(BOB),
            ],
            (
                FixedBytes::<32>::repeat_byte(1),
                Address::from_str(CARL).unwrap(),
                U256::from(75),
            )
                .abi_encode_params(),
            block,
            4,
        ),
    ]
    .iter()
    .map(|l| {
        let mut v = log_json(l);
        v["transactionHash"] = json!(hash);
        v["blockHash"] = json!(chain.hash_of(block));
        v
    })
    .collect::<Vec<_>>();
    chain.fail_receipt_batch.store(true, Ordering::SeqCst);
    for status in [None, Some("0x0"), Some("0x1")] {
        if let Some(status) = status {
            chain.receipts.lock().insert(
                hash.clone(),
                json!({"status":status,"transactionHash":hash,
                "blockNumber":format!("0x{block:x}"),"blockHash":chain.hash_of(block),"logs":logs}),
            );
            sqlx::query("UPDATE shop.sale_execution SET receipt_attempted_at=NULL")
                .execute(&db.pool)
                .await
                .unwrap();
        }
        crate::market::sale_fold::pass(&db.pool, &rpc, net)
            .await
            .unwrap();
        let states: Vec<(bool, bool, bool)> = sqlx::query_as(
            "SELECT receipt_attempted_at IS NOT NULL,receipt_read,settled FROM shop.sale_execution
            JOIN shop.sale_payment USING(network,block_number,log_index) ORDER BY log_index",
        )
        .fetch_all(&db.pool)
        .await
        .unwrap();
        let ok = status == Some("0x1");
        assert_eq!(states, [(true, ok, ok); 2]);
    }
    let legs: Vec<(i32, String, String)> = sqlx::query_as(
        "SELECT log_index,recipient,amount::text FROM shop.sale_payout ORDER BY log_index",
    )
    .fetch_all(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        legs,
        [
            (2, ALICE.into(), "250".into()),
            (4, DAVE.into(), "75".into())
        ]
    );
    db.drop().await;
}
