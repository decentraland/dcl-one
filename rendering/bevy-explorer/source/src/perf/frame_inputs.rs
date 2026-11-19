//! The frame's raw arrivals, gathered once in `First` into [`FrameInputs`]: the one place every
//! measurement enters, which the sample ([`super::populating_sample`]) and the benchmark
//! accumulator ([`super::benchmark_summary`]) consume. Each source is polled for its latest
//! value; a source that produced nothing this frame leaves `None`.
//!
//! The scope profiler's spans are not here: a frame's spans are complete only in `Last`, where
//! the table folds them ([`super::profiled_scopes`]).

use bevy::{
    diagnostic::{Diagnostic, DiagnosticsStore, EntityCountDiagnosticsPlugin},
    platform::time::Instant,
    prelude::*,
};
use common::{
    gpu_time::GPU_FRAME_TIME,
    memory::{MemoryReading, MemoryStatus},
    perf::{HudMainThreadCostDataPoint, JsHeapDataPoint, SamplingPausedReason},
    structs::{
        AppConfig, ClientWindowFocus, CurrentRealm, OutOfWorld, PrimaryUser, SessionGraphics,
        WorldHold,
    },
};
use scene_runner::renderer_context::RendererSceneContext;
use wallet::Wallet;

/// What the frame brought, as of `First`.
#[derive(Resource, Debug)]
pub struct FrameInputs {
    /// This frame's `Time<Real>` delta.
    pub frame_delta_seconds: f64,
    /// `Time<Real>` elapsed at this frame.
    pub elapsed_real_seconds: f64,
    /// A `gpu/frame_time` measurement newer than the one taken before; `None` when the
    /// renderer has none or has not produced a new one.
    pub new_gpu_frame_time_ms: Option<f32>,
    /// The memory reading; `None` when the platform gives none.
    pub memory_reading: Option<MemoryReading>,
    /// Every running scene's reported heap, summed; `None` when no scene reports one.
    pub js_heap: Option<JsHeapDataPoint>,
    /// The latest entity count; `None` before the first.
    pub entity_count: Option<u32>,
    /// The effective frame-rate target: the session override, else the configured target;
    /// `None` with vsync on or uncapped.
    pub target_fps: Option<f32>,
    /// The session and focus signals the sampling status follows.
    pub session: SessionSignals,
    /// The HUD cost window the page stored since the last frame (web); `None` on native or
    /// when none arrived.
    pub hud_main_thread_cost_window: Option<HudMainThreadCostDataPoint>,
    /// The last window that arrived, kept for the debug panel; `None` before the first.
    pub last_hud_main_thread_cost_window: Option<HudMainThreadCostDataPoint>,
}

impl FrameInputs {
    /// Before the first frame: nothing arrived.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            frame_delta_seconds: 0.0,
            elapsed_real_seconds: 0.0,
            new_gpu_frame_time_ms: None,
            memory_reading: None,
            js_heap: None,
            entity_count: None,
            target_fps: None,
            session: SessionSignals {
                signed_in: false,
                world_held: false,
                realm_connected: false,
                out_of_world: None,
                focus_paused: None,
            },
            hud_main_thread_cost_window: None,
            last_hud_main_thread_cost_window: None,
        }
    }
}

/// The session and focus signals, as the sampling status reads them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionSignals {
    /// The wallet has an address.
    pub signed_in: bool,
    /// A `WorldHold` resource exists.
    pub world_held: bool,
    /// The current realm is connected.
    pub realm_connected: bool,
    /// The primary user's `OutOfWorld`; `None` without a primary user.
    pub out_of_world: Option<bool>,
    /// `ViewportInactive` while the web viewport is inactive, else `WindowUnfocused` while the
    /// client window lacks focus; `None` otherwise.
    pub focus_paused: Option<SamplingPausedReason>,
}

/// Fills [`FrameInputs`] from every source. Runs after `TimeSystem` and after the client window
/// focus is copied in.
pub(crate) fn collect_frame_inputs_system(
    mut inputs: ResMut<FrameInputs>,
    time: Res<Time<Real>>,
    diagnostics: Res<DiagnosticsStore>,
    mut last_gpu_frame_time_measured_at: Local<Option<Instant>>,
    memory: Res<MemoryStatus>,
    scenes: Query<&RendererSceneContext>,
    config: Res<AppConfig>,
    session_graphics: Option<Res<SessionGraphics>>,
    wallet: Option<Res<Wallet>>,
    world_hold: Option<Res<WorldHold>>,
    realm: Option<Res<CurrentRealm>>,
    player: Query<Has<OutOfWorld>, With<PrimaryUser>>,
    client_window_focus: Res<ClientWindowFocus>,
) {
    #[cfg(target_arch = "wasm32")]
    let viewport_paused =
        (!crate::web::viewport_active()).then_some(SamplingPausedReason::ViewportInactive);
    #[cfg(not(target_arch = "wasm32"))]
    let viewport_paused = None;
    let focus_paused = viewport_paused.or_else(|| {
        (*client_window_focus == ClientWindowFocus::NoFocus)
            .then_some(SamplingPausedReason::WindowUnfocused)
    });
    #[cfg(target_arch = "wasm32")]
    let hud_main_thread_cost_window = crate::web::take_hud_main_thread_cost_window();
    #[cfg(not(target_arch = "wasm32"))]
    let hud_main_thread_cost_window = None;

    inputs.frame_delta_seconds = time.delta_secs_f64();
    inputs.elapsed_real_seconds = time.elapsed_secs_f64();
    // taken every frame, so a reading from a pause is consumed before sampling resumes
    inputs.new_gpu_frame_time_ms =
        new_gpu_frame_time_ms(&diagnostics, &mut last_gpu_frame_time_measured_at);
    inputs.memory_reading = memory.reading();
    inputs.js_heap = sum_scene_js_heaps(scenes.iter());
    inputs.entity_count = latest_entity_count(&diagnostics);
    inputs.target_fps = effective_target_fps(&config, session_graphics.as_deref());
    inputs.session = SessionSignals {
        signed_in: wallet
            .as_ref()
            .is_some_and(|wallet| wallet.address().is_some()),
        world_held: world_hold.is_some(),
        realm_connected: realm.as_ref().is_some_and(|realm| realm.connected),
        out_of_world: player.single().ok(),
        focus_paused,
    };
    inputs.hud_main_thread_cost_window = hud_main_thread_cost_window;
    if hud_main_thread_cost_window.is_some() {
        inputs.last_hud_main_thread_cost_window = hud_main_thread_cost_window;
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
