//! Polygon purchase badges count paid CollectionStore mints, accepted ERC721
//! bids and completed Marketplace orders, once per wallet, badge and transaction.
//! A bid or order must have been created for an existing NFT. The two payment
//! relays use the token's owner at the end of the sale block.
//!
//! The job reads only through the published NFT and sale index watermarks.
//! Missing items wait for the items mirror. Older orders without a transaction
//! hash wait for the NFT index to refill it.

use std::collections::HashSet;

use sqlx::PgConnection;

use super::super::evaluators as ev;
use super::{Pass, award, completed, init_cursor, lock_cursor};
use crate::identity::Address;

pub(crate) use crate::shared::collections::{ITEM_SHIFT, ITEM_WAIT_SECS};
const ZERO_ADDRESS: &str = "0x0000000000000000000000000000000000000000";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Log {
    Sale,
    Order,
}

impl Log {
    pub const ALL: [Log; 2] = [Log::Sale, Log::Order];

    pub fn source(self) -> &'static str {
        match self {
            Log::Sale => "sales:sale_event:matic",
            Log::Order => "sales:nft_event:matic",
        }
    }

    fn rows_sql(self) -> String {
        // The collection emits the mint Transfer immediately before Issue. The token
        // must already exist when the marketplace creates its bid or order.
        let minted_before_creation = format!("EXISTS (
            SELECT 1 FROM shop.nft_event n
            WHERE n.network = c.network AND n.contract_address = c.contract_address
              AND n.token_id = c.token_id AND n.kind = 'transfer' AND n.from_address = '{ZERO_ADDRESS}'
              AND (n.block_number, n.log_index) < (c.block_number, c.log_index))");
        let (table, kinds, buyer, item_id, eligible) = match self {
            Log::Sale => (
                "shop.sale_event",
                "e.kind IN ('mint', 'bid_accepted')",
                "e.buyer",
                format!("CASE WHEN e.kind = 'mint' THEN e.item_id ELSE div(e.token_id, {ITEM_SHIFT}) END"),
                format!("CASE WHEN e.kind = 'mint' THEN e.price > 0 ELSE (
                    SELECT c.price > 0 FROM shop.sale_event c
                    WHERE c.network = e.network AND c.contract_address = e.contract_address
                      AND c.token_id = e.token_id AND c.buyer = e.buyer AND c.kind = 'bid_created'
                      AND c.emitter = '0xb96697fa4a3361ba35b774a42c58daccaad1b8e1'
                      AND c.block_number >= 22514900 AND c.emitter = e.emitter
                      AND (c.block_number, c.log_index) < (e.block_number, e.log_index)
                      AND {minted_before_creation}
                    ORDER BY c.block_number DESC, c.log_index DESC LIMIT 1) END"),
            ),
            Log::Order => (
                "shop.nft_event",
                "e.kind = 'order_successful'",
                "e.to_address",
                format!("div(e.token_id, {ITEM_SHIFT})"),
                format!("e.amount > 0 AND EXISTS (
                    SELECT 1 FROM shop.nft_event c
                    WHERE c.network = e.network AND c.contract_address = e.contract_address
                      AND c.token_id = e.token_id AND c.kind = 'order_created'
                      AND c.detail = e.detail AND c.emitter = e.emitter
                      AND ((c.emitter = '0x02080031b45a3c67d338dd4a2cc309d28756a160' AND c.block_number >= 15202000)
                        OR (c.emitter = '0x480a0f4e360e8964e68858dd231c2922f1df45ef' AND c.block_number >= 22514900))
                      AND (c.block_number, c.log_index) < (e.block_number, e.log_index)
                      AND {minted_before_creation})"),
            ),
        };
        format!(
            "SELECT e.block_number, e.log_index, e.block_time, e.tx_hash, i.item_type,
                    CASE WHEN {eligible} THEN CASE WHEN {buyer} IN (
                        '0xed038688ecf1193f8d9717eb3930f0bf0d745cb4',
                        '0xea749fd6ba492dbc14c24fe8a3d08769229b896c'
                    ) THEN (SELECT nullif(t.to_address, '{ZERO_ADDRESS}') FROM shop.nft_event t
                            WHERE t.network = e.network AND t.contract_address = e.contract_address
                              AND t.token_id = e.token_id AND t.block_number <= e.block_number AND t.kind = 'transfer'
                            ORDER BY t.block_number DESC, t.log_index DESC LIMIT 1)
                      ELSE {buyer} END END AS buyer
             FROM {table} e LEFT JOIN shop.items i ON i.network = e.network
               AND i.contract_address = e.contract_address AND i.item_id = {item_id}
             WHERE e.network = 'matic' AND {kinds}
               AND (e.block_number, e.log_index) > ($1, $2) AND e.block_number <= $3
             ORDER BY e.block_number, e.log_index LIMIT $4"
        )
    }
}

#[derive(sqlx::FromRow)]
struct Row {
    block_number: i64,
    log_index: i32,
    block_time: i64,
    buyer: Option<String>,
    tx_hash: Option<String>,
    item_type: Option<String>,
}

fn badge_of(item_type: &str) -> &'static str {
    if item_type == "emote_v1" {
        ev::EMOTIONISTA
    } else {
        ev::FASHIONISTA
    }
}

/// The block `log`'s index has published, when it has (shop's readiness:
/// full history folded through the last indexed block within 30 minutes,
/// bound to Polygon and out of quarantine).
async fn published(conn: &mut PgConnection, log: Log) -> Result<Option<i64>, sqlx::Error> {
    let indexes: &[&str] = match log {
        Log::Sale => &["sales", "nft"],
        Log::Order => &["nft"],
    };
    sqlx::query_scalar(
        "SELECT CASE WHEN count(*) = cardinality($1::text[]) THEN min(folded_block) END FROM shop.chain_cursor \
         WHERE indexer = ANY($1) AND network = 'matic' AND full_history \
           AND chain_id = 137 AND quarantined_at IS NULL \
           AND folded_block = last_block \
           AND folded_at <= timezone('UTC', now()) \
           AND folded_at >= timezone('UTC', now()) - interval '30 minutes'",
    )
    .bind(indexes)
    .fetch_one(conn)
    .await
}

pub(super) async fn run(pass: &Pass) -> anyhow::Result<u64> {
    let mut passed = 0;
    for log in Log::ALL {
        passed += super::drain(|| batch(pass, log)).await?;
    }
    Ok(passed)
}

pub(crate) async fn batch(pass: &Pass, log: Log) -> Result<(u64, bool), sqlx::Error> {
    let mut tx = pass.pool.begin().await?;
    let Some(until) = published(&mut tx, log).await? else {
        tx.rollback().await?;
        return Ok((0, false));
    };
    let Some((block, log_index)) = lock_cursor(&mut tx, log.source()).await? else {
        init_cursor(&mut tx, log.source(), until, i32::MAX).await?;
        tx.commit().await?;
        return Ok((0, false));
    };
    super::report_lag(log.source(), until - block);
    let limit = pass.batch();
    let rows: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(log.rows_sql()))
        .bind(block)
        .bind(log_index)
        .bind(until)
        .bind(limit)
        .fetch_all(&mut *tx)
        .await?;
    if rows.is_empty() {
        tx.rollback().await?;
        return Ok((0, false));
    }
    // The last complete items pass, unix seconds.
    let items_pass: Option<i64> = sqlx::query_scalar(
        "SELECT extract(epoch FROM last_complete_pass_at)::bigint FROM shop.mirror_state \
         WHERE mirror = 'items' AND network = 'matic'",
    )
    .fetch_optional(&mut *tx)
    .await?
    .flatten();
    // (buyer, transaction hash, sale time in milliseconds, badge)
    let mut sales: Vec<(String, String, i64, &'static str)> = Vec::new();
    let now = crate::shared::now_utc();
    let mut passed = 0;
    for row in &rows {
        let at = chrono::DateTime::from_timestamp(row.block_time, 0)
            .map(|t| t.naive_utc())
            .unwrap_or(now);
        let pending = row.buyer.is_some() && !pass.too_old(at, now);
        if pending && row.item_type.is_some() && row.tx_hash.is_none() {
            tracing::warn!(
                block = row.block_number,
                log = row.log_index,
                "badge sale awaits indexed transaction hash"
            );
            break;
        }
        if pending && row.item_type.is_none() {
            let waited = items_pass
                .is_some_and(|done| done >= row.block_time.saturating_add(ITEM_WAIT_SECS));
            if !waited {
                tracing::debug!(
                    source = log.source(),
                    block = row.block_number,
                    log = row.log_index,
                    "sale of an item not mirrored yet: held"
                );
                break;
            }
            tracing::debug!(
                source = log.source(),
                block = row.block_number,
                log = row.log_index,
                "sale of an item the mirror never held: passed"
            );
        }
        passed += 1;
        if pass.too_old(at, now) {
            continue;
        }
        if let (Some(buyer), Some(tx_hash), Some(item_type)) =
            (&row.buyer, &row.tx_hash, &row.item_type)
        {
            sales.push((
                buyer.to_ascii_lowercase(),
                tx_hash.to_ascii_lowercase(),
                row.block_time.saturating_mul(1000),
                badge_of(item_type),
            ));
        }
    }
    let Some(last) = rows[..passed].last().map(|r| (r.block_number, r.log_index)) else {
        tx.rollback().await?;
        return Ok((0, false));
    };
    let addresses: Vec<String> = sales
        .iter()
        .map(|s| s.0.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let done = completed(&mut tx, &addresses).await?;
    let notify = Some(pass.asset_base.as_str());
    for (buyer, tx_hash, sale_at, badge) in &sales {
        let Ok(address) = Address::parse(buyer) else {
            continue;
        };
        if done.contains(&(buyer.clone(), badge.to_string())) {
            continue;
        }
        award(&mut tx, &address, badge, notify, |p| {
            ev::buyer(p, badge, tx_hash, *sale_at)
        })
        .await?;
    }
    super::advance(&mut tx, log.source(), last.0, last.1).await?;
    tx.commit().await?;
    super::report_lag(log.source(), until - last.0);
    Ok((
        passed as u64,
        passed == rows.len() && rows.len() as i64 == limit,
    ))
}
