//! `/social-rpc/v1/stream` (`rpc::stream`) over real WebSockets, the cases
//! of upstream #503 (`decentraland/social-service-ea` at `b6b2cef`,
//! `test/integration/stream-ws-controller.spec.ts` and
//! `docs/websocket-stream-protocol.md`): both handshakes and their
//! refusals, a subscription receiving the update its RPC stream would,
//! unsubscription, the protocol errors, and the per-address socket cap the
//! RPC socket shares.

use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::{HeaderName, HeaderValue};

use super::*;

const STREAM: &str = "/social-rpc/v1/stream";

/// A frame (or the same object as upgrade headers) signed by `who` for `get path`.
async fn signed_frame(who: &PrivateKeySigner, path: &str) -> String {
    let ts = chrono::Utc::now().timestamp_millis();
    let payload = crate::shared::signed_fetch::build_payload_v6("get", path, &ts.to_string(), "{}");
    frame_signed(
        who,
        &PrivateKeySigner::random(),
        FAR_EXPIRATION,
        ts,
        &payload,
        "{}",
    )
    .await
    .1
}

/// A stream socket authenticated on the upgrade request.
async fn with_headers(
    server: &Server,
    frame: &str,
) -> Result<WsStream, tokio_tungstenite::tungstenite::Error> {
    let mut request = format!("ws://{}{STREAM}", server.addr)
        .into_client_request()
        .unwrap();
    let frame: serde_json::Map<String, Value> = serde_json::from_str(frame).unwrap();
    for (name, value) in frame {
        request.headers_mut().insert(
            HeaderName::from_bytes(name.as_bytes()).unwrap(),
            HeaderValue::from_str(value.as_str().unwrap()).unwrap(),
        );
    }
    tokio_tungstenite::connect_async(request)
        .await
        .map(|(socket, _)| socket)
}

async fn next_json(socket: &mut WsStream) -> Value {
    match next_message(socket).await {
        WsMessage::Text(text) => serde_json::from_str(&text).unwrap(),
        other => panic!("not a JSON frame: {other:?}"),
    }
}

async fn send_json(socket: &mut WsStream, value: Value) {
    socket
        .send(WsMessage::Text(value.to_string().into()))
        .await
        .unwrap();
}

/// A ready stream socket of `who`.
async fn ready(server: &Server, who: &PrivateKeySigner) -> WsStream {
    let mut socket = with_headers(server, &signed_frame(who, "/v1/stream").await)
        .await
        .unwrap();
    assert_eq!(next_json(&mut socket).await, json!({ "type": "ready" }));
    socket
}

/// The close frame `socket` ends with, past whatever comes before it.
async fn closing_frame<S>(socket: &mut S) -> tokio_tungstenite::tungstenite::protocol::CloseFrame
where
    S: futures::Stream<Item = Result<WsMessage, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    tokio::time::timeout(crate::shared::test_wait::HANG_GUARD, async {
        loop {
            match socket.next().await {
                Some(Ok(WsMessage::Close(Some(frame)))) => return frame,
                Some(Ok(WsMessage::Close(None))) | Some(Err(_)) | None => {
                    panic!("the socket ended without a close frame")
                }
                Some(Ok(_)) => continue,
            }
        }
    })
    .await
    .expect("the socket never closed")
}

/// A ready stream socket of `who` whose client receives through a tiny
/// buffer, so what it does not read soon backs up on the server.
async fn ready_with_small_buffer(server: &Server, who: &PrivateKeySigner) -> WsStream {
    let tcp = tokio::net::TcpSocket::new_v4().unwrap();
    tcp.set_recv_buffer_size(1024).unwrap();
    let tcp = tcp.connect(server.addr).await.unwrap();
    let mut request = format!("ws://{}{STREAM}", server.addr)
        .into_client_request()
        .unwrap();
    let frame: serde_json::Map<String, Value> =
        serde_json::from_str(&signed_frame(who, "/v1/stream").await).unwrap();
    for (name, value) in frame {
        request.headers_mut().insert(
            HeaderName::from_bytes(name.as_bytes()).unwrap(),
            HeaderValue::from_str(value.as_str().unwrap()).unwrap(),
        );
    }
    let (mut socket, _) =
        tokio_tungstenite::client_async(request, tokio_tungstenite::MaybeTlsStream::Plain(tcp))
            .await
            .unwrap();
    assert_eq!(next_json(&mut socket).await, json!({ "type": "ready" }));
    socket
}

/// Nothing arrives on `socket` for a while.
async fn silent(socket: &mut WsStream) -> bool {
    tokio::time::timeout(Duration::from_millis(200), socket.next())
        .await
        .is_err()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_subscription_receives_the_update_its_rpc_stream_would() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let server = spawn_server(Some(pool.clone()), 5).await;
    let me = PrivateKeySigner::random();
    let address = format!("{:#x}", me.address());
    let other = addr("str1");
    let ctx = server.state.ctx.clone();
    seed_profiles(&ctx, &[&address, &other]).await;

    let mut socket = ready(&server, &me).await;
    send_json(&mut socket, json!({ "type": "ping" })).await;
    assert_eq!(next_json(&mut socket).await, json!({ "type": "pong" }));
    for (id, stream) in [
        ("blocks", "block_updates"),
        ("friends", "friendship_updates"),
    ] {
        send_json(
            &mut socket,
            json!({ "type": "subscribe", "id": id, "stream": stream }),
        )
        .await;
        assert_eq!(
            next_json(&mut socket).await,
            json!({ "type": "subscribed", "id": id })
        );
    }

    crate::rpc::service::ops::apply_friendship_action(
        &ctx,
        &other,
        &address,
        crate::rpc::service::Action::Request,
        Some("hey"),
    )
    .await
    .unwrap();
    let event = next_json(&mut socket).await;
    assert_eq!(
        (&event["type"], &event["id"], &event["stream"]),
        (
            &json!("event"),
            &json!("friends"),
            &json!("friendship_updates")
        )
    );
    assert_eq!(event["data"]["action"], "request");
    assert_eq!(event["data"]["address"], other);
    assert_eq!(event["data"]["message"], "hey");
    assert!(event["data"]["id"].is_string() && event["data"]["createdAt"].is_i64());

    crate::rpc::service::ops::block(&ctx, &other, &address)
        .await
        .unwrap();
    let mut seen = Vec::new();
    for _ in 0..2 {
        seen.push(next_json(&mut socket).await);
    }
    assert!(seen.contains(&json!({
        "type": "event", "id": "blocks", "stream": "block_updates",
        "data": { "address": other, "isBlocked": true }
    })));
    assert!(seen.contains(&json!({
        "type": "event", "id": "friends", "stream": "friendship_updates",
        "data": { "action": "block", "address": other }
    })));

    send_json(
        &mut socket,
        json!({ "type": "unsubscribe", "id": "blocks" }),
    )
    .await;
    assert_eq!(
        next_json(&mut socket).await,
        json!({ "type": "end", "id": "blocks", "reason": "unsubscribed" })
    );
    send_json(
        &mut socket,
        json!({ "type": "unsubscribe", "id": "friends" }),
    )
    .await;
    assert_eq!(next_json(&mut socket).await["reason"], "unsubscribed");
    crate::rpc::service::ops::unblock(&ctx, &other, &address)
        .await
        .unwrap();
    assert!(silent(&mut socket).await, "nothing follows an end");

    // The protocol's errors keep the socket open.
    for (frame, expected) in [
        (
            json!({ "type": "subscribe", "id": "x", "stream": "nope" }),
            json!({ "type": "error", "id": "x", "code": "unknown_stream", "message": "Unknown stream" }),
        ),
        (
            json!({ "type": "unsubscribe", "id": "gone" }),
            json!({ "type": "error", "id": "gone", "code": "unknown_subscription", "message": "No subscription with this id" }),
        ),
        (
            json!({ "type": "hello" }),
            json!({ "type": "error", "code": "invalid_message", "message": "Unknown message type" }),
        ),
    ] {
        send_json(&mut socket, frame).await;
        assert_eq!(next_json(&mut socket).await, expected);
    }
    send_json(
        &mut socket,
        json!({ "type": "subscribe", "id": "a", "stream": "block_updates" }),
    )
    .await;
    assert_eq!(next_json(&mut socket).await["type"], "subscribed");
    send_json(
        &mut socket,
        json!({ "type": "subscribe", "id": "a", "stream": "friendship_updates" }),
    )
    .await;
    assert_eq!(
        next_json(&mut socket).await["message"],
        "A subscription with this id already exists"
    );
    send_json(
        &mut socket,
        json!({ "type": "subscribe", "id": "b", "stream": "block_updates" }),
    )
    .await;
    assert_eq!(
        next_json(&mut socket).await["message"],
        "Already subscribed to this stream on this connection"
    );

    cleanup_pair(ctx.db(), &address, &other).await;
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

#[tokio::test]
async fn both_handshakes_are_over_the_stream_route_only() {
    let server = spawn_server(None, 5).await;
    let me = PrivateKeySigner::random();

    // A chain signed for the RPC socket opens neither handshake here.
    let refused = with_headers(&server, &signed_frame(&me, "/").await).await;
    match refused {
        Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
            assert_eq!(response.status(), 401)
        }
        other => panic!("{:?}", other.map(|_| ())),
    }
    let mut socket = connect(&server, STREAM).await;
    socket
        .send(WsMessage::Text(signed_frame(&me, "/").await.into()))
        .await
        .unwrap();
    assert_eq!(close_code(&next_message(&mut socket).await), Some(3003));

    // The first-frame fallback signs the source protocol path.
    for path in ["/v1/stream"] {
        let mut socket = connect(&server, STREAM).await;
        socket
            .send(WsMessage::Text(signed_frame(&me, path).await.into()))
            .await
            .unwrap();
        assert_eq!(
            next_json(&mut socket).await,
            json!({ "type": "ready" }),
            "{path}"
        );
    }
    // Upgrade headers use that same source protocol path.
    let mut socket = with_headers(&server, &signed_frame(&me, "/v1/stream").await)
        .await
        .unwrap();
    assert_eq!(next_json(&mut socket).await, json!({ "type": "ready" }));
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

#[tokio::test]
async fn no_chain_within_the_window_closes_3004() {
    let server = spawn_server(None, 1).await;
    let mut socket = connect(&server, STREAM).await;
    assert_eq!(close_code(&next_message(&mut socket).await), Some(3004));
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

/// Upstream's `WS_MAX_CONNECTIONS_PER_ADDRESS` (8) across both protocols:
/// the ninth authenticated socket of an address ends its oldest, here an RPC
/// socket, and the next one after that a stream socket with `3005`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_ninth_socket_of_an_address_ends_its_oldest() {
    let server = spawn_server(None, 5).await;
    let me = PrivateKeySigner::random();
    let address = format!("{:#x}", me.address());
    let admission = server.state.ctx.admission();

    let mut rpc = connect(&server, "/social-rpc").await;
    let (_, frame) = crate::rpc::handshake::test_support::frame_for(
        &me,
        "/",
        chrono::Utc::now().timestamp_millis(),
    )
    .await;
    rpc.send(WsMessage::Text(frame.into())).await.unwrap();
    assert!(matches!(next_message(&mut rpc).await, WsMessage::Binary(_)));
    assert!(
        crate::shared::test_wait::until(async || admission.sockets_of(&address) == 1).await,
        "the RPC socket holds a slot"
    );

    let mut streams = Vec::new();
    for _ in 0..crate::rpc::limits::DEFAULT_MAX_SOCKETS_PER_ADDRESS {
        streams.push(ready(&server, &me).await);
    }
    // The displaced RPC socket gets the close a stream socket gets.
    let close = closing_frame(&mut rpc).await;
    assert_eq!(
        (u16::from(close.code), close.reason.as_str()),
        (3005, "Replaced by a newer connection")
    );
    assert_eq!(admission.sockets_of(&address), 8);

    let _ninth = ready(&server, &me).await;
    let close = next_message(&mut streams[0]).await;
    assert_eq!(close_code(&close), Some(3005));
    let WsMessage::Close(Some(frame)) = close else {
        unreachable!()
    };
    assert_eq!(frame.reason.as_str(), "Replaced by a newer connection");
    assert_eq!(admission.sockets_of(&address), 8);

    // Another address is unaffected.
    let _other = ready(&server, &PrivateKeySigner::random()).await;
    assert!(silent(&mut streams[1]).await);
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

/// Upstream `safeSend` (`src/adapters/stream-sessions/component.ts:110-131`):
/// a frame sent while more than `WS_MAX_BACKPRESSURE` bytes wait to be
/// written closes the socket 1008 "Slow consumer". Here a client that sends
/// pings and reads none of the pongs.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_send_past_the_backpressure_limit_closes_slow_consumer() {
    let settings = Settings {
        auth_window_secs: 5,
        ws_max_backpressure_bytes: 1024,
        stream: crate::rpc::stream::StreamLimits {
            messages_per_second: 1e9,
            message_burst: 1_000_000_000,
            ..Default::default()
        },
        ..Settings::default()
    };
    let server = spawn_server_with(None, settings, None).await;
    let me = PrivateKeySigner::random();
    let address = format!("{:#x}", me.address());
    let socket = ready(&server, &me).await;
    let (mut sink, mut stream) = socket.split();
    // Stop sending once the server retires the session. Writing after the peer
    // closes can put tungstenite in an error state before its read half exposes
    // the close frame, which measures a client-side write failure instead.
    for _ in 0..2_000_000 {
        if server.state.ctx.admission().sockets_of(&address) == 0 {
            break;
        }
        let ping = sink.send(WsMessage::Text(r#"{"type":"ping"}"#.into()));
        match tokio::time::timeout(Duration::from_millis(200), ping).await {
            Ok(Ok(())) => continue,
            _ => break,
        }
    }
    let close = closing_frame(&mut stream).await;
    assert_eq!(
        (u16::from(close.code), close.reason.as_str()),
        (1008, "Slow consumer")
    );
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

/// Upstream's per-subscription queue (`src/utils/emitterToGenerator.ts:4`,
/// `:37-45`, `stream-sessions/component.ts:303-304`): a subscription whose
/// client stops reading waits for the socket to drain, and once 1,000 of
/// its events wait the socket closes 1008 "Subscription overflow; reconnect
/// and resync". Events of streams it does not listen to never fill it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_full_subscription_queue_closes_subscription_overflow() {
    let server = spawn_server(None, 5).await;
    let me = PrivateKeySigner::random();
    let address = format!("{:#x}", me.address());
    let mut socket = ready_with_small_buffer(&server, &me).await;
    send_json(
        &mut socket,
        json!({ "type": "subscribe", "id": "b", "stream": "block_updates" }),
    )
    .await;
    assert_eq!(
        next_json(&mut socket).await,
        json!({ "type": "subscribed", "id": "b" })
    );
    let pubsub = server.state.ctx.pubsub().clone();
    let community = || {
        SocialEvent::CommunityVoice(CommunityVoiceChatUpdate {
            community_id: "c".into(),
            ..Default::default()
        })
    };
    for _ in 0..20 {
        for _ in 0..100 {
            pubsub.publish(&address, community());
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(
        silent(&mut socket).await,
        "another stream's events reach nothing"
    );
    let (_sink, mut stream) = socket.split();
    let mut published = 0;
    while !server.state.ctx.admission().sockets_of(&address).eq(&0) && published < 1_000_000 {
        pubsub.publish(
            &address,
            SocialEvent::Block(BlockUpdate {
                address: addr("blocker"),
                is_blocked: true,
                ..Default::default()
            }),
        );
        published += 1;
        if published % 64 == 0 {
            tokio::task::yield_now().await;
        }
    }
    let close = closing_frame(&mut stream).await;
    assert_eq!(
        (u16::from(close.code), close.reason.as_str()),
        (1008, "Subscription overflow; reconnect and resync")
    );
    server.jobs.shutdown(Duration::from_secs(2)).await;
}
