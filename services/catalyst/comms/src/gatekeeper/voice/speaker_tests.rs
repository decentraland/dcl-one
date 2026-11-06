use super::*;
use crate::interfaces::test_support::{StubDirectory, StubPolicy, StubScenes};
use axum::{Router, http::StatusCode, routing::post};
use std::sync::{Arc, Mutex};

async fn exercise(
    speaker: bool,
    read_status: StatusCode,
    update_status: StatusCode,
) -> (Result<(), ApiError>, Vec<Value>) {
    let updates = Arc::new(Mutex::new(Vec::new()));
    let captured = updates.clone();
    let app = Router::new()
        .route(
            "/twirp/livekit.RoomService/GetParticipant",
            post(move || async move {
                (
                    read_status,
                    Json(serde_json::json!({
                        "identity": "wallet",
                        "metadata": r#"{"custom":"preserved","isSpeaker":true}"#
                    })),
                )
            }),
        )
        .route(
            "/twirp/livekit.RoomService/UpdateParticipant",
            post(move |Json(body): Json<Value>| {
                let captured = captured.clone();
                async move {
                    captured.lock().unwrap().push(body);
                    (update_status, Json(serde_json::json!({})))
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let host = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let pool = sqlx::PgPool::connect_lazy("postgres://postgres@127.0.0.1:1/postgres").unwrap();
    pool.close().await;
    let mut state = CommsState::new(
        pool,
        crate::http::Outbound::for_tests(),
        super::super::Settings::for_tests(),
        None,
        None,
        Arc::new(StubPolicy::default()),
        Arc::new(StubScenes::default()),
        Arc::new(crate::presence::Presence::new()),
        Arc::new(StubDirectory::default()),
    );
    state.livekit.api_url = host;
    state.livekit.api_key = "key".into();
    state.livekit.api_secret = "secret".into();
    let result = set_speaker(&state, CommunityId::new(), "wallet", speaker).await;
    server.abort();
    let bodies = updates.lock().unwrap().clone();
    (result, bodies)
}

fn assert_listener_fallback(body: &Value) {
    assert!(body.get("metadata").is_none());
    assert_eq!(body["identity"], "wallet");
    assert_eq!(
        body["permission"],
        serde_json::json!({
            "canPublish": false, "canSubscribe": true, "canPublishData": true
        })
    );
}

#[tokio::test]
async fn demotion_revokes_publishing_after_failed_read_and_reports_partial_failure() {
    let (result, updates) = exercise(false, StatusCode::SERVICE_UNAVAILABLE, StatusCode::OK).await;
    let error = result.unwrap_err();
    assert_eq!(error.code, 500);
    assert!(error.message.contains("permission revocation completed"));
    assert_eq!(updates.len(), 1);
    assert_listener_fallback(&updates[0]);
}

#[tokio::test]
async fn demotion_reports_failed_permission_revocation_after_failed_read() {
    let (result, updates) = exercise(
        false,
        StatusCode::SERVICE_UNAVAILABLE,
        StatusCode::BAD_GATEWAY,
    )
    .await;
    let error = result.unwrap_err();
    assert_eq!(error.code, 500);
    assert!(error.message.contains("permission revocation:"));
    assert!(!error.message.contains("completed"));
    assert_eq!(updates.len(), 1);
    assert_listener_fallback(&updates[0]);
}

#[tokio::test]
async fn promotion_does_not_grant_permissions_after_failed_read() {
    let (result, updates) = exercise(true, StatusCode::SERVICE_UNAVAILABLE, StatusCode::OK).await;
    assert_eq!(result.unwrap_err().code, 500);
    assert!(updates.is_empty());
}

#[tokio::test]
async fn demotion_preserves_metadata_and_listener_permissions_when_read_succeeds() {
    let (result, updates) = exercise(false, StatusCode::OK, StatusCode::OK).await;
    result.unwrap();
    assert_eq!(updates.len(), 1);
    let metadata: Value = serde_json::from_str(updates[0]["metadata"].as_str().unwrap()).unwrap();
    assert_eq!(
        metadata,
        serde_json::json!({
            "custom": "preserved", "isSpeaker": false, "isRequestingToSpeak": false
        })
    );
    assert_eq!(updates[0]["permission"]["canPublish"], false);
    assert_eq!(updates[0]["permission"]["canSubscribe"], true);
    assert_eq!(updates[0]["permission"]["canPublishData"], true);
}

#[tokio::test]
async fn demotion_does_not_claim_success_when_both_updates_fail() {
    let (result, updates) = exercise(false, StatusCode::OK, StatusCode::BAD_GATEWAY).await;
    assert!(
        result
            .unwrap_err()
            .message
            .contains("permission revocation:")
    );
    assert_eq!(updates.len(), 2);
    assert!(updates[0].get("metadata").is_some());
    assert_listener_fallback(&updates[1]);
}
