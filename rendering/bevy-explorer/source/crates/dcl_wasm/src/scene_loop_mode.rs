//! The web scene loop's scheduling mode. `both`, the default, wakes a sandbox worker's relay when
//! the engine sends its scene a tick (`inner::relay`) instead of on its 1 ms poll, and ends the
//! engine's scene loop from the frame's start (scene_runner `loop_pacing`). For diagnostics,
//! `relay` and `deadline` do one of the two and `legacy` neither.
//! Only the relays of the scenes the engine wakes ([`wakes_scene`]: the scenes the player is in,
//! portable experiences among them) wait for its send; any other scene's relay keeps the poll.
//! Set from the page (`?sceneLoop=`) or the console (`/scene_loop_mode`); the relays read it
//! from engine memory each step, so a change reaches every scene worker without a message.
//! Outside the wasm-only module so the host tests cover the names; nothing reads it natively.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SceneLoopMode {
    Legacy,
    Relay,
    Deadline,
    #[default]
    Both,
}

impl SceneLoopMode {
    pub const NAMES: &'static str = "legacy|relay|deadline|both";

    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "legacy" => Some(Self::Legacy),
            "relay" => Some(Self::Relay),
            "deadline" => Some(Self::Deadline),
            "both" => Some(Self::Both),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::Relay => "relay",
            Self::Deadline => "deadline",
            Self::Both => "both",
        }
    }

    /// The relay waits for the engine's tick instead of sleeping.
    pub fn relay_wake(self) -> bool {
        matches!(self, Self::Relay | Self::Both)
    }

    /// The scene loop's deadline comes from the frame's start.
    pub fn frame_deadline(self) -> bool {
        matches!(self, Self::Deadline | Self::Both)
    }

    pub fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Relay,
            2 => Self::Deadline,
            3 => Self::Both,
            _ => Self::Legacy,
        }
    }

    pub const fn to_u8(self) -> u8 {
        match self {
            Self::Legacy => 0,
            Self::Relay => 1,
            Self::Deadline => 2,
            Self::Both => 3,
        }
    }
}

// starts as the default, `both`
#[cfg(target_arch = "wasm32")]
static MODE: std::sync::atomic::AtomicU8 =
    std::sync::atomic::AtomicU8::new(SceneLoopMode::Both.to_u8());

#[cfg(target_arch = "wasm32")]
pub fn get() -> SceneLoopMode {
    SceneLoopMode::from_u8(MODE.load(std::sync::atomic::Ordering::Relaxed))
}

#[cfg(target_arch = "wasm32")]
pub fn set(mode: SceneLoopMode) {
    MODE.store(mode.to_u8(), std::sync::atomic::Ordering::Relaxed);
}

// The scenes whose relays wait for the engine's send in a mode that wakes relays (their scene
// ids' bits), as the engine last published them. Neither side waits for the lock: a busy lock
// leaves the engine's set for its next frame, and a relay's answer as it was.
static WAKE_SCENES: std::sync::Mutex<Vec<u64>> = std::sync::Mutex::new(Vec::new());

/// Engine, each frame: the scenes whose relays wake on its send (scene_runner `wake_scenes`: the
/// scene under the player, portable experiences and the named priority scenes).
pub fn set_wake_scenes(scenes: impl IntoIterator<Item = u64>) {
    if let Ok(mut wake) = WAKE_SCENES.try_lock() {
        wake.clear();
        wake.extend(scenes);
    }
}

/// Relay: whether the engine wakes its scene; None while the engine is publishing.
pub fn wakes_scene(scene: u64) -> Option<bool> {
    WAKE_SCENES
        .try_lock()
        .ok()
        .map(|wake| wake.contains(&scene))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mode_reads_back_by_name_and_number() {
        for mode in [
            SceneLoopMode::Legacy,
            SceneLoopMode::Relay,
            SceneLoopMode::Deadline,
            SceneLoopMode::Both,
        ] {
            assert_eq!(SceneLoopMode::parse(mode.name()), Some(mode));
            assert_eq!(SceneLoopMode::from_u8(mode.to_u8()), mode);
        }
        assert_eq!(SceneLoopMode::parse("frame"), None);
        assert_eq!(SceneLoopMode::from_u8(9), SceneLoopMode::Legacy);
        assert!(SceneLoopMode::Both.relay_wake() && SceneLoopMode::Both.frame_deadline());
        assert!(!SceneLoopMode::Legacy.relay_wake() && !SceneLoopMode::Legacy.frame_deadline());
        assert_eq!(SceneLoopMode::default(), SceneLoopMode::Both);
    }

    #[test]
    fn only_the_published_scenes_wake() {
        set_wake_scenes([3, 7]);
        assert_eq!(wakes_scene(7), Some(true));
        assert_eq!(wakes_scene(5), Some(false));
        set_wake_scenes([5]);
        assert_eq!(wakes_scene(7), Some(false));
        assert_eq!(wakes_scene(5), Some(true));
    }
}
