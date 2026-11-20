//! One remote entity, end to end: fetch the entity file from a peer, prove it
//! by its content hash, fetch every content file it declares (same proof),
//! then hand it to the batching [`LiveSyncDeployer`]. Nothing unproven is ever
//! stored, and a stored entity file that later fails its hash is evicted.

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::sync::{Arc, OnceLock};

use crate::http::CheckedRedirectClient;
use futures::StreamExt;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tracing::{debug, warn};

use super::batch_deployer::DeploymentReport;
use super::sync_deployer::LiveSyncDeployer;
use super::sync_types::{AuthChain, DeploymentContext, SyncError};
use crate::auth::{is_canonical_cid, verify_hash};
use crate::entity::{is_ipfs_v1, is_ipfs_v2};
use crate::storage::Storage;

const MAX_DOWNLOAD_RETRIES: u32 = 3;
const RETRY_WAIT_MS: u64 = 500;

const MAX_BODY_BYTES: usize = 512 * 1024 * 1024;

/// What a download without a Content-Length holds against the byte budget before its first
/// chunk; the hold grows with the decoded body from there.
const BODY_ESTIMATE_BYTES: u64 = 4 * 1024 * 1024;
/// Growth is asked for in steps at least this large, so a chunked body of a few MB does not
/// queue on the semaphore once per 16 KB chunk.
const BUDGET_GROWTH_STEP_BYTES: u64 = 4 * 1024 * 1024;
/// How long a download already holding part of the budget waits for more. Growth is asked
/// for after the bytes are buffered, so when every in-flight body needs more and none can
/// finish, the only way out is for one of them to give up and release its hold: the wait
/// turns that case into a failed attempt (retried later) instead of a hang.
const BUDGET_GROWTH_WAIT: std::time::Duration = std::time::Duration::from_secs(60);

/// Content addressing is over the DECODED file bytes, but some peers (peer.dclnodes.io) serve
/// content-addressed blobs with `Content-Encoding: gzip` regardless of Accept-Encoding, and
/// this package's reqwest has every automatic-decompression feature disabled -- the sync code
/// receives the wire bytes as sent. The transfer coding must therefore be undone here, before
/// hashing and storing, or verification fails on every such blob. It is undone chunk by chunk
/// ([`stream_body`]), so a body spooled to disk never has its wire bytes and its decoded bytes
/// in memory at once, and the caller caps the decoded size so a compression bomb cannot
/// outgrow its body cap.
pub(crate) enum TransferDecoder {
    Identity,
    Gzip(Box<flate2::write::MultiGzDecoder<Vec<u8>>>),
}

/// Compressed bytes inflated per step; deflate expands at most ~1032x, so one step yields
/// at most ~258 KiB.
const GZIP_INPUT_STEP_BYTES: usize = 256;
/// Decoded bytes [`stream_body`] holds before handing them to its sink.
const SINK_FLUSH_BYTES: usize = 1 << 20;

impl TransferDecoder {
    /// The wire bytes [`stream_body`] decodes at a time.
    fn input_step(&self) -> usize {
        match self {
            Self::Identity => usize::MAX,
            Self::Gzip(_) => GZIP_INPUT_STEP_BYTES,
        }
    }

    pub(crate) fn for_encoding(encoding: Option<&str>) -> Result<Self, String> {
        match encoding.unwrap_or("").trim().to_ascii_lowercase().as_str() {
            "" | "identity" => Ok(Self::Identity),
            "gzip" | "x-gzip" => Ok(Self::Gzip(Box::new(flate2::write::MultiGzDecoder::new(
                Vec::new(),
            )))),
            other => Err(format!("unsupported content-encoding {other:?}")),
        }
    }

    /// Appends what `chunk` decodes to (so far) onto `out`, failing once `out` passes
    /// `limit` bytes: gzip input is fed in small steps, so a compression bomb overshoots the
    /// limit by at most one step's output, never by a whole wire chunk's.
    pub(crate) fn push(
        &mut self,
        chunk: &[u8],
        out: &mut Vec<u8>,
        limit: usize,
    ) -> Result<(), String> {
        match self {
            Self::Identity => out.extend_from_slice(chunk),
            Self::Gzip(decoder) => {
                for step in chunk.chunks(GZIP_INPUT_STEP_BYTES) {
                    decoder
                        .write_all(step)
                        .map_err(|e| format!("gzip transfer decoding failed: {e}"))?;
                    out.append(decoder.get_mut());
                    if out.len() > limit {
                        return Err(format!("decoded body exceeds the {limit} bytes left"));
                    }
                }
            }
        }
        Ok(())
    }

    /// Appends whatever the coding still holds onto `out`; a gzip stream cut short fails here.
    pub(crate) fn finish(self, out: &mut Vec<u8>) -> Result<(), String> {
        match self {
            Self::Identity => Ok(()),
            Self::Gzip(decoder) => {
                let mut rest = decoder
                    .finish()
                    .map_err(|e| format!("gzip transfer decoding failed: {e}"))?;
                out.append(&mut rest);
                Ok(())
            }
        }
    }
}

pub(crate) fn response_content_encoding(resp: &reqwest::Response) -> Option<String> {
    resp.headers()
        .get(reqwest::header::CONTENT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
}

/// Where [`stream_body`] hands each run of decoded bytes: a buffer for an entity or content
/// file, the spool file plus hasher for a snapshot.
pub(crate) trait DecodedSink {
    fn accept(
        &mut self,
        decoded: &[u8],
    ) -> impl std::future::Future<Output = Result<(), String>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BodyStats {
    /// Bytes as they came off the wire, what a Content-Length promises.
    pub wire_len: u64,
    /// Bytes after transfer decoding, what is hashed and stored.
    pub decoded_len: u64,
}

/// Streams `resp`'s body through its transfer decoding into `sink`, holding one wire chunk and
/// about [`SINK_FLUSH_BYTES`] of decoded bytes in memory at a time, with `cap` enforced on both
/// the wire and the decoded bytes as they pass. Whether the wire bytes fell short of the
/// Content-Length is the caller's to judge from the stats.
pub(crate) async fn stream_body<S: DecodedSink>(
    resp: reqwest::Response,
    cap: usize,
    sink: &mut S,
) -> Result<BodyStats, String> {
    if let Some(len) = resp.content_length()
        && len > cap as u64
    {
        return Err(format!(
            "response body advertises {len} bytes, over the {cap} byte cap"
        ));
    }
    let mut decoder = TransferDecoder::for_encoding(response_content_encoding(&resp).as_deref())?;
    let mut stats = BodyStats {
        wire_len: 0,
        decoded_len: 0,
    };
    let mut run: Vec<u8> = Vec::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        // A body error names the URL, which is stripped as `SendError` strips it from a
        // transport error.
        let chunk = chunk.map_err(|e| e.without_url().to_string())?;
        stats.wire_len += chunk.len() as u64;
        if stats.wire_len > cap as u64 {
            return Err(format!("response body exceeds the {cap} byte cap"));
        }
        for piece in chunk.chunks(decoder.input_step()) {
            // `run` holds decoded bytes not yet counted in `decoded_len`.
            let left = (cap as u64).saturating_sub(stats.decoded_len) as usize;
            decoder.push(piece, &mut run, left).map_err(|e| {
                if run.len() > left {
                    format!("decoded body exceeds {cap} byte cap")
                } else {
                    e
                }
            })?;
            if run.len() >= SINK_FLUSH_BYTES {
                stats.decoded_len += run.len() as u64;
                sink.accept(&run).await?;
                run.clear();
            }
        }
        if !run.is_empty() {
            stats.decoded_len += run.len() as u64;
            sink.accept(&run).await?;
            run.clear();
        }
    }
    decoder.finish(&mut run)?;
    stats.decoded_len += run.len() as u64;
    if stats.decoded_len > cap as u64 {
        return Err(format!("decoded body exceeds {cap} byte cap"));
    }
    if !run.is_empty() {
        sink.accept(&run).await?;
    }
    Ok(stats)
}

/// Decoded file bodies held in memory across every in-flight download, in bytes: what
/// `CONCURRENT_SYNC_DOWNLOADS` bounds by count, `SYNC_INFLIGHT_BYTES_MAX` bounds by size. One
/// [`LiveSyncDeployer`] owns one, so the snapshot workers, the pointer-changes stream and the
/// retry loop draw on the same pool. A hold is taken per download for its Content-Length (an
/// estimate without one, grown as the decoded bytes outrun it) and released once the file is
/// stored; a body larger than the whole budget holds the whole budget, so it proceeds alone.
pub struct ByteBudget {
    permits: Arc<Semaphore>,
    max: u32,
}

impl ByteBudget {
    /// `max_bytes` is clamped to what one hold can ask for (`u32::MAX`, the `acquire_many`
    /// bound), and 0 -- which would stall every download -- is raised to 1.
    pub fn new(max_bytes: u64) -> Self {
        let max = max_bytes
            .clamp(1, u32::MAX as u64)
            .min(Semaphore::MAX_PERMITS as u64) as u32;
        Self {
            permits: Arc::new(Semaphore::new(max as usize)),
            max,
        }
    }

    pub fn max_bytes(&self) -> u64 {
        self.max as u64
    }

    /// Bytes currently held by in-flight downloads.
    pub fn in_use(&self) -> u64 {
        self.max as u64 - self.permits.available_permits() as u64
    }

    pub(crate) async fn reserve(&self, bytes: u64) -> Result<ByteReservation, String> {
        let want = permits_for(bytes, self.max);
        let permit = self
            .permits
            .clone()
            .acquire_many_owned(want)
            .await
            .map_err(|_| "in-flight byte budget closed".to_string())?;
        Ok(ByteReservation {
            budget: self.permits.clone(),
            max: self.max,
            held: want,
            permit,
        })
    }
}

/// Permits a body of `bytes` takes from a budget of `max`: its size, or the whole budget when
/// it is larger.
pub(crate) fn permits_for(bytes: u64, max: u32) -> u32 {
    bytes.min(max as u64) as u32
}

/// Permits a hold of `held` must add to cover `needed` bytes: at least `step` (so a chunked
/// body grows in few asks), never past `max`, and 0 once covered or once the whole budget is
/// held.
pub(crate) fn growth_for(held: u32, needed: u64, max: u32, step: u64) -> u32 {
    if needed <= held as u64 || held >= max {
        return 0;
    }
    let room = (max - held) as u64;
    (needed - held as u64).max(step).min(room) as u32
}

/// One download's hold on a [`ByteBudget`], released on drop.
pub(crate) struct ByteReservation {
    budget: Arc<Semaphore>,
    max: u32,
    held: u32,
    permit: OwnedSemaphorePermit,
}

impl ByteReservation {
    #[cfg(test)]
    pub(crate) fn held(&self) -> u64 {
        self.held as u64
    }

    /// Grows the hold to cover `bytes`, waiting at most [`BUDGET_GROWTH_WAIT`] for the others
    /// to release theirs.
    pub(crate) async fn cover(&mut self, bytes: u64) -> Result<(), String> {
        let extra = growth_for(self.held, bytes, self.max, BUDGET_GROWTH_STEP_BYTES);
        if extra == 0 {
            return Ok(());
        }
        match tokio::time::timeout(
            BUDGET_GROWTH_WAIT,
            self.budget.clone().acquire_many_owned(extra),
        )
        .await
        {
            Ok(Ok(more)) => {
                self.permit.merge(more);
                self.held += extra;
                Ok(())
            }
            Ok(Err(_)) => Err("in-flight byte budget closed".to_string()),
            Err(_) => Err(format!(
                "in-flight byte budget exhausted: holding {} bytes, {} more not granted within {}s",
                self.held,
                extra,
                BUDGET_GROWTH_WAIT.as_secs()
            )),
        }
    }
}

/// The decoded body in memory, held against the budget as it arrives: the Content-Length is
/// reserved at the first decoded run (an estimate without one) and the hold grows as the
/// decoded bytes outrun it.
pub(crate) struct BufferSink<'a> {
    bytes: Vec<u8>,
    budget: Option<&'a ByteBudget>,
    expected_len: Option<u64>,
    reservation: Option<ByteReservation>,
}

impl<'a> BufferSink<'a> {
    pub(crate) fn new(budget: Option<&'a ByteBudget>, expected_len: Option<u64>) -> Self {
        Self {
            bytes: Vec::new(),
            budget,
            expected_len,
            reservation: None,
        }
    }

    /// The body and the hold that covers it, for the caller to release once the bytes are stored.
    pub(crate) fn into_parts(self) -> (Vec<u8>, Option<ByteReservation>) {
        (self.bytes, self.reservation)
    }
}

impl DecodedSink for BufferSink<'_> {
    async fn accept(&mut self, decoded: &[u8]) -> Result<(), String> {
        if let Some(budget) = self.budget
            && self.reservation.is_none()
        {
            let bytes = self.expected_len.unwrap_or(BODY_ESTIMATE_BYTES);
            self.reservation = Some(budget.reserve(bytes).await?);
        }
        self.bytes.extend_from_slice(decoded);
        if let Some(reservation) = self.reservation.as_mut() {
            reservation.cover(self.bytes.len() as u64).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) fn gzip(data: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(data).unwrap();
    enc.finish().unwrap()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CachedEntityVerification {
    Match,
    Mismatch,
    Unverifiable,
}

fn verify_cached_entity(bytes: &[u8], entity_id: &str) -> CachedEntityVerification {
    let ipfs_shape = is_ipfs_v1(entity_id) || is_ipfs_v2(entity_id);
    if !ipfs_shape || !is_canonical_cid(entity_id) {
        return CachedEntityVerification::Unverifiable;
    }
    if verify_hash(bytes, entity_id) {
        CachedEntityVerification::Match
    } else {
        CachedEntityVerification::Mismatch
    }
}

fn eviction_locks() -> &'static parking_lot::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>> {
    static LOCKS: OnceLock<parking_lot::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> =
        OnceLock::new();
    LOCKS.get_or_init(Default::default)
}

async fn evict_corrupt_entity_file(storage: &Storage, entity_id: &str) -> &'static str {
    let lock = {
        let mut map = eviction_locks().lock();
        map.entry(entity_id.to_string()).or_default().clone()
    };

    let outcome = {
        let _guard = lock.lock().await;
        evict_corrupt_entity_file_locked(storage, entity_id).await
    };

    {
        let mut map = eviction_locks().lock();
        if let Some(existing) = map.get(entity_id)
            && Arc::ptr_eq(existing, &lock)
            && Arc::strong_count(existing) == 2
        {
            map.remove(entity_id);
        }
    }

    outcome
}

async fn evict_corrupt_entity_file_locked(storage: &Storage, entity_id: &str) -> &'static str {
    let current = match storage.retrieve(entity_id).await {
        Ok(Some(bytes)) => bytes,
        Ok(None) => {
            return "the corrupt local copy was already removed; \
                    a later retry will re-download it";
        }
        Err(_) => {
            return "the local copy could not be re-verified; \
                    it was kept and a later retry will re-check it";
        }
    };

    if verify_cached_entity(&current, entity_id) != CachedEntityVerification::Mismatch {
        return "the local copy has since been replaced by a hash-valid one, which was kept";
    }

    match storage.delete_strict(entity_id).await {
        Ok(()) => "removed the corrupt local copy so it can be re-downloaded",
        Err(_) => "could not remove the corrupt local copy; a later retry will attempt it again",
    }
}

pub struct ContentSource<'a> {
    pub client: &'a CheckedRedirectClient,
    pub storage: Arc<Storage>,
    pub servers: &'a [String],
    pub content_semaphore: Arc<Semaphore>,
}

pub struct RemoteDeployment<'a> {
    pub entity_id: &'a str,
    pub auth_chain: &'a AuthChain,
    pub context: DeploymentContext,
    pub report: Option<&'a Arc<DeploymentReport>>,
    pub retry_state: Option<(u32, u64)>,
}

pub async fn deploy_entity_streaming(
    source: ContentSource<'_>,
    deployer: &LiveSyncDeployer,
    target: RemoteDeployment<'_>,
) -> Result<(), SyncError> {
    let ContentSource {
        client,
        storage,
        servers,
        content_semaphore,
    } = source;
    let RemoteDeployment {
        entity_id,
        auth_chain,
        context,
        report,
        retry_state,
    } = target;
    // The read-through never removes a file held here until the flush takes the entity.
    let _in_flight = crate::sync_deployer::hold_buffered(entity_id);
    let budget = deployer.inflight_bytes();
    download_file_with_retries(client, storage.as_ref(), entity_id, servers, Some(budget)).await?;

    let entity_data =
        storage
            .retrieve(entity_id)
            .await?
            .ok_or_else(|| SyncError::EntityNotFound {
                entity_id: entity_id.to_string(),
            })?;

    let verification = verify_cached_entity(&entity_data, entity_id);
    if verification == CachedEntityVerification::Mismatch {
        warn!(
            entity_id,
            "Cached entity file failed content-hash verification"
        );
        let outcome = evict_corrupt_entity_file(storage.as_ref(), entity_id).await;
        return Err(SyncError::Other(format!(
            "The stored entity file for {entity_id} failed content-hash verification; {outcome}."
        )));
    }

    if entity_data.is_empty() {
        return Err(entity_parse_error(
            entity_id,
            verification,
            "the stored entity file was empty",
        ));
    }
    let files =
        entity_files(&entity_data).map_err(|e| entity_parse_error(entity_id, verification, e))?;
    let snapshot_hashes: HashSet<&str> = files.snapshots.iter().map(|(_, h)| h.as_str()).collect();

    let mut tasks: tokio::task::JoinSet<Result<(), SyncError>> = tokio::task::JoinSet::new();

    let refs: Vec<&str> = files
        .content
        .iter()
        .map(String::as_str)
        .chain(snapshot_hashes.iter().copied())
        .collect();
    let present = storage.exist_multiple(&refs).await?;
    for (hash, exists) in present {
        if exists {
            continue;
        }

        let client = client.clone();
        let storage = storage.clone();
        let servers = servers.to_vec();
        let sem = content_semaphore.clone();
        let budget = budget.clone();
        let render = snapshot_hashes.contains(hash.as_str());

        tasks.spawn(async move {
            let _permit = sem.acquire().await.map_err(|_| SyncError::Stopped)?;
            if render {
                download_render(&client, &storage, &hash, &servers, Some(&budget))
                    .await
                    .map(drop)
            } else {
                download_file_with_retries(&client, &storage, &hash, &servers, Some(&budget)).await
            }
        });
    }

    let mut first_error: Option<SyncError> = None;
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                if first_error.is_none() {
                    first_error = Some(e);
                }
            }
            Err(e) => {
                if first_error.is_none() {
                    first_error = Some(SyncError::Other(format!("task join error: {}", e)));
                }
            }
        }
    }

    if let Some(e) = first_error {
        return Err(e);
    }

    // A render already stored may be an aged orphan the collector is about to take: touching
    // it restarts the age floor that covers the gap until the flush commits its row.
    let mut snapshots = Vec::with_capacity(files.snapshots.len());
    for (key, hash) in files.snapshots {
        if storage.touch(&hash).await? {
            snapshots.push((key, hash));
        }
    }

    deployer
        .deploy_entity_with_files(
            &entity_data,
            entity_id,
            auth_chain,
            context,
            report,
            retry_state,
            snapshots,
        )
        .await?;

    Ok(())
}

fn entity_parse_error(
    entity_id: &str,
    verification: CachedEntityVerification,
    cause: impl std::fmt::Display,
) -> SyncError {
    let kept = match verification {
        CachedEntityVerification::Match => {
            "the stored bytes match the content hash, so the local copy was kept"
        }
        _ => {
            "the stored bytes could not be proven corrupt (unverifiable hash scheme), \
             so the local copy was kept"
        }
    };
    SyncError::Other(format!(
        "Failed to parse the downloaded entity file for {entity_id}; {kept}. Cause: {cause}"
    ))
}

/// The files sync downloads for an entity.
#[derive(Debug, PartialEq)]
struct EntityFiles {
    /// Every `content[]` hash, once: each must be stored before the entity deploys.
    content: Vec<String>,
    /// A profile's avatar renders named only in its metadata
    /// ([`crate::profile_snapshots::snapshot_files`]): fetched when a peer has them, and
    /// recorded in `content_files` once stored.
    snapshots: Vec<(String, String)>,
}

fn entity_files(entity_data: &[u8]) -> Result<EntityFiles, serde_json::Error> {
    let entity: serde_json::Value = serde_json::from_slice(entity_data)?;
    let declared: Vec<(String, String)> = entity
        .get("content")
        .and_then(|c| c.as_array())
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let hash = entry.get("hash")?.as_str()?;
            let file = entry.get("file").and_then(|f| f.as_str()).unwrap_or("");
            Some((file.to_string(), hash.to_string()))
        })
        .collect();
    let mut seen = HashSet::new();
    let content = declared
        .iter()
        .filter(|(_, hash)| seen.insert(hash.as_str()))
        .map(|(_, hash)| hash.clone())
        .collect();
    let is_profile = entity
        .get("type")
        .and_then(|t| t.as_str())
        .and_then(crate::validator::entity_parser::parse_entity_type)
        == Some(crate::validator::EntityType::Profile);
    let snapshots = if is_profile {
        crate::profile_snapshots::snapshot_files(entity.pointer("/metadata/avatars"), &declared)
    } else {
        Vec::new()
    };
    Ok(EntityFiles { content, snapshots })
}

/// Redirects a content file download follows (a peer may serve its files elsewhere).
pub(crate) const MAX_FILE_REDIRECTS: usize = 2;

/// Tries up to [`MAX_DOWNLOAD_RETRIES`] peers, starting at a random one, and stores the file
/// only once its decoded bytes hash to `hash`. With a `budget`, the decoded body is held
/// against it from its first chunk until the file is stored.
pub async fn download_file_with_retries(
    client: &CheckedRedirectClient,
    storage: &Storage,
    hash: &str,
    servers: &[String],
    budget: Option<&ByteBudget>,
) -> Result<(), SyncError> {
    if storage.exist(hash).await? {
        return Ok(());
    }

    if servers.is_empty() {
        return Err(SyncError::Other("No servers available".into()));
    }

    let mut last_error = None;
    let start = (uuid::Uuid::new_v4().as_u128() % servers.len() as u128) as usize;
    for attempt in 0..MAX_DOWNLOAD_RETRIES {
        let server = &servers[(start + attempt as usize) % servers.len()];
        match fetch_verified(client, hash, server, attempt, budget).await {
            Ok((bytes, reservation)) => {
                storage.store(hash, bytes.into()).await?;
                drop(reservation);
                debug!(hash, "Downloaded content file");
                return Ok(());
            }
            Err(e) => last_error = Some(e),
        }

        if attempt + 1 < MAX_DOWNLOAD_RETRIES {
            tokio::time::sleep(std::time::Duration::from_millis(
                RETRY_WAIT_MS
                    .saturating_mul(1u64 << attempt.min(6))
                    .min(30_000),
            ))
            .await;
        }
    }

    Err(last_error
        .unwrap_or_else(|| SyncError::Other(format!("Failed to download {} after retries", hash))))
}

/// A best-effort file, such as a render a profile names only in its metadata: one request to
/// each of up to [`MAX_DOWNLOAD_RETRIES`] servers, without waiting between them, so a hash no
/// peer has costs no backoff while the caller holds a download permit. `Ok(true)` once stored;
/// `Ok(false)` when no server served it, logged at `warn` unless each answered 404. Only a store
/// fault is an error.
pub(crate) async fn download_render(
    client: &CheckedRedirectClient,
    storage: &Storage,
    hash: &str,
    servers: &[String],
    budget: Option<&ByteBudget>,
) -> Result<bool, SyncError> {
    if storage.exist(hash).await? {
        return Ok(true);
    }
    let start = (uuid::Uuid::new_v4().as_u128() % servers.len().max(1) as u128) as usize;
    let mut failure = None;
    for attempt in 0..servers.len().min(MAX_DOWNLOAD_RETRIES as usize) {
        let server = &servers[(start + attempt) % servers.len()];
        match fetch_verified(client, hash, server, attempt as u32, budget).await {
            Ok((bytes, reservation)) => {
                storage.store(hash, bytes.into()).await?;
                drop(reservation);
                return Ok(true);
            }
            Err(SyncError::NotFound(_)) => {}
            Err(e) => failure = Some(e),
        }
    }
    match failure {
        Some(error) => warn!(hash, %error, "a best-effort file could not be downloaded"),
        None => debug!(hash, "no peer has a best-effort file"),
    }
    Ok(false)
}

/// One `GET {server}/contents/{hash}`: the decoded body, only when it hashes to `hash`, and
/// the budget hold that covers it until the caller has stored it.
pub(crate) async fn fetch_verified(
    client: &CheckedRedirectClient,
    hash: &str,
    server: &str,
    attempt: u32,
    budget: Option<&ByteBudget>,
) -> Result<(Vec<u8>, Option<ByteReservation>), SyncError> {
    let url = format!("{}/contents/{}", server, hash);
    let resp = crate::http::get_following_checked_redirects(client, &url, MAX_FILE_REDIRECTS)
        .await
        .map_err(|e| {
            warn!(hash, %server, error = %e, attempt, "Download request failed");
            SyncError::from(e)
        })?;
    if resp.status().as_u16() == 404 {
        return Err(SyncError::NotFound(format!(
            "404 fetching {} from {}",
            hash, server
        )));
    }
    if !resp.status().is_success() {
        let status = resp.status();
        warn!(hash, %server, %status, attempt, "Download failed");
        return Err(SyncError::Other(format!(
            "HTTP {} fetching {}",
            status, url
        )));
    }
    let expected_len = resp.content_length();
    let mut sink = BufferSink::new(budget, expected_len);
    let stats = stream_body(resp, MAX_BODY_BYTES, &mut sink)
        .await
        .map_err(|reason| {
            warn!(
                hash,
                %server,
                attempt,
                %reason,
                "Content body could not be read, trying next server"
            );
            SyncError::Other(format!("content {} from {}: {}", hash, server, reason))
        })?;
    if let Some(len) = expected_len
        && stats.wire_len < len
    {
        warn!(
            hash,
            %server,
            attempt,
            got = stats.wire_len,
            expected = len,
            "Content download TRUNCATED (short read), trying next server"
        );
        return Err(SyncError::Other(format!(
            "content {} from {} truncated: got {} of {} bytes",
            hash, server, stats.wire_len, len
        )));
    }
    let (bytes, reservation) = sink.into_parts();
    if !verify_hash(&bytes, hash) {
        warn!(
            hash,
            %server,
            attempt,
            "Downloaded content failed hash verification"
        );
        return Err(SyncError::Other(format!(
            "content hash mismatch for {} from {}",
            hash, server
        )));
    }
    Ok((bytes, reservation))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_sync_client_dials_private_addresses_only_for_configured_sources() {
        let hash = "bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenosa7776";
        // `peer.localhost` answers the loopback without DNS and is no configured source.
        let (port, hits) = crate::sync_test_support::redirecting_peer("/contents/{hash}", |port| {
            format!("http://peer.localhost:{port}/target")
        })
        .await;
        let source = format!("http://localhost:{port}");
        let config = crate::config::Config::parse_for_tests(&[
            "x",
            "--database-url",
            "postgres:///catalyst",
            "--sync-source",
            &source,
        ])
        .unwrap();
        let client = crate::sync::sync_http_client(&config).unwrap();
        assert!(
            fetch_verified(&client, hash, &source, 0, None)
                .await
                .is_err()
        );
        assert!(
            client
                .get(format!("http://peer.localhost:{port}/target"))
                .send()
                .await
                .is_err()
        );
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 0);
        let direct = client.get(format!("{source}/target")).send().await.unwrap();
        assert_eq!(direct.status(), 200);
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_snapshot_list_is_never_followed_through_a_redirect() {
        // A same-origin target on a configured source: only the redirect policy refuses it.
        let (port, hits) = crate::sync_test_support::redirecting_peer("/snapshots", |port| {
            format!("http://localhost:{port}/target")
        })
        .await;
        let server = format!("http://localhost:{port}");
        let config = crate::config::Config::parse_for_tests(&[
            "x",
            "--database-url",
            "postgres:///catalyst",
            "--sync-source",
            &server,
        ])
        .unwrap();
        let client = crate::sync::sync_http_client(&config).unwrap();
        assert!(
            crate::sync::snapshots::fetch_snapshots(&client, &server, 1)
                .await
                .is_err()
        );
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn a_file_download_redirect_to_a_private_address_is_refused() {
        use crate::http::{CheckedRedirectClient, PublicAddressResolver, ReservedNamesResolver};
        use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
        // Every name answers the loopback, as a public-looking name can;
        // `lookups` counts those of public.example. The other-origin client is
        // the one passed in, so its lookup goes through this resolver.
        let lookups = Arc::new(AtomicUsize::new(0));
        let counted = lookups.clone();
        let loopback = ReservedNamesResolver::over(move |host| {
            if host == "public.example" {
                counted.fetch_add(1, SeqCst);
            }
            Box::pin(async {
                let addrs = [std::net::SocketAddr::from(([127, 0, 0, 1], 0))];
                Ok(Box::new(addrs.into_iter()) as reqwest::dns::Addrs)
            })
        });
        let client = crate::http::HttpClientCfg::default()
            .builder()
            .dns_resolver(PublicAddressResolver::over(
                loopback.clone(),
                ["peer.example".to_string()],
            ))
            .build()
            .unwrap();
        let client =
            CheckedRedirectClient::new(client, PublicAddressResolver::over(loopback, []), 1)
                .unwrap();
        let hash = "bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenosa7776";
        for target in [
            |port: u16| format!("http://public.example:{port}/target"),
            |port: u16| format!("http://127.0.0.1:{port}/target"),
        ] {
            let (port, hits) =
                crate::sync_test_support::redirecting_peer("/contents/{hash}", target).await;
            let server = format!("http://peer.example:{port}");
            assert!(
                fetch_verified(&client, hash, &server, 0, None)
                    .await
                    .is_err()
            );
            assert_eq!(hits.load(SeqCst), 0);
        }
        assert_eq!(lookups.load(SeqCst), 1);
    }
    use crate::auth::{hash_bytes, hash_bytes_v1};
    #[cfg(unix)]
    use crate::storage::hex_prefix;
    use crate::sync_test_support::{spawn_content_server, temp_storage};

    fn files_of(entity: serde_json::Value) -> EntityFiles {
        entity_files(&serde_json::to_vec(&entity).unwrap()).unwrap()
    }

    #[test]
    fn content_hashes_are_listed_once_in_order() {
        let files = files_of(serde_json::json!({
            "type": "scene",
            "content": [
                {"file": "scene.json", "hash": "bafabc"},
                {"file": "model.glb", "hash": "bafdef"},
                {"file": "copy.glb", "hash": "bafdef"},
            ]
        }));
        assert_eq!(files.content, vec!["bafabc", "bafdef"]);
        assert!(files.snapshots.is_empty());
    }

    /// As snapshots-fetcher: only a profile's renders missing from `content[]` are added, and
    /// only to a profile.
    #[test]
    fn a_profile_adds_the_renders_its_content_does_not_list() {
        let (face, body, face256) = (
            hash_bytes_v1(b"face"),
            hash_bytes_v1(b"body"),
            hash_bytes_v1(b"face256"),
        );
        let entity = |kind: &str| {
            serde_json::json!({
                "type": kind,
                "content": [{"file": "face256.png", "hash": face256}],
                "metadata": {"avatars": [{"avatar": {"snapshots": {
                    "face256": face256,
                    "face": format!("https://peer.example/content/contents/{face}"),
                    "body": body,
                }}}]}
            })
        };
        let mut files = files_of(entity("profile"));
        files.snapshots.sort();
        assert_eq!(files.content, vec![face256.clone()]);
        assert_eq!(
            files.snapshots,
            vec![
                ("body.png".to_string(), body.clone()),
                ("face.png".to_string(), face.clone()),
            ]
        );
        // The type is read as the deployment stores it.
        assert_eq!(files_of(entity("Profiles")).snapshots.len(), 2);
        assert!(files_of(entity("wearable")).snapshots.is_empty());
    }

    const ENTITY_JSON: &[u8] = br#"{"type":"scene","content":[]}"#;
    const CORRUPT_BYTES: &[u8] = br#"{"type":"scene","#;

    #[test]
    fn test_verify_cached_entity_tri_state() {
        let v1 = hash_bytes_v1(ENTITY_JSON);
        let v0 = hash_bytes(ENTITY_JSON);

        assert_eq!(
            verify_cached_entity(ENTITY_JSON, &v1),
            CachedEntityVerification::Match
        );
        assert_eq!(
            verify_cached_entity(ENTITY_JSON, &v0),
            CachedEntityVerification::Match
        );
        assert_eq!(
            verify_cached_entity(CORRUPT_BYTES, &v1),
            CachedEntityVerification::Mismatch
        );
        assert_eq!(
            verify_cached_entity(CORRUPT_BYTES, &v0),
            CachedEntityVerification::Mismatch
        );

        assert_eq!(
            verify_cached_entity(CORRUPT_BYTES, "Qmnotavalidcid"),
            CachedEntityVerification::Unverifiable
        );
        let over_long_v1 = format!("ba{}", "a".repeat(70));
        assert_eq!(
            verify_cached_entity(CORRUPT_BYTES, &over_long_v1),
            CachedEntityVerification::Unverifiable
        );
        assert_eq!(
            verify_cached_entity(CORRUPT_BYTES, "zUnknownScheme"),
            CachedEntityVerification::Unverifiable
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn an_entity_is_held_from_its_download_until_the_attempt_ends() {
        let asked = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Semaphore::new(0));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (on_ask, on_release) = (asked.clone(), release.clone());
        let app = axum::Router::new().route(
            "/contents/{hash}",
            axum::routing::get(move || {
                let (asked, release) = (on_ask.clone(), on_release.clone());
                async move {
                    asked.notify_one();
                    let _open = release.acquire().await;
                    axum::http::StatusCode::NOT_FOUND
                }
            }),
        );
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let (storage, tmp) = temp_storage("in-flight").await;
        let deployer = LiveSyncDeployer::new(
            sqlx::PgPool::connect_lazy("postgres://nobody@127.0.0.1:1/none").unwrap(),
            1,
            crate::sync_deployer::SyncValidation::AuthChainOnly,
        );
        let id = hash_bytes_v1(format!("in-flight-{}", uuid::Uuid::new_v4()).as_bytes());
        let servers = vec![url];
        let (client, chain) = (
            crate::sync::test_sync_client(reqwest::Client::new()),
            Vec::new(),
        );
        let attempt = deploy_entity_streaming(
            ContentSource {
                client: &client,
                storage,
                servers: &servers,
                content_semaphore: Arc::new(tokio::sync::Semaphore::new(1)),
            },
            &deployer,
            RemoteDeployment {
                entity_id: &id,
                auth_chain: &chain,
                context: DeploymentContext::Synced,
                report: None,
                retry_state: None,
            },
        );
        let observe = async {
            asked.notified().await;
            let during = crate::sync_deployer::is_buffered(&id);
            release.add_permits(1);
            during
        };
        let (result, held_during_download) = tokio::join!(attempt, observe);
        assert!(held_during_download, "held while its file downloads");
        assert!(result.is_err());
        assert!(
            !crate::sync_deployer::is_buffered(&id),
            "released after the attempt"
        );
        server.abort();
        let _ = std::fs::remove_dir_all(tmp);
    }

    /// A profile's renders named only in its metadata are fetched and recorded once stored; one
    /// no peer has leaves the entity deployable, without a row, one already held is touched, and
    /// a second render under a key already taken never reaches the flush.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_synced_profile_stores_and_records_the_renders_its_content_omits() {
        let Some(pool) = crate::sync_test_support::test_pool(2).await else {
            return;
        };
        let tag = crate::sync_test_support::unique();
        let blob = |name: &str| format!("{name} {tag}").into_bytes();
        let (face256, body, face, face128, second_body) = (
            blob("face256"),
            blob("body"),
            blob("face"),
            blob("face128"),
            blob("second body"),
        );
        let [
            face256_hash,
            body_hash,
            face_hash,
            face128_hash,
            second_body_hash,
        ] = [&face256, &body, &face, &face128, &second_body].map(|b| hash_bytes_v1(b));
        let pointer = format!("0x{}", &tag[..32]);
        let entity = serde_json::to_vec(&serde_json::json!({
            "version": "v3",
            "type": "profile",
            "pointers": [pointer],
            "timestamp": 1_700_000_000_000u64,
            "content": [{"file": "face256.png", "hash": face256_hash}],
            "metadata": {"avatars": [
                {"avatar": {"snapshots": {
                    "face256": face256_hash, "body": body_hash, "face": face_hash,
                    "face128": face128_hash,
                }}},
                // A second avatar's `body` would reuse `body.png`: the flush must not see it.
                {"avatar": {"snapshots": {"body": second_body_hash}}},
            ]},
        }))
        .unwrap();
        let id = hash_bytes_v1(&entity);
        let (url, server) = crate::sync_test_support::blob_peer(HashMap::from([
            (id.clone(), entity),
            (face256_hash.clone(), face256),
            (body_hash.clone(), body),
            (second_body_hash, second_body),
        ]))
        .await;
        let (storage, tmp) = temp_storage("profile-renders").await;
        // A render the store already holds, past the collector's age floor.
        storage.store(&face128_hash, face128.into()).await.unwrap();
        let held = storage.file_path(&face128_hash).await.unwrap().unwrap();
        std::fs::File::options()
            .write(true)
            .open(&held)
            .unwrap()
            .set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(7200))
            .unwrap();
        let deployer = LiveSyncDeployer::new(
            pool.clone(),
            1,
            crate::sync_deployer::SyncValidation::AuthChainOnly,
        );
        let (client, chain, servers) = (
            crate::sync::test_sync_client(reqwest::Client::new()),
            crate::sync_test_support::signed_chain(&id).await,
            vec![url],
        );
        deploy_entity_streaming(
            ContentSource {
                client: &client,
                storage: storage.clone(),
                servers: &servers,
                content_semaphore: Arc::new(tokio::sync::Semaphore::new(2)),
            },
            &deployer,
            RemoteDeployment {
                entity_id: &id,
                auth_chain: &chain,
                context: DeploymentContext::Synced,
                report: None,
                retry_state: None,
            },
        )
        .await
        .unwrap();
        deployer.flush().await.unwrap();

        assert!(storage.exist(&body_hash).await.unwrap());
        let held_age = std::fs::metadata(&held)
            .unwrap()
            .modified()
            .unwrap()
            .elapsed()
            .unwrap_or_default();
        assert!(
            held_age < std::time::Duration::from_secs(600),
            "a held render is touched before its row is buffered: {held_age:?}"
        );
        let mut rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT cf.key, cf.content_hash FROM content.content_files cf \
             JOIN content.deployments d ON d.id = cf.deployment WHERE d.entity_id = $1",
        )
        .bind(&id)
        .fetch_all(&pool)
        .await
        .unwrap();
        rows.sort();
        assert_eq!(
            rows,
            vec![
                ("body.png".to_string(), body_hash),
                ("face128.png".to_string(), face128_hash),
                ("face256.png".to_string(), face256_hash),
            ]
        );
        server.abort();
        let _ = std::fs::remove_dir_all(tmp);
    }

    /// A stored profile whose only file is a render named in its metadata, and a deployer that is
    /// never reached.
    async fn stored_profile_with_a_render(
        tag: &str,
    ) -> (Arc<Storage>, std::path::PathBuf, String, String, Vec<u8>) {
        let render = format!("render {tag} {}", uuid::Uuid::new_v4()).into_bytes();
        let render_hash = hash_bytes_v1(&render);
        let entity = serde_json::to_vec(&serde_json::json!({
            "type": "profile",
            "pointers": ["0x0000000000000000000000000000000000000001"],
            "timestamp": 1_700_000_000_000u64,
            "content": [],
            "metadata": {"avatars": [{"avatar": {"snapshots": {"body": render_hash}}}]},
        }))
        .unwrap();
        let id = hash_bytes_v1(&entity);
        let (storage, tmp) = temp_storage(tag).await;
        storage.store(&id, entity.into()).await.unwrap();
        (storage, tmp, id, render_hash, render)
    }

    async fn deploy_stored(
        storage: Arc<Storage>,
        id: &str,
        servers: &[String],
        content_semaphore: Arc<Semaphore>,
    ) -> Result<(), SyncError> {
        let deployer = LiveSyncDeployer::new(
            sqlx::PgPool::connect_lazy("postgres://nobody@127.0.0.1:1/none").unwrap(),
            1,
            crate::sync_deployer::SyncValidation::AuthChainOnly,
        );
        let client = crate::sync::test_sync_client(reqwest::Client::new());
        deploy_entity_streaming(
            ContentSource {
                client: &client,
                storage,
                servers,
                content_semaphore,
            },
            &deployer,
            RemoteDeployment {
                entity_id: id,
                auth_chain: &Vec::new(),
                context: DeploymentContext::Synced,
                report: None,
                retry_state: None,
            },
        )
        .await
    }

    /// A render is best-effort, but a stopped sync still fails the attempt.
    #[tokio::test]
    async fn a_stop_while_fetching_a_render_fails_the_deploy() {
        let (storage, tmp, id, _, _) = stored_profile_with_a_render("render-stop").await;
        let closed = Arc::new(Semaphore::new(1));
        closed.close();
        let servers = vec!["http://127.0.0.1:1".to_string()];
        let result = deploy_stored(storage, &id, &servers, closed).await;
        assert!(matches!(result, Err(SyncError::Stopped)), "{result:?}");
        let _ = std::fs::remove_dir_all(tmp);
    }

    /// A render is best-effort, but a store that cannot keep the bytes a peer served fails the
    /// attempt instead of deploying without them.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_store_fault_on_a_render_fails_the_deploy() {
        let (storage, tmp, id, render_hash, render) =
            stored_profile_with_a_render("render-store").await;
        // A dangling link where the render's shard belongs: a read finds it absent, a write
        // cannot create the shard (even as root).
        let shard = storage.root().join(hex_prefix(&render_hash));
        assert_ne!(hex_prefix(&render_hash), hex_prefix(&id));
        std::os::unix::fs::symlink(tmp.join("nowhere"), &shard).unwrap();
        assert!(!storage.exist(&render_hash).await.unwrap());
        let (url, server) =
            crate::sync_test_support::blob_peer(HashMap::from([(render_hash, render)])).await;
        let result = deploy_stored(storage, &id, &[url], Arc::new(Semaphore::new(1))).await;
        assert!(matches!(result, Err(SyncError::Storage(_))), "{result:?}");
        server.abort();
        let _ = std::fs::remove_dir_all(tmp);
    }

    /// A render no peer serves costs one request to each peer, with no backoff between them.
    #[tokio::test]
    async fn a_missing_render_is_asked_of_each_peer_once() {
        let hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut servers = Vec::new();
        let mut handles = Vec::new();
        for _ in 0..2 {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            servers.push(format!("http://{}", listener.local_addr().unwrap()));
            let hits = hits.clone();
            let app = axum::Router::new().route(
                "/contents/{hash}",
                axum::routing::get(move || {
                    hits.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    async { axum::http::StatusCode::NOT_FOUND }
                }),
            );
            handles.push(tokio::spawn(async move {
                axum::serve(listener, app).await.unwrap()
            }));
        }
        let (storage, tmp) = temp_storage("render-404").await;
        let client = crate::sync::test_sync_client(reqwest::Client::new());
        let hash = hash_bytes_v1(b"a render no peer has");
        let started = std::time::Instant::now();
        let stored = download_render(&client, &storage, &hash, &servers, None)
            .await
            .unwrap();
        assert!(!stored);
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 2);
        // A retried download would ask a third time and wait at least 1.5 s.
        assert!(started.elapsed() < std::time::Duration::from_millis(2 * RETRY_WAIT_MS));
        handles.iter().for_each(tokio::task::JoinHandle::abort);
        let _ = std::fs::remove_dir_all(tmp);
    }

    #[tokio::test]
    async fn evict_removes_a_proven_corrupt_copy() {
        const MISMATCH_JSON: &[u8] = br#"{"type":"scene","content":[],"t":"mismatch"}"#;

        let (storage, tmp) = temp_storage("mismatch").await;
        let entity_id = hash_bytes_v1(MISMATCH_JSON);
        storage
            .store(&entity_id, bytes::Bytes::from_static(CORRUPT_BYTES))
            .await
            .unwrap();

        let outcome = evict_corrupt_entity_file(&storage, &entity_id).await;
        assert_eq!(
            outcome,
            "removed the corrupt local copy so it can be re-downloaded"
        );
        assert!(!storage.exist(&entity_id).await.unwrap());

        let _ = tokio::fs::remove_dir_all(&tmp).await;
    }

    #[tokio::test]
    async fn evict_keeps_a_healed_copy() {
        const HEALED_JSON: &[u8] = br#"{"type":"scene","content":[],"t":"healed"}"#;

        let (storage, tmp) = temp_storage("healed").await;
        let entity_id = hash_bytes_v1(HEALED_JSON);
        storage
            .store(&entity_id, bytes::Bytes::from_static(HEALED_JSON))
            .await
            .unwrap();

        let outcome = evict_corrupt_entity_file(&storage, &entity_id).await;
        assert_eq!(
            outcome,
            "the local copy has since been replaced by a hash-valid one, which was kept"
        );
        assert!(storage.exist(&entity_id).await.unwrap());

        let _ = tokio::fs::remove_dir_all(&tmp).await;
    }

    #[tokio::test]
    async fn evict_reports_an_already_removed_copy() {
        const GONE_JSON: &[u8] = br#"{"type":"scene","content":[],"t":"gone"}"#;

        let (storage, tmp) = temp_storage("gone").await;
        let entity_id = hash_bytes_v1(GONE_JSON);

        let outcome = evict_corrupt_entity_file(&storage, &entity_id).await;
        assert_eq!(
            outcome,
            "the corrupt local copy was already removed; \
             a later retry will re-download it"
        );

        let _ = tokio::fs::remove_dir_all(&tmp).await;
    }

    #[tokio::test]
    async fn concurrent_evictions_of_the_same_entity_serialize() {
        const RACE_JSON: &[u8] = br#"{"type":"scene","content":[],"t":"race"}"#;

        let (storage, tmp) = temp_storage("race").await;
        let entity_id = hash_bytes_v1(RACE_JSON);
        storage
            .store(&entity_id, bytes::Bytes::from_static(CORRUPT_BYTES))
            .await
            .unwrap();

        let mut join = tokio::task::JoinSet::new();
        for _ in 0..2 {
            let storage = storage.clone();
            let entity_id = entity_id.clone();
            join.spawn(async move { evict_corrupt_entity_file(&storage, &entity_id).await });
        }
        let mut outcomes: Vec<&'static str> = Vec::new();
        while let Some(res) = join.join_next().await {
            outcomes.push(res.unwrap());
        }
        outcomes.sort();

        assert!(outcomes.contains(&"removed the corrupt local copy so it can be re-downloaded"));
        assert!(!storage.exist(&entity_id).await.unwrap());
        assert!(!eviction_locks().lock().contains_key(&entity_id));

        let _ = tokio::fs::remove_dir_all(&tmp).await;
    }

    #[cfg(unix)]
    fn running_as_root() -> bool {
        unsafe { libc::geteuid() == 0 }
    }

    #[cfg(unix)]
    fn set_mode(path: &std::path::Path, mode: u32) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn evict_reports_failure_when_delete_fails() {
        if running_as_root() {
            eprintln!("skipping: permissions do not bind when running as root");
            return;
        }
        const DELFAIL_JSON: &[u8] = br#"{"type":"scene","content":[],"t":"delfail"}"#;

        let (storage, tmp) = temp_storage("delfail").await;
        let entity_id = hash_bytes_v1(DELFAIL_JSON);
        storage
            .store(&entity_id, bytes::Bytes::from_static(CORRUPT_BYTES))
            .await
            .unwrap();

        let shard_dir = storage.root().join(hex_prefix(&entity_id));
        set_mode(&shard_dir, 0o555);

        let outcome = evict_corrupt_entity_file(&storage, &entity_id).await;
        assert_eq!(
            outcome,
            "could not remove the corrupt local copy; \
             a later retry will attempt it again"
        );
        set_mode(&shard_dir, 0o755);
        assert!(
            storage.exist(&entity_id).await.unwrap(),
            "the copy must still be on disk"
        );

        let outcome = evict_corrupt_entity_file(&storage, &entity_id).await;
        assert_eq!(
            outcome,
            "removed the corrupt local copy so it can be re-downloaded"
        );

        let _ = tokio::fs::remove_dir_all(&tmp).await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn evict_reports_fault_not_removal_when_reverify_cannot_read() {
        if running_as_root() {
            eprintln!("skipping: permissions do not bind when running as root");
            return;
        }

        const REVERIFY_FAULT_JSON: &[u8] = br#"{"type":"scene","content":[],"t":"reverify"}"#;

        let (storage, tmp) = temp_storage("reverify-fault").await;
        let entity_id = hash_bytes_v1(REVERIFY_FAULT_JSON);
        storage
            .store(&entity_id, bytes::Bytes::from_static(CORRUPT_BYTES))
            .await
            .unwrap();

        let shard_dir = storage.root().join(hex_prefix(&entity_id));
        set_mode(&shard_dir, 0o000);

        let outcome = evict_corrupt_entity_file(&storage, &entity_id).await;
        assert_eq!(
            outcome,
            "the local copy could not be re-verified; \
             it was kept and a later retry will re-check it"
        );

        set_mode(&shard_dir, 0o755);
        assert!(storage.exist(&entity_id).await.unwrap());

        let _ = tokio::fs::remove_dir_all(&tmp).await;
    }

    #[tokio::test]
    async fn gzip_labeled_content_decodes_then_hashes_and_stores_decoded() {
        const PAYLOAD: &[u8] = br#"{"type":"scene","content":[],"t":"gzip-wire"}"#;
        let cid = hash_bytes_v1(PAYLOAD);
        let wire = gzip(PAYLOAD);
        let (base, server) = spawn_content_server(wire, Some("gzip")).await;
        let (storage, tmp) = temp_storage("gzip-wire").await;

        download_file_with_retries(
            &crate::sync::test_sync_client(reqwest::Client::new()),
            &storage,
            &cid,
            &[base],
            None,
        )
        .await
        .expect("a gzip-labeled body whose decoded bytes match the CID must be accepted");

        let stored = storage.retrieve(&cid).await.unwrap().unwrap();
        assert_eq!(
            &stored[..],
            PAYLOAD,
            "the stored bytes must be the decoded representation"
        );
        assert!(verify_hash(&stored, &cid));

        server.abort();
        let _ = tokio::fs::remove_dir_all(&tmp).await;
    }

    #[tokio::test]
    async fn gzip_labeled_body_whose_raw_bytes_match_the_cid_is_rejected() {
        const PAYLOAD: &[u8] = br#"{"type":"scene","content":[],"t":"raw-labeled"}"#;
        let cid = hash_bytes_v1(PAYLOAD);
        let (base, server) = spawn_content_server(PAYLOAD.to_vec(), Some("gzip")).await;
        let (storage, tmp) = temp_storage("raw-labeled").await;

        let err = download_file_with_retries(
            &crate::sync::test_sync_client(reqwest::Client::new()),
            &storage,
            &cid,
            &[base],
            None,
        )
        .await
        .expect_err("a non-gzip body labeled gzip must fail transfer decoding");
        assert!(
            err.to_string().contains("gzip transfer decoding failed"),
            "{err}"
        );
        assert!(!storage.exist(&cid).await.unwrap());

        server.abort();
        let _ = tokio::fs::remove_dir_all(&tmp).await;
    }

    #[tokio::test]
    async fn corrupt_gzip_content_still_fails_hash_verification() {
        const PAYLOAD: &[u8] = br#"{"type":"scene","content":[],"t":"expected"}"#;
        const OTHER: &[u8] = br#"{"type":"scene","content":[],"t":"corrupted"}"#;
        let cid = hash_bytes_v1(PAYLOAD);
        let wire = gzip(OTHER);
        let (base, server) = spawn_content_server(wire, Some("gzip")).await;
        let (storage, tmp) = temp_storage("gzip-corrupt").await;

        let err = download_file_with_retries(
            &crate::sync::test_sync_client(reqwest::Client::new()),
            &storage,
            &cid,
            &[base],
            None,
        )
        .await
        .expect_err("decoded bytes that mismatch the CID must still be rejected");
        assert!(err.to_string().contains("content hash mismatch"), "{err}");
        assert!(!storage.exist(&cid).await.unwrap());

        server.abort();
        let _ = tokio::fs::remove_dir_all(&tmp).await;
    }

    fn decode(encoding: Option<&str>, wire: &[u8], piece: usize) -> Result<Vec<u8>, String> {
        let mut decoder = TransferDecoder::for_encoding(encoding)?;
        let mut out = Vec::new();
        for chunk in wire.chunks(piece.max(1)) {
            decoder.push(chunk, &mut out, usize::MAX)?;
        }
        decoder.finish(&mut out)?;
        Ok(out)
    }

    #[test]
    fn a_gzip_bomb_stops_near_the_limit_within_one_wire_chunk() {
        let wire = gzip(&vec![0u8; 64 << 20]);
        assert!(wire.len() < 128 << 10, "{}", wire.len());
        let mut decoder = TransferDecoder::for_encoding(Some("gzip")).unwrap();
        let mut out = Vec::new();
        let err = decoder.push(&wire, &mut out, 1 << 20).unwrap_err();
        assert!(err.contains("exceeds"), "{err}");
        assert!(out.len() < 2 << 20, "inflated {} bytes", out.len());

        let mut decoder = TransferDecoder::for_encoding(Some("gzip")).unwrap();
        let mut out = Vec::new();
        decoder.push(&wire, &mut out, 64 << 20).unwrap();
        decoder.finish(&mut out).unwrap();
        assert_eq!(out.len(), 64 << 20);
    }

    #[test]
    fn identity_and_missing_encodings_pass_through() {
        let body = b"raw bytes".to_vec();
        assert_eq!(decode(None, &body, 3).unwrap(), body);
        assert_eq!(decode(Some("identity"), &body, 3).unwrap(), body);
        assert_eq!(decode(Some(" Identity "), &body, 3).unwrap(), body);
    }

    #[test]
    fn gzip_and_x_gzip_decode_to_the_original_bytes_in_any_chunking() {
        let original = b"### Decentraland json snapshot\n{\"entityId\":\"x\"}\n".to_vec();
        let wire = gzip(&original);
        for piece in [1, 7, wire.len()] {
            assert_eq!(decode(Some("gzip"), &wire, piece).unwrap(), original);
            assert_eq!(decode(Some("X-Gzip"), &wire, piece).unwrap(), original);
        }
    }

    #[test]
    fn non_gzip_bytes_labeled_gzip_fail_decode() {
        let err = decode(Some("gzip"), b"not gzip at all", 64)
            .expect_err("mislabeled body must be rejected, never hashed as-is");
        assert!(err.contains("gzip transfer decoding failed"), "{err}");
    }

    #[test]
    fn unsupported_encoding_is_rejected() {
        for enc in ["br", "zstd", "deflate", "gzip, br"] {
            let err = TransferDecoder::for_encoding(Some(enc))
                .err()
                .expect("unsupported coding must be rejected, never hashed as-is");
            assert!(err.contains("unsupported content-encoding"), "{err}");
        }
    }

    #[tokio::test]
    async fn decoded_size_cap_is_enforced_while_streaming() {
        let wire = gzip(&vec![0u8; 4096]);
        let (base, server) = spawn_content_server(wire, Some("gzip")).await;
        let resp = reqwest::Client::new()
            .get(format!("{base}/contents/bomb"))
            .send()
            .await
            .unwrap();
        let mut sink = BufferSink::new(None, None);
        let err = stream_body(resp, 100, &mut sink)
            .await
            .expect_err("a compression bomb must not outgrow the caller's cap");
        assert!(err.contains("exceeds 100 byte cap"), "{err}");
        server.abort();
    }

    /// Records the largest run handed over, keeping no bytes.
    struct LargestRun(usize, u64);

    impl DecodedSink for LargestRun {
        async fn accept(&mut self, decoded: &[u8]) -> Result<(), String> {
            self.0 = self.0.max(decoded.len());
            self.1 += decoded.len() as u64;
            Ok(())
        }
    }

    #[tokio::test]
    async fn a_highly_compressed_body_reaches_the_sink_in_bounded_runs() {
        let wire = gzip(&vec![0u8; 32 << 20]);
        let (base, server) = spawn_content_server(wire, Some("gzip")).await;
        let resp = reqwest::Client::new()
            .get(format!("{base}/contents/zeros"))
            .send()
            .await
            .unwrap();
        let mut sink = LargestRun(0, 0);
        let stats = stream_body(resp, 64 << 20, &mut sink).await.unwrap();
        assert_eq!((stats.decoded_len, sink.1), (32 << 20, 32 << 20));
        assert!(
            sink.0 < SINK_FLUSH_BYTES + (300 << 10),
            "largest run {}",
            sink.0
        );
        server.abort();
    }

    #[test]
    fn a_hold_is_the_body_size_or_the_whole_budget() {
        assert_eq!(permits_for(0, 100), 0);
        assert_eq!(permits_for(60, 100), 60);
        assert_eq!(permits_for(100, 100), 100);
        assert_eq!(
            permits_for(u64::MAX, 100),
            100,
            "a larger body proceeds alone"
        );
    }

    #[test]
    fn growth_is_stepped_bounded_and_idle_once_covered_or_whole() {
        assert_eq!(growth_for(10, 5, 100, 8), 0, "already covered");
        assert_eq!(growth_for(10, 10, 100, 8), 0, "exactly covered");
        assert_eq!(
            growth_for(10, 11, 100, 8),
            8,
            "one byte over grows by a whole step"
        );
        assert_eq!(
            growth_for(10, 50, 100, 8),
            40,
            "a big jump grows to what is needed"
        );
        assert_eq!(growth_for(10, 500, 100, 8), 90, "never past the budget");
        assert_eq!(
            growth_for(100, 500, 100, 8),
            0,
            "the whole budget held: nothing to ask"
        );
        assert_eq!(
            growth_for(98, 99, 100, 8),
            2,
            "a step is clipped to the room left"
        );
    }

    #[test]
    fn a_budget_is_clamped_to_one_hold_and_never_zero() {
        assert_eq!(ByteBudget::new(0).max_bytes(), 1);
        assert_eq!(ByteBudget::new(1 << 30).max_bytes(), 1 << 30);
        assert_eq!(ByteBudget::new(u64::MAX).max_bytes(), u32::MAX as u64);
    }

    #[tokio::test]
    async fn a_reservation_holds_grows_and_releases() {
        let budget = ByteBudget::new(100);
        let mut first = budget.reserve(60).await.unwrap();
        assert_eq!((first.held(), budget.in_use()), (60, 60));
        first.cover(61).await.unwrap();
        assert_eq!(
            (first.held(), budget.in_use()),
            (100, 100),
            "a growth step past the room left takes the room left"
        );
        first.cover(10_000).await.unwrap();
        assert_eq!(
            first.held(),
            100,
            "holding the whole budget, there is nothing to wait for"
        );
        drop(first);
        assert_eq!(budget.in_use(), 0);
    }

    #[tokio::test]
    async fn a_body_larger_than_the_budget_waits_for_everyone_then_proceeds_alone() {
        let budget = ByteBudget::new(100);
        let holder = budget.reserve(40).await.unwrap();
        let mut big = std::pin::pin!(async { budget.reserve(1_000).await.unwrap().held() });
        assert!(
            futures::poll!(&mut big).is_pending(),
            "the large body waits for the 40 bytes in flight"
        );
        drop(holder);
        assert_eq!(big.await, 100, "then it holds the whole budget");
        assert_eq!(budget.in_use(), 0, "and releases it on drop");
    }
}
