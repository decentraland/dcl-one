use bevy::prelude::*;
use common::{
    partition::{scene_partition, LoadPriority, ScenePartitionData},
    structs::{PrimaryCamera, PrimaryUser},
};
use ipfs::IpfsResource;

use crate::{initialize_scene::SceneHash, renderer_context::RendererSceneContext, ContainingScene};

/// Where the scene is relative to the camera, refreshed every frame for live scenes by
/// `update_scene_partitions`, and written by the lifecycle for scenes still loading.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct ScenePartition(pub ScenePartitionData);

/// A scene behind the camera counts as this many times farther in the tick queue: it keeps
/// ticking, and yields first when frame time runs short.
pub const BEHIND_SCENE_TICK_PENALTY: f32 = 4.0;

/// Where partitions are measured from: distance from the player, facing from the rendering
/// camera (a `PrimaryCamera` with a `Camera`). Without one, as on the headless server whose
/// stand-in `PrimaryCamera` has no `Camera` and never turns, no facing is known and nothing is
/// behind.
pub fn sampling_transform(
    camera: &Query<&GlobalTransform, (With<PrimaryCamera>, With<Camera>)>,
    player: &Query<&GlobalTransform, With<PrimaryUser>>,
) -> Option<(Vec3, Option<(Vec3, Vec3)>)> {
    let player = player.single().ok()?;
    let camera = camera
        .single()
        .ok()
        .map(|camera| (camera.translation(), camera.forward().as_vec3()));
    Some((player.translation(), camera))
}

pub fn update_scene_partitions(
    camera: Query<&GlobalTransform, (With<PrimaryCamera>, With<Camera>)>,
    player: Query<&GlobalTransform, With<PrimaryUser>>,
    mut scenes: Query<(Entity, &RendererSceneContext, Option<&mut ScenePartition>)>,
    mut commands: Commands,
) {
    let Some((origin, camera)) = sampling_transform(&camera, &player) else {
        return;
    };

    for (entity, context, maybe_partition) in scenes.iter_mut() {
        let partition = ScenePartition(scene_partition(
            origin,
            camera,
            context.parcels.iter().copied(),
        ));
        match maybe_partition {
            Some(mut current) => {
                current.set_if_neq(partition);
            }
            None => {
                commands.entity(entity).try_insert(partition);
            }
        }
    }
}

/// Every scene's download priority, by hash, for the ipfs request slots: the player's scenes
/// first, then by partition.
pub fn publish_download_priorities(
    scenes: Query<(Entity, &SceneHash, &ScenePartition)>,
    player: Query<Entity, With<PrimaryUser>>,
    containing_scene: ContainingScene,
    ipfs: Res<IpfsResource>,
) {
    let active = player
        .single()
        .map(|player| containing_scene.get(player))
        .unwrap_or_default();
    let priorities = scenes
        .iter()
        .map(|(entity, hash, partition)| {
            let priority = if active.contains(&entity) {
                LoadPriority::TOP
            } else {
                partition.0.load_priority()
            };
            (hash.0.clone(), priority)
        })
        .collect();
    ipfs.set_scene_priorities(priorities);
}

#[cfg(test)]
mod tests;
