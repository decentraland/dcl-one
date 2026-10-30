//! Helpers the price tests and the application's cross-domain tests share
//! (`cross_tests::price_rate_tests`, the shop market tests).

use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Request, StatusCode, header};
use serde_json::Value;
use tower::ServiceExt;

use super::{Settings, SharedState, State};

pub fn state_over(pool: sqlx::PgPool, settings: Settings) -> SharedState {
    Arc::new(State::new(
        pool,
        crate::http::Outbound::for_tests(),
        settings,
    ))
}

pub async fn send(
    app: Router,
    method: &str,
    path: &str,
    headers: Vec<(&str, String)>,
    body: Option<Value>,
) -> (StatusCode, HeaderMap, Value) {
    let mut req = Request::builder().method(method).uri(path);
    for (k, v) in &headers {
        req = req.header(*k, v.as_str());
    }
    let req = match body {
        Some(v) => req
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(v.to_string()))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, headers, body)
}

/// The two tests over `economy.price_snapshots` on the shared scratch
/// database: the poller test inserts the marker rows the empty-table test
/// must not see between its check and its request (a race under the
/// parallel runner, 2026-09-25).
pub static SNAPSHOTS: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
