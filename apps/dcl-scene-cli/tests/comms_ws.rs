mod common {
    pub mod wait;
    pub mod wallet;
    pub mod ws;
}

use common::ws::{
    Socket, connect_ws, handshake, recv_message, send_packet, sign_challenge, wallet_address,
};
use dcl_scene_cli::comms::proto::{WsIdentification, WsPeerUpdate, WsSignedChallenge, ws_packet};
use dcl_scene_cli::comms::{CommsState, routes};
use futures::StreamExt;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

async fn spawn_server() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = routes(Arc::new(CommsState::default()));
    // Bound and listening already: a connect waits in the backlog until served.
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    addr
}

async fn connect(addr: SocketAddr, room: &str, protocol: &'static str) -> Socket {
    connect_ws(&format!("ws://{addr}/mini-comms/{room}"), protocol).await
}

#[tokio::test]
async fn two_clients_full_room_lifecycle() {
    let addr = spawn_server().await;

    let signer_a = common::wallet::random_wallet();
    let signer_b = common::wallet::random_wallet();

    let mut client_a = connect(addr, "room-1", "rfc5").await;
    let (alias_a, peers_a) = handshake(&mut client_a, &signer_a).await;
    assert!(peers_a.is_empty(), "first peer sees an empty room");

    let mut client_b = connect(addr, "room-1", "rfc5").await;
    let (alias_b, peers_b) = handshake(&mut client_b, &signer_b).await;
    assert_ne!(alias_a, alias_b, "aliases must be distinct");
    assert_eq!(peers_b.len(), 1);
    assert_eq!(
        peers_b.get(&alias_a).map(String::as_str),
        Some(wallet_address(&signer_a).to_lowercase().as_str())
    );

    match recv_message(&mut client_a).await {
        ws_packet::Message::PeerJoinMessage(join) => {
            assert_eq!(join.alias, alias_b);
            assert_eq!(join.address, wallet_address(&signer_b).to_lowercase());
        }
        other => panic!("expected peerJoinMessage on client A, got {other:?}"),
    }

    send_packet(
        &mut client_a,
        ws_packet::Message::PeerUpdateMessage(WsPeerUpdate {
            from_alias: 4242,
            body: b"hello from a".to_vec(),
            unreliable: false,
            recipients: Vec::new(),
        }),
    )
    .await;

    match recv_message(&mut client_b).await {
        ws_packet::Message::PeerUpdateMessage(update) => {
            assert_eq!(
                update.from_alias, alias_a,
                "server must overwrite from_alias with the sender's alias"
            );
            assert_eq!(update.body, b"hello from a".to_vec());
            assert!(!update.unreliable);
        }
        other => panic!("expected peerUpdateMessage on client B, got {other:?}"),
    }

    client_a.close(None).await.unwrap();

    match recv_message(&mut client_b).await {
        ws_packet::Message::PeerLeaveMessage(leave) => {
            assert_eq!(leave.alias, alias_a);
        }
        other => panic!("expected peerLeaveMessage on client B, got {other:?}"),
    }
}

async fn expect_close(socket: &mut Socket) {
    let msg = tokio::time::timeout(common::wait::HANG_GUARD, socket.next())
        .await
        .expect("server should close a rejected handshake");
    match msg {
        Some(Ok(Message::Close(_))) | None => {}
        other => panic!("expected close after rejected auth, got {other:?}"),
    }
}

#[tokio::test]
async fn lone_signer_echo_of_the_challenge_is_rejected() {
    let addr = spawn_server().await;
    let victim = common::wallet::random_wallet();
    let mut socket = connect(addr, "room-1", "rfc5").await;
    send_packet(
        &mut socket,
        ws_packet::Message::PeerIdentification(WsIdentification {
            address: wallet_address(&victim),
        }),
    )
    .await;
    let challenge = match recv_message(&mut socket).await {
        ws_packet::Message::ChallengeMessage(c) => c.challenge_to_sign,
        other => panic!("expected challengeMessage, got {other:?}"),
    };
    let echo = format!(r#"[{{"type":"SIGNER","payload":"{challenge}","signature":""}}]"#);
    send_packet(
        &mut socket,
        ws_packet::Message::SignedChallengeForServer(WsSignedChallenge {
            auth_chain_json: echo,
        }),
    )
    .await;
    expect_close(&mut socket).await;
}

#[tokio::test]
async fn chain_owned_by_a_different_wallet_is_rejected() {
    let addr = spawn_server().await;
    let victim = common::wallet::random_wallet();
    let attacker = common::wallet::random_wallet();
    let mut socket = connect(addr, "room-1", "rfc5").await;
    send_packet(
        &mut socket,
        ws_packet::Message::PeerIdentification(WsIdentification {
            address: wallet_address(&victim),
        }),
    )
    .await;
    let challenge = match recv_message(&mut socket).await {
        ws_packet::Message::ChallengeMessage(c) => c.challenge_to_sign,
        other => panic!("expected challengeMessage, got {other:?}"),
    };
    let attacker_chain = sign_challenge(&attacker, &challenge);
    send_packet(
        &mut socket,
        ws_packet::Message::SignedChallengeForServer(WsSignedChallenge {
            auth_chain_json: attacker_chain,
        }),
    )
    .await;
    expect_close(&mut socket).await;
}

#[tokio::test]
async fn rfc4_subprotocol_negotiates_and_bad_chain_rejected() {
    let addr = spawn_server().await;

    let signer = common::wallet::random_wallet();
    let mut socket = connect(addr, "room-1", "rfc4").await;
    send_packet(
        &mut socket,
        ws_packet::Message::PeerIdentification(WsIdentification {
            address: wallet_address(&signer),
        }),
    )
    .await;
    let challenge = match recv_message(&mut socket).await {
        ws_packet::Message::ChallengeMessage(c) => c.challenge_to_sign,
        other => panic!("expected challengeMessage, got {other:?}"),
    };
    let forged = sign_challenge(&signer, &format!("{challenge}-tampered"));
    send_packet(
        &mut socket,
        ws_packet::Message::SignedChallengeForServer(WsSignedChallenge {
            auth_chain_json: forged,
        }),
    )
    .await;
    let msg = tokio::time::timeout(common::wait::HANG_GUARD, socket.next())
        .await
        .expect("server should close a failed authentication");
    match msg {
        Some(Ok(Message::Close(_))) | None => {}
        other => panic!("expected close after invalid signed challenge, got {other:?}"),
    }
}

#[tokio::test]
async fn a_frame_over_the_message_bound_ends_that_session_alone() {
    let addr = spawn_server().await;
    let signer_a = common::wallet::random_wallet();
    let signer_b = common::wallet::random_wallet();
    let mut client_a = connect(addr, "room-1", "rfc5").await;
    let (alias_a, _) = handshake(&mut client_a, &signer_a).await;
    let mut client_b = connect(addr, "room-1", "rfc5").await;
    let (alias_b, _) = handshake(&mut client_b, &signer_b).await;
    match recv_message(&mut client_a).await {
        ws_packet::Message::PeerJoinMessage(join) => assert_eq!(join.alias, alias_b),
        other => panic!("expected peerJoinMessage, got {other:?}"),
    }

    let update = |body: Vec<u8>| {
        ws_packet::Message::PeerUpdateMessage(WsPeerUpdate {
            from_alias: 0,
            body,
            unreliable: false,
            recipients: Vec::new(),
        })
    };
    send_packet(&mut client_b, update(vec![7; 200 * 1024])).await;
    match recv_message(&mut client_a).await {
        ws_packet::Message::PeerUpdateMessage(update) => {
            assert_eq!(update.from_alias, alias_b);
            assert_eq!(update.body.len(), 200 * 1024);
        }
        other => panic!("expected the large update, got {other:?}"),
    }

    send_packet(&mut client_b, update(vec![7; 300 * 1024])).await;
    match recv_message(&mut client_a).await {
        ws_packet::Message::PeerLeaveMessage(leave) => assert_eq!(leave.alias, alias_b),
        other => panic!("expected the oversized sender to leave, got {other:?}"),
    }

    send_packet(&mut client_a, update(b"still here".to_vec())).await;
    let mut client_c = connect(addr, "room-1", "rfc5").await;
    let (_, peers_c) = handshake(&mut client_c, &common::wallet::random_wallet()).await;
    assert_eq!(peers_c.keys().copied().collect::<Vec<_>>(), vec![alias_a]);
}
