//! The Pulse and archipelago wire messages: `decentraland.pulse` (the ENet /
//! WebTransport client and server messages, the cluster feed's
//! `PeerClusterChange`), `decentraland.kernel.comms.v3` (archipelago's
//! WebSocket protocol and the `engine.*` NATS shapes),
//! `decentraland.kernel.comms.rfc4` (the peer `Packet`) and
//! `decentraland.common` (vectors and the auth chain). They are compiled by
//! build.rs from the one proto source, `library/protocol/proto`; the module
//! tree below keeps the package nesting (`decentraland::pulse` reaches
//! `AuthChain` as `super::common::AuthChain`).

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
            /// The peer packets of RFC-4; the Cast presenter wraps its data
            /// channel messages in `Packet.scene`.
            pub mod rfc4 {
                include!(concat!(
                    env!("OUT_DIR"),
                    "/decentraland.kernel.comms.rfc4.rs"
                ));
            }
        }
    }
}
