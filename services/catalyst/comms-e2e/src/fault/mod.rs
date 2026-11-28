//! The fault link: a relay a client dials instead of the stack, which a scenario can disturb.
//!
//! Faults act on whole packets (UDP) or on the byte stream (TCP) and never read the content. A
//! stalled link drops what arrives, as a dead link does; a queue keeps it and lets it go together
//! when it ends, as a link that buffers does. A UDP link notes every packet it receives, as it
//! arrives, before its fault acts ([`FaultLink::history`]): one it drops before any fault (a
//! closed port, a half-open or cut flow, an upstream socket it cannot bind) is noted as lost.

mod history;
mod tcp;
mod udp;

pub use history::{Cadence, Fate, Gap, Seen, Silences, Window, cadence, windows};

use std::any::Any;
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use parking_lot::Mutex;
use serde::Serialize;
use sha2::{Digest, Sha256};
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
    Drop {
        share: f32,
    },
    /// Everything arrives this much later, in order. A delay longer than [`HELD_FOR_GOOD`] holds
    /// what it delays for that long, which is for good.
    Delay {
        by: Duration,
    },
    /// UDP: each packet arrives after a random wait inside the window, so packets overtake each
    /// other. TCP: as `Pass`.
    Reorder {
        window: Duration,
    },
    /// UDP: each packet arrives twice. TCP: as `Pass`.
    Duplicate,
    /// A path: each packet is lost with probability `loss`, and the rest arrive `delay` later plus
    /// a random wait inside `jitter`, drawn from the direction's seeded generator. None overtakes
    /// the one before it: a packet whose wait ends first waits for it. TCP: everything arrives
    /// `delay` later; nothing is lost and there is no jitter.
    Link {
        delay: Duration,
        jitter: Duration,
        loss: f32,
    },
    /// UDP: nothing gets through, and what arrives is kept, up to `limit` packets (the rest are
    /// lost). When the direction's fault changes, what was kept goes on at once, in the order it
    /// came, through the fault set then; what a flow half-opened or cut meanwhile kept is lost
    /// with it. TCP: as `Stall`, where the bytes wait in the sockets.
    Queue {
        limit: usize,
    },
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
    /// UDP: packets sent on after a wait (`Delay`, `Reorder`, `Link`).
    pub delayed: u64,
    /// UDP: packets a `Queue` kept.
    pub queued: u64,
    /// UDP: kept packets sent on when their queue ended.
    pub released: u64,
    /// UDP: packets that arrived after the history of their direction was full.
    pub unrecorded: u64,
}

/// The longest any fault holds a packet or a byte: a longer delay, up to `Duration::MAX`, holds
/// for this long.
pub const HELD_FOR_GOOD: Duration = Duration::from_secs(60 * 60 * 24 * 365 * 30);

/// When something delayed by `by` from `now` is due, never later than [`HELD_FOR_GOOD`] from
/// `now`; where even that is past the end of the clock, at once.
pub(crate) fn due<I: Later>(now: I, by: Duration) -> I {
    now.later_by(by.min(HELD_FOR_GOOD)).unwrap_or(now)
}

/// A clock a relay reads: the standard one (UDP) or tokio's (TCP).
pub(crate) trait Later: Copy {
    fn later_by(self, by: Duration) -> Option<Self>;
}

impl Later for Instant {
    fn later_by(self, by: Duration) -> Option<Self> {
        self.checked_add(by)
    }
}

impl Later for tokio::time::Instant {
    fn later_by(self, by: Duration) -> Option<Self> {
        self.checked_add(by)
    }
}

pub(crate) struct Faults {
    pub up: Fault,
    pub down: Fault,
    /// What a `Queue` keeps in each direction.
    pub up_queue: VecDeque<udp::Queued>,
    pub down_queue: VecDeque<udp::Queued>,
}

/// How one generation of flows was ended. A flow belongs to the generation that was current when
/// it began, and what happens to it is fixed by the step that ended that generation: a later step
/// never changes it.
pub(crate) mod ending {
    /// Silent for good, nothing closed.
    pub const HALF_OPEN: u8 = 0;
    /// Closed on both sides.
    pub const CUT: u8 = 1;
    /// Closed towards the client, left open and silent towards the stack.
    pub const ORPHAN: u8 = 2;
}

pub(crate) struct Shared {
    pub name: String,
    pub timeline: Timeline,
    pub faults: Mutex<Faults>,
    /// The generators of the uplink and the downlink: each direction draws its own sequence, so
    /// what one direction drew does not move the other.
    pub up_seeded: Mutex<Seeded>,
    pub down_seeded: Mutex<Seeded>,
    /// The generation new flows begin in: the number of endings so far.
    pub generation: AtomicU64,
    /// How each generation ended, indexed by generation: `half_open`, `cut` and `orphan` each push
    /// one entry (and so start the next generation). A flow of generation `g` is ended once
    /// `endings[g]` exists, and in the way it says.
    pub endings: Mutex<Vec<u8>>,
    /// The link has been dropped: everything still running lets go of what it holds.
    pub closed: AtomicBool,
    /// UDP: the listen port is closed, so a packet to it is answered "port unreachable".
    pub port_closed: AtomicBool,
    /// Wakes whatever waits out a stall, and tells it to look at the generation.
    pub changed: Notify,
    /// Stack-side halves of orphaned TCP connections, kept open until the link goes.
    pub parked: Mutex<Vec<Box<dyn Any + Send>>>,
    pub up_packets: AtomicU64,
    pub down_packets: AtomicU64,
    pub up_bytes: AtomicU64,
    pub down_bytes: AtomicU64,
    pub dropped: AtomicU64,
    pub flows: AtomicU64,
    pub delayed: AtomicU64,
    pub queued: AtomicU64,
    pub released: AtomicU64,
    /// The link's clock: [`Seen::at`] counts from here.
    pub started: Instant,
    pub history: Mutex<history::History>,
    /// UDP: the lines that send each direction's packets in order, once the relay has started.
    pub lines: Mutex<Option<udp::Lines>>,
    /// UDP: packets waiting in each direction's line. While one waits, a packet that could go at
    /// once joins the line instead, so it does not overtake.
    pub up_waiting: AtomicUsize,
    pub down_waiting: AtomicUsize,
}

impl Shared {
    pub fn fault(&self, direction: Direction) -> Fault {
        let faults = self.faults.lock();
        match direction {
            Direction::Uplink => faults.up,
            _ => faults.down,
        }
    }

    /// Ends the current generation of flows in the way `how` says and starts the next one.
    pub fn end_generation(&self, how: u8) {
        let mut endings = self.endings.lock();
        endings.push(how);
        self.generation
            .store(endings.len() as u64, Ordering::SeqCst);
    }

    /// How the flows of `generation` were ended, or `None` while that generation is current.
    pub fn fate(&self, generation: u64) -> Option<u8> {
        let index = usize::try_from(generation).ok()?;
        self.endings.lock().get(index).copied()
    }

    /// Sleeps for `by`, or until the link is dropped. `false`: the link was dropped. Registers for
    /// a wake-up before it looks at the state, so a drop that lands in between is not missed.
    pub async fn sleep_unless_closed(&self, by: Duration) -> bool {
        let sleep = tokio::time::sleep(by);
        tokio::pin!(sleep);
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self.closed.load(Ordering::SeqCst) {
                return false;
            }
            tokio::select! {
                _ = &mut sleep => return true,
                _ = &mut changed => {}
            }
        }
    }

    /// Sleeps until `deadline`, or until the link is dropped. `false`: the link was dropped.
    pub async fn sleep_until_unless_closed(&self, deadline: Instant) -> bool {
        self.sleep_unless_closed(deadline.saturating_duration_since(Instant::now()))
            .await
    }

    /// Notes a packet of `direction` that arrived at `at` on the link's clock.
    pub fn note(&self, direction: Direction, at: Duration, bytes: usize, fate: Fate) {
        self.history
            .lock()
            .push(direction == Direction::Uplink, Seen { at, bytes, fate });
    }

    /// A packet of `direction` that arrived now and that the relay drops before any fault acts:
    /// counted as dropped and noted as lost.
    pub fn lose(&self, direction: Direction, bytes: usize) {
        self.dropped.fetch_add(1, Ordering::Relaxed);
        self.note(direction, self.started.elapsed(), bytes, Fate::Lost);
    }

    /// Packets waiting in the line of `direction`.
    pub fn waiting(&self, direction: Direction) -> &AtomicUsize {
        match direction {
            Direction::Uplink => &self.up_waiting,
            _ => &self.down_waiting,
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

    fn seeded(&self, direction: Direction) -> &Mutex<Seeded> {
        match direction {
            Direction::Uplink => &self.up_seeded,
            _ => &self.down_seeded,
        }
    }

    /// A value in `0.0..1.0` from the generator of `direction` on this link.
    #[allow(
        clippy::cast_precision_loss,
        reason = "a draw below 1_000_000 is exact in f32"
    )]
    pub fn chance(&self, direction: Direction) -> f32 {
        (self.seeded(direction).lock().below(1_000_000) as f32) / 1_000_000.0
    }

    /// A wait inside `window` from the generator of `direction` on this link.
    pub fn gap(&self, direction: Direction, window: Duration) -> Duration {
        self.seeded(direction).lock().gap(window)
    }
}

/// The seed of one direction of the link `name`, from the run's seed: a replay with the same seed
/// draws the same sequence in each direction of each link, and two links of one run, or the two
/// directions of one link, draw different ones.
fn direction_seed(seed: u64, name: &str, direction: Direction) -> u64 {
    let digest = Sha256::new()
        .chain_update(b"comms-e2e fault link\0")
        .chain_update(seed.to_be_bytes())
        .chain_update(name.as_bytes())
        .chain_update([0])
        .chain_update(match direction {
            Direction::Uplink => b"uplink".as_slice(),
            _ => b"downlink".as_slice(),
        })
        .finalize();
    u64::from_be_bytes(
        digest[..8]
            .try_into()
            .expect("a digest is longer than 8 bytes"),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Udp,
    Tcp,
}

/// A relay between a client and one address of the stack. Dropping it stops relaying and frees
/// the listen port.
pub struct FaultLink {
    shared: Arc<Shared>,
    kind: Kind,
    listen: SocketAddr,
    upstream: SocketAddr,
    tasks: Vec<AbortHandle>,
}

impl FaultLink {
    /// A UDP relay (ENet, WebTransport, LiveKit media). Port 0 picks a free port.
    pub async fn udp(
        ctx: &Ctx,
        name: &str,
        listen: SocketAddr,
        upstream: SocketAddr,
    ) -> Result<FaultLink> {
        let shared = Self::shared(ctx, name);
        let (listen, tasks) = udp::start(shared.clone(), listen, upstream).await?;
        shared.record(
            "relaying",
            serde_json::json!({ "udp": listen.to_string(), "to": upstream.to_string() }),
        );
        Ok(FaultLink {
            shared,
            kind: Kind::Udp,
            listen,
            upstream,
            tasks,
        })
    }

    /// A TCP relay (HTTP, WebSocket, LiveKit signalling). Port 0 picks a free port.
    pub async fn tcp(
        ctx: &Ctx,
        name: &str,
        listen: SocketAddr,
        upstream: SocketAddr,
    ) -> Result<FaultLink> {
        let shared = Self::shared(ctx, name);
        let (listen, tasks) = tcp::start(shared.clone(), listen, upstream).await?;
        shared.record(
            "relaying",
            serde_json::json!({ "tcp": listen.to_string(), "to": upstream.to_string() }),
        );
        Ok(FaultLink {
            shared,
            kind: Kind::Tcp,
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
                up_queue: VecDeque::new(),
                down_queue: VecDeque::new(),
            }),
            up_seeded: Mutex::new(Seeded::new(direction_seed(
                ctx.seed(),
                name,
                Direction::Uplink,
            ))),
            down_seeded: Mutex::new(Seeded::new(direction_seed(
                ctx.seed(),
                name,
                Direction::Downlink,
            ))),
            generation: AtomicU64::new(0),
            endings: Mutex::new(Vec::new()),
            closed: AtomicBool::new(false),
            port_closed: AtomicBool::new(false),
            changed: Notify::new(),
            parked: Mutex::new(Vec::new()),
            up_packets: AtomicU64::new(0),
            down_packets: AtomicU64::new(0),
            up_bytes: AtomicU64::new(0),
            down_bytes: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            flows: AtomicU64::new(0),
            delayed: AtomicU64::new(0),
            queued: AtomicU64::new(0),
            released: AtomicU64::new(0),
            started: Instant::now(),
            history: Mutex::new(history::History::default()),
            lines: Mutex::new(None),
            up_waiting: AtomicUsize::new(0),
            down_waiting: AtomicUsize::new(0),
        })
    }

    /// Now on the link's clock, the one [`Seen::at`] counts on.
    pub fn clock(&self) -> Duration {
        self.shared.started.elapsed()
    }

    /// The packets of `direction` that reached the link so far, in the order they arrived, each
    /// noted before the fault acted: what the sender sent. `Both` gives the two directions
    /// together. UDP only; a TCP link notes nothing.
    pub fn history(&self, direction: Direction) -> Vec<Seen> {
        let history = self.shared.history.lock();
        match direction {
            Direction::Uplink => history.up.clone(),
            Direction::Downlink => history.down.clone(),
            Direction::Both => {
                let mut both: Vec<Seen> = history.up.iter().chain(&history.down).copied().collect();
                both.sort_by_key(|seen| seen.at);
                both
            }
        }
    }

    /// The address a client dials.
    pub fn address(&self) -> SocketAddr {
        self.listen
    }

    pub fn upstream(&self) -> SocketAddr {
        self.upstream
    }

    pub fn set(&self, direction: Direction, fault: Fault) {
        let mut released = 0;
        {
            let mut faults = self.shared.faults.lock();
            if matches!(direction, Direction::Uplink | Direction::Both) {
                faults.up = fault;
                released += udp::release(&self.shared, &mut faults, Direction::Uplink);
            }
            if matches!(direction, Direction::Downlink | Direction::Both) {
                faults.down = fault;
                released += udp::release(&self.shared, &mut faults, Direction::Downlink);
            }
        }
        self.shared.record(
            "fault",
            serde_json::json!({ "direction": direction, "fault": fault }),
        );
        if released > 0 {
            self.shared.record("released", released);
        }
        self.shared.changed.notify_waiters();
    }

    /// Everything passes again and a closed port reopens. Flows that were half-opened or
    /// orphaned stay as they are: a stale socket does not come back.
    pub fn restore(&self) {
        self.shared.port_closed.store(false, Ordering::SeqCst);
        self.set(Direction::Both, Fault::Pass);
    }

    fn end_flows(&self, how: u8, label: &str) {
        self.shared.end_generation(how);
        self.shared.record(label, ());
        self.shared.changed.notify_waiters();
    }

    /// The flows that exist now go silent for good, with nothing closed: neither side is told.
    /// A new flow from the same client passes. This is a socket both ends still believe in. A
    /// later `cut` or `orphan` ends only the flows made after this call: these stay silent until
    /// the link is dropped.
    pub fn half_open(&self) {
        self.end_flows(ending::HALF_OPEN, "half-open");
    }

    /// The flows that exist now are closed: on TCP both sides see the connection end, on UDP the
    /// next packet of a client starts a new flow from a new port, as after an address change.
    /// Flows that were already half-opened or orphaned stay as they are.
    pub fn cut(&self) {
        self.end_flows(ending::CUT, "cut");
    }

    /// TCP: the connections that exist now are closed towards the client and left open and silent
    /// towards the stack. The client knows its socket is gone and dials again; the stack still
    /// holds the old one. This is what a machine waking from sleep, or changing network, leaves
    /// behind.
    pub fn orphan(&self) -> Result<()> {
        if self.kind != Kind::Tcp {
            bail!("only a TCP link can orphan its connections");
        }
        self.end_flows(ending::ORPHAN, "orphan");
        Ok(())
    }

    /// UDP: the listen port closes, so the client's next packet is answered "port unreachable"
    /// by this machine, which a socket reports as an error on its next call. [`Self::restore`]
    /// reopens it; flows continue from where they were.
    pub fn close_port(&self) -> Result<()> {
        if self.kind != Kind::Udp {
            bail!("only a UDP link can close its port");
        }
        self.shared.port_closed.store(true, Ordering::SeqCst);
        self.shared.record("port closed", ());
        self.shared.changed.notify_waiters();
        Ok(())
    }

    pub fn counters(&self) -> Counters {
        Counters {
            up_packets: self.shared.up_packets.load(Ordering::Relaxed),
            down_packets: self.shared.down_packets.load(Ordering::Relaxed),
            up_bytes: self.shared.up_bytes.load(Ordering::Relaxed),
            down_bytes: self.shared.down_bytes.load(Ordering::Relaxed),
            dropped: self.shared.dropped.load(Ordering::Relaxed),
            flows: self.shared.flows.load(Ordering::Relaxed),
            delayed: self.shared.delayed.load(Ordering::Relaxed),
            queued: self.shared.queued.load(Ordering::Relaxed),
            released: self.shared.released.load(Ordering::Relaxed),
            unrecorded: self.shared.history.lock().missed,
        }
    }
}

impl Drop for FaultLink {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
        self.shared.closed.store(true, Ordering::SeqCst);
        self.shared.end_generation(ending::CUT);
        self.shared.parked.lock().clear();
        // what a queue kept, and what waits in a line, holds the sockets it would go out on
        {
            let mut faults = self.shared.faults.lock();
            faults.up_queue.clear();
            faults.down_queue.clear();
        }
        self.shared.lines.lock().take();
        self.shared.changed.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(seed: u64, name: &str) -> Arc<Shared> {
        let ctx = Ctx::new(
            Arc::new(crate::stack::Stack::pulse_only(
                "127.0.0.1:9".parse().unwrap(),
            )),
            "unit",
            "fault-seeds",
            0,
            seed,
            std::env::temp_dir().join("comms-e2e-unit"),
        );
        FaultLink::shared(&ctx, name)
    }

    fn draw(shared: &Shared, direction: Direction) -> f32 {
        shared.chance(direction)
    }

    #[test]
    fn each_direction_draws_the_same_sequence_whatever_the_order() {
        let (one, other) = (link(7, "pulse"), link(7, "pulse"));
        // one draws every uplink value first, the other alternates, downlink first
        let one_up: Vec<f32> = (0..32).map(|_| draw(&one, Direction::Uplink)).collect();
        let one_down: Vec<f32> = (0..32).map(|_| draw(&one, Direction::Downlink)).collect();
        let (mut other_up, mut other_down) = (Vec::new(), Vec::new());
        for _ in 0..32 {
            other_down.push(draw(&other, Direction::Downlink));
            other_up.push(draw(&other, Direction::Uplink));
        }
        assert_eq!(one_up, other_up);
        assert_eq!(one_down, other_down);
        assert_ne!(one_up, one_down, "the two directions draw apart");
    }

    #[test]
    fn two_links_of_one_run_draw_different_sequences() {
        let sequence = |seed: u64, name: &str| -> Vec<f32> {
            let shared = link(seed, name);
            (0..32).map(|_| draw(&shared, Direction::Uplink)).collect()
        };
        assert_ne!(sequence(7, "api"), sequence(7, "pulse"));
        assert_ne!(sequence(7, "pulse"), sequence(8, "pulse"));
        assert_eq!(sequence(7, "pulse"), sequence(7, "pulse"));
    }
}
