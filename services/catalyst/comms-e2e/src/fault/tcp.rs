//! The TCP relay: one upstream connection per client connection. A stall stops moving bytes and
//! closes nothing; a half-open connection stays that way for good; a cut closes both sides.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use anyhow::{Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::AbortHandle;

use super::{Direction, Fault, Shared};

pub(super) async fn start(
    shared: Arc<Shared>,
    listen: SocketAddr,
    upstream: SocketAddr,
) -> Result<(SocketAddr, Vec<AbortHandle>)> {
    let listener = TcpListener::bind(listen)
        .await
        .with_context(|| format!("the fault link cannot listen on TCP {listen}"))?;
    let listen = listener.local_addr()?;
    let task = tokio::spawn(accept(shared, listener, upstream));
    Ok((listen, vec![task.abort_handle()]))
}

async fn accept(shared: Arc<Shared>, listener: TcpListener, upstream: SocketAddr) {
    loop {
        let Ok((client, peer)) = listener.accept().await else {
            continue;
        };
        let shared = shared.clone();
        tokio::spawn(async move {
            let generation = shared.generation.load(Ordering::SeqCst);
            shared.flows.fetch_add(1, Ordering::Relaxed);
            shared.record("connection", serde_json::json!({ "client": peer.to_string() }));
            let Ok(server) = TcpStream::connect(upstream).await else {
                shared.record("connection failed", format!("nothing answers at {upstream}"));
                return;
            };
            let _ = client.set_nodelay(true);
            let _ = server.set_nodelay(true);
            let (client_read, client_write) = client.into_split();
            let (server_read, server_write) = server.into_split();
            let up = pump(shared.clone(), Direction::Uplink, client_read, server_write, generation);
            let down = pump(shared.clone(), Direction::Downlink, server_read, client_write, generation);
            // either side ending ends the pair, and with it both sockets
            tokio::select! {
                _ = up => {}
                _ = down => {}
            }
            shared.record("connection ended", serde_json::json!({ "client": peer.to_string() }));
        });
    }
}

/// What a pump does next, given the link's state.
enum Gate {
    Open(Fault),
    /// Stalled or half-open: wait for a change.
    Wait,
    /// Cut: end the connection.
    Close,
}

fn gate(shared: &Shared, direction: Direction, generation: u64) -> Gate {
    if shared.generation.load(Ordering::SeqCst) != generation {
        return if shared.closing.load(Ordering::SeqCst) {
            Gate::Close
        } else {
            Gate::Wait
        };
    }
    match shared.fault(direction) {
        Fault::Stall => Gate::Wait,
        fault => Gate::Open(fault),
    }
}

async fn pump(
    shared: Arc<Shared>,
    direction: Direction,
    mut from: OwnedReadHalf,
    mut to: OwnedWriteHalf,
    generation: u64,
) {
    let mut buffer = vec![0u8; 16 * 1024];
    loop {
        // Register for a change before looking, so one that lands in between is not missed.
        let changed = shared.changed.notified();
        tokio::pin!(changed);
        changed.as_mut().enable();
        let fault = match gate(&shared, direction, generation) {
            Gate::Close => return,
            Gate::Wait => {
                changed.await;
                continue;
            }
            Gate::Open(fault) => fault,
        };
        let length = tokio::select! {
            read = from.read(&mut buffer) => match read {
                Ok(0) | Err(_) => return,
                Ok(length) => length,
            },
            // a fault set while this side was quiet: look again before reading
            _ = &mut changed => continue,
        };
        shared.count(direction, length);
        // The bytes are read; a stall or half-open that began meanwhile must still hold them.
        loop {
            let changed = shared.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            match gate(&shared, direction, generation) {
                Gate::Close => return,
                Gate::Wait => changed.await,
                Gate::Open(_) => break,
            }
        }
        if let Fault::Delay { by } = fault {
            tokio::time::sleep(by).await;
        }
        if to.write_all(&buffer[..length]).await.is_err() {
            return;
        }
    }
}
