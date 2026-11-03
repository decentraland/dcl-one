use super::*;
use axum::{Router, body::Body, extract::Request, response::IntoResponse, routing::post};
use futures::Stream;
use std::{
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
    task::{Context, Poll},
};
use tokio::sync::mpsc;

fn source() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/conformance/social/analytics-source-cases.json"
    )))
    .unwrap()
}
fn settings(url: String) -> Settings {
    Settings {
        context: Some("social-fixture".into()),
        api_url: Some(url),
        api_token: Some("fixture-token".into()),
        environment: Some("fixture".into()),
        request_timeout_ms: None,
    }
}

#[test]
fn source_required_strings_and_node_timeout_buckets_are_preserved() {
    let source = source();
    assert_eq!(
        source["baseline"]["revision"],
        "b6b2cefefdf727b2ed47ddb887e77d226a41eea6"
    );
    let http = crate::http::Outbound::for_tests();
    assert!(
        Analytics::new(&http, Settings::default())
            .unwrap()
            .is_none(),
        "monolith optional mode differs from required standalone source configuration"
    );
    for case in source["configuration"].as_array().unwrap() {
        let mut settings = settings("https://analytics.example/events".into());
        match case["omitted"].as_str() {
            Some("ANALYTICS_CONTEXT") => settings.context = None,
            Some("ANALYTICS_API_URL") => settings.api_url = None,
            Some("ANALYTICS_API_TOKEN") => settings.api_token = None,
            Some("ENV") => settings.environment = None,
            Some("all-empty") => {
                settings.context = Some(String::new());
                settings.api_url = Some(String::new());
                settings.api_token = Some(String::new());
                settings.environment = Some(String::new());
            }
            None => {}
            other => panic!("unhandled source case {other:?}"),
        }
        match Analytics::new(&http, settings) {
            Ok(value) => assert!(value.is_some() && case["configured"] == true),
            Err(error) => assert_eq!(error, case["error"].as_str().unwrap()),
        }
    }
    for case in source["timeouts"].as_array().unwrap() {
        if !case["error"].is_null() {
            continue;
        }
        let configured = match &case["configured_number"] {
            Value::Null => None,
            Value::String(value) if value == "Infinity" => Some(f64::INFINITY),
            Value::String(value) if value == "-Infinity" => Some(f64::NEG_INFINITY),
            number => Some(number.as_f64().unwrap()),
        };
        assert_eq!(
            request_timeout(configured).as_millis(),
            case["timer_bucket_ms"].as_u64().unwrap() as u128,
            "{}",
            case["raw"]
        );
    }
    assert!(!format!("{:?}", settings(String::new())).contains("fixture-token"));
}

struct UnreadBody(Arc<AtomicBool>);
impl Stream for UnreadBody {
    type Item = Result<bytes::Bytes, std::io::Error>;
    fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Pending
    }
}
impl Drop for UnreadBody {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

#[tokio::test]
async fn fire_and_forget_posts_source_payload_once_and_cancels_unread_bodies() {
    let (sent, mut received) = mpsc::unbounded_channel();
    let cancelled = Arc::new(AtomicBool::new(false));
    let body_dropped = cancelled.clone();
    let app = Router::new().route(
        "/events",
        post(move |request: Request| {
            let sent = sent.clone();
            let dropped = body_dropped.clone();
            async move {
                let headers = request.headers().clone();
                let bytes = axum::body::to_bytes(request.into_body(), 1 << 20)
                    .await
                    .unwrap();
                sent.send((headers, serde_json::from_slice::<Value>(&bytes).unwrap()))
                    .unwrap();
                (
                    axum::http::StatusCode::SERVICE_UNAVAILABLE,
                    Body::from_stream(UnreadBody(dropped)),
                )
                    .into_response()
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let analytics = Analytics::new(
        &crate::http::Outbound::for_tests(),
        settings(format!("http://{address}/events")),
    )
    .unwrap()
    .unwrap();
    analytics.fire_event(
        "START_CALL",
        json!({"call_id":"fixture-call","env":"overridden","user_id":"fixture-caller"}),
    );
    let (headers, payload) = tokio::time::timeout(Duration::from_secs(2), received.recv())
        .await
        .unwrap()
        .unwrap();
    let source = source();
    let expected = &source["transport"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["scenario"] == "503")
        .unwrap()["calls"][0];
    assert_eq!(payload, expected["payload"]);
    assert_eq!(
        headers["x-token"],
        expected["headers"]["x-token"].as_str().unwrap()
    );
    assert_eq!(
        headers["content-type"],
        expected["headers"]["Content-Type"].as_str().unwrap()
    );
    tokio::time::timeout(Duration::from_secs(2), async {
        while !cancelled.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("unread response body cancelled without draining");
    assert!(
        tokio::time::timeout(Duration::from_millis(30), received.recv())
            .await
            .is_err(),
        "POST 503 is never retried"
    );
    server.abort();
}

#[tokio::test]
async fn a_stalled_analytics_request_times_out_without_holding_the_caller() {
    let (entered, mut entries) = mpsc::unbounded_channel();
    let app = Router::new().route(
        "/events",
        post(move || {
            let entered = entered.clone();
            async move {
                entered.send(()).unwrap();
                std::future::pending::<()>().await;
                "never"
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut config = settings(format!("http://{address}/events"));
    config.request_timeout_ms = Some(20.0);
    let analytics = Analytics::new(&crate::http::Outbound::for_tests(), config)
        .unwrap()
        .unwrap();
    let weak = Arc::downgrade(&analytics.0);
    analytics.fire_event("ACCEPT_CALL", json!({"call_id":"fixture-call"}));
    drop(analytics);
    tokio::time::timeout(Duration::from_secs(2), entries.recv())
        .await
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while weak.upgrade().is_some() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("timed-out task releases adapter and client handle");
    assert!(
        tokio::time::timeout(Duration::from_millis(30), entries.recv())
            .await
            .is_err()
    );
    server.abort();
}

#[tokio::test]
async fn analytics_deadline_spans_the_entire_redirect_chain_until_headers() {
    let fixture = source();
    assert_eq!(
        fixture["deadline_chain"]["requests"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        fixture["deadline_chain"]["logs"][0]["data"]["error"],
        "Request aborted (timed out)"
    );
    let (entered, mut entries) = mpsc::unbounded_channel();
    let release = Arc::new(tokio::sync::Notify::new());
    let first_release = release.clone();
    let app = Router::new().fallback(move |request: Request| {
        let entered = entered.clone();
        let release = first_release.clone();
        async move {
            let first = request.uri().path() == "/deadline/start";
            entered.send(first).unwrap();
            if first {
                release.notified().await;
                (
                    axum::http::StatusCode::TEMPORARY_REDIRECT,
                    [("location", "/deadline/end")],
                )
                    .into_response()
            } else {
                std::future::pending::<axum::response::Response>().await
            }
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut config = settings(format!("http://{address}/deadline/start"));
    config.request_timeout_ms = Some(1000.0);
    let analytics = Analytics::new(&crate::http::Outbound::for_tests(), config)
        .unwrap()
        .unwrap();
    let weak = Arc::downgrade(&analytics.0);
    let started = tokio::time::Instant::now();
    analytics.fire_event("DEADLINE_FIXTURE", json!({}));
    drop(analytics);
    assert_eq!(entries.recv().await, Some(true));
    tokio::time::sleep(Duration::from_millis(100)).await;
    release.notify_one();
    assert_eq!(entries.recv().await, Some(false));
    let elapsed = started.elapsed();
    assert!(elapsed < Duration::from_millis(900));
    // Real HTTP establishes the second hop; controlled Tokio time then crosses
    // the original deadline while staying before a restarted per-hop deadline.
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(1) - elapsed + elapsed / 2).await;
    for _ in 0..64 {
        if weak.upgrade().is_none() {
            break;
        }
        tokio::task::yield_now().await;
    }
    let released = weak.upgrade().is_none();
    tokio::time::resume();
    server.abort();
    assert!(
        released,
        "the second redirect hop cannot restart the timeout"
    );
}
