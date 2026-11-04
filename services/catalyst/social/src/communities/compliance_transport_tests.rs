use super::*;
use axum::{Router, body::Body, response::IntoResponse, routing::post};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::mpsc;

fn source() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/conformance/social/provider-boundary-source-cases.json"
    )))
    .unwrap()
}
fn adapter(url: String) -> Adapter {
    Adapter::new(Settings {
        api_key: Some("fixture-key".into()),
        base_url: url,
        ..Default::default()
    })
    .unwrap()
}
fn provider_response(padding: usize) -> String {
    json!({"object":"response","padding":"x".repeat(padding),"output":[{"type":"message","content":[{"type":"output_text","text":source()["large_response"]["result"].to_string()}]}]}).to_string()
}

#[tokio::test]
async fn response_above_four_mebibytes_matches_the_source_sdk_without_a_size_cap() {
    let fixture = source();
    let body =
        provider_response(fixture["large_response"]["padding_bytes"].as_u64().unwrap() as usize);
    assert_eq!(body.len() as u64, fixture["large_response"]["bytes"]);
    let requests = Arc::new(AtomicUsize::new(0));
    let count = requests.clone();
    let app = Router::new().route(
        "/responses",
        post(move || {
            let body = body.clone();
            count.fetch_add(1, Ordering::SeqCst);
            async move { ([("content-type", "application/json")], body) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let result = adapter(url)
        .validate(Content {
            name: Some("Large fixture"),
            description: None,
            thumbnail: None,
        })
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        fixture["large_response"]["result"]
    );
    assert_eq!(
        requests.load(Ordering::SeqCst) as u64,
        fixture["large_response"]["requests"]
    );
    server.abort();
}

// Establish a real response before pausing time. Every later chunk is less
// than the unchanged 300s inactivity budget after its predecessor.
#[tokio::test]
async fn streaming_body_can_cross_the_sdk_header_deadline_while_progress_continues() {
    streaming_body(true).await;
}

#[tokio::test]
async fn body_inactivity_terminates_once_without_retries_like_native_source_fetch() {
    streaming_body(false).await;
}

async fn streaming_body(finish: bool) {
    let fixture = source();
    assert_eq!(fixture["sdk_body_phase"]["before"]["timer_ms"], 600_000);
    assert_eq!(fixture["sdk_body_phase"]["before"]["timer_cleared"], true);
    assert_eq!(fixture["sdk_body_phase"]["after"]["signal_aborted"], false);
    let (chunks, receiver) = mpsc::unbounded_channel::<bytes::Bytes>();
    let receiver = Arc::new(tokio::sync::Mutex::new(Some(receiver)));
    let (entered, mut entries) = mpsc::unbounded_channel();
    let (polled, mut polls) = mpsc::unbounded_channel();
    let app = Router::new().route(
        "/responses",
        post(move || {
            let receiver = receiver.clone();
            let entered = entered.clone();
            let polled = polled.clone();
            async move {
                let receiver = receiver.lock().await.take().unwrap();
                entered.send(()).unwrap();
                let stream = futures::stream::unfold(receiver, move |mut receiver| {
                    let polled = polled.clone();
                    async move {
                        let _ = polled.send(());
                        receiver
                            .recv()
                            .await
                            .map(|chunk| (Ok::<_, std::io::Error>(chunk), receiver))
                    }
                });
                (
                    [("content-type", "application/json")],
                    Body::from_stream(stream),
                )
                    .into_response()
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    chunks
        .send(bytes::Bytes::from_static(b"{\"padding\":\""))
        .unwrap();
    let pending = tokio::spawn(async move {
        adapter(url)
            .validate(Content {
                name: Some("Fixture"),
                description: None,
                thumbnail: None,
            })
            .await
    });
    entries.recv().await.unwrap();
    // The server asks for the second chunk once the first is written to the
    // socket; clock control starts after that, not after a wall-clock wait.
    polls.recv().await.unwrap();
    polls.recv().await.unwrap();
    tokio::time::pause();
    for chunk in 0..3 {
        tokio::time::advance(Duration::from_secs(240)).await;
        if finish || chunk < 2 {
            chunks.send(bytes::Bytes::from_static(b"x")).unwrap();
        }
        for _ in 0..128 {
            tokio::task::yield_now().await;
        }
        assert!(
            !pending.is_finished(),
            "progressing body has no 600s total deadline"
        );
    }
    if !finish {
        assert_eq!(
            fixture["native_socket_body"]["after_sdk_deadline"]["completed"],
            false
        );
        assert!(
            fixture["native_socket_body"]["after_sdk_deadline"]["elapsed_tick_ms"]
                .as_u64()
                .unwrap()
                > 600_000
        );
        tokio::time::advance(Duration::from_secs(62)).await;
        for _ in 0..128 {
            tokio::task::yield_now().await;
        }
        assert!(
            pending.is_finished(),
            "300 seconds without body progress terminates the stream"
        );
        tokio::time::resume();
        assert_eq!(
            pending.await.unwrap().unwrap_err(),
            fixture["native_socket_body"]["at_inactivity_timeout"]["error"]
                .as_str()
                .unwrap()
        );
        assert_eq!(fixture["native_socket_body"]["requests"], 1);
        server.abort();
        return;
    }
    tokio::time::resume();
    let tail = provider_response(0);
    chunks
        .send(bytes::Bytes::from(format!("\",{}", &tail[1..])))
        .unwrap();
    drop(chunks);
    let result = tokio::time::timeout(Duration::from_secs(2), pending)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        fixture["sdk_body_phase"]["result"]
    );
    server.abort();
}

/// The whole provider effect is bounded: a provider that never answers fails
/// as unavailable at EFFECT_TIMEOUT, before any of the source's per-attempt
/// 600 s deadlines.
#[tokio::test]
async fn the_whole_provider_effect_is_bounded() {
    let (entered, mut entries) = mpsc::unbounded_channel();
    let app = Router::new().route(
        "/responses",
        post(move || {
            let entered = entered.clone();
            async move {
                entered.send(()).unwrap();
                std::future::pending::<()>().await;
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let pending = tokio::spawn(async move {
        adapter(url)
            .validate_bounded(Content {
                name: Some("Fixture"),
                description: None,
                thumbnail: None,
            })
            .await
    });
    // The request reached the provider; from here only controlled time moves.
    entries.recv().await.unwrap();
    tokio::time::pause();
    tokio::time::advance(EFFECT_TIMEOUT - Duration::from_secs(1)).await;
    for _ in 0..128 {
        tokio::task::yield_now().await;
    }
    assert!(!pending.is_finished(), "inside the bound the call waits");
    tokio::time::advance(Duration::from_secs(1)).await;
    tokio::time::resume();
    // Its deadline has passed: only scheduling remains, not provider time.
    let result = tokio::time::timeout(Duration::from_secs(10), pending)
        .await
        .expect("the bound ends the effect");
    assert_eq!(
        result.unwrap().unwrap_err(),
        "Unexpected error during compliance validation: Request timed out."
    );
    server.abort();
}
