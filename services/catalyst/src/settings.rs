//! Settings shared by the HTTP server and the indexing worker.
use crate::{config::Config, jobs, land_authz, shop};
use anyhow::Context;
use std::time::Duration;

/// The job runner `serve` and `index` both build: leased in
/// `JOB_LEASE_NAMESPACE` unless `JOB_LEASES_ENABLED=false`, standing fleet
/// jobs down on an API replica (`FLEET_JOBS_ENABLED=false`). One builder
/// keeps the two processes, which contend for the same leases, on the same
/// lease settings.
pub(crate) fn jobs_from_config(config: &Config, pool: &sqlx::PgPool) -> jobs::Jobs {
    let jobs = if config.node.job_leases_enabled {
        jobs::Jobs::with_leases(
            pool.clone(),
            jobs::LeaseSettings {
                namespace: config.node.job_lease_namespace.clone(),
                ..jobs::LeaseSettings::default()
            },
        )
    } else {
        jobs::Jobs::new()
    };
    jobs.with_fleet_jobs(config.node.fleet_jobs_enabled)
        .with_first_pass_now(config.node.jobs_first_pass_now)
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
            trades_sync_upstream_url: config.market_upstreams.trades_sync_upstream_url.clone(),
            trades_sync_interval_secs: config.market_upstreams.trades_sync_interval_secs,
            trades_sync_pages_per_run: config.market_upstreams.trades_sync_pages_per_run,
            trades_serve_partial: config.market_upstreams.trades_serve_partial,
            profile_cdn_base_url: config.market_upstreams.profile_cdn_base_url.clone(),
            nft_worker_base_url: config.market_upstreams.nft_worker_base_url.clone(),
            explorer_escrow_grants_enabled: config.market_upstreams.explorer_escrow_grants_enabled,
            lambdas_catalog_extensions_enabled: config
                .market_upstreams
                .lambdas_catalog_extensions_enabled,
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
        .with_historical_admin_token(Config::non_empty(
            &config.marketplace.historical_admin_token,
        ))
        .with_wert(
            shop::market::wert::WertSettings::parse(
                config.marketplace.wert_private_key.as_deref(),
                config
                    .marketplace
                    .wert_publication_fees_private_key
                    .as_deref(),
                config.marketplace.wert_api_url.as_deref(),
                config.marketplace.wert_api_key.as_deref(),
                self_hosts,
            )
            .context("WERT_* configuration")?,
        )
        .with_indexes(checked_indexes(shop::market::chain_walk::IndexSettings {
            nft_enabled: config.marketplace.marketplace_nft_index_enabled,
            mana_transfers_enabled: config.marketplace.marketplace_mana_transfers_index_enabled,
            trade_events_enabled: config.marketplace.marketplace_trade_events_index_enabled,
            sales_enabled: config.marketplace.marketplace_sales_index_enabled,
            committee_enabled: config.marketplace.marketplace_committee_index_enabled,
            third_party_registry_enabled: config
                .marketplace
                .marketplace_third_party_registry_index_enabled,
            collections_enabled: config.marketplace.marketplace_collections_index_enabled,
            collections_owned: collections_owned(
                &config.marketplace.marketplace_collections_index_owns,
            )?,
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
            chain_id_ethereum: config.marketplace.marketplace_index_chain_id_ethereum,
            chain_id_matic: config.marketplace.marketplace_index_chain_id_matic,
        })?)
        .with_portal(shop::market::chain_jobs::PortalSettings::parse(
            &config.marketplace.chain_logs_source,
            config.marketplace.chain_logs_portal_url.as_deref(),
            &config.marketplace.chain_logs_portal_dataset_ethereum,
            &config.marketplace.chain_logs_portal_dataset_matic,
            config.marketplace.chain_logs_portal_api_key.clone(),
            config.marketplace.chain_logs_portal_max_rps,
            config.marketplace.chain_logs_portal_assert_full_history,
            config.marketplace.chain_logs_portal_deadline_secs,
        )?)
        .with_tips(shop::market::tips::TipSettings::parse(
            &config.marketplace.marketplace_tip_min_amount_wei,
            config.marketplace.marketplace_tip_max_age_secs,
            config.marketplace.marketplace_tip_max_pending,
        )?)
        .with_trusted_proxies(config.node.http_trusted_proxy_cidrs.clone()),
    ))
}

fn checked_indexes(
    indexes: shop::market::chain_walk::IndexSettings,
) -> anyhow::Result<shop::market::chain_walk::IndexSettings> {
    indexes
        .check_supported_chain_ids()
        .map_err(anyhow::Error::msg)?;
    Ok(indexes)
}

/// `MARKETPLACE_COLLECTIONS_INDEX_OWNS`: the networks the collections index
/// takes from the mirror. It walks Polygon alone.
fn collections_owned(value: &str) -> anyhow::Result<Vec<shop::market::chain_walk::ChainNet>> {
    use shop::market::chain_walk::ChainNet;
    let mut out = Vec::new();
    for name in value.split(',').map(str::trim).filter(|n| !n.is_empty()) {
        match ChainNet::parse(name) {
            Some(ChainNet::Matic) => out.push(ChainNet::Matic),
            Some(ChainNet::Ethereum) => anyhow::bail!(
                "MARKETPLACE_COLLECTIONS_INDEX_OWNS names {name}, but the collections index walks Polygon alone"
            ),
            None => anyhow::bail!(
                "MARKETPLACE_COLLECTIONS_INDEX_OWNS names an unknown network {name:?}"
            ),
        }
    }
    out.dedup();
    Ok(out)
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
    let configured_interval = config.land_authz.land_authz_index_interval_secs;
    let interval = configured_interval.clamp(30, 240);
    if land_settings.index_enabled && configured_interval > interval {
        tracing::warn!(
            configured_interval_secs = configured_interval,
            effective_interval_secs = interval,
            "clamping LAND_AUTHZ_INDEX_INTERVAL_SECS to leave retry time within the public LAND freshness window"
        );
    }
    land_settings.interval = Duration::from_secs(interval);
    land_settings.requests_per_pass = config.land_authz.land_authz_index_requests_per_pass.max(1);
    land_settings.replay_from_block = config.land_authz.land_authz_replay_from_block;
    land_settings.stop_block = config.land_authz.land_authz_stop_block;
    land_settings.confirmations = config.land_authz.land_authz_confirmations;
    Ok(land_settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_collections_index_owns_polygon_alone() {
        use shop::market::chain_walk::ChainNet;
        assert_eq!(collections_owned("").unwrap(), Vec::<ChainNet>::new());
        assert_eq!(
            collections_owned(" matic , polygon ").unwrap(),
            vec![ChainNet::Matic]
        );
        let err = collections_owned("ethereum").unwrap_err().to_string();
        assert!(err.contains("walks Polygon alone"), "{err}");
        let err = collections_owned("base").unwrap_err().to_string();
        assert!(err.contains("unknown network"), "{err}");
    }

    #[test]
    fn shared_land_settings_keep_limits_and_replay_bounds() {
        let mut config =
            Config::parse_env_for_tests([("DATABASE_URL", "postgres:///catalyst")]).unwrap();
        config.land_authz.land_authz_index_interval_secs = 0;
        config.land_authz.land_authz_index_requests_per_pass = 0;
        config.land_authz.land_authz_replay_from_block = Some(42);
        config.land_authz.land_authz_stop_block = Some(99);
        config.land_authz.land_authz_confirmations = 3;
        let settings = land_settings(&config).unwrap();
        assert_eq!(settings.interval, Duration::from_secs(30));
        assert_eq!(settings.requests_per_pass, 1);
        assert_eq!(settings.replay_from_block, Some(42));
        assert_eq!(settings.stop_block, Some(99));
        assert_eq!(settings.confirmations, 3);
    }

    #[test]
    fn land_interval_above_the_public_limit_is_clamped_without_failing_startup() {
        for enabled in ["true", "false"] {
            let config = Config::parse_env_for_tests([
                ("DATABASE_URL", "postgres:///catalyst"),
                ("LAND_AUTHZ_INDEX_ENABLED", enabled),
                ("LAND_AUTHZ_INDEX_INTERVAL_SECS", "300"),
            ])
            .unwrap();
            let settings = land_settings(&config).unwrap();
            assert_eq!(settings.index_enabled, enabled == "true");
            assert_eq!(settings.interval, Duration::from_secs(240));
        }
    }

    #[tokio::test]
    async fn testnet_mirror_schedules_only_projections_for_its_catalogue_pair() {
        use catalyst_config::posture::{Posture, expand};
        use shop::market::{MarketplaceState, chain_jobs, chain_walk::ChainNet};
        use std::{collections::HashMap, sync::Arc};

        let mut vars = HashMap::from([
            ("ETH_NETWORK", "sepolia".to_owned()),
            ("DATABASE_URL", "postgres:///catalyst".to_owned()),
            ("MARKETPLACE_INDEX_CHAIN_ID_ETHEREUM", "11155111".to_owned()),
            ("MARKETPLACE_INDEX_CHAIN_ID_MATIC", "80002".to_owned()),
            ("MARKETPLACE_MANA_TRANSFERS_INDEX_ENABLED", "true".to_owned()),
            ("MARKETPLACE_COLLECTIONS_INDEX_ENABLED", "true".to_owned()),
            ("TRADE_RPC_URLS", "1=https://eth.example,137=https://polygon.example,11155111=https://sepolia.example,80002=https://amoy.example".to_owned()),
        ]);
        let posture = expand(Posture::Mirror, &|key| vars.get(key).cloned());
        vars.extend(posture);
        let config = Config::parse_env_for_tests(vars).unwrap();
        let settings = market_settings(&config, &[]).unwrap();
        let indexes = &settings.market.indexes;
        assert!(
            indexes.sales_enabled
                && indexes.committee_enabled
                && indexes.third_party_registry_enabled
        );
        let pool = sqlx::PgPool::connect_lazy("postgres:///catalyst").unwrap();
        let state = Arc::new(MarketplaceState::new(
            pool,
            crate::http::Outbound::for_tests(),
            settings,
        ));
        for (net, chain) in [(ChainNet::Ethereum, 11_155_111), (ChainNet::Matic, 80_002)] {
            let scheduled = chain_jobs::projections(&state, net, Some(false));
            let names: Vec<_> = scheduled
                .iter()
                .map(|job| {
                    let index = (job.make)();
                    assert_eq!(index.chain_id(), chain);
                    index.indexer()
                })
                .collect();
            let expected: &[&str] = match net {
                ChainNet::Ethereum => &["nft", "sales", "trade_events"],
                ChainNet::Matic => &["nft", "sales", "trade_events"],
            };
            assert_eq!(names, expected);
        }
    }

    #[test]
    fn only_catalogue_writers_require_eth_network_to_match_the_pair() {
        for switch in [
            "MARKETPLACE_NFT_INDEX_ENABLED",
            "MARKETPLACE_MIRRORS_ENABLED",
            "MARKETPLACE_SALES_INDEX_ENABLED",
        ] {
            for (network, ethereum, matic) in [
                ("sepolia", "1", "137"),
                ("mainnet", "11155111", "80002"),
                ("unsupported", "1", "137"),
                ("sepolia", "11155111", "137"),
            ] {
                let config = Config::parse_env_for_tests([
                    ("DATABASE_URL", "postgres:///catalyst"),
                    ("ETH_NETWORK", network),
                    ("MARKETPLACE_INDEX_CHAIN_ID_ETHEREUM", ethereum),
                    ("MARKETPLACE_INDEX_CHAIN_ID_MATIC", matic),
                    (switch, "true"),
                ])
                .unwrap();
                let settings = market_settings(&config, &[]).expect("API settings");
                assert!(
                    settings.check_writer_pair().is_err(),
                    "{network} {ethereum}/{matic}"
                );
            }
        }
    }

    #[test]
    fn shared_market_settings_reject_unsupported_chain_ids() {
        let mut config =
            Config::parse_env_for_tests([("DATABASE_URL", "postgres:///catalyst")]).unwrap();
        config.marketplace.marketplace_index_chain_id_ethereum = 999;
        let error = market_settings(&config, &[]).expect_err("unsupported chain");
        assert!(
            error
                .to_string()
                .contains("MARKETPLACE_INDEX_CHAIN_ID_ETHEREUM=999")
        );
    }
}
