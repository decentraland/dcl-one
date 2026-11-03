//! The retry worker keeps an entity the validator could not decide for as long as it stays
//! undecided, past the retry cap, and never retries one the validator refused. A retry cycle
//! walks every due record in its database, so this runs in a binary of its own over a scratch
//! database of its own.

use std::sync::Arc;

use alloy::signers::{Signer, local::PrivateKeySigner};
use catalyst_content::sync::retry_failed::{RetryFailedConfig, RetryFailedDeployments};
use catalyst_content::sync::{AuthChain, AuthLink, AuthLinkType, FailedDeployment, FailureReason};
use catalyst_content::sync_deployer::{
    LiveFailedDeploymentsStore, LiveSyncDeployer, SyncValidation,
};
use catalyst_content::validator::{Settings, Sources};

const TEST_KEY: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
const MAX_RETRIES: u32 = 2;

async fn signed_chain(entity_id: &str) -> AuthChain {
    let key: PrivateKeySigner = TEST_KEY.parse().unwrap();
    let signature = key.sign_message(entity_id.as_bytes()).await.unwrap();
    vec![
        AuthLink {
            link_type: AuthLinkType::SIGNER,
            payload: format!("{:#x}", key.address()),
            signature: None,
        },
        AuthLink {
            link_type: AuthLinkType::EcdsaSignedEntity,
            payload: entity_id.to_string(),
            signature: Some(signature.to_string()),
        },
    ]
}

/// Serves `body` as every content file.
async fn peer(body: Vec<u8>) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let app = axum::Router::new().route(
        "/contents/{hash}",
        axum::routing::get(move || {
            let body = body.clone();
            async move { body }
        }),
    );
    (
        url,
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() }),
    )
}

#[derive(Debug, sqlx::FromRow)]
struct Record {
    reason: String,
    retry_count: i32,
    /// `next_retry_at - failure_time`, ms.
    wait_ms: f64,
}

#[tokio::test]
async fn an_undecided_entity_outlives_the_cap_and_a_refused_one_is_never_retried() {
    let Some(pool) = catalyst_db::scratch::pool(4).await else {
        return;
    };
    let tmp = std::env::temp_dir().join(format!(
        "catalyst-retry-undecided-{}",
        uuid::Uuid::new_v4().simple()
    ));
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
        "content": [], "metadata": {"scene": {"base": "40,-40", "parcels": ["40,-40"]}}
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
                "SELECT reason, retry_count, \
                 (date_part('epoch', next_retry_at - failure_time) * 1000)::float8 AS wait_ms \
                 FROM content.failed_deployments WHERE entity_id = $1",
            )
            .bind(entity_id)
            .fetch_optional(&pool)
            .await
            .unwrap()
        }
    };
    let make_due = || {
        let pool = pool.clone();
        async move {
            sqlx::query(
                "UPDATE content.failed_deployments \
                 SET next_retry_at = next_retry_at - make_interval(days => 30), \
                     failure_time = failure_time - make_interval(days => 30)",
            )
            .execute(&pool)
            .await
            .unwrap();
        }
    };

    for (body, label) in [(&undecided, "undecided"), (&refused, "refused")] {
        let entity_id = dcl_auth::hash_bytes_v1(body);
        let (url, server) = peer(body.clone()).await;
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
                auth_chain: signed_chain(&entity_id).await,
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
            reqwest::Client::new(),
            storage.clone(),
            Arc::new(LiveSyncDeployer::new(
                pool.clone(),
                1,
                SyncValidation::Content(sources.clone()),
            )),
            store.clone(),
            Arc::new(tokio::sync::RwLock::new(vec![url])),
        );

        if label == "undecided" {
            let mut waits = Vec::new();
            for cycle in 0..MAX_RETRIES + 3 {
                let stats = retry.execute_retry_cycle().await.unwrap();
                assert_eq!(
                    (stats.still_failing, stats.evicted),
                    (1, 0),
                    "cycle {cycle}"
                );
                let record = read(entity_id.clone()).await.expect("still queued");
                assert_eq!(record.reason, "Validation undecided", "cycle {cycle}");
                assert_eq!(record.retry_count, 0, "cycle {cycle}: nothing spent");
                waits.push(record.wait_ms.round() as i64);
                make_due().await;
            }
            assert_eq!(
                waits,
                [900_000, 1_800_000, 3_600_000, 7_200_000, 14_400_000],
                "the backoff still grows"
            );
            // The prune spares it too.
            catalyst_content::sync::retry_failed::prune_failed_deployments(&pool, 7)
                .await
                .unwrap();
            assert!(read(entity_id.clone()).await.is_some());
        } else {
            let stats = retry.execute_retry_cycle().await.unwrap();
            assert_eq!(stats.still_failing, 1);
            let record = read(entity_id.clone()).await.expect("recorded");
            assert_eq!(record.reason, "Deployment rejected");
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
