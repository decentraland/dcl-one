//! Metric contracts from the pinned Social source.
use dcl_metrics::{Descriptor, Unit};

const PAYLOAD_SIZE_BUCKETS: &[f64] = &[
    64.0, 128.0, 256.0, 512.0, 1024.0, 2048.0, 4096.0, 8192.0, 16384.0,
];

const SOCKET_DURATION_BUCKETS: &[f64] = &[
    1.0, 5.0, 10.0, 30.0, 60.0, 300.0, 600.0, 1800.0, 3600.0, 7200.0, 14400.0,
];

pub const FAMILIES: &[Descriptor] = &[
    dcl_metrics::legacy_http::HTTP_REQUEST_DURATION_SECONDS,
    dcl_metrics::legacy_http::HTTP_REQUESTS_TOTAL,
    dcl_metrics::legacy_http::HTTP_REQUEST_SIZE_BYTES,
    Descriptor::counter(
        "wkc_logger_logs_total",
        "Log amounts per logger and level",
        Some(Unit::Count),
        &["level", "logger"],
    ),
    Descriptor::gauge(
        "ws_active_connections_by_protocol",
        "Active WebSocket connections by protocol",
        None,
        &["protocol"],
    ),
    Descriptor::histogram(
        "ws_connection_duration_seconds_by_protocol",
        "WebSocket connection duration by protocol",
        Some(Unit::Seconds),
        &["protocol"],
        SOCKET_DURATION_BUCKETS,
    ),
    Descriptor::counter(
        "ws_auth_errors_by_protocol",
        "Authentication failures by protocol and cause",
        Some(Unit::Count),
        &["protocol", "type"],
    ),
    Descriptor::counter(
        "ws_admission_evictions",
        "Oldest WebSocket connections closed to keep an address within its connection cap",
        Some(Unit::Count),
        &["protocol"],
    ),
    Descriptor::counter(
        "ws_admission_rejections",
        "Subscription starts rejected by the per-address rate limit",
        Some(Unit::Count),
        &["protocol", "reason"],
    ),
    Descriptor::gauge(
        "ws_active_connections",
        "Number of WebSocket active connections",
        None,
        &[],
    ),
    Descriptor::counter(
        "ws_connections_rejected",
        "Number of WebSocket connections rejected because the pool reached WS_MAX_CONCURRENT_CONNECTIONS",
        Some(Unit::Count),
        &[],
    ),
    Descriptor::counter(
        "ws_messages_received",
        "Number of WebSocket messages received",
        Some(Unit::Count),
        &[],
    ),
    Descriptor::counter(
        "ws_messages_sent",
        "Number of outbound WebSocket messages accepted by the socket (sent or buffered)",
        Some(Unit::Count),
        &[],
    ),
    Descriptor::counter(
        "ws_errors",
        "Number of WebSocket errors",
        Some(Unit::Count),
        &[],
    ),
    Descriptor::counter(
        "ws_close_codes",
        "Number of WebSocket close codes",
        Some(Unit::Count),
        &["code"],
    ),
    Descriptor::counter(
        "ws_transport_errors",
        "Number of WebSocket transport errors",
        Some(Unit::Count),
        &[],
    ),
    Descriptor::counter(
        "ws_auth_errors",
        "Number of WebSocket authentication errors",
        Some(Unit::Count),
        &["type"],
    ),
    Descriptor::counter(
        "ws_auth_race_condition_aborted",
        "Number of authentication attempts aborted due to connection closed during auth",
        Some(Unit::Count),
        &[],
    ),
    Descriptor::counter(
        "ws_idle_timeouts",
        "Number of WebSocket idle timeouts",
        Some(Unit::Count),
        &[],
    ),
    Descriptor::histogram(
        "ws_connection_duration_seconds",
        "Duration of WebSocket connections in seconds",
        Some(Unit::Seconds),
        &[],
        SOCKET_DURATION_BUCKETS,
    ),
    Descriptor::counter(
        "rpc_procedure_call_total",
        "Total number of RPC procedure calls",
        Some(Unit::Count),
        &["code", "procedure"],
    ),
    Descriptor::histogram(
        "rpc_procedure_call_duration_seconds",
        "Duration of RPC procedure calls in seconds",
        Some(Unit::Seconds),
        &["code", "procedure"],
        &[0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0, 10.0],
    ),
    Descriptor::histogram(
        "rpc_in_procedure_call_size_bytes",
        "Size of incoming RPC procedure call payloads in bytes",
        Some(Unit::Bytes),
        &["procedure"],
        PAYLOAD_SIZE_BUCKETS,
    ),
    Descriptor::histogram(
        "rpc_out_procedure_call_size_bytes",
        "Size of outgoing RPC procedure call payloads in bytes",
        Some(Unit::Bytes),
        &["code", "procedure"],
        PAYLOAD_SIZE_BUCKETS,
    ),
    Descriptor::counter(
        "rpc_updates_sent_on_subscription",
        "Number of updates sent on RPC subscription streams",
        Some(Unit::Count),
        &["event"],
    ),
    Descriptor::counter(
        "stream_ws_messages_received",
        "Number of messages received on the plain WebSocket stream protocol",
        Some(Unit::Count),
        &["type"],
    ),
    Descriptor::counter(
        "stream_ws_updates_sent",
        "Number of updates sent on plain WebSocket stream subscriptions",
        Some(Unit::Count),
        &["event"],
    ),
    Descriptor::counter(
        "stream_ws_errors",
        "Number of errors reported on the plain WebSocket stream protocol",
        Some(Unit::Count),
        &["type"],
    ),
    Descriptor::counter(
        "stream_ws_authentications",
        "Successful plain WebSocket stream authentications by handshake method",
        Some(Unit::Count),
        &["method"],
    ),
    Descriptor::counter(
        "stream_ws_close_codes",
        "Number of plain WebSocket stream protocol close codes",
        Some(Unit::Count),
        &["code"],
    ),
    Descriptor::counter(
        "ws_unexpected_send_result_events",
        "Number of unexpected send result events",
        Some(Unit::Count),
        &[],
    ),
    Descriptor::counter(
        "ws_backpressure_events",
        "Number of WebSocket messages that encountered backpressure or were dropped",
        Some(Unit::Count),
        &["result"],
    ),
    Descriptor::counter(
        "ws_drain_events",
        "Number of WebSocket drain events",
        Some(Unit::Count),
        &[],
    ),
    Descriptor::histogram(
        "ws_message_size_bytes",
        "Size of WebSocket messages in bytes",
        Some(Unit::Bytes),
        &["result"],
        PAYLOAD_SIZE_BUCKETS,
    ),
    Descriptor::gauge(
        "ws_queue_vs_backpressure_ratio",
        "Ratio of message queue size to uWebSocket buffered amount",
        None,
        &[],
    ),
    Descriptor::gauge(
        "subscribers_local_count",
        "Number of local in-memory subscriber emitters",
        None,
        &[],
    ),
    Descriptor::gauge(
        "subscribers_generators_count",
        "Number of active async generators across all subscribers",
        None,
        &[],
    ),
    Descriptor::counter(
        "subscribers_stale_cleaned",
        "Number of stale subscribers removed by reconciliation",
        Some(Unit::Count),
        &[],
    ),
    Descriptor::counter(
        "subscribers_stale_connections_cleaned",
        "Number of stale connections (socket gone, address still active) removed by reconciliation",
        Some(Unit::Count),
        &[],
    ),
    Descriptor::counter(
        "subscription_duplicates_total",
        "Number of duplicate stream subscriptions rejected by the per-(address, event) guard",
        Some(Unit::Count),
        &["event"],
    ),
    Descriptor::counter(
        "subscription_updates_dropped_total",
        "Number of subscription updates dropped because a connection's value queue overflowed (slow consumer)",
        Some(Unit::Count),
        &["event"],
    ),
    Descriptor::counter(
        "friendship_rate_limiter_unavailable",
        "Number of friendship/block mutations allowed because the Redis rate limiter was unreachable",
        Some(Unit::Count),
        &[],
    ),
    Descriptor::histogram(
        "ai_compliance_validation_duration_seconds",
        "Duration of AI compliance validation in seconds",
        Some(Unit::Seconds),
        &[],
        &[0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0],
    ),
    Descriptor::counter(
        "ai_compliance_validation_total",
        "Total number of AI compliance validations (compliant, non-compliant, failed)",
        Some(Unit::Count),
        &["result"],
    ),
];

#[cfg(test)]
mod tests {
    use super::super::metrics_builder as prometheus_builder;
    use std::collections::BTreeMap;
    #[test]
    fn social_message_histogram_matches_source_wire_buckets_counts_and_sums() {
        let capture: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/services/catalyst/tests/fixtures/conformance/social/ws-metrics-source-cases.json"
        )))
        .unwrap();
        assert_eq!(
            capture["baseline"]["revision"],
            "b6b2cefefdf727b2ed47ddb887e77d226a41eea6"
        );
        let recorder = prometheus_builder().unwrap().build_recorder();
        let handle = recorder.handle();
        dcl_metrics::with_local_recorder(&recorder, || {
            for observation in capture["observations"].as_array().unwrap() {
                dcl_metrics::histogram!(
                    "ws_message_size_bytes",
                    "result" => observation["result"].as_str().unwrap().to_owned()
                )
                .record(observation["bytes"].as_f64().unwrap());
            }
        });

        // Compare parsed wire types, exact label sets (including le), and every
        // cumulative bucket, count and sum. Label ordering and HELP text may differ.
        type Samples = BTreeMap<(String, BTreeMap<String, String>), f64>;
        fn parse(body: &str) -> (String, Samples) {
            let mut kind = None;
            let mut samples = BTreeMap::new();
            for line in body.lines() {
                if let Some(value) = line.strip_prefix("# TYPE ws_message_size_bytes ") {
                    assert!(kind.replace(value.to_owned()).is_none());
                } else if line.starts_with("ws_message_size_bytes") {
                    let (series, value) = line.split_once(' ').unwrap();
                    let (name, labels) = series.split_once('{').unwrap();
                    let labels = labels
                        .strip_suffix('}')
                        .unwrap()
                        .split(',')
                        .map(|label| {
                            let (key, value) = label.split_once('=').unwrap();
                            (key.to_owned(), value.trim_matches('"').to_owned())
                        })
                        .collect();
                    assert!(
                        samples
                            .insert((name.to_owned(), labels), value.parse().unwrap())
                            .is_none()
                    );
                }
            }
            (kind.expect("histogram TYPE metadata"), samples)
        }
        let expected = parse(capture["prometheus"].as_str().unwrap());
        assert_eq!(expected.0, "histogram");
        assert_eq!(parse(&handle.render()), expected);
    }

    #[test]
    fn all_nine_social_histograms_export_the_source_bucket_vectors() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(env!("DCL_ONE_ROOT"), "/services/catalyst/tests/fixtures/conformance/social/application-metrics-source-cases.json"))).unwrap();
        assert_eq!(fixture["histograms"].as_array().unwrap().len(), 9);
        let recorder = prometheus_builder().unwrap().build_recorder();
        let handle = recorder.handle();
        dcl_metrics::with_local_recorder(&recorder, || {
            for item in fixture["histograms"].as_array().unwrap() {
                dcl_metrics::histogram!(item["name"].as_str().unwrap().to_owned()).record(1.0);
            }
        });
        let rendered = handle.render();
        for item in fixture["histograms"].as_array().unwrap() {
            let name = item["name"].as_str().unwrap();
            assert!(
                rendered.contains(&format!("# TYPE {name} histogram")),
                "{name}"
            );
            let prefix = format!("{name}_bucket{{le=\"");
            let actual = rendered
                .lines()
                .filter_map(|line| line.strip_prefix(&prefix))
                .map(|line| line.split('"').next().unwrap().to_owned())
                .collect::<Vec<_>>();
            let expected = item["buckets"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| {
                    if value == "+Inf" {
                        "+Inf".to_owned()
                    } else {
                        value.as_f64().unwrap().to_string()
                    }
                })
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "{name}");
        }
    }
}
