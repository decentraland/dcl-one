//! The provenance and unpublish contracts on the process's scratch database
//! (`db::scratch`), each test on its own uuid-scoped entity ids and parcel
//! coordinates, so the shared tables never mix two tests' rows. The old
//! tests ran on a schema of their own with the 0001/0003/0005 migrations;
//! here schema.sql always has `local_entities` and `active_pointers.entity_type`,
//! so the old "schema detected at boot" variants collapse into one, and the
//! functions come from the schema instead of a runtime `CREATE`.

use sqlx::PgPool;

use super::*;
use crate::db::SCHEMA_SQL;

async fn pool() -> Option<PgPool> {
    crate::db::scratch::pool(4).await
}

struct Scope {
    tag: String,
    x: i32,
}

impl Scope {
    fn new() -> Self {
        let tag = uuid::Uuid::new_v4().simple().to_string()[..12].to_string();
        let x = 1_000_000 + (uuid::Uuid::new_v4().as_u128() % 1_000_000_000) as i32;
        Self { tag, x }
    }

    fn id(&self, name: &str) -> String {
        format!("bafy{}{name}", self.tag)
    }

    /// A parcel pointer of this scope: `(x + dx, y)`.
    fn parcel(&self, dx: i32, y: i32) -> String {
        format!("{},{y}", self.x + dx)
    }
}

async fn insert_deployment(pool: &PgPool, entity_id: &str, ts_ms: i64, pointers: &[&str]) -> i32 {
    let ptrs: Vec<String> = pointers.iter().map(|p| p.to_string()).collect();
    sqlx::query_scalar(
        "INSERT INTO content.deployments
            (deployer_address, version, entity_type, entity_id, entity_metadata,
             entity_timestamp, entity_pointers, local_timestamp, auth_chain)
         VALUES ('0xdeployer', 'v3', 'scene', $1, NULL,
                 to_timestamp($2 / 1000.0), $3, timezone('UTC', now()), '[]'::json)
         RETURNING id",
    )
    .bind(entity_id)
    .bind(ts_ms as f64)
    .bind(&ptrs)
    .fetch_one(pool)
    .await
    .expect("deployment insert")
}

async fn set_active_pointer(pool: &PgPool, pointer: &str, entity_id: &str) {
    sqlx::query(
        "INSERT INTO content.active_pointers (pointer, entity_id, entity_type) VALUES ($1, $2, 'scene')
         ON CONFLICT (pointer) DO UPDATE SET entity_id = EXCLUDED.entity_id",
    )
    .bind(pointer)
    .bind(entity_id)
    .execute(pool)
    .await
    .expect("active_pointers upsert");
}

async fn set_deleter(pool: &PgPool, entity_id: &str, deleter: i32) {
    sqlx::query("UPDATE content.deployments SET deleter_deployment = $2 WHERE entity_id = $1")
        .bind(entity_id)
        .bind(deleter)
        .execute(pool)
        .await
        .expect("deleter update");
}

async fn active_pointer(pool: &PgPool, pointer: &str) -> Option<String> {
    sqlx::query_scalar("SELECT entity_id FROM content.active_pointers WHERE pointer = $1")
        .bind(pointer)
        .fetch_optional(pool)
        .await
        .expect("active_pointers select")
}

/// The provenance row the deploy function writes for a local scene.
async fn record_provenance(pool: &PgPool, entity_id: &str, signer: &str) {
    sqlx::query("INSERT INTO content.local_entities (entity_id, signer) VALUES ($1, lower($2)) ON CONFLICT (entity_id) DO NOTHING")
        .bind(entity_id)
        .bind(signer)
        .execute(pool)
        .await
        .expect("provenance record");
}

#[test]
fn the_schema_carries_the_write_path_functions() {
    let expected = content_functions_sql(false);
    assert!(
        SCHEMA_SQL.contains(&expected),
        "schemas/00_content.sql must carry the write path's functions exactly as this binary \
         renders them (then `catalyst schema print > schema.sql`); replace the block after \
         `the local write path's functions` with:\n\n{expected}"
    );
    for signature in [
        super::super::write_deployer::deploy_function_signature(false),
        super::super::write_deployer::deploy_function_signature(true),
        unpublish_function_signature(),
    ] {
        let name = signature.split('(').next().unwrap();
        assert!(
            SCHEMA_SQL.contains(&format!("CREATE FUNCTION {name}(")),
            "{name}"
        );
    }
}

#[test]
fn the_unpublish_function_is_named_by_its_body_and_lives_in_content() {
    let name = unpublish_function_name();
    assert!(name.starts_with("unpublish_scene_") && name.len() == "unpublish_scene_".len() + 12);
    assert_eq!(unpublish_function_name(), name, "the name is deterministic");
    let sql = unpublish_function_sql(false);
    assert!(sql.starts_with(&format!("CREATE FUNCTION content.{name}(text, text)")));
    assert!(unpublish_function_sql(true).starts_with(&format!(
        "CREATE OR REPLACE FUNCTION content.{name}(text, text)"
    )));
    for table in [
        "content.active_pointers",
        "content.deployments",
        "content.local_entities",
    ] {
        assert!(sql.contains(table), "{table} is schema-qualified");
    }
    assert!(
        !sql.contains("deleter_deployment"),
        "the scenes the unpublished one superseded stay superseded"
    );
    assert!(
        sql.contains(r#"ORDER BY k COLLATE "C""#),
        "the locks follow the deploy function's byte order"
    );
    assert!(
        content_functions_sql(true)
            .matches("CREATE OR REPLACE FUNCTION")
            .count()
            == 3
    );
}

#[tokio::test]
async fn a_missing_function_is_named_with_its_remedy() {
    let Some(pool) = pool().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    require_function(&pool, &unpublish_function_signature())
        .await
        .expect("schema.sql installs it");
    let err = require_function(&pool, "content.unpublish_scene_000000000000(text,text)")
        .await
        .unwrap_err();
    assert!(
        err.contains("is not installed") && err.contains("catalyst schema functions"),
        "{err}"
    );
    pool.close().await;
}

#[tokio::test]
async fn provenance_recorded_and_surfaced_in_states() {
    let Some(pool) = pool().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let s = Scope::new();
    let (local, upstream, p) = (s.id("local1"), s.id("upstream2"), s.parcel(0, 1));

    let dep = insert_deployment(&pool, &local, 2_000, &[&p]).await;
    set_active_pointer(&pool, &p, &local).await;
    record_provenance(&pool, &local, "0xOwner").await;

    let prov = local_provenance(&pool, &local)
        .await
        .unwrap()
        .expect("provenance row");
    assert_eq!(prov["signer"], "0xowner");
    assert_eq!(prov["origin"], "land-publish");
    assert_eq!(prov["status"], "active");
    assert_eq!(prov["superseded"], false);
    assert!(
        prov["publishedAt"].as_i64().unwrap() > 1_700_000_000_000,
        "published_at is UTC milliseconds"
    );

    let newer = insert_deployment(&pool, &upstream, 3_000, &[&p]).await;
    assert!(newer > dep);
    set_deleter(&pool, &local, newer).await;
    let prov = local_provenance(&pool, &local)
        .await
        .unwrap()
        .expect("provenance row");
    assert_eq!(prov["status"], "superseded");
    assert_eq!(prov["superseded"], true);

    assert!(
        local_provenance(&pool, &upstream).await.unwrap().is_none(),
        "mirrored data has no provenance"
    );
    pool.close().await;
}

/// Looks the local scene at `pointer` up and unpublishes it, as the route does.
async fn unpublish_at(pool: &PgPool, pointer: &str) -> Result<UnpublishOutcome, UnpublishError> {
    let Some(scene) = local_scene_at(pool, pointer).await? else {
        return Err(UnpublishError::NotLocal(pointer.to_string()));
    };
    unpublish_local_scene(pool, pointer, &scene.entity_id).await
}

async fn deleter_of(pool: &PgPool, entity_id: &str) -> Option<i32> {
    sqlx::query_scalar("SELECT deleter_deployment FROM content.deployments WHERE entity_id = $1")
        .bind(entity_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// The freed parcels stay empty: the older scene the local one superseded is
/// not restored and keeps it as its deleter.
#[tokio::test]
async fn unpublish_leaves_the_parcels_empty_over_a_superseded_scene() {
    let Some(pool) = pool().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let s = Scope::new();
    let (up, local) = (s.id("upstream"), s.id("local"));
    let (a, b) = (s.parcel(0, 2), s.parcel(0, 3));

    insert_deployment(&pool, &up, 1_000, &[&a, &b]).await;
    let t = insert_deployment(&pool, &local, 2_000, &[&a, &b]).await;
    set_deleter(&pool, &up, t).await;
    set_active_pointer(&pool, &a, &local).await;
    set_active_pointer(&pool, &b, &local).await;
    record_provenance(&pool, &local, "0xowner").await;

    assert_eq!(
        local_scene_at(&pool, &a).await.unwrap(),
        Some(LocalScene {
            entity_id: local.clone(),
            pointers: vec![a.clone(), b.clone()],
        })
    );
    let outcome = unpublish_at(&pool, &a).await.expect("unpublish");
    assert_eq!(outcome.entity_id, local);
    let mut expected = vec![a.clone(), b.clone()];
    expected.sort();
    assert_eq!(outcome.parcels, expected);
    assert_eq!(active_pointer(&pool, &a).await, None);
    assert_eq!(active_pointer(&pool, &b).await, None);
    assert_eq!(deleter_of(&pool, &up).await, Some(t));
    assert_eq!(deleter_of(&pool, &local).await, None);

    let prov = local_provenance(&pool, &local)
        .await
        .unwrap()
        .expect("provenance row");
    assert_eq!(prov["status"], "unpublished");
    assert!(prov["tombstonedAt"].as_i64().unwrap() >= prov["publishedAt"].as_i64().unwrap());
    pool.close().await;
}

#[tokio::test]
async fn unpublish_deletes_pointer_when_nothing_underneath() {
    let Some(pool) = pool().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let s = Scope::new();
    let (only, p) = (s.id("only"), s.parcel(0, 3));
    insert_deployment(&pool, &only, 1_000, &[&p]).await;
    set_active_pointer(&pool, &p, &only).await;
    record_provenance(&pool, &only, "0xowner").await;

    let outcome = unpublish_at(&pool, &p).await.expect("unpublish");
    assert_eq!(outcome.parcels, vec![p.clone()]);
    assert_eq!(active_pointer(&pool, &p).await, None);
    pool.close().await;
}

#[tokio::test]
async fn unpublish_refuses_synced_and_already_tombstoned_entities() {
    let Some(pool) = pool().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let s = Scope::new();
    let (synced, local, p_synced, p_local) = (
        s.id("synced"),
        s.id("local4"),
        s.parcel(0, 4),
        s.parcel(0, 5),
    );
    insert_deployment(&pool, &synced, 1_000, &[&p_synced]).await;
    set_active_pointer(&pool, &p_synced, &synced).await;
    assert_eq!(local_scene_at(&pool, &p_synced).await.unwrap(), None);
    let err = unpublish_local_scene(&pool, &p_synced, &synced)
        .await
        .unwrap_err();
    assert!(matches!(err, UnpublishError::NotLocal(_)), "{err:?}");

    insert_deployment(&pool, &local, 2_000, &[&p_local]).await;
    set_active_pointer(&pool, &p_local, &local).await;
    record_provenance(&pool, &local, "0xowner").await;
    unpublish_at(&pool, &p_local)
        .await
        .expect("first unpublish");
    let err = unpublish_local_scene(&pool, &p_local, &local)
        .await
        .unwrap_err();
    assert!(matches!(err, UnpublishError::NotLocal(_)), "{err:?}");

    assert_eq!(
        active_pointer(&pool, &p_synced).await.as_deref(),
        Some(synced.as_str())
    );
    pool.close().await;
}

/// The function takes down only the scene the caller authorized: another
/// scene active at the pointer is a conflict and nothing changes.
#[tokio::test]
async fn unpublish_of_another_scene_than_the_active_one_is_a_conflict() {
    let Some(pool) = pool().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let s = Scope::new();
    let (local, other, p) = (s.id("local6"), s.id("other6"), s.parcel(0, 6));
    insert_deployment(&pool, &local, 2_000, &[&p]).await;
    set_active_pointer(&pool, &p, &local).await;
    record_provenance(&pool, &local, "0xowner").await;

    let err = unpublish_local_scene(&pool, &p, &other).await.unwrap_err();
    assert!(matches!(err, UnpublishError::Conflict), "{err:?}");
    assert_eq!(
        active_pointer(&pool, &p).await.as_deref(),
        Some(local.as_str())
    );
    assert_eq!(
        local_provenance(&pool, &local).await.unwrap().unwrap()["status"],
        "active"
    );
    pool.close().await;
}

/// A scene on several parcels: the unpublish of any one of them takes the
/// whole scene down and empties every parcel, the one an older upstream
/// scene occupied before included.
#[tokio::test]
async fn unpublish_of_one_parcel_takes_the_whole_scene_down() {
    let Some(pool) = pool().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let s = Scope::new();
    let (up, local) = (s.id("up7"), s.id("local7"));
    let (a, b) = (s.parcel(0, 7), s.parcel(1, 7));
    insert_deployment(&pool, &up, 1_000, &[&a]).await;
    let t = insert_deployment(&pool, &local, 2_000, &[&a, &b]).await;
    set_deleter(&pool, &up, t).await;
    set_active_pointer(&pool, &a, &local).await;
    set_active_pointer(&pool, &b, &local).await;
    record_provenance(&pool, &local, "0xowner").await;

    let outcome = unpublish_at(&pool, &b).await.expect("unpublish");
    let mut expected = vec![a.clone(), b.clone()];
    expected.sort();
    assert_eq!(outcome.parcels, expected);
    assert_eq!(active_pointer(&pool, &a).await, None);
    assert_eq!(active_pointer(&pool, &b).await, None);
    assert_eq!(deleter_of(&pool, &up).await, Some(t));
    pool.close().await;
}
