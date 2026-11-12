//! Real PostgreSQL regression of ordering, atomicity, replay and fencing.
//! Requires an explicitly configured disposable server with CREATEDB permission.
use std::{path::Path, str::FromStr};

use anyhow::{Context, Result};
use dcl_migration_sync::{
    model::Envelope,
    store::{self, Outcome},
};
use serde_json::{Value, json};
use sqlx::{Connection, Executor, PgConnection, PgPool, Row, postgres::PgConnectOptions};
use uuid::Uuid;

const USER: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const PLACE: &str = "11111111-1111-4111-8111-111111111111";
const MISSING: &str = "22222222-2222-4222-8222-222222222222";

fn envelope(
    generation: Uuid,
    previous: &str,
    commit: &str,
    mode: &str,
    changes: Value,
) -> Envelope {
    serde_json::from_value(json!({
        "version": 1, "source": "places", "stream": "pilot", "generation": generation,
        "previous_lsn": previous, "commit_lsn": commit, "mode": mode, "changes": changes,
    }))
    .unwrap()
}

fn upsert(entity: &str) -> Value {
    json!({"op":"upsert", "user":USER, "entity_id":entity, "user_activity":3.0, "created_at":"2026-11-12T00:00:00Z"})
}

fn delete(entity: &str) -> Value {
    json!({"op":"delete", "user":USER, "entity_id":entity})
}

async fn fixture(pool: &PgPool) -> Result<()> {
    sqlx::query(
        "INSERT INTO explore.place(id,base_position,raw,origin) VALUES($1,'0,0','{}','local')",
    )
    .bind(PLACE)
    .execute(pool)
    .await?;
    sqlx::query("INSERT INTO explore.user_likes(\"user\",entity_id,\"like\") VALUES('positive',$1,true),('negative',$1,false)")
        .bind(PLACE).execute(pool).await?;
    Ok(())
}

async fn assertions(pool: &PgPool) -> Result<()> {
    fixture(pool).await?;
    let generation = Uuid::new_v4();
    assert!(
        store::register(pool, "pilot", Uuid::nil(), "0/10")
            .await
            .is_err()
    );
    store::register(pool, "pilot", generation, "0/10").await?;
    assert_eq!(store::status(pool, "pilot").await?["paused"], true);
    store::register(pool, "pilot", generation, "0/10").await?;
    assert!(
        store::register(pool, "another-stream", generation, "0/10")
            .await
            .is_err()
    );
    assert!(
        store::register(pool, "pilot", Uuid::new_v4(), "0/10")
            .await
            .is_err()
    );
    assert!(
        store::register(pool, "pilot", generation, "0/11")
            .await
            .is_err()
    );
    let old = envelope(
        generation,
        "0/0",
        "0/10",
        "source_only",
        json!([upsert(PLACE)]),
    );
    assert!(store::enqueue(pool, &old).await.is_err());
    let wrong_generation = envelope(
        Uuid::new_v4(),
        "0/10",
        "0/20",
        "source_only",
        json!([upsert(PLACE)]),
    );
    assert!(store::enqueue(pool, &wrong_generation).await.is_err());

    let first = envelope(
        generation,
        "0/10",
        "0/20",
        "source_only",
        json!([upsert(PLACE)]),
    );
    let second = envelope(
        generation,
        "0/20",
        "0/30",
        "source_only",
        json!([delete(PLACE)]),
    );
    // Out-of-order arrival is durable but cannot overtake a missing predecessor.
    assert!(store::enqueue(pool, &second).await?);
    // Neither a wider interval nor a different predecessor may strand a
    // retained transaction by advancing past it.
    let crossing = envelope(
        generation,
        "0/10",
        "0/40",
        "source_only",
        json!([upsert(PLACE)]),
    );
    assert!(store::enqueue(pool, &crossing).await.is_err());
    let contained = envelope(
        generation,
        "0/21",
        "0/25",
        "source_only",
        json!([upsert(PLACE)]),
    );
    assert!(store::enqueue(pool, &contained).await.is_err());

    store::pause(pool, "pilot", false).await?;
    assert_eq!(store::process_next(pool, "pilot").await?, Outcome::Idle);
    assert_eq!(store::status(pool, "pilot").await?["applied_lsn"], "0/10");
    assert!(store::enqueue(pool, &first).await?);
    assert!(!store::enqueue(pool, &first).await?);
    let conflicting = envelope(
        generation,
        "0/10",
        "0/20",
        "source_only",
        json!([delete(PLACE)]),
    );
    assert!(store::enqueue(pool, &conflicting).await.is_err());
    let fork = envelope(
        generation,
        "0/10",
        "0/25",
        "source_only",
        json!([upsert(PLACE)]),
    );
    assert!(store::enqueue(pool, &fork).await.is_err());
    store::pause(pool, "pilot", true).await?;
    assert_eq!(store::process_next(pool, "pilot").await?, Outcome::Idle);
    store::pause(pool, "pilot", false).await?;

    // Concurrent workers serialize on stream ownership, never on arrival order.
    let (a, b) = tokio::join!(
        store::process_next(pool, "pilot"),
        store::process_next(pool, "pilot")
    );
    assert_eq!(a?, Outcome::Applied);
    assert_eq!(b?, Outcome::Applied);
    assert_eq!(store::status(pool, "pilot").await?["applied_lsn"], "0/30");
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM explore.user_favorites")
        .fetch_one(pool)
        .await?;
    assert_eq!(count, 0);
    let counts = sqlx::query("SELECT favorites, likes, dislikes FROM explore.place_interaction_counts WHERE entity_id=$1")
        .bind(PLACE).fetch_one(pool).await?;
    assert_eq!(counts.try_get::<i32, _>("favorites")?, 0);
    assert_eq!(counts.try_get::<i32, _>("likes")?, 1);
    assert_eq!(counts.try_get::<i32, _>("dislikes")?, 1);
    let deleted: bool = sqlx::query_scalar("SELECT deleted FROM migration_sync.favorite_versions")
        .fetch_one(pool)
        .await?;
    assert!(deleted);
    assert!(!store::enqueue(pool, &first).await?);
    assert_eq!(store::process_next(pool, "pilot").await?, Outcome::Idle);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM explore.user_favorites")
            .fetch_one(pool)
            .await?,
        0
    );
    // Registering the original baseline after progress is idempotent, not a reset.
    store::register(pool, "pilot", generation, "0/10").await?;
    assert_eq!(store::status(pool, "pilot").await?["applied_lsn"], "0/30");

    // A later invalid row rolls back earlier writes and their version records.
    let failure = envelope(
        generation,
        "0/30",
        "0/40",
        "source_only",
        json!([upsert(PLACE), upsert(MISSING)]),
    );
    let behind = envelope(
        generation,
        "0/40",
        "0/50",
        "source_only",
        json!([upsert(PLACE)]),
    );
    store::enqueue(pool, &failure).await?;
    store::enqueue(pool, &behind).await?;
    assert_eq!(store::process_next(pool, "pilot").await?, Outcome::Blocked);
    assert_eq!(store::process_next(pool, "pilot").await?, Outcome::Blocked);
    assert_eq!(store::status(pool, "pilot").await?["applied_lsn"], "0/30");
    assert_eq!(
        store::status(pool, "pilot").await?["head_error"],
        "apply_failed"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM explore.user_favorites")
            .fetch_one(pool)
            .await?,
        0
    );
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT deleted FROM migration_sync.favorite_versions")
            .fetch_one(pool)
            .await?
    );
    // Pausing/resuming never acknowledges or skips the poisoned transaction.
    store::pause(pool, "pilot", true).await?;
    store::pause(pool, "pilot", false).await?;
    assert_eq!(store::process_next(pool, "pilot").await?, Outcome::Blocked);
    Ok(())
}

async fn dual_assertions(pool: &PgPool) -> Result<()> {
    let generation = Uuid::new_v4();
    store::register(pool, "pilot", generation, "0/0").await?;
    store::pause(pool, "pilot", false).await?;
    let dual = envelope(generation, "0/0", "0/10", "dual", json!([upsert(PLACE)]));
    store::enqueue(pool, &dual).await?;
    assert_eq!(store::process_next(pool, "pilot").await?, Outcome::Blocked);
    assert_eq!(store::status(pool, "pilot").await?["applied_lsn"], "0/0");
    assert_eq!(store::status(pool, "pilot").await?["blocked"], 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM explore.user_favorites")
            .fetch_one(pool)
            .await?,
        0
    );
    Ok(())
}

async fn schema(pool: &PgPool) -> Result<()> {
    let root = Path::new(env!("DCL_ONE_ROOT")).join("schemas");
    let mut files = std::fs::read_dir(root)?.collect::<std::io::Result<Vec<_>>>()?;
    files.sort_by_key(|entry| entry.file_name());
    for entry in files {
        if entry
            .path()
            .extension()
            .is_some_and(|extension| extension == "sql")
        {
            pool.execute(sqlx::raw_sql(sqlx::AssertSqlSafe(std::fs::read_to_string(
                entry.path(),
            )?)))
            .await?;
        }
    }
    Ok(())
}

#[tokio::test]
async fn ordered_postgres_apply_preserves_boundaries_and_atomicity() -> Result<()> {
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
    for dual in [false, true] {
        let database = format!("migration_sync_test_{}", Uuid::new_v4().simple());
        admin
            .execute(sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                "CREATE DATABASE {database}"
            ))))
            .await?;
        let pool = PgPool::connect_with(options.clone().database(&database)).await?;
        let result = async {
            schema(&pool).await.context("apply current target schema")?;
            if dual {
                dual_assertions(&pool).await
            } else {
                assertions(&pool).await
            }
        }
        .await;
        pool.close().await;
        admin
            .execute(sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                "DROP DATABASE {database} WITH (FORCE)"
            ))))
            .await?;
        result?;
    }
    Ok(())
}
