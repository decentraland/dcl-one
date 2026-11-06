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
//! `latest`. One read at a time is in flight ([`RateCache`]): its answer is
//! shared with every caller that waited for it, kept [`CACHE_TTL`] when it is
//! a rate and [`FAILURE_TTL`] when it is a failure, so a dead endpoint costs
//! one read per [`FAILURE_TTL`], not one per waiting caller. A caller waits
//! at most [`READ_BUDGET`] for the lock and the read together. A failure is
//! the Shop's "price unavailable", never the spot, and never an older rate.
//! A round the feed has not refreshed for longer than upstream's
//! `MAX_STALENESS_SECONDS` (the heartbeat plus an hour), an incomplete round
//! or a non-positive answer is refused. [`FeedRead::observed_at`] is when
//! this node read the answer, so a caller never presents a cached read as
//! fresher than it is.
//!
//! The rate can still move between a quote and the block that settles it;
//! the approval a USD-pegged quote asks for carries upstream's +2% for that
//! ([`USD_PEGGED_ALLOWANCE_BUFFER_PCT`]).

use std::future::Future;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use tokio::time::Instant;

use alloy_primitives::{Address, U256};
use alloy_sol_types::SolCall;
use chrono::{DateTime, Utc};

use crate::evm::chain::ChainComponent;
use crate::evm::oracle::{decimalsCall, latestRoundDataCall};

/// Upstream's `MAX_STALENESS_SECONDS`: the feed's heartbeat plus an hour.
pub const MAX_FEED_AGE_SECS: i64 = 90_000;

/// How long one read answers for: upstream's rate query is stale after 60 s.
pub const CACHE_TTL: Duration = Duration::from_secs(30);

/// How long a failed read answers for, so callers do not queue retries.
pub const FAILURE_TTL: Duration = Duration::from_secs(5);

/// The most a caller waits for a rate: the lock and both calls together.
pub const READ_BUDGET: Duration = Duration::from_secs(12);

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
    /// When this node read it.
    pub observed_at: DateTime<Utc>,
}

/// One read in flight, its outcome shared: a rate for [`CACHE_TTL`], a
/// failure for [`FAILURE_TTL`]; each caller bounded by [`READ_BUDGET`].
pub struct RateCache {
    last: tokio::sync::Mutex<Option<(Instant, Result<FeedRead, String>)>>,
}

impl Default for RateCache {
    fn default() -> Self {
        Self::new()
    }
}

impl RateCache {
    pub const fn new() -> Self {
        Self {
            last: tokio::sync::Mutex::const_new(None),
        }
    }

    pub async fn get<F, Fut>(&self, fetch: F) -> Result<FeedRead, String>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<FeedRead, String>>,
    {
        let deadline = Instant::now() + READ_BUDGET;
        let Ok(mut last) = tokio::time::timeout_at(deadline, self.last.lock()).await else {
            return Err(format!(
                "the MANA/USD feed's read in flight did not finish within {}s",
                READ_BUDGET.as_secs()
            ));
        };
        if let Some((at, outcome)) = last.as_ref() {
            let ttl = if outcome.is_ok() {
                CACHE_TTL
            } else {
                FAILURE_TTL
            };
            if at.elapsed() < ttl {
                return outcome.clone();
            }
        }
        let outcome = match tokio::time::timeout_at(deadline, fetch()).await {
            Ok(outcome) => outcome,
            Err(_) => Err(format!(
                "the MANA/USD feed did not answer within {}s",
                READ_BUDGET.as_secs()
            )),
        };
        *last = Some((Instant::now(), outcome.clone()));
        outcome
    }
}

struct LiveFeed {
    chain: Arc<ChainComponent>,
    aggregator: Address,
    cache: RateCache,
}

static FEED: OnceLock<LiveFeed> = OnceLock::new();

/// Whether this process reads the marketplace's feed ([`install`]).
pub fn installed() -> bool {
    FEED.get().is_some()
}

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
        cache: RateCache::new(),
    });
}

/// The feed's rate, read at most [`CACHE_TTL`] ago; `None` where no feed is wired.
pub async fn read() -> Option<Result<FeedRead, String>> {
    let feed = FEED.get()?;
    Some(
        feed.cache
            .get(|| read_chain(&feed.chain, feed.aggregator))
            .await,
    )
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
        observed_at: now,
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

    fn rate(text: &str) -> FeedRead {
        let at = DateTime::<Utc>::from_timestamp(1_800_000_000, 0).unwrap();
        FeedRead {
            usd_per_mana: text.into(),
            updated_at: at,
            observed_at: at,
        }
    }

    #[tokio::test(start_paused = true)]
    async fn waiting_callers_share_one_slow_failure_then_the_feed_recovers() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let cache = Arc::new(RateCache::new());
        let reads = Arc::new(AtomicUsize::new(0));
        let started = Instant::now();
        let callers: Vec<_> = (0..4)
            .map(|_| {
                let (cache, reads) = (cache.clone(), reads.clone());
                tokio::spawn(async move {
                    cache
                        .get(|| async move {
                            reads.fetch_add(1, Ordering::SeqCst);
                            tokio::time::sleep(Duration::from_secs(8)).await;
                            Err::<FeedRead, _>("rpc down".to_string())
                        })
                        .await
                })
            })
            .collect();
        for caller in callers {
            assert_eq!(caller.await.unwrap().unwrap_err(), "rpc down");
        }
        assert_eq!(
            reads.load(Ordering::SeqCst),
            1,
            "one read for every waiting caller"
        );
        assert!(
            started.elapsed() < Duration::from_secs(9),
            "the failure is not retried in a queue"
        );
        tokio::time::advance(FAILURE_TTL).await;
        let fresh = cache
            .get(|| async {
                reads.fetch_add(1, Ordering::SeqCst);
                Ok(rate("0.26960836"))
            })
            .await;
        assert_eq!(
            fresh.unwrap().usd_per_mana,
            "0.26960836",
            "the feed is read again once the failure lapses"
        );
        assert_eq!(reads.load(Ordering::SeqCst), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn a_rate_answers_for_thirty_seconds_and_a_failure_never_serves_it_again() {
        let cache = RateCache::new();
        let a = cache.get(|| async { Ok(rate("0.1")) }).await.unwrap();
        tokio::time::advance(Duration::from_secs(29)).await;
        let at_29 = cache.get(|| async { Ok(rate("0.2")) }).await.unwrap();
        assert_eq!(at_29, a, "t=29: the read of t=0");
        tokio::time::advance(Duration::from_secs(2)).await;
        let at_31 = cache.get(|| async { Err("rpc down".to_string()) }).await;
        assert_eq!(
            at_31.unwrap_err(),
            "rpc down",
            "t=31: no rate, never the expired one"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_hung_read_is_bounded_and_so_is_the_wait_behind_it() {
        let cache = Arc::new(RateCache::new());
        let started = Instant::now();
        let hung = {
            let cache = cache.clone();
            tokio::spawn(async move {
                cache
                    .get(|| async {
                        tokio::time::sleep(Duration::from_secs(600)).await;
                        Ok(rate("0.1"))
                    })
                    .await
            })
        };
        tokio::task::yield_now().await;
        let behind = cache.get(|| async { Ok(rate("0.2")) }).await;
        assert!(
            behind.unwrap_err().contains("in flight"),
            "the caller behind gives up at its budget"
        );
        assert!(hung.await.unwrap().unwrap_err().contains("did not answer"));
        assert!(started.elapsed() <= READ_BUDGET + Duration::from_secs(1));
    }
}
