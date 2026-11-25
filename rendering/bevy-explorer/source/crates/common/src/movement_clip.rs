//! What the file name of a clip that the movement scene plays says about the avatar's motion.

use crate::structs::MoveKind;

/// The file name of a clip without folder and extension.
fn clip_stem(src: &str) -> &str {
    let file = src.rsplit(['/', '\\']).next().unwrap_or(src);
    file.rsplit_once('.').map_or(file, |(stem, _)| stem)
}

/// The clips a ground jump starts with, one for each take-off gait.
const JUMP_STARTS: [&str; 3] = ["Jump_Start", "Jog_Jump_Start", "Run_Jump_Start"];
/// The clips of a ground jump on its way up after its start clip.
const JUMP_RISES: [&str; 6] = [
    "Jump_Rise_Loop",
    "Jump_Mid",
    "Jog_Jump_Rise_Loop",
    "Jog_Jump_Mid",
    "Run_Jump_Rise_Loop",
    "Run_Jump_Mid",
];
/// The air jump's clips: an air jump plays one of them from its start.
const DOUBLE_JUMPS: [&str; 6] = [
    "DoubleJump_Base",
    "DoubleJump_Base_Right",
    "DoubleJump_Jog",
    "DoubleJump_Jog_Right",
    "DoubleJump_Run",
    "DoubleJump_Run_Right",
];

/// What the file name of a clip of the movement scene (`movement-scene/assets/animations`) says
/// about the avatar's motion. Any other clip is [`Self::None`].
///
/// - The fall loops and touchdowns are not jumps, so walking off a ledge is not a jump.
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
        match clip_stem(src) {
            stem if JUMP_STARTS.contains(&stem) || JUMP_RISES.contains(&stem) => Self::Jump,
            stem if DOUBLE_JUMPS.contains(&stem) => Self::DoubleJump,
            "Gliding_AvatarStartRig" => Self::GlideOpening,
            "Gliding_AvatarForwardRig" | "Gliding_AvatarIdleRig" => Self::Gliding,
            "Gliding_AvatarEndRig" | "Hard_Landing_GliderRig" => Self::GlideClosing,
            "Hard_Landing_Enter" | "Hard_Landing_Loop" | "Hard_Landing_Exit" if speed <= 1.0 => {
                Self::HardLanding
            }
            _ => Self::None,
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

    /// True for a clip that a jump starts with: a ground jump's start clip, or an air jump's clip.
    pub fn starts_jump(src: &str) -> bool {
        let stem = clip_stem(src);
        JUMP_STARTS.contains(&stem) || DOUBLE_JUMPS.contains(&stem)
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
            ("Jump_Rise_Loop.glb", ClipMotion::Jump),
            ("Jump_Mid.glb", ClipMotion::Jump),
            ("Jump_Fall_Loop.glb", ClipMotion::None),
            ("Jump_End.glb", ClipMotion::None),
            ("Jog_Jump_Start.glb", ClipMotion::Jump),
            ("Jog_Jump_Rise_Loop.glb", ClipMotion::Jump),
            ("Jog_Jump_Mid.glb", ClipMotion::Jump),
            ("Jog_Jump_Fall_Loop.glb", ClipMotion::None),
            ("Jog_Jump_End.glb", ClipMotion::None),
            ("Run_Jump_Start.glb", ClipMotion::Jump),
            ("Run_Jump_Rise_Loop.glb", ClipMotion::Jump),
            ("Run_Jump_Mid.glb", ClipMotion::Jump),
            ("Run_Jump_Fall_Loop.glb", ClipMotion::None),
            ("Run_Jump_End.glb", ClipMotion::None),
            ("Long_Fall_Loop.glb", ClipMotion::None),
            ("DoubleJump_Base.glb", ClipMotion::DoubleJump),
            ("DoubleJump_Base_Right.glb", ClipMotion::DoubleJump),
            ("DoubleJump_Jog.glb", ClipMotion::DoubleJump),
            ("DoubleJump_Jog_Right.glb", ClipMotion::DoubleJump),
            ("DoubleJump_Run.glb", ClipMotion::DoubleJump),
            ("DoubleJump_Run_Right.glb", ClipMotion::DoubleJump),
            ("Gliding_AvatarStartRig.glb", ClipMotion::GlideOpening),
            ("Gliding_AvatarForwardRig.glb", ClipMotion::Gliding),
            ("Gliding_AvatarIdleRig.glb", ClipMotion::Gliding),
            ("Gliding_AvatarEndRig.glb", ClipMotion::GlideClosing),
            ("Hard_Landing_GliderRig.glb", ClipMotion::GlideClosing),
            ("Hard_Landing_Enter.glb", ClipMotion::HardLanding),
            ("Hard_Landing_Loop.glb", ClipMotion::HardLanding),
            ("Hard_Landing_Exit.glb", ClipMotion::HardLanding),
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
    }

    #[test]
    fn only_the_movement_scenes_clip_names_count() {
        for (clip, motion) in [
            // the folder and the extension do not count
            ("DoubleJump_Base", ClipMotion::DoubleJump),
            ("other\\folder\\Jump_Mid.gltf", ClipMotion::Jump),
            // a jump's touchdown and fall are no jump, though they share its prefix
            ("other/folder/Jump_End.gltf", ClipMotion::None),
            ("Jump_Fall_Loop", ClipMotion::None),
            ("Run_Jump_End", ClipMotion::None),
            ("Jog_Jump_Fall_Loop.gltf", ClipMotion::None),
            // names the movement scene does not play
            ("jump_start.glb", ClipMotion::None),
            ("avatar_jump.glb", ClipMotion::None),
            ("double_jump.glb", ClipMotion::None),
            ("player_glide.glb", ClipMotion::None),
            ("Glider_Open.glb", ClipMotion::None),
        ] {
            assert_eq!(ClipMotion::of_clip(clip, 1.0), motion, "{clip}");
        }
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
            ("JumpStart.glb", false),
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
