//! `CREATE_WRITES_ENABLED` off on the router `serve` mounts: every write
//! route answers the gate's 503 at the root and under `/builder-api`, and
//! reads keep answering.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use tower::ServiceExt;

use super::support::state_without_db;
use crate::writes::DISABLED_MESSAGE;

const ID: &str = "6f1c0a52-5d0e-4c4b-9c55-0d6a1a3b7e21";

const WRITES: &[(&str, &str)] = &[
    ("PUT", "/v1/collections/{id}"),
    ("PUT", "/v1/items/{id}"),
    ("DELETE", "/v1/items/{id}"),
    ("POST", "/v1/items/{id}/files"),
    ("POST", "/v1/newsletter"),
    ("POST", "/v1/collections/{id}/publication"),
    ("PUT", "/v1/collections/{id}/publication"),
    ("PATCH", "/v1/collections/{id}/publication"),
    ("DELETE", "/v1/collections/{id}/publication"),
    ("POST", "/v1/items/{id}/videos"),
];

async fn send(app: &Router, method: &str, path: &str) -> (StatusCode, Option<String>, String) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let retry_after = response
        .headers()
        .get(header::RETRY_AFTER)
        .map(|v| v.to_str().unwrap().to_string());
    let body = axum::body::to_bytes(response.into_body(), 1 << 16)
        .await
        .unwrap();
    (
        status,
        retry_after,
        String::from_utf8_lossy(&body).into_owned(),
    )
}

#[tokio::test]
async fn with_writes_off_every_write_is_refused_on_both_mounts_and_reads_answer() {
    let state = state_without_db(None).await;
    assert!(!state.settings.writes_enabled);
    let app = Router::new()
        .merge(crate::root_router(state.clone()))
        .nest("/builder-api", crate::router(state));
    let mut answered = Vec::new();
    for mount in ["", "/builder-api"] {
        for (method, path) in WRITES {
            let path = format!("{mount}{}", path.replace("{id}", ID));
            let (status, retry_after, body) = send(&app, method, &path).await;
            let refused = status == StatusCode::SERVICE_UNAVAILABLE
                && retry_after.is_some()
                && serde_json::from_str::<serde_json::Value>(&body).is_ok_and(|v| {
                    v == serde_json::json!({ "ok": false, "data": {}, "error": DISABLED_MESSAGE })
                });
            if !refused {
                answered.push(format!("{method} {path}: {status} {body}"));
            }
        }
    }
    assert!(answered.is_empty(), "{}", answered.join("\n"));

    let (status, _, body) = send(&app, "GET", "/builder-api/ping").await;
    assert_eq!((status, body.as_str()), (StatusCode::OK, "pong"));
    let (status, _, _) = send(&app, "GET", "/v1/storage/contents/not-a-hash").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "a read is not gated");
}

/// The gate answers before routing, so the refusals above would also pass
/// for a route that is gone: with writes on, each write reaches a handler on
/// both mounts, neither 404 nor 405.
#[tokio::test]
async fn with_writes_on_every_gated_write_is_routed_on_both_mounts() {
    let state = super::support::state_with_settings(crate::Settings {
        writes_enabled: true,
        ..Default::default()
    })
    .await;
    let app = Router::new()
        .merge(crate::root_router(state.clone()))
        .nest("/builder-api", crate::router(state));
    let mut unrouted = Vec::new();
    for mount in ["", "/builder-api"] {
        for (method, path) in WRITES {
            let path = format!("{mount}{}", path.replace("{id}", ID));
            let (status, _, body) = send(&app, method, &path).await;
            if matches!(
                status,
                StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
            ) {
                unrouted.push(format!("{method} {path}: {status} {body}"));
            }
        }
    }
    assert!(unrouted.is_empty(), "{}", unrouted.join("\n"));
}
