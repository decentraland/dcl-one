//! A benchmark, not a test: how long the spawner takes to write a large scene whole, and in
//! 4 ms slices. Run it in release:
//!
//! `cargo test -p bevy_scene --features bevy_render --release --lib -- spawn_bench --ignored --nocapture`

use core::time::Duration;

use bevy_app::App;
use bevy_asset::{AssetPlugin, Assets};
use bevy_ecs::{component::Component, hierarchy::ChildOf, prelude::ReflectComponent, world::World};
use bevy_platform::time::Instant;
use bevy_reflect::Reflect;

use super::{InstanceId, SceneSpawner};
use crate::{Scene, ScenePlugin};

#[derive(Component, Reflect, Default)]
#[reflect(Component)]
struct Node(u32);

#[derive(Component, Reflect, Default)]
#[reflect(Component)]
struct Payload([f32; 16]);

const GROUPS: u32 = 50;
const LEAVES: u32 = 100;
const ENTITIES: u32 = 1 + GROUPS + GROUPS * LEAVES;
const SLICE: Duration = Duration::from_millis(4);
const REPEATS: usize = 7;

/// A root with `GROUPS` children of `LEAVES` children each, every node with two components.
fn large_scene() -> Scene {
    let mut world = World::new();
    let root = world.spawn((Node(0), Payload::default())).id();
    for group in 0..GROUPS {
        let group = world
            .spawn((Node(1 + group), Payload::default(), ChildOf(root)))
            .id();
        for leaf in 0..LEAVES {
            world.spawn((Node(leaf), Payload::default(), ChildOf(group)));
        }
    }
    Scene::new(world)
}

fn spawned(app: &App, instance: InstanceId) -> bool {
    app.world()
        .resource::<SceneSpawner>()
        .instance_is_ready(instance)
}

/// One spawn of the scene under a new parent: the time each run of the spawner took.
fn spawn_once(app: &mut App, sliced: bool) -> Vec<Duration> {
    let scene = app.world().resource::<SceneHandle>().0.clone();
    let parent = app.world_mut().spawn_empty().id();
    let instance = app
        .world_mut()
        .resource_mut::<SceneSpawner>()
        .spawn_as_child(scene, parent);
    let mut runs = Vec::new();
    while !spawned(app, instance) {
        let started = Instant::now();
        if sliced {
            app.world_mut()
                .resource_mut::<SceneSpawner>()
                .set_spawn_deadline(Some(started + SLICE));
        }
        app.update();
        runs.push(started.elapsed());
    }
    app.world_mut().entity_mut(parent).despawn();
    app.update();
    runs
}

#[derive(bevy_ecs::resource::Resource)]
struct SceneHandle(bevy_asset::Handle<Scene>);

fn bench_app() -> App {
    let mut app = App::new();
    app.add_plugins((AssetPlugin::default(), ScenePlugin))
        .register_type::<Node>()
        .register_type::<Payload>();
    let scene = app
        .world_mut()
        .resource_mut::<Assets<Scene>>()
        .add(large_scene());
    app.insert_resource(SceneHandle(scene));
    app
}

fn median<T: Ord + Copy>(mut values: Vec<T>) -> T {
    values.sort();
    values[values.len() / 2]
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

#[test]
#[ignore = "benchmark: run in release with --ignored --nocapture"]
fn spawn_bench() {
    for sliced in [false, true] {
        let mut app = bench_app();
        // the first spawn creates the archetypes; later ones reuse them, as glTFs do
        spawn_once(&mut app, sliced);
        let spawns: Vec<Vec<Duration>> =
            (0..REPEATS).map(|_| spawn_once(&mut app, sliced)).collect();
        let total = median(spawns.iter().map(|runs| runs.iter().sum()).collect());
        let longest = median(
            spawns
                .iter()
                .map(|runs| *runs.iter().max().unwrap())
                .collect(),
        );
        let runs = median(spawns.iter().map(Vec::len).collect());
        println!(
            "BENCH {} {ENTITIES} entities: total {:.2} ms ({:.2} us/entity), {runs} runs, longest run {:.2} ms",
            if sliced { "sliced 4 ms" } else { "whole" },
            ms(total),
            total.as_secs_f64() * 1e6 / f64::from(ENTITIES),
            ms(longest),
        );
    }
}
