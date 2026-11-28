//! The per-key token buckets the social, comms and shop surfaces throttle with: the
//! friendship and block RPC mutations
//! (`social::rpc::limits::FriendshipMutationLimiter`) and the plaintext
//! direct-message sends (`social::communities::friends`, 60 per signer per
//! minute, the old `catalyrst-communities` budget), and Shop's federation
//! ingest and listing writes (`market::federation`, `market::listings`); and
//! [`KeyedLimiter`], the budget of work one client source may start before it
//! is authenticated ([`preauth_keys`]: Shop's `/forward-blockchain` and every
//! contract-wallet check, `eip1654_budget`). In-process state that follows the
//! process's lifetime, never the database.

use parking_lot::Mutex;
use std::collections::hash_map::RandomState;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::BuildHasher;
use std::net::{IpAddr, Ipv6Addr};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const DEFAULT_MAX_BUCKETS: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitDecision {
    Allow,
    Deny,
}

/// A failed spend. Oversized requests cannot succeed after waiting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitDenial {
    CostExceedsCapacity,
    RetryAfter(Duration),
}

/// Debt is measured in token-nanoseconds: one token costs a window's
/// nanoseconds, and each elapsed nanosecond refills `capacity` units.
#[derive(Debug, Clone, Copy)]
struct Bucket {
    debt: u128,
    last: Instant,
    generation: u64,
}

impl Bucket {
    fn remaining(self, now: Instant, capacity: u32) -> u128 {
        self.debt.saturating_sub(
            now.saturating_duration_since(self.last).as_nanos() * u128::from(capacity),
        )
    }

    fn full_at(self, capacity: u32) -> Option<Instant> {
        self.last.checked_add(refill_time(self.debt, capacity))
    }
}

fn refill_time(debt: u128, capacity: u32) -> Duration {
    let nanos = debt.div_ceil(u128::from(capacity));
    let secs = u64::try_from(nanos / 1_000_000_000).unwrap_or(u64::MAX);
    #[allow(
        clippy::cast_possible_truncation,
        reason = "the modulus keeps it below 10^9"
    )]
    let subsec = (nanos % 1_000_000_000) as u32;
    Duration::new(secs, subsec)
}

#[derive(Debug)]
enum BucketKey {
    Exact(Arc<str>),
    Overflow(usize),
}

#[derive(Debug)]
struct Buckets {
    exact: HashMap<Arc<str>, Bucket>,
    // One deadline per stored exact bucket. A new admission reclaims at
    // most one entry and never has to scan the table.
    deadlines: BTreeSet<(Instant, Arc<str>)>,
    overflow: Box<[Option<Bucket>]>,
    generation: u64,
}

impl Buckets {
    fn new(overflow: usize) -> Self {
        Self {
            exact: HashMap::new(),
            deadlines: BTreeSet::new(),
            overflow: vec![None; overflow].into_boxed_slice(),
            generation: 0,
        }
    }

    fn next_generation(&mut self) -> u64 {
        self.generation += 1;
        self.generation
    }

    fn get(&self, key: &BucketKey) -> Option<Bucket> {
        match key {
            BucketKey::Exact(key) => self.exact.get(key).copied(),
            BucketKey::Overflow(index) => self.overflow[*index],
        }
    }

    fn set(&mut self, key: &BucketKey, value: Option<Bucket>, capacity: u32) {
        match key {
            BucketKey::Exact(key) => {
                self.remove_exact(key, capacity);
                if let Some(bucket) = value {
                    if let Some(full_at) = bucket.full_at(capacity) {
                        self.deadlines.insert((full_at, key.clone()));
                    }
                    self.exact.insert(key.clone(), bucket);
                }
            }
            BucketKey::Overflow(index) => self.overflow[*index] = value,
        }
    }

    fn remove_exact(&mut self, key: &str, capacity: u32) {
        if let Some((key, bucket)) = self.exact.remove_entry(key)
            && let Some(full_at) = bucket.full_at(capacity)
        {
            self.deadlines.remove(&(full_at, key));
        }
    }

    fn expired(&self, now: Instant) -> Option<&Arc<str>> {
        self.deadlines
            .first()
            .filter(|(full_at, _)| *full_at <= now)
            .map(|(_, key)| key)
    }
}

/// A successful spend tied to the bucket and refill generation it charged.
/// Dropping the receipt keeps the charge. A caller may refund once when
/// the work is known to have failed.
#[must_use = "drop the receipt to keep the charge, or refund it once"]
pub struct RateLimitReceipt<'a> {
    limiter: &'a RateLimiter,
    spend: Option<(BucketKey, Bucket, u128)>,
}

impl std::fmt::Debug for RateLimitReceipt<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RateLimitReceipt").finish_non_exhaustive()
    }
}

impl RateLimitReceipt<'_> {
    pub fn refund(self) {
        self.refund_with_clock(Instant::now);
    }

    fn refund_with_clock(self, now: impl FnOnce() -> Instant) {
        let Some((key, charged, cost)) = self.spend else {
            return;
        };
        let mut buckets = self.limiter.buckets.lock();
        let now = now();
        let capacity = self.limiter.capacity;
        let Some(bucket) = buckets
            .get(&key)
            .filter(|b| b.generation == charged.generation)
        else {
            return;
        };
        // Elapsed refill may already have paid this charge. Credit only the
        // part that would still be outstanding if it were the only debit.
        let refundable = Bucket {
            debt: cost,
            ..charged
        }
        .remaining(now, capacity);
        let debt = bucket.remaining(now, capacity).saturating_sub(refundable);
        let updated = (debt != 0).then_some(Bucket {
            debt,
            last: now,
            ..bucket
        });
        buckets.set(&key, updated, capacity);
    }
}

/// Exact per-key token buckets with a hard cap that includes a fixed set of
/// overflow buckets. Unknown keys at capacity share a randomly keyed hash
/// bucket. Keep them there until it fully refills, even if an exact slot
/// opens, so promotion cannot erase earlier spending.
pub struct RateLimiter {
    capacity: u32,
    window_nanos: u128,
    max_buckets: usize,
    overflow_hash: RandomState,
    buckets: Mutex<Buckets>,
}

fn overflow_slots(max_buckets: usize) -> usize {
    (max_buckets / 8).clamp(1, 1_024)
}

impl RateLimiter {
    pub fn new(capacity: u32, per: Duration) -> Self {
        Self {
            capacity,
            window_nanos: per.as_nanos().max(1),
            max_buckets: DEFAULT_MAX_BUCKETS,
            overflow_hash: RandomState::new(),
            buckets: Mutex::new(Buckets::new(overflow_slots(DEFAULT_MAX_BUCKETS))),
        }
    }

    /// Set the cap before the limiter is used.
    pub fn with_max_buckets(mut self, max_buckets: usize) -> Self {
        assert_eq!(self.buckets.get_mut().generation, 0, "limiter already used");
        self.max_buckets = max_buckets.max(1);
        self.buckets = Mutex::new(Buckets::new(overflow_slots(self.max_buckets)));
        self
    }

    pub fn len(&self) -> usize {
        let buckets = self.buckets.lock();
        buckets.exact.len() + buckets.overflow.iter().flatten().count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn locate(&self, buckets: &Buckets, key: &str, now: Instant) -> BucketKey {
        if let Some((key, _)) = buckets.exact.get_key_value(key) {
            return BucketKey::Exact(key.clone());
        }
        let len = crate::casts::len_u64(buckets.overflow.len());
        // Below the overflow length, so it fits.
        let index = crate::casts::to_usize(self.overflow_hash.hash_one(key) % len);
        let has_debt = buckets.overflow[index].is_some_and(|b| b.remaining(now, self.capacity) > 0);
        let room = buckets.exact.len() < self.max_buckets - buckets.overflow.len();
        if !has_debt && (room || buckets.expired(now).is_some()) {
            BucketKey::Exact(Arc::from(key))
        } else {
            BucketKey::Overflow(index)
        }
    }

    pub fn check(&self, key: &str) -> RateLimitDecision {
        self.check_cost(key, 1)
    }

    /// Spends weighted tokens. An impossible or zero cost leaves no state.
    pub fn check_cost(&self, key: &str, cost: u32) -> RateLimitDecision {
        match self.reserve(key, cost) {
            Ok(_) => RateLimitDecision::Allow,
            Err(_) => RateLimitDecision::Deny,
        }
    }

    pub fn reserve(&self, key: &str, cost: u32) -> Result<RateLimitReceipt<'_>, RateLimitDenial> {
        self.reserve_with_clock(key, cost, Instant::now)
    }

    fn reserve_with_clock(
        &self,
        key: &str,
        cost: u32,
        now: impl FnOnce() -> Instant,
    ) -> Result<RateLimitReceipt<'_>, RateLimitDenial> {
        if cost == 0 {
            return Ok(RateLimitReceipt {
                limiter: self,
                spend: None,
            });
        }
        if cost > self.capacity {
            return Err(RateLimitDenial::CostExceedsCapacity);
        }
        let cost = u128::from(cost) * self.window_nanos;
        let key = key.to_ascii_lowercase();
        let mut buckets = self.buckets.lock();
        let now = now();
        let key = self.locate(&buckets, &key, now);
        let old = buckets.get(&key);
        let remaining = old.map_or(0, |b| b.remaining(now, self.capacity));
        let debt = remaining + cost;
        let max_debt = u128::from(self.capacity) * self.window_nanos;
        if debt > max_debt {
            return Err(RateLimitDenial::RetryAfter(refill_time(
                debt - max_debt,
                self.capacity,
            )));
        }
        if matches!(key, BucketKey::Exact(_))
            && old.is_none()
            && let Some(expired) = buckets.expired(now).cloned()
        {
            buckets.remove_exact(&expired, self.capacity);
        }
        let generation = match old {
            Some(bucket) if remaining != 0 => bucket.generation,
            _ => buckets.next_generation(),
        };
        let charged = Bucket {
            debt,
            last: now,
            generation,
        };
        buckets.set(&key, Some(charged), self.capacity);
        Ok(RateLimitReceipt {
            limiter: self,
            spend: Some((key, charged, cost)),
        })
    }

    /// Forgive only the exact key. A shared bucket also holds others' debt.
    pub fn clear(&self, key: &str) {
        self.buckets
            .lock()
            .remove_exact(&key.to_ascii_lowercase(), self.capacity);
    }

    /// Whether the next one-token attempt would be refused, without spending.
    pub fn is_rate_limited(&self, key: &str) -> bool {
        if self.capacity == 0 {
            return true;
        }
        let key = key.to_ascii_lowercase();
        let buckets = self.buckets.lock();
        let now = Instant::now();
        let bucket = self.locate(&buckets, &key, now);
        buckets.get(&bucket).is_some_and(|b| {
            b.remaining(now, self.capacity) + self.window_nanos
                > u128::from(self.capacity) * self.window_nanos
        })
    }

    pub fn record_failed_attempt(&self, key: &str) -> RateLimitDecision {
        self.check(key)
    }
}

/// Unauthenticated work one client source (IPv6 by /64) may start a minute.
pub const PREAUTH_PER_MINUTE: u32 = 120;
/// Unauthenticated work every /64 of one IPv6 /48 may start a minute together.
pub const PREAUTH_PER_MINUTE_PER_48: u32 = 480;

/// The pre-auth keys of a client address ([`crate::client_ip::source_key`],
/// and an IPv6 address's /48) and their budgets a minute.
pub fn preauth_keys(ip: &str) -> Vec<(String, u32)> {
    let mut keys = vec![(crate::client_ip::source_key(ip), PREAUTH_PER_MINUTE)];
    if let Ok(IpAddr::V6(v6)) = ip.parse::<IpAddr>()
        && v6.to_ipv4_mapped().is_none()
    {
        let net = Ipv6Addr::from(u128::from(v6) & (u128::MAX << 80));
        keys.push((format!("{net}/48"), PREAUTH_PER_MINUTE_PER_48));
    }
    keys
}

struct KeyedBucket {
    tokens: f64,
    last: Instant,
    /// Its place in the use order.
    used: u64,
}

/// Token buckets, at most `max_keys`, each call naming its keys and their
/// budgets a minute: a key is stored only when a call spends from it, and
/// past the cap the least recently used goes, so keys a denied flood invents
/// never displace the buckets it exhausted.
pub struct KeyedLimiter {
    max_keys: usize,
    inner: Mutex<KeyedBuckets>,
}

#[derive(Default)]
struct KeyedBuckets {
    clock: u64,
    map: HashMap<String, KeyedBucket>,
    order: BTreeMap<u64, String>,
}

impl KeyedLimiter {
    pub fn new(max_keys: usize) -> Self {
        Self {
            max_keys: max_keys.max(2),
            inner: Mutex::new(KeyedBuckets::default()),
        }
    }

    /// Spends one token of every `(key, budget a minute)`, or none.
    pub fn allow(&self, keys: &[(String, u32)]) -> bool {
        let now = Instant::now();
        let mut b = self.inner.lock();
        let KeyedBuckets { clock, map, order } = &mut *b;
        let refilled = |key: &str, per_minute: u32| {
            let cap = f64::from(per_minute);
            map.get(key).map_or(cap, |b| {
                let elapsed = now.saturating_duration_since(b.last).as_secs_f64();
                (b.tokens + elapsed * cap / 60.0).min(cap)
            })
        };
        let tokens: Vec<f64> = keys.iter().map(|(k, n)| refilled(k, *n)).collect();
        let ok = tokens.iter().all(|t| *t >= 1.0);
        for ((key, _), tokens) in keys.iter().zip(tokens) {
            if !ok && !map.contains_key(key) {
                continue;
            }
            *clock += 1;
            let bucket = map.entry(key.clone()).or_insert(KeyedBucket {
                tokens,
                last: now,
                used: 0,
            });
            order.remove(&bucket.used);
            bucket.used = *clock;
            order.insert(*clock, key.clone());
            if ok {
                bucket.tokens = tokens - 1.0;
                bucket.last = now;
            }
        }
        while map.len() > self.max_keys {
            let Some((_, oldest)) = order.pop_first() else {
                break;
            };
            map.remove(&oldest);
        }
        ok
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.inner.lock().map.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spend_at<'a>(
        limiter: &'a RateLimiter,
        key: &str,
        cost: u32,
        now: Instant,
    ) -> Result<RateLimitReceipt<'a>, RateLimitDenial> {
        limiter.reserve_with_clock(key, cost, || now)
    }

    #[test]
    fn check_cost_spends_several_tokens_and_refuses_over_budget() {
        let limiter = RateLimiter::new(10, Duration::from_secs(60));
        let now = Instant::now();
        let spend = spend_at(&limiter, "a", 6, now).unwrap();
        assert_eq!(
            spend_at(&limiter, "a", 6, now).unwrap_err(),
            RateLimitDenial::RetryAfter(Duration::from_secs(12))
        );
        drop(spend_at(&limiter, "a", 4, now).unwrap());
        assert_eq!(
            spend_at(&limiter, "b", 11, now).unwrap_err(),
            RateLimitDenial::CostExceedsCapacity
        );
        drop(spend_at(&limiter, "b", 0, now).unwrap());
        assert_eq!(limiter.len(), 1, "untouched keys allocate nothing");
        spend.refund_with_clock(|| now);
        drop(spend_at(&limiter, "a", 6, now).unwrap());
        assert_eq!(
            spend_at(&limiter, "a", 1, now).unwrap_err(),
            RateLimitDenial::RetryAfter(Duration::from_secs(6))
        );
    }

    #[test]
    fn is_rate_limited_does_not_consume_and_clear_forgives_an_exact_key() {
        let limiter = RateLimiter::new(2, Duration::from_secs(60));
        assert!(!limiter.is_rate_limited("0xabc"));
        assert_eq!(limiter.check("0xabc"), RateLimitDecision::Allow);
        for _ in 0..10 {
            assert!(!limiter.is_rate_limited("0xabc"));
        }
        assert_eq!(
            limiter.record_failed_attempt("0xABC"),
            RateLimitDecision::Allow
        );
        assert!(limiter.is_rate_limited("0xabc"));
        limiter.clear("0xAbc");
        assert!(!limiter.is_rate_limited("0xabc"));
    }

    #[test]
    fn concurrent_distinct_keys_stay_within_the_state_cap() {
        let limiter = RateLimiter::new(8, Duration::from_secs(24 * 3600)).with_max_buckets(16);
        let now = Instant::now();
        drop(spend_at(&limiter, "tracked", 8, now).unwrap());
        let accepted = std::sync::atomic::AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for thread in 0..8 {
                let limiter = &limiter;
                let accepted = &accepted;
                scope.spawn(move || {
                    for index in 0..2_000 {
                        if spend_at(limiter, &format!("{thread}-{index}"), 1, now).is_ok() {
                            accepted.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        }
                        assert!(limiter.len() <= 16);
                        assert!(spend_at(limiter, "TRACKED", 1, now).is_err());
                    }
                });
            }
        });
        assert_eq!(
            accepted.into_inner(),
            13 + 2 * 8,
            "fresh keys spent both overflow budgets"
        );
        let buckets = limiter.buckets.lock();
        assert_eq!(buckets.exact.len(), 14);
        assert_eq!(buckets.deadlines.len(), 14);
        assert_eq!(buckets.overflow.len(), 2);
    }

    #[test]
    fn opening_an_exact_slot_cannot_reset_overflow_debt() {
        let limiter = RateLimiter::new(2, Duration::from_secs(60)).with_max_buckets(2);
        let now = Instant::now();
        drop(spend_at(&limiter, "tracked", 1, now).unwrap());
        drop(spend_at(&limiter, "overflow", 2, now).unwrap());
        limiter.clear("overflow");
        let half = now + Duration::from_secs(30);
        assert_eq!(
            spend_at(&limiter, "overflow", 2, half).unwrap_err(),
            RateLimitDenial::RetryAfter(Duration::from_secs(30))
        );
        assert_eq!(
            spend_at(&limiter, "colliding", 2, half).unwrap_err(),
            RateLimitDenial::RetryAfter(Duration::from_secs(30))
        );
        let full = now + Duration::from_secs(60);
        drop(spend_at(&limiter, "overflow", 2, full).unwrap());
        let buckets = limiter.buckets.lock();
        assert!(buckets.exact.contains_key("overflow"));
        assert!(!buckets.exact.contains_key("tracked"));
        assert_eq!(buckets.deadlines.len(), 1);
    }

    #[test]
    fn old_receipts_do_not_refund_a_new_bucket_generation() {
        for cap in [1, 2] {
            let limiter = RateLimiter::new(1, Duration::from_secs(60)).with_max_buckets(cap);
            let now = Instant::now();
            let old = spend_at(&limiter, "original", 1, now).unwrap();
            let full = now + Duration::from_secs(60);
            drop(spend_at(&limiter, "original", 1, full).unwrap());
            old.refund_with_clock(|| full);
            assert!(spend_at(&limiter, "original", 1, full).is_err());
        }
    }

    #[test]
    fn a_delayed_refund_does_not_erase_newer_spending_on_a_shared_bucket() {
        let limiter = RateLimiter::new(2, Duration::from_secs(2)).with_max_buckets(1);
        let now = Instant::now();
        let old = spend_at(&limiter, "old", 1, now).unwrap();
        let later = now + Duration::from_millis(750);
        drop(spend_at(&limiter, "recent", 1, later).unwrap());
        old.refund_with_clock(|| later);
        drop(spend_at(&limiter, "other", 1, later).unwrap());
        assert_eq!(
            spend_at(&limiter, "recent", 1, later).unwrap_err(),
            RateLimitDenial::RetryAfter(Duration::from_secs(1))
        );
    }

    #[test]
    fn the_budget_refuses_the_first_call_past_capacity_within_the_window() {
        let limiter = RateLimiter::new(60, Duration::from_secs(60));
        for i in 0..60 {
            assert_eq!(
                limiter.check("0xSigner"),
                RateLimitDecision::Allow,
                "call {i}"
            );
        }
        assert_eq!(
            limiter.check("0xsigner"),
            RateLimitDecision::Deny,
            "the key folds case"
        );
        assert_eq!(
            limiter.check("0xother"),
            RateLimitDecision::Allow,
            "budgets are per key"
        );
    }

    /// Cycling /64s of one /48 hits the /48 budget; denied keys are never
    /// stored, so a flood of fresh ones cannot reset an exhausted bucket.
    #[test]
    fn a_flood_of_fresh_keys_cannot_reset_a_bucket() {
        let limiter = KeyedLimiter::new(8);
        let allowed = (0..1_000u32)
            .filter(|n| limiter.allow(&preauth_keys(&format!("2001:db8:1:{n:x}::1"))))
            .count();
        assert_eq!(allowed, PREAUTH_PER_MINUTE_PER_48 as usize);
        assert!(
            limiter.len() <= 8,
            "the least recently used goes past the cap"
        );
        for _ in 0..PREAUTH_PER_MINUTE {
            assert!(limiter.allow(&preauth_keys("203.0.113.5")));
        }
        assert!(!limiter.allow(&preauth_keys("203.0.113.5")));
        let before = limiter.len();
        for n in 1_000..3_000u32 {
            assert!(!limiter.allow(&preauth_keys(&format!("2001:db8:1:{n:x}::1"))));
        }
        assert_eq!(limiter.len(), before, "denied keys are not stored");
        assert!(
            !limiter.allow(&preauth_keys("203.0.113.5")),
            "still exhausted after a flood of fresh keys"
        );
    }
}
