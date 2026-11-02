use bevy::prelude::*;
use std::collections::HashMap;

use common::{
    partition::{point_partition, scene_partition, LoadPriority, PartitionData, Tier},
    structs::{PrimaryCamera, PrimaryUser},
};
use ipfs::IpfsResource;

use crate::{initialize_scene::SceneHash, renderer_context::RendererSceneContext, ContainingScene};

/// Where a scene, or an entity with `PartitionedDownloads`, is relative to the player and the
/// camera; refreshed every frame by `update_scene_partitions`.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Partition(pub PartitionData);

/// The content hashes whose downloads take this entity's partition: an avatar's body and
/// wearables. Scenes need none; their downloads carry the scene's own hash.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct PartitionedDownloads(pub Vec<String>);

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

#[allow(clippy::type_complexity)]
pub fn update_scene_partitions(
    camera: Query<&GlobalTransform, (With<PrimaryCamera>, With<Camera>)>,
    player: Query<&GlobalTransform, With<PrimaryUser>>,
    mut scenes: Query<(Entity, &RendererSceneContext, Option<&mut Partition>)>,
    mut points: Query<
        (Entity, &GlobalTransform, Option<&mut Partition>),
        (With<PartitionedDownloads>, Without<RendererSceneContext>),
    >,
    mut commands: Commands,
) {
    let Some((origin, camera)) = sampling_transform(&camera, &player) else {
        return;
    };

    let mut apply =
        |entity: Entity, partition: Partition, current: Option<Mut<Partition>>| match current {
            Some(mut current) => {
                current.set_if_neq(partition);
            }
            None => {
                commands.entity(entity).try_insert(partition);
            }
        };

    for (entity, context, current) in scenes.iter_mut() {
        let partition = scene_partition(origin, camera, context.parcels.iter().copied());
        apply(entity, Partition(partition), current);
    }
    for (entity, transform, current) in points.iter_mut() {
        let partition = point_partition(origin, camera, transform.translation());
        apply(entity, Partition(partition), current);
    }
}

/// Every download priority, by content hash, for the ipfs request slots: the player's scenes
/// first; then the avatars' bodies and wearables, nearest avatar first, a shared wearable at the
/// best of its avatars; then the other scenes by partition.
pub fn publish_download_priorities(
    scenes: Query<(Entity, &SceneHash, &Partition)>,
    partitioned: Query<(&Partition, &PartitionedDownloads)>,
    player: Query<Entity, With<PrimaryUser>>,
    containing_scene: ContainingScene,
    ipfs: Res<IpfsResource>,
) {
    let active = player
        .single()
        .map(|player| containing_scene.get(player))
        .unwrap_or_default();
    let mut priorities: HashMap<String, LoadPriority> = scenes
        .iter()
        .map(|(entity, hash, partition)| {
            let priority = if active.contains(&entity) {
                LoadPriority::TOP
            } else {
                partition.0.load_priority(Tier::Scene)
            };
            (hash.0.clone(), priority)
        })
        .collect();
    for (partition, downloads) in &partitioned {
        let priority = partition.0.load_priority(Tier::Avatar);
        for hash in &downloads.0 {
            priorities
                .entry(hash.clone())
                .and_modify(|current| *current = (*current).min(priority))
                .or_insert(priority);
        }
    }
    ipfs.set_scene_priorities(priorities);
}

#[cfg(test)]
mod tests;
