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
use axum::extract::{Path, RawQuery, State};
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use image::codecs::jpeg::JpegEncoder;
use image::metadata::Orientation;
use image::{ImageFormat, ImageReader, Limits};
use serde_json::Value;
use tokio::sync::{Mutex as AsyncMutex, Semaphore};

use super::errors::{AppError, AppResult};
use super::file_type::Window;
use super::formatters::{
    ParsedRange, check_not_modified, content_file_headers, parse_range_header,
};
use super::get_content::{content_type, set_content_type, x_accel_redirect_path};
use super::queries::active_entity_docs_by_pointers;
use super::query_params::{parse_query_string, qs_get_string};
use super::{ContentState, SharedState};

/// `?width=` when it names a served size; any other value is ignored, as
/// upstream ignores the parameter.
fn requested_width(query: Option<&str>) -> Option<u32> {
    qs_get_string(&parse_query_string(query.unwrap_or("")), "width")
        .and_then(|w| w.parse::<u32>().ok())
        .filter(|w| valid_width(*w))
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
    RawQuery(query): RawQuery,
    method: Method,
    headers: HeaderMap,
) -> AppResult<Response> {
    let width = requested_width(query.as_deref());
    let entity = entity_by_pointer(&state, &pointer).await?;
    let hash = file_hash(&entity, "thumbnail")
        .ok_or_else(|| AppError::not_found("Entity has no thumbnail."))?;
    if state.denylist.contains(&hash) {
        return Err(AppError::not_found("Entity has no thumbnail."));
    }
    if let Some(width) = width {
        return serve_variant(&state, &hash, width, &method, &headers).await;
    }
    if let Some(h) = check_not_modified(&headers, &hash) {
        return Ok(not_modified(h));
    }
    serve_content_blob(&state, &hash, &method, &headers, "Entity has no thumbnail.").await
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
    serve_content_blob(&state, &hash, &method, &headers, "Entity has no image.").await
}

fn apply(response: &mut Response, headers: &[(&'static str, String)]) {
    let resp_headers = response.headers_mut();
    for (name, value) in headers {
        if let Ok(hv) = value.parse() {
            resp_headers.insert(*name, hv);
        }
    }
}

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

/// `missing` is upstream's 404 message for a file the store does not hold.
async fn serve_content_blob(
    state: &ContentState,
    hash: &str,
    method: &Method,
    headers: &HeaderMap,
    missing: &'static str,
) -> AppResult<Response> {
    let mut opened = open_blob(state, hash)
        .await?
        .ok_or_else(|| AppError::not_found(missing))?;
    let range_header = headers.get("range").and_then(|v| v.to_str().ok());
    let total = opened.size;
    let range = parse_range_header(range_header, Some(total));
    let mut base_headers = content_file_headers(hash, Some(total));
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
            // Upstream sniffs the ranged stream it serves.
            let mime =
                content_type(state, hash, &mut Window::range(&mut opened, start, end + 1)).await?;
            set_content_type(&mut base_headers, mime);
            let body = if *method == Method::HEAD {
                Body::empty()
            } else {
                opened.into_range_body(start, end).await?
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
            let mut window = Window::new(&mut opened, total);
            set_content_type(
                &mut base_headers,
                content_type(state, hash, &mut window).await?,
            );
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
                let head = window.into_head().await?;
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
                .ok_or_else(|| AppError::not_found("Entity has no thumbnail."))?;
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

#[derive(Clone, Copy)]
enum Rendition {
    Thumbnail(u32),
    EventPoster,
}

/// Borrows the EXIF chunk from the bounded source; no metadata buffer is allocated.
fn webp_orientation(bytes: &[u8]) -> Option<Orientation> {
    let riff = bytes.strip_prefix(b"RIFF")?;
    let (size, payload) = riff.split_first_chunk::<4>()?;
    let mut chunks = payload
        .get(..u32::from_le_bytes(*size) as usize)?
        .strip_prefix(b"WEBP")?;
    while !chunks.is_empty() {
        let (tag, rest) = chunks.split_first_chunk::<4>()?;
        let (size, rest) = rest.split_first_chunk::<4>()?;
        let size = u32::from_le_bytes(*size) as usize;
        let (data, rest) = rest.split_at_checked(size)?;
        if tag == b"EXIF" {
            return Orientation::from_exif_chunk(data.strip_prefix(b"Exif\0\0").unwrap_or(data));
        }
        chunks = rest.get(size % 2..)?;
    }
    None
}

fn resize(bytes: Bytes, rendition: Rendition) -> Result<(&'static str, Bytes), String> {
    let mut reader = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|_| "Unrecognized image")?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    const DECODE_ERROR: &str = "Image could not be decoded within limits";
    let (width, orientation) = match rendition {
        Rendition::Thumbnail(width) => {
            // GIF decoding is enabled for native poster stills only.
            if reader.format() == Some(ImageFormat::Gif) {
                return Err(DECODE_ERROR.into());
            }
            (width, None)
        }
        Rendition::EventPoster => (960, webp_orientation(&bytes)),
    };
    let image = reader.decode().map_err(|_| DECODE_ERROR)?;
    // Never enlarge small source images: larger encoded files add no detail.
    let bound = width.min(image.width().max(image.height()));
    let mut image = image.thumbnail(bound, bound);
    if let Some(orientation) = orientation {
        image.apply_orientation(orientation);
    }
    let mut output = Vec::new();
    let transparent =
        image.color().has_alpha() && image.to_rgba8().pixels().any(|pixel| pixel.0[3] < 255);
    let kind = if transparent {
        image
            .write_to(&mut Cursor::new(&mut output), ImageFormat::Png)
            .map_err(|_| "Thumbnail encoding failed")?;
        let mut kind = "image/png";
        if matches!(rendition, Rendition::Thumbnail(_)) {
            let mut webp = Cursor::new(Vec::new());
            image
                .write_to(&mut webp, ImageFormat::WebP)
                .map_err(|_| "Thumbnail encoding failed")?;
            if webp.get_ref().len() < output.len() {
                output = webp.into_inner();
                kind = "image/webp";
            }
        }
        kind
    } else {
        JpegEncoder::new_with_quality(&mut output, 80)
            .encode_image(&image.to_rgb8())
            .map_err(|_| "Thumbnail encoding failed")?;
        "image/jpeg"
    };
    const MAX_OUTPUT_BYTES: usize = 2 * 1024 * 1024;
    if output.len() > MAX_OUTPUT_BYTES && transparent && matches!(rendition, Rendition::EventPoster)
    {
        // A 640 × 640 RGBA8 still fits below 2 MiB even before PNG compression.
        output.clear();
        image
            .thumbnail(640, 640)
            .write_to(&mut Cursor::new(&mut output), ImageFormat::Png)
            .map_err(|_| "Thumbnail encoding failed")?;
    }
    if output.len() > MAX_OUTPUT_BYTES {
        return Err("Thumbnail output too large".into());
    }
    Ok((kind, Bytes::from(output)))
}

pub async fn render(bytes: Bytes, width: u32) -> Result<(&'static str, Bytes), String> {
    if !valid_width(width) {
        return Err("width must be 80, 160, 320, 640 or 960".into());
    }
    render_bounded(bytes, Rendition::Thumbnail(width)).await
}

/// Encodes a WebP or GIF event cover as a JPEG or alpha-preserving PNG for native image readers.
pub async fn render_event_poster(bytes: Bytes) -> Result<(&'static str, Bytes), String> {
    render_bounded(bytes, Rendition::EventPoster).await
}

async fn render_bounded(
    bytes: Bytes,
    rendition: Rendition,
) -> Result<(&'static str, Bytes), String> {
    let permit = RESIZERS
        .acquire()
        .await
        .map_err(|_| "Thumbnail service unavailable")?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        resize(bytes, rendition)
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
        let (kind, output) =
            resize(Bytes::from(bytes.into_inner()), Rendition::Thumbnail(320)).unwrap();
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
        let (kind, output) =
            resize(Bytes::from(bytes.into_inner()), Rendition::Thumbnail(640)).unwrap();
        assert_eq!(kind, "image/jpeg");
        assert_eq!(image::load_from_memory(&output).unwrap().width(), 228);
    }

    #[test]
    fn invalid_images_and_unbounded_sizes_are_rejected() {
        assert!(
            resize(
                Bytes::from_static(b"<svg></svg>"),
                Rendition::Thumbnail(320)
            )
            .is_err()
        );
        assert!(valid_width(80) && valid_width(160) && valid_width(640));
        assert!(!valid_width(0) && !valid_width(8192));
    }

    #[test]
    fn opaque_rgba_and_webp_sources_get_compact_jpeg_thumbnails() {
        for format in [ImageFormat::Png, ImageFormat::WebP] {
            let image = image::RgbaImage::from_pixel(1600, 900, image::Rgba([32, 64, 96, 255]));
            let mut bytes = Cursor::new(Vec::new());
            image.write_to(&mut bytes, format).unwrap();
            let (kind, output) =
                resize(Bytes::from(bytes.into_inner()), Rendition::Thumbnail(320)).unwrap();
            let decoded = image::load_from_memory(&output).unwrap();
            assert_eq!(kind, "image/jpeg");
            assert_eq!((decoded.width(), decoded.height()), (320, 180));
            assert!(output.len() < 10_000);
        }
    }

    #[tokio::test]
    async fn poster_applies_webp_orientation_and_preserves_translucent_pixels() {
        use image::ImageEncoder;
        use image::codecs::webp::WebPEncoder;

        let mut bytes = Vec::new();
        let mut encoder = WebPEncoder::new_lossless(&mut bytes);
        // Little-endian TIFF with one SHORT orientation entry (6 = rotate 90 degrees).
        encoder
            .set_exif_metadata(vec![
                0x49, 0x49, 42, 0, 8, 0, 0, 0, 1, 0, 0x12, 1, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0, 0, 0,
                0, 0,
            ])
            .unwrap();
        let pixels = image::RgbaImage::from_fn(12, 4, |x, _| {
            if x < 6 {
                image::Rgba([10, 24, 200, 128])
            } else {
                image::Rgba([188, 4, 10, 255])
            }
        });
        encoder
            .write_image(pixels.as_raw(), 12, 4, image::ExtendedColorType::Rgba8)
            .unwrap();
        assert_eq!(webp_orientation(&bytes[..bytes.len() - 1]), None);
        let (mime, body) = render_event_poster(bytes.into()).await.unwrap();
        assert_eq!(mime, "image/png");
        let rendered = image::load_from_memory(&body).unwrap().into_rgba8();
        assert_eq!(rendered.dimensions(), (4, 12));
        assert_eq!(rendered.get_pixel(0, 0), pixels.get_pixel(0, 3));
        assert_eq!(rendered.get_pixel(3, 11), pixels.get_pixel(11, 0));
    }

    #[tokio::test]
    async fn poster_renders_the_first_gif_frame_within_the_cover_size() {
        use image::codecs::gif::GifEncoder;

        let mut bytes = Vec::new();
        let frames = [[220, 30, 10, 255], [10, 30, 220, 255]].map(|color| {
            image::Frame::new(image::RgbaImage::from_pixel(20, 1000, image::Rgba(color)))
        });
        GifEncoder::new(&mut bytes).encode_frames(frames).unwrap();
        let source = Bytes::from(bytes);
        assert!(render(source.clone(), 320).await.is_err());
        let (mime, body) = render_event_poster(source).await.unwrap();
        assert_eq!(mime, "image/jpeg");
        let rendered = image::load_from_memory(&body).unwrap().into_rgb8();
        assert_eq!(rendered.height(), 960);
        assert!(rendered.width().abs_diff(19) <= 1);
        let [red, _, blue] = rendered.get_pixel(0, 0).0;
        assert!(red > 200 && blue < 30);
    }

    #[tokio::test]
    async fn poster_renders_the_first_animated_webp_frame() {
        use base64::Engine;

        // Two opaque 2 × 2 lossless frames, red followed by blue.
        let animation = base64::engine::general_purpose::STANDARD
            .decode(concat!(
                "UklGRoQAAABXRUJQVlA4WAoAAAACAAAAAQAAAQAAQU5JTQYAAAAAAAAAAABBTk1GKAAAAAAAAAAA",
                "AAEAAAEAAGQAAAJWUDhMDwAAAC8BQAAABxD9j/4HIqL/AQBBTk1GKAAAAAAAAAAAAAEAAAEAAGQA",
                "AABWUDhMDwAAAC8BQAAABxDR//4HIqL/AQA="
            ))
            .unwrap();
        let (mime, body) = render_event_poster(animation.into()).await.unwrap();
        let frame = image::load_from_memory(&body).unwrap().to_rgb8();
        assert_eq!(mime, "image/jpeg");
        assert_eq!(frame.dimensions(), (2, 2));
        assert!(frame.pixels().all(|p| p[0] > 200 && p[2] < 30));
    }

    #[tokio::test]
    async fn noisy_translucent_poster_fits_the_output_limit_as_png() {
        use rand::{RngExt, SeedableRng, rngs::StdRng};

        let mut rng = StdRng::seed_from_u64(2062);
        let pixels = image::RgbaImage::from_fn(960, 960, |_, _| {
            let [r, g, b] = rng.random::<[u8; 3]>();
            image::Rgba([r, g, b, 128])
        });
        let mut source = Cursor::new(Vec::new());
        pixels.write_to(&mut source, ImageFormat::WebP).unwrap();
        let (mime, body) = render_event_poster(source.into_inner().into())
            .await
            .unwrap();
        assert_eq!(mime, "image/png");
        assert!(body.len() <= 2 * 1024 * 1024);
        let frame = image::load_from_memory(&body).unwrap().to_rgba8();
        assert_eq!(frame.dimensions(), (640, 640));
        assert_eq!(frame.get_pixel(320, 320)[3], 128);
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

    /// CS-18: a file the store does not hold is upstream's "has no" 404.
    #[tokio::test]
    async fn a_missing_blob_answers_the_routes_message() {
        use crate::tests::support::{TEST_CID, TestNode};
        let node = TestNode::without_db("blob-missing").await;
        for message in ["Entity has no thumbnail.", "Entity has no image."] {
            let err = serve_content_blob(
                &node.state,
                TEST_CID,
                &Method::GET,
                &HeaderMap::new(),
                message,
            )
            .await
            .unwrap_err();
            assert_eq!(err.to_string(), message);
        }
    }

    #[tokio::test]
    async fn a_blob_range_streams_only_the_window() {
        use crate::tests::support::{TEST_CID, TestNode};
        let node = TestNode::without_db("blob-range").await;
        // A HEIC file, with an AVIF `ftyp` box where the range starts: upstream sniffs the
        // stream it serves, the whole file or the range.
        let mut blob = vec![0u8; 4096];
        for (i, b) in blob.iter_mut().enumerate() {
            *b = (i % 251) as u8;
        }
        blob[..12].copy_from_slice(b"\0\0\0\x18ftypheic");
        blob[100..112].copy_from_slice(b"\0\0\0\x18ftypavif");
        let blob = Bytes::from(blob);
        node.store(TEST_CID, blob.clone()).await;
        for method in [Method::GET, Method::HEAD] {
            let resp =
                serve_content_blob(&node.state, TEST_CID, &method, &HeaderMap::new(), "missing")
                    .await
                    .unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
            assert_eq!(resp.headers()[header::CONTENT_TYPE], "image/heic");
            let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .unwrap();
            let expected = if method == Method::HEAD {
                Bytes::new()
            } else {
                blob.clone()
            };
            assert_eq!(body, expected, "{method}");
        }
        let mut headers = HeaderMap::new();
        headers.insert(header::RANGE, "bytes=100-4999".parse().unwrap());

        for method in [Method::GET, Method::HEAD] {
            let resp = serve_content_blob(&node.state, TEST_CID, &method, &headers, "missing")
                .await
                .unwrap();
            let get = |name| resp.headers()[name].to_str().unwrap().to_string();
            assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
            assert_eq!(get(header::CONTENT_TYPE), "image/avif");
            assert_eq!(get(header::CONTENT_RANGE), "bytes 100-4095/4096");
            assert_eq!(get(header::CONTENT_LENGTH), "3996");
            let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .unwrap();
            let expected = if method == Method::HEAD {
                Bytes::new()
            } else {
                blob.slice(100..)
            };
            assert_eq!(body, expected, "{method}");
        }
    }

    /// An XML image is served as `application/octet-stream`, as on `?includeMimeType`.
    #[tokio::test]
    async fn an_xml_blob_is_served_as_octet_stream() {
        use crate::tests::support::{TEST_CID, TestNode};
        let node = TestNode::without_db("blob-xml").await;
        let svg = b"<?xml version=\"1.0\"?><svg xmlns=\"http://www.w3.org/2000/svg\"/>";
        node.store(TEST_CID, Bytes::from_static(svg)).await;
        for method in [Method::GET, Method::HEAD] {
            let resp =
                serve_content_blob(&node.state, TEST_CID, &method, &HeaderMap::new(), "missing")
                    .await
                    .unwrap();
            assert_eq!(
                resp.headers()[header::CONTENT_TYPE],
                "application/octet-stream",
                "{method}"
            );
        }
    }

    /// The image and thumbnail routes keep a sniffed type per window, as `/contents/{hash}` does,
    /// and serve every answer sandboxed with `nosniff`.
    #[tokio::test]
    async fn a_blob_keeps_its_sniffed_type_and_is_sandboxed() {
        use crate::tests::support::{TEST_CID, TestNode};
        let node = TestNode::without_db("blob-kept").await;
        // Past `SAMPLE`, so a kept GET serves the head and the rest of the file in one body.
        let file = |head: &[u8]| {
            let mut bytes: Vec<u8> = (0..5000).map(|i| (i % 251) as u8).collect();
            bytes[..head.len()].copy_from_slice(head);
            Bytes::from(bytes)
        };
        node.store(TEST_CID, file(b"GIF89a\x01\x00\x01\x00")).await;
        let mut ranged = HeaderMap::new();
        ranged.insert(header::RANGE, "bytes=0-1".parse().unwrap());
        let serve = |method: Method, headers: HeaderMap| {
            let state = node.state.clone();
            async move {
                serve_content_blob(&state, TEST_CID, &method, &headers, "missing")
                    .await
                    .unwrap()
            }
        };
        let first = serve(Method::HEAD, HeaderMap::new()).await;
        assert_eq!(first.headers()[header::CONTENT_TYPE], "image/gif");

        // Other bytes of the same size under the same key (a test can store them; a real key
        // cannot change).
        let bmp = file(b"BM\x36\x00\x00\x00");
        node.store(TEST_CID, bmp.clone()).await;
        for (method, headers, mime) in [
            (Method::GET, HeaderMap::new(), "image/gif"),
            (Method::HEAD, HeaderMap::new(), "image/gif"),
            (Method::GET, ranged, "image/bmp"),
        ] {
            let resp = serve(method.clone(), headers).await;
            assert_eq!(resp.headers()[header::CONTENT_TYPE], mime, "{method}");
            assert_eq!(resp.headers()["content-security-policy"], "sandbox");
            assert_eq!(resp.headers()[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
            let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .unwrap();
            if method == Method::GET && mime == "image/gif" {
                assert_eq!(body, bmp, "the whole body after a kept type");
            }
        }
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
        // CS-18: a width this node does not serve is ignored, as upstream
        // ignores the parameter: the stored file comes back.
        for width in ["81", "abc", ""] {
            let (status, ct, bytes) =
                get(format!("/queries/items/{pointer}/thumbnail?width={width}")).await;
            assert_eq!(
                (status, ct.as_str()),
                (StatusCode::OK, "image/png"),
                "{width}"
            );
            assert_eq!(bytes, png, "{width}");
        }
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
