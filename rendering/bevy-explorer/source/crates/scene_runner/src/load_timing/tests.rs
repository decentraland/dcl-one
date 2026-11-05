use bevy::{ecs::system::RunSystemOnce, prelude::*};
use common::structs::PrimaryUser;
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
fn arriving_at_a_scene_that_is_already_ready_reports_nothing() {
    let mut world = world();
    scene(&mut world, "home", &[IVec2::ZERO], 7);

    world.run_system_once(report_scene_load_times).unwrap();

    // nothing to time: the load happened before the player got here
    assert!(world.resource::<FirstRingTiming>().arrival.is_none());
}
