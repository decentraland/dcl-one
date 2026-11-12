//! The sample being populated, [`PerformancePopulatingSample`]: the sampling status, the sampled
//! seconds and the frame times accumulated so far. One system advances it with the frame's
//! [`FrameInputs`]; once it holds [`SAMPLE_DURATION_SECONDS`] of sampled time it yields a
//! [`PerformanceSnapshot`], inserted as the resource and sent as an event, and starts over.
//!
//! Frames are sampled while the player is in world, the loading screen is hidden and the window
//! is focused (`SamplingStatus::Sampling`). The frame sampling resumed on is left out, its delta
//! spanning the pause, and so are the GPU readings of the next
//! [`GPU_READINGS_SKIPPED_AFTER_RESUME`] sampled frames, which can still be the pause's.

use bevy::prelude::*;
use common::perf::{
    hiccup_threshold_ms, ClientMemoryDataPoint, FrameTimeSampleSummary, HudMainThreadCostDataPoint,
    PerformanceSnapshot, SamplingPausedReason, SamplingStatus, SAMPLE_DURATION_SECONDS,
    SNAPSHOT_TOP_PROFILED_SCOPE_COUNT,
};
use perf_output::gpu::RING as GPU_FRAME_TIME_RING;

use super::{
    frame_inputs::{FrameInputs, SessionSignals},
    profiled_scopes::{ProfiledScopeSortMetric, ProfiledScopesTable},
};

/// Sampled frames after a resume whose GPU readings are left out: a frame's GPU time is read
/// back up to [`GPU_FRAME_TIME_RING`] frames later, so a reading in these frames can belong to
/// the pause.
const GPU_READINGS_SKIPPED_AFTER_RESUME: u32 = GPU_FRAME_TIME_RING as u32;

/// The sample under way. Written by [`advance_populating_sample_system`] only.
#[derive(Resource, Debug)]
pub struct PerformancePopulatingSample {
    status: SamplingStatus,
    /// This frame's delta was counted: `Sampling`, and not the frame sampling resumed on.
    frame_sampled: bool,
    /// Seconds counted so far; paused time never counts.
    sampled_seconds: f64,
    /// Sampled frames since sampling resumed, the resumed frame itself excluded.
    sampled_frames_since_resume: u32,
    cpu_frame_times_ms: Vec<f32>,
    gpu_frame_times_ms: Vec<f32>,
    /// The highest memory reading so far; `None` until one is taken.
    used_peak_bytes: Option<u64>,
    /// The HUD cost windows that arrived while sampling, summed.
    hud_main_thread_cost_sum: Option<HudMainThreadCostDataPoint>,
}

impl PerformancePopulatingSample {
    /// Paused, not in world, nothing sampled.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            status: SamplingStatus::Paused(SamplingPausedReason::NotInWorld),
            frame_sampled: false,
            sampled_seconds: 0.0,
            sampled_frames_since_resume: 0,
            cpu_frame_times_ms: Vec::new(),
            gpu_frame_times_ms: Vec::new(),
            used_peak_bytes: None,
            hud_main_thread_cost_sum: None,
        }
    }

    pub fn status(&self) -> SamplingStatus {
        self.status
    }

    /// Whether the last advanced frame's delta was counted.
    pub fn frame_sampled(&self) -> bool {
        self.frame_sampled
    }

    pub fn sampled_seconds(&self) -> f64 {
        self.sampled_seconds
    }

    /// The highest memory reading of the sample so far; `None` until one is taken.
    pub fn used_peak_bytes(&self) -> Option<u64> {
        self.used_peak_bytes
    }

    /// Advances by one frame: sets the status from the session signals, counts the frame when
    /// sampled, and returns the snapshot when the sample is complete, without its top scopes,
    /// having started the next sample.
    pub fn advance(&mut self, inputs: &FrameInputs) -> Option<PerformanceSnapshot> {
        let next = sampling_status(inputs.session);
        let resumed = next.is_sampling() && !self.status.is_sampling();
        self.status = next;
        if resumed {
            self.sampled_frames_since_resume = 0;
        }
        self.frame_sampled = next.is_sampling() && !resumed;
        if !self.frame_sampled {
            return None;
        }
        self.sampled_seconds += inputs.frame_delta_seconds;
        self.sampled_frames_since_resume = self.sampled_frames_since_resume.saturating_add(1);
        self.cpu_frame_times_ms
            .push(inputs.frame_delta_seconds as f32 * 1000.0);
        if let Some(gpu_ms) = inputs.new_gpu_frame_time_ms {
            if self.sampled_frames_since_resume > GPU_READINGS_SKIPPED_AFTER_RESUME {
                self.gpu_frame_times_ms.push(gpu_ms);
            }
        }
        if let Some(window) = inputs.hud_main_thread_cost_window {
            match &mut self.hud_main_thread_cost_sum {
                Some(sum) => sum.add(&window),
                None => self.hud_main_thread_cost_sum = Some(window),
            }
        }
        if let Some(reading) = inputs.memory_reading {
            self.used_peak_bytes = Some(
                self.used_peak_bytes
                    .map_or(reading.used_bytes, |peak| peak.max(reading.used_bytes)),
            );
        }
        if self.sampled_seconds < SAMPLE_DURATION_SECONDS {
            return None;
        }

        let threshold = hiccup_threshold_ms(inputs.target_fps);
        let cpu_frame_times =
            FrameTimeSampleSummary::from_frame_times(&mut self.cpu_frame_times_ms, threshold)?;
        let gpu_frame_times =
            FrameTimeSampleSummary::from_frame_times(&mut self.gpu_frame_times_ms, threshold);
        let snapshot = PerformanceSnapshot {
            snapshot_duration_seconds: self.sampled_seconds,
            cpu_frame_times,
            gpu_frame_times,
            client_memory: inputs.memory_reading.map(|reading| ClientMemoryDataPoint {
                used_bytes: reading.used_bytes,
                used_peak_bytes_in_sample: self
                    .used_peak_bytes
                    .map_or(reading.used_bytes, |peak| peak.max(reading.used_bytes)),
                effective_cap_mib: reading.cap_mib,
            }),
            js_heap: inputs.js_heap,
            nearby_player_count: inputs.nearby_player_count,
            entity_count: inputs.entity_count.unwrap_or(0),
            target_fps: inputs.target_fps,
            top_profiled_scope_summaries: None,
            hud_main_thread_cost: self.hud_main_thread_cost_sum,
            taken_at_real_seconds: inputs.elapsed_real_seconds,
        };
        self.start_next_sample();
        Some(snapshot)
    }

    fn start_next_sample(&mut self) {
        self.sampled_seconds = 0.0;
        self.cpu_frame_times_ms.clear();
        self.gpu_frame_times_ms.clear();
        self.used_peak_bytes = None;
        self.hud_main_thread_cost_sum = None;
    }
}

/// The status the signals give: the loading screen (signed in, out of world, not held) and
/// not being in world pause before the focus does.
pub fn sampling_status(signals: SessionSignals) -> SamplingStatus {
    let SessionSignals {
        signed_in,
        world_held,
        realm_connected,
        out_of_world,
        focus_paused,
    } = signals;
    let loading_screen = signed_in && !world_held && out_of_world == Some(true);
    let in_world = signed_in && !world_held && realm_connected && out_of_world == Some(false);
    if loading_screen {
        SamplingStatus::Paused(SamplingPausedReason::LoadingScreen)
    } else if !in_world {
        SamplingStatus::Paused(SamplingPausedReason::NotInWorld)
    } else if let Some(reason) = focus_paused {
        SamplingStatus::Paused(reason)
    } else {
        SamplingStatus::Sampling
    }
}

/// Advances the sample with the frame's inputs; a completed sample's snapshot takes the
/// profiler's sample rows, is inserted as the resource and sent as an event.
pub(crate) fn advance_populating_sample_system(
    inputs: Res<FrameInputs>,
    mut sample: ResMut<PerformancePopulatingSample>,
    mut profiled_scopes: ResMut<ProfiledScopesTable>,
    mut snapshots: EventWriter<PerformanceSnapshot>,
    mut commands: Commands,
) {
    let Some(mut snapshot) = sample.advance(&inputs) else {
        return;
    };
    snapshot.top_profiled_scope_summaries = profiled_scopes.recording_enabled().then(|| {
        profiled_scopes.take_sample_top_scope_summaries(
            SNAPSHOT_TOP_PROFILED_SCOPE_COUNT,
            ProfiledScopeSortMetric::SelfTime,
        )
    });
    snapshots.write(snapshot.clone());
    commands.insert_resource(snapshot);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_world() -> SessionSignals {
        SessionSignals {
            signed_in: true,
            world_held: false,
            realm_connected: true,
            out_of_world: Some(false),
            focus_paused: None,
        }
    }

    fn frame(session: SessionSignals, delta_seconds: f64, gpu_ms: Option<f32>) -> FrameInputs {
        let mut inputs = FrameInputs::new();
        inputs.session = session;
        inputs.frame_delta_seconds = delta_seconds;
        inputs.new_gpu_frame_time_ms = gpu_ms;
        inputs
    }

    #[test]
    fn the_status_follows_the_signals_in_order() {
        let signals = in_world();
        assert_eq!(sampling_status(signals), SamplingStatus::Sampling);
        assert_eq!(
            sampling_status(SessionSignals {
                out_of_world: Some(true),
                focus_paused: Some(SamplingPausedReason::WindowUnfocused),
                ..signals
            }),
            SamplingStatus::Paused(SamplingPausedReason::LoadingScreen)
        );
        assert_eq!(
            sampling_status(SessionSignals {
                realm_connected: false,
                ..signals
            }),
            SamplingStatus::Paused(SamplingPausedReason::NotInWorld)
        );
        assert_eq!(
            sampling_status(SessionSignals {
                world_held: true,
                out_of_world: Some(true),
                ..signals
            }),
            SamplingStatus::Paused(SamplingPausedReason::NotInWorld)
        );
        assert_eq!(
            sampling_status(SessionSignals {
                focus_paused: Some(SamplingPausedReason::WindowUnfocused),
                ..signals
            }),
            SamplingStatus::Paused(SamplingPausedReason::WindowUnfocused)
        );
    }

    #[test]
    fn the_resume_frame_and_its_gpu_readings_are_left_out_and_the_sample_completes_at_its_duration()
    {
        let mut sample = PerformancePopulatingSample::new();
        let paused = SessionSignals {
            focus_paused: Some(SamplingPausedReason::WindowUnfocused),
            ..in_world()
        };
        assert!(sample.advance(&frame(paused, 5.0, Some(9.0))).is_none());
        assert!(!sample.frame_sampled());
        // the resume frame: its delta spans the pause
        assert!(sample.advance(&frame(in_world(), 5.0, Some(9.0))).is_none());
        assert!(!sample.frame_sampled());
        assert_eq!(sample.sampled_seconds(), 0.0);

        // a delta exact in binary, so the sampled seconds reach 15.0 on the 120th frame
        let frames_a_second = 8;
        let delta = 1.0 / frames_a_second as f64;
        let mut snapshot = None;
        let mut sampled_frames = 0;
        while snapshot.is_none() {
            snapshot = sample.advance(&frame(in_world(), delta, Some(4.0)));
            assert!(sample.frame_sampled());
            sampled_frames += 1;
        }
        let snapshot = snapshot.unwrap();
        assert_eq!(
            sampled_frames,
            SAMPLE_DURATION_SECONDS as u32 * frames_a_second
        );
        assert_eq!(snapshot.cpu_frame_times.sample_count, sampled_frames);
        assert_eq!(
            snapshot.gpu_frame_times.unwrap().sample_count,
            sampled_frames - GPU_READINGS_SKIPPED_AFTER_RESUME
        );
        assert!(snapshot.top_profiled_scope_summaries.is_none());
        // the next sample starts empty
        assert_eq!(sample.sampled_seconds(), 0.0);
        assert!(sample.advance(&frame(in_world(), delta, None)).is_none());
        assert_eq!(sample.sampled_seconds(), delta);
    }
}
