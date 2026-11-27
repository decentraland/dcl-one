//! The Ethereum ERC721Bid lifecycle below the sales walk's first window
//! that read it. The Ethereum sales walk read only `BidAccepted` (sales
//! accounting) before it also read `BidCreated` and `BidCancelled`, so a
//! cursor past the bid contract's deployment lacks the earlier creations and
//! cancellations. This repair fills them in bounded windows after each
//! publication, with the walk's spare request budget, and refolds every
//! Ethereum bid in the transaction of its last window. The Ethereum bid book
//! is read only while the repair is complete and explains the cursor
//! ([`complete_on`]); sales accounting never waits on it.
//!
//! It follows the estate creations repair (`nft_index::estate_repair`): one
//! `shop.worker_progress` row keyed to the cursor's block, hash and
//! `reorg_rewinds`, archive windows bracketed by the cursor's checkpoint
//! header, the predecessor hash checked, and each event's block hash
//! canonical.

use std::time::Duration;

use anyhow::Context;
use serde_json::json;
use sqlx::{PgConnection, PgPool};

use super::casts;
use super::chain_walk::{self, ChainNet, LogGroup, LogSource, Owner};
use super::{ethereum_sales, sales_index};
use crate::chain_index::rpc::BlockHeader;

const WORKER: &str = "ethereum_bid_history";
/// ERC721Bid's mainnet deployment block: marketplace-squid-core
/// 85c6832e96e5459b37f0f315b8e384693684e12d, src/eth/data/contracts/start-blocks.ts.
pub const DEPLOYMENT: i64 = 7_270_906;

/// The forward window and its marker commit together. A writer that walks
/// the bid contract's `BidAccepted` only advances just the cursor.
/// `key_ceiling` holds the forward block and hash; `key_position` the
/// archive predecessor hash and the rewind count that justified the prefix.
/// A marker that does not explain the cursor reopens the whole repair.
pub(super) async fn record(
    conn: &mut PgConnection,
    previous: (i64, Option<&str>),
    next: (i64, Option<&str>),
) -> sqlx::Result<()> {
    let checkpoint = |(block, hash): (i64, Option<&str>)| {
        [block.to_string(), hash.unwrap_or_default().to_owned()]
    };
    sqlx::query(
        "INSERT INTO shop.worker_progress AS p (worker, scope, position, ceiling, key_position, key_ceiling)
         SELECT $1, 1, least($2::bigint - 1, $3), $3, ARRAY[NULL::text, c.reorg_rewinds::text], $5
         FROM shop.chain_cursor c WHERE c.indexer='sales' AND c.network='ethereum'
         ON CONFLICT (worker, scope) DO UPDATE SET (position, ceiling, key_position, key_ceiling) = (
             SELECT CASE WHEN covered THEN p.position ELSE EXCLUDED.position END,
                    CASE WHEN covered THEN p.ceiling ELSE EXCLUDED.ceiling END,
                    CASE WHEN covered THEN p.key_position ELSE EXCLUDED.key_position END,
                    EXCLUDED.key_ceiling
             FROM (SELECT p.key_ceiling=$4 AND p.key_position[2]=EXCLUDED.key_position[2] AS covered) proof
         )",
    )
    .bind(WORKER)
    .bind(DEPLOYMENT)
    .bind(previous.0.max(0))
    .bind(checkpoint(previous).as_slice())
    .bind(checkpoint(next).as_slice())
    .execute(conn)
    .await?;
    Ok(())
}

/// The marker after the walk rewinds to `to`, in its transaction. Only a
/// marker that explains the cursor keeps its prefix; the walker counts the
/// rewind in this transaction after the cleanup. A repair the rewind
/// completes needs no refold here: the rewind clears the fold marker, so the
/// next fold rebuilds every bid before the book publishes again.
pub(super) async fn rewind(conn: &mut PgConnection, to: i64) -> anyhow::Result<()> {
    sqlx::query(
        "WITH retained AS (
            SELECT (c.reorg_rewinds + 1)::text AS next_rewind,
                   (SELECT block_hash FROM shop.chain_checkpoint
                    WHERE indexer=c.indexer AND network=c.network AND block_number=$2) AS kept_hash
            FROM shop.worker_progress p JOIN shop.chain_cursor c
              ON p.key_ceiling=ARRAY[c.last_block::text, coalesce(c.last_block_hash, '')]
             AND p.key_position[2]=c.reorg_rewinds::text
            WHERE p.worker=$1 AND p.scope=1 AND c.indexer='sales' AND c.network='ethereum'
            FOR UPDATE OF p
         )
         UPDATE shop.worker_progress p SET
           key_position=ARRAY[CASE WHEN p.position <= $2 THEN p.key_position[1] END, r.next_rewind],
           position=least(p.position,greatest($2,0)), ceiling=least(p.ceiling,greatest($2,0)),
           key_ceiling=ARRAY[$2::bigint::text, coalesce(r.kept_hash,'')]
         FROM retained r WHERE p.worker=$1 AND p.scope=1",
    )
    .bind(WORKER)
    .bind(to)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Whether the Ethereum bid lifecycle is whole below the sales cursor, read
/// on `conn` (a reader's snapshot): the repair reached its ceiling and its
/// marker explains the cursor (block, hash, rewinds).
pub async fn complete_on(conn: &mut PgConnection) -> sqlx::Result<bool> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM shop.worker_progress p JOIN shop.chain_cursor c
           ON p.key_ceiling = ARRAY[c.last_block::text, coalesce(c.last_block_hash, '')]
          AND p.key_position[2] = c.reorg_rewinds::text
          WHERE p.worker = $1 AND p.scope = 1 AND p.position >= p.ceiling
            AND c.indexer = 'sales' AND c.network = 'ethereum')",
    )
    .bind(WORKER)
    .fetch_one(conn)
    .await
}

/// One archive window `(position, to]` of the bid contract's creations and
/// cancellations, checked against its predecessor and the published cursor.
/// The endpoint can shorten the requested range, so the hash at the actual
/// end is the next window's predecessor.
async fn read_window(
    source: &dyn LogSource,
    (block, hash): (u64, &str),
    (position, position_hash): (i64, Option<&str>),
    to: u64,
    remaining: usize,
) -> anyhow::Result<(BlockHeader, BlockHeader, Vec<sales_index::SaleEvent>, usize)> {
    let chain = source.chain_id().await?;
    anyhow::ensure!(
        chain.value == ChainNet::Ethereum.chain_id_u64(),
        "the bid history repair reads Ethereum mainnet"
    );
    let before = source.header(block).await?;
    anyhow::ensure!(before.value.hash == hash, "bid history checkpoint diverged");
    let lower = source.header(casts::to_u64(position)?).await?;
    anyhow::ensure!(
        position_hash.is_none_or(|h| h == lower.value.hash),
        "bid history predecessor changed"
    );
    let upper = source.header(to).await?;
    let from = casts::to_u64(position)? + 1;
    let group = [LogGroup {
        addresses: vec![ethereum_sales::ERC721_BID.into()],
        topics: json!([[
            sales_index::TOPIC_BID_CREATED,
            sales_index::TOPIC_BID_CANCELLED
        ]]),
    }];
    let mut chunk = 1;
    let window = source.window(from, to, &group, &mut chunk).await?;
    source.lane_read();
    // A provider can briefly return logs from another branch between equal
    // endpoint headers: check each event block and take its canonical time.
    // A dense window persists a shorter prefix to bound these reads.
    let mut rows = Vec::new();
    let mut event_requests = 0;
    let mut reached_block = window.reached;
    for (i, logs) in window
        .logs
        .chunk_by(|a, b| chain_walk::log_position(a).0 == chain_walk::log_position(b).0)
        .enumerate()
    {
        let number = casts::to_u64(chain_walk::log_position(&logs[0]).0)?;
        if i == remaining {
            reached_block = number - 1;
            break;
        }
        let header = source.header(number).await?;
        event_requests += header.requests;
        for log in logs {
            anyhow::ensure!(
                log.block_hash
                    .as_deref()
                    .is_some_and(|h| h.eq_ignore_ascii_case(&header.value.hash)),
                "a bid event is missing its canonical block hash"
            );
            // As the forward walk: an event it cannot decode fails the
            // window, rather than leave its bid out of a history the repair
            // would then call complete.
            let row = sales_index::decode_log(log, header.value.timestamp).map_err(|error| {
                let (block, log_index) = chain_walk::log_position(log);
                anyhow::anyhow!("an undecodable bid event at {block}:{log_index}: {error}")
            })?;
            rows.extend(row);
        }
    }
    let reached = source.header(reached_block).await?;
    let upper_after = if reached_block == to {
        None
    } else {
        Some(source.header(to).await?)
    };
    let end = upper_after.as_ref().unwrap_or(&reached);
    let lower_after = source.header(casts::to_u64(position)?).await?;
    anyhow::ensure!(
        lower.value == lower_after.value && upper.value == end.value,
        "bid history window bounds changed"
    );
    let after = source.header(block).await?;
    anyhow::ensure!(
        before.value == after.value,
        "chain changed while reading bid history"
    );
    let requests = chain.requests
        + before.requests
        + lower.requests
        + upper.requests
        + window.requests
        + event_requests
        + reached.requests
        + lower_after.requests
        + upper_after.map_or(0, |h| h.requests)
        + after.requests;
    Ok((lower.value, reached.value, rows, requests))
}

/// One repair window after a publication of the Ethereum sales walk: the
/// requests it sent. The last window refolds every Ethereum bid in its
/// transaction, so the book never holds a fold that missed a repaired
/// creation once the repair reads complete.
pub(super) async fn step(
    pool: &PgPool,
    source: &dyn LogSource,
    owner: &Owner,
    window_blocks: u64,
    remaining: usize,
) -> anyhow::Result<usize> {
    let progress = || {
        sqlx::query_as::<_, (i64, i64, Option<String>, i32)>(
            "SELECT position, ceiling, key_position[1], c.reorg_rewinds FROM shop.worker_progress p JOIN shop.chain_cursor c
             ON p.key_ceiling = ARRAY[c.last_block::text, coalesce(c.last_block_hash, '')]
             AND p.key_position[2]=c.reorg_rewinds::text
             WHERE worker = $1 AND scope = 1 AND c.indexer='sales' AND c.network='ethereum' AND c.chain_id=1",
        )
        .bind(WORKER)
    };
    let (position, ceiling, position_hash, rewinds) = match progress().fetch_optional(pool).await? {
        Some(row) => row,
        None => {
            // No marker explains the published cursor (an earlier binary
            // walked it): the whole lifecycle below it is owed.
            let mut tx = pool.begin().await?;
            let (block, hash): (i64, Option<String>) = sqlx::query_as(
                "SELECT last_block, last_block_hash FROM shop.chain_cursor WHERE indexer='sales' AND network='ethereum'
                 AND chain_id=1 AND folded_block=last_block FOR UPDATE",
            )
            .fetch_one(&mut *tx)
            .await?;
            let tip = (block, hash.as_deref());
            record(&mut tx, tip, tip).await?;
            let row = progress().fetch_one(&mut *tx).await?;
            owner.check(&mut tx).await?;
            tx.commit().await?;
            row
        }
    };
    if position >= ceiling {
        return Ok(0);
    }
    // No transaction stays open while the archive answers. The bracket's
    // requests come first; the rest bounds the per-event header checks.
    let (block, hash): (i64, String) = sqlx::query_as(
        "SELECT last_block, last_block_hash FROM shop.chain_cursor WHERE indexer='sales' AND network='ethereum'
         AND folded_block=last_block AND last_block >= $1 AND owner_generation=$2 AND reorg_rewinds=$3 AND chain_id=1
         AND last_block_hash IS NOT NULL AND quarantined_at IS NULL",
    )
    .bind(ceiling)
    .bind(owner.generation)
    .bind(rewinds)
    .fetch_one(pool)
    .await?;
    let to = casts::to_u64(ceiling)?.min(casts::to_u64(position)?.saturating_add(window_blocks));
    let (lower, end, rows, requests) = tokio::time::timeout(
        Duration::from_secs(30),
        read_window(
            source,
            (casts::to_u64(block)?, &hash),
            (position, position_hash.as_deref()),
            to,
            remaining,
        ),
    )
    .await
    .context("bid history archive timed out")??;
    let mut tx = pool.begin().await?;
    // The cursor before the marker, as the forward commit locks them, and a
    // cursor that moved during the reads is refused.
    let current: (i64, String) = sqlx::query_as(
        "SELECT last_block, last_block_hash FROM shop.chain_cursor WHERE indexer='sales' AND network='ethereum'
         AND folded_block=last_block AND last_block=$1 AND last_block_hash=$2 AND owner_generation=$3
         AND reorg_rewinds=$4 AND chain_id=1 AND quarantined_at IS NULL FOR UPDATE",
    )
    .bind(block)
    .bind(&hash)
    .bind(owner.generation)
    .bind(rewinds)
    .fetch_one(&mut *tx)
    .await?;
    anyhow::ensure!(
        current == (block, hash.clone()),
        "published sales cursor changed during the bid history repair"
    );
    // An old endpoint may have aged out of the walk's 64 retained points;
    // a retained one must agree.
    let conflicts: bool = sqlx::query_scalar(
        "SELECT EXISTS (
        SELECT 1 FROM shop.chain_checkpoint WHERE indexer='sales' AND network='ethereum'
        AND ((block_number=$1 AND block_hash<>$2) OR (block_number=$3 AND block_hash<>$4)))",
    )
    .bind(position)
    .bind(&lower.hash)
    .bind(casts::to_i64(end.number)?)
    .bind(&end.hash)
    .fetch_one(&mut *tx)
    .await?;
    anyhow::ensure!(
        !conflicts,
        "the bid history repair disagrees with a retained checkpoint"
    );
    let advanced = sqlx::query(
        "UPDATE shop.worker_progress SET position=$4, key_position[1]=$5
        WHERE worker=$1 AND scope=1 AND position=$2 AND ceiling=$3 AND key_position[2]=$6::integer::text",
    )
    .bind(WORKER)
    .bind(position)
    .bind(ceiling)
    .bind(casts::to_i64(end.number)?)
    .bind(&end.hash)
    .bind(rewinds)
    .execute(&mut *tx)
    .await?;
    anyhow::ensure!(
        advanced.rows_affected() == 1,
        "bid history repair range changed"
    );
    sales_index::insert_events(&mut tx, ChainNet::Ethereum, &rows).await?;
    if u64::try_from(ceiling).ok() == Some(end.number) {
        // The forward folds never saw the repaired creations (they sit below
        // their `since`): every Ethereum bid is folded again from its whole
        // history before the marker reads complete.
        sales_index::fold_in(&mut tx, None, owner).await?;
    }
    owner.check(&mut tx).await?;
    tx.commit().await?;
    Ok(requests)
}
