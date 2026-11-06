use bevy_app::App;
use bevy_asset::{AssetPlugin, Assets, Handle};
use bevy_ecs::{
    component::Component,
    entity::Entity,
    hierarchy::{ChildOf, Children},
    observer::Trigger,
    prelude::ReflectComponent,
    resource::Resource,
    system::ResMut,
    world::World,
};
use bevy_platform::time::Instant;
use bevy_reflect::Reflect;

use super::{InstanceId, SceneInstanceReady, SceneSpawner};
use crate::{DynamicScene, Scene, ScenePlugin};

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
    use bevy_render::view::visibility::{InheritedVisibility, ViewVisibility, Visibility};

    use super::*;

    fn app() -> App {
        let mut app = super::app();
        app.register_type::<Visibility>()
            .register_type::<InheritedVisibility>()
            .register_type::<ViewVisibility>();
        app
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
    fn a_root_without_visibility_has_none_once_complete() {
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
        assert_eq!(root_visibility(&app, parent), None);
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

        let spawner = app.world().resource::<SceneSpawner>();
        assert!(spawner.hidden_instances.is_empty());
        assert!(spawner.keep_hidden.is_empty());
    }

    fn reveal(app: &mut App, instance: InstanceId) {
        app.world_mut()
            .resource_scope(|world, mut spawner: bevy_ecs::world::Mut<SceneSpawner>| {
                spawner.reveal_instance_sync(world, instance);
            });
    }
}
