//! The socket's handshake and lifetime as upstream's `ws-handler.spec.ts`
//! (`decentraland/social-service-ea` at `fb3dcb8`) drives them, over a real
//! WebSocket: the frames the explorer could send first, what closes and
//! with which code, and what an authenticated socket keeps.

use super::*;

async fn first_answer(server: &Server, frame: &str) -> WsMessage {
    let mut socket = connect(server, "/social-rpc").await;
    socket
        .send(WsMessage::Text(frame.to_string().into()))
        .await
        .unwrap();
    next_message(&mut socket).await
}

fn is_server_ready(msg: &WsMessage) -> bool {
    let WsMessage::Binary(b) = msg else {
        return false;
    };
    matches!(
        super::super::drpc::rpc_protocol::parse::parse_header(b),
        Some((
            super::super::drpc::rpc_protocol::RpcMessageTypes::ServerReady,
            _
        ))
    )
}

/// A signed frame with one field replaced by `value` (or added).
async fn valid_frame_with(key: &str, value: serde_json::Value) -> String {
    let (_, frame) = frame("/", chrono::Utc::now().timestamp_millis()).await;
    let mut frame: serde_json::Value = serde_json::from_str(&frame).unwrap();
    frame[key] = value;
    frame.to_string()
}

/// Upstream's `describe.each` of malformed first frames: each closes 3003
/// before any verification, never an internal error. An `x-identity-*`
/// value that is neither a string nor an array of strings is one of them,
/// metadata included (`ws-handler.ts:139-146`).
#[tokio::test]
async fn a_malformed_first_frame_closes_unauthorized() {
    let server = spawn_server(None, 5).await;
    for frame in ["", "null", "123", "\"x\"", "[]", "{}"] {
        assert_eq!(
            close_code(&first_answer(&server, frame).await),
            Some(3003),
            "{frame:?}"
        );
    }
    for (key, value) in [
        (
            "x-identity-timestamp",
            serde_json::json!(1_700_000_000_000_i64),
        ),
        (
            "x-identity-auth-chain-0",
            serde_json::json!({ "type": "SIGNER" }),
        ),
        ("x-identity-metadata", serde_json::json!({})),
        ("x-identity-metadata", serde_json::Value::Null),
        ("x-identity-auth-chain-1", serde_json::json!(["{}", 1])),
        ("X-Identity-Foo", serde_json::json!(1)),
        ("X-IDENTITY-METADATA", serde_json::json!({})),
    ] {
        let frame = valid_frame_with(key, value).await;
        assert_eq!(
            close_code(&first_answer(&server, &frame).await),
            Some(3003),
            "{key}"
        );
    }
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

/// `verify()` reads only the exact lowercase keys (`@dcl/crypto-middleware`
/// 6.3.0 `dist/verify.js:72`, `:321-322`): a re-cased duplicate next to the
/// lowercase header passes, and a timestamp sent only as
/// `X-Identity-Timestamp` is absent, so the socket closes 3003.
#[tokio::test]
async fn a_re_cased_identity_header_is_never_read_on_the_socket() {
    let server = spawn_server(None, 5).await;
    for (key, value) in [
        ("X-Identity-Timestamp", serde_json::json!([])),
        ("X-Identity-Timestamp", serde_json::json!("0")),
    ] {
        let frame = valid_frame_with(key, value.clone()).await;
        let answer = first_answer(&server, &frame).await;
        assert!(is_server_ready(&answer), "{key} {value}: {answer:?}");
    }
    let (_, frame) = frame("/", chrono::Utc::now().timestamp_millis()).await;
    let mut frame: serde_json::Value = serde_json::from_str(&frame).unwrap();
    let ts = frame
        .as_object_mut()
        .unwrap()
        .remove("x-identity-timestamp")
        .unwrap();
    frame["X-Identity-Timestamp"] = ts;
    assert_eq!(
        close_code(&first_answer(&server, &frame.to_string()).await),
        Some(3003)
    );
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

/// A field outside `x-identity-*` is left alone: the chain still verifies.
#[tokio::test]
async fn a_field_outside_the_identity_headers_does_not_stop_the_handshake() {
    let server = spawn_server(None, 5).await;
    for value in [
        serde_json::json!(2),
        serde_json::json!({ "nested": [1] }),
        serde_json::Value::Null,
    ] {
        let frame = valid_frame_with("version", value.clone()).await;
        let answer = first_answer(&server, &frame).await;
        assert!(is_server_ready(&answer), "{value}: {answer:?}");
    }
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

/// A contract wallet's chain whose validation cannot reach the chain (upstream:
/// the catalyst's 503) closes 3003 like any other refusal.
#[tokio::test]
async fn an_unreachable_contract_validator_closes_unauthorized() {
    static ASKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    struct Unreachable;
    #[async_trait::async_trait]
    impl crate::auth::eip1654::Eip1654Validator for Unreachable {
        async fn validate_signature(
            &self,
            _contract: &str,
            _hash: &[u8],
            _signature: &[u8],
        ) -> Result<bool, crate::auth::AuthError> {
            ASKED.store(true, std::sync::atomic::Ordering::SeqCst);
            Err(crate::auth::AuthError::Eip1654ValidationFailed(
                "unreachable".into(),
            ))
        }
    }
    let settings = Settings {
        auth_window_secs: 5,
        ..Settings::default()
    };
    let server = spawn_server_with(None, settings, Some(Arc::new(Unreachable))).await;
    let ts = chrono::Utc::now().timestamp_millis().to_string();
    let link = |kind: &str, payload: &str, signature: &str| {
        serde_json::json!({ "type": kind, "payload": payload, "signature": signature }).to_string()
    };
    let frame = serde_json::json!({
        "x-identity-auth-chain-0": link("SIGNER", &format!("0x{}", "ab".repeat(20)), ""),
        "x-identity-auth-chain-1": link(
            "ECDSA_EIP_1654_EPHEMERAL",
            &format!("Decentraland Login\nEphemeral address: 0x{}\nExpiration: 2099-01-01T00:00:00.000Z", "cd".repeat(20)),
            &format!("0x{}", "11".repeat(65)),
        ),
        "x-identity-auth-chain-2": link(
            "ECDSA_SIGNED_ENTITY",
            &crate::shared::signed_fetch::build_payload_v6("get", "/", &ts, "{}"),
            &format!("0x{}", "22".repeat(65)),
        ),
        "x-identity-timestamp": ts,
        "x-identity-metadata": "{}",
    });
    assert_eq!(
        close_code(&first_answer(&server, &frame.to_string()).await),
        Some(3003)
    );
    assert!(
        ASKED.load(std::sync::atomic::Ordering::SeqCst),
        "the refusal came before the validator"
    );
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

/// Each field as an array of strings authenticates: an array reads as its
/// first element (`firstOf`, `@dcl/crypto-middleware` 6.3.0 `dist/verify.js`).
#[tokio::test]
async fn an_array_of_strings_authenticates() {
    let server = spawn_server(None, 5).await;
    let (_, frame) = frame("/", chrono::Utc::now().timestamp_millis()).await;
    let mut frame: serde_json::Value = serde_json::from_str(&frame).unwrap();
    for value in frame.as_object_mut().unwrap().values_mut() {
        *value = serde_json::json!([value.clone()]);
    }
    let answer = first_answer(&server, &frame.to_string()).await;
    assert!(is_server_ready(&answer), "{answer:?}");
    // The metadata alone as an array passes the pre-check; its first element
    // is the signed `{}`, whatever follows it.
    for metadata in [
        serde_json::json!(["{}"]),
        serde_json::json!(["{}", "ignored"]),
    ] {
        let frame = valid_frame_with("x-identity-metadata", metadata.clone()).await;
        let answer = first_answer(&server, &frame).await;
        assert!(is_server_ready(&answer), "{metadata}: {answer:?}");
    }
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

/// A chain signed outside the freshness window closes 3003 on the socket.
#[tokio::test]
async fn an_expired_chain_closes_unauthorized() {
    let server = spawn_server(None, 5).await;
    let stale = chrono::Utc::now().timestamp_millis() - 10 * 60 * 1000;
    let (_, frame) = frame("/", stale).await;
    assert_eq!(close_code(&first_answer(&server, &frame).await), Some(3003));
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

/// The handshake timeout bounds only the wait for the first frame: an
/// authenticated socket idle past it stays open.
#[tokio::test]
async fn an_authenticated_socket_outlives_the_handshake_timeout() {
    let server = spawn_server(None, 1).await;
    let mut socket = connect(&server, "/social-rpc").await;
    let (_, frame) = frame("/", chrono::Utc::now().timestamp_millis()).await;
    socket.send(WsMessage::Text(frame.into())).await.unwrap();
    assert!(is_server_ready(&next_message(&mut socket).await));
    match tokio::time::timeout(Duration::from_millis(2500), socket.next()).await {
        Err(_) => {}
        Ok(Some(Ok(msg))) => assert!(close_code(&msg).is_none() && !msg.is_close(), "{msg:?}"),
        Ok(other) => panic!("the authenticated socket ended: {other:?}"),
    }
    assert_eq!(server.state.ctx.live_connections(), 1);
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

/// Closing an authenticated socket gives its slot back.
#[tokio::test]
async fn closing_an_authenticated_socket_releases_its_connection() {
    let server = spawn_server(None, 5).await;
    let mut socket = connect(&server, "/social-rpc").await;
    let (_, frame) = frame("/", chrono::Utc::now().timestamp_millis()).await;
    socket.send(WsMessage::Text(frame.into())).await.unwrap();
    assert!(is_server_ready(&next_message(&mut socket).await));
    assert_eq!(server.state.ctx.live_connections(), 1);
    socket.close(None).await.unwrap();
    assert!(
        crate::shared::test_wait::until(async || server.state.ctx.live_connections() == 0).await,
        "the closed socket still counts"
    );
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

/// Opens a socket and authenticates it as a wallet made for the call.
async fn authenticated(server: &Server) -> WsStream {
    let mut socket = connect(server, "/social-rpc").await;
    let ts = chrono::Utc::now().timestamp_millis();
    let payload = crate::shared::signed_fetch::build_payload_v6("get", "/", &ts.to_string(), "{}");
    let (_, frame) = frame_signed(
        &PrivateKeySigner::random(),
        &PrivateKeySigner::random(),
        FAR_EXPIRATION,
        ts,
        &payload,
        "{}",
    )
    .await;
    socket.send(WsMessage::Text(frame.into())).await.unwrap();
    assert!(is_server_ready(&next_message(&mut socket).await));
    socket
}

/// Upstream's `ws-pool-component.spec.ts` "registering multiple connections":
/// each socket counts, and closing one frees only its own slot.
#[tokio::test]
async fn each_socket_counts_and_closing_one_frees_its_slot() {
    let server = spawn_server(None, 5).await;
    let mut first = authenticated(&server).await;
    let _second = authenticated(&server).await;
    assert!(
        crate::shared::test_wait::until(async || server.state.ctx.live_connections() == 2).await,
        "two sockets, two slots"
    );
    first.close(None).await.unwrap();
    assert!(
        crate::shared::test_wait::until(async || server.state.ctx.live_connections() == 1).await,
        "closing one frees one"
    );
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

/// Upstream's pool cap (`src/logic/ws-pool/component.ts:26-29`,
/// `src/controllers/handlers/uws/ws-handler.ts:367`): with the pool full a new
/// socket closes 1013 "Unable to acquire connection", and a freed slot is
/// taken again.
#[tokio::test]
async fn a_full_pool_closes_a_new_socket_until_a_slot_frees() {
    let settings = Settings {
        auth_window_secs: 5,
        ws_max_concurrent_connections: Some(1),
        ..Settings::default()
    };
    let server = spawn_server_with(None, settings, None).await;
    let mut first = authenticated(&server).await;
    assert!(
        crate::shared::test_wait::until(async || server.state.ctx.live_connections() == 1).await
    );
    let mut refused = connect(&server, "/social-rpc").await;
    match next_message(&mut refused).await {
        WsMessage::Close(Some(frame)) => {
            assert_eq!(u16::from(frame.code), 1013);
            assert_eq!(frame.reason.as_str(), "Unable to acquire connection");
        }
        other => panic!("expected a close, got {other:?}"),
    }
    first.close(None).await.unwrap();
    assert!(
        crate::shared::test_wait::until(async || server.state.ctx.live_connections() == 0).await
    );
    let _again = authenticated(&server).await;
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

/// A verification that waits for the test: the contract wallet's link is
/// accepted only once `release` is notified.
#[derive(Default)]
struct Gate {
    asked: tokio::sync::Notify,
    release: tokio::sync::Notify,
}

#[async_trait::async_trait]
impl crate::auth::eip1654::Eip1654Validator for Gate {
    async fn validate_signature(
        &self,
        _contract: &str,
        _hash: &[u8],
        _signature: &[u8],
    ) -> Result<bool, crate::auth::AuthError> {
        self.asked.notify_one();
        self.release.notified().await;
        Ok(true)
    }
}

/// A server whose handshakes wait on the returned [`Gate`].
async fn gated_server(auth_window_secs: i64) -> (Server, Arc<Gate>) {
    let gate = Arc::new(Gate::default());
    let settings = Settings {
        auth_window_secs,
        ..Settings::default()
    };
    (
        spawn_server_with(None, settings, Some(gate.clone())).await,
        gate,
    )
}

/// A frame of `who`'s as a contract wallet: its ephemeral link is the
/// validator's to accept, then the entity link verifies as usual.
async fn contract_wallet_frame(who: &PrivateKeySigner) -> String {
    let (_, frame) = crate::rpc::handshake::test_support::frame_for(
        who,
        "/",
        chrono::Utc::now().timestamp_millis(),
    )
    .await;
    let mut frame: serde_json::Value = serde_json::from_str(&frame).unwrap();
    let mut link: serde_json::Value =
        serde_json::from_str(frame["x-identity-auth-chain-1"].as_str().unwrap()).unwrap();
    link["type"] = "ECDSA_EIP_1654_EPHEMERAL".into();
    frame["x-identity-auth-chain-1"] = link.to_string().into();
    frame.to_string()
}

/// Upstream's "reject messages during authentication"
/// (`ws-handler.ts:374-384`): a frame sent while the chain is verified is
/// answered with the in-progress error and dropped; the verdict follows.
#[tokio::test]
async fn a_frame_during_authentication_is_answered_and_dropped() {
    let (server, gate) = gated_server(5).await;
    let mut socket = connect(&server, "/social-rpc").await;
    let frame = contract_wallet_frame(&PrivateKeySigner::random()).await;
    socket.send(WsMessage::Text(frame.into())).await.unwrap();
    gate.asked.notified().await;
    for extra in [
        WsMessage::Text("hello".into()),
        WsMessage::Binary(vec![1, 2, 3].into()),
    ] {
        socket.send(extra).await.unwrap();
        assert_eq!(
            next_message(&mut socket).await,
            WsMessage::Text(
                r#"{"error":"Authentication already in progress, please try again later"}"#.into()
            )
        );
    }
    gate.release.notify_one();
    assert!(is_server_ready(&next_message(&mut socket).await));
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

/// Upstream's "abort user attachment" (`ws-handler.ts:124-133`): a socket
/// that closes while its chain is verified is never attached, and its place
/// in the pool is freed.
#[tokio::test]
async fn a_socket_closed_during_verification_is_never_attached() {
    let (server, gate) = gated_server(5).await;
    let me = PrivateKeySigner::random();
    let address = format!("{:#x}", me.address());
    let mut socket = connect(&server, "/social-rpc").await;
    socket
        .send(WsMessage::Text(contract_wallet_frame(&me).await.into()))
        .await
        .unwrap();
    gate.asked.notified().await;
    socket.close(None).await.unwrap();
    assert!(
        crate::shared::test_wait::until(async || server.state.ctx.live_connections() == 0).await,
        "the closed socket still counts"
    );
    gate.release.notify_one();
    assert_eq!(server.state.ctx.admission().sockets_of(&address), 0);
    assert!(server.state.ctx.presence_snapshot().is_empty());
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

/// Upstream's auth timer runs until the verdict (`ws-handler.ts:117-120`,
/// `:331-338`): a verification still running when the window ends closes
/// the socket 3004 "Authorization timeout".
#[tokio::test]
async fn the_auth_window_runs_through_verification() {
    let (server, gate) = gated_server(1).await;
    let mut socket = connect(&server, "/social-rpc").await;
    let frame = contract_wallet_frame(&PrivateKeySigner::random()).await;
    socket.send(WsMessage::Text(frame.into())).await.unwrap();
    gate.asked.notified().await;
    match next_message(&mut socket).await {
        WsMessage::Close(Some(frame)) => {
            assert_eq!(u16::from(frame.code), 3004);
            assert_eq!(frame.reason.as_str(), "Authorization timeout");
        }
        other => panic!("expected a close, got {other:?}"),
    }
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

/// Upstream `ws-handler.ts:323`: a socket counts from the moment it opens,
/// before any frame, so with the pool full an unauthenticated socket keeps
/// the next one out.
#[tokio::test]
async fn a_socket_counts_from_the_moment_it_opens() {
    let settings = Settings {
        auth_window_secs: 5,
        ws_max_concurrent_connections: Some(1),
        ..Settings::default()
    };
    let server = spawn_server_with(None, settings, None).await;
    let mut silent = connect(&server, "/social-rpc").await;
    assert!(
        crate::shared::test_wait::until(async || server.state.ctx.live_connections() == 1).await,
        "an open socket counts before its handshake"
    );
    let mut refused = connect(&server, "/social-rpc/v1/stream").await;
    assert_eq!(close_code(&next_message(&mut refused).await), Some(1013));
    silent.close(None).await.unwrap();
    assert!(
        crate::shared::test_wait::until(async || server.state.ctx.live_connections() == 0).await
    );
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

/// Upstream's `ws-pool` stop (`src/logic/ws-pool/component.ts:77-90`): at
/// shutdown every socket, of either protocol and authenticated or not, ends
/// with 1001 "Server shutting down".
#[tokio::test]
async fn shutdown_ends_every_socket_with_1001() {
    let server = spawn_server(None, 5).await;
    let mut sockets = vec![
        authenticated(&server).await,
        connect(&server, "/social-rpc").await,
        connect(&server, "/social-rpc/v1/stream").await,
    ];
    assert!(
        crate::shared::test_wait::until(async || server.state.ctx.live_connections() == 3).await
    );
    let clients = async {
        for socket in &mut sockets {
            match next_message(socket).await {
                WsMessage::Close(Some(frame)) => {
                    assert_eq!(u16::from(frame.code), 1001);
                    assert_eq!(frame.reason.as_str(), "Server shutting down");
                }
                other => panic!("expected a close, got {other:?}"),
            }
            // Drive the peer's queued close acknowledgement while shutdown
            // waits for the stream transport to finish its handshake.
            socket.flush().await.unwrap();
        }
    };
    tokio::join!(server.jobs.shutdown(Duration::from_secs(5)), clients);
    assert_eq!(server.state.ctx.live_connections(), 0);
}

/// Source ws-session.ts configures a millisecond deadline independently from
/// its signed-fetch timestamp window for both protocols.
#[tokio::test]
async fn both_socket_protocols_separate_auth_deadline_from_signature_age() {
    let settings = Settings {
        ws_auth_timeout_ms: Some(100.0),
        ws_auth_signature_expiration_ms: Some(10_000.0),
        ws_idle_timeout_secs: 0,
        ..Default::default()
    };
    let server = spawn_server_with(None, settings, None).await;
    for (path, signed_path) in [
        ("/social-rpc", "/"),
        ("/social-rpc/v1/stream", "/v1/stream"),
    ] {
        let mut socket = connect(&server, path).await;
        let (_, frame) = crate::rpc::handshake::test_support::frame_for(
            &PrivateKeySigner::random(),
            signed_path,
            chrono::Utc::now().timestamp_millis() - 1000,
        )
        .await;
        socket.send(WsMessage::Text(frame.into())).await.unwrap();
        let ready = next_message(&mut socket).await;
        assert!(
            !matches!(ready, WsMessage::Close(_)),
            "a long signature window remains independent: {path}"
        );
        let mut pending = connect(&server, path).await;
        let closed = tokio::time::timeout(Duration::from_secs(1), next_message(&mut pending))
            .await
            .unwrap();
        assert_eq!(close_code(&closed), Some(3004), "{path}");
    }
    server.jobs.shutdown(Duration::from_secs(2)).await;
    let settings = Settings {
        ws_auth_timeout_ms: Some(10_000.0),
        ws_auth_signature_expiration_ms: Some(20.0),
        ws_idle_timeout_secs: 0,
        ..Default::default()
    };
    let server = spawn_server_with(None, settings, None).await;
    for (path, signed_path) in [
        ("/social-rpc", "/"),
        ("/social-rpc/v1/stream", "/v1/stream"),
    ] {
        let mut socket = connect(&server, path).await;
        let (_, frame) = crate::rpc::handshake::test_support::frame_for(
            &PrivateKeySigner::random(),
            signed_path,
            chrono::Utc::now().timestamp_millis() - 1000,
        )
        .await;
        socket.send(WsMessage::Text(frame.into())).await.unwrap();
        assert_eq!(
            close_code(&next_message(&mut socket).await),
            Some(3003),
            "{path}"
        );
    }
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

#[tokio::test]
async fn both_socket_protocols_probe_idle_connections_with_ping() {
    let settings = Settings {
        ws_idle_timeout_secs: 2,
        ..Default::default()
    };
    let server = spawn_server_with(None, settings, None).await;
    for (path, signed_path) in [
        ("/social-rpc", "/"),
        ("/social-rpc/v1/stream", "/v1/stream"),
    ] {
        let mut socket = connect(&server, path).await;
        let (_, frame) = crate::rpc::handshake::test_support::frame_for(
            &PrivateKeySigner::random(),
            signed_path,
            chrono::Utc::now().timestamp_millis(),
        )
        .await;
        socket.send(WsMessage::Text(frame.into())).await.unwrap();
        let _ = next_message(&mut socket).await;
        assert!(
            matches!(next_message(&mut socket).await, WsMessage::Ping(_)),
            "{path}"
        );
        socket
            .send(WsMessage::Pong(Vec::new().into()))
            .await
            .unwrap();
        assert!(
            matches!(next_message(&mut socket).await, WsMessage::Ping(_)),
            "Pong refreshed idle state: {path}"
        );
    }
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

#[test]
fn source_numeric_auth_timer_buckets_remain_independent_from_signature_windows() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/conformance/social/ws-numbers-source-cases.json"
    )))
    .unwrap();
    for case in fixture["auth"].as_array().unwrap() {
        if case.get("error").is_some() {
            continue;
        }
        let number = case["number"]
            .as_f64()
            .unwrap_or_else(|| case["number"].as_str().unwrap().parse().unwrap());
        let settings = Settings {
            ws_auth_timeout_ms: Some(number),
            ws_auth_signature_expiration_ms: Some(number),
            ..Settings::default()
        };
        assert_eq!(
            settings.auth_timeout().as_millis(),
            case["timer_bucket_ms"].as_u64().unwrap() as u128,
            "{}",
            case["raw"]
        );
        assert_eq!(
            settings.auth_signature_expiration_ms(),
            number,
            "{}",
            case["raw"]
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ordinary_large_unary_response_is_preserved_and_a_stopped_reader_closes() {
    ordinary_large_reader_case(false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn one_accepted_stalled_response_still_expires_without_a_retry_head() {
    ordinary_large_reader_case(true).await;
}

async fn ordinary_large_reader_case(idle_only: bool) {
    use std::sync::atomic::Ordering;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let _turn = VOICE_TABLE.lock().await;
    let settings = Settings {
        // Queue capacity uses the raw number; the physical writer wraps to 1024.
        ws_max_backpressure_bytes: 4294968320.5,
        ws_idle_timeout_secs: if idle_only { 8 } else { 300 },
        ordinary_transport: crate::shared::rpc::ws::OrdinaryQueueConfig {
            retry_delay_ms: 10.0,
            max_retry_attempts: 3.0,
            max_backoff_delay_ms: 1000.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let server = spawn_server_with(Some(pool.clone()), settings, None).await;
    let key = PrivateKeySigner::random();
    let me = format!("{:#x}", key.address());
    let tcp = tokio::net::TcpSocket::new_v4().unwrap();
    tcp.set_recv_buffer_size(1024).unwrap();
    let tcp = tcp.connect(server.addr).await.unwrap();
    let (mut socket, _) = tokio_tungstenite::client_async(
        format!("ws://{}/social-rpc", server.addr),
        tokio_tungstenite::MaybeTlsStream::Plain(tcp),
    )
    .await
    .unwrap();
    let (_, frame) = crate::rpc::handshake::test_support::frame_for(
        &key,
        "/",
        chrono::Utc::now().timestamp_millis(),
    )
    .await;
    socket.send(WsMessage::Text(frame.into())).await.unwrap();
    let gate = Arc::new(SocketReadGate::default());
    let mut transport = TungsteniteTransport::new(socket);
    transport.gate = Some(gate.clone());
    let mut client = RpcClient::new(transport).await.unwrap();
    let social: SocialClient = client
        .create_port("ordinary-large")
        .await
        .unwrap()
        .load_module(MODULE_NAME)
        .await
        .unwrap();
    let friend = addr("ordinary-large");
    let entity = seed_profile(&pool, &friend, "Large ordinary response").await;
    let picture = "https://profiles.example/".to_owned() + &"u".repeat(8 * 1024 * 1024);
    let metadata = serde_json::json!({"v":{"avatars":[{"userId":friend,"name":"Large ordinary response","hasClaimedName":true,"avatar":{"snapshots":{"face256":picture}}}]}});
    sqlx::query("UPDATE content.deployments SET entity_metadata=$2 WHERE entity_id=$1")
        .bind(&entity)
        .bind(metadata)
        .execute(&pool)
        .await
        .unwrap();
    server
        .state
        .ctx
        .db()
        .apply_friendship_action(&me, &friend, "accept", true, None, None)
        .await
        .unwrap();
    let got = tokio::time::timeout(Duration::from_secs(10), social.get_friends())
        .await
        .expect("healthy large response");
    assert_eq!(
        got.friends[0].profile_picture_url, picture,
        "ordinary unary frames have no guarded 16KiB frame cap"
    );
    gate.pause_after_response.store(true, Ordering::SeqCst);
    let _ = social.get_friendship_status(&friend).await;
    assert!(gate.paused.load(Ordering::SeqCst));
    let requests = async {
        if idle_only {
            let _ = social
                .module
                .call_unary_procedure::<PaginatedFriendsProfilesResponse, _>(
                    "GetFriends",
                    GetFriendsPayload { pagination: None },
                )
                .await;
        } else {
            let _ = tokio::join!(
                social
                    .module
                    .call_unary_procedure::<PaginatedFriendsProfilesResponse, _>(
                        "GetFriends",
                        GetFriendsPayload { pagination: None }
                    ),
                social
                    .module
                    .call_unary_procedure::<PaginatedFriendsProfilesResponse, _>(
                        "GetFriends",
                        GetFriendsPayload { pagination: None }
                    )
            );
        }
    };
    tokio::pin!(requests);
    let retired = tokio::time::timeout(Duration::from_secs(10), async {
        while server.state.ctx.live_connections() != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });
    tokio::select! {
        result = retired => result.expect("idle expiry or bounded retry exhaustion must retire a stopped socket"),
        _ = &mut requests => panic!("paused reader cannot complete pending calls"),
    }
    gate.paused.store(false, Ordering::SeqCst);
    gate.resume.notify_one();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !gate.closed.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("peer must observe retired transport");
    drop_profile(&pool, &friend, &entity).await;
    cleanup_pair(server.state.ctx.db(), &me, &friend).await;
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

#[tokio::test]
async fn authenticated_text_and_binary_frames_match_source_drpc_exchange() {
    let source: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/conformance/social/ws-frames-source-cases.json"
    )))
    .unwrap();
    assert_eq!(
        source["baseline"]["revision"],
        "b6b2cefefdf727b2ed47ddb887e77d226a41eea6"
    );
    let server = spawn_server(None, 5).await;
    let mut socket = connect(&server, "/social-rpc").await;
    let (_, auth) = frame("/", chrono::Utc::now().timestamp_millis()).await;
    socket.send(WsMessage::Text(auth.into())).await.unwrap();
    let ready = next_message(&mut socket).await;
    let WsMessage::Binary(ready) = ready else {
        panic!("server ready must be binary");
    };
    assert_eq!(
        hex::encode(&ready),
        source["ready"][0]["hex"].as_str().unwrap()
    );
    for case in source["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case.get("binary").is_some())
    {
        let input = hex::decode(case["input_hex"].as_str().unwrap()).unwrap();
        let message = if case["binary"] == true {
            WsMessage::Binary(input.into())
        } else {
            WsMessage::Text(String::from_utf8(input).unwrap().into())
        };
        socket.send(message).await.unwrap();
        let response = next_message(&mut socket).await;
        let WsMessage::Binary(response) = response else {
            panic!("dRPC response must be binary");
        };
        assert_eq!(
            hex::encode(&response),
            case["responses"][0]["hex"].as_str().unwrap()
        );
    }
    socket.close(None).await.unwrap();
    server.jobs.shutdown(Duration::from_secs(2)).await;
}

#[tokio::test]
async fn socket_paths_preserve_source_cases_and_exact_legacy_mounted_aliases() {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let source: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/conformance/social/ws-paths-source-cases.json"
    )))
    .unwrap();
    assert_eq!(
        source["baseline"]["revision"],
        "b6b2cefefdf727b2ed47ddb887e77d226a41eea6"
    );
    let server = spawn_server(None, 5).await;
    for case in source["cases"].as_array().unwrap() {
        let rpc = case["protocol"] == "rpc";
        let mounted = if rpc {
            "/social-rpc"
        } else {
            "/social-rpc/v1/stream"
        };
        let (_, signed) = frame(
            case["signed_path"].as_str().unwrap(),
            chrono::Utc::now().timestamp_millis(),
        )
        .await;
        let mut signed: serde_json::Value = serde_json::from_str(&signed).unwrap();
        let mut request = format!("ws://{}{mounted}", server.addr)
            .into_client_request()
            .unwrap();
        if let Some(original) = case["original_path"].as_str() {
            request
                .headers_mut()
                .insert("x-original-path", original.parse().unwrap());
            signed["x-original-path"] = serde_json::json!(original);
        }
        // Source rejects the mounted spelling. Accepting this exact alias is
        // a compatibility extension for shipped clients, not source equality.
        let legacy_alias = case["signed_path"] == mounted;
        if legacy_alias {
            assert_eq!(case["accepted"], false, "{case}");
        }
        let accepted = case["accepted"] == true || legacy_alias;
        let headers = case["mode"] == "headers";
        if headers {
            for (name, value) in signed.as_object().unwrap() {
                request.headers_mut().insert(
                    name.parse::<tokio_tungstenite::tungstenite::http::HeaderName>()
                        .unwrap(),
                    value.as_str().unwrap().parse().unwrap(),
                );
            }
        }
        let connected = tokio_tungstenite::connect_async(request).await;
        if headers && !accepted {
            match connected {
                Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
                    assert_eq!(response.status(), 401, "{case}")
                }
                other => panic!(
                    "expected unauthorized upgrade for {case}: {:?}",
                    other.map(|_| ())
                ),
            }
            continue;
        }
        let (mut socket, _) = connected.unwrap();
        if !headers {
            socket
                .send(WsMessage::Text(signed.to_string().into()))
                .await
                .unwrap();
        }
        let response = next_message(&mut socket).await;
        if accepted {
            if rpc {
                assert!(is_server_ready(&response), "{case}: {response:?}");
            } else {
                let WsMessage::Text(text) = response else {
                    panic!("expected ready for {case}");
                };
                assert_eq!(
                    serde_json::from_str::<serde_json::Value>(&text).unwrap(),
                    serde_json::json!({"type":"ready"})
                );
            }
            socket.close(None).await.unwrap();
        } else {
            assert_eq!(close_code(&response), Some(3003), "{case}: {response:?}");
        }
    }
    server.jobs.shutdown(Duration::from_secs(2)).await;
}
