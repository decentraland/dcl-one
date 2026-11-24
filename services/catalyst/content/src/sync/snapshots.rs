//! A peer's `/snapshots` list and the snapshot files it names: validated
//! metadata, the processed/replaced decision, the hash-verified download
//! into a temporary file, and the line-by-line hand-off of every entity to
//! the batch deployer.

use std::collections::HashSet;
use std::io::{BufRead, Read};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use futures::StreamExt;
use reqwest::Client;
use sha2::Digest;
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use super::pointer_changes::is_usable_timestamp;
use super::{SnapshotMetadata, SyncDeployment, SyncError, TimeRange, Timestamp};
use crate::auth::{HashV1Writer, is_canonical_cid};
use crate::batch_deployer::{BatchDeployer, DeploymentReport};
use crate::deploy_remote_entity::{BodyStats, DecodedSink, stream_body};
use crate::http::read_peer_body_capped;
use crate::storage::{StagingGuard, Storage, create_staging_file, resolve_file_path, staging_path};

/// Cap on a snapshot body, wire and decoded, enforced as it streams to disk.
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024 * 1024;
/// `GET /snapshots` lists a handful of snapshot descriptors per entity type.
const MAX_SNAPSHOT_LIST_BYTES: usize = 16 * 1024 * 1024;

const MAX_DECOMPRESSED_SNAPSHOT_BYTES: u64 = 4 * 1024 * 1024 * 1024;

/// The maximum number of bytes in a snapshot line, excluding its newline.
const MAX_SNAPSHOT_LINE_BYTES: usize = 10 * 1024 * 1024;

const MAX_SNAPSHOT_LINE_ERROR_LOGS: u64 = 100;

/// Records parsed ahead of the deployer per batch: what one scheduling round holds in memory
/// (an entity's pointers and auth chain each) instead of a snapshot's every line.
const SNAPSHOT_BATCH_RECORDS: usize = 1000;

/// Upstream MAX_REPLACED_SNAPSHOT_HASHES. Every entry lands in the batched processed-snapshots
/// lookup, so a pathological list sizes that query; real snapshots replace tens.
const MAX_REPLACED_SNAPSHOT_HASHES: usize = 1000;

/// How many invalid /snapshots entries are logged in detail per response before the rest
/// collapse into one summary line -- the body is attacker-sized.
const MAX_INVALID_SNAPSHOT_LOGS: usize = 5;
/// Longest logged preview of an invalid snapshot entry, in bytes.
const SNAPSHOT_PREVIEW_BYTES: usize = 512;

/// The logged form of an invalid entry, cut on a character boundary at or
/// below [`SNAPSHOT_PREVIEW_BYTES`].
fn entry_preview(item: &serde_json::Value) -> String {
    let mut preview = item.to_string();
    preview.truncate(preview.floor_char_boundary(SNAPSHOT_PREVIEW_BYTES));
    preview
}

static SPOOL_SEQ: AtomicU64 = AtomicU64::new(0);

/// The stored snapshot as a byte stream. Snapshot files are gzip on the wire (peers serve them
/// stored) and are unwrapped on the way; a plain file passes through. The caller bounds the
/// read ([`MAX_DECOMPRESSED_SNAPSHOT_BYTES`]).
fn open_snapshot_reader<R: Read + 'static>(mut raw: R) -> std::io::Result<Box<dyn Read>> {
    let mut head = [0u8; 2];
    let mut got = 0;
    while got < head.len() {
        let n = raw.read(&mut head[got..])?;
        if n == 0 {
            break;
        }
        got += n;
    }
    let rejoined = std::io::Cursor::new(head[..got].to_vec()).chain(raw);
    Ok(if got == head.len() && head == [0x1f, 0x8b] {
        Box::new(flate2::read::MultiGzDecoder::new(rejoined))
    } else {
        Box::new(rejoined)
    })
}

/// Read the next line, including its newline. Return false at EOF, or fail as
/// soon as a line exceeds the cap.
fn read_line_capped<R: BufRead>(
    reader: &mut R,
    buf: &mut Vec<u8>,
    max_line: usize,
) -> std::io::Result<bool> {
    buf.clear();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Ok(!buf.is_empty());
        }
        let newline = available.iter().position(|&b| b == b'\n');
        let line_bytes = newline.unwrap_or(available.len());
        if buf.len() + line_bytes > max_line {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("snapshot line exceeds {max_line} bytes"),
            ));
        }
        let used = line_bytes + usize::from(newline.is_some());
        buf.extend_from_slice(&available[..used]);
        reader.consume(used);
        if newline.is_some() {
            return Ok(true);
        }
    }
}

/// The content hash of a body that arrives in pieces, for either CID scheme a snapshot is named
/// by: `Qm` is the SHA-256 of the bytes as a CIDv0, `ba` the chunked CIDv1 [`HashV1Writer`]
/// computes. What `verify_hash` does over a whole buffer, without the buffer.
enum CidHasher {
    V0(sha2::Sha256),
    V1(HashV1Writer),
}

impl CidHasher {
    fn for_cid(expected: &str) -> Option<Self> {
        if !is_canonical_cid(expected) {
            return None;
        }
        if expected.starts_with("Qm") {
            Some(Self::V0(sha2::Sha256::new()))
        } else if expected.starts_with("ba") {
            Some(Self::V1(HashV1Writer::new()))
        } else {
            None
        }
    }

    fn update(&mut self, data: &[u8]) {
        match self {
            Self::V0(sha) => sha.update(data),
            Self::V1(writer) => writer.update(data),
        }
    }

    fn finish(self) -> String {
        match self {
            Self::V0(sha) => {
                // `auth::cid::cid_v0_to_string` (private to `auth`): the SHA-256 multihash,
                // base58btc.
                let mut multihash = Vec::with_capacity(34);
                multihash.extend_from_slice(&[0x12, 0x20]);
                multihash.extend_from_slice(&sha.finalize());
                bs58::encode(&multihash).into_string()
            }
            Self::V1(writer) => writer.finish(),
        }
    }
}

/// Port of upstream 53e9c07's `SnapshotsFromServer`. Each snapshot stands for a whole time range, so
/// a discarded entry is a range nothing else covers, and treating the surviving subset as the
/// server's complete history would advance the frontier past those entities forever: callers that
/// record sync progress must check `discarded`, not just read `snapshots`.
#[derive(Debug, Clone)]
pub struct SnapshotsFromServer {
    pub snapshots: Vec<SnapshotMetadata>,
    pub discarded: usize,
}

/// Upstream `isValidSnapshotMetadata`. Snapshot metadata comes from untrusted servers, and
/// `time_range.end_timestamp` is what bootstrap installs as the server's last-snapshot timestamp and,
/// one poll boundary later, the durable GREATEST-monotonic frontier -- the same hazard the
/// pointer-changes timestamps are checked for.
///
/// `number_of_entities` and `generation_timestamp` are deliberately NOT validated (they default to 0
/// when missing or wrong-typed): nothing downstream reads them, and rejecting an entry over an unread
/// field would silently stop syncing from a server that reports, say, `numberOfEntities: "5"`.
pub(crate) fn parse_snapshot_metadata(
    value: &serde_json::Value,
    now_ms: Timestamp,
) -> Option<SnapshotMetadata> {
    let hash = value.get("hash")?.as_str()?;
    if !is_canonical_cid(hash) {
        return None;
    }

    let time_range = value.get("timeRange")?;
    let init_timestamp = time_range.get("initTimestamp")?.as_i64()?;
    let end_timestamp = time_range.get("endTimestamp")?.as_i64()?;
    if !is_usable_timestamp(init_timestamp, now_ms)
        || !is_usable_timestamp(end_timestamp, now_ms)
        || init_timestamp > end_timestamp
    {
        return None;
    }

    let replaced_snapshot_hashes = match value.get("replacedSnapshotHashes") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::Array(hashes)) => {
            if hashes.len() > MAX_REPLACED_SNAPSHOT_HASHES {
                return None;
            }
            let mut replaced = Vec::with_capacity(hashes.len());
            for h in hashes {
                let h = h.as_str()?;
                if !is_canonical_cid(h) {
                    return None;
                }
                replaced.push(h.to_string());
            }
            Some(replaced)
        }
        Some(_) => return None,
    };

    Some(SnapshotMetadata {
        hash: hash.to_string(),
        time_range: TimeRange {
            init_timestamp,
            end_timestamp,
        },
        number_of_entities: value
            .get("numberOfEntities")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        replaced_snapshot_hashes,
        generation_timestamp: value
            .get("generationTimestamp")
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
    })
}

pub(crate) fn parse_snapshots_response(
    items: &[serde_json::Value],
    now_ms: Timestamp,
    server: &str,
) -> SnapshotsFromServer {
    let mut snapshots = Vec::with_capacity(items.len());
    let mut discarded = 0usize;
    for item in items {
        match parse_snapshot_metadata(item, now_ms) {
            Some(snapshot) => snapshots.push(snapshot),
            None => {
                discarded += 1;
                if discarded <= MAX_INVALID_SNAPSHOT_LOGS {
                    let preview = entry_preview(item);
                    warn!(server, entry = %preview, "Ignoring invalid snapshot metadata entry");
                }
            }
        }
    }
    if discarded > MAX_INVALID_SNAPSHOT_LOGS {
        warn!(
            server,
            total = discarded,
            "Ignored additional invalid snapshot metadata entries"
        );
    }
    snapshots.sort_by_key(|s| std::cmp::Reverse(s.time_range.end_timestamp));
    SnapshotsFromServer {
        snapshots,
        discarded,
    }
}

pub async fn fetch_snapshots(
    client: &Client,
    server: &str,
    max_retries: u32,
) -> Result<SnapshotsFromServer, SyncError> {
    let url = format!("{}/snapshots", server);

    let mut last_error = None;
    for attempt in 0..max_retries {
        match client
            .get(&url)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
        {
            Ok(resp) => {
                if resp.status().is_success() {
                    let body = read_peer_body_capped(resp, MAX_SNAPSHOT_LIST_BYTES)
                        .await
                        .map_err(|e| {
                            SyncError::Other(format!("snapshot list from {server}: {e}"))
                        })?;
                    let items: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
                    let now_ms = chrono::Utc::now().timestamp_millis();
                    return Ok(parse_snapshots_response(&items, now_ms, server));
                } else {
                    let status = resp.status();
                    warn!(url = %url, %status, attempt, "Snapshot fetch failed");
                    last_error = Some(SyncError::Other(format!(
                        "HTTP {} fetching snapshots from {}",
                        status, server
                    )));
                }
            }
            Err(e) => {
                warn!(url = %url, error = %e, attempt, "Snapshot fetch request failed");
                last_error = Some(SyncError::Http(e));
            }
        }

        if attempt + 1 < max_retries {
            // Doubles to four times the wait: a dead peer costs the bootstrap
            // seconds, not minutes (peers are fetched one at a time).
            tokio::time::sleep(std::time::Duration::from_millis(
                500u64.saturating_mul(1u64 << attempt.min(2)),
            ))
            .await;
        }
    }

    Err(last_error.unwrap_or_else(|| {
        SyncError::Other(format!(
            "Failed to fetch snapshots from {} after {} retries",
            server, max_retries
        ))
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SnapshotDecision {
    /// Unless the node's own snapshot store already has it -- the one async check the caller still
    /// owns; its result never changes within a pass.
    Deploy,
    /// Some advertised replacement group is fully processed, so this snapshot's content is
    /// already covered: persist a processed mark for it and skip.
    MarkProcessed,
    /// Already processed, or entirely older than the genesis timestamp.
    Skip,
}

/// Pure core of upstream's `decideSnapshotDeploymentFromProcessedSet`, taking an already-fetched set
/// of processed hashes so a pass costs as few storage round trips as `filter_processed_in_chunks`
/// needs rather than one per snapshot.
///
/// `replaced_groups` carries one group per advertising server, and `.any()` over them matches
/// upstream's `.some()`: two peers advertising the same content hash with different replacement
/// histories must reach the same verdict regardless of which was fetched first.
///
/// On MarkProcessed the hash is added to `processed` -- mutating the caller's set is what makes the
/// snapshot that replaces THIS one skippable later in the same pass; see [`decide_snapshot_pass`].
pub(crate) fn decide_snapshot_deployment_from_processed_set(
    processed: &mut HashSet<String>,
    genesis_timestamp: Timestamp,
    snapshot_hash: &str,
    greatest_end_timestamp: Timestamp,
    replaced_groups: &[Vec<String>],
) -> SnapshotDecision {
    let snapshot_was_processed = processed.contains(snapshot_hash);
    let a_replaced_group_was_processed = replaced_groups
        .iter()
        .any(|group| !group.is_empty() && group.iter().all(|h| processed.contains(h)));

    if !snapshot_was_processed {
        if !a_replaced_group_was_processed {
            if greatest_end_timestamp > genesis_timestamp {
                return SnapshotDecision::Deploy;
            }
            return SnapshotDecision::Skip;
        }
        processed.insert(snapshot_hash.to_string());
        return SnapshotDecision::MarkProcessed;
    }
    SnapshotDecision::Skip
}

/// Fixed point of the deployment decision over one pass's candidates (upstream 53e9c07's pass loop in
/// `syncFromSnapshotsExclusively`). A mark is exactly what makes the snapshot replacing IT skippable
/// in turn, so a single pass over a replacement chain (h2 replaces h1 replaces a processed h0)
/// collapses only one link and deploys the tail for nothing. The loop terminates because "already
/// processed" and "older than genesis" drop out for good the first time, and every other pass either
/// marks at least one snapshot or ends it.
///
/// `candidates` is `(hash, greatest_end_timestamp_across_advertisers, replaced-group-per-advertiser)`.
/// The caller persists `newly_marked` and applies its own snapshot-store check to `to_deploy`.
/// Iteration order follows `candidates`, so callers pass a deterministic order.
pub(crate) fn decide_snapshot_pass(
    candidates: &[(String, Timestamp, Vec<Vec<String>>)],
    processed: &mut HashSet<String>,
    genesis_timestamp: Timestamp,
) -> (Vec<String>, Vec<String>) {
    let mut newly_marked: Vec<String> = Vec::new();
    let mut still_deployable: Vec<&(String, Timestamp, Vec<Vec<String>>)> =
        candidates.iter().collect();
    loop {
        let marked_before = processed.len();
        let mut next_round = Vec::with_capacity(still_deployable.len());
        for candidate in still_deployable {
            let (hash, greatest_end, groups) = candidate;
            match decide_snapshot_deployment_from_processed_set(
                processed,
                genesis_timestamp,
                hash,
                *greatest_end,
                groups,
            ) {
                SnapshotDecision::Deploy => next_round.push(candidate),
                SnapshotDecision::MarkProcessed => newly_marked.push(hash.clone()),
                SnapshotDecision::Skip => {}
            }
        }
        if processed.len() == marked_before {
            return (
                next_round.into_iter().map(|(h, _, _)| h.clone()).collect(),
                newly_marked,
            );
        }
        still_deployable = next_round;
    }
}

/// The same timestamp-plausibility gate /pointer-changes deltas get (upstream applies
/// `isUsableTimestamp` at both ingestion points plus /snapshots metadata).
fn usable_snapshot_line(deployment: &SyncDeployment, now_ms: Timestamp) -> bool {
    is_usable_timestamp(deployment.entity_timestamp, now_ms)
        && deployment
            .local_timestamp
            .is_none_or(|ts| is_usable_timestamp(ts, now_ms))
}

pub struct SnapshotSource<'a> {
    pub snapshot_hash: &'a str,
    pub advertisers: &'a [(&'a str, &'a CancellationToken)],
    pub max_retries: u32,
    pub retry_wait_ms: u64,
}

pub struct SnapshotFilter<'a> {
    pub genesis_timestamp: Timestamp,
    pub entity_type_filter: Option<&'a HashSet<String>>,
}

/// How many entities of one snapshot are handed to the deployer at once: the old
/// `SYNC_NONSCENE_CONCURRENCY` (64) and `SYNC_SCENE_CONCURRENCY` (1). Scenes are scheduled
/// after everything else and one at a time, because their content sets are the large ones.
#[derive(Debug, Clone, Copy)]
pub struct SnapshotScheduling {
    pub nonscene_concurrency: usize,
    pub scene_concurrency: usize,
}

impl Default for SnapshotScheduling {
    fn default() -> Self {
        Self {
            nonscene_concurrency: 64,
            scene_concurrency: 1,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct ParseStats {
    total: u64,
    parse_errors: u64,
    skipped_by_filter: u64,
}

/// The per-line decision, one snapshot's worth of state, run on the blocking thread that reads
/// the file: the JSON shape, the timestamp-plausibility gate, the genesis cutoff and the
/// entity-type allowlist. The first few skips of a snapshot are logged, the rest counted.
struct SnapshotLineFilter {
    snapshot_hash: String,
    genesis_timestamp: Timestamp,
    entity_type_filter: Option<HashSet<String>>,
    now_ms: Timestamp,
    stats: ParseStats,
}

impl SnapshotLineFilter {
    fn skip(&mut self, message: &str, error: Option<String>) {
        self.stats.parse_errors += 1;
        if self.stats.parse_errors <= MAX_SNAPSHOT_LINE_ERROR_LOGS {
            warn!(
                snapshot_hash = %self.snapshot_hash,
                error = error.as_deref().unwrap_or(""),
                "{message}"
            );
            if self.stats.parse_errors == MAX_SNAPSHOT_LINE_ERROR_LOGS {
                warn!(snapshot_hash = %self.snapshot_hash, "Suppressing further invalid snapshot lines");
            }
        }
    }

    fn parse(&mut self, line: &[u8]) -> Option<SyncDeployment> {
        let text = match std::str::from_utf8(line) {
            Ok(text) => text,
            Err(e) => {
                self.skip(
                    "Skipping snapshot line with invalid UTF-8",
                    Some(e.to_string()),
                );
                return None;
            }
        };
        let trimmed = text.trim();
        if !(trimmed.starts_with('{') && trimmed.ends_with('}')) {
            if !(trimmed.is_empty() || trimmed.starts_with("###")) {
                self.skip("Skipping unreadable snapshot line", None);
            }
            return None;
        }
        self.stats.total += 1;
        let deployment: SyncDeployment = match serde_json::from_str(trimmed) {
            Ok(d) => d,
            Err(e) => {
                self.skip("Skipping unparseable snapshot entry", Some(e.to_string()));
                return None;
            }
        };
        if let Err(reason) = deployment
            .validate_shape()
            .and_then(|()| deployment.validate_auth_chain())
        {
            self.skip("Skipping invalid snapshot entry", Some(reason.into()));
            return None;
        }
        if !usable_snapshot_line(&deployment, self.now_ms) {
            self.skip("Skipping snapshot line with implausible timestamp", None);
            return None;
        }
        if deployment.entity_timestamp < self.genesis_timestamp {
            return None;
        }
        if let Some(filter) = &self.entity_type_filter
            && !filter.contains(&deployment.entity_type)
        {
            self.stats.skipped_by_filter += 1;
            return None;
        }
        Some(deployment)
    }
}

/// Reads the stored snapshot line by line on the blocking pool and hands the deployable records
/// to `tx` in batches of [`SNAPSHOT_BATCH_RECORDS`]. Returns once the file is read, or as soon
/// as the receiver is gone (the consumer stopped). A file that inflates past
/// [`MAX_DECOMPRESSED_SNAPSHOT_BYTES`] or cannot be read is an error, not a shorter snapshot.
fn read_snapshot_records(
    path: PathBuf,
    mut filter: SnapshotLineFilter,
    tx: tokio::sync::mpsc::Sender<Vec<SyncDeployment>>,
) -> Result<ParseStats, String> {
    let file =
        std::fs::File::open(&path).map_err(|e| format!("opening {}: {e}", path.display()))?;
    let reader =
        open_snapshot_reader(file).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let mut reader = std::io::BufReader::with_capacity(
        256 * 1024,
        reader.take(MAX_DECOMPRESSED_SNAPSHOT_BYTES + 1),
    );
    let mut line = Vec::with_capacity(16 * 1024);
    let mut batch = Vec::with_capacity(SNAPSHOT_BATCH_RECORDS);
    loop {
        if tx.is_closed() {
            return Ok(filter.stats);
        }
        if !read_line_capped(&mut reader, &mut line, MAX_SNAPSHOT_LINE_BYTES)
            .map_err(|e| format!("reading {}: {e}", path.display()))?
        {
            break;
        }
        if let Some(record) = filter.parse(&line) {
            batch.push(record);
            if batch.len() >= SNAPSHOT_BATCH_RECORDS {
                let full =
                    std::mem::replace(&mut batch, Vec::with_capacity(SNAPSHOT_BATCH_RECORDS));
                if tx.blocking_send(full).is_err() {
                    return Ok(filter.stats);
                }
            }
        }
    }
    if reader.get_ref().limit() == 0 {
        return Err(format!(
            "decompressed to more than {MAX_DECOMPRESSED_SNAPSHOT_BYTES} bytes"
        ));
    }
    if !batch.is_empty() {
        let _ = tx.blocking_send(batch);
    }
    Ok(filter.stats)
}

#[derive(Debug, Default, Clone, Copy)]
struct ScheduleOutcome {
    scheduled: u64,
    errors: u64,
    stopped: bool,
}

impl ScheduleOutcome {
    fn absorb(&mut self, other: Self) {
        self.scheduled += other.scheduled;
        self.errors += other.errors;
        self.stopped |= other.stopped;
    }
}

async fn schedule_records(
    deployer: &BatchDeployer,
    records: Vec<SyncDeployment>,
    concurrency: usize,
    servers: &[String],
    report: &Arc<DeploymentReport>,
    snapshot_hash: &str,
    should_stop: &impl Fn() -> bool,
) -> ScheduleOutcome {
    let mut records = records;
    let offered = records.len() as u64;
    let probed = match deployer.retain_undeployed(&mut records).await {
        Ok(()) => true,
        Err(e) => {
            warn!(snapshot_hash, error = %e, "Batched already-deployed probe failed; probing per record");
            false
        }
    };
    let scheduled = AtomicU64::new(offered - records.len() as u64);
    let errors = AtomicU64::new(0);
    let stopped = AtomicBool::new(false);
    let (scheduled_ref, errors_ref, stopped_ref) = (&scheduled, &errors, &stopped);
    futures::stream::iter(records)
        .for_each_concurrent(concurrency, |deployment| async move {
            if stopped_ref.load(Ordering::Relaxed) {
                return;
            }
            if should_stop() {
                stopped_ref.store(true, Ordering::Relaxed);
                return;
            }
            let outcome = if probed {
                deployer
                    .schedule_undeployed(deployment, servers, Some(report))
                    .await
            } else {
                deployer
                    .schedule_entity_deployment(deployment, servers, Some(report))
                    .await
            };
            match outcome {
                Ok(()) => {
                    scheduled_ref.fetch_add(1, Ordering::Relaxed);
                }
                Err(SyncError::Stopped) => stopped_ref.store(true, Ordering::Relaxed),
                Err(e) => {
                    errors_ref.fetch_add(1, Ordering::Relaxed);
                    warn!(snapshot_hash, error = %e, "Failed to schedule entity deployment");
                }
            }
        })
        .await;
    ScheduleOutcome {
        scheduled: scheduled.into_inner(),
        errors: errors.into_inner(),
        stopped: stopped.into_inner(),
    }
}

/// Err when the snapshot could not be fully handed off -- download failure, unreadable lines, or
/// scheduling errors -- so the caller keeps the advertising servers in snapshot bootstrap with their
/// timestamps held back instead of advancing past entities that were never deployed. Readable
/// entities are still scheduled first.
///
/// The snapshot is read from disk as a stream ([`read_snapshot_records`]) and scheduled a batch
/// at a time: at most one batch is parsed ahead of the one being scheduled, whatever the
/// snapshot's size. Within a batch, scenes go after the other entities and one at a time
/// (`scene_concurrency`), as the whole-snapshot pass this replaces ordered them; the order
/// across batches is the file's.
///
/// `report` accumulates scheduled-vs-acknowledged counts; the caller re-checks it after the deployer
/// drains, since an Ok return only proves everything was SCHEDULED.
#[allow(
    clippy::too_many_arguments,
    reason = "source, filter and scheduling are already grouped; the rest are the caller's shared handles"
)]
pub async fn deploy_entities_from_snapshot(
    client: &crate::http::CheckedRedirectClient,
    storage: &Storage,
    deployer: &BatchDeployer,
    source: SnapshotSource<'_>,
    filter: SnapshotFilter<'_>,
    scheduling: SnapshotScheduling,
    report: &Arc<DeploymentReport>,
) -> Result<(), SyncError> {
    let SnapshotSource {
        snapshot_hash,
        advertisers,
        max_retries,
        retry_wait_ms,
    } = source;
    let SnapshotFilter {
        genesis_timestamp,
        entity_type_filter,
    } = filter;
    let server_list: Vec<String> = advertisers
        .iter()
        .map(|(url, _)| (*url).to_string())
        .collect();
    let should_stop = || advertisers.iter().all(|(_, cancel)| cancel.is_cancelled());
    let cancelled = async {
        for (_, cancel) in advertisers {
            cancel.cancelled().await;
        }
    };

    tokio::pin!(cancelled);
    let file = tokio::select! {
        biased;
        _ = &mut cancelled => return Err(SyncError::Stopped),
        result = download_snapshot_file(
            client,
            storage,
            snapshot_hash,
            advertisers,
            max_retries,
            retry_wait_ms,
        ) => result?,
    };
    if storage.withholds(snapshot_hash) {
        return Err(SyncError::EntityNotFound {
            entity_id: snapshot_hash.to_string(),
        });
    }

    let nonscene_concurrency = scheduling.nonscene_concurrency.max(1);
    let scene_concurrency = scheduling.scene_concurrency.max(1);

    let line_filter = SnapshotLineFilter {
        snapshot_hash: snapshot_hash.to_string(),
        genesis_timestamp,
        entity_type_filter: entity_type_filter.cloned(),
        now_ms: chrono::Utc::now().timestamp_millis(),
        stats: ParseStats::default(),
    };
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<SyncDeployment>>(1);
    let reader = tokio::task::spawn_blocking(move || {
        let result = read_snapshot_records(file.path().to_path_buf(), line_filter, tx);
        drop(file);
        result
    });

    let mut outcome = ScheduleOutcome::default();
    loop {
        let next_batch = tokio::select! {
            biased;
            _ = &mut cancelled => {
                outcome.stopped = true;
                break;
            }
            batch = rx.recv() => batch,
        };
        let Some(batch) = next_batch else {
            break;
        };
        if should_stop() {
            outcome.stopped = true;
            break;
        }
        let (scenes, others): (Vec<_>, Vec<_>) = batch
            .into_iter()
            .partition(|deployment| deployment.entity_type == "scene");
        for (records, concurrency) in [(others, nonscene_concurrency), (scenes, scene_concurrency)]
        {
            let scheduled = tokio::select! {
                biased;
                _ = &mut cancelled => {
                    outcome.stopped = true;
                    break;
                }
                result = schedule_records(
                    deployer,
                    records,
                    concurrency,
                    &server_list,
                    report,
                    snapshot_hash,
                    &should_stop,
                ) => result,
            };
            outcome.absorb(scheduled);
            if outcome.stopped {
                break;
            }
        }
        if outcome.stopped {
            break;
        }
    }
    // A reader still going returns at its next hand-off once the receiver is gone.
    drop(rx);
    let stats = match reader.await {
        Ok(Ok(stats)) => stats,
        Ok(Err(reason)) => {
            if outcome.stopped {
                return Err(SyncError::Stopped);
            }
            return Err(SyncError::Other(format!(
                "snapshot {snapshot_hash} could not be read: {reason}"
            )));
        }
        Err(join) => {
            return Err(SyncError::Other(format!(
                "snapshot {snapshot_hash} reader failed: {join}"
            )));
        }
    };

    if outcome.stopped {
        return Err(SyncError::Stopped);
    }

    let ParseStats {
        total,
        parse_errors: num_parse_errors,
        skipped_by_filter: num_skipped_by_filter,
    } = stats;
    let num_scheduled = outcome.scheduled;
    let num_schedule_errors = outcome.errors;

    info!(
        snapshot_hash,
        total,
        num_scheduled,
        num_skipped_by_filter,
        num_parse_errors,
        num_schedule_errors,
        nonscene_concurrency,
        scene_concurrency,
        "Snapshot scheduled"
    );

    if num_parse_errors > 0 || num_schedule_errors > 0 {
        return Err(SyncError::Other(format!(
            "snapshot {} was not fully deployable: {} unreadable lines, {} schedule errors \
             (scheduled {})",
            snapshot_hash, num_parse_errors, num_schedule_errors, num_scheduled
        )));
    }

    Ok(())
}

/// The snapshot's staging file and hasher while its body streams in.
struct SnapshotSpool {
    file: tokio::fs::File,
    hasher: CidHasher,
}

impl DecodedSink for SnapshotSpool {
    async fn accept(&mut self, decoded: &[u8]) -> Result<(), String> {
        self.hasher.update(decoded);
        self.file
            .write_all(decoded)
            .await
            .map_err(|e| format!("spooling snapshot to disk: {e}"))
    }
}

/// A snapshot body streamed into a staging file and hashed as it is decoded.
/// The reader owns the guard, which unlinks the file when reading ends. The
/// store's startup sweep reclaims abandoned staging files after a crash.
struct SpooledSnapshot {
    guard: StagingGuard,
    stats: BodyStats,
    digest: String,
}

async fn spool_snapshot(
    storage: &Storage,
    snapshot_hash: &str,
    resp: reqwest::Response,
) -> Result<SpooledSnapshot, String> {
    let hasher = CidHasher::for_cid(snapshot_hash)
        .ok_or_else(|| format!("{snapshot_hash} is not a canonical CID"))?;
    let final_path = resolve_file_path(storage.root(), snapshot_hash).map_err(|e| e.to_string())?;
    if let Some(dir) = final_path.parent() {
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|e| format!("creating {}: {e}", dir.display()))?;
    }
    let seq = SPOOL_SEQ.fetch_add(1, Ordering::Relaxed);
    let (file, guard) = create_staging_file(staging_path(
        &final_path.with_file_name("peer-snapshot"),
        "peer-snapshot",
        seq,
    ))
    .await
    .map_err(|e| e.to_string())?;
    let mut sink = SnapshotSpool { file, hasher };
    let stats = stream_body(resp, MAX_BODY_BYTES, &mut sink).await?;
    let SnapshotSpool { mut file, hasher } = sink;
    file.flush()
        .await
        .map_err(|e| format!("flushing snapshot to disk: {e}"))?;
    drop(file);
    Ok(SpooledSnapshot {
        guard,
        stats,
        digest: hasher.finish(),
    })
}

async fn download_snapshot_file(
    client: &crate::http::CheckedRedirectClient,
    storage: &Storage,
    snapshot_hash: &str,
    advertisers: &[(&str, &CancellationToken)],
    max_retries: u32,
    retry_wait_ms: u64,
) -> Result<StagingGuard, SyncError> {
    if storage.withholds(snapshot_hash) {
        return Err(SyncError::EntityNotFound {
            entity_id: snapshot_hash.to_string(),
        });
    }

    if advertisers.is_empty() {
        return Err(SyncError::NoServers);
    }

    let mut next = 0;
    let mut retry = 0;
    let mut last_error = None;
    while retry < max_retries {
        let Some(index) = (0..advertisers.len())
            .map(|offset| (next + offset) % advertisers.len())
            .find(|&index| !advertisers[index].1.is_cancelled())
        else {
            return Err(SyncError::Stopped);
        };
        let (server, cancel) = advertisers[index];
        next = (index + 1) % advertisers.len();
        let result = tokio::select! {
            biased;
            _ = cancel.cancelled() => continue,
            result = download_snapshot_attempt(client, storage, snapshot_hash, server) => result,
        };
        match result {
            Ok(file) => return Ok(file),
            Err(error) => {
                warn!(snapshot_hash, server, retry, %error, "Snapshot download failed");
                last_error = Some(error);
            }
        }

        // Retirement is not a failed attempt and bypasses any delay before the next peer.
        if retry + 1 < max_retries {
            let delay = std::time::Duration::from_millis(
                retry_wait_ms.saturating_mul(1u64 << retry.min(2)),
            );
            tokio::select! {
                _ = cancel.cancelled() => {},
                _ = tokio::time::sleep(delay) => {},
            }
        }
        retry += 1;
    }

    Err(last_error.unwrap_or_else(|| {
        SyncError::Other(format!(
            "Failed to download snapshot {} after {} retries",
            snapshot_hash, max_retries
        ))
    }))
}

async fn download_snapshot_attempt(
    client: &crate::http::CheckedRedirectClient,
    storage: &Storage,
    snapshot_hash: &str,
    server: &str,
) -> Result<StagingGuard, SyncError> {
    let response = crate::http::get_following_checked_redirects(
        client,
        &format!("{server}/contents/{snapshot_hash}"),
        crate::deploy_remote_entity::MAX_FILE_REDIRECTS,
    )
    .await?;
    if !response.status().is_success() {
        return Err(SyncError::Other(format!(
            "HTTP {} downloading snapshot {snapshot_hash} from {server}",
            response.status()
        )));
    }
    let expected_len = response.content_length();
    let spooled = spool_snapshot(storage, snapshot_hash, response)
        .await
        .map_err(|reason| {
            SyncError::Other(format!("snapshot {snapshot_hash} from {server}: {reason}"))
        })?;
    if let Some(len) = expected_len
        && spooled.stats.wire_len < len
    {
        return Err(SyncError::Other(format!(
            "snapshot {snapshot_hash} from {server} truncated: got {} of {len} bytes",
            spooled.stats.wire_len
        )));
    }
    if spooled.digest != snapshot_hash {
        return Err(SyncError::Other(format!(
            "snapshot hash mismatch for {snapshot_hash} from {server}"
        )));
    }
    info!(
        snapshot_hash,
        bytes = spooled.stats.decoded_len,
        "Snapshot downloaded"
    );
    Ok(spooled.guard)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{hash_bytes, hash_bytes_v1};
    use crate::deploy_remote_entity::gzip;
    use crate::sync_test_support::{spawn_content_server, temp_storage, unique};

    const NOW_MS: Timestamp = 1_750_000_000_000;
    const GOOD_HASH: &str = "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG";
    const GOOD_HASH_2: &str = "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdH";
    const GOOD_HASH_3: &str = "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdJ";

    fn staged_files(storage: &Storage, hash: &str) -> Vec<PathBuf> {
        let blob = resolve_file_path(storage.root(), hash).unwrap();
        let dir = blob.parent().unwrap();
        match std::fs::read_dir(dir) {
            Ok(entries) => entries
                .map(|entry| entry.unwrap().path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "tmp"))
                .collect(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => panic!("could not read snapshot staging directory: {error}"),
        }
    }

    #[test]
    fn a_long_invalid_entry_preview_is_cut_on_a_character_boundary() {
        // `"` takes byte 0, so every `é` starts on an odd byte and byte 512 falls inside one.
        let item = serde_json::Value::String("é".repeat(400));
        let preview = entry_preview(&item);
        assert_eq!(preview, format!("\"{}", "é".repeat(255)));
        assert_eq!(preview.len(), 511);
    }

    fn entry(hash: &str, init: i64, end: i64) -> serde_json::Value {
        serde_json::json!({
            "hash": hash,
            "timeRange": { "initTimestamp": init, "endTimestamp": end },
            "numberOfEntities": 10,
            "generationTimestamp": end,
        })
    }

    #[test]
    fn far_future_and_inverted_entries_are_discarded_and_counted() {
        let items = vec![
            entry(GOOD_HASH, 0, 1_700_000_000_000),
            entry(GOOD_HASH_2, 0, 253_402_300_799_000),
            entry(GOOD_HASH_3, 1_700_000_000_000, 1_600_000_000_000),
        ];
        let result = parse_snapshots_response(&items, NOW_MS, "https://peer.test");
        assert_eq!(result.discarded, 2);
        assert_eq!(result.snapshots.len(), 1);
        assert_eq!(result.snapshots[0].hash, GOOD_HASH);
    }

    #[test]
    fn valid_entry_parses_with_replaced_hashes_and_sorts_newest_first() {
        let mut older = entry(GOOD_HASH, 0, 1_600_000_000_000);
        older["replacedSnapshotHashes"] = serde_json::json!([GOOD_HASH_2]);
        let newer = entry(GOOD_HASH_3, 0, 1_700_000_000_000);
        let result = parse_snapshots_response(&[older, newer], NOW_MS, "s");
        assert_eq!(result.discarded, 0);
        assert_eq!(result.snapshots[0].hash, GOOD_HASH_3, "newest first");
        assert_eq!(
            result.snapshots[1].replaced_snapshot_hashes.as_deref(),
            Some(&[GOOD_HASH_2.to_string()][..])
        );
    }

    #[test]
    fn unread_fields_never_reject_an_entry() {
        let mut e = entry(GOOD_HASH, 0, 1_700_000_000_000);
        e["numberOfEntities"] = serde_json::json!("5");
        e["generationTimestamp"] = serde_json::json!("not a number");
        let parsed = parse_snapshot_metadata(&e, NOW_MS).expect("entry must survive");
        assert_eq!(parsed.number_of_entities, 0);
        assert_eq!(parsed.generation_timestamp, 0);
    }

    #[test]
    fn malformed_hash_or_replaced_hashes_reject_the_entry() {
        let traversal = entry("../../etc/passwd", 0, 1_700_000_000_000);
        assert!(parse_snapshot_metadata(&traversal, NOW_MS).is_none());

        let mut bad_replaced = entry(GOOD_HASH, 0, 1_700_000_000_000);
        bad_replaced["replacedSnapshotHashes"] = serde_json::json!(["not-a-cid"]);
        assert!(parse_snapshot_metadata(&bad_replaced, NOW_MS).is_none());

        let mut wrong_type = entry(GOOD_HASH, 0, 1_700_000_000_000);
        wrong_type["replacedSnapshotHashes"] = serde_json::json!("QmSingle");
        assert!(parse_snapshot_metadata(&wrong_type, NOW_MS).is_none());

        let mut fractional = entry(GOOD_HASH, 0, 1_700_000_000_000);
        fractional["timeRange"]["endTimestamp"] = serde_json::json!(1.7e12 + 0.5);
        assert!(
            parse_snapshot_metadata(&fractional, NOW_MS).is_none(),
            "epoch milliseconds are integers; a fraction only ever indicates a malformed server"
        );
    }

    #[test]
    fn snapshot_line_timestamps_get_the_plausibility_gate() {
        let line = |entity_ts: &str, local_ts: Option<&str>| {
            let local = local_ts.map_or(String::new(), |ts| format!(",\"localTimestamp\":{ts}"));
            format!(
                "{{\"entityId\":\"QmLine\",\"entityType\":\"scene\",\"pointers\":[\"0,0\"],\"authChain\":[],\"entityTimestamp\":{entity_ts}{local}}}"
            )
        };
        let parse = |s: &str| serde_json::from_str::<SyncDeployment>(s).unwrap();
        let now_ms = 1_700_000_000_000;

        assert!(usable_snapshot_line(
            &parse(&line("1600000000000", None)),
            now_ms
        ));
        assert!(
            !usable_snapshot_line(&parse(&line("253402300800000", None)), now_ms),
            "year-9999 entityTimestamp would win overwrite ordering forever"
        );
        assert!(
            !usable_snapshot_line(
                &parse(&line("1600000000000", Some("253402300800000"))),
                now_ms
            ),
            "an implausible localTimestamp is rejected even with a sane entityTimestamp"
        );
    }

    #[test]
    fn replacement_chain_collapses_in_a_single_call() {
        let h0 = "h0".to_string();
        let h1 = "h1".to_string();
        let h2 = "h2".to_string();
        let genesis = 0;
        let end_ts = 1_700_000_000_000;
        let candidates = vec![
            (h2.clone(), end_ts, vec![vec![h1.clone()]]),
            (h1.clone(), end_ts, vec![vec![h0.clone()]]),
        ];
        let mut processed: HashSet<String> = [h0].into_iter().collect();
        let (to_deploy, newly_marked) = decide_snapshot_pass(&candidates, &mut processed, genesis);
        assert!(
            to_deploy.is_empty(),
            "no link of a fully-replaced chain may be deployed, got {to_deploy:?}"
        );
        let mut sorted = newly_marked;
        sorted.sort();
        assert_eq!(sorted, vec![h1, h2], "both chain links must end up marked");
        assert!(processed.contains("h1") && processed.contains("h2"));
    }

    #[test]
    fn unreplaced_snapshot_deploys_and_genesis_cutoff_skips() {
        let candidates = vec![
            ("new".to_string(), 1_700_000_000_000, vec![]),
            ("ancient".to_string(), 5, vec![]),
        ];
        let mut processed = HashSet::new();
        let (to_deploy, marked) =
            decide_snapshot_pass(&candidates, &mut processed, 1_000_000_000_000);
        assert_eq!(to_deploy, vec!["new".to_string()]);
        assert!(marked.is_empty());
    }

    #[tokio::test]
    async fn gzip_labeled_snapshot_decodes_and_leaves_only_a_temporary_file() {
        const PAYLOAD: &[u8] = b"### Decentraland json snapshot\n{\"entityId\":\"Qm1\"}\n";
        let cid = hash_bytes_v1(PAYLOAD);
        let wire = gzip(PAYLOAD);
        let (base, server) = spawn_content_server(wire, Some("gzip")).await;
        let (storage, tmp) = temp_storage("snap-gzip").await;
        let cancel = CancellationToken::new();

        let spool = download_snapshot_file(
            &crate::sync::test_sync_client(Client::new()),
            &storage,
            &cid,
            &[(base.as_str(), &cancel)],
            1,
            0,
        )
        .await
        .expect("a gzip-labeled snapshot whose decoded bytes match the CID must be accepted");

        let path = spool.path().to_owned();
        let stored = tokio::fs::read(&path).await.unwrap();
        assert_eq!(
            &stored[..],
            PAYLOAD,
            "the temporary bytes must be the decoded representation"
        );
        assert!(!storage.exist(&cid).await.unwrap());
        drop(spool);
        assert!(!path.exists());

        server.abort();
        let _ = tokio::fs::remove_dir_all(&tmp).await;
    }

    #[tokio::test]
    async fn corrupt_snapshot_body_still_fails_hash_verification() {
        const PAYLOAD: &[u8] = b"### Decentraland json snapshot\n{\"entityId\":\"Qm1\"}\n";
        const OTHER: &[u8] = b"### Decentraland json snapshot\n{\"entityId\":\"Qm2\"}\n";
        let cid = hash_bytes_v1(PAYLOAD);
        let wire = gzip(OTHER);
        let (base, server) = spawn_content_server(wire, Some("gzip")).await;
        let (storage, tmp) = temp_storage("snap-corrupt").await;
        let cancel = CancellationToken::new();

        let err = download_snapshot_file(
            &crate::sync::test_sync_client(Client::new()),
            &storage,
            &cid,
            &[(base.as_str(), &cancel)],
            1,
            0,
        )
        .await
        .err()
        .expect("decoded bytes that mismatch the CID must still be rejected");
        assert!(err.to_string().contains("snapshot hash mismatch"), "{err}");
        assert!(!storage.exist(&cid).await.unwrap());
        assert!(staged_files(&storage, &cid).is_empty());

        server.abort();
        let _ = tokio::fs::remove_dir_all(&tmp).await;
    }

    #[tokio::test]
    async fn a_retired_snapshot_advertiser_does_not_spend_the_remaining_attempt() {
        const PAYLOAD: &[u8] = b"### Decentraland json snapshot\n";
        let cid = hash_bytes_v1(PAYLOAD);
        let body_stalled = Arc::new(tokio::sync::Notify::new());
        let notify_stalled = body_stalled.clone();
        let app = axum::Router::new().route(
            "/contents/{hash}",
            axum::routing::get(move || {
                let notify_stalled = notify_stalled.clone();
                async move {
                    let first = futures::stream::iter([Ok::<_, std::convert::Infallible>(
                        bytes::Bytes::from_static(b"partial snapshot"),
                    )]);
                    let remaining = futures::stream::once(async move {
                        notify_stalled.notify_one();
                        std::future::pending().await
                    });
                    axum::body::Body::from_stream(first.chain(remaining))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let retired_url = format!("http://{}", listener.local_addr().unwrap());
        let retired_server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let (current_url, current_server) = spawn_content_server(PAYLOAD.to_vec(), None).await;
        let (storage, tmp) = temp_storage("snap-retired").await;
        let retired = CancellationToken::new();
        let current = CancellationToken::new();
        let advertisers = [
            (retired_url.as_str(), &retired),
            (current_url.as_str(), &current),
        ];
        let client = crate::sync::test_sync_client(Client::new());
        let retire_peer = async {
            body_stalled.notified().await;
            while staged_files(&storage, &cid).is_empty() {
                tokio::task::yield_now().await;
            }
            retired.cancel();
        };

        let (result, ()) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            tokio::join!(
                download_snapshot_file(&client, &storage, &cid, &advertisers, 1, 0),
                retire_peer
            )
        })
        .await
        .expect("retirement must release the stalled response");
        let spool = result.expect("the current peer keeps the one download attempt");
        assert_eq!(tokio::fs::read(spool.path()).await.unwrap(), PAYLOAD);
        assert_eq!(staged_files(&storage, &cid), [spool.path().to_path_buf()]);
        drop(spool);
        assert!(staged_files(&storage, &cid).is_empty());

        for server in [retired_server, current_server] {
            server.abort();
        }
        let _ = tokio::fs::remove_dir_all(&tmp).await;
    }

    fn read_all(raw: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        open_snapshot_reader(std::io::Cursor::new(raw.to_vec()))
            .unwrap()
            .read_to_end(&mut out)
            .unwrap();
        out
    }

    #[test]
    fn a_stored_gzip_snapshot_decompresses_and_a_plain_one_passes_through() {
        const PAYLOAD: &[u8] = b"### Decentraland json snapshot\n{\"entityId\":\"Qm1\"}\n";
        assert_eq!(read_all(&gzip(PAYLOAD)), PAYLOAD);
        assert_eq!(read_all(PAYLOAD), PAYLOAD);
        assert_eq!(
            read_all(b"{"),
            b"{",
            "a file shorter than the gzip magic passes through"
        );
        assert!(read_all(b"").is_empty());
    }

    #[test]
    fn a_streamed_cid_matches_the_whole_buffer_hash_under_both_schemes() {
        for size in [0usize, 10, 262_144, 262_145, 700_000] {
            let data: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();
            let v1 = hash_bytes_v1(&data);
            let v0 = hash_bytes(&data);
            for piece in [1usize << 10, 65_536, 1 << 20] {
                let mut h1 = CidHasher::for_cid(&v1).unwrap();
                let mut h0 = CidHasher::for_cid(&v0).unwrap();
                for part in data.chunks(piece) {
                    h1.update(part);
                    h0.update(part);
                }
                assert_eq!(h1.finish(), v1, "v1 size {size} piece {piece}");
                assert_eq!(h0.finish(), v0, "v0 size {size} piece {piece}");
            }
        }
        assert!(CidHasher::for_cid("../../etc/passwd").is_none());
    }

    #[test]
    fn the_capped_line_reader_stops_at_an_overlong_line() {
        let text = b"short\nxxxxxxxxxxxxxxxxxxxxxxxx\n";
        let mut reader = std::io::BufReader::with_capacity(4, &text[..]);
        let mut buf = Vec::new();
        assert!(read_line_capped(&mut reader, &mut buf, 5).unwrap());
        assert_eq!(buf, b"short\n");
        assert!(read_line_capped(&mut reader, &mut buf, 5).is_err());
    }

    #[test]
    fn the_capped_line_reader_accepts_an_unterminated_last_line() {
        let mut reader = std::io::BufReader::with_capacity(4, &b"\nlast"[..]);
        let mut buf = Vec::new();
        assert!(read_line_capped(&mut reader, &mut buf, 4).unwrap());
        assert_eq!(buf, b"\n");
        assert!(read_line_capped(&mut reader, &mut buf, 4).unwrap());
        assert_eq!(buf, b"last");
        assert!(!read_line_capped(&mut reader, &mut buf, 4).unwrap());
    }

    #[test]
    fn snapshot_records_arrive_in_bounded_batches_and_malformed_lines_are_counted() {
        let record = |i: usize, kind: &str, timestamp: i64| {
            serde_json::json!({
                "entityId": hash_bytes_v1(&i.to_be_bytes()),
                "entityType": kind,
                "pointers": [format!("{i},0")],
                "authChain": [{"type": "SIGNER", "payload": "0x0000000000000000000000000000000000000001"}],
                "entityTimestamp": timestamp,
            })
        };
        let records = (0..2_503).map(|i| {
            record(
                i,
                if i % 5 == 0 { "scene" } else { "profile" },
                1_600_000_000_000,
            )
        });
        let mut text = String::from("### Decentraland json snapshot\n\n");
        for row in records {
            text.push_str(&format!("{row}\n"));
        }
        text.push_str("this is not json\n");
        text.push_str("{\"entityId\":1}\n");
        for row in [
            record(9_001, "scene", 253_402_300_800_000),
            record(9_002, "scene", 5),
            record(9_003, "wearable", 1_600_000_000_000),
        ] {
            text.push_str(&format!("{row}\n"));
        }
        for (field, value) in [
            ("entityId", serde_json::json!("not-a-cid")),
            ("pointers", serde_json::json!([])),
            ("authChain", serde_json::json!([])),
        ] {
            let mut row = record(9_004, "scene", 1_600_000_000_000);
            row[field] = value;
            text.push_str(&format!("{row}\n"));
        }
        let dir = std::env::temp_dir().join(format!("catalyst-snapshot-lines-{}", unique()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("snapshot.gz");
        std::fs::write(&path, gzip(text.as_bytes())).unwrap();

        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        let stats = read_snapshot_records(
            path,
            SnapshotLineFilter {
                snapshot_hash: "test".into(),
                genesis_timestamp: 1_000,
                entity_type_filter: Some(
                    ["scene".to_string(), "profile".to_string()]
                        .into_iter()
                        .collect(),
                ),
                now_ms: 1_700_000_000_000,
                stats: ParseStats::default(),
            },
            tx,
        )
        .unwrap();

        let mut sizes = Vec::new();
        let mut scenes = 0;
        while let Ok(batch) = rx.try_recv() {
            sizes.push(batch.len());
            scenes += batch.iter().filter(|d| d.entity_type == "scene").count();
        }
        assert_eq!(sizes, vec![1000, 1000, 503]);
        assert_eq!(scenes, 501);
        assert_eq!(
            stats,
            ParseStats {
                total: 2_503 + 7,
                parse_errors: 6,
                skipped_by_filter: 1
            }
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn any_advertisers_processed_group_suffices() {
        let mut processed: HashSet<String> =
            ["a".to_string(), "b".to_string()].into_iter().collect();
        let decision = decide_snapshot_deployment_from_processed_set(
            &mut processed,
            0,
            "snap",
            1_700_000_000_000,
            &[
                vec!["a".to_string(), "x".to_string()],
                vec!["a".to_string(), "b".to_string()],
            ],
        );
        assert_eq!(decision, SnapshotDecision::MarkProcessed);
        assert!(
            processed.contains("snap"),
            "mark must mutate the caller's set"
        );

        let mut processed = HashSet::new();
        let decision = decide_snapshot_deployment_from_processed_set(
            &mut processed,
            0,
            "snap2",
            1_700_000_000_000,
            &[vec![]],
        );
        assert_eq!(decision, SnapshotDecision::Deploy);
    }
}
