//! Rendered and proxied avatars, kept in the ONE blob store: each image's
//! bytes are a blob under their CIDv1 and `social.profile_images` names it
//! per (entity, kind), with an optional TTL and a byte budget swept
//! oldest-first in the background. Images served straight from the profile
//! entity's own content files never enter it: those already live in the
//! blob store under the entity's own rows.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use bytes::Bytes;
use chrono::NaiveDateTime;
use sqlx::PgPool;

use crate::storage::Storage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageKind {
    Face,
    Body,
}

impl ImageKind {
    pub fn filename(self) -> &'static str {
        match self {
            ImageKind::Face => "face.png",
            ImageKind::Body => "body.png",
        }
    }

    /// The content file a deployed profile entity carries for this image
    /// (the snapshots the client uploads with the profile).
    pub fn content_key(self) -> &'static str {
        match self {
            ImageKind::Face => "face256.png",
            ImageKind::Body => "body.png",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ImageKind::Face => "face",
            ImageKind::Body => "body",
        }
    }
}

/// Where a cached image came from (`social.profile_images.source`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageSource {
    Render,
    Origin,
}

impl ImageSource {
    fn label(self) -> &'static str {
        match self {
            ImageSource::Render => "render",
            ImageSource::Origin => "origin",
        }
    }
}

/// Bytes written since the last sweep that trigger the next one. A sweep
/// reads the whole index, so it must not run per request; amortising it over
/// this much new data keeps it rare while bounding the overshoot to roughly
/// one interval above the budget.
const SWEEP_AFTER_BYTES: u64 = 64 * 1024 * 1024;

/// Evict down to this fraction of the budget, so a sweep reclaims a useful
/// amount instead of one image per subsequent write.
const LOW_WATER: f64 = 0.9;

#[derive(Clone)]
pub struct ImageCache {
    pool: PgPool,
    storage: Arc<Storage>,
    ttl: Option<Duration>,
    max_bytes: Option<u64>,
    written_since_sweep: Arc<AtomicU64>,
    sweeping: Arc<AtomicBool>,
}

fn now() -> NaiveDateTime {
    chrono::Utc::now().naive_utc()
}

impl ImageCache {
    pub fn new(
        pool: PgPool,
        storage: Arc<Storage>,
        ttl_seconds: u64,
        max_bytes: Option<u64>,
    ) -> Self {
        Self {
            pool,
            storage,
            ttl: (ttl_seconds > 0).then(|| Duration::from_secs(ttl_seconds)),
            max_bytes,
            written_since_sweep: Arc::new(AtomicU64::new(0)),
            sweeping: Arc::new(AtomicBool::new(false)),
        }
    }

    /// The image stored for (`entity`, `kind`) within the TTL, when the store
    /// still holds its blob. A row or blob that cannot be read is a miss: the
    /// image is derived again. One probe on `profile_images_pkey`.
    pub async fn get(&self, entity: &str, kind: ImageKind) -> Option<Bytes> {
        let cid: String = sqlx::query_scalar(
            "SELECT blob_cid FROM social.profile_images \
             WHERE entity_id = $1 AND kind = $2 AND ($3::timestamp IS NULL OR stored_at > $3)",
        )
        .bind(entity)
        .bind(kind.label())
        .bind(self.fresh_after())
        .fetch_optional(&self.pool)
        .await
        .inspect_err(|e| tracing::warn!(entity, error = %e, "profile image lookup failed"))
        .ok()??;
        self.storage.retrieve(&cid).await.ok()?
    }

    /// Whether (`entity`, `kind`) has a row within the TTL, without reading
    /// its blob (the warm workers' check). A lookup that fails says no.
    pub async fn holds(&self, entity: &str, kind: ImageKind) -> bool {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM social.profile_images \
             WHERE entity_id = $1 AND kind = $2 AND ($3::timestamp IS NULL OR stored_at > $3))",
        )
        .bind(entity)
        .bind(kind.label())
        .bind(self.fresh_after())
        .fetch_one(&self.pool)
        .await
        .inspect_err(|e| tracing::warn!(entity, error = %e, "profile image lookup failed"))
        .unwrap_or(false)
    }

    fn fresh_after(&self) -> Option<NaiveDateTime> {
        self.ttl
            .and_then(|ttl| chrono::Duration::from_std(ttl).ok())
            .map(|ttl| now() - ttl)
    }

    /// Stores the bytes as a blob, then the row that names them: a blob whose
    /// row never lands is an orphan `storage::gc` collects.
    pub async fn put(
        &self,
        entity: &str,
        kind: ImageKind,
        source: ImageSource,
        data: &Bytes,
    ) -> anyhow::Result<()> {
        let cid = crate::auth::hash::hash_bytes_v1(data);
        self.storage.store(&cid, data.clone()).await?;
        sqlx::query(
            "INSERT INTO social.profile_images (entity_id, kind, blob_cid, byte_size, source, stored_at) \
             VALUES ($1, $2, $3, $4, $5, $6) \
             ON CONFLICT (entity_id, kind) DO UPDATE SET blob_cid = EXCLUDED.blob_cid, \
                 byte_size = EXCLUDED.byte_size, source = EXCLUDED.source, stored_at = EXCLUDED.stored_at",
        )
        .bind(entity)
        .bind(kind.label())
        .bind(&cid)
        .bind(i32::try_from(data.len())?)
        .bind(source.label())
        .bind(now())
        .execute(&self.pool)
        .await?;
        self.note_written(data.len() as u64);
        Ok(())
    }

    /// The sweep is detached on purpose: a render already costs seconds, and
    /// making the caller wait on the eviction would add its cost to a request
    /// that has nothing to do with it.
    fn note_written(&self, bytes: u64) {
        let Some(max) = self.max_bytes else {
            return;
        };
        let prior = self.written_since_sweep.fetch_add(bytes, Ordering::Relaxed);
        if prior + bytes < SWEEP_AFTER_BYTES {
            return;
        }
        if self
            .sweeping
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        self.written_since_sweep.store(0, Ordering::Relaxed);
        let pool = self.pool.clone();
        let sweeping = self.sweeping.clone();
        tokio::spawn(async move {
            match sweep_to_budget(&pool, max).await {
                Ok(dropped) if dropped > 0 => {
                    tracing::info!(dropped, budget = max, "profile images evicted")
                }
                Ok(_) => {}
                Err(e) => tracing::warn!(error = %e, "profile image sweep failed"),
            }
            sweeping.store(false, Ordering::Release);
        });
    }
}

/// Drops the oldest rows until the newest ones fit `LOW_WATER * max`; their
/// blobs go with the next `storage::gc` run unless another row or domain
/// names them. One sequential pass over the index, every
/// [`SWEEP_AFTER_BYTES`] written: every image is re-derivable, so the
/// ordering (when it was stored, not when it was last served) is a heuristic
/// and the budget is the contract.
pub async fn sweep_to_budget(pool: &PgPool, max: u64) -> Result<u64, sqlx::Error> {
    let target = (max as f64 * LOW_WATER) as i64;
    let done = sqlx::query(
        "DELETE FROM social.profile_images p \
         USING (SELECT entity_id, kind, \
                       sum(byte_size) OVER (ORDER BY stored_at DESC, entity_id, kind) AS kept \
                FROM social.profile_images) r \
         WHERE p.entity_id = r.entity_id AND p.kind = r.kind AND r.kept > $1 \
           AND (SELECT sum(byte_size) FROM social.profile_images) > $2",
    )
    .bind(target)
    .bind(i64::try_from(max).unwrap_or(i64::MAX))
    .execute(pool)
    .await?;
    Ok(done.rows_affected())
}
