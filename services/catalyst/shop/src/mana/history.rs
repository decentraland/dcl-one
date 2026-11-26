//! UTC closes from the Polygon MANA/USD feed. A batch uses one finalized
//! block for every oracle call and commits each completed day before reading the next.

use std::{sync::Arc, time::Duration};

use alloy_primitives::{Address, B256, I256, U256, aliases::U80};
use alloy_sol_types::{SolCall, sol};
use anyhow::{Context, anyhow, ensure};
use chrono::{NaiveDate, NaiveTime, Utc};
use serde_json::json;
use sqlx::PgPool;

use crate::evm::{
    chain::DEFAULT_RPC_TIMEOUT,
    oracle::MANA_USD_AGGREGATOR_POLYGON,
    rpc::{Rpc, RpcError, decode_hex, parse_quantity_u64},
};
use crate::jobs::{Jobs, PeriodicCfg};
use crate::market::MarketplaceState;

const BATCH_DAYS: usize = 14;
const DAY_SECONDS: i64 = 86_400;
const FIRST_DAY: NaiveDate = NaiveDate::from_ymd_opt(2021, 7, 8).unwrap();

sol! {
    function phaseId() external view returns (uint16);
    function phaseAggregators(uint16 phase) external view returns (address);
    function decimals() external view returns (uint8);
    function latestRoundData() external view returns (uint80 roundId, int256 answer, uint256 startedAt, uint256 updatedAt, uint80 answeredInRound);
    function getRoundData(uint80 id) external view returns (uint80 roundId, int256 answer, uint256 startedAt, uint256 updatedAt, uint80 answeredInRound);
}

struct Oracle<'a> {
    rpc: &'a Rpc,
    hash: B256,
}

#[derive(Clone, Copy)]
struct Round {
    index: u64,
    time: i64,
    answer: Option<I256>,
}

struct Phase {
    id: u16,
    address: Address,
    decimals: u8,
    first: Round,
    last: Round,
}

impl Round {
    fn new(
        id: U80,
        updated_at: U256,
        answer: I256,
        answered_in_round: U80,
    ) -> anyhow::Result<Self> {
        let index = u64::try_from(id).context("invalid aggregator round id")?;
        let time = i64::try_from(updated_at).context("invalid oracle round timestamp")?;
        ensure!(index > 0 && time > 0, "oracle round is incomplete");
        Ok(Self {
            index,
            time,
            answer: (answered_in_round >= id).then_some(answer),
        })
    }
}

impl Oracle<'_> {
    async fn call<C: SolCall>(&self, address: Address, call: C) -> anyhow::Result<C::Return> {
        self.call_inner(address, call, false)
            .await?
            .context("oracle call returned no data")
    }

    async fn call_inner<C: SolCall>(
        &self,
        address: Address,
        call: C,
        missing_on_revert: bool,
    ) -> anyhow::Result<Option<C::Return>> {
        let params = json!([
            { "to": format!("{address:#x}"), "data": format!("0x{}", hex::encode(call.abi_encode())) },
            { "blockHash": format!("{:#x}", self.hash), "requireCanonical": true }
        ]);
        let result = match self.rpc.call_hex("eth_call", params).await {
            Ok(result) => result,
            Err(error @ RpcError::Response { .. })
                if missing_on_revert
                    && error.is_execution_revert()
                    && error
                        .message()
                        .to_ascii_lowercase()
                        .contains("no data present") =>
            {
                return Ok(None);
            }
            Err(_) => anyhow::bail!("MANA/USD oracle call {} failed", C::SIGNATURE),
        };
        let bytes = decode_hex(&result).context("MANA/USD oracle returned invalid hex")?;
        Ok(Some(C::abi_decode_returns(&bytes)?))
    }

    async fn round(&self, address: Address, index: u64) -> anyhow::Result<Option<Round>> {
        let result = self
            .call_inner(
                address,
                getRoundDataCall {
                    id: U80::from(index),
                },
                true,
            )
            .await?;
        let Some(result) = result.filter(|result| result.updatedAt != U256::ZERO) else {
            return Ok(None);
        };
        let round = Round::new(
            result.roundId,
            result.updatedAt,
            result.answer,
            result.answeredInRound,
        )?;
        ensure!(round.index == index, "oracle returned a different round");
        Ok(Some(round))
    }

    async fn phases(&self, finalized_at: i64) -> anyhow::Result<Vec<Phase>> {
        let proxy = MANA_USD_AGGREGATOR_POLYGON;
        let current = self.call(proxy, phaseIdCall {}).await?;
        ensure!(current > 0, "MANA/USD proxy has no phase");
        let mut phases = Vec::<Phase>::new();
        for id in 1..=current {
            let address = self.call(proxy, phaseAggregatorsCall { phase: id }).await?;
            ensure!(
                address != Address::ZERO,
                "MANA/USD proxy phase {id} has no aggregator"
            );
            let Some(first) = self.round(address, 1).await? else {
                continue;
            };
            let (last, decimals) = tokio::try_join!(
                self.call(address, latestRoundDataCall {}),
                self.call(address, decimalsCall {}),
            )?;
            let last = Round::new(
                last.roundId,
                last.updatedAt,
                last.answer,
                last.answeredInRound,
            )?;
            ensure!(
                last.time >= first.time
                    && last.time <= finalized_at
                    && decimals <= 18
                    && phases
                        .last()
                        .is_none_or(|phase| phase.first.time <= first.time),
                "invalid MANA/USD phase {id}"
            );
            phases.push(Phase {
                id,
                address,
                decimals,
                first,
                last,
            });
        }
        ensure!(!phases.is_empty(), "MANA/USD feed has no populated phase");
        Ok(phases)
    }

    async fn close(&self, phase: &Phase, at: i64, from: Round) -> anyhow::Result<Round> {
        if phase.last.time <= at {
            return Ok(phase.last);
        }
        let mut best = from;
        let mut hi = phase.last.index - 1;
        while best.index < hi {
            let mid = best.index + (hi - best.index).div_ceil(2);
            let round = self
                .round(phase.address, mid)
                .await?
                .context("missing round inside a populated MANA/USD phase")?;
            ensure!(
                round.time >= best.time && round.time <= phase.last.time,
                "unordered MANA/USD round timestamps"
            );
            if round.time <= at {
                best = round;
            } else {
                hi = mid - 1;
            }
        }
        Ok(best)
    }
}

async fn fill(pool: &PgPool, rpc: &Rpc, today: NaiveDate) -> anyhow::Result<usize> {
    // The writer commits days in order. An unpriced close still
    // records its day so that the next pass advances.
    let start: NaiveDate =
        sqlx::query_scalar("SELECT coalesce(max(day) + 1, $1) FROM shop.mana_usd_daily")
            .bind(FIRST_DAY)
            .fetch_one(pool)
            .await?;
    if start >= today {
        return Ok(0);
    }

    let chain = rpc
        .call_hex("eth_chainId", json!([]))
        .await
        .map_err(|_| anyhow!("MANA/USD history could not read the RPC chain id"))?;
    ensure!(
        parse_quantity_u64(&chain) == Ok(137),
        "MANA/USD history needs a Polygon mainnet endpoint"
    );
    let block = rpc
        .call("eth_getBlockByNumber", json!(["finalized", false]))
        .await
        .map_err(|_| anyhow!("MANA/USD history could not read the finalized Polygon block"))?;
    let hash = block["hash"]
        .as_str()
        .context("missing finalized block hash")?
        .parse()?;
    let finalized_at = block["timestamp"]
        .as_str()
        .and_then(|value| parse_quantity_u64(value).ok())
        .and_then(|time| i64::try_from(time).ok())
        .context("missing finalized block timestamp")?;
    let oracle = Oracle { rpc, hash };
    let mut days = start
        .iter_days()
        .take(BATCH_DAYS)
        .take_while(|day| *day < today)
        .peekable();
    let end = |day: NaiveDate| {
        day.succ_opt()
            .expect("days before today have a successor")
            .and_time(NaiveTime::MIN)
            .and_utc()
            .timestamp()
    };
    if days.peek().is_none_or(|day| end(*day) > finalized_at) {
        return Ok(0);
    }
    let phases = oracle.phases(finalized_at).await?;
    let mut recorded = 0;
    let mut previous = None;
    for day in days.take_while(|day| end(*day) <= finalized_at) {
        let at = end(day) - 1;
        let mut usd = None;
        let mut round_id = None;
        if let Some(phase) = phases.iter().rev().find(|phase| phase.first.time <= at) {
            let from = previous
                .filter(|(id, _)| *id == phase.id)
                .map_or(phase.first, |(_, round)| round);
            let round = oracle.close(phase, at, from).await?;
            previous = Some((phase.id, round));
            // A UTC day needs its own observation. Record empty days so a
            // stalled feed does not carry an earlier close into later days.
            if round.time > at - DAY_SECONDS {
                let id = (u128::from(phase.id) << 64) | u128::from(round.index);
                round_id = Some(id.to_string());
                if let Some(answer) = round.answer.filter(|answer| answer.is_positive()) {
                    let rate = crate::types::ManaUsdRate::new(answer.into_raw(), phase.decimals)
                        .context("invalid MANA/USD close")?;
                    usd = Some(rate.decimal_text());
                }
            }
        }
        // An RPC error or the job deadline can interrupt a later day's
        // search. The next pass starts after the last complete UTC day.
        recorded += sqlx::query(
            "INSERT INTO shop.mana_usd_daily (day,usd,round_id)
             VALUES ($1,$2::text::numeric,$3::text::numeric) ON CONFLICT (day) DO NOTHING",
        )
        .bind(day)
        .bind(usd)
        .bind(round_id)
        .execute(pool)
        .await?
        .rows_affected() as usize;
    }
    Ok(recorded)
}

fn history_endpoint(state: &MarketplaceState) -> Option<String> {
    use crate::market::chain_walk::ChainNet;
    (state.settings.market.indexes.sales_enabled
        && state.settings.catalogue_chain(ChainNet::Matic) == Some(ChainNet::Matic.chain_id()))
    .then(|| {
        state
            .chain_rpc_url(ChainNet::Matic)
            .or_else(|| state.settings.polygon_rpc_url.clone())
    })
    .flatten()
}

pub(crate) fn spawn(state: &Arc<MarketplaceState>, jobs: &Jobs) {
    let Some(endpoint) = history_endpoint(state) else {
        return;
    };
    let rpc = Rpc::new(state.http.clone(), &endpoint, DEFAULT_RPC_TIMEOUT);
    let pool = state.pool.clone();
    jobs.spawn_periodic_leased(
        "mana-usd-history",
        PeriodicCfg::every(Duration::from_secs(60)).with_error_backoff(Duration::from_secs(900)),
        move |ctx| {
            let (pool, rpc) = (pool.clone(), rpc.clone());
            async move {
                let days = tokio::time::timeout(
                    Duration::from_secs(120),
                    fill(&pool, &rpc, Utc::now().date_naive()),
                )
                .await
                .context("MANA/USD history pass timed out")??;
                ctx.progress(format!("{days} UTC closes recorded"));
                Ok(())
            }
        },
    );
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
