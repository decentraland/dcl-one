//! One remote entity, end to end: fetch the entity file from a peer, prove it
//! by its content hash, fetch every content file it declares (same proof),
//! then hand it to the batching [`LiveSyncDeployer`]. Nothing unproven is ever
//! stored, and a stored entity file that later fails its hash is evicted.

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::sync::{Arc, OnceLock};

use crate::http::{CheckedRedirectClient, TransferRateGuard};
use futures::StreamExt;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tracing::{debug, warn};

use super::batch_deployer::DeploymentReport;
use super::sync::metrics;
use super::sync_deployer::LiveSyncDeployer;
use super::sync_types::{AuthChain, DeploymentContext, SyncError};
use crate::auth::{is_canonical_cid, verify_hash};
use crate::entity::{is_ipfs_v1, is_ipfs_v2};
use crate::storage::Storage;

const MAX_DOWNLOAD_ATTEMPTS: u32 = 10;
const READ_THROUGH_DOWNLOAD_ATTEMPTS: u32 = 3;
const BEST_EFFORT_ATTEMPTS: u32 = 3;

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
    let mut rate = TransferRateGuard::default();
    loop {
        let read_started = tokio::time::Instant::now();
        let Some(chunk) = stream.next().await else {
            break;
        };
        // A body error names the URL, which is stripped as `SendError` strips it from a
        // transport error.
        let chunk = chunk.map_err(|e| e.without_url().to_string())?;
        stats.wire_len += chunk.len() as u64;
        if stats.wire_len > cap as u64 {
            return Err(format!("response body exceeds the {cap} byte cap"));
        }
        rate.check(chunk.len() as u64, read_started.elapsed())
            .map_err(|e| e.to_string())?;
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

/// An entity's entry in [`eviction_locks`], removed on drop (also on unwind or cancellation)
/// once no other eviction holds it.
struct EvictionLock {
    entity_id: String,
    lock: Arc<tokio::sync::Mutex<()>>,
}

impl EvictionLock {
    fn of(entity_id: &str) -> Self {
        let lock = eviction_locks()
            .lock()
            .entry(entity_id.to_string())
            .or_default()
            .clone();
        Self {
            entity_id: entity_id.to_string(),
            lock,
        }
    }
}

impl Drop for EvictionLock {
    fn drop(&mut self) {
        let mut map = eviction_locks().lock();
        if let Some(existing) = map.get(&self.entity_id)
            && Arc::ptr_eq(existing, &self.lock)
            && Arc::strong_count(existing) == 2
        {
            map.remove(&self.entity_id);
        }
    }
}

async fn evict_corrupt_entity_file(storage: &Storage, entity_id: &str) -> &'static str {
    let entry = EvictionLock::of(entity_id);
    let _held = entry.lock.lock().await;
    evict_corrupt_entity_file_locked(storage, entity_id).await
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
        Ok(()) => {
            metrics::corrupt_entity_file_evicted();
            "removed the corrupt local copy so it can be re-downloaded"
        }
        Err(_) => "could not remove the corrupt local copy; a later retry will attempt it again",
    }
}

pub(crate) type CurrentDownloadPeers = Arc<tokio::sync::RwLock<Vec<String>>>;

/// The peers available to a transfer. Sync downloads prefer the peers advertising the
/// deployment and consult the current membership at each attempt.
#[derive(Clone)]
pub struct DownloadPeers {
    advertisers: Arc<[String]>,
    available: AvailablePeers,
}

#[derive(Clone)]
enum AvailablePeers {
    Fixed(Arc<[String]>),
    Current(CurrentDownloadPeers),
}

impl DownloadPeers {
    /// The peers a download job may pick from as it starts.
    async fn candidates(&self) -> usize {
        match &self.available {
            AvailablePeers::Fixed(peers) => peers.len(),
            AvailablePeers::Current(peers) => peers.read().await.len(),
        }
    }

    pub fn fixed(peers: &[String]) -> Self {
        Self {
            advertisers: Arc::default(),
            available: AvailablePeers::Fixed(Arc::from(peers)),
        }
    }

    pub(crate) fn current(advertisers: &[String], peers: CurrentDownloadPeers) -> Self {
        Self {
            advertisers: Arc::from(advertisers),
            available: AvailablePeers::Current(peers),
        }
    }

    async fn next(
        &self,
        tried: &mut HashSet<String>,
        rejected: &HashSet<String>,
        rotation: usize,
        retry: bool,
    ) -> Result<Option<String>, SyncError> {
        match &self.available {
            AvailablePeers::Fixed(peers) => {
                self.next_available(peers, tried, rejected, rotation, retry)
            }
            AvailablePeers::Current(peers) => {
                self.next_available(&peers.read().await, tried, rejected, rotation, retry)
            }
        }
    }

    fn next_available(
        &self,
        available: &[String],
        tried: &mut HashSet<String>,
        rejected: &HashSet<String>,
        rotation: usize,
        retry: bool,
    ) -> Result<Option<String>, SyncError> {
        if available.is_empty() {
            return Err(SyncError::NoServers);
        }
        let first_advertiser = rotation % self.advertisers.len().max(1);
        let first_fallback = rotation % available.len();
        let choose = |tried: &HashSet<String>| {
            let advertisers = self.advertisers[first_advertiser..]
                .iter()
                .chain(&self.advertisers[..first_advertiser])
                .filter(|peer| available.contains(peer));
            let fallback = available[first_fallback..]
                .iter()
                .chain(&available[..first_fallback]);
            advertisers
                .chain(fallback)
                .find(|peer| !tried.contains(peer.as_str()) && !rejected.contains(peer.as_str()))
                .cloned()
        };
        let selected = choose(tried);
        if selected.is_none() && retry && !tried.is_empty() {
            tried.clear();
            return Ok(choose(tried));
        }
        Ok(selected)
    }
}

pub struct ContentSource<'a> {
    pub client: &'a CheckedRedirectClient,
    pub storage: Arc<Storage>,
    pub servers: DownloadPeers,
    pub content_semaphore: Arc<Semaphore>,
}

pub struct RemoteDeployment<'a> {
    pub entity_id: &'a str,
    /// The type the entity was announced with, for the metric labels.
    pub entity_type: &'a str,
    pub auth_chain: &'a AuthChain,
    pub context: DeploymentContext,
    pub report: Option<&'a Arc<DeploymentReport>>,
    pub retry_state: Option<(u32, u64)>,
    /// Refuse with [`SyncError::Overtaken`], before content is fetched or validated, an entity
    /// whose file hashes to its id and that a newer active entity overtook
    /// ([`LiveSyncDeployer::is_overtaken`]).
    pub discard_overtaken: bool,
}

/// A panic in the deployment is returned as an error, so every caller records it like any
/// other failure (backed off and counted against the retry cap) instead of unwinding.
pub async fn deploy_entity_streaming(
    source: ContentSource<'_>,
    deployer: &LiveSyncDeployer,
    target: RemoteDeployment<'_>,
) -> Result<(), SyncError> {
    // Unwind safe: every lock, permit and reservation it holds is RAII or poison-tolerant.
    crate::jobs::catch_panic(deploy_entity_streaming_unguarded(source, deployer, target))
        .await
        .unwrap_or_else(|panic| {
            Err(SyncError::Other(format!(
                "the deployment panicked: {panic}"
            )))
        })
}

async fn deploy_entity_streaming_unguarded(
    source: ContentSource<'_>,
    deployer: &LiveSyncDeployer,
    target: RemoteDeployment<'_>,
) -> Result<(), SyncError> {
    let RemoteDeployment {
        entity_id,
        entity_type,
        auth_chain,
        context,
        report,
        retry_state,
        discard_overtaken,
    } = target;
    // The read-through never removes a file held here until the flush takes the entity.
    let _in_flight = crate::sync_deployer::hold_buffered(entity_id);
    let downloaded = {
        let _pending = metrics::InFlight::download(entity_type);
        download_entity(&source, deployer, entity_id, discard_overtaken).await
    };
    match &downloaded {
        Err(SyncError::Overtaken { .. } | SyncError::Stopped) => {}
        outcome => metrics::entity_downloaded(entity_type, outcome.is_ok()),
    }
    let (entity_data, files) = downloaded?;

    let started = std::time::Instant::now();
    let deployed = deployer
        .deploy_entity_with_files(
            &entity_data,
            entity_id,
            auth_chain,
            context,
            report,
            retry_state,
            Some((source.storage.as_ref(), files.snapshots)),
        )
        .await;
    metrics::entity_deployed(entity_type, started.elapsed(), deployed.is_ok());
    deployed
}

/// The entity file and every content file it needs, stored and verified; the entity's bytes
/// and the files it names.
async fn download_entity(
    source: &ContentSource<'_>,
    deployer: &LiveSyncDeployer,
    entity_id: &str,
    discard_overtaken: bool,
) -> Result<(bytes::Bytes, EntityFiles), SyncError> {
    let ContentSource {
        client,
        storage,
        servers,
        content_semaphore,
    } = source;
    let budget = deployer.inflight_bytes();
    download_file(
        client,
        storage.as_ref(),
        entity_id,
        servers,
        Some(budget),
        DownloadPolicy::Required,
    )
    .await?;

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

    // Only a file proven to hash to its id may discard: any other file is left to validation.
    if discard_overtaken
        && verification == CachedEntityVerification::Match
        && deployer.is_overtaken(&entity_data, entity_id).await?
    {
        return Err(SyncError::Overtaken {
            entity_id: entity_id.to_string(),
        });
    }

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

        let client = (*client).clone();
        let storage = storage.clone();
        let servers = servers.clone();
        let sem = content_semaphore.clone();
        let budget = budget.clone();
        let render = snapshot_hashes.contains(hash.as_str());

        tasks.spawn(async move {
            let _permit = sem.acquire().await.map_err(|_| SyncError::Stopped)?;
            let policy = if render {
                DownloadPolicy::BestEffort
            } else {
                DownloadPolicy::Required
            };
            download_file(&client, &storage, &hash, &servers, Some(&budget), policy)
                .await
                .map(drop)
        });
    }

    let mut first_error: Option<SyncError> = None;
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                // Missing peers leave the deployment unevaluated even if another file failed.
                if first_error.is_none() || matches!(e, SyncError::NoServers) {
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
    Ok((entity_data, files))
}

/// The renders of `entity_id` the store holds, each touched first: a render already stored may
/// be an aged orphan the collector is about to take, and touching it restarts the age floor
/// that covers the gap until the flush commits its row. A render absent from the store, or one
/// the process may not touch (another uid's blob), gets no row; the second is logged at `warn`.
/// Any other store fault is an error.
pub(crate) async fn touch_renders(
    storage: &Storage,
    entity_id: &str,
    renders: Vec<(String, String)>,
) -> Result<Vec<(String, String)>, SyncError> {
    let mut held = Vec::with_capacity(renders.len());
    for (key, hash) in renders {
        match storage.touch(&hash).await {
            Ok(true) => held.push((key, hash)),
            Ok(false) => {}
            Err(crate::storage::StorageError::Io(e))
                if e.kind() == std::io::ErrorKind::PermissionDenied =>
            {
                warn!(entity_id, hash, error = %e, "a stored render could not be touched; it gets no row");
            }
            Err(e) => return Err(e.into()),
        }
    }
    Ok(held)
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

fn entity_files(entity_data: &[u8]) -> Result<EntityFiles, String> {
    let entity: serde_json::Value =
        serde_json::from_slice(entity_data).map_err(|e| e.to_string())?;
    if !entity.is_object() {
        return Err("entity must be a JSON object".into());
    }
    let declared: Vec<_> = entity
        .get("content")
        .map(crate::entity::parse_peer_content)
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or_default()
        .into_iter()
        .map(|entry| (entry.file, entry.hash))
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

/// How [`download_file`] treats a file no server serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadPolicy {
    /// A file the entity needs. Sync allows ten attempts with a one-second wait after a
    /// transient failure. Read-through can try every configured source after definitive
    /// misses, and allows three transient failures with its shorter backoff.
    Required,
    /// A best-effort file, such as a render a profile names only in its metadata. Each of up
    /// to [`BEST_EFFORT_ATTEMPTS`] peers gets one request with no wait between them, so a
    /// missing hash costs no backoff while the caller holds a download permit. No available
    /// copy returns `Ok(false)`. An error beyond a 404 is logged at `warn`. Store faults are
    /// returned.
    BestEffort,
}

/// Downloads `hash` from `servers` and stores the file only once its
/// decoded bytes hash to `hash`; `Ok(true)` once it is stored (or already was). With a
/// `budget`, the decoded body is held against it from its first chunk until the file is
/// stored. `policy` sets the attempts and what a failure answers.
pub async fn download_file(
    client: &CheckedRedirectClient,
    storage: &Storage,
    hash: &str,
    servers: &DownloadPeers,
    budget: Option<&ByteBudget>,
    policy: DownloadPolicy,
) -> Result<bool, SyncError> {
    if storage.exist(hash).await? {
        return Ok(true);
    }
    let required = policy == DownloadPolicy::Required;
    let live_peers = matches!(servers.available, AvailablePeers::Current(_));
    let attempts = match policy {
        DownloadPolicy::Required if live_peers => MAX_DOWNLOAD_ATTEMPTS,
        DownloadPolicy::Required => READ_THROUGH_DOWNLOAD_ATTEMPTS,
        DownloadPolicy::BestEffort => BEST_EFFORT_ATTEMPTS,
    };

    metrics::download_job_started(servers.candidates().await);
    let mut failure = None;
    let rotation = uuid::Uuid::new_v4().as_u128() as usize;
    let mut tried = HashSet::new();
    let mut rejected = HashSet::new();
    let mut attempt = 0;
    let mut request = 0u32;
    let mut delay = None;
    while attempt < attempts {
        let server = match servers
            .next(&mut tried, &rejected, rotation, required)
            .await
        {
            Ok(Some(server)) => server,
            Err(error) if required => return Err(error),
            _ => break,
        };
        if let Some(delay) = delay.take() {
            // Select again after waiting to use the latest peer membership.
            tokio::time::sleep(delay).await;
            continue;
        }
        let result = fetch_verified(client, hash, &server, request, budget).await;
        request = request.saturating_add(1);
        match result {
            Ok((bytes, reservation)) => {
                storage.store(hash, bytes.into()).await?;
                drop(reservation);
                metrics::download_job_succeeded(request);
                debug!(hash, "Downloaded content file");
                return Ok(true);
            }
            Err(error @ (SyncError::NotFound(_) | SyncError::ContentHashMismatch { .. })) => {
                rejected.insert(server);
                if required || matches!(&error, SyncError::ContentHashMismatch { .. }) {
                    failure = Some(error);
                }
                if required && !live_peers {
                    // A definitive miss eliminates one fixed source. The backoff budget
                    // still applies to transient failures.
                    continue;
                }
            }
            Err(error) => {
                tried.insert(server);
                if required {
                    let wait_ms = if live_peers { 1_000 } else { 500 << attempt };
                    delay = Some(std::time::Duration::from_millis(wait_ms));
                }
                failure = Some(error);
            }
        }
        attempt += 1;
    }

    if required {
        return Err(failure.unwrap_or(SyncError::NoServers));
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
    let remote_server = metrics::remote_server(server);
    let started = std::time::Instant::now();
    let fetched = fetch_body(client, hash, server, attempt, budget).await;
    metrics::download_finished(
        &remote_server,
        started.elapsed(),
        fetched.as_ref().ok().map(|(bytes, _)| bytes.len() as u64),
    );
    let (bytes, reservation) = fetched?;
    if !verify_hash(&bytes, hash) {
        metrics::download_hash_error(&remote_server);
        warn!(
            hash,
            %server,
            attempt,
            "Downloaded content failed hash verification"
        );
        return Err(SyncError::ContentHashMismatch {
            hash: hash.to_owned(),
            server: server.to_owned(),
        });
    }
    Ok((bytes, reservation))
}

/// The body of one `GET {server}/contents/{hash}`, whole and decoded, not yet verified.
async fn fetch_body(
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
    Ok(sink.into_parts())
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
    use crate::sync_test_support::{RemoveOnDrop, spawn_content_server, temp_storage};

    fn files_of(entity: serde_json::Value) -> EntityFiles {
        entity_files(&serde_json::to_vec(&entity).unwrap()).unwrap()
    }

    #[test]
    fn content_hashes_are_listed_once_in_order() {
        let first = hash_bytes_v1(b"scene");
        let second = hash_bytes_v1(b"model");
        let files = files_of(serde_json::json!({
            "type": "scene",
            "content": [
                {"file": "scene.json", "hash": first},
                {"file": "model.glb", "hash": second},
                {"file": "copy.glb", "hash": second},
            ]
        }));
        assert_eq!(files.content, vec![first, second]);
        assert!(files.snapshots.is_empty());
    }

    #[test]
    fn local_and_synced_manifests_accept_omitted_content_and_historical_cids() {
        let base = serde_json::json!({"type":"scene", "pointers":["0,0"], "timestamp":1});
        let bytes = serde_json::to_vec(&base).unwrap();
        assert!(entity_files(&bytes).unwrap().content.is_empty());
        assert!(
            serde_json::from_slice::<crate::entity::Entity>(&bytes)
                .unwrap()
                .content
                .is_empty()
        );
        let legacy = hash_bytes(b"historical content");
        let mut entity = base;
        entity["content"] = serde_json::json!([{"file":"a.txt","hash":legacy}]);
        let bytes = serde_json::to_vec(&entity).unwrap();
        assert_eq!(entity_files(&bytes).unwrap().content, vec![legacy.clone()]);
        assert_eq!(
            serde_json::from_slice::<crate::entity::Entity>(&bytes)
                .unwrap()
                .content[0]
                .hash,
            legacy
        );
    }

    #[test]
    fn peer_replay_reads_legacy_null_content_as_empty() {
        let entity = br#"{"type":"scene","pointers":["0,0"],"timestamp":1,"content":null}"#;

        assert!(entity_files(entity).unwrap().content.is_empty());
        assert!(
            crate::validator::entity_parser::parse_entity_from_bytes(entity, "id")
                .unwrap()
                .content
                .is_empty()
        );
        assert!(serde_json::from_slice::<crate::entity::Entity>(entity).is_err());
    }

    #[test]
    fn local_and_synced_manifests_reject_the_same_invalid_content() {
        for (content, explanation) in [
            (serde_json::json!("not an array"), "array"),
            (
                serde_json::json!([{"file":"missing-hash"}]),
                "valid strings",
            ),
            (
                serde_json::json!([{"file":"invalid","hash":"not-a-cid"}]),
                "invalid hash",
            ),
        ] {
            let bytes = serde_json::to_vec(&serde_json::json!({
                "type":"scene", "pointers":["0,0"], "timestamp":1, "content":content
            }))
            .unwrap();
            assert!(entity_files(&bytes).unwrap_err().contains(explanation));
            assert!(
                serde_json::from_slice::<crate::entity::Entity>(&bytes)
                    .unwrap_err()
                    .to_string()
                    .contains(explanation)
            );
            assert!(
                crate::validator::entity_parser::parse_entity_from_bytes(&bytes, "id")
                    .unwrap_err()
                    .to_string()
                    .contains(explanation)
            );
        }
    }

    #[test]
    fn content_file_cap_counts_declarations_before_deduplicating_hashes() {
        let hash = hash_bytes_v1(b"duplicate");
        let entry = serde_json::json!({"file": "a", "hash": hash});
        let mut entries = vec![entry.clone(); 25_000];
        assert_eq!(
            files_of(serde_json::json!({"content": entries})).content,
            vec![hash.clone()]
        );
        entries.push(entry);
        let bytes = serde_json::to_vec(&serde_json::json!({
            "type":"scene", "pointers":["0,0"], "timestamp":1, "content": entries
        }))
        .unwrap();
        assert!(
            entity_files(&bytes)
                .unwrap_err()
                .contains("maximum of 25000")
        );
        assert!(
            serde_json::from_slice::<crate::entity::Entity>(&bytes)
                .unwrap_err()
                .to_string()
                .contains("maximum of 25000")
        );
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
        let deployer = LiveSyncDeployer::without_store(
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
                servers: DownloadPeers::fixed(&servers),
                content_semaphore: Arc::new(tokio::sync::Semaphore::new(1)),
            },
            &deployer,
            RemoteDeployment {
                entity_id: &id,
                entity_type: "scene",
                auth_chain: &chain,
                context: DeploymentContext::Synced,
                report: None,
                retry_state: None,
                discard_overtaken: false,
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
        let _tmp = RemoveOnDrop(tmp.clone());
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
            storage.clone(),
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
                servers: DownloadPeers::fixed(&servers),
                content_semaphore: Arc::new(tokio::sync::Semaphore::new(2)),
            },
            &deployer,
            RemoteDeployment {
                entity_id: &id,
                entity_type: "scene",
                auth_chain: &chain,
                context: DeploymentContext::Synced,
                report: None,
                retry_state: None,
                discard_overtaken: false,
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
            "a held render is touched before the entity is buffered: {held_age:?}"
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
            storage.clone(),
            1,
            crate::sync_deployer::SyncValidation::AuthChainOnly,
        );
        let client = crate::sync::test_sync_client(reqwest::Client::new());
        deploy_entity_streaming(
            ContentSource {
                client: &client,
                storage,
                servers: DownloadPeers::fixed(servers),
                content_semaphore,
            },
            &deployer,
            RemoteDeployment {
                entity_id: id,
                entity_type: "scene",
                auth_chain: &Vec::new(),
                context: DeploymentContext::Synced,
                report: None,
                retry_state: None,
                discard_overtaken: false,
            },
        )
        .await
    }

    /// A render is best-effort, but a stopped sync still fails the attempt.
    #[tokio::test]
    async fn a_stop_while_fetching_a_render_fails_the_deploy() {
        let (storage, tmp, id, _, _) = stored_profile_with_a_render("render-stop").await;
        let _tmp = RemoveOnDrop(tmp);
        let closed = Arc::new(Semaphore::new(1));
        closed.close();
        let servers = vec!["http://127.0.0.1:1".to_string()];
        let result = deploy_stored(storage, &id, &servers, closed).await;
        assert!(matches!(result, Err(SyncError::Stopped)), "{result:?}");
    }

    /// A render is best-effort, but a store that cannot keep the bytes a peer served fails the
    /// attempt instead of deploying without them.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_store_fault_on_a_render_fails_the_deploy() {
        let (storage, tmp, id, render_hash, render) =
            stored_profile_with_a_render("render-store").await;
        let _tmp = RemoveOnDrop(tmp.clone());
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
    }

    /// An entity whose signature does not verify never touches the renders it names: the touch
    /// runs only once the entity is verified, so an unsigned entity cannot keep an orphan alive.
    #[tokio::test]
    async fn an_unverified_profile_leaves_its_held_render_aged() {
        let (storage, tmp, id, render_hash, render) =
            stored_profile_with_a_render("render-unverified").await;
        let _tmp = RemoveOnDrop(tmp);
        storage.store(&render_hash, render.into()).await.unwrap();
        let held = storage.file_path(&render_hash).await.unwrap().unwrap();
        let aged = std::time::SystemTime::now() - std::time::Duration::from_secs(7200);
        std::fs::File::options()
            .write(true)
            .open(&held)
            .unwrap()
            .set_modified(aged)
            .unwrap();

        let result = deploy_stored(storage, &id, &[], Arc::new(Semaphore::new(1))).await;
        assert!(
            matches!(&result, Err(SyncError::DeploymentRejected(reason))
                if reason.contains("auth chain verification failed")),
            "the signature check rejects it: {result:?}"
        );
        assert_eq!(std::fs::metadata(&held).unwrap().modified().unwrap(), aged);
    }

    /// A render the process may not touch gets no row and fails nothing; an absent one gets no
    /// row either. `refused` is another uid's `0644` file the store can stat, so its refusal
    /// comes from setting its times; `hidden` sits in a shard the process may not enter, so its
    /// refusal comes from the stat.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_render_that_cannot_be_touched_is_skipped() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        if running_as_root() {
            eprintln!("skipping: permissions do not bind when running as root");
            return;
        }
        let (storage, tmp) = temp_storage("render-untouchable").await;
        let _tmp = RemoveOnDrop(tmp);
        let own_uid = std::fs::metadata(storage.root()).unwrap().uid();
        let foreign = std::path::Path::new("/etc/passwd");
        match std::fs::metadata(foreign) {
            Ok(meta) if meta.is_file() && meta.uid() != own_uid => {}
            _ => {
                eprintln!(
                    "skipping: no file owned by another uid at {}",
                    foreign.display()
                );
                return;
            }
        }
        let [held, refused, hidden, absent] =
            [b"held".as_slice(), b"refused", b"hidden", b"absent"].map(hash_bytes_v1);
        assert_ne!(hex_prefix(&held), hex_prefix(&hidden));
        assert_ne!(hex_prefix(&refused), hex_prefix(&hidden));
        for (hash, bytes) in [(&held, "held"), (&refused, "refused"), (&hidden, "hidden")] {
            storage.store(hash, bytes.as_bytes().into()).await.unwrap();
        }
        let refused_path = crate::storage::resolve_file_path(storage.root(), &refused).unwrap();
        std::fs::remove_file(&refused_path).unwrap();
        std::os::unix::fs::symlink(foreign, &refused_path).unwrap();
        assert!(storage.exist(&refused).await.unwrap(), "the stat passes");
        assert!(
            matches!(storage.touch(&refused).await,
                Err(crate::storage::StorageError::Io(e)) if e.kind() == std::io::ErrorKind::PermissionDenied),
            "setting the times is refused"
        );
        let shard = storage.root().join(hex_prefix(&hidden));
        std::fs::set_permissions(&shard, std::fs::Permissions::from_mode(0o000)).unwrap();

        let renders = vec![
            ("face.png".to_string(), held.clone()),
            ("body.png".to_string(), refused),
            ("face256.png".to_string(), hidden),
            ("body256.png".to_string(), absent),
        ];
        let touched = touch_renders(&storage, "entity", renders).await;
        std::fs::set_permissions(&shard, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(touched.unwrap(), vec![("face.png".to_string(), held)]);
    }

    /// A best-effort file a peer fails on (an error status, not a 404) is no error, and nor is
    /// having no servers; a required file with no servers is.
    #[tokio::test]
    async fn a_best_effort_file_survives_a_failing_peer_and_no_peers() {
        let hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let servers = vec![format!("http://{}", listener.local_addr().unwrap())];
        let counted = hits.clone();
        let app = axum::Router::new().route(
            "/contents/{hash}",
            axum::routing::get(move || {
                counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                async { axum::http::StatusCode::INTERNAL_SERVER_ERROR }
            }),
        );
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let (storage, tmp) = temp_storage("render-500").await;
        let _tmp = RemoveOnDrop(tmp);
        let client = crate::sync::test_sync_client(reqwest::Client::new());
        let hash = hash_bytes_v1(b"a render a peer fails on");

        let best_effort = |servers: Vec<String>| {
            let (client, storage, hash) = (&client, &storage, &hash);
            async move {
                download_file(
                    client,
                    storage,
                    hash,
                    &DownloadPeers::fixed(&servers),
                    None,
                    DownloadPolicy::BestEffort,
                )
                .await
            }
        };
        assert!(!best_effort(servers).await.unwrap());
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(!best_effort(Vec::new()).await.unwrap());
        let required = download_file(
            &client,
            &storage,
            &hash,
            &DownloadPeers::fixed(&[]),
            None,
            DownloadPolicy::Required,
        )
        .await;
        assert!(
            matches!(required, Err(SyncError::NoServers)),
            "{required:?}"
        );
        server.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn a_download_tries_its_advertiser_first_and_follows_a_peer_change() {
        use crate::sync_test_support::{RemoveOnDrop, temp_storage};
        use axum::{Router, http::StatusCode, routing::get};

        const PAYLOAD: &[u8] = b"served by the new peer";
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let (removed, advertiser, added) = (
            format!("{base}/removed"),
            format!("{base}/advertiser"),
            format!("{base}/added"),
        );
        let peers = CurrentDownloadPeers::default();
        *peers.write().await = vec![removed, advertiser.clone()];
        let servers = DownloadPeers::current(std::slice::from_ref(&advertiser), peers.clone());
        let hits = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let advertiser_hits = hits.clone();
        let removed_hits = hits.clone();
        let added_hits = hits.clone();
        let app = Router::new()
            .route(
                "/removed/contents/{hash}",
                get(move || {
                    removed_hits.lock().push("removed");
                    async { PAYLOAD }
                }),
            )
            .route(
                "/advertiser/contents/{hash}",
                get(move || {
                    let (peers, added) = (peers.clone(), added.clone());
                    advertiser_hits.lock().push("advertiser");
                    async move {
                        *peers.write().await = vec![added];
                        StatusCode::NOT_FOUND
                    }
                }),
            )
            .route(
                "/added/contents/{hash}",
                get(move || {
                    added_hits.lock().push("added");
                    async { PAYLOAD }
                }),
            );
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let (storage, tmp) = temp_storage("advertising-peer").await;
        let _tmp = RemoveOnDrop(tmp);
        let hash = hash_bytes_v1(PAYLOAD);
        assert!(
            download_file(
                &crate::sync::test_sync_client(reqwest::Client::new()),
                &storage,
                &hash,
                &servers,
                None,
                DownloadPolicy::Required,
            )
            .await
            .unwrap()
        );
        assert_eq!(*hits.lock(), ["advertiser", "added"]);
        server.abort();
    }

    #[tokio::test]
    async fn first_attempts_rotate_across_current_advertisers_before_using_fallback() {
        let first = "https://first.example".to_owned();
        let second = "https://second.example".to_owned();
        let fallback = "https://fallback.example".to_owned();
        let membership = vec![fallback, first.clone(), second.clone()];
        let current = Arc::new(tokio::sync::RwLock::new(membership));
        let peers = DownloadPeers::current(&[first.clone(), second.clone()], current);

        for (rotation, expected) in [first, second].into_iter().enumerate() {
            let selected = peers
                .next(&mut HashSet::new(), &HashSet::new(), rotation, true)
                .await
                .unwrap();
            assert_eq!(selected, Some(expected));
        }
    }

    #[tokio::test]
    async fn a_removed_advertiser_is_ineligible_for_the_first_attempt() {
        let removed = "https://removed.example".to_owned();
        let fallback = "https://fallback.example".to_owned();
        let current = Arc::new(tokio::sync::RwLock::new(vec![fallback.clone()]));
        let peers = DownloadPeers::current(&[removed], current);

        let selected = peers
            .next(&mut HashSet::new(), &HashSet::new(), 0, true)
            .await
            .unwrap();
        assert_eq!(selected, Some(fallback));
    }

    #[tokio::test(start_paused = true)]
    async fn a_required_download_skips_peers_that_returned_a_definitive_failure() {
        use axum::{Router, http::StatusCode, routing::get};
        use std::sync::atomic::{AtomicUsize, Ordering};

        let missing = Arc::new(AtomicUsize::new(0));
        let corrupt = Arc::new(AtomicUsize::new(0));
        let missing_hits = missing.clone();
        let corrupt_hits = corrupt.clone();
        let app = Router::new()
            .route(
                "/missing/contents/{hash}",
                get(move || {
                    missing_hits.fetch_add(1, Ordering::Relaxed);
                    async { StatusCode::NOT_FOUND }
                }),
            )
            .route(
                "/corrupt/contents/{hash}",
                get(move || {
                    corrupt_hits.fetch_add(1, Ordering::Relaxed);
                    async { "bytes with a different hash" }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let current = Arc::new(tokio::sync::RwLock::new(vec![
            format!("{base}/missing"),
            format!("{base}/corrupt"),
        ]));
        let (storage, tmp) = temp_storage("definitive-download-errors").await;
        let _tmp = RemoveOnDrop(tmp);
        let hash = hash_bytes_v1(b"valid content");
        let outcome = download_file(
            &crate::sync::test_sync_client(reqwest::Client::new()),
            &storage,
            &hash,
            &DownloadPeers::current(&[], current),
            None,
            DownloadPolicy::Required,
        )
        .await;

        assert!(
            matches!(
                outcome,
                Err(SyncError::NotFound(_) | SyncError::ContentHashMismatch { .. })
            ),
            "{outcome:?}"
        );
        assert_eq!(missing.load(Ordering::Relaxed), 1);
        assert_eq!(corrupt.load(Ordering::Relaxed), 1);
        server.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn a_required_download_tries_ten_times_in_sync_and_three_in_read_through() {
        use crate::sync_test_support::{RemoveOnDrop, temp_storage};
        use axum::{Router, http::StatusCode, routing::get};
        use std::sync::atomic::{AtomicUsize, Ordering};

        let hits = Arc::new(AtomicUsize::new(0));
        let counted = hits.clone();
        let app = Router::new().route(
            "/contents/{hash}",
            get(move || {
                counted.fetch_add(1, Ordering::Relaxed);
                async { StatusCode::SERVICE_UNAVAILABLE }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let (storage, tmp) = temp_storage("download-attempts").await;
        let _tmp = RemoveOnDrop(tmp);
        let hash = hash_bytes_v1(b"a required file the peer never serves");
        let current = Arc::new(tokio::sync::RwLock::new(vec![base.clone()]));
        let modes = [
            (DownloadPeers::current(&[], current), 10),
            (DownloadPeers::fixed(&[base]), 3),
        ];
        for (peers, expected) in modes {
            let outcome = download_file(
                &crate::sync::test_sync_client(reqwest::Client::new()),
                &storage,
                &hash,
                &peers,
                None,
                DownloadPolicy::Required,
            )
            .await;
            assert!(matches!(outcome, Err(SyncError::Other(_))), "{outcome:?}");
            assert_eq!(hits.swap(0, Ordering::Relaxed), expected);
        }
        server.abort();
    }

    #[tokio::test]
    async fn read_through_reaches_the_next_source_after_three_definitive_misses() {
        use axum::{Router, extract::Path, http::StatusCode, routing::get};

        let hits = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let observed = hits.clone();
        let app = Router::new().route(
            "/{peer}/contents/{hash}",
            get(move |Path((peer, _hash)): Path<(String, String)>| {
                let mut hits = observed.lock();
                hits.push(peer);
                let response = match hits.len() {
                    1 | 3 => (StatusCode::NOT_FOUND, ""),
                    2 => (StatusCode::OK, "wrong content"),
                    _ => (StatusCode::OK, "fresh content"),
                };
                async move { response }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let peers = (0..4)
            .map(|peer| format!("{base}/{peer}"))
            .collect::<Vec<_>>();
        let (storage, tmp) = temp_storage("read-through-definitive-misses").await;
        let _cleanup = RemoveOnDrop(tmp);
        let hash = hash_bytes_v1(b"fresh content");

        let copied = download_file(
            &crate::sync::test_sync_client(reqwest::Client::new()),
            &storage,
            &hash,
            &DownloadPeers::fixed(&peers),
            None,
            DownloadPolicy::Required,
        )
        .await
        .unwrap();

        assert!(copied);
        assert!(storage.exist(&hash).await.unwrap());
        let hits = hits.lock();
        assert_eq!(hits.len(), 4);
        assert_eq!(hits.iter().collect::<HashSet<_>>().len(), 4);
        server.abort();
    }

    /// Every request records its peer's origin, time and bytes or error; a body that does not
    /// hash to its name is a hash error; the job records its candidate peers and its attempts.
    #[tokio::test]
    async fn downloads_report_the_legacy_download_series() {
        let body = b"a file a peer serves".to_vec();
        let hash = hash_bytes_v1(&body);
        let (good, good_server) = spawn_content_server(body.clone(), None).await;
        let (bad, bad_server) = spawn_content_server(b"other bytes".to_vec(), None).await;
        let (storage, tmp) = temp_storage("download-metrics").await;
        let _tmp = RemoveOnDrop(tmp);
        let client = crate::sync::test_sync_client(reqwest::Client::new());
        let (_metrics, handle) = metrics::local_recorder();

        assert!(fetch_verified(&client, &hash, &bad, 0, None).await.is_err());
        let servers = DownloadPeers::fixed(std::slice::from_ref(&good));
        assert!(
            download_file(
                &client,
                &storage,
                &hash,
                &servers,
                None,
                DownloadPolicy::Required
            )
            .await
            .unwrap()
        );

        let rendered = handle.render();
        let (good, bad) = (metrics::remote_server(&good), metrics::remote_server(&bad));
        for (series, value) in [
            (
                format!("dcl_content_download_hash_errors_total{{remote_server=\"{bad}\"}}"),
                1.0,
            ),
            (
                format!("dcl_content_download_bytes_total{{remote_server=\"{good}\"}}"),
                body.len() as f64,
            ),
            (
                format!("dcl_content_download_duration_seconds_count{{remote_server=\"{good}\"}}"),
                1.0,
            ),
            ("dcl_available_servers_histogram_sum".into(), 1.0),
            ("dcl_content_download_job_succeed_retries_sum".into(), 1.0),
        ] {
            assert_eq!(
                metrics::sample(&rendered, &series),
                Some(value),
                "{series}:\n{rendered}"
            );
        }
        good_server.abort();
        bad_server.abort();
    }

    /// A render no peer serves costs one request to each peer, with no wait between them.
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
        let _tmp = RemoveOnDrop(tmp.clone());
        let client = crate::sync::test_sync_client(reqwest::Client::new());
        let hash = hash_bytes_v1(b"a render no peer has");
        let stored = download_file(
            &client,
            &storage,
            &hash,
            &DownloadPeers::fixed(&servers),
            None,
            DownloadPolicy::BestEffort,
        )
        .await
        .unwrap();
        assert!(!stored);
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 2);
        handles.iter().for_each(tokio::task::JoinHandle::abort);
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

    #[test]
    fn an_eviction_that_unwinds_leaves_no_lock_entry() {
        let entity_id = format!("bafy-eviction-unwinds-{}", std::process::id());
        let unwound = std::panic::catch_unwind(|| {
            let _entry = EvictionLock::of(&entity_id);
            assert!(eviction_locks().lock().contains_key(&entity_id));
            panic!("eviction panicked");
        });
        assert!(unwound.is_err());
        assert!(!eviction_locks().lock().contains_key(&entity_id));
    }

    #[test]
    fn an_eviction_lock_another_eviction_holds_stays_until_both_end() {
        let entity_id = format!("bafy-eviction-shared-{}", std::process::id());
        let first = EvictionLock::of(&entity_id);
        let second = EvictionLock::of(&entity_id);
        assert!(Arc::ptr_eq(&first.lock, &second.lock));
        drop(first);
        assert!(eviction_locks().lock().contains_key(&entity_id));
        drop(second);
        assert!(!eviction_locks().lock().contains_key(&entity_id));
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

        download_file(
            &crate::sync::test_sync_client(reqwest::Client::new()),
            &storage,
            &cid,
            &DownloadPeers::fixed(&[base]),
            None,
            DownloadPolicy::Required,
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

    #[tokio::test(start_paused = true)]
    async fn gzip_labeled_body_whose_raw_bytes_match_the_cid_is_rejected() {
        const PAYLOAD: &[u8] = br#"{"type":"scene","content":[],"t":"raw-labeled"}"#;
        let cid = hash_bytes_v1(PAYLOAD);
        let (base, server) = spawn_content_server(PAYLOAD.to_vec(), Some("gzip")).await;
        let (storage, tmp) = temp_storage("raw-labeled").await;

        let err = download_file(
            &crate::sync::test_sync_client(reqwest::Client::new()),
            &storage,
            &cid,
            &DownloadPeers::fixed(&[base]),
            None,
            DownloadPolicy::Required,
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

    #[tokio::test(start_paused = true)]
    async fn corrupt_gzip_content_still_fails_hash_verification() {
        const PAYLOAD: &[u8] = br#"{"type":"scene","content":[],"t":"expected"}"#;
        const OTHER: &[u8] = br#"{"type":"scene","content":[],"t":"corrupted"}"#;
        let cid = hash_bytes_v1(PAYLOAD);
        let wire = gzip(OTHER);
        let (base, server) = spawn_content_server(wire, Some("gzip")).await;
        let (storage, tmp) = temp_storage("gzip-corrupt").await;

        let err = download_file(
            &crate::sync::test_sync_client(reqwest::Client::new()),
            &storage,
            &cid,
            &DownloadPeers::fixed(&[base]),
            None,
            DownloadPolicy::Required,
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

    #[tokio::test]
    async fn a_slow_gzip_stream_fails_even_when_no_decoded_bytes_reach_the_sink() {
        use axum::{Router, body::Body, http::header, response::IntoResponse, routing::get};
        use std::time::Duration;

        let (tx, rx) = tokio::sync::mpsc::channel::<Vec<u8>>(1);
        let rx = Arc::new(tokio::sync::Mutex::new(Some(rx)));
        let app = Router::new().route(
            "/",
            get(move || async move {
                let rx = rx.lock().await.take().unwrap();
                let chunks = futures::stream::unfold(rx, |mut rx| async {
                    rx.recv()
                        .await
                        .map(|chunk| (Ok::<_, std::convert::Infallible>(chunk), rx))
                });
                (
                    [(header::CONTENT_ENCODING, "gzip")],
                    Body::from_stream(chunks),
                )
                    .into_response()
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        tx.send(gzip(b"")).await.unwrap();
        let resp = reqwest::Client::new().get(url).send().await.unwrap();

        tokio::time::pause();
        let mut sink = LargestRun(0, 0);
        let mut transfer = Box::pin(stream_body(resp, 1024, &mut sink));
        assert!(futures::poll!(&mut transfer).is_pending());
        tokio::time::advance(Duration::from_secs(61)).await;
        tx.send(gzip(b"")).await.unwrap();
        drop(tx);
        let err = transfer.await.unwrap_err();
        assert!(err.contains("fell below 4096 bytes/s"), "{err}");
        assert_eq!(sink.1, 0);
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
