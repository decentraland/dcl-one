//! `GET /convert` and `/media/convert?url=<http(s) url>[&width=]`: the
//! stand-in for `metamorph-api.decentraland.org` the web tier's image
//! derivatives call. A URL under this node's own `/content/contents/` is
//! read from the blob store. Any other source is fetched only when the node
//! allows remote sources (`MEDIA_CONVERT_REMOTE_SOURCES`, the mirror
//! posture's), through the process's guarded client ([`Outbound`]: every
//! hop resolved and pinned to a public address, redirects re-checked);
//! otherwise it is refused with 403 and nothing is sent. A content file
//! (`.../content/contents/<cid>` on any host) is read from the blob store
//! first, filled through read-through when it is on, and only then fetched
//! from the host the URL names: peers rate-limit an address that asks them
//! for many uncached files, and the sync sources are asked once per file.
//! A source is capped at 64 MiB (8 MiB for a thumbnail source) and answered
//! with its status and content type, `public, max-age=86400`.
//! `width` (80, 160, 320, 640, 960) renders a bounded derivative with the
//! content routes' renderer. At most [`THUMBNAIL_SLOTS`] thumbnail sources
//! are in hand at once (the memory bound); up to [`THUMBNAIL_QUEUE`] more
//! wait in line for [`THUMBNAIL_WAIT`], so a page's burst is served rather
//! than refused, and a request past the line, or one that waited out its
//! turn, answers 503 `Retry-After: 1`. Successful answers up to 2 MiB are
//! memoized for 5 minutes per `(width, url)`, 256 of them, and a memoized
//! answer never waits for a slot; concurrent misses of one key share one
//! fetch.

use std::time::Duration;

use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use tokio::sync::{Semaphore, SemaphorePermit};

use crate::http::read_body_capped;
use crate::shared::cache::TtlMap;

use super::{ConvertSources, SharedState};

const MAX_BODY_BYTES: usize = 64 * 1024 * 1024;
const MAX_THUMBNAIL_SOURCE_BYTES: usize = 8 * 1024 * 1024;
pub const CONVERT_CACHE_TTL: Duration = Duration::from_secs(300);
pub const CONVERT_CACHE_MAX_BODY: usize = 2 * 1024 * 1024;
const CONVERT_CACHE_MAX_ENTRIES: usize = 256;
/// Thumbnail sources in hand at once, each up to 8 MiB.
pub const THUMBNAIL_SLOTS: usize = 16;
/// Thumbnail requests waiting for a slot beyond those holding one.
pub const THUMBNAIL_QUEUE: usize = 256;
/// The longest a queued thumbnail request waits for its slot.
pub const THUMBNAIL_WAIT: Duration = Duration::from_secs(20);

/// One converted answer: status, content type, body.
pub type Converted = (u16, String, Bytes);

pub fn cache() -> TtlMap<String, Converted> {
    TtlMap::bounded(CONVERT_CACHE_TTL, CONVERT_CACHE_MAX_ENTRIES)
}

pub fn slots() -> Slots {
    Slots::new(THUMBNAIL_SLOTS, THUMBNAIL_QUEUE)
}

#[derive(Deserialize)]
pub struct ConvertParams {
    pub url: String,
    pub width: Option<u32>,
}

fn build_response(status: StatusCode, content_type: &str, body: Bytes) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(content_type)
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=86400"),
    );
    (status, headers, body).into_response()
}

/// A bounded line in front of a bounded number of slots.
pub struct Slots {
    slots: Semaphore,
    /// Slots plus line: a request that cannot take one is refused at once.
    admitted: Semaphore,
}

impl Slots {
    pub fn new(slots: usize, queue: usize) -> Self {
        Self {
            slots: Semaphore::new(slots),
            admitted: Semaphore::new(slots + queue),
        }
    }

    /// A place in line; `None` when the slots and the line are full.
    fn admit(&self) -> Option<SemaphorePermit<'_>> {
        self.admitted.try_acquire().ok()
    }

    /// A slot, waited for at most `wait`.
    async fn slot(&self, wait: Duration) -> Option<SemaphorePermit<'_>> {
        tokio::time::timeout(wait, self.slots.acquire())
            .await
            .ok()?
            .ok()
    }

    /// A slot, after waiting in line at most `wait`; `None` when the line is
    /// full or the wait ran out.
    #[cfg(test)]
    async fn acquire(&self, wait: Duration) -> Option<(SemaphorePermit<'_>, SemaphorePermit<'_>)> {
        let admitted = self.admit()?;
        let slot = self.slot(wait).await?;
        Some((admitted, slot))
    }
}

fn busy_response() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [(header::RETRY_AFTER, HeaderValue::from_static("1"))],
        "Thumbnail service busy",
    )
        .into_response()
}

fn cached_response((status, content_type, body): Converted) -> Response {
    build_response(
        StatusCode::from_u16(status).unwrap_or(StatusCode::OK),
        &content_type,
        body,
    )
}

pub async fn convert(State(state): State<SharedState>, Query(p): Query<ConvertParams>) -> Response {
    if p.width
        .is_some_and(|width| !crate::content::entity_thumbnail::valid_width(width))
    {
        return (
            StatusCode::BAD_REQUEST,
            "width must be 80, 160, 320, 640 or 960",
        )
            .into_response();
    }
    let source = match reqwest::Url::parse(&p.url) {
        Ok(u) if matches!(u.scheme(), "http" | "https") => u,
        _ => return (StatusCode::BAD_REQUEST, "url must be http(s)").into_response(),
    };
    let source = match state.convert_sources.own_content_hash(&source) {
        Some(hash) => Source::Own(hash),
        None if state.convert_sources.remote => Source::Remote {
            hash: ConvertSources::content_hash(&source),
            url: source,
        },
        None => {
            return (
                StatusCode::FORBIDDEN,
                "this node converts only its own /content/contents/ files",
            )
                .into_response();
        }
    };
    let key = p.width.map_or_else(
        || p.url.clone(),
        |width| format!("thumbnail:{width}:{}", p.url),
    );
    // Only a cacheable success is an Ok: an error or an oversized body is
    // answered to its own caller and the next caller fetches again.
    let outcome = state
        .convert_cache
        .get_or_fetch(key, || fetch_and_convert(&state, source, p.width))
        .await;
    match outcome {
        Ok(hit) => cached_response(hit),
        Err(response) => response,
    }
}

enum Source {
    /// A blob of this node's store.
    Own(String),
    /// A public URL; `hash` when it names a content file.
    Remote {
        url: reqwest::Url,
        hash: Option<String>,
    },
}

async fn fetch_and_convert(
    state: &super::MediaState,
    source: Source,
    width: Option<u32>,
) -> Result<Converted, Response> {
    // A thumbnail waits for a read-through fill or a rate-limited source with only a place in
    // line; it takes a slot (the memory bound) when the source's bytes are read, until the render.
    let slots = width.map(|_| &state.convert_slots);
    let _line = match slots {
        Some(slots) => Some(slots.admit().ok_or_else(busy_response)?),
        None => None,
    };
    let max_bytes = if width.is_some() {
        MAX_THUMBNAIL_SOURCE_BYTES
    } else {
        MAX_BODY_BYTES
    };
    let ready = match source {
        Source::Own(hash) => Ready::Stored(
            open_stored(state, &hash, max_bytes)
                .await?
                .ok_or_else(|| (StatusCode::NOT_FOUND, "not found").into_response())?,
        ),
        Source::Remote { url, hash } => {
            let stored = match hash {
                Some(hash) => open_stored(state, &hash, max_bytes).await?,
                None => None,
            };
            match stored {
                Some(opened) => Ready::Stored(opened),
                None => Ready::Remote(send_remote(state, url).await?),
            }
        }
    };
    let _slot = match slots {
        Some(slots) => Some(slots.slot(THUMBNAIL_WAIT).await.ok_or_else(busy_response)?),
        None => None,
    };
    let (status, content_type, body) = match ready {
        Ready::Stored(opened) => read_stored(opened).await?,
        Ready::Remote(upstream) => read_remote(upstream, max_bytes).await?,
    };
    if let Some(width) = width.filter(|_| status.is_success()) {
        return match crate::content::entity_thumbnail::render(body, width).await {
            Ok((kind, bytes)) => cacheable(status, kind, bytes),
            Err(message) => Err((StatusCode::UNPROCESSABLE_ENTITY, message).into_response()),
        };
    }
    if status.is_success() {
        return cacheable(status, &content_type, body);
    }
    Err(build_response(status, &content_type, body))
}

/// A source whose bytes are next to be read.
enum Ready {
    Stored(crate::storage::OpenedContent),
    Remote(reqwest::Response),
}

/// A content file from this node's store, filled through read-through first
/// when the store lacks it; `None` when neither holds it.
async fn open_stored(
    state: &super::MediaState,
    hash: &str,
    max_bytes: usize,
) -> Result<Option<crate::storage::OpenedContent>, Response> {
    let sources = &state.convert_sources;
    let Some(storage) = sources.storage.as_ref() else {
        return Ok(None);
    };
    if storage.denylist().contains(hash) {
        return Err((StatusCode::NOT_FOUND, "not found").into_response());
    }
    let mut opened = storage.open(hash).await.ok().flatten();
    if opened.is_none()
        && let Some(read_through) = &sources.read_through
        && read_through.fill_content(hash).await
    {
        opened = storage.open(hash).await.ok().flatten();
    }
    let Some(opened) = opened else {
        return Ok(None);
    };
    if opened.size > max_bytes as u64 {
        return Err((StatusCode::PAYLOAD_TOO_LARGE, "source too large").into_response());
    }
    Ok(Some(opened))
}

async fn read_stored(
    mut opened: crate::storage::OpenedContent,
) -> Result<(StatusCode, String, Bytes), Response> {
    let body = opened
        .read_range(0, opened.size.saturating_sub(1))
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "not found").into_response())?;
    let content_type = image::guess_format(&body)
        .map(|f| f.to_mime_type())
        .unwrap_or("application/octet-stream");
    Ok((StatusCode::OK, content_type.to_string(), body))
}

/// The source's final answer, its body not yet read: a 429 or 503 is asked
/// again ([`UPSTREAM_RETRIES`]).
async fn send_remote(
    state: &super::MediaState,
    source: reqwest::Url,
) -> Result<reqwest::Response, Response> {
    let mut retries = UPSTREAM_RETRIES.iter();
    let upstream = loop {
        let upstream = match state.http.get(source.clone()).send().await {
            Ok(resp) => resp,
            Err(crate::http::SendError::Refused(_)) => {
                return Err(
                    (StatusCode::FORBIDDEN, "url host is not publicly routable").into_response()
                );
            }
            Err(e) => {
                return Err((
                    StatusCode::BAD_GATEWAY,
                    format!("upstream fetch failed: {e}"),
                )
                    .into_response());
            }
        };
        match retries
            .next()
            .and_then(|ladder| upstream_retry(&upstream, *ladder))
        {
            Some(wait) => tokio::time::sleep(wait).await,
            None => break upstream,
        }
    };
    if upstream.status().is_redirection() {
        return Err((StatusCode::BAD_GATEWAY, "too many redirects").into_response());
    }
    Ok(upstream)
}

async fn read_remote(
    upstream: reqwest::Response,
    max_bytes: usize,
) -> Result<(StatusCode, String, Bytes), Response> {
    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let content_type = upstream
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();
    let body = match read_body_capped(upstream, max_bytes).await {
        Ok(body) => Bytes::from(body),
        Err(e) => {
            return Err(if e.downcast_ref::<reqwest::Error>().is_some() {
                (
                    StatusCode::BAD_GATEWAY,
                    format!(
                        "upstream read failed: {}",
                        e.to_string().replace(char::is_control, " ")
                    ),
                )
                    .into_response()
            } else {
                (StatusCode::PAYLOAD_TOO_LARGE, "source too large").into_response()
            });
        }
    };
    Ok((status, content_type, body))
}

/// The waits before each retry of a source that answered 429 or 503: peers
/// limit how fast one address may ask for files their cache lacks, and a
/// page's burst outruns that limit by a second or two.
const UPSTREAM_RETRIES: [Duration; 3] = [
    Duration::from_millis(500),
    Duration::from_secs(1),
    Duration::from_secs(2),
];
/// A `Retry-After` past this is a ban, not a burst: its answer stands.
const UPSTREAM_MAX_RETRY_AFTER: Duration = Duration::from_secs(3);

/// How long to wait before asking again, when the answer is worth retrying.
fn upstream_retry(upstream: &reqwest::Response, ladder: Duration) -> Option<Duration> {
    if !matches!(upstream.status().as_u16(), 429 | 503) {
        return None;
    }
    let after = upstream
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(crate::http::parse_retry_after);
    match after {
        Some(after) if after > UPSTREAM_MAX_RETRY_AFTER => None,
        Some(after) => Some(after.max(ladder)),
        None => Some(ladder),
    }
}

fn cacheable(status: StatusCode, content_type: &str, body: Bytes) -> Result<Converted, Response> {
    if body.len() > CONVERT_CACHE_MAX_BODY {
        Err(build_response(status, content_type, body))
    } else {
        Ok((status.as_u16(), content_type.to_string(), body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_thumbnail_request_waits_in_line_for_a_busy_slot() {
        let slots = Arc::new(Slots::new(1, 1));
        let held = slots.acquire(Duration::ZERO).await.unwrap();
        let waiter = tokio::spawn({
            let slots = slots.clone();
            async move { slots.acquire(Duration::from_secs(30)).await.is_some() }
        });
        tokio::task::yield_now().await;
        assert!(!waiter.is_finished());
        drop(held);
        assert!(waiter.await.unwrap());
    }

    #[tokio::test(start_paused = true)]
    async fn past_the_line_or_the_wait_a_request_gets_a_retryable_busy_answer() {
        let slots = Arc::new(Slots::new(1, 1));
        let _held = slots.acquire(Duration::ZERO).await.unwrap();
        let queued = tokio::spawn({
            let slots = slots.clone();
            async move { slots.acquire(Duration::from_millis(20)).await.is_some() }
        });
        tokio::task::yield_now().await;
        assert!(
            slots.acquire(Duration::from_secs(30)).await.is_none(),
            "the line is full: refused at once"
        );
        assert!(!queued.await.unwrap(), "the wait ran out");
        let response = busy_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers()[header::RETRY_AFTER], "1");
    }

    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use axum::Router;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    use crate::media::backend::Backend;
    use crate::media::{BackendKind, ConvertSources, MediaState, Settings, router};

    fn lazy_pool() -> sqlx::PgPool {
        sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(Duration::from_millis(300))
            .connect_lazy("postgres://nobody@127.0.0.1:1/nowhere")
            .unwrap()
    }

    fn app(http: crate::http::Outbound) -> Router {
        app_reading(
            http,
            ConvertSources {
                remote: true,
                ..ConvertSources::default()
            },
        )
    }

    fn app_reading(http: crate::http::Outbound, sources: ConvertSources) -> Router {
        app_with(http, sources, THUMBNAIL_SLOTS, THUMBNAIL_QUEUE)
    }

    fn app_with(
        http: crate::http::Outbound,
        sources: ConvertSources,
        slots: usize,
        queue: usize,
    ) -> Router {
        let settings = Settings::for_tests(BackendKind::Disabled);
        let backend = Backend::build(&settings, reqwest::Client::new());
        router(Arc::new(
            MediaState::with_backend(lazy_pool(), settings, backend)
                .with_http(http)
                .with_convert_sources(sources)
                .with_convert_slots(slots, queue),
        ))
    }

    fn png(width: u32, height: u32) -> Bytes {
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbImage::from_pixel(width, height, image::Rgb([200, 10, 10]))
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        Bytes::from(png.into_inner())
    }

    /// A loopback source: `/img.png` and `/content/contents/{cid}` a 400x200
    /// PNG (the latter after 50 ms, as a peer across the network), `/missing`
    /// 404; counts hits.
    async fn source() -> (String, Arc<AtomicUsize>) {
        let hits = Arc::new(AtomicUsize::new(0));
        let png = png(400, 200);
        let counter = hits.clone();
        let slow = (png.clone(), hits.clone());
        let stub = Router::new()
            .route(
                "/img.png",
                axum::routing::get(move || {
                    let png = png.clone();
                    let counter = counter.clone();
                    async move {
                        counter.fetch_add(1, Ordering::SeqCst);
                        ([(header::CONTENT_TYPE, "image/png")], png)
                    }
                }),
            )
            .route(
                "/content/contents/{cid}",
                axum::routing::get(move || {
                    let (png, counter) = slow.clone();
                    async move {
                        counter.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        ([(header::CONTENT_TYPE, "image/png")], png)
                    }
                }),
            )
            .route(
                "/missing",
                axum::routing::get(|| async { (StatusCode::NOT_FOUND, "nope") }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, stub).await.unwrap() });
        (format!("http://{addr}"), hits)
    }

    async fn get(app: &Router, uri: &str) -> (StatusCode, HeaderMap, Bytes) {
        let resp = app
            .clone()
            .oneshot(Request::get(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = resp.status();
        let headers = resp.headers().clone();
        (
            status,
            headers,
            axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
    }

    fn q(url: &str) -> String {
        url::form_urlencoded::byte_serialize(url.as_bytes()).collect()
    }

    #[tokio::test]
    async fn a_source_is_proxied_resized_memoized_and_its_errors_passed_through() {
        let (base, hits) = source().await;
        let app = app(crate::http::Outbound::for_tests());

        let (status, _, body) = get(&app, "/convert?url=ftp%3A%2F%2Fexample.org%2Fx").await;
        assert_eq!(
            (status, &body[..]),
            (StatusCode::BAD_REQUEST, &b"url must be http(s)"[..])
        );
        let (status, _, _) = get(
            &app,
            &format!("/convert?width=81&url={}", q(&format!("{base}/img.png"))),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        let (status, headers, body) = get(
            &app,
            &format!("/media/convert?url={}", q(&format!("{base}/img.png"))),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CONTENT_TYPE], "image/png");
        assert_eq!(headers[header::CACHE_CONTROL], "public, max-age=86400");
        assert!(body.starts_with(b"\x89PNG"));
        assert_eq!(hits.load(Ordering::SeqCst), 1);

        let thumb = format!("/convert?width=80&url={}", q(&format!("{base}/img.png")));
        let (status, headers, body) = get(&app, &thumb).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CONTENT_TYPE], "image/jpeg");
        let decoded = image::load_from_memory(&body).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (80, 40));
        assert_eq!(hits.load(Ordering::SeqCst), 2);
        let (status, _, again) = get(&app, &thumb).await;
        assert_eq!((status, again), (StatusCode::OK, body));
        assert_eq!(
            hits.load(Ordering::SeqCst),
            2,
            "the second call is served from the memo"
        );

        let missing = format!("/convert?url={}", q(&format!("{base}/missing")));
        let (status, _, body) = get(&app, &missing).await;
        assert_eq!((status, &body[..]), (StatusCode::NOT_FOUND, &b"nope"[..]));
    }

    #[tokio::test]
    async fn the_guarded_client_refuses_a_private_source() {
        let (base, hits) = source().await;
        let guarded = crate::http::Outbound::new(crate::http::HttpClientCfg::default()).unwrap();
        let (status, _, body) = get(
            &app(guarded),
            &format!("/convert?url={}", q(&format!("{base}/img.png"))),
        )
        .await;
        assert_eq!(
            (status, &body[..]),
            (
                StatusCode::FORBIDDEN,
                &b"url host is not publicly routable"[..]
            )
        );
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }

    /// Off the mirror posture a node sends nothing for `/convert`: a remote
    /// URL is refused before any fetch, and its own content URLs are read
    /// from the blob store.
    #[tokio::test]
    async fn without_remote_sources_only_this_nodes_own_content_converts() {
        let (base, hits) = source().await;
        let dir = std::env::temp_dir().join(format!(
            "catalyst-convert-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let storage = Arc::new(crate::storage::Storage::from_config(&dir).await.unwrap());
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbImage::from_pixel(320, 160, image::Rgb([10, 200, 10]))
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let hash = "bafkreiconvertownblobaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        storage
            .store(hash, Bytes::from(png.into_inner()))
            .await
            .unwrap();
        let own = "https://node.example.org";
        let app = app_reading(
            crate::http::Outbound::for_tests(),
            ConvertSources {
                remote: false,
                own_url: Some(url::Url::parse(own).unwrap()),
                storage: Some(storage),
                read_through: None,
            },
        );

        for remote in [
            format!("{base}/img.png"),
            format!("https://elsewhere.example.org/content/contents/{hash}"),
            format!("{own}/content/contents/{hash}?x=1"),
            format!("{own}/lambdas/x"),
        ] {
            let (status, _, body) = get(&app, &format!("/convert?url={}", q(&remote))).await;
            assert_eq!(
                (status, &body[..]),
                (
                    StatusCode::FORBIDDEN,
                    &b"this node converts only its own /content/contents/ files"[..]
                ),
                "{remote}"
            );
        }
        assert_eq!(hits.load(Ordering::SeqCst), 0, "nothing was fetched");

        let mine = q(&format!("{own}/content/contents/{hash}"));
        let (status, headers, body) = get(&app, &format!("/media/convert?url={mine}")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CONTENT_TYPE], "image/png");
        assert!(body.starts_with(b"\x89PNG"));
        let (status, headers, body) = get(&app, &format!("/convert?width=80&url={mine}")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CONTENT_TYPE], "image/jpeg");
        let decoded = image::load_from_memory(&body).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (80, 40));
        let absent = q(&format!("{own}/content/contents/bafkreimissing"));
        let (status, _, _) = get(&app, &format!("/convert?url={absent}")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A content URL: `bafkrei` and 52 base32 characters, the `n`th.
    fn cid(n: usize) -> String {
        let tag: String = n
            .to_string()
            .bytes()
            .map(|d| (d - b'0' + b'b') as char)
            .collect();
        format!("bafkrei{tag:a>52}")
    }

    #[tokio::test]
    async fn a_page_burst_of_thumbnails_waits_in_line_and_is_served() {
        let (base, hits) = source().await;
        let app = app_with(
            crate::http::Outbound::for_tests(),
            ConvertSources {
                remote: true,
                ..ConvertSources::default()
            },
            2,
            64,
        );
        let mut burst = tokio::task::JoinSet::new();
        for n in 0..40 {
            let app = app.clone();
            let url = q(&format!("{base}/content/contents/{}", cid(n)));
            burst.spawn(async move {
                get(&app, &format!("/media/convert?width=80&url={url}"))
                    .await
                    .0
            });
        }
        let statuses = burst.join_all().await;
        assert!(
            statuses.iter().all(|s| *s == StatusCode::OK),
            "{statuses:?}"
        );
        assert_eq!(hits.load(Ordering::SeqCst), 40);
    }

    #[tokio::test]
    async fn a_memoized_answer_never_waits_for_a_slot() {
        let (base, _) = source().await;
        let settings = Settings::for_tests(BackendKind::Disabled);
        let backend = Backend::build(&settings, reqwest::Client::new());
        let state = Arc::new(
            MediaState::with_backend(lazy_pool(), settings, backend)
                .with_convert_sources(ConvertSources {
                    remote: true,
                    ..ConvertSources::default()
                })
                .with_convert_slots(1, 1),
        );
        let app = router(state.clone());
        let warm = format!(
            "/media/convert?width=80&url={}",
            q(&format!("{base}/img.png"))
        );
        assert_eq!(get(&app, &warm).await.0, StatusCode::OK);

        let slot = state.convert_slots.acquire(Duration::ZERO).await.unwrap();
        let line = state.convert_slots.admitted.try_acquire().unwrap();
        assert_eq!(
            get(&app, &warm).await.0,
            StatusCode::OK,
            "served from the memo"
        );
        let cold = format!(
            "/media/convert?width=80&url={}",
            q(&format!("{base}/content/contents/{}", cid(9)))
        );
        let (status, headers, _) = get(&app, &cold).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "the line is full");
        assert_eq!(headers[header::RETRY_AFTER], "1");
        drop((slot, line));
        assert_eq!(get(&app, &cold).await.0, StatusCode::OK);
    }

    #[tokio::test]
    async fn a_rate_limited_source_is_asked_again_unless_it_bans() {
        let flaky = Arc::new(AtomicUsize::new(0));
        let banned = Arc::new(AtomicUsize::new(0));
        let png = png(400, 200);
        let stub = Router::new()
            .route(
                "/flaky.png",
                axum::routing::get({
                    let hits = flaky.clone();
                    move || {
                        let png = png.clone();
                        let hit = hits.fetch_add(1, Ordering::SeqCst);
                        async move {
                            if hit < 2 {
                                (
                                    StatusCode::TOO_MANY_REQUESTS,
                                    [(header::RETRY_AFTER, "1")],
                                    Bytes::new(),
                                )
                            } else {
                                (StatusCode::OK, [(header::CONTENT_TYPE, "image/png")], png)
                            }
                        }
                    }
                }),
            )
            .route(
                "/banned.png",
                axum::routing::get({
                    let hits = banned.clone();
                    move || {
                        hits.fetch_add(1, Ordering::SeqCst);
                        async {
                            (
                                StatusCode::TOO_MANY_REQUESTS,
                                [(header::RETRY_AFTER, "3600")],
                                "slow down",
                            )
                        }
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, stub).await.unwrap() });
        let app = app(crate::http::Outbound::for_tests());

        let url = q(&format!("{base}/flaky.png"));
        let (status, headers, _) = get(&app, &format!("/convert?width=80&url={url}")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CONTENT_TYPE], "image/jpeg");
        assert_eq!(flaky.load(Ordering::SeqCst), 3, "two 429s, then the file");

        let url = q(&format!("{base}/banned.png"));
        let (status, _, body) = get(&app, &format!("/convert?width=80&url={url}")).await;
        assert_eq!(
            (status, &body[..]),
            (StatusCode::TOO_MANY_REQUESTS, &b"slow down"[..])
        );
        assert_eq!(
            banned.load(Ordering::SeqCst),
            1,
            "an hour's Retry-After is not waited out"
        );
    }

    /// A content file is the same bytes on every host: the copy this node
    /// holds is converted and the host the URL names is never asked.
    #[tokio::test]
    async fn a_content_url_on_any_host_is_read_from_the_store_first() {
        let (base, hits) = source().await;
        let dir = std::env::temp_dir().join(format!(
            "catalyst-convert-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let storage = Arc::new(crate::storage::Storage::from_config(&dir).await.unwrap());
        let held = cid(1);
        storage.store(&held, png(320, 160)).await.unwrap();
        let app = app_reading(
            crate::http::Outbound::for_tests(),
            ConvertSources {
                remote: true,
                storage: Some(storage),
                ..ConvertSources::default()
            },
        );

        let url = q(&format!("{base}/content/contents/{held}"));
        let (status, headers, body) = get(&app, &format!("/convert?width=80&url={url}")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CONTENT_TYPE], "image/jpeg");
        let decoded = image::load_from_memory(&body).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (80, 40));
        assert_eq!(hits.load(Ordering::SeqCst), 0, "served from the store");

        let url = q(&format!("{base}/content/contents/{}", cid(2)));
        let (status, _, body) = get(&app, &format!("/convert?width=80&url={url}")).await;
        assert_eq!(status, StatusCode::OK);
        let decoded = image::load_from_memory(&body).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (80, 40));
        assert_eq!(
            hits.load(Ordering::SeqCst),
            1,
            "a file the store lacks is fetched"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_a_canonical_content_path_names_a_content_hash() {
        let hash = |s: &str| ConvertSources::content_hash(&url::Url::parse(s).unwrap());
        let cid = cid(3);
        assert_eq!(
            hash(&format!(
                "https://peer-ec1.decentraland.org/content/contents/{cid}"
            )),
            Some(cid.clone())
        );
        for other in [
            format!("https://peer.example.org/content/contents/{cid}?x=1"),
            format!("https://peer.example.org/contents/{cid}"),
            "https://peer.example.org/content/contents/not-a-cid".to_string(),
            format!("https://peer.example.org/content/contents/{cid}/x"),
        ] {
            assert_eq!(hash(&other), None, "{other}");
        }
    }

    #[test]
    fn only_a_small_success_is_cacheable() {
        assert!(cacheable(StatusCode::OK, "image/png", Bytes::from_static(b"x")).is_ok());
        let big = Bytes::from(vec![0u8; CONVERT_CACHE_MAX_BODY + 1]);
        assert!(cacheable(StatusCode::OK, "image/png", big).is_err());
    }
}
