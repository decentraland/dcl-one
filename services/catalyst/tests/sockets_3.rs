//! Archipelago and Pulse socket tests, one binary for several files so a build links few
//! test executables. tooling/catalyst-test.sh runs `sockets_*` binaries within the core
//! budget their wall-clock limits need. Each file stays a module named after it.

#[path = "archipelago_nats_recovery.rs"]
mod archipelago_nats_recovery;
#[path = "archipelago_socket_bounds.rs"]
mod archipelago_socket_bounds;
