//! How long each frame spends in bevy's scene spawner and in glTF processing
//! (`update_ready_gltfs`): the diagnostics [`SCENE_SPAWNER`] and [`GLTF_PROCESSING`], in
//! milliseconds, for the debug panel and load-time comparisons. With `spawn_timing=debug` in the
//! log filter, it also logs the model whose processing took longest in a frame, when that is a
//! millisecond or more.

use std::time::Duration;

use bevy::{
    diagnostic::{Diagnostic, DiagnosticPath, Diagnostics, RegisterDiagnostic},
    platform::time::Instant,
    prelude::*,
    scene::scene_spawner_system,
};

use crate::update_world::gltf_container::{update_ready_gltfs, GltfDefinition};

/// Milliseconds a frame spends in bevy's `scene_spawner_system`.
pub const SCENE_SPAWNER: DiagnosticPath = DiagnosticPath::const_new("instantiation/scene_spawner");

/// Milliseconds a frame spends processing spawned glTF models (`update_ready_gltfs`).
pub const GLTF_PROCESSING: DiagnosticPath =
    DiagnosticPath::const_new("instantiation/gltf_processing");

/// The slowest model is logged from this long.
const LOGGED_FROM: Duration = Duration::from_millis(1);

/// One model's share of a frame's glTF processing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProcessedModel {
    pub container: Entity,
    pub nodes: usize,
    pub time: Duration,
}

/// The model whose processing took longest this frame: `update_ready_gltfs` records each model it
/// processes, and [`SpawnTimingPlugin`] reports and clears it once processing ends.
#[derive(Resource, Default, Debug)]
pub struct SlowestProcessing(Option<ProcessedModel>);

impl SlowestProcessing {
    pub fn record(&mut self, model: ProcessedModel) {
        if self.0.is_none_or(|slowest| model.time > slowest.time) {
            self.0 = Some(model);
        }
    }
}

#[derive(Resource, Default)]
struct SpawnTiming {
    spawner_started: Option<Instant>,
    processing_started: Option<Instant>,
}

pub struct SpawnTimingPlugin;

impl Plugin for SpawnTimingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpawnTiming>()
            .init_resource::<SlowestProcessing>()
            .register_diagnostic(Diagnostic::new(SCENE_SPAWNER).with_suffix("ms"))
            .register_diagnostic(Diagnostic::new(GLTF_PROCESSING).with_suffix("ms"))
            .add_systems(
                SpawnScene,
                (
                    start_spawner.before(scene_spawner_system),
                    finish_spawner
                        .after(scene_spawner_system)
                        .before(update_ready_gltfs),
                    finish_processing.after(update_ready_gltfs),
                ),
            );
    }
}

fn milliseconds(started: Option<Instant>) -> f64 {
    started.map_or(0.0, |started| started.elapsed().as_secs_f64() * 1000.0)
}

fn start_spawner(mut timing: ResMut<SpawnTiming>) {
    timing.spawner_started = Some(Instant::now());
}

fn finish_spawner(mut timing: ResMut<SpawnTiming>, mut diagnostics: Diagnostics) {
    let started = timing.spawner_started.take();
    diagnostics.add_measurement(&SCENE_SPAWNER, || milliseconds(started));
    timing.processing_started = Some(Instant::now());
}

fn finish_processing(
    mut timing: ResMut<SpawnTiming>,
    mut slowest: ResMut<SlowestProcessing>,
    mut diagnostics: Diagnostics,
    definitions: Query<&GltfDefinition>,
) {
    let started = timing.processing_started.take();
    diagnostics.add_measurement(&GLTF_PROCESSING, || milliseconds(started));
    let Some(model) = slowest.0.take().filter(|model| model.time >= LOGGED_FROM) else {
        return;
    };
    if bevy::log::tracing::enabled!(target: "spawn_timing", bevy::log::Level::DEBUG) {
        let src = definitions
            .get(model.container)
            .map_or("a removed model", |definition| definition.src());
        debug!(
            target: "spawn_timing",
            "slowest glTF processing this frame: {:.1} ms for {} nodes of {src}",
            model.time.as_secs_f64() * 1000.0,
            model.nodes,
        );
    }
}

#[cfg(test)]
mod tests;
