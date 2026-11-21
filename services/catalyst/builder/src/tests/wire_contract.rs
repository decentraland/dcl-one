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
const ADDRESS: &str = "0x1111111111111111111111111111111111111111";

const SIGNED: &[(&str, &str, &str)] = &[
    ("GET", "/v1/items", ""),
    ("GET", "/v1/collections/{id}", ""),
    (
        "PUT",
        "/v1/collections/{id}",
        r#"{"collection":{"id":"{id}","name":"Hats"}}"#,
    ),
    ("GET", "/v1/collections/{id}/items", ""),
    ("GET", "/v1/items/{id}", ""),
    (
        "PUT",
        "/v1/items/{id}",
        r#"{"item":{"id":"{id}","name":"Cap","type":"wearable"}}"#,
    ),
    ("DELETE", "/v1/items/{id}", ""),
    ("GET", "/v1/drafts/collections", ""),
    ("GET", "/v1/drafts/items", ""),
    ("GET", "/v1/{address}/collections", ""),
    ("GET", "/v1/{address}/items", ""),
    ("GET", "/v1/collections/curation", ""),
    (
        "PATCH",
        "/v1/collections/{id}/items/{id}/status",
        r#"{"status":"approved"}"#,
    ),
    (
        "PATCH",
        "/v1/collections/{id}/items/status",
        r#"{"status":"approved","itemIds":[]}"#,
    ),
    ("GET", "/v1/collections/{id}/publication", ""),
    (
        "POST",
        "/v1/collections/{id}/publication",
        r#"{"revision":"r"}"#,
    ),
    ("GET", "/v1/collections/{id}/publication/status", ""),
    ("GET", "/v1/collections/{id}/approvalData", ""),
];

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
async fn every_signed_route_answers_an_unsigned_request_privately_with_upstreams_401() {
    let app = app(state_with_settings(crate::Settings {
        writes_enabled: true,
        admin_token: Some("example-admin-token".into()),
        ..crate::Settings::default()
    })
    .await);
    let unauthenticated = json!({ "ok": false, "data": { "message": "Invalid auth chain" }, "error": "Unauthenticated" });
    let mut wrong = Vec::new();
    for mount in ["", "/builder-api"] {
        for (method, path, body) in SIGNED {
            let path = format!(
                "{mount}{}",
                path.replace("{id}", ID).replace("{address}", ADDRESS)
            );
            let (status, cache, body) = send(&app, method, &path, &body.replace("{id}", ID)).await;
            let expected = &unauthenticated;
            if status != StatusCode::UNAUTHORIZED
                || cache.as_deref() != Some("private, no-store")
                || &body != expected
            {
                wrong.push(format!("{method} {path}: {status} {cache:?} {body}"));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[tokio::test]
async fn public_file_reads_keep_their_own_cache_rule() {
    let app = app(state_with_settings(crate::Settings::default()).await);
    let (status, cache, body) =
        send(&app, "GET", "/v1/storage/contents/not-a-hash/exists", "").await;
    assert_eq!(
        (status, cache, body),
        (
            StatusCode::NOT_FOUND,
            Some("no-store".to_string()),
            json!({ "ok": true })
        )
    );
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
async fn an_unsigned_request_is_refused_before_its_id_or_body_is_read() {
    let app = app(state_with_settings(crate::Settings {
        writes_enabled: true,
        admin_token: Some("example-admin-token".into()),
        ..crate::Settings::default()
    })
    .await);
    let unauthenticated = json!({ "ok": false, "data": { "message": "Invalid auth chain" }, "error": "Unauthenticated" });
    let mut wrong = Vec::new();
    for (method, path, body) in [
        ("PUT", "/v1/collections/{id}", "{}"),
        ("PUT", "/v1/collections/not-a-uuid", "{}"),
        ("PUT", "/v1/items/{id}", "{not json"),
        ("GET", "/v1/items/not-a-uuid", ""),
        ("DELETE", "/v1/items/not-a-uuid", ""),
        ("POST", "/v1/items/{id}/files", "not multipart"),
        ("POST", "/v1/collections/{id}/publication", "{}"),
        ("PUT", "/v1/collections/{id}/publication", "{}"),
        ("PATCH", "/v1/collections/{id}/publication", "{}"),
        ("POST", "/v1/collections/{id}/publish", "{not json"),
        ("PATCH", "/v1/collections/{id}/items/{id}/status", "{}"),
        ("PATCH", "/v1/collections/{id}/items/status", "{}"),
        ("GET", "/v1/collections/{id}/items?page=two", ""),
    ] {
        let path = path.replace("{id}", ID);
        let (status, _, answer) = send(&app, method, &path, body).await;
        if status != StatusCode::UNAUTHORIZED || answer != unauthenticated {
            wrong.push(format!("{method} {path}: {status} {answer}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
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
