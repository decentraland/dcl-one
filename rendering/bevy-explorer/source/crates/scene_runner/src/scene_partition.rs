use bevy::{
    ecs::{component::HookContext, world::DeferredWorld},
    prelude::*,
};

use common::{
    partition::{point_partition, scene_partition, LoadPriority, Tier},
    structs::{PrimaryCamera, PrimaryUser},
};
use ipfs::IpfsResource;

use crate::{initialize_scene::SceneHash, renderer_context::RendererSceneContext, ContainingScene};

// the partition components live beside the request slots they rank; the avatar systems file
// their outfits there without the scene runner
pub use ipfs::partition::{Partition, PartitionedDownloads};

/// `SceneHash` hook: the scene root appeared, so its hash ranks at its partition, or at the
/// bottom of the scene tier until it has one; the publisher raises the player's scene to the
/// top on its first frame with a download waiting. Without the ipfs resource (tests) nothing
/// is registered.
pub fn register_scene_hash(world: DeferredWorld, context: HookContext) {
    let (Some(ipfs), Some(hash)) = (
        world.get_resource::<IpfsResource>(),
        world.get::<SceneHash>(context.entity),
    ) else {
        return;
    };
    let key = world
        .get::<Partition>(context.entity)
        .map_or(LoadPriority::SCENE_PENDING, |partition| {
            partition.0.load_priority(Tier::Scene)
        });
    ipfs.register_download_priority(&hash.0, key);
}

/// `SceneHash` hook: the scene root is going (despawned, or the hash replaced), so its hash has
/// no owner; requests still queued under it rank last.
pub fn release_scene_hash(world: DeferredWorld, context: HookContext) {
    let (Some(ipfs), Some(hash)) = (
        world.get_resource::<IpfsResource>(),
        world.get::<SceneHash>(context.entity),
    ) else {
        return;
    };
    ipfs.release_download_priority(&hash.0);
}

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

/// This frame's download priority of every registered hash, written into the ipfs request
/// slots in place: the player's scenes first, even before they have a context and while the
/// player is still out of world arriving in one; then the
/// avatars' bodies and wearables, nearest avatar first, a shared wearable at the best of its
/// avatars; then the other scenes by partition, a scene with no partition yet last among them.
/// Which hashes are registered is the hooks' business (`register_scene_hash`,
/// `register_partitioned_downloads`). Runs only on a frame in which a download waits for or
/// holds a slot: a grant that chooses between downloads happens at an enqueue while others wait
/// or at a release, so it reads this frame's keys unless the gate was idle when the frame began,
/// and the steady state costs nothing.
pub fn publish_download_priorities(
    scenes: Query<(Entity, &SceneHash, Option<&Partition>)>,
    partitioned: Query<(&Partition, &PartitionedDownloads)>,
    player: Query<Entity, With<PrimaryUser>>,
    containing_scene: ContainingScene,
    ipfs: Res<IpfsResource>,
) {
    if !ipfs.reads_download_priorities() {
        return;
    }
    let active = player
        .single()
        .map(|player| containing_scene.get_oow(player))
        .unwrap_or_default();
    let scene_keys = scenes.iter().map(|(entity, hash, partition)| {
        let key = if active.contains(&entity) {
            LoadPriority::TOP
        } else {
            partition.map_or(LoadPriority::SCENE_PENDING, |partition| {
                partition.0.load_priority(Tier::Scene)
            })
        };
        (hash.0.as_str(), key)
    });
    let wearable_keys = partitioned.iter().flat_map(|(partition, downloads)| {
        let key = partition.0.load_priority(Tier::Avatar);
        downloads.0.iter().map(move |hash| (hash.as_str(), key))
    });
    ipfs.update_download_priorities(scene_keys.chain(wearable_keys));
}

#[cfg(test)]
mod tests;
