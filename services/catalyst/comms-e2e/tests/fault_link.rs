//! The fault link on its own, and a Pulse bot through it.
#![allow(clippy::unwrap_used, reason = "test support")]
#![allow(
    clippy::cast_precision_loss,
    reason = "test fixtures use small literal values"
)]

mod support;

use std::net::SocketAddr;
use std::time::Duration;

use catalyst_comms_e2e::engine::{Ctx, expect_or};
use catalyst_comms_e2e::fault::{Direction, Fate, Fault, FaultLink, windows};
use catalyst_comms_e2e::pulse::{ConnectOptions, HandshakeOptions, HandshakeOutcome, PulseBot};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};

const ANY: &str = "127.0.0.1:0";
const SOON: Duration = Duration::from_secs(5);
const QUIET: Duration = Duration::from_millis(300);

fn ctx(name: &str) -> Ctx {
    support::ctx("127.0.0.1:9".parse().unwrap(), name)
}

/// A UDP server that answers every datagram with `echo:` + the datagram, and says who sent it.
async fn udp_echo() -> (SocketAddr, tokio::sync::mpsc::UnboundedReceiver<SocketAddr>) {
    let socket = UdpSocket::bind(ANY).await.unwrap();
    let address = socket.local_addr().unwrap();
    let (senders, seen) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        let mut buffer = [0u8; 2048];
        loop {
            let Ok((length, from)) = socket.recv_from(&mut buffer).await else {
                continue;
            };
            let _ = senders.send(from);
            let mut reply = b"echo:".to_vec();
            reply.extend_from_slice(&buffer[..length]);
            let _ = socket.send_to(&reply, from).await;
        }
    });
    (address, seen)
}

async fn receive(socket: &UdpSocket, within: Duration) -> Option<Vec<u8>> {
    let mut buffer = [0u8; 2048];
    match tokio::time::timeout(within, socket.recv(&mut buffer)).await {
        Ok(Ok(length)) => Some(buffer[..length].to_vec()),
        _ => None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn udp_passes_both_ways_and_a_stall_stops_one_direction() {
    let ctx = ctx("udp");
    let (server, mut seen) = udp_echo().await;
    let link = FaultLink::udp(&ctx, "link", ANY.parse().unwrap(), server)
        .await
        .unwrap();
    let client = UdpSocket::bind(ANY).await.unwrap();
    client.connect(link.address()).await.unwrap();

    client.send(b"one").await.unwrap();
    assert_eq!(
        receive(&client, SOON).await.as_deref(),
        Some(&b"echo:one"[..])
    );
    let first_source = seen.recv().await.unwrap();

    // uplink stalled: the server hears nothing, so nothing comes back
    link.set(Direction::Uplink, Fault::Stall);
    client.send(b"two").await.unwrap();
    assert_eq!(receive(&client, QUIET).await, None);
    assert!(
        seen.try_recv().is_err(),
        "the server heard a stalled packet"
    );
    assert_eq!(link.counters().dropped, 1);

    // downlink stalled: the server hears it, the answer is lost
    link.set(Direction::Uplink, Fault::Pass);
    link.set(Direction::Downlink, Fault::Stall);
    client.send(b"three").await.unwrap();
    assert_eq!(
        seen.recv().await.unwrap(),
        first_source,
        "one flow, one source port"
    );
    assert_eq!(receive(&client, QUIET).await, None);

    // restored: the stalled packets are gone for good, new ones pass
    link.restore();
    client.send(b"four").await.unwrap();
    assert_eq!(
        receive(&client, SOON).await.as_deref(),
        Some(&b"echo:four"[..])
    );
    assert_eq!(link.counters().flows, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn udp_half_open_silences_the_old_socket_and_cut_moves_the_flow_to_a_new_port() {
    let ctx = ctx("udp-flows");
    let (server, mut seen) = udp_echo().await;
    let link = FaultLink::udp(&ctx, "link", ANY.parse().unwrap(), server)
        .await
        .unwrap();
    let old = UdpSocket::bind(ANY).await.unwrap();
    old.connect(link.address()).await.unwrap();
    old.send(b"hello").await.unwrap();
    assert!(receive(&old, SOON).await.is_some());
    let old_source = seen.recv().await.unwrap();

    link.half_open();
    old.send(b"anyone?").await.unwrap();
    assert_eq!(
        receive(&old, QUIET).await,
        None,
        "a half-open flow is silent"
    );
    assert!(seen.try_recv().is_err());
    // restoring the link does not revive it
    link.restore();
    old.send(b"still there?").await.unwrap();
    assert_eq!(receive(&old, QUIET).await, None);

    // a new socket of the same client is a new flow and passes
    let new = UdpSocket::bind(ANY).await.unwrap();
    new.connect(link.address()).await.unwrap();
    new.send(b"again").await.unwrap();
    assert_eq!(
        receive(&new, SOON).await.as_deref(),
        Some(&b"echo:again"[..])
    );
    let new_source = seen.recv().await.unwrap();
    assert_ne!(new_source, old_source);

    // cut: the same client socket continues from another upstream port
    link.cut();
    new.send(b"after the cut").await.unwrap();
    assert_eq!(
        receive(&new, SOON).await.as_deref(),
        Some(&b"echo:after the cut"[..])
    );
    assert_ne!(seen.recv().await.unwrap(), new_source);
}

/// A client socket of the link, and the proof that the link serves it.
async fn udp_client(link: &FaultLink) -> UdpSocket {
    let client = UdpSocket::bind(ANY).await.unwrap();
    client.connect(link.address()).await.unwrap();
    client.send(b"hi").await.unwrap();
    assert_eq!(
        receive(&client, SOON).await.as_deref(),
        Some(&b"echo:hi"[..])
    );
    client
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn udp_half_open_then_cut_leaves_the_half_open_flow_silent() {
    let ctx = ctx("udp-half-then-cut");
    let (server, mut seen) = udp_echo().await;
    let link = FaultLink::udp(&ctx, "link", ANY.parse().unwrap(), server)
        .await
        .unwrap();
    let first = udp_client(&link).await;
    seen.recv().await.unwrap();
    link.half_open();
    let between = udp_client(&link).await;
    let between_source = seen.recv().await.unwrap();
    link.cut();

    // the half-open flow is not revived by the cut
    for _ in 0..3 {
        first.send(b"anyone?").await.unwrap();
        assert_eq!(
            receive(&first, QUIET).await,
            None,
            "a half-open flow is silent for good"
        );
    }
    assert!(
        seen.try_recv().is_err(),
        "the server heard the half-open flow"
    );
    // the flow made between the two is cut: it continues from another upstream port
    between.send(b"after").await.unwrap();
    assert_eq!(
        receive(&between, SOON).await.as_deref(),
        Some(&b"echo:after"[..])
    );
    assert_ne!(seen.recv().await.unwrap(), between_source);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn udp_cut_then_half_open_leaves_the_cut_flow_to_start_again() {
    let ctx = ctx("udp-cut-then-half");
    let (server, mut seen) = udp_echo().await;
    let link = FaultLink::udp(&ctx, "link", ANY.parse().unwrap(), server)
        .await
        .unwrap();
    let cut = udp_client(&link).await;
    let cut_source = seen.recv().await.unwrap();
    link.cut();
    let silenced = udp_client(&link).await;
    seen.recv().await.unwrap();
    link.half_open();

    // the cut flow starts a new flow with its next packet; the half-open one is silent for good
    cut.send(b"again").await.unwrap();
    assert_eq!(
        receive(&cut, SOON).await.as_deref(),
        Some(&b"echo:again"[..])
    );
    assert_ne!(seen.recv().await.unwrap(), cut_source);
    silenced.send(b"anyone?").await.unwrap();
    assert_eq!(receive(&silenced, QUIET).await, None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_delayed_packet_does_not_hold_the_port_after_the_link_is_dropped() {
    let ctx = ctx("udp-delay-lifetime");
    let (server, _seen) = udp_echo().await;
    let link = FaultLink::udp(&ctx, "link", ANY.parse().unwrap(), server)
        .await
        .unwrap();
    let client = udp_client(&link).await;
    // the answer is held for far longer than this test runs
    link.set(
        Direction::Downlink,
        Fault::Delay {
            by: Duration::from_secs(60),
        },
    );
    client.send(b"slow").await.unwrap();
    assert_eq!(receive(&client, QUIET).await, None);
    let address = link.address();
    drop(link);
    // the port is free again at once, not when the delay is over
    let mut bound = None;
    for _ in 0..50 {
        match std::net::UdpSocket::bind(address) {
            Ok(socket) => {
                bound = Some(socket);
                break;
            }
            Err(_) => tokio::time::sleep(Duration::from_millis(20)).await,
        }
    }
    assert!(
        bound.is_some(),
        "the delayed packet still holds the listen socket"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn udp_faults_drop_delay_and_duplicate() {
    let ctx = ctx("udp-faults");
    let (server, _seen) = udp_echo().await;
    let link = FaultLink::udp(&ctx, "link", ANY.parse().unwrap(), server)
        .await
        .unwrap();
    let client = UdpSocket::bind(ANY).await.unwrap();
    client.connect(link.address()).await.unwrap();

    link.set(Direction::Uplink, Fault::Drop { share: 1.0 });
    client.send(b"lost").await.unwrap();
    assert_eq!(receive(&client, QUIET).await, None);

    link.set(
        Direction::Uplink,
        Fault::Delay {
            by: Duration::from_millis(250),
        },
    );
    let started = std::time::Instant::now();
    client.send(b"late").await.unwrap();
    assert_eq!(
        receive(&client, SOON).await.as_deref(),
        Some(&b"echo:late"[..])
    );
    assert!(
        started.elapsed() >= Duration::from_millis(240),
        "{:?}",
        started.elapsed()
    );

    link.set(Direction::Uplink, Fault::Duplicate);
    client.send(b"twice").await.unwrap();
    assert_eq!(
        receive(&client, SOON).await.as_deref(),
        Some(&b"echo:twice"[..])
    );
    assert_eq!(
        receive(&client, SOON).await.as_deref(),
        Some(&b"echo:twice"[..])
    );
}

/// A TCP server that echoes, and reports each connection it accepts and each one that ends.
async fn tcp_echo() -> (
    SocketAddr,
    tokio::sync::mpsc::UnboundedReceiver<&'static str>,
) {
    let listener = TcpListener::bind(ANY).await.unwrap();
    let address = listener.local_addr().unwrap();
    let (events, seen) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                continue;
            };
            let events = events.clone();
            let _ = events.send("accepted");
            tokio::spawn(async move {
                let mut buffer = [0u8; 1024];
                loop {
                    match stream.read(&mut buffer).await {
                        Ok(0) | Err(_) => break,
                        Ok(length) => {
                            let _ = events.send("read");
                            if stream.write_all(&buffer[..length]).await.is_err() {
                                break;
                            }
                        }
                    }
                }
                let _ = events.send("ended");
            });
        }
    });
    (address, seen)
}

async fn read_some(stream: &mut TcpStream, within: Duration) -> Option<Vec<u8>> {
    let mut buffer = [0u8; 1024];
    match tokio::time::timeout(within, stream.read(&mut buffer)).await {
        Ok(Ok(length)) => Some(buffer[..length].to_vec()),
        _ => None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tcp_passes_and_a_stall_holds_the_bytes_until_restore() {
    let ctx = ctx("tcp");
    let (server, mut seen) = tcp_echo().await;
    let link = FaultLink::tcp(&ctx, "link", ANY.parse().unwrap(), server)
        .await
        .unwrap();
    let mut client = TcpStream::connect(link.address()).await.unwrap();
    client.write_all(b"one").await.unwrap();
    assert_eq!(
        read_some(&mut client, SOON).await.as_deref(),
        Some(&b"one"[..])
    );
    assert_eq!(seen.recv().await, Some("accepted"));
    assert_eq!(seen.recv().await, Some("read"));

    link.set(Direction::Both, Fault::Stall);
    client.write_all(b"two").await.unwrap();
    assert_eq!(read_some(&mut client, QUIET).await, None);
    assert!(seen.try_recv().is_err(), "the server read during a stall");

    // unlike UDP, the stream is intact after the stall
    link.restore();
    assert_eq!(
        read_some(&mut client, SOON).await.as_deref(),
        Some(&b"two"[..])
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_tcp_delay_makes_everything_late_by_the_delay_and_no_slower() {
    let ctx = ctx("tcp-delay");
    let (server, _seen) = tcp_echo().await;
    let link = FaultLink::tcp(&ctx, "link", ANY.parse().unwrap(), server)
        .await
        .unwrap();
    let by = Duration::from_millis(300);
    link.set(Direction::Uplink, Fault::Delay { by });

    // far more than the relay reads at once: a relay that waited out the delay after every read
    // would take it dozens of times over
    let sent: Vec<u8> = (0..512 * 1024u32)
        .map(|index| (index % 251) as u8)
        .collect();
    let stream = TcpStream::connect(link.address()).await.unwrap();
    let (mut reading, mut writing) = stream.into_split();
    let started = std::time::Instant::now();
    let outgoing = sent.clone();
    let writer = tokio::spawn(async move {
        writing.write_all(&outgoing).await.unwrap();
        // the connection stays open until the echo has been read
        writing
    });
    let mut echoed = vec![0u8; sent.len()];
    tokio::time::timeout(SOON, reading.read_exact(&mut echoed[..1]))
        .await
        .expect("nothing came back")
        .unwrap();
    let first_after = started.elapsed();
    tokio::time::timeout(SOON, reading.read_exact(&mut echoed[1..]))
        .await
        .expect("the rest did not follow: the delay was taken more than once")
        .unwrap();
    let all_after = started.elapsed();
    let _writing = writer.await.unwrap();
    assert!(
        first_after >= by,
        "the first byte was back after {first_after:?}, before the delay"
    );
    assert!(
        all_after < by + Duration::from_secs(3),
        "everything was back only after {all_after:?}"
    );
    assert!(
        echoed == sent,
        "the bytes came back changed or out of order"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn half_open_keeps_the_tcp_socket_open_and_silent() {
    let ctx = ctx("tcp-half-open");
    let (server, mut seen) = tcp_echo().await;
    let link = FaultLink::tcp(&ctx, "link", ANY.parse().unwrap(), server)
        .await
        .unwrap();
    let mut stale = TcpStream::connect(link.address()).await.unwrap();
    stale.write_all(b"hello").await.unwrap();
    assert!(read_some(&mut stale, SOON).await.is_some());
    assert_eq!(seen.recv().await, Some("accepted"));
    assert_eq!(seen.recv().await, Some("read"));

    link.half_open();
    // the write succeeds, nothing reaches the server, and neither side sees a close
    stale.write_all(b"anyone?").await.unwrap();
    assert_eq!(
        read_some(&mut stale, QUIET).await,
        None,
        "no data and no close"
    );
    assert!(
        seen.try_recv().is_err(),
        "the server saw the stale socket move or end"
    );
    link.restore();
    assert_eq!(
        read_some(&mut stale, QUIET).await,
        None,
        "a stale socket does not come back"
    );

    // the same client connects again and is served
    let mut fresh = TcpStream::connect(link.address()).await.unwrap();
    fresh.write_all(b"again").await.unwrap();
    assert_eq!(
        read_some(&mut fresh, SOON).await.as_deref(),
        Some(&b"again"[..])
    );
    assert_eq!(seen.recv().await, Some("accepted"));
    assert_eq!(seen.recv().await, Some("read"));

    // a cut ends the fresh connection on both sides; the half-open one was ended by the half-open
    // and stays silent and open
    link.cut();
    assert_eq!(
        read_some(&mut fresh, SOON).await.as_deref(),
        Some(&b""[..]),
        "the client sees the end"
    );
    expect_events(&mut seen, "ended", 1).await;
    assert_eq!(
        read_some(&mut stale, QUIET).await,
        None,
        "the half-open socket is still silent and not closed"
    );
    assert_no_event(&mut seen, "ended").await;
    // the link going away releases it
    drop(link);
    expect_events(&mut seen, "ended", 1).await;
}

/// Waits until the server has reported `count` events called `kind`; others are skipped.
async fn expect_events(
    seen: &mut tokio::sync::mpsc::UnboundedReceiver<&'static str>,
    kind: &str,
    count: usize,
) {
    let mut found = 0;
    while found < count {
        match tokio::time::timeout(SOON, seen.recv()).await {
            Ok(Some(event)) if event == kind => found += 1,
            Ok(Some(_)) => {}
            other => panic!("the server reported {found} of {count} `{kind}`: {other:?}"),
        }
    }
}

/// Checks that no event called `kind` arrives for a while.
async fn assert_no_event(
    seen: &mut tokio::sync::mpsc::UnboundedReceiver<&'static str>,
    kind: &str,
) {
    let deadline = tokio::time::Instant::now() + QUIET;
    while let Ok(Some(event)) = tokio::time::timeout_at(deadline, seen.recv()).await {
        assert_ne!(event, kind, "the server saw a connection end");
    }
}

async fn tcp_link(
    name: &str,
) -> (
    Ctx,
    FaultLink,
    tokio::sync::mpsc::UnboundedReceiver<&'static str>,
) {
    let ctx = ctx(name);
    let (server, seen) = tcp_echo().await;
    let link = FaultLink::tcp(&ctx, "link", ANY.parse().unwrap(), server)
        .await
        .unwrap();
    (ctx, link, seen)
}

/// Connects and proves the connection is served.
async fn served(link: &FaultLink) -> TcpStream {
    let mut stream = TcpStream::connect(link.address()).await.unwrap();
    stream.write_all(b"hi").await.unwrap();
    assert_eq!(
        read_some(&mut stream, SOON).await.as_deref(),
        Some(&b"hi"[..])
    );
    stream
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tcp_half_open_then_cut_leaves_the_half_open_connection_silent() {
    let (_ctx, link, mut seen) = tcp_link("tcp-half-then-cut").await;
    let mut first = served(&link).await;
    link.half_open();
    // a connection made between the two is cut, the half-open one is not
    let mut between = served(&link).await;
    link.cut();
    assert_eq!(
        read_some(&mut between, SOON).await.as_deref(),
        Some(&b""[..]),
        "the connection made between the two is cut"
    );
    expect_events(&mut seen, "ended", 1).await;
    assert_eq!(
        read_some(&mut first, QUIET).await,
        None,
        "a half-open connection stays silent for good: no data, no close"
    );
    assert_no_event(&mut seen, "ended").await;
    // a connection made after both passes
    let mut after = served(&link).await;
    after.write_all(b"more").await.unwrap();
    assert!(read_some(&mut after, SOON).await.is_some());
    drop(link);
    expect_events(&mut seen, "ended", 2).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tcp_orphan_then_cut_closes_later_connections_and_keeps_the_orphans() {
    let (_ctx, link, mut seen) = tcp_link("tcp-orphan-then-cut").await;
    let mut orphaned = served(&link).await;
    // back to back: the cut must not change what the orphan already was
    link.orphan().unwrap();
    link.cut();
    assert_eq!(
        read_some(&mut orphaned, SOON).await.as_deref(),
        Some(&b""[..]),
        "the client side of the orphan is closed"
    );
    assert_no_event(&mut seen, "ended").await;

    let mut later = served(&link).await;
    link.cut();
    assert_eq!(
        read_some(&mut later, SOON).await.as_deref(),
        Some(&b""[..]),
        "a later connection is closed by the cut"
    );
    expect_events(&mut seen, "ended", 1).await;
    // the orphan's stack side is still open until the link goes
    assert_no_event(&mut seen, "ended").await;
    drop(link);
    expect_events(&mut seen, "ended", 1).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tcp_cut_then_half_open_leaves_the_cut_connection_closed() {
    let (_ctx, link, mut seen) = tcp_link("tcp-cut-then-half").await;
    let mut cut = served(&link).await;
    // back to back: the half-open must not change what the cut already was
    link.cut();
    link.half_open();
    assert_eq!(
        read_some(&mut cut, SOON).await.as_deref(),
        Some(&b""[..]),
        "the cut connection is closed"
    );
    expect_events(&mut seen, "ended", 1).await;

    let mut silent = served(&link).await;
    link.half_open();
    assert_eq!(
        read_some(&mut silent, QUIET).await,
        None,
        "a half-open connection is silent and not closed"
    );
    assert_no_event(&mut seen, "ended").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn restore_after_the_client_left_is_a_no_op() {
    let ctx = ctx("left");
    let (udp_server, _seen) = udp_echo().await;
    let udp = FaultLink::udp(&ctx, "udp", ANY.parse().unwrap(), udp_server)
        .await
        .unwrap();
    let (tcp_server, _events) = tcp_echo().await;
    let tcp = FaultLink::tcp(&ctx, "tcp", ANY.parse().unwrap(), tcp_server)
        .await
        .unwrap();
    {
        let client = UdpSocket::bind(ANY).await.unwrap();
        client.connect(udp.address()).await.unwrap();
        client.send(b"bye").await.unwrap();
        let mut stream = TcpStream::connect(tcp.address()).await.unwrap();
        stream.write_all(b"bye").await.unwrap();
        let _ = read_some(&mut stream, SOON).await;
    }
    for link in [&udp, &tcp] {
        link.set(Direction::Both, Fault::Stall);
        link.restore();
        link.half_open();
        link.cut();
        link.restore();
    }
    // and the links still work for the next client
    let client = UdpSocket::bind(ANY).await.unwrap();
    client.connect(udp.address()).await.unwrap();
    client.send(b"next").await.unwrap();
    assert_eq!(
        receive(&client, SOON).await.as_deref(),
        Some(&b"echo:next"[..])
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_bot_through_a_stalled_link_survives_a_short_stall() {
    let pulse = support::in_process_pulse().await;
    let ctx = support::ctx(pulse, "bot-through-link");
    let realm = ctx.realm("plaza");
    let link = FaultLink::udp(&ctx, "link", ANY.parse().unwrap(), pulse)
        .await
        .unwrap();

    let walker_id = ctx.identity(0);
    let walker = PulseBot::connect(&ctx, "walker", link.address(), ConnectOptions::default())
        .await
        .unwrap();
    assert_eq!(
        walker
            .handshake(&ctx, &walker_id, HandshakeOptions::default())
            .await
            .unwrap(),
        HandshakeOutcome::Accepted
    );
    walker.teleport(&realm, [8.0, 0.0, 8.0]).unwrap();
    let observer = PulseBot::join(&ctx, "observer", pulse, &ctx.identity(1))
        .await
        .unwrap();
    observer.teleport(&realm, [9.0, 0.0, 9.0]).unwrap();
    expect_or(&ctx, "the observer sees the walker", SOON, || {
        observer
            .view()
            .of(&walker_id.address())
            .map(|_| ())
            .ok_or_else(String::new)
    })
    .await
    .unwrap();

    link.set(Direction::Both, Fault::Stall);
    tokio::time::sleep(Duration::from_secs(2)).await;
    link.restore();

    walker.expect_alive().unwrap();
    for step in 1..=20 {
        walker
            .move_to([8.0 + step as f32 * 0.25, 0.0, 8.0])
            .unwrap();
        tokio::time::sleep(Duration::from_millis(60)).await;
    }
    expect_or(
        &ctx,
        "the observer sees the walker 5 m further after the stall",
        SOON,
        || {
            walker.alive()?;
            let world = observer
                .view()
                .of(&walker_id.address())
                .map(|subject| subject.world)
                .ok_or("the walker is gone from the observer")?;
            ((world[0] - 13.0).abs() < 0.2)
                .then_some(())
                .ok_or_else(|| format!("the walker is at {world:?}"))
        },
    )
    .await
    .unwrap();
    assert!(link.counters().dropped > 0, "the stall dropped nothing");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_orphaned_tcp_connection_ends_for_the_client_and_stays_open_for_the_server() {
    let ctx = ctx("tcp-orphan");
    let (server, mut seen) = tcp_echo().await;
    let link = FaultLink::tcp(&ctx, "link", ANY.parse().unwrap(), server)
        .await
        .unwrap();
    let mut client = TcpStream::connect(link.address()).await.unwrap();
    client.write_all(b"hello").await.unwrap();
    assert!(read_some(&mut client, SOON).await.is_some());
    assert_eq!(seen.recv().await, Some("accepted"));
    assert_eq!(seen.recv().await, Some("read"));

    link.orphan().unwrap();
    // the client is told at once that its socket is gone
    assert_eq!(
        read_some(&mut client, SOON).await.as_deref(),
        Some(&b""[..])
    );
    // the server is told nothing: it still holds the old connection
    assert!(
        tokio::time::timeout(QUIET, seen.recv()).await.is_err(),
        "the server saw the orphaned connection end"
    );

    // the client dials again and is served, while the old connection is still held
    let mut fresh = TcpStream::connect(link.address()).await.unwrap();
    fresh.write_all(b"again").await.unwrap();
    assert_eq!(
        read_some(&mut fresh, SOON).await.as_deref(),
        Some(&b"again"[..])
    );
    assert_eq!(seen.recv().await, Some("accepted"));
    assert_eq!(seen.recv().await, Some("read"));

    // the link going away releases what it kept
    drop(link);
    let mut ended = 0;
    while ended < 2 {
        match tokio::time::timeout(SOON, seen.recv()).await {
            Ok(Some("ended")) => ended += 1,
            Ok(Some(_)) => {}
            other => panic!("the server did not see both connections end: {other:?}"),
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_closed_udp_port_is_unreachable_and_reopens_with_the_flow_intact() {
    let ctx = ctx("udp-port");
    let (server, mut seen) = udp_echo().await;
    let link = FaultLink::udp(&ctx, "link", ANY.parse().unwrap(), server)
        .await
        .unwrap();
    let client = UdpSocket::bind(ANY).await.unwrap();
    client.connect(link.address()).await.unwrap();
    client.send(b"one").await.unwrap();
    assert_eq!(
        receive(&client, SOON).await.as_deref(),
        Some(&b"echo:one"[..])
    );
    let source = seen.recv().await.unwrap();

    link.close_port().unwrap();
    // a packet to a closed port comes back as an error on the socket, not as silence
    let mut refused = false;
    let mut buffer = [0u8; 64];
    for _ in 0..100 {
        if client.send(b"anyone?").await.is_err() {
            refused = true;
            break;
        }
        if let Ok(Err(_)) =
            tokio::time::timeout(Duration::from_millis(30), client.recv(&mut buffer)).await
        {
            refused = true;
            break;
        }
    }
    assert!(refused, "no port-unreachable error reached the client");
    assert!(
        seen.try_recv().is_err(),
        "the server heard a packet through a closed port"
    );

    link.restore();
    // the flow continues from the same upstream port: the server sees the same client
    assert!(
        echoes_again(&client).await,
        "nothing passed after the port reopened\n{}",
        link_timeline(&ctx)
    );
    assert_eq!(
        seen.recv().await.unwrap(),
        source,
        "the flow moved to another port"
    );
    assert_eq!(link.counters().flows, 1);
}

/// Sends `back` until its echo returns, for up to [`SOON`]; whether it did. A refusal (the port
/// is not bound yet) is answered at once, so each try waits a little.
async fn echoes_again(client: &UdpSocket) -> bool {
    let deadline = std::time::Instant::now() + SOON;
    let mut buffer = [0u8; 64];
    while std::time::Instant::now() < deadline {
        let _ = client.send(b"back").await;
        match tokio::time::timeout(Duration::from_millis(50), client.recv(&mut buffer)).await {
            Ok(Ok(length)) if &buffer[..length] == b"echo:back" => return true,
            Ok(Err(_)) => tokio::time::sleep(Duration::from_millis(20)).await,
            _ => {}
        }
    }
    false
}

/// The link's own timeline, for a failure message.
fn link_timeline(ctx: &Ctx) -> String {
    ctx.timeline()
        .snapshot()
        .iter()
        .map(|entry| format!("{} {} {}", entry.at_ms, entry.kind, entry.detail))
        .collect::<Vec<_>>()
        .join("\n")
}

/// While the port is closed another socket may try to take it. Where the platform lets the link
/// keep the port and still answer "port unreachable" (Linux), the other socket is refused; where
/// it does not, the other socket takes the port, and the link binds it again once it is free,
/// however long that takes: it never stays deaf.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_port_taken_while_closed_is_bound_again_once_it_is_free() {
    let ctx = ctx("udp-port-taken");
    let (server, _seen) = udp_echo().await;
    let link = FaultLink::udp(&ctx, "link", ANY.parse().unwrap(), server)
        .await
        .unwrap();
    let client = udp_client(&link).await;
    client.send(b"one").await.unwrap();
    assert_eq!(
        receive(&client, SOON).await.as_deref(),
        Some(&b"echo:one"[..])
    );

    link.close_port().unwrap();
    // the client is refused, so the link has given the port up or reserved it
    let mut refused = false;
    let mut buffer = [0u8; 64];
    for _ in 0..100 {
        if client.send(b"anyone?").await.is_err() {
            refused = true;
            break;
        }
        if let Ok(Err(_)) =
            tokio::time::timeout(Duration::from_millis(30), client.recv(&mut buffer)).await
        {
            refused = true;
            break;
        }
    }
    assert!(refused, "no port-unreachable error reached the client");
    let other = std::net::UdpSocket::bind(link.address()).ok();
    if cfg!(target_os = "linux") {
        assert!(other.is_none(), "the closed port was not kept for the link");
    }

    link.restore();
    // the other socket holds the port for longer than a link once waited for it
    tokio::time::sleep(Duration::from_secs(4)).await;
    drop(other);
    assert!(
        echoes_again(&client).await,
        "nothing passed after the port was free again\n{}",
        link_timeline(&ctx)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn each_kind_of_link_refuses_the_other_kind_of_fault() {
    let ctx = ctx("kinds");
    let (udp_server, _seen) = udp_echo().await;
    let udp = FaultLink::udp(&ctx, "udp", ANY.parse().unwrap(), udp_server)
        .await
        .unwrap();
    let (tcp_server, _events) = tcp_echo().await;
    let tcp = FaultLink::tcp(&ctx, "tcp", ANY.parse().unwrap(), tcp_server)
        .await
        .unwrap();
    assert!(udp.orphan().is_err());
    assert!(tcp.close_port().is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_dropped_link_frees_its_port_for_the_next_one() {
    let ctx = ctx("ports");
    let (udp_server, _seen) = udp_echo().await;
    let (tcp_server, _events) = tcp_echo().await;
    // The first links take the ports the operating system gives them. The next ones bind those
    // same ports, as the fronts of an explorer do from one scenario to the next.
    let mut udp_port: SocketAddr = ANY.parse().unwrap();
    let mut tcp_port: SocketAddr = ANY.parse().unwrap();
    for round in 0..3 {
        let udp = FaultLink::udp(&ctx, "udp", udp_port, udp_server)
            .await
            .unwrap_or_else(|error| panic!("round {round}: {error:#}"));
        let tcp = FaultLink::tcp(&ctx, "tcp", tcp_port, tcp_server)
            .await
            .unwrap_or_else(|error| panic!("round {round}: {error:#}"));
        (udp_port, tcp_port) = (udp.address(), tcp.address());
        // a live flow and a live connection: what a scenario leaves behind when it ends
        let client = UdpSocket::bind(ANY).await.unwrap();
        client.connect(udp.address()).await.unwrap();
        client.send(b"hello").await.unwrap();
        assert_eq!(
            receive(&client, SOON).await.as_deref(),
            Some(&b"echo:hello"[..])
        );
        let mut stream = TcpStream::connect(tcp.address()).await.unwrap();
        stream.write_all(b"hello").await.unwrap();
        assert!(read_some(&mut stream, SOON).await.is_some());
        drop(udp);
        drop(tcp);
    }
}

fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

/// The byte an echo of a one-byte datagram carries after `echo:`.
fn echoed_byte(echo: &[u8]) -> u8 {
    echo[5]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_link_delays_each_way_and_its_jitter_never_reorders() {
    let ctx = ctx("udp-link");
    let (server, _seen) = udp_echo().await;
    let link = FaultLink::udp(&ctx, "link", ANY.parse().unwrap(), server)
        .await
        .unwrap();
    let client = udp_client(&link).await;

    link.set(
        Direction::Both,
        Fault::Link {
            delay: ms(100),
            jitter: Duration::ZERO,
            loss: 0.0,
        },
    );
    let started = std::time::Instant::now();
    client.send(b"slow").await.unwrap();
    assert_eq!(
        receive(&client, SOON).await.as_deref(),
        Some(&b"echo:slow"[..])
    );
    let took = started.elapsed();
    assert!(
        took >= ms(195) && took < ms(1000),
        "a round trip took {took:?}"
    );

    // jitter: each packet waits 20 to 100 ms each way, and none overtakes the one before it
    link.set(
        Direction::Both,
        Fault::Link {
            delay: ms(20),
            jitter: ms(80),
            loss: 0.0,
        },
    );
    for index in 0..30u8 {
        client.send(&[index]).await.unwrap();
        tokio::time::sleep(ms(2)).await;
    }
    let mut back = Vec::new();
    while back.len() < 30 {
        let echo = receive(&client, SOON)
            .await
            .expect("every packet comes back");
        back.push(echoed_byte(&echo));
    }
    assert_eq!(
        back,
        (0..30).collect::<Vec<u8>>(),
        "a packet overtook another"
    );

    // spaced out so no packet waits behind another: the round trips spread over the window
    let mut round_trips = Vec::new();
    for index in 0..8u8 {
        let started = std::time::Instant::now();
        client.send(&[index]).await.unwrap();
        receive(&client, SOON).await.expect("it comes back");
        round_trips.push(started.elapsed());
    }
    let (shortest, longest) = (
        *round_trips.iter().min().unwrap(),
        *round_trips.iter().max().unwrap(),
    );
    assert!(shortest >= ms(38), "{round_trips:?}");
    assert!(longest < ms(400), "{round_trips:?}");
    assert!(
        longest - shortest >= ms(15),
        "the jitter did not spread the round trips: {round_trips:?}"
    );
    let counters = link.counters();
    assert_eq!(counters.delayed, 2 + 2 * 30 + 2 * 8);
    assert_eq!(counters.dropped, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_lossy_link_loses_its_share_and_the_same_packets_for_the_same_seed() {
    let mut runs = Vec::new();
    for _ in 0..2 {
        let ctx = ctx("udp-loss");
        let (server, _seen) = udp_echo().await;
        let link = FaultLink::udp(&ctx, "link", ANY.parse().unwrap(), server)
            .await
            .unwrap();
        let client = udp_client(&link).await;
        link.set(
            Direction::Uplink,
            Fault::Link {
                delay: Duration::ZERO,
                jitter: Duration::ZERO,
                loss: 0.3,
            },
        );
        for index in 0..200u8 {
            client.send(&[index]).await.unwrap();
            tokio::time::sleep(Duration::from_micros(500)).await;
        }
        let mut echoed = 0;
        while receive(&client, QUIET).await.is_some() {
            echoed += 1;
        }
        // the first packet is the one `udp_client` sent before the fault
        let lost: Vec<usize> = link
            .history(Direction::Uplink)
            .iter()
            .enumerate()
            .filter(|(_, seen)| seen.fate == Fate::Lost)
            .map(|(index, _)| index)
            .collect();
        assert_eq!(link.counters().dropped, lost.len() as u64);
        assert_eq!(echoed + lost.len(), 200, "every packet passed or was lost");
        assert!(
            (30..=90).contains(&lost.len()),
            "{} of 200 lost at 30 %",
            lost.len()
        );
        runs.push(lost);
    }
    assert_eq!(runs[0], runs[1], "the same seed lost other packets");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_queue_keeps_what_arrives_up_to_its_limit_and_releases_it_together_in_order() {
    let ctx = ctx("udp-queue");
    let (server, _seen) = udp_echo().await;
    let link = FaultLink::udp(&ctx, "link", ANY.parse().unwrap(), server)
        .await
        .unwrap();
    let client = udp_client(&link).await;

    link.set(Direction::Uplink, Fault::Queue { limit: 5 });
    for index in 0..8u8 {
        client.send(&[index]).await.unwrap();
    }
    assert_eq!(
        receive(&client, QUIET).await,
        None,
        "a queued packet passed"
    );
    let counters = link.counters();
    assert_eq!((counters.queued, counters.dropped), (5, 3));
    let fates: Vec<Fate> = link
        .history(Direction::Uplink)
        .iter()
        .skip(1)
        .map(|seen| seen.fate)
        .collect();
    assert_eq!(
        fates,
        [[Fate::Queued; 5].as_slice(), [Fate::Lost; 3].as_slice()].concat()
    );

    // the queue ends: what it kept goes on at once, in the order it came
    link.set(Direction::Uplink, Fault::Pass);
    let started = std::time::Instant::now();
    let mut back = Vec::new();
    for _ in 0..5 {
        let echo = receive(&client, SOON)
            .await
            .expect("a kept packet comes back");
        back.push(echoed_byte(&echo));
    }
    assert!(started.elapsed() < ms(500), "{:?}", started.elapsed());
    assert_eq!(back, vec![0, 1, 2, 3, 4]);
    assert_eq!(
        receive(&client, QUIET).await,
        None,
        "a lost packet came back"
    );
    assert_eq!(link.counters().released, 5);

    // released into a delayed link: they wait the delay, and arrive together
    link.set(Direction::Uplink, Fault::Queue { limit: 10 });
    for index in 10..13u8 {
        client.send(&[index]).await.unwrap();
    }
    assert_eq!(receive(&client, QUIET).await, None);
    link.set(
        Direction::Uplink,
        Fault::Link {
            delay: ms(200),
            jitter: Duration::ZERO,
            loss: 0.0,
        },
    );
    let started = std::time::Instant::now();
    let mut arrivals = Vec::new();
    for expected in 10..13u8 {
        let echo = receive(&client, SOON)
            .await
            .expect("a kept packet comes back");
        assert_eq!(echoed_byte(&echo), expected);
        arrivals.push(started.elapsed());
    }
    assert!(arrivals[0] >= ms(195), "{arrivals:?}");
    assert!(arrivals[2] - arrivals[0] < ms(100), "{arrivals:?}");
    let counters = link.counters();
    assert_eq!((counters.queued, counters.released), (8, 8));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn queued_packets_do_not_hold_the_port_after_the_link_is_dropped() {
    let ctx = ctx("udp-queue-lifetime");
    let (server, _seen) = udp_echo().await;
    let link = FaultLink::udp(&ctx, "link", ANY.parse().unwrap(), server)
        .await
        .unwrap();
    let client = udp_client(&link).await;
    link.set(Direction::Both, Fault::Queue { limit: 100 });
    client.send(b"kept").await.unwrap();
    assert_eq!(receive(&client, QUIET).await, None);
    assert_eq!(link.counters().queued, 1);
    let address = link.address();
    drop(link);
    let mut bound = None;
    for _ in 0..50 {
        match std::net::UdpSocket::bind(address) {
            Ok(socket) => {
                bound = Some(socket);
                break;
            }
            Err(_) => tokio::time::sleep(ms(20)).await,
        }
    }
    assert!(
        bound.is_some(),
        "a queued packet still holds the listen socket"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_history_notes_each_packet_as_it_arrives_with_its_size_and_fate() {
    let ctx = ctx("udp-history");
    let (server, _seen) = udp_echo().await;
    let link = FaultLink::udp(&ctx, "link", ANY.parse().unwrap(), server)
        .await
        .unwrap();
    let client = UdpSocket::bind(ANY).await.unwrap();
    client.connect(link.address()).await.unwrap();
    let before = link.clock();

    client.send(&[0; 10]).await.unwrap();
    assert!(receive(&client, SOON).await.is_some());
    link.set(Direction::Uplink, Fault::Drop { share: 1.0 });
    client.send(&[0; 100]).await.unwrap();
    assert_eq!(receive(&client, QUIET).await, None);
    link.set(Direction::Uplink, Fault::Delay { by: ms(50) });
    client.send(&[0; 30]).await.unwrap();
    assert!(receive(&client, SOON).await.is_some());
    let after = link.clock();

    let up = link.history(Direction::Uplink);
    let sizes: Vec<(usize, Fate)> = up.iter().map(|seen| (seen.bytes, seen.fate)).collect();
    assert_eq!(
        sizes,
        [(10, Fate::Passed), (100, Fate::Lost), (30, Fate::Delayed)]
    );
    // the downlink saw the two echoes, each the datagram and `echo:`
    let down: Vec<(usize, Fate)> = link
        .history(Direction::Downlink)
        .iter()
        .map(|seen| (seen.bytes, seen.fate))
        .collect();
    assert_eq!(down, [(15, Fate::Passed), (35, Fate::Passed)]);
    assert!(up.windows(2).all(|pair| pair[0].at <= pair[1].at));
    assert!(up[0].at >= before && up[2].at <= after);
    // both directions together, in the order they arrived
    let both = link.history(Direction::Both);
    assert_eq!(both.len(), 5);
    assert!(both.windows(2).all(|pair| pair[0].at <= pair[1].at));

    let windows = windows(&up, before, after, ms(50), 50);
    assert_eq!(windows.iter().map(|window| window.packets).sum::<u32>(), 3);
    assert_eq!(windows.iter().map(|window| window.large).sum::<u32>(), 1);
    assert_eq!(windows.iter().map(|window| window.bytes).sum::<u64>(), 140);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_tcp_link_takes_its_delay_and_a_tcp_queue_holds_the_bytes_as_a_stall_does() {
    let (_ctx, link, _seen) = tcp_link("tcp-link-queue").await;
    let mut stream = served(&link).await;
    link.set(
        Direction::Uplink,
        Fault::Link {
            delay: ms(250),
            jitter: ms(100),
            loss: 0.5,
        },
    );
    let started = std::time::Instant::now();
    stream.write_all(b"late").await.unwrap();
    assert_eq!(
        read_some(&mut stream, SOON).await.as_deref(),
        Some(&b"late"[..]),
        "a TCP link loses nothing"
    );
    assert!(started.elapsed() >= ms(240), "{:?}", started.elapsed());

    link.set(Direction::Uplink, Fault::Queue { limit: 1 });
    stream.write_all(b"held").await.unwrap();
    assert_eq!(read_some(&mut stream, QUIET).await, None);
    link.restore();
    assert_eq!(
        read_some(&mut stream, SOON).await.as_deref(),
        Some(&b"held"[..])
    );
}

/// What a queue kept for a flow that was half-opened or cut before the queue ended is lost with
/// that flow: it does not go out through the old upstream socket.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_queue_ending_after_its_flow_ended_does_not_send_what_it_kept() {
    for (name, end) in [("half-open", 0), ("cut", 1)] {
        let ctx = ctx("udp-queue-generation");
        let (server, mut seen) = udp_echo().await;
        let link = FaultLink::udp(&ctx, "link", ANY.parse().unwrap(), server)
            .await
            .unwrap();
        let client = udp_client(&link).await;
        seen.recv().await.unwrap();
        link.set(Direction::Both, Fault::Queue { limit: 10 });
        client.send(b"kept one").await.unwrap();
        client.send(b"kept two").await.unwrap();
        expect_or(&ctx, "the queue keeps both", SOON, || {
            (link.counters().queued == 2)
                .then_some(())
                .ok_or_else(|| "not yet".to_owned())
        })
        .await
        .unwrap();
        if end == 0 {
            link.half_open();
        } else {
            link.cut();
        }
        let dropped = link.counters().dropped;
        link.set(Direction::Both, Fault::Pass);
        tokio::time::sleep(QUIET).await;
        assert!(
            seen.try_recv().is_err(),
            "{name}: the stack heard what the queue kept for the old flow"
        );
        assert_eq!(receive(&client, QUIET).await, None, "{name}");
        let counters = link.counters();
        assert_eq!(
            (counters.released, counters.dropped - dropped),
            (0, 2),
            "{name}: {counters:?}"
        );
    }
}

/// The history notes the datagrams the relay drops before any fault acts: those of a half-open
/// flow, each way, and an answer that comes back while the port is closed.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_history_notes_what_a_half_open_flow_or_a_closed_port_drops() {
    let ctx = ctx("udp-history-drops");
    let (server, _seen) = udp_echo().await;
    let link = FaultLink::udp(&ctx, "link", ANY.parse().unwrap(), server)
        .await
        .unwrap();
    let client = udp_client(&link).await;
    // the datagram is on its way to the stack when the flow goes half-open: its answer is dropped
    link.set(
        Direction::Uplink,
        Fault::Link {
            delay: ms(200),
            jitter: Duration::ZERO,
            loss: 0.0,
        },
    );
    client.send(b"late").await.unwrap();
    expect_or(&ctx, "the link takes the datagram", SOON, || {
        (link.history(Direction::Uplink).len() == 2)
            .then_some(())
            .ok_or_else(|| "not yet".to_owned())
    })
    .await
    .unwrap();
    link.half_open();
    client.send(b"silent").await.unwrap();
    assert_eq!(receive(&client, ms(600)).await, None);
    let up: Vec<(usize, Fate)> = link
        .history(Direction::Uplink)
        .iter()
        .map(|seen| (seen.bytes, seen.fate))
        .collect();
    assert_eq!(
        up,
        [(2, Fate::Passed), (4, Fate::Delayed), (6, Fate::Lost)],
        "the half-open flow's datagram is noted as lost"
    );
    let down: Vec<(usize, Fate)> = link
        .history(Direction::Downlink)
        .iter()
        .map(|seen| (seen.bytes, seen.fate))
        .collect();
    assert_eq!(down, [(7, Fate::Passed), (9, Fate::Lost)]);

    // a new flow; its datagram waits on the uplink while the port closes, so the answer comes
    // back to a closed port
    let other = udp_client(&link).await;
    let before = link.history(Direction::Downlink).len();
    let noted = link.history(Direction::Uplink).len();
    other.send(b"closing").await.unwrap();
    expect_or(&ctx, "the link takes the datagram", SOON, || {
        (link.history(Direction::Uplink).len() > noted)
            .then_some(())
            .ok_or_else(|| "not yet".to_owned())
    })
    .await
    .unwrap();
    link.close_port().unwrap();
    tokio::time::sleep(ms(600)).await;
    let down = link.history(Direction::Downlink);
    assert_eq!(down.len(), before + 1, "{down:?}");
    assert_eq!(
        (down[before].bytes, down[before].fate),
        (12, Fate::Lost),
        "an answer sent while the port is closed is not noted as passed"
    );
}

/// A delay keeps packets in order: one that arrives under a shorter delay waits for one still
/// held under a longer path before it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_delay_keeps_packets_in_order() {
    let ctx = ctx("udp-delay-order");
    let (server, _seen) = udp_echo().await;
    let link = FaultLink::udp(&ctx, "link", ANY.parse().unwrap(), server)
        .await
        .unwrap();
    let client = udp_client(&link).await;
    link.set(
        Direction::Uplink,
        Fault::Link {
            delay: ms(300),
            jitter: Duration::ZERO,
            loss: 0.0,
        },
    );
    client.send(&[1]).await.unwrap();
    tokio::time::sleep(ms(20)).await;
    link.set(Direction::Uplink, Fault::Delay { by: ms(10) });
    client.send(&[2]).await.unwrap();
    let mut back = Vec::new();
    for _ in 0..2 {
        back.push(echoed_byte(
            &receive(&client, SOON).await.expect("it comes back"),
        ));
    }
    assert_eq!(back, [1, 2], "a delayed packet overtook one before it");
    // a burst under one delay arrives in the order it was sent
    for index in 10..60u8 {
        client.send(&[index]).await.unwrap();
    }
    let mut back = Vec::new();
    while back.len() < 50 {
        back.push(echoed_byte(
            &receive(&client, SOON).await.expect("it comes back"),
        ));
    }
    assert_eq!(back, (10..60).collect::<Vec<u8>>());
}

/// A delay too long for the clock holds what it delays; the relay goes on relaying.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_delay_too_long_for_the_clock_does_not_stop_the_relay() {
    let ctx = ctx("udp-delay-forever");
    let (server, _seen) = udp_echo().await;
    let link = FaultLink::udp(&ctx, "link", ANY.parse().unwrap(), server)
        .await
        .unwrap();
    let client = udp_client(&link).await;
    for fault in [
        Fault::Delay { by: Duration::MAX },
        Fault::Reorder {
            window: Duration::MAX,
        },
        Fault::Link {
            delay: Duration::MAX,
            jitter: Duration::MAX,
            loss: 0.0,
        },
    ] {
        link.set(Direction::Uplink, fault);
        let packets = link.counters().up_packets;
        client.send(b"one").await.unwrap();
        client.send(b"two").await.unwrap();
        expect_or(&ctx, "the relay takes both", SOON, || {
            (link.counters().up_packets == packets + 2)
                .then_some(())
                .ok_or(format!("{fault:?}: {:?}", link.counters()))
        })
        .await
        .unwrap();
    }
    assert_eq!(receive(&client, QUIET).await, None);

    let (_ctx, tcp, _events) = tcp_link("tcp-delay-forever").await;
    let mut stream = served(&tcp).await;
    tcp.set(Direction::Uplink, Fault::Delay { by: Duration::MAX });
    let bytes = tcp.counters().up_bytes;
    stream.write_all(b"one").await.unwrap();
    tokio::time::sleep(ms(100)).await;
    stream.write_all(b"two").await.unwrap();
    expect_or(&ctx, "the TCP relay reads both", SOON, || {
        (tcp.counters().up_bytes == bytes + 6)
            .then_some(())
            .ok_or(format!("{:?}", tcp.counters()))
    })
    .await
    .unwrap();
    assert_eq!(read_some(&mut stream, QUIET).await, None);
}
