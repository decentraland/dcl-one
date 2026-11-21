// common phsyical variables (currently only fall speed is used by both foreign avater dynamics and user dynamics...)
use std::f32::consts::PI;

use bevy::math::DVec3;

/// Steepest surface the avatar stands on, radians from up (46.5 degrees, the default movement
/// scene's grounded angle). A steeper contact is a wall or a slope it slides down; the avatar
/// walking on the ground does not climb it.
pub const MAX_GROUND_INCLINE: f32 = 46.5 * PI / 180.0;
/// Highest riser the avatar walking on the ground steps onto.
pub const MAX_STEP_HEIGHT: f32 = 0.45;
/// How far below its slope the ground may fall away under a walking avatar that the avatar
/// still follows it down (a step down); a larger drop is an edge it walks off.
pub const GROUND_SNAP_DROP: f32 = 0.15;
pub const PLAYER_GROUND_THRESHOLD: f32 = 0.05;

/// A contact normal of a surface the avatar can stand on ([`MAX_GROUND_INCLINE`]).
pub fn walkable(normal: DVec3) -> bool {
    normal.y >= f64::from(MAX_GROUND_INCLINE.cos())
}

/// Feet at `ground_height` over the ground stand on it ([`PLAYER_GROUND_THRESHOLD`]).
pub fn grounded(ground_height: f32) -> bool {
    ground_height <= PLAYER_GROUND_THRESHOLD
}

pub const PLAYER_COLLIDER_RADIUS: f32 = 0.3;
pub const PLAYER_COLLIDER_HEIGHT: f32 = 2.0;
pub const PLAYER_COLLIDER_OVERLAP: f32 = 1e-5;
