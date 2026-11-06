//! Bevy's scene spawner writes queued scene instances (glTF models, avatar parts) an entity at a
//! time and stops a few milliseconds after it starts each frame; the rest waits for the next frame.
//! Behind the loading backdrop and on headless it spawns them whole.

use std::time::Duration;

use bevy::{platform::time::Instant, prelude::*, scene::SceneSpawner};
use common::structs::{NoRenderApp, OutOfWorld, PrimaryUser};

/// How long the scene spawner writes each frame.
pub const SCENE_SPAWN_SLICE: Duration = Duration::from_millis(4);

/// Runs in `SpawnScene`, just before bevy's `scene_spawner_system`.
pub fn slice_scene_spawning(
    mut spawner: ResMut<SceneSpawner>,
    no_render: Option<Res<NoRenderApp>>,
    out_of_world: Query<(), (With<PrimaryUser>, With<OutOfWorld>)>,
) {
    let whole = no_render.is_some() || !out_of_world.is_empty();
    spawner.set_spawn_deadline((!whole).then(|| Instant::now() + SCENE_SPAWN_SLICE));
}

#[cfg(test)]
mod tests;
