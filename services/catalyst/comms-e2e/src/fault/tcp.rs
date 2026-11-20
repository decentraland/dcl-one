//! The TCP relay: one upstream connection per client connection. A stall stops moving bytes and
//! closes nothing; a half-open connection stays that way for good; a cut closes both sides; an
//! orphaned connection is closed towards the client and left open towards the stack. A delay
//! holds what was read for the delay while the relay goes on reading, so everything arrives
//! later by the delay, in order, and a transfer does not wait out the delay once for every read.
//! A connection ends when either side ends it; what a delay still holds for the other side is
//! written first.

use std::collections::VecDeque;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio::task::AbortHandle;
use tokio::time::Instant;

use super::{Direction, Fault, Shared, ending};

/// How much one direction of a connection holds while a delay is on. Past it the relay stops
/// reading until some of it is written, as a full socket buffer would.
const HELD_LIMIT: usize = 4 * 1024 * 1024;

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
                tokio::time::sleep(Duration::from_millis(20)).await;
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
    let mut backoff = Duration::from_millis(10);
    loop {
        let Ok((client, peer)) = listener.accept().await else {
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(Duration::from_secs(1));
            continue;
        };
        backoff = Duration::from_millis(10);
        tokio::spawn(admit(shared.clone(), client, peer, upstream));
    }
}

/// Takes a connection the listener accepted and relays it until it ends. The connection belongs
/// to the generation current at this call, not to the one current when its task first runs: a
/// `cut`, `half_open` or `orphan` that lands in between ends it like every other connection that
/// existed then. A connection the operating system has completed and `accept` has not returned
/// yet is not one the link knows: it begins in the generation current when it is taken.
fn admit(
    shared: Arc<Shared>,
    client: TcpStream,
    peer: SocketAddr,
    upstream: SocketAddr,
) -> impl Future<Output = ()> + Send + 'static {
    let generation = shared.generation.load(Ordering::SeqCst);
    relay(shared, client, peer, upstream, generation)
}

async fn relay(
    shared: Arc<Shared>,
    client: TcpStream,
    peer: SocketAddr,
    upstream: SocketAddr,
    generation: u64,
) {
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
}

/// What a pump does next, given the link's state.
enum Gate {
    /// Bytes may move.
    Open,
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
        // the bytes of a queue wait in the sockets, as on a stall
        Fault::Stall | Fault::Queue { .. } => Gate::Wait,
        _ => Gate::Open,
    }
}

/// Bytes a pump has read and not written yet.
struct Held {
    /// When they may be written: when they were read, plus the delay that was on then.
    due: Instant,
    bytes: Vec<u8>,
}

/// Moves bytes one way until the connection ends. What it reads waits in a queue until it is
/// due, and is written in the order it was read. On an orphan it keeps its stack-side half
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
    let mut held: VecDeque<Held> = VecDeque::new();
    let mut held_bytes = 0;
    // nothing more is read (the source ended, or the other direction did): what is held is
    // still written, then this direction ends
    let mut done_reading = false;
    let orphaned = loop {
        // Register for a change before looking, so one that lands in between is not missed.
        let changed = shared.changed.notified();
        tokio::pin!(changed);
        changed.as_mut().enable();
        let other_ended = *ended.borrow();
        if other_ended {
            done_reading = true;
        }
        // Past a stall or a half-open nothing is read and nothing is written: what is held waits,
        // unless the other direction is over, which ends this one too.
        match gate(&shared, direction, generation) {
            Gate::Close => break false,
            Gate::Orphan => break true,
            Gate::Wait if other_ended => break false,
            Gate::Wait => {
                tokio::select! {
                    _ = &mut changed => {}
                    _ = ended.changed() => {}
                }
                continue;
            }
            Gate::Open => {}
        };
        // What is due goes first.
        let next_due = held.front().map(|chunk| chunk.due);
        if next_due.is_some_and(|due| due <= Instant::now()) {
            let chunk = held.pop_front().expect("a chunk is due");
            held_bytes -= chunk.bytes.len();
            if to.write_all(&chunk.bytes).await.is_err() {
                break false;
            }
            continue;
        }
        if done_reading && held.is_empty() {
            break false;
        }
        let until_due = async {
            match next_due {
                Some(due) => tokio::time::sleep_until(due).await,
                None => std::future::pending().await,
            }
        };
        let read = tokio::select! {
            read = from.read(&mut buffer), if !done_reading && held_bytes < HELD_LIMIT => {
                Some(read)
            }
            _ = until_due => None,
            // a fault set while this side was quiet: look again
            _ = &mut changed => None,
            _ = ended.changed() => None,
        };
        match read {
            Some(Ok(length)) if length > 0 => {
                shared.count(direction, length);
                // The fault that is on now, not the one this pass began with: a fault set while
                // the pump waited may not have woken it before the bytes did.
                let delay = match shared.fault(direction) {
                    Fault::Delay { by } => by,
                    Fault::Link { delay, .. } => delay,
                    _ => Duration::ZERO,
                };
                held.push_back(Held {
                    due: Instant::now() + delay,
                    bytes: buffer[..length].to_vec(),
                });
                held_bytes += length;
            }
            Some(_) => done_reading = true,
            None => {}
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Ctx;
    use crate::fault::FaultLink;
    use crate::stack::Stack;

    fn shared() -> Arc<Shared> {
        let ctx = Ctx::new(
            Arc::new(Stack::pulse_only("127.0.0.1:9".parse().unwrap())),
            "unit",
            "tcp-relay",
            0,
            1,
            std::env::temp_dir().join("comms-e2e-unit"),
        );
        FaultLink::shared(&ctx, "link")
    }

    #[tokio::test]
    async fn what_a_delay_holds_is_still_written_when_the_other_direction_ends() {
        let shared = shared();
        shared.faults.lock().down = Fault::Delay {
            by: Duration::from_millis(200),
        };
        let (over, _) = watch::channel(false);
        let (mut stack_side, from) = tokio::io::duplex(1024);
        let (to, mut client_side) = tokio::io::duplex(1024);
        let pumping = tokio::spawn(pump(
            shared.clone(),
            Direction::Downlink,
            from,
            to,
            0,
            over.clone(),
        ));
        stack_side.write_all(b"reply").await.unwrap();
        // the pump has read the reply once it has counted it
        let read_by = Instant::now() + Duration::from_secs(2);
        while shared.down_bytes.load(Ordering::Relaxed) < 5 {
            assert!(Instant::now() < read_by, "the pump did not read the reply");
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        // the other direction ends while the reply is held for its delay
        over.send_replace(true);
        let mut written = Vec::new();
        tokio::time::timeout(
            Duration::from_secs(2),
            client_side.read_to_end(&mut written),
        )
        .await
        .expect("the pump did not end")
        .unwrap();
        assert_eq!(written, b"reply");
        pumping.await.unwrap();
    }

    #[tokio::test]
    async fn bytes_that_arrive_once_a_delay_is_set_take_that_delay() {
        // The bytes and the wake-up for the new fault can both be ready when the pump next
        // runs. Whichever it takes first, bytes that arrive under a delay are delayed.
        let by = Duration::from_millis(60);
        for round in 0..24 {
            let shared = shared();
            let (over, _) = watch::channel(false);
            let (mut stack_side, from) = tokio::io::duplex(1024);
            let (to, mut client_side) = tokio::io::duplex(1024);
            let pumping = tokio::spawn(pump(
                shared.clone(),
                Direction::Downlink,
                from,
                to,
                0,
                over.clone(),
            ));
            // the pump reaches its wait on a plain link
            tokio::time::sleep(Duration::from_millis(5)).await;
            shared.faults.lock().down = Fault::Delay { by };
            shared.changed.notify_waiters();
            let sent = Instant::now();
            stack_side.write_all(b"late").await.unwrap();
            let mut bytes = [0u8; 4];
            client_side.read_exact(&mut bytes).await.unwrap();
            assert!(
                sent.elapsed() >= by,
                "round {round}: bytes sent under a delay arrived after {:?}",
                sent.elapsed()
            );
            over.send_replace(true);
            pumping.await.unwrap();
        }
    }

    #[tokio::test]
    async fn a_connection_accepted_before_a_cut_is_cut_though_its_task_runs_after() {
        let shared = shared();
        let stack_side = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap();
        let (accepted, peer) = listener.accept().await.unwrap();
        let connection = admit(
            shared.clone(),
            accepted,
            peer,
            stack_side.local_addr().unwrap(),
        );
        // the cut lands after the accept and before the connection's task first runs
        shared.end_generation(ending::CUT);
        shared.changed.notify_waiters();
        tokio::spawn(connection);
        let mut byte = [0u8; 1];
        let read = tokio::time::timeout(Duration::from_secs(5), client.read(&mut byte))
            .await
            .expect("the connection outlived the cut");
        assert!(matches!(read, Ok(0) | Err(_)), "{read:?}");
    }
}
