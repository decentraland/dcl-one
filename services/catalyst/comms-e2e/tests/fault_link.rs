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
    let link = FaultLink::udp(&ctx, "link", ANY.parse().unwrap(), server).await.unwrap();
    let client = UdpSocket::bind(ANY).await.unwrap();
    client.connect(link.address()).await.unwrap();

    client.send(b"one").await.unwrap();
    assert_eq!(receive(&client, SOON).await.as_deref(), Some(&b"echo:one"[..]));
    let first_source = seen.recv().await.unwrap();

    // uplink stalled: the server hears nothing, so nothing comes back
    link.set(Direction::Uplink, Fault::Stall);
    client.send(b"two").await.unwrap();
    assert_eq!(receive(&client, QUIET).await, None);
    assert!(seen.try_recv().is_err(), "the server heard a stalled packet");
    assert_eq!(link.counters().dropped, 1);

    // downlink stalled: the server hears it, the answer is lost
    link.set(Direction::Uplink, Fault::Pass);
    link.set(Direction::Downlink, Fault::Stall);
    client.send(b"three").await.unwrap();
    assert_eq!(seen.recv().await.unwrap(), first_source, "one flow, one source port");
    assert_eq!(receive(&client, QUIET).await, None);

    // restored: the stalled packets are gone for good, new ones pass
    link.restore();
    client.send(b"four").await.unwrap();
    assert_eq!(receive(&client, SOON).await.as_deref(), Some(&b"echo:four"[..]));
    assert_eq!(link.counters().flows, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn udp_half_open_silences_the_old_socket_and_cut_moves_the_flow_to_a_new_port() {
    let ctx = ctx("udp-flows");
    let (server, mut seen) = udp_echo().await;
    let link = FaultLink::udp(&ctx, "link", ANY.parse().unwrap(), server).await.unwrap();
    let old = UdpSocket::bind(ANY).await.unwrap();
    old.connect(link.address()).await.unwrap();
    old.send(b"hello").await.unwrap();
    assert!(receive(&old, SOON).await.is_some());
    let old_source = seen.recv().await.unwrap();

    link.half_open();
    old.send(b"anyone?").await.unwrap();
    assert_eq!(receive(&old, QUIET).await, None, "a half-open flow is silent");
    assert!(seen.try_recv().is_err());
    // restoring the link does not revive it
    link.restore();
    old.send(b"still there?").await.unwrap();
    assert_eq!(receive(&old, QUIET).await, None);

    // a new socket of the same client is a new flow and passes
    let new = UdpSocket::bind(ANY).await.unwrap();
    new.connect(link.address()).await.unwrap();
    new.send(b"again").await.unwrap();
    assert_eq!(receive(&new, SOON).await.as_deref(), Some(&b"echo:again"[..]));
    let new_source = seen.recv().await.unwrap();
    assert_ne!(new_source, old_source);

    // cut: the same client socket continues from another upstream port
    link.cut();
    new.send(b"after the cut").await.unwrap();
    assert_eq!(receive(&new, SOON).await.as_deref(), Some(&b"echo:after the cut"[..]));
    assert_ne!(seen.recv().await.unwrap(), new_source);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn udp_faults_drop_delay_and_duplicate() {
    let ctx = ctx("udp-faults");
    let (server, _seen) = udp_echo().await;
    let link = FaultLink::udp(&ctx, "link", ANY.parse().unwrap(), server).await.unwrap();
    let client = UdpSocket::bind(ANY).await.unwrap();
    client.connect(link.address()).await.unwrap();

    link.set(Direction::Uplink, Fault::Drop { share: 1.0 });
    client.send(b"lost").await.unwrap();
    assert_eq!(receive(&client, QUIET).await, None);

    link.set(Direction::Uplink, Fault::Delay { by: Duration::from_millis(250) });
    let started = std::time::Instant::now();
    client.send(b"late").await.unwrap();
    assert_eq!(receive(&client, SOON).await.as_deref(), Some(&b"echo:late"[..]));
    assert!(started.elapsed() >= Duration::from_millis(240), "{:?}", started.elapsed());

    link.set(Direction::Uplink, Fault::Duplicate);
    client.send(b"twice").await.unwrap();
    assert_eq!(receive(&client, SOON).await.as_deref(), Some(&b"echo:twice"[..]));
    assert_eq!(receive(&client, SOON).await.as_deref(), Some(&b"echo:twice"[..]));
}

/// A TCP server that echoes, and reports each connection it accepts and each one that ends.
async fn tcp_echo() -> (SocketAddr, tokio::sync::mpsc::UnboundedReceiver<&'static str>) {
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
    let link = FaultLink::tcp(&ctx, "link", ANY.parse().unwrap(), server).await.unwrap();
    let mut client = TcpStream::connect(link.address()).await.unwrap();
    client.write_all(b"one").await.unwrap();
    assert_eq!(read_some(&mut client, SOON).await.as_deref(), Some(&b"one"[..]));
    assert_eq!(seen.recv().await, Some("accepted"));
    assert_eq!(seen.recv().await, Some("read"));

    link.set(Direction::Both, Fault::Stall);
    client.write_all(b"two").await.unwrap();
    assert_eq!(read_some(&mut client, QUIET).await, None);
    assert!(seen.try_recv().is_err(), "the server read during a stall");

    // unlike UDP, the stream is intact after the stall
    link.restore();
    assert_eq!(read_some(&mut client, SOON).await.as_deref(), Some(&b"two"[..]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn half_open_keeps_the_tcp_socket_open_and_silent() {
    let ctx = ctx("tcp-half-open");
    let (server, mut seen) = tcp_echo().await;
    let link = FaultLink::tcp(&ctx, "link", ANY.parse().unwrap(), server).await.unwrap();
    let mut stale = TcpStream::connect(link.address()).await.unwrap();
    stale.write_all(b"hello").await.unwrap();
    assert!(read_some(&mut stale, SOON).await.is_some());
    assert_eq!(seen.recv().await, Some("accepted"));
    assert_eq!(seen.recv().await, Some("read"));

    link.half_open();
    // the write succeeds, nothing reaches the server, and neither side sees a close
    stale.write_all(b"anyone?").await.unwrap();
    assert_eq!(read_some(&mut stale, QUIET).await, None, "no data and no close");
    assert!(seen.try_recv().is_err(), "the server saw the stale socket move or end");
    link.restore();
    assert_eq!(read_some(&mut stale, QUIET).await, None, "a stale socket does not come back");

    // the same client connects again and is served
    let mut fresh = TcpStream::connect(link.address()).await.unwrap();
    fresh.write_all(b"again").await.unwrap();
    assert_eq!(read_some(&mut fresh, SOON).await.as_deref(), Some(&b"again"[..]));
    assert_eq!(seen.recv().await, Some("accepted"));
    assert_eq!(seen.recv().await, Some("read"));

    // a cut ends both the stale and the fresh connection on both sides
    link.cut();
    assert_eq!(read_some(&mut fresh, SOON).await.as_deref(), Some(&b""[..]), "the client sees the end");
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
async fn restore_after_the_client_left_is_a_no_op() {
    let ctx = ctx("left");
    let (udp_server, _seen) = udp_echo().await;
    let udp = FaultLink::udp(&ctx, "udp", ANY.parse().unwrap(), udp_server).await.unwrap();
    let (tcp_server, _events) = tcp_echo().await;
    let tcp = FaultLink::tcp(&ctx, "tcp", ANY.parse().unwrap(), tcp_server).await.unwrap();
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
    assert_eq!(receive(&client, SOON).await.as_deref(), Some(&b"echo:next"[..]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_bot_through_a_stalled_link_survives_a_short_stall() {
    let pulse = support::in_process_pulse().await;
    let ctx = support::ctx(pulse, "bot-through-link");
    let realm = ctx.realm("plaza");
    let link = FaultLink::udp(&ctx, "link", ANY.parse().unwrap(), pulse).await.unwrap();

    let walker_id = ctx.identity(0);
    let walker = PulseBot::connect(&ctx, "walker", link.address(), ConnectOptions::default())
        .await
        .unwrap();
    assert_eq!(
        walker.handshake(&ctx, &walker_id, HandshakeOptions::default()).await.unwrap(),
        HandshakeOutcome::Accepted
    );
    walker.teleport(&realm, [8.0, 0.0, 8.0]).unwrap();
    let observer = PulseBot::join(&ctx, "observer", pulse, &ctx.identity(1)).await.unwrap();
    observer.teleport(&realm, [9.0, 0.0, 9.0]).unwrap();
    expect_or(&ctx, "the observer sees the walker", SOON, || {
        observer.view().of(&walker_id.address()).map(|_| ()).ok_or_else(String::new)
    })
    .await
    .unwrap();

    link.set(Direction::Both, Fault::Stall);
    tokio::time::sleep(Duration::from_secs(2)).await;
    link.restore();

    walker.expect_alive().unwrap();
    for step in 1..=20 {
        walker.move_to([8.0 + step as f32 * 0.25, 0.0, 8.0]).unwrap();
        tokio::time::sleep(Duration::from_millis(60)).await;
    }
    expect_or(&ctx, "the observer sees the walker 5 m further after the stall", SOON, || {
        walker.alive()?;
        let world = observer
            .view()
            .of(&walker_id.address())
            .map(|subject| subject.world)
            .ok_or("the walker is gone from the observer")?;
        ((world[0] - 13.0).abs() < 0.2)
            .then_some(())
            .ok_or_else(|| format!("the walker is at {world:?}"))
    })
    .await
    .unwrap();
    assert!(link.counters().dropped > 0, "the stall dropped nothing");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_orphaned_tcp_connection_ends_for_the_client_and_stays_open_for_the_server() {
    let ctx = ctx("tcp-orphan");
    let (server, mut seen) = tcp_echo().await;
    let link = FaultLink::tcp(&ctx, "link", ANY.parse().unwrap(), server).await.unwrap();
    let mut client = TcpStream::connect(link.address()).await.unwrap();
    client.write_all(b"hello").await.unwrap();
    assert!(read_some(&mut client, SOON).await.is_some());
    assert_eq!(seen.recv().await, Some("accepted"));
    assert_eq!(seen.recv().await, Some("read"));

    link.orphan().unwrap();
    // the client is told at once that its socket is gone
    assert_eq!(read_some(&mut client, SOON).await.as_deref(), Some(&b""[..]));
    // the server is told nothing: it still holds the old connection
    assert!(
        tokio::time::timeout(QUIET, seen.recv()).await.is_err(),
        "the server saw the orphaned connection end"
    );

    // the client dials again and is served, while the old connection is still held
    let mut fresh = TcpStream::connect(link.address()).await.unwrap();
    fresh.write_all(b"again").await.unwrap();
    assert_eq!(read_some(&mut fresh, SOON).await.as_deref(), Some(&b"again"[..]));
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
    let link = FaultLink::udp(&ctx, "link", ANY.parse().unwrap(), server).await.unwrap();
    let client = UdpSocket::bind(ANY).await.unwrap();
    client.connect(link.address()).await.unwrap();
    client.send(b"one").await.unwrap();
    assert_eq!(receive(&client, SOON).await.as_deref(), Some(&b"echo:one"[..]));
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
    assert!(seen.try_recv().is_err(), "the server heard a packet through a closed port");

    link.restore();
    // the flow continues from the same upstream port: the server sees the same client
    let mut echoed = false;
    for _ in 0..100 {
        let _ = client.send(b"back").await;
        if let Ok(Ok(length)) =
            tokio::time::timeout(Duration::from_millis(50), client.recv(&mut buffer)).await
        {
            if &buffer[..length] == b"echo:back" {
                echoed = true;
                break;
            }
        }
    }
    assert!(echoed, "nothing passed after the port reopened");
    assert_eq!(seen.recv().await.unwrap(), source, "the flow moved to another port");
    assert_eq!(link.counters().flows, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn each_kind_of_link_refuses_the_other_kind_of_fault() {
    let ctx = ctx("kinds");
    let (udp_server, _seen) = udp_echo().await;
    let udp = FaultLink::udp(&ctx, "udp", ANY.parse().unwrap(), udp_server).await.unwrap();
    let (tcp_server, _events) = tcp_echo().await;
    let tcp = FaultLink::tcp(&ctx, "tcp", ANY.parse().unwrap(), tcp_server).await.unwrap();
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
        assert_eq!(receive(&client, SOON).await.as_deref(), Some(&b"echo:hello"[..]));
        let mut stream = TcpStream::connect(tcp.address()).await.unwrap();
        stream.write_all(b"hello").await.unwrap();
        assert!(read_some(&mut stream, SOON).await.is_some());
        drop(udp);
        drop(tcp);
    }
}
