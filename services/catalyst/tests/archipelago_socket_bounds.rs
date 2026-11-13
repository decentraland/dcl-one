//! Runtime socket limits over real local WebSockets. No external database/broker required.
#[path = "support/archipelago.rs"]
mod archipelago;
#[path = "support/nats_broker.rs"]
mod nats_broker;
use alloy::signers::{SignerSync, local::PrivateKeySigner};
use catalyst::comms::archipelago::{AppState, config::Settings};
use catalyst::comms::protocol::archipelago::{
    Position,
    archipelago::{
        ChallengeRequestMessage, ClientPacket, Heartbeat, ServerPacket, SignedChallengeMessage,
        client_packet, server_packet,
    },
};
use futures::{SinkExt, StreamExt};
use prost::Message as _;
use std::time::Duration;
use tokio_tungstenite::tungstenite::{Error, Message};
type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;
async fn server(cap: u32) -> (u16, AppState) {
    let mut settings = Settings::for_tests();
    settings.server.max_sockets = cap;
    let state = archipelago::state(settings).await;
    let mounts = catalyst::comms::archipelago::mounts(
        state.clone(),
        true,
        Default::default(),
        catalyst::comms::stats::http::test_support::matcher(),
        std::sync::Arc::new(catalyst::content::Denylist::default()),
    );
    let app = axum::Router::new()
        .merge(mounts.root)
        .nest("/archipelago-ea-stats", mounts.stats)
        .nest("/archipelago-ea-ws-connector", mounts.connector);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        use catalyst::comms::archipelago::ws::raw_transport;
        let app = app.layer(axum::middleware::from_fn(raw_transport::connection_info));
        axum::serve(
            raw_transport::Listener(listener),
            app.into_make_service_with_connect_info::<raw_transport::Connection>(),
        )
        .await
        .unwrap();
    });
    (port, state)
}
async fn wait(mut condition: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}
fn packet(message: client_packet::Message) -> Message {
    Message::Binary(
        ClientPacket {
            message: Some(message),
            request_sequence: 0,
        }
        .encode_to_vec()
        .into(),
    )
}
async fn receive(socket: &mut Socket) -> server_packet::Message {
    loop {
        if let Message::Binary(bytes) = tokio::time::timeout(Duration::from_secs(3), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap()
        {
            return ServerPacket::decode(bytes).unwrap().message.unwrap();
        }
    }
}
async fn authenticated(port: u16) -> (Socket, String) {
    let (socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws"))
        .await
        .unwrap();
    authenticate(
        socket,
        PrivateKeySigner::random(),
        PrivateKeySigner::random(),
    )
    .await
}
async fn authenticate(
    socket: Socket,
    wallet: PrivateKeySigner,
    device: PrivateKeySigner,
) -> (Socket, String) {
    let claim = format!("{:#x}", wallet.address());
    authenticate_claim(socket, wallet, Some(device), claim).await
}

async fn authenticate_claim(
    mut socket: Socket,
    wallet: PrivateKeySigner,
    device: Option<PrivateKeySigner>,
    claim: String,
) -> (Socket, String) {
    let address = format!("{:#x}", wallet.address());
    socket
        .send(packet(client_packet::Message::ChallengeRequest(
            ChallengeRequestMessage { address: claim },
        )))
        .await
        .unwrap();
    let server_packet::Message::ChallengeResponse(response) = receive(&mut socket).await else {
        panic!("challenge expected")
    };
    send_eoa_chain(
        &mut socket,
        &wallet,
        device.as_ref(),
        &response.challenge_to_sign,
    )
    .await;
    let server_packet::Message::Welcome(welcome) = receive(&mut socket).await else {
        panic!("welcome expected")
    };
    assert_eq!(welcome.peer_id, address);
    (socket, address)
}

async fn send_eoa_chain(
    socket: &mut Socket,
    wallet: &PrivateKeySigner,
    device: Option<&PrivateKeySigner>,
    challenge: &str,
) {
    let address = format!("{:#x}", wallet.address());
    let mandate = format!(
        "Decentraland Login\nEphemeral address: {:#x}\nExpiration: 2099-12-31T00:00:00.000Z",
        device.unwrap_or(wallet).address()
    );
    let sign = |key: &PrivateKeySigner, message: &str| {
        key.sign_hash_sync(&alloy::primitives::eip191_hash_message(message.as_bytes()))
            .unwrap()
            .to_string()
    };
    let mut chain =
        vec![serde_json::json!({ "type": "SIGNER", "payload": address, "signature": "" })];
    if device.is_some() {
        chain.push(serde_json::json!({ "type": "ECDSA_EPHEMERAL", "payload": mandate, "signature": sign(&wallet, &mandate) }));
    }
    chain.push(serde_json::json!({ "type": "ECDSA_SIGNED_ENTITY", "payload": challenge, "signature": sign(device.unwrap_or(wallet), challenge) }));
    socket
        .send(packet(client_packet::Message::SignedChallenge(
            SignedChallengeMessage {
                auth_chain_json: serde_json::to_string(&chain).unwrap(),
            },
        )))
        .await
        .unwrap();
}

#[tokio::test]
async fn verified_principal_overrides_claim_for_direct_and_delegated_eoa() {
    for delegated in [false, true] {
        let (port, state) = server(4).await;
        let claim = format!("{:#x}", PrivateKeySigner::random().address());
        let wallet = PrivateKeySigner::random();
        let device = delegated.then(PrivateKeySigner::random);
        let session = format!("{:#x}", device.as_ref().unwrap_or(&wallet).address());
        let (socket, _) = tokio_tungstenite::connect_async(format!(
            "ws://127.0.0.1:{port}/archipelago-ea-ws-connector/ws"
        ))
        .await
        .unwrap();
        let (mut socket, address) = authenticate_claim(socket, wallet, device, claim.clone()).await;
        assert!(!state.registry.has_any_peer(&claim));
        assert!(state.registry.peer_link(&address, &session).is_some());
        socket.close(None).await.unwrap();
        wait(|| !state.registry.has_any_peer(&address)).await;
        catalyst::comms::archipelago::shutdown(&state).await;
    }
}

async fn challenge_as(
    socket: &mut Socket,
    claim: &str,
) -> catalyst::comms::protocol::archipelago::archipelago::ChallengeResponseMessage {
    socket
        .send(packet(client_packet::Message::ChallengeRequest(
            ChallengeRequestMessage {
                address: claim.into(),
            },
        )))
        .await
        .unwrap();
    let server_packet::Message::ChallengeResponse(response) = receive(socket).await else {
        panic!("challenge expected")
    };
    response
}

#[tokio::test]
async fn already_connected_observes_claim_without_authorizing_it() {
    for claim_connected in [false, true] {
        let (port, state) = server(4).await;
        let a = PrivateKeySigner::random();
        let b = PrivateKeySigner::random();
        let address_a = format!("{:#x}", a.address());
        let address_b = format!("{:#x}", b.address());
        let (socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws"))
            .await
            .unwrap();
        let (mut existing, _) = authenticate(
            socket,
            if claim_connected { a } else { b.clone() },
            PrivateKeySigner::random(),
        )
        .await;
        let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws"))
            .await
            .unwrap();
        let response = challenge_as(&mut socket, &address_a).await;
        assert_eq!(response.already_connected, claim_connected);
        send_eoa_chain(&mut socket, &b, None, &response.challenge_to_sign).await;
        let server_packet::Message::Welcome(welcome) = receive(&mut socket).await else {
            panic!("welcome expected")
        };
        assert_eq!(welcome.peer_id, address_b);
        assert_eq!(
            state.registry.sessions_of(&address_a).len(),
            usize::from(claim_connected)
        );
        assert_eq!(
            state.registry.sessions_of(&address_b).len(),
            if claim_connected { 1 } else { 2 }
        );
        socket.close(None).await.unwrap();
        existing.close(None).await.unwrap();
        wait(|| state.registry.authenticated_count() == 0).await;
        catalyst::comms::archipelago::shutdown(&state).await;
    }
}

#[tokio::test]
async fn local_reconnect_replaces_only_the_verified_device_and_moves_it_newest() {
    let (port, state) = server(4).await;
    let wallet = PrivateKeySigner::random();
    let devices = [PrivateKeySigner::random(), PrivateKeySigner::random()];
    let address = format!("{:#x}", wallet.address());
    let mut sockets = Vec::new();
    for device in [&devices[0], &devices[1], &devices[0]] {
        let (socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws"))
            .await
            .unwrap();
        sockets.push(
            authenticate_claim(
                socket,
                wallet.clone(),
                Some(device.clone()),
                format!("{:#x}", PrivateKeySigner::random().address()),
            )
            .await
            .0,
        );
    }
    let sessions = state.registry.sessions_of(&address);
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].session, format!("{:#x}", devices[1].address()));
    assert_eq!(sessions[1].session, format!("{:#x}", devices[0].address()));
    let _ = sockets[0].close(None).await;
    assert_eq!(
        state
            .registry
            .sessions_of(&address)
            .iter()
            .map(|peer| peer.id)
            .collect::<Vec<_>>(),
        sessions.iter().map(|peer| peer.id).collect::<Vec<_>>()
    );
    for socket in &mut sockets[1..] {
        socket.close(None).await.unwrap();
    }
    wait(|| !state.registry.has_any_peer(&address)).await;
    catalyst::comms::archipelago::shutdown(&state).await;
}

#[tokio::test]
async fn another_sockets_nonce_cannot_redeem_or_discard_the_live_issuance() {
    let (port, state) = server(4).await;
    let wallet = PrivateKeySigner::random();
    let principal = format!("{:#x}", wallet.address());
    let claim = format!("{:#x}", PrivateKeySigner::random().address());
    let (mut first, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws"))
        .await
        .unwrap();
    let (mut other, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws"))
        .await
        .unwrap();
    let first_nonce = challenge_as(&mut first, &claim).await.challenge_to_sign;
    let other_nonce = challenge_as(&mut other, &claim).await.challenge_to_sign;
    assert_ne!(first_nonce, other_nonce);
    send_eoa_chain(&mut other, &wallet, None, &first_nonce).await;
    no_application_data(&mut other).await;
    assert_eq!(state.registry.authenticated_count(), 0);
    send_eoa_chain(&mut first, &wallet, None, &first_nonce).await;
    let server_packet::Message::Welcome(welcome) = receive(&mut first).await else {
        panic!("live issuance welcome expected")
    };
    assert_eq!(welcome.peer_id, principal);
    first.close(None).await.unwrap();
    wait(|| state.registry.authenticated_count() == 0 && state.challenges.pending() == 0).await;
    catalyst::comms::archipelago::shutdown(&state).await;
}

struct PolicyBan {
    entered: tokio::sync::Semaphore,
    release: tokio::sync::Semaphore,
    actors: std::sync::Mutex<Vec<String>>,
    banned: bool,
}

// Defer feed wiring until the fixture has installed its controlled policy dependencies.
#[derive(Default)]
struct PolicyBus {
    enabled: std::sync::atomic::AtomicBool,
    bus: catalyst::comms::gatekeeper::bus::InProcessBus,
}
#[async_trait::async_trait]
impl catalyst::comms::gatekeeper::bus::NatsBus for PolicyBus {
    async fn statistics_control(
        &self,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, catalyst::comms::gatekeeper::bus::RequestError> {
        self.bus.statistics_control(payload).await
    }
    fn is_enabled(&self) -> bool {
        self.enabled.load(std::sync::atomic::Ordering::SeqCst)
    }
    fn is_connected(&self) -> bool {
        true
    }
    fn is_ready(&self) -> bool {
        true
    }
    fn connect(&self) {}
    fn subscribe(
        &self,
        subject: &str,
        queue: Option<&str>,
        handler: catalyst::comms::gatekeeper::bus::NatsHandler,
    ) -> catalyst::comms::gatekeeper::bus::NatsSubscription {
        self.bus.subscribe(subject, queue, handler)
    }
    async fn publish(
        &self,
        subject: &str,
        payload: Vec<u8>,
    ) -> catalyst::comms::gatekeeper::bus::PublishOutcome {
        self.bus.publish(subject, payload).await
    }
    async fn request(
        &self,
        subject: &str,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, catalyst::comms::gatekeeper::bus::RequestError> {
        self.bus.request(subject, payload).await
    }
    async fn flush(&self) -> bool {
        true
    }
}
impl PolicyBus {
    fn published(&self) -> Vec<(String, Vec<u8>)> {
        self.bus.published()
    }
    fn requested(&self) -> Vec<(String, Vec<u8>)> {
        self.bus.requested()
    }
}
#[async_trait::async_trait]
impl catalyst::comms::gatekeeper::moderation::BanStatus for PolicyBan {
    async fn ban(
        &self,
        actor: &catalyst::comms::Address,
        _: Option<&catalyst::comms::world_identity::SceneId>,
        _: Option<&str>,
        _: chrono::NaiveDateTime,
    ) -> Result<Option<catalyst::comms::access::Ban>, catalyst::comms::AuthorityNotEstablished>
    {
        self.actors.lock().unwrap().push(actor.as_str().into());
        self.entered.add_permits(1);
        self.release.acquire().await.unwrap().forget();
        Ok(self
            .banned
            .then_some(catalyst::comms::access::Ban::Platform))
    }
}

async fn no_application_data(socket: &mut Socket) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while let Some(Ok(message)) = socket.next().await {
            assert!(
                !matches!(message, Message::Binary(_)),
                "denied/cancelled handshake published application data"
            );
            if matches!(message, Message::Close(_)) {
                break;
            }
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn verified_policy_waits_cancel_on_close_restart_and_deadline() {
    use std::sync::Arc;
    for policy in ["initial-deny", "verified-deny", "ban"] {
        for cancellation in ["close", "restart", "deadline"] {
            let entered = Arc::new(tokio::sync::Semaphore::new(0));
            let release = Arc::new(tokio::sync::Semaphore::new(0));
            let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let deny_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let deny_url = format!("http://{}/deny", deny_listener.local_addr().unwrap());
            let (hit, gate, count) = (entered.clone(), release.clone(), calls.clone());
            let app = axum::Router::new().route(
                "/deny",
                axum::routing::get(move || {
                    let (hit, gate, count) = (hit.clone(), gate.clone(), count.clone());
                    async move {
                        let index = count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        if policy == "initial-deny" || (policy == "verified-deny" && index == 1) {
                            hit.add_permits(1);
                            gate.acquire().await.unwrap().forget();
                        }
                        axum::Json(serde_json::json!({"users":[]}))
                    }
                }),
            );
            let deny_task =
                tokio::spawn(async move { axum::serve(deny_listener, app).await.unwrap() });
            let bans = Arc::new(PolicyBan {
                entered: tokio::sync::Semaphore::new(0),
                release: tokio::sync::Semaphore::new(0),
                actors: Default::default(),
                banned: false,
            });
            let bus = Arc::new(PolicyBus::default());
            let mut settings = Settings::for_tests();
            settings.auth.handshake_timeout_ms = if cancellation == "deadline" {
                750
            } else {
                3_000
            };
            let mut state =
                archipelago::build(settings, archipelago::gatekeeper(Some(bus.clone()))).await;
            let inner = Arc::get_mut(&mut state).unwrap();
            if policy != "ban" {
                inner.deny_list = catalyst::comms::deny_list::DenyList::with_ttl(
                    Some(deny_url),
                    catalyst::http::Outbound::for_tests().allowing_private_hosts(),
                    Duration::ZERO,
                );
            }
            inner.ban_checker = catalyst::comms::archipelago::ban::BanChecker::new(
                archipelago::allow_policy(),
                bans.clone(),
            );
            bus.enabled.store(true, std::sync::atomic::Ordering::SeqCst);
            let port = archipelago::serve(state.clone()).await;
            let (mut socket, _) =
                tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws"))
                    .await
                    .unwrap();
            let wallet = PrivateKeySigner::random();
            let claim = format!("{:#x}", PrivateKeySigner::random().address());
            if policy == "initial-deny" {
                socket
                    .send(packet(client_packet::Message::ChallengeRequest(
                        ChallengeRequestMessage {
                            address: claim.clone(),
                        },
                    )))
                    .await
                    .unwrap();
            } else {
                let response = challenge_as(&mut socket, &claim).await;
                send_eoa_chain(&mut socket, &wallet, None, &response.challenge_to_sign).await;
            }
            let barrier = if policy == "ban" {
                &bans.entered
            } else {
                entered.as_ref()
            };
            tokio::time::timeout(Duration::from_secs(2), barrier.acquire())
                .await
                .unwrap()
                .unwrap()
                .forget();
            assert_eq!(state.registry.authenticated_count(), 0);
            match cancellation {
                "close" => {
                    let (bytes, ended) = raw_peer_close(&mut socket, "pending-policy").await;
                    assert!(ended.is_ok());
                    assert_eq!(&bytes[2..], b"\x03\xe8pending-policy");
                }
                "restart" => state.restart.cancel(),
                _ => {}
            }
            // Cancellation must finish without completing the held policy operation.
            wait(|| {
                state.handlers.available_permits() == state.settings.server.max_sockets as usize
            })
            .await;
            release.add_permits(2);
            bans.release.add_permits(1);
            assert_eq!(state.registry.authenticated_count(), 0);
            assert_eq!(state.challenges.pending(), 0);
            assert!(
                !state
                    .enforcement
                    .island_of(&format!("{:#x}", wallet.address()))
                    .is_some()
            );
            assert!(bus.published().is_empty());
            assert!(bus.requested().is_empty());
            if policy == "ban" {
                assert_eq!(
                    *bans.actors.lock().unwrap(),
                    [format!("{:#x}", wallet.address())]
                );
            }
            if cancellation != "close" {
                no_application_data(&mut socket).await;
            }
            catalyst::comms::archipelago::shutdown(&state).await;
            deny_task.abort();
        }
    }
}

#[tokio::test]
async fn claimed_deny_and_verified_deny_or_ban_fail_before_admission() {
    use std::sync::Arc;
    for denied in ["claim", "principal", "ban"] {
        let wallet = PrivateKeySigner::random();
        let principal = format!("{:#x}", wallet.address());
        let claim = format!("{:#x}", PrivateKeySigner::random().address());
        let denied_wallet = if denied == "claim" {
            claim.clone()
        } else if denied == "principal" {
            principal.clone()
        } else {
            String::new()
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let deny_url = format!("http://{}/deny", listener.local_addr().unwrap());
        let app = axum::Router::new().route(
            "/deny",
            axum::routing::get(move || {
                let wallet = denied_wallet.clone();
                async move { axum::Json(serde_json::json!({"users":[{"wallet":wallet}]})) }
            }),
        );
        let deny_task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let bans = Arc::new(PolicyBan {
            entered: tokio::sync::Semaphore::new(0),
            release: tokio::sync::Semaphore::new(1),
            actors: Default::default(),
            banned: denied == "ban",
        });
        let bus = Arc::new(PolicyBus::default());
        let mut state = archipelago::build(
            Settings::for_tests(),
            archipelago::gatekeeper(Some(bus.clone())),
        )
        .await;
        let inner = Arc::get_mut(&mut state).unwrap();
        inner.deny_list = catalyst::comms::deny_list::DenyList::new(
            Some(deny_url),
            catalyst::http::Outbound::for_tests().allowing_private_hosts(),
        );
        inner.ban_checker = catalyst::comms::archipelago::ban::BanChecker::new(
            archipelago::allow_policy(),
            bans.clone(),
        );
        bus.enabled.store(true, std::sync::atomic::Ordering::SeqCst);
        let port = archipelago::serve(state.clone()).await;
        let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws"))
            .await
            .unwrap();
        if denied == "claim" {
            socket
                .send(packet(client_packet::Message::ChallengeRequest(
                    ChallengeRequestMessage {
                        address: claim.clone(),
                    },
                )))
                .await
                .unwrap();
        } else {
            let challenge = challenge_as(&mut socket, &claim).await;
            send_eoa_chain(&mut socket, &wallet, None, &challenge.challenge_to_sign).await;
        }
        if denied == "ban" {
            assert!(matches!(
                receive(&mut socket).await,
                server_packet::Message::Kicked(_)
            ));
            assert_eq!(*bans.actors.lock().unwrap(), [principal.clone()]);
        } else {
            assert!(bans.actors.lock().unwrap().is_empty());
        }
        no_application_data(&mut socket).await;
        wait(|| state.handlers.available_permits() == state.settings.server.max_sockets as usize)
            .await;
        assert!(!state.registry.has_any_peer(&claim));
        assert!(!state.registry.has_any_peer(&principal));
        assert_eq!(state.challenges.pending(), 0);
        assert!(bus.published().is_empty());
        assert!(bus.requested().is_empty());
        catalyst::comms::archipelago::shutdown(&state).await;
        deny_task.abort();
    }
}

async fn raw_upgrade(port: u16) -> Socket {
    raw_upgrade_with_prefix(port, &[]).await
}

async fn raw_upgrade_with_prefix(port: u16, prefix: &[u8]) -> Socket {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut tcp = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .unwrap();
    let request = format!(
        "GET /ws HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n"
    );
    eprintln!("CLOSE_WIRE request={request:?}");
    let mut request = request.into_bytes();
    request.extend_from_slice(prefix);
    tcp.write_all(&request).await.unwrap();
    let mut response = vec![];
    tokio::time::timeout(Duration::from_secs(3), async {
        while !response.ends_with(b"\r\n\r\n") {
            response.push(tcp.read_u8().await.unwrap());
            assert!(response.len() < 8192);
        }
    })
    .await
    .unwrap();
    eprintln!(
        "CLOSE_WIRE response={:?}",
        String::from_utf8_lossy(&response)
    );
    assert!(response.starts_with(b"HTTP/1.1 101"));
    tokio_tungstenite::WebSocketStream::from_raw_socket(
        tokio_tungstenite::MaybeTlsStream::Plain(tcp),
        tokio_tungstenite::tungstenite::protocol::Role::Client,
        None,
    )
    .await
}

#[tokio::test]
async fn initial_http_read_ahead_preserves_partial_frame_and_ordinary_challenge() {
    use tokio::io::AsyncWriteExt;
    let (port, state) = server(1).await;
    // The request and first frame byte enter the same HTTP read-ahead buffer.
    let mut socket = raw_upgrade_with_prefix(port, &[0x89]).await;
    let tokio_tungstenite::MaybeTlsStream::Plain(tcp) = socket.get_mut() else {
        panic!("plaintext test transport expected")
    };
    // Complete a masked Ping only after receiving the HTTP upgrade response.
    tcp.write_all(&[0x81, 1, 2, 3, 4, b'p' ^ 1]).await.unwrap();
    assert_eq!(
        socket.next().await.unwrap().unwrap(),
        Message::Pong(b"p".to_vec().into())
    );
    socket
        .send(packet(client_packet::Message::ChallengeRequest(
            ChallengeRequestMessage {
                address: "0x0000000000000000000000000000000000000001".into(),
            },
        )))
        .await
        .unwrap();
    assert!(matches!(
        receive(&mut socket).await,
        server_packet::Message::ChallengeResponse(_)
    ));
    let (bytes, ended) = raw_peer_close(&mut socket, "read-ahead").await;
    assert_eq!(&bytes[..2], &[0x88, 12]);
    assert!(ended.is_ok());
    wait(|| state.sockets.available_permits() == 1).await;
}

// Write the masked control frame directly and keep TCP open until the server ends it.
// A WebSocket client's automatic close machinery would hide both bytes and EOF.
async fn raw_peer_close(
    socket: &mut Socket,
    reason: &str,
) -> (Vec<u8>, Result<(), std::io::ErrorKind>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let tokio_tungstenite::MaybeTlsStream::Plain(tcp) = socket.get_mut() else {
        panic!("loopback plaintext transport expected")
    };
    let start = std::time::Instant::now();
    let mut payload = vec![3, 232];
    payload.extend_from_slice(reason.as_bytes());
    let mut frame = vec![0x88, 0x80 | payload.len() as u8, 1, 2, 3, 4];
    frame.extend(
        payload
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ [1, 2, 3, 4][i % 4]),
    );
    eprintln!(
        "CLOSE_WIRE reason={reason:?} client={frame:02x?} at_us={}",
        start.elapsed().as_micros()
    );
    tcp.write_all(&frame).await.unwrap();
    let mut received = vec![];
    let ended = tokio::time::timeout(Duration::from_secs(7), async {
        loop {
            let mut bytes = [0; 4096];
            match tcp.read(&mut bytes).await {
                Ok(0) => {
                    eprintln!("CLOSE_WIRE eof at_us={}", start.elapsed().as_micros());
                    return Ok(());
                }
                Ok(n) => {
                    eprintln!(
                        "CLOSE_WIRE server={:02x?} at_us={}",
                        &bytes[..n],
                        start.elapsed().as_micros()
                    );
                    received.extend_from_slice(&bytes[..n]);
                }
                Err(error) => {
                    eprintln!(
                        "CLOSE_WIRE error={error} at_us={}",
                        start.elapsed().as_micros()
                    );
                    return Err(error.kind());
                }
            }
        }
    })
    .await
    .unwrap_or_else(|_| {
        eprintln!("CLOSE_WIRE timeout at_us={}", start.elapsed().as_micros());
        Err(std::io::ErrorKind::TimedOut)
    });
    (received, ended)
}

#[tokio::test]
async fn clean_peer_close_echoes_exact_reason_and_ends_tcp_at_each_application_stage() {
    let (port, state) = server(1).await;
    let mut failures = vec![];
    for stage in [
        "",
        "fixture",
        "after-challenge",
        "after-auth",
        "after-assignment",
    ] {
        let mut outstanding_challenge = None;
        let socket = raw_upgrade(port).await;
        let (mut socket, address) = if stage == "after-auth" || stage == "after-assignment" {
            let (socket, address) = authenticate(
                socket,
                "0000000000000000000000000000000000000000000000000000000000000001"
                    .parse()
                    .unwrap(),
                "0000000000000000000000000000000000000000000000000000000000000002"
                    .parse()
                    .unwrap(),
            )
            .await;
            (socket, Some(address))
        } else {
            let mut socket = socket;
            if stage == "after-challenge" {
                socket
                    .send(packet(client_packet::Message::ChallengeRequest(
                        ChallengeRequestMessage {
                            address: "0x0000000000000000000000000000000000000001".into(),
                        },
                    )))
                    .await
                    .unwrap();
                let server_packet::Message::ChallengeResponse(challenge) =
                    receive(&mut socket).await
                else {
                    panic!("challenge expected")
                };
                outstanding_challenge = Some(challenge.challenge_to_sign);
            }
            (socket, None)
        };
        if stage == "after-assignment" {
            let links = state.registry.sessions_of(address.as_ref().unwrap());
            assert_eq!(links.len(), 1);
            let assignment =
                catalyst::comms::protocol::archipelago::archipelago::IslandChangedMessage {
                    island_id: "close-fixture".into(),
                    conn_str: "fixture-credential".into(),
                    ..Default::default()
                };
            assert!(links[0].send(
                catalyst::comms::archipelago::registry::SocketEvent::IslandChanged(
                    assignment.encode_to_vec(),
                    None
                )
            ));
            let server_packet::Message::IslandChanged(received) = receive(&mut socket).await else {
                panic!("complete assignment expected")
            };
            assert_eq!(received, assignment);
            wait(|| !links[0].needs_assignment()).await;
        }
        let (bytes, ended) = raw_peer_close(&mut socket, stage).await;
        let mut expected = vec![0x88, (2 + stage.len()) as u8, 3, 232];
        expected.extend_from_slice(stage.as_bytes());
        if bytes != expected || ended.is_err() {
            failures.push((stage.to_string(), bytes, expected, ended));
        }
        wait(|| state.handlers.available_permits() == 1 && state.sockets.available_permits() == 1)
            .await;
        if let Some(address) = address {
            assert!(!state.registry.has_any_peer(&address));
        }
        if outstanding_challenge.is_some() {
            assert_eq!(state.challenges.pending(), 0);
        }
        assert_eq!(state.registry.authenticated_count(), 0);
    }
    assert!(failures.is_empty(), "exact echo/EOF failures: {failures:?}");
}

#[tokio::test]
async fn server_first_close_does_not_send_a_second_close_for_the_peer_reply() {
    let (port, state) = server(1).await;
    let mut socket = raw_upgrade(port).await;
    socket
        .send(Message::Binary(vec![0xff].into()))
        .await
        .unwrap();
    let close = tokio::time::timeout(Duration::from_secs(3), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(matches!(close, Message::Close(Some(frame)) if u16::from(frame.code) == 1007));
    let (remaining, ended) = raw_peer_close(&mut socket, "server-first-reply").await;
    assert!(remaining.is_empty());
    assert!(
        matches!(
            ended,
            Ok(())
                | Err(std::io::ErrorKind::ConnectionAborted
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::BrokenPipe)
        ),
        "server-first termination: {ended:?}"
    );
    wait(|| state.handlers.available_permits() == 1 && state.sockets.available_permits() == 1)
        .await;
}

struct ContractValidator {
    entered: tokio::sync::Semaphore,
    release: tokio::sync::Semaphore,
    active: std::sync::atomic::AtomicUsize,
}

/// Latest refuses a rotated signature while the selected verification-time block accepts it.
#[tokio::test]
async fn contract_fallback_accepts_verified_principal_through_configured_rpc() {
    use std::sync::{Arc, Mutex};
    let calls = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
    let recorded = calls.clone();
    let rpc = axum::Router::new().route(
        "/",
        axum::routing::post(move |axum::Json(body): axum::Json<serde_json::Value>| {
            let recorded = recorded.clone();
            async move {
                recorded.lock().unwrap().push(body.clone());
                let result = match body["method"].as_str().unwrap() {
                    "eth_call" if body["params"][1] == "latest" => {
                        serde_json::json!(format!("0x{}", "0".repeat(64)))
                    }
                    "eth_call" => serde_json::json!(format!("0x1626ba7e{}", "0".repeat(56))),
                    "eth_getBlockByNumber" => {
                        serde_json::json!({"number":"0xa", "timestamp":"0x14"})
                    }
                    "eth_blockNumber" => serde_json::json!("0xb"),
                    method => panic!("unexpected RPC method {method}"),
                };
                axum::Json(serde_json::json!({"jsonrpc":"2.0", "id":body["id"], "result":result}))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let rpc_task = tokio::spawn(async move { axum::serve(listener, rpc).await.unwrap() });
    let mut gatekeeper = archipelago::gatekeeper(None);
    Arc::get_mut(&mut gatekeeper).unwrap().eip1654 =
        Some(Arc::new(catalyst::auth::ValidationCache::new(Arc::new(
            catalyst::auth::ValidationCache::new(Arc::new(
                catalyst::auth::RpcEip1654Validator::new(url, reqwest::Client::new()),
            )),
        ))));
    let state = archipelago::build(Settings::for_tests(), gatekeeper).await;
    let port = archipelago::serve(state.clone()).await;
    let claim = format!("{:#x}", PrivateKeySigner::random().address());
    let (mut socket, principal, session, challenge, _) =
        pending_contract_as(port, false, claim.clone(), PrivateKeySigner::random()).await;
    let server_packet::Message::Welcome(welcome) = receive(&mut socket).await else {
        panic!("historical signature must welcome")
    };
    assert_eq!(welcome.peer_id, principal);
    assert_eq!(session, principal);
    assert!(!state.registry.has_any_peer(&claim));
    assert!(state.registry.has_any_peer(&principal));
    let calls = calls.lock().unwrap().clone();
    assert_eq!(
        calls
            .iter()
            .map(|v| v["method"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "eth_call",
            "eth_call",
            "eth_getBlockByNumber",
            "eth_blockNumber",
            "eth_getBlockByNumber",
            "eth_blockNumber",
            "eth_call"
        ]
    );
    for (index, hash) in [
        alloy::primitives::keccak256(challenge.as_bytes()),
        alloy::primitives::eip191_hash_message(challenge.as_bytes()),
    ]
    .iter()
    .enumerate()
    {
        let expected = catalyst::auth::rpc_validator::encode_is_valid_signature(
            hash.as_slice(),
            &[0x12, 0x34],
        );
        assert_eq!(
            calls[index]["params"],
            serde_json::json!([{"to":principal,"data":format!("0x{}", hex::encode(expected))},"latest"])
        );
    }
    assert_eq!(calls[6]["params"][1], "0xb");
    assert_eq!(calls[6]["params"][0], calls[0]["params"][0]);
    socket.close(None).await.unwrap();
    wait(|| state.challenges.pending() == 0 && !state.registry.has_any_peer(&principal)).await;
    rpc_task.abort();
}

#[tokio::test]
async fn every_fallback_rpc_phase_cancels_without_admission_or_a_following_request() {
    use std::sync::{Arc, Mutex};
    for hold in 0..8 {
        for cancellation in ["close", "restart", "deadline"] {
            let calls = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
            let entered = Arc::new(tokio::sync::Semaphore::new(0));
            let release = Arc::new(tokio::sync::Semaphore::new(0));
            let (recorded, signal, gate) = (calls.clone(), entered.clone(), release.clone());
            let rpc = axum::Router::new().route(
                "/",
                axum::routing::post(move |axum::Json(body): axum::Json<serde_json::Value>| {
                    let (calls, signal, gate) = (recorded.clone(), signal.clone(), gate.clone());
                    async move {
                        let index = {
                            let mut calls = calls.lock().unwrap();
                            let n = calls.len();
                            calls.push(body.clone());
                            n
                        };
                        if index == hold {
                            signal.add_permits(1);
                            gate.acquire().await.unwrap().forget();
                        }
                        let result = match body["method"].as_str().unwrap() {
                            "eth_call" => serde_json::json!(if index == 7 {
                                format!("0x1626ba7e{}", "0".repeat(56))
                            } else {
                                format!("0x{}", "0".repeat(64))
                            }),
                            "eth_getBlockByNumber" => {
                                serde_json::json!({"number":"0xa","timestamp":"0x14"})
                            }
                            "eth_blockNumber" => serde_json::json!("0xb"),
                            method => panic!("unexpected method {method}"),
                        };
                        axum::Json(
                            serde_json::json!({"jsonrpc":"2.0","id":body["id"],"result":result}),
                        )
                    }
                }),
            );
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}/", listener.local_addr().unwrap());
            let rpc_task = tokio::spawn(async move { axum::serve(listener, rpc).await.unwrap() });
            let bus = Arc::new(catalyst::comms::gatekeeper::bus::InProcessBus::new());
            let mut gatekeeper = archipelago::gatekeeper(Some(bus.clone()));
            Arc::get_mut(&mut gatekeeper).unwrap().eip1654 = Some(Arc::new(
                catalyst::auth::RpcEip1654Validator::new(url, reqwest::Client::new()),
            ));
            let mut settings = Settings::for_tests();
            settings.auth.handshake_timeout_ms = if cancellation == "deadline" {
                500
            } else {
                5000
            };
            let state = archipelago::build(settings, gatekeeper).await;
            archipelago::start_island_feed(&state, &catalyst::jobs::Jobs::new());
            let port = archipelago::serve(state.clone()).await;
            let (mut socket, principal, _, _, duplicate) = pending_contract(port, false).await;
            tokio::time::timeout(Duration::from_secs(3), entered.acquire())
                .await
                .unwrap()
                .unwrap()
                .forget();
            // Keep the pending crypto operation live while transport and unrelated EOA progress.
            socket.send(duplicate).await.unwrap();
            socket.send(Message::Ping(vec![9].into())).await.unwrap();
            loop {
                match socket.next().await.unwrap().unwrap() {
                    Message::Pong(bytes) => {
                        assert_eq!(bytes.as_ref(), &[9]);
                        break;
                    }
                    Message::Binary(_) => panic!("welcome during held RPC"),
                    _ => {}
                }
            }
            let eoa_started = std::time::Instant::now();
            let (mut eoa, eoa_principal) = authenticated(port).await;
            let eoa_us = eoa_started.elapsed().as_micros();
            eoa.close(None).await.unwrap();
            wait(|| !state.registry.has_any_peer(&eoa_principal)).await;
            assert_eq!(
                calls.lock().unwrap().len(),
                hold + 1,
                "duplicate spawned more verification"
            );
            let cancel_started = std::time::Instant::now();
            match cancellation {
                "close" => socket.close(None).await.unwrap(),
                "restart" => state.restart.cancel(),
                _ => {}
            }
            wait(|| {
                state.handlers.available_permits() == state.settings.server.max_sockets as usize
            })
            .await;
            let cancel_us = cancel_started.elapsed().as_micros();
            assert_eq!(state.challenges.pending(), 0);
            assert!(!state.registry.has_any_peer(&principal));
            release.add_permits(1);
            tokio::time::sleep(Duration::from_millis(15)).await;
            assert_eq!(
                calls.lock().unwrap().len(),
                hold + 1,
                "cancelled future sent another RPC"
            );
            assert!(!bus.requested().iter().any(|(subject, payload)| {
                catalyst::comms::protocol::bus::cluster::ClusterLookup::decode(subject, payload)
                    .is_ok_and(|lookup| lookup.wallet == principal)
            }));
            assert!(!bus.published().iter().any(|(subject, payload)| subject
                == &format!("peer.{principal}.connect")
                || (subject == catalyst::comms::protocol::bus::statistics::OBSERVATIONS_SUBJECT
                    && String::from_utf8_lossy(payload).contains(&principal))));
            if cancellation != "close" {
                while let Ok(Some(Ok(message))) =
                    tokio::time::timeout(Duration::from_secs(1), socket.next()).await
                {
                    assert!(!matches!(message, Message::Binary(_)));
                    if matches!(message, Message::Close(_)) {
                        break;
                    }
                }
            }
            println!(
                "FALLBACK_CANCEL phase={hold} mode={cancellation} cancellation_us={cancel_us} unrelated_eoa_us={eoa_us} calls={}",
                calls.lock().unwrap().len()
            );
            catalyst::comms::archipelago::shutdown(&state).await;
            rpc_task.abort();
        }
    }
}
#[async_trait::async_trait]
impl catalyst::auth::eip1654::Eip1654Validator for ContractValidator {
    fn fresh_validator(&self) -> Option<&dyn catalyst::auth::eip1654::Eip1654Validator> {
        Some(self)
    }
    async fn validate_signature(
        &self,
        _: &str,
        _: &[u8],
        _: &[u8],
    ) -> Result<bool, catalyst::auth::AuthError> {
        use std::sync::atomic::Ordering;
        struct Active<'a>(&'a std::sync::atomic::AtomicUsize);
        impl Drop for Active<'_> {
            fn drop(&mut self) {
                self.0.fetch_sub(1, Ordering::SeqCst);
            }
        }
        self.active.fetch_add(1, Ordering::SeqCst);
        let _active = Active(&self.active);
        self.entered.add_permits(1);
        self.release.acquire().await.unwrap().forget();
        Ok(true)
    }
}

async fn contract_server(
    timeout_ms: u64,
) -> (
    u16,
    AppState,
    std::sync::Arc<ContractValidator>,
    std::sync::Arc<catalyst::comms::gatekeeper::bus::InProcessBus>,
) {
    use std::sync::Arc;
    let validator = Arc::new(ContractValidator {
        entered: tokio::sync::Semaphore::new(0),
        release: tokio::sync::Semaphore::new(0),
        active: std::sync::atomic::AtomicUsize::new(0),
    });
    let bus = Arc::new(catalyst::comms::gatekeeper::bus::InProcessBus::new());
    let mut gatekeeper = archipelago::gatekeeper(Some(bus.clone()));
    Arc::get_mut(&mut gatekeeper).unwrap().eip1654 = Some(Arc::new(
        catalyst::auth::validation_cache::ValidationCache::new(validator.clone()),
    ));
    let mut settings = Settings::for_tests();
    settings.auth.handshake_timeout_ms = timeout_ms;
    let state = archipelago::build(settings, gatekeeper).await;
    archipelago::start_island_feed(&state, &catalyst::jobs::Jobs::new());
    (
        archipelago::serve(state.clone()).await,
        state,
        validator,
        bus,
    )
}

async fn pending_contract(port: u16, delegated: bool) -> (Socket, String, String, String, Message) {
    pending_contract_as(
        port,
        delegated,
        "0x0000000000000000000000000000000000000001".into(),
        PrivateKeySigner::random(),
    )
    .await
}

async fn pending_contract_as(
    port: u16,
    delegated: bool,
    claim: String,
    device: PrivateKeySigner,
) -> (Socket, String, String, String, Message) {
    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws"))
        .await
        .unwrap();
    let address = "0x0000000000000000000000000000000000000001".to_string();
    socket
        .send(packet(client_packet::Message::ChallengeRequest(
            ChallengeRequestMessage { address: claim },
        )))
        .await
        .unwrap();
    let server_packet::Message::ChallengeResponse(response) = receive(&mut socket).await else {
        panic!("challenge expected")
    };
    let session = if delegated {
        format!("{:#x}", device.address())
    } else {
        address.clone()
    };
    let mut chain = vec![serde_json::json!({"type":"SIGNER", "payload":address, "signature":""})];
    if delegated {
        chain.push(serde_json::json!({"type":"ECDSA_EIP_1654_EPHEMERAL",
            "payload":format!("Decentraland Login\nEphemeral address: {session}\nExpiration: 2099-12-31T00:00:00.000Z"),
            "signature":"0x1234"}));
    }
    chain.push(if delegated {
        serde_json::json!({"type":"ECDSA_SIGNED_ENTITY", "payload":response.challenge_to_sign,
            "signature":device.sign_message_sync(response.challenge_to_sign.as_bytes()).unwrap().to_string()})
    } else {
        serde_json::json!({"type":"ECDSA_EIP_1654_SIGNED_ENTITY", "payload":response.challenge_to_sign, "signature":"0x1234"})
    });
    let signed = packet(client_packet::Message::SignedChallenge(
        SignedChallengeMessage {
            auth_chain_json: serde_json::to_string(&chain).unwrap(),
        },
    ));
    socket.send(signed.clone()).await.unwrap();
    (socket, address, session, response.challenge_to_sign, signed)
}

#[tokio::test]
async fn verified_principal_overrides_claim_for_direct_and_delegated_contract() {
    for delegated in [false, true] {
        let (port, state, validator, bus) = contract_server(3_000).await;
        let claim = format!("{:#x}", PrivateKeySigner::random().address());
        let (mut socket, address, session, _, _) =
            pending_contract_as(port, delegated, claim.clone(), PrivateKeySigner::random()).await;
        validator.entered.acquire().await.unwrap().forget();
        validator.release.add_permits(1);
        let server_packet::Message::Welcome(welcome) = receive(&mut socket).await else {
            panic!("welcome expected")
        };
        assert_eq!(welcome.peer_id, address);
        assert!(state.registry.peer_link(&address, &session).is_some());
        assert!(!state.registry.has_any_peer(&claim));
        socket.send(heartbeat(4.)).await.unwrap();
        wait(|| bus.published().iter().any(|(subject, payload)| subject == catalyst::comms::protocol::bus::statistics::OBSERVATIONS_SUBJECT && catalyst::comms::protocol::bus::statistics::Observation::decode(payload).is_some_and(|observation| matches!(observation.event, catalyst::comms::protocol::bus::statistics::Event::Heartbeat { address: ref observed, .. } if observed == &address)))).await;
        socket.close(None).await.unwrap();
        wait(|| !state.registry.has_any_peer(&address)).await;
        wait(|| bus.published().iter().any(|(subject, payload)| subject == catalyst::comms::protocol::bus::statistics::OBSERVATIONS_SUBJECT && catalyst::comms::protocol::bus::statistics::Observation::decode(payload).is_some_and(|observation| matches!(observation.event, catalyst::comms::protocol::bus::statistics::Event::Disconnect { address: ref observed } if observed == &address)))).await;
        assert!(
            !bus.published()
                .iter()
                .any(|(subject, payload)| subject.contains(&claim)
                    || String::from_utf8_lossy(payload).contains(&claim))
        );
        assert!(
            !bus.requested()
                .iter()
                .any(|(subject, payload)| subject.contains(&claim)
                    || String::from_utf8_lossy(payload).contains(&claim))
        );
        catalyst::comms::archipelago::shutdown(&state).await;
    }
}

struct OrderedValidator {
    next: std::sync::atomic::AtomicUsize,
    entered: tokio::sync::Semaphore,
    gates: [tokio::sync::Semaphore; 2],
}
#[async_trait::async_trait]
impl catalyst::auth::eip1654::Eip1654Validator for OrderedValidator {
    fn fresh_validator(&self) -> Option<&dyn catalyst::auth::eip1654::Eip1654Validator> {
        Some(self)
    }
    async fn validate_signature(
        &self,
        _: &str,
        _: &[u8],
        _: &[u8],
    ) -> Result<bool, catalyst::auth::AuthError> {
        let index = self.next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.entered.add_permits(1);
        self.gates[index].acquire().await.unwrap().forget();
        Ok(true)
    }
}

#[tokio::test]
async fn local_registration_follows_successful_completion_and_preserves_other_devices() {
    use std::sync::Arc;
    for order in [[0, 1], [1, 0]] {
        for same_session in [false, true] {
            let validator = Arc::new(OrderedValidator {
                next: 0.into(),
                entered: tokio::sync::Semaphore::new(0),
                gates: std::array::from_fn(|_| tokio::sync::Semaphore::new(0)),
            });
            let bus = Arc::new(catalyst::comms::gatekeeper::bus::InProcessBus::new());
            let mut gatekeeper = archipelago::gatekeeper(Some(bus.clone()));
            Arc::get_mut(&mut gatekeeper).unwrap().eip1654 = Some(validator.clone());
            let state = archipelago::build(Settings::for_tests(), gatekeeper).await;
            archipelago::start_island_feed(&state, &catalyst::jobs::Jobs::new());
            let port = archipelago::serve(state.clone()).await;
            let device = PrivateKeySigner::random();
            let mut sockets = Vec::new();
            let mut sessions = Vec::new();
            for index in 0..2 {
                let (socket, _, session, _, _) = pending_contract_as(
                    port,
                    true,
                    "0x0000000000000000000000000000000000000001".into(),
                    if index == 0 || same_session {
                        device.clone()
                    } else {
                        PrivateKeySigner::random()
                    },
                )
                .await;
                validator.entered.acquire().await.unwrap().forget();
                sockets.push(socket);
                sessions.push(session);
            }
            let address = "0x0000000000000000000000000000000000000001";
            assert_eq!(state.registry.authenticated_count(), 0);
            validator.gates[order[0]].add_permits(1);
            assert!(matches!(
                receive(&mut sockets[order[0]]).await,
                server_packet::Message::Welcome(_)
            ));
            let old = state
                .registry
                .peer_link(address, &sessions[order[0]])
                .unwrap();
            validator.gates[order[1]].add_permits(1);
            assert!(matches!(
                receive(&mut sockets[order[1]]).await,
                server_packet::Message::Welcome(_)
            ));
            let winner = state
                .registry
                .peer_link(address, &sessions[order[1]])
                .unwrap();
            if same_session {
                assert_ne!(winner.id, old.id);
                assert!(old.is_closed());
                assert_eq!(state.registry.sessions_of(address).len(), 1);
            } else {
                assert!(!old.is_closed());
                assert_eq!(state.registry.sessions_of(address).len(), 2);
            }
            let _ = sockets[order[0]].close(None).await;
            wait(|| {
                !state
                    .registry
                    .sessions_of(address)
                    .iter()
                    .any(|link| link.id == old.id)
            })
            .await;
            assert_eq!(
                state
                    .registry
                    .peer_link(address, &sessions[order[1]])
                    .unwrap()
                    .id,
                winner.id
            );
            sockets[order[1]].send(heartbeat(7.)).await.unwrap();
            sockets[order[1]].close(None).await.unwrap();
            wait(|| !state.registry.has_any_peer(address)).await;
            catalyst::comms::archipelago::shutdown(&state).await;
        }
    }
}

#[tokio::test]
async fn contract_socket_auth_uses_configured_fresh_validator_and_admits_derived_session_once() {
    for delegated in [false, true] {
        let (port, state, validator, bus) = contract_server(3_000).await;
        let (mut socket, address, session, _, duplicate) = pending_contract(port, delegated).await;
        validator.entered.acquire().await.unwrap().forget();
        assert!(!state.registry.has_any_peer(&address));
        socket.send(duplicate).await.unwrap();
        socket.send(Message::Ping(vec![7].into())).await.unwrap();
        loop {
            match tokio::time::timeout(Duration::from_secs(2), socket.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap()
            {
                Message::Pong(bytes) => {
                    assert_eq!(bytes.as_ref(), &[7]);
                    break;
                }
                Message::Binary(_) => panic!("welcome before validation"),
                _ => {}
            }
        }
        validator.release.add_permits(1);
        let server_packet::Message::Welcome(welcome) = receive(&mut socket).await else {
            panic!("welcome expected")
        };
        assert_eq!(welcome.peer_id, address);
        assert!(state.registry.has_any_peer(&address));
        assert!(state.registry.peer_link(&address, &session).is_some());
        wait(|| {
            bus.requested().iter().any(|(subject, payload)| {
                catalyst::comms::protocol::bus::cluster::ClusterLookup::decode(subject, payload)
                    .is_ok_and(|lookup| lookup.wallet == address && lookup.session == session)
            })
        })
        .await;
        assert_eq!(
            validator.entered.available_permits(),
            0,
            "duplicate must not start another validation"
        );
        socket.close(None).await.unwrap();
        wait(|| !state.registry.has_any_peer(&address)).await;
        catalyst::comms::archipelago::shutdown(&state).await;
    }
}

#[tokio::test]
async fn pending_contract_auth_is_cancelled_by_deadline_restart_close_and_invalid_stage_data() {
    use std::sync::atomic::Ordering;
    for cancellation in ["deadline", "restart", "close", "heartbeat", "malformed"] {
        let (port, state, validator, bus) = contract_server(if cancellation == "deadline" {
            1_000
        } else {
            3_000
        })
        .await;
        let (mut socket, address, _, _challenge, _) = pending_contract(port, true).await;
        validator.entered.acquire().await.unwrap().forget();
        // A stalled contract does not block another socket's EOA verification.
        let (mut eoa, eoa_address) = authenticated(port).await;
        eoa.close(None).await.unwrap();
        wait(|| !state.registry.has_any_peer(&eoa_address)).await;
        match cancellation {
            "restart" => state.restart.cancel(),
            "close" => socket.close(None).await.unwrap(),
            "heartbeat" => socket.send(heartbeat(4.)).await.unwrap(),
            "malformed" => socket
                .send(Message::Binary(vec![0xff].into()))
                .await
                .unwrap(),
            _ => {}
        }
        wait(|| validator.active.load(Ordering::SeqCst) == 0).await;
        wait(|| state.handlers.available_permits() == state.settings.server.max_sockets as usize)
            .await;
        validator.release.add_permits(1);
        assert!(!state.registry.has_any_peer(&address));
        assert_eq!(state.challenges.pending(), 0);
        assert!(!bus.requested().iter().any(|(subject, payload)| {
            catalyst::comms::protocol::bus::cluster::ClusterLookup::decode(subject, payload)
                .is_ok_and(|lookup| lookup.wallet == address)
        }));
        assert!(!bus.published().iter().any(|(subject, payload)| subject
            == &format!("peer.{address}.connect")
            || (subject == catalyst::comms::protocol::bus::statistics::OBSERVATIONS_SUBJECT
                && String::from_utf8_lossy(payload).contains(&address))));
        if cancellation != "close" {
            while let Some(Ok(message)) =
                tokio::time::timeout(Duration::from_secs(2), socket.next())
                    .await
                    .unwrap()
            {
                assert!(
                    !matches!(message, Message::Binary(_)),
                    "cancelled auth emitted application data"
                );
                if let Message::Close(frame) = message {
                    if cancellation == "malformed" {
                        assert_eq!(u16::from(frame.unwrap().code), 1007);
                    }
                    break;
                }
            }
        }
        catalyst::comms::archipelago::shutdown(&state).await;
    }
}
fn heartbeat(x: f32) -> Message {
    packet(client_packet::Message::Heartbeat(Heartbeat {
        position: Some(Position { x, y: 0.0, z: 0.0 }),
        desired_room: Some("catalyrst".into()),
        sample_sequence: None,
    }))
}

#[tokio::test]
#[ignore = "requires PULSE_NATS_SERVER_BIN; real broker and Pulse statistics publisher"]
async fn orderly_shutdown_drains_authenticated_disconnect_to_live_pulse() {
    use catalyst::comms::gatekeeper::bus::{BrokerBus, NatsBus};
    use catalyst::comms::pulse::cluster::nats::{NatsClusterFeed, NatsFeedOptions};
    use std::sync::Arc;
    let broker = nats_broker::Broker::start(&std::env::var("PULSE_NATS_SERVER_BIN").unwrap(), None);
    let bus = Arc::new(BrokerBus::new(
        Some(broker.url()),
        "statistics-shutdown-test",
    ));
    let jobs = catalyst::jobs::Jobs::new();
    let state = catalyst::comms::archipelago::build_state(
        Settings::for_tests(),
        archipelago::gatekeeper(Some(bus.clone())),
        &jobs,
    )
    .await
    .unwrap();
    let pulse = NatsClusterFeed::spawn(NatsFeedOptions {
        url: broker.url(),
        ..Default::default()
    });
    tokio::time::timeout(Duration::from_secs(15), async {
        while !bus.is_ready() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let port = archipelago::serve(state.clone()).await;
    let (mut socket, address) = authenticated(port).await;
    socket.send(heartbeat(42.)).await.unwrap();
    tokio::time::timeout(Duration::from_secs(15), async {
        while !state
            .statistics
            .snapshot()
            .peers
            .iter()
            .any(|p| p.address == address)
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    catalyst::comms::archipelago::shutdown(&state).await;
    assert_eq!(
        state.handlers.available_permits(),
        state.settings.server.max_sockets as usize
    );
    assert_eq!(
        state.sockets.available_permits(),
        state.settings.server.max_sockets as usize
    );
    assert_eq!(state.observations.dropped(), 0);
    assert!(jobs.shutdown(Duration::from_secs(3)).await.is_empty());
    tokio::time::timeout(Duration::from_secs(15), async {
        while !state.statistics.snapshot().peers.is_empty() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("routine restart left a no-TTL peer behind");
    drop(socket);
    drop(pulse);
}

#[tokio::test]
async fn repeated_and_unknown_wire_fields_reach_the_socket_statistics_boundary() {
    let (port, state) = server(2).await;
    let (mut socket, address) = authenticated(port).await;
    socket.send(heartbeat(9.)).await.unwrap();
    wait(|| archipelago::peer(&state, &address).is_some()).await;
    let first = state.statistics.snapshot();
    socket
        .send(Message::Binary(
            vec![26, 7, 10, 5, 13, 0, 0, 128, 63, 26, 0].into(),
        ))
        .await
        .unwrap();
    // A following Ping/Pong gives a transport processing boundary after the ignored heartbeat.
    socket.send(Message::Ping(vec![1].into())).await.unwrap();
    while !matches!(socket.next().await.unwrap().unwrap(), Message::Pong(_)) {}
    assert_eq!(*state.statistics.snapshot(), *first);
    socket
        .send(Message::Binary(
            vec![26, 9, 10, 5, 13, 0, 0, 128, 63, 10, 0, 34, 0, 0].into(),
        ))
        .await
        .unwrap();
    wait(|| archipelago::peer(&state, &address).is_some_and(|p| p.position == [0.; 3])).await;
    socket.close(None).await.unwrap();
    wait(|| state.statistics.snapshot().peers.is_empty()).await;
}

#[tokio::test]
#[ignore = "95-second authenticated transport liveness capture"]
async fn idle_transport_retires_no_pong_but_preserves_pong_and_data_activity() {
    let (port, state) = server(3).await;
    let (mut silent, silent_address) = authenticated(port).await;
    let (mut responsive, responsive_address) = authenticated(port).await;
    let (mut active, active_address) = authenticated(port).await;
    silent.send(heartbeat(1.)).await.unwrap();
    responsive.send(heartbeat(2.)).await.unwrap();
    active.send(heartbeat(3.)).await.unwrap();
    wait(|| state.statistics.snapshot().peers.len() == 3).await;
    let pong = tokio::spawn(async move {
        while let Some(Ok(message)) = responsive.next().await {
            if matches!(message, Message::Ping(_)) {
                responsive.flush().await.unwrap();
            }
            if matches!(message, Message::Close(_)) {
                break;
            }
        }
    });
    tokio::time::sleep(Duration::from_secs(70)).await;
    active.send(heartbeat(4.)).await.unwrap();
    tokio::time::sleep(Duration::from_secs(25)).await;
    assert!(!state.registry.has_any_peer(&silent_address));
    assert!(state.registry.has_any_peer(&responsive_address));
    assert!(state.registry.has_any_peer(&active_address));
    assert_eq!(state.sockets.available_permits(), 1);
    assert!(
        !state
            .statistics
            .snapshot()
            .peers
            .iter()
            .any(|p| p.address == silent_address)
    );
    assert!(state.enforcement.island_of(&silent_address).is_none());
    catalyst::comms::archipelago::shutdown(&state).await;
    pong.await.unwrap();
    drop((silent, active));
}

#[tokio::test]
async fn upgrade_cap_is_shared_across_paths_and_mounts_and_released_after_transport_end() {
    let (port, state) = server(1).await;
    let (socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws"))
        .await
        .unwrap();
    assert_eq!(state.sockets.available_permits(), 0);
    for path in ["/ws", "/archipelago-ea-stats/ws"] {
        let error = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}{path}"))
            .await
            .unwrap_err();
        let Error::Http(response) = error else {
            panic!("HTTP refusal expected")
        };
        assert_eq!(response.status().as_u16(), 503);
        assert_eq!(response.headers()["retry-after"], "5");
    }
    drop(socket);
    wait(|| state.sockets.available_permits() == 1).await;
    let (socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws"))
        .await
        .unwrap();
    drop(socket);
    wait(|| state.sockets.available_permits() == 1).await;
}
#[tokio::test]
async fn baseline_v4_authentication_is_not_exposed() {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let (port, _) = server(2).await;
    for path in ["/ws/v4", "/archipelago-ea-stats/ws/v4"] {
        let Error::Http(response) =
            tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}{path}"))
                .await
                .unwrap_err()
        else {
            panic!("removed route must return an HTTP refusal")
        };
        assert_eq!(response.status().as_u16(), 404);
    }
    let mut request = format!("ws://127.0.0.1:{port}/ws")
        .into_client_request()
        .unwrap();
    request.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        "archipelago-v4, archipelago".parse().unwrap(),
    );
    let (mut socket, response) = tokio_tungstenite::connect_async(request).await.unwrap();
    assert_eq!(
        response.headers()["Sec-WebSocket-Protocol"],
        "archipelago-v4"
    );
    // Echoing a protocol name does not enable extended offer or typed-auth fields.
    // ChallengeRequest.address followed by the reserved ConnectionOffer tag.
    let mut challenge = vec![0x0a, 42];
    challenge.extend_from_slice(b"0x0000000000000000000000000000000000000001");
    challenge.extend_from_slice(&[0x12, 0]);
    let mut frame = vec![0x0a, challenge.len() as u8];
    frame.extend(challenge);
    socket.send(Message::Binary(frame.into())).await.unwrap();
    assert!(matches!(
        receive(&mut socket).await,
        server_packet::Message::ChallengeResponse(_)
    ));
}
#[tokio::test]
async fn unknown_auth_field_does_not_replace_a_valid_legacy_signature() {
    let (port, state) = server(2).await;
    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws"))
        .await
        .unwrap();
    let wallet = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    socket
        .send(packet(client_packet::Message::ChallengeRequest(
            ChallengeRequestMessage {
                address: address.clone(),
            },
        )))
        .await
        .unwrap();
    let server_packet::Message::ChallengeResponse(challenge) = receive(&mut socket).await else {
        panic!("ordinary signed challenge expected")
    };
    let signature = wallet
        .sign_message_sync(challenge.challenge_to_sign.as_bytes())
        .unwrap()
        .to_string();
    let chain = serde_json::json!([
        {"type":"SIGNER", "payload":address, "signature":""},
        {"type":"ECDSA_SIGNED_ENTITY", "payload":challenge.challenge_to_sign, "signature":signature}
    ]);
    let mut signed = SignedChallengeMessage {
        auth_chain_json: chain.to_string(),
    }
    .encode_to_vec();
    signed.extend_from_slice(&[0x12, 0]);
    let mut frame = vec![0x12];
    prost::encode_length_delimiter(signed.len(), &mut frame).unwrap();
    frame.extend(signed);
    socket.send(Message::Binary(frame.into())).await.unwrap();
    assert!(matches!(
        receive(&mut socket).await,
        server_packet::Message::Welcome(_)
    ));
    assert!(
        state.registry.has_any_peer(&address),
        "only the verified legacy chain admits; unknown fields never select authentication"
    );
}

#[tokio::test]
async fn restart_closes_a_pending_handshake_with_1012_and_refuses_new_upgrades() {
    let (port, state) = server(1).await;
    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws"))
        .await
        .unwrap();
    state.restart.cancel();
    let message = tokio::time::timeout(Duration::from_secs(2), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(matches!(message, Message::Close(Some(frame)) if u16::from(frame.code) == 1012));
    wait(|| state.sockets.available_permits() == 1).await;
    let Error::Http(response) =
        tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws"))
            .await
            .unwrap_err()
    else {
        panic!("HTTP refusal expected")
    };
    assert_eq!(response.status().as_u16(), 503);
    assert_eq!(response.headers()["retry-after"], "5");
}
#[tokio::test]
async fn statistics_observe_heartbeats_without_ownership_coalescing() {
    let (port, state) = server(1).await;
    let (mut socket, address) = authenticated(port).await;
    socket.send(heartbeat(1.0)).await.unwrap();
    wait(|| archipelago::peer(&state, &address).is_some()).await;
    socket.send(heartbeat(2.0)).await.unwrap();
    wait(|| archipelago::peer(&state, &address).unwrap().position[0] == 2.0).await;
    socket.send(heartbeat(3.0)).await.unwrap();
    wait(|| archipelago::peer(&state, &address).unwrap().position[0] == 3.0).await;
}
#[tokio::test]
async fn sustained_excess_heartbeats_close_the_real_socket_with_1008() {
    let (port, state) = server(1).await;
    for suffix in [None, Some(0), Some(36)] {
        let (socket, address) = authenticated(port).await;
        let (mut send, mut read) = socket.split();
        let Message::Binary(bytes) = heartbeat(1.0) else {
            unreachable!()
        };
        let mut bytes = bytes.to_vec();
        bytes.extend(suffix);
        // Five per tick: the limiter keeps 21 arrivals, so a host stall under 900 ms stays in-window.
        let flood = async {
            loop {
                for _ in 0..5 {
                    if send
                        .send(Message::Binary(bytes.clone().into()))
                        .await
                        .is_err()
                    {
                        std::future::pending::<()>().await;
                    }
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        };
        let close = async {
            loop {
                if let Some(Ok(Message::Close(frame))) = read.next().await {
                    return frame;
                }
            }
        };
        // Room for a second ten-second cycle when one longer stall resets the first.
        let frame = tokio::time::timeout(Duration::from_secs(25), async {
            tokio::select! { frame = close => frame, _ = flood => unreachable!() }
        })
        .await
        .unwrap();
        assert_eq!(u16::from(frame.unwrap().code), 1008);
        wait(|| archipelago::peer(&state, &address).is_none()).await;
        wait(|| !state.registry.has_any_peer(&address)).await;
        wait(|| state.sockets.available_permits() == 1).await;
    }
}

#[tokio::test]
async fn source_contract_challenge_has_32_random_bytes() {
    let (port, _) = server(2).await;
    let mut challenges = Vec::new();
    for _ in 0..2 {
        let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws"))
            .await
            .unwrap();
        socket
            .send(packet(client_packet::Message::ChallengeRequest(
                ChallengeRequestMessage {
                    address: "0x0000000000000000000000000000000000000001".into(),
                },
            )))
            .await
            .unwrap();
        let server_packet::Message::ChallengeResponse(response) = receive(&mut socket).await else {
            panic!("challenge")
        };
        assert_eq!(response.challenge_to_sign.len(), 68);
        assert!(response.challenge_to_sign.starts_with("dcl-"));
        assert!(
            response.challenge_to_sign[4..]
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
        challenges.push(response.challenge_to_sign);
    }
    assert_ne!(challenges[0], challenges[1]);
}

#[tokio::test]
async fn source_contract_malformed_packet_preserves_close_reason() {
    let (port, _) = server(2).await;
    for authenticated_first in [false, true] {
        let mut socket = if authenticated_first {
            authenticated(port).await.0
        } else {
            tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws"))
                .await
                .unwrap()
                .0
        };
        socket
            .send(Message::Binary(vec![1, 2, 3, 4, 5, 6].into()))
            .await
            .unwrap();
        let message = tokio::time::timeout(Duration::from_secs(3), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let Message::Close(Some(frame)) = message else {
            panic!("explicit close expected: {message:?}")
        };
        assert_eq!(u16::from(frame.code), 1007);
        assert_eq!(frame.reason, "Cannot decode ClientPacket");
    }
}

#[tokio::test]
async fn source_contract_text_data_uses_the_same_protobuf_decoder() {
    let (port, _) = server(2).await;
    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws"))
        .await
        .unwrap();
    let Message::Binary(bytes) = packet(client_packet::Message::ChallengeRequest(
        ChallengeRequestMessage {
            address: "0x0000000000000000000000000000000000000001".into(),
        },
    )) else {
        unreachable!()
    };
    socket
        .send(Message::Text(
            String::from_utf8(bytes.to_vec()).unwrap().into(),
        ))
        .await
        .unwrap();
    assert!(matches!(
        receive(&mut socket).await,
        server_packet::Message::ChallengeResponse(_)
    ));
    socket
        .send(Message::Text("\u{1}\u{2}\u{3}".into()))
        .await
        .unwrap();
    let message = tokio::time::timeout(Duration::from_secs(3), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(
        matches!(message, Message::Close(Some(frame)) if u16::from(frame.code) == 1007 && frame.reason == "Cannot decode ClientPacket")
    );
}

#[tokio::test]
async fn source_contract_claim_requires_lowercase_hex_prefix_but_allows_mixed_digits() {
    let (port, _) = server(2).await;
    for (address, accepted) in [
        ("0X00000000000000000000000000000000000000AB", false),
        ("0x00000000000000000000000000000000000000AB", true),
    ] {
        let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws"))
            .await
            .unwrap();
        socket
            .send(packet(client_packet::Message::ChallengeRequest(
                ChallengeRequestMessage {
                    address: address.into(),
                },
            )))
            .await
            .unwrap();
        let message = tokio::time::timeout(Duration::from_secs(3), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(matches!(message, Message::Binary(_)), accepted, "{address}");
    }
}

#[tokio::test]
async fn source_contract_negotiates_first_offered_protocol_without_selecting_auth() {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let (port, _) = server(2).await;
    for offered in ["other", "other, archipelago", "archipelago-v4, archipelago"] {
        let mut request = format!("ws://127.0.0.1:{port}/ws")
            .into_client_request()
            .unwrap();
        request
            .headers_mut()
            .insert("sec-websocket-protocol", offered.parse().unwrap());
        let (mut socket, response) = tokio_tungstenite::connect_async(request).await.unwrap();
        assert_eq!(
            response.headers()["sec-websocket-protocol"],
            offered.split(',').next().unwrap()
        );
        socket
            .send(packet(client_packet::Message::ChallengeRequest(
                ChallengeRequestMessage {
                    address: "0x0000000000000000000000000000000000000001".into(),
                },
            )))
            .await
            .unwrap();
        assert!(matches!(
            receive(&mut socket).await,
            server_packet::Message::ChallengeResponse(_)
        ));
    }
}

#[tokio::test]
async fn source_contract_connector_plain_socket_path_uses_json_fallback() {
    let (port, _) = server(2).await;
    let client = reqwest::Client::new();
    for method in [
        reqwest::Method::GET,
        reqwest::Method::HEAD,
        reqwest::Method::OPTIONS,
        reqwest::Method::POST,
    ] {
        let response = client
            .request(
                method.clone(),
                format!("http://127.0.0.1:{port}/archipelago-ea-ws-connector/ws"),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 404, "{method}");
        assert_eq!(response.headers()["access-control-allow-origin"], "*");
        assert_eq!(response.headers()["content-type"], "application/json");
        if method != reqwest::Method::HEAD {
            assert_eq!(response.text().await.unwrap(), r#"{"error":"Not Found"}"#);
        }
    }
}

#[tokio::test]
async fn source_contract_inbound_message_limit_is_16_kib() {
    let (port, _) = server(2).await;
    for size in [16384, 16385] {
        let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws"))
            .await
            .unwrap();
        let Message::Binary(bytes) = packet(client_packet::Message::ChallengeRequest(
            ChallengeRequestMessage {
                address: "0x0000000000000000000000000000000000000001".into(),
            },
        )) else {
            unreachable!()
        };
        let mut bytes = bytes.to_vec();
        let padding = size - bytes.len() - 4;
        bytes.extend_from_slice(&[0xa2, 0x06]);
        prost::encode_length_delimiter(padding, &mut bytes).unwrap();
        bytes.resize(size, 0);
        socket.send(Message::Binary(bytes.into())).await.unwrap();
        let next = tokio::time::timeout(Duration::from_secs(3), socket.next())
            .await
            .unwrap();
        assert_eq!(
            matches!(next, Some(Ok(Message::Binary(_)))),
            size == 16384,
            "{size}"
        );
    }
}
