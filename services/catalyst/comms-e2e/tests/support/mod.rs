//! A Pulse server inside the test process, on a loopback port of its own: the tests of this crate
//! need no stack.
#![allow(dead_code)]

use std::net::SocketAddr;
use std::sync::Arc;

use catalyst_comms::pulse::PulseServer;
use catalyst_comms::pulse::server::ENET_CAPACITY;
use catalyst_comms::pulse::transport::webtransport::{WtConfig, WtHost};
use catalyst_comms::pulse::transport::{Host, HostConfig, Transports};
use catalyst_comms_e2e::engine::{Ctx, Transport};
pub use catalyst_comms_e2e::stack::{Stack, WebTransportTarget};
use sha2::{Digest, Sha256};

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

/// An in-process Pulse that listens on both transports.
#[derive(Clone)]
pub struct Pulse {
    pub enet: SocketAddr,
    pub webtransport: WebTransportTarget,
}

impl Pulse {
    pub fn stack(&self) -> Arc<Stack> {
        let mut stack = Stack::pulse_only(self.enet);
        stack.pulse_webtransport = Some(self.webtransport.clone());
        Arc::new(stack)
    }

    /// A context whose bots use `transport`.
    pub fn ctx_on(&self, transport: Transport, name: &str) -> Ctx {
        ctx_with(self.stack(), transport, name)
    }
}

/// Starts `server` on ENet and WebTransport, both on `127.0.0.1`, the WebTransport side with a
/// freshly generated certificate. It lives as long as the runtime.
pub async fn serve_both(server: PulseServer) -> Pulse {
    let certified = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let hash = hex::encode(Sha256::digest(certified.cert.der().as_ref()));
    let (cert_pem, key_pem) = (certified.cert.pem(), certified.signing_key.serialize_pem());
    // the QUIC host runs a runtime of its own, which cannot start inside this one
    let (webtransport, events) = tokio::task::spawn_blocking(move || {
        WtHost::start(WtConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            cert_pem,
            key_pem,
            slot_base: ENET_CAPACITY as u32,
            slot_capacity: 256,
            max_datagram_bytes: 1200,
            max_message_bytes: 4096,
            service_timeout_ms: 1,
            server_full_reason: 6,
        })
    })
    .await
    .unwrap()
    .expect("a WebTransport port");
    let port = webtransport.local_addr().port();
    let enet = Host::bind(HostConfig {
        bind: "127.0.0.1:0".parse().unwrap(),
        max_peers: ENET_CAPACITY,
        channel_limit: 8,
    })
    .await
    .expect("a loopback port");
    let address = enet.local_addr().unwrap();
    let transports =
        Transports::with_webtransport(enet, ENET_CAPACITY as u32, webtransport, events);
    tokio::spawn(server.serve(transports, 50));
    Pulse {
        enet: address,
        webtransport: WebTransportTarget {
            url: format!("https://127.0.0.1:{port}/").parse().unwrap(),
            certificate_hash: Some(hash),
        },
    }
}

pub async fn in_process_pulse_both() -> Pulse {
    serve_both(PulseServer::new()).await
}

pub fn ctx_on_stack(stack: Arc<Stack>, transport: Transport, name: &str) -> Ctx {
    ctx_with(stack, transport, name)
}

fn ctx_with(stack: Arc<Stack>, transport: Transport, name: &str) -> Ctx {
    static RUN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let run = RUN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    Ctx::new_on(
        stack,
        &format!("t{}-{run}", std::process::id()),
        name,
        0,
        1,
        std::env::temp_dir().join("comms-e2e-tests"),
        transport,
    )
}
