//! The preview server's commands for one model of a live scene: it sends them when the model's
//! file changes or is deleted, and the rest of the scene keeps running.

use bevy::{
    asset::AssetLoadFailedEvent,
    gltf::{Gltf, GltfMesh},
    platform::collections::HashSet,
    prelude::*,
};
use ipfs::ipfs_path::content_file_path;

use crate::{
    update_world::gltf_container::{
        reload_gltf_container, GltfDefinition, GltfHandle, SceneResourceLookup,
    },
    SceneEntity,
};

/// The file `src` of the live scene `scene` changed: its models load it again.
#[derive(Event, Debug, Clone, PartialEq)]
pub(super) struct ReloadModel {
    pub scene: Entity,
    pub src: String,
}

/// The file `src` of the live scene `scene` was deleted: its models go.
#[derive(Event, Debug, Clone, PartialEq)]
pub(super) struct RemoveModel {
    pub scene: Entity,
    pub src: String,
}

/// A container whose file is being loaded again: it loads its value again once the asset is.
#[derive(Component, Debug, PartialEq)]
pub(super) struct ModelReloadPending(AssetId<Gltf>);

/// The scene names a file as its content map does, which ignores case and a leading separator.
fn same_file(a: &str, b: &str) -> bool {
    content_file_path(a).eq_ignore_ascii_case(&content_file_path(b))
}

/// A container of `scene` showing `src`.
fn shows(scene_ent: &SceneEntity, definition: &GltfDefinition, scene: Entity, src: &str) -> bool {
    scene_ent.root == scene && same_file(definition.src(), src)
}

/// Removing the definition releases the model whatever its state, also one still loading or
/// processing (`on_gltf_container_removed`). The scene putting the value again brings it back.
pub(super) fn remove_model(
    mut commands: Commands,
    mut events: EventReader<RemoveModel>,
    containers: Query<(Entity, &SceneEntity, &GltfDefinition)>,
) {
    for RemoveModel { scene, src } in events.read() {
        debug!("removing model {src} from {scene}");
        for (entity, scene_ent, definition) in &containers {
            if !shows(scene_ent, definition, *scene, src) {
                continue;
            }
            commands
                .entity(entity)
                .try_remove::<(GltfDefinition, GltfHandle, ModelReloadPending)>();
        }
    }
}

/// The asset server keeps a loaded file while any handle holds it, so the file is reloaded
/// explicitly, and its containers load their value again once it is (`finish_model_reloads`).
/// A container with no file (it was not found) tries to load it straight away.
pub(super) fn reload_model(
    mut commands: Commands,
    mut events: EventReader<ReloadModel>,
    containers: Query<(Entity, &SceneEntity, &GltfDefinition, Option<&GltfHandle>)>,
    asset_server: Res<AssetServer>,
) {
    let mut reloading = HashSet::new();
    for ReloadModel { scene, src } in events.read() {
        debug!("reloading model {src} in {scene}");
        for (entity, scene_ent, definition, h_gltf) in &containers {
            if !shows(scene_ent, definition, *scene, src) {
                continue;
            }
            let Some(id) = h_gltf.map(GltfHandle::id) else {
                reload_gltf_container(&mut commands, entity);
                continue;
            };
            if !reloading.contains(&id) {
                let Some(path) = asset_server.get_path(id) else {
                    // not loaded from a file: nothing to fetch again
                    reload_gltf_container(&mut commands, entity);
                    continue;
                };
                asset_server.reload(path.into_owned());
                reloading.insert(id);
            }
            commands.entity(entity).try_insert(ModelReloadPending(id));
        }
    }
}

/// A reload is over when the asset is loaded with its dependencies, or replaced (`source` is set
/// again: dropping it after a load is a modification too), or failed. Runs before the reloads are
/// requested in a frame, so a container waits only for events sent after its request.
pub(super) fn finish_model_reloads(
    mut commands: Commands,
    mut asset_events: EventReader<AssetEvent<Gltf>>,
    mut failures: EventReader<AssetLoadFailedEvent<Gltf>>,
    pending: Query<(Entity, &SceneEntity, &ModelReloadPending)>,
    gltfs: Res<Assets<Gltf>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    mut lookups: Query<&mut SceneResourceLookup>,
) {
    let mut done: HashSet<AssetId<Gltf>> = asset_events
        .read()
        .filter_map(|event| match event {
            AssetEvent::LoadedWithDependencies { id } => Some(*id),
            AssetEvent::Modified { id }
                if gltfs.get(*id).is_some_and(|gltf| gltf.source.is_some()) =>
            {
                Some(*id)
            }
            _ => None,
        })
        .collect();
    done.extend(failures.read().map(|failure| failure.id));
    if done.is_empty() {
        return;
    }

    let mut forgotten = HashSet::new();
    for (entity, scene_ent, ModelReloadPending(id)) in &pending {
        if !done.contains(id) {
            continue;
        }
        if forgotten.insert((scene_ent.root, *id)) {
            if let (Some(gltf), Ok(mut lookup)) = (gltfs.get(*id), lookups.get_mut(scene_ent.root))
            {
                lookup.forget_gltf(gltf, &gltf_meshes);
            }
        }
        commands.entity(entity).try_remove::<ModelReloadPending>();
        reload_gltf_container(&mut commands, entity);
    }
}

#[cfg(test)]
mod tests;
