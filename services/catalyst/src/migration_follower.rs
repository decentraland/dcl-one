//! Temporary follower process: database, explicit pilot routes and nothing else.
use anyhow::Context;
use clap::Args;
use sqlx::{PgConnection, PgPool};
use std::{net::IpAddr, time::Duration};
use uuid::Uuid;

#[derive(Args)]
pub struct FollowerConfig {
    #[arg(long, default_value = "127.0.0.1")]
    pub host: IpAddr,
    #[arg(long, default_value_t = 5213)]
    pub port: u16,
    #[arg(long)]
    pub stream: String,
    #[arg(long)]
    pub generation: Uuid,
    #[arg(long, default_value_t = 30)]
    pub max_capture_age_seconds: i64,
}

pub async fn run(config: FollowerConfig) -> anyhow::Result<()> {
    let database_url = std::env::var("MIGRATION_FOLLOWER_DATABASE_URL")
        .context("MIGRATION_FOLLOWER_DATABASE_URL is required")?;
    let token = std::env::var("MIGRATION_FOLLOWER_TOKEN")
        .context("MIGRATION_FOLLOWER_TOKEN is required")?;
    // No normal Config/bootstrap/serve initialization: those own jobs and providers.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(10))
        .after_connect(|connection, _| {
            Box::pin(async move {
                sqlx::raw_sql("SET statement_timeout = '30s'; SET lock_timeout = '5s'")
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect(&database_url)
        .await
        .context("connecting follower database")?;
    let follower = crate::explore::migration_follower::Follower::new(
        pool,
        config.stream,
        config.generation,
        config.max_capture_age_seconds,
        &token,
    )?;
    let listener = tokio::net::TcpListener::bind((config.host, config.port)).await?;
    axum::serve(
        listener,
        crate::explore::migration_follower::router(follower),
    )
    .with_graceful_shutdown(crate::serve::shutdown_signal())
    .await?;
    Ok(())
}

/// Startup admission fence, paired with migration registration's exclusive lock.
/// Retain this detached session for the runtime lifetime. Deployment must still
/// hard-stop older binaries and fence lost connections before starting migration;
/// this is not a distributed lease or permission boundary against manual SQL.
pub async fn normal_runtime_guard(pool: &PgPool) -> anyhow::Result<PgConnection> {
    let mut connection = pool.acquire().await?.detach();
    sqlx::query(
        "SELECT pg_advisory_lock_shared(hashtextextended('dcl-one/migration-follower', 0))",
    )
    .execute(&mut connection)
    .await?;
    for table in [
        "migration_sync.streams",
        "migration_sync.player_streams",
        "migration_sync.worlds_streams",
    ] {
        let exists: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
            .bind(table)
            .fetch_one(&mut connection)
            .await?;
        if exists {
            // The table names are fixed repository-owned strings, not operator input.
            let sql = match table {
                "migration_sync.streams" => "SELECT EXISTS(SELECT 1 FROM migration_sync.streams)",
                "migration_sync.player_streams" => {
                    "SELECT EXISTS(SELECT 1 FROM migration_sync.player_streams)"
                }
                _ => "SELECT EXISTS(SELECT 1 FROM migration_sync.worlds_streams)",
            };
            let registered: bool = sqlx::query_scalar(sql).fetch_one(&mut connection).await?;
            anyhow::ensure!(
                !registered,
                "normal runtime forbidden on a migration follower database; retire migration ownership before cutover"
            );
        }
    }
    Ok(connection)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Connection;

    #[tokio::test]
    async fn normal_runtime_and_migration_registration_are_mutually_exclusive() {
        let Some(pool) = crate::db::scratch::isolated_pool(3).await else {
            return;
        };
        let guard = normal_runtime_guard(&pool).await.unwrap();
        let mut transaction = pool.begin().await.unwrap();
        let admitted: bool = sqlx::query_scalar(
            "SELECT pg_try_advisory_xact_lock(hashtextextended('dcl-one/migration-follower', 0))",
        )
        .fetch_one(&mut *transaction)
        .await
        .unwrap();
        assert!(!admitted);
        transaction.rollback().await.unwrap();
        guard.close().await.unwrap();
        let mut transaction = pool.begin().await.unwrap();
        let admitted: bool = sqlx::query_scalar(
            "SELECT pg_try_advisory_xact_lock(hashtextextended('dcl-one/migration-follower', 0))",
        )
        .fetch_one(&mut *transaction)
        .await
        .unwrap();
        assert!(admitted);
        sqlx::query("INSERT INTO migration_sync.streams(name,source,generation,baseline_lsn,applied_lsn) VALUES ('places','places',$1,'0/0','0/0')")
            .bind(Uuid::new_v4()).execute(&mut *transaction).await.unwrap();
        transaction.commit().await.unwrap();
        // Paused still means migration ownership, not permission to run jobs.
        assert!(normal_runtime_guard(&pool).await.is_err());
    }
    #[tokio::test]
    async fn player_and_worlds_registration_each_fence_normal_runtime() {
        let Some(pool) = crate::db::scratch::isolated_pool(3).await else {
            return;
        };
        sqlx::query("INSERT INTO migration_sync.player_streams(name,generation,baseline,captured,applied) VALUES('player',$1,0,0,0)").bind(Uuid::new_v4()).execute(&pool).await.unwrap();
        assert!(normal_runtime_guard(&pool).await.is_err());
        sqlx::query("DELETE FROM migration_sync.player_streams")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO migration_sync.worlds_streams(name,generation,baseline_position,captured_position,applied_position) VALUES('worlds',$1,0,0,0)").bind(Uuid::new_v4()).execute(&pool).await.unwrap();
        assert!(normal_runtime_guard(&pool).await.is_err());
    }
}
