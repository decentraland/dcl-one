//! api.decentraland.org: `/v{1,2}/map.png` (the navmap atlas chunks the
//! explorer loads), the minimaps, `/v{1,2}/tiles` and `/v2/tiles/info`,
//! rendered from a mirrored copy of upstream's tile grid, plus the JSON
//! metadata and district routes and the probes ([`meta`]), and
//! `/satellite/{z}/{x}/{y}` painted over this grid (`super::satellite`). The grid is ~92k
//! tiles, 38 MB of JSON: the `map-grid` pass polls the 30-byte
//! `/v2/tiles/info` stamp and downloads the grid only when the stamp moved
//! and the last download is older than `map_min_refresh`, with
//! `If-None-Match`. The gzipped grid is one row of `explore_api.map_grid`,
//! loaded at startup, so a restart serves the last grid before its first
//! pass. A chain indexer of LAND/estate ownership and open orders would let
//! the old builder run here instead.

pub mod http_cache;
pub mod meta;
pub mod png;
pub mod render;
pub mod tiles;

use std::collections::HashMap;
use std::io::{Read as _, Write as _};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use axum::Router;
use axum::routing::get;
use chrono::NaiveDateTime;
use parking_lot::RwLock;
use serde::Deserialize;
use sqlx::PgPool;

use super::{Shared, mirror};
use crate::jobs::JobContext;
use crate::shared::cache::TtlMap;
use tiles::{Tile, TileType};

pub const MIRROR: &str = "map-grid";
const INFO_BODY_CAP: usize = 4 << 10;
const GRID_BODY_CAP: usize = 96 << 20;
const RESPONSE_CACHE_TTL: Duration = Duration::from_secs(120);
const PNG_CACHE_ENTRIES: usize = 256;
const TILES_CACHE_ENTRIES: usize = 512;

pub struct Grid {
    pub tiles: HashMap<String, Tile>,
    /// Upstream's `/v2/tiles/info` stamp when this grid was current.
    pub last_updated_at: i64,
    pub etag: Option<String>,
    pub fetched_at: NaiveDateTime,
    /// Mixed into ETags and cache keys, so two grids with one stamp still differ.
    pub content_tag: u64,
    /// estate id -> coords of its owned parcels (estate map.png selection).
    pub estates_owned: HashMap<String, Vec<(i32, i32)>>,
    /// estate id -> coords of every parcel carrying it (the fallback selection).
    pub estates_all: HashMap<String, Vec<(i32, i32)>>,
}

#[derive(Deserialize)]
struct TilesDocument {
    #[serde(default)]
    ok: bool,
    data: HashMap<String, Tile>,
}

impl Grid {
    /// Parses upstream's `{ok, data: {"x,y": tile}}` document.
    pub fn from_json(
        body: &[u8],
        last_updated_at: i64,
        etag: Option<String>,
        fetched_at: NaiveDateTime,
    ) -> anyhow::Result<Self> {
        let doc: TilesDocument = serde_json::from_slice(body).context("tiles document")?;
        anyhow::ensure!(doc.ok, "tiles document has ok=false");
        anyhow::ensure!(!doc.data.is_empty(), "tiles document has no tiles");
        Ok(Self::from_tiles(
            doc.data,
            last_updated_at,
            etag,
            fetched_at,
        ))
    }

    pub fn from_tiles(
        tiles: HashMap<String, Tile>,
        last_updated_at: i64,
        etag: Option<String>,
        fetched_at: NaiveDateTime,
    ) -> Self {
        let mut estates_owned: HashMap<String, Vec<(i32, i32)>> = HashMap::new();
        let mut estates_all: HashMap<String, Vec<(i32, i32)>> = HashMap::new();
        for tile in tiles.values() {
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
        for b in format!(
            "{last_updated_at}:{}:{}",
            etag.as_deref().unwrap_or(""),
            tiles.len()
        )
        .bytes()
        {
            content_tag ^= b as u64;
            content_tag = content_tag.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Self {
            tiles,
            last_updated_at,
            etag,
            fetched_at,
            content_tag,
            estates_owned,
            estates_all,
        }
    }
}

pub struct MapState {
    grid: RwLock<Option<Arc<Grid>>>,
    png_cache: TtlMap<String, Arc<Vec<u8>>>,
    tiles_cache: TtlMap<String, Arc<Vec<u8>>>,
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
            png_cache: TtlMap::bounded(RESPONSE_CACHE_TTL, PNG_CACHE_ENTRIES),
            tiles_cache: TtlMap::bounded(RESPONSE_CACHE_TTL, TILES_CACHE_ENTRIES),
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

    pub fn content_tag(&self) -> u64 {
        self.grid.read().as_ref().map_or(0, |g| g.content_tag)
    }

    pub fn set(&self, grid: Grid) {
        *self.grid.write() = Some(Arc::new(grid));
        self.png_cache.clear();
        self.tiles_cache.clear();
    }

    pub fn cached_png(&self, key: &str) -> Option<Arc<Vec<u8>>> {
        self.png_cache.get_fresh(&key.to_string())
    }

    pub fn store_png(&self, key: String, body: Arc<Vec<u8>>) {
        self.png_cache.insert(key, body);
    }

    pub fn cached_tiles(&self, key: &str) -> Option<Arc<Vec<u8>>> {
        self.tiles_cache.get_fresh(&key.to_string())
    }

    pub fn store_tiles(&self, key: String, body: Arc<Vec<u8>>) {
        self.tiles_cache.insert(key, body);
    }

    /// The persisted grid, if a pass ever stored one.
    pub async fn load(&self, pool: &PgPool) -> anyhow::Result<()> {
        let row: Option<(i64, Option<String>, Vec<u8>, NaiveDateTime)> = sqlx::query_as(
            "SELECT last_updated_at, etag, body_gzip, fetched_at FROM explore_api.map_grid WHERE id = 1",
        )
        .fetch_optional(pool)
        .await
        .context("reading explore_api.map_grid")?;
        let Some((last_updated_at, etag, body_gzip, fetched_at)) = row else {
            tracing::info!(
                "no mirrored tile grid yet; map routes answer 503 until the first map-grid pass"
            );
            return Ok(());
        };
        let grid = tokio::task::spawn_blocking(move || -> anyhow::Result<Grid> {
            let mut body = Vec::new();
            flate2::read::GzDecoder::new(body_gzip.as_slice())
                .read_to_end(&mut body)
                .context("inflating the stored grid")?;
            Grid::from_json(&body, last_updated_at, etag, fetched_at)
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

#[derive(Deserialize)]
struct TilesInfo {
    #[serde(rename = "lastUpdatedAt")]
    last_updated_at: i64,
}

pub async fn store(
    pool: &PgPool,
    last_updated_at: i64,
    etag: Option<&str>,
    tile_count: i64,
    body_gzip: &[u8],
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO explore_api.map_grid (id, last_updated_at, etag, tile_count, body_gzip, fetched_at)
         VALUES (1, $1, $2, $3, $4, timezone('UTC', now()))
         ON CONFLICT (id) DO UPDATE
         SET last_updated_at = EXCLUDED.last_updated_at, etag = EXCLUDED.etag,
             tile_count = EXCLUDED.tile_count, body_gzip = EXCLUDED.body_gzip,
             fetched_at = EXCLUDED.fetched_at",
    )
    .bind(last_updated_at)
    .bind(etag)
    .bind(tile_count as i32)
    .bind(body_gzip)
    .execute(pool)
    .await?;
    Ok(())
}

/// One pass: the stamp, then the grid only if it moved past the one held
/// and the refresh floor allows; the pass is complete either way, with the
/// grid's stamp as its cursor.
pub async fn run_pass(state: &Shared, ctx: &JobContext) -> anyhow::Result<()> {
    mirror::run(&state.pool, MIRROR, ctx, || async {
        let base = state.settings.sources.map.as_str();
        let info_url = format!("{base}/v2/tiles/info");
        let info = mirror::fetch(&state.http, &state.settings, reqwest::Method::GET, &info_url, None, &[], INFO_BODY_CAP).await?;
        let info: TilesInfo = serde_json::from_slice(&info.body).with_context(|| format!("{}: not {{lastUpdatedAt}}", mirror::shown(&info_url)))?;

        let current = state.map.snapshot();
        let outcome = |g: &Grid| mirror::PassOutcome { items: g.tiles.len() as i64, cursor: Some(g.last_updated_at.to_string()) };
        if let Some(grid) = &current {
            if info.last_updated_at <= grid.last_updated_at {
                return Ok(outcome(grid));
            }
            let age = (chrono::Utc::now().naive_utc() - grid.fetched_at).to_std().unwrap_or_default();
            if age < state.settings.map_min_refresh {
                tracing::info!(
                    upstream = info.last_updated_at,
                    held = grid.last_updated_at,
                    age_secs = age.as_secs(),
                    "upstream tile grid moved; next download after the refresh floor"
                );
                return Ok(outcome(grid));
            }
        }
        anyhow::ensure!(!ctx.is_cancelled(), "cancelled before the grid download");

        let grid_url = format!("{base}/v2/tiles");
        let etag = current.as_ref().and_then(|g| g.etag.clone());
        let headers: Vec<(&str, &str)> = etag.as_deref().map(|e| ("if-none-match", e)).into_iter().collect();
        let fetched = mirror::fetch(&state.http, &state.settings, reqwest::Method::GET, &grid_url, None, &headers, GRID_BODY_CAP).await?;
        let fetched_at = chrono::Utc::now().naive_utc();

        if fetched.status == reqwest::StatusCode::NOT_MODIFIED {
            let grid = current.context("304 without a held grid")?;
            let stamped = Grid::from_tiles(grid.tiles.clone(), info.last_updated_at, grid.etag.clone(), fetched_at);
            sqlx::query("UPDATE explore_api.map_grid SET last_updated_at = $1, fetched_at = $2 WHERE id = 1")
                .bind(info.last_updated_at)
                .bind(fetched_at)
                .execute(&state.pool)
                .await
                .context("restamping explore_api.map_grid")?;
            let out = outcome(&stamped);
            state.map.set(stamped);
            return Ok(out);
        }

        let last_updated_at = info.last_updated_at;
        let etag = fetched.etag.clone();
        let (grid, gz) = tokio::task::spawn_blocking(move || -> anyhow::Result<(Grid, Vec<u8>)> {
            let grid = Grid::from_json(&fetched.body, last_updated_at, etag, fetched_at)?;
            let gz = gzip(&fetched.body)?;
            Ok((grid, gz))
        })
        .await
        .context("grid parse task")??;
        store(&state.pool, grid.last_updated_at, grid.etag.as_deref(), grid.tiles.len() as i64, &gz)
            .await
            .context("storing explore_api.map_grid")?;
        tracing::info!(tiles = grid.tiles.len(), gzip_bytes = gz.len(), last_updated_at, "tile grid mirrored");
        let out = outcome(&grid);
        state.map.set(grid);
        Ok(out)
    })
    .await
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
