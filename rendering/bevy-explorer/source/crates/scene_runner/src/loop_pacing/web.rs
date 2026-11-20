//! The web scene loop's deadline mode and its waits for replies. The deadline mode
//! ([`frame_deadline`], scene loop mode `both`, the default, or `deadline`,
//! `dcl_wasm::scene_loop_mode`) ends the loop a frame target after the frame's start, less what
//! the frame still needs after the loop: the 90th percentile of that tail over the last frames
//! ([`TailEstimate`]) plus a share of the frame target. The tail is the frame's work, without its
//! waits for the render worker ([`frame_tail`]). The tail is measured with the pipelined-rendering
//! diagnostics (`PipelinedRenderingDiagnosticsPlugin`, which the explorer's performance plugin
//! adds on the web); without them no tail is measured, the deadline mode keeps the grid deadline,
//! and the scene runner warns at startup ([`warn_without_tail_diagnostics`]).

use std::time::Duration;

use bevy::platform::time::Instant;
#[cfg(target_arch = "wasm32")]
use bevy::{
    diagnostic::{DiagnosticMeasurement, DiagnosticPath, DiagnosticsStore},
    prelude::{App, First, Plugin, Res, ResMut, World},
    render::pipelined_rendering::diagnostics::{WAIT_HANDOFF, WAIT_RENDER},
};

use super::loop_bounds;

mod debug_panel;

/// Frames the tail estimate covers. It is used once they are all measured, so its 90th
/// percentile always leaves out the 3 largest: with fewer frames the nearest-rank 90th percentile
/// is the maximum, and a few heavy frames at the start would hold the loop on its floor.
pub const TAIL_FRAMES: usize = 32;
/// Added to the tail's 90th percentile, as a share of the frame target: slack for a tail beyond
/// it that grows with the frame. (A late timer at the loop's end needs none: the loop ends its
/// timed waits before the deadline by their lateness.)
pub const TAIL_MARGIN_SHARE: f64 = 0.06;

/// The deadline mode: a frame target after the frame's start, less the frame's tail after the
/// loop, within the same bounds as [`super::grid_deadline`].
pub fn frame_deadline(
    frame_start: Instant,
    loop_start: Instant,
    frame: Duration,
    tail: Duration,
) -> Instant {
    let (earliest_end, latest_end) = loop_bounds(loop_start, frame);
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

/// Sets up the web scene loop's pacing: [`LoopPacing`] and the frame's start
/// ([`web_frame_start`]).
#[cfg(target_arch = "wasm32")]
pub struct LoopPacingPlugin;

#[cfg(target_arch = "wasm32")]
impl Plugin for LoopPacingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LoopPacing>();
        app.add_systems(First, web_frame_start);
        // as the scene runner's other sections: only an app that renders has the console
        if app
            .world()
            .get_resource::<common::structs::NoRenderApp>()
            .is_none()
        {
            debug_panel::add_section(app);
        }
    }

    // the deadline mode measures the frame's tail with the pipelined-rendering diagnostics,
    // which another plugin adds
    fn finish(&self, app: &mut App) {
        warn_without_tail_diagnostics(app.world());
    }
}

/// The web scene loop across frames: the frame's start, the loop's last end and budget, the
/// frame's tail and the lateness of the loop's timed waits.
#[derive(bevy::prelude::Resource, Default)]
pub struct LoopPacing {
    frame_start: Option<Instant>,
    loop_end: Option<Instant>,
    // the last loop's, from its start to its deadline, for the debug panel
    budget: Option<Duration>,
    // from the previous frame's handoff to this frame's start, for the debug panel: the engine
    // worker's work between frames (the runner, its event loop's tasks) and, under a frame rate
    // cap, its wait for the cap's next frame; the reserve covers neither
    handoff_gap: Option<Duration>,
    tail: TailEstimate,
    wait_lateness: WaitLateness,
}

impl LoopPacing {
    /// The frame starts at `now`. The previous frame's `handoff` to the render worker and its
    /// `waits` for that worker close that frame's tail ([`frame_tail`]).
    pub fn frame_started(&mut self, now: Instant, handoff: Option<Instant>, waits: Duration) {
        self.frame_start = Some(now);
        if let (Some(handoff), Some(loop_end)) = (handoff, self.loop_end.take()) {
            if let Some(tail) = frame_tail(loop_end, handoff, waits) {
                self.tail.push(tail);
                self.handoff_gap = Some(now.saturating_duration_since(handoff));
            }
        }
    }

    /// The deadline mode's deadline for a loop that starts at `loop_start`: from the frame's start
    /// ([`frame_deadline`]) once the tail is measured and the frame rate is capped (`frame`, its
    /// target), else `grid` ([`super::grid_deadline`]).
    pub fn deadline(&self, grid: Instant, loop_start: Instant, frame: Option<Duration>) -> Instant {
        match (frame, self.frame_start) {
            (Some(frame), Some(frame_start)) => match self.tail.reserve(frame) {
                Some(reserve) => frame_deadline(frame_start, loop_start, frame, reserve),
                None => grid,
            },
            _ => grid,
        }
    }

    /// The last moment a timed wait for replies may end, when a reply ends the waits: before
    /// `deadline` by the recent waits' lateness ([`WaitLateness::wait_end`]).
    pub fn wait_end(&self, deadline: Instant, loop_start: Instant) -> Instant {
        self.wait_lateness.wait_end(deadline, loop_start)
    }

    /// Starts the frame's scene loop under the page's scene loop mode
    /// (`dcl_wasm::scene_loop_mode`), read once here: its deadline, from the frame's start in the
    /// deadline mode, and, when a reply ends its waits, the last moment a timed wait may end
    /// ([`WaitLateness::wait_end`]). `frame` is the frame target, None with an uncapped frame
    /// rate.
    #[cfg(target_arch = "wasm32")]
    pub fn begin(
        &mut self,
        grid: Instant,
        loop_start: Instant,
        frame: Option<Duration>,
    ) -> PacedLoop {
        let mode = dcl_wasm::scene_loop_mode::get();
        let deadline = if mode.frame_deadline() {
            self.deadline(grid, loop_start, frame)
        } else {
            grid
        };
        let waits_for_replies = mode.loop_waits_for_replies();
        self.budget = Some(deadline.saturating_duration_since(loop_start));
        PacedLoop {
            deadline,
            last_wait_end: if waits_for_replies {
                self.wait_end(deadline, loop_start)
            } else {
                deadline
            },
            replies_seen: waits_for_replies.then_some(0),
            mode_name: mode.name(),
            tail_reserve: frame.and_then(|frame| self.tail.reserve(frame)),
        }
    }

    /// Ends the frame's scene loop, which ended at `loop_end`: the next frame's start closes its
    /// tail.
    #[cfg(target_arch = "wasm32")]
    pub fn end(&mut self, _paced: PacedLoop, loop_end: Instant) {
        self.loop_end = Some(loop_end);
    }
}

/// One frame's scene loop on the web ([`LoopPacing::begin`]).
#[cfg(target_arch = "wasm32")]
pub struct PacedLoop {
    /// The loop starts no pass after it.
    pub deadline: Instant,
    /// No timed wait for replies ends after it.
    pub last_wait_end: Instant,
    /// When a reply ends the loop's waits, the replies counted before the pass; None in
    /// `legacy`, which sleeps instead.
    replies_seen: Option<u32>,
    /// MEASUREMENT ONLY (tick_stages): the loop's mode, and what the deadline mode leaves for the
    /// frame's tail ([`TailEstimate::reserve`]) whichever the mode; None until measured
    pub mode_name: &'static str,
    pub tail_reserve: Option<Duration>,
}

#[cfg(target_arch = "wasm32")]
impl PacedLoop {
    /// Before a pass: the replies so far, which the pass drains; a reply after them ends the wait
    /// that follows it. `legacy` takes no lock here.
    pub fn pass_start(&mut self) {
        if let Some(seen) = &mut self.replies_seen {
            *seen = dcl_wasm::scene_replies::REPLIES.seen();
        }
    }

    /// Waits `wait`, until `next_pass`, for a reply. A browser's timed wait can wake a millisecond
    /// or more late, past the deadline, so in every mode but `legacy` a reply that arrives
    /// meanwhile ends the wait, and a wait that times out records how late it woke in
    /// [`LoopPacing`]'s [`WaitLateness`], for the next frames' loops.
    pub fn wait(&mut self, world: &mut World, next_pass: Instant, wait: Duration) {
        let Some(seen) = self.replies_seen else {
            std::thread::sleep(wait);
            return;
        };
        if dcl_wasm::scene_replies::REPLIES.wait(seen, wait) {
            let now = Instant::now();
            world
                .resource_mut::<LoopPacing>()
                .wait_lateness
                .push(now, now.saturating_duration_since(next_pass));
        }
    }
}

/// The frame's work after its scene loop: from the loop's end to the handoff to the render
/// worker, less the frame's waits for that worker (`waits`). None for a handoff before the loop's
/// end, an older frame's.
pub fn frame_tail(loop_end: Instant, handoff: Instant, waits: Duration) -> Option<Duration> {
    handoff
        .checked_duration_since(loop_end)
        .map(|tail| tail.saturating_sub(waits))
}

/// `First`: the frame's start, and the previous frame's tail ([`LoopPacing::frame_started`]):
/// the handoff and the waits come from the pipelined-rendering diagnostics, which record them
/// together at the end of the app's update. The waits, for the render app to come back
/// (`WAIT_RENDER`) and for the render worker's animation frame to take the handoff
/// (`WAIT_HANDOFF`), shrink as the loop runs longer, so they are not part of what the loop must
/// leave. It reads only what the previous update recorded and writes only [`LoopPacing`], so it
/// takes no order in `First`; a system there that runs before it moves the frame's start, and the
/// deadline, later by its own time.
#[cfg(target_arch = "wasm32")]
fn web_frame_start(mut pacing: ResMut<LoopPacing>, store: Res<DiagnosticsStore>) {
    fn last<'a>(
        store: &'a DiagnosticsStore,
        path: &DiagnosticPath,
    ) -> Option<&'a DiagnosticMeasurement> {
        store.get(path)?.measurement()
    }

    let handoff = last(&store, &WAIT_HANDOFF).map(|measurement| measurement.time);
    let waits = [WAIT_RENDER, WAIT_HANDOFF]
        .iter()
        .filter_map(|path| last(&store, path))
        .map(|measurement| Duration::from_secs_f64(measurement.value.max(0.0) / 1000.0))
        .sum();
    pacing.frame_started(Instant::now(), handoff, waits);
}

/// At startup: without the pipelined-rendering diagnostics no frame's tail is measured, and the
/// deadline mode keeps the grid deadline for the whole session.
#[cfg(target_arch = "wasm32")]
fn warn_without_tail_diagnostics(world: &World) {
    let measured = world
        .get_resource::<DiagnosticsStore>()
        .is_some_and(|store| store.get(&WAIT_HANDOFF).is_some());
    if !measured {
        bevy::log::warn!(
            "no pipelined-rendering diagnostics (PipelinedRenderingDiagnosticsPlugin): \
             the web scene loop keeps the grid deadline"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loop_pacing::{
        grid_deadline,
        tests::{close, ms, T},
        LOOP_FLOOR,
    };

    /// After a frame twice the target, the grid budget sits on the floor; the deadline mode
    /// gives the frame what its start and tail leave.
    #[test]
    fn after_a_long_frame_the_deadline_follows_the_frames_start() {
        let s0 = Instant::now();
        let (_, next) = grid_deadline(s0 + T, s0, T);
        let frame_start = s0 + 2 * T;
        let s = frame_start + ms(4.7);
        let (grid, _) = grid_deadline(next, s, T);
        assert_eq!(grid - s, LOOP_FLOOR);
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

    #[test]
    fn a_frame_closes_the_previous_frames_tail() {
        let mut pacing = LoopPacing::default();
        let s = Instant::now();
        // no loop has ended yet
        pacing.frame_started(s, Some(s), Duration::ZERO);
        assert_eq!(pacing.tail.len, 0);
        // the loop ends, the frame hands over 9 ms later after 6 ms of waits: 3 ms of tail
        pacing.loop_end = Some(s + ms(5.0));
        pacing.frame_started(s + T, Some(s + ms(14.0)), ms(6.0));
        assert_eq!(pacing.frame_start, Some(s + T));
        assert_eq!(pacing.tail.len, 1);
        assert!(close(pacing.tail.ring[0], ms(3.0)));
        // and from that handoff to this frame's start
        assert!(close(pacing.handoff_gap.unwrap(), T - ms(14.0)));
        // each loop end closes one tail; a frame without a new one pushes none
        pacing.frame_started(s + 2 * T, Some(s + ms(20.0)), Duration::ZERO);
        assert_eq!(pacing.tail.len, 1);
        // a handoff from before the loop's end is an older frame's
        pacing.loop_end = Some(s + 2 * T + ms(5.0));
        pacing.frame_started(s + 3 * T, Some(s + 2 * T), Duration::ZERO);
        assert_eq!(pacing.tail.len, 1);
    }

    /// The deadline mode keeps the grid until the tail is measured and with an uncapped frame
    /// rate, then ends the loop a frame target after the frame's start, less the reserve.
    #[test]
    fn the_deadline_mode_keeps_the_grid_until_the_tail_is_measured() {
        let mut pacing = LoopPacing::default();
        let s = Instant::now();
        let grid = s + ms(9.0);
        let loop_start = s + ms(4.0);
        assert_eq!(pacing.deadline(grid, loop_start, Some(T)), grid);
        pacing.frame_started(s, None, Duration::ZERO);
        for _ in 0..TAIL_FRAMES - 1 {
            pacing.tail.push(ms(3.0));
        }
        assert_eq!(pacing.deadline(grid, loop_start, Some(T)), grid);
        pacing.tail.push(ms(3.0));
        // the reserve: 3 ms and 6 % of the frame target
        let reserve = pacing.tail.reserve(T).unwrap();
        assert_eq!(pacing.deadline(grid, loop_start, Some(T)), s + T - reserve);
        assert_eq!(pacing.deadline(grid, loop_start, None), grid);
    }

    #[test]
    fn the_waits_end_before_the_deadline_by_their_lateness() {
        let mut pacing = LoopPacing::default();
        let s = Instant::now();
        let deadline = s + ms(8.0);
        assert_eq!(pacing.wait_end(deadline, s), deadline);
        pacing.wait_lateness.push(s, ms(1.0));
        assert!(close(pacing.wait_end(deadline + T, s + T) - s, ms(7.0) + T));
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
            assert_eq!(
                lateness.wait_end(loop_start + ms(8.0), loop_start),
                loop_start
            );
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
