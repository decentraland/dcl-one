//! The ENet side of a Pulse bot: one thread, one host, one peer. It moves frames and reports
//! what the transport did; it keeps no policy of its own (no retries, no pacing, no backoff).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use rusty_enet as enet;

use super::{Channel, Command, Shared};

/// Reliable, unreliable sequenced, unreliable unsequenced: the three channels Pulse uses.
const CHANNEL_COUNT: usize = 3;
const SERVICE_TICK: Duration = Duration::from_millis(1);

fn packet(channel: Channel, bytes: &[u8]) -> enet::Packet {
    match channel {
        Channel::Reliable => enet::Packet::reliable(bytes),
        Channel::UnreliableSequenced => enet::Packet::unreliable(bytes),
        Channel::UnreliableUnsequenced => enet::Packet::unreliable_unsequenced(bytes),
    }
}

/// Binds, dials `target` and services the connection until it ends or a `Stop` arrives. Returns
/// the local address once bound, and the thread.
pub(crate) fn spawn(
    target: SocketAddr,
    bind: Option<IpAddr>,
    shared: Arc<Shared>,
    commands: Receiver<Command>,
) -> std::io::Result<(SocketAddr, std::thread::JoinHandle<()>)> {
    let bind = bind.unwrap_or(match target {
        SocketAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        SocketAddr::V6(_) => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
    });
    let socket = UdpSocket::bind((bind, 0))?;
    let local = socket.local_addr()?;
    let thread = std::thread::Builder::new()
        .name(format!("pulse-bot {}", shared.name))
        .spawn(move || run(socket, target, shared, commands))?;
    Ok((local, thread))
}

fn run(socket: UdpSocket, target: SocketAddr, shared: Arc<Shared>, commands: Receiver<Command>) {
    let mut host = match enet::Host::new(
        socket,
        enet::HostSettings {
            peer_limit: 1,
            channel_limit: CHANNEL_COUNT,
            ..Default::default()
        },
    ) {
        Ok(host) => host,
        Err(error) => return shared.transport_failed(format!("ENet host: {error:?}")),
    };
    let peer = match host.connect(target, CHANNEL_COUNT, 0) {
        Ok(peer) => peer.id(),
        Err(error) => return shared.transport_failed(format!("ENet connect: {error:?}")),
    };

    loop {
        loop {
            match commands.try_recv() {
                Ok(Command::Send { channel, bytes }) => {
                    if let Err(error) = host.peer_mut(peer).send(channel.id(), &packet(channel, &bytes)) {
                        shared.record("send failed", format!("{error:?}"));
                    }
                }
                Ok(Command::Disconnect { code }) => host.peer_mut(peer).disconnect(code),
                Ok(Command::Stop) | Err(TryRecvError::Disconnected) => {
                    host.peer_mut(peer).disconnect_now(1);
                    host.flush();
                    shared.closed_locally();
                    return;
                }
                Err(TryRecvError::Empty) => break,
            }
        }

        loop {
            match host.service() {
                Ok(Some(enet::Event::Connect { .. })) => shared.connected(),
                Ok(Some(enet::Event::Receive { packet, .. })) => {
                    for (channel, bytes) in shared.received(packet.data()) {
                        let _ = host.peer_mut(peer).send(channel.id(), &self::packet(channel, &bytes));
                    }
                }
                Ok(Some(enet::Event::Disconnect { data, .. })) => {
                    shared.disconnected(data);
                    return;
                }
                Ok(Some(_)) => {}
                Ok(None) => break,
                // A socket error is one lost datagram, not the end: ENet's own timeout decides.
                Err(error) => {
                    shared.socket_error(&error);
                    break;
                }
            }
        }
        std::thread::sleep(SERVICE_TICK);
    }
}
