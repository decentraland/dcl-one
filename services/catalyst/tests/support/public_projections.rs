//! Cross-service projection regressions. LiveKit rosters are adapter-controlled;
//! Pulse topology, discovery, signed sockets and subscriber admission are real.
use super::*;
use catalyst::comms::pulse::cluster::nats::NatsFeedOptions;
use serde_json::{Value, json};

struct Projection {
    _broker: Broker,
    _livekit: tokio::task::JoinHandle<()>,
    jobs: catalyst::jobs::Jobs,
    state: AppState,
    social: Arc<catalyst::social::rpc::State>,
    world: pulse_tracker::World,
    port: u16,
    db: db::ScratchDb,
    public_port: u16,
    _public: tokio::task::JoinHandle<()>,
    files: std::path::PathBuf,
}

impl Projection {
    async fn new() -> Self {
        let binary = gate::require_env("ARCHIPELAGO_NATS_SERVER_BIN").expect("mandatory real NATS fixture");
        let broker = Broker::start(&binary, None);
        gate::required_database_in_ci();
        let db = db::scratch_database()
            .await
            .expect("mandatory disposable PostgreSQL");
        // Existing RoomService HTTP adapter seam, with a complete stable roster.
        let app = axum::Router::new()
            .route("/twirp/livekit.RoomService/ListRooms", axum::routing::post(|| async {
                axum::Json(json!({"rooms":[{"name":"world-foo.dcl.eth","sid":"RM_lobby"},
                    {"name":"scene:main:GenesisScene","sid":"RM_shared"}]}))
            }))
            .route("/twirp/livekit.RoomService/ListParticipants", axum::routing::post(
                |axum::Json(body): axum::Json<Value>| async move {
                    let identity = if body["room"] == "world-foo.dcl.eth" { "0x00000000000000000000000000000000000000aa" }
                        else { "0x00000000000000000000000000000000000000bb" };
                    axum::Json(json!({"participants":[{"identity":identity,"sid":format!("PA_client_{identity}")},
                        {"identity":"authoritative-server","sid":format!("PA_server_{identity}")}]}))
                }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let livekit_url = format!("http://{}", listener.local_addr().unwrap());
        let livekit = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let mut gatekeeper = archipelago::gatekeeper_with_livekit(
            Some(broker_link(broker.url())),
            archipelago::configured_livekit(&livekit_url),
        );
        let shared = Arc::get_mut(&mut gatekeeper).unwrap();
        shared.pool = db.pool.clone();
        shared.cluster.control_v4_audience = Some("projection-test".into());
        shared.cluster.nats_url = Some(broker.url());
        shared.scenes = Arc::new(catalyst::comms::interfaces::test_support::StubScenes {
            room: Ok(catalyst::comms::rooms::ResolvedRoom {
                realm: catalyst::comms::world_identity::RealmId::World(
                    catalyst::comms::world_identity::WorldName::parse("foo.dcl.eth").unwrap(),
                ),
                scene_id: None,
                deployment: None,
                room: "world-foo.dcl.eth".into(),
            }),
            scene: Ok(None),
        });
        let configured = Arc::try_unwrap(gatekeeper).ok().unwrap();
        let cluster = configured.cluster.clone();
        let gatekeeper = Arc::new(configured.with_cluster(cluster).unwrap());
        let jobs = catalyst::jobs::Jobs::new();
        let state = catalyst::comms::archipelago::build_state(config(), gatekeeper, &jobs)
            .await
            .unwrap();
        assert!(state.control_v4.is_available());
        state.gatekeeper.start_island_feed(&jobs, state.clone());
        let social = Arc::new(catalyst::social::rpc::State::new(
            db.pool.clone(),
            Default::default(),
            None,
            state.gatekeeper.clone(),
        ));
        social.start_topology(&jobs, state.feed.topology.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let public = catalyst::comms::archipelago::mounts(state.clone(), true)
            .stats
            .merge(catalyst::comms::presence::router(
                state.gatekeeper.presence.clone(),
            ));
        tokio::spawn(async move { axum::serve(listener, public).await.unwrap() });
        let files = std::env::temp_dir().join(format!(
            "catalyst-public-projections-{}",
            uuid::Uuid::new_v4()
        ));
        let storage = Arc::new(
            catalyst::storage::Storage::from_config(&files)
                .await
                .unwrap(),
        );
        let content = Arc::new(catalyst::content::ContentState::new(
            db.pool.clone(),
            catalyst::content::Settings::for_tests(),
            storage,
        ));
        content
            .sync
            .set_state(catalyst::content::SyncState::Syncing);
        let about = Arc::new(catalyst::content::about_comms::AboutCommsState::new(
            catalyst::content::about_comms::Settings {
                protocol: "v3".into(),
                fixed_adapter: format!("ws-room:ws://127.0.0.1:{port}/ws"),
                version: None,
                commit_hash: None,
                v4: None,
                max_users: None,
                offline_when_unreachable: false,
            },
        ));
        assert!(about.attach_core(state.clone()));
        assert!(content.attach_about_comms(about));
        let sources = catalyst::explore::api::Sources {
            lists: String::new(),
            denylist: String::new(),
            feature_flags: String::new(),
            registry: String::new(),
            map: String::new(),
            ab_cdn: String::new(),
        };
        let mut settings = catalyst::explore::api::Settings::for_tests(sources);
        settings.mirrors_enabled = false;
        settings.ab_cdn_enabled = false;
        settings.ab_cdn_cache_dir = files.join("ab-cdn");
        let explore = catalyst::explore::api::State::new(
            settings,
            db.pool.clone(),
            catalyst::http::Outbound::for_tests(),
            content.clone(),
        )
        .await
        .unwrap();
        let public = catalyst::content::build_router(content).nest(
            "/realm-provider-ea",
            catalyst::explore::api::realm_provider::routes().with_state(explore),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let public_port = listener.local_addr().unwrap().port();
        let public_task = tokio::spawn(async move { axum::serve(listener, public).await.unwrap() });
        Self::advertised_at(public_port, false, 0, 0).await;
        let world = pulse_tracker::World::with_options(NatsFeedOptions {
            url: broker.url(),
            capacity: 64,
            discovery_interval_ms: 100,
            server_name: "projection-pulse".into(),
            commit_hash: "fixture-pin".into(),
        });
        world.enable_control_positions("projection-test");
        eprintln!(
            "projection fixture: v3_adapter=ws://127.0.0.1:{port}/ws native_Pulse_listener=absent backend=real_tracker_feed NATS={} audience=projection-test discovery_ms=100 outbox_capacity=64 max_sockets={} handshake_ms={} dedup_ms={} remote_flags=not_started limiter=not_started probes=/core-status,/stats/health,CommsCore",
            broker.url(),
            state.settings.server.max_sockets,
            state.settings.auth.handshake_timeout_ms,
            state.settings.server.island_changed_dedup_ms
        );
        Self {
            _broker: broker,
            _livekit: livekit,
            jobs,
            state,
            social,
            world,
            port,
            db,
            public_port,
            _public: public_task,
            files,
        }
    }

    async fn get(&self, path: &str) -> Value {
        let response = reqwest::get(format!("http://127.0.0.1:{}{path}", self.port))
            .await
            .unwrap();
        assert_eq!(response.status(), 200, "{path}");
        response.json().await.unwrap()
    }

    async fn advertised(&self, healthy: bool, count: u64, parcels: usize) {
        Self::advertised_at(self.public_port, healthy, count, parcels).await;
    }

    async fn advertised_at(port: u16, healthy: bool, count: u64, parcels: usize) {
        let base = format!("http://127.0.0.1:{port}");
        let response = reqwest::get(format!("{base}/about")).await.unwrap();
        assert_eq!(response.status().as_u16(), if healthy { 200 } else { 503 });
        let doc: Value = response.json().await.unwrap();
        assert_eq!(doc["healthy"], healthy);
        assert_eq!(doc["comms"]["healthy"], healthy);
        assert_eq!(doc["comms"]["usersCount"], count);
        let realms: Value = reqwest::get(format!("{base}/lambdas/explore/realms"))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(realms[0]["usersCount"], count);
        assert_eq!(realms[0]["userParcels"].as_array().unwrap().len(), parcels);
        for alias in ["main/about", "main", "about"] {
            let response = reqwest::get(format!("{base}/realm-provider-ea/{alias}"))
                .await
                .unwrap();
            assert_eq!(response.status(), 200);
            let doc: Value = response.json().await.unwrap();
            assert_eq!(doc["healthy"], healthy, "{alias}");
            assert_eq!(doc["acceptingUsers"], healthy, "{alias}");
            assert_eq!(doc["content"]["healthy"], true);
            assert_eq!(doc["comms"]["healthy"], healthy);
            assert_eq!(doc["comms"]["usersCount"], count);
            assert_eq!(doc["bff"]["userCount"], count);
        }
        let realms: Value = reqwest::get(format!("{base}/realm-provider-ea/realms"))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(realms.as_array().unwrap().len(), usize::from(healthy));
        if healthy {
            assert_eq!(realms[0]["usersCount"], count);
        }
    }

    async fn bridge(&mut self, socket: &mut WsStream, wallet: &str) {
        tokio::time::timeout(Duration::from_secs(12), async {
            while !self.social.ctx.is_online(wallet) {
                socket.send(WsMessage::Binary(packet(client_packet::Message::Heartbeat(
                    catalyst::comms::protocol::archipelago::archipelago::Heartbeat {
                        position: Some(catalyst::comms::protocol::archipelago::decentraland::common::Position {
                            x: 1.0, y: 2.0, z: 3.0 }),
                        desired_room: Some("shared-realm".into()), sample_sequence: None,
                    })))).await.unwrap();
                self.world.pass();
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }).await.expect("signed heartbeat -> configured control NATS -> real Pulse -> social");
        self.world.pass();
    }

    async fn island(&mut self, socket: &mut WsStream) -> IslandChangedMessage {
        let receiving = actual_island(socket);
        tokio::pin!(receiving);
        let mut tick = tokio::time::interval(Duration::from_millis(100));
        loop {
            tokio::select! {
                island = &mut receiving => return island,
                _ = tick.tick() => self.world.pass(),
            }
        }
    }

    async fn finish(self) {
        self.state.restart.cancel();
        self.jobs.shutdown(Duration::from_secs(5)).await;
        self._livekit.abort();
        self._public.abort();
        let _ = std::fs::remove_dir_all(&self.files);
        drop((self.state, self.social, self.world));
        self.db.teardown().await;
    }
}

#[tokio::test]
async fn baseline_pulse_startup_fallback_preserves_room_flow() {
    use catalyst::content::about_comms::CommsCore;
    let mut fixture = Projection::new().await;
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    // No native Pulse transport/registration exists. Only the legacy signed socket
    // supplies positions; the Pulse backend continues to run for fallback clients.
    let (mut socket, session) = assigned_socket(fixture.port, &wallet).await;
    fixture.bridge(&mut socket, &address).await;
    let island = fixture.island(&mut socket).await;
    jwt_payload(&island, &session);
    assert!(
        island.assignment_context.is_none(),
        "ordinary V3 wire remains unchanged"
    );
    assert!(fixture.social.ctx.is_online(&address));
    fixture
        .state
        .gatekeeper
        .presence
        .recover_complete(&fixture.state.gatekeeper)
        .await
        .unwrap();
    assert_eq!(fixture.get("/live-data").await["data"]["totalUsers"], 1);
    // The established hot-scenes endpoint counts positional connector peers,
    // independently of LiveKit world/scene occupancy and Pulse transport choice.
    sqlx::query("INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain) VALUES ('0x0000000000000000000000000000000000000001', 'v3', 'scene', 'projection-genesis', $1, timezone('UTC', now()), ARRAY['0,0'], timezone('UTC', now()), '[]'::json)")
        .bind(json!({"scene":{"base":"0,0","parcels":["0,0"]},"display":{"title":"Genesis"}}))
        .execute(&fixture.db.pool).await.unwrap();
    sqlx::query("INSERT INTO content.active_pointers (pointer, entity_id, entity_type) VALUES ('0,0', 'projection-genesis', 'scene')")
        .execute(&fixture.db.pool).await.unwrap();
    let hot = fixture.get("/hot-scenes").await;
    assert_eq!(hot[0]["id"], "projection-genesis");
    assert_eq!(hot[0]["usersTotalCount"], 1);
    let islands = fixture.get("/islands").await;
    assert!(islands["islands"].as_array().unwrap().iter().any(|i| {
        catalyst::comms::rooms::island_room_name(i["id"].as_str().unwrap()) == island.island_id
    }));
    let peer = fixture.get(&format!("/peers/{address}")).await;
    assert!(peer.to_string().contains(&address));
    wait_for(|| fixture.state.core_status().0).await;
    assert_eq!(
        fixture.state.core_status().1,
        0,
        "discovery advertises native board load; bridge-only topology has no native transport peer"
    );
    assert_eq!(fixture.get("/core-status").await["healthy"], true);
    fixture.advertised(true, 0, 1).await;
    // Close drains the configured owner/session rather than manufacturing a board removal.
    socket.close(None).await.unwrap();
    tokio::time::timeout(Duration::from_secs(12), async {
        while fixture.social.ctx.is_online(&address) {
            fixture.world.pass();
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("configured socket close removes bridge membership");
    let (mut reconnected, next_session) = assigned_socket(fixture.port, &wallet).await;
    assert_ne!(session, next_session);
    fixture.bridge(&mut reconnected, &address).await;
    // No successor credential may be delivered under unsafe configured removal;
    // heartbeat membership itself is independent of LiveKit takeover policy.
    // Stop sending heartbeats while keeping the authenticated socket open.
    tokio::time::timeout(Duration::from_secs(15), async {
        while fixture.social.ctx.is_online(&address) {
            fixture.world.pass();
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("heartbeat expiry removes stationary legacy membership");
    assert_eq!(fixture.get("/core-status").await["healthy"], true);
    drop((socket, reconnected));
    fixture.finish().await;
}

#[tokio::test]
async fn baseline_livekit_only_client_has_expected_online_projection() {
    let mut fixture = Projection::new().await;
    fixture
        .state
        .gatekeeper
        .presence
        .recover_complete(&fixture.state.gatekeeper)
        .await
        .unwrap();
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    let (mut socket, _) = assigned_socket(fixture.port, &wallet).await;
    fixture.bridge(&mut socket, &address).await;
    assert!(
        fixture.social.ctx.is_online(&address),
        "positional legacy client is bridged"
    );
    for identity in [
        "0x00000000000000000000000000000000000000aa",
        "0x00000000000000000000000000000000000000bb",
    ] {
        assert!(
            !fixture.social.ctx.is_online(identity),
            "LiveKit-only is offline under decision32"
        );
    }
    assert_eq!(
        fixture
            .state
            .gatekeeper
            .presence
            .world_participant_count("foo.dcl.eth"),
        1
    );
    assert_eq!(
        fixture
            .state
            .gatekeeper
            .presence
            .requested_scene_occupancy(&fixture.state.gatekeeper, "scene:main:GenesisScene")
            .await
            .unwrap(),
        1
    );
    let live = fixture.get("/live-data").await;
    assert_eq!(
        live["data"]["totalUsers"], 1,
        "shared scene does not widen lobby public occupancy"
    );
    assert_eq!(
        fixture
            .state
            .gatekeeper
            .presence
            .get_peer_world("0x00000000000000000000000000000000000000aa")
            .unwrap()
            .as_deref(),
        Some("foo.dcl.eth")
    );
    drop(socket);
    fixture.finish().await;
}

#[tokio::test]
async fn baseline_empty_topology_differs_from_feed_loss() {
    let mut fixture = Projection::new().await;
    fixture
        .state
        .gatekeeper
        .presence
        .recover_complete(&fixture.state.gatekeeper)
        .await
        .unwrap();
    let observed = fixture.state.gatekeeper.presence.last_updated();
    let wallet = "0x00000000000000000000000000000000000000cc";
    let session = "0x00000000000000000000000000000000000000dd";
    fixture.world.place(0, wallet, session);
    tokio::time::timeout(Duration::from_secs(8), async {
        while !fixture.social.ctx.is_online(wallet) {
            fixture.world.pass();
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
    fixture.world.remove(0);
    let empty_started = tokio::time::Instant::now();
    tokio::time::timeout(Duration::from_secs(3), async {
        while fixture.social.ctx.is_online(wallet) {
            fixture.world.pass();
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("valid produced empty clears before receipt TTL");
    assert!(empty_started.elapsed() < catalyst::comms::topology::TOPOLOGY_RECEIPT_TTL);
    assert_eq!(fixture.get("/islands").await["islands"], json!([]));
    fixture.world.place(0, wallet, session);
    tokio::time::timeout(Duration::from_secs(8), async {
        while !fixture.social.ctx.is_online(wallet) {
            fixture.world.pass();
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
    // Hold the real feed alive but stop tracker passes: its independent discovery
    // timer stays healthy while no replacement topology reaches the subscriber.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let received = fixture.state.feed.topology.snapshot().received_at;
    // Subscribe after passes stop and flush before choosing a source-time floor:
    // neither the HTTP cache nor queued older discovery can satisfy this proof.
    let observer = async_nats::connect(fixture._broker.url()).await.unwrap();
    let mut discovery = observer
        .subscribe(catalyst::comms::protocol::bus::topology::DISCOVERY_SUBJECT)
        .await
        .unwrap();
    observer.flush().await.unwrap();
    async fn advancing_discovery(sub: &mut async_nats::Subscriber, after: u64) -> u64 {
        tokio::time::timeout(Duration::from_secs(2), async {
            while let Some(frame) = sub.next().await {
                let message =
                    catalyst::comms::protocol::bus::topology::decode_discovery(&frame.payload)
                        .unwrap();
                let status = message.status.expect("produced discovery status");
                if status.current_time > after {
                    assert_eq!(status.user_count, 1);
                    return status.current_time;
                }
            }
            panic!("owned discovery subscription ended");
        })
        .await
        .expect("discovery source time must advance while tracker passes are stopped")
    }
    let stopped_at = chrono::Utc::now().timestamp_millis() as u64;
    let advancing_before_expiry = advancing_discovery(&mut discovery, stopped_at).await;
    assert!(fixture.social.ctx.is_online(wallet));
    assert_eq!(fixture.state.feed.topology.snapshot().received_at, received);
    tokio::time::sleep(Duration::from_millis(5500)).await;
    assert!(!fixture.social.ctx.is_online(wallet));
    let expired_at = chrono::Utc::now().timestamp_millis() as u64;
    let advancing_after_expiry = advancing_discovery(&mut discovery, expired_at).await;
    assert!(advancing_after_expiry > advancing_before_expiry);
    eprintln!(
        "independent discovery source times: stopped_floor={stopped_at} before_expiry={advancing_before_expiry} expired_floor={expired_at} after_expiry={advancing_after_expiry}; topology receipt unchanged"
    );
    assert_eq!(fixture.state.feed.topology.snapshot().received_at, received);
    assert_eq!(fixture.get("/core-status").await["healthy"], true);
    assert_eq!(
        fixture.get("/core-status").await["userCount"],
        1,
        "discovery is independent of receipt expiry"
    );
    assert_eq!(
        fixture
            .state
            .gatekeeper
            .presence
            .world_participant_count("foo.dcl.eth"),
        1
    );
    assert_eq!(
        fixture.state.gatekeeper.presence.last_updated(),
        observed,
        "discovery/topology never renew LiveKit source age"
    );
    let live = fixture.get("/live-data").await;
    assert_eq!(live["data"]["totalUsers"], 1);
    assert_eq!(live["lastUpdated"], observed);
    fixture.finish().await;
}

#[tokio::test]
async fn advertised_counts_follow_produced_native_discovery() {
    use catalyst::content::about_comms::CommsCore;
    let mut fixture = Projection::new().await;
    // Constructor asserted complete unavailable responses before spawning discovery.
    fixture.world.place(
        0,
        "0x00000000000000000000000000000000000000cc",
        "0x00000000000000000000000000000000000000dd",
    );
    tokio::time::timeout(Duration::from_secs(8), async {
        while fixture.state.core_status() != (true, 1) {
            fixture.world.pass();
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("real native board discovery observed");
    fixture.advertised(true, 1, 0).await;
    fixture.world.remove(0);
    tokio::time::timeout(Duration::from_secs(8), async {
        while fixture.state.core_status() != (true, 0) || !fixture.state.parcel_counts().is_empty()
        {
            fixture.world.pass();
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("produced fresh zero discovery and empty positions observed");
    fixture.advertised(true, 0, 0).await;
    fixture.finish().await;
}
