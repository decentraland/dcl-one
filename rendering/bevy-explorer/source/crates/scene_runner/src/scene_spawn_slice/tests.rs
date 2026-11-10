use std::time::Duration;

use bevy::{ecs::system::RunSystemOnce, platform::time::Instant, scene::SceneSpawner};
use common::structs::{NoRenderApp, OutOfWorld, PrimaryUser};

use super::*;

fn world_with_player() -> (World, Entity) {
    let mut world = World::new();
    world.init_resource::<SceneSpawner>();
    world.init_resource::<ProcessingDeadline>();
    world.init_resource::<InstantiationCharges>();
    let player = world.spawn(PrimaryUser::default()).id();
    (world, player)
}

/// The spawner's deadline and processing's, after a run.
fn deadlines_after_a_run(world: &mut World) -> (Option<Instant>, Option<Instant>) {
    world.run_system_once(slice_scene_spawning).unwrap();
    (
        world.resource::<SceneSpawner>().spawn_deadline(),
        world.resource::<ProcessingDeadline>().0,
    )
}

#[test]
fn a_rendering_frame_gives_spawning_and_processing_one_slice() {
    let (mut world, _) = world_with_player();

    let before = Instant::now();
    let (spawning, processing) = deadlines_after_a_run(&mut world);
    let after = Instant::now();

    let processing = processing.expect("a processing deadline");
    assert!(processing >= before + SCENE_SPAWN_SLICE);
    assert!(processing <= after + SCENE_SPAWN_SLICE);
    assert!(spawning.is_some());
}

#[test]
fn the_spawner_leaves_the_end_of_the_slice_to_processing() {
    let (mut world, _) = world_with_player();

    let (spawning, processing) = deadlines_after_a_run(&mut world);

    // processing gets time even while the spawner has a queue to fill the slice with
    assert_eq!(processing.unwrap() - spawning.unwrap(), PROCESSING_SHARE);
}

#[test]
fn behind_the_loading_backdrop_scenes_spawn_and_process_whole() {
    let (mut world, player) = world_with_player();
    world.entity_mut(player).insert(OutOfWorld);

    assert_eq!(deadlines_after_a_run(&mut world), (None, None));
}

#[test]
fn a_headless_app_spawns_and_processes_whole() {
    let (mut world, _) = world_with_player();
    world.insert_resource(NoRenderApp);

    assert_eq!(deadlines_after_a_run(&mut world), (None, None));
}

#[test]
fn arriving_back_in_world_slices_again() {
    let (mut world, player) = world_with_player();
    world.entity_mut(player).insert(OutOfWorld);
    deadlines_after_a_run(&mut world);

    world.entity_mut(player).remove::<OutOfWorld>();

    let (spawning, processing) = deadlines_after_a_run(&mut world);
    assert!(spawning.is_some() && processing.is_some());
}

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

/// One run's admissions of `costs`, in order, after `charged` was spent this frame.
fn admissions(world: &mut World, charged: Duration, costs: &[Duration]) -> Vec<bool> {
    world.resource_mut::<InstantiationCharges>().0 = charged;
    let costs = costs.to_vec();
    world
        .run_system_once(move |mut budget: InstantiationBudget| {
            let mut gate = budget.gate();
            costs.iter().map(|cost| gate.admit(*cost)).collect::<Vec<_>>()
        })
        .unwrap()
}

fn charges(world: &World) -> Duration {
    world.resource::<InstantiationCharges>().0
}

#[test]
fn the_first_item_of_a_run_is_admitted_whatever_was_spent() {
    let (mut world, _) = world_with_player();

    let admitted = admissions(&mut world, ms(10), &[ms(1), Duration::from_micros(1)]);

    assert_eq!(admitted, [true, false]);
}

#[test]
fn items_are_admitted_until_the_slice_is_charged() {
    let (mut world, _) = world_with_player();

    let admitted = admissions(&mut world, Duration::ZERO, &[ms(1); 5]);

    assert_eq!(admitted, [true, true, true, true, false]);
    assert_eq!(charges(&world), SCENE_SPAWN_SLICE);
}

#[test]
fn an_item_over_the_rest_of_the_slice_waits() {
    let (mut world, _) = world_with_player();

    let admitted = admissions(&mut world, Duration::ZERO, &[ms(3), ms(2)]);

    assert_eq!(admitted, [true, false]);
    assert_eq!(charges(&world), ms(3));
}

#[test]
fn behind_the_loading_backdrop_everything_is_admitted_uncharged() {
    let (mut world, player) = world_with_player();
    world.entity_mut(player).insert(OutOfWorld);

    let admitted = admissions(&mut world, Duration::ZERO, &[ms(1); 1_000]);

    assert!(admitted.iter().all(|admitted| *admitted));
    assert_eq!(charges(&world), Duration::ZERO);
}

#[test]
fn a_headless_app_admits_everything_uncharged() {
    let (mut world, _) = world_with_player();
    world.insert_resource(NoRenderApp);

    let admitted = admissions(&mut world, Duration::ZERO, &[ms(1); 1_000]);

    assert!(admitted.iter().all(|admitted| *admitted));
    assert_eq!(charges(&world), Duration::ZERO);
}

#[test]
fn the_slice_ends_earlier_by_what_was_charged() {
    let (mut world, _) = world_with_player();
    world.resource_mut::<InstantiationCharges>().0 = ms(3);

    let before = Instant::now();
    let (spawning, processing) = deadlines_after_a_run(&mut world);
    let after = Instant::now();

    let processing = processing.expect("a processing deadline");
    assert!(processing >= before + ms(1));
    assert!(processing <= after + ms(1));
    assert_eq!(processing - spawning.unwrap(), PROCESSING_SHARE);
    // the next frame's charges start from nothing
    assert_eq!(charges(&world), Duration::ZERO);
}

#[test]
fn charges_past_the_slice_leave_an_expired_slice() {
    let (mut world, _) = world_with_player();
    world.resource_mut::<InstantiationCharges>().0 = ms(6);

    let (_, processing) = deadlines_after_a_run(&mut world);
    let after = Instant::now();

    assert!(processing.expect("a processing deadline") <= after);
}
