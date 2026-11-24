//! The Performance debug panel section's query. It builds its JSON only when asked.

use bevy::{
    diagnostic::{Diagnostic, DiagnosticPath, DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::*,
};
use bevy_console::ConsoleCommand;
use common::{
    gpu_time::{GpuFrameTimeUnreliable, GPU_FRAME_TIME},
    memory::MemoryStatus,
    perf::{
        FrameTimeBreakdown, FrameTimeSampleSummary, PerformanceSnapshot, SamplingStatus,
        PERF_FRAME_LIMITER_SLEEP_MS, PERF_MAIN_WORLD_SCHEDULES_MS, PERF_RENDER_WORLD_CPU_MS,
        PERF_SCENE_RUNTIME_LOOP_MS, PERF_SWAPCHAIN_PRESENT_WAIT_MS,
    },
    structs::{AppConfig, SessionGraphics},
};
use scene_runner::SceneLoopTiming;
use serde_json::json;

use super::{frame_inputs::FrameInputs, populating_sample::PerformancePopulatingSample};

const BYTES_PER_MIB: u64 = 1 << 20;
const WAITING_FOR_FIRST_SNAPSHOT: &str = "Waiting for the first 15 s of sampled time.";
const GPU_FRAME_TIME_UNRELIABLE: &str = "GPU frame time is unreliable on this platform.";

/// Report the live engine readings, the latest performance snapshot, the frame-time breakdown
/// and the effective session frame-rate target as JSON.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/performance_state")]
pub(super) struct PerformanceStateCommand;

pub(super) fn performance_state(
    mut input: ConsoleCommand<PerformanceStateCommand>,
    diagnostics: Res<DiagnosticsStore>,
    time: Res<Time<Real>>,
    session: Res<SessionGraphics>,
    config: Res<AppConfig>,
    memory: Res<MemoryStatus>,
    sample: Res<PerformancePopulatingSample>,
    inputs: Res<FrameInputs>,
    snapshot: Option<Res<PerformanceSnapshot>>,
    breakdown: Res<FrameTimeBreakdown>,
    scene_loop: Option<Res<SceneLoopTiming>>,
    gpu_frame_time_unreliable: Option<Res<GpuFrameTimeUnreliable>>,
    #[cfg(all(feature = "performance-profiler", any(windows, target_os = "linux")))]
    system_info: Option<Res<bevy::diagnostic::SystemInfo>>,
) {
    if input.take().is_none_or(|parsed| parsed.is_err()) {
        return;
    }
    let hud_main_thread_cost = inputs.last_hud_main_thread_cost_window;
    let hundredths = |ms: f32| (f64::from(ms) * 100.0).round() / 100.0;
    let hud_main_thread_cost_cells = hud_main_thread_cost.map(|window| {
        let commits = window.react_commits;
        let frames = window.long_animation_frames;
        [
            commits.map(|commits| f64::from(commits.commit_count)),
            commits.map(|commits| hundredths(commits.commit_time_sum_ms)),
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
    #[cfg(all(feature = "performance-profiler", any(windows, target_os = "linux")))]
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
    #[cfg(not(all(feature = "performance-profiler", any(windows, target_os = "linux"))))]
    let system = serde_json::Value::Null;
    input.reply_ok(
        json!({
            "version": crate::version(),
            "frameMs": tenths(time.delta_secs_f64() * 1000.0),
            "fps": fps.map(f64::round),
            // the rate the scene loop paces to, so with VSync on and Max picked, the monitor's
            "targetFps": SessionGraphics::shown_fps(
                Some(&session),
                &config,
                scene_loop.and_then(|timing| timing.refresh_hz),
            ),
            "overrideFps": session.fps_target,
            "limit30": session.fps_target == Some(30),
            "gpuFrameMs": gpu_ms.map(tenths),
            "sampling": {
                "status": match sample.status() { SamplingStatus::Sampling => "sampling", SamplingStatus::Paused(_) => "paused" },
                "pausedReason": sample.status().paused_reason(),
            },
            "inProgressSample": { "sampledSeconds": tenths(sample.sampled_seconds()) },
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
            "entityCount": inputs.entity_count,
            "system": system,
            "engineMemoryMiB": memory.reading().map(|reading| reading.used_bytes / BYTES_PER_MIB),
            "unavailable": {
                "fps": "Waiting for a frame.",
                "sampling.pausedReason": "Sampling.",
                "snapshot": WAITING_FOR_FIRST_SNAPSHOT,
                "cpuFrameTimes": WAITING_FOR_FIRST_SNAPSHOT,
                "gpuFrameTimes": if gpu_frame_time_unreliable.is_some() { GPU_FRAME_TIME_UNRELIABLE } else if gpu.is_some() { WAITING_FOR_FIRST_SNAPSHOT } else { "GPU timestamps are unavailable on this renderer." },
                "gpuFrameMs": if gpu.is_some() { "Waiting for a GPU timestamp sample." } else { "GPU timestamps are unavailable on this renderer." },
                "bottleneck": if gpu_frame_time_unreliable.is_some() { GPU_FRAME_TIME_UNRELIABLE } else { "Waiting for the GPU and present readings the classification needs." },
                "hudMainThreadCost": "-",
                "entityCount": "Waiting for the first entity count.",
                "system": if !cfg!(any(windows, target_os = "linux")) {
                    "No system information on this platform."
                } else if cfg!(feature = "performance-profiler") {
                    "Waiting for Bevy's first system information reading."
                } else {
                    "Bevy's system information needs the `performance-profiler` cargo feature."
                },
                "engineMemoryMiB": "No platform memory sample.",
            },
        })
        .to_string(),
    );
}
