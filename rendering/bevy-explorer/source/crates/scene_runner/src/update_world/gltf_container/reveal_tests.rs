use bevy::{
    gltf::Gltf,
    platform::collections::{HashMap, HashSet},
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
        .register_type::<Transform>()
        .register_type::<GlobalTransform>()
        .register_type::<TransformTreeChanged>()
        .register_type::<Visibility>()
        .register_type::<InheritedVisibility>()
        .register_type::<ViewVisibility>()
        .register_type::<Name>()
        .add_systems(SpawnScene, update_ready_gltfs.after(scene_spawner_system));
    app
}

/// A one-node glTF container in a scene root, spawned the way `update_gltf` spawns it. Without a
/// scene context `update_ready_gltfs` cannot process it yet.
fn spawn_container(app: &mut App, with_context: bool) -> (Entity, InstanceId) {
    let world = app.world_mut();
    let root = world.spawn_empty().id();
    if with_context {
        world.entity_mut(root).insert((
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

    let mut model = World::new();
    model.spawn((
        Transform::default(),
        Visibility::Inherited,
        Name::new("node"),
    ));
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
        .insert(GltfLoaded(Some(instance)));
    (container, instance)
}

fn node_visibility(app: &App, container: Entity) -> Option<Visibility> {
    let node = app.world().get::<Children>(container).expect("the node")[0];
    app.world().get::<Visibility>(node).copied()
}

#[test]
fn a_processed_gltf_is_revealed() {
    let mut app = app();
    let (container, _) = spawn_container(&mut app, true);

    app.update();

    assert!(app.world().get::<GltfProcessed>(container).is_some());
    assert_eq!(
        node_visibility(&app, container),
        Some(Visibility::Inherited)
    );
}

#[test]
fn a_gltf_waiting_to_be_processed_stays_hidden() {
    let mut app = app();
    let (container, instance) = spawn_container(&mut app, false);

    app.update();

    assert!(app
        .world()
        .resource::<SceneSpawner>()
        .instance_is_ready(instance));
    assert!(app.world().get::<GltfProcessed>(container).is_none());
    assert_eq!(node_visibility(&app, container), Some(Visibility::Hidden));
}
