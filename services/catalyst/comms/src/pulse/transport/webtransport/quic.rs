// from: catalyrst/third_party/web_transport/src/host.rs (decentraland/rust-web-transport c5416501 + web_transport.patch; Apache-2.0, LICENSE in that tree)
//! The WebTransport host: binds a QUIC endpoint, accepts WebTransport sessions, and pumps
//! the reliable bidi stream and unreliable datagrams to/from a synchronously-polled event
//! queue. Mirrors ENet's `enet_host_service` model so the .NET threading model is unchanged.
//!
//! This crate is a *generic* byte pipe: it does no framing or sequencing. Pulse channel
//! semantics (length framing on the stream, datagram sequencing/dedup) live in the C# layer.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use web_transport_quinn::{Request, Session, quinn};

/// Buffer size for a single reliable-stream read. Raw chunks are forwarded as-is.
const STREAM_READ_BUF: usize = 16 * 1024;
const STREAM_SEND_WINDOW: u64 = 64 * 1024;

/// Bidirectional streams a client may hold open: the HTTP/3 CONNECT stream that carries the
/// session and the one reliable stream Pulse reads, with headroom for a client that opens
/// another. The host never accepts a second Pulse stream; [`CONNECTION_RECEIVE_WINDOW`] caps
/// what unread ones hold.
const MAX_CONCURRENT_BIDI_STREAMS: u32 = 4;
/// Unidirectional streams a client may hold open: the HTTP/3 control stream and the two QPACK
/// streams (RFC 9114 section 6.2), plus room for reserved stream types a browser may open
/// first, in any order. Pulse itself uses none; [`CONNECTION_RECEIVE_WINDOW`] caps what they
/// hold.
const MAX_CONCURRENT_UNI_STREAMS: u32 = 16;
/// Per-stream receive window: sixteen default-size (4 KiB) Pulse messages in flight. The
/// reliable stream is drained as it arrives, so this bounds what an unread stream can hold.
const STREAM_RECEIVE_WINDOW: u32 = 64 * 1024;
/// Receive window shared by every stream of one connection: the cap on stream data a
/// connection buffers, however many streams it opens.
const CONNECTION_RECEIVE_WINDOW: u32 = 4 * STREAM_RECEIVE_WINDOW;
/// Inbound datagrams buffered before the oldest is dropped: about fifty 1200-byte Pulse
/// datagrams. The maximum datagram frame size it advertises stays at its 65535-byte cap.
const DATAGRAM_RECEIVE_BUFFER: usize = 64 * 1024;
/// How long a connection may stay silent before it is closed (the QUIC default, stated
/// explicitly).
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// Settings for constructing a [`Host`].
pub struct HostConfig {
    /// UDP socket address to bind the QUIC endpoint to. Use port `0` to let the OS choose.
    pub bind_addr: SocketAddr,
    /// PEM-encoded certificate chain.
    pub cert_pem: String,
    /// PEM-encoded PKCS#8 / SEC1 private key.
    pub key_pem: String,
    /// How long a connection may stay silent before it is closed; [`IDLE_TIMEOUT`] in
    /// production.
    pub idle_timeout: Duration,
}

/// Errors returned by [`Host::new`].
#[derive(Debug)]
pub enum HostError {
    /// The certificate or key PEM could not be parsed.
    Cert(String),
    /// The QUIC endpoint could not be created or bound.
    Endpoint(String),
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HostError::Cert(m) => write!(f, "certificate error: {m}"),
            HostError::Endpoint(m) => write!(f, "endpoint error: {m}"),
        }
    }
}

impl std::error::Error for HostError {}

/// An event drained from the host, mirroring ENet's `enet_host_service` event model.
#[derive(Debug, Clone)]
pub enum Event {
    /// A new WebTransport session was established. `remote_addr` is the peer's `ip:port`.
    Connect { peer_id: u64, remote_addr: String },
    /// A session ended (peer closed, transport error, or local disconnect).
    Disconnect { peer_id: u64, reason: u32 },
    /// Raw bytes arrived on the peer's reliable bidi stream (unframed).
    StreamData { peer_id: u64, data: Bytes },
    /// An unreliable datagram arrived from the peer.
    Datagram { peer_id: u64, data: Bytes },
}

/// Commands sent from the polling thread into a connection's async task.
enum PeerCommand {
    /// Write raw bytes to the peer's reliable bidi stream.
    Stream(Bytes),
}

/// Per-connection state retained by the host so the polling thread can send to a peer.
struct PeerHandle {
    /// A clone of the session, used for the sync send-datagram / rtt / close paths.
    session: Session,
    /// Channel into the connection's writer task for the async reliable-stream path.
    cmd_tx: UnboundedSender<PeerCommand>,
}

/// A running WebTransport server. Construct with [`Host::new`], then drive it by repeatedly
/// calling [`Host::service`] from a single dedicated thread (the ENet model). The other
/// methods may be called from that same thread between `service` calls.
pub struct Host {
    runtime: Option<Runtime>,
    events_rx: Receiver<Event>,
    peers: Arc<Mutex<HashMap<u64, PeerHandle>>>,
    local_addr: SocketAddr,
}

impl Host {
    /// Bind the QUIC endpoint, start the tokio runtime, and begin accepting sessions.
    pub fn new(config: HostConfig) -> Result<Host, HostError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| HostError::Endpoint(e.to_string()))?;

        let (certs, key) = parse_pem(&config.cert_pem, &config.key_pem)?;
        let bind_addr = config.bind_addr;

        let server_config = server_config(certs, key, config.idle_timeout)?;
        // Binding a quinn endpoint needs a runtime context.
        let endpoint = runtime
            .block_on(async move { quinn::Endpoint::server(server_config, bind_addr) })
            .map_err(|e| HostError::Endpoint(e.to_string()))?;
        let server = web_transport_quinn::Server::new(endpoint);

        let local_addr = server
            .local_addr()
            .map_err(|e| HostError::Endpoint(e.to_string()))?;

        let (events_tx, events_rx) = std::sync::mpsc::channel::<Event>();
        let peers: Arc<Mutex<HashMap<u64, PeerHandle>>> = Arc::new(Mutex::new(HashMap::new()));
        let accept_peers = peers.clone();

        // Accept loop: assign sequential peer ids and serve each session on its own task.
        crate::shared::task::spawn_on(runtime.handle(), async move {
            let mut server = server;
            let mut next_id: u64 = 1;
            while let Some(request) = server.accept().await {
                let peer_id = next_id;
                next_id += 1;
                crate::shared::task::spawn(serve_connection(
                    request,
                    peer_id,
                    events_tx.clone(),
                    accept_peers.clone(),
                ));
            }
        });

        Ok(Host {
            runtime: Some(runtime),
            events_rx,
            peers,
            local_addr,
        })
    }

    /// The address the endpoint is bound to, with the OS-assigned port resolved.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Block up to `timeout` for the next event, mirroring `enet_host_service`.
    /// Returns `None` if no event arrived within `timeout`.
    pub fn service(&mut self, timeout: Duration) -> Option<Event> {
        self.events_rx.recv_timeout(timeout).ok()
    }

    /// Queue raw bytes for the peer's reliable bidi stream. Returns `false` if the peer is gone.
    pub fn send_stream(&self, peer_id: u64, data: &[u8]) -> bool {
        self.send_stream_bytes(peer_id, Bytes::copy_from_slice(data))
    }

    /// Retains the byte owner's allocation and queue permits until the stream write completes.
    pub fn send_stream_bytes(&self, peer_id: u64, data: Bytes) -> bool {
        let peers = self
            .peers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match peers.get(&peer_id) {
            Some(handle) => handle.cmd_tx.send(PeerCommand::Stream(data)).is_ok(),
            None => false,
        }
    }

    /// Send an unreliable datagram to the peer. Returns `false` on failure (gone / too large).
    pub fn send_datagram(&self, peer_id: u64, data: &[u8]) -> bool {
        let peers = self
            .peers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match peers.get(&peer_id) {
            Some(handle) => handle
                .session
                .send_datagram(Bytes::copy_from_slice(data))
                .is_ok(),
            None => false,
        }
    }

    /// Close the peer's session with an application error code. Returns `false` if gone.
    pub fn disconnect(&self, peer_id: u64, reason: u32) -> bool {
        let peers = self
            .peers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match peers.get(&peer_id) {
            Some(handle) => {
                // `Session::close` must run inside the tokio runtime context.
                if let Some(runtime) = self.runtime.as_ref() {
                    let _guard = runtime.enter();
                    handle.session.close(reason, b"");
                }
                true
            }
            None => false,
        }
    }

    /// Current smoothed RTT to the peer in microseconds, if still connected.
    pub fn rtt_us(&self, peer_id: u64) -> Option<u64> {
        let peers = self
            .peers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        peers
            .get(&peer_id)
            .map(|handle| handle.session.rtt().as_micros() as u64)
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            // Don't block teardown indefinitely on in-flight connection tasks.
            runtime.shutdown_timeout(Duration::from_millis(200));
        }
    }
}

/// Establish one session, publish its lifecycle/data as [`Event`]s, and pump it until close.
async fn serve_connection(
    request: Request,
    peer_id: u64,
    events_tx: Sender<Event>,
    peers: Arc<Mutex<HashMap<u64, PeerHandle>>>,
) {
    let session = match request.ok().await {
        Ok(session) => session,
        Err(_) => return,
    };
    session.set_send_window(STREAM_SEND_WINDOW);
    let remote_addr = session.remote_address().to_string();

    let (cmd_tx, cmd_rx) = unbounded_channel::<PeerCommand>();
    peers
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(
            peer_id,
            PeerHandle {
                session: session.clone(),
                cmd_tx,
            },
        );
    let _ = events_tx.send(Event::Connect {
        peer_id,
        remote_addr,
    });

    // Unreliable datagrams in.
    crate::shared::task::spawn(datagram_loop(session.clone(), peer_id, events_tx.clone()));
    // Reliable bidi stream: the peer opens it; we forward raw chunks and write queued bytes.
    crate::shared::task::spawn(stream_loop(
        session.clone(),
        peer_id,
        events_tx.clone(),
        cmd_rx,
    ));

    // This task is the disconnect monitor.
    let _ = session.closed().await;
    let _ = events_tx.send(Event::Disconnect { peer_id, reason: 0 });
    peers
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(&peer_id);
}

/// Forward inbound datagrams as [`Event::Datagram`] until the session closes.
async fn datagram_loop(session: Session, peer_id: u64, events_tx: Sender<Event>) {
    while let Ok(data) = session.read_datagram().await {
        if events_tx.send(Event::Datagram { peer_id, data }).is_err() {
            break;
        }
    }
}

/// Accept the peer's reliable bidi stream, forward inbound chunks as [`Event::StreamData`],
/// and write outbound [`PeerCommand::Stream`] bytes.
async fn stream_loop(
    session: Session,
    peer_id: u64,
    events_tx: Sender<Event>,
    mut cmd_rx: UnboundedReceiver<PeerCommand>,
) {
    let (mut send, mut recv) = match tokio::select! {
        stream = session.accept_bi() => stream,
        _ = session.closed() => return,
    } {
        Ok(pair) => pair,
        Err(_) => return,
    };

    let reader = crate::shared::task::spawn(async move {
        let mut buf = vec![0u8; STREAM_READ_BUF];
        while let Ok(Some(n)) = recv.read(&mut buf).await {
            let data = Bytes::copy_from_slice(&buf[..n]);
            if events_tx.send(Event::StreamData { peer_id, data }).is_err() {
                break;
            }
        }
    });

    loop {
        let command = tokio::select! {
            command = cmd_rx.recv() => command,
            _ = session.closed() => break,
        };
        let Some(PeerCommand::Stream(bytes)) = command else {
            break;
        };
        let written = tokio::select! {
            result = send.write_all(&bytes) => result.is_ok(),
            _ = session.closed() => false,
        };
        if !written {
            session.close(0, b"stream write failed");
            break;
        }
    }

    reader.abort();
}

/// The QUIC server configuration: TLS 1.3 with the HTTP/3 ALPN, as `web-transport-quinn`'s
/// builder sets it up, plus [`transport_config`]'s stream and buffer limits.
fn server_config(
    certs: Vec<rustls_pki_types::CertificateDer<'static>>,
    key: rustls_pki_types::PrivateKeyDer<'static>,
    idle_timeout: Duration,
) -> Result<quinn::ServerConfig, HostError> {
    let mut tls = rustls::ServerConfig::builder_with_provider(
        web_transport_quinn::crypto::default_provider(),
    )
    .with_protocol_versions(&[&rustls::version::TLS13])
    .map_err(|e| HostError::Cert(e.to_string()))?
    .with_no_client_auth()
    .with_single_cert(certs, key)
    .map_err(|e| HostError::Cert(e.to_string()))?;
    tls.alpn_protocols = vec![web_transport_quinn::ALPN.as_bytes().to_vec()];
    let tls = quinn::crypto::rustls::QuicServerConfig::try_from(tls)
        .map_err(|e| HostError::Cert(e.to_string()))?;
    let mut config = quinn::ServerConfig::with_crypto(Arc::new(tls));
    config.transport_config(Arc::new(transport_config(idle_timeout)));
    Ok(config)
}

/// Stream counts, flow-control windows and buffers sized to what one Pulse session uses, so
/// quinn buffers at most [`CONNECTION_RECEIVE_WINDOW`] of stream data and
/// [`DATAGRAM_RECEIVE_BUFFER`] of datagrams per connection. These caps stop at quinn: the
/// stream and datagram loops forward what they read into the host's unbounded events channel,
/// which grows if [`Host::service`] falls behind.
///
/// A connection silent for a third of the idle timeout is pinged, as ENet pings its peers: a
/// live client that sends nothing stays connected until Pulse ends its session with a reason
/// (the authentication timeout equals the idle timeout), while one that stops answering still
/// times out.
fn transport_config(idle_timeout: Duration) -> quinn::TransportConfig {
    let mut transport = quinn::TransportConfig::default();
    transport
        .max_concurrent_bidi_streams(MAX_CONCURRENT_BIDI_STREAMS.into())
        .max_concurrent_uni_streams(MAX_CONCURRENT_UNI_STREAMS.into())
        .stream_receive_window(STREAM_RECEIVE_WINDOW.into())
        .receive_window(CONNECTION_RECEIVE_WINDOW.into())
        .datagram_receive_buffer_size(Some(DATAGRAM_RECEIVE_BUFFER))
        .max_idle_timeout(Some(
            idle_timeout
                .try_into()
                .expect("the idle timeout fits a QUIC varint"),
        ))
        .keep_alive_interval(Some(idle_timeout / 3));
    transport
}

/// Parse a PEM certificate chain and private key into the rustls types
/// `web-transport-quinn` expects.
fn parse_pem(
    cert_pem: &str,
    key_pem: &str,
) -> Result<
    (
        Vec<rustls_pki_types::CertificateDer<'static>>,
        rustls_pki_types::PrivateKeyDer<'static>,
    ),
    HostError,
> {
    let mut cert_rd = std::io::BufReader::new(cert_pem.as_bytes());
    let certs = rustls_pemfile::certs(&mut cert_rd)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| HostError::Cert(e.to_string()))?;
    if certs.is_empty() {
        return Err(HostError::Cert("no certificates found in PEM".into()));
    }

    let mut key_rd = std::io::BufReader::new(key_pem.as_bytes());
    let key = rustls_pemfile::private_key(&mut key_rd)
        .map_err(|e| HostError::Cert(e.to_string()))?
        .ok_or_else(|| HostError::Cert("no private key found in PEM".into()))?;

    Ok((certs, key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    /// How long a stream open or write may wait before the test treats it as held back.
    const HELD_BACK: Duration = Duration::from_millis(500);

    #[test]
    fn the_transport_config_carries_every_pulse_limit() {
        // quinn exposes no getters, so read the values back from its field-by-field Debug.
        let config = format!("{:?}", transport_config(IDLE_TIMEOUT));
        for field in [
            "max_concurrent_bidi_streams: 4,",
            "max_concurrent_uni_streams: 16,",
            "max_idle_timeout: Some(30000),",
            "stream_receive_window: 65536,",
            "receive_window: 262144,",
            "datagram_receive_buffer_size: Some(65536),",
        ] {
            assert!(config.contains(field), "missing `{field}` in {config}");
        }
    }

    fn listen() -> (Host, Vec<u8>) {
        listen_with_idle_timeout(IDLE_TIMEOUT)
    }

    fn listen_with_idle_timeout(idle_timeout: Duration) -> (Host, Vec<u8>) {
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let hash = Sha256::digest(cert.cert.der()).to_vec();
        let host = Host::new(HostConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            cert_pem: cert.cert.pem(),
            key_pem: cert.signing_key.serialize_pem(),
            idle_timeout,
        })
        .unwrap();
        (host, hash)
    }

    async fn connect(address: SocketAddr, hash: Vec<u8>) -> Session {
        web_transport_quinn::ClientBuilder::new()
            .with_server_certificate_hashes(vec![hash])
            .unwrap()
            .connect(url::Url::parse(&format!("https://{address}/")).unwrap())
            .await
            .unwrap()
    }

    /// Writes until the peer's flow control holds the stream back; returns the bytes accepted.
    async fn fill(send: &mut web_transport_quinn::SendStream) -> usize {
        let chunk = vec![0u8; 16 * 1024];
        let mut accepted = 0;
        while accepted < 4 * 1024 * 1024 {
            match tokio::time::timeout(HELD_BACK, send.write(&chunk)).await {
                Ok(Ok(n)) => accepted += n,
                _ => break,
            }
        }
        accepted
    }

    #[cfg(feature = "test-support")]
    #[tokio::test]
    async fn client_reports_a_refusal_while_opening_its_reliable_stream() {
        check_client_refusal(false).await;
    }

    #[cfg(feature = "test-support")]
    #[tokio::test]
    async fn client_reports_a_refusal_before_the_connect_response() {
        check_client_refusal(true).await;
    }

    #[cfg(feature = "test-support")]
    async fn check_client_refusal(close_before_response: bool) {
        use super::super::client::{Client, ClientConfig, ClientError};
        use crate::pulse::hardening::DisconnectReason;

        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let hash = Sha256::digest(cert.cert.der()).to_vec();
        let (certs, key) = parse_pem(&cert.cert.pem(), &cert.signing_key.serialize_pem()).unwrap();
        let mut config = server_config(certs, key, IDLE_TIMEOUT).unwrap();
        // CONNECT uses the only bidirectional stream, so client setup has to wait for credit.
        Arc::get_mut(&mut config.transport)
            .unwrap()
            .max_concurrent_bidi_streams(1_u8.into());
        let endpoint = quinn::Endpoint::server(config, "127.0.0.1:0".parse().unwrap()).unwrap();
        let address = endpoint.local_addr().unwrap();
        let mut server = web_transport_quinn::Server::new(endpoint);
        let timeout = Duration::from_secs(2);
        let dialing = tokio::task::spawn_blocking(move || {
            Client::connect_within(
                ClientConfig {
                    url: format!("https://{address}/"),
                    server_cert_hash: Some(hash),
                },
                Some(timeout),
            )
            .err()
        });

        let request = tokio::time::timeout(timeout, server.accept())
            .await
            .expect("client should send CONNECT")
            .unwrap();
        let refusal = DisconnectReason::PreAuthIpLimitExhausted.code();
        let _session = if close_before_response {
            let code = web_transport_quinn::proto::error_to_http3(refusal);
            request.conn().close(code.try_into().unwrap(), b"refused");
            None
        } else {
            let session = request.ok().await.unwrap();
            session.close(refusal, b"");
            Some(session)
        };
        match dialing.await.unwrap() {
            Some(ClientError::Closed { code, reason }) => {
                assert_eq!(code, refusal);
                assert_eq!(reason, if close_before_response { "refused" } else { "" });
            }
            Some(error) => panic!("expected the server refusal, got {error}"),
            None => panic!("client opened a stream despite the server refusal"),
        }
    }

    #[test]
    fn a_live_session_that_sends_nothing_outlives_the_idle_timeout() {
        // Pulse ends a session that never authenticates after 30 s, the idle timeout too: the
        // session must stay open for Pulse to close it with its reason.
        let idle_timeout = Duration::from_secs(1);
        let (mut host, hash) = listen_with_idle_timeout(idle_timeout);
        let address = host.local_addr();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let session = runtime.block_on(connect(address, hash));

        let deadline = std::time::Instant::now() + 3 * idle_timeout;
        while std::time::Instant::now() < deadline {
            if let Some(Event::Disconnect { .. }) = host.service(Duration::from_millis(50)) {
                panic!("the idle timeout closed a live session that sends nothing");
            }
        }
        drop(session);
    }

    #[test]
    fn a_session_gets_its_reliable_stream_and_no_more_than_the_stream_limit() {
        let (mut host, hash) = listen();
        let address = host.local_addr();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let (_session, _send) = runtime.block_on(async move {
            let session = connect(address, hash).await;
            let (mut send, _recv) = session.open_bi().await.unwrap();
            send.write_all(b"pulse").await.unwrap();
            // The CONNECT stream and the Pulse stream hold two of the limit.
            let mut extra = vec![];
            for _ in 2..MAX_CONCURRENT_BIDI_STREAMS {
                extra.push(session.open_bi().await.unwrap());
            }
            assert!(
                tokio::time::timeout(HELD_BACK, session.open_bi())
                    .await
                    .is_err(),
                "a bidirectional stream past the limit must wait for stream credit"
            );
            (session, (send, extra))
        });

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            assert!(
                std::time::Instant::now() < deadline,
                "reliable stream data not delivered"
            );
            if let Some(Event::StreamData { data, .. }) = host.service(Duration::from_millis(50)) {
                assert_eq!(&data[..], b"pulse");
                break;
            }
        }
    }

    #[test]
    fn unread_unidirectional_streams_buffer_at_most_the_connection_window() {
        let (host, hash) = listen();
        let address = host.local_addr();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async move {
            let session = connect(address, hash).await;
            // The client's HTTP/3 control stream already holds one of the limit.
            let mut streams = vec![];
            for _ in 1..MAX_CONCURRENT_UNI_STREAMS {
                streams.push(session.open_uni().await.unwrap());
            }
            assert!(
                tokio::time::timeout(HELD_BACK, session.open_uni())
                    .await
                    .is_err(),
                "a unidirectional stream past the limit must wait for stream credit"
            );
            // Enough streams that their windows together exceed the connection window.
            let filled = (CONNECTION_RECEIVE_WINDOW / STREAM_RECEIVE_WINDOW + 1) as usize;
            let mut total = 0;
            for send in &mut streams[..filled] {
                let accepted = fill(send).await;
                assert!(
                    accepted <= STREAM_RECEIVE_WINDOW as usize,
                    "an unread stream accepted {accepted} bytes"
                );
                total += accepted;
            }
            assert!(total <= CONNECTION_RECEIVE_WINDOW as usize);
        });
    }
}
