use bevy::{
    gltf::Gltf,
    platform::{
        collections::{HashMap, HashSet},
        time::Instant,
    },
    render::mesh::PrimitiveTopology,
    scene::{scene_spawner_system, InstanceId, ScenePlugin, SceneSpawner},
    transform::components::TransformTreeChanged,
};
use common::structs::AppConfig;
use dcl_component::{proto_components::sdk::components::PbGltfContainer, SceneEntityId};
use dcl_types::SceneId;
use scene_material::{SceneBounds, SceneMaterial};

use super::*;
use crate::{renderer_context::RendererSceneContext, update_world::ComponentTracker, SceneEntity};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin))
        .init_asset::<Gltf>()
        .init_asset::<StandardMaterial>()
        .init_asset::<SceneMaterial>()
        .init_asset::<AnimationGraph>()
        .init_asset::<AnimationClip>()
        .init_asset::<Mesh>()
        .init_resource::<AppConfig>()
        .init_resource::<GltfNameCache>()
        .init_resource::<ProcessingDeadline>()
        .init_resource::<SlowestProcessing>()
        .register_type::<Transform>()
        .register_type::<GlobalTransform>()
        .register_type::<TransformTreeChanged>()
        .register_type::<Visibility>()
        .register_type::<InheritedVisibility>()
        .register_type::<ViewVisibility>()
        .register_type::<Name>()
        .add_systems(SpawnScene, update_ready_gltfs.after(scene_spawner_system))
        .add_observer(on_gltf_container_removed);
    app
}

/// A scene root, with a context only once `give_context` runs: without one `update_ready_gltfs`
/// cannot process its glTFs yet.
fn scene_root(app: &mut App) -> Entity {
    app.world_mut().spawn_empty().id()
}

fn give_context(app: &mut App, root: Entity) {
    app.world_mut().entity_mut(root).insert((
        RendererSceneContext::new(
            SceneId(root),
            "hash".to_owned(),
            String::new(),
            false,
            0,
            String::new(),
            IVec2::ZERO,
            HashSet::default(),
            Vec::new(),
            SceneBounds::default(),
            Vec::new(),
            root,
            0.0,
            false,
            "",
            false,
            false,
        ),
        SceneResourceLookup::default(),
        ComponentTracker::default(),
    ));
}

/// A glTF container in `root` whose model is a node named `nodes[0]` with the other names as
/// its children, spawned the way `update_gltf` spawns it.
fn spawn_container(app: &mut App, root: Entity, nodes: &[&str]) -> (Entity, InstanceId) {
    let world = app.world_mut();

    let mut model = World::new();
    let top = model
        .spawn((
            Transform::default(),
            Visibility::Inherited,
            Name::new(nodes[0].to_owned()),
        ))
        .id();
    for name in &nodes[1..] {
        model.spawn((
            Transform::default(),
            Visibility::Inherited,
            Name::new(name.to_string()),
            ChildOf(top),
        ));
    }
    let h_scene = world.resource_mut::<Assets<Scene>>().add(Scene::new(model));
    let h_gltf = world.resource_mut::<Assets<Gltf>>().add(Gltf {
        scenes: vec![h_scene.clone()],
        named_scenes: HashMap::default(),
        meshes: Vec::new(),
        named_meshes: HashMap::default(),
        materials: Vec::new(),
        named_materials: HashMap::default(),
        nodes: Vec::new(),
        named_nodes: HashMap::default(),
        skins: Vec::new(),
        named_skins: HashMap::default(),
        default_scene: Some(h_scene.clone()),
        animations: Vec::new(),
        named_animations: HashMap::default(),
        source: None,
    });

    let container = world
        .spawn((
            SceneEntity {
                root,
                scene_id: SceneId(root),
                id: SceneEntityId {
                    id: 512,
                    generation: 0,
                },
            },
            GltfDefinition(PbGltfContainer::default()),
            GltfHandle(h_gltf),
        ))
        .id();
    let instance = world
        .resource_mut::<SceneSpawner>()
        .spawn_as_child_hidden(h_scene, container);
    world
        .entity_mut(container)
        .insert(GltfLoaded::start(Some(instance)));
    (container, instance)
}

/// From now on processing starts past its deadline each frame, so it processes one node.
fn one_node_per_frame(app: &mut App) {
    let past = Instant::now();
    app.world_mut()
        .resource_mut::<SceneSpawner>()
        .set_spawn_deadline(Some(past));
    app.insert_resource(ProcessingDeadline(Some(past)));
}

fn is_processed(app: &App, container: Entity) -> bool {
    app.world().get::<GltfProcessed>(container).is_some()
}

fn node_visibility(app: &App, container: Entity) -> Option<Visibility> {
    let node = app.world().get::<Children>(container).expect("the node")[0];
    app.world().get::<Visibility>(node).copied()
}

/// The spawned node named `name`.
fn node(app: &mut App, name: &str) -> Entity {
    app.world_mut()
        .query::<(Entity, &Name)>()
        .iter(app.world())
        .find(|(_, node)| node.as_str() == name)
        .map(|(entity, _)| entity)
        .expect("a node with that name")
}

fn has_node(app: &mut App, name: &str) -> bool {
    app.world_mut()
        .query::<&Name>()
        .iter(app.world())
        .any(|node| node.as_str() == name)
}

/// A glTF container in `root` with nothing to process: its file failed or has no scene.
fn spawn_empty_container(app: &mut App, root: Entity) -> Entity {
    app.world_mut()
        .spawn((
            SceneEntity {
                root,
                scene_id: SceneId(root),
                id: SceneEntityId {
                    id: 513,
                    generation: 0,
                },
            },
            GltfDefinition(PbGltfContainer::default()),
            GltfHandle(Handle::default()),
            GltfLoaded::start(None),
        ))
        .id()
}

/// The node paths processing recorded, sorted.
fn processed_names(app: &App, container: Entity) -> Vec<String> {
    let mut names: Vec<_> = app
        .world()
        .get::<GltfProcessed>(container)
        .expect("a processed container")
        .named_nodes
        .keys()
        .cloned()
        .collect();
    names.sort();
    names
}

#[test]
fn a_processed_gltf_is_revealed() {
    let mut app = app();
    let root = scene_root(&mut app);
    give_context(&mut app, root);
    let (container, _) = spawn_container(&mut app, root, &["node"]);

    app.update();

    assert!(is_processed(&app, container));
    assert_eq!(
        node_visibility(&app, container),
        Some(Visibility::Inherited)
    );
}

#[test]
fn a_gltf_waiting_to_be_processed_stays_hidden() {
    let mut app = app();
    let root = scene_root(&mut app);
    let (container, instance) = spawn_container(&mut app, root, &["node"]);

    app.update();

    assert!(app
        .world()
        .resource::<SceneSpawner>()
        .instance_is_ready(instance));
    assert!(!is_processed(&app, container));
    assert_eq!(node_visibility(&app, container), Some(Visibility::Hidden));
}

#[test]
fn past_its_deadline_processing_takes_one_node_per_frame() {
    let mut app = app();
    let root = scene_root(&mut app);
    let (container, _) = spawn_container(&mut app, root, &["top", "a", "b"]);
    // spawned whole, not processed: no context yet
    app.update();
    give_context(&mut app, root);
    one_node_per_frame(&mut app);

    app.update();
    app.update();
    assert!(!is_processed(&app, container));
    assert_eq!(node_visibility(&app, container), Some(Visibility::Hidden));

    app.update();
    assert!(is_processed(&app, container));
    assert_eq!(
        node_visibility(&app, container),
        Some(Visibility::Inherited)
    );
}

#[test]
fn a_gltf_processed_over_frames_names_every_node() {
    let mut app = app();
    let root = scene_root(&mut app);
    let (container, _) = spawn_container(&mut app, root, &["top", "a", "b"]);
    app.update();
    give_context(&mut app, root);
    one_node_per_frame(&mut app);

    while !is_processed(&app, container) {
        app.update();
    }

    assert_eq!(processed_names(&app, container), ["top", "top/a", "top/b"]);
}

#[test]
fn a_light_node_despawned_by_an_earlier_slice_does_not_stop_processing() {
    let mut app = app();
    let root = scene_root(&mut app);
    let (container, _) = spawn_container(&mut app, root, &["top", "sun", "moon", "a"]);
    // spawned whole, not processed: no context yet
    app.update();
    // processing despawns directional lights; with two, whatever the node order, a later
    // frame goes back to the model after one of them is gone
    for name in ["sun", "moon"] {
        let light = node(&mut app, name);
        app.world_mut()
            .entity_mut(light)
            .insert(DirectionalLight::default());
    }
    give_context(&mut app, root);
    one_node_per_frame(&mut app);

    for _ in 0..4 {
        app.update();
    }

    assert!(is_processed(&app, container));
    assert_eq!(processed_names(&app, container), ["top", "top/a"]);
    assert_eq!(
        node_visibility(&app, container),
        Some(Visibility::Inherited)
    );
}

#[test]
fn a_model_whose_material_is_gone_is_spawned_again() {
    let mut app = app();
    let root = scene_root(&mut app);
    let (container, instance) = spawn_container(&mut app, root, &["top", "painted"]);
    // spawned whole, not processed: no context yet
    app.update();
    let triangle = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
    );
    let mesh = app.world_mut().resource_mut::<Assets<Mesh>>().add(triangle);
    let mut materials = app.world_mut().resource_mut::<Assets<StandardMaterial>>();
    let material = materials.add(StandardMaterial::default());
    materials.remove(&material);
    let painted = node(&mut app, "painted");
    app.world_mut()
        .entity_mut(painted)
        .insert((Mesh3d(mesh), MeshMaterial3d(material)));
    give_context(&mut app, root);

    app.update();
    // update_gltf spawns a container again once it has no GltfLoaded
    assert!(app.world().get::<GltfLoaded>(container).is_none());

    app.update();
    assert!(!app
        .world()
        .resource::<SceneSpawner>()
        .instance_is_ready(instance));
}

/// Nodes of every model that processing has reached.
fn processed_nodes(app: &mut App) -> usize {
    app.world_mut()
        .query_filtered::<(), (With<Name>, With<ContainerEntity>)>()
        .iter(app.world())
        .count()
}

#[test]
fn a_gltf_in_progress_finishes_before_the_next_one_starts() {
    let mut app = app();
    let (first_scene, second_scene) = (scene_root(&mut app), scene_root(&mut app));
    let (first, _) = spawn_container(&mut app, first_scene, &["top", "a"]);
    let (second, _) = spawn_container(&mut app, second_scene, &["top", "a"]);
    app.update();
    one_node_per_frame(&mut app);
    // the later model starts first: the earlier one's scene has no context yet
    give_context(&mut app, second_scene);
    app.update();
    give_context(&mut app, first_scene);
    assert_eq!(processed_nodes(&mut app), 1);

    // the earlier model comes first in the query: only the order processing keeps puts the
    // started one before it
    app.update();
    assert!(is_processed(&app, second));
    assert!(!is_processed(&app, first));
    assert_eq!(processed_nodes(&mut app), 2);

    app.update();
    app.update();
    assert!(is_processed(&app, first));
}

#[test]
fn without_a_deadline_every_gltf_is_processed_at_once() {
    let mut app = app();
    let root = scene_root(&mut app);
    give_context(&mut app, root);
    let (first, _) = spawn_container(&mut app, root, &["top", "a", "b"]);
    let (second, _) = spawn_container(&mut app, root, &["top", "a"]);

    app.update();

    assert!(is_processed(&app, first) && is_processed(&app, second));
}

#[test]
fn removing_the_container_drops_its_progress() {
    let mut app = app();
    let root = scene_root(&mut app);
    let (container, _) = spawn_container(&mut app, root, &["top", "a", "b"]);
    app.update();
    give_context(&mut app, root);
    one_node_per_frame(&mut app);
    app.update();
    // processing has started: one node is done
    assert_eq!(processed_nodes(&mut app), 1);
    assert!(!is_processed(&app, container));

    app.world_mut()
        .entity_mut(container)
        .remove::<GltfDefinition>();
    app.update();

    assert!(app.world().get::<GltfProcessing>(container).is_none());
    // and its model, whose nodes processed so far already collide
    assert!(!has_node(&mut app, "top"));
    assert!(!has_node(&mut app, "a"));
}

#[test]
fn removing_the_container_while_its_model_spawns_leaves_nothing() {
    let mut app = app();
    let root = scene_root(&mut app);
    give_context(&mut app, root);
    // the spawner's deadline too: it writes one entity a frame
    one_node_per_frame(&mut app);
    let (container, instance) = spawn_container(&mut app, root, &["top", "a", "b"]);
    app.update();
    let spawner = |app: &App| {
        app.world()
            .resource::<SceneSpawner>()
            .instance_is_ready(instance)
    };
    assert!(!spawner(&app));

    app.world_mut()
        .entity_mut(container)
        .remove::<GltfDefinition>();
    for _ in 0..4 {
        app.update();
    }

    assert!(!spawner(&app));
    assert!(!has_node(&mut app, "top"));
    assert!(!has_node(&mut app, "a"));
}

#[test]
fn a_model_with_nothing_to_process_does_not_wait_behind_one_in_progress() {
    let mut app = app();
    let root = scene_root(&mut app);
    let (busy, _) = spawn_container(&mut app, root, &["top", "a", "b"]);
    app.update();
    give_context(&mut app, root);
    one_node_per_frame(&mut app);
    app.update();
    let empty = spawn_empty_container(&mut app, root);

    app.update();

    assert!(is_processed(&app, empty));
    assert!(!is_processed(&app, busy));
}
