// from: catalyrst/crates/catalyrst-archipelago/tests/control_extensions_websocket.rs
use std::{sync::Arc, time::Duration};

use alloy::signers::{SignerSync, local::PrivateKeySigner};
use catalyst::comms::archipelago::{AppState, config::Settings};
use catalyst::comms::gatekeeper::bus::InProcessBus;
use catalyst::comms::protocol::archipelago::{archipelago as pb, decentraland::common};
use futures::{SinkExt, StreamExt};
use prost::Message as _;
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

#[path = "support/archipelago.rs"]
mod archipelago;
#[path = "support/db.rs"]
mod db;
#[path = "support/fence.rs"]
mod fence;
// `fence` loads `gate` for itself; this binary needs it at the top as well.
#[allow(clippy::duplicate_mod)]
#[path = "support/gate.rs"]
mod gate;
#[path = "support/nats_broker.rs"]
mod nats_broker;
use fence::Fixture;

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

struct Server {
    url: String,
    state: AppState,
    bus: Arc<InProcessBus>,
    task: tokio::task::JoinHandle<()>,
    plans: Arc<
        std::sync::Mutex<
            std::collections::HashMap<
                String,
                catalyst::comms::protocol::pulse::decentraland::pulse::PeerClusterSnapshot,
            >,
        >,
    >,
    _subscriber:
        Arc<catalyst::comms::gatekeeper::admission::cluster::subscriber::ClusterSubscriber>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Server {
    async fn start(fixture: &Fixture) -> Self {
        Self::with_authority(fixture, true).await
    }

    async fn with_authority(fixture: &Fixture, available: bool) -> Self {
        Self::with_pool(fixture.pool.clone(), available, None).await
    }

    async fn with_pool(
        pool: sqlx::PgPool,
        available: bool,
        handshake_timeout_ms: Option<u64>,
    ) -> Self {
        Self::with_validator(pool, available, handshake_timeout_ms, None).await
    }

    async fn with_validator(
        pool: sqlx::PgPool,
        available: bool,
        handshake_timeout_ms: Option<u64>,
        validator: Option<Arc<dyn catalyst::auth::Eip1654Validator>>,
    ) -> Self {
        Self::with_policy(pool, available, handshake_timeout_ms, validator, None, None).await
    }

    async fn with_policy(
        pool: sqlx::PgPool,
        available: bool,
        handshake_timeout_ms: Option<u64>,
        validator: Option<Arc<dyn catalyst::auth::Eip1654Validator>>,
        deny_url: Option<String>,
        bans: Option<Arc<dyn catalyst::comms::gatekeeper::moderation::BanStatus>>,
    ) -> Self {
        let mut settings = Settings::for_tests();
        settings.auth.deny_list_url = deny_url;
        if let Some(timeout) = handshake_timeout_ms {
            settings.auth.handshake_timeout_ms = timeout;
        }
        let bus = Arc::new(InProcessBus::new());
        let plans = Arc::new(std::sync::Mutex::new(std::collections::HashMap::<
            String,
            catalyst::comms::protocol::pulse::decentraland::pulse::PeerClusterSnapshot,
        >::new()));
        let replies = plans.clone();
        bus.respond_to_requests(Arc::new(move |subject, payload| {
            let lookup =
                catalyst::comms::protocol::bus::cluster::ClusterLookup::decode(subject, payload)
                    .map_err(|_| catalyst::comms::gatekeeper::bus::RequestError::Invalid)?;
            let replies = replies.lock().unwrap();
            let plan = replies
                .get(&lookup.wallet)
                .filter(|p| p.session == lookup.session)
                .ok_or(catalyst::comms::gatekeeper::bus::RequestError::NoResponders)?;
            Ok(plan.encode_to_vec())
        }));
        // An audience with no reachable fence table is the "configured but failed" authority.
        let pool = if available {
            pool
        } else {
            archipelago::lazy_pool()
        };
        let gatekeeper = archipelago::gatekeeper(Some(bus.clone()));
        let mut gatekeeper = Arc::try_unwrap(gatekeeper)
            .ok()
            .expect("unshared gatekeeper");
        gatekeeper.pool = pool;
        gatekeeper.eip1654 = validator;
        if let Some(bans) = bans {
            gatekeeper.bans = bans;
        }
        let config = catalyst::comms::gatekeeper::admission::cluster::ClusterConfig {
            control_v4_audience: Some("test-realm".into()),
            nats_url: Some("nats://fixture".into()),
            ..Default::default()
        };
        let gatekeeper = Arc::new(
            gatekeeper
                .with_cluster(config)
                .unwrap()
                .with_bus(bus.clone()),
        );
        let state = archipelago::build(settings, gatekeeper).await;
        assert_eq!(state.control_v4.is_available(), available);
        let subscriber = catalyst::comms::gatekeeper::admission::cluster::subscriber::ClusterSubscriber::new(bus.clone(), Arc::new(catalyst::comms::gatekeeper::admission::cluster::gateway::StateGateway::new(state.gatekeeper.clone())), Arc::new(catalyst::comms::gatekeeper::admission::cluster::peer_state::ClusterPeerState::default()), state.gatekeeper.cluster.clone());
        state
            .gatekeeper
            .attach_island_subscriber_for_tests(&subscriber);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}/ws", listener.local_addr().unwrap());
        let app = catalyst::comms::archipelago::mounts(
            state.clone(),
            true,
            Default::default(),
            catalyst::comms::stats::http::test_support::matcher(),
            std::sync::Arc::new(catalyst::content::Denylist::default()),
        )
        .stats;
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            url,
            state,
            bus,
            task,
            plans,
            _subscriber: subscriber,
        }
    }

    async fn stamp(
        &self,
        owner: &catalyst::comms::archipelago::control_v4::V4Owner,
        room: &str,
    ) -> catalyst::comms::gatekeeper::admission::cluster::fence::PulseAdmissionStamp {
        let safety = catalyst::comms::gatekeeper::admission::cluster::safety::SafetyStore::new(
            self.state.gatekeeper.pool.clone(),
        )
        .snapshot(&owner.address)
        .await
        .unwrap();
        let mut plans = self.plans.lock().unwrap();
        let version = plans
            .get(&owner.address)
            .map(|p| {
                p.assignment_version
                    + u64::from(
                        p.session != owner.session
                            || catalyst::comms::rooms::island_room_name(&p.cluster_id) != room,
                    )
            })
            .unwrap_or(1);
        let snapshot = catalyst::comms::protocol::pulse::decentraland::pulse::PeerClusterSnapshot {
            selected_origin: 1,
            selected_identity: Some(1),
            assignment_state: 1,
            wallet: owner.address.clone(),
            session: owner.session.clone(),
            cluster_id: room.strip_prefix("island-").unwrap_or(room).into(),
            realm: "test-realm".into(),
            source_incarnation: "legacy-stamp-fixture".into(),
            assignment_version: version,
            token_not_before: Some(0),
            pass: 1,
            pending_takeovers: vec![],
        };
        let mut stamp = catalyst::comms::gatekeeper::admission::cluster::fence::PulseAdmissionStamp::from_snapshot(&snapshot, snapshot.token_not_before.unwrap()).unwrap();
        stamp.safety_generation = safety.generation;
        stamp.safety_token_floor = safety.token_floor;
        plans.insert(owner.address.clone(), snapshot);
        stamp
    }

    async fn connect(&self) -> Socket {
        let mut request = self.url.clone().into_client_request().unwrap();
        request
            .headers_mut()
            .insert("Sec-WebSocket-Protocol", "archipelago".parse().unwrap());
        let (socket, response) = tokio_tungstenite::connect_async(request).await.unwrap();
        assert_eq!(response.headers()["Sec-WebSocket-Protocol"], "archipelago");
        socket
    }
}

/// Waits until the feed job has drained every control position queued so far: a "never
/// published" check made before then passes whatever the outbox still holds.
async fn drained(server: &Server) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while server.state.outbox.queued() > 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("the outbox never drained");
    tokio::task::yield_now().await;
}

fn packet(sequence: u64, message: pb::client_packet::Message) -> Vec<u8> {
    pb::ClientPacket {
        request_sequence: sequence,
        message: Some(message),
    }
    .encode_to_vec()
}

async fn send(socket: &mut Socket, bytes: &[u8]) {
    socket
        .send(Message::Binary(bytes.to_vec().into()))
        .await
        .unwrap();
}

async fn receive(socket: &mut Socket) -> pb::ServerPacket {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match socket
                .next()
                .await
                .expect("socket open")
                .expect("valid frame")
            {
                Message::Binary(bytes) => return pb::ServerPacket::decode(bytes.as_ref()).unwrap(),
                Message::Ping(_) | Message::Pong(_) => continue,
                other => panic!("unexpected frame: {other:?}"),
            }
        }
    })
    .await
    .expect("bounded response")
}

async fn legacy_open(server: &Server, wallet: &PrivateKeySigner) -> (Socket, String) {
    let mut socket = server.connect().await;
    send(
        &mut socket,
        &packet(
            0,
            pb::client_packet::Message::ChallengeRequest(pb::ChallengeRequestMessage {
                address: format!("{:#x}", wallet.address()),
            }),
        ),
    )
    .await;
    let Some(pb::server_packet::Message::ChallengeResponse(challenge)) =
        receive(&mut socket).await.message
    else {
        panic!("legacy challenge expected")
    };
    assert!(challenge.challenge_to_sign.starts_with("dcl-"));
    (socket, challenge.challenge_to_sign)
}

async fn legacy_sign(
    socket: &mut Socket,
    wallet: &PrivateKeySigner,
    signer: &PrivateKeySigner,
    challenge: &str,
) {
    let mut links = vec![
        serde_json::json!({"type": "SIGNER", "payload": format!("{:#x}", wallet.address()), "signature": ""}),
    ];
    if wallet.address() != signer.address() {
        let mandate = format!(
            "Decentraland Login\nEphemeral address: {:#x}\nExpiration: 2099-12-31T00:00:00.000Z",
            signer.address()
        );
        let signature = wallet
            .sign_message_sync(mandate.as_bytes())
            .unwrap()
            .to_string();
        links.push(serde_json::json!({"type": "ECDSA_EPHEMERAL", "payload": mandate, "signature": signature}));
    }
    links.push(serde_json::json!({"type": "ECDSA_SIGNED_ENTITY", "payload": challenge, "signature": signer.sign_message_sync(challenge.as_bytes()).unwrap().to_string()}));
    send(
        socket,
        &packet(
            0,
            pb::client_packet::Message::SignedChallenge(pb::SignedChallengeMessage {
                auth_chain_json: serde_json::to_string(&links).unwrap(),
            }),
        ),
    )
    .await;
}

async fn current_owner(
    server: &Server,
    fixture: &Fixture,
    wallet: &PrivateKeySigner,
) -> catalyst::comms::archipelago::control_v4::V4Owner {
    let address = format!("{:#x}", wallet.address());
    let (session, epoch): (String, i64) = sqlx::query_as("SELECT owner_session, owner_epoch FROM social.archipelago_v4_assignments WHERE deployment_audience = $1 AND owner_address = $2 AND lane_key = 'realm'")
        .bind(&server.state.gatekeeper.cluster.control_v4_audience).bind(&address).fetch_one(&fixture.pool).await.unwrap();
    catalyst::comms::archipelago::control_v4::V4Owner {
        address,
        session,
        epoch: epoch as u64,
    }
}

async fn admit_legacy(socket: &mut Socket, wallet: &PrivateKeySigner) {
    send(
        socket,
        &packet(
            0,
            pb::client_packet::Message::ChallengeRequest(pb::ChallengeRequestMessage {
                address: format!("{:#x}", wallet.address()),
            }),
        ),
    )
    .await;
    let Some(pb::server_packet::Message::ChallengeResponse(challenge)) =
        receive(socket).await.message
    else {
        panic!("legacy challenge expected")
    };
    assert!(challenge.challenge_to_sign.starts_with("dcl-"));
    legacy_sign(socket, wallet, wallet, &challenge.challenge_to_sign).await;
    let welcome = receive(socket).await;
    assert_eq!(welcome.request_sequence, 0);
    let Some(pb::server_packet::Message::Welcome(welcome)) = welcome.message else {
        panic!("legacy welcome expected")
    };
    assert_eq!(welcome.peer_id, format!("{:#x}", wallet.address()));
    assert_eq!(welcome.encoded_len(), 44);
}

#[tokio::test]
async fn fenced_completed_assignment_peer_close_echoes_then_releases_exact_owner() {
    use catalyst::comms::archipelago::control_v4::{AssignmentPayload, LaneKey};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    let server = Server::start(&fixture).await;
    let wallet: PrivateKeySigner =
        "0000000000000000000000000000000000000000000000000000000000000001"
            .parse()
            .unwrap();
    let address = format!("{:#x}", wallet.address());
    let mut socket = server.connect().await;
    admit_legacy(&mut socket, &wallet).await;
    let owner = current_owner(&server, &fixture, &wallet).await;
    let lane = LaneKey::parse("realm").unwrap();
    server
        .state
        .control_v4
        .publish_assignment(
            &owner,
            &lane,
            AssignmentPayload {
                island_id: "island-close".into(),
                connection_string: "fixture-credential".into(),
                from_island_id: None,
                peers: Default::default(),
                pulse_admission: Some(server.stamp(&owner, "island-close").await),
                token_expires_at_unix: None,
            },
        )
        .await
        .unwrap();
    let Some(pb::server_packet::Message::IslandChanged(assignment)) =
        receive(&mut socket).await.message
    else {
        panic!("complete authorized assignment expected")
    };
    assert_eq!(assignment.conn_str, "fixture-credential");
    let tokio_tungstenite::MaybeTlsStream::Plain(tcp) = socket.get_mut() else {
        panic!("plaintext")
    };
    let at = std::time::Instant::now();
    let close = [
        0x88, 0x88, 1, 2, 3, 4, 2, 0xea, 0x65, 0x61, 0x6f, 0x61, 0x66, 0x60,
    ];
    eprintln!(
        "CLOSE_FENCED client={close:02x?} at_us={}",
        at.elapsed().as_micros()
    );
    tcp.write_all(&close).await.unwrap();
    let mut received = vec![];
    tokio::time::timeout(Duration::from_secs(7), tcp.read_to_end(&mut received))
        .await
        .unwrap()
        .unwrap();
    eprintln!(
        "CLOSE_FENCED server={received:02x?} eof_at_us={}",
        at.elapsed().as_micros()
    );
    assert_eq!(received, b"\x88\x08\x03\xe8fenced");
    tokio::time::timeout(Duration::from_secs(3), async {
        while server.state.registry.has_any_peer(&address)
            || server.state.sockets.available_permits()
                != server.state.settings.server.max_sockets as usize
            || server.state.handlers.available_permits()
                != server.state.settings.server.max_sockets as usize
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(
        server
            .state
            .control_v4
            .snapshot(&owner, &lane)
            .await
            .is_err()
    );
    assert!(server.state.enforcement.island_of(&address).is_none());
    let disconnects = || {
        use catalyst::comms::protocol::bus::statistics::{
            Event, OBSERVATIONS_SUBJECT, Observation,
        };
        server.bus.published().iter().filter(|(subject, payload)| {
            subject == OBSERVATIONS_SUBJECT && Observation::decode(payload).is_some_and(|observation| matches!(observation.event, Event::Disconnect { address: wallet } if wallet == address))
        }).count()
    };
    tokio::time::timeout(Duration::from_secs(3), async {
        while disconnects() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(disconnects(), 1);
    drop((socket, server));
    fixture.finish().await;
}

async fn legacy_barrier(socket: &mut Socket, token: u8) {
    socket
        .send(Message::Ping(vec![token].into()))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match socket
                .next()
                .await
                .expect("legacy socket stays open")
                .expect("valid transport")
            {
                Message::Pong(payload) if payload.as_ref() == [token] => return,
                Message::Pong(payload) if payload.len() == 1 && payload[0] < token => {}
                Message::Ping(payload) => socket.send(Message::Pong(payload)).await.unwrap(),
                Message::Binary(bytes) => assert!(
                    matches!(
                        pb::ServerPacket::decode(bytes).unwrap().message,
                        Some(pb::server_packet::Message::IslandChanged(_))
                    ),
                    "ordinary heartbeats must not enqueue acknowledgements or errors"
                ),
                frame => panic!("unexpected legacy barrier frame: {frame:?}"),
            }
        }
    })
    .await
    .expect("server must consume preceding heartbeat frames and answer the ping");
}

#[tokio::test]
async fn configured_legacy_heartbeat_reaches_real_pulse_clustering_over_nats() {
    use catalyst::comms::gatekeeper::bus::BrokerBus;
    use catalyst::comms::pulse::{
        cluster::{
            ClusterOptions, ClusterTracker,
            nats::{NatsClusterFeed, NatsFeedOptions},
        },
        decentraland::pulse::PeerClusterSnapshot,
        interest::SPATIAL_GRID_CELL_SIZE,
        realm_grids::RealmSpatialGrids,
        snapshot::{IdentityBoard, SnapshotBoard},
    };
    let Some(binary) = gate::require_env("PULSE_NATS_SERVER_BIN") else {
        return;
    };
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    let broker = nats_broker::Broker::start(&binary, None);
    let feed = Arc::new(NatsClusterFeed::spawn(NatsFeedOptions {
        url: broker.url(),
        discovery_interval_ms: 0,
        ..Default::default()
    }));
    feed.enable_control_positions("test-realm").unwrap();
    let bus = Arc::new(BrokerBus::new(Some(broker.url()), "catalyst-comms"));
    let mut gatekeeper = archipelago::gatekeeper(Some(bus));
    let shared = Arc::get_mut(&mut gatekeeper).unwrap();
    shared.pool = fixture.pool.clone();
    shared.cluster.control_v4_audience = Some("test-realm".into());
    let state = archipelago::build(Settings::for_tests(), gatekeeper).await;
    assert!(state.control_v4.is_available());
    let port = archipelago::serve(state.clone()).await;
    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws"))
        .await
        .unwrap();
    let wallet = PrivateKeySigner::random();
    admit_legacy(&mut socket, &wallet).await;
    let address = format!("{:#x}", wallet.address());
    let heartbeat = packet(
        0,
        pb::client_packet::Message::Heartbeat(pb::Heartbeat {
            position: Some(common::Position {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            }),
            desired_room: Some("shared-realm".into()),
            sample_sequence: None,
        }),
    );
    let mut tracker = ClusterTracker::new(
        ClusterOptions {
            dwell_passes: 1,
            ..Default::default()
        },
        8,
        feed.clone(),
    );
    let grids = RealmSpatialGrids::new(SPATIAL_GRID_CELL_SIZE, 8);
    let board = SnapshotBoard::new(8, 4);
    let mut identity = IdentityBoard::new(8);
    tracker.bind_identity(&mut identity);
    let client = async_nats::connect(broker.url()).await.unwrap();
    let snapshot = tokio::time::timeout(Duration::from_secs(12), async {
        loop {
            send(&mut socket, &heartbeat).await;
            tracker.run_pass(&grids, &board, &identity);
            if let Ok(Ok(reply)) = tokio::time::timeout(
                Duration::from_millis(200),
                client.request(
                    format!("peer.{address}.cluster_lookup"),
                    address.clone().into_bytes().into(),
                ),
            )
            .await
            {
                let snapshot = PeerClusterSnapshot::decode(reply.payload).unwrap();
                if !snapshot.cluster_id.is_empty() {
                    break snapshot;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("configured legacy heartbeat must reach Pulse's real producer/lookup state");
    assert_eq!(snapshot.wallet, address);
    assert_eq!(snapshot.session, address);
    assert_eq!(snapshot.realm, "shared-realm");
    assert!(snapshot.pass > 0);
    assert!(!snapshot.source_incarnation.is_empty());
    assert_eq!(tracker.board().peers[0].position.x, 1.0);
    socket.close(None).await.unwrap();
    state.restart.cancel();
    drop((socket, state, tracker, feed, client));
    fixture.finish().await;
}

#[tokio::test]
async fn control_heartbeats_publish_only_current_realm_owners_and_epoch_scoped_closes() {
    use catalyst::comms::protocol::bus::control_position::{ControlPosition, SUBJECT};
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    let server = Server::start(&fixture).await;
    let wallet = PrivateKeySigner::random();
    let (mut legacy, challenge) = legacy_open(&server, &wallet).await;
    legacy_sign(&mut legacy, &wallet, &wallet, &challenge).await;
    assert!(matches!(
        receive(&mut legacy).await.message,
        Some(pb::server_packet::Message::Welcome(_))
    ));
    let old = current_owner(&server, &fixture, &wallet).await;
    let heartbeat = packet(
        0,
        pb::client_packet::Message::Heartbeat(pb::Heartbeat {
            position: Some(common::Position {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            }),
            desired_room: Some("shared-realm".into()),
            sample_sequence: None,
        }),
    );
    let records = || {
        server
            .bus
            .published()
            .into_iter()
            .filter(|(subject, _)| subject == SUBJECT)
            .map(|(_, payload)| ControlPosition::decode(&payload).expect("valid control position"))
            .collect::<Vec<_>>()
    };
    send(&mut legacy, &heartbeat).await;
    tokio::time::timeout(Duration::from_secs(3), async {
        while !records()
            .iter()
            .any(|record| record.epoch == old.epoch && record.position.is_some())
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("legacy heartbeat publishes without a Pulse connection");
    let first = records()
        .into_iter()
        .find(|record| record.position.is_some())
        .unwrap();
    assert_eq!(first.wallet, old.address);
    assert_eq!(first.session, old.session);
    assert_eq!(first.audience, "test-realm");
    assert_eq!(first.realm, "shared-realm");
    assert_eq!(first.position, Some([1.0, 2.0, 3.0]));

    let mut enhanced = server.connect().await;
    admit_legacy(&mut enhanced, &wallet).await;
    let next = current_owner(&server, &fixture, &wallet).await;
    assert!(next.epoch > old.epoch);
    send(&mut enhanced, &heartbeat).await;
    tokio::time::timeout(Duration::from_secs(3), async {
        while !records()
            .iter()
            .any(|record| record.epoch == next.epoch && record.position.is_some())
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("replacement legacy heartbeat publishes without a Pulse connection");
    drop(legacy);
    tokio::time::timeout(Duration::from_secs(3), async {
        while !records()
            .iter()
            .any(|record| record.epoch == old.epoch && record.position.is_none())
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("superseded socket closes its own epoch only");
    drained(&server).await;
    assert!(
        !records()
            .iter()
            .any(|record| record.epoch == next.epoch && record.position.is_none())
    );

    let scene_lane =
        catalyst::comms::archipelago::control_v4::LaneKey::parse("scene:plaza").unwrap();
    let realm_lane = catalyst::comms::archipelago::control_v4::LaneKey::parse("realm").unwrap();
    let scene_owner = catalyst::comms::archipelago::control_v4::V4Owner {
        address: next.address.clone(),
        session: next.session.clone(),
        epoch: next.epoch + 1,
    };
    let before = records().len();
    server
        .state
        .control_v4
        .claim_lanes(&scene_owner, &[scene_lane])
        .await
        .unwrap();
    assert!(
        server
            .state
            .control_v4
            .snapshot(&next, &realm_lane)
            .await
            .is_ok(),
        "a scene-only owner cannot displace the current realm socket"
    );
    server.state.control_v4.release(&scene_owner).await.unwrap();
    assert!(
        server
            .state
            .control_v4
            .snapshot(&next, &realm_lane)
            .await
            .is_ok(),
        "scene-only release cannot release the realm owner"
    );
    drained(&server).await;
    assert_eq!(
        records().len(),
        before,
        "scene-only ownership cannot move or close realm membership"
    );
    enhanced.close(None).await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while !records()
            .iter()
            .any(|record| record.epoch == next.epoch && record.position.is_none())
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("current realm close publishes tombstone");
    for pair in records().windows(2) {
        assert!(pair[1].sequence > pair[0].sequence);
    }
    drop(server);
    fixture.finish().await;
}

#[tokio::test]
async fn legacy_heartbeat_authority_deadline_never_publishes_an_unverified_position() {
    use catalyst::comms::protocol::bus::control_position::{ControlPosition, SUBJECT};
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    let server = Server::start(&fixture).await;
    let wallet = PrivateKeySigner::random();
    let mut socket = server.connect().await;
    admit_legacy(&mut socket, &wallet).await;
    let mut held = Vec::new();
    for _ in 0..4 {
        held.push(fixture.pool.acquire().await.unwrap());
    }
    let started = std::time::Instant::now();
    send(
        &mut socket,
        &packet(
            0,
            pb::client_packet::Message::Heartbeat(pb::Heartbeat {
                position: Some(common::Position {
                    x: 99.0,
                    y: 0.0,
                    z: 99.0,
                }),
                desired_room: Some("test-realm".into()),
                sample_sequence: None,
            }),
        ),
    )
    .await;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match socket.next().await {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => break,
                Some(Ok(Message::Binary(bytes))) => {
                    let response = pb::ServerPacket::decode(bytes).unwrap();
                    let Some(pb::server_packet::Message::Error(error)) = response.message else {
                        panic!("unavailable authority must not grant or acknowledge position")
                    };
                    assert_eq!(error.code, 1005);
                    break;
                }
                Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {}
                other => panic!("unexpected authority failure reply {other:?}"),
            }
        }
    })
    .await
    .expect("whole authority operation is bounded, including pool acquisition");
    assert!(started.elapsed() < Duration::from_secs(5));
    drained(&server).await;
    assert!(
        !server
            .bus
            .published()
            .iter()
            .filter(|(subject, _)| subject == SUBJECT)
            .filter_map(|(_, bytes)| ControlPosition::decode(bytes))
            .any(|record| record.position.is_some())
    );
    assert!(
        server.bus.published().iter().any(|(subject, bytes)| subject
            == catalyst::comms::protocol::bus::statistics::OBSERVATIONS_SUBJECT
            && catalyst::comms::protocol::bus::statistics::Observation::decode(bytes).is_some()),
        "authenticated statistics observations do not acquire ownership"
    );
    drop(held);
    drop(socket);
    drop(server);
    fixture.finish().await;
}

#[tokio::test]
async fn a_legacy_socket_receives_the_new_fenced_same_room_assignment() {
    use catalyst::comms::archipelago::control_v4::{AssignmentPayload, LaneKey};
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    let server = Server::start(&fixture).await;
    let realm = LaneKey::parse("realm").unwrap();
    for delegated in [false, true] {
        let wallet = PrivateKeySigner::random();
        let signer = if delegated {
            PrivateKeySigner::random()
        } else {
            wallet.clone()
        };
        let mut old_socket = server.connect().await;
        admit_legacy(&mut old_socket, &wallet).await;
        let old = current_owner(&server, &fixture, &wallet).await;
        let old_state = server
            .state
            .control_v4
            .snapshot(&old, &realm)
            .await
            .unwrap();
        let (mut legacy, challenge) = legacy_open(&server, &wallet).await;
        legacy_sign(&mut legacy, &wallet, &signer, &challenge).await;
        let welcome = receive(&mut legacy).await;
        assert_eq!(welcome.request_sequence, 0);
        let Some(pb::server_packet::Message::Welcome(welcome)) = welcome.message else {
            panic!("legacy welcome")
        };
        assert_eq!(welcome.encoded_len(), 44);
        let next = current_owner(&server, &fixture, &wallet).await;
        assert!(next.epoch > old.epoch);
        assert_eq!(next.session, format!("{:#x}", signer.address()));
        let claimed = server
            .state
            .control_v4
            .snapshot(&next, &realm)
            .await
            .unwrap();
        assert!(claimed.assignment.is_none());
        assert!(claimed.fencing_token > old_state.fencing_token);
        assert!(server.state.control_v4.renew(&old).await.is_err());
        assert!(server.state.control_v4.release(&old).await.is_err());
        assert!(
            server
                .state
                .control_v4
                .snapshot(&old, &realm)
                .await
                .is_err()
        );
        for token in ["first-current-credential", "renewed-same-room-credential"] {
            server
                .state
                .control_v4
                .publish_assignment(
                    &next,
                    &realm,
                    AssignmentPayload {
                        island_id: "island-same-room".into(),
                        connection_string: token.into(),
                        from_island_id: None,
                        peers: Default::default(),
                        pulse_admission: Some(server.stamp(&next, "island-same-room").await),
                        token_expires_at_unix: None,
                    },
                )
                .await
                .unwrap();
            let packet = receive(&mut legacy).await;
            assert_eq!(packet.request_sequence, 0);
            let Some(pb::server_packet::Message::IslandChanged(assignment)) = packet.message else {
                panic!("legacy assignment")
            };
            assert_eq!(assignment.island_id, "island-same-room");
            assert_eq!(assignment.conn_str, token);
            assert!(assignment.assignment_context.is_none());
        }
        assert!(
            server
                .state
                .registry
                .peer_link(&next.address, &next.session)
                .is_none(),
            "protected socket must not consume the unfenced feed"
        );
        let mut replacement = server.connect().await;
        admit_legacy(&mut replacement, &wallet).await;
        let replacement_owner = current_owner(&server, &fixture, &wallet).await;
        drop(legacy);
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(
            server
                .state
                .control_v4
                .snapshot(&replacement_owner, &realm)
                .await
                .is_ok(),
            "old legacy close must not release the newer owner"
        );
        drop((old_socket, replacement));
    }
    drop(server);
    fixture.finish().await;
}

#[tokio::test]
async fn a_delayed_legacy_authentication_cannot_take_a_newer_legacy_owner() {
    use catalyst::comms::archipelago::control_v4::LaneKey;
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    let server = Server::start(&fixture).await;
    let other_replica = Server::start(&fixture).await;
    let wallet = PrivateKeySigner::random();
    let (mut delayed, challenge) = legacy_open(&server, &wallet).await;
    let mut newer = other_replica.connect().await;
    admit_legacy(&mut newer, &wallet).await;
    let owner = current_owner(&server, &fixture, &wallet).await;
    let realm = LaneKey::parse("realm").unwrap();
    let before = server
        .state
        .control_v4
        .snapshot(&owner, &realm)
        .await
        .unwrap();
    legacy_sign(&mut delayed, &wallet, &wallet, &challenge).await;
    assert!(matches!(
        receive(&mut delayed).await.message,
        Some(pb::server_packet::Message::Kicked(_))
    ));
    assert_eq!(
        before,
        server
            .state
            .control_v4
            .snapshot(&owner, &realm)
            .await
            .unwrap()
    );
    drop((delayed, newer, server, other_replica));
    fixture.finish().await;
}

#[tokio::test]
async fn baseline_legacy_ws_reconnect_preserves_ownership() {
    use catalyst::comms::archipelago::control_v4::{AssignmentPayload, LaneKey};
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    let first_replica = Server::start(&fixture).await;
    let second_replica = Server::start(&fixture).await;
    let realm = LaneKey::parse("realm").unwrap();
    for delegated in [false, true] {
        let wallet = PrivateKeySigner::random();
        let next_signer = if delegated {
            PrivateKeySigner::random()
        } else {
            wallet.clone()
        };
        let (mut old_socket, old_challenge) = legacy_open(&first_replica, &wallet).await;
        legacy_sign(&mut old_socket, &wallet, &wallet, &old_challenge).await;
        assert!(matches!(
            receive(&mut old_socket).await.message,
            Some(pb::server_packet::Message::Welcome(_))
        ));
        let old_owner = current_owner(&first_replica, &fixture, &wallet).await;

        let (mut current_socket, current_challenge) = legacy_open(&second_replica, &wallet).await;
        legacy_sign(
            &mut current_socket,
            &wallet,
            &next_signer,
            &current_challenge,
        )
        .await;
        assert!(matches!(
            receive(&mut current_socket).await.message,
            Some(pb::server_packet::Message::Welcome(_))
        ));
        let current_owner = current_owner(&second_replica, &fixture, &wallet).await;
        assert!(current_owner.epoch > old_owner.epoch);
        assert_eq!(
            current_owner.session,
            format!("{:#x}", next_signer.address())
        );
        assert!(matches!(
            receive(&mut old_socket).await.message,
            Some(pb::server_packet::Message::Kicked(_))
        ));

        second_replica
            .state
            .control_v4
            .publish_assignment(
                &current_owner,
                &realm,
                AssignmentPayload {
                    island_id: "island-replica-current-room".into(),
                    connection_string: "replica-current-credential".into(),
                    from_island_id: None,
                    peers: Default::default(),
                    pulse_admission: Some(
                        second_replica
                            .stamp(&current_owner, "island-replica-current-room")
                            .await,
                    ),
                    token_expires_at_unix: None,
                },
            )
            .await
            .unwrap();
        let Some(pb::server_packet::Message::IslandChanged(assignment)) =
            receive(&mut current_socket).await.message
        else {
            panic!("current replica assignment expected")
        };
        assert_eq!(assignment.island_id, "island-replica-current-room");
        assert_eq!(assignment.conn_str, "replica-current-credential");
        assert!(assignment.assignment_context.is_none());
        assert!(
            first_replica
                .state
                .control_v4
                .snapshot(&old_owner, &realm)
                .await
                .is_err()
        );
        drop((old_socket, current_socket));
    }
    drop((first_replica, second_replica));
    fixture.finish().await;
}

#[tokio::test]
async fn a_stalled_authority_closes_a_protected_legacy_socket_within_the_operation_budget() {
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    let server = Server::start(&fixture).await;
    let wallet = PrivateKeySigner::random();
    let (mut socket, challenge) = legacy_open(&server, &wallet).await;
    legacy_sign(&mut socket, &wallet, &wallet, &challenge).await;
    assert!(matches!(
        receive(&mut socket).await.message,
        Some(pb::server_packet::Message::Welcome(_))
    ));

    let mut held = Vec::new();
    for _ in 0..4 {
        held.push(fixture.pool.acquire().await.unwrap());
    }
    tokio::time::timeout(Duration::from_secs(7), async {
        loop {
            match socket.next().await {
                None | Some(Ok(Message::Close(_))) => break,
                Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {}
                Some(Ok(frame)) => {
                    panic!("authority outage must not emit a fallback frame: {frame:?}")
                }
                Some(Err(_)) => break,
            }
        }
    })
    .await
    .expect("authority operation budget must bound the protected socket lifetime");
    drop(held);
    drop((socket, server));
    fixture.finish().await;
}

async fn legacy_claim_recovers_after_database_stall(statement_timeout_ms: u64) {
    use catalyst::comms::archipelago::control_v4::{AssignmentPayload, LaneKey};
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    let authority_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with(fixture.pool.connect_options().as_ref().clone())
        .await
        .unwrap();
    let server = Server::with_pool(authority_pool.clone(), true, Some(1500)).await;
    sqlx::query("SELECT set_config('statement_timeout', $1, false)")
        .bind(format!("{statement_timeout_ms}ms"))
        .execute(&authority_pool)
        .await
        .unwrap();
    let wallet = PrivateKeySigner::random();
    let (mut interrupted, challenge) = legacy_open(&server, &wallet).await;
    let mut held = fixture.pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE social.archipelago_v4_assignments IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *held)
        .await
        .unwrap();
    legacy_sign(&mut interrupted, &wallet, &wallet, &challenge).await;
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            match interrupted.next().await {
                None | Some(Ok(Message::Close(_))) | Some(Err(_)) => break,
                Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {}
                Some(Ok(Message::Binary(bytes))) => {
                    let packet = pb::ServerPacket::decode(bytes.as_ref()).unwrap();
                    assert!(
                        !matches!(packet.message, Some(pb::server_packet::Message::Kicked(_))),
                        "a database stall after the challenge must not terminally reject the client"
                    );
                    panic!("no admission is valid while the authority is stalled: {packet:?}");
                }
                Some(Ok(frame)) => panic!("unexpected stalled-claim frame: {frame:?}"),
            }
        }
    })
    .await
    .expect("stalled claim must close within its handshake budget");
    held.rollback().await.unwrap();
    sqlx::query("SELECT 1")
        .execute(&authority_pool)
        .await
        .unwrap();
    let address = format!("{:#x}", wallet.address());
    let claimed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM social.archipelago_v4_assignments WHERE owner_address = $1",
    )
    .bind(&address)
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert_eq!(claimed, 0, "a cancelled claim must not commit later");

    let (mut retry, fresh_challenge) = legacy_open(&server, &wallet).await;
    assert_ne!(challenge, fresh_challenge);
    legacy_sign(&mut retry, &wallet, &wallet, &fresh_challenge).await;
    assert!(matches!(
        receive(&mut retry).await.message,
        Some(pb::server_packet::Message::Welcome(_))
    ));
    let owner = current_owner(&server, &fixture, &wallet).await;
    server
        .state
        .control_v4
        .publish_assignment(
            &owner,
            &LaneKey::parse("realm").unwrap(),
            AssignmentPayload {
                island_id: "island-recovered-room".into(),
                connection_string: "recovered-credential".into(),
                from_island_id: None,
                peers: Default::default(),
                pulse_admission: Some(server.stamp(&owner, "island-recovered-room").await),
                token_expires_at_unix: None,
            },
        )
        .await
        .unwrap();
    let Some(pb::server_packet::Message::IslandChanged(assignment)) =
        receive(&mut retry).await.message
    else {
        panic!("reconnected client must receive the current room")
    };
    assert_eq!(assignment.island_id, "island-recovered-room");
    drop((interrupted, retry, server));
    authority_pool.close().await;
    fixture.finish().await;
}

#[tokio::test]
async fn authority_unavailable_during_legacy_claim_is_retryable() {
    legacy_claim_recovers_after_database_stall(200).await;
}

#[tokio::test]
async fn authority_deadline_during_legacy_claim_is_retryable() {
    legacy_claim_recovers_after_database_stall(10_000).await;
}

#[tokio::test]
async fn configured_authority_failure_does_not_offer_an_unfenced_legacy_handshake() {
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    let server = Server::with_authority(&fixture, false).await;
    let wallet = PrivateKeySigner::random();
    let mut socket = server.connect().await;
    send(
        &mut socket,
        &packet(
            0,
            pb::client_packet::Message::ChallengeRequest(pb::ChallengeRequestMessage {
                address: format!("{:#x}", wallet.address()),
            }),
        ),
    )
    .await;
    let frame = tokio::time::timeout(Duration::from_secs(2), socket.next())
        .await
        .unwrap();
    assert!(
        !matches!(frame, Some(Ok(Message::Binary(_)))),
        "no unfenced challenge or welcome may be offered"
    );
    assert!(
        !server
            .state
            .registry
            .has_any_peer(&format!("{:#x}", wallet.address()))
    );
    drop((socket, server));
    fixture.finish().await;
}

#[tokio::test]
async fn common_endpoint_keeps_legacy_bytes_heartbeats_and_invalid_input_bounds() {
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    let server = Server::start(&fixture).await;
    let last_control = || {
        server
            .bus
            .published()
            .iter()
            .filter(|(subject, _)| {
                subject == catalyst::comms::protocol::bus::control_position::SUBJECT
            })
            .filter_map(|(_, bytes)| {
                catalyst::comms::protocol::bus::control_position::ControlPosition::decode(bytes)
            })
            .filter_map(|record| record.position)
            .last()
    };
    let wallet = PrivateKeySigner::random();
    let legacy_wallet = PrivateKeySigner::random();
    let mut legacy = server.connect().await;
    send(
        &mut legacy,
        &packet(
            0,
            pb::client_packet::Message::ChallengeRequest(pb::ChallengeRequestMessage {
                address: format!("{:#x}", legacy_wallet.address()),
            }),
        ),
    )
    .await;
    let Some(pb::server_packet::Message::ChallengeResponse(challenge)) =
        receive(&mut legacy).await.message
    else {
        panic!("legacy challenge expected")
    };
    assert!(challenge.challenge_to_sign.starts_with("dcl-"));
    let signature = legacy_wallet
        .sign_message_sync(challenge.challenge_to_sign.as_bytes())
        .unwrap()
        .to_string();
    let chain = serde_json::json!([
        {"type":"SIGNER", "payload":format!("{:#x}", legacy_wallet.address()), "signature":""},
        {"type":"ECDSA_SIGNED_ENTITY", "payload":challenge.challenge_to_sign, "signature":signature}
    ]);
    send(
        &mut legacy,
        &packet(
            0,
            pb::client_packet::Message::SignedChallenge(pb::SignedChallengeMessage {
                auth_chain_json: chain.to_string(),
            }),
        ),
    )
    .await;
    let welcome = receive(&mut legacy).await;
    assert_eq!(welcome.request_sequence, 0);
    assert!(matches!(
        welcome.message,
        Some(pb::server_packet::Message::Welcome(
            pb::WelcomeMessage { .. }
        ))
    ));
    let mut current = server.connect().await;
    admit_legacy(&mut current, &wallet).await;
    let position = |x, desired_room: Option<String>| {
        packet(
            0,
            pb::client_packet::Message::Heartbeat(pb::Heartbeat {
                position: Some(common::Position { x, y: 2.0, z: 3.0 }),
                desired_room,
                sample_sequence: None,
            }),
        )
    };
    let heartbeat = position(1.0, None);
    assert_eq!(heartbeat.len(), 19);
    for _ in 0..1101 {
        send(&mut current, &heartbeat).await;
    }
    legacy_barrier(&mut current, 1).await;
    tokio::time::timeout(Duration::from_secs(3), async {
        while !last_control().is_some_and(|position| position == [1.0, 2.0, 3.0]) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("initial full legacy position applies");
    send(&mut current, &position(2.0, None)).await;
    send(&mut current, &position(3.0, None)).await;
    legacy_barrier(&mut current, 2).await;
    tokio::time::timeout(Duration::from_secs(3), async {
        while last_control().unwrap()[0] != 3.0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the latest quiet sample eventually applies");
    for desired_room in ["x".repeat(257), "room\nname".into()] {
        send(&mut current, &position(99.0, Some(desired_room))).await;
    }
    send(&mut current, &position(f32::NAN, None)).await;
    legacy_barrier(&mut current, 3).await;
    tokio::time::pause();
    tokio::time::advance(Duration::from_millis(600)).await;
    tokio::time::resume();
    legacy_barrier(&mut current, 3).await;
    assert_eq!(
        last_control().unwrap(),
        [3.0, 2.0, 3.0],
        "consumed invalid realms and non-finite positions cannot replace the current sample"
    );
    send(&mut current, &position(4.0, None)).await;
    legacy_barrier(&mut current, 4).await;
    tokio::time::timeout(Duration::from_secs(3), async {
        while last_control().unwrap()[0] != 4.0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the same legacy connection remains usable after invalid inputs");
    current.close(None).await.unwrap();
    legacy.close(None).await.unwrap();
    drop(server);
    fixture.finish().await;
}

#[tokio::test]
async fn pulse_stamp_rejects_stale_sql_visibility_after_commit_wait_without_changing_legacy_wire() {
    use catalyst::comms::archipelago::control_v4::{AssignmentPayload, LaneKey};
    use catalyst::comms::gatekeeper::admission::cluster::fence::PulseAdmissionStamp;
    let fixture = Fixture::new()
        .await
        .expect("mandatory disposable PostgreSQL fixture");
    let server = Server::start(&fixture).await;
    let realm = LaneKey::parse("realm").unwrap();
    for change in ["realm", "source", "version", "floor", "missing", "pass"] {
        let wallet = PrivateKeySigner::random();
        let (mut ws, challenge) = legacy_open(&server, &wallet).await;
        legacy_sign(&mut ws, &wallet, &wallet, &challenge).await;
        assert!(matches!(
            receive(&mut ws).await.message,
            Some(pb::server_packet::Message::Welcome(_))
        ));
        let owner = current_owner(&server, &fixture, &wallet).await;
        let stamp = server.stamp(&owner, "island-stamp-current").await;
        // The SQL update is dispatched but cannot become visible until this transaction releases.
        let mut held = fixture.pool.begin().await.unwrap();
        sqlx::query("SELECT 1 FROM social.archipelago_v4_assignments WHERE deployment_audience = $1 AND owner_address = $2 AND lane_key = 'realm' FOR UPDATE")
            .bind("test-realm").bind(&owner.address).fetch_one(&mut *held).await.unwrap();
        let authority = server.state.control_v4.clone();
        let publishing_owner = owner.clone();
        let lane = realm.clone();
        let publishing = tokio::spawn(async move {
            authority
                .publish_assignment(
                    &publishing_owner,
                    &lane,
                    AssignmentPayload {
                        island_id: "island-stamp-current".into(),
                        connection_string: "old-credential".into(),
                        from_island_id: None,
                        peers: Default::default(),
                        pulse_admission: (change != "missing").then_some(stamp),
                        token_expires_at_unix: None,
                    },
                )
                .await
        });
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let waiting: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname = current_database() AND wait_event_type = 'Lock' AND query LIKE '%social.archipelago_v4_assignments%')").fetch_one(&fixture.pool).await.unwrap();
                if waiting { break; } tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }).await.expect("assignment producer paused at SQL visibility boundary");
        {
            let mut plans = server.plans.lock().unwrap();
            let current = plans.get_mut(&owner.address).unwrap();
            match change {
                "realm" => current.realm = "next-realm".into(),
                "source" => current.source_incarnation = "next-source".into(),
                "version" => current.assignment_version += 1,
                "floor" => current.token_not_before = Some(1),
                "pass" => current.pass += 1,
                "missing" => {}
                _ => unreachable!(),
            }
        }
        held.commit().await.unwrap();
        publishing.await.unwrap().unwrap();
        if change == "pass" {
            let Some(pb::server_packet::Message::IslandChanged(packet)) =
                receive(&mut ws).await.message
            else {
                panic!("unchanged identity across a fresh pass must deliver")
            };
            assert_eq!(packet.conn_str, "old-credential");
            assert_eq!(
                server
                    .state
                    .enforcement
                    .island_of(&owner.address)
                    .as_deref(),
                Some("island-stamp-current"),
                "fenced assignment records moderation room before any heartbeat"
            );
            assert!(packet.assignment_context.is_none());
            server.state.enforcement.kick_peer(&owner.address, "banned");
            assert!(matches!(
                receive(&mut ws).await.message,
                Some(pb::server_packet::Message::Kicked(_))
            ));
            assert!(
                server.state.enforcement.island_of(&owner.address).is_none(),
                "moderation retires a configured pre-heartbeat assignment"
            );
        } else {
            assert!(
                tokio::time::timeout(Duration::from_millis(2400), ws.next())
                    .await
                    .is_err(),
                "{change}: stale row crossed final credential boundary"
            );
            let current = server
                .plans
                .lock()
                .unwrap()
                .get(&owner.address)
                .unwrap()
                .clone();
            let safety = catalyst::comms::gatekeeper::admission::cluster::safety::SafetyStore::new(
                fixture.pool.clone(),
            )
            .snapshot(&owner.address)
            .await
            .unwrap();
            let mut fresh_stamp =
                PulseAdmissionStamp::from_snapshot(&current, current.token_not_before.unwrap())
                    .unwrap();
            fresh_stamp.safety_generation = safety.generation;
            fresh_stamp.safety_token_floor = safety.token_floor;
            server
                .state
                .control_v4
                .publish_assignment(
                    &owner,
                    &realm,
                    AssignmentPayload {
                        island_id: "island-stamp-current".into(),
                        connection_string: "fresh-credential".into(),
                        from_island_id: None,
                        peers: Default::default(),
                        pulse_admission: Some(fresh_stamp),
                        token_expires_at_unix: None,
                    },
                )
                .await
                .unwrap();
            let Some(pb::server_packet::Message::IslandChanged(packet)) =
                receive(&mut ws).await.message
            else {
                panic!("fresh stamped row must deliver")
            };
            assert_eq!(packet.conn_str, "fresh-credential");
            assert!(packet.assignment_context.is_none());
        }
        drop(ws);
    }
    drop(server);
    fixture.finish().await;
}

#[tokio::test]
async fn contract_authentication_preserves_configured_owner_fences() {
    struct Valid;
    #[async_trait::async_trait]
    impl catalyst::auth::Eip1654Validator for Valid {
        fn fresh_validator(&self) -> Option<&dyn catalyst::auth::Eip1654Validator> {
            Some(self)
        }
        async fn validate_signature(
            &self,
            _: &str,
            _: &[u8],
            _: &[u8],
        ) -> Result<bool, catalyst::auth::AuthError> {
            Ok(true)
        }
    }
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    let server =
        Server::with_validator(fixture.pool.clone(), true, None, Some(Arc::new(Valid))).await;
    let wallet = PrivateKeySigner::random();
    let (mut delayed, old_challenge) = legacy_open(&server, &wallet).await;
    let (mut current, new_challenge) = legacy_open(&server, &wallet).await;
    let signed = |challenge: &str| {
        packet(0, pb::client_packet::Message::SignedChallenge(pb::SignedChallengeMessage {
        auth_chain_json: serde_json::json!([
            {"type":"SIGNER", "payload":format!("{:#x}", wallet.address()), "signature":""},
            {"type":"ECDSA_EIP_1654_SIGNED_ENTITY", "payload":challenge, "signature":"0x1234"}
        ]).to_string()
    }))
    };
    send(&mut current, &signed(&new_challenge)).await;
    assert!(matches!(
        receive(&mut current).await.message,
        Some(pb::server_packet::Message::Welcome(_))
    ));
    let before = current_owner(&server, &fixture, &wallet).await;
    send(&mut delayed, &signed(&old_challenge)).await;
    assert!(matches!(
        receive(&mut delayed).await.message,
        Some(pb::server_packet::Message::Kicked(_))
    ));
    let after = current_owner(&server, &fixture, &wallet).await;
    assert_eq!((before.session, before.epoch), (after.session, after.epoch));
    catalyst::comms::archipelago::shutdown(&server.state).await;
    drop((current, delayed, server));
    fixture.finish().await;
}

#[tokio::test]
async fn historical_contract_authentication_keeps_verified_principal_and_configured_fences() {
    struct Historical {
        starts: std::sync::Mutex<Vec<i64>>,
    }
    #[async_trait::async_trait]
    impl catalyst::auth::Eip1654Validator for Historical {
        fn fresh_validator(&self) -> Option<&dyn catalyst::auth::Eip1654Validator> {
            Some(self)
        }
        async fn validate_signature(
            &self,
            _: &str,
            _: &[u8],
            _: &[u8],
        ) -> Result<bool, catalyst::auth::AuthError> {
            Ok(false)
        }
        async fn verification_time_block(
            &self,
            started: i64,
        ) -> Result<u64, catalyst::auth::AuthError> {
            self.starts.lock().unwrap().push(started);
            Ok(4)
        }
        async fn validate_signature_at_block(
            &self,
            _: &str,
            _: &[u8],
            _: &[u8],
            block: u64,
        ) -> Result<bool, catalyst::auth::AuthError> {
            assert_eq!(block, 4);
            Ok(true)
        }
    }
    let fixture = Fixture::new()
        .await
        .expect("mandatory disposable PostgreSQL fixture");
    let validator = Arc::new(Historical {
        starts: std::sync::Mutex::new(Vec::new()),
    });
    let server = Server::with_validator(
        fixture.pool.clone(),
        true,
        None,
        Some(Arc::new(catalyst::auth::ValidationCache::new(
            validator.clone(),
        ))),
    )
    .await;
    let claim = PrivateKeySigner::random();
    let principal = PrivateKeySigner::random();
    let address = format!("{:#x}", principal.address());
    let (mut old, old_nonce) = legacy_open(&server, &claim).await;
    let (mut current, new_nonce) = legacy_open(&server, &claim).await;
    let signed = |nonce: &str| {
        packet(
            0,
            pb::client_packet::Message::SignedChallenge(pb::SignedChallengeMessage {
                auth_chain_json: serde_json::json!([
                    {"type":"SIGNER","payload":address,"signature":""},
                    {"type":"ECDSA_EIP_1654_SIGNED_ENTITY","payload":nonce,"signature":"0x1234"}
                ])
                .to_string(),
            }),
        )
    };
    send(&mut current, &signed(&new_nonce)).await;
    let Some(pb::server_packet::Message::Welcome(welcome)) = receive(&mut current).await.message
    else {
        panic!("historical signature welcome expected")
    };
    assert_eq!(welcome.peer_id, address);
    let before = current_owner(&server, &fixture, &principal).await;
    assert_eq!(before.session, address);
    send(&mut old, &signed(&old_nonce)).await;
    assert!(matches!(
        receive(&mut old).await.message,
        Some(pb::server_packet::Message::Kicked(_))
    ));
    let after = current_owner(&server, &fixture, &principal).await;
    assert_eq!((before.session, before.epoch), (after.session, after.epoch));
    let claimed_rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM social.archipelago_v4_assignments WHERE owner_address = $1",
    )
    .bind(format!("{:#x}", claim.address()))
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert_eq!(claimed_rows, 0);
    assert!(
        !server
            .state
            .registry
            .has_any_peer(&format!("{:#x}", claim.address()))
    );
    assert_eq!(validator.starts.lock().unwrap().len(), 2);
    catalyst::comms::archipelago::shutdown(&server.state).await;
    drop((current, old, server));
    fixture.finish().await;
}

#[tokio::test]
async fn configured_verified_principal_owns_sql_and_preserves_exact_release() {
    struct Valid;
    #[async_trait::async_trait]
    impl catalyst::auth::Eip1654Validator for Valid {
        fn fresh_validator(&self) -> Option<&dyn catalyst::auth::Eip1654Validator> {
            Some(self)
        }
        async fn validate_signature(
            &self,
            _: &str,
            _: &[u8],
            _: &[u8],
        ) -> Result<bool, catalyst::auth::AuthError> {
            Ok(true)
        }
    }
    let fixture = Fixture::new()
        .await
        .expect("mandatory disposable PostgreSQL fixture");
    let server =
        Server::with_validator(fixture.pool.clone(), true, None, Some(Arc::new(Valid))).await;
    for contract in [false, true] {
        for delegated in [false, true] {
            let claim = PrivateKeySigner::random();
            let wallet = PrivateKeySigner::random();
            let device = if delegated {
                PrivateKeySigner::random()
            } else {
                wallet.clone()
            };
            let principal = format!("{:#x}", wallet.address());
            let session = format!("{:#x}", device.address());
            let claim_address = format!("{:#x}", claim.address());
            let mut sockets = Vec::new();
            let mut owners = Vec::new();
            for _ in 0..2 {
                let (mut socket, nonce) = legacy_open(&server, &claim).await;
                if contract {
                    let mut links = vec![
                        serde_json::json!({"type":"SIGNER", "payload":principal, "signature":""}),
                    ];
                    if delegated {
                        links.push(serde_json::json!({"type":"ECDSA_EIP_1654_EPHEMERAL", "payload":format!("Decentraland Login\nEphemeral address: {session}\nExpiration: 2099-12-31T00:00:00.000Z"), "signature":"0x1234"}));
                    }
                    links.push(if delegated {
                        serde_json::json!({"type":"ECDSA_SIGNED_ENTITY", "payload":nonce, "signature":device.sign_message_sync(nonce.as_bytes()).unwrap().to_string()})
                    } else {
                        serde_json::json!({"type":"ECDSA_EIP_1654_SIGNED_ENTITY", "payload":nonce, "signature":"0x1234"})
                    });
                    send(
                        &mut socket,
                        &packet(
                            0,
                            pb::client_packet::Message::SignedChallenge(
                                pb::SignedChallengeMessage {
                                    auth_chain_json: serde_json::to_string(&links).unwrap(),
                                },
                            ),
                        ),
                    )
                    .await;
                } else {
                    legacy_sign(&mut socket, &wallet, &device, &nonce).await;
                }
                let Some(pb::server_packet::Message::Welcome(welcome)) =
                    receive(&mut socket).await.message
                else {
                    panic!("verified principal welcome expected")
                };
                assert_eq!(welcome.peer_id, principal);
                assert!(!server.state.registry.has_any_peer(&claim_address));
                assert!(server.state.registry.has_any_peer(&principal));
                let owner = current_owner(&server, &fixture, &wallet).await;
                assert_eq!(owner.session, session);
                let claimed_rows: i64 = sqlx::query_scalar("SELECT count(*) FROM social.archipelago_v4_assignments WHERE owner_address = $1").bind(&claim_address).fetch_one(&fixture.pool).await.unwrap();
                assert_eq!(claimed_rows, 0);
                send(
                    &mut socket,
                    &packet(
                        0,
                        pb::client_packet::Message::Heartbeat(pb::Heartbeat {
                            position: Some(common::Position {
                                x: 1.,
                                y: 2.,
                                z: 3.,
                            }),
                            desired_room: None,
                            sample_sequence: None,
                        }),
                    ),
                )
                .await;
                legacy_barrier(&mut socket, 8).await;
                drained(&server).await;
                assert!(server.bus.published().iter().any(|(subject, payload)| subject == catalyst::comms::protocol::bus::control_position::SUBJECT && catalyst::comms::protocol::bus::control_position::ControlPosition::decode(payload).is_some_and(|position| position.wallet == principal && position.session == session && position.epoch == owner.epoch && position.position == Some([1., 2., 3.]))));
                owners.push(owner);
                sockets.push(socket);
            }
            assert!(owners[1].epoch > owners[0].epoch);
            let _ = sockets[0].close(None).await;
            tokio::time::timeout(Duration::from_secs(3), async {
                while server.state.handlers.available_permits()
                    != server.state.settings.server.max_sockets as usize - 1
                {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
            let still_current = current_owner(&server, &fixture, &wallet).await;
            assert_eq!(still_current.epoch, owners[1].epoch);
            assert_eq!(still_current.session, session);
            assert!(
                server
                    .state
                    .control_v4
                    .snapshot(
                        &owners[1],
                        &catalyst::comms::archipelago::control_v4::LaneKey::parse("realm").unwrap()
                    )
                    .await
                    .is_ok(),
                "old cleanup cannot expire the current owner"
            );
            assert!(server.state.registry.has_any_peer(&principal));
            sockets[1].close(None).await.unwrap();
            tokio::time::timeout(Duration::from_secs(3), async {
                while server.state.registry.has_any_peer(&principal) {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
            assert!(
                !server
                    .bus
                    .published()
                    .iter()
                    .any(|(subject, payload)| subject.contains(&claim_address)
                        || String::from_utf8_lossy(payload).contains(&claim_address))
            );
            assert!(
                !server
                    .bus
                    .requested()
                    .iter()
                    .any(|(subject, payload)| subject.contains(&claim_address)
                        || String::from_utf8_lossy(payload).contains(&claim_address))
            );
        }
    }
    catalyst::comms::archipelago::shutdown(&server.state).await;
    drop(server);
    fixture.finish().await;
}

#[tokio::test]
async fn configured_claim_and_principal_policy_denials_never_claim_sql() {
    let fixture = Fixture::new()
        .await
        .expect("mandatory disposable PostgreSQL fixture");
    for denied in ["claim", "principal", "ban"] {
        let claim = PrivateKeySigner::random();
        let wallet = PrivateKeySigner::random();
        let address_a = format!("{:#x}", claim.address());
        let address_b = format!("{:#x}", wallet.address());
        let denied_wallet = if denied == "claim" {
            address_a.clone()
        } else if denied == "principal" {
            address_b.clone()
        } else {
            String::new()
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/deny", listener.local_addr().unwrap());
        let app = axum::Router::new().route(
            "/deny",
            axum::routing::get(move || {
                let address = denied_wallet.clone();
                async move { axum::Json(serde_json::json!({"users":[{"wallet":address}]})) }
            }),
        );
        let deny_task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let bans =
            catalyst::comms::gatekeeper::moderation::test_support::StubBanStatus(Ok((denied
                == "ban")
                .then_some(catalyst::comms::access::Ban::Platform)));
        let server = Server::with_policy(
            fixture.pool.clone(),
            true,
            None,
            None,
            Some(url),
            Some(Arc::new(bans)),
        )
        .await;
        let mut socket = if denied == "claim" {
            let mut socket = server.connect().await;
            send(
                &mut socket,
                &packet(
                    0,
                    pb::client_packet::Message::ChallengeRequest(pb::ChallengeRequestMessage {
                        address: address_a.clone(),
                    }),
                ),
            )
            .await;
            socket
        } else {
            let (mut socket, nonce) = legacy_open(&server, &claim).await;
            legacy_sign(&mut socket, &wallet, &wallet, &nonce).await;
            socket
        };
        if denied == "ban" {
            assert!(matches!(
                receive(&mut socket).await.message,
                Some(pb::server_packet::Message::Kicked(_))
            ));
        }
        tokio::time::timeout(Duration::from_secs(3), async {
            while let Some(Ok(message)) = socket.next().await {
                assert!(
                    !matches!(message, Message::Binary(_)),
                    "policy denial must never welcome or assign"
                );
                if matches!(message, Message::Close(_)) {
                    break;
                }
            }
        })
        .await
        .unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            while server.state.handlers.available_permits()
                != server.state.settings.server.max_sockets as usize
            {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        let claims: i64 = sqlx::query_scalar("SELECT count(*) FROM social.archipelago_v4_assignments WHERE owner_address = $1 OR owner_address = $2").bind(&address_a).bind(&address_b).fetch_one(&fixture.pool).await.unwrap();
        assert_eq!(claims, 0);
        assert_eq!(server.state.registry.authenticated_count(), 0);
        assert_eq!(server.state.challenges.pending(), 0);
        assert!(server.bus.published().is_empty());
        assert!(server.bus.requested().is_empty());
        catalyst::comms::archipelago::shutdown(&server.state).await;
        drop(server);
        deny_task.abort();
    }
    fixture.finish().await;
}

#[tokio::test]
async fn configured_distinct_sessions_share_the_realm_owner_row() {
    let fixture = Fixture::new()
        .await
        .expect("mandatory disposable PostgreSQL fixture");
    let server = Server::start(&fixture).await;
    let wallet = PrivateKeySigner::random();
    let devices = [PrivateKeySigner::random(), PrivateKeySigner::random()];
    let (mut first, nonce) = legacy_open(&server, &wallet).await;
    legacy_sign(&mut first, &wallet, &devices[0], &nonce).await;
    assert!(matches!(
        receive(&mut first).await.message,
        Some(pb::server_packet::Message::Welcome(_))
    ));
    let original = current_owner(&server, &fixture, &wallet).await;
    let (mut second, nonce) = legacy_open(&server, &wallet).await;
    legacy_sign(&mut second, &wallet, &devices[1], &nonce).await;
    assert!(matches!(
        receive(&mut second).await.message,
        Some(pb::server_packet::Message::Welcome(_))
    ));
    let current = current_owner(&server, &fixture, &wallet).await;
    assert!(current.epoch > original.epoch);
    assert_ne!(current.session, original.session);
    assert_eq!(current.session, format!("{:#x}", devices[1].address()));
    assert!(
        matches!(
            receive(&mut first).await.message,
            Some(pb::server_packet::Message::Kicked(_))
        ),
        "configured signaling sessions contend for the wallet-wide realm owner"
    );
    let _ = first.close(None).await;
    let after = current_owner(&server, &fixture, &wallet).await;
    assert_eq!(after.epoch, current.epoch);
    assert_eq!(after.session, current.session);
    second.close(None).await.unwrap();
    catalyst::comms::archipelago::shutdown(&server.state).await;
    drop(server);
    fixture.finish().await;
}

#[tokio::test]
async fn configured_close_during_claim_can_commit_before_receive() {
    use tokio::io::AsyncReadExt;
    let fixture = Fixture::new()
        .await
        .expect("mandatory disposable PostgreSQL fixture");
    let server = Server::start(&fixture).await;
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    let (mut socket, nonce) = legacy_open(&server, &wallet).await;
    sqlx::query("INSERT INTO social.island_admission_fences(wallet) VALUES ($1)")
        .bind(&address)
        .execute(&fixture.pool)
        .await
        .unwrap();
    let mut held = fixture.pool.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM social.island_admission_fences WHERE wallet = $1 FOR UPDATE")
        .bind(&address)
        .fetch_one(&mut *held)
        .await
        .unwrap();
    legacy_sign(&mut socket, &wallet, &wallet, &nonce).await;
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let waiting: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname = current_database() AND wait_event_type = 'Lock' AND query LIKE '%social.island_admission_fences%')").fetch_one(&fixture.pool).await.unwrap();
            if waiting { break; }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await.expect("claim reached the actual SQL wallet lock");
    assert!(!server.state.registry.has_any_peer(&address));
    socket.close(None).await.unwrap();
    // SQL claim completion precedes consumption of pending transport input.
    held.commit().await.unwrap();
    let tokio_tungstenite::MaybeTlsStream::Plain(tcp) = socket.get_mut() else {
        panic!("loopback plaintext expected")
    };
    let mut bytes = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), tcp.read_to_end(&mut bytes))
        .await
        .unwrap()
        .unwrap();
    eprintln!("CONFIGURED_CLAIM_CLOSE bytes={bytes:02x?}");
    assert!(
        !bytes.is_empty(),
        "transport eventually completes its terminal handling"
    );
    tokio::time::timeout(Duration::from_secs(3), async {
        while server.state.handlers.available_permits()
            != server.state.settings.server.max_sockets as usize
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(!server.state.registry.has_any_peer(&address));
    let committed: i64 = sqlx::query_scalar("SELECT count(*) FROM social.archipelago_v4_assignments WHERE owner_address = $1 AND lane_key = 'realm'").bind(&address).fetch_one(&fixture.pool).await.unwrap();
    assert_eq!(
        committed, 1,
        "Close does not cancel an already dispatched SQL claim"
    );
    catalyst::comms::archipelago::shutdown(&server.state).await;
    drop(server);
    fixture.finish().await;
}
