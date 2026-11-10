//! How long each frame spends in bevy's scene spawner and in glTF processing (`update_ready_gltfs`),
//! for load-time comparisons. Filter the log with `spawn_timing=debug`.

use std::time::Duration;

use bevy::{diagnostic::FrameCount, prelude::*, scene::scene_spawner_system};
use web_time::Instant;

use super::update_ready_gltfs;

/// Frames that spend less than this on both are not logged.
const LOGGED_FROM: Duration = Duration::from_millis(1);

#[derive(Resource, Default)]
struct SpawnTiming {
    spawner_started: Option<Instant>,
    spawner: Duration,
    processing_started: Option<Instant>,
}

pub(super) fn register(app: &mut App) {
    app.init_resource::<SpawnTiming>().add_systems(
        SpawnScene,
        (
            start_spawner.before(scene_spawner_system),
            finish_spawner
                .after(scene_spawner_system)
                .before(update_ready_gltfs),
            finish_processing.after(update_ready_gltfs),
        )
            .run_if(logged),
    );
}

/// Whether the log takes `spawn_timing` lines: otherwise nothing is timed.
fn logged() -> bool {
    bevy::log::tracing::enabled!(target: "spawn_timing", bevy::log::Level::DEBUG)
}

fn start_spawner(mut timing: ResMut<SpawnTiming>) {
    timing.spawner_started = Some(Instant::now());
}

fn finish_spawner(mut timing: ResMut<SpawnTiming>) {
    timing.spawner = timing
        .spawner_started
        .take()
        .map(|started| started.elapsed())
        .unwrap_or_default();
    timing.processing_started = Some(Instant::now());
}

fn finish_processing(mut timing: ResMut<SpawnTiming>, frame: Res<FrameCount>) {
    let processing = timing
        .processing_started
        .take()
        .map(|started| started.elapsed())
        .unwrap_or_default();
    if timing.spawner.max(processing) >= LOGGED_FROM {
        debug!(
            target: "spawn_timing",
            "frame {}: scene spawner {:.1} ms, glTF processing {:.1} ms",
            frame.0,
            timing.spawner.as_secs_f64() * 1000.0,
            processing.as_secs_f64() * 1000.0
        );
    }
}
