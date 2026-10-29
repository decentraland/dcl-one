//! Decentraland RPC, vendored from `dcl-rpc` 2.3.6 (decentraland/rpc-rust)
//! through the old workspace's `catalyrst-drpc` crate: protobuf messages over
//! any [`Transport`](transport::Transport), ports, modules and streams. The
//! explorer speaks this protocol to the social service, so the wire format is
//! the contract; nothing here may change a message shape.
//!
//! Copied as ordinary modules rather than a crate (README: a domain name alone
//! is not a reason for a crate). Changes against the vendored copy:
//!
//! - `async-channel` -> `tokio::sync::mpsc` (same unbounded semantics; one
//!   fewer dependency).
//! - The cargo features are gone: the server half and the client half are
//!   always compiled (the end-to-end tests and `tests/social_live.rs` drive
//!   the server as the explorer would), the in-process memory transport only
//!   under `cfg(test)`.
//! - The service generator (`codegen.rs`) is not carried: the one service is
//!   registered by hand in `social::rpc::service` and the messages are
//!   pre-generated (`rpc_protocol/generated.rs`, `proto/generated/*`).
//! - The warp/tungstenite websocket transports are not carried; the axum
//!   websocket transport lives in `social::rpc::ws`.
//!
//! Protocol shape: a client connects over a transport, receives `SERVER_READY`,
//! creates ports, each mirrored by a [`server::RpcServerPort`]; a port owns
//! the modules (a `service` in the `.proto`) registered for it, and every
//! procedure of a server shares one `Arc<Context>`. Closing a port leaves both
//! the client and the server alive.

pub mod client;
pub mod messages_handlers;
pub mod rpc_protocol;
pub mod server;
pub mod service_module_definition;
pub mod stream_protocol;
pub mod transport;

#[derive(Debug)]
pub enum CommonError {
    ProtocolError,
    TransportError,
    TransportNotAttached,
    UnexpectedError(String),
    TransportWasClosed,
}
