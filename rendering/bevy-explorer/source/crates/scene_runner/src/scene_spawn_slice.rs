//! Instantiation each frame stops a few milliseconds after bevy's scene spawner starts: the
//! spawner writes queued scene instances (glTF models, avatar parts) an entity at a time, then
//! `update_ready_gltfs` processes spawned glTFs a node at a time, within one slice; the rest waits
//! for the next frame. The spawner stops a little before the slice ends, so processing gets time
//! even while spawns are queued. Behind the loading backdrop and on headless both run whole.

use std::time::Duration;

use bevy::{platform::time::Instant, prelude::*, scene::SceneSpawner};
use common::structs::{NoRenderApp, OutOfWorld, PrimaryUser};

/// How long spawning and glTF processing run together each frame.
pub const SCENE_SPAWN_SLICE: Duration = Duration::from_millis(4);

/// The end of the slice kept for glTF processing.
pub const PROCESSING_SHARE: Duration = Duration::from_millis(1);

/// When glTF processing stops this frame. `None`: it processes whole.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq)]
pub struct ProcessingDeadline(pub Option<Instant>);

/// Runs in `SpawnScene`, just before bevy's `scene_spawner_system`.
pub fn slice_scene_spawning(
    mut spawner: ResMut<SceneSpawner>,
    mut processing: ResMut<ProcessingDeadline>,
    no_render: Option<Res<NoRenderApp>>,
    out_of_world: Query<(), (With<PrimaryUser>, With<OutOfWorld>)>,
) {
    let whole = no_render.is_some() || !out_of_world.is_empty();
    let end = (!whole).then(|| Instant::now() + SCENE_SPAWN_SLICE);
    spawner.set_spawn_deadline(end.map(|end| end - PROCESSING_SHARE));
    processing.0 = end;
}

pub(crate) mod diagnostics;

#[cfg(test)]
mod tests;
