use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::{
    memory_reclaim::{MemoryReclaimSettings, MemoryReclaimState, ReclaimBudget, ReclaimSimulation},
    structs::PreviewMode,
};
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};

pub(crate) fn register(app: &mut App) {
    app.add_console_command::<MemoryReclaimCommand, _>(memory_reclaim)
        .add_preview_debug_inputs(DebugSection {
            title: "Asset release",
            query: "/memory_reclaim",
            fields: &[
                ("Measured RAM pressure", "ram"),
                ("Measured GPU pressure", "gpu"),
                ("RAM cleanup active", "ramActive"),
                ("GPU cleanup active", "gpuActive"),
                ("Last cleanup duration (ms)", "elapsedMs"),
                ("Time allowance exhausted", "budgetExhausted"),
                ("Cache pins and releases", "caches"),
                ("Policy", "policy"),
            ],
            actions: &[],
            inputs: &[
                DebugInput {
                    label: "Simulated pressure (session)",
                    field: "simulation",
                    command: "/memory_reclaim {value}",
                    kind: DebugInputKind::Select(&[
                        ("Off", "off"),
                        ("RAM", "ram"),
                        ("GPU", "gpu"),
                        ("RAM and GPU", "both"),
                    ]),
                },
                DebugInput {
                    label: "Cleanup allowance (ms)",
                    field: "budgetMs",
                    command: "/memory_reclaim --budget-ms {value}",
                    kind: DebugInputKind::Number {
                        min: 0.1,
                        max: 16.0,
                        step: 0.1,
                    },
                },
            ],
        });
}

/// Inspect cache reclamation; preview builds can simulate pressure and set a session allowance.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/memory_reclaim")]
struct MemoryReclaimCommand {
    #[arg(value_parser = ["off", "ram", "gpu", "both"])]
    simulation: Option<String>,
    #[arg(long)]
    budget_ms: Option<f64>,
}

fn memory_reclaim(
    mut input: ConsoleCommand<MemoryReclaimCommand>,
    mut settings: ResMut<MemoryReclaimSettings>,
    state: Res<MemoryReclaimState>,
    budget: Res<ReclaimBudget>,
    preview: Option<Res<PreviewMode>>,
) {
    let Some(Ok(args)) = input.take() else {
        return;
    };
    if args.simulation.is_some() || args.budget_ms.is_some() {
        if !preview.is_some_and(|preview| preview.is_preview) {
            input.reply_failed("Memory reclamation controls require preview mode");
            return;
        }
        if args
            .budget_ms
            .is_some_and(|ms| !ms.is_finite() || !(0.1..=16.0).contains(&ms))
        {
            input.reply_failed("Cleanup allowance must be between 0.1 and 16 milliseconds");
            return;
        }
        if let Some(simulation) = args.simulation {
            settings.simulation = match simulation.as_str() {
                "ram" => ReclaimSimulation::Ram,
                "gpu" => ReclaimSimulation::Gpu,
                "both" => ReclaimSimulation::Both,
                _ => ReclaimSimulation::Off,
            };
        }
        if let Some(ms) = args.budget_ms {
            settings.frame_budget = std::time::Duration::from_secs_f64(ms / 1000.0);
        }
        input.reply_ok("Session reclamation settings updated; pressure applies next frame");
        return;
    }
    input.reply_ok(serde_json::json!({
        "ram": state.ram.map(|tier| format!("{tier:?}")),
        "gpu": state.gpu.map(|tier| format!("{tier:?}")),
        "ramActive": state.ram_active, "gpuActive": state.gpu_active,
        "simulation": format!("{:?}", settings.simulation).to_lowercase(),
        "controlsAvailable": preview.is_some_and(|preview| preview.is_preview),
        "budgetMs": settings.frame_budget.as_secs_f64() * 1000.0,
        "elapsedMs": budget.elapsed.as_secs_f64() * 1000.0,
        "budgetExhausted": budget.budget_exhausted,
        "policy": "Start above 70%; stop below 65%, independently per domain. Cooperative allowance; deferred asset destruction is outside this timing. Counts are cache pins, not freed bytes.",
        "caches": budget.caches().iter().map(|cache| serde_json::json!({
            "name": cache.spec.name, "domains": format!("{:?}", cache.spec.domains),
            "entries": cache.entries, "released": cache.released, "inspected": cache.inspected,
            "lastReleased": cache.last_released, "lastInspected": cache.last_inspected,
            "scanStatus": cache.scan_status.as_str(), "completedScans": cache.completed_scans,
            "lastScanReleased": cache.last_scan_released,
            "releaseLimit": cache.spec.max_releases, "inspectionLimit": cache.spec.max_inspections,
        })).collect::<Vec<_>>(),
        "unavailable": { "ram": "No usable engine RAM reading.", "gpu": "No usable GPU budget; this platform or renderer may not expose one." },
    }).to_string());
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_console::ConsoleCommandEntered;
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };

    fn app(preview: Option<bool>) -> App {
        let mut app = App::new();
        app.add_plugins(console::ConsolePlugin {
            add_bevy_console: false,
        })
        .init_resource::<MemoryReclaimSettings>()
        .init_resource::<MemoryReclaimState>()
        .init_resource::<ReclaimBudget>();
        if let Some(is_preview) = preview {
            app.insert_resource(PreviewMode {
                is_preview,
                ..default()
            });
        }
        register(&mut app);
        app.update();
        app
    }

    fn command(app: &mut App, args: &[&str]) -> Result<String, String> {
        let reply = Arc::new(Mutex::new(None));
        let output = reply.clone();
        app.world_mut().send_event(ConsoleCommandEntered {
            command_name: "/memory_reclaim".into(),
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
    fn mutations_require_preview_while_read_only_queries_remain_available() {
        for preview in [None, Some(false)] {
            let mut app = app(preview);
            for args in [
                vec!["both"],
                vec!["off"],
                vec!["--budget-ms", "1.5"],
                vec!["both", "--budget-ms", "1.5"],
            ] {
                let error = command(&mut app, &args).unwrap_err();
                assert!(error.contains("require preview mode"), "{error}");
                let settings = app.world().resource::<MemoryReclaimSettings>();
                assert_eq!(settings.simulation, ReclaimSimulation::Off);
                assert_eq!(settings.frame_budget, Duration::from_millis(1));
            }
            let query: serde_json::Value =
                serde_json::from_str(&command(&mut app, &[]).unwrap()).unwrap();
            assert_eq!(query["controlsAvailable"], false);
            assert_eq!(query["simulation"], "off");
            assert_eq!(query["budgetMs"], 1.0);
        }
    }

    #[test]
    fn preview_budget_validation_is_atomic_with_simulation_changes() {
        let mut app = app(Some(true));
        command(&mut app, &["ram", "--budget-ms", "1.5"]).unwrap();
        let settings = app.world().resource::<MemoryReclaimSettings>();
        assert_eq!(settings.simulation, ReclaimSimulation::Ram);
        assert_eq!(settings.frame_budget, Duration::from_micros(1500));

        for invalid in ["NaN", "inf", "-inf", "-1", "0", "16.1"] {
            let budget_arg = format!("--budget-ms={invalid}");
            let error = command(&mut app, &["both", &budget_arg]).unwrap_err();
            assert!(error.contains("between 0.1 and 16"), "{invalid}: {error}");
            let settings = app.world().resource::<MemoryReclaimSettings>();
            assert_eq!(settings.simulation, ReclaimSimulation::Ram, "{invalid}");
            assert_eq!(
                settings.frame_budget,
                Duration::from_micros(1500),
                "{invalid}"
            );
        }

        for (value, expected) in [
            ("0.1", Duration::from_micros(100)),
            ("16", Duration::from_millis(16)),
        ] {
            command(&mut app, &["both", "--budget-ms", value]).unwrap();
            let settings = app.world().resource::<MemoryReclaimSettings>();
            assert_eq!(settings.simulation, ReclaimSimulation::Both);
            assert_eq!(settings.frame_budget, expected);
        }
        let query: serde_json::Value =
            serde_json::from_str(&command(&mut app, &[]).unwrap()).unwrap();
        assert_eq!(query["controlsAvailable"], true);
        assert_eq!(query["simulation"], "both");
        assert_eq!(query["budgetMs"], 16.0);
    }
}
