//! The web scene loop's scheduling mode. `both`, the default, wakes a sandbox worker's relay when
//! the engine sends its scene a tick (`inner::relay`) instead of on its 1 ms poll, and ends the
//! engine's scene loop from the frame's start (scene_runner `loop_pacing`). For diagnostics,
//! `relay` and `deadline` do one of the two and `legacy` neither. Every mode but `legacy` also
//! ends the scene loop's wait for replies when one arrives (`scene_replies`).
//! Only the relays of the scenes the engine wakes (each scene's `dcl_types::SceneWake`: the scene
//! the player is in, portable experiences and the named priority scenes) wait for its send; any
//! other scene's relay keeps the poll.
//! Set from the page (`?sceneLoop=`) or, without a reload, the debug panel (scene_runner
//! `/scene_loop_mode`); the engine's scene loop reads it each frame and the relays from engine
//! memory each step, so a change reaches every scene worker without a message. Outside the
//! wasm-only module so the host tests cover the names; nothing reads it natively.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SceneLoopMode {
    Legacy,
    Relay,
    Deadline,
    #[default]
    Both,
}

impl SceneLoopMode {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "legacy" => Some(Self::Legacy),
            "relay" => Some(Self::Relay),
            "deadline" => Some(Self::Deadline),
            "both" => Some(Self::Both),
            _ => None,
        }
    }

    /// The page parameter's name for the mode ([`Self::parse`]).
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

    /// The engine's scene loop, with replies outstanding, waits until one arrives
    /// (`scene_replies`) instead of sleeping for a fixed poll. Every mode but `legacy`.
    pub fn loop_waits_for_replies(self) -> bool {
        self != Self::Legacy
    }

    #[cfg(any(target_arch = "wasm32", test))]
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Relay,
            2 => Self::Deadline,
            3 => Self::Both,
            _ => Self::Legacy,
        }
    }

    #[cfg(any(target_arch = "wasm32", test))]
    const fn to_u8(self) -> u8 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mode_reads_back_by_name_and_number() {
        for (name, mode) in [
            ("legacy", SceneLoopMode::Legacy),
            ("relay", SceneLoopMode::Relay),
            ("deadline", SceneLoopMode::Deadline),
            ("both", SceneLoopMode::Both),
        ] {
            assert_eq!(SceneLoopMode::parse(name), Some(mode));
            assert_eq!(mode.name(), name);
            assert_eq!(SceneLoopMode::from_u8(mode.to_u8()), mode);
        }
        assert_eq!(SceneLoopMode::parse("frame"), None);
        assert_eq!(SceneLoopMode::from_u8(9), SceneLoopMode::Legacy);
        assert!(SceneLoopMode::Both.relay_wake() && SceneLoopMode::Both.frame_deadline());
        assert!(!SceneLoopMode::Legacy.relay_wake() && !SceneLoopMode::Legacy.frame_deadline());
        assert!(SceneLoopMode::Relay.loop_waits_for_replies());
        assert!(SceneLoopMode::Deadline.loop_waits_for_replies());
        assert!(!SceneLoopMode::Legacy.loop_waits_for_replies());
        assert_eq!(SceneLoopMode::default(), SceneLoopMode::Both);
    }
}
