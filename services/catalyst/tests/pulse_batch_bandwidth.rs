// from: catalyrst/crates/catalyrst-pulse/tests/batch_bandwidth.rs
#[path = "support/wt_client.rs"]
mod wt_client;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

use catalyst::comms::pulse::{
    batch::{BatchSubject, MAX_BATCH_BYTES, SeqEncoding, encode_batches},
    decentraland::pulse,
    transport::{Event, Host, HostConfig, Packet, PeerId},
};
use catalyst::shared::test_wait::HANG_GUARD;
use prost::Message;

/// What ends each ENet tick: a reliable packet on the movement channel, which ENet hands over
/// only after the unreliable packets queued ahead of it on that channel.
const MARKER: [u8; 1] = [0xff];

struct Relay {
    addr: SocketAddr,
    bytes: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Relay {
    fn start(server: SocketAddr) -> Self {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        socket
            .set_read_timeout(Some(Duration::from_millis(20)))
            .unwrap();
        let addr = socket.local_addr().unwrap();
        let bytes = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (count, stopping) = (bytes.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            let mut client = None;
            let mut buffer = [0; 65536];
            while !stopping.load(Ordering::Relaxed) {
                match socket.recv_from(&mut buffer) {
                    Ok((len, source)) => {
                        let target = if source == server {
                            count.fetch_add(len + 28, Ordering::SeqCst);
                            client.expect("client initiated connection")
                        } else {
                            client = Some(source);
                            server
                        };
                        socket.send_to(&buffer[..len], target).unwrap();
                    }
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                        ) => {}
                    Err(error) => panic!("relay: {error}"),
                }
            }
        });
        Self {
            addr,
            bytes,
            stop,
            thread: Some(thread),
        }
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
    }
}

fn config() -> HostConfig {
    HostConfig {
        bind: "127.0.0.1:0".parse().unwrap(),
        max_peers: 2,
        channel_limit: 2,
    }
}

fn movement_packets(batch: bool, tick: u32) -> Vec<Vec<u8>> {
    let expected: Vec<_> = (4096..4135)
        .map(|subject_id| pulse::PlayerStateDeltaTier0 {
            subject_id,
            baseline_seq: 5000 + tick,
            new_seq: 5001 + tick,
            server_tick: 100000 + tick * 33,
            position_x: Some(128 + tick),
            position_y: Some(4200),
            position_z: Some(200),
            velocity_x: Some(40),
            velocity_z: Some(41),
            rotation_y: Some(64),
            movement_blend: Some(10),
            ..Default::default()
        })
        .collect();
    let messages: Vec<_> = if batch {
        let subjects: Vec<_> = expected
            .iter()
            .map(|delta| BatchSubject::from_delta(delta, 1))
            .collect();
        encode_batches(
            expected[0].server_tick,
            &subjects,
            MAX_BATCH_BYTES,
            SeqEncoding::AbsoluteBaseline,
        )
        .into_iter()
        .map(|encoded| {
            pulse::server_message::Message::PlayerStateDeltaBatchBaseline(
                pulse::PlayerStateDeltaBatch {
                    server_tick: encoded.server_tick,
                    subject_count: encoded.subject_count,
                    payload: encoded.payload,
                },
            )
        })
        .collect()
    } else {
        expected
            .iter()
            .copied()
            .map(pulse::server_message::Message::PlayerStateDelta)
            .collect()
    };
    messages
        .into_iter()
        .map(|message| {
            pulse::ServerMessage {
                message: Some(message),
            }
            .encode_to_vec()
        })
        .collect()
}

fn subjects(packet: &[u8]) -> usize {
    match pulse::ServerMessage::decode(packet).unwrap().message {
        Some(pulse::server_message::Message::PlayerStateDelta(_)) => 1,
        Some(pulse::server_message::Message::PlayerStateDeltaBatchBaseline(batch)) => {
            batch.subject_count as usize
        }
        other => panic!("not a movement packet: {other:?}"),
    }
}

/// Queues [`MARKER`] behind everything the server holds for `peer` and returns what the client
/// takes in ahead of it: by then every byte the server sent before it has crossed the relay.
async fn through_marker(server: &mut Host, client: &mut Host, peer: PeerId) -> Vec<Vec<u8>> {
    server
        .send(peer, Packet::reliable(1, MARKER.to_vec()))
        .await
        .unwrap();
    server.flush();
    let started = Instant::now();
    let mut received = Vec::new();
    loop {
        assert!(
            started.elapsed() < HANG_GUARD,
            "the ENet marker never arrived"
        );
        match client.service().await.unwrap() {
            Some(Event::Receive { packet, .. }) if packet.data.as_ref() == MARKER => {
                return received;
            }
            Some(Event::Receive { packet, .. }) => received.push(packet.data.to_vec()),
            Some(Event::Disconnect { .. }) => panic!("ENet disconnected mid-measurement"),
            Some(_) => {}
            // Nothing arrived yet: let the server resend the marker if it was lost.
            None => {
                server.service().await.unwrap();
            }
        }
    }
}

/// Egress bytes per movement subject the client took in. ENet's packet throttle may drop
/// unreliable movement under load before it is sent, so a dropped packet costs neither
/// bytes nor subjects; with nothing dropped this is the whole run's bytes over its subjects.
async fn measure(batch: bool) -> f64 {
    let mut server = Host::bind(config()).await.unwrap();
    let relay = Relay::start(server.local_addr().unwrap());
    let mut client = Host::bind(config()).await.unwrap();
    client.connect(relay.addr, 2).unwrap();
    let mut peer = None;
    let mut connected = false;
    let connecting = Instant::now();
    while peer.is_none() || !connected {
        assert!(connecting.elapsed() < HANG_GUARD, "ENet connect hung");
        if let Some(Event::Connect { peer: id, .. }) = server.service().await.unwrap() {
            peer = Some(id);
        }
        if let Some(Event::Connect { .. }) = client.service().await.unwrap() {
            connected = true;
        }
    }
    let peer = peer.unwrap();
    // The handshake's tail is behind the relay once a marker queued after it arrives.
    assert!(
        through_marker(&mut server, &mut client, peer)
            .await
            .is_empty()
    );
    let start = relay.bytes.load(Ordering::SeqCst);
    let mut delivered = 0;
    for tick in 0..20 {
        let sent = movement_packets(batch, tick);
        for payload in &sent {
            server
                .send(peer, Packet::unreliable(1, payload.clone()))
                .await
                .unwrap();
        }
        let received = through_marker(&mut server, &mut client, peer).await;
        let mut unsent = sent.iter();
        for packet in &received {
            assert!(
                unsent.any(|sent| sent == packet),
                "tick {tick}: the client took in a packet out of the order it was sent"
            );
        }
        delivered += received
            .iter()
            .map(|packet| subjects(packet))
            .sum::<usize>();
    }
    assert!(delivered > 0, "no movement reached the client");
    (relay.bytes.load(Ordering::SeqCst) - start) as f64 / delivered as f64
}

#[tokio::test]
async fn crowded_movement_halves_enet_ipv4_egress_bytes() {
    let legacy = measure(false).await;
    let negotiated = measure(true).await;
    println!(
        "39 subjects, 20 ticks, ENet + UDP + IPv4 egress per delivered subject: legacy={legacy:.1}, negotiated={negotiated:.1}, reduction={:.2}%",
        100.0 * (1.0 - negotiated / legacy)
    );
    assert!(
        negotiated * 2.0 <= legacy,
        "50% network-byte target missed: {negotiated:.1}/{legacy:.1} bytes per subject"
    );
}

fn measure_webtransport(batch: bool) -> usize {
    use catalyst::comms::pulse::transport::webtransport::{WtConfig, WtHost};
    use sha2::{Digest, Sha256};
    use wt_client::{Client, ClientConfig, ClientEvent};

    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let hash = Sha256::digest(certificate.cert.der().as_ref()).to_vec();
    let (host, mut events) = WtHost::start(WtConfig {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        cert_pem: certificate.cert.pem(),
        key_pem: certificate.signing_key.serialize_pem(),
        slot_base: 4095,
        slot_capacity: 2,
        max_datagram_bytes: 1200,
        max_message_bytes: 4096,
        service_timeout_ms: 1,
        server_full_reason: 6,
    })
    .unwrap();
    let relay = Relay::start(host.local_addr());
    let mut client = Client::connect(ClientConfig {
        url: format!("https://{}/", relay.addr),
        server_cert_hash: Some(hash),
    })
    .unwrap();
    let waiting = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let connected = async {
        loop {
            match events.recv().await {
                Some(Event::Connect { peer, .. }) => break peer,
                Some(_) => {}
                None => panic!("the WebTransport host stopped"),
            }
        }
    };
    let peer = waiting
        .block_on(async { tokio::time::timeout(HANG_GUARD, connected).await })
        .expect("WebTransport connect hung");
    let mut datagram = || match client.service(HANG_GUARD) {
        Some(ClientEvent::Datagram { data }) => Some(data.to_vec()),
        Some(ClientEvent::Disconnected { reason }) => panic!("WebTransport disconnected: {reason}"),
        Some(_) => None,
        None => panic!("a WebTransport datagram never arrived"),
    };
    // The handshake's tail went out ahead of the first datagram the client takes in.
    host.send(peer as u32, Packet::unreliable(1, vec![0]));
    let warmup = loop {
        if let Some(data) = datagram() {
            break data;
        }
    };
    assert_eq!(warmup, [0]);
    let start = relay.bytes.load(Ordering::SeqCst);
    // QUIC drops a datagram only when its send buffer overflows, and each tick waits for the
    // last one, so every datagram is waited for.
    for tick in 0..20 {
        let mut expected = movement_packets(batch, tick);
        for payload in &expected {
            host.send(peer as u32, Packet::unreliable(1, payload.clone()));
        }
        let mut received = Vec::new();
        while received.len() < expected.len() {
            received.extend(datagram());
        }
        expected.sort();
        received.sort();
        assert_eq!(received, expected);
    }
    let bytes = relay.bytes.load(Ordering::SeqCst) - start;
    client.disconnect(0);
    bytes
}

#[test]
fn crowded_movement_halves_webtransport_ipv4_egress_bytes() {
    let legacy = measure_webtransport(false);
    let negotiated = measure_webtransport(true);
    println!(
        "39 subjects, 20 ticks, QUIC + UDP + IPv4 egress: legacy={legacy}, negotiated={negotiated}, reduction={:.2}%",
        100.0 * (1.0 - negotiated as f64 / legacy as f64)
    );
    assert!(
        negotiated * 2 <= legacy,
        "50% network-byte target missed: {negotiated}/{legacy}"
    );
}
