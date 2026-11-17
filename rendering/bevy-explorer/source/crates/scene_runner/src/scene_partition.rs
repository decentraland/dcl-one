use bevy::{
    ecs::{component::HookContext, world::DeferredWorld},
    prelude::*,
};

use common::{
    partition::{point_partition, scene_partition, LoadPriority, PartitionData, Tier},
    structs::{PrimaryCamera, PrimaryUser},
};
use ipfs::IpfsResource;

use crate::{initialize_scene::SceneHash, renderer_context::RendererSceneContext, ContainingScene};

/// Where a scene, or an entity with `PartitionedDownloads` or `PartitionedEmoteDownloads`, is
/// relative to the player and the camera; refreshed every frame by `update_scene_partitions`.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Partition(pub PartitionData);

/// The content hashes whose downloads take this entity's partition: an avatar's body and
/// wearables. Scenes need none; their downloads carry the scene's own hash. Inserting it
/// registers the hashes with the ipfs request slots and replacing or removing it (an outfit
/// change, a despawn) releases them, so the slots know exactly which hashes have an owner.
/// On the player (`PrimaryUser`) the hashes, and those of `PartitionedEmoteDownloads`, rank
/// first instead, ahead of the scenes the player stands in.
#[derive(Component, Clone, Debug, Default, PartialEq)]
#[component(
    on_insert = register_partitioned_downloads,
    on_replace = release_partitioned_downloads
)]
pub struct PartitionedDownloads(pub Vec<String>);

/// The content hashes of the emotes this entity plays, filed like [`PartitionedDownloads`]: an
/// emote's definition, model and sounds download under its own entity hash, which no scene root
/// holds. It is a component of its own because the emotes change with playback and the outfit
/// with the profile, and each list is replaced whole.
#[derive(Component, Clone, Debug, Default, PartialEq)]
#[component(
    on_insert = register_partitioned_emote_downloads,
    on_replace = release_partitioned_emote_downloads
)]
pub struct PartitionedEmoteDownloads(pub Vec<String>);

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

fn register_partitioned_downloads(world: DeferredWorld, context: HookContext) {
    if let Some(downloads) = world.get::<PartitionedDownloads>(context.entity) {
        register_avatar_hashes(&world, context.entity, &downloads.0);
    }
}

fn release_partitioned_downloads(world: DeferredWorld, context: HookContext) {
    if let Some(downloads) = world.get::<PartitionedDownloads>(context.entity) {
        release_avatar_hashes(&world, &downloads.0);
    }
}

fn register_partitioned_emote_downloads(world: DeferredWorld, context: HookContext) {
    if let Some(downloads) = world.get::<PartitionedEmoteDownloads>(context.entity) {
        register_avatar_hashes(&world, context.entity, &downloads.0);
    }
}

fn release_partitioned_emote_downloads(world: DeferredWorld, context: HookContext) {
    if let Some(downloads) = world.get::<PartitionedEmoteDownloads>(context.entity) {
        release_avatar_hashes(&world, &downloads.0);
    }
}

/// The hashes rank by `avatar_key` from the moment they are filed, an avatar that has no
/// partition yet at the bottom of the avatar tier. Without the ipfs resource (tests) nothing is
/// registered.
fn register_avatar_hashes(world: &DeferredWorld, entity: Entity, hashes: &[String]) {
    let Some(ipfs) = world.get_resource::<IpfsResource>() else {
        return;
    };
    let key = avatar_key(
        world.get::<PrimaryUser>(entity).is_some(),
        world.get::<Partition>(entity),
    )
    .unwrap_or(LoadPriority::AVATAR_PENDING);
    for hash in hashes {
        ipfs.register_download_priority(hash, key);
    }
}

/// The key of the hashes an avatar holds. The player's own rank first, ahead of the scenes the
/// player stands in and with or without a partition: one outfit and the emotes the player
/// plays are few and small, and the player is waiting on them. Another avatar's rank at its
/// partition in the avatar tier, and it has no key to publish until it has a partition.
fn avatar_key(is_player: bool, partition: Option<&Partition>) -> Option<LoadPriority> {
    if is_player {
        Some(LoadPriority::OWN_AVATAR)
    } else {
        partition.map(|partition| partition.0.load_priority(Tier::Avatar))
    }
}

/// `key` for each of `hashes`, or nothing without a key.
fn keyed(
    key: Option<LoadPriority>,
    hashes: &[String],
) -> impl Iterator<Item = (&str, LoadPriority)> {
    key.into_iter()
        .flat_map(move |key| hashes.iter().map(move |hash| (hash.as_str(), key)))
}

fn release_avatar_hashes(world: &DeferredWorld, hashes: &[String]) {
    let Some(ipfs) = world.get_resource::<IpfsResource>() else {
        return;
    };
    for hash in hashes {
        ipfs.release_download_priority(hash);
    }
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
        (
            Or<(With<PartitionedDownloads>, With<PartitionedEmoteDownloads>)>,
            Without<RendererSceneContext>,
        ),
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
/// slots in place: the player's own body, wearables and emotes first; then the player's scenes,
/// even before they have a context and while the player is still out of world arriving in one;
/// then the other avatars' bodies, wearables and emotes, nearest avatar first, a shared one at
/// the best of its avatars (the player's own, when the player wears or plays it too); then the
/// other scenes by partition, a scene with no partition yet last among them.
/// Which hashes are registered is the hooks' business (`register_scene_hash`,
/// `register_partitioned_downloads`, `register_partitioned_emote_downloads`). Runs only on a
/// frame in which a download waits for or holds a slot: a grant that chooses between downloads
/// happens at an enqueue while others wait or at a release, so it reads this frame's keys unless
/// the gate was idle when the frame began, and the steady state costs nothing.
pub fn publish_download_priorities(
    scenes: Query<(Entity, &SceneHash, Option<&Partition>)>,
    partitioned: Query<(Option<&Partition>, &PartitionedDownloads, Has<PrimaryUser>)>,
    partitioned_emotes: Query<(
        Option<&Partition>,
        &PartitionedEmoteDownloads,
        Has<PrimaryUser>,
    )>,
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
    let wearable_keys = partitioned
        .iter()
        .flat_map(|(partition, downloads, is_player)| {
            keyed(avatar_key(is_player, partition), &downloads.0)
        });
    let emote_keys = partitioned_emotes
        .iter()
        .flat_map(|(partition, downloads, is_player)| {
            keyed(avatar_key(is_player, partition), &downloads.0)
        });
    ipfs.update_download_priorities(scene_keys.chain(wearable_keys).chain(emote_keys));
}

#[cfg(test)]
mod tests;
