//! `social.client_events`: one row per accepted explorer or web event, keyed
//! by Segment's `messageId` (a redelivery inserts nothing), immutable, purged
//! by `created_at` (DATABASE.md, "Segment webhook events workload").

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;

#[derive(Debug, Clone)]
pub struct ClientEventRow {
    pub key: String,
    pub source: &'static str,
    pub sub_type: &'static str,
    /// Lower-case `0x` address: the chain owner or the identify's wallet.
    pub address: String,
    pub session_id: Option<String>,
    pub anonymous_id: Option<String>,
    pub realm: Option<String>,
    pub ip: Option<String>,
    pub reported_at: DateTime<Utc>,
    pub received_at: DateTime<Utc>,
    /// The sub-type part of the notifier's event metadata.
    pub metadata: Value,
}

/// Inserts `row`; `false` when its key is already stored.
pub async fn insert_client_event(pool: &PgPool, row: &ClientEventRow) -> sqlx::Result<bool> {
    let result = sqlx::query(
        "INSERT INTO social.client_events \
         (key, source, sub_type, address, session_id, anonymous_id, realm, ip, reported_at, received_at, metadata) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) \
         ON CONFLICT (key) DO NOTHING",
    )
    .bind(&row.key)
    .bind(row.source)
    .bind(row.sub_type)
    .bind(&row.address)
    .bind(&row.session_id)
    .bind(&row.anonymous_id)
    .bind(&row.realm)
    .bind(&row.ip)
    .bind(row.reported_at.naive_utc())
    .bind(row.received_at.naive_utc())
    .bind(&row.metadata)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub const PURGE_BATCH: i64 = 5_000;
pub const PURGE_MAX_BATCHES: usize = 200;

/// Deletes rows created before `cutoff`, oldest first, in batches of
/// [`PURGE_BATCH`] keys and at most [`PURGE_MAX_BATCHES`] a pass; the count deleted.
pub async fn purge(pool: &PgPool, cutoff: DateTime<Utc>) -> sqlx::Result<u64> {
    let mut deleted = 0;
    for _ in 0..PURGE_MAX_BATCHES {
        let batch = sqlx::query(
            "DELETE FROM social.client_events WHERE key IN \
             (SELECT key FROM social.client_events WHERE created_at < $1 ORDER BY created_at, key LIMIT $2)",
        )
        .bind(cutoff.naive_utc())
        .bind(PURGE_BATCH)
        .execute(pool)
        .await?
        .rows_affected();
        deleted += batch;
        if batch < PURGE_BATCH as u64 {
            break;
        }
    }
    Ok(deleted)
}
