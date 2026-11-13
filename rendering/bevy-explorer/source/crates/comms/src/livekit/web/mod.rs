//! Livekit on the web: the page owns the livekit-client objects (the engine runs on a worker,
//! which has no WebRTC) and the engine drives them through plain-data handles keyed by page
//! ids. Commands go engine → page and events/replies come back over `futures_channel`
//! (lock-free: the page must never block). Replies to async requests and room events are
//! dispatched into engine-local tokio channels by [`dispatch_page_events`] each frame, so the
//! page never touches a tokio waker.

pub mod host;
mod local_audio_track;
mod local_participant;
mod local_track;
mod local_track_publication;
mod participant;
mod remote_audio_track;
mod remote_participant;
mod remote_track;
mod remote_track_publication;
mod remote_video_track;
mod room;
mod room_event;
mod track_sid;
mod track_source;

use std::{
    error::Error,
    fmt::{Display, Formatter},
    sync::{
        atomic::{AtomicU32, Ordering},
        LazyLock, Mutex,
    },
};

use bevy::prelude::*;
use dashmap::DashMap;
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender};
use media::MediaId;
use once_cell::sync::OnceCell;
use tokio::sync::{mpsc, oneshot};

pub use crate::livekit::web::{
    local_audio_track::LocalAudioTrack, local_participant::LocalParticipant,
    local_track::LocalTrack, local_track_publication::LocalTrackPublication,
    participant::Participant, remote_audio_track::RemoteAudioTrack,
    remote_participant::RemoteParticipant, remote_track::RemoteTrack,
    remote_track_publication::RemoteTrackPublication, remote_video_track::RemoteVideoTrack,
    room::Room, room_event::RoomEvent, track_sid::TrackSid, track_source::TrackSource,
};

/// Engine-allocated id of a room.
pub type RoomId = u32;
/// Page-allocated id of a participant / publication / track object.
pub type ObjectId = u32;
pub type RequestId = u32;

pub type RoomResult<T> = Result<T, RoomError>;

#[derive(Debug, Default, Clone)]
pub struct RoomOptions {
    pub auto_subscribe: bool,
    pub adaptive_stream: bool,
    pub dynacast: bool,
    // pub e2ee: Option<E2eeOptions>,
    // pub rtc_config: RtcConfiguration,
    pub join_retries: u32,
}

#[derive(Debug, Clone)]
pub struct RoomError {
    message: String,
    pub status: Option<u16>,
}

impl From<String> for RoomError {
    fn from(message: String) -> Self {
        Self {
            message,
            status: None,
        }
    }
}

impl Display for RoomError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for RoomError {}

#[derive(Debug, Clone)]
pub struct DataPacket {
    pub payload: Vec<u8>,
    pub topic: Option<String>,
    pub reliable: bool,
    pub destination_identities: Vec<ParticipantIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deref)]
pub struct ParticipantIdentity(pub String);

impl std::fmt::Display for ParticipantIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    Audio,
    Video,
}

impl TrackKind {
    fn from_js_str(kind: &str) -> Self {
        match kind {
            "audio" => Self::Audio,
            "video" => Self::Video,
            other => {
                error!("TrackKind was not a known kind. Was '{other}'. Assuming Audio.");
                Self::Audio
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deref)]
pub struct ParticipantSid(String);

impl std::fmt::Display for ParticipantSid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Kind of the packet.
///
/// Keep in track with
/// [https://github.com/livekit/protocol/blob/e7532dfc617d0c920eb905a93b6ca0d3ca4033e9/protobufs/livekit_models.proto#L324]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataPacketKind {
    Reliable = 0,
    Lossy = 1,
}

#[derive(Debug)]
pub enum ConnectionState {
    Connected,
    Reconnecting,
    Disconnected,
}

#[derive(Debug, Clone)]
pub struct LocalVideoTrack {
    id: ObjectId,
}

#[derive(Debug)]
pub struct TrackPublishOptions {
    // pub video_encoding: Option<VideoEncoding>,
    // pub audio_encoding: Option<AudioEncoding>,
    // pub video_codec: VideoCodec,
    // pub dtx: bool,
    // pub red: bool,
    pub simulcast: Option<bool>,
    pub source: TrackSource,
    pub stream: Option<String>,
    pub preconnect_buffer: Option<bool>,
}

impl Default for TrackPublishOptions {
    fn default() -> Self {
        Self {
            // video_encoding: None,
            // audio_encoding: None,
            // video_codec: VideoCodec::VP8,
            // dtx: true,
            // red: true,
            simulcast: Some(true),
            source: TrackSource::Unknown,
            stream: Some("".to_string()),
            preconnect_buffer: Some(false),
        }
    }
}

#[derive(Debug, Default)]
pub struct AudioCaptureOptions {
    pub auto_gain_control: Option<bool>,
    pub channel_count: Option<u64>,
    pub echo_cancellation: Option<bool>,
    pub latency: Option<f64>,
    pub noise_suppression: Option<bool>,
    pub voice_isolation: Option<bool>,
    pub sample_rate: Option<u64>,
    pub sample_size: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionQuality {
    Excellent,
    Good,
    Poor,
    Lost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(i32)]
pub enum DisconnectReason {
    UnknownReason = 0,
    /// the client initiated the disconnect
    ClientInitiated = 1,
    /// another participant with the same identity has joined the room
    DuplicateIdentity = 2,
    /// the server instance is shutting down
    ServerShutdown = 3,
    /// RoomService.RemoveParticipant was called
    ParticipantRemoved = 4,
    /// RoomService.DeleteRoom was called
    RoomDeleted = 5,
    /// the client is attempting to resume a session, but server is not aware of it
    StateMismatch = 6,
    /// client was unable to connect fully
    JoinFailure = 7,
    /// Cloud-only, the server requested Participant to migrate the connection elsewhere
    Migration = 8,
    /// the signal websocket was closed unexpectedly
    SignalClose = 9,
    /// the room was closed, due to all Standard and Ingress participants having left
    RoomClosed = 10,
    /// SIP callee did not respond in time
    UserUnavailable = 11,
    /// SIP callee rejected the call (busy)
    UserRejected = 12,
    /// SIP protocol failure or unexpected response
    SipTrunkFailure = 13,
    /// server timed out a participant session
    ConnectionTimeout = 14,
    /// media stream failure or media timeout
    MediaFailure = 15,
    /// agent encountered an error
    AgentError = 16,
}

impl DisconnectReason {
    fn from_i32(int: i32) -> Option<Self> {
        Some(match int {
            0 => DisconnectReason::UnknownReason,
            1 => DisconnectReason::ClientInitiated,
            2 => DisconnectReason::DuplicateIdentity,
            3 => DisconnectReason::ServerShutdown,
            4 => DisconnectReason::ParticipantRemoved,
            5 => DisconnectReason::RoomDeleted,
            6 => DisconnectReason::StateMismatch,
            7 => DisconnectReason::JoinFailure,
            8 => DisconnectReason::Migration,
            9 => DisconnectReason::SignalClose,
            10 => DisconnectReason::RoomClosed,
            11 => DisconnectReason::UserUnavailable,
            12 => DisconnectReason::UserRejected,
            13 => DisconnectReason::SipTrunkFailure,
            14 => DisconnectReason::ConnectionTimeout,
            15 => DisconnectReason::MediaFailure,
            16 => DisconnectReason::AgentError,
            _ => return None,
        })
    }
}

/// Engine → page.
#[derive(Debug)]
pub enum LivekitCommand {
    Connect {
        request: RequestId,
        room: RoomId,
        url: String,
        token: String,
        options: RoomOptions,
    },
    Close {
        request: RequestId,
        room: RoomId,
    },
    /// The engine dropped its handle: release the livekit room.
    DropRoom(RoomId),
    PublishData {
        request: RequestId,
        room: RoomId,
        packet: DataPacket,
    },
    SetSubscribed {
        publication: ObjectId,
        subscribed: bool,
    },
    PanAndVolume {
        track: ObjectId,
        pan: f32,
        volume: f32,
    },
    SetVolume {
        track: ObjectId,
        volume: f32,
    },
    /// Hands the remote video track's element to the media host as media `media_id`.
    AttachVideo {
        track: ObjectId,
        media_id: MediaId,
    },
    CreateLocalAudioTrack {
        request: RequestId,
        options: AudioCaptureOptions,
    },
    SetLocalAudioEnabled {
        track: ObjectId,
        enabled: bool,
    },
    StopLocalAudio {
        request: RequestId,
        track: ObjectId,
        release: bool,
    },
    PublishTrack {
        request: RequestId,
        room: RoomId,
        track: ObjectId,
        options: TrackPublishOptions,
    },
    UnpublishTrack {
        request: RequestId,
        room: RoomId,
        track: ObjectId,
    },
    SetupMicrophonePermission,
    PromptMicrophonePermission,
}

/// Page → engine.
#[derive(Debug)]
pub enum LivekitEvent {
    Reply {
        request: RequestId,
        reply: Reply,
    },
    Room {
        room: RoomId,
        event: RoomEvent,
    },
    /// The browser's microphone availability / permission (`"granted"`, `"prompt"`,
    /// `"denied"`) changed.
    Microphone {
        available: bool,
        permission: String,
    },
}

/// The page's answer to a request.
#[derive(Debug)]
pub enum Reply {
    Connect(Result<ConnectedRoom, RoomError>),
    Close(Result<(), String>),
    PublishData(Result<(), String>),
    LocalAudioTrack(Result<LocalAudioTrack, String>),
    LocalAudioStopped(Result<(), String>),
    Publish(Result<LocalTrackPublication, String>),
}

#[derive(Debug)]
pub struct ConnectedRoom {
    pub room: RoomId,
    pub name: String,
    pub metadata: String,
    pub local_participant: LocalParticipant,
}

/// Set by the page ([`host::livekit_host_main`]) before the engine starts.
struct HostHandle {
    commands: UnboundedSender<LivekitCommand>,
    events: Mutex<Option<UnboundedReceiver<LivekitEvent>>>,
}

static HOST: OnceCell<HostHandle> = OnceCell::new();
static NEXT_ROOM_ID: AtomicU32 = AtomicU32::new(1);
static NEXT_REQUEST_ID: AtomicU32 = AtomicU32::new(1);
/// Engine-side: requests awaiting their [`Reply`].
static PENDING: LazyLock<DashMap<RequestId, oneshot::Sender<Reply>>> = LazyLock::new(DashMap::new);
/// Engine-side: a room's event queue (the receiver is what [`Room::connect`] returns) and the
/// metadata the page last reported for it.
struct RoomChannel {
    events: mpsc::UnboundedSender<RoomEvent>,
    metadata: String,
}

static ROOM_EVENTS: LazyLock<DashMap<RoomId, RoomChannel>> = LazyLock::new(DashMap::new);

fn send_command(command: LivekitCommand) {
    match HOST.get() {
        Some(host) => {
            let _ = host.commands.unbounded_send(command);
        }
        None => debug!("no livekit host: dropping {command:?}"),
    }
}

/// Sends the command built for a fresh request id and awaits the page's reply.
async fn request(command: impl FnOnce(RequestId) -> LivekitCommand) -> RoomResult<Reply> {
    let Some(host) = HOST.get() else {
        return Err(RoomError::from("no livekit host".to_owned()));
    };
    let request = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
    let (sender, receiver) = oneshot::channel();
    PENDING.insert(request, sender);
    if host.commands.unbounded_send(command(request)).is_err() {
        return Err(RoomError::from("livekit host is gone".to_owned()));
    }
    receiver
        .await
        .map_err(|_| RoomError::from("livekit host dropped the request".to_owned()))
}

fn register_room_events(room: RoomId) -> mpsc::UnboundedReceiver<RoomEvent> {
    let (sender, receiver) = mpsc::unbounded_channel();
    ROOM_EVENTS.insert(
        room,
        RoomChannel {
            events: sender,
            metadata: String::new(),
        },
    );
    receiver
}

/// Records the metadata the page reported for `room` at connect.
fn set_room_metadata(room: RoomId, metadata: &str) {
    if let Some(mut channel) = ROOM_EVENTS.get_mut(&room) {
        metadata.clone_into(&mut channel.metadata);
    }
}

/// The metadata the page last reported for `room`; empty once the room is unregistered.
fn room_metadata(room: RoomId) -> String {
    ROOM_EVENTS
        .get(&room)
        .map(|channel| channel.metadata.clone())
        .unwrap_or_default()
}

fn unregister_room_events(room: RoomId) {
    ROOM_EVENTS.remove(&room);
}

pub fn setup_microphone_permission() {
    send_command(LivekitCommand::SetupMicrophonePermission);
}

pub fn prompt_microphone_permission() {
    send_command(LivekitCommand::PromptMicrophonePermission);
}

/// The browser's latest word on the microphone, from [`LivekitEvent::Microphone`].
#[derive(Resource)]
pub struct MicrophoneStatus {
    pub available: bool,
    /// `"granted"`, `"prompt"` or `"denied"`
    pub permission: String,
}

impl Default for MicrophoneStatus {
    fn default() -> Self {
        Self {
            available: false,
            permission: "denied".to_owned(),
        }
    }
}

#[derive(Resource)]
pub struct LivekitEventReceiver(Option<UnboundedReceiver<LivekitEvent>>);

/// Takes the page → engine event receiver; call from the plugin's build.
pub fn take_event_receiver() -> LivekitEventReceiver {
    let events = HOST
        .get()
        .and_then(|host| host.events.lock().unwrap().take());
    if events.is_none() {
        warn!("no livekit host: livekit rooms will not connect");
    }
    LivekitEventReceiver(events)
}

/// Delivers the page's replies and room events into the engine-side channels awaiting them.
pub fn dispatch_page_events(
    mut receiver: ResMut<LivekitEventReceiver>,
    mut microphone: ResMut<MicrophoneStatus>,
) {
    let Some(receiver) = receiver.0.as_mut() else {
        return;
    };
    while let Ok(event) = receiver.try_recv() {
        match event {
            LivekitEvent::Reply { request, reply } => {
                if let Reply::Connect(Ok(connected)) = &reply {
                    set_room_metadata(connected.room, &connected.metadata);
                }
                match PENDING.remove(&request) {
                    Some((_, sender)) => {
                        // the requester may have been aborted
                        let _ = sender.send(reply);
                    }
                    None => debug!("no requester for reply {request}: {reply:?}"),
                }
            }
            LivekitEvent::Room { room, event } => {
                let Some(mut channel) = ROOM_EVENTS.get_mut(&room) else {
                    debug!("event for unknown room {room}: {event:?}");
                    continue;
                };
                // the metadata is written before the event that reports it is forwarded
                if let RoomEvent::RoomMetadataChanged { metadata } = &event {
                    channel.metadata.clone_from(metadata);
                }
                let receiver_gone = channel.events.send(event).is_err();
                // removing while the entry's shard guard is held would deadlock
                drop(channel);
                if receiver_gone {
                    // the engine's LivekitRoom is gone
                    ROOM_EVENTS.remove(&room);
                }
            }
            LivekitEvent::Microphone {
                available,
                permission,
            } => {
                microphone.available = available;
                microphone.permission = permission;
            }
        }
    }
}
