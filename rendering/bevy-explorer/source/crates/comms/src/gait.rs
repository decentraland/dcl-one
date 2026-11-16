//! The gait a remote player is moving at, as Pulse carries it in `movement_blend`, and the rules
//! that turn it into and out of a clip choice.
//!
//! `movement_blend` runs 0..3 over idle, walk, jog and run. The sender picks the gait whose speed
//! tier the player is in and writes a blend that rounds back to it; a receiver reads the gait as
//! the rounded blend, with anything above [`WALK_EPSILON`] at least a walk. Pure functions, so a
//! test drives them without an app.

use bevy::prelude::*;
use common::structs::LocomotionSpeeds;

/// The top of the blend scale.
pub const BLEND_MAX: f32 = 3.0;
/// A blend at or below this reads as standing still.
pub const WALK_EPSILON: f32 = 0.05;
/// The fallback picker shows the walk clip up to this speed (m/s) and the run clip above it.
pub const WALK_CLIP_MAX_SPEED: f32 = 2.6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gait {
    Idle,
    Walk,
    Jog,
    Run,
}

/// The gait and falling state last received from a remote player, read by the velocity fallback.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct RemoteGait {
    /// `None` when the received blend is not a plausible gait.
    pub gait: Option<Gait>,
    pub falling: bool,
}

/// The clip family the velocity fallback shows for a moving player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FallbackGait {
    Walk,
    Run,
}

/// How far above a gait's speed limit a player still counts as being in that gait.
const TIER_TOLERANCE: f32 = 1.05;
/// How far above the rounding point a raised blend sits.
const ROUNDING_MARGIN: f32 = 0.01;

/// The `movement_blend` to send for a player moving at `planar_speed` (m/s, horizontal) under the
/// given locomotion speeds. The gait is the tier the speed falls in: walk within the walk limit,
/// jog within the jog limit, run above it (each limit with [`TIER_TOLERANCE`]). Within jog and run
/// the blend is the gait index times the fraction of the limit, raised to just above the rounding
/// point below it, so a receiver rounding the blend recovers the gait. A received walk therefore
/// only ever describes a speed within the sender's walk limit.
pub fn movement_blend(planar_speed: f32, speeds: &LocomotionSpeeds) -> f32 {
    if planar_speed.is_nan() || planar_speed <= 0.0 {
        return 0.0;
    }
    let walk = speeds.walk.max(f32::EPSILON);
    let jog = speeds.jog.max(walk);
    let run = speeds.run.max(jog);
    let blend = if planar_speed <= walk * TIER_TOLERANCE {
        (planar_speed / walk).min(1.0)
    } else if jog > walk && planar_speed <= jog * TIER_TOLERANCE {
        (2.0 * planar_speed / jog).clamp(1.5 + ROUNDING_MARGIN, 2.0)
    } else {
        (3.0 * planar_speed / run).clamp(2.5 + ROUNDING_MARGIN, BLEND_MAX)
    };
    if blend <= WALK_EPSILON {
        0.0
    } else {
        blend
    }
}

/// Whether the player is airborne and descending.
pub fn is_falling(grounded: bool, vertical_velocity: f32) -> bool {
    !grounded && vertical_velocity < 0.0
}

/// The gait a received `movement_blend` names, or `None` when it is not a plausible blend.
pub fn gait_from_blend(blend: f32) -> Option<Gait> {
    if !blend.is_finite() || !(0.0..=BLEND_MAX + 1e-3).contains(&blend) {
        return None;
    }
    if blend <= WALK_EPSILON {
        return Some(Gait::Idle);
    }
    Some(match blend.round() as u32 {
        0 | 1 => Gait::Walk,
        2 => Gait::Jog,
        _ => Gait::Run,
    })
}

/// Walk or run clip for a player moving at `planar_speed` (m/s). A received walk is trusted at any
/// speed. A received jog or run is trusted only above [`WALK_CLIP_MAX_SPEED`], where it agrees with
/// the speed rule anyway: a sender that still writes speed squared marks a 2.5 m/s walk as a run.
/// Idle or an implausible gait defers to speed.
pub fn fallback_gait(received: Option<Gait>, planar_speed: f32) -> FallbackGait {
    if received == Some(Gait::Walk) || planar_speed <= WALK_CLIP_MAX_SPEED {
        FallbackGait::Walk
    } else {
        FallbackGait::Run
    }
}

#[cfg(test)]
mod tests;
