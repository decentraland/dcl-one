//! The retry worker keeps an entity the validator could not decide for up to a week, past the
//! retry cap and the prune, and never retries one the validator refused. A retry cycle
//! walks every due record in its database, so this runs in a binary of its own over a scratch
//! database of its own.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    reason = "test fixtures use small literal values"
)]
#![allow(clippy::unwrap_used, reason = "test support")]

mod common;

use std::sync::Arc;

use catalyst_content::failed_deployments_store::LiveFailedDeploymentsStore;
use catalyst_content::sync::retry_failed::{RetryFailedConfig, RetryFailedDeployments};
use catalyst_content::sync::{FailedDeployment, FailureReason};
use catalyst_content::sync_deployer::{LiveSyncDeployer, SyncValidation};
use catalyst_content::validator::{Settings, Sources};
use catalyst_shared::test_keys::{ENTITY_KEY, signed_chain};
use common::{peer, temp_dir};

const MAX_RETRIES: u32 = 2;

#[derive(Debug, sqlx::FromRow)]
struct Record {
    reason: String,
    retry_count: i32,
    error_description: String,
    /// `next_retry_at - now()`, minutes.
    wait_min: f64,
    /// `now() - failure_time`, minutes.
    age_min: f64,
}

#[tokio::test]
async fn an_undecided_entity_outlives_the_cap_and_a_refused_one_is_never_retried() {
    let Some(pool) = catalyst_db::scratch::pool(4).await else {
        return;
    };
    let tmp = temp_dir("retry-undecided");
    let storage = Arc::new(catalyst_storage::Storage::from_config(&tmp).await.unwrap());
    // No L1 RPC: a scene's LAND access cannot be decided.
    let sources = Arc::new(Sources::new(
        pool.clone(),
        storage.clone(),
        dcl_http::Outbound::for_tests(),
        None,
        Settings {
            eth_network: "mainnet".into(),
            ..Default::default()
        },
    ));
    let timestamp = chrono::Utc::now().timestamp_millis() - 86_400_000;
    let undecided = serde_json::to_vec(&serde_json::json!({
        "version": "v3", "type": "scene", "pointers": ["40,-40"], "timestamp": timestamp,
        "content": [],
        "metadata": {"main": "bin/index.js", "scene": {"base": "40,-40", "parcels": ["40,-40"]}}
    }))
    .unwrap();
    let refused = serde_json::to_vec(&serde_json::json!({
        "version": "v3", "type": "outfits",
        "pointers": [format!("0x{}:outfits", "ab".repeat(20))], "timestamp": timestamp,
        "content": [], "metadata": {"outfits": [], "namesForExtraSlots": []}
    }))
    .unwrap();

    let store = Arc::new(LiveFailedDeploymentsStore::new(pool.clone()));
    let read = |entity_id: String| {
        let pool = pool.clone();
        async move {
            sqlx::query_as::<_, Record>(
                "SELECT reason, retry_count, error_description, \
                 (date_part('epoch', next_retry_at - timezone('UTC', now())) / 60)::float8 AS wait_min, \
                 (date_part('epoch', timezone('UTC', now()) - failure_time) / 60)::float8 AS age_min \
                 FROM content.failed_deployments WHERE entity_id = $1",
            )
            .bind(entity_id)
            .fetch_optional(&pool)
            .await
            .unwrap()
        }
    };
    // Moves every record back by `extra` minutes past its wait: its wait served and then some.
    // The extra second keeps it due against the worker's millisecond clock.
    let elapse = |extra: i32| {
        let pool = pool.clone();
        async move {
            sqlx::query(
                "UPDATE content.failed_deployments \
                 SET failure_time = failure_time - (next_retry_at - timezone('UTC', now())) \
                                    - make_interval(mins => $1), \
                     next_retry_at = timezone('UTC', now()) - make_interval(mins => $1, secs => 1)",
            )
            .bind(extra)
            .execute(&pool)
            .await
            .unwrap();
        }
    };

    for (body, label) in [(&undecided, "undecided"), (&refused, "refused")] {
        let entity_id = dcl_auth::hash_bytes_v1(body);
        let (url, server) = peer(vec![(entity_id.clone(), body.clone())]).await;
        store
            .report_failure(FailedDeployment {
                entity_type: if label == "undecided" {
                    "scene"
                } else {
                    "outfits"
                }
                .into(),
                entity_id: entity_id.clone(),
                reason: FailureReason::DeploymentError,
                auth_chain: signed_chain(ENTITY_KEY, &entity_id).await,
                error_description: "content server unreachable".into(),
                failure_timestamp: chrono::Utc::now().timestamp_millis(),
                snapshot_hash: None,
                retry_count: 0,
                next_retry_at: 0,
            })
            .await
            .unwrap();
        let retry = RetryFailedDeployments::new(
            RetryFailedConfig {
                max_retries: MAX_RETRIES,
                ..Default::default()
            },
            catalyst_content::sync::test_sync_client(reqwest::Client::new()),
            storage.clone(),
            Arc::new(LiveSyncDeployer::new(
                pool.clone(),
                storage.clone(),
                1,
                SyncValidation::Content(sources.clone()),
            )),
            store.clone(),
            Arc::new(tokio::sync::RwLock::new(vec![url])),
        );

        if label == "undecided" {
            let mut waits = Vec::new();
            for cycle in 0..5 {
                let stats = retry.execute_retry_cycle().await.unwrap();
                assert_eq!(
                    (stats.still_failing, stats.evicted),
                    (1, 0),
                    "cycle {cycle}"
                );
                let record = read(entity_id.clone()).await.expect("still queued");
                assert_eq!(record.reason, "Validation undecided", "cycle {cycle}");
                assert_eq!(record.retry_count, 0, "cycle {cycle}: nothing spent");
                waits.push(record.wait_min.round() as i64);
                elapse(0).await;
            }
            assert_eq!(waits, [15, 15, 30, 60, 120], "the backoff doubles");

            // Sync reports it undecided again: the first failure and the deadline stay.
            let before = read(entity_id.clone()).await.unwrap();
            store
                .report_failure(FailedDeployment {
                    entity_type: "scene".into(),
                    entity_id: entity_id.clone(),
                    reason: FailureReason::ValidationUndecided,
                    auth_chain: signed_chain(ENTITY_KEY, &entity_id).await,
                    error_description: "undecided".into(),
                    failure_timestamp: chrono::Utc::now().timestamp_millis(),
                    snapshot_hash: None,
                    retry_count: 0,
                    next_retry_at: 0,
                })
                .await
                .unwrap();
            let after = read(entity_id.clone()).await.unwrap();
            assert_eq!(after.age_min.round(), before.age_min.round());
            assert_eq!(after.wait_min.round(), before.wait_min.round());
            retry.execute_retry_cycle().await.unwrap();
            let record = read(entity_id.clone()).await.unwrap();
            assert_eq!(record.wait_min.round(), 240.0, "the backoff kept growing");

            // Past the retry cap and the prune it stays.
            sqlx::query("UPDATE content.failed_deployments SET retry_count = $1")
                .bind(MAX_RETRIES as i32 + 5)
                .execute(&pool)
                .await
                .unwrap();
            elapse(0).await;
            let stats = retry.execute_retry_cycle().await.unwrap();
            assert_eq!((stats.still_failing, stats.evicted), (1, 0));
            catalyst_content::failed_deployments_store::prune_failed_deployments(&pool, 0)
                .await
                .unwrap();
            let record = read(entity_id.clone())
                .await
                .expect("never evicted or pruned");
            assert_eq!(record.reason, "Validation undecided");

            // A week undecided: given up, recorded as a refusal.
            elapse(7 * 24 * 60).await;
            let stats = retry.execute_retry_cycle().await.unwrap();
            assert_eq!(stats.still_failing, 1);
            let record = read(entity_id.clone()).await.expect("recorded");
            assert_eq!(record.reason, "Deployment rejected");
            assert!(
                record
                    .error_description
                    .starts_with("Gave up after 7 days undecided:"),
                "{}",
                record.error_description
            );
            assert!(record.age_min < 1.0, "the prune counts from the give-up");
            elapse(0).await;
            let stats = retry.execute_retry_cycle().await.unwrap();
            assert_eq!(
                (stats.still_failing, stats.evicted),
                (0, 0),
                "never retried"
            );
        } else {
            let stats = retry.execute_retry_cycle().await.unwrap();
            assert_eq!(stats.still_failing, 1);
            let record = read(entity_id.clone()).await.expect("recorded");
            assert_eq!(record.reason, "Deployment rejected");
            assert!(
                record
                    .error_description
                    .contains("You can only alter your own outfits"),
                "{}",
                record.error_description
            );
            sqlx::query(
                "UPDATE content.failed_deployments SET next_retry_at = '1970-01-01' \
                 WHERE entity_id = $1",
            )
            .bind(&entity_id)
            .execute(&pool)
            .await
            .unwrap();
            let stats = retry.execute_retry_cycle().await.unwrap();
            assert_eq!(
                (stats.succeeded, stats.still_failing, stats.evicted),
                (0, 0, 0),
                "a refusal is not retried"
            );
            assert!(
                read(entity_id.clone()).await.is_some(),
                "and stays recorded"
            );
        }
        sqlx::query("DELETE FROM content.failed_deployments WHERE entity_id = $1")
            .bind(&entity_id)
            .execute(&pool)
            .await
            .unwrap();
        server.abort();
    }

    let _ = tokio::fs::remove_dir_all(&tmp).await;
    pool.close().await;
}
