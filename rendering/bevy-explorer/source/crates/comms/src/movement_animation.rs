//! The animation fields of the rfc4 `Movement` that this client sends. Jumps, the glider and hard
//! landings come from the file name of the clip that the movement scene plays. Under any other
//! scene, `double` or `glide` in its clip's path marks the air jump or the glide, and other jumps
//! come from the take-off.

use bevy::prelude::*;
use common::{movement_clip::ClipMotion, structs::LocomotionSpeeds};
use dcl_component::proto_components::kernel::comms::rfc4::{self, movement::GlideState};

use crate::gait;

/// m/s, up positive. An avatar off the ground that falls faster is in a long fall.
const LONG_FALLING_SPEED: f32 = -8.0;

/// The prop state of the glider that the clip shows.
fn glide_state(motion: ClipMotion) -> GlideState {
    match motion {
        ClipMotion::GlideOpening => GlideState::OpeningProp,
        ClipMotion::Gliding => GlideState::Gliding,
        ClipMotion::GlideClosing => GlideState::ClosingProp,
        _ => GlideState::PropClosed,
    }
}

/// Whether the avatar has jumped since it last stood on the ground.
#[derive(Debug, Default, Clone, Copy)]
pub struct JumpCount {
    jumped: bool,
}

impl JumpCount {
    /// The `jump_count` to send: 2 while the clip shows the air jump or the glide, else 1 after a
    /// jump until the ground. Receivers play a double jump for as long as the count stays at 2, so
    /// the fall after it, or a glider closed in the air, counts 1.
    pub fn update(&mut self, grounded: bool, motion: ClipMotion, jump_in_flight: bool) -> u32 {
        if grounded {
            self.jumped = false;
        }
        self.jumped |= motion.jumps() > 0 || (!grounded && jump_in_flight);
        motion.jumps().max(u32::from(self.jumped))
    }
}

/// An rfc4 `Movement` with only the animation fields set. The sender fills the rest.
pub fn wire_animation(
    velocity: Vec3,
    speeds: &LocomotionSpeeds,
    grounded: bool,
    motion: ClipMotion,
    jump_count: u32,
) -> rfc4::Movement {
    let is_falling = gait::is_falling(grounded, velocity.y);
    rfc4::Movement {
        movement_blend_value: gait::movement_blend(velocity.xz().length(), speeds),
        is_grounded: grounded,
        is_jumping: !grounded && jump_count > 0 && !is_falling,
        is_falling,
        is_long_fall: !grounded && velocity.y < LONG_FALLING_SPEED,
        is_stunned: motion == ClipMotion::HardLanding,
        jump_count: jump_count as i32,
        glide_state: glide_state(motion) as i32,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use common::structs::MoveKind;

    use super::*;

    const GAITS: LocomotionSpeeds = LocomotionSpeeds {
        walk: 1.5,
        jog: 8.0,
        run: 10.0,
    };

    #[test]
    fn the_jump_count_holds_in_the_air_and_resets_on_the_ground() {
        let mut count = JumpCount::default();
        // the start clip may begin before the feet leave the ground
        assert_eq!(count.update(true, ClipMotion::Jump, false), 1);
        assert_eq!(count.update(false, ClipMotion::Jump, false), 1);
        // the fall loop keeps the jump
        assert_eq!(count.update(false, ClipMotion::None, false), 1);
        assert_eq!(count.update(false, ClipMotion::DoubleJump, false), 2);
        assert_eq!(count.update(false, ClipMotion::Gliding, false), 2);
        // a glider closed in the air is the jump again
        assert_eq!(count.update(false, ClipMotion::GlideClosing, false), 1);
        assert_eq!(count.update(true, ClipMotion::GlideClosing, false), 0);
        // walking off a ledge is no jump
        assert_eq!(count.update(false, ClipMotion::None, false), 0);
    }

    #[test]
    fn a_jump_the_engine_has_in_flight_counts_one_until_the_ground() {
        let mut count = JumpCount::default();
        // a clip that names no jump, or none at all, while the engine's jump is in the air
        assert_eq!(count.update(false, ClipMotion::None, true), 1);
        assert_eq!(count.update(false, ClipMotion::DoubleJump, true), 2);
        assert_eq!(count.update(true, ClipMotion::None, true), 0);
        // falling without a jump
        assert_eq!(count.update(false, ClipMotion::None, false), 0);
    }

    #[test]
    fn the_fall_after_a_double_jump_reaches_receivers_as_no_double_jump() {
        let received = |count: &mut JumpCount, motion, jump_in_flight| {
            let jumps = count.update(false, motion, jump_in_flight);
            let wire = wire_animation(Vec3::new(0.0, -4.0, 6.0), &GAITS, false, motion, jumps);
            crate::global_crdt::remote_move_kind(
                wire.glide_state(),
                wire.jump_count,
                wire.is_jumping || wire.is_long_jump,
                wire.is_grounded,
            )
        };
        for jump_in_flight in [false, true] {
            let mut count = JumpCount::default();
            received(&mut count, ClipMotion::Jump, jump_in_flight);
            assert_eq!(
                received(&mut count, ClipMotion::DoubleJump, jump_in_flight),
                Some(MoveKind::DoubleJump)
            );
            // the fall loop ends the double jump: the receiver plays the jump in flight, which falls
            assert_eq!(
                received(&mut count, ClipMotion::None, jump_in_flight),
                Some(MoveKind::Jump)
            );
        }
        let mut count = JumpCount::default();
        received(&mut count, ClipMotion::Jump, false);
        received(&mut count, ClipMotion::DoubleJump, false);
        received(&mut count, ClipMotion::Gliding, false);
        assert_eq!(
            received(&mut count, ClipMotion::GlideClosing, false),
            Some(MoveKind::Glide)
        );
        assert_eq!(
            received(&mut count, ClipMotion::None, false),
            Some(MoveKind::Jump)
        );
    }

    #[test]
    fn falls_come_from_the_vertical_speed_off_the_ground() {
        let wire = |velocity, grounded, motion, jumps| {
            wire_animation(velocity, &GAITS, grounded, motion, jumps)
        };
        let rising = wire(Vec3::new(0.0, 4.0, 8.0), false, ClipMotion::Jump, 1);
        assert!(rising.is_jumping && !rising.is_falling && !rising.is_grounded);
        assert_eq!((rising.movement_blend_value, rising.jump_count), (2.0, 1));

        let falling = wire(Vec3::new(0.0, -4.0, 0.0), false, ClipMotion::None, 1);
        assert!(falling.is_falling && !falling.is_long_fall && !falling.is_jumping);
        let far = wire(Vec3::new(0.0, -9.0, 0.0), false, ClipMotion::None, 0);
        assert!(far.is_falling && far.is_long_fall);
        // on the ground nothing falls, whatever the speed
        let landed = wire(Vec3::new(0.0, -9.0, 0.0), true, ClipMotion::HardLanding, 0);
        assert!(landed.is_grounded && !landed.is_falling && landed.is_stunned);

        let gliding = wire(Vec3::new(0.0, -1.5, 6.0), false, ClipMotion::Gliding, 2);
        assert_eq!(gliding.glide_state, GlideState::Gliding as i32);
        assert_eq!(
            wire(Vec3::ZERO, true, ClipMotion::None, 0).glide_state,
            GlideState::PropClosed as i32
        );
    }
}
