//! One Prometheus exporter installer for the Pulse and serve processes.
use crate::pulse::metrics::{CLUSTER_SIZE_BUCKETS, RESYNC_SEQ_GAP_BUCKETS};
pub use metrics_exporter_prometheus::PrometheusHandle;
use metrics_exporter_prometheus::{BuildError, Matcher, PrometheusBuilder};
use std::net::SocketAddr;

/// The social sockets' lifetime buckets (upstream social-service-ea
/// `ws_connection_duration_seconds_by_protocol`, seconds).
const SOCIAL_SOCKET_DURATION_BUCKETS: [f64; 11] = [
    1.0, 5.0, 10.0, 30.0, 60.0, 300.0, 600.0, 1800.0, 3600.0, 7200.0, 14400.0,
];
/// Source transport send-attempt sizes, including accepted and dropped frames.
const SOCIAL_MESSAGE_SIZE_BUCKETS: [f64; 9] = [
    64.0, 128.0, 256.0, 512.0, 1024.0, 2048.0, 4096.0, 8192.0, 16384.0,
];

/// The worlds histograms' buckets, as worlds-content-server's `src/metrics.ts` declares them.
const WORLDS_BUCKETS: [(&str, &[f64]); 7] = [
    (
        "deployment_processing_stage_duration_seconds",
        &[
            0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 15.0, 30.0, 60.0, 120.0, 300.0,
        ],
    ),
    (
        "multipart_upload_receive_duration_seconds",
        &[
            0.1, 0.5, 1.0, 2.5, 5.0, 10.0, 20.0, 30.0, 60.0, 120.0, 300.0, 600.0,
        ],
    ),
    (
        "multipart_upload_size_bytes",
        &[
            1024.0,
            1_048_576.0,
            10_485_760.0,
            104_857_600.0,
            367_001_600.0,
        ],
    ),
    (
        "garbage_collection_duration_seconds",
        &[1.0, 5.0, 15.0, 60.0, 300.0, 900.0, 1800.0, 3600.0, 7200.0],
    ),
    (
        "partial_upload_duration_seconds",
        &[
            5.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0, 900.0, 1800.0, 2700.0, 3600.0,
        ],
    ),
    (
        "partial_upload_batches_per_upload",
        &[1.0, 2.0, 3.0, 5.0, 10.0, 20.0, 50.0, 100.0, 250.0],
    ),
    (
        "partial_upload_cleanup_duration_seconds",
        &[0.1, 0.5, 1.0, 5.0, 15.0, 30.0, 60.0, 120.0, 300.0],
    ),
];

/// Preserve aggregatable Pulse and social histograms in every recorder, including tests.
pub fn prometheus_builder() -> Result<PrometheusBuilder, BuildError> {
    let mut builder = PrometheusBuilder::new();
    for (name, buckets) in WORLDS_BUCKETS {
        builder = builder.set_buckets_for_metric(Matcher::Full(name.into()), buckets)?;
    }
    for name in [
        "ws_connection_duration_seconds_by_protocol",
        "ws_connection_duration_seconds",
    ] {
        builder = builder
            .set_buckets_for_metric(Matcher::Full(name.into()), &SOCIAL_SOCKET_DURATION_BUCKETS)?;
    }
    for name in [
        "ws_message_size_bytes",
        "rpc_in_procedure_call_size_bytes",
        "rpc_out_procedure_call_size_bytes",
    ] {
        builder = builder
            .set_buckets_for_metric(Matcher::Full(name.into()), &SOCIAL_MESSAGE_SIZE_BUCKETS)?;
    }
    for name in ["http_request_duration_seconds", "http_request_size_bytes"] {
        builder = builder.set_buckets_for_metric(
            Matcher::Full(name.into()),
            &[
                0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ],
        )?;
    }
    builder
        .set_buckets_for_metric(
            Matcher::Full("rpc_procedure_call_duration_seconds".into()),
            &[0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0, 10.0],
        )?
        .set_buckets_for_metric(
            Matcher::Full("ai_compliance_validation_duration_seconds".into()),
            &[0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0],
        )?
        .set_buckets_for_metric(
            Matcher::Full("pulse_resync_seq_gap".into()),
            &RESYNC_SEQ_GAP_BUCKETS,
        )?
        .set_buckets_for_metric(
            Matcher::Full("pulse_cluster_size".into()),
            &CLUSTER_SIZE_BUCKETS,
        )
}

/// Install once during fatal process startup, inside Tokio. Callers must propagate
/// an error to process termination, never retry in a surviving runtime: the pinned
/// exporter starts detached upkeep in `build`, even when binding subsequently fails.
/// Both production callers propagate through `main`, which drops that runtime.
/// Duplicate recorder rejection drops the unspawned HTTP listener, but its upkeep
/// also lasts until runtime exit. This is not a reusable/recoverable installer.
pub fn install_prometheus_exporter(bind: SocketAddr) -> anyhow::Result<PrometheusHandle> {
    let runtime = tokio::runtime::Handle::try_current()?;
    let (recorder, exporter) = prometheus_builder()?
        .with_http_listener(bind)
        .build()
        .map_err(|e| anyhow::anyhow!("failed to bind prometheus exporter on {bind}: {e}"))?;
    let handle = recorder.handle();
    metrics::set_global_recorder(recorder)
        .map_err(|e| anyhow::anyhow!("failed to install prometheus recorder: {e}"))?;
    runtime.spawn(exporter);
    Ok(handle)
}

/// Serve's exporter is operator-local; exposing it requires an explicit proxy.
pub fn serve_metrics_bind(raw: &str) -> anyhow::Result<SocketAddr> {
    let bind: SocketAddr = raw
        .parse()
        .map_err(|e| anyhow::anyhow!("SERVE_METRICS_BIND `{raw}`: {e}"))?;
    anyhow::ensure!(
        bind.ip().is_loopback(),
        "SERVE_METRICS_BIND must use a loopback address"
    );
    Ok(bind)
}

#[cfg(test)]
mod tests {
    use super::*;
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
        metrics::with_local_recorder(&recorder, || {
            for observation in capture["observations"].as_array().unwrap() {
                metrics::histogram!(
                    "ws_message_size_bytes",
                    "result" => observation["result"].as_str().unwrap().to_owned()
                )
                .record(observation["bytes"].as_f64().unwrap());
            }
        });

        // Compare parsed wire types, exact label sets (including le), and every
        // cumulative bucket, count and sum. Label ordering and HELP text may differ.
        fn parse(body: &str) -> (String, BTreeMap<(String, BTreeMap<String, String>), f64>) {
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
        metrics::with_local_recorder(&recorder, || {
            for item in fixture["histograms"].as_array().unwrap() {
                metrics::histogram!(item["name"].as_str().unwrap().to_owned()).record(1.0);
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

    #[test]
    fn serve_bind_accepts_only_loopback_sockets() {
        for raw in ["127.0.0.1:5006", "[::1]:5006"] {
            assert!(serve_metrics_bind(raw).is_ok());
        }
        for raw in ["0.0.0.0:5006", "[::]:5006", "192.0.2.1:5006", "invalid"] {
            assert!(serve_metrics_bind(raw).is_err(), "{raw}");
        }
    }

    #[tokio::test]
    async fn occupied_exporter_port_is_a_startup_error() {
        let occupied = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        assert!(install_prometheus_exporter(occupied.local_addr().unwrap()).is_err());
    }
}
