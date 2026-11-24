//! Historical venue terms, ordered before the sale within its block.
//! A missing event remains unknown; current contract state cannot fill it.

use std::sync::LazyLock;

use alloy_primitives::{U256, keccak256};
use anyhow::{Context, ensure};
use serde_json::json;
use sqlx::PgConnection;

use super::chain_walk::{self, ChainNet, LogGroup};
use super::{ethereum_sales, nft_index, sales_index, trade_events};
use crate::chain_index::rpc::RpcLog;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Venue {
    Legacy,
    Royalties,
    Store,
    Offchain,
}

struct Event {
    topic: String,
    venues: Venue,
    field: &'static str,
    word: Word,
}
enum Word {
    Last,
    Address,
}
static EVENTS: LazyLock<Vec<Event>> = LazyLock::new(|| {
    use Venue::*;
    [
        (
            Legacy,
            "OwnershipTransferred(address,address)",
            "collector",
            Word::Address,
        ),
        (
            Legacy,
            "ChangedOwnerCutPerMillion(uint256)",
            "fee_ppm",
            Word::Last,
        ),
        (
            Royalties,
            "FeesCollectorSet(address,address)",
            "collector",
            Word::Address,
        ),
        (
            Royalties,
            "ChangedFeesCollectorCutPerMillion(uint256)",
            "fee_ppm",
            Word::Last,
        ),
        (
            Royalties,
            "ChangedRoyaltiesCutPerMillion(uint256)",
            "royalty_ppm",
            Word::Last,
        ),
        (
            Store,
            "SetFeeOwner(address,address)",
            "collector",
            Word::Address,
        ),
        (Store, "SetFee(uint256,uint256)", "fee_ppm", Word::Last),
        (
            Offchain,
            "FeeCollectorUpdated(address,address)",
            "collector",
            Word::Address,
        ),
        (
            Offchain,
            "FeeRateUpdated(address,uint256)",
            "fee_ppm",
            Word::Last,
        ),
        (
            Offchain,
            "RoyaltiesRateUpdated(address,uint256)",
            "royalty_ppm",
            Word::Last,
        ),
    ]
    .into_iter()
    .map(|(venues, signature, field, word)| Event {
        topic: format!("{:#x}", keccak256(signature)),
        venues,
        field,
        word,
    })
    .collect()
});

fn venues(net: ChainNet) -> Vec<(&'static str, Venue)> {
    let mut venues: Vec<_> = nft_index::legacy_marketplaces(net)
        .iter()
        .enumerate()
        .map(|(i, &(address, _))| {
            (
                address,
                if i == 0 {
                    Venue::Legacy
                } else {
                    Venue::Royalties
                },
            )
        })
        .collect();
    venues.extend(
        trade_events::marketplaces(net)
            .iter()
            .map(|&(a, _, _)| (a, Venue::Offchain)),
    );
    match net {
        ChainNet::Matic => venues.extend([
            (sales_index::STORE_MATIC, Venue::Store),
            (sales_index::BID_V2_MATIC, Venue::Royalties),
        ]),
        ChainNet::Ethereum => venues.push((ethereum_sales::ERC721_BID, Venue::Legacy)),
    }
    venues
}

pub fn group(net: ChainNet) -> LogGroup {
    LogGroup {
        addresses: venues(net).into_iter().map(|(a, _)| a.into()).collect(),
        topics: json!([EVENTS.iter().map(|e| &e.topic).collect::<Vec<_>>()]),
    }
}

pub async fn write(conn: &mut PgConnection, net: ChainNet, logs: &[RpcLog]) -> anyhow::Result<()> {
    let known = venues(net);
    for log in logs {
        let Some(event) = EVENTS.iter().find(|e| {
            log.topics
                .first()
                .is_some_and(|t| e.topic.eq_ignore_ascii_case(t))
        }) else {
            continue;
        };
        if !known
            .iter()
            .any(|(a, v)| a.eq_ignore_ascii_case(&log.address) && *v == event.venues)
        {
            continue;
        }
        let value = match event.word {
            Word::Address => {
                chain_walk::topic_address(log.topics.get(2).context("missing fee recipient")?)
                    .context("invalid fee recipient")?
            }
            Word::Last => {
                let data = hex::decode(log.data.trim_start_matches("0x"))?;
                ensure!(
                    data.len() == if event.venues == Venue::Store { 64 } else { 32 },
                    "invalid fee event width"
                );
                U256::from_be_slice(&data[data.len() - 32..]).to_string()
            }
        };
        let (block, index) = chain_walk::log_position(log);
        sqlx::query("INSERT INTO shop.sale_fee_change (network, venue, field, block_number, log_index, value)
                     VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING")
            .bind(net.db_name()).bind(log.address.to_ascii_lowercase()).bind(event.field).bind(block)
            .bind(i32::try_from(index)?).bind(value).execute(&mut *conn).await?;
    }
    Ok(())
}

pub async fn rewind(conn: &mut PgConnection, net: ChainNet, to: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM shop.sale_fee_change WHERE network = $1 AND block_number > $2")
        .bind(net.db_name())
        .bind(to)
        .execute(conn)
        .await?;
    Ok(())
}
