use std::sync::Arc;

use bevy::{
    asset::io::memory::{Dir, MemoryAssetReader},
    ecs::system::RunSystemOnce,
    math::FloatOrd,
    prelude::*,
};
use common::{
    partition::{LoadPriority, PartitionData, Tier},
    structs::{PrimaryCamera, PrimaryUser},
};
use dcl_types::SceneId;
use ipfs::{IpfsIo, IpfsResource, Ticket};
use scene_material::SceneBounds;

use super::*;
use crate::{
    initialize_scene::{LiveScenes, PointerResult, PortableScenes, SceneHash, ScenePointers},
    renderer_context::RendererSceneContext,
};

const PLAYER_POS: Vec3 = Vec3::new(8.0, 0.0, -8.0);

fn world_with_player(camera_forward: Option<Vec3>) -> World {
    let mut world = World::new();
    world.spawn((
        PrimaryUser::default(),
        GlobalTransform::from_translation(PLAYER_POS),
    ));
    if let Some(forward) = camera_forward {
        spawn_rendering_camera(&mut world, PLAYER_POS, forward);
    }
    world
}

fn spawn_rendering_camera(world: &mut World, position: Vec3, forward: Vec3) {
    world.spawn((
        PrimaryCamera::default(),
        Camera::default(),
        GlobalTransform::from(Transform::from_translation(position).looking_to(forward, Vec3::Y)),
    ));
}

fn insert_ipfs(world: &mut World, slots: usize) {
    world.insert_resource(IpfsResource {
        inner: Arc::new(IpfsIo::new(
            false,
            Box::new(MemoryAssetReader {
                root: Dir::default(),
            }),
            None,
            Default::default(),
            slots,
            tokio::sync::mpsc::unbounded_channel().0,
        )),
    });
}

/// Fill the request slots and queue one more request, so a publish is not skipped. The
/// tickets must outlive the publish.
fn hold_a_waiting_download(ipfs: &IpfsIo, slots: usize) -> Vec<Arc<Ticket>> {
    let gate = ipfs.request_slots();
    (0..=slots)
        .map(|i| gate.ticket(Some(format!("held-{i}"))))
        .collect()
}

/// The player's parcel (0,0) points at `hash`, which is live as `scene`.
fn player_stands_on(world: &mut World, hash: &str, scene: Entity) {
    world.resource_mut::<ScenePointers>().insert(
        IVec2::ZERO,
        PointerResult::Exists {
            hash: hash.to_owned(),
            urn: None,
        },
    );
    world
        .resource_mut::<LiveScenes>()
        .scenes
        .insert(hash.to_owned(), scene);
}

fn scene_with_parcels(world: &mut World, parcels: &[IVec2]) -> Entity {
    let root = world.spawn_empty().id();
    let context = RendererSceneContext::new(
        SceneId(root),
        "hash".to_owned(),
        String::new(),
        false,
        0,
        String::new(),
        IVec2::ZERO,
        parcels.iter().copied().collect(),
        Vec::new(),
        SceneBounds::default(),
        Vec::new(),
        root,
        0.0,
        false,
        "",
        false,
        false,
    );
    world.entity_mut(root).insert(context);
    root
}

#[test]
fn partitions_a_live_scene_to_its_nearest_parcel() {
    let mut world = world_with_player(Some(Vec3::X));
    let scene = scene_with_parcels(&mut world, &[IVec2::new(2, 0), IVec2::new(3, 0)]);

    world.run_system_once(update_scene_partitions).unwrap();

    let partition = world.get::<Partition>(scene).unwrap().0;
    assert_eq!(partition.sqr_distance, 576.0);
    assert_eq!(partition.parcel_distance, 2);
    assert!(!partition.is_behind);
}

#[test]
fn distance_from_the_player_facing_from_the_camera() {
    // third person near a boundary: the camera sits two parcels east of the player, looking away
    let mut world = world_with_player(None);
    spawn_rendering_camera(&mut world, Vec3::new(40.0, 0.0, -8.0), Vec3::X);
    let scene = scene_with_parcels(&mut world, &[IVec2::ZERO]);

    world.run_system_once(update_scene_partitions).unwrap();

    let partition = world.get::<Partition>(scene).unwrap().0;
    assert_eq!(
        partition.sqr_distance, 0.0,
        "measured from the player, who stands on it"
    );
    assert!(partition.is_behind, "every corner is behind the camera");
}

#[test]
fn uses_the_players_position_when_there_is_no_camera() {
    let mut world = world_with_player(None);
    // the player looks along -z (parcel +y), so this scene is behind them; with no
    // rendering camera no facing is known and nothing is behind
    let scene = scene_with_parcels(&mut world, &[IVec2::new(0, -3)]);

    world.run_system_once(update_scene_partitions).unwrap();

    let partition = world.get::<Partition>(scene).unwrap().0;
    assert!(!partition.is_behind);
    assert_eq!(partition.parcel_distance, 3);
}

#[test]
fn a_camera_that_does_not_render_gives_no_facing() {
    // the headless server's stand-in camera: a PrimaryCamera with no Camera, never turned
    let mut world = world_with_player(None);
    world.spawn((
        PrimaryCamera::default(),
        GlobalTransform::from(Transform::from_translation(PLAYER_POS).looking_to(Vec3::X, Vec3::Y)),
    ));
    let scene = scene_with_parcels(&mut world, &[IVec2::new(-3, 0)]);

    world.run_system_once(update_scene_partitions).unwrap();

    let partition = world.get::<Partition>(scene).unwrap().0;
    assert!(!partition.is_behind);
    assert_eq!(partition.parcel_distance, 3);
    assert_eq!(partition.sqr_distance, 1600.0);
}

#[test]
fn a_scene_without_parcels_is_nearest() {
    let mut world = world_with_player(Some(Vec3::X));
    let scene = scene_with_parcels(&mut world, &[]);

    world.run_system_once(update_scene_partitions).unwrap();

    assert_eq!(
        world.get::<Partition>(scene).unwrap().0,
        PartitionData::NEAREST
    );
}

#[test]
fn an_entity_with_partitioned_downloads_takes_its_positions_partition() {
    let mut world = world_with_player(Some(Vec3::X));
    let avatar = world
        .spawn((
            PartitionedDownloads(vec!["wearable".to_owned()]),
            GlobalTransform::from_translation(Vec3::new(40.0, 0.0, -8.0)),
        ))
        .id();

    world.run_system_once(update_scene_partitions).unwrap();

    let partition = world.get::<Partition>(avatar).unwrap().0;
    assert_eq!(partition.sqr_distance, 1024.0);
    assert_eq!(partition.parcel_distance, 2);
    assert!(!partition.is_behind);
}

#[test]
fn publishes_a_wearable_by_the_best_avatar_wearing_it() {
    let mut world = world_with_player(Some(Vec3::X));
    insert_ipfs(&mut world, 1);
    world.init_resource::<ScenePointers>();
    world.init_resource::<PortableScenes>();
    world.init_resource::<LiveScenes>();
    let near = PartitionData {
        sqr_distance: 100.0,
        parcel_distance: 1,
        is_behind: false,
    };
    let far = PartitionData {
        sqr_distance: 6400.0,
        parcel_distance: 5,
        is_behind: true,
    };
    world.spawn((
        PartitionedDownloads(vec!["shared".to_owned(), "far-only".to_owned()]),
        Partition(far),
    ));
    let near_avatar = world
        .spawn((
            PartitionedDownloads(vec!["shared".to_owned()]),
            Partition(near),
        ))
        .id();
    let _waiting = hold_a_waiting_download(world.resource::<IpfsResource>(), 1);

    world.run_system_once(publish_download_priorities).unwrap();

    let ipfs = world.resource::<IpfsResource>();
    assert_eq!(
        ipfs.scene_priority("shared"),
        Some(near.load_priority(Tier::Avatar))
    );
    assert_eq!(
        ipfs.scene_priority("far-only"),
        Some(far.load_priority(Tier::Avatar))
    );

    // the near avatar walks off beyond the far one: the shared wearable follows the best left
    let farther = PartitionData {
        sqr_distance: 20000.0,
        parcel_distance: 9,
        is_behind: false,
    };
    world.get_mut::<Partition>(near_avatar).unwrap().0 = farther;
    world.run_system_once(publish_download_priorities).unwrap();
    assert_eq!(
        world.resource::<IpfsResource>().scene_priority("shared"),
        Some(far.load_priority(Tier::Avatar))
    );
}

#[test]
fn publishes_each_scenes_key_with_the_players_scene_on_top() {
    let mut world = world_with_player(Some(Vec3::X));
    insert_ipfs(&mut world, 1);
    // the player stands on "home"; "far" is behind the camera, five parcels away
    let mut pointers = ScenePointers::default();
    pointers.insert(
        IVec2::ZERO,
        PointerResult::Exists {
            hash: "home".to_owned(),
            urn: None,
        },
    );
    world.insert_resource(pointers);
    world.init_resource::<PortableScenes>();
    world.init_resource::<LiveScenes>();
    let home = world
        .spawn((
            SceneHash("home".to_owned()),
            Partition(PartitionData::NEAREST),
        ))
        .id();
    world.spawn((
        SceneHash("far".to_owned()),
        Partition(PartitionData {
            sqr_distance: 6400.0,
            parcel_distance: 5,
            is_behind: true,
        }),
    ));
    world
        .resource_mut::<LiveScenes>()
        .scenes
        .insert("home".to_owned(), home);
    let _waiting = hold_a_waiting_download(world.resource::<IpfsResource>(), 1);

    world.run_system_once(publish_download_priorities).unwrap();

    let ipfs = world.resource::<IpfsResource>();
    assert_eq!(ipfs.scene_priority("home"), Some(LoadPriority::TOP));
    assert_eq!(
        ipfs.scene_priority("far"),
        Some(LoadPriority {
            tier: Tier::Scene,
            parcel_distance: 5,
            is_behind: true,
            sqr_distance: FloatOrd(6400.0),
        })
    );
    assert_eq!(ipfs.scene_priority("other"), None);
}

#[test]
fn publishes_nothing_while_no_download_waits_for_a_slot() {
    let mut world = world_with_player(Some(Vec3::X));
    insert_ipfs(&mut world, 1);
    world.init_resource::<ScenePointers>();
    world.init_resource::<PortableScenes>();
    world.init_resource::<LiveScenes>();
    let home = world
        .spawn((
            SceneHash("home".to_owned()),
            Partition(PartitionData::NEAREST),
        ))
        .id();
    player_stands_on(&mut world, "home", home);

    world.run_system_once(publish_download_priorities).unwrap();
    assert_eq!(
        world.resource::<IpfsResource>().scene_priority("home"),
        Some(PartitionData::NEAREST.load_priority(Tier::Scene)),
        "nothing waits for a slot, so the key stays as registered"
    );

    let _waiting = hold_a_waiting_download(world.resource::<IpfsResource>(), 1);
    world.run_system_once(publish_download_priorities).unwrap();
    assert_eq!(
        world.resource::<IpfsResource>().scene_priority("home"),
        Some(LoadPriority::TOP)
    );
}

#[test]
fn a_scene_registers_its_hash_at_spawn_and_releases_it_at_despawn() {
    let mut world = world_with_player(None);
    insert_ipfs(&mut world, 1);

    let scene = world.spawn(SceneHash("s".to_owned())).id();
    assert_eq!(
        world.resource::<IpfsResource>().scene_priority("s"),
        Some(LoadPriority::SCENE_PENDING),
        "no partition yet: the bottom of the scene tier"
    );

    world.despawn(scene);
    assert_eq!(world.resource::<IpfsResource>().scene_priority("s"), None);
}

#[test]
fn the_players_scene_is_active_before_it_has_a_context() {
    let mut world = world_with_player(Some(Vec3::X));
    insert_ipfs(&mut world, 1);
    world.init_resource::<ScenePointers>();
    world.init_resource::<PortableScenes>();
    world.init_resource::<LiveScenes>();
    // spawned this frame: no context, no partition
    let home = world.spawn(SceneHash("home".to_owned())).id();
    player_stands_on(&mut world, "home", home);
    let _waiting = hold_a_waiting_download(world.resource::<IpfsResource>(), 1);

    world.run_system_once(publish_download_priorities).unwrap();

    assert_eq!(
        world.resource::<IpfsResource>().scene_priority("home"),
        Some(LoadPriority::TOP)
    );
}

#[test]
fn a_scene_without_a_partition_stays_at_the_bottom_of_the_scene_tier() {
    let mut world = world_with_player(Some(Vec3::X));
    insert_ipfs(&mut world, 1);
    world.init_resource::<ScenePointers>();
    world.init_resource::<PortableScenes>();
    world.init_resource::<LiveScenes>();
    world.spawn(SceneHash("loading".to_owned()));
    let _waiting = hold_a_waiting_download(world.resource::<IpfsResource>(), 1);

    world.run_system_once(publish_download_priorities).unwrap();

    assert_eq!(
        world.resource::<IpfsResource>().scene_priority("loading"),
        Some(LoadPriority::SCENE_PENDING)
    );
}

#[test]
fn an_avatars_wearables_register_at_its_partition_and_release_at_despawn() {
    let mut world = world_with_player(None);
    insert_ipfs(&mut world, 1);
    let near = PartitionData {
        sqr_distance: 100.0,
        parcel_distance: 1,
        is_behind: false,
    };

    let avatar = world
        .spawn((
            PartitionedDownloads(vec!["body".to_owned(), "hat".to_owned()]),
            Partition(near),
        ))
        .id();
    let ipfs = world.resource::<IpfsResource>();
    assert_eq!(
        ipfs.scene_priority("body"),
        Some(near.load_priority(Tier::Avatar))
    );
    assert_eq!(
        ipfs.scene_priority("hat"),
        Some(near.load_priority(Tier::Avatar))
    );

    world.despawn(avatar);
    let ipfs = world.resource::<IpfsResource>();
    assert_eq!(ipfs.scene_priority("body"), None);
    assert_eq!(ipfs.scene_priority("hat"), None);
}

#[test]
fn an_avatar_without_a_partition_registers_at_the_bottom_of_the_avatar_tier() {
    let mut world = world_with_player(None);
    insert_ipfs(&mut world, 1);

    world.spawn(PartitionedDownloads(vec!["body".to_owned()]));

    assert_eq!(
        world.resource::<IpfsResource>().scene_priority("body"),
        Some(LoadPriority::AVATAR_PENDING)
    );
}

#[test]
fn a_shared_wearable_survives_one_avatar_leaving() {
    let mut world = world_with_player(None);
    insert_ipfs(&mut world, 1);
    let near = PartitionData {
        sqr_distance: 100.0,
        parcel_distance: 1,
        is_behind: false,
    };
    let far = PartitionData {
        sqr_distance: 6400.0,
        parcel_distance: 5,
        is_behind: true,
    };
    let near_avatar = world
        .spawn((
            PartitionedDownloads(vec!["shared".to_owned()]),
            Partition(near),
        ))
        .id();
    world.spawn((
        PartitionedDownloads(vec!["shared".to_owned()]),
        Partition(far),
    ));

    world.despawn(near_avatar);

    assert_eq!(
        world.resource::<IpfsResource>().scene_priority("shared"),
        Some(near.load_priority(Tier::Avatar)),
        "still registered, at the key it had; the next publish lowers it to the far avatar's"
    );
}

#[test]
fn an_outfit_change_swaps_the_registered_hashes() {
    let mut world = world_with_player(None);
    insert_ipfs(&mut world, 1);
    let near = PartitionData {
        sqr_distance: 100.0,
        parcel_distance: 1,
        is_behind: false,
    };
    let avatar = world
        .spawn((
            PartitionedDownloads(vec!["old".to_owned(), "kept".to_owned()]),
            Partition(near),
        ))
        .id();

    world.entity_mut(avatar).insert(PartitionedDownloads(vec![
        "new".to_owned(),
        "kept".to_owned(),
    ]));

    let ipfs = world.resource::<IpfsResource>();
    assert_eq!(ipfs.scene_priority("old"), None);
    assert_eq!(
        ipfs.scene_priority("new"),
        Some(near.load_priority(Tier::Avatar))
    );
    assert_eq!(
        ipfs.scene_priority("kept"),
        Some(near.load_priority(Tier::Avatar))
    );
}

/// The publisher's per-frame work at a busy plaza: 20 scenes in range, 50 avatars with 10
/// wearables each (353 distinct, neighbours sharing a few), with a download waiting and with
/// none. Run with `--ignored --nocapture`.
#[test]
#[ignore]
fn overhead_of_publishing_at_a_busy_plaza() {
    let mut world = world_with_player(Some(Vec3::X));
    insert_ipfs(&mut world, 32);
    world.init_resource::<ScenePointers>();
    world.init_resource::<PortableScenes>();
    world.init_resource::<LiveScenes>();
    // live scenes with two parcels each, so the run partitions them from their parcels as the
    // client does
    for i in 0..20 {
        let scene = scene_with_parcels(&mut world, &[IVec2::new(i, 0), IVec2::new(i, 1)]);
        world
            .entity_mut(scene)
            .insert(SceneHash(format!("scene-{i}")));
    }
    // avatars spread over the plaza, partitioned from their positions
    for a in 0..50 {
        world.spawn((
            PartitionedDownloads((0..10).map(|w| format!("wearable-{}", a * 7 + w)).collect()),
            GlobalTransform::from_translation(Vec3::new((a * 3) as f32, 0.0, -(a as f32))),
        ));
    }
    // the systems the publisher needs, as one schedule, so the run has no one-off cost
    let mut schedule = Schedule::default();
    schedule.add_systems((update_scene_partitions, publish_download_priorities).chain());
    schedule.run(&mut world);

    let waiting = hold_a_waiting_download(world.resource::<IpfsResource>(), 32);
    let start = std::time::Instant::now();
    for _ in 0..200 {
        schedule.run(&mut world);
    }
    let busy = start.elapsed() / 200;

    drop(waiting);
    let start = std::time::Instant::now();
    for _ in 0..200 {
        schedule.run(&mut world);
    }
    let idle = start.elapsed() / 200;
    println!(
        "publisher: 20 live scenes + 50 avatars x 10 wearables: {busy:?} per frame with a download waiting (partitions, keys written in place); {idle:?} with none (partitions only)"
    );
}
