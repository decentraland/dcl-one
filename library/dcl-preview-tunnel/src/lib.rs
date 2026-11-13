pub mod config;
/// The wire types, defined once in dcl-preview-tunnel-protocol so this relay
/// and every agent (dcl-scene-cli included) can never drift out of sync with
/// each other; re-exported here at the path this crate always used.
pub use dcl_preview_tunnel_protocol as protocol;

pub use config::Config;

use axum::{
    Router,
    body::{Body, Bytes},
    extract::{
        FromRequestParts, Path as AxPath, Request, State, WebSocketUpgrade,
        ws::{CloseFrame, Message, WebSocket},
    },
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{any, get},
};
use futures::{SinkExt, StreamExt};
use protocol::{ChannelKind, Control, Resume, encode_data};
use rand::RngExt;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

const ID_ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";
const ID_LEN: usize = 10;
const HELLO_TIMEOUT: Duration = Duration::from_secs(10);
const EVENT_SEND_TIMEOUT: Duration = Duration::from_secs(5);
const CHANNEL_EVENT_BUFFER: usize = 256;
const TRUNK_BUFFER: usize = 1024;
const SERVICE_WORKER: &str = "service-worker";
const SERVICE_WORKER_ALLOWED: &str = "service-worker-allowed";

pub struct AppState {
    cfg: Config,
    epochs: AtomicU64,
    agents: Mutex<HashMap<String, AgentEntry>>,
}

impl AppState {
    pub fn new(cfg: Config) -> Self {
        AppState {
            cfg,
            epochs: AtomicU64::new(0),
            agents: Mutex::new(HashMap::new()),
        }
    }
}

struct AgentEntry {
    resume_key: String,
    epoch: u64,
    conn: Option<AgentConn>,
}

#[derive(Clone)]
struct AgentConn {
    tx: mpsc::Sender<TrunkOut>,
    channels: Arc<Mutex<HashMap<u32, Channel>>>,
    next_ch: Arc<AtomicU32>,
}

enum TrunkOut {
    Control(Control),
    Data(u32, bool, Vec<u8>),
}

struct Channel {
    is_ws: bool,
    reply: Option<oneshot::Sender<Control>>,
    events: mpsc::Sender<ChanEvent>,
}

enum ChanEvent {
    Data {
        binary: bool,
        payload: Vec<u8>,
    },
    End,
    Close {
        code: Option<u16>,
        reason: Option<String>,
    },
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/t/_connect", get(trunk))
        .route("/t/{id}", any(public_bare))
        .route("/t/{id}/", any(public_root))
        .route("/t/{id}/{*path}", any(public_sub))
        .with_state(state)
}

fn lock<'a, T>(m: &'a Mutex<T>) -> std::sync::MutexGuard<'a, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

async fn trunk(State(st): State<Arc<AppState>>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| handle_trunk(socket, st))
}

async fn recv_control(socket: &mut WebSocket) -> Option<Control> {
    loop {
        match socket.recv().await {
            Some(Ok(Message::Text(text))) => return Control::decode(&text),
            Some(Ok(Message::Close(_))) | None => return None,
            Some(Ok(_)) => continue,
            Some(Err(_)) => return None,
        }
    }
}

async fn close_with(socket: &mut WebSocket, code: u16, reason: &str) {
    let _ = socket
        .send(Message::Close(Some(CloseFrame {
            code,
            reason: reason.to_string().into(),
        })))
        .await;
}

async fn handle_trunk(mut socket: WebSocket, st: Arc<AppState>) {
    let hello = tokio::time::timeout(HELLO_TIMEOUT, recv_control(&mut socket)).await;
    let (token, resume, agent) = match hello {
        Ok(Some(Control::Hello {
            token,
            resume,
            agent,
        })) => (token, resume, agent),
        _ => {
            close_with(&mut socket, 4400, "expected a hello control message").await;
            return;
        }
    };
    let authorized = if st.cfg.tokens.is_empty() {
        st.cfg.allow_anonymous
    } else {
        token
            .as_deref()
            .is_some_and(|t| st.cfg.tokens.iter().any(|x| constant_time_eq(x, t)))
    };
    if !authorized {
        close_with(&mut socket, 4401, "invalid token").await;
        return;
    }
    let (tx, mut rx) = mpsc::channel::<TrunkOut>(TRUNK_BUFFER);
    let conn = AgentConn {
        tx,
        channels: Arc::new(Mutex::new(HashMap::new())),
        next_ch: Arc::new(AtomicU32::new(0)),
    };
    let (id, resume_key, epoch) = match register(&st, conn.clone(), resume) {
        Ok(r) => r,
        Err(reason) => {
            close_with(&mut socket, 4409, reason).await;
            return;
        }
    };
    let welcome = Control::Welcome {
        id: id.clone(),
        public_url: format!("{}/t/{}", st.cfg.public_base().trim_end_matches('/'), id),
        resume_key,
        ping_s: st.cfg.ping_secs,
    };
    if socket
        .send(Message::Text(welcome.encode().into()))
        .await
        .is_err()
    {
        teardown(&st, &id, epoch, &conn);
        return;
    }
    tracing::info!(id, agent, "tunnel agent connected");
    let ping = Duration::from_secs(st.cfg.ping_secs);
    let mut last_in = Instant::now();
    let mut ticker = tokio::time::interval_at(Instant::now() + ping, ping);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let (mut sink, mut stream) = socket.split();
    loop {
        tokio::select! {
            out = rx.recv() => match out {
                Some(TrunkOut::Control(c)) => {
                    if sink.send(Message::Text(c.encode().into())).await.is_err() {
                        break;
                    }
                }
                Some(TrunkOut::Data(ch, binary, payload)) => {
                    if sink
                        .send(Message::Binary(encode_data(ch, binary, &payload).into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                None => break,
            },
            incoming = stream.next() => match incoming {
                Some(Ok(msg)) => {
                    last_in = Instant::now();
                    match msg {
                        Message::Text(text) => handle_agent_control(&conn, &text).await,
                        Message::Binary(bytes) => {
                            if let Some(frame) = protocol::decode_data(&bytes) {
                                route_event(
                                    &conn,
                                    frame.ch,
                                    ChanEvent::Data {
                                        binary: frame.binary,
                                        payload: frame.payload,
                                    },
                                )
                                .await;
                            }
                        }
                        Message::Close(_) => break,
                        _ => {}
                    }
                }
                Some(Err(_)) | None => break,
            },
            _ = ticker.tick() => {
                if last_in.elapsed() > ping * 3 {
                    tracing::warn!(id, "tunnel agent unresponsive, dropping trunk");
                    break;
                }
                if sink.send(Message::Text(Control::Ping.encode().into())).await.is_err() {
                    break;
                }
            }
        }
    }
    tracing::info!(id, "tunnel agent disconnected");
    teardown(&st, &id, epoch, &conn);
}

fn register(
    st: &Arc<AppState>,
    conn: AgentConn,
    resume: Option<Resume>,
) -> Result<(String, String, u64), &'static str> {
    let mut agents = lock(&st.agents);
    let epoch = st.epochs.fetch_add(1, Ordering::Relaxed) + 1;
    if let Some(res) = resume
        && let Some(entry) = agents.get_mut(&res.id)
        && entry.resume_key == res.key
    {
        if let Some(old) = entry.conn.take() {
            fail_channels(&old);
        }
        entry.epoch = epoch;
        entry.conn = Some(conn);
        return Ok((res.id, entry.resume_key.clone(), epoch));
    }
    let full = agents.len() >= st.cfg.max_agents;
    let id = if st.cfg.allow_ids.is_empty() {
        if full {
            return Err("tunnel capacity reached");
        }
        loop {
            let cand = random_id();
            if !agents.contains_key(&cand) {
                break cand;
            }
        }
    } else {
        // At the cap only a grace-held id can be taken, since it adds no entry.
        let free = |cand: &&String| {
            agents
                .get(cand.as_str())
                .map_or(!full, |e| e.conn.is_none())
        };
        match st.cfg.allow_ids.iter().find(free) {
            Some(id) => id.clone(),
            None if full => return Err("tunnel capacity reached"),
            None => return Err("no free tunnel id (TUNNEL_ALLOW_IDS exhausted)"),
        }
    };
    // A grace-held allow-listed id is reused in place; only a new entry counts.
    if full && !agents.contains_key(&id) {
        return Err("tunnel capacity reached");
    }
    let resume_key = random_key();
    if let Some(prev) = agents.insert(
        id.clone(),
        AgentEntry {
            resume_key: resume_key.clone(),
            epoch,
            conn: Some(conn),
        },
    ) && let Some(old) = prev.conn
    {
        fail_channels(&old);
    }
    Ok((id, resume_key, epoch))
}

fn teardown(st: &Arc<AppState>, id: &str, epoch: u64, conn: &AgentConn) {
    fail_channels(conn);
    {
        let mut agents = lock(&st.agents);
        match agents.get_mut(id) {
            Some(entry) if entry.epoch == epoch => entry.conn = None,
            _ => return,
        }
    }
    let st = st.clone();
    let id = id.to_string();
    let grace = st.cfg.grace_secs;
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(grace)).await;
        let mut agents = lock(&st.agents);
        if agents
            .get(&id)
            .is_some_and(|e| e.epoch == epoch && e.conn.is_none())
        {
            agents.remove(&id);
            tracing::info!(id, "tunnel id released after grace period");
        }
    });
}

fn fail_channels(conn: &AgentConn) {
    let mut channels = lock(&conn.channels);
    for (_, chan) in channels.drain() {
        if let Some(reply) = chan.reply {
            let _ = reply.send(Control::OpenErr {
                ch: 0,
                error: "tunnel agent disconnected".into(),
            });
        }
        let _ = chan.events.try_send(ChanEvent::Close {
            code: Some(1012),
            reason: Some("tunnel agent disconnected".into()),
        });
    }
}

async fn handle_agent_control(conn: &AgentConn, text: &str) {
    match Control::decode(text) {
        Some(reply @ (Control::OpenOk { .. } | Control::OpenErr { .. })) => {
            deliver_reply(conn, reply);
        }
        Some(Control::End { ch }) => route_event(conn, ch, ChanEvent::End).await,
        Some(Control::Close { ch, code, reason }) => {
            route_event(conn, ch, ChanEvent::Close { code, reason }).await;
            lock(&conn.channels).remove(&ch);
        }
        Some(Control::Ping) => {
            let _ = conn.tx.try_send(TrunkOut::Control(Control::Pong));
        }
        _ => {}
    }
}

fn deliver_reply(conn: &AgentConn, reply: Control) {
    let (ch, failed) = match &reply {
        Control::OpenOk { ch, .. } => (*ch, false),
        Control::OpenErr { ch, .. } => (*ch, true),
        _ => return,
    };
    let sender = {
        let mut channels = lock(&conn.channels);
        let sender = channels.get_mut(&ch).and_then(|c| c.reply.take());
        if failed {
            channels.remove(&ch);
        }
        sender
    };
    if let Some(tx) = sender {
        let _ = tx.send(reply);
    }
}

async fn route_event(conn: &AgentConn, ch: u32, event: ChanEvent) {
    let Some((events, is_ws)) = lock(&conn.channels)
        .get(&ch)
        .map(|c| (c.events.clone(), c.is_ws))
    else {
        return;
    };
    let is_end = matches!(event, ChanEvent::End);
    match events.send_timeout(event, EVENT_SEND_TIMEOUT).await {
        Ok(()) => {
            if is_end && !is_ws {
                lock(&conn.channels).remove(&ch);
            }
        }
        Err(_) => {
            lock(&conn.channels).remove(&ch);
            let _ = conn.tx.try_send(TrunkOut::Control(Control::Close {
                ch,
                code: Some(1013),
                reason: Some("slow consumer".into()),
            }));
        }
    }
}

/// Length leaks; contents do not.
fn constant_time_eq(expected: &str, given: &str) -> bool {
    expected.len() == given.len()
        && expected
            .bytes()
            .zip(given.bytes())
            .fold(0u8, |diff, (a, b)| diff | (a ^ b))
            == 0
}

fn random_id() -> String {
    let mut v = rand::rng().random::<u128>();
    (0..ID_LEN)
        .map(|_| {
            let c = ID_ALPHABET[(v & 31) as usize] as char;
            v >>= 5;
            c
        })
        .collect()
}

fn random_key() -> String {
    format!("{:032x}", rand::rng().random::<u128>())
}

async fn public_root(
    State(st): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
    req: Request,
) -> Response {
    handle_public(st, id, "/".to_string(), req).await
}

/// `/t/{id}` without the slash: a worker script here would default to scope
/// `/t/`, every tunnel's, so worker script fetches are refused.
async fn public_bare(
    State(st): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
    req: Request,
) -> Response {
    if req.headers().contains_key(SERVICE_WORKER) {
        return (
            StatusCode::FORBIDDEN,
            "service workers must be served under /t/<id>/",
        )
            .into_response();
    }
    handle_public(st, id, "/".to_string(), req).await
}

async fn public_sub(
    State(st): State<Arc<AppState>>,
    AxPath((id, path)): AxPath<(String, String)>,
    req: Request,
) -> Response {
    handle_public(st, id, format!("/{path}"), req).await
}

async fn handle_public(st: Arc<AppState>, id: String, path: String, req: Request) -> Response {
    let conn = {
        let agents = lock(&st.agents);
        match agents.get(&id) {
            None => {
                return (StatusCode::NOT_FOUND, "unknown tunnel id").into_response();
            }
            Some(entry) => match &entry.conn {
                None => {
                    return (
                        StatusCode::BAD_GATEWAY,
                        "tunnel agent disconnected \u{2014} waiting for it to reconnect",
                    )
                        .into_response();
                }
                Some(conn) => conn.clone(),
            },
        }
    };
    let is_ws = req
        .headers()
        .get(header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false);
    if is_ws {
        handle_public_ws(st, conn, path, req).await
    } else {
        handle_public_http(st, conn, path, req).await
    }
}

/// Browser credentials for the relay's host never reach an agent: cookies and
/// ambient `Authorization` schemes; a page's explicit bearer still passes.
fn is_ambient_credential(name: &header::HeaderName, value: &header::HeaderValue) -> bool {
    if name == header::COOKIE {
        return true;
    }
    name == header::AUTHORIZATION
        && !value.to_str().ok().is_some_and(|v| {
            v.trim_start()
                .split_once(' ')
                .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        })
}

/// Response headers whose effect is origin-wide, so on the shared origin they
/// would reach every tunnel: state, workers, reporting, transport policy and
/// client hints.
const ORIGIN_WIDE_RESPONSE_HEADERS: &[&str] = &[
    "set-cookie",
    "set-cookie2",
    SERVICE_WORKER_ALLOWED,
    "clear-site-data",
    "nel",
    "report-to",
    "reporting-endpoints",
    "strict-transport-security",
    "alt-svc",
    "accept-ch",
    "critical-ch",
    "origin-agent-cluster",
    "set-login",
];

fn is_cross_tunnel_response_header(name: &str) -> bool {
    ORIGIN_WIDE_RESPONSE_HEADERS
        .iter()
        .any(|blocked| name.eq_ignore_ascii_case(blocked))
}

fn collect_headers(headers: &HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .filter(|(name, _)| !protocol::is_hop_by_hop(name.as_str()))
        .filter(|(name, value)| !is_ambient_credential(name, value))
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|v| (name.as_str().to_string(), v.to_string()))
        })
        .collect()
}

/// A channel's entry in its agent's table. Dropping it frees the slot and, if
/// the agent was told about the channel and has not ended it, closes it there.
/// That `Close` may follow the relay's own `End`; agents treat a `Close` for a
/// finished or unknown channel as a no-op.
struct ChannelSlot {
    conn: AgentConn,
    ch: u32,
    announced: bool,
    close_code: Option<u16>,
    close_reason: Option<&'static str>,
}

impl ChannelSlot {
    /// The agent already knows the channel is over.
    fn settle(&mut self) {
        self.announced = false;
    }
}

impl Drop for ChannelSlot {
    fn drop(&mut self) {
        let present = lock(&self.conn.channels).remove(&self.ch).is_some();
        if present && self.announced {
            let close = TrunkOut::Control(Control::Close {
                ch: self.ch,
                code: self.close_code,
                reason: self.close_reason.map(str::to_owned),
            });
            // A full trunk queue still gets the close, bounded like any other send.
            if let Err(mpsc::error::TrySendError::Full(close)) = self.conn.tx.try_send(close)
                && let Ok(runtime) = tokio::runtime::Handle::try_current()
            {
                let tx = self.conn.tx.clone();
                runtime.spawn(async move {
                    let _ = tx.send_timeout(close, EVENT_SEND_TIMEOUT).await;
                });
            }
        }
    }
}

type OpenedChannel = (
    ChannelSlot,
    oneshot::Receiver<Control>,
    mpsc::Receiver<ChanEvent>,
);

fn open_channel(st: &AppState, conn: &AgentConn, is_ws: bool) -> Option<OpenedChannel> {
    let mut channels = lock(&conn.channels);
    if channels.len() >= st.cfg.max_channels_per_agent {
        return None;
    }
    let ch = conn.next_ch.fetch_add(1, Ordering::Relaxed) + 1;
    let (reply_tx, reply_rx) = oneshot::channel();
    let (events_tx, events_rx) = mpsc::channel(CHANNEL_EVENT_BUFFER);
    channels.insert(
        ch,
        Channel {
            is_ws,
            reply: Some(reply_tx),
            events: events_tx,
        },
    );
    let slot = ChannelSlot {
        conn: conn.clone(),
        ch,
        announced: false,
        close_code: is_ws.then_some(1001),
        close_reason: None,
    };
    Some((slot, reply_rx, events_rx))
}

fn too_large() -> Response {
    (StatusCode::PAYLOAD_TOO_LARGE, "request body too large").into_response()
}

fn too_many_channels() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        "tunnel agent is at its concurrent request limit",
    )
        .into_response()
}

async fn await_open_reply(
    st: &AppState,
    slot: &mut ChannelSlot,
    reply_rx: oneshot::Receiver<Control>,
) -> Result<Control, (StatusCode, &'static str)> {
    match tokio::time::timeout(Duration::from_secs(st.cfg.open_timeout_secs), reply_rx).await {
        Ok(Ok(control)) => Ok(control),
        Ok(Err(_)) => Err((StatusCode::BAD_GATEWAY, "tunnel agent disconnected")),
        Err(_) => {
            slot.close_reason = Some("open timed out");
            Err((
                StatusCode::GATEWAY_TIMEOUT,
                "tunnel agent did not answer in time",
            ))
        }
    }
}

async fn handle_public_http(
    st: Arc<AppState>,
    conn: AgentConn,
    path: String,
    req: Request,
) -> Response {
    let (parts, body) = req.into_parts();
    let declared = parts
        .headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    if declared.is_some_and(|len| len > st.cfg.body_max_bytes as u64) {
        return too_large();
    }
    // Claim the slot before buffering, so a request over the limit costs no body.
    let Some((mut slot, reply_rx, events_rx)) = open_channel(&st, &conn, false) else {
        return too_many_channels();
    };
    let ch = slot.ch;
    // A trickled body must not hold the slot past the read deadline.
    let read = axum::body::to_bytes(body, st.cfg.body_max_bytes);
    let body = match tokio::time::timeout(Duration::from_secs(st.cfg.body_timeout_secs), read).await
    {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(_)) => return too_large(),
        Err(_) => {
            return (
                StatusCode::REQUEST_TIMEOUT,
                "request body not received in time",
            )
                .into_response();
        }
    };
    let open = Control::Open {
        ch,
        kind: ChannelKind::Http,
        method: Some(parts.method.as_str().to_string()),
        path,
        query: parts.uri.query().map(str::to_string),
        headers: collect_headers(&parts.headers),
        subprotocols: None,
    };
    if conn.tx.send(TrunkOut::Control(open)).await.is_err() {
        return (StatusCode::BAD_GATEWAY, "tunnel agent disconnected").into_response();
    }
    slot.announced = true;
    if !body.is_empty()
        && conn
            .tx
            .send(TrunkOut::Data(ch, true, body.to_vec()))
            .await
            .is_err()
    {
        return (StatusCode::BAD_GATEWAY, "tunnel agent disconnected").into_response();
    }
    if conn
        .tx
        .send(TrunkOut::Control(Control::End { ch }))
        .await
        .is_err()
    {
        return (StatusCode::BAD_GATEWAY, "tunnel agent disconnected").into_response();
    }
    let reply = match await_open_reply(&st, &mut slot, reply_rx).await {
        Ok(reply) => reply,
        Err(refusal) => return refusal.into_response(),
    };
    match reply {
        Control::OpenOk {
            status, headers, ..
        } => {
            let mut builder = Response::builder()
                .status(StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY));
            for (name, value) in headers.unwrap_or_default() {
                if protocol::is_hop_by_hop(&name)
                    || name.eq_ignore_ascii_case("content-length")
                    || is_cross_tunnel_response_header(&name)
                {
                    continue;
                }
                builder = builder.header(name, value);
            }
            // The body owns the slot: a client that goes away frees it.
            let stream = futures::stream::unfold(
                (events_rx, slot),
                |(mut events_rx, mut slot)| async move {
                    match events_rx.recv().await {
                        Some(ChanEvent::Data { payload, .. }) => Some((
                            Ok::<Bytes, std::io::Error>(Bytes::from(payload)),
                            (events_rx, slot),
                        )),
                        Some(ChanEvent::End | ChanEvent::Close { .. }) | None => {
                            slot.settle();
                            None
                        }
                    }
                },
            );
            builder.body(Body::from_stream(stream)).unwrap_or_else(|_| {
                (
                    StatusCode::BAD_GATEWAY,
                    "invalid response from tunnel agent",
                )
                    .into_response()
            })
        }
        Control::OpenErr { error, .. } => (
            StatusCode::BAD_GATEWAY,
            format!("tunnel agent error: {error}"),
        )
            .into_response(),
        _ => (
            StatusCode::BAD_GATEWAY,
            "unexpected reply from tunnel agent",
        )
            .into_response(),
    }
}

async fn handle_public_ws(
    st: Arc<AppState>,
    conn: AgentConn,
    path: String,
    req: Request,
) -> Response {
    let (mut parts, _body) = req.into_parts();
    let upgrade =
        match <WebSocketUpgrade as FromRequestParts<()>>::from_request_parts(&mut parts, &()).await
        {
            Ok(upgrade) => upgrade,
            Err(e) => return e.into_response(),
        };
    let subprotocols: Vec<String> = parts
        .headers
        .get_all(header::SEC_WEBSOCKET_PROTOCOL)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    let Some((mut slot, reply_rx, events_rx)) = open_channel(&st, &conn, true) else {
        return too_many_channels();
    };
    let ch = slot.ch;
    let open = Control::Open {
        ch,
        kind: ChannelKind::Ws,
        method: None,
        path,
        query: parts.uri.query().map(str::to_string),
        headers: collect_headers(&parts.headers),
        subprotocols: (!subprotocols.is_empty()).then_some(subprotocols),
    };
    if conn.tx.send(TrunkOut::Control(open)).await.is_err() {
        return (StatusCode::BAD_GATEWAY, "tunnel agent disconnected").into_response();
    }
    slot.announced = true;
    let reply = match await_open_reply(&st, &mut slot, reply_rx).await {
        Ok(reply) => reply,
        Err(refusal) => return refusal.into_response(),
    };
    let subprotocol = match reply {
        Control::OpenOk { subprotocol, .. } => subprotocol,
        Control::OpenErr { error, .. } => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("tunnel agent error: {error}"),
            )
                .into_response();
        }
        _ => {
            return (
                StatusCode::BAD_GATEWAY,
                "unexpected reply from tunnel agent",
            )
                .into_response();
        }
    };
    let upgrade = match subprotocol {
        Some(sp) => upgrade.protocols([sp]),
        None => upgrade,
    };
    // A failed upgrade drops the closure, and the slot with it.
    upgrade.on_upgrade(move |socket| pump_public_ws(socket, slot, events_rx))
}

async fn pump_public_ws(
    socket: WebSocket,
    mut slot: ChannelSlot,
    mut events_rx: mpsc::Receiver<ChanEvent>,
) {
    let conn = slot.conn.clone();
    let ch = slot.ch;
    let (mut sink, mut stream) = socket.split();
    loop {
        tokio::select! {
            event = events_rx.recv() => match event {
                Some(ChanEvent::Data { binary, payload }) => {
                    let msg = if binary {
                        Message::Binary(payload.into())
                    } else {
                        match String::from_utf8(payload) {
                            Ok(text) => Message::Text(text.into()),
                            Err(_) => break,
                        }
                    };
                    if sink.send(msg).await.is_err() {
                        break;
                    }
                }
                Some(ChanEvent::Close { code, reason }) => {
                    let _ = sink
                        .send(Message::Close(Some(CloseFrame {
                            code: code.unwrap_or(1000),
                            reason: reason.unwrap_or_default().into(),
                        })))
                        .await;
                    slot.settle();
                    break;
                }
                Some(ChanEvent::End) => continue,
                None => {
                    let _ = sink
                        .send(Message::Close(Some(CloseFrame {
                            code: 1012,
                            reason: "tunnel restarting".to_string().into(),
                        })))
                        .await;
                    slot.settle();
                    break;
                }
            },
            incoming = stream.next() => match incoming {
                Some(Ok(Message::Text(text))) => {
                    if conn
                        .tx
                        .send(TrunkOut::Data(ch, false, text.as_bytes().to_vec()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Some(Ok(Message::Binary(bytes))) => {
                    if conn
                        .tx
                        .send(TrunkOut::Data(ch, true, bytes.to_vec()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Some(Ok(Message::Close(frame))) => {
                    let _ = conn.tx.try_send(TrunkOut::Control(Control::Close {
                        ch,
                        code: frame.as_ref().map(|f| f.code),
                        reason: frame
                            .filter(|f| !f.reason.is_empty())
                            .map(|f| f.reason.to_string()),
                    }));
                    slot.settle();
                    break;
                }
                Some(Ok(_)) => continue,
                Some(Err(_)) | None => break,
            },
        }
    }
    drop(slot);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_id_is_ten_lowercase_base32_chars() {
        for _ in 0..50 {
            let id = random_id();
            assert_eq!(id.len(), ID_LEN);
            assert!(id.bytes().all(|b| ID_ALPHABET.contains(&b)));
        }
        assert_ne!(random_id(), random_id());
    }

    #[test]
    fn random_key_is_32_hex_chars() {
        let key = random_key();
        assert_eq!(key.len(), 32);
        assert!(key.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn collect_headers_drops_ambient_credentials_and_keeps_a_bearer() {
        let mut headers = HeaderMap::new();
        headers.insert(header::ACCEPT, "*/*".parse().unwrap());
        headers.insert(header::COOKIE, "session=example".parse().unwrap());
        headers.insert(header::AUTHORIZATION, "Basic ZXhhbXBsZQ==".parse().unwrap());
        assert_eq!(
            collect_headers(&headers),
            vec![("accept".to_string(), "*/*".to_string())]
        );
        headers.insert(header::AUTHORIZATION, "Bearer example".parse().unwrap());
        assert!(
            collect_headers(&headers)
                .contains(&("authorization".to_string(), "Bearer example".to_string()))
        );
    }

    #[test]
    fn token_comparison_matches_only_the_exact_token() {
        assert!(constant_time_eq("secret", "secret"));
        assert!(!constant_time_eq("secret", "secreT"));
        assert!(!constant_time_eq("secret", "secret2"));
    }

    #[test]
    fn cross_tunnel_response_headers_are_recognised() {
        for name in [
            "Set-Cookie",
            "service-worker-allowed",
            "Clear-Site-Data",
            "NEL",
            "Report-To",
            "Reporting-Endpoints",
            "Strict-Transport-Security",
            "Alt-Svc",
            "Accept-CH",
            "Critical-CH",
            "Origin-Agent-Cluster",
            "Set-Login",
        ] {
            assert!(is_cross_tunnel_response_header(name), "{name}");
        }
        assert!(!is_cross_tunnel_response_header("content-type"));
    }

    #[test]
    fn collect_headers_strips_hop_by_hop() {
        let mut headers = HeaderMap::new();
        headers.insert(header::ACCEPT, "*/*".parse().unwrap());
        headers.insert(header::CONNECTION, "upgrade".parse().unwrap());
        headers.insert(header::UPGRADE, "websocket".parse().unwrap());
        headers.insert(header::SEC_WEBSOCKET_PROTOCOL, "rfc5".parse().unwrap());
        let collected = collect_headers(&headers);
        assert_eq!(collected, vec![("accept".to_string(), "*/*".to_string())]);
    }

    fn agent() -> AgentConn {
        AgentConn {
            tx: mpsc::channel(1).0,
            channels: Arc::new(Mutex::new(HashMap::new())),
            next_ch: Arc::new(AtomicU32::new(0)),
        }
    }

    #[test]
    fn at_the_agent_cap_any_grace_held_allowed_id_is_reused() {
        let st = Arc::new(AppState::new(Config {
            allow_ids: vec!["a".into(), "b".into()],
            max_agents: 1,
            allow_anonymous: true,
            ..Config::default()
        }));
        lock(&st.agents).insert(
            "b".into(),
            AgentEntry {
                resume_key: random_key(),
                epoch: 0,
                conn: None,
            },
        );
        let (id, _, _) = register(&st, agent(), None).unwrap();
        assert_eq!(id, "b", "the free id that adds no entry is the one taken");
        assert_eq!(
            register(&st, agent(), None).unwrap_err(),
            "tunnel capacity reached"
        );
    }
}
