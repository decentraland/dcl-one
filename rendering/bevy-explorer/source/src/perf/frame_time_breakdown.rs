//! Where the previous frame's wall time went ([`FrameTimeBreakdown`]): main-world schedules
//! (`First` through `Last`), the scene loop and its frame-limiter sleep (`SceneLoopTiming`), the
//! render world's CPU time and its swapchain wait, and the GPU time (`gpu/frame_time`).
//!
//! The render world runs on its own thread, and on the web in a worker: its timers reach the
//! main world through atomics.

use std::{
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::Duration,
};

use bevy::{
    diagnostic::{
        Diagnostic, DiagnosticPath, Diagnostics, DiagnosticsStore, FrameTimeDiagnosticsPlugin,
    },
    platform::time::Instant,
    prelude::*,
    render::{view::window::prepare_windows, Render, RenderApp, RenderSet},
};
use common::{
    gpu_time::GPU_FRAME_TIME,
    perf::{
        classify_frame_bottleneck, FrameTimeBreakdown, PERF_FRAME_LIMITER_SLEEP_MS,
        PERF_MAIN_WORLD_SCHEDULES_MS, PERF_RENDER_WORLD_CPU_MS, PERF_SCENE_RUNTIME_LOOP_MS,
        PERF_SWAPCHAIN_PRESENT_WAIT_MS,
    },
};
use scene_runner::SceneLoopTiming;

/// The main world's frame, `First` to `Last`.
#[derive(Resource)]
pub struct MainWorldFrameTimer {
    started: Option<Instant>,
    /// The last completed frame's `First` to `Last`.
    last_frame_schedules_ms: f32,
}

impl MainWorldFrameTimer {
    // not started is the timer's initial state, not a default value of the type
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            started: None,
            last_frame_schedules_ms: 0.0,
        }
    }
}

pub fn mark_main_world_frame_start_system(mut timer: ResMut<MainWorldFrameTimer>) {
    timer.started = Some(Instant::now());
}

pub fn mark_main_world_frame_end_system(mut timer: ResMut<MainWorldFrameTimer>) {
    if let Some(started) = timer.started.take() {
        timer.last_frame_schedules_ms = started.elapsed().as_secs_f32() * 1000.0;
    }
}

/// The render world's timers of its latest frame, published to the main world: a sequence that
/// moves with each frame, the schedule's CPU microseconds without the swapchain wait, and the
/// wait's microseconds.
#[derive(Resource, Clone)]
pub struct RenderWorldTimingsShared(Arc<RenderWorldTimingsAtomics>);

pub struct RenderWorldTimingsAtomics {
    published_frame_sequence: AtomicU32,
    render_world_cpu_us: AtomicU32,
    swapchain_present_wait_us: AtomicU32,
}

impl RenderWorldTimingsShared {
    fn new() -> Self {
        Self(Arc::new(RenderWorldTimingsAtomics {
            published_frame_sequence: AtomicU32::new(0),
            render_world_cpu_us: AtomicU32::new(0),
            swapchain_present_wait_us: AtomicU32::new(0),
        }))
    }

    /// Stores both timers, then moves the sequence.
    fn publish(&self, render_cpu: Duration, present_wait: Duration) {
        let micros = |duration: Duration| duration.as_micros().min(u32::MAX as u128) as u32;
        self.0
            .render_world_cpu_us
            .store(micros(render_cpu), Ordering::Relaxed);
        self.0
            .swapchain_present_wait_us
            .store(micros(present_wait), Ordering::Relaxed);
        self.0
            .published_frame_sequence
            .fetch_add(1, Ordering::Release);
    }

    /// The latest timers, in milliseconds, when the sequence moved since `last_seen_sequence`, which it updates.
    fn take_if_published_since(&self, last_seen_sequence: &mut u32) -> Option<(f32, f32)> {
        let sequence = self.0.published_frame_sequence.load(Ordering::Acquire);
        if sequence == *last_seen_sequence {
            return None;
        }
        *last_seen_sequence = sequence;
        Some((
            self.0.render_world_cpu_us.load(Ordering::Relaxed) as f32 / 1000.0,
            self.0.swapchain_present_wait_us.load(Ordering::Relaxed) as f32 / 1000.0,
        ))
    }
}

/// Render world: the frame's start, and the swapchain wait's start and length.
#[derive(Resource)]
struct RenderWorldFrameTimer {
    started: Option<Instant>,
    present_wait_started: Option<Instant>,
    present_wait: Duration,
}

/// Adds the render-world timers when there is a render app, and the publisher's shared state to
/// the main world.
pub fn add_render_world_timers(app: &mut App) {
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    let shared = RenderWorldTimingsShared::new();
    render_app
        .insert_resource(RenderWorldFrameTimer {
            started: None,
            present_wait_started: None,
            present_wait: Duration::ZERO,
        })
        .insert_resource(shared.clone())
        .add_systems(
            Render,
            (
                mark_render_world_frame_start_system.in_set(RenderSet::ExtractCommands),
                mark_present_wait_start_system
                    .in_set(RenderSet::ManageViews)
                    .before(prepare_windows),
                mark_present_wait_end_system
                    .in_set(RenderSet::ManageViews)
                    .after(prepare_windows),
                mark_render_world_frame_end_system.in_set(RenderSet::PostCleanup),
            ),
        );
    app.insert_resource(shared);
}

fn mark_render_world_frame_start_system(mut timer: ResMut<RenderWorldFrameTimer>) {
    timer.started = Some(Instant::now());
    timer.present_wait = Duration::ZERO;
}

fn mark_present_wait_start_system(mut timer: ResMut<RenderWorldFrameTimer>) {
    timer.present_wait_started = Some(Instant::now());
}

fn mark_present_wait_end_system(mut timer: ResMut<RenderWorldFrameTimer>) {
    if let Some(started) = timer.present_wait_started.take() {
        timer.present_wait = started.elapsed();
    }
}

fn mark_render_world_frame_end_system(
    mut timer: ResMut<RenderWorldFrameTimer>,
    shared: Res<RenderWorldTimingsShared>,
) {
    if let Some(started) = timer.started.take() {
        let whole = started.elapsed();
        shared.publish(whole.saturating_sub(timer.present_wait), timer.present_wait);
    }
}

/// Assembles [`FrameTimeBreakdown`] from the previous frame's timers and records
/// the `perf/*` diagnostics. The bottleneck is classified from the diagnostics' smoothed values,
/// so it does not flicker with one frame.
#[allow(clippy::too_many_arguments)]
pub fn publish_frame_time_breakdown_system(
    time: Res<Time<Real>>,
    main_timer: Res<MainWorldFrameTimer>,
    scene_loop: Option<Res<SceneLoopTiming>>,
    render: Option<Res<RenderWorldTimingsShared>>,
    mut last_seen_render_sequence: Local<u32>,
    mut last_render_world_timings: Local<Option<(f32, f32)>>,
    store: Res<DiagnosticsStore>,
    mut diagnostics: Diagnostics,
    mut breakdown: ResMut<FrameTimeBreakdown>,
) {
    if let Some(new) =
        render.and_then(|shared| shared.take_if_published_since(&mut last_seen_render_sequence))
    {
        *last_render_world_timings = Some(new);
    }
    let (render_world_cpu_ms, present_wait_ms) = last_render_world_timings.unwrap_or((0.0, 0.0));
    let scene_loop = scene_loop.as_deref().copied();
    let scene_runtime_loop_ms = scene_loop.map_or(0.0, |timing| timing.scene_loop_ms);
    let frame_limiter_sleep_ms = scene_loop.map_or(0.0, |timing| timing.frame_limiter_sleep_ms);
    let whole_frame_wall_ms = time.delta_secs_f64() as f32 * 1000.0;

    diagnostics.add_measurement(&PERF_MAIN_WORLD_SCHEDULES_MS, || {
        f64::from(main_timer.last_frame_schedules_ms)
    });
    diagnostics.add_measurement(&PERF_SCENE_RUNTIME_LOOP_MS, || {
        f64::from(scene_runtime_loop_ms)
    });
    diagnostics.add_measurement(&PERF_FRAME_LIMITER_SLEEP_MS, || {
        f64::from(frame_limiter_sleep_ms)
    });
    diagnostics.add_measurement(&PERF_RENDER_WORLD_CPU_MS, || f64::from(render_world_cpu_ms));
    if last_render_world_timings.is_some() {
        diagnostics.add_measurement(&PERF_SWAPCHAIN_PRESENT_WAIT_MS, || {
            f64::from(present_wait_ms)
        });
    }

    let smoothed = |path: &DiagnosticPath| store.get(path).and_then(Diagnostic::smoothed);
    let gpu_frame_time_ms = smoothed(&GPU_FRAME_TIME).map(|ms| ms as f32);
    let smoothed_frame = smoothed(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .map_or(whole_frame_wall_ms, |ms| ms as f32);
    // what paces the frame: the frame limiter's sleep natively; on the web, where the scene loop
    // never sleeps, the render worker's wait for the browser's next animation frame
    #[cfg(not(target_arch = "wasm32"))]
    let frame_pacing_wait_ms =
        smoothed(&PERF_FRAME_LIMITER_SLEEP_MS).map_or(frame_limiter_sleep_ms, |ms| ms as f32);
    #[cfg(target_arch = "wasm32")]
    let frame_pacing_wait_ms =
        smoothed(&bevy::render::pipelined_rendering::diagnostics::WAIT_FRAME)
            .map_or(0.0, |ms| ms as f32);
    let smoothed_present = last_render_world_timings
        .map(|_| smoothed(&PERF_SWAPCHAIN_PRESENT_WAIT_MS).map_or(present_wait_ms, |ms| ms as f32));
    *breakdown = FrameTimeBreakdown {
        whole_frame_wall_ms,
        main_world_schedules_ms: main_timer.last_frame_schedules_ms,
        scene_runtime_loop_ms,
        frame_limiter_sleep_ms,
        render_world_cpu_ms,
        swapchain_present_wait_ms: last_render_world_timings.map(|_| present_wait_ms),
        gpu_frame_time_ms,
        scene_ticks_run_count: scene_loop.map_or(0, |timing| timing.scene_schedule_run_count),
        scene_jobs_left_count: scene_loop.map_or(0, |timing| timing.eligible_scene_jobs_left_count),
        bottleneck: classify_frame_bottleneck(
            smoothed_frame,
            frame_pacing_wait_ms,
            gpu_frame_time_ms,
            smoothed_present,
        ),
    };
}
