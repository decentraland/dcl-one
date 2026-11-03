//! Legacy uWS GET scrape contract, reading the one existing process recorder.
//! This family filter does not establish label/instrumentation or reset parity.
use super::State as RpcState;
use axum::{
    body::Body,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;

// The pinned source registers all metadata, but prom-client only creates
// unlabelled counters and gauges before their first observation.
const DECLARATIONS: &[(&str, &str, &str)] = &[
    (
        "http_request_duration_seconds",
        "histogram",
        "Request duration in seconds.",
    ),
    (
        "http_requests_total",
        "counter",
        "Total number of HTTP requests",
    ),
    (
        "http_request_size_bytes",
        "histogram",
        "Duration of HTTP requests size in bytes",
    ),
    (
        "wkc_logger_logs_total",
        "counter",
        "Log amounts per logger and level",
    ),
    (
        "ws_active_connections_by_protocol",
        "gauge",
        "Active WebSocket connections by protocol",
    ),
    (
        "ws_connection_duration_seconds_by_protocol",
        "histogram",
        "WebSocket connection duration by protocol",
    ),
    (
        "ws_auth_errors_by_protocol",
        "counter",
        "Authentication failures by protocol and cause",
    ),
    (
        "ws_admission_evictions",
        "counter",
        "Oldest WebSocket connections closed to keep an address within its connection cap",
    ),
    (
        "ws_admission_rejections",
        "counter",
        "Subscription starts rejected by the per-address rate limit",
    ),
    (
        "ws_active_connections",
        "gauge",
        "Number of WebSocket active connections",
    ),
    (
        "ws_connections_rejected",
        "counter",
        "Number of WebSocket connections rejected because the pool reached WS_MAX_CONCURRENT_CONNECTIONS",
    ),
    (
        "ws_messages_received",
        "counter",
        "Number of WebSocket messages received",
    ),
    (
        "ws_messages_sent",
        "counter",
        "Number of outbound WebSocket messages accepted by the socket (sent or buffered)",
    ),
    ("ws_errors", "counter", "Number of WebSocket errors"),
    (
        "ws_close_codes",
        "counter",
        "Number of WebSocket close codes",
    ),
    (
        "ws_transport_errors",
        "counter",
        "Number of WebSocket transport errors",
    ),
    (
        "ws_auth_errors",
        "counter",
        "Number of WebSocket authentication errors",
    ),
    (
        "ws_auth_race_condition_aborted",
        "counter",
        "Number of authentication attempts aborted due to connection closed during auth",
    ),
    (
        "ws_idle_timeouts",
        "counter",
        "Number of WebSocket idle timeouts",
    ),
    (
        "ws_connection_duration_seconds",
        "histogram",
        "Duration of WebSocket connections in seconds",
    ),
    (
        "rpc_procedure_call_total",
        "counter",
        "Total number of RPC procedure calls",
    ),
    (
        "rpc_procedure_call_duration_seconds",
        "histogram",
        "Duration of RPC procedure calls in seconds",
    ),
    (
        "rpc_in_procedure_call_size_bytes",
        "histogram",
        "Size of incoming RPC procedure call payloads in bytes",
    ),
    (
        "rpc_out_procedure_call_size_bytes",
        "histogram",
        "Size of outgoing RPC procedure call payloads in bytes",
    ),
    (
        "rpc_updates_sent_on_subscription",
        "counter",
        "Number of updates sent on RPC subscription streams",
    ),
    (
        "stream_ws_messages_received",
        "counter",
        "Number of messages received on the plain WebSocket stream protocol",
    ),
    (
        "stream_ws_updates_sent",
        "counter",
        "Number of updates sent on plain WebSocket stream subscriptions",
    ),
    (
        "stream_ws_errors",
        "counter",
        "Number of errors reported on the plain WebSocket stream protocol",
    ),
    (
        "stream_ws_authentications",
        "counter",
        "Successful plain WebSocket stream authentications by handshake method",
    ),
    (
        "stream_ws_close_codes",
        "counter",
        "Number of plain WebSocket stream protocol close codes",
    ),
    (
        "ws_unexpected_send_result_events",
        "counter",
        "Number of unexpected send result events",
    ),
    (
        "ws_backpressure_events",
        "counter",
        "Number of WebSocket messages that encountered backpressure or were dropped",
    ),
    (
        "ws_drain_events",
        "counter",
        "Number of WebSocket drain events",
    ),
    (
        "ws_message_size_bytes",
        "histogram",
        "Size of WebSocket messages in bytes",
    ),
    (
        "ws_queue_vs_backpressure_ratio",
        "gauge",
        "Ratio of message queue size to uWebSocket buffered amount",
    ),
    (
        "subscribers_local_count",
        "gauge",
        "Number of local in-memory subscriber emitters",
    ),
    (
        "subscribers_generators_count",
        "gauge",
        "Number of active async generators across all subscribers",
    ),
    (
        "subscribers_stale_cleaned",
        "counter",
        "Number of stale subscribers removed by reconciliation",
    ),
    (
        "subscribers_stale_connections_cleaned",
        "counter",
        "Number of stale connections (socket gone, address still active) removed by reconciliation",
    ),
    (
        "subscription_duplicates_total",
        "counter",
        "Number of duplicate stream subscriptions rejected by the per-(address, event) guard",
    ),
    (
        "subscription_updates_dropped_total",
        "counter",
        "Number of subscription updates dropped because a connection's value queue overflowed (slow consumer)",
    ),
    (
        "friendship_rate_limiter_unavailable",
        "counter",
        "Number of friendship/block mutations allowed because the Redis rate limiter was unreachable",
    ),
    (
        "ai_compliance_validation_duration_seconds",
        "histogram",
        "Duration of AI compliance validation in seconds",
    ),
    (
        "ai_compliance_validation_total",
        "counter",
        "Total number of AI compliance validations (compliant, non-compliant, failed)",
    ),
];

pub(super) fn register() {
    metrics::gauge!("ws_active_connections").increment(0.0);
    metrics::counter!("ws_connections_rejected").increment(0);
    metrics::counter!("ws_messages_received").increment(0);
    metrics::counter!("ws_messages_sent").increment(0);
    metrics::counter!("ws_errors").increment(0);
    metrics::counter!("ws_transport_errors").increment(0);
    metrics::counter!("ws_auth_race_condition_aborted").increment(0);
    metrics::counter!("ws_idle_timeouts").increment(0);
    metrics::counter!("ws_unexpected_send_result_events").increment(0);
    metrics::counter!("ws_drain_events").increment(0);
    metrics::gauge!("ws_queue_vs_backpressure_ratio").increment(0.0);
    metrics::gauge!("subscribers_local_count").increment(0.0);
    metrics::gauge!("subscribers_generators_count").increment(0.0);
    metrics::counter!("subscribers_stale_cleaned").increment(0);
    metrics::counter!("subscribers_stale_connections_cleaned").increment(0);
    metrics::counter!("friendship_rate_limiter_unavailable").increment(0);
}

pub(super) fn http_finished(
    method: String,
    handler: &'static str,
    code: u16,
    started: std::time::Instant,
) {
    let code = code.to_string();
    metrics::counter!("http_requests_total", "method" => method.clone(), "handler" => handler, "code" => code.clone()).increment(1);
    metrics::histogram!("http_request_duration_seconds", "method" => method, "handler" => handler, "code" => code).record(started.elapsed().as_secs_f64());
}

pub(super) fn log(logger: &'static str, level: &'static str) {
    metrics::counter!("wkc_logger_logs_total", "logger" => logger, "level" => match level { "debug" => "DEBUG", "info" => "INFO", "warn" => "WARNING", "error" => "ERROR", value => value }).increment(1);
}

fn procedure_label(name: &str) -> String {
    let mut chars = name.chars();
    chars
        .next()
        .map(|first| first.to_lowercase().collect::<String>() + chars.as_str())
        .unwrap_or_default()
}

// JSON.stringify formats integral doubles without a decimal suffix and uses
// decimal notation through 1e-6; serde's float formatting is intentionally different.
pub(crate) fn json_bytes(value: &serde_json::Value) -> usize {
    use serde_json::Value;
    match value {
        Value::Null => 4,
        Value::Bool(value) => {
            if *value {
                4
            } else {
                5
            }
        }
        Value::String(value) => serde_json::to_string(value).unwrap().len(),
        Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                return value.to_string().len();
            }
            if let Some(value) = value.as_u64() {
                return value.to_string().len();
            }
            let value = value.as_f64().unwrap();
            if value == 0.0 {
                1
            } else if value.abs() >= 1e21 || value.abs() < 1e-6 {
                let text = format!("{value:e}");
                text.len() + usize::from(!text.split_once('e').unwrap().1.starts_with('-'))
            } else {
                value.to_string().len()
            }
        }
        Value::Array(values) => {
            2 + values.iter().map(json_bytes).sum::<usize>() + values.len().saturating_sub(1)
        }
        Value::Object(values) => {
            2 + values
                .iter()
                .map(|(key, value)| {
                    serde_json::to_string(key).unwrap().len() + 1 + json_bytes(value)
                })
                .sum::<usize>()
                + values.len().saturating_sub(1)
        }
    }
}

pub(super) fn rpc_request(name: &str, request: &serde_json::Value) -> std::time::Instant {
    let started = std::time::Instant::now();
    metrics::histogram!("rpc_in_procedure_call_size_bytes", "procedure" => procedure_label(name))
        .record(json_bytes(request) as f64);
    started
}

pub(super) fn rpc_finished(name: &str, code: &'static str, started: std::time::Instant) {
    let procedure = procedure_label(name);
    metrics::counter!("rpc_procedure_call_total", "code" => code, "procedure" => procedure.clone())
        .increment(1);
    metrics::histogram!("rpc_procedure_call_duration_seconds", "code" => code, "procedure" => procedure).record(started.elapsed().as_secs_f64());
}

fn response_code(response: &serde_json::Value) -> &'static str {
    if let Some(case) = response["response"]["$case"].as_str() {
        return match case {
            "ok" | "accepted" | "requests" => "OK",
            "notFound" => "NOT_FOUND",
            "internalServerError" => "INTERNAL_SERVER_ERROR",
            "invalidRequest" => "INVALID_REQUEST",
            "profileNotFound" => "PROFILE_NOT_FOUND",
            "forbiddenError" => "FORBIDDEN_ERROR",
            "conflictingError" => "CONFLICTING_ERROR",
            "invalidFriendshipAction" => "INVALID_FRIENDSHIP_ACTION",
            _ => "UNKNOWN",
        };
    }
    if response.get("paginationData").is_some()
        || (response.get("blockedUsers").is_some() && response.get("blockedByUsers").is_some())
    {
        "OK"
    } else {
        "UNKNOWN"
    }
}

pub(super) fn rpc_response(name: &str, response: &serde_json::Value, started: std::time::Instant) {
    let code = response_code(response);
    metrics::histogram!("rpc_out_procedure_call_size_bytes", "code" => code, "procedure" => procedure_label(name)).record(json_bytes(response) as f64);
    rpc_finished(name, code, started);
}

pub(super) struct RpcStream {
    name: &'static str,
    started: std::time::Instant,
    failed: bool,
}
impl RpcStream {
    pub(super) fn new(name: &'static str) -> Self {
        Self {
            name,
            started: rpc_request(name, &serde_json::json!({})),
            failed: false,
        }
    }
    pub(super) fn failed(&mut self) {
        self.failed = true;
    }
    pub(super) fn update(&self, event: &'static str) {
        metrics::counter!("rpc_updates_sent_on_subscription", "event" => event).increment(1);
    }
}
impl Drop for RpcStream {
    fn drop(&mut self) {
        rpc_finished(
            self.name,
            if self.failed { "STREAM_ERROR" } else { "OK" },
            self.started,
        );
    }
}

pub(super) async fn serve_if_requested(
    State(state): State<Arc<RpcState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let configured = state
        .settings
        .metrics_path
        .as_deref()
        .filter(|path| !path.is_empty())
        .unwrap_or("/metrics");
    let path = format!("/social-rpc{configured}");
    // /status was registered before metrics on the source uWS app.
    if request.method() != axum::http::Method::GET
        || request.uri().path() != path
        || configured == "/status"
    {
        let uri = request.uri().path();
        if !uri.starts_with("/social-rpc") {
            return next.run(request).await;
        }
        if matches!(uri, "/social-rpc/info" | "/social-rpc/health") {
            return next.run(request).await;
        }
        let get = request.method() == axum::http::Method::GET;
        let upgrade = request.headers().contains_key(header::SEC_WEBSOCKET_KEY);
        let handler = if get && uri == "/social-rpc/status" {
            "/status"
        } else if uri == "/social-rpc/health/live" {
            "/health/live"
        } else if get
            && uri.starts_with("/social-rpc/v1/users/")
            && uri.ends_with("/privacy-settings")
        {
            "/v1/users/:address/privacy-settings"
        } else if upgrade && uri == "/social-rpc" {
            "/ws"
        } else if upgrade && uri == "/social-rpc/v1/stream" {
            "/v1/stream"
        } else {
            ""
        };
        let started = std::time::Instant::now();
        let method = request.method().as_str().to_ascii_lowercase();
        let response = next.run(request).await;
        http_finished(method, handler, response.status().as_u16(), started);
        return response;
    }
    if let Some(expected) = state
        .settings
        .metrics_bearer_token
        .as_deref()
        .filter(|token| !token.is_empty())
    {
        let mut parts = request
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|header| header.to_str().ok())
            .unwrap_or("")
            .split(' ');
        let scheme = parts.next();
        let presented = parts.next();
        if scheme != Some("Bearer")
            || !presented.is_some_and(|presented| {
                crate::auth::admin_bearer::timing_safe_eq(presented.as_bytes(), expected.as_bytes())
            })
        {
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }
    // Authorization precedes rendering. Dropping the request future needs no
    // write into a disconnected uWS response object in this HTTP transport.
    let rendered = state
        .settings
        .metrics_handle
        .as_ref()
        .map(|handle| handle.render())
        .unwrap_or_default();
    (
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        filter_families(&rendered),
    )
        .into_response()
}

pub(super) fn filter_families(rendered: &str) -> String {
    let mut out = String::new();
    for (family, kind, help) in DECLARATIONS {
        out.push_str(&format!("# HELP {family} {help}\n# TYPE {family} {kind}\n"));
        for line in rendered.lines().filter(|line| !line.starts_with('#')) {
            let name = line.split(['{', ' ']).next().unwrap_or("");
            if name == *family
                || ["_sum", "_count", "_bucket"]
                    .iter()
                    .any(|suffix| name.strip_suffix(suffix) == Some(*family))
            {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_registration_has_exact_metadata_and_only_sixteen_initial_zeros() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(env!("DCL_ONE_ROOT"), "/services/catalyst/tests/fixtures/conformance/social/application-metrics-source-cases.json"))).unwrap();
        let recorder = crate::comms::metrics::prometheus_builder()
            .unwrap()
            .build_recorder();
        let handle = recorder.handle();
        metrics::with_local_recorder(&recorder, register);
        let rendered = filter_families(&handle.render());
        let metadata = |body: &str| {
            body.lines()
                .filter(|line| line.starts_with('#'))
                .map(str::to_owned)
                .collect::<std::collections::BTreeSet<_>>()
        };
        let samples = |body: &str| {
            body.lines()
                .filter(|line| !line.starts_with('#') && !line.is_empty())
                .map(str::to_owned)
                .collect::<std::collections::BTreeSet<_>>()
        };
        assert_eq!(
            metadata(&rendered),
            metadata(fixture["initial"].as_str().unwrap())
        );
        assert_eq!(
            samples(&rendered),
            samples(fixture["initial"].as_str().unwrap())
        );
        assert_eq!(samples(&rendered).len(), 16);
        metrics::with_local_recorder(&recorder, || {
            metrics::counter!("shop_orders_total").increment(99)
        });
        assert!(!filter_families(&handle.render()).contains("shop_orders_total"));
    }
    #[tokio::test]
    async fn status_privacy_health_and_fallback_emit_source_http_labels_and_scraping_is_unmeasured()
    {
        use tower::ServiceExt;
        let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(env!("DCL_ONE_ROOT"), "/services/catalyst/tests/fixtures/conformance/social/application-metrics-source-cases.json"))).unwrap();
        let recorder = crate::comms::metrics::prometheus_builder()
            .unwrap()
            .build_recorder();
        let handle = recorder.handle();
        let _recording = metrics::set_default_local_recorder(&recorder);
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://localhost/unused")
            .unwrap();
        let state = Arc::new(RpcState::new(
            pool,
            super::super::Settings {
                metrics_handle: Some(handle.clone()),
                ..Default::default()
            },
            None,
            Arc::new(crate::comms::gatekeeper::voice::rooms::NoVoiceRuntime),
        ));
        let app = super::super::router(state);
        for (method, path, status) in [
            ("GET", "/social-rpc/status", 200),
            ("GET", "/social-rpc/v1/users/invalid/privacy-settings", 400),
            ("POST", "/social-rpc/health/live", 200),
            ("HEAD", "/social-rpc/missing", 404),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), status);
        }
        let before = handle.render();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/social-rpc/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let rendered = handle.render();
        for item in fixture["httpObservations"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| {
                item["name"] == "http_requests_total" || item["name"] == "wkc_logger_logs_total"
            })
        {
            let name = item["name"].as_str().unwrap();
            let matching = |body: &str| {
                body.lines()
                    .filter(|line| {
                        line.starts_with(name)
                            && item["labels"]
                                .as_object()
                                .unwrap()
                                .iter()
                                .all(|(key, value)| {
                                    line.contains(&format!(
                                        "{key}=\"{}\"",
                                        value
                                            .as_str()
                                            .map(str::to_owned)
                                            .unwrap_or_else(|| value.to_string())
                                    ))
                                })
                    })
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            };
            assert_eq!(matching(&rendered).len(), 1, "{item}");
            assert!(matching(&rendered)[0].ends_with(" 1"), "{item}");
            assert_eq!(
                matching(&before),
                matching(&rendered),
                "scrape changed {name}"
            );
        }
        assert!(
            !rendered.contains("http_request_size_bytes_bucket"),
            "uWS never measures request sizes"
        );
    }

    #[test]
    fn unary_wrapper_sizes_codes_and_thrown_errors_match_source_observations() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(env!("DCL_ONE_ROOT"), "/services/catalyst/tests/fixtures/conformance/social/application-metrics-source-cases.json"))).unwrap();
        for case in fixture["unary"].as_array().unwrap() {
            let recorder = crate::comms::metrics::prometheus_builder()
                .unwrap()
                .build_recorder();
            let handle = recorder.handle();
            metrics::with_local_recorder(&recorder, || {
                let name = case["procedure"].as_str().unwrap();
                let started = rpc_request(name, &case["params"]);
                if case["throws"] == true {
                    rpc_finished(name, "ERROR", started);
                } else {
                    rpc_response(name, &case["result"], started);
                }
            });
            let rendered = handle.render();
            for observation in case["observations"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|item| item["name"] != "rpc_procedure_call_duration_seconds")
            {
                let name = observation["name"].as_str().unwrap();
                let name = if name.ends_with("_size_bytes") {
                    format!("{name}_sum")
                } else {
                    name.to_owned()
                };
                let matches = rendered
                    .lines()
                    .filter(|line| {
                        line.starts_with(&name)
                            && observation["labels"].as_object().unwrap().iter().all(
                                |(key, value)| {
                                    line.contains(&format!("{key}=\"{}\"", value.as_str().unwrap()))
                                },
                            )
                    })
                    .collect::<Vec<_>>();
                assert_eq!(matches.len(), 1, "{observation}");
                let actual: f64 = matches[0]
                    .split_whitespace()
                    .last()
                    .unwrap()
                    .parse()
                    .unwrap();
                assert_eq!(
                    actual,
                    observation["value"].as_f64().unwrap(),
                    "{observation}"
                );
            }
        }
    }

    #[tokio::test]
    async fn ordinary_social_api_metrics_match_actual_source_listener_boundaries() {
        use tower::ServiceExt;
        let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(env!("DCL_ONE_ROOT"), "/services/catalyst/tests/fixtures/conformance/social/application-metrics-source-cases.json"))).unwrap();
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://localhost/unused")
            .unwrap();
        let directory =
            std::env::temp_dir().join(format!("social-metrics-api-{}", uuid::Uuid::new_v4()));
        let storage = Arc::new(
            crate::storage::Storage::from_config(&directory)
                .await
                .unwrap(),
        );
        let mut settings = crate::communities::Settings::for_tests();
        settings.admin_token = Some("fixture-admin".into());
        let communities = Arc::new(crate::communities::State::new(
            pool.clone(),
            storage,
            settings,
            None,
            &crate::http::Outbound::for_tests(),
            crate::communities::State::explore_for_tests(&pool),
        ));
        let rpc = Arc::new(RpcState::new(
            pool,
            super::super::Settings::default(),
            None,
            Arc::new(crate::comms::gatekeeper::voice::rooms::NoVoiceRuntime),
        ));
        let gateway = crate::social_api_gateway_with_limit(
            crate::communities::router(communities.clone())
                .merge(super::super::http::router(rpc))
                .route(
                    "/federation/metrics-fixture",
                    axum::routing::get(|| async { "native" }),
                ),
            64,
        );
        let app = axum::Router::new()
            .merge(
                crate::communities::router(communities).layer(axum::middleware::from_fn(
                    super::super::http::instrument_metrics,
                )),
            )
            .nest("/social-api", gateway)
            .route("/other-domain", axum::routing::get(|| async { "native" }));
        for case in fixture["apiCases"].as_array().unwrap() {
            let recorder = crate::comms::metrics::prometheus_builder()
                .unwrap()
                .build_recorder();
            let handle = recorder.handle();
            let _recording = metrics::set_default_local_recorder(&recorder);
            let body = case["body"].as_str().unwrap_or("");
            let mut request = Request::builder()
                .method(case["method"].as_str().unwrap())
                .uri(format!("/social-api{}", case["path"].as_str().unwrap()));
            if !body.is_empty() {
                request = request
                    .header(header::CONTENT_LENGTH, body.len())
                    .header(header::CONTENT_TYPE, "application/json");
            }
            let response = app
                .clone()
                .oneshot(request.body(Body::from(body.to_owned())).unwrap())
                .await
                .unwrap();
            assert_eq!(
                response.status().as_u16(),
                case["status"].as_u64().unwrap() as u16,
                "{case}"
            );
            let rendered = handle.render();
            if case["observations"].as_array().unwrap().is_empty() {
                assert!(!rendered.contains("http_requests_total"), "{case}");
            }
            for observation in case["observations"].as_array().unwrap() {
                let family = observation["name"].as_str().unwrap();
                let name = if family.ends_with("_size_bytes") {
                    format!("{family}_sum")
                } else {
                    family.to_owned()
                };
                let samples = rendered
                    .lines()
                    .filter(|line| {
                        line.starts_with(&name)
                            && observation["labels"].as_object().unwrap().iter().all(
                                |(key, value)| {
                                    line.contains(&format!(
                                        "{key}=\"{}\"",
                                        value
                                            .as_str()
                                            .map(str::to_owned)
                                            .unwrap_or_else(|| value.to_string())
                                    ))
                                },
                            )
                    })
                    .collect::<Vec<_>>();
                assert_eq!(samples.len(), 1, "{observation}\n{rendered}");
                assert_eq!(
                    samples[0]
                        .split_whitespace()
                        .last()
                        .unwrap()
                        .parse::<f64>()
                        .unwrap(),
                    observation["value"].as_f64().unwrap(),
                    "{observation}"
                );
                let duration_count = samples[0]
                    .replacen(&name, "http_request_duration_seconds_count", 1)
                    .rsplit_once(' ')
                    .unwrap()
                    .0
                    .to_owned();
                assert!(
                    rendered
                        .lines()
                        .any(|line| line == format!("{duration_count} 1")),
                    "duration measured with same source labels"
                );
            }
        }
        let recorder = crate::comms::metrics::prometheus_builder()
            .unwrap()
            .build_recorder();
        let handle = recorder.handle();
        let _recording = metrics::set_default_local_recorder(&recorder);
        for path in ["/social-api/federation/metrics-fixture", "/other-domain"] {
            assert_eq!(
                app.clone()
                    .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                    .await
                    .unwrap()
                    .status(),
                StatusCode::OK
            );
        }
        let body = serde_json::json!({"domain": {}, "message": {}, "signature": ""}).to_string();
        let native = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/communities")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::CONTENT_LENGTH, body.len())
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            native
                .extensions()
                .get::<crate::communities::fed::NativeEnvelopeResponse>()
                .is_some(),
            "actual envelope classifier owns the exclusion"
        );
        assert!(!handle.render().contains("http_requests_total"));
        drop(app);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
