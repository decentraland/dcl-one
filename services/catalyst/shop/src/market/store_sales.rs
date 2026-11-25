//! A store execution ends at its Bought log. Each line has an explicit
//! price and is paired with at most one preceding Issue from that execution.

use std::collections::{HashMap, VecDeque};

use anyhow::Context;
use sqlx::PgConnection;

use super::chain_walk::log_position;
use super::sales_index::{KIND_MINT, SaleEvent, decode_bought};
use crate::chain_index::rpc::RpcLog;

#[derive(Debug)]
pub(super) struct StoreLine {
    block: i64,
    bought: i32,
    ordinal: i32,
    hash: String,
    contract: String,
    item: String,
    beneficiary: String,
    price: String,
    issue: Option<i32>,
}

pub(super) fn pair(rows: &mut [SaleEvent], logs: &[RpcLog]) -> anyhow::Result<Vec<StoreLine>> {
    let mut issues = HashMap::<&str, Vec<usize>>::new();
    for (i, row) in rows.iter().enumerate().filter(|(_, r)| r.kind == KIND_MINT) {
        issues.entry(&row.tx_hash).or_default().push(i);
    }
    let mut bought = Vec::new();
    for log in logs {
        match decode_bought(log) {
            Ok(Some((hash, lines))) => bought.push((log_position(log), hash, lines)),
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(%error, "sales index: skipping an undecodable store purchase");
                // The log still ends this fill. Its Issues cannot price the next purchase.
                if let Some(hash) = &log.transaction_hash {
                    bought.push((log_position(log), hash.to_ascii_lowercase(), Vec::new()));
                }
            }
        }
    }
    bought.sort_by_key(|(position, _, _)| *position);
    let mut bounds = HashMap::new();
    let mut out = Vec::new();
    for ((block, index), hash, lines) in bought {
        let index = i32::try_from(index)?;
        let previous = bounds.insert(hash.clone(), index).unwrap_or(-1);
        let mut tokens = HashMap::<(String, String, String), VecDeque<(i32, usize)>>::new();
        let mut eligible = issues.get(hash.as_str()).cloned().unwrap_or_default();
        eligible.sort_by_key(|i| rows[*i].log_index);
        for i in eligible {
            let row = &rows[i];
            if row.block_number == block && row.log_index > previous && row.log_index < index {
                tokens
                    .entry((
                        row.contract_address.clone(),
                        row.item_id.clone().context("mint without item")?,
                        row.buyer.clone(),
                    ))
                    .or_default()
                    .push_back((row.log_index, i));
            }
        }
        for (ordinal, (contract, item, beneficiary, price)) in lines.into_iter().enumerate() {
            let key = (contract, item, beneficiary);
            let issue = tokens.get_mut(&key).and_then(VecDeque::pop_front);
            let (contract, item, beneficiary) = key;
            out.push((
                StoreLine {
                    block,
                    bought: index,
                    ordinal: i32::try_from(ordinal)?,
                    hash: hash.clone(),
                    contract,
                    item,
                    beneficiary,
                    price,
                    issue: issue.map(|(log, _)| log),
                },
                issue.map(|(_, row)| row),
            ));
        }
    }
    Ok(out
        .into_iter()
        .map(|(line, issue)| {
            if let Some(i) = issue {
                rows[i].price = Some(line.price.clone());
            }
            line
        })
        .collect())
}

pub(super) async fn write(
    conn: &mut PgConnection,
    lines: &[StoreLine],
    times: &HashMap<i64, i64>,
) -> anyhow::Result<()> {
    for chunk in lines.chunks(1000) {
        sqlx::query(
            "INSERT INTO shop.store_sale_line
               (block_number, bought_log, ordinal, tx_hash, contract_address, item_id, beneficiary, price, issue_log, block_time)
             SELECT block, bought, ordinal, tx, contract, item::numeric, beneficiary, price::numeric, issue, block_time
             FROM unnest($1::bigint[], $2::int[], $3::int[], $4::text[], $5::text[], $6::text[], $7::text[], $8::text[], $9::int[], $10::bigint[])
               AS u(block, bought, ordinal, tx, contract, item, beneficiary, price, issue, block_time)
             WHERE block_time IS NOT NULL
             ON CONFLICT (block_number, bought_log, ordinal) DO UPDATE SET issue_log = EXCLUDED.issue_log
             WHERE store_sale_line.tx_hash = EXCLUDED.tx_hash AND store_sale_line.issue_log IS NULL AND EXCLUDED.issue_log IS NOT NULL"
        )
        .bind(chunk.iter().map(|r| r.block).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.bought).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.ordinal).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.hash.as_str()).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.contract.as_str()).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.item.as_str()).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.beneficiary.as_str()).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.price.as_str()).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.issue).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| times.get(&r.block).copied()).collect::<Vec<_>>())
        .execute(&mut *conn).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::sales_index::{
        self,
        tests::{bought_log, issue_log},
    };
    use super::*;
    use alloy_primitives::U256;

    #[test]
    fn identical_store_lines_consume_their_issues_in_log_order() {
        let issuer = "0x0000000000000000000000000000000000000011";
        let recipient = "0x0000000000000000000000000000000000000022";
        let hash = format!("0x{:064x}", 10);
        let mut rows: Vec<_> = [3, 1]
            .into_iter()
            .map(|index| {
                let mut l = issue_log(
                    issuer,
                    recipient,
                    U256::from(index),
                    7,
                    sales_index::STORE_MATIC,
                    100,
                    index,
                );
                l.transaction_hash = Some(hash.clone());
                sales_index::decode_log(&l, 1000).unwrap().unwrap()
            })
            .collect();
        let line = (issuer, 7, 50, recipient);
        let result = pair(&mut rows, &[bought_log(&[line, line], &hash, 100, 4)]).unwrap();
        assert_eq!(
            result
                .iter()
                .map(|l| (l.ordinal, l.issue))
                .collect::<Vec<_>>(),
            [(0, Some(1)), (1, Some(3))]
        );
    }
}
