// from: catalyrst/crates/catalyrst-archipelago/tests/ws_island_dedup.rs
#[path = "support/gate.rs"]
mod gate;
#[path = "support/nats_broker.rs"]
mod nats_broker;
use alloy::signers::{SignerSync, local::PrivateKeySigner};
#[path = "support/archipelago.rs"]
mod archipelago;
use catalyst::comms::archipelago::AppState;
use catalyst::comms::archipelago::config::Settings;
use catalyst::comms::archipelago::feed::{Routed, route_island};
use catalyst::comms::gatekeeper::admission::cluster::sockets::IslandSockets;
use catalyst::comms::gatekeeper::bus::{BrokerBus, InProcessBus, NatsBus, RequestError};
use catalyst::comms::protocol::archipelago::archipelago::{
    ChallengeRequestMessage, ClientPacket, IslandChangedMessage, ServerPacket,
    SignedChallengeMessage, client_packet, server_packet,
};
use catalyst::comms::protocol::bus::cluster::ClusterLookup;
use futures::{SinkExt, StreamExt};
use prost::Message as _;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message as WsMessage;

type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

fn test_config(island_changed_dedup_ms: u64) -> Settings {
    let mut settings = archipelago::settings();
    settings.server.island_changed_dedup_ms = island_changed_dedup_ms;
    settings
}

/// The connector over an in-process gatekeeper link, which records what reaches the broker.
async fn start_server(dedup_ms: u64) -> (u16, AppState, Arc<InProcessBus>) {
    let bus = Arc::new(InProcessBus::new());
    let state = archipelago::state_with(
        test_config(dedup_ms),
        archipelago::lazy_pool(),
        archipelago::gatekeeper(Some(bus.clone())),
    )
    .await;
    let port = archipelago::serve(state.clone()).await;
    (port, state, bus)
}

/// The (wallet, session) of every lookup the island subscriber asked Pulse, in order.
type Lookups = Arc<Mutex<Vec<(String, String)>>>;

/// [`start_server`] with the gatekeeper's island subscriber running beside the connector, as
/// `serve` wires them: each session the connector announces becomes one lookup, which goes
/// unanswered.
async fn start_announcing_server(dedup_ms: u64) -> (u16, AppState, Arc<InProcessBus>, Lookups) {
    let bus = Arc::new(InProcessBus::new());
    let lookups = Lookups::default();
    let asked = lookups.clone();
    bus.respond_to_requests(Arc::new(move |subject, payload| {
        if let Ok(lookup) = ClusterLookup::decode(subject, payload) {
            asked.lock().unwrap().push((lookup.wallet, lookup.session));
        }
        Err(RequestError::TimedOut)
    }));
    let state = archipelago::state_with(
        test_config(dedup_ms),
        archipelago::lazy_pool(),
        archipelago::gatekeeper(Some(bus.clone())),
    )
    .await;
    archipelago::start_island_feed(&state, &catalyst::jobs::Jobs::new());
    let port = archipelago::serve(state.clone()).await;
    (port, state, bus, lookups)
}

/// The sessions of `address` the subscriber was asked about, once at least `count` have been.
async fn announced(lookups: &Lookups, address: &str, count: usize) -> Vec<String> {
    let of_address = || {
        lookups
            .lock()
            .unwrap()
            .iter()
            .filter(|(wallet, _)| wallet == address)
            .map(|(_, session)| session.clone())
            .collect::<Vec<_>>()
    };
    tokio::time::timeout(Duration::from_secs(3), async {
        while of_address().len() < count {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("the announcements never reached the island subscriber");
    of_address()
}

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
            WsMessage::Binary(bytes) => {
                return ServerPacket::decode(bytes.as_ref()).ok()?.message;
            }
            WsMessage::Ping(_) | WsMessage::Pong(_) => continue,
            _ => return None,
        }
    }
}

async fn recv_island(ws: &mut WsStream, timeout: Duration) -> Option<String> {
    match recv_msg(ws, timeout).await {
        Some(server_packet::Message::IslandChanged(m)) => Some(m.island_id),
        _ => None,
    }
}

struct Device {
    ephemeral: PrivateKeySigner,
}

impl Device {
    fn new() -> Self {
        Self {
            ephemeral: PrivateKeySigner::random(),
        }
    }

    fn session(&self) -> String {
        format!("{:#x}", self.ephemeral.address())
    }
}

/// A full delegating chain: the wallet signs the ephemeral's mandate, the ephemeral signs the
/// challenge. This is the only shape that gives one wallet two distinct session keys, which is
/// what island assignments are addressed by.
async fn handshake(port: u16, wallet: &PrivateKeySigner, device: &Device) -> WsStream {
    let (mut ws, challenge) = begin_handshake(port, wallet).await;
    finish_handshake(&mut ws, wallet, device, &challenge).await;
    ws
}

async fn begin_handshake(port: u16, wallet: &PrivateKeySigner) -> (WsStream, String) {
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
        other => panic!("expected ChallengeResponse, got {other:?}"),
    };
    assert!(challenge.starts_with("dcl-"));
    assert_eq!(challenge.len(), 52);
    (ws, challenge)
}

async fn finish_handshake(
    ws: &mut WsStream,
    wallet: &PrivateKeySigner,
    device: &Device,
    challenge: &str,
) {
    ws.send(WsMessage::Binary(signed_challenge(
        wallet, device, challenge,
    )))
    .await
    .expect("send signed challenge");

    match recv_msg(ws, Duration::from_secs(3)).await {
        Some(server_packet::Message::Welcome(_)) => {}
        other => panic!("expected Welcome, got {other:?}"),
    }
}

fn signed_challenge(
    wallet: &PrivateKeySigner,
    device: &Device,
    challenge: &str,
) -> tokio_tungstenite::tungstenite::Bytes {
    let address = format!("{:#x}", wallet.address());
    let mandate = format!(
        "Decentraland Login\nEphemeral address: {}\nExpiration: 2099-12-31T00:00:00.000Z",
        device.session()
    );
    let mandate_sig = wallet
        .sign_hash_sync(&alloy::primitives::eip191_hash_message(mandate.as_bytes()))
        .expect("sign mandate");
    let challenge_sig = device
        .ephemeral
        .sign_hash_sync(&alloy::primitives::eip191_hash_message(
            challenge.as_bytes(),
        ))
        .expect("sign challenge");

    let chain = serde_json::json!([
        { "type": "SIGNER", "payload": address, "signature": "" },
        { "type": "ECDSA_EPHEMERAL", "payload": mandate, "signature": mandate_sig.to_string() },
        { "type": "ECDSA_SIGNED_ENTITY", "payload": challenge, "signature": challenge_sig.to_string() }
    ]);

    encode(client_packet::Message::SignedChallenge(
        SignedChallengeMessage {
            auth_chain_json: chain.to_string(),
        },
    ))
}

#[tokio::test]
async fn concurrent_same_wallet_challenges_both_welcome_and_route_independently() {
    let (port, state, _publisher) = start_server(0).await;
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    let first_device = Device::new();
    let second_device = Device::new();
    let (mut first, a) = begin_handshake(port, &wallet).await;
    let (mut second, b) = begin_handshake(port, &wallet).await;
    assert_ne!(a, b);
    finish_handshake(&mut first, &wallet, &first_device, &a).await;
    finish_handshake(&mut second, &wallet, &second_device, &b).await;
    for (device, socket, room) in [
        (&first_device, &mut first, "first"),
        (&second_device, &mut second, "second"),
    ] {
        assert!(
            state
                .registry
                .peer_link(&address, &device.session())
                .is_some()
        );
        route_island(&state, &address, &device.session(), &island(room));
        assert_eq!(
            recv_island(socket, Duration::from_secs(3)).await.as_deref(),
            Some(room)
        );
    }
}

#[tokio::test]
async fn a_delayed_older_handshake_cannot_take_a_newer_sessions_socket() {
    let (port, state, _publisher) = start_server(0).await;
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    let device = Device::new();
    let (mut older, older_challenge) = begin_handshake(port, &wallet).await;
    let (mut newer, newer_challenge) = begin_handshake(port, &wallet).await;

    finish_handshake(&mut newer, &wallet, &device, &newer_challenge).await;
    let current = state
        .registry
        .peer_link(&address, &device.session())
        .expect("newer handshake was admitted");
    older
        .send(WsMessage::Binary(signed_challenge(
            &wallet,
            &device,
            &older_challenge,
        )))
        .await
        .unwrap();
    assert!(matches!(
        recv_msg(&mut older, Duration::from_secs(3)).await,
        Some(server_packet::Message::Kicked(_))
    ));
    assert_eq!(
        state
            .registry
            .peer_link(&address, &device.session())
            .map(|link| link.id),
        Some(current.id)
    );

    route_island(
        &state,
        &address,
        &device.session(),
        &island("newer-still-owns"),
    );
    assert_eq!(
        recv_island(&mut newer, Duration::from_secs(3))
            .await
            .as_deref(),
        Some("newer-still-owns")
    );
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

#[tokio::test]
async fn the_feed_delivers_a_room_only_to_the_session_it_names() {
    let (port, state, _pub) = start_server(10_000).await;
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());

    let desktop = Device::new();
    let mobile = Device::new();
    let mut ws_desktop = handshake(port, &wallet, &desktop).await;
    let mut ws_mobile = handshake(port, &wallet, &mobile).await;
    assert!(
        state
            .registry
            .peer_link(&address, &desktop.session())
            .unwrap()
            .needs_assignment()
    );
    assert!(
        state
            .registry
            .peer_link(&address, &mobile.session())
            .unwrap()
            .needs_assignment()
    );

    assert_eq!(
        route_island(&state, &address, &desktop.session(), &island("island-A")),
        Routed::Delivered
    );

    assert_eq!(
        recv_island(&mut ws_desktop, Duration::from_secs(3)).await,
        Some("island-A".into())
    );
    assert!(
        !state
            .registry
            .peer_link(&address, &desktop.session())
            .unwrap()
            .needs_assignment()
    );
    assert!(
        state
            .registry
            .peer_link(&address, &mobile.session())
            .unwrap()
            .needs_assignment()
    );
    assert_eq!(
        recv_island(&mut ws_mobile, Duration::from_millis(400)).await,
        None,
        "the other device of the same wallet is not told about this assignment"
    );
    assert_eq!(
        state.peers.island_of(&address).as_deref(),
        Some("island-A"),
        "the assignment is recorded even though no heartbeat has arrived yet"
    );
}

#[tokio::test]
async fn a_repeated_room_inside_the_window_is_dropped_and_a_new_one_is_not() {
    let (port, state, _pub) = start_server(10_000).await;
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    let device = Device::new();
    let mut ws = handshake(port, &wallet, &device).await;
    let session = device.session();

    route_island(&state, &address, &session, &island("island-A"));
    assert_eq!(
        recv_island(&mut ws, Duration::from_secs(3)).await,
        Some("island-A".into())
    );
    route_island(&state, &address, &session, &island("island-A"));
    assert_eq!(recv_island(&mut ws, Duration::from_millis(100)).await, None);
    assert_eq!(state.feed.coalesced_count(), 0);
    assert_eq!(state.feed.socket_assignments_written_count(), 1);
    route_island(&state, &address, &session, &island("island-B"));
    assert_eq!(
        recv_island(&mut ws, Duration::from_secs(3)).await,
        Some("island-B".into()),
        "the repeat is dropped, so the next packet is the new room"
    );
    assert_eq!(
        recv_island(&mut ws, Duration::from_millis(400)).await,
        None,
        "nothing else follows"
    );
    assert_eq!(
        state.feed.delivered_count(),
        3,
        "the feed itself delivered all three; the window is a socket-side filter"
    );
    assert_eq!(
        state.feed.deduplicated_count(),
        1,
        "the suppressed repeat is the counter the runbook reads"
    );
}

#[tokio::test]
async fn a_zero_window_forwards_every_repeat() {
    let (port, state, _pub) = start_server(0).await;
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    let device = Device::new();
    let mut ws = handshake(port, &wallet, &device).await;
    let session = device.session();

    route_island(&state, &address, &session, &island("island-A"));
    assert_eq!(
        recv_island(&mut ws, Duration::from_secs(3)).await,
        Some("island-A".into())
    );
    route_island(&state, &address, &session, &island("island-A"));
    assert_eq!(
        recv_island(&mut ws, Duration::from_secs(3)).await,
        Some("island-A".into())
    );
}

#[tokio::test]
async fn a_stale_session_and_an_unknown_wallet_are_told_apart() {
    let (port, state, _pub) = start_server(10_000).await;
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    let device = Device::new();
    let _ws = handshake(port, &wallet, &device).await;

    let stale = format!("{:#x}", PrivateKeySigner::random().address());
    assert_eq!(
        route_island(&state, &address, &stale, &island("island-A")),
        Routed::NoSessionSocket
    );
    assert_eq!(
        route_island(
            &state,
            "0x00000000000000000000000000000000000000ff",
            &stale,
            &island("island-A"),
        ),
        Routed::NoSocket
    );
    assert_eq!(state.feed.no_session_socket_count(), 1);
}

#[tokio::test]
async fn a_reconnect_of_the_same_device_supersedes_the_previous_socket() {
    let (port, state, _pub) = start_server(10_000).await;
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    let device = Device::new();

    let mut old = handshake(port, &wallet, &device).await;
    let mut fresh = handshake(port, &wallet, &device).await;

    match recv_msg(&mut old, Duration::from_secs(3)).await {
        Some(server_packet::Message::Kicked(_)) => {}
        other => panic!("the superseded socket must be kicked, got {other:?}"),
    }

    assert_eq!(
        route_island(&state, &address, &device.session(), &island("island-A"),),
        Routed::Delivered
    );
    assert_eq!(
        recv_island(&mut fresh, Duration::from_secs(3)).await,
        Some("island-A".into())
    );
}

/// The welcome announces the session to the gatekeeper's island subscriber by call, which asks
/// Pulse for that session's cluster; nothing crosses the broker for it.
#[tokio::test]
async fn the_welcome_announces_the_session() {
    let (port, _state, bus, lookups) = start_announcing_server(10_000).await;
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    let device = Device::new();

    let _ws = handshake(port, &wallet, &device).await;

    let sessions = announced(&lookups, &address, 1).await;
    assert!(
        sessions.iter().all(|session| session == &device.session()),
        "the announce names the session, so the minter can address this device: {sessions:?}"
    );
    assert!(bus.published().is_empty(), "the announce crosses no broker");
}

/// Closing a socket tells the broker nothing: `peer.*.disconnect` had no consumer and is gone.
#[tokio::test]
async fn a_closed_socket_publishes_nothing() {
    let (port, state, bus, lookups) = start_announcing_server(10_000).await;
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    let device = Device::new();

    let mut ws = handshake(port, &wallet, &device).await;
    announced(&lookups, &address, 1).await;
    ws.close(None).await.ok();
    tokio::time::timeout(Duration::from_secs(3), async {
        while state.registry.has_peer(&address) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the closed socket was never retired");
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        bus.published().is_empty(),
        "no peer.*.disconnect: {:?}",
        bus.published()
    );
    assert_eq!(state.outbox.dropped(), 0);
}

/// The feed job's re-announce pass, seen where it lands: each session it announces is one lookup
/// the island subscriber makes. A link that drops and returns restarts the cycle with a full
/// sweep, so every live session is announced again within a readiness sample of the return.
#[tokio::test]
async fn a_re_announce_pass_announces_every_live_session() {
    let (port, _state, bus, lookups) = start_announcing_server(10_000).await;
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());

    let desktop = Device::new();
    let mobile = Device::new();
    let _ws_desktop = handshake(port, &wallet, &desktop).await;
    let _ws_mobile = handshake(port, &wallet, &mobile).await;
    let sessions = |announced: &[String]| {
        let mut sessions = announced.to_vec();
        sessions.sort();
        sessions.dedup();
        sessions
    };
    let mut expected = vec![desktop.session(), mobile.session()];
    expected.sort();

    let welcomed = announced(&lookups, &address, 2).await;
    assert_eq!(
        sessions(&welcomed),
        expected,
        "the handshakes themselves announced each session"
    );

    // Down long enough for the feed job's readiness sample (every 100 ms) to see it. A lookup
    // needs the link, so nothing reaches Pulse meanwhile.
    bus.set_connected(false);
    let before = announced(&lookups, &address, 0).await.len();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        announced(&lookups, &address, 0).await.len(),
        before,
        "nothing is asked while the link is down"
    );

    bus.set_connected(true);
    let all = announced(&lookups, &address, before + 2).await;
    assert_eq!(
        sessions(&all[before..]),
        expected,
        "every live session is re-announced, so a connect lost while the link was down is replayed"
    );
}

#[tokio::test]
async fn a_live_socket_retains_its_assignment_until_teardown_after_heartbeat_expiry() {
    let (port, state, _) = start_server(10_000).await;
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    let device = Device::new();
    let mut ws = handshake(port, &wallet, &device).await;
    assert_eq!(
        route_island(&state, &address, &device.session(), &island("island-kept")),
        Routed::Delivered
    );
    assert_eq!(
        recv_island(&mut ws, Duration::from_secs(3)).await,
        Some("island-kept".into())
    );
    state.peers.upsert_peer_at(
        address.clone(),
        [0.0; 3],
        [0, 0],
        "realm".into(),
        chrono::Utc::now() - chrono::Duration::days(1),
    );
    assert_eq!(state.peers.expire_stale_once(), 1);
    assert_eq!(state.peers.island_of(&address), Some("island-kept".into()));
    ws.close(None).await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while state.registry.has_peer(&address) || state.peers.island_of(&address).is_some() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("socket teardown must clear its assignment");
}

#[tokio::test]
async fn an_oversized_assignment_is_counted_without_breaking_the_live_socket() {
    let (port, state, _) = start_server(10_000).await;
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    let device = Device::new();
    let mut ws = handshake(port, &wallet, &device).await;
    let session = device.session();
    let oversized = IslandChangedMessage {
        conn_str: "x".repeat(catalyst::comms::archipelago::registry::MAX_ASSIGNMENT_BYTES),
        ..island("island-oversized")
    };
    assert_eq!(
        route_island(&state, &address, &session, &oversized),
        Routed::TooLarge
    );
    assert_eq!(state.feed.assignment_oversized_count(), 1);
    assert_eq!(
        route_island(&state, &address, &session, &island("island-valid")),
        Routed::Delivered
    );
    assert_eq!(
        recv_island(&mut ws, Duration::from_secs(3)).await,
        Some("island-valid".into())
    );
}

#[tokio::test]
async fn an_assignment_backlog_keeps_only_the_latest_pending_room() {
    let (port, state, _) = start_server(0).await;
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    let device = Device::new();
    let mut ws = handshake(port, &wallet, &device).await;
    let session = device.session();
    for index in 0..1_024 {
        route_island(
            &state,
            &address,
            &session,
            &island(&format!("island-{index}")),
        );
    }
    assert_eq!(
        recv_island(&mut ws, Duration::from_secs(3)).await,
        Some("island-1023".into()),
        "a paused consumer must not replay an unbounded history of old rooms"
    );
    assert_eq!(recv_island(&mut ws, Duration::from_millis(100)).await, None);
    assert_eq!(state.feed.coalesced_count(), 1_023);
    assert_eq!(state.feed.socket_assignments_written_count(), 1);
}

#[tokio::test]
async fn a_kick_preempts_the_assignment_backlog_and_rejects_later_updates() {
    let (port, state, _) = start_server(0).await;
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    let device = Device::new();
    let mut ws = handshake(port, &wallet, &device).await;
    let session = device.session();
    for index in 0..1_024 {
        route_island(
            &state,
            &address,
            &session,
            &island(&format!("island-{index}")),
        );
    }
    assert_eq!(state.registry.kick(&address), 1);
    assert_eq!(
        route_island(&state, &address, &session, &island("too-late")),
        Routed::NoSocket
    );
    assert!(
        matches!(
            recv_msg(&mut ws, Duration::from_secs(3)).await,
            Some(server_packet::Message::Kicked(_))
        ),
        "a kick must take precedence over queued assignment work"
    );
    tokio::time::timeout(Duration::from_secs(3), async {
        while state.registry.has_peer(&address) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the kicked socket did not retire its registration");
}

#[tokio::test]
async fn an_oversized_client_frame_retires_only_its_socket() {
    let (port, state, _) = start_server(0).await;
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    let first_device = Device::new();
    let second_device = Device::new();
    let mut first = handshake(port, &wallet, &first_device).await;
    let mut second = handshake(port, &wallet, &second_device).await;
    first
        .send(WsMessage::Binary(
            vec![0; catalyst::comms::archipelago::registry::MAX_ASSIGNMENT_BYTES + 1].into(),
        ))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while state
            .registry
            .peer_link(&address, &first_device.session())
            .is_some()
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("oversized frame did not retire its socket");
    assert!(
        state
            .registry
            .peer_link(&address, &second_device.session())
            .is_some()
    );
    assert_eq!(
        route_island(
            &state,
            &address,
            &second_device.session(),
            &island("still-live")
        ),
        Routed::Delivered
    );
    assert_eq!(
        recv_island(&mut second, Duration::from_secs(3)).await,
        Some("still-live".into())
    );
}

#[tokio::test]
async fn a_non_reading_socket_is_retired_on_write_timeout_or_kick() {
    for kick in [false, true] {
        let (port, state, _) = start_server(0).await;
        let wallet = PrivateKeySigner::random();
        let address = format!("{:#x}", wallet.address());
        let device = Device::new();
        let tcp = tokio::net::TcpSocket::new_v4().unwrap();
        tcp.set_recv_buffer_size(2_048).unwrap();
        let tcp = tcp.connect(([127, 0, 0, 1], port).into()).await.unwrap();
        let (mut ws, _) = tokio_tungstenite::client_async(
            format!("ws://127.0.0.1:{port}/ws"),
            tokio_tungstenite::MaybeTlsStream::Plain(tcp),
        )
        .await
        .unwrap();
        ws.send(WsMessage::Binary(encode(
            client_packet::Message::ChallengeRequest(ChallengeRequestMessage {
                address: address.clone(),
            }),
        )))
        .await
        .unwrap();
        let challenge = match recv_msg(&mut ws, Duration::from_secs(3)).await {
            Some(server_packet::Message::ChallengeResponse(response)) => response.challenge_to_sign,
            _ => panic!("expected challenge"),
        };
        finish_handshake(&mut ws, &wallet, &device, &challenge).await;
        let link = state
            .registry
            .peer_link(&address, &device.session())
            .unwrap();
        let session = device.session();
        let mut previous = 0;
        let mut unchanged = 0;
        let mut blocked = false;
        // Keep the actual TCP socket alive, but stop polling its WebSocket reader.
        for index in 0..1_024 {
            let assignment = IslandChangedMessage {
                assignment_context: None,
                island_id: format!("room-{index}"),
                conn_str: "x".repeat(48 * 1024),
                ..Default::default()
            };
            route_island(&state, &address, &session, &assignment);
            tokio::time::sleep(Duration::from_millis(2)).await;
            let written = state.feed.socket_assignments_written_count();
            if written == previous && written > 0 {
                unchanged += 1;
            } else {
                unchanged = 0;
            }
            previous = written;
            if unchanged == 20 {
                blocked = true;
                break;
            }
        }
        assert!(blocked, "fixture never exerted TCP write backpressure");
        assert!(!link.is_closed());
        if kick {
            assert_eq!(state.registry.kick(&address), 1);
        }
        let budget = if kick {
            Duration::from_secs(1)
        } else {
            Duration::from_secs(7)
        };
        tokio::time::timeout(budget, async {
            while state.registry.has_peer(&address) {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("a non-reading socket held its registration after retirement");
        assert!(link.is_closed());
        assert!(state.peers.island_of(&address).is_none());
        assert_eq!(state.feed.socket_write_timeout_count(), u64::from(!kick));
        drop(ws);
    }
}

/// Over a live broker the connector's announces reach the island subscriber by call, and the
/// subscriber's lookups are what crosses the broker: an observer subscribed to
/// `peer.{wallet}.cluster_lookup` stands in for Pulse and never answers.
#[tokio::test]
async fn an_assigned_socket_recovers_a_lost_move_and_reannounces_after_broker_restart() {
    let Some(binary) = gate::require_env("ARCHIPELAGO_NATS_SERVER_BIN") else {
        return;
    };
    struct Task(tokio::task::JoinHandle<()>);
    impl Drop for Task {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    async fn next_lookup(sub: &mut async_nats::Subscriber) -> async_nats::Message {
        tokio::time::timeout(Duration::from_secs(40), sub.next())
            .await
            .expect("an assigned socket was never reconciled")
            .expect("lookup subscription ended")
    }
    async fn wait_until(mut condition: impl FnMut() -> bool) {
        tokio::time::timeout(Duration::from_secs(12), async {
            while !condition() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("broker state did not converge");
    }

    let broker = nats_broker::Broker::start(&binary, None);
    let broker_port = broker.port;
    let cfg = test_config(0);
    let bus: Arc<dyn NatsBus> = Arc::new(BrokerBus::new(Some(broker.url()), "catalyst-comms"));
    let jobs = catalyst::jobs::Jobs::new();
    let state = catalyst::comms::archipelago::build_state(
        cfg,
        archipelago::gatekeeper(Some(bus.clone())),
        &jobs,
    )
    .await
    .expect("state");
    archipelago::start_island_feed(&state, &jobs);
    assert!(state.gatekeeper.bus.is_enabled());
    wait_until(|| bus.is_ready()).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = catalyst::comms::archipelago::mounts(state.clone(), true).stats;
    let _server = Task(tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    }));
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    let device = Device::new();
    let observer = async_nats::connect(broker.url()).await.unwrap();
    let mut lookups = observer
        .subscribe(format!("peer.{address}.cluster_lookup"))
        .await
        .unwrap();
    observer.flush().await.unwrap();
    let mut ws = handshake(port, &wallet, &device).await;
    assert_eq!(
        next_lookup(&mut lookups).await.payload.as_ref(),
        device.session().as_bytes()
    );
    let link = state
        .registry
        .peer_link(&address, &device.session())
        .unwrap();
    state.deliver_island(&address, &device.session(), &island("before-move"), None);
    assert_eq!(
        recv_island(&mut ws, Duration::from_secs(3)).await,
        Some("before-move".into())
    );
    assert!(
        !link.needs_assignment(),
        "the first assignment reached the wire"
    );
    // Readiness can race authentication. Silence across two reconciliation ticks ends the
    // bounded startup burst without assuming how many of its passes observed this socket.
    let mut startup_announcements = 0;
    loop {
        let message = match tokio::time::timeout(Duration::from_secs(11), lookups.next()).await {
            Ok(Some(message)) => message,
            Ok(None) => panic!("lookup subscription ended during startup restoration"),
            Err(_) => break,
        };
        assert_eq!(message.payload.as_ref(), device.session().as_bytes());
        startup_announcements += 1;
        assert!(
            startup_announcements <= 3,
            "startup restoration exceeded its immediate pass and two followups"
        );
    }
    assert!(
        tokio::time::timeout(Duration::from_secs(6), lookups.next())
            .await
            .is_err(),
        "assigned sockets should not use the five-second initial-assignment retry"
    );

    // The source moves, but its edge-triggered assignment is lost. Only a new
    // current-session announcement can ask the downstream pipeline to recover it.
    assert_eq!(
        next_lookup(&mut lookups).await.payload.as_ref(),
        device.session().as_bytes()
    );
    assert!(bus.is_ready());
    assert_eq!(
        state
            .registry
            .peer_link(&address, &device.session())
            .unwrap()
            .id,
        link.id,
        "recovery must not require replacing the authenticated socket"
    );
    state.deliver_island(&address, &device.session(), &island("after-move"), None);
    assert_eq!(
        recv_island(&mut ws, Duration::from_secs(3)).await,
        Some("after-move".into())
    );
    drop(broker);
    wait_until(|| !bus.is_ready()).await;
    let _restarted = nats_broker::Broker::start(&binary, Some(broker_port));
    wait_until(|| bus.is_ready()).await;
    let message = tokio::time::timeout(Duration::from_secs(12), lookups.next())
        .await
        .expect("broker restoration did not reannounce an already-assigned socket")
        .unwrap();
    assert_eq!(message.payload.as_ref(), device.session().as_bytes());
    assert!(!link.needs_assignment());
    ws.close(None).await.unwrap();
    jobs.shutdown(Duration::from_secs(5)).await;
}

#[tokio::test]
async fn same_room_refreshes_deliver_the_entire_assignment_to_only_the_named_session() {
    let (port, state, _) = start_server(10_000).await;
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    let desktop = Device::new();
    let mobile = Device::new();
    let mut first = handshake(port, &wallet, &desktop).await;
    let mut second = handshake(port, &wallet, &mobile).await;
    let mut assignment = island("same-room");
    for step in 0..5 {
        match step {
            1 => {
                assignment.conn_str = "livekit:wss://example.invalid?access_token=refreshed".into()
            }
            2 => assignment.from_island_id = Some("previous-room".into()),
            3 => {
                assignment.peers.insert(
                    "peer".into(),
                    catalyst::comms::protocol::archipelago::Position {
                        x: 1.0,
                        y: 2.0,
                        z: 3.0,
                    },
                );
            }
            4 => assignment.assignment_context = Some(Default::default()),
            _ => {}
        }
        assert_eq!(
            route_island(&state, &address, &desktop.session(), &assignment),
            Routed::Delivered
        );
        assert_eq!(
            recv_msg(&mut first, Duration::from_secs(3)).await,
            Some(server_packet::Message::IslandChanged(assignment.clone())),
            "refresh step {step}"
        );
        assert_eq!(recv_msg(&mut second, Duration::from_millis(50)).await, None);
        route_island(&state, &address, &desktop.session(), &assignment);
        assert_eq!(recv_msg(&mut first, Duration::from_millis(50)).await, None);
    }
    // A second session of the same wallet starts with its own delivery state.
    route_island(&state, &address, &mobile.session(), &assignment);
    assert_eq!(
        recv_msg(&mut second, Duration::from_secs(3)).await,
        Some(server_packet::Message::IslandChanged(assignment))
    );
}

#[tokio::test]
async fn an_expired_window_delivers_the_identical_assignment_again() {
    let (port, state, _) = start_server(50).await;
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    let device = Device::new();
    let mut ws = handshake(port, &wallet, &device).await;
    let assignment = island("same-room");
    for _ in 0..2 {
        route_island(&state, &address, &device.session(), &assignment);
        assert_eq!(
            recv_msg(&mut ws, Duration::from_secs(3)).await,
            Some(server_packet::Message::IslandChanged(assignment.clone()))
        );
        tokio::time::sleep(Duration::from_millis(75)).await;
    }
}

#[tokio::test]
async fn replacement_socket_has_no_predecessor_delivery_state_or_delayed_predecessor_updates() {
    use catalyst::comms::archipelago::registry::SocketEvent;
    let (port, state, _) = start_server(10_000).await;
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    let device = Device::new();
    let mut old = handshake(port, &wallet, &device).await;
    let predecessor = state
        .registry
        .peer_link(&address, &device.session())
        .unwrap();
    let assignment = island("same-room");
    route_island(&state, &address, &device.session(), &assignment);
    assert_eq!(
        recv_msg(&mut old, Duration::from_secs(3)).await,
        Some(server_packet::Message::IslandChanged(assignment.clone()))
    );
    let mut fresh = handshake(port, &wallet, &device).await;
    assert!(matches!(
        recv_msg(&mut old, Duration::from_secs(3)).await,
        Some(server_packet::Message::Kicked(_))
    ));
    let mut delayed = assignment.clone();
    delayed.conn_str = "stale-predecessor-token".into();
    assert!(
        !predecessor.send(SocketEvent::IslandChanged(delayed.encode_to_vec(), None)),
        "a closed predecessor must reject even a changed assignment"
    );
    assert_eq!(recv_msg(&mut fresh, Duration::from_millis(100)).await, None);
    assert_eq!(state.feed.socket_assignments_written_count(), 1);
    route_island(&state, &address, &device.session(), &assignment);
    assert_eq!(
        recv_msg(&mut fresh, Duration::from_secs(3)).await,
        Some(server_packet::Message::IslandChanged(assignment))
    );
    assert_eq!(state.feed.socket_assignments_written_count(), 2);
}
