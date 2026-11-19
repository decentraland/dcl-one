//! The "Performance Metrics" event, one per `common::perf::PerformanceSnapshot` event read.

use bevy::prelude::*;
use common::perf::PerformanceSnapshot;

use crate::{
    data_definition::{SegmentEvent, SegmentEventPerformanceMetrics, SegmentGpuFrameTimes},
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
        metrics.add_event(
            SegmentEvent::PerformanceMetrics(SegmentEventPerformanceMetrics::from_snapshot(
                snapshot,
            )),
            config.common(),
        );
    }
}

impl SegmentEventPerformanceMetrics {
    fn from_snapshot(snapshot: &PerformanceSnapshot) -> Self {
        let cpu = &snapshot.cpu_frame_times;
        Self {
            samples: cpu.sample_count,
            total_time: cpu.mean_ms * cpu.sample_count as f32,
            hiccups_in_thousand_frames: (u64::from(cpu.frames_over_hiccup_threshold_count) * 1000
                / u64::from(cpu.sample_count.max(1)))
                as u32,
            hiccups_time: cpu.frames_over_hiccup_threshold_time_sum_ms / 1000.0,
            hiccup_count: cpu.frames_over_hiccup_threshold_count,
            hiccup_sample_count: cpu.sample_count,
            hiccup_threshold_ms: cpu.hiccup_threshold_ms,
            hiccup_time_ms: cpu.frames_over_hiccup_threshold_time_sum_ms,
            hiccup_excess_time_ms: cpu.frames_over_hiccup_threshold_excess_sum_ms,
            gpu: snapshot
                .gpu_frame_times
                .as_ref()
                .map(|gpu| SegmentGpuFrameTimes {
                    gpu_hiccup_count: gpu.frames_over_hiccup_threshold_count,
                    gpu_hiccup_sample_count: gpu.sample_count,
                    gpu_hiccup_threshold_ms: gpu.hiccup_threshold_ms,
                    gpu_hiccup_time_ms: gpu.frames_over_hiccup_threshold_time_sum_ms,
                    gpu_hiccup_excess_time_ms: gpu.frames_over_hiccup_threshold_excess_sum_ms,
                    gpu_min_frame_time: gpu.min_ms,
                    gpu_max_frame_time: gpu.max_ms,
                    gpu_mean_frame_time: gpu.mean_ms,
                    gpu_p1_frame_time: gpu.p1_ms,
                    gpu_p5_frame_time: gpu.p5_ms,
                    gpu_p10_frame_time: gpu.p10_ms,
                    gpu_p20_frame_time: gpu.p20_ms,
                    gpu_p50_frame_time: gpu.p50_ms,
                    gpu_p75_frame_time: gpu.p75_ms,
                    gpu_p80_frame_time: gpu.p80_ms,
                    gpu_p90_frame_time: gpu.p90_ms,
                    gpu_p95_frame_time: gpu.p95_ms,
                    gpu_p99_frame_time: gpu.p99_ms,
                }),
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

            // TODO: reported by the telemetry change
            player_count: -1,
            used_jsheap_size: -1,
            memory_usage: -1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_definition::{
        build_segment_event_batch_item, SegmentEventCommonExplorerFields,
    };
    use common::perf::FrameTimeSampleSummary;
    use serde_json::{json, Value};

    fn batch_item(cpu: &mut [f32], gpu: &mut [f32]) -> Value {
        let snapshot = PerformanceSnapshot {
            snapshot_duration_seconds: 15.0,
            cpu_frame_times: FrameTimeSampleSummary::from_frame_times(cpu, 50.0).unwrap(),
            gpu_frame_times: FrameTimeSampleSummary::from_frame_times(gpu, 50.0),
            js_heap: None,
            entity_count: 0,
            target_fps: None,
            hud_main_thread_cost: None,
            taken_at_real_seconds: 15.0,
        };
        let event = SegmentEvent::PerformanceMetrics(
            SegmentEventPerformanceMetrics::from_snapshot(&snapshot),
        );
        let common =
            SegmentEventCommonExplorerFields::new("session".into(), "version".into(), None);
        serde_json::to_value(build_segment_event_batch_item(
            "user".into(),
            &common,
            &event,
            chrono::Utc::now(),
            "message".into(),
        ))
        .unwrap()
    }

    #[test]
    fn performance_metrics_reports_cpu_and_gpu_counts_durations_and_percentiles() {
        let mut gpu: Vec<_> = (0..100).map(|n| n as f32).collect();
        let event = batch_item(&mut [16.0, 50.0, 62.0, 63.0, 29.0, 20.0], &mut gpu);
        assert_eq!(event["event"], "Performance Metrics");
        let properties = &event["properties"];
        let expected = json!({
            "session_id": "session",
            "samples": 6,
            "total_time": 240.0,
            "hiccups_in_thousand_frames": 333,
            "hiccups_time": 0.125,
            "median_frame_time": 50.0,
            "p50_frame_time": 50.0,
            "hiccup_count": 2,
            "hiccup_sample_count": 6,
            "hiccup_threshold_ms": 50.0,
            "hiccup_time_ms": 125.0,
            "hiccup_excess_time_ms": 25.0,
            "gpu_hiccup_count": 49,
            "gpu_hiccup_sample_count": 100,
            "gpu_hiccup_threshold_ms": 50.0,
            "gpu_hiccup_time_ms": 3675.0,
            "gpu_hiccup_excess_time_ms": 1225.0,
            "gpu_min_frame_time": 0.0,
            "gpu_max_frame_time": 99.0,
            "gpu_mean_frame_time": 49.5,
        });
        for (key, value) in expected.as_object().unwrap() {
            assert_eq!(&properties[key], value, "{key}");
        }
        for p in [1, 5, 10, 20, 50, 75, 80, 90, 95, 99] {
            let key = format!("gpu_p{p}_frame_time");
            assert_eq!(properties[&key], f64::from(p), "{key}");
        }
    }

    #[test]
    fn unavailable_gpu_readings_are_omitted_and_measured_zero_is_preserved() {
        let unavailable = batch_item(&mut [16.0], &mut []);
        let properties = unavailable["properties"].as_object().unwrap();
        assert!(properties.keys().all(|key| !key.starts_with("gpu")));
        assert_eq!(properties["hiccup_count"], 0);
        assert_eq!(properties["hiccup_excess_time_ms"], 0.0);

        let measured = batch_item(&mut [16.0], &mut [0.0, 50.0]);
        let properties = &measured["properties"];
        assert_eq!(properties["gpu_hiccup_sample_count"], 2);
        assert_eq!(properties["gpu_hiccup_count"], 0);
        assert_eq!(properties["gpu_hiccup_time_ms"], 0.0);
        assert_eq!(properties["gpu_hiccup_excess_time_ms"], 0.0);
        assert_eq!(properties["gpu_min_frame_time"], 0.0);
    }
}
