//! The Pulse and archipelago wire messages: `decentraland.pulse` (the ENet /
//! WebTransport client and server messages, the cluster feed's
//! `PeerClusterChange`), `decentraland.kernel.comms.v3` (archipelago's
//! WebSocket protocol and the `engine.*` NATS shapes) and `decentraland.common` (vectors and
//! the auth chain). They are compiled by build.rs from the one proto source,
//! `library/protocol/proto`; the module tree below keeps the package nesting
//! (`decentraland::pulse` reaches `AuthChain` as `super::common::AuthChain`).

#[allow(clippy::all)]
pub mod decentraland {
    pub mod common {
        include!(concat!(env!("OUT_DIR"), "/decentraland.common.rs"));
    }
    pub mod pulse {
        include!(concat!(env!("OUT_DIR"), "/decentraland.pulse.rs"));
    }
    pub mod kernel {
        pub mod comms {
            pub mod v3 {
                include!(concat!(env!("OUT_DIR"), "/decentraland.kernel.comms.v3.rs"));
            }
        }
    }
}
