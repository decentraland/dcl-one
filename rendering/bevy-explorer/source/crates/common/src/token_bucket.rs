//! A rate limit as a bucket of tokens.

/// A token bucket on the caller's clock: tokens come back at a steady rate up to a capacity, and
/// whatever it limits spends a whole one. Its times are `f64` seconds on a clock that only moves
/// on, and it knows nothing of Bevy, so a test drives it without an app.
#[derive(Clone, Copy, Debug)]
pub struct TokenBucket {
    refill_per_second: f64,
    capacity: f64,
    /// How far below full the bucket was at `refilled_at`, in tokens.
    spent: f64,
    /// The time `spent` is as of.
    refilled_at: f64,
}

impl TokenBucket {
    /// What a whole token may fall short by: the refill is a product of `f64` times, and a clock
    /// that adds up frame lengths reaches a tenth of a second a rounding error early.
    const ROUNDING: f64 = 1e-9;

    /// A full bucket that regains `refill_per_second` tokens a second and holds `capacity`.
    pub const fn new(refill_per_second: f64, capacity: f64) -> Self {
        Self {
            refill_per_second,
            capacity,
            spent: 0.0,
            refilled_at: 0.0,
        }
    }

    /// How far below full the bucket is at `now`.
    fn spent_at(&self, now: f64) -> f64 {
        let elapsed = (now - self.refilled_at).max(0.0);
        (self.spent - elapsed * self.refill_per_second).max(0.0)
    }

    /// Whether a whole token is held at `now`.
    pub fn has_token(&self, now: f64) -> bool {
        self.capacity - self.spent_at(now) >= 1.0 - Self::ROUNDING
    }

    /// Spend a token at `now`. Without a whole one it returns `false` and spends nothing; the part
    /// of a token that has come back is kept.
    pub fn try_spend(&mut self, now: f64) -> bool {
        if !self.has_token(now) {
            return false;
        }
        self.spent = (self.spent_at(now) + 1.0).min(self.capacity);
        self.refilled_at = now;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::TokenBucket;

    #[test]
    fn a_token_bucket_spends_whole_tokens_and_keeps_the_part_that_returned() {
        let mut bucket = TokenBucket::new(10.0, 2.0);
        assert!(bucket.try_spend(0.0));
        assert!(bucket.try_spend(0.0));
        // Empty: nothing is spent, and asking does not lose the part of a token that has come back.
        assert!(!bucket.has_token(0.05));
        assert!(!bucket.try_spend(0.05));
        assert!(!bucket.try_spend(0.09));
        assert!(bucket.has_token(0.1));
        assert!(bucket.try_spend(0.1));
        assert!(!bucket.try_spend(0.1));
    }

    #[test]
    fn a_token_bucket_holds_no_more_than_its_capacity() {
        let mut bucket = TokenBucket::new(10.0, 2.0);
        assert!(bucket.try_spend(0.0));
        // A long quiet spell fills it again, and no further.
        assert!(bucket.try_spend(100.0));
        assert!(bucket.try_spend(100.0));
        assert!(!bucket.try_spend(100.0));
    }

    #[test]
    fn a_token_bucket_takes_a_clock_that_adds_up_frame_lengths() {
        let mut bucket = TokenBucket::new(10.0, 1.0);
        assert!(bucket.try_spend(0.0));
        // Six 60 Hz frames add up to a rounding error under a tenth of a second.
        let now: f64 = (0..6).map(|_| 1.0 / 60.0).sum();
        assert!(now < 0.1);
        assert!(bucket.try_spend(now));
    }
}
