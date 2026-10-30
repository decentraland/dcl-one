//! The three key/value namespaces over the ONE pool, keyed by the stable
//! [`SceneNamespace`] (the world, or the Genesis base parcel) the room
//! resolver mints from the active deployment (never a content hash: a
//! redeploy keeps its rows). Every
//! statement carries `realm = $1 AND scene_namespace = $2` (and the player
//! for `player_storage`), so a scene can reach no other scene's rows and a
//! player no other player's.
//!
//! Quotas: a write takes `pg_advisory_xact_lock` on the scope's key (a
//! world's quota spans all its scenes, a shared realm's is per scene), reads
//! the size picture and inserts only when the limits hold, in one statement
//! (`quota_upsert_sql`); the aggregate is the `*_size_idx` index-only range.
//! The current time is a bind.
//!
//! Cache: a per-process memo of single values (source: the row; freshness:
//! `STORAGE_CACHE_TTL_SECONDS`; invalidation: every write and delete path in
//! this process, per key or per scope prefix; rebuild: none, a miss re-reads).
//! Another process's write invalidates nothing here, so the memo is off by
//! default on every node. Opt in with `STORAGE_CACHE_ENABLED=true` only for a
//! single storage-serving process. The TTL bounds stale reads if another
//! writer exists; primary and replica roles do not make caching safe.

use crate::shared::now_utc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use sqlx::{PgExecutor, PgPool, Row};

use crate::comms::world_identity::SceneNamespace;
use crate::shared::identity::Address;

use super::errors::ApiError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NamespaceLimits {
    pub max_value_size_bytes: i64,
    pub max_total_size_bytes: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageCacheConfig {
    pub enabled: bool,
    pub ttl_seconds: u64,
    pub max_entries: u64,
    pub max_value_bytes: usize,
}

#[derive(Debug)]
pub struct StorageEntry {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Copy)]
pub struct SizeInfo {
    pub existing_value_size: i64,
    pub total_size: i64,
}

pub fn value_size_bytes(serialized: &str) -> i64 {
    serialized.len() as i64
}

fn prefix_pattern(prefix: Option<&str>) -> Option<String> {
    prefix.filter(|p| !p.is_empty()).map(|p| {
        let mut pat = String::with_capacity(p.len() + 1);
        for c in p.chars() {
            if matches!(c, '\\' | '%' | '_') {
                pat.push('\\');
            }
            pat.push(c);
        }
        pat.push('%');
        pat
    })
}

/// The advisory-lock key of a write: a world's quota spans its scenes
/// (`<ns>:<realm>`), a shared realm's is per scene (`<ns>:<realm>:<base>`),
/// a player's is per player under either.
pub(crate) fn quota_lock_key(namespace: &str, ns: &SceneNamespace, player: Option<&str>) -> String {
    let mut key = if ns.is_shared_realm() {
        format!("{namespace}:{}:{}", ns.realm(), ns.scene_namespace())
    } else {
        format!("{namespace}:{}", ns.realm())
    };
    if let Some(player) = player {
        key.push(':');
        key.push_str(player);
    }
    key
}

/// Quota check and upsert as one statement: `info` is the scope's size
/// picture (taken after the advisory lock, so it sees every earlier writer),
/// the insert runs only when the limits hold, and the row reports which so
/// the caller can raise the exact `check_limits` error otherwise.
fn quota_upsert_sql(size_info: String, guarded_insert: &str) -> String {
    format!(
        "WITH info AS ({size_info}), ins AS ({guarded_insert} RETURNING 1)
         SELECT info.existing, info.total, EXISTS (SELECT 1 FROM ins) AS inserted FROM info"
    )
}

fn quota_outcome(
    row: &sqlx::postgres::PgRow,
    size: i64,
    limits: NamespaceLimits,
) -> Result<(), ApiError> {
    if row.get::<bool, _>("inserted") {
        return Ok(());
    }
    check_limits(
        size,
        SizeInfo {
            existing_value_size: row.get("existing"),
            total_size: row.get("total"),
        },
        limits,
    )?;
    Err(ApiError::internal(
        "storage quota check and upsert disagreed",
    ))
}

/// `$1` realm, `$2` scene namespace, `$3` the key being written (its current
/// size is credited back). A world's total spans its non-retired scopes; a
/// shared realm's is the one scene namespace. Legacy scopes without an
/// identity still count, except the parked `world` bucket once that world
/// has an active identity. Neither exclusion deletes or reassigns bytes.
fn size_info_sql(table: &str, ns: &SceneNamespace, extra_where: &str) -> String {
    let mut sql = format!(
        "SELECT COALESCE(MAX(value_size) FILTER (WHERE scene_namespace = $2 AND key = $3), 0)::bigint AS existing,
                COALESCE(SUM(value_size), 0)::bigint AS total
         FROM worlds.{table} WHERE realm = $1{extra_where}"
    );
    if ns.is_shared_realm() {
        sql.push_str(" AND scene_namespace = $2");
    } else {
        sql.push_str(&format!(
            " AND NOT EXISTS (SELECT 1 FROM explore.world_scene_place i \
             WHERE i.world_name = $1 AND NOT i.active AND i.identity::text = worlds.{table}.scene_namespace) \
             AND (scene_namespace <> 'world' OR NOT EXISTS ( \
               SELECT 1 FROM explore.world_scene_place i WHERE i.world_name = $1 AND i.active))"
        ));
    }
    sql
}

pub fn check_limits(
    new_value_size: i64,
    info: SizeInfo,
    limits: NamespaceLimits,
) -> Result<(), ApiError> {
    if new_value_size > limits.max_value_size_bytes {
        return Err(ApiError::bad_request(format!(
            "Value size ({} bytes) exceeds the maximum allowed size ({} bytes)",
            new_value_size, limits.max_value_size_bytes
        )));
    }
    let projected = info.total_size - info.existing_value_size + new_value_size;
    if projected > limits.max_total_size_bytes {
        return Err(ApiError::bad_request(format!(
            "Total storage size would exceed the maximum allowed ({} bytes). Current usage: {} bytes. Delete existing data to free up space",
            limits.max_total_size_bytes, info.total_size
        )));
    }
    Ok(())
}

fn world_value_cache_key(ns: &SceneNamespace, key: &str) -> String {
    format!("w:{}:{}:{key}", ns.realm(), ns.scene_namespace())
}

fn world_scene_prefix(ns: &SceneNamespace) -> String {
    format!("w:{}:{}:", ns.realm(), ns.scene_namespace())
}

fn player_value_cache_key(ns: &SceneNamespace, player: &str, key: &str) -> String {
    format!("p:{}:{}:{player}:{key}", ns.realm(), ns.scene_namespace())
}

fn player_scope_prefix(ns: &SceneNamespace, player: Option<&str>) -> String {
    match player {
        Some(p) => format!("p:{}:{}:{p}:", ns.realm(), ns.scene_namespace()),
        None => format!("p:{}:{}:", ns.realm(), ns.scene_namespace()),
    }
}

pub struct StorageCache {
    cfg: StorageCacheConfig,
    map: DashMap<String, (Instant, Arc<str>)>,
}

impl StorageCache {
    pub fn new(cfg: StorageCacheConfig) -> Self {
        Self {
            cfg,
            map: DashMap::new(),
        }
    }

    fn ttl(&self) -> Duration {
        Duration::from_secs(self.cfg.ttl_seconds)
    }

    pub fn get(&self, key: &str) -> Option<Arc<str>> {
        if !self.cfg.enabled {
            return None;
        }
        let entry = self.map.get(key)?;
        (entry.0.elapsed() < self.ttl()).then(|| entry.1.clone())
    }

    /// Bounded: at capacity the expired entries go first, then the whole
    /// memo (a miss costs one PK read; a stale hit would cost correctness).
    pub fn insert(&self, key: String, value: Arc<str>) {
        if !self.cfg.enabled || value.len() > self.cfg.max_value_bytes {
            return;
        }
        if self.map.len() as u64 >= self.cfg.max_entries {
            let ttl = self.ttl();
            self.map.retain(|_, (at, _)| at.elapsed() < ttl);
            if self.map.len() as u64 >= self.cfg.max_entries {
                self.map.clear();
            }
        }
        self.map.insert(key, (Instant::now(), value));
    }

    pub fn invalidate(&self, key: &str) {
        if self.cfg.enabled {
            self.map.remove(key);
        }
    }

    pub fn invalidate_prefix(&self, prefix: &str) {
        if self.cfg.enabled {
            self.map.retain(|k, _| !k.starts_with(prefix));
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.map.len()
    }
}

pub struct Store {
    pub pool: PgPool,
    cache: StorageCache,
}

impl Store {
    pub fn new(pool: PgPool, cache_cfg: StorageCacheConfig) -> Self {
        Self {
            pool,
            cache: StorageCache::new(cache_cfg),
        }
    }

    pub async fn world_get(
        &self,
        ns: &SceneNamespace,
        key: &str,
    ) -> Result<Option<Arc<str>>, ApiError> {
        let cache_key = world_value_cache_key(ns, key);
        if let Some(hit) = self.cache.get(&cache_key) {
            return Ok(Some(hit));
        }
        let row = sqlx::query("SELECT value::text AS value FROM worlds.world_storage WHERE realm = $1 AND scene_namespace = $2 AND key = $3")
            .bind(ns.realm())
            .bind(ns.scene_namespace())
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        let Some(row) = row else { return Ok(None) };
        let value: Arc<str> = Arc::from(row.get::<String, _>("value"));
        self.cache.insert(cache_key, value.clone());
        Ok(Some(value))
    }

    pub async fn world_upsert_with_quota(
        &self,
        ns: &SceneNamespace,
        key: &str,
        serialized: &str,
        limits: NamespaceLimits,
    ) -> Result<(), ApiError> {
        let size = value_size_bytes(serialized);
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(quota_lock_key("world-storage", ns, None))
            .execute(&mut *tx)
            .await?;
        let sql = quota_upsert_sql(
            size_info_sql("world_storage", ns, ""),
            "INSERT INTO worlds.world_storage (realm, scene_namespace, key, value, value_size, created_at, updated_at)
             SELECT $1::text, $2::text, $3::text, $4::jsonb, ($5::bigint)::int, $8::timestamp, $8::timestamp FROM info
             WHERE $5::bigint <= $6::bigint AND info.total - info.existing + $5::bigint <= $7::bigint
             ON CONFLICT (realm, scene_namespace, key)
             DO UPDATE SET value = $4::jsonb, value_size = ($5::bigint)::int, updated_at = $8::timestamp",
        );
        let row = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(ns.realm())
            .bind(ns.scene_namespace())
            .bind(key)
            .bind(serialized)
            .bind(size)
            .bind(limits.max_value_size_bytes)
            .bind(limits.max_total_size_bytes)
            .bind(now_utc())
            .fetch_one(&mut *tx)
            .await?;
        quota_outcome(&row, size, limits)?;
        tx.commit().await?;
        self.cache.invalidate(&world_value_cache_key(ns, key));
        Ok(())
    }

    pub async fn world_delete(&self, ns: &SceneNamespace, key: &str) -> Result<(), ApiError> {
        sqlx::query("DELETE FROM worlds.world_storage WHERE realm = $1 AND scene_namespace = $2 AND key = $3")
            .bind(ns.realm())
            .bind(ns.scene_namespace())
            .bind(key)
            .execute(&self.pool)
            .await?;
        self.cache.invalidate(&world_value_cache_key(ns, key));
        Ok(())
    }

    pub async fn world_delete_all(&self, ns: &SceneNamespace) -> Result<(), ApiError> {
        sqlx::query("DELETE FROM worlds.world_storage WHERE realm = $1 AND scene_namespace = $2")
            .bind(ns.realm())
            .bind(ns.scene_namespace())
            .execute(&self.pool)
            .await?;
        self.cache.invalidate_prefix(&world_scene_prefix(ns));
        Ok(())
    }

    /// The listing and its count in one statement; an empty page past the
    /// start falls back to the count so `total` stays exact.
    pub async fn world_list_page(
        &self,
        ns: &SceneNamespace,
        limit: i64,
        offset: i64,
        prefix: Option<&str>,
    ) -> Result<(Vec<StorageEntry>, i64), ApiError> {
        let pat = prefix_pattern(prefix);
        let rows = sqlx::query(
            "SELECT key, value::text AS value, count(*) OVER ()::bigint AS total FROM worlds.world_storage
             WHERE realm = $1 AND scene_namespace = $2
               AND ($3::text IS NULL OR key LIKE $3)
             ORDER BY key ASC LIMIT $4 OFFSET $5",
        )
        .bind(ns.realm())
        .bind(ns.scene_namespace())
        .bind(&pat)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let total = match rows.first() {
            Some(r) => r.get("total"),
            None if offset > 0 => self.world_count(ns, prefix).await?,
            None => 0,
        };
        let entries = rows
            .into_iter()
            .map(|r| StorageEntry {
                key: r.get("key"),
                value: r.get("value"),
            })
            .collect();
        Ok((entries, total))
    }

    pub async fn world_count(
        &self,
        ns: &SceneNamespace,
        prefix: Option<&str>,
    ) -> Result<i64, ApiError> {
        let row = sqlx::query(
            "SELECT COUNT(*)::bigint AS count FROM worlds.world_storage
             WHERE realm = $1 AND scene_namespace = $2 AND ($3::text IS NULL OR key LIKE $3)",
        )
        .bind(ns.realm())
        .bind(ns.scene_namespace())
        .bind(prefix_pattern(prefix))
        .fetch_one(&self.pool)
        .await?;
        Ok(row.get("count"))
    }

    pub async fn world_size_info(&self, ns: &SceneNamespace) -> Result<SizeInfo, ApiError> {
        size_info_in(&self.pool, size_info_sql("world_storage", ns, ""), ns, None).await
    }

    pub async fn player_get(
        &self,
        ns: &SceneNamespace,
        player: &Address,
        key: &str,
    ) -> Result<Option<Arc<str>>, ApiError> {
        let cache_key = player_value_cache_key(ns, player.as_str(), key);
        if let Some(hit) = self.cache.get(&cache_key) {
            return Ok(Some(hit));
        }
        let row = sqlx::query(
            "SELECT value::text AS value FROM worlds.player_storage
             WHERE realm = $1 AND scene_namespace = $2 AND player_address = $3 AND key = $4",
        )
        .bind(ns.realm())
        .bind(ns.scene_namespace())
        .bind(player)
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else { return Ok(None) };
        let value: Arc<str> = Arc::from(row.get::<String, _>("value"));
        self.cache.insert(cache_key, value.clone());
        Ok(Some(value))
    }

    pub async fn player_upsert_with_quota(
        &self,
        ns: &SceneNamespace,
        player: &Address,
        key: &str,
        serialized: &str,
        limits: NamespaceLimits,
    ) -> Result<(), ApiError> {
        let size = value_size_bytes(serialized);
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(quota_lock_key("player-storage", ns, Some(player.as_str())))
            .execute(&mut *tx)
            .await?;
        let sql = quota_upsert_sql(
            size_info_sql("player_storage", ns, " AND player_address = $4"),
            "INSERT INTO worlds.player_storage (realm, scene_namespace, player_address, key, value, value_size, created_at, updated_at)
             SELECT $1::text, $2::text, $4::text, $3::text, $5::jsonb, ($6::bigint)::int, $9::timestamp, $9::timestamp FROM info
             WHERE $6::bigint <= $7::bigint AND info.total - info.existing + $6::bigint <= $8::bigint
             ON CONFLICT (realm, scene_namespace, player_address, key)
             DO UPDATE SET value = $5::jsonb, value_size = ($6::bigint)::int, updated_at = $9::timestamp",
        );
        let row = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(ns.realm())
            .bind(ns.scene_namespace())
            .bind(key)
            .bind(player)
            .bind(serialized)
            .bind(size)
            .bind(limits.max_value_size_bytes)
            .bind(limits.max_total_size_bytes)
            .bind(now_utc())
            .fetch_one(&mut *tx)
            .await?;
        quota_outcome(&row, size, limits)?;
        tx.commit().await?;
        self.cache
            .invalidate(&player_value_cache_key(ns, player.as_str(), key));
        Ok(())
    }

    pub async fn player_delete(
        &self,
        ns: &SceneNamespace,
        player: &Address,
        key: &str,
    ) -> Result<(), ApiError> {
        sqlx::query(
            "DELETE FROM worlds.player_storage
             WHERE realm = $1 AND scene_namespace = $2 AND player_address = $3 AND key = $4",
        )
        .bind(ns.realm())
        .bind(ns.scene_namespace())
        .bind(player)
        .bind(key)
        .execute(&self.pool)
        .await?;
        self.cache
            .invalidate(&player_value_cache_key(ns, player.as_str(), key));
        Ok(())
    }

    pub async fn player_delete_all_for_player(
        &self,
        ns: &SceneNamespace,
        player: &Address,
    ) -> Result<(), ApiError> {
        sqlx::query("DELETE FROM worlds.player_storage WHERE realm = $1 AND scene_namespace = $2 AND player_address = $3")
            .bind(ns.realm())
            .bind(ns.scene_namespace())
            .bind(player)
            .execute(&self.pool)
            .await?;
        self.cache
            .invalidate_prefix(&player_scope_prefix(ns, Some(player.as_str())));
        Ok(())
    }

    pub async fn player_delete_all(&self, ns: &SceneNamespace) -> Result<(), ApiError> {
        sqlx::query("DELETE FROM worlds.player_storage WHERE realm = $1 AND scene_namespace = $2")
            .bind(ns.realm())
            .bind(ns.scene_namespace())
            .execute(&self.pool)
            .await?;
        self.cache.invalidate_prefix(&player_scope_prefix(ns, None));
        Ok(())
    }

    pub async fn player_list_page(
        &self,
        ns: &SceneNamespace,
        player: &Address,
        limit: i64,
        offset: i64,
        prefix: Option<&str>,
    ) -> Result<(Vec<StorageEntry>, i64), ApiError> {
        let pat = prefix_pattern(prefix);
        let rows = sqlx::query(
            "SELECT key, value::text AS value, count(*) OVER ()::bigint AS total FROM worlds.player_storage
             WHERE realm = $1 AND scene_namespace = $2 AND player_address = $3
               AND ($4::text IS NULL OR key LIKE $4)
             ORDER BY key ASC LIMIT $5 OFFSET $6",
        )
        .bind(ns.realm())
        .bind(ns.scene_namespace())
        .bind(player)
        .bind(&pat)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let total = match rows.first() {
            Some(r) => r.get("total"),
            None if offset > 0 => self.player_count(ns, player, prefix).await?,
            None => 0,
        };
        let entries = rows
            .into_iter()
            .map(|r| StorageEntry {
                key: r.get("key"),
                value: r.get("value"),
            })
            .collect();
        Ok((entries, total))
    }

    pub async fn player_count(
        &self,
        ns: &SceneNamespace,
        player: &Address,
        prefix: Option<&str>,
    ) -> Result<i64, ApiError> {
        let row = sqlx::query(
            "SELECT COUNT(*)::bigint AS count FROM worlds.player_storage
             WHERE realm = $1 AND scene_namespace = $2 AND player_address = $3
               AND ($4::text IS NULL OR key LIKE $4)",
        )
        .bind(ns.realm())
        .bind(ns.scene_namespace())
        .bind(player)
        .bind(prefix_pattern(prefix))
        .fetch_one(&self.pool)
        .await?;
        Ok(row.get("count"))
    }

    pub async fn player_list_players_page(
        &self,
        ns: &SceneNamespace,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<String>, i64), ApiError> {
        let rows = sqlx::query(
            "SELECT player_address, count(*) OVER ()::bigint AS total
             FROM (SELECT DISTINCT player_address FROM worlds.player_storage
                   WHERE realm = $1 AND scene_namespace = $2) d
             ORDER BY player_address ASC LIMIT $3 OFFSET $4",
        )
        .bind(ns.realm())
        .bind(ns.scene_namespace())
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let total = match rows.first() {
            Some(r) => r.get("total"),
            None if offset > 0 => self.player_count_players(ns).await?,
            None => 0,
        };
        let players = rows.into_iter().map(|r| r.get("player_address")).collect();
        Ok((players, total))
    }

    pub async fn player_count_players(&self, ns: &SceneNamespace) -> Result<i64, ApiError> {
        let row = sqlx::query(
            "SELECT COUNT(DISTINCT player_address)::bigint AS count FROM worlds.player_storage
             WHERE realm = $1 AND scene_namespace = $2",
        )
        .bind(ns.realm())
        .bind(ns.scene_namespace())
        .fetch_one(&self.pool)
        .await?;
        Ok(row.get("count"))
    }

    pub async fn player_size_info(
        &self,
        ns: &SceneNamespace,
        player: &Address,
    ) -> Result<SizeInfo, ApiError> {
        size_info_in(
            &self.pool,
            size_info_sql("player_storage", ns, " AND player_address = $4"),
            ns,
            Some(player),
        )
        .await
    }

    pub async fn env_get_enc(
        &self,
        ns: &SceneNamespace,
        key: &str,
    ) -> Result<Option<Vec<u8>>, ApiError> {
        let row = sqlx::query("SELECT value_enc FROM worlds.env_variables WHERE realm = $1 AND scene_namespace = $2 AND key = $3")
            .bind(ns.realm())
            .bind(ns.scene_namespace())
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| r.get::<Vec<u8>, _>("value_enc")))
    }

    pub async fn env_upsert_with_quota(
        &self,
        ns: &SceneNamespace,
        key: &str,
        value_enc: &[u8],
        size: i64,
        limits: NamespaceLimits,
    ) -> Result<(), ApiError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(quota_lock_key("env-storage", ns, None))
            .execute(&mut *tx)
            .await?;
        let sql = quota_upsert_sql(
            size_info_sql("env_variables", ns, ""),
            "INSERT INTO worlds.env_variables (realm, scene_namespace, key, value_enc, value_size, created_at, updated_at)
             SELECT $1::text, $2::text, $3::text, $4::bytea, ($5::bigint)::int, $8::timestamp, $8::timestamp FROM info
             WHERE $5::bigint <= $6::bigint AND info.total - info.existing + $5::bigint <= $7::bigint
             ON CONFLICT (realm, scene_namespace, key)
             DO UPDATE SET value_enc = $4::bytea, value_size = ($5::bigint)::int, updated_at = $8::timestamp",
        );
        let row = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(ns.realm())
            .bind(ns.scene_namespace())
            .bind(key)
            .bind(value_enc)
            .bind(size)
            .bind(limits.max_value_size_bytes)
            .bind(limits.max_total_size_bytes)
            .bind(now_utc())
            .fetch_one(&mut *tx)
            .await?;
        quota_outcome(&row, size, limits)?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn env_delete(&self, ns: &SceneNamespace, key: &str) -> Result<(), ApiError> {
        sqlx::query("DELETE FROM worlds.env_variables WHERE realm = $1 AND scene_namespace = $2 AND key = $3")
            .bind(ns.realm())
            .bind(ns.scene_namespace())
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn env_delete_all(&self, ns: &SceneNamespace) -> Result<(), ApiError> {
        sqlx::query("DELETE FROM worlds.env_variables WHERE realm = $1 AND scene_namespace = $2")
            .bind(ns.realm())
            .bind(ns.scene_namespace())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn env_list_keys_page(
        &self,
        ns: &SceneNamespace,
        limit: i64,
        offset: i64,
        prefix: Option<&str>,
    ) -> Result<(Vec<String>, i64), ApiError> {
        let pat = prefix_pattern(prefix);
        let rows = sqlx::query(
            "SELECT key, count(*) OVER ()::bigint AS total FROM worlds.env_variables
             WHERE realm = $1 AND scene_namespace = $2
               AND ($3::text IS NULL OR key LIKE $3)
             ORDER BY key ASC LIMIT $4 OFFSET $5",
        )
        .bind(ns.realm())
        .bind(ns.scene_namespace())
        .bind(&pat)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let total = match rows.first() {
            Some(r) => r.get("total"),
            None if offset > 0 => self.env_count(ns, prefix).await?,
            None => 0,
        };
        let keys = rows.into_iter().map(|r| r.get("key")).collect();
        Ok((keys, total))
    }

    pub async fn env_count(
        &self,
        ns: &SceneNamespace,
        prefix: Option<&str>,
    ) -> Result<i64, ApiError> {
        let row = sqlx::query(
            "SELECT COUNT(*)::bigint AS count FROM worlds.env_variables
             WHERE realm = $1 AND scene_namespace = $2 AND ($3::text IS NULL OR key LIKE $3)",
        )
        .bind(ns.realm())
        .bind(ns.scene_namespace())
        .bind(prefix_pattern(prefix))
        .fetch_one(&self.pool)
        .await?;
        Ok(row.get("count"))
    }

    pub async fn env_size_info(&self, ns: &SceneNamespace) -> Result<SizeInfo, ApiError> {
        size_info_in(&self.pool, size_info_sql("env_variables", ns, ""), ns, None).await
    }
}

/// The `/usage` picture: `$3` (the key being written) is NULL, so `existing`
/// is 0 and `total` the scope's sum.
async fn size_info_in<'e, E: PgExecutor<'e>>(
    exec: E,
    sql: String,
    ns: &SceneNamespace,
    player: Option<&Address>,
) -> Result<SizeInfo, ApiError> {
    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(ns.realm())
        .bind(ns.scene_namespace())
        .bind(None::<&str>);
    if let Some(player) = player {
        query = query.bind(player);
    }
    let row = query.fetch_one(exec).await?;
    Ok(SizeInfo {
        existing_value_size: row.get("existing"),
        total_size: row.get("total"),
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::comms::world_identity::{Parcel, RealmId};

    fn cache_cfg(enabled: bool) -> StorageCacheConfig {
        StorageCacheConfig {
            enabled,
            ttl_seconds: 60,
            max_entries: 100,
            max_value_bytes: 64,
        }
    }

    /// A world scope's identity is fixed here; the scene's base does not matter.
    fn ns(realm: &str, base: &str) -> SceneNamespace {
        SceneNamespace::new(
            &RealmId::parse(realm).unwrap(),
            Parcel::parse(base).unwrap(),
            Some(uuid::Uuid::from_u128(1)),
        )
        .unwrap()
    }

    #[test]
    fn cache_keys_scope_by_namespace_scene_and_player() {
        let world = ns("foo.dcl.eth", "1,1");
        assert_eq!(
            world_value_cache_key(&world, "k"),
            "w:foo.dcl.eth:00000000-0000-0000-0000-000000000001:k",
            "a world scene's scope is its identity"
        );
        let p1 = ns("main", "1,1");
        let w = world_value_cache_key(&p1, "k");
        assert_eq!(w, "w:main:1,1:k");
        assert!(w.starts_with(&world_scene_prefix(&p1)));

        let p = player_value_cache_key(&p1, "0xabc", "k");
        assert_eq!(p, "p:main:1,1:0xabc:k");
        assert!(p.starts_with(&player_scope_prefix(&p1, Some("0xabc"))));
        assert!(p.starts_with(&player_scope_prefix(&p1, None)));
        assert!(!p.starts_with(&world_scene_prefix(&p1)));
        assert!(
            !player_value_cache_key(&p1, "0xabcd", "k")
                .starts_with(&player_scope_prefix(&p1, Some("0xabc")))
        );
    }

    #[test]
    fn cache_hits_after_insert_and_misses_after_invalidate() {
        let cache = StorageCache::new(cache_cfg(true));
        let key = world_value_cache_key(&ns("w.dcl.eth", "0,0"), "k");
        assert_eq!(cache.get(&key), None);

        cache.insert(key.clone(), Arc::from("{\"a\":1}"));
        assert_eq!(cache.get(&key).as_deref(), Some("{\"a\":1}"));

        cache.invalidate(&key);
        assert_eq!(cache.get(&key), None);
    }

    #[test]
    fn prefix_invalidation_clears_the_scope_and_spares_others() {
        let cache = StorageCache::new(cache_cfg(true));
        let (p1, p2) = (ns("w.dcl.eth", "1,1"), ns("main", "2,2"));
        let mine = player_value_cache_key(&p1, "0xabc", "k");
        let other_player = player_value_cache_key(&p1, "0xdef", "k");
        let other_scene = player_value_cache_key(&p2, "0xabc", "k");
        for k in [&mine, &other_player, &other_scene] {
            cache.insert(k.clone(), Arc::from("1"));
        }

        cache.invalidate_prefix(&player_scope_prefix(&p1, Some("0xabc")));
        assert_eq!(cache.get(&mine), None);
        assert!(cache.get(&other_player).is_some());
        assert!(cache.get(&other_scene).is_some());

        cache.invalidate_prefix(&player_scope_prefix(&p1, None));
        assert_eq!(cache.get(&other_player), None);
        assert!(cache.get(&other_scene).is_some());
    }

    #[test]
    fn disabled_cache_and_oversized_values_are_never_stored() {
        let cache = StorageCache::new(cache_cfg(false));
        cache.insert("k".to_string(), Arc::from("1"));
        assert_eq!(cache.get("k"), None);

        let cache = StorageCache::new(cache_cfg(true));
        let oversized = "x".repeat(65);
        cache.insert(
            "big".to_string(),
            Arc::from(format!("\"{oversized}\"").as_str()),
        );
        assert_eq!(cache.get("big"), None);
    }

    #[test]
    fn the_cache_is_bounded() {
        let cache = StorageCache::new(StorageCacheConfig {
            enabled: true,
            ttl_seconds: 60,
            max_entries: 3,
            max_value_bytes: 64,
        });
        for i in 0..3 {
            cache.insert(format!("k{i}"), Arc::from("1"));
        }
        assert_eq!(cache.len(), 3);
        cache.insert("k3".to_string(), Arc::from("1"));
        assert_eq!(
            cache.len(),
            1,
            "at capacity with nothing expired the memo is dropped, not grown"
        );
        assert_eq!(cache.get("k3").as_deref(), Some("1"));
    }

    #[test]
    fn prefix_pattern_escapes_like_wildcards() {
        assert_eq!(prefix_pattern(None), None);
        assert_eq!(prefix_pattern(Some("")), None);
        assert_eq!(prefix_pattern(Some("abc")).as_deref(), Some("abc%"));
        assert_eq!(prefix_pattern(Some("a%b")).as_deref(), Some("a\\%b%"));
        assert_eq!(prefix_pattern(Some("a_b")).as_deref(), Some("a\\_b%"));
        assert_eq!(prefix_pattern(Some("a\\b")).as_deref(), Some("a\\\\b%"));
    }

    #[test]
    fn quota_lock_scope_is_world_scene_or_player() {
        let world = ns("Foo.dcl.eth", "1,1");
        let genesis = ns("artemis", "1,1");
        assert_eq!(
            quota_lock_key("world-storage", &world, None),
            "world-storage:foo.dcl.eth"
        );
        assert_eq!(
            quota_lock_key("world-storage", &genesis, None),
            "world-storage:main:1,1"
        );
        assert_eq!(
            quota_lock_key("player-storage", &world, Some("0xabc")),
            "player-storage:foo.dcl.eth:0xabc"
        );
        assert_eq!(
            quota_lock_key("player-storage", &genesis, Some("0xabc")),
            "player-storage:main:1,1:0xabc"
        );
    }

    #[test]
    fn size_query_credits_the_exact_scene_and_scopes_shared_realms_per_scene() {
        let world = size_info_sql("world_storage", &ns("foo.dcl.eth", "0,0"), "");
        assert!(world.contains("FILTER (WHERE scene_namespace = $2 AND key = $3)"));
        assert!(world.contains("FROM worlds.world_storage WHERE realm = $1"));
        assert!(!world.trim_end().ends_with("AND scene_namespace = $2"));

        let shared = size_info_sql("world_storage", &ns("main", "0,0"), "");
        assert!(shared.trim_end().ends_with("AND scene_namespace = $2"));

        let player = size_info_sql(
            "player_storage",
            &ns("main", "0,0"),
            " AND player_address = $4",
        );
        assert!(player.contains("AND player_address = $4 AND scene_namespace = $2"));
    }
}
