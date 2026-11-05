//! The per-key token bucket the social and comms surfaces throttle with: the
//! friendship and block RPC mutations
//! (`social::rpc::limits::FriendshipMutationLimiter`) and the plaintext
//! direct-message sends (`social::communities::friends`, 60 per signer per
//! minute, the old `catalyrst-communities` budget). In-process state that
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
/// bound.
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
        self.buckets.retain(|_, bucket| {
            let b = bucket.get_mut();
            let elapsed = now.saturating_duration_since(b.last).as_secs_f64();
            b.tokens + elapsed * refill_per_sec < capacity
        });
        if self.buckets.len() >= self.max_buckets {
            self.buckets.clear();
        }
    }

    pub fn check(&self, key: &str) -> RateLimitDecision {
        self.maybe_sweep();
        let now = Instant::now();
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
        b.last = now;
        if b.tokens >= 1.0 {
            b.tokens -= 1.0;
            RateLimitDecision::Allow
        } else {
            RateLimitDecision::Deny
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
}
