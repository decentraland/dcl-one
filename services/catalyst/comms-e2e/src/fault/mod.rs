//! The fault link: a relay a client dials instead of the stack, which a scenario can disturb.
//!
//! Faults act on whole packets (UDP) or on the byte stream (TCP) and never read the content. A
//! stalled link drops what arrives, as a dead link does; it does not queue it for later.

mod tcp;
mod udp;

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use anyhow::Result;
use parking_lot::Mutex;
use serde::Serialize;
use tokio::sync::Notify;
use tokio::task::AbortHandle;

use crate::engine::{Ctx, Seeded, Timeline};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Direction {
    /// Client to stack.
    Uplink,
    /// Stack to client.
    Downlink,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum Fault {
    Pass,
    /// Nothing gets through. On UDP the packets are lost; on TCP the bytes wait in the sockets.
    Stall,
    /// UDP: each packet is lost with this probability. TCP: as `Pass`.
    Drop { share: f32 },
    /// Everything arrives this much later, in order.
    Delay { by: Duration },
    /// UDP: each packet arrives after a random wait inside the window, so packets overtake each
    /// other. TCP: as `Pass`.
    Reorder { window: Duration },
    /// UDP: each packet arrives twice. TCP: as `Pass`.
    Duplicate,
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
pub struct Counters {
    pub up_packets: u64,
    pub down_packets: u64,
    pub up_bytes: u64,
    pub down_bytes: u64,
    /// Packets or chunks a fault kept from arriving.
    pub dropped: u64,
    /// Client flows (UDP) or connections (TCP) seen so far.
    pub flows: u64,
}

pub(crate) struct Faults {
    pub up: Fault,
    pub down: Fault,
}

pub(crate) struct Shared {
    pub name: String,
    pub timeline: Timeline,
    pub faults: Mutex<Faults>,
    pub seeded: Mutex<Seeded>,
    /// Bumped by `half_open` and `cut`: a flow older than this is dead (half-open) or closed (cut).
    pub generation: AtomicU64,
    /// Whether the last bump closes old flows (`cut`) or only silences them (`half_open`).
    pub closing: AtomicBool,
    /// Wakes TCP pumps waiting out a stall, and tells them to look at the generation.
    pub changed: Notify,
    pub up_packets: AtomicU64,
    pub down_packets: AtomicU64,
    pub up_bytes: AtomicU64,
    pub down_bytes: AtomicU64,
    pub dropped: AtomicU64,
    pub flows: AtomicU64,
}

impl Shared {
    pub fn fault(&self, direction: Direction) -> Fault {
        let faults = self.faults.lock();
        match direction {
            Direction::Uplink => faults.up,
            _ => faults.down,
        }
    }

    pub fn count(&self, direction: Direction, bytes: usize) {
        let (packets, total) = match direction {
            Direction::Uplink => (&self.up_packets, &self.up_bytes),
            _ => (&self.down_packets, &self.down_bytes),
        };
        packets.fetch_add(1, Ordering::Relaxed);
        total.fetch_add(bytes as u64, Ordering::Relaxed);
    }

    pub fn record(&self, kind: &str, detail: impl Serialize) {
        self.timeline.record(&self.name, kind, detail);
    }

    /// A value in `0.0..1.0` from the link's own seeded generator.
    pub fn chance(&self) -> f32 {
        (self.seeded.lock().below(1_000_000) as f32) / 1_000_000.0
    }

    pub fn gap(&self, window: Duration) -> Duration {
        self.seeded.lock().gap(window)
    }
}

/// A relay between a client and one address of the stack. Dropping it stops relaying and frees
/// the listen port.
pub struct FaultLink {
    shared: Arc<Shared>,
    listen: SocketAddr,
    upstream: SocketAddr,
    tasks: Vec<AbortHandle>,
}

impl FaultLink {
    /// A UDP relay (ENet, WebTransport, LiveKit media). Port 0 picks a free port.
    pub async fn udp(ctx: &Ctx, name: &str, listen: SocketAddr, upstream: SocketAddr) -> Result<FaultLink> {
        let shared = Self::shared(ctx, name);
        let (listen, tasks) = udp::start(shared.clone(), listen, upstream).await?;
        shared.record("relaying", serde_json::json!({ "udp": listen.to_string(), "to": upstream.to_string() }));
        Ok(FaultLink {
            shared,
            listen,
            upstream,
            tasks,
        })
    }

    /// A TCP relay (HTTP, WebSocket, LiveKit signalling). Port 0 picks a free port.
    pub async fn tcp(ctx: &Ctx, name: &str, listen: SocketAddr, upstream: SocketAddr) -> Result<FaultLink> {
        let shared = Self::shared(ctx, name);
        let (listen, tasks) = tcp::start(shared.clone(), listen, upstream).await?;
        shared.record("relaying", serde_json::json!({ "tcp": listen.to_string(), "to": upstream.to_string() }));
        Ok(FaultLink {
            shared,
            listen,
            upstream,
            tasks,
        })
    }

    fn shared(ctx: &Ctx, name: &str) -> Arc<Shared> {
        Arc::new(Shared {
            name: name.to_owned(),
            timeline: ctx.timeline().clone(),
            faults: Mutex::new(Faults {
                up: Fault::Pass,
                down: Fault::Pass,
            }),
            seeded: Mutex::new(Seeded::new(ctx.seed() ^ 0x6c69_6e6b)),
            generation: AtomicU64::new(0),
            closing: AtomicBool::new(false),
            changed: Notify::new(),
            up_packets: AtomicU64::new(0),
            down_packets: AtomicU64::new(0),
            up_bytes: AtomicU64::new(0),
            down_bytes: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            flows: AtomicU64::new(0),
        })
    }

    /// The address a client dials.
    pub fn address(&self) -> SocketAddr {
        self.listen
    }

    pub fn upstream(&self) -> SocketAddr {
        self.upstream
    }

    pub fn set(&self, direction: Direction, fault: Fault) {
        {
            let mut faults = self.shared.faults.lock();
            if matches!(direction, Direction::Uplink | Direction::Both) {
                faults.up = fault;
            }
            if matches!(direction, Direction::Downlink | Direction::Both) {
                faults.down = fault;
            }
        }
        self.shared.record("fault", serde_json::json!({ "direction": direction, "fault": fault }));
        self.shared.changed.notify_waiters();
    }

    /// Everything passes again. Flows that were half-opened stay dead: a stale socket does not
    /// come back.
    pub fn restore(&self) {
        self.set(Direction::Both, Fault::Pass);
    }

    /// The flows that exist now go silent for good, with nothing closed: neither side is told.
    /// A new flow from the same client passes. This is a socket left behind by sleep or by a
    /// change of network.
    pub fn half_open(&self) {
        self.shared.closing.store(false, Ordering::SeqCst);
        self.shared.generation.fetch_add(1, Ordering::SeqCst);
        self.shared.record("half-open", ());
        self.shared.changed.notify_waiters();
    }

    /// The flows that exist now are closed: on TCP both sides see the connection end, on UDP the
    /// next packet of a client starts a new flow from a new port, as after an address change.
    pub fn cut(&self) {
        self.shared.closing.store(true, Ordering::SeqCst);
        self.shared.generation.fetch_add(1, Ordering::SeqCst);
        self.shared.record("cut", ());
        self.shared.changed.notify_waiters();
    }

    pub fn counters(&self) -> Counters {
        Counters {
            up_packets: self.shared.up_packets.load(Ordering::Relaxed),
            down_packets: self.shared.down_packets.load(Ordering::Relaxed),
            up_bytes: self.shared.up_bytes.load(Ordering::Relaxed),
            down_bytes: self.shared.down_bytes.load(Ordering::Relaxed),
            dropped: self.shared.dropped.load(Ordering::Relaxed),
            flows: self.shared.flows.load(Ordering::Relaxed),
        }
    }
}

impl Drop for FaultLink {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
        self.shared.closing.store(true, Ordering::SeqCst);
        self.shared.generation.fetch_add(1, Ordering::SeqCst);
        self.shared.changed.notify_waiters();
    }
}
