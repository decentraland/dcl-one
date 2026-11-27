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
        .init_resource::<SharedMeshes>()
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

fn triangle() -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
    )
}

#[test]
fn a_model_whose_material_is_gone_is_spawned_again() {
    let mut app = app();
    let root = scene_root(&mut app);
    let (container, instance) = spawn_container(&mut app, root, &["top", "painted"]);
    // spawned whole, not processed: no context yet
    app.update();
    let mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(triangle());
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

/// What an animated model leaves on its container.
fn animated(app: &mut App, container: Entity) {
    app.world_mut().entity_mut(container).insert((
        AnimationPlayer::default(),
        AnimationGraphHandle::default(),
        Clips {
            default: None,
            named: HashMap::default(),
        },
        PlaysOnce,
    ));
}

fn is_animated(app: &App, container: Entity) -> bool {
    let container = app.world().entity(container);
    container.contains::<AnimationPlayer>()
        || container.contains::<AnimationGraphHandle>()
        || container.contains::<Clips>()
        || container.contains::<PlaysOnce>()
}

#[test]
fn a_model_without_animations_drops_the_player_of_the_model_it_replaces() {
    let mut app = app();
    let root = scene_root(&mut app);
    give_context(&mut app, root);
    let (container, _) = spawn_container(&mut app, root, &["node"]);
    animated(&mut app, container);

    app.update();

    assert!(is_processed(&app, container));
    assert!(!is_animated(&app, container));
}

#[test]
fn removing_the_container_drops_its_player() {
    let mut app = app();
    let root = scene_root(&mut app);
    give_context(&mut app, root);
    let (container, _) = spawn_container(&mut app, root, &["node"]);
    app.update();
    animated(&mut app, container);

    app.world_mut()
        .entity_mut(container)
        .remove::<GltfDefinition>();
    app.update();

    assert!(!is_animated(&app, container));
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

/// The name of the primitive `spawn_mesh_node` gives its node.
const PRIMITIVE: &str = "mesh";

/// A glTF container in `root` whose model is a node named `name` with one primitive, laid out
/// as the loader does it: the node has the name and the extras, and its child, named
/// [`PRIMITIVE`], has the mesh. Spawned whole and not processed: the scene has no context yet.
fn spawn_mesh_node(app: &mut App, root: Entity, name: &str, mesh: Mesh) -> Entity {
    let (container, _) = spawn_container(app, root, &[name, PRIMITIVE]);
    app.update();
    let mesh = app.world_mut().resource_mut::<Assets<Mesh>>().add(mesh);
    let primitive = node(app, PRIMITIVE);
    app.world_mut().entity_mut(primitive).insert(Mesh3d(mesh));
    container
}

/// Gives the spawned node named `name` the glTF extras `json`.
fn give_extras(app: &mut App, name: &str, json: &str) {
    let node = node(app, name);
    app.world_mut().entity_mut(node).insert(GltfExtras {
        value: json.to_owned(),
    });
}

/// Sets the masks the container gives its visible and its invisible meshes.
fn set_container_masks(
    app: &mut App,
    container: Entity,
    visible: Option<u32>,
    invisible: Option<u32>,
) {
    let mut definition = app
        .world_mut()
        .get_mut::<GltfDefinition>(container)
        .expect("the container");
    definition.0.visible_meshes_collision_mask = visible;
    definition.0.invisible_meshes_collision_mask = invisible;
}

/// Processes the models of `root`, then gives the mask of the collider the primitive got.
fn processed_collider_mask<T: ColliderType>(app: &mut App, root: Entity) -> Option<u32> {
    give_context(app, root);
    app.update();
    let primitive = node(app, PRIMITIVE);
    app.world()
        .get::<MeshCollider<T>>(primitive)
        .map(|collider| collider.collision_mask)
}

#[test]
fn a_nodes_collider_mask_extra_beats_the_containers_mask() {
    let mut app = app();
    let root = scene_root(&mut app);
    let container = spawn_mesh_node(&mut app, root, "floor_collider", triangle());
    set_container_masks(&mut app, container, None, Some(512));
    give_extras(&mut app, "floor_collider", r#"{"dcl_collider_mask":256}"#);

    assert_eq!(
        processed_collider_mask::<CtCollider>(&mut app, root),
        Some(256)
    );
}

#[test]
fn a_primitives_collider_mask_extra_is_read_too() {
    let mut app = app();
    let root = scene_root(&mut app);
    spawn_mesh_node(&mut app, root, "floor_collider", triangle());
    give_extras(&mut app, PRIMITIVE, r#"{"dcl_collider_mask":256}"#);

    assert_eq!(
        processed_collider_mask::<CtCollider>(&mut app, root),
        Some(256)
    );
}

#[test]
fn the_extra_spelled_collision_mask_is_read_too() {
    let mut app = app();
    let root = scene_root(&mut app);
    spawn_mesh_node(&mut app, root, "floor_collider", triangle());
    give_extras(&mut app, "floor_collider", r#"{"dcl_collision_mask":256}"#);

    assert_eq!(
        processed_collider_mask::<CtCollider>(&mut app, root),
        Some(256)
    );
}

#[test]
fn a_node_naming_its_mask_both_ways_gets_the_collider_mask_one() {
    let mut app = app();
    let root = scene_root(&mut app);
    spawn_mesh_node(&mut app, root, "floor_collider", triangle());
    give_extras(
        &mut app,
        "floor_collider",
        r#"{"dcl_collider_mask":256,"dcl_collision_mask":512}"#,
    );

    assert_eq!(
        processed_collider_mask::<CtCollider>(&mut app, root),
        Some(256)
    );
}

#[test]
fn a_collider_mask_extra_that_is_not_a_u32_leaves_the_other_spelling() {
    let mut app = app();
    let root = scene_root(&mut app);
    spawn_mesh_node(&mut app, root, "floor_collider", triangle());
    give_extras(
        &mut app,
        "floor_collider",
        r#"{"dcl_collider_mask":"256","dcl_collision_mask":512}"#,
    );

    assert_eq!(
        processed_collider_mask::<CtCollider>(&mut app, root),
        Some(512)
    );
}

#[test]
fn a_node_with_other_extras_leaves_the_containers_visible_mask() {
    let mut app = app();
    let root = scene_root(&mut app);
    let container = spawn_mesh_node(&mut app, root, "wall", triangle());
    let pointer_only = ColliderLayer::ClPointer as u32;
    set_container_masks(&mut app, container, Some(pointer_only), None);
    give_extras(&mut app, "wall", r#"{"author":"someone"}"#);

    assert_eq!(
        processed_collider_mask::<CtCollider>(&mut app, root),
        Some(pointer_only)
    );
}

#[test]
fn a_node_with_other_extras_leaves_the_containers_invisible_mask() {
    let mut app = app();
    let root = scene_root(&mut app);
    let container = spawn_mesh_node(&mut app, root, "wall_collider", triangle());
    set_container_masks(&mut app, container, None, Some(512));
    give_extras(&mut app, "wall_collider", r#"{"author":"someone"}"#);

    assert_eq!(
        processed_collider_mask::<CtCollider>(&mut app, root),
        Some(512)
    );
}

#[test]
fn a_trigger_under_a_node_with_other_extras_gets_the_containers_invisible_mask() {
    let mut app = app();
    let root = scene_root(&mut app);
    let container = spawn_mesh_node(&mut app, root, "zone_trigger", triangle());
    set_container_masks(&mut app, container, None, Some(512));
    give_extras(&mut app, "zone_trigger", r#"{"author":"someone"}"#);

    assert_eq!(
        processed_collider_mask::<CtTrigger>(&mut app, root),
        Some(512)
    );
}

#[test]
fn a_skinned_mesh_under_a_node_with_other_extras_gets_no_collider() {
    let mut app = app();
    let root = scene_root(&mut app);
    let skinned = triangle()
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_JOINT_INDEX,
            VertexAttributeValues::Uint16x4(vec![[0; 4]; 3]),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, vec![[1.0, 0.0, 0.0, 0.0]; 3]);
    spawn_mesh_node(&mut app, root, "arm_collider", skinned);
    let primitive = node(&mut app, PRIMITIVE);
    app.world_mut().entity_mut(primitive).insert(SkinnedMesh {
        inverse_bindposes: Handle::default(),
        joints: Vec::new(),
    });
    give_extras(&mut app, "arm_collider", r#"{"author":"someone"}"#);

    assert_eq!(processed_collider_mask::<CtCollider>(&mut app, root), None);
    // processed as a skinned mesh, which is what leaves it without a collider
    assert!(app.world().get::<NoFrustumCulling>(primitive).is_some());
}

#[test]
fn a_file_with_its_data_inside_references_no_file() {
    assert!(uris_in_file([None, Some("data:image/png;base64,AAAA")]));
    assert!(uris_in_file([]));
    assert!(!uris_in_file([None, Some("textures/wood.png")]));
    assert!(!uris_in_file([Some("model.bin")]));
    assert!(!uris_in_file([Some("data:tex.png")]));
}

#[test]
fn two_scenes_with_the_same_mesh_draw_one_copy() {
    let mut app = app();
    let roots = [scene_root(&mut app), scene_root(&mut app)];
    let primitives = ["primitive_a", "primitive_b"];
    for (root, primitive) in roots.into_iter().zip(primitives) {
        spawn_container(&mut app, root, &["top", primitive]);
        app.update();
        let mesh = app.world().resource::<AssetServer>().add(triangle());
        let node = node(&mut app, primitive);
        app.world_mut().entity_mut(node).insert(Mesh3d(mesh));
    }
    for root in roots {
        give_context(&mut app, root);
    }
    app.update();

    let [a, b] = primitives.map(|primitive| {
        let node = node(&mut app, primitive);
        app.world().get::<Mesh3d>(node).expect("a mesh").id()
    });
    assert_eq!(a, b);
    assert_eq!(app.world().resource::<SharedMeshes>().meshes(), 1);
    for root in roots {
        let lookup = app.world().get::<SceneResourceLookup>(root).unwrap();
        assert_eq!(lookup.mesh_ids_by_hash.len(), 1);
    }
}

/// A container in `root` whose node `primitive` draws a new triangle, ready to be processed.
fn spawn_triangle(app: &mut App, root: Entity, primitive: &str) -> Handle<Mesh> {
    spawn_container(app, root, &[&format!("top_{primitive}"), primitive]);
    app.update();
    let mesh = app.world().resource::<AssetServer>().add(triangle());
    let node = node(app, primitive);
    app.world_mut()
        .entity_mut(node)
        .insert(Mesh3d(mesh.clone()));
    mesh
}

/// The mesh the node `primitive` draws.
fn drawn_mesh(app: &mut App, primitive: &str) -> AssetId<Mesh> {
    let node = node(app, primitive);
    app.world().get::<Mesh3d>(node).expect("a mesh").id()
}

#[test]
fn without_a_salt_a_scene_takes_only_its_own_cached_mesh() {
    let mut app = app();
    // `getrandom` failing cannot be caused from a test
    app.world_mut().resource_mut::<SharedMeshes>().hash_salt = None;
    let [a, b] = [scene_root(&mut app), scene_root(&mut app)];

    // scene A: two containers with the triangle
    spawn_triangle(&mut app, a, "a_first");
    spawn_triangle(&mut app, a, "a_second");
    give_context(&mut app, a);
    app.update();
    let a_mesh = drawn_mesh(&mut app, "a_first");
    assert_eq!(drawn_mesh(&mut app, "a_second"), a_mesh);

    // scene B: one container with the same triangle
    spawn_triangle(&mut app, b, "b_first");
    give_context(&mut app, b);
    app.update();
    let b_mesh = drawn_mesh(&mut app, "b_first");
    assert_ne!(b_mesh, a_mesh);

    // a later container of A
    let context = app
        .world_mut()
        .entity_mut(a)
        .take::<RendererSceneContext>()
        .expect("a context");
    let a_later = spawn_triangle(&mut app, a, "a_later");
    app.world_mut().entity_mut(a).insert(context);
    app.update();
    let lookup = app.world().get::<SceneResourceLookup>(a).unwrap();
    assert!(lookup
        .mesh_ids_by_hash
        .values()
        .any(|id| *id == a_later.id()));
    assert_eq!(drawn_mesh(&mut app, "a_later"), a_later.id());
}

/// [`app`] that also runs `update_gltf`, with an ipfs client that has nothing to read.
fn update_app() -> App {
    use bevy::asset::io::memory::{Dir, MemoryAssetReader};

    let mut app = app();
    let ipfs = ipfs::IpfsResource {
        inner: std::sync::Arc::new(ipfs::IpfsIo::new(
            false,
            Box::new(MemoryAssetReader {
                root: Dir::default(),
            }),
            None,
            Default::default(),
            1,
            tokio::sync::mpsc::unbounded_channel().0,
        )),
    };
    app.init_asset::<ipfs::EntityDefinition>()
        .init_asset::<Image>()
        .init_resource::<SharedGltfs>()
        .init_resource::<crate::ScenePointers>()
        .init_resource::<crate::LiveScenes>()
        .init_resource::<crate::PortableScenes>()
        .insert_resource(ipfs)
        .add_systems(Update, update_gltf);
    app
}

/// A container in `root` whose model, a node named `name`, the asset server reports loaded, and
/// whose definition is the source already acted on.
fn spawn_loaded_container(app: &mut App, root: Entity, name: &str) -> Entity {
    let mut model = World::new();
    model.spawn((
        Transform::default(),
        Visibility::Inherited,
        Name::new(name.to_owned()),
    ));
    let world = app.world_mut();
    let h_scene = world.resource_mut::<Assets<Scene>>().add(Scene::new(model));
    let h_gltf = world.resource::<AssetServer>().add(Gltf {
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
        default_scene: Some(h_scene),
        animations: Vec::new(),
        named_animations: HashMap::default(),
        source: None,
    });
    let definition = PbGltfContainer {
        src: "model.glb".to_owned(),
        ..Default::default()
    };
    world
        .spawn((
            SceneEntity {
                root,
                scene_id: SceneId(root),
                id: SceneEntityId {
                    id: 512,
                    generation: 0,
                },
            },
            GltfDefinition(definition.clone()),
            GltfSource(definition),
            GltfHandle(h_gltf),
        ))
        .id()
}

#[test]
fn a_container_whose_definition_is_put_again_every_frame_is_spawned() {
    let mut app = update_app();
    let root = scene_root(&mut app);
    give_context(&mut app, root);
    let container = spawn_loaded_container(&mut app, root, "model_node");

    // the same definition put again each frame
    for _ in 0..3 {
        let definition = app
            .world()
            .get::<GltfDefinition>(container)
            .unwrap()
            .0
            .clone();
        app.world_mut()
            .entity_mut(container)
            .insert(GltfDefinition(definition));
        app.update();
    }
    assert!(app.world().get::<GltfLoaded>(container).is_some());
}

const URL: &str = "https://content.example/contents/bafkreimodel";

/// An app that keeps the shared caches as the explorer does.
fn sharing_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Gltf>()
        .init_asset::<Mesh>()
        .init_resource::<GltfNameCache>()
        .init_resource::<SharedGltfs>()
        .init_resource::<SharedMeshes>()
        .add_systems(Update, (maintain_gltf_caches, maintain_shared_meshes));
    app
}

/// A model loaded through the asset server, as a scene's glTF is.
fn load_model(app: &mut App) -> Handle<Gltf> {
    let model = Gltf {
        scenes: Vec::new(),
        named_scenes: HashMap::default(),
        meshes: Vec::new(),
        named_meshes: HashMap::default(),
        materials: Vec::new(),
        named_materials: HashMap::default(),
        nodes: Vec::new(),
        named_nodes: HashMap::default(),
        skins: Vec::new(),
        named_skins: HashMap::default(),
        default_scene: None,
        animations: Vec::new(),
        named_animations: HashMap::default(),
        source: None,
    };
    let handle = app.world().resource::<AssetServer>().add(model);
    app.update();
    handle
}

/// What a container whose model loaded as `h_gltf` from [`URL`] takes instead.
fn adopt(app: &mut App, h_gltf: &Handle<Gltf>, references_no_file: bool) -> Option<AssetId<Gltf>> {
    let asset_server = app.world().resource::<AssetServer>().clone();
    app.world_mut()
        .resource_mut::<SharedGltfs>()
        .adopt(&asset_server, URL.to_owned(), h_gltf, references_no_file)
        .map(|h_shared| h_shared.id())
}

/// Frames enough for a dropped asset's removal to reach the caches.
fn settle(app: &mut App) {
    for _ in 0..3 {
        app.update();
    }
}

#[test]
fn scenes_that_load_a_model_at_the_same_time_end_on_the_first_copy() {
    let mut app = sharing_app();
    let first = load_model(&mut app);
    let second = load_model(&mut app);

    assert_eq!(adopt(&mut app, &first, true), None);
    // the second copy, loaded
    assert_eq!(adopt(&mut app, &second, true), Some(first.id()));
    // the first copy, for another container
    assert_eq!(adopt(&mut app, &first, true), None);
    assert_eq!(app.world().resource::<SharedGltfs>().models(), 1);
}

#[test]
fn a_copy_that_loads_after_the_shared_one_goes_is_shared_instead() {
    let mut app = sharing_app();
    let first = load_model(&mut app);
    assert_eq!(adopt(&mut app, &first, true), None);
    let second = load_model(&mut app);

    // the first copy dropped while the second loads
    drop(first);
    assert_eq!(adopt(&mut app, &second, true), None);

    let third = load_model(&mut app);
    assert_eq!(adopt(&mut app, &third, true), Some(second.id()));
}

#[test]
fn a_model_that_references_other_files_is_not_shared() {
    let mut app = sharing_app();
    let first = load_model(&mut app);
    let second = load_model(&mut app);
    assert_eq!(adopt(&mut app, &first, false), None);
    assert_eq!(adopt(&mut app, &second, false), None);
    assert_eq!(app.world().resource::<SharedGltfs>().models(), 0);
}

#[test]
fn a_removed_model_is_no_longer_shared() {
    let mut app = sharing_app();
    let first = load_model(&mut app);
    assert_eq!(adopt(&mut app, &first, true), None);
    assert_eq!(app.world().resource::<SharedGltfs>().models(), 1);

    drop(first);
    settle(&mut app);
    assert_eq!(app.world().resource::<SharedGltfs>().models(), 0);
}

#[test]
fn a_removed_mesh_leaves_the_shared_meshes() {
    let mut app = sharing_app();
    let mesh = app.world().resource::<AssetServer>().add(triangle());
    app.update();
    let mut shared = app.world_mut().resource_mut::<SharedMeshes>();
    shared.hash_by_id.insert(mesh.id(), 7);
    shared.by_hash.insert(
        (7, true),
        CachedMeshData {
            mesh_id: mesh.id(),
            is_skinned: false,
            shape: None,
            maybe_collider: None,
        },
    );
    assert_eq!(app.world().resource::<SharedMeshes>().meshes(), 1);

    drop(mesh);
    settle(&mut app);
    assert_eq!(app.world().resource::<SharedMeshes>().meshes(), 0);
}
