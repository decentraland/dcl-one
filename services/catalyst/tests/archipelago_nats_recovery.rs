// from: catalyrst/crates/catalyrst-archipelago/tests/nats_recovery.rs
//! The connector's reconciliation over a real broker. Connects reach the gatekeeper's island
//! subscriber by call, so what crosses the broker for them is the subscriber's lookup of each
//! announced session (`peer.{wallet}.cluster_lookup`): an observer subscribed to it stands in
//! for Pulse.
#[path = "support/archipelago.rs"]
mod archipelago;
#[path = "support/db.rs"]
mod db;
#[path = "support/gate.rs"]
mod gate;
#[path = "support/nats_broker.rs"]
mod nats_broker;
#[path = "support/pulse_tracker.rs"]
mod pulse_tracker;
use alloy::signers::{SignerSync, local::PrivateKeySigner};
use catalyst::comms::archipelago::{AppState, config::Settings, registry::SocketEvent};
use catalyst::comms::gatekeeper::admission::cluster::sockets::IslandSockets;
use catalyst::comms::gatekeeper::bus::{BrokerBus, NatsBus};
use catalyst::comms::protocol::archipelago::archipelago::{
    ChallengeRequestMessage, ClientPacket, IslandChangedMessage, ServerPacket,
    SignedChallengeMessage, client_packet, server_packet,
};
use futures::{SinkExt, StreamExt};
use nats_broker::Broker;
use prost::Message;
use std::{collections::HashSet, sync::Arc, time::Duration};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message as WsMessage;

type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

fn config() -> Settings {
    archipelago::settings()
}

/// The gatekeeper's link to the test broker: `serve`'s one NATS client, the connector's too.
fn broker_link(url: String) -> Arc<dyn NatsBus> {
    Arc::new(BrokerBus::new(Some(url), "catalyst-comms"))
}

/// The connector over a gatekeeper whose link is `link`, its feed job and the gatekeeper's
/// island subscriber (attached to its sockets) on `jobs`, as `serve` wires them.
async fn state_over(
    cfg: Settings,
    link: &Arc<dyn NatsBus>,
    jobs: &catalyst::jobs::Jobs,
) -> AppState {
    let state = catalyst::comms::archipelago::build_state(
        cfg,
        archipelago::gatekeeper(Some(link.clone())),
        jobs,
    )
    .await
    .expect("state");
    archipelago::start_island_feed(&state, jobs);
    state
}

/// Stands in for Pulse on `subject`: forwards each lookup's subject and payload, and answers
/// it at once with an empty reply, which the subscriber reads as "unknown" instead of holding
/// the request open until its deadline.
async fn pulse_stand_in(url: String, subject: &str) -> mpsc::UnboundedReceiver<(String, Vec<u8>)> {
    let client = async_nats::connect(url).await.unwrap();
    let mut lookups = client.subscribe(subject.to_owned()).await.unwrap();
    client.flush().await.unwrap();
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Some(message) = lookups.next().await {
            if let Some(reply) = message.reply.clone() {
                let _ = client.publish(reply, Vec::new().into()).await;
            }
            if tx
                .send((message.subject.to_string(), message.payload.to_vec()))
                .is_err()
            {
                break;
            }
        }
    });
    rx
}

async fn wait_for(mut condition: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(12), async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("broker state did not converge");
}

async fn next(subscriber: &mut async_nats::Subscriber) -> async_nats::Message {
    tokio::time::timeout(Duration::from_secs(12), subscriber.next())
        .await
        .expect("session was not reconciled")
        .expect("observer subscription ended")
}

async fn next_lookup(
    lookups: &mut mpsc::UnboundedReceiver<(String, Vec<u8>)>,
) -> (String, Vec<u8>) {
    tokio::time::timeout(Duration::from_secs(12), lookups.recv())
        .await
        .expect("session was not reconciled")
        .expect("the Pulse stand-in ended")
}

fn drain(lookups: &mut mpsc::UnboundedReceiver<(String, Vec<u8>)>) {
    while lookups.try_recv().is_ok() {}
}

fn packet(message: client_packet::Message) -> tokio_tungstenite::tungstenite::Bytes {
    ClientPacket {
        request_sequence: 0,
        message: Some(message),
    }
    .encode_to_vec()
    .into()
}

async fn next_packet(ws: &mut WsStream) -> server_packet::Message {
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(3), ws.next())
            .await
            .expect("server packet timed out")
            .expect("websocket closed")
            .expect("websocket frame");
        if let WsMessage::Binary(bytes) = frame {
            return ServerPacket::decode(bytes.as_ref())
                .expect("valid server packet")
                .message
                .expect("server packet body");
        }
    }
}

async fn assigned_socket(port: u16, wallet: &PrivateKeySigner) -> (WsStream, String) {
    let (ws, session, _) = assigned_socket_with_key(port, wallet).await;
    (ws, session)
}

async fn assigned_socket_with_key(
    port: u16,
    wallet: &PrivateKeySigner,
) -> (WsStream, String, PrivateKeySigner) {
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws"))
        .await
        .expect("connect websocket");
    let address = format!("{:#x}", wallet.address());
    ws.send(WsMessage::Binary(packet(
        client_packet::Message::ChallengeRequest(ChallengeRequestMessage {
            address: address.clone(),
        }),
    )))
    .await
    .expect("send challenge request");
    let challenge = match next_packet(&mut ws).await {
        server_packet::Message::ChallengeResponse(response) => response.challenge_to_sign,
        _ => panic!("challenge response expected"),
    };
    let ephemeral = PrivateKeySigner::random();
    let session = format!("{:#x}", ephemeral.address());
    let mandate = format!(
        "Decentraland Login\nEphemeral address: {session}\nExpiration: 2099-12-31T00:00:00.000Z"
    );
    let mandate_signature = wallet
        .sign_hash_sync(&alloy::primitives::eip191_hash_message(mandate.as_bytes()))
        .expect("sign mandate");
    let challenge_signature = ephemeral
        .sign_hash_sync(&alloy::primitives::eip191_hash_message(
            challenge.as_bytes(),
        ))
        .expect("sign challenge");
    let chain = serde_json::json!([
        { "type": "SIGNER", "payload": address, "signature": "" },
        { "type": "ECDSA_EPHEMERAL", "payload": mandate, "signature": mandate_signature.to_string() },
        { "type": "ECDSA_SIGNED_ENTITY", "payload": challenge, "signature": challenge_signature.to_string() }
    ]);
    ws.send(WsMessage::Binary(packet(
        client_packet::Message::SignedChallenge(SignedChallengeMessage {
            auth_chain_json: chain.to_string(),
        }),
    )))
    .await
    .expect("send signed challenge");
    assert!(matches!(
        next_packet(&mut ws).await,
        server_packet::Message::Welcome(_)
    ));
    (ws, session, ephemeral)
}

fn island(island_id: &str) -> IslandChangedMessage {
    IslandChangedMessage {
        assignment_context: None,
        island_id: island_id.into(),
        conn_str: "livekit:wss://example.invalid?access_token=t".into(),
        from_island_id: None,
        peers: Default::default(),
    }
}

async fn wait_for_island(ws: &mut WsStream) {
    assert!(matches!(
        next_packet(ws).await,
        server_packet::Message::IslandChanged(_)
    ));
}

#[tokio::test]
async fn reconciliation_announces_current_sessions_after_a_real_broker_restart() {
    /// More sessions than any queue on the way used to hold (the connector's 1,024-slot outbox).
    const SESSIONS: usize = 1_031;
    let Some(binary) = gate::require_env("ARCHIPELAGO_NATS_SERVER_BIN") else {
        return;
    };
    let broker = Broker::start(&binary, None);
    let port = broker.port;
    let cfg = config();
    let mut lookups = pulse_stand_in(broker.url(), "peer.*.cluster_lookup").await;

    let bus = broker_link(broker.url());
    let jobs = catalyst::jobs::Jobs::new();
    let state = state_over(cfg, &bus, &jobs).await;
    let address = format!("0x{:040x}", 1);
    let old_session = format!("0x{:040x}", 2);
    let (old, _old_events, _) = state.registry.on_peer_connected(&address, &old_session);
    assert!(state.gatekeeper.bus.is_enabled());
    wait_for(|| bus.is_ready()).await;
    assert_eq!(next_lookup(&mut lookups).await.1, old_session.as_bytes());
    tokio::time::sleep(Duration::from_millis(100)).await;
    drain(&mut lookups);
    assert_eq!(
        next_lookup(&mut lookups).await.1,
        old_session.as_bytes(),
        "a live stationary session must be re-announced without a reconnect"
    );

    drop(broker);
    old.close();
    state
        .registry
        .on_peer_disconnected(&address, &old_session, old.id);
    wait_for(|| !bus.is_ready()).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    drain(&mut lookups);

    let mut expected = HashSet::new();
    let mut receivers = Vec::new();
    let mut first = None;
    for index in 0..SESSIONS {
        let wallet = format!("0x{:040x}", index + 1);
        let session = format!("0x{:040x}", index + 10_000);
        let (_, events, _) = state.registry.on_peer_connected(&wallet, &session);
        if first.is_none() {
            first = Some((wallet.clone(), session.clone()));
        }
        receivers.push(events);
        expected.insert((format!("peer.{wallet}.cluster_lookup"), session));
    }
    let _recovered = Broker::start(&binary, Some(port));
    wait_for(|| bus.is_ready()).await;
    tokio::time::timeout(Duration::from_secs(20), async {
        while !expected.is_empty() {
            let (subject, payload) = next_lookup(&mut lookups).await;
            let session = String::from_utf8(payload).unwrap();
            assert_ne!(
                session, old_session,
                "a retired session was announced on the recovered broker"
            );
            expected.remove(&(subject, session));
        }
    })
    .await
    .expect("a registry larger than 1,024 sessions was starved during reconciliation");

    let (wallet, session) = first.unwrap();
    let assignment = IslandChangedMessage {
        assignment_context: None,
        island_id: "recovered-island".into(),
        ..Default::default()
    };
    state.deliver_island(&wallet, &session, &assignment, None);
    let received = tokio::time::timeout(Duration::from_secs(3), receivers[0].recv())
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(received, SocketEvent::IslandChanged(bytes, _) if bytes == assignment.encode_to_vec())
    );
    assert!(state.registry.peer_link(&address, &old_session).is_none());
    jobs.shutdown(Duration::from_secs(5)).await;
    // The link is the gatekeeper's and outlives the connector's job: it closes with the last
    // state that owns it. The stopped jobs have dropped the connector and the island subscriber;
    // the gatekeeper state (and the outbox's clone of the link) live in the connector's state,
    // and the link's supervisor and subscriptions hold no strong owner of it.
    let link = Arc::downgrade(&bus);
    drop(bus);
    drop(state);
    assert!(link.upgrade().is_none(), "the link outlived its owners");
}

#[tokio::test]
async fn broker_recovery_retries_an_assigned_socket_after_downstream_subscribes_late() {
    let Some(binary) = gate::require_env("ARCHIPELAGO_NATS_SERVER_BIN") else {
        return;
    };
    let broker = Broker::start(&binary, None);
    let port = broker.port;
    let cfg = config();
    let bus = broker_link(broker.url());
    let jobs = catalyst::jobs::Jobs::new();
    let state = state_over(cfg, &bus, &jobs).await;
    let ws_port = archipelago::serve(state.clone()).await;
    assert!(state.gatekeeper.bus.is_enabled());
    wait_for(|| bus.is_ready()).await;

    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    let observer = async_nats::connect(broker.url()).await.unwrap();
    let subject = format!("peer.{address}.cluster_lookup");
    let mut initial = observer.subscribe(subject.clone()).await.unwrap();
    observer.flush().await.unwrap();
    let (mut ws, session) = assigned_socket(ws_port, &wallet).await;
    assert_eq!(
        next(&mut initial).await.payload.as_ref(),
        session.as_bytes()
    );

    state.deliver_island(&address, &session, &island("assigned-before-restart"), None);
    wait_for_island(&mut ws).await;
    let link = state.registry.peer_link(&address, &session).unwrap();
    assert!(!link.needs_assignment(), "assignment reached the socket");
    while matches!(
        tokio::time::timeout(Duration::from_millis(100), initial.next()).await,
        Ok(Some(_))
    ) {}

    drop(broker);
    wait_for(|| !bus.is_ready()).await;
    let restarted = Broker::start(&binary, Some(port));
    wait_for(|| bus.is_ready()).await;
    assert_eq!(
        next(&mut initial).await.payload.as_ref(),
        session.as_bytes(),
        "the first recovery announcement is intentionally consumed before downstream subscribes"
    );

    let downstream = async_nats::connect(restarted.url()).await.unwrap();
    let mut after_restore = downstream.subscribe(subject).await.unwrap();
    downstream.flush().await.unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(12), after_restore.next())
            .await
            .expect("post-reconnect retry did not reach a late downstream subscription")
            .expect("downstream subscription ended")
            .payload
            .as_ref(),
        session.as_bytes()
    );
    assert_eq!(
        state.registry.peer_link(&address, &session).unwrap().id,
        link.id,
        "recovery did not resurrect a stale socket owner"
    );

    ws.close(None).await.unwrap();
    jobs.shutdown(Duration::from_secs(5)).await;
}

/// A stalled broker cannot hold readiness: the link's own probe misses its deadline and drops
/// it, nothing is announced meanwhile, and the live session is announced again once the broker
/// continues.
#[cfg(unix)]
#[tokio::test]
async fn a_stalled_broker_cannot_hold_readiness() {
    let Some(binary) = gate::require_env("ARCHIPELAGO_NATS_SERVER_BIN") else {
        return;
    };
    let mut broker = Broker::start(&binary, None);
    let cfg = config();
    let mut lookups = pulse_stand_in(broker.url(), "peer.*.cluster_lookup").await;
    let bus = broker_link(broker.url());
    let jobs = catalyst::jobs::Jobs::new();
    let state = state_over(cfg, &bus, &jobs).await;
    let address = format!("0x{:040x}", 91);
    let session = format!("0x{:040x}", 92);
    let (_, _events, _) = state.registry.on_peer_connected(&address, &session);
    assert!(state.gatekeeper.bus.is_enabled());
    wait_for(|| bus.is_ready()).await;
    assert_eq!(next_lookup(&mut lookups).await.1, session.as_bytes());

    broker.signal("-STOP");
    tokio::time::timeout(Duration::from_secs(10), async {
        while bus.is_ready() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("the probe deadline did not invalidate readiness");
    assert!(!state.outbox.is_linked());

    broker.signal("-CONT");
    wait_for(|| bus.is_ready()).await;
    drain(&mut lookups);
    assert_eq!(next_lookup(&mut lookups).await.1, session.as_bytes());
    jobs.shutdown(Duration::from_secs(5)).await;
}

/// Transport fault injection only: every lookup still traverses the real broker and real Pulse.
struct CompletionFaultBus {
    inner: Arc<dyn NatsBus>,
    lose: std::sync::atomic::AtomicUsize,
    completions: std::sync::Mutex<Vec<Vec<u8>>>,
}

#[async_trait::async_trait]
impl NatsBus for CompletionFaultBus {
    fn is_enabled(&self) -> bool {
        self.inner.is_enabled()
    }
    fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }
    fn is_ready(&self) -> bool {
        self.inner.is_ready()
    }
    fn connect(&self) {
        self.inner.connect()
    }
    fn subscribe(
        &self,
        subject: &str,
        queue: Option<&str>,
        handler: catalyst::comms::gatekeeper::bus::NatsHandler,
    ) -> catalyst::comms::gatekeeper::bus::NatsSubscription {
        self.inner.subscribe(subject, queue, handler)
    }
    async fn request(
        &self,
        subject: &str,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, catalyst::comms::gatekeeper::bus::RequestError> {
        self.inner.request(subject, payload).await
    }
    async fn publish(
        &self,
        subject: &str,
        payload: Vec<u8>,
    ) -> catalyst::comms::gatekeeper::bus::PublishOutcome {
        if subject.ends_with(".takeover_completed") {
            self.completions.lock().unwrap().push(payload.clone());
            if self
                .lose
                .fetch_update(
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                    |n| n.checked_sub(1),
                )
                .is_ok()
            {
                return catalyst::comms::gatekeeper::bus::PublishOutcome::Submitted;
            }
        }
        self.inner.publish(subject, payload).await
    }
}

struct ControlledLivekit {
    url: String,
    removals: Arc<std::sync::atomic::AtomicUsize>,
    unknown: Arc<std::sync::atomic::AtomicBool>,
    task: tokio::task::JoinHandle<()>,
}
impl ControlledLivekit {
    async fn start() -> Self {
        use axum::{Json, Router, http::StatusCode, routing::post};
        let removals = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let unknown = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let count = removals.clone();
        let fault = unknown.clone();
        let app = Router::new()
            .route("/twirp/livekit.RoomService/GetParticipant", post(|| async { (StatusCode::NOT_FOUND, Json(serde_json::json!({"code":"not_found","msg":"participant does not exist"}))) }))
            .route("/twirp/livekit.RoomService/RemoveParticipant", post(move || { let count = count.clone(); let fault = fault.clone(); async move {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if fault.load(std::sync::atomic::Ordering::SeqCst) { (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"code":"internal","msg":"unknown outcome"}))) }
                else { (StatusCode::OK, Json(serde_json::json!({}))) }
            }}));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            url,
            removals,
            unknown,
            task,
        }
    }
    fn count(&self) -> usize {
        self.removals.load(std::sync::atomic::Ordering::SeqCst)
    }
}
impl Drop for ControlledLivekit {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn real_gatekeeper(
    pool: sqlx::PgPool,
    livekit: &ControlledLivekit,
) -> catalyst::comms::gatekeeper::SharedState {
    use catalyst::comms::interfaces::test_support::{StubDirectory, StubPolicy, StubScenes};
    let mut settings = catalyst::comms::gatekeeper::Settings::for_tests();
    settings.livekit = archipelago::configured_livekit(&livekit.url);
    Arc::new(catalyst::comms::gatekeeper::CommsState::new(
        pool,
        catalyst::http::Outbound::for_tests().allowing_private_hosts(),
        settings,
        None,
        None,
        Arc::new(StubPolicy::default()),
        Arc::new(StubScenes::default()),
        Arc::new(catalyst::comms::presence::Presence::new()),
        Arc::new(StubDirectory::default()),
    ))
}

async fn real_state(
    bus: Arc<dyn NatsBus>,
    jobs: &catalyst::jobs::Jobs,
    livekit: &ControlledLivekit,
    pool: Option<sqlx::PgPool>,
    audience: Option<String>,
) -> AppState {
    let state = real_gatekeeper(
        pool.expect("production fixture requires shared PostgreSQL"),
        livekit,
    );
    let state = Arc::try_unwrap(state)
        .ok()
        .unwrap()
        .with_cluster(
            catalyst::comms::gatekeeper::admission::cluster::ClusterConfig {
                nats_url: Some("nats://fixture".into()),
                self_hosted_token_quarantine: false,
                control_v4_audience: audience,
                ..Default::default()
            },
        )
        .unwrap()
        .with_bus(bus);
    catalyst::comms::archipelago::build_state(config(), Arc::new(state), jobs)
        .await
        .unwrap()
}

async fn real_lookup(
    client: &async_nats::Client,
    wallet: &str,
    session: &str,
) -> Option<catalyst::comms::protocol::pulse::decentraland::pulse::PeerClusterSnapshot> {
    let reply = tokio::time::timeout(
        Duration::from_millis(300),
        client.request(
            format!("peer.{wallet}.cluster_lookup"),
            session.as_bytes().to_vec().into(),
        ),
    )
    .await
    .ok()?
    .ok()?;
    catalyst::comms::protocol::pulse::decentraland::pulse::PeerClusterSnapshot::decode(
        reply.payload,
    )
    .ok()
}

async fn actual_island(ws: &mut WsStream) -> IslandChangedMessage {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let frame = ws.next().await.expect("current socket open").unwrap();
            if let WsMessage::Binary(bytes) = frame {
                if let Some(server_packet::Message::IslandChanged(island)) =
                    ServerPacket::decode(bytes.as_ref()).unwrap().message
                {
                    return island;
                }
            }
        }
    })
    .await
    .expect("real tracker/feed -> NATS -> production gatekeeper -> signed WS assignment")
}

fn jwt_payload(island: &IslandChangedMessage, session: &str) -> serde_json::Value {
    use base64::Engine;
    let token = island.conn_str.split_once("access_token=").unwrap().1;
    let payload: serde_json::Value = serde_json::from_slice(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(token.split('.').nth(1).unwrap())
            .unwrap(),
    )
    .unwrap();
    let metadata: serde_json::Value =
        serde_json::from_str(payload["metadata"].as_str().unwrap()).unwrap();
    assert_eq!(metadata["catalyrstIsland"]["session"], session);
    assert_eq!(payload["video"]["room"], island.island_id);
    payload
}

async fn no_island(ws: &mut WsStream) {
    let observed = tokio::time::timeout(Duration::from_millis(400), async {
        while let Some(Ok(frame)) = ws.next().await {
            if let WsMessage::Binary(bytes) = frame {
                if matches!(
                    ServerPacket::decode(bytes.as_ref()).unwrap().message,
                    Some(server_packet::Message::IslandChanged(_))
                ) {
                    return true;
                }
            }
        }
        false
    })
    .await;
    assert!(
        !matches!(observed, Ok(true)),
        "predecessor received stale/successor credentials"
    );
}

#[tokio::test]
async fn real_pulse_retained_recovery_reaches_signed_ws_with_lost_completion_and_no_stale_packet() {
    use catalyst::comms::protocol::pulse::decentraland::pulse::TakeoverCompleted;
    let binary =
        gate::require_env("ARCHIPELAGO_NATS_SERVER_BIN").expect("mandatory real NATS fixture");
    gate::required_database_in_ci();
    let db = db::scratch_database()
        .await
        .expect("mandatory disposable PostgreSQL durable recovery fixture");
    let broker = Broker::start(&binary, None);
    let client = async_nats::connect(broker.url()).await.unwrap();
    let fault = Arc::new(CompletionFaultBus {
        inner: broker_link(broker.url()),
        lose: 1.into(),
        completions: Default::default(),
    });
    let livekit = ControlledLivekit::start().await;
    let jobs = catalyst::jobs::Jobs::new();
    let state = real_state(fault.clone(), &jobs, &livekit, Some(db.pool.clone()), None).await;
    let port = archipelago::serve(state.clone()).await;
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    let (mut a, session_a) = assigned_socket(port, &wallet).await;
    let mut world = pulse_tracker::World::new(broker.url());
    world.place(0, &address, &session_a);
    world.pass();
    // This assignment hint predates the gatekeeper subscription and is deliberately lost.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let world = Arc::new(std::sync::Mutex::new(world));
    let ticking = world.clone();
    let ticks = tokio::spawn(async move {
        loop {
            ticking.lock().unwrap().pass();
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });
    state.gatekeeper.start_island_feed(&jobs, state.clone());
    let first = actual_island(&mut a).await;
    jwt_payload(&first, &session_a);
    // Registration refresh in the same room must deliver the complete new credential.
    tokio::time::sleep(Duration::from_millis(1100)).await;
    world.lock().unwrap().place(0, &address, &session_a);
    let refreshed = actual_island(&mut a).await;
    assert_eq!(first.island_id, refreshed.island_id);
    assert_ne!(first.conn_str, refreshed.conn_str);
    jwt_payload(&refreshed, &session_a);
    let (mut b, session_b) = assigned_socket(port, &wallet).await;
    let (mut c, session_c) = assigned_socket(port, &wallet).await;
    {
        let mut world = world.lock().unwrap();
        world.place(0, &address, &session_b);
        world.pass();
        world.place(0, &address, &session_c);
        world.pass();
    }
    let successor = actual_island(&mut c).await;
    let claims = jwt_payload(&successor, &session_c);
    assert_eq!(
        livekit.count(),
        1,
        "lost completion must not repeat confirmed destructive RPC; blocked B was never admitted"
    );
    let completions = fault.completions.lock().unwrap().clone();
    assert!(completions.len() >= 2);
    assert!(completions.iter().all(|bytes| bytes == &completions[0]));
    let completed = TakeoverCompleted::decode(completions[0].as_slice()).unwrap();
    assert!(claims["nbf"].as_u64().unwrap() >= completed.revoke_before);
    let plan = real_lookup(&client, &address, &session_c).await.unwrap();
    assert_eq!(plan.token_not_before, Some(completed.revoke_before));
    assert!(plan.pending_takeovers.is_empty());
    // Duplicate/stale completion cannot spuriously advance the aggregate floor.
    client
        .publish(
            format!("peer.{address}.takeover_completed"),
            TakeoverCompleted {
                takeover_id: completed.takeover_id,
                revoke_before: completed.revoke_before + 10000,
            }
            .encode_to_vec()
            .into(),
        )
        .await
        .unwrap();
    client.flush().await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        real_lookup(&client, &address, &session_c)
            .await
            .unwrap()
            .token_not_before,
        plan.token_not_before
    );
    no_island(&mut a).await;
    no_island(&mut b).await;
    // Settled/no-in-flight gatekeeper restart loses local state while real Pulse remains live.
    // C was admitted: the next successor requires its distinct retained cleanup.
    jobs.shutdown(Duration::from_secs(5)).await;
    let jobs = catalyst::jobs::Jobs::new();
    let state = real_state(fault.clone(), &jobs, &livekit, Some(db.pool.clone()), None).await;
    let port = archipelago::serve(state.clone()).await;
    let (mut d, session_d) = assigned_socket(port, &wallet).await;
    world.lock().unwrap().place(0, &address, &session_d);
    world.lock().unwrap().pass();
    state.gatekeeper.start_island_feed(&jobs, state.clone());
    jwt_payload(&actual_island(&mut d).await, &session_d);
    assert_eq!(livekit.count(), 2);
    no_island(&mut c).await;
    ticks.abort();
    jobs.shutdown(Duration::from_secs(5)).await;
    db.teardown().await;
}

#[tokio::test]
async fn real_pulse_departed_pending_wallet_cleans_without_socket_and_unknown_dispatch_never_retries()
 {
    let binary =
        gate::require_env("ARCHIPELAGO_NATS_SERVER_BIN").expect("mandatory real NATS fixture");
    gate::required_database_in_ci();
    let db = db::scratch_database()
        .await
        .expect("mandatory disposable PostgreSQL durable recovery fixture");
    let broker = Broker::start(&binary, None);
    let client = async_nats::connect(broker.url()).await.unwrap();
    let fault = Arc::new(CompletionFaultBus {
        inner: broker_link(broker.url()),
        lose: 1.into(),
        completions: Default::default(),
    });
    let livekit = ControlledLivekit::start().await;
    let mut world = pulse_tracker::World::new(broker.url());
    let address = format!("0x{:040x}", 201);
    let a = format!("0x{:040x}", 202);
    let b = format!("0x{:040x}", 203);
    world.place(0, &address, &a);
    world.pass();
    world.place(0, &address, &b);
    world.pass();
    world.remove(0);
    world.pass();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let jobs = catalyst::jobs::Jobs::new();
    let state = real_state(fault.clone(), &jobs, &livekit, Some(db.pool.clone()), None).await;
    state.gatekeeper.start_island_feed(&jobs, state.clone());
    wait_for(|| livekit.count() == 1).await;
    tokio::time::timeout(Duration::from_secs(10), async {
        while real_lookup(&client, &address, &b).await.is_some() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(livekit.count(), 1);
    let completions = fault.completions.lock().unwrap().clone();
    assert!(completions.len() >= 2);
    assert_eq!(completions[0], completions[1]);
    assert!(state.registry.peer_link(&address, &b).is_none());
    // A separate operation gets an adapter-controlled unknown outcome and remains retained.
    let address2 = format!("0x{:040x}", 301);
    let c = format!("0x{:040x}", 302);
    let d = format!("0x{:040x}", 303);
    livekit
        .unknown
        .store(true, std::sync::atomic::Ordering::SeqCst);
    world.place(1, &address2, &c);
    world.pass();
    world.place(1, &address2, &d);
    world.pass();
    world.remove(1);
    world.pass();
    wait_for(|| livekit.count() == 2).await;
    tokio::time::sleep(Duration::from_millis(2300)).await;
    assert_eq!(
        livekit.count(),
        2,
        "periodic hints must never replay an unknown dispatch"
    );
    assert_eq!(
        real_lookup(&client, &address2, &d)
            .await
            .unwrap()
            .pending_takeovers
            .len(),
        1
    );
    jobs.shutdown(Duration::from_secs(5)).await;
    db.teardown().await;
}

#[tokio::test]
async fn real_pulse_configured_legacy_owner_completes_durable_fenced_takeover() {
    let binary =
        gate::require_env("ARCHIPELAGO_NATS_SERVER_BIN").expect("mandatory real NATS fixture");
    gate::required_database_in_ci();
    let db = db::scratch_database()
        .await
        .expect("mandatory disposable PostgreSQL fixture");
    let broker = Broker::start(&binary, None);
    let livekit = ControlledLivekit::start().await;
    let jobs = catalyst::jobs::Jobs::new();
    let state = real_state(
        broker_link(broker.url()),
        &jobs,
        &livekit,
        Some(db.pool.clone()),
        Some("retained-test".into()),
    )
    .await;
    let port = archipelago::serve(state.clone()).await;
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    let (mut a, session_a) = assigned_socket(port, &wallet).await;
    let mut world = pulse_tracker::World::new(broker.url());
    world.place(0, &address, &session_a);
    world.pass();
    let world = Arc::new(std::sync::Mutex::new(world));
    let ticking = world.clone();
    let ticks = tokio::spawn(async move {
        loop {
            ticking.lock().unwrap().pass();
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });
    state.gatekeeper.start_island_feed(&jobs, state.clone());
    jwt_payload(&actual_island(&mut a).await, &session_a);
    // An unstamped retained row cannot authorize credentials; stationary reconciliation
    // repopulates it from the actual Pulse plan even while the old token is still fresh.
    sqlx::query("UPDATE social.archipelago_v4_assignments SET assignment_json = assignment_json - 'pulseAdmission', assignment_revision = assignment_revision + 1 WHERE deployment_audience = $1 AND owner_address = $2 AND lane_key = 'realm'")
        .bind("retained-test").bind(&address).execute(&db.pool).await.unwrap();
    state.gatekeeper.announce_session(&address, &session_a);
    jwt_payload(&actual_island(&mut a).await, &session_a);
    let restored: serde_json::Value = sqlx::query_scalar("SELECT assignment_json FROM social.archipelago_v4_assignments WHERE deployment_audience = $1 AND owner_address = $2 AND lane_key = 'realm'")
        .bind("retained-test").bind(&address).fetch_one(&db.pool).await.unwrap();
    assert!(
        restored["pulseAdmission"]["assignmentVersion"]
            .as_u64()
            .unwrap()
            > 0
    );
    let (mut b, session_b) = assigned_socket(port, &wallet).await;
    world.lock().unwrap().place(0, &address, &session_b);
    let next = actual_island(&mut b).await;
    let claims = jwt_payload(&next, &session_b);
    assert_eq!(
        livekit.count(),
        1,
        "configured owner uses a positively settled durable cutoff"
    );
    let (generation, floor): (i64, i64) = sqlx::query_as(
        "SELECT generation,token_floor FROM social.island_admission_fences WHERE wallet=$1",
    )
    .bind(&address)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert!(generation > 1 && floor > 0);
    assert!(claims["nbf"].as_u64().unwrap() >= floor as u64);
    let client = async_nats::connect(broker.url()).await.unwrap();
    assert!(
        real_lookup(&client, &address, &session_b)
            .await
            .unwrap()
            .pending_takeovers
            .is_empty()
    );
    no_island(&mut a).await;
    ticks.abort();
    jobs.shutdown(Duration::from_secs(5)).await;
    drop(a);
    drop(b);
    drop(state);
    db.teardown().await;
}

async fn replica_refresh(
    state: catalyst::comms::gatekeeper::SharedState,
    root: &PrivateKeySigner,
    session: &PrivateKeySigner,
) -> (axum::http::StatusCode, Vec<u8>) {
    use catalyst::shared::signed_fetch::{
        AUTH_CHAIN_HEADER_PREFIX, AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER, build_payload_v6,
    };
    use tower::ServiceExt;
    let timestamp = chrono::Utc::now().timestamp_millis().to_string();
    let session_address = format!("{:#x}", session.address());
    let mandate = format!(
        "Decentraland Login\nEphemeral address: {session_address}\nExpiration: 2099-12-31T00:00:00.000Z"
    );
    let payload = build_payload_v6("post", "/island-refresh", &timestamp, "{}");
    let chain = serde_json::json!([
        {"type":"SIGNER","payload":format!("{:#x}",root.address()),"signature":""},
        {"type":"ECDSA_EPHEMERAL","payload":mandate,"signature":root.sign_hash_sync(&alloy::primitives::eip191_hash_message(mandate.as_bytes())).unwrap().to_string()},
        {"type":"ECDSA_SIGNED_ENTITY","payload":payload,"signature":session.sign_hash_sync(&alloy::primitives::eip191_hash_message(payload.as_bytes())).unwrap().to_string()}
    ]);
    let mut request = axum::http::Request::post("/island-refresh")
        .header("content-type", "application/x-protobuf")
        .header("accept", "application/x-protobuf")
        .header(AUTH_TIMESTAMP_HEADER, timestamp)
        .header(AUTH_METADATA_HEADER, "{}");
    for (i, link) in chain.as_array().unwrap().iter().enumerate() {
        request = request.header(format!("{AUTH_CHAIN_HEADER_PREFIX}{i}"), link.to_string());
    }
    let response = catalyst::comms::gatekeeper::router(state)
        .oneshot(request.body(axum::body::Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap()
        .to_vec();
    (status, body)
}

#[tokio::test]
async fn real_pulse_read_only_replica_refresh_and_unknown_restart_survive_ban_lift() {
    use catalyst::comms::gatekeeper::admission::cluster::{ClusterConfig, safety::SafetyStore};
    use catalyst::comms::gatekeeper::moderation::store::CreateBan;
    let binary =
        gate::require_env("ARCHIPELAGO_NATS_SERVER_BIN").expect("mandatory real NATS fixture");
    gate::required_database_in_ci();
    let db = db::scratch_database()
        .await
        .expect("mandatory disposable PostgreSQL replica fixture");
    let broker = Broker::start(&binary, None);
    let livekit = ControlledLivekit::start().await;
    let jobs = catalyst::jobs::Jobs::new();
    let primary = real_state(
        broker_link(broker.url()),
        &jobs,
        &livekit,
        Some(db.pool.clone()),
        Some("replica-test".into()),
    )
    .await;
    let port = archipelago::serve(primary.clone()).await;
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    let (mut a, session_a, key_a) = assigned_socket_with_key(port, &wallet).await;
    let mut world = pulse_tracker::World::new(broker.url());
    world.place(0, &address, &session_a);
    world.pass();
    let world = Arc::new(std::sync::Mutex::new(world));
    let ticking = world.clone();
    let ticks = tokio::spawn(async move {
        loop {
            ticking.lock().unwrap().pass();
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });
    primary.gatekeeper.start_island_feed(&jobs, primary.clone());
    actual_island(&mut a).await;
    // A separately constructed serve state starts no connector or destructive
    // subscriber. It uses the production read-only verifier and shared pool.
    let replica = real_gatekeeper(db.pool.clone(), &livekit);
    let replica_bus = broker_link(broker.url());
    let replica = Arc::new(
        Arc::try_unwrap(replica)
            .ok()
            .unwrap()
            .with_cluster(ClusterConfig {
                nats_url: Some("nats://fixture".into()),
                control_v4_audience: Some("replica-test".into()),
                island_token_ttl_seconds: 1,
                self_hosted_token_quarantine: false,
                ..Default::default()
            })
            .unwrap()
            .with_bus(replica_bus.clone()),
    );
    replica_bus.connect();
    wait_for(|| replica_bus.is_ready()).await;
    let (status, body) = replica_refresh(replica.clone(), &wallet, &key_a).await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let refreshed =
        catalyst::comms::gatekeeper::admission::island_refresh::IslandRefreshResponse::decode(
            body.as_slice(),
        )
        .unwrap();
    assert_eq!(
        refreshed.expires_in_seconds, 60,
        "refresh remains a maximum issuer even when configured island TTL is1s"
    );
    assert_eq!(livekit.count(), 0, "replica refresh performs no removal");
    livekit
        .unknown
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let (mut b, session_b) = assigned_socket(port, &wallet).await;
    world.lock().unwrap().place(0, &address, &session_b);
    world.lock().unwrap().pass();
    wait_for(|| livekit.count() == 1).await;
    let unknown:(uuid::Uuid,i64)=sqlx::query_as("SELECT dispatch_id,cutoff FROM social.island_cleanup_dispatches WHERE wallet=$1 AND state='dispatched'").bind(&address).fetch_one(&db.pool).await.unwrap();
    let actor = catalyst::comms::Address::parse(&address).unwrap();
    replica
        .moderation
        .create_ban(CreateBan {
            banned_address: actor.clone(),
            banned_by: "moderator".into(),
            reason: "test".into(),
            custom_message: None,
            banned_device_id: None,
            duration_ms: None,
        })
        .await
        .unwrap();
    replica
        .moderation
        .lift_ban(&actor, "moderator")
        .await
        .unwrap();
    let (status, body) = replica_refresh(replica.clone(), &wallet, &key_a).await;
    assert_ne!(status, axum::http::StatusCode::OK);
    assert!(body.is_empty());
    jobs.shutdown(Duration::from_secs(5)).await;
    let restarted_jobs = catalyst::jobs::Jobs::new();
    let restarted = real_state(
        broker_link(broker.url()),
        &restarted_jobs,
        &livekit,
        Some(db.pool.clone()),
        Some("replica-test".into()),
    )
    .await;
    let port = archipelago::serve(restarted.clone()).await;
    let (mut c, session_c) = assigned_socket(port, &wallet).await;
    world.lock().unwrap().place(0, &address, &session_c);
    world.lock().unwrap().pass();
    restarted
        .gatekeeper
        .start_island_feed(&restarted_jobs, restarted.clone());
    let other = PrivateKeySigner::random();
    let other_address = format!("{:#x}", other.address());
    let (mut unrelated, other_session) = assigned_socket(port, &other).await;
    world
        .lock()
        .unwrap()
        .place(1, &other_address, &other_session);
    world.lock().unwrap().pass();
    actual_island(&mut unrelated).await;
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert_eq!(
        livekit.count(),
        1,
        "restart/lift/new owner cannot repeat an unknown dispatch"
    );
    assert!(
        SafetyStore::new(db.pool.clone())
            .snapshot(&address)
            .await
            .is_err()
    );
    assert_eq!(sqlx::query_as::<_,(uuid::Uuid,i64)>("SELECT dispatch_id,cutoff FROM social.island_cleanup_dispatches WHERE wallet=$1 AND state='dispatched'").bind(&address).fetch_one(&db.pool).await.unwrap(),unknown);
    no_island(&mut b).await;
    no_island(&mut c).await;
    ticks.abort();
    restarted_jobs.shutdown(Duration::from_secs(5)).await;
    drop(primary);
    drop(restarted);
    drop(replica);
    db.teardown().await;
}

#[path = "support/public_projections.rs"]
mod public_projections;
