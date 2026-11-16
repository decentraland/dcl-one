//! Decides when the local player's movement goes to the Pulse server.
//!
//! Three kinds of send share one token bucket. Ordinary movement goes out at most every
//! [`REGULAR_SPACING_SECONDS`]; a player who stands still sends a keepalive every
//! [`IDLE_KEEPALIVE_SECONDS`]; and an edge (a take-off, a landing, a jump, a change of scene-driven
//! clip) goes out on the frame it happens, inside the spacing, so other players see it at once.
//!
//! The server debits a bucket for every movement input and disconnects a peer that finds it empty:
//! 20 inputs a second, 16 held (`services/catalyst/comms/src/pulse/hardening.rs`,
//! `DEFAULT_INPUT_MAX_HZ` and `DEFAULT_INPUT_BURST`; the rejection is `InputRateExceeded` in
//! `server/mod.rs`). The bucket here refills [`MOVEMENT_REFILL_PER_SECOND`] tokens a second, holds
//! [`MOVEMENT_BURST`], and every send costs one. Any window of one second therefore holds at most
//! [`MOVEMENT_MAX_SENDS_PER_SECOND`] sends, under the server's 20 and its burst of 16, which leaves
//! room for frames that bunch up in transit.
//!
//! A bucket rather than a sliding window of sends, because a window lets a storm of edges (a grounded
//! flag flickering at its threshold, a scene swapping clips every frame) spend a whole second of
//! budget at once and then go silent, ordinary movement included. The bucket refills as it empties,
//! so a storm settles to one send per [`REGULAR_SPACING_SECONDS`], the cadence of ordinary movement.
//! An edge that finds the bucket empty is not lost: the caller keeps comparing against the last state
//! it sent, so the edge is still there on the next frame a token is back.
//!
//! A pure object, like [`crate::pulse::pacing`]: its times are `f64` seconds on the caller's clock
//! and it knows nothing of Bevy or the connection, so a test drives it without an app.

/// Tokens the bucket regains each second: half of the server's 20 inputs a second, and the rate of
/// ordinary movement.
pub const MOVEMENT_REFILL_PER_SECOND: f64 = 10.0;
/// Tokens the bucket holds: sends that can follow one another at once after a quiet spell.
pub const MOVEMENT_BURST: f64 = 3.0;
/// The most sends in any window of one second: the burst plus a second of refill.
pub const MOVEMENT_MAX_SENDS_PER_SECOND: u32 = (MOVEMENT_BURST + MOVEMENT_REFILL_PER_SECOND) as u32;
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
    /// A take-off, a landing, a new jump or a new clip: sends at once, a token allowing.
    Edge,
}

/// Whether a head angle moved by more than [`HEAD_EPSILON_DEGREES`] since the last send.
pub fn head_turned(last_degrees: f32, now_degrees: f32) -> bool {
    (now_degrees - last_degrees).abs() > HEAD_EPSILON_DEGREES
}

/// Remembers the bucket and when movement last went out. The default is a full bucket with nothing
/// sent, so the first ask is allowed.
pub struct MovementPacer {
    /// Tokens held as of `updated_at`.
    tokens: f64,
    updated_at: f64,
    last_sent: Option<f64>,
}

impl Default for MovementPacer {
    fn default() -> Self {
        Self {
            tokens: MOVEMENT_BURST,
            updated_at: 0.0,
            last_sent: None,
        }
    }
}

impl MovementPacer {
    /// The tokens held at `now`.
    fn tokens_at(&self, now: f64) -> f64 {
        let elapsed = (now - self.updated_at).max(0.0);
        (self.tokens + elapsed * MOVEMENT_REFILL_PER_SECOND).min(MOVEMENT_BURST)
    }

    /// Whether a send is allowed at `now` for `change`. It does not record one: call
    /// [`record_send`](Self::record_send) when the packet goes out.
    pub fn should_send(&self, now: f64, change: Change) -> bool {
        // A whole token pays for a send. The same tolerance on every comparison absorbs the rounding
        // of times that are sums of frame lengths: six 60 Hz frames add up to 0.09999999999999998.
        if self.tokens_at(now) < 1.0 - 1e-9 {
            return false;
        }
        let Some(last) = self.last_sent else {
            return true;
        };
        let elapsed = now - last;
        match change {
            Change::Edge => true,
            Change::Moving => elapsed >= REGULAR_SPACING_SECONDS - 1e-9,
            Change::Idle => elapsed >= IDLE_KEEPALIVE_SECONDS - 1e-9,
        }
    }

    /// Note that a packet went out at `now`.
    pub fn record_send(&mut self, now: f64) {
        self.tokens = (self.tokens_at(now) - 1.0).max(0.0);
        self.updated_at = now;
        self.last_sent = Some(now);
    }
}

#[cfg(test)]
mod tests;
