//! Ethereum sales inputs outside the NFT walk: accepted ERC721 bids and
//! the price the names controller charged for each registration.

use std::collections::HashMap;

use alloy_sol_types::{SolType, sol_data};
use anyhow::{Context, ensure};
use serde_json::json;
use sqlx::{PgConnection, PgPool};

use super::chain_walk::{self, ChainIndex, ChainNet, INDEXER_SALES, LogGroup};
use super::{nft_index, sales_index};
use crate::chain_index::rpc::RpcLog;

pub const ERC721_BID: &str = "0xe479dfd9664c693b2e2992300930b00bfde08233";
pub const BURNING_NAME_CONTROLLER: &str = "0x6843291bd86857d97f0d269e698939fb10d60772";
pub const NAME_CONTROLLER: &str = "0xbe92b49aee993adea3a002adcda189a2b7dec56c";
pub const NAME_CONTROLLERS: [&str; 2] = [BURNING_NAME_CONTROLLER, NAME_CONTROLLER];

/// NameBought(address indexed caller, address indexed beneficiary, uint256 price, string name).
pub const TOPIC_NAME_BOUGHT: &str =
    "0xb8c56202a5ae8b00edfcd57a54ec6c3fb8d2f6deb3067a7ba11408a7bd014a3e";

struct NamePurchase {
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
        nft_index::start_block(self.net())
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
                topics: json!([[sales_index::TOPIC_BID_ACCEPTED]]),
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
            let (block, index) = chain_walk::log_position(log);
            let time = *times
                .get(&block)
                .context("missing Ethereum sale block time")?;
            if log.address.eq_ignore_ascii_case(ERC721_BID) {
                if let Some(bid) = sales_index::decode_log(log, time)? {
                    ensure!(
                        bid.kind == sales_index::KIND_BID_ACCEPTED,
                        "unexpected Ethereum bid log"
                    );
                    bids.push(bid);
                }
            } else if NAME_CONTROLLERS
                .iter()
                .any(|a| log.address.eq_ignore_ascii_case(a))
            {
                ensure!(
                    log.topics.len() == 3 && log.topics[0].eq_ignore_ascii_case(TOPIC_NAME_BOUGHT),
                    "unexpected name controller log"
                );
                let caller =
                    chain_walk::topic_address(&log.topics[1]).context("NameBought caller")?;
                let beneficiary =
                    chain_walk::topic_address(&log.topics[2]).context("NameBought beneficiary")?;
                let data = hex::decode(log.data.trim_start_matches("0x"))?;
                let (price, name) =
                    <(sol_data::Uint<256>, sol_data::String)>::abi_decode_params(&data)
                        .context("NameBought payload")?;
                let hash = log
                    .transaction_hash
                    .as_deref()
                    .context("NameBought transaction hash")?;
                names.push(NamePurchase {
                    venue: log.address.to_ascii_lowercase(),
                    block,
                    index: i32::try_from(index)?,
                    time,
                    caller,
                    beneficiary,
                    price: price.to_string(),
                    name,
                    tx_hash: hash.to_ascii_lowercase(),
                });
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

    async fn rewind(&self, conn: &mut PgConnection, to: i64) -> anyhow::Result<()> {
        super::sale_terms::rewind(conn, self.net(), to).await?;
        sqlx::query("DELETE FROM shop.name_purchase WHERE block_number > $1")
            .bind(to)
            .execute(&mut *conn)
            .await?;
        sqlx::query("DELETE FROM shop.sale_event WHERE network = 'ethereum' AND block_number > $1")
            .bind(to)
            .execute(conn)
            .await?;
        Ok(())
    }

    async fn fold(
        &self,
        _: &PgPool,
        _: Option<i64>,
        _: i64,
        _: &chain_walk::Owner,
    ) -> anyhow::Result<bool> {
        Ok(true)
    }
}
