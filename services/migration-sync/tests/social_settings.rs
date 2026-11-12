//! Actual source SQL capture and mapped target facts, without running social APIs.
use anyhow::{Context, Result};
use dcl_migration_sync::social_settings::{self, Change};
use serde_json::Value;
use sqlx::{Connection, Executor, PgConnection, PgPool, postgres::PgConnectOptions};
use std::str::FromStr;
use uuid::Uuid;
const ADDRESS: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

async fn captured(source: &PgPool, position: i64) -> Result<Vec<Change>> {
    let payloads:Vec<Value>=sqlx::query_scalar("SELECT payload FROM migration_capture_social_settings.changes WHERE position=$1 ORDER BY ordinal")
        .bind(position).fetch_all(source).await?;
    payloads
        .into_iter()
        .map(|value| {
            let change: Change = serde_json::from_value(value)?;
            change.validate()?;
            Ok(change)
        })
        .collect()
}
async fn assertions(source: &PgPool, target: &PgPool) -> Result<()> {
    sqlx::raw_sql("CREATE TABLE public.social_settings(address varchar(42) PRIMARY KEY,private_messages_privacy varchar NOT NULL DEFAULT 'only_friends' CHECK(private_messages_privacy IN ('only_friends','all')),blocked_users_messages_visibility varchar NOT NULL DEFAULT 'show_messages' CHECK(blocked_users_messages_visibility IN ('show_messages','do_not_show_messages')),show_situation_reactions varchar NOT NULL DEFAULT 'show' CHECK(show_situation_reactions IN ('show','hide')))").execute(source).await?;
    sqlx::raw_sql(social_settings::SOURCE_SQL)
        .execute(source)
        .await?;
    sqlx::query("INSERT INTO migration_capture_social_settings.scope(address) VALUES($1)")
        .bind(ADDRESS)
        .execute(source)
        .await?;
    sqlx::raw_sql("CREATE SCHEMA social; CREATE TABLE social.user_settings(address text PRIMARY KEY,private_messages_privacy text NOT NULL DEFAULT 'only_friends' CHECK(private_messages_privacy IN ('only_friends','all')),blocked_users_messages_visibility text NOT NULL DEFAULT 'show_messages' CHECK(blocked_users_messages_visibility IN ('show_messages','do_not_show_messages')),show_situation_reactions text NOT NULL DEFAULT 'show' CHECK(show_situation_reactions IN ('show','hide')),updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC',now()))").execute(target).await?;
    let scope = vec![ADDRESS.to_owned()];
    let mut src = source.acquire().await?;
    let mut dst = target.acquire().await?;
    // Absence is distinct from a stored row containing the source DB defaults.
    assert!(social_settings::read_snapshot(&mut src).await?.is_empty());
    assert!(
        social_settings::canonical_target(&mut dst, &scope)
            .await?
            .is_empty()
    );
    sqlx::query("INSERT INTO public.social_settings(address) VALUES($1)")
        .bind(ADDRESS)
        .execute(source)
        .await?;
    let changes = captured(source, 1).await?;
    assert_eq!(changes.len(), 1);
    for change in &changes {
        social_settings::apply(&mut dst, change).await?;
    }
    let baseline = social_settings::read_snapshot(&mut src).await?;
    assert_eq!(
        serde_json::to_value(&baseline)?,
        serde_json::to_value(social_settings::canonical_target(&mut dst, &scope).await?)?
    );
    let before: chrono::NaiveDateTime =
        sqlx::query_scalar("SELECT updated_at FROM social.user_settings WHERE address=$1")
            .bind(ADDRESS)
            .fetch_one(&mut *dst)
            .await?;
    social_settings::apply(&mut dst, &changes[0]).await?;
    let after: chrono::NaiveDateTime =
        sqlx::query_scalar("SELECT updated_at FROM social.user_settings WHERE address=$1")
            .bind(ADDRESS)
            .fetch_one(&mut *dst)
            .await?;
    assert_eq!(
        before, after,
        "identical replay must not fabricate a newer observation"
    );
    let mut tx = source.begin().await?;
    sqlx::query(
        "UPDATE public.social_settings SET private_messages_privacy='all' WHERE address=$1",
    )
    .bind(ADDRESS)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE public.social_settings SET show_situation_reactions='hide',blocked_users_messages_visibility='do_not_show_messages' WHERE address=$1").bind(ADDRESS).execute(&mut *tx).await?;
    tx.commit().await?;
    let updates = captured(source, 2).await?;
    assert_eq!(
        updates.len(),
        2,
        "one committed source transaction retains both writes"
    );
    let mut apply = target.begin().await?;
    for change in &updates {
        social_settings::apply(&mut apply, change).await?;
    }
    apply.commit().await?;
    assert_eq!(
        serde_json::to_value(social_settings::read_snapshot(&mut src).await?)?,
        serde_json::to_value(social_settings::canonical_target(&mut dst, &scope).await?)?
    );
    let mut rolled_back = source.begin().await?;
    sqlx::query("DELETE FROM public.social_settings WHERE address=$1")
        .bind(ADDRESS)
        .execute(&mut *rolled_back)
        .await?;
    rolled_back.rollback().await?;
    assert!(captured(source, 3).await?.is_empty());
    sqlx::query("DELETE FROM public.social_settings WHERE address=$1")
        .bind(ADDRESS)
        .execute(source)
        .await?;
    let deletes = captured(source, 3).await?;
    assert_eq!(deletes.len(), 1);
    assert!(deletes[0].deleted());
    social_settings::apply(&mut dst, &deletes[0]).await?;
    assert!(
        social_settings::canonical_target(&mut dst, &scope)
            .await?
            .is_empty(),
        "delete preserves absent-row reader defaults"
    );
    assert!(
        sqlx::query("TRUNCATE public.social_settings")
            .execute(source)
            .await
            .is_err()
    );
    sqlx::query("INSERT INTO public.social_settings(address) VALUES('0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb')").execute(source).await?;
    assert!(
        captured(source, 4).await?.is_empty(),
        "unscoped source changes remain outside the stream"
    );
    sqlx::query("INSERT INTO public.social_settings(address) VALUES('0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA')").execute(source).await?;
    assert!(
        social_settings::read_snapshot(&mut src).await.is_err(),
        "case collisions are rejected instead of guessed"
    );
    assert!(captured(source, 4).await.is_err());
    Ok(())
}
#[tokio::test]
async fn settings_capture_preserves_values_absence_and_transaction_order() -> Result<()> {
    let url = match std::env::var("MIGRATION_SYNC_TEST_DATABASE_URL") {
        Ok(url) => url,
        Err(_) if std::env::var_os("CI").is_some() => {
            anyhow::bail!("CI requires MIGRATION_SYNC_TEST_DATABASE_URL")
        }
        Err(_) => {
            eprintln!(
                "set MIGRATION_SYNC_TEST_DATABASE_URL to run social settings PostgreSQL coverage"
            );
            return Ok(());
        }
    };
    let options = PgConnectOptions::from_str(&url)?;
    let mut admin = PgConnection::connect_with(&options).await?;
    let source_name = format!("migration_social_source_{}", Uuid::new_v4().simple());
    let target_name = format!("migration_social_target_{}", Uuid::new_v4().simple());
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
        .context("social settings capture/mapping scenario");
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
