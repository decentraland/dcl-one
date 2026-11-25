//! The root probe on an actual serve replica and its configured service checks.
use std::{process::Stdio, time::Duration};

use catalyst::db::scratch::{OwnedDatabase, Schema};
use reqwest::StatusCode;
use serde_json::{Value, json};
use tokio::process::{Child, Command};

async fn wait_for(
    child: &mut Child,
    client: &reqwest::Client,
    url: &str,
    content_status: Option<&str>,
) -> Value {
    tokio::time::timeout(Duration::from_secs(45), async {
        loop {
            assert!(
                child.try_wait().unwrap().is_none(),
                "serve exited before {url} reported content {content_status:?}"
            );
            if let Ok(resp) = client.get(url).send().await {
                assert_eq!(resp.status(), StatusCode::OK);
                let report: Value = resp.json().await.unwrap();
                if report["details"]["content"]["status"].as_str() == content_status {
                    return report;
                }
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .expect("timed out waiting for serve readiness")
}

#[tokio::test]
async fn node_readiness_uses_the_configured_services_on_a_real_replica() {
    let Some(db) = OwnedDatabase::new("readiness", Schema::Full, 2).await else {
        return;
    };
    let root = std::env::temp_dir().join(format!("catalyst-readiness-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(4))
        .build()
        .unwrap();
    for standalone in [true, false] {
        let port = std::net::TcpListener::bind(("127.0.0.1", 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let url = format!("http://127.0.0.1:{port}/health");
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_catalyst"));
        cmd.arg("serve")
            .env_clear()
            .current_dir(&root)
            .env("DATABASE_URL", &db.url)
            .env("HTTP_SERVER_HOST", "127.0.0.1")
            .env("HTTP_SERVER_PORT", port.to_string())
            .env("SERVE_METRICS_BIND", "127.0.0.1:0")
            .env("FLEET_JOBS_ENABLED", "false")
            .env("LIVEKIT_ALLOW_DEV_CREDS", "true")
            .env("ENCRYPTION_KEY", "07".repeat(32))
            .env("SESSION_SECRET", uuid::Uuid::new_v4().simple().to_string())
            .env("SERVE_UNSYNCED", standalone.to_string())
            .env("AB_CDN_CACHE_DIR", root.join("cdn"))
            .env("STORAGE_ROOT_FOLDER", root.join("content"))
            .env("BVIMPOSTERS_STORE_ROOT", root.join("imposters"))
            .env("PROFILE_IMAGES_CACHE_DIR", root.join("profiles"))
            .kill_on_drop(true)
            .stdout(Stdio::null());
        if !standalone {
            cmd.env("COMMS_FIXED_ADAPTER", "archipelago:ws://127.0.0.1/ws");
        }
        let mut child = cmd.spawn().unwrap();
        let mut expected = json!({
            "status": "pass",
            "details": {
                "server": {"required": true, "status": "pass"},
                "postgres": {"required": false, "status": "pass"},
                "social-rpc": {"required": true, "status": "pass"}
            }
        });
        if !standalone {
            expected["details"]["comms"] = json!({"required": false, "status": "fail"});
            expected["details"]["content"] = json!({"required": false, "status": "fail"});
            assert_eq!(
                wait_for(&mut child, &client, &url, Some("fail")).await,
                expected
            );
            let sync = catalyst::content::SyncStatus::default();
            sync.set_state(catalyst::content::SyncState::Syncing);
            catalyst::content::sync::status_mirror::publish(&db.pool, &sync, "test-leader")
                .await
                .unwrap();
            expected["details"]["content"] = json!({"required": false, "status": "pass"});
        }
        assert_eq!(
            wait_for(&mut child, &client, &url, (!standalone).then_some("pass")).await,
            expected
        );
        child.kill().await.unwrap();
    }
    db.drop().await;
    std::fs::remove_dir_all(root).unwrap();
}
