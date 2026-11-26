//! Archipelago and Pulse socket tests, one binary for several files so a build links few
//! test executables. tooling/catalyst-test.sh runs `sockets_*` binaries within the core
//! budget their wall-clock limits need. Each file stays a module named after it.
#![allow(
    clippy::unwrap_used,
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "test support"
)]

// Support shared by several modules, loaded once; the modules `use crate::<name>`.
#[path = "support/archipelago.rs"]
mod archipelago;
#[path = "support/db.rs"]
mod db;
#[path = "support/fence.rs"]
mod fence;
#[path = "support/gate.rs"]
mod gate;
#[path = "support/nats_broker.rs"]
mod nats_broker;

#[path = "archipelago_ban_reaches_v4_sockets.rs"]
mod archipelago_ban_reaches_v4_sockets;
#[path = "archipelago_control_extensions_websocket.rs"]
mod archipelago_control_extensions_websocket;
#[path = "archipelago_control_v4_authority.rs"]
mod archipelago_control_v4_authority;
#[path = "archipelago_feed_stats.rs"]
mod archipelago_feed_stats;
#[path = "archipelago_http_shapes.rs"]
mod archipelago_http_shapes;
#[path = "pulse_baseline_contract.rs"]
mod pulse_baseline_contract;
#[path = "pulse_batch_bandwidth.rs"]
mod pulse_batch_bandwidth;
#[path = "pulse_cluster_lookup.rs"]
mod pulse_cluster_lookup;
#[path = "pulse_nats_live.rs"]
mod pulse_nats_live;
#[path = "pulse_webtransport_loopback.rs"]
mod pulse_webtransport_loopback;
#[path = "pulse_webtransport_wire.rs"]
mod pulse_webtransport_wire;
