//! The stage 5 worlds acceptance flow against RUNNING processes
//! (`tests/worlds/live.sh` prepares the database and runs this): on a fresh
//! database with `WORLDS_WRITES_ENABLED=true`, a test wallet publishes a small
//! SDK7 multiplayer world through `POST /worlds-content-server/entities`; a
//! client fetches `/world/{name}/about`; the authoritative host loads the
//! scene and two clients join its room over `/scene-state/ws/{name}` through
//! the room resolver and relay CRDT to each other; a voice token is minted
//! through `/comms-gatekeeper/get-scene-adapter` with the worlds policy, and
//! refused after the owner revokes the player's access, which also closes the
//! player's running socket; player and world storage values persist across a
//! `kill -9` and restart of the node; `catalyst pulse` runs in its own
//! process, legacy clients authenticate with signed timestamp handshakes,
//! Pulse's cluster feed reaches archipelago over NATS, and the same wallet
//! heartbeating into archipelago shows on `/hot-scenes`. Skips unless
//! `CATALYST_WORLDS_LIVE_BIN` is set.
//!
//! Environment (the script exports every one): `CATALYST_WORLDS_LIVE_BIN`
//! (the `catalyst` binary), `_DATABASE_URL` (an empty schema-applied
//! database), `_ROOT` (a scratch directory for the blob store, the Pulse
//! replay journal and the process logs), `_PORT` (the node), `_PULSE_PORT`
//! (Pulse's ENet/UDP bind) and `_NATS_SERVER_BIN` (optional: a nats-server
//! binary; without it the feed assertions are skipped and say so).

#[path = "support/nats_broker.rs"]
mod nats_broker;

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use alloy::signers::{Signer, local::PrivateKeySigner};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use futures::{SinkExt, StreamExt};
use hmac::{Hmac, KeyInit, Mac};
use prost::Message as _;
use reqwest::header::CONTENT_TYPE;
use serde_json::{Value, json};
use sha2::Sha256;
use sqlx::postgres::PgPoolOptions;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use uuid::Uuid;

use catalyst::auth::hash::hash_bytes_v1;
use catalyst::auth::{AuthLink, AuthLinkType};
use catalyst::comms::protocol::archipelago::{Position, archipelago};
use catalyst::comms::pulse::decentraland::pulse::{
    ClientMessage, HandshakeRequest, PlayerInitialState, PlayerState, ServerMessage,
    client_message, server_message,
};
use catalyst::comms::pulse::interest::{ParcelEncoder, ParcelEncoderOptions};
use catalyst::comms::pulse::transport::{Event, Host, HostConfig, Packet, PeerId};
use catalyst::shared::signed_fetch::{
    AUTH_CHAIN_HEADER_PREFIX, AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER, build_payload_v6,
};
use catalyst::worlds::scene_state::crdt::{CrdtMessage, decode_batch, encode_batch};
use catalyst::worlds::scene_state::protocol::{MessageType, encode_message};

const METADATA: &str = r#"{"signer":"dcl:explorer"}"#;
/// A 1x1 opaque PNG (the navmap thumbnail).
const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00,
    0x00, 0x03, 0x01, 0x01, 0x00, 0x18, 0xDD, 0x8D, 0xB0, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
    0x44, 0xAE, 0x42, 0x60, 0x82,
];
/// The scene the world publishes: a small SDK7 multiplayer server bundle the
/// host runs in its V8 isolate. `registerScene` installs the client observer
/// (the host hands it `open`/`close` events with a client handle); each tick
/// the scene drains every client's CRDT (`getMessages`) and forwards it to
/// the others (`sendCrdtMessage`, applied to the server's engine on the way),
/// which is the multiplayer relay an SDK7 server scene performs.
const GAME_JS: &str = r#"
const engine = require('~system/EngineApi');
const clients = new Map();
registerScene({ reservedLocalEntities: 512, networkEntitiesLimit: { serverLimit: 512, clientLimit: 512 } }, function (event) {
  if (event.type === 'open') { clients.set(event.clientId, event.client); }
  if (event.type === 'close') { clients.delete(event.clientId); }
});
let ticks = 0;
module.exports.onStart = function () { console.log('acceptance world started'); };
module.exports.onUpdate = function (dt) {
  ticks += 1;
  for (const [id, client] of clients) {
    const messages = client.getMessages();
    for (const message of messages) {
      for (const [otherId, other] of clients) {
        if (otherId !== id) { other.sendCrdtMessage(message); }
      }
    }
  }
  if (ticks % 600 === 0) { engine.crdtGetState(); }
};
"#;
const REALM: &str = "dcl-one";

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| {
        panic!("{name} must be set for the acceptance flow (tests/worlds/live.sh exports it)")
    })
}

fn random_token(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).unwrap();
    URL_SAFE_NO_PAD.encode(buf)
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

// ── wallets and signed fetches ─────────────────────────────────────────────

struct Wallet {
    root: PrivateKeySigner,
    ephemeral: PrivateKeySigner,
}

impl Wallet {
    fn random() -> Self {
        Self {
            root: PrivateKeySigner::random(),
            ephemeral: PrivateKeySigner::random(),
        }
    }

    fn address(&self) -> String {
        format!("{:#x}", self.root.address()).to_lowercase()
    }

    fn mandate(&self) -> String {
        format!(
            "Decentraland Login\nEphemeral address: {:#x}\nExpiration: 2099-01-01T00:00:00.000Z",
            self.ephemeral.address()
        )
    }

    /// The explorer's three-link chain over `payload`: SIGNER, the ephemeral
    /// mandate signed by the wallet, the payload signed by the ephemeral.
    async fn chain(&self, payload: &str) -> Vec<AuthLink> {
        let mandate = self.mandate();
        let mandate_sig = self
            .root
            .sign_message(mandate.as_bytes())
            .await
            .unwrap()
            .to_string();
        let entity_sig = self
            .ephemeral
            .sign_message(payload.as_bytes())
            .await
            .unwrap()
            .to_string();
        vec![
            AuthLink {
                link_type: AuthLinkType::SIGNER,
                payload: format!("{:#x}", self.root.address()),
                signature: None,
            },
            AuthLink {
                link_type: AuthLinkType::EcdsaEphemeral,
                payload: mandate,
                signature: Some(mandate_sig),
            },
            AuthLink {
                link_type: AuthLinkType::EcdsaSignedEntity,
                payload: payload.to_string(),
                signature: Some(entity_sig),
            },
        ]
    }

    /// The signed-fetch header bag over `method:path:timestamp:metadata`.
    async fn signed(&self, method: &str, path: &str, metadata: &str) -> Vec<(String, String)> {
        let timestamp = now_ms().to_string();
        let payload = build_payload_v6(method, path, &timestamp, metadata);
        let mut out = vec![
            (AUTH_TIMESTAMP_HEADER.to_string(), timestamp),
            (AUTH_METADATA_HEADER.to_string(), metadata.to_string()),
        ];
        for (i, link) in self.chain(&payload).await.iter().enumerate() {
            out.push((
                format!("{AUTH_CHAIN_HEADER_PREFIX}{i}"),
                serde_json::to_string(link).unwrap(),
            ));
        }
        out
    }

    /// The scene host's Auth frame: the same header bag as JSON, signed over
    /// `get:<public path>`.
    async fn auth_frame(&self, path: &str, metadata: &str) -> Vec<u8> {
        let headers = self.signed("get", path, metadata).await;
        let map: serde_json::Map<String, Value> = headers
            .into_iter()
            .map(|(k, v)| (k, Value::String(v)))
            .collect();
        encode_message(MessageType::Auth, Value::Object(map).to_string().as_bytes())
    }
}

struct Proc {
    child: Child,
    name: &'static str,
}

impl Proc {
    fn spawn(
        name: &'static str,
        bin: &str,
        arg: &str,
        env: &[(String, String)],
        log: &Path,
    ) -> Self {
        let out = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
            .unwrap();
        let err = out.try_clone().unwrap();
        let child = Command::new(bin)
            .arg(arg)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("HOME", std::env::var("HOME").unwrap_or_default())
            .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .stdin(Stdio::null())
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err))
            .spawn()
            .unwrap_or_else(|e| panic!("spawning {name}: {e}"));
        Self { child, name }
    }

    /// `kill -9`: no flush, no graceful shutdown.
    fn kill_9(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        eprintln!("[{}] killed with SIGKILL", self.name);
    }

    fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        if self.alive() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

struct Rig {
    bin: String,
    dsn: String,
    root: PathBuf,
    port: u16,
    pulse_port: u16,
    nats_url: Option<String>,
    livekit_key: String,
    livekit_secret: String,
    service_token: String,
    admin_token: String,
    encryption_key: String,
    authority_key: String,
    authoritative: Wallet,
}

impl Rig {
    fn serve_env(&self) -> Vec<(String, String)> {
        let mut env: Vec<(String, String)> = [
            ("DATABASE_URL", self.dsn.clone()),
            ("HTTP_SERVER_PORT", self.port.to_string()),
            ("PUBLIC_URL", format!("http://127.0.0.1:{}", self.port)),
            (
                "STORAGE_ROOT_FOLDER",
                self.root.join("content").display().to_string(),
            ),
            (
                "PROFILE_IMAGES_CACHE_DIR",
                self.root.join("profile-images").display().to_string(),
            ),
            (
                "AB_CDN_CACHE_DIR",
                self.root.join("ab-cdn").display().to_string(),
            ),
            (
                "BVIMPOSTERS_STORE_ROOT",
                self.root.join("bvimposters").display().to_string(),
            ),
            ("SERVE_UNSYNCED", "true".into()),
            ("SYNC_ENABLED", "false".into()),
            ("RUST_LOG", "info".into()),
            ("REALM_NAME", REALM.into()),
            ("WORLDS_WRITES_ENABLED", "true".into()),
            ("WORLDS_PERSONAL_WORLDS_MAX", "5".into()),
            ("CATALYRST_WORLDS_ADMIN_TOKEN", self.admin_token.clone()),
            ("ENCRYPTION_KEY", self.encryption_key.clone()),
            ("AUTHORITATIVE_SERVER_ADDRESS", self.authoritative.address()),
            ("LIVEKIT_HOST", "livekit.invalid".into()),
            ("LIVEKIT_API_KEY", self.livekit_key.clone()),
            ("LIVEKIT_API_SECRET", self.livekit_secret.clone()),
            ("COMMS_GATEKEEPER_AUTH_TOKEN", self.service_token.clone()),
            ("PULSE_ROOM_AUTHORITY_KEY", self.authority_key.clone()),
            ("SCENE_HOST_ENABLED", "true".into()),
            (
                "SCENE_HOST_AUTHORITATIVE_KEY",
                hex::encode(self.authoritative.root.to_bytes()),
            ),
            (
                "SCENE_HOST_STORAGE_URL",
                format!("http://127.0.0.1:{}", self.port),
            ),
            ("SCENE_HOST_STORAGE_ALLOW_HTTP", "true".into()),
            ("SCENE_HOST_AUTH_TIMEOUT_SECS", "5".into()),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        if let Some(url) = &self.nats_url {
            env.push(("NATS_URL".into(), url.clone()));
        }
        env
    }

    fn pulse_env(&self, metrics_port: u16) -> Vec<(String, String)> {
        let mut env: Vec<(String, String)> = [
            ("PULSE_BIND", format!("127.0.0.1:{}", self.pulse_port)),
            ("PULSE_METRICS_BIND", format!("127.0.0.1:{metrics_port}")),
            ("PULSE_WT_ENABLED", "false".into()),
            (
                "PULSE_ROOM_AUTHORITY_URL",
                format!(
                    "http://127.0.0.1:{}/internal/pulse/room-authority/v1",
                    self.port
                ),
            ),
            ("PULSE_ROOM_AUTHORITY_KEY", self.authority_key.clone()),
            ("PULSE_ROOM_AUTHORITY_ALLOW_LOOPBACK_HTTP", "true".into()),
            ("PULSE_CLUSTERS_ENABLED", "true".into()),
            ("PULSE_CLUSTERS_PASS_INTERVAL_MS", "200".into()),
            ("PULSE_CLUSTERS_DWELL_PASSES", "1".into()),
            ("PULSE_NATS_DISCOVERY_INTERVAL_MS", "500".into()),
            (
                "PULSE_REPLAY_STATE_PATH",
                self.root.join("pulse-replay.tsv").display().to_string(),
            ),
            ("RUST_LOG", "info".into()),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        if let Some(url) = &self.nats_url {
            env.push(("PULSE_NATS_URL".into(), url.clone()));
        }
        env
    }

    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    async fn start_node(&self) -> Proc {
        let node = Proc::spawn(
            "serve",
            &self.bin,
            "serve",
            &self.serve_env(),
            &self.root.join("serve.log"),
        );
        wait_http(&self.url("/about"), Duration::from_secs(60)).await;
        node
    }
}

async fn wait_http(url: &str, within: Duration) {
    let deadline = Instant::now() + within;
    let client = reqwest::Client::new();
    loop {
        if let Ok(resp) = client.get(url).send().await
            && resp.status().is_success()
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "{url} did not answer within {within:?}"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn wait_until<F, Fut>(what: &str, within: Duration, mut probe: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = Instant::now() + within;
    loop {
        if probe().await {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "{what} did not happen within {within:?}"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

struct Node {
    base: String,
    http: reqwest::Client,
}

impl Node {
    async fn send(
        &self,
        method: &str,
        path: &str,
        headers: Vec<(String, String)>,
        body: Option<(Vec<u8>, &str)>,
    ) -> (u16, Value) {
        let mut req = self.http.request(
            method.to_uppercase().parse().unwrap(),
            format!("{}{path}", self.base),
        );
        for (k, v) in headers {
            req = req.header(k, v);
        }
        if let Some((bytes, ct)) = body {
            req = req.header(CONTENT_TYPE, ct).body(bytes);
        }
        let resp = req
            .send()
            .await
            .unwrap_or_else(|e| panic!("{method} {path}: {e}"));
        let status = resp.status().as_u16();
        let bytes = resp.bytes().await.unwrap();
        let value = serde_json::from_slice(&bytes)
            .unwrap_or(Value::String(String::from_utf8_lossy(&bytes).into()));
        (status, value)
    }

    async fn get(&self, path: &str) -> (u16, Value) {
        self.send("get", path, Vec::new(), None).await
    }

    /// A signed fetch over the public path (the nested prefix included, as a
    /// browser at this origin would sign it; the worlds, storage and scene
    /// host routes verify the path they were served at).
    async fn signed(
        &self,
        w: &Wallet,
        method: &str,
        path: &str,
        metadata: &str,
        json: Option<Value>,
    ) -> (u16, Value) {
        let route = path.split('?').next().unwrap();
        self.signed_over(w, method, path, route, metadata, json)
            .await
    }

    /// A signed fetch whose signature covers `signed_path`: the comms
    /// gatekeeper verifies its vhost-root route (`/get-scene-adapter`, what
    /// `comms-gatekeeper.<domain>` clients sign; a prefix proxy forwards
    /// the public path as `x-original-path`).
    async fn signed_over(
        &self,
        w: &Wallet,
        method: &str,
        path: &str,
        signed_path: &str,
        metadata: &str,
        json: Option<Value>,
    ) -> (u16, Value) {
        let headers = w.signed(method, signed_path, metadata).await;
        let body = json.map(|v| (v.to_string().into_bytes(), "application/json"));
        self.send(method, path, headers, body).await
    }
}

fn multipart(parts: &[(&str, Option<&str>, &[u8])]) -> (Vec<u8>, String) {
    let boundary = format!("----catalyst{}", Uuid::new_v4().simple());
    let mut body = Vec::new();
    for (name, filename, value) in parts {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        match filename {
            Some(f) => body.extend_from_slice(
                format!("Content-Disposition: form-data; name=\"{name}\"; filename=\"{f}\"\r\nContent-Type: application/octet-stream\r\n\r\n").as_bytes(),
            ),
            None => body.extend_from_slice(format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes()),
        }
        body.extend_from_slice(value);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (body, format!("multipart/form-data; boundary={boundary}"))
}

// ── the scene host socket ──────────────────────────────────────────────────

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

struct SceneSocket {
    ws: Ws,
    /// The first entity of the range the Init frame granted.
    range_start: u32,
}

/// Connects, sends the Auth frame and returns the socket with its Init
/// range, or `None` when the host closed it instead of admitting.
async fn join_scene(rig: &Rig, world: &str, wallet: &Wallet) -> Option<SceneSocket> {
    let path = format!("/scene-state/ws/{world}");
    let (mut ws, _) =
        tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{}{path}", rig.port))
            .await
            .ok()?;
    ws.send(WsMessage::Binary(
        wallet.auth_frame(&path, METADATA).await.into(),
    ))
    .await
    .unwrap();
    match tokio::time::timeout(Duration::from_secs(10), ws.next())
        .await
        .expect("an answer within 10 s")
    {
        Some(Ok(WsMessage::Binary(b))) if b.first() == Some(&(MessageType::Init as u8)) => {
            let range_start = u32::from_be_bytes([b[1], b[2], b[3], b[4]]);
            Some(SceneSocket { ws, range_start })
        }
        Some(Ok(WsMessage::Close(_))) | None => None,
        other => panic!("unexpected first frame: {other:?}"),
    }
}

/// The next CRDT frame's messages, skipping anything else.
async fn next_crdt(ws: &mut Ws, within: Duration) -> Option<Vec<CrdtMessage>> {
    let deadline = tokio::time::Instant::now() + within;
    loop {
        match tokio::time::timeout_at(deadline, ws.next()).await {
            Ok(Some(Ok(WsMessage::Binary(b)))) if b.first() == Some(&(MessageType::Crdt as u8)) => {
                return Some(decode_batch(&b[1..]));
            }
            Ok(Some(Ok(WsMessage::Close(_)))) | Ok(None) | Ok(Some(Err(_))) => return None,
            Ok(Some(Ok(_))) => continue,
            Err(_) => return None,
        }
    }
}

/// Whether the host closes the socket within `within`.
async fn closed(ws: &mut Ws, within: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + within;
    loop {
        match tokio::time::timeout_at(deadline, ws.next()).await {
            Ok(Some(Ok(WsMessage::Close(_)))) | Ok(None) | Ok(Some(Err(_))) => return true,
            Ok(Some(Ok(_))) => continue,
            Err(_) => return false,
        }
    }
}

// ── the Pulse client ───────────────────────────────────────────────────────

struct PulseClient {
    host: Host,
    peer: PeerId,
    inbox: VecDeque<server_message::Message>,
}

impl PulseClient {
    async fn connect(addr: SocketAddr) -> Self {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let mut host = Host::bind(HostConfig {
                bind: "127.0.0.1:0".parse().unwrap(),
                max_peers: 1,
                channel_limit: 3,
            })
            .await
            .unwrap();
            let peer = host.connect(addr, 3).unwrap();
            let attempt = Instant::now() + Duration::from_secs(3);
            while Instant::now() < attempt {
                match tokio::time::timeout(Duration::from_millis(100), host.service()).await {
                    Ok(Ok(Some(Event::Connect { .. }))) => {
                        return Self {
                            host,
                            peer,
                            inbox: VecDeque::new(),
                        };
                    }
                    Ok(Ok(Some(Event::Disconnect { .. }))) => break,
                    _ => {}
                }
            }
            assert!(
                Instant::now() < deadline,
                "Pulse at {addr} did not accept an ENet connection within 30 s"
            );
        }
    }

    /// Queues a reliable frame and services the socket so it leaves (ENet
    /// sends on service/flush, not on enqueue).
    async fn send(&mut self, message: client_message::Message) {
        let bytes = ClientMessage {
            message: Some(message),
        }
        .encode_to_vec();
        self.host
            .send(self.peer, Packet::reliable(0, bytes))
            .await
            .unwrap();
        self.host.flush();
        self.pump(Duration::from_millis(50)).await;
    }

    /// Services the socket until `pick` accepts a message or `within` passes.
    async fn expect<T>(
        &mut self,
        what: &str,
        within: Duration,
        pick: impl Fn(&server_message::Message) -> Option<T>,
    ) -> T {
        let deadline = Instant::now() + within;
        loop {
            while let Some(message) = self.inbox.pop_front() {
                if let Some(found) = pick(&message) {
                    return found;
                }
            }
            assert!(
                Instant::now() < deadline,
                "{what}: nothing within {within:?}"
            );
            match tokio::time::timeout(Duration::from_millis(100), self.host.service()).await {
                Ok(Ok(Some(Event::Receive { packet, .. }))) => {
                    if let Ok(msg) = ServerMessage::decode(packet.data.as_ref())
                        && let Some(inner) = msg.message
                    {
                        self.inbox.push_back(inner);
                    }
                }
                Ok(Ok(Some(Event::Disconnect { .. }))) => {
                    panic!("{what}: Pulse disconnected the client")
                }
                _ => {}
            }
        }
    }

    /// Keeps the socket serviced for `dur` (ENet acks, keepalives).
    async fn pump(&mut self, dur: Duration) {
        let deadline = Instant::now() + dur;
        while Instant::now() < deadline {
            if let Ok(Ok(Some(Event::Receive { packet, .. }))) =
                tokio::time::timeout(Duration::from_millis(50), self.host.service()).await
                && let Ok(msg) = ServerMessage::decode(packet.data.as_ref())
                && let Some(inner) = msg.message
            {
                self.inbox.push_back(inner);
            }
        }
    }

    /// Admit a legacy player standing at `parcel` in `REALM`.
    async fn handshake(&mut self, wallet: &Wallet, parcel: (i32, i32)) {
        let encoder = ParcelEncoder::new(ParcelEncoderOptions::default());
        let mut state = PlayerState {
            parcel_index: encoder.encode(parcel.0, parcel.1),
            ..Default::default()
        };
        state.set_position_x_f(8.0);
        state.set_position_z_f(8.0);
        let headers: serde_json::Map<String, Value> = wallet
            .signed("connect", "/", METADATA)
            .await
            .into_iter()
            .map(|(key, value)| (key, Value::String(value)))
            .collect();
        self.send(client_message::Message::Handshake(HandshakeRequest {
            auth_chain: serde_json::to_vec(&headers).unwrap(),
            profile_version: 0,
            initial_state: Some(PlayerInitialState {
                state: Some(state),
                realm: REALM.to_string(),
                ..Default::default()
            }),
            protocol_features: 0,
        }))
        .await;
        let result = self
            .expect("legacy handshake", Duration::from_secs(10), |m| match m {
                server_message::Message::Handshake(result) => Some(result.clone()),
                _ => None,
            })
            .await;
        assert!(result.success, "legacy admission refused: {result:?}");
    }
}

// ── archipelago's legacy socket ────────────────────────────────────────────

fn archipelago_packet(message: archipelago::client_packet::Message) -> WsMessage {
    WsMessage::Binary(
        archipelago::ClientPacket {
            request_sequence: 0,
            message: Some(message),
        }
        .encode_to_vec()
        .into(),
    )
}

async fn archipelago_recv(
    ws: &mut Ws,
    within: Duration,
) -> Option<archipelago::server_packet::Message> {
    let deadline = tokio::time::Instant::now() + within;
    loop {
        match tokio::time::timeout_at(deadline, ws.next()).await.ok()?? {
            Ok(WsMessage::Binary(bytes)) => {
                return archipelago::ServerPacket::decode(bytes.as_ref())
                    .ok()?
                    .message;
            }
            Ok(WsMessage::Ping(_)) | Ok(WsMessage::Pong(_)) => continue,
            _ => return None,
        }
    }
}

/// The signed-challenge handshake of the legacy `archipelago` socket and one
/// heartbeat at `position` in `REALM`.
async fn archipelago_heartbeat(rig: &Rig, wallet: &Wallet, position: [f32; 3]) -> Ws {
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{}/ws", rig.port))
        .await
        .expect("archipelago /ws");
    ws.send(archipelago_packet(
        archipelago::client_packet::Message::ChallengeRequest(
            archipelago::ChallengeRequestMessage {
                address: wallet.address(),
            },
        ),
    ))
    .await
    .unwrap();
    let challenge = match archipelago_recv(&mut ws, Duration::from_secs(5)).await {
        Some(archipelago::server_packet::Message::ChallengeResponse(r)) => r.challenge_to_sign,
        other => panic!("expected a ChallengeResponse, got {other:?}"),
    };
    let chain = serde_json::to_string(&wallet.chain(&challenge).await).unwrap();
    ws.send(archipelago_packet(
        archipelago::client_packet::Message::SignedChallenge(archipelago::SignedChallengeMessage {
            auth_chain_json: chain,
        }),
    ))
    .await
    .unwrap();
    match archipelago_recv(&mut ws, Duration::from_secs(5)).await {
        Some(archipelago::server_packet::Message::Welcome(_)) => {}
        other => panic!("expected Welcome, got {other:?}"),
    }
    ws.send(archipelago_packet(
        archipelago::client_packet::Message::Heartbeat(archipelago::Heartbeat {
            position: Some(Position {
                x: position[0],
                y: position[1],
                z: position[2],
            }),
            desired_room: Some(REALM.to_string()),
            ..Default::default()
        }),
    ))
    .await
    .unwrap();
    ws
}

fn jwt_from_adapter(adapter: &str) -> String {
    adapter
        .split("access_token=")
        .nth(1)
        .expect("an access_token in the adapter URL")
        .to_string()
}

fn jwt_payload(jwt: &str) -> Value {
    serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(jwt.split('.').nth(1).unwrap())
            .unwrap(),
    )
    .unwrap()
}

fn verify_hs256(jwt: &str, secret: &str) -> bool {
    let (signed, signature) = jwt.rsplit_once('.').unwrap();

    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(signed.as_bytes());
    URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()) == signature
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_world_is_published_hosted_joined_voiced_revoked_persisted_and_pulsed() {
    let Ok(bin) = std::env::var("CATALYST_WORLDS_LIVE_BIN") else {
        eprintln!("skipping: CATALYST_WORLDS_LIVE_BIN not set (tests/worlds/live.sh runs this)");
        return;
    };
    let root = PathBuf::from(env("CATALYST_WORLDS_LIVE_ROOT"));
    std::fs::create_dir_all(root.join("content")).unwrap();
    std::fs::create_dir_all(root.join("profile-images")).unwrap();
    let nats_bin = std::env::var("CATALYST_WORLDS_LIVE_NATS_SERVER_BIN")
        .ok()
        .filter(|s| !s.trim().is_empty());
    let broker = nats_bin
        .as_deref()
        .map(|b| nats_broker::Broker::start(b, None));
    if broker.is_none() {
        eprintln!("no nats-server binary: the Pulse -> archipelago feed assertions are skipped");
    }
    let rig = Rig {
        bin,
        dsn: env("CATALYST_WORLDS_LIVE_DATABASE_URL"),
        root: root.clone(),
        port: env("CATALYST_WORLDS_LIVE_PORT").parse().unwrap(),
        pulse_port: env("CATALYST_WORLDS_LIVE_PULSE_PORT").parse().unwrap(),
        nats_url: broker.as_ref().map(|b| b.url()),
        livekit_key: format!("API{}", &random_token(9)[..12]),
        livekit_secret: random_token(36),
        service_token: random_token(24),
        admin_token: random_token(24),
        encryption_key: hex::encode({
            let mut k = [0u8; 32];
            getrandom::fill(&mut k).unwrap();
            k
        }),
        authority_key: random_token(32),
        authoritative: Wallet::random(),
    };
    let mut node = rig.start_node().await;
    let http = Node {
        base: rig.url(""),
        http: reqwest::Client::new(),
    };
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&rig.dsn)
        .await
        .unwrap();

    // 1. A test wallet publishes a small SDK7 multiplayer world (its personal
    //    world, so no NAME authority is needed) through the publish route.
    let owner = Wallet::random();
    let player = Wallet::random();
    let world = format!("{}.dcl.eth", owner.address());
    let game_hash = hash_bytes_v1(GAME_JS.as_bytes());
    let thumb_hash = hash_bytes_v1(PNG_1X1);
    let entity = json!({
        "version": "v3",
        "type": "scene",
        "timestamp": now_ms(),
        "pointers": ["0,0"],
        "content": [{ "file": "bin/game.js", "hash": game_hash }, { "file": "thumb.png", "hash": thumb_hash }],
        "metadata": {
            "display": { "title": "Acceptance world", "navmapThumbnail": "thumb.png" },
            "main": "bin/game.js",
            "runtimeVersion": "7",
            "worldConfiguration": { "name": world },
            "scene": { "base": "0,0", "parcels": ["0,0"] }
        }
    });
    let entity_bytes = serde_json::to_vec(&entity).unwrap();
    let entity_id = hash_bytes_v1(&entity_bytes);
    let chain = serde_json::to_string(&owner.chain(&entity_id).await).unwrap();
    let (body, content_type) = multipart(&[
        ("entityId", None, entity_id.as_bytes()),
        ("authChain", None, chain.as_bytes()),
        ("entity.json", Some("entity.json"), &entity_bytes),
        ("bin/game.js", Some("game.js"), GAME_JS.as_bytes()),
        ("thumb.png", Some("thumb.png"), PNG_1X1),
    ]);
    let (status, published) = http
        .send(
            "post",
            "/worlds-content-server/entities",
            Vec::new(),
            Some((body, &content_type)),
        )
        .await;
    assert_eq!(status, 200, "publish: {published}");
    assert_eq!(
        published["message"].as_str().map(|m| m.contains(&world)),
        Some(true),
        "{published}"
    );
    eprintln!("[1] published {world} as {entity_id}");

    // 2. A client fetches /world/{name}/about: the scene URN is the entity.
    let (status, about) = http.get(&format!("/world/{world}/about")).await;
    assert_eq!(status, 200, "{about}");
    let urn = about["configurations"]["scenesUrn"][0]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(
        urn.starts_with(&format!("urn:decentraland:entity:{entity_id}")),
        "{about}"
    );
    let (status, _) = http
        .get(&format!("/worlds-content-server/world/{world}/about"))
        .await;
    assert_eq!(status, 200);
    eprintln!("[2] /world/{world}/about advertises {urn}");

    // 3. The authoritative host starts the scene on first contact and two
    //    clients join its room; CRDT round-trips through the relay.
    let mut owner_socket = join_scene(&rig, &world, &owner)
        .await
        .expect("the owner joins the host");
    let mut player_socket = join_scene(&rig, &world, &player)
        .await
        .expect("the player joins the unrestricted world");
    let (status, host_status) = http.get("/scene-state/status").await;
    assert_eq!(status, 200);
    assert_eq!(
        host_status["loadedScenes"],
        json!([format!("{world}:{entity_id}")]),
        "{host_status}"
    );
    let from_player = CrdtMessage::Put {
        entity: player_socket.range_start + 7,
        component_id: 1,
        timestamp: 1,
        data: vec![1, 2, 3],
    };
    player_socket
        .ws
        .send(WsMessage::Binary(
            encode_message(
                MessageType::Crdt,
                &encode_batch(std::slice::from_ref(&from_player)),
            )
            .into(),
        ))
        .await
        .unwrap();
    assert_eq!(
        next_crdt(&mut owner_socket.ws, Duration::from_secs(10)).await,
        Some(vec![from_player.clone()]),
        "the owner sees the player's CRDT"
    );
    let from_owner = CrdtMessage::Put {
        entity: owner_socket.range_start + 3,
        component_id: 1,
        timestamp: 1,
        data: vec![9],
    };
    owner_socket
        .ws
        .send(WsMessage::Binary(
            encode_message(
                MessageType::Crdt,
                &encode_batch(std::slice::from_ref(&from_owner)),
            )
            .into(),
        ))
        .await
        .unwrap();
    assert_eq!(
        next_crdt(&mut player_socket.ws, Duration::from_secs(10)).await,
        Some(vec![from_owner]),
        "the player sees the owner's CRDT"
    );
    eprintln!("[3] host loaded {world}:{entity_id}; two sockets relayed CRDT both ways");

    // 4. A voice token for the same room through comms with the worlds policy.
    let adapter_meta =
        json!({ "signer": "dcl:explorer", "realmName": world, "sceneId": world }).to_string();
    let (status, minted) = http
        .signed_over(
            &player,
            "post",
            "/comms-gatekeeper/get-scene-adapter",
            "/get-scene-adapter",
            &adapter_meta,
            Some(json!({})),
        )
        .await;
    assert_eq!(status, 200, "{minted}");
    let player_jwt = jwt_from_adapter(minted["adapter"].as_str().unwrap());
    let claims = jwt_payload(&player_jwt);
    let room = format!("world-{world}-{entity_id}");
    assert_eq!(claims["video"]["room"], room, "{claims}");
    assert_eq!(claims["sub"], player.address());
    assert!(
        verify_hs256(&player_jwt, &rig.livekit_secret),
        "the token is signed with the configured LiveKit secret"
    );
    eprintln!("[4] comms minted a LiveKit token for {room}");

    // 5. Revocation: the owner restricts access to an allow-list without the
    //    player; the mint is refused and the running socket is closed at once
    //    by the worlds core's revocation hook (the sweep keeps its default
    //    30 s interval here, so a close within 10 s proves the hook); the
    //    owner stays.
    let access_meta =
        json!({ "signer": "dcl:explorer", "type": "allow-list", "wallets": [] }).to_string();
    let (status, body) = http
        .signed(
            &owner,
            "post",
            &format!("/worlds-content-server/world/{world}/permissions/access"),
            &access_meta,
            None,
        )
        .await;
    assert_eq!(status, 204, "{body}");
    let (status, refused) = http
        .signed_over(
            &player,
            "post",
            "/comms-gatekeeper/get-scene-adapter",
            "/get-scene-adapter",
            &adapter_meta,
            Some(json!({})),
        )
        .await;
    assert_eq!(
        status, 401,
        "the revoked player must not get a token: {refused}"
    );
    assert!(
        closed(&mut player_socket.ws, Duration::from_secs(10)).await,
        "the revoked player's running socket must close (the hook, not the 30 s sweep)"
    );
    assert!(
        join_scene(&rig, &world, &player).await.is_none(),
        "and cannot come back"
    );
    assert!(
        !closed(&mut owner_socket.ws, Duration::from_secs(2)).await,
        "the owner's socket stays open"
    );
    let (status, _) = http
        .signed(
            &owner,
            "put",
            &format!(
                "/worlds-content-server/world/{world}/permissions/access/{}",
                player.address()
            ),
            METADATA,
            None,
        )
        .await;
    assert_eq!(status, 204);
    let (status, _) = http
        .signed_over(
            &player,
            "post",
            "/comms-gatekeeper/get-scene-adapter",
            "/get-scene-adapter",
            &adapter_meta,
            Some(json!({})),
        )
        .await;
    assert_eq!(status, 200, "allow-listed again: the mint is back");
    let player_socket = join_scene(&rig, &world, &player)
        .await
        .expect("and so is the room");
    eprintln!(
        "[5] revocation refused the mint and closed the running socket; the allow-list readmits"
    );

    // 6. Player and world storage values, written by the owner (the scene's
    //    authority) for the player's scope, survive a kill -9 and restart.
    let storage_meta = json!({ "signer": "dcl:explorer", "realmName": world, "parcel": "0,0", "sceneId": entity_id }).to_string();
    let player_key = format!("/world-storage/players/{}/values/score", player.address());
    let (status, stored) = http
        .signed(
            &owner,
            "put",
            &player_key,
            &storage_meta,
            Some(json!({ "value": { "score": 42, "level": "b" } })),
        )
        .await;
    assert_eq!(status, 200, "{stored}");
    let (status, stored) = http
        .signed(
            &owner,
            "put",
            "/world-storage/values/greeting",
            &storage_meta,
            Some(json!({ "value": "hello" })),
        )
        .await;
    assert_eq!(status, 200, "{stored}");
    let (status, denied) = http
        .signed(
            &player,
            "get",
            "/world-storage/values/greeting",
            &storage_meta,
            None,
        )
        .await;
    assert_eq!(
        status, 401,
        "a visitor holds no storage capability: {denied}"
    );
    drop(owner_socket);
    drop(player_socket);
    node.kill_9();
    let mut node = rig.start_node().await;
    let (status, read) = http
        .signed(&owner, "get", &player_key, &storage_meta, None)
        .await;
    assert_eq!(
        (status, &read["value"]),
        (200, &json!({ "score": 42, "level": "b" })),
        "{read}"
    );
    let (status, read) = http
        .signed(
            &owner,
            "get",
            "/world-storage/values/greeting",
            &storage_meta,
            None,
        )
        .await;
    assert_eq!((status, &read["value"]), (200, &json!("hello")), "{read}");
    let (status, host_status) = http.get("/scene-state/status").await;
    assert_eq!(
        (status, host_status["loadedScenes"].as_array().map(Vec::len)),
        (200, Some(0)),
        "nothing persisted for the host: it reloads on contact"
    );
    let player_socket = join_scene(&rig, &world, &player)
        .await
        .expect("the world reloads from the rows and the blob store after the restart");
    drop(player_socket);
    eprintln!(
        "[6] storage values survived kill -9 + restart; the host reloaded the scene on contact"
    );

    // 7. catalyst pulse in its own process admits legacy signed handshakes.
    let metrics_port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let mut pulse = Proc::spawn(
        "pulse",
        &rig.bin,
        "pulse",
        &rig.pulse_env(metrics_port),
        &root.join("pulse.log"),
    );
    wait_http(
        &format!("http://127.0.0.1:{metrics_port}/metrics"),
        Duration::from_secs(30),
    )
    .await;
    let pulse_addr: SocketAddr = format!("127.0.0.1:{}", rig.pulse_port).parse().unwrap();
    let mut owner_pulse = PulseClient::connect(pulse_addr).await;
    owner_pulse.handshake(&owner, (40, -20)).await;
    let mut player_pulse = PulseClient::connect(pulse_addr).await;
    player_pulse.handshake(&player, (40, -20)).await;
    eprintln!("[7] pulse: two legacy clients admitted with signed timestamp authentication");

    // 8. Pulse's cluster feed reaches archipelago over NATS (engine.islands,
    //    engine.discovery); the same wallet heartbeating into archipelago's
    //    socket shows on /hot-scenes over a seeded Genesis scene at 40,-20.
    sqlx::query(
        "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain) \
         VALUES ('0x0000000000000000000000000000000000000001', 'v3', 'scene', $1, $2, timezone('UTC', now()), $3, timezone('UTC', now()), '[]'::json)",
    )
    .bind("bafkreiacceptancegenesis")
    .bind(sqlx::types::Json(json!({ "display": { "title": "genesis plaza stub" }, "scene": { "base": "40,-20", "parcels": ["40,-20"] } })))
    .bind(vec!["40,-20".to_string()])
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO content.active_pointers (pointer, entity_id, entity_type) VALUES ('40,-20', 'bafkreiacceptancegenesis', 'scene')")
        .execute(&pool)
        .await
        .unwrap();
    let _heartbeat_socket = archipelago_heartbeat(&rig, &player, [648.0, 0.0, -312.0]).await;
    wait_until(
        "/hot-scenes lists the heartbeating wallet's scene",
        Duration::from_secs(15),
        || async {
            let (status, hot) = http.get("/archipelago-ea-stats/hot-scenes").await;
            status == 200
                && hot.as_array().is_some_and(|scenes| {
                    scenes
                        .iter()
                        .any(|s| s["id"] == "bafkreiacceptancegenesis" && s["usersTotalCount"] == 1)
                })
        },
    )
    .await;
    let (status, peers) = http
        .get(&format!(
            "/archipelago-ea-stats/comms/peers?id={}",
            player.address()
        ))
        .await;
    assert_eq!(status, 200);
    assert_eq!(peers["peers"][0]["parcel"], json!([40, -20]), "{peers}");
    if broker.is_some() {
        // The Pulse sockets stay serviced while the cluster pass publishes.
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            player_pulse.pump(Duration::from_millis(200)).await;
            owner_pulse.pump(Duration::from_millis(200)).await;
            let (status, islands) = http.get("/archipelago-ea-stats/islands").await;
            let listed = status == 200
                && islands["islands"].as_array().is_some_and(|islands| {
                    islands.iter().any(|i| {
                        i["peers"].as_array().is_some_and(|p| {
                            p.iter().any(|peer| peer["address"] == player.address())
                        })
                    })
                });
            if listed {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "archipelago did not show Pulse's island over NATS within 20 s: {islands}"
            );
        }
        let (status, health) = http.get("/archipelago-ea-stats/stats/health").await;
        assert_eq!(status, 200);
        assert_eq!(health["feed_connected"], true, "{health}");
        assert!(
            health["islands_total"].as_u64().unwrap_or(0) >= 1,
            "{health}"
        );
        let (status, core) = http.get("/archipelago-ea-stats/core-status").await;
        assert_eq!((status, &core["healthy"]), (200, &json!(true)), "{core}");
        eprintln!(
            "[8] archipelago: /hot-scenes reflects the heartbeat; Pulse's island reached /islands over NATS ({health})"
        );
    } else {
        eprintln!(
            "[8] archipelago: /hot-scenes reflects the heartbeat; the NATS feed was not exercised (no nats-server binary)"
        );
    }

    // 9. Stop every process this test started.
    pulse.kill_9();
    assert!(!pulse.alive());
    node.kill_9();
    assert!(!node.alive());
    drop(broker);
    eprintln!("[9] pulse, serve and the broker stopped");
}
