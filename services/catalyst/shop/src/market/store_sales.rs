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
        if let Some((hash, lines)) = decode_bought(log)? {
            bought.push((log_position(log), hash, lines));
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

pub(super) async fn write(conn: &mut PgConnection, lines: &[StoreLine]) -> Result<(), sqlx::Error> {
    for chunk in lines.chunks(1000) {
        sqlx::query(
            "INSERT INTO shop.store_sale_line
               (block_number, bought_log, ordinal, tx_hash, contract_address, item_id, beneficiary, price, issue_log)
             SELECT block, bought, ordinal, tx, contract, item::numeric, beneficiary, price::numeric, issue
             FROM unnest($1::bigint[], $2::int[], $3::int[], $4::text[], $5::text[], $6::text[], $7::text[], $8::text[], $9::int[])
               AS u(block, bought, ordinal, tx, contract, item, beneficiary, price, issue)
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
        .execute(&mut *conn).await?;
    }
    Ok(())
}
