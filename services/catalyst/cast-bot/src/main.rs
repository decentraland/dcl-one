//! `cast-bot`: the LiveKit participant of one Cast presentation. The node
//! (`comms::cast_presenter::bot`, which documents the protocol) starts one
//! per session and drives it with JSON lines on stdin; events go to stdout,
//! logs to stderr. It never reconnects: a lost connection reports
//! `disconnected` and ends the process, as a closed stdin does, so the node
//! ends the session.
//!
//! Only presentation commands cross the pipe: a data packet is forwarded when
//! its sender is one of the room metadata's `presenters` and its bytes carry
//! `presentation:` (every command's `type` does), so the scene's own traffic
//! (movement, chat) never reaches the node. Events queue at most
//! [`OUT_QUEUE`] lines; a data packet that finds the queue full is dropped.

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, anyhow};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use livekit::options::{TrackPublishOptions, VideoCodec, VideoEncoding};
use livekit::prelude::*;
use livekit::webrtc::native::yuv_helper;
use livekit::webrtc::prelude::{I420Buffer, VideoFrame, VideoRotation};
use livekit::webrtc::video_source::native::NativeVideoSource;
use livekit::webrtc::video_source::{RtcVideoSource, VideoResolution};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const HEARTBEAT: Duration = Duration::from_millis(500);
/// The largest command line: a 1920x1080 RGBA frame in base64 is ~11 MB.
const MAX_LINE: usize = 16 * 1024 * 1024;
/// Lines waiting for stdout.
const OUT_QUEUE: usize = 256;
/// What every presentation command's `type` starts with (`comms::cast_presenter::wire`).
const COMMAND_MARK: &[u8] = b"presentation:";

#[derive(Deserialize)]
struct Command {
    #[serde(rename = "type")]
    kind: String,
    id: Option<u64>,
    url: Option<String>,
    token: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    rgba: Option<String>,
    data: Option<String>,
    metadata: Option<String>,
}

/// One writer for stdout, so lines never interleave; bounded, so a node that
/// stops reading costs at most [`OUT_QUEUE`] lines.
#[derive(Clone)]
struct Out {
    tx: mpsc::Sender<Value>,
    dropped: Arc<AtomicU64>,
}

impl Out {
    /// A reply or a room state change: waits for room in the queue.
    async fn send(&self, line: Value) {
        let _ = self.tx.send(line).await;
    }

    /// A data packet: dropped when the queue is full.
    fn offer(&self, line: Value) {
        if let Err(mpsc::error::TrySendError::Full(_)) = self.tx.try_send(line) {
            let dropped = self.dropped.fetch_add(1, Ordering::Relaxed) + 1;
            if dropped.is_power_of_two() {
                eprintln!("cast-bot: stdout queue full, {dropped} data packets dropped");
            }
        }
    }
}

fn spawn_stdout() -> (Out, tokio::task::JoinHandle<()>) {
    let (tx, mut rx) = mpsc::channel::<Value>(OUT_QUEUE);
    let task = tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        while let Some(line) = rx.recv().await {
            let mut bytes = serde_json::to_vec(&line).unwrap_or_default();
            bytes.push(b'\n');
            if stdout.write_all(&bytes).await.is_err() || stdout.flush().await.is_err() {
                break;
            }
        }
    });
    let out = Out {
        tx,
        dropped: Default::default(),
    };
    (out, task)
}

/// Reads one line of at most [`MAX_LINE`] bytes; `None` at end of input.
async fn read_line<R: AsyncBufReadExt + Unpin>(reader: &mut R) -> anyhow::Result<Option<String>> {
    let mut line = Vec::new();
    loop {
        let buf = reader.fill_buf().await?;
        if buf.is_empty() {
            return Ok((!line.is_empty()).then(|| String::from_utf8_lossy(&line).into_owned()));
        }
        let (take, done) = match buf.iter().position(|&b| b == b'\n') {
            Some(i) => (i + 1, true),
            None => (buf.len(), false),
        };
        line.extend_from_slice(&buf[..take]);
        reader.consume(take);
        if line.len() > MAX_LINE {
            return Err(anyhow!("command line over {MAX_LINE} bytes"));
        }
        if done {
            line.pop();
            return Ok(Some(String::from_utf8_lossy(&line).into_owned()));
        }
    }
}

/// The slide track and the frame it repeats.
struct Video {
    source: NativeVideoSource,
    frame: Arc<Mutex<Option<VideoFrame<I420Buffer>>>>,
}

fn to_i420(rgba: &[u8], width: u32, height: u32) -> anyhow::Result<I420Buffer> {
    if rgba.len() != (width as usize) * (height as usize) * 4 {
        return Err(anyhow!("frame is not {width}x{height} RGBA"));
    }
    let mut buffer = I420Buffer::new(width, height);
    let (stride_y, stride_u, stride_v) = buffer.strides();
    let (y, u, v) = buffer.data_mut();
    // libyuv's "ABGR" is R, G, B, A in memory.
    yuv_helper::abgr_to_i420(
        rgba,
        width * 4,
        y,
        stride_y,
        u,
        stride_u,
        v,
        stride_v,
        width as i32,
        height as i32,
    );
    Ok(buffer)
}

fn now_us() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros() as i64)
        .unwrap_or_default()
}

fn capture(video: &Video) {
    if let Some(frame) = video.frame.lock().unwrap().as_mut() {
        frame.timestamp_us = now_us();
        video.source.capture_frame(frame);
    }
}

async fn publish_video(room: &Room, width: u32, height: u32) -> anyhow::Result<Video> {
    let source = NativeVideoSource::new(VideoResolution { width, height }, true);
    let track =
        LocalVideoTrack::create_video_track("presentation", RtcVideoSource::Native(source.clone()));
    // As upstream's rtc-node publisher (livekit-publisher/component.ts:119-137).
    let options = TrackPublishOptions {
        source: TrackSource::Screenshare,
        video_codec: VideoCodec::VP8,
        video_encoding: Some(VideoEncoding {
            max_bitrate: 6_000_000,
            max_framerate: 30.0,
        }),
        simulcast: false,
        stream: "presentation".into(),
        ..Default::default()
    };
    room.local_participant()
        .publish_track(LocalTrack::Video(track), options)
        .await
        .context("publish failed")?;
    let video = Video {
        source,
        frame: Arc::new(Mutex::new(None)),
    };
    let heartbeat = Video {
        source: video.source.clone(),
        frame: video.frame.clone(),
    };
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(HEARTBEAT);
        loop {
            tick.tick().await;
            capture(&heartbeat);
        }
    });
    Ok(video)
}

fn participant_count(room: &Room) -> usize {
    room.remote_participants().len()
}

/// The room metadata's `presenters`, lowercased, as the node reads them
/// (`comms::cast_presenter::wire::presenters_from_room_metadata`).
fn presenters(metadata: &str) -> HashSet<String> {
    let Ok(Value::Object(parsed)) = serde_json::from_str::<Value>(metadata) else {
        return HashSet::new();
    };
    let Some(Value::Array(list)) = parsed.get("presenters") else {
        return HashSet::new();
    };
    list.iter()
        .filter_map(Value::as_str)
        .filter(|p| !p.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Whether a packet can be a presentation command: a presenter sent it and
/// it carries a command `type`.
fn is_command(presenters: &HashSet<String>, identity: &str, payload: &[u8]) -> bool {
    presenters.contains(&identity.to_lowercase())
        && payload
            .windows(COMMAND_MARK.len())
            .any(|window| window == COMMAND_MARK)
}

/// Forwards the room's events; the receiver fires when the connection is lost.
fn forward_events(
    room: Arc<Room>,
    mut events: mpsc::UnboundedReceiver<RoomEvent>,
    out: Out,
) -> oneshot::Receiver<()> {
    let (lost_tx, lost) = oneshot::channel();
    tokio::spawn(async move {
        let mut presenters = presenters(&room.metadata());
        let mut reason = "room events ended".to_string();
        while let Some(event) = events.recv().await {
            match event {
                RoomEvent::DataReceived {
                    payload,
                    participant,
                    participant_identity,
                    ..
                } => {
                    let identity = participant
                        .map(|p| p.identity().to_string())
                        .or(participant_identity.map(|i| i.to_string()))
                        .unwrap_or_else(|| "unknown".into());
                    if is_command(&presenters, &identity, &payload) {
                        out.offer(json!({
                            "type": "dataReceived",
                            "identity": identity,
                            "payload": BASE64.encode(payload.as_slice()),
                        }));
                    }
                }
                RoomEvent::RoomMetadataChanged { metadata, .. } => {
                    presenters = self::presenters(&metadata);
                    out.send(json!({"type": "roomMetadata", "metadata": metadata}))
                        .await;
                }
                RoomEvent::ParticipantConnected(_) | RoomEvent::ParticipantDisconnected(_) => {
                    out.send(
                        json!({"type": "participantCount", "count": participant_count(&room)}),
                    )
                    .await;
                }
                RoomEvent::Disconnected { reason: why } => {
                    reason = format!("{why:?}");
                    break;
                }
                _ => {}
            }
        }
        out.send(json!({"type": "disconnected", "reason": reason}))
            .await;
        let _ = lost_tx.send(());
    });
    lost
}

/// `connect-failed` when LiveKit refused the token (a 4xx from its
/// validation, or a token it cannot read), `connect-unavailable` otherwise.
fn connect_failure(e: &RoomError) -> &'static str {
    use livekit::rtc_engine::EngineError;
    match e {
        RoomError::Engine(EngineError::Signal(signal)) => {
            let text = signal.to_string();
            if text.starts_with("client error:") || text.starts_with("access token") {
                "connect-failed"
            } else {
                "connect-unavailable"
            }
        }
        _ => "connect-unavailable",
    }
}

async fn run(out: Out) -> anyhow::Result<()> {
    let mut stdin = BufReader::with_capacity(1 << 20, tokio::io::stdin());
    let init: Command = match read_line(&mut stdin).await? {
        Some(line) => serde_json::from_str(&line).context("unreadable init")?,
        None => return Ok(()),
    };
    if init.kind != "init" {
        return Err(anyhow!("the first command must be init"));
    }
    let url = init.url.unwrap_or_default();
    let token = init.token.unwrap_or_default();
    let mut options = RoomOptions::default();
    options.auto_subscribe = false;
    let connected =
        tokio::time::timeout(CONNECT_TIMEOUT, Room::connect(&url, &token, options)).await;
    let (room, events) = match connected {
        Ok(Ok(joined)) => joined,
        Ok(Err(e)) => {
            out.send(
                json!({"type": "error", "code": connect_failure(&e), "message": e.to_string()}),
            )
            .await;
            return Ok(());
        }
        Err(_) => {
            out.send(json!({"type": "error", "code": "connect-unavailable", "message": "LiveKit connect timed out after 20s"}))
                .await;
            return Ok(());
        }
    };
    let room = Arc::new(room);
    out.send(json!({
        "type": "ready",
        "roomMetadata": room.metadata(),
        "participantCount": participant_count(&room),
    }))
    .await;
    let mut lost = forward_events(room.clone(), events, out.clone());

    let mut video: Option<Video> = None;
    loop {
        let read = tokio::select! {
            read = read_line(&mut stdin) => read,
            // The connection is gone: `disconnected` is on its way out.
            _ = &mut lost => break,
        };
        let line = match read {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(e) => {
                out.send(json!({"type": "error", "code": "bad-command", "message": e.to_string()}))
                    .await;
                break;
            }
        };
        let command: Command = match serde_json::from_str(&line) {
            Ok(command) => command,
            Err(e) => {
                out.send(json!({"type": "error", "code": "bad-command", "message": e.to_string()}))
                    .await;
                continue;
            }
        };
        let id = command.id;
        let result: anyhow::Result<()> = async {
            match command.kind.as_str() {
                "publishVideo" => {
                    let (w, h) = (command.width.unwrap_or(0), command.height.unwrap_or(0));
                    video = Some(publish_video(&room, w, h).await?);
                    Ok(())
                }
                "frame" => {
                    let video = video.as_ref().ok_or_else(|| anyhow!("no video track"))?;
                    let rgba = BASE64.decode(command.rgba.unwrap_or_default())?;
                    let buffer = to_i420(
                        &rgba,
                        command.width.unwrap_or(0),
                        command.height.unwrap_or(0),
                    )?;
                    *video.frame.lock().unwrap() = Some(VideoFrame {
                        rotation: VideoRotation::VideoRotation0,
                        timestamp_us: now_us(),
                        frame_metadata: None,
                        buffer,
                    });
                    capture(video);
                    Ok(())
                }
                "publishData" => {
                    let payload = BASE64.decode(command.data.unwrap_or_default())?;
                    room.local_participant()
                        .publish_data(DataPacket {
                            payload,
                            reliable: true,
                            ..Default::default()
                        })
                        .await?;
                    Ok(())
                }
                "updateMetadata" => {
                    room.local_participant()
                        .set_metadata(command.metadata.unwrap_or_default())
                        .await?;
                    Ok(())
                }
                "shutdown" => Ok(()),
                other => Err(anyhow!("unknown command {other}")),
            }
        }
        .await;
        let shutdown = command.kind == "shutdown";
        if shutdown {
            let _ = room.close().await;
        }
        match result {
            Ok(()) => out.send(json!({"type": "ack", "id": id})).await,
            Err(e) => {
                out.send(json!({"type": "error", "id": id, "code": "command-failed", "message": e.to_string()}))
                    .await
            }
        }
        if shutdown {
            break;
        }
    }
    let _ = room.close().await;
    Ok(())
}

#[tokio::main]
async fn main() {
    let (out, writer) = spawn_stdout();
    let failure = run(out.clone()).await.err();
    if let Some(e) = &failure {
        eprintln!("cast-bot: {e:#}");
        out.send(json!({"type": "error", "code": "bad-command", "message": e.to_string()}))
            .await;
    }
    // The event task holds a sender too; give the writer a moment to drain.
    drop(out);
    let _ = tokio::time::timeout(Duration::from_secs(1), writer).await;
    std::process::exit(if failure.is_some() { 1 } else { 0 });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_presenters_commands_cross() {
        let list = presenters(r#"{"presenters":["0xAbC","",7]}"#);
        assert_eq!(list, HashSet::from(["0xabc".to_string()]));
        let command = br#"{"type":"presentation:navigate","action":"next"}"#;
        assert!(is_command(&list, "0xABC", command));
        assert!(!is_command(&list, "0xdef", command));
        assert!(!is_command(&list, "0xabc", b"\x0a\x02movement"));
        assert!(presenters("not json").is_empty());
    }
}
