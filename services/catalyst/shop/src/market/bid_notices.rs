//! The `bid_received` producer for on-chain bids: each ERC721BidV2
//! `BidCreated` the `sales` index keeps (`shop.sale_event`, Polygon) tells
//! the token's owner "Bid Received", as events-notifier's bids poller did
//! (`events-notifier@5ceb8abc`, `src/adapters/producers/bid-received.ts`).
//! The recipient is the owner when the bid was placed, the subgraph's
//! `seller`: the nft walk's latest transfer before the bid, or the indexed
//! owner when there is none or the contract still owes a backfill (as the
//! bids fold reads it). A bid by that owner notifies nobody. The metadata has the keys of a local bid's
//! notification (`trade_events::bid_received_metadata`) with the poller's
//! values: its bids link, MANA to two decimals and network `polygon`.
//!
//! A bid placed through this node's Shop is a signed off-chain trade
//! (`listings::create_trade`), which notifies there and never emits
//! `BidCreated`. A `placeBid` the relay forwards as a meta-transaction does
//! emit it and is announced here only: the relay writes no notification.
//!
//! Reads stop at the lower published block of the sales and nft indexes
//! while both are ready. The cursor (`shop.notification_cursor`,
//! `bid_received`) is seeded on the first pass just before
//! `MARKETPLACE_BID_NOTICES_FROM_BLOCK` when set, so the bids since a
//! previous notifier stopped are announced, and otherwise at the sales
//! index's published block, so history is never announced. It advances
//! with each page's notifications in one transaction; ids are reorg-stable
//! (`notice_cursor::tx_log_key`), so a retried page inserts nothing twice.
//! A bid on a token no index describes holds the cursor while
//! `item_awaited`, as the sale producers do.

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::chain_walk::{self, ChainNet, INDEXER_NFT, INDEXER_SALES};
use super::notice_cursor;
use super::trade_events::{self, AssetMeta};
use crate::jobs::{Jobs, PeriodicCfg};
use crate::shared::collections::item_awaited;
use crate::shared::identity::Address;
use crate::shared::notifications::{NewNotification, format_mana, insert_notifications};

pub const KIND: &str = "bid_received";
const PRODUCER: &str = notice_cursor::BID_RECEIVED;
const NET: ChainNet = ChainNet::Matic;
const PAGE_SIZE: i64 = 200;
const POLL_SECS: u64 = 60;

/// Over the `BidCreated` log's [`notice_cursor::tx_log_key`].
pub fn notification_id(network: &str, key: &str) -> Uuid {
    notice_cursor::notification_id("bid-received", network, key)
}

/// Where the cursor starts: just before `from_block` when the operator set
/// it, else after the published block.
fn seed_point(from_block: Option<u64>, published: i64) -> (i64, i32) {
    match from_block {
        Some(b) => ((b as i64).saturating_sub(1), i32::MAX),
        None => (published, i32::MAX),
    }
}

/// What the producer reads beside the database.
#[derive(Clone, Debug)]
pub struct Settings {
    pub marketplace: String,
    pub land_images: String,
    /// Whether the items mirror runs: a bid waits for its asset only then.
    pub items_mirror_enabled: bool,
    /// `MARKETPLACE_BID_NOTICES_FROM_BLOCK`.
    pub from_block: Option<u64>,
}

#[derive(sqlx::FromRow)]
struct BidRow {
    id_key: String,
    block_number: i64,
    log_index: i32,
    block_time: i64,
    contract_address: String,
    token_id: String,
    bidder: String,
    seller: Option<String>,
    price: String,
}

/// `$1` network, `$2`/`$3` cursor, `$4` last block read, `$5` page size.
/// A page walks `shop.sale_event`'s primary key from the cursor; each row's
/// owner is one backward probe of `nft_event_token_idx` and one primary key
/// read of `shop.nft`.
fn rows_sql() -> String {
    format!(
        "SELECT {key} AS id_key, e.block_number, e.log_index, e.block_time, e.contract_address::text AS contract_address, \
                e.token_id::text AS token_id, lower(e.buyer) AS bidder, e.price::text AS price, \
                lower(COALESCE((SELECT n.to_address FROM shop.nft_event n \
                    WHERE n.network = e.network AND n.contract_address = e.contract_address AND n.token_id = e.token_id \
                      AND n.kind = 'transfer' AND (n.block_number, n.log_index) < (e.block_number, e.log_index) \
                      AND NOT EXISTS (SELECT 1 FROM shop.nft_contract k \
                        WHERE k.network = e.network AND k.contract_address = e.contract_address AND k.backfill_from_block IS NOT NULL \
                          AND COALESCE(k.backfilled_to_block, k.backfill_from_block - 1) < k.walked_from_block - 1) \
                    ORDER BY n.block_number DESC, n.log_index DESC LIMIT 1), \
                  (SELECT o.owner FROM shop.nft o \
                    WHERE o.network = e.network AND o.contract_address = e.contract_address AND o.token_id = e.token_id))) AS seller \
         FROM shop.sale_event e \
         WHERE e.network = $1 AND e.kind = 'bid_created' AND (e.block_number, e.log_index) > ($2, $3) AND e.block_number <= $4 \
         ORDER BY e.block_number, e.log_index LIMIT $5",
        key = notice_cursor::tx_log_key(
            "sale_event",
            "e",
            "(e.kind || ':' || e.contract_address || ':' || e.token_id || ':' || coalesce(e.bid_id, ''))",
        )
    )
}

/// The notification of one bid; `None` without a seller or for the owner's own bid.
fn notice(settings: &Settings, row: &BidRow, asset: &AssetMeta) -> Option<NewNotification> {
    let seller = row.seller.as_deref()?;
    if seller == row.bidder {
        return None;
    }
    let address = Address::parse(seller).ok()?;
    let asset = AssetMeta {
        seller: seller.to_string(),
        ..asset.clone()
    };
    let metadata = trade_events::bid_received_metadata(
        &asset,
        format!(
            "{}/account?assetType=nft&section=bids",
            settings.marketplace
        ),
        &row.price,
        &format_mana(&json!(row.price)),
        "polygon",
    );
    Some(NewNotification {
        id: notification_id(NET.db_name(), &row.id_key),
        address,
        kind: KIND.to_string(),
        metadata,
        occurred_at: DateTime::from_timestamp(row.block_time, 0)
            .unwrap_or_default()
            .naive_utc(),
    })
}

/// What one pass did.
#[derive(Debug, PartialEq, Eq)]
pub enum Pass {
    Emitted(u64),
    Waiting(&'static str),
}

/// One pass over the published bids after the cursor, a page at a time.
pub async fn run_once(
    pool: &PgPool,
    settings: &Settings,
    now: DateTime<Utc>,
) -> anyhow::Result<Pass> {
    let Some(sales) = chain_walk::ready_cursor(pool, INDEXER_SALES, NET, now).await else {
        return Ok(Pass::Waiting("the matic sales index is not published"));
    };
    let Some(nft) = chain_walk::ready_cursor(pool, INDEXER_NFT, NET, now).await else {
        return Ok(Pass::Waiting(
            "the matic nft index is not published (a bid's owner is unknown)",
        ));
    };
    let published = sales.folded_block.unwrap_or(sales.last_block);
    let upto = published.min(nft.folded_block.unwrap_or(nft.last_block));
    let (seed_block, seed_log) = seed_point(settings.from_block, published);
    notice_cursor::seed(pool, PRODUCER, None, seed_block, seed_log).await?;
    let network = NET.db_name();
    let mut emitted = 0;
    loop {
        let mut txn = pool.begin().await?;
        let (_, since_block, since_log) = notice_cursor::lock(&mut txn, PRODUCER).await?;
        let rows: Vec<BidRow> = sqlx::query_as(sqlx::AssertSqlSafe(rows_sql()))
            .bind(network)
            .bind(since_block)
            .bind(since_log)
            .bind(upto)
            .bind(PAGE_SIZE)
            .fetch_all(&mut *txn)
            .await?;
        if rows.is_empty() {
            txn.rollback().await?;
            break;
        }
        let keys: Vec<(String, String)> = rows
            .iter()
            .map(|r| (r.contract_address.clone(), r.token_id.clone()))
            .collect();
        let metas = trade_events::nft_metas(pool, &settings.land_images, network, &keys).await?;
        let items_pass: Option<i64> = sqlx::query_scalar(
            "SELECT extract(epoch FROM last_complete_pass_at)::bigint FROM shop.mirror_state \
             WHERE mirror = 'items' AND network = $1",
        )
        .bind(network)
        .fetch_optional(&mut *txn)
        .await?
        .flatten();
        let ts = now.timestamp();
        let held = rows.iter().position(|r| {
            !metas.contains_key(&(r.contract_address.clone(), r.token_id.clone()))
                && item_awaited(settings.items_mirror_enabled, items_pass, r.block_time, ts)
        });
        let ready = &rows[..held.unwrap_or(rows.len())];
        let Some(last) = ready.last() else {
            tracing::debug!(
                block = rows[0].block_number,
                log = rows[0].log_index,
                "bid on a token not described yet: held"
            );
            txn.rollback().await?;
            break;
        };
        let (done_block, done_log) = (last.block_number, last.log_index);
        let batch: Vec<NewNotification> = ready
            .iter()
            .filter_map(|r| {
                let asset = metas.get(&(r.contract_address.clone(), r.token_id.clone()))?;
                notice(settings, r, asset)
            })
            .collect();
        emitted += insert_notifications(&mut *txn, &batch).await?;
        notice_cursor::advance(&mut txn, PRODUCER, None, done_block, done_log).await?;
        txn.commit().await?;
        if held.is_some() || rows.len() < PAGE_SIZE as usize {
            break;
        }
    }
    Ok(Pass::Emitted(emitted))
}

pub fn spawn(settings: Settings, sales_index_enabled: bool, pool: PgPool, jobs: &Jobs) {
    tracing::info!(
        sales_index_enabled,
        from_block = settings.from_block,
        "bid_received producer up: reads the published Polygon sales and nft indexes"
    );
    jobs.spawn_periodic_leased(
        "bid-notifications",
        PeriodicCfg::every(Duration::from_secs(POLL_SECS))
            .with_jitter(Duration::from_secs(10))
            .with_error_backoff(Duration::from_secs(600)),
        move |ctx| {
            let (pool, settings) = (pool.clone(), settings.clone());
            async move {
                match run_once(&pool, &settings, Utc::now()).await? {
                    Pass::Emitted(0) => {}
                    Pass::Emitted(n) => {
                        tracing::info!(emitted = n, "bid_received notifications emitted");
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
mod tests {
    use super::*;
    use crate::market::transfer_notices::tests::{PrivateDb, publish};
    use serde_json::Value;

    const OWNER: &str = "0x00000000000000000000000000000000000000a1";
    const BIDDER: &str = "0x00000000000000000000000000000000000000b2";
    const NEW_OWNER: &str = "0x00000000000000000000000000000000000000e5";
    const CREATOR: &str = "0x00000000000000000000000000000000000000c3";
    const COLLECTION: &str = "0x00000000000000000000000000000000000000d4";
    const BID: &str = super::super::sales_index::BID_V2_MATIC;
    const T0: i64 = 1_700_000_000;

    #[test]
    fn the_cursor_starts_before_the_operators_block_or_after_the_published_one() {
        assert_eq!(seed_point(Some(60), 100), (59, i32::MAX));
        assert_eq!(seed_point(Some(160), 100), (159, i32::MAX));
        assert_eq!(seed_point(None, 100), (100, i32::MAX));
        assert_ne!(
            notification_id("matic", "0xa:0:k"),
            notice_cursor::notification_id("bid-accepted", "matic", "0xa:0:k")
        );
    }

    fn settings(from_block: Option<u64>, items_mirror_enabled: bool) -> Settings {
        Settings {
            marketplace: "https://market.example".into(),
            land_images: "https://api.example".into(),
            items_mirror_enabled,
            from_block,
        }
    }

    async fn seed_assets(pool: &PgPool) {
        sqlx::query(
            "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, is_completed, is_approved, is_editable, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ('matic', $1, 137, 'Coll', $2, $2, 'urn:coll', true, true, false, 1, 1, 1, timezone('UTC', now()))",
        )
        .bind(COLLECTION)
        .bind(CREATOR)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, total_supply, max_supply, available, price, image, name, category, body_shapes, search_is_collection_approved, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ('matic', $1, 0, 137, 'wearable_v2', 'urn:coll:0', $2, 'epic', 1, 100, 99, 10, 'https://img/0', 'Shoes', 'feet', ARRAY['BaseMale'], true, 1, 1, 1, timezone('UTC', now()))",
        )
        .bind(COLLECTION)
        .bind(CREATOR)
        .execute(pool)
        .await
        .unwrap();
        // Token 7 is minted to OWNER and sold to NEW_OWNER at block 90.
        sqlx::query(
            "INSERT INTO shop.nft (network, contract_address, token_id, owner, item_id, issued_id, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
             VALUES ('matic', $1, 7, $2, 0, 7, 10, $3, 90, 0, $3)",
        )
        .bind(COLLECTION)
        .bind(NEW_OWNER)
        .bind(T0)
        .execute(pool)
        .await
        .unwrap();
        for (block, from, to) in [(10, nft_zero(), OWNER), (90, OWNER, NEW_OWNER)] {
            sqlx::query(
                "INSERT INTO shop.nft_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, from_address, to_address) \
                 VALUES ('matic', $1, 0, $2, 'transfer', $3, $3, 7, $4, $5)",
            )
            .bind(block)
            .bind(T0 + block)
            .bind(COLLECTION)
            .bind(from)
            .bind(to)
            .execute(pool)
            .await
            .unwrap();
        }
    }

    fn nft_zero() -> &'static str {
        super::super::nft_index::ZERO_ADDRESS
    }

    /// A `BidCreated` row at `block` (log 1) on `token` by `bidder`.
    async fn bid(pool: &PgPool, block: i64, token: i64, bidder: &str, price: &str) {
        sqlx::query(
            "INSERT INTO shop.sale_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, bid_id, buyer, price, expires_at, tx_hash) \
             VALUES ('matic', $1, 1, $2, 'bid_created', $3, $4, $5, $6, $7, $8::numeric, $2 + 86400, $9)",
        )
        .bind(block)
        .bind(T0 + block)
        .bind(BID)
        .bind(COLLECTION)
        .bind(token)
        .bind(format!("0x{block:064x}"))
        .bind(bidder)
        .bind(price)
        .bind(format!("0x{:064x}", block + 1_000_000))
        .execute(pool)
        .await
        .unwrap();
    }

    async fn stored(pool: &PgPool) -> Vec<(String, Value, i64)> {
        sqlx::query_as(
            "SELECT address, metadata, extract(epoch FROM occurred_at)::bigint FROM social.notifications \
             WHERE type = 'bid_received' ORDER BY occurred_at, id",
        )
        .fetch_all(pool)
        .await
        .unwrap()
    }

    async fn cursor(pool: &PgPool) -> (i64, i32) {
        sqlx::query_as(
            "SELECT block_number, log_index FROM shop.notification_cursor WHERE producer = 'bid_received'",
        )
        .fetch_one(pool)
        .await
        .unwrap()
    }

    /// Indexed `BidCreated` rows notify the token's owner at the bid, once:
    /// nothing before both indexes publish; a cursor seeded at the
    /// operator's block announces the bids from it, not before; reads stop
    /// at the lower published block; the owner's own bid notifies nobody; a
    /// rewound cursor inserts nothing twice; a bid on an undescribed token
    /// holds the cursor only while the items mirror may still bring it.
    #[tokio::test]
    async fn indexed_onchain_bids_notify_the_owner_once() {
        let Some(db) = PrivateDb::create("bidnotif").await else {
            return;
        };
        let pool = &db.pool;
        seed_assets(pool).await;
        let s = settings(Some(60), false);
        let one_mana = "1000000000000000000";

        bid(pool, 50, 7, BIDDER, one_mana).await; // before the operator's block
        bid(pool, 70, 7, BIDDER, "1500000000000000000").await; // owner: OWNER
        bid(pool, 80, 7, OWNER, one_mana).await; // the owner's own bid
        bid(pool, 95, 7, BIDDER, "2000000000000000000").await; // owner: NEW_OWNER
        bid(pool, 150, 7, BIDDER, one_mana).await; // past the published nft block

        assert_eq!(
            run_once(pool, &s, Utc::now()).await.unwrap(),
            Pass::Waiting("the matic sales index is not published")
        );
        publish(pool, INDEXER_SALES, "matic", 200, T0 + 200).await;
        assert!(matches!(
            run_once(pool, &s, Utc::now()).await.unwrap(),
            Pass::Waiting(_)
        ));
        publish(pool, INDEXER_NFT, "matic", 120, T0 + 120).await;

        assert_eq!(
            run_once(pool, &s, Utc::now()).await.unwrap(),
            Pass::Emitted(2)
        );
        assert_eq!(
            cursor(pool).await,
            (95, 1),
            "stops at the nft index's block"
        );
        let rows = stored(pool).await;
        assert_eq!(rows.len(), 2, "{rows:?}");
        assert_eq!(rows[0].0, OWNER);
        assert_eq!(rows[0].2, T0 + 70, "occurred at the bid's block time");
        assert_eq!(
            rows[0].1,
            json!({
                "image": "https://img/0",
                "seller": OWNER,
                "category": "wearable",
                "rarity": "epic",
                "link": "https://market.example/account?assetType=nft&section=bids",
                "nftName": "Shoes",
                "price": "1500000000000000000",
                "title": "Bid Received",
                "description": "You received a bid of 1.50 MANA for this Shoes.",
                "network": "polygon",
            })
        );
        assert_eq!(rows[1].0, NEW_OWNER, "the owner when the bid was placed");
        assert_eq!(rows[1].1["seller"], NEW_OWNER);
        assert_eq!(
            rows[1].1["description"],
            "You received a bid of 2.00 MANA for this Shoes."
        );

        assert_eq!(
            run_once(pool, &s, Utc::now()).await.unwrap(),
            Pass::Emitted(0),
            "nothing twice"
        );

        // A changed setting does not move a seeded cursor.
        let later = settings(Some(150), false);
        publish(pool, INDEXER_NFT, "matic", 200, T0 + 200).await;
        assert_eq!(
            run_once(pool, &later, Utc::now()).await.unwrap(),
            Pass::Emitted(1),
            "the bid at 150, once the nft index passes it"
        );

        // A replay over the same logs inserts nothing twice.
        sqlx::query(
            "UPDATE shop.notification_cursor SET block_number = 59, log_index = 2147483647 WHERE producer = 'bid_received'",
        )
        .execute(pool)
        .await
        .unwrap();
        assert_eq!(
            run_once(pool, &s, Utc::now()).await.unwrap(),
            Pass::Emitted(0),
            "a replayed page inserts nothing twice"
        );
        assert_eq!(stored(pool).await.len(), 3);

        // A recent bid on a token no index describes waits while the mirror runs.
        bid(pool, 190, 9, BIDDER, one_mana).await;
        sqlx::query("UPDATE shop.sale_event SET block_time = $1 WHERE block_number = 190")
            .bind(Utc::now().timestamp() - 60)
            .execute(pool)
            .await
            .unwrap();
        let mirrored = settings(None, true);
        assert_eq!(
            run_once(pool, &mirrored, Utc::now()).await.unwrap(),
            Pass::Emitted(0)
        );
        assert_eq!(
            cursor(pool).await,
            (150, 1),
            "held before the undescribed bid"
        );
        assert_eq!(
            run_once(pool, &s, Utc::now()).await.unwrap(),
            Pass::Emitted(0)
        );
        assert_eq!(cursor(pool).await, (190, 1), "passed with the mirror off");

        let names: Vec<String> =
            sqlx::query_scalar("SELECT producer FROM shop.notification_cursor")
                .fetch_all(pool)
                .await
                .unwrap();
        assert_eq!(names, [PRODUCER]);
        db.drop().await;
    }
}
