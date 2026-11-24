//! What the file name of a movement clip that a scene plays says about the avatar's motion.

use crate::structs::MoveKind;

/// The phases of a hard landing's stun, by word of the file name.
const STUN_PHASES: [&str; 3] = ["enter", "loop", "exit"];

/// The lowercase words of a clip's file name, without folder and extension. Words break at
/// anything but a letter or digit, and where a capital follows a lowercase letter or digit:
/// `Gliding_AvatarStartRig` is `gliding avatar start rig`.
fn clip_words(src: &str) -> Vec<String> {
    let file = src.rsplit(['/', '\\']).next().unwrap_or(src);
    let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    let mut words = Vec::new();
    let mut word = String::new();
    let mut after_lower = false;
    for c in stem.chars() {
        let breaks = !c.is_alphanumeric() || (after_lower && c.is_uppercase());
        if breaks && !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
        if c.is_alphanumeric() {
            word.extend(c.to_lowercase());
        }
        after_lower = c.is_lowercase() || c.is_numeric();
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

/// What the file name of a movement clip says about the avatar's motion, read from the words of
/// the name (see [`clip_words`]), so `Avatar_DoubleJump`, `double_jump` and `doublejump` all
/// name the air jump.
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
        Self::of_words(&clip_words(src), speed)
    }

    fn of_words(words: &[String], speed: f32) -> Self {
        let has = |word: &str| words.iter().any(|w| w == word);
        if has("doublejump") || words.windows(2).any(|w| w[0] == "double" && w[1] == "jump") {
            Self::DoubleJump
        } else if words.iter().any(|w| w.starts_with("glid")) {
            if has("start") || has("open") {
                Self::GlideOpening
            } else if has("end") || has("close") || has("landing") {
                Self::GlideClosing
            } else {
                Self::Gliding
            }
        } else if has("jump") && !has("fall") && !has("end") {
            Self::Jump
        } else if speed <= 1.0
            && has("hard")
            && has("landing")
            && STUN_PHASES.iter().any(|phase| has(phase))
        {
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
        let words = clip_words(src);
        match Self::of_words(&words, 1.0) {
            Self::DoubleJump => true,
            Self::Jump => words.iter().any(|w| w == "start"),
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
    fn other_clip_names_match_by_word_whatever_the_case() {
        for (clip, motion) in [
            ("Avatar_DoubleJump.glb", ClipMotion::DoubleJump),
            ("double_jump", ClipMotion::DoubleJump),
            ("a\\doublejump.gltf", ClipMotion::DoubleJump),
            ("DOUBLEJUMP", ClipMotion::DoubleJump),
            ("player_glide.glb", ClipMotion::Gliding),
            ("GLIDE", ClipMotion::Gliding),
            // `end` inside a word closes nothing
            ("Gliding_Descend.glb", ClipMotion::Gliding),
            ("Glider_Open.glb", ClipMotion::GlideOpening),
            ("Glider_Close.glb", ClipMotion::GlideClosing),
            ("avatar_jump.glb", ClipMotion::Jump),
            ("Jump_Fall.glb", ClipMotion::None),
            ("Doubled.glb", ClipMotion::None),
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
            ("JumpStart.glb", true),
            ("DoubleJump_Base.glb", true),
            ("DoubleJump_Jog_Right.glb", true),
            ("Jump_Rise_Loop.glb", false),
            ("Run_Jump_Mid.glb", false),
            ("Jump_Fall_Loop.glb", false),
            ("Jog_Jump_End.glb", false),
            // `start` inside a word starts nothing
            ("Jump_Restart.glb", false),
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
