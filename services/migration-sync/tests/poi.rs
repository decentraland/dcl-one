//! Narrow POI job outcomes, without copying any other catalogue field.
use anyhow::Result;
use dcl_migration_sync::poi::{self, PRODUCER, PoiResult};
use dcl_migration_sync::{capture, store};
use serde_json::{Value, json};
use sqlx::{Connection, Executor, PgConnection, PgPool, postgres::PgConnectOptions};
use std::{path::Path, str::FromStr};
use uuid::Uuid;

const ID: &str = "a1111111-1111-4111-8111-111111111111";
const OUTSIDE: &str = "b1111111-1111-4111-8111-111111111111";

async fn assertions(pool: &PgPool) -> Result<()> {
    let root = Path::new(env!("DCL_ONE_ROOT"));
    let mut files =
        std::fs::read_dir(root.join("schemas"))?.collect::<std::io::Result<Vec<_>>>()?;
    files.sort_by_key(|entry| entry.file_name());
    for file in files {
        if file
            .path()
            .extension()
            .is_some_and(|extension| extension == "sql")
        {
            pool.execute(sqlx::raw_sql(sqlx::AssertSqlSafe(std::fs::read_to_string(
                file.path(),
            )?)))
            .await?;
        }
    }
    pool.execute(sqlx::raw_sql("CREATE TABLE public.places(id char(36) PRIMARY KEY,world boolean NOT NULL DEFAULT false,disabled boolean NOT NULL DEFAULT false,categories text[] NOT NULL DEFAULT '{}',favorites int NOT NULL DEFAULT 0); CREATE TABLE public.place_categories(place_id char(36),category_id text,PRIMARY KEY(place_id,category_id)); CREATE TABLE public.user_favorites(\"user\" char(42),entity_id char(36),user_activity int,created_at timestamptz,PRIMARY KEY(\"user\",entity_id)); CREATE TABLE public.user_likes(\"user\" char(42),entity_id char(36),\"like\" boolean,user_activity int,created_at timestamptz,updated_at timestamptz,PRIMARY KEY(\"user\",entity_id));")).await?;
    for name in ["places.sql", "poi.sql"] {
        pool.execute(sqlx::raw_sql(sqlx::AssertSqlSafe(std::fs::read_to_string(
            root.join("services/migration-sync/source").join(name),
        )?)))
        .await?;
    }
    sqlx::query("INSERT INTO migration_capture.scope VALUES($1::text::uuid)")
        .bind(ID)
        .execute(pool)
        .await?;
    sqlx::query("INSERT INTO public.places(id,categories) VALUES($1,ARRAY['art'])")
        .bind(ID)
        .execute(pool)
        .await?;
    // Scoped non-POI drift would make the old job do more than the mapped bit.
    assert!(
        sqlx::query("SELECT migration_capture.apply_poi_membership($1::text::uuid,true)")
            .bind(ID)
            .execute(pool)
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT position FROM migration_capture.control")
            .fetch_one(pool)
            .await?,
        0
    );
    sqlx::query("INSERT INTO public.place_categories VALUES($1,'art')")
        .bind(ID)
        .execute(pool)
        .await?;
    let mut rolled_back = pool.begin().await?;
    sqlx::query("SELECT migration_capture.apply_poi_membership($1::text::uuid,true)")
        .bind(ID)
        .execute(&mut *rolled_back)
        .await?;
    rolled_back.rollback().await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT position FROM migration_capture.control")
            .fetch_one(pool)
            .await?,
        0
    );
    sqlx::query("SELECT migration_capture.apply_poi_membership($1::text::uuid,true)")
        .bind(ID)
        .execute(pool)
        .await?;
    let command: Value =
        sqlx::query_scalar("SELECT command FROM migration_capture.transactions WHERE position=1")
            .fetch_one(pool)
            .await?;
    assert_eq!(
        command,
        json!({"producer":PRODUCER,"entity_id":ID,"poi":true})
    );
    let evidence: Value =
        sqlx::query_scalar("SELECT payload FROM migration_capture.changes WHERE position=1")
            .fetch_one(pool)
            .await?;
    assert_eq!(
        evidence,
        json!({"op":"poi_membership","entity_id":ID,"poi":true})
    );
    // Outside scope, retain normal source normalized-category refresh semantics.
    sqlx::query("INSERT INTO public.places(id,categories) VALUES($1,ARRAY['stale'])")
        .bind(OUTSIDE)
        .execute(pool)
        .await?;
    sqlx::query("INSERT INTO public.place_categories VALUES($1,'music')")
        .bind(OUTSIDE)
        .execute(pool)
        .await?;
    sqlx::query("SELECT migration_capture.apply_poi_membership($1::text::uuid,true)")
        .bind(OUTSIDE)
        .execute(pool)
        .await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT position FROM migration_capture.control")
            .fetch_one(pool)
            .await?,
        1
    );
    let outside: Vec<String> =
        sqlx::query_scalar("SELECT categories FROM public.places WHERE id=$1")
            .bind(OUTSIDE)
            .fetch_one(pool)
            .await?;
    assert!(
        outside.contains(&"music".into())
            && outside.contains(&"poi".into())
            && !outside.contains(&"stale".into())
    );
    // A normal category writer is observed but receives no trusted producer.
    sqlx::query("UPDATE public.places SET categories=ARRAY['art'] WHERE id=$1")
        .bind(ID)
        .execute(pool)
        .await?;
    assert_eq!(
        sqlx::query_scalar::<_, Option<Value>>(
            "SELECT command FROM migration_capture.transactions WHERE position=2"
        )
        .fetch_one(pool)
        .await?,
        None
    );
    assert!(
        sqlx::query("TRUNCATE public.places")
            .execute(pool)
            .await
            .is_err()
    );

    let raw = json!({"source":"places","local":true,"world":false,"categories":["art","featured"],"title":"keep","ranking":7});
    sqlx::query("INSERT INTO explore.place(id,base_position,categories,raw,origin,likes,favorites) VALUES($1,'0,0',ARRAY['art','featured'],$2,'local',3,2)").bind(ID).bind(&raw).execute(pool).await?;
    let mut outcome = PoiResult {
        producer: PRODUCER.into(),
        entity_id: ID.into(),
        poi: true,
    };
    let mut tx = pool.begin().await?;
    poi::apply(&mut tx, &outcome).await?;
    tx.commit().await?;
    let mapped:Value=sqlx::query_scalar("SELECT jsonb_build_object('categories',categories,'raw',raw,'likes',likes,'favorites',favorites) FROM explore.place WHERE id=$1").bind(ID).fetch_one(pool).await?;
    assert_eq!(mapped["categories"], json!(["art", "featured", "poi"]));
    assert_eq!(mapped["raw"]["title"], "keep");
    assert_eq!(mapped["raw"]["ranking"], 7);
    assert_eq!(mapped["likes"], 3);
    assert_eq!(mapped["favorites"], 2);
    let mut tx = pool.begin().await?;
    poi::apply(&mut tx, &outcome).await?;
    tx.commit().await?;
    outcome.poi = false;
    let mut tx = pool.begin().await?;
    poi::apply(&mut tx, &outcome).await?;
    tx.commit().await?;
    assert_eq!(
        sqlx::query_scalar::<_, Value>("SELECT raw FROM explore.place WHERE id=$1")
            .bind(ID)
            .fetch_one(pool)
            .await?,
        raw
    );
    // A mapper failure must leave the surrounding stream transaction available to roll back.
    for invalid in [
        json!({}),
        json!({"source":"content","local":true,"world":false,"categories":["art","featured"]}),
        json!({"source":"places","local":true,"world":false,"categories":["different"]}),
    ] {
        let mut tx = pool.begin().await?;
        sqlx::query("UPDATE explore.place SET raw=$2 WHERE id=$1")
            .bind(ID)
            .bind(invalid)
            .execute(&mut *tx)
            .await?;
        assert!(poi::apply(&mut tx, &outcome).await.is_err());
        tx.rollback().await?;
    }
    // The residual uses the same baseline/order as native commands. Untagged
    // evidence blocks without applying, and later tagged results cannot pass it.
    capture::bootstrap(pool, pool, "hybrid").await?;
    store::pause(pool, "hybrid", false).await?;
    sqlx::query("SELECT migration_capture.apply_poi_membership($1::text::uuid,true)")
        .bind(ID)
        .execute(pool)
        .await?;
    assert!(capture::pump_once(pool, pool, "hybrid").await?);
    assert_eq!(
        store::process_next(pool, "hybrid").await?,
        store::Outcome::Applied
    );
    sqlx::query("UPDATE public.places SET categories=ARRAY['art'] WHERE id=$1")
        .bind(ID)
        .execute(pool)
        .await?;
    assert!(capture::pump_once(pool, pool, "hybrid").await?);
    assert_eq!(
        store::process_next(pool, "hybrid").await?,
        store::Outcome::Blocked
    );
    assert_eq!(
        store::status(pool, "hybrid").await?["head_error"],
        "unreceipted_mutation"
    );
    sqlx::query("SELECT migration_capture.apply_poi_membership($1::text::uuid,true)")
        .bind(ID)
        .execute(pool)
        .await?;
    assert!(capture::pump_once(pool, pool, "hybrid").await?);
    assert_eq!(
        store::process_next(pool, "hybrid").await?,
        store::Outcome::Blocked
    );
    let retained: Vec<String> =
        sqlx::query_scalar("SELECT categories FROM explore.place WHERE id=$1")
            .bind(ID)
            .fetch_one(pool)
            .await?;
    assert_eq!(retained, vec!["art", "featured", "poi"]);

    outcome.entity_id = OUTSIDE.into();
    assert!(
        poi::apply(&mut *pool.acquire().await?, &outcome)
            .await
            .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn poi_capture_and_mapping_preserve_only_the_authorized_membership() -> Result<()> {
    let Ok(url) = std::env::var("MIGRATION_SYNC_TEST_DATABASE_URL") else {
        assert!(
            std::env::var_os("CI").is_none(),
            "CI requires MIGRATION_SYNC_TEST_DATABASE_URL"
        );
        return Ok(());
    };
    let options = PgConnectOptions::from_str(&url)?;
    let mut admin = PgConnection::connect_with(&options).await?;
    let name = format!("poi_residual_{}", Uuid::new_v4().simple());
    admin
        .execute(sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "CREATE DATABASE {name}"
        ))))
        .await?;
    let pool = PgPool::connect_with(options.database(&name)).await?;
    let result = assertions(&pool).await;
    pool.close().await;
    admin
        .execute(sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE {name} WITH (FORCE)"
        ))))
        .await?;
    result
}
