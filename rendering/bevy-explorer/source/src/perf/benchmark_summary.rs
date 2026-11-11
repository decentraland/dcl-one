//! The benchmark run's summary: two `perf_summary` log lines when the autopilot's measured phase
//! ends.
//!
//! While `Measuring`, every frame's `Time<Real>` delta and every new GPU frame time are kept, so
//! the lines summarise the whole measured phase, not one sample. The profiler is enabled for the
//! phase; the top scopes are the sum of every snapshot's top rows and what the table holds at the
//! end.
//!
//! Line 1, `key=value` pairs in this order:
//! `cpu_p99_ms cpu_frames_over_hiccup_threshold_count cpu_hiccup_threshold_ms gpu_p99_ms
//! gpu_frames_over_hiccup_threshold_count used_peak_mib js_heap_used_mib entity_count
//! sample_count snapshot_duration_seconds`; a GPU value is `n/a` without timestamp queries, the
//! heap `n/a` without a reporting scene.
//! Line 2: `top_scopes=<name>:<self ms>,...`, up to 30, highest self time first; a name has its
//! commas, colons and equals signs replaced by spaces.

use std::collections::HashMap;

use bevy::{diagnostic::DiagnosticsStore, platform::time::Instant, prelude::*};
use common::{
    autopilot::AutopilotPhase,
    memory::MemoryStatus,
    perf::{
        hiccup_threshold_ms, FrameTimeSampleSummary, PerformanceSnapshot, ProfiledScopeSummary,
        SNAPSHOT_TOP_PROFILED_SCOPE_COUNT,
    },
    structs::{AppConfig, SessionGraphics},
};
use scene_runner::renderer_context::RendererSceneContext;

use super::{
    in_progress_sample::{
        effective_target_fps, latest_entity_count, new_gpu_frame_time_ms, sum_scene_js_heaps,
    },
    profiled_scopes::{ProfiledScopeSortMetric, ProfiledScopesTable},
};

pub const PERF_SUMMARY_LOG_TARGET: &str = "perf_summary";

#[derive(Default)]
pub struct BenchmarkMeasuredPhaseAccumulator {
    measured_phase_started: bool,
    cpu_frame_times_ms: Vec<f32>,
    gpu_frame_times_ms: Vec<f32>,
    last_gpu_frame_time_measured_at: Option<Instant>,
    used_peak_bytes: u64,
    /// Self time per scope name, summed over the snapshots' top rows.
    scope_self_time_sum_ms: HashMap<String, f32>,
    sampled_seconds: f64,
    perf_summary_logged: bool,
}

/// `Update`, after the sampler. Runs only under the autopilot (the phase resource exists then).
#[allow(clippy::too_many_arguments)]
pub fn accumulate_benchmark_measured_phase_system(
    phase: Option<Res<AutopilotPhase>>,
    time: Res<Time<Real>>,
    diagnostics: Res<DiagnosticsStore>,
    memory: Res<MemoryStatus>,
    scenes: Query<&RendererSceneContext>,
    config: Res<AppConfig>,
    session: Option<Res<SessionGraphics>>,
    mut profiled_scopes: ResMut<ProfiledScopesTable>,
    mut snapshots: EventReader<PerformanceSnapshot>,
    mut run: Local<BenchmarkMeasuredPhaseAccumulator>,
) {
    let Some(phase) = phase else {
        return;
    };
    match *phase {
        AutopilotPhase::Arriving => {}
        AutopilotPhase::Measuring => {
            if !run.measured_phase_started {
                run.measured_phase_started = true;
                profiled_scopes.set_recording_enabled(true);
                // the frame the phase changed spans the arrival
                return;
            }
            run.cpu_frame_times_ms
                .push(time.delta_secs_f64() as f32 * 1000.0);
            run.sampled_seconds += time.delta_secs_f64();
            let run = &mut *run;
            if let Some(gpu_ms) =
                new_gpu_frame_time_ms(&diagnostics, &mut run.last_gpu_frame_time_measured_at)
            {
                run.gpu_frame_times_ms.push(gpu_ms);
            }
            if let Some(reading) = memory.reading() {
                run.used_peak_bytes = run.used_peak_bytes.max(reading.used_bytes);
            }
            for snapshot in snapshots.read() {
                add_scope_self_times(
                    run,
                    snapshot
                        .top_profiled_scope_summaries
                        .as_deref()
                        .unwrap_or_default(),
                );
            }
        }
        AutopilotPhase::Ending { exit_code } => {
            if run.perf_summary_logged || !run.measured_phase_started {
                return;
            }
            run.perf_summary_logged = true;
            let run = &mut *run;
            for snapshot in snapshots.read() {
                add_scope_self_times(
                    run,
                    snapshot
                        .top_profiled_scope_summaries
                        .as_deref()
                        .unwrap_or_default(),
                );
            }
            add_scope_self_times(
                run,
                &profiled_scopes.top_scope_summaries(
                    SNAPSHOT_TOP_PROFILED_SCOPE_COUNT,
                    ProfiledScopeSortMetric::SelfTime,
                ),
            );
            profiled_scopes.set_recording_enabled(false);
            if exit_code != 0 {
                return;
            }
            let threshold = hiccup_threshold_ms(effective_target_fps(&config, session.as_deref()));
            let cpu =
                FrameTimeSampleSummary::from_frame_times(&mut run.cpu_frame_times_ms, threshold);
            let gpu =
                FrameTimeSampleSummary::from_frame_times(&mut run.gpu_frame_times_ms, threshold);
            let (values_line, top_scopes_line) = perf_summary_log_lines(
                cpu.as_ref(),
                gpu.as_ref(),
                run.used_peak_bytes,
                sum_scene_js_heaps(scenes.iter()).map(|heap| heap.used_bytes_sum),
                u64::from(latest_entity_count(&diagnostics).unwrap_or(0)),
                run.sampled_seconds,
                &run.scope_self_time_sum_ms,
            );
            info!(target: PERF_SUMMARY_LOG_TARGET, "{values_line}");
            info!(target: PERF_SUMMARY_LOG_TARGET, "{top_scopes_line}");
        }
    }
}

fn add_scope_self_times(
    run: &mut BenchmarkMeasuredPhaseAccumulator,
    rows: &[ProfiledScopeSummary],
) {
    for row in rows {
        *run.scope_self_time_sum_ms
            .entry(row.scope_name.clone())
            .or_default() += row.self_time_ms;
    }
}

const BYTES_PER_MIB: u64 = 1 << 20;

/// The two lines, as the module doc lays them out.
pub fn perf_summary_log_lines(
    cpu: Option<&FrameTimeSampleSummary>,
    gpu: Option<&FrameTimeSampleSummary>,
    used_peak_bytes: u64,
    js_heap_used_bytes: Option<u64>,
    entity_count: u64,
    sampled_seconds: f64,
    scope_self_time_sum_ms: &HashMap<String, f32>,
) -> (String, String) {
    let ms = |value: Option<f32>| value.map_or("n/a".to_owned(), |ms| format!("{ms:.2}"));
    let count = |value: Option<u32>| value.map_or("n/a".to_owned(), |count| count.to_string());
    let values_line = format!(
        "cpu_p99_ms={} cpu_frames_over_hiccup_threshold_count={} cpu_hiccup_threshold_ms={} gpu_p99_ms={} gpu_frames_over_hiccup_threshold_count={} used_peak_mib={} js_heap_used_mib={} entity_count={} sample_count={} snapshot_duration_seconds={:.1}",
        ms(cpu.map(|cpu| cpu.p99_ms)),
        count(cpu.map(|cpu| cpu.frames_over_hiccup_threshold_count)),
        ms(cpu.map(|cpu| cpu.hiccup_threshold_ms)),
        ms(gpu.map(|gpu| gpu.p99_ms)),
        count(gpu.map(|gpu| gpu.frames_over_hiccup_threshold_count)),
        used_peak_bytes / BYTES_PER_MIB,
        js_heap_used_bytes.map_or("n/a".to_owned(), |bytes| (bytes / BYTES_PER_MIB).to_string()),
        entity_count,
        count(cpu.map(|cpu| cpu.sample_count)),
        sampled_seconds,
    );
    let mut scopes: Vec<(&String, &f32)> = scope_self_time_sum_ms.iter().collect();
    scopes.sort_by(|a, b| b.1.total_cmp(a.1).then_with(|| a.0.cmp(b.0)));
    let top_scopes_line = format!(
        "top_scopes={}",
        scopes
            .iter()
            .take(SNAPSHOT_TOP_PROFILED_SCOPE_COUNT)
            .map(|(name, self_ms)| format!("{}:{self_ms:.2}", name.replace([',', ':', '='], " ")))
            .collect::<Vec<_>>()
            .join(",")
    );
    (values_line, top_scopes_line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lines_hold_every_key_in_order_and_the_scopes_by_self_time() {
        let mut cpu_times = vec![10.0, 20.0, 80.0, 15.0];
        let cpu = FrameTimeSampleSummary::from_frame_times(&mut cpu_times, 50.0).unwrap();
        let scopes = HashMap::from([("a::b".to_owned(), 1.5), ("x, y: z=w".to_owned(), 9.0)]);
        let (values_line, top_scopes_line) = perf_summary_log_lines(
            Some(&cpu),
            None,
            300 * BYTES_PER_MIB,
            Some(64 * BYTES_PER_MIB),
            1234,
            90.04,
            &scopes,
        );
        assert_eq!(
            values_line,
            "cpu_p99_ms=80.00 cpu_frames_over_hiccup_threshold_count=1 cpu_hiccup_threshold_ms=50.00 gpu_p99_ms=n/a gpu_frames_over_hiccup_threshold_count=n/a used_peak_mib=300 js_heap_used_mib=64 entity_count=1234 sample_count=4 snapshot_duration_seconds=90.0"
        );
        assert_eq!(
            top_scopes_line,
            "top_scopes=x  y  z w:9.00,a::b:1.50".replace("a::b", "a  b")
        );
    }
}
