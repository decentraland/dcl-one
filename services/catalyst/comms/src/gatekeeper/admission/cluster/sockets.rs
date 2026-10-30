//! Where the island subscriber hands the island assignments it mints: the `/ws` connector's
//! sockets, in the same process (`comms::archipelago` implements the trait). The gatekeeper
//! owns the seam so it imports nothing from the connector.

use crate::protocol::archipelago::archipelago::IslandChangedMessage;

/// What became of one assignment handed to [`IslandSockets::deliver_island`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IslandDelivery {
    /// In the addressed socket's mailbox: not a WebSocket write or a client receipt.
    Delivered,
    /// No socket here took it: this process holds no socket of that wallet (and session), or
    /// the socket refused it. Island assignments reach only sockets in the process that mints
    /// them, the primary `serve`.
    Undelivered,
}

/// The connector's live sockets, as the island subscriber delivers to them.
///
/// `session` names the socket registered under that validated session key.
/// Delivery is to its mailbox, not a client receipt; reconnect announcements
/// recover assignments for sessions this process does not currently hold.
pub trait IslandSockets: Send + Sync {
    fn deliver_island(
        &self,
        wallet: &str,
        session: &str,
        assignment: &IslandChangedMessage,

        _pulse_admission: Option<
            &crate::gatekeeper::admission::cluster::fence::PulseAdmissionStamp,
        >,
    ) -> IslandDelivery;
}
