//! api.decentraland.org: `/v{1,2}/map.png` (the navmap atlas chunks the
//! explorer loads), the minimaps, `/v{1,2}/tiles` and `/v2/tiles/info`,
//! rendered from the tile grid this node builds from its own database
//! ([`build`]: LAND and estates from the land-authz index, listings, names
//! and rentals from Shop's indexes), plus the JSON metadata and district
//! routes and the probes ([`meta`]), and `/satellite/{z}/{x}/{y}` painted
//! over this grid (`super::satellite`). The grid is ~92k tiles, 38 MB of
//! JSON: the leased `explorer-map-grid` job rebuilds it every minute
//! ([`run_pass`]) and stores it, gzipped, as the one row of
//! `explore_api.map_grid` only when its body changed, with a new stamp
//! (`lastUpdatedAt`). The row is loaded at startup, so a restart serves the
//! last grid before its first pass, and every process reloads it when
//! another one stored a newer grid ([`reload_if_newer`]). A pass whose sources are not all
//! published builds nothing and keeps the held grid; until a grid exists
//! the map routes answer the unavailable envelope naming the source the
//! last pass waited for.

pub mod build;
pub mod http_cache;
pub mod meta;
pub mod png;
pub mod render;
pub mod tiles;

use std::collections::HashMap;
use std::io::{Read as _, Write as _};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::shared::routing::get;
use anyhow::Context as _;
use axum::Router;
use axum::response::Response;
use chrono::NaiveDateTime;
use parking_lot::RwLock;
use serde::Deserialize;
use sha2::{Digest as _, Sha256};
use sqlx::PgPool;

use super::{Shared, mirror};
use crate::chain_index::source::{Source, unavailable};
use crate::jobs::JobContext;
pub use build::{LandMarket, LandMarketError, LandMarketSnapshot};
use tiles::{Tile, TileType};

/// The pass marker in `explore_api.mirror_passes` (`GET /mirrors`).
pub const MIRROR: &str = "map-grid";
/// The leased job that builds the grid.
pub const JOB: &str = "explorer-map-grid";
/// The job, in every process, that reloads a grid another process built.
pub const RELOAD_JOB: &str = "explorer-map-grid-reload";
/// Pause between two builds.
pub const BUILD_INTERVAL: Duration = Duration::from_secs(60);
const RESPONSE_CACHE_TTL: Duration = Duration::from_secs(120);
const PNG_CACHE_ENTRIES: usize = 256;
const TILES_CACHE_ENTRIES: usize = 512;
const PNG_CACHE_BYTES: usize = 128 << 20;
const TILES_CACHE_BYTES: usize = 256 << 20;
const FLIGHT_PRUNE_AT: usize = 64;

pub struct Grid {
    pub tiles: HashMap<String, Tile>,
    /// The grid's stamp (`/v2/tiles/info`'s `lastUpdatedAt`, ms): it moves
    /// only when the body changes.
    pub last_updated_at: i64,
    /// When the stored body was built (or, for a row the old mirror wrote,
    /// downloaded).
    pub fetched_at: NaiveDateTime,
    /// SHA-256 of [`build::canonical_body`]: a pass compares it to decide
    /// whether anything changed.
    pub digest: [u8; 32],
    /// Mixed into ETags and cache keys, so two grids with one stamp still differ.
    pub content_tag: u64,
    /// estate id -> coords of its owned parcels (estate map.png selection).
    pub estates_owned: HashMap<String, Vec<(i32, i32)>>,
    /// estate id -> coords of every parcel carrying it (the fallback selection).
    pub estates_all: HashMap<String, Vec<(i32, i32)>>,
    /// `(min_x, max_x, min_y, max_y)` over every tile; `None` for an empty grid.
    pub bounds: Option<(i32, i32, i32, i32)>,
}

#[derive(Deserialize)]
struct TilesDocument {
    #[serde(default)]
    ok: bool,
    data: HashMap<String, Tile>,
}

impl Grid {
    /// Parses a stored `{ok, data: {"x,y": tile}}` document.
    pub fn from_json(
        body: &[u8],
        last_updated_at: i64,
        fetched_at: NaiveDateTime,
    ) -> anyhow::Result<Self> {
        let doc: TilesDocument = serde_json::from_slice(body).context("tiles document")?;
        anyhow::ensure!(doc.ok, "tiles document has ok=false");
        anyhow::ensure!(!doc.data.is_empty(), "tiles document has no tiles");
        Ok(Self::from_tiles(doc.data, last_updated_at, fetched_at))
    }

    pub fn from_tiles(
        tiles: HashMap<String, Tile>,
        last_updated_at: i64,
        fetched_at: NaiveDateTime,
    ) -> Self {
        let digest = Sha256::digest(build::canonical_body(&tiles)).into();
        Self::from_parts(tiles, last_updated_at, fetched_at, digest)
    }

    fn from_parts(
        tiles: HashMap<String, Tile>,
        last_updated_at: i64,
        fetched_at: NaiveDateTime,
        digest: [u8; 32],
    ) -> Self {
        let mut estates_owned: HashMap<String, Vec<(i32, i32)>> = HashMap::new();
        let mut estates_all: HashMap<String, Vec<(i32, i32)>> = HashMap::new();
        let mut bounds: Option<(i32, i32, i32, i32)> = None;
        for tile in tiles.values() {
            bounds = Some(match bounds {
                None => (tile.x, tile.x, tile.y, tile.y),
                Some((x1, x2, y1, y2)) => (
                    x1.min(tile.x),
                    x2.max(tile.x),
                    y1.min(tile.y),
                    y2.max(tile.y),
                ),
            });
            if let Some(estate) = &tile.estate_id {
                estates_all
                    .entry(estate.clone())
                    .or_default()
                    .push((tile.x, tile.y));
                if tile.tile_type == TileType::Owned {
                    estates_owned
                        .entry(estate.clone())
                        .or_default()
                        .push((tile.x, tile.y));
                }
            }
        }
        let mut content_tag: u64 = 0xcbf2_9ce4_8422_2325;
        for b in last_updated_at
            .to_le_bytes()
            .iter()
            .chain(digest.iter())
            .copied()
        {
            content_tag ^= b as u64;
            content_tag = content_tag.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Self {
            tiles,
            last_updated_at,
            fetched_at,
            digest,
            content_tag,
            estates_owned,
            estates_all,
            bounds,
        }
    }
}

/// A TTL response cache bounded by entry count and total body bytes; the
/// oldest entries go first when either bound is reached.
type CachedBody = (Instant, Arc<Vec<u8>>);

struct ResponseCache {
    entries: parking_lot::Mutex<CachedBodies>,
    ttl: Duration,
    max_entries: usize,
    max_bytes: usize,
}

#[derive(Default)]
struct CachedBodies {
    map: HashMap<String, CachedBody>,
    bytes: usize,
}

impl CachedBodies {
    fn remove(&mut self, key: &str) -> Option<Arc<Vec<u8>>> {
        let (_, body) = self.map.remove(key)?;
        self.bytes -= body.len();
        Some(body)
    }
}

impl ResponseCache {
    fn new(ttl: Duration, max_entries: usize, max_bytes: usize) -> Self {
        Self {
            entries: parking_lot::Mutex::new(CachedBodies::default()),
            ttl,
            max_entries,
            max_bytes,
        }
    }

    fn get_fresh(&self, key: &str) -> Option<Arc<Vec<u8>>> {
        let entries = self.entries.lock();
        let (at, body) = entries.map.get(key)?;
        (at.elapsed() < self.ttl).then(|| body.clone())
    }

    fn insert(&self, key: String, body: Arc<Vec<u8>>) {
        if body.len() > self.max_bytes {
            return;
        }
        // Evicted bodies are freed after the lock is released.
        let mut evicted = Vec::new();
        {
            let mut entries = self.entries.lock();
            evicted.extend(entries.remove(&key));
            let full = |e: &CachedBodies| {
                e.map.len() >= self.max_entries || e.bytes + body.len() > self.max_bytes
            };
            if full(&entries) {
                let expired: Vec<String> = entries
                    .map
                    .iter()
                    .filter(|(_, (at, _))| at.elapsed() >= self.ttl)
                    .map(|(k, _)| k.clone())
                    .collect();
                for k in expired {
                    evicted.extend(entries.remove(&k));
                }
            }
            while !entries.map.is_empty() && full(&entries) {
                let oldest = entries
                    .map
                    .iter()
                    .min_by_key(|(_, (at, _))| *at)
                    .map(|(k, _)| k.clone())
                    .expect("a non-empty map has an oldest entry");
                evicted.extend(entries.remove(&oldest));
            }
            entries.bytes += body.len();
            entries.map.insert(key, (Instant::now(), body));
        }
        drop(evicted);
    }

    fn clear(&self) {
        let old = std::mem::take(&mut *self.entries.lock());
        drop(old);
    }

    #[cfg(test)]
    fn bytes(&self) -> usize {
        let entries = self.entries.lock();
        assert_eq!(
            entries.bytes,
            entries.map.values().map(|(_, b)| b.len()).sum::<usize>(),
            "the byte count follows the entries"
        );
        entries.bytes
    }
}

pub struct MapState {
    grid: RwLock<Option<Arc<Grid>>>,
    /// The source the last pass found unpublished, and why: what the map
    /// routes name while no grid is held.
    waiting: parking_lot::Mutex<Option<(Source, String)>>,
    png_cache: ResponseCache,
    tiles_cache: ResponseCache,
    tiles_flights: parking_lot::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    #[cfg(test)]
    tiles_builds: std::sync::atomic::AtomicUsize,
}

impl Default for MapState {
    fn default() -> Self {
        Self::new()
    }
}

impl MapState {
    pub fn new() -> Self {
        Self {
            grid: RwLock::new(None),
            waiting: parking_lot::Mutex::new(None),
            png_cache: ResponseCache::new(RESPONSE_CACHE_TTL, PNG_CACHE_ENTRIES, PNG_CACHE_BYTES),
            tiles_cache: ResponseCache::new(
                RESPONSE_CACHE_TTL,
                TILES_CACHE_ENTRIES,
                TILES_CACHE_BYTES,
            ),
            tiles_flights: Default::default(),
            #[cfg(test)]
            tiles_builds: Default::default(),
        }
    }

    pub fn snapshot(&self) -> Option<Arc<Grid>> {
        self.grid.read().clone()
    }

    pub fn is_ready(&self) -> bool {
        self.grid.read().is_some()
    }

    pub fn last_updated_at(&self) -> i64 {
        self.grid.read().as_ref().map_or(0, |g| g.last_updated_at)
    }

    /// The unavailable envelope a map route answers while no grid is held:
    /// the source the last pass waited for, else the land-authz index (the
    /// first source a build reads). `no-store`, so no cache in front keeps
    /// it past the first grid.
    pub fn unavailable(&self) -> Response {
        let waiting = self.waiting.lock().clone();
        let mut resp = match waiting {
            Some((source, why)) => unavailable(
                source,
                &format!("the tile grid has not been built yet: {why}"),
            ),
            None => unavailable(
                Source::LandAuthzIndex,
                "the tile grid has not been built yet: it is built from the land-authz index and Shop's \
                 marketplace indexes once they are published",
            ),
        };
        resp.headers_mut().insert(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("no-store"),
        );
        resp
    }

    /// The lock a tiles miss holds while building `key`'s body.
    pub fn tiles_flight(&self, key: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut flights = self.tiles_flights.lock();
        // Idle locks are dropped in batches, not on every call.
        if flights.len() >= FLIGHT_PRUNE_AT {
            flights.retain(|_, lock| Arc::strong_count(lock) > 1);
        }
        flights.entry(key.to_string()).or_default().clone()
    }

    #[cfg(test)]
    pub fn tiles_builds(&self) -> usize {
        self.tiles_builds.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub(super) fn count_tiles_build(&self) {
        #[cfg(test)]
        self.tiles_builds
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn content_tag(&self) -> u64 {
        self.grid.read().as_ref().map_or(0, |g| g.content_tag)
    }

    pub fn set(&self, grid: Grid) {
        *self.grid.write() = Some(Arc::new(grid));
        self.png_cache.clear();
        self.tiles_cache.clear();
    }

    pub fn cached_png(&self, key: &str) -> Option<Arc<Vec<u8>>> {
        self.png_cache.get_fresh(key)
    }

    pub fn store_png(&self, key: String, body: Arc<Vec<u8>>) {
        self.png_cache.insert(key, body);
    }

    pub fn cached_tiles(&self, key: &str) -> Option<Arc<Vec<u8>>> {
        self.tiles_cache.get_fresh(key)
    }

    pub fn store_tiles(&self, key: String, body: Arc<Vec<u8>>) {
        self.tiles_cache.insert(key, body);
    }

    /// The persisted grid, if a pass ever stored one.
    pub async fn load(&self, pool: &PgPool) -> anyhow::Result<()> {
        let row: Option<(i64, Vec<u8>, NaiveDateTime)> = sqlx::query_as(
            "SELECT last_updated_at, body_gzip, fetched_at FROM explore_api.map_grid WHERE id = 1",
        )
        .fetch_optional(pool)
        .await
        .context("reading explore_api.map_grid")?;
        let Some((last_updated_at, body_gzip, fetched_at)) = row else {
            tracing::info!(
                "no tile grid stored yet; map routes answer 503 until the first map-grid build"
            );
            return Ok(());
        };
        let grid = tokio::task::spawn_blocking(move || -> anyhow::Result<Grid> {
            let mut body = Vec::new();
            flate2::read::GzDecoder::new(body_gzip.as_slice())
                .read_to_end(&mut body)
                .context("inflating the stored grid")?;
            Grid::from_json(&body, last_updated_at, fetched_at)
        })
        .await
        .context("grid load task")??;
        tracing::info!(
            tiles = grid.tiles.len(),
            last_updated_at,
            "tile grid loaded from the database"
        );
        self.set(grid);
        Ok(())
    }
}

pub fn gzip(body: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    enc.write_all(body)?;
    Ok(enc.finish()?)
}

/// Stores the built grid as the one row; `etag` is NULL (a column of the
/// mirror this builder replaced).
pub async fn store(
    pool: &PgPool,
    last_updated_at: i64,
    tile_count: i64,
    body_gzip: &[u8],
    built_at: NaiveDateTime,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO explore_api.map_grid (id, last_updated_at, etag, tile_count, body_gzip, fetched_at)
         VALUES (1, $1, NULL, $2, $3, $4)
         ON CONFLICT (id) DO UPDATE
         SET last_updated_at = EXCLUDED.last_updated_at, etag = NULL,
             tile_count = EXCLUDED.tile_count, body_gzip = EXCLUDED.body_gzip,
             fetched_at = EXCLUDED.fetched_at",
    )
    .bind(last_updated_at)
    .bind(tile_count as i32)
    .bind(body_gzip)
    .bind(built_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Reloads the stored grid when its stamp is newer than the held one: the
/// build job runs in one process of the fleet and the routes in every one.
/// One primary-key read when nothing changed; `true` when it reloaded.
pub async fn reload_if_newer(state: &Shared) -> anyhow::Result<bool> {
    let stored: Option<i64> =
        sqlx::query_scalar("SELECT last_updated_at FROM explore_api.map_grid WHERE id = 1")
            .fetch_optional(&state.pool)
            .await
            .context("reading explore_api.map_grid's stamp")?;
    if stored.is_none_or(|s| s <= state.map.last_updated_at()) {
        return Ok(false);
    }
    state.map.load(&state.pool).await?;
    Ok(true)
}

/// The stamp a changed grid takes: now, and at least a second past the
/// held one, so `lastUpdatedAt` and `Last-Modified` always move forward.
fn next_stamp(held: Option<i64>, now_ms: i64) -> i64 {
    held.map_or(now_ms, |h| now_ms.max(h + 1000))
}

/// One pass ([`build`]): the land-authz snapshot and Shop's listings, the
/// grid composed, and the body stored and swapped in only when it differs
/// from the held one. A source that is not published fails the pass naming
/// it (`/mirrors`' `lastError`) and keeps the held grid; no partial grid is
/// built. The pass is complete either way once a grid is built or found
/// unchanged, with the grid's stamp as its cursor.
pub async fn run_pass(state: &Shared, ctx: &JobContext) -> anyhow::Result<()> {
    mirror::run(&state.pool, MIRROR, ctx, || async {
        let now = chrono::Utc::now();
        let (land, market) = match (state.land_authz.get(), state.land_market.get()) {
            (Some(land), Some(market)) => (land.clone(), market.clone()),
            (None, _) => {
                return waiting(
                    state,
                    Source::LandAuthzIndex,
                    "no land-authz index is attached to this process".into(),
                );
            }
            (Some(_), None) => {
                return waiting(
                    state,
                    Source::NftIndex,
                    "no Shop marketplace reader is attached to this process".into(),
                );
            }
        };
        let land_registry = land.settings.land_registry.to_ascii_lowercase();
        let estate_registry = land.settings.estate_registry.to_ascii_lowercase();
        let snapshot = match build::read_land(&land).await {
            Ok(s) => s,
            Err(LandMarketError::Unready(source, why)) => return waiting(state, source, why),
            Err(LandMarketError::Failed(e)) => return Err(e),
        };
        let listings = match market.snapshot(&land_registry, &estate_registry, now).await {
            Ok(m) => m,
            Err(LandMarketError::Unready(source, why)) => return waiting(state, source, why),
            Err(LandMarketError::Failed(e)) => {
                return Err(e.context("reading Shop's LAND listings"));
            }
        };
        anyhow::ensure!(!ctx.is_cancelled(), "cancelled before the grid build");
        let now_ms = now.timestamp_millis();
        let held = state.map.snapshot();
        let held_digest = held.as_ref().map(|g| g.digest);
        let built = tokio::task::spawn_blocking(move || -> anyhow::Result<Option<Built>> {
            let tiles = build::compose(
                &land_registry,
                &estate_registry,
                &snapshot,
                &listings,
                now_ms,
            );
            let body = build::canonical_body(&tiles);
            let digest: [u8; 32] = Sha256::digest(&body).into();
            if held_digest == Some(digest) {
                return Ok(None);
            }
            Ok(Some((tiles, digest, gzip(&body)?)))
        })
        .await
        .context("grid build task")??;
        *state.map.waiting.lock() = None;
        let Some((tiles, digest, gz)) = built else {
            let grid = held.context("an unchanged grid is a held one")?;
            return Ok(mirror::PassOutcome {
                items: grid.tiles.len() as i64,
                cursor: Some(grid.last_updated_at.to_string()),
            });
        };
        let stamp = next_stamp(held.as_ref().map(|g| g.last_updated_at), now_ms);
        let built_at = now.naive_utc();
        store(&state.pool, stamp, tiles.len() as i64, &gz, built_at)
            .await
            .context("storing explore_api.map_grid")?;
        tracing::info!(
            tiles = tiles.len(),
            gzip_bytes = gz.len(),
            last_updated_at = stamp,
            "tile grid built"
        );
        let grid = Grid::from_parts(tiles, stamp, built_at, digest);
        let out = mirror::PassOutcome {
            items: grid.tiles.len() as i64,
            cursor: Some(stamp.to_string()),
        };
        state.map.set(grid);
        Ok(out)
    })
    .await
}

/// A grid whose body differs from the held one: the tiles, the body's
/// digest and the gzipped body to store.
type Built = (HashMap<String, Tile>, [u8; 32], Vec<u8>);

/// Records what the pass waits for and fails it, naming the source.
fn waiting(state: &Shared, source: Source, why: String) -> anyhow::Result<mirror::PassOutcome> {
    *state.map.waiting.lock() = Some((source, why.clone()));
    anyhow::bail!("{} not ready: {why}", source.name())
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/v1/map.png", get(png::map_png))
        .route("/v2/map.png", get(png::map_png))
        .route("/v1/minimap.png", get(png::minimap_png))
        .route("/v1/estatemap.png", get(png::estate_minimap_png))
        .route("/v1/parcels/{x}/{y}/map.png", get(png::parcel_map_png))
        .route("/v2/parcels/{x}/{y}/map.png", get(png::parcel_map_png))
        .route("/v1/estates/{estate_id}/map.png", get(png::estate_map_png))
        .route("/v2/estates/{estate_id}/map.png", get(png::estate_map_png))
        .route("/v1/tiles", get(tiles::get_legacy_tiles))
        .route("/v2/tiles", get(tiles::get_tiles))
        .route("/v2/tiles/info", get(tiles::tiles_info))
        .route("/v2/parcels/{x}/{y}", get(meta::get_parcel))
        .route("/v2/estates/{id}", get(meta::get_estate))
        .route("/v2/contracts/{address}/tokens/{id}", get(meta::get_token))
        .route("/v2/districts", get(meta::get_districts))
        .route("/v2/districts/{id}", get(meta::get_district))
        .route(
            "/v2/addresses/{address}/contributions",
            get(meta::get_contributions),
        )
        .route("/ping", get(meta::ping))
        .route("/ready", get(meta::ready))
        .route("/v2/ping", get(meta::ping))
        .route("/v2/ready", get(meta::ready))
        .route("/satellite/{z}/{x}/{y}", get(super::satellite::tile))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_cache_stays_within_its_byte_budget() {
        let cache = ResponseCache::new(Duration::from_secs(60), 100, 100);
        for i in 0..10 {
            cache.insert(format!("k{i}"), Arc::new(vec![0; 40]));
        }
        assert!(cache.bytes() <= 100, "{}", cache.bytes());
        assert!(cache.get_fresh("k9").is_some(), "the newest entry is kept");
        assert!(
            cache.get_fresh("k0").is_none(),
            "the oldest entry is evicted"
        );
        cache.insert("huge".into(), Arc::new(vec![0; 101]));
        assert!(
            cache.get_fresh("huge").is_none(),
            "an over-budget body is not cached"
        );
        assert!(cache.get_fresh("k9").is_some());
    }

    #[test]
    fn response_cache_drops_every_expired_entry_once_full() {
        let cache = ResponseCache::new(Duration::from_secs(60), 3, 1 << 20);
        for k in ["a", "b", "c"] {
            cache.insert(k.into(), Arc::new(vec![1]));
        }
        let long_ago = Instant::now()
            .checked_sub(Duration::from_secs(120))
            .expect("a clock past two minutes");
        for k in ["a", "b"] {
            cache.entries.lock().map.get_mut(k).unwrap().0 = long_ago;
        }
        cache.insert("d".into(), Arc::new(vec![1]));
        let entries = cache.entries.lock();
        let mut keys: Vec<&str> = entries.map.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            ["c", "d"],
            "both expired entries go, not just the oldest one"
        );
        drop(entries);
        assert_eq!(cache.bytes(), 2);
        assert!(cache.get_fresh("c").is_some());
    }

    #[test]
    fn response_cache_stays_within_its_entry_cap() {
        let cache = ResponseCache::new(Duration::from_secs(60), 3, 1 << 20);
        for i in 0..10 {
            cache.insert(format!("k{i}"), Arc::new(vec![1]));
        }
        assert_eq!(cache.entries.lock().map.len(), 3);
        cache.insert("k9".into(), Arc::new(vec![2]));
        assert_eq!(
            cache.entries.lock().map.len(),
            3,
            "replacing a key needs no room"
        );
        assert_eq!(cache.get_fresh("k9").as_deref(), Some(&vec![2]));
    }

    #[test]
    fn concurrent_tile_misses_share_one_build_lock() {
        let map = MapState::new();
        let a = map.tiles_flight("k");
        assert!(Arc::ptr_eq(&a, &map.tiles_flight("k")));
        assert!(!Arc::ptr_eq(&a, &map.tiles_flight("other")));
        drop(a);
        for i in 0..1_000 {
            map.tiles_flight(&format!("idle-{i}"));
        }
        assert!(
            map.tiles_flights.lock().len() <= FLIGHT_PRUNE_AT + 1,
            "idle locks are dropped"
        );
    }

    #[test]
    fn grid_bounds_span_every_tile() {
        let tile = |x: i32, y: i32| -> Tile {
            serde_json::from_value(serde_json::json!({
                "id": format!("{x},{y}"), "x": x, "y": y, "type": "road",
                "top": false, "left": false, "topLeft": false
            }))
            .unwrap()
        };
        let tiles: HashMap<String, Tile> = [tile(-3, 2), tile(5, -7), tile(0, 9)]
            .into_iter()
            .map(|t| (t.id.clone(), t))
            .collect();
        let grid = Grid::from_tiles(tiles, 1, chrono::Utc::now().naive_utc());
        assert_eq!(grid.bounds, Some((-3, 5, -7, 9)));
    }
}
