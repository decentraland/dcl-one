//! A retried deployment keeps its failure record until the flush that writes it commits: the
//! retry worker deleted the record as soon as the entity was buffered, so a crash before the
//! flush, or a flush that failed and could not record it again, lost the entity for good. A retry cycle walks every due record in its database, so this runs
//! in a binary of its own over a scratch database of its own.

mod common;

use std::sync::Arc;

use catalyst_content::failed_deployments_store::LiveFailedDeploymentsStore;
use catalyst_content::sync::retry_failed::{RetryFailedConfig, RetryFailedDeployments};
use catalyst_content::sync::{FailedDeployment, FailureReason};
use catalyst_content::sync_deployer::LiveSyncDeployer;
use catalyst_shared::test_keys::{ENTITY_KEY, signed_chain};
use common::{peer, temp_dir};

#[tokio::test]
async fn a_retried_deployment_keeps_its_record_until_its_write_commits() {
    let Some(pool) = catalyst_db::scratch::pool(4).await else {
        return;
    };
    let body = br#"{"version":"v3","type":"scene","pointers":["retry:0,0"],"timestamp":1000,"content":[],"metadata":{"z":9007199254740993,"a":{"z":1,"a":2}}}"#.to_vec();
    let original = body.clone();
    let entity_id = dcl_auth::hash_bytes_v1(&body);

    let (url, server) = peer(vec![(entity_id.clone(), body)]).await;

    let store = Arc::new(LiveFailedDeploymentsStore::new(pool.clone()));
    store
        .report_failure(FailedDeployment {
            entity_type: "scene".into(),
            entity_id: entity_id.clone(),
            reason: FailureReason::DeploymentError,
            auth_chain: signed_chain(ENTITY_KEY, &entity_id).await,
            error_description: "content server unreachable".into(),
            failure_timestamp: 1_700_000_000_000,
            snapshot_hash: None,
            retry_count: 0,
            next_retry_at: 0,
        })
        .await
        .unwrap();

    let tmp = temp_dir("retry-record");
    let storage = Arc::new(catalyst_storage::Storage::from_config(&tmp).await.unwrap());
    let deployer = Arc::new(LiveSyncDeployer::new(
        pool.clone(),
        1,
        catalyst_content::sync_deployer::SyncValidation::AuthChainOnly,
    ));
    let retry = RetryFailedDeployments::new(
        RetryFailedConfig::default(),
        catalyst_content::sync::test_sync_client(reqwest::Client::new()),
        storage.clone(),
        deployer.clone(),
        store.clone(),
        Arc::new(tokio::sync::RwLock::new(vec![url.clone()])),
    );
    let count = |sql: &'static str| {
        let pool = pool.clone();
        let entity_id = entity_id.clone();
        async move {
            sqlx::query_scalar::<_, i64>(sql)
                .bind(entity_id)
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    let recorded = "SELECT count(*) FROM content.failed_deployments WHERE entity_id = $1";
    let deployed = "SELECT count(*) FROM content.deployments WHERE entity_id = $1";

    let stats = retry.execute_retry_cycle().await.unwrap();
    assert_eq!(stats.succeeded, 1);
    assert_eq!(count(deployed).await, 0, "the retry only buffered it");
    assert_eq!(
        count(recorded).await,
        1,
        "a buffered deployment is not durable: its record stays"
    );

    // Lose the in-memory batch, then rebuild from the durable record and bytes.
    drop(retry);
    drop(deployer);
    assert_eq!(
        storage
            .retrieve(&entity_id)
            .await
            .unwrap()
            .unwrap()
            .as_ref(),
        original
    );
    let deployer = Arc::new(LiveSyncDeployer::new(
        pool.clone(),
        1,
        catalyst_content::sync_deployer::SyncValidation::AuthChainOnly,
    ));
    let retry = RetryFailedDeployments::new(
        RetryFailedConfig::default(),
        catalyst_content::sync::test_sync_client(reqwest::Client::new()),
        storage,
        deployer.clone(),
        store,
        Arc::new(tokio::sync::RwLock::new(vec![url])),
    );
    sqlx::query(
        "UPDATE content.failed_deployments SET next_retry_at=to_timestamp(0) WHERE entity_id=$1",
    )
    .bind(&entity_id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(retry.execute_retry_cycle().await.unwrap().succeeded, 1);
    assert_eq!(count(deployed).await, 0);
    assert_eq!(count(recorded).await, 1);
    deployer.flush().await.unwrap();
    assert_eq!(count(deployed).await, 1);
    assert_eq!(
        count(recorded).await,
        0,
        "the transaction that wrote it cleared the record"
    );

    let metadata: String = sqlx::query_scalar(
        "SELECT entity_metadata::text FROM content.deployments WHERE entity_id=$1",
    )
    .bind(&entity_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        metadata,
        r#"{"v":{"z":9007199254740993,"a":{"z":1,"a":2}}}"#
    );
    server.abort();
    let _ = tokio::fs::remove_dir_all(&tmp).await;
    pool.close().await;
}
