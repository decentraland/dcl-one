//! Decides when the local player's movement goes to the Pulse server.
//!
//! Three kinds of send share one [`TokenBucket`]. Ordinary movement goes out at most every
//! [`REGULAR_SPACING_SECONDS`]; a player who stands still sends a keepalive every
//! [`IDLE_KEEPALIVE_SECONDS`]; and an edge goes out on the frame it happens, inside the spacing, so
//! other players see it at once. An edge is a change of scene-driven clip, or a change of velocity
//! that no ordinary acceleration makes: a jump impulse, a landing, an abrupt start, stop or turn
//! ([`EDGE_VELOCITY_CHANGE`]).
//!
//! The server debits a bucket for every movement input and disconnects a peer that finds it empty:
//! 20 inputs a second, 16 held (`services/catalyst/comms/src/pulse/hardening.rs`,
//! `DEFAULT_INPUT_MAX_HZ` and `DEFAULT_INPUT_BURST`; the rejection is `InputRateExceeded` in
//! `server/mod.rs`). The bucket here refills [`MOVEMENT_REFILL_PER_SECOND`] tokens a second, holds
//! [`MOVEMENT_BURST`], and every send costs one. Any window of one second therefore holds at most
//! the burst plus a second of refill, 13 sends, under the server's 20 and its burst of 16. The
//! margin covers frames that bunch up in transit: a stall of `T` seconds delivered at once puts
//! `MOVEMENT_BURST + 10 T` inputs in front of the server, which stays under its 16 for `T` up to
//! about 1.3 s. A deployment that lowers `PULSE_INPUT_MAX_HZ` or `PULSE_INPUT_BURST` below these
//! numbers would disconnect clients, as it would for any sender.
//!
//! A bucket rather than a sliding window of sends, because a window lets a storm of edges (a scene
//! swapping clips every frame, a velocity jittering across the threshold) spend a whole second of
//! budget at once and then go silent, ordinary movement included. The bucket refills as it
//! empties, so a storm settles to one send per [`REGULAR_SPACING_SECONDS`], the cadence of
//! ordinary movement. An edge that finds the bucket empty is not lost: the caller keeps comparing
//! against the last state it sent, so the edge is still there on the next frame a token is back.
//!
//! A pure object, like [`super::pacing`]: its times are `f64` seconds on the caller's clock and it
//! knows nothing of Bevy or the connection, so a test drives it without an app.

use super::pacing::TokenBucket;

/// Tokens the bucket regains each second: half of the server's 20 inputs a second, and the rate of
/// ordinary movement.
pub const MOVEMENT_REFILL_PER_SECOND: f64 = 10.0;
/// Tokens the bucket holds: sends that can follow one another at once after a quiet spell.
pub const MOVEMENT_BURST: f64 = 3.0;
/// The shortest time between two sends of ordinary movement.
pub const REGULAR_SPACING_SECONDS: f64 = 0.1;
/// The longest time between two sends while nothing changes.
pub const IDLE_KEEPALIVE_SECONDS: f64 = 1.0;
/// A change of velocity since the last send beyond this, in m/s, is an edge.
///
/// With the movement scene's defaults, a jump's rise (`JUMP_RISE_DECEL`, 40 m/s²) reaches it in
/// one [`REGULAR_SPACING_SECONDS`], a 1 m jump lands at 4.5 m/s, and a launch (8.9 m/s) or a key
/// release or reversal at jog speed (8 m/s) passes it.
pub const EDGE_VELOCITY_CHANGE: f32 = 4.0;

/// What changed since the last send.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// Nothing: only the keepalive is due.
    Idle,
    /// Position, rotation, velocity, head or pointing moved: the next regular slot sends.
    Moving,
    /// A new clip, or a velocity change past [`EDGE_VELOCITY_CHANGE`]: sends at once, a token
    /// allowing.
    Edge,
}

impl Change {
    /// The kind of change since the last send. `velocity_change` is the length of the difference
    /// between the velocity now and the velocity last sent, in m/s; `moved` is whether anything the
    /// regular slot carries changed.
    pub fn since_last_send(clip_changed: bool, velocity_change: f32, moved: bool) -> Self {
        if clip_changed || velocity_change > EDGE_VELOCITY_CHANGE {
            Change::Edge
        } else if moved {
            Change::Moving
        } else {
            Change::Idle
        }
    }
}

/// Remembers the bucket and when movement last went out. The default is a full bucket with nothing
/// sent, so the first ask is allowed.
pub struct MovementPacer {
    bucket: TokenBucket,
    last_sent: Option<f64>,
}

impl Default for MovementPacer {
    fn default() -> Self {
        Self {
            bucket: TokenBucket::new(MOVEMENT_REFILL_PER_SECOND, MOVEMENT_BURST),
            last_sent: None,
        }
    }
}

impl MovementPacer {
    /// What a spacing may fall short by: times are sums of frame lengths, and six 60 Hz frames add
    /// up to 0.09999999999999998.
    const ROUNDING: f64 = 1e-9;

    /// Whether a send is allowed at `now` for `change`. It does not record one: call
    /// [`record_send`](Self::record_send) when the packet goes out.
    pub fn should_send(&self, now: f64, change: Change) -> bool {
        if !self.bucket.has_token(now) {
            return false;
        }
        let Some(last) = self.last_sent else {
            return true;
        };
        let elapsed = now - last;
        match change {
            Change::Edge => true,
            Change::Moving => elapsed >= REGULAR_SPACING_SECONDS - Self::ROUNDING,
            Change::Idle => elapsed >= IDLE_KEEPALIVE_SECONDS - Self::ROUNDING,
        }
    }

    /// Note that a packet went out at `now`.
    pub fn record_send(&mut self, now: f64) {
        self.bucket.try_spend(now);
        self.last_sent = Some(now);
    }
}

#[cfg(test)]
mod tests;
