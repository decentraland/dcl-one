//! The presenter's wire contract, checks and session behavior, against a
//! fake bot. Upstream references are `decentraland/cast-presenter-server`
//! @ cae78612.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use prost::Message;
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tower::ServiceExt;

use tokio::sync::watch;

use super::bot::{BotLauncher, BotLink, ConnectError, Connected, RoomData, RoomState};
use super::render::{Frame, PdfDocument, fit_render_size, is_media_url};
use super::session::{IDLE_TIMEOUT, LiveKitTarget, Presenter};
use super::wire::*;
use super::{files, ssrf};
use crate::protocol::pulse::decentraland::kernel::comms::rfc4::{Packet, Scene, packet};
use crate::shared::livekit::RoomServiceClient;

// ── fake bot ──

#[derive(Debug, Clone, PartialEq)]
enum Sent {
    PublishVideo(u32, u32),
    Frame(u32, u32),
    Data(Vec<u8>),
    Metadata(String),
    Shutdown,
}

/// Run once, on the bot's first data packet.
type OnFirstData = Box<dyn FnOnce() + Send>;

#[derive(Default)]
struct FakeBot {
    sent: Mutex<Vec<Sent>>,
    on_first_data: Mutex<Option<OnFirstData>>,
    /// The process ended: a command fails.
    exited: AtomicBool,
    /// While set, `shutdown` waits for it.
    shutdown_gate: Mutex<Option<Arc<tokio::sync::Notify>>>,
    /// While set, `publish_data` waits for it, and `publish_waiting` is set.
    publish_gate: Mutex<Option<Arc<tokio::sync::Notify>>>,
    publish_waiting: AtomicBool,
    /// The track and each data message, stamped from [`ORDER`] so that the
    /// output of several bots can be put in one order.
    stamps: Mutex<Vec<(u64, Value)>>,
}

/// One clock for every fake bot's [`FakeBot::stamps`].
static ORDER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl FakeBot {
    fn stamp(&self, message: Value) {
        let at = ORDER.fetch_add(1, Ordering::SeqCst);
        self.stamps.lock().unwrap().push((at, message));
    }
}

#[async_trait::async_trait]
impl BotLink for FakeBot {
    async fn publish_video(&self, w: u32, h: u32) -> anyhow::Result<()> {
        self.sent.lock().unwrap().push(Sent::PublishVideo(w, h));
        self.stamp(json!({"type": "track"}));
        Ok(())
    }
    async fn push_frame(&self, frame: &Frame) -> anyhow::Result<()> {
        assert_eq!(frame.rgba.len(), (frame.width * frame.height * 4) as usize);
        self.sent
            .lock()
            .unwrap()
            .push(Sent::Frame(frame.width, frame.height));
        Ok(())
    }
    async fn publish_data(&self, packet: Vec<u8>) -> anyhow::Result<()> {
        let gate = self.publish_gate.lock().unwrap().take();
        if let Some(gate) = gate {
            self.publish_waiting.store(true, Ordering::SeqCst);
            gate.notified().await;
        }
        anyhow::ensure!(!self.exited.load(Ordering::SeqCst), "bot exited");
        if let Some(incoming) = decode_packet(&packet) {
            self.stamp(Value::Object(incoming.message));
        }
        self.sent.lock().unwrap().push(Sent::Data(packet));
        let hook = self.on_first_data.lock().unwrap().take();
        if let Some(hook) = hook {
            hook();
        }
        Ok(())
    }
    async fn update_metadata(&self, metadata: String) -> anyhow::Result<()> {
        self.sent.lock().unwrap().push(Sent::Metadata(metadata));
        Ok(())
    }
    async fn shutdown(&self) {
        let gate = self.shutdown_gate.lock().unwrap().clone();
        if let Some(gate) = gate {
            gate.notified().await;
        }
        self.sent.lock().unwrap().push(Sent::Shutdown);
    }
}

/// The room side of a fake bot: what the bot would report.
#[derive(Clone)]
struct FakeRoom {
    data: mpsc::Sender<RoomData>,
    state: Arc<watch::Sender<RoomState>>,
}

impl FakeRoom {
    async fn data(&self, identity: &str, payload: Vec<u8>) {
        let identity = identity.to_string();
        self.data
            .send(RoomData { identity, payload })
            .await
            .unwrap();
    }
    fn metadata(&self, metadata: &str) {
        self.state
            .send_modify(|room| room.metadata = metadata.to_string());
    }
    fn participants(&self, count: usize) {
        self.state.send_modify(|room| room.participants = count);
    }
}

/// A LiveKit that accepts any token but those whose identity ends in
/// `:refused` (refused) or `:unavailable` (no answer): the node's own check
/// runs before it.
struct FakeLauncher {
    room_metadata: String,
    /// The room metadata once the bot sends its first data packet.
    metadata_on_first_data: Option<String>,
    /// How long a connect takes.
    delay: Duration,
    /// The URLs bots were started for.
    dialed: Mutex<Vec<String>>,
    bots: Mutex<Vec<(Arc<FakeBot>, Option<FakeRoom>)>>,
}

impl FakeLauncher {
    fn new(room_metadata: Value) -> Arc<Self> {
        Self::slow(room_metadata, Duration::ZERO)
    }
    fn slow(room_metadata: Value, delay: Duration) -> Arc<Self> {
        Arc::new(Self {
            room_metadata: room_metadata.to_string(),
            metadata_on_first_data: None,
            delay,
            dialed: Mutex::new(Vec::new()),
            bots: Mutex::new(Vec::new()),
        })
    }
    fn last(&self) -> (Arc<FakeBot>, FakeRoom) {
        let bots = self.bots.lock().unwrap();
        let (bot, room) = bots.last().expect("a bot");
        (bot.clone(), room.clone().expect("a connected bot"))
    }
    /// The last bot's process ends: its room channels close.
    fn exit_last(&self) {
        let mut bots = self.bots.lock().unwrap();
        let (bot, room) = bots.last_mut().expect("a bot");
        bot.exited.store(true, Ordering::SeqCst);
        *room = None;
    }
}

#[async_trait::async_trait]
impl BotLauncher for FakeLauncher {
    async fn connect(&self, url: &str, token: &str) -> Result<Connected, ConnectError> {
        self.dialed.lock().unwrap().push(url.to_string());
        tokio::time::sleep(self.delay).await;
        let identity =
            crate::shared::livekit::verify_access_token(API_KEY, API_SECRET, token, unix_now())
                .and_then(|claims| claims.get("sub")?.as_str().map(str::to_string))
                .unwrap_or_default();
        if identity.ends_with(":unavailable") {
            return Err(ConnectError::Unavailable("no bot".into()));
        }
        if identity.is_empty() || identity.ends_with(":refused") {
            return Err(ConnectError::Refused("unauthorized".into()));
        }
        let bot = Arc::new(FakeBot::default());
        let (data_tx, data) = mpsc::channel(16);
        let (state_tx, room) = watch::channel(RoomState {
            metadata: self.room_metadata.clone(),
            participants: 1,
        });
        let state = Arc::new(state_tx);
        if let Some(metadata) = self.metadata_on_first_data.clone() {
            let state = state.clone();
            *bot.on_first_data.lock().unwrap() = Some(Box::new(move || {
                state.send_modify(|room| room.metadata = metadata);
            }));
        }
        let fake_room = FakeRoom {
            data: data_tx,
            state,
        };
        self.bots
            .lock()
            .unwrap()
            .push((bot.clone(), Some(fake_room)));
        Ok(Connected {
            link: bot,
            data,
            room,
        })
    }
}

/// The node's LiveKit in these tests.
const OWN_LIVEKIT: &str = "wss://lk.example.org";
const API_KEY: &str = "APIpresenter";
const API_SECRET: &str = "presenter-test-secret";

fn target() -> LiveKitTarget {
    LiveKitTarget {
        client_url: OWN_LIVEKIT.into(),
        connect_url: "ws://livekit.internal:7880".into(),
        api_key: API_KEY.into(),
        api_secret: API_SECRET.into(),
    }
}

fn unix_now() -> u64 {
    crate::shared::livekit::unix_now().unwrap()
}

/// A token signed with `secret` carrying `claims`, valid for 10 minutes
/// from now unless they say otherwise.
fn token_with(secret: &str, mut claims: Value) -> String {
    let now = unix_now();
    let claims_map = claims.as_object_mut().unwrap();
    for (key, value) in [
        ("iss", json!(API_KEY)),
        ("nbf", json!(now)),
        ("exp", json!(now + 600)),
    ] {
        claims_map.entry(key).or_insert(value);
    }
    crate::shared::livekit::sign_hs256(
        secret,
        br#"{"alg":"HS256","typ":"JWT"}"#,
        claims.to_string().as_bytes(),
    )
    .unwrap()
}

/// The gatekeeper's presentation-bot grant over `room`.
fn bot_grant(room: &str) -> Value {
    json!({"roomJoin": true, "room": room, "canPublish": true, "canSubscribe": true,
           "canPublishData": true, "canUpdateOwnMetadata": true, "roomList": false})
}

/// A presentation-bot token of `room` whose identity ends in `tail` (see
/// [`FakeLauncher`]).
fn room_token(room: &str, tail: &str) -> String {
    token_with(
        API_SECRET,
        json!({"sub": format!("presentation-bot:{room}:{tail}"), "video": bot_grant(room)}),
    )
}

/// [`room_token`] of `scene:room`.
fn bot_token(tail: &str) -> String {
    room_token("scene:room", tail)
}

/// A valid presentation-bot token LiveKit accepts.
static GOOD: LazyLock<String> = LazyLock::new(|| bot_token("access:1"));

/// A player's scene-room token, as admission mints it: microphone only.
fn guest_token() -> String {
    let mut grant = bot_grant("scene:room");
    grant["canPublishSources"] = json!(["microphone"]);
    token_with(
        API_SECRET,
        json!({"sub": "0x1111111111111111111111111111111111111111", "video": grant}),
    )
}

// ── a PDF to present ──

/// A PDF of `pages` 400x300 pages; the first carries a link to a video at
/// [40 30 240 150] and one to a web page.
fn test_pdf(pages: usize) -> Vec<u8> {
    let mut objects: Vec<String> = Vec::new();
    objects.push("<< /Type /Catalog /Pages 2 0 R >>".into());
    let kids: Vec<String> = (0..pages).map(|i| format!("{} 0 R", 3 + i * 2)).collect();
    objects.push(format!(
        "<< /Type /Pages /Kids [{}] /Count {pages} >>",
        kids.join(" ")
    ));
    for i in 0..pages {
        let content = 3 + i * 2 + 1;
        let annots = if i == 0 {
            "/Annots [<< /Type /Annot /Subtype /Link /Rect [240 150 40 30] /A << /S /URI /URI (https://www.youtube.com/watch?v=abc) >> >> \
             << /Type /Annot /Subtype /Link /Rect [0 0 10 10] /A << /S /URI /URI (https://example.org/page) >> >>]"
        } else {
            ""
        };
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 300] /Contents {content} 0 R {annots} >>"
        ));
        let stream = format!("0 0 1 rg {} 0 100 100 re f", 10 * i);
        objects.push(format!(
            "<< /Length {} >>\nstream\n{stream}\nendstream",
            stream.len()
        ));
    }
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

/// A presenter's command as the Admin Toolkit's scene sends it.
fn command_packet(message: Value, scene_id: &str) -> Vec<u8> {
    let json = message.to_string();
    let mut data = vec![3u8, 12, 0];
    data.extend_from_slice(b"presentation");
    data.extend_from_slice(json.as_bytes());
    Packet {
        message: Some(packet::Message::Scene(Scene {
            scene_id: scene_id.into(),
            data,
        })),
        protocol_version: 100,
    }
    .encode_to_vec()
}

fn data_messages(bot: &FakeBot) -> Vec<(Value, String)> {
    bot.sent
        .lock()
        .unwrap()
        .iter()
        .filter_map(|s| match s {
            Sent::Data(packet) => {
                let incoming = decode_packet(packet).expect("own packet decodes");
                Some((
                    Value::Object(incoming.message),
                    incoming.scene_id.unwrap_or_default(),
                ))
            }
            _ => None,
        })
        .collect()
}

async fn settle() {
    for _ in 0..50 {
        tokio::task::yield_now().await;
    }
    tokio::time::sleep(Duration::from_millis(50)).await;
}

/// Waits up to 10 s of real time for `done`, then settles; panics if it
/// never holds. The wait never advances tokio time, so it also works while
/// the clock is paused and rendering runs on blocking threads.
async fn settle_until(done: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(
            std::time::Instant::now() < deadline,
            "condition not met within 10 s"
        );
        tokio::task::yield_now().await;
        std::thread::sleep(Duration::from_millis(1));
    }
    settle().await;
}

// ── wire contract ──

#[test]
fn the_packet_is_an_rfc4_scene_message_on_the_presentation_topic() {
    // comms.ts:8-18, 54-60: [3][len LE16][topic][json] in Packet.scene, version 100.
    let packet = encode_packet(r#"{"type":"presentation:get-state"}"#, "scene-1");
    let decoded = Packet::decode(packet.as_slice()).unwrap();
    assert_eq!(decoded.protocol_version, 100);
    let Some(packet::Message::Scene(scene)) = decoded.message else {
        panic!("not a scene message")
    };
    assert_eq!(scene.scene_id, "scene-1");
    let mut expected = vec![3u8, 12, 0];
    expected.extend_from_slice(b"presentation{\"type\":\"presentation:get-state\"}");
    assert_eq!(scene.data, expected);
    // Byte for byte: field 6 (scene) then field 11 (protocol_version = 100).
    assert_eq!(&packet[..2], &[0x32, (2 + 7 + 2 + expected.len()) as u8]);
    assert_eq!(&packet[packet.len() - 2..], &[0x58, 100]);
}

#[test]
fn incoming_packets_decode_like_upstream() {
    let incoming =
        decode_packet(&command_packet(json!({"type": "presentation:stop"}), "s")).unwrap();
    assert_eq!(incoming.message["type"], "presentation:stop");
    assert_eq!(incoming.scene_id.as_deref(), Some("s"));
    // An empty scene id is none.
    let incoming = decode_packet(&command_packet(json!({"type": "x"}), "")).unwrap();
    assert_eq!(incoming.scene_id, None);
    // Raw JSON when the bytes are not a Packet.
    let raw = decode_packet(br#"{"type":"presentation:get-state"}"#).unwrap();
    assert_eq!(raw.message["type"], "presentation:get-state");
    assert_eq!(raw.scene_id, None);
    // Another topic, another packet kind, or garbage: dropped.
    let mut other = Packet::decode(command_packet(json!({}), "s").as_slice()).unwrap();
    if let Some(packet::Message::Scene(scene)) = &mut other.message {
        scene.data = vec![3, 4, 0, b'c', b'h', b'a', b't', b'{', b'}'];
    }
    assert_eq!(decode_packet(&other.encode_to_vec()), None);
    let chat = Packet {
        message: Some(packet::Message::Chat(Default::default())),
        protocol_version: 100,
    };
    assert_eq!(decode_packet(&chat.encode_to_vec()), None);
    assert_eq!(decode_packet(b"\xff\xfe not json"), None);
}

#[test]
fn presenters_are_the_lowercased_room_metadata_list() {
    let set = presenters_from_room_metadata(r#"{"presenters":["0xABC","",7,"stream:x"]}"#);
    assert_eq!(set.len(), 2);
    assert!(set.contains("0xabc") && set.contains("stream:x"));
    assert!(presenters_from_room_metadata("").is_empty());
    assert!(presenters_from_room_metadata(r#"{"presenters":"0xabc"}"#).is_empty());
    assert!(presenters_from_room_metadata("not json").is_empty());
}

fn sample_state() -> PresentationState {
    PresentationState {
        id: "11111111-2222-3333-4444-555555555555".into(),
        file_name: "Quarterly Review".into(),
        slide_count: 15,
        current_slide: 2,
        file_type: FileType::Pdf,
        slide_videos: vec![SlideVideo {
            url: "https://drive.google.com/x".into(),
            geometry: Geometry {
                x: 100,
                y: 200,
                width: 640,
                height: 360,
            },
        }],
        video_state: VideoState::Idle,
        overlay: DEFAULT_OVERLAY,
        composition: None,
    }
}

#[test]
fn the_state_broadcast_and_bot_metadata_match_upstream_byte_for_byte() {
    // component.ts:320-335, 1432-1448; livekit-publisher/component.ts:62, 208-211.
    let state = sample_state();
    assert_eq!(
        state_message(&state),
        r#"{"type":"presentation:state","id":"11111111-2222-3333-4444-555555555555","fileName":"Quarterly Review","slideCount":15,"currentSlide":2,"fileType":"pdf","slideVideos":[{"url":"https://drive.google.com/x","geometry":{"x":100,"y":200,"width":640,"height":360}}],"videoState":"idle","overlay":{"x":0,"y":1,"size":"small"}}"#
    );
    assert_eq!(
        bot_metadata(&state.id, None),
        r#"{"role":"presentation","presentationId":"11111111-2222-3333-4444-555555555555"}"#
    );
    assert_eq!(
        bot_metadata(&state.id, Some(&state)),
        r#"{"role":"presentation","presentationId":"11111111-2222-3333-4444-555555555555","id":"11111111-2222-3333-4444-555555555555","fileName":"Quarterly Review","slideCount":15,"currentSlide":2,"fileType":"pdf","slideVideos":[{"url":"https://drive.google.com/x","geometry":{"x":100,"y":200,"width":640,"height":360}}],"videoState":"idle","overlay":{"x":0,"y":1,"size":"small"}}"#
    );
    assert_eq!(
        stopped_message("abc"),
        r#"{"type":"presentation:stopped","id":"abc"}"#
    );
    let info = PresentationInfo {
        id: "abc".into(),
        file_name: "deck".into(),
        slide_count: 3,
        current_slide: 0,
        file_type: FileType::Pdf,
    };
    assert_eq!(
        serde_json::to_string(&info).unwrap(),
        r#"{"id":"abc","fileName":"deck","slideCount":3,"currentSlide":0,"fileType":"pdf"}"#
    );
}

#[test]
fn overlay_numbers_print_as_json_stringify() {
    let mut overlay = DEFAULT_OVERLAY;
    OverlayPatch {
        x: Some(0.5),
        y: Some(0.000001),
        size: Some(OverlaySize::Large),
    }
    .apply(&mut overlay);
    assert_eq!(
        serde_json::to_string(&overlay).unwrap(),
        r#"{"x":0.5,"y":0.000001,"size":"large"}"#
    );
}

#[test]
fn commands_parse_as_upstream_reads_them() {
    let parse = |v: Value| parse_command(v.as_object().unwrap());
    assert_eq!(
        parse(json!({"type": "presentation:navigate", "action": "next"})),
        Some(Command::Navigate(Navigate::Next))
    );
    assert_eq!(
        parse(json!({"type": "presentation:navigate", "action": "goto", "slideIndex": 3})),
        Some(Command::Navigate(Navigate::Goto(Some(3.0))))
    );
    // Number.isInteger and >= 0; anything else is no index.
    for bad in [json!(1.5), json!(-1), json!("2"), json!(null)] {
        assert_eq!(
            parse(json!({"type": "presentation:navigate", "action": "goto", "slideIndex": bad})),
            Some(Command::Navigate(Navigate::Goto(None)))
        );
    }
    assert_eq!(
        parse(json!({"type": "presentation:navigate", "action": "jump"})),
        None
    );
    assert_eq!(
        parse(json!({"type": "presentation:get-state"})),
        Some(Command::GetState)
    );
    assert_eq!(
        parse(json!({"type": "presentation:stop"})),
        Some(Command::Stop)
    );
    assert_eq!(
        parse(json!({"type": "presentation:overlay:update", "x": 2, "size": "small"})),
        Some(Command::Overlay(OverlayPatch {
            x: Some(1.0),
            y: None,
            size: Some(OverlaySize::Small)
        }))
    );
    // One invalid field drops the whole update; an empty one is nothing.
    assert_eq!(
        parse(json!({"type": "presentation:overlay:update", "x": 0.2, "size": "huge"})),
        None
    );
    assert_eq!(parse(json!({"type": "presentation:overlay:update"})), None);
    assert_eq!(
        parse(json!({"type": "presentation:video:play", "videoIndex": 0})),
        Some(Command::VideoPlay(Some(0)))
    );
    assert_eq!(
        parse(json!({"type": "presentation:video:pause"})),
        Some(Command::Unsupported("presentation:video:pause".into()))
    );
    assert_eq!(
        parse(json!({"type": "presentation:presenter:claim"})),
        Some(Command::Claim)
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&video_unavailable_message(Some(2))).unwrap(),
        json!({
            "type": "presentation:error",
            "code": "video-playback-failed",
            "message": "Video playback is not available on this server",
            "videoIndex": 2
        })
    );
    assert_eq!(parse(json!({"type": "presentation:unknown"})), None);
}

// ── request checks ──

#[test]
fn file_names_are_sanitized_and_typed_like_node() {
    assert_eq!(
        files::sanitize_filename("../a/b/My Deck (1).pdf"),
        "My_Deck__1_.pdf"
    );
    assert_eq!(files::sanitize_filename("dir/"), "dir");
    // One `_` per UTF-16 code unit.
    assert_eq!(files::sanitize_filename("é😀.pdf"), "___.pdf");
    assert_eq!(files::file_type_from_name("a.PDF"), Some(FileType::Pdf));
    assert_eq!(files::file_type_from_name("a.pptx"), Some(FileType::Pptx));
    assert_eq!(files::file_type_from_name(".pdf"), None);
    assert_eq!(files::file_type_from_name("..pdf"), Some(FileType::Pdf));
    assert_eq!(files::file_type_from_name("a.pdf.txt"), None);
    assert_eq!(files::file_type_from_name("pdf"), None);
    assert_eq!(files::presentation_name("deck.v2.pdf"), "deck.v2");
    assert_eq!(files::presentation_name(".pdf"), "Presentation");
}

#[test]
fn magic_bytes_gate_each_type() {
    assert!(files::magic_bytes_match(b"%PDF-1.7", FileType::Pdf));
    assert!(!files::magic_bytes_match(b"%PD", FileType::Pdf));
    assert!(!files::magic_bytes_match(b"PK\x03\x04", FileType::Pdf));
    assert!(files::magic_bytes_match(b"PK\x03\x04rest", FileType::Pptx));
}

#[test]
fn livekit_urls_and_presenter_identities() {
    let own = target();
    let check = |url: &str| files::check_livekit_url(url, &own);
    assert_eq!(check("wss://lk.example.org"), Ok(()));
    assert_eq!(check("wss://lk.example.org/"), Ok(()));
    // Only the node's own LiveKit: a caller's server is refused.
    assert_eq!(
        check("wss://attacker.example"),
        Err("livekitUrl is not this node's LiveKit server")
    );
    assert_eq!(
        check("wss://lk.example.org:8443"),
        Err("livekitUrl is not this node's LiveKit server")
    );
    // ws:// on loopback only when it is the node's own (the local stack).
    assert_eq!(
        check("ws://127.0.0.1:7880"),
        Err("livekitUrl must use wss:// protocol")
    );
    let local = LiveKitTarget {
        client_url: "ws://127.0.0.1:7880".into(),
        connect_url: "ws://127.0.0.1:7880".into(),
        ..target()
    };
    assert_eq!(
        files::check_livekit_url("ws://127.0.0.1:7880", &local),
        Ok(())
    );
    assert_eq!(
        files::check_livekit_url("ws://127.0.0.1:7881", &local),
        Err("livekitUrl must use wss:// protocol")
    );
    assert_eq!(
        check("ws://lk.example.org"),
        Err("livekitUrl must use wss:// protocol")
    );
    assert_eq!(
        check("https://lk.example.org"),
        Err("livekitUrl must use wss:// protocol")
    );
    assert_eq!(check("nope"), Err("livekitUrl is not a valid URL"));
    // The bot dials the node's route: LIVEKIT_API_HOST (or LIVEKIT_HOST) as a ws URL.
    let mut livekit = crate::shared::livekit::LiveKit {
        api_key: API_KEY.into(),
        api_secret: API_SECRET.into(),
        client_url: "wss://lk.example.org".into(),
        api_url: "http://livekit:7880".into(),
        configured: true,
    };
    assert_eq!(
        LiveKitTarget::from_livekit(&livekit),
        Some(LiveKitTarget {
            client_url: "wss://lk.example.org".into(),
            connect_url: "ws://livekit:7880".into(),
            ..target()
        })
    );
    assert!(!format!("{:?}", target()).contains(API_SECRET));
    livekit.client_url.clear();
    assert_eq!(LiveKitTarget::from_livekit(&livekit), None);
    assert_eq!(
        files::presenter_identity(Some("stream:place:1")).as_deref(),
        Some("stream:place:1")
    );
    assert_eq!(files::presenter_identity(Some("bob")), None);
    assert_eq!(files::presenter_identity(None), None);
}

#[test]
fn sharing_links_and_download_names() {
    let slides = files::resolve_file_url("https://docs.google.com/presentation/d/AbC_1/edit");
    assert_eq!(
        slides.url,
        "https://docs.google.com/presentation/d/AbC_1/export?format=pdf"
    );
    assert_eq!(slides.filename.as_deref(), Some("AbC_1.pdf"));
    let drive = files::resolve_file_url("https://drive.google.com/file/d/XyZ/view");
    assert_eq!(
        drive.url,
        "https://drive.usercontent.google.com/download?id=XyZ&export=download&confirm=t"
    );
    let url = url::Url::parse("https://files.example.org/a/my%20deck.pdf").unwrap();
    assert_eq!(files::download_filename(None, &url), "my%20deck.pdf");
    assert_eq!(
        files::download_filename(Some("attachment; filename*=UTF-8''Q3%20plan.pdf"), &url),
        "Q3 plan.pdf"
    );
    assert_eq!(
        files::download_filename(Some(r#"inline; filename="bad%zz.pdf""#), &url),
        "bad%zz.pdf"
    );
    let root = url::Url::parse("https://files.example.org/").unwrap();
    assert_eq!(files::download_filename(None, &root), "presentation");
}

#[test]
fn private_addresses_are_refused() {
    for ip in [
        "127.0.0.1",
        "10.1.2.3",
        "172.16.0.1",
        "172.31.255.255",
        "192.168.1.1",
        "169.254.169.254",
        "100.64.0.1",
        "198.18.0.1",
        "224.0.0.1",
        "240.0.0.1",
        "255.255.255.255",
        "0.0.0.0",
        "0.1.2.3",
        "192.0.0.8",
        "::1",
        "::",
        "fe80::1",
        "fc00::1",
        "fd12::1",
        "::ffff:127.0.0.1",
        "::ffff:10.0.0.1",
        "::7f00:1",
        "2002:a00:1::",
        "64:ff9b::a00:1",
        // IPv4-translated.
        "::ffff:0:a00:1",
        "::ffff:0:7f00:1",
        // Local-use NAT64, 10.0.0.1 in its /96, /64 and /48 layouts.
        "64:ff9b:1::a00:1",
        "64:ff9b:1:0:a:0:100::",
        "64:ff9b:1:a00:0:100::",
        "fec0::1",
        "ff02::1",
        "2001:0:4136:e378::1",
    ] {
        assert!(
            ssrf::is_private_ip(ip.parse().unwrap()),
            "{ip} must be private"
        );
    }
    for ip in [
        "8.8.8.8",
        "172.32.0.1",
        "100.128.0.1",
        "2606:4700::1111",
        "::ffff:8.8.8.8",
        "2002:808:808::",
        "192.0.1.1",
        "2001:4860::8888",
        "::ffff:0:808:808",
        "64:ff9b:1::808:808",
        "64:ff9b:1:0:8:808:800::",
    ] {
        assert!(
            !ssrf::is_private_ip(ip.parse().unwrap()),
            "{ip} must be public"
        );
    }
}

#[tokio::test]
async fn urls_must_be_https_to_public_hosts() {
    assert_eq!(
        ssrf::validate_url("http://example.org/a.pdf")
            .await
            .unwrap_err(),
        "URL must use HTTPS, got http: for http://example.org/a.pdf"
    );
    assert_eq!(
        ssrf::validate_url("not a url").await.unwrap_err(),
        "Invalid URL: not a url"
    );
    assert_eq!(
        ssrf::validate_url("https://127.0.0.1/a.pdf")
            .await
            .unwrap_err(),
        "URL resolves to private IP: 127.0.0.1"
    );
    assert_eq!(
        ssrf::validate_url("https://[::1]/a.pdf").await.unwrap_err(),
        "URL resolves to private IP: [::1]"
    );
    assert!(matches!(
        ssrf::download("https://169.254.169.254/latest").await,
        Err(ssrf::DownloadError::InvalidUrl(_))
    ));
}

// ── rendering ──

#[test]
fn pages_fit_full_hd_with_even_sides() {
    assert_eq!(fit_render_size(400.0, 300.0).unwrap(), (3.6, 1440, 1080));
    let (_, w, h) = fit_render_size(612.0, 792.0).unwrap();
    assert_eq!((w, h), (1080, 1398));
    let (_, w, h) = fit_render_size(1.0, 100000.0).unwrap();
    assert_eq!((w, h), (2, 1920));
    assert!(fit_render_size(0.0, 1.0).is_err());
    assert!(is_media_url("https://youtu.be/x"));
    assert!(is_media_url("https://cdn.example.org/clip.MP4?x=1"));
    assert!(!is_media_url("https://example.org/page"));
}

#[test]
fn a_pdf_renders_with_its_video_links() {
    let doc = PdfDocument::load(test_pdf(2)).unwrap();
    assert_eq!(doc.page_count(), 2);
    let (frame, videos) = doc.render(0).unwrap();
    assert_eq!((frame.width, frame.height), (1440, 1080));
    assert_eq!(
        videos,
        vec![SlideVideo {
            url: "https://www.youtube.com/watch?v=abc".into(),
            geometry: Geometry {
                x: 144,
                y: 540,
                width: 720,
                height: 432
            },
        }]
    );
    let pixel = |x: u32, y: u32| {
        let i = ((y * frame.width + x) * 4) as usize;
        frame.rgba[i..i + 4].to_vec()
    };
    // White page, the blue square bottom-left, the video box black.
    assert_eq!(pixel(1400, 20), vec![255, 255, 255, 255]);
    assert_eq!(pixel(20, 1060), vec![0, 0, 255, 255]);
    assert_eq!(pixel(150, 550), vec![0, 0, 0, 255]);
    let (_, videos) = doc.render(1).unwrap();
    assert!(videos.is_empty());
    assert!(doc.render(2).is_err());
    assert!(PdfDocument::load(b"%PDF-garbage".to_vec()).is_err());
}

// ── sessions ──

const PRESENTER: &str = "0xPresenter";
/// The presenter's public URL in the tests that compose.
const SLIDES_AT: &str = "https://cast-presenter-service.example.org";
const STREAMER: &str = "stream:place:1";

async fn presenting(pages: usize) -> (Arc<Presenter>, Arc<FakeLauncher>, PresentationInfo) {
    presenting_at(pages, None).await
}

/// A presentation by [`STREAMER`] on a presenter with `public_url`.
async fn presenting_at(
    pages: usize,
    public_url: Option<&str>,
) -> (Arc<Presenter>, Arc<FakeLauncher>, PresentationInfo) {
    let launcher = FakeLauncher::new(json!({"presenters": [PRESENTER.to_lowercase()]}));
    let presenter = Presenter::new(launcher.clone(), target(), 10, public_url.map(Into::into));
    let info = presenter
        .create(test_pdf(pages), &GOOD, "Deck.pdf", Some(STREAMER.into()))
        .await
        .unwrap();
    (presenter, launcher, info)
}

/// Every state the bot sent: the broadcasts, then the metadata.
fn states(bot: &FakeBot) -> Vec<Value> {
    let sent = bot.sent.lock().unwrap().clone();
    let metadata = sent.into_iter().filter_map(|s| match s {
        Sent::Metadata(m) => serde_json::from_str(&m).ok(),
        _ => None,
    });
    let broadcasts = data_messages(bot).into_iter().map(|(m, _)| m);
    broadcasts.chain(metadata).collect()
}

/// `GET` on the presenter's router: the status, the content type and cache
/// control, the body.
async fn fetch(
    presenter: impl Into<Option<Arc<Presenter>>>,
    path: &str,
) -> (StatusCode, String, Vec<u8>) {
    let request = Request::get(path).body(Body::empty()).unwrap();
    let router = super::router(presenter.into());
    let (parts, body) = router.oneshot(request).await.unwrap().into_parts();
    let header = |name| parts.headers.get(name).map_or("", |v| v.to_str().unwrap());
    let headers = format!("{}; {}", header("content-type"), header("cache-control"));
    let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    (parts.status, headers, body.to_vec())
}

async fn send(room: &FakeRoom, identity: &str, command: Value) {
    room.data(identity, command_packet(command, "")).await;
}

/// With no public URL the node does not compose: no state names a slide or a
/// presenter, a claim changes nothing, and the slides route has no picture.
#[tokio::test]
async fn without_a_public_url_no_state_names_a_slide() {
    let (presenter, launcher, info) = presenting(2).await;
    let (bot, room) = launcher.last();
    send(
        &room,
        PRESENTER,
        json!({"type": "presentation:presenter:claim"}),
    )
    .await;
    let next = json!({"type": "presentation:navigate", "action": "next"});
    send(&room, PRESENTER, next).await;
    settle_until(|| data_messages(&bot).len() >= 2).await;
    assert_eq!(data_messages(&bot).len(), 2);
    for state in states(&bot) {
        assert_eq!(state.get("slide"), None, "{state}");
        assert_eq!(state.get("presenterIdentity"), None, "{state}");
    }
    let path = format!("/presentations/{}/slides/0123456789abcdef.png", info.id);
    assert_eq!(fetch(presenter, &path).await.0, StatusCode::NOT_FOUND);
    assert_eq!(fetch(None, &path).await.0, StatusCode::NOT_FOUND);
}

/// Client composition: every state that names a slide names one the route
/// serves at that moment, and the session keeps the last eight.
#[tokio::test]
async fn a_composing_session_serves_each_slide_it_names() {
    use sha2::{Digest, Sha256};
    let (presenter, launcher, info) = presenting_at(10, Some(SLIDES_AT)).await;
    let (bot, room) = launcher.last();
    let slides = format!("/presentations/{}/slides/", info.id);
    let status = async |path: &str| fetch(presenter.clone(), path).await.0;
    let mut paths = Vec::new();
    for slide in 0..10 {
        settle_until(|| states(&bot).iter().any(|s| s["currentSlide"] == slide)).await;
        let all = states(&bot);
        let shown: Vec<&Value> = all.iter().filter(|s| s["currentSlide"] == slide).collect();
        // The broadcast and the metadata carry the same fields.
        assert_eq!(shown.len(), 2, "{all:?}");
        assert_eq!(shown[0]["slide"], shown[1]["slide"]);
        assert_eq!(shown[0]["presenterIdentity"], STREAMER);
        // The address a composing client fetches: the public URL, the
        // session's UUID, 16 hex characters.
        let url = shown[0]["slide"]["url"].as_str().unwrap();
        let path = url.strip_prefix(SLIDES_AT).expect(url).to_string();
        let (code, headers, png) = fetch(presenter.clone(), &path).await;
        assert_eq!(code, StatusCode::OK);
        assert_eq!(headers, "image/png; private, max-age=31536000, immutable");
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
        let hash = hex::encode(&Sha256::digest(&png)[..8]);
        assert_eq!(path, format!("{slides}{hash}.png"));
        assert!(!paths.contains(&path), "each slide has its own picture");
        paths.push(path);
        let next = json!({"type": "presentation:navigate", "action": "next"});
        send(&room, PRESENTER, next).await;
    }
    // Only the connect metadata, before the first slide, names none.
    let unnamed = |s: &&Value| s.get("slide").is_none();
    assert_eq!(states(&bot).iter().filter(unnamed).count(), 1);
    // Ten slides shown: the last eight are kept.
    for (slide, path) in paths.iter().enumerate() {
        assert_eq!(status(path).await == StatusCode::OK, slide >= 2, "{slide}");
    }
    for unknown in ["0123456789abcdef.png", "x", ".png"] {
        let path = format!("{slides}{unknown}");
        assert_eq!(status(&path).await, StatusCode::NOT_FOUND);
    }
    // A presenter claims the camera place; anyone else does not.
    let claim = json!({"type": "presentation:presenter:claim"});
    send(&room, "0xSomeoneElse", claim.clone()).await;
    send(&room, PRESENTER, claim).await;
    let claimed = |s: &&Value| s["presenterIdentity"] == PRESENTER;
    settle_until(|| states(&bot).iter().filter(claimed).count() == 2).await;
    let intruder = |s: &Value| s["presenterIdentity"] == "0xSomeoneElse";
    assert!(!states(&bot).iter().any(intruder));
    let sent = bot.sent.lock().unwrap().clone();
    // On the wire the fields follow `overlay` (component.ts:1450-1476).
    let Some(Sent::Metadata(last)) = sent.last() else {
        panic!("{sent:?}")
    };
    assert!(last.contains(r#","overlay":{"x":0,"y":1,"size":"small"},"slide":{"url":""#));
    assert!(last.ends_with(r#"","width":1440,"height":1080},"presenterIdentity":"0xPresenter","playingVideoIndex":null,"videoPositionMs":null,"videoPositionAt":null}"#));
    // A stopped session serves nothing.
    presenter.stop(&info.id).await;
    assert_eq!(status(&paths[9]).await, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_connects_publishes_the_first_slide_and_broadcasts() {
    let (presenter, launcher, info) = presenting(3).await;
    assert_eq!(
        (
            info.file_name.as_str(),
            info.slide_count,
            info.current_slide
        ),
        ("Deck", 3, 0)
    );
    let (bot, _) = launcher.last();
    let sent = bot.sent.lock().unwrap().clone();
    let metadata = format!(
        r#"{{"role":"presentation","presentationId":"{}"}}"#,
        info.id
    );
    assert_eq!(sent[0], Sent::Metadata(metadata));
    assert_eq!(sent[1], Sent::PublishVideo(1440, 1080));
    assert_eq!(sent[2], Sent::Frame(1440, 1080));
    let (state, scene) = &data_messages(&bot)[0];
    assert_eq!(state["type"], "presentation:state");
    assert_eq!(state["currentSlide"], 0);
    assert_eq!(
        state["slideVideos"][0]["url"],
        "https://www.youtube.com/watch?v=abc"
    );
    assert_eq!(scene, "");
    assert!(matches!(sent.last(), Some(Sent::Metadata(m)) if m.contains("\"currentSlide\":0")));
    assert_eq!(presenter.active_sessions(), 1);
}

#[tokio::test]
async fn presenters_navigate_and_others_are_ignored() {
    let (presenter, launcher, _) = presenting(3).await;
    let (bot, events) = launcher.last();
    let next = json!({"type": "presentation:navigate", "action": "next"});
    events
        .data("0xSomeoneElse", command_packet(next.clone(), "intruder"))
        .await;
    settle().await;
    assert_eq!(data_messages(&bot).len(), 1);
    events
        .data(PRESENTER, command_packet(next, "scene-7"))
        .await;
    settle_until(|| data_messages(&bot).len() >= 2).await;
    let messages = data_messages(&bot);
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1].0["currentSlide"], 1);
    // The answer carries the presenter's scene id.
    assert_eq!(messages[1].1, "scene-7");
    // goto out of range, and to the current slide: no broadcast.
    for slide in [9, 1] {
        let goto = json!({"type": "presentation:navigate", "action": "goto", "slideIndex": slide});
        events
            .data(PRESENTER, command_packet(goto, "scene-7"))
            .await;
    }
    // get-state answers with the current state; raw JSON works too.
    events
        .data(PRESENTER, br#"{"type":"presentation:get-state"}"#.to_vec())
        .await;
    settle_until(|| data_messages(&bot).len() >= 3).await;
    let messages = data_messages(&bot);
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[2].0["currentSlide"], 1);
    // A presenter added later through the room metadata is obeyed.
    events.metadata(r#"{"presenters":["0xnew"]}"#);
    events
        .data(
            "0xNEW",
            command_packet(
                json!({"type": "presentation:navigate", "action": "prev"}),
                "",
            ),
        )
        .await;
    settle_until(|| data_messages(&bot).len() >= 4).await;
    assert_eq!(data_messages(&bot).last().unwrap().0["currentSlide"], 0);
    drop(presenter);
}

#[tokio::test]
async fn a_presenter_added_during_creation_is_obeyed() {
    // The room metadata changes after the session read its presenters and
    // before its loop starts: the loop starts from the new list.
    let launcher = Arc::new(FakeLauncher {
        metadata_on_first_data: Some(r#"{"presenters":["0xlate"]}"#.into()),
        ..Arc::try_unwrap(FakeLauncher::new(json!({"presenters": []})))
            .ok()
            .unwrap()
    });
    let presenter = Presenter::new(launcher.clone(), target(), 10, None);
    presenter
        .create(test_pdf(2), &GOOD, "Deck.pdf", None)
        .await
        .unwrap();
    let (bot, events) = launcher.last();
    events
        .data(
            "0xLATE",
            command_packet(
                json!({"type": "presentation:navigate", "action": "next"}),
                "",
            ),
        )
        .await;
    settle_until(|| {
        data_messages(&bot)
            .last()
            .is_some_and(|m| m.0["currentSlide"] == 1)
    })
    .await;
    assert_eq!(data_messages(&bot).last().unwrap().0["currentSlide"], 1);
}

#[tokio::test]
async fn overlay_updates_coalesce_into_one_broadcast() {
    let (_presenter, launcher, _) = presenting(1).await;
    let (bot, events) = launcher.last();
    for x in [0.1, 0.2, 0.3] {
        events
            .data(
                PRESENTER,
                command_packet(json!({"type": "presentation:overlay:update", "x": x}), ""),
            )
            .await;
    }
    settle().await;
    assert_eq!(data_messages(&bot).len(), 1);
    settle_until(|| data_messages(&bot).len() >= 2).await;
    let messages = data_messages(&bot);
    assert_eq!(messages.len(), 2);
    assert_eq!(
        messages[1].0["overlay"],
        json!({"x": 0.3, "y": 1, "size": "small"})
    );
}

#[tokio::test]
async fn stop_announces_and_disconnects() {
    let (presenter, launcher, info) = presenting(1).await;
    let (bot, events) = launcher.last();
    events
        .data(
            PRESENTER,
            command_packet(json!({"type": "presentation:stop"}), "sc"),
        )
        .await;
    settle_until(|| {
        presenter.active_sessions() == 0
            && bot.sent.lock().unwrap().last() == Some(&Sent::Shutdown)
            && data_messages(&bot)
                .last()
                .is_some_and(|m| m.0["type"] == "presentation:stopped")
    })
    .await;
    let messages = data_messages(&bot);
    assert_eq!(
        messages.last().unwrap().0,
        json!({"type": "presentation:stopped", "id": info.id})
    );
    assert_eq!(messages.last().unwrap().1, "sc");
    assert_eq!(bot.sent.lock().unwrap().last(), Some(&Sent::Shutdown));
    assert_eq!(presenter.active_sessions(), 0);
}

#[tokio::test]
async fn a_bot_that_disconnects_ends_its_session() {
    let (presenter, launcher, _) = presenting(1).await;
    launcher.exit_last();
    settle_until(|| presenter.active_sessions() == 0).await;
    assert_eq!(presenter.active_sessions(), 0);
}

/// The room service of the LiveKit at `livekit.connect_url`, as the
/// gatekeeper calls it, and a gatekeeper for which every bot holds its room's
/// active Cast credential but those in `revoked`.
struct TestRooms {
    http: reqwest::Client,
    livekit: LiveKitTarget,
    revoked: Mutex<Vec<String>>,
}

impl TestRooms {
    fn new(livekit: LiveKitTarget) -> Arc<Self> {
        Arc::new(Self {
            http: reqwest::Client::new(),
            livekit,
            revoked: Mutex::default(),
        })
    }
    /// No LiveKit answers: an eviction fails at once.
    fn offline() -> Arc<Self> {
        Self::new(LiveKitTarget {
            connect_url: "ws://127.0.0.1:9".into(),
            ..target()
        })
    }
    /// The bot of `room_token(room, tail)` no longer holds the credential.
    fn revoke(&self, room: &str, tail: &str) {
        let identity = format!("presentation-bot:{room}:{tail}");
        self.revoked.lock().unwrap().push(identity);
    }
}

#[async_trait::async_trait]
impl super::bot::Rooms for TestRooms {
    fn room_service(&self) -> RoomServiceClient<'_> {
        let own = &self.livekit;
        RoomServiceClient::new(&self.http, &own.connect_url, &own.api_key, &own.api_secret)
    }
    async fn holds_credential(&self, _room: &str, identity: &str) -> anyhow::Result<bool> {
        Ok(!self.revoked.lock().unwrap().iter().any(|r| r == identity))
    }
}

/// A killed bot says nothing and LiveKit keeps its participant: the node
/// sends `presentation:stopped` in its place and removes it. A bot that
/// left by itself is not removed.
#[tokio::test]
async fn a_bot_that_ends_without_leaving_is_removed_from_its_room() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let seen = calls.clone();
    let record = move |uri: axum::http::Uri, axum::Json(body): axum::Json<Value>| {
        seen.lock().unwrap().push((uri.path().to_string(), body));
        async { axum::Json(json!({})) }
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let connect_url = format!("ws://{}", listener.local_addr().unwrap());
    let app = axum::Router::new().fallback(record);
    crate::shared::task::spawn_with(async move { axum::serve(listener, app).await.unwrap() });

    let (presenter, launcher, info) = presenting(1).await;
    let own = LiveKitTarget {
        connect_url,
        ..target()
    };
    presenter.set_room_service(TestRooms::new(own));
    presenter.stop(&info.id).await;
    assert!(calls.lock().unwrap().is_empty());

    let info = presenter
        .create(test_pdf(1), &GOOD, "b.pdf", None)
        .await
        .unwrap();
    launcher.exit_last();
    settle_until(|| calls.lock().unwrap().len() >= 2).await;
    assert_eq!(presenter.active_sessions(), 0);
    let stopped = encode_packet(&stopped_message(&info.id), "");
    let stopped = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, stopped);
    let identity = "presentation-bot:scene:room:access:1";
    assert_eq!(
        *calls.lock().unwrap(),
        [
            (
                "/twirp/livekit.RoomService/SendData".to_string(),
                json!({"room": "scene:room", "data": stopped, "kind": "RELIABLE"}),
            ),
            (
                "/twirp/livekit.RoomService/RemoveParticipant".to_string(),
                json!({"room": "scene:room", "identity": identity}),
            ),
        ]
    );
}

#[tokio::test]
async fn sessions_without_participants_stop_after_five_minutes() {
    let (presenter, launcher, _) = presenting(1).await;
    let (bot, events) = launcher.last();
    let start = Instant::now();
    // With a participant the session stays active.
    presenter.reap_idle(start + IDLE_TIMEOUT * 2).await;
    assert_eq!(presenter.active_sessions(), 1);
    events.participants(0);
    settle().await;
    presenter
        .reap_idle(start + IDLE_TIMEOUT * 2 + Duration::from_secs(60))
        .await;
    assert_eq!(presenter.active_sessions(), 1);
    presenter
        .reap_idle(start + IDLE_TIMEOUT * 3 + Duration::from_secs(1))
        .await;
    assert_eq!(presenter.active_sessions(), 0);
    assert_eq!(bot.sent.lock().unwrap().last(), Some(&Sent::Shutdown));
}

/// The reported quota failure: a scene admin shares a deck, then shares
/// another without a Stop reaching the node. On a node with one slot the
/// second share takes the first one's place instead of answering 429.
#[tokio::test]
async fn sharing_again_without_stop_takes_the_scenes_own_slot() {
    let launcher = FakeLauncher::new(json!({}));
    let presenter = Presenter::new(launcher.clone(), target(), 1, None);
    presenter.set_room_service(TestRooms::offline());
    let first = presenter
        .create(test_pdf(1), &bot_token("access:1"), "a.pdf", None)
        .await
        .unwrap();
    let (first_bot, _) = launcher.last();
    let second = presenter
        .create(test_pdf(1), &bot_token("access:2"), "b.pdf", None)
        .await;
    assert!(second.is_ok(), "second share refused: {second:?}");
    settle_until(|| first_bot.sent.lock().unwrap().contains(&Sent::Shutdown)).await;
    let last = data_messages(&first_bot).last().unwrap().0.clone();
    assert_eq!(
        last,
        json!({"type": "presentation:stopped", "id": first.id})
    );
    assert_eq!(presenter.active_sessions(), 1);
}

/// A room runs one presentation: a new bot token of the room stops the
/// running one as it takes the slot, so a full node still takes it; another
/// room is refused. The running token again answers 409 and stops nothing.
/// A new presentation that fails has stopped the room's one all the same.
#[tokio::test]
async fn a_new_presentation_replaces_its_rooms_running_one() {
    let launcher = FakeLauncher::new(json!({}));
    let presenter = Presenter::new(launcher.clone(), target(), 2, None);
    presenter.set_room_service(TestRooms::offline());
    let first = presenter
        .create(test_pdf(1), &GOOD, "a.pdf", None)
        .await
        .unwrap();
    presenter
        .create(test_pdf(1), &room_token("other", "access:1"), "o.pdf", None)
        .await
        .unwrap();
    let shut = |bot: usize| {
        let (bot, _) = &launcher.bots.lock().unwrap()[bot];
        bot.sent.lock().unwrap().contains(&Sent::Shutdown)
    };
    assert!(matches!(
        presenter.create(test_pdf(1), &GOOD, "a.pdf", None).await,
        Err(super::session::CreateError::TokenInUse)
    ));
    let full = "Maximum concurrent presentations reached (2)";
    let third = room_token("third", "access:1");
    let refused = presenter.create(test_pdf(1), &third, "t.pdf", None).await;
    assert_eq!(refused.unwrap_err().to_string(), full);
    // An upload is refused before it is read: its room is not known yet.
    let token = bot_token("access:2");
    let fields = [
        ("livekitToken", token.as_str()),
        ("livekitUrl", OWN_LIVEKIT),
    ];
    let (ct, mp) = multipart(Some(("b.pdf", &test_pdf(1))), &fields);
    let answer = post(super::router(Some(presenter.clone())), &ct, mp).await;
    assert_eq!(
        answer,
        (StatusCode::TOO_MANY_REQUESTS, json!({"error": full}))
    );
    assert_eq!(launcher.bots.lock().unwrap().len(), 2);
    assert!(!shut(0) && !shut(1));

    let second = presenter
        .create(test_pdf(1), &token, "b.pdf", None)
        .await
        .unwrap();
    settle_until(|| shut(0)).await;
    assert_eq!(presenter.active_sessions(), 2);
    let stopped = |bot: usize| {
        data_messages(&launcher.bots.lock().unwrap()[bot].0)
            .last()
            .unwrap()
            .0
            .clone()
    };
    assert_eq!(
        stopped(0),
        json!({"type": "presentation:stopped", "id": first.id})
    );
    assert!(!shut(1) && !shut(2));

    // The second presentation runs when a new one that LiveKit refuses comes.
    assert_eq!(presenter.active_sessions(), 2);
    let failed = presenter
        .create(test_pdf(1), &bot_token("access:refused"), "c.pdf", None)
        .await;
    assert!(failed.is_err());
    settle_until(|| shut(2)).await;
    assert_eq!(
        stopped(2),
        json!({"type": "presentation:stopped", "id": second.id})
    );
    assert_eq!(presenter.active_sessions(), 1);
    assert!(!shut(1));
    // The replaced token is free again.
    assert!(presenter.reserve(&GOOD).await.is_ok());
}

/// A replaced bot holds its slot until it has shut down: the handover runs
/// one bot beyond the limit, and further shares of the room on a full node
/// are refused until the replaced bot is gone.
#[tokio::test]
async fn a_replaced_bot_holds_its_slot_until_it_shut_down() {
    let launcher = FakeLauncher::new(json!({}));
    let presenter = Presenter::new(launcher.clone(), target(), 1, None);
    presenter.set_room_service(TestRooms::offline());
    presenter
        .create(test_pdf(1), &GOOD, "a.pdf", None)
        .await
        .unwrap();
    let (first, _) = launcher.last();
    let gate = Arc::new(tokio::sync::Notify::new());
    *first.shutdown_gate.lock().unwrap() = Some(gate.clone());
    let second = presenter
        .create(test_pdf(1), &bot_token("access:2"), "b.pdf", None)
        .await
        .unwrap();
    let announced = data_messages(&first).last().unwrap().0.clone();
    assert_eq!(announced["type"], "presentation:stopped");
    let full = "Maximum concurrent presentations reached (1)";
    for tail in ["access:3", "access:4"] {
        let token = bot_token(tail);
        let again = presenter.create(test_pdf(1), &token, "c.pdf", None).await;
        assert_eq!(again.unwrap_err().to_string(), full);
    }
    assert_eq!(launcher.bots.lock().unwrap().len(), 2);
    gate.notify_one();
    settle_until(|| first.sent.lock().unwrap().contains(&Sent::Shutdown)).await;
    presenter
        .create(test_pdf(1), &bot_token("access:3"), "c.pdf", None)
        .await
        .unwrap();
    let (replaced, _) = launcher.bots.lock().unwrap()[1].clone();
    settle_until(|| replaced.sent.lock().unwrap().contains(&Sent::Shutdown)).await;
    let last = data_messages(&replaced).last().unwrap().0.clone();
    assert_eq!(
        last,
        json!({"type": "presentation:stopped", "id": second.id})
    );
    assert_eq!(presenter.active_sessions(), 1);
}

/// A creation replaced while it waits for its own predecessor's stop sends
/// no state, and its successor waits for that predecessor too: A, reserved
/// in an empty room, replaces P when admitted and waits for P's stop, which
/// stalls; B replaces A meanwhile. B publishes its track and state only once
/// P's and A's stops are out, and A says nothing. A client that takes the
/// last message it got, as the Admin Toolkit did before it checked ids,
/// ends on B.
#[tokio::test]
async fn a_new_presentation_waits_for_every_earlier_stop_of_its_room() {
    let launcher = FakeLauncher::new(json!({}));
    let presenter = Presenter::new(launcher.clone(), target(), 3, None);
    presenter.set_room_service(TestRooms::offline());
    let token_a = bot_token("access:a");
    let slot_a = presenter.reserve(&token_a).await.unwrap();
    presenter
        .create(test_pdf(1), &bot_token("access:p"), "p.pdf", None)
        .await
        .unwrap();
    let (p_bot, _) = launcher.last();
    let gate = Arc::new(tokio::sync::Notify::new());
    *p_bot.publish_gate.lock().unwrap() = Some(gate.clone());
    let creating_a = tokio::spawn({
        let presenter = presenter.clone();
        async move {
            presenter
                .create_reserved(slot_a, test_pdf(1), &token_a, "a.pdf", None)
                .await
        }
    });
    settle_until(|| p_bot.publish_waiting.load(Ordering::SeqCst)).await;
    let (a_bot, _) = launcher.last();
    let creating_b = tokio::spawn({
        let presenter = presenter.clone();
        async move {
            let token = bot_token("access:b");
            presenter.create(test_pdf(1), &token, "b.pdf", None).await
        }
    });
    settle_until(|| {
        launcher.bots.lock().unwrap().len() == 3
            && a_bot.sent.lock().unwrap().contains(&Sent::Shutdown)
    })
    .await;
    let (b_bot, _) = launcher.last();
    // P's stop still stalls: B has published nothing.
    assert!(b_bot.stamps.lock().unwrap().is_empty());
    gate.notify_one();
    let a = creating_a.await.unwrap().unwrap();
    let b = creating_b.await.unwrap().unwrap();
    settle().await;

    let a_messages: Vec<Value> = data_messages(&a_bot).into_iter().map(|m| m.0).collect();
    assert_eq!(
        a_messages,
        [json!({"type": "presentation:stopped", "id": a.id})]
    );
    // Every message of the room, in the order the bots sent them.
    let mut room: Vec<(u64, String)> = [("P", &p_bot), ("A", &a_bot), ("B", &b_bot)]
        .iter()
        .flat_map(|(name, bot)| {
            let stamps = bot.stamps.lock().unwrap().clone();
            stamps
                .into_iter()
                .map(move |(at, m)| (at, format!("{name} {}", m["type"].as_str().unwrap())))
        })
        .collect();
    room.sort_by_key(|(at, _)| *at);
    let room: Vec<String> = room.into_iter().map(|(_, m)| m).collect();
    assert_eq!(
        room,
        [
            "P track",
            "P presentation:state",
            "A track",
            "A presentation:stopped",
            "P presentation:stopped",
            "B track",
            "B presentation:state",
        ]
    );
    assert_eq!(states(&b_bot).first().unwrap()["id"], b.id);
    assert_eq!(presenter.active_sessions(), 1);
    assert!(!b_bot.sent.lock().unwrap().contains(&Sent::Shutdown));
}

/// A presentation stopped by its presenter ends before the next one of its
/// room appears, though no new share replaced it: the next share waits for
/// a stop that stalls.
#[tokio::test]
async fn a_new_presentation_waits_for_a_stop_in_progress() {
    let launcher = FakeLauncher::new(json!({}));
    let presenter = Presenter::new(launcher.clone(), target(), 3, None);
    presenter.set_room_service(TestRooms::offline());
    let first = presenter
        .create(test_pdf(1), &bot_token("access:1"), "a.pdf", None)
        .await
        .unwrap();
    let (first_bot, _) = launcher.last();
    let gate = Arc::new(tokio::sync::Notify::new());
    *first_bot.publish_gate.lock().unwrap() = Some(gate.clone());
    let stopping = tokio::spawn({
        let presenter = presenter.clone();
        let id = first.id.clone();
        async move { presenter.stop(&id).await }
    });
    settle_until(|| first_bot.publish_waiting.load(Ordering::SeqCst)).await;
    let creating = tokio::spawn({
        let presenter = presenter.clone();
        async move {
            let token = bot_token("access:2");
            presenter.create(test_pdf(1), &token, "b.pdf", None).await
        }
    });
    settle_until(|| launcher.bots.lock().unwrap().len() == 2).await;
    let (second_bot, _) = launcher.last();
    assert!(second_bot.stamps.lock().unwrap().is_empty());
    gate.notify_one();
    stopping.await.unwrap();
    creating.await.unwrap().unwrap();
    let stopped = first_bot.stamps.lock().unwrap().last().unwrap().0;
    let track = second_bot.stamps.lock().unwrap().first().unwrap().0;
    assert!(stopped < track);
}

/// A bot token of a revoked or rotated Cast credential, still signed and
/// unexpired, replaces nothing: on a full node it is refused before a bot
/// starts, otherwise its bot is shut down when it would join the room's
/// presentation (401). A creation whose credential is revoked while it runs
/// does not replace the presentation admitted meanwhile.
#[tokio::test]
async fn a_revoked_credential_replaces_nothing() {
    let launcher = FakeLauncher::slow(json!({}), Duration::from_millis(50));
    let rooms = TestRooms::offline();
    rooms.revoke("scene:room", "access:old");
    let presenter = Presenter::new(launcher.clone(), target(), 1, None);
    presenter.set_room_service(rooms.clone());
    presenter
        .create(test_pdf(1), &GOOD, "a.pdf", None)
        .await
        .unwrap();
    let old = bot_token("access:old");
    let refused = presenter.create(test_pdf(1), &old, "x.pdf", None).await;
    let full = "Maximum concurrent presentations reached (1)";
    assert_eq!(refused.unwrap_err().to_string(), full);
    let json_form = json!({"url": "https://example.org/x.pdf", "livekitToken": old,
                           "livekitUrl": OWN_LIVEKIT});
    let (status, _) = post(
        super::router(Some(presenter.clone())),
        "application/json",
        json_form.to_string().into_bytes(),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    let (bot, _) = launcher.last();
    settle().await;
    assert!(!bot.sent.lock().unwrap().contains(&Sent::Shutdown));
    assert_eq!(launcher.bots.lock().unwrap().len(), 1);

    let roomy = Presenter::new(launcher.clone(), target(), 3, None);
    roomy.set_room_service(rooms.clone());
    roomy
        .create(test_pdf(1), &GOOD, "a.pdf", None)
        .await
        .unwrap();
    let (kept, _) = launcher.last();
    let beside = roomy.create(test_pdf(1), &old, "x.pdf", None).await;
    assert!(matches!(
        beside,
        Err(super::session::CreateError::InvalidCredentials(_))
    ));
    settle().await;
    assert!(!kept.sent.lock().unwrap().contains(&Sent::Shutdown));
    assert!(
        launcher
            .last()
            .0
            .sent
            .lock()
            .unwrap()
            .contains(&Sent::Shutdown)
    );
    assert_eq!(roomy.active_sessions(), 1);

    // Revoked in flight: both start in an empty room, the revoked one
    // replaces nothing when it is admitted.
    let empty = room_token("empty", "access:new");
    let stale = room_token("empty", "access:stale");
    rooms.revoke("empty", "access:stale");
    let (fresh, revoked) = tokio::join!(
        roomy.create(test_pdf(1), &empty, "n.pdf", None),
        roomy.create(test_pdf(1), &stale, "s.pdf", None),
    );
    let fresh = fresh.unwrap().id;
    settle().await;
    let bots = launcher.bots.lock().unwrap().clone();
    let live: Vec<_> = bots[bots.len() - 2..]
        .iter()
        .filter(|(bot, _)| !bot.sent.lock().unwrap().contains(&Sent::Shutdown))
        .map(|(bot, _)| states(bot).last().unwrap()["presentationId"].clone())
        .collect();
    assert_eq!(live, [json!(fresh)], "revoked: {revoked:?}");
}

/// Two creations of one room at once both run; the one admitted last stops
/// the other, so the room ends with one presentation.
#[tokio::test]
async fn creations_of_one_room_at_once_leave_one_presentation() {
    let launcher = FakeLauncher::slow(json!({}), Duration::from_millis(50));
    let presenter = Presenter::new(launcher.clone(), target(), 3, None);
    presenter.set_room_service(TestRooms::offline());
    let tokens = [bot_token("access:1"), bot_token("access:2")];
    let (a, b) = tokio::join!(
        presenter.create(test_pdf(1), &tokens[0], "a.pdf", None),
        presenter.create(test_pdf(1), &tokens[1], "b.pdf", None),
    );
    let ids = [a.unwrap().id, b.unwrap().id];
    let bots = launcher.bots.lock().unwrap().clone();
    assert_eq!(bots.len(), 2);
    let shut = |bot: &FakeBot| bot.sent.lock().unwrap().contains(&Sent::Shutdown);
    settle_until(|| bots.iter().any(|(bot, _)| shut(bot))).await;
    assert_eq!(presenter.active_sessions(), 1);
    assert_eq!(bots.iter().filter(|(bot, _)| shut(bot)).count(), 1);
    let (stopped, _) = bots.iter().find(|(bot, _)| shut(bot)).unwrap();
    let last = data_messages(stopped).last().unwrap().0.clone();
    assert_eq!(last["type"], "presentation:stopped");
    assert!(ids.iter().any(|id| last["id"] == *id));
}

/// A stopping session keeps its token until its bot has shut down: a retry
/// with that token inside the stop gets a 409, never a bot the old
/// session's cleanup would remove by the same identity.
#[tokio::test]
async fn a_token_stays_taken_until_its_stopped_bot_shut_down() {
    let launcher = FakeLauncher::new(json!({}));
    let presenter = Presenter::new(launcher.clone(), target(), 3, None);
    let info = presenter
        .create(test_pdf(1), &GOOD, "a.pdf", None)
        .await
        .unwrap();
    let (bot, _room) = launcher.last();
    let gate = Arc::new(tokio::sync::Notify::new());
    *bot.shutdown_gate.lock().unwrap() = Some(gate.clone());
    let stopping = tokio::spawn({
        let presenter = presenter.clone();
        async move { presenter.stop(&info.id).await }
    });
    settle_until(|| presenter.active_sessions() == 0).await;
    assert!(matches!(
        presenter.reserve(&GOOD).await,
        Err(super::session::CreateError::TokenInUse)
    ));
    gate.notify_one();
    stopping.await.unwrap();
    assert!(bot.sent.lock().unwrap().contains(&Sent::Shutdown));
    assert!(presenter.reserve(&GOOD).await.is_ok());
}

#[tokio::test]
async fn the_concurrency_limit_answers_before_connecting() {
    let launcher = FakeLauncher::new(json!({}));
    let presenter = Presenter::new(launcher.clone(), target(), 1, None);
    presenter
        .create(test_pdf(1), &GOOD, "a.pdf", None)
        .await
        .unwrap();
    let err = presenter
        .create(test_pdf(1), &room_token("b", "access:1"), "b.pdf", None)
        .await
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Maximum concurrent presentations reached (1)"
    );
    assert_eq!(launcher.bots.lock().unwrap().len(), 1);
    // 0 is unlimited.
    let unlimited = Presenter::new(launcher.clone(), target(), 0, None);
    for room in ["a", "b", "c"] {
        let token = room_token(room, "access:1");
        unlimited
            .create(test_pdf(1), &token, "c.pdf", None)
            .await
            .unwrap();
    }
}

// ── the route ──

/// The Cast page posts the form cross-origin: the preflight is answered for
/// any origin, without credentials.
#[tokio::test]
async fn the_route_answers_a_cors_preflight_for_any_origin_without_credentials() {
    let response = super::router(None)
        .oneshot(
            Request::options("/presentations")
                .header("origin", "https://cast.example")
                .header("access-control-request-method", "POST")
                .header("access-control-request-headers", "content-type")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let headers = response.headers();
    assert_eq!(headers["access-control-allow-origin"], "*");
    assert_eq!(headers["access-control-allow-headers"], "content-type");
    assert!(!headers.contains_key("access-control-allow-credentials"));
}

async fn post(app: axum::Router, content_type: &str, body: Vec<u8>) -> (StatusCode, Value) {
    let response = app
        .oneshot(
            Request::post("/presentations")
                .header("content-type", content_type)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    assert_eq!(response.headers()["x-frame-options"], "DENY");
    assert_eq!(
        response.headers()["content-security-policy"],
        "default-src 'none'"
    );
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn multipart(file: Option<(&str, &[u8])>, fields: &[(&str, &str)]) -> (String, Vec<u8>) {
    let boundary = "XBOUNDARYX";
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }
    if let Some((name, bytes)) = file {
        body.extend_from_slice(
            format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: application/octet-stream\r\n\r\n").as_bytes(),
        );
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

fn creds() -> [(&'static str, &'static str); 2] {
    [("livekitToken", GOOD.as_str()), ("livekitUrl", OWN_LIVEKIT)]
}

#[tokio::test]
async fn the_route_answers_upstreams_errors() {
    let launcher = FakeLauncher::new(json!({}));
    let app = || super::router(Some(Presenter::new(launcher.clone(), target(), 10, None)));
    let err = |m: &str| json!({ "error": m });

    let (status, body) = post(super::router(None), "application/json", b"{}".to_vec()).await;
    assert_eq!(
        (status, body),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            err("Presentation service unavailable")
        )
    );
    assert_eq!(
        post(app(), "text/plain", b"x".to_vec()).await,
        (
            StatusCode::BAD_REQUEST,
            err("Content-Type must be multipart/form-data or application/json")
        )
    );
    assert_eq!(
        post(app(), "application/json", b"{".to_vec()).await,
        (
            StatusCode::BAD_REQUEST,
            err("Request body must be valid JSON")
        )
    );
    assert_eq!(
        post(app(), "application/json", b"[1]".to_vec()).await,
        (
            StatusCode::BAD_REQUEST,
            err("Request body must be a JSON object")
        )
    );
    assert_eq!(
        post(
            app(),
            "application/json",
            json!({"livekitToken": "t"}).to_string().into_bytes()
        )
        .await,
        (StatusCode::BAD_REQUEST, err("Missing url"))
    );
    assert_eq!(
        post(
            app(),
            "application/json",
            json!({"url": "https://x"}).to_string().into_bytes()
        )
        .await,
        (
            StatusCode::BAD_REQUEST,
            err("Missing livekitToken or livekitUrl")
        )
    );
    let body = |token: &str, lk: &str, url: &str| {
        json!({"url": url, "livekitToken": token, "livekitUrl": lk})
            .to_string()
            .into_bytes()
    };
    assert_eq!(
        post(
            app(),
            "application/json",
            body("t", "https://lk", "https://x")
        )
        .await,
        (
            StatusCode::BAD_REQUEST,
            err("livekitUrl must use wss:// protocol")
        )
    );
    assert_eq!(
        post(
            app(),
            "application/json",
            body("bad", OWN_LIVEKIT, "https://x")
        )
        .await,
        (StatusCode::UNAUTHORIZED, err("Invalid LiveKit credentials"))
    );
    // A presentation-bot token LiveKit refuses (revoked, say) answers the same.
    assert_eq!(
        post(
            app(),
            "application/json",
            body(&bot_token("access:refused"), OWN_LIVEKIT, "https://x")
        )
        .await,
        (StatusCode::UNAUTHORIZED, err("Invalid LiveKit credentials"))
    );
    // Another LiveKit: refused before any bot starts.
    let dialed = launcher.dialed.lock().unwrap().len();
    assert_eq!(
        post(
            app(),
            "application/json",
            body(&GOOD, "wss://attacker.example", "https://x")
        )
        .await,
        (
            StatusCode::BAD_REQUEST,
            err("livekitUrl is not this node's LiveKit server")
        )
    );
    assert_eq!(launcher.dialed.lock().unwrap().len(), dialed);
    // The bot or LiveKit unavailable is not a refused token.
    assert_eq!(
        post(
            app(),
            "application/json",
            body(&bot_token("access:unavailable"), OWN_LIVEKIT, "https://x")
        )
        .await,
        (
            StatusCode::SERVICE_UNAVAILABLE,
            err("Presentation service unavailable")
        )
    );
    // The bot dials the node's route, never the request's URL.
    assert!(
        launcher
            .dialed
            .lock()
            .unwrap()
            .iter()
            .all(|url| url == "ws://livekit.internal:7880")
    );
    assert_eq!(
        post(
            app(),
            "application/json",
            body(&GOOD, OWN_LIVEKIT, "http://x/a.pdf")
        )
        .await,
        (
            StatusCode::BAD_REQUEST,
            err("URL must use HTTPS, got http: for http://x/a.pdf")
        )
    );
    assert_eq!(
        post(
            app(),
            "application/json",
            body(&GOOD, OWN_LIVEKIT, "https://10.0.0.1/a.pdf")
        )
        .await,
        (
            StatusCode::BAD_REQUEST,
            err("URL resolves to private IP: 10.0.0.1")
        )
    );
    let big = vec![b' '; super::files::MAX_JSON_BODY_SIZE + 1];
    assert_eq!(
        post(app(), "application/json", big).await,
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            err("Request exceeds maximum size of 1MB")
        )
    );

    let (ct, mp) = multipart(None, &creds());
    assert_eq!(
        post(app(), &ct, mp).await,
        (
            StatusCode::BAD_REQUEST,
            err("Missing file in multipart upload")
        )
    );
    let (ct, mp) = multipart(Some(("a.pdf", b"%PDF")), &creds()[..1]);
    assert_eq!(
        post(app(), &ct, mp).await,
        (
            StatusCode::BAD_REQUEST,
            err("Missing livekitToken or livekitUrl")
        )
    );
    let (ct, mp) = multipart(Some(("a.key", b"%PDF")), &creds());
    assert_eq!(
        post(app(), &ct, mp).await,
        (
            StatusCode::BAD_REQUEST,
            err("Unsupported file type. Only .pdf and .pptx files are supported.")
        )
    );
    let (ct, mp) = multipart(Some(("a.pdf", b"PK\x03\x04")), &creds());
    assert_eq!(
        post(app(), &ct, mp).await,
        (
            StatusCode::BAD_REQUEST,
            err("File content does not match expected PDF format")
        )
    );
    let (ct, mp) = multipart(Some(("a.pptx", b"PK\x03\x04")), &creds());
    assert_eq!(
        post(app(), &ct, mp).await,
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            err("Unable to render the presentation")
        )
    );
    // A token LiveKit refuses on the multipart path is upstream's generic 500.
    let refused = bot_token("access:refused");
    let (ct, mp) = multipart(
        Some(("a.pdf", &test_pdf(1))),
        &[("livekitToken", &refused), ("livekitUrl", OWN_LIVEKIT)],
    );
    assert_eq!(
        post(app(), &ct, mp).await,
        (StatusCode::INTERNAL_SERVER_ERROR, err("Internal error"))
    );
    // A body over 100 MiB: 413 by its declared length, or as it streams in.
    let (ct, mut mp) = multipart(Some(("a.pdf", b"%PDF")), &creds());
    mp.resize(super::files::MAX_FILE_SIZE + 1, b' ');
    let declared = app()
        .oneshot(
            Request::post("/presentations")
                .header("content-type", &ct)
                .header("content-length", mp.len())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(declared.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(
        post(app(), &ct, mp).await,
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            err("Request exceeds maximum size of 100MB")
        )
    );
    // An unreadable PDF too.
    let (ct, mp) = multipart(Some(("a.pdf", b"%PDF-not really")), &creds());
    assert_eq!(
        post(app(), &ct, mp).await,
        (StatusCode::INTERNAL_SERVER_ERROR, err("Internal error"))
    );
}

#[tokio::test]
async fn a_multipart_upload_creates_a_presentation() {
    let launcher = FakeLauncher::new(json!({}));
    let presenter = Presenter::new(launcher.clone(), target(), 1, None);
    let (ct, mp) = multipart(Some(("My Deck.pdf", &test_pdf(2))), &creds());
    let (status, body) = post(super::router(Some(presenter.clone())), &ct, mp).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["fileName"], "My_Deck");
    assert_eq!(body["slideCount"], 2);
    assert_eq!(body["currentSlide"], 0);
    assert_eq!(body["fileType"], "pdf");
    let (ct, mp) = multipart(Some(("b.pdf", &test_pdf(1))), &creds());
    assert_eq!(
        post(super::router(Some(presenter)), &ct, mp).await,
        (
            StatusCode::TOO_MANY_REQUESTS,
            json!({"error": "Maximum concurrent presentations reached (1)"})
        )
    );
}

#[tokio::test]
async fn a_dropped_request_still_finishes_its_presentation() {
    let launcher = FakeLauncher::slow(
        json!({"presenters": [PRESENTER.to_lowercase()]}),
        Duration::from_millis(100),
    );
    let presenter = Presenter::new(launcher.clone(), target(), 1, None);
    let create = presenter.create(test_pdf(1), &GOOD, "a.pdf", None);
    assert!(
        tokio::time::timeout(Duration::from_millis(10), create)
            .await
            .is_err()
    );
    // The session runs: its commands are handled and it stops.
    settle_until(|| presenter.active_sessions() == 1 && !launcher.bots.lock().unwrap().is_empty())
        .await;
    assert_eq!(presenter.active_sessions(), 1);
    let (bot, events) = launcher.last();
    events
        .data(
            PRESENTER,
            command_packet(json!({"type": "presentation:stop"}), ""),
        )
        .await;
    settle_until(|| {
        presenter.active_sessions() == 0 && bot.sent.lock().unwrap().last() == Some(&Sent::Shutdown)
    })
    .await;
    assert_eq!(presenter.active_sessions(), 0);
    assert_eq!(bot.sent.lock().unwrap().last(), Some(&Sent::Shutdown));
    // Its slot is free again.
    presenter
        .create(test_pdf(1), &GOOD, "b.pdf", None)
        .await
        .unwrap();
}

#[tokio::test]
async fn the_json_path_holds_one_slot_from_the_check_to_the_session() {
    let launcher = FakeLauncher::new(json!({}));
    let presenter = Presenter::new(launcher.clone(), target(), 1, None);
    let slot = presenter.reserve(&GOOD).await.unwrap();
    assert!(matches!(
        presenter.reserve(&GOOD).await,
        Err(super::session::CreateError::MaxConcurrent(1))
    ));
    assert!(matches!(
        presenter.create(test_pdf(1), &GOOD, "a.pdf", None).await,
        Err(super::session::CreateError::MaxConcurrent(1))
    ));
    presenter.validate_credentials(&slot, &GOOD).await.unwrap();
    assert!(matches!(
        presenter
            .validate_credentials(&slot, &bot_token("access:refused"))
            .await,
        Err(super::session::CreateError::InvalidCredentials(_))
    ));
    // Refused here, before a bot starts.
    let dialed = launcher.dialed.lock().unwrap().len();
    assert!(matches!(
        presenter.validate_credentials(&slot, &guest_token()).await,
        Err(super::session::CreateError::InvalidCredentials(_))
    ));
    assert_eq!(launcher.dialed.lock().unwrap().len(), dialed);
    presenter
        .create_reserved(slot, test_pdf(1), &GOOD, "a.pdf", None)
        .await
        .unwrap();
    assert_eq!(presenter.active_sessions(), 1);
    assert!(presenter.reserve(&GOOD).await.is_err());
}

#[test]
fn only_presentation_bot_tokens_of_this_livekit_pass() {
    let own = target();
    let now = unix_now();
    let check = |token: &str| own.check_bot_token(token, now).map(drop);
    assert_eq!(check(&GOOD), Ok(()));
    // The gatekeeper's untagged form, and LiveKit's defaults for absent flags.
    let untagged = token_with(
        API_SECRET,
        json!({"sub": "presentation-bot:scene:room:x", "video": {"roomJoin": true, "room": "scene:room"}}),
    );
    assert_eq!(check(&untagged), Ok(()));
    let refused = "not a valid token of this node's LiveKit";
    assert_eq!(check("bad"), Err(refused));
    let other_key = token_with(
        "another-secret",
        json!({"sub": "presentation-bot:scene:room:x", "video": bot_grant("scene:room")}),
    );
    assert_eq!(check(&other_key), Err(refused));
    let other_issuer = token_with(
        API_SECRET,
        json!({"iss": "APIother", "sub": "presentation-bot:scene:room:x", "video": bot_grant("scene:room")}),
    );
    assert_eq!(check(&other_issuer), Err(refused));
    let expired = token_with(
        API_SECRET,
        json!({"sub": "presentation-bot:scene:room:x", "video": bot_grant("scene:room"),
               "nbf": now - 900, "exp": now - 120}),
    );
    assert_eq!(check(&expired), Err(refused));
    let not_yet = token_with(
        API_SECRET,
        json!({"sub": "presentation-bot:scene:room:x", "video": bot_grant("scene:room"),
               "nbf": now + 120}),
    );
    assert_eq!(check(&not_yet), Err(refused));
    // A player's scene token, and bot identities of another room.
    let identity = "not a presentation-bot identity of the granted room";
    assert_eq!(check(&guest_token()), Err(identity));
    for sub in [
        "presentation-bot:scene:other:x",
        "presentation-bot:scene:room",
        "presentation-bot:scene:roomy:x",
        "stream:place:x:y",
    ] {
        let token = token_with(
            API_SECRET,
            json!({"sub": sub, "video": bot_grant("scene:room")}),
        );
        assert_eq!(check(&token), Err(identity), "{sub}");
    }
    let no_room = token_with(
        API_SECRET,
        json!({"sub": "presentation-bot:scene:room:x", "video": {"roomJoin": true}}),
    );
    assert_eq!(check(&no_room), Err("no room in the grant"));
    let mut no_join = bot_grant("scene:room");
    no_join["roomJoin"] = json!(false);
    let no_join = token_with(
        API_SECRET,
        json!({"sub": "presentation-bot:scene:room:x", "video": no_join}),
    );
    assert_eq!(check(&no_join), Err("the grant does not join the room"));
    let cannot_publish = "the grant cannot publish a screen share";
    let mut no_publish = bot_grant("scene:room");
    no_publish["canPublish"] = json!(false);
    let mut microphone = bot_grant("scene:room");
    microphone["canPublishSources"] = json!(["microphone"]);
    let mut screen_share = bot_grant("scene:room");
    screen_share["canPublishSources"] = json!(["microphone", "screen_share"]);
    let as_bot = |grant: Value| {
        token_with(
            API_SECRET,
            json!({"sub": "presentation-bot:scene:room:x", "video": grant}),
        )
    };
    assert_eq!(check(&as_bot(no_publish)), Err(cannot_publish));
    assert_eq!(check(&as_bot(microphone)), Err(cannot_publish));
    assert_eq!(check(&as_bot(screen_share)), Ok(()));
}

#[tokio::test]
async fn a_token_that_is_not_a_bot_token_starts_nothing() {
    let launcher = FakeLauncher::new(json!({}));
    let presenter = Presenter::new(launcher.clone(), target(), 1, None);
    let app = || super::router(Some(presenter.clone()));
    let invalid = (
        StatusCode::UNAUTHORIZED,
        json!({"error": "Invalid LiveKit credentials"}),
    );
    let other_key = token_with(
        "another-secret",
        json!({"sub": "presentation-bot:scene:room:x", "video": bot_grant("scene:room")}),
    );
    let expired = token_with(
        API_SECRET,
        json!({"sub": "presentation-bot:scene:room:x", "video": bot_grant("scene:room"),
               "nbf": unix_now() - 900, "exp": unix_now() - 120}),
    );
    for token in [guest_token(), other_key, expired] {
        let body = json!({"url": "https://example.org/a.pdf", "livekitToken": token, "livekitUrl": OWN_LIVEKIT});
        assert_eq!(
            post(app(), "application/json", body.to_string().into_bytes()).await,
            invalid
        );
        let (ct, mp) = multipart(
            Some(("a.pdf", &test_pdf(1))),
            &[("livekitToken", &token), ("livekitUrl", OWN_LIVEKIT)],
        );
        assert_eq!(post(app(), &ct, mp).await, invalid);
    }
    // No bot started and no slot held.
    assert!(launcher.dialed.lock().unwrap().is_empty());
    let (ct, mp) = multipart(Some(("a.pdf", &test_pdf(1))), &creds());
    assert_eq!(post(app(), &ct, mp).await.0, StatusCode::CREATED);
    assert_eq!(launcher.dialed.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn uploads_are_read_under_a_permit() {
    let launcher = FakeLauncher::new(json!({}));
    let presenter = Presenter::new(launcher.clone(), target(), 1, None);
    // An upload being read takes the only permit: the next is answered 429
    // before its body is read.
    let reading = presenter.upload_permit().unwrap();
    let (ct, mp) = multipart(Some(("a.pdf", &test_pdf(1))), &creds());
    assert_eq!(
        post(super::router(Some(presenter.clone())), &ct, mp.clone()).await,
        (
            StatusCode::TOO_MANY_REQUESTS,
            json!({"error": "Maximum concurrent presentations reached (1)"})
        )
    );
    assert!(launcher.dialed.lock().unwrap().is_empty());
    drop(reading);
    // Every answer gives the permit back, an error included.
    let (bad_ct, bad) = multipart(Some(("a.pdf", b"PK\x03\x04")), &creds());
    assert_eq!(
        post(super::router(Some(presenter.clone())), &bad_ct, bad)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        post(super::router(Some(presenter.clone())), &ct, mp)
            .await
            .0,
        StatusCode::CREATED
    );
    assert!(presenter.upload_permit().is_ok());
    // Unlimited slots, unlimited uploads.
    let unlimited = Presenter::new(launcher, target(), 0, None);
    assert!(unlimited.upload_permit().unwrap().is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn timed_out_renders_keep_their_thread_and_no_more_start() {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    let pool = Arc::new(super::session::RenderPool::new(
        2,
        Duration::from_millis(100),
        Duration::from_millis(100),
    ));
    let release = Arc::new(AtomicBool::new(false));
    let (running, started, finished) = (
        Arc::new(AtomicUsize::new(0)),
        Arc::new(AtomicUsize::new(0)),
        Arc::new(AtomicUsize::new(0)),
    );
    let stuck = || {
        let (release, running, started, finished) = (
            release.clone(),
            running.clone(),
            started.clone(),
            finished.clone(),
        );
        move || {
            started.fetch_add(1, Ordering::SeqCst);
            running.fetch_add(1, Ordering::SeqCst);
            while !release.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
            running.fetch_sub(1, Ordering::SeqCst);
            finished.fetch_add(1, Ordering::SeqCst);
        }
    };
    // Six renders that never end on their own, three rounds of two: the
    // first two time out and keep their threads, the rest find none free.
    for round in 0..3 {
        let runs: Vec<_> = (0..2)
            .map(|_| {
                let pool = pool.clone();
                let work = stuck();
                tokio::spawn(async move { pool.run(work).await })
            })
            .collect();
        for run in runs {
            let outcome = run.await.unwrap();
            if round == 0 {
                assert!(matches!(
                    outcome,
                    Err(super::session::RenderError::TimedOut)
                ));
            } else {
                assert!(matches!(outcome, Err(super::session::RenderError::Busy)));
            }
        }
    }
    // Only the first two ever ran: the rest were busy waiting for a thread.
    assert_eq!(started.load(Ordering::SeqCst), 2);
    assert_eq!(running.load(Ordering::SeqCst), 2);
    // Once they return, their threads serve again.
    release.store(true, Ordering::SeqCst);
    while finished.load(Ordering::SeqCst) < 2 {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(matches!(pool.run(|| 7).await, Ok(7)));
}

#[tokio::test]
async fn a_full_render_pool_drops_slide_changes_and_creations_but_stops_nothing() {
    let (presenter, launcher, _) = presenting(3).await;
    let (bot, events) = launcher.last();
    let next = json!({"type": "presentation:navigate", "action": "next"});
    // Stuck renders elsewhere hold every thread; the waits below run on
    // paused time.
    let threads = presenter.hold_render_threads().await;
    tokio::time::pause();
    let frames = |bot: &FakeBot| {
        bot.sent
            .lock()
            .unwrap()
            .iter()
            .filter(|s| matches!(s, Sent::Frame(..)))
            .count()
    };
    events
        .data(PRESENTER, command_packet(next.clone(), "scene-1"))
        .await;
    tokio::time::sleep(super::session::RENDER_WAIT * 2).await;
    settle_until(|| frames(&bot) >= 2 && data_messages(&bot).len() >= 2).await;
    // The slide change is dropped: the session runs on slide 0, re-pushes
    // its frame and broadcasts that state.
    assert_eq!(presenter.active_sessions(), 1);
    assert!(!bot.sent.lock().unwrap().contains(&Sent::Shutdown));
    assert_eq!(frames(&bot), 2);
    let messages = data_messages(&bot);
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1].0["currentSlide"], 0);
    // A creation answers busy, not a load failure, and gives back its slot.
    let other = room_token("other", "access:1");
    assert!(matches!(
        presenter
            .create(test_pdf(1), &other, "Other.pdf", None)
            .await,
        Err(super::session::CreateError::Busy)
    ));
    assert_eq!(presenter.active_sessions(), 1);
    let (other, _) = launcher.last();
    assert_eq!(other.sent.lock().unwrap().last(), Some(&Sent::Shutdown));
    // With threads free again, the next slide change renders.
    drop(threads);
    tokio::time::resume();
    events
        .data(PRESENTER, command_packet(next, "scene-1"))
        .await;
    settle_until(|| frames(&bot) >= 3).await;
    assert_eq!(data_messages(&bot).last().unwrap().0["currentSlide"], 1);
    assert_eq!(frames(&bot), 3);
    assert_eq!(presenter.active_sessions(), 1);
}

// ── downloads ──

/// A local server: `/redirect` to a private host, `/big` one byte over 64,
/// `/a.pdf` a file.
async fn file_server() -> std::net::SocketAddr {
    use axum::response::IntoResponse;
    let app = axum::Router::new()
        .route(
            "/redirect",
            axum::routing::get(|| async {
                (StatusCode::FOUND, [("location", "http://10.0.0.1/a.pdf")]).into_response()
            }),
        )
        .route("/big", axum::routing::get(|| async { vec![b'x'; 65] }))
        .route(
            "/a.pdf",
            axum::routing::get(|| async { b"%PDF-1.4".to_vec() }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    crate::shared::task::spawn_with(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

#[tokio::test]
async fn downloads_stop_at_private_redirects_and_the_size_limit() {
    let addr = file_server().await;
    // The test server's address only; every other private address stays refused.
    let guard = ssrf::Guard {
        https_only: false,
        allowed: Some(addr.ip()),
        max_bytes: 64,
    };
    let fetched = ssrf::download_guarded(&format!("http://{addr}/a.pdf"), &guard)
        .await
        .unwrap();
    assert_eq!(
        (fetched.bytes.as_slice(), fetched.filename.as_str()),
        (&b"%PDF-1.4"[..], "a.pdf")
    );
    assert_eq!(
        ssrf::download_guarded(&format!("http://{addr}/redirect"), &guard)
            .await
            .unwrap_err(),
        ssrf::DownloadError::InvalidUrl("URL resolves to private IP: 10.0.0.1".into())
    );
    assert_eq!(
        ssrf::download_guarded(&format!("http://{addr}/big"), &guard)
            .await
            .unwrap_err(),
        ssrf::DownloadError::TooLarge
    );
}

// ── against a real LiveKit ──

/// A token the way the gatekeeper's presentation-bot token is shaped.
fn signed_token(key: &str, secret: &str, identity: &str, room: &str) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let claims = json!({
        "iss": key, "sub": identity, "nbf": now, "exp": now + 600,
        "video": {"roomJoin": true, "room": room, "canPublish": true, "canSubscribe": true,
                  "canPublishData": true, "canUpdateOwnMetadata": true},
    });
    crate::shared::livekit::sign_hs256(
        secret,
        br#"{"alg":"HS256","typ":"JWT"}"#,
        claims.to_string().as_bytes(),
    )
    .unwrap()
}

/// The node's presenter with the real `cast-bot` (`CAST_BOT_BIN`) and a
/// `livekit-server` (`LIVEKIT_SERVER_BIN`) on free loopback ports: the stdio
/// protocol end to end. Ignored by default (it needs both binaries);
/// `.github/workflows/cast-presenter.yml` runs it with `--ignored`. Without
/// both it says so and passes, except under CI, where it fails.
/// `services/catalyst/cast-bot/tests` covers the data channel and the track
/// as a viewer sees them.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires CAST_BOT_BIN and LIVEKIT_SERVER_BIN; run by .github/workflows/cast-presenter.yml"]
async fn a_presentation_goes_live_on_a_real_livekit() {
    let (Ok(bot_bin), Ok(server_bin)) = (
        std::env::var("CAST_BOT_BIN"),
        std::env::var("LIVEKIT_SERVER_BIN"),
    ) else {
        assert!(
            std::env::var("CI").as_deref() != Ok("true"),
            "CAST_BOT_BIN and LIVEKIT_SERVER_BIN are required in CI"
        );
        eprintln!("SKIPPED: set CAST_BOT_BIN and LIVEKIT_SERVER_BIN to run this test");
        return;
    };
    let free = || {
        std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    };
    let (port, tcp, udp) = (free(), free(), free());
    let (key, secret) = (
        "presenterkey",
        "presentersecret-presentersecret-presentersecret",
    );
    let mut server = tokio::process::Command::new(server_bin)
        .arg("--config-body")
        .arg(format!(
            "port: {port}\nbind_addresses: [127.0.0.1]\nrtc:\n  tcp_port: {tcp}\n  udp_port: {udp}\n  use_external_ip: false\n  node_ip: 127.0.0.1\nkeys:\n  {key}: {secret}\nlogging:\n  level: warn\n"
        ))
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    for _ in 0..100 {
        if tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let url = format!("ws://127.0.0.1:{port}");
    let presenter = Presenter::new(
        Arc::new(super::bot::ProcessLauncher {
            binary: bot_bin.clone().into(),
        }),
        LiveKitTarget {
            client_url: url.clone(),
            connect_url: url.clone(),
            api_key: key.into(),
            api_secret: secret.into(),
        },
        10,
        None,
    );
    // A refused token: 401 on the JSON path, before any download.
    let app = super::router(Some(presenter.clone()));
    let refused =
        json!({"url": "https://example.org/a.pdf", "livekitToken": "nope", "livekitUrl": url});
    assert_eq!(
        post(app, "application/json", refused.to_string().into_bytes()).await,
        (
            StatusCode::UNAUTHORIZED,
            json!({"error": "Invalid LiveKit credentials"})
        )
    );
    let token = signed_token(key, secret, "presentation-bot:room:access:1", "room");
    let slot = presenter.reserve(&token).await.unwrap();
    presenter.validate_credentials(&slot, &token).await.unwrap();
    let info = presenter
        .create_reserved(slot, test_pdf(2), &token, "Live.pdf", None)
        .await
        .unwrap();
    assert_eq!((info.file_name.as_str(), info.slide_count), ("Live", 2));
    assert_eq!(presenter.active_sessions(), 1);
    presenter.stop(&info.id).await;
    assert_eq!(presenter.active_sessions(), 0);

    // A new presentation of the room replaces the running one: its bot
    // leaves and the new one is the room's only bot. A token whose bot is
    // there is refused. Then that bot is killed: the node removes it from the
    // room at once (LiveKit alone keeps it, and its last slide, for about
    // 20 s), and a presentation of another room goes on.
    let own = presenter.livekit().clone();
    let rooms = TestRooms::new(own);
    presenter.set_room_service(rooms.clone());
    let kept_bot = "presentation-bot:other:access:2";
    let kept = presenter
        .create(
            test_pdf(1),
            &signed_token(key, secret, kept_bot, "other"),
            "Kept.pdf",
            None,
        )
        .await
        .unwrap();
    let replaced_bot = "presentation-bot:room:access:3";
    let last_bot = "presentation-bot:room:access:4";
    let replaced = signed_token(key, secret, replaced_bot, "room");
    let last = signed_token(key, secret, last_bot, "room");
    presenter
        .create(test_pdf(1), &replaced, "Replaced.pdf", None)
        .await
        .unwrap();
    let service = || super::bot::Rooms::room_service(&*rooms);
    let in_room = |room: &'static str| async move {
        let mut bots = service().list_participant_identities(room).await;
        bots.sort();
        bots
    };
    assert_eq!(in_room("room").await, [replaced_bot]);
    presenter
        .create(test_pdf(1), &last, "Killed.pdf", None)
        .await
        .unwrap();
    for _ in 0..50 {
        if in_room("room").await == [last_bot] {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(
        in_room("room").await,
        [last_bot],
        "the replaced bot is in the room"
    );
    assert_eq!(presenter.active_sessions(), 2);
    let twice = presenter.create(test_pdf(1), &last, "Twice.pdf", None);
    let used = "This token already has a presentation";
    assert_eq!(twice.await.unwrap_err().to_string(), used);
    service().send_data("room", b"from the node").await.unwrap();
    let name = std::path::Path::new(&bot_bin).file_name().unwrap();
    let killed = std::process::Command::new("pkill")
        .args(["-KILL", "-n", "-P", &std::process::id().to_string(), "-x"])
        .arg(name)
        .status()
        .unwrap();
    assert!(killed.success());
    for _ in 0..50 {
        if in_room("room").await.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        in_room("room").await.is_empty(),
        "the killed bot is in the room"
    );
    assert_eq!(in_room("other").await, [kept_bot]);
    assert_eq!(presenter.active_sessions(), 1);
    presenter.stop(&kept.id).await;

    // LiveKit goes away: the bot reports it and exits, and the session ends.
    let info = presenter
        .create(test_pdf(1), &token, "Again.pdf", None)
        .await
        .unwrap();
    assert_eq!(presenter.active_sessions(), 1);
    server.kill().await.unwrap();
    for _ in 0..600 {
        if presenter.active_sessions() == 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(presenter.active_sessions(), 0, "{} still live", info.id);
}
