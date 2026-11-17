//! Archipelago and Pulse socket tests, one binary for several files so a build links few
//! test executables. tooling/catalyst-test.sh runs `sockets_*` binaries within the core
//! budget their wall-clock limits need. Each file stays a module named after it.

#[path = "archipelago_ban_check.rs"]
mod archipelago_ban_check;
#[path = "archipelago_ban_eviction.rs"]
mod archipelago_ban_eviction;
#[path = "archipelago_hot_scenes.rs"]
mod archipelago_hot_scenes;
#[path = "archipelago_proto_wire.rs"]
mod archipelago_proto_wire;
#[path = "archipelago_upstream_parity.rs"]
mod archipelago_upstream_parity;
#[path = "archipelago_ws_island_dedup.rs"]
mod archipelago_ws_island_dedup;
// Builds only with test-support, as its own target did.
#[cfg(feature = "test-support")]
#[path = "pulse_generation.rs"]
mod pulse_generation;
#[path = "pulse_application_transport.rs"]
mod pulse_application_transport;
#[path = "pulse_loopback.rs"]
mod pulse_loopback;
#[path = "pulse_quantize_differential.rs"]
mod pulse_quantize_differential;
