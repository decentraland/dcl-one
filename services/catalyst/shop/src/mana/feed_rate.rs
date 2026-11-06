//! The Shop's MANA/USD rate: the answer of the feed the settlement chain's
//! marketplace converts USD-pegged prices with (`MANA_USD_AGGREGATOR_ADDRESS`
//! over `RPC_ENDPOINT_POLYGON`), read live as upstream's `readManaUsdRate`
//! does (shop@3a322cf9 `app/src/lib/mana-rate.ts`), so a MANA price the Shop
//! shows is the one the contract charges. The market spot (CoinGecko) can sit
//! far from that feed (on Amoy the feed is a test answer), and it stays the
//! economy routes' price only.
//!
//! [`install`] wires the feed once per process (`serve`); [`read`] answers
//! `None` where it is not wired (no endpoint or aggregator: the Shop prices at
//! the spot as before). A read is `decimals()` and `latestRoundData()` at
//! `latest`, kept [`CACHE_TTL`] so a page of prices costs one round trip. A
//! failed read is not kept, and is the Shop's "price unavailable", never the
//! spot. A round the feed has not refreshed for longer than upstream's
//! `MAX_STALENESS_SECONDS` (the heartbeat plus an hour), an incomplete round
//! or a non-positive answer is refused.
//!
//! The rate can still move between a quote and the block that settles it;
//! the approval a USD-pegged quote asks for carries upstream's +2% for that
//! ([`USD_PEGGED_ALLOWANCE_BUFFER_PCT`]).

use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use alloy_primitives::{Address, U256};
use alloy_sol_types::SolCall;
use chrono::{DateTime, Utc};

use crate::evm::chain::ChainComponent;
use crate::evm::oracle::{decimalsCall, latestRoundDataCall};

/// Upstream's `MAX_STALENESS_SECONDS`: the feed's heartbeat plus an hour.
pub const MAX_FEED_AGE_SECS: i64 = 90_000;

/// How long one read answers for: upstream's rate query is stale after 60 s.
pub const CACHE_TTL: Duration = Duration::from_secs(30);

/// Upstream's approval buffer over a USD-pegged trade's MANA (shop@3a322cf9
/// `app/src/lib/buy.ts` `tradeManaPriceWei`: `mul(102).div(100)`).
pub const USD_PEGGED_ALLOWANCE_BUFFER_PCT: u64 = 2;

/// One read of the feed.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedRead {
    /// USD per MANA as exact decimal text ("0.26960836").
    pub usd_per_mana: String,
    /// When the feed last updated its answer.
    pub updated_at: DateTime<Utc>,
}

struct LiveFeed {
    chain: Arc<ChainComponent>,
    aggregator: Address,
    last: tokio::sync::Mutex<Option<(Instant, FeedRead)>>,
}

static FEED: OnceLock<LiveFeed> = OnceLock::new();

/// Wires the feed for this process; a second call is ignored.
pub fn install(chain: Option<Arc<ChainComponent>>, aggregator: Address) {
    let Some(chain) = chain.filter(|_| aggregator != Address::ZERO) else {
        tracing::info!(
            "shop MANA/USD: no RPC_ENDPOINT_POLYGON or aggregator; the Shop prices at the market spot"
        );
        return;
    };
    let _ = FEED.set(LiveFeed {
        chain,
        aggregator,
        last: tokio::sync::Mutex::new(None),
    });
}

/// The feed's rate, at most [`CACHE_TTL`] old; `None` where no feed is wired.
pub async fn read() -> Option<Result<FeedRead, String>> {
    let feed = FEED.get()?;
    let mut last = feed.last.lock().await;
    if let Some((at, read)) = last.as_ref()
        && at.elapsed() < CACHE_TTL
    {
        return Some(Ok(read.clone()));
    }
    let fresh = read_chain(&feed.chain, feed.aggregator).await;
    if let Ok(read) = &fresh {
        *last = Some((Instant::now(), read.clone()));
    } else {
        *last = None;
    }
    Some(fresh)
}

async fn read_chain(chain: &ChainComponent, aggregator: Address) -> Result<FeedRead, String> {
    let decimals = chain
        .eth_call(aggregator, &decimalsCall {}.abi_encode())
        .await
        .map_err(|e| format!("the MANA/USD feed's decimals could not be read ({e})"))?;
    let round = chain
        .eth_call(aggregator, &latestRoundDataCall {}.abi_encode())
        .await
        .map_err(|e| format!("the MANA/USD feed's latest round could not be read ({e})"))?;
    reading(&decimals, &round, Utc::now())
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
            "the MANA/USD feed answers {}, not a positive rate",
            round.answer
        ));
    }
    if round.answeredInRound < round.roundId {
        return Err("the MANA/USD feed's latest round is incomplete".into());
    }
    let updated_at = i64::try_from(round.updatedAt)
        .ok()
        .filter(|s| *s > 0)
        .and_then(|s| DateTime::<Utc>::from_timestamp(s, 0))
        .ok_or_else(|| {
            format!(
                "the MANA/USD feed's updatedAt {} is not a time",
                round.updatedAt
            )
        })?;
    let age = (now - updated_at).num_seconds();
    if age > MAX_FEED_AGE_SECS {
        return Err(format!(
            "the MANA/USD feed's latest round is {age} s old (at most {MAX_FEED_AGE_SECS} s)"
        ));
    }
    Ok(FeedRead {
        usd_per_mana: decimal_text(round.answer.into_raw(), decimals),
        updated_at,
    })
}

/// `answer / 10^decimals` as exact decimal text, trailing zeros dropped.
pub fn decimal_text(answer: U256, decimals: u8) -> String {
    let digits = answer.to_string();
    let scale = usize::from(decimals);
    if scale == 0 {
        return digits;
    }
    let padded = format!("{digits:0>width$}", width = scale + 1);
    let (whole, fraction) = padded.split_at(padded.len() - scale);
    let fraction = fraction.trim_end_matches('0');
    if fraction.is_empty() {
        whole.to_string()
    } else {
        format!("{whole}.{fraction}")
    }
}

/// The approval a USD-pegged trade's `mana` asks for: upstream's +2%.
pub fn usd_pegged_allowance(mana: U256) -> U256 {
    mana.checked_mul(U256::from(100 + USD_PEGGED_ALLOWANCE_BUFFER_PCT))
        .map(|scaled| scaled / U256::from(100))
        .unwrap_or(mana)
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
    fn a_fresh_round_is_the_exact_rate_at_its_decimals() {
        let now = DateTime::<Utc>::from_timestamp(1_800_000_000, 0).unwrap();
        let read = reading(
            &decimals(8),
            &encode_round(7, 26_960_836, now.timestamp() - 3_600, 7),
            now,
        )
        .unwrap();
        assert_eq!(
            read.usd_per_mana, "0.26960836",
            "the Amoy answer of 2026-11-06"
        );
        assert_eq!(read.updated_at.timestamp(), now.timestamp() - 3_600);
        let read = reading(
            &decimals(8),
            &encode_round(9, 7_240_733, now.timestamp(), 9),
            now,
        )
        .unwrap();
        assert_eq!(
            read.usd_per_mana, "0.07240733",
            "the Polygon answer of 2026-11-06"
        );
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

    #[test]
    fn the_rate_text_is_exact() {
        assert_eq!(decimal_text(U256::from(26_960_836u64), 8), "0.26960836");
        assert_eq!(decimal_text(U256::from(100_000_000u64), 8), "1");
        assert_eq!(decimal_text(U256::from(150_000_000u64), 8), "1.5");
        assert_eq!(decimal_text(U256::from(5u64), 8), "0.00000005");
        assert_eq!(decimal_text(U256::from(42u64), 0), "42");
    }

    #[test]
    fn a_usd_pegged_approval_carries_two_percent() {
        assert_eq!(
            usd_pegged_allowance(U256::from(5_192_717_317_816_109_517u128)),
            U256::from(5_296_571_664_172_431_707u128)
        );
        assert_eq!(usd_pegged_allowance(U256::from(100u64)), U256::from(102u64));
    }
}
