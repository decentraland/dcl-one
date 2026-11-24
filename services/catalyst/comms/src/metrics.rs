//! Pulse recorder setup and Prometheus builder for compatibility tests.
pub use dcl_metrics::PrometheusHandle;
use dcl_metrics::{PrometheusBuilder, Registry};
use std::net::SocketAddr;

pub(crate) mod declarations;
pub use declarations::FAMILIES;

pub fn prometheus_builder() -> anyhow::Result<PrometheusBuilder> {
    Registry::new(&[
        crate::pulse::metrics::FAMILIES,
        dcl_metrics::legacy_http::FAMILIES,
    ])?
    .builder(PrometheusBuilder::new())
}

/// Install Pulse's global exporter with its compatibility histogram buckets.
pub fn install_prometheus_exporter(bind: SocketAddr) -> anyhow::Result<PrometheusHandle> {
    dcl_metrics::Registry::new(&[crate::pulse::metrics::FAMILIES])?
        .listen(PrometheusBuilder::new(), bind)
}

pub fn serve_metrics_bind(raw: &str) -> anyhow::Result<SocketAddr> {
    dcl_metrics::loopback_bind("SERVE_METRICS_BIND", raw)
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
