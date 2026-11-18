//! The LiveKit side of a presentation: a `cast-bot` process per session
//! (`services/catalyst/cast-bot`), which owns the session's only LiveKit
//! connection and publishes the slide track. The node never links
//! libwebrtc. The process speaks JSON lines, the way upstream drives its
//! `cast-sidecar` (`docs/architecture.md`, "Sidecar IPC"): commands on stdin,
//! events on stdout, logs on stderr.
//!
//! - `{"type":"init","url","token"}` is the first line, so the token is never
//!   on argv or in the environment. The bot connects and answers
//!   `{"type":"ready","roomMetadata","participantCount"}`, or
//!   `{"type":"error","code":"connect-failed","message"}` and exits.
//! - Later commands carry an `id` and get one `{"type":"ack","id"}` or
//!   `{"type":"error","id","code","message"}`: `publishVideo {width,height}`
//!   (a SCREENSHARE track named `presentation`, VP8, 6 Mbps, 30 fps, no
//!   simulcast, as upstream's rtc-node publisher), `frame {width,height,rgba}`
//!   (base64 RGBA; the bot repeats the last frame every 500 ms),
//!   `publishData {data}` (base64 bytes, reliable), `updateMetadata
//!   {metadata}` and `shutdown`. A command without an answer in 10 s fails.
//! - Events: `dataReceived {identity, payload}` (base64 packet),
//!   `roomMetadata {metadata}`, `participantCount {count}` (remote
//!   participants) and `disconnected {reason}`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{Mutex, mpsc, oneshot};

use super::render::Frame;

const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(25);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
const EXIT_WAIT: Duration = Duration::from_secs(3);
/// Room events queued for a session before the oldest waits.
const EVENT_QUEUE: usize = 256;

/// What the room tells a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BotEvent {
    /// A data packet and its sender's identity (original case).
    Data {
        identity: String,
        payload: Vec<u8>,
    },
    RoomMetadata(String),
    /// Remote participants in the room.
    ParticipantCount(usize),
    Disconnected(String),
}

/// The room as the bot found it on connect.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ready {
    pub room_metadata: String,
    pub participant_count: usize,
}

/// A failed connect: the token or URL was refused, or the bot did not start.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("LiveKit connect failed: {0}")]
pub struct ConnectError(pub String);

/// A connected bot.
#[async_trait::async_trait]
pub trait BotLink: Send + Sync {
    async fn publish_video(&self, width: u32, height: u32) -> anyhow::Result<()>;
    async fn push_frame(&self, frame: &Frame) -> anyhow::Result<()>;
    async fn publish_data(&self, packet: Vec<u8>) -> anyhow::Result<()>;
    async fn update_metadata(&self, metadata: String) -> anyhow::Result<()>;
    /// Disconnects and releases the bot; never fails.
    async fn shutdown(&self);
}

/// A connected bot, its events and the room it joined.
pub struct Connected {
    pub link: Arc<dyn BotLink>,
    pub events: mpsc::Receiver<BotEvent>,
    pub ready: Ready,
}

/// Starts bots.
#[async_trait::async_trait]
pub trait BotLauncher: Send + Sync {
    async fn connect(&self, url: &str, token: &str) -> Result<Connected, ConnectError>;
}

/// Starts `cast-bot` processes from a binary path.
pub struct ProcessLauncher {
    pub binary: PathBuf,
}

type Pending = Arc<std::sync::Mutex<HashMap<u64, oneshot::Sender<Result<(), String>>>>>;

struct ProcessLink {
    stdin: Mutex<Option<ChildStdin>>,
    child: Mutex<Option<Child>>,
    pending: Pending,
    next_id: AtomicU64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Line {
    #[serde(rename = "type")]
    kind: String,
    id: Option<u64>,
    message: Option<String>,
    identity: Option<String>,
    payload: Option<String>,
    metadata: Option<String>,
    room_metadata: Option<String>,
    participant_count: Option<usize>,
    count: Option<usize>,
    reason: Option<String>,
}

async fn write_line(stdin: &Mutex<Option<ChildStdin>>, line: &Value) -> anyhow::Result<()> {
    let mut text = serde_json::to_vec(line)?;
    text.push(b'\n');
    let mut guard = stdin.lock().await;
    let pipe = guard
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("bot stdin closed"))?;
    pipe.write_all(&text).await?;
    pipe.flush().await?;
    Ok(())
}

impl ProcessLink {
    async fn command(&self, mut command: Value) -> anyhow::Result<()> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        command["id"] = json!(id);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        if let Err(e) = write_line(&self.stdin, &command).await {
            self.pending.lock().unwrap().remove(&id);
            return Err(e);
        }
        match tokio::time::timeout(COMMAND_TIMEOUT, rx).await {
            Ok(Ok(Ok(()))) => Ok(()),
            Ok(Ok(Err(why))) => Err(anyhow::anyhow!(why)),
            Ok(Err(_)) => Err(anyhow::anyhow!("bot exited")),
            Err(_) => {
                self.pending.lock().unwrap().remove(&id);
                Err(anyhow::anyhow!("bot did not answer in time"))
            }
        }
    }
}

#[async_trait::async_trait]
impl BotLink for ProcessLink {
    async fn publish_video(&self, width: u32, height: u32) -> anyhow::Result<()> {
        self.command(json!({"type": "publishVideo", "width": width, "height": height}))
            .await
    }

    async fn push_frame(&self, frame: &Frame) -> anyhow::Result<()> {
        self.command(json!({
            "type": "frame",
            "width": frame.width,
            "height": frame.height,
            "rgba": BASE64.encode(frame.rgba.as_slice()),
        }))
        .await
    }

    async fn publish_data(&self, packet: Vec<u8>) -> anyhow::Result<()> {
        self.command(json!({"type": "publishData", "data": BASE64.encode(packet)}))
            .await
    }

    async fn update_metadata(&self, metadata: String) -> anyhow::Result<()> {
        self.command(json!({"type": "updateMetadata", "metadata": metadata}))
            .await
    }

    async fn shutdown(&self) {
        let _ =
            tokio::time::timeout(SHUTDOWN_TIMEOUT, self.command(json!({"type": "shutdown"}))).await;
        self.stdin.lock().await.take();
        if let Some(mut child) = self.child.lock().await.take()
            && tokio::time::timeout(EXIT_WAIT, child.wait()).await.is_err()
        {
            let _ = child.kill().await;
        }
    }
}

fn event_of(line: &Line) -> Option<BotEvent> {
    match line.kind.as_str() {
        "dataReceived" => Some(BotEvent::Data {
            identity: line.identity.clone().unwrap_or_else(|| "unknown".into()),
            payload: BASE64.decode(line.payload.as_deref()?).ok()?,
        }),
        "roomMetadata" => Some(BotEvent::RoomMetadata(
            line.metadata.clone().unwrap_or_default(),
        )),
        "participantCount" => Some(BotEvent::ParticipantCount(line.count.unwrap_or(0))),
        "disconnected" => Some(BotEvent::Disconnected(
            line.reason.clone().unwrap_or_default(),
        )),
        _ => None,
    }
}

#[async_trait::async_trait]
impl BotLauncher for ProcessLauncher {
    async fn connect(&self, url: &str, token: &str) -> Result<Connected, ConnectError> {
        let mut child = Command::new(&self.binary)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| ConnectError(format!("cannot start the bot: {e}")))?;
        let stdin = Mutex::new(child.stdin.take());
        let stdout = child.stdout.take().expect("piped stdout");
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    tracing::debug!(target: "cast_bot", "{line}");
                }
            });
        }
        let pending: Pending = Default::default();
        let (events_tx, events) = mpsc::channel(EVENT_QUEUE);
        let (ready_tx, ready_rx) = oneshot::channel::<Result<Ready, String>>();
        let reader_pending = pending.clone();
        tokio::spawn(async move {
            let mut ready_tx = Some(ready_tx);
            // A frame line is ~11 MB, but the bot only sends small events.
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(text)) = lines.next_line().await {
                let Ok(line) = serde_json::from_str::<Line>(&text) else {
                    tracing::warn!(target: "cast_bot", "unreadable bot line");
                    continue;
                };
                match (line.kind.as_str(), line.id) {
                    ("ready", _) => {
                        if let Some(tx) = ready_tx.take() {
                            let _ = tx.send(Ok(Ready {
                                room_metadata: line.room_metadata.clone().unwrap_or_default(),
                                participant_count: line.participant_count.unwrap_or(0),
                            }));
                        }
                    }
                    ("ack", Some(id)) => {
                        if let Some(tx) = reader_pending.lock().unwrap().remove(&id) {
                            let _ = tx.send(Ok(()));
                        }
                    }
                    ("error", Some(id)) => {
                        if let Some(tx) = reader_pending.lock().unwrap().remove(&id) {
                            let _ = tx.send(Err(line.message.clone().unwrap_or_default()));
                        }
                    }
                    ("error", None) => {
                        let why = line.message.clone().unwrap_or_default();
                        if let Some(tx) = ready_tx.take() {
                            let _ = tx.send(Err(why));
                        } else {
                            let _ = events_tx.send(BotEvent::Disconnected(why)).await;
                        }
                    }
                    _ => {
                        if let Some(event) = event_of(&line) {
                            let _ = events_tx.send(event).await;
                        }
                    }
                }
            }
            if let Some(tx) = ready_tx.take() {
                let _ = tx.send(Err("bot exited before connecting".into()));
            }
            // Fail what is still waiting, and tell the session the bot is gone.
            reader_pending.lock().unwrap().clear();
            let _ = events_tx
                .send(BotEvent::Disconnected("bot exited".into()))
                .await;
        });
        let link = ProcessLink {
            stdin,
            child: Mutex::new(Some(child)),
            pending,
            next_id: AtomicU64::new(1),
        };
        let init = json!({"type": "init", "url": url, "token": token});
        let ready = async {
            write_line(&link.stdin, &init)
                .await
                .map_err(|e| e.to_string())?;
            match tokio::time::timeout(CONNECT_TIMEOUT, ready_rx).await {
                Ok(Ok(result)) => result,
                Ok(Err(_)) => Err("bot exited before connecting".to_string()),
                Err(_) => Err("LiveKit connect timed out".to_string()),
            }
        }
        .await;
        match ready {
            Ok(ready) => Ok(Connected {
                link: Arc::new(link),
                events,
                ready,
            }),
            Err(why) => {
                link.shutdown().await;
                Err(ConnectError(why))
            }
        }
    }
}
