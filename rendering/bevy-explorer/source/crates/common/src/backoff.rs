//! Capped doubling waits between retries.

use std::time::Duration;

/// Waits that start at `start` and double after each one, up to `cap`.
#[derive(Clone, Copy, Debug)]
pub struct Backoff {
    cap: Duration,
    next: Duration,
}

impl Backoff {
    pub const fn new(start: Duration, cap: Duration) -> Self {
        Self { cap, next: start }
    }

    /// The next wait; the one after is twice as long, at most `cap`.
    pub fn delay(&mut self) -> Duration {
        let delay = self.next;
        self.next = delay.saturating_mul(2).min(self.cap);
        delay
    }

    /// The wait after `failures` consecutive failures, without keeping state: `start`
    /// doubled `failures - 1` times, at most `cap`; nothing after none.
    pub fn after(start: Duration, cap: Duration, failures: u32) -> Duration {
        if failures == 0 {
            return Duration::ZERO;
        }
        start
            .saturating_mul(2u32.saturating_pow(failures - 1))
            .min(cap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delays_double_up_to_the_cap() {
        let mut backoff = Backoff::new(Duration::from_millis(25), Duration::from_millis(100));
        let delays: Vec<_> = (0..4).map(|_| backoff.delay()).collect();
        assert_eq!(delays, [25, 50, 100, 100].map(Duration::from_millis));
    }

    #[test]
    fn the_wait_after_failures_saturates_at_the_cap() {
        let after =
            |failures| Backoff::after(Duration::from_millis(250), Duration::from_secs(5), failures);
        assert_eq!(after(0), Duration::ZERO);
        assert_eq!(after(1), Duration::from_millis(250));
        assert_eq!(after(5), Duration::from_secs(4));
        assert_eq!(after(6), Duration::from_secs(5));
        assert_eq!(after(u32::MAX), Duration::from_secs(5));
    }
}
