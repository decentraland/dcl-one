//! Isolated PostgreSQL coverage of committed owner/quota capture and fencing.
use anyhow::{Context, Result};
use dcl_migration_sync::worlds::{self, Outcome};
use serde_json::Value;
use sqlx::{Connection, Executor, PgConnection, PgPool, postgres::PgConnectOptions};
use std::{path::Path, str::FromStr};
use uuid::Uuid;

const A: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const C: &str = "0xcccccccccccccccccccccccccccccccccccccccc";
const WORLD: &str = "pilot.dcl.eth";

async fn fixture(source: &PgPool, target: &PgPool) -> Result<()> {
    source.execute(sqlx::raw_sql("CREATE TABLE public.worlds(name text PRIMARY KEY, owner text, updated_at timestamp DEFAULT '2026-01-01'); CREATE TABLE public.blocked(wallet text PRIMARY KEY,created_at timestamp NOT NULL,updated_at timestamp NOT NULL);")).await?;
    target.execute(sqlx::raw_sql("CREATE SCHEMA worlds; CREATE TABLE worlds.worlds(name text PRIMARY KEY,owner text,mirror_source text); CREATE TABLE worlds.quota_blocks(wallet text PRIMARY KEY,created_at timestamp NOT NULL,updated_at timestamp NOT NULL); CREATE SCHEMA migration_sync;")).await?;
    let schema = std::fs::read_to_string(
        Path::new(env!("DCL_ONE_ROOT")).join("schemas/72_worlds_migration.sql"),
    )?;
    target
        .execute(sqlx::raw_sql(sqlx::AssertSqlSafe(schema)))
        .await?;
    for pool in [source, target] {
        let statement = if std::ptr::eq(pool, source) {
            "INSERT INTO public.worlds(name,owner) VALUES($1,$2)"
        } else {
            "INSERT INTO worlds.worlds(name,owner) VALUES($1,$2)"
        };
        sqlx::query(statement)
            .bind(WORLD)
            .bind(A)
            .execute(pool)
            .await?;
    }
    Ok(())
}
async fn install(source: &PgPool) -> Result<()> {
    worlds::install(source, &[WORLD.to_owned()], &[A.to_owned(), B.to_owned()]).await?;
    Ok(())
}
async fn start(source: &PgPool, target: &PgPool) -> Result<()> {
    install(source).await?;
    worlds::bootstrap(source, target, "worlds").await?;
    worlds::set_paused(target, "worlds", false, true).await
}
async fn owner(target: &PgPool) -> Result<String> {
    Ok(
        sqlx::query_scalar("SELECT owner FROM worlds.worlds WHERE name=$1")
            .bind(WORLD)
            .fetch_one(target)
            .await?,
    )
}
async fn deliver(source: &PgPool, target: &PgPool) -> Result<()> {
    assert!(worlds::pump_once(source, target, "worlds").await?);
    assert_eq!(
        worlds::apply_next(target, "worlds").await?,
        Outcome::Applied
    );
    Ok(())
}
async fn state(target: &PgPool) -> Result<Value> {
    worlds::status(target, "worlds").await
}

async fn behavior(source: &PgPool, target: &PgPool) -> Result<()> {
    start(source, target).await?;
    // Installation is repeatable only with exactly the same immutable scope.
    install(source).await?;
    assert!(
        worlds::install(source, &[WORLD.to_owned()], &[A.to_owned()])
            .await
            .is_err()
    );
    assert!(worlds::bootstrap(source, target, "worlds").await.is_err());
    assert_eq!(worlds::apply_next(target, "worlds").await?, Outcome::Idle);
    assert!(!worlds::pump_once(source, target, "worlds").await?);
    // Rollback cannot create a cursor gap or leaked fact.
    let mut tx = source.begin().await?;
    sqlx::query("UPDATE worlds SET owner=$1 WHERE name=$2")
        .bind(B)
        .bind(WORLD)
        .execute(&mut *tx)
        .await?;
    let mut contender = source.begin().await?;
    let admitted: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(179321,2)")
        .fetch_one(&mut *contender)
        .await?;
    assert!(!admitted, "source writes serialize before row mutations");
    assert!(
        !worlds::pump_once(source, target, "worlds").await?,
        "uncommitted source facts stay invisible"
    );
    contender.rollback().await?;
    tx.rollback().await?;
    assert!(!worlds::pump_once(source, target, "worlds").await?);
    // Two facts in a committed source transaction become visible atomically.
    let mut tx = source.begin().await?;
    sqlx::query("UPDATE worlds SET owner=$1 WHERE name=$2")
        .bind(B)
        .bind(WORLD)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO blocked VALUES($1,'2026-01-01','2026-01-01')")
        .bind(B)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    worlds::set_paused(target, "worlds", true, false).await?;
    assert!(worlds::pump_once(source, target, "worlds").await?);
    assert_eq!(worlds::apply_next(target, "worlds").await?, Outcome::Idle);
    assert_eq!(owner(target).await?, A);
    worlds::set_paused(target, "worlds", false, true).await?;
    assert_eq!(
        worlds::apply_next(target, "worlds").await?,
        Outcome::Applied
    );
    assert_eq!(owner(target).await?, B);
    assert_eq!(state(target).await?["applied_position"], 1);
    assert_eq!(worlds::apply_next(target, "worlds").await?, Outcome::Idle);
    assert!(!worlds::pump_once(source, target, "worlds").await?);
    // Refresh preserves original creation time; deletes and later episodes survive.
    sqlx::query("UPDATE blocked SET updated_at='2026-01-03' WHERE wallet=$1")
        .bind(B)
        .execute(source)
        .await?;
    deliver(source, target).await?;
    let times: (String, String) = sqlx::query_as(
        "SELECT created_at::text,updated_at::text FROM worlds.quota_blocks WHERE wallet=$1",
    )
    .bind(B)
    .fetch_one(target)
    .await?;
    assert_eq!(
        times,
        ("2026-01-01 00:00:00".into(), "2026-01-03 00:00:00".into())
    );
    sqlx::query("DELETE FROM blocked WHERE wallet=$1")
        .bind(B)
        .execute(source)
        .await?;
    deliver(source, target).await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM worlds.quota_blocks")
            .fetch_one(target)
            .await?,
        0
    );
    sqlx::query("INSERT INTO blocked VALUES($1,'2026-02-01','2026-02-01')")
        .bind(B)
        .execute(source)
        .await?;
    deliver(source, target).await?;
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT created_at::text FROM worlds.quota_blocks")
            .fetch_one(target)
            .await?,
        "2026-02-01 00:00:00"
    );
    // Unrelated fields, worlds and wallets never create residual events.
    source.execute(sqlx::raw_sql("UPDATE worlds SET updated_at='2026-03-01'; INSERT INTO worlds VALUES('outside.eth',NULL,now());")).await?;
    sqlx::query("INSERT INTO blocked VALUES($1,now(),now())")
        .bind(C)
        .execute(source)
        .await?;
    assert!(!worlds::pump_once(source, target, "worlds").await?);
    assert!(
        source
            .execute(sqlx::raw_sql("TRUNCATE blocked"))
            .await
            .is_err()
    );
    // A transfer outside the wallet allowlist blocks the whole ordered head.
    sqlx::query("UPDATE worlds SET owner=$1 WHERE name=$2")
        .bind(C)
        .bind(WORLD)
        .execute(source)
        .await?;
    assert!(worlds::pump_once(source, target, "worlds").await?);
    assert_eq!(
        worlds::apply_next(target, "worlds").await?,
        Outcome::Blocked
    );
    assert_eq!(owner(target).await?, B);
    assert_eq!(state(target).await?["applied_position"], 4);
    assert!(
        worlds::set_paused(target, "worlds", false, true)
            .await
            .is_err()
    );
    Ok(())
}
async fn conflict(source: &PgPool, target: &PgPool) -> Result<()> {
    start(source, target).await?;
    let mut tx = source.begin().await?;
    sqlx::query("UPDATE worlds SET owner=$1 WHERE name=$2")
        .bind(B)
        .bind(WORLD)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO blocked VALUES($1,'2026-01-01','2026-01-01')")
        .bind(B)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    sqlx::query("INSERT INTO worlds.quota_blocks VALUES($1,'2026-01-02','2026-01-02')")
        .bind(B)
        .execute(target)
        .await?;
    assert!(worlds::pump_once(source, target, "worlds").await?);
    assert_eq!(
        worlds::apply_next(target, "worlds").await?,
        Outcome::Blocked
    );
    assert_eq!(
        owner(target).await?,
        A,
        "earlier owner change must roll back with quota conflict"
    );
    assert_eq!(state(target).await?["applied_position"], 0);
    Ok(())
}
async fn identity(source: &PgPool, target: &PgPool) -> Result<()> {
    start(source, target).await?;
    sqlx::query("DELETE FROM worlds WHERE name=$1")
        .bind(WORLD)
        .execute(source)
        .await?;
    assert!(worlds::pump_once(source, target, "worlds").await?);
    assert_eq!(
        worlds::apply_next(target, "worlds").await?,
        Outcome::Blocked
    );
    assert_eq!(owner(target).await?, A);
    Ok(())
}
async fn lock_timeout(source: &PgPool, target: &PgPool) -> Result<()> {
    start(source, target).await?;
    sqlx::query("UPDATE worlds SET owner=$1 WHERE name=$2")
        .bind(B)
        .bind(WORLD)
        .execute(source)
        .await?;
    assert!(worlds::pump_once(source, target, "worlds").await?);
    let mut holder = target.begin().await?;
    sqlx::query("SELECT owner FROM worlds.worlds WHERE name=$1 FOR UPDATE")
        .bind(WORLD)
        .execute(&mut *holder)
        .await?;
    let options = target
        .connect_options()
        .as_ref()
        .clone()
        .options([("lock_timeout", "50ms")]);
    let limited = PgPool::connect_with(options).await?;
    let error = worlds::apply_next(&limited, "worlds")
        .await
        .expect_err("locked world must time out");
    assert!(error.downcast_ref::<sqlx::Error>().is_some());
    assert_eq!(state(target).await?["applied_position"], 0);
    assert!(state(target).await?["error_code"].is_null());
    assert_eq!(state(target).await?["paused"], false);
    holder.rollback().await?;
    assert_eq!(
        worlds::apply_next(&limited, "worlds").await?,
        Outcome::Applied
    );
    assert_eq!(owner(target).await?, B);
    limited.close().await;
    Ok(())
}

async fn fencing(source: &PgPool, target: &PgPool) -> Result<()> {
    install(source).await?;
    // An active normal-runtime guard prevents registration; no partial state.
    let mut guard = target.acquire().await?;
    sqlx::query("SELECT pg_advisory_lock_shared(hashtextextended('dcl-one/migration-follower',0))")
        .execute(&mut *guard)
        .await?;
    assert!(worlds::bootstrap(source, target, "worlds").await.is_err());
    sqlx::query(
        "SELECT pg_advisory_unlock_shared(hashtextextended('dcl-one/migration-follower',0))",
    )
    .execute(&mut *guard)
    .await?;
    drop(guard);
    sqlx::query("UPDATE worlds.worlds SET owner=$1")
        .bind(B)
        .execute(target)
        .await?;
    assert!(worlds::bootstrap(source, target, "worlds").await.is_err());
    sqlx::query("UPDATE worlds.worlds SET owner=$1")
        .bind(A)
        .execute(target)
        .await?;
    worlds::bootstrap(source, target, "worlds").await?;
    assert!(
        worlds::set_paused(target, "worlds", false, false)
            .await
            .is_err()
    );
    worlds::set_paused(target, "worlds", false, true).await?;
    sqlx::query("DELETE FROM migration_worlds_capture.wallet_scope WHERE wallet=$1")
        .bind(B)
        .execute(source)
        .await?;
    assert!(worlds::pump_once(source, target, "worlds").await.is_err());
    sqlx::query("INSERT INTO migration_worlds_capture.wallet_scope VALUES($1)")
        .bind(B)
        .execute(source)
        .await?;
    sqlx::query("UPDATE migration_worlds_capture.control SET generation=$1")
        .bind(Uuid::new_v4())
        .execute(source)
        .await?;
    assert!(worlds::pump_once(source, target, "worlds").await.is_err());
    assert_eq!(state(target).await?["captured_position"], 0);
    Ok(())
}

#[tokio::test]
async fn worlds_outcomes_preserve_commits_deletes_and_scope() -> Result<()> {
    let url = match std::env::var("MIGRATION_SYNC_TEST_DATABASE_URL") {
        Ok(url) => url,
        Err(_) if std::env::var_os("CI").is_some() => {
            anyhow::bail!("CI requires MIGRATION_SYNC_TEST_DATABASE_URL")
        }
        Err(_) => {
            eprintln!("PostgreSQL integration not run: set MIGRATION_SYNC_TEST_DATABASE_URL");
            return Ok(());
        }
    };
    let options = PgConnectOptions::from_str(&url)?;
    let mut admin = PgConnection::connect_with(&options).await?;
    for scenario in 0..5 {
        let source_db = format!("worlds_source_{}", Uuid::new_v4().simple());
        let target_db = format!("worlds_target_{}", Uuid::new_v4().simple());
        for name in [&source_db, &target_db] {
            admin
                .execute(sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                    "CREATE DATABASE {name}"
                ))))
                .await?;
        }
        let source = PgPool::connect_with(options.clone().database(&source_db)).await?;
        let target = PgPool::connect_with(options.clone().database(&target_db)).await?;
        let result = async {
            fixture(&source, &target).await.context("fixture")?;
            match scenario {
                0 => behavior(&source, &target).await,
                1 => conflict(&source, &target).await,
                2 => identity(&source, &target).await,
                3 => fencing(&source, &target).await,
                _ => lock_timeout(&source, &target).await,
            }
        }
        .await;
        source.close().await;
        target.close().await;
        for name in [&source_db, &target_db] {
            admin
                .execute(sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                    "DROP DATABASE {name} WITH (FORCE)"
                ))))
                .await?;
        }
        result.with_context(|| format!("scenario {scenario}"))?;
    }
    Ok(())
}
