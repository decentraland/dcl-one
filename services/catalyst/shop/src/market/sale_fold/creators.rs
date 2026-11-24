//! At a store mint the public seller is the collection creator. A creator
//! can transfer creatorship multiple times in a block. Read the end state by
//! canonical block hash and undo each later change using its old address.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use alloy_primitives::keccak256;
use anyhow::{Context, ensure};
use serde_json::json;
use sqlx::PgConnection;

use super::{ChainNet, Rpc, chain_walk};
use crate::chain_index::rpc::{RpcLog, hex_to_i64};

static CHANGED: LazyLock<String> = LazyLock::new(|| {
    format!(
        "{:#x}",
        keccak256("CreatorshipTransferred(address,address)")
    )
});

pub(super) async fn write(
    c: &mut PgConnection,
    rpc: &Rpc<'_>,
    net: ChainNet,
    from: i64,
    to: i64,
) -> anyhow::Result<Vec<i64>> {
    type Mint = (i64, i32, i32, String, i32);
    if net != ChainNet::Matic {
        return Ok(Vec::new());
    }
    let mints: Vec<Mint> = sqlx::query_as(
        "WITH pending AS (
           SELECT block_number,log_index FROM shop.sale_execution e WHERE network=$1 AND kind='store'
             AND block_number<=$2 AND (creator_attempted_at IS NULL OR creator_attempted_at<timezone('UTC',now())-interval '5 minutes')
             AND EXISTS (SELECT FROM shop.sale_asset a WHERE (a.network,a.block_number,a.log_index)=(e.network,e.block_number,e.log_index) AND seller IS NULL)
           ORDER BY creator_attempted_at NULLS FIRST,block_number LIMIT 32)
         SELECT a.block_number,a.log_index,a.ordinal,a.contract_address,l.issue_log
         FROM shop.sale_asset a JOIN shop.store_sale_line l ON (a.block_number,a.log_index,a.ordinal)=(l.block_number,l.bought_log,l.ordinal)
         JOIN shop.sale_execution e ON (e.network,e.block_number,e.log_index)=(a.network,a.block_number,a.log_index)
         WHERE a.network=$1 AND e.kind='store' AND a.seller IS NULL AND
           ((a.block_number>$2 AND a.block_number<=$3) OR (a.block_number,a.log_index) IN (SELECT * FROM pending))
         ORDER BY a.block_number,a.contract_address,l.issue_log")
        .bind(net.db_name()).bind(from).bind(to).fetch_all(&mut *c).await?;
    let mut days = BTreeSet::new();
    let heights: BTreeSet<_> = mints.iter().map(|x| x.0 as u64).collect();
    sqlx::query(
        "UPDATE shop.sale_execution SET creator_attempted_at=timezone('UTC',now())
        WHERE network=$1 AND kind='store' AND block_number=ANY($2)",
    )
    .bind(net.db_name())
    .bind(heights.iter().map(|&h| h as i64).collect::<Vec<_>>())
    .execute(&mut *c)
    .await?;
    let (headers, _) = rpc
        .block_headers(&heights.into_iter().collect::<Vec<_>>())
        .await?;
    let selector = format!("0x{}", hex::encode(&keccak256("creator()")[..4]));
    let groups: Vec<_> = mints.chunk_by(|a, b| a.0 == b.0 && a.3 == b.3).collect();
    for batch in groups.chunks(16) {
        let mut requests = Vec::new();
        for (i, g) in batch.iter().enumerate() {
            let m = &g[0];
            let hash = &headers[&(m.0 as u64)].hash;
            requests.push(json!({"jsonrpc":"2.0","id":2*i,"method":"eth_call",
                "params":[{"to":m.3,"data":selector},{"blockHash":hash,"requireCanonical":true}]}));
            requests.push(json!({"jsonrpc":"2.0","id":2*i+1,"method":"eth_getLogs",
                "params":[{"blockHash":hash,"address":m.3,"topics":[*CHANGED]}]}));
        }
        let mut answers = BTreeMap::new();
        for a in rpc.batch(&requests).await? {
            let id = a["id"].as_u64().context("creator response id")? as usize;
            ensure!(
                id < requests.len() && answers.insert(id, a).is_none(),
                "duplicate or invalid creator response"
            );
        }
        for (i, g) in batch.iter().enumerate() {
            let m = &g[0];
            let hash = &headers[&(m.0 as u64)].hash;
            let Some(mut creator) = answers
                .get(&(2 * i))
                .and_then(|a| a["result"].as_str())
                .and_then(chain_walk::topic_address)
            else {
                continue;
            };
            let Some(changes) = answers
                .get(&(2 * i + 1))
                .and_then(|a| a["result"].as_array())
            else {
                continue;
            };
            let mut history = Vec::new();
            for value in changes {
                let l: RpcLog = serde_json::from_value(value.clone())?;
                ensure!(
                    l.block_hash
                        .as_deref()
                        .is_some_and(|h| hash.eq_ignore_ascii_case(h))
                        && l.address.eq_ignore_ascii_case(&m.3)
                        && l.topics.len() == 3
                        && l.topics[0].eq_ignore_ascii_case(&CHANGED),
                    "creator history mismatch"
                );
                history.push((
                    hex_to_i64(&l.log_index).context("creator event index")?,
                    chain_walk::topic_address(&l.topics[1]).context("previous creator")?,
                    chain_walk::topic_address(&l.topics[2]).context("new creator")?,
                ));
            }
            history.sort_by_key(|(at, _, _)| *at);
            for &(block, index, ordinal, _, issue) in g.iter().rev() {
                while history
                    .last()
                    .is_some_and(|(at, _, _)| *at > i64::from(issue))
                {
                    let (_, prev, next) = history.pop().unwrap();
                    ensure!(next == creator, "creator chain inconsistent");
                    creator = prev;
                }
                days.insert(block);
                sqlx::query("UPDATE shop.sale_asset SET seller=$5 WHERE (network,block_number,log_index,ordinal)=($1,$2,$3,$4)")
                    .bind(net.db_name()).bind(block).bind(index).bind(ordinal).bind(&creator).execute(&mut *c).await?;
            }
        }
    }
    Ok(sqlx::query_scalar("SELECT DISTINCT block_time/86400*86400 FROM shop.sale_execution WHERE network=$1 AND block_number=ANY($2)")
        .bind(net.db_name()).bind(days.into_iter().collect::<Vec<_>>()).fetch_all(&mut *c).await?)
}
