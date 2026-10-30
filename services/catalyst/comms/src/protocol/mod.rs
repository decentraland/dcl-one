//! What the services share, and the only module through which they reach one
//! another: the Pulse and archipelago protos, the NATS message set between the
//! Pulse process and `catalyst serve` (`bus`), and the room authority between
//! Pulse and the gatekeeper (its frames, limits, claim types and proof
//! verifier). It imports nothing from the services.

pub mod archipelago;
pub mod bus;
pub mod pulse;
pub mod room_authority;
