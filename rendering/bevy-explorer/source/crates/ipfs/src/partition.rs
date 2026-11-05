//! Where an entity's downloads rank: its partition relative to the player and the camera, and
//! the content hashes filed under it. Scenes and avatars both carry these; the scene runner
//! refreshes the partitions every frame and the avatar systems file their outfits here.

use bevy::{
    ecs::{component::HookContext, world::DeferredWorld},
    prelude::*,
};
use common::partition::{LoadPriority, PartitionData, Tier};

use crate::IpfsResource;

/// Where a scene, or an entity with `PartitionedDownloads`, is relative to the player and the
/// camera; refreshed every frame by the scene runner's `update_scene_partitions`.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Partition(pub PartitionData);

/// The content hashes whose downloads take this entity's partition: an avatar's body and
/// wearables. Scenes need none; their downloads carry the scene's own hash. Inserting it
/// registers the hashes with the ipfs request slots and replacing or removing it (an outfit
/// change, a despawn) releases them, so the slots know exactly which hashes have an owner.
#[derive(Component, Clone, Debug, Default, PartialEq)]
#[component(
    on_insert = register_partitioned_downloads,
    on_replace = release_partitioned_downloads
)]
pub struct PartitionedDownloads(pub Vec<String>);

fn register_partitioned_downloads(world: DeferredWorld, context: HookContext) {
    let (Some(ipfs), Some(downloads)) = (
        world.get_resource::<IpfsResource>(),
        world.get::<PartitionedDownloads>(context.entity),
    ) else {
        return;
    };
    let key = world
        .get::<Partition>(context.entity)
        .map_or(LoadPriority::AVATAR_PENDING, |partition| {
            partition.0.load_priority(Tier::Avatar)
        });
    for hash in &downloads.0 {
        ipfs.register_download_priority(hash, key);
    }
}

fn release_partitioned_downloads(world: DeferredWorld, context: HookContext) {
    let (Some(ipfs), Some(downloads)) = (
        world.get_resource::<IpfsResource>(),
        world.get::<PartitionedDownloads>(context.entity),
    ) else {
        return;
    };
    for hash in &downloads.0 {
        ipfs.release_download_priority(hash);
    }
}
