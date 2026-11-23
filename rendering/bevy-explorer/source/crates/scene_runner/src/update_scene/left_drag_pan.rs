//! Holding the pointer button and moving turns the camera, behind the `bevy-left-drag-camera`
//! flag. The drag holds the same `"camera"` cursor lock the right button does.
//!
//! A press that the flag, the cursor position and the frame's state accept arms the pan; once the
//! motion since the press passes [`START_DISTANCE`] it holds the lock until the button is up. An
//! interruption (a dialog, a scene taking the lock, a lost browser lock) is final for that press:
//! the pan does not resume until the button is released and pressed again.

use bevy::prelude::*;

/// The flag that enables the pan.
pub const FLAG: &str = "bevy-left-drag-camera";

/// The motion since the press, in pixels, that must be exceeded before the pan starts: a still
/// click stays a click.
pub const START_DISTANCE: f32 = 1.0;

/// How close to a window edge, in logical pixels, a press may land and still arm the pan.
pub const EDGE_MARGIN: f32 = 10.0;

/// Why a press did not arm, or why a pan ended before the button went up.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reason {
    /// The press landed on scene UI.
    OverUi,
    /// The press landed within [`EDGE_MARGIN`] of a window edge.
    NearEdge,
    /// The window has no cursor position.
    NoCursor,
    /// A dialog or HUD surface is open.
    Dialog,
    /// The right-click toggle or a scene already holds the camera.
    CameraLocked,
    /// A scene camera is active.
    Cinematic,
    /// Photo mode owns the camera.
    PhotoMode,
    /// The scene captured the press for `PetDrag` or `PetDragLocked`.
    SceneDrag,
    /// The window or page lost focus.
    NoFocus,
    /// The browser dropped or refused the pointer lock.
    LockLost,
    /// A scene changed the pointer lock.
    SceneLock,
}

impl Reason {
    pub fn as_str(self) -> &'static str {
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
            Reason::LockLost => "browser lock dropped or refused",
            Reason::SceneLock => "scene changed the pointer lock",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Phase {
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
    pub fn as_str(self) -> &'static str {
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
pub struct Frame {
    /// The pointer action went down this frame, at the level scenes receive it.
    pub press: bool,
    /// The pointer action is down, at that level.
    pub held: bool,
    /// The pointer's movement this frame, in pixels.
    pub motion: Vec2,
    /// The flag is on. Read on the press frame only.
    pub enabled: bool,
    /// Why this press cannot arm. Sampled on the press frame only.
    pub press_refusal: Option<Reason>,
    /// Why the pan cannot hold the camera now. Sampled every frame.
    pub blocked: Option<Reason>,
}

#[derive(Resource, Default, Debug)]
pub struct LeftDragPan {
    phase: Phase,
    last_reason: Option<Reason>,
}

impl LeftDragPan {
    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn is_panning(&self) -> bool {
        self.phase == Phase::Panning
    }

    /// The reason for the last refusal or interruption.
    pub fn last_reason(&self) -> Option<Reason> {
        self.last_reason
    }

    /// Ends an armed or running pan for good: it does not resume for this press.
    pub fn interrupt(&mut self, reason: Reason) {
        if matches!(self.phase, Phase::Armed { .. } | Phase::Panning) {
            self.phase = Phase::Spent;
            self.last_reason = Some(reason);
        }
    }

    /// Advances one frame; returns whether the pan holds the camera lock.
    pub fn tick(&mut self, frame: &Frame) -> bool {
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
                        None => self.phase = Phase::Armed { motion: Vec2::ZERO },
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
    fn the_flag_off_never_arms() {
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
    fn ui_under_the_cursor_mid_drag_is_ignored() {
        // the press-time facts are not part of later frames, so UI under the locked cursor
        // cannot end a running pan
        let mut pan = panning();
        assert!(pan.tick(&held(Vec2::new(2.0, 2.0))));
        assert!(pan.is_panning());
    }

    #[test]
    fn a_press_near_an_edge_never_pans() {
        for _ in [(); 4] {
            let mut pan = LeftDragPan::default();
            let mut frame = press();
            frame.press_refusal = Some(Reason::NearEdge);
            assert!(!pan.tick(&frame));
            assert!(!pan.tick(&held(Vec2::new(9.0, 9.0))));
            assert_eq!(pan.last_reason(), Some(Reason::NearEdge));
        }
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
    fn a_missing_release_edge_still_ends_the_pan() {
        // no `just_up` reaches us: the button simply reads as not held
        let mut pan = panning();
        assert!(!pan.tick(&up()));
        assert_eq!(pan.phase(), Phase::Idle);
        // and the next held frame without a press edge does not pan
        assert!(!pan.tick(&held(Vec2::new(9.0, 0.0))));
        assert_eq!(pan.phase(), Phase::Idle);
    }

    #[test]
    fn focus_loss_ends_the_pan() {
        let mut pan = panning();
        let mut frame = held(Vec2::ZERO);
        frame.blocked = Some(Reason::NoFocus);
        assert!(!pan.tick(&frame));
        // a new pan needs a new press edge
        assert!(!pan.tick(&held(Vec2::new(9.0, 0.0))));
        assert!(!pan.tick(&up()));
        assert!(!pan.tick(&press()));
        assert!(pan.tick(&held(Vec2::new(9.0, 0.0))));
    }

    #[test]
    fn a_browser_release_ends_the_pan() {
        let mut pan = panning();
        let mut frame = held(Vec2::ZERO);
        frame.blocked = Some(Reason::LockLost);
        assert!(!pan.tick(&frame));
        assert_eq!(pan.phase(), Phase::Spent);
    }

    #[test]
    fn a_refused_lock_ends_the_pan() {
        // the page reports the lock was not granted on the frame after the pan began
        let mut pan = armed();
        assert!(pan.tick(&held(Vec2::new(5.0, 0.0))));
        let mut frame = held(Vec2::new(5.0, 0.0));
        frame.blocked = Some(Reason::LockLost);
        assert!(!pan.tick(&frame));
        assert!(!pan.tick(&held(Vec2::new(5.0, 0.0))));
    }

    #[test]
    fn the_flag_is_read_on_the_press_only() {
        // a flag that turns on mid-press does not arm the press in progress
        let mut pan = LeftDragPan::default();
        let mut frame = press();
        frame.enabled = false;
        assert!(!pan.tick(&frame));
        let mut frame = held(Vec2::new(9.0, 0.0));
        frame.enabled = true;
        assert!(!pan.tick(&frame));
        assert_eq!(pan.phase(), Phase::Idle);
        // and a flag that turns off mid-pan does not end it
        let mut pan = panning();
        let mut frame = held(Vec2::ZERO);
        frame.enabled = false;
        assert!(pan.tick(&frame));
    }
}
