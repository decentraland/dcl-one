use bevy::{
    ecs::{component::HookContext, world::DeferredWorld},
    prelude::*,
};

use common::{
    partition::{point_partition, scene_partition, LoadPriority, PartitionData, Tier},
    structs::{CameraOverride, PrimaryCamera, PrimaryUser},
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
/// player, nearest band first, rounded to whole frames (`tick_rate_allows`). A scene beyond the
/// last band ticks at `FAR_SCENE_TICK_RATE`.
pub(crate) const SCENE_TICK_RATE_BANDS: [(f32, f32); 5] = [
    (16.0, 40.0),
    (32.0, 30.0),
    (48.0, 15.0),
    (64.0, 10.0),
    (80.0, 5.0),
];

/// The most ticks a second of a scene beyond the last band.
pub(crate) const FAR_SCENE_TICK_RATE: f32 = 0.5;

/// The most ticks a second of a scene behind the camera.
pub(crate) const BEHIND_SCENE_TICK_RATE: f32 = 1.0;

/// Every rate `scene_tick_rate` gives: the bands nearest first, then behind the camera, then
/// beyond the last band.
pub(crate) const SCENE_TICK_RATE_CAPS: [f32; SCENE_TICK_RATE_BANDS.len() + 2] = {
    let mut caps = [0.0; SCENE_TICK_RATE_BANDS.len() + 2];
    let mut band = 0;
    while band < SCENE_TICK_RATE_BANDS.len() {
        caps[band] = SCENE_TICK_RATE_BANDS[band].1;
        band += 1;
    }
    caps[band] = BEHIND_SCENE_TICK_RATE;
    caps[band + 1] = FAR_SCENE_TICK_RATE;
    caps
};

/// The steepest camera pitch, as the height of the camera's unit forward direction, at which the
/// behind cap applies.
pub(crate) const BEHIND_SCENE_MAX_FORWARD_Y: f32 = 0.5;

/// Whether a camera facing `forward` (unit length) applies the behind cap.
pub(crate) fn behind_cap_applies(forward: Vec3) -> bool {
    forward.y.abs() < BEHIND_SCENE_MAX_FORWARD_Y
}

/// Why every scene ticks at the full rate this frame, or None while the distance bands cap them.
/// `camera` is the rendering camera and whether it frames a photo; `caps_off` is the
/// `/scene_tick_rate_caps` switch. The photo camera and a scene's cinematic camera lift every
/// cap.
pub(crate) fn tick_rate_caps_lifted(
    camera: Option<(&PrimaryCamera, bool)>,
    caps_off: bool,
) -> Option<&'static str> {
    match camera {
        _ if caps_off => Some("Turned off"),
        None => Some("No rendering camera"),
        Some((_, true)) => Some("Photo mode"),
        Some((primary, false))
            if matches!(primary.scene_override, Some(CameraOverride::Cinematic(_))) =>
        {
            Some("Cinematic camera")
        }
        Some(_) => None,
    }
}

/// What the rendering camera, `camera` (with whether it frames a photo), allows this frame: why no
/// scene is capped, or None while the bands cap them (`tick_rate_caps_lifted`), and whether the
/// behind cap applies at its pitch (`behind_cap_applies`).
pub(crate) fn camera_tick_rate_caps(
    camera: Option<(&GlobalTransform, &PrimaryCamera, bool)>,
    caps_off: bool,
) -> (Option<&'static str>, bool) {
    let lifted = tick_rate_caps_lifted(
        camera.map(|(_, primary, photo_mode)| (primary, photo_mode)),
        caps_off,
    );
    let behind_cap =
        camera.is_some_and(|(camera, ..)| behind_cap_applies(camera.forward().as_vec3()));
    (lifted, behind_cap)
}

/// The most ticks a second of a scene at `partition`: its distance band's rate, or
/// `FAR_SCENE_TICK_RATE` beyond the last band, at most `BEHIND_SCENE_TICK_RATE` behind the
/// camera from the second band on.
pub(crate) fn scene_tick_rate(partition: &PartitionData) -> f32 {
    let band = SCENE_TICK_RATE_BANDS
        .iter()
        .position(|(distance, _)| partition.sqr_distance < distance * distance);
    let rate = band.map_or(FAR_SCENE_TICK_RATE, |band| SCENE_TICK_RATE_BANDS[band].1);
    if partition.is_behind && band != Some(0) {
        rate.min(BEHIND_SCENE_TICK_RATE)
    } else {
        rate
    }
}

/// The age a scene due this frame is ranked by. A scene its band let through ranks by the time
/// since its interval passed, at least a frame, so holding it back does not rank it as starved; a
/// scene at the full rate, or released on unread global updates, by the time since it was last
/// sent.
pub(crate) fn queue_age(cap: Option<f32>, released: bool, age: f64, frame: f64) -> f64 {
    match cap {
        Some(rate) if !released => (age - 1.0 / f64::from(rate)).max(frame),
        _ => age,
    }
}

/// Whether a scene ticking at most `rate` times a second, last sent `age` seconds ago, may tick
/// in a frame of `frame` seconds. `age` grows a frame at a time, so the interval counts as
/// passed within a quarter of a frame of it: frame jitter then cannot miss an interval of whole
/// frames, and an interval of one and a half frames always waits for the second.
pub(crate) fn tick_rate_allows(rate: f32, age: f64, frame: f64) -> bool {
    (age + frame * 0.25) * rate as f64 >= 1.0
}

/// The global updates delivered to a capped scene's host since its last dispatch from which it
/// is due: a quarter of `SCENE_GLOBAL_UPDATE_CAPACITY`.
pub(crate) const GLOBAL_UPDATE_BACKLOG_DUE: u64 =
    common::structs::SCENE_GLOBAL_UPDATE_CAPACITY as u64 / 4;

/// The peer messages handed to a capped scene's inbound queues (string bus, binary bus, topic
/// inbox) since its last dispatch from which it is due: a quarter of the smallest queue, the
/// topic inbox (645 messages; each bus holds 1,024).
pub(crate) const SCENE_MESSAGE_BACKLOG_DUE: u64 = 160;

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
