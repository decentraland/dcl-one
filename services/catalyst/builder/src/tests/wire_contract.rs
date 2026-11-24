//! The wire every builder route answers on both mounts `serve` composes:
//! a route that reads a signature answers privately, refused or not, and an
//! unsigned request gets upstream's `Unauthenticated` with nothing the node
//! knows in it.

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use serde_json::{Value, json};
use tower::ServiceExt;

use super::support::state_with_settings;
use crate::shared::signed_fetch::test_support::signed_headers;

const ID: &str = "6f1c0a52-5d0e-4c4b-9c55-0d6a1a3b7e21";
fn app(state: crate::SharedState) -> Router {
    Router::new()
        .merge(crate::root_router(state.clone()))
        .nest("/builder-api", crate::router(state))
}

async fn send(
    app: &Router,
    method: &str,
    path: &str,
    body: &str,
) -> (StatusCode, Option<String>, Value) {
    send_with(app, method, path, body, HeaderMap::new()).await
}

async fn send_signed(
    app: &Router,
    method: &str,
    path: &str,
    body: &str,
) -> (StatusCode, Option<String>, Value) {
    let signed_path = path.split('?').next().unwrap();
    let auth = signed_headers(&method.to_lowercase(), signed_path, "{}", true).await;
    send_with(app, method, path, body, auth).await
}

async fn send_with(
    app: &Router,
    method: &str,
    path: &str,
    body: &str,
    auth: HeaderMap,
) -> (StatusCode, Option<String>, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    for (name, value) in &auth {
        request = request.header(name, value);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let cache = response
        .headers()
        .get(header::CACHE_CONTROL)
        .map(|v| v.to_str().unwrap().to_string());
    let body = axum::body::to_bytes(response.into_body(), 1 << 16)
        .await
        .unwrap();
    (
        status,
        cache,
        serde_json::from_slice(&body).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn a_request_the_router_refuses_answers_the_envelope_without_the_parsers_words() {
    let app = app(state_with_settings(crate::Settings {
        writes_enabled: true,
        ..crate::Settings::default()
    })
    .await);
    let refusal = |error: &str| json!({ "ok": false, "data": {}, "error": error });
    for (method, path, body, status, error) in [
        (
            "PUT",
            format!("/v1/collections/{ID}"),
            "{}",
            StatusCode::BAD_REQUEST,
            "Invalid request body",
        ),
        (
            "PUT",
            format!("/v1/items/{ID}"),
            "{not json",
            StatusCode::BAD_REQUEST,
            "Invalid request body",
        ),
        (
            "PATCH",
            "/v1/drafts/items".to_string(),
            "",
            StatusCode::METHOD_NOT_ALLOWED,
            "Method Not Allowed",
        ),
    ] {
        let (got, _, body) = send_signed(&app, method, &path, body).await;
        assert_eq!((got, body), (status, refusal(error)), "{method} {path}");
    }
}

#[tokio::test]
async fn a_path_no_builder_route_serves_is_left_to_the_router_it_is_merged_into() {
    let app = app(state_with_settings(crate::Settings::default()).await);
    let response = app
        .oneshot(Request::get("/v1/projects").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(response.headers().get(header::CONTENT_TYPE), None);
    assert_eq!(response.headers().get(header::CACHE_CONTROL), None);
}

#[tokio::test]
async fn the_collection_listing_answers_privately_only_when_signed() {
    let app = app(state_with_settings(crate::Settings::default()).await);
    for mount in ["", "/builder-api"] {
        let (status, cache, _) = send(&app, "GET", &format!("{mount}/v1/collections"), "").await;
        assert_eq!(
            (status, cache),
            (StatusCode::UNAUTHORIZED, None),
            "{mount} unsigned listing"
        );
        let path = format!("{mount}/v1/collections");
        let (status, cache, _) = send_signed(&app, "GET", &path, "").await;
        assert_eq!(
            (status, cache.as_deref()),
            (StatusCode::SERVICE_UNAVAILABLE, Some("private, no-store")),
            "{mount} signed listing, with no committee source to decide it"
        );
    }
}
