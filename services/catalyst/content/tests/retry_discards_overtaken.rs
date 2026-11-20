//! The retry worker discards a failed entity a newer active entity overtook on any of its
//! pointers: its record goes and no deployment row is written, where it would only have landed
//! as a superseded row. A failed entity no newer entity overtook is deployed as before. A retry
//! cycle walks every due record in its database, so this runs in a binary of its own over a
//! scratch database of its own.

use std::sync::Arc;

use alloy::signers::{Signer, local::PrivateKeySigner};
use catalyst_content::failed_deployments_store::LiveFailedDeploymentsStore;
use catalyst_content::sync::retry_failed::{RetryFailedConfig, RetryFailedDeployments};
use catalyst_content::sync::{
    AuthChain, AuthLink, AuthLinkType, DeploymentContext, FailedDeployment, FailureReason,
};
use catalyst_content::sync_deployer::{LiveSyncDeployer, SyncValidation};

const TEST_KEY: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

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

/// Serves each body at `/contents/{its hash}`.
async fn peer(bodies: Vec<Vec<u8>>) -> (String, tokio::task::JoinHandle<()>) {
    let files: Arc<std::collections::HashMap<String, Vec<u8>>> = Arc::new(
        bodies
            .into_iter()
            .map(|b| (dcl_auth::hash_bytes_v1(&b), b))
            .collect(),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let app = axum::Router::new().route(
        "/contents/{hash}",
        axum::routing::get(
            move |axum::extract::Path(hash): axum::extract::Path<String>| {
                let files = files.clone();
                async move {
                    files
                        .get(&hash)
                        .cloned()
                        .ok_or(axum::http::StatusCode::NOT_FOUND)
                }
            },
        ),
    );
    (
        url,
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() }),
    )
}

fn scene(pointers: &[&str], timestamp: i64) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "version": "v3", "type": "scene", "pointers": pointers, "timestamp": timestamp,
        "content": [], "metadata": null
    }))
    .unwrap()
}

#[tokio::test]
async fn an_overtaken_failure_is_discarded_and_the_others_are_deployed() {
    let Some(pool) = catalyst_db::scratch::pool(4).await else {
        return;
    };
    let tmp = std::env::temp_dir().join(format!(
        "catalyst-retry-overtaken-{}",
        uuid::Uuid::new_v4().simple()
    ));
    let storage = Arc::new(catalyst_storage::Storage::from_config(&tmp).await.unwrap());
    let deployer = Arc::new(LiveSyncDeployer::new(
        pool.clone(),
        1,
        SyncValidation::AuthChainOnly,
    ));

    // A newer scene holds 10,10 and 10,11.
    let now = chrono::Utc::now().timestamp_millis();
    let newer = scene(&["10,10", "10,11"], now);
    let newer_id = dcl_auth::hash_bytes_v1(&newer);
    deployer
        .deploy_entity(
            &newer,
            &newer_id,
            &signed_chain(&newer_id).await,
            DeploymentContext::Synced,
            None,
            None,
        )
        .await
        .unwrap();
    deployer.flush().await.unwrap();

    // An older failure sharing one parcel with the newer scene (the other is free), an older
    // one on free parcels only, and one newer than the scene on a parcel it holds.
    let overtaken = scene(&["10,10", "10,12"], now - 60_000);
    let overtaken_id = dcl_auth::hash_bytes_v1(&overtaken);
    let free = scene(&["10,20"], now - 60_000);
    let free_id = dcl_auth::hash_bytes_v1(&free);
    let later = scene(&["10,11"], now + 60_000);
    let later_id = dcl_auth::hash_bytes_v1(&later);

    let store = Arc::new(LiveFailedDeploymentsStore::new(pool.clone()));
    for id in [&overtaken_id, &free_id, &later_id] {
        store
            .report_failure(FailedDeployment {
                entity_type: "scene".into(),
                entity_id: id.clone(),
                reason: FailureReason::DeploymentError,
                auth_chain: signed_chain(id).await,
                error_description: "content server unreachable".into(),
                failure_timestamp: now,
                snapshot_hash: None,
                retry_count: 0,
                next_retry_at: 0,
            })
            .await
            .unwrap();
    }

    let (url, server) = peer(vec![overtaken, free, later]).await;
    let retry = RetryFailedDeployments::new(
        RetryFailedConfig::default(),
        catalyst_content::sync::test_sync_client(reqwest::Client::new()),
        storage.clone(),
        deployer.clone(),
        store.clone(),
        Arc::new(tokio::sync::RwLock::new(vec![url])),
    );
    let stats = retry.execute_retry_cycle().await.unwrap();
    deployer.flush().await.unwrap();
    assert_eq!(
        (stats.discarded, stats.succeeded, stats.still_failing),
        (1, 2, 0)
    );

    let deployed = |id: String| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM content.deployments WHERE entity_id = $1)",
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };
    let recorded = |id: String| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM content.failed_deployments WHERE entity_id = $1)",
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };
    assert!(
        !deployed(overtaken_id.clone()).await,
        "discarded, not stored as history"
    );
    assert!(!recorded(overtaken_id).await, "its failure record is gone");
    let active_on = |pointer: &'static str| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, String>(
                "SELECT entity_id FROM content.active_pointers WHERE pointer = $1",
            )
            .bind(pointer)
            .fetch_optional(&pool)
            .await
            .unwrap()
        }
    };
    assert_eq!(active_on("10,12").await, None, "the free parcel stays free");
    for (id, pointer) in [(free_id, "10,20"), (later_id, "10,11")] {
        assert!(deployed(id.clone()).await);
        assert!(!recorded(id.clone()).await);
        assert_eq!(active_on(pointer).await, Some(id));
    }

    server.abort();
    let _ = std::fs::remove_dir_all(tmp);
}
