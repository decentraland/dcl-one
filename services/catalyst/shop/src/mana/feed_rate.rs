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
//! the round refused when the marketplace's check would revert at that
//! block, when it runs ahead of the clock, or past
//! `CREDITS_USD_PEGGED_FEED_MAX_AGE_SECS`.
//!
//! Only the newest marketplace is read: every venue the Shop signs new
//! listings for converts at that one. A USD-pegged trade signed for an older
//! marketplace on the same chain is quoted at this rate too; on Polygon and
//! Amoy today every marketplace points at the same aggregator.
//!
//! No request waits on that read. [`install`] starts one refresher per
//! process ([`spawn_refresher`]) that keeps a slot current, and readers answer
//! from it without blocking, two ways:
//!
//! - [`current`], for showing a price (catalog, item, import, credits
//!   pricing): the last good read while it is at most [`SERVE_FOR`] old and
//!   its round within the local age bound. A round nearing the marketplace's
//!   tolerance does not take browsing down.
//! - [`current_for_purchase`], for the purchase quote: also refused once the
//!   round is within [`SETTLE_MARGIN_SECS`] of the marketplace's tolerance,
//!   measured on the chain's clock (the read block's timestamp plus the time
//!   since the read), so a node whose clock runs behind the chain does not
//!   offer a round the contract has already stopped accepting.
//!
//! Neither ever answers the spot or an older rate. A node with no endpoint
//! for the settlement chain, or no marketplace on it, wires no feed and
//! prices at the spot as before (both answer `None`).
//!
//! The refresher reads again [`REFRESH_EVERY`] while the round is far from
//! the tolerance, just before the round leaves it, and every
//! [`REFRESH_MIN`] once it has, until the feed publishes a newer round. Each
//! read runs in its own task: one that panics is logged as a failed read and
//! the refresher goes on.
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

pub use crate::types::decimal_text;

/// How often the refresher reads while the round is far from the tolerance.
pub const REFRESH_EVERY: Duration = Duration::from_secs(15);

/// How often it reads once the round is near or past the tolerance.
pub const REFRESH_MIN: Duration = Duration::from_secs(2);

/// How long one good read is shown: four refreshes, so a failed refresh
/// never makes the Shop's prices flicker to unavailable.
pub const SERVE_FOR: Duration = Duration::from_secs(60);

/// How long before the marketplace's tolerance ends, on the chain's clock, a
/// purchase stops being quoted at a round: the next block's time.
pub const SETTLE_MARGIN_SECS: i64 = 3;

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
    /// When the feed set this answer (the round's `updatedAt`).
    pub updated_at: DateTime<Utc>,
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

impl Good {
    /// The round's age on the chain's clock at `now`.
    fn chain_round_age(&self, now: DateTime<Utc>) -> i64 {
        let since_read = (now - self.observed_at).num_seconds().max(0);
        self.reading.block_time_s + since_read - self.reading.updated_at_s
    }

    /// Seconds left, on the chain's clock, before a purchase stops being quoted.
    fn settle_left(&self, now: DateTime<Utc>) -> i64 {
        let tolerance = i64::try_from(self.reading.tolerance_secs).unwrap_or(i64::MAX);
        tolerance
            .saturating_sub(SETTLE_MARGIN_SECS)
            .saturating_sub(self.chain_round_age(now))
    }

    fn shop_rate(&self) -> ShopRate {
        ShopRate {
            rate: self.reading.rate,
            observed_at: self.observed_at,
            updated_at: DateTime::<Utc>::from_timestamp(self.reading.updated_at_s, 0)
                .unwrap_or(self.observed_at),
        }
    }
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
    /// stops showing it once it is [`SERVE_FOR`] old.
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

    fn good(&self) -> Option<Good> {
        *self.good.read().unwrap()
    }

    /// The rate to show at `now`: refused once the read is [`SERVE_FOR`] old
    /// or its round past the local age bound.
    pub fn at(&self, now: DateTime<Utc>) -> Result<ShopRate, Unavailable> {
        let good = self.good().ok_or(Unavailable)?;
        let observed_for = (now - good.observed_at).num_seconds();
        let round_age = now.timestamp() - good.reading.updated_at_s;
        let served_for = i64::try_from(SERVE_FOR.as_secs()).unwrap_or(i64::MAX);
        let max_age = i64::try_from(self.max_age_secs).unwrap_or(i64::MAX);
        if observed_for > served_for || round_age > max_age {
            return Err(Unavailable);
        }
        Ok(good.shop_rate())
    }

    /// The rate to quote a purchase at `now`: [`Self::at`], and refused once
    /// the round is within [`SETTLE_MARGIN_SECS`] of the marketplace's
    /// tolerance on the chain's clock (a 0 tolerance is never quoted).
    pub fn for_purchase_at(&self, now: DateTime<Utc>) -> Result<ShopRate, Unavailable> {
        let rate = self.at(now)?;
        let good = self.good().ok_or(Unavailable)?;
        if good.settle_left(now) < 0 {
            return Err(Unavailable);
        }
        Ok(rate)
    }

    /// When the refresher should read next, after a refresh at `now`.
    pub fn next_read_in(&self, now: DateTime<Utc>) -> Duration {
        let Some(good) = self.good() else {
            return REFRESH_MIN;
        };
        let left = u64::try_from(good.settle_left(now)).unwrap_or(0);
        Duration::from_secs(left).clamp(REFRESH_MIN, REFRESH_EVERY)
    }
}

static SLOT: OnceLock<Arc<RateSlot>> = OnceLock::new();

/// Wires the slot of the settlement chain's newest marketplace for this
/// process and starts its refresher; a second call is ignored.
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
    spawn_refresher(slot, feed, chain_id, marketplace);
}

/// Keeps `slot` current from `feed`: each read in its own task, so a panic is
/// a failed read and the loop goes on.
pub fn spawn_refresher(
    slot: Arc<RateSlot>,
    feed: Arc<dyn MarketplaceFeed>,
    chain_id: u64,
    marketplace: Address,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let reader = feed.clone();
            let read = tokio::spawn(async move { reader.read(chain_id, marketplace).await })
                .await
                .unwrap_or_else(|e| Err(format!("the feed read ended abnormally: {e}")));
            let now = Utc::now();
            slot.record(read, now);
            tokio::time::sleep(slot.next_read_in(now)).await;
        }
    })
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

/// The rate to show a price at now: `None` where no feed is wired (price at
/// the spot), else the last good read or [`Unavailable`]. Never blocks.
pub fn current() -> Option<Result<ShopRate, Unavailable>> {
    slot().map(|slot| slot.at(Utc::now()))
}

/// The rate to quote a purchase at now ([`RateSlot::for_purchase_at`]).
pub fn current_for_purchase() -> Option<Result<ShopRate, Unavailable>> {
    slot().map(|slot| slot.for_purchase_at(Utc::now()))
}

/// Whether this process prices at the marketplace's feed.
pub fn installed() -> bool {
    slot().is_some()
}

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

    /// A read at `block_time_s` (the node's clock in step with the chain's).
    fn read_at(
        usd_per_mana: &str,
        updated_at_s: i64,
        tolerance_secs: u64,
        block_time_s: i64,
    ) -> FeedReading {
        FeedReading {
            rate: ManaUsdRate::parse_usd_per_mana(usd_per_mana).unwrap(),
            aggregator: Address::repeat_byte(0xaa),
            tolerance_secs,
            updated_at_s,
            block: 1,
            block_hash: B256::ZERO,
            block_time_s,
        }
    }

    /// A read now, with Amoy's 27 s tolerance.
    pub(crate) fn reading(usd_per_mana: &str, updated_at_s: i64) -> FeedReading {
        read_at(usd_per_mana, updated_at_s, 27, Utc::now().timestamp())
    }

    fn at(s: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(s, 0).unwrap()
    }

    const T0: i64 = 1_800_000_000;

    #[test]
    fn nothing_read_yet_is_unavailable() {
        let slot = RateSlot::new(3_600);
        assert_eq!(slot.at(at(T0)), Err(Unavailable));
        assert_eq!(slot.for_purchase_at(at(T0)), Err(Unavailable));
        assert_eq!(slot.next_read_in(at(T0)), REFRESH_MIN);
    }

    #[test]
    fn a_good_read_is_shown_for_a_minute_through_failed_refreshes() {
        let slot = RateSlot::new(3_600);
        slot.record(Ok(read_at("0.26960836", T0 - 10, 3_600, T0)), at(T0));
        let shown = slot.at(at(T0 + 59)).unwrap();
        assert_eq!(shown.usd_per_mana(), "0.26960836");
        assert_eq!(
            shown.observed_at,
            at(T0),
            "the time of the read, not of serving it"
        );
        assert_eq!(shown.updated_at, at(T0 - 10), "the time the feed set it");
        slot.record(Err("rpc down".into()), at(T0 + 15));
        slot.record(Err("rpc down".into()), at(T0 + 30));
        assert!(
            slot.at(at(T0 + 60)).is_ok(),
            "failed refreshes keep the last good read"
        );
        assert_eq!(
            slot.at(at(T0 + 61)),
            Err(Unavailable),
            "past a minute it is not shown"
        );
        slot.record(Ok(read_at("0.3", T0 + 70, 3_600, T0 + 75)), at(T0 + 75));
        assert_eq!(slot.at(at(T0 + 76)).unwrap().usd_per_mana(), "0.3");
    }

    #[test]
    fn a_round_past_the_local_bound_is_not_shown() {
        let slot = RateSlot::new(3_600);
        slot.record(Ok(read_at("0.26960836", T0 - 3_590, 7_200, T0)), at(T0));
        assert!(slot.at(at(T0 + 10)).is_ok());
        assert_eq!(slot.at(at(T0 + 11)), Err(Unavailable));
    }

    #[test]
    fn browsing_keeps_its_price_while_a_purchase_stops_at_the_tolerance() {
        // Amoy: 27 s tolerance, read 10 s into the round.
        let slot = RateSlot::new(3_600);
        slot.record(Ok(read_at("0.26960836", T0 - 10, 27, T0)), at(T0));
        assert!(
            slot.for_purchase_at(at(T0 + 14)).is_ok(),
            "24 s = 27 - 3: still quoted"
        );
        assert_eq!(
            slot.for_purchase_at(at(T0 + 15)),
            Err(Unavailable),
            "inside the margin"
        );
        assert!(slot.at(at(T0 + 15)).is_ok(), "the catalog still shows it");
        assert!(slot.at(at(T0 + 59)).is_ok());
        slot.record(Err("the marketplace would revert".into()), at(T0 + 20));
        assert_eq!(
            slot.for_purchase_at(at(T0 + 21)),
            Err(Unavailable),
            "a failed refresh does not revive it"
        );
        slot.record(Ok(read_at("0.27", T0 + 25, 27, T0 + 26)), at(T0 + 26));
        assert_eq!(
            slot.for_purchase_at(at(T0 + 27)).unwrap().usd_per_mana(),
            "0.27"
        );
    }

    #[test]
    fn the_purchase_cut_follows_the_chains_clock_not_this_nodes() {
        // This node's clock runs 10 s behind the chain: the round is older on chain than it looks here.
        let slot = RateSlot::new(3_600);
        slot.record(Ok(read_at("0.26960836", T0 - 10, 27, T0 + 10)), at(T0));
        assert!(slot.for_purchase_at(at(T0 + 4)).is_ok(), "chain age 24 s");
        assert_eq!(
            slot.for_purchase_at(at(T0 + 5)),
            Err(Unavailable),
            "chain age 25 s: inside the margin"
        );
    }

    #[test]
    fn a_zero_tolerance_is_never_quoted_for_a_purchase() {
        let slot = RateSlot::new(3_600);
        slot.record(Ok(read_at("0.26960836", T0, 0, T0)), at(T0));
        assert_eq!(slot.for_purchase_at(at(T0)), Err(Unavailable));
        assert!(slot.at(at(T0)).is_ok());
    }

    #[test]
    fn a_read_late_in_the_tolerance_is_refreshed_quickly() {
        let slot = RateSlot::new(3_600);
        slot.record(Ok(read_at("0.26960836", T0 - 1, 54, T0)), at(T0));
        assert_eq!(
            slot.next_read_in(at(T0)),
            REFRESH_EVERY,
            "50 s left: the usual cadence"
        );
        slot.record(Ok(read_at("0.26960836", T0 - 18, 27, T0)), at(T0));
        assert_eq!(
            slot.next_read_in(at(T0)),
            Duration::from_secs(6),
            "just before it leaves the tolerance"
        );
        slot.record(Ok(read_at("0.26960836", T0 - 23, 27, T0)), at(T0));
        assert_eq!(
            slot.next_read_in(at(T0)),
            REFRESH_MIN,
            "inside the margin: until a newer round"
        );
    }

    struct Scripted {
        calls: std::sync::atomic::AtomicUsize,
    }

    #[async_trait::async_trait]
    impl MarketplaceFeed for Scripted {
        async fn read(&self, _chain_id: u64, _marketplace: Address) -> Result<FeedReading, String> {
            let call = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if call == 0 {
                panic!("a read that panics");
            }
            Ok(reading("0.26960836", Utc::now().timestamp()))
        }
    }

    #[tokio::test(start_paused = true)]
    async fn the_refresher_outlives_a_read_that_panics() {
        let slot = Arc::new(RateSlot::new(3_600));
        let feed = Arc::new(Scripted {
            calls: std::sync::atomic::AtomicUsize::new(0),
        });
        let refresher = spawn_refresher(slot.clone(), feed.clone(), 80_002, Address::ZERO);
        tokio::time::sleep(REFRESH_MIN + Duration::from_millis(100)).await;
        assert!(
            feed.calls.load(std::sync::atomic::Ordering::SeqCst) >= 2,
            "it read again after the panic"
        );
        assert_eq!(slot.at(Utc::now()).unwrap().usd_per_mana(), "0.26960836");
        assert!(!refresher.is_finished());
        refresher.abort();
    }

    #[test]
    fn the_rate_text_is_exact() {
        assert_eq!(decimal_text(U256::from(26_960_836u64), 8), "0.26960836");
        assert_eq!(decimal_text(U256::from(100_000_000u64), 8), "1");
        assert_eq!(decimal_text(U256::from(150_000_000u64), 8), "1.5");
        assert_eq!(decimal_text(U256::from(5u64), 8), "0.00000005");
        assert_eq!(decimal_text(U256::from(42u64), 0), "42");
        let rate = ManaUsdRate::new(U256::from(269_608_360_000_000_000u128), 18).unwrap();
        assert_eq!(
            rate.decimal_text(),
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
        assert_eq!(rate.decimal_text(), "0.26960836");
        let (rate, taken_at) = crate::import::current_rate(&lazy_pool())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(rate.decimal_text(), "0.26960836");
        assert_eq!(taken_at, now - 5, "the time the feed set the answer");
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

        // Past the purchase cut: the quote refuses, the catalog and the rest still price.
        wired(Some(reading("0.26960836", now - 25)));
        assert_eq!(
            crate::mana::quote::mana_usd_rate(&lazy_pool())
                .await
                .unwrap(),
            None
        );
        assert!(crate::market::items::mana_usd_rate(&market).await.is_ok());
        assert!(local.mana_usd().await.is_ok());

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
