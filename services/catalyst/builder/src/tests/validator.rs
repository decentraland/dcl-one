//! The wearable validator's work as creators, curators and the validator
//! meet it, through the router as `serve` mounts it: a standard collection's
//! first publication and a creator's changes submission record a validation
//! request; the validator claims it, extends its lease and posts a verdict,
//! each call signed with the shared secret; a leased job abandons, expires
//! and deletes requests. Each test has a database of its own and drives the
//! node's clock.

use std::cell::Cell;

use chrono::{DateTime, Duration, Utc};
use serde_json::{Value, json};
use sqlx::PgPool;

use super::support::wallet;

thread_local! {
    static NOW: Cell<i64> = const { Cell::new(0) };
    static SENT: Cell<i64> = const { Cell::new(0) };
}

pub(super) fn clock() -> DateTime<Utc> {
    DateTime::from_timestamp_millis(NOW.with(Cell::get)).unwrap()
}

pub(super) fn set_clock(at: DateTime<Utc>) {
    NOW.with(|now| now.set(at.timestamp_millis()));
}

pub(super) fn advance(by: Duration) {
    set_clock(clock() + by);
}

pub(super) fn address(key: &str) -> String {
    format!("{:#x}", wallet(key).address())
}

/// The `builder` flags document as the flags mirror stores it, with
/// `builder-auto-curation` holding `value`; `None` stores no document.
pub(super) async fn set_flag(pool: &PgPool, value: Option<Value>) {
    sqlx::query("DELETE FROM explore_api.feature_flags WHERE app = 'builder'")
        .execute(pool)
        .await
        .unwrap();
    if let Some(value) = value {
        sqlx::query(
            "INSERT INTO explore_api.feature_flags (app, document, fetched_at) VALUES ('builder', $1, timezone('UTC', now()))",
        )
        .bind(json!({ "flags": { "builder-auto-curation": value }, "variants": {} }))
        .execute(pool)
        .await
        .unwrap();
    }
}

pub(super) async fn own_pool() -> Option<PgPool> {
    let pool = crate::db::scratch::isolated_pool(4).await?;
    sqlx::query(
        "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at, pass_started_at) \
         VALUES ('collections', 'matic', 'https://subgraph.example', timezone('UTC', now()), timezone('UTC', now()))",
    )
    .execute(&pool)
    .await
    .unwrap();
    set_flag(&pool, Some(json!(true))).await;
    Some(pool)
}
