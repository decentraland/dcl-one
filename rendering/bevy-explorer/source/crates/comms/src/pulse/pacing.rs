//! Keeps a player's teleports and emotes within the servers' event budget.
//!
//! Both Pulse servers debit one bucket per peer for every teleport, emote start, emote stop and
//! scene-listener update: a token returns every 50 ms, up to 16 held (Pulse
//! `src/DCLPulse/appsettings.json:44-47`; dcl-one `services/catalyst/comms/src/pulse/hardening.rs`,
//! `DEFAULT_DISCRETE_RATE_PER_SEC`). A peer that finds it empty is disconnected, and for a reason
//! that is terminal. A scene can empty it within a second: a durationless `movePlayerTo` every frame
//! is a teleport every frame. unity-explorer has no limiter; this goes beyond it, so that a scene
//! cannot end the session this way.
//!
//! The pacer keeps a bucket of half that size, which leaves room for frames that bunch up in transit,
//! and holds back what it does not allow. Only the latest teleport and the latest emote command
//! wait, since an older one of the kind says nothing the newer does not. A pure object, like the
//! reconnect policy: its times are `f64` seconds on the caller's clock, and it knows nothing of Bevy
//! or the connection, so a test drives it without an app. Scene-listener updates keep their own
//! spacing and do not pass through it.

use std::collections::VecDeque;

use common::util::dcl_assert;

use super::transport::PulseFrame;

/// How many events a second the pacer lets out: half of the servers' 20
/// (Pulse `src/DCLPulse/appsettings.json:44-47`).
pub const PACED_EVENTS_PER_SECOND: f64 = 10.0;
/// How many it lets out at once after a quiet spell: half of the servers' burst of 16.
pub const PACED_EVENTS_BURST: f64 = 8.0;

/// A bucket that lets teleports and emotes out at [`PACED_EVENTS_PER_SECOND`], holding the latest of
/// each kind while it is empty. The default is a full bucket with nothing waiting, which is what a
/// new connection starts from: the servers count each peer's budget from full.
#[derive(Default)]
pub struct DiscreteEventPacer {
    /// How far below full the bucket was at `refilled_at`, in events: zero is a full bucket, so
    /// the default is one.
    spent: f64,
    /// The time `spent` is as of.
    refilled_at: f64,
    /// The frames that wait, in the order of their latest offer: at most one of each kind.
    waiting: VecDeque<PulseFrame>,
}

impl DiscreteEventPacer {
    /// Hold a `Teleport` or `Emote` frame; it replaces a waiting frame of the same kind.
    pub fn offer(&mut self, frame: PulseFrame) {
        dcl_assert!(
            matches!(
                frame.kind,
                super::transport::FrameKind::Teleport | super::transport::FrameKind::Emote
            ),
            "only teleports and emotes are paced, not {:?}",
            frame.kind
        );
        self.waiting.retain(|waiting| waiting.kind != frame.kind);
        self.waiting.push_back(frame);
    }

    /// The waiting frames the bucket allows now, in the order of their latest offer.
    pub fn release(&mut self, now: f64) -> Vec<PulseFrame> {
        let elapsed = (now - self.refilled_at).max(0.0);
        self.spent = (self.spent - elapsed * PACED_EVENTS_PER_SECOND).max(0.0);
        self.refilled_at = now;

        let mut released = Vec::new();
        // A whole token pays for a frame; the part of one that has returned is kept.
        while PACED_EVENTS_BURST - self.spent >= 1.0 {
            let Some(frame) = self.waiting.pop_front() else {
                break;
            };
            self.spent += 1.0;
            released.push(frame);
        }
        released
    }
}

#[cfg(test)]
mod tests;
