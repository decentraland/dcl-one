use bevy::ecs::system::RunSystemOnce;
use common::structs::{Arrivals, OutOfWorld, PrimaryUser};
use dcl_types::SceneId;
use scene_material::SceneBounds;
use web_time::Instant;

use super::*;
use crate::{
    initialize_scene::{
        LiveScenes, PointerResult, PortableScenes, SceneHash, SceneLoading, ScenePointers,
    },
    renderer_context::RendererSceneContext,
};

#[test]
fn ring_of_is_the_chebyshev_distance_to_the_nearest_parcel() {
    assert_eq!(
        ring_of(IVec2::ZERO, [IVec2::new(1, 0), IVec2::new(3, 3)]),
        1
    );
    assert_eq!(ring_of(IVec2::ZERO, [IVec2::new(-2, 5)]), 5);
    assert_eq!(ring_of(IVec2::ZERO, []), u16::MAX);
}

/// A world with the player in the middle of parcel (0,0), where scene "home" is.
fn world() -> World {
    let mut world = World::new();
    world.init_resource::<PortableScenes>();
    world.init_resource::<LiveScenes>();
    world.init_resource::<FirstRingTiming>();
    let mut pointers = ScenePointers::default();
    pointers.insert(
        IVec2::ZERO,
        PointerResult::Exists {
            hash: "home".to_owned(),
            urn: None,
        },
    );
    world.insert_resource(pointers);
    world.spawn((
        PrimaryUser::default(),
        GlobalTransform::from_translation(Vec3::new(8.0, 0.0, -8.0)),
    ));
    world
}

fn scene(world: &mut World, hash: &str, parcels: &[IVec2], tick: u32) -> Entity {
    let root = world
        .spawn((SpawnedAt(Instant::now()), SceneHash(hash.to_owned())))
        .id();
    let mut context = RendererSceneContext::new(
        SceneId(root),
        hash.to_owned(),
        String::new(),
        false,
        0,
        hash.to_owned(),
        parcels[0],
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
    context.tick_number = tick;
    world.entity_mut(root).insert(context);
    world
        .resource_mut::<LiveScenes>()
        .scenes
        .insert(hash.to_owned(), root);
    root
}

fn set_tick(world: &mut World, scene: Entity, tick: u32) {
    world
        .get_mut::<RendererSceneContext>(scene)
        .unwrap()
        .tick_number = tick;
}

#[test]
fn a_scene_is_logged_once_it_reaches_its_fifth_tick() {
    let mut world = world();
    let loading = scene(&mut world, "home", &[IVec2::ZERO], 2);

    world.run_system_once(report_scene_load_times).unwrap();
    assert!(!world.entity(loading).contains::<ReadyLogged>());

    set_tick(&mut world, loading, 5);
    world.run_system_once(report_scene_load_times).unwrap();
    assert!(world.entity(loading).contains::<ReadyLogged>());
}

#[test]
fn the_first_ring_is_reported_once_the_players_scene_and_its_neighbours_are_ready() {
    let mut world = world();
    let home = scene(&mut world, "home", &[IVec2::ZERO], 2);
    let neighbour = scene(&mut world, "east", &[IVec2::X], 2);
    let far = scene(&mut world, "far", &[IVec2::new(5, 0)], 0);

    // arrival: the player's scene is still loading
    world.run_system_once(report_scene_load_times).unwrap();
    assert!(world.resource::<FirstRingTiming>().arrival.is_some());
    assert!(!world.resource::<FirstRingTiming>().reported);

    // the player's scene alone is not enough
    set_tick(&mut world, home, 5);
    world.run_system_once(report_scene_load_times).unwrap();
    assert!(!world.resource::<FirstRingTiming>().reported);

    // the first ring completes; the far scene does not matter
    set_tick(&mut world, neighbour, 5);
    world.run_system_once(report_scene_load_times).unwrap();
    assert!(world.resource::<FirstRingTiming>().reported);
    assert!(!world.entity(far).contains::<ReadyLogged>());
}

#[test]
fn a_failed_neighbour_does_not_hold_the_report() {
    let mut world = world();
    let home = scene(&mut world, "home", &[IVec2::ZERO], 2);
    world.spawn((
        SpawnedAt(Instant::now()),
        SceneHash("broken".to_owned()),
        SceneLoading::Failed,
    ));

    world.run_system_once(report_scene_load_times).unwrap();
    set_tick(&mut world, home, 5);
    world.run_system_once(report_scene_load_times).unwrap();

    assert!(world.resource::<FirstRingTiming>().reported);
}

#[test]
fn landing_behind_the_loading_screen_times_the_destination_scene() {
    let mut world = world();
    let player = world
        .query_filtered::<Entity, With<PrimaryUser>>()
        .single(&world)
        .unwrap();
    world.entity_mut(player).insert(OutOfWorld::default());
    let home = scene(&mut world, "home", &[IVec2::ZERO], 2);

    // the destination is still loading while the player waits for it
    world.run_system_once(report_scene_load_times).unwrap();
    assert!(world.resource::<FirstRingTiming>().arrival.is_some());

    // the player is let in once it is ready, and the arrival is reported
    set_tick(&mut world, home, 6);
    world.entity_mut(player).remove::<OutOfWorld>();
    world.run_system_once(report_scene_load_times).unwrap();
    let timing = world.resource::<FirstRingTiming>();
    assert_eq!(timing.arrival.map(|(scene, _)| scene), Some(home));
    assert!(timing.reported);
}

#[test]
fn an_arrival_whose_scene_despawns_is_not_reported() {
    let mut world = world();
    let home = scene(&mut world, "home", &[IVec2::ZERO], 2);

    world.run_system_once(report_scene_load_times).unwrap();
    assert!(world.resource::<FirstRingTiming>().arrival.is_some());

    // a realm change purges the scene and its pointers before it is ready
    world.despawn(home);
    world.resource_mut::<LiveScenes>().scenes.clear();
    world.insert_resource(ScenePointers::default());
    world.run_system_once(report_scene_load_times).unwrap();

    let timing = world.resource::<FirstRingTiming>();
    assert!(timing.arrival.is_none());
    assert!(!timing.reported);

    // the new realm's spawn scene is the next arrival, timed and reported on its own
    let mut pointers = ScenePointers::default();
    pointers.insert(
        IVec2::ZERO,
        PointerResult::Exists {
            hash: "new".to_owned(),
            urn: None,
        },
    );
    world.insert_resource(pointers);
    let new = scene(&mut world, "new", &[IVec2::ZERO], 2);
    world.run_system_once(report_scene_load_times).unwrap();
    assert_eq!(
        world
            .resource::<FirstRingTiming>()
            .arrival
            .map(|(scene, _)| scene),
        Some(new)
    );

    set_tick(&mut world, new, 5);
    world.run_system_once(report_scene_load_times).unwrap();
    assert!(world.resource::<FirstRingTiming>().reported);
}

#[test]
fn arriving_at_a_scene_that_is_already_ready_reports_nothing() {
    let mut world = world();
    scene(&mut world, "home", &[IVec2::ZERO], 7);

    world.run_system_once(report_scene_load_times).unwrap();

    // nothing to time: the load happened before the player got here
    assert!(world.resource::<FirstRingTiming>().arrival.is_none());
}

#[test]
fn the_first_ring_is_ready_for_the_arrival_that_reaches_the_world() {
    let mut world = world();
    world.init_resource::<Arrivals>();
    let player = world
        .query_filtered::<Entity, With<PrimaryUser>>()
        .single(&world)
        .unwrap();
    world.entity_mut(player).insert(OutOfWorld::default());
    let home = scene(&mut world, "home", &[IVec2::ZERO], 2);
    let neighbour = scene(&mut world, "east", &[IVec2::X], 2);

    // behind the loading screen nothing is ready, even once the ring is
    world.run_system_once(report_scene_load_times).unwrap();
    set_tick(&mut world, home, 6);
    set_tick(&mut world, neighbour, 5);
    world.run_system_once(report_scene_load_times).unwrap();
    assert!(world.resource::<FirstRingTiming>().ready.is_none());

    // the player is let in: arrival 1, its first ring already loaded
    world.entity_mut(player).remove::<OutOfWorld>();
    world.resource_mut::<Arrivals>().0 = 1;
    world.run_system_once(report_scene_load_times).unwrap();
    let ready = world.resource::<FirstRingTiming>().ready.unwrap();
    assert_eq!(ready.arrival, 1);

    // a later frame of the same arrival keeps the first instant
    world.run_system_once(report_scene_load_times).unwrap();
    assert_eq!(
        world.resource::<FirstRingTiming>().ready.unwrap().at,
        ready.at
    );
}

#[test]
fn an_arrival_waits_for_a_loading_neighbour() {
    let mut world = world();
    world.insert_resource(Arrivals(1));
    scene(&mut world, "home", &[IVec2::ZERO], 7);
    let neighbour = scene(&mut world, "east", &[IVec2::X], 2);

    world.run_system_once(report_scene_load_times).unwrap();
    assert!(world.resource::<FirstRingTiming>().ready.is_none());

    set_tick(&mut world, neighbour, 5);
    world.run_system_once(report_scene_load_times).unwrap();
    assert_eq!(
        world
            .resource::<FirstRingTiming>()
            .ready
            .map(|ready| ready.arrival),
        Some(1)
    );
}

#[test]
fn an_arrival_at_an_empty_parcel_is_ready_at_once() {
    let mut world = world();
    world.insert_resource(Arrivals(3));

    world.run_system_once(report_scene_load_times).unwrap();

    assert_eq!(
        world
            .resource::<FirstRingTiming>()
            .ready
            .map(|ready| ready.arrival),
        Some(3)
    );
}

#[test]
fn first_ring_json_is_null_before_an_arrival_is_ready() {
    let mut timing = FirstRingTiming::default();
    let now = Instant::now();
    assert!(first_ring_json(&timing, now).is_null());

    timing.ready = Some(FirstRingReady {
        arrival: 2,
        at: now,
    });
    let reply = first_ring_json(&timing, now + std::time::Duration::from_millis(1500));
    assert_eq!(reply["arrival"], 2);
    assert_eq!(reply["sinceReadyMs"], 1500);
}
