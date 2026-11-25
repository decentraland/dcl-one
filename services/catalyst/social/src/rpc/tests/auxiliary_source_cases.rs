//! Actual pinned status/privacy handler captures. Runtime sample values,
//! Node collectors, disconnected writes and database-error text are separate scope.
use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, header},
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
    let privacy_address = "0x9711111111111111111111111111111111111111";
    for case in fixture["cases"].as_array().unwrap() {
        let operation = case["operation"].as_str().unwrap();
        if operation == "metrics" {
            continue;
        }
        // The real SQL error boundary has its own source-executed replay in
        // communities::tests::referral_errors. A socket abort uses different transports.
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
            unknown => panic!("unknown capture {unknown}"),
        };
        let app = crate::rpc::router(state);
        let builder = Request::builder().method(Method::GET).uri(&path);
        let before = chrono::Utc::now().timestamp_millis();
        let response = app
            .oneshot(builder.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
        let after = chrono::Utc::now().timestamp_millis();
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
    sqlx::query("DELETE FROM social.user_settings WHERE address=$1")
        .bind(privacy_address)
        .execute(&pool)
        .await
        .unwrap();
}
