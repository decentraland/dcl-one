use std::net::SocketAddr;
use std::time::Duration;

use axum::extract::{ConnectInfo, Path, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Router};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::Deserialize;
use serde_json::{Value, json};
use socketioxide::SocketIo;
use socketioxide::extract::{AckSender, SocketRef, TryData};

use super::{
    AppState, CreateRequestBody, CreateRequestResponse, DEFAULT_BODY_SIZE_BYTES,
    HttpOutcomeMessage, RelayJson, claim_outcome, create_request, get_request_v2, naive,
    now_micros, post_request_validation, submit_outcome,
};
use crate::jobs::{Jobs, PeriodicCfg};
use crate::shared::cors::{decentraland_cors, is_decentraland_origin};

const SESSIONS: dcl_metrics::CounterFamily<0> = dcl_metrics::CounterFamily::new(
    "dcl_auth_socketio_sessions_total",
    "Auth Socket.IO namespace sessions opened.",
    None,
    &[],
);
const EVENTS: dcl_metrics::CounterFamily<1> = dcl_metrics::CounterFamily::new(
    "dcl_auth_socketio_events_total",
    "Inbound auth Socket.IO events handled.",
    None,
    &["event"],
);

/// Socket.IO exposes namespace and application callbacks for both Engine.IO
/// polling and WebSocket clients.
pub const SOCKET_FAMILIES: &[dcl_metrics::Descriptor] = &[SESSIONS.descriptor, EVENTS.descriptor];

const PATH: &str = "/auth-api/socket.io/";
const PUSH_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Default)]
pub(crate) struct SocketBindings(DashMap<String, Binding>);

struct Binding {
    socket: SocketRef,
    expiration: DateTime<Utc>,
    validation_sent: bool,
}

impl SocketBindings {
    pub(crate) fn spawn(&self, state: &AppState, jobs: &Jobs) {
        let state = std::sync::Arc::downgrade(state);
        jobs.spawn_periodic(
            "auth-api-socket-push",
            PeriodicCfg::every(PUSH_INTERVAL),
            move |_| {
                let state = state.clone();
                async move {
                    if let Some(state) = state.upgrade() {
                        state.auth_sockets.push(&state).await?;
                    }
                    Ok(())
                }
            },
        );
    }

    pub(super) async fn push(&self, state: &AppState) -> sqlx::Result<()> {
        let now = now_micros();
        self.0
            .retain(|_, binding| binding.expiration >= now && binding.socket.connected());
        let ids: Vec<_> = self.0.iter().map(|entry| entry.key().clone()).collect();
        if ids.is_empty() {
            return Ok(());
        }
        let ready: Vec<(String, i16, bool, String)> = sqlx::query_as(
            "SELECT request_id, code, requires_validation, status
             FROM explore_api.auth_requests
             WHERE request_id = ANY($1) AND expiration >= $2
               AND (requires_validation OR status <> 'pending')",
        )
        .bind(&ids)
        .bind(naive(now))
        .fetch_all(&state.pool)
        .await?;

        for (id, code, needs_validation, status) in ready {
            if status == "fulfilled" {
                self.0.remove(&id);
                continue;
            }
            let Some(mut binding) = self.0.get_mut(&id) else {
                continue;
            };
            if needs_validation && !binding.validation_sent {
                binding.validation_sent = binding
                    .socket
                    .emit(
                        "request-validation-status",
                        &json!({ "requestId": id, "code": code }),
                    )
                    .is_ok();
            }
            if status == "signed" {
                let socket = binding.socket.clone();
                drop(binding);
                if let Err(error) = self.deliver_outcome(state, &id, &socket).await {
                    tracing::warn!(%error, "auth relay socket outcome delivery failed");
                }
            }
        }
        Ok(())
    }

    async fn deliver_outcome(
        &self,
        state: &AppState,
        id: &str,
        socket: &SocketRef,
    ) -> sqlx::Result<()> {
        let mut tx = state.pool.begin().await?;
        if let Some(outcome) = claim_outcome(&mut *tx, id, now_micros()).await? {
            let outcome: Value =
                serde_json::from_str(&outcome).map_err(|err| sqlx::Error::Decode(Box::new(err)))?;
            if socket.emit("outcome", &outcome).is_err() {
                // A failed enqueue leaves the outcome available to HTTP polling.
                return tx.rollback().await;
            }
        }

        // An enqueued outcome must not be sent again on this socket if commit fails.
        self.0.remove(id);
        tx.commit().await
    }
}

async fn check_origin(req: Request, next: Next) -> Response {
    let desktop_websocket = req
        .headers()
        .get(header::UPGRADE)
        .is_some_and(|value| value.as_bytes().eq_ignore_ascii_case(b"websocket"));
    if req.headers().get(header::ORIGIN).is_some_and(|origin| {
        !origin.to_str().is_ok_and(|value| {
            is_decentraland_origin(value) || (value == "null" && desktop_websocket)
        })
    }) {
        return StatusCode::FORBIDDEN.into_response();
    }
    next.run(req).await
}

pub(crate) fn router(state: AppState) -> Router {
    let (service, io) = SocketIo::builder()
        .req_path("/auth-api/socket.io")
        .max_payload(DEFAULT_BODY_SIZE_BYTES as u64)
        .ws_max_frame_size(DEFAULT_BODY_SIZE_BYTES)
        .build_svc();
    io.ns("/", move |socket: SocketRef| {
        let state = state.clone();
        async move { register(socket, state) }
    });
    Router::new()
        .route_service(PATH, service.clone())
        .route_service("/auth-api/socket.io", service)
        .layer(middleware::from_fn(check_origin))
        .layer(middleware::from_fn(decentraland_cors))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestId {
    request_id: uuid::Uuid,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SocketOutcome {
    request_id: uuid::Uuid,
    #[serde(flatten)]
    outcome: HttpOutcomeMessage,
}

async fn ack_response(ack: AckSender, response: Response) {
    let _ = ack.send(&json_response(response).await);
}

async fn json_response(response: Response) -> Value {
    let success = response.status().is_success();
    let bytes = axum::body::to_bytes(response.into_body(), 2 * DEFAULT_BODY_SIZE_BYTES).await;
    match bytes {
        Ok(bytes) if success && bytes.is_empty() => json!({}),
        Ok(bytes) => serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| json!({ "error": "Internal server error" })),
        Err(_) => json!({ "error": "Internal server error" }),
    }
}

fn metadata(socket: &SocketRef) -> (Option<Extension<ConnectInfo<SocketAddr>>>, HeaderMap) {
    let parts = socket.req_parts();
    (
        parts.extensions.get().copied().map(Extension),
        parts.headers.clone(),
    )
}

/// The client source of the socket's connecting request, for the node's
/// budget of contract-wallet checks (`eip1654_budget`): socketioxide runs each
/// event outside that request's scope, so an event's checks are charged to it
/// here.
fn client_source(
    state: &AppState,
    peer: Option<&Extension<ConnectInfo<SocketAddr>>>,
    headers: &HeaderMap,
) -> Option<String> {
    crate::shared::client_ip::distinct_client_ip(
        headers,
        peer.map(|Extension(ConnectInfo(peer))| *peer),
        &state.settings.trusted_proxy_cidrs,
    )
}

fn bad_message(ack: AckSender) {
    let _ = ack.send(&json!({ "error": "Invalid request body" }));
}

fn register(socket: SocketRef, state: AppState) {
    SESSIONS.handle([]).increment(1);
    let create_state = state.clone();
    socket.on(
        "request",
        move |socket: SocketRef, TryData(body): TryData<CreateRequestBody>, ack: AckSender| {
            let state = create_state.clone();
            async move {
                EVENTS.handle(["request"]).increment(1);
                let Ok(body) = body else {
                    return bad_message(ack);
                };
                let (peer, headers) = metadata(&socket);
                let source = client_source(&state, peer.as_ref(), &headers);
                let response = crate::shared::eip1654_budget::with_source(source, async {
                    crate::shared::eip1654_budget::debug_assert_scoped("the auth socket's request");
                    create_request(State(state.clone()), peer, headers, RelayJson(body)).await
                })
                .await;
                if response.status() != StatusCode::CREATED {
                    return ack_response(ack, response).await;
                }
                let body = json_response(response).await;
                if let Ok(created) = serde_json::from_value::<CreateRequestResponse>(body.clone())
                    && ack.send(&body).is_ok()
                    && socket.connected()
                {
                    state.auth_sockets.0.insert(
                        created.request_id,
                        Binding {
                            socket,
                            expiration: created.expiration,
                            validation_sent: false,
                        },
                    );
                }
            }
        },
    );

    let recover_state = state.clone();
    socket.on(
        "recover",
        move |TryData(data): TryData<RequestId>, ack: AckSender| {
            let state = recover_state.clone();
            async move {
                EVENTS.handle(["recover"]).increment(1);
                let Ok(data) = data else {
                    return bad_message(ack);
                };
                ack_response(
                    ack,
                    get_request_v2(State(state), Path(data.request_id.to_string())).await,
                )
                .await;
            }
        },
    );

    let validation_state = state.clone();
    socket.on(
        "request-validation-status",
        move |TryData(data): TryData<RequestId>, ack: AckSender| {
            let state = validation_state.clone();
            async move {
                EVENTS.handle(["request-validation-status"]).increment(1);
                let Ok(data) = data else {
                    return bad_message(ack);
                };
                ack_response(
                    ack,
                    post_request_validation(State(state), Path(data.request_id.to_string())).await,
                )
                .await;
            }
        },
    );

    socket.on(
        "outcome",
        move |socket: SocketRef, TryData(data): TryData<SocketOutcome>, ack: AckSender| {
            let state = state.clone();
            async move {
                EVENTS.handle(["outcome"]).increment(1);
                let Ok(data) = data else {
                    return bad_message(ack);
                };
                let (peer, headers) = metadata(&socket);
                let source = client_source(&state, peer.as_ref(), &headers);
                let response = crate::shared::eip1654_budget::with_source(source, async {
                    crate::shared::eip1654_budget::debug_assert_scoped("the auth socket's outcome");
                    submit_outcome(
                        State(state),
                        peer,
                        headers,
                        Path(data.request_id.to_string()),
                        RelayJson(data.outcome),
                    )
                    .await
                })
                .await;
                ack_response(ack, response).await;
            }
        },
    );
}
