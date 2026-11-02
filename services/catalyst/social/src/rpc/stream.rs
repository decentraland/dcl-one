//! `/v1/stream` (upstream #503, `docs/websocket-stream-protocol.md` at
//! `b6b2cef`): the six subscription streams of the RPC socket over a plain
//! WebSocket of JSON text frames, served next to it at
//! `/social-rpc/v1/stream` (upstream's uWS host serves the RPC socket at `/`
//! and this one at `/v1/stream`).
//!
//! The handshake is the RPC socket's signed fetch, signed for `get` and
//! `/v1/stream` (the served path, or an `x-original-path` ending in it, too;
//! never `/`, so a chain captured for one socket cannot open the other):
//! either as `x-identity-*` headers of the upgrade request, verified before
//! the upgrade (`401` refused, `503` when the verification could not run), or
//! as the first frame, with `auth_in_progress` for any frame sent while it is
//! verified, `3003` when refused and `3004` when none arrives in the auth
//! window. A ready socket gets `{"type":"ready"}`, then answers `subscribe`,
//! `unsubscribe` and `ping`.
//!
//! Each stream reads the same per-address channel (`pubsub`) as its RPC
//! procedure, with the same filters and delivery guards, and maps its events
//! to the protocol's JSON: addresses only, never profiles. Both protocols
//! share the per-address socket cap and presence (`limits::Admission`,
//! `Context::mark_online`): the address's last socket of either kind ending
//! ends its private call.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::State as AxumState;
use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use serde_json::{Map, Value, json};
use tokio::sync::{Notify, broadcast::error::RecvError, mpsc};
use tokio::task::JoinHandle;

use crate::rpc::State;
use crate::rpc::context::Context;
use crate::rpc::limits::Eviction;
use crate::rpc::proto::v2::{
    CommunityVoiceChatStatus, ConnectivityStatus, PrivateVoiceChatStatus, friendship_update,
};
use crate::rpc::pubsub::SocialEvent;
use crate::shared::rpc::handshake::{
    HandshakeError, handshake_path, validator_ref, verify_handshake,
};

/// The route upstream signs and serves (`src/adapters/stream-sessions/constants.ts`).
pub const STREAM_PATH: &str = "/v1/stream";
/// `STREAM_WS_MAX_PAYLOAD_LENGTH`: enough for a signed-fetch auth chain.
const MAX_PAYLOAD_BYTES: usize = 16 * 1024;
/// `STREAM_WS_MESSAGES_PER_SECOND` / `STREAM_WS_MESSAGE_BURST`: every
/// application frame, the auth frame included, draws from it.
const MESSAGES_PER_SECOND: f64 = 20.0;
const MESSAGE_BURST: f64 = 40.0;
/// Events queued per subscription before the socket is closed `1008`.
const SUBSCRIPTION_QUEUE: usize = 1_000;
/// A subscription or unsubscription id: 1 to 64 UTF-16 units, as JS counts.
const MAX_ID_UNITS: usize = 64;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Stream {
    Friendships,
    FriendConnectivity,
    Blocks,
    PrivateVoiceChats,
    CommunityMemberConnectivity,
    CommunityVoiceChats,
}

impl Stream {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "friendship_updates" => Self::Friendships,
            "friend_connectivity_updates" => Self::FriendConnectivity,
            "block_updates" => Self::Blocks,
            "private_voice_chat_updates" => Self::PrivateVoiceChats,
            "community_member_connectivity_updates" => Self::CommunityMemberConnectivity,
            "community_voice_chat_updates" => Self::CommunityVoiceChats,
            _ => return None,
        })
    }

    fn name(self) -> &'static str {
        match self {
            Self::Friendships => "friendship_updates",
            Self::FriendConnectivity => "friend_connectivity_updates",
            Self::Blocks => "block_updates",
            Self::PrivateVoiceChats => "private_voice_chat_updates",
            Self::CommunityMemberConnectivity => "community_member_connectivity_updates",
            Self::CommunityVoiceChats => "community_voice_chat_updates",
        }
    }
}

/// The candidate paths of the handshake signature: the route, the path this
/// node serves it at and a proxy's `x-original-path` ending with the route.
fn stream_paths(headers: &HeaderMap, served: &str) -> Vec<String> {
    let mut paths = vec![handshake_path(headers, STREAM_PATH)];
    for p in [STREAM_PATH, served] {
        if !paths.iter().any(|q| q == p) {
            paths.push(p.to_string());
        }
    }
    paths
}

/// The upgrade request's `x-identity-*` headers as the first frame's JSON
/// object, so both handshakes verify through the same function.
fn header_frame(headers: &HeaderMap) -> Option<String> {
    let mut frame = Map::new();
    for (name, value) in headers {
        if name.as_str().starts_with("x-identity-") {
            frame.insert(
                name.as_str().to_string(),
                Value::String(String::from_utf8_lossy(value.as_bytes()).into_owned()),
            );
        }
    }
    (!frame.is_empty()).then(|| Value::Object(frame).to_string())
}

async fn verify(state: &State, paths: &[String], frame: &str) -> Result<String, HandshakeError> {
    verify_handshake(
        frame,
        "get",
        paths,
        state.settings.auth_window_secs,
        chrono::Utc::now().timestamp(),
        validator_ref(&state.eip1654),
    )
    .await
    .map(|address| address.into_string())
}

pub async fn ws_upgrade(
    AxumState(state): AxumState<Arc<State>>,
    headers: HeaderMap,
    uri: Uri,
    ws: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Response {
    let ws = match ws {
        Ok(ws) => ws,
        Err(_) if !headers.contains_key(axum::http::header::SEC_WEBSOCKET_KEY) => {
            return super::not_found().await;
        }
        Err(rejection) => return rejection.into_response(),
    };
    let paths = stream_paths(&headers, uri.path());
    let verified = match header_frame(&headers) {
        None => None,
        Some(frame) => match verify(&state, &paths, &frame).await {
            Ok(address) => Some(address),
            Err(HandshakeError::CatalystUnavailable(e)) => {
                tracing::warn!(error = %e, "stream upgrade: the chain could not be verified");
                return StatusCode::SERVICE_UNAVAILABLE.into_response();
            }
            Err(e) => {
                tracing::info!(error = %e, "stream upgrade refused");
                return StatusCode::UNAUTHORIZED.into_response();
            }
        },
    };
    ws.max_message_size(MAX_PAYLOAD_BYTES)
        .on_upgrade(move |socket| async move {
            let mut session = Session::new(state, socket);
            session.run(paths, verified).await;
        })
        .into_response()
}

/// The per-connection token bucket of incoming application frames.
struct MessageBudget {
    tokens: f64,
    updated_at: Instant,
}

impl MessageBudget {
    fn take(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(self.updated_at).as_secs_f64();
        self.tokens = (self.tokens + elapsed * MESSAGES_PER_SECOND).min(MESSAGE_BURST);
        self.updated_at = now;
        if self.tokens < 1.0 {
            return false;
        }
        self.tokens -= 1.0;
        true
    }
}

/// What a subscription task hands the socket loop.
enum Outgoing {
    Event {
        serial: u64,
        data: Value,
    },
    /// The subscription's source finished (`reason: "completed"`).
    Completed {
        serial: u64,
    },
}

struct Subscription {
    id: String,
    stream: Stream,
    task: JoinHandle<()>,
}

/// How a socket ended, past the handshake.
enum Ending {
    Gone,
    Close(u16, &'static str),
}

struct Session {
    state: Arc<State>,
    socket: WebSocket,
    budget: MessageBudget,
    evicted: Arc<Notify>,
    overflow: Arc<Notify>,
    subscriptions: HashMap<u64, Subscription>,
    next_serial: u64,
}

impl Session {
    fn new(state: Arc<State>, socket: WebSocket) -> Self {
        Self {
            state,
            socket,
            budget: MessageBudget {
                tokens: MESSAGE_BURST,
                updated_at: Instant::now(),
            },
            evicted: Arc::new(Notify::new()),
            overflow: Arc::new(Notify::new()),
            subscriptions: HashMap::new(),
            next_serial: 0,
        }
    }

    fn ctx(&self) -> &Context {
        &self.state.ctx
    }

    async fn send(&mut self, message: Value) -> bool {
        self.socket
            .send(Message::Text(message.to_string().into()))
            .await
            .is_ok()
    }

    async fn close(&mut self, code: u16, reason: &'static str) {
        let _ = self
            .socket
            .send(Message::Close(Some(CloseFrame {
                code,
                reason: reason.into(),
            })))
            .await;
    }

    async fn error(&mut self, code: &str, message: &str, id: Option<&str>) -> bool {
        let mut error = json!({ "type": "error", "code": code, "message": message });
        if let Some(id) = id {
            error["id"] = json!(id);
        }
        self.send(error).await
    }

    async fn run(&mut self, paths: Vec<String>, verified: Option<String>) {
        if let Some(max) = self.state.settings.ws_max_concurrent_connections
            && self.ctx().live_connections() >= max
        {
            tracing::warn!(max, "rejecting stream connection: pool is full");
            self.close(1013, "Unable to acquire connection").await;
            return;
        }
        self.ctx().connection_opened();
        let address = match verified {
            Some(address) => Some(address),
            None => self.first_frame_handshake(&paths).await,
        };
        if let Some(address) = address {
            self.serve(&address).await;
        }
        self.ctx().connection_closed();
    }

    /// The fallback handshake: the first application frame is the chain.
    async fn first_frame_handshake(&mut self, paths: &[String]) -> Option<String> {
        let window = Duration::from_secs(self.state.settings.auth_window_secs.max(1) as u64);
        let deadline = tokio::time::sleep(window);
        tokio::pin!(deadline);
        let frame = loop {
            tokio::select! {
                _ = &mut deadline => {
                    self.close(3004, "Authorization timeout").await;
                    return None;
                }
                message = self.socket.recv() => match message {
                    Some(Ok(Message::Text(text))) => break text.to_string(),
                    Some(Ok(Message::Binary(bytes))) => break String::from_utf8_lossy(&bytes).into_owned(),
                    Some(Ok(Message::Ping(_) | Message::Pong(_))) => continue,
                    _ => return None,
                },
            }
        };
        if !self.budget.take() {
            self.close(1008, "Message rate exceeded; reconnect with backoff")
                .await;
            return None;
        }
        let state = self.state.clone();
        let verification = verify(&state, paths, &frame);
        tokio::pin!(verification);
        let verdict = loop {
            tokio::select! {
                verdict = &mut verification => break verdict,
                _ = &mut deadline => {
                    self.close(3004, "Authorization timeout").await;
                    return None;
                }
                message = self.socket.recv() => match message {
                    Some(Ok(Message::Text(_) | Message::Binary(_))) => {
                        if !self.budget.take() {
                            self.close(1008, "Message rate exceeded; reconnect with backoff").await;
                            return None;
                        }
                        self.error("auth_in_progress", "Authentication already in progress", None).await;
                    }
                    Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                    _ => return None,
                },
            }
        };
        match verdict {
            Ok(address) => Some(address),
            Err(e) => {
                tracing::info!(error = %e, "stream handshake failed");
                self.close(3003, "Unauthorized").await;
                None
            }
        }
    }

    /// An authenticated socket: its slot, presence and `ready`, then the
    /// message loop; on the way out every subscription ends, the slot is
    /// freed and the address's last socket ends its private call.
    async fn serve(&mut self, address: &str) {
        let ctx = self.ctx().clone();
        ctx.admission()
            .claim(address, Eviction::Stream(self.evicted.clone()));
        ctx.mark_online(address);
        let (tx, mut rx) = mpsc::channel::<Outgoing>(SUBSCRIPTION_QUEUE);
        let ending = if self.send(json!({ "type": "ready" })).await {
            self.message_loop(address, &tx, &mut rx).await
        } else {
            Ending::Gone
        };
        for (_, subscription) in self.subscriptions.drain() {
            subscription.task.abort();
        }
        if let Ending::Close(code, reason) = ending {
            self.close(code, reason).await;
        }
        ctx.admission().release(address, &self.evicted);
        if ctx.mark_offline(address) {
            let address = address.to_string();
            tokio::spawn(async move {
                ctx.end_private_voice_chat_on_disconnect(&address).await;
            });
        }
    }

    async fn message_loop(
        &mut self,
        address: &str,
        tx: &mpsc::Sender<Outgoing>,
        rx: &mut mpsc::Receiver<Outgoing>,
    ) -> Ending {
        let evicted = self.evicted.clone();
        let overflow = self.overflow.clone();
        loop {
            tokio::select! {
                _ = evicted.notified() => return Ending::Close(3005, "Replaced by a newer connection"),
                _ = overflow.notified() => {
                    return Ending::Close(1008, "Subscription overflow; reconnect and resync");
                }
                outgoing = rx.recv() => {
                    let Some(outgoing) = outgoing else { return Ending::Gone };
                    if !self.deliver(outgoing).await {
                        return Ending::Gone;
                    }
                }
                message = self.socket.recv() => {
                    let raw = match message {
                        Some(Ok(Message::Text(text))) => text.as_bytes().to_vec(),
                        Some(Ok(Message::Binary(bytes))) => bytes.to_vec(),
                        Some(Ok(Message::Ping(_) | Message::Pong(_))) => continue,
                        _ => return Ending::Gone,
                    };
                    if !self.budget.take() {
                        return Ending::Close(1008, "Message rate exceeded; reconnect with backoff");
                    }
                    if !self.handle(address, tx, &raw).await {
                        return Ending::Gone;
                    }
                }
            }
        }
    }

    /// An event or completion of a subscription still open; anything queued
    /// for one already ended is dropped, so nothing follows its `end`.
    async fn deliver(&mut self, outgoing: Outgoing) -> bool {
        match outgoing {
            Outgoing::Event { serial, data } => {
                let Some(subscription) = self.subscriptions.get(&serial) else {
                    return true;
                };
                let event = json!({
                    "type": "event",
                    "id": subscription.id,
                    "stream": subscription.stream.name(),
                    "data": data,
                });
                self.send(event).await
            }
            Outgoing::Completed { serial } => {
                let Some(subscription) = self.subscriptions.remove(&serial) else {
                    return true;
                };
                self.send(json!({ "type": "end", "id": subscription.id, "reason": "completed" }))
                    .await
            }
        }
    }

    async fn handle(&mut self, address: &str, tx: &mpsc::Sender<Outgoing>, raw: &[u8]) -> bool {
        match parse_message(raw) {
            Err(e) => self.error(e.code, e.message, e.id.as_deref()).await,
            Ok(ClientMessage::Ping) => self.send(json!({ "type": "pong" })).await,
            Ok(ClientMessage::Unsubscribe { id }) => {
                let serial = self
                    .subscriptions
                    .iter()
                    .find(|(_, s)| s.id == id)
                    .map(|(serial, _)| *serial);
                let Some(subscription) = serial.and_then(|s| self.subscriptions.remove(&s)) else {
                    return self
                        .error(
                            "unknown_subscription",
                            "No subscription with this id",
                            Some(&id),
                        )
                        .await;
                };
                subscription.task.abort();
                self.send(json!({ "type": "end", "id": id, "reason": "unsubscribed" }))
                    .await
            }
            Ok(ClientMessage::Subscribe { id, stream }) => {
                let Some(stream) = Stream::parse(&stream) else {
                    return self
                        .error("unknown_stream", "Unknown stream", Some(&id))
                        .await;
                };
                if self.subscriptions.values().any(|s| s.id == id) {
                    return self
                        .error(
                            "duplicate_subscription",
                            "A subscription with this id already exists",
                            Some(&id),
                        )
                        .await;
                }
                if self.subscriptions.values().any(|s| s.stream == stream) {
                    return self
                        .error(
                            "duplicate_subscription",
                            "Already subscribed to this stream on this connection",
                            Some(&id),
                        )
                        .await;
                }
                let admission = self.ctx().admission();
                // A socket a newer one displaced holds no slot; its close is under way.
                if !admission.holds(address, &self.evicted) {
                    return self
                        .error(
                            "stream_error",
                            "The subscription could not be started",
                            Some(&id),
                        )
                        .await;
                }
                if !admission.admit_subscription(address) {
                    return self
                        .error(
                            "rate_limited",
                            "Subscription rate exceeded; retry with backoff",
                            Some(&id),
                        )
                        .await;
                }
                self.next_serial += 1;
                let serial = self.next_serial;
                // Subscribed before `subscribed` is sent: nothing published after it is missed.
                let task = spawn_stream(
                    self.ctx().clone(),
                    address.to_string(),
                    stream,
                    serial,
                    tx.clone(),
                    self.overflow.clone(),
                );
                self.subscriptions.insert(
                    serial,
                    Subscription {
                        id: id.clone(),
                        stream,
                        task,
                    },
                );
                self.send(json!({ "type": "subscribed", "id": id })).await
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ClientMessage {
    Subscribe { id: String, stream: String },
    Unsubscribe { id: String },
    Ping,
}

#[derive(Debug, PartialEq, Eq)]
struct ProtocolError {
    code: &'static str,
    message: &'static str,
    id: Option<String>,
}

fn protocol_error(code: &'static str, message: &'static str, id: Option<String>) -> ProtocolError {
    ProtocolError { code, message, id }
}

fn valid_id(value: Option<&Value>) -> Option<String> {
    let id = value?.as_str()?;
    let units = id.encode_utf16().count();
    (1..=MAX_ID_UNITS).contains(&units).then(|| id.to_string())
}

/// Upstream `parseStreamMessage` (`src/adapters/stream-sessions/messages.ts`):
/// the untrusted frame validated once, an invalid id never echoed back.
fn parse_message(raw: &[u8]) -> Result<ClientMessage, ProtocolError> {
    let value: Value = serde_json::from_slice(raw)
        .map_err(|_| protocol_error("invalid_message", "Messages must be valid JSON", None))?;
    let Value::Object(object) = value else {
        return Err(protocol_error(
            "invalid_message",
            "Messages must be JSON objects",
            None,
        ));
    };
    match object.get("type").and_then(Value::as_str) {
        Some("ping") => Ok(ClientMessage::Ping),
        Some("subscribe") => {
            let id = valid_id(object.get("id")).ok_or_else(|| {
                protocol_error(
                    "invalid_message",
                    "id must be a string of 1 to 64 characters",
                    None,
                )
            })?;
            match object.get("stream").and_then(Value::as_str) {
                Some(stream) => Ok(ClientMessage::Subscribe {
                    id,
                    stream: stream.to_string(),
                }),
                None => Err(protocol_error("unknown_stream", "Unknown stream", Some(id))),
            }
        }
        Some("unsubscribe") => valid_id(object.get("id"))
            .map(|id| ClientMessage::Unsubscribe { id })
            .ok_or_else(|| {
                protocol_error("unknown_subscription", "No subscription with this id", None)
            }),
        _ => Err(protocol_error(
            "invalid_message",
            "Unknown message type",
            None,
        )),
    }
}

fn connectivity_name(status: i32) -> Option<&'static str> {
    Some(match ConnectivityStatus::try_from(status).ok()? {
        ConnectivityStatus::Online => "online",
        ConnectivityStatus::Offline => "offline",
        ConnectivityStatus::Away => "away",
    })
}

/// One subscription: the address's channel, taken before it returns, read
/// by a task that maps each event of `stream` to its JSON and queues it.
fn spawn_stream(
    ctx: Context,
    me: String,
    stream: Stream,
    serial: u64,
    tx: mpsc::Sender<Outgoing>,
    overflow: Arc<Notify>,
) -> JoinHandle<()> {
    let mut rx = ctx.pubsub().subscribe(&me);
    tokio::spawn(async move {
        let queue = |data: Value| match tx.try_send(Outgoing::Event { serial, data }) {
            Ok(()) => true,
            Err(mpsc::error::TrySendError::Full(_)) => {
                overflow.notify_one();
                false
            }
            Err(mpsc::error::TrySendError::Closed(_)) => false,
        };
        if stream == Stream::FriendConnectivity
            && !online_friends_snapshot(&ctx, &me, &queue).await
        {
            return;
        }
        loop {
            let event = match rx.recv().await {
                Ok(event) => event,
                Err(RecvError::Lagged(skipped)) => {
                    // Updates cannot be replayed: the client reconnects and resyncs.
                    tracing::warn!(skipped, "stream subscription lagged");
                    overflow.notify_one();
                    return;
                }
                Err(RecvError::Closed) => break,
            };
            if matches!(&*event, SocialEvent::ConnectivityResync)
                && matches!(
                    stream,
                    Stream::FriendConnectivity | Stream::CommunityMemberConnectivity
                )
            {
                break;
            }
            let Some(data) = event_data(&ctx, &me, stream, &event).await else {
                continue;
            };
            if !queue(data) {
                return;
            }
        }
        let _ = tx.send(Outgoing::Completed { serial }).await;
    })
}

/// One `online` event per friend online when the subscription starts, as
/// the RPC stream starts (upstream `getOnlineFriendAddresses`).
async fn online_friends_snapshot(ctx: &Context, me: &str, queue: &impl Fn(Value) -> bool) -> bool {
    let mut after = String::new();
    loop {
        let Ok(page) = ctx.db().connectivity_friends_page(me, &after, true).await else {
            return true;
        };
        let snapshot = ctx.topology().snapshot();
        for friend in &page {
            if snapshot.is_online(friend)
                && !queue(json!({ "address": friend, "status": "online" }))
            {
                return false;
            }
        }
        match page.last() {
            Some(last) if page.len() >= 256 => after = last.clone(),
            _ => return true,
        }
    }
}

async fn allowed(guard: crate::rpc::drpc::stream_protocol::DeliveryGuard) -> bool {
    matches!(guard().await, Ok(true))
}

/// The protocol's JSON for one channel event of `stream`, after the same
/// recipient filters and delivery guards the RPC stream applies; `None` when
/// the event is not this stream's or must not reach `me`.
async fn event_data(ctx: &Context, me: &str, stream: Stream, event: &SocialEvent) -> Option<Value> {
    match (stream, event) {
        (Stream::Friendships, SocialEvent::Friendship(update)) => {
            use friendship_update::Update;
            let (action, user) = match update.update.as_ref()? {
                Update::Request(request) => {
                    let mut data = json!({
                        "action": "request",
                        "id": request.id,
                        "address": request.friend.as_ref()?.address,
                        "createdAt": request.created_at,
                    });
                    if let Some(message) = request.message.as_deref().filter(|m| !m.is_empty()) {
                        data["message"] = json!(message);
                    }
                    return Some(data);
                }
                Update::Accept(r) => ("accept", r.user.as_ref()),
                Update::Reject(r) => ("reject", r.user.as_ref()),
                Update::Delete(r) => ("delete", r.user.as_ref()),
                Update::Cancel(r) => ("cancel", r.user.as_ref()),
                Update::Block(r) => ("block", r.user.as_ref()),
            };
            Some(json!({ "action": action, "address": user?.address }))
        }
        (Stream::Blocks, SocialEvent::Block(update)) => {
            Some(json!({ "address": update.address, "isBlocked": update.is_blocked }))
        }
        (Stream::PrivateVoiceChats, SocialEvent::PrivateVoice(update)) => {
            let status = match PrivateVoiceChatStatus::try_from(update.status).ok()? {
                PrivateVoiceChatStatus::VoiceChatRequested => "requested",
                PrivateVoiceChatStatus::VoiceChatAccepted => "accepted",
                PrivateVoiceChatStatus::VoiceChatRejected => "rejected",
                PrivateVoiceChatStatus::VoiceChatEnded => "ended",
                PrivateVoiceChatStatus::VoiceChatExpired => "expired",
            };
            let mut data = json!({ "callId": update.call_id, "status": status });
            if status == "requested"
                && let Some(caller) = &update.caller
            {
                data["callerAddress"] = json!(caller.address);
            }
            if let Some(credentials) = &update.credentials {
                data["connectionUrl"] = json!(credentials.connection_url);
            }
            Some(data)
        }
        (Stream::CommunityVoiceChats, SocialEvent::CommunityVoice(update)) => {
            let status = match CommunityVoiceChatStatus::try_from(update.status).ok()? {
                CommunityVoiceChatStatus::CommunityVoiceChatStarted => "started",
                CommunityVoiceChatStatus::CommunityVoiceChatEnded => "ended",
            };
            let mut data = json!({
                "communityId": update.community_id,
                "status": status,
                "createdAt": update.created_at,
                "positions": update.positions,
                "worlds": update.worlds,
                "isMember": update.is_member,
                "communityName": update.community_name,
            });
            if let Some(ended_at) = update.ended_at {
                data["endedAt"] = json!(ended_at);
            }
            if let Some(image) = &update.community_image {
                data["communityImage"] = json!(image);
            }
            Some(data)
        }
        (
            Stream::FriendConnectivity,
            SocialEvent::FriendConnectivity(update)
            | SocialEvent::TopologyFriendConnectivity(update, _),
        ) => {
            let friend = update.friend.as_ref()?;
            let online = update.status == ConnectivityStatus::Online as i32;
            let generation = match event {
                SocialEvent::TopologyFriendConnectivity(_, generation) => {
                    if !ctx.topology().queued_status_is_current(
                        *generation,
                        &friend.address,
                        online,
                    ) {
                        return None;
                    }
                    *generation
                }
                _ => ctx.topology().snapshot().generation,
            };
            let guard = ctx.friend_delivery_guard(
                me.to_string(),
                friend.address.clone(),
                online,
                generation,
            );
            if !allowed(guard).await {
                return None;
            }
            Some(json!({ "address": friend.address, "status": connectivity_name(update.status)? }))
        }
        (Stream::CommunityMemberConnectivity, SocialEvent::CommunityMemberStatus(update)) => {
            Some(json!({
                "communityId": update.community_id,
                "memberAddress": update.member.as_ref()?.address,
                "status": connectivity_name(update.status)?,
            }))
        }
        (
            Stream::CommunityMemberConnectivity,
            SocialEvent::CommunityMember(update) | SocialEvent::TopologyCommunityMember(update, _),
        ) => {
            if !allowed(ctx.event_delivery_guard(me, event)?).await {
                return None;
            }
            Some(json!({
                "communityId": update.community_id,
                "memberAddress": update.member.as_ref()?.address,
                "status": connectivity_name(update.status)?,
            }))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(raw: &str) -> Result<ClientMessage, ProtocolError> {
        parse_message(raw.as_bytes())
    }

    #[test]
    fn messages_parse_as_upstream_validates_them() {
        assert_eq!(parse(r#"{"type":"ping"}"#), Ok(ClientMessage::Ping));
        assert_eq!(
            parse(r#"{"type":"subscribe","id":"a","stream":"block_updates"}"#),
            Ok(ClientMessage::Subscribe {
                id: "a".into(),
                stream: "block_updates".into()
            })
        );
        assert_eq!(
            parse("not json").unwrap_err().message,
            "Messages must be valid JSON"
        );
        assert_eq!(
            parse("[]").unwrap_err().message,
            "Messages must be JSON objects"
        );
        assert_eq!(
            parse(r#"{"type":"other"}"#).unwrap_err().message,
            "Unknown message type"
        );
        let long = "x".repeat(65);
        let refused = parse(&format!(
            r#"{{"type":"subscribe","id":"{long}","stream":"s"}}"#
        ));
        assert_eq!(
            refused,
            Err(protocol_error(
                "invalid_message",
                "id must be a string of 1 to 64 characters",
                None
            ))
        );
        assert_eq!(
            parse(r#"{"type":"subscribe","id":"a","stream":7}"#),
            Err(protocol_error(
                "unknown_stream",
                "Unknown stream",
                Some("a".into())
            ))
        );
        assert_eq!(
            parse(r#"{"type":"unsubscribe","id":""}"#),
            Err(protocol_error(
                "unknown_subscription",
                "No subscription with this id",
                None
            ))
        );
        // 64 UTF-16 units, as JS counts `id.length`: 32 astral characters fit.
        let astral = "\u{1F600}".repeat(32);
        assert!(parse(&format!(r#"{{"type":"unsubscribe","id":"{astral}"}}"#)).is_ok());
        let astral = "\u{1F600}".repeat(33);
        assert!(parse(&format!(r#"{{"type":"unsubscribe","id":"{astral}"}}"#)).is_err());
    }

    #[test]
    fn the_signature_is_over_the_stream_route_never_the_rpc_root() {
        let mut headers = HeaderMap::new();
        assert_eq!(
            stream_paths(&headers, "/social-rpc/v1/stream"),
            vec!["/v1/stream", "/social-rpc/v1/stream"]
        );
        headers.insert(
            "x-original-path",
            axum::http::HeaderValue::from_static("/edge/v1/stream?x=1"),
        );
        assert_eq!(
            stream_paths(&headers, "/social-rpc/v1/stream"),
            vec!["/edge/v1/stream", "/v1/stream", "/social-rpc/v1/stream"]
        );
        headers.insert("x-original-path", axum::http::HeaderValue::from_static("/"));
        assert!(!stream_paths(&headers, "/social-rpc/v1/stream").contains(&"/".to_string()));
    }
}
