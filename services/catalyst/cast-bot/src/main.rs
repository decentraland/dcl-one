//! `cast-bot`: the LiveKit participant of one Cast presentation. The node
//! (`comms::cast_presenter::bot`, which documents the protocol) starts one
//! per session and drives it with JSON lines on stdin; events go to stdout,
//! logs to stderr. It never reconnects: a lost connection or a closed stdin
//! ends the process.

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
use tokio::sync::mpsc;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const HEARTBEAT: Duration = Duration::from_millis(500);
/// The largest command line: a 1920x1080 RGBA frame in base64 is ~11 MB.
const MAX_LINE: usize = 16 * 1024 * 1024;

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

/// One writer for stdout, so lines never interleave.
#[derive(Clone)]
struct Out(mpsc::UnboundedSender<Value>);

impl Out {
    fn send(&self, line: Value) {
        let _ = self.0.send(line);
    }
}

fn spawn_stdout() -> (Out, tokio::task::JoinHandle<()>) {
    let (tx, mut rx) = mpsc::unbounded_channel::<Value>();
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
    (Out(tx), task)
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

fn forward_events(room: Arc<Room>, mut events: mpsc::UnboundedReceiver<RoomEvent>, out: Out) {
    tokio::spawn(async move {
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
                    out.send(json!({
                        "type": "dataReceived",
                        "identity": identity,
                        "payload": BASE64.encode(payload.as_slice()),
                    }));
                }
                RoomEvent::RoomMetadataChanged { metadata, .. } => {
                    out.send(json!({"type": "roomMetadata", "metadata": metadata}));
                }
                RoomEvent::ParticipantConnected(_) | RoomEvent::ParticipantDisconnected(_) => {
                    out.send(
                        json!({"type": "participantCount", "count": participant_count(&room)}),
                    );
                }
                RoomEvent::Disconnected { reason } => {
                    out.send(json!({"type": "disconnected", "reason": format!("{reason:?}")}));
                    break;
                }
                _ => {}
            }
        }
    });
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
            out.send(json!({"type": "error", "code": "connect-failed", "message": e.to_string()}));
            return Ok(());
        }
        Err(_) => {
            out.send(json!({"type": "error", "code": "connect-failed", "message": "LiveKit connect timed out after 20s"}));
            return Ok(());
        }
    };
    let room = Arc::new(room);
    out.send(json!({
        "type": "ready",
        "roomMetadata": room.metadata(),
        "participantCount": participant_count(&room),
    }));
    forward_events(room.clone(), events, out.clone());

    let mut video: Option<Video> = None;
    loop {
        let line = match read_line(&mut stdin).await {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(e) => {
                out.send(json!({"type": "error", "code": "bad-command", "message": e.to_string()}));
                break;
            }
        };
        let command: Command = match serde_json::from_str(&line) {
            Ok(command) => command,
            Err(e) => {
                out.send(json!({"type": "error", "code": "bad-command", "message": e.to_string()}));
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
            Ok(()) => out.send(json!({"type": "ack", "id": id})),
            Err(e) => out.send(json!({"type": "error", "id": id, "code": "command-failed", "message": e.to_string()})),
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
        out.send(json!({"type": "error", "code": "bad-command", "message": e.to_string()}));
    }
    // The event task holds a sender too; give the writer a moment to drain.
    drop(out);
    let _ = tokio::time::timeout(Duration::from_secs(1), writer).await;
    std::process::exit(if failure.is_some() { 1 } else { 0 });
}
