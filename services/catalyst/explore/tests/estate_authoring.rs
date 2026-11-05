use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use catalyst_explore::{ExploreState, Settings, events::moderation, router};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn request(app: &axum::Router, method: &str, path: &str, body: Value) -> Value {
    let metadata = json!({"body_sha256": hex::encode(sha2::Sha256::digest(body.to_string().as_bytes())), "nonce": uuid::Uuid::new_v4().to_string()}).to_string();
    let headers = catalyst_shared::signed_fetch::test_support::signed_headers(
        &method.to_lowercase(),
        path,
        &metadata,
        true,
    )
    .await;
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    for (name, value) in &headers {
        request = request.header(name, value);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body: Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 1 << 20)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

use sha2::Digest;

#[tokio::test]
async fn public_create_patch_and_admin_create_use_native_estate_membership() {
    // A separate test binary has its own scratch database and native publication.
    let Some(pool) = catalyst_db::scratch::pool(4).await else {
        return;
    };
    let land_settings = catalyst_land_authz::Settings::for_tests();
    sqlx::query("INSERT INTO land_authz.index_cursor(network,land_registry,estate_registry,last_block,folded_block,folded_at,full_replay_started) VALUES($1,$2,$3,1,1,timezone('UTC',now()),true)")
        .bind(&land_settings.network).bind(&land_settings.land_registry).bind(&land_settings.estate_registry).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO land_authz.token_right(token_address,token_id,x,y,updated_block,updated_log) VALUES($1,100,17,18,1,0),($1,101,19,20,1,0)")
        .bind(&land_settings.land_registry).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO land_authz.estate_land(land_token_id,estate_token_id,updated_block,updated_log) VALUES(100,500,1,0),(101,600,1,0)")
        .execute(&pool).await.unwrap();
    let settings = Settings {
        user_writes_enabled: true,
        ..Settings::for_tests()
    };
    let state = Arc::new(ExploreState::new(pool.clone(), settings, None));
    state
        .places
        .set_land_authz(Arc::new(catalyst_land_authz::LandAuthzState::new(
            pool.clone(),
            dcl_http::Outbound::for_tests(),
            land_settings,
            None,
        )));
    let app = router(state.clone());
    let body = json!({"name":"Native estate", "start_at":"2099-06-01T18:00:00Z", "duration":3600000, "x":17, "y":18});
    let created = request(&app, "POST", "/api/events", body.clone()).await;
    assert_eq!(created["data"]["estate_id"], "500");
    let path = format!("/api/events/{}", created["data"]["id"].as_str().unwrap());
    let edited = request(&app, "PATCH", &path, json!({"x":19,"y":20})).await;
    assert_eq!(edited["data"]["estate_id"], "600");
    let admin_id = moderation::create(
        &pool,
        &serde_json::from_value(body).unwrap(),
        "operator",
        &state.settings,
        Some(&state.places),
    )
    .await
    .unwrap();
    let estate: String =
        sqlx::query_scalar("SELECT raw->>'estate_id' FROM explore.event WHERE id=$1")
            .bind(admin_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(estate, "500");
}
