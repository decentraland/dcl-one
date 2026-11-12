//! The sampling status, the sample being accumulated and the snapshot it becomes.
//!
//! Frames are sampled while the player is in world, the loading screen is hidden and the window
//! is focused (`SamplingStatus::Sampling`). A sample collects `Time<Real>` frame deltas and GPU
//! frame times for [`SAMPLE_DURATION_SECONDS`] of sampled time, then becomes a
//! [`PerformanceSnapshot`]: the latest is a resource, and every one is sent as an event. The
//! frame sampling resumed on is left out, and so are the GPU readings of the next
//! [`GPU_READINGS_SKIPPED_AFTER_RESUME`] frames, which can still be the pause's.

use bevy::{
    diagnostic::{Diagnostic, DiagnosticsStore, EntityCountDiagnosticsPlugin},
    platform::time::Instant,
    prelude::*,
};
use common::{
    gpu_time::GPU_FRAME_TIME,
    memory::MemoryStatus,
    perf::{
        hiccup_threshold_ms, ClientMemoryDataPoint, FrameTimeSampleSummary, InProgressSample,
        JsHeapDataPoint, PerformanceSnapshot, SamplingPausedReason, SamplingStatus,
        SAMPLE_DURATION_SECONDS, SNAPSHOT_TOP_PROFILED_SCOPE_COUNT,
    },
    structs::{AppConfig, CurrentRealm, OutOfWorld, PrimaryUser, SessionGraphics, WorldHold},
};
use comms::global_crdt::ForeignPlayer;
use perf_output::gpu::RING as GPU_FRAME_TIME_RING;
use scene_runner::renderer_context::RendererSceneContext;
use wallet::Wallet;

use super::profiled_scopes::{ProfiledScopeSortMetric, ProfiledScopesTable};

/// Sampled frames after a resume whose GPU readings are left out: a frame's GPU time is read
/// back up to [`GPU_FRAME_TIME_RING`] frames later, so a reading in these frames can belong to
/// the pause.
const GPU_READINGS_SKIPPED_AFTER_RESUME: u32 = GPU_FRAME_TIME_RING as u32;

/// The frame times of the sample being accumulated, and the memory peak seen during it.
#[derive(Resource)]
pub struct InProgressSampleMeasurements {
    cpu_frame_times_ms: Vec<f32>,
    gpu_frame_times_ms: Vec<f32>,
    pub used_peak_bytes_in_sample: u64,
    last_gpu_frame_time_measured_at: Option<Instant>,
    /// Sampled frames since sampling resumed, the resumed frame itself excluded.
    sampled_frames_since_resume: u32,
    /// The HUD cost windows that arrived while sampling, summed.
    #[cfg(target_arch = "wasm32")]
    hud_main_thread_cost_sum: Option<common::perf::HudMainThreadCostDataPoint>,
}

impl InProgressSampleMeasurements {
    // empty is the start of a sample, not a default value of the type
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            cpu_frame_times_ms: Vec::new(),
            gpu_frame_times_ms: Vec::new(),
            used_peak_bytes_in_sample: 0,
            last_gpu_frame_time_measured_at: None,
            sampled_frames_since_resume: 0,
            #[cfg(target_arch = "wasm32")]
            hud_main_thread_cost_sum: None,
        }
    }

    fn reset(&mut self) {
        self.cpu_frame_times_ms.clear();
        self.gpu_frame_times_ms.clear();
        self.used_peak_bytes_in_sample = 0;
        #[cfg(target_arch = "wasm32")]
        {
            self.hud_main_thread_cost_sum = None;
        }
    }
}

/// The `gpu/frame_time` measurement when it is newer than `last_taken`, which it updates.
pub fn new_gpu_frame_time_ms(
    diagnostics: &DiagnosticsStore,
    last_taken: &mut Option<Instant>,
) -> Option<f32> {
    let measurement = diagnostics
        .get(&GPU_FRAME_TIME)
        .and_then(Diagnostic::measurement)?;
    if *last_taken == Some(measurement.time) {
        return None;
    }
    *last_taken = Some(measurement.time);
    Some(measurement.value as f32)
}

/// The latest entity count; `None` before the first.
pub fn latest_entity_count(diagnostics: &DiagnosticsStore) -> Option<u32> {
    diagnostics
        .get(&EntityCountDiagnosticsPlugin::ENTITY_COUNT)
        .and_then(Diagnostic::value)
        .map(|count| count as u32)
}

/// Sets [`SamplingStatus`] from the world state, the loading screen and the focus: the primary
/// window's on native, the page's and the viewport's on the web (`BrowserPageFocus`); advances
/// the in-progress sample by the `Time<Real>` delta while `Sampling`, except on the frame
/// sampling resumed, whose delta spans the pause.
#[allow(clippy::too_many_arguments)]
pub fn update_sampling_status_system(
    mut status: ResMut<SamplingStatus>,
    mut sample: ResMut<InProgressSample>,
    time: Res<Time<Real>>,
    wallet: Option<Res<Wallet>>,
    world_hold: Option<Res<WorldHold>>,
    realm: Option<Res<CurrentRealm>>,
    player: Query<Has<OutOfWorld>, With<PrimaryUser>>,
    #[cfg(not(target_arch = "wasm32"))] window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    #[cfg(target_arch = "wasm32")] browser_page_focus: Res<crate::web::BrowserPageFocus>,
) {
    let signed_in = wallet
        .as_ref()
        .is_some_and(|wallet| wallet.address().is_some());
    let world_held = world_hold.is_some();
    let realm_connected = realm.as_ref().is_some_and(|realm| realm.connected);
    let out_of_world = player.single().ok();
    // the loading screen: signed in, out of world, not held
    let loading_screen = signed_in && !world_held && out_of_world == Some(true);
    let in_world = signed_in && !world_held && realm_connected && out_of_world == Some(false);
    #[cfg(not(target_arch = "wasm32"))]
    let focus_paused = (!window.single().map_or(true, |window| window.focused))
        .then_some(SamplingPausedReason::WindowUnfocused);
    // the page's focus, not the canvas's DOM focus (`Window::focused`)
    #[cfg(target_arch = "wasm32")]
    let focus_paused = browser_page_focus.sampling_paused_reason();
    let next = if loading_screen {
        SamplingStatus::Paused(SamplingPausedReason::LoadingScreen)
    } else if !in_world {
        SamplingStatus::Paused(SamplingPausedReason::NotInWorld)
    } else if let Some(reason) = focus_paused {
        SamplingStatus::Paused(reason)
    } else {
        SamplingStatus::Sampling
    };
    let resumed = next.is_sampling() && !status.is_sampling();
    status.set_if_neq(next);
    if next.is_sampling() && !resumed {
        sample.sampled_seconds += time.delta_secs_f64();
    }
}

/// While `Sampling`, takes the frame's `Time<Real>` delta and any new GPU frame time,
/// except on the frame sampling resumed; when the sample reaches [`SAMPLE_DURATION_SECONDS`],
/// builds the [`PerformanceSnapshot`], inserts it as the resource, sends it as an event and
/// starts the next sample.
#[allow(clippy::too_many_arguments)]
pub fn sample_frame_times_system(
    status: Res<SamplingStatus>,
    mut sample: ResMut<InProgressSample>,
    mut frame_times: ResMut<InProgressSampleMeasurements>,
    time: Res<Time<Real>>,
    diagnostics: Res<DiagnosticsStore>,
    memory: Res<MemoryStatus>,
    scenes: Query<&RendererSceneContext>,
    nearby_players: Query<(), With<ForeignPlayer>>,
    config: Res<AppConfig>,
    session: Option<Res<SessionGraphics>>,
    mut profiled_scopes: ResMut<ProfiledScopesTable>,
    mut snapshots: EventWriter<PerformanceSnapshot>,
    mut commands: Commands,
    #[cfg(target_arch = "wasm32")] hud_cost: Res<crate::web::LatestHudMainThreadCostWindow>,
) {
    // taken every frame, so a GPU reading from the pause is consumed before sampling resumes
    let frame_times = &mut *frame_times;
    let new_gpu_ms = new_gpu_frame_time_ms(
        &diagnostics,
        &mut frame_times.last_gpu_frame_time_measured_at,
    );
    // `is_changed` on the frame the status was written: the transition into `Sampling`
    let resumed = status.is_sampling() && status.is_changed();
    if resumed {
        frame_times.sampled_frames_since_resume = 0;
    }
    if !status.is_sampling() || resumed {
        return;
    }
    frame_times.sampled_frames_since_resume =
        frame_times.sampled_frames_since_resume.saturating_add(1);
    frame_times
        .cpu_frame_times_ms
        .push(time.delta_secs_f64() as f32 * 1000.0);
    if let Some(gpu_ms) = new_gpu_ms {
        if frame_times.sampled_frames_since_resume > GPU_READINGS_SKIPPED_AFTER_RESUME {
            frame_times.gpu_frame_times_ms.push(gpu_ms);
        }
    }
    // `is_changed` on the frame a window arrived
    #[cfg(target_arch = "wasm32")]
    if hud_cost.is_changed() {
        if let Some(window) = hud_cost.0 {
            match &mut frame_times.hud_main_thread_cost_sum {
                Some(sum) => sum.add(&window),
                None => frame_times.hud_main_thread_cost_sum = Some(window),
            }
        }
    }
    let memory_reading = memory.reading();
    if let Some(reading) = memory_reading {
        frame_times.used_peak_bytes_in_sample = frame_times
            .used_peak_bytes_in_sample
            .max(reading.used_bytes);
    }
    if sample.sampled_seconds < SAMPLE_DURATION_SECONDS {
        return;
    }

    let target_fps = effective_target_fps(&config, session.as_deref());
    let threshold = hiccup_threshold_ms(target_fps);
    let Some(cpu_frame_times) =
        FrameTimeSampleSummary::from_frame_times(&mut frame_times.cpu_frame_times_ms, threshold)
    else {
        return;
    };
    let gpu_frame_times =
        FrameTimeSampleSummary::from_frame_times(&mut frame_times.gpu_frame_times_ms, threshold);
    let snapshot = PerformanceSnapshot {
        snapshot_duration_seconds: sample.sampled_seconds,
        cpu_frame_times,
        gpu_frame_times,
        client_memory: memory_reading.map(|reading| ClientMemoryDataPoint {
            used_bytes: reading.used_bytes,
            used_peak_bytes_in_sample: frame_times.used_peak_bytes_in_sample,
            effective_cap_mib: reading.cap_mib,
        }),
        js_heap: sum_scene_js_heaps(scenes.iter()),
        nearby_player_count: nearby_players.iter().count() as u32,
        entity_count: latest_entity_count(&diagnostics).unwrap_or(0),
        target_fps,
        top_profiled_scope_summaries: profiled_scopes.recording_enabled().then(|| {
            profiled_scopes.take_sample_top_scope_summaries(
                SNAPSHOT_TOP_PROFILED_SCOPE_COUNT,
                ProfiledScopeSortMetric::SelfTime,
            )
        }),
        #[cfg(target_arch = "wasm32")]
        hud_main_thread_cost: frame_times.hud_main_thread_cost_sum,
        #[cfg(not(target_arch = "wasm32"))]
        hud_main_thread_cost: None,
        taken_at_real_seconds: time.elapsed_secs_f64(),
    };
    snapshots.write(snapshot.clone());
    commands.insert_resource(snapshot);
    sample.sampled_seconds = 0.0;
    frame_times.reset();
}

/// The effective frame-rate target: the session override, else the configured target; `None` with vsync on (the monitor sets the rate) or uncapped.
pub fn effective_target_fps(config: &AppConfig, session: Option<&SessionGraphics>) -> Option<f32> {
    if config.graphics.vsync {
        return None;
    }
    let target = session.map_or(config.graphics.fps_target, |session| {
        session.fps_target(config)
    });
    (target > 0 && target < 999).then_some(target as f32)
}

/// Every running scene's reported heap, summed; `None` when no scene reports one.
pub fn sum_scene_js_heaps<'a>(
    scenes: impl Iterator<Item = &'a RendererSceneContext>,
) -> Option<JsHeapDataPoint> {
    let mut sum = JsHeapDataPoint {
        used_bytes_sum: 0,
        limit_bytes_sum: 0,
        external_bytes_sum: 0,
        reporting_scene_count: 0,
    };
    for counters in scenes.filter_map(|scene| scene.resource_counters.as_ref()) {
        if counters.heap_limit == 0 {
            continue;
        }
        sum.used_bytes_sum += counters.heap_used;
        sum.limit_bytes_sum += counters.heap_limit;
        sum.external_bytes_sum += counters.external_memory;
        sum.reporting_scene_count += 1;
    }
    (sum.reporting_scene_count > 0).then_some(sum)
}
