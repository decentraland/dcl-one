//! Native Pulse driver — a dedicated OS thread running a current-thread Tokio runtime around the
//! ENet host.
//!
//! ENet is a synchronous, non-thread-safe poll loop, so it gets its own thread that owns the host.
//! Rather than busy-polling, the thread blocks in [`tokio::select!`] on the two work sources — the
//! outbound [`PulseFrame`] channel and UDP-socket readability — plus a coarse maintenance tick so
//! ENet's own timers (pings, reliable retransmits) keep ticking when otherwise idle. After any
//! wake it drains outbound onto the peer and services the host. The host's socket is a Tokio
//! [`UdpSocket`] driven non-blocking via `try_recv_from` / `try_send_to`. The thread never waits
//! on the protocol layer: a reliable message a full inbound queue cannot take is held in a
//! [`Backlog`], and an unreliable one is dropped.
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

use super::pacing::DiscreteEventPacer;
use super::transport::{
    collapse_inputs, FrameKind, PulseDisconnect, PulseDriverChannels, PulseFrame, PulseReliability,
    PulseStatus, PulseTransportConfig, UplinkSnapshot,
};

/// Channels in server order: RELIABLE=0, UNRELIABLE_SEQUENCED=1, UNRELIABLE_UNSEQUENCED=2
/// (the server's `ENetChannel.COUNT`).
const CHANNEL_COUNT: usize = 3;
const CHANNEL_RELIABLE: u8 = 0;

/// How long to block with no inbound/outbound activity before servicing the host anyway, so ENet's
/// pings and reliable retransmits keep ticking on an otherwise-idle connection.
const MAINTENANCE_INTERVAL: Duration = Duration::from_millis(100);

/// How long the DNS lookup and the ENet connect each have before the attempt fails, retryably.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Reliable messages held while the protocol layer's queue is full. Past it the connection fails
/// (retryably) rather than grow without end behind an engine that has stopped reading.
const MAX_HELD_RELIABLE: usize = 65_536;
/// Bytes held while the protocol layer's queue is full. The host keeps ENet's default limit of
/// 32 MiB a packet, so the count alone would not bound the memory held; past either cap the
/// connection fails alike.
const MAX_HELD_BYTES: usize = 16 * 1024 * 1024;

/// How often the uplink is probed with a ping, whatever is in flight. ENet's own ping goes out
/// only `ping_interval` (500 ms) after the last acknowledgement, and only while nothing reliable is
/// in flight (`enet_protocol_send_outgoing_commands` in the vendored `src/c/protocol.rs`, where
/// `last_receive_time` is written only by `enet_protocol_handle_acknowledge`). So it never has
/// more than one ping out, an acknowledgement comes back at best once per round trip, and a lost
/// one waits for ENet's retransmission timeout (the round trip and four times its variance,
/// doubling on each retry). Probing every interval keeps acknowledgements arriving about that
/// often on a working uplink at any round trip. A ping carries no packet, so ENet's send window,
/// which counts packet bytes in transit, does not hold it back.
const PROBE_INTERVAL: Duration = Duration::from_millis(250);

/// How long reliable traffic (a probe at least) may be in flight with no acknowledgement arriving
/// before the uplink counts as stalled and inputs are held: what is sent meanwhile can land at
/// once when it recovers, and the servers charge their input budget (burst 16) on arrival. On a
/// working uplink that takes two probes or acknowledgements lost in a row, or one where jitter
/// widens the gap it leaves past this. An acknowledgement shows the uplink was up a round trip
/// earlier, so the gate closes about a round trip, this and a loop pass after a stall begins;
/// [`UplinkGate::input_spacing`] keeps what is sent in that time within the burst.
const STALL_AFTER: Duration = Duration::from_millis(600);

/// The smoothed round trip from which inputs are spaced by it ([`UplinkGate::input_spacing`]).
const PACE_INPUTS_FROM: Duration = Duration::from_millis(600);

/// Inputs that may go out in a round trip and [`STALL_AFTER`] from [`PACE_INPUTS_FROM`] on: half
/// the servers' input burst of 16, which leaves room for jitter, a loop pass and a held input
/// let out when the uplink is confirmed.
const INPUTS_PER_STALL_WINDOW: u32 = 8;

/// Reliable messages that may be unacknowledged before teleports and emotes wait: half the servers'
/// burst of 16, so ENet retransmitting all of them at once after a stall still fits. Handshakes,
/// resyncs and announcements count too (a burst of them delays a teleport a round trip), and are
/// never held.
const MAX_UNACKNOWLEDGED_RELIABLE: usize = 8;

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
            Err(err) if is_transient(&err) => {
                bevy::log::debug!("pulse: send skipped: {err}");
                Ok(0)
            }
            Err(err) => Err(err),
        }
    }

    fn receive(
        &mut self,
        buffer: &mut [u8; enet::MTU_MAX],
    ) -> io::Result<Option<(SocketAddr, enet::PacketReceived)>> {
        match self.0.try_recv_from(buffer) {
            Ok((len, address)) => Ok(Some((address, enet::PacketReceived::Complete(len)))),
            Err(err) if is_transient(&err) => {
                bevy::log::debug!("pulse: receive skipped: {err}");
                Ok(None)
            }
            Err(err) => Err(err),
        }
    }
}

/// `ENOBUFS`, which has no stable [`ErrorKind`]: the send queue is momentarily full.
#[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
const ENOBUFS: Option<i32> = Some(55);
#[cfg(any(target_os = "linux", target_os = "android"))]
const ENOBUFS: Option<i32> = Some(105);
#[cfg(windows)]
const ENOBUFS: Option<i32> = Some(10055); // WSAENOBUFS
#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "linux",
    target_os = "android",
    windows
)))]
const ENOBUFS: Option<i32> = None;

/// A socket error the network recovers from on its own: a lost route, an ICMP unreachable, a full
/// buffer. The datagram is lost like any other and ENet's own timeout decides whether the peer is
/// gone, rather than one error ending the session.
fn is_transient(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        ErrorKind::WouldBlock
            | ErrorKind::Interrupted
            | ErrorKind::ConnectionReset
            | ErrorKind::ConnectionRefused
            | ErrorKind::NetworkUnreachable
            | ErrorKind::HostUnreachable
            | ErrorKind::NetworkDown
    ) || (ENOBUFS.is_some() && err.raw_os_error() == ENOBUFS)
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

/// The first IPv4 address, else the first IPv6 one: the servers bind `0.0.0.0` by default, so IPv6
/// is only a fallback.
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
    /// The bytes of the messages in `held`.
    held_bytes: usize,
    max_messages: usize,
    max_bytes: usize,
}

/// Holding one more message would put the backlog over one of its caps.
#[derive(Debug, PartialEq)]
struct BacklogFull;

impl Backlog {
    fn with_caps(max_messages: usize, max_bytes: usize) -> Self {
        Self {
            held: VecDeque::new(),
            held_bytes: 0,
            max_messages,
            max_bytes,
        }
    }

    /// Hand `bytes` over, or, when the queue is full, hold it (reliable) or drop it (unreliable).
    /// A reliable message never overtakes one already held, and is not held past either cap.
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
        if self.held.len() >= self.max_messages || self.held_bytes + bytes.len() > self.max_bytes {
            return Err(BacklogFull);
        }
        self.held_bytes += bytes.len();
        self.held.push_back(bytes);
        Ok(())
    }

    /// Hand over what is held, in order, while there is room.
    fn flush(&mut self, tx: &mpsc::Sender<Vec<u8>>) {
        while let Some(bytes) = self.held.pop_front() {
            let len = bytes.len();
            if let Err(TrySendError::Full(bytes)) = tx.try_send(bytes) {
                self.held.push_front(bytes);
                break;
            }
            self.held_bytes -= len;
        }
    }
}

/// What the peer says about its reliable traffic (see the `rusty_enet` accessors).
#[derive(Debug, Clone, Copy, Default)]
struct PeerAcks {
    /// Anything reliable, a ping included, is unacknowledged.
    unacknowledged: bool,
    /// Reliable messages unacknowledged, pings excluded.
    packets: usize,
    /// Changes whenever an acknowledgement arrives.
    last_acknowledgement: u32,
    /// ENet's smoothed round trip.
    round_trip: Duration,
}

impl PeerAcks {
    fn of<S: enet::Socket>(peer: &enet::Peer<S>) -> Self {
        Self {
            unacknowledged: peer.has_unacknowledged_reliable(),
            packets: peer.unacknowledged_reliable_packets(),
            last_acknowledgement: peer.last_acknowledgement(),
            round_trip: peer.round_trip_time(),
        }
    }
}

/// Keeps what a stalled uplink would deliver in one burst within the servers' budgets. ENet
/// retransmits every unacknowledged teleport and emote at once when the link comes back, and the
/// servers charge them on arrival; a session that overruns a budget ends for good. So teleports and
/// emotes wait while [`MAX_UNACKNOWLEDGED_RELIABLE`] reliable messages are unacknowledged, and
/// inputs wait while traffic is in flight with no acknowledgement for [`STALL_AFTER`]. Only the
/// latest frame of each kind waits; an input also waits behind a waiting teleport or emote, whose
/// start carries an older pose. From a round trip of [`PACE_INPUTS_FROM`], inputs are also spaced
/// by it (see [`UplinkGate::input_spacing`]). A teleport that goes drops a held input, whose pose
/// it supersedes, so the servers never get an older pose after it.
#[derive(Default)]
struct UplinkGate {
    /// Since when reliable traffic has been in flight with no acknowledgement arriving.
    unacknowledged_since: Option<Instant>,
    /// The peer's acknowledgement mark as last observed.
    last_acknowledgement: u32,
    /// Reliable messages unacknowledged, counting those let out since the last observation.
    unacknowledged_reliable: usize,
    /// At most one frame of each kind, in the order they are to go.
    held: Vec<PulseFrame>,
    /// When the uplink was last probed.
    probed_at: Option<Instant>,
    /// Probes sent.
    probes: u64,
    /// ENet's smoothed round trip as last observed.
    round_trip: Duration,
    /// When an input last went out.
    input_sent_at: Option<Instant>,
    /// Since when an input has waited for the uplink, not for its spacing.
    input_held_since: Option<Instant>,
    /// Holds of an input that have ended, their total and the longest.
    input_holds: u64,
    input_held_for: Duration,
    longest_input_hold: Duration,
}

impl UplinkGate {
    /// What the peer says at `now`.
    fn observe(&mut self, now: Instant, acks: PeerAcks) {
        let acknowledged = acks.last_acknowledgement != self.last_acknowledgement;
        self.last_acknowledgement = acks.last_acknowledgement;
        self.unacknowledged_reliable = acks.packets;
        self.round_trip = acks.round_trip;
        if !acks.unacknowledged {
            self.unacknowledged_since = None;
        } else if acknowledged {
            // A busy link is not a stalled one: measure from the latest acknowledgement.
            self.unacknowledged_since = Some(now);
        } else {
            self.unacknowledged_since.get_or_insert(now);
        }
    }

    /// Whether to ping now: no probe went out for [`PROBE_INTERVAL`], whatever is in flight. With
    /// several probes in flight, an acknowledgement arrives about every interval on a working
    /// uplink at any round trip, and a lost probe or acknowledgement leaves a gap of one interval
    /// more rather than one retransmission timeout.
    fn wants_probe(&mut self, now: Instant) -> bool {
        let due = self
            .probed_at
            .is_none_or(|at| now.duration_since(at) >= PROBE_INTERVAL);
        if due {
            self.probed_at = Some(now);
            self.probes += 1;
        }
        due
    }

    /// What the gate holds back at `now`, for the debug panel and the Pulse state reply.
    fn snapshot(&self, now: Instant) -> UplinkSnapshot {
        let holds = |kind| self.held.iter().any(|held| held.kind == kind);
        let holding = self.input_held_since.map(|since| now.duration_since(since));
        UplinkSnapshot {
            stalled: self.stalled(now),
            unacknowledged_reliable: self.unacknowledged_reliable,
            held_teleport: holds(FrameKind::Teleport),
            held_emote: holds(FrameKind::Emote),
            held_input: self.input_held_since.is_some(),
            probes: self.probes,
            round_trip: self.round_trip,
            input_spacing: self.input_spacing(),
            input_holds: self.input_holds + u64::from(holding.is_some()),
            input_held_for: self.input_held_for + holding.unwrap_or_default(),
            longest_input_hold: self.longest_input_hold.max(holding.unwrap_or_default()),
        }
    }

    /// The least time between two inputs. Inputs sent between the start of a stall and the gate
    /// closing, about a round trip and [`STALL_AFTER`] later, reach the servers together when a
    /// stall that queued them ends. Below [`PACE_INPUTS_FROM`] that is at most about 1.3 s, in
    /// which `movement_pacing` sends at most 16 (its burst of 3 and 10 a second); from it on,
    /// inputs are spaced so that at most [`INPUTS_PER_STALL_WINDOW`] go out in that time.
    fn input_spacing(&self) -> Duration {
        if self.round_trip < PACE_INPUTS_FROM {
            return Duration::ZERO;
        }
        (self.round_trip + STALL_AFTER) / INPUTS_PER_STALL_WINDOW
    }

    /// How long until a held input may go by its spacing, when that is what it waits for; the
    /// driver wakes then, rather than at its next maintenance tick.
    fn paced_for(&self, now: Instant) -> Option<Duration> {
        if !self.held.iter().any(|held| held.kind == FrameKind::Input) {
            return None;
        }
        let due = self.input_sent_at? + self.input_spacing();
        (due > now).then(|| due - now)
    }

    /// How long the driver may block with nothing to do: until a paced input is due, and at most
    /// [`MAINTENANCE_INTERVAL`].
    fn wake_within(&self, now: Instant) -> Duration {
        self.paced_for(now)
            .map_or(MAINTENANCE_INTERVAL, |due| due.min(MAINTENANCE_INTERVAL))
    }

    fn stalled(&self, now: Instant) -> bool {
        self.unacknowledged_since
            .is_some_and(|since| now.duration_since(since) >= STALL_AFTER)
    }

    /// The frames to queue now, in order: what was held and may go, then `frames` as far as they
    /// may; the rest is held.
    fn admit(&mut self, frames: Vec<PulseFrame>, now: Instant) -> Vec<PulseFrame> {
        let stalled = self.stalled(now);
        let was_holding = !self.held.is_empty();
        let out = self.admit_inner(frames, stalled, now);
        let waits_for_uplink = |held: &PulseFrame| stalled || DiscreteEventPacer::paces(held.kind);
        let input_held = self.held.iter().any(|held| held.kind == FrameKind::Input)
            && self.held.iter().any(waits_for_uplink);
        match (self.input_held_since, input_held) {
            (None, true) => self.input_held_since = Some(now),
            (Some(since), false) => {
                let held = now.duration_since(since);
                self.input_holds += 1;
                self.input_held_for += held;
                self.longest_input_hold = self.longest_input_hold.max(held);
                self.input_held_since = None;
            }
            _ => {}
        }
        match (was_holding, self.held.is_empty()) {
            (false, false) if stalled => {
                bevy::log::debug!("pulse: uplink unconfirmed, holding frames")
            }
            (false, false)
                if self
                    .held
                    .iter()
                    .any(|held| DiscreteEventPacer::paces(held.kind)) =>
            {
                bevy::log::debug!("pulse: reliable messages unacknowledged, holding frames")
            }
            (false, false) => bevy::log::debug!("pulse: holding an input for its spacing"),
            (true, true) => bevy::log::debug!("pulse: held frames sent"),
            _ => {}
        }
        out
    }

    fn admit_inner(
        &mut self,
        frames: Vec<PulseFrame>,
        stalled: bool,
        now: Instant,
    ) -> Vec<PulseFrame> {
        let spacing = self.input_spacing();
        // Withdraw first, so nothing waits a pass behind a teleport for the realm left.
        if frames
            .iter()
            .any(|frame| frame.kind == FrameKind::WithdrawTeleport)
        {
            self.held.retain(|held| held.kind != FrameKind::Teleport);
        }
        let mut out = Vec::new();
        // A held input and a newer one right behind it are one: only the newer goes.
        let queue = std::mem::take(&mut self.held).into_iter().chain(frames);
        for frame in collapse_inputs(queue.collect()) {
            if frame.kind == FrameKind::WithdrawTeleport {
                // The realm a held teleport names was left.
                self.held.retain(|held| held.kind != FrameKind::Teleport);
                continue;
            }
            let discrete = DiscreteEventPacer::paces(frame.kind);
            let hold = if discrete {
                stalled || self.unacknowledged_reliable >= MAX_UNACKNOWLEDGED_RELIABLE
            } else if frame.kind == FrameKind::Input {
                stalled
                    || self
                        .held
                        .iter()
                        .any(|held| DiscreteEventPacer::paces(held.kind))
                    || self.input_sent_at.is_some_and(|at| now < at + spacing)
            } else {
                false
            };
            if hold {
                self.held.retain(|held| held.kind != frame.kind);
                self.held.push(frame);
            } else {
                if frame.reliability == PulseReliability::Reliable {
                    self.unacknowledged_reliable += 1;
                }
                if frame.kind == FrameKind::Input {
                    self.input_sent_at = Some(now);
                }
                if frame.kind == FrameKind::Teleport {
                    // A held input is older than the teleport, which supersedes its pose.
                    self.held.retain(|held| held.kind != FrameKind::Input);
                }
                out.push(frame);
            }
        }
        out
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
    let mut backlog = Backlog::with_caps(MAX_HELD_RELIABLE, MAX_HELD_BYTES);
    let mut gate = UplinkGate::default();

    while !stop.load(Ordering::Relaxed) {
        // Whatever the protocol layer has made room for since the last pass.
        backlog.flush(&channels.inbound);
        // ENet would give up on its own only after up to 30 s. The loop wakes at least every
        // `MAINTENANCE_INTERVAL`, so this is checked about as often.
        if !connected && connecting_since.elapsed() >= CONNECT_TIMEOUT {
            return abandon(&mut host, peer, channels, "connect timed out".to_owned());
        }

        // Block until there's work: an outbound frame to send, an inbound datagram to read, or the
        // maintenance deadline so ENet's timers keep ticking.
        let outbound = tokio::select! {
            frame = channels.outbound.recv() => Some(frame),
            _ = host.socket().0.readable() => None,
            _ = tokio::time::sleep(gate.wake_within(Instant::now())) => None,
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
        // its newest before it reaches the peer. While the uplink is unconfirmed the gate holds the
        // latest of each kind instead, and lets it out once it is.
        let now = Instant::now();
        pass(&mut host, peer, connected, &mut gate, batch, now);
        if let Ok(mut report) = channels.uplink.lock() {
            *report = Some(gate.snapshot(now));
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
                        return abandon(
                            &mut host,
                            peer,
                            channels,
                            format!(
                                "inbound backlog over {MAX_HELD_RELIABLE} messages or \
                                 {MAX_HELD_BYTES} bytes"
                            ),
                        );
                    }
                }
                // This host keeps receive_policy=None; preserve exhaustive handling of vendor events.
                Ok(Some(enet::Event::ReceiveLimitExceeded { .. })) => {}
                Ok(None) => break,
                Err(err) => {
                    return abandon(
                        &mut host,
                        peer,
                        channels,
                        format!("enet service error: {err}"),
                    )
                }
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

/// One pass of the gate over the peer at `now`: what the peer says, a probe when one is due, and
/// the frames of `batch` that may go, queued. Returns the frames queued.
fn pass<S: enet::Socket>(
    host: &mut enet::Host<S>,
    peer: enet::PeerID,
    connected: bool,
    gate: &mut UplinkGate,
    batch: Vec<PulseFrame>,
    now: Instant,
) -> Vec<PulseFrame> {
    let acks = if connected {
        PeerAcks::of(host.peer(peer))
    } else {
        PeerAcks::default()
    };
    gate.observe(now, acks);
    if connected && gate.wants_probe(now) {
        host.peer_mut(peer).ping();
    }
    let admitted = gate.admit(collapse_inputs(batch), now);
    for frame in &admitted {
        queue_frame(host, peer, frame);
    }
    admitted
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
fn queue_frame<S: enet::Socket>(host: &mut enet::Host<S>, peer: enet::PeerID, frame: &PulseFrame) {
    let (channel_id, packet) = packet_for_frame(frame);
    if let Err(err) = host.peer_mut(peer).send(channel_id, &packet) {
        bevy::log::warn!("pulse: peer send failed: {err}");
    }
}

/// Report a failure the driver detects itself, before or after the connection comes up: setup,
/// DNS, connect, the inbound backlog over a cap, or servicing ENet. Always retryable — the
/// protocol layer reconnects.
fn fail(channels: &mut PulseDriverChannels, message: String) {
    bevy::log::warn!("pulse: {message}");
    let _ = channels.status.try_send(PulseStatus::Failed(message));
}

/// [`fail`], for a connection `host.connect` has started, ending the peer with `NONE` (0): a
/// failure, where leaving on purpose sends `GRACEFUL`. The server hears it only once the peer is
/// connected, on the backlog-cap and service-error paths, and then lets the peer go at once rather
/// than at its own timeout. Until VERIFY_CONNECT the peer has no id from the server, and ENet drops
/// any command but CONNECT that carries the unassigned one (`src/c/protocol.rs` in the vendored
/// port, which dcl-one's server runs too). So on a connect timeout the notice never arrives, and
/// the server keeps its half-open peer until its own timeout.
fn abandon<S: enet::Socket>(
    host: &mut enet::Host<S>,
    peer: enet::PeerID,
    channels: &mut PulseDriverChannels,
    message: String,
) {
    // Flushes the notice out before it resets the peer, so the host can go right after.
    host.peer_mut(peer).disconnect_now(0);
    fail(channels, message);
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
#[cfg(test)]
mod link_sim_tests;
