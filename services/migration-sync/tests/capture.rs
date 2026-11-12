//! Snapshot migration followed by ordered native commands, never row fallback.
use anyhow::Result;
use dcl_migration_sync::{capture, store};
use sqlx::{Connection, Executor, PgConnection, PgPool, postgres::PgConnectOptions};
use std::{path::Path, str::FromStr};
use uuid::Uuid;
const USER: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const PLACE: &str = "a1111111-1111-4111-8111-111111111111";

async fn assertions(source: &PgPool, target: &PgPool, noncanonical: bool) -> Result<()> {
    source.execute(sqlx::raw_sql("CREATE TABLE places(id char(36) PRIMARY KEY,world boolean NOT NULL DEFAULT false,disabled boolean NOT NULL DEFAULT false,favorites integer NOT NULL DEFAULT 0); CREATE TABLE user_favorites(\"user\" char(42),entity_id char(36),user_activity integer NOT NULL,created_at timestamptz NOT NULL DEFAULT now(),PRIMARY KEY(\"user\",entity_id)); INSERT INTO places(id) VALUES('a1111111-1111-4111-8111-111111111111');")).await?;
    let mut files = std::fs::read_dir(Path::new(env!("DCL_ONE_ROOT")).join("schemas"))?
        .collect::<std::io::Result<Vec<_>>>()?;
    files.sort_by_key(|e| e.file_name());
    for file in files {
        if file.path().extension().is_some_and(|e| e == "sql") {
            target
                .execute(sqlx::raw_sql(sqlx::AssertSqlSafe(std::fs::read_to_string(
                    file.path(),
                )?)))
                .await?;
        }
    }
    sqlx::query(
        "INSERT INTO explore.place(id,base_position,raw,origin) VALUES($1,'0,0','{}','local')",
    )
    .bind(PLACE)
    .execute(target)
    .await?;
    let id = Uuid::parse_str(PLACE)?;
    capture::install(source, &[id]).await?;
    capture::install(source, &[id]).await?;
    assert!(capture::install(source, &[Uuid::new_v4()]).await.is_err());
    if noncanonical {
        sqlx::query("INSERT INTO user_favorites VALUES($1,$2,1,now())")
            .bind(USER)
            .bind(PLACE.to_uppercase())
            .execute(source)
            .await?;
        assert!(capture::bootstrap(source, target, "pilot").await.is_err());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM migration_sync.streams")
                .fetch_one(target)
                .await?,
            0
        );
        return Ok(());
    }
    // Non-UUID IDs exist in the same source table but are outside this scope.
    sqlx::query("INSERT INTO user_favorites VALUES($1,'outside.dcl.eth',1,now())")
        .bind(USER)
        .execute(source)
        .await?;
    assert_eq!(capture::source_info(source).await?.position, 0);
    let mut first = source.begin().await?;
    sqlx::query("INSERT INTO user_favorites VALUES($1,$2,3,now())")
        .bind(USER)
        .bind(PLACE)
        .execute(&mut *first)
        .await?;
    let src = source.clone();
    let dst = target.clone();
    let baseline = tokio::spawn(async move { capture::bootstrap(&src, &dst, "pilot").await });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(!baseline.is_finished());
    first.commit().await?;
    assert_eq!(baseline.await??.position, 1);
    assert!(capture::bootstrap(source, target, "pilot").await.is_err());
    store::pause(target, "pilot", false).await?;
    assert_eq!(
        capture::reconcile(source, target, "pilot").await?["matches"],
        true
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM explore.user_favorites")
            .fetch_one(target)
            .await?,
        1
    );
    let mut rolled_back = source.begin().await?;
    sqlx::query("DELETE FROM user_favorites WHERE entity_id=$1")
        .bind(PLACE)
        .execute(&mut *rolled_back)
        .await?;
    rolled_back.rollback().await?;
    assert!(!capture::pump_once(source, target, "pilot").await?);

    // Source acceptance captures an equivalent command; only native target
    // execution is permitted after the one-time baseline migration.
    let call = "SELECT migration_capture.execute_favorite($1,$2,$3,$4,false,3.0)";
    let operation = Uuid::new_v4();
    let response: serde_json::Value = sqlx::query_scalar(call)
        .bind(operation)
        .bind("a".repeat(64))
        .bind(USER)
        .bind(id)
        .fetch_one(source)
        .await?;
    let duplicate: serde_json::Value = sqlx::query_scalar(call)
        .bind(operation)
        .bind("a".repeat(64))
        .bind(USER)
        .bind(id)
        .fetch_one(source)
        .await?;
    assert_eq!(response, duplicate);
    assert!(
        sqlx::query_scalar::<_, serde_json::Value>(call)
            .bind(operation)
            .bind("b".repeat(64))
            .bind(USER)
            .bind(id)
            .fetch_one(source)
            .await
            .is_err()
    );
    assert!(capture::pump_once(source, target, "pilot").await?);
    assert_eq!(
        store::process_next(target, "pilot").await?,
        store::Outcome::AwaitingCommand
    );
    let pointer = catalyst_explore::migration_replay::ReplayRequest {
        stream: "pilot".into(),
        generation: capture::source_info(source).await?.generation,
        commit_lsn: "0/2".into(),
    };
    catalyst_explore::migration_replay::replay(target, &pointer).await?;
    assert_eq!(
        store::process_next(target, "pilot").await?,
        store::Outcome::Applied
    );
    assert_eq!(
        capture::reconcile(source, target, "pilot").await?["matches"],
        true
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM explore.user_favorites")
            .fetch_one(target)
            .await?,
        0
    );

    // A direct writer is observable but is not approved for selective DB sync.
    sqlx::query("INSERT INTO user_favorites VALUES($1,$2,9,now())")
        .bind(USER)
        .bind(PLACE)
        .execute(source)
        .await?;
    assert!(capture::pump_once(source, target, "pilot").await?);
    assert_eq!(
        store::process_next(target, "pilot").await?,
        store::Outcome::Blocked
    );
    assert_eq!(
        store::status(target, "pilot").await?["head_error"],
        "unreceipted_mutation"
    );
    assert_eq!(store::status(target, "pilot").await?["applied_lsn"], "0/2");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM explore.user_favorites")
            .fetch_one(target)
            .await?,
        0
    );
    // Later accepted commands are retained but cannot overtake unknown effects.
    sqlx::query_scalar::<_, serde_json::Value>(call)
        .bind(Uuid::new_v4())
        .bind("c".repeat(64))
        .bind(USER)
        .bind(id)
        .fetch_one(source)
        .await?;
    assert!(capture::pump_once(source, target, "pilot").await?);
    assert!(
        catalyst_explore::migration_replay::replay(
            target,
            &catalyst_explore::migration_replay::ReplayRequest {
                commit_lsn: "0/4".into(),
                ..pointer.clone()
            }
        )
        .await
        .is_err()
    );
    store::pause(target, "pilot", true).await?;
    dcl_migration_sync::ops::retry_blocked(target, "pilot", "0/3").await?;
    store::pause(target, "pilot", false).await?;
    assert_eq!(
        store::process_next(target, "pilot").await?,
        store::Outcome::Blocked
    );
    assert_eq!(store::status(target, "pilot").await?["applied_lsn"], "0/2");
    assert!(
        sqlx::query("TRUNCATE user_favorites")
            .execute(source)
            .await
            .is_err()
    );
    // Capture remains read-only and detects drift instead of acknowledging it.
    assert!(!capture::pump_once(source, target, "pilot").await?);
    let generation = pointer.generation;
    sqlx::query("UPDATE migration_capture.control SET generation=$1")
        .bind(Uuid::new_v4())
        .execute(source)
        .await?;
    assert!(capture::pump_once(source, target, "pilot").await.is_err());
    sqlx::query("UPDATE migration_capture.control SET generation=$1")
        .bind(generation)
        .execute(source)
        .await?;
    sqlx::query("INSERT INTO migration_capture.scope VALUES($1)")
        .bind(Uuid::new_v4())
        .execute(source)
        .await?;
    assert!(capture::pump_once(source, target, "pilot").await.is_err());
    Ok(())
}
#[tokio::test]
async fn captures_source_transactions_and_bootstraps_consistently() -> Result<()> {
    let Ok(url) = std::env::var("MIGRATION_SYNC_TEST_DATABASE_URL") else {
        assert!(
            std::env::var_os("CI").is_none(),
            "CI requires MIGRATION_SYNC_TEST_DATABASE_URL"
        );
        return Ok(());
    };
    run_fixture(&url, false).await?;
    run_fixture(&url, true).await
}

async fn run_fixture(url: &str, noncanonical: bool) -> Result<()> {
    let options = PgConnectOptions::from_str(url)?;
    let mut admin = PgConnection::connect_with(&options).await?;
    let names = [
        format!("capture_source_{}", Uuid::new_v4().simple()),
        format!("capture_target_{}", Uuid::new_v4().simple()),
    ];
    for name in &names {
        admin
            .execute(sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                "CREATE DATABASE {name}"
            ))))
            .await?;
    }
    let source = PgPool::connect_with(options.clone().database(&names[0])).await?;
    let target = PgPool::connect_with(options.database(&names[1])).await?;
    let result = assertions(&source, &target, noncanonical).await;
    source.close().await;
    target.close().await;
    for name in &names {
        admin
            .execute(sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                "DROP DATABASE {name} WITH (FORCE)"
            ))))
            .await?;
    }
    result
}
