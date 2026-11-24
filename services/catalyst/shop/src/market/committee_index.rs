//! The curation committee index: the Committee contracts'
//! `MemberSet(address indexed member, bool value)` logs on the Polygon chain
//! the walk reads (`MARKETPLACE_INDEX_CHAIN_ID_MATIC`: 137, or Amoy's 80002),
//! kept in
//! `shop.committee_event` with the cursor advance. A wallet is a member
//! when its latest event across the chain's contracts (chain order) set it,
//! as the collections subgraph's `Account.isCommitteeMember` folds them.

use std::collections::HashMap;

use anyhow::{Context, ensure};
use serde_json::json;
use sqlx::{PgConnection, PgPool};

use super::chain_walk::{ChainIndex, ChainNet, INDEXER_COMMITTEE, LogGroup, topic_address};
use crate::chain_index::rpc::{RpcLog, hex_to_i64};

/// `keccak256("MemberSet(address,bool)")`.
pub const MEMBER_SET: &str = "0x4efc5168d98ad88d2d4784344a20876aea2fa3e8aa503b5fb3164a09019ca3d6";

/// The two Polygon Committee contracts, in deployment order (`OldCommittee`
/// and `Committee` under `matic` in `evm/addresses.json`), and the
/// collections subgraph's start block for the first
/// (`decentraland/collections-graph` `config/matic.json`
/// `startBlock_old_committee` at ab6c586).
pub const CONTRACTS: [&str; 2] = [
    "0x71d9350ef44e1e451f00e447c0dff2d1fb75510a",
    "0xaeec95a8aa671a6d3fec56594827d7804964fa70",
];
pub const START_BLOCK: u64 = 15_202_000;

/// The Amoy Committee contract (`Committee` under `amoy` in
/// `evm/addresses.json`, which lists no Amoy `OldCommittee`) and the
/// collections subgraph's start block for it
/// (`decentraland/collections-graph` `config/amoy.json`
/// `startBlock_committee` at 5ffbfbe).
pub const AMOY_CONTRACTS: [&str; 1] = ["0x53c9c388000b6fc91b91fb466742d0e869785c3b"];
pub const AMOY_START_BLOCK: u64 = 5_706_678;

/// The Committee contracts of `chain_id` and the block its walk starts
/// from; none for any other chain.
pub fn deployments(chain_id: i64) -> (&'static [&'static str], u64) {
    match chain_id {
        137 => (&CONTRACTS, START_BLOCK),
        80_002 => (&AMOY_CONTRACTS, AMOY_START_BLOCK),
        _ => (&[], 0),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct MemberSet {
    block: i64,
    index: i32,
    contract: String,
    member: String,
    is_member: bool,
}

fn decode(contracts: &[&str], log: &RpcLog) -> anyhow::Result<MemberSet> {
    let contract = log.address.to_ascii_lowercase();
    ensure!(
        contracts.contains(&contract.as_str()),
        "MemberSet from an unknown emitter"
    );
    ensure!(
        log.topics.len() == 2 && log.topics[0].eq_ignore_ascii_case(MEMBER_SET),
        "invalid MemberSet topics"
    );
    let member = topic_address(&log.topics[1]).context("invalid member topic")?;
    let data = hex::decode(log.data.strip_prefix("0x").context("data prefix")?)?;
    ensure!(
        data.len() == 32 && data[..31].iter().all(|b| *b == 0) && data[31] <= 1,
        "invalid MemberSet value"
    );
    let block = hex_to_i64(&log.block_number)
        .filter(|n| *n >= 0)
        .context("invalid block")?;
    let index = hex_to_i64(&log.log_index)
        .and_then(|n| i32::try_from(n).ok())
        .filter(|n| *n >= 0)
        .context("invalid log index")?;
    Ok(MemberSet {
        block,
        index,
        contract,
        member,
        is_member: data[31] == 1,
    })
}

/// Polygon only: the curation committee, on the chain its RPC serves.
pub struct CommitteeIndex {
    pub chain_id: i64,
}

impl CommitteeIndex {
    /// The walk of Polygon mainnet.
    pub fn mainnet() -> Self {
        Self {
            chain_id: ChainNet::Matic.chain_id(),
        }
    }
}

#[async_trait::async_trait]
impl ChainIndex for CommitteeIndex {
    fn indexer(&self) -> &'static str {
        INDEXER_COMMITTEE
    }
    fn net(&self) -> ChainNet {
        ChainNet::Matic
    }
    fn chain_id(&self) -> i64 {
        self.chain_id
    }
    fn start_block(&self) -> u64 {
        deployments(self.chain_id).1
    }
    async fn groups(
        &self,
        _: &PgPool,
        _: u64,
        _: u64,
        _: (u64, i64),
    ) -> anyhow::Result<Vec<LogGroup>> {
        Ok(vec![LogGroup {
            addresses: deployments(self.chain_id)
                .0
                .iter()
                .map(|a| a.to_string())
                .collect(),
            topics: json!([MEMBER_SET]),
        }])
    }
    async fn write(
        &self,
        conn: &mut PgConnection,
        logs: &[RpcLog],
        times: &HashMap<i64, i64>,
    ) -> anyhow::Result<usize> {
        let contracts = deployments(self.chain_id).0;
        let rows = logs
            .iter()
            .map(|log| decode(contracts, log))
            .collect::<anyhow::Result<Vec<_>>>()?;
        let time = |block: i64| {
            times
                .get(&block)
                .copied()
                .context("missing block timestamp")
        };
        let block_times = rows
            .iter()
            .map(|r| time(r.block))
            .collect::<anyhow::Result<Vec<_>>>()?;
        let written = sqlx::query(
            "INSERT INTO shop.committee_event (network, block_number, log_index, block_time, contract_address, member, is_member) \
             SELECT $1, b, i, t, c, m, v FROM UNNEST($2::bigint[], $3::int[], $4::bigint[], $5::text[], $6::text[], $7::bool[]) AS u(b, i, t, c, m, v) \
             ON CONFLICT (network, block_number, log_index) DO NOTHING",
        )
        .bind(ChainNet::Matic.db_name())
        .bind(rows.iter().map(|r| r.block).collect::<Vec<_>>())
        .bind(rows.iter().map(|r| r.index).collect::<Vec<_>>())
        .bind(block_times)
        .bind(rows.iter().map(|r| r.contract.as_str()).collect::<Vec<_>>())
        .bind(rows.iter().map(|r| r.member.as_str()).collect::<Vec<_>>())
        .bind(rows.iter().map(|r| r.is_member).collect::<Vec<_>>())
        .execute(conn)
        .await?
        .rows_affected();
        Ok(written as usize)
    }
    async fn rewind(&self, conn: &mut PgConnection, to: i64) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM shop.committee_event WHERE network = $1 AND block_number > $2")
            .bind(ChainNet::Matic.db_name())
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
        _: &super::chain_walk::Owner,
    ) -> anyhow::Result<bool> {
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contracts_match_the_pinned_address_book() {
        let pinned: serde_json::Value =
            serde_json::from_str(include_str!("../evm/addresses.json")).unwrap();
        let book = |key: &str, name: &str| pinned[key][name].as_str().unwrap().to_lowercase();
        assert_eq!(
            CONTRACTS,
            [book("matic", "OldCommittee"), book("matic", "Committee")]
        );
        assert_eq!(AMOY_CONTRACTS, [book("amoy", "Committee")]);
    }

    #[test]
    fn a_walk_reads_its_chains_committee_and_no_other() {
        assert_eq!(CommitteeIndex::mainnet().chain_id(), 137);
        assert_eq!(CommitteeIndex::mainnet().start_block(), START_BLOCK);
        let amoy = CommitteeIndex { chain_id: 80_002 };
        assert_eq!(
            (amoy.chain_id(), amoy.start_block()),
            (80_002, AMOY_START_BLOCK)
        );
        let member = "0x00000000000000000000000000000000000000aa";
        assert!(decode(deployments(80_002).0, &log(AMOY_CONTRACTS[0], member, 1)).is_ok());
        for contract in CONTRACTS {
            assert!(
                decode(deployments(80_002).0, &log(contract, member, 1)).is_err(),
                "{contract}"
            );
        }
        assert!(decode(deployments(137).0, &log(AMOY_CONTRACTS[0], member, 1)).is_err());
        assert_eq!(deployments(1).0.len(), 0);
    }

    fn log(contract: &str, member: &str, value: u8) -> RpcLog {
        RpcLog {
            address: contract.into(),
            topics: vec![MEMBER_SET.into(), format!("0x{:0>64}", &member[2..])],
            data: format!("0x{value:064x}"),
            block_number: "0x10".into(),
            log_index: "0x2".into(),
            block_timestamp: None,
            block_hash: None,
            transaction_hash: None,
        }
    }

    #[test]
    fn member_set_topic_is_the_event_signature() {
        assert_eq!(
            format!(
                "{:#x}",
                alloy_primitives::keccak256("MemberSet(address,bool)")
            ),
            MEMBER_SET
        );
    }

    #[test]
    fn decodes_both_contracts_and_rejects_other_emitters() {
        let member = "0x00000000000000000000000000000000000000aa";
        for contract in CONTRACTS {
            let set = decode(
                &CONTRACTS,
                &log(
                    &contract.to_ascii_uppercase().replace("0X", "0x"),
                    member,
                    1,
                ),
            )
            .unwrap();
            assert_eq!(
                set,
                MemberSet {
                    block: 16,
                    index: 2,
                    contract: contract.into(),
                    member: member.into(),
                    is_member: true,
                }
            );
            assert!(
                !decode(&CONTRACTS, &log(contract, member, 0))
                    .unwrap()
                    .is_member
            );
            assert!(decode(&CONTRACTS, &log(contract, member, 2)).is_err());
        }
        assert!(
            decode(
                &CONTRACTS,
                &log("0x00000000000000000000000000000000000000bb", member, 1)
            )
            .is_err()
        );
    }
}
