use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::UdpSocket;

use rusty_enet as enet;

use super::application_budget::{ApplicationBudget, ApplicationGlobal, ApplicationPayload};
use crate::pulse::transport::packet::Packet;
use crate::pulse::transport::peer::PeerId;

const SERVICE_POLL: Duration = Duration::from_millis(10);
const NATIVE_RECEIVE_POLICY: enet::ReceivePolicy = enet::ReceivePolicy {
    maximum_packet_size: 8192,
    maximum_queued_bytes: 65536,
    maximum_queued_commands: 256,
};
pub(crate) const RELAY_TIMEOUT_MS: u32 = 5000;
/// How often the socket warns about the datagrams errors cost it; the metric counts every one.
const SOCKET_ERROR_WARNING_INTERVAL: Duration = Duration::from_secs(10);
/// Receive errors one `receive` call skips before it hands the service loop back.
const RECEIVE_ERRORS_PER_CALL: usize = 64;
/// Winsock codes std maps to no `ErrorKind`.
#[cfg(windows)]
const WSAEMSGSIZE: i32 = 10040;
#[cfg(windows)]
const WSAENETRESET: i32 = 10052;

#[derive(Debug, Clone)]
pub struct HostConfig {
    pub bind: SocketAddr,
    pub max_peers: usize,
    pub channel_limit: u8,
}

#[derive(Debug)]
pub enum Event {
    /// Local native parser outcome. Aggregate limits only refuse the incoming command.
    ReceiveLimitExceeded {
        peer: PeerId,
        channel: u8,
        kind: enet::ReceiveLimitKind,
    },
    Connect {
        peer: PeerId,
        /// Source IP, when the transport knows it at connect time (WebTransport). ENet leaves
        /// this `None`; the server resolves it via [`Host::peer_ip`].
        ip: Option<String>,
        /// Immutable WebTransport connection ID for refusing this connection even after slot reuse.
        webtransport_connection: Option<u64>,
    },
    Receive {
        peer: PeerId,
        channel: u8,
        packet: Packet,
    },
    Disconnect {
        peer: PeerId,
    },
    /// Transport-level malformed input (a stream frame over the cap and up to twice it, whose
    /// payload is skipped, or a datagram too short to hold its header). Only WebTransport emits
    /// this -- ENet frames packets itself. The server charges it against the shared
    /// corrupted-packet budget, exactly like a protobuf decode failure, and disconnects the peer
    /// on exhaustion.
    Corrupt {
        peer: PeerId,
    },
    /// A WebTransport stream frame declared a payload over twice the message cap, and the
    /// stream is not read past it. Like an ENet packet over twice its receive buffer, the server
    /// disconnects the peer at once with `PacketCorrupted`.
    Oversized {
        peer: PeerId,
    },
}

pub struct Host {
    inner: enet::Host<ReadySocket>,
    application_global: ApplicationGlobal,
    application_peers: HashMap<PeerId, ApplicationBudget>,
}

struct ReadySocket {
    socket: UdpSocket,
    warnings: WarningThrottle,
    #[cfg(test)]
    faults: SocketFaults,
}

impl ReadySocket {
    fn new(socket: UdpSocket) -> Self {
        Self {
            socket,
            warnings: WarningThrottle::default(),
            #[cfg(test)]
            faults: SocketFaults::default(),
        }
    }

    /// A datagram the socket could not send or receive. ENet carries on without it: it resends
    /// what was reliable and times out a peer that stays unreachable.
    fn lost(&mut self, operation: &'static str, error: &std::io::Error) {
        crate::pulse::metrics::enet_socket_error(operation, error.kind());
        if let Some(held_back) = self.warnings.due(std::time::Instant::now()) {
            tracing::warn!(
                operation,
                %error,
                held_back,
                "ENet socket error cost one datagram; Pulse keeps serving"
            );
        }
    }

    fn try_send_to(&self, buffer: &[u8], address: SocketAddr) -> std::io::Result<usize> {
        #[cfg(test)]
        if let Some(error) = self.faults.take(|queues| &mut queues.send) {
            return Err(error);
        }
        self.socket.try_send_to(buffer, address)
    }

    fn try_recv_from(&self, buffer: &mut [u8]) -> std::io::Result<(usize, SocketAddr)> {
        #[cfg(test)]
        if let Some(error) = self.faults.take(|queues| &mut queues.receive) {
            return Err(error);
        }
        self.socket.try_recv_from(buffer)
    }
}

impl enet::Socket for ReadySocket {
    type Address = SocketAddr;
    type Error = std::io::Error;

    fn init(&mut self, _: enet::SocketOptions) -> std::io::Result<()> {
        self.socket.set_broadcast(true)
    }

    /// Every send error concerns this datagram and its address (no route to it, a route or rule
    /// that refuses it, no buffer for it now), so it costs that datagram. A socket that is gone
    /// fails the next receive as well, which stops the server.
    fn send(&mut self, address: SocketAddr, buffer: &[u8]) -> std::io::Result<usize> {
        match self.try_send_to(buffer, address) {
            Ok(sent) => Ok(sent),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(0),
            Err(error) => {
                self.lost("send", &error);
                Ok(0)
            }
        }
    }

    fn receive(
        &mut self,
        buffer: &mut [u8; enet::MTU_MAX],
    ) -> std::io::Result<Option<(SocketAddr, enet::PacketReceived)>> {
        for _ in 0..RECEIVE_ERRORS_PER_CALL {
            match self.try_recv_from(buffer) {
                Ok((len, address)) => {
                    return Ok(Some((address, enet::PacketReceived::Complete(len))));
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(None),
                // Interrupted before it read anything: nothing is lost, so read again.
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) if reports_one_datagram(&error) => self.lost("receive", &error),
                Err(error) => {
                    tracing::error!(%error, "ENet socket failed; Pulse stops");
                    return Err(error);
                }
            }
        }
        // What is still queued waits for the next service call.
        Ok(None)
    }
}

/// A receive error that reports on one datagram, not on the socket: the ICMP answer to an earlier
/// datagram (a peer's closed port or unreachable address: Windows reports it by default, Linux
/// only to a connected socket or one with IP_RECVERR), and on Windows a datagram larger than the
/// buffer (WSAEMSGSIZE; Linux truncates it instead) or an expired TTL (WSAENETRESET).
fn reports_one_datagram(error: &std::io::Error) -> bool {
    use std::io::ErrorKind::{
        ConnectionRefused, ConnectionReset, HostUnreachable, NetworkUnreachable,
    };
    if matches!(
        error.kind(),
        ConnectionReset | ConnectionRefused | HostUnreachable | NetworkUnreachable
    ) {
        return true;
    }
    #[cfg(windows)]
    if matches!(error.raw_os_error(), Some(WSAEMSGSIZE | WSAENETRESET)) {
        return true;
    }
    false
}

/// One warning per interval about socket errors the server carried on through.
#[derive(Default)]
struct WarningThrottle {
    last: Option<std::time::Instant>,
    held_back: u64,
}

impl WarningThrottle {
    /// Whether a warning is due at `now`, with how many errors went unwarned since the last one.
    fn due(&mut self, now: std::time::Instant) -> Option<u64> {
        match self.last {
            Some(last) if now.saturating_duration_since(last) < SOCKET_ERROR_WARNING_INTERVAL => {
                self.held_back += 1;
                None
            }
            _ => {
                self.last = Some(now);
                Some(std::mem::take(&mut self.held_back))
            }
        }
    }
}

/// Errors the socket returns in place of its next real calls, one per call and in order: how a
/// test produces what the operating system reports without making the network misbehave.
#[cfg(test)]
#[derive(Clone, Default)]
pub(crate) struct SocketFaults(std::sync::Arc<std::sync::Mutex<FaultQueues>>);

#[cfg(test)]
#[derive(Default)]
struct FaultQueues {
    send: std::collections::VecDeque<std::io::Error>,
    receive: std::collections::VecDeque<std::io::Error>,
}

#[cfg(test)]
impl SocketFaults {
    pub(crate) fn fail_next_send(&self, error: std::io::Error) {
        self.0.lock().unwrap().send.push_back(error);
    }

    pub(crate) fn fail_next_receive(&self, error: std::io::Error) {
        self.0.lock().unwrap().receive.push_back(error);
    }

    /// Faults no socket call has returned yet.
    pub(crate) fn pending(&self) -> usize {
        let queues = self.0.lock().unwrap();
        queues.send.len() + queues.receive.len()
    }

    fn take(
        &self,
        queue: impl FnOnce(&mut FaultQueues) -> &mut std::collections::VecDeque<std::io::Error>,
    ) -> Option<std::io::Error> {
        queue(&mut self.0.lock().unwrap()).pop_front()
    }
}

#[cfg(test)]
thread_local! {
    pub static HOST_FLUSH_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// An ENet clock that moves only when a test advances it, so round trips and timeouts are
/// measured in the test's steps rather than in how fast the machine delivers loopback datagrams.
#[cfg(test)]
#[derive(Clone)]
pub(crate) struct ManualClock(std::sync::Arc<std::sync::atomic::AtomicU64>);

#[cfg(test)]
impl ManualClock {
    /// Starts well past zero: ENet reads a zero timestamp as "unset".
    pub(crate) fn new() -> Self {
        Self(std::sync::Arc::new(std::sync::atomic::AtomicU64::new(
            1_000_000,
        )))
    }

    pub(crate) fn now(&self) -> Duration {
        Duration::from_millis(self.0.load(std::sync::atomic::Ordering::SeqCst))
    }

    pub(crate) fn advance(&self, by: Duration) {
        self.0
            .fetch_add(by.as_millis() as u64, std::sync::atomic::Ordering::SeqCst);
    }
}

fn to_enet_peer(id: PeerId) -> enet::PeerID {
    enet::PeerID(id as usize)
}

fn packet_kind(packet: &Packet) -> enet::PacketKind {
    if packet.flags.is_reliable() {
        enet::PacketKind::Reliable
    } else if packet.flags.is_unsequenced() {
        enet::PacketKind::Unreliable { sequenced: false }
    } else {
        enet::PacketKind::Unreliable { sequenced: true }
    }
}

impl Host {
    pub async fn bind(config: HostConfig) -> std::io::Result<Self> {
        let socket = Self::bound(&config).await?;
        Self::over(socket, &config, None)
    }

    /// [`Host::bind`] with ENet's timestamps read from `clock`.
    #[cfg(test)]
    pub(crate) async fn bind_with_clock(
        config: HostConfig,
        clock: ManualClock,
    ) -> std::io::Result<Self> {
        let socket = Self::bound(&config).await?;
        Self::over(socket, &config, Some(Box::new(move || clock.now())))
    }

    /// [`Host::bind`] whose socket returns `faults` in place of its next real calls.
    #[cfg(test)]
    pub(crate) async fn bind_with_faults(
        config: HostConfig,
        faults: SocketFaults,
    ) -> std::io::Result<Self> {
        let mut host = Self::bind(config).await?;
        host.inner.socket_mut().faults = faults;
        Ok(host)
    }

    async fn bound(config: &HostConfig) -> std::io::Result<UdpSocket> {
        let socket = UdpSocket::bind(config.bind).await?;
        socket.writable().await?;
        Ok(socket)
    }

    fn over(
        socket: UdpSocket,
        config: &HostConfig,
        time: Option<Box<dyn Fn() -> Duration>>,
    ) -> std::io::Result<Self> {
        let mut settings = enet::HostSettings {
            receive_policy: Some(NATIVE_RECEIVE_POLICY),
            peer_limit: config.max_peers,
            channel_limit: config.channel_limit as usize,
            ..Default::default()
        };
        if let Some(time) = time {
            settings.time = time;
        }
        let inner = enet::Host::new(ReadySocket::new(socket), settings)
            .map_err(|e| std::io::Error::other(format!("enet host: {e:?}")))?;
        Ok(Self {
            inner,
            application_global: ApplicationBudget::global(),
            application_peers: HashMap::new(),
        })
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.inner.socket().socket.local_addr()
    }

    pub fn peer_ip(&mut self, peer: PeerId) -> Option<String> {
        self.inner
            .get_peer_mut(to_enet_peer(peer))
            .and_then(|p| p.address())
            .map(|addr| addr.ip().to_string())
    }

    pub fn connect(&mut self, addr: SocketAddr, channels: u8) -> std::io::Result<PeerId> {
        let peer = self
            .inner
            .connect(addr, channels as usize, 0)
            .map_err(|e| std::io::Error::other(format!("{e:?}")))?;
        Ok(peer.id().0 as PeerId)
    }

    /// An error is the socket's own: what is left once [`ReadySocket`] has carried on through the
    /// errors that cost one datagram. It means the socket is gone.
    pub async fn service(&mut self) -> std::io::Result<Option<Event>> {
        let event = self.inner.service()?.map(|e| e.no_ref());
        match event {
            Some(enet::EventNoRef::ReceiveLimitExceeded {
                peer,
                channel_id,
                kind,
            }) => Ok(Some(Event::ReceiveLimitExceeded {
                peer: peer.0 as PeerId,
                channel: channel_id,
                kind,
            })),
            Some(enet::EventNoRef::Connect { peer, .. }) => {
                self.application_peers.remove(&(peer.0 as PeerId));
                Ok(Some(Event::Connect {
                    peer: peer.0 as PeerId,
                    ip: None,
                    webtransport_connection: None,
                }))
            }
            Some(enet::EventNoRef::Disconnect { peer, .. }) => {
                self.application_peers.remove(&(peer.0 as PeerId));
                Ok(Some(Event::Disconnect {
                    peer: peer.0 as PeerId,
                }))
            }
            Some(enet::EventNoRef::Receive {
                peer,
                channel_id,
                packet,
            }) => {
                let packet = Packet::from_enet(channel_id, &packet);
                Ok(Some(Event::Receive {
                    peer: peer.0 as PeerId,
                    channel: channel_id,
                    packet,
                }))
            }
            None => {
                tokio::select! {
                    ready = self.inner.socket().socket.readable() => { ready?; }
                    _ = tokio::time::sleep(SERVICE_POLL) => {}
                }
                Ok(None)
            }
        }
    }

    pub async fn disconnect(&mut self, peer: PeerId, reason: u32) -> std::io::Result<()> {
        if let Some(p) = self.inner.get_peer_mut(to_enet_peer(peer)) {
            p.disconnect(reason);
            self.inner.flush();
        }
        Ok(())
    }

    pub async fn disconnect_now(&mut self, peer: PeerId, reason: u32) -> std::io::Result<()> {
        self.application_peers.remove(&peer);
        if let Some(p) = self.inner.get_peer_mut(to_enet_peer(peer)) {
            p.disconnect_now(reason);
            self.inner.flush();
        }
        Ok(())
    }

    pub async fn send(&mut self, peer: PeerId, packet: Packet) -> std::io::Result<()> {
        let kind = packet_kind(&packet);
        let raw = if packet.flags.is_unthrottled() {
            enet::Packet::new_unthrottled(packet.data.as_ref(), kind)
        } else {
            enet::Packet::new(packet.data.as_ref(), kind)
        };
        let Some(p) = self.inner.get_peer_mut(to_enet_peer(peer)) else {
            return Ok(());
        };
        if p.state() != enet::PeerState::Connected {
            return Ok(());
        }
        let _ = p.send(packet.channel, &raw);
        Ok(())
    }

    pub async fn send_application(&mut self, peer: PeerId, packet: Packet) -> std::io::Result<()> {
        self.queue_application(peer, packet, false).await
    }

    pub(super) async fn queue_application(
        &mut self,
        peer: PeerId,
        packet: Packet,
        control: bool,
    ) -> std::io::Result<()> {
        let kind = packet_kind(&packet);
        let p = self
            .inner
            .get_peer_mut(to_enet_peer(peer))
            .filter(|peer| peer.state() == enet::PeerState::Connected)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotConnected,
                    "application peer unavailable",
                )
            })?;
        let budget = self
            .application_peers
            .entry(peer)
            .or_insert_with(|| ApplicationBudget::new(self.application_global.clone()));
        let permit = budget.reserve(packet.data.len(), control)?;
        let payload = Box::new(ApplicationPayload {
            bytes: packet.data,
            _permit: permit,
        });
        let raw = if packet.flags.is_unthrottled() {
            enet::Packet::new_unthrottled(payload, kind)
        } else {
            enet::Packet::new(payload, kind)
        };
        p.send(packet.channel, &raw)
            .map_err(|error| std::io::Error::other(format!("application send: {error:?}")))
    }

    pub fn set_relay_timeout(&mut self, peer: PeerId, relay: bool) {
        let bound = if relay { RELAY_TIMEOUT_MS } else { 0 };
        if let Some(p) = self.inner.get_peer_mut(to_enet_peer(peer)) {
            p.set_timeout(0, bound, bound);
        }
    }

    #[cfg(test)]
    pub(crate) fn peer_timeout(&mut self, peer: PeerId) -> Option<(u32, u32)> {
        let debug = format!("{:?}", self.inner.get_peer_mut(to_enet_peer(peer))?);
        let field = |name: &str| {
            let (_, rest) = debug.split_once(name)?;
            let digits = rest.trim_start_matches([':', ' ']);
            let end = digits.find(|c: char| !c.is_ascii_digit())?;
            digits[..end].parse::<u32>().ok()
        };
        Some((field("timeoutMinimum")?, field("timeoutMaximum")?))
    }

    /// One `enet_host_flush` for the whole tick's outbox; `send` only queues.
    pub fn flush(&mut self) {
        #[cfg(test)]
        HOST_FLUSH_CALLS.with(|c| c.set(c.get() + 1));
        self.inner.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pulse::transport::packet::Packet;

    fn loopback() -> SocketAddr {
        "127.0.0.1:0".parse().unwrap()
    }

    #[tokio::test]
    async fn flush_called_once_per_tick() {
        let mut server = Host::bind(HostConfig {
            bind: loopback(),
            max_peers: 8,
            channel_limit: 8,
        })
        .await
        .unwrap();
        let server_addr = server.local_addr().unwrap();

        let mut client = Host::bind(HostConfig {
            bind: loopback(),
            max_peers: 8,
            channel_limit: 8,
        })
        .await
        .unwrap();
        client.connect(server_addr, 8).unwrap();

        let mut server_peer: Option<PeerId> = None;
        for _ in 0..200 {
            if let Some(Event::Connect { peer, .. }) = server.service().await.unwrap() {
                server_peer = Some(peer);
                break;
            }
            let _ = client.service().await.unwrap();
        }
        let peer = server_peer.expect("enet loopback handshake did not complete");

        HOST_FLUSH_CALLS.with(|c| c.set(0));
        const M: usize = 20;
        for _ in 0..M {
            server
                .send(peer, Packet::reliable(0, vec![1u8, 2, 3]))
                .await
                .unwrap();
        }
        server.flush();

        assert_eq!(
            HOST_FLUSH_CALLS.with(|c| c.get()),
            1,
            "expected a single coalesced flush for the whole tick, not one per message"
        );
    }

    #[tokio::test]
    async fn a_socket_that_keeps_failing_hands_the_service_loop_back() {
        let faults = SocketFaults::default();
        let queued = 1000;
        for _ in 0..queued {
            faults.fail_next_receive(std::io::ErrorKind::ConnectionReset.into());
        }
        let mut host = Host::bind_with_faults(
            HostConfig {
                bind: loopback(),
                max_peers: 8,
                channel_limit: 8,
            },
            faults.clone(),
        )
        .await
        .unwrap();

        assert!(host.service().await.unwrap().is_none());
        assert!(
            faults.pending() > 0,
            "one service call went through all {queued} errors before returning"
        );
    }

    #[test]
    fn socket_error_warnings_come_once_per_interval_and_count_the_ones_held_back() {
        let start = std::time::Instant::now();
        let mut throttle = WarningThrottle::default();
        let just_before = SOCKET_ERROR_WARNING_INTERVAL - Duration::from_millis(1);

        assert_eq!(throttle.due(start), Some(0));
        assert_eq!(throttle.due(start + Duration::from_millis(1)), None);
        assert_eq!(throttle.due(start + just_before), None);
        assert_eq!(throttle.due(start + SOCKET_ERROR_WARNING_INTERVAL), Some(2));
        assert_eq!(
            throttle.due(start + SOCKET_ERROR_WARNING_INTERVAL + Duration::from_millis(1)),
            None
        );
    }
}
