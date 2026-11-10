use bevy::{ecs::system::RunSystemOnce, platform::time::Instant, scene::SceneSpawner};
use common::structs::{NoRenderApp, OutOfWorld, PrimaryUser};

use super::*;

fn world_with_player() -> (World, Entity) {
    let mut world = World::new();
    world.init_resource::<SceneSpawner>();
    world.init_resource::<ProcessingDeadline>();
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
