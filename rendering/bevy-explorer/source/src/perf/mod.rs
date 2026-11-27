//! Performance monitoring: what the explorer measures about itself. The types are in
//! `common::perf`; this module fills them and publishes the `perf/*` diagnostics.
//!
//! - [`frame_inputs`]: the frame's raw arrivals, gathered once in `First`: the entry point of
//!   every measurement below;
//! - [`populating_sample`]: the sampling status and the sample being populated, which becomes a
//!   `PerformanceSnapshot` every 15 s of sampled time;
//! - [`frame_time_breakdown`]: where the previous frame's wall time went;
//! - [`profiled_scopes`]: the in-app scope profiler over Bevy's tracing spans;
//! - [`performance_issue_log`]: the warn-level signal for a long frame or a slow sample;
//! - [`benchmark_summary`]: the autopilot run's `perf_summary` log lines, from the same inputs;
//! - [`allocation_counter`]: allocations per frame;
//! - [`debug_panel`], [`profiler_debug_panel`]: the Performance section, and the Render and
//!   Profiler sections.
//!
//! The scope profiler, the allocation counter and the Render and Profiler sections exist only
//! under the `performance-profiler` cargo feature, which release builds leave out; the rest
//! measures in every build.

#[cfg(feature = "performance-profiler")]
pub mod allocation_counter;
pub mod benchmark_summary;
mod debug_panel;
pub mod frame_inputs;
pub mod frame_time_breakdown;
pub mod performance_issue_log;
pub mod populating_sample;
#[cfg(feature = "performance-profiler")]
pub mod profiled_scopes;
#[cfg(feature = "performance-profiler")]
mod profiler_debug_panel;

use bevy::{
    diagnostic::{Diagnostic, RegisterDiagnostic},
    prelude::*,
    time::TimeSystem,
};
use common::perf::{
    FrameTimeBreakdown, PerformanceSnapshot, PERF_ALLOCATED_BYTES_PER_FRAME,
    PERF_ALLOCATION_COUNT_PER_FRAME, PERF_FRAME_LIMITER_SLEEP_MS, PERF_MAIN_WORLD_SCHEDULES_MS,
    PERF_RENDER_WORLD_CPU_MS, PERF_SCENE_RUNTIME_LOOP_MS, PERF_SWAPCHAIN_PRESENT_WAIT_MS,
};
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};

/// The measurement and its debug panel sections.
pub struct PerformanceMonitoringPlugin;

impl Plugin for PerformanceMonitoringPlugin {
    fn build(&self, app: &mut App) {
        use debug_panel::{performance_state, PerformanceStateCommand};
        app.add_plugins(PerformanceMeasurementPlugin);
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
                    ("VSync", "vsync"),
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
        #[cfg(feature = "performance-profiler")]
        {
            use profiler_debug_panel::{
                profile_enable, profile_top, render_state, ProfileEnableCommand, ProfileTopCommand,
                RenderStateCommand,
            };
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
    }
}

/// The measurement, apart from the debug panel so a test can build it alone.
pub(crate) struct PerformanceMeasurementPlugin;

impl Plugin for PerformanceMeasurementPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<common::perf::FocusPause>()
            .insert_resource(frame_inputs::FrameInputs::new())
            .insert_resource(populating_sample::PerformancePopulatingSample::new())
            .insert_resource(frame_time_breakdown::MainWorldFrameTimer::new())
            .insert_resource(FrameTimeBreakdown {
                whole_frame_wall_ms: 0.0,
                main_world_schedules_ms: 0.0,
                scene_runtime_loop_ms: 0.0,
                frame_limiter_sleep_ms: 0.0,
                render_world_cpu_ms: 0.0,
                swapchain_present_wait_ms: None,
                gpu_frame_time_ms: None,
                scene_ticks_run_count: 0,
                scene_jobs_left_count: 0,
                bottleneck: None,
            })
            .add_event::<PerformanceSnapshot>()
            .register_diagnostic(Diagnostic::new(PERF_MAIN_WORLD_SCHEDULES_MS).with_suffix("ms"))
            .register_diagnostic(Diagnostic::new(PERF_SCENE_RUNTIME_LOOP_MS).with_suffix("ms"))
            .register_diagnostic(Diagnostic::new(PERF_FRAME_LIMITER_SLEEP_MS).with_suffix("ms"))
            .register_diagnostic(Diagnostic::new(PERF_RENDER_WORLD_CPU_MS).with_suffix("ms"))
            .register_diagnostic(Diagnostic::new(PERF_SWAPCHAIN_PRESENT_WAIT_MS).with_suffix("ms"))
            .register_diagnostic(Diagnostic::new(PERF_ALLOCATION_COUNT_PER_FRAME))
            .register_diagnostic(Diagnostic::new(PERF_ALLOCATED_BYTES_PER_FRAME).with_suffix("B"))
            .add_systems(
                First,
                (
                    frame_time_breakdown::mark_main_world_frame_start_system.before(TimeSystem),
                    frame_inputs::collect_frame_inputs_system.after(TimeSystem),
                ),
            )
            .add_systems(
                PreUpdate,
                frame_time_breakdown::publish_frame_time_breakdown_system,
            )
            .add_systems(
                Update,
                (
                    populating_sample::advance_populating_sample_system,
                    performance_issue_log::log_performance_issue_system,
                )
                    .chain(),
            )
            .add_systems(
                Last,
                (
                    // in `Last`, so a phase set in `Update` is seen in the same frame
                    benchmark_summary::accumulate_benchmark_measured_phase_system,
                    frame_time_breakdown::mark_main_world_frame_end_system,
                )
                    .chain(),
            );
        #[cfg(feature = "performance-profiler")]
        app.insert_resource(profiled_scopes::ProfiledScopesTable::new())
            .add_systems(
                Last,
                (
                    profiled_scopes::fold_profiled_scopes_system
                        .before(benchmark_summary::accumulate_benchmark_measured_phase_system),
                    allocation_counter::publish_frame_allocations_system,
                )
                    .before(frame_time_breakdown::mark_main_world_frame_end_system),
            );
        // publishes the `pipelined/*` timings, among them the render worker's wait for the next
        // animation frame
        #[cfg(target_arch = "wasm32")]
        app.add_plugins(bevy::render::pipelined_rendering::PipelinedRenderingDiagnosticsPlugin);
    }

    /// Adds the render-world timers to the render app.
    fn finish(&self, app: &mut App) {
        frame_time_breakdown::add_render_world_timers(app);
    }
}
