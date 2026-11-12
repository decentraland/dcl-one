use base64::{prelude::BASE64_STANDARD, Engine};
use bevy::{
    asset::io::memory::{Dir, MemoryAssetReader},
    ecs::system::SystemState,
};
use serde_json::{json, Value};

use super::*;

fn fixture() -> (World, Entity, Arc<IpfsIo>) {
    let mut world = World::new();
    world.init_resource::<Assets<EntityDefinition>>();
    let definition = world
        .resource_mut::<Assets<EntityDefinition>>()
        .add(EntityDefinition {
            id: "scene".to_owned(),
            metadata: Some(json!({
                "source": {"projectId": "retained-project"},
                "scene": {"base": "-7,13", "parcels": ["-6,13", "-7,13"]},
                "display": {"title": "Retained project"}
            })),
            ..Default::default()
        });
    let scene = world.spawn(SceneEntityDefinitionHandle(definition)).id();
    let io = Arc::new(IpfsIo::new(
        false,
        Box::new(MemoryAssetReader {
            root: Dir::default(),
        }),
        None,
        Default::default(),
        1,
        tokio::sync::mpsc::unbounded_channel().0,
    ));
    io.add_collection(
        "scene".to_owned(),
        ContentMap::from_content([
            (
                "scene.js".to_owned(),
                format!(
                    "b64-{}",
                    BASE64_STANDARD.encode("/workspace/project/scene.js-fixture")
                ),
            ),
            ("assets/imported/chair.glb".to_owned(), "chair".to_owned()),
        ]),
        None,
        None,
    );
    (world, scene, io)
}

#[test]
fn live_editor_target_keeps_identity_when_registry_metadata_is_unavailable() {
    let (mut world, scene, io) = fixture();
    let mut state = SystemState::<SceneMetadata>::new(&mut world);
    let snapshot = state.get(&world).snapshot(scene, "scene").unwrap();
    let target = bevy::tasks::block_on(scene_target_json(&io, "scene", snapshot)).unwrap();
    let target: Value = serde_json::from_str(&target).unwrap();
    assert_eq!(
        target,
        json!({
            "hash": "scene",
            "root": "/workspace/project",
            "projectId": "retained-project",
            "parcels": ["-6,13", "-7,13"],
            "title": "Retained project"
        })
    );
    assert_eq!(
        bevy::tasks::block_on(io.scene_content_files("scene")),
        ["assets/imported/chair.glb", "scene.js"]
    );
    assert!(bevy::tasks::block_on(io.scene_metadata("scene")).is_none());
}

#[test]
fn editor_target_reports_missing_or_invalid_fallback_metadata() {
    let (_, _, io) = fixture();
    let error = bevy::tasks::block_on(scene_target_json(&io, "scene", None)).unwrap_err();
    assert!(error.contains("unavailable"));
    io.add_collection(
        "scene".to_owned(),
        ContentMap::default(),
        None,
        Some("invalid JSON".to_owned()),
    );
    let error = bevy::tasks::block_on(scene_target_json(&io, "scene", None)).unwrap_err();
    assert!(error.contains("invalid scene metadata"));
}

#[test]
fn editor_target_without_an_owner_uses_available_registry_metadata() {
    let (mut world, scene, io) = fixture();
    world
        .entity_mut(scene)
        .remove::<SceneEntityDefinitionHandle>();
    io.add_collection(
        "scene".to_owned(),
        ContentMap::default(),
        None,
        Some(json!({"source": {"projectId": "fallback-project"}}).to_string()),
    );
    let mut state = SystemState::<SceneMetadata>::new(&mut world);
    let snapshot = state.get(&world).snapshot(scene, "scene").unwrap();
    assert!(snapshot.is_none());
    let target = bevy::tasks::block_on(scene_target_json(&io, "scene", snapshot)).unwrap();
    let target: Value = serde_json::from_str(&target).unwrap();
    assert_eq!(target["projectId"], "fallback-project");
}
