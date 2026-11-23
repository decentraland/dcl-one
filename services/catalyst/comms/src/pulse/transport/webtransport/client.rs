// from: catalyrst/third_party/web_transport/src/client.rs (decentraland/rust-web-transport c5416501; Apache-2.0)
//! WebTransport client: connects to a server, opens the reliable bidi stream, and pumps
//! datagrams + stream bytes to/from a synchronously-polled event queue. A single session.
//!
//! For tests and test tools that reach Pulse over WebTransport as a client does (the `comms-e2e`
//! bots); it is compiled only with the `test-support` feature. Like the host, it is a generic
//! byte pipe driven by repeated [`Client::service`] calls; framing and sequencing are the
//! caller's ([`super::framing`]).

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use bytes::Bytes;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use web_transport_quinn::{Session, SessionError, WebTransportError};

/// Buffer size for a single reliable-stream read. Raw chunks are forwarded as-is.
const STREAM_READ_BUF: usize = 16 * 1024;

/// The longest [`Client::disconnect`] waits for the close frame to leave.
const CLOSE_FLUSH: Duration = Duration::from_millis(200);

/// Settings for [`Client::connect`].
pub struct ClientConfig {
    /// Server URL, e.g. `"https://127.0.0.1:4433/"`.
    pub url: String,
    /// SHA-256 of the server's certificate (the WebTransport `serverCertificateHashes` trust
    /// path, for self-signed dev certs). `None` validates against the system root store.
    pub server_cert_hash: Option<Vec<u8>>,
}

/// Errors returned by [`Client::connect`].
#[derive(Debug)]
pub enum ClientError {
    /// The URL or trust configuration was invalid.
    Config(String),
    /// The connection (TLS / WebTransport handshake / stream open) failed.
    Connect(String),
    /// The peer closed the session before the reliable stream was ready.
    Closed { code: u32, reason: String },
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::Config(m) => write!(f, "config error: {m}"),
            ClientError::Connect(m) => write!(f, "connect error: {m}"),
            ClientError::Closed { code, reason } => {
                write!(f, "session closed with {code}: {reason}")
            }
        }
    }
}

impl std::error::Error for ClientError {}

/// An event drained from the client. There is a single session, so no peer id.
#[derive(Debug, Clone)]
pub enum ClientEvent {
    /// The session ended (server closed, transport error, or local disconnect). `reason` is the
    /// server's close code, 0 when the session ended without one.
    Disconnected { reason: u32 },
    /// Raw bytes arrived on the reliable bidi stream (unframed).
    StreamData { data: Bytes },
    /// An unreliable datagram arrived.
    Datagram { data: Bytes },
}

/// Command from the polling thread into the stream-writer task.
enum ClientCommand {
    Stream(Vec<u8>),
}

/// A connected WebTransport client. Construct with [`Client::connect`], then drive it by
/// repeatedly calling [`Client::service`] from a single dedicated thread.
pub struct Client {
    runtime: Option<Runtime>,
    events_rx: Receiver<ClientEvent>,
    session: Session,
    cmd_tx: UnboundedSender<ClientCommand>,
}

impl Client {
    /// Connect to the server, open the reliable bidi stream, and start pumping events.
    /// Blocks until the session is established (or fails).
    pub fn connect(config: ClientConfig) -> Result<Client, ClientError> {
        Client::connect_within(config, None)
    }

    /// [`Client::connect`] that gives up after `timeout` instead of waiting for QUIC's own
    /// handshake timeout.
    pub fn connect_within(
        config: ClientConfig,
        timeout: Option<Duration>,
    ) -> Result<Client, ClientError> {
        Client::connect_within_from(config, timeout, None)
    }

    /// [`Client::connect_within`] from a chosen local address: the server counts its per-address
    /// limits by the source address of a connection. Needs `server_cert_hash` (a client bound to
    /// an address does not use the system trust store).
    pub fn connect_within_from(
        config: ClientConfig,
        timeout: Option<Duration>,
        local: Option<IpAddr>,
    ) -> Result<Client, ClientError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            // Each test client drives just one connection. A swarm creates many clients.
            .worker_threads(1)
            .enable_all()
            .build()
            .map_err(|e| ClientError::Connect(e.to_string()))?;

        let url = url::Url::parse(&config.url).map_err(|e| ClientError::Config(e.to_string()))?;
        let cert_hash = config.server_cert_hash;

        let connecting = async move {
            let client = match (local, cert_hash) {
                (Some(local), Some(hash)) => bound_client(local, hash)?,
                (Some(_), None) => {
                    return Err(ClientError::Config(
                        "a client bound to a local address needs a certificate hash".to_owned(),
                    ));
                }
                (None, Some(hash)) => web_transport_quinn::ClientBuilder::new()
                    .with_server_certificate_hashes(vec![hash])
                    .map_err(|e| ClientError::Connect(e.to_string()))?,
                (None, None) => web_transport_quinn::ClientBuilder::new()
                    .with_system_roots()
                    .map_err(|e| ClientError::Connect(e.to_string()))?,
            };

            let session = client
                .connect(url)
                .await
                .map_err(|e| ClientError::Connect(e.to_string()))?;
            // Client opens the reliable bidi stream; the server accepts it (server `accept_bi`).
            let (send, recv) = session.open_bi().await.map_err(|error| match error {
                SessionError::WebTransportError(WebTransportError::Closed(code, reason)) => {
                    ClientError::Closed { code, reason }
                }
                other => ClientError::Connect(other.to_string()),
            })?;
            Ok::<_, ClientError>((session, send, recv))
        };
        let (session, send, recv) = runtime.block_on(async move {
            match timeout {
                Some(limit) => tokio::time::timeout(limit, connecting)
                    .await
                    .map_err(|_| ClientError::Connect(format!("no session within {limit:?}")))?,
                None => connecting.await,
            }
        })?;

        let (events_tx, events_rx) = std::sync::mpsc::channel::<ClientEvent>();
        let (cmd_tx, cmd_rx) = unbounded_channel::<ClientCommand>();

        runtime.spawn(datagram_loop(session.clone(), events_tx.clone()));
        runtime.spawn(stream_reader(recv, events_tx.clone()));
        runtime.spawn(stream_writer(send, cmd_rx));
        runtime.spawn(disconnect_monitor(session.clone(), events_tx));

        Ok(Client {
            runtime: Some(runtime),
            events_rx,
            session,
            cmd_tx,
        })
    }

    /// Block up to `timeout` for the next event. Returns `None` on timeout.
    pub fn service(&mut self, timeout: Duration) -> Option<ClientEvent> {
        self.events_rx.recv_timeout(timeout).ok()
    }

    /// Queue raw bytes for the reliable bidi stream. Returns `false` if the session is gone.
    pub fn send_stream(&self, data: &[u8]) -> bool {
        self.cmd_tx
            .send(ClientCommand::Stream(data.to_vec()))
            .is_ok()
    }

    /// Send an unreliable datagram. Returns `false` on failure (gone / too large).
    pub fn send_datagram(&self, data: &[u8]) -> bool {
        self.session
            .send_datagram(Bytes::copy_from_slice(data))
            .is_ok()
    }

    /// Close the session with an application error code. Returns once the close frame has been
    /// handed to the socket (or after [`CLOSE_FLUSH`]), so that dropping the client right after
    /// cannot discard it: the server then hears the close instead of waiting for its idle timeout.
    pub fn disconnect(&self, reason: u32) {
        let Some(runtime) = self.runtime.as_ref() else {
            return;
        };
        // a session the peer has already closed sends nothing
        let already_closed = self.session.close_reason().is_some();
        // `Session::close` must run inside the tokio runtime context.
        let _guard = runtime.enter();
        self.session.close(reason, b"");
        if already_closed {
            return;
        }
        let connection: &web_transport_quinn::quinn::Connection = &self.session;
        runtime.block_on(async {
            let deadline = tokio::time::Instant::now() + CLOSE_FLUSH;
            while connection.stats().frame_tx.connection_close == 0
                && tokio::time::Instant::now() < deadline
            {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        });
    }

    /// Current smoothed RTT to the server in microseconds.
    pub fn rtt_us(&self) -> u64 {
        self.session.rtt().as_micros() as u64
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_timeout(Duration::from_millis(200));
        }
    }
}

async fn datagram_loop(session: Session, events_tx: Sender<ClientEvent>) {
    while let Ok(data) = session.read_datagram().await {
        if events_tx.send(ClientEvent::Datagram { data }).is_err() {
            break;
        }
    }
}

async fn stream_reader(mut recv: web_transport_quinn::RecvStream, events_tx: Sender<ClientEvent>) {
    let mut buf = vec![0u8; STREAM_READ_BUF];
    while let Ok(Some(n)) = recv.read(&mut buf).await {
        let data = Bytes::copy_from_slice(&buf[..n]);
        if events_tx.send(ClientEvent::StreamData { data }).is_err() {
            break;
        }
    }
}

async fn stream_writer(
    mut send: web_transport_quinn::SendStream,
    mut cmd_rx: UnboundedReceiver<ClientCommand>,
) {
    while let Some(ClientCommand::Stream(bytes)) = cmd_rx.recv().await {
        if send.write_all(&bytes).await.is_err() {
            break;
        }
    }
}

async fn disconnect_monitor(session: Session, events_tx: Sender<ClientEvent>) {
    // The code the server closed the session with; 0 when the session ended some other way
    // (idle timeout, lost connection, local close).
    let reason = match session.closed().await {
        SessionError::WebTransportError(WebTransportError::Closed(code, _)) => code,
        _ => 0,
    };
    let _ = events_tx.send(ClientEvent::Disconnected { reason });
}

/// A client whose QUIC endpoint is bound to `local` (any port) and that trusts the one server
/// certificate with SHA-256 `hash`: what the builder does, except for the local address, which the
/// builder does not let a caller choose.
fn bound_client(local: IpAddr, hash: Vec<u8>) -> Result<web_transport_quinn::Client, ClientError> {
    use web_transport_quinn::quinn;
    let fail = |error: &dyn std::fmt::Display| ClientError::Connect(error.to_string());
    let provider = web_transport_quinn::crypto::default_provider();
    let mut crypto = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|e| fail(&e))?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(PinnedCertificate { provider, hash }))
        .with_no_client_auth();
    crypto.alpn_protocols = vec![web_transport_quinn::ALPN.as_bytes().to_vec()];
    let quic = quinn::crypto::rustls::QuicClientConfig::try_from(crypto).map_err(|e| fail(&e))?;
    let endpoint = quinn::Endpoint::client(SocketAddr::new(local, 0)).map_err(|e| fail(&e))?;
    Ok(web_transport_quinn::Client::new(
        endpoint,
        quinn::ClientConfig::new(Arc::new(quic)),
    ))
}

/// Accepts the one certificate whose SHA-256 is `hash` (the WebTransport
/// `serverCertificateHashes` trust path).
#[derive(Debug)]
struct PinnedCertificate {
    provider: Arc<rustls::crypto::CryptoProvider>,
    hash: Vec<u8>,
}

impl rustls::client::danger::ServerCertVerifier for PinnedCertificate {
    fn verify_server_cert(
        &self,
        end_entity: &rustls_pki_types::CertificateDer<'_>,
        _intermediates: &[rustls_pki_types::CertificateDer<'_>],
        _server_name: &rustls_pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls_pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        let seen = web_transport_quinn::crypto::sha256(&self.provider, end_entity);
        if seen.as_ref() == self.hash.as_slice() {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::InvalidCertificate(
                rustls::CertificateError::UnknownIssuer,
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls_pki_types::CertificateDer<'_>,
        signed: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            signed,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls_pki_types::CertificateDer<'_>,
        signed: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            signed,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}
