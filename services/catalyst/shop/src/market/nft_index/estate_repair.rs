//! Historical names stay at their existing values until the complete old
//! prefix is in nft_event. The last window refolds the registry atomically.

use super::*;
use chain_walk::{LogSource, Owner};

const WORKER: &str = "nft_estate_creations";
/// Ethereum deployment of the Estate proxy, transaction 0x0c784545242537b44d24029f9c843973cb7f37256ede544af373de5cb758a1b9.
const DEPLOYMENT: i64 = 6_236_547;

/// Capture the cursor before the first forward write of the upgraded walk,
/// or at its first publication if it was already caught up. A fresh walk
/// has no old prefix to repair. Its completed marker prevents a second scan.
pub(super) async fn initialize(conn: &mut PgConnection) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO shop.worker_progress (worker, scope, position, ceiling)
        SELECT $1, 1, least($2::bigint - 1, greatest(last_block, 0)), greatest(last_block, 0)
        FROM shop.chain_cursor WHERE indexer = 'nft' AND network = 'ethereum' AND chain_id = 1
        ON CONFLICT DO NOTHING",
    )
    .bind(WORKER)
    .bind(DEPLOYMENT)
    .execute(conn)
    .await?;
    Ok(())
}

pub(super) async fn rewind(conn: &mut PgConnection, to: i64) -> sqlx::Result<()> {
    // Everything above the rewind will be read by the ordinary forward walk.
    sqlx::query("UPDATE shop.worker_progress SET position = least(position, $2), ceiling = least(ceiling, $2)
        WHERE worker = $1 AND scope = 1 AND ceiling > $2")
        .bind(WORKER).bind(to.max(0)).execute(conn).await?;
    Ok(())
}

pub(super) async fn step(
    pool: &PgPool,
    source: &dyn LogSource,
    owner: &Owner,
    window_blocks: u64,
) -> anyhow::Result<usize> {
    let progress = || {
        sqlx::query_as::<_, (i64, i64)>(
            "SELECT position, ceiling FROM shop.worker_progress WHERE worker = $1 AND scope = 1",
        )
        .bind(WORKER)
    };
    let (position, ceiling) = match progress().fetch_optional(pool).await? {
        Some(row) => row,
        None => {
            let mut tx = pool.begin().await?;
            initialize(&mut tx).await?;
            let row = progress().fetch_one(&mut *tx).await?;
            owner.check(&mut tx).await?;
            tx.commit().await?;
            row
        }
    };
    if position >= ceiling {
        return Ok(0);
    }
    let (block, hash): (i64, String) = sqlx::query_as(
        "SELECT last_block, last_block_hash FROM shop.chain_cursor WHERE indexer='nft' AND network='ethereum'
         AND folded_block=last_block AND last_block >= $1 AND owner_generation=$2 AND chain_id=1
         AND last_block_hash IS NOT NULL AND quarantined_at IS NULL")
        .bind(ceiling).bind(owner.generation).fetch_one(pool).await?;
    let chain = source.chain_id().await?;
    anyhow::ensure!(chain.value == 1, "estate repair requires Ethereum mainnet");
    let before = source.header(block as u64).await?;
    anyhow::ensure!(
        before.value.hash == hash,
        "estate repair checkpoint diverged"
    );
    let from = position as u64 + 1;
    let to = (ceiling as u64).min(from.saturating_add(window_blocks - 1));
    let group = [LogGroup {
        addresses: vec![ESTATE_REGISTRY_MAINNET.into()],
        topics: json!([[TOPIC_CREATE_ESTATE]]),
    }];
    let mut chunk = 1;
    let window = source.window(from, to, &group, &mut chunk).await?;
    source.lane_read();
    let times = source.block_times(&window.logs).await?;
    let after = source.header(block as u64).await?;
    anyhow::ensure!(
        before.value == after.value,
        "chain changed while reading estate creations"
    );
    let mut rows = Vec::new();
    for log in &window.logs {
        let time = times
            .value
            .get(&chain_walk::log_position(log).0)
            .context("missing estate creation block time")?;
        if let Some(row) = decode_log(log, *time)? {
            rows.push(row);
        }
    }

    let mut tx = pool.begin().await?;
    let advanced = sqlx::query(
        "UPDATE shop.worker_progress SET position=$4
        WHERE worker=$1 AND scope=1 AND position=$2 AND ceiling=$3",
    )
    .bind(WORKER)
    .bind(position)
    .bind(ceiling)
    .bind(window.reached as i64)
    .execute(&mut *tx)
    .await?;
    anyhow::ensure!(advanced.rows_affected() == 1, "estate repair range changed");
    insert_events(&mut tx, ChainNet::Ethereum, &rows).await?;
    if window.reached as i64 == ceiling {
        // A newer valid Update still wins, including when an old fold had
        // previously overwritten it with a malformed document.
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
    Ok(chain.requests + before.requests + window.requests + times.requests + after.requests)
}
