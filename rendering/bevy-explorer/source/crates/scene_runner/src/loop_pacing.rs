//! Where the scene loop ends each frame.
//!
//! [`grid_deadline`] keeps the deadline on a grid of frame targets: after a long frame the loop
//! gets what is left of the grid, down to a 1 ms floor, and keeps the loss for the next frames
//! (B_n = clamp(B_{n-1} + T - I_n, 1 ms, T)). Native builds always use it. The web adds the
//! deadline mode and the scene loop's waits for replies (`web`); only the wasm build reads the
//! scene loop mode.

use std::time::Duration;

use web_time::Instant;

#[cfg(any(target_arch = "wasm32", test))]
mod web;
#[cfg(target_arch = "wasm32")]
pub(crate) use web::{web_frame_start, WebLoopPacing};

/// The scene loop always runs at least this long.
pub const LOOP_FLOOR: Duration = Duration::from_millis(1);

/// The deadline on the grid of frame targets (`next_frame_end`), bounded so a loop neither burns
/// through a backlog after a hitch (a deadline already in the past still leaves the floor) nor
/// runs longer than a frame target when the deadline is stale (an fps change, an uncapped frame).
/// Returns the deadline and the next frame's grid point.
pub fn grid_deadline(
    next_frame_end: Instant,
    loop_start: Instant,
    frame: Duration,
) -> (Instant, Instant) {
    let earliest_end = loop_start + LOOP_FLOOR;
    let latest_end = loop_start + frame.max(LOOP_FLOOR);
    let target_end_time = next_frame_end.clamp(earliest_end, latest_end);
    (target_end_time, target_end_time + frame)
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
