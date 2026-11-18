//! The router against a real `livekit-server` and the node's schema: a member's community
//! message reaches the other member with the author stamped in, and a non-member's does not.
//! Needs `MESSAGE_ROUTER_LIVEKIT_SERVER_BIN` (a `livekit-server` binary) and
//! `CATALYST_TEST_DATABASE_URL`; skips without either. The gatekeeper's token route is stood in
//! for by a server that checks the same signature and mints the same grant
//! (`comms::gatekeeper::message_router` has its own tests).

use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use catalyst_message_router::connection::Retry;
use catalyst_message_router::membership::PgMembership;
use catalyst_message_router::metrics::Connection;
use catalyst_message_router::participant::Router;
use catalyst_message_router::proto::rfc4;
use catalyst_message_router::rate_limiter::{Limits, RateLimiter};
use catalyst_message_router::token::{TokenClient, check_url};
use catalyst_shared::message_router::{self as contract, TOKEN_PATH, TokenRequest, TokenResponse};
use dcl_livekit_token::{AccessToken, VideoGrants};
use livekit::prelude::*;
use prost::Message;
use uuid::Uuid;

const API_KEY: &str = "APIrouterE2E";
const API_SECRET: &str = "router-e2e-secret-of-at-least-thirty-two-bytes";
const ROOM: &str = "private-messages";
const PREFIX: &str = "message-router-dev";
const ALICE: &str = "0x00000000000000000000000000000000000a11ce";
const BOB: &str = "0x0000000000000000000000000000000000000b0b";
const CAROL: &str = "0x00000000000000000000000000000000000ca201";

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn grants(can_publish: bool) -> VideoGrants {
    VideoGrants {
        room_join: true,
        room: ROOM.into(),
        can_publish,
        can_subscribe: true,
        can_publish_data: true,
        can_update_own_metadata: false,
        room_list: Some(false),
        can_publish_sources: None,
    }
}

fn token(identity: &str) -> String {
    AccessToken::new(API_KEY, API_SECRET, identity, grants(false))
        .to_jwt()
        .unwrap()
}

struct Server(std::process::Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start_livekit(binary: &str, port: u16) -> Server {
    let rtc_tcp = free_port();
    let rtc_udp = free_port();
    let config = format!(
        "port: {port}\nbind_addresses:\n  - 127.0.0.1\nrtc:\n  tcp_port: {rtc_tcp}\n  \
         udp_port: {rtc_udp}\n  use_external_ip: false\nkeys:\n  {API_KEY}: {API_SECRET}\n\
         logging:\n  level: warn\n"
    );
    Server(
        std::process::Command::new(binary)
            .env("LIVEKIT_CONFIG", config)
            .spawn()
            .expect("starting livekit-server"),
    )
}

/// The stand-in token route: the contract's signature, the router's grant.
async fn start_token_route(key: [u8; 32], livekit_url: String) -> String {
    use axum::http::{HeaderMap, StatusCode};
    use axum::response::IntoResponse;
    let app = axum::Router::new().route(
        TOKEN_PATH,
        axum::routing::post(move |headers: HeaderMap, body: axum::body::Bytes| {
            let livekit_url = livekit_url.clone();
            async move {
                let header = |name: &str| {
                    headers
                        .get(name)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or_default()
                        .to_owned()
                };
                if !contract::verify(
                    &key,
                    &header(contract::HEADER_TIMESTAMP),
                    &header(contract::HEADER_NONCE),
                    &body,
                    &header(contract::HEADER_SIGNATURE),
                ) {
                    return StatusCode::UNAUTHORIZED.into_response();
                }
                let request: TokenRequest = serde_json::from_slice(&body).unwrap();
                let identity = contract::identity(PREFIX, request.replica);
                let token = AccessToken::new(API_KEY, API_SECRET, &identity, grants(false))
                    .to_jwt()
                    .unwrap();
                axum::Json(TokenResponse {
                    url: livekit_url,
                    token,
                    room: ROOM.into(),
                    identity,
                })
                .into_response()
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}{TOKEN_PATH}")
}

async fn community(pool: &sqlx::PgPool) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO social.communities (id, name, owner_address) VALUES ($1, 'router e2e', $2)",
    )
    .bind(id)
    .bind(ALICE)
    .execute(pool)
    .await
    .unwrap();
    for (address, role) in [(ALICE, "owner"), (BOB, "member")] {
        sqlx::query(
            "INSERT INTO social.community_members (community_id, member_address, role) \
             VALUES ($1, $2, $3)",
        )
        .bind(id)
        .bind(address)
        .bind(role)
        .execute(pool)
        .await
        .unwrap();
    }
    id
}

async fn join(
    url: &str,
    identity: &str,
) -> (Room, tokio::sync::mpsc::UnboundedReceiver<RoomEvent>) {
    let mut options = RoomOptions::default();
    options.auto_subscribe = false;
    Room::connect(url, &token(identity), options)
        .await
        .unwrap_or_else(|e| panic!("{identity} joining: {e}"))
}

fn chat(text: &str) -> Vec<u8> {
    rfc4::Packet {
        message: Some(rfc4::packet::Message::Chat(rfc4::Chat {
            message: text.into(),
            timestamp: 1.0,
            forwarded_from: None,
        })),
        protocol_version: 100,
    }
    .encode_to_vec()
}

async fn send(room: &Room, router: &str, topic: &str, payload: Vec<u8>) {
    room.local_participant()
        .publish_data(DataPacket {
            payload,
            topic: Some(topic.into()),
            reliable: true,
            destination_identities: vec![ParticipantIdentity(router.into())],
        })
        .await
        .unwrap();
}

/// The next chat packet `events` delivers within `wait`: (sender identity, topic, packet).
async fn next_chat(
    events: &mut tokio::sync::mpsc::UnboundedReceiver<RoomEvent>,
    wait: Duration,
) -> Option<(String, Option<String>, rfc4::Chat)> {
    let deadline = tokio::time::Instant::now() + wait;
    loop {
        let event = tokio::time::timeout_at(deadline, events.recv())
            .await
            .ok()??;
        if let RoomEvent::DataReceived {
            payload,
            topic,
            participant,
            participant_identity,
            ..
        } = event
        {
            let sender = participant
                .map(|p| p.identity().0)
                .or(participant_identity.map(|i| i.0))
                .unwrap_or_default();
            if let Ok(rfc4::Packet {
                message: Some(rfc4::packet::Message::Chat(chat)),
                ..
            }) = rfc4::Packet::decode(payload.as_slice())
            {
                return Some((sender, topic, chat));
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_s_message_reaches_members_and_a_stranger_s_does_not() {
    let Ok(binary) = std::env::var("MESSAGE_ROUTER_LIVEKIT_SERVER_BIN") else {
        return;
    };
    let Some(pool) = catalyst_db::scratch::pool(2).await else {
        return;
    };
    let port = free_port();
    let _server = start_livekit(&binary, port);
    let livekit_url = format!("ws://127.0.0.1:{port}");
    let key = [11u8; 32];
    let token_url = start_token_route(key, livekit_url.clone()).await;
    let community = community(&pool).await;
    let topic = format!("community:{community}");

    let connection = Connection::default();
    let router = Arc::new(Router {
        tokens: TokenClient::new(check_url(&token_url).unwrap(), key, 0).unwrap(),
        livekit_url: None,
        retry: Retry {
            attempts: 20,
            delay: Duration::from_millis(500),
        },
        max_packet_size_bytes: 8192,
        members: Arc::new(PgMembership::new(pool.clone())),
        limiter: Arc::new(Mutex::new(RateLimiter::new(Limits::default()))),
        connection: connection.clone(),
    });
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let running = tokio::spawn(router.clone().run(async {
        let _ = stopped.await;
    }));
    for _ in 0..100 {
        if connection.get() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(connection.get(), "the router joined the room");
    let identity = contract::identity(PREFIX, 0);

    let (alice, _alice_events) = join(&livekit_url, ALICE).await;
    let (_bob, mut bob_events) = join(&livekit_url, BOB).await;
    let (carol, mut carol_events) = join(&livekit_url, CAROL).await;
    // let the joins settle on the server before sending
    tokio::time::sleep(Duration::from_millis(500)).await;

    send(&alice, &identity, &topic, chat("hello members")).await;
    let (sender, received_topic, message) = next_chat(&mut bob_events, Duration::from_secs(10))
        .await
        .expect("bob receives alice's message");
    assert_eq!(sender, identity, "relayed by the router");
    assert_eq!(received_topic.as_deref(), Some(topic.as_str()));
    assert_eq!(message.message, "hello members");
    assert_eq!(message.forwarded_from.as_deref(), Some(ALICE));
    assert!(
        next_chat(&mut carol_events, Duration::from_secs(1))
            .await
            .is_none(),
        "a non-member receives nothing"
    );

    send(&carol, &identity, &topic, chat("let me in")).await;
    assert!(
        next_chat(&mut bob_events, Duration::from_secs(3))
            .await
            .is_none(),
        "a non-member's message is not relayed"
    );

    let _ = stop.send(());
    running.await.unwrap().unwrap();
    assert!(!connection.get());
}
