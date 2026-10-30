//! The write path peer sync drives: entity files that already passed the
//! content-hash check are verified (auth chain), buffered, and flushed in
//! batches of up to [`BATCH_SIZE`] rows into `content.deployments`,
//! `content.content_files` and `content.active_pointers` under the same pointer
//! locks the local deploy function takes. A flush that fails re-reports every
//! entity it carried to `content.failed_deployments`, where the retry worker
//! owns it.
//!
//! The old node relied on migration 0007's `dcl_auth_chain_consistent` CHECK
//! (a Postgres extension, absent in production: the check was a no-op there)
//! to refuse an inconsistent auth chain at insert time; it recovered ECDSA
//! links and passed EIP-1654 links through. v2 has no extension:
//! [`LiveSyncDeployer::deploy_entity`] runs `verify_peer_auth_chain` with the
//! same rule -- ECDSA recovered locally, ephemeral expiry against the entity
//! timestamp, contract-wallet links accepted as the peer validated them
//! ([`crate::auth::PeerValidated`]), an expiration that parses as no known date
//! accepted as the peer's verdict too (the old node read it with `new Date`;
//! 2020 history carries `Date.toString()` payloads) -- before an entity is
//! buffered, so sync never depends on an RPC and a contract's answer today
//! never refuses the peer's history. A failed verification comes back as
//! [`SyncError::DeploymentRejected`], which the callers record as a failed
//! deployment with that reason.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde_json::Value;
use sqlx::PgPool;
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};

use super::batch_deployer::DeploymentReport;
use super::deploy_remote_entity::ByteBudget;
use super::sync_types::{
    AuthChain, DeploymentContext, FailedDeployment, FailureReason, SyncError, Timestamp,
};
use super::write_deployer::lock_deployment_pointers;
use crate::auth::verify::{ChainVerdict, verify_peer_auth_chain};

mod scene_parcels;

struct ParsedEntity {
    deployer_address: String,
    version: String,
    entity_type: String,
    entity_id: String,
    entity_metadata: Value,
    entity_timestamp: f64,
    entity_pointers: Vec<String>,
    auth_chain: Value,
    content: Vec<(String, String)>,
    report: Option<Arc<DeploymentReport>>,
    /// Retry state the entity must be recorded with if the batch flush that carries it fails.
    /// The deployer is asynchronous -- `deploy_entity` returns once the entity is buffered, so
    /// the caller's attempt is already spent by the time a flush failure re-reports it, and
    /// reporting a fresh 0/0 here would rewind the backoff the retry worker had accrued.
    /// The wait stays relative: the deadline is stamped when the failure is recorded, not when
    /// the attempt was dispatched, so a slow attempt does not eat the wait it just earned.
    retry_count: u32,
    backoff_ms: u64,
    /// The flush-backlog slot this entity occupies from `deploy_entity` until the flush that
    /// carries it is done ([`SyncMemoryLimits::flush_backlog_max`]); dropped with the entity.
    backlog_permit: Option<OwnedSemaphorePermit>,
}

fn flush_failure_record(
    entity: &ParsedEntity,
    auth_chain: AuthChain,
    error_description: String,
    retry: (u32, u64),
) -> FailedDeployment {
    let now_ms = chrono::Utc::now().timestamp_millis();
    FailedDeployment {
        entity_type: entity.entity_type.clone(),
        entity_id: entity.entity_id.clone(),
        reason: FailureReason::DeploymentError,
        auth_chain,
        error_description,
        failure_timestamp: now_ms,
        snapshot_hash: None,
        retry_count: retry.0,
        next_retry_at: now_ms + retry.1 as i64,
    }
}

/// Strongest retry state per entity id in a batch. One batch can carry the same entity twice --
/// the sync path keeps re-encountering exactly the entity the retry worker keeps re-attempting --
/// and the single record a failed flush writes has to be the stronger sighting, or the accrued
/// attempts are lost to whichever copy happened to be buffered first.
fn strongest_retry_state(entities: &[ParsedEntity]) -> HashMap<&str, (u32, u64)> {
    let mut strongest: HashMap<&str, (u32, u64)> = HashMap::with_capacity(entities.len());
    for entity in entities {
        let slot = strongest
            .entry(entity.entity_id.as_str())
            .or_insert((entity.retry_count, entity.backoff_ms));
        slot.0 = slot.0.max(entity.retry_count);
        slot.1 = slot.1.max(entity.backoff_ms);
    }
    strongest
}

fn reason_label(reason: &FailureReason) -> &'static str {
    match reason {
        FailureReason::DeploymentError => "Deployment error",
        FailureReason::NoEntity => "No entity",
    }
}

fn reason_from_label(label: &str) -> FailureReason {
    match label {
        "No entity" => FailureReason::NoEntity,
        _ => FailureReason::DeploymentError,
    }
}

fn parse_entity_for_deploy(
    entity_data: &[u8],
    entity_id: &str,
    auth_chain: &AuthChain,
) -> Result<ParsedEntity, SyncError> {
    let entity: Value = serde_json::from_slice(entity_data)?;

    let pointers: Vec<String> = entity
        .get("pointers")
        .and_then(|p| p.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_lowercase()))
                .collect()
        })
        .unwrap_or_default();

    let timestamp = entity
        .get("timestamp")
        .and_then(|t| t.as_f64())
        .unwrap_or(0.0);
    let entity_type = entity
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("unknown")
        .to_string();
    let version = entity
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("v3")
        .to_string();

    let deployer = auth_chain
        .first()
        .map(|link| link.payload.clone())
        .unwrap_or_default();

    let content: Vec<(String, String)> = entity
        .get("content")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|c| {
                    let file = c.get("file").and_then(|f| f.as_str());
                    let hash = c.get("hash").and_then(|h| h.as_str());
                    match (file, hash) {
                        (Some(f), Some(h)) => Some((f.to_string(), h.to_string())),
                        _ => None,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let auth_chain_json =
        serde_json::to_value(auth_chain).map_err(|e| SyncError::Storage(e.to_string()))?;
    let metadata = match entity.get("metadata") {
        Some(m) if !m.is_null() => serde_json::json!({"v": m}),
        _ => Value::Null,
    };

    Ok(ParsedEntity {
        deployer_address: deployer,
        version,
        entity_type,
        entity_id: entity_id.to_string(),
        entity_metadata: metadata,
        entity_timestamp: timestamp,
        entity_pointers: pointers,
        auth_chain: auth_chain_json,
        content,
        report: None,
        retry_count: 0,
        backoff_ms: 0,
        backlog_permit: None,
    })
}

/// The ephemeral links are checked against the entity timestamp, as upstream does: a key
/// that expired after signing still signed a valid deployment. Contract-wallet links and
/// expirations no known date shape parses are the peer's verdict (the verdict lists the
/// latter); every other failure is a rejection the callers record.
async fn verify_entity_auth_chain(
    entity_id: &str,
    auth_chain: &AuthChain,
    entity_timestamp_ms: i64,
) -> Result<ChainVerdict, SyncError> {
    verify_peer_auth_chain(auth_chain, entity_id, entity_timestamp_ms)
        .await
        .map_err(|e| {
            SyncError::DeploymentRejected(format!(
                "auth chain verification failed for {entity_id}: {e}"
            ))
        })
}

/// What [`LiveSyncDeployer::deploy_entity`] checks before it buffers, without buffering: the
/// read-through path runs it before it stores the entity file, so a forged chain leaves nothing
/// behind.
pub async fn verify_synced_entity(
    entity_data: &[u8],
    entity_id: &str,
    auth_chain: &AuthChain,
) -> Result<(), SyncError> {
    let parsed = parse_entity_for_deploy(entity_data, entity_id, auth_chain)?;
    verify_entity_auth_chain(entity_id, auth_chain, parsed.entity_timestamp as i64).await?;
    Ok(())
}

pub const BATCH_SIZE: usize = 500;
const BATCH_TIMEOUT_MS: u64 = 200;
/// The old `SYNC_FLUSH_CONCURRENCY` default.
pub const DEFAULT_FLUSH_CONCURRENCY: usize = 8;

/// `SYNC_INFLIGHT_BYTES_MAX`: decoded file bodies in memory across every in-flight download.
pub const DEFAULT_INFLIGHT_BYTES_MAX: u64 = 1024 * 1024 * 1024;
/// `SYNC_FLUSH_BACKLOG_MAX`: entities verified and waiting for a database flush.
pub const DEFAULT_FLUSH_BACKLOG_MAX: usize = 4096;

/// What bounds the sync write path's memory: the bytes of file bodies in flight
/// ([`ByteBudget`]) and the entities buffered ahead of the database. Without the second, a
/// database slower than the peers let the flush backlog grow without limit -- every batch a
/// spawned task holding 500 parsed entities, waiting for a flush slot -- and producers never
/// noticed; with it, `deploy_entity` waits for a slot instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncMemoryLimits {
    pub inflight_bytes_max: u64,
    pub flush_backlog_max: usize,
}

impl SyncMemoryLimits {
    pub const DEFAULT: Self = Self {
        inflight_bytes_max: DEFAULT_INFLIGHT_BYTES_MAX,
        flush_backlog_max: DEFAULT_FLUSH_BACKLOG_MAX,
    };
}

impl Default for SyncMemoryLimits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Clone)]
pub struct LiveSyncDeployer {
    pool: PgPool,
    batch: Arc<Mutex<Vec<ParsedEntity>>>,

    flush_sem: Arc<Semaphore>,
    in_flight: Arc<std::sync::atomic::AtomicUsize>,
    idle_notify: Arc<tokio::sync::Notify>,
    lost: Arc<std::sync::atomic::AtomicU64>,
    trusted_expirations: Arc<std::sync::atomic::AtomicU64>,
    /// One permit per entity between `deploy_entity` and the end of its flush.
    backlog: Arc<Semaphore>,
    flush_backlog_max: usize,
    inflight_bytes: Arc<ByteBudget>,
}

fn spawn_flush(
    pool: PgPool,
    flush_sem: Arc<Semaphore>,
    in_flight: Arc<std::sync::atomic::AtomicUsize>,
    idle_notify: Arc<tokio::sync::Notify>,
    lost: Arc<std::sync::atomic::AtomicU64>,
    entities: Vec<ParsedEntity>,
) {
    if entities.is_empty() {
        return;
    }
    in_flight.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    tokio::spawn(async move {
        let _permit = flush_sem.acquire().await;
        if let Err(e) = flush_batch(&pool, &entities).await {
            tracing::error!(error = %e, count = entities.len(), "Batch flush failed");
            let failed_store = LiveFailedDeploymentsStore::new(pool.clone());
            let mut strongest = strongest_retry_state(&entities);
            for entity in &entities {
                let Some(retry) = strongest.remove(entity.entity_id.as_str()) else {
                    continue;
                };
                let auth_chain: AuthChain = match serde_json::from_value(entity.auth_chain.clone())
                {
                    Ok(chain) => chain,
                    Err(conv_err) => {
                        lost.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        if let Some(report) = &entity.report {
                            report.record_lost();
                        }
                        tracing::error!(
                            entity_id = %entity.entity_id,
                            error = %conv_err,
                            "Entity dropped by a failed batch flush has an unconvertible auth \
                             chain; counting it as lost instead of recording an un-retryable row"
                        );
                        continue;
                    }
                };
                let failure = flush_failure_record(
                    entity,
                    auth_chain,
                    format!("batch flush failed: {}", e),
                    retry,
                );
                if let Err(record_err) = failed_store.report_failure(failure).await {
                    lost.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if let Some(report) = &entity.report {
                        report.record_lost();
                    }
                    tracing::error!(
                        entity_id = %entity.entity_id,
                        error = %record_err,
                        "Entity dropped by a failed batch flush could not be recorded as a \
                         failed deployment; holding the sync frontier back"
                    );
                }
            }
        }
        in_flight.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        idle_notify.notify_waiters();
    });
}

impl LiveSyncDeployer {
    /// `flush_concurrency` bounds concurrent batch flushes ([`DEFAULT_FLUSH_CONCURRENCY`]);
    /// the memory limits are the defaults.
    pub fn new(pool: PgPool, flush_concurrency: usize) -> Self {
        Self::with_limits(pool, flush_concurrency, SyncMemoryLimits::default())
    }

    pub fn with_limits(pool: PgPool, flush_concurrency: usize, limits: SyncMemoryLimits) -> Self {
        let flush_backlog_max = limits.flush_backlog_max.max(1);
        Self {
            pool,
            batch: Arc::new(Mutex::new(Vec::with_capacity(BATCH_SIZE))),
            flush_sem: Arc::new(Semaphore::new(flush_concurrency.max(1))),
            in_flight: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            idle_notify: Arc::new(tokio::sync::Notify::new()),
            lost: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            trusted_expirations: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            backlog: Arc::new(Semaphore::new(flush_backlog_max)),
            flush_backlog_max,
            inflight_bytes: Arc::new(ByteBudget::new(limits.inflight_bytes_max)),
        }
    }

    /// The byte budget every download through this deployer's write path draws on.
    pub fn inflight_bytes(&self) -> &Arc<ByteBudget> {
        &self.inflight_bytes
    }

    /// Entities verified and buffered or in a flush that has not finished.
    pub fn flush_backlog(&self) -> usize {
        self.flush_backlog_max - self.backlog.available_permits()
    }

    /// The batch ticker as a job of the shared lifecycle (`sync-batch-flush`
    /// in `/admin/api/jobs`), scoped to the leased sync job that owns this
    /// deployer (`parent`; it ends with the lease): every [`BATCH_TIMEOUT_MS`]
    /// a partial batch that no arrival completed is handed to a flush.
    /// `deploy_entity` flushes full batches itself, and [`flush`](Self::flush)
    /// drains on demand, so a deployer without the ticker (the tests) only
    /// lacks the timeout.
    pub fn spawn_flush_ticker(&self, jobs: &crate::jobs::Jobs, parent: &crate::jobs::JobContext) {
        let background = self.clone();
        let cfg =
            crate::jobs::PeriodicCfg::every(std::time::Duration::from_millis(BATCH_TIMEOUT_MS))
                .after_first_interval();
        jobs.spawn_periodic_scoped(parent, "sync-batch-flush", cfg, move |_ctx| {
            let background = background.clone();
            async move {
                let mut buf = background.batch.lock().await;
                if !buf.is_empty() {
                    background.trigger_flush(std::mem::take(&mut *buf));
                }
                Ok(())
            }
        });
    }

    /// Counts the batch in flight before returning, so callers take the buffer and call this
    /// under one lock: [`flush`](Self::flush) must never see the buffer empty while a taken
    /// batch is not yet counted, or it returns before that batch is durable.
    fn trigger_flush(&self, entities: Vec<ParsedEntity>) {
        spawn_flush(
            self.pool.clone(),
            self.flush_sem.clone(),
            self.in_flight.clone(),
            self.idle_notify.clone(),
            self.lost.clone(),
            entities,
        );
    }

    /// Entities this deployer dropped without any durable record: the batch flush failed AND
    /// recording them in failed_deployments failed too. Callers compare this before and after a
    /// drain -- any growth means the sync frontier must not advance, because nothing will
    /// re-deliver those entities.
    pub fn lost_count(&self) -> u64 {
        self.lost.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Entities buffered with an ephemeral expiration that parsed as no known date, kept on
    /// the peer's word (each one is also a debug line with its entity id).
    pub fn trusted_expiration_count(&self) -> u64 {
        self.trusted_expirations
            .load(std::sync::atomic::Ordering::SeqCst)
    }
}

async fn flush_batch(pool: &PgPool, entities: &[ParsedEntity]) -> Result<(), SyncError> {
    if entities.is_empty() {
        return Ok(());
    }

    let entities: Vec<&ParsedEntity> = {
        let mut seen = std::collections::HashSet::with_capacity(entities.len());
        entities
            .iter()
            .filter(|e| seen.insert(e.entity_id.clone()))
            .collect()
    };

    let count = entities.len();
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| SyncError::Storage(e.to_string()))?;

    {
        let all_pointers: Vec<String> = entities
            .iter()
            .flat_map(|e| e.entity_pointers.iter().cloned())
            .collect();
        lock_deployment_pointers(&mut tx, &all_pointers)
            .await
            .map_err(|e| SyncError::Storage(e.to_string()))?;
    }

    let mut deployer_addrs: Vec<String> = Vec::with_capacity(count);
    let mut versions: Vec<String> = Vec::with_capacity(count);
    let mut entity_types: Vec<String> = Vec::with_capacity(count);
    let mut entity_ids: Vec<String> = Vec::with_capacity(count);
    let mut metadatas: Vec<Value> = Vec::with_capacity(count);
    let mut timestamps: Vec<f64> = Vec::with_capacity(count);
    let mut pointers_json: Vec<Value> = Vec::with_capacity(count);
    let mut auth_chains: Vec<Value> = Vec::with_capacity(count);

    for e in &entities {
        deployer_addrs.push(e.deployer_address.clone());
        versions.push(e.version.clone());
        entity_types.push(e.entity_type.clone());
        entity_ids.push(e.entity_id.clone());
        metadatas.push(e.entity_metadata.clone());
        timestamps.push(e.entity_timestamp);
        pointers_json.push(Value::Array(
            e.entity_pointers
                .iter()
                .map(|p| Value::String(p.clone()))
                .collect(),
        ));
        auth_chains.push(e.auth_chain.clone());
    }

    let rows: Vec<(String, i32)> = sqlx::query_as(
        r#"
        INSERT INTO content.deployments
            (deployer_address, version, entity_type, entity_id, entity_metadata,
             entity_timestamp, entity_pointers, local_timestamp, auth_chain)
        SELECT da, v, et, ei, em::json,
               to_timestamp(ts / 1000.0),
               ARRAY(SELECT json_array_elements_text(ep::json)),
               now(), ac::json
        FROM unnest(
            $1::text[], $2::text[], $3::text[], $4::text[],
            $5::jsonb[], $6::float8[], $7::jsonb[], $8::jsonb[]
        ) AS t(da, v, et, ei, em, ts, ep, ac)
        ON CONFLICT (entity_id) DO NOTHING
        RETURNING entity_id, id
        "#,
    )
    .bind(&deployer_addrs)
    .bind(&versions)
    .bind(&entity_types)
    .bind(&entity_ids)
    .bind(&metadatas)
    .bind(&timestamps)
    .bind(&pointers_json)
    .bind(&auth_chains)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| SyncError::Storage(e.to_string()))?;

    // Every entity of the batch is in content.deployments once this commits, so its failure
    // record goes in the same transaction: the retry worker never drops one earlier.
    sqlx::query("DELETE FROM content.failed_deployments WHERE entity_id = ANY($1)")
        .bind(&entity_ids)
        .execute(&mut *tx)
        .await
        .map_err(|e| SyncError::Storage(e.to_string()))?;

    if rows.is_empty() {
        tx.commit()
            .await
            .map_err(|e| SyncError::Storage(e.to_string()))?;
        return Ok(());
    }

    let id_map: HashMap<&str, i32> = rows.iter().map(|r| (r.0.as_str(), r.1)).collect();

    let mut cf_deployments: Vec<i32> = Vec::new();
    let mut cf_hashes: Vec<String> = Vec::new();
    let mut cf_keys: Vec<String> = Vec::new();

    let mut ap_dedup: HashMap<String, (String, String, f64)> = HashMap::new();

    for e in entities.iter() {
        let Some(&dep_id) = id_map.get(e.entity_id.as_str()) else {
            continue;
        };

        for (key, hash) in &e.content {
            cf_deployments.push(dep_id);
            cf_hashes.push(hash.clone());
            cf_keys.push(key.clone());
        }

        for ptr in &e.entity_pointers {
            let replace = match ap_dedup.get(ptr) {
                None => true,
                Some((existing_id, _, existing_ts)) => {
                    e.entity_timestamp > *existing_ts
                        || (e.entity_timestamp == *existing_ts
                            && e.entity_id.to_lowercase() > existing_id.to_lowercase())
                }
            };
            if replace {
                ap_dedup.insert(
                    ptr.clone(),
                    (
                        e.entity_id.clone(),
                        e.entity_type.clone(),
                        e.entity_timestamp,
                    ),
                );
            }
        }
    }

    let mut ap_pointers: Vec<String> = Vec::with_capacity(ap_dedup.len());
    let mut ap_entity_ids: Vec<String> = Vec::with_capacity(ap_dedup.len());
    let mut ap_entity_types: Vec<String> = Vec::with_capacity(ap_dedup.len());

    for (ptr, (eid, etype, _)) in ap_dedup {
        ap_pointers.push(ptr);
        ap_entity_ids.push(eid);
        ap_entity_types.push(etype);
    }

    if !cf_deployments.is_empty() {
        sqlx::query(
            r#"
            INSERT INTO content.content_files (deployment, content_hash, key)
            SELECT unnest($1::int[]), unnest($2::text[]), unnest($3::text[])
            "#,
        )
        .bind(&cf_deployments)
        .bind(&cf_hashes)
        .bind(&cf_keys)
        .execute(&mut *tx)
        .await
        .map_err(|e| SyncError::Storage(e.to_string()))?;
    }

    let mut ow_ids: Vec<i32> = Vec::with_capacity(rows.len());
    let mut ow_types: Vec<String> = Vec::with_capacity(rows.len());
    let mut ow_eids: Vec<String> = Vec::with_capacity(rows.len());
    let mut ow_ts: Vec<f64> = Vec::with_capacity(rows.len());
    let mut ow_ptrs: Vec<Value> = Vec::with_capacity(rows.len());
    for e in &entities {
        let Some(&dep_id) = id_map.get(e.entity_id.as_str()) else {
            continue;
        };
        ow_ids.push(dep_id);
        ow_types.push(e.entity_type.clone());
        ow_eids.push(e.entity_id.clone());
        ow_ts.push(e.entity_timestamp);
        ow_ptrs.push(Value::Array(
            e.entity_pointers
                .iter()
                .map(|p| Value::String(p.clone()))
                .collect(),
        ));
    }

    sqlx::query(
        r#"
        UPDATE content.deployments AS old
        SET deleter_deployment = hit.new_id
        FROM (
            WITH batch AS (
                SELECT d.new_id, d.etype, d.new_eid, d.new_ts,
                       ARRAY(SELECT json_array_elements_text(d.ptrs_json::json)) AS ptrs
                FROM (
                    SELECT unnest($1::int4[]) AS new_id,
                           unnest($2::text[]) AS etype,
                           unnest($3::text[]) AS new_eid,
                           to_timestamp(unnest($4::float8[]) / 1000.0) AS new_ts,
                           unnest($5::jsonb[]) AS ptrs_json
                ) d
            ),
            -- Pointer overlap alone drives the candidate step. With the type or
            -- timestamp conditions in the same scan the planner ANDs the GIN
            -- pointer index with a btree that covers most of the table
            -- (500k+ entries per batch row: 1-2 s per batch of 15 on a 650k
            -- row node); the GIN scan alone finds the few overlapping rows in
            -- milliseconds and the cheap conditions filter them afterwards.
            cand AS MATERIALIZED (
                SELECT c.id AS old_id, c.entity_type AS old_type,
                       c.entity_timestamp AS old_ts, c.entity_id AS old_eid,
                       c.deleter_deployment AS old_deleter,
                       batch.new_id, batch.etype, batch.new_eid, batch.new_ts
                FROM batch
                JOIN content.deployments c ON c.entity_pointers && batch.ptrs
            )
            -- The nearest newer batch row becomes the deleter unless the
            -- current one is nearer still: the deleter is always the nearest
            -- successor, whatever order the rows arrived in.
            SELECT DISTINCT ON (old_id) old_id, new_id
            FROM cand
            LEFT JOIN content.deployments cur ON cur.id = cand.old_deleter
            WHERE old_type = etype
              AND old_id <> new_id
              AND (old_ts < new_ts OR (old_ts = new_ts AND lower(old_eid) < lower(new_eid)))
              AND (cur.id IS NULL
                   OR new_ts < cur.entity_timestamp
                   OR (new_ts = cur.entity_timestamp
                       AND lower(new_eid) < lower(cur.entity_id)))
            ORDER BY old_id, new_ts, lower(new_eid)
        ) AS hit
        WHERE old.id = hit.old_id
        "#,
    )
    .bind(&ow_ids)
    .bind(&ow_types)
    .bind(&ow_eids)
    .bind(&ow_ts)
    .bind(&ow_ptrs)
    .execute(&mut *tx)
    .await
    .map_err(|e| SyncError::Storage(e.to_string()))?;

    sqlx::query(
        r#"
        UPDATE content.deployments AS n
        SET deleter_deployment = sub.newer_id
        FROM (
            SELECT nr.id,
                   (SELECT c.id FROM (
                        SELECT d2.id, d2.entity_timestamp, d2.entity_id, d2.entity_type
                        FROM content.deployments d2
                        WHERE d2.entity_pointers && nr.entity_pointers
                          AND d2.id <> nr.id
                          AND (d2.entity_timestamp > nr.entity_timestamp
                               OR (d2.entity_timestamp = nr.entity_timestamp
                                   AND lower(d2.entity_id) > lower(nr.entity_id)))
                        OFFSET 0
                    ) c
                    WHERE c.entity_type = nr.entity_type
                      -- An unpublished local scene succeeds nothing: an older
                      -- peer scene arriving after the unpublish takes the pointer.
                      AND NOT EXISTS (
                          SELECT 1 FROM content.local_entities le
                          WHERE le.entity_id = c.entity_id AND le.tombstoned_at IS NOT NULL)
                    ORDER BY c.entity_timestamp, lower(c.entity_id)
                    LIMIT 1) AS newer_id
            FROM content.deployments nr
            WHERE nr.id = ANY($1)
        ) sub
        WHERE n.id = sub.id
          AND sub.newer_id IS NOT NULL
          AND n.deleter_deployment IS DISTINCT FROM sub.newer_id
        "#,
    )
    .bind(&ow_ids)
    .execute(&mut *tx)
    .await
    .map_err(|e| SyncError::Storage(e.to_string()))?;

    sqlx::query(
        r#"
        DELETE FROM content.active_pointers AS ap
        USING content.deployments d
        WHERE ap.entity_id = d.entity_id
          AND d.deleter_deployment IS NOT NULL
          AND (d.deleter_deployment = ANY($1) OR d.id = ANY($1))
        "#,
    )
    .bind(&ow_ids)
    .execute(&mut *tx)
    .await
    .map_err(|e| SyncError::Storage(e.to_string()))?;

    if !ap_pointers.is_empty() {
        sqlx::query(
            r#"
            INSERT INTO content.active_pointers (pointer, entity_id, entity_type)
            SELECT t.pointer, t.entity_id, t.entity_type
            FROM unnest($1::text[], $2::text[], $3::text[]) AS t(pointer, entity_id, entity_type)
            JOIN content.deployments dep ON dep.entity_id = t.entity_id
            WHERE dep.deleter_deployment IS NULL
            ON CONFLICT (pointer) DO UPDATE
                SET entity_id = EXCLUDED.entity_id, entity_type = EXCLUDED.entity_type
                WHERE NOT EXISTS (
                    SELECT 1
                    FROM content.deployments cur, content.deployments incoming
                    WHERE cur.entity_id = active_pointers.entity_id
                      AND incoming.entity_id = EXCLUDED.entity_id
                      AND (cur.entity_timestamp > incoming.entity_timestamp
                           OR (cur.entity_timestamp = incoming.entity_timestamp
                               AND lower(cur.entity_id) > lower(EXCLUDED.entity_id)))
                )
            "#,
        )
        .bind(&ap_pointers)
        .bind(&ap_entity_ids)
        .bind(&ap_entity_types)
        .execute(&mut *tx)
        .await
        .map_err(|e| SyncError::Storage(e.to_string()))?;
    }

    tx.commit()
        .await
        .map_err(|e| SyncError::Storage(e.to_string()))?;

    tracing::info!(
        count = rows.len(),
        batch_size = count,
        "Batch flush committed"
    );
    Ok(())
}

impl LiveSyncDeployer {
    /// Verifies the auth chain, then buffers the entity; returns once it is buffered, so the
    /// write itself is asynchronous (see [`LiveSyncDeployer::flush`]). `retry_state` is the
    /// (attempts spent, backoff) of the retry worker's attempt, kept for the failure record a
    /// failed flush writes.
    pub async fn deploy_entity(
        &self,
        entity_data: &[u8],
        entity_id: &str,
        auth_chain: &AuthChain,
        _context: DeploymentContext,
        report: Option<&Arc<DeploymentReport>>,
        retry_state: Option<(u32, u64)>,
    ) -> Result<(), SyncError> {
        let mut parsed = parse_entity_for_deploy(entity_data, entity_id, auth_chain)?;
        let verdict =
            verify_entity_auth_chain(entity_id, auth_chain, parsed.entity_timestamp as i64).await?;
        scene_parcels::report_drift(
            &parsed.entity_type,
            entity_id,
            &parsed.entity_pointers,
            parsed.entity_metadata.get("v"),
        );
        if !verdict.trusted_expirations.is_empty() {
            self.trusted_expirations
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            tracing::debug!(
                entity_id,
                expirations = ?verdict.trusted_expirations,
                "Synced entity's ephemeral expiration parses as no known date; kept as the \
                 peer's verdict"
            );
        }
        parsed.report = report.cloned();
        if let Some((retry_count, backoff_ms)) = retry_state {
            parsed.retry_count = retry_count;
            parsed.backoff_ms = backoff_ms;
        }
        // Taken after verification, so a rejected entity never waits on the database, and
        // before buffering: a full backlog holds the producer here until a flush lands.
        parsed.backlog_permit = Some(
            self.backlog
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| SyncError::Stopped)?,
        );

        let mut buf = self.batch.lock().await;
        buf.push(parsed);
        if buf.len() >= BATCH_SIZE {
            self.trigger_flush(std::mem::take(&mut *buf));
        }

        Ok(())
    }

    /// Flushes the buffer and waits for every in-flight flush to finish.
    pub async fn flush(&self) -> Result<(), SyncError> {
        {
            let mut buf = self.batch.lock().await;
            self.trigger_flush(std::mem::take(&mut *buf));
        }

        loop {
            let notified = self.idle_notify.notified();
            if self.in_flight.load(std::sync::atomic::Ordering::SeqCst) == 0 {
                return Ok(());
            }
            notified.await;
        }
    }
}

/// Whether `entity_id` is already deployed at `timestamp_ms` or later: the bloom-guarded
/// skip in the batch deployer. (The old `LiveDeploymentRepository::is_entity_deployed`.)
pub async fn is_entity_deployed(
    pool: &PgPool,
    entity_id: &str,
    timestamp_ms: Timestamp,
) -> Result<bool, SyncError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM content.deployments
                       WHERE entity_id = $1 AND entity_timestamp >= to_timestamp($2 / 1000.0))",
    )
    .bind(entity_id)
    .bind(timestamp_ms as f64)
    .fetch_one(pool)
    .await
    .map_err(|e| SyncError::Storage(e.to_string()))?;
    Ok(exists)
}

/// The ids among `entities` already deployed at their timestamp or later: [`is_entity_deployed`]
/// for a whole batch in one statement.
pub async fn deployed_entity_ids(
    pool: &PgPool,
    entities: &[(&str, Timestamp)],
) -> Result<HashSet<String>, SyncError> {
    let ids: Vec<&str> = entities.iter().map(|(id, _)| *id).collect();
    let timestamps: Vec<f64> = entities.iter().map(|(_, ts)| *ts as f64).collect();
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT d.entity_id FROM unnest($1::text[], $2::float8[]) AS r(entity_id, ts)
         JOIN content.deployments d
           ON d.entity_id = r.entity_id AND d.entity_timestamp >= to_timestamp(r.ts / 1000.0)",
    )
    .bind(&ids)
    .bind(&timestamps)
    .fetch_all(pool)
    .await
    .map_err(|e| SyncError::Storage(e.to_string()))?;
    Ok(rows.into_iter().collect())
}

/// The columns every read of `content.failed_deployments` maps into a [`FailedDeployment`],
/// plus the raw deadline (`due_at`) that keys the due order: the epoch-millisecond projection
/// truncates, so a keyset cursor built from it could sit one millisecond before its row and
/// re-read it forever. The projection aliases `next_retry_at`, so an ORDER BY on that name binds
/// to the alias; the due queries qualify the column.
macro_rules! failed_columns {
    () => {
        "entity_id, entity_type, reason, auth_chain, error_description, \
         COALESCE(snapshot_hash, '') AS snapshot_hash, retry_count, \
         date_part('epoch', next_retry_at) * 1000 AS next_retry_at, \
         date_part('epoch', failure_time) * 1000 AS failure_timestamp, \
         next_retry_at AS due_at"
    };
}

/// A record the worker may act on: its auth chain is a non-empty JSON array. The CASE keeps
/// `json_array_length` off any non-array value, whatever order the planner evaluates the quals in.
macro_rules! has_auth_chain {
    () => {
        "CASE WHEN json_typeof(auth_chain) = 'array' THEN json_array_length(auth_chain) > 0 ELSE false END"
    };
}

#[derive(sqlx::FromRow)]
struct FailedRow {
    entity_id: String,
    entity_type: String,
    reason: String,
    auth_chain: Value,
    error_description: String,
    snapshot_hash: String,
    retry_count: i32,
    next_retry_at: f64,
    failure_timestamp: f64,
    due_at: chrono::NaiveDateTime,
}

impl From<FailedRow> for FailedDeployment {
    fn from(r: FailedRow) -> Self {
        FailedDeployment {
            entity_id: r.entity_id,
            entity_type: r.entity_type,
            reason: reason_from_label(&r.reason),
            auth_chain: serde_json::from_value(r.auth_chain).unwrap_or_default(),
            error_description: r.error_description,
            failure_timestamp: r.failure_timestamp as i64,
            snapshot_hash: if r.snapshot_hash.is_empty() {
                None
            } else {
                Some(r.snapshot_hash)
            },
            retry_count: r.retry_count.max(0) as u32,
            next_retry_at: r.next_retry_at as i64,
        }
    }
}

fn naive_utc(epoch_ms: i64) -> chrono::NaiveDateTime {
    chrono::DateTime::from_timestamp_millis(epoch_ms)
        .unwrap_or(chrono::DateTime::UNIX_EPOCH)
        .naive_utc()
}

/// Position in the due order `(next_retry_at, entity_id)`: the key of the last record a
/// [`LiveFailedDeploymentsStore::select_due`] page returned, handed back to fetch the page after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DueCursor {
    pub next_retry_at: chrono::NaiveDateTime,
    pub entity_id: String,
}

/// One page of due records and the cursor that continues after it (`None` for an empty page).
#[derive(Debug, Default)]
pub struct DueBatch {
    pub failures: Vec<FailedDeployment>,
    pub cursor: Option<DueCursor>,
}

/// A record the give-up pass dropped, as the worker logs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvictedRecord {
    pub entity_id: String,
    pub entity_type: String,
    pub retry_count: u32,
}

pub struct LiveFailedDeploymentsStore {
    pool: PgPool,
}

impl LiveFailedDeploymentsStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// GREATEST-monotonic on the retry columns: the sync path reports a failure with a fresh
    /// entity's zeroed retry state every time an entity reappears in a snapshot, and that must
    /// never rewind a backoff deadline or attempt count the retry worker already advanced.
    pub async fn report_failure(&self, failure: FailedDeployment) -> Result<(), SyncError> {
        let reason = reason_label(&failure.reason);
        sqlx::query(
            r#"INSERT INTO content.failed_deployments (entity_id, entity_type, failure_time, reason, auth_chain, error_description, snapshot_hash, retry_count, next_retry_at)
               VALUES ($1, $2, to_timestamp($9::double precision / 1000.0), $3, $4::json, $5, $6, $7, to_timestamp($8::double precision / 1000.0))
               ON CONFLICT (entity_id) DO UPDATE
               SET failure_time = EXCLUDED.failure_time, reason = $3, error_description = $5,
                   retry_count = GREATEST(failed_deployments.retry_count, EXCLUDED.retry_count),
                   next_retry_at = GREATEST(failed_deployments.next_retry_at, EXCLUDED.next_retry_at)"#,
        )
        .bind(&failure.entity_id)
        .bind(&failure.entity_type)
        .bind(reason)
        .bind(serde_json::to_value(&failure.auth_chain).unwrap_or_else(|_| Value::Array(Vec::new())))
        .bind(&failure.error_description)
        .bind(failure.snapshot_hash.as_deref().unwrap_or(""))
        .bind(failure.retry_count as i32)
        .bind(failure.next_retry_at as f64)
        .bind(failure.failure_timestamp as f64)
        .execute(&self.pool)
        .await
        .map_err(|e| SyncError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Every record with its payload, in no particular order. Diagnostics and tests only: the
    /// retry worker reads its due batches through [`Self::select_due`] instead of loading the
    /// whole table.
    pub async fn get_all_failed(&self) -> Result<Vec<FailedDeployment>, SyncError> {
        let rows: Vec<FailedRow> = sqlx::query_as(concat!(
            "SELECT ",
            failed_columns!(),
            " FROM content.failed_deployments"
        ))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SyncError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(FailedDeployment::from).collect())
    }

    /// The next `limit` records that are due at `now_ms` and still under the retry cap, in the
    /// due order `(next_retry_at, entity_id)`, continuing after `after` when the caller is paging
    /// through one cycle's due set. Records whose auth chain is an empty array are left out in
    /// SQL: they can never be retried, so they must not take a slot of the batch cycle after
    /// cycle. `failed_deployments_due_idx` serves the range and the order.
    pub async fn select_due(
        &self,
        now_ms: i64,
        max_retries: u32,
        after: Option<&DueCursor>,
        limit: usize,
    ) -> Result<DueBatch, SyncError> {
        const FIRST: &str = concat!(
            "SELECT ",
            failed_columns!(),
            " FROM content.failed_deployments
             WHERE next_retry_at <= $1 AND retry_count < $2 AND ",
            has_auth_chain!(),
            " ORDER BY failed_deployments.next_retry_at, failed_deployments.entity_id
             LIMIT $3"
        );
        const NEXT: &str = concat!(
            "SELECT ",
            failed_columns!(),
            " FROM content.failed_deployments
             WHERE next_retry_at <= $1 AND retry_count < $2 AND ",
            has_auth_chain!(),
            " AND (next_retry_at, entity_id) > ($4, $5)
             ORDER BY failed_deployments.next_retry_at, failed_deployments.entity_id
             LIMIT $3"
        );
        let query = match after {
            None => sqlx::query_as::<_, FailedRow>(FIRST),
            Some(_) => sqlx::query_as::<_, FailedRow>(NEXT),
        }
        .bind(naive_utc(now_ms))
        .bind(max_retries.min(i32::MAX as u32) as i32)
        .bind(limit.min(i64::MAX as usize) as i64);
        let query = match after {
            None => query,
            Some(cursor) => query.bind(cursor.next_retry_at).bind(&cursor.entity_id),
        };
        let rows: Vec<FailedRow> = query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| SyncError::Storage(e.to_string()))?;
        let cursor = rows.last().map(|r| DueCursor {
            next_retry_at: r.due_at,
            entity_id: r.entity_id.clone(),
        });
        Ok(DueBatch {
            failures: rows.into_iter().map(FailedDeployment::from).collect(),
            cursor,
        })
    }

    /// Drops up to `limit` records that are due at `now_ms` and at or over the retry cap, oldest
    /// deadline first, answering which ones went. A record at the cap is shed once its deadline
    /// passes, so the give-up walks the same due prefix of `failed_deployments_due_idx` as
    /// [`Self::select_due`] instead of scanning the whole table for the cap. Records without an
    /// auth chain are never evicted here (nor retried); the TTL prune takes them. The outer
    /// `retry_count >= $2` re-check keeps the [`Self::remove_exhausted`] guard against a row a
    /// successful deployment cleared and a fresh failure re-created under the cap.
    pub async fn remove_exhausted_due(
        &self,
        now_ms: i64,
        max_retries: u32,
        limit: usize,
    ) -> Result<Vec<EvictedRecord>, SyncError> {
        let rows: Vec<(String, String, i32)> = sqlx::query_as(concat!(
            "DELETE FROM content.failed_deployments
             WHERE entity_id IN (
                 SELECT entity_id FROM content.failed_deployments
                 WHERE next_retry_at <= $1 AND retry_count >= $2 AND ",
            has_auth_chain!(),
            "    ORDER BY next_retry_at, entity_id
                 LIMIT $3)
               AND retry_count >= $2
             RETURNING entity_id, entity_type, retry_count"
        ))
        .bind(naive_utc(now_ms))
        .bind(max_retries.min(i32::MAX as u32) as i32)
        .bind(limit.min(i64::MAX as usize) as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SyncError::Storage(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|(entity_id, entity_type, retry_count)| EvictedRecord {
                entity_id,
                entity_type,
                retry_count: retry_count.max(0) as u32,
            })
            .collect())
    }

    /// Records whose deadline is still ahead of `now_ms`: the backlog gauge of the cycle report.
    /// An index-only range over `failed_deployments_due_idx`; it counts at-cap rows still inside
    /// their last backoff too, since the cap lives in the heap.
    pub async fn count_waiting(&self, now_ms: i64) -> Result<u64, SyncError> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT count(*) FROM content.failed_deployments WHERE next_retry_at > $1",
        )
        .bind(naive_utc(now_ms))
        .fetch_one(&self.pool)
        .await
        .map_err(|e| SyncError::Storage(e.to_string()))?;
        Ok(count.max(0) as u64)
    }

    /// Drops the entries that have burned through the retry cap, answering which ones actually
    /// went. The `retry_count >= $2` guard is the point of the call: a give-up decision taken
    /// against an earlier read must not delete a row that a successful deployment cleared and a
    /// fresh failure has since re-created with a lower count. Chunked so one give-up pass over a
    /// large backlog never grows into a single unbounded statement; a chunk that fails leaves its
    /// entries in place for the next cycle.
    pub async fn remove_exhausted(
        &self,
        entity_ids: &[String],
        min_retry_count: u32,
    ) -> Result<Vec<String>, SyncError> {
        let mut removed = Vec::new();
        for chunk in entity_ids.chunks(EXHAUSTED_DELETE_BATCH_SIZE) {
            let rows: Vec<(String,)> = sqlx::query_as(
                "DELETE FROM content.failed_deployments WHERE entity_id = ANY($1::text[]) AND retry_count >= $2 RETURNING entity_id",
            )
            .bind(chunk)
            .bind(min_retry_count as i32)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| SyncError::Storage(e.to_string()))?;
            removed.extend(rows.into_iter().map(|r| r.0));
        }
        Ok(removed)
    }
}

/// Entity ids per give-up DELETE. `ANY($1)` binds the whole chunk as one parameter, so this is
/// not about the bind-parameter ceiling: it bounds each statement's payload, lock footprint and
/// duration.
const EXHAUSTED_DELETE_BATCH_SIZE: usize = 1000;

#[cfg(test)]
mod tests;
