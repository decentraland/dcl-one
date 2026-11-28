//! The node's clock a test drives, a test key's address and a scratch
//! database of the test's own whose Polygon collections copy has a complete
//! pass.

use std::cell::Cell;

use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;

use super::support::wallet;

thread_local! {
    static NOW: Cell<i64> = const { Cell::new(0) };
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

pub(super) async fn own_pool() -> Option<PgPool> {
    let pool = crate::db::scratch::isolated_pool(4).await?;
    sqlx::query(
        "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at, pass_started_at) \
         VALUES ('collections', 'matic', 'https://subgraph.example', timezone('UTC', now()), timezone('UTC', now()))",
    )
    .execute(&pool)
    .await
    .unwrap();
    Some(pool)
}
