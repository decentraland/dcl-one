//! `/v1/thirdParties/{id}`: who reads a provider, its free slots, and who
//! removes or flags a virtual third party, against a loopback registry.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, header};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use super::support::{
    FakeCommittee, FakeRegistry, OWNER_KEY, registry_index, temp_storage, wallet,
};
use crate::shared::signed_fetch::test_support::signed_headers_by;

const STRANGER_KEY: &str = "0x2222222222222222222222222222222222222222222222222222222222222222";
const REGISTRY_KEY: &str = "0x3333333333333333333333333333333333333333333333333333333333333333";
const PROVIDER: &str = "urn:decentraland:matic:collections-thirdparty:tp-routes";

struct App {
    pool: PgPool,
    registry: FakeRegistry,
    router: Router,
}

fn address(key: &str) -> String {
    format!("{:#x}", wallet(key).address())
}

async fn app() -> Option<App> {
    let pool = crate::db::scratch::isolated_pool(8).await?;
    let registry = FakeRegistry::up();
    let state = Arc::new(
        crate::State::new(
            pool.clone(),
            temp_storage().await,
            Arc::default(),
            dcl_http::Outbound::for_tests(),
            Some(registry_index(&FakeCommittee::with(&[]), &registry).await),
            None,
            crate::Settings {
                writes_enabled: true,
                ..crate::Settings::default()
            },
        )
        .unwrap(),
    );
    Some(App {
        router: registry.published(&pool, crate::router(state)),
        pool,
        registry,
    })
}

async fn virtual_row(app: &App, managers: &[&str]) {
    let managers: Vec<String> = managers.iter().map(|k| address(k)).collect();
    sqlx::query(
        "INSERT INTO builder.virtual_third_parties (id, managers, raw_metadata) VALUES ($1, $2, 'tp:1:Routes:A test')",
    )
    .bind(PROVIDER)
    .bind(managers)
    .execute(&app.pool)
    .await
    .unwrap();
}

async fn row(app: &App) -> Option<bool> {
    sqlx::query_scalar("SELECT is_programmatic FROM builder.virtual_third_parties WHERE id = $1")
        .bind(PROVIDER)
        .fetch_optional(&app.pool)
        .await
        .unwrap()
}

async fn curated_items(app: &App, count: usize) {
    let collection = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO builder.collections (id, name, eth_address, third_party_id, urn_suffix) VALUES ($1, 'Routes', $2, $3, $4)",
    )
    .bind(collection)
    .bind(address(OWNER_KEY))
    .bind(PROVIDER)
    .bind(format!("{PROVIDER}:routes"))
    .execute(&app.pool)
    .await
    .unwrap();
    for _ in 0..count {
        let item = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO builder.items (id, collection_id, eth_address, name, type) VALUES ($1, $2, $3, 'Hat', 'wearable')",
        )
        .bind(item)
        .bind(collection)
        .bind(address(OWNER_KEY))
        .execute(&app.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO builder.item_curations (id, item_id, status, content_hash) VALUES ($1, $2, 'pending', $3)",
        )
        .bind(Uuid::new_v4())
        .bind(item)
        .bind("ab".repeat(32))
        .execute(&app.pool)
        .await
        .unwrap();
    }
}

async fn send(app: &App, key: &str, method: &str, path: &str, body: &str) -> (u16, Value) {
    let mut request = Request::builder().method(method).uri(path);
    let now = chrono::Utc::now().timestamp_millis().to_string();
    for (name, value) in
        signed_headers_by(&wallet(key), &method.to_lowercase(), path, "{}", true, &now)
            .await
            .iter()
    {
        request = request.header(name, value);
    }
    let request = request
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = app.router.clone().oneshot(request).await.unwrap();
    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn path(suffix: &str) -> String {
    format!("/v1/thirdParties/{PROVIDER}{suffix}")
}

#[tokio::test]
async fn any_signer_reads_a_registered_provider_and_a_null_root_reads_as_empty() {
    let Some(app) = app().await else { return };
    app.registry
        .put(PROVIDER, &[&address(REGISTRY_KEY)], true, false);
    app.registry.set(PROVIDER, "root", Value::Null);
    let (status, body) = send(&app, STRANGER_KEY, "GET", &path(""), "").await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["data"]["id"], PROVIDER);
    assert_eq!(body["data"]["published"], true);
    assert_eq!(body["data"]["maxItems"], "10");
    assert_eq!(body["data"]["root"], "");
}

#[tokio::test]
async fn a_virtual_third_party_reads_only_to_its_managers() {
    let Some(app) = app().await else { return };
    virtual_row(&app, &[OWNER_KEY]).await;
    let (status, body) = send(&app, OWNER_KEY, "GET", &path(""), "").await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["data"]["name"], "Routes");
    assert_eq!(body["data"]["description"], "A test");
    assert_eq!(body["data"]["published"], false);
    assert_eq!(body["data"]["maxItems"], "0");
    let (status, body) = send(&app, STRANGER_KEY, "GET", &path(""), "").await;
    assert_eq!(status, 404, "{body}");
    assert_eq!(body["error"], "The Third Party doesn't exists.");
}

#[tokio::test]
async fn a_providers_manager_reads_its_free_slots_and_anyone_else_is_refused() {
    let Some(app) = app().await else { return };
    app.registry
        .put(PROVIDER, &[&address(OWNER_KEY)], true, false);
    curated_items(&app, 3).await;
    let (status, body) = send(&app, OWNER_KEY, "GET", &path("/slots"), "").await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["data"], 7);
    let (status, body) = send(&app, STRANGER_KEY, "GET", &path("/slots"), "").await;
    assert_eq!(status, 401, "{body}");
}

#[tokio::test]
async fn a_virtual_providers_slots_go_negative_with_its_curated_items() {
    let Some(app) = app().await else { return };
    virtual_row(&app, &[OWNER_KEY]).await;
    curated_items(&app, 2).await;
    let (status, body) = send(&app, OWNER_KEY, "GET", &path("/slots"), "").await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["data"], -2);
}

#[tokio::test]
async fn deleting_a_missing_virtual_third_party_is_404_even_while_the_registry_is_down() {
    let Some(app) = app().await else { return };
    app.registry.fail(true);
    let (status, body) = send(&app, OWNER_KEY, "DELETE", &path(""), "").await;
    assert_eq!(status, 404, "{body}");
}

#[tokio::test]
async fn a_virtual_third_party_the_registry_lacks_is_kept_for_its_manager_and_refused_to_others() {
    let Some(app) = app().await else { return };
    virtual_row(&app, &[OWNER_KEY]).await;
    let (status, body) = send(&app, OWNER_KEY, "DELETE", &path(""), "").await;
    assert_eq!(status, 400, "{body}");
    assert_eq!(
        body["error"],
        "The Third Party can only be deleted if it's already on the graph."
    );
    let (status, body) = send(&app, STRANGER_KEY, "DELETE", &path(""), "").await;
    assert_eq!(status, 401, "{body}");
    assert_eq!(row(&app).await, Some(false));
}

#[tokio::test]
async fn a_registered_providers_manager_deletes_its_virtual_third_party_even_unlisted() {
    let Some(app) = app().await else { return };
    virtual_row(&app, &[STRANGER_KEY]).await;
    app.registry
        .put(PROVIDER, &[&address(REGISTRY_KEY)], true, false);
    let (status, body) = send(&app, STRANGER_KEY, "DELETE", &path(""), "").await;
    assert_eq!(status, 401, "{body}");
    assert_eq!(row(&app).await, Some(false));
    let (status, body) = send(&app, REGISTRY_KEY, "DELETE", &path(""), "").await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body, json!({ "ok": true }));
    assert_eq!(row(&app).await, None);
}

#[tokio::test]
async fn flagging_needs_a_boolean_in_a_json_body() {
    let Some(app) = app().await else { return };
    virtual_row(&app, &[OWNER_KEY]).await;
    for body in ["{not json", "{}", r#"{"isProgrammatic":"yes"}"#] {
        let (status, answer) = send(&app, OWNER_KEY, "PATCH", &path(""), body).await;
        assert_eq!(status, 400, "{body}: {answer}");
        assert_eq!(answer["error"], "Invalid request body", "{body}");
    }
}

#[tokio::test]
async fn flagging_a_missing_virtual_third_party_is_404_even_while_the_registry_is_down() {
    let Some(app) = app().await else { return };
    app.registry.fail(true);
    let (status, body) = send(
        &app,
        OWNER_KEY,
        "PATCH",
        &path(""),
        r#"{"isProgrammatic":true}"#,
    )
    .await;
    assert_eq!(status, 404, "{body}");
}

#[tokio::test]
async fn only_a_listed_wallet_that_manages_the_provider_flags_it() {
    let Some(app) = app().await else { return };
    virtual_row(&app, &[OWNER_KEY, STRANGER_KEY]).await;
    app.registry
        .put(PROVIDER, &[&address(OWNER_KEY)], true, false);
    let flag = r#"{"isProgrammatic":true}"#;
    let (status, body) = send(&app, STRANGER_KEY, "PATCH", &path(""), flag).await;
    assert_eq!(status, 401, "{body}");
    let (status, body) = send(&app, REGISTRY_KEY, "PATCH", &path(""), flag).await;
    assert_eq!(status, 401, "{body}");
    assert_eq!(row(&app).await, Some(false));
    let (status, body) = send(&app, OWNER_KEY, "PATCH", &path(""), flag).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(row(&app).await, Some(true));
}
