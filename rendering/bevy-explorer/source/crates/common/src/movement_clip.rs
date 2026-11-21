//! What the file name of a clip that a scene plays says about the avatar's motion.

use crate::structs::MoveKind;

/// The file name of a clip without folder and extension.
fn clip_stem(src: &str) -> &str {
    let file = src.rsplit(['/', '\\']).next().unwrap_or(src);
    file.rsplit_once('.').map_or(file, |(stem, _)| stem)
}

/// Whether `text` contains `word`, ignoring ASCII case.
fn contains_ignore_case(text: &str, word: &str) -> bool {
    text.as_bytes()
        .windows(word.len())
        .any(|window| window.eq_ignore_ascii_case(word.as_bytes()))
}

/// The clips of a ground jump on its way up, from its start clip, for each take-off gait.
const JUMPS: [&str; 9] = [
    "Jump_Start",
    "Jog_Jump_Start",
    "Run_Jump_Start",
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
/// about the avatar's motion. Any other clip is [`Self::None`]. [`Self::of_other_scene_clip`] reads
/// the clips of any other scene.
///
/// The fall loops and touchdowns are not jumps, so walking off a ledge is not a jump.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ClipMotion {
    #[default]
    None,
    Jump,
    DoubleJump,
    GlideOpening,
    Gliding,
    GlideClosing,
}

impl ClipMotion {
    pub fn of_clip(src: &str) -> Self {
        match clip_stem(src) {
            stem if JUMPS.contains(&stem) => Self::Jump,
            stem if DOUBLE_JUMPS.contains(&stem) => Self::DoubleJump,
            "Gliding_AvatarStartRig" => Self::GlideOpening,
            "Gliding_AvatarForwardRig" | "Gliding_AvatarIdleRig" => Self::Gliding,
            "Gliding_AvatarEndRig"
            | "Gliding_Idle_Jump_EndRig"
            | "Gliding_Jog_Jump_EndRig"
            | "Gliding_Run_Jump_EndRig" => Self::GlideClosing,
            _ => Self::None,
        }
    }

    /// What the path of a clip of any other scene says: `glide` in it, in any case, is the glide,
    /// and `double` the air jump. Nothing else in it counts.
    pub fn of_other_scene_clip(src: &str) -> Self {
        if contains_ignore_case(src, "glide") {
            Self::Gliding
        } else if contains_ignore_case(src, "double") {
            Self::DoubleJump
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
            Self::None | Self::GlideClosing => 0,
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
            ("Jump_Rise_Loop.glb", ClipMotion::Jump),
            ("Jump_Mid.glb", ClipMotion::Jump),
            ("Jump_Fall_Loop.glb", ClipMotion::None),
            ("Idle_Jump_End.glb", ClipMotion::None),
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
            ("Gliding_Idle_Jump_EndRig.glb", ClipMotion::GlideClosing),
            ("Gliding_Jog_Jump_EndRig.glb", ClipMotion::GlideClosing),
            ("Gliding_Run_Jump_EndRig.glb", ClipMotion::GlideClosing),
            // a hard landing is no motion the clip names
            ("Hard_Landing.glb", ClipMotion::None),
            ("Hard_Landing_Enter.glb", ClipMotion::None),
            ("Hard_Landing_Loop.glb", ClipMotion::None),
            ("Hard_Landing_Exit.glb", ClipMotion::None),
        ] {
            assert_eq!(
                ClipMotion::of_clip(&format!("{anim}{clip}")),
                motion,
                "{clip}"
            );
        }
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
            assert_eq!(ClipMotion::of_clip(clip), motion, "{clip}");
        }
    }

    #[test]
    fn another_scenes_clip_names_only_its_air_jump_and_glide() {
        for (clip, motion) in [
            ("assets/MyDoubleJump.glb", ClipMotion::DoubleJump),
            ("DOUBLE.glb", ClipMotion::DoubleJump),
            ("glide.glb", ClipMotion::Gliding),
            ("anims/Player_Glide_Double.glb", ClipMotion::Gliding),
            ("custom_jump.glb", ClipMotion::None),
            ("Jump_Start.glb", ClipMotion::None),
            ("Gliding_AvatarForwardRig.glb", ClipMotion::None),
            ("Hard_Landing_Enter.glb", ClipMotion::None),
            ("gl", ClipMotion::None),
        ] {
            assert_eq!(ClipMotion::of_other_scene_clip(clip), motion, "{clip}");
        }
    }
}
