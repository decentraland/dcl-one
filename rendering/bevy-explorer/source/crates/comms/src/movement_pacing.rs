//! Decides when the local player's movement goes to the Pulse server.
//!
//! Three kinds of send share one budget. Ordinary movement goes out at most every
//! [`REGULAR_SPACING_SECONDS`]; a player who stands still sends a keepalive every
//! [`IDLE_KEEPALIVE_SECONDS`]; and an edge (a take-off, a landing, a jump, a change of scene-driven
//! clip) goes out on the frame it happens, inside the spacing, so other players see it at once.
//!
//! The server debits a bucket for every movement input and disconnects a peer that finds it empty:
//! 20 inputs a second, 16 held (`services/catalyst/comms/src/pulse/hardening.rs`,
//! `DEFAULT_INPUT_MAX_HZ` and `DEFAULT_INPUT_BURST`; the rejection is `InputRateExceeded` in
//! `server/mod.rs`). The pacer lets out at most [`MOVEMENT_SENDS_PER_SECOND`] in any window of one
//! second, half the refill rate and under the burst, which leaves room for frames that bunch up in
//! transit. An edge that finds the budget spent is not lost: the caller keeps comparing against the
//! last state it sent, so the edge is still there on the next frame the budget allows.
//!
//! A pure object, like [`crate::pulse::pacing`]: its times are `f64` seconds on the caller's clock
//! and it knows nothing of Bevy or the connection, so a test drives it without an app.

use std::collections::VecDeque;

/// The most movement inputs let out in any window of one second: half of the server's 20 a second.
pub const MOVEMENT_SENDS_PER_SECOND: u32 = 10;
/// The shortest time between two sends of ordinary movement.
pub const REGULAR_SPACING_SECONDS: f64 = 0.1;
/// The longest time between two sends while nothing changes.
pub const IDLE_KEEPALIVE_SECONDS: f64 = 1.0;
/// A head turn beyond this many degrees sends at the next slot instead of waiting for the keepalive.
///
/// The wire carries head yaw and pitch in 7 bits over 0..360, a step of 360 / 127 = 2.8 degrees
/// (`pulse_shared.proto`), so a turn under one step cannot change what the server sends on.
pub const HEAD_EPSILON_DEGREES: f32 = 3.0;

/// What changed since the last send.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// Nothing: only the keepalive is due.
    Idle,
    /// Position, rotation, velocity, head or pointing moved: the next regular slot sends.
    Moving,
    /// A take-off, a landing, a new jump or a new clip: sends at once, budget allowing.
    Edge,
}

/// Whether a head angle moved by more than [`HEAD_EPSILON_DEGREES`] since the last send.
pub fn head_turned(last_degrees: f32, now_degrees: f32) -> bool {
    (now_degrees - last_degrees).abs() > HEAD_EPSILON_DEGREES
}

/// Remembers when movement last went out. The default has sent nothing, so the first ask is allowed.
#[derive(Default)]
pub struct MovementPacer {
    /// The times of the sends in the last second, oldest first.
    recent: VecDeque<f64>,
    last_sent: Option<f64>,
}

impl MovementPacer {
    /// Whether a send is allowed at `now` for `change`. It does not record one: call
    /// [`record_send`](Self::record_send) when the packet goes out.
    pub fn should_send(&self, now: f64, change: Change) -> bool {
        let in_window = self.recent.iter().filter(|&&sent| sent > now - 1.0).count();
        if in_window >= MOVEMENT_SENDS_PER_SECOND as usize {
            return false;
        }
        let Some(last) = self.last_sent else {
            return true;
        };
        let elapsed = now - last;
        match change {
            Change::Edge => true,
            Change::Moving => elapsed >= REGULAR_SPACING_SECONDS,
            Change::Idle => elapsed >= IDLE_KEEPALIVE_SECONDS,
        }
    }

    /// Note that a packet went out at `now`.
    pub fn record_send(&mut self, now: f64) {
        while self.recent.front().is_some_and(|&sent| sent <= now - 1.0) {
            self.recent.pop_front();
        }
        self.recent.push_back(now);
        self.last_sent = Some(now);
    }
}

#[cfg(test)]
mod tests;
