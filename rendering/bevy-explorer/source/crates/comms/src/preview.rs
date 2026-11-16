use std::time::Duration;

use anyhow::{anyhow, bail};
use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use bevy_console::ConsoleCommand;
use common::{
    rpc::RpcStreamSender,
    structs::{CurrentRealm, PreviewCommand, PreviewMode},
    util::TaskExt,
};
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use dcl_component::proto_components::sdk::development::WsSceneMessage;
use platform::IntoClientRequest;
use prost::Message;
use serde_json::json;
use system_bridge::{SystemApi, TextureOptimizationState, TextureOptimizationStatus};

pub struct PreviewPlugin;

impl Plugin for PreviewPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<PreviewCommand>()
            .add_event::<PreviewTextureOptimization>()
            .init_resource::<LastTextureOptimization>()
            .add_systems(PostUpdate, connect_preview_server.after(ipfs::change_realm))
            .add_systems(Update, pipe_texture_optimization_stream)
            .add_console_command::<PreviewTexturesCommand, _>(preview_textures_cmd)
            .add_debug_section(DebugSection {
                title: "Preview texture optimization",
                query: "/preview_textures",
                fields: &[
                    ("State", "state"),
                    ("Optimized files served", "enabled"),
                    ("Textures (done / total / failed)", "textures"),
                    ("Bytes (source / KTX2)", "bytes"),
                    ("Replayed at connect", "replay"),
                    ("Statuses received", "received"),
                ],
                actions: &[],
                inputs: &[],
            });
    }
}

/// A texture import status the preview socket delivered, on its way to the HUD.
#[derive(Event, Clone, Debug)]
pub struct PreviewTextureOptimization(pub TextureOptimizationStatus);

/// The last texture import status and how many arrived, for the debug panel.
#[derive(Resource, Default)]
pub struct LastTextureOptimization {
    pub last: Option<TextureOptimizationStatus>,
    pub received: u32,
}

#[allow(clippy::type_complexity)]
fn connect_preview_server(
    mut preview: ResMut<PreviewMode>,
    mut task: Local<
        Option<(
            Task<Result<(), anyhow::Error>>,
            tokio::sync::mpsc::UnboundedReceiver<PreviewCommand>,
            tokio::sync::mpsc::UnboundedReceiver<TextureOptimizationStatus>,
        )>,
    >,
    current_realm: Res<CurrentRealm>,
    mut writer: EventWriter<PreviewCommand>,
    mut textures: EventWriter<PreviewTextureOptimization>,
) {
    let Some(server) = preview.server.as_ref() else {
        preview.is_preview = false;
        return;
    };

    if current_realm.is_changed() {
        *task = None;
    }

    if &current_realm.address != server
        && current_realm.about_url.strip_suffix("/about") != Some(server)
    {
        preview.is_preview = false;
        return;
    }

    let mut restart = task.as_ref().is_none();
    if let Some(res) = task.as_mut().and_then(|t| t.0.complete()) {
        if let Err(err) = res {
            warn!("preview socket error: {err}, restarting");
        }
        restart = true;
        *task = None;
    }
    if restart {
        let (sx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (texture_sx, texture_rx) = tokio::sync::mpsc::unbounded_channel();
        *task = Some((
            IoTaskPool::get().spawn(handle_preview_socket(server.clone(), sx, Some(texture_sx))),
            rx,
            texture_rx,
        ));
    }

    while let Some(command) = task.as_mut().and_then(|(_, rx, _)| rx.try_recv().ok()) {
        writer.write(command);
    }
    while let Some(status) = task.as_mut().and_then(|(_, _, rx)| rx.try_recv().ok()) {
        textures.write(PreviewTextureOptimization(status));
    }

    preview.is_preview = true;
}

/// Hands each texture import status to every `GetTextureOptimizationStream` subscriber (the HUD's
/// bridge scene) and keeps the last one for the debug panel. A new subscriber gets only what
/// arrives after it subscribed: a status is news once, and the preview server marks its replays.
fn pipe_texture_optimization_stream(
    mut requests: EventReader<SystemApi>,
    mut statuses: EventReader<PreviewTextureOptimization>,
    mut senders: Local<Vec<RpcStreamSender<TextureOptimizationStatus>>>,
    mut last: ResMut<LastTextureOptimization>,
) {
    senders.extend(requests.read().filter_map(|ev| match ev {
        SystemApi::GetTextureOptimizationStream(sender) => Some(sender.clone()),
        _ => None,
    }));
    senders.retain(|s| !s.is_closed());

    for PreviewTextureOptimization(status) in statuses.read() {
        for sender in senders.iter() {
            let _ = sender.send(status.clone());
        }
        last.last = Some(status.clone());
        last.received += 1;
    }
}

/// The last texture import status the preview server sent, for the debug panel.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/preview_textures")]
struct PreviewTexturesCommand;

fn preview_textures_cmd(
    mut input: ConsoleCommand<PreviewTexturesCommand>,
    last: Res<LastTextureOptimization>,
    preview: Res<PreviewMode>,
) {
    if input.take().is_none() {
        return;
    }
    input.reply_ok(texture_status_json(&last, preview.is_preview).to_string());
}

fn texture_status_json(last: &LastTextureOptimization, is_preview: bool) -> serde_json::Value {
    let unavailable = if is_preview {
        "The preview server has sent no texture status yet: it sends one while it imports and when an import changes the served files."
    } else {
        "Not connected to a preview server."
    };
    match &last.last {
        Some(s) => json!({
            "state": s.state,
            "enabled": s.enabled,
            "textures": [s.done, s.total, s.failed],
            "bytes": [s.bytes_before, s.bytes_after],
            "replay": s.replay,
            "received": last.received,
        }),
        None => json!({
            "received": last.received,
            "unavailable": { "": unavailable },
        }),
    }
}

/// The payload of the preview server's `TEXTURE_OPTIMIZATION` text frame.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TextureOptimizationWire {
    scene_id: String,
    state: String,
    done: u32,
    total: u32,
    failed: u32,
    bytes_before: u64,
    bytes_after: u64,
    enabled: bool,
    // set by the preview server on the last status it sends a client as it connects
    #[serde(default)]
    replay: bool,
}

/// The status the HUD sees for a preview text frame, when it is the texture import's
/// `{"type":"TEXTURE_OPTIMIZATION","payload":{...}}`; `None` for any other text frame. The status
/// is never a binary `WsSceneMessage`: the Unity explorer reads every binary frame it can't name
/// as a scene update.
pub fn texture_optimization_status(text: &str) -> Option<TextureOptimizationStatus> {
    #[derive(serde::Deserialize)]
    struct Frame {
        #[serde(rename = "type")]
        kind: String,
        payload: Option<serde_json::Value>,
    }
    let frame: Frame = serde_json::from_str(text).ok()?;
    if frame.kind != "TEXTURE_OPTIMIZATION" {
        return None;
    }
    let wire: TextureOptimizationWire = serde_json::from_value(frame.payload?).ok()?;
    let state = match wire.state.as_str() {
        "RUNNING" => TextureOptimizationState::Running,
        "READY" => TextureOptimizationState::Ready,
        "FAILED" => TextureOptimizationState::Failed,
        _ => return None,
    };
    Some(TextureOptimizationStatus {
        scene_id: wire.scene_id,
        state,
        done: wire.done,
        total: wire.total,
        failed: wire.failed,
        bytes_before: wire.bytes_before as f64,
        bytes_after: wire.bytes_after as f64,
        enabled: wire.enabled,
        replay: wire.replay,
    })
}

/// Runs the preview socket until it closes, then backs off. Reloads go to `sender`; texture import
/// statuses go to `textures` when given, and are dropped otherwise.
pub async fn handle_preview_socket(
    server: String,
    sender: tokio::sync::mpsc::UnboundedSender<PreviewCommand>,
    textures: Option<tokio::sync::mpsc::UnboundedSender<TextureOptimizationStatus>>,
) -> Result<(), anyhow::Error> {
    let result = preview_socket(server, sender, textures).await;
    // back off on every exit path, not just a clean disconnect: a preview server with
    // no ws endpoint fails the upgrade immediately and the caller respawns us as soon
    // as the task completes, so an early error spins one attempt per frame
    async_std::task::sleep(Duration::from_secs(5)).await;
    result
}

async fn preview_socket(
    server: String,
    sender: tokio::sync::mpsc::UnboundedSender<PreviewCommand>,
    textures: Option<tokio::sync::mpsc::UnboundedSender<TextureOptimizationStatus>>,
) -> Result<(), anyhow::Error> {
    let (protocol, rest) = server
        .split_once("//")
        .ok_or(anyhow!("invalid preview server address `{server}`"))?;
    let remote_address = if protocol == "http:" {
        format!("ws://{rest}")
    } else if protocol == "https:" {
        format!("wss://{rest}")
    } else {
        bail!("invalid preview server protocol `{protocol}` from `{server}`");
    };

    let request = remote_address.into_client_request()?;
    let stream = platform::websocket(request).await?;
    debug!("preview socket connected");

    let (_, mut read) = stream.split();

    while let Some(msg) = read.next().await {
        let msg = msg?;
        info!("preview server message: {msg}");
        // text frames are the JSON twins of the scene updates, and the texture import's status,
        // which comes as text alone
        if msg.is_text() {
            if let (Some(textures), Some(status)) =
                (&textures, texture_optimization_status(msg.to_text()?))
            {
                textures.send(status)?;
            }
            continue;
        }
        let data = msg.into_data();
        let Ok(ws_scene_message) = WsSceneMessage::decode(data.as_slice()) else {
            debug!("Not a prost message.");
            continue;
        };
        let Some(message) = ws_scene_message.message else {
            debug!("Empty preview message.");
            continue;
        };

        sender.send(message.into())?;
    }

    warn!("preview socket disconnected, waiting 5 secs to attempt reconnect");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(state: &str, replay: bool) -> String {
        json!({
            "type": "TEXTURE_OPTIMIZATION",
            "payload": {
                "sceneId": "scene-1",
                "state": state,
                "done": 2,
                "total": 3,
                "failed": 1,
                "bytesBefore": 300,
                "bytesAfter": 200,
                "enabled": true,
                "replay": replay,
            }
        })
        .to_string()
    }

    #[test]
    fn a_texture_status_text_frame_maps_to_the_hud_status() {
        assert_eq!(
            texture_optimization_status(&frame("READY", false)),
            Some(TextureOptimizationStatus {
                scene_id: "scene-1".into(),
                state: TextureOptimizationState::Ready,
                done: 2,
                total: 3,
                failed: 1,
                bytes_before: 300.0,
                bytes_after: 200.0,
                enabled: true,
                replay: false,
            })
        );
    }

    #[test]
    fn every_wire_state_maps() {
        for (wire, state) in [
            ("RUNNING", TextureOptimizationState::Running),
            ("READY", TextureOptimizationState::Ready),
            ("FAILED", TextureOptimizationState::Failed),
        ] {
            assert_eq!(
                texture_optimization_status(&frame(wire, false))
                    .unwrap()
                    .state,
                state
            );
        }
        assert_eq!(texture_optimization_status(&frame("DONE", false)), None);
    }

    #[test]
    fn other_text_frames_are_not_a_status() {
        let scene_update =
            json!({"type": "SCENE_UPDATE", "payload": {"sceneId": "s", "sceneType": "scene"}});
        assert_eq!(texture_optimization_status(&scene_update.to_string()), None);
        assert_eq!(texture_optimization_status("not json"), None);
        // a server that predates `replay` sends none: not a replay
        let mut old: serde_json::Value = serde_json::from_str(&frame("READY", true)).unwrap();
        old["payload"].as_object_mut().unwrap().remove("replay");
        assert!(
            !texture_optimization_status(&old.to_string())
                .unwrap()
                .replay
        );
    }

    #[test]
    fn the_status_reaches_the_hud_as_camel_case_json() {
        let status = texture_optimization_status(&frame("RUNNING", true)).unwrap();
        assert_eq!(
            serde_json::to_value(&status).unwrap(),
            json!({
                "sceneId": "scene-1",
                "state": "running",
                "done": 2,
                "total": 3,
                "failed": 1,
                "bytesBefore": 300.0,
                "bytesAfter": 200.0,
                "enabled": true,
                "replay": true,
            })
        );
    }

    #[test]
    fn the_debug_panel_explains_a_missing_status() {
        let empty = LastTextureOptimization::default();
        let v = texture_status_json(&empty, false);
        assert_eq!(v["received"], 0);
        assert_eq!(v["unavailable"][""], "Not connected to a preview server.");
        let filled = LastTextureOptimization {
            last: texture_optimization_status(&frame("FAILED", false)),
            received: 4,
        };
        let v = texture_status_json(&filled, true);
        assert_eq!(v["state"], "failed");
        assert_eq!(v["textures"], json!([2, 3, 1]));
        assert_eq!(v["bytes"], json!([300.0, 200.0]));
        assert_eq!(v["received"], 4);
    }
}
