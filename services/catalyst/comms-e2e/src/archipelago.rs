//! The archipelago bot: the challenge handshake, heartbeats, and what the connector told it.
//!
//! The bot has no policy of its own. It dials, signs what it is asked to sign, sends what a
//! scenario tells it to send and keeps a view of what the connector said. It never reconnects.
//! The handshake comes in two steps ([`ArchipelagoBot::begin`] up to the challenge,
//! [`ArchipelagoBot::finish`] from the signature to the welcome) so a scenario can interleave the
//! handshakes of two devices.
//!
//! A connection string carries an access token. The timeline redacts it; nothing here puts one in
//! an error message.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use base64::Engine as _;
use catalyst_comms::protocol::archipelago::Position;
use catalyst_comms::protocol::archipelago::archipelago::{
    ChallengeRequestMessage, ClientPacket, Heartbeat, KickedReason, ServerPacket,
    SignedChallengeMessage, client_packet, server_packet,
};
use futures::{SinkExt, StreamExt};
use parking_lot::Mutex;
use prost::Message as _;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use url::Url;

use crate::engine::timeline::redact_text;
use crate::engine::{Ctx, Timeline, expect_or};
use crate::identity::Identity;

/// The kick reason the connector sends the older of two sessions of one device or wallet.
pub const KICKED_NEW_SESSION: i32 = KickedReason::KrNewSession as i32;

/// The sub-protocol name the connector is dialled with.
const SUB_PROTOCOL: &str = "archipelago";

const DEFAULT_DEADLINE: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Default)]
pub struct ArchipelagoOptions {
    /// Dial this address instead of the one in the URL: a fault link in front of the connector.
    pub via: Option<SocketAddr>,
    /// How long the transport and each handshake step may take. Absent: 10 s.
    pub deadline: Option<Duration>,
}

/// One island assignment as the connector sent it.
#[derive(Clone)]
pub struct Assignment {
    pub island: String,
    /// `livekit:<signalling url>?access_token=<jwt>`: carries a token, never print it.
    pub connection_string: String,
    /// Milliseconds since the scenario started.
    pub at_ms: u64,
}

impl std::fmt::Debug for Assignment {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Assignment")
            .field("island", &self.island)
            .field("connection_string", &redact_text(&self.connection_string))
            .field("at_ms", &self.at_ms)
            .finish()
    }
}

/// What one archipelago socket has been told.
#[derive(Debug, Clone, Default)]
pub struct ArchipelagoView {
    pub welcomed: bool,
    /// Whether the connector said, with its challenge, that the wallet already had a socket.
    pub already_connected: Option<bool>,
    /// Every island assignment, in the order received.
    pub assignments: Vec<Assignment>,
    /// The reason of a kick (`KICKED_NEW_SESSION` is the only one defined).
    pub kicked: Option<i32>,
    /// Control errors and frames that did not decode, as text without any token.
    pub errors: Vec<String>,
    /// The socket ended, from either side.
    pub closed: bool,
}

/// How a handshake ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchipelagoOutcome {
    Welcomed,
    /// The connector sent a kick instead of a welcome (a banned wallet gets this).
    Kicked(i32),
    /// The connector closed the socket without a welcome.
    Closed,
}

enum Command {
    Send(Vec<u8>),
    Close,
}

struct Shared {
    name: String,
    timeline: Timeline,
    view: Mutex<ArchipelagoView>,
    challenge: Mutex<Option<String>>,
}

impl Shared {
    fn record(&self, kind: &str, detail: impl serde::Serialize) {
        self.timeline.record(&self.name, kind, detail);
    }

    fn on_packet(&self, packet: ServerPacket) {
        let Some(message) = packet.message else {
            self.view
                .lock()
                .errors
                .push("an empty server packet".to_owned());
            self.record("empty packet", ());
            return;
        };
        match message {
            server_packet::Message::ChallengeResponse(response) => {
                self.record(
                    "challenge",
                    serde_json::json!({
                        "already_connected": response.already_connected,
                        "length": response.challenge_to_sign.len(),
                    }),
                );
                self.view.lock().already_connected = Some(response.already_connected);
                *self.challenge.lock() = Some(response.challenge_to_sign);
            }
            server_packet::Message::Welcome(welcome) => {
                self.record("welcome", serde_json::json!({ "peer_id": welcome.peer_id }));
                self.view.lock().welcomed = true;
            }
            server_packet::Message::IslandChanged(change) => {
                let at_ms = self.timeline.elapsed_ms();
                self.record(
                    "assignment",
                    serde_json::json!({
                        "island": change.island_id,
                        "from": change.from_island_id,
                        "peers": change.peers.len(),
                        "connection_string": change.conn_str,
                    }),
                );
                self.view.lock().assignments.push(Assignment {
                    island: change.island_id,
                    connection_string: change.conn_str,
                    at_ms,
                });
            }
            server_packet::Message::Kicked(kicked) => {
                self.record("kicked", kicked.reason);
                self.view.lock().kicked = Some(kicked.reason);
            }
            server_packet::Message::Error(error) => {
                let text = format!("control error {} (retry {})", error.code, error.retry);
                self.record("control error", &text);
                self.view.lock().errors.push(text);
            }
            server_packet::Message::LeftIsland(_)
            | server_packet::Message::JoinIsland(_)
            | server_packet::Message::Ack(_) => {}
        }
    }

    fn on_closed(&self, why: &str) {
        self.record("closed", why);
        self.view.lock().closed = true;
    }
}

pub struct ArchipelagoBot {
    shared: Arc<Shared>,
    commands: UnboundedSender<Command>,
    deadline: Duration,
}

/// The URL a bot dials for an API base: `http` becomes `ws`, `https` becomes `wss`, the path is
/// `/ws`.
pub fn socket_url(api: &Url) -> Result<Url> {
    let mut url = api.clone();
    let scheme = match api.scheme() {
        "http" | "ws" => "ws",
        "https" | "wss" => "wss",
        other => {
            bail!("the API address has the scheme {other}, which is not an HTTP or WebSocket one")
        }
    };
    url.set_scheme(scheme)
        .map_err(|()| anyhow!("the scheme of {api} cannot be changed"))?;
    url.set_path("/ws");
    url.set_query(None);
    Ok(url)
}

impl ArchipelagoBot {
    /// Dials the connector at the API address of the stack file, handshakes and waits for the
    /// welcome.
    pub async fn open(ctx: &Ctx, name: &str, identity: &Identity) -> Result<ArchipelagoBot> {
        ArchipelagoBot::open_with(ctx, name, identity, ArchipelagoOptions::default()).await
    }

    /// [`Self::open`] through `options`.
    pub async fn open_with(
        ctx: &Ctx,
        name: &str,
        identity: &Identity,
        options: ArchipelagoOptions,
    ) -> Result<ArchipelagoBot> {
        let url = api_socket_url(ctx)?;
        ArchipelagoBot::connect(ctx, name, &url, identity, options).await
    }

    /// [`Self::begin`] and [`Self::finish`]: fails unless the connector welcomes.
    pub async fn connect(
        ctx: &Ctx,
        name: &str,
        url: &Url,
        identity: &Identity,
        options: ArchipelagoOptions,
    ) -> Result<ArchipelagoBot> {
        let bot = ArchipelagoBot::begin(ctx, name, url, identity, options).await?;
        bot.finish(ctx, identity).await?;
        Ok(bot)
    }

    /// Dials, asks for a challenge and waits until it arrives. The wallet is the identity's.
    pub async fn begin(
        ctx: &Ctx,
        name: &str,
        url: &Url,
        identity: &Identity,
        options: ArchipelagoOptions,
    ) -> Result<ArchipelagoBot> {
        let deadline = options.deadline.unwrap_or(DEFAULT_DEADLINE);
        let mut dialled = url.clone();
        if let Some(via) = options.via {
            dialled
                .set_ip_host(via.ip())
                .map_err(|()| anyhow!("{via} cannot be the host of {url}"))?;
            dialled
                .set_port(Some(via.port()))
                .map_err(|()| anyhow!("{via} cannot be the port of {url}"))?;
        }
        let mut request = dialled
            .as_str()
            .into_client_request()
            .with_context(|| format!("{name}: {dialled} is not a WebSocket address"))?;
        request.headers_mut().append(
            "Sec-WebSocket-Protocol",
            SUB_PROTOCOL.parse().expect("a header value"),
        );
        let shared = Arc::new(Shared {
            name: name.to_owned(),
            timeline: ctx.timeline().clone(),
            view: Mutex::new(ArchipelagoView::default()),
            challenge: Mutex::new(None),
        });
        shared.record("dialing", dialled.as_str());
        let (stream, _) = tokio::time::timeout(deadline, tokio_tungstenite::connect_async(request))
            .await
            .map_err(|_| {
                anyhow!("{name}: the connector at {dialled} did not accept the socket in time")
            })?
            .with_context(|| format!("{name}: dialing {dialled}"))?;
        shared.record("transport connected", ());
        let (commands, receiver) = unbounded_channel();
        tokio::spawn(run(stream, shared.clone(), receiver));
        let bot = ArchipelagoBot {
            shared,
            commands,
            deadline,
        };
        bot.shared.record(
            "challenge request sent",
            serde_json::json!({ "address": identity.address() }),
        );
        bot.send_packet(client_packet::Message::ChallengeRequest(
            ChallengeRequestMessage {
                address: identity.address(),
            },
        ));
        expect_or(
            ctx,
            &format!("{name} receives a challenge"),
            deadline,
            || {
                if bot.shared.challenge.lock().is_some() {
                    return Ok(());
                }
                let view = bot.view();
                match (view.kicked, view.closed) {
                    (Some(reason), _) => {
                        Err(format!("fatal: the connector kicked it ({reason}) instead"))
                    }
                    (None, true) => {
                        Err("fatal: the connector closed the socket before a challenge".to_owned())
                    }
                    (None, false) => Err("no challenge yet".to_owned()),
                }
            },
        )
        .await?;
        Ok(bot)
    }

    /// Signs the challenge, sends it and waits for the connector's answer. Fails if the answer is
    /// anything but a welcome; [`Self::finish_outcome`] says which answer instead.
    pub async fn finish(&self, ctx: &Ctx, identity: &Identity) -> Result<()> {
        match self.finish_outcome(ctx, identity).await? {
            ArchipelagoOutcome::Welcomed => Ok(()),
            ArchipelagoOutcome::Kicked(reason) => {
                bail!(
                    "{}: the connector kicked it ({reason}) instead of welcoming it",
                    self.name()
                )
            }
            ArchipelagoOutcome::Closed => {
                bail!(
                    "{}: the connector closed the socket instead of welcoming it",
                    self.name()
                )
            }
        }
    }

    /// [`Self::finish`] that reports a kick or a close as an outcome. It still fails for a
    /// challenge that does not start with `dcl-` (nothing is signed then) and when no answer
    /// comes in time.
    pub async fn finish_outcome(
        &self,
        ctx: &Ctx,
        identity: &Identity,
    ) -> Result<ArchipelagoOutcome> {
        let name = self.name().to_owned();
        let challenge = self
            .shared
            .challenge
            .lock()
            .clone()
            .ok_or_else(|| anyhow!("{name}: finish was called before a challenge arrived"))?;
        if !challenge.starts_with("dcl-") {
            self.shared
                .record("challenge refused", "it does not start with dcl-");
            bail!("{name}: the challenge does not start with `dcl-`; nothing was signed");
        }
        self.shared.record(
            "signed challenge sent",
            serde_json::json!({ "session": identity.session() }),
        );
        self.send_packet(client_packet::Message::SignedChallenge(
            SignedChallengeMessage {
                auth_chain_json: identity.archipelago_chain(&challenge),
            },
        ));
        expect_or(
            ctx,
            &format!("{name} gets an answer to its signed challenge"),
            self.deadline,
            || {
                let view = self.view();
                if view.welcomed {
                    Ok(ArchipelagoOutcome::Welcomed)
                } else if let Some(reason) = view.kicked {
                    Ok(ArchipelagoOutcome::Kicked(reason))
                } else if view.closed {
                    Ok(ArchipelagoOutcome::Closed)
                } else {
                    Err("no answer yet".to_owned())
                }
            },
        )
        .await
    }

    pub fn name(&self) -> &str {
        &self.shared.name
    }

    /// One position heartbeat, in world coordinates, for the default realm.
    pub fn heartbeat(&self, world: [f32; 3]) {
        self.heartbeat_in(None, world);
    }

    /// One position heartbeat asking for the room `realm`.
    pub fn heartbeat_in(&self, realm: Option<&str>, world: [f32; 3]) {
        self.shared.record(
            "heartbeat sent",
            serde_json::json!({ "world": world, "realm": realm }),
        );
        self.send_packet(client_packet::Message::Heartbeat(Heartbeat {
            position: Some(Position {
                x: world[0],
                y: world[1],
                z: world[2],
            }),
            desired_room: realm.map(str::to_owned),
            sample_sequence: None,
        }));
    }

    /// Sends bytes as they are: a frame that is not a packet at all.
    pub fn send_raw(&self, bytes: Vec<u8>) {
        let _ = self.commands.send(Command::Send(bytes));
    }

    fn send_packet(&self, message: client_packet::Message) {
        self.send_raw(
            ClientPacket {
                request_sequence: 0,
                message: Some(message),
            }
            .encode_to_vec(),
        );
    }

    pub fn view(&self) -> ArchipelagoView {
        self.shared.view.lock().clone()
    }

    /// The assignments received so far, oldest first.
    pub fn assignments(&self) -> Vec<Assignment> {
        self.shared.view.lock().assignments.clone()
    }

    /// Fails with a fatal reason once the socket has ended or the connector kicked this bot, so a
    /// wait that asked ends at once.
    pub fn alive(&self) -> Result<(), String> {
        let view = self.shared.view.lock();
        if let Some(reason) = view.kicked {
            return Err(format!("fatal: the connector kicked it ({reason})"));
        }
        if view.closed {
            return Err("fatal: the connector closed the socket".to_owned());
        }
        Ok(())
    }

    /// Ends the socket now.
    pub fn close(self) {
        self.shared.record("close requested", ());
        // the socket task also ends, with the same close frame, when the bot is dropped
        let _ = self.commands.send(Command::Close);
    }
}

fn api_socket_url(ctx: &Ctx) -> Result<Url> {
    let api = ctx
        .stack()
        .api
        .as_ref()
        .ok_or_else(|| anyhow!("the stack file has no API address (`api`)"))?;
    socket_url(api)
}

type Stream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Runs until the socket ends or the bot is dropped.
async fn run(
    stream: Stream,
    shared: Arc<Shared>,
    mut commands: tokio::sync::mpsc::UnboundedReceiver<Command>,
) {
    let (mut write, mut read) = stream.split();
    loop {
        tokio::select! {
            frame = read.next() => match frame {
                Some(Ok(Message::Binary(bytes))) => match ServerPacket::decode(bytes.as_ref()) {
                    Ok(packet) => shared.on_packet(packet),
                    Err(_) => {
                        shared.view.lock().errors.push("a frame that is not a server packet".to_owned());
                        shared.record("undecodable frame", bytes.len());
                    }
                },
                Some(Ok(Message::Close(frame))) => {
                    shared.on_closed(&match frame {
                        Some(frame) => format!("the connector closed the socket: {} {}", frame.code, frame.reason),
                        None => "the connector closed the socket".to_owned(),
                    });
                    return;
                }
                Some(Ok(_)) => {}
                Some(Err(error)) => {
                    shared.on_closed(&format!("the socket failed: {error}"));
                    return;
                }
                None => {
                    shared.on_closed("the socket ended");
                    return;
                }
            },
            command = commands.recv() => match command {
                Some(Command::Send(bytes)) => {
                    if let Err(error) = write.send(Message::Binary(bytes.into())).await {
                        shared.on_closed(&format!("sending failed: {error}"));
                        return;
                    }
                }
                Some(Command::Close) | None => {
                    let _ = write.send(Message::Close(None)).await;
                    shared.on_closed("closed by the bot");
                    return;
                }
            },
        }
    }
}

/// A connection string taken apart.
#[derive(Clone, PartialEq, Eq)]
pub struct ConnectionString {
    /// The LiveKit signalling address, as the string names it (the stack advertises a front
    /// address nothing may listen on).
    pub livekit_url: String,
    pub claims: TokenClaims,
    token: String,
}

impl ConnectionString {
    /// The access token. Pass it on, never print it.
    pub fn token(&self) -> &str {
        &self.token
    }
}

impl std::fmt::Debug for ConnectionString {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ConnectionString")
            .field("livekit_url", &self.livekit_url)
            .field("claims", &self.claims)
            .finish_non_exhaustive()
    }
}

/// What a LiveKit access token claims, read without checking the signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenClaims {
    pub room: String,
    /// The `sub` claim: the identity the token joins as (the wallet).
    pub identity: String,
    /// Unix seconds.
    pub expires_at: u64,
    pub not_before: Option<u64>,
    pub can_publish: bool,
    pub can_subscribe: bool,
}

/// Reads `livekit:<signalling url>?access_token=<jwt>`: the URL and the claims of the token. The
/// signature is not checked: a scenario asks who the token is for, the room service decides
/// whether it is valid.
pub fn read_connection_string(connection_string: &str) -> Result<ConnectionString> {
    let rest = connection_string
        .strip_prefix("livekit:")
        .ok_or_else(|| anyhow!("a connection string starts with `livekit:`"))?;
    let (url, query) = rest
        .split_once('?')
        .ok_or_else(|| anyhow!("the connection string carries no query"))?;
    let token = query
        .split('&')
        .find_map(|pair| pair.strip_prefix("access_token="))
        .ok_or_else(|| anyhow!("the connection string carries no access token"))?;
    Ok(ConnectionString {
        livekit_url: url.to_owned(),
        claims: read_token_claims(token)?,
        token: token.to_owned(),
    })
}

/// The claims of a JWT, signature unchecked.
pub fn read_token_claims(token: &str) -> Result<TokenClaims> {
    let mut parts = token.split('.');
    let (Some(_header), Some(payload), Some(_signature), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        bail!("the access token does not have three parts");
    };
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .map_err(|_| anyhow!("the access token payload is not base64"))?;
    let claims: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| anyhow!("the access token payload is not JSON"))?;
    let video = claims.get("video");
    let flag = |key: &str| {
        video
            .and_then(|video| video.get(key))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
    };
    Ok(TokenClaims {
        room: video
            .and_then(|video| video.get("room"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("the access token names no room"))?
            .to_owned(),
        identity: claims
            .get("sub")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("the access token names no identity (`sub`)"))?
            .to_owned(),
        expires_at: claims
            .get("exp")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| anyhow!("the access token has no expiry (`exp`)"))?,
        not_before: claims.get("nbf").and_then(serde_json::Value::as_u64),
        can_publish: flag("canPublish"),
        can_subscribe: flag("canSubscribe"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dcl_livekit_token::{AccessToken, join_grants};

    fn connection_string(room: &str, identity: &str) -> String {
        let token = AccessToken::new("key", "secret", identity, join_grants(room))
            .to_jwt()
            .unwrap();
        format!("livekit:ws://127.0.0.1:6214?access_token={token}")
    }

    #[test]
    fn a_connection_string_gives_its_room_identity_and_url() {
        let text = connection_string("island-7", "0xabc");
        let parsed = read_connection_string(&text).unwrap();
        assert_eq!(parsed.livekit_url, "ws://127.0.0.1:6214");
        assert_eq!(parsed.claims.room, "island-7");
        assert_eq!(parsed.claims.identity, "0xabc");
        assert!(parsed.claims.can_publish && parsed.claims.can_subscribe);
        assert!(parsed.claims.expires_at > crate::identity::now_ms() as u64 / 1000);
        assert!(parsed.claims.not_before.is_some());
    }

    #[test]
    fn a_connection_string_does_not_print_its_token() {
        let text = connection_string("island-7", "0xabc");
        let parsed = read_connection_string(&text).unwrap();
        let token = parsed.token().to_owned();
        assert!(!format!("{parsed:?}").contains(&token));
        let assignment = Assignment {
            island: "i".into(),
            connection_string: text,
            at_ms: 1,
        };
        assert!(!format!("{assignment:?}").contains(&token));
        assert!(format!("{assignment:?}").contains("<redacted>"));
    }

    #[test]
    fn a_malformed_connection_string_is_an_error_that_names_the_part() {
        let message = |text: &str| read_connection_string(text).unwrap_err().to_string();
        assert!(message("wss://host").contains("livekit:"));
        assert!(message("livekit:ws://host").contains("no query"));
        assert!(message("livekit:ws://host?x=1").contains("no access token"));
        assert!(message("livekit:ws://host?access_token=abc").contains("three parts"));
        assert!(message("livekit:ws://host?access_token=a.!!.c").contains("base64"));
        // a token without a room is refused, and the message holds no token
        let no_room = format!(
            "livekit:ws://host?access_token=a.{}.c",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"sub":"x","exp":1}"#)
        );
        let text = message(&no_room);
        assert!(text.contains("no room"), "{text}");
    }

    #[test]
    fn the_socket_url_follows_the_api_scheme() {
        let url = |text: &str| socket_url(&Url::parse(text).unwrap()).unwrap().to_string();
        assert_eq!(url("http://127.0.0.1:5210"), "ws://127.0.0.1:5210/ws");
        assert_eq!(url("https://example.com/base?x=1"), "wss://example.com/ws");
        assert!(socket_url(&Url::parse("ftp://host").unwrap()).is_err());
    }
}
