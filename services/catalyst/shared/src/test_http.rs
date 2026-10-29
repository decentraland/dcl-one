//! The one `Router` request every domain's write-gate tests send: build a
//! method and a path, run it through [`tower::ServiceExt::oneshot`], read
//! the status and the body back as a string.

use axum::Router;
use axum::body::Body;
use axum::extract::Request;
use axum::http::StatusCode;
use tower::ServiceExt;

/// Sends `method path` to `app` and reads its status and body.
pub async fn call(app: &Router, method: &str, path: &str) -> (StatusCode, String) {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let body = axum::body::to_bytes(res.into_body(), 1 << 16)
        .await
        .unwrap();
    (status, String::from_utf8_lossy(&body).into_owned())
}
