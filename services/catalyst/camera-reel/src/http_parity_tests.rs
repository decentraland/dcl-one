use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use tower::ServiceExt;

use crate::http::{JsonBody, Query};
use crate::{Settings, State, router};

async fn app() -> (Router, std::path::PathBuf) {
    app_with_token(None).await
}

async fn app_with_token(token: Option<&str>) -> (Router, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("camera-http-{}", uuid::Uuid::new_v4()));
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://localhost/camera_http_unused")
        .unwrap();
    let storage = Arc::new(crate::storage::Storage::from_config(&dir).await.unwrap());
    let mut settings = Settings::new("https://camera.example");
    settings.writes_enabled = true;
    settings.metrics_bearer_token = token.map(str::to_owned);
    let state = Arc::new(State::new(pool, storage, settings, None));
    (
        Router::new().nest("/camera-reel-service", router(state)),
        dir,
    )
}

#[tokio::test]
async fn health_and_api_preflights_keep_their_individual_policy() {
    let (app, dir) = app().await;
    for (path, methods, age) in [
        ("/api/images", "DELETE, GET, POST, PATCH", "300"),
        ("/api/unknown", "DELETE, GET, POST, PATCH", "300"),
        ("/health/live", "GET, OPTIONS", "3600"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("OPTIONS")
                    .uri(format!("/camera-reel-service{path}"))
                    .header(header::ORIGIN, "https://play.example")
                    .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                    .header(
                        header::ACCESS_CONTROL_REQUEST_HEADERS,
                        "x-identity-auth-chain-7,content-type",
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        assert_eq!(
            response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
            "https://play.example"
        );
        assert_eq!(
            response.headers()[header::ACCESS_CONTROL_ALLOW_METHODS],
            methods
        );
        assert_eq!(response.headers()[header::ACCESS_CONTROL_MAX_AGE], age);
        assert_eq!(
            response.headers()[header::ACCESS_CONTROL_ALLOW_HEADERS],
            "x-identity-auth-chain-7,content-type"
        );
        assert!(
            !response
                .headers()
                .contains_key(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
        );
        assert!(
            to_bytes(response.into_body(), 4096)
                .await
                .unwrap()
                .is_empty()
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn cors_validates_preflight_method_and_preserves_plain_options() {
    let (app, dir) = app().await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/camera-reel-service/api/images")
                .header(header::ORIGIN, "https://play.example")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "PUT")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(
        !response
            .headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
    );
    assert_eq!(
        to_bytes(response.into_body(), 4096).await.unwrap(),
        "Requested method is not allowed"
    );
    let response = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/camera-reel-service/api/images")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn health_and_missing_paths_preserve_status_and_body() {
    let (app, dir) = app().await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/camera-reel-service/health/live")
                .header(header::ORIGIN, "https://play.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
    assert_eq!(
        response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        "https://play.example"
    );
    assert!(
        !response
            .headers()
            .contains_key(header::ACCESS_CONTROL_EXPOSE_HEADERS)
    );
    assert_eq!(
        to_bytes(response.into_body(), 4096).await.unwrap(),
        "\"alive\""
    );
    for (method, path) in [
        ("GET", "/api/unknown"),
        ("PUT", "/api/images"),
        ("HEAD", "/health/live"),
        ("HEAD", "/api/users/example"),
        ("HEAD", "/api/users/example/images"),
        ("HEAD", "/api/images/example"),
        ("HEAD", "/api/images/example/metadata"),
        ("HEAD", "/api/places/example/images"),
        ("HEAD", "/api/wearables/example/images"),
        ("HEAD", "/api/docs/openapi.json"),
        ("HEAD", "/api/docs/ui/"),
        ("HEAD", "/api/docs/ui/swagger-ui.css"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(format!("/camera-reel-service{path}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{method} {path}");
        assert!(
            to_bytes(response.into_body(), 4096)
                .await
                .unwrap()
                .is_empty()
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[derive(Deserialize)]
struct Page {
    limit: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Places {
    places_ids: Vec<String>,
}

fn extraction_app() -> Router {
    Router::new()
        .route(
            "/query",
            get(|Query(page): Query<Page>| async move { Json(page.limit) }),
        )
        .route(
            "/json",
            post(|JsonBody(places): JsonBody<Places>| async move { Json(places.places_ids) }),
        )
}

#[tokio::test]
async fn query_rejections_match_the_form_parser_messages() {
    for (query, expected) in [
        (
            "limit=nope",
            "Query deserialize error: invalid digit found in string",
        ),
        (
            "limit=-1",
            "Query deserialize error: invalid digit found in string",
        ),
        (
            "limit=",
            "Query deserialize error: cannot parse integer from empty string",
        ),
        (
            "limit=1&limit=2",
            "Query deserialize error: unsupported value",
        ),
    ] {
        let response = extraction_app()
            .oneshot(
                Request::builder()
                    .uri(format!("/query?{query}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/plain; charset=utf-8"
        );
        assert_eq!(
            to_bytes(response.into_body(), 4096).await.unwrap(),
            expected,
            "{query}"
        );
    }
}

#[tokio::test]
async fn json_rejections_keep_plain_text_and_the_original_size_limit() {
    for (content_type, body, expected) in [
        ("text/plain", "{}", "Content type error"),
        (
            "application/json",
            "{}",
            "Json deserialize error: missing field `placesIds` at line 1 column 2",
        ),
    ] {
        let response = extraction_app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/json")
                    .header(header::CONTENT_TYPE, content_type)
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            to_bytes(response.into_body(), 4096).await.unwrap(),
            expected
        );
    }
    let response = extraction_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/json")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::CONTENT_LENGTH, "2097153")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(
        to_bytes(response.into_body(), 4096).await.unwrap(),
        "JSON payload (2097153 bytes) is larger than allowed (limit: 2097152 bytes)."
    );
}

#[tokio::test]
async fn openapi_describes_visible_person_screen_rect() {
    let response = crate::docs::openapi_json().await;
    let spec: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 100_000).await.unwrap()).unwrap();
    assert_eq!(
        spec["components"]["schemas"]["ScreenRect"]["required"],
        json!(["x", "y", "width", "height"])
    );
    assert_eq!(
        spec["components"]["schemas"]["User"]["properties"]["screenRect"]["allOf"][0]["$ref"],
        "#/components/schemas/ScreenRect"
    );
    assert_eq!(spec["paths"].as_object().unwrap().len(), 9);
}

#[tokio::test]
async fn metrics_enforces_bearer_before_recording_and_preserves_service_series() {
    let (unconfigured, dir) = app().await;
    let response = unconfigured
        .oneshot(
            Request::builder()
                .uri("/camera-reel-service/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert!(
        to_bytes(response.into_body(), 4096)
            .await
            .unwrap()
            .is_empty()
    );
    std::fs::remove_dir_all(dir).unwrap();

    let (app, dir) = app_with_token(Some("camera-token")).await;
    for (token, message) in [
        (None, "Token not present"),
        (Some("Basic wrong"), "Wrong schema"),
        (Some("Bearer wrong"), "Invalid token"),
    ] {
        let mut request = Request::builder().uri("/camera-reel-service/metrics");
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, token);
        }
        let response = app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(to_bytes(response.into_body(), 4096).await.unwrap(), message);
    }
    let scrape = || {
        Request::builder()
            .uri("/camera-reel-service/metrics")
            .header(header::AUTHORIZATION, "Bearer camera-token")
            .body(Body::empty())
            .unwrap()
    };
    let response = app.clone().oneshot(scrape()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/plain; charset=utf-8"
    );
    assert!(
        to_bytes(response.into_body(), 4096)
            .await
            .unwrap()
            .is_empty(),
        "denied scrapes are not observations"
    );
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/camera-reel-service/health/live")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/camera-reel-service/api/missing")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = app.clone().oneshot(scrape()).await.unwrap();
    let body = String::from_utf8(
        to_bytes(response.into_body(), 100_000)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        body.contains(
            "http_requests_total{code=\"200\",handler=\"/health/live\",method=\"GET\"} 1"
        ),
        "{body}"
    );
    assert!(
        body.contains("http_requests_total{code=\"404\",handler=\"*\",method=\"GET\"} 1"),
        "{body}"
    );
    assert!(
        body.contains("http_requests_total{code=\"200\",handler=\"*\",method=\"GET\"} 1"),
        "previous successful scrape counted"
    );
    assert!(!body.contains("401"));
    for bucket in [
        "0.005", "0.01", "0.025", "0.05", "0.1", "0.25", "0.5", "1", "2.5", "5", "10", "20", "30",
        "60", "+Inf",
    ] {
        assert!(
            body.contains(&format!("le=\"{bucket}\"")),
            "missing {bucket}"
        );
    }
    for method in ["POST", "OPTIONS", "HEAD"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri("/camera-reel-service/metrics")
                    .header(header::AUTHORIZATION, "Bearer camera-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{method}");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn swagger_serves_local_assets_and_uses_the_mounted_document_url() {
    let (app, dir) = app().await;
    for (file, mime, needle) in [
        ("", "text/html", "swagger-ui-bundle.js"),
        ("swagger-ui.css", "text/css", ".swagger-ui"),
        (
            "swagger-ui-bundle.js",
            "application/javascript",
            "SwaggerUIBundle",
        ),
        (
            "swagger-initializer.js",
            "application/javascript",
            "/camera-reel-service/api/docs/openapi.json",
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/camera-reel-service/api/docs/ui/{file}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{file}");
        assert_eq!(response.headers()[header::CONTENT_TYPE], mime);
        let bytes = to_bytes(response.into_body(), 3_000_000).await.unwrap();
        assert!(
            std::str::from_utf8(&bytes).unwrap().contains(needle),
            "{file}"
        );
    }
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/camera-reel-service/api/docs/ui/not-an-asset.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/camera-reel-service/api/docs/ui")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn openapi_keeps_all_declared_paths_and_existing_schema_contracts() {
    let baseline: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/conformance/social/camera-reel/docs-openapi.raw.body"
    )))
    .unwrap();
    let response = crate::docs::openapi_json().await;
    let spec: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 100_000).await.unwrap()).unwrap();
    assert_eq!(spec["paths"], baseline["paths"]);
    for (name, schema) in baseline["components"]["schemas"].as_object().unwrap() {
        if name == "User" {
            let mut user = spec["components"]["schemas"][name].clone();
            user["properties"]
                .as_object_mut()
                .unwrap()
                .remove("screenRect");
            assert_eq!(&user, schema);
        } else {
            assert_eq!(&spec["components"]["schemas"][name], schema, "{name}");
        }
    }
}
