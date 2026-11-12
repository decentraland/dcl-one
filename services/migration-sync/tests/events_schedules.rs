//! Real source schema capture and target schedule projection; no Events HTTP server.
use anyhow::{Context, Result};
use dcl_migration_sync::events_schedules::{self, ScheduleChange};
use serde_json::Value;
use sqlx::{Connection, Executor, PgConnection, PgPool, postgres::PgConnectOptions};
use std::{path::Path, str::FromStr};
use uuid::Uuid;

const ID: &str = "11111111-1111-4111-8111-111111111111";
const OTHER: &str = "22222222-2222-4222-8222-222222222222";

async fn assertions(source: &PgPool, target: &PgPool) -> Result<()> {
    // Column types/nullability from Events migrations at 770e43f8.
    sqlx::raw_sql("CREATE TABLE public.schedule(id uuid PRIMARY KEY,name varchar(50) NOT NULL,description varchar(255),image varchar(255),theme varchar(25),background varchar(30)[] NOT NULL,active_since timestamptz NOT NULL DEFAULT now(),active_until timestamptz NOT NULL DEFAULT now(),active boolean NOT NULL DEFAULT false,created_at timestamptz NOT NULL DEFAULT now(),updated_at timestamptz NOT NULL DEFAULT now())")
        .execute(source).await?;
    sqlx::raw_sql(events_schedules::SOURCE_SQL)
        .execute(source)
        .await?;
    sqlx::query("INSERT INTO migration_capture_events.scope VALUES($1),($2)")
        .bind(Uuid::parse_str(ID)?)
        .bind(Uuid::parse_str(OTHER)?)
        .execute(source)
        .await?;
    let mut files = std::fs::read_dir(Path::new(env!("DCL_ONE_ROOT")).join("schemas"))?
        .collect::<std::io::Result<Vec<_>>>()?;
    files.sort_by_key(|f| f.file_name());
    for file in files {
        if file.path().extension().is_some_and(|ext| ext == "sql") {
            sqlx::raw_sql(sqlx::AssertSqlSafe(std::fs::read_to_string(file.path())?))
                .execute(target)
                .await?;
        }
    }
    sqlx::query("INSERT INTO public.schedule(id,name,description,image,theme,background,active_since,active_until,active,created_at,updated_at) VALUES($1,'Schedule',NULL,NULL,NULL,ARRAY['red','blue'],'2026-01-01T10:00:00+02:00','2027-01-01T10:00:00+02:00',true,'2025-01-01T00:00:00Z','2025-01-01T00:00:00Z')")
        .bind(Uuid::parse_str(ID)?).execute(source).await?;
    let initial: Value =
        sqlx::query_scalar("SELECT payload FROM migration_capture_events.changes WHERE position=1")
            .fetch_one(source)
            .await?;
    let initial: ScheduleChange = serde_json::from_value(initial)?;
    initial.validate()?;
    let mut tx = target.begin().await?;
    events_schedules::apply(&mut tx, &initial).await?;
    tx.commit().await?;
    assert_eq!(
        events_schedules::read_snapshot(&mut *source.acquire().await?).await?,
        events_schedules::read_target(&mut *target.acquire().await?, &[ID.into(), OTHER.into()])
            .await?
    );
    // Reject unbounded arrays at the database preflight, before row decoding.
    // Roll back so these malformed fixtures do not change the capture sequence.
    let mut tx = source.begin().await?;
    sqlx::query("UPDATE public.schedule SET background=array_fill('x'::varchar(30), ARRAY[1001]) WHERE id=$1")
        .bind(Uuid::parse_str(ID)?).execute(&mut *tx).await?;
    assert!(
        events_schedules::read_snapshot(&mut tx)
            .await
            .unwrap_err()
            .to_string()
            .contains("source background exceeds bounded array")
    );
    tx.rollback().await?;
    for background in [
        serde_json::json!(vec!["x"; 1001]),
        serde_json::json!({"invalid":"shape"}),
        serde_json::json!(["x".repeat(1048577)]),
    ] {
        let mut tx = target.begin().await?;
        sqlx::query("UPDATE explore.schedules_local SET background=$2 WHERE id=$1")
            .bind(ID)
            .bind(background)
            .execute(&mut *tx)
            .await?;
        assert!(
            events_schedules::read_target(&mut tx, &[ID.into()])
                .await
                .unwrap_err()
                .to_string()
                .contains("target background exceeds bounded array or has invalid shape")
        );
        tx.rollback().await?;
    }
    // PATCH can leave updated_at unchanged; source commit order still wins.
    sqlx::query("UPDATE public.schedule SET name='Renamed',active=false,theme='future-theme',background=ARRAY[]::varchar(30)[] WHERE id=$1")
        .bind(Uuid::parse_str(ID)?).execute(source).await?;
    let changed: Value =
        sqlx::query_scalar("SELECT payload FROM migration_capture_events.changes WHERE position=2")
            .fetch_one(source)
            .await?;
    let changed: ScheduleChange = serde_json::from_value(changed)?;
    let mut tx = target.begin().await?;
    events_schedules::apply(&mut tx, &changed).await?;
    tx.commit().await?;
    let target_rows =
        events_schedules::read_target(&mut *target.acquire().await?, &[ID.into()]).await?;
    assert_eq!(target_rows[0].name, "Renamed");
    assert!(!target_rows[0].active);
    assert_eq!(target_rows[0].theme.as_deref(), Some("future-theme"));
    assert_eq!(
        target_rows,
        events_schedules::read_snapshot(&mut *source.acquire().await?).await?
    );
    // A rollback cannot publish outbox progress or a deletion.
    let mut tx = source.begin().await?;
    sqlx::query("DELETE FROM public.schedule")
        .execute(&mut *tx)
        .await?;
    tx.rollback().await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT position FROM migration_capture_events.control")
            .fetch_one(source)
            .await?,
        2
    );
    // Key changes retain old-key deletion and new-key insertion in one source tx.
    sqlx::query("UPDATE public.schedule SET id=$2 WHERE id=$1")
        .bind(Uuid::parse_str(ID)?)
        .bind(Uuid::parse_str(OTHER)?)
        .execute(source)
        .await?;
    let changes: Vec<Value> = sqlx::query_scalar(
        "SELECT payload FROM migration_capture_events.changes WHERE position=3 ORDER BY ordinal",
    )
    .fetch_all(source)
    .await?;
    assert_eq!(changes.len(), 2);
    let mut tx = target.begin().await?;
    for change in changes {
        events_schedules::apply(&mut tx, &serde_json::from_value(change)?).await?;
    }
    tx.commit().await?;
    assert_eq!(
        events_schedules::read_target(&mut *target.acquire().await?, &[ID.into(), OTHER.into()])
            .await?,
        events_schedules::read_snapshot(&mut *source.acquire().await?).await?
    );
    assert!(
        sqlx::query("TRUNCATE public.schedule")
            .execute(source)
            .await
            .is_err()
    );
    sqlx::query("DELETE FROM public.schedule")
        .execute(source)
        .await?;
    let deleted: Value =
        sqlx::query_scalar("SELECT payload FROM migration_capture_events.changes WHERE position=4")
            .fetch_one(source)
            .await?;
    let deleted = serde_json::from_value(deleted)?;
    let mut tx = target.begin().await?;
    events_schedules::apply(&mut tx, &deleted).await?;
    events_schedules::apply(&mut tx, &deleted).await?;
    tx.commit().await?;
    assert!(
        events_schedules::read_target(&mut *target.acquire().await?, &[ID.into(), OTHER.into()])
            .await?
            .is_empty()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM social.event_notification_intents")
            .fetch_one(target)
            .await?,
        0
    );
    Ok(())
}

#[tokio::test]
async fn source_schedule_capture_preserves_committed_facts_without_effects() -> Result<()> {
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
    let source_name = format!("migration_event_source_{}", Uuid::new_v4().simple());
    let target_name = format!("migration_event_target_{}", Uuid::new_v4().simple());
    for name in [&source_name, &target_name] {
        admin
            .execute(sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                "CREATE DATABASE {name}"
            ))))
            .await?;
    }
    let source = PgPool::connect_with(options.clone().database(&source_name)).await?;
    let target = PgPool::connect_with(options.database(&target_name)).await?;
    let result = assertions(&source, &target)
        .await
        .context("schedule capture/projection assertions");
    source.close().await;
    target.close().await;
    for name in [&source_name, &target_name] {
        admin
            .execute(sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                "DROP DATABASE {name} WITH (FORCE)"
            ))))
            .await?;
    }
    result
}
