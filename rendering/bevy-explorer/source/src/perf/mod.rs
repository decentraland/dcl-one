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
//! - [`allocation_counter`]: allocations per frame.
//!
//! Bevy's spans and the allocation counter exist only under the `profiler` cargo feature (every
//! native build); the rest measures in every build.

#[cfg(feature = "profiler")]
pub mod allocation_counter;
pub mod benchmark_summary;
pub mod frame_inputs;
pub mod frame_time_breakdown;
pub mod performance_issue_log;
pub mod populating_sample;
pub mod profiled_scopes;

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

pub struct PerformanceMonitoringPlugin;

impl Plugin for PerformanceMonitoringPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(frame_inputs::FrameInputs::new())
            .insert_resource(populating_sample::PerformancePopulatingSample::new())
            .insert_resource(profiled_scopes::ProfiledScopesTable::new())
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
                    profiled_scopes::fold_profiled_scopes_system,
                    // in `Last`, so a phase set in `Update` is seen in the same frame
                    benchmark_summary::accumulate_benchmark_measured_phase_system,
                    frame_time_breakdown::mark_main_world_frame_end_system,
                )
                    .chain(),
            );
        #[cfg(feature = "profiler")]
        app.add_systems(Last, allocation_counter::publish_frame_allocations_system);
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
