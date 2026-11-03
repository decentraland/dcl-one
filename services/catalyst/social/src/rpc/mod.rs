//! The social RPC: the Decentraland-RPC WebSocket unity-explorer opens for
//! friends, friendship requests and their events, presence, blocks, the
//! social settings, private calls and community voice rooms (the old
//! `catalyrst-social-rpc` executable, a port of decentraland/social-service-ea
//! over `dcl-rpc`). The socket is [`ws::ws_upgrade`]: a signed handshake
//! frame (`handshake`), then protobuf-over-WebSocket framed by `drpc` with
//! the one module `service` registers; every procedure is answered from
//! `social.*` over the ONE pool (`db`) and fans its events out in-process
//! (`pubsub`; local delivery). Online status is reconstructed independently on
//! every replica from the complete Pulse topology over the existing comms NATS link.
//!
//! Voice needs LiveKit: call persistence and media operations use the gatekeeper's
//! `VoiceRooms` seam (`comms::gatekeeper::voice::rooms`), which its state
//! implements in `catalyst serve`; its LiveKit configuration controls call
//! availability. Presence questions are answered from the
//! shared presence tables.
//!
//! Wiring (main.rs): `let social_rpc = Arc::new(social::rpc::State::new(pool,
//! settings, eip1654, gatekeeper.clone()))`; `social_rpc.spawn_jobs(&jobs)`
//! (the RPC server loop and the private-voice expiry sweep);
//! `.merge(social::rpc::router(social_rpc))`. Routes: `GET /social-rpc`
//! (the upgrade; its signature binds the source protocol path `/`, independently
//! of the connection URL and proxy headers), `GET /social-rpc/info`,
//! `GET /social-rpc/health` (local), the plain JSON stream
//! `GET /social-rpc/v1/stream` (`stream`, upstream's `/v1/stream`), and the
//! upstream uWS host's HTTP routes
//! (`src/controllers/routes/uws.routes.ts`): `GET /social-rpc/status`, any
//! method on `/social-rpc/health/live`,
//! `GET /social-rpc/v1/users/{address}/privacy-settings` and a 404
//! `{"error":"Not Found"}` for every other `/social-rpc/*` path or method,
//! those wrapped routes carry `Access-Control-Allow-Origin: *`. The configured
//! GET metrics path reads a filtered view of the existing recorder without CORS;
//! then the bearer-gated
//! `/admin/social/*` (`admin`). The procedures' HTTP twins on the social-api
//! host are `http::router`, which `social::router` mounts beside the
//! communities routes.

pub mod admin;
pub mod context;
pub mod db;
mod fanout;
pub mod http;
pub use crate::shared::rpc::{drpc, handshake};
pub mod limits;
mod metrics;
pub mod profiles;
pub mod proto;
pub mod pubsub;
pub mod service;
pub mod stream;
mod topology;
pub mod voice;
pub mod ws;

#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, State as AxumState};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use axum::{Json, Router};
use parking_lot::Mutex;
use serde_json::json;
use sqlx::PgPool;
use tokio::sync::OnceCell;

use crate::auth::eip1654::Eip1654Validator;
use crate::comms::gatekeeper::voice::rooms::VoiceRooms;
use crate::jobs::{Jobs, PeriodicCfg};
use crate::rpc::context::{Context, VoiceSettings};
use crate::rpc::db::Db;
use crate::rpc::drpc::server::{RpcServer, ServerEventsSender};
use crate::rpc::profiles::Profiles;
use crate::shared::rpc::ws::{AxumWsTransport, OrdinaryQueueConfig};

/// The socket's settings (the old `catalyrst-social-rpc` env, minus its
/// database and gatekeeper URLs, which the ONE pool and the voice seam
/// replace).
#[derive(Clone, Debug)]
pub struct Settings {
    /// `AUTH_WINDOW_SECS`: how long the first frame may take to arrive and how
    /// far its timestamp may sit from the server clock.
    pub auth_window_secs: i64,
    /// WS_AUTH_TIMEOUT_IN_MS; unset keeps the legacy AUTH_WINDOW_SECS fallback.
    pub ws_auth_timeout_ms: Option<f64>,
    /// WS_AUTH_SIGNATURE_EXPIRATION_IN_MS, independent from the first-frame timer.
    pub ws_auth_signature_expiration_ms: Option<f64>,
    /// WS_IDLE_TIMEOUT_IN_SECONDS; zero disables automatic ping/idle expiry.
    pub ws_idle_timeout_secs: u64,
    /// `PRIVATE_VOICE_CHAT_EXPIRATION_TIME` (ms): a call not accepted within it expires.
    pub private_voice_chat_expiration_ms: i64,
    /// `PRIVATE_VOICE_CHAT_JOB_INTERVAL` (ms): the expiry sweep's tick.
    pub private_voice_chat_job_interval_ms: u64,
    /// `PRIVATE_VOICE_CHAT_EXPIRATION_BATCH_SIZE`: expired calls reclaimed per DELETE.
    pub private_voice_chat_expiration_batch_size: i64,
    /// COMMUNITY_VOICE_CHAT_POLLING_JOB_INTERVAL in milliseconds.
    pub community_voice_chat_polling_job_interval_ms: u64,
    /// `WS_MAX_CONCURRENT_CONNECTIONS`: unset = unlimited.
    pub ws_max_concurrent_connections: Option<usize>,
    /// `WS_MAX_PAYLOAD_LENGTH`: the largest inbound frame.
    pub ws_max_payload_bytes: usize,
    /// Raw `WS_MAX_BACKPRESSURE` number: the ordinary queue uses it directly;
    /// both socket writers use its native uWS unsigned-32-bit byte threshold.
    pub ws_max_backpressure_bytes: f64,
    pub ordinary_transport: OrdinaryQueueConfig,
    /// The per-address limits both sockets share (`WS_MAX_CONNECTIONS_PER_ADDRESS`,
    /// `WS_SUBSCRIPTIONS_PER_SECOND`, `WS_SUBSCRIPTION_BURST`, `WS_ADDRESS_BUDGET_CAPACITY`).
    pub admission: limits::AdmissionLimits,
    pub friendship_limits: limits::FriendshipLimits,
    /// `/v1/stream`'s own (`STREAM_WS_*`).
    pub stream: stream::StreamLimits,
    /// `CATALYRST_SOCIAL_RPC_ADMIN_TOKEN`: the `/admin/social` bearer; unset refuses.
    pub admin_token: Option<String>,
    /// `SOCIAL_WRITES_ENABLED`: false answers every mutating procedure with a
    /// 503 `WritesDisabled` remote error (`social::writes`).
    pub writes_enabled: bool,
    /// Source COMMIT_HASH string; absent returns empty, without build fallback.
    pub commit_hash: Option<String>,
    pub current_version: Option<String>,
    pub metrics_path: Option<String>,
    pub metrics_bearer_token: Option<String>,
    pub metrics_reset_at_night: bool,
    /// Existing process recorder; this route installs no exporter or listener.
    pub metrics_handle: Option<crate::comms::metrics::PrometheusHandle>,
    pub profile_source: Option<Arc<crate::communities::profiles::RegistryReader>>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auth_window_secs: 300,
            ws_auth_timeout_ms: None,
            ws_auth_signature_expiration_ms: None,
            ws_idle_timeout_secs: 300,
            private_voice_chat_expiration_ms: 60_000,
            private_voice_chat_job_interval_ms: 1_000,
            private_voice_chat_expiration_batch_size: 20,
            community_voice_chat_polling_job_interval_ms: 45_000,
            ws_max_concurrent_connections: None,
            ws_max_payload_bytes: 1024 * 1024,
            ws_max_backpressure_bytes: 131072.0,
            ordinary_transport: OrdinaryQueueConfig::default(),
            admission: Default::default(),
            friendship_limits: Default::default(),
            stream: Default::default(),
            admin_token: None,
            writes_enabled: true,
            commit_hash: None,
            current_version: None,
            metrics_path: None,
            metrics_bearer_token: None,
            metrics_reset_at_night: false,
            metrics_handle: None,
            profile_source: None,
        }
    }
}

impl Settings {
    pub(crate) fn auth_timeout(&self) -> Duration {
        let configured = self
            .ws_auth_timeout_ms
            .unwrap_or_else(|| self.auth_window_secs as f64 * 1000.0);
        // Node setTimeout inserts into an integer millisecond bucket and uses
        // one millisecond outside its supported positive signed-32-bit range.
        let ms = if (1.0..=i32::MAX as f64).contains(&configured) {
            configured.trunc() as u64
        } else {
            1
        };
        Duration::from_millis(ms)
    }
    pub(crate) fn auth_signature_expiration_ms(&self) -> f64 {
        self.ws_auth_signature_expiration_ms
            .unwrap_or_else(|| self.auth_window_secs as f64 * 1000.0)
    }
}

pub struct State {
    pub settings: Settings,
    pub ctx: Context,
    pub eip1654: Option<Arc<dyn Eip1654Validator>>,
    rpc_events: OnceCell<ServerEventsSender<AxumWsTransport>>,
    /// Armed after registering Social's subject; local/no-bus states have no gate.
    fanout_ready: Mutex<Option<Arc<std::sync::atomic::AtomicBool>>>,
    /// The server, until `spawn_jobs` takes it into its job.
    server: Mutex<Option<RpcServer<Context, AxumWsTransport>>>,
}

impl State {
    pub fn new(
        pool: PgPool,
        settings: Settings,
        eip1654: Option<Arc<dyn Eip1654Validator>>,
        voice: Arc<dyn VoiceRooms>,
    ) -> Self {
        let profiles = settings
            .profile_source
            .as_ref()
            .map(|source| Profiles::from_registry(source.clone()))
            .unwrap_or_else(|| Profiles::new(Some(pool.clone())));
        let ctx = Context::new(
            VoiceSettings {
                private_voice_chat_expiration_ms: settings.private_voice_chat_expiration_ms,
                private_voice_chat_expiration_batch_size: settings
                    .private_voice_chat_expiration_batch_size,
            },
            Db::new(pool),
            profiles,
            voice,
        );
        ctx.set_writes_enabled(settings.writes_enabled);
        Self::with_context(settings, ctx, eip1654)
    }

    /// A state over an already built context (the tests hand in a fake voice
    /// runtime or a pool-less profile reader).
    pub fn with_context(
        settings: Settings,
        ctx: Context,
        eip1654: Option<Arc<dyn Eip1654Validator>>,
    ) -> Self {
        if settings.metrics_reset_at_night {
            tracing::warn!(
                "WKC_METRICS_RESET_AT_NIGHT is configured; social-view reset remains unsupported; the shared recorder is preserved"
            );
        }
        ctx.admission().configure(settings.admission);
        ctx.friendship_limiter()
            .configure(settings.friendship_limits);
        let (sender, server) = build_rpc_server(ctx.clone());
        let rpc_events = OnceCell::new();
        rpc_events.set(sender).ok();
        Self {
            settings,
            ctx,
            eip1654,
            rpc_events,
            fanout_ready: Mutex::new(None),
            server: Mutex::new(Some(server)),
        }
    }

    pub(crate) fn initial_fanout_ready(&self) -> bool {
        self.fanout_ready
            .lock()
            .as_ref()
            .is_none_or(|ready| ready.load(std::sync::atomic::Ordering::Acquire))
    }

    /// The handle the socket attaches transports through; `None` before the
    /// server job took the server (never, after `spawn_jobs`).
    pub fn rpc_events(&self) -> Option<&ServerEventsSender<AxumWsTransport>> {
        self.rpc_events.get()
    }

    /// The RPC server loop (one job; at shutdown it ends every socket of
    /// both protocols with 1001 "Server shutting down", as upstream's
    /// `ws-pool` stop) and the private-voice expiry sweep (a periodic job on
    /// the configured tick).
    pub fn spawn_jobs(self: &Arc<Self>, jobs: &Jobs) {
        if let Some(mut server) = self.server.lock().take() {
            let sockets = self.ctx.clone();
            jobs.spawn("social-rpc-server", move |ctx| async move {
                tokio::select! {
                    _ = ctx.cancel.cancelled() => {}
                    _ = server.run() => {}
                }
                let (code, reason) = SHUTTING_DOWN;
                sockets.end_sockets(code, reason, SHUTDOWN_BUDGET).await;
                Ok(())
            });
        }
        let limiter_ctx = self.ctx.clone();
        jobs.spawn_periodic_leased(
            "friendship-mutation-limit-expiry",
            PeriodicCfg::every(Duration::from_secs(1)),
            move |_job| {
                let ctx = limiter_ctx.clone();
                async move {
                    ctx.friendship_limiter().cleanup_expired().await?;
                    ctx.profiles().cleanup_expired().await?;
                    Ok(())
                }
            },
        );
        let ctx = self.ctx.clone();
        let expiration_ms = self.settings.private_voice_chat_expiration_ms;
        let batch_size = self.settings.private_voice_chat_expiration_batch_size;
        let interval =
            Duration::from_millis(self.settings.private_voice_chat_job_interval_ms.max(1));
        let community_context = self.ctx.clone();
        jobs.spawn_periodic_leased(
            "community-voice-chat-notifications",
            PeriodicCfg::every(Duration::from_millis(
                self.settings
                    .community_voice_chat_polling_job_interval_ms
                    .max(1),
            )),
            move |_job| {
                let ctx = community_context.clone();
                async move {
                    ctx.poll_community_voice_notifications().await;
                    Ok(())
                }
            },
        );
        jobs.spawn_periodic_leased(
            "private-voice-chat-expiry",
            PeriodicCfg::every(interval),
            move |_job| {
                let ctx = ctx.clone();
                async move {
                    ctx.sweep_private_voice_chats_if_due(expiration_ms, batch_size)
                        .await;
                    Ok(())
                }
            },
        );
    }
}

/// The close every socket gets at shutdown (upstream `src/logic/ws-pool/component.ts:85`).
pub const SHUTTING_DOWN: (u16, &str) = (1001, "Server shutting down");
/// How long shutdown waits for the sockets to close.
const SHUTDOWN_BUDGET: Duration = Duration::from_secs(2);

/// The server with its transport handlers: a connected socket registers its
/// identity, takes its address's socket slot (displacing the oldest past the
/// cap, `limits::Admission`) and counts the address present (across its
/// sockets of both protocols); a closed one forgets it, frees the slot and,
/// when it was the address's last, ends the call it was on (#479).
fn build_rpc_server(
    ctx: Context,
) -> (
    ServerEventsSender<AxumWsTransport>,
    RpcServer<Context, AxumWsTransport>,
) {
    let mut server: RpcServer<Context, AxumWsTransport> = RpcServer::create(ctx.clone());
    let sender = server.get_server_events_sender();

    let bind_ctx = ctx.clone();
    server.set_on_transport_connected_handler(move |transport, transport_id| {
        let address = transport.address().to_string();
        bind_ctx.register_identity(transport_id, address.clone());
        bind_ctx.register_kill_handle(transport_id, transport.close_handle());
        bind_ctx
            .admission()
            .claim(&address, transport.close_handle(), limits::Protocol::Rpc);
        bind_ctx.mark_online(&address);
    });
    let forget_ctx = ctx.clone();
    server.set_on_transport_closes_handler(move |transport, transport_id| {
        forget_ctx.forget_identity(transport_id);
        let address = transport.address().to_string();
        forget_ctx
            .admission()
            .release(&address, &transport.close_handle());
        if forget_ctx.mark_offline(&address) {
            let voice_ctx = forget_ctx.clone();
            tokio::spawn(async move {
                voice_ctx
                    .end_private_voice_chat_on_disconnect(&address)
                    .await;
            });
        }
    });
    server.set_module_registrator_handler(service::register_service);
    (sender, server)
}

/// The socket at `/social-rpc`, the upstream host's HTTP routes and the local
/// info/health beside it (one `Access-Control-Allow-Origin` layer over them,
/// as upstream's `wrap`), and the admin routes.
pub fn router(state: Arc<State>) -> Router {
    Router::new()
        .route("/social-rpc", get(ws::ws_upgrade).fallback(not_found))
        .route(
            "/social-rpc/v1/stream",
            get(stream::ws_upgrade).fallback(not_found),
        )
        .route("/social-rpc/info", get(info))
        .route("/social-rpc/health", get(health))
        .route("/social-rpc/health/live", any(health_live))
        // upstream's `app.get` takes GET alone; HEAD falls to its `ANY /*`
        .route(
            "/social-rpc/status",
            get(status).head(not_found).fallback(not_found),
        )
        .route(
            "/social-rpc/v1/users/{address}/privacy-settings",
            get(privacy_settings).head(not_found).fallback(not_found),
        )
        .route("/social-rpc/{*rest}", any(not_found))
        .layer(axum::middleware::map_response(allow_any_origin))
        .nest("/admin/social", admin::router())
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            metrics::serve_if_requested,
        ))
        .with_state(state)
}

/// Upstream's `wrap`: `*` unless the handler set an origin itself.
async fn allow_any_origin(mut response: Response) -> Response {
    response
        .headers_mut()
        .entry(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .or_insert(HeaderValue::from_static("*"));
    response
}

/// Upstream's `ANY /*`.
pub(super) async fn not_found() -> Response {
    (StatusCode::NOT_FOUND, Json(json!({ "error": "Not Found" }))).into_response()
}

/// `handlers/uws/status-handler.ts`.
async fn status(AxumState(state): AxumState<Arc<State>>) -> Json<serde_json::Value> {
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    Json(json!({
        "version": state.settings.current_version.as_deref().unwrap_or(""),
        "currentTime": current_time,
        "commitHash": state.settings.commit_hash.as_deref().unwrap_or(""),
    }))
}

/// `handlers/uws/privacy-handler.ts`: the address lowercased (never trimmed),
/// then the stored privacy or the absent-row `all`.
async fn privacy_settings(
    AxumState(state): AxumState<Arc<State>>,
    Path(address): Path<String>,
) -> Response {
    let Ok(address) = crate::shared::identity::Address::parse_untrimmed(&address) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Invalid address" })),
        )
            .into_response();
    };
    match state.ctx.db().get_social_settings(address.as_str()).await {
        Ok(row) => Json(json!({
            "private_messages_privacy": row.unwrap_or_default().private_messages_privacy,
        }))
        .into_response(),
        Err(db::DbError::Sqlx(e)) => {
            tracing::warn!(error = %e, "privacy-settings read failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": crate::communities::http::database_message(&e) })),
            )
                .into_response()
        }
    }
}

async fn info() -> Json<serde_json::Value> {
    Json(
        json!({ "service": "catalyrst-social-rpc", "version": env!("CARGO_PKG_VERSION"), "ws": "/social-rpc" }),
    )
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "healthy": true }))
}

async fn health_live() -> &'static str {
    "alive"
}
