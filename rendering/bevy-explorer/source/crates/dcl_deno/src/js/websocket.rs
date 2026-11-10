use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    net::SocketAddr,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, OnceLock,
    },
    time::Duration,
};

use common::rpc::{RpcCall, RpcResultSender};
use dcl_types::{RpcCalls, SceneResourceCounters};
use deno_core::{
    anyhow::{self, anyhow},
    error::AnyError,
    futures::{
        future::LocalBoxFuture,
        stream::{SplitSink, SplitStream},
        FutureExt, SinkExt, StreamExt,
    },
    op2, AsyncRefCell, CancelFuture, CancelHandle, OpDecl, OpState, RcRef, Resource, ResourceId,
    ToJsBuffer,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::{mpsc, Notify},
    task::JoinHandle,
};
use tokio_tungstenite::{
    tungstenite::{
        self,
        handshake::{
            client::{generate_key, Response},
            derive_accept_key,
            machine::TryParse,
        },
        protocol::{frame::coding::CloseCode, CloseFrame, Role, WebSocketConfig},
        Message,
    },
    MaybeTlsStream, WebSocketStream,
};

use super::plain::Plain;

const MAX_OPEN_SOCKETS: usize = 32;
const MAX_WS_BUFFERED_BYTES: usize = 8 * 1024 * 1024;
/// Charged per queued message on top of its payload, so empty sends also fill the cap.
const MESSAGE_OVERHEAD: usize = 64;
/// Inbound messages of this size or more, per frame or reassembled from fragments, close the
/// socket before the payload is buffered.
const MAX_INBOUND_MESSAGE_BYTES: usize = 1024 * 1024;
/// Received events waiting for the scene to read them. The reader stops reading the socket
/// while this is full, so a socket holds at most this many decoded messages plus the one
/// waiting to be queued.
const INBOUND_QUEUE: usize = 1;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);
/// Response heads larger than this fail the handshake.
const MAX_RESPONSE_HEAD_BYTES: usize = 64 * 1024;
/// How long a socket waits for the peer's close frame, from when its own close frame is
/// written or the peer's arrives, before it is dropped.
const CLOSE_TIMEOUT: Duration = Duration::from_secs(5);
/// How long `close()` waits for queued sends and the close frame to be written before the
/// socket is dropped, so a peer that stops reading still frees the slot.
const CLOSE_DRAIN_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_URL_BYTES: usize = 8 * 1024;
const MAX_PROTOCOLS_BYTES: usize = 4 * 1024;
const USER_AGENT: &str = "DCLExplorer/0.1";

const KIND_TEXT: u16 = 0;
const KIND_BINARY: u16 = 1;
const KIND_CLOSE: u16 = 2;
const KIND_ERROR: u16 = 3;

const CLOSE_NO_STATUS: u16 = 1005;
const CLOSE_ABNORMAL: u16 = 1006;
const CLOSE_PROTOCOL_ERROR: u16 = 1002;

pub fn ops() -> Vec<OpDecl> {
    vec![
        op_ws_create(),
        op_ws_connect(),
        op_ws_send_text(),
        op_ws_send_binary(),
        op_ws_send_binary_ab(),
        op_ws_close(),
        op_ws_next_event(),
        op_ws_get_buffer(),
        op_ws_get_buffer_as_string(),
        op_ws_get_close_code(),
        op_ws_get_error(),
        op_ws_get_buffered_amount(),
    ]
}

/// Looks up the addresses a socket may dial. Every answer is vetted again with
/// [`common::util::validate_public_addrs`] before anything is dialled.
pub(super) trait WsResolver {
    fn resolve(
        &self,
        host: &str,
        port: u16,
        allow_private: bool,
    ) -> LocalBoxFuture<'static, Result<Vec<SocketAddr>, AnyError>>;
}

struct PublicResolver;

impl WsResolver for PublicResolver {
    fn resolve(
        &self,
        host: &str,
        port: u16,
        allow_private: bool,
    ) -> LocalBoxFuture<'static, Result<Vec<SocketAddr>, AnyError>> {
        let host = host.to_owned();
        Box::pin(
            async move { common::util::resolve_public_addrs(&host, port, allow_private).await },
        )
    }
}

/// The network seams of a scene's sockets, replaceable per runtime.
#[derive(Clone)]
pub(super) struct WsNet {
    pub resolver: Rc<dyn WsResolver>,
    pub handshake_timeout: Duration,
    pub close_timeout: Duration,
    pub close_drain_timeout: Duration,
}

impl Default for WsNet {
    fn default() -> Self {
        Self {
            resolver: Rc::new(PublicResolver),
            handshake_timeout: HANDSHAKE_TIMEOUT,
            close_timeout: CLOSE_TIMEOUT,
            close_drain_timeout: CLOSE_DRAIN_TIMEOUT,
        }
    }
}

/// Sockets a scene holds: connecting, open, and closing until the stream is dropped.
#[derive(Clone, Default)]
struct WsSlots(Arc<AtomicUsize>);

/// One of a scene's [`MAX_OPEN_SOCKETS`]. Shared by the resource and the task that owns the
/// stream, and released by whichever lets go first: closing the resource frees it at once,
/// while an aborted task drops its stream only when the runtime next polls it.
struct SlotGuard {
    slots: Arc<AtomicUsize>,
    released: AtomicBool,
}

impl SlotGuard {
    fn release(&self) {
        if !self.released.swap(true, Ordering::SeqCst) {
            self.slots.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

impl Drop for SlotGuard {
    fn drop(&mut self) {
        self.release();
    }
}

fn take_slot(state: &mut OpState) -> Result<Arc<SlotGuard>, AnyError> {
    if !state.has::<WsSlots>() {
        state.put(WsSlots::default());
    }
    let slots = state.borrow::<WsSlots>().0.clone();
    if !add_within(&slots, 1, MAX_OPEN_SOCKETS) {
        anyhow::bail!("WebSocket refused: the scene already holds {MAX_OPEN_SOCKETS} sockets");
    }
    Ok(Arc::new(SlotGuard {
        slots,
        released: AtomicBool::new(false),
    }))
}

/// Adds `add` unless the total would pass `max`.
fn add_within(counter: &AtomicUsize, add: usize, max: usize) -> bool {
    let mut current = counter.load(Ordering::SeqCst);
    loop {
        let Some(next) = current.checked_add(add).filter(|next| *next <= max) else {
            return false;
        };
        match counter.compare_exchange(current, next, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => return true,
            Err(actual) => current = actual,
        }
    }
}

pub(super) fn open_sockets(state: &OpState) -> u64 {
    state
        .try_borrow::<WsSlots>()
        .map_or(0, |slots| slots.0.load(Ordering::SeqCst) as u64)
}

fn count(state: &mut OpState, f: impl FnOnce(&mut SceneResourceCounters)) {
    if let Some(counters) = state.try_borrow_mut::<SceneResourceCounters>() {
        f(counters);
    }
}

struct WsTarget {
    uri: http::Uri,
    /// `scheme://host:port`, what the permission prompt shows.
    origin: String,
    /// The URL host as the resolver takes it, with IPv6 literals bracketed.
    resolve_host: String,
    /// The host for TLS server-name checks, without brackets; `None` for `ws`.
    tls_host: Option<String>,
    port: u16,
    protocols: Vec<String>,
}

fn parse_target(url: &str, protocols: &str, allow_private: bool) -> Result<WsTarget, AnyError> {
    if url.len() > MAX_URL_BYTES {
        anyhow::bail!("WebSocket URL is longer than {MAX_URL_BYTES} bytes");
    }
    let mut parsed = deno_core::url::Url::parse(url)?;
    let secure = match parsed.scheme() {
        "wss" => true,
        "ws" => false,
        _ => anyhow::bail!("WebSocket URL must use ws:// or wss://"),
    };
    if !secure && !allow_private {
        anyhow::bail!("WebSocket URL scheme must be `wss`");
    }
    if parsed.fragment().is_some() {
        anyhow::bail!("WebSocket URL must not have a fragment");
    }
    let host = parsed
        .host_str()
        .filter(|host| !host.is_empty())
        .ok_or_else(|| anyhow!("WebSocket URL has no host"))?
        .to_owned();
    let port = parsed
        .port_or_known_default()
        .filter(|port| *port != 0)
        .ok_or_else(|| anyhow!("WebSocket URL has no usable port"))?;
    // credentials in the URL are never sent
    let _ = parsed.set_username("");
    let _ = parsed.set_password(None);
    let uri: http::Uri = parsed.as_str().parse()?;
    let origin = format!("{}://{host}:{port}", parsed.scheme());

    if protocols.len() > MAX_PROTOCOLS_BYTES {
        anyhow::bail!("WebSocket protocols are longer than {MAX_PROTOCOLS_BYTES} bytes");
    }
    let protocols: Vec<String> = if protocols.is_empty() {
        Vec::new()
    } else {
        protocols.split(", ").map(str::to_owned).collect()
    };
    for (i, protocol) in protocols.iter().enumerate() {
        if protocol.is_empty() || !protocol.bytes().all(is_token_byte) {
            anyhow::bail!("invalid WebSocket protocol `{protocol}`");
        }
        if protocols[..i]
            .iter()
            .any(|other| other.eq_ignore_ascii_case(protocol))
        {
            anyhow::bail!("WebSocket protocol `{protocol}` is repeated");
        }
    }

    let tls_host = secure.then(|| {
        host.strip_prefix('[')
            .and_then(|h| h.strip_suffix(']'))
            .unwrap_or(&host)
            .to_owned()
    });
    Ok(WsTarget {
        uri,
        origin,
        resolve_host: host,
        tls_host,
        port,
        protocols,
    })
}

// RFC 9110 tchar
fn is_token_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b)
}

enum ConnectError {
    /// Policy: the user, the address policy or the scene's limits.
    Refused(AnyError),
    /// The network: resolution, connect, TLS, the handshake, or the timeout.
    Failed(AnyError),
}

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct Connected {
    socket: Socket,
    protocol: String,
}

enum Outbound {
    Message(Message, usize),
    Close(Option<CloseFrame>),
}

enum Inbound {
    Text(String),
    Binary(Vec<u8>),
    Close(u16, String),
    Error(String),
}

/// Outbound messages queued but not yet written.
#[derive(Default)]
struct Backlog {
    /// Payload bytes, as `bufferedAmount` reports them.
    bytes: AtomicUsize,
    /// Payload bytes plus [`MESSAGE_OVERHEAD`] per message, held under the cap.
    charged: AtomicUsize,
}

impl Backlog {
    fn release(&self, len: usize) {
        self.bytes.fetch_sub(len, Ordering::SeqCst);
        self.charged
            .fetch_sub(len + MESSAGE_OVERHEAD, Ordering::SeqCst);
    }
}

/// A scene socket. Dropping or closing the resource drops the connection and frees its slot.
struct WsResource {
    connect: RefCell<Option<LocalBoxFuture<'static, Result<Connected, ConnectError>>>>,
    connect_cancel: CancelHandle,
    slot: RefCell<Option<Arc<SlotGuard>>>,
    outbound: RefCell<Option<mpsc::UnboundedSender<Outbound>>>,
    inbound: AsyncRefCell<Option<mpsc::Receiver<Inbound>>>,
    inbound_cancel: CancelHandle,
    draining: Arc<Notify>,
    close_requested: Cell<bool>,
    backlog: Arc<Backlog>,
    pump: RefCell<Option<JoinHandle<()>>>,
    text: RefCell<Option<String>>,
    binary: RefCell<Option<Vec<u8>>>,
    close_code: Cell<u16>,
    message: RefCell<String>,
}

impl WsResource {
    fn shut_down(&self) {
        self.connect_cancel.cancel();
        self.inbound_cancel.cancel();
        self.connect.borrow_mut().take();
        self.outbound.borrow_mut().take();
        if let Some(pump) = self.pump.borrow_mut().take() {
            pump.abort();
        }
        if let Some(slot) = self.slot.borrow_mut().take() {
            slot.release();
        }
    }
}

impl Resource for WsResource {
    fn name(&self) -> Cow<'_, str> {
        "webSocket".into()
    }

    fn close(self: Rc<Self>) {
        self.shut_down();
    }
}

impl Drop for WsResource {
    fn drop(&mut self) {
        if let Some(pump) = self.pump.get_mut().take() {
            pump.abort();
        }
        if let Some(slot) = self.slot.get_mut().take() {
            slot.release();
        }
    }
}

/// Validates the URL and the scene's allowance, takes a slot and asks the user. The connection
/// itself is made by [`op_ws_connect`]; closing the returned resource cancels it.
#[op2(fast)]
#[smi]
pub fn op_ws_create(
    state: &mut OpState,
    #[string] url: String,
    #[string] protocols: String,
) -> Result<ResourceId, AnyError> {
    let result = create(state, &url, &protocols);
    if result.is_err() {
        count(state, |c| c.ws_refused += 1);
    }
    result
}

fn create(state: &mut OpState, url: &str, protocols: &str) -> Result<ResourceId, AnyError> {
    let allow_private = super::fetch::allow_private_network(state);
    let target = parse_target(url, protocols, allow_private)?;
    let net = state.try_borrow::<WsNet>().cloned().unwrap_or_default();
    let slot = take_slot(state)?;

    let (sx, rx) = RpcResultSender::channel();
    state
        .try_borrow_mut::<RpcCalls>()
        .ok_or_else(|| anyhow!("WebSocket is unavailable before the scene starts"))?
        .push(RpcCall::RequestGenericPermission {
            ty: common::structs::PermissionType::Websocket,
            message: Some(target.origin.clone()),
            response: sx,
        })?;

    let connect = async move {
        match rx.await {
            Ok(true) => {}
            Ok(false) => return Err(ConnectError::Refused(anyhow!("User denied the WebSocket"))),
            Err(_) => {
                return Err(ConnectError::Failed(anyhow!(
                    "the WebSocket permission request was dropped"
                )))
            }
        }
        if tokio::runtime::Handle::try_current().is_err() {
            return Err(ConnectError::Failed(anyhow!("no network runtime")));
        }
        tokio::time::timeout(net.handshake_timeout, dial(target, &net, allow_private))
            .await
            .unwrap_or_else(|_| {
                Err(ConnectError::Failed(anyhow!(
                    "WebSocket handshake timed out"
                )))
            })
    };

    let resource = WsResource {
        connect: RefCell::new(Some(connect.boxed_local())),
        connect_cancel: CancelHandle::new(),
        slot: RefCell::new(Some(slot)),
        outbound: RefCell::new(None),
        inbound: AsyncRefCell::new(None),
        inbound_cancel: CancelHandle::new(),
        draining: Arc::new(Notify::new()),
        close_requested: Cell::new(false),
        backlog: Arc::default(),
        pump: RefCell::new(None),
        text: RefCell::new(None),
        binary: RefCell::new(None),
        close_code: Cell::new(CLOSE_ABNORMAL),
        message: RefCell::new(String::new()),
    };
    Ok(state.resource_table.add(resource))
}

async fn dial(
    target: WsTarget,
    net: &WsNet,
    allow_private: bool,
) -> Result<Connected, ConnectError> {
    // one resolution per socket; exactly these addresses are dialled
    let addrs = net
        .resolver
        .resolve(&target.resolve_host, target.port, allow_private)
        .await
        .map_err(|e| {
            if e.is::<common::util::NonPublicAddress>() {
                ConnectError::Refused(e)
            } else {
                ConnectError::Failed(e)
            }
        })?;
    common::util::validate_public_addrs(&addrs, allow_private).map_err(ConnectError::Refused)?;
    if addrs.is_empty() {
        return Err(ConnectError::Failed(anyhow!(
            "WebSocket host did not resolve"
        )));
    }
    let failed = |e: AnyError| ConnectError::Failed(e);
    let tcp = TcpStream::connect(&addrs[..])
        .await
        .map_err(|e| failed(e.into()))?;
    let stream = match &target.tls_host {
        Some(host) => {
            let tls = tls_connector()
                .map_err(failed)?
                .connect(host, tcp)
                .await
                .map_err(|e| failed(e.into()))?;
            MaybeTlsStream::NativeTls(tls)
        }
        None => MaybeTlsStream::Plain(tcp),
    };

    upgrade(stream, &target).await.map_err(failed)
}

/// The client side of the opening handshake (RFC 6455 section 4). A server that selects none
/// of the requested subprotocols is accepted, and the scene sees an empty `protocol`.
async fn upgrade(
    mut stream: MaybeTlsStream<TcpStream>,
    target: &WsTarget,
) -> Result<Connected, AnyError> {
    let key = generate_key();
    let path = target.uri.path_and_query().map_or("/", |p| p.as_str());
    let host = target
        .uri
        .authority()
        .ok_or_else(|| anyhow!("WebSocket URL has no host"))?;
    // the path is URL-serialized and the protocols are checked tokens, so neither holds CR or LF
    let mut request = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\
         Sec-WebSocket-Version: 13\r\nSec-WebSocket-Key: {key}\r\nUser-Agent: {USER_AGENT}\r\n"
    );
    if !target.protocols.is_empty() {
        request.push_str(&format!(
            "Sec-WebSocket-Protocol: {}\r\n",
            target.protocols.join(", ")
        ));
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes()).await?;
    stream.flush().await?;

    let mut buf = Vec::new();
    let (head_len, response) = loop {
        if let Some(parsed) = Response::try_parse(&buf)? {
            break parsed;
        }
        if buf.len() > MAX_RESPONSE_HEAD_BYTES {
            anyhow::bail!("WebSocket handshake response is too large");
        }
        let mut chunk = [0u8; 4096];
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            anyhow::bail!("WebSocket server closed the connection during the handshake");
        }
        buf.extend_from_slice(&chunk[..read]);
    };
    let protocol = verify_response(&response, &key, &target.protocols)?;
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_INBOUND_MESSAGE_BYTES - 1))
        .max_frame_size(Some(MAX_INBOUND_MESSAGE_BYTES - 1));
    let tail = buf.split_off(head_len);
    let socket =
        WebSocketStream::from_partially_read(stream, tail, Role::Client, Some(config)).await;
    Ok(Connected { socket, protocol })
}

/// Checks the server's handshake response and returns the subprotocol it selected, or "".
fn verify_response(
    response: &Response,
    key: &str,
    requested: &[String],
) -> Result<String, AnyError> {
    if response.status() != http::StatusCode::SWITCHING_PROTOCOLS {
        anyhow::bail!("WebSocket handshake failed: HTTP {}", response.status());
    }
    let headers = response.headers();
    let single = |name: &str| -> Result<Option<&str>, AnyError> {
        let mut values = headers.get_all(name).iter();
        let value = values.next();
        if values.next().is_some() {
            anyhow::bail!("WebSocket handshake response repeats {name}");
        }
        value
            .map(|v| {
                v.to_str()
                    .map_err(|_| anyhow!("WebSocket handshake response has an invalid {name}"))
            })
            .transpose()
    };
    if !single("upgrade")?.is_some_and(|v| v.eq_ignore_ascii_case("websocket")) {
        anyhow::bail!("WebSocket handshake response does not upgrade to websocket");
    }
    let connection = headers
        .get_all("connection")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .any(|token| token.trim().eq_ignore_ascii_case("upgrade"));
    if !connection {
        anyhow::bail!("WebSocket handshake response does not upgrade the connection");
    }
    if single("sec-websocket-accept")? != Some(derive_accept_key(key.as_bytes()).as_str()) {
        anyhow::bail!("WebSocket handshake response has the wrong accept key");
    }
    // no extensions are offered, so any in the response was not requested
    if headers.contains_key("sec-websocket-extensions") {
        anyhow::bail!("WebSocket server selected an extension that was not requested");
    }
    match single("sec-websocket-protocol")? {
        None => Ok(String::new()),
        Some(protocol) if requested.iter().any(|r| r == protocol) => Ok(protocol.to_owned()),
        Some(_) => anyhow::bail!("WebSocket server selected a subprotocol that was not requested"),
    }
}

/// One connector for every socket, so the system roots are loaded once.
fn tls_connector() -> Result<tokio_native_tls::TlsConnector, AnyError> {
    static CONNECTOR: OnceLock<Result<native_tls::TlsConnector, String>> = OnceLock::new();
    CONNECTOR
        .get_or_init(|| {
            native_tls::TlsConnector::builder()
                .min_protocol_version(Some(native_tls::Protocol::Tlsv12))
                .build()
                .map_err(|e| e.to_string())
        })
        .clone()
        .map(Into::into)
        .map_err(|e| anyhow!("TLS is unavailable: {e}"))
}

fn get_resource(state: &OpState, rid: ResourceId) -> Result<Rc<WsResource>, AnyError> {
    state.resource_table.get::<WsResource>(rid)
}

/// Completes the connection [`op_ws_create`] started and returns the agreed protocol.
#[op2(async)]
#[string]
pub async fn op_ws_connect(
    state: Rc<RefCell<OpState>>,
    #[smi] rid: ResourceId,
) -> Result<String, AnyError> {
    let resource = get_resource(&state.borrow(), rid)?;
    let connect = resource
        .connect
        .borrow_mut()
        .take()
        .ok_or_else(|| anyhow!("WebSocket is not connecting"))?;
    let result = std::panic::AssertUnwindSafe(connect)
        .catch_unwind()
        .or_cancel(RcRef::map(&resource, |r| &r.connect_cancel))
        .await;
    let connected = match result {
        Ok(Ok(Ok(connected))) => connected,
        Ok(Ok(Err(ConnectError::Refused(e)))) => {
            resource.shut_down();
            count(&mut state.borrow_mut(), |c| c.ws_refused += 1);
            return Err(e);
        }
        Ok(Ok(Err(ConnectError::Failed(e)))) => {
            resource.shut_down();
            count(&mut state.borrow_mut(), |c| c.ws_failed += 1);
            return Err(e);
        }
        Ok(Err(_)) => {
            resource.shut_down();
            count(&mut state.borrow_mut(), |c| c.ws_failed += 1);
            anyhow::bail!("WebSocket handshake failed");
        }
        Err(_) => {
            resource.shut_down();
            anyhow::bail!("WebSocket was closed before the connection was established");
        }
    };

    let Some(slot) = resource.slot.borrow().clone() else {
        anyhow::bail!("WebSocket was closed before the connection was established");
    };
    let handle = tokio::runtime::Handle::try_current()?;
    let (outbound_sx, outbound_rx) = mpsc::unbounded_channel();
    let (inbound_sx, inbound_rx) = mpsc::channel(INBOUND_QUEUE);
    let net = state
        .borrow()
        .try_borrow::<WsNet>()
        .cloned()
        .unwrap_or_default();
    let pump = handle.spawn(pump(
        connected.socket,
        outbound_rx,
        inbound_sx,
        resource.backlog.clone(),
        resource.draining.clone(),
        (net.close_timeout, net.close_drain_timeout),
        slot,
    ));
    *resource.pump.borrow_mut() = Some(pump);
    *resource.outbound.borrow_mut() = Some(outbound_sx);
    *RcRef::map(&resource, |r| &r.inbound).borrow_mut().await = Some(inbound_rx);
    count(&mut state.borrow_mut(), |c| c.ws_opened += 1);
    Ok(connected.protocol)
}

/// Moves frames between the socket and the resource until the connection ends, the resource
/// goes away, a close handshake outlives `close_timeout`, or `close()` outlives
/// `close_drain_timeout`. Dropping it drops the socket.
async fn pump(
    socket: Socket,
    outbound: mpsc::UnboundedReceiver<Outbound>,
    inbound: mpsc::Sender<Inbound>,
    backlog: Arc<Backlog>,
    draining: Arc<Notify>,
    (close_timeout, close_drain_timeout): (Duration, Duration),
    _slot: Arc<SlotGuard>,
) {
    let (sink, stream) = socket.split();
    let closing = Notify::new();
    let linger = async {
        closing.notified().await;
        tokio::time::sleep(close_timeout).await;
    };
    let drain = async {
        draining.notified().await;
        tokio::time::sleep(close_drain_timeout).await;
    };
    tokio::select! {
        _ = read(stream, inbound.clone(), &closing) => {}
        _ = write(sink, outbound, inbound, backlog, &closing) => {}
        _ = linger => {}
        _ = drain => {}
    }
}

async fn read(mut stream: SplitStream<Socket>, inbound: mpsc::Sender<Inbound>, closing: &Notify) {
    while let Some(message) = stream.next().await {
        let event = match message {
            Ok(Message::Text(text)) => Inbound::Text(text.as_str().to_owned()),
            Ok(Message::Binary(data)) => Inbound::Binary(data.to_vec()),
            Ok(Message::Close(frame)) => {
                closing.notify_one();
                let (code, reason) = match frame {
                    Some(frame) => (u16::from(frame.code), frame.reason.as_str().to_owned()),
                    None => (CLOSE_NO_STATUS, String::new()),
                };
                Inbound::Close(received_close_code(code), reason)
            }
            Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_)) => continue,
            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => break,
            Err(e) => {
                let _ = inbound.send(Inbound::Error(e.to_string())).await;
                return;
            }
        };
        if inbound.send(event).await.is_err() {
            return;
        }
    }
}

async fn write(
    mut sink: SplitSink<Socket, Message>,
    mut outbound: mpsc::UnboundedReceiver<Outbound>,
    inbound: mpsc::Sender<Inbound>,
    backlog: Arc<Backlog>,
    closing: &Notify,
) {
    while let Some(next) = outbound.recv().await {
        let result = match next {
            Outbound::Message(message, len) => {
                let result = sink.send(message).await;
                backlog.release(len);
                result
            }
            Outbound::Close(frame) => {
                let result = sink.send(Message::Close(frame)).await;
                closing.notify_one();
                result
            }
        };
        match result {
            Ok(()) => {}
            // the reader reports the end of the connection
            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {}
            Err(e) => {
                let _ = inbound.send(Inbound::Error(e.to_string())).await;
                return;
            }
        }
    }
}

/// A received close code a scene may see: no code is 1005, and a code that may not appear on
/// the wire is a protocol error.
fn received_close_code(code: u16) -> u16 {
    match code {
        CLOSE_NO_STATUS => CLOSE_NO_STATUS,
        1004 | 1006 | 1015 => CLOSE_PROTOCOL_ERROR,
        1000..=4999 => code,
        _ => CLOSE_PROTOCOL_ERROR,
    }
}

/// Queues a message of `len` payload bytes; `message` is built only once the send is accepted.
fn send(
    state: &mut OpState,
    rid: ResourceId,
    len: usize,
    message: impl FnOnce() -> Message,
) -> Result<(), AnyError> {
    let resource = get_resource(state, rid)?;
    if resource.close_requested.get() {
        anyhow::bail!("WebSocket is closing");
    }
    let outbound = resource.outbound.borrow();
    let outbound = outbound
        .as_ref()
        .ok_or_else(|| anyhow!("WebSocket is not open"))?;
    // Refuse a send that would push still-pending outbound bytes past the cap, so a scene
    // writing faster than the peer drains cannot grow the sidecar heap without bound.
    if !add_within(
        &resource.backlog.charged,
        len.saturating_add(MESSAGE_OVERHEAD),
        MAX_WS_BUFFERED_BYTES,
    ) {
        anyhow::bail!(
            "WebSocket send refused: outbound buffer exceeds {MAX_WS_BUFFERED_BYTES} bytes"
        );
    }
    resource.backlog.bytes.fetch_add(len, Ordering::SeqCst);
    if outbound.send(Outbound::Message(message(), len)).is_err() {
        resource.backlog.release(len);
        anyhow::bail!("WebSocket is closed");
    }
    count(state, |c| {
        c.ws_msgs_out += 1;
        c.ws_bytes_out += len as u64;
    });
    Ok(())
}

#[op2(fast)]
pub fn op_ws_send_text(
    state: &mut OpState,
    #[smi] rid: ResourceId,
    #[string] data: String,
) -> Result<(), AnyError> {
    send(state, rid, data.len(), || Message::text(data))
}

#[op2]
pub fn op_ws_send_binary(
    state: &mut OpState,
    #[smi] rid: ResourceId,
    #[anybuffer] data: &[u8],
) -> Result<(), AnyError> {
    send(state, rid, data.len(), || Message::binary(data.to_vec()))
}

#[op2(fast)]
pub fn op_ws_send_binary_ab(
    state: &mut OpState,
    #[smi] rid: ResourceId,
    #[arraybuffer] data: &[u8],
) -> Result<(), AnyError> {
    send(state, rid, data.len(), || Message::binary(data.to_vec()))
}

/// Starts the closing handshake once [`op_ws_connect`] has finished, even if the scene has
/// not yet seen that result; before then it cancels the connection. The socket is dropped
/// when the peer answers or after the close timeout.
#[op2]
pub fn op_ws_close(
    state: &mut OpState,
    #[smi] rid: ResourceId,
    #[from_v8] code: Plain<Option<f64>>,
    #[from_v8] reason: Plain<Option<String>>,
) -> Result<(), AnyError> {
    let code = match code.0 {
        None => None,
        Some(code)
            if code == 1000.0 || (code.fract() == 0.0 && (3000.0..5000.0).contains(&code)) =>
        {
            Some(code as u16)
        }
        Some(_) => {
            anyhow::bail!("The close code must be either 1000 or in the range of 3000 to 4999.")
        }
    };
    let reason = reason.0.unwrap_or_default();
    if reason.len() > 123 {
        anyhow::bail!("The close reason may not be longer than 123 bytes.");
    }
    let resource = get_resource(state, rid)?;
    let outbound = resource.outbound.borrow().clone();
    let Some(outbound) = outbound else {
        // still connecting: abandon the handshake and free the slot now
        resource.shut_down();
        return Ok(());
    };
    if resource.close_requested.replace(true) {
        return Ok(());
    }
    let frame = match (code, reason.is_empty()) {
        (None, true) => None,
        (code, _) => Some(CloseFrame {
            code: CloseCode::from(code.unwrap_or(1000)),
            reason: reason.into(),
        }),
    };
    resource.draining.notify_one();
    let _ = outbound.send(Outbound::Close(frame));
    Ok(())
}

/// Waits for the next event and returns its kind: 0 text, 1 binary, 2 close, 3 error. The
/// payload, close code and reason or error are read with the getters below.
#[op2(async)]
pub async fn op_ws_next_event(
    state: Rc<RefCell<OpState>>,
    #[smi] rid: ResourceId,
) -> Result<u16, AnyError> {
    let resource = get_resource(&state.borrow(), rid)?;
    let event = {
        let mut inbound = RcRef::map(&resource, |r| &r.inbound).borrow_mut().await;
        let Some(receiver) = &mut *inbound else {
            anyhow::bail!("WebSocket is not open");
        };
        receiver
            .recv()
            .or_cancel(RcRef::map(&resource, |r| &r.inbound_cancel))
            .await
    };
    let kind = match event {
        Ok(Some(Inbound::Text(text))) => {
            count(&mut state.borrow_mut(), |c| {
                c.ws_msgs_in += 1;
                c.ws_bytes_in += text.len() as u64;
            });
            *resource.text.borrow_mut() = Some(text);
            KIND_TEXT
        }
        Ok(Some(Inbound::Binary(data))) => {
            count(&mut state.borrow_mut(), |c| {
                c.ws_msgs_in += 1;
                c.ws_bytes_in += data.len() as u64;
            });
            *resource.binary.borrow_mut() = Some(data);
            KIND_BINARY
        }
        Ok(Some(Inbound::Close(code, reason))) => {
            resource.close_code.set(code);
            *resource.message.borrow_mut() = reason;
            KIND_CLOSE
        }
        Ok(Some(Inbound::Error(message))) => {
            *resource.message.borrow_mut() = message;
            KIND_ERROR
        }
        // the connection ended without a close frame
        Ok(None) | Err(_) => {
            resource.close_code.set(CLOSE_ABNORMAL);
            resource.message.borrow_mut().clear();
            KIND_CLOSE
        }
    };
    Ok(kind)
}

#[op2]
#[serde]
pub fn op_ws_get_buffer(
    state: &mut OpState,
    #[smi] rid: ResourceId,
) -> Result<Option<ToJsBuffer>, AnyError> {
    let resource = get_resource(state, rid)?;
    let data = resource.binary.borrow_mut().take();
    Ok(data.map(ToJsBuffer::from))
}

#[op2]
#[string]
pub fn op_ws_get_buffer_as_string(
    state: &mut OpState,
    #[smi] rid: ResourceId,
) -> Result<Option<String>, AnyError> {
    let resource = get_resource(state, rid)?;
    let text = resource.text.borrow_mut().take();
    Ok(text)
}

#[op2(fast)]
#[smi]
pub fn op_ws_get_close_code(state: &mut OpState, #[smi] rid: ResourceId) -> Result<u32, AnyError> {
    Ok(get_resource(state, rid)?.close_code.get().into())
}

/// The close reason after a close event, or the error message after an error event.
#[op2]
#[string]
pub fn op_ws_get_error(state: &mut OpState, #[smi] rid: ResourceId) -> Result<String, AnyError> {
    let resource = get_resource(state, rid)?;
    let message = std::mem::take(&mut *resource.message.borrow_mut());
    Ok(message)
}

#[op2(fast)]
#[smi]
pub fn op_ws_get_buffered_amount(
    state: &mut OpState,
    #[smi] rid: ResourceId,
) -> Result<u32, AnyError> {
    let buffered = get_resource(state, rid)?
        .backlog
        .bytes
        .load(Ordering::SeqCst);
    Ok(u32::try_from(buffered).unwrap_or(u32::MAX))
}
