#![cfg(unix)]
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};

#[tokio::test]
async fn index_process_restarts_without_binding_or_stopping_the_api_port() {
    let Some(url) = catalyst::db::scratch::url().await else {
        return;
    };
    let api = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let address = api.local_addr().unwrap();
    let mut previous = None;
    for _ in 0..2 {
        let mut child = Command::new(env!("CARGO_BIN_EXE_catalyst"))
            .arg("index")
            .env_clear()
            .env("DATABASE_URL", &url)
            .env("HTTP_SERVER_HOST", address.ip().to_string())
            .env("HTTP_SERVER_PORT", address.port().to_string())
            .env("PG_POOL_SIZE", "2")
            .env("RUST_LOG", "info")
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let pid = child.id().unwrap();
        assert_ne!(previous, Some(pid));
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        tokio::time::timeout(Duration::from_secs(30), async {
            while let Some(line) = lines.next_line().await.unwrap() {
                if line.contains("index worker started") {
                    return;
                }
            }
            panic!("index process exited before startup");
        })
        .await
        .expect("index process startup timeout");
        assert!(child.try_wait().unwrap().is_none());
        tokio::net::TcpStream::connect(address).await.unwrap();
        assert!(
            Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .status()
                .await
                .unwrap()
                .success()
        );
        assert!(
            tokio::time::timeout(Duration::from_secs(40), child.wait())
                .await
                .unwrap()
                .unwrap()
                .success()
        );
        tokio::net::TcpStream::connect(address).await.unwrap();
        previous = Some(pid);
    }
}
