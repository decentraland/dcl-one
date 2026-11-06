//! Ordering and per-frame time budget for scene raycasts. Requests deferred for 60 frames or more
//! go first, then those in the player's scene, then by scene priority, then the longest-waiting;
//! casting stops once the frame's budget is spent and the rest wait for a later frame.

use std::{cmp::Ordering, time::Duration};

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::structs::AppConfig;
use web_time::Instant;

/// What the raycast pass did on the last frame that had at least one eligible raycast: casts run,
/// casts left for a later frame, and the time spent casting. A frame with nothing to cast (most
/// frames between scene ticks) leaves it as it was.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq)]
pub struct RaycastStats {
    pub ran: u32,
    pub deferred: u32,
    pub spent: Duration,
}

/// Report, as JSON, what the raycast pass did on the last frame that had at least one raycast to
/// cast: `budgetMs`, `ran`, `deferred` and `spentMs`.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/raycast_stats")]
pub(super) struct RaycastStatsCommand;

pub(super) fn raycast_stats(
    mut input: ConsoleCommand<RaycastStatsCommand>,
    stats: Res<RaycastStats>,
    config: Res<AppConfig>,
) {
    if let Some(Ok(_)) = input.take() {
        input.reply_ok(
            serde_json::json!({
                "budgetMs": config.raycast_budget_ms,
                "ran": stats.ran,
                "deferred": stats.deferred,
                "spentMs": stats.spent.as_secs_f32() * 1000.0,
            })
            .to_string(),
        );
    }
}

/// The most a hand-edited `raycast_budget_ms` can make the budget, as the setting's range allows.
pub const MAX_BUDGET_MS: u32 = 10;

/// A request deferred for this many frames is starved and runs ahead of the scene order.
pub const STARVED_AFTER_FRAMES: u32 = 60;

/// The frame's budget for a configured `raycast_budget_ms`, capped at [`MAX_BUDGET_MS`].
pub fn budget(raycast_budget_ms: u32) -> Duration {
    Duration::from_millis(raycast_budget_ms.min(MAX_BUDGET_MS) as u64)
}

/// The fields a pending raycast request is ordered by.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    /// The frame the request was first deferred on, 0 if it is not waiting.
    pub deferred_since: u32,
    /// The request's scene contains the player.
    pub in_player_scene: bool,
    /// The scene's `RendererSceneContext::priority` (lower runs sooner).
    pub priority: f32,
    /// The frame the request last ran, 0 if never.
    pub last_run: u32,
    /// Final tie-break so the order is stable across frames.
    pub entity_id: u32,
}

impl Candidate {
    /// Deferred for [`STARVED_AFTER_FRAMES`] or more as of `frame`.
    pub fn starved(&self, frame: u32) -> bool {
        self.deferred_since != 0
            && frame.saturating_sub(self.deferred_since) >= STARVED_AFTER_FRAMES
    }
}

/// Starved requests first (the longest-deferred first), so a scene that fills the budget every
/// frame cannot hold the others back forever; then the player's scene, then lower priority, then
/// never-run, then the oldest run, then entity id. `frame` is the current engine frame.
pub fn candidate_order(a: &Candidate, b: &Candidate, frame: u32) -> Ordering {
    let (a_starved, b_starved) = (a.starved(frame), b.starved(frame));
    b_starved
        .cmp(&a_starved)
        .then_with(|| {
            if a_starved && b_starved {
                a.deferred_since.cmp(&b.deferred_since)
            } else {
                Ordering::Equal
            }
        })
        .then_with(|| b.in_player_scene.cmp(&a.in_player_scene))
        .then_with(|| a.priority.total_cmp(&b.priority))
        .then_with(|| (a.last_run != 0).cmp(&(b.last_run != 0)))
        .then_with(|| a.last_run.cmp(&b.last_run))
        .then_with(|| a.entity_id.cmp(&b.entity_id))
}

/// Runs `items` in order until `budget` is spent, always running at least one. `now` is read once
/// before the first item and once after each `run`; an item starts only while the time spent is
/// under budget, so the item that crosses the line still finishes. `run` returns `false` when it
/// declined the item, which then counts as neither ran nor deferred. Items left when the budget
/// runs out count as deferred. `spent` is zero when no item ran.
pub fn run_within_budget<T>(
    items: Vec<T>,
    budget: Duration,
    now: &mut dyn FnMut() -> Instant,
    run: &mut dyn FnMut(T) -> bool,
) -> RaycastStats {
    let mut stats = RaycastStats::default();
    let total = items.len();
    if total == 0 {
        return stats;
    }
    let start = now();
    let mut spent = Duration::ZERO;
    for (index, item) in items.into_iter().enumerate() {
        if spent >= budget && stats.ran >= 1 {
            stats.deferred = (total - index) as u32;
            break;
        }
        if run(item) {
            stats.ran += 1;
        }
        spent = now().saturating_duration_since(start);
    }
    if stats.ran > 0 {
        stats.spent = spent;
    }
    stats
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use web_time::Instant;

    fn clock(step_ms: u64) -> impl FnMut() -> Instant {
        let start = Instant::now();
        let mut calls = 0u64;
        move || {
            calls += 1;
            start + Duration::from_millis(step_ms * calls)
        }
    }

    #[test]
    fn order_player_scene_then_priority_then_oldest_then_entity() {
        let c = |p, pr, lr, e| Candidate {
            deferred_since: 0,
            in_player_scene: p,
            priority: pr,
            last_run: lr,
            entity_id: e,
        };
        let mut v = [
            c(false, 1.0, 5, 9),
            c(true, 9.0, 7, 1),
            c(true, 9.0, 0, 3),
            c(true, 9.0, 0, 2),
            c(false, 0.5, 1, 4),
        ];
        v.sort_by(|a, b| candidate_order(a, b, 100));
        assert_eq!(
            v.iter().map(|c| c.entity_id).collect::<Vec<_>>(),
            vec![2, 3, 1, 4, 9]
        );
    }

    #[test]
    fn starved_requests_run_ahead_of_the_scene_order() {
        let c = |deferred_since, in_player_scene, e| Candidate {
            deferred_since,
            in_player_scene,
            priority: 1.0,
            last_run: 5,
            entity_id: e,
        };
        // at frame 200: 1 and 2 are in the player's scene; 3 waited 60 frames, 4 waited 100,
        // 5 only 59 (not yet starved), 6 is not waiting
        let mut v = [
            c(0, true, 1),
            c(150, true, 2),
            c(140, false, 3),
            c(100, false, 4),
            c(141, false, 5),
            c(0, false, 6),
        ];
        v.sort_by(|a, b| candidate_order(a, b, 200));
        assert_eq!(
            v.iter().map(|c| c.entity_id).collect::<Vec<_>>(),
            vec![4, 3, 1, 2, 5, 6]
        );
        assert!(!c(0, false, 7).starved(u32::MAX));
    }

    #[test]
    fn a_hand_edited_budget_is_capped_at_10_ms() {
        assert_eq!(budget(2), Duration::from_millis(2));
        assert_eq!(budget(10), Duration::from_millis(10));
        assert_eq!(budget(500), Duration::from_millis(10));
    }

    #[test]
    fn budget_loop_runs_at_least_one_then_defers_the_rest() {
        let mut ran = vec![];
        let stats = run_within_budget(vec![1, 2, 3], Duration::ZERO, &mut clock(1), &mut |i| {
            ran.push(i);
            true
        });
        assert_eq!(ran, vec![1]);
        assert_eq!((stats.ran, stats.deferred), (1, 2));
    }

    #[test]
    fn budget_loop_runs_the_cast_that_crosses_the_line() {
        // 1 ms per cast, 2 ms budget: the second cast starts at 1 ms spent and runs, ending at
        // 2 ms; the third starts with the budget spent and is deferred
        let mut ran = vec![];
        let stats = run_within_budget(
            vec![1, 2, 3, 4],
            Duration::from_millis(2),
            &mut clock(1),
            &mut |i| {
                ran.push(i);
                true
            },
        );
        assert_eq!(ran, vec![1, 2]);
        assert_eq!((stats.ran, stats.deferred), (2, 2));
    }

    #[test]
    fn budget_loop_skips_items_the_runner_declines() {
        let stats = run_within_budget(
            vec![1, 2],
            Duration::from_secs(1),
            &mut clock(0),
            &mut |i| i == 2,
        );
        assert_eq!((stats.ran, stats.deferred), (1, 0));
    }

    #[test]
    fn budget_loop_with_nothing_reports_zero() {
        let stats = run_within_budget(
            Vec::<u32>::new(),
            Duration::from_secs(1),
            &mut clock(1),
            &mut |_| true,
        );
        assert_eq!(
            (stats.ran, stats.deferred, stats.spent),
            (0, 0, Duration::ZERO)
        );
    }
}
