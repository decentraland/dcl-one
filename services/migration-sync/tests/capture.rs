//! Source-local capture and target bootstrap against disposable PostgreSQL databases.
use anyhow::Result;
use dcl_migration_sync::{capture, store};
use sqlx::{Connection, Executor, PgConnection, PgPool, postgres::PgConnectOptions};
use std::{path::Path, str::FromStr};
use uuid::Uuid;
const USER: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const PLACE: &str = "11111111-1111-4111-8111-111111111111";

async fn assertions(source: &PgPool, target: &PgPool) -> Result<()> {
    source.execute(sqlx::raw_sql("CREATE TABLE places(id uuid PRIMARY KEY,world boolean NOT NULL DEFAULT false,disabled boolean NOT NULL DEFAULT false,favorites integer NOT NULL DEFAULT 0); CREATE TABLE user_favorites(\"user\" text,entity_id uuid,user_activity integer NOT NULL,created_at timestamptz NOT NULL DEFAULT now(),PRIMARY KEY(\"user\",entity_id)); CREATE TABLE user_likes(\"user\" text,entity_id uuid,\"like\" boolean NOT NULL,user_activity integer NOT NULL,created_at timestamptz NOT NULL DEFAULT now(),updated_at timestamptz NOT NULL DEFAULT now(),PRIMARY KEY(\"user\",entity_id)); INSERT INTO places(id) VALUES('11111111-1111-4111-8111-111111111111');")).await?;
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
    // An uncommitted source writer holds the common fence: baseline must include
    // its eventual commit, never skip a late-committing allocated position.
    let mut first = source.begin().await?;
    sqlx::query("INSERT INTO user_favorites VALUES($1,$2,3,now())")
        .bind(USER)
        .bind(id)
        .execute(&mut *first)
        .await?;
    sqlx::query(
        "INSERT INTO user_likes VALUES($1,$2,true,5,'2026-01-01T00:00:00Z','2026-01-02T00:00:00Z')",
    )
    .bind(USER)
    .bind(id)
    .execute(&mut *first)
    .await?;
    let src = source.clone();
    let dst = target.clone();
    let baseline = tokio::spawn(async move { capture::bootstrap(&src, &dst, "pilot").await });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(!baseline.is_finished());
    first.commit().await?;
    assert_eq!(baseline.await??.position, 1);
    store::pause(target, "pilot", false).await?;
    assert_eq!(
        capture::reconcile(source, target, "pilot").await?["matches"],
        true
    );
    // Complete favorite rows alone cannot make old adapter metadata compatible.
    sqlx::query("UPDATE migration_sync.capture_sources SET adapter='places_outbox_v1'")
        .execute(target)
        .await?;
    assert!(
        capture::reconcile(source, target, "pilot")
            .await
            .unwrap_err()
            .to_string()
            .contains("adapters differ")
    );
    sqlx::query("UPDATE migration_sync.capture_sources SET adapter='places_outbox_v2'")
        .execute(target)
        .await?;
    sqlx::query("UPDATE explore.place_interaction_counts SET favorites=99")
        .execute(target)
        .await?;
    assert_eq!(
        capture::reconcile(source, target, "pilot").await?["counts_match"],
        false
    );
    sqlx::query("UPDATE explore.place_interaction_counts SET favorites=1")
        .execute(target)
        .await?;
    let mut rolled_back = source.begin().await?;
    sqlx::query("DELETE FROM user_favorites")
        .execute(&mut *rolled_back)
        .await?;
    rolled_back.rollback().await?;
    assert_eq!(capture::source_info(source).await?.position, 1);
    sqlx::query("DELETE FROM user_favorites")
        .execute(source)
        .await?;
    assert!(capture::pump_once(source, target, "pilot").await?);
    assert_eq!(
        store::process_next(target, "pilot").await?,
        store::Outcome::Applied
    );
    assert_eq!(
        capture::reconcile(source, target, "pilot").await?["matches"],
        true
    );
    // An existing client vote flip and metadata update have no receipt header.
    // Source-only capture preserves both timestamps and derives all counters.
    sqlx::query(
        "UPDATE user_likes SET \"like\"=false,user_activity=9,updated_at='2026-02-03T04:05:06Z'",
    )
    .execute(source)
    .await?;
    assert!(capture::pump_once(source, target, "pilot").await?);
    assert_eq!(
        store::process_next(target, "pilot").await?,
        store::Outcome::Applied
    );
    assert_eq!(
        capture::reconcile(source, target, "pilot").await?["matches"],
        true
    );
    sqlx::query("UPDATE explore.place_interaction_counts SET dislikes=99")
        .execute(target)
        .await?;
    assert_eq!(
        capture::reconcile(source, target, "pilot").await?["counts_match"],
        false
    );
    sqlx::query("UPDATE explore.place_interaction_counts SET dislikes=1")
        .execute(target)
        .await?;
    // Capture several facts from both tables as one source transaction.
    let mut mixed = source.begin().await?;
    sqlx::query("INSERT INTO user_favorites VALUES($1,$2,1,now())")
        .bind(USER)
        .bind(id)
        .execute(&mut *mixed)
        .await?;
    sqlx::query("DELETE FROM user_likes")
        .execute(&mut *mixed)
        .await?;
    sqlx::query("DELETE FROM user_favorites")
        .execute(&mut *mixed)
        .await?;
    mixed.commit().await?;
    assert!(capture::pump_once(source, target, "pilot").await?);
    assert_eq!(
        store::process_next(target, "pilot").await?,
        store::Outcome::Applied
    );
    assert_eq!(
        capture::reconcile(source, target, "pilot").await?["matches"],
        true
    );
    assert!(
        sqlx::query("TRUNCATE user_likes")
            .execute(source)
            .await
            .is_err()
    );
    // A second writer cannot commit past an earlier open capture transaction.
    let mut earlier = source.begin().await?;
    sqlx::query("INSERT INTO user_favorites VALUES($1,$2,3,now())")
        .bind(USER)
        .bind(id)
        .execute(&mut *earlier)
        .await?;
    let second_source = source.clone();
    let later = tokio::spawn(async move {
        sqlx::query("INSERT INTO user_favorites VALUES('0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb','11111111-1111-4111-8111-111111111111',4,now())")
            .execute(&second_source)
            .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(!later.is_finished());
    assert!(!capture::pump_once(source, target, "pilot").await?);
    earlier.commit().await?;
    later.await??;
    for _ in 0..2 {
        assert!(capture::pump_once(source, target, "pilot").await?);
        assert_eq!(
            store::process_next(target, "pilot").await?,
            store::Outcome::Applied
        );
    }
    assert_eq!(
        capture::reconcile(source, target, "pilot").await?["matches"],
        true
    );
    sqlx::query("DELETE FROM user_favorites")
        .execute(source)
        .await?;
    assert!(capture::pump_once(source, target, "pilot").await?);
    assert_eq!(
        store::process_next(target, "pilot").await?,
        store::Outcome::Applied
    );
    assert!(
        sqlx::query("TRUNCATE user_favorites")
            .execute(source)
            .await
            .is_err()
    );
    // Accepted command and final source state are captured in the same commit.
    let op = Uuid::new_v4();
    let call = "SELECT migration_capture.execute_favorite($1,$2,$3,$4,true,7::float8)";
    let response: serde_json::Value = sqlx::query_scalar(call)
        .bind(op)
        .bind("a".repeat(64))
        .bind(USER)
        .bind(id)
        .fetch_one(source)
        .await?;
    assert_eq!(response["user_favorite"], true);
    let head = capture::source_info(source).await?.position;
    let duplicate: serde_json::Value = sqlx::query_scalar(call)
        .bind(op)
        .bind("a".repeat(64))
        .bind(USER)
        .bind(id)
        .fetch_one(source)
        .await?;
    assert_eq!(response, duplicate);
    assert_eq!(capture::source_info(source).await?.position, head);
    assert!(
        sqlx::query_scalar::<_, serde_json::Value>(call)
            .bind(op)
            .bind("b".repeat(64))
            .bind(USER)
            .bind(id)
            .fetch_one(source)
            .await
            .is_err()
    );
    assert!(capture::pump_once(source, target, "pilot").await?);
    // A new no-op request still gets a durable empty transaction receipt.
    sqlx::query_scalar::<_, serde_json::Value>(call)
        .bind(Uuid::new_v4())
        .bind("a".repeat(64))
        .bind(USER)
        .bind(id)
        .fetch_one(source)
        .await?;
    assert!(capture::pump_once(source, target, "pilot").await?);
    let empty:i64=sqlx::query_scalar("SELECT count(*) FROM migration_capture.transactions t WHERE command IS NOT NULL AND NOT EXISTS(SELECT 1 FROM migration_capture.changes c WHERE c.position=t.position)").fetch_one(source).await?;
    assert_eq!(empty, 1);
    assert!(!capture::pump_once(source, target, "pilot").await?);
    // The runtime source connection is genuinely read-only: polling never
    // performs source acknowledgements or source business mutations.
    let read_options = source
        .connect_options()
        .as_ref()
        .clone()
        .options([("default_transaction_read_only", "on")]);
    let read_only = PgPool::connect_with(read_options).await?;
    assert!(!capture::pump_once(&read_only, target, "pilot").await?);
    read_only.close().await;
    // Invalid canonical keys are quarantined by refusing advancement, never lost.
    sqlx::query("INSERT INTO user_favorites VALUES('bad-wallet',$1,1,now())")
        .bind(id)
        .execute(source)
        .await?;
    assert!(capture::pump_once(source, target, "pilot").await.is_err());
    let captured: i64 =
        sqlx::query_scalar("SELECT captured_position FROM migration_sync.capture_sources")
            .fetch_one(target)
            .await?;
    assert_eq!(captured, head + 1);
    sqlx::query("DELETE FROM places").execute(source).await?;
    let retained: serde_json::Value = sqlx::query_scalar(call)
        .bind(op)
        .bind("a".repeat(64))
        .bind(USER)
        .bind(id)
        .fetch_one(source)
        .await?;
    assert_eq!(retained, response);
    assert!(
        sqlx::query_scalar::<_, serde_json::Value>(call)
            .bind(Uuid::new_v4())
            .bind("a".repeat(64))
            .bind(USER)
            .bind(id)
            .fetch_one(source)
            .await
            .is_err()
    );

    let generation = capture::source_info(source).await?.generation;
    sqlx::query("UPDATE migration_capture.control SET generation=$1")
        .bind(Uuid::new_v4())
        .execute(source)
        .await?;
    assert!(
        capture::pump_once(source, target, "pilot")
            .await
            .unwrap_err()
            .to_string()
            .contains("generation changed")
    );
    sqlx::query("UPDATE migration_capture.control SET generation=$1")
        .bind(generation)
        .execute(source)
        .await?;
    sqlx::query("INSERT INTO migration_capture.scope VALUES($1)")
        .bind(Uuid::new_v4())
        .execute(source)
        .await?;
    assert!(
        capture::pump_once(source, target, "pilot")
            .await
            .unwrap_err()
            .to_string()
            .contains("scope changed")
    );
    // A v1 installation cannot be mistaken for complete interaction capture.
    sqlx::raw_sql("ALTER TABLE migration_capture.control DROP CONSTRAINT control_version_check; UPDATE migration_capture.control SET version=1")
        .execute(source).await?;
    assert!(capture::source_info(source).await.is_err());
    assert!(
        capture::install(source, &[id])
            .await
            .unwrap_err()
            .to_string()
            .contains("version lacks")
    );
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
    let options = PgConnectOptions::from_str(&url)?;
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
    let result = assertions(&source, &target).await;
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
