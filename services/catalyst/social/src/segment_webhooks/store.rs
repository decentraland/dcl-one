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

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use chrono::Utc;
    use sqlx::postgres::PgListener;

    use super::*;

    const ADDRESS: &str = "0x00000000000000000000000000000000000000e1";

    fn row(key: &str) -> ClientEventRow {
        ClientEventRow {
            key: key.to_string(),
            source: "client",
            sub_type: "move-to-parcel",
            address: ADDRESS.to_string(),
            session_id: None,
            anonymous_id: Some("anon-1".to_string()),
            realm: Some("main".to_string()),
            ip: Some("203.0.113.9".to_string()),
            reported_at: Utc::now(),
            received_at: Utc::now(),
            metadata: serde_json::json!({ "parcel": { "newParcel": "(1, 1)" } }),
        }
    }

    /// The trigger of `schemas/40_social.sql`: one `social.client_events`
    /// notification per stored row, the identity columns without the
    /// metadata, and none for a redelivered key.
    #[tokio::test]
    async fn every_stored_client_event_is_announced_once() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let mut listener = PgListener::connect_with(&pool).await.unwrap();
        listener.listen("social.client_events").await.unwrap();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let key = format!("trigger-{nanos}");
        assert!(insert_client_event(&pool, &row(&key)).await.unwrap());
        let payload = loop {
            let notification = tokio::time::timeout(Duration::from_secs(5), listener.recv())
                .await
                .expect("announced within 5 s")
                .unwrap();
            assert_eq!(notification.channel(), "social.client_events");
            let payload: serde_json::Value = serde_json::from_str(notification.payload()).unwrap();
            if payload["key"] == key {
                break payload;
            }
        };
        assert_eq!(payload["source"], "client");
        assert_eq!(payload["sub_type"], "move-to-parcel");
        assert_eq!(payload["address"], ADDRESS);
        assert_eq!(payload["anonymous_id"], "anon-1");
        assert_eq!(payload["realm"], "main");
        assert_eq!(payload["ip"], "203.0.113.9");
        assert!(payload["session_id"].is_null());
        assert!(payload["reported_at"].is_string());
        assert!(payload.get("metadata").is_none());
        assert!(!insert_client_event(&pool, &row(&key)).await.unwrap());
        let deadline = tokio::time::Instant::now() + Duration::from_millis(500);
        while let Ok(Ok(notification)) = tokio::time::timeout_at(deadline, listener.recv()).await {
            let payload: serde_json::Value = serde_json::from_str(notification.payload()).unwrap();
            assert_ne!(payload["key"], key, "a redelivered key announces nothing");
        }
        sqlx::query("DELETE FROM social.client_events WHERE key = $1")
            .bind(&key)
            .execute(&pool)
            .await
            .unwrap();
    }
}
