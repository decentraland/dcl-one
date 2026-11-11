//! The warn-level signal of a performance issue while sampling: one frame of a second or more,
//! or a completed sample whose mean frame rate is under 15 FPS. One line per sample, under the
//! target `perf`, carrying the frame-time breakdown.

use bevy::prelude::*;
use common::perf::{FrameTimeBreakdown, PerformanceSnapshot, SamplingStatus};

pub const PERFORMANCE_ISSUE_LOG_TARGET: &str = "perf";
pub const LONG_FRAME_THRESHOLD_MS: f32 = 1000.0;
pub const LOW_MEAN_FPS_THRESHOLD: f32 = 15.0;

/// Logs at most one issue per sample.
pub fn log_performance_issue_system(
    status: Res<SamplingStatus>,
    time: Res<Time<Real>>,
    breakdown: Res<FrameTimeBreakdown>,
    mut snapshots: EventReader<PerformanceSnapshot>,
    mut logged_this_sample: Local<bool>,
) {
    let mut sample_completed = false;
    for snapshot in snapshots.read() {
        sample_completed = true;
        let mean_fps = snapshot.cpu_frame_times.mean_fps();
        if !*logged_this_sample && mean_fps < LOW_MEAN_FPS_THRESHOLD {
            *logged_this_sample = true;
            warn!(
                target: PERFORMANCE_ISSUE_LOG_TARGET,
                "mean frame rate {mean_fps:.1} FPS over {:.0} s of sampled time (p99 {:.0} ms); {}",
                snapshot.snapshot_duration_seconds,
                snapshot.cpu_frame_times.p99_ms,
                describe_frame_time_breakdown(&breakdown),
            );
        }
    }
    if sample_completed {
        *logged_this_sample = false;
        return;
    }
    if !status.is_sampling() || status.is_changed() || *logged_this_sample {
        return;
    }
    let frame_ms = time.delta_secs_f64() as f32 * 1000.0;
    if frame_ms >= LONG_FRAME_THRESHOLD_MS {
        *logged_this_sample = true;
        warn!(
            target: PERFORMANCE_ISSUE_LOG_TARGET,
            "a frame took {frame_ms:.0} ms; {}",
            describe_frame_time_breakdown(&breakdown),
        );
    }
}

fn describe_frame_time_breakdown(breakdown: &FrameTimeBreakdown) -> String {
    let optional = |value: Option<f32>| value.map_or("n/a".to_owned(), |ms| format!("{ms:.1}"));
    format!(
        "breakdown ms: main world {:.1}, scene loop {:.1}, sleep {:.1}, render CPU {:.1}, present wait {}, GPU {}; bottleneck {}",
        breakdown.main_world_schedules_ms,
        breakdown.scene_runtime_loop_ms,
        breakdown.frame_limiter_sleep_ms,
        breakdown.render_world_cpu_ms,
        optional(breakdown.swapchain_present_wait_ms),
        optional(breakdown.gpu_frame_time_ms),
        breakdown
            .bottleneck
            .map_or("unknown".to_owned(), |bottleneck| format!("{bottleneck:?}")),
    )
}
