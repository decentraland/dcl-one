//! The UDP relay: one upstream socket per client flow, bound to the client's own IP address, so
//! the stack sees one flow per actor and counts its per-address limits as it would without the
//! link.

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use futures::future::BoxFuture;
use parking_lot::Mutex;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::task::AbortHandle;

use super::{Direction, Fate, Fault, Faults, Shared, ending};

const MAX_DATAGRAM: usize = 65_535;

/// Sends one packet on, to where the packet was going.
type Sender = Arc<dyn Fn(Vec<u8>) -> BoxFuture<'static, ()> + Send + Sync>;

/// A packet a `Queue` keeps, and the way it goes on.
pub(crate) struct Queued {
    packet: Vec<u8>,
    send: Sender,
}

/// A packet that waits in a line until it is due.
struct Due {
    at: Instant,
    packet: Vec<u8>,
    send: Sender,
}

/// The two directions' lines: each sends its packets in the order they joined it, each once it is
/// due, so a packet due earlier waits for the one before it.
pub(crate) struct Lines {
    up: mpsc::UnboundedSender<Due>,
    down: mpsc::UnboundedSender<Due>,
}

fn queue_of(faults: &mut Faults, direction: Direction) -> &mut VecDeque<Queued> {
    match direction {
        Direction::Uplink => &mut faults.up_queue,
        _ => &mut faults.down_queue,
    }
}

fn fault_of(faults: &Faults, direction: Direction) -> Fault {
    match direction {
        Direction::Uplink => faults.up,
        _ => faults.down,
    }
}

/// What a fault does with one packet.
enum Action {
    Now,
    Twice,
    /// After a wait of its own: it may overtake others (`Delay`, `Reorder`).
    Alone(Instant),
    /// In the line, once due: it never overtakes (`Link`).
    InLine(Instant),
    Lose,
}

impl Action {
    fn fate(&self) -> Fate {
        match self {
            Action::Now | Action::Twice => Fate::Passed,
            Action::Alone(_) | Action::InLine(_) => Fate::Delayed,
            Action::Lose => Fate::Lost,
        }
    }
}

/// What `fault` does with the next packet of `direction`, drawing from the direction's generator.
/// A `Queue` is dealt with before this is asked.
fn decide(shared: &Shared, direction: Direction, fault: Fault) -> Action {
    let now = Instant::now();
    match fault {
        Fault::Pass => Action::Now,
        Fault::Stall | Fault::Queue { .. } => Action::Lose,
        Fault::Drop { share } => {
            if shared.chance(direction) < share {
                Action::Lose
            } else {
                Action::Now
            }
        }
        Fault::Delay { by } => Action::Alone(now + by),
        Fault::Reorder { window } => Action::Alone(now + shared.gap(direction, window)),
        Fault::Duplicate => Action::Twice,
        Fault::Link {
            delay,
            jitter,
            loss,
        } => {
            if loss > 0.0 && shared.chance(direction) < loss {
                return Action::Lose;
            }
            let extra = if jitter.is_zero() {
                Duration::ZERO
            } else {
                shared.gap(direction, jitter)
            };
            let wait = delay + extra;
            if wait.is_zero() {
                Action::Now
            } else {
                Action::InLine(now + wait)
            }
        }
    }
}

/// Puts a packet in the line of `direction`, to go at `at`.
fn join_line(shared: &Shared, direction: Direction, at: Instant, packet: Vec<u8>, send: Sender) {
    let lines = shared.lines.lock();
    let Some(lines) = lines.as_ref() else {
        // the link is gone
        return;
    };
    let line = match direction {
        Direction::Uplink => &lines.up,
        _ => &lines.down,
    };
    shared.waiting(direction).fetch_add(1, Ordering::SeqCst);
    if line.send(Due { at, packet, send }).is_err() {
        shared.waiting(direction).fetch_sub(1, Ordering::SeqCst);
    }
}

/// Sends the packets of one direction's line, each once it is due, in order.
async fn line(
    shared: Arc<Shared>,
    direction: Direction,
    mut packets: mpsc::UnboundedReceiver<Due>,
) {
    while let Some(due) = packets.recv().await {
        // what waits is dropped with the link, sockets and all
        if !shared.sleep_until_unless_closed(due.at).await {
            return;
        }
        (due.send)(due.packet).await;
        shared.waiting(direction).fetch_sub(1, Ordering::SeqCst);
    }
}

fn start_lines(shared: &Arc<Shared>) -> Vec<AbortHandle> {
    let (up, up_packets) = mpsc::unbounded_channel();
    let (down, down_packets) = mpsc::unbounded_channel();
    *shared.lines.lock() = Some(Lines { up, down });
    vec![
        tokio::spawn(line(shared.clone(), Direction::Uplink, up_packets)).abort_handle(),
        tokio::spawn(line(shared.clone(), Direction::Downlink, down_packets)).abort_handle(),
    ]
}

/// Sends a packet that may go at once: straight away, or behind the packets that wait in the
/// line, so it does not overtake them.
async fn go(shared: &Arc<Shared>, direction: Direction, packet: Vec<u8>, send: Sender) {
    if shared.waiting(direction).load(Ordering::SeqCst) > 0 {
        join_line(shared, direction, Instant::now(), packet, send);
    } else {
        send(packet).await;
    }
}

/// Carries out what a fault decided for one packet.
async fn act(
    shared: &Arc<Shared>,
    direction: Direction,
    action: Action,
    packet: Vec<u8>,
    send: Sender,
) {
    match action {
        Action::Now => go(shared, direction, packet, send).await,
        Action::Twice => {
            go(shared, direction, packet.clone(), send.clone()).await;
            go(shared, direction, packet, send).await;
        }
        // A held packet lives no longer than the link: the task that holds it also holds the
        // listen socket, which must be free when the link is dropped, whatever the delay.
        Action::Alone(at) => {
            shared.delayed.fetch_add(1, Ordering::Relaxed);
            let shared = shared.clone();
            tokio::spawn(async move {
                if shared.sleep_until_unless_closed(at).await {
                    send(packet).await;
                }
            });
        }
        Action::InLine(at) => {
            shared.delayed.fetch_add(1, Ordering::Relaxed);
            join_line(shared, direction, at, packet, send);
        }
        Action::Lose => {
            shared.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// The fault of `direction` was just set: if it is no longer a queue, what the queue kept goes on
/// through it, in the order it came, ahead of anything that arrives later. Called with the faults
/// locked, so no packet slips between. The number of packets released.
pub(super) fn release(shared: &Arc<Shared>, faults: &mut Faults, direction: Direction) -> usize {
    let fault = fault_of(faults, direction);
    if matches!(fault, Fault::Queue { .. }) {
        return 0;
    }
    let kept: Vec<Queued> = queue_of(faults, direction).drain(..).collect();
    let count = kept.len();
    shared.released.fetch_add(count as u64, Ordering::Relaxed);
    for Queued { packet, send } in kept {
        match decide(shared, direction, fault) {
            Action::Now => join_line(shared, direction, Instant::now(), packet, send),
            Action::Twice => {
                join_line(
                    shared,
                    direction,
                    Instant::now(),
                    packet.clone(),
                    send.clone(),
                );
                join_line(shared, direction, Instant::now(), packet, send);
            }
            Action::Alone(at) => {
                shared.delayed.fetch_add(1, Ordering::Relaxed);
                let shared = shared.clone();
                tokio::spawn(async move {
                    if shared.sleep_until_unless_closed(at).await {
                        send(packet).await;
                    }
                });
            }
            Action::InLine(at) => {
                shared.delayed.fetch_add(1, Ordering::Relaxed);
                join_line(shared, direction, at, packet, send);
            }
            Action::Lose => {
                shared.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    count
}

struct Flow {
    upstream: Arc<UdpSocket>,
    generation: u64,
    pump: AbortHandle,
}

impl Drop for Flow {
    // The pump holds the listen socket: a flow that goes must take its pump along, or the port
    // stays bound after the link is dropped.
    fn drop(&mut self) {
        self.pump.abort();
    }
}

/// Binds `address`, waiting out a port that a link dropped a moment ago is still releasing.
async fn bind(address: SocketAddr) -> std::io::Result<UdpSocket> {
    let mut last = None;
    for _ in 0..100 {
        match UdpSocket::bind(address).await {
            Ok(socket) => return Ok(socket),
            Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => last = Some(error),
            Err(error) => return Err(error),
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    Err(last.expect("a failed attempt left its error"))
}

/// The listen socket, which a closed port takes away and a restore puts back.
type Listen = Arc<Mutex<Option<Arc<UdpSocket>>>>;

/// What a closed port keeps of the listen socket. On Linux the socket stays bound, connected to
/// its own address: a datagram from anyone else finds no socket, so this machine answers "port
/// unreachable" as for a port nobody holds, and no other socket can take the port meanwhile.
/// Elsewhere a connected socket drops such a datagram without an answer, so the socket is given
/// up and the port is bound again on restore.
async fn reserve(socket: Arc<UdpSocket>, address: SocketAddr) -> Option<Arc<UdpSocket>> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    socket.connect(address).await.ok().map(|()| socket)
}

/// Binds the link's port again after a restore. Until it is free (another socket took it while
/// it was closed) the relay tries again every few milliseconds and the timeline says so: the
/// link never gives up its port while it lives. `None`: the link was dropped first.
async fn rebind(shared: &Shared, address: SocketAddr) -> Option<UdpSocket> {
    let mut reported = false;
    loop {
        match UdpSocket::bind(address).await {
            Ok(socket) => {
                if reported {
                    shared.record("port free again", address.to_string());
                }
                return Some(socket);
            }
            Err(error) if !reported => {
                shared.record(
                    "port taken",
                    format!("cannot bind {address} again yet, trying until it is free: {error}"),
                );
                reported = true;
            }
            Err(_) => {}
        }
        if !shared
            .sleep_unless_closed(std::time::Duration::from_millis(20))
            .await
        {
            return None;
        }
    }
}

pub(super) async fn start(
    shared: Arc<Shared>,
    listen: SocketAddr,
    upstream: SocketAddr,
) -> Result<(SocketAddr, Vec<AbortHandle>)> {
    let socket = Arc::new(
        bind(listen)
            .await
            .with_context(|| format!("the fault link cannot listen on UDP {listen}"))?,
    );
    let listen = socket.local_addr()?;
    let mut tasks = start_lines(&shared);
    tasks.push(tokio::spawn(relay(shared, socket, listen, upstream)).abort_handle());
    Ok((listen, tasks))
}

async fn relay(
    shared: Arc<Shared>,
    socket: Arc<UdpSocket>,
    address: SocketAddr,
    upstream: SocketAddr,
) {
    let listen: Listen = Arc::new(Mutex::new(Some(socket)));
    let mut flows: HashMap<SocketAddr, Flow> = HashMap::new();
    let mut buffer = vec![0u8; MAX_DATAGRAM];
    loop {
        // A closed port: take the socket away from the flows (keeping the port where the
        // platform allows), wait for the restore, bind the same port again.
        if shared.port_closed.load(Ordering::SeqCst) {
            let taken = listen.lock().take();
            let reserved = match taken {
                Some(socket) => reserve(socket, address).await,
                None => None,
            };
            loop {
                let changed = shared.changed.notified();
                tokio::pin!(changed);
                changed.as_mut().enable();
                if !shared.port_closed.load(Ordering::SeqCst)
                    || shared.closed.load(Ordering::SeqCst)
                {
                    break;
                }
                changed.await;
            }
            drop(reserved);
            match rebind(&shared, address).await {
                Some(socket) => {
                    *listen.lock() = Some(Arc::new(socket));
                    shared.record("port reopened", ());
                }
                None => return,
            }
        }
        let socket = listen.lock().clone().expect("the port is open here");
        let changed = shared.changed.notified();
        tokio::pin!(changed);
        changed.as_mut().enable();
        if shared.port_closed.load(Ordering::SeqCst) {
            continue;
        }
        let (length, client) = tokio::select! {
            // a close that has arrived wins over a datagram waiting in the socket
            biased;
            _ = &mut changed => continue,
            received = socket.recv_from(&mut buffer) => match received {
                Ok(received) => received,
                // Windows reports an earlier send to a closed port here; the relay goes on.
                Err(_) => continue,
            },
        };
        drop(socket);
        uplink(
            &shared,
            &mut flows,
            &listen,
            upstream,
            client,
            buffer[..length].to_vec(),
        )
        .await;
    }
}

/// One datagram from `client`: through the client's flow (made if it has none) towards the
/// stack, as the uplink's fault says.
async fn uplink(
    shared: &Arc<Shared>,
    flows: &mut HashMap<SocketAddr, Flow>,
    listen: &Listen,
    upstream: SocketAddr,
    client: SocketAddr,
    packet: Vec<u8>,
) {
    if shared.port_closed.load(Ordering::SeqCst) {
        // the port closed while this datagram was being received: a closed port passes nothing
        shared.dropped.fetch_add(1, Ordering::Relaxed);
        return;
    }
    let generation = shared.generation.load(Ordering::SeqCst);
    let fate = flows
        .get(&client)
        .and_then(|flow| shared.fate(flow.generation));
    match fate {
        Some(ending::HALF_OPEN) => {
            // this flow is silent for good, whatever happened to the link since
            shared.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        }
        // cut: the next packet starts a new flow, from a new upstream port
        Some(_) => {
            flows.remove(&client);
        }
        None => {}
    }
    if let std::collections::hash_map::Entry::Vacant(slot) = flows.entry(client) {
        let Ok(socket) = UdpSocket::bind((client.ip(), 0)).await else {
            shared.record(
                "flow failed",
                format!("cannot bind an upstream socket on {}", client.ip()),
            );
            return;
        };
        if socket.connect(upstream).await.is_err() {
            return;
        }
        let socket = Arc::new(socket);
        shared.flows.fetch_add(1, Ordering::Relaxed);
        shared.record("flow", serde_json::json!({ "client": client.to_string() }));
        let pump = tokio::spawn(downlink(
            shared.clone(),
            socket.clone(),
            listen.clone(),
            client,
            generation,
        ));
        slot.insert(Flow {
            upstream: socket,
            generation,
            pump: pump.abort_handle(),
        });
    }
    let target = flows[&client].upstream.clone();
    forward(shared, Direction::Uplink, packet, move |packet| {
        let target = target.clone();
        async move {
            let _ = target.send(&packet).await;
        }
    })
    .await;
}

async fn downlink(
    shared: Arc<Shared>,
    upstream: Arc<UdpSocket>,
    listen: Listen,
    client: SocketAddr,
    generation: u64,
) {
    let mut buffer = vec![0u8; MAX_DATAGRAM];
    loop {
        let length = match upstream.recv(&mut buffer).await {
            Ok(length) => length,
            Err(_) => continue,
        };
        if shared.fate(generation).is_some() {
            // half-open or cut: this flow answers nobody any more
            shared.dropped.fetch_add(1, Ordering::Relaxed);
            continue;
        }
        let packet = buffer[..length].to_vec();
        let listen = listen.clone();
        let counted = shared.clone();
        forward(&shared, Direction::Downlink, packet, move |packet| {
            // the socket is looked up per packet: a closed port has none
            let socket = listen.lock().clone();
            let counted = counted.clone();
            async move {
                match socket {
                    Some(socket) => {
                        let _ = socket.send_to(&packet, client).await;
                    }
                    None => {
                        counted.dropped.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        })
        .await;
    }
}

/// Notes one packet in the history, then applies the direction's fault to it and sends it on if
/// the fault lets it.
async fn forward<F, Fut>(shared: &Arc<Shared>, direction: Direction, packet: Vec<u8>, send: F)
where
    F: Fn(Vec<u8>) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let at = shared.started.elapsed();
    let bytes = packet.len();
    shared.count(direction, bytes);
    let send: Sender = Arc::new(move |packet| Box::pin(send(packet)));
    let action = {
        let mut faults = shared.faults.lock();
        if let Fault::Queue { limit } = fault_of(&faults, direction) {
            let queue = queue_of(&mut faults, direction);
            let fate = if queue.len() < limit {
                queue.push_back(Queued { packet, send });
                shared.queued.fetch_add(1, Ordering::Relaxed);
                Fate::Queued
            } else {
                shared.dropped.fetch_add(1, Ordering::Relaxed);
                Fate::Lost
            };
            drop(faults);
            shared.note(direction, at, bytes, fate);
            return;
        }
        decide(shared, direction, fault_of(&faults, direction))
    };
    shared.note(direction, at, bytes, action.fate());
    act(shared, direction, action, packet, send).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Ctx;
    use crate::fault::FaultLink;
    use std::time::Duration;

    #[tokio::test]
    async fn a_datagram_received_after_the_port_closed_is_not_forwarded() {
        let ctx = Ctx::new(
            Arc::new(crate::stack::Stack::pulse_only(
                "127.0.0.1:9".parse().unwrap(),
            )),
            "unit",
            "udp-closed-port",
            0,
            1,
            std::env::temp_dir().join("comms-e2e-unit"),
        );
        let link = FaultLink::udp(
            &ctx,
            "closing",
            "127.0.0.1:0".parse().unwrap(),
            "127.0.0.1:9".parse().unwrap(),
        )
        .await
        .unwrap();
        link.close_port().unwrap();
        let stack = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let listen: Listen = Arc::new(Mutex::new(None));
        let mut flows = HashMap::new();
        // what the relay does with a datagram its receive returned after the close
        uplink(
            &link.shared,
            &mut flows,
            &listen,
            stack.local_addr().unwrap(),
            "127.0.0.1:40000".parse().unwrap(),
            b"late".to_vec(),
        )
        .await;
        let mut buffer = [0u8; 64];
        let arrived =
            tokio::time::timeout(Duration::from_millis(300), stack.recv_from(&mut buffer)).await;
        assert!(arrived.is_err(), "a datagram crossed a closed port");
        assert_eq!(link.counters().dropped, 1);
        assert_eq!(link.counters().up_packets, 0);
    }
}
