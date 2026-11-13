//! The reopen schedule of a failed media source: up to [`MAX_REOPENS`] reopens, the first after
//! the first delay (2 s) and each later one after twice the one before (4, 8, 16 and 32 s). The
//! count restarts once the source opens again; a pause cancels a scheduled reopen.

use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

/// Reopens after which a still failing source gives up.
pub const MAX_REOPENS: u32 = 5;
/// The wait before the first reopen, unless [`set_first_reopen_delay`] changed it.
pub const FIRST_REOPEN_DELAY: Duration = Duration::from_secs(2);

static FIRST_REOPEN_DELAY_MS: AtomicU64 = AtomicU64::new(FIRST_REOPEN_DELAY.as_millis() as u64);

/// The wait before the first reopen of every source failing from now on.
pub fn first_reopen_delay() -> Duration {
    Duration::from_millis(FIRST_REOPEN_DELAY_MS.load(Ordering::Relaxed))
}

/// Sets [`first_reopen_delay`] (`/media_retry_delay`); later waits double from it.
pub fn set_first_reopen_delay(delay: Duration) {
    FIRST_REOPEN_DELAY_MS.store(delay.as_millis() as u64, Ordering::Relaxed);
}

/// The wait before reopen `attempt` (from 1).
pub fn reopen_delay(first: Duration, attempt: u32) -> Duration {
    first.saturating_mul(1 << attempt.clamp(1, MAX_REOPENS).saturating_sub(1))
}

/// What follows a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryStep {
    /// Reopen the source after `delay`; `attempt` counts from 1 to [`MAX_REOPENS`].
    Reopen { attempt: u32, delay: Duration },
    /// The last reopen failed too: the source stays failed.
    GiveUp,
}

/// One source's failures since it last opened.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MediaRetry {
    attempts: u32,
    gave_up: bool,
}

impl MediaRetry {
    /// Counts a failure of the source.
    pub fn fail(&mut self, first_delay: Duration) -> RetryStep {
        if self.gave_up || self.attempts >= MAX_REOPENS {
            self.gave_up = true;
            return RetryStep::GiveUp;
        }
        self.attempts += 1;
        RetryStep::Reopen {
            attempt: self.attempts,
            delay: reopen_delay(first_delay, self.attempts),
        }
    }

    /// The source opened: the next failure starts the schedule again. A source that gave up
    /// stays given up.
    pub fn opened(&mut self) {
        if !self.gave_up {
            self.attempts = 0;
        }
    }

    /// A scheduled reopen was cancelled (a pause): the next failure starts the schedule again.
    pub fn cancel(&mut self) {
        self.opened();
    }

    pub fn attempts(&self) -> u32 {
        self.attempts
    }

    pub fn gave_up(&self) -> bool {
        self.gave_up
    }
}

/// A source's reopen state, for the `/media_state` rows.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct MediaRetryStatus {
    /// Reopens since the source last opened.
    pub attempt: u32,
    /// The wait before the scheduled reopen, while one is scheduled.
    pub next_delay: Option<Duration>,
    pub gave_up: bool,
    /// The last failure's kind.
    pub last_error: Option<&'static str>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIRST: Duration = FIRST_REOPEN_DELAY;

    fn reopen(attempt: u32, secs: u64) -> RetryStep {
        RetryStep::Reopen {
            attempt,
            delay: Duration::from_secs(secs),
        }
    }

    #[test]
    fn five_reopens_two_to_thirty_two_seconds_then_give_up() {
        let mut retry = MediaRetry::default();
        let steps: Vec<_> = (0..6).map(|_| retry.fail(FIRST)).collect();
        assert_eq!(
            steps,
            [
                reopen(1, 2),
                reopen(2, 4),
                reopen(3, 8),
                reopen(4, 16),
                reopen(5, 32),
                RetryStep::GiveUp
            ]
        );
        assert!(retry.gave_up());
        assert_eq!(retry.fail(FIRST), RetryStep::GiveUp);
    }

    #[test]
    fn opening_restarts_the_schedule() {
        let mut retry = MediaRetry::default();
        retry.fail(FIRST);
        retry.fail(FIRST);
        retry.opened();
        assert_eq!(retry.attempts(), 0);
        assert_eq!(retry.fail(FIRST), reopen(1, 2));
    }

    #[test]
    fn cancelling_restarts_the_schedule() {
        let mut retry = MediaRetry::default();
        retry.fail(FIRST);
        retry.fail(FIRST);
        retry.fail(FIRST);
        retry.cancel();
        assert_eq!(retry.fail(FIRST), reopen(1, 2));
    }

    #[test]
    fn giving_up_is_final() {
        let mut retry = MediaRetry::default();
        for _ in 0..=MAX_REOPENS {
            retry.fail(FIRST);
        }
        retry.opened();
        retry.cancel();
        assert!(retry.gave_up());
        assert_eq!(retry.fail(FIRST), RetryStep::GiveUp);
    }

    #[test]
    fn delays_double_from_the_first() {
        let first = Duration::from_millis(50);
        let delays: Vec<_> = (1..=MAX_REOPENS)
            .map(|attempt| reopen_delay(first, attempt).as_millis())
            .collect();
        assert_eq!(delays, [50, 100, 200, 400, 800]);
    }
}
