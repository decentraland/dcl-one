//! When a final disconnect from the room connects again, as upstream's
//! `logic/connection-handlers/disconnected.ts` decides it, over LiveKit's `DisconnectReason`
//! numbers (livekit_models.proto) so the decision is testable without the SDK.

use std::time::Duration;

/// LiveKit's `DisconnectReason`, by its protocol number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    Unknown,
    ClientInitiated,
    DuplicateIdentity,
    ServerShutdown,
    ParticipantRemoved,
    RoomDeleted,
    StateMismatch,
    JoinFailure,
    Migration,
    SignalClose,
    RoomClosed,
    UserUnavailable,
    UserRejected,
    SipTrunkFailure,
    /// A reason newer than upstream's switch (connection timeout, media failure, agent error,
    /// or one this build does not know), which falls to its `default` branch.
    Other(i32),
}

impl Reason {
    pub fn from_code(code: i32) -> Self {
        match code {
            0 => Self::Unknown,
            1 => Self::ClientInitiated,
            2 => Self::DuplicateIdentity,
            3 => Self::ServerShutdown,
            4 => Self::ParticipantRemoved,
            5 => Self::RoomDeleted,
            6 => Self::StateMismatch,
            7 => Self::JoinFailure,
            8 => Self::Migration,
            9 => Self::SignalClose,
            10 => Self::RoomClosed,
            11 => Self::UserUnavailable,
            12 => Self::UserRejected,
            13 => Self::SipTrunkFailure,
            other => Self::Other(other),
        }
    }
}

/// Whether to connect again (with a fresh token) after `reason`. No for a disconnect this
/// process asked for, for another participant with this identity (a second replica holding the
/// same identity would otherwise take turns evicting this one), for a deleted room and for the
/// SIP reasons, which cannot happen here; yes for everything else. On a no the process exits
/// non-zero, so a supervisor that restarts on exit starts it again (after its own back-off).
pub fn reconnects(reason: Reason) -> bool {
    !matches!(
        reason,
        Reason::ClientInitiated
            | Reason::DuplicateIdentity
            | Reason::RoomDeleted
            | Reason::UserUnavailable
            | Reason::UserRejected
            | Reason::SipTrunkFailure
    )
}

/// Upstream's `retry`: up to `attempts` tries, `delay` apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Retry {
    pub attempts: u32,
    pub delay: Duration,
}

impl Default for Retry {
    fn default() -> Self {
        Self {
            attempts: 5,
            delay: Duration::from_secs(1),
        }
    }
}

impl Retry {
    /// Runs `attempt(n)` for n in 1..=attempts, sleeping `delay` between failures; the last
    /// error when every attempt failed.
    pub async fn run<T, E, F, Fut>(&self, mut attempt: F) -> Result<T, E>
    where
        F: FnMut(u32) -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
    {
        let mut n = 1;
        loop {
            match attempt(n).await {
                Ok(value) => return Ok(value),
                Err(e) if n >= self.attempts.max(1) => return Err(e),
                Err(_) => {
                    tokio::time::sleep(self.delay).await;
                    n += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnects_as_upstream_disconnected_handler() {
        let no = [1, 2, 5, 11, 12, 13];
        for code in 0..=20 {
            assert_eq!(
                reconnects(Reason::from_code(code)),
                !no.contains(&code),
                "code {code}: {:?}",
                Reason::from_code(code)
            );
        }
    }

    #[test]
    fn codes_map_to_livekit_names() {
        assert_eq!(Reason::from_code(2), Reason::DuplicateIdentity);
        assert_eq!(Reason::from_code(9), Reason::SignalClose);
        assert_eq!(Reason::from_code(14), Reason::Other(14));
    }

    #[tokio::test(start_paused = true)]
    async fn retry_stops_at_the_first_success_or_after_the_last_attempt() {
        let retry = Retry::default();
        let mut tried = Vec::new();
        let result: Result<u32, &str> = retry
            .run(|n| {
                tried.push(n);
                async move { if n == 3 { Ok(n) } else { Err("down") } }
            })
            .await;
        assert_eq!((result, tried.as_slice()), (Ok(3), &[1, 2, 3][..]));

        let started = tokio::time::Instant::now();
        let mut count = 0;
        let result: Result<(), &str> = retry
            .run(|_| {
                count += 1;
                async { Err("down") }
            })
            .await;
        assert_eq!((result, count), (Err("down"), 5));
        // four sleeps of one second between five attempts
        assert_eq!(started.elapsed(), Duration::from_secs(4));
    }
}
