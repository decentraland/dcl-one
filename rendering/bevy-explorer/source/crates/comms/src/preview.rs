use std::time::Duration;

use anyhow::{anyhow, bail};
use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use common::{
    backoff::Backoff,
    structs::{CurrentRealm, PreviewCommand, PreviewMode},
    util::TaskExt,
};
use dcl_component::proto_components::sdk::development::WsSceneMessage;
use platform::IntoClientRequest;
use prost::Message;

pub struct PreviewPlugin;

impl Plugin for PreviewPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<PreviewCommand>()
            .add_systems(PostUpdate, connect_preview_server.after(ipfs::change_realm));
    }
}

/// The wait before a reconnect: 250 ms, doubling with each failed connection, at most 5 s.
const RECONNECT_START: Duration = Duration::from_millis(250);
const RECONNECT_CAP: Duration = Duration::from_secs(5);
/// A connection that ended sooner counts as a failure, so a server that accepts the upgrade
/// and closes at once does not reset the wait.
const MIN_SESSION: Duration = Duration::from_secs(1);

#[allow(clippy::type_complexity)]
fn connect_preview_server(
    mut preview: ResMut<PreviewMode>,
    mut failures: Local<u32>,
    mut retry_at: Local<Option<web_time::Instant>>,
    mut task: Local<
        Option<(
            Task<Result<Duration, anyhow::Error>>,
            tokio::sync::mpsc::UnboundedReceiver<PreviewCommand>,
        )>,
    >,
    current_realm: Res<CurrentRealm>,
    mut writer: EventWriter<PreviewCommand>,
) {
    let Some(server) = preview.server.as_ref() else {
        preview.is_preview = false;
        return;
    };

    if current_realm.is_changed() {
        *failures = 0;
        *retry_at = None;
        *task = None;
    }

    if &current_realm.address != server
        && current_realm.about_url.strip_suffix("/about") != Some(server)
    {
        preview.is_preview = false;
        return;
    }

    if let Some(result) = task.as_mut().and_then(|t| t.0.complete()) {
        *task = None;
        *failures = match result {
            Ok(lasted) if lasted >= MIN_SESSION => 0,
            Ok(lasted) => {
                warn!("preview socket closed after {lasted:?}");
                failures.saturating_add(1)
            }
            Err(err) => {
                warn!("preview socket error: {err}");
                failures.saturating_add(1)
            }
        };
        // wait after every exit: a preview server with no ws endpoint fails the upgrade
        // immediately, and an unpaused respawn would spin one attempt per frame
        *retry_at = Some(
            web_time::Instant::now()
                + Backoff::after(RECONNECT_START, RECONNECT_CAP, (*failures).max(1)),
        );
    }
    let due = match *retry_at {
        Some(at) => web_time::Instant::now() >= at,
        None => true,
    };
    if task.is_none() && due {
        *retry_at = None;
        let (sx, rx) = tokio::sync::mpsc::unbounded_channel();
        *task = Some((
            IoTaskPool::get().spawn(handle_preview_socket(server.clone(), sx)),
            rx,
        ));
    }

    while let Some(command) = task.as_mut().and_then(|(_, rx)| rx.try_recv().ok()) {
        writer.write(command);
    }

    preview.is_preview = true;
}

/// Connects to the preview server and forwards its messages until the socket ends.
/// Ok with the time the connection lasted, however it ended; Err when none was made.
pub async fn handle_preview_socket(
    server: String,
    sender: tokio::sync::mpsc::UnboundedSender<PreviewCommand>,
) -> Result<Duration, anyhow::Error> {
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
    let connected_at = web_time::Instant::now();
    debug!("preview socket connected");

    let (_, mut read) = stream.split();

    while let Some(msg) = read.next().await {
        let msg = match msg {
            Ok(msg) => msg,
            Err(err) => {
                warn!("preview socket error: {err}");
                break;
            }
        };
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

        if sender.send(message.into()).is_err() {
            break;
        }
    }

    warn!("preview socket disconnected");
    Ok(connected_at.elapsed())
}
