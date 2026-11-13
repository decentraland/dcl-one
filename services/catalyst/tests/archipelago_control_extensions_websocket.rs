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
    fence: Arc<catalyst::comms::gatekeeper::admission::cluster::fence::PgAssignmentFence>,
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
        let fence = Arc::new(
            catalyst::comms::gatekeeper::admission::cluster::fence::PgAssignmentFence::new(
                state.gatekeeper.pool.clone(),
                "test-realm".into(),
            )
            .unwrap(),
        );
        let subscriber = catalyst::comms::gatekeeper::admission::cluster::subscriber::ClusterSubscriber::new_with_assignment_fence(bus.clone(), Arc::new(catalyst::comms::gatekeeper::admission::cluster::gateway::StateGateway::new(state.gatekeeper.clone())), Arc::new(catalyst::comms::gatekeeper::admission::cluster::peer_state::ClusterPeerState::default()), state.gatekeeper.cluster.clone(), Some(fence.clone()));
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
            fence,
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
            selected_identity: plans
                .get(&owner.address)
                .filter(|p| p.session == owner.session)
                .and_then(|p| p.selected_identity)
                .or(Some(1)),
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

    // These transport/SQL fixtures select a native registration explicitly. The real
    // broker test below covers legacy producer, Pulse election and selected acquisition.
    async fn select_native_owner(
        &self,
        wallet: &PrivateKeySigner,
        session: &str,
        room: &str,
    ) -> catalyst::comms::archipelago::control_v4::V4Owner {
        use catalyst::comms::gatekeeper::admission::cluster::fence::AssignmentFence;
        let address = format!("{:#x}", wallet.address());
        let previous = self.plans.lock().unwrap().get(&address).cloned();
        let owner = catalyst::comms::archipelago::control_v4::V4Owner {
            address: address.clone(),
            session: session.into(),
            epoch: 0,
        };
        self.stamp(&owner, room).await;
        let snapshot = {
            let mut plans = self.plans.lock().unwrap();
            let selected = plans.get_mut(&address).unwrap();
            selected.selected_identity = Some(
                previous
                    .as_ref()
                    .and_then(|p| p.selected_identity)
                    .unwrap_or(0)
                    + 1,
            );
            selected.assignment_version = previous.as_ref().map_or(1, |p| p.assignment_version + 1);
            selected.clone()
        };
        let lease = self
            .fence
            .lock_selected(&snapshot)
            .await
            .expect("selected coordinator claim");
        catalyst::comms::archipelago::control_v4::V4Owner {
            epoch: lease.token_fence().owner_epoch,
            ..owner
        }
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
async fn fenced_completed_assignment_peer_close_echoes_and_preserves_selected_native_owner() {
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
    let owner = server
        .select_native_owner(&wallet, &format!("{:#x}", wallet.address()), "island-close")
        .await;
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
            .is_ok()
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
    let binary = gate::require_env("PULSE_NATS_SERVER_BIN").expect("mandatory NATS fixture");
    let fixture = Fixture::new()
        .await
        .expect("mandatory disposable PostgreSQL fixture");
    let broker = nats_broker::Broker::start(&binary, None);
    let feed = Arc::new(NatsClusterFeed::spawn(NatsFeedOptions {
        url: broker.url(),
        discovery_interval_ms: 0,
        ..Default::default()
    }));
    feed.enable_control_positions("test-realm").unwrap();
    let bus = Arc::new(BrokerBus::new(Some(broker.url()), "catalyst-comms"));
    use axum::{Json, Router, http::StatusCode, routing::post};
    let media = Router::new()
        .route(
            "/twirp/livekit.RoomService/GetParticipant",
            post(|| async {
                (
                    StatusCode::NOT_FOUND,
                    Json(
                        serde_json::json!({"code":"not_found","msg":"participant does not exist"}),
                    ),
                )
            }),
        )
        .route(
            "/twirp/livekit.RoomService/RemoveParticipant",
            post(|| async { Json(serde_json::json!({})) }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let media_url = format!("http://{}", listener.local_addr().unwrap());
    let media_task = tokio::spawn(async move { axum::serve(listener, media).await.unwrap() });
    let mut gatekeeper = Arc::try_unwrap(archipelago::gatekeeper_with_livekit(
        None,
        archipelago::configured_livekit(&media_url),
    ))
    .ok()
    .unwrap();
    gatekeeper.pool = fixture.pool.clone();
    let gatekeeper = Arc::new(
        gatekeeper
            .with_cluster(
                catalyst::comms::gatekeeper::admission::cluster::ClusterConfig {
                    control_v4_audience: Some("test-realm".into()),
                    nats_url: Some(broker.url()),
                    self_hosted_token_quarantine: false,
                    ..Default::default()
                },
            )
            .unwrap()
            .with_bus(bus),
    );
    let jobs = catalyst::jobs::Jobs::new();
    let state = catalyst::comms::archipelago::build_state(Settings::for_tests(), gatekeeper, &jobs)
        .await
        .unwrap();
    archipelago::start_island_feed(&state, &jobs);
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
    let mut grids = RealmSpatialGrids::new(SPATIAL_GRID_CELL_SIZE, 8);
    let mut board = SnapshotBoard::new(8, 4);
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
    assert_eq!(
        snapshot.selected_origin, 3,
        "the ordinary producer sends v2 intents"
    );
    let Some(pb::server_packet::Message::IslandChanged(assignment)) =
        receive(&mut socket).await.message
    else {
        panic!("selected legacy-only credentials expected")
    };
    assert!(assignment.conn_str.starts_with("livekit:"));
    let initial: (String, i64, serde_json::Value) = sqlx::query_as("SELECT owner_session,owner_epoch,assignment_json FROM social.archipelago_v4_assignments WHERE deployment_audience='test-realm' AND owner_address=$1 AND lane_key='realm'")
        .bind(&address).fetch_one(&fixture.pool).await.unwrap();
    assert_eq!(initial.0, address);
    assert_eq!(
        initial.2["pulseAdmission"]["selectedOrigin"],
        "legacyIntentV2"
    );
    assert_eq!(
        initial.2["pulseAdmission"]["selectedIdentity"].as_u64(),
        snapshot.selected_identity
    );
    let device = PrivateKeySigner::random();
    let device_session = format!("{:#x}", device.address());
    let (mut other, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws"))
        .await
        .unwrap();
    send(
        &mut other,
        &packet(
            0,
            pb::client_packet::Message::ChallengeRequest(pb::ChallengeRequestMessage {
                address: address.clone(),
            }),
        ),
    )
    .await;
    let Some(pb::server_packet::Message::ChallengeResponse(challenge)) =
        receive(&mut other).await.message
    else {
        panic!("challenge")
    };
    legacy_sign(&mut other, &wallet, &device, &challenge.challenge_to_sign).await;
    assert!(matches!(
        receive(&mut other).await.message,
        Some(pb::server_packet::Message::Welcome(_))
    ));
    let unchanged: i64 = sqlx::query_scalar("SELECT owner_epoch FROM social.archipelago_v4_assignments WHERE deployment_audience='test-realm' AND owner_address=$1 AND lane_key='realm'")
        .bind(&address).fetch_one(&fixture.pool).await.unwrap();
    assert_eq!(
        unchanged, initial.1,
        "Welcome alone changes no room authority"
    );
    legacy_barrier(&mut socket, 17).await;
    // A second real producer wins by completion, while the first signaling link remains live.
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            send(&mut other, &heartbeat).await;
            tracker.run_pass(&grids, &board, &identity);
            if let Ok(Some(Ok(Message::Binary(bytes)))) =
                tokio::time::timeout(Duration::from_millis(100), other.next()).await
            {
                if matches!(
                    pb::ServerPacket::decode(bytes).unwrap().message,
                    Some(pb::server_packet::Message::IslandChanged(_))
                ) {
                    break;
                }
            }
        }
    })
    .await
    .expect("second selected session receives actual committed credentials");
    let legacy_epoch: i64 = sqlx::query_scalar("SELECT owner_epoch FROM social.archipelago_v4_assignments WHERE deployment_audience='test-realm' AND owner_address=$1 AND lane_key='realm' AND owner_session=$2")
        .bind(&address).bind(&device_session).fetch_one(&fixture.pool).await.unwrap();
    assert!(legacy_epoch > initial.1);
    legacy_barrier(&mut socket, 18).await;
    // A native registration has no /ws, yet must acquire the known selected SQL predecessor.
    let native = format!("{:#x}", PrivateKeySigner::random().address());
    let position = catalyst::comms::pulse::decentraland::common::Vector3 {
        x: 1.0,
        y: 2.0,
        z: 3.0,
    };
    board.set_active(0);
    board.publish(
        0,
        catalyst::comms::pulse::snapshot::PeerSnapshot {
            global_position: position,
            realm: Some("shared-realm".into()),
            ..Default::default()
        },
    );
    identity.set_with_session(0, address.clone(), native.clone());
    grids.set(0, "shared-realm", position);
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            tracker.run_pass(&grids, &board, &identity);
            let owned: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM social.archipelago_v4_assignments WHERE deployment_audience='test-realm' AND owner_address=$1 AND lane_key='realm' AND owner_session=$2 AND assignment_json->'pulseAdmission'->>'selectedOrigin'='native')")
                .bind(&address).bind(&native).fetch_one(&fixture.pool).await.unwrap();
            if owned { break; }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }).await.expect("native selected SQL publication without a native signaling socket");
    legacy_barrier(&mut socket, 19).await;
    legacy_barrier(&mut other, 20).await;
    identity.remove(0);
    grids.remove(0);
    board.clear_active(0);
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            send(&mut other, &heartbeat).await;
            tracker.run_pass(&grids, &board, &identity);
            if let Ok(Some(Ok(Message::Binary(bytes)))) =
                tokio::time::timeout(Duration::from_millis(100), other.next()).await
            {
                if matches!(
                    pb::ServerPacket::decode(bytes).unwrap().message,
                    Some(pb::server_packet::Message::IslandChanged(_))
                ) {
                    break;
                }
            }
        }
    })
    .await
    .expect("native departure requires fresh selected legacy publication and credentials");
    let restored: String = sqlx::query_scalar("SELECT owner_session FROM social.archipelago_v4_assignments WHERE deployment_audience='test-realm' AND owner_address=$1 AND lane_key='realm'")
        .bind(&address).fetch_one(&fixture.pool).await.unwrap();
    assert_eq!(restored, device_session);
    legacy_barrier(&mut socket, 21).await;
    send(&mut socket, &heartbeat).await;
    legacy_barrier(&mut socket, 22).await;
    other.close(None).await.unwrap();
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            send(&mut socket, &heartbeat).await;
            tracker.run_pass(&grids, &board, &identity);
            if let Ok(Some(Ok(Message::Binary(bytes)))) =
                tokio::time::timeout(Duration::from_millis(100), socket.next()).await
            {
                if matches!(
                    pb::ServerPacket::decode(bytes).unwrap().message,
                    Some(pb::server_packet::Message::IslandChanged(_))
                ) {
                    break;
                }
            }
        }
    })
    .await
    .expect("closing the winner promotes a still-live legacy signaling transport");
    let promoted:serde_json::Value=sqlx::query_scalar("SELECT assignment_json FROM social.archipelago_v4_assignments WHERE deployment_audience='test-realm' AND owner_address=$1 AND lane_key='realm' AND owner_session=$2")
        .bind(&address).bind(&address).fetch_one(&fixture.pool).await.unwrap();
    assert!(
        promoted["pulseAdmission"]["selectedIdentity"]
            .as_u64()
            .unwrap()
            > snapshot.selected_identity.unwrap(),
        "promotion allocates a new candidate epoch"
    );
    socket.close(None).await.unwrap();
    catalyst::comms::archipelago::shutdown(&state).await;
    media_task.abort();
    drop((socket, other, state, tracker, feed, client, jobs));
    fixture.finish().await;
}

#[tokio::test]
async fn control_heartbeats_publish_completion_ranked_v2_and_epoch_scoped_closes() {
    use catalyst::comms::protocol::bus::control_position::LegacyRoomIntent;
    let fixture = Fixture::new()
        .await
        .expect("required disposable PostgreSQL");
    let server = Server::start(&fixture).await;
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    let mut first = server.connect().await;
    admit_legacy(&mut first, &wallet).await;
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
            .iter()
            .filter(|(subject, _)| subject == LegacyRoomIntent::SUBJECT)
            .map(|(_, payload)| LegacyRoomIntent::decode(payload).unwrap())
            .collect::<Vec<_>>()
    };
    send(&mut first, &heartbeat).await;
    tokio::time::timeout(Duration::from_secs(5), async {
        while records().is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let old = records()[0].clone();
    assert_eq!(old.wallet, address);
    assert_eq!(old.session, address);
    assert_eq!(old.realm, "shared-realm");
    let mut next = server.connect().await;
    admit_legacy(&mut next, &wallet).await;
    send(&mut next, &heartbeat).await;
    tokio::time::timeout(Duration::from_secs(5), async {
        while !records()
            .iter()
            .any(|record| record.candidate_epoch > old.candidate_epoch && record.position.is_some())
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let current = records()
        .into_iter()
        .find(|record| record.candidate_epoch > old.candidate_epoch && record.position.is_some())
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !records().iter().any(|record| {
            record.candidate_epoch == old.candidate_epoch && record.position.is_none()
        }) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    drop(first);
    legacy_barrier(&mut next, 10).await;
    let realm_owner = server
        .select_native_owner(&wallet, &address, "island-scene-independent")
        .await;
    let scene_owner = catalyst::comms::archipelago::control_v4::V4Owner {
        address: address.clone(),
        session: address.clone(),
        epoch: server
            .state
            .control_v4
            .next_connection_epoch()
            .await
            .unwrap(),
    };
    let scene = catalyst::comms::archipelago::control_v4::LaneKey::parse("scene:plaza").unwrap();
    server
        .state
        .control_v4
        .claim_lanes(&scene_owner, &[scene])
        .await
        .unwrap();
    server.state.control_v4.release(&scene_owner).await.unwrap();
    assert!(
        server
            .state
            .control_v4
            .snapshot(
                &realm_owner,
                &catalyst::comms::archipelago::control_v4::LaneKey::parse("realm").unwrap()
            )
            .await
            .is_ok()
    );
    assert!(
        !records()
            .iter()
            .any(|r| r.candidate_epoch == current.candidate_epoch && r.position.is_none())
    );
    next.close(None).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !records()
            .iter()
            .any(|r| r.candidate_epoch == current.candidate_epoch && r.position.is_none())
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(
        !server.bus.published().iter().any(
            |(subject, _)| subject == catalyst::comms::protocol::bus::control_position::SUBJECT
        )
    );
    for epoch in [old.candidate_epoch, current.candidate_epoch] {
        let values: Vec<_> = records()
            .into_iter()
            .filter(|r| r.candidate_epoch == epoch)
            .collect();
        assert!(
            values
                .windows(2)
                .all(|pair| pair[1].sequence > pair[0].sequence)
        );
    }
    drop(server);
    fixture.finish().await;
}

#[tokio::test]
async fn legacy_heartbeat_authority_deadline_preserves_signaling_without_publishing_an_unverified_intent()
 {
    use catalyst::comms::protocol::bus::control_position::LegacyRoomIntent;
    let fixture = Fixture::new()
        .await
        .expect("required disposable PostgreSQL");
    let server = Server::start(&fixture).await;
    let wallet = PrivateKeySigner::random();
    let mut socket = server.connect().await;
    admit_legacy(&mut socket, &wallet).await;
    let mut held = Vec::new();
    for _ in 0..4 {
        held.push(fixture.pool.acquire().await.unwrap());
    }
    send(
        &mut socket,
        &packet(
            0,
            pb::client_packet::Message::Heartbeat(pb::Heartbeat {
                position: Some(common::Position {
                    x: 99.,
                    y: 0.,
                    z: 99.,
                }),
                desired_room: None,
                sample_sequence: None,
            }),
        ),
    )
    .await;
    legacy_barrier(&mut socket, 4).await;
    tokio::time::sleep(Duration::from_millis(2200)).await;
    legacy_barrier(&mut socket, 5).await;
    assert!(
        !server
            .bus
            .published()
            .iter()
            .any(|(subject, _)| subject == LegacyRoomIntent::SUBJECT)
    );
    assert!(
        server.bus.published().iter().any(|(subject, _)| subject
            == catalyst::comms::protocol::bus::statistics::OBSERVATIONS_SUBJECT)
    );
    drop(held);
    socket.close(None).await.unwrap();
    drop((socket, server));
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
        let old = server
            .select_native_owner(
                &wallet,
                &format!("{:#x}", wallet.address()),
                "island-same-room",
            )
            .await;
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
        let before_selection = current_owner(&server, &fixture, &wallet).await;
        assert_eq!(
            (before_selection.session, before_selection.epoch),
            (old.session.clone(), old.epoch),
            "Welcome does not acquire the room"
        );
        let next = server
            .select_native_owner(
                &wallet,
                &format!("{:#x}", signer.address()),
                "island-same-room",
            )
            .await;
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
                .is_some(),
            "configured signaling uses the same exact-session registry"
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
async fn later_configured_authentication_completion_replaces_the_same_session_without_sql() {
    let fixture = Fixture::new()
        .await
        .expect("required disposable PostgreSQL");
    let server = Server::start(&fixture).await;
    let wallet = PrivateKeySigner::random();
    let (mut delayed, nonce) = legacy_open(&server, &wallet).await;
    let mut current = server.connect().await;
    admit_legacy(&mut current, &wallet).await;
    let address = format!("{:#x}", wallet.address());
    let before = server
        .state
        .registry
        .peer_link(&address, &address)
        .unwrap()
        .id;
    legacy_sign(&mut delayed, &wallet, &wallet, &nonce).await;
    assert!(matches!(
        receive(&mut delayed).await.message,
        Some(pb::server_packet::Message::Welcome(_))
    ));
    assert!(matches!(
        receive(&mut current).await.message,
        Some(pb::server_packet::Message::Kicked(_))
    ));
    assert!(
        server
            .state
            .registry
            .peer_link(&address, &address)
            .unwrap()
            .id
            > before
    );
    let rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM social.archipelago_v4_assignments WHERE owner_address=$1",
    )
    .bind(&address)
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert_eq!(rows, 0);
    delayed.close(None).await.unwrap();
    drop((delayed, current, server));
    fixture.finish().await;
}

#[tokio::test]
async fn configured_replica_signaling_cannot_steal_a_fresh_foreign_room() {
    use catalyst::comms::archipelago::control_v4::{AssignmentPayload, LaneKey};
    let fixture = Fixture::new()
        .await
        .expect("required disposable PostgreSQL");
    let first_replica = Server::start(&fixture).await;
    let second_replica = Server::start(&fixture).await;
    let wallet = PrivateKeySigner::random();
    let device = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    let session = format!("{:#x}", device.address());
    let mut first = first_replica.connect().await;
    admit_legacy(&mut first, &wallet).await;
    let old = first_replica
        .select_native_owner(&wallet, &address, "island-replica-current-room")
        .await;
    let (mut second, nonce) = legacy_open(&second_replica, &wallet).await;
    legacy_sign(&mut second, &wallet, &device, &nonce).await;
    assert!(matches!(
        receive(&mut second).await.message,
        Some(pb::server_packet::Message::Welcome(_))
    ));
    let current = current_owner(&first_replica, &fixture, &wallet).await;
    assert_eq!(current.epoch, old.epoch);
    assert_eq!(current.session, old.session);
    legacy_barrier(&mut first, 4).await;
    legacy_barrier(&mut second, 5).await;
    // Only the owning coordinator can hand over its exact predecessor after positive selection.
    let next = first_replica
        .select_native_owner(&wallet, &session, "island-replica-current-room")
        .await;
    let selected = first_replica
        .plans
        .lock()
        .unwrap()
        .get(&address)
        .unwrap()
        .clone();
    second_replica
        .plans
        .lock()
        .unwrap()
        .insert(address.clone(), selected);
    first_replica
        .state
        .control_v4
        .publish_assignment(
            &next,
            &LaneKey::parse("realm").unwrap(),
            AssignmentPayload {
                island_id: "island-replica-current-room".into(),
                connection_string: "replica-current-credential".into(),
                from_island_id: None,
                peers: Default::default(),
                pulse_admission: Some(
                    first_replica
                        .stamp(&next, "island-replica-current-room")
                        .await,
                ),
                token_expires_at_unix: None,
            },
        )
        .await
        .unwrap();
    let Some(pb::server_packet::Message::IslandChanged(assignment)) =
        receive(&mut second).await.message
    else {
        panic!("selected replica assignment")
    };
    assert_eq!(assignment.conn_str, "replica-current-credential");
    legacy_barrier(&mut first, 6).await;
    drop(first);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        second_replica
            .state
            .control_v4
            .snapshot(&next, &LaneKey::parse("realm").unwrap())
            .await
            .is_ok()
    );
    second.close(None).await.unwrap();
    drop((second, first_replica, second_replica));
    fixture.finish().await;
}

#[tokio::test]
async fn a_stalled_authority_detaches_room_work_and_keeps_authenticated_signaling() {
    let fixture = Fixture::new()
        .await
        .expect("required disposable PostgreSQL");
    let server = Server::start(&fixture).await;
    let wallet = PrivateKeySigner::random();
    let mut socket = server.connect().await;
    admit_legacy(&mut socket, &wallet).await;
    let mut held = Vec::new();
    for _ in 0..4 {
        held.push(fixture.pool.acquire().await.unwrap());
    }
    tokio::time::sleep(Duration::from_millis(2200)).await;
    legacy_barrier(&mut socket, 6).await;
    drop(held);
    socket.close(None).await.unwrap();
    drop((socket, server));
    fixture.finish().await;
}

async fn legacy_claim_recovers_after_database_stall(statement_timeout_ms: u64) {
    use catalyst::comms::archipelago::control_v4::{AssignmentPayload, LaneKey};
    let fixture = Fixture::new()
        .await
        .expect("required disposable PostgreSQL");
    let authority_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with(fixture.pool.connect_options().as_ref().clone())
        .await
        .unwrap();
    let server = Server::with_pool(authority_pool.clone(), true, Some(1500)).await;
    sqlx::query("SELECT set_config('statement_timeout',$1,false)")
        .bind(format!("{statement_timeout_ms}ms"))
        .execute(&authority_pool)
        .await
        .unwrap();
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    let (mut socket, nonce) = legacy_open(&server, &wallet).await;
    let mut held = fixture.pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE social.archipelago_v4_assignments IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *held)
        .await
        .unwrap();
    legacy_sign(&mut socket, &wallet, &wallet, &nonce).await;
    assert!(matches!(
        receive(&mut socket).await.message,
        Some(pb::server_packet::Message::Welcome(_))
    ));
    legacy_barrier(&mut socket, 7).await;
    held.rollback().await.unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM social.archipelago_v4_assignments WHERE owner_address=$1",
    )
    .bind(&address)
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert_eq!(count, 0, "authentication never dispatches a claim");
    let owner = server
        .select_native_owner(&wallet, &address, "island-recovered-room")
        .await;
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
        receive(&mut socket).await.message
    else {
        panic!("recovered assignment")
    };
    assert_eq!(assignment.conn_str, "recovered-credential");
    socket.close(None).await.unwrap();
    drop((socket, server));
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
async fn configured_authority_failure_keeps_authenticated_signaling_available() {
    let fixture = Fixture::new()
        .await
        .expect("mandatory disposable PostgreSQL fixture");
    let server = Server::with_authority(&fixture, false).await;
    let wallet = PrivateKeySigner::random();
    let mut socket = server.connect().await;
    admit_legacy(&mut socket, &wallet).await;
    assert!(
        server
            .state
            .registry
            .has_any_peer(&format!("{:#x}", wallet.address()))
    );
    socket.send(Message::Ping(vec![7].into())).await.unwrap();
    assert!(matches!(socket.next().await, Some(Ok(Message::Pong(_)))));
    socket.close(None).await.unwrap();
    catalyst::comms::archipelago::shutdown(&server.state).await;
    drop(server);
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
        let owner = server
            .select_native_owner(
                &wallet,
                &format!("{:#x}", wallet.address()),
                "island-stamp-current",
            )
            .await;
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
    let address = format!("{:#x}", wallet.address());
    let before = server
        .state
        .registry
        .peer_link(&address, &address)
        .unwrap()
        .id;
    send(&mut delayed, &signed(&old_challenge)).await;
    assert!(matches!(
        receive(&mut delayed).await.message,
        Some(pb::server_packet::Message::Welcome(_))
    ));
    let after = server
        .state
        .registry
        .peer_link(&address, &address)
        .unwrap()
        .id;
    assert!(after > before);
    assert!(matches!(
        receive(&mut current).await.message,
        Some(pb::server_packet::Message::Kicked(_))
    ));
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
    let before = server
        .state
        .registry
        .peer_link(&address, &address)
        .unwrap()
        .id;
    send(&mut old, &signed(&old_nonce)).await;
    assert!(matches!(
        receive(&mut old).await.message,
        Some(pb::server_packet::Message::Welcome(_))
    ));
    let after = server
        .state
        .registry
        .peer_link(&address, &address)
        .unwrap()
        .id;
    assert!(after > before);
    assert!(matches!(
        receive(&mut current).await.message,
        Some(pb::server_packet::Message::Kicked(_))
    ));
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
                let owner = server
                    .select_native_owner(&wallet, &session, "island-principal")
                    .await;
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
                tokio::time::timeout(Duration::from_secs(5), async { loop {
                    if server.bus.published().iter().any(|(subject,payload)| subject==catalyst::comms::protocol::bus::control_position::LegacyRoomIntent::SUBJECT && catalyst::comms::protocol::bus::control_position::LegacyRoomIntent::decode(payload).is_some_and(|intent| intent.wallet==principal && intent.session==session && intent.position==Some([1.,2.,3.]))) { break; }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }}).await.expect("verified principal v2 intent");
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
async fn configured_distinct_sessions_keep_signaling_without_claiming_a_room() {
    let fixture = Fixture::new()
        .await
        .expect("mandatory disposable PostgreSQL fixture");
    let server = Server::start(&fixture).await;
    let wallet = PrivateKeySigner::random();
    let devices = [PrivateKeySigner::random(), PrivateKeySigner::random()];
    let mut sockets = Vec::new();
    for device in &devices {
        let (mut socket, nonce) = legacy_open(&server, &wallet).await;
        legacy_sign(&mut socket, &wallet, device, &nonce).await;
        assert!(matches!(
            receive(&mut socket).await.message,
            Some(pb::server_packet::Message::Welcome(_))
        ));
        sockets.push(socket);
    }
    let address = format!("{:#x}", wallet.address());
    assert_eq!(server.state.registry.sessions_of(&address).len(), 2);
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM social.archipelago_v4_assignments WHERE owner_address=$1 AND lane_key='realm'")
        .bind(&address).fetch_one(&fixture.pool).await.unwrap();
    assert_eq!(rows, 0, "signaling completion is not room selection");
    for mut socket in sockets {
        socket.send(Message::Ping(vec![8].into())).await.unwrap();
        assert!(matches!(socket.next().await, Some(Ok(Message::Pong(_)))));
        socket.close(None).await.unwrap();
    }
    catalyst::comms::archipelago::shutdown(&server.state).await;
    drop(server);
    fixture.finish().await;
}

#[tokio::test]
async fn configured_close_during_blocked_room_sql_does_not_claim_from_authentication() {
    let fixture = Fixture::new()
        .await
        .expect("mandatory disposable PostgreSQL fixture");
    let server = Server::start(&fixture).await;
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    let (mut socket, nonce) = legacy_open(&server, &wallet).await;
    let mut held = fixture.pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE social.archipelago_v4_assignments IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *held)
        .await
        .unwrap();
    legacy_sign(&mut socket, &wallet, &wallet, &nonce).await;
    assert!(matches!(
        receive(&mut socket).await.message,
        Some(pb::server_packet::Message::Welcome(_))
    ));
    socket.close(None).await.unwrap();
    held.rollback().await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while server.state.registry.has_any_peer(&address) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let committed:i64=sqlx::query_scalar("SELECT count(*) FROM social.archipelago_v4_assignments WHERE owner_address=$1 AND lane_key='realm'").bind(&address).fetch_one(&fixture.pool).await.unwrap();
    assert_eq!(committed, 0);
    catalyst::comms::archipelago::shutdown(&server.state).await;
    drop(server);
    fixture.finish().await;
}
