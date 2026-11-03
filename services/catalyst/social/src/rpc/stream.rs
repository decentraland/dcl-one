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
//! share the socket pool, the per-address socket cap and presence
//! (`limits::Admission`, `Context::mark_online`): the address's last socket
//! of either kind ending ends its private call.
//!
//! Backpressure is upstream's (`src/adapters/stream-sessions/component.ts`):
//! a writer task drains the socket and counts the bytes handed to it and not
//! yet written, as uWS's buffered amount; a subscription hands its next
//! event over only once that is zero, a send while more than
//! `WS_MAX_BACKPRESSURE` bytes wait closes the socket 1008 `Slow consumer`,
//! and a subscription whose queue of 1,000 events is full closes it 1008
//! `Subscription overflow; reconnect and resync`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::State as AxumState;
use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use serde_json::{Map, Value, json};
use tokio::sync::{broadcast::error::RecvError, mpsc, watch};
use tokio::task::JoinHandle;

use crate::rpc::State;
use crate::rpc::context::Context;
use crate::rpc::limits::Protocol;
use crate::rpc::proto::v2::{
    CommunityVoiceChatStatus, ConnectivityStatus, PrivateVoiceChatStatus, friendship_update,
};
use crate::rpc::pubsub::SocialEvent;
use crate::rpc::ws::{AUTH_TIMEOUT, POOL_FULL, UNAUTHORIZED};
use crate::shared::rpc::handshake::{
    HandshakeError, handshake_path, validator_ref, verify_handshake_ms,
};
use crate::shared::rpc::ws::{IdleAction, IdleTimer, SocketClose, close_message};

/// The route upstream signs and serves (`src/adapters/stream-sessions/constants.ts`).
pub const STREAM_PATH: &str = "/v1/stream";
/// Events queued per subscription (upstream `MAX_VALUE_QUEUE_SIZE`,
/// `src/utils/emitterToGenerator.ts:4`, a constant there too).
const SUBSCRIPTION_QUEUE: usize = 1_000;
/// How long a closing socket may take to write what is queued before its
/// close frame; past it the connection is dropped.
const CLOSE_BUDGET: Duration = Duration::from_secs(10);
/// A subscription or unsubscription id: 1 to 64 UTF-16 units, as JS counts.
const MAX_ID_UNITS: usize = 64;

const MESSAGE_RATE: (u16, &str) = (1008, "Message rate exceeded; reconnect with backoff");
const SLOW_CONSUMER: (u16, &str) = (1008, "Slow consumer");
const SUBSCRIPTION_OVERFLOW: (u16, &str) = (1008, "Subscription overflow; reconnect and resync");
/// Close codes counted under their own label; any other is `other`
/// (upstream `KNOWN_CLOSE_CODES`).
const KNOWN_CLOSE_CODES: [u16; 11] = [
    1000, 1001, 1005, 1006, 1008, 1009, 1011, 1013, 3003, 3004, 3005,
];

/// `/v1/stream`'s own limits (upstream `stream-ws-handler.ts:28`,
/// `stream-sessions/component.ts:68-69`), each positive.
#[derive(Clone, Copy, Debug)]
pub struct StreamLimits {
    /// `STREAM_WS_MAX_PAYLOAD_LENGTH`: the largest inbound frame; enough for
    /// a signed-fetch auth chain.
    pub max_payload_bytes: usize,
    /// `STREAM_WS_MESSAGES_PER_SECOND` / `STREAM_WS_MESSAGE_BURST`: every
    /// application frame, the auth frame included, draws from this bucket.
    pub messages_per_second: f64,
    pub message_burst: usize,
}

impl Default for StreamLimits {
    fn default() -> Self {
        Self {
            max_payload_bytes: 16 * 1024,
            messages_per_second: 20.0,
            message_burst: 40,
        }
    }
}

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

    /// Whether `event` is one this stream listens to, as each upstream
    /// generator listens to its one event: only these fill its queue.
    fn carries(self, event: &SocialEvent) -> bool {
        use SocialEvent as E;
        match self {
            Self::Friendships => matches!(event, E::Friendship(_)),
            Self::Blocks => matches!(event, E::Block(_)),
            Self::PrivateVoiceChats => matches!(event, E::PrivateVoice(_)),
            Self::CommunityVoiceChats => matches!(event, E::CommunityVoice(_)),
            Self::FriendConnectivity => matches!(
                event,
                E::FriendConnectivity(_)
                    | E::TopologyFriendConnectivity(..)
                    | E::ConnectivityResync
            ),
            Self::CommunityMemberConnectivity => matches!(
                event,
                E::CommunityMember(_)
                    | E::CommunityMemberStatus(_)
                    | E::TopologyCommunityMember(..)
                    | E::ConnectivityResync
            ),
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
    let verdict = verify_handshake_ms(
        frame,
        "get",
        paths,
        state.settings.auth_signature_expiration_ms(),
        chrono::Utc::now().timestamp_millis(),
        validator_ref(&state.eip1654),
    )
    .await
    .map(|address| address.into_string());
    if let Err(e) = &verdict {
        let kind = match e {
            HandshakeError::CatalystUnavailable(_) => "server_error",
            _ => "client_rejected",
        };
        metrics::counter!(
            "ws_auth_errors_by_protocol",
            "protocol" => Protocol::Stream.label(),
            "type" => kind
        )
        .increment(1);
    }
    verdict
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
    ws.max_message_size(state.settings.stream.max_payload_bytes)
        .on_upgrade(move |mut socket| async move {
            let max = state.settings.ws_max_concurrent_connections;
            let Some(slot) = state.ctx.open_socket(Protocol::Stream, max) else {
                tracing::warn!(max, "rejecting stream connection: pool is full");
                let _ = socket.send(close_message(POOL_FULL)).await;
                return;
            };
            let code = Session::new(state, socket, slot.close.clone())
                .run(paths, verified)
                .await;
            let label = if KNOWN_CLOSE_CODES.contains(&code) {
                code.to_string()
            } else {
                "other".into()
            };
            metrics::counter!("stream_ws_close_codes", "code" => label).increment(1);
        })
        .into_response()
}

/// The per-connection token bucket of incoming application frames.
struct MessageBudget {
    tokens: f64,
    updated_at: Instant,
    limits: StreamLimits,
}

impl MessageBudget {
    fn take(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(self.updated_at).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.limits.messages_per_second)
            .min(self.limits.message_burst as f64);
        self.updated_at = now;
        if self.tokens < 1.0 {
            return false;
        }
        self.tokens -= 1.0;
        true
    }
}

/// What a subscription hands the socket loop.
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

/// One live subscription; dropping it stops its tasks.
struct Subscription {
    id: String,
    stream: Stream,
    tasks: [JoinHandle<()>; 2],
}

impl Drop for Subscription {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

/// The socket's write half, drained by its own task. `buffered` counts the
/// bytes handed to the task and not yet written (uWS's buffered amount).
struct Outbox {
    tx: Option<mpsc::UnboundedSender<Message>>,
    buffered: Arc<watch::Sender<usize>>,
    max_backpressure: usize,
    writer: JoinHandle<()>,
}

impl Outbox {
    fn spawn(
        mut sink: SplitSink<WebSocket, Message>,
        max_backpressure: usize,
        idle: IdleTimer,
        close: Arc<SocketClose>,
    ) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
        let buffered = Arc::new(watch::Sender::new(0usize));
        let written = buffered.clone();
        let writer = tokio::spawn(async move {
            loop {
                let message = tokio::select! {
                    action = idle.next() => match action {
                        IdleAction::Ping => {
                            if sink.send(Message::Ping(Vec::new().into())).await.is_err() { break; }
                            continue;
                        }
                        IdleAction::Expired => { close.peer_gone(); return; }
                    },
                    message=rx.recv()=>match message { Some(message)=>message,None=>break },
                };
                let len = message_len(&message);
                let closing = matches!(message, Message::Close(_));
                let sent = sink.send(message).await.is_ok();
                written.send_modify(|b| *b = b.saturating_sub(len));
                if sent {
                    idle.activity();
                }
                if !sent || closing {
                    break;
                }
            }
            let _ = tokio::time::timeout(Duration::from_millis(100), sink.close()).await;
        });
        Self {
            tx: Some(tx),
            buffered,
            max_backpressure,
            writer,
        }
    }

    /// Queues `text` unless more than the backpressure limit is waiting
    /// (upstream's `DROPPED`); false when it was not queued.
    fn send(&self, text: String) -> Result<(), Dropped> {
        let Some(tx) = &self.tx else {
            return Err(Dropped::Gone);
        };
        if *self.buffered.borrow() > self.max_backpressure {
            return Err(Dropped::Backpressure);
        }
        self.buffered.send_modify(|b| *b += text.len());
        tx.send(Message::Text(text.into()))
            .map_err(|_| Dropped::Gone)
    }

    /// Sends the close frame after whatever is queued, then waits for the
    /// writer to finish.
    async fn close(mut self, frame: Option<(u16, &'static str)>) {
        if let (Some(tx), Some(frame)) = (self.tx.take(), frame) {
            let _ = tx.send(close_message(frame));
        }
        self.tx = None;
        let _ = tokio::time::timeout(CLOSE_BUDGET, &mut self.writer).await;
        self.writer.abort();
    }
}

enum Dropped {
    Backpressure,
    Gone,
}

fn message_len(message: &Message) -> usize {
    match message {
        Message::Text(text) => text.len(),
        Message::Binary(bytes) => bytes.len(),
        _ => 0,
    }
}

struct Session {
    state: Arc<State>,
    incoming: SplitStream<WebSocket>,
    idle: IdleTimer,
    outbox: Outbox,
    close: Arc<SocketClose>,
    /// The close code the socket ended with, for `stream_ws_close_codes`.
    code: u16,
    budget: MessageBudget,
    subscriptions: HashMap<u64, Subscription>,
    next_serial: u64,
}

impl Session {
    fn new(state: Arc<State>, socket: WebSocket, close: Arc<SocketClose>) -> Self {
        let (sink, incoming) = socket.split();
        let limits = state.settings.stream;
        let idle = IdleTimer::new(state.settings.ws_idle_timeout_secs);
        let outbox = Outbox::spawn(
            sink,
            state.settings.ws_max_backpressure_bytes,
            idle.clone(),
            close.clone(),
        );
        Self {
            state,
            incoming,
            idle,
            outbox,
            close,
            code: 1006,
            budget: MessageBudget {
                tokens: limits.message_burst as f64,
                updated_at: Instant::now(),
                limits,
            },
            subscriptions: HashMap::new(),
            next_serial: 0,
        }
    }

    fn ctx(&self) -> &Context {
        &self.state.ctx
    }

    /// Queues a frame; a send past the backpressure limit closes the
    /// socket 1008 `Slow consumer` (upstream `safeSend`). False once the
    /// socket is ending.
    fn send(&self, message: Value) -> bool {
        match self.outbox.send(message.to_string()) {
            Ok(()) => true,
            Err(Dropped::Backpressure) => {
                self.policy_close("slow_consumer", SLOW_CONSUMER);
                false
            }
            Err(Dropped::Gone) => {
                self.close.peer_gone();
                false
            }
        }
    }

    /// Upstream `closeForPolicy`: counted under its type, then closed 1008.
    fn policy_close(&self, kind: &'static str, (code, reason): (u16, &'static str)) {
        if !self.close.is_ended() {
            metrics::counter!("stream_ws_errors", "type" => kind).increment(1);
        }
        self.close.end(code, reason);
    }

    fn error(&self, code: &'static str, message: &str, id: Option<&str>) -> bool {
        metrics::counter!("stream_ws_errors", "type" => code).increment(1);
        let mut error = json!({ "type": "error", "code": code, "message": message });
        if let Some(id) = id {
            error["id"] = json!(id);
        }
        self.send(error)
    }

    /// The next application frame; `None` once the peer is gone (its close
    /// code kept for the metric).
    async fn recv(&mut self) -> Option<Vec<u8>> {
        loop {
            let message = self.incoming.next().await;
            if matches!(&message, Some(Ok(_))) {
                self.idle.activity();
            }
            match message {
                Some(Ok(Message::Text(text))) => return Some(text.as_bytes().to_vec()),
                Some(Ok(Message::Binary(bytes))) => return Some(bytes.to_vec()),
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => continue,
                Some(Ok(Message::Close(frame))) => {
                    self.code = frame.map_or(1005, |f| f.code);
                    return None;
                }
                Some(Err(_)) | None => return None,
            }
        }
    }

    /// Serves the socket until it ends; answers its close code.
    async fn run(mut self, paths: Vec<String>, verified: Option<String>) -> u16 {
        let address = match verified {
            Some(address) => {
                metrics::counter!("stream_ws_authentications", "method" => "upgrade_headers")
                    .increment(1);
                Some(address)
            }
            None => self.first_frame_handshake(&paths).await,
        };
        if let Some(address) = address {
            self.serve(&address).await;
        }
        self.subscriptions.clear();
        let frame = self.close.frame();
        if let Some((code, _)) = frame {
            self.code = code;
        }
        self.outbox.close(frame).await;
        self.code
    }

    /// The fallback handshake: the first application frame is the chain.
    async fn first_frame_handshake(&mut self, paths: &[String]) -> Option<String> {
        let close = self.close.clone();
        let window = self.state.settings.auth_timeout();
        let deadline = tokio::time::sleep(window);
        tokio::pin!(deadline);
        let frame = tokio::select! {
            _ = close.ended() => return None,
            _ = &mut deadline => {
                close.end(AUTH_TIMEOUT.0, AUTH_TIMEOUT.1);
                return None;
            }
            frame = self.recv() => match frame {
                Some(frame) => String::from_utf8_lossy(&frame).into_owned(),
                None => {
                    close.peer_gone();
                    return None;
                }
            },
        };
        if !self.budget.take() {
            self.policy_close("message_rate", MESSAGE_RATE);
            return None;
        }
        let state = self.state.clone();
        let verification = verify(&state, paths, &frame);
        tokio::pin!(verification);
        let verdict = loop {
            tokio::select! {
                biased;
                _ = close.ended() => return None,
                frame = self.recv() => {
                    if frame.is_none() {
                        close.peer_gone();
                        return None;
                    }
                    if !self.budget.take() {
                        self.policy_close("message_rate", MESSAGE_RATE);
                        return None;
                    }
                    self.error("auth_in_progress", "Authentication already in progress", None);
                }
                _ = &mut deadline => {
                    close.end(AUTH_TIMEOUT.0, AUTH_TIMEOUT.1);
                    return None;
                }
                verdict = &mut verification => break verdict,
            }
        };
        match verdict {
            Ok(address) => {
                metrics::counter!("stream_ws_authentications", "method" => "first_frame")
                    .increment(1);
                Some(address)
            }
            Err(e) => {
                tracing::info!(error = %e, "stream handshake failed");
                close.end(UNAUTHORIZED.0, UNAUTHORIZED.1);
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
            .claim(address, self.close.clone(), Protocol::Stream);
        ctx.mark_online(address);
        let (tx, mut rx) = mpsc::channel::<Outgoing>(1);
        if self.send(json!({ "type": "ready" })) {
            self.message_loop(address, &tx, &mut rx).await;
        }
        self.subscriptions.clear();
        ctx.admission().release(address, &self.close);
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
    ) {
        let close = self.close.clone();
        loop {
            tokio::select! {
                _ = close.ended() => return,
                outgoing = rx.recv() => {
                    let Some(outgoing) = outgoing else { return };
                    if !self.deliver(outgoing) {
                        return;
                    }
                }
                frame = self.recv() => {
                    let Some(raw) = frame else {
                        close.peer_gone();
                        return;
                    };
                    if !self.budget.take() {
                        self.policy_close("message_rate", MESSAGE_RATE);
                        return;
                    }
                    if !self.handle(address, tx, &raw) {
                        return;
                    }
                }
            }
        }
    }

    /// An event or completion of a subscription still open; anything queued
    /// for one already ended is dropped, so nothing follows its `end`.
    fn deliver(&mut self, outgoing: Outgoing) -> bool {
        match outgoing {
            Outgoing::Event { serial, data } => {
                let Some(subscription) = self.subscriptions.get(&serial) else {
                    return true;
                };
                let stream = subscription.stream.name();
                let event = json!({
                    "type": "event",
                    "id": subscription.id,
                    "stream": stream,
                    "data": data,
                });
                let sent = self.send(event);
                if sent {
                    metrics::counter!("stream_ws_updates_sent", "event" => stream).increment(1);
                }
                sent
            }
            Outgoing::Completed { serial } => {
                let Some(subscription) = self.subscriptions.remove(&serial) else {
                    return true;
                };
                self.send(json!({ "type": "end", "id": subscription.id, "reason": "completed" }))
            }
        }
    }

    fn handle(&mut self, address: &str, tx: &mpsc::Sender<Outgoing>, raw: &[u8]) -> bool {
        let message = match parse_message(raw) {
            Err(e) => {
                metrics::counter!("stream_ws_messages_received", "type" => e.received).increment(1);
                return self.error(e.code, e.message, e.id.as_deref());
            }
            Ok(message) => message,
        };
        let kind = match &message {
            ClientMessage::Ping => "ping",
            ClientMessage::Subscribe { .. } => "subscribe",
            ClientMessage::Unsubscribe { .. } => "unsubscribe",
        };
        metrics::counter!("stream_ws_messages_received", "type" => kind).increment(1);
        match message {
            ClientMessage::Ping => self.send(json!({ "type": "pong" })),
            ClientMessage::Unsubscribe { id } => {
                let serial = self
                    .subscriptions
                    .iter()
                    .find(|(_, s)| s.id == id)
                    .map(|(serial, _)| *serial);
                if serial.and_then(|s| self.subscriptions.remove(&s)).is_none() {
                    return self.error(
                        "unknown_subscription",
                        "No subscription with this id",
                        Some(&id),
                    );
                }
                self.send(json!({ "type": "end", "id": id, "reason": "unsubscribed" }))
            }
            ClientMessage::Subscribe { id, stream } => {
                let Some(stream) = Stream::parse(&stream) else {
                    return self.error("unknown_stream", "Unknown stream", Some(&id));
                };
                if self.subscriptions.values().any(|s| s.id == id) {
                    return self.error(
                        "duplicate_subscription",
                        "A subscription with this id already exists",
                        Some(&id),
                    );
                }
                if self.subscriptions.values().any(|s| s.stream == stream) {
                    return self.error(
                        "duplicate_subscription",
                        "Already subscribed to this stream on this connection",
                        Some(&id),
                    );
                }
                let admission = self.ctx().admission();
                // A socket a newer one displaced holds no slot; its close is under way.
                if !admission.holds(address, &self.close) {
                    return self.error(
                        "stream_error",
                        "The subscription could not be started",
                        Some(&id),
                    );
                }
                if !admission.admit_subscription(address) {
                    return self.error(
                        "rate_limited",
                        "Subscription rate exceeded; retry with backoff",
                        Some(&id),
                    );
                }
                self.next_serial += 1;
                let serial = self.next_serial;
                // Subscribed before `subscribed` is sent: nothing published after it is missed.
                let tasks = spawn_stream(
                    self.ctx().clone(),
                    address.to_string(),
                    stream,
                    serial,
                    tx.clone(),
                    self.close.clone(),
                    self.outbox.buffered.subscribe(),
                );
                self.subscriptions.insert(
                    serial,
                    Subscription {
                        id: id.clone(),
                        stream,
                        tasks,
                    },
                );
                self.send(json!({ "type": "subscribed", "id": id }))
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
    /// The `type` label of `stream_ws_messages_received` (upstream `receivedType`).
    received: &'static str,
}

fn protocol_error(code: &'static str, message: &'static str, id: Option<String>) -> ProtocolError {
    let received = match code {
        "unknown_subscription" => "unsubscribe",
        "unknown_stream" => "subscribe",
        _ => "invalid",
    };
    ProtocolError {
        code,
        message,
        id,
        received,
    }
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
            let id = valid_id(object.get("id")).ok_or_else(|| ProtocolError {
                received: "subscribe",
                ..protocol_error(
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

/// One subscription: the address's channel, taken before it returns. A
/// forwarding task moves the events `stream` listens to into the
/// subscription's queue of [`SUBSCRIPTION_QUEUE`]; a full queue (or a lagging
/// channel) closes the socket, as an update cannot be replayed. A second
/// task maps each to its JSON and hands it to the socket loop once nothing
/// is waiting to be written.
fn spawn_stream(
    ctx: Context,
    me: String,
    stream: Stream,
    serial: u64,
    tx: mpsc::Sender<Outgoing>,
    close: Arc<SocketClose>,
    mut buffered: watch::Receiver<usize>,
) -> [JoinHandle<()>; 2] {
    let mut events = ctx.pubsub().subscribe(&me);
    let (queue_tx, mut queue) = mpsc::channel::<Arc<SocialEvent>>(SUBSCRIPTION_QUEUE);
    let overflow = move || {
        if !close.is_ended() {
            metrics::counter!("stream_ws_errors", "type" => "subscription_overflow").increment(1);
        }
        close.end(SUBSCRIPTION_OVERFLOW.0, SUBSCRIPTION_OVERFLOW.1);
    };
    let forward = tokio::spawn(async move {
        loop {
            let event = match events.recv().await {
                Ok(event) => event,
                Err(RecvError::Lagged(skipped)) => {
                    tracing::warn!(skipped, "stream subscription lagged");
                    return overflow();
                }
                Err(RecvError::Closed) => return,
            };
            if !stream.carries(&event) {
                continue;
            }
            match queue_tx.try_send(event) {
                Ok(()) => {}
                Err(mpsc::error::TrySendError::Full(_)) => return overflow(),
                Err(mpsc::error::TrySendError::Closed(_)) => return,
            }
        }
    });
    let process = tokio::spawn(async move {
        // Upstream waits for the socket to drain before each event.
        let mut hand_off = async |data: Value| {
            buffered.wait_for(|waiting| *waiting == 0).await.is_ok()
                && tx.send(Outgoing::Event { serial, data }).await.is_ok()
        };
        if stream == Stream::FriendConnectivity {
            for friend in online_friends(&ctx, &me).await {
                if !hand_off(json!({ "address": friend, "status": "online" })).await {
                    return;
                }
            }
        }
        while let Some(event) = queue.recv().await {
            if matches!(&*event, SocialEvent::ConnectivityResync) {
                break;
            }
            let Some(data) = event_data(&ctx, &me, stream, &event).await else {
                continue;
            };
            if !hand_off(data).await {
                return;
            }
        }
        let _ = tx.send(Outgoing::Completed { serial }).await;
    });
    [forward, process]
}

/// The friends online when the subscription starts, one `online` event each,
/// as the RPC stream starts (upstream `getOnlineFriendAddresses`); a failed
/// read ends the list where it failed.
async fn online_friends(ctx: &Context, me: &str) -> Vec<String> {
    let mut online = Vec::new();
    let mut after = String::new();
    loop {
        let Ok(page) = ctx.db().connectivity_friends_page(me, &after, true).await else {
            return online;
        };
        let snapshot = ctx.topology().snapshot();
        online.extend(page.iter().filter(|f| snapshot.is_online(f)).cloned());
        match page.last() {
            Some(last) if page.len() >= 256 => after = last.clone(),
            _ => return online,
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
            Err(ProtocolError {
                received: "subscribe",
                ..protocol_error(
                    "invalid_message",
                    "id must be a string of 1 to 64 characters",
                    None
                )
            })
        );
        assert_eq!(parse("[]").unwrap_err().received, "invalid");
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

/// Exercise the production mapper through the captured source channel names.
#[cfg(test)]
pub(crate) async fn captured_event_data(
    ctx: &Context,
    me: &str,
    id: &str,
    event: &SocialEvent,
) -> Option<Value> {
    let stream = match id {
        "friendship" => Stream::Friendships,
        "friend-connectivity" => Stream::FriendConnectivity,
        "block" => Stream::Blocks,
        "private-voice" => Stream::PrivateVoiceChats,
        "community-member" => Stream::CommunityMemberConnectivity,
        "community-voice" => Stream::CommunityVoiceChats,
        _ => return None,
    };
    if !stream.carries(event) {
        return None;
    }
    event_data(ctx, me, stream, event).await
}
