//! The socket: an axum WebSocket upgrade, counted from the moment it opens
//! (closed `1013` when the pool is full), whose first frame must be the
//! signed handshake (`handshake.rs`) within the auth window: `3004
//! Authorization timeout` when none arrives, `3003 Unauthorized` when it is
//! refused, and any frame sent while it is verified answered with upstream's
//! `{"error":"Authentication already in progress, please try again later"}`.
//! A socket that closes during verification is never attached. After it the
//! socket is a binary [`Transport`](crate::shared::rpc::drpc::transport::Transport)
//! attached to the RPC server, which answers `SERVER_READY` and serves the
//! ports the client creates (upstream `src/controllers/handlers/uws/ws-handler.ts`).

use std::sync::Arc;

use anyhow::{Result, anyhow};
use axum::extract::State as AxumState;
use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, Uri};
use axum::response::{IntoResponse, Response};

use crate::rpc::State;
use crate::rpc::limits::Protocol;
use crate::shared::identity::Address;
use crate::shared::rpc::handshake::{HandshakeError, validator_ref, verify_handshake_number_ms};
use crate::shared::rpc::ws::{
    AxumWsTransport, IdleAction, IdleTimer, RPC_TRANSPORT_CLOSED, SocketClose, close_message,
};

/// The pool is full (upstream `ws-handler.ts:367`).
pub const POOL_FULL: (u16, &str) = (1013, "Unable to acquire connection");
pub const UNAUTHORIZED: (u16, &str) = (3003, "Unauthorized");
pub const AUTH_TIMEOUT: (u16, &str) = (3004, "Authorization timeout");
const AUTH_IN_PROGRESS: &str =
    r#"{"error":"Authentication already in progress, please try again later"}"#;

pub async fn ws_upgrade(
    AxumState(state): AxumState<Arc<State>>,
    headers: HeaderMap,
    _uri: Uri,
    ws: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Response {
    // upstream's `app.ws('/')` takes upgrades only; a request that is not one
    // (no `Sec-WebSocket-Key`) falls to its `ANY /*`. A broken upgrade keeps
    // axum's answer (400, 405 or 426): what uWS answers it is not in the
    // reference's source.
    let ws = match ws {
        Ok(ws) => ws,
        Err(_) if !headers.contains_key(axum::http::header::SEC_WEBSOCKET_KEY) => {
            return super::not_found().await;
        }
        Err(rejection) => return rejection.into_response(),
    };
    let paths = vec!["/".to_owned()];
    ws.max_message_size(state.settings.ws_max_payload_bytes)
        .on_upgrade(move |socket| async move {
            if let Err(err) = handle_connection(state, paths, socket).await {
                tracing::warn!(error = %err, "social-rpc ws connection ended");
            }
        })
        .into_response()
}

async fn handle_connection(
    state: Arc<State>,
    paths: Vec<String>,
    mut socket: WebSocket,
) -> Result<()> {
    let max = state.settings.ws_max_concurrent_connections;
    let Some(slot) = state.ctx.open_socket(Protocol::Rpc, max) else {
        tracing::warn!(max, "rejecting ws connection: pool is full");
        let _ = socket.send(close_message(POOL_FULL)).await;
        return Ok(());
    };
    let idle = IdleTimer::new(state.settings.ws_idle_timeout_secs);
    let Some(address) = auth_handshake(&state, &paths, &mut socket, &slot.close, &idle).await
    else {
        return Ok(());
    };
    tracing::info!(address = %address, "social-rpc client authenticated");

    let Some(events) = state.rpc_events() else {
        return Err(anyhow!(
            "rpc server not running; State::spawn_jobs was not called"
        ));
    };
    let (transport, writer) = AxumWsTransport::spawn_with_ordinary_queue(
        socket,
        address.into_string(),
        slot.close.clone(),
        idle,
        state.settings.ws_max_backpressure_bytes,
        state.settings.ordinary_transport,
    );
    if let Err(e) = events.send_attach_transport(Arc::new(transport)) {
        let (code, reason) = RPC_TRANSPORT_CLOSED;
        slot.close.end(code, reason);
        return Err(anyhow!("attach transport: {e:?}"));
    }
    // The socket keeps its place in the pool until its close frame is out.
    let _ = writer.await;
    Ok(())
}

/// The first frame, verified; `None` once the socket is closed (with the
/// frame upstream sends, or by its peer).
async fn auth_handshake(
    state: &State,
    paths: &[String],
    socket: &mut WebSocket,
    close: &SocketClose,
    idle: &IdleTimer,
) -> Option<Address> {
    let deadline = tokio::time::sleep(state.settings.auth_timeout());
    tokio::pin!(deadline);
    let frame = loop {
        tokio::select! {
            _ = close.ended() => return end(socket, close.frame()).await,
            action = idle.next() => match action {
                IdleAction::Ping => { if socket.send(Message::Ping(Vec::new().into())).await.is_err() { return None; } },
                IdleAction::Expired => { close.peer_gone(); return None; },
            },
            _ = &mut deadline => return end(socket, Some(AUTH_TIMEOUT)).await,
            message = socket.recv() => match message {
                Some(Ok(Message::Text(text))) => { idle.activity(); break text.to_string(); },
                Some(Ok(Message::Binary(bytes))) => { idle.activity(); break String::from_utf8_lossy(&bytes).into_owned(); },
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => { idle.activity(); continue; },
                _ => return None,
            },
        }
    };
    let verification = verify_handshake_number_ms(
        &frame,
        "get",
        paths,
        state.settings.auth_signature_expiration_ms(),
        chrono::Utc::now().timestamp_millis(),
        validator_ref(&state.eip1654),
    );
    tokio::pin!(verification);
    // Upstream's timer runs until the verdict, and a frame sent meanwhile is
    // answered and dropped (`ws-handler.ts:374-384`).
    // Biased: a frame or a close already received is handled before the verdict.
    let verdict = loop {
        tokio::select! {
            biased;
            _ = close.ended() => return end(socket, close.frame()).await,
            action = idle.next() => match action {
                IdleAction::Ping => { if socket.send(Message::Ping(Vec::new().into())).await.is_err() { return None; } },
                IdleAction::Expired => { close.peer_gone(); return None; },
            },
            message = socket.recv() => match message {
                Some(Ok(Message::Text(_) | Message::Binary(_))) => {
                    idle.activity();
                    let _ = socket.send(Message::Text(AUTH_IN_PROGRESS.into())).await;
                }
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => { idle.activity(); }
                _ => {
                    tracing::info!("socket closed during authentication; not attached");
                    return None;
                }
            },
            _ = &mut deadline => return end(socket, Some(AUTH_TIMEOUT)).await,
            verdict = &mut verification => break verdict,
        }
    };
    match verdict {
        Ok(address) => Some(address),
        Err(err) => {
            let kind = match err {
                HandshakeError::CatalystUnavailable(_) => "server_error",
                _ => "client_rejected",
            };
            metrics::counter!(
                "ws_auth_errors_by_protocol",
                "protocol" => Protocol::Rpc.label(),
                "type" => kind
            )
            .increment(1);
            tracing::info!(%err, "auth handshake failed");
            end(socket, Some(UNAUTHORIZED)).await
        }
    }
}

async fn end(socket: &mut WebSocket, frame: Option<(u16, &'static str)>) -> Option<Address> {
    if let Some(frame) = frame {
        let _ = socket.send(close_message(frame)).await;
    }
    None
}
