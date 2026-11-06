//! The Shop's MANA/USD rate: the answer of the feed the settlement chain's
//! newest marketplace converts USD-pegged prices with, as upstream's
//! `readManaUsdRate` reads it (shop@3a322cf9 `app/src/lib/mana-rate.ts`), so a
//! MANA price the Shop shows is the one the contract charges. The market spot
//! (CoinGecko) can sit far from that feed (on Amoy the feed is a test answer),
//! and it stays the economy routes' price only.
//!
//! The read is `credits::feed::ChainFeed`'s, the one a credits authorization
//! sizes with: the marketplace's own `manaUsdAggregator()` and
//! `manaUsdAggregatorTolerance()`, the aggregator's `decimals()` and
//! `latestRoundData()`, all at one block hash, the endpoint's chain checked,
//! the round refused when the marketplace's check would revert, when it runs
//! ahead of the clock, or past `CREDITS_USD_PEGGED_FEED_MAX_AGE_SECS`.
//!
//! No request waits on that read. [`install`] starts one refresher per
//! process that reads every [`REFRESH_EVERY`] into a slot; [`current`] answers
//! from the slot without blocking: the last good read while it is at most
//! [`SERVE_FOR`] old, its round still within the marketplace's own
//! `manaUsdAggregatorTolerance()` and within the local age bound, and
//! "unavailable" otherwise (never the spot, never an older rate). The
//! tolerance cut is by this node's clock, a conservative stand-in for the
//! next block's: a purchase is not offered on a round the contract has
//! already stopped accepting. A node with
//! no endpoint for the settlement chain, or no marketplace on it, wires no
//! feed and prices at the spot as before ([`current`] answers `None`).
//!
//! The rate can still move between a quote and the block that settles it:
//! the approval a USD-pegged quote asks for carries upstream's +2%
//! ([`USD_PEGGED_ALLOWANCE_BUFFER_PCT`]).

use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

use alloy_primitives::{Address, U256};
use chrono::{DateTime, Utc};

use crate::credits::feed::{FeedReading, MarketplaceFeed};
use crate::types::ManaUsdRate;

/// How often the refresher reads the feed.
pub const REFRESH_EVERY: Duration = Duration::from_secs(15);

/// How long one good read is served: four refreshes, so a single failed
/// refresh never makes the Shop's prices flicker to unavailable.
pub const SERVE_FOR: Duration = Duration::from_secs(60);

/// Upstream's approval buffer over a USD-pegged trade's MANA (shop@3a322cf9
/// `app/src/lib/buy.ts` `tradeManaPriceWei`: `mul(102).div(100)`). It is an
/// approval margin for the rate moving before the block that settles the
/// trade, not a slippage bound: the broker's `USD_PEGGED_SLIPPAGE_BPS` (1%)
/// is what a broker purchase checks the settled MANA against.
pub const USD_PEGGED_ALLOWANCE_BUFFER_PCT: u64 = 2;

/// The rate as the Shop prices with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShopRate {
    pub rate: ManaUsdRate,
    /// When this node read it.
    pub observed_at: DateTime<Utc>,
}

impl ShopRate {
    /// USD per MANA as exact decimal text ("0.26960836").
    pub fn usd_per_mana(&self) -> String {
        self.rate.decimal_text()
    }
}

/// Why there is no rate to price with. The detail is logged, never sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unavailable;

impl std::fmt::Display for Unavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the marketplace's MANA/USD feed has no current answer")
    }
}

#[derive(Debug, Clone, Copy)]
struct Good {
    reading: FeedReading,
    observed_at: DateTime<Utc>,
}

/// The slot the refresher writes and every reader reads.
pub struct RateSlot {
    good: RwLock<Option<Good>>,
    max_age_secs: u64,
}

impl RateSlot {
    pub fn new(max_age_secs: u64) -> Self {
        Self {
            good: RwLock::new(None),
            max_age_secs,
        }
    }

    /// Records one refresh. A failure keeps the last good read: [`Self::at`]
    /// stops serving it once it is [`SERVE_FOR`] old.
    pub fn record(&self, read: Result<FeedReading, String>, now: DateTime<Utc>) {
        match read {
            Ok(reading) => {
                *self.good.write().unwrap() = Some(Good {
                    reading,
                    observed_at: now,
                })
            }
            Err(reason) => {
                tracing::warn!(%reason, "the marketplace's MANA/USD feed could not be read")
            }
        }
    }

    /// The rate to price with at `now`: refused once the read is
    /// [`SERVE_FOR`] old, once its round is past the marketplace's tolerance
    /// (the contract would revert), or past the local age bound.
    pub fn at(&self, now: DateTime<Utc>) -> Result<ShopRate, Unavailable> {
        let good = (*self.good.read().unwrap()).ok_or(Unavailable)?;
        let observed_for = (now - good.observed_at).num_seconds();
        let round_age = now.timestamp() - good.reading.updated_at_s;
        let served_for = i64::try_from(SERVE_FOR.as_secs()).unwrap_or(i64::MAX);
        let max_age = i64::try_from(self.max_age_secs).unwrap_or(i64::MAX);
        let tolerance = i64::try_from(good.reading.tolerance_secs).unwrap_or(i64::MAX);
        if observed_for > served_for || round_age > max_age || round_age > tolerance {
            return Err(Unavailable);
        }
        Ok(ShopRate {
            rate: good.reading.rate,
            observed_at: good.observed_at,
        })
    }
}

static SLOT: OnceLock<Arc<RateSlot>> = OnceLock::new();

/// Starts the refresher of the settlement chain's newest marketplace and
/// wires its slot for this process; a second call is ignored.
pub fn install(
    feed: Arc<dyn MarketplaceFeed>,
    chain_id: u64,
    marketplace: Address,
    max_age_secs: u64,
) {
    let slot = Arc::new(RateSlot::new(max_age_secs));
    if SLOT.set(slot.clone()).is_err() {
        return;
    }
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(REFRESH_EVERY);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            slot.record(feed.read(chain_id, marketplace).await, Utc::now());
        }
    });
}

#[cfg(test)]
thread_local! {
    static TEST_SLOT: std::cell::RefCell<Option<Arc<RateSlot>>> = const { std::cell::RefCell::new(None) };
}

/// Wires `slot` for the current test's thread (a `#[tokio::test]` runs its
/// tasks there), leaving the process's slot and other tests alone.
#[cfg(test)]
pub(crate) fn with_test_slot(slot: Option<Arc<RateSlot>>) {
    TEST_SLOT.with(|s| *s.borrow_mut() = slot);
}

#[cfg(test)]
fn slot() -> Option<Arc<RateSlot>> {
    TEST_SLOT.with(|s| s.borrow().clone())
}

#[cfg(not(test))]
fn slot() -> Option<Arc<RateSlot>> {
    SLOT.get().cloned()
}

/// The Shop's rate now: `None` where no feed is wired (price at the spot),
/// else the last good read or [`Unavailable`]. Never blocks on the network.
pub fn current() -> Option<Result<ShopRate, Unavailable>> {
    slot().map(|slot| slot.at(Utc::now()))
}

/// Whether this process prices at the marketplace's feed.
pub fn installed() -> bool {
    slot().is_some()
}

pub use crate::types::decimal_text;

/// The approval a USD-pegged trade's `mana` asks for: upstream's +2%.
pub fn usd_pegged_allowance(mana: U256) -> U256 {
    mana.checked_mul(U256::from(100 + USD_PEGGED_ALLOWANCE_BUFFER_PCT))
        .map(|scaled| scaled / U256::from(100))
        .unwrap_or(mana)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use alloy_primitives::B256;

    pub(crate) fn reading(usd_per_mana: &str, updated_at_s: i64) -> FeedReading {
        with_tolerance(usd_per_mana, updated_at_s, 27)
    }

    fn with_tolerance(usd_per_mana: &str, updated_at_s: i64, tolerance_secs: u64) -> FeedReading {
        FeedReading {
            rate: ManaUsdRate::parse_usd_per_mana(usd_per_mana).unwrap(),
            aggregator: Address::repeat_byte(0xaa),
            tolerance_secs,
            updated_at_s,
            block: 1,
            block_hash: B256::ZERO,
        }
    }

    fn at(s: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(s, 0).unwrap()
    }

    #[test]
    fn nothing_read_yet_is_unavailable() {
        assert_eq!(RateSlot::new(3_600).at(at(1_800_000_000)), Err(Unavailable));
    }

    #[test]
    fn a_good_read_serves_for_a_minute_through_failed_refreshes() {
        let slot = RateSlot::new(3_600);
        let t0 = 1_800_000_000;
        slot.record(Ok(with_tolerance("0.26960836", t0 - 10, 3_600)), at(t0));
        let served = slot.at(at(t0 + 59)).unwrap();
        assert_eq!(served.usd_per_mana(), "0.26960836");
        assert_eq!(
            served.observed_at,
            at(t0),
            "the time of the read, not of serving it"
        );
        slot.record(Err("rpc down".into()), at(t0 + 15));
        slot.record(Err("rpc down".into()), at(t0 + 30));
        assert!(
            slot.at(at(t0 + 60)).is_ok(),
            "failed refreshes keep the last good read"
        );
        assert_eq!(
            slot.at(at(t0 + 61)),
            Err(Unavailable),
            "past a minute it is not served"
        );
        slot.record(Ok(with_tolerance("0.3", t0 + 70, 3_600)), at(t0 + 75));
        assert_eq!(
            slot.at(at(t0 + 76)).unwrap().usd_per_mana(),
            "0.3",
            "a good refresh brings it back"
        );
    }

    #[test]
    fn a_round_past_the_feed_bound_is_not_served() {
        let slot = RateSlot::new(3_600);
        let t0 = 1_800_000_000;
        slot.record(Ok(with_tolerance("0.26960836", t0 - 3_590, 7_200)), at(t0));
        assert!(slot.at(at(t0 + 10)).is_ok());
        assert_eq!(slot.at(at(t0 + 11)), Err(Unavailable));
    }

    #[test]
    fn a_round_past_the_marketplaces_tolerance_is_not_served() {
        // Amoy's marketplace accepts a round at most 27 s old.
        let slot = RateSlot::new(3_600);
        let t0 = 1_800_000_000;
        slot.record(Ok(reading("0.26960836", t0 - 26)), at(t0));
        assert!(
            slot.at(at(t0 + 1)).is_ok(),
            "26 + 1 = 27 s: exactly the tolerance"
        );
        assert_eq!(
            slot.at(at(t0 + 2)),
            Err(Unavailable),
            "28 s: the contract would revert"
        );
        slot.record(Err("the marketplace would revert".into()), at(t0 + 15));
        assert_eq!(
            slot.at(at(t0 + 16)),
            Err(Unavailable),
            "a failed refresh does not revive it"
        );
        assert_eq!(slot.at(at(t0 + 60)), Err(Unavailable));
        slot.record(Ok(reading("0.27", t0 + 30)), at(t0 + 31));
        assert_eq!(
            slot.at(at(t0 + 32)).unwrap().usd_per_mana(),
            "0.27",
            "a fresh round serves again"
        );
    }

    #[test]
    fn the_rate_text_is_exact() {
        assert_eq!(decimal_text(U256::from(26_960_836u64), 8), "0.26960836");
        assert_eq!(decimal_text(U256::from(100_000_000u64), 8), "1");
        assert_eq!(decimal_text(U256::from(150_000_000u64), 8), "1.5");
        assert_eq!(decimal_text(U256::from(5u64), 8), "0.00000005");
        assert_eq!(decimal_text(U256::from(42u64), 0), "42");
        let feed = ShopRate {
            rate: ManaUsdRate::new(U256::from(269_608_360_000_000_000u128), 18).unwrap(),
            observed_at: at(0),
        };
        assert_eq!(
            feed.usd_per_mana(),
            "0.26960836",
            "the feed's 18-decimal rate"
        );
    }

    #[test]
    fn a_usd_pegged_approval_carries_two_percent() {
        assert_eq!(
            usd_pegged_allowance(U256::from(5_192_717_317_816_109_517u128)),
            U256::from(5_296_571_664_172_431_707u128)
        );
        assert_eq!(usd_pegged_allowance(U256::from(100u64)), U256::from(102u64));
    }

    fn lazy_pool() -> sqlx::PgPool {
        sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://nobody@127.0.0.1:1/nowhere")
            .unwrap()
    }

    pub(crate) fn wired(read: Option<FeedReading>) -> Arc<RateSlot> {
        let slot = Arc::new(RateSlot::new(3_600));
        if let Some(read) = read {
            slot.record(Ok(read), Utc::now());
        }
        with_test_slot(Some(slot.clone()));
        slot
    }

    // Each reader answers from the slot alone (the pool below cannot connect), and
    // with no current answer it is "unavailable", never the spot.
    #[tokio::test]
    async fn every_shop_reader_prices_at_the_slot_and_never_falls_back() {
        let now = Utc::now().timestamp();
        wired(Some(reading("0.26960836", now - 5)));
        let rate = crate::mana::quote::mana_usd_rate(&lazy_pool())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(decimal_text(rate.rate(), rate.decimals()), "0.26960836");
        let (rate, observed) = crate::import::current_rate(&lazy_pool())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(decimal_text(rate.rate(), rate.decimals()), "0.26960836");
        assert!((now - observed).abs() <= 2, "the time of the read");
        let local = crate::credits::pricing::LocalMarket {
            pool: lazy_pool(),
            market: None,
            price: None,
            max_staleness_secs: 60,
        };
        use crate::credits::pricing::Market as _;
        assert_eq!(
            local.mana_usd().await.unwrap(),
            "0.26960836",
            "with no PRICE_POLL either"
        );
        let market = crate::market::MarketplaceState::new(
            lazy_pool(),
            crate::http::Outbound::for_tests(),
            crate::market::Settings::for_tests(),
        );
        assert_eq!(
            crate::market::items::mana_usd_rate(&market)
                .await
                .unwrap()
                .decimal_text(),
            "0.26960836"
        );

        wired(None);
        assert_eq!(
            crate::mana::quote::mana_usd_rate(&lazy_pool())
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            crate::import::current_rate(&lazy_pool()).await.unwrap(),
            None
        );
        let refused = local.mana_usd().await.unwrap_err().to_string();
        assert!(
            refused.contains("no current answer") && !refused.contains("http"),
            "{refused}"
        );
        assert!(crate::market::items::mana_usd_rate(&market).await.is_err());
        with_test_slot(None);
    }
}
