// from: catalyrst/crates/catalyrst-server/tests/sync_flush_overwrites.rs
//! The batched sync flush's overwrite semantics against a real postgres: a newer entity on
//! a pointer marks the older one deleted whichever order they arrive in (across flushes or
//! inside one batch), active_pointers follows the winner, and the startup repair
//! (`resolve_deleter_deployments`) agrees with the legacy successor rule. Needs
//! CATALYST_TEST_DATABASE_URL; each test gets its own scratch database.

use std::sync::Arc;

use sqlx::PgPool;

use catalyst::content::sync::{DeploymentContext, LiveDeploymentRepository, SyncStatus};
use catalyst::content::sync_deployer::{DEFAULT_FLUSH_CONCURRENCY, LiveSyncDeployer};

#[path = "support/auth.rs"]
mod auth;
#[path = "support/db.rs"]
mod db;

fn entity_json(
    entity_type: &str,
    pointers: &[&str],
    timestamp: f64,
    content: &[(&str, &str)],
) -> Vec<u8> {
    let content: Vec<serde_json::Value> = content
        .iter()
        .map(|(file, hash)| serde_json::json!({"file": file, "hash": hash}))
        .collect();
    serde_json::to_vec(&serde_json::json!({
        "version": "v3",
        "type": entity_type,
        "pointers": pointers,
        "timestamp": timestamp,
        "content": content,
        "metadata": {"name": "test"},
    }))
    .unwrap()
}

fn deployer(pool: &PgPool) -> LiveSyncDeployer {
    LiveSyncDeployer::new(pool.clone(), DEFAULT_FLUSH_CONCURRENCY)
}

async fn deploy(
    deployer: &LiveSyncDeployer,
    entity_id: &str,
    entity_type: &str,
    pointers: &[&str],
    timestamp: f64,
    content: &[(&str, &str)],
) {
    deployer
        .deploy_entity(
            &entity_json(entity_type, pointers, timestamp, content),
            entity_id,
            &auth::signed_chain(entity_id).await,
            DeploymentContext::Synced,
            None,
            None,
        )
        .await
        .unwrap();
}

async fn deleter_of(pool: &PgPool, entity_id: &str) -> Option<i32> {
    let (deleter,): (Option<i32>,) =
        sqlx::query_as("SELECT deleter_deployment FROM content.deployments WHERE entity_id = $1")
            .bind(entity_id)
            .fetch_one(pool)
            .await
            .unwrap();
    deleter
}

async fn deployment_id(pool: &PgPool, entity_id: &str) -> i32 {
    let (id,): (i32,) = sqlx::query_as("SELECT id FROM content.deployments WHERE entity_id = $1")
        .bind(entity_id)
        .fetch_one(pool)
        .await
        .unwrap();
    id
}

async fn active_pointer(pool: &PgPool, pointer: &str) -> Option<String> {
    sqlx::query_scalar("SELECT entity_id FROM content.active_pointers WHERE pointer = $1")
        .bind(pointer)
        .fetch_optional(pool)
        .await
        .unwrap()
}

async fn active_by_hash(pool: &PgPool, hash: &str) -> Vec<String> {
    catalyst::content::queries::active_entity_ids_by_content_hash(pool, hash)
        .await
        .unwrap()
}

#[tokio::test]
async fn newer_first_then_older_marks_late_arrival_overwritten() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();

    let deployer = deployer(&pool);
    deploy(
        &deployer,
        "bafynewer1",
        "wearable",
        &["urn:pointer:a"],
        2_000_000.0,
        &[("thumbnail.png", "bafyhashnew1")],
    )
    .await;
    deployer.flush().await.unwrap();
    deploy(
        &deployer,
        "bafyolder1",
        "wearable",
        &["urn:pointer:a"],
        1_000_000.0,
        &[("thumbnail.png", "bafyhashold1")],
    )
    .await;
    deployer.flush().await.unwrap();

    let newer_id = deployment_id(&pool, "bafynewer1").await;
    assert_eq!(deleter_of(&pool, "bafyolder1").await, Some(newer_id));
    assert_eq!(deleter_of(&pool, "bafynewer1").await, None);
    assert_eq!(
        active_pointer(&pool, "urn:pointer:a").await.as_deref(),
        Some("bafynewer1")
    );
    assert!(active_by_hash(&pool, "bafyhashold1").await.is_empty());
    assert_eq!(
        active_by_hash(&pool, "bafyhashnew1").await,
        vec!["bafynewer1".to_string()]
    );

    db.teardown().await;
}

#[tokio::test]
async fn older_first_then_newer_marks_older_overwritten() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();

    let deployer = deployer(&pool);
    deploy(
        &deployer,
        "bafyolder2",
        "scene",
        &["10,10"],
        1_000_000.0,
        &[("scene.json", "bafyhashold2")],
    )
    .await;
    deployer.flush().await.unwrap();
    deploy(
        &deployer,
        "bafynewer2",
        "scene",
        &["10,10"],
        2_000_000.0,
        &[("scene.json", "bafyhashnew2")],
    )
    .await;
    deployer.flush().await.unwrap();

    let newer_id = deployment_id(&pool, "bafynewer2").await;
    assert_eq!(deleter_of(&pool, "bafyolder2").await, Some(newer_id));
    assert_eq!(deleter_of(&pool, "bafynewer2").await, None);
    assert_eq!(
        active_pointer(&pool, "10,10").await.as_deref(),
        Some("bafynewer2")
    );
    assert!(active_by_hash(&pool, "bafyhashold2").await.is_empty());

    db.teardown().await;
}

#[tokio::test]
async fn intra_batch_overwrite_leaves_exactly_one_survivor() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();

    let deployer = deployer(&pool);
    deploy(
        &deployer,
        "bafyolder3",
        "scene",
        &["20,20"],
        1_000_000.0,
        &[("scene.json", "bafyhashold3")],
    )
    .await;
    deploy(
        &deployer,
        "bafynewer3",
        "scene",
        &["20,20"],
        2_000_000.0,
        &[("scene.json", "bafyhashnew3")],
    )
    .await;
    deployer.flush().await.unwrap();

    let newer_id = deployment_id(&pool, "bafynewer3").await;
    assert_eq!(deleter_of(&pool, "bafyolder3").await, Some(newer_id));
    assert_eq!(deleter_of(&pool, "bafynewer3").await, None);
    assert_eq!(
        active_pointer(&pool, "20,20").await.as_deref(),
        Some("bafynewer3")
    );

    db.teardown().await;
}

#[tokio::test]
async fn same_hash_in_two_files_returns_entity_twice() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();

    let deployer = deployer(&pool);
    deploy(
        &deployer,
        "bafydup1",
        "wearable",
        &["urn:pointer:dup"],
        1_000_000.0,
        &[
            ("male/model.glb", "bafyhashshared"),
            ("female/model.glb", "bafyhashshared"),
        ],
    )
    .await;
    deployer.flush().await.unwrap();

    assert_eq!(
        active_by_hash(&pool, "bafyhashshared").await,
        vec!["bafydup1".to_string(), "bafydup1".to_string()]
    );

    deploy(
        &deployer,
        "bafydup2",
        "wearable",
        &["urn:pointer:dup"],
        2_000_000.0,
        &[("thumbnail.png", "bafyhashother")],
    )
    .await;
    deployer.flush().await.unwrap();

    assert!(active_by_hash(&pool, "bafyhashshared").await.is_empty());

    db.teardown().await;
}

#[tokio::test]
async fn shrunk_pointer_set_clears_uncovered_active_pointer() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();

    let deployer = deployer(&pool);
    deploy(
        &deployer,
        "bafyolder4",
        "scene",
        &["30,30", "30,31"],
        1_000_000.0,
        &[("scene.json", "bafyhashold4")],
    )
    .await;
    deployer.flush().await.unwrap();
    assert_eq!(
        active_pointer(&pool, "30,30").await.as_deref(),
        Some("bafyolder4")
    );
    assert_eq!(
        active_pointer(&pool, "30,31").await.as_deref(),
        Some("bafyolder4")
    );

    deploy(
        &deployer,
        "bafynewer4",
        "scene",
        &["30,30"],
        2_000_000.0,
        &[("scene.json", "bafyhashnew4")],
    )
    .await;
    deployer.flush().await.unwrap();

    let newer_id = deployment_id(&pool, "bafynewer4").await;
    assert_eq!(deleter_of(&pool, "bafyolder4").await, Some(newer_id));
    assert_eq!(
        active_pointer(&pool, "30,30").await.as_deref(),
        Some("bafynewer4")
    );
    assert_eq!(active_pointer(&pool, "30,31").await, None);

    db.teardown().await;
}

#[tokio::test]
async fn startup_repair_matches_legacy_successors_for_overlapping_pointer_histories() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();
    sqlx::query(
        "INSERT INTO content.deployments
         (deployer_address, version, entity_type, entity_id, entity_timestamp,
          entity_pointers, local_timestamp, auth_chain, deleter_deployment)
         SELECT 'test', 'v3', CASE WHEN n%7=0 THEN 'profile' ELSE 'scene' END,
                'entity-'||lpad(n::text,5,'0'),
                timestamp '2026-01-01' + (n%19)*interval '1 second',
                CASE WHEN n%23=0 THEN ARRAY[]::text[]
                     WHEN n%29=0 THEN ARRAY[NULL]::text[]
                     ELSE ARRAY['p-'||(n%43), 'p-'||(n%17), 'p-'||(n%43), NULL] END,
                timestamp '2026-01-01', '[]',
                CASE WHEN n%31=0 THEN 9999 ELSE NULL END
         FROM generate_series(1,600) n",
    )
    .execute(&pool)
    .await
    .unwrap();
    let expected: Vec<(i32, Option<i32>)> = sqlx::query_as(
        "SELECT older.id, COALESCE(older.deleter_deployment, (
            SELECT newer.id FROM content.deployments newer
            WHERE newer.entity_type = older.entity_type
              AND newer.entity_id != older.entity_id
              AND newer.entity_pointers && older.entity_pointers
              AND newer.deleter_deployment IS NULL
              AND (newer.entity_timestamp, newer.entity_id)
                    > (older.entity_timestamp, older.entity_id)
            ORDER BY newer.entity_timestamp, newer.entity_id LIMIT 1
         )) FROM content.deployments older ORDER BY older.id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let repo = LiveDeploymentRepository::new(pool.clone(), Arc::new(SyncStatus::default()));
    repo.resolve_deleter_deployments().await.unwrap();
    let actual: Vec<(i32, Option<i32>)> =
        sqlx::query_as("SELECT id, deleter_deployment FROM content.deployments ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(actual, expected);
    repo.resolve_deleter_deployments().await.unwrap();
    let repeated: Vec<(i32, Option<i32>)> =
        sqlx::query_as("SELECT id, deleter_deployment FROM content.deployments ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(repeated, actual);
    db.teardown().await;
}
