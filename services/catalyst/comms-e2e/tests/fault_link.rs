//! The fault link on its own, and a Pulse bot through it.

mod support;

use std::net::SocketAddr;
use std::time::Duration;

use catalyst_comms_e2e::engine::{Ctx, expect_or};
use catalyst_comms_e2e::fault::{Direction, Fault, FaultLink};
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
    let mut echoed = false;
    for _ in 0..100 {
        let _ = client.send(b"back").await;
        if let Ok(Ok(length)) =
            tokio::time::timeout(Duration::from_millis(50), client.recv(&mut buffer)).await
            && &buffer[..length] == b"echo:back"
        {
            echoed = true;
            break;
        }
    }
    assert!(echoed, "nothing passed after the port reopened");
    assert_eq!(
        seen.recv().await.unwrap(),
        source,
        "the flow moved to another port"
    );
    assert_eq!(link.counters().flows, 1);
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
    // fixed ports, as the fronts of an explorer have: the next scenario binds the same ones
    let udp_port = {
        let probe = UdpSocket::bind(ANY).await.unwrap();
        probe.local_addr().unwrap()
    };
    let tcp_port = {
        let probe = TcpListener::bind(ANY).await.unwrap();
        probe.local_addr().unwrap()
    };
    for round in 0..3 {
        let udp = FaultLink::udp(&ctx, "udp", udp_port, udp_server)
            .await
            .unwrap_or_else(|error| panic!("round {round}: {error:#}"));
        let tcp = FaultLink::tcp(&ctx, "tcp", tcp_port, tcp_server)
            .await
            .unwrap_or_else(|error| panic!("round {round}: {error:#}"));
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
