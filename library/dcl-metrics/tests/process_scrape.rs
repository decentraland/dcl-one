//! Exercise HTTP routing, the upgrade and a service declaration through one real scrape.
#![allow(clippy::unwrap_used, reason = "test support")]

use axum::{Router, extract::WebSocketUpgrade, http::StatusCode, response::Response, routing::get};
use dcl_metrics::{CounterFamily, PrometheusBuilder, Registry, http, ws::Socket};
use futures::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};

const LOOKUPS: CounterFamily<1> = CounterFamily::new(
    "example_lookups_total",
    "Completed example lookups.",
    None,
    &["result"],
);

async fn lookup() -> StatusCode {
    LOOKUPS.handle(["missing"]).increment(1);
    StatusCode::NOT_FOUND
}

async fn echo(upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(|socket| async move {
        let mut socket = Socket::new(socket, "echo");
        if let Some(Ok(message)) = socket.next().await {
            socket.send(message).await.unwrap();
        }
    })
}

#[tokio::test]
async fn one_process_exposes_service_http_and_websocket_observations() {
    let registry = Registry::new(&[&[LOOKUPS.descriptor]]).unwrap();
    let handle = registry.install(PrometheusBuilder::new()).unwrap();
    let service = Router::new()
        .route("/lookup/{id}", get(lookup))
        .route("/ws", get(echo));
    let app = http::instrument(service, "example")
        .route("/metrics", get(move || dcl_metrics::render(handle.clone())));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(axum::serve(listener, app).into_future());

    let response = reqwest::get(format!("http://{addr}/lookup/42"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let (mut client, _) = connect_async(format!("ws://{addr}/ws")).await.unwrap();
    client.send(Message::text("hello")).await.unwrap();
    assert_eq!(
        client.next().await.unwrap().unwrap(),
        Message::text("hello")
    );

    let response = reqwest::get(format!("http://{addr}/metrics"))
        .await
        .unwrap();
    assert_eq!(
        response.headers()["content-type"],
        dcl_metrics::CONTENT_TYPE
    );
    let body = response.text().await.unwrap();
    println!("{body}");
    for expected in [
        "example_lookups_total{result=\"missing\"} 1",
        "dcl_ws_active_connections{protocol=\"echo\"} 0",
        "dcl_ws_message_bytes_total{protocol=\"echo\",direction=\"in\",outcome=\"received\"} 5",
        "dcl_ws_message_bytes_total{protocol=\"echo\",direction=\"out\",outcome=\"flushed\"} 5",
        "dcl_http_requests_total{component=\"example\",route=\"/lookup/{id}\",method=\"GET\",status_code=\"404\"} 1",
        "dcl_http_requests_total{component=\"example\",route=\"/ws\",method=\"GET\",status_code=\"101\"} 1",
    ] {
        assert!(
            body.lines().any(|line| line == expected),
            "missing {expected} in {body}"
        );
    }
    server.abort();
}
