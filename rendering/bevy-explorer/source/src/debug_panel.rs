use bevy::{
    diagnostic::{Diagnostic, DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::*,
};
use bevy_console::ConsoleCommand;
use common::{
    gpu_time::GPU_FRAME_TIME,
    memory::{GpuMemoryBudget, MemoryStatus},
    structs::{AppConfig, PreviewMode, SessionGraphics},
};
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};

pub(super) fn register(app: &mut App) {
    crate::memory_reclaim_debug::register(app);
    app.add_console_command::<MemoryCommand, _>(debug_memory)
        .add_preview_debug_inputs(DebugSection {
            title: "Memory and limits",
            query: "/debug_memory",
            fields: &[
                ("Engine memory (MiB)", "used"),
                ("Platform capacity (MiB)", "capacity"),
                ("Effective cap (MiB)", "cap"),
                ("Memory pressure", "pressure"),
                ("Thresholds (MiB)", "thresholds"),
                ("GPU memory pools", "gpu"),
            ],
            actions: &[("Restore platform limit", "/debug_memory 0")],
            inputs: &[DebugInput {
                label: "Session memory limit (MiB)",
                field: "limit",
                command: "/debug_memory {value}",
                kind: DebugInputKind::Number {
                    min: 0.0,
                    max: u32::MAX as f64,
                    step: 1.0,
                },
            }],
        });
    app.add_console_command::<PerformanceStateCommand, _>(performance_state)
        .add_debug_section(DebugSection {
            title: "Performance",
            inputs: &[DebugInput {
                label: "30 FPS limit",
                field: "limit30",
                command: "/session_graphics fps {value}",
                kind: DebugInputKind::ActionButton {
                    enable: ("Limit to 30 FPS", "30"),
                    disable: ("Use configured FPS", ""),
                },
            }],
            query: "/performance_state",
            fields: &[
                ("Client version", "version"),
                ("Frame rate (FPS)", "fps"),
                ("Frame duration (ms)", "frameMs"),
                ("Min FPS in last 1,000 frames", "minFps"),
                ("Max FPS in last 1,000 frames", "maxFps"),
                ("Hiccups in last 1,000 frames", "hiccups"),
                ("Target frame rate (FPS)", "targetFps"),
                ("GPU frame time (ms)", "gpuFrameMs"),
                ("Engine memory (MiB)", "engineMemoryMiB"),
            ],
            actions: &[],
        });
}

/// Report the live engine readings and the effective session frame-rate target as JSON.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/performance_state")]
struct PerformanceStateCommand;

fn performance_state(
    mut input: ConsoleCommand<PerformanceStateCommand>,
    window: Res<analytics::frame::LastFrameWindow>,
    diagnostics: Res<DiagnosticsStore>,
    time: Res<Time<Real>>,
    session: Res<SessionGraphics>,
    config: Res<AppConfig>,
    memory: Res<MemoryStatus>,
) {
    if let Some(Ok(_)) = input.take() {
        let fps = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FPS)
            .and_then(Diagnostic::smoothed)
            .or_else(|| {
                let seconds = time.delta_secs_f64();
                (seconds > 0.0).then(|| seconds.recip())
            });
        let gpu = diagnostics.get(&GPU_FRAME_TIME);
        let gpu_ms = gpu.and_then(Diagnostic::smoothed);
        let first_window = "Waiting for the first 1,000 frames.";
        input.reply_ok(
            serde_json::json!({
                "version": crate::version(),
                "frameMs": (time.delta_secs_f64() * 10000.0).round() / 10.0,
                "minFps": window.min_fps, "maxFps": window.max_fps, "hiccups": window.hiccups,
                "fps": fps.map(f64::round),
                "targetFps": session.fps_target(&config),
                "overrideFps": session.fps_target,
                "limit30": session.fps_target == Some(30),
                "gpuFrameMs": gpu_ms.map(|ms| (ms * 10.0).round() / 10.0),
                "engineMemoryMiB": memory.reading().map(|reading| reading.used_bytes >> 20),
                "unavailable": {
                    "minFps": first_window, "maxFps": first_window, "hiccups": first_window,
                    "fps": "Waiting for a frame.",
                    "gpuFrameMs": if gpu.is_some() { "Waiting for a GPU timestamp sample." } else { "GPU timestamps are unavailable on this renderer." },
                    "engineMemoryMiB": "No platform memory sample.",
                },
            })
            .to_string(),
        );
    }
}

/// Inspect the engine memory budget; preview mode can set a session limit in MiB (zero restores capacity).
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/debug_memory")]
struct MemoryCommand {
    limit_mib: Option<u32>,
}

fn debug_memory(
    mut cmd: ConsoleCommand<MemoryCommand>,
    mut memory: ResMut<MemoryStatus>,
    gpu: Option<Res<GpuMemoryBudget>>,
    preview: Option<Res<PreviewMode>>,
) {
    let Some(Ok(args)) = cmd.take() else { return };
    if let Some(limit) = args.limit_mib {
        if !preview.is_some_and(|preview| preview.is_preview) {
            cmd.reply_failed("Memory limit controls require preview mode");
            return;
        }
        memory.limit_mib = std::num::NonZeroU64::new(limit.into());
        cmd.reply_ok("Updated session memory limit");
        return;
    }
    let reading = memory.reading();
    let pools = match gpu.as_deref() {
        Some(GpuMemoryBudget::Available(pools)) => Some(pools.iter().map(|p| serde_json::json!({
            "pool": format!("{:?}", p.kind), "usedMiB": p.used_bytes >> 20, "budgetMiB": p.budget_bytes.get() >> 20
        })).collect::<Vec<_>>()),
        _ => None,
    };
    let ram_reason = "No platform memory sample.";
    cmd.reply_ok(serde_json::json!({
        "used": reading.map(|r| r.used_bytes >> 20), "capacity": reading.map(|r| r.capacity_mib),
        "cap": reading.map(|r| r.cap_mib), "limit": memory.limit_mib.map_or(0, |n| n.get()),
        "controlsAvailable": preview.is_some_and(|preview| preview.is_preview),
        "pressure": reading.map(|r| format!("{:?}", r.pressure())), "gpu": pools,
        "thresholds": reading.map(|r| { let t = r.thresholds(); serde_json::json!({
            "abundance": t.abundance_mib.round(), "warning": t.warning_mib.round(), "full": t.full_mib.round()
        }) }),
        "unavailable": {
            "used":ram_reason, "capacity":ram_reason, "cap":ram_reason,
            "pressure":ram_reason, "thresholds":ram_reason,
            "gpu":match gpu.as_deref() {
                Some(GpuMemoryBudget::NotApplicable) => "This platform does not expose a GPU memory budget.",
                _ => "The graphics driver has not provided a GPU memory budget.",
            }
        }
    }).to_string());
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_console::ConsoleCommandEntered;
    use common::{memory::MemorySample, memory_reclaim::MemoryReclaimPlugin};
    use std::sync::{Arc, Mutex};

    fn app(preview: Option<bool>) -> App {
        let mut app = App::new();
        app.add_plugins((
            console::ConsolePlugin {
                add_bevy_console: false,
            },
            MemoryReclaimPlugin,
        ))
        .init_resource::<MemoryStatus>()
        .init_resource::<analytics::frame::LastFrameWindow>()
        .init_resource::<DiagnosticsStore>()
        .init_resource::<Time<Real>>()
        .init_resource::<SessionGraphics>()
        .init_resource::<AppConfig>();
        if let Some(is_preview) = preview {
            app.insert_resource(PreviewMode {
                is_preview,
                ..default()
            });
        }
        MemoryStatus::record(
            app.world_mut().resource_mut::<MemoryStatus>(),
            Some(MemorySample {
                used_bytes: 400 << 20,
                capacity_bytes: 1_000 << 20,
            }),
        );
        register(&mut app);
        app.finish();
        app.cleanup();
        app.update();
        app
    }

    fn command(app: &mut App, name: &str, args: &[&str]) -> Result<String, String> {
        let reply = Arc::new(Mutex::new(None));
        let output = reply.clone();
        app.world_mut().send_event(ConsoleCommandEntered {
            command_name: name.into(),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            responder: Some(Arc::new(move |result| {
                *output.lock().unwrap() = Some(result);
            })),
        });
        app.update();
        let mut result = reply.lock().unwrap();
        result.take().expect("command must reply")
    }

    #[test]
    fn memory_limit_mutations_require_preview_while_readings_remain_available() {
        for preview in [None, Some(false)] {
            let mut app = app(preview);
            for limit in ["1", "0"] {
                let error = command(&mut app, "/debug_memory", &[limit]).unwrap_err();
                assert!(error.contains("require preview mode"), "{error}");
                assert!(app.world().resource::<MemoryStatus>().limit_mib.is_none());
            }
            let query: serde_json::Value =
                serde_json::from_str(&command(&mut app, "/debug_memory", &[]).unwrap()).unwrap();
            assert_eq!(query["controlsAvailable"], false);
            assert_eq!(query["limit"], 0);
            assert_eq!(query["used"], 400);
            assert_eq!(query["capacity"], 1_000);
            assert_eq!(query["cap"], 1_000);
            assert_eq!(query["pressure"], "Abundance");
        }
    }

    #[test]
    fn preview_memory_limit_can_be_set_and_restored() {
        let mut app = app(Some(true));
        command(&mut app, "/debug_memory", &["500"]).unwrap();
        let query: serde_json::Value =
            serde_json::from_str(&command(&mut app, "/debug_memory", &[]).unwrap()).unwrap();
        assert_eq!(query["controlsAvailable"], true);
        assert_eq!(query["limit"], 500);
        assert_eq!(query["cap"], 500);
        assert_eq!(query["pressure"], "Full");

        command(&mut app, "/debug_memory", &["0"]).unwrap();
        let query: serde_json::Value =
            serde_json::from_str(&command(&mut app, "/debug_memory", &[]).unwrap()).unwrap();
        assert_eq!(query["limit"], 0);
        assert_eq!(query["cap"], 1_000);
        assert_eq!(query["pressure"], "Abundance");
    }
}
