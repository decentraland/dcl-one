//! The asset-packs catalogue (`GET /v1/assetPacks`) and the asset files
//! behind it (`GET /contents/{hash}`). The catalogue is one reference row,
//! `builder.asset_catalog` (the `data` array of the builder-server
//! envelope), written by the operator command [`build`] from an
//! `@dcl/asset-packs` tree and served whole with a 30 s in-process memo;
//! the hash -> file-name map a content type needs derives from it at load.
//! The files are blobs in the ONE store: [`build`] stores them, [`seed`]
//! pre-pulls a list, and a miss on serve pulls `<bucket>/contents/{hash}`
//! from `BUILDER_CONTENT_BUCKET_URL` (the item bucket), then from
//! `BUILDER_ASSET_PACKS_BUCKET_URL` (the asset-packs bucket that holds the
//! scene templates' `Qm` models), through the ONE guarded client, verifies
//! the bytes against the CID (raw digest or, for legacy `Qm` keys, the
//! UnixFS hash) and stores them. A bucket answering 404, 410 or 403 (S3's
//! answer for a key it does not hold) passes to the next; when every bucket
//! misses, the hash is a five-minute miss. The old LRU pull cache and its byte budget are gone: the catalogue
//! bounds the pullable set, and the store never unlinks.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use bytes::Bytes;
use chrono::NaiveDateTime;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::shared::cache::TtlMap;
use crate::storage::Storage;
use dcl_auth::{hash_bytes_v0_unixfs, hash_bytes_v1, is_canonical_cid, verify_hash};
use dcl_http::{Outbound, read_body_capped};

const MISS_TTL: Duration = Duration::from_secs(300);
const CATALOG_RECHECK: Duration = Duration::from_secs(30);
const MISS_CACHE_MAX: usize = 65536;
const MAX_CONTENT_BYTES: usize = 256 * 1024 * 1024;
const PULL_TIMEOUT: Duration = Duration::from_secs(120);

/// The buckets a miss is pulled from, tried in order.
pub struct PullSource {
    pub buckets: Vec<String>,
    pub http: Outbound,
}

pub struct CatalogStore {
    pool: PgPool,
    blobs: Arc<Storage>,
    pull: Option<PullSource>,
    catalog: Mutex<Option<(Instant, Option<Arc<CachedCatalog>>)>>,
    misses: TtlMap<String, ()>,
    pulling: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

/// Per-hash pull gate; the map entry goes away with the last holder.
struct PullGate<'a> {
    store: &'a CatalogStore,
    hash: String,
    gate: Arc<tokio::sync::Mutex<()>>,
}

impl Drop for PullGate<'_> {
    fn drop(&mut self) {
        if let Ok(mut m) = self.store.pulling.lock()
            && Arc::strong_count(&self.gate) <= 2
            && m.get(&self.hash)
                .is_some_and(|g| Arc::ptr_eq(g, &self.gate))
        {
            m.remove(&self.hash);
        }
    }
}

struct CachedCatalog {
    built_at: NaiveDateTime,
    body: Bytes,
    names: HashMap<String, String>,
}

pub struct Content {
    pub bytes: Bytes,
    pub content_type: &'static str,
}

#[derive(Debug)]
pub enum UpstreamError {
    Status(u16),
    Unreachable(anyhow::Error),
    Rejected(anyhow::Error),
}

impl fmt::Display for UpstreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Status(s) => write!(f, "upstream answered {s}"),
            Self::Unreachable(e) => write!(f, "upstream unreachable: {e:#}"),
            Self::Rejected(e) => write!(f, "upstream response rejected: {e:#}"),
        }
    }
}

#[derive(Debug)]
pub enum ContentError {
    InvalidHash,
    NotFound,
    Upstream(UpstreamError),
    Io(anyhow::Error),
}

pub fn content_type_for_name(name: &str) -> Option<&'static str> {
    let ext = name.rsplit('.').next()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "glb" => "model/gltf-binary",
        "gltf" => "model/gltf+json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "json" => "application/json",
        "js" | "mjs" => "text/javascript",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "wav" => "audio/wav",
        "mp4" => "video/mp4",
        "ktx2" => "image/ktx2",
        "bin" | "crdt" => "application/octet-stream",
        _ => return None,
    })
}

pub fn sniff_content_type(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG") {
        "image/png"
    } else if bytes.starts_with(b"\xFF\xD8\xFF") {
        "image/jpeg"
    } else if bytes.starts_with(b"glTF") {
        "model/gltf-binary"
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else if bytes
        .iter()
        .find(|b| !b.is_ascii_whitespace())
        .is_some_and(|b| *b == b'{' || *b == b'[')
    {
        "application/json"
    } else {
        "application/octet-stream"
    }
}

/// The packs array of a catalogue document in any of its layouts: a bare
/// array, the builder-server `{ data }` envelope or the npm `{ assetPacks }`.
pub fn packs_of(raw: Value) -> Result<Vec<Value>> {
    let packs = match raw {
        Value::Array(a) => a,
        Value::Object(mut o) => o
            .remove("data")
            .or_else(|| o.remove("assetPacks"))
            .and_then(|v| v.as_array().cloned())
            .ok_or_else(|| anyhow!("the catalogue has neither a data nor an assetPacks array"))?,
        _ => return Err(anyhow!("the catalogue is not a JSON object")),
    };
    Ok(packs)
}

fn index_names(packs: &[Value]) -> HashMap<String, String> {
    let mut names = HashMap::new();
    for pack in packs {
        if let Some(h) = pack.get("thumbnail").and_then(Value::as_str) {
            names.insert(h.to_string(), "thumbnail.png".to_string());
        }
        let Some(assets) = pack.get("assets").and_then(Value::as_array) else {
            continue;
        };
        for asset in assets {
            let Some(contents) = asset.get("contents").and_then(Value::as_object) else {
                continue;
            };
            for (file, hash) in contents {
                if let Some(h) = hash.as_str() {
                    names.insert(h.to_string(), file.clone());
                }
            }
        }
    }
    names
}

/// `GET url` through the guarded client; `Ok(None)` for a 404/410, the bytes
/// only when they hash to `hash` and fit `max_bytes`.
pub async fn fetch_verified(
    http: &Outbound,
    url: &str,
    hash: &str,
    max_bytes: usize,
) -> Result<Option<Vec<u8>>, UpstreamError> {
    let resp = http
        .get(url)
        .timeout(PULL_TIMEOUT)
        .send()
        .await
        .map_err(|e| {
            UpstreamError::Unreachable(anyhow::Error::from(e).context(format!("GET {url}")))
        })?;
    let status = resp.status();
    if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::GONE {
        return Ok(None);
    }
    if !status.is_success() {
        return Err(UpstreamError::Status(status.as_u16()));
    }
    let bytes = read_body_capped(resp, max_bytes).await.map_err(|e| {
        if e.to_string().contains("cap") {
            UpstreamError::Rejected(e.context(format!("{url} exceeds {max_bytes} bytes")))
        } else {
            UpstreamError::Unreachable(e.context(format!("read {url}")))
        }
    })?;
    if !bytes_match_cid(&bytes, hash) {
        return Err(UpstreamError::Rejected(anyhow!(
            "{url} served bytes whose CID is not {hash}"
        )));
    }
    Ok(Some(bytes))
}

pub fn bytes_match_cid(bytes: &[u8], hash: &str) -> bool {
    verify_hash(bytes, hash) || (hash.starts_with("Qm") && hash_bytes_v0_unixfs(bytes) == hash)
}

impl CatalogStore {
    pub fn new(pool: PgPool, blobs: Arc<Storage>, pull: Option<PullSource>) -> Result<Self> {
        let pull = match pull {
            Some(p) => {
                let buckets: Vec<String> = p
                    .buckets
                    .iter()
                    .map(|b| b.trim().trim_end_matches('/').to_string())
                    .collect();
                if buckets.is_empty() || buckets.iter().any(String::is_empty) {
                    return Err(anyhow!("pull-through needs a content bucket URL"));
                }
                Some(PullSource {
                    buckets,
                    http: p.http,
                })
            }
            None => None,
        };
        Ok(Self {
            pool,
            blobs,
            pull,
            catalog: Mutex::new(None),
            misses: TtlMap::bounded(MISS_TTL, MISS_CACHE_MAX),
            pulling: Mutex::new(HashMap::new()),
        })
    }

    pub fn pulls_through(&self) -> bool {
        self.pull.is_some()
    }

    fn remember_catalog(&self, catalog: Option<Arc<CachedCatalog>>) {
        if let Ok(mut slot) = self.catalog.lock() {
            *slot = Some((Instant::now(), catalog));
        }
    }

    /// The catalogue row, re-read at most every [`CATALOG_RECHECK`] and
    /// re-rendered only when `built_at` moved.
    async fn load_catalog(&self) -> Result<Option<Arc<CachedCatalog>>> {
        let cached = self
            .catalog
            .lock()
            .ok()
            .and_then(|c| c.as_ref().map(|(at, c)| (*at, c.clone())));
        if let Some((checked, catalog)) = &cached
            && checked.elapsed() < CATALOG_RECHECK
        {
            return Ok(catalog.clone());
        }
        let row: Option<(Value, NaiveDateTime)> =
            sqlx::query_as("SELECT packs, built_at FROM builder.asset_catalog WHERE id = 1")
                .fetch_optional(&self.pool)
                .await
                .context("reading builder.asset_catalog")?;
        let Some((packs, built_at)) = row else {
            self.remember_catalog(None);
            return Ok(None);
        };
        if let Some((_, Some(catalog))) = cached
            && catalog.built_at == built_at
        {
            self.remember_catalog(Some(catalog.clone()));
            return Ok(Some(catalog));
        }
        let packs = packs_of(packs)?;
        let names = index_names(&packs);
        let body = Bytes::from(serde_json::to_vec(&json!({ "ok": true, "data": packs }))?);
        let cached = Arc::new(CachedCatalog {
            built_at,
            body,
            names,
        });
        self.remember_catalog(Some(cached.clone()));
        Ok(Some(cached))
    }

    /// The builder-server envelope, `None` until the catalogue is built.
    pub async fn asset_packs(&self) -> Result<Option<Bytes>> {
        Ok(self.load_catalog().await?.map(|c| c.body.clone()))
    }

    fn pull_gate(&self, hash: &str) -> PullGate<'_> {
        let gate = self
            .pulling
            .lock()
            .ok()
            .map(|mut m| m.entry(hash.to_string()).or_default().clone())
            .unwrap_or_default();
        PullGate {
            store: self,
            hash: hash.to_string(),
            gate,
        }
    }

    async fn content_type(&self, hash: &str, bytes: &[u8]) -> &'static str {
        let named = match self.load_catalog().await {
            Ok(Some(c)) => c.names.get(hash).and_then(|n| content_type_for_name(n)),
            _ => None,
        };
        named.unwrap_or_else(|| sniff_content_type(bytes))
    }

    fn recently_missed(&self, hash: &str) -> bool {
        self.misses.get_fresh(&hash.to_string()).is_some()
    }

    fn record_miss(&self, hash: &str) {
        self.misses.insert(hash.to_string(), ());
    }

    async fn served(&self, hash: &str, bytes: Bytes) -> Content {
        let content_type = self.content_type(hash, &bytes).await;
        Content {
            bytes,
            content_type,
        }
    }

    async fn stored(&self, hash: &str) -> Result<Option<Bytes>, ContentError> {
        self.blobs
            .retrieve(hash)
            .await
            .map_err(|e| ContentError::Io(anyhow::Error::from(e).context(format!("read {hash}"))))
    }

    pub async fn content(&self, hash: &str) -> Result<Content, ContentError> {
        if !is_canonical_cid(hash) {
            return Err(ContentError::InvalidHash);
        }
        if let Some(bytes) = self.stored(hash).await? {
            return Ok(self.served(hash, bytes).await);
        }
        let Some(pull) = self.pull.as_ref() else {
            return Err(ContentError::NotFound);
        };
        if self.recently_missed(hash) {
            return Err(ContentError::NotFound);
        }
        let gate = self.pull_gate(hash);
        let _flight = match gate.gate.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                let guard = gate.gate.lock().await;
                // A follower re-reads whatever the flight ahead of it stored or missed.
                if let Some(bytes) = self.stored(hash).await? {
                    return Ok(self.served(hash, bytes).await);
                }
                if self.recently_missed(hash) {
                    return Err(ContentError::NotFound);
                }
                guard
            }
        };
        let Some(bytes) = pull_first(pull, hash)
            .await
            .map_err(ContentError::Upstream)?
        else {
            self.record_miss(hash);
            return Err(ContentError::NotFound);
        };
        let bytes = Bytes::from(bytes);
        self.blobs.store(hash, bytes.clone()).await.map_err(|e| {
            ContentError::Io(anyhow::Error::from(e).context(format!("store {hash}")))
        })?;
        Ok(self.served(hash, bytes).await)
    }
}

/// The first bucket holding `hash`, verified. `Ok(None)` when every bucket
/// answered 404, 410 or 403; otherwise the first failure when none served it.
async fn pull_first(pull: &PullSource, hash: &str) -> Result<Option<Vec<u8>>, UpstreamError> {
    let mut failure = None;
    for bucket in &pull.buckets {
        let url = format!("{bucket}/contents/{hash}");
        match fetch_verified(&pull.http, &url, hash, MAX_CONTENT_BYTES).await {
            Ok(Some(bytes)) => return Ok(Some(bytes)),
            Ok(None) | Err(UpstreamError::Status(403)) => {}
            Err(e) => {
                tracing::warn!(error = %e, hash, bucket, "builder content pull failed");
                failure.get_or_insert(e);
            }
        }
    }
    failure.map_or(Ok(None), Err)
}

/// `catalyst builder seed <contents-base-url> <cid>...`: pre-pulls the listed
/// files into the ONE store, verified; reports the liars and the misses.
pub async fn seed(
    blobs: &Storage,
    http: &Outbound,
    base_url: &str,
    hashes: &[String],
) -> Result<usize> {
    let base = base_url.trim_end_matches('/');
    let mut stored = 0usize;
    let mut failed: Vec<String> = Vec::new();
    for hash in hashes {
        if !is_canonical_cid(hash) {
            failed.push(format!("{hash}: not a canonical CID"));
            continue;
        }
        if blobs.exist(hash).await? {
            continue;
        }
        match fetch_verified(http, &format!("{base}/{hash}"), hash, MAX_CONTENT_BYTES).await {
            Ok(Some(bytes)) => {
                blobs.store(hash, Bytes::from(bytes)).await?;
                stored += 1;
            }
            Ok(None) => failed.push(format!("{hash}: not served by {base}")),
            Err(e) => failed.push(format!("{hash}: {e}")),
        }
    }
    println!(
        "seed: {} fetched, {} already present, {} failed",
        stored,
        hashes.len() - stored - failed.len(),
        failed.len(),
    );
    if failed.is_empty() {
        Ok(stored)
    } else {
        Err(anyhow!("seed failed for:\n  {}", failed.join("\n  ")))
    }
}

fn read_json(path: &Path) -> Result<Value> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))
}

fn subdirs_with_data(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out: Vec<PathBuf> = fs::read_dir(dir)
        .with_context(|| format!("read dir {}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir() && p.join("data.json").exists())
        .collect();
    out.sort();
    Ok(out)
}

fn asset_files(root: &Path) -> Result<Vec<(String, PathBuf)>> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                walk(base, &path, out)?;
            } else {
                let rel = path
                    .strip_prefix(base)?
                    .to_string_lossy()
                    .replace('\\', "/");
                if rel != "data.json" {
                    out.push((rel, path));
                }
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(root, root, &mut out)?;
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

fn model_from_composite(composite: &Value) -> Option<String> {
    for component in composite.get("components")?.as_array()? {
        if component.get("name").and_then(Value::as_str) == Some("core::GltfContainer") {
            let src = component
                .get("data")?
                .get("0")?
                .get("json")?
                .get("src")?
                .as_str()?;
            return Some(src.strip_prefix("{assetPath}/").unwrap_or(src).to_string());
        }
    }
    None
}

async fn store_file(blobs: &Storage, bytes: Vec<u8>, stored: &mut usize) -> Result<String> {
    let hash = hash_bytes_v1(&bytes);
    if !blobs.exist(&hash).await? {
        blobs
            .store(&hash, Bytes::from(bytes))
            .await
            .with_context(|| format!("store {hash}"))?;
        *stored += 1;
    }
    Ok(hash)
}

/// `catalyst builder catalog <packs-dir>` (the old `catalyrst-builder catalog
/// <packs> <dir>`): walks the `@dcl/asset-packs` tree, stores every asset
/// file in the ONE store under its CIDv1 and upserts the one catalogue row.
pub async fn build(pool: &PgPool, blobs: &Storage, packs_dir: &str) -> Result<()> {
    let mut packs_json: Vec<Value> = Vec::new();
    let mut stored = 0usize;
    let mut total_assets = 0usize;

    for pack_dir in subdirs_with_data(Path::new(packs_dir))? {
        let pack_data = read_json(&pack_dir.join("data.json"))?;
        let title = pack_data
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let pack_id = pack_data
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| {
                pack_dir
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            });

        let assets_dir = pack_dir.join("assets");
        if !assets_dir.is_dir() {
            continue;
        }

        let pack_thumbnail = match fs::read(pack_dir.join("thumbnail.png")) {
            Ok(bytes) => Value::String(store_file(blobs, bytes, &mut stored).await?),
            Err(_) => Value::Null,
        };

        let mut assets_json: Vec<Value> = Vec::new();
        for asset_dir in subdirs_with_data(&assets_dir)? {
            if asset_dir
                .to_string_lossy()
                .contains("/admin_toolkit/assets/")
            {
                continue;
            }
            let data = read_json(&asset_dir.join("data.json"))?;
            let composite = read_json(&asset_dir.join("composite.json")).unwrap_or(Value::Null);

            // Decide on the asset before storing anything: the catalogue row must
            // name every blob the walk stores (the backup guard treats the named
            // set as boot state), so an asset without a model stores nothing.
            let files = asset_files(&asset_dir)?;
            let model = model_from_composite(&composite).or_else(|| {
                files
                    .iter()
                    .map(|(rel, _)| rel)
                    .find(|k| k.ends_with(".glb") || k.ends_with(".gltf"))
                    .cloned()
            });
            let Some(model) = model.filter(|m| files.iter().any(|(rel, _)| rel == m)) else {
                continue;
            };

            let mut contents: BTreeMap<String, String> = BTreeMap::new();
            for (rel, abs) in files {
                let bytes = fs::read(&abs)?;
                contents.insert(rel, store_file(blobs, bytes, &mut stored).await?);
            }

            let mut asset = data.as_object().cloned().unwrap_or_default();
            asset.insert("asset_pack_id".into(), json!(pack_id));
            asset.insert("model".into(), json!(model));
            if let Some(thumb) = contents.get("thumbnail.png") {
                asset.insert("thumbnail".into(), json!(thumb));
            }
            asset.insert("contents".into(), json!(contents));
            if !composite.is_null() {
                asset.insert("composite".into(), composite);
            }
            assets_json.push(Value::Object(asset));
        }

        total_assets += assets_json.len();
        packs_json.push(json!({
            "id": pack_id,
            "title": title,
            "thumbnail": pack_thumbnail,
            "eth_address": Value::Null,
            "assets": assets_json,
        }));
    }

    let packs = packs_json.len();
    write_catalog(pool, Value::Array(packs_json), packs_dir).await?;
    println!(
        "catalog: {packs} packs, {total_assets} assets, {stored} content files stored -> builder.asset_catalog"
    );
    Ok(())
}

/// `catalyst builder catalog <catalog.json>`: the catalogue the npm
/// `@dcl/asset-packs` package ships, which names every file by its hash
/// instead of carrying it. Upserts the same row [`build`] writes and stores
/// nothing; the files are pulled from `BUILDER_CONTENT_BUCKET_URL` on first
/// read like any other miss.
pub async fn import(pool: &PgPool, catalog_json: &str) -> Result<()> {
    let catalog = read_json(Path::new(catalog_json))?;
    let packs = packs_from_catalog_json(&catalog)?;
    let (n_packs, n_assets) = (
        packs.as_array().map_or(0, Vec::len),
        packs.as_array().map_or(0, |p| {
            p.iter()
                .map(|p| p["assets"].as_array().map_or(0, Vec::len))
                .sum()
        }),
    );
    write_catalog(pool, packs, catalog_json).await?;
    println!(
        "catalog: {n_packs} packs, {n_assets} assets from {catalog_json} -> builder.asset_catalog"
    );
    Ok(())
}

/// The row [`build`] would write for the packs in an npm `catalog.json`
/// (`{"assetPacks": [{id, name, thumbnail, assets: [{.., composite, contents}]}]}`):
/// `name` becomes `title`, each asset gains its pack id, model and
/// thumbnail, and an asset without a model is left out, as [`build`] does.
pub fn packs_from_catalog_json(catalog: &Value) -> Result<Value> {
    let packs = catalog
        .get("assetPacks")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("catalog.json has no assetPacks array"))?;
    let mut out = Vec::with_capacity(packs.len());
    for pack in packs {
        let pack_id = pack.get("id").and_then(Value::as_str).unwrap_or_default();
        let mut assets = Vec::new();
        for asset in pack
            .get("assets")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(mut asset) = asset.as_object().cloned() else {
                continue;
            };
            let contents = asset
                .get("contents")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            let composite = asset.get("composite").cloned().unwrap_or(Value::Null);
            let model = model_from_composite(&composite).or_else(|| {
                contents
                    .keys()
                    .find(|k| k.ends_with(".glb") || k.ends_with(".gltf"))
                    .cloned()
            });
            let Some(model) = model.filter(|m| contents.contains_key(m)) else {
                continue;
            };
            asset.insert("asset_pack_id".into(), json!(pack_id));
            asset.insert("model".into(), json!(model));
            if let Some(thumb) = contents.get("thumbnail.png") {
                asset.insert("thumbnail".into(), thumb.clone());
            }
            assets.push(Value::Object(asset));
        }
        out.push(json!({
            "id": pack_id,
            "title": pack.get("name").and_then(Value::as_str).unwrap_or_default(),
            "thumbnail": pack.get("thumbnail").cloned().unwrap_or(Value::Null),
            "eth_address": Value::Null,
            "assets": assets,
        }));
    }
    Ok(Value::Array(out))
}

/// Upserts the one catalogue row (`packs` must be an array).
pub async fn write_catalog(pool: &PgPool, packs: Value, source: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO builder.asset_catalog (id, packs, source, built_at) VALUES (1, $1, $2, timezone('UTC', now())) \
         ON CONFLICT (id) DO UPDATE SET packs = EXCLUDED.packs, source = EXCLUDED.source, built_at = EXCLUDED.built_at",
    )
    .bind(packs)
    .bind(source)
    .execute(pool)
    .await
    .context("writing builder.asset_catalog")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dcl_auth::hash_bytes;

    #[test]
    fn legacy_builder_qm_hashes_are_ipfs_unixfs_not_catalyst_raw_digests() {
        let glb = b"glTF legacy builder model";
        let catalyst = hash_bytes(glb);
        let ipfs = hash_bytes_v0_unixfs(glb);
        assert_ne!(catalyst, ipfs);
        assert!(bytes_match_cid(glb, &catalyst));
        assert!(bytes_match_cid(glb, &ipfs));
        assert!(bytes_match_cid(glb, &hash_bytes_v1(glb)));
        assert!(!bytes_match_cid(b"other bytes", &ipfs));
    }

    #[test]
    fn an_npm_catalog_json_becomes_the_row_the_packs_walk_writes() {
        let catalog = json!({"assetPacks": [{
            "id": "p1", "name": "Cyberpunk", "thumbnail": "bafkpack",
            "assets": [
                {"id": "a1", "name": "A Blue", "category": "decorations",
                 "composite": {"components": [{"name": "core::GltfContainer",
                     "data": {"0": {"json": {"src": "{assetPath}/A_Blue.glb"}}}}]},
                 "contents": {"A_Blue.glb": "bafyglb", "thumbnail.png": "bafkthumb"}},
                {"id": "a2", "name": "No model", "contents": {"thumbnail.png": "bafkx"}}
            ]
        }]});
        let packs = packs_from_catalog_json(&catalog).unwrap();
        let pack = &packs[0];
        assert_eq!(
            (pack["id"].as_str(), pack["title"].as_str()),
            (Some("p1"), Some("Cyberpunk"))
        );
        assert_eq!(pack["thumbnail"], "bafkpack");
        assert!(pack["eth_address"].is_null());
        let assets = pack["assets"].as_array().unwrap();
        assert_eq!(assets.len(), 1, "an asset without a model is left out");
        assert_eq!(assets[0]["model"], "A_Blue.glb");
        assert_eq!(assets[0]["thumbnail"], "bafkthumb");
        assert_eq!(assets[0]["asset_pack_id"], "p1");
        assert_eq!(assets[0]["contents"]["A_Blue.glb"], "bafyglb");
        assert!(packs_from_catalog_json(&json!({})).is_err());
    }

    #[test]
    fn content_types_come_from_the_name_then_the_bytes() {
        assert_eq!(content_type_for_name("m/a.glb"), Some("model/gltf-binary"));
        assert_eq!(content_type_for_name("thumbnail.PNG"), Some("image/png"));
        assert_eq!(content_type_for_name("weird.xyz"), None);
        assert_eq!(sniff_content_type(b"\x89PNG\r\n"), "image/png");
        assert_eq!(sniff_content_type(b"glTF\x02"), "model/gltf-binary");
        assert_eq!(sniff_content_type(b"  {\"a\":1}"), "application/json");
        assert_eq!(sniff_content_type(b"RIFF0000WEBPVP8 "), "image/webp");
        assert_eq!(sniff_content_type(b"\x00\x01"), "application/octet-stream");
    }

    #[test]
    fn packs_of_accepts_every_catalogue_layout() {
        assert_eq!(packs_of(json!([{ "id": "p" }])).unwrap().len(), 1);
        assert_eq!(
            packs_of(json!({ "data": [{ "id": "p" }] })).unwrap().len(),
            1
        );
        assert_eq!(
            packs_of(json!({ "assetPacks": [{ "id": "p" }, { "id": "q" }] }))
                .unwrap()
                .len(),
            2
        );
        assert!(packs_of(json!({ "nope": [] })).is_err());
        assert!(packs_of(json!("x")).is_err());
        let names = index_names(&[
            json!({ "thumbnail": "bafythumb", "assets": [{ "contents": { "m/a.glb": "bafya" } }] }),
        ]);
        assert_eq!(names.get("bafya").map(String::as_str), Some("m/a.glb"));
        assert_eq!(
            names.get("bafythumb").map(String::as_str),
            Some("thumbnail.png")
        );
    }
}
