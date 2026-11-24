//! Proof of the log coverage needed by sales accounting. An existing
//! published walk may predate the tx hash, price, bid fee, or trade extras.
//! Its owner repairs those fields below its cursor in bounded windows.
use std::collections::BTreeSet;

use anyhow::{Context, ensure};
use serde_json::json;
use sqlx::PgPool;

use super::chain_walk::{
    self, ChainIndex, ChainNet, INDEXER_NFT, INDEXER_SALES, INDEXER_TRADE_EVENTS, LogGroup,
    LogSource, Owner, Spent,
};
use super::{ethereum_sales, nft_index, sales_index, trade_events};

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
    for log in logs {
        let Some(topic) = log.topics.first() else {
            continue;
        };
        let block = chain_walk::log_position(log).0;
        let required = match ix.indexer() {
            INDEXER_NFT => {
                topic.eq_ignore_ascii_case(nft_index::TOPIC_ORDER_SUCCESSFUL)
                    && nft_index::legacy_marketplaces(ix.net())
                        .iter()
                        .any(|(address, _)| log.address.eq_ignore_ascii_case(address))
            }
            INDEXER_TRADE_EVENTS => [trade_events::TOPIC_TRADED, trade_events::TOPIC_TRADED_V3]
                .iter()
                .any(|t| topic.eq_ignore_ascii_case(t)),
            INDEXER_SALES => [
                sales_index::TOPIC_ISSUE,
                sales_index::TOPIC_BID_ACCEPTED,
                sales_index::TOPIC_BOUGHT,
                ethereum_sales::TOPIC_NAME_BOUGHT,
            ]
            .iter()
            .any(|t| topic.eq_ignore_ascii_case(t)),
            _ => false,
        };
        if !required {
            continue;
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
            INDEXER_SALES
                if topic.eq_ignore_ascii_case(sales_index::TOPIC_BID_ACCEPTED)
                    || topic.eq_ignore_ascii_case(sales_index::TOPIC_ISSUE) =>
            {
                sales_index::decode_log(log, time)?;
            }
            INDEXER_TRADE_EVENTS => {
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
                (SELECT min(block_number) FROM shop.store_sale_line WHERE issue_log IS NULL AND block_number <= last_block) END)
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
            nft_index::legacy_marketplaces(net).iter().map(|v| v.0),
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
    let to = anchor.min(from.saturating_add(9_999));
    ensure!(from <= to, "sales repair moved beyond its cursor");
    let before = source.header(anchor).await?;
    used.read(before.requests);
    let mut window = source.window(from, to, &groups(owner), chunk).await?;
    used.window(&window);
    source.lane_read();
    if owner.indexer == INDEXER_SALES && owner.net == ChainNet::Matic {
        let mut addresses = BTreeSet::new();
        for log in &window.logs {
            if let Some((_, lines)) = sales_index::decode_bought(log)? {
                addresses.extend(lines.into_iter().map(|(contract, _, _, _)| contract));
            }
        }
        if !addresses.is_empty() {
            let issuers = LogGroup {
                addresses: addresses.into_iter().collect(),
                topics: json!([[sales_index::TOPIC_ISSUE]]),
            };
            let issues = source
                .window(from, window.reached, &[issuers], chunk)
                .await?;
            used.window(&issues);
            window.reached = issues.reached;
            window
                .logs
                .retain(|log| chain_walk::log_position(log).0 <= issues.reached as i64);
            window.logs.extend(issues.logs);
            window.logs.sort_by_key(chain_walk::log_position);
        }
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
    let count = sqlx::query(
        "UPDATE shop.chain_cursor SET sale_input_version = $3,
          sale_verified_through = CASE WHEN sale_input_version IS DISTINCT FROM $3 OR sale_covered_rewinds IS DISTINCT FROM reorg_rewinds THEN $4 ELSE greatest(sale_verified_through, $4) END,
          sale_covered_rewinds = reorg_rewinds, sale_history_revision = sale_history_revision + 1
         WHERE indexer = $1 AND network = $2 AND last_block = $5 AND last_block_hash = $6
           AND $7 <= CASE WHEN sale_input_version = $3 AND sale_covered_rewinds = reorg_rewinds THEN coalesce(sale_verified_through + 1,start_block) ELSE start_block END")
        .bind(owner.indexer).bind(owner.net.db_name()).bind(VERSION).bind(window.reached as i64)
        .bind(anchor as i64).bind(hash).bind(next).execute(&mut *tx).await?.rows_affected();
    ensure!(count == 1, "sales repair coverage changed");
    owner.check(&mut tx).await?;
    tx.commit().await?;
    Ok(used)
}
