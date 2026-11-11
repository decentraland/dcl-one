//! Performance monitoring: what the explorer measures about itself. The types are in
//! `common::perf`; this module fills them and publishes the `perf/*` diagnostics.
//!
//! - [`in_progress_sample`]: the sampling status and the 15 s samples that become
//!   `PerformanceSnapshot`s;
//! - [`frame_time_breakdown`]: where the previous frame's wall time went;
//! - [`profiled_scopes`]: the in-app scope profiler over Bevy's tracing spans;
//! - [`performance_issue_log`]: the warn-level signal for a long frame or a slow sample;
//! - [`benchmark_summary`]: the autopilot run's `perf_summary` log lines;
//! - [`allocation_counter`]: allocations per frame.

pub mod allocation_counter;
pub mod benchmark_summary;
pub mod frame_time_breakdown;
pub mod in_progress_sample;
pub mod performance_issue_log;
pub mod profiled_scopes;

use bevy::{
    diagnostic::{Diagnostic, RegisterDiagnostic},
    prelude::*,
    time::TimeSystem,
};
use common::perf::{
    FrameTimeBreakdown, InProgressSample, PerformanceSnapshot, SamplingPausedReason,
    SamplingStatus, PERF_ALLOCS_COUNT_PER_FRAME, PERF_ALLOC_BYTES_PER_FRAME,
    PERF_FRAME_LIMITER_SLEEP_MS, PERF_MAIN_WORLD_SCHEDULES_MS, PERF_RENDER_WORLD_CPU_MS,
    PERF_SCENE_RUNTIME_LOOP_MS, PERF_SWAPCHAIN_PRESENT_WAIT_MS,
};

pub struct PerformanceMonitoringPlugin;

impl Plugin for PerformanceMonitoringPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SamplingStatus::Paused(SamplingPausedReason::NotInWorld))
            .insert_resource(InProgressSample::new())
            .insert_resource(in_progress_sample::InProgressSampleFrameTimes::new())
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
            .register_diagnostic(Diagnostic::new(PERF_ALLOCS_COUNT_PER_FRAME))
            .register_diagnostic(Diagnostic::new(PERF_ALLOC_BYTES_PER_FRAME).with_suffix("B"))
            .add_systems(
                First,
                (
                    frame_time_breakdown::mark_main_world_frame_start_system.before(TimeSystem),
                    in_progress_sample::update_sampling_status_system.after(TimeSystem),
                ),
            )
            .add_systems(
                PreUpdate,
                frame_time_breakdown::publish_frame_time_breakdown_system,
            )
            .add_systems(
                Update,
                (
                    in_progress_sample::sample_frame_times_system,
                    performance_issue_log::log_performance_issue_system,
                    benchmark_summary::accumulate_benchmark_run_system,
                )
                    .chain(),
            )
            .add_systems(
                Last,
                (
                    profiled_scopes::fold_profiled_scopes_system,
                    allocation_counter::publish_frame_allocations_system,
                    frame_time_breakdown::mark_main_world_frame_end_system,
                )
                    .chain(),
            );
    }

    /// The render-world timers need the render app, which exists once the render plugin has
    /// finished (on the web, `finish` runs on the render worker).
    fn finish(&self, app: &mut App) {
        frame_time_breakdown::add_render_world_timers(app);
    }
}
