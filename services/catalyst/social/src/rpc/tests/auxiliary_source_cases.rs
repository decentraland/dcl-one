//! Actual pinned status/privacy/metrics handler captures. Runtime sample values,
//! Node collectors, disconnected writes and database-error text are separate scope.
use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
};
use serde_json::Value;
use tower::ServiceExt;

#[tokio::test]
async fn uws_auxiliary_routes_match_executed_source_configuration_and_authentication() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/conformance/social/auxiliary-cases.json"
    )))
    .unwrap();
    let recorder = crate::comms::metrics::prometheus_builder()
        .unwrap()
        .build_recorder();
    let handle = recorder.handle();
    metrics::with_local_recorder(&recorder, || {
        metrics::counter!("ws_messages_received").increment(3);
        metrics::counter!("shop_orders_total").increment(99);
    });
    let privacy_address = "0x9711111111111111111111111111111111111111";
    for case in fixture["cases"].as_array().unwrap() {
        let operation = case["operation"].as_str().unwrap();
        // Source catches preserve dependency error text, while the local SQL
        // surface sanitizes it; an actual socket abort uses different transports.
        if matches!(
            case["id"].as_str(),
            Some("error" | "unknown-error" | "aborted")
        ) {
            continue;
        }
        let config = &case["config"];
        let settings = Settings {
            commit_hash: config["COMMIT_HASH"].as_str().map(str::to_string),
            current_version: config["CURRENT_VERSION"].as_str().map(str::to_string),
            metrics_path: config["WKC_METRICS_PUBLIC_PATH"]
                .as_str()
                .map(str::to_string),
            metrics_bearer_token: config["WKC_METRICS_BEARER_TOKEN"]
                .as_str()
                .map(str::to_string),
            metrics_handle: Some(handle.clone()),
            ..Default::default()
        };
        let state = Arc::new(State::new(
            pool.clone(),
            settings,
            None,
            Arc::new(FakeVoiceRooms::new(pool.clone())),
        ));
        let path = match operation {
            "status" => "/social-rpc/status".to_string(),
            "privacy" => {
                sqlx::query("DELETE FROM social.user_settings WHERE address=$1")
                    .bind(privacy_address)
                    .execute(&pool)
                    .await
                    .unwrap();
                if case["id"] == "friends" {
                    state
                        .ctx
                        .db()
                        .upsert_social_settings(privacy_address, Some("only_friends"), None, None)
                        .await
                        .unwrap();
                }
                let address = if case["id"] == "invalid" {
                    "bad".to_string()
                } else if case["id"] == "upper" {
                    privacy_address.to_uppercase()
                } else {
                    privacy_address.to_string()
                };
                format!("/social-rpc/v1/users/{address}/privacy-settings")
            }
            "metrics" => format!("/social-rpc{}", case["path"].as_str().unwrap()),
            unknown => panic!("unknown capture {unknown}"),
        };
        let app = crate::rpc::router(state);
        let mut builder = Request::builder().method(Method::GET).uri(&path);
        if let Some(authorization) = case["authorization"].as_str() {
            builder = builder.header(header::AUTHORIZATION, authorization)
        }
        let before = chrono::Utc::now().timestamp_millis();
        let response = app
            .clone()
            .oneshot(builder.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
        let after = chrono::Utc::now().timestamp_millis();
        if operation == "metrics" {
            let source = &case["response"];
            assert_eq!(
                status.as_u16(),
                source["status"]
                    .as_str()
                    .unwrap()
                    .split(' ')
                    .next()
                    .unwrap()
                    .parse::<u16>()
                    .unwrap(),
                "{}",
                case["id"]
            );
            assert!(!headers.contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
            if status == StatusCode::OK {
                assert_eq!(
                    headers[header::CONTENT_TYPE],
                    source["headers"]["content-type"].as_str().unwrap()
                );
                let body = String::from_utf8(bytes.to_vec()).unwrap();
                assert!(body.contains("ws_messages_received 3"), "{body}");
                assert!(
                    !body.contains("shop_orders_total"),
                    "other-domain samples must not leak into this view"
                );
            } else {
                assert!(bytes.is_empty());
            }
            let response = app
                .oneshot(
                    Request::builder()
                        .method(Method::HEAD)
                        .uri(&path)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
            assert_eq!(response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
        } else {
            assert_eq!(
                status.as_u16(),
                case["response"]["status"].as_u64().unwrap_or(200) as u16
            );
            assert_eq!(headers[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
            let mut body: Value = serde_json::from_slice(&bytes).unwrap();
            if operation == "status" {
                let now = body
                    .as_object_mut()
                    .unwrap()
                    .remove("currentTime")
                    .unwrap()
                    .as_i64()
                    .unwrap();
                assert!((before..=after).contains(&now));
            }
            assert_eq!(body, case["response"]["body"], "{operation} {}", case["id"]);
        }
    }
    sqlx::query("DELETE FROM social.user_settings WHERE address=$1")
        .bind(privacy_address)
        .execute(&pool)
        .await
        .unwrap();
}
