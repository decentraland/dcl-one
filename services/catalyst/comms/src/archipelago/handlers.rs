use super::AppState;
use serde::Serialize;
/// The feed counters are appended, never reordered: the byte-exact shape assertions upstream
/// parity keeps are written against the first four fields.
#[derive(Serialize)]
struct HealthResp {
    healthy: bool,
    uptime_secs: i64,
    peers_total: usize,
    islands_total: usize,
    feed_connected: bool,
    feed_delivered: u64,
    feed_no_session_socket: u64,
    feed_deduplicated: u64,
    feed_undecodable: u64,
    feed_publish_dropped: u64,
    statistics_publish_dropped: u64,
    feed_assignment_coalesced: u64,
    feed_assignment_oversized: u64,
    socket_write_timeout: u64,
    socket_assignments_written: u64,
}

pub fn diagnostics(s: &AppState) -> String {
    let snapshot = s.statistics.snapshot();
    let uptime = s.started_at.elapsed().as_secs() as i64;
    crate::stats::render::json(&HealthResp {
        healthy: true,
        uptime_secs: uptime,
        peers_total: snapshot.peers.len(),
        islands_total: crate::stats::render::public_islands(&snapshot).len(),
        feed_connected: s.outbox.is_linked(),
        feed_delivered: s.feed.delivered_count(),
        feed_no_session_socket: s.feed.no_session_socket_count(),
        feed_deduplicated: s.feed.deduplicated_count(),
        feed_undecodable: s.feed.undecodable_count(),
        feed_publish_dropped: s.outbox.dropped(),
        statistics_publish_dropped: s.observations.dropped(),
        feed_assignment_coalesced: s.feed.coalesced_count(),
        feed_assignment_oversized: s.feed.assignment_oversized_count(),
        socket_write_timeout: s.feed.socket_write_timeout_count(),
        socket_assignments_written: s.feed.socket_assignments_written_count(),
    })
}

/// The connector host has its own status count, methods, CORS and fallback.
pub fn connector(state: AppState, primary: bool) -> axum::Router {
    connector_configured(
        state,
        primary,
        crate::stats::http::Settings::from_env(),
        std::env::var("CURRENT_VERSION").unwrap_or_default(),
        std::env::var("COMMIT_HASH").unwrap_or_default(),
    )
}

// Connector routes use byte segments, including empty segments, and terminate after
// 100 segments. Parameter spelling has no effect; a wildcard never visits children.
struct NativeMetricsPath(Vec<Vec<u8>>);
fn native_segments(mut path: &[u8]) -> impl Iterator<Item = &[u8]> {
    std::iter::from_fn(move || {
        if path.is_empty() {
            return None;
        }
        path = &path[1..];
        let end = path.iter().position(|&b| b == b'/').unwrap_or(path.len());
        let segment = &path[..end];
        path = &path[end..];
        Some(segment)
    })
    .take(100)
}
impl NativeMetricsPath {
    fn new(pattern: &str) -> Self {
        Self(
            native_segments(pattern.as_bytes())
                .map(|segment| {
                    if segment.first() == Some(&b':') {
                        b":".to_vec()
                    } else {
                        segment.to_vec()
                    }
                })
                .collect(),
        )
    }
    fn matches(&self, path: &str) -> bool {
        let mut values = native_segments(path.as_bytes());
        for (index, token) in self.0.iter().enumerate() {
            let Some(value) = values.next() else {
                return false;
            };
            match token.first() {
                Some(b'*') => return index + 1 == self.0.len(),
                Some(b':') if !value.is_empty() => {}
                _ if token == value => {}
                _ => return false,
            }
        }
        values.next().is_none()
    }
    fn replaces_status(&self) -> bool {
        matches!(self.0.as_slice(), [segment] if segment == b"status")
    }
}

fn connector_configured(
    state: AppState,
    primary: bool,
    settings: crate::stats::http::Settings,
    version: String,
    commit_hash: String,
) -> axum::Router {
    let http = std::sync::Arc::new(ConnectorHttp {
        hooks: crate::stats::runtime::Runtime::connector(state.clone()),
        metrics_path: NativeMetricsPath::new(&settings.metrics_path),
        state: state.clone(),
        settings,
        version,
        commit_hash,
    });
    let sockets = if primary {
        super::ws::connector_routes(http.clone()).with_state(state)
    } else {
        axum::Router::new()
    };
    sockets.fallback(move |method, uri, headers| {
        let http = http.clone();
        async move { http.respond(method, uri, headers).await }
    })
}

pub(super) struct ConnectorHttp {
    state: AppState,
    hooks: std::sync::Arc<crate::stats::runtime::Runtime>,
    metrics_path: NativeMetricsPath,
    settings: crate::stats::http::Settings,
    version: String,
    commit_hash: String,
}
impl ConnectorHttp {
    pub(super) async fn respond(
        &self,
        method: axum::http::Method,
        uri: axum::http::Uri,
        headers: axum::http::HeaderMap,
    ) -> axum::response::Response {
        use crate::stats::http::Hooks;
        use axum::{body::Body, response::Response};
        let Self {
            state,
            hooks,
            metrics_path,
            settings,
            version,
            commit_hash,
        } = self;
        let start = std::time::Instant::now();
        let path = uri.path();
        let method = method.as_str();
        let metric = method == "GET"
            && (path != "/status" || metrics_path.replaces_status())
            && metrics_path.matches(path);
        let (code, body, mime, cors, handler) = if metric {
            let authorized = settings.metrics_bearer_token.as_ref().is_none_or(|token| {
                let mut parts = headers
                    .get("authorization")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .split(' ');
                parts.next() == Some("Bearer")
                    && parts.next().is_some_and(|candidate| {
                        crate::stats::http::token_matches(token, candidate)
                    })
            });
            if !authorized {
                (401, String::new(), None, false, "")
            } else {
                match hooks.metrics().await {
                    Ok(body) => (
                        200,
                        body,
                        Some("text/plain; version=0.0.4; charset=utf-8"),
                        false,
                        "",
                    ),
                    Err(_) => (500, String::new(), None, false, ""),
                }
            }
        } else if path == "/status" && method == "GET" {
            #[derive(Serialize)]
            struct Status {
                version: String,
                #[serde(rename = "currentTime")]
                current_time: i64,
                #[serde(rename = "commitHash")]
                commit_hash: String,
                #[serde(rename = "userCount")]
                user_count: usize,
            }
            (
                200,
                crate::stats::render::json(&Status {
                    version: version.clone(),
                    current_time: chrono::Utc::now().timestamp_millis(),
                    commit_hash: commit_hash.clone(),
                    user_count: state.registry.authenticated_count(),
                }),
                Some("application/json"),
                true,
                "/status",
            )
        } else if path == "/health/live" {
            (200, "alive".into(), None, true, "/health/live")
        } else {
            (
                404,
                r#"{"error":"Not Found"}"#.into(),
                Some("application/json"),
                true,
                "",
            )
        };
        if !metric {
            hooks.observe_connector(crate::stats::http::Observation {
                method: method.into(),
                handler: handler.into(),
                code,
                request_size: 0.,
                duration: start.elapsed(),
            });
        }
        let mut response = Response::builder().status(code);
        if let Some(mime) = mime {
            response = response.header("content-type", mime);
        }
        if cors {
            response = response.header("access-control-allow-origin", "*");
        }
        if method == "HEAD" {
            response = response.header("content-length", body.len());
        }
        response
            .body(if method == "HEAD" {
                Body::empty()
            } else {
                Body::from(body)
            })
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use tower::ServiceExt;

    async fn app(pattern: &str) -> axum::Router {
        app_with_state(pattern).await.0
    }

    async fn app_with_state(pattern: &str) -> (axum::Router, super::AppState) {
        let state = super::super::test_support::state_over(
            super::super::Settings::for_tests(),
            std::sync::Arc::new(crate::gatekeeper::bus::DisabledBus),
        )
        .await;
        let app = axum::Router::new().nest(
            "/archipelago-ea-ws-connector",
            super::connector_configured(
                state.clone(),
                true,
                crate::stats::http::Settings {
                    metrics_path: pattern.into(),
                    metrics_bearer_token: Some("fixture".into()),
                    ..Default::default()
                },
                "version".into(),
                "commit".into(),
            ),
        );
        (app, state)
    }

    async fn request(
        app: axum::Router,
        method: &str,
        path: &str,
        authorized: bool,
    ) -> (u16, axum::http::HeaderMap, String) {
        request_with_headers(app, method, path, authorized, &[]).await
    }

    async fn request_with_headers(
        app: axum::Router,
        method: &str,
        path: &str,
        authorized: bool,
        headers: &[(&str, &str)],
    ) -> (u16, axum::http::HeaderMap, String) {
        let mut request = Request::builder()
            .method(method)
            .uri(format!("/archipelago-ea-ws-connector{path}"));
        if authorized {
            request = request.header("authorization", "Bearer fixture");
        }
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let response = app
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let code = response.status().as_u16();
        let headers = response.headers().clone();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (code, headers, String::from_utf8(bytes.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn connector_method_flood_preserves_http_without_creating_metric_series() {
        let app = app("/metrics").await;
        let paths = ["/ws", "/unknown", "/health/live", "/status", "/metrics"];
        for index in 0..64 {
            for path in paths {
                let (code, headers, body) =
                    request(app.clone(), &format!("FOO{index}"), path, false).await;
                assert_eq!(
                    code,
                    if path == "/health/live" { 200 } else { 404 },
                    "{path}"
                );
                assert_eq!(headers["access-control-allow-origin"], "*");
                assert_eq!(
                    body,
                    if path == "/health/live" {
                        "alive"
                    } else {
                        r#"{"error":"Not Found"}"#
                    }
                );
            }
        }
        let (code, _, body) = request(app.clone(), "GET", "/metrics", true).await;
        assert_eq!(code, 200);
        assert!(
            !body
                .lines()
                .any(|line| line.starts_with("http_requests_total{")),
            "{body}"
        );
        assert!(
            !body
                .lines()
                .any(|line| line.starts_with("http_request_duration_seconds_count{")),
            "{body}"
        );
        for method in ["PROPFIND", "CONNECT", "TRACE", "get", "pRoPfInD"] {
            for path in paths {
                let (code, _, _) = request(app.clone(), method, path, false).await;
                assert_eq!(
                    code,
                    if path == "/health/live" { 200 } else { 404 },
                    "{method} {path}"
                );
            }
        }
        let (_, _, body) = request(app, "GET", "/metrics", true).await;
        let methods = body
            .lines()
            .filter(|line| line.starts_with("http_requests_total{"))
            .map(|line| {
                line.split("method=\"")
                    .nth(1)
                    .unwrap()
                    .split('"')
                    .next()
                    .unwrap()
            })
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            methods,
            ["connect", "get", "propfind", "trace"]
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>(),
            "{body}"
        );
        assert!(!body.contains("foo"), "{body}");
    }

    #[tokio::test]
    async fn connector_metrics_http_selection_preserves_native_precedence_and_methods() {
        for (pattern, path, get_handler) in [
            ("/:tenant", "/status", "status"),
            ("/*", "/status", "status"),
            ("/status", "/status", "metrics"),
            ("xstatus", "/status", "metrics"),
            ("/health/live", "/health/live", "metrics"),
            ("/*", "/health/live", "metrics"),
            ("/scrape/:tenant?", "/scrape/alice", "metrics"),
            ("/scrape/*", "/scrape", "fallback"),
            ("/scrape/*", "/scrape/", "metrics"),
            ("/ws", "/ws", "metrics"),
            ("/:tenant", "/ws", "metrics"),
            ("/*", "/ws", "metrics"),
            ("/metrics", "/ws", "fallback"),
            ("/:tenant", "/health/live", "health"),
            ("/ws", "/health/live", "health"),
        ] {
            let app = app(pattern).await;
            for method in ["GET", "HEAD", "POST", "OPTIONS"] {
                for authorized in [false, true] {
                    let handler = if method == "GET" {
                        get_handler
                    } else if path == "/health/live" {
                        "health"
                    } else {
                        "fallback"
                    };
                    let (code, headers, body) =
                        request(app.clone(), method, path, authorized).await;
                    let expected = match handler {
                        "metrics" if !authorized => 401,
                        "fallback" => 404,
                        _ => 200,
                    };
                    assert_eq!(
                        code, expected,
                        "{pattern} {method} {path} auth={authorized}"
                    );
                    assert_eq!(
                        headers.contains_key("access-control-allow-origin"),
                        handler != "metrics"
                    );
                    if handler == "metrics" {
                        if authorized {
                            assert_eq!(
                                headers["content-type"],
                                "text/plain; version=0.0.4; charset=utf-8"
                            );
                            assert!(body.contains("# TYPE http_requests_total counter"));
                        } else {
                            assert!(body.is_empty());
                            assert!(!headers.contains_key("content-type"));
                        }
                    } else if method == "HEAD" {
                        assert!(body.is_empty());
                        assert_eq!(
                            headers["content-length"],
                            if handler == "health" { "5" } else { "21" }
                        );
                    } else if handler == "status" {
                        assert_eq!(headers["content-type"], "application/json");
                        assert!(body.contains("\"userCount\":0"));
                    } else if handler == "health" {
                        assert_eq!(body, "alive");
                        assert!(!headers.contains_key("content-type"));
                    } else {
                        assert_eq!(headers["content-type"], "application/json");
                        assert_eq!(body, r#"{"error":"Not Found"}"#);
                    }
                }
            }
        }
    }

    #[tokio::test]
    async fn connector_metrics_observes_selected_handlers_and_skips_rejected_scrapes() {
        let app = app("/:tenant").await;
        assert_eq!(request(app.clone(), "GET", "/status", false).await.0, 200);
        assert_eq!(request(app.clone(), "GET", "/scrape", false).await.0, 401);
        assert_eq!(request(app.clone(), "HEAD", "/scrape", true).await.0, 404);
        assert_eq!(request(app.clone(), "POST", "/scrape", true).await.0, 404);
        assert_eq!(request(app.clone(), "GET", "/ws", false).await.0, 401);
        let (_, _, body) = request(app.clone(), "GET", "/scrape", true).await;
        let mut counters: Vec<_> = body
            .lines()
            .filter(|line| line.starts_with("http_requests_total{"))
            .collect();
        assert_eq!(counters.len(), 3, "{body}");
        for (handler, method, code) in [
            ("/status", "get", 200),
            ("", "head", 404),
            ("", "post", 404),
        ] {
            assert!(
                counters
                    .iter()
                    .any(|line| line.contains(&format!("handler=\"{handler}\""))
                        && line.contains(&format!("method=\"{method}\""))
                        && line.contains(&format!("code=\"{code}\""))
                        && line.ends_with(" 1")),
                "{body}"
            );
        }
        let next = request(app.clone(), "GET", "/scrape", true).await.2;
        let mut next: Vec<_> = next
            .lines()
            .filter(|line| line.starts_with("http_requests_total{"))
            .collect();
        counters.sort_unstable();
        next.sort_unstable();
        assert_eq!(counters, next);
        assert_eq!(
            request(app.clone(), "GET", "/health/live", false).await.0,
            200
        );
        let body = request(app, "GET", "/scrape", true).await.2;
        let health: Vec<_> = body
            .lines()
            .filter(|line| {
                line.starts_with("http_requests_total{")
                    && line.contains("handler=\"/health/live\"")
            })
            .collect();
        assert_eq!(health.len(), 1, "{body}");
        assert!(
            health[0].contains("method=\"get\"")
                && health[0].contains("code=\"200\"")
                && health[0].ends_with(" 1"),
            "{body}"
        );
    }

    fn counters(body: &str) -> std::collections::BTreeMap<(String, String, u16), u64> {
        body.lines()
            .filter(|line| line.starts_with("http_requests_total{"))
            .map(|line| {
                let (labels, value) = line.rsplit_once(' ').unwrap();
                let labels: std::collections::BTreeMap<_, _> = labels
                    .strip_prefix("http_requests_total{")
                    .unwrap()
                    .strip_suffix('}')
                    .unwrap()
                    .split(',')
                    .map(|label| {
                        let (name, value) = label.split_once('=').unwrap();
                        (name, value.trim_matches('"'))
                    })
                    .collect();
                (
                    (
                        labels["method"].into(),
                        labels["handler"].into(),
                        labels["code"].parse().unwrap(),
                    ),
                    value.parse().unwrap(),
                )
            })
            .collect()
    }

    #[tokio::test]
    async fn connector_metrics_socket_yield_uses_http_selection_and_one_observation() {
        for pattern in ["/ws", "/:tenant", "/*", "/metrics"] {
            let app = app(pattern).await;
            let scrape = if pattern == "/metrics" {
                "/metrics"
            } else {
                "/ws"
            };
            let mut previous = counters(&request(app.clone(), "GET", scrape, true).await.2);
            for path in ["/ws", "/ws?x=1"] {
                for method in ["GET", "HEAD", "POST", "OPTIONS"] {
                    for key in [
                        None,
                        Some("12345678901234567890123"),
                        Some("1234567890123456789012345"),
                        Some("dGhlIHNhbXBsZSBub25jZQ=="),
                    ] {
                        if method == "GET" && key.is_some_and(|key| key.len() == 24) {
                            continue;
                        }
                        for upgrade in [false, true] {
                            for bearer in [None, Some("Bearer wrong"), Some("Bearer fixture")] {
                                let mut pairs = Vec::new();
                                if let Some(key) = key {
                                    pairs.push(("sec-websocket-key", key));
                                }
                                if upgrade {
                                    pairs.extend([
                                        ("upgrade", "websocket"),
                                        ("connection", "Upgrade"),
                                        ("sec-websocket-version", "13"),
                                    ]);
                                }
                                if let Some(bearer) = bearer {
                                    pairs.push(("authorization", bearer));
                                }
                                let metric = method == "GET" && pattern != "/metrics";
                                let authorized = bearer == Some("Bearer fixture");
                                let expected = if metric {
                                    if authorized { 200 } else { 401 }
                                } else {
                                    404
                                };
                                let (code, headers, body) =
                                    request_with_headers(app.clone(), method, path, false, &pairs)
                                        .await;
                                assert_eq!(code, expected, "{pattern} {method} {path} {pairs:?}");
                                assert!(!headers.contains_key("www-authenticate"));
                                assert_eq!(
                                    headers.contains_key("access-control-allow-origin"),
                                    !metric
                                );
                                if metric && authorized {
                                    assert_eq!(
                                        headers["content-type"],
                                        "text/plain; version=0.0.4; charset=utf-8"
                                    );
                                    assert!(body.contains("# TYPE http_requests_total counter"));
                                } else if metric {
                                    assert!(body.is_empty());
                                    assert!(!headers.contains_key("content-type"));
                                } else {
                                    assert_eq!(headers["content-type"], "application/json");
                                    assert_eq!(headers["access-control-allow-origin"], "*");
                                    if method == "HEAD" {
                                        assert!(body.is_empty());
                                        assert_eq!(headers["content-length"], "21");
                                    } else {
                                        assert_eq!(body, r#"{"error":"Not Found"}"#);
                                    }
                                }
                                let current =
                                    counters(&request(app.clone(), "GET", scrape, true).await.2);
                                if metric {
                                    assert_eq!(
                                        current, previous,
                                        "metrics must not observe {pairs:?}"
                                    );
                                } else {
                                    let mut expected = previous.clone();
                                    let label = (method.to_ascii_lowercase(), String::new(), 404);
                                    *expected.entry(label).or_default() += 1;
                                    assert_eq!(
                                        current, expected,
                                        "one fallback observation: {pairs:?}"
                                    );
                                }
                                previous = current;
                            }
                        }
                    }
                }
            }
        }
    }

    #[tokio::test]
    async fn connector_metrics_socket_claim_keeps_non_key_header_rejections() {
        let key = ("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==");
        let connection = ("connection", "Upgrade");
        let upgrade = ("upgrade", "websocket");
        let version = ("sec-websocket-version", "13");
        for pattern in ["/ws", "/:tenant", "/*", "/metrics"] {
            let app = app(pattern).await;
            let scrape = if pattern == "/metrics" {
                "/metrics"
            } else {
                "/ws"
            };
            let before = counters(&request(app.clone(), "GET", scrape, true).await.2);
            for (pairs, message) in [
                (
                    vec![key, upgrade, version],
                    "Connection header did not include 'upgrade'",
                ),
                (
                    vec![key, ("connection", "close"), upgrade, version],
                    "Connection header did not include 'upgrade'",
                ),
                (
                    vec![key, connection, version],
                    "`Upgrade` header did not include 'websocket'",
                ),
                (
                    vec![key, connection, ("upgrade", "other"), version],
                    "`Upgrade` header did not include 'websocket'",
                ),
                (
                    vec![key, connection, upgrade],
                    "`Sec-WebSocket-Version` header did not include '13'",
                ),
                (
                    vec![key, connection, upgrade, ("sec-websocket-version", "12")],
                    "`Sec-WebSocket-Version` header did not include '13'",
                ),
            ] {
                for authorized in [false, true] {
                    let (code, headers, body) =
                        request_with_headers(app.clone(), "GET", "/ws?x=1", authorized, &pairs)
                            .await;
                    assert_eq!(code, 400, "{pattern} {pairs:?}");
                    assert_eq!(body, message);
                    assert_eq!(headers["content-type"], "text/plain; charset=utf-8");
                    assert!(!headers.contains_key("access-control-allow-origin"));
                }
            }
            assert_eq!(counters(&request(app, "GET", scrape, true).await.2), before);
        }
    }

    #[tokio::test]
    async fn connector_metrics_socket_upgrade_bypasses_colliding_scrapes_and_challenges() {
        use crate::protocol::archipelago::archipelago::{
            ChallengeRequestMessage, ClientPacket, ServerPacket, client_packet, server_packet,
        };
        use futures::{SinkExt, StreamExt};
        use prost::Message as _;
        use std::time::Duration;
        use tokio_tungstenite::tungstenite::Message;
        for pattern in ["/ws", "/:tenant", "/*"] {
            let (app, state) = app_with_state(pattern).await;
            let available = state.sockets.available_permits();
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let stop = tokio_util::sync::CancellationToken::new();
            let server = tokio::spawn(
                axum::serve(listener, app.clone())
                    .with_graceful_shutdown(stop.clone().cancelled_owned())
                    .into_future(),
            );
            let (mut socket, response) = tokio::time::timeout(
                Duration::from_secs(3),
                tokio_tungstenite::connect_async(format!(
                    "ws://{address}/archipelago-ea-ws-connector/ws?x=1"
                )),
            )
            .await
            .unwrap()
            .unwrap();
            assert_eq!(response.status().as_u16(), 101);
            socket
                .send(Message::Binary(
                    ClientPacket {
                        message: Some(client_packet::Message::ChallengeRequest(
                            ChallengeRequestMessage {
                                address: "0x0000000000000000000000000000000000000001".into(),
                            },
                        )),
                        request_sequence: 0,
                    }
                    .encode_to_vec()
                    .into(),
                ))
                .await
                .unwrap();
            let message = tokio::time::timeout(Duration::from_secs(3), socket.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            let Message::Binary(bytes) = message else {
                panic!("challenge packet expected")
            };
            let Some(server_packet::Message::ChallengeResponse(response)) =
                ServerPacket::decode(bytes).unwrap().message
            else {
                panic!("challenge expected")
            };
            assert_eq!(response.challenge_to_sign.len(), 68);
            assert!(response.challenge_to_sign.starts_with("dcl-"));
            socket.close(None).await.unwrap();
            drop(socket);
            tokio::time::timeout(Duration::from_secs(3), async {
                while state.sockets.available_permits() != available {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            stop.cancel();
            tokio::time::timeout(Duration::from_secs(3), server)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            let body = request(app, "GET", "/ws", true).await.2;
            let values = counters(&body);
            assert_eq!(values.len(), 1, "{body}");
            assert_eq!(
                values.get(&("get".into(), "/ws".into(), 101)),
                Some(&1),
                "{body}"
            );
        }
    }

    #[test]
    fn connector_metrics_native_segments_preserve_separators_and_bytes() {
        for (pattern, path, expected) in [
            ("/scrape/*", "/scrape", false),
            ("/scrape/*", "/scrape/", true),
            ("/scrape/*", "/scrape/a/b", true),
            ("/scrape/*suffix", "/scrape/a", true),
            ("/scrape/*/tail", "/scrape/a/tail", false),
            ("/scrape/:tenant?", "/scrape/a", true),
            ("/scrape/:tenant?", "/scrape/", false),
            ("/scrape/:", "/scrape/a", true),
            ("/scrape/:", "/scrape/", false),
            ("/scrape/:tenant(\\d+)", "/scrape/abc", true),
            ("/scrape/:tenant-name", "/scrape/a", true),
            ("/scrape/\\:literal", "/scrape/:literal", false),
            ("/scrape/\\:literal", "/scrape/\\:literal", true),
            ("/scrape/(.*)", "/scrape/a", false),
            ("/scrape/(.*)", "/scrape/(.*)", true),
            ("/metrics//", "/metrics/", false),
            ("/metrics//", "/metrics//", true),
            ("/metrics//", "/metrics///", false),
            ("/metrics", "/METRICS", false),
            ("/é", "/É", false),
            ("/é", "/%C3%A9", false),
            ("metrics", "/etrics", true),
            ("é", "é", true),
            ("", "", true),
            ("", "/", false),
            ("/", "/", true),
            ("/", "", false),
        ] {
            assert_eq!(
                super::NativeMetricsPath::new(pattern).matches(path),
                expected,
                "{pattern:?} {path:?}"
            );
        }
    }

    #[test]
    fn connector_metrics_native_segment_limit_terminates_registration_and_traversal() {
        let path = |count| "/a".repeat(count);
        for (pattern_count, path_count, expected) in [
            (99, 99, true),
            (99, 100, false),
            (99, 101, false),
            (100, 99, false),
            (100, 100, true),
            (100, 101, true),
            (101, 99, false),
            (101, 100, true),
            (101, 101, true),
        ] {
            assert_eq!(
                super::NativeMetricsPath::new(&path(pattern_count)).matches(&path(path_count)),
                expected,
                "{pattern_count} {path_count}"
            );
        }
    }
}
