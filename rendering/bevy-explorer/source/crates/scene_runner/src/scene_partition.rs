use std::time::Duration;

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

/// A scene behind the camera is sent at most this often (unity-explorer's `behindFps = 1`).
pub const BEHIND_SCENE_TICK_INTERVAL: Duration = Duration::from_secs(1);

/// Where partitions are measured from: the rendering camera's position and forward, or
/// the player's position and no forward (zero: nothing is behind) when nothing renders. The
/// headless server has a stand-in `PrimaryCamera` without a `Camera` that never turns.
pub fn sampling_transform(
    camera: &Query<&GlobalTransform, (With<PrimaryCamera>, With<Camera>)>,
    player: &Query<&GlobalTransform, With<PrimaryUser>>,
) -> Option<(Vec3, Vec3)> {
    if let Ok(camera) = camera.single() {
        return Some((camera.translation(), camera.forward().as_vec3()));
    }
    player
        .single()
        .ok()
        .map(|player| (player.translation(), Vec3::ZERO))
}

pub fn update_scene_partitions(
    camera: Query<&GlobalTransform, (With<PrimaryCamera>, With<Camera>)>,
    player: Query<&GlobalTransform, With<PrimaryUser>>,
    mut scenes: Query<(Entity, &RendererSceneContext, Option<&mut ScenePartition>)>,
    mut commands: Commands,
) {
    let Some((position, forward)) = sampling_transform(&camera, &player) else {
        return;
    };

    for (entity, context, maybe_partition) in scenes.iter_mut() {
        let partition = ScenePartition(scene_partition(
            position,
            forward,
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
