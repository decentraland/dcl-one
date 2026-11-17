//! The animation fields this player sends in a Pulse state: `movement_blend` and `is_falling`.
//!
//! `movement_blend` runs 0..3 over idle, walk, jog and run. The Unity client lerps it straight into
//! its animator, so it is encoded the way that client encodes it: the gait index times the fraction
//! of the gait's speed limit. Pure functions, so a test drives them without an app.

use common::structs::LocomotionSpeeds;

/// The top of the blend scale.
const BLEND_MAX: f32 = 3.0;
/// A blend at or below this reads as standing still.
const WALK_EPSILON: f32 = 0.05;
/// How far above a gait's speed limit a player still counts as being in that gait.
const TIER_TOLERANCE: f32 = 1.05;
/// The vertical speed (m/s) below which an airborne player is falling.
const FALL_SPEED: f32 = -3.0;

/// The `movement_blend` to send for a player moving at `planar_speed` (m/s, horizontal) under the
/// given locomotion speeds. The explorer has no walk or jog input mode, so the gait is the tier the
/// speed falls in: walk within the walk limit, jog within the jog limit, run above it (each limit
/// with [`TIER_TOLERANCE`]). Each tier's blend stays inside its own range, which keeps the blend
/// non-decreasing in speed across the tier edges.
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
        (2.0 * planar_speed / jog).clamp(1.0, 2.0)
    } else {
        (3.0 * planar_speed / run).clamp(2.0, BLEND_MAX)
    };
    if blend <= WALK_EPSILON {
        0.0
    } else {
        blend
    }
}

/// Whether the player is airborne and falling faster than [`FALL_SPEED`]. This is the Unity
/// client's fall-animation rule and its shipped `AnimationFallSpeed` of -3 m/s, so a falling
/// player shows the same clip on that client's screens.
pub fn is_falling(grounded: bool, vertical_velocity: f32) -> bool {
    !grounded && vertical_velocity < FALL_SPEED
}

#[cfg(test)]
mod tests;
