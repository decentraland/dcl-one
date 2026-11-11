//! The "Performance Metrics" event, one per completed performance sample
//! (`common::perf::PerformanceSnapshot`, taken by the engine's sampler in `src/perf/`).

use bevy::prelude::*;
use common::perf::PerformanceSnapshot;

use crate::{
    data_definition::{SegmentEvent, SegmentEventPerformanceMetrics},
    segment_system::{SegmentConfig, SegmentMetricsEvents},
};

pub struct FrameMetricPlugin;

impl Plugin for FrameMetricPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<PerformanceSnapshot>()
            .add_systems(Update, report_performance_snapshot_system);
    }
}

fn report_performance_snapshot_system(
    mut snapshots: EventReader<PerformanceSnapshot>,
    mut metrics: ResMut<SegmentMetricsEvents>,
    config: Option<Res<SegmentConfig>>,
) {
    let Some(config) = config else {
        return;
    };
    for snapshot in snapshots.read() {
        let cpu = &snapshot.cpu_frame_times;
        metrics.add_event(
            SegmentEvent::PerformanceMetrics(SegmentEventPerformanceMetrics {
                samples: cpu.sample_count,
                total_time: cpu.mean_ms * cpu.sample_count as f32,
                hiccups_in_thousand_frames: (u64::from(cpu.frames_over_hiccup_threshold_count)
                    * 1000
                    / u64::from(cpu.sample_count.max(1)))
                    as u32,
                hiccups_time: cpu.frames_over_hiccup_threshold_time_sum_ms / 1000.0,
                min_frame_time: cpu.min_ms,
                max_frame_time: cpu.max_ms,
                mean_frame_time: cpu.mean_ms,
                median_frame_time: cpu.p50_ms,
                p1_frame_time: cpu.p1_ms,
                p5_frame_time: cpu.p5_ms,
                p10_frame_time: cpu.p10_ms,
                p20_frame_time: cpu.p20_ms,
                p50_frame_time: cpu.p50_ms,
                p75_frame_time: cpu.p75_ms,
                p80_frame_time: cpu.p80_ms,
                p90_frame_time: cpu.p90_ms,
                p95_frame_time: cpu.p95_ms,
                p99_frame_time: cpu.p99_ms,

                // not reported
                player_count: -1,
                used_jsheap_size: -1,
                memory_usage: -1,
            }),
            config.common(),
        );
    }
}
