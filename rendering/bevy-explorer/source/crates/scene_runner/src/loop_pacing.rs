//! Where the scene loop ends each frame.
//!
//! The native rule ([`legacy_deadline`]) keeps the deadline on a grid of frame targets: after a
//! long frame the loop gets what is left of the grid, down to a 1 ms floor, and keeps the loss
//! for the next frames (B_n = clamp(B_{n-1} + T - I_n, 1 ms, T)). On the web, the deadline mode
//! ([`frame_deadline`], scene loop mode `both`, the default, or `deadline`,
//! `dcl_wasm::scene_loop_mode`) instead ends the loop a frame target after the frame's start, less what the frame still needs
//! after the loop: the 90th percentile of that tail over the last frames ([`TailEstimate`]) plus
//! a share of the frame target. The tail is the frame's work, without its waits for the render worker
//! ([`frame_tail`]). Natively the scene loop is unchanged; only the wasm build reads the mode.

use std::time::Duration;

use web_time::Instant;

/// Frames the tail estimate covers. It is used once they are all measured, so its 90th
/// percentile always leaves out the 3 largest: with fewer frames the nearest-rank 90th percentile
/// is the maximum, and a few heavy frames at the start would hold the loop on its floor.
pub const TAIL_FRAMES: usize = 32;
/// Added to the tail's 90th percentile, as a share of the frame target: slack for a tail beyond
/// it that grows with the frame. (A late timer at the loop's end needs none: the loop ends its
/// timed waits before the deadline by their lateness.)
pub const TAIL_MARGIN_SHARE: f64 = 0.06;
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

    /// The nearest-rank 90th percentile, once [`TAIL_FRAMES`] frames were measured.
    pub fn p90(&self) -> Option<Duration> {
        if self.len < TAIL_FRAMES {
            return None;
        }
        let mut sorted = self.ring;
        Some(p90(&mut sorted[..self.len]))
    }

    /// What the deadline mode leaves for the tail: its 90th percentile plus
    /// [`TAIL_MARGIN_SHARE`] of the frame target.
    pub fn reserve(&self, frame: Duration) -> Option<Duration> {
        self.p90().map(|p90| p90 + frame.mul_f64(TAIL_MARGIN_SHARE))
    }
}

// the nearest-rank 90th percentile of a non-empty set
fn p90(values: &mut [Duration]) -> Duration {
    values.sort_unstable();
    values[(values.len() * 9).div_ceil(10) - 1]
}

/// Timed-out waits for a reply the lateness estimate covers.
pub const LATENESS_WAITS: usize = 32;
/// How long a timed-out wait's lateness counts. A wait is timed only while the margin leaves room
/// for one, so without this a margin at or above the loop's budget, after a burst of late wakes
/// (a background tab's coarse timers, a busy start), would stop the waits that measure it and
/// keep the loop spinning through its whole budget for the rest of the session.
pub const LATENESS_AGE: Duration = Duration::from_secs(1);

/// How much later than asked the scene loop's last [`LATENESS_WAITS`] timed-out waits for a reply
/// woke (web), each counted for [`LATENESS_AGE`]. No timed wait ends closer to the deadline than
/// their 90th percentile ([`WaitLateness::wait_end`]).
#[derive(Clone, Debug, Default)]
pub struct WaitLateness {
    ring: [Option<(Instant, Duration)>; LATENESS_WAITS],
    next: usize,
}

impl WaitLateness {
    /// A wait that timed out at `at`, `late` after it was due.
    pub fn push(&mut self, at: Instant, late: Duration) {
        self.ring[self.next] = Some((at, late));
        self.next = (self.next + 1) % LATENESS_WAITS;
    }

    /// The nearest-rank 90th percentile of the waits at most [`LATENESS_AGE`] before `now`; None
    /// without one. A few waits give their maximum: a margin too large spins longer, one too
    /// small ends the loop late, and either lasts at most [`LATENESS_AGE`].
    pub fn p90(&self, now: Instant) -> Option<Duration> {
        let mut recent = [Duration::ZERO; LATENESS_WAITS];
        let mut len = 0;
        for (at, late) in self.ring.iter().flatten() {
            if now.saturating_duration_since(*at) <= LATENESS_AGE {
                recent[len] = *late;
                len += 1;
            }
        }
        (len > 0).then(|| p90(&mut recent[..len]))
    }

    /// The last moment a timed wait of a loop that starts at `loop_start` may end: `deadline` less
    /// the 90th percentile, and not before the loop's start (then no wait is timed).
    pub fn wait_end(&self, deadline: Instant, loop_start: Instant) -> Instant {
        deadline
            .checked_sub(self.p90(loop_start).unwrap_or_default())
            .unwrap_or(loop_start)
            .max(loop_start)
    }
}

/// The web scene loop's frame: its start, the loop's last end, and the tail estimate.
#[cfg(target_arch = "wasm32")]
#[derive(bevy::prelude::Resource, Default)]
pub struct WebLoopPacing {
    pub frame_start: Option<Instant>,
    pub loop_end: Option<Instant>,
    pub tail: TailEstimate,
    pub wait_lateness: WaitLateness,
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
        for _ in 0..TAIL_FRAMES - 1 {
            tail.push(ms(3.0));
        }
        assert_eq!(tail.reserve(T), None);
        tail.push(ms(3.0));
        // 6 % of the frame target: 1 ms at 60 fps, 0.5 ms at 120 fps
        assert!(close(tail.reserve(T).unwrap(), ms(4.0)));
        assert!(close(tail.reserve(T / 2).unwrap(), ms(3.5)));
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

    /// The first estimate covers a full window: three heavy frames at the start do not reach the
    /// reserve, which would hold the loop on its floor.
    #[test]
    fn heavy_frames_at_the_start_do_not_set_the_reserve() {
        let mut tail = TailEstimate::default();
        for _ in 0..3 {
            tail.push(ms(30.0));
        }
        for _ in 3..TAIL_FRAMES - 1 {
            tail.push(ms(2.0));
            assert_eq!(tail.reserve(T), None);
        }
        tail.push(ms(2.0));
        assert!(close(tail.reserve(T).unwrap(), ms(3.0)));
    }

    /// A burst of late wakes leaves the loop no timed wait, so nothing measures the lateness
    /// again; once the burst ages out the loop waits, and a fresh wait sets the margin.
    #[test]
    fn a_burst_of_late_wakes_stops_the_waits_only_until_it_ages_out() {
        let burst = Instant::now();
        let mut lateness = WaitLateness::default();
        assert_eq!(lateness.p90(burst), None);
        // a background tab: every timed wait wakes 10 ms late
        for _ in 0..LATENESS_WAITS {
            lateness.push(burst, ms(10.0));
        }
        // back in front, 8 ms budgets: the margin leaves no timed wait, frame after frame
        let mut loop_start = burst;
        while loop_start - burst <= LATENESS_AGE {
            assert_eq!(lateness.wait_end(loop_start + ms(8.0), loop_start), loop_start);
            loop_start += T;
        }
        // the burst aged out: the loop waits up to its deadline, and its first timed-out wait
        // gives the margin
        assert_eq!(lateness.p90(loop_start), None);
        assert_eq!(
            lateness.wait_end(loop_start + ms(8.0), loop_start),
            loop_start + ms(8.0)
        );
        lateness.push(loop_start + ms(1.0), ms(1.0));
        loop_start += T;
        assert!(close(
            lateness.wait_end(loop_start + ms(8.0), loop_start) - loop_start,
            ms(7.0)
        ));
    }

    #[test]
    fn the_wait_lateness_is_the_90th_percentile_of_the_recent_waits() {
        let start = Instant::now();
        let mut lateness = WaitLateness::default();
        // a few waits give their maximum
        lateness.push(start, ms(0.2));
        lateness.push(start, ms(1.5));
        assert_eq!(lateness.p90(start), Some(ms(1.5)));
        // 1..=32 ms: rank ceil(0.9 * 32) = 29; the window holds the last 32
        for i in 1..=LATENESS_WAITS {
            lateness.push(start, ms(i as f64));
        }
        assert!(close(lateness.p90(start).unwrap(), ms(29.0)));
        // only the waits within the age count
        let later = start + LATENESS_AGE / 2;
        for _ in 0..3 {
            lateness.push(later, ms(1.0));
        }
        let now = start + LATENESS_AGE + ms(1.0);
        assert_eq!(lateness.p90(now), Some(ms(1.0)));
    }
}
