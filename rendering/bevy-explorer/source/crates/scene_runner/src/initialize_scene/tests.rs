use bevy::ecs::system::RunSystemOnce;
use common::structs::ServerConfiguration;

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
    world.init_resource::<Time>();
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
            SceneIpfsLocation::ContentFile { hash, file } => format!("{hash}/{file}"),
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

fn spawn_live_neighbour(world: &mut World) -> Entity {
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
    neighbour
}

fn move_player(world: &mut World, x: f32) {
    let mut users = world.query_filtered::<&mut GlobalTransform, With<PrimaryUser>>();
    *users.single_mut(world).unwrap() = GlobalTransform::from_translation(Vec3::new(x, 0.0, -8.0));
}

#[test]
fn live_scene_is_kept_through_the_unload_margin() {
    let mut world = lifecycle_world();
    // distances wider than the keep ring, so the margin alone decides
    world.insert_resource(SceneLoadDistance {
        load: 40.0,
        unload: 20.0,
        load_imposter: 0.0,
    });
    let neighbour = spawn_live_neighbour(&mut world);

    // parcel (-2,0), ring 3: 46 m from its nearest edge, past the 40 m load distance, inside
    // the 20 m margin
    move_player(&mut world, -30.0);
    world.run_system_once(process_scene_lifecycle).unwrap();
    assert!(
        world.get_entity(neighbour).is_ok(),
        "a live scene inside the unload margin is kept"
    );
    assert!(world
        .resource::<LiveScenes>()
        .scenes
        .contains_key("neighbour"));

    // parcel (-4,0), ring 5: 66 m, past the margin
    move_player(&mut world, -50.0);
    world.run_system_once(process_scene_lifecycle).unwrap();
    assert!(world.get_entity(neighbour).is_err());
    assert!(!world
        .resource::<LiveScenes>()
        .scenes
        .contains_key("neighbour"));
}

#[test]
fn ring_one_goes_live_at_a_zero_load_distance() {
    let mut world = lifecycle_world();
    world.insert_resource(SceneLoadDistance {
        load: 0.0,
        unload: 0.0,
        load_imposter: 0.0,
    });
    spawn_current_scene(&mut world, SceneLoading::Failed);

    world.run_system_once(process_scene_lifecycle).unwrap();

    assert_eq!(spawned_hashes(&world), vec!["neighbour".to_owned()]);
}

/// A negative load distance (a headless worker that takes its scenes from a control channel)
/// loads nothing by position: the ring stays off.
#[test]
fn the_ring_is_off_when_loading_by_position_is() {
    let mut world = lifecycle_world();
    world.insert_resource(SceneLoadDistance {
        load: -1.0,
        unload: 0.0,
        load_imposter: 0.0,
    });
    spawn_current_scene(&mut world, SceneLoading::Failed);

    world.run_system_once(process_scene_lifecycle).unwrap();

    assert!(spawned_hashes(&world).is_empty());
}

#[test]
fn ring_parcels_is_the_square_around_the_center() {
    let center = IVec2::new(3, -2);
    assert_eq!(
        crate::ring_parcels(center, 0).collect::<Vec<_>>(),
        vec![center]
    );
    let ring_one = crate::ring_parcels(center, 1).collect::<Vec<_>>();
    assert_eq!(ring_one.len(), 9);
    assert!(ring_one.contains(&(center + IVec2::new(-1, 1))));
    assert_eq!(crate::ring_parcels(center, -1).count(), 0);
}

#[test]
fn live_scene_is_kept_to_ring_two_at_a_zero_load_distance() {
    let mut world = lifecycle_world();
    world.insert_resource(SceneLoadDistance {
        load: 0.0,
        unload: 0.0,
        load_imposter: 0.0,
    });
    let neighbour = spawn_live_neighbour(&mut world);

    // parcel (-1,0): the neighbour is in ring 2
    move_player(&mut world, -8.0);
    world.run_system_once(process_scene_lifecycle).unwrap();
    assert!(world.get_entity(neighbour).is_ok());

    // parcel (-2,0): ring 3
    move_player(&mut world, -24.0);
    world.run_system_once(process_scene_lifecycle).unwrap();
    assert!(world.get_entity(neighbour).is_err());
    assert!(!world
        .resource::<LiveScenes>()
        .scenes
        .contains_key("neighbour"));
}

/// Nine scenes in a row east of the player, who stands on a parcel with no scene of its own, all
/// inside the load distance; the first wave is spawned.
fn first_wave_world() -> World {
    let mut world = lifecycle_world();
    world.insert_resource(SceneLoadDistance {
        load: 200.0,
        unload: 0.0,
        load_imposter: 0.0,
    });
    let mut pointers = ScenePointers::default();
    for k in 1..=9 {
        pointers.insert(
            IVec2::new(k, 0),
            PointerResult::Exists {
                hash: format!("s{k}"),
                urn: None,
            },
        );
    }
    world.insert_resource(pointers);

    world.run_system_once(process_scene_lifecycle).unwrap();
    let first_wave: Vec<_> = (1..=SCENES_PER_WAVE).map(|k| format!("s{k}")).collect();
    assert_eq!(spawned_hashes(&world), first_wave);
    world
}

/// The scenes spawned after the first wave, in no particular order: they spawn in one frame.
fn later_hashes(world: &World) -> HashSet<String> {
    spawned_hashes(world)
        .into_iter()
        .skip(SCENES_PER_WAVE)
        .collect()
}

fn hashes(range: std::ops::RangeInclusive<i32>) -> HashSet<String> {
    range.map(|k| format!("s{k}")).collect()
}

#[test]
fn scenes_in_range_spawn_in_waves_nearest_first() {
    let mut world = first_wave_world();

    // the wave is full while they load
    world.run_system_once(process_scene_lifecycle).unwrap();
    assert_eq!(spawned_hashes(&world).len(), SCENES_PER_WAVE);

    // once they are done (a failed load is as done as it gets) the rest come in
    let loading: Vec<Entity> = world
        .resource::<LiveScenes>()
        .scenes
        .values()
        .copied()
        .collect();
    for entity in loading {
        world.entity_mut(entity).insert(SceneLoading::Failed);
    }
    world.run_system_once(process_scene_lifecycle).unwrap();
    assert_eq!(later_hashes(&world), hashes(7..=9));
}

#[test]
fn a_ready_scene_frees_one_slot_for_the_nearest_waiting() {
    let mut world = first_wave_world();

    // the first scene passes its first frames
    let entity = world.resource::<LiveScenes>().scenes["s1"];
    let mut context = RendererSceneContext::new(
        SceneId(entity),
        "s1".to_owned(),
        String::new(),
        false,
        0,
        "s1".to_owned(),
        IVec2::new(1, 0),
        [IVec2::new(1, 0)].into_iter().collect(),
        Vec::new(),
        SceneBounds::default(),
        Vec::new(),
        entity,
        0.0,
        false,
        "",
        false,
        false,
    );
    context.tick_number = 7;
    world.entity_mut(entity).insert(context);

    world.run_system_once(process_scene_lifecycle).unwrap();
    assert_eq!(later_hashes(&world), hashes(7..=7));
}

#[test]
fn a_stalled_wave_stops_holding_its_slots() {
    let mut world = first_wave_world();

    world
        .resource_mut::<Time>()
        .advance_by(WAVE_SLOT_TIMEOUT + Duration::from_secs(1));
    world.run_system_once(process_scene_lifecycle).unwrap();
    assert_eq!(later_hashes(&world), hashes(7..=9));
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
fn a_smart_wearable_loads_from_its_content_file_while_the_current_scene_loads() {
    let mut world = lifecycle_world();
    spawn_current_scene(&mut world, SceneLoading::SceneEntity);
    let location = SceneIpfsLocation::ContentFile {
        hash: "wearable".to_owned(),
        file: "male.smart_wearable".to_owned(),
    };
    world.resource_mut::<PortableScenes>().insert(
        "wearable-sw-male".to_owned(),
        PortableSource {
            pid: "urn:decentraland:matic:collections-v2:0xabc:0".to_owned(),
            parent_scene: None,
            ens: None,
            super_user: false,
            kind: PortableKind::SmartWearable {
                name: "wearable".to_owned(),
                location: location.clone(),
            },
        },
    );

    world.run_system_once(process_scene_lifecycle).unwrap();

    // the current scene still defers its neighbour, but not the wearable
    assert!(world.resource::<CurrentSceneLoading>().0);
    let events = world.resource::<Events<LoadSceneEvent>>();
    let locations = events
        .iter_current_update_events()
        .map(|ev| &ev.location)
        .collect::<Vec<_>>();
    assert_eq!(locations, [&location]);
    assert!(world
        .resource::<LiveScenes>()
        .scenes
        .contains_key("wearable-sw-male"));
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

/// an L of three parcels and one parcel apart from it: two connected regions
const SPLIT_SCENE: [IVec2; 4] = [IVec2::ZERO, IVec2::X, IVec2::Y, IVec2::splat(5)];

/// the height limit over each parcel of a scene with these parcels, in a realm whose `about`
/// carries this configuration
fn heights_over(parcels: &[IVec2], config: &ServerConfiguration) -> Vec<f32> {
    let realm = CurrentRealm {
        config: config.clone(),
        ..Default::default()
    };
    let regions = scene_bound_regions(&parcels.iter().copied().collect(), &realm);
    parcels
        .iter()
        .map(|parcel| {
            let over = regions
                .iter()
                .filter(|r| {
                    parcel.cmpge(r.parcel_min()).all() && parcel.cmple(r.parcel_max()).all()
                })
                .collect::<Vec<_>>();
            assert_eq!(over.len(), 1, "{parcel} is in one region");
            over[0].height
        })
        .collect()
}

#[test]
fn a_world_scene_is_bounded_at_the_fixed_height() {
    let world = ServerConfiguration {
        scenes_urn: Some(vec!["urn:decentraland:entity:world-scene".to_owned()]),
        ..Default::default()
    };

    assert_eq!(heights_over(&[IVec2::ZERO], &world), [FIXED_SCENE_HEIGHT]);
    assert_eq!(heights_over(&SPLIT_SCENE, &world), [FIXED_SCENE_HEIGHT; 4]);
}

#[test]
fn a_preview_scene_is_bounded_at_the_fixed_height() {
    // a preview lists the project's parcels, whether or not it lists its scene
    for preview in [
        ServerConfiguration {
            scenes_urn: Some(Vec::new()),
            local_scene_parcels: Some(vec!["0,0".to_owned()]),
            ..Default::default()
        },
        ServerConfiguration {
            scenes_urn: Some(vec!["urn:decentraland:entity:preview-scene".to_owned()]),
            local_scene_parcels: Some(vec!["0,0".to_owned()]),
            ..Default::default()
        },
    ] {
        assert_eq!(heights_over(&[IVec2::ZERO], &preview), [FIXED_SCENE_HEIGHT]);
        assert_eq!(
            heights_over(&SPLIT_SCENE, &preview),
            [FIXED_SCENE_HEIGHT; 4]
        );
    }
}

#[test]
fn a_city_scene_is_bounded_by_the_parcel_count_of_each_region() {
    // a city realm lists neither its scenes nor a project's parcels: no list, or an empty one
    for city in [
        ServerConfiguration::default(),
        ServerConfiguration {
            scenes_urn: Some(Vec::new()),
            ..Default::default()
        },
        ServerConfiguration {
            scenes_urn: Some(Vec::new()),
            local_scene_parcels: Some(Vec::new()),
            ..Default::default()
        },
    ] {
        // log2(parcels + 1) * 20
        assert_eq!(heights_over(&[IVec2::ZERO], &city), [20.0]);
        assert_eq!(heights_over(&SPLIT_SCENE, &city), [40.0, 40.0, 40.0, 20.0]);
    }
}
