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
    #[allow(
        clippy::unwrap_used,
        reason = "PrimaryUser is a mandatory singleton; a missing player is an engine bug"
    )]
    pub fn player(&self) -> Entity {
        self.player.single().unwrap()
    }

    #[allow(
        clippy::unwrap_used,
        reason = "PrimaryCamera is a mandatory singleton; a missing camera is an engine bug"
    )]
    pub fn camera(&self) -> Entity {
        self.camera.single().unwrap()
    }

    pub fn primary_or_scene(
        &self,
        id: SceneEntityId,
        context: &RendererSceneContext,
    ) -> Option<Entity> {
        match id {
            SceneEntityId::PLAYER => Some(self.player()),
            SceneEntityId::CAMERA => Some(self.camera()),
            _ => context.bevy_entity(id),
        }
    }
}
