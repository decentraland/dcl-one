//! Per-sender token buckets, as upstream's `adapters/rate-limiter.ts`: a bucket starts full,
//! refills continuously at `refill_per_second` up to `max_tokens` (the burst), and each routed
//! packet takes one token. The periodic [`RateLimiter::sweep`] drops buckets idle long enough to
//! be full again: such a bucket is indistinguishable from a fresh one, so memory stays bounded
//! without changing behaviour.

use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limits {
    pub max_tokens: f64,
    pub refill_per_second: f64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_tokens: 10.0,
            refill_per_second: 5.0,
        }
    }
}

struct Bucket {
    tokens: f64,
    last_refill: Instant,
}

pub struct RateLimiter {
    limits: Limits,
    /// How long an empty bucket takes to fill.
    full_refill: Duration,
    buckets: HashMap<String, Bucket>,
}

impl RateLimiter {
    pub fn new(limits: Limits) -> Self {
        let full_refill = Duration::try_from_secs_f64(limits.max_tokens / limits.refill_per_second)
            .unwrap_or(Duration::MAX);
        Self {
            limits,
            full_refill,
            buckets: HashMap::new(),
        }
    }

    /// Takes one token from `key`'s bucket; false when the bucket has less than one.
    pub fn allow(&mut self, key: &str, now: Instant) -> bool {
        let Limits {
            max_tokens,
            refill_per_second,
        } = self.limits;
        let bucket = match self.buckets.get_mut(key) {
            Some(bucket) => {
                let elapsed = now.saturating_duration_since(bucket.last_refill);
                if !elapsed.is_zero() {
                    bucket.tokens =
                        (bucket.tokens + elapsed.as_secs_f64() * refill_per_second).min(max_tokens);
                    bucket.last_refill = now;
                }
                bucket
            }
            None => self.buckets.entry(key.to_owned()).or_insert(Bucket {
                tokens: max_tokens,
                last_refill: now,
            }),
        };
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Drops every bucket untouched for at least the full-refill time.
    pub fn sweep(&mut self, now: Instant) {
        let full_refill = self.full_refill;
        self.buckets
            .retain(|_, bucket| now.saturating_duration_since(bucket.last_refill) < full_refill);
    }

    pub fn len(&self) -> usize {
        self.buckets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buckets.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_sender_has_the_burst_then_is_limited() {
        let mut limiter = RateLimiter::new(Limits::default());
        let now = Instant::now();
        for _ in 0..10 {
            assert!(limiter.allow("0xa", now));
        }
        assert!(!limiter.allow("0xa", now));
        // another sender has its own bucket
        assert!(limiter.allow("0xb", now));
    }

    #[test]
    fn tokens_refill_at_the_configured_rate_up_to_the_burst() {
        let mut limiter = RateLimiter::new(Limits::default());
        let start = Instant::now();
        for _ in 0..10 {
            assert!(limiter.allow("0xa", start));
        }
        // 5 per second: 200 ms gives one token, 100 ms more gives half of one
        let later = start + Duration::from_millis(200);
        assert!(limiter.allow("0xa", later));
        assert!(!limiter.allow("0xa", later + Duration::from_millis(100)));
        // a long idle never holds more than the burst
        let idle = later + Duration::from_secs(3600);
        for _ in 0..10 {
            assert!(limiter.allow("0xa", idle));
        }
        assert!(!limiter.allow("0xa", idle));
    }

    #[test]
    fn the_sweep_drops_only_buckets_that_would_be_full_again() {
        let mut limiter = RateLimiter::new(Limits::default());
        let start = Instant::now();
        assert!(limiter.allow("0xidle", start));
        assert!(limiter.allow("0xbusy", start + Duration::from_secs(1)));
        // 10 tokens at 5 per second fill in 2 s
        limiter.sweep(start + Duration::from_millis(1999));
        assert_eq!(limiter.len(), 2);
        limiter.sweep(start + Duration::from_secs(2));
        assert_eq!(limiter.len(), 1);
        limiter.sweep(start + Duration::from_secs(3));
        assert!(limiter.is_empty());
    }

    #[test]
    fn a_clock_step_back_takes_no_tokens_away_nor_adds_any() {
        let mut limiter = RateLimiter::new(Limits {
            max_tokens: 1.0,
            refill_per_second: 1.0,
        });
        let now = Instant::now() + Duration::from_secs(10);
        assert!(limiter.allow("0xa", now));
        assert!(!limiter.allow("0xa", now - Duration::from_secs(5)));
    }
}
