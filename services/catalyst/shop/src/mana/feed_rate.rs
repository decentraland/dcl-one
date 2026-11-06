//! The `shop-mana-rate` job: the Shop's MANA/USD rate is the answer of the
//! feed the settlement chain's marketplace converts USD-pegged prices with
//! (`MANA_USD_AGGREGATOR_ADDRESS` over `RPC_ENDPOINT_POLYGON`), as upstream's
//! `readManaUsdRate` reads it (shop@3a322cf9 `app/src/lib/mana-rate.ts`). The
//! market spot (CoinGecko) can sit far from that feed (on Amoy the feed is a
//! test answer), and a MANA price the Shop shows must be the one the contract
//! charges. Each pass reads `decimals()` and `latestRoundData()` at `latest`
//! and appends one `economy.price_snapshots` row with source
//! `marketplace-feed`, which only the Shop reads (`price::store::shop_rate`).
//!
//! A round the feed itself has not refreshed for longer than upstream's
//! `MAX_STALENESS_SECONDS` (the heartbeat plus an hour), an incomplete round
//! or a non-positive answer writes nothing: the Shop's prices then go stale
//! and unavailable rather than priced off a dead feed.

use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::{Address, U256};
use alloy_sol_types::SolCall;
use anyhow::Context as _;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::evm::chain::ChainComponent;
use crate::evm::oracle::{decimalsCall, latestRoundDataCall};
use crate::jobs::{Jobs, PeriodicCfg};
use crate::price::poller::{SnapshotRow, insert_snapshot};
use crate::price::store::SOURCE_MARKETPLACE_FEED;

/// Upstream's `MAX_STALENESS_SECONDS`: the feed's heartbeat plus an hour.
pub const MAX_FEED_AGE_SECS: i64 = 90_000;

/// Five passes fit [`crate::price::store::SNAPSHOT_MAX_AGE`] (30 min), as the
/// market spot's own default poll does.
pub const INTERVAL: Duration = Duration::from_secs(300);

/// One read of the feed: its answer as USD per MANA and when it last updated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeedRead {
    pub usd_per_mana: f64,
    pub updated_at: DateTime<Utc>,
}

/// Decodes and checks a `decimals()` and a `latestRoundData()` answer at `now`.
pub fn reading(
    decimals_ret: &[u8],
    round_ret: &[u8],
    now: DateTime<Utc>,
) -> Result<FeedRead, String> {
    let decimals =
        decimalsCall::abi_decode_returns(decimals_ret).map_err(|e| format!("decimals(): {e}"))?;
    let round = latestRoundDataCall::abi_decode_returns(round_ret)
        .map_err(|e| format!("latestRoundData(): {e}"))?;
    if round.answer.is_negative() || round.answer.is_zero() {
        return Err(format!(
            "the feed answers {}, not a positive rate",
            round.answer
        ));
    }
    if round.answeredInRound < round.roundId {
        return Err("the feed's latest round is incomplete".into());
    }
    let updated_at = i64::try_from(round.updatedAt)
        .ok()
        .and_then(|s| DateTime::<Utc>::from_timestamp(s, 0))
        .filter(|t| t.timestamp() > 0)
        .ok_or_else(|| format!("the feed's updatedAt {} is not a time", round.updatedAt))?;
    let age = (now - updated_at).num_seconds();
    if age > MAX_FEED_AGE_SECS {
        return Err(format!(
            "the feed's latest round is {age} s old (at most {MAX_FEED_AGE_SECS} s)"
        ));
    }
    let answer: U256 = round.answer.into_raw();
    let usd_per_mana = answer
        .to_string()
        .parse::<f64>()
        .map_err(|e| e.to_string())?
        / 10f64.powi(i32::from(decimals));
    if !usd_per_mana.is_finite() || usd_per_mana <= 0.0 {
        return Err(format!(
            "the feed's answer {answer} at {decimals} decimals is not a rate"
        ));
    }
    Ok(FeedRead {
        usd_per_mana,
        updated_at,
    })
}

/// One pass: read the feed and append its row.
pub async fn run_pass(
    chain: &ChainComponent,
    aggregator: Address,
    pool: &PgPool,
) -> anyhow::Result<FeedRead> {
    let decimals = chain
        .eth_call(aggregator, &decimalsCall {}.abi_encode())
        .await
        .map_err(|e| anyhow::anyhow!("reading the MANA/USD feed's decimals: {e}"))?;
    let round = chain
        .eth_call(aggregator, &latestRoundDataCall {}.abi_encode())
        .await
        .map_err(|e| anyhow::anyhow!("reading the MANA/USD feed's latest round: {e}"))?;
    let now = Utc::now();
    let read = reading(&decimals, &round, now)
        .map_err(|e| anyhow::anyhow!("the MANA/USD feed at {aggregator:#x}: {e}"))?;
    let row = SnapshotRow {
        source_updated_at: Some(read.updated_at),
        mana_usd: Some(read.usd_per_mana),
        mana_eth: None,
        mana_btc: None,
        mana_matic: None,
        matic_usd: None,
        mana_market_cap_usd: None,
        mana_volume_24h_usd: None,
        mana_price_change_24h_pct: None,
    };
    insert_snapshot(pool, SOURCE_MARKETPLACE_FEED, &row, now)
        .await
        .context("appending the marketplace feed's rate")?;
    Ok(read)
}

/// Starts the job where the settlement chain has an endpoint and writes are on.
pub fn spawn(
    jobs: &Jobs,
    pool: PgPool,
    chain: Option<Arc<ChainComponent>>,
    aggregator: Address,
    writes_enabled: bool,
) {
    let Some(chain) = chain.filter(|_| writes_enabled && aggregator != Address::ZERO) else {
        tracing::info!(
            "shop-mana-rate: not started (no RPC_ENDPOINT_POLYGON, no aggregator, or writes off); the Shop prices at the market spot"
        );
        return;
    };
    jobs.spawn_periodic_leased(
        "shop-mana-rate",
        PeriodicCfg::every(INTERVAL).with_jitter(Duration::from_secs(15)).with_error_backoff(Duration::from_secs(60)),
        move |_ctx| {
            let (chain, pool) = (chain.clone(), pool.clone());
            async move {
                let read = run_pass(&chain, aggregator, &pool).await?;
                tracing::debug!(usd_per_mana = read.usd_per_mana, updated_at = %read.updated_at, "shop-mana-rate: wrote the marketplace feed's rate");
                Ok(())
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::I256;

    fn word(v: U256) -> [u8; 32] {
        v.to_be_bytes::<32>()
    }

    fn encode_round(round_id: u64, answer: i64, updated_at: i64, answered_in: u64) -> Vec<u8> {
        let mut out = Vec::with_capacity(5 * 32);
        out.extend_from_slice(&word(U256::from(round_id)));
        out.extend_from_slice(&I256::try_from(answer).unwrap().to_be_bytes::<32>());
        out.extend_from_slice(&word(U256::from(updated_at as u64)));
        out.extend_from_slice(&word(U256::from(updated_at as u64)));
        out.extend_from_slice(&word(U256::from(answered_in)));
        out
    }

    fn decimals(d: u8) -> Vec<u8> {
        word(U256::from(d)).to_vec()
    }

    #[test]
    fn a_fresh_round_is_the_rate_at_its_decimals() {
        let now = DateTime::<Utc>::from_timestamp(1_800_000_000, 0).unwrap();
        let read = reading(
            &decimals(8),
            &encode_round(7, 26_960_836, now.timestamp() - 3_600, 7),
            now,
        )
        .unwrap();
        assert_eq!(
            read.usd_per_mana, 0.269_608_36,
            "the Amoy answer of 2026-11-06"
        );
        assert_eq!(read.updated_at.timestamp(), now.timestamp() - 3_600);
    }

    #[test]
    fn a_dead_incomplete_or_non_positive_round_is_refused() {
        let now = DateTime::<Utc>::from_timestamp(1_800_000_000, 0).unwrap();
        let old = now.timestamp() - MAX_FEED_AGE_SECS - 1;
        assert!(
            reading(&decimals(8), &encode_round(7, 26_960_836, old, 7), now)
                .unwrap_err()
                .contains("old")
        );
        assert!(
            reading(
                &decimals(8),
                &encode_round(7, 26_960_836, now.timestamp(), 6),
                now
            )
            .unwrap_err()
            .contains("incomplete")
        );
        assert!(
            reading(&decimals(8), &encode_round(7, 0, now.timestamp(), 7), now)
                .unwrap_err()
                .contains("positive")
        );
        assert!(
            reading(&decimals(8), &encode_round(7, -5, now.timestamp(), 7), now)
                .unwrap_err()
                .contains("positive")
        );
        assert!(
            reading(&decimals(8), &encode_round(7, 26_960_836, 0, 7), now)
                .unwrap_err()
                .contains("not a time")
        );
    }
}
