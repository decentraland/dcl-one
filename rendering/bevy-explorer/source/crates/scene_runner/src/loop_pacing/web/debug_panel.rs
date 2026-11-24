//! The web scene loop's section in the runtime debug panel ("Scene Loop"): the scheduling mode,
//! which the tester switches without a reload (`/scene_loop_mode`, as `?sceneLoop=` sets it at
//! load), where the last loop's deadline came from and its budget, the time from the previous
//! frame's handoff to this frame's start, the frame tail's reserve, the lateness of the loop's
//! timed waits for replies and the scenes whose relays wake on the engine's send
//! (`/scene_loop_state`).

use std::time::Duration;

use bevy::platform::time::Instant;
use serde_json::{json, Value};

use super::{LoopPacing, TAIL_FRAMES};

/// The scene loop mode as the section shows it (`dcl_wasm::scene_loop_mode::SceneLoopMode`).
struct ModeView {
    name: &'static str,
    frame_deadline: bool,
    waits_for_replies: bool,
}

// milliseconds, to the microsecond
fn ms(duration: Duration) -> f64 {
    (duration.as_secs_f64() * 1e6).round() / 1e3
}

/// `/scene_loop_state`: `frame` is the frame target (None uncapped), `tail_measured` whether the
/// pipelined-rendering diagnostics measure the frame's tail, `woken` the titles of the scenes the
/// engine wakes.
fn state(
    pacing: &LoopPacing,
    mode: ModeView,
    frame: Option<Duration>,
    tail_measured: bool,
    now: Instant,
    woken: Vec<String>,
) -> Value {
    let reserve = frame.and_then(|frame| pacing.tail.reserve(frame));
    let tail_reason = if frame.is_none() {
        "The frame rate is uncapped: the loop keeps the grid deadline.".to_owned()
    } else if !tail_measured {
        "No pipelined-rendering diagnostics (PipelinedRenderingDiagnosticsPlugin): \
         no frame tail is measured."
            .to_owned()
    } else {
        format!(
            "Measuring the frame tail: {} of {TAIL_FRAMES} frames.",
            pacing.tail.len
        )
    };
    let deadline = if !mode.frame_deadline {
        "Grid (this mode keeps it)"
    } else if reserve.is_some() && pacing.frame_start.is_some() {
        "Frame start"
    } else {
        "Grid (no tail reserve yet)"
    };
    let (lateness, lateness_reason) = if mode.waits_for_replies {
        (
            pacing.wait_lateness.p90(now),
            "No wait for a reply timed out in the last second.",
        )
    } else {
        (
            None,
            "The legacy mode sleeps its waits for replies and records no lateness.",
        )
    };
    json!({
        "mode": mode.name,
        "deadline": deadline,
        "budget": pacing.budget.map(ms),
        "handoffGap": pacing.handoff_gap.map(ms),
        "tail": reserve.zip(pacing.tail.p90()).map(|(reserve, p90)| [ms(p90), ms(reserve)]),
        "tailFrames": pacing.tail.len,
        "lateness": lateness.map(ms),
        "woken": {"count": woken.len(), "scenes": woken},
        "unavailable": {
            "budget": "No scene loop has run yet.",
            "handoffGap": "No frame has handed over to the render worker after a scene loop yet.",
            "tail": tail_reason,
            "lateness": lateness_reason,
        },
    })
}

#[cfg(target_arch = "wasm32")]
pub(super) use commands::add_section;

#[cfg(target_arch = "wasm32")]
mod commands {
    use bevy::{
        diagnostic::DiagnosticsStore,
        platform::time::Instant,
        prelude::{App, Query, Res},
        render::pipelined_rendering::diagnostics::WAIT_HANDOFF,
    };
    use bevy_console::ConsoleCommand;
    use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};
    use dcl_wasm::scene_loop_mode::{self, SceneLoopMode};

    use super::{state, ModeView};
    use crate::{loop_pacing::LoopPacing, renderer_context::RendererSceneContext, SceneUpdates};

    const MODES: &[(&str, &str)] = &[
        ("both (relay wake and frame-start deadline)", "both"),
        ("relay (relay wake only)", "relay"),
        ("deadline (frame-start deadline only)", "deadline"),
        ("legacy (neither, no reply wake)", "legacy"),
    ];

    pub fn add_section(app: &mut App) {
        app.add_console_command::<SceneLoopState, _>(scene_loop_state)
            .add_console_command::<SceneLoopModeCommand, _>(set_scene_loop_mode)
            .add_debug_section(DebugSection {
                title: "Scene Loop",
                query: "/scene_loop_state",
                fields: &[
                    ("Deadline from", "deadline"),
                    ("Last loop budget (ms)", "budget"),
                    (
                        "Handoff to the next frame's start, last frame (ms)",
                        "handoffGap",
                    ),
                    (
                        "Frame tail reserve (ms) (90th percentile / with margin)",
                        "tail",
                    ),
                    ("Frames in the tail estimate", "tailFrames"),
                    (
                        "Timed reply wait lateness, last second (ms) (90th percentile)",
                        "lateness",
                    ),
                    (
                        "Scenes whose relays wake on the engine's send",
                        "woken.count",
                    ),
                    ("Woken scenes", "woken.scenes"),
                ],
                actions: &[],
                inputs: &[DebugInput {
                    label: "Scheduling mode (?sceneLoop=)",
                    field: "mode",
                    command: "/scene_loop_mode {value}",
                    kind: DebugInputKind::Select(MODES),
                }],
            });
    }

    #[derive(clap::Parser, ConsoleCommand)]
    #[command(name = "/scene_loop_state")]
    struct SceneLoopState;

    fn scene_loop_state(
        mut input: ConsoleCommand<SceneLoopState>,
        pacing: Res<LoopPacing>,
        updates: Res<SceneUpdates>,
        store: Res<DiagnosticsStore>,
        scenes: Query<&RendererSceneContext>,
    ) {
        if input.take().is_none() {
            return;
        }
        let mode = scene_loop_mode::get();
        let woken = scenes
            .iter()
            .filter(|context| context.wake().is_some_and(|wake| wake.get()))
            .map(|context| context.title.clone())
            .collect();
        let view = ModeView {
            name: mode.name(),
            frame_deadline: mode.frame_deadline(),
            waits_for_replies: mode.loop_waits_for_replies(),
        };
        let tail_measured = store.get(&WAIT_HANDOFF).is_some();
        input.reply_ok(
            state(
                &pacing,
                view,
                updates.frame_target,
                tail_measured,
                Instant::now(),
                woken,
            )
            .to_string(),
        );
    }

    /// Switches the web scene loop's scheduling mode, as `?sceneLoop=` does at load: the engine's
    /// loop reads it at its next frame and each relay at its next step.
    #[derive(clap::Parser, ConsoleCommand)]
    #[command(name = "/scene_loop_mode")]
    struct SceneLoopModeCommand {
        mode: String,
    }

    fn set_scene_loop_mode(mut input: ConsoleCommand<SceneLoopModeCommand>) {
        if let Some(Ok(command)) = input.take() {
            match SceneLoopMode::parse(&command.mode) {
                Some(mode) => {
                    scene_loop_mode::set(mode);
                    input.reply_ok(format!("scene loop mode {}", mode.name()));
                }
                None => input.reply_failed("choose legacy, relay, deadline or both"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loop_pacing::tests::{ms as dur, T};

    const BOTH: ModeView = ModeView {
        name: "both",
        frame_deadline: true,
        waits_for_replies: true,
    };

    #[test]
    fn before_the_tail_is_measured_the_section_says_why() {
        let now = Instant::now();
        let pacing = LoopPacing::default();
        let state = state(&pacing, BOTH, Some(T), true, now, Vec::new());
        assert_eq!(state["mode"], "both");
        assert_eq!(state["deadline"], "Grid (no tail reserve yet)");
        assert!(state["budget"].is_null() && state["tail"].is_null());
        assert_eq!(
            state["unavailable"]["tail"],
            "Measuring the frame tail: 0 of 32 frames."
        );
        assert!(state["lateness"].is_null());
        assert_eq!(state["woken"]["count"], 0);

        let uncapped = super::state(&pacing, BOTH, None, true, now, Vec::new());
        assert_eq!(
            uncapped["unavailable"]["tail"],
            "The frame rate is uncapped: the loop keeps the grid deadline."
        );
    }

    #[test]
    fn a_measured_loop_shows_its_budget_reserve_lateness_and_woken_scenes() {
        let now = Instant::now();
        let mut pacing = LoopPacing::default();
        pacing.frame_started(now, None, Duration::ZERO);
        for _ in 0..TAIL_FRAMES {
            pacing.tail.push(dur(3.0));
        }
        pacing.budget = Some(dur(7.25));
        pacing.handoff_gap = Some(dur(0.4));
        pacing.wait_lateness.push(now, dur(1.5));
        let state = state(
            &pacing,
            BOTH,
            Some(T),
            true,
            now,
            vec!["Genesis Plaza".to_owned()],
        );
        assert_eq!(state["deadline"], "Frame start");
        assert_eq!(state["budget"], 7.25);
        assert_eq!(state["handoffGap"], 0.4);
        // 3 ms and 6 % of the frame target
        assert_eq!(state["tail"], json!([3.0, 4.0]));
        assert_eq!(state["tailFrames"], 32);
        assert_eq!(state["lateness"], 1.5);
        assert_eq!(
            state["woken"],
            json!({"count": 1, "scenes": ["Genesis Plaza"]})
        );

        // legacy keeps the grid and records no lateness
        let legacy = ModeView {
            name: "legacy",
            frame_deadline: false,
            waits_for_replies: false,
        };
        let state = super::state(&pacing, legacy, Some(T), true, now, Vec::new());
        assert_eq!(state["deadline"], "Grid (this mode keeps it)");
        assert!(state["lateness"].is_null());
    }
}
