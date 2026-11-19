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
//!   `{"type":"error","code","message"}` and exits: `connect-failed` when
//!   LiveKit refused the token, `connect-unavailable` when it could not be
//!   reached or did not answer.
//! - Later commands carry an `id` and get one `{"type":"ack","id"}` or
//!   `{"type":"error","id","code","message"}`: `publishVideo {width,height}`
//!   (a SCREENSHARE track named `presentation`, VP8, 6 Mbps, 30 fps, no
//!   simulcast, as upstream's rtc-node publisher), `frame {width,height,rgba}`
//!   (base64 RGBA; the bot repeats the last frame every 500 ms),
//!   `publishData {data}` (base64 bytes, reliable), `updateMetadata
//!   {metadata}` and `shutdown`. A command without an answer in 10 s fails.
//! - Events: `dataReceived {identity, payload}` (base64 packet; only from
//!   the room metadata's presenters and carrying `presentation:`, so scene
//!   traffic never crosses), `roomMetadata {metadata}`, `participantCount
//!   {count}` (remote participants) and `disconnected {reason}`, after which
//!   the bot exits.
//!
//! One reader takes the bot's stdout and never waits on the session: replies
//! complete their commands at once, the room's metadata and participant
//! count are kept as the latest value ([`RoomState`]), and data packets queue
//! up to [`DATA_QUEUE`]; a packet that finds the queue full is dropped and
//! counted. The session sees the bot gone when the queue closes.

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
use tokio::sync::{Mutex, mpsc, oneshot, watch};

use super::render::Frame;

const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(25);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
const EXIT_WAIT: Duration = Duration::from_secs(3);
/// Data packets queued for a session; more are dropped.
pub const DATA_QUEUE: usize = 256;

/// A data packet and its sender's identity (original case).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomData {
    pub identity: String,
    pub payload: Vec<u8>,
}

/// The room as the bot last reported it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RoomState {
    pub metadata: String,
    /// Remote participants in the room.
    pub participants: usize,
}

/// A failed connect: the bot refused the token, or it did not start or answer.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum ConnectError {
    /// LiveKit refused the connection (the bot's `connect-failed`).
    #[error("LiveKit connect failed: {0}")]
    Refused(String),
    /// The bot did not start, exited or did not answer in time.
    #[error("cast-bot unavailable: {0}")]
    Unavailable(String),
}

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

/// A connected bot, the room's data packets (closed when the bot is gone)
/// and the room's state (as of connect, then each change).
pub struct Connected {
    pub link: Arc<dyn BotLink>,
    pub data: mpsc::Receiver<RoomData>,
    pub room: watch::Receiver<RoomState>,
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
    code: Option<String>,
    message: Option<String>,
    identity: Option<String>,
    payload: Option<String>,
    metadata: Option<String>,
    room_metadata: Option<String>,
    participant_count: Option<usize>,
    count: Option<usize>,
    reason: Option<String>,
}

async fn write_line(stdin: &Mutex<Option<ChildStdin>>, line: &[u8]) -> anyhow::Result<()> {
    let mut guard = stdin.lock().await;
    let pipe = guard
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("bot stdin closed"))?;
    pipe.write_all(line).await?;
    pipe.flush().await?;
    Ok(())
}

fn json_line(value: &Value) -> anyhow::Result<Vec<u8>> {
    let mut line = serde_json::to_vec(value)?;
    line.push(b'\n');
    Ok(line)
}

/// A `frame` command line: ~11 MB for 1920x1080, built off the runtime.
fn frame_line(id: u64, frame: &Frame) -> Vec<u8> {
    let rgba = frame.rgba.as_slice();
    let mut line = format!(
        r#"{{"type":"frame","id":{id},"width":{},"height":{},"rgba":""#,
        frame.width, frame.height
    )
    .into_bytes();
    // Base64 needs no JSON escaping; encoded in place, with no second copy.
    let start = line.len();
    line.resize(start + rgba.len().div_ceil(3) * 4, 0);
    let written = BASE64
        .encode_slice(rgba, &mut line[start..])
        .expect("sized for the encoding");
    line.truncate(start + written);
    line.extend_from_slice(b"\"}\n");
    line
}

impl ProcessLink {
    fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Writes command `id`'s line and waits for its answer.
    async fn request(&self, id: u64, line: &[u8]) -> anyhow::Result<()> {
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        if let Err(e) = write_line(&self.stdin, line).await {
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

    async fn command(&self, mut command: Value) -> anyhow::Result<()> {
        let id = self.next_id();
        command["id"] = json!(id);
        self.request(id, &json_line(&command)?).await
    }
}

#[async_trait::async_trait]
impl BotLink for ProcessLink {
    async fn publish_video(&self, width: u32, height: u32) -> anyhow::Result<()> {
        self.command(json!({"type": "publishVideo", "width": width, "height": height}))
            .await
    }

    async fn push_frame(&self, frame: &Frame) -> anyhow::Result<()> {
        let id = self.next_id();
        let frame = frame.clone();
        let line = tokio::task::spawn_blocking(move || frame_line(id, &frame)).await?;
        self.request(id, &line).await
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

/// Reads the bot's stdout until it closes; never waits on the session.
fn spawn_reader(
    stdout: tokio::process::ChildStdout,
    pending: Pending,
    ready_tx: oneshot::Sender<Result<(), ConnectError>>,
    data_tx: mpsc::Sender<RoomData>,
    room_tx: watch::Sender<RoomState>,
) {
    crate::shared::task::spawn(async move {
        let mut ready_tx = Some(ready_tx);
        let mut dropped: u64 = 0;
        // A frame line is ~11 MB, but the bot only sends small lines.
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(text)) = lines.next_line().await {
            let Ok(line) = serde_json::from_str::<Line>(&text) else {
                tracing::warn!(target: "cast_bot", "unreadable bot line");
                continue;
            };
            match (line.kind.as_str(), line.id) {
                ("ready", _) => {
                    room_tx.send_replace(RoomState {
                        metadata: line.room_metadata.unwrap_or_default(),
                        participants: line.participant_count.unwrap_or(0),
                    });
                    if let Some(tx) = ready_tx.take() {
                        let _ = tx.send(Ok(()));
                    }
                }
                ("ack", Some(id)) => {
                    if let Some(tx) = pending.lock().unwrap().remove(&id) {
                        let _ = tx.send(Ok(()));
                    }
                }
                ("error", Some(id)) => {
                    if let Some(tx) = pending.lock().unwrap().remove(&id) {
                        let _ = tx.send(Err(line.message.unwrap_or_default()));
                    }
                }
                ("error", None) => {
                    let why = line.message.unwrap_or_default();
                    match ready_tx.take() {
                        Some(tx) if line.code.as_deref() == Some("connect-failed") => {
                            let _ = tx.send(Err(ConnectError::Refused(why)));
                        }
                        Some(tx) => {
                            let _ = tx.send(Err(ConnectError::Unavailable(why)));
                        }
                        None => tracing::warn!(target: "cast_bot", "bot failed: {why}"),
                    }
                }
                ("dataReceived", _) => {
                    let Some(payload) = line.payload.and_then(|p| BASE64.decode(p).ok()) else {
                        continue;
                    };
                    let data = RoomData {
                        identity: line.identity.unwrap_or_else(|| "unknown".into()),
                        payload,
                    };
                    if let Err(mpsc::error::TrySendError::Full(_)) = data_tx.try_send(data) {
                        dropped += 1;
                        if dropped.is_power_of_two() {
                            tracing::warn!(target: "cast_bot", dropped, "session busy, data packets dropped");
                        }
                    }
                }
                ("roomMetadata", _) => {
                    let metadata = line.metadata.unwrap_or_default();
                    room_tx.send_modify(|room| room.metadata = metadata);
                }
                ("participantCount", _) => {
                    let count = line.count.unwrap_or(0);
                    room_tx.send_modify(|room| room.participants = count);
                }
                ("disconnected", _) => {
                    let reason = line.reason.unwrap_or_default();
                    tracing::warn!(target: "cast_bot", %reason, "bot disconnected from LiveKit");
                }
                _ => {}
            }
        }
        if let Some(tx) = ready_tx.take() {
            let _ = tx.send(Err(ConnectError::Unavailable(
                "bot exited before connecting".into(),
            )));
        }
        // Fail what is still waiting; dropping the senders tells the session.
        pending.lock().unwrap().clear();
    });
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
            .map_err(|e| ConnectError::Unavailable(format!("cannot start the bot: {e}")))?;
        let stdin = Mutex::new(child.stdin.take());
        let stdout = child.stdout.take().expect("piped stdout");
        if let Some(stderr) = child.stderr.take() {
            crate::shared::task::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    tracing::debug!(target: "cast_bot", "{line}");
                }
            });
        }
        let pending: Pending = Default::default();
        let (data_tx, data) = mpsc::channel(DATA_QUEUE);
        let (room_tx, room) = watch::channel(RoomState::default());
        let (ready_tx, ready_rx) = oneshot::channel();
        spawn_reader(stdout, pending.clone(), ready_tx, data_tx, room_tx);
        let link = ProcessLink {
            stdin,
            child: Mutex::new(Some(child)),
            pending,
            next_id: AtomicU64::new(1),
        };
        let ready = async {
            let init = json_line(&json!({"type": "init", "url": url, "token": token}))
                .map_err(|e| ConnectError::Unavailable(e.to_string()))?;
            write_line(&link.stdin, &init)
                .await
                .map_err(|e| ConnectError::Unavailable(e.to_string()))?;
            match tokio::time::timeout(CONNECT_TIMEOUT, ready_rx).await {
                Ok(Ok(result)) => result,
                Ok(Err(_)) => Err(ConnectError::Unavailable(
                    "bot exited before connecting".into(),
                )),
                Err(_) => Err(ConnectError::Unavailable(
                    "LiveKit connect timed out".into(),
                )),
            }
        }
        .await;
        match ready {
            Ok(()) => Ok(Connected {
                link: Arc::new(link),
                data,
                room,
            }),
            Err(why) => {
                link.shutdown().await;
                Err(why)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_line_is_the_json_command() {
        let frame = Frame {
            rgba: Arc::new(vec![1, 2, 3, 4, 5, 6, 7, 8]),
            width: 2,
            height: 1,
        };
        let line = frame_line(7, &frame);
        assert_eq!(line.last(), Some(&b'\n'));
        let value: Value = serde_json::from_slice(&line).unwrap();
        assert_eq!(
            value,
            json!({"type": "frame", "id": 7, "width": 2, "height": 1,
                   "rgba": BASE64.encode([1, 2, 3, 4, 5, 6, 7, 8])})
        );
    }
}
