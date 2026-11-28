//! The JSON-RPC relay proper: one request or one batch planned against the
//! allowlist, the network map and the memo, the remainder forwarded to the
//! upstream as one batch (ids rewritten to slot indexes, anything the
//! upstream does not answer retried per element), every answer normalised
//! to a JSON-RPC 2.0 envelope echoing the caller's id.

use std::collections::HashMap;
use std::time::Duration;

use futures::StreamExt;
use serde_json::{Value, json};

use super::{RpcMemo, Shared, State, UPSTREAM_TIMEOUT};

/// The most requests one batch may carry; a longer batch is refused whole
/// with one `-32600` error and never reaches the upstream.
pub const MAX_BATCH: usize = 100;

/// The most bytes read from one upstream answer.
const MAX_UPSTREAM_BODY: usize = 16 * 1024 * 1024;

/// The most per-element forwards in flight when the upstream leaves batch
/// slots unanswered.
const FALLBACK_CONCURRENCY: usize = 8;

/// The most per-element fallback forwards in flight across every request
/// at once (`State::fallback_slots`).
pub(super) const FALLBACK_SLOTS: usize = 32;

/// How long the per-element fallback of one batch may run; slots still
/// unanswered then get a `-32603` error.
const FALLBACK_BUDGET: Duration = UPSTREAM_TIMEOUT;

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
}

fn normalize_response(id: Value, body: Value) -> Value {
    let Value::Object(mut map) = body else {
        return rpc_error(
            id,
            -32603,
            "Upstream returned a non-object JSON-RPC response",
        );
    };
    map.insert("jsonrpc".into(), Value::String("2.0".into()));
    map.insert("id".into(), id.clone());
    if !map.contains_key("result") && !map.contains_key("error") {
        return rpc_error(
            id,
            -32603,
            "Upstream response carried neither result nor error",
        );
    }
    Value::Object(map)
}

fn id_of(req: &Value) -> Value {
    req.get("id").cloned().unwrap_or(Value::Null)
}

struct Forward {
    id: Value,
    req: Value,
    memo: Option<(String, Duration)>,
}

enum Plan {
    Ready(Value),
    Forward(Forward),
}

fn memo_key(req: &Value, network: &str, method: &str) -> Option<(String, Duration)> {
    let ttl = RpcMemo::ttl_for(method)?;
    let no_params = match req.get("params") {
        None | Some(Value::Null) => true,
        Some(Value::Array(params)) => params.is_empty(),
        Some(_) => false,
    };
    no_params.then(|| (format!("{}:{method}", network.to_ascii_lowercase()), ttl))
}

fn plan(state: &State, network: &str, upstream: Option<&str>, req: Value) -> Plan {
    let id = id_of(&req);
    let Some(method) = req.get("method").and_then(|m| m.as_str()) else {
        return Plan::Ready(rpc_error(id, -32600, "Invalid Request: missing method"));
    };
    if !state.is_method_allowed(method) {
        return Plan::Ready(rpc_error(
            id,
            -32601,
            &format!("Method not allowed on read-only relay: {method}"),
        ));
    }
    if upstream.is_none() {
        return Plan::Ready(rpc_error(
            id,
            -32602,
            &format!("Unsupported network: {network}"),
        ));
    }
    let memo = memo_key(&req, network, method);
    if let Some((key, _)) = &memo
        && let Some(body) = state.memo.get(key)
    {
        return Plan::Ready(normalize_response(id, body));
    }
    Plan::Forward(Forward { id, req, memo })
}

fn remember(state: &State, memo: Option<(String, Duration)>, body: &Value) {
    if let Some((key, ttl)) = memo {
        state.memo.put(key, ttl, body);
    }
}

#[derive(Debug)]
enum PostError {
    Transport(String),
    /// The answer could not be read whole: over `MAX_UPSTREAM_BODY` or cut
    /// off. Never retried per element.
    Unreadable(String),
    InvalidJson(String),
}

impl std::fmt::Display for PostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(error) => write!(f, "Upstream request failed: {error}"),
            Self::Unreadable(error) => write!(f, "Upstream response unreadable: {error}"),
            Self::InvalidJson(error) => write!(f, "Upstream returned invalid JSON: {error}"),
        }
    }
}

/// One upstream POST: through the guarded client, or the pooled client as
/// is when the operator opted private upstreams in. Error texts never carry
/// the URL (`SendError` strips it; `reqwest::Error::without_url`).
async fn post_json(state: &State, upstream: &str, body: &Value) -> Result<Value, PostError> {
    let resp = if state.settings.allow_private_upstream {
        state
            .http
            .raw()
            .post(upstream)
            .timeout(UPSTREAM_TIMEOUT)
            .json(body)
            .send()
            .await
            .map_err(|e| PostError::Transport(e.without_url().to_string()))?
    } else {
        state
            .http
            .post(upstream)
            .timeout(UPSTREAM_TIMEOUT)
            .json(body)
            .send()
            .await
            .map_err(|e| PostError::Transport(e.to_string()))?
    };
    let bytes = crate::http::read_body_capped(resp, MAX_UPSTREAM_BODY)
        .await
        .map_err(|e| PostError::Unreadable(e.to_string()))?;
    serde_json::from_slice(&bytes)
        .map_err(|_| PostError::InvalidJson("error decoding response body".into()))
}

pub async fn handle_single(state: &Shared, network: &str, req: Value) -> Value {
    let upstream = state.upstream_for(network);
    match plan(state, network, upstream.as_deref(), req) {
        Plan::Ready(body) => body,
        Plan::Forward(item) => {
            let body = forward(state, upstream.as_deref().unwrap_or(""), item.id, item.req).await;
            remember(state, item.memo, &body);
            body
        }
    }
}

async fn forward(state: &State, upstream: &str, id: Value, req: Value) -> Value {
    match post_json(state, upstream, &req).await {
        Ok(body) => normalize_response(id, body),
        Err(error) => rpc_error(id, -32603, &error.to_string()),
    }
}

/// One upstream batch with ids rewritten to slot indexes; anything the batch
/// does not answer falls back to per-element forwards, at most
/// `FALLBACK_CONCURRENCY` at a time, within the relay-wide
/// `FALLBACK_SLOTS`, and for at most `budget` in all.
async fn forward_batch(
    state: &Shared,
    upstream: &str,
    items: &[Forward],
    budget: Duration,
) -> Vec<Value> {
    if items.len() == 1 {
        return vec![forward(state, upstream, items[0].id.clone(), items[0].req.clone()).await];
    }
    let wire: Vec<Value> = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let mut req = item.req.clone();
            if let Value::Object(map) = &mut req {
                map.insert("id".into(), json!(i));
            }
            req
        })
        .collect();
    let body = match post_json(state, upstream, &Value::Array(wire)).await {
        Ok(body) => Some(body),
        Err(error @ (PostError::Transport(_) | PostError::Unreadable(_))) => {
            let message = error.to_string();
            return items
                .iter()
                .map(|item| rpc_error(item.id.clone(), -32603, &message))
                .collect();
        }
        Err(PostError::InvalidJson(_)) => None,
    };
    let mut by_slot: HashMap<usize, Value> = HashMap::new();
    if let Some(Value::Array(elements)) = body {
        for element in elements {
            // An id beyond `usize` names no slot.
            if let Some(slot) = element
                .get("id")
                .and_then(Value::as_u64)
                .and_then(|id| usize::try_from(id).ok())
            {
                by_slot.insert(slot, element);
            }
        }
    }
    let mut out: Vec<Option<Value>> = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            by_slot
                .remove(&i)
                .map(|element| normalize_response(item.id.clone(), element))
        })
        .collect();
    let unanswered: Vec<usize> = (0..items.len()).filter(|&i| out[i].is_none()).collect();
    let mut retries = futures::stream::iter(unanswered)
        .map(|i| async move {
            let _slot = state.fallback_slots.acquire().await;
            let item = &items[i];
            (
                i,
                forward(state, upstream, item.id.clone(), item.req.clone()).await,
            )
        })
        .buffer_unordered(FALLBACK_CONCURRENCY);
    let deadline = tokio::time::Instant::now() + budget;
    while let Ok(Some((i, body))) = tokio::time::timeout_at(deadline, retries.next()).await {
        out[i] = Some(body);
    }
    out.into_iter()
        .zip(items)
        .map(|(body, item)| {
            body.unwrap_or_else(|| {
                rpc_error(item.id.clone(), -32603, "Upstream did not answer in time")
            })
        })
        .collect()
}

pub async fn handle_payload(state: &Shared, network: &str, payload: Value) -> Value {
    match payload {
        Value::Array(items) => {
            if items.is_empty() {
                return rpc_error(Value::Null, -32600, "Invalid Request: empty batch");
            }
            if items.len() > MAX_BATCH {
                return rpc_error(
                    Value::Null,
                    -32600,
                    &format!("Invalid Request: a batch carries at most {MAX_BATCH} requests"),
                );
            }
            let upstream = state.upstream_for(network);
            let mut out: Vec<Option<Value>> = Vec::with_capacity(items.len());
            let mut slots: Vec<usize> = Vec::new();
            let mut forwards: Vec<Forward> = Vec::new();
            for (slot, item) in items.into_iter().enumerate() {
                match plan(state, network, upstream.as_deref(), item) {
                    Plan::Ready(body) => out.push(Some(body)),
                    Plan::Forward(item) => {
                        out.push(None);
                        slots.push(slot);
                        forwards.push(item);
                    }
                }
            }
            if !forwards.is_empty() {
                let bodies = forward_batch(
                    state,
                    upstream.as_deref().unwrap_or(""),
                    &forwards,
                    FALLBACK_BUDGET,
                )
                .await;
                for ((slot, item), body) in slots.into_iter().zip(forwards).zip(bodies) {
                    remember(state, item.memo, &body);
                    out[slot] = Some(body);
                }
            }
            Value::Array(out.into_iter().flatten().collect())
        }
        single @ Value::Object(_) => handle_single(state, network, single).await,
        other => rpc_error(
            id_of(&other),
            -32600,
            "Invalid Request: expected object or array",
        ),
    }
}

pub fn parse_error(e: &serde_json::Error) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": Value::Null,
        "error": { "code": -32700, "message": format!("Parse error: {e}") },
    })
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::ops::rpc::{Settings, State};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn error_envelope_is_jsonrpc_2_0_conformant() {
        let e = rpc_error(json!(7), -32601, "nope");
        assert_eq!(e["jsonrpc"], json!("2.0"));
        assert_eq!(e["id"], json!(7));
        assert_eq!(e["error"]["code"], json!(-32601));
        assert!(
            e.get("result").is_none(),
            "error response must not carry result"
        );
    }

    #[test]
    fn normalize_forces_jsonrpc_and_echoes_id() {
        let out = normalize_response(json!(42), json!({ "id": "abc", "result": "0x1" }));
        assert_eq!(out["jsonrpc"], json!("2.0"));
        assert_eq!(out["id"], json!(42));
        assert_eq!(out["result"], json!("0x1"));
        assert!(out.get("error").is_none());
    }

    #[test]
    fn normalize_preserves_upstream_error() {
        let out = normalize_response(
            json!(1),
            json!({ "error": { "code": -32000, "message": "reverted" } }),
        );
        assert_eq!(out["jsonrpc"], json!("2.0"));
        assert_eq!(out["id"], json!(1));
        assert_eq!(out["error"]["code"], json!(-32000));
        assert!(out.get("result").is_none());
    }

    #[test]
    fn normalize_repairs_resultless_and_errorless_body() {
        let out = normalize_response(json!(3), json!({ "foo": "bar" }));
        assert_eq!(out["id"], json!(3));
        assert_eq!(out["error"]["code"], json!(-32603));
    }

    #[test]
    fn normalize_rejects_non_object_body() {
        let out = normalize_response(json!(9), json!(["not", "an", "object"]));
        assert_eq!(out["id"], json!(9));
        assert_eq!(out["error"]["code"], json!(-32603));
    }

    pub(crate) fn test_state(networks: &[(&str, &str)]) -> Shared {
        State::new(
            Settings::for_tests(networks),
            crate::http::Outbound::for_tests(),
            crate::ops::rpc::tests::lazy_pool(),
        )
    }

    #[tokio::test]
    async fn unsupported_network_returns_invalid_params() {
        let state = test_state(&[("polygon", "https://example/polygon")]);
        let req = json!({ "jsonrpc": "2.0", "id": 5, "method": "eth_blockNumber" });
        let out = handle_single(&state, "solana", req).await;
        assert_eq!(out["jsonrpc"], json!("2.0"));
        assert_eq!(out["id"], json!(5));
        assert_eq!(out["error"]["code"], json!(-32602));
        assert!(
            out["error"]["message"].as_str().unwrap().contains("solana"),
            "error should name the offending network"
        );
    }

    pub(crate) struct Upstream {
        pub posts: Arc<AtomicUsize>,
        pub payloads: Arc<std::sync::Mutex<Vec<Value>>>,
        /// The most requests the stub ever held at once.
        pub peak: Arc<AtomicUsize>,
    }

    fn answer(req: &Value) -> Value {
        let method = req["method"].as_str().unwrap_or("");
        let result = match method {
            "eth_blockNumber" => json!("0x10"),
            "net_version" => json!("137"),
            "eth_getBalance" => json!(format!("0xbal{}", req["params"][0].as_str().unwrap_or(""))),
            _ => json!(null),
        };
        json!({ "jsonrpc": "2.0", "id": req["id"], "result": result })
    }

    pub(crate) async fn mock_upstream(batches: bool) -> (String, Upstream) {
        mock_upstream_with_reply(if batches {
            BatchReply::Complete
        } else {
            BatchReply::Unsupported
        })
        .await
    }

    #[derive(Clone, Copy)]
    enum BatchReply {
        Complete,
        Unsupported,
        Body(axum::http::StatusCode, &'static str),
        /// Batches as `Unsupported`; single requests are never answered.
        Hang,
    }

    async fn mock_upstream_with_reply(batch_reply: BatchReply) -> (String, Upstream) {
        use axum::response::IntoResponse;
        use axum::{Json, Router, routing::post};
        let posts = Arc::new(AtomicUsize::new(0));
        let payloads = Arc::new(std::sync::Mutex::new(Vec::new()));
        let in_flight = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let (p, l, f, k) = (
            posts.clone(),
            payloads.clone(),
            in_flight.clone(),
            peak.clone(),
        );
        let app = Router::new().route(
            "/",
            post(move |Json(payload): Json<Value>| {
                let (p, l, f, k) = (p.clone(), l.clone(), f.clone(), k.clone());
                async move {
                    p.fetch_add(1, Ordering::SeqCst);
                    l.lock().unwrap().push(payload.clone());
                    k.fetch_max(f.fetch_add(1, Ordering::SeqCst) + 1, Ordering::SeqCst);
                    if payload.is_object() {
                        // Hold single requests briefly so overlapping ones are seen.
                        let hold = match batch_reply {
                            BatchReply::Hang => Duration::from_secs(3600),
                            _ => Duration::from_millis(10),
                        };
                        tokio::time::sleep(hold).await;
                    }
                    f.fetch_sub(1, Ordering::SeqCst);
                    let reply = match &payload {
                        Value::Array(items) => match batch_reply {
                            BatchReply::Complete => {
                                Value::Array(items.iter().map(answer).collect())
                            }
                            BatchReply::Unsupported | BatchReply::Hang => json!({
                                "jsonrpc": "2.0", "id": null,
                                "error": { "code": -32600, "message": "batch not supported" }
                            }),
                            BatchReply::Body(status, body) => {
                                return (status, body).into_response();
                            }
                        },
                        single => answer(single),
                    };
                    Json(reply).into_response()
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (
            format!("http://{addr}/"),
            Upstream {
                posts,
                payloads,
                peak,
            },
        )
    }

    fn batch_payload() -> Value {
        json!([
            { "jsonrpc": "2.0", "id": "a", "method": "eth_getBalance", "params": ["0x1", "latest"] },
            { "jsonrpc": "2.0", "id": 7, "method": "eth_sendRawTransaction", "params": ["0x"] },
            { "jsonrpc": "2.0", "id": "a", "method": "eth_getBalance", "params": ["0x2", "latest"] },
            { "jsonrpc": "2.0", "method": "net_version" },
        ])
    }

    fn check_batch(out: &Value) {
        let out = out.as_array().unwrap();
        assert_eq!(out.len(), 4);
        assert_eq!(out[0]["id"], json!("a"));
        assert_eq!(out[0]["result"], json!("0xbal0x1"));
        assert_eq!(out[1]["id"], json!(7));
        assert_eq!(out[1]["error"]["code"], json!(-32601));
        assert_eq!(out[2]["id"], json!("a"));
        assert_eq!(out[2]["result"], json!("0xbal0x2"));
        assert_eq!(out[3]["id"], json!(null));
        assert_eq!(out[3]["result"], json!("137"));
        for item in out {
            assert_eq!(item["jsonrpc"], json!("2.0"));
        }
    }

    #[tokio::test]
    async fn batch_is_one_upstream_post_with_ids_echoed() {
        let (url, upstream) = mock_upstream(true).await;
        let state = test_state(&[("polygon", &url)]);
        let out = handle_payload(&state, "polygon", batch_payload()).await;
        check_batch(&out);
        assert_eq!(upstream.posts.load(Ordering::SeqCst), 1);
        let sent = upstream.payloads.lock().unwrap();
        let wire = sent[0].as_array().unwrap();
        assert_eq!(
            wire.len(),
            3,
            "the locally rejected element never goes upstream"
        );
        assert_eq!(wire[0]["id"], json!(0));
        assert_eq!(wire[2]["id"], json!(2));
        assert_eq!(wire[2]["method"], json!("net_version"));
    }

    #[tokio::test]
    async fn batch_falls_back_per_element_when_upstream_rejects_batches() {
        let (url, upstream) = mock_upstream(false).await;
        let state = test_state(&[("polygon", &url)]);
        let out = handle_payload(&state, "polygon", batch_payload()).await;
        check_batch(&out);
        assert_eq!(upstream.posts.load(Ordering::SeqCst), 4);
        let sent = upstream.payloads.lock().unwrap();
        assert!(sent[1..].iter().all(|p| p.is_object()));
        assert_eq!(
            sent[1..].iter().filter(|p| p["id"] == json!("a")).count(),
            2
        );
    }

    fn balance_batch(len: usize) -> Value {
        Value::Array(
            (0..len)
                .map(|i| {
                    json!({ "id": i, "method": "eth_getBalance", "params": [format!("0x{i}"), "latest"] })
                })
                .collect(),
        )
    }

    #[tokio::test]
    async fn an_oversized_batch_is_refused_without_an_upstream_call() {
        let (url, upstream) = mock_upstream(false).await;
        let state = test_state(&[("polygon", &url)]);
        let out = handle_payload(&state, "polygon", balance_batch(MAX_BATCH + 1)).await;
        assert_eq!(out["jsonrpc"], json!("2.0"));
        assert_eq!(out["id"], json!(null));
        assert_eq!(out["error"]["code"], json!(-32600));
        assert_eq!(upstream.posts.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn the_per_element_fallback_is_bounded() {
        let (url, upstream) = mock_upstream(false).await;
        let state = test_state(&[("polygon", &url)]);
        let out = handle_payload(&state, "polygon", balance_batch(MAX_BATCH)).await;
        let out = out.as_array().unwrap();
        assert_eq!(out.len(), MAX_BATCH);
        for (i, item) in out.iter().enumerate() {
            assert_eq!(item["id"], json!(i));
            assert_eq!(item["result"], json!(format!("0xbal0x{i}")));
        }
        assert_eq!(upstream.posts.load(Ordering::SeqCst), MAX_BATCH + 1);
        let peak = upstream.peak.load(Ordering::SeqCst);
        assert!(
            (1..=FALLBACK_CONCURRENCY).contains(&peak),
            "{peak} requests in flight"
        );
    }

    #[tokio::test]
    async fn the_per_element_fallback_stops_at_its_budget() {
        let (url, upstream) = mock_upstream_with_reply(BatchReply::Hang).await;
        let state = test_state(&[("polygon", &url)]);
        let items: Vec<Forward> = (0..3)
            .map(|i| Forward {
                id: json!(i),
                req: json!({ "id": i, "method": "eth_getBalance", "params": ["0x1", "latest"] }),
                memo: None,
            })
            .collect();
        let started = std::time::Instant::now();
        let out = forward_batch(&state, &url, &items, Duration::from_millis(200)).await;
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_eq!(upstream.posts.load(Ordering::SeqCst), 4);
        for (i, item) in out.iter().enumerate() {
            assert_eq!(item["id"], json!(i));
            assert_eq!(item["error"]["code"], json!(-32603));
        }
    }

    #[tokio::test]
    async fn fallback_forwards_share_one_relay_wide_limit() {
        let (url, upstream) = mock_upstream(false).await;
        let state = test_state(&[("polygon", &url)]);
        let requests = FALLBACK_SLOTS / FALLBACK_CONCURRENCY + 2;
        let outs = futures::future::join_all(
            (0..requests).map(|_| handle_payload(&state, "polygon", balance_batch(MAX_BATCH))),
        )
        .await;
        for out in outs {
            assert_eq!(out.as_array().unwrap().len(), MAX_BATCH);
            assert!(out[0]["result"].is_string(), "{}", out[0]);
        }
        let peak = upstream.peak.load(Ordering::SeqCst);
        // The batch posts themselves are answered at once and may overlap too.
        assert!(
            peak <= FALLBACK_SLOTS + requests,
            "{peak} requests in flight"
        );
    }

    /// An answer over the cap fails every slot after the one batch call,
    /// with no per-element retry.
    #[tokio::test]
    async fn an_over_cap_batch_answer_is_not_retried_per_element() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let posts = Arc::new(AtomicUsize::new(0));
        let received = posts.clone();
        let server = tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buffer = [0; 4096];
                assert!(socket.read(&mut buffer).await.unwrap() > 0);
                received.fetch_add(1, Ordering::SeqCst);
                let head = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n",
                    MAX_UPSTREAM_BODY + 1
                );
                socket.write_all(head.as_bytes()).await.unwrap();
                socket.shutdown().await.unwrap();
            }
        });
        let state = test_state(&[("polygon", &url)]);
        let out = handle_payload(&state, "polygon", balance_batch(3)).await;
        server.abort();
        assert_eq!(posts.load(Ordering::SeqCst), 1);
        for (i, item) in out.as_array().unwrap().iter().enumerate() {
            assert_eq!(item["id"], json!(i));
            assert_eq!(item["error"]["code"], json!(-32603));
            let message = item["error"]["message"].as_str().unwrap();
            assert!(
                message.starts_with("Upstream response unreadable"),
                "{message}"
            );
        }
    }

    #[tokio::test]
    async fn malformed_or_incomplete_batches_retry_only_unanswered_slots_even_on_http_error() {
        for (reply, expected_posts) in [
            ("not JSON", 4),
            (
                r#"[{"id":0,"result":"0xbal0x1"},{"id":2,"result":"137"}]"#,
                2,
            ),
        ] {
            let (url, upstream) = mock_upstream_with_reply(BatchReply::Body(
                axum::http::StatusCode::BAD_GATEWAY,
                reply,
            ))
            .await;
            let state = test_state(&[("polygon", &url)]);
            let out = handle_payload(&state, "polygon", batch_payload()).await;
            check_batch(&out);
            assert_eq!(upstream.posts.load(Ordering::SeqCst), expected_posts);
            let sent = upstream.payloads.lock().unwrap();
            assert!(sent[1..].iter().all(Value::is_object));
            if expected_posts == 2 {
                assert_eq!(sent[1]["id"], json!("a"));
                assert_eq!(sent[1]["params"][0], json!("0x2"));
            }
        }
    }

    #[tokio::test]
    async fn transport_failure_returns_every_batch_slot_without_individual_retries_or_url() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        for allow_private in [false, true] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}/v3/SECRET", listener.local_addr().unwrap());
            let posts = Arc::new(AtomicUsize::new(0));
            let received = posts.clone();
            let server = tokio::spawn(async move {
                loop {
                    let (mut socket, _) = listener.accept().await.unwrap();
                    let mut buffer = [0; 4096];
                    assert!(socket.read(&mut buffer).await.unwrap() > 0);
                    received.fetch_add(1, Ordering::SeqCst);
                    socket.shutdown().await.unwrap();
                }
            });
            let mut settings = Settings::for_tests(&[("polygon", &url)]);
            settings.allow_private_upstream = allow_private;
            let state = State::new(
                settings,
                crate::http::Outbound::for_tests(),
                crate::ops::rpc::tests::lazy_pool(),
            );
            let out = handle_payload(&state, "polygon", batch_payload()).await;
            server.abort();
            assert_eq!(posts.load(Ordering::SeqCst), 1);
            for index in [0, 2, 3] {
                assert_eq!(
                    out[index],
                    json!({
                        "jsonrpc": "2.0",
                        "id": if index == 3 { Value::Null } else { json!("a") },
                        "error": {
                            "code": -32603,
                            "message": "Upstream request failed: error sending request"
                        }
                    })
                );
            }
            assert_eq!(out[1]["error"]["code"], json!(-32601));
        }
    }

    #[tokio::test]
    async fn malformed_single_response_preserves_error_text_without_the_upstream_url() {
        use axum::{Router, http::StatusCode, routing::post};

        for allow_private in [false, true] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}/v3/SECRET", listener.local_addr().unwrap());
            let app = Router::new().route(
                "/v3/SECRET",
                post(|| async { (StatusCode::BAD_GATEWAY, "not JSON") }),
            );
            let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
            let mut settings = Settings::for_tests(&[("polygon", &url)]);
            settings.allow_private_upstream = allow_private;
            let state = State::new(
                settings,
                crate::http::Outbound::for_tests(),
                crate::ops::rpc::tests::lazy_pool(),
            );
            let out = handle_single(
                &state,
                "polygon",
                json!({ "id": "single", "method": "eth_getBalance", "params": ["0x1", "latest"] }),
            )
            .await;
            server.abort();
            assert_eq!(
                out,
                json!({
                    "jsonrpc": "2.0",
                    "id": "single",
                    "error": {
                        "code": -32603,
                        "message": "Upstream returned invalid JSON: error decoding response body"
                    }
                })
            );
        }
    }

    #[tokio::test]
    async fn block_number_is_memoized_briefly() {
        let (url, upstream) = mock_upstream(true).await;
        let state = test_state(&[("polygon", &url)]);
        let req = json!({ "jsonrpc": "2.0", "id": 1, "method": "eth_blockNumber", "params": [] });
        let first = handle_single(&state, "polygon", req.clone()).await;
        assert_eq!(first["result"], json!("0x10"));
        let second = handle_single(
            &state,
            "POLYGON",
            json!({ "id": 2, "method": "eth_blockNumber" }),
        )
        .await;
        assert_eq!(second["id"], json!(2));
        assert_eq!(second["result"], json!("0x10"));
        assert_eq!(upstream.posts.load(Ordering::SeqCst), 1);
        let batch = handle_payload(&state, "polygon", json!([req])).await;
        assert_eq!(batch[0]["result"], json!("0x10"));
        assert_eq!(upstream.posts.load(Ordering::SeqCst), 1);
        handle_single(
            &state,
            "polygon",
            json!({ "id": 3, "method": "eth_getBalance", "params": ["0x1", "latest"] }),
        )
        .await;
        assert_eq!(upstream.posts.load(Ordering::SeqCst), 2);
    }

    /// The guarded path (no private opt-in): the test client admits the
    /// loopback stub, so the same relay runs through `Outbound::post`.
    #[tokio::test]
    async fn the_guarded_client_relays_too() {
        let (url, upstream) = mock_upstream(true).await;
        let mut settings = Settings::for_tests(&[("polygon", &url)]);
        settings.allow_private_upstream = false;
        let state = State::new(
            settings,
            crate::http::Outbound::for_tests(),
            crate::ops::rpc::tests::lazy_pool(),
        );
        let out = handle_single(
            &state,
            "polygon",
            json!({ "id": 1, "method": "net_version" }),
        )
        .await;
        assert_eq!(out["result"], json!("137"));
        assert_eq!(upstream.posts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn an_unreachable_upstream_is_an_internal_error_without_the_url() {
        let state = test_state(&[("polygon", "http://127.0.0.1:1/v3/SECRET")]);
        let out = handle_single(
            &state,
            "polygon",
            json!({ "id": 1, "method": "eth_getBalance", "params": ["0x1", "latest"] }),
        )
        .await;
        assert_eq!(out["error"]["code"], json!(-32603));
        let message = out["error"]["message"].as_str().unwrap();
        assert!(message.starts_with("Upstream request failed"), "{message}");
        assert!(!message.contains("SECRET"), "{message}");
    }
}
