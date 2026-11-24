//! What the file name of a movement clip that a scene plays says about the avatar's motion.

use crate::structs::MoveKind;

/// The phases of a hard landing's stun, by file name prefix.
const STUN_CLIPS: [&str; 3] = [
    "hard_landing_enter",
    "hard_landing_loop",
    "hard_landing_exit",
];

/// The file name of a clip, lowercase, without folder and extension.
fn clip_name(src: &str) -> String {
    let file = src.rsplit(['/', '\\']).next().unwrap_or(src);
    file.rsplit_once('.')
        .map_or(file, |(stem, _)| stem)
        .to_lowercase()
}

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
        let name = clip_name(src);
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

    /// Jumps since the ground. A glide only follows the air jump, so it counts two.
    pub fn jumps(self) -> u32 {
        match self {
            Self::Jump => 1,
            Self::DoubleJump | Self::GlideOpening | Self::Gliding => 2,
            Self::None | Self::GlideClosing | Self::HardLanding => 0,
        }
    }

    /// True for the clip of a jump on its way up: [`Self::Jump`] and [`Self::DoubleJump`].
    pub fn is_jump(self) -> bool {
        matches!(self, Self::Jump | Self::DoubleJump)
    }

    /// True for a clip that a jump starts with: a jump's start clip, or an air jump's clip.
    pub fn starts_jump(src: &str) -> bool {
        match Self::of_clip(src, 1.0) {
            Self::DoubleJump => true,
            Self::Jump => clip_name(src).ends_with("start"),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn a_jump_starts_with_its_start_clip_or_an_air_jump_clip() {
        let anim = "assets/animations/";
        for (clip, starts) in [
            ("Jump_Start.glb", true),
            ("Jog_Jump_Start.glb", true),
            ("Run_Jump_Start.glb", true),
            ("DoubleJump_Base.glb", true),
            ("DoubleJump_Jog_Right.glb", true),
            ("Jump_Rise_Loop.glb", false),
            ("Run_Jump_Mid.glb", false),
            ("Jump_Fall_Loop.glb", false),
            ("Jog_Jump_End.glb", false),
            ("Gliding_AvatarStartRig.glb", false),
            ("Walk.glb", false),
        ] {
            assert_eq!(
                ClipMotion::starts_jump(&format!("{anim}{clip}")),
                starts,
                "{clip}"
            );
        }
    }
}
