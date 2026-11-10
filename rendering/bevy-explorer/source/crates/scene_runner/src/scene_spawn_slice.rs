//! Instantiation each frame stops a few milliseconds after bevy's scene spawner starts: the
//! spawner writes queued scene instances (glTF models, avatar parts) an entity at a time, then
//! `update_ready_gltfs` processes spawned glTFs a node at a time, within one slice; the rest waits
//! for the next frame. The spawner stops a little before the slice ends, so processing gets time
//! even while spawns are queued. Earlier in the frame, in `PostLoop`, the mesh renderer and
//! materials charge each entity's first mesh and first material to the same slice
//! ([`InstantiationBudget`]), which then ends that much earlier. Behind the loading backdrop and
//! on headless everything runs whole.

use std::time::Duration;

use bevy::{ecs::system::SystemParam, platform::time::Instant, prelude::*, scene::SceneSpawner};
use common::structs::{NoRenderApp, OutOfWorld, PrimaryUser};

/// How long instantiation runs each frame: what primitives and materials charge, then spawning
/// and glTF processing.
pub const SCENE_SPAWN_SLICE: Duration = Duration::from_millis(4);

/// The end of the slice kept for glTF processing.
pub const PROCESSING_SHARE: Duration = Duration::from_millis(1);

/// When glTF processing stops this frame. `None`: it processes whole.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq)]
pub struct ProcessingDeadline(pub Option<Instant>);

/// The estimated cost of what primitives and materials instantiated this frame, in `PostLoop`:
/// the work lands later in the frame (the command flush, `PostUpdate`, the render world), where
/// no clock in those systems sees it. The slice ends that much earlier for spawning and
/// processing.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq)]
pub struct InstantiationCharges(pub Duration);

/// The frame's instantiation slice as `PostLoop` systems see it.
#[derive(SystemParam)]
pub struct InstantiationBudget<'w, 's> {
    charges: ResMut<'w, InstantiationCharges>,
    no_render: Option<Res<'w, NoRenderApp>>,
    out_of_world: Query<'w, 's, (), (With<PrimaryUser>, With<OutOfWorld>)>,
}

impl InstantiationBudget<'_, '_> {
    /// False behind the loading backdrop and on headless: everything instantiates whole.
    pub fn is_bounded(&self) -> bool {
        self.no_render.is_none() && self.out_of_world.is_empty()
    }

    /// A system run's admissions.
    pub fn gate(&mut self) -> Gate<'_> {
        Gate {
            bounded: self.is_bounded(),
            charges: &mut self.charges.0,
            admitted: 0,
            deferred: 0,
        }
    }

    fn take_charges(&mut self) -> Duration {
        std::mem::take(&mut self.charges.0)
    }
}

/// Admits a system run's items against the slice: the run's first item whatever was spent, so
/// every consumer moves each frame; then each item while it fits in what is left.
pub struct Gate<'a> {
    bounded: bool,
    charges: &'a mut Duration,
    admitted: usize,
    deferred: usize,
}

impl Gate<'_> {
    pub fn admit(&mut self, cost: Duration) -> bool {
        if !self.bounded {
            return true;
        }
        if self.admitted > 0 && *self.charges + cost > SCENE_SPAWN_SLICE {
            self.deferred += 1;
            return false;
        }
        *self.charges += cost;
        self.admitted += 1;
        true
    }

    /// Logs the run under target `instantiation_budget` when it deferred anything.
    pub fn report(&self, consumer: &'static str) {
        if self.deferred > 0 {
            debug!(
                target: "instantiation_budget",
                "{consumer}: {} admitted, {} deferred, {:.2} ms charged this frame",
                self.admitted,
                self.deferred,
                self.charges.as_secs_f64() * 1000.0
            );
        }
    }
}

/// Runs in `SpawnScene`, just before bevy's `scene_spawner_system`.
pub fn slice_scene_spawning(
    mut spawner: ResMut<SceneSpawner>,
    mut processing: ResMut<ProcessingDeadline>,
    mut budget: InstantiationBudget,
) {
    let charged = budget.take_charges();
    let end = budget
        .is_bounded()
        .then(|| Instant::now() + SCENE_SPAWN_SLICE.saturating_sub(charged));
    spawner.set_spawn_deadline(end.map(|end| end - PROCESSING_SHARE));
    processing.0 = end;
}

pub(crate) mod diagnostics;

#[cfg(test)]
mod tests;
