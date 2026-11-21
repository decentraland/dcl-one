//! With history pruning on, the retry worker discards a failed entity a newer active entity of
//! its type overtook on any of its pointers: its record goes and no deployment row is written,
//! where it would only have landed as a superseded row the prune removes. With pruning off, or
//! when the newer entity is this node's own scene (whose superseded rows the prune keeps), the
//! entity is deployed as a superseded row. A discard that fails backs off like a failed attempt.
//! A retry cycle walks every due record in its database, so each test runs over a fresh database
//! of its own.

mod common;

use std::sync::Arc;

use catalyst_content::failed_deployments_store::LiveFailedDeploymentsStore;
use catalyst_content::sync::retry_failed::{RetryFailedConfig, RetryFailedDeployments, RetryStats};
use catalyst_content::sync::{DeploymentContext, FailedDeployment, FailureReason};
use catalyst_content::sync_deployer::{LiveSyncDeployer, SyncValidation};
use catalyst_shared::test_keys::{ENTITY_KEY, signed_chain};
use common::{peer, temp_dir};
use sqlx::PgPool;

/// An entity body and its id; `salt` tells apart bodies that are otherwise equal.
fn entity(entity_type: &str, pointers: &[&str], timestamp: i64, salt: u32) -> (Vec<u8>, String) {
    let body = serde_json::to_vec(&serde_json::json!({
        "version": "v3", "type": entity_type, "pointers": pointers, "timestamp": timestamp,
        "content": [], "metadata": { "salt": salt }
    }))
    .unwrap();
    let id = dcl_auth::hash_bytes_v1(&body);
    (body, id)
}

/// [`entity`] named by its CIDv0 (`Qm…`, mixed case) instead of its CIDv1.
fn entity_v0(entity_type: &str, pointers: &[&str], timestamp: i64, salt: u32) -> (Vec<u8>, String) {
    let (body, _) = entity(entity_type, pointers, timestamp, salt);
    let id = dcl_auth::hash_bytes(&body);
    (body, id)
}

fn scene(pointers: &[&str], timestamp: i64) -> (Vec<u8>, String) {
    entity("scene", pointers, timestamp, 0)
}

struct Node {
    pool: PgPool,
    deployer: Arc<LiveSyncDeployer>,
    store: Arc<LiveFailedDeploymentsStore>,
    tmp: std::path::PathBuf,
}

impl Node {
    async fn new() -> Option<Self> {
        let pool = catalyst_db::scratch::isolated_pool(4).await?;
        Some(Self {
            deployer: Arc::new(LiveSyncDeployer::new(
                pool.clone(),
                1,
                SyncValidation::AuthChainOnly,
            )),
            store: Arc::new(LiveFailedDeploymentsStore::new(pool.clone())),
            tmp: temp_dir("retry-overtaken"),
            pool,
        })
    }

    /// Deploys `body` the way sync does: the newer entities the failures meet.
    async fn deploy(&self, (body, id): &(Vec<u8>, String)) {
        self.deployer
            .deploy_entity(
                body,
                id,
                &signed_chain(ENTITY_KEY, id).await,
                DeploymentContext::Synced,
                None,
                None,
            )
            .await
            .unwrap();
        self.deployer.flush().await.unwrap();
    }

    /// Records each entity as a due failed deployment, serves it from a peer and runs one retry
    /// cycle with history pruning on or off.
    async fn retry(&self, failed: &[(Vec<u8>, String, &str)], pruning: bool) -> RetryStats {
        let now = chrono::Utc::now().timestamp_millis();
        for (_, id, entity_type) in failed {
            self.store
                .report_failure(FailedDeployment {
                    entity_type: entity_type.to_string(),
                    entity_id: id.clone(),
                    reason: FailureReason::DeploymentError,
                    auth_chain: signed_chain(ENTITY_KEY, id).await,
                    error_description: "content server unreachable".into(),
                    failure_timestamp: now,
                    snapshot_hash: None,
                    retry_count: 0,
                    next_retry_at: 0,
                })
                .await
                .unwrap();
        }
        let (url, server) = peer(
            failed
                .iter()
                .map(|(body, id, _)| (id.clone(), body.clone()))
                .collect(),
        )
        .await;
        let retry = RetryFailedDeployments::new(
            RetryFailedConfig {
                discard_overtaken: pruning,
                ..Default::default()
            },
            catalyst_content::sync::test_sync_client(reqwest::Client::new()),
            Arc::new(
                catalyst_storage::Storage::from_config(&self.tmp)
                    .await
                    .unwrap(),
            ),
            self.deployer.clone(),
            self.store.clone(),
            Arc::new(tokio::sync::RwLock::new(vec![url])),
        );
        let stats = retry.execute_retry_cycle().await.unwrap();
        self.deployer.flush().await.unwrap();
        server.abort();
        stats
    }

    async fn deployed(&self, id: &str) -> bool {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM content.deployments WHERE entity_id = $1)")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn superseded(&self, id: &str) -> bool {
        sqlx::query_scalar(
            "SELECT deleter_deployment IS NOT NULL FROM content.deployments WHERE entity_id = $1",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    async fn recorded(&self, id: &str) -> bool {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM content.failed_deployments WHERE entity_id = $1)",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    async fn active_on(&self, pointer: &str) -> Option<String> {
        sqlx::query_scalar("SELECT entity_id FROM content.active_pointers WHERE pointer = $1")
            .bind(pointer)
            .fetch_optional(&self.pool)
            .await
            .unwrap()
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.tmp);
    }
}

fn counts(stats: &RetryStats) -> (u64, u64, u64) {
    (stats.discarded, stats.succeeded, stats.still_failing)
}

#[tokio::test]
async fn an_overtaken_failure_is_discarded_and_the_others_are_deployed() {
    let Some(node) = Node::new().await else {
        return;
    };
    // A newer scene holds 10,10 and 10,11.
    let now = chrono::Utc::now().timestamp_millis();
    node.deploy(&scene(&["10,10", "10,11"], now)).await;

    // An older failure sharing one parcel with the newer scene (the other is free), an older
    // one on free parcels only, and one newer than the scene on a parcel it holds.
    let (overtaken, overtaken_id) = scene(&["10,10", "10,12"], now - 60_000);
    let (free, free_id) = scene(&["10,20"], now - 60_000);
    let (later, later_id) = scene(&["10,11"], now + 60_000);
    let stats = node
        .retry(
            &[
                (overtaken, overtaken_id.clone(), "scene"),
                (free, free_id.clone(), "scene"),
                (later, later_id.clone(), "scene"),
            ],
            true,
        )
        .await;
    assert_eq!(counts(&stats), (1, 2, 0));

    assert!(
        !node.deployed(&overtaken_id).await,
        "discarded, not stored as history"
    );
    assert!(
        !node.recorded(&overtaken_id).await,
        "its failure record is gone"
    );
    assert_eq!(
        node.active_on("10,12").await,
        None,
        "the free parcel stays free"
    );
    for (id, pointer) in [(free_id, "10,20"), (later_id, "10,11")] {
        assert!(node.deployed(&id).await);
        assert!(!node.recorded(&id).await);
        assert_eq!(node.active_on(pointer).await, Some(id));
    }
}

#[tokio::test]
async fn with_pruning_off_an_overtaken_failure_is_stored_as_a_superseded_row() {
    let Some(node) = Node::new().await else {
        return;
    };
    let now = chrono::Utc::now().timestamp_millis();
    let newer = scene(&["10,10"], now);
    node.deploy(&newer).await;
    let (overtaken, overtaken_id) = scene(&["10,10", "10,12"], now - 60_000);

    let stats = node
        .retry(&[(overtaken, overtaken_id.clone(), "scene")], false)
        .await;
    assert_eq!(counts(&stats), (0, 1, 0));
    assert!(node.deployed(&overtaken_id).await);
    assert!(node.superseded(&overtaken_id).await, "history, not active");
    assert!(!node.recorded(&overtaken_id).await);
    assert_eq!(node.active_on("10,10").await, Some(newer.1));
}

#[tokio::test]
async fn equal_timestamps_order_by_the_lowercased_entity_id() {
    let Some(node) = Node::new().await else {
        return;
    };
    let now = chrono::Utc::now().timestamp_millis();
    // Two pairs of scenes at one timestamp; in each, `low` sorts before `high` by id.
    let pair = |pointer: &'static str| {
        let mut both = [
            entity("scene", &[pointer], now, 1),
            entity("scene", &[pointer], now, 2),
        ];
        both.sort_by_key(|(_, id)| id.to_lowercase());
        let [low, high] = both;
        (low, high)
    };
    // On 30,30 the higher id is active, so the lower one is overtaken; on 30,31 the lower one
    // is active, so the higher one is newer and takes the pointer.
    let (low, high) = pair("30,30");
    node.deploy(&high).await;
    let (low2, high2) = pair("30,31");
    node.deploy(&low2).await;

    let stats = node
        .retry(
            &[
                (low.0, low.1.clone(), "scene"),
                (high2.0, high2.1.clone(), "scene"),
            ],
            true,
        )
        .await;
    assert_eq!(counts(&stats), (1, 1, 0));
    assert!(!node.deployed(&low.1).await);
    assert_eq!(node.active_on("30,30").await, Some(high.1));
    assert_eq!(node.active_on("30,31").await, Some(high2.1));
}

#[tokio::test]
async fn pointers_compare_lowercased() {
    let Some(node) = Node::new().await else {
        return;
    };
    let now = chrono::Utc::now().timestamp_millis();
    node.deploy(&entity("profile", &["0xabcdef"], now, 0)).await;
    let (older, older_id) = entity("profile", &["0xABCdef"], now - 60_000, 0);

    let stats = node
        .retry(&[(older, older_id.clone(), "profile")], true)
        .await;
    assert_eq!(counts(&stats), (1, 0, 0));
    assert!(!node.deployed(&older_id).await);
    assert!(!node.recorded(&older_id).await);
}

#[tokio::test]
async fn a_newer_entity_of_another_type_does_not_overtake() {
    let Some(node) = Node::new().await else {
        return;
    };
    let now = chrono::Utc::now().timestamp_millis();
    node.deploy(&entity("profile", &["10,10"], now, 0)).await;
    let (older, older_id) = scene(&["10,10"], now - 60_000);

    let stats = node
        .retry(&[(older, older_id.clone(), "scene")], true)
        .await;
    assert_eq!(counts(&stats), (0, 1, 0));
    assert!(node.deployed(&older_id).await);
    assert!(!node.superseded(&older_id).await, "succession is per type");
    assert!(!node.recorded(&older_id).await);
}

#[tokio::test]
async fn a_failure_this_nodes_own_scene_overtook_is_kept_as_history() {
    let Some(node) = Node::new().await else {
        return;
    };
    let now = chrono::Utc::now().timestamp_millis();
    let local = scene(&["10,10"], now);
    node.deploy(&local).await;
    sqlx::query("INSERT INTO content.local_entities (entity_id, signer) VALUES ($1, '0xabc')")
        .bind(&local.1)
        .execute(&node.pool)
        .await
        .unwrap();
    let (older, older_id) = scene(&["10,10"], now - 60_000);

    let stats = node
        .retry(&[(older, older_id.clone(), "scene")], true)
        .await;
    assert_eq!(counts(&stats), (0, 1, 0));
    assert!(node.deployed(&older_id).await);
    assert!(node.superseded(&older_id).await);
    assert_eq!(node.active_on("10,10").await, Some(local.1));
}

#[tokio::test]
async fn a_failed_discard_backs_off_like_a_failed_attempt() {
    let Some(node) = Node::new().await else {
        return;
    };
    let now = chrono::Utc::now().timestamp_millis();
    node.deploy(&scene(&["10,10"], now)).await;
    sqlx::raw_sql(
        "CREATE FUNCTION refuse_delete() RETURNS trigger LANGUAGE plpgsql AS
             $$ BEGIN RAISE EXCEPTION 'refused'; END $$;
         CREATE TRIGGER refuse_delete BEFORE DELETE ON content.failed_deployments
             FOR EACH ROW EXECUTE FUNCTION refuse_delete();",
    )
    .execute(&node.pool)
    .await
    .unwrap();
    let (older, older_id) = scene(&["10,10"], now - 60_000);

    let stats = node
        .retry(&[(older, older_id.clone(), "scene")], true)
        .await;
    assert_eq!(counts(&stats), (0, 0, 1));
    assert!(!node.deployed(&older_id).await);
    let (retry_count, next_retry_at): (i32, f64) = sqlx::query_as(
        "SELECT retry_count, date_part('epoch', next_retry_at) * 1000
         FROM content.failed_deployments WHERE entity_id = $1",
    )
    .bind(&older_id)
    .fetch_one(&node.pool)
    .await
    .unwrap();
    assert_eq!(retry_count, 1, "the attempt is spent");
    assert!(next_retry_at as i64 > now, "and backed off");
}

#[tokio::test]
async fn mixed_case_ids_order_lowercased() {
    let Some(node) = Node::new().await else {
        return;
    };
    let now = chrono::Utc::now().timestamp_millis();
    // At one timestamp a CIDv1 (`bafy…`) sorts before a CIDv0 (`Qm…`) only once lowercased:
    // byte order puts `Q` before `b`.
    node.deploy(&entity("scene", &["40,40"], now, 1)).await;
    let newer_v0 = entity_v0("scene", &["40,40"], now, 2);
    let active_v0 = entity_v0("scene", &["40,41"], now, 3);
    node.deploy(&active_v0).await;
    let (older_v1, older_v1_id) = entity("scene", &["40,41"], now, 4);

    let stats = node
        .retry(
            &[
                (newer_v0.0, newer_v0.1.clone(), "scene"),
                (older_v1, older_v1_id.clone(), "scene"),
            ],
            true,
        )
        .await;
    assert_eq!(counts(&stats), (1, 1, 0));
    assert_eq!(node.active_on("40,40").await, Some(newer_v0.1));
    assert!(!node.deployed(&older_v1_id).await);
    assert_eq!(node.active_on("40,41").await, Some(active_v0.1));
}

#[tokio::test]
async fn an_active_pointer_row_without_a_type_still_overtakes() {
    let Some(node) = Node::new().await else {
        return;
    };
    let now = chrono::Utc::now().timestamp_millis();
    node.deploy(&scene(&["10,10"], now)).await;
    // Succession reads the deployment's type, not the pointer row's.
    sqlx::query("UPDATE content.active_pointers SET entity_type = NULL WHERE pointer = '10,10'")
        .execute(&node.pool)
        .await
        .unwrap();
    let (older, older_id) = scene(&["10,10"], now - 60_000);

    let stats = node
        .retry(&[(older, older_id.clone(), "scene")], true)
        .await;
    assert_eq!(counts(&stats), (1, 0, 0));
    assert!(!node.deployed(&older_id).await);
}

#[tokio::test]
async fn a_timestamp_postgres_cannot_hold_is_left_to_validation() {
    let Some(node) = Node::new().await else {
        return;
    };
    node.deploy(&scene(&["10,10"], chrono::Utc::now().timestamp_millis()))
        .await;
    for timestamp in [-1e20, 1e20] {
        let body = serde_json::to_vec(&serde_json::json!({
            "version": "v3", "type": "scene", "pointers": ["10,10"], "timestamp": timestamp,
            "content": [], "metadata": {}
        }))
        .unwrap();
        let id = dcl_auth::hash_bytes_v1(&body);
        assert!(
            !node.deployer.is_overtaken(&body, &id).await.unwrap(),
            "{timestamp}"
        );
    }
}

#[tokio::test]
async fn a_stored_file_that_cannot_be_hash_checked_is_not_discarded() {
    let Some(node) = Node::new().await else {
        return;
    };
    let now = chrono::Utc::now().timestamp_millis();
    node.deploy(&scene(&["10,10"], now)).await;
    let (older, older_hash) = scene(&["10,10"], now - 60_000);
    // A storable id of no IPFS shape (a CIDv1 is 59 characters), so the stored file cannot be
    // checked against it.
    let id = format!("{older_hash}a");
    catalyst_storage::Storage::from_config(&node.tmp)
        .await
        .unwrap()
        .store(&id, older.clone().into())
        .await
        .unwrap();

    let stats = node.retry(&[(older, id.clone(), "scene")], true).await;
    assert_eq!(stats.discarded, 0);
    assert!(node.deployed(&id).await || node.recorded(&id).await);
}
