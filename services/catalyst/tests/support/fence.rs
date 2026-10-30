//! A scratch database for the tests that exercise the v4 control authority over
//! `social.archipelago_v4_*` (created by `schema.sql`, the comms section of
//! `schema/40_social.sql`), with a small pool of its own. Needs
//! `CATALYST_TEST_DATABASE_URL`; skips otherwise.
#![allow(dead_code)]

#[path = "gate.rs"]
mod gate;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

use crate::db::{ScratchDb, scratch_database};

pub struct Fixture {
    db: Option<ScratchDb>,
    /// A four-connection pool of its own, so a test can hold the whole pool to stall the authority.
    pub pool: PgPool,
}

impl Fixture {
    pub async fn new() -> Option<Self> {
        gate::required_database_in_ci();
        let db = scratch_database().await?;
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .acquire_timeout(std::time::Duration::from_secs(10))
            .connect_with(db.pool.connect_options().as_ref().clone())
            .await
            .expect("connect the fixture pool");
        Some(Self { db: Some(db), pool })
    }

    pub async fn finish(mut self) {
        self.pool.close().await;
        if let Some(db) = self.db.take() {
            db.teardown().await;
        }
    }
}
