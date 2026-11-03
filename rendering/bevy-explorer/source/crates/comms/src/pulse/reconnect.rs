//! When Pulse connects again after a failure, and when it gives up.
//!
//! A pure policy: told what failed and when, it answers with the time to retry at, or that no
//! retry can help. It knows nothing of Bevy, the driver or the handshake, and its times are `f64`
//! seconds on the caller's clock, so a test drives it without an app. There is no jitter, as in the
//! reference client.
//!
//! The numbers are unity-explorer's (paths under `Explorer/Assets/DCL/`), which recovers two ways.
//! A connection that was up and dropped waits a fixed time chosen by the server's reason:
//! `ReconnectionSettings` in `Multiplayer/Movement/Systems/PulseMultiplayerBus.Disconnects.cs`, its
//! values set in `PluginSystem/Global/Plugin Settings.asset`. An attempt that never got its
//! handshake accepted backs off, doubling: `CONNECTION_RETRY_POLICY` in
//! `Multiplayer/Connections/Pulse/PulseMultiplayerService.cs`, capped by
//! `WebRequests/RetryPolicy.cs`. Two things are not Unity's, and each says so where it applies:
//! the refusal floors also hold before acceptance, and codes 17 and 18.

use super::transport::PulseDisconnect;

/// unity-explorer `ReconnectionSettings.DefaultRetryDelayMs` (3000): the wait after a connection
/// that had been accepted drops with no reason, a graceful close, an auth timeout or an auth
/// failure.
pub const DEFAULT_RETRY_DELAY: f64 = 3.0;
/// unity-explorer `ReconnectionSettings.AuthExhaustionRetryDelayMs` (10000): the least wait after
/// the per-IP or pre-auth cap refuses a connection (codes 7 and 8). Code 17 shares it, being in
/// their family on the server; unity-explorer does not know that code.
pub const AUTH_EXHAUSTION_RETRY_DELAY: f64 = 10.0;
/// unity-explorer `ReconnectionSettings.ServerFullRetryDelayMs` (60000): the least wait after the
/// server is full (code 6). Unity applies it only to a connection that was up; here it floors the
/// backoff before acceptance too.
pub const SERVER_FULL_RETRY_DELAY: f64 = 60.0;
/// The wait after the first failed attempt: unity-explorer `CONNECTION_RETRY_POLICY` in
/// `PulseMultiplayerService.cs` (1000 ms), doubling with each failed attempt.
pub const ATTEMPT_BACKOFF_START: f64 = 1.0;
/// unity-explorer `RetryPolicy.MAX_DELAY_BETWEEN_ATTEMPTS_MS` (60000): where the attempt backoff
/// stops.
pub const ATTEMPT_BACKOFF_CAP: f64 = 60.0;
/// The first wait after the server refuses a scene listener for its IP's cap (code 18). Not in
/// unity-explorer, whose disconnect reasons end at 16: the server's contract (decentraland/Pulse
/// `docs/hardening.md`, "Client recovery") asks for a long backoff from 5-10 s, doubling, capped in
/// the minutes, because that capacity frees only when another listener on the IP leaves.
pub const LISTENER_LIMIT_START: f64 = 10.0;
/// Where the wait after a scene-listener refusal stops: five minutes.
pub const LISTENER_LIMIT_CAP: f64 = 300.0;

/// What went wrong with the connection.
#[derive(Debug)]
pub enum Failure {
    /// Never established: DNS, socket, connect timeout, or the driver gone without a status.
    Unreached,
    /// The server's reason; `accepted` is whether this connection's handshake had been accepted.
    Disconnected {
        reason: PulseDisconnect,
        accepted: bool,
    },
    /// Rejected (`success = false`), unanswered within the response timeout, or not signable.
    HandshakeFailed,
}

/// What to do about a failure.
#[derive(Debug, PartialEq)]
pub enum Next {
    /// Connect again at this time.
    RetryAt(f64),
    /// No retry can help: reconnecting against it would only loop.
    Dead,
}

/// Counts the failures since the connection last worked and turns each new one into a wait.
#[derive(Debug, Default)]
pub struct ReconnectPolicy {
    /// Failed attempts since the last acceptance or reset: the *n* of the attempt backoff.
    attempts: u32,
    /// Scene-listener refusals (code 18) since the last acceptance or reset. They wait on a
    /// sequence of their own, which no other failure advances or restarts.
    listener_refusals: u32,
}

impl ReconnectPolicy {
    /// What to do about `failure`, which happened at `now`. A failure counts as an attempt unless
    /// it is terminal or a drop after acceptance, so call this once per failure.
    pub fn next(&mut self, failure: Failure, now: f64) -> Next {
        // The least wait the reason asks for, whatever the attempt backoff has reached.
        let floor = match failure {
            Failure::Unreached | Failure::HandshakeFailed => 0.0,
            Failure::Disconnected { reason, accepted } => match reason {
                // The server ended it for good, or the code is one we do not know: a retry would
                // only repeat it.
                PulseDisconnect::DuplicateSession
                | PulseDisconnect::Banned
                | PulseDisconnect::InputRateExceeded
                | PulseDisconnect::DiscreteEventRateExceeded
                | PulseDisconnect::InvalidInputField
                | PulseDisconnect::InvalidEmoteField
                | PulseDisconnect::InvalidTeleportField
                | PulseDisconnect::HandshakeReplayRejected
                | PulseDisconnect::InvalidHandshakeField
                | PulseDisconnect::PacketCorrupted
                | PulseDisconnect::InvalidSceneListenerField
                | PulseDisconnect::Unknown(_) => return Next::Dead,
                PulseDisconnect::None
                | PulseDisconnect::Graceful
                | PulseDisconnect::AuthTimeout
                | PulseDisconnect::AuthFailed => {
                    // A connection that was working and dropped is not a failed attempt.
                    if accepted {
                        return Next::RetryAt(now + DEFAULT_RETRY_DELAY);
                    }
                    0.0
                }
                PulseDisconnect::ServerFull => SERVER_FULL_RETRY_DELAY,
                PulseDisconnect::PreAuthIpLimit
                | PulseDisconnect::PreAuthBudget
                | PulseDisconnect::IpConnectionLimitExceeded => AUTH_EXHAUSTION_RETRY_DELAY,
                PulseDisconnect::SceneListenerIpLimitExceeded => {
                    self.listener_refusals = self.listener_refusals.saturating_add(1);
                    doubled(
                        LISTENER_LIMIT_START,
                        LISTENER_LIMIT_CAP,
                        self.listener_refusals,
                    )
                }
            },
        };

        self.attempts = self.attempts.saturating_add(1);
        let backoff = doubled(ATTEMPT_BACKOFF_START, ATTEMPT_BACKOFF_CAP, self.attempts);
        Next::RetryAt(now + backoff.max(floor))
    }

    /// The server accepted this connection's handshake: the failures before it no longer count.
    pub fn on_accepted(&mut self) {
        self.reset();
    }

    /// Forgets every failure: the next one is attempt 1, and the listener sequence starts over.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// The `step`th wait of a sequence that starts at `start`, doubles with each step and stops at
/// `cap`. Steps count from 1.
fn doubled(start: f64, cap: f64, step: u32) -> f64 {
    // `powi` takes an `i32`; a step count beyond it saturates to infinity, which `min` brings down
    // to the cap.
    let doublings = i32::try_from(step.saturating_sub(1)).unwrap_or(i32::MAX);
    (start * 2f64.powi(doublings)).min(cap)
}

#[cfg(test)]
mod tests;
