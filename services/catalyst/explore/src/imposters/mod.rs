//! The imposter supply for the bevy-explorer web client's distant LOD
//! (the old `catalyrst-bvimposters` single, port 5165, which the explorer
//! reaches at the `/bvimposters` prefix baked into its wasm):
//! `GET /imposters/realms/{realm}/{level}/{x},{y}.{crc}.zip` and the
//! `-spec.json` member of the same zip, served from a crc-keyed,
//! realm-independent zip store ([`crate::filecache`], its own
//! bounded directory beside the ONE hash-addressed store because the key is
//! tile + crc, not a content hash); the crc is the ETag, so a matching
//! `If-None-Match` costs no store work; `GET /status` reports the store.
//! Every miss is a plain 404 (the explorer treats an absent imposter as
//! "none yet"). One periodic job sweeps interrupted landings and evicts down
//! to the byte budget.
//!
//! This node renders its own imposters: there is no upstream imposter store
//! to read through. With `BVIMPOSTERS_BAKE_ENABLED` a miss queues the tile
//! for the engine's `impost` renderer ([`bake`]), unless its bakes keep
//! failing ([`quarantine`]), and the zips it writes land in the store; off,
//! the store holds what an operator seeded. The queue sits beside the store
//! and one leased process bakes it, so every process serving the store (an
//! API replica too) serves the route alike and its misses all get baked. The operator subcommands
//! `catalyst bvimposters seed <dir>` ([`seed`]) and `catalyst bvimposters
//! quarantine [list]` ([`quarantine_list::apply`]) work on the same store.

pub mod bake;
pub mod key;
pub mod quarantine;
pub mod quarantine_list;
pub mod seed;
pub mod zips;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path, State as AxState};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde_json::json;

use crate::filecache::{FileCache, Served, Source, Supply};
use crate::jobs::{Jobs, PeriodicCfg};
use key::{ImposterKey, parse_spec_request, parse_zip_request};

pub const DEFAULT_STORE_ROOT: &str = "/var/lib/catalyst/bvimposters";
pub const DEFAULT_STORE_MAX_BYTES: u64 = 21_474_836_480;
pub const DEFAULT_MEM_CACHE_BYTES: u64 = 268_435_456;
const MAINTENANCE_INTERVAL: Duration = Duration::from_secs(600);

/// The old `BVIMPOSTERS_*` variables as `Config` carries them.
#[derive(Clone, Debug, Default)]
pub struct RawSettings {
    pub store_root: Option<String>,
    pub store_max_bytes: Option<u64>,
    pub mem_cache_bytes: Option<u64>,
    pub bake: RawBakeSettings,
}

/// The old `BVIMPOSTERS_BAKE_*` / `BVIMPOSTERS_IMPOST_*` variables.
#[derive(Clone, Debug, Default)]
pub struct RawBakeSettings {
    pub enabled: bool,
    pub wrapper: Option<String>,
    pub impost_bin: Option<String>,
    pub impost_server: Option<String>,
    pub impost_content_server: Option<String>,
    pub impost_sdk6_adaption_layer: Option<String>,
    pub queue_depth: Option<u64>,
    pub timeout_secs: Option<u64>,
    pub max_failures: Option<u64>,
    pub quarantine_secs: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub store_root: PathBuf,
    pub store_max_bytes: u64,
    pub mem_cache_bytes: usize,
    /// Bake-on-miss; `None` = off.
    pub bake: Option<bake::BakeConfig>,
    pub bake_max_failures: u32,
    pub bake_quarantine_secs: u64,
}

impl Settings {
    pub fn resolve(raw: RawSettings) -> anyhow::Result<Self> {
        let non_empty = |v: &Option<String>| {
            v.as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        };
        let store_root = PathBuf::from(
            non_empty(&raw.store_root).unwrap_or_else(|| DEFAULT_STORE_ROOT.to_string()),
        );
        let bake = if raw.bake.enabled {
            let server = non_empty(&raw.bake.impost_server)
                .context("BVIMPOSTERS_IMPOST_SERVER (the realm the renderer loads) is required with BVIMPOSTERS_BAKE_ENABLED")?;
            Some(bake::BakeConfig {
                wrapper: non_empty(&raw.bake.wrapper).unwrap_or_default(),
                bin: non_empty(&raw.bake.impost_bin).unwrap_or_else(|| "impost".to_string()),
                server,
                content_server: non_empty(&raw.bake.impost_content_server).unwrap_or_default(),
                sdk6_adaption_layer: non_empty(&raw.bake.impost_sdk6_adaption_layer)
                    .unwrap_or_default(),
                timeout: Duration::from_secs(
                    raw.bake.timeout_secs.filter(|s| *s > 0).unwrap_or(1800),
                ),
                queue_depth: raw.bake.queue_depth.unwrap_or(1).clamp(1, 2) as usize,
            })
        } else {
            None
        };
        Ok(Self {
            store_root,
            store_max_bytes: raw
                .store_max_bytes
                .filter(|b| *b > 0)
                .unwrap_or(DEFAULT_STORE_MAX_BYTES),
            mem_cache_bytes: raw
                .mem_cache_bytes
                .filter(|b| *b > 0)
                .unwrap_or(DEFAULT_MEM_CACHE_BYTES) as usize,
            bake,
            bake_max_failures: raw.bake.max_failures.filter(|n| *n > 0).unwrap_or(3) as u32,
            bake_quarantine_secs: raw.bake.quarantine_secs.unwrap_or(86_400),
        })
    }

    pub fn for_tests(store_root: PathBuf) -> Self {
        Self {
            store_root,
            store_max_bytes: u64::MAX,
            mem_cache_bytes: 64 << 20,
            bake: None,
            bake_max_failures: 3,
            bake_quarantine_secs: 86_400,
        }
    }
}

pub struct State {
    pub settings: Settings,
    pub store: Arc<FileCache>,
    pub supply: Supply<ImposterKey>,
    /// The bake failure quarantine (`<store root>/quarantine.json`).
    pub quarantine: Arc<quarantine::Quarantine>,
    pub bake: Option<Arc<bake::BakeQueue>>,
}

pub type Shared = Arc<State>;

impl State {
    /// Creates the store directories and runs the boot sweep.
    pub async fn new(settings: Settings) -> anyhow::Result<Shared> {
        let store = Arc::new(FileCache::new(
            settings.store_root.clone(),
            settings.store_max_bytes,
        ));
        store.init().with_context(|| {
            format!(
                "initializing the imposter store at {}",
                settings.store_root.display()
            )
        })?;
        {
            let store = store.clone();
            tokio::task::spawn_blocking(move || store.maintenance_pass())
                .await
                .context("imposter store boot sweep")?;
        }
        tracing::info!(
            store = %settings.store_root.display(),
            budget_bytes = settings.store_max_bytes,
            bake = settings.bake.is_some(),
            "imposter supply"
        );
        let quarantine = Arc::new(quarantine::Quarantine::load(
            settings.store_root.join("quarantine.json"),
            settings.bake_max_failures,
            settings.bake_quarantine_secs,
        ));
        let bake = settings.bake.clone().map(|cfg| {
            tracing::info!(bin = %cfg.bin, server = %cfg.server, queue_depth = cfg.queue_depth, "imposter bake-on-miss enabled");
            bake::BakeQueue::new(cfg, store.clone(), quarantine.clone())
        });
        Ok(Arc::new(Self {
            supply: Supply::new(store.clone(), settings.mem_cache_bytes),
            settings,
            store,
            quarantine,
            bake,
        }))
    }

    fn maybe_enqueue_bake(&self, tile: key::TileKey) {
        let Some(bake) = self.bake.as_ref() else {
            return;
        };
        if self.quarantine.is_quarantined(&tile) {
            tracing::info!(tile = %tile.label(), "bake suppressed, tile quarantined");
            return;
        }
        if bake.enqueue(tile) {
            tracing::info!(tile = %tile.label(), "bake enqueued");
        }
    }

    /// The sweep + eviction pass every ten minutes, after the boot pass.
    pub fn spawn_jobs(self: &Arc<Self>, jobs: &Jobs) {
        if let Some(bake) = &self.bake {
            bake.spawn_worker(jobs);
        }
        let store = self.store.clone();
        jobs.spawn_periodic(
            "imposters-maintenance",
            PeriodicCfg::every(MAINTENANCE_INTERVAL).after_first_interval(),
            move |_ctx| {
                let store = store.clone();
                async move {
                    tokio::task::spawn_blocking(move || store.maintenance_pass())
                        .await
                        .context("imposter store maintenance")?;
                    Ok(())
                }
            },
        );
    }
}

/// `/status`, `/ping` and the tile route; mounted under `/bvimposters`.
pub fn router(state: Shared) -> Router {
    Router::new()
        .route("/status", get(status))
        .route("/ping", get(|| async { "ok" }))
        .route("/imposters/realms/{realm}/{level}/{file}", get(imposter))
        .with_state(state)
}

async fn status(AxState(state): AxState<Shared>) -> Response {
    let usage = state.store.usage_snapshot();
    let (bake_queue, bake_inflight) = state
        .bake
        .as_ref()
        .map(|bake| bake.snapshot())
        .unwrap_or_default();
    let quarantine: Vec<_> = state
        .quarantine
        .entries()
        .into_iter()
        .map(|(key, entry)| json!({ "key": key, "until": entry.until, "failures": entry.failures }))
        .collect();
    axum::Json(json!({
        "store_bytes": usage.bytes,
        "store_entries": usage.entries,
        "budget_bytes": state.store.max_bytes(),
        "bake_enabled": state.bake.is_some(),
        "bake_queue": bake_queue.iter().map(key::TileKey::label).collect::<Vec<_>>(),
        "bake_inflight": bake_inflight.map(|tile| tile.label()),
        "quarantine": quarantine,
    }))
    .into_response()
}

pub async fn imposter(
    AxState(state): AxState<Shared>,
    Path((_realm, level, file)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Response {
    if file.ends_with("-spec.json") {
        return spec(&state, &level, &file).await;
    }
    let Some(key) = parse_zip_request(&level, &file) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if etag_matches(&headers, key.crc) {
        return not_modified(key.crc);
    }
    match state.supply.get_stored(&key.rel()).await {
        Served::Hit(bytes, source) => zip_response(bytes, key.crc, source),
        Served::Miss => {
            state.maybe_enqueue_bake(key.tile);
            StatusCode::NOT_FOUND.into_response()
        }
    }
}

async fn spec(state: &Shared, level: &str, file: &str) -> Response {
    let Some(key) = parse_spec_request(level, file) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(zip) = state.supply.get_local(&key, &key.rel()).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match zips::extract_spec(&zip, &key) {
        Ok(spec) => (
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            )],
            spec,
        )
            .into_response(),
        Err(e) => {
            tracing::warn!(%level, %file, error = %e, "stored zip missing spec member");
            StatusCode::NOT_FOUND.into_response()
        }
    }
}

fn etag_value(crc: u32) -> Option<HeaderValue> {
    HeaderValue::from_str(&format!("\"{crc}\"")).ok()
}

/// The crc in the key is the ETag; a matching If-None-Match needs no store work.
fn etag_matches(headers: &HeaderMap, crc: u32) -> bool {
    let Some(raw) = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    let wanted = format!("\"{crc}\"");
    raw.split(',').map(str::trim).any(|tag| {
        tag == "*" || tag == wanted || tag.strip_prefix("W/").is_some_and(|t| t == wanted)
    })
}

fn not_modified(crc: u32) -> Response {
    let mut response = StatusCode::NOT_MODIFIED.into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );
    if let Some(etag) = etag_value(crc) {
        headers.insert(header::ETAG, etag);
    }
    response
}

fn zip_response(bytes: Bytes, crc: u32, source: Source) -> Response {
    let source_value = match source {
        Source::Store => HeaderValue::from_static("store"),
        Source::Upstream => HeaderValue::from_static("cdn"),
    };
    let mut response = (
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/zip"),
            ),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=31536000, immutable"),
            ),
        ],
        bytes,
    )
        .into_response();
    if let Some(etag) = etag_value(crc) {
        response.headers_mut().insert(header::ETAG, etag);
    }
    response
        .headers_mut()
        .insert(HeaderName::from_static("x-bvi-source"), source_value);
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filecache::tests::{TempDir, tempdir};

    async fn state_with(dir: &TempDir) -> Shared {
        let settings = Settings::for_tests(dir.0.join("root"));
        std::fs::create_dir_all(&settings.store_root).unwrap();
        State::new(settings).await.unwrap()
    }

    fn put_zip(state: &Shared, key: &ImposterKey) {
        let path = state.store.path(&key.rel()).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, zips::test_zip_bytes(key.tile.x, key.tile.y, key.crc)).unwrap();
    }

    async fn get_zip_with(state: &Shared, key: &ImposterKey, headers: HeaderMap) -> Response {
        imposter(
            AxState(state.clone()),
            Path((
                "content".to_string(),
                key.tile.level.to_string(),
                key.zip_name(),
            )),
            headers,
        )
        .await
    }

    #[cfg(unix)]
    fn fake_impost(dir: &TempDir, body: &str) -> String {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.0.join("impost.sh");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path.display().to_string()
    }

    #[cfg(unix)]
    async fn bake_state(dir: &TempDir, bin: String, max_failures: u32) -> (Shared, Jobs) {
        // a realm that is ready: the bake waits for `/about` to answer 200
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server = format!("http://{}", listener.local_addr().unwrap());
        let realm = Router::new().route("/about", get(|| async { "{}" }));
        tokio::spawn(async move { axum::serve(listener, realm).await });
        let mut settings = Settings::for_tests(dir.0.join("root"));
        settings.bake = Some(bake::BakeConfig {
            // Through sh, not exec: exec of the just-written script fails with
            // ETXTBSY while a sibling test's forked child still holds its write fd.
            wrapper: "/bin/sh".into(),
            bin,
            server,
            content_server: String::new(),
            sdk6_adaption_layer: String::new(),
            timeout: Duration::from_secs(20),
            queue_depth: 1,
        });
        settings.bake_max_failures = max_failures;
        std::fs::create_dir_all(&settings.store_root).unwrap();
        let state = State::new(settings).await.unwrap();
        let jobs = Jobs::new();
        state.spawn_jobs(&jobs);
        (state, jobs)
    }

    #[cfg(unix)]
    async fn status_of(state: &Shared) -> serde_json::Value {
        let response = status(AxState(state.clone())).await;
        serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 1 << 20)
                .await
                .unwrap(),
        )
        .unwrap()
    }

    /// With bake-on-miss on, a missed tile is queued for the renderer, the
    /// zips it writes land in the store and the next request is a hit; a
    /// tile whose bakes keep failing is quarantined and not queued again.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_miss_is_baked_and_a_poison_tile_is_quarantined() {
        let dir = tempdir("imp-bake");
        let key = ImposterKey::new(0, 2, 4, 777).unwrap();
        let fixture = dir.0.join("2,4.777.zip");
        std::fs::write(&fixture, zips::test_zip_bytes(2, 4, 777)).unwrap();
        let body = format!(
            "out=\"\"\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"--zip-output\" ]; then out=\"$2\"; fi\n  shift\ndone\nmkdir -p \"$out/imposters/realms/enc/0\"\ncp '{}' \"$out/imposters/realms/enc/0/\"",
            fixture.display()
        );
        let (state, _jobs) = bake_state(&dir, fake_impost(&dir, &body), 3).await;
        assert_eq!(status_of(&state).await["bake_enabled"], true);
        assert_eq!(
            get_zip_with(&state, &key, HeaderMap::new()).await.status(),
            StatusCode::NOT_FOUND
        );
        let landed = state.store.path(&key.rel()).unwrap();
        assert!(
            crate::shared::test_wait::until(async || landed.exists()).await,
            "the baked zip landed in the store"
        );
        let fresh = State::new(state.settings.clone()).await.unwrap();
        let response = get_zip_with(&fresh, &key, HeaderMap::new()).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["x-bvi-source"], "store");

        let poison_dir = tempdir("imp-bake-poison");
        let (poisoned, _jobs) =
            bake_state(&poison_dir, fake_impost(&poison_dir, "exit 1"), 1).await;
        let tile_key = ImposterKey::new(0, 6, 8, 9).unwrap();
        assert_eq!(
            get_zip_with(&poisoned, &tile_key, HeaderMap::new())
                .await
                .status(),
            StatusCode::NOT_FOUND
        );
        assert!(
            crate::shared::test_wait::until(async || {
                poisoned.quarantine.is_quarantined(&tile_key.tile)
            })
            .await,
            "the failing tile was never quarantined"
        );
        let status = status_of(&poisoned).await;
        assert_eq!(status["quarantine"][0]["key"], "0/6,8", "{status}");
        get_zip_with(&poisoned, &tile_key, HeaderMap::new()).await;
        assert_eq!(
            poisoned.bake.as_ref().unwrap().snapshot().0,
            Vec::<key::TileKey>::new(),
            "a quarantined tile is not queued"
        );
    }

    /// An API replica runs no baker, yet its miss is baked: the marker it
    /// leaves beside the shared store reaches the process holding the lease.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_replica_miss_is_baked_by_the_leased_baker() {
        let dir = tempdir("imp-bake-replica");
        let key = ImposterKey::new(0, 4, 2, 555).unwrap();
        let fixture = dir.0.join("4,2.555.zip");
        std::fs::write(&fixture, zips::test_zip_bytes(4, 2, 555)).unwrap();
        let body = format!(
            "out=\"\"\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"--zip-output\" ]; then out=\"$2\"; fi\n  shift\ndone\nmkdir -p \"$out/imposters/realms/enc/0\"\ncp '{}' \"$out/imposters/realms/enc/0/\"",
            fixture.display()
        );
        let (baker, _jobs) = bake_state(&dir, fake_impost(&dir, &body), 3).await;
        let replica = State::new(baker.settings.clone()).await.unwrap();
        let replica_jobs = Jobs::new().with_fleet_jobs(false);
        replica.spawn_jobs(&replica_jobs);
        assert_eq!(status_of(&replica).await["bake_enabled"], true);
        assert_eq!(
            get_zip_with(&replica, &key, HeaderMap::new())
                .await
                .status(),
            StatusCode::NOT_FOUND
        );
        let landed = baker.store.path(&key.rel()).unwrap();
        assert!(
            crate::shared::test_wait::until(async || landed.exists()).await,
            "the replica's miss was baked"
        );
        let response = get_zip_with(&replica, &key, HeaderMap::new()).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn if_none_match_on_the_crc_etag_short_circuits() {
        let dir = tempdir("imp-etag");
        let state = state_with(&dir).await;
        let key = ImposterKey::new(0, 2, 100, 777).unwrap();

        let mut headers = HeaderMap::new();
        headers.insert(header::IF_NONE_MATCH, HeaderValue::from_static("\"777\""));
        let response = get_zip_with(&state, &key, headers).await;
        assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(response.headers()[header::ETAG], "\"777\"");

        let mut headers = HeaderMap::new();
        headers.insert(
            header::IF_NONE_MATCH,
            HeaderValue::from_static("W/\"1\", \"777\""),
        );
        assert_eq!(
            get_zip_with(&state, &key, headers).await.status(),
            StatusCode::NOT_MODIFIED
        );

        put_zip(&state, &key);
        let mut headers = HeaderMap::new();
        headers.insert(header::IF_NONE_MATCH, HeaderValue::from_static("\"778\""));
        let response = get_zip_with(&state, &key, headers).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::ETAG], "\"777\"");
        assert_eq!(response.headers()["x-bvi-source"], "store");
        assert_eq!(response.headers()[header::CONTENT_TYPE], "application/zip");
    }

    #[tokio::test]
    async fn spec_is_served_from_the_stored_zip() {
        let dir = tempdir("imp-spec");
        let state = state_with(&dir).await;
        let key = ImposterKey::new(0, 0, 100, 3504527830).unwrap();
        let path = |state: &Shared| {
            imposter(
                AxState(state.clone()),
                Path((
                    "content".to_string(),
                    "0".to_string(),
                    "0,100.3504527830-spec.json".to_string(),
                )),
                HeaderMap::new(),
            )
        };
        assert_eq!(
            path(&state).await.status(),
            StatusCode::NOT_FOUND,
            "a spec is never fetched on its own"
        );
        put_zip(&state, &key);
        let response = path(&state).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
        let body = axum::body::to_bytes(response.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["crc"], 3504527830u64);
    }

    #[tokio::test]
    async fn the_store_alone_answers() {
        let dir = tempdir("imp-store");
        let state = state_with(&dir).await;
        let key = ImposterKey::new(1, 2, -4, 9).unwrap();
        assert_eq!(
            get_zip_with(&state, &key, HeaderMap::new()).await.status(),
            StatusCode::NOT_FOUND
        );
        put_zip(&state, &key);
        let response = get_zip_with(&state, &key, HeaderMap::new()).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["x-bvi-source"], "store");
        let app = router(state.clone());
        use tower::ServiceExt;
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/status")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["bake_enabled"], false);
        assert!(v.get("readthrough_enabled").is_none());
    }

    #[test]
    fn settings_resolve_defaults_and_require_a_realm_to_bake() {
        let s = Settings::resolve(RawSettings::default()).unwrap();
        assert_eq!(s.store_root, PathBuf::from(DEFAULT_STORE_ROOT));
        assert_eq!(s.store_max_bytes, DEFAULT_STORE_MAX_BYTES);
        assert!(s.bake.is_none());
        let enabled = |server: Option<&str>| RawSettings {
            bake: RawBakeSettings {
                enabled: true,
                impost_server: server.map(str::to_string),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(
            Settings::resolve(enabled(None)).is_err(),
            "a bake with no realm to render is refused"
        );
        let s = Settings::resolve(enabled(Some("http://127.0.0.1:5210"))).unwrap();
        let bake = s.bake.unwrap();
        assert_eq!(bake.server, "http://127.0.0.1:5210");
        assert_eq!(bake.bin, "impost");
        assert_eq!(bake.queue_depth, 1);
    }
}
