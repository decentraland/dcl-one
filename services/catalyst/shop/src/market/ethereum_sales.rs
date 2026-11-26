//! Ethereum sales inputs outside the NFT walk: the ERC721Bid lifecycle
//! (created, accepted and cancelled bids, folded into `shop.nft_bid`) and
//! the price the names controller charged for each registration. A cursor
//! walked when only acceptances were read gets the earlier creations and
//! cancellations from `ethereum_bid_history`.

use std::collections::HashMap;

use alloy_sol_types::{SolType, sol_data};
use anyhow::{Context, ensure};
use serde_json::json;
use sqlx::{PgConnection, PgPool};

use super::chain_walk::{self, ChainIndex, ChainNet, INDEXER_SALES, LogGroup};
use super::{nft_index, sales_index};
use crate::chain_index::rpc::{BlockHeader, RpcLog};

pub const ERC721_BID: &str = "0xe479dfd9664c693b2e2992300930b00bfde08233";
pub const BURNING_NAME_CONTROLLER: &str = "0x6843291bd86857d97f0d269e698939fb10d60772";
pub const NAME_CONTROLLER: &str = "0xbe92b49aee993adea3a002adcda189a2b7dec56c";
pub const NAME_CONTROLLERS: [&str; 2] = [BURNING_NAME_CONTROLLER, NAME_CONTROLLER];

/// NameBought(address indexed caller, address indexed beneficiary, uint256 price, string name).
pub const TOPIC_NAME_BOUGHT: &str =
    "0xb8c56202a5ae8b00edfcd57a54ec6c3fb8d2f6deb3067a7ba11408a7bd014a3e";

pub(super) struct NamePurchase {
    venue: String,
    block: i64,
    index: i32,
    time: i64,
    caller: String,
    beneficiary: String,
    price: String,
    name: String,
    tx_hash: String,
}

pub(super) fn decode_name(log: &RpcLog, time: i64) -> anyhow::Result<NamePurchase> {
    let (block, index) = chain_walk::log_position(log);
    ensure!(
        log.topics.len() == 3 && log.topics[0].eq_ignore_ascii_case(TOPIC_NAME_BOUGHT),
        "unexpected name controller log"
    );
    let caller = chain_walk::topic_address(&log.topics[1]).context("NameBought caller")?;
    let beneficiary =
        chain_walk::topic_address(&log.topics[2]).context("NameBought beneficiary")?;
    let data = hex::decode(log.data.trim_start_matches("0x"))?;
    let (price, name) = <(sol_data::Uint<256>, sol_data::String)>::abi_decode_params(&data)
        .context("NameBought payload")?;
    let hash = log
        .transaction_hash
        .as_deref()
        .context("NameBought transaction hash")?;
    Ok(NamePurchase {
        venue: log.address.to_ascii_lowercase(),
        block,
        index: i32::try_from(index)?,
        time,
        caller,
        beneficiary,
        price: price.to_string(),
        name,
        tx_hash: hash.to_ascii_lowercase(),
    })
}

pub struct EthereumSalesIndex;

#[async_trait::async_trait]
impl ChainIndex for EthereumSalesIndex {
    fn indexer(&self) -> &'static str {
        INDEXER_SALES
    }

    fn net(&self) -> ChainNet {
        ChainNet::Ethereum
    }

    fn start_block(&self) -> u64 {
        nft_index::deployments::Deployments::mainnet(self.net()).start
    }

    async fn groups(
        &self,
        _: &PgPool,
        _: u64,
        _: u64,
        _: (u64, i64),
    ) -> anyhow::Result<Vec<LogGroup>> {
        Ok(vec![
            super::sale_terms::group(self.net()),
            LogGroup {
                addresses: vec![ERC721_BID.into()],
                topics: json!([[
                    sales_index::TOPIC_BID_CREATED,
                    sales_index::TOPIC_BID_ACCEPTED,
                    sales_index::TOPIC_BID_CANCELLED
                ]]),
            },
            LogGroup {
                addresses: NAME_CONTROLLERS.iter().map(|&a| a.to_owned()).collect(),
                topics: json!([[TOPIC_NAME_BOUGHT]]),
            },
        ])
    }

    async fn write(
        &self,
        conn: &mut PgConnection,
        logs: &[RpcLog],
        times: &HashMap<i64, i64>,
    ) -> anyhow::Result<usize> {
        super::sale_terms::write(conn, self.net(), logs).await?;
        let mut bids = Vec::new();
        let mut names = Vec::new();
        for log in logs {
            let block = chain_walk::log_position(log).0;
            let time = *times
                .get(&block)
                .context("missing Ethereum sale block time")?;
            if log.address.eq_ignore_ascii_case(ERC721_BID) {
                if let Some(bid) = sales_index::decode_log(log, time)? {
                    ensure!(
                        [
                            sales_index::KIND_BID_CREATED,
                            sales_index::KIND_BID_ACCEPTED,
                            sales_index::KIND_BID_CANCELLED
                        ]
                        .contains(&bid.kind),
                        "unexpected Ethereum bid log"
                    );
                    bids.push(bid);
                }
            } else if NAME_CONTROLLERS
                .iter()
                .any(|a| log.address.eq_ignore_ascii_case(a))
            {
                names.push(decode_name(log, time)?);
            }
        }
        let mut written = sales_index::insert_events(conn, self.net(), &bids).await?;
        for chunk in names.chunks(1000) {
            let result = sqlx::query(
                "INSERT INTO shop.name_purchase (block_number, log_index, block_time, caller, beneficiary, price, name, tx_hash, venue)
                 SELECT b, i, t, caller, beneficiary, price::numeric, name, tx, venue FROM
                 unnest($1::bigint[], $2::integer[], $3::bigint[], $4::text[], $5::text[], $6::text[], $7::text[], $8::text[], $9::text[])
                 AS u(b,i,t,caller,beneficiary,price,name,tx,venue) ON CONFLICT (block_number, log_index) DO NOTHING"
            )
            .bind(chunk.iter().map(|row| row.block).collect::<Vec<_>>())
            .bind(chunk.iter().map(|row| row.index).collect::<Vec<_>>())
            .bind(chunk.iter().map(|row| row.time).collect::<Vec<_>>())
            .bind(chunk.iter().map(|row| &row.caller).collect::<Vec<_>>())
            .bind(chunk.iter().map(|row| &row.beneficiary).collect::<Vec<_>>())
            .bind(chunk.iter().map(|row| &row.price).collect::<Vec<_>>())
            .bind(chunk.iter().map(|row| &row.name).collect::<Vec<_>>())
            .bind(chunk.iter().map(|row| &row.tx_hash).collect::<Vec<_>>())
            .bind(chunk.iter().map(|row| &row.venue).collect::<Vec<_>>())
            .execute(&mut *conn).await?;
            written += result.rows_affected() as usize;
        }
        Ok(written)
    }

    async fn record_coverage(
        &self,
        conn: &mut PgConnection,
        previous: (i64, Option<&str>),
        end: &BlockHeader,
    ) -> anyhow::Result<()> {
        super::ethereum_bid_history::record(conn, previous, (end.number as i64, Some(&end.hash)))
            .await?;
        Ok(())
    }

    async fn rewind(&self, conn: &mut PgConnection, to: i64) -> anyhow::Result<()> {
        super::sale_terms::rewind(conn, self.net(), to).await?;
        sqlx::query("DELETE FROM shop.name_purchase WHERE block_number > $1")
            .bind(to)
            .execute(&mut *conn)
            .await?;
        // The deleted bid events' keys lose their rows; the cleared fold
        // marker makes the next fold a rebuild, which restores each key that
        // keeps a creation.
        sqlx::query(
            "WITH gone AS (DELETE FROM shop.sale_event WHERE network = 'ethereum' AND block_number > $1 \
                 RETURNING contract_address, token_id, buyer) \
             DELETE FROM shop.nft_bid b USING (SELECT DISTINCT contract_address, token_id, buyer FROM gone) g \
             WHERE b.network = 'ethereum' AND b.contract_address = g.contract_address AND b.token_id = g.token_id \
               AND b.bidder = g.buyer",
        )
        .bind(to)
        .execute(&mut *conn)
        .await?;
        super::ethereum_bid_history::rewind(conn, to).await?;
        Ok(())
    }

    async fn fold(
        &self,
        pool: &PgPool,
        since: Option<i64>,
        _: i64,
        owner: &chain_walk::Owner,
    ) -> anyhow::Result<bool> {
        let bids = sales_index::fold(pool, since, owner).await?;
        tracing::info!(bids, "ethereum sales index: folded");
        Ok(true)
    }

    async fn after_publish(
        &self,
        pool: &PgPool,
        source: &dyn chain_walk::LogSource,
        owner: &chain_walk::Owner,
        window_blocks: u64,
        remaining: usize,
    ) -> anyhow::Result<usize> {
        super::ethereum_bid_history::step(pool, source, owner, window_blocks, remaining).await
    }
}
