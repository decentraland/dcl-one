//! The UDP relay: one upstream socket per client flow, bound to the client's own IP address, so
//! the stack sees one flow per actor and counts its per-address limits as it would without the
//! link.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use anyhow::{Context, Result};
use tokio::net::UdpSocket;
use tokio::task::AbortHandle;

use super::{Direction, Fault, Shared};

const MAX_DATAGRAM: usize = 65_535;

struct Flow {
    upstream: Arc<UdpSocket>,
    generation: u64,
    pump: AbortHandle,
}

pub(super) async fn start(
    shared: Arc<Shared>,
    listen: SocketAddr,
    upstream: SocketAddr,
) -> Result<(SocketAddr, Vec<AbortHandle>)> {
    let socket = Arc::new(
        UdpSocket::bind(listen)
            .await
            .with_context(|| format!("the fault link cannot listen on UDP {listen}"))?,
    );
    let listen = socket.local_addr()?;
    let task = tokio::spawn(relay(shared, socket, upstream));
    Ok((listen, vec![task.abort_handle()]))
}

async fn relay(shared: Arc<Shared>, listen: Arc<UdpSocket>, upstream: SocketAddr) {
    let mut flows: HashMap<SocketAddr, Flow> = HashMap::new();
    let mut buffer = vec![0u8; MAX_DATAGRAM];
    loop {
        let (length, client) = match listen.recv_from(&mut buffer).await {
            Ok(received) => received,
            // Windows reports an earlier send to a closed port here; it is not the end of the relay.
            Err(_) => continue,
        };
        let generation = shared.generation.load(Ordering::SeqCst);
        let stale = flows.get(&client).is_some_and(|flow| flow.generation != generation);
        if stale {
            if shared.closing.load(Ordering::SeqCst) {
                // cut: the next packet starts a new flow, from a new upstream port
                if let Some(flow) = flows.remove(&client) {
                    flow.pump.abort();
                }
            } else {
                // half-open: this flow is silent for good
                shared.dropped.fetch_add(1, Ordering::Relaxed);
                continue;
            }
        }
        if !flows.contains_key(&client) {
            let Ok(socket) = UdpSocket::bind((client.ip(), 0)).await else {
                shared.record("flow failed", format!("cannot bind an upstream socket on {}", client.ip()));
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
    listen: Arc<UdpSocket>,
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
        forward(&shared, Direction::Downlink, packet, move |packet| {
            let listen = listen.clone();
            async move {
                let _ = listen.send_to(&packet, client).await;
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
