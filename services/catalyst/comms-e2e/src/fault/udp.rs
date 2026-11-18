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
            received = socket.recv_from(&mut buffer) => match received {
                Ok(received) => received,
                // Windows reports an earlier send to a closed port here; the relay goes on.
                Err(_) => continue,
            },
            _ = &mut changed => continue,
        };
        drop(socket);

        let generation = shared.generation.load(Ordering::SeqCst);
        let stale = flows
            .get(&client)
            .is_some_and(|flow| flow.generation != generation);
        if stale {
            if shared.ending.load(Ordering::SeqCst) == ending::HALF_OPEN {
                // this flow is silent for good
                shared.dropped.fetch_add(1, Ordering::Relaxed);
                continue;
            }
            // cut: the next packet starts a new flow, from a new upstream port
            flows.remove(&client);
        }
        if !flows.contains_key(&client) {
            let Ok(socket) = UdpSocket::bind((client.ip(), 0)).await else {
                shared.record(
                    "flow failed",
                    format!("cannot bind an upstream socket on {}", client.ip()),
                );
                continue;
            };
            if socket.connect(upstream).await.is_err() {
                continue;
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
            flows.insert(
                client,
                Flow {
                    upstream: socket,
                    generation,
                    pump: pump.abort_handle(),
                },
            );
        }
        let flow = &flows[&client];
        let packet = buffer[..length].to_vec();
        let target = flow.upstream.clone();
        forward(&shared, Direction::Uplink, packet, move |packet| {
            let target = target.clone();
            async move {
                let _ = target.send(&packet).await;
            }
        })
        .await;
    }
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
        if shared.generation.load(Ordering::SeqCst) != generation {
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
            if shared.chance() < share {
                shared.dropped.fetch_add(1, Ordering::Relaxed);
            } else {
                send(packet).await;
            }
        }
        Fault::Delay { by } => {
            tokio::spawn(async move {
                tokio::time::sleep(by).await;
                send(packet).await;
            });
        }
        Fault::Reorder { window } => {
            let wait = shared.gap(window);
            tokio::spawn(async move {
                tokio::time::sleep(wait).await;
                send(packet).await;
            });
        }
        Fault::Duplicate => {
            send(packet.clone()).await;
            send(packet).await;
        }
    }
}
