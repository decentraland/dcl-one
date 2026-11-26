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

use dashmap::DashMap;
use parking_lot::Mutex;
use std::collections::{BTreeMap, HashMap};
use std::net::{IpAddr, Ipv6Addr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub const DEFAULT_MAX_BUCKETS: usize = 10_000;

const SWEEP_EVERY: u64 = 512;
const MIN_SWEEP_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitDecision {
    Allow,
    Deny,
}

struct Bucket {
    tokens: f64,
    last: Instant,
}

/// Token bucket per key. Sweeps reclaim fully refilled buckets. When all
/// slots are occupied, new keys are refused while tracked keys keep their
/// balances. Sweeps are spaced out to bound the cost of a new-key flood.
pub struct RateLimiter {
    capacity: f64,
    refill_per_sec: f64,
    max_buckets: usize,
    checks: AtomicU64,
    // Serializes new keys and remembers the last sweep. Tracked keys spend
    // from their own bucket locks.
    admission: Mutex<Option<Instant>>,
    buckets: DashMap<String, Mutex<Bucket>>,
}

impl RateLimiter {
    pub fn new(capacity: u32, per: Duration) -> Self {
        Self {
            capacity: capacity as f64,
            refill_per_sec: capacity as f64 / per.as_secs_f64(),
            max_buckets: DEFAULT_MAX_BUCKETS,
            checks: AtomicU64::new(0),
            admission: Mutex::new(None),
            buckets: DashMap::new(),
        }
    }

    pub fn with_max_buckets(mut self, max_buckets: usize) -> Self {
        self.max_buckets = max_buckets.max(1);
        self
    }

    pub fn len(&self) -> usize {
        self.buckets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buckets.is_empty()
    }

    fn maybe_sweep(&self) {
        let n = self.checks.fetch_add(1, Ordering::Relaxed);
        if n.is_multiple_of(SWEEP_EVERY)
            && let Some(mut last_sweep) = self.admission.try_lock()
        {
            self.sweep_if_due(&mut last_sweep);
        }
    }

    fn sweep_if_due(&self, last_sweep: &mut Option<Instant>) {
        let now = Instant::now();
        if last_sweep.is_none_or(|last| now.duration_since(last) >= MIN_SWEEP_INTERVAL) {
            self.sweep();
            *last_sweep = Some(Instant::now());
        }
    }

    fn sweep(&self) {
        let now = Instant::now();
        let capacity = self.capacity;
        let refill_per_sec = self.refill_per_sec;
        self.buckets.retain(|_, bucket| {
            let b = bucket.get_mut();
            let elapsed = now.saturating_duration_since(b.last).as_secs_f64();
            b.tokens + elapsed * refill_per_sec < capacity
        });
    }

    pub fn check(&self, key: &str) -> RateLimitDecision {
        self.check_cost(key, 1)
    }

    /// Spends `cost` tokens at once (e.g. bytes in KiB); a cost above the
    /// capacity is always refused and a zero cost spends nothing.
    pub fn check_cost(&self, key: &str, cost: u32) -> RateLimitDecision {
        if cost == 0 {
            return RateLimitDecision::Allow;
        }
        let cost = f64::from(cost);
        if cost > self.capacity {
            return RateLimitDecision::Deny;
        }
        self.maybe_sweep();
        let key = key.to_ascii_lowercase();
        if let Some(entry) = self.buckets.get(&key) {
            return self.spend(&mut entry.lock(), cost);
        }

        let mut last_sweep = self.admission.lock();
        // Another request may have inserted this key while we waited.
        if let Some(entry) = self.buckets.get(&key) {
            return self.spend(&mut entry.lock(), cost);
        }
        if self.buckets.len() >= self.max_buckets {
            self.sweep_if_due(&mut last_sweep);
            if self.buckets.len() >= self.max_buckets {
                return RateLimitDecision::Deny;
            }
        }
        self.buckets.insert(
            key,
            Mutex::new(Bucket {
                tokens: self.capacity - cost,
                last: Instant::now(),
            }),
        );
        RateLimitDecision::Allow
    }

    fn spend(&self, b: &mut Bucket, cost: f64) -> RateLimitDecision {
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(b.last).as_secs_f64();
        b.tokens = (b.tokens + elapsed * self.refill_per_sec).min(self.capacity);
        b.last = now;
        if b.tokens >= cost {
            b.tokens -= cost;
            RateLimitDecision::Allow
        } else {
            RateLimitDecision::Deny
        }
    }

    /// Returns `cost` tokens spent on work that did not happen.
    pub fn refund(&self, key: &str, cost: u32) {
        if let Some(entry) = self.buckets.get(&key.to_ascii_lowercase()) {
            let mut b = entry.lock();
            b.tokens = (b.tokens + f64::from(cost)).min(self.capacity);
        }
    }

    /// Called once the caller proves legitimate, so earlier failures cannot
    /// lock out a user who then gets it right.
    pub fn clear(&self, key: &str) {
        self.buckets.remove(&key.to_ascii_lowercase());
    }

    /// Whether a key is out of tokens or has no slot, without spending a
    /// token or sweeping. Admission can change before the next check.
    pub fn is_rate_limited(&self, key: &str) -> bool {
        match self.buckets.get(&key.to_ascii_lowercase()) {
            Some(entry) => {
                let b = entry.lock();
                let elapsed = Instant::now()
                    .saturating_duration_since(b.last)
                    .as_secs_f64();
                (b.tokens + elapsed * self.refill_per_sec).min(self.capacity) < 1.0
            }
            None => self.buckets.len() >= self.max_buckets,
        }
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

    #[test]
    fn check_cost_spends_several_tokens_and_refuses_over_budget() {
        let limiter = RateLimiter::new(10, Duration::from_secs(60));
        assert_eq!(limiter.check_cost("a", 6), RateLimitDecision::Allow);
        assert_eq!(limiter.check_cost("a", 6), RateLimitDecision::Deny);
        assert_eq!(limiter.check_cost("a", 4), RateLimitDecision::Allow);
        assert_eq!(limiter.check_cost("b", 11), RateLimitDecision::Deny);
        assert_eq!(limiter.check_cost("a", 0), RateLimitDecision::Allow);
        limiter.refund("a", 6);
        assert_eq!(limiter.check_cost("a", 6), RateLimitDecision::Allow);
        assert_eq!(limiter.check_cost("a", 1), RateLimitDecision::Deny);
    }

    #[test]
    fn is_rate_limited_does_not_consume_and_clear_forgives() {
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
    fn distinct_keys_never_grow_past_the_cap() {
        let limiter = RateLimiter::new(3, Duration::from_secs(60)).with_max_buckets(64);
        for i in 0..10_000 {
            limiter.check(&format!("ip-{i}"));
        }
        assert!(
            limiter.len() <= 64,
            "attacker-keyed buckets grew to {}",
            limiter.len()
        );
    }

    #[test]
    fn a_full_limiter_keeps_the_budgets_of_tracked_wallets() {
        let limiter = RateLimiter::new(2, Duration::from_secs(60)).with_max_buckets(2);
        assert_eq!(limiter.check_cost("exhausted", 2), RateLimitDecision::Allow);
        assert_eq!(limiter.check("tracked"), RateLimitDecision::Allow);
        assert!(limiter.is_rate_limited("new"));

        *limiter.admission.lock() = None;
        for i in 0..SWEEP_EVERY * 2 {
            assert_eq!(
                limiter.check(&format!("fresh-{i}")),
                RateLimitDecision::Deny
            );
        }
        assert_eq!(limiter.len(), 2);
        assert_eq!(limiter.check("EXHAUSTED"), RateLimitDecision::Deny);
        assert_eq!(limiter.check("tracked"), RateLimitDecision::Allow);
        assert_eq!(limiter.check("tracked"), RateLimitDecision::Deny);

        limiter.refund("tracked", 2);
        *limiter.admission.lock() = None;
        assert_eq!(limiter.check("new"), RateLimitDecision::Allow);
        assert_eq!(limiter.check("exhausted"), RateLimitDecision::Deny);
        assert_eq!(limiter.len(), 2);
    }

    #[test]
    fn concurrent_new_keys_share_the_hard_capacity() {
        let limiter = RateLimiter::new(1, Duration::from_secs(60)).with_max_buckets(2);
        assert_eq!(limiter.check("existing"), RateLimitDecision::Allow);
        let start = std::sync::Barrier::new(8);
        let allowed = std::thread::scope(|scope| {
            let threads: Vec<_> = (0..8)
                .map(|i| {
                    let (start, limiter) = (&start, &limiter);
                    scope.spawn(move || {
                        start.wait();
                        limiter.check(&format!("new-{i}")) == RateLimitDecision::Allow
                    })
                })
                .collect();
            threads
                .into_iter()
                .map(|thread| usize::from(thread.join().unwrap()))
                .sum::<usize>()
        });
        assert_eq!(allowed, 1);
        assert_eq!(limiter.len(), 2);
        assert_eq!(limiter.check("existing"), RateLimitDecision::Deny);
    }

    #[test]
    fn a_refilled_bucket_is_swept_but_a_deficient_one_survives() {
        let limiter = RateLimiter::new(2, Duration::from_secs(60));
        assert_eq!(limiter.check("full"), RateLimitDecision::Allow);
        limiter.buckets.get("full").unwrap().lock().tokens = 2.0;
        assert_eq!(limiter.check("spent"), RateLimitDecision::Allow);
        assert_eq!(limiter.check("spent"), RateLimitDecision::Allow);
        limiter.sweep();
        assert!(limiter.buckets.get("full").is_none());
        assert!(limiter.buckets.get("spent").is_some());
    }

    #[test]
    fn a_swept_key_starts_from_a_full_bucket() {
        let limiter = RateLimiter::new(2, Duration::from_secs(60));
        assert_eq!(limiter.check("a"), RateLimitDecision::Allow);
        assert_eq!(limiter.check("a"), RateLimitDecision::Allow);
        assert_eq!(limiter.check("a"), RateLimitDecision::Deny);
        limiter.buckets.get("a").unwrap().lock().tokens = 2.0;
        limiter.sweep();
        assert!(limiter.is_empty());
        assert_eq!(limiter.check("a"), RateLimitDecision::Allow);
        assert_eq!(limiter.len(), 1);
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
