use crate::scene::b64_content_hash;
use prost::Message;
use serde_json::json;
use std::path::{Path, PathBuf};

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/decentraland.sdk.development.rs"));
}

use crate::textures::import::{ImportState, ImportStatus};
use proto::{
    TextureOptimization, UpdateModel, UpdateModelType, UpdateScene, WsSceneMessage,
    texture_optimization, ws_scene_message,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReloadEvent {
    Scene,
    Model { path: PathBuf, removed: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReloadFrame {
    Text(String),
    Binary(Vec<u8>),
}

pub fn scene_update_json(scene_id: &str) -> String {
    json!({
        "type": "SCENE_UPDATE",
        "payload": { "sceneId": scene_id, "sceneType": "scene" }
    })
    .to_string()
}

fn frame(message: ws_scene_message::Message) -> Vec<u8> {
    WsSceneMessage {
        message: Some(message),
    }
    .encode_to_vec()
}

pub fn update_scene_frame(scene_id: &str) -> Vec<u8> {
    frame(ws_scene_message::Message::UpdateScene(UpdateScene {
        scene_id: scene_id.to_string(),
    }))
}

pub fn update_model_frame(scene_id: &str, src: &str, hash: &str, removed: bool) -> Vec<u8> {
    let r#type = if removed {
        UpdateModelType::UmtRemove
    } else {
        UpdateModelType::UmtChange
    };
    frame(ws_scene_message::Message::UpdateModel(UpdateModel {
        scene_id: scene_id.to_string(),
        src: src.to_string(),
        hash: hash.to_string(),
        r#type: r#type as i32,
    }))
}

fn proto_state(state: ImportState) -> texture_optimization::State {
    match state {
        ImportState::Running => texture_optimization::State::TosRunning,
        ImportState::Ready => texture_optimization::State::TosReady,
        ImportState::Failed => texture_optimization::State::TosFailed,
    }
}

fn state_name(state: ImportState) -> &'static str {
    match state {
        ImportState::Running => "RUNNING",
        ImportState::Ready => "READY",
        ImportState::Failed => "FAILED",
    }
}

pub fn texture_optimization_frame(scene_id: &str, status: &ImportStatus) -> Vec<u8> {
    frame(ws_scene_message::Message::TextureOptimization(
        TextureOptimization {
            scene_id: scene_id.to_string(),
            state: proto_state(status.state) as i32,
            done: status.done,
            total: status.total,
            failed: status.failed,
            bytes_before: status.bytes_before,
            bytes_after: status.bytes_after,
            enabled: status.enabled,
        },
    ))
}

/// The JSON twin of [`texture_optimization_frame`], as `SCENE_UPDATE` has
/// one; `state` is `RUNNING`, `READY` or `FAILED`.
pub fn texture_optimization_json(scene_id: &str, status: &ImportStatus) -> String {
    json!({
        "type": "TEXTURE_OPTIMIZATION",
        "payload": {
            "sceneId": scene_id,
            "state": state_name(status.state),
            "done": status.done,
            "total": status.total,
            "failed": status.failed,
            "bytesBefore": status.bytes_before,
            "bytesAfter": status.bytes_after,
            "enabled": status.enabled,
        }
    })
    .to_string()
}

/// The background texture import's status: a text frame for clients that
/// read JSON, then the `WsSceneMessage`. Reloads nothing by itself.
pub fn texture_optimization_frames(scene_id: &str, status: &ImportStatus) -> Vec<ReloadFrame> {
    vec![
        ReloadFrame::Text(texture_optimization_json(scene_id, status)),
        ReloadFrame::Binary(texture_optimization_frame(scene_id, status)),
    ]
}

pub fn model_src(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub fn reload_frames(
    root: &Path,
    scene_id: &str,
    machine: &str,
    event: &ReloadEvent,
) -> Vec<ReloadFrame> {
    let binary = match event {
        ReloadEvent::Scene => update_scene_frame(scene_id),
        ReloadEvent::Model { path, removed } => {
            let src = model_src(root, path);
            // a model the texture overlay rewrote is served from its stage
            let hash = match crate::textures::staged_source(root, &src) {
                Some(staged) => crate::scene::b64_content_hash_in_root(
                    &crate::scene::root_tag(root, machine),
                    &src,
                    &staged,
                ),
                None => b64_content_hash(&root.join(&src).display().to_string(), machine),
            };
            update_model_frame(scene_id, &src, &hash, *removed)
        }
    };
    vec![
        ReloadFrame::Text(scene_update_json(scene_id)),
        ReloadFrame::Binary(binary),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_model(bytes: &[u8]) -> UpdateModel {
        match WsSceneMessage::decode(bytes).unwrap().message {
            Some(ws_scene_message::Message::UpdateModel(u)) => u,
            other => panic!("unexpected oneof: {other:?}"),
        }
    }

    fn model_frames(removed: bool) -> Vec<ReloadFrame> {
        let root = Path::new("/proj");
        let event = ReloadEvent::Model {
            path: root.join("assets/tree.glb"),
            removed,
        };
        reload_frames(root, "scene-1", "host", &event)
    }

    fn binary_frame(frames: &[ReloadFrame]) -> &[u8] {
        assert_eq!(frames.len(), 2);
        assert!(matches!(&frames[0], ReloadFrame::Text(t) if t.contains("SCENE_UPDATE")));
        let ReloadFrame::Binary(bytes) = &frames[1] else {
            panic!("second frame must be binary");
        };
        bytes
    }

    #[test]
    fn update_scene_golden_bytes() {
        let bytes = update_scene_frame("abc");
        assert_eq!(bytes, vec![0x0a, 0x05, 0x0a, 0x03, 0x61, 0x62, 0x63]);
    }

    #[test]
    fn update_scene_round_trip() {
        let bytes = update_scene_frame("b64-c2NlbmU=");
        let decoded = WsSceneMessage::decode(bytes.as_slice()).unwrap();
        match decoded.message {
            Some(ws_scene_message::Message::UpdateScene(u)) => {
                assert_eq!(u.scene_id, "b64-c2NlbmU=");
            }
            other => panic!("unexpected oneof: {other:?}"),
        }
    }

    #[test]
    fn update_model_round_trip() {
        let u = decode_model(&update_model_frame(
            "scene-1",
            "assets/tree.glb",
            "b64-aGFzaA==",
            false,
        ));
        assert_eq!(u.scene_id, "scene-1");
        assert_eq!(u.src, "assets/tree.glb");
        assert_eq!(u.hash, "b64-aGFzaA==");
        assert_eq!(u.r#type, UpdateModelType::UmtChange as i32);
        assert_eq!(u.r#type(), UpdateModelType::UmtChange);
    }

    #[test]
    fn removed_model_round_trips_as_umt_remove() {
        let u = decode_model(&update_model_frame(
            "scene-1",
            "assets/tree.glb",
            "b64-aGFzaA==",
            true,
        ));
        assert_eq!(u.r#type(), UpdateModelType::UmtRemove);
        assert_eq!(u.src, "assets/tree.glb");
    }

    #[test]
    fn scene_frames_text_first_and_unchanged() {
        let frames = reload_frames(Path::new("/proj"), "scene-1", "host", &ReloadEvent::Scene);
        assert_eq!(frames.len(), 2);
        let expected_json = json!({
            "type": "SCENE_UPDATE",
            "payload": { "sceneId": "scene-1", "sceneType": "scene" }
        })
        .to_string();
        assert_eq!(frames[0], ReloadFrame::Text(expected_json));
        assert_eq!(
            frames[1],
            ReloadFrame::Binary(update_scene_frame("scene-1"))
        );
    }

    #[test]
    fn model_frames_use_relative_src_and_the_content_mapping_hash() {
        let frames = model_frames(false);
        let u = decode_model(binary_frame(&frames));
        assert_eq!(u.src, "assets/tree.glb");
        assert_eq!(
            u.hash,
            crate::scene::b64_content_hash("/proj/assets/tree.glb", "host")
        );
        assert_eq!(u.scene_id, "scene-1");
        assert_eq!(u.r#type(), UpdateModelType::UmtChange);
    }

    fn status(state: ImportState) -> ImportStatus {
        ImportStatus {
            state,
            done: 2,
            total: 3,
            failed: 1,
            bytes_before: 300,
            bytes_after: 200,
            enabled: true,
        }
    }

    #[test]
    fn texture_optimization_golden_bytes() {
        let bytes = texture_optimization_frame("s", &status(ImportState::Ready));
        assert_eq!(
            bytes,
            vec![
                0x1a, 0x13, // field 3 of the oneof, 19 bytes
                0x0a, 0x01, 0x73, // scene_id "s"
                0x10, 0x01, // state TOS_READY
                0x18, 0x02, // done
                0x20, 0x03, // total
                0x28, 0x01, // failed
                0x30, 0xac, 0x02, // bytes_before 300
                0x38, 0xc8, 0x01, // bytes_after 200
                0x40, 0x01, // enabled
            ]
        );
    }

    #[test]
    fn texture_optimization_round_trips_every_state() {
        for (state, wire) in [
            (ImportState::Running, texture_optimization::State::TosRunning),
            (ImportState::Ready, texture_optimization::State::TosReady),
            (ImportState::Failed, texture_optimization::State::TosFailed),
        ] {
            let bytes = texture_optimization_frame("scene-1", &status(state));
            let t = match WsSceneMessage::decode(bytes.as_slice()).unwrap().message {
                Some(ws_scene_message::Message::TextureOptimization(t)) => t,
                other => panic!("unexpected oneof: {other:?}"),
            };
            assert_eq!(t.scene_id, "scene-1");
            assert_eq!(t.state(), wire);
            assert_eq!((t.done, t.total, t.failed), (2, 3, 1));
            assert_eq!((t.bytes_before, t.bytes_after), (300, 200));
            assert!(t.enabled);
        }
    }

    #[test]
    fn texture_optimization_json_mirrors_the_message() {
        let frames = texture_optimization_frames("scene-1", &status(ImportState::Running));
        assert_eq!(frames.len(), 2);
        let ReloadFrame::Text(text) = &frames[0] else {
            panic!("the JSON frame goes first");
        };
        let v: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(
            v,
            json!({
                "type": "TEXTURE_OPTIMIZATION",
                "payload": {
                    "sceneId": "scene-1",
                    "state": "RUNNING",
                    "done": 2,
                    "total": 3,
                    "failed": 1,
                    "bytesBefore": 300,
                    "bytesAfter": 200,
                    "enabled": true
                }
            })
        );
        assert_eq!(
            frames[1],
            ReloadFrame::Binary(texture_optimization_frame(
                "scene-1",
                &status(ImportState::Running)
            ))
        );
        let failed = texture_optimization_json("x", &status(ImportState::Failed));
        assert!(failed.contains("\"state\":\"FAILED\""), "{failed}");
    }

    #[test]
    fn removed_model_frames_keep_the_legacy_text_frame() {
        let frames = model_frames(true);
        let u = decode_model(binary_frame(&frames));
        assert_eq!(u.r#type(), UpdateModelType::UmtRemove);
    }
}
