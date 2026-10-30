// from: catalyrst/crates/catalyrst-archipelago/tests/ban_eviction.rs
//! A ban that lands mid-session evicts the wallet from the signalling socket, the peer
//! directory and the LiveKit room. The ban authority is the local policy over a scratch
//! database (required in CI); LiveKit's RoomService is an HTTP stub over the real primary job.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::Uri;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{Value, json};

use alloy::signers::{SignerSync, local::PrivateKeySigner};
use catalyst::comms::archipelago::config::{ClusterConfig, Settings};
use catalyst::comms::archipelago::peers::PeerDirectory;
use catalyst::comms::archipelago::registry::{PeersRegistry, SocketEvent};
use catalyst::comms::archipelago::{self as connector, AppState};
use catalyst::comms::gatekeeper::admission::cluster::gateway::StateGateway;
use catalyst::comms::gatekeeper::admission::cluster::sockets::IslandSockets;
use catalyst::comms::gatekeeper::admission::cluster::subscriber::ClusterGateway;
use catalyst::comms::gatekeeper::{CommsState, LiveKit, SharedState};
use catalyst::comms::interfaces::test_support::{StubDirectory, StubScenes};
use catalyst::comms::protocol::archipelago::Position;
use catalyst::comms::protocol::archipelago::archipelago::{
    ChallengeRequestMessage, ClientPacket, Heartbeat, IslandChangedMessage, KickedReason,
    ServerPacket, SignedChallengeMessage, client_packet, server_packet,
};
use catalyst::comms::rooms::island_room_name;
use catalyst::jobs::{JobState, Jobs};
use catalyst::worlds::policy::{PersonalWorldsPolicy, Policy};
use futures::{SinkExt, StreamExt};
use prost::Message as _;
use tokio_tungstenite::tungstenite::Message as WsMessage;

#[path = "support/archipelago.rs"]
mod archipelago;
#[path = "support/db.rs"]
mod db;

/// The ban authority over the scratch database: `ban` blocks a wallet for the next check.
struct Bans {
    pool: sqlx::PgPool,
}

impl Bans {
    fn policy(&self) -> Arc<Policy> {
        Arc::new(Policy::new(
            self.pool.clone(),
            PersonalWorldsPolicy::disabled(),
            None,
        ))
    }

    async fn ban(&self, address: &str) {
        sqlx::query(
            "INSERT INTO worlds.blocked_wallets (wallet) VALUES ($1) ON CONFLICT DO NOTHING",
        )
        .bind(address.to_ascii_lowercase())
        .execute(&self.pool)
        .await
        .unwrap();
    }
}

fn config(ban_sweep_interval_secs: u64) -> Settings {
    let mut settings = archipelago::settings();
    settings.cluster.ban_sweep_interval_secs = ban_sweep_interval_secs;
    settings
}

async fn scratch_database() -> Option<db::ScratchDb> {
    if std::env::var_os("GITHUB_ACTIONS").is_some() {
        assert!(
            std::env::var("CATALYST_TEST_DATABASE_URL").is_ok_and(|value| !value.trim().is_empty()),
            "ban eviction requires disposable PostgreSQL in CI"
        );
    }
    db::scratch_database().await
}

/// Build every pool-owning dependency from the same real database, as serve does.
fn gatekeeper(bans: &Bans, livekit: LiveKit) -> SharedState {
    let mut settings = catalyst::comms::gatekeeper::Settings::for_tests();
    settings.livekit = livekit;
    // This SFU fixture confirms the supplied token cutoff on a successful RPC.
    // Self-hosted quarantine/restart is covered separately by the real-Pg library suite.
    Arc::new(
        CommsState::new(
            bans.pool.clone(),
            catalyst::http::Outbound::for_tests().allowing_private_hosts(),
            settings,
            None,
            None,
            bans.policy(),
            Arc::new(StubScenes::default()),
            Arc::new(catalyst::comms::presence::Presence::new()),
            Arc::new(StubDirectory::default()),
        )
        .with_cluster(
            catalyst::comms::gatekeeper::admission::cluster::ClusterConfig {
                self_hosted_token_quarantine: false,
                ..Default::default()
            },
        )
        .unwrap(),
    )
}

async fn start_server(cfg: Settings, bans: &Bans) -> (u16, AppState, Jobs) {
    let livekit = catalyst::comms::gatekeeper::Settings::for_tests().livekit;
    start_server_beside(cfg, gatekeeper(bans, livekit)).await
}

/// The connector beside `gatekeeper`, whose LiveKit mints its grants and evicts its kicks.
async fn start_server_beside(cfg: Settings, gatekeeper: SharedState) -> (u16, AppState, Jobs) {
    let jobs = Jobs::new();
    let state = connector::build_state(cfg, gatekeeper, &jobs)
        .await
        .expect("state");
    connector::spawn_jobs(&state, &jobs);
    state.gatekeeper.spawn_jobs(&jobs);
    let port = archipelago::serve(state.clone()).await;
    (port, state, jobs)
}

type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

fn encode(msg: client_packet::Message) -> tokio_tungstenite::tungstenite::Bytes {
    ClientPacket {
        request_sequence: 0,
        message: Some(msg),
    }
    .encode_to_vec()
    .into()
}

async fn recv_msg(ws: &mut WsStream, timeout: Duration) -> Option<server_packet::Message> {
    loop {
        let frame = tokio::time::timeout(timeout, ws.next()).await.ok()??;
        match frame.ok()? {
            WsMessage::Binary(bytes) => return ServerPacket::decode(bytes.as_ref()).ok()?.message,
            WsMessage::Ping(_) | WsMessage::Pong(_) => continue,
            _ => return None,
        }
    }
}

async fn handshake(port: u16, wallet: &PrivateKeySigner) -> WsStream {
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws"))
        .await
        .expect("ws connect");
    let address = format!("{:#x}", wallet.address());
    ws.send(WsMessage::Binary(encode(
        client_packet::Message::ChallengeRequest(ChallengeRequestMessage {
            address: address.clone(),
        }),
    )))
    .await
    .expect("send challenge request");

    let challenge = match recv_msg(&mut ws, Duration::from_secs(3)).await {
        Some(server_packet::Message::ChallengeResponse(r)) => r.challenge_to_sign,
        other => panic!("expected challenge response, got {other:?}"),
    };

    let hash = alloy::primitives::eip191_hash_message(challenge.as_bytes());
    let sig = wallet.sign_hash_sync(&hash).expect("sign");
    let chain = json!([
        { "type": "SIGNER", "payload": address, "signature": "" },
        { "type": "ECDSA_SIGNED_ENTITY", "payload": challenge, "signature": sig.to_string() }
    ]);
    ws.send(WsMessage::Binary(encode(
        client_packet::Message::SignedChallenge(SignedChallengeMessage {
            auth_chain_json: chain.to_string(),
        }),
    )))
    .await
    .expect("send signed challenge");

    match recv_msg(&mut ws, Duration::from_secs(3)).await {
        Some(server_packet::Message::Welcome(_)) => {}
        other => panic!("expected welcome, got {other:?}"),
    }
    ws
}

async fn wait_for_peer(state: &AppState, addr: &str) {
    for _ in 0..60 {
        if state.peers.peer(addr).is_some() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the heartbeat never reached the peer directory");
}

async fn send_heartbeat(ws: &mut WsStream) {
    let hb = Heartbeat {
        sample_sequence: None,
        position: Some(Position {
            x: 8.0,
            y: 0.0,
            z: 8.0,
        }),
        desired_room: Some("catalyrst".into()),
    };
    ws.send(WsMessage::Binary(encode(
        client_packet::Message::Heartbeat(hb),
    )))
    .await
    .expect("send heartbeat");
}

#[tokio::test]
async fn mid_session_ban_sweep_kicks_and_closes_socket() {
    let Some(scratch) = scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let bans = Bans {
        pool: scratch.pool.clone(),
    };
    let (port, state, jobs) = start_server(config(30), &bans).await;

    let wallet = PrivateKeySigner::random();
    let addr = format!("{:#x}", wallet.address());
    let mut ws = handshake(port, &wallet).await;

    send_heartbeat(&mut ws).await;
    wait_for_peer(&state, &addr).await;

    bans.ban(&addr).await;
    state.peers.ban_sweep_once().await;

    match recv_msg(&mut ws, Duration::from_secs(3)).await {
        Some(server_packet::Message::Kicked(k)) => {
            assert_eq!(k.reason, KickedReason::KrNewSession as i32);
        }
        other => panic!("banned peer should receive a Kicked packet, got {other:?}"),
    }
    assert!(
        recv_msg(&mut ws, Duration::from_secs(3)).await.is_none(),
        "socket is closed after the kick"
    );
    assert!(
        state.peers.peer(&addr).is_none(),
        "banned peer evicted from the peer directory"
    );
    jobs.shutdown(Duration::from_secs(2)).await;
    scratch.teardown().await;
}

#[tokio::test]
async fn periodic_ban_sweep_evicts_without_manual_trigger() {
    let Some(scratch) = scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let bans = Bans {
        pool: scratch.pool.clone(),
    };
    let (port, state, jobs) = start_server(config(1), &bans).await;

    let wallet = PrivateKeySigner::random();
    let addr = format!("{:#x}", wallet.address());
    let mut ws = handshake(port, &wallet).await;

    send_heartbeat(&mut ws).await;
    wait_for_peer(&state, &addr).await;

    bans.ban(&addr).await;

    match recv_msg(&mut ws, Duration::from_secs(6)).await {
        Some(server_packet::Message::Kicked(_)) => {}
        other => panic!("the periodic sweep should kick the banned peer, got {other:?}"),
    }
    assert!(state.peers.peer(&addr).is_none());
    jobs.shutdown(Duration::from_secs(2)).await;
    scratch.teardown().await;
}

#[tokio::test]
async fn the_sweep_kicks_every_session_of_the_banned_wallet_and_drops_it() {
    let Some(scratch) = scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let bans = Bans {
        pool: scratch.pool.clone(),
    };
    let ban = archipelago::ban_checker(bans.policy());
    let registry = PeersRegistry::new();
    let peers = PeerDirectory::new(
        ClusterConfig::default(),
        archipelago::gatekeeper(None),
        ban,
        Arc::clone(&registry),
    );
    let wallet = "0x0000000000000000000000000000000000000abc";
    let (_desktop, mut desktop_events, _) = registry.on_peer_connected(wallet, "0xsession1");
    let (_mobile, mut mobile_events, _) = registry.on_peer_connected(wallet, "0xsession2");

    peers.upsert_peer(wallet.into(), [8.0, 0.0, 8.0], [0, 0], "r".into());

    bans.ban(wallet).await;
    peers.ban_sweep_once().await;

    assert!(
        matches!(desktop_events.try_recv(), Ok(SocketEvent::Kicked)),
        "every live session of the banned wallet is kicked"
    );
    assert!(matches!(mobile_events.try_recv(), Ok(SocketEvent::Kicked)));
    assert!(
        peers.peer(wallet).is_none(),
        "the sweep removes the banned peer"
    );

    peers.upsert_peer(wallet.into(), [8.0, 0.0, 8.0], [0, 0], "r".into());
    assert!(
        peers.peer(wallet).is_some(),
        "a later heartbeat re-admits the wallet; the ban gate is the sweep and the handshake, \
         not a local kicked set"
    );
    scratch.teardown().await;
}

#[derive(Default)]
struct SeenRemoval {
    room: Option<String>,
    identity: Option<String>,
    auth: Option<String>,
    dispatch: Option<uuid::Uuid>,
    calls: usize,
    room_reads: usize,
    roster_reads: usize,
    completed: usize,
}

struct LivekitMock {
    pool: sqlx::PgPool,
    room: String,
    wallet: String,
    api_url: String,
    seen: Mutex<SeenRemoval>,
    entered: tokio::sync::Notify,
    release: Option<Arc<tokio::sync::Notify>>,
    completed: tokio::sync::Notify,
}

async fn room_service(
    State(mock): State<Arc<LivekitMock>>,
    uri: Uri,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    match uri.path().rsplit('/').next().unwrap() {
        "ListRooms" => {
            if let Some(names) = body.get("names") {
                assert_eq!(*names, json!([mock.room]));
                mock.seen.lock().unwrap().room_reads += 1;
            } else {
                // Other real primary jobs also read the complete room inventory.
                // Count only filtered reads toward discovery/SID preflight below.
                assert_eq!(body, json!({}));
            }
            Json(json!({"rooms":[{"name":mock.room,"sid":"RM_I7"}]}))
        }
        "ListParticipants" => {
            assert_eq!(body["room"], mock.room);
            let mut seen = mock.seen.lock().unwrap();
            seen.roster_reads += 1;
            Json(if seen.completed == 0 {
                json!({"participants":[{"identity":mock.wallet,"sid":"PA_peer"}]})
            } else {
                json!({"participants":[]})
            })
        }
        "RemoveParticipant" => {
            assert_eq!(body["room"], mock.room);
            assert_eq!(body["identity"], mock.wallet);
            let cutoff = body["revokeTokenTs"]
                .as_str()
                .expect("durable cutoff on actual RPC")
                .parse::<i64>()
                .unwrap();
            // The actual HTTP boundary observes an already committed exact attempt,
            // linked to a real captured intent, before it performs its remote effect.
            let dispatch: uuid::Uuid = sqlx::query_scalar(
                "SELECT d.dispatch_id FROM social.island_cleanup_dispatches d JOIN social.island_admission_fences f ON f.wallet=d.wallet JOIN social.livekit_removal_intents i ON i.root_key=d.intent_root_key AND i.target_key=d.intent_target_key AND i.generation=d.intent_generation WHERE d.wallet=$1 AND d.room_name=$2 AND d.participant_identity=$1 AND d.cutoff=$3 AND d.state='dispatched' AND d.purpose='island_enforcement' AND d.enforcement_cause='connection' AND d.pulse_source IS NULL AND d.pulse_takeover_id IS NULL AND d.settled_at IS NULL AND f.generation>=d.fence_generation AND i.cause_kind='connection' AND i.actor=$1 AND i.room_sid='RM_I7'"
            ).bind(&mock.wallet).bind(&mock.room).bind(cutoff).fetch_one(&mock.pool).await
                .expect("durable wallet hazard and typed captured intent committed before RPC");
            {
                let mut seen = mock.seen.lock().unwrap();
                seen.room = body["room"].as_str().map(str::to_owned);
                seen.identity = body["identity"].as_str().map(str::to_owned);
                seen.auth = headers
                    .get("authorization")
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_owned);
                seen.dispatch = Some(dispatch);
                seen.calls += 1;
            }
            // An accepted remote action outlives the caller's HTTP timeout.
            let remote = tokio::spawn(async move {
                mock.entered.notify_one();
                if let Some(release) = &mock.release {
                    release.notified().await;
                }
                mock.seen.lock().unwrap().completed += 1;
                mock.completed.notify_one();
                json!({})
            });
            Json(remote.await.expect("accepted SFU action completed"))
        }
        other => panic!("unexpected RoomService method {other}"),
    }
}

async fn start_livekit_mock(
    pool: sqlx::PgPool,
    wallet: &str,
    pause: bool,
) -> (Arc<LivekitMock>, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mock = Arc::new(LivekitMock {
        pool,
        room: island_room_name("I7"),
        wallet: wallet.to_owned(),
        api_url: format!("http://{}", listener.local_addr().unwrap()),
        seen: Default::default(),
        entered: Default::default(),
        release: pause.then(|| Arc::new(tokio::sync::Notify::new())),
        completed: Default::default(),
    });
    let app = Router::new()
        .fallback(post(room_service))
        .with_state(mock.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (mock, server)
}

fn primary(gatekeeper: &SharedState) -> Jobs {
    let jobs = Jobs::new();
    gatekeeper.spawn_jobs(&jobs);
    jobs
}

async fn wait_for_passes(jobs: &Jobs, passes: u64) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let report = jobs.report();
            let worker = report
                .iter()
                .find(|job| job.name == "comms-livekit-ban-removal")
                .unwrap();
            assert_ne!(worker.state, JobState::Failed, "{:?}", worker.last_error);
            if worker.passes >= passes {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("actual primary removal worker progressed");
}

async fn wait_for_dispatch(pool: &sqlx::PgPool, wallet: &str, state: &str) -> uuid::Uuid {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let id = sqlx::query_scalar("SELECT dispatch_id FROM social.island_cleanup_dispatches WHERE wallet=$1 AND state=$2")
                .bind(wallet).bind(state).fetch_optional(pool).await.unwrap();
            if let Some(id) = id { return id; }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.expect("persisted attempt reached expected state")
}

async fn assert_enqueued_without_rpc(state: &SharedState, mock: &LivekitMock) {
    assert!(
        state
            .kick_island_participant(&mock.room, &mock.wallet)
            .await
    );
    let (cause, scope): (String, Value) = sqlx::query_as("SELECT cause_kind,scope FROM social.livekit_removal_intents WHERE wallet=$1 AND target_key=''")
        .bind(&mock.wallet).fetch_one(&mock.pool).await.unwrap();
    assert_eq!(cause, "connection");
    assert_eq!(scope, json!([mock.room]));
    assert_eq!(mock.seen.lock().unwrap().calls, 0, "enqueue is not an RPC");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM social.island_cleanup_dispatches WHERE wallet=$1"
        )
        .bind(&mock.wallet)
        .fetch_one(&mock.pool)
        .await
        .unwrap(),
        0
    );
}

fn assert_exact_removal(mock: &LivekitMock) {
    let seen = mock.seen.lock().unwrap();
    assert_eq!(seen.room.as_deref(), Some(mock.room.as_str()));
    assert_eq!(seen.identity.as_deref(), Some(mock.wallet.as_str()));
    let auth = seen
        .auth
        .as_deref()
        .expect("admin call carries a bearer token");
    assert!(
        auth.starts_with("Bearer "),
        "authorization is a bearer token, got {auth}"
    );
    assert!(seen.dispatch.is_some());
    assert!(
        seen.room_reads >= 2 && seen.roster_reads >= 1,
        "real discovery and SID preflight ran"
    );
    assert_eq!(seen.calls, 1);
}

#[tokio::test]
async fn remove_participant_calls_livekit_room_service() {
    let Some(scratch) = scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let bans = Bans {
        pool: scratch.pool.clone(),
    };
    let wallet = format!("{:#x}", PrivateKeySigner::random().address());
    bans.ban(&wallet).await;
    let (mock, server) = start_livekit_mock(scratch.pool.clone(), &wallet, false).await;
    let state = gatekeeper(&bans, archipelago::configured_livekit(&mock.api_url));
    assert_enqueued_without_rpc(&state, &mock).await;

    let replica = Jobs::new().with_fleet_jobs(false);
    state.spawn_jobs(&replica);
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let report = replica.report();
            let worker = report
                .iter()
                .find(|job| job.name == "comms-livekit-ban-removal")
                .unwrap();
            if worker.state == JobState::StoodDown {
                break;
            }
            assert_ne!(worker.state, JobState::Failed);
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("replica worker explicitly stood down");
    assert_eq!(
        mock.seen.lock().unwrap().calls,
        0,
        "replica starts no destructive RPC"
    );
    replica.shutdown(Duration::from_secs(2)).await;

    let jobs = primary(&state);
    let id = wait_for_dispatch(&scratch.pool, &wallet, "confirmed").await;
    assert_exact_removal(&mock);
    assert_eq!(mock.seen.lock().unwrap().dispatch, Some(id));
    let cutoff: i64 = sqlx::query_scalar(
        "SELECT cutoff FROM social.island_cleanup_dispatches WHERE dispatch_id=$1",
    )
    .bind(id)
    .fetch_one(&scratch.pool)
    .await
    .unwrap();
    let gateway = StateGateway::new(state.clone());
    assert!(
        gateway
            .safety_store()
            .unwrap()
            .snapshot(&wallet)
            .await
            .unwrap()
            .token_floor
            >= cutoff as u64
    );
    jobs.shutdown(Duration::from_secs(2)).await;

    // Restart through the real constructor/runner: a confirmed operation is not resent,
    // even once policy is lifted; the durable floor remains available to admission.
    sqlx::query("DELETE FROM worlds.blocked_wallets WHERE wallet=$1")
        .bind(&wallet)
        .execute(&scratch.pool)
        .await
        .unwrap();
    let restarted = gatekeeper(&bans, archipelago::configured_livekit(&mock.api_url));
    let jobs = primary(&restarted);
    wait_for_passes(&jobs, 2).await;
    assert_eq!(mock.seen.lock().unwrap().calls, 1);
    assert!(
        StateGateway::new(restarted)
            .safety_store()
            .unwrap()
            .snapshot(&wallet)
            .await
            .unwrap()
            .token_floor
            >= cutoff as u64
    );
    jobs.shutdown(Duration::from_secs(2)).await;
    server.abort();
    scratch.teardown().await;
}

#[tokio::test]
async fn the_sweep_evicts_the_livekit_participant_of_a_wallet_that_never_heartbeat() {
    let Some(scratch) = scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let bans = Bans {
        pool: scratch.pool.clone(),
    };
    let wallet = PrivateKeySigner::random();
    let addr = format!("{:#x}", wallet.address());
    let (mock, server) = start_livekit_mock(scratch.pool.clone(), &addr, false).await;
    let gatekeeper = gatekeeper(&bans, archipelago::configured_livekit(&mock.api_url));
    let (port, state, jobs) = start_server_beside(config(3600), gatekeeper).await;
    let mut ws = handshake(port, &wallet).await;
    state.deliver_island(
        &addr,
        &addr,
        &IslandChangedMessage {
            assignment_context: None,
            island_id: mock.room.clone(),
            conn_str: "livekit:wss://example.invalid?access_token=t".into(),
            from_island_id: None,
            peers: Default::default(),
        },
        None,
    );
    match recv_msg(&mut ws, Duration::from_secs(3)).await {
        Some(server_packet::Message::IslandChanged(m)) => assert_eq!(m.island_id, mock.room),
        other => panic!("expected the island assignment, got {other:?}"),
    }
    assert!(
        state.peers.peer(&addr).is_none(),
        "no heartbeat has arrived"
    );
    assert_eq!(
        state.peers.island_of(&addr).as_deref(),
        Some(mock.room.as_str())
    );
    bans.ban(&addr).await;
    state.peers.ban_sweep_once().await;
    match recv_msg(&mut ws, Duration::from_secs(3)).await {
        Some(server_packet::Message::Kicked(k)) => {
            assert_eq!(k.reason, KickedReason::KrNewSession as i32)
        }
        other => panic!("banned socket must be kicked, got {other:?}"),
    }
    assert!(
        recv_msg(&mut ws, Duration::from_secs(3)).await.is_none(),
        "socket closes after kick"
    );
    assert!(state.peers.peer(&addr).is_none());
    assert!(
        state.peers.island_of(&addr).is_none(),
        "assignment is evicted without a heartbeat"
    );
    wait_for_dispatch(&scratch.pool, &addr, "confirmed").await;
    assert_exact_removal(&mock);
    jobs.shutdown(Duration::from_secs(2)).await;
    server.abort();
    scratch.teardown().await;
}

#[tokio::test]
async fn lifting_policy_before_primary_dispatch_cancels_without_an_rpc() {
    let Some(scratch) = scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let bans = Bans {
        pool: scratch.pool.clone(),
    };
    let wallet = format!("{:#x}", PrivateKeySigner::random().address());
    bans.ban(&wallet).await;
    let (mock, server) = start_livekit_mock(scratch.pool.clone(), &wallet, false).await;
    let state = gatekeeper(&bans, archipelago::configured_livekit(&mock.api_url));
    assert_enqueued_without_rpc(&state, &mock).await;
    sqlx::query("DELETE FROM worlds.blocked_wallets WHERE wallet=$1")
        .bind(&wallet)
        .execute(&scratch.pool)
        .await
        .unwrap();
    let jobs = primary(&state);
    wait_for_passes(&jobs, 2).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM social.livekit_removal_intents WHERE wallet=$1"
        )
        .bind(&wallet)
        .fetch_one(&scratch.pool)
        .await
        .unwrap(),
        0,
        "real worker consumed the cancelled intent"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM social.island_cleanup_dispatches WHERE wallet=$1"
        )
        .bind(&wallet)
        .fetch_one(&scratch.pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(mock.seen.lock().unwrap().calls, 0);
    jobs.shutdown(Duration::from_secs(2)).await;
    server.abort();
    scratch.teardown().await;
}

#[tokio::test]
async fn unknown_island_removal_survives_primary_restart_and_policy_lift_without_retry() {
    let Some(scratch) = scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let bans = Bans {
        pool: scratch.pool.clone(),
    };
    let wallet = format!("{:#x}", PrivateKeySigner::random().address());
    bans.ban(&wallet).await;
    let (mock, server) = start_livekit_mock(scratch.pool.clone(), &wallet, true).await;
    let state = gatekeeper(&bans, archipelago::configured_livekit(&mock.api_url));
    assert_enqueued_without_rpc(&state, &mock).await;
    let jobs = primary(&state);
    tokio::time::timeout(Duration::from_secs(10), mock.entered.notified())
        .await
        .expect("actual RPC entered after marker");
    let id = wait_for_dispatch(&scratch.pool, &wallet, "dispatched").await;
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let attempts: i32 = sqlx::query_scalar("SELECT attempts FROM social.livekit_removal_intents WHERE wallet=$1 AND target_key<>''")
                .bind(&wallet).fetch_one(&scratch.pool).await.unwrap();
            if attempts >= 1 { break; }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.expect("real HTTP timeout recorded scheduling failure");
    assert!(
        StateGateway::new(state)
            .safety_store()
            .unwrap()
            .snapshot(&wallet)
            .await
            .is_err()
    );
    jobs.shutdown(Duration::from_secs(2)).await;
    sqlx::query("DELETE FROM worlds.blocked_wallets WHERE wallet=$1")
        .bind(&wallet)
        .execute(&scratch.pool)
        .await
        .unwrap();
    // Make scheduling due without changing the immutable unknown attempt.
    sqlx::query("UPDATE social.livekit_removal_intents SET next_attempt_at=timezone('UTC',clock_timestamp()) WHERE wallet=$1 AND target_key<>''")
        .bind(&wallet).execute(&scratch.pool).await.unwrap();
    let restarted = gatekeeper(&bans, archipelago::configured_livekit(&mock.api_url));
    let gateway = StateGateway::new(restarted.clone());
    assert!(
        gateway
            .safety_store()
            .unwrap()
            .snapshot(&wallet)
            .await
            .is_err()
    );
    let jobs = primary(&restarted);
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let attempts: i32 = sqlx::query_scalar("SELECT attempts FROM social.livekit_removal_intents WHERE wallet=$1 AND target_key<>''")
                .bind(&wallet).fetch_one(&scratch.pool).await.unwrap();
            if attempts >= 2 { break; }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.expect("restarted real worker rechecked due unknown without RPC");
    mock.release.as_ref().unwrap().notify_one();
    tokio::time::timeout(Duration::from_secs(3), mock.completed.notified())
        .await
        .expect("accepted remote effect outlived timeout");
    assert_exact_removal(&mock);
    assert_eq!(
        wait_for_dispatch(&scratch.pool, &wallet, "dispatched").await,
        id
    );
    assert!(
        gateway
            .safety_store()
            .unwrap()
            .snapshot(&wallet)
            .await
            .is_err(),
        "late remote effect alone is not settlement proof"
    );
    jobs.shutdown(Duration::from_secs(2)).await;
    server.abort();
    scratch.teardown().await;
}
