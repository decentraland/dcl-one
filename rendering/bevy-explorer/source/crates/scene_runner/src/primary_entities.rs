use bevy::{ecs::system::SystemParam, prelude::*};
use common::structs::{PrimaryCamera, PrimaryUser};
use dcl_component::SceneEntityId;

use crate::renderer_context::RendererSceneContext;

#[derive(SystemParam)]
pub struct PrimaryEntities<'w, 's> {
    player: Query<'w, 's, Entity, With<PrimaryUser>>,
    camera: Query<'w, 's, Entity, With<PrimaryCamera>>,
}

impl PrimaryEntities<'_, '_> {
    pub fn player(&self) -> Option<Entity> {
        self.player.single().ok()
    }

    pub fn camera(&self) -> Option<Entity> {
        self.camera.single().ok()
    }

    pub fn primary_or_scene(
        &self,
        id: SceneEntityId,
        context: &RendererSceneContext,
    ) -> Option<Entity> {
        match id {
            SceneEntityId::PLAYER => self.player(),
            SceneEntityId::CAMERA => self.camera(),
            _ => context.bevy_entity(id),
        }
    }
}
