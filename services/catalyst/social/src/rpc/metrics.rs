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

pub(super) const FAMILIES: &[&str] = &[
    "http_request_duration_seconds",
    "http_requests_total",
    "http_request_size_bytes",
    "wkc_logger_logs_total",
    "ws_active_connections_by_protocol",
    "ws_connection_duration_seconds_by_protocol",
    "ws_auth_errors_by_protocol",
    "ws_admission_evictions",
    "ws_admission_rejections",
    "ws_active_connections",
    "ws_connections_rejected",
    "ws_messages_received",
    "ws_messages_sent",
    "ws_errors",
    "ws_close_codes",
    "ws_transport_errors",
    "ws_auth_errors",
    "ws_auth_race_condition_aborted",
    "ws_idle_timeouts",
    "ws_connection_duration_seconds",
    "rpc_procedure_call_total",
    "rpc_procedure_call_duration_seconds",
    "rpc_in_procedure_call_size_bytes",
    "rpc_out_procedure_call_size_bytes",
    "rpc_updates_sent_on_subscription",
    "stream_ws_messages_received",
    "stream_ws_updates_sent",
    "stream_ws_errors",
    "stream_ws_authentications",
    "stream_ws_close_codes",
    "ws_unexpected_send_result_events",
    "ws_backpressure_events",
    "ws_drain_events",
    "ws_message_size_bytes",
    "ws_queue_vs_backpressure_ratio",
    "subscribers_local_count",
    "subscribers_generators_count",
    "subscribers_stale_cleaned",
    "subscribers_stale_connections_cleaned",
    "subscription_duplicates_total",
    "subscription_updates_dropped_total",
    "friendship_rate_limiter_unavailable",
    "ai_compliance_validation_duration_seconds",
    "ai_compliance_validation_total",
];

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
        return next.run(request).await;
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
    for line in rendered.lines() {
        let name = if line.starts_with("# HELP ") || line.starts_with("# TYPE ") {
            line.split_whitespace().nth(2)
        } else if line.starts_with('#') {
            None
        } else {
            line.split(['{', ' ']).next()
        };
        let Some(name) = name else { continue };
        if FAMILIES.iter().any(|family| {
            name == *family
                || ["_sum", "_count", "_bucket"]
                    .iter()
                    .any(|suffix| name.strip_suffix(suffix) == Some(*family))
        }) {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_view_names_pinned_source_families_and_excludes_other_domains() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/services/catalyst/tests/fixtures/conformance/social/auxiliary-cases.json"
        )))
        .unwrap();
        let expected = fixture["metric_declarations"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            FAMILIES
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>(),
            expected
        );
        assert_eq!(
            filter_families(
                "# TYPE ws_message_size_bytes histogram\nws_message_size_bytes_bucket{le=\"64\",result=\"success\"} 1\nws_message_size_bytes_count{result=\"success\"} 1\nws_message_size_bytes_sum{result=\"success\"} 32\n# TYPE shop_orders_total counter\nshop_orders_total 99\n"
            ),
            "# TYPE ws_message_size_bytes histogram\nws_message_size_bytes_bucket{le=\"64\",result=\"success\"} 1\nws_message_size_bytes_count{result=\"success\"} 1\nws_message_size_bytes_sum{result=\"success\"} 32\n"
        );
    }
}
