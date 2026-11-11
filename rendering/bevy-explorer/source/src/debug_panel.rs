//! The engine's own debug panel sections: Performance, Render, Profiler and Memory and limits.
//! Each query builds its JSON only when asked.

use std::collections::BTreeMap;

use bevy::{
    diagnostic::{Diagnostic, DiagnosticPath, DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::*,
    render::renderer::RenderAdapterInfo,
};
use bevy_console::ConsoleCommand;
use common::{
    gpu_time::GPU_FRAME_TIME,
    memory::{GpuMemoryBudget, MemoryStatus},
    perf::{
        FrameTimeBreakdown, FrameTimeSampleSummary, InProgressSample, PerformanceSnapshot,
        SamplingStatus, PERF_ALLOCATED_BYTES_PER_FRAME, PERF_ALLOCATION_COUNT_PER_FRAME,
        PERF_FRAME_LIMITER_SLEEP_MS, PERF_MAIN_WORLD_SCHEDULES_MS, PERF_RENDER_WORLD_CPU_MS,
        PERF_SCENE_RUNTIME_LOOP_MS, PERF_SWAPCHAIN_PRESENT_WAIT_MS,
    },
    structs::{AppConfig, PrimaryUser, SessionGraphics},
};
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};
use scene_material::SceneMaterial;
use scene_runner::{
    renderer_context::RendererSceneContext, visible_content_bytes::scene_visible_content_bytes,
    ContainerEntity, ContainingScene,
};
use serde_json::{json, Value};

use crate::perf::{
    in_progress_sample::{latest_entity_count, sum_scene_js_heaps, InProgressSampleMeasurements},
    profiled_scopes::{ProfiledScopeSortMetric, ProfiledScopesTable},
};

const BYTES_PER_MIB: u64 = 1 << 20;
const WAITING_FOR_FIRST_SNAPSHOT: &str = "Waiting for the first 15 s of sampled time.";

pub(super) fn register(app: &mut App) {
    app.add_console_command::<MemoryCommand, _>(debug_memory)
        .add_debug_section(DebugSection {
            title: "Memory and limits",
            query: "/debug_memory",
            fields: &[
                ("Engine memory (MiB)", "used"),
                (
                    "Peak engine memory in the sample (MiB)",
                    "usedPeakMiBInSample",
                ),
                ("Platform capacity (MiB)", "capacity"),
                ("Effective cap (MiB)", "cap"),
                ("Memory pressure", "pressure"),
                ("Thresholds (MiB)", "thresholds"),
                ("GPU memory pools", "gpu"),
                (
                    "JS heaps (used MiB / limit MiB / external MiB / scenes)",
                    "jsHeap",
                ),
                ("Current scene textures (count / MiB)", "textures"),
                ("Current scene meshes (count / MiB)", "meshes"),
            ],
            actions: &[
                ("Restore platform limit", "/debug_memory 0"),
                (
                    "Simulate Warning pressure",
                    "/debug_memory --simulate warning",
                ),
                ("Simulate Full pressure", "/debug_memory --simulate full"),
                ("Stop simulating", "/debug_memory --simulate off"),
            ],
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
                ("GPU frame time (ms)", "gpuFrameMs"),
                ("Target frame rate (FPS)", "targetFps"),
                ("Sampling", "sampling.status"),
                ("Sampling paused because", "sampling.pausedReason"),
                ("Sampled seconds in progress", "inProgressSample.sampledSeconds"),
                ("Snapshot duration (s)", "snapshot.durationSeconds"),
                ("Snapshot taken at (s since launch)", "snapshot.takenAtRealSeconds"),
                (
                    "CPU frame time (min / mean / p50 / p95 / p99 / max, ms)",
                    "cpuFrameTimes.summary",
                ),
                (
                    "CPU frames over the hiccup threshold (count / threshold ms)",
                    "cpuFrameTimes.hiccups",
                ),
                (
                    "GPU frame time (min / mean / p50 / p95 / p99 / max, ms)",
                    "gpuFrameTimes.summary",
                ),
                (
                    "GPU frames over the hiccup threshold (count / threshold ms)",
                    "gpuFrameTimes.hiccups",
                ),
                (
                    "Frame time breakdown (main world / scene loop / sleep / render CPU / present wait / GPU, ms)",
                    "frameTimeBreakdown",
                ),
                ("Bottleneck", "bottleneck"),
                (
                    "HUD main thread, last window (React commits / commit ms / long frames / HUD script ms / style+layout ms)",
                    "hudMainThreadCost",
                ),
                ("Entities", "entityCount"),
                ("CPU usage (system % / process %)", "system.cpuUsage"),
                ("System (OS / CPU / cores / RAM)", "system.info"),
                ("Engine memory (MiB)", "engineMemoryMiB"),
            ],
            actions: &[],
        });
    app.add_console_command::<RenderStateCommand, _>(render_state)
        .add_debug_section(DebugSection {
            title: "Render",
            query: "/render_state",
            fields: &[
                ("Adapter (name / backend / driver)", "adapter"),
                ("Render passes (CPU ms / GPU ms)", "passSummary"),
                (
                    "Pipeline statistics (vertex / fragment shader invocations)",
                    "pipeline",
                ),
            ],
            actions: &[],
            inputs: &[],
        });
    app.add_console_command::<ProfileTopCommand, _>(profile_top)
        .add_console_command::<ProfileEnableCommand, _>(profile_enable)
        .add_debug_section(DebugSection {
            title: "Profiler",
            query: "/profile_top",
            fields: &[
                ("Profiler", "enabled"),
                ("Sorted by", "sort"),
                (
                    "Top scopes (kind, calls, self ms, inclusive ms, max call ms, smoothed ms per frame)",
                    "scopeSummary",
                ),
                ("Allocations per frame (count / bytes)", "allocations"),
            ],
            actions: &[("Reset the table", "/profile_enable reset")],
            inputs: &[DebugInput {
                label: "Recording",
                field: "enabled",
                command: "/profile_enable {value}",
                kind: DebugInputKind::ActionButton {
                    enable: ("Enable profiler", "on"),
                    disable: ("Disable profiler", "off"),
                },
            }],
        });
}

/// Report the live engine readings, the latest performance snapshot, the frame-time breakdown
/// and the effective session frame-rate target as JSON.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/performance_state")]
struct PerformanceStateCommand;

#[allow(clippy::too_many_arguments)]
fn performance_state(
    mut input: ConsoleCommand<PerformanceStateCommand>,
    diagnostics: Res<DiagnosticsStore>,
    time: Res<Time<Real>>,
    session: Res<SessionGraphics>,
    config: Res<AppConfig>,
    memory: Res<MemoryStatus>,
    status: Res<SamplingStatus>,
    in_progress: Res<InProgressSample>,
    snapshot: Option<Res<PerformanceSnapshot>>,
    breakdown: Res<FrameTimeBreakdown>,
    #[cfg(any(windows, target_os = "linux"))] system_info: Option<
        Res<bevy::diagnostic::SystemInfo>,
    >,
    #[cfg(target_arch = "wasm32")] hud_cost: Res<crate::web::LatestHudMainThreadCostWindow>,
) {
    if input.take().is_none_or(|parsed| parsed.is_err()) {
        return;
    }
    #[cfg(target_arch = "wasm32")]
    let hud_main_thread_cost = hud_cost.0;
    #[cfg(not(target_arch = "wasm32"))]
    let hud_main_thread_cost: Option<common::perf::HudMainThreadCostDataPoint> = None;
    let hundredths = |ms: f32| (f64::from(ms) * 100.0).round() / 100.0;
    let hud_main_thread_cost_cells = hud_main_thread_cost.map(|window| {
        let frames = window.long_animation_frames;
        [
            Some(f64::from(window.react_commit_count)),
            Some(hundredths(window.react_commit_time_sum_ms)),
            frames.map(|frames| f64::from(frames.frame_count)),
            frames.map(|frames| hundredths(frames.hud_script_time_sum_ms)),
            frames.map(|frames| hundredths(frames.style_and_layout_time_sum_ms)),
        ]
    });
    let smoothed = |path: &DiagnosticPath| diagnostics.get(path).and_then(Diagnostic::smoothed);
    let tenths = |ms: f64| (ms * 10.0).round() / 10.0;
    let fps = smoothed(&FrameTimeDiagnosticsPlugin::FPS).or_else(|| {
        let seconds = time.delta_secs_f64();
        (seconds > 0.0).then(|| seconds.recip())
    });
    let gpu = diagnostics.get(&GPU_FRAME_TIME);
    let gpu_ms = gpu.and_then(Diagnostic::smoothed);
    let frame_time_summary_json = |summary: &FrameTimeSampleSummary| {
        let values = [
            summary.min_ms,
            summary.mean_ms,
            summary.p50_ms,
            summary.p95_ms,
            summary.p99_ms,
            summary.max_ms,
        ]
        .map(|ms| tenths(f64::from(ms)));
        let hiccups = [
            f64::from(summary.frames_over_hiccup_threshold_count),
            tenths(f64::from(summary.hiccup_threshold_ms)),
        ];
        json!({
            "summary": values,
            "hiccups": hiccups,
            "sampleCount": summary.sample_count,
            "minMs": summary.min_ms, "maxMs": summary.max_ms, "meanMs": summary.mean_ms,
            "p50Ms": summary.p50_ms, "p95Ms": summary.p95_ms, "p99Ms": summary.p99_ms,
            "framesOverHiccupThresholdCount": summary.frames_over_hiccup_threshold_count,
            "framesOverHiccupThresholdTimeSumMs": summary.frames_over_hiccup_threshold_time_sum_ms,
            "hiccupThresholdMs": summary.hiccup_threshold_ms,
        })
    };
    let snapshot = snapshot.as_deref();
    let breakdown_smoothed = [
        smoothed(&PERF_MAIN_WORLD_SCHEDULES_MS),
        smoothed(&PERF_SCENE_RUNTIME_LOOP_MS),
        smoothed(&PERF_FRAME_LIMITER_SLEEP_MS),
        smoothed(&PERF_RENDER_WORLD_CPU_MS),
        smoothed(&PERF_SWAPCHAIN_PRESENT_WAIT_MS),
        gpu_ms,
    ]
    .map(|ms| ms.map(tenths));
    #[cfg(any(windows, target_os = "linux"))]
    let system = {
        use bevy::diagnostic::SystemInformationDiagnosticsPlugin;
        let cpu_usage = [
            smoothed(&SystemInformationDiagnosticsPlugin::SYSTEM_CPU_USAGE),
            smoothed(&SystemInformationDiagnosticsPlugin::PROCESS_CPU_USAGE),
        ]
        .map(|percent| percent.map(tenths));
        json!({
            "cpuUsage": cpu_usage[0].map(|_| cpu_usage),
            "info": system_info.as_deref().map(|info| {
                format!("{} / {} / {} / {}", info.os, info.cpu, info.core_count, info.memory)
            }),
        })
    };
    #[cfg(not(any(windows, target_os = "linux")))]
    let system = Value::Null;
    input.reply_ok(
        json!({
            "version": crate::version(),
            "frameMs": tenths(time.delta_secs_f64() * 1000.0),
            "fps": fps.map(f64::round),
            "targetFps": session.fps_target(&config),
            "overrideFps": session.fps_target,
            "limit30": session.fps_target == Some(30),
            "gpuFrameMs": gpu_ms.map(tenths),
            "sampling": {
                "status": match *status { SamplingStatus::Sampling => "sampling", SamplingStatus::Paused(_) => "paused" },
                "pausedReason": status.paused_reason(),
            },
            "inProgressSample": { "sampledSeconds": tenths(in_progress.sampled_seconds) },
            "snapshot": snapshot.map(|snapshot| json!({
                "durationSeconds": tenths(snapshot.snapshot_duration_seconds),
                "takenAtRealSeconds": tenths(snapshot.taken_at_real_seconds),
                "hudMainThreadCost": snapshot.hud_main_thread_cost,
            })),
            "cpuFrameTimes": snapshot.map(|snapshot| frame_time_summary_json(&snapshot.cpu_frame_times)),
            "gpuFrameTimes": snapshot.and_then(|snapshot| snapshot.gpu_frame_times.as_ref()).map(frame_time_summary_json),
            "frameTimeBreakdown": breakdown_smoothed,
            "bottleneck": breakdown.bottleneck,
            "hudMainThreadCost": hud_main_thread_cost_cells,
            "entityCount": latest_entity_count(&diagnostics),
            "system": system,
            "engineMemoryMiB": memory.reading().map(|reading| reading.used_bytes / BYTES_PER_MIB),
            "unavailable": {
                "fps": "Waiting for a frame.",
                "sampling.pausedReason": "Sampling.",
                "snapshot": WAITING_FOR_FIRST_SNAPSHOT,
                "cpuFrameTimes": WAITING_FOR_FIRST_SNAPSHOT,
                "gpuFrameTimes": if gpu.is_some() { WAITING_FOR_FIRST_SNAPSHOT } else { "GPU timestamps are unavailable on this renderer." },
                "gpuFrameMs": if gpu.is_some() { "Waiting for a GPU timestamp sample." } else { "GPU timestamps are unavailable on this renderer." },
                "bottleneck": "Waiting for the GPU and present readings the classification needs.",
                "hudMainThreadCost": "-",
                "entityCount": "Waiting for the first entity count.",
                "system": "No system information on this platform.",
                "engineMemoryMiB": "No platform memory sample.",
            },
        })
        .to_string(),
    );
}

/// Report the render adapter, every render pass's CPU and GPU time and the pipeline statistics
/// Bevy's render diagnostics record, as JSON.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/render_state")]
struct RenderStateCommand;

fn render_state(
    mut input: ConsoleCommand<RenderStateCommand>,
    diagnostics: Res<DiagnosticsStore>,
    adapter: Option<Res<RenderAdapterInfo>>,
) {
    if input.take().is_none_or(|parsed| parsed.is_err()) {
        return;
    }
    let hundredths = |ms: f64| (ms * 100.0).round() / 100.0;
    // `render/<span>/elapsed_cpu|elapsed_gpu|<statistic>`; a pass is (cpu ms, gpu ms)
    let mut passes: BTreeMap<&str, (Option<f64>, Option<f64>)> = BTreeMap::new();
    let mut vertex_invocations = None;
    let mut fragment_invocations = None;
    for diagnostic in diagnostics.iter() {
        let Some((span, field)) = diagnostic
            .path()
            .as_str()
            .strip_prefix("render/")
            .and_then(|rest| rest.rsplit_once('/'))
        else {
            continue;
        };
        let value = diagnostic.smoothed().map(hundredths);
        match field {
            "elapsed_cpu" => passes.entry(span).or_default().0 = value,
            "elapsed_gpu" => passes.entry(span).or_default().1 = value,
            "vertex_shader_invocations" => vertex_invocations = value,
            "fragment_shader_invocations" => fragment_invocations = value,
            _ => {}
        }
    }
    let format_ms_or_dash = |ms: Option<f64>| ms.map_or("-".to_owned(), |ms| format!("{ms:.2}"));
    let pass_summary: Vec<String> = passes
        .iter()
        .map(|(name, (cpu, gpu))| {
            format!(
                "{name}: {} / {}",
                format_ms_or_dash(*cpu),
                format_ms_or_dash(*gpu)
            )
        })
        .collect();
    input.reply_ok(
        json!({
            "adapter": adapter.as_deref().map(|adapter| {
                format!("{} / {:?} / {}", adapter.name, adapter.backend, adapter.driver)
            }),
            "passes": passes.iter().map(|(name, (cpu, gpu))| json!({
                "name": name, "cpuMs": cpu, "gpuMs": gpu,
            })).collect::<Vec<_>>(),
            "passSummary": (!pass_summary.is_empty()).then(|| pass_summary.join("\n")),
            "pipeline": vertex_invocations.map(|vertex| [Some(vertex), fragment_invocations]),
            "unavailable": {
                "adapter": "No render adapter in this world.",
                "passSummary": "No render pass has reported a time yet (Bevy's render diagnostics run on native only).",
                "pipeline": "Pipeline statistics need a Vulkan adapter.",
            },
        })
        .to_string(),
    );
}

/// Report the scope profiler's top scopes as JSON: `count` rows (20 by default) sorted by
/// `sort` (self time by default), plus the allocations per frame.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/profile_top")]
struct ProfileTopCommand {
    count: Option<usize>,
    #[arg(value_enum)]
    sort: Option<ProfiledScopeSortMetric>,
}

fn profile_top(
    mut input: ConsoleCommand<ProfileTopCommand>,
    table: Res<ProfiledScopesTable>,
    diagnostics: Res<DiagnosticsStore>,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    let sort = command.sort.unwrap_or(ProfiledScopeSortMetric::SelfTime);
    let rows = table.top_scope_summaries(command.count.unwrap_or(20), sort);
    let hundredths = |ms: f32| (f64::from(ms) * 100.0).round() / 100.0;
    let scope_summary: Vec<String> = rows
        .iter()
        .map(|row| {
            format!(
                "{}: {:?}, {}, {:.2}, {:.2}, {:.2}, {:.2}",
                row.scope_name,
                row.scope_kind,
                row.call_count,
                row.self_time_ms,
                row.inclusive_time_ms,
                row.max_single_call_ms,
                row.smoothed_self_time_per_frame_ms,
            )
        })
        .collect();
    let allocations = diagnostics
        .get(&PERF_ALLOCATION_COUNT_PER_FRAME)
        .and_then(Diagnostic::smoothed)
        .map(|count| {
            [
                Some(count.round()),
                diagnostics
                    .get(&PERF_ALLOCATED_BYTES_PER_FRAME)
                    .and_then(Diagnostic::smoothed)
                    .map(f64::round),
            ]
        });
    input.reply_ok(
        json!({
            "enabled": table.recording_enabled(),
            "sort": format!("{sort:?}"),
            "scopeSummaries": rows.iter().map(|row| json!({
                "scopeName": row.scope_name, "scopeKind": row.scope_kind, "callCount": row.call_count,
                "selfTimeMs": hundredths(row.self_time_ms), "inclusiveTimeMs": hundredths(row.inclusive_time_ms),
                "maxSingleCallMs": hundredths(row.max_single_call_ms),
                "smoothedSelfTimePerFrameMs": hundredths(row.smoothed_self_time_per_frame_ms),
            })).collect::<Vec<_>>(),
            "scopeSummary": (!scope_summary.is_empty()).then(|| scope_summary.join("\n")),
            "allocations": allocations,
            "unavailable": {
                "scopeSummary": if table.recording_enabled() { "No scope has been recorded yet." } else { "Profiler disabled: enable it to record scopes." },
                "allocations": "Waiting for the first frame's allocation counts.",
            },
        })
        .to_string(),
    );
}

/// Start (`on`) or stop (`off`) the scope profiler's recording, or `reset` its table.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/profile_enable")]
struct ProfileEnableCommand {
    #[arg(value_enum)]
    action: ProfilerRecordingAction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
enum ProfilerRecordingAction {
    On,
    Off,
    Reset,
}

fn profile_enable(
    mut input: ConsoleCommand<ProfileEnableCommand>,
    mut table: ResMut<ProfiledScopesTable>,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    match command.action {
        ProfilerRecordingAction::On => table.set_recording_enabled(true),
        ProfilerRecordingAction::Off => table.set_recording_enabled(false),
        ProfilerRecordingAction::Reset => table.reset(),
    }
    input.reply_ok(json!({ "enabled": table.recording_enabled() }).to_string());
}

/// Inspect the engine memory budget, set its session limit in MiB (zero restores platform
/// capacity), or simulate a pressure tier by setting the limit from the current use.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/debug_memory")]
struct MemoryCommand {
    limit_mib: Option<u32>,
    #[arg(long, value_enum)]
    simulate: Option<SimulatedMemoryPressure>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
enum SimulatedMemoryPressure {
    /// A cap that puts the current use at 72 %: the Warning tier.
    Warning,
    /// A cap that puts the current use at 80 %: the Full tier.
    Full,
    /// The platform's capacity again.
    Off,
}

#[allow(clippy::too_many_arguments)]
fn debug_memory(
    mut cmd: ConsoleCommand<MemoryCommand>,
    mut memory: ResMut<MemoryStatus>,
    gpu: Option<Res<GpuMemoryBudget>>,
    frame_times: Res<InProgressSampleMeasurements>,
    scenes: Query<&RendererSceneContext>,
    containing_scene: ContainingScene,
    player: Query<Entity, With<PrimaryUser>>,
    mesh_handles: Query<(&Mesh3d, &ContainerEntity, &Visibility)>,
    material_handles: Query<(
        &MeshMaterial3d<SceneMaterial>,
        &ContainerEntity,
        &Visibility,
    )>,
    meshes: Res<Assets<Mesh>>,
    materials: Res<Assets<SceneMaterial>>,
    images: Res<Assets<Image>>,
) {
    let Some(Ok(args)) = cmd.take() else { return };
    if let Some(simulate) = args.simulate {
        let used_mib = memory
            .reading()
            .map(|reading| reading.used_bytes / BYTES_PER_MIB);
        let limit = match (simulate, used_mib) {
            (SimulatedMemoryPressure::Off, _) => None,
            (_, None) => {
                cmd.reply_failed("No platform memory sample to simulate from");
                return;
            }
            (SimulatedMemoryPressure::Warning, Some(used)) => {
                Some((used as f64 / 0.72).ceil() as u64)
            }
            (SimulatedMemoryPressure::Full, Some(used)) => Some((used as f64 / 0.80).ceil() as u64),
        };
        memory.limit_mib = limit.and_then(std::num::NonZeroU64::new);
        cmd.reply_ok(
            json!({
                "limit": memory.limit_mib.map_or(0, |n| n.get()),
                "pressure": memory.reading().map(|r| format!("{:?}", r.pressure())),
            })
            .to_string(),
        );
        return;
    }
    if let Some(limit) = args.limit_mib {
        memory.limit_mib = std::num::NonZeroU64::new(limit.into());
        cmd.reply_ok("Updated session memory limit");
        return;
    }
    let reading = memory.reading();
    let pools = match gpu.as_deref() {
        Some(GpuMemoryBudget::Available(pools)) => Some(pools.iter().map(|p| json!({
            "pool": format!("{:?}", p.kind), "usedMiB": p.used_bytes >> 20, "budgetMiB": p.budget_bytes.get() >> 20
        })).collect::<Vec<_>>()),
        _ => None,
    };
    let js_heap = sum_scene_js_heaps(scenes.iter());
    let current_scene_content = player
        .single()
        .ok()
        .and_then(|player| containing_scene.get_parcel(player))
        .map(|scene| {
            scene_visible_content_bytes(
                scene,
                mesh_handles.iter(),
                material_handles.iter(),
                &meshes,
                &materials,
                &images,
            )
        });
    let count_and_mib = |count: u32, bytes: u64| -> Value {
        json!([
            count,
            (bytes as f64 / BYTES_PER_MIB as f64 * 10.0).round() / 10.0
        ])
    };
    let ram_reason = "No platform memory sample.";
    cmd.reply_ok(json!({
        "used": reading.map(|r| r.used_bytes >> 20), "capacity": reading.map(|r| r.capacity_mib),
        "usedPeakMiBInSample": reading
            .and((frame_times.used_peak_bytes_in_sample > 0).then_some(()))
            .map(|_| frame_times.used_peak_bytes_in_sample >> 20),
        "cap": reading.map(|r| r.cap_mib), "limit": memory.limit_mib.map_or(0, |n| n.get()),
        "pressure": reading.map(|r| format!("{:?}", r.pressure())), "gpu": pools,
        "thresholds": reading.map(|r| { let t = r.thresholds(); json!({
            "abundance": t.abundance_mib.round(), "warning": t.warning_mib.round(), "full": t.full_mib.round()
        }) }),
        "jsHeap": js_heap.map(|heap| json!([
            heap.used_bytes_sum >> 20, heap.limit_bytes_sum >> 20, heap.external_bytes_sum >> 20, heap.reporting_scene_count
        ])),
        "textures": current_scene_content.map(|c| count_and_mib(c.texture_count, c.texture_bytes_sum)),
        "meshes": current_scene_content.map(|c| count_and_mib(c.mesh_count, c.mesh_bytes_sum)),
        "unavailable": {
            "used":ram_reason, "capacity":ram_reason, "cap":ram_reason,
            "usedPeakMiBInSample": if reading.is_some() { "Waiting for the first sampled frame." } else { ram_reason },
            "pressure":ram_reason, "thresholds":ram_reason,
            "gpu":match gpu.as_deref() {
                Some(GpuMemoryBudget::NotApplicable) => "This platform does not expose a GPU memory budget.",
                _ => "The graphics driver has not provided a GPU memory budget.",
            },
            "jsHeap": if cfg!(target_arch = "wasm32") { "No running scene reports a heap: this browser has no memory API." } else { "No running scene has reported a heap yet." },
            "textures": "The player is not on a parcel scene.",
            "meshes": "The player is not on a parcel scene.",
        }
    }).to_string());
}
