//! An extension for ENet allowing easier use with connection based networking (such as TCP, WebRTC,
//! etc).
//!
//! This module provides a very similar interface to that of the connectionless version with the
//! main difference being how interfaces are defined (using [`connected::Connection`] instead of
//! [`Socket`](`crate::Socket`)) and how connections are made (see
//! [`connected::Host::add_connection`]).
//!
//! The main purpose of this module is to handle the awkward state where both sides are already
//! technically connected, but haven't communicated yet, as well as juggling between multiple
//! sockets instead of just the one (as you would with a connectionless socket).

use core::{mem::replace, time::Duration};

#[cfg(feature = "std")]
use std::{
    io,
    net::{SocketAddr, TcpStream},
};

use crate::Vec;

#[cfg(doc)]
use crate::connected;

/// Errors for [`connected::Host::add_connection`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AddConnectionError<C: Connection> {
    /// Failed to add connection because there were no available ENet connection slots.
    NoAvailablePeers,
    /// Failed to initialize the connection ([`connected::Connection::init`] failed).
    FailedToInitializeConnection(C::Error),
}

impl<C: Connection> core::fmt::Debug for AddConnectionError<C> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            AddConnectionError::NoAvailablePeers => f.write_str("NoAvailablePeers"),
            AddConnectionError::FailedToInitializeConnection(err) => f
                .debug_tuple("FailedToInitializeConnection")
                .field(&err)
                .finish(),
        }
    }
}

impl<C: Connection> core::fmt::Display for AddConnectionError<C> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            AddConnectionError::NoAvailablePeers => f.write_str(
                "Failed to add connection because there were no available ENet connection slots.",
            ),
            AddConnectionError::FailedToInitializeConnection(_) => {
                f.write_str("Failed to initialize the connection.")
            }
        }
    }
}

#[derive(Clone)]
struct Address<A: crate::Address> {
    address: A,
    id: ConnectionID,
    generation: usize,
}

impl<A: crate::Address> Address<A> {
    fn new(address: A, index: ConnectionID, generation: usize) -> Self {
        Self {
            address,
            id: index,
            generation,
        }
    }

    fn connection_id(&self) -> ConnectionID {
        self.id
    }
}

impl<A: crate::Address> crate::Address for Address<A> {
    fn same(&self, other: &Self) -> bool {
        self.id == other.id
            && self.generation == other.generation
            && A::same(&self.address, &other.address)
    }

    fn same_host(&self, other: &Self) -> bool {
        self.id == other.id
            && self.generation == other.generation
            && A::same_host(&self.address, &other.address)
    }

    fn is_broadcast(&self) -> bool {
        A::is_broadcast(&self.address)
    }
}

/// A trait for implementing connection based sockets, similar to [`Socket`](`crate::Socket`).
///
/// An implementation for [`std::net::TcpStream`] is provided out of the box.
///
/// An implementation of [`ReadWrite`](`crate::ReadWrite`) is provided with an address type of `()`.
#[allow(clippy::missing_errors_doc)]
pub trait Connection {
    /// The address type to use, which must implement [`Address`](`crate::Address`).
    type Address: crate::Address;
    /// Errors returned by this connection.
    type Error: crate::SocketError;

    /// Initialize the socket with options passed down by ENet.
    ///
    /// Called in [`connected::Host::add_connection`]. If this function returns an error, it is
    /// bubbled up through [`connected::Host::add_connection`].
    fn init(&mut self) -> Result<Self::Address, Self::Error>;

    /// Try to send data. Should return the number of bytes successfully sent, or an error.
    fn send(&mut self, buffer: &[u8]) -> Result<usize, Self::Error>;

    /// Try to receive data from the socket into a buffer of size [`MTU_MAX`](`crate::MTU_MAX`).
    ///
    /// A received packet should be written into the provided buffer. If a packet is received that
    /// is larger than [`MTU_MAX`](`crate::MTU_MAX`), it should simply be discarded. ENet will never
    /// send a packet that is larger than this maximum, so if one is received, it was not sent by
    /// ENet.
    ///
    /// The return value should be `Ok(None)` if no packet was received. If a packet was received,
    /// the amount of bytes received should be returned. Packets received may be complete or
    /// partial. See [`PacketReceived`](`crate::PacketReceived`) for more info.
    fn receive(
        &mut self,
        buffer: &mut [u8; crate::MTU_MAX],
    ) -> Result<Option<crate::PacketReceived>, Self::Error>;
}

impl<E: crate::SocketError> Connection for crate::ReadWrite<(), E> {
    type Address = ();
    type Error = E;

    fn init(&mut self) -> Result<Self::Address, Self::Error> {
        Ok(())
    }

    fn send(&mut self, buffer: &[u8]) -> Result<usize, Self::Error> {
        crate::Socket::send(self, (), buffer)
    }

    fn receive(
        &mut self,
        buffer: &mut [u8; crate::MTU_MAX],
    ) -> Result<Option<crate::PacketReceived>, Self::Error> {
        Ok(crate::Socket::receive(self, buffer)?.map(|(_, received)| received))
    }
}

type SocketInterface<C> =
    crate::ReadWrite<Address<<C as Connection>::Address>, <C as Connection>::Error>;

/// In ENet, one peer must establish a connection to the other peer. This can be a little confusing
/// in a connection based environment, where this has already happened. Still, one side needs to
/// initiate what is essentially a handshake, and the other needs to receive it.
///
/// The initiator provides details for [`Host::connect`](`crate::Host::connect`) to start the
/// handshake, while the receiver (which simply awaits this handshake) provides a timeout in case
/// that handshake never arrives. If the receiver times out, an
/// [`connected::Event::Disconnect`] is fired for that connection.
///
/// The `channel_count` and `data` correspond with those in
/// [`Host::connect`](`crate::Host::connect`).
pub enum ConnectionKind<C: Connection> {
    /// Initiate a connection. This is usually from the TCP connector or WebRTC offerer.
    Initiator {
        /// The connection object to send to and receive from.
        connection: C,
        /// The number of ENet channels to allow.
        channel_count: usize,
        /// Data sent to the remote peer on connect.
        data: u32,
    },
    /// Receive a connection. This is usually from the TCP listener or WebRTC answerer.
    Receiver {
        /// The connection object to send to and receive from.
        connection: C,
        /// The amount of time to wait for the initiator before giving up.
        timeout: Duration,
    },
}

/// A newtype around a `usize`, representing a unique identifier for a connection based peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConnectionID(pub usize);

#[derive(Default)]
struct PeerSettings {
    timeout: Option<(u32, u32, u32)>,
    ping_interval: Option<u32>,
    throttle: Option<(u32, u32, u32)>,
    mtu: Option<u16>,
}

impl PeerSettings {
    fn apply<C: Connection + 'static>(&self, peer: &mut crate::Peer<SocketInterface<C>>) {
        if let Some((limit, minimum, maximum)) = self.timeout {
            peer.set_timeout(limit, minimum, maximum);
        }
        if let Some(ping_interval) = self.ping_interval {
            peer.set_ping_interval(ping_interval);
        }
        if let Some((interval, acceleration, deceleration)) = self.throttle {
            peer.set_throttle(interval, acceleration, deceleration);
        }
        if let Some(mtu) = self.mtu {
            _ = peer.set_mtu(mtu);
        }
    }
}

enum PeerState<C: Connection + 'static> {
    Disconnected {
        last_peer_ptr: Option<*mut crate::Peer<SocketInterface<C>>>,
    },
    AwaitingPeer {
        connection: C,
        address: Address<C::Address>,
        since: Duration,
        timeout: Duration,
        settings: PeerSettings,
    },
    HasPeer {
        connection: C,
        address: Address<C::Address>,
        peer_ptr: *mut crate::Peer<SocketInterface<C>>,
    },
    Disconnecting {
        connection: C,
        address: Address<C::Address>,
        last_peer_ptr: Option<*mut crate::Peer<SocketInterface<C>>>,
        last_send: Duration,
    },
}

/// A peer, associated with a [`connected::Host`], which may or may not be connected.
///
/// To check on the connectivity of a peer, see [`connected::Peer::state`].
pub struct Peer<C: Connection + 'static> {
    id: ConnectionID,
    state: PeerState<C>,
}

impl<C: Connection> Peer<C> {
    fn peer(&self) -> Option<&mut crate::Peer<SocketInterface<C>>> {
        match self.state {
            PeerState::HasPeer { peer_ptr, .. } => unsafe { Some(&mut *peer_ptr) },
            _ => None,
        }
    }

    fn peer_or_last_peer(&self) -> Option<&mut crate::Peer<SocketInterface<C>>> {
        match self.state {
            PeerState::Disconnected { last_peer_ptr, .. } => unsafe {
                last_peer_ptr.map(|peer_ptr| &mut *peer_ptr)
            },
            PeerState::HasPeer { peer_ptr, .. } => unsafe { Some(&mut *peer_ptr) },
            PeerState::Disconnecting { last_peer_ptr, .. } => unsafe {
                last_peer_ptr.map(|peer_ptr| &mut *peer_ptr)
            },
            _ => None,
        }
    }

    fn settings(&mut self) -> Option<&mut PeerSettings> {
        match &mut self.state {
            PeerState::AwaitingPeer { settings, .. } => Some(settings),
            _ => None,
        }
    }

    /// Get the [`connected::ConnectionID`] of this peer.
    #[must_use]
    pub fn id(&self) -> ConnectionID {
        self.id
    }

    /// Get a reference to the underlying connection.
    pub fn connection(&self) -> Option<&C> {
        match &self.state {
            PeerState::Disconnected { .. } => None,
            PeerState::AwaitingPeer { connection, .. } => Some(connection),
            PeerState::HasPeer { connection, .. } => Some(connection),
            PeerState::Disconnecting { connection, .. } => Some(connection),
        }
    }

    /// Get a mutable reference to the underlying connection.
    pub fn connection_mut(&mut self) -> Option<&mut C> {
        match &mut self.state {
            PeerState::Disconnected { .. } => None,
            PeerState::AwaitingPeer { connection, .. } => Some(connection),
            PeerState::HasPeer { connection, .. } => Some(connection),
            PeerState::Disconnecting { connection, .. } => Some(connection),
        }
    }

    /// See [`Peer::ping`](`crate::Peer::ping`).
    pub fn ping(&mut self) {
        if let Some(peer) = self.peer() {
            peer.ping();
        }
    }

    /// See [`Peer::send`](`crate::Peer::ping`).
    ///
    /// # Errors
    ///
    /// May return any of the [`error::PeerSendError`](`crate::error::PeerSendError`)) variants on
    /// failure.
    pub fn send(
        &mut self,
        channel_id: u8,
        packet: &crate::Packet,
    ) -> Result<(), crate::error::PeerSendError> {
        self.peer()
            .map_or(Err(crate::error::PeerSendError::NotConnected), |peer| {
                peer.send(channel_id, packet)
            })
    }

    /// See [`Peer::disconnect`](`crate::Peer::disconnect`).
    pub fn disconnect(&mut self, data: u32) {
        if let Some(peer) = self.peer() {
            peer.disconnect(data);
        } else {
            self.state = PeerState::Disconnected {
                last_peer_ptr: None,
            };
        }
    }

    /// See [`Peer::disconnect_now`](`crate::Peer::disconnect_now`).
    pub fn disconnect_now(&mut self, data: u32) {
        if let Some(peer) = self.peer() {
            peer.disconnect_now(data);
        } else {
            self.state = PeerState::Disconnected {
                last_peer_ptr: None,
            };
        }
    }

    /// See [`Peer::disconnect_later`](`crate::Peer::disconnect_later`).
    pub fn disconnect_later(&mut self, data: u32) {
        if let Some(peer) = self.peer() {
            peer.disconnect_later(data);
        } else {
            self.state = PeerState::Disconnected {
                last_peer_ptr: None,
            };
        }
    }

    /// See [`Peer::reset`](`crate::Peer::reset`).
    pub fn reset(&mut self) {
        if let Some(peer) = self.peer() {
            peer.reset();
        }
        self.state = PeerState::Disconnected {
            last_peer_ptr: None,
        };
    }

    /// See [`Peer::set_timeout`](`crate::Peer::set_timeout`).
    pub fn set_timeout(&mut self, limit: u32, minimum: u32, maximum: u32) {
        if let Some(peer) = self.peer() {
            peer.set_timeout(limit, minimum, maximum);
        } else if let Some(settings) = self.settings() {
            settings.timeout = Some((limit, minimum, maximum));
        }
    }

    /// See [`Peer::set_ping_interval`](`crate::Peer::set_ping_interval`).
    pub fn set_ping_interval(&mut self, ping_interval: u32) {
        if let Some(peer) = self.peer() {
            peer.set_ping_interval(ping_interval);
        } else if let Some(settings) = self.settings() {
            settings.ping_interval = Some(ping_interval);
        }
    }

    /// See [`Peer::set_throttle`](`crate::Peer::set_throttle`).
    pub fn set_throttle(&mut self, interval: u32, acceleration: u32, deceleration: u32) {
        if let Some(peer) = self.peer() {
            peer.set_throttle(interval, acceleration, deceleration);
        } else if let Some(settings) = self.settings() {
            settings.throttle = Some((interval, acceleration, deceleration));
        }
    }

    /// See [`Peer::mtu`](`crate::Peer::mtu`).
    #[must_use]
    pub fn mtu(&self) -> u16 {
        self.peer_or_last_peer().map_or(0, |peer| peer.mtu())
    }

    /// See [`Peer::set_mtu`](`crate::Peer::set_mtu`).
    ///
    /// # Errors
    ///
    /// Returns [`error::BadParameter`](`crate::error::BadParameter`) if `mtu` is greater than
    /// [`consts::PROTOCOL_MAXIMUM_MTU`](`crate::consts::PROTOCOL_MAXIMUM_MTU`) or less than
    /// [`consts::PROTOCOL_MINIMUM_MTU`](`crate::consts::PROTOCOL_MINIMUM_MTU`).
    pub fn set_mtu(&mut self, mtu: u16) -> Result<(), crate::error::BadParameter> {
        if mtu > crate::consts::PROTOCOL_MAXIMUM_MTU as u16
            || mtu < crate::consts::PROTOCOL_MINIMUM_MTU as u16
        {
            return Err(crate::error::BadParameter {
                method: "ConnectedPeer::set_mtu",
                parameter: "mtu",
            });
        }
        #[allow(clippy::option_if_let_else)]
        if let Some(peer) = self.peer() {
            peer.set_mtu(mtu)
        } else if let Some(settings) = self.settings() {
            settings.mtu = Some(mtu);
            Ok(())
        } else {
            Ok(())
        }
    }

    /// See [`Peer::state`](`crate::Peer::state`).
    #[must_use]
    pub fn state(&self) -> crate::PeerState {
        self.peer_or_last_peer()
            .map_or(crate::PeerState::Disconnected, |peer| peer.state())
    }

    /// See [`Peer::connected`](`crate::Peer::connected`).
    #[must_use]
    pub fn connected(&self) -> bool {
        self.state() == crate::PeerState::Connected
    }

    /// See [`Peer::channel_count`](`crate::Peer::channel_count`).
    #[must_use]
    pub fn channel_count(&self) -> usize {
        self.peer_or_last_peer()
            .map_or(0, |peer| peer.channel_count())
    }

    /// See [`Peer::incoming_bandwidth`](`crate::Peer::incoming_bandwidth`).
    #[must_use]
    pub fn incoming_bandwidth(&self) -> u32 {
        self.peer_or_last_peer()
            .map_or(0, |peer| peer.incoming_bandwidth())
    }

    /// See [`Peer::outgoing_bandwidth`](`crate::Peer::outgoing_bandwidth`).
    #[must_use]
    pub fn outgoing_bandwidth(&self) -> u32 {
        self.peer_or_last_peer()
            .map_or(0, |peer| peer.outgoing_bandwidth())
    }

    /// See [`Peer::incoming_data_total`](`crate::Peer::incoming_data_total`).
    #[must_use]
    pub fn incoming_data_total(&self) -> u32 {
        self.peer_or_last_peer()
            .map_or(0, |peer| peer.incoming_data_total())
    }

    /// See [`Peer::outgoing_data_total`](`crate::Peer::outgoing_data_total`).
    #[must_use]
    pub fn outgoing_data_total(&self) -> u32 {
        self.peer_or_last_peer()
            .map_or(0, |peer| peer.outgoing_data_total())
    }

    /// See [`Peer::packets_sent`](`crate::Peer::packets_sent`).
    #[must_use]
    pub fn packets_sent(&self) -> u32 {
        self.peer_or_last_peer()
            .map_or(0, |peer| peer.packets_sent())
    }

    /// See [`Peer::packets_lost`](`crate::Peer::packets_lost`).
    #[must_use]
    pub fn packets_lost(&self) -> u32 {
        self.peer_or_last_peer()
            .map_or(0, |peer| peer.packets_lost())
    }

    /// See [`Peer::packet_loss`](`crate::Peer::packet_loss`).
    #[must_use]
    pub fn packet_loss(&self) -> u32 {
        self.peer_or_last_peer()
            .map_or(0, |peer| peer.packet_loss())
    }

    /// See [`Peer::packet_loss_variance`](`crate::Peer::packet_loss_variance`).
    #[must_use]
    pub fn packet_loss_variance(&self) -> u32 {
        self.peer_or_last_peer()
            .map_or(0, |peer| peer.packet_loss_variance())
    }

    /// See [`Peer::ping_interval`](`crate::Peer::ping_interval`).
    #[must_use]
    pub fn ping_interval(&self) -> Duration {
        self.peer_or_last_peer()
            .map_or(Duration::ZERO, |peer| peer.ping_interval())
    }

    /// See [`Peer::round_trip_time`](`crate::Peer::round_trip_time`).
    #[must_use]
    pub fn round_trip_time(&self) -> Duration {
        self.peer_or_last_peer()
            .map_or(Duration::ZERO, |peer| peer.round_trip_time())
    }

    /// See [`Peer::round_trip_time_variance`](`crate::Peer::round_trip_time_variance`).
    #[must_use]
    pub fn round_trip_time_variance(&self) -> Duration {
        self.peer_or_last_peer()
            .map_or(Duration::ZERO, |peer| peer.round_trip_time_variance())
    }

    /// See [`Peer::address`](`crate::Peer::address`).
    #[must_use]
    pub fn address(&self) -> Option<C::Address> {
        self.peer().map_or_else(
            || None,
            |peer| peer.address().map(|address| address.address),
        )
    }
}

/// An ENet event returned by [`connected::Host::service`].
pub enum Event<'a, C: Connection + 'static> {
    /// A new peer has connected.
    Connect {
        /// Peer that generated the event.
        peer: &'a mut Peer<C>,
        /// Data associated with the event, sent by the peer on connect.
        data: u32,
    },
    /// A peer has disconnected.
    Disconnect {
        /// Peer that generated the event.
        peer: &'a mut Peer<C>,
        /// Data associated with the event, sent by the peer on disconnect.
        data: u32,
    },
    /// A peer sent a packet to us.
    Receive {
        /// Peer that generated the event.
        peer: &'a mut Peer<C>,
        /// Channel the peer sent the packet on.
        channel_id: u8,
        /// The actual packet data.
        packet: crate::Packet,
    },
    /// An incoming command exceeded the optional receive policy.
    ReceiveLimitExceeded {
        /// Peer that generated the event.
        peer: &'a mut Peer<C>,
        /// Channel of the refused command.
        channel_id: u8,
        /// The exceeded limit; only packet size is terminal.
        kind: crate::ReceiveLimitKind,
    },
}

impl<'a, C: Connection + 'static> Event<'a, C> {
    /// Remove the peer reference from this event, converting into an
    /// [`connected::EventNoRef`].
    #[must_use]
    pub fn no_ref(self) -> EventNoRef {
        match self {
            Self::ReceiveLimitExceeded {
                peer,
                channel_id,
                kind,
            } => EventNoRef::ReceiveLimitExceeded {
                peer: peer.id(),
                channel_id,
                kind,
            },
            Self::Connect { peer, data } => EventNoRef::Connect {
                peer: peer.id(),
                data,
            },
            Self::Disconnect { peer, data } => EventNoRef::Disconnect {
                peer: peer.id(),
                data,
            },
            Self::Receive {
                peer,
                channel_id,
                packet,
            } => EventNoRef::Receive {
                peer: peer.id(),
                channel_id,
                packet,
            },
        }
    }
}

/// An ENet event, like [`Event`](`crate::Event`), but without peer references.
///
/// Acquired with [`connected::Event::no_ref`].
#[derive(Debug, Clone)]
pub enum EventNoRef {
    /// A new peer has connected.
    Connect {
        /// Peer that generated the event.
        peer: ConnectionID,
        /// Data associated with the event, sent by the peer on connect.
        data: u32,
    },
    /// A peer has disconnected.
    Disconnect {
        /// Peer that generated the event.
        peer: ConnectionID,
        /// Data associated with the event, sent by the peer on disconnect.
        data: u32,
    },
    /// A peer sent a packet to us.
    Receive {
        /// Peer that generated the event.
        peer: ConnectionID,
        /// Channel the peer sent the packet on.
        channel_id: u8,
        /// The actual packet data.
        packet: crate::Packet,
    },
    /// An incoming command exceeded the optional receive policy.
    ReceiveLimitExceeded {
        /// Peer that generated the event.
        peer: ConnectionID,
        /// Channel of the refused command.
        channel_id: u8,
        /// The exceeded limit; only packet size is terminal.
        kind: crate::ReceiveLimitKind,
    },
}

/// A host for communicating with connection based peers.
///
/// Rather than [`connect`](`crate::Host::connect`), this type of host uses
/// [`add_connection`](`Host::add_connection`).
///
/// Requires a [`connected::Connection`] implementation.
pub struct Host<C: Connection + 'static> {
    host: crate::Host<SocketInterface<C>>,
    peers: Vec<Peer<C>>,
    next_generation: usize,
}

impl<C: Connection> Host<C> {
    /// Creates a host for communicating to connection based peers.
    ///
    /// Supports [`std::net::TcpStream`] out of the box, but other transport protocols can be
    /// provided by implementing the [`connected::Connection`] trait.
    ///
    /// # Errors
    ///
    /// Returns [`error::BadParameter`](`crate::error::BadParameter`) if one of the host settings
    /// was invalid:
    /// - If any configured [`crate::ReceivePolicy`] limit is `0`.
    /// - If [`HostSettings::channel_limit`](`crate::HostSettings::channel_limit`) is equal to `0`.
    /// - If [`HostSettings::incoming_bandwidth_limit`](`crate::HostSettings::incoming_bandwidth_limit`)
    ///   is equal to `Some(0)`.
    /// - If [`HostSettings::outgoing_bandwidth_limit`](`crate::HostSettings::outgoing_bandwidth_limit`)
    ///   is equal to `Some(0)`.
    /// - If [`HostSettings::peer_limit`](`crate::HostSettings::peer_limit`) is equal to `0` or
    ///   greater than [`consts::PROTOCOL_MAXIMUM_PEER_ID`](`crate::consts::PROTOCOL_MAXIMUM_PEER_ID`).
    pub fn new(settings: crate::HostSettings) -> Result<Self, crate::error::BadParameter> {
        let mut peers = Vec::new();
        peers.reserve_exact(settings.peer_limit);
        for index in 0..settings.peer_limit {
            peers.push(Peer {
                id: ConnectionID(index),
                state: PeerState::Disconnected {
                    last_peer_ptr: None,
                },
            });
        }
        Ok(Self {
            host: crate::Host::new(crate::ReadWrite::new(), settings).map_err(|err| match err {
                crate::error::HostNewError::BadParameter(err) => err,
                crate::error::HostNewError::FailedToInitializeSocket(..) => unreachable!(),
            })?,
            peers,
            next_generation: 0,
        })
    }

    /// Adds a connection for this host to manage. The connection should already be established on
    /// both sides.
    ///
    /// See [`connected::ConnectionKind`] for more information on how connections are established.
    ///
    /// # Errors
    ///
    /// Returns [`connected::AddConnectionError::NoAvailablePeers`] if the connection kind was
    /// [`connected::ConnectionKind::Initiator`], but there were no peer slots available for the
    /// connection.
    ///
    /// Returns [`connected::AddConnectionError::FailedToInitializeConnection`] with the
    /// connection error if the call to [`connected::Connection::init`] fails.
    pub fn add_connection(
        &mut self,
        kind: ConnectionKind<C>,
    ) -> Result<&mut Peer<C>, AddConnectionError<C>> {
        let mut connection_id = None;
        for (index, peer) in self.peers.iter().enumerate() {
            if let PeerState::Disconnected { .. } = &peer.state {
                connection_id = Some(ConnectionID(index));
                break;
            }
        }
        let Some(connection_id) = connection_id else {
            return Err(AddConnectionError::NoAvailablePeers);
        };
        match kind {
            ConnectionKind::Initiator {
                mut connection,
                channel_count,
                data,
            } => {
                let generation = self.next_generation;
                self.next_generation += 1;
                let address = Address::new(
                    connection
                        .init()
                        .map_err(|err| AddConnectionError::FailedToInitializeConnection(err))?,
                    connection_id,
                    generation,
                );
                if let Ok(peer_ptr) = self.host.connect(address.clone(), channel_count, data) {
                    self.peers[connection_id.0].state = PeerState::HasPeer {
                        connection,
                        address,
                        peer_ptr,
                    };
                } else {
                    return Err(AddConnectionError::NoAvailablePeers);
                }
            }
            ConnectionKind::Receiver {
                mut connection,
                timeout,
            } => {
                let generation = self.next_generation;
                self.next_generation += 1;
                let address = Address::new(
                    connection
                        .init()
                        .map_err(|err| AddConnectionError::FailedToInitializeConnection(err))?,
                    connection_id,
                    generation,
                );
                self.peers[connection_id.0].state = PeerState::AwaitingPeer {
                    connection,
                    address,
                    since: self.host.now(),
                    timeout,
                    settings: PeerSettings::default(),
                };
            }
        }
        Ok(self.peer_mut(connection_id))
    }

    // A terminal declaration can replace Connect in the final handshake datagram.
    // Associate that exact native peer before exposing either event to its caller.
    fn attach_native_peer(&mut self, native_id: crate::PeerID) -> Option<ConnectionID> {
        let peer = self.host.peer_mut(native_id);
        let peer_ptr = peer as *mut _;
        let Some(native_address) = peer.address() else {
            peer.reset();
            return None;
        };
        let connection = native_address.connection_id();
        // Another connection's early return can leave an old-generation datagram
        // staged after its AwaitingPeer wrapper has expired or been replaced.
        let owns_native = self
            .peers
            .get(connection.0)
            .is_some_and(|peer| match &peer.state {
                PeerState::AwaitingPeer { address, .. } => {
                    crate::Address::same(address, &native_address)
                }
                PeerState::HasPeer {
                    address,
                    peer_ptr: current,
                    ..
                } => crate::Address::same(address, &native_address) && *current == peer_ptr,
                PeerState::Disconnected { .. } | PeerState::Disconnecting { .. } => false,
            });
        if !owns_native {
            // Reset only the event's native peer. The replacement wrapper and its
            // potentially different native peer must not receive or act on it.
            self.host.peer_mut(native_id).reset();
            return None;
        }
        let connection_peer = self.peer_mut(connection);
        connection_peer.state = match replace(
            &mut connection_peer.state,
            PeerState::Disconnected {
                last_peer_ptr: None,
            },
        ) {
            PeerState::AwaitingPeer {
                connection,
                address,
                settings,
                ..
            } => {
                unsafe { settings.apply::<C>(&mut *peer_ptr) };
                PeerState::HasPeer {
                    connection,
                    address,
                    peer_ptr,
                }
            }
            PeerState::HasPeer {
                connection,
                address,
                peer_ptr,
            } => PeerState::HasPeer {
                connection,
                address,
                peer_ptr,
            },
            _ => unreachable!(),
        };
        Some(connection)
    }

    fn handle_event(&mut self, event: crate::EventNoRef) -> Option<Event<C>> {
        let now = self.host.now();
        Some(match event {
            crate::EventNoRef::ReceiveLimitExceeded {
                peer,
                channel_id,
                kind,
            } => {
                let connection = self.attach_native_peer(peer)?;
                Event::ReceiveLimitExceeded {
                    peer: self.peer_mut(connection),
                    channel_id,
                    kind,
                }
            }
            crate::EventNoRef::Connect { peer, data } => {
                let connection = self.attach_native_peer(peer)?;
                Event::Connect {
                    peer: self.peer_mut(connection),
                    data,
                }
            }
            crate::EventNoRef::Disconnect { peer, data } => {
                let peer = self.host.peer_mut(peer);
                let connection_id = peer
                    .address()
                    .expect("Peer should have an address.")
                    .connection_id();
                let peer = self.peer_mut(connection_id);
                peer.state = match replace(
                    &mut peer.state,
                    PeerState::Disconnected {
                        last_peer_ptr: None,
                    },
                ) {
                    PeerState::AwaitingPeer {
                        connection,
                        address,
                        ..
                    } => PeerState::Disconnecting {
                        connection,
                        address,
                        last_peer_ptr: None,
                        last_send: now,
                    },
                    PeerState::HasPeer {
                        connection,
                        address,
                        peer_ptr,
                        ..
                    } => PeerState::Disconnecting {
                        connection,
                        address,
                        last_peer_ptr: Some(peer_ptr),
                        last_send: now,
                    },
                    _ => unreachable!(),
                };
                Event::Disconnect {
                    peer: self.peer_mut(connection_id),
                    data,
                }
            }
            crate::EventNoRef::Receive {
                peer,
                channel_id,
                packet,
            } => {
                let peer = self.host.peer_mut(peer);
                let connection = peer
                    .address()
                    .expect("Peer should have an address.")
                    .connection_id();
                Event::Receive {
                    peer: self.peer_mut(connection),
                    channel_id,
                    packet,
                }
            }
        })
    }

    /// Checks for any queued events on the host and dispatches one if available.
    pub fn check_events(&mut self) -> Option<Event<C>> {
        #[allow(clippy::option_if_let_else)]
        match self.host.check_events() {
            Some(event) => {
                let event = event.no_ref();
                self.handle_event(event)
            }
            None => None,
        }
    }

    /// Checks for events on the host and shuttles packets between the host and its peers.
    ///
    /// Should be called fairly regularly for adequate performance.
    pub fn service(&mut self) -> Option<Event<C>> {
        let now = self.host.now();
        let mut disconnect_event = None;
        for peer in &mut self.peers {
            if let PeerState::AwaitingPeer { since, timeout, .. } = &mut peer.state {
                if *since + *timeout < now {
                    peer.state = PeerState::Disconnected {
                        last_peer_ptr: None,
                    };
                    disconnect_event = Some(peer.id);
                    break;
                }
            }
            if let PeerState::Disconnecting {
                last_send,
                last_peer_ptr,
                ..
            } = &mut peer.state
            {
                if *last_send + Duration::from_secs(2) < now {
                    peer.state = PeerState::Disconnected {
                        last_peer_ptr: last_peer_ptr.take(),
                    };
                }
            }
            match &mut peer.state {
                PeerState::Disconnected { .. } => {}
                PeerState::AwaitingPeer {
                    connection,
                    address,
                    ..
                }
                | PeerState::HasPeer {
                    connection,
                    address,
                    ..
                }
                | PeerState::Disconnecting {
                    connection,
                    address,
                    ..
                } => {
                    let mut buffer = [0; crate::MTU_MAX];
                    match connection.receive(&mut buffer) {
                        Ok(Some(crate::PacketReceived::Complete(size))) => {
                            self.host
                                .socket_mut()
                                .write(address.clone(), (buffer[0..size]).to_vec());
                        }
                        Err(_) => {
                            peer.reset();
                            disconnect_event = Some(peer.id);
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }
        if let Some(connection) = disconnect_event {
            return Some(Event::Disconnect {
                peer: self.peer_mut(connection),
                data: 0,
            });
        }
        while let Some((packet_address, packet)) = self.host.socket_mut().read() {
            if let Some(peer) = self.peers.get_mut(packet_address.id.0) {
                let mut disconnect = false;
                if let Peer {
                    state:
                        PeerState::AwaitingPeer {
                            connection,
                            address,
                            ..
                        }
                        | PeerState::HasPeer {
                            connection,
                            address,
                            ..
                        },
                    ..
                } = peer
                {
                    if packet_address.generation == address.generation
                        && connection.send(&packet).is_err()
                    {
                        disconnect = true;
                    }
                }
                if let Peer {
                    state:
                        PeerState::Disconnecting {
                            connection,
                            address,
                            last_send,
                            ..
                        },
                    ..
                } = peer
                {
                    if packet_address.generation == address.generation
                        && connection.send(&packet).is_ok()
                    {
                        *last_send = now;
                    }
                }
                if disconnect {
                    peer.disconnect(0);
                }
            }
        }
        match self.host.service() {
            Ok(Some(event)) => {
                let event = event.no_ref();
                self.handle_event(event)
            }
            Ok(None) => None,
            Err(_) => {
                // Ignoring errors here is fine. It means one of the sockets errored while servicing
                // the host. Most likely, the socket is dead, and will timeout.
                // We don't known which socket because of limitations in the ReadWrite API's Error
                // type, that could maybe be improved upon.
                None
            }
        }
    }

    /// See [`Host::flush`](`crate::Host::flush`).
    pub fn flush(&mut self) {
        self.host.flush();
    }

    /// See [`Host::peer_limit`](`crate::Host::peer_limit`).
    #[must_use]
    pub fn peer_limit(&self) -> usize {
        self.host.peer_limit()
    }

    /// Get a reference to a single peer, if it exists.
    ///
    /// # Note
    ///
    /// Acquires the peer object, even if the peer is not in a connected state.
    ///
    /// # Panics
    ///
    /// Panics if the connection ID is outside the bounds of peers allocated for this host. Use
    /// [`connected::Host::get_peer`] for a non-panicking version.
    #[must_use]
    pub fn peer(&self, connection: ConnectionID) -> &Peer<C> {
        self.peers
            .get(connection.0)
            .expect("Expected the connection id to be in bounds.")
    }

    /// Get a reference to a single peer.
    ///
    /// # Note
    ///
    /// Acquires the peer object, even if the peer is not in a connected state.
    #[must_use]
    pub fn get_peer(&self, connection: ConnectionID) -> Option<&Peer<C>> {
        self.peers.get(connection.0)
    }

    /// Get a mutable reference to a single peer.
    ///
    /// # Note
    ///
    /// Acquires the peer object, even if the peer is not in a connected state.
    ///
    /// # Panics
    ///
    /// Panics if the connection ID is outside the bounds of peers allocated for this host. Use
    /// [`connected::Host::get_peer_mut`] for a non-panicking version.
    #[must_use]
    pub fn peer_mut(&mut self, connection: ConnectionID) -> &mut Peer<C> {
        self.peers
            .get_mut(connection.0)
            .expect("Expected the connection id to be in bounds.")
    }

    /// Get a mutable reference to a single peer, if it exists.
    ///
    /// # Note
    ///
    /// Acquires the peer object, even if the peer is not in a connected state.
    #[must_use]
    pub fn get_peer_mut(&mut self, connection: ConnectionID) -> Option<&mut Peer<C>> {
        self.peers.get_mut(connection.0)
    }

    /// Iterate over all peer objects.
    ///
    /// # Note
    ///
    /// Acquires the peer objects, even if the peers are not in a connected state. Use
    /// [`connected::Host::connected_peers`] for only connected peers.
    pub fn peers(&mut self) -> impl Iterator<Item = &Peer<C>> {
        self.peers.iter()
    }

    /// Mutably iterate over all peer objects.
    ///
    /// # Note
    ///
    /// Acquires the peer objects, even if the peers are not in a connected state. Use
    /// [`connected::Host::connected_peers_mut`] for only connected peers.
    pub fn peers_mut(&mut self) -> impl Iterator<Item = &mut Peer<C>> {
        self.peers.iter_mut()
    }

    /// Iterate over all connected peers.
    pub fn connected_peers(&mut self) -> impl Iterator<Item = &Peer<C>> {
        self.peers.iter().filter(|peer| peer.connected())
    }

    /// Mutably iterate over all connected peers.
    pub fn connected_peers_mut(&mut self) -> impl Iterator<Item = &mut Peer<C>> {
        self.peers.iter_mut().filter(|peer| peer.connected())
    }

    /// See [`Host::broadcast`](`crate::Host::broadcast`).
    pub fn broadcast(&mut self, channel_id: u8, packet: &crate::Packet) {
        self.host.broadcast(channel_id, packet);
    }

    /// See [`Host::channel_limit`](`crate::Host::channel_limit`).
    #[must_use]
    pub fn channel_limit(&self) -> usize {
        self.host.channel_limit()
    }

    /// See [`Host::set_channel_limit`](`crate::Host::set_channel_limit`).
    ///
    /// # Errors
    ///
    /// Returns [`error::BadParameter`](`crate::error::BadParameter`) if `channel_limit` is `0`.
    pub fn set_channel_limit(
        &mut self,
        channel_limit: usize,
    ) -> Result<(), crate::error::BadParameter> {
        self.host.set_channel_limit(channel_limit)
    }

    /// See [`Host::bandwidth_limit`](`crate::Host::bandwidth_limit`).
    #[must_use]
    pub fn bandwidth_limit(&self) -> (Option<u32>, Option<u32>) {
        self.host.bandwidth_limit()
    }

    /// See [`Host::set_bandwidth_limit`](`crate::Host::set_bandwidth_limit`).
    ///
    /// # Errors
    ///
    /// Returns [`error::BadParameter`](`crate::error::BadParameter`) if `incoming_bandwidth_limit`
    /// or `outgoing_bandwidth_limit` is `Some(0)`.
    pub fn set_bandwidth_limit(
        &mut self,
        incoming_bandwidth_limit: Option<u32>,
        outgoing_bandwidth_limit: Option<u32>,
    ) -> Result<(), crate::error::BadParameter> {
        self.host
            .set_bandwidth_limit(incoming_bandwidth_limit, outgoing_bandwidth_limit)
    }

    /// See [`Host::mtu`](`crate::Host::mtu`).
    #[must_use]
    pub fn mtu(&self) -> u16 {
        self.host.mtu()
    }

    /// See [`Host::set_mtu`](`crate::Host::set_mtu`).
    ///
    /// # Errors
    ///
    /// Returns [`error::BadParameter`](`crate::error::BadParameter`) if `mtu` is greater than
    /// [`consts::PROTOCOL_MAXIMUM_MTU`](`crate::consts::PROTOCOL_MAXIMUM_MTU`) or less than
    /// [`consts::PROTOCOL_MINIMUM_MTU`](`crate::consts::PROTOCOL_MINIMUM_MTU`).
    pub fn set_mtu(&mut self, mtu: u16) -> Result<(), crate::error::BadParameter> {
        self.host.set_mtu(mtu)
    }

    /// See [`Host::now`](`crate::Host::now`).
    #[must_use]
    pub fn now(&self) -> Duration {
        self.host.now()
    }
}

#[cfg(feature = "std")]
impl Connection for TcpStream {
    type Address = SocketAddr;
    type Error = io::Error;

    fn init(&mut self) -> Result<Self::Address, Self::Error> {
        self.set_nonblocking(true)?;
        self.set_nodelay(true)?;
        self.peer_addr()
    }

    fn send(&mut self, buffer: &[u8]) -> Result<usize, Self::Error> {
        use std::io::{ErrorKind, Write};
        match self.write(buffer) {
            Ok(sent_length) => Ok(sent_length),
            Err(err) if err.kind() == ErrorKind::WouldBlock => Ok(0),
            Err(err) => Err(err),
        }
    }

    fn receive(
        &mut self,
        buffer: &mut [u8; crate::MTU_MAX],
    ) -> Result<Option<crate::PacketReceived>, Self::Error> {
        use std::io::{ErrorKind, Read};
        match self.read(buffer) {
            Ok(recv_length) => Ok(Some(crate::PacketReceived::Complete(recv_length))),
            Err(err) if err.kind() == ErrorKind::WouldBlock => Ok(None),
            Err(err) => Err(err),
        }
    }
}

#[cfg(all(test, feature = "std"))]
mod receive_policy_tests {
    use super::*;
    use crate::{Box, HostSettings, Packet, ReadWrite, ReceiveLimitKind, ReceivePolicy};
    type Connection = ReadWrite<(), std::convert::Infallible>;

    fn transfer(a: &mut Host<Connection>, b: &mut Host<Connection>) {
        while let Some(((), bytes)) = a.peer_mut(ConnectionID(0)).connection_mut().unwrap().read() {
            b.peer_mut(ConnectionID(0))
                .connection_mut()
                .unwrap()
                .write((), bytes);
        }
    }

    #[test]
    fn native_receive_handshake_terminal_event_owns_peer_for_disconnect_and_reset_reuse() {
        for reset in [false, true] {
            let mut server = Host::new(HostSettings {
                peer_limit: 1,
                channel_limit: 3,
                receive_policy: Some(ReceivePolicy {
                    maximum_packet_size: 8192,
                    maximum_queued_bytes: 65536,
                    maximum_queued_commands: 256,
                }),
                ..Default::default()
            })
            .unwrap();
            for generation in 0..2 {
                server
                    .add_connection(ConnectionKind::Receiver {
                        connection: Connection::new(),
                        timeout: Duration::from_secs(3),
                    })
                    .unwrap();
                let mut client = Host::new(HostSettings {
                    peer_limit: 1,
                    channel_limit: 3,
                    ..Default::default()
                })
                .unwrap();
                client
                    .add_connection(ConnectionKind::Initiator {
                        connection: Connection::new(),
                        channel_count: 3,
                        data: 0,
                    })
                    .unwrap();
                let mut connected = false;
                for _ in 0..32 {
                    if matches!(client.service(), Some(Event::Connect { .. })) {
                        connected = true;
                        break;
                    }
                    transfer(&mut client, &mut server);
                    assert!(
                        server.service().is_none(),
                        "receiver must not publish Connect before final ACK"
                    );
                    transfer(&mut server, &mut client);
                }
                assert!(connected);
                assert!(matches!(
                    server.peer(ConnectionID(0)).state,
                    PeerState::AwaitingPeer { .. }
                ));
                client
                    .peer_mut(ConnectionID(0))
                    .send(0, &Packet::reliable(vec![7]))
                    .unwrap();
                // Service twice: native service flushes the final handshake ACK plus valid input,
                // then the connected driver copies that datagram into the connection's output.
                assert!(client.service().is_none());
                assert!(client.service().is_none());
                let (_, mut bytes) = client
                    .peer_mut(ConnectionID(0))
                    .connection_mut()
                    .unwrap()
                    .read()
                    .expect("final ACK plus reliable input datagram");
                assert_eq!(bytes[4] & 15, 1, "final VERIFY_CONNECT ACK first");
                assert_eq!(bytes[12] & 15, 6, "valid reliable input follows ACK");
                // A structurally valid first fragment declaration, before any receiver Connect.
                bytes.extend_from_slice(&[0x88, 1, 0, 1, 0, 1, 0, 1]);
                bytes.extend_from_slice(&1u32.to_be_bytes());
                bytes.extend_from_slice(&0u32.to_be_bytes());
                bytes.extend_from_slice(&8193u32.to_be_bytes());
                bytes.extend_from_slice(&0u32.to_be_bytes());
                bytes.push(9);
                // Following same-datagram input must not reach another channel's queue.
                bytes.extend_from_slice(&[0x86, 2, 0, 1, 0, 1, 11]);
                let later = bytes.clone();
                server
                    .peer_mut(ConnectionID(0))
                    .connection_mut()
                    .unwrap()
                    .write((), bytes);
                let event = server
                    .service()
                    .expect("terminal event replaces receiver Connect");
                match event {
                    Event::ReceiveLimitExceeded {
                        peer,
                        channel_id,
                        kind,
                    } => {
                        assert_eq!(channel_id, 1);
                        assert_eq!(kind, ReceiveLimitKind::PacketSize);
                        let native = peer.peer().expect("limit event must own the native peer");
                        assert_eq!(native.address().unwrap().generation, generation);
                        unsafe {
                            assert!((*native.0).receive_terminal);
                            assert_eq!((*native.0).queued_receive_commands, 1, "queued input precedes the terminal command, following input is suppressed");
                            assert!((*native.0).queued_receive_bytes > 1);
                        }
                        if reset {
                            peer.reset();
                        } else {
                            peer.disconnect_now(16);
                        }
                    }
                    _ => panic!("terminal declaration must suppress Connect and queued Receive"),
                }
                let native = server.host.peer(crate::PeerID(0)).0;
                unsafe {
                    assert_eq!((*native).state,crate::ENET_PEER_STATE_DISCONNECTED,"cleanup must reach exact native session, reset={reset}, generation={generation}");
                    assert_eq!((*native).queued_receive_bytes, 0);
                    assert_eq!((*native).queued_receive_commands, 0);
                    assert!(!(*native).receive_terminal);
                }
                // The native slot is already released above. Explicit reset also releases the
                // transport wrapper after disconnect_now, which does not close its connection.
                server.peer_mut(ConnectionID(0)).reset();
                // A late old-generation datagram must not resurrect a terminal native session.
                let old_address = Address::new((), ConnectionID(0), generation);
                server.host.socket_mut().write(old_address, later);
                assert!(server.service().is_none());
            }
            // Prove a normal replacement can complete on the only native slot after both attacks.
            server
                .add_connection(ConnectionKind::Receiver {
                    connection: Connection::new(),
                    timeout: Duration::from_secs(3),
                })
                .unwrap();
            let mut replacement = Host::new(HostSettings {
                peer_limit: 1,
                channel_limit: 3,
                ..Default::default()
            })
            .unwrap();
            replacement
                .add_connection(ConnectionKind::Initiator {
                    connection: Connection::new(),
                    channel_count: 3,
                    data: 0,
                })
                .unwrap();
            let (mut a, mut b) = (false, false);
            for _ in 0..32 {
                a |= matches!(replacement.service(), Some(Event::Connect { .. }));
                transfer(&mut replacement, &mut server);
                b |= matches!(server.service(), Some(Event::Connect { .. }));
                transfer(&mut server, &mut replacement);
                if a && b {
                    break;
                }
            }
            assert!(
                a && b,
                "replacement handshake needs the released only native slot"
            );
        }
    }
    type StagedConnection = ReadWrite<(), std::io::Error>;
    fn staged_transfer(a: &mut Host<StagedConnection>, b: &mut Host<StagedConnection>) {
        while let Some(((), bytes)) = a.peer_mut(ConnectionID(0)).connection_mut().unwrap().read() {
            b.peer_mut(ConnectionID(0))
                .connection_mut()
                .unwrap()
                .write((), bytes);
        }
    }

    // Reproduce public service staging: receiver0's old final ACK/data enters the native
    // inbound queue, but receiver1's receive error returns before that queue is parsed.
    fn staged_old_limit(reset: bool) -> Host<StagedConnection> {
        use std::sync::{
            atomic::{AtomicU64, Ordering},
            Arc,
        };
        let clock = Arc::new(AtomicU64::new(1000));
        let timer = clock.clone();
        let mut server = Host::new(HostSettings {
            peer_limit: 3,
            channel_limit: 3,
            time: Box::new(move || Duration::from_millis(timer.load(Ordering::SeqCst))),
            receive_policy: Some(ReceivePolicy {
                maximum_packet_size: 8192,
                maximum_queued_bytes: 65536,
                maximum_queued_commands: 256,
            }),
            ..Default::default()
        })
        .unwrap();
        server
            .add_connection(ConnectionKind::Receiver {
                connection: StagedConnection::new(),
                timeout: Duration::from_secs(1),
            })
            .unwrap();
        let mut old = Host::new(HostSettings {
            peer_limit: 1,
            channel_limit: 3,
            ..Default::default()
        })
        .unwrap();
        old.add_connection(ConnectionKind::Initiator {
            connection: StagedConnection::new(),
            channel_count: 3,
            data: 0,
        })
        .unwrap();
        let mut connected = false;
        for _ in 0..32 {
            if matches!(old.service(), Some(Event::Connect { .. })) {
                connected = true;
                break;
            }
            staged_transfer(&mut old, &mut server);
            assert!(server.service().is_none());
            staged_transfer(&mut server, &mut old);
        }
        assert!(connected);
        old.peer_mut(ConnectionID(0))
            .send(0, &Packet::reliable(vec![7]))
            .unwrap();
        assert!(old.service().is_none());
        assert!(old.service().is_none());
        let (_, mut bytes) = old
            .peer_mut(ConnectionID(0))
            .connection_mut()
            .unwrap()
            .read()
            .unwrap();
        assert_eq!(bytes[4] & 15, 1);
        assert_eq!(bytes[12] & 15, 6);
        bytes.extend_from_slice(&[0x88, 1, 0, 1, 0, 1, 0, 1]);
        bytes.extend_from_slice(&1u32.to_be_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes.extend_from_slice(&8193u32.to_be_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes.push(9);
        server
            .add_connection(ConnectionKind::Receiver {
                connection: StagedConnection::new(),
                timeout: Duration::from_secs(1),
            })
            .unwrap();
        server
            .peer_mut(ConnectionID(0))
            .connection_mut()
            .unwrap()
            .write((), bytes);
        server
            .peer_mut(ConnectionID(1))
            .connection_mut()
            .unwrap()
            .error(std::io::Error::other("later connection receive failure"));
        assert!(
            matches!(server.service(),Some(Event::Disconnect { peer,.. }) if peer.id()==ConnectionID(1))
        );
        if reset {
            server.peer_mut(ConnectionID(0)).reset();
        } else {
            clock.store(2001, Ordering::SeqCst);
            assert!(
                matches!(server.service(),Some(Event::Disconnect { peer,.. }) if peer.id()==ConnectionID(0))
            );
        }
        // AwaitingPeer never owned this native handshake, so the staged datagram still
        // addresses a live old native session after wrapper reset/timeout.
        unsafe {
            assert_eq!(
                (*server.host.peer(crate::PeerID(0)).0).state,
                crate::ENET_PEER_STATE_ACKNOWLEDGING_CONNECT
            );
        }
        server
    }

    #[test]
    fn native_receive_staged_old_limit_cannot_attach_replacement_awaiting_or_has_peer() {
        for reset in [false, true] {
            for initiator in [false, true] {
                let mut server = staged_old_limit(reset);
                let mut replacement = Host::new(HostSettings {
                    peer_limit: 1,
                    channel_limit: 3,
                    ..Default::default()
                })
                .unwrap();
                let expected_ptr = if initiator {
                    server
                        .add_connection(ConnectionKind::Initiator {
                            connection: StagedConnection::new(),
                            channel_count: 3,
                            data: 0,
                        })
                        .unwrap();
                    replacement
                        .add_connection(ConnectionKind::Receiver {
                            connection: StagedConnection::new(),
                            timeout: Duration::from_secs(3),
                        })
                        .unwrap();
                    Some(server.peer(ConnectionID(0)).peer().unwrap().0)
                } else {
                    server
                        .add_connection(ConnectionKind::Receiver {
                            connection: StagedConnection::new(),
                            timeout: Duration::from_secs(3),
                        })
                        .unwrap();
                    replacement
                        .add_connection(ConnectionKind::Initiator {
                            connection: StagedConnection::new(),
                            channel_count: 3,
                            data: 0,
                        })
                        .unwrap();
                    None
                };
                assert!(server.service().is_none(),"old terminal event must not be attributed to replacement, reset={reset}, initiator={initiator}");
                let old = server.host.peer(crate::PeerID(0)).0;
                unsafe {
                    assert_eq!((*old).state, crate::ENET_PEER_STATE_DISCONNECTED);
                    assert_eq!((*old).queued_receive_bytes, 0);
                    assert_eq!((*old).queued_receive_commands, 0);
                    assert!(!(*old).receive_terminal);
                }
                match &server.peer(ConnectionID(0)).state {
                    PeerState::AwaitingPeer { address, .. } => {
                        assert!(!initiator);
                        assert_eq!(address.generation, 2);
                    }
                    PeerState::HasPeer {
                        address, peer_ptr, ..
                    } => {
                        assert!(initiator);
                        assert_eq!(address.generation, 2);
                        assert_eq!(unsafe { (**peer_ptr).0 }, expected_ptr.unwrap());
                    }
                    _ => panic!("replacement wrapper was mutated by old limit"),
                }
                let (mut a, mut b) = (false, false);
                for _ in 0..64 {
                    a |= matches!(server.service(), Some(Event::Connect { .. }));
                    staged_transfer(&mut server, &mut replacement);
                    b |= matches!(replacement.service(), Some(Event::Connect { .. }));
                    staged_transfer(&mut replacement, &mut server);
                    if a && b {
                        break;
                    }
                }
                assert!(
                    a && b,
                    "replacement must progress after stale session release"
                );
                replacement
                    .peer_mut(ConnectionID(0))
                    .send(0, &Packet::reliable(vec![8]))
                    .unwrap();
                let mut received = false;
                for _ in 0..32 {
                    let _ = replacement.service();
                    staged_transfer(&mut replacement, &mut server);
                    if let Some(Event::Receive { packet, .. }) = server.service() {
                        assert_eq!(packet.data(), [8]);
                        received = true;
                        break;
                    }
                    staged_transfer(&mut server, &mut replacement);
                }
                assert!(received);
            }
        }
    }

    #[test]
    fn native_receive_staged_old_limit_drops_disconnected_or_closing_wrapper() {
        for closing in [false, true] {
            let mut server = staged_old_limit(true);
            if closing {
                server
                    .add_connection(ConnectionKind::Receiver {
                        connection: StagedConnection::new(),
                        timeout: Duration::from_secs(3),
                    })
                    .unwrap();
                let now = server.now();
                let peer = server.peer_mut(ConnectionID(0));
                let state = replace(
                    &mut peer.state,
                    PeerState::Disconnected {
                        last_peer_ptr: None,
                    },
                );
                if let PeerState::AwaitingPeer {
                    connection,
                    address,
                    ..
                } = state
                {
                    peer.state = PeerState::Disconnecting {
                        connection,
                        address,
                        last_peer_ptr: None,
                        last_send: now,
                    };
                } else {
                    unreachable!();
                }
            }
            assert!(server.service().is_none(),"stale terminal event must be discarded without mutating closing/disconnected wrapper");
            assert!(if closing {
                matches!(
                    server.peer(ConnectionID(0)).state,
                    PeerState::Disconnecting { .. }
                )
            } else {
                matches!(
                    server.peer(ConnectionID(0)).state,
                    PeerState::Disconnected { .. }
                )
            });
            unsafe {
                let old = server.host.peer(crate::PeerID(0)).0;
                assert_eq!((*old).state, crate::ENET_PEER_STATE_DISCONNECTED);
                assert_eq!((*old).queued_receive_bytes, 0);
                assert_eq!((*old).queued_receive_commands, 0);
            }
        }
    }
    struct IdentityConnection {
        address: usize,
        data: ReadWrite<(), std::io::Error>,
    }
    impl super::Connection for IdentityConnection {
        type Address = usize;
        type Error = std::io::Error;
        fn init(&mut self) -> Result<usize, Self::Error> {
            Ok(self.address)
        }
        fn send(&mut self, buffer: &[u8]) -> Result<usize, Self::Error> {
            crate::Socket::send(&mut self.data, (), buffer)
        }
        fn receive(
            &mut self,
            buffer: &mut [u8; crate::MTU_MAX],
        ) -> Result<Option<crate::PacketReceived>, Self::Error> {
            Ok(crate::Socket::receive(&mut self.data, buffer)?.map(|(_, received)| received))
        }
    }

    #[test]
    fn native_receive_full_address_and_has_peer_pointer_are_checked_independently() {
        for pointer_only in [false, true] {
            let mut server: Host<IdentityConnection> = Host::new(HostSettings {
                peer_limit: 2,
                channel_limit: 3,
                receive_policy: Some(ReceivePolicy {
                    maximum_packet_size: 8192,
                    maximum_queued_bytes: 65536,
                    maximum_queued_commands: 256,
                }),
                ..Default::default()
            })
            .unwrap();
            // Old native identity and new wrapper deliberately share index/generation.
            // AwaitingPeer must still compare underlying address; HasPeer with the same
            // full address must still compare its exact native peer pointer.
            let old = server
                .host
                .connect(Address::new(1, ConnectionID(0), 0), 3, 0)
                .unwrap();
            let old_id = old.id();
            let old_ptr = old.0;
            unsafe {
                crate::enet_peer_on_connect(old_ptr);
                (*old_ptr).state = crate::ENET_PEER_STATE_CONNECTED;
                let mut command: crate::ENetProtocol = core::mem::zeroed();
                command.header.command = 0x86;
                command.header.channel_id = 0;
                command.header.reliable_sequence_number = 1;
                assert!(!crate::enet_peer_queue_incoming_command(
                    old_ptr,
                    &command,
                    [7u8].as_ptr(),
                    1,
                    crate::ENET_PACKET_FLAG_RELIABLE,
                    0
                )
                .is_null());
                crate::enet_peer_receive_limit(old_ptr, 0, 1);
                assert_eq!((*old_ptr).queued_receive_commands, 1);
            }
            let connection = IdentityConnection {
                address: if pointer_only { 1 } else { 2 },
                data: ReadWrite::new(),
            };
            let replacement_ptr = if pointer_only {
                server
                    .add_connection(ConnectionKind::Initiator {
                        connection,
                        channel_count: 3,
                        data: 0,
                    })
                    .unwrap();
                Some(server.peer(ConnectionID(0)).peer().unwrap().0)
            } else {
                server
                    .add_connection(ConnectionKind::Receiver {
                        connection,
                        timeout: Duration::from_secs(3),
                    })
                    .unwrap();
                None
            };
            assert!(server
                .handle_event(crate::EventNoRef::ReceiveLimitExceeded {
                    peer: old_id,
                    channel_id: 0,
                    kind: ReceiveLimitKind::PacketSize
                })
                .is_none());
            unsafe {
                assert_eq!((*old_ptr).state, crate::ENET_PEER_STATE_DISCONNECTED);
                assert_eq!((*old_ptr).queued_receive_bytes, 0);
                assert_eq!((*old_ptr).queued_receive_commands, 0);
            }
            if let Some(expected) = replacement_ptr {
                assert_eq!(server.peer(ConnectionID(0)).peer().unwrap().0, expected);
                unsafe {
                    assert_eq!((*expected).state, crate::ENET_PEER_STATE_CONNECTING);
                }
            } else {
                assert!(matches!(
                    server.peer(ConnectionID(0)).state,
                    PeerState::AwaitingPeer { .. }
                ));
            }
        }
    }
}
