//! Root HTTP server lifecycle.
use crate::{comms, jobs};
use anyhow::Context;
use std::{sync::Arc, time::Duration};

/// How long the workers get to stop (flush, persist cursors) before they are aborted.
const JOBS_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) fn register_metrics() {
    comms::gatekeeper::admission::cluster::metrics::register_series();
    crate::worlds::core::metrics::register_series();
    // Every process starts bootstrapping, as the legacy content server reported from its start;
    // the sync leader or the status mirror moves the state on.
    crate::content::sync::metrics::sync_state(&crate::content::SyncState::Bootstrapping);
}

fn warn_retired_metrics_settings() {
    let mut configured = Vec::new();
    for name in [
        "WKC_METRICS_PUBLIC_PATH",
        "WKC_METRICS_BEARER_TOKEN",
        "WKC_METRICS_COLLECT_DEFAULT",
        "WKC_METRICS_RESET_AT_NIGHT",
        "CAMERA_REEL_METRICS_BEARER_TOKEN",
    ] {
        for key in [name.to_owned(), format!("{name}_FILE")] {
            if std::env::var_os(&key).is_some() {
                configured.push(key);
            }
        }
    }
    if !configured.is_empty() {
        tracing::warn!(
            settings = %configured.join(", "),
            "remove these retired metrics settings and scrape /metrics at the address configured by SERVE_METRICS_BIND"
        );
    }
}

pub(super) fn install_metrics(raw: &str) -> anyhow::Result<()> {
    warn_retired_metrics_settings();
    let bind = comms::metrics::serve_metrics_bind(raw)?;
    let registry = registry()?;
    registry.listen(dcl_metrics::PrometheusBuilder::new(), bind)?;
    register_metrics();
    tracing::info!(%bind, "serve /metrics listening");
    Ok(())
}

fn registry() -> anyhow::Result<dcl_metrics::Registry> {
    dcl_metrics::Registry::new(&[
        crate::builder::handlers::collections::FAMILIES,
        jobs::FAMILIES,
        crate::storage::FAMILIES,
        crate::worlds::core::metrics::FAMILIES,
        crate::social::rpc::FAMILIES,
        crate::social::SERVICE_FAMILIES,
        crate::comms::metrics::FAMILIES,
        crate::content::metrics::FAMILIES,
        crate::content::series::FAMILIES,
        crate::content::sync::metrics::FAMILIES,
        comms::archipelago::metrics::FAMILIES,
        crate::chain_index::portal::FAMILIES,
        crate::explore::api::auth_api::FAMILIES,
        crate::explore::api::map::FAMILIES,
        crate::explore::api::registry::FAMILIES,
        crate::explore::api::auth_api::SOCKET_FAMILIES,
        crate::shop::market::profiles::FAMILIES,
        crate::shop::market::lambdas_metrics::FAMILIES,
        crate::shop::credits::metrics::FAMILIES,
        crate::shop::market::orders::FAMILIES,
        crate::chain_index::graph::FAMILIES,
    ])
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

#[cfg(test)]
mod tests {
    #[test]
    fn all_serve_metric_declarations_can_share_one_recorder() {
        super::registry().unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn the_serve_registry_renders_the_subgraph_series() {
        crate::index::tests::assert_registry_renders_subgraph_series(super::registry().unwrap())
            .await;
    }

    /// The serve registry exports the content histograms with their declared buckets.
    #[test]
    fn serve_renders_the_content_histograms_with_their_buckets() {
        let recorder = super::registry()
            .unwrap()
            .builder(dcl_metrics::PrometheusBuilder::new())
            .unwrap()
            .build_recorder();
        let handle = recorder.handle();
        dcl_metrics::with_local_recorder(&recorder, || {
            dcl_metrics::histogram!("dcl_partial_upload_stage_duration_seconds", "stage" => "hash")
                .record(1.0);
            dcl_metrics::histogram!(
                "dcl_content_server_snapshot_generation_time",
                "result" => "success",
                "interval_size" => "day",
                "reason" => "cover_time_range"
            )
            .record(1.0);
        });
        let rendered = handle.render();
        for (name, buckets) in [
            (
                "dcl_partial_upload_stage_duration_seconds",
                &[
                    0.005, 0.025, 0.1, 0.25, 0.5, 1., 2.5, 5., 10., 30., 60., 120.,
                ][..],
            ),
            (
                "dcl_content_server_snapshot_generation_time",
                &[0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1., 2.5, 5., 10.][..],
            ),
        ] {
            assert!(
                rendered.contains(&format!("# TYPE {name} histogram")),
                "{name}: {rendered}"
            );
            let le: Vec<&str> = rendered
                .lines()
                .filter(|line| line.starts_with(&format!("{name}_bucket{{")))
                .filter_map(|line| line.split("le=\"").nth(1)?.split('"').next())
                .collect();
            let mut expected: Vec<String> = buckets.iter().map(f64::to_string).collect();
            expected.push("+Inf".to_string());
            assert_eq!(le, expected, "{name}");
        }
    }

    /// The serve registry exports every sync histogram with its declared buckets, and each sync
    /// family's HELP and TYPE once.
    #[test]
    fn serve_renders_the_sync_families_once_with_their_buckets() {
        use crate::content::sync::metrics as sync;
        let recorder = super::registry()
            .unwrap()
            .builder(dcl_metrics::PrometheusBuilder::new())
            .unwrap()
            .build_recorder();
        let handle = recorder.handle();
        dcl_metrics::with_local_recorder(&recorder, || {
            let peer = "https://peer.example/content";
            sync::download_finished("https://peer.example", std::time::Duration::ZERO, Some(1));
            sync::download_hash_error("https://peer.example");
            sync::pointer_changes_page(peer, std::time::Duration::ZERO);
            sync::download_job_started(1);
            sync::download_job_succeeded(1);
            sync::entity_deployed("scene", std::time::Duration::ZERO, true);
            sync::server_phases(0, 0, 0);
        });
        let rendered = handle.render();
        let histograms = sync::FAMILIES
            .iter()
            .filter(|family| rendered.contains(&format!("# TYPE {} histogram", family.name)))
            .count();
        assert_eq!(histograms, 5, "{rendered}");
        for family in sync::FAMILIES {
            for kind in ["HELP", "TYPE"] {
                let line = format!("# {kind} {} ", family.name);
                assert!(
                    rendered.matches(&line).count() <= 1,
                    "{line} repeated:\n{rendered}"
                );
            }
        }
        for (name, buckets) in [
            (
                "dcl_content_download_duration_seconds",
                &[
                    0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1., 2.5, 5., 10., 30., 60.,
                ][..],
            ),
            (
                "dcl_catalysts_pointer_changes_response_time_seconds",
                &[
                    0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1., 2.5, 5., 10., 30., 60.,
                ][..],
            ),
            (
                "dcl_content_download_job_succeed_retries",
                &[1., 2., 3., 4., 5., 6., 7., 8., 9., 10., 15., 20., 30.][..],
            ),
            (
                "dcl_available_servers_histogram",
                &[
                    1., 2., 3., 4., 5., 6., 7., 8., 9., 10., 11., 12., 13., 14., 15.,
                ][..],
            ),
            (
                "dcl_deployment_time",
                &[0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1., 2.5, 5., 10.][..],
            ),
        ] {
            let le: Vec<&str> = rendered
                .lines()
                .filter(|line| line.starts_with(&format!("{name}_bucket{{")))
                .filter_map(|line| line.split("le=\"").nth(1)?.split('"').next())
                .collect();
            let mut expected: Vec<String> = buckets.iter().map(f64::to_string).collect();
            expected.push("+Inf".to_string());
            assert_eq!(le, expected, "{name}");
        }
    }
}
