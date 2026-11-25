//! The authoritative host runs upstream's real room protocol end to end: a
//! scene built with `authoritativeMultiplayer` under the host isolate, joined
//! through the preview's mini-comms door, answers a fake explorer peer the way
//! upstream's `@dcl/sdk/network` promises — a unicast CRDT state reply to the
//! peer that asked, and a registered-message pong broadcast with the asker's
//! address as the sender context.
//!
//! Wire facts pinned here (js-sdk-toolchain `network/binary-message-bus.js`
//! and `network/events/protocol.js` on the auth-server line): a scene frame is
//! `[type u8][payload]`; the host stamps inbound frames as
//! `[senderLen u8][sender][frame]`; a custom event's payload is the envelope
//! `{eventType: String, timestamp: Int64}` followed by the registered schema,
//! all little-endian with u32 length-prefixed utf8 strings.
//!
//! An explorer (bevy, Unity) wraps each scene frame in an rfc4 `Packet` and
//! prefixes the frame with one CommsMessageType byte (2 = binary), reads
//! scene traffic in its `scene-<sceneId>` room when it uses the scene
//! gatekeeper, and sends its position as an rfc4 `Position`.

mod common {
    pub mod testgate;
    pub mod wait;
    pub mod wallet;
    pub mod ws;
}

use common::ws::{connect_ws, handshake};
use dcl_scene_cli::build::{self, BuildOptions};
use dcl_scene_cli::comms::proto::{WsPacket, WsPeerUpdate, ws_packet};
use dcl_scene_cli::comms::{CommsState, routes};
use dcl_scene_cli::{host, init};
use futures::{SinkExt, StreamExt};
use prost::Message as _;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_tungstenite::tungstenite::Message;

const HOST_ADDRESS: &str = "0x0000000000000000000000000000000000000000";
const CUSTOM_EVENT: u8 = 6;
const REQ_CRDT_STATE: u8 = 8;
const RES_CRDT_STATE: u8 = 9;

/// The proof scene: one synced cube (so the state reply carries bytes), a
/// server-side ping handler that answers with the sender the room handed it,
/// and a `whereAmI` handler that answers with the height the server reads off
/// the asker's player Transform (-1 while it has none).
const SCENE: &str = r#"import { engine, Transform, MeshRenderer, PlayerIdentityData, Schemas } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'
import { isServer, registerMessages, syncEntity } from '@dcl/sdk/network'

const room = registerMessages({
  ping: Schemas.Map({ n: Schemas.Int }),
  pong: Schemas.Map({ n: Schemas.Int, from: Schemas.String }),
  whereAmI: Schemas.Map({ n: Schemas.Int }),
  here: Schemas.Map({ n: Schemas.Int, y100: Schemas.Int })
})

export function main() {
  const cube = engine.addEntity()
  Transform.create(cube, { position: Vector3.create(8, 1, 8) })
  MeshRenderer.setBox(cube)
  syncEntity(cube, [Transform.componentId], 1)
  if (isServer()) {
    room.onMessage('ping', (msg, ctx) => {
      room.send('pong', { n: msg.n + 1, from: ctx?.from ?? '' })
    })
    room.onMessage('whereAmI', (msg, ctx) => {
      let y100 = -1
      for (const [entity, identity] of engine.getEntitiesWith(PlayerIdentityData)) {
        if (identity.address.toLowerCase() !== (ctx?.from ?? '').toLowerCase()) continue
        const t = Transform.getOrNull(entity)
        if (t) y100 = Math.round(t.position.y * 100)
      }
      room.send('here', { n: msg.n, y100 })
    })
  }
}
"#;

fn node_bin() -> Option<PathBuf> {
    match build::find_node() {
        Some(p) => Some(p),
        None => common::testgate::unavailable(
            "node",
            "install node; the authoritative host runs the scene under it",
        ),
    }
}

/// The host tests run one at a time: each holds this for its whole run, so
/// the environment one writes is never read by another.
static ONE_HOST_TEST: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A fresh scaffold, in a folder of its own named `name`, with the flag set
/// and the proof scene in place, built the way `start` builds it.
async fn build_flagged_scene(name: &str) -> PathBuf {
    for key in ["DCL_PRIVATE_KEY", "RUST_LOG", "NO_COLOR"] {
        // SAFETY: called under `ONE_HOST_TEST`, and the tests outside it read
        // no environment.
        unsafe { std::env::remove_var(key) };
    }
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("host-room")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    init::init(&init::InitOptions {
        dir: root.clone(),
        project: Some(init::ProjectKind::Scene),
        yes: true,
        node_modules_only: false,
    })
    .expect("scaffolding the proof scene");
    std::fs::write(root.join("src/index.ts"), SCENE).unwrap();
    let scene_json = root.join("scene.json");
    let mut scene: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&scene_json).unwrap()).unwrap();
    scene["authoritativeMultiplayer"] = serde_json::Value::Bool(true);
    std::fs::write(&scene_json, serde_json::to_string_pretty(&scene).unwrap()).unwrap();
    build::build(&BuildOptions {
        built_in: false,
        dir: root.clone(),
        production: true,
        ignore_composite: false,
        custom_entry_point: false,
        skip_type_check: false,
        out_root: None,
        quiet: true,
    })
    .await
    .expect("building the proof scene");
    dunce::canonicalize(root).unwrap()
}

/// The comms routes alone, served with connect info so the loopback-only
/// host door admits the isolate.
async fn spawn_comms() -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = routes(Arc::new(CommsState::default()));
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    addr
}

fn u32_le(n: u32) -> [u8; 4] {
    n.to_le_bytes()
}

/// `[CUSTOM_EVENT][envelope][payload]` for `ping {n}`.
fn ping_frame(n: i32, timestamp: i64) -> Vec<u8> {
    let mut out = vec![CUSTOM_EVENT];
    out.extend_from_slice(&u32_le(4));
    out.extend_from_slice(b"ping");
    out.extend_from_slice(&timestamp.to_le_bytes());
    out.extend_from_slice(&n.to_le_bytes());
    out
}

/// Decodes a `pong {n, from}` custom event; `None` for any other frame.
fn decode_pong(frame: &[u8]) -> Option<(i32, String)> {
    let mut at = 0usize;
    let take = |at: &mut usize, n: usize| -> Option<&[u8]> {
        let slice = frame.get(*at..*at + n)?;
        *at += n;
        Some(slice)
    };
    if *take(&mut at, 1)?.first()? != CUSTOM_EVENT {
        return None;
    }
    let read_string = |at: &mut usize| -> Option<String> {
        let len = u32::from_le_bytes(take(at, 4)?.try_into().ok()?) as usize;
        String::from_utf8(take(at, len)?.to_vec()).ok()
    };
    if read_string(&mut at)? != "pong" {
        return None;
    }
    take(&mut at, 8)?;
    let n = i32::from_le_bytes(take(&mut at, 4)?.try_into().ok()?);
    let from = read_string(&mut at)?;
    (at == frame.len()).then_some((n, from))
}

/// The next peer update on the socket before `deadline`, skipping presence
/// traffic; the host may take a while to load the scene under node.
async fn next_update(socket: &mut common::ws::Socket, deadline: Instant) -> WsPeerUpdate {
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        assert!(!left.is_zero(), "timed out waiting for a host update");
        let msg = tokio::time::timeout(left, socket.next())
            .await
            .expect("timed out waiting for a host update")
            .expect("the room closed")
            .expect("ws frame error");
        match msg {
            Message::Binary(bytes) => {
                let packet = WsPacket::decode(bytes.as_ref()).expect("valid WsPacket");
                if let Some(ws_packet::Message::PeerUpdateMessage(u)) = packet.message {
                    return u;
                }
            }
            Message::Ping(_) | Message::Pong(_) => continue,
            other => panic!("unexpected frame {other:?}"),
        }
    }
}

async fn send_scene_frame(socket: &mut common::ws::Socket, body: Vec<u8>) {
    let packet = WsPacket {
        message: Some(ws_packet::Message::PeerUpdateMessage(WsPeerUpdate {
            from_alias: 0,
            body,
            unreliable: false,
            recipients: Vec::new(),
        })),
    };
    socket
        .send(Message::Binary(packet.encode_to_vec().into()))
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_host_answers_state_requests_and_registered_messages() {
    let _one = ONE_HOST_TEST.lock().await;
    if node_bin().is_none() {
        return;
    }
    let root = build_flagged_scene("state-and-messages").await;
    let addr = spawn_comms().await;
    let preview = format!("http://{addr}");
    let _isolate = host::spawn_isolate(&root, &preview, "room-1").expect("spawning the host");

    let signer = common::wallet::random_wallet();
    let me = common::ws::wallet_address(&signer).to_lowercase();
    let mut peer = connect_ws(&format!("ws://{addr}/mini-comms/room-1"), "rfc5").await;
    let (_alias, peers) = handshake(&mut peer, &signer).await;
    let deadline = Instant::now() + Duration::from_secs(90);
    wait_for_host(&mut peer, &peers, deadline).await;

    // upstream's client opens with REQ_CRDT_STATE; the host answers the asker
    // alone with RES_CRDT_STATE (the relay unicasts on the host's `to` list)
    send_scene_frame(&mut peer, vec![REQ_CRDT_STATE]).await;
    let state = next_update(&mut peer, deadline).await;
    assert_eq!(
        state.body.first(),
        Some(&RES_CRDT_STATE),
        "the host answers a state request with RES_CRDT_STATE, got {:?}",
        &state.body[..state.body.len().min(16)]
    );
    assert!(
        state.body.len() > 1,
        "the synced cube gives the state reply a body"
    );

    // a registered message reaches the server handler with the sender in its
    // context, and the reply comes back through the same room
    send_scene_frame(&mut peer, ping_frame(1, 1_700_000_000_000)).await;
    let (n, from) = loop {
        let update = next_update(&mut peer, deadline).await;
        if let Some(pong) = decode_pong(&update.body) {
            break pong;
        }
    };
    assert_eq!(n, 2, "the handler adds one");
    assert_eq!(from, me, "the server handler sees the asker's address");
}

/// The host may have joined before the peer (in its welcome) or after (a join).
async fn wait_for_host(
    peer: &mut common::ws::Socket,
    peers: &std::collections::HashMap<u32, String>,
    deadline: Instant,
) {
    if peers.values().any(|a| a == HOST_ADDRESS) {
        return;
    }
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let msg = tokio::time::timeout(left, peer.next())
            .await
            .expect("timed out waiting for the host to join")
            .unwrap()
            .unwrap();
        if let Message::Binary(bytes) = msg {
            let packet = WsPacket::decode(bytes.as_ref()).unwrap();
            if let Some(ws_packet::Message::PeerJoinMessage(j)) = packet.message
                && j.address == HOST_ADDRESS
            {
                return;
            }
        }
    }
}

const COMMS_BINARY: u8 = 2;
const EXPLORER_SCENE_ID: &str = "bafkreiexplorerproof";

fn varint(mut n: u64) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let byte = (n & 0x7f) as u8;
        n >>= 7;
        if n == 0 {
            out.push(byte);
            return out;
        }
        out.push(byte | 0x80);
    }
}

fn field_bytes(field: u64, bytes: &[u8]) -> Vec<u8> {
    let mut out = varint((field << 3) | 2);
    out.extend(varint(bytes.len() as u64));
    out.extend_from_slice(bytes);
    out
}

fn field_f32(field: u64, value: f32) -> Vec<u8> {
    let mut out = varint((field << 3) | 5);
    out.extend_from_slice(&value.to_le_bytes());
    out
}

/// `protocol_version: 100`, the field every explorer packet carries.
fn protocol_version() -> Vec<u8> {
    let mut out = varint(11 << 3);
    out.extend(varint(100));
    out
}

/// An explorer's scene packet: rfc4 `Packet { scene: { scene_id, data } }`
/// with the data framed `[CommsMessageType::Binary][frame]`.
fn explorer_scene(frame: &[u8]) -> Vec<u8> {
    let mut data = vec![COMMS_BINARY];
    data.extend_from_slice(frame);
    let mut scene = field_bytes(1, EXPLORER_SCENE_ID.as_bytes());
    scene.extend(field_bytes(2, &data));
    let mut out = field_bytes(6, &scene);
    out.extend(protocol_version());
    out
}

/// An explorer's rfc4 `Position` (world space, metres).
fn explorer_position(x: f32, y: f32, z: f32) -> Vec<u8> {
    let mut position = varint(1 << 3);
    position.extend(varint(1));
    for (field, value) in [(3, x), (4, y), (5, z), (9, 1.0)] {
        position.extend(field_f32(field, value));
    }
    let mut out = field_bytes(1, &position);
    out.extend(protocol_version());
    out
}

/// The length-delimited `field` of a protobuf message, if present.
fn take_field(message: &[u8], field: u64) -> Option<Vec<u8>> {
    let mut at = 0usize;
    let read_varint = |at: &mut usize| -> Option<u64> {
        let mut n = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = *message.get(*at)?;
            *at += 1;
            n |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Some(n);
            }
        }
        None
    };
    while at < message.len() {
        let tag = read_varint(&mut at)?;
        match tag & 7 {
            0 => {
                read_varint(&mut at)?;
            }
            2 => {
                let len = read_varint(&mut at)? as usize;
                let bytes = message.get(at..at + len)?.to_vec();
                at += len;
                if tag >> 3 == field {
                    return Some(bytes);
                }
            }
            5 => at += 4,
            1 => at += 8,
            _ => return None,
        }
    }
    None
}

/// The SDK frame inside a scene packet the host sent an explorer, with the
/// binary type byte checked and removed; `None` for any other packet.
fn host_scene_frame(body: &[u8]) -> Option<Vec<u8>> {
    // the host's rfc4 wrapper ends every packet with `protocol_version: 100`;
    // anything else is a raw frame sent before it knew the scene id
    if !body.ends_with(&protocol_version()) {
        return None;
    }
    let scene = take_field(body, 6)?;
    assert_eq!(
        take_field(&scene, 1).as_deref(),
        Some(EXPLORER_SCENE_ID.as_bytes()),
        "the host stamps the scene id the explorer used"
    );
    let data = take_field(&scene, 2)?;
    assert_eq!(
        data.first(),
        Some(&COMMS_BINARY),
        "the host frames scene data as binary for explorers"
    );
    Some(data[1..].to_vec())
}

/// `[CUSTOM_EVENT][envelope][payload]` for `whereAmI {n}`.
fn where_frame(n: i32) -> Vec<u8> {
    let mut out = vec![CUSTOM_EVENT];
    out.extend_from_slice(&u32_le(8));
    out.extend_from_slice(b"whereAmI");
    out.extend_from_slice(&1_700_000_000_000i64.to_le_bytes());
    out.extend_from_slice(&n.to_le_bytes());
    out
}

/// Decodes a `here {n, y100}` custom event; `None` for any other frame.
fn decode_here(frame: &[u8]) -> Option<(i32, i32)> {
    let name = b"here";
    let head = 1 + 4 + name.len() + 8;
    if frame.len() != head + 8
        || frame[0] != CUSTOM_EVENT
        || frame[1..5] != u32_le(name.len() as u32)
        || &frame[5..9] != name
    {
        return None;
    }
    let n = i32::from_le_bytes(frame[head..head + 4].try_into().ok()?);
    let y100 = i32::from_le_bytes(frame[head + 4..head + 8].try_into().ok()?);
    Some((n, y100))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_host_serves_an_explorer_in_its_scene_room() {
    let _one = ONE_HOST_TEST.lock().await;
    if node_bin().is_none() {
        return;
    }
    let root = build_flagged_scene("explorer-scene-room").await;
    let addr = spawn_comms().await;
    let preview = format!("http://{addr}");
    // as `start` does: the host joins the realm room
    let _isolate = host::spawn_isolate(&root, &preview, "room-1").expect("spawning the host");

    // an explorer that used the scene gatekeeper sits in the scene room only
    let signer = common::wallet::random_wallet();
    let mut peer = connect_ws(
        &format!("ws://{addr}/mini-comms/scene-{EXPLORER_SCENE_ID}"),
        "rfc5",
    )
    .await;
    let (_alias, peers) = handshake(&mut peer, &signer).await;
    let deadline = Instant::now() + Duration::from_secs(90);
    wait_for_host(&mut peer, &peers, deadline).await;

    send_scene_frame(&mut peer, explorer_scene(&[REQ_CRDT_STATE])).await;
    let state = loop {
        let update = next_update(&mut peer, deadline).await;
        if let Some(frame) = host_scene_frame(&update.body) {
            break frame;
        }
    };
    assert_eq!(
        state.first(),
        Some(&RES_CRDT_STATE),
        "the host answers an explorer's framed state request across rooms, got {:?}",
        &state[..state.len().min(16)]
    );

    // the explorer's position is the player Transform server code reads
    send_scene_frame(&mut peer, explorer_position(8.0, 2.5, 9.0)).await;
    let mut asked = 0;
    let y100 = loop {
        asked += 1;
        send_scene_frame(&mut peer, explorer_scene(&where_frame(asked))).await;
        let answer = loop {
            let update = next_update(&mut peer, deadline).await;
            if let Some((n, y100)) = host_scene_frame(&update.body).and_then(|f| decode_here(&f))
                && n == asked
            {
                break y100;
            }
        };
        if answer >= 0 || Instant::now() >= deadline {
            break answer;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    };
    assert_eq!(
        y100, 250,
        "the server reads the explorer's height off its Transform"
    );
}

#[test]
fn explorer_frames_round_trip_through_the_parsers() {
    let packet = explorer_scene(&[RES_CRDT_STATE, 7]);
    assert_eq!(host_scene_frame(&packet), Some(vec![RES_CRDT_STATE, 7]));
    assert_eq!(host_scene_frame(&explorer_position(1.0, 2.0, 3.0)), None);
    let mut here = vec![CUSTOM_EVENT];
    here.extend_from_slice(&u32_le(4));
    here.extend_from_slice(b"here");
    here.extend_from_slice(&0i64.to_le_bytes());
    here.extend_from_slice(&3i32.to_le_bytes());
    here.extend_from_slice(&250i32.to_le_bytes());
    assert_eq!(decode_here(&here), Some((3, 250)));
    assert_eq!(decode_here(&where_frame(1)), None);
}

#[test]
fn ping_frames_round_trip_through_the_decoder() {
    let mut pong = ping_frame(7, 42);
    pong.splice(5..9, *b"pong");
    pong.extend_from_slice(&u32_le(2));
    pong.extend_from_slice(b"me");
    assert_eq!(decode_pong(&pong), Some((7, "me".to_string())));
    assert_eq!(decode_pong(&ping_frame(1, 1)), None, "a ping is not a pong");
    assert_eq!(decode_pong(&[RES_CRDT_STATE, 1, 2]), None);
}
