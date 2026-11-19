//! A failed call on the ENet UDP socket against a running server with two connected clients: an
//! error about one datagram costs that datagram, an error that means the socket is gone stops the
//! server with that error.

use std::future::Future;
use std::io::{Error, ErrorKind};
use std::pin::Pin;
use std::time::{Duration, Instant};

use super::*;
use crate::protocol::pulse::decentraland::pulse::HandshakeRequest;
use crate::pulse::transport::host::SocketFaults;

type Client = rusty_enet::Host<std::net::UdpSocket>;
type Running = Pin<Box<dyn Future<Output = anyhow::Result<()>>>>;

const DEADLINE: Duration = Duration::from_secs(10);
const METRIC: &str = "pulse_enet_socket_errors_total";

/// An error Windows returns for a socket that is not one any more (WSAENOTSOCK), and its Unix
/// counterpart (EBADF, 9 on Linux and macOS alike).
#[cfg(windows)]
const SOCKET_GONE: i32 = 10038;
#[cfg(not(windows))]
const SOCKET_GONE: i32 = 9;

fn host_config() -> HostConfig {
    HostConfig {
        bind: "127.0.0.1:0".parse().unwrap(),
        max_peers: 4,
        channel_limit: 3,
    }
}

/// A Pulse server running its own service loop on loopback, and two ENet clients connected to it.
struct Rig {
    server: Running,
    clients: Vec<Client>,
}

impl Rig {
    async fn start(host: Host) -> Self {
        let address = host.local_addr().unwrap();
        let server: Running =
            Box::pin(PulseServer::new().serve(Transports::enet_only(host, 4), 50));
        let clients = (0..2)
            .map(|_| {
                let mut client = rusty_enet::Host::new(
                    std::net::UdpSocket::bind("127.0.0.1:0").unwrap(),
                    rusty_enet::HostSettings {
                        peer_limit: 1,
                        channel_limit: 3,
                        ..Default::default()
                    },
                )
                .unwrap();
                client.connect(address, 3, 0).unwrap();
                client
            })
            .collect();
        let mut rig = Self { server, clients };
        let mut connected = [false; 2];
        let deadline = Instant::now() + DEADLINE;
        while !connected.iter().all(|c| *c) {
            assert!(Instant::now() < deadline, "clients did not connect");
            for (index, event) in rig.step().await {
                if matches!(event, rusty_enet::EventNoRef::Connect { .. }) {
                    connected[index] = true;
                }
            }
        }
        rig
    }

    /// Lets the server's loop run for a moment, then services every client. Panics if the server
    /// stopped.
    async fn step(&mut self) -> Vec<(usize, rusty_enet::EventNoRef)> {
        tokio::select! {
            stopped = self.server.as_mut() => panic!("the server stopped: {stopped:?}"),
            _ = tokio::time::sleep(Duration::from_millis(2)) => {}
        }
        let mut events = Vec::new();
        for (index, client) in self.clients.iter_mut().enumerate() {
            while let Some(event) = client.service().unwrap() {
                events.push((index, event.no_ref()));
            }
        }
        events
    }

    async fn run_for(&mut self, duration: Duration) {
        let until = Instant::now() + duration;
        while Instant::now() < until {
            self.step().await;
        }
    }

    async fn run_until_returned(&mut self, faults: &SocketFaults) {
        let deadline = Instant::now() + DEADLINE;
        while faults.pending() > 0 {
            assert!(Instant::now() < deadline, "the socket was never called");
            self.step().await;
        }
    }

    /// Sends a handshake the server cannot parse; the server answers it with a refusal.
    fn send_unparsable_handshake(&mut self, index: usize) {
        let request = client_msg(client_message::Message::Handshake(HandshakeRequest {
            auth_chain: b"not json".to_vec(),
            profile_version: 0,
            initial_state: None,
            protocol_features: 0,
        }));
        let client = &mut self.clients[index];
        client
            .peer_mut(rusty_enet::PeerID(0))
            .send(channel::RELIABLE, &rusty_enet::Packet::reliable(request))
            .unwrap();
        client.flush();
    }

    /// The server receives this client's datagram and sends one back: it still serves the client.
    async fn handshake_is_answered(&mut self, index: usize) {
        self.send_unparsable_handshake(index);
        let deadline = Instant::now() + DEADLINE;
        loop {
            assert!(
                Instant::now() < deadline,
                "client {index}'s handshake got no answer"
            );
            for (from, event) in self.step().await {
                let rusty_enet::EventNoRef::Receive { packet, .. } = event else {
                    continue;
                };
                let answer = ServerMessage::decode(packet.data()).unwrap();
                if from == index
                    && matches!(
                        answer.message,
                        Some(server_message::Message::Handshake(HandshakeResponse {
                            success: false,
                            ..
                        }))
                    )
                {
                    return;
                }
            }
        }
    }
}

/// The value of `pulse_enet_socket_errors_total` for one operation and error kind.
fn counted(rendered: &str, operation: &str, kind: &str) -> u64 {
    rendered
        .lines()
        .filter(|line| line.starts_with(METRIC))
        .filter(|line| line.contains(&format!("operation=\"{operation}\"")))
        .filter(|line| line.contains(&format!("kind=\"{kind}\"")))
        .map(|line| line.rsplit(' ').next().unwrap().parse::<u64>().unwrap())
        .sum()
}

#[derive(Clone, Copy, Debug)]
enum Operation {
    Send,
    Receive,
}

/// Errors that concern one datagram, as each operating system reports them (see the transport's
/// `ReadySocket`): the expected `kind` label is written out by hand.
fn per_datagram_errors() -> Vec<(Operation, Error, &'static str)> {
    let mut errors = vec![
        // A peer vanished: the ICMP port unreachable its closed port sent back.
        (
            Operation::Receive,
            Error::from(ErrorKind::ConnectionReset),
            "ConnectionReset",
        ),
        (
            Operation::Receive,
            Error::from(ErrorKind::ConnectionRefused),
            "ConnectionRefused",
        ),
        // ICMP host or network unreachable for an earlier datagram.
        (
            Operation::Receive,
            Error::from(ErrorKind::HostUnreachable),
            "HostUnreachable",
        ),
        (
            Operation::Receive,
            Error::from(ErrorKind::NetworkUnreachable),
            "NetworkUnreachable",
        ),
        (
            Operation::Receive,
            Error::from(ErrorKind::Interrupted),
            "Interrupted",
        ),
        // No route to one peer's address (Linux ENETUNREACH, Windows WSAENETUNREACH).
        (
            Operation::Send,
            Error::from(ErrorKind::NetworkUnreachable),
            "NetworkUnreachable",
        ),
        // A route that refuses the address (Linux EINVAL for a blackhole route or port 0).
        (
            Operation::Send,
            Error::from(ErrorKind::InvalidInput),
            "InvalidInput",
        ),
        // A route or firewall rule that prohibits it (Linux EACCES, EPERM).
        (
            Operation::Send,
            Error::from(ErrorKind::PermissionDenied),
            "PermissionDenied",
        ),
    ];
    #[cfg(windows)]
    errors.extend([
        // The error the Windows incident reported: WSAECONNRESET after a peer vanished.
        (
            Operation::Receive,
            Error::from_raw_os_error(10054),
            "ConnectionReset",
        ),
        // WSAEMSGSIZE: a datagram larger than the receive buffer, which Linux truncates instead.
        (
            Operation::Receive,
            Error::from_raw_os_error(10040),
            "Uncategorized",
        ),
        // WSAENETRESET: the ICMP time-exceeded answer to an earlier datagram.
        (
            Operation::Receive,
            Error::from_raw_os_error(10052),
            "Uncategorized",
        ),
    ]);
    errors
}

#[tokio::test]
async fn an_error_about_one_datagram_costs_that_datagram_and_both_clients_stay_served() {
    for (operation, error, kind) in per_datagram_errors() {
        let recorder = crate::metrics::prometheus_builder()
            .unwrap()
            .build_recorder();
        let metrics = recorder.handle();
        let _recording = ::metrics::set_default_local_recorder(&recorder);
        let faults = SocketFaults::default();
        let host = Host::bind_with_faults(host_config(), faults.clone())
            .await
            .unwrap();
        let mut rig = Rig::start(host).await;

        let reported = format!("{operation:?} {error:?}");
        match operation {
            Operation::Send => faults.fail_next_send(error),
            Operation::Receive => faults.fail_next_receive(error),
        }
        rig.run_until_returned(&faults).await;
        rig.handshake_is_answered(0).await;
        rig.handshake_is_answered(1).await;

        let operation = match operation {
            Operation::Send => "send",
            Operation::Receive => "receive",
        };
        assert_eq!(
            counted(&metrics.render(), operation, kind),
            1,
            "{reported}: {}",
            metrics.render()
        );
    }
}

#[tokio::test]
async fn a_receive_error_that_means_the_socket_is_gone_stops_the_server_with_that_error() {
    let faults = SocketFaults::default();
    let host = Host::bind_with_faults(host_config(), faults.clone())
        .await
        .unwrap();
    let mut rig = Rig::start(host).await;

    faults.fail_next_receive(Error::from_raw_os_error(SOCKET_GONE));
    let stopped = tokio::time::timeout(DEADLINE, rig.server.as_mut())
        .await
        .expect("the server kept running on a socket that is gone");

    let error = stopped.expect_err("the server stopped without an error");
    let os_error = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<Error>())
        .and_then(Error::raw_os_error);
    assert_eq!(os_error, Some(SOCKET_GONE), "{error:?}");
}

/// The Windows incident, with no fault injected: a client's socket closes without an ENet
/// disconnect, and the server's next datagrams to it meet a closed port. Windows reports the ICMP
/// port unreachable on the server's next `recvfrom` (WSAECONNRESET); Linux does not report it to
/// an unconnected socket, so there the server never sees an error.
#[tokio::test]
async fn a_client_whose_socket_closed_does_not_stop_the_server() {
    let recorder = crate::metrics::prometheus_builder()
        .unwrap()
        .build_recorder();
    let metrics = recorder.handle();
    let _recording = ::metrics::set_default_local_recorder(&recorder);
    let mut rig = Rig::start(Host::bind(host_config()).await.unwrap()).await;

    // The server's answer to this goes to a port that is closed by the time it is sent.
    rig.send_unparsable_handshake(1);
    drop(rig.clients.remove(1));
    // ENet pings every 500 ms: a second holds more datagrams to the closed port.
    rig.run_for(Duration::from_secs(1)).await;
    rig.handshake_is_answered(0).await;

    if cfg!(windows) {
        assert!(
            counted(&metrics.render(), "receive", "ConnectionReset") >= 1,
            "Windows reported no closed port: {}",
            metrics.render()
        );
    }
}
