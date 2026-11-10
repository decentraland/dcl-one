use bevy_app::App;
use bevy_asset::{AssetPlugin, Assets, Handle};
use bevy_ecs::{
    component::Component,
    entity::Entity,
    hierarchy::{ChildOf, Children},
    observer::Trigger,
    prelude::ReflectComponent,
    query::Has,
    reflect::AppTypeRegistry,
    resource::Resource,
    system::{Query, ResMut},
    world::{OnAdd, World},
};
use bevy_platform::time::Instant;
use bevy_reflect::Reflect;

use super::{InstanceId, SceneInstanceReady, SceneSpawner};
use crate::{DynamicScene, DynamicSceneBuilder, Scene, ScenePlugin};

#[derive(Component, Reflect, Default, Debug, PartialEq)]
#[reflect(Component)]
struct Node(u32);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((AssetPlugin::default(), ScenePlugin))
        .register_type::<Node>();
    app
}

fn add_scene(app: &mut App, build: impl FnOnce(&mut World)) -> Handle<Scene> {
    let mut world = World::new();
    build(&mut world);
    app.world_mut()
        .resource_mut::<Assets<Scene>>()
        .add(Scene::new(world))
}

/// A root `Node(first)` with children `Node(first + 1)`, … (`len` nodes in all).
fn tree(first: u32, len: u32) -> impl FnOnce(&mut World) {
    move |world| {
        let root = world.spawn(Node(first)).id();
        for n in first + 1..first + len {
            world.spawn((Node(n), ChildOf(root)));
        }
    }
}

/// `Node(first)` with child `Node(first + 1)`, its child `Node(first + 2)`, … (`len` nodes). The
/// scene's archetype order puts the last node first: it is written before its parent unless
/// writing follows the hierarchy.
fn chain(first: u32, len: u32) -> impl FnOnce(&mut World) {
    move |world| {
        let mut parent = world.spawn(Node(first)).id();
        for n in first + 1..first + len {
            parent = world.spawn((Node(n), ChildOf(parent))).id();
        }
    }
}

/// The nodes in the order the spawner writes them, and whether each was attached to a parent by
/// then.
#[derive(Resource, Default)]
struct Written(Vec<(u32, bool)>);

fn record_writes(app: &mut App) {
    app.init_resource::<Written>().add_observer(
        |trigger: Trigger<OnAdd, Node>,
         nodes: Query<(&Node, Has<ChildOf>)>,
         mut written: ResMut<Written>| {
            let (node, attached) = nodes.get(trigger.target()).unwrap();
            written.0.push((node.0, attached));
        },
    );
}

fn written(app: &App) -> &[(u32, bool)] {
    &app.world().resource::<Written>().0
}

fn spawner(app: &mut App) -> bevy_ecs::world::Mut<'_, SceneSpawner> {
    app.world_mut().resource_mut::<SceneSpawner>()
}

/// From now on every run of the spawner starts past its deadline, so it writes one entity.
fn one_entity_per_run(app: &mut App) {
    spawner(app).set_spawn_deadline(Some(Instant::now()));
}

fn nodes(app: &mut App) -> Vec<u32> {
    let mut nodes: Vec<u32> = app
        .world_mut()
        .query::<&Node>()
        .iter(app.world())
        .map(|node| node.0)
        .collect();
    nodes.sort();
    nodes
}

fn is_ready(app: &App, instance: InstanceId) -> bool {
    app.world()
        .resource::<SceneSpawner>()
        .instance_is_ready(instance)
}

fn children(app: &App, entity: Entity) -> Vec<Entity> {
    app.world()
        .get::<Children>(entity)
        .map(|children| children.to_vec())
        .unwrap_or_default()
}

/// Whether the spawner has stopped tracking `instance`'s parent.
fn parent_is_forgotten(app: &App, instance: InstanceId) -> bool {
    let spawner = app.world().resource::<SceneSpawner>();
    !spawner.instance_parents.contains_key(&instance)
        && spawner
            .scenes_with_parent
            .iter()
            .all(|(id, _)| *id != instance)
}

#[test]
fn without_a_deadline_an_instance_spawns_whole() {
    let mut app = app();
    let scene = add_scene(&mut app, tree(0, 3));
    let parent = app.world_mut().spawn_empty().id();
    let instance = spawner(&mut app).spawn_as_child(scene, parent);

    app.update();

    assert!(is_ready(&app, instance));
    assert_eq!(nodes(&mut app), [0, 1, 2]);
}

#[test]
fn past_its_deadline_the_spawner_writes_one_entity_per_run() {
    let mut app = app();
    one_entity_per_run(&mut app);
    let scene = add_scene(&mut app, tree(0, 3));
    let parent = app.world_mut().spawn_empty().id();
    let instance = spawner(&mut app).spawn_as_child(scene, parent);

    app.update();
    assert!(!is_ready(&app, instance));
    assert_eq!(nodes(&mut app).len(), 1);

    app.update();
    assert!(!is_ready(&app, instance));
    assert_eq!(nodes(&mut app).len(), 2);

    app.update();
    assert!(is_ready(&app, instance));
    assert_eq!(nodes(&mut app), [0, 1, 2]);
}

#[test]
fn a_sliced_instance_keeps_its_hierarchy_and_child_order() {
    let mut app = app();
    one_entity_per_run(&mut app);
    let scene = add_scene(&mut app, tree(0, 3));
    let parent = app.world_mut().spawn_empty().id();
    let instance = spawner(&mut app).spawn_as_child(scene, parent);

    while !is_ready(&app, instance) {
        app.update();
    }

    let [root] = children(&app, parent)[..] else {
        panic!("one root under the parent");
    };
    assert_eq!(app.world().get::<Node>(root), Some(&Node(0)));
    let below: Vec<_> = children(&app, root)
        .into_iter()
        .map(|child| app.world().get::<Node>(child).unwrap().0)
        .collect();
    assert_eq!(below, [1, 2]);
}

#[test]
fn a_sliced_instance_writes_parents_before_children() {
    let mut app = app();
    record_writes(&mut app);
    one_entity_per_run(&mut app);
    let scene = add_scene(&mut app, chain(0, 3));
    let parent = app.world_mut().spawn_empty().id();
    let instance = spawner(&mut app).spawn_as_child(scene, parent);

    while !is_ready(&app, instance) {
        app.update();
    }

    let order: Vec<_> = written(&app).iter().map(|(node, _)| *node).collect();
    assert_eq!(order, [0, 1, 2]);
}

#[test]
fn without_a_deadline_roots_are_attached_once_written_as_stock_does() {
    let mut app = app();
    record_writes(&mut app);
    let scene = add_scene(&mut app, tree(0, 3));
    let parent = app.world_mut().spawn_empty().id();
    let instance = spawner(&mut app).spawn_as_child(scene, parent);

    app.update();

    assert!(is_ready(&app, instance));
    assert!(written(&app).contains(&(0, false)));
    assert_eq!(children(&app, parent).len(), 1);
}

#[test]
fn an_unwritten_entity_despawned_by_others_mid_spawn_stays_gone_with_its_subtree() {
    let mut app = app();
    one_entity_per_run(&mut app);
    let scene = add_scene(&mut app, chain(0, 3));
    let parent = app.world_mut().spawn_empty().id();
    let instance = spawner(&mut app).spawn_as_child(scene.clone(), parent);
    app.update();
    // the instance's entity for `Node(1)`, not written yet
    let source = app
        .world()
        .resource::<Assets<Scene>>()
        .get(&scene)
        .unwrap()
        .world
        .iter_entities()
        .find(|entity| entity.get::<Node>() == Some(&Node(1)))
        .unwrap()
        .id();
    let unwritten = app
        .world()
        .resource::<SceneSpawner>()
        .scene_in_progress
        .as_ref()
        .expect("in progress")
        .entity_map[&source];

    app.world_mut().entity_mut(unwritten).despawn();
    for _ in 0..5 {
        app.update();
    }

    // as a whole spawn would be with that entity despawned afterwards: no new one in its place,
    // and no dead id among the root's children
    assert!(is_ready(&app, instance));
    assert_eq!(nodes(&mut app), [0]);
    // its only child gone, the root has no `Children` left, as Bevy's own hooks would leave it
    let root = children(&app, parent)[0];
    assert!(app.world().get::<Children>(root).is_none());
}

#[test]
fn a_spawner_can_spawn_into_another_world() {
    let mut first = app();
    let mut second = app();
    let first_scene = add_scene(&mut first, tree(0, 2));
    let second_scene = add_scene(&mut second, tree(10, 2));
    let mut spawner = SceneSpawner::default();
    spawner.spawn(first_scene);
    spawner.spawn_queued_scenes(first.world_mut()).unwrap();

    spawner.set_spawn_deadline(Some(Instant::now()));
    let instance = spawner.spawn(second_scene);
    for _ in 0..3 {
        spawner.spawn_queued_scenes(second.world_mut()).unwrap();
    }

    assert!(spawner.instance_is_ready(instance));
}

#[test]
#[should_panic(expected = "part-written in another World")]
fn a_spawner_will_not_finish_an_instance_in_another_world() {
    let mut first = app();
    let mut second = app();
    let scene = add_scene(&mut first, tree(0, 3));
    let mut spawner = SceneSpawner::default();
    spawner.set_spawn_deadline(Some(Instant::now()));
    spawner.spawn(scene);
    spawner.spawn_queued_scenes(first.world_mut()).unwrap();

    // its entity ids mean nothing there
    let _ = spawner.spawn_queued_scenes(second.world_mut());
}

#[test]
fn a_sliced_instance_is_attached_from_its_first_run() {
    let mut app = app();
    one_entity_per_run(&mut app);
    let scene = add_scene(&mut app, tree(0, 3));
    let parent = app.world_mut().spawn_empty().id();
    let instance = spawner(&mut app).spawn_as_child(scene, parent);

    app.update();

    assert!(!is_ready(&app, instance));
    assert_eq!(children(&app, parent).len(), 1);
}

#[test]
fn instances_spawn_one_at_a_time_in_request_order() {
    let mut app = app();
    one_entity_per_run(&mut app);
    let first = add_scene(&mut app, tree(0, 2));
    let second = add_scene(&mut app, tree(10, 1));
    let parent = app.world_mut().spawn_empty().id();
    let first = spawner(&mut app).spawn_as_child(first, parent);
    let second = spawner(&mut app).spawn_as_child(second, parent);

    app.update();
    app.update();
    assert!(is_ready(&app, first));
    assert!(!is_ready(&app, second));

    app.update();
    assert!(is_ready(&app, second));
}

#[derive(Resource, Default)]
struct ReadyEvents(Vec<InstanceId>);

#[test]
fn the_ready_event_fires_once_when_the_instance_is_complete() {
    let mut app = app();
    app.init_resource::<ReadyEvents>();
    app.add_observer(
        |trigger: Trigger<SceneInstanceReady>, mut events: ResMut<ReadyEvents>| {
            events.0.push(trigger.event().instance_id);
        },
    );
    one_entity_per_run(&mut app);
    let scene = add_scene(&mut app, tree(0, 2));
    let parent = app.world_mut().spawn_empty().id();
    let instance = spawner(&mut app).spawn_as_child(scene, parent);

    app.update();
    assert!(app.world().resource::<ReadyEvents>().0.is_empty());

    app.update();
    app.update();
    assert_eq!(app.world().resource::<ReadyEvents>().0, [instance]);
}

#[test]
fn despawning_the_parent_mid_spawn_leaves_nothing_behind() {
    let mut app = app();
    one_entity_per_run(&mut app);
    let scene = add_scene(&mut app, tree(0, 3));
    let entities_before = app.world().entities().len();
    let parent = app.world_mut().spawn_empty().id();
    let instance = spawner(&mut app).spawn_as_child(scene, parent);
    app.update();

    app.world_mut().entity_mut(parent).despawn();
    app.update();

    assert!(!is_ready(&app, instance));
    assert_eq!(app.world().entities().len(), entities_before);
    assert!(parent_is_forgotten(&app, instance));
}

#[test]
fn despawning_an_instance_mid_spawn_leaves_nothing_behind() {
    let mut app = app();
    one_entity_per_run(&mut app);
    let scene = add_scene(&mut app, tree(0, 3));
    let parent = app.world_mut().spawn_empty().id();
    let entities_before = app.world().entities().len();
    let instance = spawner(&mut app).spawn_as_child(scene, parent);
    app.update();

    spawner(&mut app).despawn_instance(instance);
    app.update();
    app.update();

    assert!(!is_ready(&app, instance));
    assert_eq!(app.world().entities().len(), entities_before);
    assert!(children(&app, parent).is_empty());
    assert!(parent_is_forgotten(&app, instance));
}

#[test]
fn unregistering_an_instance_mid_spawn_leaves_nothing_behind() {
    let mut app = app();
    one_entity_per_run(&mut app);
    let scene = add_scene(&mut app, tree(0, 3));
    let parent = app.world_mut().spawn_empty().id();
    let entities_before = app.world().entities().len();
    let instance = spawner(&mut app).spawn_as_child(scene, parent);
    app.update();

    spawner(&mut app).unregister_instance(instance);
    app.update();
    app.update();
    app.update();

    assert!(!is_ready(&app, instance));
    assert_eq!(app.world().entities().len(), entities_before);
    assert!(nodes(&mut app).is_empty());
    assert!(parent_is_forgotten(&app, instance));
}

#[test]
fn despawning_an_instance_queued_behind_another_cancels_it() {
    let mut app = app();
    one_entity_per_run(&mut app);
    let first = add_scene(&mut app, tree(0, 3));
    let queued = add_scene(&mut app, tree(10, 2));
    let parent = app.world_mut().spawn_empty().id();
    let first = spawner(&mut app).spawn_as_child(first, parent);
    let queued = spawner(&mut app).spawn_as_child(queued, parent);
    app.update();

    spawner(&mut app).despawn_instance(queued);
    while !is_ready(&app, first) {
        app.update();
    }
    app.update();
    app.update();

    assert!(!is_ready(&app, queued));
    assert_eq!(nodes(&mut app), [0, 1, 2]);
    assert!(parent_is_forgotten(&app, queued));
}

#[test]
fn despawning_the_root_mid_spawn_completes_the_instance_without_it() {
    let mut app = app();
    app.init_resource::<ReadyEvents>();
    app.add_observer(
        |trigger: Trigger<SceneInstanceReady>, mut events: ResMut<ReadyEvents>| {
            events.0.push(trigger.event().instance_id);
        },
    );
    one_entity_per_run(&mut app);
    let scene = add_scene(&mut app, chain(0, 3));
    let parent = app.world_mut().spawn_empty().id();
    let entities_before = app.world().entities().len();
    let instance = spawner(&mut app).spawn_as_child(scene, parent);
    app.update();
    let root = children(&app, parent)[0];

    app.world_mut().entity_mut(root).despawn();
    for _ in 0..5 {
        app.update();
    }

    // as a whole spawn whose root was despawned afterwards: ready, with nothing left
    assert!(is_ready(&app, instance));
    assert_eq!(app.world().resource::<ReadyEvents>().0, [instance]);
    assert!(nodes(&mut app).is_empty());
    assert_eq!(app.world().entities().len(), entities_before);
}

/// A component the type registry does not know: writing it fails.
#[derive(Component)]
struct Unregistered;

#[test]
fn an_error_mid_spawn_leaves_nothing_behind() {
    let mut app = app();
    one_entity_per_run(&mut app);
    let scene = add_scene(&mut app, |world: &mut World| {
        let root = world.spawn(Node(0)).id();
        world.spawn((Node(1), Unregistered, ChildOf(root)));
    });
    let parent = app.world_mut().spawn_empty().id();
    let entities_before = app.world().entities().len();
    let instance = spawner(&mut app).spawn_as_child(scene, parent);
    let spawn = |app: &mut App| {
        app.world_mut()
            .resource_scope(|world, mut spawner: bevy_ecs::world::Mut<SceneSpawner>| {
                spawner.spawn_queued_scenes(world)
            })
    };
    // the root, then its child, which fails
    spawn(&mut app).unwrap();
    assert!(spawn(&mut app).is_err());

    assert!(!is_ready(&app, instance));
    assert_eq!(app.world().entities().len(), entities_before);
    assert!(parent_is_forgotten(&app, instance));
}

#[test]
fn lifting_the_deadline_mid_spawn_finishes_the_instance_in_one_run() {
    let mut app = app();
    one_entity_per_run(&mut app);
    let scene = add_scene(&mut app, chain(0, 4));
    let parent = app.world_mut().spawn_empty().id();
    let instance = spawner(&mut app).spawn_as_child(scene, parent);
    app.update();
    assert!(!is_ready(&app, instance));

    spawner(&mut app).set_spawn_deadline(None);
    app.update();

    assert!(is_ready(&app, instance));
    assert_eq!(nodes(&mut app), [0, 1, 2, 3]);
}

/// [`tree`] `(0, 3)`, but the children name their parent and the parent lists none of them.
fn unlisted_children_scene(app: &mut App) -> Handle<Scene> {
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let mut world = World::new();
    world.insert_resource(registry.clone());
    tree(0, 3)(&mut world);
    let entities: Vec<Entity> = world.iter_entities().map(|entity| entity.id()).collect();
    let dynamic = DynamicSceneBuilder::from_world(&world)
        .deny_component::<Children>()
        .extract_entities(entities.into_iter())
        .build();
    let scene = Scene::from_dynamic_scene(&dynamic, &registry).unwrap();
    app.world_mut().resource_mut::<Assets<Scene>>().add(scene)
}

#[test]
fn entities_whose_parent_is_outside_the_scene_are_all_written() {
    let mut app = app();
    one_entity_per_run(&mut app);
    // siblings extracted without the parent they name
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let mut world = World::new();
    world.insert_resource(registry.clone());
    let outside = world.spawn(Node(0)).id();
    let siblings = [
        world.spawn((Node(1), ChildOf(outside))).id(),
        world.spawn((Node(2), ChildOf(outside))).id(),
    ];
    let dynamic = DynamicSceneBuilder::from_world(&world)
        .extract_entities(siblings.into_iter())
        .build();
    let scene = Scene::from_dynamic_scene(&dynamic, &registry).unwrap();
    let scene = app.world_mut().resource_mut::<Assets<Scene>>().add(scene);
    let parent = app.world_mut().spawn_empty().id();
    let instance = spawner(&mut app).spawn_as_child(scene, parent);

    for _ in 0..5 {
        app.update();
    }

    // as a whole spawn writes them: the entity mapper's placeholder for that parent is no
    // despawned parent
    assert!(is_ready(&app, instance));
    assert_eq!(nodes(&mut app), [1, 2]);
}

#[test]
fn a_ready_entity_whose_parent_is_despawned_before_it_is_written_goes_too() {
    let mut app = app();
    one_entity_per_run(&mut app);
    let scene = unlisted_children_scene(&mut app);
    let parent = app.world_mut().spawn_empty().id();
    let instance = spawner(&mut app).spawn_as_child(scene, parent);
    // the root; then both children are made ready, since the root does not list them, and one
    // is written
    app.update();
    app.update();
    let root = children(&app, parent)[0];

    app.world_mut().entity_mut(root).despawn();
    for _ in 0..5 {
        app.update();
    }

    // the child written before the root went stays, as with a whole spawn; the other is never
    // written under a parent that is gone
    assert!(is_ready(&app, instance));
    assert_eq!(nodes(&mut app).len(), 1);
}

#[test]
fn a_scene_whose_hierarchy_misses_entities_is_still_written_in_full_in_slices() {
    let mut app = app();
    record_writes(&mut app);
    one_entity_per_run(&mut app);
    let scene = unlisted_children_scene(&mut app);
    let parent = app.world_mut().spawn_empty().id();
    let instance = spawner(&mut app).spawn_as_child(scene, parent);

    for _ in 0..10 {
        app.update();
    }

    assert!(is_ready(&app, instance));
    assert_eq!(nodes(&mut app), [0, 1, 2]);
    // each entity written once, one a run: nothing started over or was written whole
    assert_eq!(written(&app).len(), 3);
}

#[test]
fn despawning_a_written_entity_mid_spawn_completes_the_instance_without_its_subtree() {
    let mut app = app();
    record_writes(&mut app);
    one_entity_per_run(&mut app);
    let scene = add_scene(&mut app, chain(0, 4));
    let parent = app.world_mut().spawn_empty().id();
    let instance = spawner(&mut app).spawn_as_child(scene, parent);
    app.update();
    app.update();
    let root = children(&app, parent)[0];
    let written_child = children(&app, root)[0];

    app.world_mut().entity_mut(written_child).despawn();
    for _ in 0..5 {
        app.update();
    }

    // as a whole spawn would be with that entity despawned afterwards
    assert!(is_ready(&app, instance));
    assert_eq!(nodes(&mut app), [0]);
    assert_eq!(written(&app).len(), 2);
}

#[test]
fn an_error_in_a_whole_spawn_leaves_nothing_behind() {
    let mut app = app();
    let scene = add_scene(&mut app, |world: &mut World| {
        let root = world.spawn(Node(0)).id();
        world.spawn((Node(1), Unregistered, ChildOf(root)));
    });
    let parent = app.world_mut().spawn_empty().id();
    let entities_before = app.world().entities().len();
    let instance = spawner(&mut app).spawn_as_child(scene, parent);

    let result =
        app.world_mut()
            .resource_scope(|world, mut spawner: bevy_ecs::world::Mut<SceneSpawner>| {
                spawner.spawn_queued_scenes(world)
            });

    assert!(result.is_err());
    assert!(!is_ready(&app, instance));
    assert_eq!(app.world().entities().len(), entities_before);
    assert!(parent_is_forgotten(&app, instance));
}

#[test]
fn the_spawner_reports_what_is_queued_and_what_is_being_written() {
    let mut app = app();
    one_entity_per_run(&mut app);
    let first = add_scene(&mut app, tree(0, 3));
    let second = add_scene(&mut app, tree(10, 2));
    let parent = app.world_mut().spawn_empty().id();
    spawner(&mut app).spawn_as_child(first, parent);
    spawner(&mut app).spawn_as_child(second, parent);

    app.update();

    let progress = app.world().resource::<SceneSpawner>().spawn_progress();
    assert_eq!(progress.queued, 1);
    assert_eq!(progress.writing, Some((1, 3)));
}

#[test]
fn an_instance_unregistered_and_replaced_in_one_frame_leaves_only_the_new_one() {
    let mut app = app();
    one_entity_per_run(&mut app);
    let old = add_scene(&mut app, tree(0, 3));
    let new = add_scene(&mut app, tree(10, 2));
    let parent = app.world_mut().spawn_empty().id();
    let old = spawner(&mut app).spawn_as_child(old, parent);
    app.update();

    spawner(&mut app).unregister_instance(old);
    let new = spawner(&mut app).spawn_as_child(new, parent);
    while !is_ready(&app, new) {
        app.update();
    }

    assert!(!is_ready(&app, old));
    assert_eq!(nodes(&mut app), [10, 11]);
    assert_eq!(children(&app, parent).len(), 1);
}

#[test]
fn a_scene_changed_mid_spawn_starts_its_instance_over() {
    let mut app = app();
    one_entity_per_run(&mut app);
    let scene = add_scene(&mut app, tree(0, 3));
    let parent = app.world_mut().spawn_empty().id();
    let instance = spawner(&mut app).spawn_as_child(scene.clone(), parent);
    app.update();

    let mut changed = World::new();
    tree(10, 2)(&mut changed);
    app.world_mut()
        .resource_mut::<Assets<Scene>>()
        .insert(scene.id(), Scene::new(changed));
    while !is_ready(&app, instance) {
        app.update();
    }

    assert_eq!(nodes(&mut app), [10, 11]);
    assert_eq!(children(&app, parent).len(), 1);
}

#[test]
fn a_scene_removed_mid_spawn_waits_for_it_to_return() {
    let mut app = app();
    one_entity_per_run(&mut app);
    let scene = add_scene(&mut app, tree(0, 3));
    let parent = app.world_mut().spawn_empty().id();
    let instance = spawner(&mut app).spawn_as_child(scene.clone(), parent);
    app.update();

    app.world_mut()
        .resource_mut::<Assets<Scene>>()
        .remove(scene.id());
    app.update();
    app.update();
    assert!(!is_ready(&app, instance));
    assert!(nodes(&mut app).is_empty());

    let mut back = World::new();
    tree(0, 3)(&mut back);
    app.world_mut()
        .resource_mut::<Assets<Scene>>()
        .insert(scene.id(), Scene::new(back));
    while !is_ready(&app, instance) {
        app.update();
    }
    assert_eq!(nodes(&mut app), [0, 1, 2]);
}

#[test]
fn dynamic_scenes_still_spawn_whole() {
    let mut app = app();
    one_entity_per_run(&mut app);
    let mut world = World::new();
    world.insert_resource(
        app.world()
            .resource::<bevy_ecs::reflect::AppTypeRegistry>()
            .clone(),
    );
    tree(0, 3)(&mut world);
    let scene = app
        .world_mut()
        .resource_mut::<Assets<DynamicScene>>()
        .add(DynamicScene::from_world(&world));
    let instance = spawner(&mut app).spawn_dynamic(scene);

    app.update();

    assert!(is_ready(&app, instance));
    assert_eq!(nodes(&mut app), [0, 1, 2]);
}

#[cfg(feature = "bevy_render")]
mod visibility {
    use bevy_app::TaskPoolPlugin;
    use bevy_asset::AssetApp;
    use bevy_render::{
        mesh::Mesh,
        view::visibility::{InheritedVisibility, ViewVisibility, Visibility, VisibilityPlugin},
    };

    use super::*;

    fn app() -> App {
        let mut app = super::app();
        app.register_type::<Visibility>()
            .register_type::<InheritedVisibility>()
            .register_type::<ViewVisibility>();
        app
    }

    /// With Bevy's visibility propagation, which resolves a child whose parent has no
    /// visibility yet as visible.
    fn propagating_app() -> App {
        let mut app = app();
        app.add_plugins((TaskPoolPlugin::default(), VisibilityPlugin))
            .init_asset::<Mesh>();
        app
    }

    /// [`chain`], every node `Visibility::Inherited`.
    fn shown_chain(len: u32) -> impl FnOnce(&mut World) {
        move |world| {
            let mut parent = world.spawn((Node(0), Visibility::Inherited)).id();
            for n in 1..len {
                parent = world
                    .spawn((Node(n), Visibility::Inherited, ChildOf(parent)))
                    .id();
            }
        }
    }

    /// The nodes visibility propagation resolved as visible.
    fn shown_nodes(app: &mut App) -> Vec<u32> {
        let mut shown: Vec<u32> = app
            .world_mut()
            .query::<(&Node, &InheritedVisibility)>()
            .iter(app.world())
            .filter(|(_, inherited)| inherited.get())
            .map(|(node, _)| node.0)
            .collect();
        shown.sort();
        shown
    }

    #[test]
    fn a_partial_instance_shows_nothing_written_until_it_is_complete() {
        let mut app = propagating_app();
        one_entity_per_run(&mut app);
        let scene = add_scene(&mut app, shown_chain(3));
        let parent = app.world_mut().spawn(Visibility::Visible).id();
        let instance = spawner(&mut app).spawn_as_child(scene, parent);

        loop {
            app.update();
            if is_ready(&app, instance) {
                break;
            }
            assert_eq!(shown_nodes(&mut app), Vec::<u32>::new());
        }

        assert_eq!(shown_nodes(&mut app), [0, 1, 2]);
    }

    /// A root `Node(0)` with `root_visibility`, and one child `Node(1)`.
    fn shown_tree(root_visibility: Option<Visibility>) -> impl FnOnce(&mut World) {
        move |world| {
            let root = world.spawn(Node(0)).id();
            if let Some(visibility) = root_visibility {
                world.entity_mut(root).insert(visibility);
            }
            world.spawn((Node(1), Visibility::Inherited, ChildOf(root)));
        }
    }

    fn root_visibility(app: &App, parent: Entity) -> Option<Visibility> {
        let [root] = children(app, parent)[..] else {
            panic!("one root under the parent");
        };
        app.world().get::<Visibility>(root).copied()
    }

    #[test]
    fn a_partial_instance_is_hidden_until_it_is_complete() {
        let mut app = app();
        one_entity_per_run(&mut app);
        let scene = add_scene(&mut app, shown_tree(Some(Visibility::Visible)));
        let parent = app.world_mut().spawn_empty().id();
        let instance = spawner(&mut app).spawn_as_child(scene, parent);

        app.update();
        assert_eq!(root_visibility(&app, parent), Some(Visibility::Hidden));

        app.update();
        assert!(is_ready(&app, instance));
        assert_eq!(root_visibility(&app, parent), Some(Visibility::Visible));
    }

    #[test]
    fn a_root_without_visibility_inherits_once_complete() {
        let mut app = app();
        one_entity_per_run(&mut app);
        let scene = add_scene(&mut app, shown_tree(None));
        let parent = app.world_mut().spawn_empty().id();
        let instance = spawner(&mut app).spawn_as_child(scene, parent);

        app.update();
        assert_eq!(root_visibility(&app, parent), Some(Visibility::Hidden));

        while !is_ready(&app, instance) {
            app.update();
        }
        // what hiding added stays, since something may require it by now, and follows the parent
        assert_eq!(root_visibility(&app, parent), Some(Visibility::Inherited));
        let root = children(&app, parent)[0];
        assert!(app.world().get::<InheritedVisibility>(root).is_some());
        assert!(app.world().get::<ViewVisibility>(root).is_some());
    }

    #[test]
    fn every_top_level_entity_stays_hidden_until_the_instance_is_complete() {
        let mut app = app();
        one_entity_per_run(&mut app);
        let scene = add_scene(&mut app, |world: &mut World| {
            for n in [0, 10] {
                let root = world.spawn((Node(n), Visibility::Visible)).id();
                world.spawn((Node(n + 1), Visibility::Inherited, ChildOf(root)));
            }
        });
        let parent = app.world_mut().spawn_empty().id();
        let instance = spawner(&mut app).spawn_as_child(scene, parent);

        loop {
            app.update();
            if is_ready(&app, instance) {
                break;
            }
            // including a root written after the hiding, which writes its own visibility
            let roots = children(&app, parent);
            assert_eq!(roots.len(), 2);
            for root in roots {
                assert_eq!(
                    app.world().get::<Visibility>(root),
                    Some(&Visibility::Hidden)
                );
            }
        }

        for root in children(&app, parent) {
            assert_eq!(
                app.world().get::<Visibility>(root),
                Some(&Visibility::Visible)
            );
        }
    }

    #[test]
    fn a_visibility_set_while_hidden_is_kept_when_revealed() {
        let mut app = app();
        let scene = add_scene(&mut app, shown_tree(Some(Visibility::Inherited)));
        let parent = app.world_mut().spawn_empty().id();
        let instance = spawner(&mut app).spawn_as_child_hidden(scene, parent);
        app.update();
        let root = children(&app, parent)[0];
        app.world_mut().entity_mut(root).insert(Visibility::Visible);

        reveal(&mut app, instance);

        assert_eq!(root_visibility(&app, parent), Some(Visibility::Visible));
    }

    #[test]
    fn a_whole_spawn_keeps_the_scenes_visibility() {
        let mut app = app();
        let scene = add_scene(&mut app, shown_tree(Some(Visibility::Visible)));
        let parent = app.world_mut().spawn_empty().id();
        spawner(&mut app).spawn_as_child(scene, parent);

        app.update();

        assert_eq!(root_visibility(&app, parent), Some(Visibility::Visible));
    }

    #[test]
    fn a_hidden_spawn_stays_hidden_until_revealed() {
        let mut app = app();
        one_entity_per_run(&mut app);
        let scene = add_scene(&mut app, shown_tree(Some(Visibility::Visible)));
        let parent = app.world_mut().spawn_empty().id();
        let instance = spawner(&mut app).spawn_as_child_hidden(scene, parent);

        while !is_ready(&app, instance) {
            app.update();
        }
        app.update();
        assert_eq!(root_visibility(&app, parent), Some(Visibility::Hidden));

        reveal(&mut app, instance);
        assert_eq!(root_visibility(&app, parent), Some(Visibility::Visible));
    }

    #[test]
    fn a_hidden_spawn_without_a_deadline_is_hidden_at_once() {
        let mut app = app();
        let scene = add_scene(&mut app, shown_tree(Some(Visibility::Visible)));
        let parent = app.world_mut().spawn_empty().id();
        let instance = spawner(&mut app).spawn_as_child_hidden(scene, parent);

        app.update();
        assert!(is_ready(&app, instance));
        assert_eq!(root_visibility(&app, parent), Some(Visibility::Hidden));

        reveal(&mut app, instance);
        assert_eq!(root_visibility(&app, parent), Some(Visibility::Visible));
    }

    #[test]
    fn revealing_a_hidden_spawn_before_it_completes_shows_it_on_completion() {
        let mut app = app();
        one_entity_per_run(&mut app);
        let scene = add_scene(&mut app, shown_tree(Some(Visibility::Visible)));
        let parent = app.world_mut().spawn_empty().id();
        let instance = spawner(&mut app).spawn_as_child_hidden(scene, parent);
        app.update();

        reveal(&mut app, instance);
        assert_eq!(root_visibility(&app, parent), Some(Visibility::Hidden));
        while !is_ready(&app, instance) {
            app.update();
        }

        assert_eq!(root_visibility(&app, parent), Some(Visibility::Visible));
    }

    #[test]
    fn a_despawned_hidden_instance_is_forgotten() {
        let mut app = app();
        let scene = add_scene(&mut app, shown_tree(Some(Visibility::Visible)));
        let parent = app.world_mut().spawn_empty().id();
        let instance = spawner(&mut app).spawn_as_child_hidden(scene, parent);
        app.update();

        spawner(&mut app).despawn_instance(instance);
        app.update();

        assert!(app
            .world()
            .resource::<SceneSpawner>()
            .keep_hidden
            .is_empty());
        let hidden_roots = app
            .world_mut()
            .query::<&crate::scene_spawner::HiddenSceneRoot>()
            .iter(app.world())
            .count();
        assert_eq!(hidden_roots, 0);
    }

    fn reveal(app: &mut App, instance: InstanceId) {
        app.world_mut()
            .resource_scope(|world, mut spawner: bevy_ecs::world::Mut<SceneSpawner>| {
                spawner.reveal_instance_sync(world, instance);
            });
    }
}
