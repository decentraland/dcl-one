//! Per-frame time cap for the raycasts of scenes the player is not in. Raycasts of the system
//! scene, every portable and the scenes the player stands in run every frame without a cap, as
//! scenes drive movement and other frame-sensitive logic from their results. The rest are capped
//! at a tenth of the target frame and run nearest scene first, then the longest since it last ran;
//! what does not fit waits for a later frame. A capped scene becomes uncapped once the player
//! enters it, so no capped request can hold back the scene the player is in.

use std::{cmp::Ordering, time::Duration};

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use web_time::Instant;

use crate::SceneUpdates;

/// The capped tier may spend one tenth of the target frame.
pub const BUDGET_DIVISOR: u32 = 10;

/// The frame length assumed when the frame rate is uncapped: 60 fps.
pub const UNCAPPED_FRAME: Duration = Duration::from_nanos(1_000_000_000 / 60);

/// The most `/raycast_budget` can set, in milliseconds.
pub const MAX_OVERRIDE_MS: f32 = 10.0;

/// What one tier of the raycast pass did on the last frame it had at least one eligible raycast:
/// casts run, casts left for a later frame, and the time spent casting. A frame with nothing to
/// cast in the tier (most frames between scene ticks) leaves it as it was.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct TierStats {
    pub ran: u32,
    pub deferred: u32,
    pub spent: Duration,
}

/// The last pass of each tier, and the budget the capped tier ran under.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq)]
pub struct RaycastStats {
    pub budget: Duration,
    pub capped: TierStats,
    pub uncapped: TierStats,
}

/// A console override of the capped tier's budget, for tests and the debug panel; None follows
/// the target frame.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq)]
pub struct RaycastBudgetOverride(pub Option<Duration>);

/// The capped tier's budget: the override, or a tenth of the target frame (of 60 fps when the
/// frame rate is uncapped). The target, not the measured frame, so a slow frame does not raise
/// the next frame's budget.
pub fn budget(frame_target: Option<Duration>, budget_override: RaycastBudgetOverride) -> Duration {
    budget_override
        .0
        .unwrap_or_else(|| frame_target.unwrap_or(UNCAPPED_FRAME) / BUDGET_DIVISOR)
}

/// Report, as JSON, the capped tier's `budgetMs` and its last pass (`ran`, `deferred`,
/// `spentMs`), and the uncapped tier's last pass (`uncappedRan`, `uncappedSpentMs`).
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/raycast_stats")]
pub(super) struct RaycastStatsCommand;

fn ms(duration: Duration) -> f32 {
    duration.as_secs_f32() * 1000.0
}

pub(super) fn raycast_stats(
    mut input: ConsoleCommand<RaycastStatsCommand>,
    stats: Res<RaycastStats>,
) {
    if let Some(Ok(_)) = input.take() {
        input.reply_ok(
            serde_json::json!({
                "budgetMs": ms(stats.budget),
                "ran": stats.capped.ran,
                "deferred": stats.capped.deferred,
                "spentMs": ms(stats.capped.spent),
                "uncappedRan": stats.uncapped.ran,
                "uncappedSpentMs": ms(stats.uncapped.spent),
            })
            .to_string(),
        );
    }
}

/// Show the capped raycast tier's per-frame budget, override it in milliseconds (0 to 10), or
/// return it to a tenth of the target frame with `auto`.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/raycast_budget")]
pub(super) struct RaycastBudgetCommand {
    /// Milliseconds, or `auto`
    value: Option<String>,
}

/// The override `/raycast_budget <value>` asks for.
pub fn parse_override(value: &str) -> Result<RaycastBudgetOverride, String> {
    if value == "auto" {
        return Ok(RaycastBudgetOverride(None));
    }
    match value.parse::<f32>() {
        Ok(ms) if (0.0..=MAX_OVERRIDE_MS).contains(&ms) => Ok(RaycastBudgetOverride(Some(
            Duration::from_secs_f32(ms / 1000.0),
        ))),
        _ => Err(format!(
            "raycast budget must be `auto` or between 0 and {MAX_OVERRIDE_MS} ms"
        )),
    }
}

pub(super) fn raycast_budget(
    mut input: ConsoleCommand<RaycastBudgetCommand>,
    mut budget_override: ResMut<RaycastBudgetOverride>,
    updates: Res<SceneUpdates>,
) {
    if let Some(Ok(command)) = input.take() {
        if let Some(value) = command.value {
            match parse_override(&value) {
                Ok(value) => *budget_override = value,
                Err(e) => {
                    input.reply_failed(e);
                    return;
                }
            }
        }
        let mode = if budget_override.0.is_some() {
            "override"
        } else {
            "auto"
        };
        input.reply_ok(format!(
            "raycast budget: {:.2} ms ({mode})",
            ms(budget(updates.frame_target, *budget_override))
        ));
    }
}

/// The fields a pending capped raycast request is ordered by.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    /// The scene's `RendererSceneContext::priority`, its squared distance (lower runs sooner).
    pub priority: f32,
    /// The scene frame the request last ran, 0 if never.
    pub last_run: u32,
    /// Final tie-break so the order is stable across frames.
    pub entity_id: u32,
}

/// The nearest scene first, then the longest since it last ran (never-run first), then entity id.
pub fn candidate_order(a: &Candidate, b: &Candidate) -> Ordering {
    a.priority
        .total_cmp(&b.priority)
        .then_with(|| a.last_run.cmp(&b.last_run))
        .then_with(|| a.entity_id.cmp(&b.entity_id))
}

/// Runs `items` in order while the time spent is under `budget`. `now` is read once before the
/// first item and once after each `run`; the item that crosses the line still finishes. `run`
/// returns `false` when it declined the item, which then counts as neither ran nor deferred.
/// Items left when the budget runs out count as deferred.
pub fn run_within_budget<T>(
    items: Vec<T>,
    budget: Duration,
    now: &mut dyn FnMut() -> Instant,
    run: &mut dyn FnMut(T) -> bool,
) -> TierStats {
    let mut stats = TierStats::default();
    let total = items.len();
    if total == 0 {
        return stats;
    }
    let start = now();
    for (index, item) in items.into_iter().enumerate() {
        if stats.spent >= budget {
            stats.deferred = (total - index) as u32;
            break;
        }
        if run(item) {
            stats.ran += 1;
        }
        stats.spent = now().saturating_duration_since(start);
    }
    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clock(step_ms: u64) -> impl FnMut() -> Instant {
        let start = Instant::now();
        let mut calls = 0u64;
        move || {
            calls += 1;
            start + Duration::from_millis(step_ms * calls)
        }
    }

    #[test]
    fn order_is_nearest_scene_then_oldest_then_entity() {
        let c = |priority, last_run, entity_id| Candidate {
            priority,
            last_run,
            entity_id,
        };
        let mut v = [
            c(4.0, 5, 9),
            c(1.0, 7, 1),
            c(1.0, 0, 3),
            c(1.0, 0, 2),
            c(2.0, 1, 4),
        ];
        v.sort_by(candidate_order);
        assert_eq!(
            v.iter().map(|c| c.entity_id).collect::<Vec<_>>(),
            vec![2, 3, 1, 4, 9]
        );
    }

    #[test]
    fn budget_is_a_tenth_of_the_target_frame() {
        let auto = RaycastBudgetOverride(None);
        let at = |fps: u64| budget(Some(Duration::from_nanos(1_000_000_000 / fps)), auto);
        assert_eq!(at(60).as_micros(), 1666);
        assert_eq!(at(144).as_micros(), 694);
        assert_eq!(at(240).as_micros(), 416);
        assert_eq!(budget(None, auto), at(60));
    }

    #[test]
    fn an_override_replaces_the_frame_share() {
        let set = RaycastBudgetOverride(Some(Duration::ZERO));
        assert_eq!(budget(Some(Duration::from_millis(16)), set), Duration::ZERO);
    }

    #[test]
    fn override_accepts_auto_and_0_to_10_ms() {
        assert_eq!(parse_override("auto"), Ok(RaycastBudgetOverride(None)));
        assert_eq!(
            parse_override("0"),
            Ok(RaycastBudgetOverride(Some(Duration::ZERO)))
        );
        assert_eq!(
            parse_override("10"),
            Ok(RaycastBudgetOverride(Some(Duration::from_millis(10))))
        );
        for bad in ["10.5", "-1", "fast"] {
            assert!(parse_override(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_zero_budget_defers_everything() {
        let mut ran = vec![];
        let stats = run_within_budget(vec![1, 2, 3], Duration::ZERO, &mut clock(1), &mut |i| {
            ran.push(i);
            true
        });
        assert!(ran.is_empty());
        assert_eq!(
            stats,
            TierStats {
                ran: 0,
                deferred: 3,
                spent: Duration::ZERO
            }
        );
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
    fn an_unlimited_budget_runs_everything() {
        let stats = run_within_budget(vec![1, 2, 3], Duration::MAX, &mut clock(5), &mut |_| true);
        assert_eq!((stats.ran, stats.deferred), (3, 0));
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
        assert_eq!(stats, TierStats::default());
    }
}
