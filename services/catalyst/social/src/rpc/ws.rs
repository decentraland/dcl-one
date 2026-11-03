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
use std::time::Duration;

use anyhow::{Result, anyhow};
use axum::extract::State as AxumState;
use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, Uri};
use axum::response::{IntoResponse, Response};
use futures::{Sink, SinkExt};
use tokio::time::Instant;

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
// As in the shared transport, terminal delivery has a separate, bounded
// cleanup budget. It never restarts authentication.
const CLOSE_BUDGET: Duration = Duration::from_millis(100);
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
    if !state.initial_fanout_ready() {
        return axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    // The mounted alias is retained for shipped clients; proxy headers do not
    // choose the signed path. The source protocol path remains accepted.
    let paths = vec!["/".to_owned(), "/social-rpc".to_owned()];
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
        super::metrics::log("ws-handler", "warn");
        metrics::counter!("ws_close_codes", "code" => "1013").increment(1);
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
    let deadline = Instant::now() + state.settings.auth_timeout();
    let timeout = tokio::time::sleep_until(deadline);
    tokio::pin!(timeout);
    let frame = loop {
        tokio::select! {
            _ = close.ended() => return end(socket, close, close.frame()).await,
            _ = &mut timeout => return end(socket, close, Some(AUTH_TIMEOUT)).await,
            action = idle.next() => match action {
                IdleAction::Ping => {
                    if !send_auth_frame(socket, Message::Ping(Vec::new().into()), deadline, close).await {
                        return end(socket, close, close.frame()).await;
                    }
                },
                IdleAction::Expired => { close.peer_gone(); return None; },
            },
            message = socket.recv() => match message {
                Some(Ok(Message::Text(text))) => { metrics::counter!("ws_messages_received").increment(1); idle.activity(); break text.to_string(); },
                Some(Ok(Message::Binary(bytes))) => { metrics::counter!("ws_messages_received").increment(1); idle.activity(); break String::from_utf8_lossy(&bytes).into_owned(); },
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => { idle.activity(); continue; },
                Some(Ok(Message::Close(frame))) => { close.peer_closed(frame.map_or(1005, |frame| frame.code)); return None; },
                _ => { close.peer_gone(); return None; },
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
    // Biased: the absolute deadline and a close win; a received frame is handled
    // before the verdict.
    let verdict = loop {
        tokio::select! {
            biased;
            _ = close.ended() => return end(socket, close, close.frame()).await,
            _ = &mut timeout => return end(socket, close, Some(AUTH_TIMEOUT)).await,
            action = idle.next() => match action {
                IdleAction::Ping => {
                    if !send_auth_frame(socket, Message::Ping(Vec::new().into()), deadline, close).await {
                        return end(socket, close, close.frame()).await;
                    }
                },
                IdleAction::Expired => { close.peer_gone(); return None; },
            },
            message = socket.recv() => match message {
                Some(Ok(Message::Text(_) | Message::Binary(_))) => {
                    idle.activity();
                    super::metrics::log("ws-handler", "warn");
                    if !send_auth_frame(socket, Message::Text(AUTH_IN_PROGRESS.into()), deadline, close).await {
                        return end(socket, close, close.frame()).await;
                    }
                }
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => { idle.activity(); }
                Some(Ok(Message::Close(frame))) => { close.peer_closed(frame.map_or(1005, |frame| frame.code)); return None; },
                _ => {
                    close.peer_gone();
                    tracing::info!("socket closed during authentication; not attached");
                    return None;
                }
            },
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
            metrics::counter!("ws_auth_errors", "type" => kind).increment(1);
            super::metrics::log(
                "ws-handler",
                if kind == "server_error" {
                    "error"
                } else {
                    "warn"
                },
            );
            tracing::info!(%err, "auth handshake failed");
            end(socket, close, Some(UNAUTHORIZED)).await
        }
    }
}

async fn send_auth_frame<S>(
    socket: &mut S,
    message: Message,
    deadline: Instant,
    close: &SocketClose,
) -> bool
where
    S: Sink<Message> + Unpin,
{
    let sent = tokio::select! {
        biased;
        _ = close.ended() => return false,
        sent = tokio::time::timeout_at(deadline, socket.send(message)) => sent,
    };
    match sent {
        Ok(Ok(())) => true,
        Ok(Err(_)) => {
            close.peer_gone();
            false
        }
        Err(_) => {
            close.end(AUTH_TIMEOUT.0, AUTH_TIMEOUT.1);
            false
        }
    }
}

async fn end<S>(
    socket: &mut S,
    close: &SocketClose,
    frame: Option<(u16, &'static str)>,
) -> Option<Address>
where
    S: Sink<Message> + Unpin,
{
    if let Some(frame) = frame {
        close.end(frame.0, frame.1);
        let _ = tokio::time::timeout(CLOSE_BUDGET, socket.send(close_message(frame))).await;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    #[derive(Default)]
    struct StalledSink {
        frames: Vec<Message>,
        flush: bool,
    }
    impl Sink<Message> for StalledSink {
        type Error = std::convert::Infallible;
        fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn start_send(mut self: Pin<&mut Self>, frame: Message) -> Result<(), Self::Error> {
            self.frames.push(frame);
            Ok(())
        }
        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            if self.flush {
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        }
        fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            self.poll_flush(cx)
        }
    }

    #[tokio::test(start_paused = true)]
    async fn auth_ping_and_in_progress_writes_share_the_original_absolute_deadline() {
        for frame in [
            Message::Ping(Vec::new().into()),
            Message::Text(AUTH_IN_PROGRESS.into()),
        ] {
            let close = SocketClose::default();
            let mut sink = StalledSink::default();
            let deadline = Instant::now() + Duration::from_secs(10);
            // A first auth write completes late in the window. It cannot grant
            // a fresh window to the next write, even if that flush stalls.
            tokio::time::advance(Duration::from_secs(8)).await;
            sink.flush = true;
            assert!(
                send_auth_frame(
                    &mut sink,
                    Message::Ping(Vec::new().into()),
                    deadline,
                    &close
                )
                .await
            );
            sink.flush = false;
            assert!(!send_auth_frame(&mut sink, frame.clone(), deadline, &close).await);
            assert_eq!(Instant::now(), deadline);
            assert_eq!(close.frame(), Some(AUTH_TIMEOUT));
            assert_eq!(sink.frames.len(), 2);
            assert_eq!(sink.frames[1], frame);

            assert!(end(&mut sink, &close, close.frame()).await.is_none());
            assert_eq!(Instant::now(), deadline + CLOSE_BUDGET);
            assert_eq!(close.frame(), Some(AUTH_TIMEOUT));
        }
    }

    #[tokio::test(start_paused = true)]
    async fn auth_write_cancellation_drops_stalled_flush_without_renewing_deadline() {
        let close = SocketClose::default();
        let mut sink = StalledSink::default();
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut send = Box::pin(send_auth_frame(
            &mut sink,
            Message::Text(AUTH_IN_PROGRESS.into()),
            deadline,
            &close,
        ));
        assert!(futures::poll!(&mut send).is_pending());
        close.peer_gone();
        assert!(!send.await);
        assert!(Instant::now() < deadline);
        assert_eq!(close.frame(), None);
        assert_eq!(sink.frames.len(), 1);
    }
}
