//! The `transfer_received` producer (the old squid's "gift" notification,
//! marketplace-squid-core `polygon/main.ts` post-batch reconciliation): a
//! Polygon collection transfer that is neither a mint, a burn nor part of a
//! sale tells its receiver. The source is the `nft` index's `transfer` rows
//! on `matic`; a sale is a legacy `OrderSuccessful` (`nft_event`) or an
//! off-chain `Traded` naming the token (`trade_event`) in the same block, a
//! hop through the ERC721Bid contract, or a leg through a broker escrow.
//! The log keeps no tx hash, so the same-block match stands in for the old
//! same-transaction one: a sale's Transfer and its event share a block.
//! Reads stop at the lower published block of the two indexes; the first
//! pass starts at the newest published transfer, so history is never
//! announced. Progress is the `transfer_received` row of
//! `shop.notification_cursor`, advanced with each page's notifications.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use serde_json::{Map, Value, json};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::MarketplaceState;
use super::chain_walk::{self, ChainNet, INDEXER_NFT, INDEXER_TRADE_EVENTS};
use crate::evm::contracts_addrs::DclContracts;
use crate::jobs::{Jobs, PeriodicCfg};
use crate::social::Address;
use crate::social::notifications::store::{NewNotification, insert_notifications};

pub const KIND: &str = "transfer_received";
const PRODUCER: &str = "transfer_received";
const PAGE_SIZE: i64 = 500;
const POLL_SECS: u64 = 60;
const ZERO_ADDRESS: &str = "0x0000000000000000000000000000000000000000";
/// The collections' `baseURI` the old squid built `tokenURI` from.
const COLLECTION_BASE_URI: &str =
    "https://peer.decentraland.org/lambdas/collections/standard/erc721/";

pub fn notification_id(network: &str, block_number: i64, log_index: i32) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_OID,
        format!("nft-transfer:{network}:{block_number}:{log_index}").as_bytes(),
    )
}

/// What one pass did: inserted notifications, or why it waited.
#[derive(Debug, PartialEq, Eq)]
pub enum Pass {
    Emitted(u64),
    Waiting(&'static str),
}

/// The producer's `(block_number, log_index)` and `end_time`, locked for the page.
pub(crate) async fn lock_cursor(
    conn: &mut PgConnection,
    producer: &str,
) -> sqlx::Result<(Option<i64>, i64, i32)> {
    sqlx::query_as("SELECT end_time, block_number, log_index FROM shop.notification_cursor WHERE producer = $1 FOR UPDATE")
        .bind(producer)
        .fetch_one(conn)
        .await
}

/// Seeds the producer's row once; an existing row is left alone.
pub(crate) async fn seed_cursor(
    pool: &PgPool,
    producer: &str,
    end_time: Option<i64>,
    block_number: i64,
    log_index: i32,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO shop.notification_cursor (producer, end_time, block_number, log_index) \
         VALUES ($1, $2, $3, $4) ON CONFLICT (producer) DO NOTHING",
    )
    .bind(producer)
    .bind(end_time)
    .bind(block_number)
    .bind(log_index)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn advance_cursor(
    conn: &mut PgConnection,
    producer: &str,
    end_time: Option<i64>,
    block_number: i64,
    log_index: i32,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE shop.notification_cursor SET end_time = $2, block_number = $3, log_index = $4, \
         updated_at = timezone('UTC', now()) WHERE producer = $1",
    )
    .bind(producer)
    .bind(end_time)
    .bind(block_number)
    .bind(log_index)
    .execute(conn)
    .await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct TransferRow {
    block_number: i64,
    log_index: i32,
    block_time: i64,
    contract_address: String,
    from_address: Option<String>,
    to_address: Option<String>,
    item_id: Option<String>,
    issued_id: Option<String>,
    gift: bool,
}

fn metadata(row: &TransferRow, chain_id: i64) -> Value {
    let mut m = Map::new();
    m.insert("senderAddress".into(), json!(row.from_address));
    m.insert("receiverAddress".into(), json!(row.to_address));
    if let (Some(item), Some(issued)) = (&row.item_id, &row.issued_id) {
        m.insert(
            "tokenUri".into(),
            json!(format!(
                "{COLLECTION_BASE_URI}{chain_id}/{}/{item}/{issued}",
                row.contract_address
            )),
        );
    }
    Value::Object(m)
}

/// The addresses whose transfers are a sale's legs: the ERC721Bid contract
/// and every broker escrow.
async fn sale_hops(pool: &PgPool, chain_id: i64) -> sqlx::Result<Vec<String>> {
    let mut hops: Vec<String> = sqlx::query_scalar(
        "SELECT lower(escrow_address) FROM shop.broker_purchases \
         UNION SELECT lower(escrow_address) FROM shop.escrow_actions WHERE escrow_address IS NOT NULL",
    )
    .fetch_all(pool)
    .await?;
    if let Some(c) = u64::try_from(chain_id)
        .ok()
        .and_then(DclContracts::for_chain)
    {
        hops.push(format!("{:#x}", c.bid_v2));
    }
    Ok(hops)
}

/// One pass over the published transfers after the cursor, a page at a time.
pub async fn run_once(pool: &PgPool, chain_id: i64) -> anyhow::Result<Pass> {
    let now = Utc::now();
    let Some(nft) = chain_walk::ready_cursor(pool, INDEXER_NFT, ChainNet::Matic, now).await else {
        return Ok(Pass::Waiting("the matic nft index is not published"));
    };
    let Some(trades) =
        chain_walk::ready_cursor(pool, INDEXER_TRADE_EVENTS, ChainNet::Matic, now).await
    else {
        return Ok(Pass::Waiting(
            "the matic trade_events index is not published (sales cannot be told from gifts)",
        ));
    };
    let upto = nft.last_block.min(trades.last_block);
    let network = ChainNet::Matic.db_name();
    let latest: Option<(i64, i32)> = sqlx::query_as(
        "SELECT block_number, log_index FROM shop.nft_event \
         WHERE network = $1 AND kind = 'transfer' AND block_number <= $2 \
         ORDER BY block_number DESC, log_index DESC LIMIT 1",
    )
    .bind(network)
    .bind(upto)
    .fetch_optional(pool)
    .await?;
    let (seed_block, seed_log) = latest.unwrap_or((0, -1));
    seed_cursor(pool, PRODUCER, None, seed_block, seed_log).await?;
    let hops = sale_hops(pool, chain_id).await?;
    let mut emitted = 0;
    loop {
        let mut txn = pool.begin().await?;
        let (_, since_block, since_log) = lock_cursor(&mut txn, PRODUCER).await?;
        let rows: Vec<TransferRow> = sqlx::query_as(
            "SELECT e.block_number, e.log_index, e.block_time, e.contract_address, \
                    lower(e.from_address) AS from_address, lower(e.to_address) AS to_address, \
                    n.item_id::text AS item_id, n.issued_id::text AS issued_id, \
                    (lower(e.from_address) <> $5 AND lower(e.to_address) <> $5 \
                     AND lower(e.from_address) <> lower(e.to_address) \
                     AND NOT lower(e.from_address) = ANY($6) AND NOT lower(e.to_address) = ANY($6) \
                     AND NOT EXISTS (SELECT 1 FROM shop.nft_event o \
                        WHERE o.network = e.network AND o.block_number = e.block_number \
                          AND o.kind = 'order_successful' \
                          AND o.contract_address = e.contract_address AND o.token_id = e.token_id) \
                     AND NOT EXISTS (SELECT 1 FROM shop.trade_event t \
                        CROSS JOIN LATERAL (SELECT jsonb_build_array(jsonb_build_object( \
                            'assetType', '3', 'contractAddress', e.contract_address, 'value', e.token_id::text)) AS asset) a \
                        WHERE t.network = e.network AND t.block_number = e.block_number AND t.kind = 'traded' \
                          AND (t.assets->'sent' @> a.asset OR t.assets->'received' @> a.asset)) \
                    ) IS TRUE AS gift \
             FROM shop.nft_event e \
             LEFT JOIN shop.nft n ON n.network = e.network AND n.contract_address = e.contract_address AND n.token_id = e.token_id \
             WHERE e.network = $1 AND (e.block_number, e.log_index) > ($2, $3) AND e.block_number <= $4 \
               AND e.kind = 'transfer' \
             ORDER BY e.block_number, e.log_index LIMIT $7",
        )
        .bind(network)
        .bind(since_block)
        .bind(since_log)
        .bind(upto)
        .bind(ZERO_ADDRESS)
        .bind(&hops)
        .bind(PAGE_SIZE)
        .fetch_all(&mut *txn)
        .await?;
        let Some(last) = rows.last() else {
            txn.rollback().await?;
            break;
        };
        let (last_block, last_log) = (last.block_number, last.log_index);
        let batch: Vec<NewNotification> = rows
            .iter()
            .filter(|r| r.gift)
            .filter_map(|r| {
                let to = Address::parse(r.to_address.as_deref()?).ok()?;
                Some(NewNotification {
                    id: notification_id(network, r.block_number, r.log_index),
                    address: to,
                    kind: KIND.to_string(),
                    metadata: metadata(r, chain_id),
                    occurred_at: chrono::DateTime::from_timestamp(r.block_time, 0)
                        .unwrap_or_default()
                        .naive_utc(),
                })
            })
            .collect();
        emitted += insert_notifications(&mut *txn, &batch).await?;
        advance_cursor(&mut txn, PRODUCER, None, last_block, last_log).await?;
        txn.commit().await?;
        if rows.len() < PAGE_SIZE as usize {
            break;
        }
    }
    Ok(Pass::Emitted(emitted))
}

pub fn spawn(state: &Arc<MarketplaceState>, jobs: &Jobs) {
    let pool = state.pool.clone();
    let chain_id = state.settings.market.indexes.chain_id(ChainNet::Matic);
    if !state.settings.market.indexes.nft_enabled
        || !state.settings.market.indexes.trade_events_enabled
    {
        tracing::info!(
            "transfer_received notifications wait for the matic nft and trade_events indexes \
             (MARKETPLACE_NFT_INDEX_ENABLED, MARKETPLACE_TRADE_EVENTS_INDEX_ENABLED)"
        );
    }
    jobs.spawn_periodic_leased(
        "transfer-received-notifications",
        PeriodicCfg::every(Duration::from_secs(POLL_SECS))
            .with_jitter(Duration::from_secs(10))
            .with_error_backoff(Duration::from_secs(600)),
        move |ctx| {
            let pool = pool.clone();
            async move {
                match run_once(&pool, chain_id).await? {
                    Pass::Emitted(n) => {
                        if n > 0 {
                            tracing::info!(emitted = n, "transfer_received notifications emitted");
                        }
                        ctx.progress(format!("{n} emitted"));
                    }
                    Pass::Waiting(why) => ctx.progress(format!("waiting: {why}")),
                }
                Ok(())
            }
        },
    );
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

    /// A database of its own: the chain cursors and producer cursors are global.
    pub(crate) struct PrivateDb {
        name: String,
        pub pool: PgPool,
        admin: PgPool,
    }

    impl PrivateDb {
        pub(crate) async fn create(tag: &str) -> Option<Self> {
            let base = std::env::var("CATALYST_TEST_DATABASE_URL").ok()?;
            let opts: PgConnectOptions = base.parse().unwrap();
            let admin = PgPoolOptions::new()
                .max_connections(1)
                .connect_with(opts.database("postgres"))
                .await
                .unwrap();
            let name = format!(
                "catalyst_test_{tag}_{}",
                &Uuid::new_v4().simple().to_string()[..12]
            );
            sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
                .execute(&admin)
                .await
                .unwrap();
            let mut url = url::Url::parse(&base).unwrap();
            url.set_path(&format!("/{name}"));
            let pool = crate::db::connect_pool(
                url.as_str(),
                &crate::db::PoolSettings {
                    max_connections: 2,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
            crate::db::apply_schema(&pool).await.unwrap();
            Some(Self { name, pool, admin })
        }

        pub(crate) async fn drop(self) {
            self.pool.close().await;
            let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
                "DROP DATABASE \"{}\" WITH (FORCE)",
                self.name
            )))
            .execute(&self.admin)
            .await;
        }
    }

    /// A published chain cursor at `block` (mined at `time`).
    pub(crate) async fn publish(
        pool: &PgPool,
        indexer: &str,
        network: &str,
        block: i64,
        time: i64,
    ) {
        sqlx::query(
            "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, last_block_time, fold_through, folded_block, folded_at, full_history) \
             VALUES ($1, $2, 0, $3, $4, $3, $3, $5, true) \
             ON CONFLICT (indexer, network) DO UPDATE SET last_block = $3, last_block_time = $4, fold_through = $3, folded_block = $3, folded_at = $5",
        )
        .bind(indexer)
        .bind(network)
        .bind(block)
        .bind(time)
        .bind(Utc::now().naive_utc())
        .execute(pool)
        .await
        .unwrap();
    }

    #[test]
    fn transfer_id_is_stable_and_unique_per_event() {
        assert_eq!(
            notification_id("matic", 10, 1),
            notification_id("matic", 10, 1)
        );
        assert_ne!(
            notification_id("matic", 10, 1),
            notification_id("matic", 10, 2)
        );
        assert_ne!(
            notification_id("matic", 10, 1),
            notification_id("ethereum", 10, 1)
        );
    }

    /// Nothing before both indexes publish; the first pass starts at the
    /// newest published transfer; a gift notifies its receiver with the old
    /// metadata; mints, burns, self-transfers, legacy order and Traded
    /// sales, bid-contract hops and unpublished blocks notify nobody; a
    /// retried pass inserts nothing twice.
    #[tokio::test]
    async fn published_gifts_notify_their_receivers_once() {
        let Some(db) = PrivateDb::create("giftnotif").await else {
            return;
        };
        let pool = db.pool.clone();
        let collection = "0x00000000000000000000000000000000000c011e";
        let alice = "0x00000000000000000000000000000000000000a1";
        let bob = "0x00000000000000000000000000000000000000b0";
        let bid = "0xb96697fa4a3361ba35b774a42c58daccaad1b8e1";
        let transfer = |block: i64, log: i32, token: i64, from: &'static str, to: &'static str| {
            let pool = pool.clone();
            async move {
                sqlx::query(
                    "INSERT INTO shop.nft_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, from_address, to_address) \
                     VALUES ('matic', $1, $2, 1700000000 + $1, 'transfer', $3, $3, $4, $5, $6)",
                )
                .bind(block)
                .bind(log)
                .bind(collection)
                .bind(token)
                .bind(from)
                .bind(to)
                .execute(&pool)
                .await
                .unwrap();
            }
        };
        transfer(100, 0, 1, alice, bob).await;
        assert_eq!(
            run_once(&pool, 137).await.unwrap(),
            Pass::Waiting("the matic nft index is not published")
        );
        publish(&pool, INDEXER_NFT, "matic", 105, 1_700_000_105).await;
        assert!(matches!(
            run_once(&pool, 137).await.unwrap(),
            Pass::Waiting(_)
        ));
        publish(&pool, INDEXER_TRADE_EVENTS, "matic", 120, 1_700_000_120).await;
        assert_eq!(
            run_once(&pool, 137).await.unwrap(),
            Pass::Emitted(0),
            "the first pass starts at the newest published transfer"
        );
        // A gift of token (item 3, issue 12).
        let token: i64 = 12;
        sqlx::query(
            "INSERT INTO shop.nft (network, contract_address, token_id, owner, item_id, issued_id, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
             VALUES ('matic', $1, $2, $3, 3, 12, 1, 1, 101, 0, 1)",
        )
        .bind(collection)
        .bind(token)
        .bind(bob)
        .execute(&pool)
        .await
        .unwrap();
        transfer(101, 0, token, alice, bob).await;
        transfer(101, 1, 2, ZERO_ADDRESS, bob).await;
        transfer(101, 2, 3, bob, ZERO_ADDRESS).await;
        // To oneself, in any case.
        transfer(101, 3, 8, bob, "0x00000000000000000000000000000000000000B0").await;
        // A legacy order sale.
        transfer(102, 0, 4, alice, bob).await;
        sqlx::query(
            "INSERT INTO shop.nft_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, from_address, to_address) \
             VALUES ('matic', 102, 1, 1700000102, 'order_successful', '0x480a0f4e360e8964e68858dd231c2922f1df45ef', $1, 4, $2, $3)",
        )
        .bind(collection)
        .bind(alice)
        .bind(bob)
        .execute(&pool)
        .await
        .unwrap();
        // An off-chain Traded sale.
        transfer(103, 0, 5, alice, bob).await;
        sqlx::query(
            "INSERT INTO shop.trade_event (network, block_number, log_index, block_time, marketplace_address, kind, caller, hashed_signature, signer, assets, tx_hash) \
             VALUES ('matic', 103, 1, 1700000103, '0x540fb08edb56aae562864b390542c97f562825ba', 'traded', $1, '0xsig', $2, $3, '0xtx')",
        )
        .bind(bob)
        .bind(alice)
        .bind(json!({
            "sent": [{"assetType": "3", "contractAddress": collection, "value": "5", "beneficiary": bob}],
            "received": [{"assetType": "1", "contractAddress": "0xa1c57f48f0deb89f569dfbe6e2b7f46d33606fd4", "value": "1000", "beneficiary": alice}],
            "uses": "0"
        }))
        .execute(&pool)
        .await
        .unwrap();
        // An ERC721Bid acceptance: owner -> bid contract -> bidder.
        transfer(104, 0, 6, alice, bid).await;
        transfer(104, 1, 6, bid, bob).await;
        // Past the published nft block.
        transfer(106, 0, 7, alice, bob).await;
        assert_eq!(run_once(&pool, 137).await.unwrap(), Pass::Emitted(1));
        assert_eq!(
            run_once(&pool, 137).await.unwrap(),
            Pass::Emitted(0),
            "nothing twice"
        );
        let rows: Vec<(Uuid, String, Value)> = sqlx::query_as(
            "SELECT id, address, metadata FROM social.notifications WHERE type = $1",
        )
        .bind(KIND)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, notification_id("matic", 101, 0));
        assert_eq!(rows[0].1, bob);
        assert_eq!(
            rows[0].2,
            json!({
                "senderAddress": alice,
                "receiverAddress": bob,
                "tokenUri": format!("{COLLECTION_BASE_URI}137/{collection}/3/12"),
            })
        );
        let cursor: (i64, i32) = sqlx::query_as(
            "SELECT block_number, log_index FROM shop.notification_cursor WHERE producer = $1",
        )
        .bind(PRODUCER)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(cursor, (104, 1), "the cursor stops at the published block");
        db.drop().await;
    }

    /// The cursor holds through an index that stops being published, and
    /// moves back when the index rewinds after a deep reorg or a late
    /// backfill writes rows below it: replacement rows are announced, the
    /// rows read before are not announced twice.
    #[tokio::test]
    async fn the_cursor_survives_a_pause_and_replays_a_rewind() {
        let Some(db) = PrivateDb::create("giftreorg").await else {
            return;
        };
        let pool = db.pool.clone();
        let collection = "0x00000000000000000000000000000000000c011e";
        let alice = "0x00000000000000000000000000000000000000a1";
        let bob = "0x00000000000000000000000000000000000000b0";
        let carol = "0x00000000000000000000000000000000000000c0";
        let transfer = |block: i64, log: i32, token: i64, to: &'static str| {
            let pool = pool.clone();
            async move {
                sqlx::query(
                    "INSERT INTO shop.nft_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, from_address, to_address) \
                     VALUES ('matic', $1, $2, 1700000000 + $1, 'transfer', $3, $3, $4, $5, $6)",
                )
                .bind(block)
                .bind(log)
                .bind(collection)
                .bind(token)
                .bind(alice)
                .bind(to)
                .execute(&pool)
                .await
                .unwrap();
            }
        };
        let notified = || {
            let pool = pool.clone();
            async move {
                sqlx::query_scalar::<_, String>(
                    "SELECT address FROM social.notifications WHERE type = $1 ORDER BY address",
                )
                .bind(KIND)
                .fetch_all(&pool)
                .await
                .unwrap()
            }
        };
        let cursor = || {
            let pool = pool.clone();
            async move {
                sqlx::query_as::<_, (i64, i32)>(
                    "SELECT block_number, log_index FROM shop.notification_cursor WHERE producer = $1",
                )
                .bind(PRODUCER)
                .fetch_one(&pool)
                .await
                .unwrap()
            }
        };
        publish(&pool, INDEXER_NFT, "matic", 100, 1_700_000_100).await;
        publish(&pool, INDEXER_TRADE_EVENTS, "matic", 100, 1_700_000_100).await;
        assert_eq!(run_once(&pool, 137).await.unwrap(), Pass::Emitted(0));
        transfer(101, 0, 1, bob).await;
        publish(&pool, INDEXER_NFT, "matic", 101, 1_700_000_101).await;
        publish(&pool, INDEXER_TRADE_EVENTS, "matic", 101, 1_700_000_101).await;
        assert_eq!(run_once(&pool, 137).await.unwrap(), Pass::Emitted(1));

        // Not ready (the fold marker cleared): nothing read, the cursor kept.
        sqlx::query(
            "UPDATE shop.chain_cursor SET folded_block = NULL, folded_at = NULL WHERE indexer = $1",
        )
        .bind(INDEXER_NFT)
        .execute(&pool)
        .await
        .unwrap();
        transfer(102, 0, 2, bob).await;
        assert!(matches!(
            run_once(&pool, 137).await.unwrap(),
            Pass::Waiting(_)
        ));
        assert_eq!(cursor().await, (101, 0));
        publish(&pool, INDEXER_NFT, "matic", 102, 1_700_000_102).await;
        publish(&pool, INDEXER_TRADE_EVENTS, "matic", 102, 1_700_000_102).await;
        assert_eq!(
            run_once(&pool, 137).await.unwrap(),
            Pass::Emitted(1),
            "resumes where it stopped"
        );
        assert_eq!(cursor().await, (102, 0));

        // A reorg rewinds the nft index: block 102's log is replaced.
        let mut tx = pool.begin().await.unwrap();
        chain_walk::replay_notifications(&mut tx, INDEXER_NFT, ChainNet::Matic, -1)
            .await
            .unwrap();
        sqlx::query("DELETE FROM shop.nft_event WHERE network = 'matic' AND block_number > 101")
            .execute(&mut *tx)
            .await
            .unwrap();
        chain_walk::rewind(&mut tx, INDEXER_NFT, ChainNet::Matic, -1)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        assert!(
            cursor().await < (101, 0),
            "moved back below the rewritten rows"
        );
        transfer(102, 1, 3, carol).await;
        publish(&pool, INDEXER_NFT, "matic", 102, 1_700_000_102).await;
        assert_eq!(
            run_once(&pool, 137).await.unwrap(),
            Pass::Emitted(1),
            "the replacement log is announced, the rest not twice"
        );
        assert_eq!(notified().await, vec![bob, bob, carol]);

        // A late backfill writes a gift below the cursor, within the replay window.
        let mut tx = pool.begin().await.unwrap();
        transfer(100, 5, 4, carol).await;
        chain_walk::replay_notifications(&mut tx, INDEXER_NFT, ChainNet::Matic, 99)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(cursor().await, (99, i32::MAX));
        assert_eq!(run_once(&pool, 137).await.unwrap(), Pass::Emitted(1));
        assert_eq!(cursor().await, (102, 1));
        db.drop().await;
    }
}
