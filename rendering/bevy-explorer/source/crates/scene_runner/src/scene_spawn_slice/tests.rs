use std::time::Duration;

use bevy::{ecs::system::RunSystemOnce, platform::time::Instant, scene::SceneSpawner};
use common::structs::{NoRenderApp, OutOfWorld, PrimaryUser};
use dcl_types::scene_response_channel;

use super::*;
use crate::SceneUpdates;

const SIXTY_FPS: Duration = Duration::from_nanos(1_000_000_000 / 60);

/// The scene loop's state with nothing in flight, targeting `frame_target`.
fn scene_updates(frame_target: Option<Duration>) -> SceneUpdates {
    let (sender, receiver) = scene_response_channel();
    SceneUpdates {
        sender,
        receiver,
        jobs_in_flight: Default::default(),
        update_deadline: web_time::SystemTime::now(),
        eligible_jobs: 0,
        scene_queue: Default::default(),
        loop_end_time: web_time::Instant::now(),
        frame_target,
        priority_scenes: Default::default(),
        frames: 0,
        skipped_in_flight: 0,
        skipped_already_sent: 0,
    }
}

/// A rendering app's world targeting 60 fps, with the player in world.
fn world_with_player() -> (World, Entity) {
    let mut world = World::new();
    world.init_resource::<SceneSpawner>();
    world.init_resource::<ProcessingDeadline>();
    world.insert_resource(scene_updates(Some(SIXTY_FPS)));
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

fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

#[test]
fn the_slice_is_a_quarter_of_the_target_frame() {
    let at = |fps: u64| milliseconds(slice(Some(Duration::from_nanos(1_000_000_000 / fps))));

    assert!((at(60) - 4.17).abs() < 0.01, "{}", at(60));
    assert!((at(144) - 1.74).abs() < 0.01, "{}", at(144));
    assert!((at(30) - 8.33).abs() < 0.01, "{}", at(30));
}

#[test]
fn an_uncapped_frame_rate_gets_the_slice_of_60_fps() {
    assert_eq!(slice(None), slice(Some(SIXTY_FPS)));
}

#[test]
fn processing_keeps_a_quarter_of_the_slice() {
    let slice = slice(Some(SIXTY_FPS));

    assert_eq!(processing_share(slice), slice / 4);
}

#[test]
fn a_rendering_frame_gives_spawning_and_processing_one_slice() {
    let (mut world, _) = world_with_player();

    let before = Instant::now();
    let (spawning, processing) = deadlines_after_a_run(&mut world);
    let after = Instant::now();

    let processing = processing.expect("a processing deadline");
    let slice = slice(Some(SIXTY_FPS));
    assert!(processing >= before + slice);
    assert!(processing <= after + slice);
    assert!(spawning.is_some());
}

#[test]
fn a_faster_target_frame_gets_a_shorter_slice() {
    let (mut world, _) = world_with_player();
    let target = Duration::from_nanos(1_000_000_000 / 144);
    world.insert_resource(scene_updates(Some(target)));

    let before = Instant::now();
    let (_, processing) = deadlines_after_a_run(&mut world);
    let after = Instant::now();

    let processing = processing.expect("a processing deadline");
    assert!(processing >= before + slice(Some(target)));
    assert!(processing <= after + slice(Some(target)));
}

#[test]
fn the_spawner_leaves_the_end_of_the_slice_to_processing() {
    let (mut world, _) = world_with_player();

    let (spawning, processing) = deadlines_after_a_run(&mut world);

    // processing gets time even while the spawner has a queue to fill the slice with
    assert_eq!(
        processing.unwrap() - spawning.unwrap(),
        processing_share(slice(Some(SIXTY_FPS)))
    );
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
