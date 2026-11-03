//! Initial subscription readiness gates Social only; later outages stay transient.
use super::*;
use crate::comms::gatekeeper::bus::{
    DisabledBus, InProcessBus, NatsBus, NatsHandler, NatsSubscription, PublishOutcome, RequestError,
};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

struct DelayedBus {
    inner: InProcessBus,
    ready: AtomicBool,
    connected: AtomicBool,
    subscribed: AtomicBool,
}
#[async_trait::async_trait]
impl NatsBus for DelayedBus {
    fn is_enabled(&self) -> bool {
        true
    }
    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }
    fn is_ready(&self) -> bool {
        assert!(
            self.subscribed.load(Ordering::Acquire),
            "readiness must be observed after Social registration"
        );
        self.ready.load(Ordering::Acquire)
    }
    fn connect(&self) {}
    fn subscribe(
        &self,
        subject: &str,
        queue: Option<&str>,
        handler: NatsHandler,
    ) -> NatsSubscription {
        assert_eq!(subject, crate::comms::protocol::bus::SOCIAL_EVENTS_SUBJECT);
        let sub = self.inner.subscribe(subject, queue, handler);
        self.subscribed.store(true, Ordering::Release);
        sub
    }
    async fn publish(&self, subject: &str, payload: Vec<u8>) -> PublishOutcome {
        if self.ready.load(Ordering::Acquire) {
            self.inner.publish(subject, payload).await
        } else {
            PublishOutcome::NoConnection
        }
    }
    async fn request(&self, subject: &str, payload: Vec<u8>) -> Result<Vec<u8>, RequestError> {
        self.inner.request(subject, payload).await
    }
}

async fn rpc_ready(server: &Server) -> (String, RpcClient<TungsteniteTransport>, SocialClient) {
    let mut socket = connect(server, "/social-rpc").await;
    let (me, auth) = frame("/", chrono::Utc::now().timestamp_millis()).await;
    socket.send(WsMessage::Text(auth.into())).await.unwrap();
    let mut client = RpcClient::new(TungsteniteTransport::new(socket))
        .await
        .unwrap();
    let social = client
        .create_port("startup")
        .await
        .unwrap()
        .load_module(MODULE_NAME)
        .await
        .unwrap();
    (me, client, social)
}
async fn json_ready(server: &Server) -> WsStream {
    let mut socket = connect(server, "/social-rpc/v1/stream").await;
    let (_, auth) = frame("/v1/stream", chrono::Utc::now().timestamp_millis()).await;
    socket.send(WsMessage::Text(auth.into())).await.unwrap();
    let WsMessage::Text(ready) = next_message(&mut socket).await else {
        panic!("JSON ready")
    };
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&ready).unwrap(),
        serde_json::json!({"type":"ready"})
    );
    socket
}

#[tokio::test]
async fn both_upgrades_wait_for_initial_social_subscription_without_slots_or_auth() {
    let server = spawn_server(None, 5).await;
    let bus = Arc::new(DelayedBus {
        inner: InProcessBus::default(),
        ready: AtomicBool::new(false),
        connected: AtomicBool::new(false),
        subscribed: AtomicBool::new(false),
    });
    server.state.start_fanout(&server.jobs, bus.clone());
    let source: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/conformance/social/startup-source-cases.json"
    )))
    .unwrap();
    assert_eq!(
        source["baseline"]["revision"],
        "b6b2cefefdf727b2ed47ddb887e77d226a41eea6"
    );
    let order = source["component_key_order"].as_array().unwrap();
    let position = |name: &str| order.iter().position(|value| value == name).unwrap();
    assert!(position("pubsub") < position("rpcServer"));
    let source_case = |id: &str| {
        source["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["id"] == id)
            .unwrap()
    };
    for stage in [
        "sub-connect-pending",
        "pub-connect-pending",
        "subscriptions-pending",
    ] {
        assert_eq!(source_case(stage)["state"]["listen_count"], 0);
        // A connected link alone does not install the subscription readiness fence.
        bus.connected
            .store(stage == "subscriptions-pending", Ordering::Release);
        for path in ["/social-rpc", "/social-rpc/v1/stream"] {
            let mut request = format!("ws://{}{path}", server.addr)
                .into_client_request()
                .unwrap();
            // This would be an auth refusal if the upgrade gate ran after auth.
            request
                .headers_mut()
                .insert("x-identity-timestamp", "invalid".parse().unwrap());
            match tokio_tungstenite::connect_async(request).await {
                Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
                    assert_eq!(response.status(), 503);
                    assert_eq!(response.body().as_deref(), Some(&[][..]));
                }
                other => panic!(
                    "no101 before subscription readiness: {:?}",
                    other.map(|_| ())
                ),
            }
            assert_eq!(
                server.state.ctx.live_connections(),
                0,
                "upgrade consumes no socket slot"
            );
        }
    }
    assert_eq!(
        source_case("subscription-failure")["state"]["listen_count"],
        0
    );
    assert_eq!(
        source_case("subscription-failure")["error"],
        "synthetic subscription failure"
    );
    let status = reqwest::get(format!("http://{}/social-rpc/status", server.addr))
        .await
        .unwrap();
    assert_eq!(status.status(), 200, "other routes remain available");
    let fallback = reqwest::get(format!("http://{}/social-rpc", server.addr))
        .await
        .unwrap();
    assert_eq!(fallback.status(), 404, "ordinary fallback is unchanged");
    assert_eq!(source_case("ready")["state"]["listen_count"], 1);
    bus.ready.store(true, Ordering::Release);
    tokio::time::timeout(Duration::from_secs(2), async {
        while !server.state.initial_fanout_ready() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("first ready transition");
    let (me, _client, social) = rpc_ready(&server).await;
    let mut rpc: Generator<BlockUpdate> = social
        .module
        .call_server_streams_procedure("SubscribeToBlockUpdates", ())
        .await
        .unwrap();
    let mut json = json_ready(&server).await;
    json.send(WsMessage::Text(
        serde_json::json!({"type":"subscribe","id":"startup-blocks","stream":"block_updates"})
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    let WsMessage::Text(subscribed) = next_message(&mut json).await else {
        panic!("subscribed")
    };
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&subscribed).unwrap()["type"],
        "subscribed"
    );
    tokio::time::timeout(Duration::from_secs(2), async {
        while server.state.ctx.pubsub().subscriber_count(&me) < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let address = addr("startup-event");
    let event = crate::rpc::fanout::Event::target(
        &me,
        &SocialEvent::Block(BlockUpdate {
            address: address.clone(),
            is_blocked: true,
            stream_closed: None,
        }),
    )
    .unwrap();
    assert_eq!(
        bus.publish(
            crate::comms::protocol::bus::SOCIAL_EVENTS_SUBJECT,
            event.encode(Uuid::new_v4()).unwrap()
        )
        .await,
        PublishOutcome::Submitted
    );
    let update = tokio::time::timeout(Duration::from_secs(2), rpc.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(update.address, address);
    let WsMessage::Text(update) = next_message(&mut json).await else {
        panic!("JSON update")
    };
    let update: serde_json::Value = serde_json::from_str(&update).unwrap();
    assert_eq!(update["type"], "event");
    assert_eq!(update["data"]["address"], address);
    // An outage after initial readiness does not revoke admission or existing sockets.
    assert_eq!(
        source_case("outage-after-ready")["state"]["listen_count"],
        1
    );
    assert_eq!(source_case("outage-after-ready")["state"]["close_count"], 0);
    bus.ready.store(false, Ordering::Release);
    bus.connected.store(false, Ordering::Release);
    assert!(server.state.initial_fanout_ready());
    let (_me, _another_client, _social) = rpc_ready(&server).await;
    let mut another_json = json_ready(&server).await;
    assert_eq!(server.state.ctx.live_connections(), 4);
    json.close(None).await.unwrap();
    another_json.close(None).await.unwrap();
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

#[tokio::test]
async fn a_disabled_bus_keeps_local_socket_admission_available() {
    let server = spawn_server(None, 5).await;
    server
        .state
        .start_fanout(&server.jobs, Arc::new(DisabledBus));
    assert!(server.state.initial_fanout_ready());
    let (_me, _client, _social) = rpc_ready(&server).await;
    let mut json = json_ready(&server).await;
    assert_eq!(server.state.ctx.live_connections(), 2);
    json.close(None).await.unwrap();
    server.jobs.shutdown(Duration::from_secs(2)).await;
}
