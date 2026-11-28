//! `POST /v1/items/:id/videos`: a smart wearable's preview video, the one
//! file part `video.mp4` of a multipart form, up to [`DEFAULT_MAX_BYTES`].
//! The handler decides access, checks that the item names a video and
//! admits the upload ([`Videos::admit`]) before it reads a byte of the body.
//! The body then streams through [`Framed`], which bounds what the multipart
//! parser may hold beside the video's own bytes, into one spool file beside
//! the blob store, hashed as it arrives and walked as an MP4 container
//! ([`Mp4`]); every read waits at most until the upload's deadline and the
//! idle bound after the last bytes, so a sender that stalls is cut and
//! frees its place, and an upload behind the pace the largest video needs
//! gives its place to a newcomer that earns it ([`super::places`]). A
//! wallet and an upload source have a day of upload time
//! ([`WALLET_VIDEO_SECS_A_DAY`], [`SOURCE_VIDEO_SECS_A_DAY`]), spent by
//! the time each upload holds its place, bytes or none. The spool file is
//! synced, the file goes to the blob store, and only then, in one
//! transaction with the item locked, the decision runs again, the item must
//! still name the hash at `video.mp4` and the upload is recorded while the
//! bytes are still stored, as [`super::upload`] stores files.

use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::task::{Context, Poll};
use std::time::{Duration, SystemTime};

use axum::body::{Body, BodyDataStream, Bytes};
use axum::extract::{FromRequest, Multipart, Request};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use dcl_auth::hash::HashV1Writer;
use futures::Stream;
use serde_json::json;
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::time::Instant;

use crate::access::{self, ItemChange};
use crate::chain_state;
use crate::http::ApiError;
use crate::storage::{StagingGuard, Storage, create_staging_file};

use super::ItemsComponent;
use super::places::{self, Admission, Places, Timing};
use super::save::lock_item;
use super::upload::{lock_store_shared, store_collected};

/// The file name an item gives its preview video, and the upload's one field.
pub const VIDEO_FILE: &str = "video.mp4";

/// builder-server's `AWS_MAX_VIDEO_SIZE` default: 250 MB, decimal.
pub const DEFAULT_MAX_BYTES: u64 = 250_000_000;

/// The route's body bound: the largest video and room for its form.
pub const BODY_LIMIT: usize = DEFAULT_MAX_BYTES as usize + 1024 * 1024;

/// The longest an upload's body may take to arrive: a 250 MB video at
/// about 1.7 Mbit/s.
pub const DEFAULT_DEADLINE: Duration = Duration::from_secs(20 * 60);

/// Videos the node reads at once (`BUILDER_VIDEO_UPLOADS_MAX`), and of them
/// per wallet and per upload source, so filling the node takes as many
/// wallets and half as many sources; the spool holds at most
/// [`DEFAULT_MAX_BYTES`] for each.
pub const MAX_UPLOADS: usize = 32;
pub const MAX_UPLOADS_PER_SIGNER: usize = 1;
pub const MAX_UPLOADS_PER_SOURCE: usize = 2;

/// The time one wallet may spend uploading videos a day: 36 largest videos
/// at 5 Mbit/s.
pub const WALLET_VIDEO_SECS_A_DAY: Duration = Duration::from_secs(4 * 3_600);
/// The time one upload source may spend uploading videos a day.
pub const SOURCE_VIDEO_SECS_A_DAY: Duration = Duration::from_secs(12 * 3_600);

/// The seconds of video uploading each wallet and source spent in the last
/// day.
pub struct Used {
    pub wallets: crate::shared::limits::RateLimiter,
    pub sources: crate::shared::limits::RateLimiter,
}

impl Default for Used {
    fn default() -> Self {
        let day = Duration::from_secs(86_400);
        let secs = |d: Duration| u32::try_from(d.as_secs()).unwrap_or(u32::MAX);
        Self {
            wallets: crate::limits::budget(secs(WALLET_VIDEO_SECS_A_DAY), day),
            sources: crate::limits::budget(secs(SOURCE_VIDEO_SECS_A_DAY), day),
        }
    }
}

/// Bytes the multipart parser may have taken from the body beyond the
/// video's bytes it handed on: the part headers, boundaries and look-ahead.
pub const FRAMING_BYTES: u64 = 64 * 1024;

const SLICE_BYTES: usize = 16 * 1024;

/// How old a spool file must be to be a crashed upload's, far past
/// [`DEFAULT_DEADLINE`] and the store after it.
pub const SPOOL_ORPHAN_AGE: Duration = Duration::from_secs(60 * 60);

const SPOOL_DIR: &str = "item-videos";

/// Where uploads spool: beside the blob tree, so storing is a hard link.
pub fn spool_root(storage: &Storage) -> PathBuf {
    storage.scratch_dir().join(SPOOL_DIR)
}

/// The video uploads' bounds and the uploads in flight.
pub struct Videos {
    pub max_bytes: u64,
    pub deadline: Duration,
    pub idle_timeout: Duration,
    pub places: Places,
    /// What each wallet and source spent uploading today.
    pub used: Arc<Used>,
}

impl Default for Videos {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_BYTES,
            deadline: DEFAULT_DEADLINE,
            idle_timeout: places::IDLE_TIMEOUT,
            places: Places::new(
                "video uploads",
                MAX_UPLOADS,
                MAX_UPLOADS_PER_SIGNER,
                MAX_UPLOADS_PER_SOURCE,
            ),
            used: Arc::default(),
        }
    }
}

/// An admitted video upload: its place, and who pays its time.
pub struct VideoAdmission {
    place: Admission,
    wallet: String,
    source: Option<String>,
    started: Instant,
    ended: AtomicBool,
    used: Arc<Used>,
}

impl VideoAdmission {
    /// Its place, which its body reads through.
    pub fn place(&mut self) -> &mut Admission {
        &mut self.place
    }

    /// A newcomer took its place, or, a newcomer itself, it found none.
    pub fn gave_way(&self) -> bool {
        self.place.gave_way()
    }

    /// Its body has arrived: no newcomer takes its place while it stores,
    /// and the time it took counts toward its wallet's and source's day.
    pub fn received(&self) {
        self.place.body_arrived();
        if self.ended.swap(true, Ordering::SeqCst) {
            return;
        }
        let now = Instant::now();
        let secs = u32::try_from(
            now.saturating_duration_since(self.started)
                .as_secs_f64()
                .ceil() as u64,
        )
        .unwrap_or(u32::MAX);
        let at = now.into_std();
        self.used.wallets.charge_at(&self.wallet, secs, at);
        if let Some(source) = &self.source {
            self.used.sources.charge_at(source, secs, at);
        }
    }
}

impl Drop for VideoAdmission {
    fn drop(&mut self) {
        self.received();
    }
}

impl Videos {
    /// A place for `signer`'s upload from `source` ([`Places::admit`]) among
    /// the node's places, [`MAX_UPLOADS_PER_SIGNER`] and
    /// [`MAX_UPLOADS_PER_SOURCE`], while the wallet and the source have
    /// time left of their day ([`WALLET_VIDEO_SECS_A_DAY`],
    /// [`SOURCE_VIDEO_SECS_A_DAY`]; 429 with `Retry-After` otherwise). Its
    /// body must arrive within the deadline and the idle bound, and an upload
    /// behind the pace [`Videos::max_bytes`] needs to arrive by the deadline
    /// gives its place to a newcomer that earns it ([`Places`]).
    pub fn admit(
        &self,
        signer: &str,
        source: Option<&str>,
        declared: Option<u64>,
    ) -> Result<VideoAdmission, Response> {
        let now = Instant::now();
        let at = now.into_std();
        for (spent, key) in [
            (&self.used.wallets, Some(signer)),
            (&self.used.sources, source),
        ] {
            if let Some(key) = key {
                spent
                    .take_at(key, 1, at)
                    .map_err(|wait| ApiError::Limited(wait).into_response())?;
                spent.refund(key, 1);
            }
        }
        let place = self.places.admit(
            signer,
            source,
            Timing {
                deadline: self.deadline,
                idle: self.idle_timeout,
                most: self.max_bytes,
            },
            declared,
        )?;
        Ok(VideoAdmission {
            place,
            wallet: signer.to_string(),
            source: source.map(str::to_string),
            started: now,
            ended: AtomicBool::new(false),
            used: self.used.clone(),
        })
    }

    /// Uploads holding a place.
    pub fn in_flight(&self) -> usize {
        self.places.in_flight()
    }

    pub fn idle(&self) -> bool {
        self.in_flight() == 0
    }

    /// Removes the spool files at least [`SPOOL_ORPHAN_AGE`] old, which only
    /// an upload a crash or restart cut short leaves.
    pub async fn sweep(&self, root: &Path) {
        let Ok(mut entries) = tokio::fs::read_dir(root).await else {
            return;
        };
        let now = SystemTime::now();
        while let Ok(Some(entry)) = entries.next_entry().await {
            let orphan = entry
                .metadata()
                .await
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|m| now.duration_since(m).ok())
                .is_some_and(|age| age >= SPOOL_ORPHAN_AGE);
            if orphan {
                let _ = tokio::fs::remove_file(entry.path()).await;
            }
        }
    }
}

/// A received video: its spool file (removed when dropped), hash and size.
pub struct Spooled {
    guard: StagingGuard,
    hash: String,
    size: u64,
}

impl Spooled {
    /// The video's bytes.
    pub fn size(&self) -> u64 {
        self.size
    }
}

fn invalid() -> ApiError {
    ApiError::bad_request("Invalid multipart upload")
}

fn not_mp4() -> ApiError {
    ApiError::bad_request("The video is not an MP4 file")
}

/// 413 for a video past `max_bytes`.
pub(crate) fn too_large(max_bytes: u64) -> ApiError {
    ApiError::http(413, "The video is too large").with_data(json!({ "maxBytes": max_bytes }))
}

fn io(e: std::io::Error) -> ApiError {
    ApiError::internal(format!("spooling a video: {e}"))
}

/// Reads `request`'s form into a spool file under `root`, through `place`
/// ([`Admission::read`], [`Admission::arrived`]): one file part named
/// [`VIDEO_FILE`], at most `videos.max_bytes`, an MP4 container, synced to
/// disk before it is answered.
pub async fn receive(
    videos: &Videos,
    root: &Path,
    request: Request,
    place: &mut Admission,
) -> Result<Spooled, ApiError> {
    let too_large = || too_large(videos.max_bytes);
    let delivered = Arc::new(AtomicU64::new(0));
    let (parts, body) = request.into_parts();
    let body = Body::from_stream(Framed {
        inner: body.into_data_stream(),
        pending: Bytes::new(),
        pulled: 0,
        delivered: delivered.clone(),
        rest: false,
    });
    let mut multipart = Multipart::from_request(Request::from_parts(parts, body), &())
        .await
        .map_err(|_| invalid())?;
    let failed = |e: axum::extract::multipart::MultipartError| {
        if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
            too_large()
        } else {
            invalid()
        }
    };
    let mut field = place
        .read(multipart.next_field())
        .await?
        .map_err(failed)?
        .ok_or_else(|| ApiError::bad_request("The upload carries no video"))?;
    if field.name() != Some(VIDEO_FILE) || field.file_name().is_none() {
        return Err(ApiError::bad_request(
            "The video is sent as the file video.mp4",
        ));
    }
    tokio::fs::create_dir_all(root).await.map_err(io)?;
    let path = root.join(format!(
        "{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let (file, guard) = create_staging_file(path)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut file = BufWriter::with_capacity(256 * 1024, file);
    let (mut hasher, mut container, mut size) = (HashV1Writer::new(), Mp4::default(), 0u64);
    while let Some(chunk) = place.read(field.chunk()).await?.map_err(failed)? {
        delivered.fetch_add(chunk.len() as u64, Ordering::SeqCst);
        place.arrived(chunk.len())?;
        size += chunk.len() as u64;
        if size > videos.max_bytes {
            return Err(too_large());
        }
        container.feed(&chunk);
        if container.broken {
            return Err(not_mp4());
        }
        hasher.update(&chunk);
        file.write_all(&chunk).await.map_err(io)?;
    }
    drop(field);
    if place
        .read(multipart.next_field())
        .await?
        .map_err(failed)?
        .is_some()
    {
        return Err(ApiError::bad_request("The upload carries one video only"));
    }
    if !container.complete() {
        return Err(not_mp4());
    }
    file.flush().await.map_err(io)?;
    file.get_ref().sync_all().await.map_err(io)?;
    Ok(Spooled {
        guard,
        hash: hasher.finish(),
        size,
    })
}

/// The request body in slices of at most 16 KiB, one per poll, so the
/// multipart parser never drains more than one slice at a time; it fails
/// once the parser holds more than [`FRAMING_BYTES`] it hasn't handed on.
struct Framed {
    inner: BodyDataStream,
    pending: Bytes,
    pulled: u64,
    delivered: Arc<AtomicU64>,
    rest: bool,
}

impl Stream for Framed {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = &mut *self;
        if this.rest {
            this.rest = false;
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }
        if this.pending.is_empty() {
            match Pin::new(&mut this.inner).poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(std::io::Error::other(e))));
                }
                Poll::Ready(Some(Ok(bytes))) => this.pending = bytes,
            }
        }
        let slice = this.pending.split_to(SLICE_BYTES.min(this.pending.len()));
        this.pulled += slice.len() as u64;
        if this.pulled - this.delivered.load(Ordering::SeqCst) > FRAMING_BYTES {
            return Poll::Ready(Some(Err(std::io::Error::other(
                "multipart framing over its bound",
            ))));
        }
        this.rest = true;
        Poll::Ready(Some(Ok(slice)))
    }
}

/// An ISO base media (MP4) file's top-level boxes, walked as its bytes
/// arrive: an `ftyp` box first, a `moov` box somewhere, and boxes that end
/// exactly where the file does.
#[derive(Default)]
pub(crate) struct Mp4 {
    at: u64,
    next: u64,
    head: Vec<u8>,
    boxes: u64,
    moov: bool,
    to_end: bool,
    pub(crate) broken: bool,
}

impl Mp4 {
    pub(crate) fn feed(&mut self, mut bytes: &[u8]) {
        while !bytes.is_empty() && !self.broken {
            if self.to_end {
                self.at += bytes.len() as u64;
                return;
            }
            if self.at < self.next {
                let skip = (self.next - self.at).min(bytes.len() as u64);
                self.at += skip;
                bytes = &bytes[skip as usize..];
                continue;
            }
            let want = if self.head.len() >= 8 && self.size32() == 1 {
                16
            } else {
                8
            };
            let take = (want - self.head.len()).min(bytes.len());
            self.head.extend_from_slice(&bytes[..take]);
            self.at += take as u64;
            bytes = &bytes[take..];
            if self.head.len() == 8 && self.size32() == 1 {
                continue;
            }
            if self.head.len() == want {
                self.open();
            }
        }
    }

    fn size32(&self) -> u32 {
        u32::from_be_bytes(self.head[..4].try_into().expect("four bytes"))
    }

    fn open(&mut self) {
        let start = self.at - self.head.len() as u64;
        let kind: [u8; 4] = self.head[4..8].try_into().expect("four bytes");
        let size = match self.size32() {
            0 => {
                self.to_end = true;
                0
            }
            1 => u64::from_be_bytes(self.head[8..16].try_into().expect("eight bytes")),
            size => u64::from(size),
        };
        let first_ok = self.boxes > 0 || (&kind == b"ftyp" && (self.to_end || size >= 16));
        if !first_ok || (!self.to_end && size < self.head.len() as u64) {
            self.broken = true;
            return;
        }
        self.moov |= &kind == b"moov";
        self.boxes += 1;
        self.next = start.saturating_add(size);
        self.head.clear();
    }

    pub(crate) fn complete(&self) -> bool {
        !self.broken && self.moov && (self.to_end || (self.head.is_empty() && self.at == self.next))
    }
}

impl ItemsComponent {
    /// Stores `video` for the item `change` was decided for, if the decision
    /// holds again and the item still names it at [`VIDEO_FILE`]: the bytes
    /// go to the blob store first, holding no lock, then one transaction
    /// with the item locked decides again and records the upload under the
    /// store lock's shared side, once the bytes are still stored (503
    /// otherwise). Answers whether the record is new to the signer.
    pub async fn save_item_video(
        &self,
        change: &ItemChange,
        video: Spooled,
    ) -> Result<bool, ApiError> {
        let id = change.id();
        {
            let mut conn = self.pool.acquire().await?;
            let view = chain_state::for_decision(&mut conn).await?;
            access::confirm_item_change(&mut conn, &view, change).await?;
            named_video(&mut conn, id, &video.hash).await?;
        }
        if !self.held(&video.hash).await? {
            self.blobs
                .store_file(&video.hash, video.guard.path())
                .await
                .map_err(|e| ApiError::internal(format!("storing {}: {e}", video.hash)))?;
        }
        let mut tx = self.pool.begin().await?;
        let view = chain_state::for_decision(&mut tx).await?;
        lock_item(&mut tx, id, None).await?;
        access::confirm_item_change(&mut tx, &view, change).await?;
        named_video(&mut tx, id, &video.hash).await?;
        lock_store_shared(&mut tx).await?;
        if !self.held(&video.hash).await? {
            return Err(store_collected());
        }
        let added: bool = sqlx::query_scalar(
            "INSERT INTO builder.uploaded_contents (hash, size, uploaded_by, video) VALUES ($1, $2, $3, true) \
             ON CONFLICT (hash, uploaded_by) DO UPDATE SET video = true RETURNING xmax = 0",
        )
        .bind(&video.hash)
        .bind(video.size as i64)
        .bind(change.signer())
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(added)
    }

    async fn held(&self, hash: &str) -> Result<bool, ApiError> {
        self.blobs
            .exist(hash)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))
    }

    /// Whether item `id` names a video at [`VIDEO_FILE`].
    pub async fn names_video(&self, id: uuid::Uuid) -> Result<bool, ApiError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM builder.item_contents WHERE item_id = $1 AND file = $2)",
        )
        .bind(id)
        .bind(VIDEO_FILE)
        .fetch_one(&self.pool)
        .await?)
    }
}

/// Item `id` names `hash` at [`VIDEO_FILE`] (400 otherwise).
async fn named_video(
    conn: &mut sqlx::PgConnection,
    id: uuid::Uuid,
    hash: &str,
) -> Result<(), ApiError> {
    let named: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM builder.item_contents WHERE item_id = $1 AND file = $2 AND hash = $3)",
    )
    .bind(id)
    .bind(VIDEO_FILE)
    .bind(hash)
    .fetch_one(conn)
    .await?;
    if !named {
        return Err(ApiError::bad_request("The item does not name this video")
            .with_data(json!({ "hash": hash })));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Mp4;

    fn walk(chunks: &[&[u8]]) -> Mp4 {
        let mut mp4 = Mp4::default();
        for chunk in chunks {
            mp4.feed(chunk);
        }
        mp4
    }

    const FTYP: &[u8] = b"\0\0\0\x10ftypisom\0\0\x02\0";
    const MOOV: &[u8] = b"\0\0\0\x08moov";

    #[test]
    fn the_box_walk_takes_split_headers_large_sizes_and_a_box_to_the_end() {
        let whole = [FTYP, MOOV].concat();
        for split in 0..whole.len() {
            assert!(
                walk(&[&whole[..split], &whole[split..]]).complete(),
                "split at {split}"
            );
        }
        let large = [FTYP, b"\0\0\0\x01mdat\0\0\0\0\0\0\0\x14abcd", MOOV].concat();
        assert!(walk(&[&large]).complete());
        let to_end = [FTYP, b"\0\0\0\0moovanything after"].concat();
        assert!(walk(&[&to_end]).complete());
    }

    #[test]
    fn the_box_walk_refuses_what_no_player_opens() {
        assert!(walk(&[b"<html><body>"]).broken, "a page");
        assert!(
            walk(&[b"\0\0\0\x08ftyp", MOOV]).broken,
            "an ftyp without its brand"
        );
        assert!(walk(&[MOOV, FTYP]).broken, "moov before ftyp");
        assert!(!walk(&[FTYP, b"\0\0\0\x08mdat"]).complete(), "no moov");
        assert!(
            !walk(&[FTYP, b"\0\0\0\x10moov1234"]).complete(),
            "a truncated box"
        );
        assert!(
            !walk(&[FTYP, MOOV, b"\0\0"]).complete(),
            "a truncated header"
        );
        assert!(
            walk(&[FTYP, b"\0\0\0\x04moov"]).broken,
            "a box smaller than its header"
        );
        assert!(!walk(&[]).complete(), "nothing");
    }
}

#[cfg(test)]
mod admission_tests {
    use super::*;

    fn site(n: usize) -> String {
        format!("198.51.100.{n}")
    }

    fn hold(videos: &Videos, wallet: &str, source: &str) -> VideoAdmission {
        videos
            .admit(wallet, Some(source), None)
            .map_err(|refused| refused.status())
            .expect("a place")
    }

    /// A finished upload counts its time once toward its wallet's day: the
    /// route calls `received` when the body has arrived, then drops the
    /// admission after storing.
    #[tokio::test(start_paused = true)]
    async fn a_finished_upload_counts_its_time_once_toward_its_wallets_day() {
        let videos = Videos::default();
        for n in 0..2 {
            let upload = hold(&videos, "0xcreator", &site(n));
            tokio::time::advance(Duration::from_secs(2 * 3_600 - 60)).await;
            upload.received();
            drop(upload);
        }
        assert!(
            videos.admit("0xcreator", Some(&site(2)), None).is_ok(),
            "two finished uploads of under two hours each leave time in a four-hour day"
        );
    }

    /// One video at a time per wallet, and at most its hours of uploading a
    /// day per wallet and per address.
    #[tokio::test(start_paused = true)]
    async fn a_wallet_uploads_one_video_at_a_time_and_its_hours_a_day() {
        let videos = Videos::default();
        let first = hold(&videos, "0xone", "203.0.113.3");
        let second = videos.admit("0xone", Some("203.0.113.4"), None);
        assert_eq!(second.err().map(|r| r.status().as_u16()), Some(429));
        tokio::time::advance(WALLET_VIDEO_SECS_A_DAY + Duration::from_secs(1)).await;
        drop(first);
        let later = videos.admit("0xone", Some("203.0.113.5"), None);
        let refused = later.err().expect("its day of uploading is spent");
        assert_eq!(refused.status().as_u16(), 429);
        assert!(
            refused
                .headers()
                .contains_key(axum::http::header::RETRY_AFTER)
        );
        assert!(videos.admit("0xtwo", Some("203.0.113.5"), None).is_ok());
    }
}
