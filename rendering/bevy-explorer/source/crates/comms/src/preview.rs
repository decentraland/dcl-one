use std::{collections::HashSet, time::Duration};

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
use dcl_component::proto_components::sdk::development::{
    texture_optimization, ws_scene_message, TextureOptimization, WsSceneMessage,
};
use platform::IntoClientRequest;
use prost::Message;
use serde_json::json;
use system_bridge::{SystemApi, TextureOptimizationState, TextureOptimizationStatus};
use web_time::Instant;

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
/// arrives after it subscribed: a status is news once, and the replay rule is the socket's.
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

/// The status the HUD sees for a `TextureOptimization` message.
pub fn texture_optimization_status(
    message: &TextureOptimization,
    replay: bool,
) -> TextureOptimizationStatus {
    TextureOptimizationStatus {
        scene_id: message.scene_id.clone(),
        state: match message.state() {
            texture_optimization::State::TosRunning => TextureOptimizationState::Running,
            texture_optimization::State::TosReady => TextureOptimizationState::Ready,
            texture_optimization::State::TosFailed => TextureOptimizationState::Failed,
        },
        done: message.done,
        total: message.total,
        failed: message.failed,
        bytes_before: message.bytes_before as f64,
        bytes_after: message.bytes_after as f64,
        enabled: message.enabled,
        replay,
    }
}

/// How long after the socket opens a scene's first texture status still counts as the replay.
/// The server writes the replay before anything else, so it arrives at once.
const REPLAY_WINDOW: Duration = Duration::from_secs(2);

/// Tells the server's replay of its last texture status from news, per connection. A connecting
/// client is sent each project's last status first: the first status for a scene that arrives
/// within [`REPLAY_WINDOW`] of the socket opening is that replay. Every later one, and a first one
/// that arrives later, is news.
pub struct ReplayGate {
    opened: Instant,
    seen: HashSet<String>,
}

impl ReplayGate {
    pub fn new(opened: Instant) -> Self {
        Self {
            opened,
            seen: HashSet::new(),
        }
    }

    pub fn is_replay(&mut self, scene_id: &str, now: Instant) -> bool {
        let first = self.seen.insert(scene_id.to_owned());
        first && now.saturating_duration_since(self.opened) <= REPLAY_WINDOW
    }
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
    let mut replays = ReplayGate::new(Instant::now());

    let (_, mut read) = stream.split();

    while let Some(msg) = read.next().await {
        let msg = msg?;
        info!("preview server message: {msg}");
        let data = msg.into_data();
        let Ok(ws_scene_message) = WsSceneMessage::decode(data.as_slice()) else {
            debug!("Not a prost message.");
            continue;
        };
        let Some(message) = ws_scene_message.message else {
            debug!("Empty preview message.");
            continue;
        };

        if let ws_scene_message::Message::TextureOptimization(t) = &message {
            let replay = replays.is_replay(&t.scene_id, Instant::now());
            if let Some(textures) = &textures {
                textures.send(texture_optimization_status(t, replay))?;
            }
            continue;
        }

        if let Some(command) = PreviewCommand::from_message(message) {
            sender.send(command)?;
        }
    }

    warn!("preview socket disconnected, waiting 5 secs to attempt reconnect");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(state: texture_optimization::State) -> TextureOptimization {
        TextureOptimization {
            scene_id: "scene-1".into(),
            state: state as i32,
            done: 2,
            total: 3,
            failed: 1,
            bytes_before: 300,
            bytes_after: 200,
            enabled: true,
        }
    }

    #[test]
    fn a_texture_status_is_routed_and_never_reloads() {
        let msg = message(texture_optimization::State::TosReady);
        let wire = WsSceneMessage {
            message: Some(ws_scene_message::Message::TextureOptimization(msg.clone())),
        }
        .encode_to_vec();
        let decoded = WsSceneMessage::decode(wire.as_slice())
            .unwrap()
            .message
            .unwrap();
        assert!(PreviewCommand::from_message(decoded.clone()).is_none());
        let ws_scene_message::Message::TextureOptimization(t) = decoded else {
            panic!("not a texture status");
        };
        assert_eq!(
            texture_optimization_status(&t, false),
            TextureOptimizationStatus {
                scene_id: "scene-1".into(),
                state: TextureOptimizationState::Ready,
                done: 2,
                total: 3,
                failed: 1,
                bytes_before: 300.0,
                bytes_after: 200.0,
                enabled: true,
                replay: false,
            }
        );
    }

    #[test]
    fn every_wire_state_maps() {
        for (wire, state) in [
            (
                texture_optimization::State::TosRunning,
                TextureOptimizationState::Running,
            ),
            (
                texture_optimization::State::TosReady,
                TextureOptimizationState::Ready,
            ),
            (
                texture_optimization::State::TosFailed,
                TextureOptimizationState::Failed,
            ),
        ] {
            assert_eq!(
                texture_optimization_status(&message(wire), true).state,
                state
            );
        }
    }

    #[test]
    fn the_status_reaches_the_hud_as_camel_case_json() {
        let status =
            texture_optimization_status(&message(texture_optimization::State::TosRunning), true);
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
    fn only_the_first_status_per_scene_at_connect_is_a_replay() {
        let opened = Instant::now();
        let mut gate = ReplayGate::new(opened);
        assert!(gate.is_replay("a", opened));
        assert!(gate.is_replay("b", opened + Duration::from_millis(5)));
        assert!(!gate.is_replay("a", opened + Duration::from_millis(10)));
        assert!(!gate.is_replay("b", opened + Duration::from_millis(600)));
    }

    #[test]
    fn a_first_status_long_after_connect_is_news() {
        let opened = Instant::now();
        let mut gate = ReplayGate::new(opened);
        assert!(!gate.is_replay("a", opened + REPLAY_WINDOW + Duration::from_millis(1)));
        assert!(!gate.is_replay("a", opened + REPLAY_WINDOW * 2));
    }

    #[test]
    fn a_new_connection_replays_again() {
        let opened = Instant::now();
        let mut gate = ReplayGate::new(opened);
        assert!(gate.is_replay("a", opened));
        let mut reconnected = ReplayGate::new(opened + Duration::from_secs(30));
        assert!(reconnected.is_replay("a", opened + Duration::from_secs(30)));
    }

    #[test]
    fn the_debug_panel_explains_a_missing_status() {
        let empty = LastTextureOptimization::default();
        let v = texture_status_json(&empty, false);
        assert_eq!(v["received"], 0);
        assert_eq!(v["unavailable"][""], "Not connected to a preview server.");
        let filled = LastTextureOptimization {
            last: Some(texture_optimization_status(
                &message(texture_optimization::State::TosFailed),
                false,
            )),
            received: 4,
        };
        let v = texture_status_json(&filled, true);
        assert_eq!(v["state"], "failed");
        assert_eq!(v["textures"], json!([2, 3, 1]));
        assert_eq!(v["bytes"], json!([300.0, 200.0]));
        assert_eq!(v["received"], 4);
    }
}
