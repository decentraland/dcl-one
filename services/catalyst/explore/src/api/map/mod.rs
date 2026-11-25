//! api.decentraland.org: `/v{1,2}/map.png` (the navmap atlas chunks the
//! explorer loads), the minimaps, `/v{1,2}/tiles` and `/v2/tiles/info`,
//! rendered from the tile grid this node builds from its own database
//! ([`build`]: LAND and estates from the land-authz index, listings, names
//! and rentals from Shop's indexes), plus the JSON metadata and district
//! routes and the probes ([`meta`]), and `/satellite/{z}/{x}/{y}` painted
//! over this grid (`super::satellite`). The grid is ~92k tiles, 38 MB of
//! JSON: the leased `explorer-map-grid` job rebuilds it when the marks of its
//! inputs move ([`Watcher`], read every 5 s; [`run_pass`]), reading every
//! input in one snapshot of the database, and
//! stores it, gzipped, as the one row of `explore_api.map_grid`, the grid's
//! revision, only when its body differs from the stored one, with a new
//! stamp (`lastUpdatedAt`). The row is loaded at startup, so a restart serves
//! the last grid before its first pass, and every process reloads it when
//! another one stored a newer grid ([`follow`]); a process only ever moves to
//! a newer revision. A pass whose sources are not all published builds
//! nothing and keeps the held grid; until a grid exists the map routes answer
//! the unavailable envelope naming the source the build waits for.

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
/// How often every process looks for a newer stored grid ([`follow`]).
pub const FOLLOW_INTERVAL: Duration = Duration::from_secs(10);
/// How often the build job reads the marks of its inputs ([`Watcher`]).
pub const WATCH_INTERVAL: Duration = Duration::from_secs(5);
/// A build even when the marks did not move, for any change they miss.
pub const SAFETY_INTERVAL: Duration = Duration::from_secs(600);
/// The wait after a failed pass, doubling up to [`RETRY_MAX`].
pub const RETRY: Duration = Duration::from_secs(60);
pub const RETRY_MAX: Duration = Duration::from_secs(300);
/// The least time between two records of the grid checked current.
pub const CHECK_RECORD_INTERVAL: Duration = Duration::from_secs(60);
/// The least age of a stored revision before a changed grid replaces it
/// (`Settings::map_min_publish_interval`): each revision is a ~4.4 MB row
/// (~4.8 MB of WAL) every process reloads in ~80 ms, so a token relisted
/// many times a minute does not cost that each time.
pub const MIN_PUBLISH_INTERVAL: Duration = Duration::from_secs(60);
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
    /// When the stored body was built.
    pub fetched_at: NaiveDateTime,
    /// SHA-256 of [`build::canonical_body`], the body's identity: a stored
    /// revision carries it, in hex, as its `etag`.
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
    /// Parses a stored `{ok, data: {"x,y": tile}}` document. Its digest is
    /// the row's `etag` when that is one (a build stored it), else the
    /// SHA-256 of `body` as stored, never a re-serialization of the grid.
    pub fn from_json(
        body: &[u8],
        last_updated_at: i64,
        fetched_at: NaiveDateTime,
        etag: Option<&str>,
    ) -> anyhow::Result<Self> {
        let doc: TilesDocument = serde_json::from_slice(body).context("tiles document")?;
        anyhow::ensure!(doc.ok, "tiles document has ok=false");
        anyhow::ensure!(!doc.data.is_empty(), "tiles document has no tiles");
        let digest = etag
            .and_then(|e| <[u8; 32]>::try_from(hex::decode(e).ok()?).ok())
            .unwrap_or_else(|| Sha256::digest(body).into());
        Ok(Self::from_parts(
            doc.data,
            last_updated_at,
            fetched_at,
            digest,
        ))
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
    /// When a build last found the served revision current against its
    /// published sources (ms; 0 before any), as `explore_api.map_grid`'s
    /// `checked_at` records it: `/v2/tiles/info`'s `checkedAt`, what the
    /// map's age label reads.
    checked_at: std::sync::atomic::AtomicI64,
    /// When a changed grid held back by the publication floor may publish.
    held_until: parking_lot::Mutex<Option<Instant>>,
    /// The revision the last pass left current (published, or found equal
    /// to its build), for the [`Watcher`]'s checks; `None` after a pass that
    /// held a change back or was overtaken.
    confirmed: parking_lot::Mutex<Option<Current>>,
    /// The marks of the inputs the last pass built from, read in its own
    /// snapshot at its clock; `None` when it could not read them. What the
    /// [`Watcher`] compares later marks with.
    pass_marks: parking_lot::Mutex<Option<String>>,
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
            checked_at: Default::default(),
            held_until: parking_lot::Mutex::new(None),
            confirmed: parking_lot::Mutex::new(None),
            pass_marks: parking_lot::Mutex::new(None),
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

    /// When the build last found the grid current (ms), 0 before any.
    pub fn checked_at(&self) -> i64 {
        self.checked_at.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn note_checked(&self, at_ms: i64) {
        self.checked_at
            .fetch_max(at_ms, std::sync::atomic::Ordering::Relaxed);
    }

    /// The unavailable envelope a map route answers while no grid is held:
    /// the source the last build waited for (this process's, or the one the
    /// build recorded in `explore_api.mirror_passes`, [`follow`]); with none
    /// known, the grid itself, without naming a source it cannot vouch for.
    /// `no-store`, so no cache in front keeps it past the first grid.
    pub fn unavailable(&self) -> Response {
        let waiting = self.waiting.lock().clone();
        let mut resp = match waiting {
            Some((source, why)) => unavailable(
                source,
                &format!("the tile grid has not been built yet: {why}"),
            ),
            None => super::unavailable(
                MIRROR,
                None,
                "the tile grid has not been built yet: it is built from the land-authz index and Shop's \
                 marketplace indexes once they are published; GET /mirrors (map-grid) shows what its \
                 build waits for",
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

    /// Serves `grid`, whatever is held.
    #[cfg(test)]
    pub fn set(&self, grid: Grid) {
        *self.grid.write() = Some(Arc::new(grid));
        self.png_cache.clear();
        self.tiles_cache.clear();
    }

    /// Serves `grid` only if its stamp is newer than the held one's, decided
    /// under the grid lock, so a slow reload of an older revision never
    /// replaces a newer one installed meanwhile. `true` when installed.
    pub fn install_newer(&self, grid: Grid) -> bool {
        let mut held = self.grid.write();
        if held
            .as_ref()
            .is_some_and(|h| h.last_updated_at >= grid.last_updated_at)
        {
            return false;
        }
        *held = Some(Arc::new(grid));
        drop(held);
        self.png_cache.clear();
        self.tiles_cache.clear();
        true
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

    /// The persisted grid, if a pass ever stored one, unless a newer one is held.
    pub async fn load(&self, pool: &PgPool) -> anyhow::Result<()> {
        let Some(grid) = fetch_stored(pool).await? else {
            tracing::info!(
                "no tile grid stored yet; map routes answer 503 until the first map-grid build"
            );
            return Ok(());
        };
        let (tiles, stamp) = (grid.tiles.len(), grid.last_updated_at);
        if self.install_newer(grid) {
            tracing::info!(
                tiles,
                last_updated_at = stamp,
                "tile grid loaded from the database"
            );
        }
        Ok(())
    }
}

/// The stored grid, inflated and parsed; `None` before the first build.
pub async fn fetch_stored(pool: &PgPool) -> anyhow::Result<Option<Grid>> {
    let row: Option<(i64, Option<String>, Vec<u8>, NaiveDateTime)> = sqlx::query_as(
        "SELECT last_updated_at, etag, body_gzip, fetched_at FROM explore_api.map_grid WHERE id = 1",
    )
    .fetch_optional(pool)
    .await
    .context("reading explore_api.map_grid")?;
    let Some((last_updated_at, etag, body_gzip, fetched_at)) = row else {
        return Ok(None);
    };
    let grid = tokio::task::spawn_blocking(move || -> anyhow::Result<Grid> {
        let mut body = Vec::new();
        flate2::read::GzDecoder::new(body_gzip.as_slice())
            .read_to_end(&mut body)
            .context("inflating the stored grid")?;
        Grid::from_json(&body, last_updated_at, fetched_at, etag.as_deref())
    })
    .await
    .context("grid load task")??;
    Ok(Some(grid))
}

pub fn gzip(body: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    enc.write_all(body)?;
    Ok(enc.finish()?)
}

/// A stored revision of the grid.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub(super) struct Revision {
    /// `last_updated_at`: the stamp, which only grows.
    pub stamp: i64,
    /// The hex SHA-256 of the body a build stored ([`build::canonical_body`]).
    pub etag: Option<String>,
    /// `fetched_at`: when it was built, on the database clock; its age is
    /// what the publication floor measures (a stamp can run ahead of it).
    pub built_at: NaiveDateTime,
}

/// A stored revision's identity: its stamp and etag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Current {
    pub stamp: i64,
    pub etag: Option<String>,
}

/// The stored revision, read on `executor`: the pool, or a build's input
/// snapshot.
pub(super) async fn stored_revision<'e, E>(executor: E) -> anyhow::Result<Option<Revision>>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query_as(
        "SELECT last_updated_at AS stamp, etag, fetched_at AS built_at FROM explore_api.map_grid WHERE id = 1",
    )
        .fetch_optional(executor)
        .await
        .context("reading explore_api.map_grid's revision")
}

/// Stores a built grid as the one row on `executor`, guarded on the revision
/// the build compared against (`prev`, `None` for no row): `false`, writing
/// nothing, when another publisher stored a revision meanwhile. A stored
/// grid was found current when built (`checked_at`).
pub(super) async fn store_guarded<'e, E>(
    executor: E,
    prev: Option<&Revision>,
    stamp: i64,
    digest_hex: &str,
    tile_count: i64,
    body_gzip: &[u8],
    built_at: NaiveDateTime,
) -> sqlx::Result<bool>
where
    E: sqlx::PgExecutor<'e>,
{
    let written = match prev {
        None => {
            sqlx::query(
                "INSERT INTO explore_api.map_grid (id, last_updated_at, etag, tile_count, body_gzip, fetched_at, checked_at)
                 VALUES (1, $1, $2, $3, $4, $5, $5) ON CONFLICT (id) DO NOTHING",
            )
            .bind(stamp)
            .bind(digest_hex)
            .bind(tile_count as i32)
            .bind(body_gzip)
            .bind(built_at)
            .execute(executor)
            .await?
        }
        Some(prev) => {
            sqlx::query(
                "UPDATE explore_api.map_grid
                 SET last_updated_at = $1, etag = $2, tile_count = $3, body_gzip = $4, fetched_at = $5, checked_at = $5
                 WHERE id = 1 AND last_updated_at = $6 AND etag IS NOT DISTINCT FROM $7",
            )
            .bind(stamp)
            .bind(digest_hex)
            .bind(tile_count as i32)
            .bind(body_gzip)
            .bind(built_at)
            .bind(prev.stamp)
            .bind(prev.etag.as_deref())
            .execute(executor)
            .await?
        }
    };
    Ok(written.rows_affected() == 1)
}

/// Records on `executor` that the stored revision `rev` was found current at
/// `at` (`checked_at`, which never moves back): `false`, writing nothing,
/// when the stored revision is another, so a check never vouches for a
/// revision it did not compare.
pub(super) async fn confirm_checked<'e, E>(
    executor: E,
    rev: &Current,
    at: NaiveDateTime,
) -> sqlx::Result<bool>
where
    E: sqlx::PgExecutor<'e>,
{
    let written = sqlx::query(
        "UPDATE explore_api.map_grid SET checked_at = GREATEST(checked_at, $3)
         WHERE id = 1 AND last_updated_at = $1 AND etag IS NOT DISTINCT FROM $2",
    )
    .bind(rev.stamp)
    .bind(rev.etag.as_deref())
    .bind(at)
    .execute(executor)
    .await?;
    Ok(written.rows_affected() == 1)
}

/// [`confirm_checked`] for the build job: on its lease's session when it has
/// one (the write commits while it holds the lease, or not at all), and the
/// time noted for `/v2/tiles/info` when this process serves that revision.
async fn confirm_current(
    state: &Shared,
    ctx: &JobContext,
    rev: &Current,
    at: NaiveDateTime,
) -> anyhow::Result<bool> {
    let confirmed = match ctx.lease() {
        Some(lease) => {
            let owned = rev.clone();
            lease
                .with_connection(move |conn| {
                    Box::pin(async move { confirm_checked(conn, &owned, at).await })
                })
                .await
                .context("recording the map grid checked on the lease session")?
        }
        None => confirm_checked(&state.pool, rev, at)
            .await
            .context("recording the map grid checked")?,
    };
    if confirmed && state.map.last_updated_at() == rev.stamp {
        state.map.note_checked(at.and_utc().timestamp_millis());
    }
    Ok(confirmed)
}

/// Reloads the stored grid when its stamp is newer than the held one: the
/// build job runs in one process of the fleet and the routes in every one.
/// One primary-key read when nothing changed; `true` when it installed a
/// newer grid ([`MapState::install_newer`]).
pub async fn reload_if_newer(state: &Shared) -> anyhow::Result<bool> {
    let stored = stored_revision(&state.pool).await?;
    reload_past(state, stored.map(|r| r.stamp)).await
}

/// Installs the stored grid when its stamp (`stored`, `None` for no row) is
/// newer than the held one.
async fn reload_past(state: &Shared, stored: Option<i64>) -> anyhow::Result<bool> {
    if stored.is_none_or(|stamp| stamp <= state.map.last_updated_at()) {
        return Ok(false);
    }
    Ok(fetch_stored(&state.pool)
        .await?
        .is_some_and(|grid| state.map.install_newer(grid)))
}

/// The `explorer-map-grid-reload` job's pass, in every process: one
/// primary-key read of the stored revision's stamp and `checked_at`, a
/// reload when it is newer than the held grid, and the check time when it is
/// the served one; then what the last build waits for, as
/// `explore_api.mirror_passes` records it, so a process that does not hold
/// the build's lease names the same source while no grid exists.
pub async fn follow(state: &Shared) -> anyhow::Result<()> {
    // Each half runs whatever the other did; the first failure is reported.
    let stored = sqlx::query_as::<_, (i64, Option<NaiveDateTime>)>(
        "SELECT last_updated_at, checked_at FROM explore_api.map_grid WHERE id = 1",
    )
    .fetch_optional(&state.pool)
    .await
    .context("reading explore_api.map_grid's revision");
    let reloaded = match &stored {
        Ok(row) => reload_past(state, row.map(|(stamp, _)| stamp)).await,
        Err(_) => Ok(false),
    };
    // A check of the served revision, never of one this process has not loaded.
    if let Ok(Some((stamp, Some(at)))) = &stored
        && *stamp == state.map.last_updated_at()
    {
        state.map.note_checked(at.and_utc().timestamp_millis());
    }
    let marker = sqlx::query_scalar::<_, Option<String>>(
        "SELECT last_error FROM explore_api.mirror_passes WHERE mirror = $1",
    )
    .bind(MIRROR)
    .fetch_optional(&state.pool)
    .await
    .context("reading the map-grid pass marker");
    if let Ok(last_error) = &marker {
        *state.map.waiting.lock() = last_error
            .clone()
            .flatten()
            .as_deref()
            .and_then(recorded_wait);
    }
    stored?;
    reloaded?;
    marker.map(|_| ())
}

/// [`reload_if_newer`] for a pass that has nothing of its own to store: a
/// failure is logged, never the pass's.
async fn reload_quietly(state: &Shared) {
    if let Err(e) = reload_if_newer(state).await {
        tracing::warn!(error = %e, "reloading the stored tile grid failed; the next reload retries");
    }
}

/// The outcome of a pass that serves a revision it did not build: the tile
/// count and stamp of the one grid this process serves.
fn served(state: &Shared) -> mirror::PassOutcome {
    match state.map.snapshot() {
        Some(g) => mirror::PassOutcome {
            items: g.tiles.len() as i64,
            cursor: Some(g.last_updated_at.to_string()),
        },
        None => mirror::PassOutcome {
            items: 0,
            cursor: None,
        },
    }
}

/// The source and reason a failed build recorded (`"<source> not ready:
/// <why>"`, [`waiting`]); `None` for any other failure.
fn recorded_wait(last_error: &str) -> Option<(Source, String)> {
    let (name, why) = last_error.split_once(" not ready: ")?;
    [
        Source::LandAuthzIndex,
        Source::NftIndex,
        Source::TradesMirror,
    ]
    .into_iter()
    .find(|s| s.name() == name)
    .map(|s| (s, why.to_string()))
}

/// The stamp a changed grid takes: now, and at least a second past the
/// stored one, so `lastUpdatedAt` and `Last-Modified` always move forward.
fn next_stamp(stored: Option<i64>, now_ms: i64) -> i64 {
    stored.map_or(now_ms, |h| now_ms.max(h.saturating_add(1000)))
}

/// One pass ([`build`]). Every input is read in one REPEATABLE READ, READ
/// ONLY snapshot that the land-authz index opens once it is published, Shop's
/// readiness and rows included, so no rewind committed meanwhile mixes into
/// it; the stored revision (or its absence) is read in that snapshot too,
/// as the one these inputs follow. The grid is composed and its body
/// compared with that revision, not this process's grid: an equal body
/// writes nothing (and a process holding an older grid reloads the stored
/// one); a different one waits until that revision is
/// [`MIN_PUBLISH_INTERVAL`] old, then is stored with a write guarded on it
/// and served only if newer than what this process holds; a leased build
/// writes on its lease's session. The build's clock is the snapshot's
/// (`now()`), one PostgreSQL transaction clock for all its comparisons. A build whose
/// revision another publisher replaced after its snapshot began is
/// discarded: a pass never overwrites a revision published since its
/// snapshot.
/// A source that is not published fails the pass naming it (`/mirrors`'
/// `lastError`) and keeps the held grid; no partial grid is built. The pass
/// also reads its inputs' marks ([`read_marks`]) in the snapshot, for the
/// [`Watcher`].
pub async fn run_pass(state: &Shared, ctx: &JobContext) -> anyhow::Result<()> {
    mirror::run(&state.pool, MIRROR, ctx, || async {
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
        let chain_id = build::land_chain_id(&land.settings.network)
            .with_context(|| format!("no chain is known for LAND network {}", land.settings.network))?;
        let land_registry = land.settings.land_registry.to_ascii_lowercase();
        let estate_registry = land.settings.estate_registry.to_ascii_lowercase();
        // The published snapshot, or why not: a database error is the
        // pass's own failure, never read as the index being unpublished.
        let snapshot = crate::land_authz::indexer::ready_snapshot(
            &land.pool,
            &land.settings.network,
            &land.settings.land_registry,
            &land.settings.estate_registry,
        )
        .await
        .context("opening the land-authz snapshot")?;
        let Some(mut snapshot) = snapshot else {
            return waiting(
                state,
                Source::LandAuthzIndex,
                "the land-authz index has not published a recent fold of the configured LAND and Estate registries"
                    .into(),
            );
        };
        // The build's clock is the snapshot's: every comparison the build
        // makes (readiness windows, expiry, the stamp, the publication floor)
        // uses this one PostgreSQL transaction clock.
        let now: chrono::DateTime<chrono::Utc> = sqlx::query_scalar("SELECT now()")
            .fetch_one(&mut *snapshot.tx)
            .await
            .context("reading the snapshot's clock")?;
        let expected = stored_revision(&mut *snapshot.tx).await?;
        let land_rows = build::read_land(&mut snapshot.tx, &land_registry, &estate_registry).await?;
        let listings = match market
            .snapshot(&mut snapshot.tx, chain_id, &land_registry, &estate_registry, now)
            .await
        {
            Ok(m) => m,
            Err(LandMarketError::Unready(source, why)) => return waiting(state, source, why),
            Err(LandMarketError::Failed(e)) => {
                return Err(e.context("reading Shop's LAND listings"));
            }
        };
        // The marks of these very inputs, in this snapshot at its clock: the
        // build job compares later marks with them, so a change that lands
        // after the snapshot moves them even when a later one reverts it. A
        // pass that cannot read them leaves none and builds anyway.
        *state.map.pass_marks.lock() =
            match read_marks(&mut snapshot.tx, &land, market.as_ref(), true, now).await {
                Ok(read) => Some(read.marks),
                Err(e) => {
                    tracing::warn!(error = %format!("{e:#}"), "reading the map grid's marks in its snapshot failed");
                    None
                }
            };
        // A read-only snapshot: nothing to commit.
        drop(snapshot);
        anyhow::ensure!(!ctx.is_cancelled(), "cancelled before the grid build");
        *state.map.waiting.lock() = None;
        let now_ms = now.timestamp_millis();
        let (tiles, body, digest) = tokio::task::spawn_blocking(move || {
            let tiles = build::compose(&land_registry, &estate_registry, &land_rows, &listings, now_ms);
            let body = build::canonical_body(&tiles);
            let digest: [u8; 32] = Sha256::digest(&body).into();
            (tiles, body, digest)
        })
        .await
        .context("grid build task")?;
        publish(state, ctx, expected, tiles, body, digest, now).await
    })
    .await
}

/// Stores and serves a built grid against `expected`, the revision its input
/// snapshot saw ([`run_pass`]).
async fn publish(
    state: &Shared,
    ctx: &JobContext,
    expected: Option<Revision>,
    tiles: HashMap<String, Tile>,
    body: Vec<u8>,
    digest: [u8; 32],
    now: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<mirror::PassOutcome> {
    let digest_hex = hex::encode(digest);
    if let Some(Revision {
        etag: Some(etag), ..
    }) = &expected
        && *etag == digest_hex
    {
        // The revision these inputs follow is this body: nothing to write
        // but that it was found current; serve the newest stored one.
        reload_quietly(state).await;
        let current = Current {
            stamp: expected.as_ref().map_or(0, |r| r.stamp),
            etag: Some(digest_hex),
        };
        if confirm_current(state, ctx, &current, now.naive_utc()).await? {
            *state.map.confirmed.lock() = Some(current);
        }
        return Ok(served(state));
    }
    let floor = chrono::Duration::from_std(state.settings.map_min_publish_interval)
        .unwrap_or(chrono::Duration::MAX);
    if let Some(prev) = &expected
        && now.naive_utc()
            < prev
                .built_at
                .checked_add_signed(floor)
                .unwrap_or(NaiveDateTime::MAX)
    {
        // A revision younger than the floor stays; the build job runs again
        // when it is old enough ([`Watcher`]).
        let wait = (prev.built_at + floor - now.naive_utc())
            .to_std()
            .unwrap_or_default();
        *state.map.held_until.lock() = Some(Instant::now() + wait);
        ctx.progress(format!(
            "{MIRROR}: changed grid held back for {} s, until the stored revision is {} s old",
            wait.as_secs(),
            state.settings.map_min_publish_interval.as_secs()
        ));
        reload_quietly(state).await;
        return Ok(served(state));
    }
    let stamp = next_stamp(expected.as_ref().map(|r| r.stamp), now.timestamp_millis());
    let gz = tokio::task::spawn_blocking(move || gzip(&body))
        .await
        .context("grid gzip task")??;
    anyhow::ensure!(!ctx.is_cancelled(), "cancelled before storing the grid");
    let built_at = now.naive_utc();
    let tile_count = tiles.len() as i64;
    let stored_it = match ctx.lease() {
        // On the lease's own session: the write commits while this process
        // holds the lease, or not at all; a lease lost while the statement
        // waits takes the statement with it.
        Some(lease) => {
            let (prev, digest_hex) = (expected.clone(), digest_hex.clone());
            lease
                .with_connection(move |conn| {
                    Box::pin(async move {
                        store_guarded(
                            conn,
                            prev.as_ref(),
                            stamp,
                            &digest_hex,
                            tile_count,
                            &gz,
                            built_at,
                        )
                        .await
                    })
                })
                .await
                .context("storing explore_api.map_grid on the lease session")?
        }
        // An unleased run (tests, a single-process node) has no lease to lose.
        None => store_guarded(
            &state.pool,
            expected.as_ref(),
            stamp,
            &digest_hex,
            tile_count,
            &gz,
            built_at,
        )
        .await
        .context("storing explore_api.map_grid")?,
    };
    if !stored_it {
        // Another publisher stored a revision after this build's snapshot
        // began: its grid is served and this build discarded; the next pass
        // reads its inputs after it.
        tracing::warn!("another process stored a tile grid during this build; serving it instead");
        reload_quietly(state).await;
        return Ok(served(state));
    }
    tracing::info!(
        tiles = tiles.len(),
        last_updated_at = stamp,
        "tile grid built"
    );
    let out = mirror::PassOutcome {
        items: tiles.len() as i64,
        cursor: Some(stamp.to_string()),
    };
    state
        .map
        .install_newer(Grid::from_parts(tiles, stamp, built_at, digest));
    if state.map.last_updated_at() == stamp {
        state
            .map
            .note_checked(built_at.and_utc().timestamp_millis());
    }
    *state.map.confirmed.lock() = Some(Current {
        stamp,
        etag: Some(digest_hex),
    });
    Ok(out)
}

/// Records what the pass waits for and fails it, naming the source.
fn waiting(state: &Shared, source: Source, why: String) -> anyhow::Result<mirror::PassOutcome> {
    *state.map.waiting.lock() = Some((source, why.clone()));
    anyhow::bail!("{} not ready: {why}", source.name())
}

/// What [`watch_marks`] read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watched {
    /// Every source a build needs is ready.
    pub ready: bool,
    /// Moves whenever an input of a build changes; compared for equality.
    pub marks: String,
    /// The database clock the marks were read at.
    pub at: chrono::DateTime<chrono::Utc>,
}

/// The marks of every input of a build, read on a pooled connection at its
/// clock ([`read_marks`]) after the land-authz index's readiness. `None`
/// while the land-authz index or Shop's reader is not attached.
pub async fn watch_marks(state: &Shared) -> anyhow::Result<Option<Watched>> {
    let (Some(land), Some(market)) = (state.land_authz.get(), state.land_market.get()) else {
        return Ok(None);
    };
    let land_ready = crate::land_authz::indexer::ready_cursor(
        &land.pool,
        &land.settings.network,
        &land.settings.land_registry,
        &land.settings.estate_registry,
    )
    .await
    .is_some();
    let mut conn = state
        .pool
        .acquire()
        .await
        .context("a connection for the marks")?;
    let now: chrono::DateTime<chrono::Utc> = sqlx::query_scalar("SELECT now()")
        .fetch_one(&mut *conn)
        .await
        .context("reading the database clock")?;
    read_marks(&mut conn, land, market.as_ref(), land_ready, now)
        .await
        .map(Some)
}

/// The marks of every input of a build, read on `conn` at `now`: whether the
/// land-authz index is published (`land_ready`), its rewrite count and newest
/// folded LAND/Estate events ([`build::LAND_MARKS_SQL`]), and Shop's side
/// ([`LandMarket::marks`]). [`watch_marks`] reads them on a pooled
/// connection; [`run_pass`] reads them in its snapshot, at its clock, so
/// both compare equal for equal inputs.
async fn read_marks(
    conn: &mut sqlx::PgConnection,
    land: &crate::land_authz::LandAuthzState,
    market: &dyn LandMarket,
    land_ready: bool,
    now: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<Watched> {
    let chain_id = build::land_chain_id(&land.settings.network).with_context(|| {
        format!(
            "no chain is known for LAND network {}",
            land.settings.network
        )
    })?;
    let land_registry = land.settings.land_registry.to_ascii_lowercase();
    let estate_registry = land.settings.estate_registry.to_ascii_lowercase();
    let land_marks: Option<String> = sqlx::query_scalar(build::LAND_MARKS_SQL)
        .bind(&land.settings.network)
        .fetch_optional(&mut *conn)
        .await
        .context("reading the land-authz marks")?;
    let shop = market
        .marks(conn, chain_id, &land_registry, &estate_registry, now)
        .await
        .map_err(|e| match e {
            LandMarketError::Unready(source, why) => {
                anyhow::anyhow!("{} not ready: {why}", source.name())
            }
            LandMarketError::Failed(e) => e.context("reading Shop's marks"),
        })?;
    Ok(Watched {
        ready: land_ready && shop.ready,
        marks: format!(
            "{land_ready}|{}|{}",
            land_marks.as_deref().unwrap_or("-"),
            shop.marks
        ),
        at: now,
    })
}

/// What a [`Watcher::tick`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tick {
    /// A pass ran and completed (built, unchanged, or held back by the floor).
    Built,
    /// A pass ran and failed; the next is due after the retry wait.
    Failed,
    /// Nothing moved and every source is ready: the revision the last pass
    /// left current was recorded checked current again (`checked_at`).
    Checked,
    /// Nothing to do this tick.
    Idle,
}

/// The build job's decisions, one [`Self::tick`] every [`WATCH_INTERVAL`]:
/// a pass ([`run_pass`]) when it is due (a held-back publication, a retry,
/// a pass that another publisher overtook, or [`SAFETY_INTERVAL`] without
/// one), or when the marks moved since the ones the last pass read in its
/// own snapshot, at its clock ([`run_pass`]). So a change that lands after
/// that snapshot moves them even when a later change reverts it (a rental
/// listed, then closed). A pass that could not read them leaves none, so the
/// next readable marks run a pass before any check; a second such pass in a
/// row backs off as a failed one does. Moved marks wait while a change is
/// held back by the floor (the pass at its end reads every input) and while
/// a failed pass waits for its retry, unless every source turned ready
/// meanwhile; marks that cannot be read move nothing. So a failing read or
/// pass backs off from [`RETRY`] to [`RETRY_MAX`] instead of building every
/// tick. Otherwise, while every source is ready, the marks equal the last
/// pass's and that pass left a revision current, it records that the
/// revision still is (`checked_at`, at most every
/// [`CHECK_RECORD_INTERVAL`]), guarded on that revision.
#[derive(Debug)]
pub struct Watcher {
    /// The marks the last completed pass read in its snapshot; `None` when
    /// it could not read them.
    seen: Option<String>,
    /// Every source was ready when the marks before the last pass were read.
    ready_seen: bool,
    /// When a pass is due whatever the marks say; `None`: right away.
    due: Option<Instant>,
    retry: Duration,
    /// The last pass failed, or the last two could not read their marks:
    /// moved marks wait for its retry.
    failing: bool,
    /// The last completed pass could not read its marks.
    unmarked: bool,
    /// When the change the last pass held back may publish.
    held_until: Option<Instant>,
    /// The revision the last pass left current, built from inputs whose
    /// marks are `seen`; `None` after a failed, held-back or overtaken pass.
    current: Option<Current>,
    last_checked: Option<Instant>,
}

impl Default for Watcher {
    fn default() -> Self {
        Self {
            seen: None,
            ready_seen: false,
            due: None,
            retry: RETRY,
            failing: false,
            unmarked: false,
            held_until: None,
            current: None,
            last_checked: None,
        }
    }
}

impl Watcher {
    /// One decision at `now`.
    pub async fn tick(&mut self, state: &Shared, ctx: &JobContext, now: Instant) -> Tick {
        let watched = match watch_marks(state).await {
            Ok(w) => w,
            Err(e) => {
                tracing::warn!(error = %format!("{e:#}"), "reading the map grid's input marks failed");
                None
            }
        };
        let marks = watched.as_ref().map(|w| w.marks.clone());
        let ready = watched.as_ref().is_some_and(|w| w.ready);
        // Unreadable marks move nothing: `due` decides.
        let moved = marks.is_some() && marks != self.seen;
        let due = self.due.is_none_or(|d| now >= d);
        let holding = self.held_until.is_some_and(|h| now < h);
        let recovered = self.failing && ready && !self.ready_seen;
        if due || recovered || (moved && !holding && !self.failing) {
            self.ready_seen = ready;
            *state.map.held_until.lock() = None;
            *state.map.confirmed.lock() = None;
            *state.map.pass_marks.lock() = None;
            return match run_pass(state, ctx).await {
                Ok(()) => {
                    self.held_until = state.map.held_until.lock().take();
                    self.current = state.map.confirmed.lock().take();
                    self.due = match (self.held_until, &self.current) {
                        (Some(held), _) => Some(held),
                        (None, Some(_)) => Some(now + SAFETY_INTERVAL),
                        // Another publisher stored a revision during the
                        // pass: compare the inputs with it now.
                        (None, None) => None,
                    };
                    if self.current.is_some() {
                        self.last_checked = Some(now);
                    }
                    // Compared from now on with the marks of the snapshot this
                    // pass built from, not the ones read before it.
                    self.seen = state.map.pass_marks.lock().take();
                    if self.unmarked && self.seen.is_none() {
                        // A second pass in a row that could not read its
                        // marks backs off as a failed one does.
                        self.failing = true;
                        self.due = self.due.map(|d| d.min(now + self.retry));
                        self.retry = (self.retry * 2).min(RETRY_MAX);
                    } else {
                        // Without marks nothing vouches for this revision:
                        // the next readable marks differ from none and run
                        // a pass before any check.
                        self.unmarked = self.seen.is_none();
                        self.failing = false;
                        self.retry = RETRY;
                    }
                    Tick::Built
                }
                Err(e) => {
                    // A failed pass leaves nothing current, and moved marks
                    // wait for its retry: `seen` stays the last completed
                    // pass's.
                    tracing::warn!(error = %format!("{e:#}"), "map grid pass failed");
                    self.failing = true;
                    self.held_until = None;
                    self.current = None;
                    self.due = Some(now + self.retry);
                    self.retry = (self.retry * 2).min(RETRY_MAX);
                    Tick::Failed
                }
            };
        }
        let record = self
            .last_checked
            .is_none_or(|t| now.duration_since(t) >= CHECK_RECORD_INTERVAL);
        // Only the marks of the snapshot the revision was built from vouch
        // for it.
        if let (Some(w), Some(current)) = (&watched, &self.current)
            && w.ready
            && self.seen.as_deref() == Some(w.marks.as_str())
            && record
        {
            match confirm_current(state, ctx, current, w.at.naive_utc()).await {
                Ok(true) => {
                    self.last_checked = Some(now);
                    return Tick::Checked;
                }
                // Another revision is stored: compare the inputs with it now.
                Ok(false) => {
                    self.current = None;
                    self.due = None;
                }
                Err(e) => {
                    tracing::warn!(error = %format!("{e:#}"), "recording the map grid checked failed")
                }
            }
        }
        Tick::Idle
    }
}

/// The leased build job's body: a [`Watcher`] ticking every
/// [`WATCH_INTERVAL`] until cancelled.
pub async fn watch(state: &Shared, ctx: &JobContext) -> anyhow::Result<()> {
    let mut watcher = Watcher::default();
    loop {
        watcher.tick(state, ctx, Instant::now()).await;
        tokio::select! {
            _ = ctx.cancel.cancelled() => return Ok(()),
            _ = tokio::time::sleep(WATCH_INTERVAL) => {}
        }
    }
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
