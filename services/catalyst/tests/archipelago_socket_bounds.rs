//! Runtime socket limits over real local WebSockets. No external database/broker required.
#[path = "support/archipelago.rs"]
mod archipelago;
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
    let mounts = catalyst::comms::archipelago::mounts(state.clone(), true);
    let app = axum::Router::new()
        .merge(mounts.root)
        .nest("/archipelago-ea-stats", mounts.stats);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
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
        match tokio::time::timeout(Duration::from_secs(3), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap()
        {
            Message::Binary(bytes) => return ServerPacket::decode(bytes).unwrap().message.unwrap(),
            _ => {}
        }
    }
}
async fn authenticated(port: u16) -> (Socket, String) {
    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws"))
        .await
        .unwrap();
    let wallet = PrivateKeySigner::random();
    let device = PrivateKeySigner::random();
    let address = format!("{:#x}", wallet.address());
    socket
        .send(packet(client_packet::Message::ChallengeRequest(
            ChallengeRequestMessage {
                address: address.clone(),
            },
        )))
        .await
        .unwrap();
    let server_packet::Message::ChallengeResponse(response) = receive(&mut socket).await else {
        panic!("challenge expected")
    };
    let mandate = format!(
        "Decentraland Login\nEphemeral address: {:#x}\nExpiration: 2099-12-31T00:00:00.000Z",
        device.address()
    );
    let sign = |key: &PrivateKeySigner, message: &str| {
        key.sign_hash_sync(&alloy::primitives::eip191_hash_message(message.as_bytes()))
            .unwrap()
            .to_string()
    };
    let chain = serde_json::json!([
        { "type": "SIGNER", "payload": address, "signature": "" },
        { "type": "ECDSA_EPHEMERAL", "payload": mandate, "signature": sign(&wallet, &mandate) },
        { "type": "ECDSA_SIGNED_ENTITY", "payload": response.challenge_to_sign, "signature": sign(&device, &response.challenge_to_sign) }
    ]);
    socket
        .send(packet(client_packet::Message::SignedChallenge(
            SignedChallengeMessage {
                auth_chain_json: chain.to_string(),
            },
        )))
        .await
        .unwrap();
    assert!(matches!(
        receive(&mut socket).await,
        server_packet::Message::Welcome(_)
    ));
    (socket, address)
}
fn heartbeat(x: f32) -> Message {
    packet(client_packet::Message::Heartbeat(Heartbeat {
        position: Some(Position { x, y: 0.0, z: 0.0 }),
        desired_room: Some("catalyrst".into()),
        sample_sequence: None,
    }))
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
    assert_eq!(response.headers()["Sec-WebSocket-Protocol"], "archipelago");
    // ChallengeRequest.address followed by the reserved ConnectionOffer tag.
    let mut challenge = vec![0x0a, 42];
    challenge.extend_from_slice(b"0x0000000000000000000000000000000000000001");
    challenge.extend_from_slice(&[0x12, 0]);
    let mut frame = vec![0x0a, challenge.len() as u8];
    frame.extend(challenge);
    socket.send(Message::Binary(frame.into())).await.unwrap();
    let closed = tokio::time::timeout(Duration::from_secs(2), socket.next())
        .await
        .unwrap();
    assert!(!matches!(closed, Some(Ok(Message::Binary(_)))));
}
#[tokio::test]
async fn typed_auth_field_cannot_admit_beside_a_valid_legacy_signature() {
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
    let closed = tokio::time::timeout(Duration::from_secs(2), socket.next())
        .await
        .unwrap();
    assert!(!matches!(closed, Some(Ok(Message::Binary(_)))));
    assert!(
        !state.registry.has_any_peer(&address),
        "typed auth cannot admit even beside a valid signed legacy chain"
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
async fn heartbeat_burst_eventually_applies_latest_position_when_sender_goes_quiet() {
    let (port, state) = server(1).await;
    let (mut socket, address) = authenticated(port).await;
    socket.send(heartbeat(1.0)).await.unwrap();
    wait(|| state.peers.peer(&address).is_some()).await;
    socket.send(heartbeat(2.0)).await.unwrap();
    socket.send(heartbeat(3.0)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert_eq!(state.peers.peer(&address).unwrap().position[0], 1.0);
    wait(|| state.peers.peer(&address).unwrap().position[0] == 3.0).await;
}
#[tokio::test]
async fn sustained_excess_heartbeats_close_the_real_socket_with_1008() {
    let (port, _) = server(1).await;
    let (socket, _) = authenticated(port).await;
    let (mut send, mut read) = socket.split();
    let flood = async {
        loop {
            if send.send(heartbeat(1.0)).await.is_err() {
                std::future::pending::<()>().await;
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
    let frame = tokio::time::timeout(Duration::from_secs(13), async {
        tokio::select! { frame = close => frame, _ = flood => unreachable!() }
    })
    .await
    .unwrap();
    assert_eq!(u16::from(frame.unwrap().code), 1008);
}
