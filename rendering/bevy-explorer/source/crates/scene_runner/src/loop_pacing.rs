//! Where the scene loop ends each frame, and how it waits for scene replies.
//!
//! [`grid_deadline`] keeps the deadline on a grid of frame targets: after a long frame the loop
//! gets what is left of the grid, down to a 1 ms floor, and keeps the loss for the next frames
//! (B_n = clamp(B_{n-1} + T - I_n, 1 ms, T)).
//!
//! [`LoopPacing`] paces `run_scene_loop` on this platform, with one API on both:
//! [`LoopPacing::begin`] steps its [`FrameGrid`] to the frame's grid deadline and starts the
//! frame's `PacedLoop` (its deadline, the last moment a wait for replies may end, and that wait),
//! and [`LoopPacing::end`] closes it. Natively (`native`) the loop keeps the grid deadline, waits
//! with the spin sleeper's native sleep and, with a frame cap, sleeps to the deadline after the
//! loop. The web (`web`) adds the deadline mode and the scene loop's waits for replies; only the
//! wasm build reads the scene loop mode. [`LoopPacingPlugin`] sets either up.

use std::time::Duration;

use bevy::platform::time::Instant;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::{LoopPacing, LoopPacingPlugin};
#[cfg(any(target_arch = "wasm32", test))]
mod web;
#[cfg(target_arch = "wasm32")]
pub(crate) use web::{LoopPacing, LoopPacingPlugin};

/// The scene loop always runs at least this long.
pub const LOOP_FLOOR: Duration = Duration::from_millis(1);

/// The grid's step with an uncapped frame rate: long enough that the grid never cuts the loop
/// short.
const UNCAPPED_FRAME: Duration = Duration::from_secs(100);

/// The grid of frame targets the scene loop's deadline keeps ([`grid_deadline`]).
pub struct FrameGrid {
    /// Absolute wall-clock deadline for the end of the next frame. Accumulating this
    /// by the frame target (rather than restarting the clock from `Instant::now()` each
    /// frame) means a frame that overruns is repaid by a correspondingly shorter sleep,
    /// so the long-run rate matches the target instead of losing the overshoot forever.
    next_frame_end: Instant,
}

impl Default for FrameGrid {
    // the first deadline clamps it to the loop's bounds
    fn default() -> Self {
        Self {
            next_frame_end: Instant::now(),
        }
    }
}

impl FrameGrid {
    /// The grid deadline of a loop that starts at `loop_start`; steps the grid to the next
    /// frame's. `frame` is the frame target, None with an uncapped frame rate.
    pub fn advance(&mut self, loop_start: Instant, frame: Option<Duration>) -> Instant {
        let (deadline, next_frame_end) = grid_deadline(
            self.next_frame_end,
            loop_start,
            frame.unwrap_or(UNCAPPED_FRAME),
        );
        self.next_frame_end = next_frame_end;
        deadline
    }
}

/// The deadline on the grid of frame targets (`next_frame_end`), bounded so a loop neither burns
/// through a backlog after a hitch (a deadline already in the past still leaves the floor) nor
/// runs longer than a frame target when the deadline is stale (an fps change, an uncapped frame).
/// Returns the deadline and the next frame's grid point.
pub fn grid_deadline(
    next_frame_end: Instant,
    loop_start: Instant,
    frame: Duration,
) -> (Instant, Instant) {
    let (earliest_end, latest_end) = loop_bounds(loop_start, frame);
    let target_end_time = next_frame_end.clamp(earliest_end, latest_end);
    (target_end_time, target_end_time + frame)
}

/// Where a loop that starts at `loop_start` may end: no sooner than the floor after its start, and
/// no later than a frame target after it.
fn loop_bounds(loop_start: Instant, frame: Duration) -> (Instant, Instant) {
    (loop_start + LOOP_FLOOR, loop_start + frame.max(LOOP_FLOOR))
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) const T: Duration = Duration::from_micros(16_667);

    pub(super) fn ms(v: f64) -> Duration {
        Duration::from_secs_f64(v / 1000.0)
    }

    pub(super) fn close(a: Duration, b: Duration) -> bool {
        a.abs_diff(b) < Duration::from_micros(2)
    }

    /// B_n = clamp(B_{n-1} + T - I_n, 1 ms, T), over a run with long frames and short ones.
    #[test]
    fn the_grid_budget_carries_each_frames_loss() {
        let mut s = Instant::now();
        let (target, mut next) = grid_deadline(s, s, T);
        let mut budget = target - s;
        assert_eq!(budget, LOOP_FLOOR);
        for interval in [
            16.7, 16.7, 33.4, 16.7, 16.0, 16.0, 40.0, 10.0, 16.7, 16.7, 16.7, 8.0,
        ] {
            let interval = ms(interval);
            s += interval;
            let (target, n) = grid_deadline(next, s, T);
            next = n;
            let expected = (budget + T)
                .checked_sub(interval)
                .unwrap_or_default()
                .clamp(LOOP_FLOOR, T);
            let got = target - s;
            assert!(close(got, expected), "B {got:?}, expected {expected:?}");
            budget = got;
        }
    }
}
