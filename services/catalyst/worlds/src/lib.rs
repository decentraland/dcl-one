//! Worlds: the catalogue and scenes/files mirrors of an upstream worlds
//! content server (`mirror`), and the read routes a client boots a world
//! with: `/world/{name}/about`, `/world/{name}/scenes`, `/index` and the
//! worlds status with mirror freshness at `/worlds/status`. World files live
//! in the ONE blob store, so `GET /contents/{hash}` is the content module's
//! route; `/about`'s `scenesUrn` points there.
//!
//! Stage 5 adds the shared vocabulary and decisions the local write paths
//! and `comms` build on: the world identity types (`comms::world_identity`:
//! WorldName, SceneId, RealmId, ParcelSet, SceneNamespace, PlayerAddress),
//! `policy` (the permission core: owner/deployer/streamer/visitor/admin/ban
//! decisions over the shared SQL and the name/LAND authority; its access
//! vocabulary is `comms::access`) and `rooms` (the ONE room resolver: active
//! deployment, storage namespace and the old room-name contracts). The
//! deploy, permissions, settings, comms and storage routes are ported on
//! top of them: `core` (the worlds content server's product surface),
//! `storage` (world/player/env values), `scene_state` (the authoritative
//! scene host) and `fed` (the peer-listing mirror). Communication and
//! multiplayer (Pulse, the archipelago sockets, the gatekeeper and live
//! presence) lives in `catalyst-comms`, which takes `policy` through its
//! `AccessPolicy` interface and `rooms` through its `SceneIdentity` interface.
//! [`app_router`] composes them at the old gateway's prefixes behind the
//! [`writes`] switch.

// The crates this one builds on, under their module names (AGENTS.md).
pub(crate) use catalyst_chain_index as chain_index;
use catalyst_comms as comms;
pub(crate) use catalyst_config::config;
pub(crate) use catalyst_content as content;
#[cfg(test)]
pub(crate) use catalyst_db as db;
pub(crate) use catalyst_explore as explore;
pub(crate) use catalyst_jobs as jobs;
pub(crate) use catalyst_land_authz as land_authz;
pub(crate) use catalyst_shared as shared;
pub(crate) use dcl_auth as auth;
pub(crate) use dcl_http as http;

pub mod about;
mod available_content;
pub mod core;
pub mod fed;
pub mod index;
pub mod mirror;
pub mod policy;
pub mod preview_wearables;
pub mod queries;
pub mod rooms;
pub mod scene_state;
pub mod scenes;
pub mod status;
pub mod storage;
pub mod writes;

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use axum::Router;
use axum::routing::get;
use sqlx::PgPool;

use crate::config::Config;
use crate::jobs::{Jobs, PeriodicCfg};
use crate::shared::cors;
use crate::shared::nul_guard;
use catalyst_storage::Storage;

pub use crate::shared::identity::ZERO_ADDRESS;
pub use mirror::MirrorSettings;

/// What the read routes need from [`Config`], resolved once.
#[derive(Clone, Debug)]
pub struct Settings {
    /// This node's root URL: `scenesUrn` base and every `/contents/` link.
    pub public_url: String,
    /// The catalyst content/lambdas a world's visitors use for profiles and
    /// wearables; on a combined node these are this node's own.
    pub content_public_url: String,
    pub lambdas_public_url: String,
    pub network_id: i64,
    /// `GLOBAL_SCENES_URN`: whitespace-separated URNs every world loads.
    pub global_scenes_urn: Vec<String>,
    /// `WORLDS_REALM_NAME_STRIP_ENS`: strip `.dcl.eth` from the realm name of
    /// worlds published here so a stock client does not resolve them against
    /// Decentraland's registry. Mirrored worlds keep it either way.
    pub realm_name_strip_ens: bool,
    pub map_parcel_view_url: String,
    /// `MAP_ESTATE_VIEW_URL`.
    pub map_estate_view_url: String,
    pub commit_hash: Option<String>,
    /// `PREVIEW_WEARABLE_URNS`: the node-wide smart wearables a world with no
    /// selection of its own previews.
    pub preview_wearable_urns: Vec<String>,
    /// The signaling URL comms hands explorers (the resolved `LIVEKIT_HOST`);
    /// empty when no LiveKit is attached, which `/status` reports as offline.
    pub livekit_client_url: String,
    /// Whether `/about` advertises this node's world comms mint: LiveKit has
    /// real credentials and a signaling URL. Off serves `offline:offline`.
    pub comms_online: bool,
    /// The realm `/about`'s SFU liveness, shared: while it is down worlds
    /// advertise `offline:offline` too. It probes the realm's fixed adapter,
    /// assumed to be the same LiveKit as `LIVEKIT_HOST`; without that probe
    /// it is always alive.
    pub sfu: crate::content::about_comms::SfuHealth,
    /// `WORLDS_PERSONAL_WORLDS_MAX` / `WORLDS_PERSONAL_WORLD_MAX_SIZE_BYTES`
    /// as `/worlds/status` reports them (the policy itself lives in [`policy`]).
    pub personal_worlds_max: u64,
    pub personal_world_max_size_bytes: u64,
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

/// `NETWORK_ID` as the old worlds service read it, derived from `ETH_NETWORK`.
pub fn network_id_for(eth_network: &str) -> i64 {
    match eth_network.trim().to_ascii_lowercase().as_str() {
        "sepolia" => 11_155_111,
        _ => 1,
    }
}

impl Settings {
    /// The fields `Config` already has. The three worlds-specific ones
    /// (`global_scenes_urn`, `realm_name_strip_ens`, `map_estate_view_url`)
    /// take their old defaults; `serve` overrides them from the config once
    /// those flags exist.
    pub fn from_config(config: &Config) -> Self {
        let content = crate::content::Settings::from_config(config);
        let public_url = non_empty(config.node.public_url.as_deref())
            .unwrap_or_else(|| format!("http://{}", config.node.bind_addr()))
            .trim_end_matches('/')
            .to_string();
        Self {
            public_url,
            content_public_url: content.content_public_url,
            lambdas_public_url: content.lambdas_public_url,
            network_id: network_id_for(&config.node.eth_network),
            global_scenes_urn: Vec::new(),
            realm_name_strip_ens: true,
            map_parcel_view_url: content.map_parcel_view_url,
            map_estate_view_url: "http://127.0.0.1:5162/v1/estatemap.png".to_string(),
            commit_hash: content.commit_hash,
            preview_wearable_urns: preview_wearables::parse_urns(
                config
                    .worlds_content
                    .preview_wearable_urns
                    .as_deref()
                    .unwrap_or(""),
            ),
            livekit_client_url: String::new(),
            comms_online: false,
            sfu: crate::content::about_comms::SfuHealth::always_alive(),
            personal_worlds_max: config.worlds.worlds_personal_worlds_max,
            personal_world_max_size_bytes: config.worlds.worlds_personal_world_max_size_bytes,
        }
    }

    pub fn with_global_scenes_urn(mut self, raw: Option<&str>) -> Self {
        self.global_scenes_urn = raw
            .unwrap_or("")
            .split_whitespace()
            .map(str::to_string)
            .collect();
        self
    }

    pub fn for_tests(public_url: &str) -> Self {
        Self {
            public_url: public_url.trim_end_matches('/').to_string(),
            content_public_url: "https://peer.example.org/content".to_string(),
            lambdas_public_url: "https://peer.example.org/lambdas".to_string(),
            network_id: 1,
            global_scenes_urn: Vec::new(),
            realm_name_strip_ens: true,
            map_parcel_view_url: "http://127.0.0.1:5162/v1/minimap.png".to_string(),
            map_estate_view_url: "http://127.0.0.1:5162/v1/estatemap.png".to_string(),
            commit_hash: None,
            preview_wearable_urns: Vec::new(),
            livekit_client_url: String::new(),
            comms_online: false,
            sfu: crate::content::about_comms::SfuHealth::always_alive(),
            personal_worlds_max: 0,
            personal_world_max_size_bytes: policy::DEFAULT_PERSONAL_WORLD_MAX_SIZE_BYTES as u64,
        }
    }
}

pub struct WorldsState {
    pub pool: PgPool,
    pub storage: Arc<Storage>,
    pub settings: Settings,
    /// The live presence (`comms::presence`, shared with `core::CoreState`),
    /// read by `/status`; a fresh, empty one until `with_presence` shares it.
    pub presence: Arc<comms::presence::Presence>,
    /// `None` when neither `WORLDS_CONTENT_MIRROR_ENABLED` nor the read-through
    /// ([`MirrorSettings::from_config`]) is on: the tables then only ever hold local rows.
    pub mirror: Option<MirrorSettings>,
    pub http: crate::http::Outbound,
    /// `/world/{name}/preview-wearables`' resolved entities (120 s).
    pub preview_memo: crate::shared::cache::TtlMap<String, Arc<Vec<serde_json::Value>>>,
    /// `READ_THROUGH_ENABLED` with the mirror on: a world held with no scene is mirrored on
    /// first read ([`WorldsState::fill_world`]).
    read_through: OnceLock<WorldReadThrough>,
}

pub type SharedState = Arc<WorldsState>;

/// What a world read-through runs with: its fills, the download bound the content read-through
/// shares, and the policy a mirrored scene's signer must satisfy.
#[derive(Clone)]
pub struct WorldReadThrough {
    pub fills: crate::content::read_through::Fills,
    pub downloads: crate::content::read_through::Downloads,
    pub policy: Arc<policy::Policy>,
}

impl WorldsState {
    pub fn new(
        pool: PgPool,
        storage: Arc<Storage>,
        settings: Settings,
        mirror: Option<MirrorSettings>,
        http: crate::http::Outbound,
    ) -> Self {
        Self {
            pool,
            storage,
            settings,
            presence: Arc::new(comms::presence::Presence::new()),
            mirror,
            http,
            preview_memo: preview_wearables::memo(),
            read_through: OnceLock::new(),
        }
    }

    /// Attaches the read-through once; `false` if one was attached already. Without a
    /// mirror it never fills: the mirror's source and guards are the world source.
    pub fn attach_read_through(&self, read_through: WorldReadThrough) -> bool {
        self.read_through.set(read_through).is_ok()
    }

    pub fn read_through(&self) -> Option<&WorldReadThrough> {
        self.read_through.get()
    }

    /// Mirrors a world this node holds no scene of from the mirror source, waiting at most
    /// the read-through's guard; `true` when the caller should read again.
    pub async fn fill_world(&self, name: &str) -> bool {
        let (Some(read_through), Some(mirror)) = (self.read_through.get(), self.mirror.clone())
        else {
            return false;
        };
        let Ok(world) = crate::comms::world_identity::WorldName::parse(name) else {
            return false;
        };
        if !world.is_dcl_world_name() {
            return false;
        }
        let key = world.key().to_string();
        if !mirror.names.is_empty() && !mirror.names.contains(&key) {
            return false;
        }
        let (http, pool, storage) = (self.http.clone(), self.pool.clone(), self.storage.clone());
        let checks = read_through.clone();
        read_through
            .fills
            .fill(vec![format!("world:{key}")], move |_| async move {
                let verify = mirror::Verify {
                    policy: &checks.policy,
                    downloads: &checks.downloads,
                };
                if let Err(e) =
                    mirror::mirror_one(&http, &pool, &storage, &mirror, &key, &verify).await
                {
                    tracing::warn!(world = %key, error = %e, "read-through: world not mirrored");
                }
            })
            .await
    }

    pub fn with_presence(mut self, presence: Arc<comms::presence::Presence>) -> Self {
        self.presence = presence;
        self
    }

    /// Registers the mirror as the periodic job `worlds-content-mirror`: one pass is
    /// the catalogue (`GET /index`) followed by the scenes/files walk, which
    /// resumes from its persisted cursor after a restart. Nothing is spawned
    /// without a mirror source.
    pub fn spawn_jobs(self: &Arc<Self>, jobs: &Jobs) {
        let Some(mirror) = self.mirror.clone().filter(|m| m.scheduled) else {
            tracing::info!(
                read_through = self.mirror.is_some(),
                "WORLDS_CONTENT_MIRROR_ENABLED=false: no worlds mirror pass"
            );
            return;
        };
        let state = self.clone();
        let cfg = PeriodicCfg::every(mirror.interval)
            .with_jitter(Duration::from_secs(30))
            .with_error_backoff(Duration::from_secs(3600));
        jobs.spawn_periodic_leased("worlds-content-mirror", cfg, move |ctx| {
            let (state, mirror) = (state.clone(), mirror.clone());
            async move {
                let summary =
                    mirror::run_pass(&state.http, &state.pool, &state.storage, &mirror, &ctx)
                        .await?;
                tracing::info!(?summary, "worlds mirror pass");
                Ok(())
            }
        });
    }
}

/// The worlds read routes with the same CORS/NUL guards as the content
/// routes, so the router can be merged into the app or served on its own.
pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/world/{world_name}/about", get(about::get_about))
        .route("/world/{world_name}/scenes", get(scenes::list_scenes))
        .route(
            "/world/{world_name}/preview-wearables",
            get(preview_wearables::get_preview_wearables),
        )
        .route("/index", get(index::get_index))
        .route("/worlds/status", get(status::get_status))
        .layer(axum::middleware::from_fn(nul_guard::nul_guard_middleware))
        .layer(axum::middleware::from_fn(cors::cors_middleware))
        .with_state(state)
}

/// The stage-5 worlds states `serve` builds; [`app_router`] owns their
/// routes.
pub struct Stage5 {
    pub core: core::SharedState,
    pub storage: storage::SharedState,
    /// `None` when `SCENE_HOST_ENABLED=false`.
    pub scene_host: Option<scene_state::SharedState>,
    pub fed: Arc<fed::FedState>,
    /// `WORLDS_WRITES_ENABLED`.
    pub writes_enabled: bool,
}

/// The old gateway's location table for the worlds services
/// (`catalyrst/nixos/web-gateway.nix`, `web.nix`): `/worlds-content-server`
/// (stripped; the vhost of the same name served the root), `/world-storage/`
/// (stripped, `x-original-path` set) and `/scene-state` (stripped, a
/// WebSocket); archipelago's `/archipelago-ea-stats` and root routes are
/// comms', mounted by `serve` right after this router. Here every prefix is
/// nested and the routers whose paths do not collide with the content
/// server's are merged at the root too: the worlds reads and
/// `core::root_router` (everything but `POST /entities`, `POST
/// /entities/active` and `DELETE /entities/{name}`, which are the content
/// server's at the root) and the federation routes. The
/// `/worlds-content-server` nest carries the stage-4 reads, the worlds
/// status, `GET /contents/{hash}` and `GET /available-content` from the ONE
/// blob store and the full product surface. The core, storage and federation routers sit behind
/// [`writes::guard`] with the process's `WORLDS_WRITES_ENABLED`.
pub fn app_router(
    worlds: SharedState,
    blobs: Arc<crate::content::ContentState>,
    s: &Stage5,
) -> Router {
    let guard = |r: Router| writes::guard(r, s.writes_enabled);
    let content_server = router(worlds.clone())
        .route(
            "/status",
            get(status::get_status).with_state(worlds.clone()),
        )
        .route(
            "/contents/{hashId}",
            get(crate::content::get_content::get_content)
                .head(crate::content::get_content::get_content)
                .with_state(blobs.clone()),
        )
        // The deploy clients' pre-upload check, over the same blob store.
        .route(
            "/available-content",
            get(available_content::get)
                .with_state(blobs.clone())
                .layer(axum::middleware::from_fn(nul_guard::nul_guard_middleware))
                .layer(axum::middleware::from_fn(cors::cors_middleware)),
        )
        .route(
            "/ipfs/{hashId}",
            get(crate::content::get_content::get_content)
                .head(crate::content::get_content::get_content)
                .with_state(blobs),
        )
        .route(
            "/openapi.json",
            get(|| async { crate::shared::openapi::document(crate::shared::openapi::WORLDS) }),
        )
        .merge(guard(core::router(s.core.clone())));
    let mut app = Router::new()
        .merge(router(worlds))
        .merge(guard(core::root_router(s.core.clone())))
        .nest("/worlds-content-server", content_server)
        .nest("/world-storage", guard(storage::router(s.storage.clone())))
        .merge(guard(fed::router(s.fed.clone())));
    if let Some(host) = &s.scene_host {
        app = app.nest(scene_state::ROUTE_PREFIX, scene_state::router(host.clone()));
    }
    app
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_derive_the_worlds_urls_from_the_node_config() {
        let config = Config::parse_for_tests(&[
            "x",
            "--database-url",
            "postgres:///c",
            "--public-url",
            "https://worlds.example.org/",
            "--eth-network",
            "sepolia",
            "--preview-wearable-urns",
            "urn:a, urn:b",
        ])
        .unwrap();
        let s = Settings::from_config(&config).with_global_scenes_urn(Some(" urn:a  urn:b "));
        assert_eq!(s.public_url, "https://worlds.example.org");
        assert_eq!(s.content_public_url, "https://worlds.example.org/content/");
        assert_eq!(s.lambdas_public_url, "https://worlds.example.org/lambdas/");
        assert_eq!(s.network_id, 11_155_111);
        assert_eq!(s.global_scenes_urn, vec!["urn:a", "urn:b"]);
        assert!(s.realm_name_strip_ens);
        assert_eq!(s.preview_wearable_urns, vec!["urn:a", "urn:b"]);

        let config = Config::parse_for_tests(&["x", "--database-url", "postgres:///c"]).unwrap();
        let s = Settings::from_config(&config);
        assert_eq!(s.public_url, "http://127.0.0.1:5210");
        assert_eq!(s.network_id, 1);
        assert!(s.global_scenes_urn.is_empty());
    }
}
