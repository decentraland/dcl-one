// common phsyical variables (currently only fall speed is used by both foreign avater dynamics and user dynamics...)
use std::f32::consts::PI;

use bevy::math::DVec3;

pub const MAX_CLIMBABLE_INCLINE: f32 = 1.5 * PI / 4.0; // radians from up - equal to 60 degree incline
pub const MAX_STEP_HEIGHT: f32 = 0.5;
/// The steepest surface the avatar stands on, in radians from up. Matches `GROUNDED_ANGLE` in the
/// movement scene.
pub const MAX_GROUND_INCLINE: f32 = 46.5 * PI / 180.0;
pub const PLAYER_GROUND_THRESHOLD: f32 = 0.05;
/// Feet within this of the ground end a jump for its clip, its take-off time and the broadcast jump
/// count; looser than [`PLAYER_GROUND_THRESHOLD`].
pub const PLAYER_LANDED_THRESHOLD: f32 = 0.2;

/// True when the feet are within [`PLAYER_LANDED_THRESHOLD`] of the ground: the one test of landed
/// against airborne.
pub fn landed(ground_height: f32) -> bool {
    ground_height < PLAYER_LANDED_THRESHOLD
}

/// True for a contact normal within [`MAX_GROUND_INCLINE`] of up.
pub fn walkable(normal: DVec3) -> bool {
    normal.y >= f64::from(MAX_GROUND_INCLINE.cos())
}

pub const PLAYER_COLLIDER_RADIUS: f32 = 0.3;
pub const PLAYER_COLLIDER_HEIGHT: f32 = 2.0;
pub const PLAYER_COLLIDER_OVERLAP: f32 = 1e-5;
