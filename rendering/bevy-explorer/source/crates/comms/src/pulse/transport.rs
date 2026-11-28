//! Transport boundary for Pulse — the only seam that differs per platform.
//!
//! The driver knows nothing about protobuf, identity, the decoder, or the parcel grid. Its whole
//! contract is: connect to `host:port`, then move bytes between two channels — outbound
//! [`PulseFrame`]s (with a reliability tag the driver maps to ENet channel flags / WebTransport
//! stream-vs-datagram, and a [`FrameKind`] it reads only to collapse queued inputs) and inbound raw
//! `ServerMessage` bytes — and report [`PulseStatus`]. Because the seam is bytes, everything above
//! it (handshake, decode, state, glue) is shared and written once, regardless of whether the driver
//! is the native ENet thread or the wasm WebTransport task.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::mpsc;

/// Channel/reliability selector, mirroring the server's `PacketMode`. The driver maps this to its
/// transport's primitive (ENet reliable/unreliable-sequenced/unsequenced; WebTransport
/// stream/datagram).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PulseReliability {
    /// Reliable ordered — handshake, resync, teleport, emotes (server channel 0).
    Reliable,
    /// Unreliable sequenced — high-frequency state input (server channel 1).
    UnreliableSequenced,
    /// Unreliable unordered.
    UnreliableUnsequenced,
}

/// What a frame carries, for the decisions made below the protocol layer: collapsing (inputs)
/// and pacing (teleports, emotes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    /// A `PlayerStateInput`. A newer one supersedes the input queued right before it; an input
    /// with another frame between them is kept.
    Input,
    /// A `TeleportRequest`.
    Teleport,
    /// An `EmoteStart` or `EmoteStop`.
    Emote,
    /// Everything else: handshakes, resync, profile announcements, scene-listener updates.
    Control,
    /// Never sent: a driver holding a teleport back drops it, as the pacer drops its waiting one,
    /// because the realm it names was left.
    WithdrawTeleport,
}

/// One outbound unit of work: an already-encoded `ClientMessage` plus how to deliver it.
#[derive(Debug)]
pub struct PulseFrame {
    pub bytes: Vec<u8>,
    pub reliability: PulseReliability,
    pub kind: FrameKind,
}

/// In each run of consecutive `Input` frames only the newest survives, since each carries the
/// player's whole state; every other frame keeps its place.
pub fn collapse_inputs(frames: Vec<PulseFrame>) -> Vec<PulseFrame> {
    let mut collapsed: Vec<PulseFrame> = Vec::with_capacity(frames.len());
    for frame in frames {
        // An input replaces the input right before it, so each run shrinks to its newest.
        if frame.kind == FrameKind::Input
            && collapsed
                .last()
                .is_some_and(|last| last.kind == FrameKind::Input)
        {
            collapsed.pop();
        }
        collapsed.push(frame);
    }
    collapsed
}

/// Connection lifecycle, surfaced from the driver to the protocol layer.
#[derive(Debug, Clone)]
pub enum PulseStatus {
    Connecting,
    Connected,
    /// Connected-then-dropped, carrying the server's reason (see [`PulseDisconnect`]).
    Disconnected(PulseDisconnect),
    /// A failure the driver reports itself, before or after the connection came up: DNS, a connect
    /// timeout, an inbound backlog over a cap, and the like. Always retryable.
    Failed(String),
}

/// Server disconnect reason. This is an out-of-band ABI shared with the server
/// (`Pulse.Transport.DisconnectReason`), *not* a wire message: it arrives as the ENet disconnect
/// event's `data` code. The driver maps that `u32` here so the protocol layer can decide whether a
/// reconnect could plausibly help — several reasons are terminal by the server's design (ban,
/// eviction, flagged misbehaviour) and must not trigger a reconnect loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PulseDisconnect {
    /// No reason supplied (e.g. a plain network timeout where the remote set no code).
    None,
    /// Clean shutdown / server stopping.
    Graceful,
    /// Our `PENDING_AUTH` deadline was exceeded (handshake too slow).
    AuthTimeout,
    /// Handshake validation failed.
    AuthFailed,
    /// Evicted by a newer connection with the same wallet — retrying fights that session.
    DuplicateSession,
    /// Banned platform-wide.
    Banned,
    /// Server at capacity.
    ServerFull,
    /// Per-source-IP pre-auth connection cap exceeded.
    PreAuthIpLimit,
    /// Global pre-auth budget exhausted.
    PreAuthBudget,
    /// Sent `PlayerStateInput` faster than the server's cap (misbehaving client).
    InputRateExceeded,
    /// Exceeded the discrete-event (emote/teleport) cap (misbehaving client).
    DiscreteEventRateExceeded,
    /// `PlayerStateInput` carried an invalid field.
    InvalidInputField,
    /// `EmoteStart` carried an invalid field.
    InvalidEmoteField,
    /// `TeleportRequest` carried an invalid field (oversized/empty realm, bad parcel).
    InvalidTeleportField,
    /// A handshake with the same (wallet, timestamp) was replayed inside the anti-replay window.
    HandshakeReplayRejected,
    /// A handshake carried an invalid field: a `HandshakeRequest`'s malformed `PlayerInitialState`,
    /// or a `SceneListenerHandshakeRequest`'s AoI (malformed, or over the server's parcel budget).
    InvalidHandshakeField,
    /// Sustained corrupt/oversized packets (buggy client, fuzzer, or amplification probe).
    PacketCorrupted,
    /// Per-source-IP concurrent-connection cap exceeded. Unlike [`Self::PreAuthIpLimit`],
    /// authenticated connections count against it; it frees as the IP's other connections close.
    IpConnectionLimitExceeded,
    /// Per-source-IP concurrent scene-listener cap exceeded. It frees only when another listener
    /// from the same IP disconnects, which may take minutes or never.
    SceneListenerIpLimitExceeded,
    /// `SceneListenerUpdate` carried an invalid AoI (no rects, inverted/out-of-range rect, or over
    /// the server's parcel budget). The AoI in force when it arrived is untouched.
    InvalidSceneListenerField,
    /// A code this client build doesn't recognise. Treated as terminal to be safe.
    Unknown(u32),
}

impl PulseDisconnect {
    /// The code we close with when leaving on purpose — the server's `DisconnectReason.GRACEFUL`,
    /// as the reference client sends.
    pub const GRACEFUL_CODE: u32 = 1;

    /// Map an ENet disconnect `data` code to a reason. Mirrors the server's `DisconnectReason` enum
    /// 1:1; unrecognised codes (a newer server) fall through to [`PulseDisconnect::Unknown`].
    pub fn from_code(code: u32) -> Self {
        match code {
            0 => Self::None,
            1 => Self::Graceful,
            2 => Self::AuthTimeout,
            3 => Self::AuthFailed,
            4 => Self::DuplicateSession,
            5 => Self::Banned,
            6 => Self::ServerFull,
            7 => Self::PreAuthIpLimit,
            8 => Self::PreAuthBudget,
            9 => Self::InputRateExceeded,
            10 => Self::DiscreteEventRateExceeded,
            11 => Self::InvalidInputField,
            12 => Self::InvalidEmoteField,
            13 => Self::InvalidTeleportField,
            14 => Self::HandshakeReplayRejected,
            15 => Self::InvalidHandshakeField,
            16 => Self::PacketCorrupted,
            17 => Self::IpConnectionLimitExceeded,
            18 => Self::SceneListenerIpLimitExceeded,
            19 => Self::InvalidSceneListenerField,
            other => Self::Unknown(other),
        }
    }
}

/// Where to connect. Identity / `server_id` / realm live in the protocol layer (`PulseConfig`),
/// not here — the driver only needs an address (and, later for WebTransport, TLS details).
#[derive(Debug, Clone)]
pub struct PulseTransportConfig {
    pub host: String,
    pub port: u16,
    /// SHA-256 of the server's certificate, for the wasm WebTransport driver only. A browser refuses a
    /// self-signed dev cert via the CA path, so it must be pinned by hash (the reference client's
    /// `serverCertHash`); `None` uses default CA trust, which is what a production cert needs. Ignored
    /// by the native ENet driver.
    pub cert_hash: Option<Vec<u8>>,
}

/// What the native driver's uplink gate holds back, as of its last pass, for the debug panel and
/// the Pulse state reply.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UplinkSnapshot {
    /// Reliable traffic has gone unacknowledged long enough that inputs are held.
    pub stalled: bool,
    /// Reliable messages unacknowledged; teleports and emotes wait at 8.
    pub unacknowledged_reliable: usize,
    pub held_teleport: bool,
    pub held_emote: bool,
    /// The latest input waits for the uplink (only one ever waits). An input waiting only for its
    /// spacing is not held.
    pub held_input: bool,
    /// Pings sent to probe the uplink.
    pub probes: u64,
    /// ENet's smoothed round trip.
    pub round_trip: Duration,
    /// The least time between two inputs; zero below the round trip that paces them.
    pub input_spacing: Duration,
    /// Times an input was held since the connection came up, the one going on included.
    pub input_holds: u64,
    /// Their total length.
    pub input_held_for: Duration,
    /// The longest of them.
    pub longest_input_hold: Duration,
}

/// The driver's latest [`UplinkSnapshot`]; `None` from a driver without a gate (WebTransport).
pub type UplinkReport = Arc<Mutex<Option<UplinkSnapshot>>>;

/// Protocol-layer end of the boundary (held by the Bevy plugin). `Send + Sync`, so it lives in a
/// resource.
pub struct PulseLink {
    outbound: mpsc::Sender<PulseFrame>,
    pub(super) traffic: crate::traffic::PayloadTraffic,
    pub(super) uplink: UplinkReport,
    pub inbound: mpsc::Receiver<Vec<u8>>,
    pub status: mpsc::Receiver<PulseStatus>,
}

impl PulseLink {
    pub(crate) fn send(&mut self, frame: PulseFrame) -> bool {
        let size = frame.bytes.len();
        if self.outbound.try_send(frame).is_err() {
            return false;
        }
        self.traffic.record_sent(size);
        true
    }

    pub(crate) fn receive(&mut self) -> Option<Vec<u8>> {
        let bytes = self.inbound.try_recv().ok()?;
        self.traffic.record_received(bytes.len());
        Some(bytes)
    }

    /// Tell the driver to drop a teleport it holds back. In band, so a teleport sent after this is
    /// never the one dropped.
    pub(crate) fn withdraw_teleport(&mut self) {
        // A full queue drops the signal, and a held teleport then goes once the uplink is
        // confirmed: at worst a brief misplacement in the realm left, which is accepted. The queue
        // is full only while the driver is not draining it.
        let _ = self.outbound.try_send(PulseFrame {
            bytes: Vec::new(),
            reliability: PulseReliability::Reliable,
            kind: FrameKind::WithdrawTeleport,
        });
    }

    pub(crate) fn uplink(&self) -> Option<UplinkSnapshot> {
        self.uplink.lock().ok().and_then(|report| *report)
    }
}

/// Driver end of the boundary (moved into the native thread / wasm task).
pub struct PulseDriverChannels {
    pub outbound: mpsc::Receiver<PulseFrame>,
    pub inbound: mpsc::Sender<Vec<u8>>,
    pub status: mpsc::Sender<PulseStatus>,
    pub uplink: UplinkReport,
}

/// Build a matched [`PulseLink`] / [`PulseDriverChannels`] pair.
pub fn pulse_channels(capacity: usize) -> (PulseLink, PulseDriverChannels) {
    let (outbound_tx, outbound_rx) = mpsc::channel(capacity);
    let (inbound_tx, inbound_rx) = mpsc::channel(capacity);
    let (status_tx, status_rx) = mpsc::channel(16);
    let uplink = UplinkReport::default();

    (
        PulseLink {
            outbound: outbound_tx,
            traffic: Default::default(),
            uplink: uplink.clone(),
            inbound: inbound_rx,
            status: status_rx,
        },
        PulseDriverChannels {
            outbound: outbound_rx,
            inbound: inbound_tx,
            status: status_tx,
            uplink,
        },
    )
}

/// Owns the running driver. Dropping (or `stop()`) tears it down; on native it also joins the
/// thread.
pub struct PulseDriverHandle {
    stop: Arc<AtomicBool>,
    #[cfg(not(target_arch = "wasm32"))]
    join: Option<std::thread::JoinHandle<()>>,
}

impl PulseDriverHandle {
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for PulseDriverHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// Spawn the platform driver. Native runs a dedicated thread (ENet is a synchronous single-thread
/// poll loop); wasm is a no-op today and becomes the WebTransport task later — same boundary.
pub fn spawn_pulse_driver(
    config: PulseTransportConfig,
    channels: PulseDriverChannels,
) -> PulseDriverHandle {
    let stop = Arc::new(AtomicBool::new(false));

    #[cfg(not(target_arch = "wasm32"))]
    {
        let join = super::native::spawn(config, channels, stop.clone());
        PulseDriverHandle {
            stop,
            join: Some(join),
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        super::wasm::spawn(config, channels, stop.clone());
        PulseDriverHandle { stop }
    }
}

#[cfg(test)]
mod tests;
