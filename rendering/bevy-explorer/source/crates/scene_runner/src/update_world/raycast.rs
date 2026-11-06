use bevy::prelude::*;

use dcl_component::{proto_components::sdk::components::PbRaycast, SceneComponentId};
use dcl_types::interface::ComponentPosition;

use super::AddCrdtInterfaceExt;

pub struct RaycastPlugin;

#[derive(Component, Debug)]
pub struct Raycast {
    pub raycast: PbRaycast,
    pub last_run: u32,
    /// The engine frame the request was first left for a later frame by the raycast budget, 0
    /// while it is not waiting; cleared when it runs.
    pub deferred_since: u32,
}

impl From<PbRaycast> for Raycast {
    fn from(value: PbRaycast) -> Self {
        Self {
            raycast: value,
            last_run: 0,
            deferred_since: 0,
        }
    }
}

impl Plugin for RaycastPlugin {
    fn build(&self, app: &mut App) {
        app.add_crdt_lww_component::<PbRaycast, Raycast>(
            SceneComponentId::RAYCAST,
            ComponentPosition::EntityOnly,
        );
    }
}
