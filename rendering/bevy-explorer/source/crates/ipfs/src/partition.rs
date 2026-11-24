//! Download ownership shared by the avatar renderer and scene runner.
use crate::IpfsResource;
use bevy::{
    ecs::{component::HookContext, world::DeferredWorld},
    prelude::*,
};
use common::{
    partition::{LoadPriority, PartitionData, Tier},
    structs::PrimaryUser,
};
/// Where a scene, or an entity with `PartitionedDownloads` or `PartitionedEmoteDownloads`, is
/// relative to the player and the camera; refreshed every frame by `update_scene_partitions`.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Partition(pub PartitionData);

/// The content hashes whose downloads take this entity's partition: an avatar's body and
/// wearables. Scenes need none; their downloads carry the scene's own hash. Inserting it
/// registers the hashes with the ipfs request slots and replacing or removing it (an outfit
/// change, a despawn) releases them, so the slots know exactly which hashes have an owner.
/// On the player (`PrimaryUser`) or a [`PlayerDownloads`] preview the hashes, and those of
/// `PartitionedEmoteDownloads`, rank first instead, ahead of the scenes the player stands in.
#[derive(Component, Clone, Debug, Default, PartialEq)]
#[component(
    on_insert = register_partitioned_downloads,
    on_replace = release_partitioned_downloads
)]
pub struct PartitionedDownloads(pub Vec<String>);

/// The content hashes of the emotes this entity plays and, on the player, the emotes they have
/// equipped, filed like [`PartitionedDownloads`]: an emote's definition, model and sounds
/// download under its own entity hash, which no scene root holds. It is a component of its own because the emotes change with playback and the outfit
/// with the profile, and each list is replaced whole.
#[derive(Component, Clone, Debug, Default, PartialEq)]
#[component(
    on_insert = register_partitioned_emote_downloads,
    on_replace = release_partitioned_emote_downloads
)]
pub struct PartitionedEmoteDownloads(pub Vec<String>);

/// Marks an avatar that is not the player whose downloads rank with the player's avatar.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct PlayerDownloads;

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
        world.get::<PrimaryUser>(entity).is_some()
            || world.get::<PlayerDownloads>(entity).is_some(),
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
pub fn avatar_key(is_player: bool, partition: Option<&Partition>) -> Option<LoadPriority> {
    if is_player {
        Some(LoadPriority::OWN_AVATAR)
    } else {
        partition.map(|partition| partition.0.load_priority(Tier::Avatar))
    }
}

/// `key` for each of `hashes`, or nothing without a key.
pub fn keyed(
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
