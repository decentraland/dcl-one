//! The gait this player sends in Pulse's `movement_blend`.
//!
//! `movement_blend` runs 0..3 over idle, walk, jog and run. The sender picks the gait whose speed
//! tier the player is in and writes a blend that rounds back to it; [`gait_from_blend`] is the
//! rounding rule a receiver applies, with anything above [`WALK_EPSILON`] at least a walk. Other
//! clients smooth the blend instead of tiering it, so a received gait says nothing reliable about
//! the sender's speed tier and the velocity fallback does not read it. Pure functions, so a test
//! drives them without an app.

use common::structs::LocomotionSpeeds;

/// The top of the blend scale.
pub const BLEND_MAX: f32 = 3.0;
/// A blend at or below this reads as standing still.
pub const WALK_EPSILON: f32 = 0.05;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gait {
    Idle,
    Walk,
    Jog,
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

#[cfg(test)]
mod tests;
