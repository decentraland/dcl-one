//! Socket-local observation and retirement at the accepted TCP I/O boundary.
use axum::extract::Request;
use axum::extract::connect_info::{ConnectInfo, Connected};
use axum::middleware::Next;
use axum::response::Response;
use axum::serve::IncomingStream;
use parking_lot::Mutex;
use std::io::{self, IoSlice};
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Notify;
use tokio::time::Instant;

#[derive(Default)]
struct State {
    armed: bool,
    terminal: bool,
    last_read: Option<Instant>,
    activity: Option<Instant>,
    generation: u64,
}

#[derive(Clone, Default)]
pub(super) struct Signal {
    state: Arc<Mutex<State>>,
    changed: Arc<Notify>,
}

impl Signal {
    pub(super) fn arm(&self) {
        let mut state = self.state.lock();
        if !state.terminal && !state.armed {
            state.armed = true;
            // HTTP may have read the first frame prefix ahead of the upgrade.
            // Carry that read into the admitted socket exactly once.
            state.activity = state.last_read;
            self.changed.notify_waiters();
        }
    }

    pub(super) fn take_activity(&self) -> Option<Instant> {
        self.state.lock().activity.take()
    }

    pub(super) fn notified(&self) -> tokio::sync::futures::Notified<'_> {
        self.changed.notified()
    }

    pub(super) fn retire(&self) {
        let mut state = self.state.lock();
        state.terminal = true;
        state.activity = None;
        self.changed.notify_waiters();
    }

    /// The idle expiry and the last raw read share one linearization point.
    pub(super) fn retire_if_quiet(&self) -> bool {
        let mut state = self.state.lock();
        if state.activity.is_some() {
            return false;
        }
        state.terminal = true;
        self.changed.notify_waiters();
        true
    }
}

/// Wraps the existing listener without changing addresses or accept retries.
pub struct Listener(pub TcpListener);

impl axum::serve::Listener for Listener {
    type Io = AcceptedIo<TcpStream>;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        let (io, address) = axum::serve::Listener::accept(&mut self.0).await;
        (AcceptedIo::new(io, Signal::default()), address)
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.0.local_addr()
    }
}

/// Per-connection metadata, projected to the usual address by `connection_info`.
#[derive(Clone)]
pub struct Connection {
    address: SocketAddr,
    signal: Signal,
}

impl Connected<IncomingStream<'_, Listener>> for Connection {
    fn connect_info(stream: IncomingStream<'_, Listener>) -> Self {
        Self {
            address: *stream.remote_addr(),
            signal: stream.io().signal.clone(),
        }
    }
}

/// Preserve address extraction while attaching the private upgrade handle.
pub async fn connection_info(mut request: Request, next: Next) -> Response {
    if let Some(ConnectInfo(connection)) = request
        .extensions()
        .get::<ConnectInfo<Connection>>()
        .cloned()
    {
        request
            .extensions_mut()
            .insert(ConnectInfo(connection.address));
        request.extensions_mut().insert(connection.signal);
    }
    next.run(request).await
}

/// Every parser write path reaches this gate, including automatic control replies.
pub struct AcceptedIo<S> {
    inner: S,
    signal: Signal,
}

impl<S> AcceptedIo<S> {
    fn new(inner: S, signal: Signal) -> Self {
        Self { inner, signal }
    }
}

fn retired() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "WebSocket transport retired")
}

impl<S: AsyncRead + Unpin> AsyncRead for AcceptedIo<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let mut state = this.signal.state.lock();
        if state.terminal {
            return Poll::Ready(Ok(()));
        }
        let before = buffer.filled().len();
        let result = Pin::new(&mut this.inner).poll_read(cx, buffer);
        if matches!(result, Poll::Ready(Ok(()))) && buffer.filled().len() > before {
            let now = Instant::now();
            state.last_read = Some(now);
            state.generation = state.generation.wrapping_add(1);
            if state.armed {
                state.activity = Some(now);
                this.signal.changed.notify_waiters();
            }
        }
        result
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for AcceptedIo<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let state = this.signal.state.lock();
        if state.terminal {
            return Poll::Ready(Err(retired()));
        }
        Pin::new(&mut this.inner).poll_write(cx, bytes)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let state = this.signal.state.lock();
        if state.terminal {
            return Poll::Ready(Err(retired()));
        }
        Pin::new(&mut this.inner).poll_write_vectored(cx, bytes)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let state = this.signal.state.lock();
        if state.terminal {
            return Poll::Ready(Err(retired()));
        }
        Pin::new(&mut this.inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let state = this.signal.state.lock();
        if state.terminal {
            return Poll::Ready(Err(retired()));
        }
        Pin::new(&mut this.inner).poll_shutdown(cx)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use futures::{SinkExt, StreamExt};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio_tungstenite::{
        WebSocketStream,
        tungstenite::{Message, protocol::Role},
    };

    #[tokio::test(start_paused = true)]
    async fn positive_raw_reads_observe_partial_headers_masks_and_payloads() {
        let (server, mut client) = tokio::io::duplex(128);
        let signal = Signal::default();
        signal.arm();
        let mut parser = WebSocketStream::from_raw_socket(
            AcceptedIo::new(server, signal.clone()),
            Role::Server,
            None,
        )
        .await;
        for fragment in [&[0x82][..], &[0x84, 1, 2][..], &[3, 4, 8][..]] {
            tokio::time::advance(std::time::Duration::from_secs(4)).await;
            client.write_all(fragment).await.unwrap();
            assert!(futures::poll!(parser.next()).is_pending());
            assert_eq!(signal.take_activity(), Some(Instant::now()));
            assert!(futures::poll!(parser.next()).is_pending());
            assert_eq!(signal.take_activity(), None, "Pending must not reset idle");
        }
        drop(client);
        assert!(parser.next().await.unwrap().is_err());
        assert_eq!(signal.take_activity(), None, "EOF must not reset idle");
    }

    #[tokio::test(start_paused = true)]
    async fn admission_consumes_http_read_ahead_once_and_terminal_input_cannot_rearm() {
        let (server, mut client) = tokio::io::duplex(32);
        let signal = Signal::default();
        let mut io = AcceptedIo::new(server, signal.clone());
        client.write_all(b"http-and-frame-prefix").await.unwrap();
        let mut bytes = [0; 32];
        assert!(io.read(&mut bytes).await.unwrap() > 0);
        assert_eq!(
            signal.take_activity(),
            None,
            "unadmitted HTTP must stay unarmed"
        );
        signal.arm();
        assert_eq!(signal.take_activity(), Some(Instant::now()));
        assert_eq!(signal.take_activity(), None);
        signal.retire();
        client.write_all(b"late").await.unwrap();
        assert_eq!(io.read(&mut bytes).await.unwrap(), 0);
        signal.arm();
        assert_eq!(signal.take_activity(), None);
        assert!(io.write_all(b"blocked").await.is_err());
    }

    struct NoProgress(bool);
    impl AsyncRead for NoProgress {
        fn poll_read(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            _: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            Poll::Ready(if self.0 {
                Err(io::Error::other("read failed"))
            } else {
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn zero_and_error_reads_never_advance_activity_or_generation() {
        for error in [false, true] {
            let signal = Signal::default();
            signal.arm();
            let mut io = AcceptedIo::new(NoProgress(error), signal.clone());
            let mut buffer = [0; 1];
            let result = io.read(&mut buffer).await;
            if error {
                assert!(result.is_err());
            } else {
                assert_eq!(result.unwrap(), 0);
            }
            assert_eq!(signal.take_activity(), None);
            assert_eq!(signal.state.lock().generation, 0);
        }
    }

    #[tokio::test]
    async fn ordinary_output_and_clean_close_still_reach_the_lower_transport() {
        let (server, mut client) = tokio::io::duplex(128);
        let signal = Signal::default();
        signal.arm();
        let mut parser =
            WebSocketStream::from_raw_socket(AcceptedIo::new(server, signal), Role::Server, None)
                .await;
        parser
            .send(Message::Binary(b"ok".to_vec().into()))
            .await
            .unwrap();
        let mut frame = [0; 4];
        client.read_exact(&mut frame).await.unwrap();
        assert_eq!(&frame, b"\x82\x02ok");
        client
            .write_all(&[0x88, 0x82, 1, 2, 3, 4, 2, 234])
            .await
            .unwrap();
        assert!(matches!(parser.next().await, Some(Ok(Message::Close(_)))));
        parser.flush().await.unwrap();
        client.read_exact(&mut frame).await.unwrap();
        assert_eq!(frame, [0x88, 2, 3, 232]);
    }

    #[tokio::test(start_paused = true)]
    async fn an_observed_read_wins_idle_retirement_and_terminal_state_is_permanent() {
        let (server, mut client) = tokio::io::duplex(8);
        let signal = Signal::default();
        signal.arm();
        let mut io = AcceptedIo::new(server, signal.clone());
        client.write_all(&[0x82]).await.unwrap();
        let mut byte = [0];
        io.read_exact(&mut byte).await.unwrap();
        assert!(!signal.retire_if_quiet());
        assert_eq!(signal.take_activity(), Some(Instant::now()));
        assert!(signal.retire_if_quiet());
        client.write_all(&[0x80]).await.unwrap();
        assert_eq!(io.read(&mut byte).await.unwrap(), 0);
        assert_eq!(signal.take_activity(), None);
        assert!(io.write_vectored(&[IoSlice::new(b"late")]).await.is_err());
        assert!(io.flush().await.is_err());
        assert!(io.shutdown().await.is_err());
    }

    async fn wait_until(label: &str, mut condition: impl FnMut() -> bool) {
        let began = std::time::Instant::now();
        while !condition() {
            assert!(
                began.elapsed() < std::time::Duration::from_secs(3),
                "timed out waiting for {label} at Tokio time {:?}",
                Instant::now()
            );
            // Keep paused time from auto-advancing while the reactor handles TCP.
            tokio::task::yield_now().await;
        }
    }

    async fn advance_timer(logical_deadline: Instant) {
        assert!(Instant::now() < logical_deadline);
        let barrier = tokio::time::sleep_until(logical_deadline);
        tokio::pin!(barrier);
        assert!(futures::poll!(barrier.as_mut()).is_pending());
        // Sleep deadlines round up to a millisecond tick. Crossing that full
        // resolution reaches the effective tick even when the clock is frozen
        // at a nanosecond deadline between ticks. Await the registered barrier
        // so advancing the clock is not mistaken for processing its timers.
        let after_tick = logical_deadline + std::time::Duration::from_millis(1);
        tokio::time::advance(after_tick.duration_since(Instant::now())).await;
        barrier.await;
        assert_eq!(Instant::now(), after_tick);
    }

    async fn assert_before_probe(tcp: &TcpStream, logical_deadline: Instant) {
        let before = logical_deadline - std::time::Duration::from_millis(1);
        tokio::time::advance(before.duration_since(Instant::now())).await;
        let mut bytes = [0; 32];
        assert_eq!(
            tcp.try_read(&mut bytes).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }

    async fn mounted_listener() -> (
        u16,
        crate::archipelago::AppState,
        tokio::sync::mpsc::Receiver<Signal>,
        tokio::task::JoinHandle<()>,
    ) {
        let mut settings = crate::archipelago::Settings::for_tests();
        settings.server.max_sockets = 1;
        settings.auth.handshake_timeout_ms = 600_000;
        let state = crate::archipelago::test_support::state_over(
            settings,
            Arc::new(crate::gatekeeper::bus::DisabledBus),
        )
        .await;
        let mounts = crate::archipelago::mounts(
            state.clone(),
            true,
            Default::default(),
            crate::stats::http::test_support::matcher(),
            Arc::new(crate::content::Denylist::default()),
        );
        let (capture, signals) = tokio::sync::mpsc::channel(4);
        let app = mounts
            .root
            .route(
                "/address",
                axum::routing::get(|ConnectInfo(address): ConnectInfo<SocketAddr>| async move {
                    address.to_string()
                }),
            )
            .layer(axum::middleware::from_fn(
                move |request: Request, next: Next| {
                    let capture = capture.clone();
                    async move {
                        capture
                            .try_send(request.extensions().get::<Signal>().unwrap().clone())
                            .unwrap();
                        next.run(request).await
                    }
                },
            ))
            .layer(axum::middleware::from_fn(connection_info));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let task = tokio::spawn(async move {
            axum::serve(
                Listener(listener),
                app.into_make_service_with_connect_info::<Connection>(),
            )
            .await
            .unwrap();
        });
        (port, state, signals, task)
    }

    async fn admitted_with_prefix(port: u16, prefix: &[u8]) -> TcpStream {
        let mut tcp = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let mut request = b"GET /ws HTTP/1.1\r\nHost: localhost\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n".to_vec();
        request.extend_from_slice(prefix);
        tcp.write_all(&request).await.unwrap();
        let mut response = Vec::new();
        while !response.ends_with(b"\r\n\r\n") {
            response.push(tcp.read_u8().await.unwrap());
            assert!(response.len() < 8192);
        }
        assert!(response.starts_with(b"HTTP/1.1 101"));
        tcp
    }

    async fn partial_idle_scenario(read_ahead: bool) {
        let (port, state, mut signals, task) = mounted_listener().await;
        // Prevent automatic clock jumps during OS readiness waits. Timer
        // processing is synchronized separately by a registered Sleep barrier.
        let clock_guard = tokio::spawn(async {
            loop {
                tokio::task::yield_now().await;
            }
        });
        tokio::time::pause();
        let mut tcp = admitted_with_prefix(port, if read_ahead { &[0x82] } else { &[] }).await;
        let signal = signals.recv().await.unwrap();
        wait_until(
            "admitted WebSocket initialization consumes its raw snapshot",
            || {
                let raw = signal.state.lock();
                raw.armed && raw.activity.is_none()
            },
        )
        .await;
        let epoch = Instant::from_std(state.started_at);
        let mut probe = epoch
            + std::time::Duration::from_secs(
                (Instant::now().duration_since(epoch).as_secs() / 4 + 19) * 4,
            );
        if !read_ahead {
            tokio::time::advance(
                (probe - std::time::Duration::from_secs(1)).duration_since(Instant::now()),
            )
            .await;
            let before = signal.state.lock().generation;
            tcp.write_all(&[0x82]).await.unwrap();
            wait_until("positive raw header read", || {
                signal.state.lock().generation > before
            })
            .await;
            probe += std::time::Duration::from_secs(72);
        }
        // No complete frame exists. The reader must still drive the raw seam.
        assert_before_probe(&tcp, probe).await;
        assert!(!signal.state.lock().terminal);
        advance_timer(probe).await;
        let mut wire = Vec::new();
        wait_until("first idle Ping bytes on the actual TCP stream", || {
            let mut bytes = [0; 32];
            if let Ok(n) = tcp.try_read(&mut bytes) {
                wire.extend_from_slice(&bytes[..n]);
            }
            wire == [0x89, 0]
        })
        .await;
        for (label, fragment) in [
            ("positive raw partial mask read", &[0x84, 1, 2][..]),
            ("positive raw partial payload read", &[3, 4, 8][..]),
        ] {
            // The first fragment arrives in probe grace; the second arrives
            // immediately before the new first-stage deadline.
            let arrival = probe + std::time::Duration::from_secs(15);
            tokio::time::advance(arrival.duration_since(Instant::now())).await;
            let before = signal.state.lock().generation;
            tcp.write_all(fragment).await.unwrap();
            wait_until(label, || signal.state.lock().generation > before).await;
            let next = epoch
                + std::time::Duration::from_secs(
                    (arrival.duration_since(epoch).as_secs() / 4 + 19) * 4,
                );
            // Cross the old grace boundary after observing the actual lower read.
            tokio::time::advance(std::time::Duration::from_secs(2)).await;
            assert!(!signal.state.lock().terminal);
            assert_eq!(state.sockets.available_permits(), 0);
            probe = next - std::time::Duration::from_secs(16);
        }
        let final_probe = probe + std::time::Duration::from_secs(16);
        assert_before_probe(&tcp, final_probe).await;
        assert!(!signal.state.lock().terminal);
        advance_timer(final_probe).await;
        wait_until("second idle Ping bytes after partial-input resets", || {
            let mut bytes = [0; 32];
            if let Ok(n) = tcp.try_read(&mut bytes) {
                wire.extend_from_slice(&bytes[..n]);
            }
            wire.len() == 4
        })
        .await;
        assert_eq!(wire, [0x89, 0, 0x89, 0]);
        advance_timer(Instant::now() + std::time::Duration::from_secs(16)).await;
        wait_until(
            "idle grace retires raw I/O and returns the physical permit",
            || signal.state.lock().terminal && state.sockets.available_permits() == 1,
        )
        .await;
        let generation = signal.state.lock().generation;
        let _ = tcp.write_all(&[9, 9, 9]).await;
        assert_eq!(signal.state.lock().generation, generation);
        assert_eq!(signal.take_activity(), None);
        tokio::time::resume();
        clock_guard.abort();
        task.abort();
    }

    #[tokio::test]
    async fn mounted_partial_header_mask_and_payload_reset_idle_before_parsing() {
        partial_idle_scenario(false).await;
    }

    #[tokio::test]
    async fn mounted_http_read_ahead_is_consumed_once_before_partial_frame_activity() {
        partial_idle_scenario(true).await;
    }

    #[tokio::test]
    async fn ordinary_http_preserves_peer_address_and_stays_unarmed() {
        let (port, _state, mut signals, task) = mounted_listener().await;
        let mut tcp = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let local = tcp.local_addr().unwrap();
        tcp.write_all(b"GET /address HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let mut response = String::new();
        tcp.read_to_string(&mut response).await.unwrap();
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.ends_with(&local.to_string()));
        assert!(!signals.recv().await.unwrap().state.lock().armed);
        task.abort();
    }
    struct Scheduled {
        inner: tokio::io::DuplexStream,
        allowance: Arc<std::sync::atomic::AtomicUsize>,
    }
    impl AsyncRead for Scheduled {
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buffer: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            Pin::new(&mut self.get_mut().inner).poll_read(cx, buffer)
        }
    }
    impl AsyncWrite for Scheduled {
        fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            bytes: &[u8],
        ) -> Poll<io::Result<usize>> {
            use std::sync::atomic::Ordering;
            let this = self.get_mut();
            let allowed = this.allowance.load(Ordering::SeqCst).min(bytes.len());
            if allowed == 0 {
                return Poll::Pending;
            }
            let result = Pin::new(&mut this.inner).poll_write(cx, &bytes[..allowed]);
            if let Poll::Ready(Ok(n)) = result {
                this.allowance.fetch_sub(n, Ordering::SeqCst);
            }
            result
        }
        fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Pin::new(&mut self.get_mut().inner).poll_flush(cx)
        }
        fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
        }
    }
    struct ScriptListener(Option<AcceptedIo<Scheduled>>);
    impl axum::serve::Listener for ScriptListener {
        type Io = AcceptedIo<Scheduled>;
        type Addr = SocketAddr;
        async fn accept(&mut self) -> (Self::Io, SocketAddr) {
            match self.0.take() {
                Some(io) => (io, "127.0.0.1:1".parse().unwrap()),
                None => std::future::pending().await,
            }
        }
        fn local_addr(&self) -> io::Result<SocketAddr> {
            Ok("127.0.0.1:1".parse().unwrap())
        }
    }
    impl Connected<IncomingStream<'_, ScriptListener>> for Connection {
        fn connect_info(stream: IncomingStream<'_, ScriptListener>) -> Self {
            Self {
                address: *stream.remote_addr(),
                signal: stream.io().signal.clone(),
            }
        }
    }

    async fn scripted_upgrade() -> (
        axum::extract::ws::WebSocket,
        tokio::io::DuplexStream,
        Signal,
        Arc<std::sync::atomic::AtomicUsize>,
        tokio::task::JoinHandle<()>,
    ) {
        use std::sync::atomic::AtomicUsize;
        let (server, mut client) = tokio::io::duplex(1024);
        let signal = Signal::default();
        let allowance = Arc::new(AtomicUsize::new(usize::MAX));
        let io = AcceptedIo::new(
            Scheduled {
                inner: server,
                allowance: allowance.clone(),
            },
            signal.clone(),
        );
        let (give, take) = tokio::sync::oneshot::channel();
        let give = Arc::new(Mutex::new(Some(give)));
        let app = axum::Router::new()
            .route(
                "/ws",
                axum::routing::get(
                    move |upgrade: axum::extract::ws::WebSocketUpgrade,
                          axum::Extension(signal): axum::Extension<Signal>| {
                        let give = give.clone();
                        async move {
                            signal.arm();
                            upgrade
                                .write_buffer_size(0)
                                .on_upgrade(move |socket| async move {
                                    give.lock().take().unwrap().send(socket).unwrap();
                                })
                        }
                    },
                ),
            )
            .layer(axum::middleware::from_fn(connection_info));
        let task = tokio::spawn(async move {
            axum::serve(
                ScriptListener(Some(io)),
                app.into_make_service_with_connect_info::<Connection>(),
            )
            .await
            .unwrap();
        });
        client.write_all(b"GET /ws HTTP/1.1\r\nHost: localhost\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n").await.unwrap();
        let mut response = Vec::new();
        while !response.ends_with(b"\r\n\r\n") {
            response.push(client.read_u8().await.unwrap());
        }
        assert!(response.starts_with(b"HTTP/1.1 101"));
        let socket = take.await.unwrap();
        (socket, client, signal, allowance, task)
    }

    #[tokio::test]
    async fn axum_reader_and_writer_cannot_drain_a_partial_frame_after_retirement() {
        use std::sync::atomic::Ordering;
        for opcode in [0x89, 0x88] {
            let (mut socket, mut client, signal, allowance, task) = scripted_upgrade().await;
            allowance.store(2, Ordering::SeqCst);
            let mut sending = Box::pin(socket.send(axum::extract::ws::Message::Binary(
                b"secret".to_vec().into(),
            )));
            assert!(futures::poll!(&mut sending).is_pending());
            drop(sending);
            let mut prefix = [0; 2];
            client.read_exact(&mut prefix).await.unwrap();
            assert_eq!(prefix, [0x82, 6]);
            // Queue incoming Ping/Close while the application suffix is held.
            client.write_all(&[opcode, 0x80, 1, 2, 3, 4]).await.unwrap();
            let incoming = socket.next().await.unwrap().unwrap();
            assert!(matches!(
                incoming,
                axum::extract::ws::Message::Ping(_) | axum::extract::ws::Message::Close(_)
            ));
            // The real reader has now queued its automatic Pong/Close behind
            // the application suffix. Retirement must fence its next flush.
            signal.retire();
            allowance.store(usize::MAX, Ordering::SeqCst);
            // Poll Axum's parser and its queued automatic control response.
            assert!(socket.next().await.unwrap().is_err());
            assert!(SinkExt::flush(&mut socket).await.is_err());
            let mut bytes = [0; 16];
            {
                let read = client.read(&mut bytes);
                tokio::pin!(read);
                assert!(futures::poll!(read.as_mut()).is_pending());
            }
            drop(socket);
            task.abort();
        }
    }
    #[tokio::test]
    async fn production_partial_write_retirement_keeps_capacity_until_the_reader_drops() {
        use std::sync::atomic::Ordering;
        for opcode in [0x89, 0x88] {
            let (socket, mut client, signal, allowance, task) = scripted_upgrade().await;
            let state = crate::archipelago::test_support::state_over(
                crate::archipelago::Settings::for_tests(),
                Arc::new(crate::gatekeeper::bus::DisabledBus),
            )
            .await;
            let permit = state.sockets.clone().acquire_owned().await.unwrap();
            let capacity = state.settings.server.max_sockets as usize;
            let clock_guard = tokio::spawn(async {
                loop {
                    tokio::task::yield_now().await;
                }
            });
            tokio::time::pause();
            allowance.store(2, Ordering::SeqCst);
            let mut socket = super::super::io::WebSocket::new(
                socket,
                state.clone(),
                permit,
                Some(signal.clone()),
            );
            assert!(
                super::super::io::send(
                    &mut socket,
                    axum::extract::ws::Message::Binary(b"secret".to_vec().into()),
                    None,
                    &state.feed,
                )
                .await
            );
            let mut prefix = [0; 2];
            client.read_exact(&mut prefix).await.unwrap();
            assert_eq!(prefix, [0x82, 6]);
            client.write_all(&[opcode, 0x80, 1, 2, 3, 4]).await.unwrap();
            assert!(matches!(
                socket.recv().await,
                Some(Ok(
                    axum::extract::ws::Message::Ping(_) | axum::extract::ws::Message::Close(_)
                ))
            ));
            if opcode == 0x89 {
                let write_deadline = Instant::now() + std::time::Duration::from_secs(5);
                let before_write = write_deadline - std::time::Duration::from_millis(1);
                tokio::time::advance(before_write.duration_since(Instant::now())).await;
                assert!(!signal.state.lock().terminal);
                assert_eq!(state.feed.socket_write_timeout_count(), 0);
                advance_timer(write_deadline).await;
                wait_until("partial-write deadline publishes raw retirement", || {
                    signal.state.lock().terminal
                })
                .await;
                assert_eq!(state.feed.socket_write_timeout_count(), 1);
            } else {
                // Close must retire the raw gate before recv returns, while the
                // writer may still be waiting for its cancellation poll.
                assert!(signal.state.lock().terminal);
            }
            allowance.store(usize::MAX, Ordering::SeqCst);
            assert_eq!(state.sockets.available_permits(), capacity - 1);
            let mut bytes = [0; 16];
            {
                let read = client.read(&mut bytes);
                tokio::pin!(read);
                assert!(futures::poll!(read.as_mut()).is_pending());
            }
            drop(socket);
            wait_until(
                "reader drop returns every physical transport permit",
                || state.sockets.available_permits() == capacity,
            )
            .await;
            assert_eq!(signal.take_activity(), None);
            tokio::time::resume();
            clock_guard.abort();
            task.abort();
        }
    }
}
