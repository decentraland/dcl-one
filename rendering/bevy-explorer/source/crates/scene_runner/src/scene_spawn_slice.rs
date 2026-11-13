//! Instantiation each frame stops a share of the target frame after bevy's scene spawner starts:
//! the spawner writes queued scene instances (glTF models, avatar parts) an entity at a time, then
//! `update_ready_gltfs` processes spawned glTFs a node at a time, within one slice; the rest waits
//! for the next frame. The spawner stops a little before the slice ends, so processing gets time
//! even while spawns are queued. Behind the loading backdrop and on headless both run whole.
//!
//! [`SceneSpawnSlicePlugin`] owns the slice: its deadlines, how long the spawner and processing
//! take each frame (`spawn_timing`) and the debug panel's Instantiation section.

use std::time::Duration;

use bevy::{
    platform::time::Instant,
    prelude::*,
    scene::{scene_spawner_system, SceneSpawner},
};
use common::structs::{NoRenderApp, OutOfWorld, PrimaryUser};

use crate::{update_scene::raycast_budget::UNCAPPED_FRAME, SceneUpdates};

/// Spawning and glTF processing together may spend a quarter of the target frame.
pub const SLICE_DIVISOR: u32 = 4;

/// The last quarter of the slice is kept for glTF processing.
pub const PROCESSING_DIVISOR: u32 = 4;

/// How long spawning and glTF processing run together each frame: a quarter of the target frame
/// (of 60 fps when the frame rate is uncapped), 4.2 ms at 60 fps. The target, not the measured
/// frame, so a slow frame does not raise the next frame's slice.
pub fn slice(frame_target: Option<Duration>) -> Duration {
    frame_target.unwrap_or(UNCAPPED_FRAME) / SLICE_DIVISOR
}

/// The end of `slice` kept for glTF processing.
pub fn processing_share(slice: Duration) -> Duration {
    slice / PROCESSING_DIVISOR
}

/// When glTF processing stops this frame. `None`: it processes whole.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq)]
pub struct ProcessingDeadline(pub Option<Instant>);

pub struct SceneSpawnSlicePlugin;

impl Plugin for SceneSpawnSlicePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ProcessingDeadline>()
            .add_systems(
                SpawnScene,
                slice_scene_spawning.before(scene_spawner_system),
            )
            .add_plugins(spawn_timing::SpawnTimingPlugin);
        if app.world().get_resource::<NoRenderApp>().is_none() {
            diagnostics::add_debug_section(app);
        }
    }
}

/// Runs in `SpawnScene`, just before bevy's `scene_spawner_system`.
pub fn slice_scene_spawning(
    mut spawner: ResMut<SceneSpawner>,
    mut processing: ResMut<ProcessingDeadline>,
    updates: Res<SceneUpdates>,
    no_render: Option<Res<NoRenderApp>>,
    out_of_world: Query<(), (With<PrimaryUser>, With<OutOfWorld>)>,
) {
    let whole = no_render.is_some() || !out_of_world.is_empty();
    let slice = (!whole).then(|| slice(updates.frame_target));
    let end = slice.map(|slice| Instant::now() + slice);
    spawner.set_spawn_deadline(
        end.zip(slice)
            .map(|(end, slice)| end - processing_share(slice)),
    );
    processing.0 = end;
}

mod diagnostics;
pub mod spawn_timing;

#[cfg(test)]
mod tests;
