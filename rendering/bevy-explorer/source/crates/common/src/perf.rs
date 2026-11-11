//! Performance monitoring types: the sampling status, the sample in progress and the snapshot
//! it becomes, the frame-time breakdown and the profiler's scope summaries.

use bevy::{diagnostic::DiagnosticPath, prelude::*};
use serde::Serialize;

/// Why frame sampling is paused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SamplingPausedReason {
    NotInWorld,
    LoadingScreen,
    WindowUnfocused,
    /// The page is hidden (a background tab) or the host deactivated the viewport.
    #[cfg(target_arch = "wasm32")]
    ViewportInactive,
}

/// Whether frames are being sampled right now.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SamplingStatus {
    Sampling,
    Paused(SamplingPausedReason),
}

impl SamplingStatus {
    pub fn is_sampling(self) -> bool {
        self == SamplingStatus::Sampling
    }

    pub fn paused_reason(self) -> Option<SamplingPausedReason> {
        match self {
            SamplingStatus::Sampling => None,
            SamplingStatus::Paused(reason) => Some(reason),
        }
    }
}

/// Sampled seconds that complete one in-progress sample and produce one [`PerformanceSnapshot`].
pub const SAMPLE_DURATION_SECONDS: f64 = 15.0;

/// The sample being accumulated right now.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct InProgressSample {
    /// Seconds accumulated while [`SamplingStatus`] was `Sampling`; paused time never counts.
    pub sampled_seconds: f64,
}

impl InProgressSample {
    // the empty sample is the start of sampling, not a default value of the type
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            sampled_seconds: 0.0,
        }
    }
}

/// Summary statistics of one completed sample's frame times: count, extremes, mean, percentiles
/// and the frames over the hiccup threshold. Every time is in milliseconds.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FrameTimeSampleSummary {
    pub sample_count: u32,
    pub min_ms: f32,
    pub max_ms: f32,
    pub mean_ms: f32,
    pub p1_ms: f32,
    pub p5_ms: f32,
    pub p10_ms: f32,
    pub p20_ms: f32,
    pub p50_ms: f32,
    pub p75_ms: f32,
    pub p80_ms: f32,
    pub p90_ms: f32,
    pub p95_ms: f32,
    pub p99_ms: f32,
    pub hiccup_threshold_ms: f32,
    pub frames_over_hiccup_threshold_count: u32,
    /// Sum of the frame times of the frames over the threshold.
    pub frames_over_hiccup_threshold_time_sum_ms: f32,
    /// Sum of (frame time − threshold) over the frames over the threshold.
    pub frames_over_hiccup_threshold_excess_sum_ms: f32,
}

impl FrameTimeSampleSummary {
    /// Sorts `frame_times_ms` in place and computes every field. `None` when the sample has no
    /// frame times. A percentile is the value at index `count * p / 100` of the sorted times.
    pub fn from_frame_times(frame_times_ms: &mut [f32], hiccup_threshold_ms: f32) -> Option<Self> {
        if frame_times_ms.is_empty() {
            return None;
        }
        frame_times_ms.sort_by(f32::total_cmp);
        let count = frame_times_ms.len();
        let percentile = |p: usize| frame_times_ms[(count * p / 100).min(count - 1)];
        let sum: f64 = frame_times_ms.iter().map(|&ms| f64::from(ms)).sum();
        let over =
            &frame_times_ms[frame_times_ms.partition_point(|&ms| ms <= hiccup_threshold_ms)..];
        Some(Self {
            sample_count: count as u32,
            min_ms: frame_times_ms[0],
            max_ms: frame_times_ms[count - 1],
            mean_ms: (sum / count as f64) as f32,
            p1_ms: percentile(1),
            p5_ms: percentile(5),
            p10_ms: percentile(10),
            p20_ms: percentile(20),
            p50_ms: percentile(50),
            p75_ms: percentile(75),
            p80_ms: percentile(80),
            p90_ms: percentile(90),
            p95_ms: percentile(95),
            p99_ms: percentile(99),
            hiccup_threshold_ms,
            frames_over_hiccup_threshold_count: over.len() as u32,
            frames_over_hiccup_threshold_time_sum_ms: over.iter().sum(),
            frames_over_hiccup_threshold_excess_sum_ms: over
                .iter()
                .map(|ms| ms - hiccup_threshold_ms)
                .sum(),
        })
    }

    /// Frames per second at the mean frame time.
    pub fn mean_fps(&self) -> f32 {
        if self.mean_ms > 0.0 {
            1000.0 / self.mean_ms
        } else {
            0.0
        }
    }
}

/// Hiccup threshold for a frame-rate target: the larger of 50 ms and two target frame times.
/// `None` or a non-positive target means uncapped.
pub fn hiccup_threshold_ms(target_fps: Option<f32>) -> f32 {
    const MIN_HICCUP_THRESHOLD_MS: f32 = 50.0;
    const TARGET_FRAME_TIME_MULTIPLIER: f32 = 2.0;
    match target_fps {
        Some(fps) if fps > 0.0 => {
            MIN_HICCUP_THRESHOLD_MS.max(TARGET_FRAME_TIME_MULTIPLIER * 1000.0 / fps)
        }
        _ => MIN_HICCUP_THRESHOLD_MS,
    }
}

/// The client's memory at one moment: resident size on native, the wasm allocator's live bytes
/// on the web.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ClientMemoryDataPoint {
    pub used_bytes: u64,
    /// Highest `used_bytes` seen during the sample.
    pub used_peak_bytes_in_sample: u64,
    pub effective_cap_mib: u64,
}

/// Every running scene's JavaScript heap, summed, at one moment. Native: the Deno runtime's heap
/// statistics. Web: `performance.measureUserAgentSpecificMemory()` per scene worker where the
/// browser has it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct JsHeapDataPoint {
    pub used_bytes_sum: u64,
    pub limit_bytes_sum: u64,
    pub external_bytes_sum: u64,
    pub reporting_scene_count: u32,
}

/// Top scopes a snapshot carries.
pub const SNAPSHOT_TOP_PROFILED_SCOPE_COUNT: usize = 30;

/// One completed sample: its frame times summarised and the state of the client when it ended.
#[derive(Resource, Event, Clone, Debug, PartialEq, Serialize)]
pub struct PerformanceSnapshot {
    /// Sampled seconds the snapshot covers: [`SAMPLE_DURATION_SECONDS`] plus the last sampled
    /// frame's delta.
    pub snapshot_duration_seconds: f64,
    pub cpu_frame_times: FrameTimeSampleSummary,
    /// `None` when the renderer has no timestamp queries.
    pub gpu_frame_times: Option<FrameTimeSampleSummary>,
    /// `None` when the platform gives no memory sample.
    pub client_memory: Option<ClientMemoryDataPoint>,
    /// `None` when no heap was reported.
    pub js_heap: Option<JsHeapDataPoint>,
    pub nearby_player_count: u32,
    pub entity_count: u32,
    pub target_fps: Option<f32>,
    /// Top [`SNAPSHOT_TOP_PROFILED_SCOPE_COUNT`] scopes by self time over the sample; `None` without a
    /// recorded profiler table.
    pub top_profiled_scope_summaries: Option<Vec<ProfiledScopeSummary>>,
    /// `Time<Real>` elapsed seconds when the sample completed.
    pub taken_at_real_seconds: f64,
}

/// The previous frame's wall time split by the stage that spent it: main-world schedules, scene
/// loop, frame-limiter sleep, render-world CPU, present wait and GPU. Milliseconds.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Serialize)]
pub struct FrameTimeBreakdown {
    /// `Time<Real>` delta of the frame.
    pub whole_frame_wall_ms: f32,
    /// Main world, `First` through `Last`, including the scene loop and the sleep.
    pub main_world_schedules_ms: f32,
    /// `run_scene_loop` until its deadline or until no scene work remains, sleep excluded.
    pub scene_runtime_loop_ms: f32,
    /// The frame limiter's sleep inside `run_scene_loop` (native only; 0 on web).
    pub frame_limiter_sleep_ms: f32,
    /// Render world, `ExtractCommands` through cleanup, CPU side; extraction and the present wait
    /// excluded.
    pub render_world_cpu_ms: f32,
    /// Blocking wait for the swapchain image (vsync); `None` when not measurable.
    pub swapchain_present_wait_ms: Option<f32>,
    /// GPU execution time of the frame's render graph (`gpu/frame_time`); `None` without
    /// timestamp queries.
    pub gpu_frame_time_ms: Option<f32>,
    pub scene_ticks_run_count: u32,
    pub scene_jobs_left_count: u32,
    /// `None` until the readings the classification needs exist for the frame.
    pub bottleneck: Option<FrameBottleneck>,
}

/// Which stage bounds the frame rate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameBottleneck {
    MainThreadCpu,
    Gpu,
    SwapchainPresentWait,
    /// The frame limiter's sleep: the frame-rate cap holds the frame.
    FrameRateLimiter,
    /// CPU busy time and GPU time within 20 % of each other.
    CpuAndGpuBalanced,
}

/// Classification margin: a stage bounds the frame when it is this much longer than the other.
const BOTTLENECK_MIN_RATIO: f32 = 1.2;
/// A wait that takes this share of the frame bounds it.
const BOTTLENECK_MIN_WAIT_SHARE_OF_FRAME: f32 = 0.2;

/// Compares CPU busy time (whole frame minus sleep), GPU time and present wait with a 20 %
/// margin. A frame-limiter sleep or a present wait of a fifth of the frame or more bounds the
/// frame (`FrameRateLimiter` for the limiter, `SwapchainPresentWait` for vsync); otherwise the longer of CPU busy
/// and GPU time does, or `CpuAndGpuBalanced` when they are within the margin. `None` without a GPU time.
pub fn classify_frame_bottleneck(
    whole_frame_wall_ms: f32,
    frame_limiter_sleep_ms: f32,
    gpu_frame_time_ms: Option<f32>,
    swapchain_present_wait_ms: Option<f32>,
) -> Option<FrameBottleneck> {
    if whole_frame_wall_ms <= 0.0 {
        return None;
    }
    if frame_limiter_sleep_ms >= BOTTLENECK_MIN_WAIT_SHARE_OF_FRAME * whole_frame_wall_ms {
        return Some(FrameBottleneck::FrameRateLimiter);
    }
    if swapchain_present_wait_ms
        .is_some_and(|wait| wait >= BOTTLENECK_MIN_WAIT_SHARE_OF_FRAME * whole_frame_wall_ms)
    {
        return Some(FrameBottleneck::SwapchainPresentWait);
    }
    let gpu = gpu_frame_time_ms?;
    let cpu_busy = (whole_frame_wall_ms - frame_limiter_sleep_ms).max(0.0);
    Some(if gpu > cpu_busy * BOTTLENECK_MIN_RATIO {
        FrameBottleneck::Gpu
    } else if cpu_busy > gpu * BOTTLENECK_MIN_RATIO {
        FrameBottleneck::MainThreadCpu
    } else {
        FrameBottleneck::CpuAndGpuBalanced
    })
}

/// Summary of every call of one named scope (a tracing span) since the last reset.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProfiledScopeSummary {
    pub scope_name: String,
    pub scope_kind: ProfiledScopeKind,
    pub call_count: u32,
    /// Time inside the scope excluding the child scopes entered on the same thread.
    pub self_time_ms: f32,
    /// Time inside the scope including its child scopes.
    pub inclusive_time_ms: f32,
    /// Longest single call of the scope.
    pub max_single_call_ms: f32,
    /// Exponential moving average of per-frame self time over the last 60 frames.
    pub smoothed_self_time_per_frame_ms: f32,
}

/// Which tracing span produced a profiled scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfiledScopeKind {
    /// Bevy's `system` span: one ECS system run.
    EcsSystem,
    /// Bevy's `schedule` span: First, PreUpdate, Update, the scene loop schedule and so on.
    Schedule,
    /// Bevy's `node` span: one render-graph node.
    RenderGraphNode,
    /// Any other span of Bevy's: the app update, the render graph run, the present.
    BevyInternal,
    /// Any other span whose target is not a Bevy crate.
    ExplorerSpan,
}

/// Diagnostic paths of the frame-time breakdown and the allocation counts.
pub const PERF_MAIN_WORLD_SCHEDULES_MS: DiagnosticPath =
    DiagnosticPath::const_new("perf/main_world_schedules_ms");
pub const PERF_SCENE_RUNTIME_LOOP_MS: DiagnosticPath =
    DiagnosticPath::const_new("perf/scene_runtime_loop_ms");
pub const PERF_FRAME_LIMITER_SLEEP_MS: DiagnosticPath =
    DiagnosticPath::const_new("perf/frame_limiter_sleep_ms");
pub const PERF_RENDER_WORLD_CPU_MS: DiagnosticPath =
    DiagnosticPath::const_new("perf/render_world_cpu_ms");
pub const PERF_SWAPCHAIN_PRESENT_WAIT_MS: DiagnosticPath =
    DiagnosticPath::const_new("perf/swapchain_present_wait_ms");
pub const PERF_ALLOCATION_COUNT_PER_FRAME: DiagnosticPath =
    DiagnosticPath::const_new("perf/allocation_count_per_frame");
pub const PERF_ALLOCATED_BYTES_PER_FRAME: DiagnosticPath =
    DiagnosticPath::const_new("perf/allocated_bytes_per_frame");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_threshold_is_fifty_ms_or_two_target_frames() {
        assert_eq!(hiccup_threshold_ms(None), 50.0);
        assert_eq!(hiccup_threshold_ms(Some(0.0)), 50.0);
        assert_eq!(hiccup_threshold_ms(Some(60.0)), 50.0);
        assert_eq!(hiccup_threshold_ms(Some(999.0)), 50.0);
        assert!((hiccup_threshold_ms(Some(30.0)) - 66.666_67).abs() < 0.001);
        assert_eq!(hiccup_threshold_ms(Some(10.0)), 200.0);
    }

    #[test]
    fn an_empty_sample_has_no_summary() {
        assert_eq!(
            FrameTimeSampleSummary::from_frame_times(&mut [], 50.0),
            None
        );
    }

    #[test]
    fn a_single_frame_fills_every_percentile_with_itself() {
        let summary = FrameTimeSampleSummary::from_frame_times(&mut [16.0], 50.0).unwrap();
        assert_eq!(summary.sample_count, 1);
        assert_eq!(summary.min_ms, 16.0);
        assert_eq!(summary.max_ms, 16.0);
        assert_eq!(summary.mean_ms, 16.0);
        assert_eq!(summary.p1_ms, 16.0);
        assert_eq!(summary.p99_ms, 16.0);
        assert_eq!(summary.frames_over_hiccup_threshold_count, 0);
    }

    #[test]
    fn percentiles_index_the_sorted_times_and_hiccups_sum_over_the_threshold() {
        // 100 frames: 1..=100 ms, shuffled
        let mut times: Vec<f32> = (1..=100).rev().map(|ms| ms as f32).collect();
        let summary = FrameTimeSampleSummary::from_frame_times(&mut times, 90.0).unwrap();
        assert_eq!(summary.min_ms, 1.0);
        assert_eq!(summary.max_ms, 100.0);
        assert_eq!(summary.mean_ms, 50.5);
        assert_eq!(summary.p1_ms, 2.0);
        assert_eq!(summary.p50_ms, 51.0);
        assert_eq!(summary.p99_ms, 100.0);
        assert_eq!(summary.hiccup_threshold_ms, 90.0);
        // 91..=100
        assert_eq!(summary.frames_over_hiccup_threshold_count, 10);
        assert_eq!(summary.frames_over_hiccup_threshold_time_sum_ms, 955.0);
        assert_eq!(summary.frames_over_hiccup_threshold_excess_sum_ms, 55.0);
        assert!((summary.mean_fps() - 1000.0 / 50.5).abs() < 1e-3);
    }

    #[test]
    fn the_bottleneck_follows_the_twenty_percent_margin() {
        use FrameBottleneck::*;
        assert_eq!(classify_frame_bottleneck(0.0, 0.0, Some(1.0), None), None);
        assert_eq!(classify_frame_bottleneck(16.0, 0.0, None, None), None);
        // the limiter slept a fifth of the frame
        assert_eq!(
            classify_frame_bottleneck(33.0, 20.0, Some(5.0), Some(0.0)),
            Some(FrameRateLimiter)
        );
        // vsync held the frame
        assert_eq!(
            classify_frame_bottleneck(16.6, 0.0, Some(4.0), Some(8.0)),
            Some(SwapchainPresentWait)
        );
        assert_eq!(
            classify_frame_bottleneck(20.0, 0.0, Some(30.0), Some(1.0)),
            Some(Gpu)
        );
        assert_eq!(
            classify_frame_bottleneck(20.0, 1.0, Some(10.0), Some(1.0)),
            Some(MainThreadCpu)
        );
        assert_eq!(
            classify_frame_bottleneck(20.0, 1.0, Some(18.0), None),
            Some(CpuAndGpuBalanced)
        );
    }

    #[test]
    fn the_status_knows_its_reason() {
        assert!(SamplingStatus::Sampling.is_sampling());
        assert_eq!(SamplingStatus::Sampling.paused_reason(), None);
        let paused = SamplingStatus::Paused(SamplingPausedReason::LoadingScreen);
        assert!(!paused.is_sampling());
        assert_eq!(
            paused.paused_reason(),
            Some(SamplingPausedReason::LoadingScreen)
        );
        assert_eq!(
            serde_json::to_string(&paused).unwrap(),
            r#"{"paused":"loading_screen"}"#
        );
    }
}
