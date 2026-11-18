//! The router's series, under upstream's names (`src/metrics.ts`), recorded through the
//! `metrics` facade into the Prometheus exporter as `serve` and Pulse do:
//!
//! - `message_delivery_total{outcome}`: delivered, failed, rejected_oversized, rate_limited;
//! - `message_delivery_latency`: one routed packet, from decode to the last batch, in
//!   milliseconds as upstream, over millisecond buckets;
//! - `livekit_connection_status`: 1 while connected to the room, 0 otherwise.
//!
//! [`serve`] answers `/metrics`, `/health/live` and `/health/ready` (ready while connected).

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use axum::http::StatusCode;
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};

use crate::routing::Outcome;

pub const DELIVERY_TOTAL: &str = "message_delivery_total";
pub const DELIVERY_LATENCY: &str = "message_delivery_latency";
pub const CONNECTION_STATUS: &str = "livekit_connection_status";

const LATENCY_BUCKETS_MS: [f64; 12] = [
    1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0,
];

pub fn outcome(outcome: Outcome) {
    metrics::counter!(DELIVERY_TOTAL, "outcome" => outcome.label()).increment(1);
}

pub fn latency(elapsed: Duration) {
    metrics::histogram!(DELIVERY_LATENCY).record(elapsed.as_secs_f64() * 1000.0);
}

/// Whether the router is in the room: the gauge and the readiness answer.
#[derive(Clone, Default)]
pub struct Connection(Arc<AtomicBool>);

impl Connection {
    pub fn set(&self, connected: bool) {
        self.0.store(connected, Ordering::Release);
        metrics::gauge!(CONNECTION_STATUS).set(if connected { 1.0 } else { 0.0 });
    }

    pub fn get(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

pub fn prometheus_builder() -> PrometheusBuilder {
    PrometheusBuilder::new()
        .set_buckets_for_metric(Matcher::Full(DELIVERY_LATENCY.into()), &LATENCY_BUCKETS_MS)
        .expect("the latency buckets are not empty")
}

/// Installs the process recorder; once, at startup.
pub fn install() -> anyhow::Result<PrometheusHandle> {
    let recorder = prometheus_builder().build_recorder();
    let handle = recorder.handle();
    metrics::set_global_recorder(recorder)
        .map_err(|e| anyhow::anyhow!("installing the prometheus recorder: {e}"))?;
    // every outcome and the gauge are present from the first scrape
    for each in [
        Outcome::Delivered,
        Outcome::Failed,
        Outcome::RejectedOversized,
        Outcome::RateLimited,
    ] {
        metrics::counter!(DELIVERY_TOTAL, "outcome" => each.label()).absolute(0);
    }
    metrics::gauge!(CONNECTION_STATUS).set(0.0);
    Ok(handle)
}

pub fn router(handle: PrometheusHandle, connection: Connection) -> axum::Router {
    use axum::routing::get;
    axum::Router::new()
        .route(
            "/metrics",
            get(move || {
                let handle = handle.clone();
                async move { handle.render() }
            }),
        )
        .route("/health/live", get(|| async { "alive" }))
        .route(
            "/health/ready",
            get(move || {
                let connection = connection.clone();
                async move {
                    if connection.get() {
                        (StatusCode::OK, "ready")
                    } else {
                        (StatusCode::SERVICE_UNAVAILABLE, "not in the room")
                    }
                }
            }),
        )
}

/// Serves [`router`] on `bind` until the process ends.
pub async fn serve(
    bind: SocketAddr,
    handle: PrometheusHandle,
    connection: Connection,
) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .map_err(|e| anyhow::anyhow!("binding MESSAGE_ROUTER_METRICS_BIND {bind}: {e}"))?;
    tracing::info!(%bind, "message router /metrics and /health listening");
    // a recorder built without the exporter's listener drains its histograms only here
    let upkeep = handle.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(5));
        loop {
            tick.tick().await;
            upkeep.run_upkeep();
        }
    });
    axum::serve(listener, router(handle, connection)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower::ServiceExt;

    #[tokio::test]
    async fn series_render_under_upstream_names_and_readiness_follows_the_room() {
        let recorder = prometheus_builder().build_recorder();
        let handle = recorder.handle();
        let connection = Connection::default();
        metrics::with_local_recorder(&recorder, || {
            outcome(Outcome::Delivered);
            outcome(Outcome::RateLimited);
            outcome(Outcome::RateLimited);
            latency(Duration::from_millis(7));
            connection.set(true);
        });
        let app = router(handle, connection.clone());
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::get("/metrics")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let text = String::from_utf8(
            axum::body::to_bytes(response.into_body(), 1 << 20)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(
            text.contains(r#"message_delivery_total{outcome="delivered"} 1"#),
            "{text}"
        );
        assert!(
            text.contains(r#"message_delivery_total{outcome="rate_limited"} 2"#),
            "{text}"
        );
        assert!(
            text.contains(r#"message_delivery_latency_bucket{le="10"} 1"#),
            "{text}"
        );
        assert!(
            text.contains(r#"message_delivery_latency_bucket{le="5"} 0"#),
            "{text}"
        );
        assert!(text.contains("livekit_connection_status 1"), "{text}");

        let ready = |app: axum::Router| async move {
            app.oneshot(
                axum::http::Request::get("/health/ready")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
            .status()
        };
        assert_eq!(ready(app.clone()).await, StatusCode::OK);
        connection.0.store(false, Ordering::Release);
        assert_eq!(ready(app).await, StatusCode::SERVICE_UNAVAILABLE);
    }
}
