//! The relay's public routes: `POST /{network}` (one request or a batch),
//! `GET /{network}` as the WebSocket upgrade (each text frame one payload,
//! answered in order on the same socket), `GET /health`, and `GET /`, the
//! networks index the old nginx vhost answered statically.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Map, Value};

use super::{Shared, relay};

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/", get(index))
        .route("/health", get(|| async { "ok" }))
        .route("/{network}", post(http_rpc).get(ws_upgrade))
}

/// `{"mainnet": "<public_base>/mainnet", ...}` for every configured network.
async fn index(State(state): State<Shared>) -> Response {
    let base = &state.settings.public_base;
    let networks: Map<String, Value> = state
        .upstreams_snapshot()
        .keys()
        .map(|n| (n.clone(), Value::String(format!("{base}/{n}"))))
        .collect();
    Json(Value::Object(networks)).into_response()
}

async fn http_rpc(
    Path(network): Path<String>,
    State(state): State<Shared>,
    Json(payload): Json<Value>,
) -> Response {
    let resp = relay::handle_payload(&state, &network, payload).await;
    (StatusCode::OK, Json(resp)).into_response()
}

async fn ws_upgrade(
    Path(network): Path<String>,
    State(state): State<Shared>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state, network))
}

async fn handle_socket(mut socket: WebSocket, state: Shared, network: String) {
    while let Some(incoming) = socket.recv().await {
        let Ok(msg) = incoming else { break };
        match msg {
            Message::Text(t) => {
                let reply = match serde_json::from_str::<Value>(&t) {
                    Ok(payload) => relay::handle_payload(&state, &network, payload).await,
                    Err(e) => relay::parse_error(&e),
                };
                if socket
                    .send(Message::Text(reply.to_string().into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            Message::Ping(ref p) if socket.send(Message::Pong(p.clone())).await.is_err() => break,
            Message::Close(_) => break,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::future::IntoFuture;

    use axum::body::Body;
    use axum::http::{Request, header};
    use futures::{SinkExt, StreamExt};
    use serde_json::json;
    use tower::ServiceExt;

    use super::*;
    use crate::ops::rpc::relay::tests::{mock_upstream, test_state};
    use crate::ops::rpc::{Settings, State as RpcState};

    async fn call(
        app: &Router,
        method: &str,
        path: &str,
        bearer: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut req = Request::builder().method(method).uri(path);
        if let Some(b) = bearer {
            req = req.header(header::AUTHORIZATION, format!("Bearer {b}"));
        }
        let body = match body {
            Some(v) => {
                req = req.header(header::CONTENT_TYPE, "application/json");
                Body::from(v.to_string())
            }
            None => Body::empty(),
        };
        let resp = app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes)
                .unwrap_or(Value::String(String::from_utf8_lossy(&bytes).into()))
        };
        (status, value)
    }

    #[tokio::test]
    async fn post_relays_and_the_index_lists_the_networks() {
        let (url, upstream) = mock_upstream(true).await;
        let app = crate::ops::rpc::router(test_state(&[("polygon", &url), ("mainnet", &url)]));
        let (status, out) = call(
            &app,
            "POST",
            "/polygon",
            None,
            Some(json!({"jsonrpc": "2.0", "id": 1, "method": "net_version"})),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["result"], json!("137"));
        assert_eq!(upstream.posts.load(std::sync::atomic::Ordering::SeqCst), 1);
        let (status, out) = call(
            &app,
            "POST",
            "/polygon",
            None,
            Some(json!({"id": 2, "method": "eth_sendRawTransaction", "params": ["0x"]})),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::OK,
            "a refused method is a JSON-RPC error, not an HTTP one"
        );
        assert_eq!(out["error"]["code"], json!(-32601));
        let (status, out) = call(&app, "GET", "/", None, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            out,
            json!({"mainnet": "http://localhost/rpc/mainnet", "polygon": "http://localhost/rpc/polygon"})
        );
        assert_eq!(
            call(&app, "GET", "/health", None, None).await,
            (StatusCode::OK, Value::String("ok".into()))
        );
    }

    #[tokio::test]
    async fn admin_routes_fail_closed_and_edit_the_process_state() {
        let mut settings = Settings::for_tests(&[("polygon", "https://example/polygon")]);
        settings.admin_token = Some("t0k".into());
        let state = RpcState::new(
            settings,
            crate::http::Outbound::for_tests(),
            crate::ops::rpc::tests::lazy_pool(),
        );
        let app = crate::ops::rpc::router(state.clone());
        assert_eq!(
            call(&app, "GET", "/admin/rpc/config", None, None).await.0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            call(&app, "GET", "/admin/rpc/config", Some("nope"), None)
                .await
                .0,
            StatusCode::FORBIDDEN
        );
        let (status, cfg) = call(&app, "GET", "/admin/rpc/config", Some("t0k"), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(cfg["networks"]["polygon"], json!("https://example/polygon"));
        assert!(
            cfg["methods"]
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m == "eth_call")
        );

        let (status, out) = call(
            &app,
            "POST",
            "/admin/rpc/methods",
            Some("t0k"),
            Some(json!({"method": "eth_feeHistory"})),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["added"], json!(true));
        assert!(state.is_method_allowed("eth_feeHistory"));
        let (status, _) = call(&app, "POST", "/admin/rpc/methods/reset", Some("t0k"), None).await;
        assert_eq!(status, StatusCode::OK);
        assert!(!state.is_method_allowed("eth_feeHistory"));

        let (status, out) = call(
            &app,
            "POST",
            "/admin/rpc/networks",
            Some("t0k"),
            Some(json!({"network": "MAINNET", "url": "http://10.0.0.5:8545"})),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{out}");
        let (status, out) = call(
            &app,
            "POST",
            "/admin/rpc/networks",
            Some("t0k"),
            Some(json!({"network": "MAINNET", "url": "https://rpc.example.org/v3/KEY"})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{out}");
        assert_eq!(
            state.upstream_for("mainnet").as_deref(),
            Some("https://rpc.example.org/v3/KEY")
        );
        let (status, out) = call(
            &app,
            "DELETE",
            "/admin/rpc/networks/mainnet",
            Some("t0k"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["removed"], json!(true));
        assert_eq!(state.upstream_for("mainnet"), None);
    }

    /// The audit rows of the edits above on the scratch database: host,
    /// never the URL.
    #[tokio::test]
    async fn admin_edits_are_audited_with_the_host_only() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let mut settings = Settings::for_tests(&[]);
        settings.admin_token = Some("t0k".into());
        let state = RpcState::new(settings, crate::http::Outbound::for_tests(), pool.clone());
        let app = crate::ops::rpc::router(state);
        let network = format!("net{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);
        let (status, _) = call(
            &app,
            "POST",
            "/admin/rpc/networks",
            Some("t0k"),
            Some(json!({"network": network, "url": "https://rpc.example.org/v3/SECRETKEY"})),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = call(
            &app,
            "DELETE",
            &format!("/admin/rpc/networks/{network}"),
            Some("t0k"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let rows: Vec<(String, String, String, Value)> = sqlx::query_as(
            "SELECT actor, action, result, detail FROM ops.admin_audit WHERE domain = 'rpc' AND target = $1 ORDER BY id",
        )
        .bind(&network)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            (rows[0].0.as_str(), rows[0].1.as_str(), rows[0].2.as_str()),
            ("token:CATALYRST_RPC_ADMIN_TOKEN", "networks.upsert", "ok")
        );
        assert_eq!(rows[0].3["host"], json!("rpc.example.org"));
        assert!(!rows[0].3.to_string().contains("SECRETKEY"));
        assert_eq!(rows[1].1, "networks.remove");
        assert_eq!(rows[1].3["removed"], json!(true));
    }

    #[tokio::test]
    async fn the_websocket_answers_each_frame_in_order() {
        let (url, _upstream) = mock_upstream(true).await;
        let app = crate::ops::rpc::router(test_state(&[("polygon", &url)]));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(axum::serve(listener, app).into_future());
        let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/polygon"))
            .await
            .unwrap();
        ws.send(tokio_tungstenite::tungstenite::Message::Text(
            json!({"id": 1, "method": "net_version"}).to_string().into(),
        ))
        .await
        .unwrap();
        ws.send(tokio_tungstenite::tungstenite::Message::Text(
            "not json".into(),
        ))
        .await
        .unwrap();
        ws.send(tokio_tungstenite::tungstenite::Message::Text(
            json!([{"id": "b", "method": "eth_blockNumber"}])
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
        let mut replies = Vec::new();
        while replies.len() < 3 {
            if let tokio_tungstenite::tungstenite::Message::Text(t) =
                ws.next().await.unwrap().unwrap()
            {
                replies.push(serde_json::from_str::<Value>(&t).unwrap())
            }
        }
        assert_eq!(replies[0]["id"], json!(1));
        assert_eq!(replies[0]["result"], json!("137"));
        assert_eq!(replies[1]["error"]["code"], json!(-32700));
        assert_eq!(replies[2][0]["id"], json!("b"));
        assert_eq!(replies[2][0]["result"], json!("0x10"));
        ws.close(None).await.unwrap();
    }
}
