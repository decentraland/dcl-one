//! The wait for a stack that is back after a restart: the realm description answers 503 while
//! parts of the stack are still coming up.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use axum::Router;
use axum::http::StatusCode;
use axum::routing::get;
use catalyst_comms_e2e::engine::Ctx;
use catalyst_comms_e2e::scenarios::connections_support::expect_realm_description;
use catalyst_comms_e2e::stack::Stack;

/// A server that answers `unavailable` times with 503 and then with a description that names an
/// adapter (or none, when `adapter` is empty).
async fn about(unavailable: u32, adapter: &'static str) -> (url::Url, Arc<AtomicU32>) {
    let asked = Arc::new(AtomicU32::new(0));
    let counter = asked.clone();
    let router = Router::new().route(
        "/about",
        get(move || {
            let counter = counter.clone();
            async move {
                if counter.fetch_add(1, Ordering::SeqCst) < unavailable {
                    return (StatusCode::SERVICE_UNAVAILABLE, "{}".to_owned());
                }
                (
                    StatusCode::OK,
                    serde_json::json!({ "comms": { "adapter": adapter } }).to_string(),
                )
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (format!("http://{address}/").parse().unwrap(), asked)
}

fn ctx() -> Ctx {
    Ctx::new(
        Arc::new(Stack::pulse_only("127.0.0.1:9".parse().unwrap())),
        &format!("t{}", std::process::id()),
        "realm-description",
        0,
        1,
        std::env::temp_dir().join("comms-e2e-tests"),
    )
}

#[tokio::test]
async fn the_wait_outlasts_a_description_that_answers_503_while_the_stack_comes_back() {
    let (api, asked) = about(3, "archipelago:archipelago:ws://example.invalid/ws").await;
    let description = expect_realm_description(&ctx(), &api, "it is back", Duration::from_secs(20))
        .await
        .unwrap();
    assert_eq!(
        description.adapter,
        "archipelago:archipelago:ws://example.invalid/ws"
    );
    assert!(asked.load(Ordering::SeqCst) >= 4);
}

#[tokio::test]
async fn a_description_that_stays_503_fails_with_the_status_it_answered() {
    let (api, _) = about(u32::MAX, "").await;
    let error = expect_realm_description(&ctx(), &api, "it is back", Duration::from_millis(600))
        .await
        .unwrap_err();
    let text = error.to_string();
    assert!(text.contains("expected: it is back"), "{text}");
    assert!(text.contains("HTTP 503"), "{text}");
}

#[tokio::test]
async fn a_200_that_names_no_adapter_is_not_back() {
    let (api, _) = about(0, "").await;
    let error = expect_realm_description(&ctx(), &api, "it is back", Duration::from_millis(600))
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("names no comms adapter"),
        "{error}"
    );
}
