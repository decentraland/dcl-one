//! `catalyst serve` joins every area's routes into one table before it binds
//! its port, and axum refuses a path two areas register. This starts the
//! binary on the scratch database and waits for routes of several areas to
//! answer, so a collision fails here instead of when a node starts.
#![cfg(unix)]
use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::process::Command;

async fn free_port() -> u16 {
    tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[tokio::test]
async fn serve_joins_every_areas_routes_and_answers() {
    let Some(url) = catalyst::db::scratch::url().await else {
        return;
    };
    let (api, metrics) = (free_port().await, free_port().await);
    let root = std::env::temp_dir().join(format!(
        "catalyst-serve-routes-{}-{api}",
        std::process::id()
    ));
    let dir = |name: &str| root.join(name).display().to_string();
    let mut child = Command::new(env!("CARGO_BIN_EXE_catalyst"))
        .arg("serve")
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("NODE_POSTURE", "journey")
        .env("DATABASE_URL", &url)
        .env("PG_POOL_SIZE", "4")
        .env("HTTP_SERVER_HOST", "127.0.0.1")
        .env("HTTP_SERVER_PORT", api.to_string())
        .env("SERVE_METRICS_BIND", format!("127.0.0.1:{metrics}"))
        .env("PUBLIC_URL", format!("http://127.0.0.1:{api}"))
        .env("STORAGE_ROOT_FOLDER", dir("content"))
        .env("AB_CDN_CACHE_DIR", dir("ab-cdn"))
        .env("PROFILE_IMAGES_CACHE_DIR", dir("profile-images"))
        .env("BVIMPOSTERS_STORE_ROOT", dir("imposters"))
        .env("SESSION_SECRET", "a".repeat(64))
        .env("ENCRYPTION_KEY", "b".repeat(64))
        .env("RUST_LOG", "warn")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let client = reqwest::Client::new();
    let mut answered = Vec::new();
    for _ in 0..600 {
        if let Some(status) = child.try_wait().unwrap() {
            let mut stderr = String::new();
            child
                .stderr
                .take()
                .unwrap()
                .read_to_string(&mut stderr)
                .await
                .unwrap();
            let _ = std::fs::remove_dir_all(&root);
            panic!("serve exited with {status} before answering: {stderr}");
        }
        answered.clear();
        for path in [
            "/builder-api/ping",
            "/builder-api/v1/info",
            "/v1/info",
            "/shop-api/v1/campaign-collections?tag=x",
        ] {
            if let Ok(response) = client
                .get(format!("http://127.0.0.1:{api}{path}"))
                .send()
                .await
            {
                answered.push((path, response.status().as_u16()));
            }
        }
        if answered.len() == 4 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    child.kill().await.unwrap();
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(answered.len(), 4, "serve never answered: {answered:?}");
    assert!(
        answered.iter().all(|(_, status)| *status != 404),
        "{answered:?}"
    );
}
