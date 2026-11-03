//! Sync state in the database: the global `sync_frontier` and `sync_heartbeat`
//! system properties, the per-peer cursors (`content.server_sync_cursors`)
//! and the processed-snapshot marks. Every write to the frontier and the
//! cursors is GREATEST-monotonic, so a stale offer can never rewind a
//! persisted resume point. The gauges the status routes read are the shared
//! [`SyncStatus`], updated on the same calls that persist.

use std::collections::HashSet;
use std::sync::Arc;

use futures::TryStreamExt;
use sqlx::PgPool;

use super::{SyncError, SyncStatus, Timestamp};
use crate::bloom_filter::BloomFilter;

const DEPLOYED_ENTITY_IDS_SQL: &str = "SELECT entity_id FROM content.deployments";

pub struct LiveDeploymentRepository {
    pool: PgPool,
    status: Arc<SyncStatus>,
}

impl LiveDeploymentRepository {
    pub fn new(pool: PgPool, status: Arc<SyncStatus>) -> Self {
        Self { pool, status }
    }

    /// Seeds the batch deployer's bloom filter at startup: every deployed
    /// entity id, streamed into `bloom` row by row over one connection (the
    /// socket is the backpressure) instead of a `Vec` of all ids first. A
    /// bloom hit still verifies against the table (`is_entity_deployed`).
    /// Returns the number of ids seeded.
    pub async fn seed_deployed_entity_ids(
        &self,
        bloom: &mut BloomFilter,
    ) -> Result<u64, SyncError> {
        let mut ids = sqlx::query_scalar::<_, String>(DEPLOYED_ENTITY_IDS_SQL).fetch(&self.pool);
        let mut count = 0u64;
        while let Some(id) = ids
            .try_next()
            .await
            .map_err(|e| SyncError::Storage(e.to_string()))?
        {
            bloom.add(&id);
            count += 1;
        }
        Ok(count)
    }

    pub async fn is_entity_deployed(
        &self,
        entity_id: &str,
        timestamp_ms: Timestamp,
    ) -> Result<bool, SyncError> {
        crate::sync_deployer::is_entity_deployed(&self.pool, entity_id, timestamp_ms).await
    }

    pub async fn get_sync_frontier(&self) -> Result<Timestamp, SyncError> {
        let row: Option<String> = sqlx::query_scalar(
            "SELECT value FROM content.system_properties WHERE key = 'sync_frontier'",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SyncError::Storage(e.to_string()))?;
        let ts = row.and_then(|v| v.parse::<Timestamp>().ok()).unwrap_or(0);
        if ts > 0 {
            self.status.set_sync_frontier_ms(ts);
        }
        Ok(ts)
    }

    pub async fn set_sync_frontier(&self, timestamp: Timestamp) -> Result<(), SyncError> {
        sqlx::query(
            "INSERT INTO content.system_properties (key, value) VALUES ('sync_frontier', $1)
             ON CONFLICT (key) DO UPDATE SET value = $1",
        )
        .bind(timestamp.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| SyncError::Storage(e.to_string()))?;
        self.status.set_sync_frontier_ms(timestamp);
        Ok(())
    }

    pub async fn advance_sync_frontier(&self, timestamp: Timestamp) -> Result<(), SyncError> {
        sqlx::query(ADVANCE_SYNC_FRONTIER_SQL)
            .bind(timestamp.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| SyncError::Storage(e.to_string()))?;
        self.status.advance_sync_frontier_ms(timestamp);
        Ok(())
    }

    /// The server's own persisted resume point, or None when the server has never had a
    /// confirmed durable point.
    pub async fn get_server_sync_cursor(
        &self,
        server_url: &str,
    ) -> Result<Option<Timestamp>, SyncError> {
        sqlx::query_scalar(
            "SELECT cursor_ms FROM content.server_sync_cursors WHERE server_url = $1",
        )
        .bind(server_url)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SyncError::Storage(e.to_string()))
    }

    /// GREATEST-monotonic advance of one server's own cursor, the per-server counterpart of
    /// `advance_sync_frontier`: a stale offer can never rewind the server's persisted resume
    /// point.
    pub async fn advance_server_sync_cursor(
        &self,
        server_url: &str,
        timestamp: Timestamp,
    ) -> Result<(), SyncError> {
        sqlx::query(ADVANCE_SERVER_SYNC_CURSOR_SQL)
            .bind(server_url)
            .bind(timestamp)
            .execute(&self.pool)
            .await
            .map_err(|e| SyncError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn set_sync_heartbeat(&self, timestamp: Timestamp) -> Result<(), SyncError> {
        sqlx::query(
            "INSERT INTO content.system_properties (key, value) VALUES ('sync_heartbeat', $1)
             ON CONFLICT (key) DO UPDATE SET value = $1",
        )
        .bind(timestamp.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| SyncError::Storage(e.to_string()))?;
        self.status.set_heartbeat_ms(timestamp);
        Ok(())
    }

    /// Repairs `deleter_deployment` for every row an intra-batch overwrite could not see:
    /// each pointer's successor chain, ordered by (entity_timestamp, lower(entity_id)), per type.
    /// An unpublished local scene is a link of the chain like any other (its parcels stay
    /// empty); a row the repair gives a deleter gives up its pointers, and the snapshot windows
    /// that may still list it are marked for regeneration, in the same transaction.
    pub async fn resolve_deleter_deployments(&self) -> Result<(), SyncError> {
        let start = std::time::Instant::now();
        let storage = |e: sqlx::Error| SyncError::Storage(e.to_string());
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let (superseded, cleared, outdated): (i64, i64, i64) = sqlx::query_as(
            r#"
            WITH pointer_successors AS (
                SELECT id AS older_id,
                       lead(id) OVER (
                           PARTITION BY entity_type, pointer
                           ORDER BY entity_timestamp, lower(entity_id)
                       ) AS newer_id
                FROM (
                    SELECT DISTINCT id, entity_type, entity_timestamp, entity_id,
                           unnest(entity_pointers) AS pointer
                    FROM content.deployments d
                    WHERE deleter_deployment IS NULL
                ) pointers
                WHERE pointer IS NOT NULL
            ), successors AS (
                SELECT DISTINCT ON (p.older_id) p.older_id, newer.id AS newer_id
                FROM pointer_successors p
                JOIN content.deployments newer ON newer.id = p.newer_id
                ORDER BY p.older_id, newer.entity_timestamp, lower(newer.entity_id)
            ), superseded AS (
                UPDATE content.deployments older
                SET deleter_deployment = successors.newer_id
                FROM successors
                WHERE older.id = successors.older_id
                  AND older.deleter_deployment IS NULL
                RETURNING older.entity_id, older.entity_timestamp
            ), cleared AS (
                DELETE FROM content.active_pointers ap
                USING superseded s
                WHERE ap.entity_id = s.entity_id
                RETURNING ap.pointer
            ), outdated AS (
                -- a generation stamped before its window began is regenerated on the next pass
                UPDATE content.snapshots sn
                SET generation_time = 'epoch'::timestamp
                WHERE EXISTS (
                    SELECT 1 FROM superseded s
                    WHERE s.entity_timestamp BETWEEN sn.init_timestamp AND sn.end_timestamp)
                RETURNING 1
            )
            SELECT (SELECT count(*) FROM superseded), (SELECT count(*) FROM cleared),
                   (SELECT count(*) FROM outdated)
            "#,
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(storage)?;
        tx.commit().await.map_err(storage)?;

        tracing::info!(
            rows_affected = superseded,
            pointers_cleared = cleared,
            snapshots_outdated = outdated,
            elapsed_ms = start.elapsed().as_millis() as u64,
            "Resolved deleter_deployment"
        );
        Ok(())
    }
}

/// GREATEST-monotonic upsert: a stale offer can never rewind the persisted frontier.
const ADVANCE_SYNC_FRONTIER_SQL: &str = "INSERT INTO content.system_properties (key, value) VALUES ('sync_frontier', $1) ON CONFLICT (key) DO UPDATE SET value = GREATEST(content.system_properties.value::bigint, EXCLUDED.value::bigint)::text";

/// GREATEST-monotonic upsert of one server's own cursor: the per-server resume state has the
/// same never-rewinds guarantee as the global frontier.
const ADVANCE_SERVER_SYNC_CURSOR_SQL: &str = "INSERT INTO content.server_sync_cursors (server_url, cursor_ms, updated_at) VALUES ($1, $2, timezone('UTC', now())) ON CONFLICT (server_url) DO UPDATE SET cursor_ms = GREATEST(content.server_sync_cursors.cursor_ms, EXCLUDED.cursor_ms), updated_at = timezone('UTC', now())";

pub struct LiveProcessedSnapshotStore {
    pool: PgPool,
}

const PROCESSED_SNAPSHOT_LOOKUP_CHUNK_SIZE: usize = 1000;

impl LiveProcessedSnapshotStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn filter_processed(&self, hashes: &[String]) -> Result<HashSet<String>, SyncError> {
        let rows: Vec<String> =
            sqlx::query_scalar("SELECT hash FROM content.processed_snapshots WHERE hash = ANY($1)")
                .bind(hashes)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| SyncError::Storage(e.to_string()))?;
        Ok(rows.into_iter().collect())
    }

    /// Port of upstream's `filterProcessedSnapshotsInChunks`. Serial rather than concurrent on
    /// purpose: the point is to bound the load one decision pass puts on storage, and issuing every
    /// chunk at once would keep the peak it is meant to remove.
    pub async fn filter_processed_in_chunks(
        &self,
        hashes: impl IntoIterator<Item = String>,
    ) -> Result<HashSet<String>, SyncError> {
        let mut processed = HashSet::new();
        let mut chunk: Vec<String> = Vec::with_capacity(PROCESSED_SNAPSHOT_LOOKUP_CHUNK_SIZE);
        for hash in hashes {
            chunk.push(hash);
            if chunk.len() == PROCESSED_SNAPSHOT_LOOKUP_CHUNK_SIZE {
                processed.extend(self.filter_processed(&chunk).await?);
                chunk.clear();
            }
        }
        if !chunk.is_empty() {
            processed.extend(self.filter_processed(&chunk).await?);
        }
        Ok(processed)
    }

    /// The subset of `hashes` this node generated itself: rows of its own
    /// `content.snapshots` (upstream's `snapshotStorage.has`). A peer
    /// advertising one serves exactly the entities this node already holds
    /// active, so the bootstrap pass skips it without downloading. The table
    /// holds one row per generation window (tens of rows), so the probe is a
    /// scan of it, not an index.
    pub async fn own_snapshot_hashes(
        &self,
        hashes: &[String],
    ) -> Result<HashSet<String>, SyncError> {
        if hashes.is_empty() {
            return Ok(HashSet::new());
        }
        let rows: Vec<String> =
            sqlx::query_scalar("SELECT DISTINCT hash FROM content.snapshots WHERE hash = ANY($1)")
                .bind(hashes)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| SyncError::Storage(e.to_string()))?;
        Ok(rows.into_iter().collect())
    }

    pub async fn mark_processed(&self, hash: &str) -> Result<(), SyncError> {
        self.mark_processed_many(std::slice::from_ref(&hash.to_string()))
            .await
    }

    pub async fn mark_processed_many(&self, hashes: &[String]) -> Result<(), SyncError> {
        if hashes.is_empty() {
            return Ok(());
        }
        sqlx::query(
            "INSERT INTO content.processed_snapshots (hash, process_time)
             SELECT h, timezone('UTC', now()) FROM unnest($1::text[]) AS h
             ON CONFLICT DO NOTHING",
        )
        .bind(hashes)
        .execute(&self.pool)
        .await
        .map_err(|e| SyncError::Storage(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync_test_support::{test_pool, unique};

    #[test]
    fn sync_frontier_upsert_is_greatest_monotonic() {
        assert!(
            ADVANCE_SYNC_FRONTIER_SQL.contains(
                "GREATEST(content.system_properties.value::bigint, EXCLUDED.value::bigint)"
            )
        );
        assert!(ADVANCE_SYNC_FRONTIER_SQL.contains("ON CONFLICT (key) DO UPDATE"));
    }

    #[test]
    fn server_sync_cursor_upsert_is_greatest_monotonic() {
        assert!(
            ADVANCE_SERVER_SYNC_CURSOR_SQL
                .contains("GREATEST(content.server_sync_cursors.cursor_ms, EXCLUDED.cursor_ms)")
        );
        assert!(ADVANCE_SERVER_SYNC_CURSOR_SQL.contains("ON CONFLICT (server_url) DO UPDATE"));
    }

    /// A database of its own for a repair test (the repair rewrites the whole table), with the
    /// admin pool that drops it.
    async fn repair_database() -> Option<(PgPool, PgPool, String)> {
        let base_url = std::env::var("CATALYST_TEST_DATABASE_URL").ok()?;
        let opts: sqlx::postgres::PgConnectOptions = base_url.parse().unwrap();
        let admin = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_with(opts.clone().database("postgres"))
            .await
            .unwrap();
        let name = format!("catalyst_test_repair_{}", &unique()[..12]);
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
            .execute(&admin)
            .await
            .unwrap();
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect_with(opts.database(&name))
            .await
            .unwrap();
        crate::db::apply_schema(&pool).await.unwrap();
        Some((admin, pool, name))
    }

    async fn drop_repair_database(admin: PgPool, pool: PgPool, name: &str) {
        pool.close().await;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE \"{name}\" WITH (FORCE)"
        )))
        .execute(&admin)
        .await
        .unwrap();
    }

    async fn insert_scene(pool: &PgPool, id: &str, ts: f64, pointer: &str) {
        sqlx::query(
            "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id,
                 entity_timestamp, entity_pointers, local_timestamp, auth_chain)
             VALUES ('0xabc', 'v3', 'scene', $1, to_timestamp($2 / 1000.0), ARRAY[$3],
                     timezone('UTC', now()), '[]')",
        )
        .bind(id)
        .bind(ts)
        .bind(pointer)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn deleters(pool: &PgPool) -> Vec<(String, Option<String>)> {
        sqlx::query_as(
            "SELECT o.entity_id, d.entity_id FROM content.deployments o
             LEFT JOIN content.deployments d ON d.id = o.deleter_deployment ORDER BY o.entity_id",
        )
        .fetch_all(pool)
        .await
        .unwrap()
    }

    /// The repair a process without a sync worker runs at startup: skipped while a
    /// sync leader holds the `sync` lease, applied once the lease is free.
    #[tokio::test]
    async fn the_startup_repair_runs_only_without_a_sync_leader() {
        use crate::jobs::{JobState, Jobs, LeaseSettings, Leases};
        let Some((admin, pool, name)) = repair_database().await else {
            return;
        };
        insert_scene(&pool, "bafyolder", 1_000.0, "0,0").await;
        insert_scene(&pool, "bafynewer", 2_000.0, "0,0").await;
        let lease_settings = LeaseSettings {
            namespace: format!("repair-{}", &unique()[..12]),
            ..LeaseSettings::default()
        };
        let leader = Arc::new(Leases::new(pool.clone(), lease_settings.clone()));
        let held = leader
            .try_acquire("sync")
            .await
            .unwrap()
            .expect("sync lease");

        let run = |jobs: Arc<Jobs>| {
            let pool = pool.clone();
            async move {
                super::super::worker::spawn_startup_repair(&jobs, pool);
                for _ in 0..200 {
                    let report = jobs.report();
                    let job = report.iter().find(|j| j.name == "deleter-repair").unwrap();
                    if job.state == JobState::Finished {
                        return job.progress.clone();
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
                panic!(
                    "the repair job never finished: {:?}",
                    jobs.report()
                        .iter()
                        .map(|j| (&j.name, j.state))
                        .collect::<Vec<_>>()
                );
            }
        };
        let jobs = Arc::new(Jobs::with_leases(pool.clone(), lease_settings.clone()));
        let progress = run(jobs.clone()).await;
        assert_eq!(
            progress.as_deref(),
            Some("skipped: a sync leader holds the sync lease")
        );
        jobs.shutdown(std::time::Duration::from_secs(5)).await;
        assert_eq!(
            deleters(&pool).await,
            vec![("bafynewer".into(), None), ("bafyolder".into(), None)]
        );

        held.release().await;
        let jobs = Arc::new(Jobs::with_leases(pool.clone(), lease_settings));
        assert_eq!(run(jobs.clone()).await.as_deref(), Some("done"));
        jobs.shutdown(std::time::Duration::from_secs(5)).await;
        assert_eq!(
            deleters(&pool).await,
            vec![
                ("bafynewer".into(), None),
                ("bafyolder".into(), Some("bafynewer".into())),
            ]
        );
        drop(leader);

        drop_repair_database(admin, pool, &name).await;
    }

    /// The bootstrap repair walks each pointer's chain of undeleted rows; an
    /// unpublished local scene is a link of it like any other, so the peer scene
    /// older than it gains the tombstone as its deleter.
    #[tokio::test]
    async fn the_deleter_repair_keeps_unpublished_local_scenes_in_the_chain() {
        let Some((admin, pool, name)) = repair_database().await else {
            return;
        };
        for (id, ts) in [
            ("bafypeer", 1_000.0),
            ("bafylocal", 2_000.0),
            ("bafyold", 500.0),
        ] {
            insert_scene(&pool, id, ts, "0,0").await;
        }
        sqlx::query(
            "INSERT INTO content.local_entities (entity_id, signer, tombstoned_at)
             VALUES ('bafylocal', '0xabc', timezone('UTC', now()))",
        )
        .execute(&pool)
        .await
        .unwrap();

        LiveDeploymentRepository::new(pool.clone(), Arc::new(SyncStatus::default()))
            .resolve_deleter_deployments()
            .await
            .unwrap();
        assert_eq!(
            deleters(&pool).await,
            vec![
                ("bafylocal".into(), None),
                ("bafyold".into(), Some("bafypeer".into())),
                ("bafypeer".into(), Some("bafylocal".into())),
            ]
        );

        drop_repair_database(admin, pool, &name).await;
    }

    /// A row the repair gives a deleter gives up its pointers: on 0,0 the scene an
    /// unpublished local scene had superseded but that still holds the parcel goes,
    /// leaving it empty; on 0,1 the newest scene keeps the parcel.
    #[tokio::test]
    async fn the_deleter_repair_clears_the_pointers_of_rows_it_supersedes() {
        let Some((admin, pool, name)) = repair_database().await else {
            return;
        };
        for (id, ts, pointer) in [
            ("bafyold", 500.0, "0,0"),
            ("bafypeer", 1_000.0, "0,0"),
            ("bafylocal", 2_000.0, "0,0"),
            ("bafymid", 1_500.0, "0,1"),
            ("bafynewest", 3_000.0, "0,1"),
        ] {
            insert_scene(&pool, id, ts, pointer).await;
        }
        for sql in [
            "INSERT INTO content.local_entities (entity_id, signer, tombstoned_at)
             VALUES ('bafylocal', '0xabc', timezone('UTC', now()))",
            "UPDATE content.deployments o SET deleter_deployment = d.id
             FROM content.deployments d
             WHERE o.entity_id = 'bafyold' AND d.entity_id = 'bafypeer'",
            "INSERT INTO content.active_pointers (pointer, entity_id, entity_type)
             VALUES ('0,0', 'bafypeer', 'scene'), ('0,1', 'bafynewest', 'scene')",
        ] {
            sqlx::query(sql).execute(&pool).await.unwrap();
        }

        LiveDeploymentRepository::new(pool.clone(), Arc::new(SyncStatus::default()))
            .resolve_deleter_deployments()
            .await
            .unwrap();
        assert_eq!(
            deleters(&pool).await,
            vec![
                ("bafylocal".into(), None),
                ("bafymid".into(), Some("bafynewest".into())),
                ("bafynewest".into(), None),
                ("bafyold".into(), Some("bafypeer".into())),
                ("bafypeer".into(), Some("bafylocal".into())),
            ]
        );
        let active: Vec<(String, String)> = sqlx::query_as(
            "SELECT pointer, entity_id FROM content.active_pointers ORDER BY pointer",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(active, vec![("0,1".into(), "bafynewest".into())]);

        drop_repair_database(admin, pool, &name).await;
    }

    #[tokio::test]
    async fn processed_marks_round_trip_in_chunks() {
        let Some(pool) = test_pool(2).await else {
            return;
        };
        let store = LiveProcessedSnapshotStore::new(pool.clone());
        let tag = unique();
        let hashes: Vec<String> = (0..2_005).map(|i| format!("snap-{tag}-{i}")).collect();
        store.mark_processed_many(&hashes[..1_500]).await.unwrap();
        store.mark_processed(&hashes[1_500]).await.unwrap();
        store.mark_processed(&hashes[1_500]).await.unwrap();

        let processed = store
            .filter_processed_in_chunks(hashes.iter().cloned())
            .await
            .unwrap();
        assert_eq!(processed.len(), 1_501);
        assert!(processed.contains(&hashes[0]) && processed.contains(&hashes[1_500]));
        assert!(!processed.contains(&hashes[1_501]));

        sqlx::query("DELETE FROM content.processed_snapshots WHERE hash LIKE $1")
            .bind(format!("snap-{tag}-%"))
            .execute(&pool)
            .await
            .unwrap();
    }

    /// The own-snapshot probe answers exactly the advertised hashes that are
    /// rows of this node's content.snapshots, whatever their window.
    #[tokio::test]
    async fn own_snapshot_hashes_are_the_rows_of_content_snapshots() {
        let Some(pool) = test_pool(2).await else {
            return;
        };
        let store = LiveProcessedSnapshotStore::new(pool.clone());
        let tag = unique();
        let ours = [format!("bafy-own-{tag}-a"), format!("bafy-own-{tag}-b")];
        let offset: i64 = sqlx::query_scalar("SELECT (random() * 1e9)::bigint")
            .fetch_one(&pool)
            .await
            .unwrap();
        for (i, hash) in ours.iter().enumerate() {
            sqlx::query(
                "INSERT INTO content.snapshots
                     (hash, init_timestamp, end_timestamp, replaced_hashes, number_of_entities, generation_time)
                 VALUES ($1, to_timestamp(4102444800 + $2 + $3 * 86400) AT TIME ZONE 'UTC',
                         to_timestamp(4102444800 + $2 + ($3 + 1) * 86400) AT TIME ZONE 'UTC',
                         '{}', 0, timezone('UTC', now()))",
            )
            .bind(hash)
            .bind(offset as f64)
            .bind(i as f64)
            .execute(&pool)
            .await
            .unwrap();
        }
        let advertised = vec![ours[0].clone(), format!("bafy-peer-{tag}"), ours[1].clone()];
        let own = store.own_snapshot_hashes(&advertised).await.unwrap();
        assert_eq!(own, ours.iter().cloned().collect::<HashSet<_>>());
        assert!(store.own_snapshot_hashes(&[]).await.unwrap().is_empty());

        sqlx::query("DELETE FROM content.snapshots WHERE hash LIKE $1")
            .bind(format!("bafy-own-{tag}-%"))
            .execute(&pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn the_bloom_seed_streams_every_deployed_entity_id() {
        let Some(pool) = test_pool(2).await else {
            return;
        };
        let repo = LiveDeploymentRepository::new(pool.clone(), Arc::new(SyncStatus::default()));
        let tag = unique();
        let ids: Vec<String> = (0..3).map(|i| format!("bafk-seed-{tag}-{i}")).collect();
        for id in &ids {
            sqlx::query(
                "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id,
                    entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain)
                 VALUES ($1, 'v3', 'profile', $2, '{}', now(), ARRAY[$1], now(), '[]')",
            )
            .bind(format!("0x{tag:0>40}"))
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        }

        let mut bloom = BloomFilter::with_capacity(100_000);
        let seeded = repo.seed_deployed_entity_ids(&mut bloom).await.unwrap();
        let in_table: i64 = sqlx::query_scalar("SELECT count(*) FROM content.deployments")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(seeded >= ids.len() as u64, "seeded {seeded} ids");
        assert!(
            seeded <= in_table as u64,
            "seeded {seeded} ids, table holds {in_table}"
        );
        for id in &ids {
            assert!(
                bloom.maybe_contains(id),
                "{id} was streamed into the filter"
            );
        }
        assert!(!bloom.maybe_contains(&format!("bafk-seed-{tag}-missing")));

        sqlx::query("DELETE FROM content.deployments WHERE entity_id LIKE $1")
            .bind(format!("bafk-seed-{tag}-%"))
            .execute(&pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn the_heartbeat_and_frontier_feed_the_shared_status() {
        let Some(pool) = test_pool(2).await else {
            return;
        };
        let status = Arc::new(SyncStatus::default());
        let repo = LiveDeploymentRepository::new(pool.clone(), status.clone());
        let server = format!("https://{}.example/content", unique());

        repo.set_sync_heartbeat(4_242).await.unwrap();
        assert_eq!(status.heartbeat_ms(), Some(4_242));

        repo.advance_server_sync_cursor(&server, 7).await.unwrap();
        repo.advance_server_sync_cursor(&server, 5).await.unwrap();
        assert_eq!(repo.get_server_sync_cursor(&server).await.unwrap(), Some(7));

        sqlx::query("DELETE FROM content.server_sync_cursors WHERE server_url = $1")
            .bind(&server)
            .execute(&pool)
            .await
            .unwrap();
    }
}
