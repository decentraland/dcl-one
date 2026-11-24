use bevy::{
    ecs::{component::HookContext, world::DeferredWorld},
    prelude::*,
};

use common::{
    partition::{point_partition, scene_partition, LoadPriority, PartitionData, Tier},
    structs::{PrimaryCamera, PrimaryUser},
};
use ipfs::IpfsResource;

use crate::{
    initialize_scene::SceneHash, renderer_context::RendererSceneContext, scene_lod::SceneLod,
    ContainingScene,
};

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

/// The most ticks a second of a scene whose nearest parcel is under each distance (m) from the
/// player, nearest band first. A scene beyond the last band ticks at `FAR_SCENE_TICK_RATE`.
pub(crate) const SCENE_TICK_RATE_BANDS: [(f32, f32); 5] = [
    (16.0, 40.0),
    (32.0, 30.0),
    (48.0, 15.0),
    (64.0, 10.0),
    (80.0, 5.0),
];

/// The ticks a second of a scene beyond the last band, so it keeps reading the channels it
/// reads only while ticking. A global CRDT backlog makes it due sooner
/// (`GLOBAL_CRDT_BACKLOG_DUE`).
pub(crate) const FAR_SCENE_TICK_RATE: f32 = 0.5;

/// The most ticks a second of a scene behind the camera.
pub(crate) const BEHIND_SCENE_TICK_RATE: f32 = 1.0;

/// The most ticks a second of a scene at `partition`: its distance band's rate, or
/// `FAR_SCENE_TICK_RATE` beyond the last band, at most `BEHIND_SCENE_TICK_RATE` behind the
/// camera.
pub(crate) fn scene_tick_rate(partition: &PartitionData) -> f32 {
    let rate = SCENE_TICK_RATE_BANDS
        .iter()
        .find(|(distance, _)| partition.sqr_distance < distance * distance)
        .map_or(FAR_SCENE_TICK_RATE, |(_, rate)| *rate);
    if partition.is_behind {
        rate.min(BEHIND_SCENE_TICK_RATE)
    } else {
        rate
    }
}

/// Whether a scene ticking at most `rate` times a second, last sent `age` seconds ago, may tick
/// in a frame of `frame` seconds. `age` grows a frame at a time, so the interval counts as
/// passed within a quarter of a frame of it: frame jitter then cannot miss an interval of whole
/// frames, and an interval of one and a half frames always waits for the second.
pub(crate) fn tick_rate_allows(rate: f32, age: f64, frame: f64) -> bool {
    (age + frame * 0.25) * rate as f64 >= 1.0
}

/// A global CRDT backlog, in updates the slowest scene has not read, from which every capped
/// scene is due: half its receiver's capacity, so it reads them before any is dropped.
pub(crate) const GLOBAL_CRDT_BACKLOG_DUE: usize =
    comms::global_crdt::GLOBAL_CRDT_SCENE_CAPACITY / 2;

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
pub fn publish_download_priorities(
    scenes: Query<(Entity, &SceneHash, Option<&Partition>)>,
    partitioned: Query<(Option<&Partition>, &PartitionedDownloads, Has<PrimaryUser>)>,
    partitioned_emotes: Query<(
        Option<&Partition>,
        &PartitionedEmoteDownloads,
        Has<PrimaryUser>,
    )>,
    lods: Query<(&Partition, &SceneLod)>,
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
