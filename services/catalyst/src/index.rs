//! Chain and marketplace indexing in a process without an HTTP listener.
use crate::{config::Config, content, db, http, jobs, land_authz, shop};
use anyhow::Context;
use std::{sync::Arc, time::Duration};

pub async fn run(mut config: Config) -> anyhow::Result<()> {
    config.node.init_tracing();
    tracing::info!("{}", catalyst_config::posture::startup_line(&config));
    config.resolve_secret_files()?;
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
    let jobs = if config.node.job_leases_enabled {
        jobs::Jobs::with_leases(
            pool.clone(),
            jobs::LeaseSettings {
                namespace: config.node.job_lease_namespace.clone(),
                ..Default::default()
            },
        )
    } else {
        jobs::Jobs::new()
    }
    .with_fleet_jobs(config.node.fleet_jobs_enabled)
    .with_first_pass_now(config.node.jobs_first_pass_now);
    let market = Arc::new(shop::market::MarketplaceState::new(
        pool.clone(),
        http.clone(),
        market_settings(&config, &content::own_hosts(&config))?,
    ));
    market.spawn_index_jobs(&jobs);
    let land = Arc::new(land_authz::LandAuthzState::new(
        pool.clone(),
        http.clone(),
        land_settings(&config)?,
        Some(market.index.clone()),
    ));
    land.spawn_jobs(&jobs);
    let credits = Arc::new(shop::credits::State::new(
        pool.clone(),
        http,
        credits_settings(&config)?,
        shop::credits::Sources {
            market: Some(market),
            price: None,
            economy: None,
        },
        None,
    ));
    credits.spawn_usage_index(&jobs);
    tracing::info!(jobs = ?jobs.report().iter().map(|j| &j.name).collect::<Vec<_>>(), "index worker started");
    crate::serve::shutdown_signal().await;
    let aborted = jobs.shutdown(Duration::from_secs(30)).await;
    if !aborted.is_empty() {
        tracing::warn!(?aborted, "index jobs aborted at shutdown");
    }
    pool.close().await;
    Ok(())
}

pub(crate) fn market_settings(
    config: &Config,
    self_hosts: &[String],
) -> anyhow::Result<shop::market::Settings> {
    Ok(shop::market::Settings::from_config(
        config,
        &shop::market::Upstreams {
            eth_collections_subgraph_url: config
                .market_upstreams
                .eth_collections_subgraph_url
                .clone(),
            matic_collections_subgraph_url: config
                .market_upstreams
                .matic_collections_subgraph_url
                .clone(),
            marketplace_subgraph_url: config.market_upstreams.marketplace_subgraph_url.clone(),
            land_subgraph_url: config.market_upstreams.land_subgraph_url.clone(),
            trades_sync_upstream_url: config.market_upstreams.trades_sync_upstream_url.clone(),
            trades_sync_interval_secs: config.market_upstreams.trades_sync_interval_secs,
            trades_sync_pages_per_run: config.market_upstreams.trades_sync_pages_per_run,
            trades_serve_partial: config.market_upstreams.trades_serve_partial,
            profile_cdn_base_url: config.market_upstreams.profile_cdn_base_url.clone(),
        },
    )?
    .with_market(
        shop::market::MarketSettings::parse(
            config.marketplace.marketplace_mirrors_enabled,
            config.marketplace.marketplace_mirror_interval_secs,
            config.marketplace.marketplace_mirror_pages_per_tick,
            config.marketplace.trade_rpc_urls.as_deref(),
            Config::non_empty(&config.marketplace.market_admin_token),
            config.marketplace.marketplace_catalog_cache_ttl_secs,
            &config.social.marketplace_base_url,
            self_hosts,
        )
        .context("MARKETPLACE_* / TRADE_RPC_URLS configuration")?
        .with_indexes(shop::market::chain_walk::IndexSettings {
            nft_enabled: config.marketplace.marketplace_nft_index_enabled,
            mana_transfers_enabled: config.marketplace.marketplace_mana_transfers_index_enabled,
            trade_events_enabled: config.marketplace.marketplace_trade_events_index_enabled,
            interval: Duration::from_secs(
                config.marketplace.marketplace_index_interval_secs.max(1),
            ),
            requests_per_pass: config
                .marketplace
                .marketplace_index_requests_per_pass
                .max(1),
            window_blocks: config.marketplace.marketplace_index_window_blocks.max(1),
            address_chunk: config.marketplace.marketplace_index_address_chunk.max(1),
            start_block_ethereum: config.marketplace.marketplace_index_start_block_ethereum,
            start_block_matic: config.marketplace.marketplace_index_start_block_matic,
            stop_block_ethereum: config.marketplace.marketplace_index_stop_block_ethereum,
            stop_block_matic: config.marketplace.marketplace_index_stop_block_matic,
            neighbours_interval: Duration::from_secs(
                config.marketplace.marketplace_neighbours_interval_secs,
            ),
            coupon_refresh_interval: Duration::from_secs(
                config.marketplace.marketplace_coupon_refresh_interval_secs,
            ),
        }),
    ))
}

pub(crate) fn land_settings(config: &Config) -> anyhow::Result<land_authz::Settings> {
    let mut land_settings =
        land_authz::Settings::for_network(&config.node.eth_network, config.content.eth_rpc_url()?)
            .with_context(|| {
                format!(
                    "ETH_NETWORK={} names no LAND/Estate registries (mainnet|sepolia)",
                    config.node.eth_network
                )
            })?;
    land_settings.index_enabled = config.land_authz.land_authz_index_enabled;
    land_settings.interval =
        Duration::from_secs(config.land_authz.land_authz_index_interval_secs.max(30));
    land_settings.requests_per_pass = config.land_authz.land_authz_index_requests_per_pass.max(1);
    land_settings.replay_from_block = config.land_authz.land_authz_replay_from_block;
    land_settings.stop_block = config.land_authz.land_authz_stop_block;
    Ok(land_settings)
}

pub(crate) fn credits_settings(config: &Config) -> anyhow::Result<shop::credits::Settings> {
    let non_empty = Config::non_empty;
    shop::credits::Settings::parse(shop::credits::RawSettings {
        writes_enabled: config.ops.ops_writes_enabled,
        admin_token: non_empty(&config.shop.catalyrst_credits_admin_token),
        captcha_secret: non_empty(&config.shop.credits_captcha_secret),
        captcha_verify_url: config.shop.credits_captcha_verify_url.clone(),
        stripe_secret_key: non_empty(&config.shop.stripe_secret_key),
        stripe_webhook_secret: non_empty(&config.shop.stripe_webhook_secret),
        stripe_api_base: config.shop.stripe_api_base.clone(),
        credits_currency: config.shop.credits_currency.clone(),
        marketplace_markup_bps: config.shop.marketplace_markup_bps,
        mana_price_max_staleness_secs: {
            if let Some(warning) = config.price_staleness_warning() {
                tracing::warn!("{warning}");
            }
            config.shop.mana_price_max_staleness_secs
        },
        checkout_fulfillment_mode: config.shop.checkout_fulfillment_mode.clone(),
        require_purchase_intent: config.shop.credits_require_purchase_intent,
        checkout_worker_interval_secs: config.shop.checkout_worker_interval_secs,
        checkout_max_attempts: config.shop.checkout_max_attempts,
        escrow_lock_days: config.shop.escrow_lock_days,
        mock_fulfillment: config.shop.credits_mock_fulfillment,
        mock_card: config.shop.credits_mock_card,
        credits_signer_key: non_empty(&config.shop.credits_signer_key),
        credits_manager_contract: non_empty(&config.shop.credits_manager_contract),
        credits_onchain_authorize_enabled: config.shop.credits_onchain_authorize_enabled,
        checkout_success_url: config.shop.credits_checkout_success_url.clone(),
        checkout_cancel_url: config.shop.credits_checkout_cancel_url.clone(),
        checkout_success_url_builder: non_empty(&config.shop.credits_checkout_success_url_builder),
        checkout_cancel_url_builder: non_empty(&config.shop.credits_checkout_cancel_url_builder),
        usage_index_enabled: config.shop.credits_usage_index_enabled,
        usage_rpc_url: non_empty(&config.economy.rpc_endpoint_polygon),
        usage_chain_id: config.economy.collections_chain_id as i64,
        usage_start_block: config.shop.credits_usage_start_block,
        usage_interval_secs: config.shop.credits_usage_index_interval_secs,
        usage_requests_per_pass: config.shop.credits_usage_index_requests_per_pass,
        usage_replay_from_block: config.shop.credits_usage_replay_from_block,
        usage_stop_block: config.shop.credits_usage_stop_block,
        credit_ttl_seconds: config.shop.credits_ttl_seconds,
        credit_cap_headroom_pct: config.shop.credits_cap_headroom_pct,
        credit_buffer_bps: config.shop.credits_oracle_buffer_bps,
        credits_revoker_key: non_empty(&config.shop.credits_revoker_key),
    })
    .context("CREDITS_* / STRIPE_* / CHECKOUT_* configuration")
}
