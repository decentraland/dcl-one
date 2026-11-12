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
pub mod holdings;
pub mod http;
pub mod limits;
pub mod manifest;
pub mod metrics;
pub mod names;
pub mod notices;
pub mod owners;
pub mod partial;
pub mod permissions;
pub mod quota;
pub mod reprocess;
pub mod scenes;
pub mod settings;
pub mod settings_policy;
pub mod store;
pub mod upload_limits;
pub mod upload_spool;
pub mod wallet;

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

pub use partial::PartialUploadLimits;
pub use upload_limits::UploadLimits;

/// Rows per multi-row access-log INSERT; the drain takes whatever is queued up to this.
const ACCESS_LOG_BATCH: usize = 256;

/// `WORLDS_OWNER_RECONCILE_INTERVAL_SECS`' default: 12 h, as the upstream's job.
pub const DEFAULT_OWNER_RECONCILE_INTERVAL: std::time::Duration =
    std::time::Duration::from_secs(12 * 3600);
/// WCS `SCENE_EVICTION_TTL_MS` default: 7 days.
pub const DEFAULT_SCENE_EVICTION_TTL: std::time::Duration =
    std::time::Duration::from_secs(7 * 24 * 3600);

/// What the write surface needs beside the shared handles.
#[derive(Clone)]
pub struct Settings {
    /// `CATALYRST_WORLDS_ADMIN_TOKEN`: the bearer of the `/admin/*` routes; unset refuses them.
    pub admin_token: Option<String>,
    /// `MAX_IN_FLIGHT_UPLOAD_BYTES`, `MAX_CONCURRENT_UPLOADS`,
    /// `MAX_IN_FLIGHT_UPLOAD_FILES`, `MULTIPART_UPLOAD_TIMEOUT_MS`,
    /// `DEPLOYMENT_PROCESSING_TIMEOUT_MS`, `MAX_CONCURRENT_UPLOADS_PER_SOURCE`,
    /// `MAX_IN_FLIGHT_UPLOAD_BYTES_PER_SOURCE`.
    pub upload_limits: UploadLimits,
    /// `GLOBAL_SCENES_URN`: scenes the configuration names, kept by the blob
    /// store collector (`/admin/gc`).
    pub global_scenes_urn: Vec<String>,
    /// `SCENE_EVICTION_TTL_MS`: how long a retired scene is kept.
    pub scene_eviction_ttl: std::time::Duration,
    /// This node's root URL, the realm a deploy's success message links.
    pub public_url: String,
    /// `MAX_FILE_COUNT`: entries a deployed entity's `content` may list.
    pub max_file_count: usize,
    /// The partial (multi-request) upload limits (`PENDING_DEPLOYMENT_TTL`, `MAX_PENDING_*`, ...).
    pub partial_uploads: PartialUploadLimits,
    /// `WORLDS_OWNER_RECONCILE_INTERVAL_SECS`: the `worlds-owner-reconcile` pause.
    pub owner_reconcile_interval: std::time::Duration,
    /// `ACCESS_MAX_WALLETS`, `ACCESS_MAX_COMMUNITIES`: the access allow-list caps.
    pub access_caps: store::AccessCaps,
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
            scene_eviction_ttl: DEFAULT_SCENE_EVICTION_TTL,
            public_url: String::new(),
            max_file_count: deploy::DEFAULT_MAX_FILE_COUNT,
            partial_uploads: PartialUploadLimits::default(),
            owner_reconcile_interval: DEFAULT_OWNER_RECONCILE_INTERVAL,
            access_caps: store::AccessCaps::default(),
        })
    }

    /// The partial upload limits, validated against the world size cap.
    pub fn with_partial_uploads(mut self, limits: PartialUploadLimits) -> anyhow::Result<Self> {
        self.partial_uploads = limits.validate(crate::policy::MAX_SCENE_SIZE_BYTES)?;
        Ok(self)
    }

    pub fn with_scene_eviction_ttl_ms(mut self, ms: u64) -> Self {
        self.scene_eviction_ttl = std::time::Duration::from_millis(ms);
        self
    }

    pub fn with_public_url(mut self, public_url: &str) -> Self {
        self.public_url = public_url.trim_end_matches('/').to_string();
        self
    }

    pub fn with_owner_reconcile_interval_secs(mut self, secs: u64) -> Self {
        self.owner_reconcile_interval = std::time::Duration::from_secs(secs);
        self
    }

    pub fn with_access_caps(mut self, wallets: u32, communities: u32) -> Self {
        self.access_caps = store::AccessCaps {
            wallets: wallets.into(),
            communities: communities.into(),
        };
        self
    }

    pub fn with_max_file_count(mut self, max_file_count: u32) -> Self {
        self.max_file_count = max_file_count as usize;
        self
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
            scene_eviction_ttl: DEFAULT_SCENE_EVICTION_TTL,
            public_url: "https://worlds.example".to_string(),
            max_file_count: deploy::DEFAULT_MAX_FILE_COUNT,
            partial_uploads: PartialUploadLimits::default(),
            owner_reconcile_interval: DEFAULT_OWNER_RECONCILE_INTERVAL,
            access_caps: store::AccessCaps::default(),
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
    /// Re-evaluate current policy without invalidating shared-secret admissions.
    /// This advisory carries no persisted access-change cutoff.
    AccessRecheck { world: WorldName },
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
            Revocation::AccessRecheck { world } => RoomRevocation::WorldAccessRecheck { world },
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
    /// The name deny list, the same instance as `policy`'s ([`Policy::names`]).
    pub names: Arc<names::NameDenyList>,
    access_log_tx: mpsc::UnboundedSender<store::AccessLogEntry>,
    /// Taken by `spawn_jobs`; `flush_access_log` drains it in process until then.
    access_log_rx: Mutex<Option<mpsc::UnboundedReceiver<store::AccessLogEntry>>>,
    /// The running rooms a revocation reaches ([`CoreState::add_revocation_sink`]).
    revocation_sinks: parking_lot::RwLock<Vec<Arc<dyn RevocationSink>>>,
    /// `WORLDS_FORWARD_UPSTREAM_URL`: every local publish is queued for it in
    /// the publish transaction and sent by the `worlds-forward` job.
    pub forwarder: Option<Arc<crate::content::forward::Forwarder>>,
    /// The per-wallet storage allowance (unlimited without chain reads).
    pub allowance: Arc<dyn quota::SpaceAllowance>,
    /// The user notices (permission grants, storage quota) and their links.
    pub notices: Arc<notices::Notices>,
    /// The local asset-bundle converter `POST /reprocess-ab` hands scenes to
    /// (`ABGEN_ADMIN_URL`); `None` answers that route 500.
    pub converter: Option<reprocess::Converter>,
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
        let names = policy.names().clone();
        Self {
            pool,
            storage,
            policy,
            settings,
            eip1654,
            presence: Arc::new(Presence::new()),
            names,
            access_log_tx,
            access_log_rx: Mutex::new(Some(access_log_rx)),
            revocation_sinks: parking_lot::RwLock::new(Vec::new()),
            forwarder: None,
            allowance: Arc::new(quota::Unlimited),
            notices: Arc::new(notices::Notices::default()),
            converter: None,
        }
    }

    /// The asset-bundle converter `POST /reprocess-ab` hands scenes to.
    pub fn with_converter(mut self, converter: Option<reprocess::Converter>) -> Self {
        self.converter = converter;
        self
    }

    /// The storage allowance source ([`holdings::configured`]).
    pub fn with_allowance(mut self, allowance: Arc<dyn quota::SpaceAllowance>) -> Self {
        self.allowance = allowance;
        self
    }

    /// Where user notices go (social's directory) and the Builder they link.
    pub fn with_notices(mut self, notices: notices::Notices) -> Self {
        self.notices = Arc::new(notices);
        self
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
        // Persist the entire affected set before advisory wakeups. Preserve an
        // earlier edit's cutoff/revision so delayed closures cannot revoke fresh
        // secret admissions or erase newer edit obligations.
        let names: Vec<String> = match sqlx::query_scalar(
            "INSERT INTO social.world_access_rechecks AS q(world_name) \
             SELECT wac.world_name FROM worlds.world_access_communities wac \
             JOIN worlds.worlds w ON w.name=wac.world_name \
             WHERE wac.community_id=$1 AND w.access_type='allow-list' \
             ON CONFLICT(world_name) DO UPDATE SET next_attempt_at=LEAST(q.next_attempt_at,timezone('UTC',now())) \
             RETURNING world_name",
        ).bind(community).fetch_all(&self.pool).await {
            Ok(names) => names,
            Err(error) => {
                tracing::warn!(%error, %community, "community closure recheck could not be persisted; attempting advisory policy recheck");
                for world in self.worlds_listing_community(community).await {
                    self.revoked(Revocation::AccessRecheck { world }).await;
                }
                return;
            }
        };
        for world in names.iter().filter_map(|name| WorldName::parse(name).ok()) {
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
    /// in batches until shutdown, flushing what is left on cancel; the
    /// fleet-wide hourly `worlds-scene-eviction` of expired retired scenes;
    /// the fleet-wide `worlds-owner-reconcile` ([`owners`]); and the
    /// fleet-wide `worlds-partial-upload-cleanup` of expired partial uploads.
    pub fn spawn_jobs(self: &Arc<Self>, jobs: &Jobs) {
        let state = self.clone();
        jobs.spawn_periodic_leased(
            "worlds-owner-reconcile",
            crate::jobs::PeriodicCfg::every(self.settings.owner_reconcile_interval)
                .with_jitter(std::time::Duration::from_secs(60)),
            move |_ctx| {
                let state = state.clone();
                async move {
                    let report = owners::reconcile_owners(&state)
                        .await
                        .map_err(|e| anyhow::anyhow!("reconciling world owners: {e:?}"))?;
                    tracing::info!(?report, "world owners reconciled");
                    Ok(())
                }
            },
        );
        let state = self.clone();
        jobs.spawn_periodic_leased(
            "worlds-partial-upload-cleanup",
            crate::jobs::PeriodicCfg::every(std::time::Duration::from_millis(
                self.settings.partial_uploads.cleanup_interval_ms,
            )),
            move |_ctx| {
                let state = state.clone();
                async move {
                    partial::delete_expired_uploads(&state).await;
                    Ok(())
                }
            },
        );
        metrics::partial_capacity(self.settings.partial_uploads.max_pending_bytes);
        let state = self.clone();
        jobs.spawn_periodic_leased(
            "worlds-scene-eviction",
            crate::jobs::PeriodicCfg::every(std::time::Duration::from_secs(3600)),
            move |_ctx| {
                let state = state.clone();
                async move {
                    let evicted =
                        store::evict_retired_scenes(&state.pool, state.settings.scene_eviction_ttl)
                            .await
                            .map_err(|e| anyhow::anyhow!("evicting retired scenes: {e:?}"))?;
                    tracing::info!(evicted, "retired world scenes evicted");
                    Ok(())
                }
            },
        );
        let state = self.clone();
        jobs.spawn_periodic_leased(
            "worlds-quota",
            crate::jobs::PeriodicCfg::every(quota::QUOTA_PASS_INTERVAL),
            move |_ctx| {
                let state = state.clone();
                async move { state.quota_pass().await }
            },
        );
        let limits = &self.settings.upload_limits;
        metrics::upload_capacity(limits);
        let spool_root = upload_spool::spool_root(&self.storage);
        // Older than any live upload: left by a process that died mid-request.
        let stale_after = catalyst_storage::gc::orphan_age_floor(
            limits.multipart_upload_timeout_ms,
            limits.deployment_processing_timeout_ms,
        );
        tokio::spawn(async move {
            upload_spool::sweep_stale(&spool_root, stale_after).await;
        });
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
        .route("/reprocess-ab", post(reprocess::reprocess_ab))
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
            post(permissions::get_addresses_for_parcel_permission)
                .put(permissions::put_permissions_parcels_address)
                .delete(permissions::delete_permissions_parcels_address),
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
        .route("/world/{world_name}/limits", get(limits::get_world_limits))
        .route(
            "/world/{world_name}/scenes/{scene_coord}",
            delete(scenes::delete_scene),
        )
        .route("/wallet/contribute", get(wallet::get_contributable_domains))
        .route("/wallet/{wallet}/stats", get(wallet::get_wallet_stats))
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
