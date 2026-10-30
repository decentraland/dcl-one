//! The local write path behind `POST /entities` ([`super::create_entity`]):
//! an uploaded entity file plus its content files become rows in
//! `content.deployments`, `content.content_files`, `content.active_pointers`
//! and (for a local scene) `content.local_entities`, all in ONE database
//! call, a plpgsql function named after the hash of its own body and defined
//! in `schemas/00_content.sql` ([`super::land_publish::content_functions_sql`]).
//!
//! Before that call: the request TTLs, the operator denylist
//! ([`super::denylist::refusal`]: a listed entity id, content hash or
//! pointer is refused), the per-pointer rate limits, and the validation.
//! With the content validator attached ([`WriteDeployer::with_validator`],
//! as `serve` builds it) that is the old `ContentValidator` in full: the
//! auth chain (the shared EIP-1654 validator for contract wallets), the
//! per-type rules and size caps, content existence, and the access step per
//! type, whose LAND leg is `land_authz::can_deploy_batch` (the index once its
//! fold is published, the marketplace subgraph's owner leg before that,
//! undecidable when neither can answer). Without it (a deployer built for a
//! test) the partial checks below run: the chain, the LAND leg of a local
//! scene and content existence. Peer sync does not use this deployer: it
//! batches through [`super::sync_deployer::LiveSyncDeployer`], which shares
//! [`lock_deployment_pointers`].

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

use bytes::Bytes;
use futures::{StreamExt, stream};
use parking_lot::Mutex;
use serde_json::Value;
use sqlx::PgPool;
use tokio::sync::Semaphore;
use tracing::{info, warn};

use super::Denylist;
use super::entity::{Entity, EntityType};
use crate::auth::verify::verify_auth_chain_async;
use crate::auth::{AuthChain, AuthError, Eip1654Validator, hash_bytes, hash_bytes_v1};
use crate::land_authz::LandAuthzState;
use crate::storage::Storage;

#[derive(Debug, Clone)]
pub enum DeployFailure {
    Rejected(Vec<String>),
    /// This node could not decide, so the deposit is neither accepted nor at fault; answer 503 so
    /// the caller retries instead of being told their content does not exist.
    Unavailable(Vec<String>),
}

impl DeployFailure {
    pub fn errors(&self) -> &[String] {
        match self {
            DeployFailure::Rejected(errors) | DeployFailure::Unavailable(errors) => errors,
        }
    }
}

impl From<Vec<String>> for DeployFailure {
    fn from(errors: Vec<String>) -> Self {
        DeployFailure::Rejected(errors)
    }
}

struct HashedUpload {
    v0: String,
    v1: String,
    bytes: Bytes,
}

static UPLOAD_HASH_JOBS: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(4)));

async fn bounded_upload_hash<T: Send + 'static>(
    limit: Arc<Semaphore>,
    hash: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    let permit = limit
        .acquire_owned()
        .await
        .map_err(|_| "Upload hashing capacity is unavailable.".to_string())?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        hash()
    })
    .await
    .map_err(|e| format!("upload hashing failed: {e}"))
}

async fn hash_uploads(files: Vec<Bytes>) -> Result<Vec<HashedUpload>, String> {
    bounded_upload_hash(Arc::clone(&UPLOAD_HASH_JOBS), move || {
        files
            .into_iter()
            .map(|bytes| HashedUpload {
                v0: hash_bytes(&bytes),
                v1: hash_bytes_v1(&bytes),
                bytes,
            })
            .collect()
    })
    .await
}

fn deployment_files(
    entity: &Entity,
    entity_bytes: &Bytes,
    uploads: &[HashedUpload],
) -> HashMap<String, Bytes> {
    let by_hash: HashMap<&str, &Bytes> = uploads
        .iter()
        .flat_map(|upload| {
            [
                (upload.v0.as_str(), &upload.bytes),
                (upload.v1.as_str(), &upload.bytes),
            ]
        })
        .collect();
    let mut files = HashMap::from([(entity.id.clone(), entity_bytes.clone())]);
    for content in &entity.content {
        if let Some(bytes) = by_hash.get(content.hash.as_str()) {
            files.insert(content.hash.clone(), (*bytes).clone());
        }
    }
    files
}

async fn store_deployment_files(
    storage: &Storage,
    files: HashMap<String, Bytes>,
) -> Result<(), String> {
    let mut writes = stream::iter(files.into_iter().map(|(hash, bytes)| async move {
        storage
            .store(&hash, bytes)
            .await
            .map_err(|e| format!("failed to store deployment file {hash}: {e}"))
    }))
    .buffer_unordered(8);
    let mut first_error = None;
    while let Some(result) = writes.next().await {
        if let Err(error) = result {
            first_error.get_or_insert(error);
        }
    }
    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[cfg(test)]
fn happened_before_cmp(a_ts: i64, a_id: &str, b_ts: i64, b_id: &str) -> std::cmp::Ordering {
    a_ts.cmp(&b_ts)
        .then_with(|| a_id.to_lowercase().cmp(&b_id.to_lowercase()))
}

#[cfg(test)]
fn stored_is_newer_or_equal(stored_ts: i64, stored_id: &str, in_ts: i64, in_id: &str) -> bool {
    happened_before_cmp(in_ts, in_id, stored_ts, stored_id) == std::cmp::Ordering::Less
}

#[cfg(test)]
fn stored_is_overwritten(stored_ts: i64, stored_id: &str, in_ts: i64, in_id: &str) -> bool {
    happened_before_cmp(stored_ts, stored_id, in_ts, in_id) == std::cmp::Ordering::Less
}

fn metadata_unchanged(stored_wrapped: Option<&Value>, incoming: Option<&Value>) -> bool {
    let active = match stored_wrapped {
        Some(meta) => meta.get("v").cloned().unwrap_or(Value::Null),
        None => return false,
    };
    let new_meta = incoming.cloned().unwrap_or(Value::Null);
    active == new_meta
}

/// Measured on tokio's clock, which is the system's outside a paused test runtime.
struct TtlBucket {
    ttl: Duration,
    max_size: usize,
    entries: HashMap<String, tokio::time::Instant>,
}

impl TtlBucket {
    fn new(ttl: Duration, max_size: usize) -> Self {
        Self {
            ttl,
            max_size,
            entries: HashMap::new(),
        }
    }

    fn prune(&mut self, now: tokio::time::Instant) {
        let ttl = self.ttl;
        self.entries.retain(|_, t| now.duration_since(*t) < ttl);
    }

    fn is_limited(&mut self, pointers: &[String]) -> bool {
        let now = tokio::time::Instant::now();
        self.prune(now);
        let ttl_hit = pointers.iter().any(|p| self.entries.contains_key(p));
        let size_hit = self.entries.len() > self.max_size;
        ttl_hit || size_hit
    }

    fn record(&mut self, pointers: &[String]) {
        let now = tokio::time::Instant::now();
        for p in pointers {
            self.entries.insert(p.clone(), now);
        }
    }
}

struct DeployRateLimiter {
    buckets: Mutex<HashMap<&'static str, TtlBucket>>,
    unchanged_profile: Mutex<TtlBucket>,
}

impl DeployRateLimiter {
    fn with_reference_defaults() -> Self {
        let mut m = HashMap::new();
        m.insert("profile", TtlBucket::new(Duration::from_secs(3), 500));
        m.insert("scene", TtlBucket::new(Duration::from_secs(20), 1000));
        m.insert("wearable", TtlBucket::new(Duration::from_secs(20), 1000));
        m.insert("store", TtlBucket::new(Duration::from_secs(3), 300));
        m.insert("emote", TtlBucket::new(Duration::from_secs(20), 1000));
        m.insert("outfits", TtlBucket::new(Duration::from_secs(3), 2000));
        Self {
            buckets: Mutex::new(m),
            unchanged_profile: Mutex::new(TtlBucket::new(Duration::from_secs(300), usize::MAX)),
        }
    }

    fn is_rate_limited(&self, etype: &str, pointers: &[String]) -> bool {
        let mut b = self.buckets.lock();
        b.get_mut(etype)
            .map(|bk| bk.is_limited(pointers))
            .unwrap_or(false)
    }

    fn record(&self, etype: &str, pointers: &[String]) {
        if let Some(bk) = self.buckets.lock().get_mut(etype) {
            bk.record(pointers);
        }
    }

    fn is_unchanged_limited(&self, pointers: &[String]) -> bool {
        self.unchanged_profile.lock().is_limited(pointers)
    }

    #[cfg(test)]
    fn clear(&self) {
        for bucket in self.buckets.lock().values_mut() {
            bucket.entries.clear();
        }
        self.unchanged_profile.lock().entries.clear();
    }

    fn record_unchanged(&self, pointers: &[String]) {
        self.unchanged_profile.lock().record(pointers);
    }
}

pub struct WriteDeployer {
    pool: PgPool,
    storage: Arc<Storage>,
    /// The shared EIP-1654 validator (`crate::auth::cached_rpc_validator`);
    /// `None` without `ETH_RPC_URL`: a contract-wallet link is undecidable.
    eip1654: Option<Arc<dyn Eip1654Validator>>,
    /// The LAND rights source of a local scene deploy (`land_authz::can_deploy_batch`:
    /// the index once its fold is published, the marketplace subgraph's owner
    /// leg before that; the old `LandOperatorResolver` over the squid index).
    /// The partial checks' LAND leg (`None` runs none) and the unpublish
    /// route's parcel check.
    land_authz: Option<Arc<LandAuthzState>>,
    /// The full content validator (`super::validator`); when attached it
    /// replaces the partial checks in `validate`.
    validator: Option<Arc<super::validator::Sources>>,
    /// The shared denylist (`ContentState::denylist`): a deploy naming a
    /// listed id is refused.
    denylist: Option<Arc<Denylist>>,
    rate_limiter: DeployRateLimiter,
    /// The deploy function per provenance variant, once its existence was checked.
    deploy_fns: [tokio::sync::OnceCell<String>; 2],
    /// Upstream forwarding (`CONTENT_FORWARD_UPSTREAM_URL`): a local deploy
    /// queues its `content.forward_outbox` row in the deploy statement
    /// ([`forward_call`]) and wakes the worker.
    forwarder: Option<Arc<super::forward::Forwarder>>,
}

impl WriteDeployer {
    pub fn new(pool: PgPool, storage: Arc<Storage>, eip1654: Arc<dyn Eip1654Validator>) -> Self {
        Self::with_optional_eip1654(pool, storage, Some(eip1654))
    }

    /// `serve`'s constructor: `None` without `ETH_RPC_URL` (a contract-wallet
    /// link is then undecidable, 503).
    pub fn with_optional_eip1654(
        pool: PgPool,
        storage: Arc<Storage>,
        eip1654: Option<Arc<dyn Eip1654Validator>>,
    ) -> Self {
        Self {
            pool,
            storage,
            eip1654,
            land_authz: None,
            validator: None,
            denylist: None,
            rate_limiter: DeployRateLimiter::with_reference_defaults(),
            deploy_fns: [tokio::sync::OnceCell::new(), tokio::sync::OnceCell::new()],
            forwarder: None,
        }
    }

    pub fn with_forwarder(mut self, forwarder: Option<Arc<super::forward::Forwarder>>) -> Self {
        self.forwarder = forwarder;
        self
    }

    pub fn forwarder(&self) -> Option<&Arc<super::forward::Forwarder>> {
        self.forwarder.as_ref()
    }

    pub fn with_land_authz(mut self, land: Option<Arc<LandAuthzState>>) -> Self {
        self.land_authz = land;
        self
    }

    pub fn with_validator(mut self, sources: Arc<super::validator::Sources>) -> Self {
        self.validator = Some(sources);
        self
    }

    pub fn with_denylist(mut self, denylist: Arc<Denylist>) -> Self {
        self.denylist = Some(denylist);
        self
    }

    pub fn land_authz(&self) -> Option<&Arc<LandAuthzState>> {
        self.land_authz.as_ref()
    }

    pub fn eip1654(&self) -> Option<&dyn Eip1654Validator> {
        self.eip1654.as_deref()
    }

    /// Forgets every per-pointer TTL: a route test deploys to one pointer
    /// more than once inside the 3-20 s windows.
    #[cfg(test)]
    pub(crate) fn clear_rate_limits(&self) {
        self.rate_limiter.clear();
    }

    /// The old validator's LAND leg for a local scene: every pointer must be
    /// a parcel the signer may deploy to. A source that cannot decide
    /// (`land_authz::Unavailable`) is `Unavailable`, never a grant and never
    /// a refusal blamed on the depositor.
    async fn validate_land_rights(
        &self,
        entity: &Entity,
        signer: &str,
    ) -> Result<(), DeployFailure> {
        let Some(land) = self.land_authz.as_ref() else {
            return Ok(());
        };
        if entity.entity_type != EntityType::Scene {
            return Ok(());
        }
        let mut coords = Vec::with_capacity(entity.pointers.len());
        for pointer in &entity.pointers {
            let parsed = pointer.split_once(',').and_then(|(x, y)| {
                Some((x.trim().parse::<i32>().ok()?, y.trim().parse::<i32>().ok()?))
            });
            match parsed {
                Some(c) => coords.push(c),
                None => {
                    return Err(DeployFailure::Rejected(vec![format!(
                        "Scene pointers must be x,y parcel coordinates. Invalid pointer: ({pointer})"
                    )]));
                }
            }
        }
        let decisions = crate::land_authz::can_deploy_batch(land, &coords, signer)
            .await
            .map_err(|e| {
                DeployFailure::Unavailable(vec![format!("Error checking parcel access: {e}")])
            })?;
        let errors: Vec<String> = coords
            .iter()
            .zip(decisions.iter())
            .filter(|(_, d)| !d.verdict.allows())
            .map(|((x, y), _)| format!("The provided Eth Address does not have access to the following parcel: ({x},{y})"))
            .collect();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(DeployFailure::Rejected(errors))
        }
    }

    /// `Ok(None)` is provably absent, `Err` a storage fault: damage this node cannot read is
    /// never reported as a depositor's miss.
    async fn is_content_stored_already(
        &self,
        hashes: &[String],
    ) -> Result<HashMap<String, bool>, String> {
        let refs: Vec<&str> = hashes.iter().map(|h| h.as_str()).collect();
        self.storage
            .exist_multiple(&refs)
            .await
            .map_err(|e| e.to_string())
    }

    /// The ephemeral links are checked against the entity timestamp, as upstream does: a key
    /// that expired after signing still signed a valid deployment.
    async fn validate_signature(
        &self,
        entity_id: &str,
        auth_chain: &AuthChain,
        timestamp: i64,
    ) -> Result<(), AuthError> {
        verify_auth_chain_async(
            auth_chain,
            entity_id,
            Some(timestamp),
            self.eip1654.as_deref(),
        )
        .await
    }

    async fn validate(
        &self,
        entity: &Entity,
        files: &HashMap<String, Vec<u8>>,
        auth_chain: &AuthChain,
        context: &str,
    ) -> Result<(), DeployFailure> {
        if let Some(sources) = &self.validator {
            let ctx = super::validator::Context {
                sources,
                auth_chain,
            };
            return super::validator::validate(entity, files, &ctx)
                .await
                .map_err(super::validator::into_deploy_failure);
        }
        if let Err(e) = self
            .validate_signature(&entity.id, auth_chain, entity.timestamp)
            .await
        {
            let message = format!("The auth chain is invalid: {e}");
            return Err(match e {
                AuthError::Eip1654ValidationFailed(_) | AuthError::Eip1654NotImplemented => {
                    DeployFailure::Unavailable(vec![message])
                }
                _ => DeployFailure::Rejected(vec![message]),
            });
        }
        if context == "LOCAL"
            && let Some(signer) = auth_chain.first().map(|link| link.payload.to_lowercase())
        {
            self.validate_land_rights(entity, &signer).await?;
        }

        let not_uploaded: Vec<String> = entity
            .content
            .iter()
            .map(|c| c.hash.clone())
            .filter(|hash| !files.contains_key(hash))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        if !not_uploaded.is_empty() {
            let stored = self
                .is_content_stored_already(&not_uploaded)
                .await
                .map_err(|e| DeployFailure::Unavailable(vec![e]))?;
            let mut missing: Vec<String> = not_uploaded
                .into_iter()
                .filter(|hash| !stored.get(hash).copied().unwrap_or(false))
                .collect();
            if !missing.is_empty() {
                missing.sort();
                return Err(DeployFailure::Rejected(vec![format!(
                    "These content files are neither uploaded nor already stored: {}",
                    missing.join(", ")
                )]));
            }
        }
        Ok(())
    }

    async fn is_content_unchanged(&self, entity: &Entity) -> bool {
        let Some(pointer) = entity.pointers.first() else {
            return false;
        };
        let row: Option<Option<Value>> = sqlx::query_scalar(
            r#"
            SELECT d.entity_metadata
            FROM content.active_pointers ap
            JOIN content.deployments d ON d.entity_id = ap.entity_id
            WHERE ap.pointer = $1
            "#,
        )
        .bind(pointer.to_lowercase())
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten();
        let stored = row.flatten();
        metadata_unchanged(stored.as_ref(), entity.metadata.as_ref())
    }
}

pub(crate) const REQUEST_TTL_BACKWARDS_MS: i64 = 20 * 60 * 1000;
pub(crate) const REQUEST_TTL_FORWARDS_MS: i64 = 15 * 60 * 1000;

impl WriteDeployer {
    /// `context` is `"LOCAL"` for a deploy against this node's write API (a local scene then
    /// gets its `local_entities` provenance row); anything else is a mirrored write.
    pub async fn deploy_entity(
        &self,
        files: Vec<Bytes>,
        entity_id: &str,
        auth_chain: Value,
        context: &str,
    ) -> Result<i64, DeployFailure> {
        let started = Instant::now();
        let uploads = hash_uploads(files)
            .await
            .map_err(|e| DeployFailure::Unavailable(vec![e]))?;
        let hash_us = started.elapsed().as_micros() as u64;
        let entity_bytes = uploads
            .iter()
            .find(|upload| upload.v1 == entity_id || upload.v0 == entity_id)
            .map(|upload| upload.bytes.clone())
            .ok_or_else(|| {
                vec!["The entity file was not part of the uploaded files.".to_string()]
            })?;

        let mut entity: Entity = serde_json::from_slice(&entity_bytes)
            .map_err(|e| vec![format!("There was a problem parsing the entity: {e}")])?;

        if entity.id.is_empty() {
            entity.id = entity_id.to_string();
        } else if entity.id != entity_id {
            return Err(
                vec!["Entity id does not match the uploaded entity file.".to_string()].into(),
            );
        }

        if let Some(errors) = self
            .denylist
            .as_deref()
            .and_then(|d| super::denylist::refusal(&entity, d))
        {
            warn!(entity_id, ?errors, "deployment refused: denylisted");
            return Err(DeployFailure::Rejected(errors));
        }

        let now_ms = chrono::Utc::now().timestamp_millis();
        if now_ms - entity.timestamp > REQUEST_TTL_BACKWARDS_MS {
            return Err(vec![
                "The request is not recent enough, please submit it again with a new timestamp."
                    .to_string(),
            ]
            .into());
        }
        if now_ms - entity.timestamp < -REQUEST_TTL_FORWARDS_MS {
            return Err(vec!["The request timestamp is too far in the future.".to_string()].into());
        }

        let pointers_lc: Vec<String> = entity.pointers.iter().map(|p| p.to_lowercase()).collect();
        let rate_limited_msg = || {
            vec![format!(
                "Entity rate limited (entityId={} pointers={}).",
                entity.id,
                entity.pointers.join(",")
            )]
        };
        if self
            .rate_limiter
            .is_rate_limited(entity.entity_type.as_str(), &pointers_lc)
        {
            return Err(rate_limited_msg().into());
        }
        let content_unchanged =
            entity.entity_type == EntityType::Profile && self.is_content_unchanged(&entity).await;
        if content_unchanged && self.rate_limiter.is_unchanged_limited(&pointers_lc) {
            return Err(rate_limited_msg().into());
        }

        self.rate_limiter
            .record(entity.entity_type.as_str(), &pointers_lc);
        if content_unchanged {
            self.rate_limiter.record_unchanged(&pointers_lc);
        }

        let declared: HashSet<&str> = entity
            .content
            .iter()
            .map(|c| c.hash.as_str())
            .chain(std::iter::once(entity.id.as_str()))
            .collect();
        let mut vfiles: HashMap<String, Vec<u8>> = HashMap::new();
        for upload in &uploads {
            let key = if declared.contains(upload.v1.as_str()) {
                &upload.v1
            } else if declared.contains(upload.v0.as_str()) {
                &upload.v0
            } else {
                &upload.v1
            };

            if let Some(prev) = vfiles.insert(key.clone(), upload.bytes.to_vec())
                && prev != upload.bytes
            {
                return Err(vec![format!(
                    "two different uploaded files map to the same content hash {key}"
                )]
                .into());
            }
        }

        let auth_chain: AuthChain = serde_json::from_value(auth_chain)
            .map_err(|e| vec![format!("invalid auth chain: {e}")])?;

        let validation_started = Instant::now();
        match self.validate(&entity, &vfiles, &auth_chain, context).await {
            Ok(()) => {}
            Err(DeployFailure::Rejected(errors)) => {
                warn!(entity_id, ?errors, "deployment rejected by validation");
                return Err(DeployFailure::Rejected(errors));
            }
            Err(DeployFailure::Unavailable(errors)) => {
                warn!(
                    entity_id,
                    ?errors,
                    "deployment undecidable; this node is damaged"
                );
                return Err(DeployFailure::Unavailable(errors));
            }
        }

        let validation_us = validation_started.elapsed().as_micros() as u64;
        let persist_started = Instant::now();
        let creation_ts = self
            .persist(&entity, &entity_bytes, &uploads, &auth_chain, context)
            .await?;

        let persist_us = persist_started.elapsed().as_micros() as u64;
        let cleanup_started = Instant::now();
        drop(vfiles);
        drop(uploads);
        tracing::debug!(target: "catalyst_perf", phase="deploy", entity_id, hash_us, validation_us, persist_us, cleanup_us=cleanup_started.elapsed().as_micros() as u64, elapsed_us=started.elapsed().as_micros() as u64);
        Ok(creation_ts)
    }
}

const NEWER_ENTITY_MESSAGE: &str =
    "There is a newer entity pointed by one or more of the pointers you provided.";

#[derive(sqlx::FromRow)]
struct PersistOutcome {
    dep_id: Option<i32>,
    newer: bool,
    overwrote: i32,
    cleared: i32,
}

/// The whole write as one statement off the pointer lock: the newer-entity check and the
/// overwrite calculation read the locked snapshot, the deployment insert is gated on the former,
/// and every dependent write (provenance, content files, pointer upsert, pointer clears,
/// overwritten marks) hangs off the inserted row, so a conflict or a newer entity makes them all
/// no-ops.
fn persist_statement(local_provenance: bool) -> String {
    let provenance = if local_provenance {
        "
        provenance AS (
            INSERT INTO content.local_entities (entity_id, signer)
            SELECT $4, $11 FROM ins
            ON CONFLICT (entity_id) DO NOTHING
        ),"
    } else {
        ""
    };
    format!(
        r#"
        WITH newer AS (
            SELECT 1 AS present FROM content.deployments
            WHERE entity_type = $3
              AND entity_pointers && $7
              AND (entity_timestamp > to_timestamp($6 / 1000.0)
                   OR (entity_timestamp = to_timestamp($6 / 1000.0) AND lower(entity_id) > lower($4)))
            LIMIT 1
        ),
        overwrote AS (
            SELECT dep1.id, dep1.entity_pointers
            FROM content.deployments AS dep1
            LEFT JOIN content.deployments AS dep2 ON dep1.deleter_deployment = dep2.id
            WHERE dep1.entity_type = $3
              AND dep1.entity_pointers && $7
              AND (dep1.entity_timestamp < to_timestamp($6 / 1000.0)
                   OR (dep1.entity_timestamp = to_timestamp($6 / 1000.0) AND lower(dep1.entity_id) < lower($4)))
              AND (dep2.id IS NULL
                   OR dep2.entity_timestamp > to_timestamp($6 / 1000.0)
                   OR (dep2.entity_timestamp = to_timestamp($6 / 1000.0) AND lower(dep2.entity_id) > lower($4)))
        ),
        ins AS (
            INSERT INTO content.deployments
                (deployer_address, version, entity_type, entity_id, entity_metadata,
                 entity_timestamp, entity_pointers, local_timestamp, auth_chain)
            SELECT $1, $2, $3, $4, $5, to_timestamp($6 / 1000.0), $7, now(), $8
            WHERE NOT EXISTS (SELECT 1 FROM newer)
            ON CONFLICT (entity_id) DO NOTHING
            RETURNING id
        ),{provenance}
        files AS (
            INSERT INTO content.content_files (deployment, content_hash, key)
            SELECT ins.id, c.hash, c.key
            FROM ins, unnest($9::text[], $10::text[]) AS c(hash, key)
        ),
        pointed AS (
            INSERT INTO content.active_pointers (pointer, entity_id, entity_type)
            SELECT p.pointer, $4, $3 FROM ins, unnest($7::text[]) AS p(pointer)
            ON CONFLICT (pointer) DO UPDATE
                SET entity_id = EXCLUDED.entity_id, entity_type = EXCLUDED.entity_type
                WHERE NOT EXISTS (
                    SELECT 1 FROM content.deployments cur
                    WHERE cur.entity_id = active_pointers.entity_id
                      AND (cur.entity_timestamp > to_timestamp($6 / 1000.0)
                           OR (cur.entity_timestamp = to_timestamp($6 / 1000.0)
                               AND lower(cur.entity_id) > lower(EXCLUDED.entity_id)))
                )
        ),
        cleared AS (
            DELETE FROM content.active_pointers
            WHERE EXISTS (SELECT 1 FROM ins)
              AND pointer IN (SELECT unnest(entity_pointers) FROM overwrote)
              AND NOT (pointer = ANY($7))
            RETURNING pointer
        ),
        overwritten AS (
            UPDATE content.deployments SET deleter_deployment = ins.id
            FROM ins
            WHERE deployments.id IN (SELECT id FROM overwrote)
            RETURNING deployments.id
        )
        SELECT (SELECT id FROM ins) AS dep_id,
               EXISTS (SELECT 1 FROM newer) AS newer,
               (SELECT count(*) FROM overwritten)::int AS overwrote,
               (SELECT count(*) FROM cleared)::int AS cleared
        "#
    )
}

/// The same write as one function call. The pointer lock is the first statement of the body
/// and the CTE the second, so the CTE's snapshot follows the lock; a single client statement
/// takes its snapshot before it can block on the lock, which is why the transaction shape
/// needed BEGIN, lock, CTE, COMMIT. The name is a hash of the body: a binary always calls the
/// function that matches its own SQL, and two binaries mid-rollout coexist in one database.
/// Positional `$n` inside plpgsql are the function's parameters, so the CTE is embedded as is.
/// The function lives in schema `content` next to the tables it writes.
pub(crate) fn deploy_function_name(local_provenance: bool) -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(deploy_function_body(local_provenance));
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("deploy_entity_{}", &hex[..12])
}

/// The locks in byte order (`COLLATE "C"`), the order `lock_deployment_pointers`
/// sorts in Rust: under the database's own collation (`en_US.UTF-8` on the
/// lab and the deploy tree) `0,10` sorts before `0,-2` and `-1,0` after
/// `0,1`, and two writers taking overlapping locks in two orders can
/// deadlock.
fn deploy_function_body(local_provenance: bool) -> String {
    format!(
        "#variable_conflict use_column\nBEGIN\n\
         PERFORM pg_advisory_xact_lock(hashtext(k)) FROM (SELECT k FROM (SELECT DISTINCT unnest($7) AS k) d ORDER BY k COLLATE \"C\") locks;\n\
         RETURN QUERY{};\nEND",
        persist_statement(local_provenance)
    )
}

const DEPLOY_FUNCTION_ARGS: &str =
    "text, text, text, text, json, double precision, text[], json, text[], text[], text";

/// The `to_regprocedure` spelling the deployer checks before its first call.
pub(crate) fn deploy_function_signature(local_provenance: bool) -> String {
    format!(
        "content.{}({DEPLOY_FUNCTION_ARGS})",
        deploy_function_name(local_provenance)
    )
}

/// `schemas/00_content.sql` carries the `or_replace = false` form
/// (`land_publish::content_functions_sql`).
pub(crate) fn deploy_function_sql(local_provenance: bool, or_replace: bool) -> String {
    format!(
        "CREATE {}FUNCTION content.{}({DEPLOY_FUNCTION_ARGS}) \
         RETURNS TABLE (dep_id integer, newer boolean, overwrote integer, cleared integer) \
         LANGUAGE plpgsql AS $deploy$ {} $deploy$",
        if or_replace { "OR REPLACE " } else { "" },
        deploy_function_name(local_provenance),
        deploy_function_body(local_provenance)
    )
}

pub(crate) fn deploy_function_call(name: &str) -> String {
    format!(
        "SELECT dep_id, newer, overwrote, cleared FROM content.{name}($1, $2, $3, $4, $5::json, $6, $7, $8::json, $9, $10, $11)"
    )
}

/// [`deploy_function_call`] with the forwarding outbox row (`$12` the target)
/// in the same statement: the row exists exactly when the call inserted the
/// deployment, commits or rolls back with it, and the function, its locks and
/// its hash-derived name are unchanged.
pub(crate) fn forward_call(name: &str) -> String {
    format!(
        "WITH d AS MATERIALIZED ({}), \
              queued AS (INSERT INTO content.forward_outbox (entity_id, target) \
                         SELECT $4, $12 FROM d WHERE d.dep_id IS NOT NULL AND NOT d.newer \
                         ON CONFLICT (entity_id) DO NOTHING) \
         SELECT dep_id, newer, overwrote, cleared FROM d",
        deploy_function_call(name)
    )
}

impl WriteDeployer {
    async fn persist(
        &self,
        entity: &Entity,
        entity_bytes: &Bytes,
        uploads: &[HashedUpload],
        auth_chain: &AuthChain,
        context: &str,
    ) -> Result<i64, DeployFailure> {
        let unavailable = |e: String| DeployFailure::Unavailable(vec![e]);
        let started = Instant::now();
        store_deployment_files(
            &self.storage,
            deployment_files(entity, entity_bytes, uploads),
        )
        .await
        .map_err(unavailable)?;

        let storage_us = started.elapsed().as_micros() as u64;
        let db_started = Instant::now();
        let deployer_address = auth_chain
            .first()
            .map(|l| l.payload.clone())
            .unwrap_or_default();
        let metadata = match &entity.metadata {
            Some(m) if !m.is_null() => serde_json::json!({ "v": m }),
            _ => Value::Null,
        };
        let pointers: Vec<String> = {
            let mut seen = std::collections::HashSet::new();
            entity
                .pointers
                .iter()
                .map(|p| p.to_lowercase())
                .filter(|p| seen.insert(p.clone()))
                .collect()
        };
        let auth_chain_json =
            serde_json::to_value(auth_chain).map_err(|e| unavailable(e.to_string()))?;
        let hashes: Vec<String> = entity.content.iter().map(|c| c.hash.clone()).collect();
        let keys: Vec<String> = entity.content.iter().map(|c| c.file.clone()).collect();

        let local_provenance = context == "LOCAL" && entity.entity_type == EntityType::Scene;
        // The schema installs the function (the runtime role cannot); the
        // first call checks the name this binary computes is there.
        let deploy_fn = self.deploy_fns[local_provenance as usize]
            .get_or_try_init(|| async {
                super::land_publish::require_function(
                    &self.pool,
                    &deploy_function_signature(local_provenance),
                )
                .await?;
                Ok::<_, String>(deploy_function_name(local_provenance))
            })
            .await
            .map_err(unavailable)?;

        // Only a deploy against this node's own write API is forwarded.
        let forward = self.forwarder.as_ref().filter(|_| context == "LOCAL");
        let call = match forward {
            Some(_) => forward_call(deploy_fn),
            None => deploy_function_call(deploy_fn),
        };
        let mut query = sqlx::query_as::<_, PersistOutcome>(sqlx::AssertSqlSafe(call))
            .bind(&deployer_address)
            .bind(&entity.version)
            .bind(entity.entity_type.as_str())
            .bind(&entity.id)
            .bind(&metadata)
            .bind(entity.timestamp as f64)
            .bind(&pointers)
            .bind(&auth_chain_json)
            .bind(&hashes)
            .bind(&keys)
            .bind(deployer_address.to_lowercase());
        if let Some(forwarder) = forward {
            query = query.bind(forwarder.target());
        }
        let outcome = query
            .fetch_one(&self.pool)
            .await
            .map_err(|e| unavailable(format!("deployment write failed: {e}")))?;

        let now_ms = chrono::Utc::now().timestamp_millis();
        tracing::debug!(target: "catalyst_perf", phase="persist", entity_id=%entity.id, storage_us, db_us=db_started.elapsed().as_micros() as u64);

        if outcome.newer {
            return Err(DeployFailure::Rejected(vec![
                NEWER_ENTITY_MESSAGE.to_string(),
            ]));
        }
        let Some(dep_id) = outcome.dep_id else {
            info!(entity_id = %entity.id, "entity already deployed; treating as success");
            return Ok(now_ms);
        };
        info!(
            entity_id = %entity.id,
            dep_id,
            overwrote = outcome.overwrote,
            cleared = outcome.cleared,
            forwarded = forward.is_some(),
            "deployment committed"
        );
        if let Some(forwarder) = forward {
            forwarder.kick();
        }
        Ok(now_ms)
    }
}

/// Transaction-scoped advisory locks on every pointer a write touches, taken in one sorted
/// order so two writers over overlapping pointer sets cannot deadlock. The sync batch flush
/// takes these before its inserts; the deploy function takes the same locks in its body.
pub async fn lock_deployment_pointers(
    conn: &mut sqlx::PgConnection,
    pointers: &[String],
) -> Result<(), sqlx::Error> {
    let mut keys = pointers.to_vec();
    keys.sort_unstable();
    keys.dedup();
    sqlx::query(
        r#"SELECT pg_advisory_xact_lock(hashtext(pointer))
           FROM (SELECT pointer FROM unnest($1::text[]) WITH ORDINALITY AS p(pointer, ordinal)
                 ORDER BY ordinal) ordered"#,
    )
    .bind(&keys)
    .execute(conn)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests;
