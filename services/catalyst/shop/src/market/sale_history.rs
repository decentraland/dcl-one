//! Proof of the log coverage needed by sales accounting. An existing
//! published walk may predate the tx hash, price, bid fee, or trade extras.
//! Its owner repairs those fields below its cursor in bounded windows.
use std::collections::{BTreeMap, BTreeSet};

use crate::chain_index::rpc::RpcError;
use anyhow::{Context, ensure};
use serde_json::json;
use sqlx::PgPool;

use super::chain_walk::{
    self, ChainIndex, ChainNet, INDEXER_NFT, INDEXER_SALES, INDEXER_TRADE_EVENTS, LogGroup,
    LogSource, Owner, Spent,
};
use super::{ethereum_sales, nft_index, sales_index, trade_events};

pub const REPAIR_FAILURES: dcl_metrics::CounterFamily<2> = dcl_metrics::CounterFamily::new(
    "catalyst_index_sales_repair_failures_total",
    "Failed historical sale repair attempts by projection and network.",
    Some(dcl_metrics::Unit::Count),
    &["indexer", "network"],
);
pub const FAMILIES: &[dcl_metrics::Descriptor] = &[REPAIR_FAILURES.descriptor];

#[derive(Debug, thiserror::Error)]
#[error("sales history repair starting at block {from} after {requests} requests")]
pub struct RepairAttempt {
    from: u64,
    pub requests: usize,
}

pub const VERSION: i32 = 1;

pub fn required(indexer: &str) -> bool {
    matches!(indexer, INDEXER_NFT | INDEXER_SALES | INDEXER_TRADE_EVENTS)
}

/// Reject an incomplete economic event before its block can extend verified coverage.
/// Other NFT and auction topics keep their existing decoding policy.
pub fn validate_window(
    ix: &dyn ChainIndex,
    logs: &[crate::chain_index::rpc::RpcLog],
    times: &std::collections::HashMap<i64, i64>,
) -> anyhow::Result<()> {
    logs.iter().try_for_each(|log| validate_log(ix, log, times))
}

pub(super) fn validate_log(
    ix: &dyn ChainIndex,
    log: &crate::chain_index::rpc::RpcLog,
    times: &std::collections::HashMap<i64, i64>,
) -> anyhow::Result<()> {
    let Some(topic) = log.topics.first() else {
        return Ok(());
    };
    let block = chain_walk::log_position(log).0;
    let required = match ix.indexer() {
        INDEXER_NFT => {
            topic.eq_ignore_ascii_case(nft_index::TOPIC_ORDER_SUCCESSFUL)
                && nft_index::deployments::Deployments::mainnet(ix.net())
                    .markets
                    .iter()
                    .any(|(address, _)| log.address.eq_ignore_ascii_case(address))
        }
        INDEXER_TRADE_EVENTS => [trade_events::TOPIC_TRADED, trade_events::TOPIC_TRADED_V3]
            .iter()
            .any(|t| topic.eq_ignore_ascii_case(t)),
        INDEXER_SALES => [
            sales_index::TOPIC_BID_ACCEPTED,
            sales_index::TOPIC_BOUGHT,
            ethereum_sales::TOPIC_NAME_BOUGHT,
        ]
        .iter()
        .any(|t| topic.eq_ignore_ascii_case(t)),
        _ => false,
    };
    if ix.indexer() == INDEXER_SALES {
        super::sale_terms::decode(ix.net(), log)?;
    }
    if !required {
        return Ok(());
    }
    let time = *times.get(&block).context("missing sale block time")?;
    let hash = log
        .transaction_hash
        .as_deref()
        .context("missing sale transaction")?;
    ensure!(
        hash.strip_prefix("0x")
            .is_some_and(|h| h.len() == 64 && h.bytes().all(|c| c.is_ascii_hexdigit())),
        "invalid sale transaction"
    );
    match ix.indexer() {
        INDEXER_NFT => {
            let row = nft_index::decode_log(log, time)?.context("missing sale order")?;
            ensure!(
                row.to_address.is_some() && row.from_address.is_some(),
                "missing order parties"
            );
        }
        INDEXER_SALES if topic.eq_ignore_ascii_case(sales_index::TOPIC_BID_ACCEPTED) => {
            sales_index::decode_log(log, time)?;
        }
        INDEXER_SALES if topic.eq_ignore_ascii_case(sales_index::TOPIC_BOUGHT) => {
            sales_index::decode_bought(log)?;
        }
        INDEXER_SALES if topic.eq_ignore_ascii_case(ethereum_sales::TOPIC_NAME_BOUGHT) => {
            ethereum_sales::decode_name(log, time)?;
        }
        INDEXER_TRADE_EVENTS => {
            trade_events::decode_log(log, time)?;
            ensure!(
                log.topics
                    .get(1)
                    .and_then(|t| chain_walk::topic_address(t))
                    .is_some(),
                "missing trade caller"
            );
        }
        _ => {}
    }
    Ok(())
}

pub async fn pending(pool: &PgPool, ix: &dyn ChainIndex) -> anyhow::Result<Option<i64>> {
    if !required(ix.indexer()) || ix.chain_id() != ix.net().chain_id() {
        return Ok(None);
    }
    // An incomplete Bought can also arrive above a repaired prefix when the
    // collections mirror did not yet name one of that purchase's issuers.
    Ok(sqlx::query_scalar::<_, Option<i64>>(
        "SELECT least(
           CASE WHEN sale_input_version IS DISTINCT FROM $3 OR sale_covered_rewinds IS DISTINCT FROM reorg_rewinds OR sale_verified_through IS NULL THEN start_block
                WHEN sale_verified_through < last_block THEN sale_verified_through + 1 END,
           CASE WHEN indexer = 'sales' AND network = 'matic' THEN
                (SELECT min(block_number) FROM shop.verified_store_sale_line WHERE issue_log IS NULL AND NOT issue_checked AND block_number <= last_block) END)
         FROM shop.chain_cursor WHERE indexer = $1 AND network = $2 AND full_history AND chain_id = $4
           AND last_block >= start_block AND last_block_hash IS NOT NULL AND quarantined_at IS NULL AND diverged_at IS NULL")
        .bind(ix.indexer()).bind(ix.net().db_name()).bind(VERSION).bind(ix.chain_id())
        .fetch_optional(pool).await?.flatten())
}

fn group(addresses: impl IntoIterator<Item = &'static str>, topics: &[&str]) -> LogGroup {
    LogGroup {
        addresses: addresses.into_iter().map(str::to_owned).collect(),
        topics: json!([topics]),
    }
}

fn groups(owner: &Owner) -> Vec<LogGroup> {
    let mut groups = match (owner.indexer, owner.net) {
        (INDEXER_NFT, net) => vec![group(
            nft_index::deployments::Deployments::mainnet(net)
                .markets
                .iter()
                .map(|v| v.0),
            &[nft_index::TOPIC_ORDER_SUCCESSFUL],
        )],
        (INDEXER_TRADE_EVENTS, net) => vec![group(
            trade_events::marketplaces(net).iter().map(|v| v.0),
            &[trade_events::TOPIC_TRADED, trade_events::TOPIC_TRADED_V3],
        )],
        (INDEXER_SALES, ChainNet::Ethereum) => vec![
            group(
                [ethereum_sales::ERC721_BID],
                &[sales_index::TOPIC_BID_ACCEPTED],
            ),
            group(
                ethereum_sales::NAME_CONTROLLERS,
                &[ethereum_sales::TOPIC_NAME_BOUGHT],
            ),
        ],
        (INDEXER_SALES, ChainNet::Matic) => vec![
            group([sales_index::STORE_MATIC], &[sales_index::TOPIC_BOUGHT]),
            group(
                [sales_index::BID_V2_MATIC],
                &[sales_index::TOPIC_BID_ACCEPTED],
            ),
        ],
        _ => Vec::new(),
    };
    if owner.indexer == INDEXER_SALES {
        groups.push(super::sale_terms::group(owner.net));
    }
    groups
}

// A provider may narrow the interval repeatedly before the request budget
// ends. Keep its last upper bound for the next pass, including after restart.
async fn remember_upper(
    pool: &PgPool,
    owner: &Owner,
    from: u64,
    to: u64,
    upper: u64,
) -> anyhow::Result<()> {
    let upper = if upper < to {
        upper
    } else {
        from + (to - from) / 2
    };
    let mut tx = pool.begin().await?;
    owner.check(&mut tx).await?;
    sqlx::query(
        "UPDATE shop.chain_cursor SET sale_repair_upper=$3
        WHERE indexer=$1 AND network=$2",
    )
    .bind(owner.indexer)
    .bind(owner.net.db_name())
    .bind(i64::try_from(upper)?)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

// Keep every log of an accepted block together, and only as many blocks
// missing timestamps as the caller can afford to fetch.
fn trim_for_times(window: &mut chain_walk::Window, from: u64, requests: usize) -> bool {
    let missing: BTreeSet<_> = window
        .logs
        .iter()
        .filter(|l| {
            l.block_timestamp
                .as_deref()
                .and_then(crate::chain_index::rpc::hex_to_i64)
                .is_none()
        })
        .map(|l| chain_walk::log_position(l).0)
        .collect();
    let capacity = requests.saturating_mul(crate::chain_index::rpc::HEADER_BATCH);
    if let Some(&block) = missing.iter().nth(capacity) {
        if block as u64 == from {
            return false;
        }
        window.reached = block as u64 - 1;
        window
            .logs
            .retain(|l| chain_walk::log_position(l).0 < block);
    }
    true
}

pub async fn repair(
    pool: &PgPool,
    source: &dyn LogSource,
    ix: &dyn ChainIndex,
    owner: &Owner,
    cursor: Option<(u64, &str)>,
    budget: usize,
    chunk: &mut usize,
) -> anyhow::Result<Spent> {
    let mut used = Spent::default();
    let Some((anchor, hash)) = cursor.filter(|_| budget > 0) else {
        return Ok(used);
    };
    let Some(next) = pending(pool, ix).await? else {
        return Ok(used);
    };
    let from = u64::try_from(next)?;
    if let Err(error) = repair_window(
        pool,
        source,
        ix,
        owner,
        (from, anchor, hash),
        (budget, chunk),
        &mut used,
    )
    .await
    {
        let failed_requests = match error.downcast_ref::<RpcError>() {
            Some(RpcError::Counted { requests, .. } | RpcError::Budget { requests, .. }) => {
                *requests
            }
            Some(_) => 1, // A single header request.
            None => 0,
        };
        return Err(error.context(RepairAttempt {
            from,
            requests: used.requests + failed_requests,
        }));
    }
    Ok(used)
}

async fn repair_window(
    pool: &PgPool,
    source: &dyn LogSource,
    ix: &dyn ChainIndex,
    owner: &Owner,
    (from, anchor, hash): (u64, u64, &str),
    (budget, chunk): (usize, &mut usize),
    used: &mut Spent,
) -> anyhow::Result<()> {
    let next = from as i64;
    let groups = groups(owner);
    let store = owner.indexer == INDEXER_SALES && owner.net == ChainNet::Matic;
    let log_cost = chain_walk::requests_for(&groups, *chunk);
    // Bracket the window and leave room for the issuer filter and block times.
    let overhead = 2 + log_cost + usize::from(store);
    let width = budget
        .saturating_sub(overhead)
        .saturating_mul(crate::chain_index::rpc::HEADER_BATCH)
        .min(10_000);
    if width == 0 {
        return Ok(());
    }
    let upper: Option<i64> = sqlx::query_scalar(
        "SELECT sale_repair_upper FROM shop.chain_cursor WHERE indexer=$1 AND network=$2",
    )
    .bind(owner.indexer)
    .bind(owner.net.db_name())
    .fetch_one(pool)
    .await?;
    let to = anchor
        .min(from.saturating_add(width as u64 - 1))
        .min(upper.filter(|u| *u >= next).map_or(anchor, |u| u as u64));
    ensure!(from <= to, "sales repair moved beyond its cursor");
    let before = source.header(anchor).await?;
    used.read(before.requests);
    let mut window = match source
        .archive_window(from, to, &groups, chunk, budget - used.charge - 1)
        .await
    {
        Ok(window) => window,
        Err(RpcError::Budget { upper, requests }) => {
            remember_upper(pool, owner, from, to, upper).await?;
            used.read(requests);
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    used.window(&window);
    source.lane_read();
    let mut issuers_read = Vec::new();
    if store {
        let mut by_block: BTreeMap<i64, BTreeSet<String>> = BTreeMap::new();
        for log in &window.logs {
            if let Some((_, lines)) = sales_index::decode_bought(log)? {
                by_block
                    .entry(chain_walk::log_position(log).0)
                    .or_default()
                    .extend(lines.into_iter().map(|(contract, _, _, _)| contract));
            }
        }
        let candidates: BTreeSet<_> = by_block.values().flatten().cloned().collect();
        // The catalogue the walk follows (`nft_index::catalogue_set_sql`).
        let listed: BTreeMap<String, i64> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT contract_address, created_at FROM ({}) AS c WHERE contract_address = ANY($1)",
            nft_index::catalogue_set_sql(ChainNet::Matic, 137, ix.catalogue_native())
        )))
        .bind(candidates.into_iter().collect::<Vec<_>>())
        .fetch_all(pool)
        .await?
        .into_iter()
        .collect();
        let mut block_times = std::collections::HashMap::new();
        if !listed.is_empty() {
            if !trim_for_times(&mut window, from, budget.saturating_sub(used.charge + 2)) {
                return Ok(());
            }
            let times = source.block_times(&window.logs).await?;
            used.read(times.requests);
            block_times = times.value;
            by_block.retain(|b, _| *b <= window.reached as i64);
        }
        // Keep whole blocks. A hot window can name more issuers than fit in
        // the remaining address chunks. Its suffix belongs to the next pass.
        let max_addresses = budget
            .saturating_sub(used.charge + 2)
            .saturating_mul((*chunk).max(1));
        let mut addresses = BTreeSet::new();
        for (block, mut names) in by_block {
            names.retain(|name| {
                listed
                    .get(name)
                    .zip(block_times.get(&block))
                    .is_some_and(|(created, purchased)| created <= purchased)
            });
            let extra = names.iter().filter(|a| !addresses.contains(*a)).count();
            if addresses.len() + extra > max_addresses {
                if block as u64 == from {
                    return Ok(());
                }
                window.reached = block as u64 - 1;
                window
                    .logs
                    .retain(|l| chain_walk::log_position(l).0 < block);
                break;
            }
            addresses.extend(names);
        }
        issuers_read = addresses.into_iter().collect();
        if !issuers_read.is_empty() {
            let issuers = LogGroup {
                addresses: issuers_read.clone(),
                topics: json!([[sales_index::TOPIC_ISSUE]]),
            };
            let issues = match source
                .archive_window(
                    from,
                    window.reached,
                    &[issuers],
                    chunk,
                    budget - used.charge - 1,
                )
                .await
            {
                Ok(window) => window,
                Err(RpcError::Budget { upper, requests }) => {
                    remember_upper(pool, owner, from, window.reached, upper).await?;
                    used.read(requests);
                    return Ok(());
                }
                Err(error) => return Err(error.into()),
            };
            used.window(&issues);
            window.reached = issues.reached;
            window
                .logs
                .retain(|log| chain_walk::log_position(log).0 <= issues.reached as i64);
            window.logs.extend(issues.logs);
            window.logs.sort_by_key(chain_walk::log_position);
        }
        for log in &mut window.logs {
            if let Some(time) = block_times.get(&chain_walk::log_position(log).0) {
                log.block_timestamp = Some(format!("0x{time:x}"));
            }
        }
    }
    if used.charge >= budget || !trim_for_times(&mut window, from, budget - used.charge - 1) {
        return Ok(());
    }
    let times = source.block_times(&window.logs).await?;
    used.read(times.requests);
    let after = source.header(anchor).await?;
    used.read(after.requests);
    ensure!(
        before.value == after.value,
        "chain changed during sales history repair"
    );
    if after.value.hash != hash {
        return Err(chain_walk::Diverged {
            block: anchor,
            stored: hash.into(),
            chain: after.value.hash,
            requests: used.requests,
        }
        .into());
    }
    validate_window(ix, &window.logs, &times.value)?;
    let mut tx = pool.begin().await?;
    // Book repair and folding share the cursor-first order. Trade-events
    // recovery locks its event rows before the cursor.
    let book = matches!(owner.indexer, INDEXER_NFT | INDEXER_SALES);
    if book {
        owner.check(&mut tx).await?;
    }
    if owner.indexer == INDEXER_NFT {
        // An NFT write also commits its pending collection discoveries. A
        // repair of legacy orders only ever enriches the named order rows.
        let mut orders = Vec::new();
        for log in &window.logs {
            let time = times
                .value
                .get(&chain_walk::log_position(log).0)
                .context("missing legacy sale block time")?;
            let order = nft_index::decode_log(log, *time)?.context("missing legacy sale")?;
            ensure!(order.tx_hash.is_some(), "legacy sale with no transaction");
            orders.push(order);
        }
        nft_index::insert_events(&mut tx, owner.net, &orders).await?;
    } else {
        ix.write(&mut tx, &window.logs, &times.value).await?;
    }
    if !issuers_read.is_empty() {
        // Mark only the factory issuers this exact window actually probed.
        let mut blocks = Vec::new();
        let mut indices = Vec::new();
        let mut hashes = Vec::new();
        for log in &window.logs {
            if let Some((hash, _)) = sales_index::decode_bought(log)? {
                let (block, index) = chain_walk::log_position(log);
                blocks.push(block);
                indices.push(i32::try_from(index)?);
                hashes.push(hash);
            }
        }
        sqlx::query(
            "UPDATE shop.store_sale_line s SET issue_checked=true
            FROM unnest($1::bigint[],$2::int[],$3::text[]) b(block,idx,hash)
            WHERE (s.block_number,s.bought_log,s.tx_hash)=(b.block,b.idx,b.hash)
              AND s.issue_log IS NULL AND NOT s.issue_checked AND s.contract_address=ANY($4)
              AND EXISTS (SELECT FROM shop.verified_store_sale_line v
                  WHERE (v.block_number,v.bought_log,v.ordinal)=(s.block_number,s.bought_log,s.ordinal))",
        )
        .bind(blocks)
        .bind(indices)
        .bind(hashes)
        .bind(issuers_read)
        .execute(&mut *tx)
        .await?;
    }
    let count = sqlx::query(
        "UPDATE shop.chain_cursor SET sale_input_version = $3, sale_repair_upper=NULL,
          sale_verified_through = CASE WHEN sale_input_version IS DISTINCT FROM $3 OR sale_covered_rewinds IS DISTINCT FROM reorg_rewinds THEN $4 ELSE greatest(sale_verified_through, $4) END,
          sale_covered_rewinds = reorg_rewinds, sale_dirty_from = least(sale_dirty_from,$7), sale_history_revision = sale_history_revision + 1
         WHERE indexer = $1 AND network = $2 AND last_block = $5 AND last_block_hash = $6
           AND $7 <= CASE WHEN sale_input_version = $3 AND sale_covered_rewinds = reorg_rewinds THEN coalesce(sale_verified_through + 1,start_block) ELSE start_block END")
        .bind(owner.indexer).bind(owner.net.db_name()).bind(VERSION).bind(window.reached as i64)
        .bind(anchor as i64).bind(hash).bind(next).execute(&mut *tx).await?.rows_affected();
    ensure!(count == 1, "sales repair coverage changed");
    if !book {
        owner.check(&mut tx).await?;
    }
    tx.commit().await?;
    Ok(())
}
