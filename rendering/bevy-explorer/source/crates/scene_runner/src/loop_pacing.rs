//! Where the scene loop ends each frame.
//!
//! The native rule ([`legacy_deadline`]) keeps the deadline on a grid of frame targets: after a
//! long frame the loop gets what is left of the grid, down to a 1 ms floor, and keeps the loss
//! for the next frames (B_n = clamp(B_{n-1} + T - I_n, 1 ms, T)). On the web, the deadline mode
//! ([`frame_deadline`], scene loop mode `both`, the default, or `deadline`,
//! `dcl_wasm::scene_loop_mode`) instead ends the loop a frame target after the frame's start, less what the frame still needs
//! after the loop: the 90th percentile of that tail over the last frames ([`TailEstimate`]) plus
//! a margin. The tail is the frame's work, without its waits for the render worker
//! ([`frame_tail`]). Natively the scene loop is unchanged; only the wasm build reads the mode.

use std::time::Duration;

use web_time::Instant;

/// Frames the tail estimate covers.
pub const TAIL_FRAMES: usize = 32;
/// Frames measured before the estimate is used.
pub const TAIL_MIN_FRAMES: usize = TAIL_FRAMES / 4;
/// Added to the tail's 90th percentile: the frame timer wakes up to a millisecond late.
pub const TAIL_MARGIN: Duration = Duration::from_millis(1);
/// The scene loop always runs at least this long.
pub const LOOP_FLOOR: Duration = Duration::from_millis(1);

/// The current rule: the deadline on the grid of frame targets (`next_frame_end`), bounded so a
/// loop neither burns through a backlog after a hitch (a deadline already in the past still
/// leaves the floor) nor runs longer than a frame target when the deadline is stale (an fps
/// change, an uncapped frame). Returns the deadline and the next frame's grid point.
pub fn legacy_deadline(
    next_frame_end: Instant,
    loop_start: Instant,
    frame: Duration,
) -> (Instant, Instant) {
    let earliest_end = loop_start + LOOP_FLOOR;
    let latest_end = loop_start + frame.max(LOOP_FLOOR);
    let target_end_time = next_frame_end.clamp(earliest_end, latest_end);
    (target_end_time, target_end_time + frame)
}

/// The deadline mode: a frame target after the frame's start, less the frame's tail after the
/// loop, within the same bounds as [`legacy_deadline`].
pub fn frame_deadline(
    frame_start: Instant,
    loop_start: Instant,
    frame: Duration,
    tail: Duration,
) -> Instant {
    let earliest_end = loop_start + LOOP_FLOOR;
    let latest_end = loop_start + frame.max(LOOP_FLOOR);
    (frame_start + frame)
        .checked_sub(tail)
        .unwrap_or(earliest_end)
        .clamp(earliest_end, latest_end)
}

/// The frame's work after the scene loop ([`frame_tail`]: the rest of the frame's schedules and
/// extract), over the last [`TAIL_FRAMES`] frames.
#[derive(Clone, Debug, Default)]
pub struct TailEstimate {
    ring: [Duration; TAIL_FRAMES],
    len: usize,
    next: usize,
}

impl TailEstimate {
    pub fn push(&mut self, tail: Duration) {
        self.ring[self.next] = tail;
        self.next = (self.next + 1) % TAIL_FRAMES;
        self.len = (self.len + 1).min(TAIL_FRAMES);
    }

    /// The nearest-rank 90th percentile, once [`TAIL_MIN_FRAMES`] frames were measured.
    pub fn p90(&self) -> Option<Duration> {
        if self.len < TAIL_MIN_FRAMES {
            return None;
        }
        let mut sorted = self.ring;
        let sorted = &mut sorted[..self.len];
        sorted.sort_unstable();
        Some(sorted[(self.len * 9).div_ceil(10) - 1])
    }

    /// What the deadline mode leaves for the tail: its 90th percentile plus [`TAIL_MARGIN`].
    pub fn reserve(&self) -> Option<Duration> {
        self.p90().map(|p90| p90 + TAIL_MARGIN)
    }
}

/// The web scene loop's frame: its start, the loop's last end, and the tail estimate.
#[cfg(target_arch = "wasm32")]
#[derive(bevy::prelude::Resource, Default)]
pub struct WebLoopPacing {
    pub frame_start: Option<Instant>,
    pub loop_end: Option<Instant>,
    pub tail: TailEstimate,
    /// How much later than asked the loop's last timed-out waits for a reply woke: the same
    /// window and percentile, over waits instead of frames. No timed wait ends closer to the
    /// deadline than its 90th percentile.
    pub wait_lateness: TailEstimate,
}

/// The frame's work after its scene loop: from the loop's end to the handoff to the render
/// worker, less the frame's waits for that worker (`waits`). None for a handoff before the loop's
/// end, an older frame's.
pub fn frame_tail(loop_end: Instant, handoff: Instant, waits: Duration) -> Option<Duration> {
    handoff
        .checked_duration_since(loop_end)
        .map(|tail| tail.saturating_sub(waits))
}

/// `First`: the frame's start, and the previous frame's tail ([`frame_tail`]): the handoff and the
/// waits come from the pipelined-rendering diagnostics, which record them together at the end of
/// the app's update. The waits, for the render app to come back (`WAIT_RENDER`) and for the render
/// worker's animation frame to take the handoff (`WAIT_HANDOFF`), shrink as the loop runs longer,
/// so they are not part of what the loop must leave.
#[cfg(target_arch = "wasm32")]
pub fn web_frame_start(
    mut pacing: bevy::prelude::ResMut<WebLoopPacing>,
    store: Option<bevy::prelude::Res<bevy::diagnostic::DiagnosticsStore>>,
) {
    use bevy::{
        diagnostic::{DiagnosticMeasurement, DiagnosticPath, DiagnosticsStore},
        render::pipelined_rendering::diagnostics::{WAIT_HANDOFF, WAIT_RENDER},
    };

    fn last<'a>(
        store: Option<&'a DiagnosticsStore>,
        path: &DiagnosticPath,
    ) -> Option<&'a DiagnosticMeasurement> {
        store?.get(path)?.measurement()
    }

    let now = Instant::now();
    pacing.frame_start = Some(now);
    let store = store.as_deref();
    let handoff = last(store, &WAIT_HANDOFF)
        // bevy's own Instant: carried over as its age
        .and_then(|measurement| now.checked_sub(measurement.time.elapsed()));
    let waits = [WAIT_RENDER, WAIT_HANDOFF]
        .iter()
        .filter_map(|path| last(store, path))
        .map(|measurement| Duration::from_secs_f64(measurement.value.max(0.0) / 1000.0))
        .sum();
    if let (Some(handoff), Some(loop_end)) = (handoff, pacing.loop_end.take()) {
        if let Some(tail) = frame_tail(loop_end, handoff, waits) {
            pacing.tail.push(tail);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: Duration = Duration::from_micros(16_667);

    fn ms(v: f64) -> Duration {
        Duration::from_secs_f64(v / 1000.0)
    }

    fn close(a: Duration, b: Duration) -> bool {
        a.abs_diff(b) < Duration::from_micros(2)
    }

    /// B_n = clamp(B_{n-1} + T - I_n, 1 ms, T), over a run with long frames and short ones.
    #[test]
    fn the_legacy_budget_carries_each_frames_loss() {
        let mut s = Instant::now();
        let (target, mut next) = legacy_deadline(s, s, T);
        let mut budget = target - s;
        assert_eq!(budget, LOOP_FLOOR);
        for interval in [
            16.7, 16.7, 33.4, 16.7, 16.0, 16.0, 40.0, 10.0, 16.7, 16.7, 16.7, 8.0,
        ] {
            let interval = ms(interval);
            s += interval;
            let (target, n) = legacy_deadline(next, s, T);
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

    /// After a frame twice the target, the legacy budget sits on the floor; the deadline mode
    /// gives the frame what its start and tail leave.
    #[test]
    fn after_a_long_frame_the_deadline_follows_the_frames_start() {
        let s0 = Instant::now();
        let (_, next) = legacy_deadline(s0 + T, s0, T);
        let frame_start = s0 + 2 * T;
        let s = frame_start + ms(4.7);
        let (legacy, _) = legacy_deadline(next, s, T);
        assert_eq!(legacy - s, LOOP_FLOOR);
        let tail = ms(5.0);
        let deadline = frame_deadline(frame_start, s, T, tail);
        assert_eq!(deadline, frame_start + T - tail);
    }

    #[test]
    fn the_deadline_keeps_the_floor_and_the_ceiling() {
        let s = Instant::now();
        // the frame's start and its tail leave less than the floor
        assert_eq!(
            frame_deadline(s, s + ms(10.0), T, ms(8.0)),
            s + ms(10.0) + LOOP_FLOOR
        );
        // a tail longer than the frame
        assert_eq!(
            frame_deadline(s, s + ms(1.0), T, ms(40.0)),
            s + ms(1.0) + LOOP_FLOOR
        );
        // no tail and a loop that starts with the frame: at most a frame target
        assert_eq!(frame_deadline(s, s, T, Duration::ZERO), s + T);
        assert_eq!(frame_deadline(s + ms(3.0), s, T, Duration::ZERO), s + T);
    }

    #[test]
    fn the_tail_leaves_out_the_waits_for_the_render_worker() {
        let loop_end = Instant::now();
        // 3 ms of work after the loop, 6 ms waiting for the animation frame
        let handoff = loop_end + ms(9.0);
        assert_eq!(frame_tail(loop_end, handoff, ms(6.0)), Some(ms(3.0)));
        assert_eq!(
            frame_tail(loop_end, handoff, ms(12.0)),
            Some(Duration::ZERO)
        );
        // the last handoff was an older frame's
        assert_eq!(frame_tail(handoff, loop_end, ms(1.0)), None);
    }

    #[test]
    fn the_tail_estimate_is_the_90th_percentile_of_the_last_frames() {
        let mut tail = TailEstimate::default();
        for _ in 0..TAIL_MIN_FRAMES - 1 {
            tail.push(ms(3.0));
        }
        assert_eq!(tail.reserve(), None);
        tail.push(ms(3.0));
        assert_eq!(tail.reserve(), Some(ms(3.0) + TAIL_MARGIN));
        // 1..=32 ms: rank ceil(0.9 * 32) = 29
        for i in 1..=TAIL_FRAMES {
            tail.push(ms(i as f64));
        }
        assert!(close(tail.p90().unwrap(), ms(29.0)));
        // the oldest frames leave the window
        for _ in 0..TAIL_FRAMES {
            tail.push(ms(2.0));
        }
        assert!(close(tail.p90().unwrap(), ms(2.0)));
    }
}
