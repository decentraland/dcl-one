//! The per-key token bucket the social, comms and shop surfaces throttle with: the
//! friendship and block RPC mutations
//! (`social::rpc::limits::FriendshipMutationLimiter`) and the plaintext
//! direct-message sends (`social::communities::friends`, 60 per signer per
//! minute, the old `catalyrst-communities` budget), and Shop's federation
//! ingest and listing writes (`market::federation`, `market::listings`). In-process state that
//! follows the process's lifetime, never the database.

use dashmap::DashMap;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub const DEFAULT_MAX_BUCKETS: usize = 10_000;

const SWEEP_EVERY: u64 = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitDecision {
    Allow,
    Deny,
}

struct Bucket {
    tokens: f64,
    last: Instant,
}

/// Token bucket per key. Keys are attacker-chosen, so the map is pruned: a
/// fully refilled bucket is dropped on a periodic sweep, and the map is
/// hard-capped so a burst of distinct keys cannot grow the process without
/// bound: at the cap the fullest buckets go first, so the keys that spent
/// most (a caller being refused, a bucket in debt) are the last forgiven.
pub struct RateLimiter {
    capacity: f64,
    refill_per_sec: f64,
    max_buckets: usize,
    checks: AtomicU64,
    buckets: DashMap<String, Mutex<Bucket>>,
}

impl RateLimiter {
    pub fn new(capacity: u32, per: Duration) -> Self {
        Self {
            capacity: capacity as f64,
            refill_per_sec: capacity as f64 / per.as_secs_f64(),
            max_buckets: DEFAULT_MAX_BUCKETS,
            checks: AtomicU64::new(0),
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
        if n.is_multiple_of(SWEEP_EVERY) || self.buckets.len() >= self.max_buckets {
            self.sweep();
        }
    }

    fn sweep(&self) {
        let now = Instant::now();
        let capacity = self.capacity;
        let refill_per_sec = self.refill_per_sec;
        let level = |b: &Bucket| {
            b.tokens + now.saturating_duration_since(b.last).as_secs_f64() * refill_per_sec
        };
        self.buckets
            .retain(|_, bucket| level(bucket.get_mut()) < capacity);
        if self.buckets.len() >= self.max_buckets {
            let mut levels: Vec<(f64, String)> = self
                .buckets
                .iter()
                .map(|entry| (level(&entry.value().lock()), entry.key().clone()))
                .collect();
            levels.sort_by(|a, b| a.0.total_cmp(&b.0));
            let keep = self.max_buckets - self.max_buckets.div_ceil(10);
            for (_, key) in levels.into_iter().skip(keep) {
                self.buckets.remove(&key);
            }
        }
    }

    pub fn check(&self, key: &str) -> RateLimitDecision {
        self.check_cost(key, 1)
    }

    /// Spends `cost` tokens at once (e.g. bytes in KiB); a cost above the
    /// capacity is always refused and a zero cost spends nothing.
    pub fn check_cost(&self, key: &str, cost: u32) -> RateLimitDecision {
        match self.take_at(key, cost, Instant::now()) {
            Ok(()) => RateLimitDecision::Allow,
            Err(_) => RateLimitDecision::Deny,
        }
    }

    /// Spends `cost` tokens at `now`; a refusal spends nothing and carries
    /// how long until the tokens are back (whole milliseconds, rounded up;
    /// `Duration::MAX` for a cost above the capacity).
    pub fn take_at(&self, key: &str, cost: u32, now: Instant) -> Result<(), Duration> {
        if cost == 0 {
            return Ok(());
        }
        let cost = f64::from(cost);
        if cost > self.capacity {
            return Err(Duration::MAX);
        }
        self.with_bucket(key, now, |b| {
            if b.tokens >= cost {
                b.tokens -= cost;
                Ok(())
            } else {
                let millis = ((cost - b.tokens) / self.refill_per_sec * 1000.0 - 1e-6).ceil();
                Err(Duration::from_millis(millis.max(1.0) as u64))
            }
        })
    }

    /// Spends `cost` tokens at `now` whatever the balance, for work whose
    /// cost is known only once done: the bucket may go into debt, which
    /// later [`Self::take_at`] calls wait out.
    pub fn charge_at(&self, key: &str, cost: u32, now: Instant) {
        if cost > 0 {
            self.with_bucket(key, now, |b| b.tokens -= f64::from(cost));
        }
    }

    fn with_bucket<T>(&self, key: &str, now: Instant, f: impl FnOnce(&mut Bucket) -> T) -> T {
        self.maybe_sweep();
        let entry = self
            .buckets
            .entry(key.to_ascii_lowercase())
            .or_insert_with(|| {
                Mutex::new(Bucket {
                    tokens: self.capacity,
                    last: now,
                })
            });
        let mut b = entry.lock();
        let elapsed = now.saturating_duration_since(b.last).as_secs_f64();
        b.tokens = (b.tokens + elapsed * self.refill_per_sec).min(self.capacity);
        b.last = b.last.max(now);
        f(&mut b)
    }

    /// The tokens `key` has spent and not yet got back at `now`: beyond the
    /// capacity while its bucket is in debt.
    pub fn spent_at(&self, key: &str, now: Instant) -> f64 {
        self.buckets
            .get(&key.to_ascii_lowercase())
            .map_or(0.0, |entry| {
                let b = entry.lock();
                let elapsed = now.saturating_duration_since(b.last).as_secs_f64();
                let level = (b.tokens + elapsed * self.refill_per_sec).min(self.capacity);
                (self.capacity - level).max(0.0)
            })
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

    /// Whether the next attempt would be refused, without spending a token.
    pub fn is_rate_limited(&self, key: &str) -> bool {
        match self.buckets.get(&key.to_ascii_lowercase()) {
            Some(entry) => {
                let b = entry.lock();
                let elapsed = Instant::now()
                    .saturating_duration_since(b.last)
                    .as_secs_f64();
                (b.tokens + elapsed * self.refill_per_sec).min(self.capacity) < 1.0
            }
            None => false,
        }
    }

    pub fn record_failed_attempt(&self, key: &str) -> RateLimitDecision {
        self.check(key)
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
    fn a_flood_of_new_keys_never_forgives_a_key_in_debt() {
        let limiter = RateLimiter::new(3, Duration::from_secs(60)).with_max_buckets(8);
        let now = Instant::now();
        limiter.charge_at("refused caller", 30, now);
        assert!(limiter.take_at("refused caller", 1, now).is_err());
        for i in 0..1_000 {
            let _ = limiter.take_at(&format!("flood-{i}"), 1, now);
        }
        assert!(limiter.len() <= 8);
        assert!(
            limiter.take_at("refused caller", 1, now).is_err(),
            "the flood reset the refused caller's budget"
        );
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
    fn a_refusal_says_when_the_next_token_is_back_and_a_charge_runs_into_debt() {
        let limiter = RateLimiter::new(60, Duration::from_secs(60));
        let t0 = Instant::now();
        assert_eq!(limiter.take_at("a", 60, t0), Ok(()));
        assert_eq!(limiter.take_at("a", 1, t0), Err(Duration::from_secs(1)));
        assert_eq!(
            limiter.take_at("a", 1, t0 + Duration::from_millis(400)),
            Err(Duration::from_millis(600))
        );
        assert_eq!(limiter.take_at("a", 1, t0 + Duration::from_secs(1)), Ok(()));
        limiter.charge_at("a", 30, t0 + Duration::from_secs(1));
        assert_eq!(
            limiter.take_at("a", 1, t0 + Duration::from_secs(1)),
            Err(Duration::from_secs(31))
        );
        assert_eq!(
            limiter.take_at("a", 1, t0 + Duration::from_secs(32)),
            Ok(())
        );
        assert_eq!(
            limiter.take_at("a", 61, t0 + Duration::from_secs(600)),
            Err(Duration::MAX),
            "a cost above the capacity never fits"
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
}
