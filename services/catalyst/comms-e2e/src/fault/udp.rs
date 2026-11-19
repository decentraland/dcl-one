//! The UDP relay: one upstream socket per client flow, bound to the client's own IP address, so
//! the stack sees one flow per actor and counts its per-address limits as it would without the
//! link.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use tokio::net::UdpSocket;
use tokio::task::AbortHandle;

use super::{Direction, Fault, Shared, ending};

const MAX_DATAGRAM: usize = 65_535;

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
    let task = tokio::spawn(relay(shared, socket, listen, upstream));
    Ok((listen, vec![task.abort_handle()]))
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
        // A closed port: give the socket up, wait for the restore, bind the same port again.
        if shared.port_closed.load(Ordering::SeqCst) {
            listen.lock().take();
            loop {
                let changed = shared.changed.notified();
                tokio::pin!(changed);
                changed.as_mut().enable();
                if !shared.port_closed.load(Ordering::SeqCst) {
                    break;
                }
                changed.await;
            }
            match bind(address).await {
                Ok(socket) => {
                    *listen.lock() = Some(Arc::new(socket));
                    shared.record("port reopened", ());
                }
                Err(error) => {
                    shared.record("port lost", format!("cannot bind {address} again: {error}"));
                    return;
                }
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

/// Applies the direction's fault to one packet and sends what survives.
async fn forward<F, Fut>(shared: &Arc<Shared>, direction: Direction, packet: Vec<u8>, send: F)
where
    F: Fn(Vec<u8>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    shared.count(direction, packet.len());
    match shared.fault(direction) {
        Fault::Pass => send(packet).await,
        Fault::Stall => {
            shared.dropped.fetch_add(1, Ordering::Relaxed);
        }
        Fault::Drop { share } => {
            if shared.chance(direction) < share {
                shared.dropped.fetch_add(1, Ordering::Relaxed);
            } else {
                send(packet).await;
            }
        }
        // A held packet lives no longer than the link: the task that holds it also holds the
        // listen socket, which must be free when the link is dropped, whatever the delay.
        Fault::Delay { by } => {
            let shared = shared.clone();
            tokio::spawn(async move {
                if shared.sleep_unless_closed(by).await {
                    send(packet).await;
                }
            });
        }
        Fault::Reorder { window } => {
            let wait = shared.gap(direction, window);
            let shared = shared.clone();
            tokio::spawn(async move {
                if shared.sleep_unless_closed(wait).await {
                    send(packet).await;
                }
            });
        }
        Fault::Duplicate => {
            send(packet.clone()).await;
            send(packet).await;
        }
    }
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
