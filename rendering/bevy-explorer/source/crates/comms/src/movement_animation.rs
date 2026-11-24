//! The animation fields of the rfc4 `Movement` that this client sends. Jumps, the glider and hard
//! landings come from the file name of the clip that the movement scene plays.

use bevy::prelude::*;
use common::structs::{LocomotionSpeeds, MoveKind};
use dcl_component::proto_components::kernel::comms::rfc4::{self, movement::GlideState};

use crate::gait;

/// m/s, up positive. An avatar off the ground that falls faster is in a long fall.
const LONG_FALLING_SPEED: f32 = -8.0;

/// The phases of a hard landing's stun, by file name prefix.
const STUN_CLIPS: [&str; 3] = [
    "hard_landing_enter",
    "hard_landing_loop",
    "hard_landing_exit",
];

/// What the file name of a movement clip says about the avatar's motion. Matched
/// case-insensitively on the file name, without folder and extension.
///
/// - The fall loops are not jumps, so walking off a ledge is not a jump.
/// - The stun clips count only at speed 1 or slower, because the scene speeds up the Exit clip as
///   the stun ends.
/// - The plain `Hard_Landing` clip is no stun. The scene plays it for any big drop, and faster for
///   a soft landing.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ClipMotion {
    #[default]
    None,
    Jump,
    DoubleJump,
    GlideOpening,
    Gliding,
    GlideClosing,
    HardLanding,
}

impl ClipMotion {
    pub fn of_clip(src: &str, speed: f32) -> Self {
        let file = src.rsplit(['/', '\\']).next().unwrap_or(src);
        let name = file
            .rsplit_once('.')
            .map_or(file, |(stem, _)| stem)
            .to_lowercase();
        if name.starts_with("doublejump") {
            Self::DoubleJump
        } else if name.starts_with("hard_landing") && name.contains("glider") {
            Self::GlideClosing
        } else if name.starts_with("glid") {
            if name.contains("start") {
                Self::GlideOpening
            } else if name.contains("end") {
                Self::GlideClosing
            } else {
                Self::Gliding
            }
        } else if ["jump_start", "jump_rise_loop", "jump_mid"]
            .iter()
            .any(|phase| name.ends_with(phase))
        {
            Self::Jump
        } else if speed <= 1.0 && STUN_CLIPS.iter().any(|phase| name.starts_with(phase)) {
            Self::HardLanding
        } else {
            Self::None
        }
    }

    pub fn of_move_kind(kind: MoveKind) -> Self {
        match kind {
            MoveKind::Jump => Self::Jump,
            MoveKind::DoubleJump => Self::DoubleJump,
            MoveKind::Glide => Self::Gliding,
            _ => Self::None,
        }
    }

    /// A glide only follows the air jump, so it counts two.
    fn jumps(self) -> u32 {
        match self {
            Self::Jump => 1,
            Self::DoubleJump | Self::GlideOpening | Self::Gliding => 2,
            Self::None | Self::GlideClosing | Self::HardLanding => 0,
        }
    }

    fn glide_state(self) -> GlideState {
        match self {
            Self::GlideOpening => GlideState::OpeningProp,
            Self::Gliding => GlideState::Gliding,
            Self::GlideClosing => GlideState::ClosingProp,
            _ => GlideState::PropClosed,
        }
    }
}

/// Jumps since the avatar last stood on the ground.
#[derive(Debug, Default, Clone, Copy)]
pub struct JumpCount(u32);

impl JumpCount {
    pub fn update(&mut self, grounded: bool, motion: ClipMotion, jump_in_flight: bool) -> u32 {
        if grounded {
            self.0 = 0;
        }
        let engine_jumps = u32::from(!grounded && jump_in_flight);
        self.0 = self.0.max(motion.jumps()).max(engine_jumps);
        self.0
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
        glide_state: motion.glide_state() as i32,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GAITS: LocomotionSpeeds = LocomotionSpeeds {
        walk: 1.5,
        jog: 8.0,
        run: 10.0,
    };

    #[test]
    fn the_movement_scenes_clips_name_their_motion() {
        let anim = "assets/animations/";
        for (clip, motion) in [
            ("Idle.glb", ClipMotion::None),
            ("Walk.glb", ClipMotion::None),
            ("Jog.glb", ClipMotion::None),
            ("Run.glb", ClipMotion::None),
            ("Jog_Stop.glb", ClipMotion::None),
            ("Jump_Start.glb", ClipMotion::Jump),
            ("Jog_Jump_Rise_Loop.glb", ClipMotion::Jump),
            ("Run_Jump_Mid.glb", ClipMotion::Jump),
            ("Jump_Fall_Loop.glb", ClipMotion::None),
            ("Run_Jump_End.glb", ClipMotion::None),
            ("Long_Fall_Loop.glb", ClipMotion::None),
            ("DoubleJump_Base.glb", ClipMotion::DoubleJump),
            ("DoubleJump_Run_Right.glb", ClipMotion::DoubleJump),
            ("Gliding_AvatarStartRig.glb", ClipMotion::GlideOpening),
            ("Gliding_AvatarForwardRig.glb", ClipMotion::Gliding),
            ("Gliding_AvatarIdleRig.glb", ClipMotion::Gliding),
            ("Gliding_AvatarEndRig.glb", ClipMotion::GlideClosing),
            ("Hard_Landing_GliderRig.glb", ClipMotion::GlideClosing),
            ("Hard_Landing_Enter.glb", ClipMotion::HardLanding),
            ("Hard_Landing_Loop.glb", ClipMotion::HardLanding),
            // a big drop that does not stun plays the plain clip at full speed
            ("Hard_Landing.glb", ClipMotion::None),
        ] {
            assert_eq!(
                ClipMotion::of_clip(&format!("{anim}{clip}"), 1.0),
                motion,
                "{clip}"
            );
        }
        // the soft landing replays the plain clip's end faster, and the stun's Exit plays faster
        // once the stun is over
        assert_eq!(
            ClipMotion::of_clip("assets/animations/Hard_Landing.glb", 1.5),
            ClipMotion::None
        );
        assert_eq!(
            ClipMotion::of_clip("assets/animations/Hard_Landing_Exit.glb", 1.2),
            ClipMotion::None
        );
        // case, folder and extension do not matter
        assert_eq!(ClipMotion::of_clip("GLIDE", 1.0), ClipMotion::Gliding);
        assert_eq!(
            ClipMotion::of_clip("a\\doublejump.gltf", 1.0),
            ClipMotion::DoubleJump
        );
    }

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
        assert_eq!(count.update(false, ClipMotion::GlideClosing, false), 2);
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
