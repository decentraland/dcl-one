use bevy::{
    ecs::{component::HookContext, world::DeferredWorld},
    platform::collections::HashSet,
    prelude::*,
};

use common::{
    partition::{point_partition, scene_partition, LoadPriority, Tier},
    structs::{PrimaryCamera, PrimaryUser},
};
use ipfs::IpfsResource;

use crate::{
    initialize_scene::SceneHash, renderer_context::RendererSceneContext, scene_lod::SceneLod,
    ContainingScene,
};

use ipfs::partition::PlayerDownloads;
use ipfs::partition::{avatar_key, keyed};
pub use ipfs::partition::{Partition, PartitionedDownloads, PartitionedEmoteDownloads};

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
    Some((player.translation(), camera_facing(camera)))
}

/// The rendering camera's position and forward direction, `None` without one (the headless
/// server): what "in front of the camera" is measured against.
pub(crate) fn camera_facing(
    camera: &Query<&GlobalTransform, (With<PrimaryCamera>, With<Camera>)>,
) -> Option<(Vec3, Vec3)> {
    camera
        .single()
        .ok()
        .map(|camera| (camera.translation(), camera.forward().as_vec3()))
}

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
    mut lods: Query<
        (Entity, &SceneLod, Option<&mut Partition>),
        (
            Without<RendererSceneContext>,
            Without<PartitionedDownloads>,
            Without<PartitionedEmoteDownloads>,
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
    for (entity, lod, current) in lods.iter_mut() {
        let partition = scene_partition(origin, camera, lod.parcels.iter().copied());
        apply(entity, Partition(partition), current);
    }
}

/// This frame's download priority of every registered hash, written into the ipfs request
/// slots in place: the player's own body, wearables and emotes first; then the player's scenes,
/// even before they have a context and while the player is still out of world arriving in one;
/// then the other avatars' bodies, wearables and emotes, nearest avatar first, a shared one at
/// the best of its avatars (the player's own, when the player wears or plays it too); then the
/// other scenes by partition, a scene with no partition yet last among them; then the scenes'
/// stand-ins, whose files a spawned scene's own key outranks.
/// Which hashes are registered is the hooks' business (`register_scene_hash`,
/// `register_partitioned_downloads`, `register_partitioned_emote_downloads`, `scene_lod`'s). Runs
/// only on a frame in which a download waits for or holds a slot: a grant that chooses between
/// downloads happens at an enqueue while others wait or at a release, so it reads this frame's
/// keys unless the gate was idle when the frame began, and the steady state costs nothing.
/// While engine memory is full, held requests keep the gate busy; then it publishes only when the
/// player's scenes change, the one change that moves a hash across the hold (the player's own
/// avatar and previews rank from their registration). Other keys only order held requests.
pub fn publish_download_priorities(
    scenes: Query<(Entity, &SceneHash, Option<&Partition>)>,
    partitioned: Query<(
        Option<&Partition>,
        &PartitionedDownloads,
        Has<PrimaryUser>,
        Has<PlayerDownloads>,
    )>,
    partitioned_emotes: Query<(
        Option<&Partition>,
        &PartitionedEmoteDownloads,
        Has<PrimaryUser>,
        Has<PlayerDownloads>,
    )>,
    lods: Query<(&Partition, &SceneLod)>,
    player: Query<Entity, With<PrimaryUser>>,
    containing_scene: ContainingScene,
    ipfs: Res<IpfsResource>,
    mut last_active: Local<HashSet<Entity>>,
) {
    if !ipfs.reads_download_priorities() {
        return;
    }
    let active = player
        .single()
        .map(|player| containing_scene.get_oow(player))
        .unwrap_or_default();
    if active == *last_active && ipfs.downloads_held_from().is_some() {
        return;
    }
    last_active.clone_from(&active);
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
    let wearable_keys =
        partitioned
            .iter()
            .flat_map(|(partition, downloads, is_player, player_downloads)| {
                keyed(
                    avatar_key(is_player || player_downloads, partition),
                    &downloads.0,
                )
            });
    let emote_keys = partitioned_emotes.iter().flat_map(
        |(partition, downloads, is_player, player_downloads)| {
            keyed(
                avatar_key(is_player || player_downloads, partition),
                &downloads.0,
            )
        },
    );
    let lod_keys = lods
        .iter()
        .map(|(partition, lod)| (lod.hash.as_str(), partition.0.load_priority(Tier::Lod)));
    ipfs.update_download_priorities(
        scene_keys
            .chain(wearable_keys)
            .chain(emote_keys)
            .chain(lod_keys),
    );
}

#[cfg(test)]
mod tests;
