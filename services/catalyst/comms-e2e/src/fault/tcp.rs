//! The TCP relay: one upstream connection per client connection. A stall stops moving bytes and
//! closes nothing; a half-open connection stays that way for good; a cut closes both sides; an
//! orphaned connection is closed towards the client and left open towards the stack.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use anyhow::{Context, Result};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio::task::AbortHandle;

use super::{Direction, Fault, Shared, ending};

pub(super) async fn start(
    shared: Arc<Shared>,
    listen: SocketAddr,
    upstream: SocketAddr,
) -> Result<(SocketAddr, Vec<AbortHandle>)> {
    // a port that a link dropped a moment ago may still be on its way out
    let mut attempt = 0;
    let listener = loop {
        match TcpListener::bind(listen).await {
            Ok(listener) => break listener,
            Err(error) if error.kind() == std::io::ErrorKind::AddrInUse && attempt < 100 => {
                attempt += 1;
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("the fault link cannot listen on TCP {listen}"));
            }
        }
    };
    let listen = listener.local_addr()?;
    let task = tokio::spawn(accept(shared, listener, upstream));
    Ok((listen, vec![task.abort_handle()]))
}

async fn accept(shared: Arc<Shared>, listener: TcpListener, upstream: SocketAddr) {
    // `accept` can keep failing (no file handles left): back off instead of spinning
    let mut backoff = std::time::Duration::from_millis(10);
    loop {
        let Ok((client, peer)) = listener.accept().await else {
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(std::time::Duration::from_secs(1));
            continue;
        };
        backoff = std::time::Duration::from_millis(10);
        let shared = shared.clone();
        tokio::spawn(async move {
            let generation = shared.generation.load(Ordering::SeqCst);
            shared.flows.fetch_add(1, Ordering::Relaxed);
            shared.record(
                "connection",
                serde_json::json!({ "client": peer.to_string() }),
            );
            let Ok(server) = TcpStream::connect(upstream).await else {
                shared.record(
                    "connection failed",
                    format!("nothing answers at {upstream}"),
                );
                return;
            };
            let _ = client.set_nodelay(true);
            let _ = server.set_nodelay(true);
            let (client_read, client_write) = client.into_split();
            let (server_read, server_write) = server.into_split();
            // one pump ending tells the other, which then lets go of its halves too
            let (over, _) = watch::channel(false);
            let up = pump(
                shared.clone(),
                Direction::Uplink,
                client_read,
                server_write,
                generation,
                over.clone(),
            );
            let down = pump(
                shared.clone(),
                Direction::Downlink,
                server_read,
                client_write,
                generation,
                over.clone(),
            );
            tokio::join!(up, down);
            shared.record(
                "connection ended",
                serde_json::json!({ "client": peer.to_string() }),
            );
        });
    }
}

/// What a pump does next, given the link's state.
enum Gate {
    Open(Fault),
    /// Stalled or half-open: wait for a change.
    Wait,
    /// Cut: end the connection on both sides.
    Close,
    /// Orphan: let the client side go, keep the stack side open and silent.
    Orphan,
}

fn gate(shared: &Shared, direction: Direction, generation: u64) -> Gate {
    // a dropped link ends everything, a half-open connection included
    if shared.closed.load(Ordering::SeqCst) {
        return Gate::Close;
    }
    if let Some(how) = shared.fate(generation) {
        return match how {
            ending::CUT => Gate::Close,
            ending::ORPHAN => Gate::Orphan,
            _ => Gate::Wait,
        };
    }
    match shared.fault(direction) {
        Fault::Stall => Gate::Wait,
        fault => Gate::Open(fault),
    }
}

/// Moves bytes one way until the connection ends. On an orphan it keeps its stack-side half
/// alive in the link and drops its client-side half.
async fn pump<R, W>(
    shared: Arc<Shared>,
    direction: Direction,
    mut from: R,
    mut to: W,
    generation: u64,
    over: watch::Sender<bool>,
) where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let mut ended = over.subscribe();
    let mut buffer = vec![0u8; 16 * 1024];
    let orphaned = loop {
        // Register for a change before looking, so one that lands in between is not missed.
        let changed = shared.changed.notified();
        tokio::pin!(changed);
        changed.as_mut().enable();
        if *ended.borrow() {
            break false;
        }
        let fault = match gate(&shared, direction, generation) {
            Gate::Close => break false,
            Gate::Orphan => break true,
            Gate::Wait => {
                tokio::select! {
                    _ = &mut changed => {}
                    _ = ended.changed() => {}
                }
                continue;
            }
            Gate::Open(fault) => fault,
        };
        let length = tokio::select! {
            read = from.read(&mut buffer) => match read {
                Ok(0) | Err(_) => break false,
                Ok(length) => length,
            },
            // a fault set while this side was quiet: look again before reading
            _ = &mut changed => continue,
            _ = ended.changed() => continue,
        };
        shared.count(direction, length);
        // The bytes are read; a stall or half-open that began meanwhile must still hold them.
        let verdict = loop {
            let changed = shared.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if *ended.borrow() {
                break Some(false);
            }
            match gate(&shared, direction, generation) {
                Gate::Close => break Some(false),
                Gate::Orphan => break Some(true),
                Gate::Wait => {
                    tokio::select! {
                        _ = &mut changed => {}
                        _ = ended.changed() => {}
                    }
                }
                Gate::Open(_) => break None,
            }
        };
        if let Some(orphaned) = verdict {
            break orphaned;
        }
        if let Fault::Delay { by } = fault
            && !shared.sleep_unless_closed(by).await
        {
            break false;
        }
        if to.write_all(&buffer[..length]).await.is_err() {
            break false;
        }
    };
    if orphaned {
        // the stack-side half stays open in the link; the client-side half closes here
        let mut parked = shared.parked.lock();
        match direction {
            Direction::Uplink => parked.push(Box::new(to)),
            _ => parked.push(Box::new(from)),
        }
        return;
    }
    let _ = over.send(true);
}
