//! Native Pulse driver — a dedicated OS thread running a current-thread Tokio runtime around the
//! ENet host.
//!
//! ENet is a synchronous, non-thread-safe poll loop, so it gets its own thread that owns the host.
//! Rather than busy-polling, the thread blocks in [`tokio::select!`] on the two work sources — the
//! outbound [`PulseFrame`] channel and UDP-socket readability — plus a coarse maintenance tick so
//! ENet's own timers (pings, reliable retransmits) keep ticking when otherwise idle. After any
//! wake it drains outbound onto the peer and services the host. The host's socket is a Tokio
//! [`UdpSocket`] driven non-blocking via `try_recv_from` / `try_send_to`. The thread never waits
//! on the protocol layer: what a full inbound queue cannot take is held in a [`Backlog`].
//!
//! The host is a (locally patched) [`rusty_enet`] — a pure-Rust port of ENet retargeted to the
//! nxrighthere/SoftwareGuy ENet-CSharp "modified protocol" fork the server runs (the patch is the
//! three header-flag constants in the fork's `src/c/protocol.rs`). No compressor and no checksum,
//! matching the server's host exactly.

use std::collections::VecDeque;
use std::io::{self, ErrorKind};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use rusty_enet as enet;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::{self, error::TrySendError};

use super::transport::{
    collapse_inputs, PulseDisconnect, PulseDriverChannels, PulseFrame, PulseReliability,
    PulseStatus, PulseTransportConfig,
};

/// Channels in server order: RELIABLE=0, UNRELIABLE_SEQUENCED=1, UNRELIABLE_UNSEQUENCED=2
/// (the server's `ENetChannel.COUNT`).
const CHANNEL_COUNT: usize = 3;
const CHANNEL_RELIABLE: u8 = 0;

/// How long to block with no inbound/outbound activity before servicing the host anyway, so ENet's
/// pings and reliable retransmits keep ticking on an otherwise-idle connection.
const MAINTENANCE_INTERVAL: Duration = Duration::from_millis(100);

/// unity-explorer `ENetTransportOptions.ConnectTimeoutMs` (10000), for DNS and for the ENet
/// connect: `ResolveIPv4Async` and `ConnectAsync` in `ENetTransport.cs`, both in
/// `Explorer/Assets/DCL/Multiplayer/Connections/Pulse/ENet/`.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Reliable messages held while the protocol layer's queue is full. Past it the connection fails
/// (retryably) rather than grow without end behind an engine that has stopped reading.
const MAX_HELD_RELIABLE: usize = 65_536;

pub(super) fn spawn(
    config: PulseTransportConfig,
    channels: PulseDriverChannels,
    stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("pulse-enet".into())
        .spawn(move || run(config, channels, &stop))
        .expect("failed to spawn pulse-enet thread")
}

/// Tokio [`UdpSocket`] adapter implementing rusty_enet's [`enet::Socket`]. The newtype is required
/// by the orphan rule; the methods just bridge to Tokio's non-blocking datagram ops.
struct PulseSocket(UdpSocket);

impl enet::Socket for PulseSocket {
    type Address = SocketAddr;
    type Error = io::Error;

    fn init(&mut self, _options: enet::SocketOptions) -> io::Result<()> {
        // Tokio sockets are already non-blocking; match the std impl's broadcast flag.
        self.0.set_broadcast(true)
    }

    fn send(&mut self, address: SocketAddr, buffer: &[u8]) -> io::Result<usize> {
        match self.0.try_send_to(buffer, address) {
            Ok(sent) => Ok(sent),
            Err(err) if err.kind() == ErrorKind::WouldBlock => Ok(0),
            Err(err) => Err(err),
        }
    }

    fn receive(
        &mut self,
        buffer: &mut [u8; enet::MTU_MAX],
    ) -> io::Result<Option<(SocketAddr, enet::PacketReceived)>> {
        match self.0.try_recv_from(buffer) {
            Ok((len, address)) => Ok(Some((address, enet::PacketReceived::Complete(len)))),
            Err(err) if err.kind() == ErrorKind::WouldBlock => Ok(None),
            Err(err) => Err(err),
        }
    }
}

fn run(config: PulseTransportConfig, mut channels: PulseDriverChannels, stop: &AtomicBool) {
    let _ = channels.status.try_send(PulseStatus::Connecting);

    // One OS thread, one current-thread runtime — keeps ENet single-threaded while letting us block
    // on socket/channel readiness instead of spinning.
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => return fail(&mut channels, format!("tokio runtime build failed: {err}")),
    };
    runtime.block_on(drive(&mut channels, &config, stop));
    // A lookup still running on the runtime's blocking pool cannot be cancelled, and dropping the
    // runtime would wait for it while `PulseDriverHandle::drop` waits on this thread: leave it
    // behind.
    runtime.shutdown_background();
}

/// Resolve the server address (DNS). `None` ends the driver: either the failure was reported, as
/// never-established → Failed (retryable), or the protocol layer dropped the link meanwhile, and
/// nobody is left to tell.
async fn resolve(
    channels: &mut PulseDriverChannels,
    config: &PulseTransportConfig,
) -> Option<SocketAddr> {
    let mut lookup = std::pin::pin!(tokio::time::timeout(
        CONNECT_TIMEOUT,
        tokio::net::lookup_host((config.host.as_str(), config.port)),
    ));
    // The lookup is a blocking call that cannot be cancelled, so the link's drop is raced against
    // it: the driver can leave at once, and not when the resolver gives up.
    let outcome = loop {
        tokio::select! {
            outcome = &mut lookup => break outcome,
            frame = channels.outbound.recv() => match frame {
                // Nothing can be sent before the connection is up: `queue_frame` would drop it.
                Some(_) => {}
                None => return None,
            },
        }
    };

    let message = match outcome {
        Ok(Ok(addrs)) => match pick_address(addrs) {
            Some(address) => return Some(address),
            None => format!("no address for {}", config.host),
        },
        Ok(Err(err)) => format!("resolve {} failed: {err}", config.host),
        Err(_) => format!("resolve {} timed out", config.host),
    };
    fail(channels, message);
    None
}

/// The first IPv4 address, else the first IPv6 one. unity-explorer takes the first IPv4 address
/// and fails without one (`ENetTransport.ResolveIPv4Async`), and the servers bind `0.0.0.0` by
/// default, so IPv6 is only a fallback.
fn pick_address(addrs: impl IntoIterator<Item = SocketAddr>) -> Option<SocketAddr> {
    let mut first_ipv6 = None;
    for address in addrs {
        if address.is_ipv4() {
            return Some(address);
        }
        first_ipv6.get_or_insert(address);
    }
    first_ipv6
}

/// Reliable messages the protocol layer's queue had no room for, in arrival order. Waiting for
/// room is not an option: the thread would service nothing meanwhile, and the server drops a peer
/// that stops acknowledging within seconds.
struct Backlog {
    held: VecDeque<Vec<u8>>,
    cap: usize,
}

/// The backlog is at its cap.
#[derive(Debug, PartialEq)]
struct BacklogFull;

impl Backlog {
    fn with_cap(cap: usize) -> Self {
        Self {
            held: VecDeque::new(),
            cap,
        }
    }

    /// Hand `bytes` over, or, when the queue is full, hold it (reliable) or drop it (unreliable).
    /// A reliable message never overtakes one already held.
    fn deliver(
        &mut self,
        tx: &mpsc::Sender<Vec<u8>>,
        bytes: Vec<u8>,
        reliable: bool,
    ) -> Result<(), BacklogFull> {
        if !reliable {
            // Superseded by the next one anyway.
            let _ = tx.try_send(bytes);
            return Ok(());
        }
        let bytes = if self.held.is_empty() {
            match tx.try_send(bytes) {
                Err(TrySendError::Full(bytes)) => bytes,
                // Handed over, or the link is gone and the driver is about to stop.
                _ => return Ok(()),
            }
        } else {
            bytes
        };
        if self.held.len() >= self.cap {
            return Err(BacklogFull);
        }
        self.held.push_back(bytes);
        Ok(())
    }

    /// Hand over what is held, in order, while there is room.
    fn flush(&mut self, tx: &mpsc::Sender<Vec<u8>>) {
        while let Some(bytes) = self.held.pop_front() {
            if let Err(TrySendError::Full(bytes)) = tx.try_send(bytes) {
                self.held.push_front(bytes);
                break;
            }
        }
    }
}

async fn drive(
    channels: &mut PulseDriverChannels,
    config: &PulseTransportConfig,
    stop: &AtomicBool,
) {
    let Some(address) = resolve(channels, config).await else {
        return;
    };

    // An IPv4 socket cannot send to an IPv6 address, so the socket follows the address.
    let local: IpAddr = if address.is_ipv6() {
        Ipv6Addr::UNSPECIFIED.into()
    } else {
        Ipv4Addr::UNSPECIFIED.into()
    };
    // Ephemeral local UDP socket; ENet drives it non-blocking via `PulseSocket`.
    let socket = match UdpSocket::bind((local, 0)).await {
        Ok(socket) => PulseSocket(socket),
        Err(err) => return fail(channels, format!("udp bind failed: {err}")),
    };

    // One peer (the server), three channels, no compressor/checksum (`HostSettings` defaults).
    let mut host = match enet::Host::new(
        socket,
        enet::HostSettings {
            peer_limit: 1,
            channel_limit: CHANNEL_COUNT,
            ..Default::default()
        },
    ) {
        Ok(host) => host,
        Err(err) => return fail(channels, format!("enet host create failed: {err}")),
    };

    let peer = match host.connect(address, CHANNEL_COUNT, 0) {
        Ok(peer) => peer.id(),
        Err(err) => return fail(channels, format!("enet connect failed: {err}")),
    };
    let connecting_since = Instant::now();
    let mut connected = false;
    let mut backlog = Backlog::with_cap(MAX_HELD_RELIABLE);

    while !stop.load(Ordering::Relaxed) {
        // Whatever the protocol layer has made room for since the last pass.
        backlog.flush(&channels.inbound);
        // ENet would give up on its own only after up to 30 s. The loop wakes at least every
        // `MAINTENANCE_INTERVAL`, so this is checked about as often.
        if !connected && connecting_since.elapsed() >= CONNECT_TIMEOUT {
            return fail(channels, "connect timed out".to_owned());
        }

        // Block until there's work: an outbound frame to send, an inbound datagram to read, or the
        // maintenance deadline so ENet's timers keep ticking.
        let outbound = tokio::select! {
            frame = channels.outbound.recv() => Some(frame),
            _ = host.socket().0.readable() => None,
            _ = tokio::time::sleep(MAINTENANCE_INTERVAL) => None,
        };
        let mut batch = Vec::new();
        match outbound {
            // Channel closed — the protocol layer dropped the link; stop the driver.
            Some(None) => break,
            Some(Some(frame)) => batch.push(frame),
            None => {}
        }

        // Drain any further queued outbound without blocking; the next service flushes them.
        while let Ok(frame) = channels.outbound.try_recv() {
            batch.push(frame);
        }
        // Frames queued behind each other go out as one batch, so a run of inputs is cut down to
        // its newest before it reaches the peer.
        for frame in collapse_inputs(batch) {
            queue_frame(&mut host, peer, &frame);
        }

        // Service the host: socket I/O + dispatch all ready events.
        loop {
            match host.service() {
                Ok(Some(enet::Event::Connect { .. })) => {
                    connected = true;
                    let _ = channels.status.try_send(PulseStatus::Connected);
                }
                Ok(Some(enet::Event::Disconnect { data, .. })) => {
                    let reason = PulseDisconnect::from_code(data);
                    let _ = channels.status.try_send(PulseStatus::Disconnected(reason));
                    return;
                }
                Ok(Some(enet::Event::Receive {
                    channel_id, packet, ..
                })) => {
                    // Every packet is surfaced, on or off a Pulse realm: the protocol layer's
                    // decoder must see each `PlayerLeft` to keep its subject table an honest mirror
                    // of the server's view of us, and it gates what reaches the engine itself
                    // (nothing routes without a live routing entity). So a reliable packet is held
                    // for room rather than dropped while the main thread stalls, and this thread
                    // goes on servicing ENet meanwhile. Unreliable ones are superseded anyway.
                    let bytes = packet.data().to_vec();
                    let reliable = channel_id == CHANNEL_RELIABLE;
                    if backlog.deliver(&channels.inbound, bytes, reliable).is_err() {
                        return fail(
                            channels,
                            format!("inbound backlog over {MAX_HELD_RELIABLE} messages"),
                        );
                    }
                }
                // This host keeps receive_policy=None; preserve exhaustive handling of vendor events.
                Ok(Some(enet::Event::ReceiveLimitExceeded { .. })) => {}
                Ok(None) => break,
                Err(err) => return fail(channels, format!("enet service error: {err}")),
            }
        }
    }

    // Stop requested by the protocol layer (session dropped) — disconnect cleanly and flush.
    host.peer_mut(peer)
        .disconnect(PulseDisconnect::GRACEFUL_CODE);
    host.flush();
    let _ = channels
        .status
        .try_send(PulseStatus::Disconnected(PulseDisconnect::Graceful));
}

fn packet_for_frame(frame: &PulseFrame) -> (u8, enet::Packet) {
    match frame.reliability {
        PulseReliability::Reliable => (0, enet::Packet::reliable(frame.bytes.as_slice())),
        PulseReliability::UnreliableSequenced => (
            1,
            enet::Packet::new_unthrottled(
                frame.bytes.as_slice(),
                enet::PacketKind::Unreliable { sequenced: true },
            ),
        ),
        PulseReliability::UnreliableUnsequenced => (
            2,
            enet::Packet::unreliable_unsequenced(frame.bytes.as_slice()),
        ),
    }
}

/// Queue one outbound [`PulseFrame`] onto the peer. Channel and packet kind together reproduce the
/// server's `ENetChannel` wire commands: reliable → `SEND_RELIABLE`, unreliable-sequenced →
/// `SEND_UNRELIABLE`, unreliable-unsequenced → `SEND_UNSEQUENCED`.
fn queue_frame(host: &mut enet::Host<PulseSocket>, peer: enet::PeerID, frame: &PulseFrame) {
    let (channel_id, packet) = packet_for_frame(frame);
    if let Err(err) = host.peer_mut(peer).send(channel_id, &packet) {
        bevy::log::warn!("pulse: peer send failed: {err}");
    }
}

/// Report a never-established failure (DNS/socket/connect). Always transient — the protocol layer
/// retries.
fn fail(channels: &mut PulseDriverChannels, message: String) {
    bevy::log::warn!("pulse: {message}");
    let _ = channels.status.try_send(PulseStatus::Failed(message));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pulse::transport::FrameKind;

    #[test]
    fn native_frame_mapping_preserves_channels_and_explicit_throttle_bypass() {
        for (reliability, channel, kind, bypass) in [
            (
                PulseReliability::Reliable,
                0,
                enet::PacketKind::Reliable,
                false,
            ),
            (
                PulseReliability::UnreliableSequenced,
                1,
                enet::PacketKind::Unreliable { sequenced: true },
                true,
            ),
            (
                PulseReliability::UnreliableUnsequenced,
                2,
                enet::PacketKind::Unreliable { sequenced: false },
                false,
            ),
        ] {
            let frame = PulseFrame {
                bytes: vec![1, 2, 3],
                reliability,
                kind: FrameKind::Control,
            };
            let (actual_channel, packet) = packet_for_frame(&frame);
            assert_eq!(actual_channel, channel);
            assert_eq!(packet.kind(), kind);
            assert_eq!(packet.is_unthrottled(), bypass);
            assert_eq!(packet.data(), frame.bytes.as_slice());
        }
    }
}

#[cfg(test)]
mod driver_tests;
