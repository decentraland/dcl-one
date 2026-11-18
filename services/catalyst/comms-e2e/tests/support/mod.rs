//! A Pulse server inside the test process, on a loopback port of its own: the tests of this crate
//! need no stack.
#![allow(dead_code)]

use std::net::SocketAddr;
use std::sync::Arc;

use catalyst_comms::pulse::PulseServer;
use catalyst_comms::pulse::server::ENET_CAPACITY;
use catalyst_comms::pulse::transport::{Host, HostConfig, Transports};
use catalyst_comms_e2e::engine::Ctx;
use catalyst_comms_e2e::stack::Stack;

/// Starts `server` on `127.0.0.1` and returns its ENet address. It lives as long as the runtime.
pub async fn serve(server: PulseServer) -> SocketAddr {
    let enet = Host::bind(HostConfig {
        bind: "127.0.0.1:0".parse().unwrap(),
        max_peers: ENET_CAPACITY,
        channel_limit: 8,
    })
    .await
    .expect("a loopback port");
    let address = enet.local_addr().unwrap();
    let transports = Transports::enet_only(enet, ENET_CAPACITY as u32);
    tokio::spawn(server.serve(transports, 50));
    address
}

pub async fn in_process_pulse() -> SocketAddr {
    serve(PulseServer::new()).await
}

pub fn stack_at(pulse: SocketAddr) -> Arc<Stack> {
    Arc::new(Stack::pulse_only(pulse))
}

/// A context for a test that drives bots directly, without the runner.
pub fn ctx(pulse: SocketAddr, name: &str) -> Ctx {
    static RUN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let run = RUN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    Ctx::new(
        stack_at(pulse),
        &format!("t{}-{run}", std::process::id()),
        name,
        0,
        1,
        std::env::temp_dir().join("comms-e2e-tests"),
    )
}
