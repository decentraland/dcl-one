//! Root HTTP server lifecycle.
use crate::{comms, content, jobs};
use anyhow::Context;
use std::{sync::Arc, time::Duration};

/// How long the workers get to stop (flush, persist cursors) before they are aborted.
const JOBS_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) fn register_metrics() {
    comms::gatekeeper::admission::cluster::metrics::register_series();
    content::series::register_series();
}

pub(super) fn install_metrics(raw: &str) -> anyhow::Result<comms::metrics::PrometheusHandle> {
    let bind = comms::metrics::serve_metrics_bind(raw)?;
    let handle = comms::metrics::install_prometheus_exporter(bind)?;
    register_metrics();
    tracing::info!(%bind, "serve /metrics listening");
    Ok(handle)
}

/// Installs the shutdown signal handlers now and returns the wait for one of
/// them. Call it before logging that the process is ready, so a signal sent
/// in reply to that line finds its handler.
pub(crate) fn shutdown_signal() -> impl std::future::Future<Output = ()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut interrupt = signal(SignalKind::interrupt()).expect("installing the SIGINT handler");
        let mut term = signal(SignalKind::terminate()).expect("installing the SIGTERM handler");
        async move {
            tokio::select! {
                _ = interrupt.recv() => tracing::info!("SIGINT received"),
                _ = term.recv() => tracing::info!("SIGTERM received"),
            }
        }
    }
    #[cfg(not(unix))]
    async {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("shutdown signal received");
    }
}

pub(super) async fn run_http(
    addr: String,
    app: axum::Router,
    connector_lifecycle: comms::archipelago::AppState,
    jobs: &Arc<jobs::Jobs>,
) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("binding {addr} (HTTP_SERVER_HOST + HTTP_SERVER_PORT)"))?;
    connector_lifecycle
        .started
        .store(true, std::sync::atomic::Ordering::Release);
    let shutdown = shutdown_signal();
    tracing::info!(%addr, "catalyst listening");
    let shutdown_jobs = jobs.clone();
    let app = app.layer(axum::middleware::from_fn(
        comms::archipelago::ws::raw_transport::connection_info,
    ));
    axum::serve(
        comms::archipelago::ws::raw_transport::Listener(listener),
        axum::ServiceExt::<axum::extract::Request>::into_make_service_with_connect_info::<
            comms::archipelago::ws::raw_transport::Connection,
        >(app),
    )
    .with_graceful_shutdown(async move {
        shutdown.await;
        comms::archipelago::shutdown(&connector_lifecycle).await;
        let aborted = shutdown_jobs.shutdown(JOBS_SHUTDOWN_TIMEOUT).await;
        if !aborted.is_empty() {
            tracing::warn!(?aborted, "jobs aborted at shutdown");
        }
        tracing::info!("workers stopped; closing the listener");
    })
    .await?;
    Ok(())
}
