//! The sync client ignores the proxy environment, so its address check applies
//! to the peer it dials. The environment is process-wide, so this runs in a
//! binary of its own.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

#[tokio::test]
async fn the_sync_client_ignores_the_proxy_environment() {
    let proxy = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy.local_addr().unwrap();
    let proxied = Arc::new(AtomicUsize::new(0));
    let counted = proxied.clone();
    tokio::spawn(async move {
        while let Ok((socket, _)) = proxy.accept().await {
            counted.fetch_add(1, SeqCst);
            drop(socket);
        }
    });
    let peer = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let source = format!("http://localhost:{}", peer.local_addr().unwrap().port());
    let app = axum::Router::new().route("/ok", axum::routing::get(|| async { "ok" }));
    tokio::spawn(async move { axum::serve(peer, app).await.unwrap() });
    // SAFETY: the only test in this binary, set before any client is built.
    unsafe {
        for var in ["http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY"] {
            std::env::set_var(var, format!("http://{proxy_addr}"));
        }
        std::env::remove_var("NO_PROXY");
        std::env::remove_var("no_proxy");
    }
    let config = catalyst_config::config::Config::parse_for_tests(&[
        "x",
        "--database-url",
        "postgres:///catalyst",
        "--sync-source",
        &source,
    ])
    .unwrap();
    let client = catalyst_content::sync::sync_http_client(&config).unwrap();

    let resp = client.get(format!("{source}/ok")).send().await.unwrap();
    assert_eq!(resp.text().await.unwrap(), "ok");
    assert_eq!(proxied.load(SeqCst), 0);
}
