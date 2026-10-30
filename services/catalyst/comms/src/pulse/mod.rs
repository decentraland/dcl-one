//! Pulse, the realm-wide real-time layer (port of decentraland/Pulse): ENet/UDP
//! and WebTransport sessions, the handshake and hardening policies, the
//! per-peer snapshot boards, area-of-interest simulation, peer clustering and
//! the cluster feed archipelago's ws-connector consumes.
//! It lives in `catalyst-comms`; module placement is not
//! process placement (AGENTS.md, "Ownership"): the loop is latency-sensitive
//! and owns a UDP socket, so it runs as `catalyst pulse` in its own process
//! ([`env::run_from_env`]), with the `PULSE_*` environment contract the old
//! executable documented ([`env::ENV_DOCS`]). Pulse persists nothing and
//! imports nothing from the gatekeeper: the admission proof of the
//! application relay is minted and verified by comms from the claim/proof
//! types in [`crate::protocol::room_authority`].
//!
//! External runtimes stay separate processes: the NATS broker carrying the
//! cluster feed (`PULSE_NATS_URL`; blank leaves the tracker in stats-only
//! mode). The application relay implementation and room-authority boundary
//! remain available internally; production has no relay activation or admission capability.

pub mod application_relay;
pub mod bans;
pub mod batch;
pub mod cluster;
pub mod env;
pub mod flags;
#[cfg(test)]
pub mod fuzz;
pub mod handshake;
pub mod hardening;
pub mod interest;
pub mod ip_limiter;
pub mod messages;
pub mod metrics;
pub mod quantize;
pub mod realm_grids;
pub mod server;
pub mod simulation;
pub mod snapshot;
pub mod transport;

pub use crate::protocol::pulse::decentraland;
pub use crate::protocol::pulse::decentraland::pulse::PeerClusterChange;
pub use cluster::{ClusterOptions, ClusterPass, ClusterSession, ClusterTracker};
pub use env::run_from_env;
pub use handshake::{HandshakeError, VerifiedHandshake, verify_handshake};
pub use realm_grids::RealmSpatialGrids;
pub use server::PulseServer;
