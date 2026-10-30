use crate::{Packet, Peer, PeerID, Socket};

/// The incoming resource limit that refused a command. This is a local event, not wire data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiveLimitKind {
    /// A structurally valid packet declaration exceeded the incoming payload bound.
    PacketSize,
    /// Queued payload, bitmap and descriptors would exceed the byte bound.
    QueuedBytes,
    /// Another queued command would exceed the command bound.
    QueuedCommands,
}

/// An ENet event returned by [`Host::service`](`crate::Host::service`).
#[derive(Debug)]
pub enum Event<'a, S: Socket> {
    /// A new peer has connected.
    Connect {
        /// Peer that generated the event.
        peer: &'a mut Peer<S>,
        /// Data associated with the event, sent by the peer on connect.
        data: u32,
    },
    /// A peer has disconnected.
    Disconnect {
        /// Peer that generated the event.
        peer: &'a mut Peer<S>,
        /// Data associated with the event, sent by the peer on disconnect.
        data: u32,
    },
    /// A peer sent a packet to us.
    Receive {
        /// Peer that generated the event.
        peer: &'a mut Peer<S>,
        /// Channel the peer sent the packet on.
        channel_id: u8,
        /// The actual packet data.
        packet: Packet,
    },
    /// An incoming command exceeded the optional receive policy.
    ReceiveLimitExceeded {
        /// Peer that generated the event.
        peer: &'a mut Peer<S>,
        /// Channel of the refused command.
        channel_id: u8,
        /// The exceeded limit; only packet size is terminal.
        kind: crate::ReceiveLimitKind,
    },
}

impl<S: Socket> Event<'_, S> {
    /// Remove the peer reference from this event, converting into an [`EventNoRef`].
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

/// An ENet event, like [`Event`], but without peer references.
///
/// Acquired with [`Event::no_ref`].
#[derive(Debug, Clone)]
pub enum EventNoRef {
    /// A new peer has connected.
    Connect {
        /// Peer that generated the event.
        peer: PeerID,
        /// Data associated with the event, sent by the peer on connect.
        data: u32,
    },
    /// A peer has disconnected.
    Disconnect {
        /// Peer that generated the event.
        peer: PeerID,
        /// Data associated with the event, sent by the peer on disconnect.
        data: u32,
    },
    /// A peer sent a packet to us.
    Receive {
        /// Peer that generated the event.
        peer: PeerID,
        /// Channel the peer sent the packet on.
        channel_id: u8,
        /// The actual packet data.
        packet: Packet,
    },
    /// An incoming command exceeded the optional receive policy.
    ReceiveLimitExceeded {
        /// Peer that generated the event.
        peer: PeerID,
        /// Channel of the refused command.
        channel_id: u8,
        /// The exceeded limit; only packet size is terminal.
        kind: crate::ReceiveLimitKind,
    },
}
