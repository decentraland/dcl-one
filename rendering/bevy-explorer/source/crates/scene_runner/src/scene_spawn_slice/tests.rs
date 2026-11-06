use bevy::{ecs::system::RunSystemOnce, platform::time::Instant, scene::SceneSpawner};
use common::structs::{NoRenderApp, OutOfWorld, PrimaryUser};

use super::*;

fn world_with_player() -> (World, Entity) {
    let mut world = World::new();
    world.init_resource::<SceneSpawner>();
    let player = world.spawn(PrimaryUser::default()).id();
    (world, player)
}

fn deadline_after_a_run(world: &mut World) -> Option<Instant> {
    world.run_system_once(slice_scene_spawning).unwrap();
    world.resource::<SceneSpawner>().spawn_deadline()
}

#[test]
fn a_rendering_frame_gives_the_spawner_its_slice() {
    let (mut world, _) = world_with_player();

    let before = Instant::now();
    let deadline = deadline_after_a_run(&mut world).expect("a deadline");
    let after = Instant::now();

    assert!(deadline >= before + SCENE_SPAWN_SLICE);
    assert!(deadline <= after + SCENE_SPAWN_SLICE);
}

#[test]
fn behind_the_loading_backdrop_scenes_spawn_whole() {
    let (mut world, player) = world_with_player();
    world.entity_mut(player).insert(OutOfWorld);

    assert_eq!(deadline_after_a_run(&mut world), None);
}

#[test]
fn a_headless_app_spawns_scenes_whole() {
    let (mut world, _) = world_with_player();
    world.insert_resource(NoRenderApp);

    assert_eq!(deadline_after_a_run(&mut world), None);
}

#[test]
fn arriving_back_in_world_slices_again() {
    let (mut world, player) = world_with_player();
    world.entity_mut(player).insert(OutOfWorld);
    deadline_after_a_run(&mut world);

    world.entity_mut(player).remove::<OutOfWorld>();

    assert!(deadline_after_a_run(&mut world).is_some());
}
