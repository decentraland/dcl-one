//! What the parcel scene the player stands in says about portable experiences
//! (`featureToggles.portableExperiences`), applied to every listed portable: `disabled` pauses
//! them through [`RendererSceneContext::blocked`] and hides their UI, `hideUi` hides their UI.

use bevy::prelude::*;
use common::{
    sets::SceneSets,
    structs::{PortableExperiencesToggle, PrimaryUser},
};

use crate::{
    initialize_scene::{LiveScenes, PortableScenes, PortableSource, SuperUserScene},
    renderer_context::RendererSceneContext,
    ContainingScene,
};

/// The `blocked` reason a portable carries while the player stands in a scene that disables
/// portable experiences.
pub const PORTABLE_EXPERIENCES_BLOCK: &str = "portable_experiences_disabled";

pub struct PortableExperiencesPlugin;

impl Plugin for PortableExperiencesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, block_portables.in_set(SceneSets::PostInit));
    }
}

/// A portable the player sees, can stop, and that a scene's toggle applies to: not a startup
/// portable and not a super-user scene (the HUD's own).
pub fn is_listed(source: &PortableSource, is_super: bool) -> bool {
    !source.is_startup() && !source.super_user && !is_super
}

/// The toggle of the parcel scene the player stands in.
pub fn current_toggle<'a>(
    player: Option<Entity>,
    containing_scene: &ContainingScene,
    context: impl Fn(Entity) -> Option<&'a RendererSceneContext>,
) -> PortableExperiencesToggle {
    player
        .and_then(|player| containing_scene.get_parcel(player))
        .and_then(context)
        .map(|ctx| ctx.portable_experiences)
        .unwrap_or_default()
}

fn block_portables(
    player: Query<Entity, With<PrimaryUser>>,
    containing_scene: ContainingScene,
    portables: Res<PortableScenes>,
    live_scenes: Res<LiveScenes>,
    mut contexts: Query<(&mut RendererSceneContext, Has<SuperUserScene>)>,
) {
    let toggle = current_toggle(player.single().ok(), &containing_scene, |scene| {
        contexts.get(scene).ok().map(|(ctx, _)| ctx)
    });
    let block = toggle == PortableExperiencesToggle::Disabled;
    for (hash, source) in portables.iter() {
        let Some(scene) = live_scenes.scenes.get(hash) else {
            continue;
        };
        let Ok((mut ctx, is_super)) = contexts.get_mut(*scene) else {
            continue;
        };
        if !is_listed(source, is_super) {
            continue;
        }
        // compare first so an unchanged context is not marked changed every frame
        if block && !ctx.blocked.contains(PORTABLE_EXPERIENCES_BLOCK) {
            ctx.blocked.insert(PORTABLE_EXPERIENCES_BLOCK);
        } else if !block && ctx.blocked.contains(PORTABLE_EXPERIENCES_BLOCK) {
            ctx.blocked.remove(PORTABLE_EXPERIENCES_BLOCK);
        }
    }
}
