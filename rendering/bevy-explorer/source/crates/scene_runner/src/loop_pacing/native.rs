//! The scene loop's pacing natively: the grid deadline ([`super::grid_deadline`]), waits for
//! replies on the native sleep, kept clear of the deadline by the spin sleeper's accuracy, and
//! with a frame cap the frame limiter's sleep to the deadline after the loop.

use std::time::Duration;

use bevy::{
    platform::time::Instant,
    prelude::{App, Plugin, Resource, World},
};
use spin_sleep::SpinSleeper;

pub struct LoopPacingPlugin;

impl Plugin for LoopPacingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LoopPacing>();
    }
}

#[derive(Resource, Default)]
pub struct LoopPacing {
    sleeper: SpinSleeper,
}

impl LoopPacing {
    /// Starts the frame's scene loop on the grid deadline `grid`. A native sleep can wake late
    /// by the sleeper's accuracy, so no wait for replies ends closer to the deadline than that.
    /// `frame` is the frame target, None with an uncapped frame rate.
    pub fn begin(
        &mut self,
        grid: Instant,
        _loop_start: Instant,
        frame: Option<Duration>,
    ) -> PacedLoop {
        PacedLoop {
            deadline: grid,
            last_wait_end: grid - Duration::new(0, self.sleeper.native_accuracy_ns()),
            capped: frame.is_some(),
        }
    }

    /// Ends the frame's scene loop: with a frame cap, sleeps to its deadline (the frame limiter).
    pub fn end(&mut self, paced: PacedLoop, _loop_end: Instant) {
        if paced.capped {
            if let Some(sleep_time) = paced.deadline.checked_duration_since(Instant::now()) {
                self.sleeper.sleep(sleep_time);
            }
        }
    }
}

/// One frame's scene loop ([`LoopPacing::begin`]).
pub struct PacedLoop {
    /// The loop starts no pass after it.
    pub deadline: Instant,
    /// No wait for replies ends after it.
    pub last_wait_end: Instant,
    capped: bool,
}

impl PacedLoop {
    /// Before a pass: nothing to note natively.
    pub fn pass_start(&mut self) {}

    /// Waits `wait`, until `next_pass`: the sleep the sleeper's accuracy is for.
    pub fn wait(&mut self, _world: &mut World, _next_pass: Instant, wait: Duration) {
        spin_sleep::native_sleep(wait);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loop_pacing::tests::{ms, T};

    #[test]
    fn the_loop_keeps_the_grid_and_its_waits_clear_of_it_by_the_sleepers_accuracy() {
        let mut pacing = LoopPacing::default();
        let s = Instant::now();
        let grid = s + ms(9.0);
        let paced = pacing.begin(grid, s, Some(T));
        assert_eq!(paced.deadline, grid);
        assert_eq!(
            grid - paced.last_wait_end,
            Duration::new(0, pacing.sleeper.native_accuracy_ns())
        );
    }

    /// With a frame cap the loop's end sleeps to the deadline; uncapped it returns at once.
    #[test]
    fn the_frame_limiter_sleeps_to_the_deadline_only_with_a_frame_cap() {
        let mut pacing = LoopPacing::default();
        let s = Instant::now();
        let capped = pacing.begin(s + ms(2.0), s, Some(T));
        pacing.end(capped, s);
        assert!(Instant::now() >= s + ms(2.0));

        let s = Instant::now();
        let uncapped = pacing.begin(s + Duration::from_secs(10), s, None);
        pacing.end(uncapped, s);
        assert!(Instant::now() < s + Duration::from_secs(10));
    }
}
