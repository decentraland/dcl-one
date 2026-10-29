//! One throwaway database per test, created on the cluster CATALYST_TEST_DATABASE_URL
//! points at and seeded with schema.sql, so the global sync state (the frontier, the
//! cursors, resolve_deleter_deployments over the whole table) can be asserted without
//! other tests in the way. Dropped on teardown; a test that panics leaves its
//! `catalyst_test_*` database behind for the next `DROP DATABASE`.

use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};
use std::time::Duration;

pub struct ScratchDb {
    pub pool: PgPool,
    admin: PgPool,
    name: String,
}

pub async fn scratch_database() -> Option<ScratchDb> {
    let url = std::env::var("CATALYST_TEST_DATABASE_URL").ok()?;
    let base: PgConnectOptions = url
        .parse()
        .expect("CATALYST_TEST_DATABASE_URL must be a postgres DSN");
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(base.clone().database("postgres"))
        .await
        .unwrap_or_else(|e| panic!("connecting to disposable PostgreSQL: {e}"));
    let name = format!("catalyst_test_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
        .execute(&admin)
        .await
        .unwrap_or_else(|e| panic!("CREATE DATABASE {name}: {e}"));
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(base.database(&name).options([("TimeZone", "UTC")]))
        .await
        .unwrap_or_else(|e| panic!("connecting to {name}: {e}"));
    catalyst::db::apply_schema(&pool)
        .await
        .unwrap_or_else(|e| panic!("applying schema.sql to {name}: {e}"));
    Some(ScratchDb { pool, admin, name })
}

impl ScratchDb {
    pub async fn teardown(self) {
        self.pool.close().await;
        let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE \"{}\" WITH (FORCE)",
            self.name
        )))
        .execute(&self.admin)
        .await;
    }
}
