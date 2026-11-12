//! End-to-end independent source streams into one isolated target.
use anyhow::{Context, Result};
use dcl_migration_sync::{
    domain_capture::{self, Domain},
    model::Envelope,
    store::{self, Outcome},
};
use serde_json::json;
use sqlx::{Connection, Executor, PgConnection, PgPool, postgres::PgConnectOptions};
use std::{path::Path, str::FromStr};
use uuid::Uuid;
const ID: &str = "11111111-1111-4111-8111-111111111111";
const WALLET: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

async fn assertions(events: &PgPool, social: &PgPool, target: &PgPool) -> Result<()> {
    sqlx::raw_sql("CREATE TABLE public.schedule(id uuid PRIMARY KEY,name varchar(50) NOT NULL,description varchar(255),image varchar(255),theme varchar(25),background varchar(30)[] NOT NULL,active_since timestamptz NOT NULL DEFAULT now(),active_until timestamptz NOT NULL DEFAULT now(),active boolean NOT NULL DEFAULT false,created_at timestamptz NOT NULL DEFAULT now(),updated_at timestamptz NOT NULL DEFAULT now())").execute(events).await?;
    sqlx::raw_sql("CREATE TABLE public.social_settings(address varchar(42) PRIMARY KEY,private_messages_privacy varchar NOT NULL DEFAULT 'only_friends',blocked_users_messages_visibility varchar NOT NULL DEFAULT 'show_messages',show_situation_reactions varchar NOT NULL DEFAULT 'show')").execute(social).await?;
    let mut files = std::fs::read_dir(Path::new(env!("DCL_ONE_ROOT")).join("schemas"))?
        .collect::<std::io::Result<Vec<_>>>()?;
    files.sort_by_key(|file| file.file_name());
    for file in files {
        if file.path().extension().is_some_and(|ext| ext == "sql") {
            sqlx::raw_sql(sqlx::AssertSqlSafe(std::fs::read_to_string(file.path())?))
                .execute(target)
                .await?;
        }
    }
    let event_scope = vec![ID.into()];
    let social_scope = vec![WALLET.into()];
    domain_capture::install(events, Domain::Events, &event_scope).await?;
    domain_capture::install(social, Domain::SocialSettings, &social_scope).await?;
    domain_capture::install(events, Domain::Events, &event_scope).await?;
    assert!(
        domain_capture::install(social, Domain::SocialSettings, &["invalid".into()])
            .await
            .is_err()
    );
    let mut writer = events.begin().await?;
    sqlx::query("INSERT INTO schedule(id,name,background) VALUES($1,'Initial',ARRAY['blue'])")
        .bind(Uuid::parse_str(ID)?)
        .execute(&mut *writer)
        .await?;
    let source_copy = events.clone();
    let target_copy = target.clone();
    let snapshot = tokio::spawn(async move {
        domain_capture::bootstrap(&source_copy, &target_copy, Domain::Events, "event-pilot").await
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(
        !snapshot.is_finished(),
        "baseline must wait for the earlier source transaction"
    );
    writer.commit().await?;
    let event_baseline = snapshot.await??;
    assert_eq!(event_baseline.position, 1);
    let social_baseline =
        domain_capture::bootstrap(social, target, Domain::SocialSettings, "social-pilot").await?;
    assert!(
        domain_capture::bootstrap(events, target, Domain::Events, "other-event")
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM migration_sync.streams")
            .fetch_one(target)
            .await?,
        2
    );
    // An absent social row remains absent through bootstrap, preserving fallback semantics.
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM social.user_settings")
            .fetch_one(target)
            .await?,
        0
    );
    store::pause(target, "event-pilot", false).await?;
    store::pause(target, "social-pilot", false).await?;
    assert_eq!(
        domain_capture::reconcile(events, target, Domain::Events, "event-pilot").await?["matches"],
        true
    );
    assert_eq!(
        domain_capture::reconcile(social, target, Domain::SocialSettings, "social-pilot").await?["matches"],
        true
    );
    // Paused target application does not prevent durable source capture.
    store::pause(target, "event-pilot", true).await?;
    sqlx::query("UPDATE schedule SET name='Changed' WHERE id=$1")
        .bind(Uuid::parse_str(ID)?)
        .execute(events)
        .await?;
    assert!(domain_capture::pump_once(events, target, Domain::Events, "event-pilot").await?);
    assert_eq!(
        store::process_next(target, "event-pilot").await?,
        Outcome::Idle
    );
    assert!(
        domain_capture::reconcile(events, target, Domain::Events, "event-pilot")
            .await
            .is_err()
    );
    sqlx::query("INSERT INTO social_settings(address,private_messages_privacy) VALUES($1,'all')")
        .bind(WALLET)
        .execute(social)
        .await?;
    assert!(
        domain_capture::pump_once(social, target, Domain::SocialSettings, "social-pilot").await?
    );
    assert_eq!(
        store::process_next(target, "social-pilot").await?,
        Outcome::Applied
    );
    store::pause(target, "event-pilot", false).await?;
    assert_eq!(
        store::process_next(target, "event-pilot").await?,
        Outcome::Applied
    );
    assert!(!domain_capture::pump_once(events, target, Domain::Events, "event-pilot").await?);
    for (source, domain, stream) in [
        (events, Domain::Events, "event-pilot"),
        (social, Domain::SocialSettings, "social-pilot"),
    ] {
        assert_eq!(
            domain_capture::reconcile(source, target, domain, stream).await?["matches"],
            true
        );
    }
    // Payload source and declared stream identity cannot redirect writes.
    let wrong: Envelope = serde_json::from_value(
        json!({"version":1,"source":"events","stream":"social-pilot","generation":social_baseline.generation,"previous_lsn":"0/1","commit_lsn":"0/2","mode":"source_only","changes":[{"op":"schedule_delete","id":ID}]}),
    )?;
    assert!(store::enqueue(target, &wrong).await.is_err());
    let escaped: Envelope = serde_json::from_value(
        json!({"version":1,"source":"events","stream":"event-pilot","generation":event_baseline.generation,"previous_lsn":"0/2","commit_lsn":"0/3","mode":"source_only","changes":[{"op":"schedule_delete","id":Uuid::new_v4()}]}),
    )?;
    assert!(store::enqueue(target, &escaped).await.is_err());
    let mixed: Envelope = serde_json::from_value(
        json!({"version":1,"source":"events","stream":"event-pilot","generation":event_baseline.generation,"previous_lsn":"0/2","commit_lsn":"0/3","mode":"source_only","changes":[{"op":"social_settings_delete","address":WALLET}]}),
    )?;
    assert!(mixed.validate().is_err());
    sqlx::query("DELETE FROM schedule").execute(events).await?;
    sqlx::query("DELETE FROM social_settings")
        .execute(social)
        .await?;
    for (source, domain, stream) in [
        (events, Domain::Events, "event-pilot"),
        (social, Domain::SocialSettings, "social-pilot"),
    ] {
        assert!(domain_capture::pump_once(source, target, domain, stream).await?);
        assert_eq!(store::process_next(target, stream).await?, Outcome::Applied);
        assert_eq!(
            domain_capture::reconcile(source, target, domain, stream).await?["matches"],
            true
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM migration_sync.domain_versions WHERE deleted"
        )
        .fetch_one(target)
        .await?,
        2
    );
    // Deletion leaves no target settings row, preserving absent-row semantics.
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM social.user_settings")
            .fetch_one(target)
            .await?,
        0
    );
    // A large retained transaction fails before loading payloads or advancing its offset.
    sqlx::query("INSERT INTO schedule(id,name,background) VALUES($1,'Large',ARRAY(SELECT repeat('x',30)::varchar(30) FROM generate_series(1,40000)))")
        .bind(Uuid::parse_str(ID)?).execute(events).await?;
    assert!(
        domain_capture::pump_once(events, target, Domain::Events, "event-pilot")
            .await
            .is_err()
    );
    assert_eq!(
        store::status(target, "event-pilot").await?["applied_lsn"],
        "0/3"
    );
    sqlx::query("UPDATE migration_capture_events.control SET generation=$1")
        .bind(Uuid::new_v4())
        .execute(events)
        .await?;
    assert!(
        domain_capture::pump_once(events, target, Domain::Events, "event-pilot")
            .await
            .is_err()
    );
    assert!(
        domain_capture::pump_once(social, target, Domain::Events, "social-pilot")
            .await
            .is_err()
    );
    Ok(())
}
#[tokio::test]
async fn independent_domains_share_target_without_sharing_authority() -> Result<()> {
    let url = match std::env::var("MIGRATION_SYNC_TEST_DATABASE_URL") {
        Ok(url) => url,
        Err(_) if std::env::var_os("CI").is_some() => {
            anyhow::bail!("CI requires MIGRATION_SYNC_TEST_DATABASE_URL")
        }
        Err(_) => {
            eprintln!("domain PG test skipped: set MIGRATION_SYNC_TEST_DATABASE_URL");
            return Ok(());
        }
    };
    let options = PgConnectOptions::from_str(&url)?;
    let mut admin = PgConnection::connect_with(&options).await?;
    let mut databases = Vec::new();
    let mut pools = Vec::new();
    for _ in 0..3 {
        let name = format!("migration_domains_{}", Uuid::new_v4().simple());
        admin
            .execute(sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                "CREATE DATABASE {name}"
            ))))
            .await?;
        pools.push(PgPool::connect_with(options.clone().database(&name)).await?);
        databases.push(name);
    }
    let result = assertions(&pools[0], &pools[1], &pools[2])
        .await
        .context("independent source domain assertions");
    for pool in pools {
        pool.close().await;
    }
    for name in databases {
        admin
            .execute(sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                "DROP DATABASE {name} WITH (FORCE)"
            ))))
            .await?;
    }
    result
}
