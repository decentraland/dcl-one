//! The reopen schedule of a failed media source: up to [`MAX_REOPENS`] reopens, the first after
//! the first delay (2 s) and each later one after twice the one before (4, 8, 16 and 32 s). The
//! count restarts after [`HEALTHY_PLAYBACK`] of playback since the last failure, so a source that
//! fails again at the same point gives up. A pause keeps the count.

use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

/// Reopens after which a still failing source gives up.
pub const MAX_REOPENS: u32 = 5;
/// The wait before the first reopen, unless [`set_first_reopen_delay`] changed it.
pub const FIRST_REOPEN_DELAY: Duration = Duration::from_secs(2);
/// Playback since the last failure after which the count restarts.
pub const HEALTHY_PLAYBACK: Duration = Duration::from_secs(5);

static FIRST_REOPEN_DELAY_MS: AtomicU64 = AtomicU64::new(FIRST_REOPEN_DELAY.as_millis() as u64);

/// The wait before the first reopen of every source failing from now on.
pub fn first_reopen_delay() -> Duration {
    Duration::from_millis(FIRST_REOPEN_DELAY_MS.load(Ordering::Relaxed))
}

/// Sets [`first_reopen_delay`]; later waits double from it.
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
    /// Reopen the source after `delay`.
    Reopen { delay: Duration },
    /// The last reopen failed too: the source stays failed.
    GiveUp,
}

/// One source's failures since it last played for [`HEALTHY_PLAYBACK`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MediaRetry {
    attempts: u32,
    /// Playback since the last failure.
    played: Duration,
}

impl MediaRetry {
    /// Counts a failure of the source: a reopen after its delay, or, past [`MAX_REOPENS`], giving
    /// up.
    pub fn fail(&mut self, first_delay: Duration) -> RetryStep {
        if self.attempts >= MAX_REOPENS {
            return RetryStep::GiveUp;
        }
        self.attempts += 1;
        self.played = Duration::ZERO;
        RetryStep::Reopen {
            delay: reopen_delay(first_delay, self.attempts),
        }
    }

    /// The source played for `step`; [`HEALTHY_PLAYBACK`] of it since the last failure restarts
    /// the count.
    pub fn played(&mut self, step: Duration) {
        if self.attempts == 0 {
            return;
        }
        self.played += step;
        if self.played >= HEALTHY_PLAYBACK {
            self.attempts = 0;
            self.played = Duration::ZERO;
        }
    }

    pub fn attempts(&self) -> u32 {
        self.attempts
    }
}

/// A source's reopen state.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct MediaRetryStatus {
    /// Reopens since the source last played for [`HEALTHY_PLAYBACK`].
    pub attempt: u32,
    /// Failures counted since the source was created.
    pub failures: u32,
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

    fn reopen(secs: u64) -> RetryStep {
        RetryStep::Reopen {
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
                reopen(2),
                reopen(4),
                reopen(8),
                reopen(16),
                reopen(32),
                RetryStep::GiveUp
            ]
        );
        assert_eq!(retry.fail(FIRST), RetryStep::GiveUp);
    }

    #[test]
    fn healthy_playback_restarts_the_schedule() {
        let mut retry = MediaRetry::default();
        retry.fail(FIRST);
        retry.fail(FIRST);
        retry.played(HEALTHY_PLAYBACK - Duration::from_millis(250));
        assert_eq!(retry.attempts(), 2);
        retry.played(Duration::from_millis(250));
        assert_eq!(retry.attempts(), 0);
        assert_eq!(retry.fail(FIRST), reopen(2));
        assert_eq!(retry.attempts(), 1);
    }

    #[test]
    fn failing_again_before_healthy_playback_keeps_counting() {
        let mut retry = MediaRetry::default();
        for _ in 0..MAX_REOPENS {
            retry.fail(FIRST);
            retry.played(HEALTHY_PLAYBACK / 2);
        }
        assert_eq!(retry.fail(FIRST), RetryStep::GiveUp);
    }

    #[test]
    fn every_failure_after_the_last_reopen_gives_up() {
        let mut retry = MediaRetry::default();
        for _ in 0..=MAX_REOPENS {
            retry.fail(FIRST);
        }
        assert_eq!(retry.fail(FIRST), RetryStep::GiveUp);
        assert_eq!(retry.attempts(), MAX_REOPENS);
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
