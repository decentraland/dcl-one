//! The worlds content server's product surface beyond the stage-4 reads: a
//! world publish (`POST /entities`) and unpublish, the permission and
//! access-list edits, the world settings, the manifest and public listing,
//! the access log the LiveKit webhook feeds, and the bearer-gated operator
//! routes. Every authorization question goes through
//! [`crate::policy::Policy`] (owner by personal rule or the name
//! authority, the stored owner being its cache; deployment grants covering
//! the footprint; the shared ban tables), every blob to the ONE blob store, every row to `worlds.*`
//! over the ONE pool.
//!
//! Not here, by ownership: the world comms mints (`/worlds/{name}/comms`,
//! `/worlds/{name}/scenes/{id}/comms`, `/get-comms-adapter/{room}`) belong
//! to the comms gatekeeper (`comms::gatekeeper::admission::world_mint`), which owns the
//! LiveKit transport and calls `Policy::can_access` / `rooms::resolve`
//! through its interfaces; the live presence the webhook feeds
//! (`/live-data`, `/wallet/{w}/connected-world`) is `comms::presence`;
//! `/world/{name}/about`, `/scenes`, `/index` and `/worlds/status` are the
//! stage-4 `worlds` reads; the per-scene storage routes are the storage
//! slice.
//!
//! Left out of this port and listed in its return: the worlds `/gc`
//! collector (the blob store is shared with the content server: a
//! worlds-only orphan sweep would delete synced content;
//! `store::active_content_keys` is this domain's active set for a
//! store-wide collector), the contents-directory temp reaper (the blob
//! store's own staging files), `/world/{name}/preview-wearables`, the
//! `/health` and per-service `/status` shapes, `openapi.json`, and the
//! worlds federation routes.

pub mod active;
pub mod admin;
pub mod auth;
pub mod deploy;
mod deploy_form;
mod deploy_validate;
pub mod forwarding;
pub mod http;
pub mod manifest;
pub mod names;
pub mod permissions;
pub mod scenes;
pub mod settings;
pub mod settings_policy;
pub mod store;
pub mod upload_limits;

#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, post, put};
use parking_lot::Mutex;
use sqlx::PgPool;
use tokio::sync::mpsc;

use crate::auth::Eip1654Validator;
use crate::comms::presence::Presence;
use crate::comms::world_identity::WorldName;
use crate::jobs::Jobs;
use crate::policy::Policy;
use crate::shared::cors;
use crate::shared::identity::Address;
use crate::shared::nul_guard;
use catalyst_storage::Storage;

pub use upload_limits::UploadLimits;

/// Rows per multi-row access-log INSERT; the drain takes whatever is queued up to this.
const ACCESS_LOG_BATCH: usize = 256;

/// What the write surface needs beside the shared handles.
#[derive(Clone)]
pub struct Settings {
    /// `CATALYRST_WORLDS_ADMIN_TOKEN`: the bearer of the `/admin/*` routes; unset refuses them.
    pub admin_token: Option<String>,
    /// `MAX_IN_FLIGHT_UPLOAD_BYTES`, `MAX_CONCURRENT_UPLOADS`,
    /// `MAX_IN_FLIGHT_UPLOAD_FILES`, `MULTIPART_UPLOAD_TIMEOUT_MS`,
    /// `DEPLOYMENT_PROCESSING_TIMEOUT_MS`.
    pub upload_limits: UploadLimits,
    /// `GLOBAL_SCENES_URN`: scenes the configuration names, kept by the blob
    /// store collector (`/admin/gc`).
    pub global_scenes_urn: Vec<String>,
}

impl std::fmt::Debug for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Settings")
            .field("admin_token", &self.admin_token.as_ref().map(|_| "<set>"))
            .field("upload_limits", &self.upload_limits)
            .finish()
    }
}

impl Settings {
    /// Validates the limits the way the old `Config::from_env` did (every
    /// knob positive, the byte budget at least one deploy payload).
    pub fn resolve(
        admin_token: Option<String>,
        upload_limits: UploadLimits,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            admin_token: admin_token.filter(|t| !t.trim().is_empty()),
            upload_limits: upload_limits.validate(deploy::MAX_UPLOAD_SIZE_BYTES as u64)?,
            global_scenes_urn: Vec::new(),
        })
    }

    pub fn with_global_scenes_urn(mut self, raw: Option<&str>) -> Self {
        self.global_scenes_urn = raw
            .map(|r| r.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default();
        self
    }

    pub fn for_tests(admin_token: Option<&str>) -> Self {
        Self {
            admin_token: admin_token.map(str::to_string),
            upload_limits: UploadLimits::default(),
            global_scenes_urn: Vec::new(),
        }
    }
}

/// What a permission or block edit revoked, handed to the running rooms
/// the moment the row changed (DATABASE.md: "Permission revocation must
/// also take effect in the running room"). The scene host closes the
/// sockets and comms evicts the LiveKit participants; the periodic
/// `scene-host-sweep` remains the safety net for changes made outside these
/// routes (comms' scene bans, an operator's SQL).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Revocation {
    /// One wallet lost its admission to the world (`DELETE
    /// /permissions/access/{address}`).
    Address { world: WorldName, address: Address },
    /// The world's access setting changed (`POST /permissions/access`: a new
    /// type, a new allow-list, a rotated secret): every admission but the
    /// owner's is decided again, and a shared-secret admission is void until
    /// the client presents the new secret.
    AccessSetting { world: WorldName },
    /// The operator blocked the wallet everywhere (`POST /admin/blocked/{w}`).
    Platform { address: Address },
}

/// A running-room authority that enforces a [`Revocation`] at once: the
/// scene host ([`crate::scene_state::SceneHostState`]) and the gatekeeper's
/// LiveKit rooms ([`CommsRoomRevocations`]). Sinks log their own failures;
/// the route does not depend on them.
#[async_trait::async_trait]
pub trait RevocationSink: Send + Sync {
    async fn revoke(&self, revocation: &Revocation);
}

/// The worlds core's running-room authority for LiveKit: a permission or
/// block edit evicts the affected participants from the world's rooms
/// through the gatekeeper's one revocation entry point
/// (`comms::gatekeeper::CommsState::revoke`, detached from the request; an
/// SFU outage is logged under `comms-revocation`, never surfaced to the
/// owner's edit). This sink only maps a [`Revocation`] onto the
/// gatekeeper's `RoomRevocation`.
pub struct CommsRoomRevocations(pub crate::comms::gatekeeper::SharedState);

#[async_trait::async_trait]
impl RevocationSink for CommsRoomRevocations {
    async fn revoke(&self, revocation: &Revocation) {
        use crate::comms::gatekeeper::RoomRevocation;
        self.0.revoke(match revocation.clone() {
            Revocation::Address { world, address } => {
                RoomRevocation::WorldAddress { world, address }
            }
            Revocation::AccessSetting { world } => RoomRevocation::WorldAccess { world },
            Revocation::Platform { address } => RoomRevocation::Everywhere { address },
        });
    }
}

/// The worlds half of the ONE LiveKit webhook: the gatekeeper's
/// `POST /comms-gatekeeper/livekit-webhook` (the deploy tree's root alias
/// `= /livekit-webhook` points there) verifies the bearer, feeds the live
/// presence and hands every event here, where a `participant_joined` /
/// `participant_left` on a worlds service room (`world-…`, `scene-…`,
/// parsed by the gatekeeper) is one row of the batched access log.
impl crate::comms::gatekeeper::events::RoomEventSink for CoreState {
    fn observe_room_event(&self, event: &crate::comms::protocol::bus::room_events::RoomEvent) {
        use crate::comms::protocol::bus::room_events::{RoomEventKind, RoomType};
        if event.room_type() != RoomType::World {
            return;
        }
        let action = match event.kind() {
            RoomEventKind::Joined => "join",
            RoomEventKind::Left => "leave",
            _ => return,
        };
        self.record_access(&event.realm_name, &event.identity, action, &event.room);
    }
}

pub struct CoreState {
    pub pool: PgPool,
    pub storage: Arc<Storage>,
    /// The permission core, ONE instance shared with the storage routes, the
    /// scene host and comms.
    pub policy: Arc<Policy>,
    pub settings: Settings,
    /// Contract-wallet (EIP-1654) signature check for the deploy chain and
    /// the signed-fetch routes; `None` refuses such chains.
    pub eip1654: Option<Arc<dyn Eip1654Validator>>,
    /// The live presence (`comms::presence`), shared with the read surface
    /// (`WorldsState`), whose `/status` reports the occupied world rooms, and
    /// read by the presence archive.
    pub presence: Arc<Presence>,
    pub names: names::NameDenyList,
    access_log_tx: mpsc::UnboundedSender<store::AccessLogEntry>,
    /// Taken by `spawn_jobs`; `flush_access_log` drains it in process until then.
    access_log_rx: Mutex<Option<mpsc::UnboundedReceiver<store::AccessLogEntry>>>,
    /// The running rooms a revocation reaches ([`CoreState::add_revocation_sink`]).
    revocation_sinks: parking_lot::RwLock<Vec<Arc<dyn RevocationSink>>>,
    /// `WORLDS_FORWARD_UPSTREAM_URL`: every local publish is queued for it in
    /// the publish transaction and sent by the `worlds-forward` job.
    pub forwarder: Option<Arc<crate::content::forward::Forwarder>>,
}

pub type SharedState = Arc<CoreState>;

impl CoreState {
    pub fn new(
        pool: PgPool,
        storage: Arc<Storage>,
        policy: Arc<Policy>,
        settings: Settings,
        eip1654: Option<Arc<dyn Eip1654Validator>>,
    ) -> Self {
        let (access_log_tx, access_log_rx) = mpsc::unbounded_channel();
        Self {
            pool,
            storage,
            policy,
            settings,
            eip1654,
            presence: Arc::new(Presence::new()),
            names: names::NameDenyList::default(),
            access_log_tx,
            access_log_rx: Mutex::new(Some(access_log_rx)),
            revocation_sinks: parking_lot::RwLock::new(Vec::new()),
            forwarder: None,
        }
    }

    /// Turns on upstream forwarding ([`forwarding::LANE`]); `serve` spawns its job.
    pub fn with_forwarder(
        mut self,
        forwarder: Option<Arc<crate::content::forward::Forwarder>>,
    ) -> Self {
        self.forwarder = forwarder;
        self
    }

    /// Hands the presence registry the read surface shares (`WorldsState`).
    pub fn with_presence(mut self, presence: Arc<Presence>) -> Self {
        self.presence = presence;
        self
    }

    /// Registers a running-room authority; `serve` adds the scene host and
    /// comms once they are built (they are built after this state).
    pub fn add_revocation_sink(&self, sink: Arc<dyn RevocationSink>) {
        self.revocation_sinks.write().push(sink);
    }

    /// Hands `revocation` to every registered room authority, after the row
    /// changed, before the route answers.
    pub async fn revoked(&self, revocation: Revocation) {
        let sinks: Vec<Arc<dyn RevocationSink>> = self.revocation_sinks.read().clone();
        for sink in sinks {
            sink.revoke(&revocation).await;
        }
        tracing::info!(?revocation, "revocation handed to the running rooms");
    }

    /// Re-decides `address` in `world` and revokes only when no longer
    /// admitted; an undetermined answer keeps it (the sweep re-checks).
    pub async fn revoke_unless_admitted(&self, world: &WorldName, address: &Address) {
        let now = chrono::Utc::now().naive_utc();
        match self
            .policy
            .can_access(world, address, None, None, None, now)
            .await
        {
            Ok(crate::policy::Access::Allowed(_)) => {}
            Ok(_) => {
                self.revoked(Revocation::Address {
                    world: world.clone(),
                    address: address.clone(),
                })
                .await
            }
            Err(error) => {
                tracing::warn!(%error, %world, %address, "access undetermined; participant kept")
            }
        }
    }

    /// `address` left, was kicked from or banned from `community`: each
    /// allow-list world listing it re-decides the wallet.
    pub async fn community_member_removed(&self, community: uuid::Uuid, address: &Address) {
        for world in self.worlds_listing_community(community).await {
            self.revoke_unless_admitted(&world, address).await;
        }
    }

    /// `community` was deleted, suspended or deactivated: each allow-list
    /// world listing it re-decides every participant.
    pub async fn community_closed(&self, community: uuid::Uuid) {
        for world in self.worlds_listing_community(community).await {
            self.revoked(Revocation::AccessSetting { world }).await;
        }
    }

    async fn worlds_listing_community(&self, community: uuid::Uuid) -> Vec<WorldName> {
        let names: Vec<String> = match sqlx::query_scalar(
            "SELECT wac.world_name FROM worlds.world_access_communities wac \
               JOIN worlds.worlds w ON w.name = wac.world_name \
             WHERE wac.community_id = $1 AND w.access_type = 'allow-list'",
        )
        .bind(community)
        .fetch_all(&self.pool)
        .await
        {
            Ok(names) => names,
            Err(error) => {
                tracing::warn!(%error, %community, "worlds listing the community unreadable; no revocation");
                return Vec::new();
            }
        };
        names
            .iter()
            .filter_map(|n| WorldName::parse(n).ok())
            .collect()
    }

    /// Queues an access-log row for the batched drain (failures were only
    /// ever logged).
    pub fn record_access(&self, world: &str, address: &str, action: &'static str, room: &str) {
        let entry = store::AccessLogEntry {
            world_name: world.to_lowercase(),
            address: address.to_lowercase(),
            action,
            room: room.to_string(),
        };
        if self.access_log_tx.send(entry).is_err() {
            tracing::warn!("world access log drain is gone; row dropped");
        }
    }

    /// Inserts whatever is queued right now, without the job (tests, and a
    /// process that never spawned it). A no-op once the job owns the receiver.
    pub async fn flush_access_log(&self) -> Result<usize, sqlx::Error> {
        let mut batch = Vec::new();
        {
            let mut guard = self.access_log_rx.lock();
            if let Some(rx) = guard.as_mut() {
                while let Ok(entry) = rx.try_recv() {
                    batch.push(entry);
                }
            }
        }
        store::insert_access_log(&self.pool, &batch).await?;
        Ok(batch.len())
    }

    /// The `worlds-access-log` job: drains the queue into `world_access_log`
    /// in batches until shutdown, flushing what is left on cancel.
    pub fn spawn_jobs(self: &Arc<Self>, jobs: &Jobs) {
        let Some(mut rx) = self.access_log_rx.lock().take() else {
            return;
        };
        let state = self.clone();
        jobs.spawn("worlds-access-log", move |ctx| async move {
            let mut batch = Vec::with_capacity(ACCESS_LOG_BATCH);
            loop {
                let received = tokio::select! {
                    n = rx.recv_many(&mut batch, ACCESS_LOG_BATCH) => n,
                    _ = ctx.cancel.cancelled() => {
                        while let Ok(entry) = rx.try_recv() {
                            batch.push(entry);
                        }
                        0
                    }
                };
                if !batch.is_empty()
                    && let Err(e) = store::insert_access_log(&state.pool, &batch).await
                {
                    tracing::warn!(error = %e, rows = batch.len(), "failed to persist world access log rows");
                }
                batch.clear();
                if received == 0 {
                    return Ok(());
                }
            }
        });
    }
}

/// The routes at their old paths. Merged at the root of the app, three of
/// them collide with the content server's (`POST /entities`,
/// `POST /entities/active`, `/entities/{...}`); [`root_router`] leaves those
/// three out, so the full surface answers under the worlds prefix and the
/// rest at the root too.
pub fn router(state: SharedState) -> Router {
    content_colliding_routes()
        .merge(non_colliding_routes())
        .route("/gc", post(admin::garbage_collect))
        .route("/admin/gc", post(admin::garbage_collect))
        .layer(axum::middleware::from_fn(nul_guard::nul_guard_middleware))
        .layer(axum::middleware::from_fn(cors::cors_middleware))
        .with_state(state)
}

pub fn root_router(state: SharedState) -> Router {
    non_colliding_routes()
        .layer(axum::middleware::from_fn(nul_guard::nul_guard_middleware))
        .layer(axum::middleware::from_fn(cors::cors_middleware))
        .with_state(state)
}

/// The two multipart routes run under their own deadlines (the upload and
/// processing timeouts), not the 30 s request timeout of the others.
fn upload_routes() -> Router<SharedState> {
    Router::new().route(
        "/world/{world_name}/settings",
        put(settings::update_world_settings).layer(DefaultBodyLimit::max(
            settings::MAX_SETTINGS_UPLOAD_WIRE_BYTES,
        )),
    )
}

fn content_colliding_routes() -> Router<SharedState> {
    Router::new()
        .route(
            "/entities",
            post(deploy::deploy_entity)
                .layer(DefaultBodyLimit::max(deploy::MAX_UPLOAD_WIRE_SIZE_BYTES)),
        )
        .route(
            "/entities/active",
            post(active::active_entities).layer(DefaultBodyLimit::max(256 * 1024)),
        )
        .route("/entities/{world_name}", delete(scenes::undeploy_world))
}

fn non_colliding_routes() -> Router<SharedState> {
    upload_routes().merge(timed_routes())
}

fn timed_routes() -> Router<SharedState> {
    Router::new()
        .route("/worlds", get(manifest::get_worlds))
        .route(
            "/world/{world_name}/manifest",
            get(manifest::get_world_manifest),
        )
        .route(
            "/world/{world_name}/permissions",
            get(permissions::get_permissions),
        )
        .route(
            "/world/{world_name}/permissions/{permission_name}",
            post(permissions::post_permissions),
        )
        .route(
            "/world/{world_name}/permissions/{permission_name}/{address}",
            put(permissions::put_permissions_address)
                .delete(permissions::delete_permissions_address),
        )
        .route(
            "/world/{world_name}/permissions/{permission_name}/address/{address}/parcels",
            get(permissions::get_allowed_parcels_for_permission)
                .post(permissions::post_permission_parcels)
                .delete(permissions::delete_permission_parcels),
        )
        .route(
            "/world/{world_name}/permissions/{permission_name}/parcels",
            post(permissions::get_addresses_for_parcel_permission),
        )
        .route(
            "/world/{world_name}/permissions/access/communities/{community_id}",
            put(permissions::put_permissions_access_community)
                .delete(permissions::delete_permissions_access_community),
        )
        .route(
            "/world/{world_name}/settings",
            get(settings::get_world_settings),
        )
        .route(
            "/world/{world_name}/scenes/{scene_coord}",
            delete(scenes::delete_scene),
        )
        .route("/admin/worlds", get(admin::list_worlds))
        .route("/admin/worlds/{world_name}", get(admin::world_detail))
        .route(
            "/admin/worlds/{world_name}/enable",
            post(admin::enable_world),
        )
        .route(
            "/admin/worlds/{world_name}/disable",
            post(admin::disable_world),
        )
        .route(
            "/admin/worlds/{world_name}/ban-status",
            get(admin::world_ban_status),
        )
        .route("/admin/blocked", get(admin::list_blocked))
        .route(
            "/admin/blocked/{wallet}",
            post(admin::block_wallet).delete(admin::unblock_wallet),
        )
        .route("/admin/access-log", get(admin::access_log))
        .route("/admin/forwarding", get(forwarding::list))
        .route("/admin/forwarding/{entity_id}", get(forwarding::get))
        .route(
            "/admin/forwarding/{entity_id}/retry",
            post(forwarding::retry),
        )
        .layer(axum::middleware::from_fn(request_timeout))
}

/// The old `request_timeout_layer(30)` over the non-upload routes.
async fn request_timeout(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    match tokio::time::timeout(Duration::from_secs(30), next.run(req)).await {
        Ok(response) => response,
        Err(_) => http::envelope(axum::http::StatusCode::REQUEST_TIMEOUT, "request timed out"),
    }
}

#[cfg(test)]
mod redaction_tests {
    use super::*;

    #[test]
    fn debug_output_never_carries_the_admin_token() {
        let s = Settings::for_tests(Some("admin-0xabab"));
        let shown = format!("{s:?}");
        assert!(
            !shown.contains("abab") && shown.contains("admin_token: Some(\"<set>\")"),
            "{shown}"
        );
    }
}
