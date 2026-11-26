//! Chain and marketplace indexing in a process with a private metrics listener.
mod metrics;

use crate::settings::{jobs_from_config, land_settings, market_settings};
use crate::{config::Config, content, db, http, land_authz, shop};
use anyhow::Context;
use std::{sync::Arc, time::Duration};

pub async fn run(mut config: Config) -> anyhow::Result<()> {
    config.node.init_tracing();
    tracing::info!("{}", catalyst_config::posture::startup_line(&config));
    config.resolve_secret_files()?;
    let metrics_bind =
        dcl_metrics::loopback_bind("INDEX_METRICS_BIND", &config.node.index_metrics_bind)?;
    registry()?.listen(dcl_metrics::PrometheusBuilder::new(), metrics_bind)?;
    tracing::info!(%metrics_bind, "index /metrics listening");
    let pool = db::connect_pool(
        &config.node.database_url,
        &db::PoolSettings {
            max_connections: config.node.pg_pool_size,
            ..Default::default()
        },
    )
    .await
    .context("connecting index worker to DATABASE_URL")?;
    db::bootstrap(&pool).await?;
    let http = http::Outbound::new(http::HttpClientCfg::serve(
        config.content.connections_max_idle,
    ))?;
    let jobs = jobs_from_config(&config, &pool);
    let settings = market_settings(&config, &content::own_hosts(&config))?;
    settings.check_writer_pair()?;
    settings
        .market
        .indexes
        .check_catalogue_db(
            &pool,
            settings.writes_catalogue(),
            [
                settings.eth_collections_subgraph_url.as_deref(),
                settings.matic_collections_subgraph_url.as_deref(),
            ],
        )
        .await?;
    let market = Arc::new(shop::market::MarketplaceState::new(
        pool.clone(),
        http.clone(),
        settings,
    ));
    market.spawn_index_jobs(&jobs, config.shop.shop_rentals_index_enabled);
    let land = Arc::new(land_authz::LandAuthzState::new(
        pool.clone(),
        http,
        land_settings(&config)?,
    ));
    land.spawn_jobs(&jobs);
    metrics::spawn(
        &jobs,
        &market,
        &land.settings,
        config.shop.shop_rentals_index_enabled,
    );
    let shutdown = crate::serve::shutdown_signal();
    tracing::info!(jobs = ?jobs.report().iter().map(|j| &j.name).collect::<Vec<_>>(), "index worker started");
    shutdown.await;
    let aborted = jobs.shutdown(Duration::from_secs(30)).await;
    if !aborted.is_empty() {
        tracing::warn!(?aborted, "index jobs aborted at shutdown");
    }
    pool.close().await;
    Ok(())
}

/// The index process's families. Its mirrors and readers query subgraphs
/// through `chain_index::graph`, so it declares the `subgraph_*` series too.
fn registry() -> anyhow::Result<dcl_metrics::Registry> {
    dcl_metrics::Registry::new(&[
        crate::jobs::FAMILIES,
        crate::chain_index::portal::FAMILIES,
        crate::chain_index::graph::FAMILIES,
        crate::shop::market::SALES_REPAIR_METRICS,
        crate::shop::market::mirrors::FAMILIES,
        crate::shop::market::collections_v1::FAMILIES,
        metrics::FAMILIES,
    ])
}

#[cfg(test)]
pub(crate) mod tests {
    use serde_json::json;

    /// A subgraph query recorded through `registry` renders the `subgraph_*`
    /// series, the duration as a histogram with its declared buckets: without
    /// the declaration it would render as a summary.
    pub(crate) async fn assert_registry_renders_subgraph_series(registry: dcl_metrics::Registry) {
        let recorder = registry
            .builder(dcl_metrics::PrometheusBuilder::new())
            .unwrap()
            .build_recorder();
        let handle = recorder.handle();
        let _recording = dcl_metrics::set_default_local_recorder(&recorder);
        let (index, _) = crate::chain_index::graph::test_support::index_with_names().await;
        let url = index.settings.marketplace_subgraph_url.clone().unwrap();
        crate::chain_index::graph::graph_query(&index, &url, "{ nfts { id } }", json!({}))
            .await
            .unwrap();
        let rendered = handle.render();
        for line in [
            "# TYPE subgraph_query_duration_seconds histogram".to_string(),
            format!("subgraph_query_duration_seconds_bucket{{url=\"{url}\",le=\"60\"}} 1"),
            format!("subgraph_ok_total{{url=\"{url}\"}} 1"),
        ] {
            assert!(rendered.contains(&line), "missing {line}:\n{rendered}");
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn the_index_registry_renders_the_subgraph_series() {
        assert_registry_renders_subgraph_series(super::registry().unwrap()).await;
    }

    #[test]
    fn the_index_registry_declares_catalogue_proof_outcomes() {
        let recorder = super::registry()
            .unwrap()
            .build_recorder(dcl_metrics::PrometheusBuilder::new())
            .unwrap();
        let _recording = dcl_metrics::set_default_local_recorder(&recorder);
        dcl_metrics::counter!("catalyst_catalogue_mirror_proof_outcomes_total",
            "mirror" => "items", "network" => "matic", "outcome" => "rpc_auth_failed")
        .increment(1);
        let rendered = recorder.handle().render();
        assert!(rendered.contains(
            "# HELP catalyst_catalogue_mirror_proof_outcomes_total Mainnet catalogue mirror proof failures and skips."
        ), "{rendered}");
    }
}
