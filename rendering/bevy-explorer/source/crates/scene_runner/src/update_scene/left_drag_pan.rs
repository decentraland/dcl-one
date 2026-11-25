//! Holding the pointer button and moving turns the camera, when the player's Left-Drag Camera
//! setting (`AppConfig::left_drag_camera`) is on. The drag holds the same `"camera"` cursor lock
//! the right button does.
//!
//! A press that the setting, the cursor position and the frame's state accept arms the pan; once
//! the pointer motion since the press passes [`START_DISTANCE`] it holds the lock until the button
//! is up. An interruption (a dialog, a scene taking the lock, a lost browser lock) is final for
//! that press: the pan does not resume until the button is released and pressed again.

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::structs::AppConfig;
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use serde_json::json;

/// Owns the pan's state and its debug section. Added with the pointer results, which a headless
/// app does not run; without it the pointer lock never pans.
pub(crate) struct LeftDragPanPlugin;

impl Plugin for LeftDragPanPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LeftDragPan>()
            .add_console_command::<LeftDragState, _>(left_drag_state)
            .add_debug_section(DebugSection {
                title: "Left-drag camera",
                query: "/left_drag_state",
                fields: &[
                    ("Setting", "setting"),
                    ("Phase", "phase"),
                    ("Last refusal or interruption", "reason"),
                ],
                inputs: &[],
                actions: &[],
            });
    }
}

/// Read the left-drag camera's setting, phase and last refusal or interruption.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/left_drag_state")]
struct LeftDragState;

fn left_drag_state(
    mut cmd: ConsoleCommand<LeftDragState>,
    config: Res<AppConfig>,
    pan: Res<LeftDragPan>,
) {
    let Some(Ok(_)) = cmd.take() else { return };
    cmd.reply_ok(
        json!({
            "setting": if config.left_drag_camera { "on" } else { "off" },
            "phase": pan.phase().as_str(),
            "reason": pan.last_reason().map(Reason::as_str),
            "unavailable": {"reason": "No press has been refused or interrupted yet."},
        })
        .to_string(),
    );
}

/// The pointer motion since the press that must be exceeded before the pan starts: a still click
/// stays a click. It is in the units of the `POINTER_SET` analog input, which is scaled by the
/// player's pointer sensitivity and also counts key and stick input bound to pointer movement.
pub(crate) const START_DISTANCE: f32 = 1.0;

/// How close to a window edge, in logical pixels, a press may land and still arm the pan.
pub(crate) const EDGE_MARGIN: f32 = 10.0;

/// Why a press did not arm, or why a pan ended before the button went up.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Reason {
    /// The press landed on scene UI.
    OverUi,
    /// The press landed within [`EDGE_MARGIN`] of a window edge.
    NearEdge,
    /// The window has no cursor position.
    NoCursor,
    /// A dialog or HUD surface is open.
    Dialog,
    /// The right button, held or toggled on, or a scene already holds the camera.
    CameraLocked,
    /// A scene camera is active.
    Cinematic,
    /// Photo mode owns the camera.
    PhotoMode,
    /// The scene captured the press for `PetDrag` or `PetDragLocked`.
    SceneDrag,
    /// The window or page lost focus.
    NoFocus,
    /// The browser dropped the pointer lock it granted during the pan, or refused it for a press
    /// made with a mouse button or key. A refusal for a gamepad press is not one.
    LockLost,
    /// A scene changed the pointer lock.
    SceneLock,
}

impl Reason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Reason::OverUi => "press over scene UI",
            Reason::NearEdge => "press near a window edge",
            Reason::NoCursor => "no cursor position",
            Reason::Dialog => "dialog or HUD surface open",
            Reason::CameraLocked => "camera already locked",
            Reason::Cinematic => "scene camera active",
            Reason::PhotoMode => "photo mode",
            Reason::SceneDrag => "scene drag captured the press",
            Reason::NoFocus => "window focus lost",
            Reason::LockLost => "browser lock dropped, or refused for a mouse or key press",
            Reason::SceneLock => "scene changed the pointer lock",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub(crate) enum Phase {
    /// No press is being tracked.
    #[default]
    Idle,
    /// Pressed and accepted; `motion` is the movement since the press frame.
    Armed { motion: Vec2 },
    /// Holding the camera lock.
    Panning,
    /// The press was refused or interrupted; nothing happens until the button is up.
    Spent,
}

impl Phase {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Phase::Idle => "idle",
            Phase::Armed { .. } => "armed",
            Phase::Panning => "panning",
            Phase::Spent => "spent",
        }
    }
}

/// One frame's facts for [`LeftDragPan::tick`].
#[derive(Default, Clone, Copy, Debug)]
pub(crate) struct Frame {
    /// The pointer action went down this frame, at the level scenes receive it.
    pub(crate) press: bool,
    /// The pointer action is down, at that level.
    pub(crate) held: bool,
    /// The pointer motion this frame, in the units of [`START_DISTANCE`].
    pub(crate) motion: Vec2,
    /// The Left-Drag Camera setting is on. Read on the press frame only.
    pub(crate) enabled: bool,
    /// A mouse button or key went down with the press, which gives the page the user activation
    /// a browser pointer lock needs. False for a gamepad press. Read on the press frame only.
    pub(crate) press_activates: bool,
    /// Why this press cannot arm. Sampled on the press frame only.
    pub(crate) press_refusal: Option<Reason>,
    /// Why the pan cannot hold the camera now. Sampled every frame.
    pub(crate) blocked: Option<Reason>,
}

#[derive(Resource, Default, Debug)]
pub(crate) struct LeftDragPan {
    phase: Phase,
    last_reason: Option<Reason>,
    /// The browser granted the pointer lock during this pan.
    lock_granted: bool,
    /// The press that armed this pan gave the page user activation (see `Frame::press_activates`).
    press_activates: bool,
}

impl LeftDragPan {
    pub(crate) fn phase(&self) -> Phase {
        self.phase
    }

    pub(crate) fn is_panning(&self) -> bool {
        self.phase == Phase::Panning
    }

    /// The reason for the last refusal or interruption.
    pub(crate) fn last_reason(&self) -> Option<Reason> {
        self.last_reason
    }

    /// Reads the browser pointer lock (`None` natively) and returns whether a running pan lost
    /// it: the lock was granted during this pan and is now gone, or the press gave user activation
    /// and the browser still refused it. A gamepad press gives the page no user activation, so the
    /// browser always refuses its lock; that refusal is not a loss, and the pan turns the camera
    /// without the lock.
    pub(crate) fn lock_lost(&mut self, locked: Option<bool>) -> bool {
        if !self.is_panning() {
            self.lock_granted = false;
            return false;
        }
        match locked {
            Some(true) => {
                self.lock_granted = true;
                false
            }
            Some(false) => self.lock_granted || self.press_activates,
            None => false,
        }
    }

    /// Ends an armed or running pan for good: it does not resume for this press.
    pub(crate) fn interrupt(&mut self, reason: Reason) {
        if matches!(self.phase, Phase::Armed { .. } | Phase::Panning) {
            self.phase = Phase::Spent;
            self.last_reason = Some(reason);
        }
    }

    /// Advances one frame; returns whether the pan holds the camera lock.
    pub(crate) fn tick(&mut self, frame: &Frame) -> bool {
        // a missing release edge (the button let go outside the window) ends the press too
        if !frame.held {
            self.phase = Phase::Idle;
            return false;
        }

        match self.phase {
            Phase::Idle => {
                if frame.press && frame.enabled {
                    match frame.press_refusal.or(frame.blocked) {
                        Some(reason) => {
                            self.phase = Phase::Spent;
                            self.last_reason = Some(reason);
                        }
                        // the press frame's own motion is not counted
                        None => {
                            self.phase = Phase::Armed { motion: Vec2::ZERO };
                            self.press_activates = frame.press_activates;
                        }
                    }
                }
                false
            }
            Phase::Armed { motion } => {
                if let Some(reason) = frame.blocked {
                    self.interrupt(reason);
                    return false;
                }
                let motion = motion + frame.motion;
                if motion.length_squared() > START_DISTANCE * START_DISTANCE {
                    self.phase = Phase::Panning;
                    true
                } else {
                    self.phase = Phase::Armed { motion };
                    false
                }
            }
            Phase::Panning => {
                if let Some(reason) = frame.blocked {
                    self.interrupt(reason);
                    false
                } else {
                    true
                }
            }
            Phase::Spent => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press() -> Frame {
        Frame {
            press: true,
            held: true,
            enabled: true,
            ..Default::default()
        }
    }

    fn held(motion: Vec2) -> Frame {
        Frame {
            held: true,
            motion,
            ..Default::default()
        }
    }

    fn up() -> Frame {
        Frame::default()
    }

    fn armed() -> LeftDragPan {
        let mut pan = LeftDragPan::default();
        assert!(!pan.tick(&press()));
        assert_eq!(pan.phase(), Phase::Armed { motion: Vec2::ZERO });
        pan
    }

    fn panning() -> LeftDragPan {
        let mut pan = armed();
        assert!(pan.tick(&held(Vec2::new(5.0, 0.0))));
        pan
    }

    #[test]
    fn a_drag_pans_until_release() {
        let mut pan = armed();
        assert!(pan.tick(&held(Vec2::new(3.0, 0.0))));
        assert!(pan.tick(&held(Vec2::ZERO)));
        assert!(!pan.tick(&up()));
        assert_eq!(pan.phase(), Phase::Idle);
    }

    #[test]
    fn motion_at_the_threshold_does_not_start() {
        let mut pan = armed();
        assert!(!pan.tick(&held(Vec2::new(1.0, 0.0))));
        assert!(matches!(pan.phase(), Phase::Armed { .. }));
        assert!(!pan.tick(&held(Vec2::ZERO)));
        assert!(matches!(pan.phase(), Phase::Armed { .. }));
    }

    #[test]
    fn a_click_without_motion_does_nothing() {
        let mut pan = armed();
        assert!(!pan.tick(&held(Vec2::ZERO)));
        assert!(!pan.tick(&up()));
        assert_eq!(pan.phase(), Phase::Idle);
        assert_eq!(pan.last_reason(), None);
    }

    #[test]
    fn motion_before_the_press_does_not_count() {
        let mut pan = LeftDragPan::default();
        let mut frame = press();
        frame.motion = Vec2::new(50.0, 0.0);
        assert!(!pan.tick(&frame));
        assert_eq!(pan.phase(), Phase::Armed { motion: Vec2::ZERO });
        assert!(!pan.tick(&held(Vec2::ZERO)));
    }

    #[test]
    fn a_slow_drag_still_starts() {
        let mut pan = armed();
        let step = Vec2::new(0.3, 0.0);
        assert!(!pan.tick(&held(step)));
        assert!(!pan.tick(&held(step)));
        assert!(!pan.tick(&held(step)));
        assert!(pan.tick(&held(step)));
    }

    #[test]
    fn the_setting_off_never_arms() {
        let mut pan = LeftDragPan::default();
        let mut frame = press();
        frame.enabled = false;
        assert!(!pan.tick(&frame));
        assert!(!pan.tick(&held(Vec2::new(9.0, 0.0))));
        assert_eq!(pan.phase(), Phase::Idle);
    }

    #[test]
    fn a_press_over_ui_never_pans() {
        let mut pan = LeftDragPan::default();
        let mut frame = press();
        frame.press_refusal = Some(Reason::OverUi);
        assert!(!pan.tick(&frame));
        assert_eq!(pan.phase(), Phase::Spent);
        assert!(!pan.tick(&held(Vec2::new(9.0, 0.0))));
        assert_eq!(pan.last_reason(), Some(Reason::OverUi));
        // a new press after the release is judged afresh
        assert!(!pan.tick(&up()));
        assert!(!pan.tick(&press()));
        assert!(matches!(pan.phase(), Phase::Armed { .. }));
    }

    #[test]
    fn a_blocked_press_never_arms() {
        for reason in [
            Reason::Dialog,
            Reason::CameraLocked,
            Reason::Cinematic,
            Reason::PhotoMode,
            Reason::NoFocus,
        ] {
            let mut pan = LeftDragPan::default();
            let mut frame = press();
            frame.blocked = Some(reason);
            assert!(!pan.tick(&frame));
            assert_eq!(pan.phase(), Phase::Spent);
            assert!(!pan.tick(&held(Vec2::new(9.0, 0.0))));
            assert_eq!(pan.last_reason(), Some(reason));
        }
    }

    #[test]
    fn a_block_mid_drag_ends_the_pan_for_good() {
        for reason in [
            Reason::Dialog,
            Reason::CameraLocked,
            Reason::Cinematic,
            Reason::PhotoMode,
            Reason::SceneDrag,
            Reason::NoFocus,
            Reason::LockLost,
        ] {
            for mut pan in [panning(), armed()] {
                let mut frame = held(Vec2::new(9.0, 0.0));
                frame.blocked = Some(reason);
                assert!(!pan.tick(&frame));
                assert_eq!(pan.phase(), Phase::Spent);
                // the cause going away with the button still down does not resume
                assert!(!pan.tick(&held(Vec2::new(9.0, 0.0))));
                assert_eq!(pan.last_reason(), Some(reason));
                assert!(!pan.tick(&up()));
                assert_eq!(pan.phase(), Phase::Idle);
            }
        }
    }

    #[test]
    fn an_interrupt_spends_a_running_pan_only() {
        let mut pan = LeftDragPan::default();
        pan.interrupt(Reason::SceneLock);
        assert_eq!(pan.phase(), Phase::Idle);
        let mut pan = panning();
        pan.interrupt(Reason::SceneLock);
        assert!(!pan.tick(&held(Vec2::new(9.0, 0.0))));
        assert_eq!(pan.last_reason(), Some(Reason::SceneLock));
    }

    #[test]
    fn a_held_frame_without_a_press_edge_does_not_pan() {
        let mut pan = panning();
        assert!(!pan.tick(&up()));
        assert!(!pan.tick(&held(Vec2::new(9.0, 0.0))));
        assert_eq!(pan.phase(), Phase::Idle);
    }

    #[test]
    fn a_lock_refused_for_a_pad_press_is_not_lost() {
        // `panning()` presses with no mouse button or key, as a pad does
        let mut pan = panning();
        assert!(!pan.lock_lost(Some(false)));
        assert!(!pan.lock_lost(Some(false)));
        assert!(pan.tick(&held(Vec2::ZERO)));
    }

    #[test]
    fn a_lock_refused_for_a_mouse_press_is_lost() {
        let mut pan = LeftDragPan::default();
        let mut frame = press();
        frame.press_activates = true;
        assert!(!pan.tick(&frame));
        assert!(pan.tick(&held(Vec2::new(5.0, 0.0))));
        assert!(pan.lock_lost(Some(false)));
        let mut frame = held(Vec2::ZERO);
        frame.blocked = Some(Reason::LockLost);
        assert!(!pan.tick(&frame));
        assert_eq!(pan.last_reason(), Some(Reason::LockLost));
    }

    #[test]
    fn a_granted_lock_that_drops_is_lost() {
        let mut pan = panning();
        assert!(!pan.lock_lost(Some(true)));
        assert!(pan.lock_lost(Some(false)));
    }

    #[test]
    fn a_grant_does_not_carry_over_to_the_next_pan() {
        let mut pan = panning();
        assert!(!pan.lock_lost(Some(true)));
        assert!(!pan.tick(&up()));
        assert!(!pan.lock_lost(Some(false)));
        assert!(!pan.tick(&press()));
        assert!(pan.tick(&held(Vec2::new(9.0, 0.0))));
        assert!(
            !pan.lock_lost(Some(false)),
            "this pan was never granted the lock"
        );
    }

    #[test]
    fn natively_the_lock_is_never_lost() {
        let mut pan = panning();
        assert!(!pan.lock_lost(None));
    }

    #[test]
    fn a_new_press_pans_again_after_an_interruption() {
        let mut pan = panning();
        let mut frame = held(Vec2::ZERO);
        frame.blocked = Some(Reason::NoFocus);
        assert!(!pan.tick(&frame));
        assert!(!pan.tick(&up()));
        assert!(!pan.tick(&press()));
        assert!(pan.tick(&held(Vec2::new(9.0, 0.0))));
    }

    #[test]
    fn the_setting_is_read_on_the_press_only() {
        // a setting that turns on mid-press does not arm the press in progress
        let mut pan = LeftDragPan::default();
        let mut frame = press();
        frame.enabled = false;
        assert!(!pan.tick(&frame));
        let mut frame = held(Vec2::new(9.0, 0.0));
        frame.enabled = true;
        assert!(!pan.tick(&frame));
        assert_eq!(pan.phase(), Phase::Idle);
        // and a setting that turns off mid-pan does not end it
        let mut pan = panning();
        let mut frame = held(Vec2::ZERO);
        frame.enabled = false;
        assert!(pan.tick(&frame));
    }
}
