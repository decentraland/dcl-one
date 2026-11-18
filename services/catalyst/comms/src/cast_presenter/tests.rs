//! The presenter's wire contract, checks and session behavior, against a
//! fake bot. Upstream references are `decentraland/cast-presenter-server`
//! @ cae78612.

use std::sync::{Arc, Mutex};
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

// ── fake bot ──

#[derive(Debug, Clone, PartialEq)]
enum Sent {
    PublishVideo(u32, u32),
    Frame(u32, u32),
    Data(Vec<u8>),
    Metadata(String),
    Shutdown,
}

#[derive(Default)]
struct FakeBot {
    sent: Mutex<Vec<Sent>>,
}

#[async_trait::async_trait]
impl BotLink for FakeBot {
    async fn publish_video(&self, w: u32, h: u32) -> anyhow::Result<()> {
        self.sent.lock().unwrap().push(Sent::PublishVideo(w, h));
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
        self.sent.lock().unwrap().push(Sent::Data(packet));
        Ok(())
    }
    async fn update_metadata(&self, metadata: String) -> anyhow::Result<()> {
        self.sent.lock().unwrap().push(Sent::Metadata(metadata));
        Ok(())
    }
    async fn shutdown(&self) {
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

struct FakeLauncher {
    accept_token: &'static str,
    room_metadata: String,
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
            accept_token: "good-token",
            room_metadata: room_metadata.to_string(),
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
        self.bots.lock().unwrap().last_mut().expect("a bot").1 = None;
    }
}

#[async_trait::async_trait]
impl BotLauncher for FakeLauncher {
    async fn connect(&self, url: &str, token: &str) -> Result<Connected, ConnectError> {
        self.dialed.lock().unwrap().push(url.to_string());
        tokio::time::sleep(self.delay).await;
        match token {
            "unavailable" => return Err(ConnectError::Unavailable("no bot".into())),
            token if token != self.accept_token => {
                return Err(ConnectError::Refused("unauthorized".into()));
            }
            _ => {}
        }
        let bot = Arc::new(FakeBot::default());
        let (data_tx, data) = mpsc::channel(16);
        let (state_tx, room) = watch::channel(RoomState {
            metadata: self.room_metadata.clone(),
            participants: 1,
        });
        let fake_room = FakeRoom {
            data: data_tx,
            state: Arc::new(state_tx),
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

fn target() -> LiveKitTarget {
    LiveKitTarget {
        client_url: OWN_LIVEKIT.into(),
        connect_url: "ws://livekit.internal:7880".into(),
    }
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
        Some(Command::Unsupported("presentation:video:play".into()))
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
    assert_eq!(
        LiveKitTarget::from_endpoints("wss://lk.example.org", "http://livekit:7880"),
        Some(LiveKitTarget {
            client_url: "wss://lk.example.org".into(),
            connect_url: "ws://livekit:7880".into(),
        })
    );
    assert_eq!(
        LiveKitTarget::from_endpoints("", "http://livekit:7880"),
        None
    );
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

async fn presenting(pages: usize) -> (Arc<Presenter>, Arc<FakeLauncher>, PresentationInfo) {
    let launcher = FakeLauncher::new(json!({"presenters": [PRESENTER.to_lowercase()]}));
    let presenter = Presenter::new(launcher.clone(), target(), 10);
    let info = presenter
        .create(test_pdf(pages), "good-token", "Deck.pdf")
        .await
        .unwrap();
    (presenter, launcher, info)
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
    settle().await;
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
    settle().await;
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
    settle().await;
    assert_eq!(data_messages(&bot).last().unwrap().0["currentSlide"], 0);
    drop(presenter);
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
    tokio::time::sleep(Duration::from_millis(300)).await;
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
    settle().await;
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
    settle().await;
    assert_eq!(presenter.active_sessions(), 0);
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

#[tokio::test]
async fn the_concurrency_limit_answers_before_connecting() {
    let launcher = FakeLauncher::new(json!({}));
    let presenter = Presenter::new(launcher.clone(), target(), 1);
    presenter
        .create(test_pdf(1), "good-token", "a.pdf")
        .await
        .unwrap();
    let err = presenter
        .create(test_pdf(1), "good-token", "b.pdf")
        .await
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Maximum concurrent presentations reached (1)"
    );
    assert_eq!(launcher.bots.lock().unwrap().len(), 1);
    // 0 is unlimited.
    let unlimited = Presenter::new(launcher.clone(), target(), 0);
    for _ in 0..3 {
        unlimited
            .create(test_pdf(1), "good-token", "c.pdf")
            .await
            .unwrap();
    }
}

// ── the route ──

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

const CREDS: [(&str, &str); 2] = [
    ("livekitToken", "good-token"),
    ("livekitUrl", "wss://lk.example.org"),
];

#[tokio::test]
async fn the_route_answers_upstreams_errors() {
    let launcher = FakeLauncher::new(json!({}));
    let app = || super::router(Some(Presenter::new(launcher.clone(), target(), 10)));
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
    // Another LiveKit: refused before any bot starts.
    let dialed = launcher.dialed.lock().unwrap().len();
    assert_eq!(
        post(
            app(),
            "application/json",
            body("good-token", "wss://attacker.example", "https://x")
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
            body("unavailable", OWN_LIVEKIT, "https://x")
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
            body("good-token", OWN_LIVEKIT, "http://x/a.pdf")
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
            body("good-token", OWN_LIVEKIT, "https://10.0.0.1/a.pdf")
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

    let (ct, mp) = multipart(None, &CREDS);
    assert_eq!(
        post(app(), &ct, mp).await,
        (
            StatusCode::BAD_REQUEST,
            err("Missing file in multipart upload")
        )
    );
    let (ct, mp) = multipart(Some(("a.pdf", b"%PDF")), &CREDS[..1]);
    assert_eq!(
        post(app(), &ct, mp).await,
        (
            StatusCode::BAD_REQUEST,
            err("Missing livekitToken or livekitUrl")
        )
    );
    let (ct, mp) = multipart(Some(("a.key", b"%PDF")), &CREDS);
    assert_eq!(
        post(app(), &ct, mp).await,
        (
            StatusCode::BAD_REQUEST,
            err("Unsupported file type. Only .pdf and .pptx files are supported.")
        )
    );
    let (ct, mp) = multipart(Some(("a.pdf", b"PK\x03\x04")), &CREDS);
    assert_eq!(
        post(app(), &ct, mp).await,
        (
            StatusCode::BAD_REQUEST,
            err("File content does not match expected PDF format")
        )
    );
    let (ct, mp) = multipart(Some(("a.pptx", b"PK\x03\x04")), &CREDS);
    assert_eq!(
        post(app(), &ct, mp).await,
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            err("Unable to render the presentation")
        )
    );
    // A refused token on the multipart path is upstream's generic 500.
    let (ct, mp) = multipart(
        Some(("a.pdf", &test_pdf(1))),
        &[("livekitToken", "bad"), ("livekitUrl", OWN_LIVEKIT)],
    );
    assert_eq!(
        post(app(), &ct, mp).await,
        (StatusCode::INTERNAL_SERVER_ERROR, err("Internal error"))
    );
    // A body over 100 MiB: 413 by its declared length, or as it streams in.
    let (ct, mut mp) = multipart(Some(("a.pdf", b"%PDF")), &CREDS);
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
    let (ct, mp) = multipart(Some(("a.pdf", b"%PDF-not really")), &CREDS);
    assert_eq!(
        post(app(), &ct, mp).await,
        (StatusCode::INTERNAL_SERVER_ERROR, err("Internal error"))
    );
}

#[tokio::test]
async fn a_multipart_upload_creates_a_presentation() {
    let launcher = FakeLauncher::new(json!({}));
    let presenter = Presenter::new(launcher.clone(), target(), 1);
    let (ct, mp) = multipart(Some(("My Deck.pdf", &test_pdf(2))), &CREDS);
    let (status, body) = post(super::router(Some(presenter.clone())), &ct, mp).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["fileName"], "My_Deck");
    assert_eq!(body["slideCount"], 2);
    assert_eq!(body["currentSlide"], 0);
    assert_eq!(body["fileType"], "pdf");
    let (ct, mp) = multipart(Some(("b.pdf", &test_pdf(1))), &CREDS);
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
    let presenter = Presenter::new(launcher.clone(), target(), 1);
    let create = presenter.create(test_pdf(1), "good-token", "a.pdf");
    assert!(
        tokio::time::timeout(Duration::from_millis(10), create)
            .await
            .is_err()
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    // The session runs: its commands are handled and it stops.
    assert_eq!(presenter.active_sessions(), 1);
    let (bot, events) = launcher.last();
    events
        .data(
            PRESENTER,
            command_packet(json!({"type": "presentation:stop"}), ""),
        )
        .await;
    settle().await;
    assert_eq!(presenter.active_sessions(), 0);
    assert_eq!(bot.sent.lock().unwrap().last(), Some(&Sent::Shutdown));
    // Its slot is free again.
    presenter
        .create(test_pdf(1), "good-token", "b.pdf")
        .await
        .unwrap();
}

#[tokio::test]
async fn the_credential_check_holds_a_slot() {
    let launcher = FakeLauncher::slow(json!({}), Duration::from_millis(100));
    let presenter = Presenter::new(launcher.clone(), target(), 1);
    let checking = crate::shared::task::spawn_with({
        let presenter = presenter.clone();
        async move { presenter.validate_credentials("good-token").await }
    });
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(matches!(
        presenter.validate_credentials("good-token").await,
        Err(super::session::CreateError::MaxConcurrent(1))
    ));
    checking.await.unwrap().unwrap();
    presenter.validate_credentials("good-token").await.unwrap();
    assert!(matches!(
        presenter.validate_credentials("bad").await,
        Err(super::session::CreateError::InvalidCredentials(_))
    ));
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
            binary: bot_bin.into(),
        }),
        LiveKitTarget {
            client_url: url.clone(),
            connect_url: url.clone(),
        },
        10,
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
    presenter.validate_credentials(&token).await.unwrap();
    let info = presenter
        .create(test_pdf(2), &token, "Live.pdf")
        .await
        .unwrap();
    assert_eq!((info.file_name.as_str(), info.slide_count), ("Live", 2));
    assert_eq!(presenter.active_sessions(), 1);
    presenter.stop(&info.id).await;
    assert_eq!(presenter.active_sessions(), 0);

    // LiveKit goes away: the bot reports it and exits, and the session ends.
    let info = presenter
        .create(test_pdf(1), &token, "Again.pdf")
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
