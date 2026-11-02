use bevy::ecs::system::RunSystemOnce;
use common::structs::PrimaryCamera;

use super::*;
use crate::scene_partition::ScenePartition;

fn lifecycle_world() -> World {
    let mut world = World::new();
    setup_lifecycle(&mut world);
    world
}

fn lifecycle_world_with(scenes: &[(IVec2, &str)], camera_forward: Vec3) -> World {
    let mut world = World::new();
    setup_lifecycle_with(&mut world, scenes, camera_forward);
    world
}

fn setup_lifecycle(world: &mut World) {
    setup_lifecycle_with(
        world,
        &[(IVec2::ZERO, "current"), (IVec2::X, "neighbour")],
        Vec3::X,
    );
}

fn setup_lifecycle_with(world: &mut World, scenes: &[(IVec2, &str)], camera_forward: Vec3) {
    world.init_resource::<PortableScenes>();
    world.init_resource::<LiveScenes>();
    world.init_resource::<Events<LoadSceneEvent>>();
    world.init_resource::<CurrentImposterScene>();
    world.init_resource::<PreviewMode>();
    world.init_resource::<CurrentSceneLoading>();
    world.insert_resource(SceneLoadDistance {
        load: 20.0,
        unload: 10.0,
        load_imposter: 0.0,
    });

    let mut pointers = ScenePointers::default();
    for (parcel, hash) in scenes {
        pointers.insert(
            *parcel,
            PointerResult::Exists {
                hash: (*hash).to_owned(),
                urn: None,
            },
        );
    }
    world.insert_resource(pointers);

    // stand in the middle of parcel (0,0), the camera at the same spot
    let position = Vec3::new(8.0, 0.0, -8.0);
    world.spawn((
        PrimaryUser::default(),
        GlobalTransform::from_translation(position),
    ));
    world.spawn((
        PrimaryCamera::default(),
        Camera::default(),
        GlobalTransform::from(
            Transform::from_translation(position).looking_to(camera_forward, Vec3::Y),
        ),
    ));
}

fn spawned_hashes(world: &World) -> Vec<String> {
    // `Events::update` never runs here, so this holds every event written so far
    world
        .resource::<Events<LoadSceneEvent>>()
        .iter_current_update_events()
        .map(|ev| match &ev.location {
            SceneIpfsLocation::Hash(hash) => hash.clone(),
            SceneIpfsLocation::Urn(urn) => urn.clone(),
        })
        .collect()
}

fn spawn_current_scene(world: &mut World, state: SceneLoading) -> Entity {
    let current = world.spawn((SceneHash("current".to_owned()), state)).id();
    world
        .resource_mut::<LiveScenes>()
        .scenes
        .insert("current".to_owned(), current);
    current
}

#[test]
fn failed_current_scene_does_not_defer_other_scenes() {
    let mut world = lifecycle_world();
    let current = spawn_current_scene(&mut world, SceneLoading::Failed);

    world.run_system_once(process_scene_lifecycle).unwrap();

    assert!(
        !world.resource::<CurrentSceneLoading>().0,
        "a failed current scene must not count as loading"
    );
    // the failed entity is kept and tracked, so it isn't respawned
    assert!(world.get_entity(current).is_ok());
    assert_eq!(
        world.resource::<LiveScenes>().scenes.get("current"),
        Some(&current)
    );
    // and the neighbouring scene is no longer deferred behind it
    assert_eq!(spawned_hashes(&world), vec!["neighbour".to_owned()]);
}

#[test]
fn loading_current_scene_defers_other_scenes() {
    let mut world = lifecycle_world();
    spawn_current_scene(&mut world, SceneLoading::SceneEntity);

    world.run_system_once(process_scene_lifecycle).unwrap();

    assert!(world.resource::<CurrentSceneLoading>().0);
    assert!(spawned_hashes(&world).is_empty());
}

#[test]
fn empty_definition_is_kept_without_deferring_and_despawned_out_of_range() {
    use std::path::Path;

    use bevy::asset::io::{
        memory::{Dir, MemoryAssetReader},
        AssetSourceBuilder,
    };
    use ipfs::{EntityDefinitionLoader, IpfsIo, IpfsResource};

    // an active-entities response with nothing at the pointer
    let dir = Dir::default();
    dir.insert_asset_text(Path::new("empty.entity_definition"), "[]");

    let mut app = App::new();
    app.register_asset_source(
        "mem",
        AssetSourceBuilder::default()
            .with_reader(move || Box::new(MemoryAssetReader { root: dir.clone() })),
    );
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<EntityDefinition>()
        .init_asset_loader::<EntityDefinitionLoader>()
        .insert_resource(IpfsResource {
            inner: std::sync::Arc::new(IpfsIo::new(
                false,
                Box::new(MemoryAssetReader {
                    root: Dir::default(),
                }),
                None,
                Default::default(),
                1,
                tokio::sync::mpsc::unbounded_channel().0,
            )),
        })
        .add_systems(Update, (load_scene_json, process_scene_lifecycle).chain());
    setup_lifecycle(app.world_mut());

    let h_definition = app
        .world()
        .resource::<AssetServer>()
        .load::<EntityDefinition>("mem://empty.entity_definition");
    let current = app
        .world_mut()
        .spawn((
            SceneHash("current".to_owned()),
            SceneLoading::SceneEntity,
            SceneEntityDefinitionHandle(h_definition),
        ))
        .id();
    app.world_mut()
        .resource_mut::<LiveScenes>()
        .scenes
        .insert("current".to_owned(), current);

    let mut failed = false;
    for _ in 0..1000 {
        app.update();
        if matches!(
            app.world().get::<SceneLoading>(current),
            Some(SceneLoading::Failed)
        ) {
            failed = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(failed, "empty definition should mark the scene as failed");

    // the next lifecycle pass sees the failed load as terminal
    app.update();
    assert!(!app.world().resource::<CurrentSceneLoading>().0);
    assert_eq!(
        app.world().resource::<LiveScenes>().scenes.get("current"),
        Some(&current)
    );
    assert!(app.world().get_entity(current).is_ok());
    // the current scene is never respawned, and no longer defers its neighbour
    assert_eq!(spawned_hashes(app.world()), vec!["neighbour".to_owned()]);

    // leave the range
    let mut users = app
        .world_mut()
        .query_filtered::<&mut GlobalTransform, With<PrimaryUser>>();
    *users.single_mut(app.world_mut()).unwrap() =
        GlobalTransform::from_translation(Vec3::new(1000.0, 0.0, -1000.0));
    app.update();

    assert!(app.world().get_entity(current).is_err());
    assert!(!app
        .world()
        .resource::<LiveScenes>()
        .scenes
        .contains_key("current"));
}

const EAST_WEST: [(IVec2, &str); 3] = [
    (IVec2::ZERO, "current"),
    (IVec2::X, "east"),
    (IVec2::NEG_X, "west"),
];

#[test]
fn spawns_in_front_before_behind() {
    let mut world = lifecycle_world_with(&EAST_WEST, Vec3::X);
    spawn_current_scene(&mut world, SceneLoading::Failed);

    world.run_system_once(process_scene_lifecycle).unwrap();

    assert_eq!(
        spawned_hashes(&world),
        vec!["east".to_owned(), "west".to_owned()]
    );
}

#[test]
fn turning_the_camera_swaps_the_order() {
    let mut world = lifecycle_world_with(&EAST_WEST, Vec3::NEG_X);
    spawn_current_scene(&mut world, SceneLoading::Failed);

    world.run_system_once(process_scene_lifecycle).unwrap();

    assert_eq!(
        spawned_hashes(&world),
        vec!["west".to_owned(), "east".to_owned()]
    );
}

#[test]
fn nearer_scenes_spawn_first() {
    let mut world = lifecycle_world_with(
        &[
            (IVec2::ZERO, "current"),
            (IVec2::X, "near"),
            (IVec2::new(3, 0), "far"),
        ],
        Vec3::X,
    );
    world.insert_resource(SceneLoadDistance {
        load: 60.0,
        unload: 10.0,
        load_imposter: 0.0,
    });
    spawn_current_scene(&mut world, SceneLoading::Failed);

    world.run_system_once(process_scene_lifecycle).unwrap();

    assert_eq!(
        spawned_hashes(&world),
        vec!["near".to_owned(), "far".to_owned()]
    );
}

#[test]
fn a_spawned_scene_carries_its_partition() {
    let mut world = lifecycle_world_with(&EAST_WEST, Vec3::X);
    spawn_current_scene(&mut world, SceneLoading::Failed);

    world.run_system_once(process_scene_lifecycle).unwrap();

    let live = world.resource::<LiveScenes>().scenes.clone();
    for (hash, entity) in &live {
        assert!(
            world.get::<ScenePartition>(*entity).is_some(),
            "{hash} has no partition"
        );
    }
    let east = world.get::<ScenePartition>(live["east"]).unwrap().0;
    assert_eq!(east.parcel_distance, 1);
    assert!(!east.is_behind);
}
