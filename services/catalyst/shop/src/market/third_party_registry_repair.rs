//! Classify old provider versions without withdrawing the current registry.
//! The archive must prove each old log against a checkpoint in the publication.

use anyhow::ensure;
use sqlx::PgPool;

use super::{Change, LogSource, Owner, decode, log_group, valid_id};
use crate::market::chain_walk::{INDEXER_THIRD_PARTY_REGISTRY, log_position};

pub(super) async fn backfill_reviews(
    pool: &PgPool,
    source: &dyn LogSource,
    owner: &Owner,
    window_blocks: u64,
    remaining: usize,
) -> anyhow::Result<usize> {
    if remaining < 4 {
        return Ok(0);
    }
    let window_span = window_blocks.saturating_sub(1);
    let Some((head, hash, from, first_uncovered)) =
        sqlx::query_as::<_, (i64, String, i64, Option<i64>)>(
        "SELECT p.block_number,p.block_hash,old.block_number,uncovered.block_number
         FROM shop.chain_cursor c
         JOIN shop.chain_checkpoint p ON (p.indexer,p.network,p.block_number)=(c.indexer,c.network,c.folded_block)
         CROSS JOIN LATERAL (
             SELECT block_number FROM shop.third_party_registry_event
             WHERE is_review IS NULL AND block_number<=p.block_number ORDER BY block_number LIMIT 1
         ) old
         LEFT JOIN LATERAL (
             SELECT e.block_number FROM shop.third_party_registry_event e
             WHERE e.is_review IS NULL AND e.block_number BETWEEN old.block_number AND p.block_number
               AND e.block_number-old.block_number<=$2 AND NOT EXISTS (
                 SELECT 1 FROM shop.chain_checkpoint known
                 WHERE (known.indexer,known.network,known.block_number)=(c.indexer,c.network,e.block_number)
               )
             ORDER BY e.block_number LIMIT 1
         ) uncovered ON true
         WHERE c.indexer=$1 AND c.network='matic' AND c.chain_id=137 AND c.full_history
           AND c.quarantined_at IS NULL AND c.diverged_at IS NULL AND c.rebuild_requested_at IS NULL",
    )
    .bind(INDEXER_THIRD_PARTY_REGISTRY)
    .bind(i64::try_from(window_span).unwrap_or(i64::MAX))
    .fetch_optional(pool)
    .await?
    else {
        return Ok(0);
    };
    if first_uncovered == Some(from) {
        return Ok(0);
    }
    let from = u64::try_from(from)?;
    let tip = u64::try_from(head)?;
    let mut to = tip.min(from.saturating_add(window_span));
    if let Some(block) = first_uncovered {
        to = to.min(u64::try_from(block - 1)?);
    }
    let before = source.header(tip).await?;
    ensure!(
        before.value.hash == hash,
        "registry review repair anchor changed"
    );
    let groups = [log_group()];
    let batch = source
        .archive_window(from, to, &groups, &mut 1, remaining - 3)
        .await?;
    let mut blocks = Vec::new();
    let mut logs = Vec::new();
    let mut ids = Vec::new();
    let mut reviews = Vec::new();
    let mut hashes = Vec::new();
    let mut reached = batch.reached;
    let mut used = before.requests + batch.requests;
    for group in batch
        .logs
        .chunk_by(|a, b| log_position(a).0 == log_position(b).0)
    {
        let number = u64::try_from(log_position(&group[0]).0)?;
        if used + 1 >= remaining {
            reached = number - 1;
            break;
        }
        let header = source.header(number).await?;
        used += header.requests;
        for log in group {
            ensure!(
                log.block_hash
                    .as_deref()
                    .is_some_and(|hash| hash.eq_ignore_ascii_case(&header.value.hash)),
                "registry review repair log was not on the canonical branch"
            );
            let event = decode(log)?;
            if valid_id(&event.id) {
                blocks.push(event.block);
                logs.push(event.index);
                ids.push(event.id);
                reviews.push(matches!(event.change, Change::Reviewed { .. }));
                hashes.push(header.value.hash.clone());
            }
        }
    }
    let after = source.header(tip).await?;
    ensure!(
        after.value == before.value,
        "registry review repair anchor changed"
    );
    used += after.requests;
    source.lane_read();

    let mut tx = pool.begin().await?;
    let anchored: bool = sqlx::query_scalar(
        "SELECT EXISTS (
           SELECT 1 FROM shop.chain_cursor c
           JOIN shop.chain_checkpoint p ON (p.indexer,p.network,p.block_number)=(c.indexer,c.network,c.folded_block)
           WHERE c.indexer=$1 AND c.network='matic' AND p.block_number=$2 AND p.block_hash=$3
             AND c.quarantined_at IS NULL AND c.diverged_at IS NULL AND c.rebuild_requested_at IS NULL
           FOR UPDATE OF c
         )",
    )
    .bind(INDEXER_THIRD_PARTY_REGISTRY)
    .bind(head)
    .bind(hash)
    .fetch_one(&mut *tx)
    .await?;
    ensure!(
        anchored,
        "registry review publication changed during repair"
    );
    let (missing, unproven): (bool, bool) = sqlx::query_as(
        "WITH archive AS (
             SELECT * FROM unnest($1::bigint[],$2::integer[],$3::text[],$4::boolean[],$5::text[])
                 AS a(block_number,log_index,provider_id,is_review,block_hash)
         ), proven AS (
             SELECT a.* FROM archive a
             JOIN shop.chain_checkpoint p ON p.indexer=$8 AND p.network='matic'
                AND (p.block_number,p.block_hash)=(a.block_number,a.block_hash)
         ), fixed AS (
             UPDATE shop.third_party_registry_event e SET is_review=a.is_review
             FROM proven a
             WHERE (e.block_number,e.log_index,e.provider_id)=(a.block_number,a.log_index,a.provider_id)
               AND e.block_number BETWEEN $6 AND $7 AND e.is_review IS NULL
             RETURNING e.block_number,e.log_index
         ), remaining AS (
             SELECT e.block_number,e.log_index,e.provider_id FROM shop.third_party_registry_event e
             WHERE e.block_number BETWEEN $6 AND $7 AND e.is_review IS NULL
               AND NOT EXISTS (SELECT 1 FROM fixed f WHERE (f.block_number,f.log_index)=(e.block_number,e.log_index))
         )
         SELECT EXISTS (
                 SELECT 1 FROM remaining e WHERE NOT EXISTS (
                    SELECT 1 FROM archive a
                    WHERE (e.block_number,e.log_index,e.provider_id)=(a.block_number,a.log_index,a.provider_id))
                ), EXISTS (
                 SELECT 1 FROM remaining e JOIN archive a USING(block_number,log_index,provider_id)
                )",
    )
    .bind(blocks).bind(logs).bind(ids).bind(reviews).bind(hashes)
    .bind(i64::try_from(from)?).bind(i64::try_from(reached)?)
    .bind(INDEXER_THIRD_PARTY_REGISTRY)
    .fetch_one(&mut *tx).await?;
    ensure!(!missing, "registry review archive omitted an indexed event");
    ensure!(
        !unproven,
        "registry review archive block has no matching published checkpoint"
    );
    owner.check(&mut tx).await?;
    tx.commit().await?;
    Ok(used)
}
