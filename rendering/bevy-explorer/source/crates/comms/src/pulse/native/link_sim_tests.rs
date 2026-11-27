//! The uplink gate over a simulated link: the driver's pass and two real ENet hosts on a virtual
//! clock, with a round trip, jitter, loss and a stall set per test. The link draws from a seeded
//! generator, so every run is the same.
//!
//! The client side wakes as the driver does: on an input, on a datagram, and otherwise once a
//! [`MAINTENANCE_INTERVAL`] has passed (or sooner when the gate asks). The server side is a plain
//! ENet host serviced every millisecond that streams an unreliable datagram every 50 ms, as the
//! Pulse server streams the world, and charges every input that arrives against its input bucket
//! (20 a second, burst 16: `services/catalyst/comms/src/pulse/hardening.rs`).

use std::cell::{Cell, RefCell};
use std::net::SocketAddrV4;
use std::rc::Rc;

use super::*;
use crate::pulse::movement_pacing::{Change, MovementPacer};

const CLIENT: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 1), 1));
const SERVER: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 2), 2));

/// The servers' input bucket (`DEFAULT_INPUT_BURST`, `DEFAULT_INPUT_MAX_HZ`).
const SERVER_INPUT_BURST: u32 = 16;
const SERVER_INPUT_REFILL_MS: u64 = 1000 / 20;

/// The path between the two hosts.
#[derive(Clone, Copy)]
struct Link {
    round_trip_ms: u64,
    /// The round trip of each exchange is off by up to this, either way.
    jitter_ms: u64,
    /// The share of datagrams lost, each way.
    loss: f64,
    /// Client datagrams sent from the first time to the second wait in the path and arrive
    /// together after it.
    stall: Option<(u64, u64)>,
}

impl Link {
    fn round_trip(round_trip_ms: u64) -> Self {
        Self {
            round_trip_ms,
            jitter_ms: 0,
            loss: 0.0,
            stall: None,
        }
    }
}

struct Datagram {
    at: u64,
    order: u64,
    to: SocketAddr,
    bytes: Vec<u8>,
}

/// The datagrams in the path.
struct Wire {
    now: Rc<Cell<u64>>,
    link: Link,
    random: u64,
    order: u64,
    queued: Vec<Datagram>,
    /// Lose what the client sends until cleared.
    lose_uplink: bool,
}

impl Wire {
    /// A uniform draw in [0, 1).
    fn draw(&mut self) -> f64 {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 7;
        self.random ^= self.random << 17;
        (self.random >> 11) as f64 / (1u64 << 53) as f64
    }

    fn send(&mut self, from: SocketAddr, to: SocketAddr, bytes: &[u8]) {
        let now = self.now.get();
        let uplink = from == CLIENT;
        let lost = self.draw() < self.link.loss;
        if lost || (uplink && self.lose_uplink) {
            return;
        }
        // Each way takes half the round trip, off by up to half the jitter.
        let jitter = (self.draw() * 2.0 - 1.0) * self.link.jitter_ms as f64 / 2.0;
        let one_way = (self.link.round_trip_ms as f64 / 2.0 + jitter).max(0.0) as u64;
        let mut at = now + one_way;
        if let Some((start, end)) = self.link.stall.filter(|_| uplink) {
            if (start..end).contains(&now) {
                at = end + one_way;
            }
        }
        self.order += 1;
        self.queued.push(Datagram {
            at,
            order: self.order,
            to,
            bytes: bytes.to_vec(),
        });
    }

    fn due(&self, to: SocketAddr) -> Option<usize> {
        let now = self.now.get();
        self.queued
            .iter()
            .enumerate()
            .filter(|(_, datagram)| datagram.to == to && datagram.at <= now)
            .min_by_key(|(_, datagram)| (datagram.at, datagram.order))
            .map(|(index, _)| index)
    }

    fn receive(&mut self, to: SocketAddr) -> Option<Vec<u8>> {
        let index = self.due(to)?;
        Some(self.queued.swap_remove(index).bytes)
    }
}

struct SimSocket {
    address: SocketAddr,
    wire: Rc<RefCell<Wire>>,
}

impl enet::Socket for SimSocket {
    type Address = SocketAddr;
    type Error = io::Error;

    fn send(&mut self, address: SocketAddr, buffer: &[u8]) -> io::Result<usize> {
        self.wire.borrow_mut().send(self.address, address, buffer);
        Ok(buffer.len())
    }

    fn receive(
        &mut self,
        buffer: &mut [u8; enet::MTU_MAX],
    ) -> io::Result<Option<(SocketAddr, enet::PacketReceived)>> {
        let Some(bytes) = self.wire.borrow_mut().receive(self.address) else {
            return Ok(None);
        };
        buffer[..bytes.len()].copy_from_slice(&bytes);
        let from = if self.address == CLIENT {
            SERVER
        } else {
            CLIENT
        };
        Ok(Some((from, enet::PacketReceived::Complete(bytes.len()))))
    }
}

/// What the movement systems offer the driver.
#[derive(Clone, Copy, PartialEq)]
enum Supply {
    /// Ordinary movement: an input every `REGULAR_SPACING_SECONDS`.
    Regular,
    /// The most `movement_pacing` lets out: an edge on every 60 Hz frame.
    Edges,
}

/// One driver pass: when, and what the gate said after it.
#[derive(Clone, Copy)]
struct Pass {
    at: u64,
    held_input: bool,
    stalled: bool,
    inputs_sent: usize,
}

struct Sim {
    now: Rc<Cell<u64>>,
    wire: Rc<RefCell<Wire>>,
    client: enet::Host<SimSocket>,
    server: enet::Host<SimSocket>,
    peer: enet::PeerID,
    gate: UplinkGate,
    connected: bool,
    base: Instant,
    last_pass: u64,
    supply: Supply,
    pacer: MovementPacer,
    next_frame: u64,
    /// Lose the first probe sent at or after this time.
    lose_probe_at: Option<u64>,
    passes: Vec<Pass>,
    /// When each input reached the server.
    arrivals: Vec<u64>,
}

fn host(
    address: SocketAddr,
    wire: &Rc<RefCell<Wire>>,
    now: &Rc<Cell<u64>>,
) -> enet::Host<SimSocket> {
    let now = now.clone();
    enet::Host::new(
        SimSocket {
            address,
            wire: wire.clone(),
        },
        enet::HostSettings {
            peer_limit: 1,
            channel_limit: CHANNEL_COUNT,
            time: Box::new(move || Duration::from_millis(now.get())),
            seed: Some(7),
            ..Default::default()
        },
    )
    .unwrap()
}

impl Sim {
    /// Connected over `link`, with ENet's round trip estimate settled.
    fn new(link: Link, supply: Supply, seed: u64) -> Self {
        let now = Rc::new(Cell::new(1_000));
        let wire = Rc::new(RefCell::new(Wire {
            now: now.clone(),
            link: Link {
                stall: None,
                ..link
            },
            // Never zero, which the generator would keep.
            random: seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1,
            order: 0,
            queued: Vec::new(),
            lose_uplink: false,
        }));
        let mut client = host(CLIENT, &wire, &now);
        let server = host(SERVER, &wire, &now);
        let peer = client.connect(SERVER, CHANNEL_COUNT, 0).unwrap().id();
        let mut sim = Self {
            now,
            wire,
            client,
            server,
            peer,
            gate: UplinkGate::default(),
            connected: false,
            base: Instant::now(),
            last_pass: 0,
            supply,
            pacer: MovementPacer::default(),
            next_frame: 0,
            lose_probe_at: None,
            passes: Vec::new(),
            arrivals: Vec::new(),
        };
        let deadline = sim.time() + 30_000;
        while !sim.connected {
            assert!(sim.time() < deadline, "never connected");
            sim.step();
        }
        sim.run_for(10_000);
        sim.wire.borrow_mut().link = link;
        sim.passes.clear();
        sim.arrivals.clear();
        sim
    }

    fn time(&self) -> u64 {
        self.now.get()
    }

    fn run_for(&mut self, millis: u64) {
        let end = self.time() + millis;
        while self.time() < end {
            self.step();
        }
    }

    /// The inputs the movement systems hand the driver this millisecond.
    fn offered(&mut self, now: u64) -> Vec<PulseFrame> {
        if !self.connected || now < self.next_frame {
            return Vec::new();
        }
        let seconds = now as f64 / 1000.0;
        let send = match self.supply {
            Supply::Regular => {
                self.next_frame = now + 100;
                true
            }
            Supply::Edges => {
                self.next_frame = now + 16;
                let send = self.pacer.should_send(seconds, Change::Edge);
                if send {
                    self.pacer.record_send(seconds);
                }
                send
            }
        };
        if !send {
            return Vec::new();
        }
        vec![PulseFrame {
            bytes: vec![0; 40],
            reliability: PulseReliability::UnreliableSequenced,
            kind: FrameKind::Input,
        }]
    }

    fn step(&mut self) {
        let now = self.now.get() + 1;
        self.now.set(now);

        let mut stream = now.is_multiple_of(50);
        while let Ok(Some(event)) = self.server.service() {
            match event {
                enet::Event::Receive { channel_id: 1, .. } => self.arrivals.push(now),
                enet::Event::Connect { .. } => stream = true,
                _ => {}
            }
        }
        let up = matches!(
            self.server.peer(enet::PeerID(0)).state(),
            enet::PeerState::Connected
        );
        if stream && up {
            let packet = enet::Packet::unreliable_unsequenced([0u8; 64].as_slice());
            self.server
                .peer_mut(enet::PeerID(0))
                .send(2, &packet)
                .unwrap();
            self.server.flush();
        }

        let batch = self.offered(now);
        let readable = self.wire.borrow().due(CLIENT).is_some();
        let woken = now - self.last_pass >= self.wake_after(self.last_pass).as_millis() as u64;
        if batch.is_empty() && !readable && !woken {
            return;
        }
        self.last_pass = now;
        let at = self.base + Duration::from_millis(now);
        let probes = self.gate.probes;
        let sent = pass(
            &mut self.client,
            self.peer,
            self.connected,
            &mut self.gate,
            batch,
            at,
        );
        let lose = self.gate.probes > probes && self.lose_probe_at.is_some_and(|from| now >= from);
        if lose {
            self.lose_probe_at = None;
        }
        self.wire.borrow_mut().lose_uplink = lose;
        while let Ok(Some(event)) = self.client.service() {
            if let enet::Event::Connect { .. } = event {
                self.connected = true;
            }
        }
        self.wire.borrow_mut().lose_uplink = false;
        let snapshot = self.gate.snapshot(at);
        self.passes.push(Pass {
            at: now,
            held_input: snapshot.held_input,
            stalled: snapshot.stalled,
            inputs_sent: sent
                .iter()
                .filter(|frame| frame.kind == FrameKind::Input)
                .count(),
        });
    }

    /// How long the driver blocks with nothing to do.
    fn wake_after(&self, now: u64) -> Duration {
        self.gate
            .wake_within(self.base + Duration::from_millis(now))
    }

    /// The input holds since the passes were last cleared: when each began and how long it
    /// lasted, a hold still open at the end lasting until the last pass.
    fn holds(&self) -> Vec<(u64, u64)> {
        let mut holds = Vec::new();
        let mut since = None;
        for pass in &self.passes {
            match (since, pass.held_input) {
                (None, true) => since = Some(pass.at),
                (Some(start), false) => {
                    holds.push((start, pass.at - start));
                    since = None;
                }
                _ => {}
            }
        }
        if let (Some(start), Some(last)) = (since, self.passes.last()) {
            holds.push((start, last.at - start));
        }
        holds
    }

    /// Inputs the server's bucket turned away, each of which ends the session.
    fn rejected_inputs(&self) -> usize {
        let mut tokens = SERVER_INPUT_BURST;
        let mut refilled = 0;
        let mut rejected = 0;
        for &at in &self.arrivals {
            if refilled == 0 {
                refilled = at;
            }
            let refills = (at - refilled) / SERVER_INPUT_REFILL_MS;
            tokens = (tokens + refills as u32).min(SERVER_INPUT_BURST);
            refilled += refills * SERVER_INPUT_REFILL_MS;
            if tokens == 0 {
                rejected += 1;
            } else {
                tokens -= 1;
            }
        }
        rejected
    }

    /// Inputs sent from `start` to `end`.
    fn inputs_sent(&self, start: u64, end: u64) -> usize {
        self.passes
            .iter()
            .filter(|pass| (start..end).contains(&pass.at))
            .map(|pass| pass.inputs_sent)
            .sum()
    }

    /// The first pass at or after `start` that found the uplink stalled.
    fn closed_at(&self, start: u64) -> Option<u64> {
        self.passes
            .iter()
            .find(|pass| pass.at >= start && pass.stalled)
            .map(|pass| pass.at)
    }
}

fn describe(holds: &[(u64, u64)], window: u64) -> String {
    let held: u64 = holds.iter().map(|(_, length)| length).sum();
    let lengths: Vec<u64> = holds.iter().map(|(_, length)| *length).collect();
    format!(
        "{} holds, {:.1}% of {} ms held, lengths {:?}",
        holds.len(),
        held as f64 * 100.0 / window as f64,
        window,
        &lengths[..lengths.len().min(12)]
    )
}

/// On a healthy link, however long its round trip, inputs are never held.
#[test]
fn a_healthy_link_with_a_long_round_trip_never_holds_inputs() {
    let mut failures = Vec::new();
    for round_trip in [100, 300, 600, 800, 1200, 2000] {
        let mut sim = Sim::new(Link::round_trip(round_trip), Supply::Regular, 1);
        sim.run_for(30_000);
        let holds = sim.holds();
        let start = sim.passes.first().map_or(0, |pass| pass.at);
        let sent = sim.inputs_sent(start, start + 30_000);
        let snapshot = sim
            .gate
            .snapshot(sim.base + Duration::from_millis(sim.time()));
        println!(
            "healthy {round_trip} ms: {}, {:.1} inputs a second, round trip {:?}, spacing {:?}",
            describe(&holds, 30_000),
            sent as f64 / 30.0,
            snapshot.round_trip,
            snapshot.input_spacing
        );
        if !holds.is_empty() {
            failures.push(round_trip);
        }
    }
    assert!(failures.is_empty(), "inputs held at {failures:?} ms");
}

/// The input holds, in 20 runs at a 300 ms round trip off by up to `jitter_ms`, that losing one
/// probe causes.
fn holds_after_one_lost_probe(jitter_ms: u64) -> Vec<u64> {
    let link = Link {
        jitter_ms,
        ..Link::round_trip(300)
    };
    let mut lengths = Vec::new();
    for seed in 1..=20 {
        let mut sim = Sim::new(link, Supply::Regular, seed);
        sim.lose_probe_at = Some(sim.time() + 1_000);
        sim.run_for(5_000);
        lengths.extend(sim.holds().iter().map(|(_, length)| *length));
    }
    println!(
        "one lost probe at 300 ± {jitter_ms} ms: {} holds in 20 runs, lengths {lengths:?}",
        lengths.len()
    );
    lengths
}

/// At a 300 ms round trip that varies by 50 ms either way, losing one probe does not hold inputs.
#[test]
fn one_lost_probe_does_not_hold_inputs() {
    let lengths = holds_after_one_lost_probe(50);
    assert!(lengths.is_empty(), "held: {lengths:?}");
}

/// At 300 ms ± 100, the acknowledgements on either side of a lost probe can land more than
/// [`STALL_AFTER`] apart, which holds an input briefly; never for a loop pass.
#[test]
fn one_lost_probe_on_a_jittery_link_holds_inputs_for_less_than_a_pass() {
    let lengths = holds_after_one_lost_probe(100);
    let pass = MAINTENANCE_INTERVAL.as_millis() as u64;
    assert!(
        lengths.iter().all(|&length| length < pass),
        "held: {lengths:?}"
    );
}

/// Two percent of datagrams lost each way at 300 ± 100 ms: how often inputs are held. A lost
/// acknowledgement counts like a lost probe.
#[test]
fn a_lossy_link_rarely_holds_inputs() {
    let link = Link {
        jitter_ms: 100,
        loss: 0.02,
        ..Link::round_trip(300)
    };
    let mut sim = Sim::new(link, Supply::Regular, 3);
    sim.run_for(120_000);
    let holds = sim.holds();
    println!("2% loss at 300 ± 100 ms: {}", describe(&holds, 120_000));
    assert!(holds.len() <= 1, "{}", describe(&holds, 120_000));
}

/// Whatever the round trip, up to 3 s and off by up to a quarter either way, the inputs sent
/// between the start of a stall and the gate closing, which the link delivers together when it
/// recovers, stay within the servers' input burst, and the servers' bucket never turns one away.
/// The movement systems offer the most `movement_pacing` lets out.
#[test]
fn a_stall_delivers_no_more_inputs_than_the_servers_burst() {
    let mut failures = Vec::new();
    for round_trip in [100, 300, 500, 600, 700, 800, 1200, 2000, 3000] {
        let mut worst = (0, 0, 0);
        let steady = Link::round_trip(round_trip);
        let jittery = Link {
            jitter_ms: round_trip / 4,
            ..steady
        };
        for (link, phase) in [steady, jittery]
            .into_iter()
            .flat_map(|link| [0, 60, 120, 180, 240].map(|phase| (link, phase)))
        {
            let mut sim = Sim::new(link, Supply::Edges, 5 + phase);
            sim.run_for(phase);
            let start = sim.time() + 2_000;
            let end = start + round_trip + 3_000;
            sim.wire.borrow_mut().link.stall = Some((start, end));
            sim.run_for(end - sim.time() + 2 * round_trip + 2_000);
            let closed = sim.closed_at(start).map_or(u64::MAX, |at| at - start);
            let burst = sim.inputs_sent(start, end);
            let rejected = sim.rejected_inputs();
            if (burst, rejected) > (worst.0, worst.2) {
                worst = (burst, closed, rejected);
            }
        }
        let (burst, closed, rejected) = worst;
        println!(
            "stall at {round_trip} ms: gate closed {closed} ms in, {burst} inputs sent into it, \
             {rejected} turned away"
        );
        if burst > SERVER_INPUT_BURST as usize || rejected > 0 {
            failures.push(round_trip);
        }
    }
    assert!(failures.is_empty(), "over the burst at {failures:?} ms");
}
