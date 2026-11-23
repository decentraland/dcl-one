//! `harness-util join`: connect to a room, stay, and print what happens as one JSON object per
//! line on standard output. Standard input carries line commands; its end leaves the room.
//!
//! The token never appears on the command line (it is read from an environment variable) and is
//! never printed.

use std::time::Duration;

use anyhow::{anyhow, bail, Context};
use livekit::options::TrackPublishOptions;
use livekit::prelude::*;
use livekit::webrtc::audio_frame::AudioFrame;
use livekit::webrtc::audio_source::native::NativeAudioSource;
use livekit::webrtc::audio_source::{AudioSourceOptions, RtcAudioSource};
use livekit::DisconnectReason;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, BufReader};

const SAMPLE_RATE: u32 = 48_000;
const SAMPLES_PER_FRAME: u32 = SAMPLE_RATE / 100;

pub struct JoinArguments {
    pub url: String,
    pub token: String,
    pub publish_microphone: bool,
}

/// The server URL and token of a `livekit:<url>?access_token=<token>` connection string.
pub fn parse_connection_string(connection: &str) -> anyhow::Result<(String, String)> {
    let rest = connection
        .trim()
        .strip_prefix("livekit:")
        .ok_or_else(|| anyhow!("the connection string does not start with livekit:"))?;
    let (url, query) = rest
        .split_once('?')
        .ok_or_else(|| anyhow!("the connection string has no query"))?;
    let token = query
        .split('&')
        .find_map(|pair| pair.strip_prefix("access_token="))
        .filter(|token| !token.is_empty())
        .ok_or_else(|| anyhow!("the connection string has no access_token"))?;
    if url.is_empty() {
        bail!("the connection string has no server URL");
    }
    Ok((url.to_owned(), token.to_owned()))
}

fn emit(value: Value) {
    println!("{value}");
}

fn track_event(event: &str, identity: &str, publication: &impl PublicationInfo) -> Value {
    json!({
        "event": event,
        "identity": identity,
        "sid": publication.track_sid(),
        "kind": publication.track_kind(),
        "source": publication.track_source(),
    })
}

trait PublicationInfo {
    fn track_sid(&self) -> String;
    fn track_kind(&self) -> &'static str;
    fn track_source(&self) -> &'static str;
}

macro_rules! publication_info {
    ($type:ty) => {
        impl PublicationInfo for $type {
            fn track_sid(&self) -> String {
                self.sid().to_string()
            }
            fn track_kind(&self) -> &'static str {
                kind_name(self.kind())
            }
            fn track_source(&self) -> &'static str {
                source_name(self.source())
            }
        }
    };
}

publication_info!(RemoteTrackPublication);
publication_info!(LocalTrackPublication);

fn kind_name(kind: TrackKind) -> &'static str {
    match kind {
        TrackKind::Audio => "audio",
        TrackKind::Video => "video",
    }
}

fn source_name(source: TrackSource) -> &'static str {
    match source {
        TrackSource::Unknown => "unknown",
        TrackSource::Camera => "camera",
        TrackSource::Microphone => "microphone",
        TrackSource::Screenshare => "screenshare",
        TrackSource::ScreenshareAudio => "screenshare_audio",
    }
}

fn state_name(state: ConnectionState) -> &'static str {
    match state {
        ConnectionState::Disconnected => "disconnected",
        ConnectionState::Connected => "connected",
        ConnectionState::Reconnecting => "reconnecting",
    }
}

/// Publishes a track that carries silence, with the microphone source, without a sound device.
async fn publish_silent_microphone(room: &Room) -> anyhow::Result<()> {
    let source = NativeAudioSource::new(AudioSourceOptions::default(), SAMPLE_RATE, 1, 1000);
    let track =
        LocalAudioTrack::create_audio_track("microphone", RtcAudioSource::Native(source.clone()));
    room.local_participant()
        .publish_track(
            LocalTrack::Audio(track),
            TrackPublishOptions {
                source: TrackSource::Microphone,
                ..Default::default()
            },
        )
        .await
        .context("publishing the microphone track")?;
    tokio::spawn(async move {
        let frame = AudioFrame::new(SAMPLE_RATE, 1, SAMPLES_PER_FRAME);
        let mut tick = tokio::time::interval(Duration::from_millis(10));
        loop {
            tick.tick().await;
            if source.capture_frame(&frame).await.is_err() {
                break;
            }
        }
    });
    Ok(())
}

fn snapshot(room: &Room) -> Value {
    let participants: Vec<Value> = room
        .remote_participants()
        .values()
        .map(|participant| {
            json!({
                "identity": participant.identity().to_string(),
                "sid": participant.sid().to_string(),
                "tracks": participant
                    .track_publications()
                    .values()
                    .map(|publication| json!({
                        "sid": publication.track_sid(),
                        "kind": publication.track_kind(),
                        "source": publication.track_source(),
                    }))
                    .collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({ "event": "status", "state": state_name(room.connection_state()), "participants": participants })
}

/// Returns the process exit code.
pub async fn run(arguments: JoinArguments) -> i32 {
    let JoinArguments {
        url,
        token,
        publish_microphone,
    } = arguments;
    let mut options = RoomOptions::default();
    options.auto_subscribe = true;
    options.adaptive_stream = false;
    options.dynacast = false;

    let (room, mut events) = match Room::connect(&url, &token, options).await {
        Ok(connected) => connected,
        Err(error) => {
            let message = error.to_string().replace(&token, "<token>");
            emit(json!({ "event": "error", "message": format!("connect failed: {message}") }));
            return 1;
        }
    };
    emit(json!({
        "event": "connected",
        "room": room.name(),
        "identity": room.local_participant().identity().to_string(),
        "sid": room.local_participant().sid().to_string(),
        "room_sid": room.sid().await.to_string(),
    }));
    for participant in room.remote_participants().values() {
        emit(json!({
            "event": "participant_connected",
            "identity": participant.identity().to_string(),
            "sid": participant.sid().to_string(),
        }));
        for publication in participant.track_publications().values() {
            emit(track_event(
                "track_published",
                participant.identity().as_str(),
                publication,
            ));
        }
    }
    if publish_microphone {
        if let Err(error) = publish_silent_microphone(&room).await {
            emit(json!({ "event": "error", "message": format!("{error:#}") }));
            room.close().await.ok();
            return 1;
        }
    }

    let (line_sender, mut lines) = tokio::sync::mpsc::unbounded_channel::<String>();
    tokio::spawn(async move {
        let mut input = BufReader::new(tokio::io::stdin()).lines();
        while let Ok(Some(line)) = input.next_line().await {
            if line_sender.send(line).is_err() {
                return;
            }
        }
        // end of input leaves the room
        let _ = line_sender.send("leave".to_owned());
    });

    loop {
        tokio::select! {
            line = lines.recv() => {
                match line.as_deref().map(str::trim) {
                    Some("status") => emit(snapshot(&room)),
                    Some("leave") | None => {
                        room.close().await.ok();
                        emit(json!({ "event": "disconnected", "reason": DisconnectReason::ClientInitiated.as_str_name() }));
                        return 0;
                    }
                    Some("") => {}
                    Some(other) => emit(json!({ "event": "error", "message": format!("unknown command {other}") })),
                }
            }
            event = events.recv() => {
                let Some(event) = event else {
                    emit(json!({ "event": "error", "message": "the event stream ended" }));
                    return 1;
                };
                match event {
                    RoomEvent::ParticipantConnected(participant) => emit(json!({
                        "event": "participant_connected",
                        "identity": participant.identity().to_string(),
                        "sid": participant.sid().to_string(),
                    })),
                    RoomEvent::ParticipantDisconnected(participant) => emit(json!({
                        "event": "participant_disconnected",
                        "identity": participant.identity().to_string(),
                        "sid": participant.sid().to_string(),
                    })),
                    RoomEvent::TrackPublished { publication, participant } => {
                        emit(track_event("track_published", participant.identity().as_str(), &publication));
                    }
                    RoomEvent::TrackUnpublished { publication, participant } => {
                        emit(track_event("track_unpublished", participant.identity().as_str(), &publication));
                    }
                    RoomEvent::TrackSubscribed { publication, participant, .. } => {
                        emit(track_event("track_subscribed", participant.identity().as_str(), &publication));
                    }
                    RoomEvent::ConnectionStateChanged(state) => {
                        emit(json!({ "event": "connection_state", "state": state_name(state) }));
                    }
                    RoomEvent::Reconnecting => emit(json!({ "event": "reconnecting" })),
                    RoomEvent::Reconnected => emit(json!({ "event": "reconnected" })),
                    RoomEvent::Disconnected { reason } => {
                        emit(json!({ "event": "disconnected", "reason": reason.as_str_name() }));
                        return if reason == DisconnectReason::ClientInitiated { 0 } else { 2 };
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_connection_string;

    #[test]
    fn a_connection_string_gives_its_url_and_token() {
        let (url, token) =
            parse_connection_string("livekit:ws://127.0.0.1:5214?access_token=aaa.bbb.ccc")
                .unwrap();
        assert_eq!(url, "ws://127.0.0.1:5214");
        assert_eq!(token, "aaa.bbb.ccc");
        let (url, token) =
            parse_connection_string("livekit:wss://host/rtc?x=1&access_token=t&y=2\n").unwrap();
        assert_eq!((url.as_str(), token.as_str()), ("wss://host/rtc", "t"));
    }

    #[test]
    fn a_malformed_connection_string_is_refused() {
        for bad in [
            "ws://host?access_token=t",
            "livekit:ws://host",
            "livekit:ws://host?other=1",
            "livekit:ws://host?access_token=",
            "livekit:?access_token=t",
        ] {
            assert!(parse_connection_string(bad).is_err(), "{bad}");
        }
    }
}
