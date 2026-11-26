use bevy::ecs::system::RunSystemOnce;
use common::structs::ServerConfiguration;
use comms::SetCurrentScene;

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
    world.init_resource::<CurrentRealm>();
    world.init_resource::<comms::SceneAdmission>();
    world.init_resource::<wallet::Wallet>();
    world.init_resource::<Time>();
    world.init_resource::<AppConfig>();
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

fn zero_load_distance(world: &mut World, live_scene_ring: u16) {
    world.insert_resource(SceneLoadDistance {
        load: 0.0,
        unload: 0.0,
        load_imposter: 0.0,
    });
    world.resource_mut::<AppConfig>().live_scene_ring = live_scene_ring;
}

#[test]
fn live_ring_zero_runs_only_the_players_own_scene() {
    let mut world = lifecycle_world();
    zero_load_distance(&mut world, 0);
    spawn_current_scene(&mut world, SceneLoading::Failed);

    world.run_system_once(process_scene_lifecycle).unwrap();

    // the neighbour, in ring 1, stays out
    assert!(spawned_hashes(&world).is_empty());
}

#[test]
fn a_wider_live_ring_goes_live_and_keeps_one_ring_further() {
    let mut world = lifecycle_world();
    zero_load_distance(&mut world, 2);

    // parcel (-1,0), on no scene: the neighbour is in ring 2
    move_player(&mut world, -8.0);
    world.run_system_once(process_scene_lifecycle).unwrap();
    assert!(spawned_hashes(&world).contains(&"neighbour".to_owned()));

    let mut world = lifecycle_world();
    zero_load_distance(&mut world, 2);
    let neighbour = spawn_live_neighbour(&mut world);

    // parcel (-2,0): ring 3, the keep ring
    move_player(&mut world, -24.0);
    world.run_system_once(process_scene_lifecycle).unwrap();
    assert!(world.get_entity(neighbour).is_ok());

    // parcel (-3,0): ring 4
    move_player(&mut world, -40.0);
    world.run_system_once(process_scene_lifecycle).unwrap();
    assert!(world.get_entity(neighbour).is_err());
}

#[test]
fn an_out_of_range_live_ring_is_clamped() {
    let mut world = lifecycle_world();
    // past the setting's 0 to 3 range, as a hand-edited config could hold
    zero_load_distance(&mut world, u16::MAX);
    let neighbour = spawn_live_neighbour(&mut world);

    // parcel (-3,0): ring 4, the keep ring of ring 3
    move_player(&mut world, -40.0);
    world.run_system_once(process_scene_lifecycle).unwrap();
    assert!(world.get_entity(neighbour).is_ok());

    // parcel (-4,0): ring 5
    move_player(&mut world, -56.0);
    world.run_system_once(process_scene_lifecycle).unwrap();
    assert!(world.get_entity(neighbour).is_err());
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
    // the deferred neighbour is still one the lifecycle loads around the player
    let required = &world.resource::<LiveScenes>().required;
    assert_eq!(required.around, Some(IVec2::ZERO));
    assert!(required.hashes.contains("neighbour"));
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
fn scene_ban_unloads_only_the_matching_scene_until_admitted() {
    let mut world = lifecycle_world();
    let mut wallet = wallet::Wallet::default();
    wallet.finalize_as_guest_with_seed([1; 32]);
    let address = wallet.address().unwrap();
    world.insert_resource(wallet);
    world.insert_resource(CurrentRealm::default());
    let current = spawn_current_scene(&mut world, SceneLoading::SceneEntity);
    world.insert_resource(
        comms::SceneAdmission::from_refusal(
            403,
            Some("User is banned from this scene"),
            false,
            "current",
        )
        .for_session("", address),
    );

    world.run_system_once(process_scene_lifecycle).unwrap();
    assert!(world.get_entity(current).is_err());
    assert!(!world.resource::<CurrentSceneLoading>().0);
    assert_eq!(spawned_hashes(&world), vec!["neighbour"]);
    // the banned scene is not one the lifecycle loads
    let required = &world.resource::<LiveScenes>().required.hashes;
    assert!(!required.contains("current"));
    assert!(required.contains("neighbour"));

    world.run_system_once(process_scene_lifecycle).unwrap();
    assert!(!world
        .resource::<LiveScenes>()
        .scenes
        .contains_key("current"));

    for x in [24.0, 8.0, 24.0, 8.0] {
        move_player(&mut world, x);
        world.run_system_once(process_scene_lifecycle).unwrap();
        assert!(!world
            .resource::<LiveScenes>()
            .scenes
            .contains_key("current"));
        assert!(!spawned_hashes(&world).iter().any(|hash| hash == "current"));
    }

    world.insert_resource(comms::SceneAdmission::allowed("current"));
    world.run_system_once(process_scene_lifecycle).unwrap();
    assert!(world
        .resource::<LiveScenes>()
        .scenes
        .contains_key("current"));
}

#[test]
fn returning_from_another_scene_targets_the_unloaded_banned_parcel() {
    let mut app = App::new();
    setup_lifecycle(app.world_mut());
    let mut wallet = wallet::Wallet::default();
    wallet.finalize_as_guest_with_seed([1; 32]);
    let address = wallet.address().unwrap();
    app.insert_resource(wallet)
        .insert_resource(
            comms::SceneAdmission::from_refusal(
                403,
                Some("User is banned from this scene"),
                false,
                "current",
            )
            .for_session("", address),
        )
        .add_event::<SetCurrentScene>()
        .add_systems(Update, crate::update_scene_room);
    app.world_mut()
        .resource_mut::<CurrentRealm>()
        .config
        .realm_name = Some("realm".into());
    let entity = spawn_live_neighbour(app.world_mut());
    app.world_mut()
        .entity_mut(entity)
        .insert(RendererSceneContext::new(
            SceneId(entity),
            "neighbour".into(),
            String::new(),
            false,
            0,
            "Neighbour".into(),
            IVec2::X,
            [IVec2::X].into_iter().collect(),
            Vec::new(),
            SceneBounds::default(),
            Vec::new(),
            entity,
            0.0,
            false,
            "",
            false,
            false,
        ));
    for (x, expected) in [
        (24.0, "neighbour"),
        (8.0, "current"),
        (24.0, "neighbour"),
        (8.0, "current"),
    ] {
        move_player(app.world_mut(), x);
        app.update();
        let events = app.world().resource::<Events<SetCurrentScene>>();
        let mut reader = events.get_cursor();
        assert_eq!(reader.read(events).last().unwrap().scene_id, expected);
        assert!(!app
            .world()
            .resource::<LiveScenes>()
            .scenes
            .contains_key("current"));
    }
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

/// What the player parcel report said: the parcel before, the parcel, the scene there and whether
/// the player was in world.
#[derive(Resource, Default)]
struct ReportedParcels(Vec<(Option<IVec2>, IVec2, Option<Option<String>>, bool)>);

/// An app running the player parcel report as the plugin does.
fn parcel_report_app() -> App {
    let mut app = App::new();
    app.init_resource::<ScenePointers>()
        .init_resource::<ReportedParcels>()
        .add_event::<PlayerParcelChanged>()
        .add_systems(
            PostUpdate,
            (
                player_parcel_report(),
                |mut changes: EventReader<PlayerParcelChanged>,
                 mut reported: ResMut<ReportedParcels>| {
                    for change in changes.read() {
                        reported.0.push((
                            change.old,
                            change.parcel,
                            change.scene.clone(),
                            change.in_world,
                        ));
                    }
                },
            )
                .chain(),
        );
    app
}

// A new parcel reports the one before; a change of the scene listed at the parcel the player
// stands on (a new realm at the same parcel), or entering the world, reports the parcel again.
// A player placed this frame is reported at the new parcel, with whether it is in world.
#[test]
fn the_player_parcel_reports_moves_scene_changes_and_entering_the_world() {
    let mut app = parcel_report_app();
    // behind the loading screen; its global transform is stale and never read
    let player = app
        .world_mut()
        .spawn((
            PrimaryUser::default(),
            Transform::from_xyz(8.0, 0.0, -8.0),
            GlobalTransform::from_xyz(-100.0, 0.0, 100.0),
            OutOfWorld::default(),
        ))
        .id();
    app.update();
    app.update();
    {
        let mut pointers = app.world_mut().resource_mut::<ScenePointers>();
        pointers.set_realm(IVec2::ZERO, IVec2::X);
        pointers.set_all_scenes_known();
    }
    app.update();
    // placed at the spawn point on the next parcel and let in
    app.world_mut()
        .get_mut::<Transform>(player)
        .unwrap()
        .translation = Vec3::new(24.0, 0.0, -8.0);
    app.world_mut().entity_mut(player).remove::<OutOfWorld>();
    app.update();
    app.world_mut()
        .entity_mut(player)
        .insert(OutOfWorld::default());
    app.update();

    assert_eq!(
        app.world().resource::<ReportedParcels>().0,
        [
            (None, IVec2::ZERO, None, false),
            (Some(IVec2::ZERO), IVec2::ZERO, Some(None), false),
            (Some(IVec2::ZERO), IVec2::X, Some(None), true),
            (Some(IVec2::X), IVec2::X, Some(None), false),
        ]
    );
}

// A teleport moves the player at once and defers its `OutOfWorld`. The frame it is made, the
// player is reported at the target out of world, never in world there.
#[test]
fn the_player_parcel_of_a_teleport_is_reported_out_of_world() {
    #[derive(Resource)]
    struct Teleport;
    fn teleport(
        mut commands: Commands,
        mut player: Single<(Entity, &mut Transform), With<PrimaryUser>>,
    ) {
        player.1.translation = Vec3::new(24.0, 0.0, -8.0);
        commands.entity(player.0).insert(OutOfWorld::default());
        commands.remove_resource::<Teleport>();
    }
    let mut app = parcel_report_app();
    app.add_systems(Update, teleport.run_if(resource_exists::<Teleport>));
    app.world_mut()
        .spawn((PrimaryUser::default(), Transform::from_xyz(8.0, 0.0, -8.0)));
    app.update();
    app.insert_resource(Teleport);
    app.update();
    app.update();

    assert_eq!(
        app.world().resource::<ReportedParcels>().0,
        [
            (None, IVec2::ZERO, None, true),
            (Some(IVec2::ZERO), IVec2::X, None, false),
        ]
    );
}

/// An app running the realm lifecycle as the plugin does. Its player stands in the world on
/// parcel 0,0, and `target` is where the next realm change lands them.
fn realm_app(target: RealmInitialLocation) -> (App, Entity) {
    use bevy::asset::io::memory::{Dir, MemoryAssetReader};
    use ipfs::{IpfsIo, IpfsResource};

    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
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
        .init_resource::<CurrentRealm>()
        .insert_resource(target)
        .add_systems(PostUpdate, realm_lifecycle_systems());
    setup_lifecycle(app.world_mut());
    let player = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryUser>>()
        .single(app.world())
        .unwrap();
    (app, player)
}

/// A live realm: one that lists `scenes` (a world), or none of them (a city).
fn live_realm(scenes: &[&str], spawn_parcel: Option<IVec2>) -> CurrentRealm {
    CurrentRealm {
        about_url: "https://realm.example/about".to_owned(),
        address: "https://realm.example".to_owned(),
        config: ServerConfiguration {
            scenes_urn: Some(scenes.iter().map(|urn| (*urn).to_owned()).collect()),
            ..Default::default()
        },
        spawn_parcel,
        ..Default::default()
    }
}

/// The realm becomes `realm`, and the lifecycle runs once on the change.
fn change_realm_to(app: &mut App, realm: CurrentRealm) {
    *app.world_mut().resource_mut::<CurrentRealm>() = realm;
    app.update();
}

/// Runs the app until `done`, for work that ends on another thread.
fn run_until(app: &mut App, done: impl Fn(&App) -> bool) {
    for _ in 0..5000 {
        if done(app) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
        app.update();
    }
    panic!("the app never got there");
}

/// The spawn point the player's landing names: `None` in the world, `Some(None)` on a landing
/// that names none.
fn landing_spawn_point(app: &App, player: Entity) -> Option<Option<String>> {
    app.world()
        .entity(player)
        .get::<OutOfWorld>()
        .map(|out_of_world| out_of_world.spawn_point.clone())
}

/// Where a landing put the player; `None` when none has moved them.
fn landing_position(app: &App, player: Entity) -> Option<Vec3> {
    app.world()
        .entity(player)
        .get::<Transform>()
        .map(|transform| transform.translation)
}

fn landing_target(app: &App) -> &RealmInitialLocation {
    app.world().resource::<RealmInitialLocation>()
}

fn door() -> Option<String> {
    Some("door".to_owned())
}

#[test]
fn a_realm_change_naming_a_parcel_lands_on_it_with_its_spawn_point() {
    let (mut app, player) = realm_app(RealmInitialLocation::Parcel {
        parcel: IVec2::new(3, 4),
        spawn_point: door(),
    });

    change_realm_to(&mut app, live_realm(&[], None));

    assert_eq!(landing_spawn_point(&app, player), Some(door()));
    // the middle of parcel 3,4
    assert_eq!(
        landing_position(&app, player),
        Some(Vec3::new(56.0, 0.0, -72.0))
    );
    assert_eq!(*landing_target(&app), RealmInitialLocation::None);
}

#[test]
fn a_realm_change_to_a_city_lands_on_its_spawn_point_where_the_player_stands() {
    let (mut app, player) = realm_app(RealmInitialLocation::Base {
        spawn_point: door(),
    });

    change_realm_to(&mut app, live_realm(&[], None));

    assert_eq!(landing_spawn_point(&app, player), Some(door()));
    assert_eq!(
        landing_position(&app, player),
        Some(Vec3::new(8.0, 0.0, -8.0))
    );
    assert_eq!(*landing_target(&app), RealmInitialLocation::None);
}

#[test]
fn a_realm_change_to_a_world_lands_on_its_spawn_parcel_with_the_spawn_point() {
    let (mut app, player) = realm_app(RealmInitialLocation::Base {
        spawn_point: door(),
    });

    change_realm_to(
        &mut app,
        live_realm(&["urn:decentraland:entity:lobby"], Some(IVec2::new(5, 6))),
    );

    assert_eq!(landing_spawn_point(&app, player), Some(door()));
    // the middle of parcel 5,6
    assert_eq!(
        landing_position(&app, player),
        Some(Vec3::new(88.0, 0.0, -104.0))
    );
    assert_eq!(*landing_target(&app), RealmInitialLocation::None);
}

/// A directory of the system's temporary one, removed with this.
struct ScratchDir(std::path::PathBuf);

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// A world without a spawn parcel lands on its first scene, which is known only once read. The
// spawn point waits on the target meanwhile, and a landing that was under way before the realm
// change does not lend its own to the new realm.
#[test]
fn a_realm_change_to_a_world_without_a_spawn_parcel_names_the_spawn_point_once_its_scene_resolves()
{
    use ipfs::{EndpointConfig, IpfsResource, ServerAbout};

    // the world's one scene, on parcel 7,8, in a directory the content reads are allowed
    let dir = std::env::temp_dir().join(format!("realm-landing-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let dir = ScratchDir(dir.canonicalize().unwrap());
    let scene = serde_json::json!({
        "pointers": ["7,8"],
        "content": [],
        "metadata": { "main": "index.js", "scene": { "base": "7,8", "parcels": ["7,8"] } },
    });
    std::fs::write(dir.0.join("lobby"), scene.to_string()).unwrap();
    let path = dir.0.to_string_lossy().replace('\\', "/");
    let path = path.trim_start_matches("//?/").trim_start_matches('/');
    let urn = format!("urn:decentraland:entity:lobby?=&baseUrl=file:///{path}/");

    let (mut app, player) = realm_app(RealmInitialLocation::Base {
        spawn_point: door(),
    });
    let ipfs = app.world().resource::<IpfsResource>().clone();
    ipfs.add_allowed_file_root(dir.0.clone());
    ipfs.set_realm_about(ServerAbout {
        content: Some(EndpointConfig {
            healthy: true,
            public_url: "https://realm.example/content".to_owned(),
        }),
        ..Default::default()
    });
    app.world_mut().entity_mut(player).insert(OutOfWorld {
        spawn_point: Some("roof".to_owned()),
    });

    change_realm_to(&mut app, live_realm(&[&urn], None));

    assert_eq!(landing_spawn_point(&app, player), Some(None));
    assert_eq!(
        *landing_target(&app),
        RealmInitialLocation::Base {
            spawn_point: door()
        }
    );

    run_until(&mut app, |app| {
        *landing_target(app) == RealmInitialLocation::None
    });

    assert_eq!(landing_spawn_point(&app, player), Some(door()));
    // the middle of parcel 7,8
    assert_eq!(
        landing_position(&app, player),
        Some(Vec3::new(120.0, 0.0, -136.0))
    );
}

// A teleport inside the realm made while a realm change is on its way gives up that change's
// landing for its own (`RealmInitialLocation::None`, and `OutOfWorld` with the name it asked
// for). The realm change still lands, and the landing goes on in the new realm.
#[test]
fn a_landing_under_way_keeps_its_spawn_point_through_a_realm_change_without_a_target() {
    let (mut app, player) = realm_app(RealmInitialLocation::None);
    // the middle of parcel 1,2
    let asked = Vec3::new(24.0, 0.0, -40.0);
    app.world_mut().entity_mut(player).insert((
        Transform::from_translation(asked),
        OutOfWorld {
            spawn_point: Some("roof".to_owned()),
        },
    ));

    change_realm_to(
        &mut app,
        live_realm(&["urn:decentraland:entity:lobby"], Some(IVec2::new(5, 6))),
    );

    assert_eq!(
        landing_spawn_point(&app, player),
        Some(Some("roof".to_owned()))
    );
    assert_eq!(landing_position(&app, player), Some(asked));
}

#[test]
fn a_realm_change_without_a_target_puts_a_player_in_the_world_out_of_it() {
    let (mut app, player) = realm_app(RealmInitialLocation::None);

    change_realm_to(&mut app, live_realm(&[], None));

    assert_eq!(landing_spawn_point(&app, player), Some(None));
    assert_eq!(landing_position(&app, player), None);
}

/// `realm_app` with realm changes asked for through `ipfs::change_realm`, and its player in the
/// world of a realm that answered.
fn realm_app_taking_changes() -> (App, Entity) {
    use bevy_console::PrintConsoleLine;
    use ipfs::{world_access::WorldCommsSecret, ChangeRealmEvent, IpfsResource, ServerAbout};

    let (mut app, player) = realm_app(RealmInitialLocation::None);
    app.add_event::<ChangeRealmEvent>()
        .add_event::<PrintConsoleLine>()
        .init_resource::<WorldCommsSecret>()
        .add_systems(Update, ipfs::change_realm);
    app.world()
        .resource::<IpfsResource>()
        .set_realm_about(ServerAbout::default());
    run_until(&mut app, |app| {
        !app.world().resource::<CurrentRealm>().about_url.is_empty()
    });
    app.world_mut().entity_mut(player).remove::<OutOfWorld>();
    (app, player)
}

/// A teleport to parcel 3,4 of `realm`, on the spawn point `door`, as `teleport_player` asks for
/// it. Returns where the realm change answers.
fn teleport_to_realm(
    app: &mut App,
    realm: String,
) -> common::rpc::RpcResultReceiver<Result<(), String>> {
    *app.world_mut().resource_mut::<RealmInitialLocation>() = RealmInitialLocation::Parcel {
        parcel: IVec2::new(3, 4),
        spawn_point: door(),
    };
    let (response, answer) = common::rpc::RpcResultSender::channel();
    app.world_mut().send_event(ipfs::ChangeRealmEvent {
        new_realm: realm,
        content_server_override: None,
        response,
        report: false,
    });
    answer
}

// A realm change that fails keeps the player in the realm they are in, and gives up the landing
// it was to make: the next realm change does not land on its parcel or its spawn point.
#[test]
fn a_realm_change_that_fails_leaves_no_landing_for_the_next_one() {
    let (mut app, player) = realm_app_taking_changes();

    // a realm that does not answer: nothing listens on this port
    let closed = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let mut answer = teleport_to_realm(&mut app, format!("http://{closed}"));
    run_until(&mut app, |app| {
        *landing_target(app) == RealmInitialLocation::None
    });

    assert!(matches!(answer.try_recv(), Ok(Err(_))));
    assert_eq!(landing_spawn_point(&app, player), None);
    assert_eq!(landing_position(&app, player), None);

    change_realm_to(&mut app, live_realm(&[], None));

    assert_eq!(landing_spawn_point(&app, player), Some(None));
    assert_eq!(landing_position(&app, player), None);
}

// A scene preview refuses every realm change, and gives up the landing as a change that failed
// does.
#[test]
fn a_realm_change_refused_in_a_preview_leaves_no_landing_for_the_next_one() {
    let (mut app, player) = realm_app_taking_changes();
    app.world_mut().resource_mut::<PreviewMode>().is_preview = true;

    let mut answer = teleport_to_realm(&mut app, "mana.dcl.eth".to_owned());
    app.update();

    assert!(matches!(answer.try_recv(), Ok(Err(_))));
    assert_eq!(*landing_target(&app), RealmInitialLocation::None);
    assert_eq!(landing_spawn_point(&app, player), None);

    change_realm_to(&mut app, live_realm(&[], None));

    assert_eq!(landing_spawn_point(&app, player), Some(None));
    assert_eq!(landing_position(&app, player), None);
}
