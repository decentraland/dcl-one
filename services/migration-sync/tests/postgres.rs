//! Real PostgreSQL regression of ordering, atomicity, replay and fencing.
//! Requires an explicitly configured disposable server with CREATEDB permission.
use std::{path::Path, str::FromStr};

use anyhow::{Context, Result};
use dcl_migration_sync::{
    model::Envelope,
    store::{self, Outcome},
};
use serde_json::{Value, json};
use sqlx::{Connection, Executor, PgConnection, PgPool, postgres::PgConnectOptions};
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

    // Unreceipted changes remain evidence, never an automatic database fallback.
    let (a, b) = tokio::join!(
        store::process_next(pool, "pilot"),
        store::process_next(pool, "pilot")
    );
    assert_eq!(a?, Outcome::Blocked);
    assert_eq!(b?, Outcome::Blocked);
    assert_eq!(store::status(pool, "pilot").await?["applied_lsn"], "0/10");
    assert_eq!(
        store::status(pool, "pilot").await?["head_error"],
        "unreceipted_mutation"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM explore.user_favorites")
            .fetch_one(pool)
            .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM migration_sync.favorite_versions")
            .fetch_one(pool)
            .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM explore.place_interaction_counts")
            .fetch_one(pool)
            .await?,
        0
    );
    store::pause(pool, "pilot", true).await?;
    dcl_migration_sync::ops::retry_blocked(pool, "pilot", "0/20").await?;
    store::pause(pool, "pilot", false).await?;
    assert_eq!(store::process_next(pool, "pilot").await?, Outcome::Blocked);
    assert_eq!(store::status(pool, "pilot").await?["applied_lsn"], "0/10");

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

async fn native_assertions(pool: &PgPool) -> Result<()> {
    use catalyst_explore::migration_replay::{ReplayRequest, replay};
    fixture(pool).await?;
    let generation = Uuid::new_v4();
    store::register(pool, "pilot", generation, "0/0").await?;
    store::pause(pool, "pilot", false).await?;
    let mut dual = envelope(generation, "0/0", "0/1", "dual", json!([upsert(PLACE)]));
    dual.command = Some(serde_json::from_value(json!({
        "operation_id":Uuid::new_v4(), "request_hash":"a".repeat(64),
        "user":USER,"entity_id":PLACE,"favorite":true,"user_activity":3.0,
        "expected":{"user_activity":3.0,"created_at":"2026-11-12T00:00:00Z"}
    }))?);
    store::enqueue(pool, &dual).await?;
    assert_eq!(
        store::process_next(pool, "pilot").await?,
        Outcome::AwaitingCommand
    );
    let pointer = ReplayRequest {
        stream: "pilot".into(),
        generation,
        commit_lsn: "0/1".into(),
    };
    // Real native mutation + receipt. Simulate losing its HTTP response, retry
    // before acknowledging it, then classify without replaying captured SQL.
    let (first, duplicate, worker) = tokio::join!(
        replay(pool, &pointer),
        replay(pool, &pointer),
        store::process_next(pool, "pilot")
    );
    let first = first?;
    assert_eq!(duplicate?.operation_id, first.operation_id);
    assert!(matches!(
        worker?,
        Outcome::Applied | Outcome::AwaitingCommand
    ));
    assert_eq!(
        replay(pool, &pointer).await?.operation_id,
        first.operation_id
    );
    assert!(matches!(
        store::process_next(pool, "pilot").await?,
        Outcome::Applied | Outcome::Idle
    ));
    assert_eq!(store::status(pool, "pilot").await?["applied_lsn"], "0/1");
    assert_eq!(
        replay(pool, &pointer).await?.operation_id,
        first.operation_id
    );
    // Native no-op keeps the original source timestamp and still gets a receipt.
    let mut noop = dual.clone();
    noop.previous_lsn = "0/1".into();
    noop.commit_lsn = "0/2".into();
    noop.changes.clear();
    noop.command.as_mut().unwrap().operation_id = Uuid::new_v4();
    store::enqueue(pool, &noop).await?;
    let noopp = ReplayRequest {
        commit_lsn: "0/2".into(),
        ..pointer.clone()
    };
    replay(pool, &noopp).await?;
    assert_eq!(store::process_next(pool, "pilot").await?, Outcome::Applied);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM explore.user_favorites")
        .fetch_one(pool)
        .await?;
    assert_eq!(count, 1);
    // A later accepted delete executes natively. An older retry cannot undo it.
    let mut removal = dual.clone();
    removal.previous_lsn = "0/2".into();
    removal.commit_lsn = "0/3".into();
    removal.changes = serde_json::from_value(json!([delete(PLACE)]))?;
    let removal_command = removal.command.as_mut().unwrap();
    removal_command.operation_id = Uuid::new_v4();
    removal_command.favorite = false;
    removal_command.expected = None;
    store::enqueue(pool, &removal).await?;
    replay(
        pool,
        &ReplayRequest {
            commit_lsn: "0/3".into(),
            ..pointer.clone()
        },
    )
    .await?;
    assert_eq!(store::process_next(pool, "pilot").await?, Outcome::Applied);
    replay(pool, &pointer).await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM explore.user_favorites")
            .fetch_one(pool)
            .await?,
        0
    );
    // Reusing an operation ID for another accepted position cannot execute.
    let mut reused = dual.clone();
    reused.previous_lsn = "0/3".into();
    reused.commit_lsn = "0/4".into();
    store::enqueue(pool, &reused).await?;
    assert!(
        replay(
            pool,
            &ReplayRequest {
                commit_lsn: "0/4".into(),
                ..pointer.clone()
            }
        )
        .await
        .is_err()
    );
    assert_eq!(store::process_next(pool, "pilot").await?, Outcome::Blocked);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM explore.user_favorites")
            .fetch_one(pool)
            .await?,
        0
    );
    assert!(
        dcl_migration_sync::ops::retry_blocked(pool, "pilot", "0/4")
            .await
            .is_err()
    );
    store::pause(pool, "pilot", true).await?;
    assert!(
        dcl_migration_sync::ops::retry_blocked(pool, "pilot", "0/2")
            .await
            .is_err()
    );
    dcl_migration_sync::ops::retry_blocked(pool, "pilot", "0/4").await?;
    store::pause(pool, "pilot", false).await?;
    assert_eq!(store::process_next(pool, "pilot").await?, Outcome::Blocked);
    // Captured payload remains immutable even after execution.
    dual.command.as_mut().unwrap().request_hash = "b".repeat(64);
    assert!(store::enqueue(pool, &dual).await.is_err());
    Ok(())
}

async fn native_failure_assertions(pool: &PgPool) -> Result<()> {
    use catalyst_explore::migration_replay::{ReplayRequest, replay};
    fixture(pool).await?;
    let generation = Uuid::new_v4();
    store::register(pool, "pilot", generation, "0/0").await?;
    store::pause(pool, "pilot", false).await?;
    let mut dual = envelope(generation, "0/1", "0/2", "dual", json!([upsert(PLACE)]));
    dual.command = Some(serde_json::from_value(json!({
        "operation_id":Uuid::new_v4(),"request_hash":"a".repeat(64),
        "user":USER,"entity_id":PLACE,"favorite":true,"user_activity":3.0,
        "expected":{"user_activity":3.0,"created_at":"2026-11-12T00:00:00Z"}
    }))?);
    store::enqueue(pool, &dual).await?;
    let pointer = ReplayRequest {
        stream: "pilot".into(),
        generation,
        commit_lsn: "0/2".into(),
    };
    assert!(replay(pool, &pointer).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM explore.user_favorites")
            .fetch_one(pool)
            .await?,
        0
    );
    // Fill the predecessor using an accepted native command; its different
    // resulting activity makes the following command fail equivalence checks.
    let mut predecessor = dual.clone();
    predecessor.previous_lsn = "0/0".into();
    predecessor.commit_lsn = "0/1".into();
    let mut different = upsert(PLACE);
    different["user_activity"] = json!(9.0);
    predecessor.changes = serde_json::from_value(json!([different]))?;
    let command = predecessor.command.as_mut().unwrap();
    command.operation_id = Uuid::new_v4();
    command.user_activity = 9.0;
    command.expected.as_mut().unwrap().user_activity = 9.0;
    store::enqueue(pool, &predecessor).await?;
    replay(
        pool,
        &ReplayRequest {
            commit_lsn: "0/1".into(),
            ..pointer.clone()
        },
    )
    .await?;
    assert_eq!(store::process_next(pool, "pilot").await?, Outcome::Applied);
    assert!(replay(pool, &pointer).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM migration_sync.command_receipts")
            .fetch_one(pool)
            .await?,
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, f64>("SELECT user_activity FROM explore.user_favorites")
            .fetch_one(pool)
            .await?,
        9.0
    );
    assert_eq!(
        store::process_next(pool, "pilot").await?,
        Outcome::AwaitingCommand
    );
    let command = store::pending_command(pool, "pilot").await?.unwrap();
    store::reject_command(pool, &command).await?;
    assert_eq!(store::process_next(pool, "pilot").await?, Outcome::Blocked);
    // A delayed request cannot execute a rejected head, even if an operator
    // repairs its data; an explicit paused retry is still required.
    sqlx::query("UPDATE explore.user_favorites SET user_activity=3.0")
        .execute(pool)
        .await?;
    assert!(replay(pool, &pointer).await.is_err());
    store::pause(pool, "pilot", true).await?;
    dcl_migration_sync::ops::retry_blocked(pool, "pilot", "0/2").await?;
    store::pause(pool, "pilot", false).await?;
    replay(pool, &pointer).await?;
    assert_eq!(store::process_next(pool, "pilot").await?, Outcome::Applied);
    Ok(())
}

async fn http_dispatch(pool: &PgPool, status: u16) -> Result<Outcome> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!(
        "http://{}/internal/migration/favorites/replay",
        listener.local_addr()?
    );
    let target = pool.clone();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut bytes = Vec::new();
        let (header_end, length) = loop {
            let mut buffer = [0; 4096];
            let read = socket.read(&mut buffer).await.unwrap();
            assert!(read > 0);
            bytes.extend_from_slice(&buffer[..read]);
            if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                let headers = std::str::from_utf8(&bytes[..end])
                    .unwrap()
                    .to_ascii_lowercase();
                assert!(headers.starts_with("post /internal/migration/favorites/replay http/1.1"));
                assert!(headers.contains("authorization: bearer test-token"));
                let length = headers
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length: "))
                    .unwrap()
                    .parse::<usize>()
                    .unwrap();
                break (end + 4, length);
            }
        };
        while bytes.len() < header_end + length {
            let mut buffer = [0; 4096];
            let read = socket.read(&mut buffer).await.unwrap();
            assert!(read > 0);
            bytes.extend_from_slice(&buffer[..read]);
        }
        let pointer: catalyst_explore::migration_replay::ReplayRequest =
            serde_json::from_slice(&bytes[header_end..header_end + length]).unwrap();
        assert_eq!(pointer.stream, "pilot");
        if status == 0 {
            tokio::time::sleep(std::time::Duration::from_millis(80)).await;
            return;
        }
        if status == 1 {
            // Commit real native state then lose the entire HTTP response.
            catalyst_explore::migration_replay::replay(&target, &pointer)
                .await
                .unwrap();
            return;
        }
        socket
            .write_all(
                format!("HTTP/1.1 {status} Test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .await
            .unwrap();
    });
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(if status == 0 {
            20
        } else {
            2000
        }))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let outcome =
        dcl_migration_sync::dispatch::process_next(pool, "pilot", &client, &endpoint, "test-token")
            .await?;
    server.await?;
    Ok(outcome)
}

async fn dispatch_assertions(pool: &PgPool) -> Result<()> {
    fixture(pool).await?;
    let generation = Uuid::new_v4();
    store::register(pool, "pilot", generation, "0/0").await?;
    store::pause(pool, "pilot", false).await?;
    let mut dual = envelope(generation, "0/0", "0/1", "dual", json!([upsert(PLACE)]));
    dual.command = Some(serde_json::from_value(json!({
        "operation_id":Uuid::new_v4(),"request_hash":"a".repeat(64),
        "user":USER,"entity_id":PLACE,"favorite":true,"user_activity":3.0,
        "expected":{"user_activity":3.0,"created_at":"2026-11-12T00:00:00Z"}
    }))?);
    store::enqueue(pool, &dual).await?;
    for status in [200, 503, 0] {
        assert_eq!(http_dispatch(pool, status).await?, Outcome::AwaitingCommand);
        assert_eq!(store::status(pool, "pilot").await?["applied_lsn"], "0/0");
    }
    assert_eq!(http_dispatch(pool, 1).await?, Outcome::Applied);
    let mut next = dual.clone();
    next.previous_lsn = "0/1".into();
    next.commit_lsn = "0/2".into();
    next.changes.clear();
    next.command.as_mut().unwrap().operation_id = Uuid::new_v4();
    store::enqueue(pool, &next).await?;
    assert_eq!(http_dispatch(pool, 400).await?, Outcome::Blocked);
    let mut outside = next.clone();
    outside.previous_lsn = "0/2".into();
    outside.commit_lsn = "0/3".into();
    outside.command.as_mut().unwrap().entity_id = MISSING.into();
    outside.command.as_mut().unwrap().operation_id = Uuid::new_v4();
    store::enqueue(pool, &outside).await?;
    sqlx::query("INSERT INTO migration_sync.capture_sources(stream,generation,captured_position,observed_position,last_contact_at) VALUES('pilot',$1,3,3,now())")
        .bind(generation).execute(pool).await?;
    sqlx::query("INSERT INTO migration_sync.capture_scope(stream,generation,entity_id) VALUES('pilot',$1,$2)")
        .bind(generation).bind(Uuid::parse_str(PLACE)?).execute(pool).await?;
    let refused = catalyst_explore::migration_replay::replay(
        pool,
        &catalyst_explore::migration_replay::ReplayRequest {
            stream: "pilot".into(),
            generation,
            commit_lsn: "0/3".into(),
        },
    )
    .await
    .unwrap_err();
    assert!(refused.to_string().contains("scope"));
    outside.previous_lsn = "0/3".into();
    outside.commit_lsn = "0/4".into();
    assert!(store::enqueue(pool, &outside).await.is_err());
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
    for scenario in 0..5 {
        let database = format!("migration_sync_test_{}", Uuid::new_v4().simple());
        admin
            .execute(sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                "CREATE DATABASE {database}"
            ))))
            .await?;
        let pool = PgPool::connect_with(options.clone().database(&database)).await?;
        let result = async {
            schema(&pool).await.context("apply current target schema")?;
            match scenario {
                0 => assertions(&pool).await,
                1 => dual_assertions(&pool).await,
                2 => native_assertions(&pool).await,
                3 => native_failure_assertions(&pool).await,
                _ => dispatch_assertions(&pool).await,
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
