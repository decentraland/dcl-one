//! An item's thumbnail and image by pointer: `GET`/`HEAD
//! /queries/items/{pointer}/{thumbnail,image}` (at the root and under
//! `/content`) and `/lambdas/collections/contents/{pointer}/{thumbnail,image}`.
//! The pointer's active entity names the file in its metadata; the bytes
//! come from the ONE blob store with the content route's headers, ranges and
//! `X-Accel-Redirect`, and the operator denylist hides the entity or the
//! file. `?width=` (80, 160, 320, 640, 960) serves a bounded derivative of
//! the stored thumbnail, rendered here (never fetched), memoized in a
//! hard-bounded LRU with single-flight misses. The v2 store keeps no
//! compressed blobs, so the old gzip source decode is gone. With
//! read-through on, a pointer or file this node does not hold is filled from
//! the sync sources first, as `/entities/active` and `/contents` do.

use std::collections::HashMap;
use std::future::Future;
use std::io::Cursor;
use std::sync::{Arc, LazyLock, Mutex, Weak};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use image::codecs::jpeg::JpegEncoder;
use image::{ImageFormat, ImageReader, Limits};
use serde_json::Value;
use tokio::sync::{Mutex as AsyncMutex, Semaphore};

use super::errors::{AppError, AppResult};
use super::formatters::{
    ParsedRange, check_not_modified, content_file_headers, parse_range_header,
};
use super::get_content::{detect_content_type, set_content_type, x_accel_redirect_path};
use super::queries::active_entity_docs_by_pointers;
use super::{ContentState, SharedState};

#[derive(Default, serde::Deserialize)]
pub struct ThumbnailQuery {
    pub width: Option<u32>,
}

pub fn valid_width(width: u32) -> bool {
    matches!(width, 80 | 160 | 320 | 640 | 960)
}

async fn entity_by_pointer(state: &ContentState, pointer: &str) -> AppResult<Value> {
    let pointers = [pointer.to_string()];
    let docs = match state.read_through() {
        Some(read_through) => read_through.active_docs(false, &pointers).await?,
        None => active_entity_docs_by_pointers(&state.pool, &pointers).await?,
    };
    let entity = docs
        .first()
        .and_then(|d| d.to_value())
        .ok_or_else(|| AppError::not_found("Entity not found."))?;
    let entity_id = entity.get("id").and_then(|id| id.as_str()).unwrap_or("");
    if entity_id.is_empty() || state.denylist.contains(entity_id) {
        return Err(AppError::not_found("Entity not found."));
    }
    Ok(entity)
}

/// The content hash of the file `metadata.<field>` names.
fn file_hash(entity: &Value, field: &str) -> Option<String> {
    let path = entity.get("metadata")?.get(field)?.as_str()?;
    entity.get("content")?.as_array()?.iter().find_map(|item| {
        let file = item.get("file").or_else(|| item.get("key"))?.as_str()?;
        (file == path)
            .then(|| {
                item.get("hash")
                    .and_then(|h| h.as_str())
                    .map(str::to_string)
            })
            .flatten()
    })
}

fn not_modified(headers: Vec<(&'static str, String)>) -> Response {
    let mut response = StatusCode::NOT_MODIFIED.into_response();
    let resp_headers = response.headers_mut();
    for (name, value) in headers {
        if let Ok(hv) = value.parse() {
            resp_headers.insert(name, hv);
        }
    }
    response
}

/// `GET`/`HEAD .../{pointer}/thumbnail`.
pub async fn get_entity_thumbnail(
    State(state): State<SharedState>,
    Path(pointer): Path<String>,
    Query(query): Query<ThumbnailQuery>,
    method: Method,
    headers: HeaderMap,
) -> AppResult<Response> {
    if query.width.is_some_and(|width| !valid_width(width)) {
        return Ok((
            StatusCode::BAD_REQUEST,
            "width must be 80, 160, 320, 640 or 960",
        )
            .into_response());
    }
    let entity = entity_by_pointer(&state, &pointer).await?;
    let hash = file_hash(&entity, "thumbnail")
        .ok_or_else(|| AppError::not_found("Entity has no thumbnail."))?;
    if state.denylist.contains(&hash) {
        return Err(AppError::not_found("Entity has no thumbnail."));
    }
    if let Some(width) = query.width {
        return serve_variant(&state, &hash, width, &method, &headers).await;
    }
    if let Some(h) = check_not_modified(&headers, &hash) {
        return Ok(not_modified(h));
    }
    serve_content_blob(&state, &hash, &method, &headers).await
}

/// `GET`/`HEAD .../{pointer}/image`.
pub async fn get_entity_image(
    State(state): State<SharedState>,
    Path(pointer): Path<String>,
    method: Method,
    headers: HeaderMap,
) -> AppResult<Response> {
    let entity = entity_by_pointer(&state, &pointer).await?;
    let hash =
        file_hash(&entity, "image").ok_or_else(|| AppError::not_found("Entity has no image."))?;
    if state.denylist.contains(&hash) {
        return Err(AppError::not_found("Entity has no image."));
    }
    if let Some(h) = check_not_modified(&headers, &hash) {
        return Ok(not_modified(h));
    }
    serve_content_blob(&state, &hash, &method, &headers).await
}

fn apply(response: &mut Response, headers: &[(&'static str, String)]) {
    let resp_headers = response.headers_mut();
    for (name, value) in headers {
        if let Ok(hv) = value.parse() {
            resp_headers.insert(*name, hv);
        }
    }
}

/// The stored file with its sniffed type, ranges and the accel redirect.
/// A stored file, filled from the sync sources first when read-through is on.
async fn open_blob(
    state: &ContentState,
    hash: &str,
) -> AppResult<Option<crate::storage::OpenedContent>> {
    if let Some(opened) = state.storage.open(hash).await? {
        return Ok(Some(opened));
    }
    match state.read_through() {
        Some(read_through) if read_through.fill_content(hash).await => {
            Ok(state.storage.open(hash).await?)
        }
        _ => Ok(None),
    }
}

async fn serve_content_blob(
    state: &ContentState,
    hash: &str,
    method: &Method,
    headers: &HeaderMap,
) -> AppResult<Response> {
    let mut opened = open_blob(state, hash)
        .await?
        .ok_or_else(|| AppError::not_found("Content not found."))?;
    let head = opened.read_range(0, 31).await?;
    let detected = detect_content_type(&head);
    let range_header = headers.get("range").and_then(|v| v.to_str().ok());
    let total = opened.size;
    let range = parse_range_header(range_header, Some(total));
    let mut base_headers = content_file_headers(hash, Some(total));
    set_content_type(&mut base_headers, detected);
    match range {
        Some(ParsedRange::Unsatisfiable) => {
            let mut response = StatusCode::RANGE_NOT_SATISFIABLE.into_response();
            apply(
                &mut response,
                &[
                    ("Content-Range", format!("bytes */{total}")),
                    ("Access-Control-Expose-Headers", "Content-Range".into()),
                ],
            );
            Ok(response)
        }
        Some(ParsedRange::Range { start, end }) => {
            let body: Bytes = if *method == Method::HEAD {
                Bytes::new()
            } else {
                opened.read_range(start, end).await?
            };
            let mut response = (StatusCode::PARTIAL_CONTENT, body).into_response();
            apply(&mut response, &base_headers);
            apply(
                &mut response,
                &[
                    ("Content-Range", format!("bytes {start}-{end}/{total}")),
                    ("Content-Length", (end - start + 1).to_string()),
                ],
            );
            Ok(response)
        }
        None => {
            if let Some(accel) =
                x_accel_redirect_path(state.settings.storage_x_accel_base.as_deref(), hash)
            {
                base_headers.retain(|(n, _)| *n != "Content-Length");
                let mut response = (StatusCode::OK, Body::empty()).into_response();
                apply(&mut response, &base_headers);
                apply(
                    &mut response,
                    &[("X-Accel-Redirect", accel), ("Content-Length", "0".into())],
                );
                return Ok(response);
            }
            let body = if *method == Method::HEAD {
                Body::empty()
            } else {
                opened.into_body(head)
            };
            let mut response = (StatusCode::OK, body).into_response();
            apply(&mut response, &base_headers);
            Ok(response)
        }
    }
}

const MAX_SOURCE: u64 = 8 * 1024 * 1024;
/// A collection pointer can move to new content; the ETag includes the resolved hash.
const CACHE_CONTROL: &str = "public, max-age=300, stale-while-revalidate=86400";
static CACHE: LazyLock<ThumbnailCache> =
    LazyLock::new(|| ThumbnailCache::new(128, 16, Duration::from_secs(3600)));
/// Decodes at once: a quarter of the cores, 2 to 4, each bounded to 128 MiB.
static RESIZERS: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(resizers(cores())));

fn cores() -> usize {
    std::thread::available_parallelism().map_or(1, usize::from)
}

fn resizers(cores: usize) -> usize {
    (cores / 4).clamp(2, 4)
}

fn variant_key(hash: &str, width: u32) -> String {
    format!("thumbnail-v1-{width}-{hash}")
}

async fn serve_variant(
    state: &ContentState,
    hash: &str,
    width: u32,
    method: &Method,
    headers: &HeaderMap,
) -> AppResult<Response> {
    let key = variant_key(hash, width);
    if check_not_modified(headers, &key).is_some() {
        return Ok((
            StatusCode::NOT_MODIFIED,
            [
                (header::ETAG, format!("\"{key}\"")),
                (header::CACHE_CONTROL, CACHE_CONTROL.into()),
            ],
        )
            .into_response());
    }
    let (kind, bytes) = CACHE
        .get_or_fetch(key.clone(), || async {
            let mut source = open_blob(state, hash)
                .await?
                .ok_or_else(|| AppError::not_found("Content not found."))?;
            if source.size > MAX_SOURCE {
                return Err(AppError::invalid("Thumbnail source too large"));
            }
            let bytes = source.read_range(0, source.size.saturating_sub(1)).await?;
            render(bytes, width).await.map_err(AppError::invalid)
        })
        .await?;
    let len = bytes.len();
    let body = if *method == Method::HEAD {
        Bytes::new()
    } else {
        bytes
    };
    Ok((
        [
            (header::CONTENT_TYPE, kind.to_string()),
            (header::CONTENT_LENGTH, len.to_string()),
            (header::CACHE_CONTROL, CACHE_CONTROL.into()),
            (header::ETAG, format!("\"{key}\"")),
        ],
        body,
    )
        .into_response())
}

fn resize(bytes: Bytes, width: u32) -> Result<(&'static str, Bytes), String> {
    let mut reader = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|_| "Unrecognized image")?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|_| "Image could not be decoded within limits")?;
    // Never enlarge small source images: larger encoded files add no detail.
    let bound = width.min(image.width().max(image.height()));
    let image = image.thumbnail(bound, bound);
    let mut output = Vec::new();
    let transparent =
        image.color().has_alpha() && image.to_rgba8().pixels().any(|pixel| pixel.0[3] < 255);
    let kind = if transparent {
        image
            .write_to(&mut Cursor::new(&mut output), ImageFormat::Png)
            .map_err(|_| "Thumbnail encoding failed")?;
        let mut webp = Cursor::new(Vec::new());
        image
            .write_to(&mut webp, ImageFormat::WebP)
            .map_err(|_| "Thumbnail encoding failed")?;
        if webp.get_ref().len() < output.len() {
            output = webp.into_inner();
            "image/webp"
        } else {
            "image/png"
        }
    } else {
        JpegEncoder::new_with_quality(&mut output, 80)
            .encode_image(&image.to_rgb8())
            .map_err(|_| "Thumbnail encoding failed")?;
        "image/jpeg"
    };
    if output.len() > 2 * 1024 * 1024 {
        return Err("Thumbnail output too large".into());
    }
    Ok((kind, Bytes::from(output)))
}

pub async fn render(bytes: Bytes, width: u32) -> Result<(&'static str, Bytes), String> {
    if !valid_width(width) {
        return Err("width must be 80, 160, 320, 640 or 960".into());
    }
    let permit = RESIZERS
        .acquire()
        .await
        .map_err(|_| "Thumbnail service unavailable")?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        resize(bytes, width)
    })
    .await
    .map_err(|_| "Thumbnail task failed")?
}

type Thumbnail = (&'static str, Bytes);

struct Entry {
    value: Thumbnail,
    inserted: Instant,
    used: Instant,
}

/// Completed thumbnails and in-flight coordination have separate, hard bounds.
struct ThumbnailCache {
    entries: Mutex<HashMap<String, Entry>>,
    flights: Mutex<HashMap<String, Weak<AsyncMutex<()>>>>,
    requests: Semaphore,
    capacity: usize,
    ttl: Duration,
}

impl ThumbnailCache {
    fn new(capacity: usize, parallel: usize, ttl: Duration) -> Self {
        assert!(capacity > 0 && parallel > 0);
        Self {
            entries: Mutex::new(HashMap::new()),
            flights: Mutex::new(HashMap::new()),
            requests: Semaphore::new(parallel),
            capacity,
            ttl,
        }
    }

    fn get(&self, key: &str) -> Option<Thumbnail> {
        let mut entries = self.entries.lock().unwrap();
        let entry = entries.get_mut(key)?;
        if entry.inserted.elapsed() >= self.ttl {
            entries.remove(key);
            return None;
        }
        entry.used = Instant::now();
        Some(entry.value.clone())
    }

    fn insert(&self, key: String, value: Thumbnail) {
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|_, entry| entry.inserted.elapsed() < self.ttl);
        while entries.len() >= self.capacity && !entries.contains_key(&key) {
            let oldest = entries
                .iter()
                .min_by_key(|(_, entry)| entry.used)
                .map(|(k, _)| k.clone());
            match oldest {
                Some(k) => {
                    entries.remove(&k);
                }
                None => break,
            }
        }
        let now = Instant::now();
        entries.insert(
            key,
            Entry {
                value,
                inserted: now,
                used: now,
            },
        );
    }

    async fn get_or_fetch<F, Fut>(&self, key: String, fetch: F) -> AppResult<Thumbnail>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = AppResult<Thumbnail>>,
    {
        if let Some(value) = self.get(&key) {
            return Ok(value);
        }
        // Register keys only after admission, so the coordination map cannot
        // grow with unbounded queued misses. No body is read before admission.
        let _permit = tokio::time::timeout(Duration::from_secs(10), self.requests.acquire())
            .await
            .map_err(|_| AppError::ServiceUnavailable("Thumbnail service busy".into()))?
            .map_err(|_| AppError::ServiceUnavailable("Thumbnail service unavailable".into()))?;
        let flight = {
            let mut flights = self.flights.lock().unwrap();
            flights.retain(|_, flight| flight.strong_count() > 0);
            flights
                .get(&key)
                .and_then(Weak::upgrade)
                .unwrap_or_else(|| {
                    let flight = Arc::new(AsyncMutex::new(()));
                    flights.insert(key.clone(), Arc::downgrade(&flight));
                    flight
                })
        };
        // Live coordination is never evicted with completed entries. Dropping
        // a cancelled leader releases this lock and lets a waiter retry.
        let _leader = flight.lock().await;
        if let Some(value) = self.get(&key) {
            return Ok(value);
        }
        let value = fetch().await?;
        self.insert(key, value.clone());
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::{Barrier, mpsc};
    use tokio::task::JoinSet;

    #[test]
    fn decodes_scale_with_the_machine_within_their_bounds() {
        assert_eq!([1, 4, 8, 12, 18, 64].map(resizers), [2, 2, 2, 3, 4, 4]);
    }

    fn value() -> Thumbnail {
        ("image/webp", Bytes::from_static(b"thumbnail"))
    }

    #[test]
    fn validators_distinguish_originals_sizes_and_redeployed_content() {
        let small = variant_key("hash-a", 80);
        let mut headers = HeaderMap::new();
        headers.insert(
            header::IF_NONE_MATCH,
            format!("\"{small}\"").parse().unwrap(),
        );
        assert!(check_not_modified(&headers, &small).is_some());
        assert!(check_not_modified(&headers, &variant_key("hash-a", 160)).is_none());
        assert!(check_not_modified(&headers, &variant_key("hash-b", 80)).is_none());
        assert!(check_not_modified(&headers, "hash-a").is_none());
    }

    #[test]
    fn the_metadata_names_the_file() {
        let e = serde_json::json!({ "metadata": { "thumbnail": "t.png", "image": "i.png" },
                                    "content": [{ "file": "t.png", "hash": "bafyt" }, { "key": "i.png", "hash": "bafyi" }] });
        assert_eq!(file_hash(&e, "thumbnail").as_deref(), Some("bafyt"));
        assert_eq!(file_hash(&e, "image").as_deref(), Some("bafyi"));
        assert!(file_hash(&e, "missing").is_none());
    }

    #[test]
    fn thumbnail_preserves_transparency_and_bounds_dimensions() {
        let image = image::RgbaImage::from_pixel(1600, 900, image::Rgba([32, 64, 96, 128]));
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, ImageFormat::Png).unwrap();
        let (kind, output) = resize(Bytes::from(bytes.into_inner()), 320).unwrap();
        let decoded = image::load_from_memory(&output).unwrap();
        assert!(matches!(kind, "image/png" | "image/webp"));
        assert_eq!((decoded.width(), decoded.height()), (320, 180));
        assert_eq!(decoded.to_rgba8().get_pixel(0, 0).0[3], 128);
    }

    #[test]
    fn tiff_sources_become_browser_readable_thumbnails() {
        let image = image::RgbImage::from_pixel(228, 160, image::Rgb([32, 64, 96]));
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, ImageFormat::Tiff).unwrap();
        let (kind, output) = resize(Bytes::from(bytes.into_inner()), 640).unwrap();
        assert_eq!(kind, "image/jpeg");
        assert_eq!(image::load_from_memory(&output).unwrap().width(), 228);
    }

    #[test]
    fn invalid_images_and_unbounded_sizes_are_rejected() {
        assert!(resize(Bytes::from_static(b"<svg></svg>"), 320).is_err());
        assert!(valid_width(80) && valid_width(160) && valid_width(640));
        assert!(!valid_width(0) && !valid_width(8192));
    }

    #[test]
    fn opaque_rgba_and_webp_sources_get_compact_jpeg_thumbnails() {
        for format in [ImageFormat::Png, ImageFormat::WebP] {
            let image = image::RgbaImage::from_pixel(1600, 900, image::Rgba([32, 64, 96, 255]));
            let mut bytes = Cursor::new(Vec::new());
            image.write_to(&mut bytes, format).unwrap();
            let (kind, output) = resize(Bytes::from(bytes.into_inner()), 320).unwrap();
            let decoded = image::load_from_memory(&output).unwrap();
            assert_eq!(kind, "image/jpeg");
            assert_eq!((decoded.width(), decoded.height()), (320, 180));
            assert!(output.len() < 10_000);
        }
    }

    #[tokio::test]
    async fn concurrent_completions_cannot_overfill_the_cache() {
        let cache = Arc::new(ThumbnailCache::new(2, 8, Duration::from_secs(60)));
        let release = Arc::new(Semaphore::new(0));
        let (started, mut started_rx) = mpsc::unbounded_channel();
        let mut tasks = JoinSet::new();
        for key in 0..24 {
            let (cache, release, started) = (cache.clone(), release.clone(), started.clone());
            tasks.spawn(async move {
                cache
                    .get_or_fetch(key.to_string(), || async {
                        started.send(()).unwrap();
                        let _release = release.acquire().await.unwrap();
                        Ok(value())
                    })
                    .await
                    .unwrap();
            });
        }
        for _ in 0..8 {
            started_rx.recv().await.unwrap();
        }
        assert_eq!(cache.flights.lock().unwrap().len(), 8);
        release.add_permits(8);
        while let Some(result) = tasks.join_next().await {
            result.unwrap();
            assert!(cache.entries.lock().unwrap().len() <= 2);
            assert!(cache.flights.lock().unwrap().len() <= 8);
        }
        assert_eq!(cache.entries.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn duplicate_misses_share_one_fetch_and_pending_keys_stay_bounded() {
        let cache = Arc::new(ThumbnailCache::new(2, 2, Duration::from_secs(60)));
        let start = Arc::new(Barrier::new(9));
        let calls = Arc::new(AtomicUsize::new(0));
        let release = Arc::new(Semaphore::new(0));
        let (started, mut started_rx) = mpsc::unbounded_channel();
        let mut tasks = JoinSet::new();
        for _ in 0..8 {
            let (cache, start, calls, release, started) = (
                cache.clone(),
                start.clone(),
                calls.clone(),
                release.clone(),
                started.clone(),
            );
            tasks.spawn(async move {
                start.wait().await;
                cache
                    .get_or_fetch("same".into(), || async {
                        calls.fetch_add(1, Ordering::SeqCst);
                        started.send(()).unwrap();
                        let _release = release.acquire().await.unwrap();
                        Ok(value())
                    })
                    .await
                    .unwrap()
            });
        }
        start.wait().await;
        started_rx.recv().await.unwrap();
        assert_eq!(cache.flights.lock().unwrap().len(), 1);
        release.add_permits(1);
        while let Some(result) = tasks.join_next().await {
            assert_eq!(result.unwrap(), value());
            assert!(cache.flights.lock().unwrap().len() <= 2);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn cancelled_leaders_allow_a_waiter_to_retry() {
        let cache = Arc::new(ThumbnailCache::new(2, 2, Duration::from_secs(60)));
        let (started, mut started_rx) = mpsc::unbounded_channel();
        let leader_cache = cache.clone();
        let leader = tokio::spawn(async move {
            leader_cache
                .get_or_fetch("same".into(), || async {
                    started.send(()).unwrap();
                    std::future::pending().await
                })
                .await
        });
        started_rx.recv().await.unwrap();
        let waiter = cache.get_or_fetch("same".into(), || async { Ok(value()) });
        tokio::pin!(waiter);
        assert!(futures::poll!(&mut waiter).is_pending());
        leader.abort();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), waiter)
                .await
                .unwrap()
                .unwrap(),
            value()
        );
    }

    #[tokio::test]
    async fn an_items_thumbnail_and_image_are_served_by_pointer() {
        use crate::tests::support::TestNode;
        use tower::ServiceExt;
        let Some(node) = TestNode::with_db("item-thumbnail").await else {
            return;
        };
        let tag = uuid::Uuid::new_v4().simple().to_string();
        let pointer = format!(
            "urn:decentraland:matic:collections-v2:0x{}:1",
            &tag[..40.min(tag.len())]
        );
        let image = image::RgbaImage::from_pixel(400, 200, image::Rgba([10, 20, 30, 255]));
        let mut png = Cursor::new(Vec::new());
        image.write_to(&mut png, ImageFormat::Png).unwrap();
        let png = Bytes::from(png.into_inner());
        let thumb_hash = crate::auth::hash_bytes_v1(&png);
        node.store(&thumb_hash, png.clone()).await;
        let entity_id = format!("bafyitem{tag}");
        let id: i32 = sqlx::query_scalar(
            "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain) \
             VALUES ('0x1', 'v3', 'wearable', $1, $2, to_timestamp(1700000000), ARRAY[$3], timezone('UTC', now()), '[]'::json) RETURNING id",
        )
        .bind(&entity_id)
        .bind(serde_json::json!({ "v": { "thumbnail": "thumbnail.png", "image": "image.png" } }))
        .bind(&pointer)
        .fetch_one(&node.state.pool)
        .await
        .unwrap();
        for key in ["thumbnail.png", "image.png"] {
            sqlx::query("INSERT INTO content.content_files (deployment, content_hash, key) VALUES ($1, $2, $3)").bind(id).bind(&thumb_hash).bind(key).execute(&node.state.pool).await.unwrap();
        }
        sqlx::query("INSERT INTO content.active_pointers (pointer, entity_id, entity_type) VALUES ($1, $2, 'wearable')").bind(&pointer).bind(&entity_id).execute(&node.state.pool).await.unwrap();

        let get = |uri: String| {
            let router = node.router();
            async move {
                let resp = router
                    .oneshot(axum::http::Request::get(uri).body(Body::empty()).unwrap())
                    .await
                    .unwrap();
                let status = resp.status();
                let ct = resp
                    .headers()
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_string();
                let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
                    .await
                    .unwrap();
                (status, ct, bytes)
            }
        };
        for prefix in [
            "/queries/items",
            "/content/queries/items",
            "/lambdas/collections/contents",
        ] {
            let (status, ct, bytes) = get(format!("{prefix}/{pointer}/thumbnail")).await;
            assert_eq!(
                (status, ct.as_str()),
                (StatusCode::OK, "image/png"),
                "{prefix}"
            );
            assert_eq!(bytes, png, "{prefix}");
            let (status, _, bytes) = get(format!("{prefix}/{pointer}/image")).await;
            assert_eq!((status, bytes), (StatusCode::OK, png.clone()), "{prefix}");
        }
        let (status, ct, bytes) = get(format!("/queries/items/{pointer}/thumbnail?width=80")).await;
        assert_eq!((status, ct.as_str()), (StatusCode::OK, "image/jpeg"));
        assert_eq!(image::load_from_memory(&bytes).unwrap().width(), 80);
        let (status, _, _) = get(format!("/queries/items/{pointer}/thumbnail?width=81")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (status, _, bytes) = get(
            "/queries/items/urn:decentraland:matic:collections-v2:0xnone:1/thumbnail".to_string(),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            bytes,
            Bytes::from_static(br#"{"error":"Entity not found."}"#)
        );
        node.state.denylist.add(&thumb_hash);
        let (status, _, bytes) = get(format!("/queries/items/{pointer}/image")).await;
        assert_eq!(
            (status, bytes),
            (
                StatusCode::NOT_FOUND,
                Bytes::from_static(br#"{"error":"Entity has no image."}"#)
            )
        );
        node.state.denylist.add(&entity_id);
        let (status, _, _) =
            get(format!("/lambdas/collections/contents/{pointer}/thumbnail")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn insertion_evicts_the_least_recently_used_completed_entry() {
        let cache = ThumbnailCache::new(2, 2, Duration::from_secs(60));
        cache.insert("a".into(), value());
        cache.insert("b".into(), value());
        assert!(cache.get("a").is_some());
        cache.insert("c".into(), value());
        assert!(cache.get("a").is_some());
        assert!(cache.get("b").is_none());
        assert!(cache.get("c").is_some());
    }
}
