//! Historical names stay at their existing values until the complete old
//! prefix is in nft_event. The last window refolds the registry atomically.

use super::*;
use crate::chain_index::rpc::BlockHeader;
use chain_walk::{LogSource, Owner};
use std::time::Duration;

const WORKER: &str = "nft_estate_creations";
/// Ethereum deployment of the Estate proxy, transaction 0x0c784545242537b44d24029f9c843973cb7f37256ede544af373de5cb758a1b9.
const DEPLOYMENT: i64 = 6_236_547;

/// The forward window and its marker commit together. A pre-CreateEstate
/// writer advances only the cursor. When its checkpoint differs, reopen
/// the registry prefix through that cursor, even after a completed repair.
pub(super) async fn record(
    conn: &mut PgConnection,
    previous: (i64, Option<&str>),
    next: (i64, Option<&str>),
) -> sqlx::Result<()> {
    let checkpoint = |(block, hash): (i64, Option<&str>)| {
        [block.to_string(), hash.unwrap_or_default().to_owned()]
    };
    sqlx::query(
        "INSERT INTO shop.worker_progress AS p (worker, scope, position, ceiling, key_ceiling)
        VALUES ($1, 1, least($2::bigint - 1, $3), $3, $5)
        ON CONFLICT (worker, scope) DO UPDATE SET
          position = CASE WHEN p.key_ceiling = $4 THEN p.position ELSE EXCLUDED.position END,
          ceiling = CASE WHEN p.key_ceiling = $4 THEN p.ceiling ELSE EXCLUDED.ceiling END,
          key_position = CASE WHEN p.key_ceiling = $4 THEN p.key_position ELSE '{}'::text[] END,
          key_ceiling = EXCLUDED.key_ceiling",
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

pub(super) async fn rewind(conn: &mut PgConnection, to: i64) -> sqlx::Result<()> {
    // Preserve proven coverage below the rewind. If the previous cursor
    // was already unaccounted for, the next capture will reconcile it.
    sqlx::query("UPDATE shop.worker_progress p SET
        key_position = CASE WHEN position > $2 THEN '{}'::text[] ELSE key_position END,
        position = least(position, greatest($2, 0)), ceiling = least(ceiling, greatest($2, 0)),
        key_ceiling = CASE WHEN p.key_ceiling = ARRAY[c.last_block::text, coalesce(c.last_block_hash, '')]
          THEN ARRAY[$2::bigint::text, coalesce((SELECT block_hash FROM shop.chain_checkpoint
                    WHERE indexer=c.indexer AND network=c.network AND block_number=$2), '')]
          ELSE p.key_ceiling END
        FROM shop.chain_cursor c WHERE worker=$1 AND scope=1 AND c.indexer='nft' AND c.network='ethereum'")
        .bind(WORKER).bind(to).execute(conn).await?;
    Ok(())
}

/// Check the historical window against its predecessor and the published
/// cursor. The endpoint can shorten the requested range, so persist the hash
/// at the actual end as the next window's predecessor.
async fn read_window(
    source: &dyn LogSource,
    (block, hash): (u64, &str),
    (position, position_hash): (i64, Option<&str>),
    to: u64,
    remaining: usize,
) -> anyhow::Result<(BlockHeader, BlockHeader, Vec<NftEvent>, usize)> {
    let chain = source.chain_id().await?;
    anyhow::ensure!(chain.value == 1, "estate repair requires Ethereum mainnet");
    let before = source.header(block).await?;
    anyhow::ensure!(
        before.value.hash == hash,
        "estate repair checkpoint diverged"
    );
    let lower = source.header(position as u64).await?;
    anyhow::ensure!(
        position_hash.is_none_or(|h| h == lower.value.hash),
        "estate repair predecessor changed"
    );
    let upper = source.header(to).await?;
    let from = position as u64 + 1;
    let group = [LogGroup {
        addresses: vec![ESTATE_REGISTRY_MAINNET.into()],
        topics: json!([[TOPIC_CREATE_ESTATE]]),
    }];
    let mut chunk = 1;
    let window = source.window(from, to, &group, &mut chunk).await?;
    source.lane_read();
    // A provider can briefly return logs from another branch between equal
    // endpoint headers. Check each event block and take its canonical time.
    // In a dense window, persist a shorter prefix to bound these extra reads.
    let mut rows = Vec::new();
    let mut event_requests = 0;
    let mut reached_block = window.reached;
    for (i, logs) in window
        .logs
        .chunk_by(|a, b| chain_walk::log_position(a).0 == chain_walk::log_position(b).0)
        .enumerate()
    {
        let number = chain_walk::log_position(&logs[0]).0 as u64;
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
                "estate creation is missing its canonical block hash"
            );
            if let Some(row) = decode_log(log, header.value.timestamp)? {
                rows.push(row);
            }
        }
    }
    let reached = source.header(reached_block).await?;
    let upper_after = if reached_block == to {
        None
    } else {
        Some(source.header(to).await?)
    };
    let end = upper_after.as_ref().unwrap_or(&reached);
    let lower_after = source.header(position as u64).await?;
    anyhow::ensure!(
        lower.value == lower_after.value && upper.value == end.value,
        "estate repair window bounds changed"
    );
    let after = source.header(block).await?;
    anyhow::ensure!(
        before.value == after.value,
        "chain changed while reading estate creations"
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

pub(super) async fn step(
    pool: &PgPool,
    source: &dyn LogSource,
    owner: &Owner,
    window_blocks: u64,
    remaining: usize,
) -> anyhow::Result<usize> {
    let progress = || {
        sqlx::query_as::<_, (i64, i64, Option<String>)>(
            "SELECT position, ceiling, key_position[1] FROM shop.worker_progress p JOIN shop.chain_cursor c
             ON p.key_ceiling = ARRAY[c.last_block::text, coalesce(c.last_block_hash, '')]
             WHERE worker = $1 AND scope = 1 AND c.indexer='nft' AND c.network='ethereum' AND c.chain_id=1",
        )
        .bind(WORKER)
    };
    let (position, ceiling, position_hash) = match progress().fetch_optional(pool).await? {
        Some(row) => row,
        None => {
            let mut tx = pool.begin().await?;
            let (block, hash): (i64, Option<String>) = sqlx::query_as(
                "SELECT last_block, last_block_hash FROM shop.chain_cursor WHERE indexer='nft' AND network='ethereum'
                 AND chain_id=1 AND folded_block=last_block FOR UPDATE")
                .fetch_one(&mut *tx).await?;
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
    let mut tx = pool.begin().await?;
    let (block, hash): (i64, String) = sqlx::query_as(
        "SELECT last_block, last_block_hash FROM shop.chain_cursor WHERE indexer='nft' AND network='ethereum'
         AND folded_block=last_block AND last_block >= $1 AND owner_generation=$2 AND chain_id=1
         AND last_block_hash IS NOT NULL AND quarantined_at IS NULL")
        .bind(ceiling).bind(owner.generation).fetch_one(&mut *tx).await?;
    let to = (ceiling as u64).min((position as u64).saturating_add(window_blocks));
    // All reads run after publication, in a transaction that holds no row
    // locks during network I/O. Abandon a slow archive and retry its window.
    let (lower, end, rows, requests) = tokio::time::timeout(
        Duration::from_secs(30),
        read_window(
            source,
            (block as u64, &hash),
            (position, position_hash.as_deref()),
            to,
            remaining,
        ),
    )
    .await
    .context("estate repair archive timed out")??;
    // Old endpoints may already have aged out of the walk's 64 retained
    // checkpoints. Verify any retained bounds as well as the published tip.
    let conflicts: bool = sqlx::query_scalar(
        "SELECT EXISTS (
        SELECT 1 FROM shop.chain_checkpoint WHERE indexer='nft' AND network='ethereum'
        AND ((block_number=$1 AND block_hash<>$2) OR (block_number=$3 AND block_hash<>$4)))",
    )
    .bind(position)
    .bind(&lower.hash)
    .bind(end.number as i64)
    .bind(&end.hash)
    .fetch_one(&mut *tx)
    .await?;
    anyhow::ensure!(
        !conflicts,
        "estate repair disagrees with a retained checkpoint"
    );
    let advanced = sqlx::query(
        "UPDATE shop.worker_progress SET position=$4, key_position=ARRAY[$5::text]
        WHERE worker=$1 AND scope=1 AND position=$2 AND ceiling=$3",
    )
    .bind(WORKER)
    .bind(position)
    .bind(ceiling)
    .bind(end.number as i64)
    .bind(&end.hash)
    .execute(&mut *tx)
    .await?;
    anyhow::ensure!(advanced.rows_affected() == 1, "estate repair range changed");
    insert_events(&mut tx, ChainNet::Ethereum, &rows).await?;
    if end.number as i64 == ceiling {
        // A newer valid Update still wins, including when an old fold had
        // previously overwritten it with a malformed document.
        sqlx::query("DELETE FROM shop.land_data WHERE token_address=$1")
            .bind(ESTATE_REGISTRY_MAINNET)
            .execute(&mut *tx)
            .await?;
        fold_registry_data(&mut tx, -1, Some(ESTATE_REGISTRY_MAINNET)).await?;
    }
    // A concurrent rewind can invalidate the prefix even when this worker
    // still holds the same owner generation.
    sqlx::query(
        "SELECT 1 FROM shop.chain_cursor WHERE indexer='nft' AND network='ethereum'
        AND last_block=$1 AND folded_block=last_block AND last_block_hash=$2 FOR UPDATE",
    )
    .bind(block)
    .bind(hash)
    .fetch_one(&mut *tx)
    .await?;
    owner.check(&mut tx).await?;
    tx.commit().await?;
    Ok(requests)
}
