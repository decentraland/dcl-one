//! Ordering and per-frame time budget for scene raycasts. Requests in the player's scene go
//! first, then by scene priority, then the longest-waiting; casting stops once the frame's budget
//! is spent and the rest wait for a later frame.

use std::{cmp::Ordering, time::Duration};

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::structs::AppConfig;
use web_time::Instant;

/// What the last frame's raycast pass did: casts run, casts left for a later frame, and the time
/// spent casting.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq)]
pub struct RaycastStats {
    pub ran: u32,
    pub deferred: u32,
    pub spent: Duration,
}

/// Report, as JSON, what the last frame's raycast pass did: `budgetMs`, `ran`, `deferred` and
/// `spentMs`.
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

/// The fields a pending raycast request is ordered by.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    /// The request's scene contains the player.
    pub in_player_scene: bool,
    /// The scene's `RendererSceneContext::priority` (lower runs sooner).
    pub priority: f32,
    /// The frame the request last ran, 0 if never.
    pub last_run: u32,
    /// Final tie-break so the order is stable across frames.
    pub entity_id: u32,
}

/// Player scene first, then lower priority, then never-run, then the oldest run, then entity id.
pub fn candidate_order(a: &Candidate, b: &Candidate) -> Ordering {
    b.in_player_scene
        .cmp(&a.in_player_scene)
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
        v.sort_by(candidate_order);
        assert_eq!(
            v.iter().map(|c| c.entity_id).collect::<Vec<_>>(),
            vec![2, 3, 1, 4, 9]
        );
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
