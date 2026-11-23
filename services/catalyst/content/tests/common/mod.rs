//! Helpers the retry test binaries share.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// A peer serving each body at `/contents/{its id}`.
pub async fn peer(files: Vec<(String, Vec<u8>)>) -> (String, tokio::task::JoinHandle<()>) {
    let files: Arc<HashMap<String, Vec<u8>>> = Arc::new(files.into_iter().collect());
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

/// A fresh path under the system temp dir for a test's content storage.
pub fn temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "catalyst-{label}-{}",
        uuid::Uuid::new_v4().simple()
    ))
}
