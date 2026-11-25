use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

use bevy::{
    asset::{
        io::{
            memory::{Dir, MemoryAssetReader},
            AssetSourceBuilder, AssetSourceId, Reader,
        },
        AssetLoader, LoadContext, LoadState,
    },
    gltf::Gltf,
    platform::collections::HashMap,
};
use common::structs::PreviewCommand;
use dcl_component::{proto_components::sdk::components::PbGltfContainer, SceneEntityId};
use dcl_types::SceneId;
use scene_material::SceneMaterial;

use super::*;
use crate::{initialize_scene::LiveScenes, util::handle_preview_command, Toasts};

/// The containers whose definition changed, as `update_gltf` would see them.
#[derive(Resource, Default)]
struct Changed(Vec<Entity>);

fn record_changes(
    definitions: Query<Entity, bevy::ecs::query::Changed<GltfDefinition>>,
    mut changed: ResMut<Changed>,
) {
    changed.0.extend(definitions.iter());
}

/// The systems of `SceneUtilPlugin` the model commands go through, in its order.
fn app() -> App {
    app_with_files(Dir::default())
}

fn app_with_files(files: Dir) -> App {
    let mut app = App::new();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSourceBuilder::default().with_reader(move || {
            Box::new(MemoryAssetReader {
                root: files.clone(),
            })
        }),
    );
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Gltf>()
        .init_asset::<GltfMesh>()
        .add_event::<PreviewCommand>()
        .add_event::<ReloadModel>()
        .add_event::<RemoveModel>()
        .init_resource::<LiveScenes>()
        .init_resource::<Toasts>()
        .init_resource::<Changed>()
        .add_systems(
            Update,
            (
                finish_model_reloads,
                handle_preview_command,
                remove_model,
                reload_model,
            )
                .chain(),
        )
        .add_systems(PostUpdate, record_changes);
    app
}

/// A live scene named `hash`.
fn live_scene(app: &mut App, hash: &str) -> Entity {
    let scene = app.world_mut().spawn_empty().id();
    app.world_mut()
        .resource_mut::<LiveScenes>()
        .scenes
        .insert(hash.to_owned(), scene);
    scene
}

fn container(app: &mut App, scene: Entity, src: &str, h_gltf: Option<Handle<Gltf>>) -> Entity {
    let mut entity = app.world_mut().spawn((
        SceneEntity {
            root: scene,
            scene_id: SceneId(scene),
            id: SceneEntityId {
                id: 512,
                generation: 0,
            },
        },
        GltfDefinition::from(PbGltfContainer {
            src: src.to_owned(),
            ..Default::default()
        }),
        GltfDefinitionGeneration::default(),
    ));
    if let Some(h_gltf) = h_gltf {
        entity.insert(GltfHandle::for_test(h_gltf));
    }
    entity.id()
}

fn command(app: &mut App, command: PreviewCommand) {
    app.world_mut().send_event(command);
}

fn reload(scene_id: &str, src: &str) -> PreviewCommand {
    PreviewCommand::ReloadModel {
        hash: "b64-unused".to_owned(),
        src: src.to_owned(),
        scene_id: scene_id.to_owned(),
    }
}

fn remove(scene_id: &str, src: &str) -> PreviewCommand {
    PreviewCommand::RemoveModel {
        hash: "b64-unused".to_owned(),
        src: src.to_owned(),
        scene_id: scene_id.to_owned(),
    }
}

/// Runs frames until `done`, or panics after a few seconds.
fn update_until(app: &mut App, what: &str, mut done: impl FnMut(&mut App) -> bool) {
    for _ in 0..500 {
        app.update();
        if done(app) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for {what}");
}

/// Runs a frame and returns the containers whose definition changed in it.
fn changed_in_next_frame(app: &mut App) -> Vec<Entity> {
    app.world_mut().resource_mut::<Changed>().0.clear();
    app.update();
    std::mem::take(&mut app.world_mut().resource_mut::<Changed>().0)
}

struct RecordingGltfLoader(Arc<Mutex<Vec<Vec<u8>>>>);

impl AssetLoader for RecordingGltfLoader {
    type Asset = Gltf;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _: &Self::Settings,
        _: &mut LoadContext<'_>,
    ) -> Result<Gltf, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        self.0.lock().unwrap().push(bytes);
        Ok(Gltf {
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
        })
    }

    fn extensions(&self) -> &[&str] {
        &["previewgltf"]
    }
}

#[test]
fn consecutive_edits_read_new_bytes_and_reprocess_the_same_container() {
    let files = Dir::default();
    files.insert_asset_text(Path::new("model.previewgltf"), "first");
    let loads = Arc::new(Mutex::new(Vec::new()));
    let mut app = app_with_files(files.clone());
    app.register_asset_loader(RecordingGltfLoader(loads.clone()));
    let scene = live_scene(&mut app, "b64-scene");
    let handle: Handle<Gltf> = app
        .world()
        .resource::<AssetServer>()
        .load("model.previewgltf");
    let id = handle.id();
    let model = container(&mut app, scene, "model.previewgltf", Some(handle));
    update_until(&mut app, "initial bytes", |app| {
        app.world()
            .resource::<AssetServer>()
            .is_loaded_with_dependencies(id)
    });
    app.update();
    app.update();

    for (index, bytes) in ["second", "third"].into_iter().enumerate() {
        files.insert_asset_text(Path::new("model.previewgltf"), bytes);
        command(&mut app, reload("b64-scene", "model.previewgltf"));
        assert!(changed_in_next_frame(&mut app).is_empty());
        assert!(app.world().get::<ModelReloadPending>(model).is_some());
        app.world_mut().resource_mut::<Changed>().0.clear();
        update_until(&mut app, "updated bytes and container", |app| {
            app.world().get::<ModelReloadPending>(model).is_none()
        });
        assert_eq!(loads.lock().unwrap().len(), index + 2);
        assert_eq!(loads.lock().unwrap().last().unwrap(), bytes.as_bytes());
        assert_eq!(app.world().resource::<Changed>().0, vec![model]);
        assert_eq!(
            app.world().resource::<LiveScenes>().scenes["b64-scene"],
            scene
        );
    }
}

#[test]
fn reload_loads_the_file_again_and_then_the_containers_showing_it() {
    let mut app = app();
    let scene = live_scene(&mut app, "b64-scene");
    // there is no loader for it, so each load of the file fails: a failure ends a reload too
    let h_gltf: Handle<Gltf> = app.world().resource::<AssetServer>().load("model.glb");
    let id = h_gltf.id();
    let model = container(&mut app, scene, "model.glb", Some(h_gltf.clone()));
    // the content map ignores case and a leading separator
    let same_file = container(&mut app, scene, "/Model.GLB", Some(h_gltf));
    let other = container(&mut app, scene, "other.glb", None);
    update_until(&mut app, "the first load", |app| {
        matches!(
            app.world().resource::<AssetServer>().load_state(id),
            LoadState::Failed(_)
        )
    });
    // let the first load's failure event go by
    app.update();
    app.update();

    command(&mut app, reload("b64-scene", "model.glb"));
    // the file is loading again: nothing loads the old one in the meantime
    assert_eq!(changed_in_next_frame(&mut app), Vec::<Entity>::new());
    for entity in [model, same_file] {
        assert_eq!(
            app.world().get::<ModelReloadPending>(entity),
            Some(&ModelReloadPending {
                asset_id: id,
                generation: 1
            })
        );
    }
    assert!(app.world().get::<ModelReloadPending>(other).is_none());

    let mut changed = Vec::new();
    update_until(&mut app, "the reload", |app| {
        changed.append(&mut app.world_mut().resource_mut::<Changed>().0);
        app.world().get::<ModelReloadPending>(model).is_none()
    });
    changed.append(&mut app.world_mut().resource_mut::<Changed>().0);
    changed.sort();
    let mut expected = vec![model, same_file];
    expected.sort();
    assert_eq!(changed, expected);
    assert!(app.world().get::<ModelReloadPending>(same_file).is_none());
}

#[test]
fn reload_forgets_the_scene_data_kept_for_the_old_file() {
    let mut app = app();
    let scene = live_scene(&mut app, "b64-scene");
    let other_scene = live_scene(&mut app, "b64-other");
    let material = Assets::<StandardMaterial>::default().reserve_handle();
    let scene_material = Assets::<SceneMaterial>::default().reserve_handle();
    let h_gltf = app.world_mut().resource_mut::<Assets<Gltf>>().add(Gltf {
        scenes: Vec::new(),
        named_scenes: HashMap::default(),
        meshes: Vec::new(),
        named_meshes: HashMap::default(),
        materials: vec![material.clone()],
        named_materials: HashMap::default(),
        nodes: Vec::new(),
        named_nodes: HashMap::default(),
        skins: Vec::new(),
        named_skins: HashMap::default(),
        default_scene: None,
        animations: Vec::new(),
        named_animations: HashMap::default(),
        source: None,
    });
    let id = h_gltf.id();
    let lookup = || {
        let mut lookup = SceneResourceLookup::default();
        lookup
            .materials
            .insert(material.clone(), scene_material.clone());
        lookup
    };
    app.world_mut().entity_mut(scene).insert(lookup());
    app.world_mut().entity_mut(other_scene).insert(lookup());
    let model = container(&mut app, scene, "model.glb", Some(h_gltf));
    app.world_mut()
        .entity_mut(model)
        .insert(ModelReloadPending {
            asset_id: id,
            generation: 0,
        });
    app.update();

    app.world_mut()
        .send_event(AssetEvent::<Gltf>::LoadedWithDependencies { id });
    assert_eq!(changed_in_next_frame(&mut app), vec![model]);
    assert!(app
        .world()
        .get::<SceneResourceLookup>(scene)
        .unwrap()
        .materials
        .is_empty());
    assert_eq!(
        app.world()
            .get::<SceneResourceLookup>(other_scene)
            .unwrap()
            .materials
            .len(),
        1
    );
}

#[test]
fn remove_releases_the_containers_showing_the_file() {
    let mut app = app();
    let scene = live_scene(&mut app, "b64-scene");
    let other_scene = live_scene(&mut app, "b64-other");
    let h_gltf = app
        .world_mut()
        .resource_mut::<Assets<Gltf>>()
        .reserve_handle();
    let model = container(&mut app, scene, "model.glb", Some(h_gltf.clone()));
    let other = container(&mut app, scene, "other.glb", Some(h_gltf.clone()));
    let in_other_scene = container(&mut app, other_scene, "model.glb", Some(h_gltf));
    app.update();

    command(&mut app, remove("b64-scene", "model.glb"));
    app.update();

    // the definition going is what releases the model in any state (on_gltf_container_removed)
    assert!(app.world().get::<GltfDefinition>(model).is_none());
    assert!(app.world().get::<GltfHandle>(model).is_none());
    for entity in [other, in_other_scene] {
        assert!(app.world().get::<GltfDefinition>(entity).is_some());
        assert!(app.world().get::<GltfHandle>(entity).is_some());
    }
}

#[test]
fn a_command_for_a_scene_that_is_not_live_does_not_stop_the_next_ones() {
    let mut app = app();
    let scene = live_scene(&mut app, "b64-scene");
    let model = container(&mut app, scene, "model.glb", None);
    let removed = container(&mut app, scene, "removed.glb", None);
    app.update();

    command(&mut app, reload("b64-gone", "model.glb"));
    command(&mut app, remove("b64-gone", "removed.glb"));
    command(&mut app, reload("b64-scene", "model.glb"));
    command(&mut app, remove("b64-scene", "removed.glb"));
    assert_eq!(changed_in_next_frame(&mut app), vec![model]);
    assert!(app.world().get::<GltfDefinition>(removed).is_none());
}

#[test]
fn with_an_inspector_attached_model_commands_only_toast() {
    let mut app = app();
    let scene = live_scene(&mut app, "b64-scene");
    let mut context = crate::renderer_context::RendererSceneContext::new(
        SceneId(scene),
        "b64-scene".to_owned(),
        String::new(),
        false,
        0,
        String::new(),
        IVec2::ZERO,
        Default::default(),
        Vec::new(),
        Default::default(),
        Vec::new(),
        scene,
        0.0,
        false,
        "",
        false,
        false,
    );
    context.inspected = true;
    app.world_mut().entity_mut(scene).insert(context);
    let model = container(&mut app, scene, "model.glb", None);
    let removed = container(&mut app, scene, "removed.glb", None);
    app.update();

    command(&mut app, reload("b64-scene", "model.glb"));
    command(&mut app, remove("b64-scene", "removed.glb"));
    assert!(!changed_in_next_frame(&mut app).contains(&model));
    assert!(app.world().get::<GltfDefinition>(removed).is_some());
    assert!(app
        .world()
        .resource::<Toasts>()
        .0
        .contains_key("reload-inspected"));
    assert!(app
        .world()
        .resource::<LiveScenes>()
        .scenes
        .contains_key("b64-scene"));
}
