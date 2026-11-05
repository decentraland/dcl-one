use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use prost::Message;
use tokio::sync::Mutex;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use super::RpcServer;
use crate::rpc::drpc::client::{RpcClient, RpcClientModule, ServiceClient};
use crate::rpc::drpc::rpc_protocol::{
    CreatePort, RequestModule, RpcMessageTypes, parse::build_message_identifier,
};
use crate::rpc::drpc::service_module_definition::ServiceModuleDefinition;
use crate::rpc::drpc::transport::memory::MemoryTransport;
use crate::rpc::drpc::transport::{Transport, TransportError, TransportMessage};

/// A socket that admits its first `admitted` sends and then never admits another byte, like
/// a client that stopped reading once its byte budget filled.
struct StalledTransport {
    inbound: Mutex<UnboundedReceiver<TransportMessage>>,
    admitted: AtomicUsize,
    stalled_sends: Arc<AtomicUsize>,
}

#[async_trait]
impl Transport for StalledTransport {
    async fn receive(&self) -> Result<TransportMessage, TransportError> {
        self.inbound
            .lock()
            .await
            .recv()
            .await
            .ok_or(TransportError::Closed)
    }

    async fn send(&self, _message: TransportMessage) -> Result<(), TransportError> {
        let admitted = self
            .admitted
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1));
        if admitted.is_ok() {
            return Ok(());
        }
        self.stalled_sends.fetch_add(1, Ordering::SeqCst);
        std::future::pending().await
    }

    async fn close(&self) {}
}

fn stalled(
    admitted: usize,
) -> (
    Arc<StalledTransport>,
    UnboundedSender<Vec<u8>>,
    Arc<AtomicUsize>,
) {
    let (client, inbound) = unbounded_channel();
    let stalled_sends = Arc::new(AtomicUsize::new(0));
    let transport = Arc::new(StalledTransport {
        inbound: Mutex::new(inbound),
        admitted: AtomicUsize::new(admitted),
        stalled_sends: stalled_sends.clone(),
    });
    (transport, client, stalled_sends)
}

struct Module;

impl<T: Transport> ServiceClient<T> for Module {
    fn set_client_module(_client_module: RpcClientModule<T>) -> Self {
        Module
    }
}

fn server() -> RpcServer<(), dyn Transport> {
    let mut server = RpcServer::<(), dyn Transport>::create(());
    server.set_module_registrator_handler(|port| {
        port.register_module("Module".to_string(), ServiceModuleDefinition::new());
    });
    server
}

async fn wait_for(counter: &AtomicUsize, expected: usize) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while counter.load(Ordering::SeqCst) < expected {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("the stalled client reached its stalled send");
}

/// The healthy client attaches, creates a port and loads a module while `stalled` is stuck.
async fn healthy_client_is_served(events: &super::ServerEventsSender<dyn Transport>) {
    let (client_transport, server_transport) = MemoryTransport::create();
    events
        .send_attach_transport(Arc::new(server_transport))
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut client = RpcClient::new(client_transport).await.unwrap();
        let port = client.create_port("healthy").await.unwrap();
        port.load_module::<Module>("Module").await.unwrap();
    })
    .await
    .expect(
        "a stalled client must not hold up another client's attach, CreatePort or RequestModule",
    );
}

#[tokio::test]
async fn stalled_server_ready_does_not_block_other_clients() {
    let mut server = server();
    let events = server.get_server_events_sender();
    tokio::spawn(async move { server.run().await });

    let (transport, _client, stalled_sends) = stalled(0);
    events.send_attach_transport(transport).unwrap();
    wait_for(&stalled_sends, 1).await;

    healthy_client_is_served(&events).await;
}

#[tokio::test]
async fn stalled_port_and_module_replies_do_not_block_other_clients() {
    let mut server = server();
    let events = server.get_server_events_sender();
    tokio::spawn(async move { server.run().await });

    // ServerReady is admitted; every reply after it waits forever.
    let (transport, client, stalled_sends) = stalled(1);
    events.send_attach_transport(transport).unwrap();
    client
        .send(
            CreatePort {
                message_identifier: build_message_identifier(RpcMessageTypes::CreatePort as u32, 1),
                port_name: "stalled".to_string(),
            }
            .encode_to_vec(),
        )
        .unwrap();
    wait_for(&stalled_sends, 1).await;
    client
        .send(
            RequestModule {
                message_identifier: build_message_identifier(
                    RpcMessageTypes::RequestModule as u32,
                    2,
                ),
                port_id: 1,
                module_name: "Module".to_string(),
            }
            .encode_to_vec(),
        )
        .unwrap();
    wait_for(&stalled_sends, 2).await;

    healthy_client_is_served(&events).await;
}

/// Connected callbacks follow attach order even when an older ServerReady
/// stalls. Social's per-wallet admission is handled before transport attach.
#[tokio::test]
async fn connected_callbacks_follow_arrival_before_server_ready() {
    let mut server = server();
    let (connected, mut connections) = unbounded_channel();
    server.set_on_transport_connected_handler(move |_, id| {
        connected.send(id).unwrap();
    });
    let events = server.get_server_events_sender();
    let task = tokio::spawn(async move { server.run().await });
    let (slow, _client, stalled_sends) = stalled(0);
    events.send_attach_transport(slow).unwrap();
    wait_for(&stalled_sends, 1).await;
    healthy_client_is_served(&events).await;
    assert_eq!(connections.try_recv().unwrap(), 1);
    assert_eq!(connections.try_recv().unwrap(), 2);
    task.abort();
}

struct FailedReadyTransport;

#[async_trait]
impl Transport for FailedReadyTransport {
    async fn receive(&self) -> Result<TransportMessage, TransportError> {
        panic!("a failed handshake must never start reading");
    }

    async fn send(&self, _: TransportMessage) -> Result<(), TransportError> {
        Err(TransportError::Closed)
    }

    async fn close(&self) {}
}

#[tokio::test]
async fn a_failed_server_ready_releases_its_registered_transport() {
    let mut server = server();
    let (events, mut observed) = unbounded_channel();
    let connected = events.clone();
    server.set_on_transport_connected_handler(move |_, id| {
        connected.send(("connected", id)).unwrap();
    });
    server.set_on_transport_closes_handler(move |_, id| {
        events.send(("closed", id)).unwrap();
    });
    let sender = server.get_server_events_sender();
    let task = tokio::spawn(async move { server.run().await });
    sender
        .send_attach_transport(Arc::new(FailedReadyTransport))
        .unwrap();
    for expected in [("connected", 1), ("closed", 1)] {
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), observed.recv())
                .await
                .unwrap(),
            Some(expected)
        );
    }
    task.abort();
}
