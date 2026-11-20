//! Avatar renders a profile names in `metadata.avatars[].avatar.snapshots` but not in
//! `content[]`: a profile could reference images an earlier version of it uploaded. Sync
//! fetches them as the reference snapshots-fetcher does, and records the stored ones in
//! `content.content_files` so the blob collector and the history prune keep them. [`repair`]
//! adds the rows to profiles stored without them, as the reference catalyst's content-files
//! fixer did (`catalyst content repair-profile-snapshots`).

use std::collections::{HashMap, HashSet};

use futures::StreamExt;
use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;

use crate::auth::is_canonical_cid;
use crate::deploy_remote_entity::download_file_with_retries;
use crate::http::CheckedRedirectClient;
use crate::storage::Storage;

/// Snapshot values read from one profile; far above a real profile (four per avatar), so it
/// only bounds hostile or corrupt metadata, as snapshots-fetcher's cap does.
pub const MAX_SNAPSHOTS_PER_PROFILE: usize = 1000;

/// Blobs [`repair`] fetches at once.
const REPAIR_FETCH_CONCURRENCY: usize = 8;

/// The `content_files` rows a profile's snapshots add to its `content` (`(key, hash)` pairs):
/// one per snapshot whose hash `content` does not list, keyed `{name}.png` as the reference
/// fixer keys them. A value is a hash or a `.../contents/{hash}` URL; one that is not a
/// canonical CID, or whose key `content` or an earlier snapshot holds (a deployment's keys are
/// unique), adds nothing.
pub fn snapshot_files(
    avatars: Option<&Value>,
    content: &[(String, String)],
) -> Vec<(String, String)> {
    let Some(avatars) = avatars.and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut hashes: HashSet<&str> = content.iter().map(|(_, h)| h.as_str()).collect();
    let mut keys: HashSet<String> = content.iter().map(|(k, _)| k.clone()).collect();
    let mut files = Vec::new();
    let snapshots = avatars
        .iter()
        .filter_map(|a| a.pointer("/avatar/snapshots").and_then(Value::as_object))
        .flatten()
        .take(MAX_SNAPSHOTS_PER_PROFILE);
    for (name, value) in snapshots {
        let Some(hash) = value.as_str().map(hash_of_snapshot) else {
            continue;
        };
        if !is_canonical_cid(hash) || hashes.contains(hash) {
            continue;
        }
        let key = format!("{name}.png");
        if keys.insert(key.clone()) {
            hashes.insert(hash);
            files.push((key, hash.to_string()));
        }
    }
    files
}

/// The hash a snapshot value names: itself, or what follows `/contents/` in a URL.
pub(crate) fn hash_of_snapshot(value: &str) -> &str {
    value
        .rfind("/contents/")
        .map_or(value, |i| &value[i + "/contents/".len()..])
}

/// What one [`repair`] pass did.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct RepairReport {
    /// Active profiles read.
    pub profiles: u64,
    /// `content_files` rows inserted.
    pub rows: u64,
    /// Blobs fetched from the peers.
    pub fetched: u64,
    /// Snapshots left without a row: no peer served the blob. A later pass retries them.
    pub unavailable: u64,
}

/// One pass over the active profiles, `batch` at a time by id: each snapshot file
/// [`snapshot_files`] finds missing from the profile's `content_files` is fetched from
/// `servers` when the store lacks it, and gets its row once the blob is stored. Rows are
/// inserted under the store's shared lock after the blobs are re-checked, so a collector batch
/// never unlinks a blob a row it did not see names; a row already present is kept, so a pass
/// can be repeated or interrupted.
pub async fn repair(
    pool: &PgPool,
    storage: &Storage,
    client: &CheckedRedirectClient,
    servers: &[String],
    batch: i64,
) -> anyhow::Result<RepairReport> {
    let mut report = RepairReport::default();
    let mut after = 0i32;
    loop {
        let profiles: Vec<(i32, Option<Value>)> = sqlx::query_as(
            "SELECT id, entity_metadata -> 'v' -> 'avatars' FROM content.deployments \
             WHERE id > $1 AND entity_type = 'profile' AND deleter_deployment IS NULL \
             ORDER BY id LIMIT $2",
        )
        .bind(after)
        .bind(batch)
        .fetch_all(pool)
        .await?;
        let Some(&(last, _)) = profiles.last() else {
            return Ok(report);
        };
        after = last;
        report.profiles += profiles.len() as u64;

        let ids: Vec<i32> = profiles.iter().map(|(id, _)| *id).collect();
        let rows: Vec<(i32, String, String)> = sqlx::query_as(
            "SELECT deployment, key, content_hash FROM content.content_files \
             WHERE deployment = ANY($1)",
        )
        .bind(&ids)
        .fetch_all(pool)
        .await?;
        let mut content: HashMap<i32, Vec<(String, String)>> = HashMap::new();
        for (deployment, key, hash) in rows {
            content.entry(deployment).or_default().push((key, hash));
        }
        let missing: Vec<(i32, String, String)> = profiles
            .iter()
            .flat_map(|(id, avatars)| {
                let content = content.get(id).map_or(&[][..], Vec::as_slice);
                snapshot_files(avatars.as_ref(), content)
                    .into_iter()
                    .map(|(key, hash)| (*id, key, hash))
            })
            .collect();
        if missing.is_empty() {
            continue;
        }

        let hashes: HashSet<&str> = missing.iter().map(|(_, _, h)| h.as_str()).collect();
        let held = storage
            .exist_multiple(&hashes.into_iter().collect::<Vec<_>>())
            .await?;
        let fetches = held.iter().filter(|(_, held)| !**held).map(|(hash, _)| async move {
            let fetched = download_file_with_retries(client, storage, hash, servers, None).await;
            if let Err(e) = &fetched {
                tracing::warn!(hash, error = %e, "profile snapshot repair: no peer served the blob");
            }
            fetched.is_ok()
        });
        report.fetched += futures::stream::iter(fetches)
            .buffer_unordered(REPAIR_FETCH_CONCURRENCY)
            .filter(|ok| std::future::ready(*ok))
            .count()
            .await as u64;

        let mut tx = pool.begin().await?;
        crate::storage::gc::lock_store_shared(&mut tx).await?;
        let stored = storage
            .exist_multiple(&held.keys().map(String::as_str).collect::<Vec<_>>())
            .await?;
        let (mut deployments, mut keys, mut hashes) = (Vec::new(), Vec::new(), Vec::new());
        for (deployment, key, hash) in missing {
            if stored.get(&hash).copied().unwrap_or(false) {
                deployments.push(deployment);
                keys.push(key);
                hashes.push(hash);
            } else {
                report.unavailable += 1;
            }
        }
        let inserted = sqlx::query(
            "INSERT INTO content.content_files (deployment, key, content_hash) \
             SELECT r.deployment, r.key, r.hash \
             FROM unnest($1::int[], $2::text[], $3::text[]) AS r(deployment, key, hash) \
             WHERE EXISTS (SELECT 1 FROM content.deployments d \
                           WHERE d.id = r.deployment AND d.deleter_deployment IS NULL) \
             ON CONFLICT (deployment, key) DO NOTHING",
        )
        .bind(&deployments)
        .bind(&keys)
        .bind(&hashes)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        report.rows += inserted.rows_affected();
        tracing::info!(after, ?report, "profile snapshot repair: batch done");
    }
}

#[cfg(test)]
mod tests;
