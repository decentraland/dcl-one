//! The snapshot generator: what makes this node a `SYNC_SOURCE`. Every
//! `SNAPSHOT_GENERATION_INTERVAL_HOURS` (default 6) the active deployments
//! are written, one file per generation window
//! ([`time_range::divide_time_in_years_months_weeks_and_days`]), into the ONE
//! blob store, and each window becomes a `content.snapshots` row that
//! `GET /snapshots` lists and `GET /contents/{hash}` serves.
//!
//! File format (the old generator's, byte for byte): the line
//! `### Decentraland json snapshot`, then one JSON object per active
//! deployment whose `entity_timestamp` lies in the window, inclusive at both
//! ends (an entity on a shared boundary is in both windows), in
//! `(entity_timestamp, entity_id)` order:
//! `{"entityId","entityType","pointers","authChain","entityTimestamp"}`,
//! `authChain` as the stored document re-serialized by `serde_json` (object
//! keys sorted, as neither tree enables `preserve_order`). The hash is the
//! file's CIDv1 (`hash_bytes_v1`). `tests/fixtures/snapshots/` pins both
//! against the old generator's output.
//!
//! One pass, per window: an existing row for exactly that window is reused
//! when no active deployment in the window landed after its generation time
//! and the stored bytes still hash to its name (re-read once per process;
//! a mismatch re-keys the file and the row, the 2026-05 bulk-import repair);
//! otherwise the window is generated and, in ONE transaction, every row it
//! contains is deleted and the new row inserted with their hashes as
//! `replacedSnapshotHashes`. Rows that match no window are pruned. The
//! derived rows come from `content.deployments` (source), are fresh to
//! within one interval (freshness), and are rebuilt whole, no window reused,
//! by `POST /admin/api/content/snapshots/regenerate` (rebuild): the path
//! for what the outdated probe cannot see, an entity overwritten after its
//! window was written, which the window keeps listing until then (as the
//! old generator did).
//!
//! Deviations from the old generator, each a fix:
//! * The replaced rows and the new one change in one transaction, after the
//!   new file is stored: the old deleted the stale rows first and generated
//!   after, so a peer reading `/snapshots` in between (minutes, for a year
//!   window) saw a gap, bootstrapped past it and never fetched those
//!   entities.
//! * The generation time is the start of the oldest transaction open on the
//!   database when the window is read, not the wall clock: a flush that began
//!   before the read and committed after it stamps `local_timestamp` earlier
//!   than the read, so against the wall clock its entity was invisible to the
//!   file AND to the outdated probe, and the window never regenerated.
//! * Rows are streamed into the file instead of loaded as a vector first,
//!   and ties on `entity_timestamp` are broken by `entity_id` (the old order
//!   was the plan's), so equal data always yields the same CID.
//! * Blobs are never unlinked: the store is shared with every other domain,
//!   a replaced snapshot's bytes may be any other consumer's content, and
//!   deletion is the store-wide collector's decision.
//! * Generation waits for [`SyncState::Syncing`], not the old string match
//!   that also passed `PartiallySynced`: a file written while profiles are
//!   still bootstrapping would be marked processed by a peer and replaced
//!   later, and the replacement is skipped as already covered.
//!
//! A node without a sync worker holds only what its database holds and has
//! nothing to wait for; it generates as the old node did.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use bytes::Bytes;
use chrono::NaiveDateTime;
use futures::TryStreamExt;
use parking_lot::Mutex;
use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use super::time_range::{self, SNAPSHOTS_INIT_TIMESTAMP_MS};
use super::wire_types::{SnapshotMetadata, TimeRange, Timestamp};
use super::{ContentState, SyncState};
use crate::jobs::{JobContext, Jobs, PeriodicCfg};

pub const SNAPSHOT_HEADER: &str = "### Decentraland json snapshot";

/// `SNAPSHOT_GENERATION_INTERVAL_HOURS`' default, the old binary's.
pub const DEFAULT_INTERVAL_HOURS: u64 = 6;

pub const JOB_NAME: &str = "content-snapshots";

/// The job wakes this often to see whether a pass is due or was requested;
/// a wake that finds neither is two atomic loads.
const TICK: Duration = Duration::from_secs(15);

/// A failed pass is tried again after this long (or the interval, when shorter).
const RETRY_AFTER_FAILURE: Duration = Duration::from_secs(15 * 60);

/// Rows between cancellation checks while a window streams.
const CANCEL_CHECK_ROWS: u64 = 10_000;

#[derive(Debug, thiserror::Error)]
pub enum GenerationError {
    #[error("database: {0}")]
    Db(#[from] sqlx::Error),
    #[error("storage: {0}")]
    Storage(#[from] crate::storage::StorageError),
    #[error("{0}")]
    Invalid(String),
    #[error("cancelled")]
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub interval: Duration,
    pub tick: Duration,
}

impl Settings {
    /// `None` for 0: generation off (the job is not spawned and the console's
    /// regenerate answers the old 501).
    pub fn from_hours(hours: u64) -> Option<Self> {
        (hours > 0).then(|| Self {
            interval: Duration::from_secs(hours.saturating_mul(3600)),
            tick: TICK,
        })
    }
}

/// What one pass did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PassSummary {
    pub windows: usize,
    pub reused: usize,
    pub generated: usize,
    pub rekeyed: usize,
    pub pruned: usize,
    pub entities_written: u64,
    pub bytes_written: u64,
}

/// The generator's state as the console reports it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratorReport {
    pub interval_hours: u64,
    /// The gate: `Syncing` on a node with a sync worker, always on one without.
    pub ready: bool,
    pub sync_state: &'static str,
    pub requested: bool,
    pub running: bool,
    pub last_started_ms: Option<i64>,
    pub last_finished_ms: Option<i64>,
    pub last_summary: Option<PassSummary>,
    pub last_error: Option<String>,
}

#[derive(Default)]
struct LastPass {
    attempt: Option<Instant>,
    succeeded: bool,
    started_ms: Option<i64>,
    finished_ms: Option<i64>,
    summary: Option<PassSummary>,
    error: Option<String>,
}

pub struct SnapshotGenerator {
    content: Arc<ContentState>,
    sync_attached: bool,
    settings: Settings,
    requested: AtomicBool,
    running: tokio::sync::Mutex<()>,
    /// Hashes whose stored bytes this process wrote, or re-read and matched.
    /// The store is content-addressed, so a match is permanent; re-deriving
    /// it every pass would re-read every reused snapshot, gigabytes an hour,
    /// to learn the same fact.
    verified: Mutex<HashSet<String>>,
    last: Mutex<LastPass>,
}

impl SnapshotGenerator {
    /// `sync_attached`: whether this process runs the sync worker, whose
    /// `Syncing` state then gates generation.
    pub fn new(content: Arc<ContentState>, sync_attached: bool, settings: Settings) -> Arc<Self> {
        Arc::new(Self {
            content,
            sync_attached,
            settings,
            requested: AtomicBool::new(false),
            running: tokio::sync::Mutex::new(()),
            verified: Mutex::new(HashSet::new()),
            last: Mutex::new(LastPass::default()),
        })
    }

    /// Registers the periodic job (`content-snapshots`).
    pub fn spawn_job(self: &Arc<Self>, jobs: &Jobs) {
        let generator = self.clone();
        jobs.spawn_periodic_leased(
            JOB_NAME,
            PeriodicCfg::every(self.settings.tick),
            move |ctx| {
                let generator = generator.clone();
                async move { generator.tick(ctx).await }
            },
        );
    }

    pub fn ready(&self) -> bool {
        !self.sync_attached || self.content.sync.state() == SyncState::Syncing
    }

    /// The console's regenerate: at its next wake (within the tick), once the
    /// gate is open, the job rebuilds every window from the current rows
    /// instead of reusing any. This is the rebuild path of the derived rows:
    /// it also refreshes a window whose entity was overwritten, which the
    /// outdated probe (active rows landed after the generation) cannot see.
    /// A window whose lines did not change keeps its CID.
    pub fn request_regeneration(&self) -> GeneratorReport {
        self.requested.store(true, Ordering::SeqCst);
        self.report()
    }

    pub fn report(&self) -> GeneratorReport {
        let last = self.last.lock();
        GeneratorReport {
            interval_hours: self.settings.interval.as_secs() / 3600,
            ready: self.ready(),
            sync_state: self.content.sync.state_name(),
            requested: self.requested.load(Ordering::SeqCst),
            running: self.running.try_lock().is_err(),
            last_started_ms: last.started_ms,
            last_finished_ms: last.finished_ms,
            last_summary: last.summary.clone(),
            last_error: last.error.clone(),
        }
    }

    fn due(&self) -> bool {
        let last = self.last.lock();
        match last.attempt {
            None => true,
            Some(at) => {
                let wait = if last.succeeded {
                    self.settings.interval
                } else {
                    self.settings.interval.min(RETRY_AFTER_FAILURE)
                };
                at.elapsed() >= wait
            }
        }
    }

    async fn tick(&self, ctx: JobContext) -> anyhow::Result<()> {
        if !self.ready() {
            return Ok(());
        }
        let forced = self.requested.swap(false, Ordering::SeqCst);
        if !forced && !self.due() {
            return Ok(());
        }
        let end_ms = chrono::Utc::now().timestamp_millis();
        match self.run_pass(end_ms, forced, Some(&ctx.cancel)).await {
            Ok(summary) => {
                ctx.progress(format!(
                    "{} windows: {} reused, {} generated, {} pruned",
                    summary.windows, summary.reused, summary.generated, summary.pruned
                ));
                Ok(())
            }
            Err(GenerationError::Cancelled) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("snapshot generation: {e}")),
        }
    }

    /// One pass over the windows from [`SNAPSHOTS_INIT_TIMESTAMP_MS`] to
    /// `end_ms`, as the job runs it (tests pass a fixed `end_ms`). Returns the
    /// windows' metadata in time order.
    pub async fn generate(
        &self,
        end_ms: Timestamp,
    ) -> Result<(Vec<SnapshotMetadata>, PassSummary), GenerationError> {
        let summary = self.run_pass(end_ms, false, None).await?;
        Ok((all_snapshots(&self.content.pool).await?, summary))
    }

    /// The pass a regenerate request runs: every window rebuilt, none reused.
    pub async fn regenerate(
        &self,
        end_ms: Timestamp,
    ) -> Result<(Vec<SnapshotMetadata>, PassSummary), GenerationError> {
        let summary = self.run_pass(end_ms, true, None).await?;
        Ok((all_snapshots(&self.content.pool).await?, summary))
    }

    async fn run_pass(
        &self,
        end_ms: Timestamp,
        rebuild: bool,
        cancel: Option<&CancellationToken>,
    ) -> Result<PassSummary, GenerationError> {
        let _running = self.running.lock().await;
        {
            let mut last = self.last.lock();
            last.attempt = Some(Instant::now());
            last.started_ms = Some(chrono::Utc::now().timestamp_millis());
        }
        let started = Instant::now();
        let outcome = self.pass(end_ms, rebuild, cancel).await;
        let mut last = self.last.lock();
        last.finished_ms = Some(chrono::Utc::now().timestamp_millis());
        match &outcome {
            Ok(summary) => {
                info!(
                    rebuild,
                    windows = summary.windows,
                    reused = summary.reused,
                    generated = summary.generated,
                    rekeyed = summary.rekeyed,
                    pruned = summary.pruned,
                    entities = summary.entities_written,
                    bytes = summary.bytes_written,
                    elapsed_ms = started.elapsed().as_millis() as u64,
                    "Snapshot generation complete"
                );
                last.succeeded = true;
                last.summary = Some(summary.clone());
                last.error = None;
                // A pass over an empty `content.deployments` wrote nothing and
                // is not the interval's generation: it leaves the job due, so
                // the first deployment (a local deploy on a node without a
                // sync worker) gets its windows at the next tick, not
                // SNAPSHOT_GENERATION_INTERVAL_HOURS later. The probe that
                // decides it is one indexed row read per tick.
                if summary.windows == 0 {
                    last.attempt = None;
                }
            }
            Err(e) => {
                last.succeeded = false;
                last.error = Some(e.to_string());
            }
        }
        outcome
    }

    async fn pass(
        &self,
        end_ms: Timestamp,
        rebuild: bool,
        cancel: Option<&CancellationToken>,
    ) -> Result<PassSummary, GenerationError> {
        let pool = &self.content.pool;
        // Nothing deployed, nothing to snapshot: an empty database writes no
        // file and no row (the old generator wrote fifteen empty windows), so
        // a stamped host that has synced nothing yet stays empty for
        // `catalyst-restore --replace-empty`. One row probe, not a count.
        let any: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM content.deployments)")
            .fetch_one(pool)
            .await?;
        if !any {
            return Ok(PassSummary::default());
        }
        let division = time_range::divide_time_in_years_months_weeks_and_days(&TimeRange {
            init_timestamp: SNAPSHOTS_INIT_TIMESTAMP_MS,
            end_timestamp: end_ms,
        });
        let saved = saved_rows(pool).await?;
        let mut summary = PassSummary {
            windows: division.intervals.len(),
            ..Default::default()
        };

        for interval in &division.intervals {
            if cancel.is_some_and(CancellationToken::is_cancelled) {
                return Err(GenerationError::Cancelled);
            }
            if let Some(candidate) = saved.iter().find(|s| !rebuild && s.time_range == *interval) {
                let outdated = snapshot_is_outdated(pool, interval, candidate.generation_timestamp)
                    .await
                    .unwrap_or_else(|e| {
                        warn!(error = %e, "outdated probe failed; regenerating the window");
                        true
                    });
                if !outdated {
                    match self.reusable(candidate).await {
                        Reuse::Verified => {
                            summary.reused += 1;
                            continue;
                        }
                        Reuse::Rekeyed => {
                            summary.reused += 1;
                            summary.rekeyed += 1;
                            continue;
                        }
                        Reuse::No => {}
                    }
                }
            }
            let written = generate_window(pool, &self.content.storage, *interval, cancel).await?;
            self.verified.lock().insert(written.hash);
            summary.generated += 1;
            summary.entities_written += written.entities;
            summary.bytes_written += written.bytes;
        }

        summary.pruned = prune_outside(pool, &division.intervals).await?;
        if summary.generated + summary.rekeyed + summary.pruned > 0 {
            self.content.deployments_cache.clear();
        }
        Ok(summary)
    }

    /// Reuse is the one path that republishes a hash without recomputing it,
    /// so it is the one path that has to check: `exist()` answers "is there a
    /// file at this key", not whether the bytes hash to it (a 2026-05 bulk
    /// import wrote five snapshots under CIDs from a broken multi-level DAG,
    /// and the frozen windows re-advertised them for months). When bytes and
    /// key disagree the bytes are the truth: the file is re-keyed, the row
    /// follows, and the valid content is kept.
    async fn reusable(&self, candidate: &SavedRow) -> Reuse {
        let Some(advertised) = candidate.hash.as_deref() else {
            return Reuse::No;
        };
        if self.verified.lock().contains(advertised) {
            return Reuse::Verified;
        }
        let storage = &self.content.storage;
        let actual = match storage.stored_content_hash(advertised).await {
            Ok(Some(actual)) => actual,
            Ok(None) => return Reuse::No,
            Err(e) => {
                error!(hash = advertised, error = %e, "Failed to read snapshot content to verify its hash");
                return Reuse::No;
            }
        };
        if actual == advertised {
            self.verified.lock().insert(actual);
            return Reuse::Verified;
        }
        warn!(
            advertised,
            actual, "Stored snapshot does not hash to its advertised CID; re-keying"
        );
        match storage.rekey(advertised, &actual).await {
            Ok(true) => {}
            Ok(false) => return Reuse::No,
            Err(e) => {
                error!(advertised, actual, error = %e, "Failed to re-key snapshot content");
                return Reuse::No;
            }
        }
        match update_snapshot_hash(
            &self.content.pool,
            &candidate.time_range,
            advertised,
            &actual,
        )
        .await
        {
            Ok(true) => {
                self.verified.lock().insert(actual);
                Reuse::Rekeyed
            }
            Ok(false) => {
                warn!(
                    advertised,
                    "Snapshot row already moved by another writer; regenerating"
                );
                Reuse::No
            }
            Err(e) => {
                error!(advertised, actual, error = %e, "Failed to re-key snapshot row");
                Reuse::No
            }
        }
    }
}

enum Reuse {
    Verified,
    Rekeyed,
    No,
}

/// One `content.snapshots` row as the generator reads it.
#[derive(Debug, Clone)]
struct SavedRow {
    hash: Option<String>,
    time_range: TimeRange,
    generation_timestamp: Timestamp,
}

struct Written {
    hash: String,
    entities: u64,
    bytes: u64,
}

/// A line of the file. Field order and names are the wire (and the CID).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotLine<'a> {
    pub entity_id: &'a str,
    pub entity_type: &'a str,
    pub pointers: &'a [String],
    pub auth_chain: &'a Value,
    pub entity_timestamp: i64,
}

#[derive(sqlx::FromRow)]
struct WindowRow {
    entity_id: String,
    entity_type: String,
    pointers: Vec<String>,
    auth_chain: Value,
    entity_timestamp: i64,
}

/// The window's active deployments, inclusive at both ends, in file order.
/// The ORDER BY names the table's columns (`d.`): the bare name would bind
/// to the millisecond output column of the same name, a sort no index can
/// serve. Planned per call (`persistent(false)`): a day window is a backward
/// range scan of `deployments_entity_timestamp_entity_id_idx`
/// (`entity_timestamp DESC, entity_id DESC`, this order without a sort),
/// a year window whatever its row count makes cheapest
/// (`METRICS.md`, "Snapshot generator").
const WINDOW_SQL: &str = r#"
    SELECT d.entity_id,
           d.entity_type,
           d.entity_pointers AS pointers,
           d.auth_chain,
           (date_part('epoch', d.entity_timestamp) * 1000)::bigint AS entity_timestamp
    FROM content.deployments d
    WHERE d.deleter_deployment IS NULL
      AND d.entity_timestamp BETWEEN $1 AND $2
      AND NOT EXISTS (
          SELECT 1 FROM content.local_entities le
          WHERE le.entity_id = d.entity_id AND le.tombstoned_at IS NOT NULL)
    ORDER BY d.entity_timestamp, d.entity_id
"#;

/// Whether an active deployment in the window landed after `generation_ms`
/// (the old `snapshot_is_outdated`, inclusive window, strict `>`), or a
/// local unpublish since then touched it: a scene tombstoned after the
/// generation shares a pointer with a deployment of the window (the scene
/// itself, which the windows leave out once tombstoned, and the older
/// scenes the unpublish restored, whose `local_timestamp` is old). The
/// second leg reads the tombstones since the generation
/// (`local_entities_tombstoned_idx`, a handful) and the GIN pointer index. Planned
/// per call (`persistent(false)`): with the values, the planner walks
/// `deployments_local_timestamp_lower_entity_id_idx` from the generation
/// time (the rows landed since, a handful) and filters the window; the
/// generic plan a cached statement falls back to after five calls scans the
/// whole window's range of the `entity_timestamp` index instead (161,758
/// rows and 19,715 buffers against 3 in `METRICS.md`, "Snapshot generator").
pub async fn snapshot_is_outdated(
    pool: &PgPool,
    window: &TimeRange,
    generation_ms: Timestamp,
) -> Result<bool, GenerationError> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (
             SELECT 1 FROM content.deployments
             WHERE deleter_deployment IS NULL
               AND entity_timestamp BETWEEN $1 AND $2
               AND local_timestamp > $3)
          OR EXISTS (
             SELECT 1 FROM content.local_entities le
             JOIN content.deployments t ON t.entity_id = le.entity_id
             JOIN content.deployments d ON d.entity_pointers && t.entity_pointers
             WHERE le.tombstoned_at > $3
               AND d.entity_timestamp BETWEEN $1 AND $2)",
    )
    .persistent(false)
    .bind(naive(window.init_timestamp)?)
    .bind(naive(window.end_timestamp)?)
    .bind(naive(generation_ms)?)
    .fetch_one(pool)
    .await?)
}

/// The instant a window read is complete as of: the start of the oldest
/// transaction open on this database, or now. Any row the read cannot see
/// belongs to a transaction open at this call or begun after it, and its
/// `local_timestamp` (that transaction's `now()`) is at or after this
/// value, so the outdated probe's strict `>` flags its window on the next
/// pass. Called before the read; a millisecond is taken off because the
/// column carries microseconds and the row stores milliseconds. The
/// sessions are visible because every writer of `content.deployments` runs
/// as this pool's role.
async fn read_horizon_ms(pool: &PgPool) -> Result<Timestamp, GenerationError> {
    let ms: i64 = sqlx::query_scalar(
        "SELECT floor(EXTRACT(EPOCH FROM LEAST(now(), min(xact_start))) * 1000)::bigint
         FROM pg_stat_activity
         WHERE datname = current_database()
           AND pid <> pg_backend_pid()
           AND xact_start IS NOT NULL",
    )
    .fetch_one(pool)
    .await?;
    Ok(ms - 1)
}

async fn generate_window(
    pool: &PgPool,
    storage: &crate::storage::Storage,
    window: TimeRange,
    cancel: Option<&CancellationToken>,
) -> Result<Written, GenerationError> {
    let started = Instant::now();
    let generation_ms = read_horizon_ms(pool).await?;
    let (bytes, entities) = read_window(pool, &window, cancel).await?;
    let number_of_entities = i32::try_from(entities).map_err(|_| {
        GenerationError::Invalid(format!(
            "snapshot deployment count {entities} overflows the i32 column"
        ))
    })?;
    let (hash, bytes) =
        tokio::task::spawn_blocking(move || (crate::auth::hash_bytes_v1(&bytes), bytes))
            .await
            .map_err(|e| GenerationError::Invalid(format!("hashing task: {e}")))?;
    let size = bytes.len() as u64;
    storage.store(&hash, Bytes::from(bytes)).await?;

    let mut tx = pool.begin().await?;
    let gone: Vec<Option<String>> = sqlx::query_scalar(
        "WITH gone AS (
             DELETE FROM content.snapshots
             WHERE init_timestamp >= $1 AND end_timestamp <= $2
             RETURNING hash, init_timestamp, end_timestamp)
         SELECT hash FROM gone ORDER BY init_timestamp, end_timestamp",
    )
    .bind(naive(window.init_timestamp)?)
    .bind(naive(window.end_timestamp)?)
    .fetch_all(&mut *tx)
    .await?;
    let replaced: Vec<String> = gone.into_iter().flatten().filter(|h| *h != hash).collect();
    sqlx::query(
        "INSERT INTO content.snapshots
             (hash, init_timestamp, end_timestamp, replaced_hashes, number_of_entities, generation_time)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&hash)
    .bind(naive(window.init_timestamp)?)
    .bind(naive(window.end_timestamp)?)
    .bind(&replaced)
    .bind(number_of_entities)
    .bind(naive(generation_ms)?)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    info!(
        hash,
        window = time_range::interval_size_label(&window),
        init = window.init_timestamp,
        end = window.end_timestamp,
        entities,
        bytes = size,
        replaced = replaced.len(),
        elapsed_ms = started.elapsed().as_millis() as u64,
        "Snapshot generated"
    );
    Ok(Written {
        hash,
        entities,
        bytes: size,
    })
}

/// The file bytes and the line count. Rows stream from one connection into
/// the buffer; the buffer is the whole file (the store takes it whole).
async fn read_window(
    pool: &PgPool,
    window: &TimeRange,
    cancel: Option<&CancellationToken>,
) -> Result<(Vec<u8>, u64), GenerationError> {
    let mut buf = Vec::with_capacity(64 * 1024);
    buf.extend_from_slice(SNAPSHOT_HEADER.as_bytes());
    buf.push(b'\n');
    let mut rows = sqlx::query_as::<_, WindowRow>(WINDOW_SQL)
        .persistent(false)
        .bind(naive(window.init_timestamp)?)
        .bind(naive(window.end_timestamp)?)
        .fetch(pool);
    let mut count = 0u64;
    while let Some(row) = rows.try_next().await? {
        write_line(
            &mut buf,
            &SnapshotLine {
                entity_id: &row.entity_id,
                entity_type: &row.entity_type,
                pointers: &row.pointers,
                auth_chain: &row.auth_chain,
                entity_timestamp: row.entity_timestamp,
            },
        )?;
        count += 1;
        if count.is_multiple_of(CANCEL_CHECK_ROWS)
            && cancel.is_some_and(CancellationToken::is_cancelled)
        {
            return Err(GenerationError::Cancelled);
        }
    }
    Ok((buf, count))
}

pub fn write_line(buf: &mut Vec<u8>, line: &SnapshotLine<'_>) -> Result<(), GenerationError> {
    serde_json::to_writer(&mut *buf, line).map_err(|e| GenerationError::Invalid(e.to_string()))?;
    buf.push(b'\n');
    Ok(())
}

async fn saved_rows(pool: &PgPool) -> Result<Vec<SavedRow>, GenerationError> {
    let rows: Vec<(Option<String>, NaiveDateTime, NaiveDateTime, NaiveDateTime)> = sqlx::query_as(
        "SELECT hash, init_timestamp, end_timestamp, generation_time FROM content.snapshots",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(hash, init, end, generated)| SavedRow {
            hash,
            time_range: TimeRange {
                init_timestamp: init.and_utc().timestamp_millis(),
                end_timestamp: end.and_utc().timestamp_millis(),
            },
            generation_timestamp: generated.and_utc().timestamp_millis(),
        })
        .collect())
}

/// hash, init_timestamp, end_timestamp, replaced_hashes, number_of_entities, generation_time.
type SnapshotRow = (
    Option<String>,
    NaiveDateTime,
    NaiveDateTime,
    Vec<String>,
    i32,
    NaiveDateTime,
);

/// Every row, oldest window first, in the wire shape `/snapshots` serves.
pub async fn all_snapshots(pool: &PgPool) -> Result<Vec<SnapshotMetadata>, GenerationError> {
    let rows: Vec<SnapshotRow> = sqlx::query_as(
        "SELECT hash, init_timestamp, end_timestamp, replaced_hashes, number_of_entities, generation_time
         FROM content.snapshots
         ORDER BY init_timestamp, end_timestamp",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(hash, init, end, replaced, entities, generated)| SnapshotMetadata {
                hash: hash.unwrap_or_default(),
                time_range: TimeRange {
                    init_timestamp: init.and_utc().timestamp_millis(),
                    end_timestamp: end.and_utc().timestamp_millis(),
                },
                number_of_entities: entities.max(0) as u64,
                replaced_snapshot_hashes: Some(replaced),
                generation_timestamp: generated.and_utc().timestamp_millis(),
            },
        )
        .collect())
}

/// Pinned to the old hash as well as the window, so two repairs racing for
/// one row cannot both claim it: the second matches nothing and regenerates.
async fn update_snapshot_hash(
    pool: &PgPool,
    window: &TimeRange,
    old_hash: &str,
    new_hash: &str,
) -> Result<bool, GenerationError> {
    let done = sqlx::query(
        "UPDATE content.snapshots SET hash = $1
         WHERE hash = $2 AND init_timestamp = $3 AND end_timestamp = $4",
    )
    .bind(new_hash)
    .bind(old_hash)
    .bind(naive(window.init_timestamp)?)
    .bind(naive(window.end_timestamp)?)
    .execute(pool)
    .await?;
    Ok(done.rows_affected() > 0)
}

/// Deletes every row whose window is not one of `windows` (a division from
/// another genesis, a remainder-era row); their contents were folded into a
/// current window, whose row replaced them, or were never whole days.
async fn prune_outside(pool: &PgPool, windows: &[TimeRange]) -> Result<usize, GenerationError> {
    let inits = windows
        .iter()
        .map(|w| naive(w.init_timestamp))
        .collect::<Result<Vec<_>, _>>()?;
    let ends = windows
        .iter()
        .map(|w| naive(w.end_timestamp))
        .collect::<Result<Vec<_>, _>>()?;
    let pruned = sqlx::query(
        "DELETE FROM content.snapshots s
         WHERE NOT EXISTS (
             SELECT 1 FROM unnest($1::timestamp[], $2::timestamp[]) AS w(init_timestamp, end_timestamp)
             WHERE w.init_timestamp = s.init_timestamp AND w.end_timestamp = s.end_timestamp)",
    )
    .bind(&inits)
    .bind(&ends)
    .execute(pool)
    .await?
    .rows_affected();
    if pruned > 0 {
        info!(pruned, "Pruned snapshot rows outside the division windows");
    }
    Ok(pruned as usize)
}

/// Milliseconds since the epoch as the UTC `timestamp without time zone`
/// the columns hold.
fn naive(ms: Timestamp) -> Result<NaiveDateTime, GenerationError> {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|t| t.naive_utc())
        .ok_or_else(|| GenerationError::Invalid(format!("timestamp {ms} ms is out of range")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The old generator serialized its `SnapshotSyncDeployment` (these five
    /// fields in this order, camelCase) with `serde_json::to_string` and a
    /// newline; the auth chain came back from the `json` column as a
    /// `serde_json::Value`, whose maps are sorted without `preserve_order`.
    #[test]
    fn a_line_is_the_old_serialization() {
        let chain = json!([
            {"type": "SIGNER", "payload": "0xabc", "signature": ""},
            {"type": "ECDSA_SIGNED_ENTITY", "payload": "bafyid", "signature": "0x01"}
        ]);
        let pointers = vec!["0,0".to_string(), "urn:decentraland:ethereum:x".to_string()];
        let mut buf = Vec::new();
        write_line(
            &mut buf,
            &SnapshotLine {
                entity_id: "bafyid",
                entity_type: "scene",
                pointers: &pointers,
                auth_chain: &chain,
                entity_timestamp: 1_600_000_000_123,
            },
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "{\"entityId\":\"bafyid\",\"entityType\":\"scene\",\"pointers\":[\"0,0\",\"urn:decentraland:ethereum:x\"],\
             \"authChain\":[{\"payload\":\"0xabc\",\"signature\":\"\",\"type\":\"SIGNER\"},\
             {\"payload\":\"bafyid\",\"signature\":\"0x01\",\"type\":\"ECDSA_SIGNED_ENTITY\"}],\
             \"entityTimestamp\":1600000000123}\n"
        );
    }

    /// The sync reader's line type parses what the generator writes, and the
    /// header is the line it skips (`###`).
    #[test]
    fn the_sync_reader_parses_a_generated_file() {
        let chain = json!([{"type": "SIGNER", "payload": "0xabc"}]);
        let pointers = vec!["0xabc".to_string()];
        let mut buf = format!("{SNAPSHOT_HEADER}\n").into_bytes();
        for (id, ts) in [("bafya", 1_000), ("bafyb", 2_000)] {
            write_line(
                &mut buf,
                &SnapshotLine {
                    entity_id: id,
                    entity_type: "profile",
                    pointers: &pointers,
                    auth_chain: &chain,
                    entity_timestamp: ts,
                },
            )
            .unwrap();
        }
        let text = String::from_utf8(buf).unwrap();
        let mut lines = text.lines();
        assert!(lines.next().unwrap().starts_with("###"));
        let parsed: Vec<crate::sync_types::SyncDeployment> =
            lines.map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].entity_id, "bafya");
        assert_eq!(parsed[1].entity_timestamp, 2_000);
        assert_eq!(parsed[1].local_timestamp, None);
    }

    #[test]
    fn zero_hours_turns_generation_off() {
        assert_eq!(Settings::from_hours(0), None);
        let s = Settings::from_hours(DEFAULT_INTERVAL_HOURS).unwrap();
        assert_eq!(s.interval, Duration::from_secs(6 * 3600));
        assert_eq!(s.tick, TICK);
    }

    #[test]
    fn naive_round_trips_milliseconds_as_utc() {
        let t = naive(1_577_836_800_123).unwrap();
        assert_eq!(t.to_string(), "2020-01-01 00:00:00.123");
        assert_eq!(t.and_utc().timestamp_millis(), 1_577_836_800_123);
    }
}
