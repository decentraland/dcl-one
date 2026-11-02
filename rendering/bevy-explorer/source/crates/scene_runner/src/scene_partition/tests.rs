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
use ipfs::{IpfsIo, IpfsResource};
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
    world.insert_resource(IpfsResource {
        inner: Arc::new(IpfsIo::new(
            false,
            Box::new(MemoryAssetReader {
                root: Dir::default(),
            }),
            None,
            Default::default(),
            1,
            tokio::sync::mpsc::unbounded_channel().0,
        )),
    });
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
    world.spawn((
        PartitionedDownloads(vec!["shared".to_owned()]),
        Partition(near),
    ));

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
}

#[test]
fn publishes_each_scenes_key_with_the_players_scene_on_top() {
    let mut world = world_with_player(Some(Vec3::X));
    world.insert_resource(IpfsResource {
        inner: Arc::new(IpfsIo::new(
            false,
            Box::new(MemoryAssetReader {
                root: Dir::default(),
            }),
            None,
            Default::default(),
            1,
            tokio::sync::mpsc::unbounded_channel().0,
        )),
    });
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

/// The publisher's per-frame work at a busy plaza: 20 scenes in range, 50 avatars with 10
/// wearables each. Run with `--ignored --nocapture`.
#[test]
#[ignore]
fn overhead_of_publishing_at_a_busy_plaza() {
    let mut world = world_with_player(Some(Vec3::X));
    world.insert_resource(IpfsResource {
        inner: Arc::new(IpfsIo::new(
            false,
            Box::new(MemoryAssetReader {
                root: Dir::default(),
            }),
            None,
            Default::default(),
            32,
            tokio::sync::mpsc::unbounded_channel().0,
        )),
    });
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
            PartitionedDownloads(
                (0..10)
                    .map(|w| format!("wearable-{}", (a * 7 + w) % 120))
                    .collect(),
            ),
            GlobalTransform::from_translation(Vec3::new((a * 3) as f32, 0.0, -(a as f32))),
        ));
    }
    // the systems the publisher needs, as one schedule, so the run has no one-off cost
    let mut schedule = Schedule::default();
    schedule.add_systems((update_scene_partitions, publish_download_priorities).chain());
    schedule.run(&mut world);

    let start = std::time::Instant::now();
    for _ in 0..200 {
        schedule.run(&mut world);
    }
    println!(
        "publisher: 20 live scenes + 50 avatars x 10 wearables: {:?} per frame (partitions, map build, publish)",
        start.elapsed() / 200
    );
}
