//! What clients parse: the data-channel packet, the bot's participant
//! metadata and the state broadcasts, as upstream cast-presenter-server
//! (`decentraland/cast-presenter-server` @ cae78612) writes them.
//!
//! - Packet (`src/adapters/livekit-publisher/comms.ts:1-86`): every message is
//!   UTF-8 JSON on the `presentation` topic, framed `[3][topic length, u16
//!   little endian][topic][json]` inside an RFC-4 `Packet { scene { scene_id,
//!   data } }` with `protocol_version` 100. An incoming packet that is not a
//!   `Packet` is read as raw JSON; a `Packet` of another kind, or a scene
//!   message on another topic, is dropped.
//! - Presenters (`comms.ts:35-48`): the room metadata's `presenters` array,
//!   non-empty strings lowercased; the bot obeys only those identities
//!   (`component.ts:83-107`), compared lowercased.
//! - Bot metadata (`component.ts:62` and `208-211`): `{"role":"presentation",
//!   "presentationId":<id>}` on connect, then that object followed by the
//!   state's fields after every broadcast.
//! - State (`src/logic/presentation-manager/component.ts:1432-1448`): `id`,
//!   `fileName`, `slideCount`, `currentSlide`, `fileType`, `slideVideos`,
//!   `videoState`, `overlay`, in that order; broadcast as
//!   `{"type":"presentation:state", ...state}` (`component.ts:320-335`), and
//!   `{"type":"presentation:stopped","id":<id>}` on stop
//!   (`component.ts:1405`). Numbers print as `JSON.stringify` prints them.

use prost::Message;
use serde::Serialize;
use serde::ser::Serializer;
use serde_json::Value;
use serde_json::value::RawValue;

use crate::protocol::pulse::decentraland::kernel::comms::rfc4::{Packet, Scene, packet};
use crate::shared::js::number_to_string;

/// The data-channel topic of every presentation message.
pub const PRESENTATION_TOPIC: &str = "presentation";
/// `Packet.protocol_version` on the bot's packets.
pub const PROTOCOL_VERSION: u32 = 100;
/// The first byte of a scene message that carries a topic (`CommsMessageType::CommsData`).
const MSG_TYPE_COMMS_DATA: u8 = 3;
/// The prefix of every command the bot handles.
pub const COMMAND_PREFIX: &str = "presentation:";

/// `[3][topic length, u16 LE][topic][json]`.
fn encode_comms_payload(topic: &str, json: &str) -> Vec<u8> {
    let topic = topic.as_bytes();
    let mut out = Vec::with_capacity(3 + topic.len() + json.len());
    out.push(MSG_TYPE_COMMS_DATA);
    // As upstream: the low 16 bits of the length.
    out.push((topic.len() & 0xff) as u8);
    out.push(((topic.len() >> 8) & 0xff) as u8);
    out.extend_from_slice(topic);
    out.extend_from_slice(json.as_bytes());
    out
}

fn decode_comms_payload(scene_data: &[u8]) -> Option<(String, String)> {
    let (&kind, inner) = scene_data.split_first()?;
    if kind != MSG_TYPE_COMMS_DATA || inner.len() < 2 {
        return None;
    }
    let topic_len = usize::from(inner[0]) | (usize::from(inner[1]) << 8);
    if inner.len() < 2 + topic_len {
        return None;
    }
    let topic = String::from_utf8_lossy(&inner[2..2 + topic_len]).into_owned();
    let data = String::from_utf8_lossy(&inner[2 + topic_len..]).into_owned();
    Some((topic, data))
}

/// A message the bot publishes, wrapped for the data channel. `scene_id` is
/// the scene id of the last presenter command (empty before the first).
pub fn encode_packet(json: &str, scene_id: &str) -> Vec<u8> {
    Packet {
        message: Some(packet::Message::Scene(Scene {
            scene_id: scene_id.to_string(),
            data: encode_comms_payload(PRESENTATION_TOPIC, json),
        })),
        protocol_version: PROTOCOL_VERSION,
    }
    .encode_to_vec()
}

/// A decoded incoming message and the scene id its packet carried (`None`
/// for raw JSON or an empty scene id).
#[derive(Debug, Clone, PartialEq)]
pub struct Incoming {
    pub message: serde_json::Map<String, Value>,
    pub scene_id: Option<String>,
}

fn json_object(text: &str) -> Option<serde_json::Map<String, Value>> {
    match serde_json::from_str::<Value>(text).ok()? {
        Value::Object(map) => Some(map),
        // Upstream keeps any truthy JSON value, but only an object has a
        // `type`, so nothing else reaches a handler.
        _ => None,
    }
}

/// An incoming data packet: an RFC-4 scene message on the presentation
/// topic, or raw JSON when the bytes are not a `Packet`.
pub fn decode_packet(payload: &[u8]) -> Option<Incoming> {
    match Packet::decode(payload) {
        Ok(packet) => {
            let Some(packet::Message::Scene(scene)) = packet.message else {
                return None;
            };
            let (topic, data) = decode_comms_payload(&scene.data)?;
            if topic != PRESENTATION_TOPIC {
                return None;
            }
            let message = json_object(&data)?;
            let scene_id = (!scene.scene_id.is_empty()).then_some(scene.scene_id);
            Some(Incoming { message, scene_id })
        }
        Err(_) => {
            let message = json_object(&String::from_utf8_lossy(payload))?;
            Some(Incoming {
                message,
                scene_id: None,
            })
        }
    }
}

/// The room metadata's `presenters`: non-empty strings, lowercased; empty
/// for missing or malformed metadata.
pub fn presenters_from_room_metadata(metadata: &str) -> std::collections::HashSet<String> {
    let Ok(Value::Object(parsed)) = serde_json::from_str::<Value>(metadata) else {
        return Default::default();
    };
    let Some(Value::Array(presenters)) = parsed.get("presenters") else {
        return Default::default();
    };
    presenters
        .iter()
        .filter_map(Value::as_str)
        .filter(|p| !p.is_empty())
        // JavaScript's toLowerCase.
        .map(str::to_lowercase)
        .collect()
}

/// A number as `JSON.stringify` writes it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JsNumber(pub f64);

impl Serialize for JsNumber {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if !self.0.is_finite() {
            return serializer.serialize_unit();
        }
        let text = number_to_string(self.0);
        let raw = RawValue::from_string(text).map_err(serde::ser::Error::custom)?;
        raw.serialize(serializer)
    }
}

/// The two file types upstream accepts (`src/logic/file-validator.ts:10-13`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileType {
    Pdf,
    Pptx,
}

impl FileType {
    pub fn as_str(self) -> &'static str {
        match self {
            FileType::Pdf => "pdf",
            FileType::Pptx => "pptx",
        }
    }
}

/// A media link's box on the rendered slide, in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Geometry {
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
}

/// One media link of the current slide (`SlideVideoInfo`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SlideVideo {
    pub url: String,
    pub geometry: Geometry,
}

/// The camera bubble's size preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OverlaySize {
    Small,
    Large,
}

/// The camera-bubble layout (`src/logic/overlay-layout.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Overlay {
    pub x: JsNumber,
    pub y: JsNumber,
    pub size: OverlaySize,
}

/// A new session's layout: small, bottom-left.
pub const DEFAULT_OVERLAY: Overlay = Overlay {
    x: JsNumber(0.0),
    y: JsNumber(1.0),
    size: OverlaySize::Small,
};

/// The fields one `presentation:overlay:update` carries.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct OverlayPatch {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub size: Option<OverlaySize>,
}

impl OverlayPatch {
    pub fn is_empty(&self) -> bool {
        self.x.is_none() && self.y.is_none() && self.size.is_none()
    }

    pub fn apply(&self, overlay: &mut Overlay) {
        if let Some(x) = self.x {
            overlay.x = JsNumber(x);
        }
        if let Some(y) = self.y {
            overlay.y = JsNumber(y);
        }
        if let Some(size) = self.size {
            overlay.size = size;
        }
    }
}

/// `parseOverlayUpdate`: `x` and `y` finite numbers clamped to `[0, 1]`,
/// `size` `small` or `large`; any invalid field drops the whole command.
pub fn parse_overlay_update(message: &serde_json::Map<String, Value>) -> Option<OverlayPatch> {
    let mut patch = OverlayPatch::default();
    for (axis, slot) in [("x", &mut patch.x), ("y", &mut patch.y)] {
        // `undefined` skips the field; JSON has no undefined, so only absence does.
        let Some(value) = message.get(axis) else {
            continue;
        };
        let number = value.as_f64().filter(|n| n.is_finite())?;
        *slot = Some(number.clamp(0.0, 1.0));
    }
    if let Some(size) = message.get("size") {
        patch.size = Some(match size.as_str()? {
            "small" => OverlaySize::Small,
            "large" => OverlaySize::Large,
            _ => return None,
        });
    }
    Some(patch)
}

/// The playback state; phase one never leaves `idle` (no video playback).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VideoState {
    Idle,
}

/// `PresentationState` with client composition off (`types.ts:35-64`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresentationState {
    pub id: String,
    pub file_name: String,
    pub slide_count: usize,
    pub current_slide: usize,
    pub file_type: FileType,
    pub slide_videos: Vec<SlideVideo>,
    pub video_state: VideoState,
    pub overlay: Overlay,
}

#[derive(Serialize)]
struct Typed<'a, T: Serialize> {
    #[serde(rename = "type")]
    kind: &'a str,
    #[serde(flatten)]
    body: &'a T,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BotMetadata<'a, T: Serialize> {
    role: &'a str,
    presentation_id: &'a str,
    #[serde(flatten)]
    state: Option<&'a T>,
}

/// `{"type":"presentation:state", ...state}`.
pub fn state_message(state: &PresentationState) -> String {
    serde_json::to_string(&Typed {
        kind: "presentation:state",
        body: state,
    })
    .expect("state serializes")
}

/// Upstream's `broadcastError` for a play that failed:
/// `{"type":"presentation:error","code":"video-playback-failed","message":..,"videoIndex":..}`.
pub fn video_unavailable_message(video_index: Option<u64>) -> String {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Failure {
        code: &'static str,
        message: &'static str,
        #[serde(skip_serializing_if = "Option::is_none")]
        video_index: Option<u64>,
    }
    serde_json::to_string(&Typed {
        kind: "presentation:error",
        body: &Failure {
            code: "video-playback-failed",
            message: "Video playback is not available on this server",
            video_index,
        },
    })
    .expect("error serializes")
}

/// `{"type":"presentation:stopped","id":<id>}`.
pub fn stopped_message(id: &str) -> String {
    #[derive(Serialize)]
    struct Id<'a> {
        id: &'a str,
    }
    serde_json::to_string(&Typed {
        kind: "presentation:stopped",
        body: &Id { id },
    })
    .expect("stop serializes")
}

/// The bot's participant metadata: on connect without a state, then with it.
pub fn bot_metadata(presentation_id: &str, state: Option<&PresentationState>) -> String {
    serde_json::to_string(&BotMetadata {
        role: "presentation",
        presentation_id,
        state,
    })
    .expect("metadata serializes")
}

/// `POST /presentations`' 201 body (`PresentationInfo`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresentationInfo {
    pub id: String,
    pub file_name: String,
    pub slide_count: usize,
    pub current_slide: usize,
    pub file_type: FileType,
}

/// A navigation request (`presentation:navigate`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Navigate {
    Next,
    Prev,
    /// `goto` with `slideIndex` when it is a non-negative integer.
    Goto(Option<f64>),
}

/// A command the bot acts on in this phase.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Navigate(Navigate),
    GetState,
    Stop,
    Overlay(OverlayPatch),
    /// `presentation:video:play` with its `videoIndex`: this node plays no
    /// video, and says so with upstream's error event.
    VideoPlay(Option<u64>),
    /// The other `presentation:video:*` and `presentation:presenter:claim`: known to
    /// upstream, not acted on here (no video playback, no client composition).
    Unsupported(String),
}

/// What a presenter's message asks, or `None` when the bot ignores it:
/// another `type`, a `navigate` with an unknown action, an overlay update
/// with an invalid or no field.
pub fn parse_command(message: &serde_json::Map<String, Value>) -> Option<Command> {
    match message.get("type").and_then(Value::as_str)? {
        "presentation:navigate" => {
            let navigate = match message.get("action").and_then(Value::as_str)? {
                "next" => Navigate::Next,
                "prev" => Navigate::Prev,
                "goto" => Navigate::Goto(
                    message
                        .get("slideIndex")
                        .and_then(Value::as_f64)
                        // Number.isInteger and >= 0.
                        .filter(|n| n.is_finite() && n.fract() == 0.0 && *n >= 0.0),
                ),
                _ => return None,
            };
            Some(Command::Navigate(navigate))
        }
        "presentation:get-state" => Some(Command::GetState),
        "presentation:stop" => Some(Command::Stop),
        "presentation:overlay:update" => {
            let patch = parse_overlay_update(message)?;
            (!patch.is_empty()).then_some(Command::Overlay(patch))
        }
        "presentation:video:play" => Some(Command::VideoPlay(
            message.get("videoIndex").and_then(Value::as_u64),
        )),
        kind @ ("presentation:video:pause"
        | "presentation:video:stop"
        | "presentation:presenter:claim") => Some(Command::Unsupported(kind.to_string())),
        _ => None,
    }
}
