//! The `bid_received` producer for on-chain bids: each ERC721BidV2
//! `BidCreated` the `sales` index keeps (`shop.sale_event`, Polygon) on a
//! Decentraland collection token (`shop.collections`) tells the token's
//! owner "Bid Received", as events-notifier's bids poller did
//! (`events-notifier@5ceb8abc`, `src/adapters/producers/bid-received.ts`,
//! which read the Polygon collections subgraph's wearable and emote bids).
//! The bid contract accepts bids on any ERC721, and the sales index keeps
//! them all: a bid on another contract notifies nobody and never holds the
//! cursor.
//!
//! The recipient is the owner when the bid was placed, the subgraph's
//! `seller`: the nft walk's latest transfer (a mint is one) before the bid,
//! read as the bids fold reads it (`sales_index`). While the contract owes
//! an nft backfill that owner is unknown and the bid holds the cursor, at
//! most [`ITEM_HOLD_MAX_SECS`] after it was placed; with no backfill owed
//! and no earlier transfer, or past that bound, it notifies nobody. The
//! current owner (`shop.nft`) is never read as a stand-in: it may be the
//! bidder who has since accepted the bid. A bid by that owner notifies
//! nobody. The metadata has the keys of a local bid's notification
//! (`trade_events::bid_received_metadata`) with the poller's values: its
//! bids link, MANA to two decimals and network `polygon`.
//!
//! A bid placed through this node's Shop is a signed off-chain trade
//! (`listings::create_trade`), which notifies there and never emits
//! `BidCreated`. A `placeBid` the relay forwards as a meta-transaction does
//! emit it and is announced here only: the relay writes no notification.
//!
//! Reads stop at the lower published block of the sales and nft indexes
//! while both are ready, read by each page in the statement that reads its
//! bids, so a rewind between pages never pairs a bid above the fork with
//! the truncated transfers. The cursor (`shop.notification_cursor`,
//! `bid_received`) is seeded on the first pass at the sales index's
//! published block, so history is never announced; `catalyst shop
//! notice-rewind` moves it back for a cutover. It advances with each
//! page's notifications in one transaction; ids are reorg-stable
//! (`notice_cursor::tx_log_key`), so a retried page inserts nothing twice.
//! A bid on a collection token no index describes yet holds the cursor
//! while `item_awaited`, as the sale producers do.

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::chain_walk::{self, ChainNet, INDEXER_NFT, INDEXER_SALES};
use super::notice_cursor;
use super::trade_events::{self, AssetMeta};
use crate::jobs::{Jobs, PeriodicCfg};
use crate::shared::collections::{ITEM_HOLD_MAX_SECS, item_awaited};
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

/// What the producer reads beside the database.
#[derive(Clone, Debug)]
pub struct Settings {
    pub marketplace: String,
    pub land_images: String,
    /// Whether the items mirror runs: a bid waits for its asset only then.
    pub items_mirror_enabled: bool,
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
    /// The token's owner when the bid was placed; `None` when no earlier
    /// transfer is indexed or the contract owes a backfill.
    seller: Option<String>,
    price: String,
    /// The contract is a Decentraland collection the mirror knows.
    collection: bool,
    /// The nft walk still owes the contract a backfill.
    backfill_owed: bool,
}

/// `$1` network, `$2`/`$3` cursor, `$4` the indexes read (sales, nft),
/// `$5` page size, `$6` now. The bound, the lower published block of the
/// two, is read with the rows in one statement, so one snapshot: an nft
/// rewind that commits after the pass checked both indexes lowers it (or
/// withdraws the index) before a bid above the fork can be matched against
/// the truncated transfers. With either index not ready there is no bound
/// and no row. A page walks `shop.sale_event`'s primary key from the
/// cursor; each row adds one backward probe of `nft_event_token_idx` and
/// primary key reads of `shop.nft_contract` and `shop.collections`.
fn rows_sql() -> String {
    format!(
        "WITH bound AS (SELECT min(c.last_block) AS upto FROM shop.chain_cursor c \
             WHERE c.network = $1 AND c.indexer = ANY($4) AND {ready} HAVING count(*) = 2) \
         SELECT {key} AS id_key, e.block_number, e.log_index, e.block_time, e.contract_address::text AS contract_address, \
                e.token_id::text AS token_id, lower(e.buyer) AS bidder, e.price::text AS price, \
                lower((SELECT n.to_address FROM shop.nft_event n \
                    WHERE n.network = e.network AND n.contract_address = e.contract_address AND n.token_id = e.token_id \
                      AND n.kind = 'transfer' AND (n.block_number, n.log_index) < (e.block_number, e.log_index) \
                    ORDER BY n.block_number DESC, n.log_index DESC LIMIT 1)) AS seller, \
                EXISTS (SELECT 1 FROM shop.collections c \
                    WHERE c.network = e.network AND c.contract_address = e.contract_address) AS collection, \
                EXISTS (SELECT 1 FROM shop.nft_contract k \
                    WHERE k.network = e.network AND k.contract_address = e.contract_address AND k.backfill_from_block IS NOT NULL \
                      AND COALESCE(k.backfilled_to_block, k.backfill_from_block - 1) < k.walked_from_block - 1) AS backfill_owed \
         FROM shop.sale_event e CROSS JOIN bound \
         WHERE e.network = $1 AND e.kind = 'bid_created' AND (e.block_number, e.log_index) > ($2, $3) AND e.block_number <= bound.upto \
         ORDER BY e.block_number, e.log_index LIMIT $5",
        ready = chain_walk::ready_sql("c", "$6"),
        key = notice_cursor::tx_log_key(
            "sale_event",
            "e",
            "(e.kind || ':' || e.contract_address || ':' || e.token_id || ':' || coalesce(e.bid_id, ''))",
        )
    )
}

/// Whether a bid holds the cursor at `now`: its collection item may still
/// be mirrored, or its owner at the bid awaits the contract's backfill.
fn holds(
    row: &BidRow,
    described: bool,
    items_mirror_enabled: bool,
    items_pass: Option<i64>,
    now: i64,
) -> bool {
    if !row.collection {
        return false;
    }
    (!described && item_awaited(items_mirror_enabled, items_pass, row.block_time, now))
        || (row.backfill_owed && now.saturating_sub(row.block_time) < ITEM_HOLD_MAX_SECS)
}

/// The notification of one bid; `None` off a Decentraland collection,
/// without a known owner at the bid or for the owner's own bid.
fn notice(settings: &Settings, row: &BidRow, asset: &AssetMeta) -> Option<NewNotification> {
    if !row.collection || row.backfill_owed {
        return None;
    }
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
    if let Some(why) = start(pool, now).await? {
        return Ok(Pass::Waiting(why));
    }
    pages(pool, settings, now).await
}

/// Why the pass reads nothing, or `None` once both indexes are ready, and
/// the first seed at the sales index's published block. Each page reads its
/// own bound ([`rows_sql`]); these checks only report and seed.
async fn start(pool: &PgPool, now: DateTime<Utc>) -> anyhow::Result<Option<&'static str>> {
    let Some(sales) = chain_walk::ready_cursor(pool, INDEXER_SALES, NET, now).await else {
        return Ok(Some("the matic sales index is not published"));
    };
    if chain_walk::ready_cursor(pool, INDEXER_NFT, NET, now)
        .await
        .is_none()
    {
        return Ok(Some(
            "the matic nft index is not published (a bid's owner is unknown)",
        ));
    }
    let published = sales.folded_block.unwrap_or(sales.last_block);
    notice_cursor::seed(pool, PRODUCER, None, published, i32::MAX).await?;
    Ok(None)
}

/// The pages after the cursor, each under its own published bound.
async fn pages(pool: &PgPool, settings: &Settings, now: DateTime<Utc>) -> anyhow::Result<Pass> {
    let network = NET.db_name();
    let mut emitted = 0;
    loop {
        let mut txn = pool.begin().await?;
        let (_, since_block, since_log) = notice_cursor::lock(&mut txn, PRODUCER).await?;
        let rows: Vec<BidRow> = sqlx::query_as(sqlx::AssertSqlSafe(rows_sql()))
            .bind(network)
            .bind(since_block)
            .bind(since_log)
            .bind([INDEXER_SALES, INDEXER_NFT])
            .bind(PAGE_SIZE)
            .bind(now.naive_utc())
            .fetch_all(&mut *txn)
            .await?;
        if rows.is_empty() {
            txn.rollback().await?;
            break;
        }
        let keys: Vec<(String, String)> = rows
            .iter()
            .filter(|r| r.collection)
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
            let described = metas.contains_key(&(r.contract_address.clone(), r.token_id.clone()));
            holds(r, described, settings.items_mirror_enabled, items_pass, ts)
        });
        let ready = &rows[..held.unwrap_or(rows.len())];
        let Some(last) = ready.last() else {
            tracing::debug!(
                block = rows[0].block_number,
                log = rows[0].log_index,
                "bid on a collection token not described yet or owing a backfill: held"
            );
            txn.rollback().await?;
            break;
        };
        let (done_block, done_log) = (last.block_number, last.log_index);
        // A ready collection bid still owing a backfill passed its hold bound.
        for r in ready.iter().filter(|r| r.collection && r.backfill_owed) {
            tracing::warn!(
                block = r.block_number,
                log = r.log_index,
                contract = %r.contract_address,
                token = %r.token_id,
                "bid_received: the contract's nft backfill outlasted the hold; the bid is not announced"
            );
        }
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
    /// An ERC721 outside Decentraland's collections.
    const OTHER: &str = "0x00000000000000000000000000000000000000f6";
    const BID: &str = super::super::sales_index::BID_V2_MATIC;
    const T0: i64 = 1_700_000_000;

    fn row(collection: bool, backfill_owed: bool, block_time: i64) -> BidRow {
        BidRow {
            id_key: String::new(),
            block_number: 1,
            log_index: 1,
            block_time,
            contract_address: COLLECTION.into(),
            token_id: "7".into(),
            bidder: BIDDER.into(),
            seller: None,
            price: "1".into(),
            collection,
            backfill_owed,
        }
    }

    #[test]
    fn only_a_collection_bid_holds_and_never_past_the_bound() {
        let t = T0;
        assert!(holds(&row(true, false, t), false, true, None, t + 60));
        assert!(
            !holds(&row(true, false, t), true, true, None, t + 60),
            "described"
        );
        assert!(
            !holds(&row(false, true, t), false, true, None, t + 60),
            "another contract never holds"
        );
        assert!(
            holds(&row(true, true, t), true, false, None, t + 60),
            "a backfill holds with the mirror off"
        );
        assert!(
            !holds(
                &row(true, true, t),
                true,
                false,
                None,
                t + ITEM_HOLD_MAX_SECS
            ),
            "a stalled backfill"
        );
        assert_ne!(
            notification_id("matic", "0xa:0:k"),
            notice_cursor::notification_id("bid-accepted", "matic", "0xa:0:k")
        );
    }

    fn settings(items_mirror_enabled: bool) -> Settings {
        Settings {
            marketplace: "https://market.example".into(),
            land_images: "https://api.example".into(),
            items_mirror_enabled,
        }
    }

    /// A `shop.nft` row of `COLLECTION`'s item 0 owned by `owner`.
    async fn nft(pool: &PgPool, token: i64, owner: &str, minted: i64, transferred: i64) {
        sqlx::query(
            "INSERT INTO shop.nft (network, contract_address, token_id, owner, item_id, issued_id, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
             VALUES ('matic', $1, $2, $3, 0, $2, $4, $6, $5, 0, $6)",
        )
        .bind(COLLECTION)
        .bind(token)
        .bind(owner)
        .bind(minted)
        .bind(transferred)
        .bind(T0)
        .execute(pool)
        .await
        .unwrap();
    }

    /// A transfer of `COLLECTION`'s `token` at `(block, log)`.
    async fn transfer(pool: &PgPool, block: i64, log: i32, token: i64, from: &str, to: &str) {
        sqlx::query(
            "INSERT INTO shop.nft_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, from_address, to_address) \
             VALUES ('matic', $1, $2, $3, 'transfer', $4, $4, $5, $6, $7)",
        )
        .bind(block)
        .bind(log)
        .bind(T0 + block)
        .bind(COLLECTION)
        .bind(token)
        .bind(from)
        .bind(to)
        .execute(pool)
        .await
        .unwrap();
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
        nft(pool, 7, NEW_OWNER, 10, 90).await;
        transfer(pool, 10, 0, 7, nft_zero(), OWNER).await;
        transfer(pool, 90, 0, 7, OWNER, NEW_OWNER).await;
    }

    fn nft_zero() -> &'static str {
        super::super::nft_index::ZERO_ADDRESS
    }

    /// A `BidCreated` row at `(block, log)` on `contract`'s `token` by `bidder`.
    async fn bid_on(
        pool: &PgPool,
        block: i64,
        log: i32,
        contract: &str,
        token: i64,
        bidder: &str,
        price: &str,
    ) {
        sqlx::query(
            "INSERT INTO shop.sale_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, bid_id, buyer, price, expires_at, tx_hash) \
             VALUES ('matic', $1, $2, $3, 'bid_created', $4, $5, $6, $7, $8, $9::numeric, $3 + 86400, $10)",
        )
        .bind(block)
        .bind(log)
        .bind(T0 + block)
        .bind(BID)
        .bind(contract)
        .bind(token)
        .bind(format!("0x{block:064x}"))
        .bind(bidder)
        .bind(price)
        .bind(format!("0x{:064x}", block + 1_000_000))
        .execute(pool)
        .await
        .unwrap();
    }

    /// A `BidCreated` row at `block` (log 1) on `COLLECTION`'s `token`.
    async fn bid(pool: &PgPool, block: i64, token: i64, bidder: &str, price: &str) {
        bid_on(pool, block, 1, COLLECTION, token, bidder, price).await;
    }

    /// Dates the bids at `block` a minute ago, within every hold's bound.
    async fn recent(pool: &PgPool, block: i64) {
        sqlx::query("UPDATE shop.sale_event SET block_time = $1 WHERE block_number = $2")
            .bind(Utc::now().timestamp() - 60)
            .bind(block)
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
    /// nothing before both indexes publish; a cursor seeded below the head
    /// announces the bids after it; reads stop at the lower published
    /// block; the owner's own bid notifies nobody; a rewound cursor inserts
    /// nothing twice; a bid on an undescribed collection token holds the
    /// cursor only while the items mirror may still bring it.
    #[tokio::test]
    async fn indexed_onchain_bids_notify_the_owner_once() {
        let Some(db) = PrivateDb::create("bidnotif").await else {
            return;
        };
        let pool = &db.pool;
        seed_assets(pool).await;
        let s = settings(false);
        let one_mana = "1000000000000000000";
        // A cutover seed (`catalyst shop notice-rewind`) just before block 60.
        notice_cursor::seed(pool, PRODUCER, None, 59, i32::MAX)
            .await
            .unwrap();

        bid(pool, 50, 7, BIDDER, one_mana).await; // before the seed
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

        publish(pool, INDEXER_NFT, "matic", 200, T0 + 200).await;
        assert_eq!(
            run_once(pool, &s, Utc::now()).await.unwrap(),
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

        // A recent bid on a collection token no index describes waits while the mirror runs.
        bid(pool, 190, 9, BIDDER, one_mana).await;
        recent(pool, 190).await;
        let mirrored = settings(true);
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

    /// The recipient is the owner at the bid and nothing else: a first pass
    /// seeds at the published block and announces no history; a transfer
    /// (a mint too) counts before the bid by log index within its block; a
    /// bid before any transfer notifies nobody, not the current owner; a
    /// bid on another ERC721 never holds the feed, and a page ready up to a
    /// held bid advances to it; a contract owing a backfill holds its
    /// recent bids until the backfill completes, then notifies the owner at
    /// the bid even though the bidder owns the token now.
    #[tokio::test]
    async fn the_owner_at_the_bid_is_never_guessed() {
        let Some(db) = PrivateDb::create("bidowner").await else {
            return;
        };
        let pool = &db.pool;
        seed_assets(pool).await;
        let (on, off) = (settings(true), settings(false));
        let one_mana = "1000000000000000000";

        bid(pool, 50, 7, BIDDER, one_mana).await; // history
        publish(pool, INDEXER_SALES, "matic", 100, T0 + 100).await;
        publish(pool, INDEXER_NFT, "matic", 100, T0 + 100).await;
        assert_eq!(
            run_once(pool, &on, Utc::now()).await.unwrap(),
            Pass::Emitted(0)
        );
        assert_eq!(
            cursor(pool).await,
            (100, i32::MAX),
            "seeded at the published block"
        );

        // Token 8: minted to OWNER at (120, 0), sold to NEW_OWNER at (130, 2).
        nft(pool, 8, NEW_OWNER, 120, 130).await;
        transfer(pool, 120, 0, 8, nft_zero(), OWNER).await;
        transfer(pool, 130, 2, 8, OWNER, NEW_OWNER).await;
        bid(pool, 115, 8, BIDDER, one_mana).await; // before the mint: nobody
        bid(pool, 120, 8, BIDDER, one_mana).await; // after the mint: OWNER
        bid(pool, 130, 8, BIDDER, one_mana).await; // before the sale: OWNER
        bid_on(pool, 140, 1, OTHER, 1, BIDDER, "1").await; // not a collection
        recent(pool, 140).await;
        bid(pool, 141, 7, BIDDER, one_mana).await; // NEW_OWNER
        bid(pool, 150, 9, BIDDER, one_mana).await; // undescribed: held
        recent(pool, 150).await;
        publish(pool, INDEXER_SALES, "matic", 200, T0 + 200).await;
        publish(pool, INDEXER_NFT, "matic", 200, T0 + 200).await;

        assert_eq!(
            run_once(pool, &on, Utc::now()).await.unwrap(),
            Pass::Emitted(3)
        );
        assert_eq!(
            cursor(pool).await,
            (141, 1),
            "past the other contract's bid, held at the undescribed one"
        );
        let to: Vec<String> = stored(pool).await.into_iter().map(|r| r.0).collect();
        assert_eq!(to, [OWNER, OWNER, NEW_OWNER]);
        assert_eq!(
            run_once(pool, &off, Utc::now()).await.unwrap(),
            Pass::Emitted(0)
        );
        assert_eq!(cursor(pool).await, (150, 1));

        // COLLECTION owes a backfill; the bidder has since bought token 7.
        sqlx::query(
            "INSERT INTO shop.nft_contract (network, contract_address, walked_from_block, backfill_from_block) VALUES ('matic', $1, 200, 5)",
        )
        .bind(COLLECTION)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query("UPDATE shop.nft SET owner = $1 WHERE token_id = 7")
            .bind(BIDDER)
            .execute(pool)
            .await
            .unwrap();
        bid(pool, 155, 7, BIDDER, one_mana).await; // older than the bound: passed
        bid(pool, 160, 7, BIDDER, "2000000000000000000").await;
        recent(pool, 160).await;
        assert_eq!(
            run_once(pool, &off, Utc::now()).await.unwrap(),
            Pass::Emitted(0)
        );
        assert_eq!(
            cursor(pool).await,
            (155, 1),
            "held while the backfill is owed"
        );
        assert_eq!(stored(pool).await.len(), 3);

        sqlx::query(
            "UPDATE shop.nft_contract SET backfilled_to_block = 199 WHERE contract_address = $1",
        )
        .bind(COLLECTION)
        .execute(pool)
        .await
        .unwrap();
        assert_eq!(
            run_once(pool, &off, Utc::now()).await.unwrap(),
            Pass::Emitted(1)
        );
        assert_eq!(cursor(pool).await, (160, 1));
        let rows = stored(pool).await;
        assert_eq!(rows.len(), 4);
        assert_eq!(
            rows[3].0, NEW_OWNER,
            "the owner at the bid, not the bidder who owns it now"
        );
        assert_eq!(rows[3].1["seller"], NEW_OWNER);
        db.drop().await;
    }

    /// An nft rewind that commits after the pass checked both indexes, on
    /// another connection, never pairs a bid above the fork with the
    /// truncated transfers: the page reads the lowered bound (or the
    /// withdrawn index) with its bids, leaves the cursor in place, and once
    /// the nft index walks and publishes again only the owner at the bid
    /// hears of it.
    #[tokio::test]
    async fn a_rewind_after_the_checks_never_pairs_a_bid_with_truncated_transfers() {
        let Some(db) = PrivateDb::create("bidrewind").await else {
            return;
        };
        let pool = &db.pool;
        seed_assets(pool).await; // minted to OWNER at 10, sold to NEW_OWNER at 90
        let s = settings(false);
        notice_cursor::seed(pool, PRODUCER, None, 59, i32::MAX)
            .await
            .unwrap();
        bid(pool, 95, 7, BIDDER, "1000000000000000000").await; // owner: NEW_OWNER
        publish(pool, INDEXER_SALES, "matic", 200, T0 + 200).await;
        publish(pool, INDEXER_NFT, "matic", 200, T0 + 200).await;
        assert_eq!(start(pool, Utc::now()).await.unwrap(), None, "both ready");

        // On another connection, as a divergence runs: the publication is
        // withdrawn first (the rows still the old chain's), then the
        // recovery rewinds to the fork at 80 and publishes there.
        let mut other = pool.acquire().await.unwrap();
        sqlx::query(
            "UPDATE shop.chain_cursor SET folded_block = NULL, folded_at = NULL, diverged_at = timezone('UTC', now()) \
             WHERE indexer = 'nft' AND network = 'matic'",
        )
        .execute(&mut *other)
        .await
        .unwrap();
        assert_eq!(pages(pool, &s, Utc::now()).await.unwrap(), Pass::Emitted(0));
        assert_eq!(cursor(pool).await, (59, i32::MAX), "withdrawn: no bound");
        let mut rewind = sqlx::Connection::begin(&mut *other).await.unwrap();
        sqlx::query("DELETE FROM shop.nft_event WHERE network = 'matic' AND block_number > 80")
            .execute(&mut *rewind)
            .await
            .unwrap();
        sqlx::query(
            "UPDATE shop.chain_cursor SET last_block = 80, last_block_hash = NULL, fold_through = 80, folded_block = 80, \
             folded_at = timezone('UTC', now()), diverged_at = NULL WHERE indexer = 'nft' AND network = 'matic'",
        )
        .execute(&mut *rewind)
        .await
        .unwrap();
        rewind.commit().await.unwrap();
        drop(other);
        assert_eq!(pages(pool, &s, Utc::now()).await.unwrap(), Pass::Emitted(0));
        assert_eq!(
            cursor(pool).await,
            (59, i32::MAX),
            "the bid above the fork waits"
        );
        assert!(
            stored(pool).await.is_empty(),
            "nobody guessed from either history"
        );

        // Walked and published again: the owner at the bid, once.
        transfer(pool, 90, 0, 7, OWNER, NEW_OWNER).await;
        publish(pool, INDEXER_NFT, "matic", 200, T0 + 200).await;
        assert_eq!(
            run_once(pool, &s, Utc::now()).await.unwrap(),
            Pass::Emitted(1)
        );
        let rows = stored(pool).await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, NEW_OWNER);
        db.drop().await;
    }
}
