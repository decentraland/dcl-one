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
        "/library/dcl-http/tests/fixtures/social-analytics-source-cases.json"
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
    assert!(
        Analytics::new(
            &http,
            Settings {
                api_url: Some(String::new()),
                ..Default::default()
            }
        )
        .unwrap()
        .is_none(),
        "posture-injected empty URL alone does not activate a provider"
    );
    for environment in ["", "prd"] {
        assert!(
            Analytics::new(
                &http,
                Settings {
                    environment: Some(environment.into()),
                    ..Default::default()
                }
            )
            .unwrap()
            .is_none(),
            "ENV alone does not configure analytics"
        );
    }
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

#[test]
fn shared_environment_alone_does_not_activate_analytics_but_partial_settings_fail() {
    let http = crate::http::Outbound::for_tests();
    for environment in ["", "prd", "dev"] {
        for api_url in [None, Some(String::new())] {
            assert!(
                Analytics::new(
                    &http,
                    Settings {
                        environment: Some(environment.into()),
                        api_url,
                        ..Default::default()
                    }
                )
                .unwrap()
                .is_none()
            );
        }
    }
    for partial in [
        Settings {
            context: Some(String::new()),
            ..Default::default()
        },
        Settings {
            api_token: Some(String::new()),
            ..Default::default()
        },
        Settings {
            api_url: Some("https://analytics.example/events".into()),
            ..Default::default()
        },
        Settings {
            request_timeout_ms: Some(1000.0),
            ..Default::default()
        },
    ] {
        assert!(
            Analytics::new(
                &http,
                Settings {
                    environment: Some("prd".into()),
                    ..partial
                }
            )
            .is_err(),
            "partial analytics settings must fail startup"
        );
    }
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
    config.request_timeout_ms = Some(5_000.0);
    let analytics = Analytics::new(&crate::http::Outbound::for_tests(), config)
        .unwrap()
        .unwrap();
    let weak = Arc::downgrade(&analytics.0);
    analytics.fire_event("ACCEPT_CALL", json!({"call_id":"fixture-call"}));
    drop(analytics);
    tokio::time::timeout(Duration::from_secs(30), entries.recv())
        .await
        .unwrap()
        .unwrap();
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(6)).await;
    task_turns().await;
    assert!(
        weak.upgrade().is_none(),
        "timed-out task releases adapter and client handle"
    );
    tokio::time::resume();
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
    // Controlled Tokio time from the first request: real HTTP progresses while
    // the test yields, and virtual time moves only on explicit advances.
    tokio::time::pause();
    analytics.fire_event("DEADLINE_FIXTURE", json!({}));
    drop(analytics);
    assert!(yield_until(&mut entries).await, "the first hop");
    // The first hop holds 600 ms of the one-second deadline.
    tokio::time::advance(Duration::from_millis(600)).await;
    release.notify_one();
    assert!(!yield_until(&mut entries).await, "the redirected hop");
    assert!(weak.upgrade().is_some(), "the chain is within its deadline");
    // 1,100 ms since the event crosses the deadline; 500 ms into the second
    // hop is still inside a deadline that a hop would have restarted.
    tokio::time::advance(Duration::from_millis(500)).await;
    let mut released = false;
    for _ in 0..10_000 {
        if weak.upgrade().is_none() {
            released = true;
            break;
        }
        tokio::task::yield_now().await;
    }
    tokio::time::resume();
    server.abort();
    assert!(
        released,
        "the second redirect hop cannot restart the timeout"
    );
}

/// Yields (keeping paused time still) until the server reports a request.
async fn yield_until(entries: &mut mpsc::UnboundedReceiver<bool>) -> bool {
    for _ in 0..1_000_000 {
        match entries.try_recv() {
            Ok(first) => return first,
            Err(mpsc::error::TryRecvError::Empty) => tokio::task::yield_now().await,
            Err(error) => panic!("{error}"),
        }
    }
    panic!("the request did not reach the server");
}

#[tokio::test]
async fn configured_analytics_header_budget_can_exceed_the_shared_read_deadline() {
    let (entered, mut entries) = mpsc::unbounded_channel();
    let release = Arc::new(tokio::sync::Notify::new());
    let response_release = release.clone();
    let app = Router::new().route(
        "/events",
        post(move || {
            let entered = entered.clone();
            let release = response_release.clone();
            async move {
                entered.send(()).unwrap();
                release.notified().await;
                "unused body"
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/conformance/social/provider-boundary-source-cases.json"
    )))
    .unwrap();
    let mut config = settings(format!("http://{address}/events"));
    config.request_timeout_ms = Some(
        fixture["analytics"]["configured_header_timer_ms"]
            .as_f64()
            .unwrap(),
    );
    let shared = crate::http::Outbound::new(crate::http::HttpClientCfg::serve(16)).unwrap();
    let analytics = Analytics::new(&shared, config).unwrap().unwrap();
    let weak = Arc::downgrade(&analytics.0);
    analytics.fire_event("SLOW_HEADERS_FIXTURE", json!({}));
    drop(analytics);
    entries.recv().await.unwrap();
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(26)).await;
    for _ in 0..64 {
        tokio::task::yield_now().await;
    }
    assert!(
        weak.upgrade().is_some(),
        "the configured source 60s timer outlives the shared 25s read deadline"
    );
    tokio::time::resume();
    release.notify_one();
    tokio::time::timeout(Duration::from_secs(2), async {
        while weak.upgrade().is_some() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(
        entries.try_recv().is_err(),
        "no retry after successful headers"
    );
    server.abort();
}

fn provider_boundary_source() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/conformance/social/provider-boundary-source-cases.json"
    )))
    .unwrap()
}
async fn task_turns() {
    for _ in 0..128 {
        tokio::task::yield_now().await;
    }
}

#[tokio::test]
async fn native_analytics_headers_budget_precedes_a_larger_configured_timer() {
    let fixture = provider_boundary_source();
    let source = &fixture["native_analytics"]["headers"];
    assert_eq!(source["native_budget_ms"], 300_000);
    assert_eq!(source["before_deadline"]["completed"], false);
    assert_eq!(source["completed"], true);
    let (entered, mut entries) = mpsc::unbounded_channel();
    let app = Router::new().fallback(move || {
        let entered = entered.clone();
        async move {
            entered.send(()).unwrap();
            std::future::pending::<String>().await
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut config = settings(url);
    config.request_timeout_ms = Some(source["sdk_timer_ms"].as_f64().unwrap());
    let analytics = Analytics::new(
        &crate::http::Outbound::new(crate::http::HttpClientCfg::serve(16)).unwrap(),
        config,
    )
    .unwrap()
    .unwrap();
    let weak = Arc::downgrade(&analytics.0);
    analytics.fire_event("NATIVE_HEADERS_FIXTURE", json!({}));
    drop(analytics);
    entries.recv().await.unwrap();
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(299)).await;
    task_turns().await;
    assert!(weak.upgrade().is_some());
    tokio::time::advance(Duration::from_secs(2)).await;
    task_turns().await;
    assert!(
        weak.upgrade().is_none(),
        "native headers timeout remains below the configured 600s SDK deadline"
    );
    tokio::time::resume();
    assert!(
        entries.try_recv().is_err(),
        "a timed out analytics POST is not retried"
    );
    server.abort();
}

#[tokio::test]
async fn native_analytics_connect_budget_also_covers_a_stalled_tls_handshake() {
    let fixture = provider_boundary_source();
    let source = &fixture["native_analytics"]["connect"];
    assert_eq!(source["native_budget_ms"], 10_000);
    assert_eq!(source["before_deadline"]["completed"], false);
    assert_eq!(source["completed"], true);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("https://{}", listener.local_addr().unwrap());
    let (entered, received) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        entered.send(()).unwrap();
        std::future::pending::<()>().await;
        drop(socket);
    });
    let mut config = settings(url);
    config.request_timeout_ms = Some(source["sdk_timer_ms"].as_f64().unwrap());
    let analytics = Analytics::new(
        &crate::http::Outbound::new(crate::http::HttpClientCfg::serve(16)).unwrap(),
        config,
    )
    .unwrap()
    .unwrap();
    let weak = Arc::downgrade(&analytics.0);
    analytics.fire_event("NATIVE_CONNECT_FIXTURE", json!({}));
    drop(analytics);
    received.await.unwrap();
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(9)).await;
    task_turns().await;
    assert!(weak.upgrade().is_some());
    tokio::time::advance(Duration::from_secs(2)).await;
    task_turns().await;
    assert!(
        weak.upgrade().is_none(),
        "the native 10s connection budget precedes SDK timeout"
    );
    tokio::time::resume();
    server.abort();
}

#[tokio::test]
async fn native_analytics_headers_budget_restarts_per_redirect_inside_the_sdk_deadline() {
    let fixture = provider_boundary_source();
    let source = &fixture["native_analytics"]["redirect-chain"];
    assert_eq!(source["after_second_hop"]["completed"], false);
    assert!(
        source["after_second_hop"]["elapsed_tick_ms"]
            .as_u64()
            .unwrap()
            > 300_000
    );
    let (entered, mut entries) = mpsc::unbounded_channel();
    let release = Arc::new(tokio::sync::Notify::new());
    let response_release = release.clone();
    let app = Router::new().fallback(move |request: Request| {
        let entered = entered.clone();
        let release = response_release.clone();
        async move {
            let first = request.uri().path() == "/start";
            entered.send(first).unwrap();
            release.notified().await;
            if first {
                (
                    axum::http::StatusCode::TEMPORARY_REDIRECT,
                    [("location", "/end")],
                )
                    .into_response()
            } else {
                "unused body".into_response()
            }
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/start", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut config = settings(url);
    config.request_timeout_ms = Some(source["sdk_timer_ms"].as_f64().unwrap());
    let analytics = Analytics::new(
        &crate::http::Outbound::new(crate::http::HttpClientCfg::serve(16)).unwrap(),
        config,
    )
    .unwrap()
    .unwrap();
    let weak = Arc::downgrade(&analytics.0);
    analytics.fire_event("NATIVE_CHAIN_FIXTURE", json!({}));
    drop(analytics);
    assert_eq!(entries.recv().await, Some(true));
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(240)).await;
    task_turns().await;
    assert!(weak.upgrade().is_some());
    tokio::time::resume();
    release.notify_one();
    assert_eq!(entries.recv().await, Some(false));
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(240)).await;
    task_turns().await;
    assert!(
        weak.upgrade().is_some(),
        "a redirect hop gets its own native headers timeout"
    );
    tokio::time::resume();
    release.notify_one();
    tokio::time::timeout(Duration::from_secs(2), async {
        while weak.upgrade().is_some() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    server.abort();
}
