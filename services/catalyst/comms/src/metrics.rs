//! One Prometheus exporter installer for the Pulse and serve processes.
use crate::pulse::metrics::{CLUSTER_SIZE_BUCKETS, RESYNC_SEQ_GAP_BUCKETS};
use metrics_exporter_prometheus::{BuildError, Matcher, PrometheusBuilder};
use std::net::SocketAddr;

/// Preserve aggregatable Pulse histograms in every recorder, including tests.
pub fn prometheus_builder() -> Result<PrometheusBuilder, BuildError> {
    PrometheusBuilder::new()
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
pub fn install_prometheus_exporter(bind: SocketAddr) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Handle::try_current()?;
    let (recorder, exporter) = prometheus_builder()?
        .with_http_listener(bind)
        .build()
        .map_err(|e| anyhow::anyhow!("failed to bind prometheus exporter on {bind}: {e}"))?;
    metrics::set_global_recorder(recorder)
        .map_err(|e| anyhow::anyhow!("failed to install prometheus recorder: {e}"))?;
    runtime.spawn(exporter);
    Ok(())
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
