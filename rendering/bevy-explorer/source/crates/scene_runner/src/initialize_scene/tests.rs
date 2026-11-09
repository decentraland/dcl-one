use bevy::ecs::system::RunSystemOnce;

use super::*;

fn lifecycle_world() -> World {
    let mut world = World::new();
    setup_lifecycle(&mut world);
    world
}

fn setup_lifecycle(world: &mut World) {
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
    for (parcel, hash) in [(IVec2::ZERO, "current"), (IVec2::X, "neighbour")] {
        pointers.insert(
            parcel,
            PointerResult::Exists {
                hash: hash.to_owned(),
                urn: None,
            },
        );
    }
    world.insert_resource(pointers);

    // stand in the middle of parcel (0,0)
    world.spawn((
        PrimaryUser::default(),
        GlobalTransform::from_translation(Vec3::new(8.0, 0.0, -8.0)),
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
fn live_scene_is_kept_through_the_unload_margin() {
    let mut world = lifecycle_world();
    // the neighbour (parcel 1,0: x from 16 to 32) is live
    let neighbour = world
        .spawn((
            SceneHash("neighbour".to_owned()),
            SceneLoading::SceneSpawned,
        ))
        .id();
    world
        .resource_mut::<LiveScenes>()
        .scenes
        .insert("neighbour".to_owned(), neighbour);
    let mut users = world.query_filtered::<&mut GlobalTransform, With<PrimaryUser>>();

    // 21 m from its nearest edge: past the 20 m load distance, inside the 10 m margin
    *users.single_mut(&mut world).unwrap() =
        GlobalTransform::from_translation(Vec3::new(-5.0, 0.0, -8.0));
    world.run_system_once(process_scene_lifecycle).unwrap();
    assert!(
        world.get_entity(neighbour).is_ok(),
        "a live scene inside the unload margin is kept"
    );
    assert!(world
        .resource::<LiveScenes>()
        .scenes
        .contains_key("neighbour"));

    // 31 m: past the margin
    *users.single_mut(&mut world).unwrap() =
        GlobalTransform::from_translation(Vec3::new(-15.0, 0.0, -8.0));
    world.run_system_once(process_scene_lifecycle).unwrap();
    assert!(world.get_entity(neighbour).is_err());
    assert!(!world
        .resource::<LiveScenes>()
        .scenes
        .contains_key("neighbour"));
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
