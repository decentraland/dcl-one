//! Observe durable cursors independently of the workers that advance them.
use crate::chain_index::{
    logs::{LogSource, RpcSource},
    rpc::Rpc,
};
use crate::shop::market::{MarketplaceState, chain_jobs, chain_walk::ChainNet, mirrors, trades};
use crate::{
    comms, http,
    jobs::{Jobs, PeriodicCfg},
    land_authz,
};
use anyhow::Context;
use chrono::{NaiveDateTime, Utc};
use futures::future::join_all;
use sqlx::PgPool;
use std::{
    collections::{BTreeSet, HashMap},
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};

pub(super) fn install() -> anyhow::Result<bool> {
    let Some(raw) = std::env::var_os("INDEX_METRICS_BIND") else {
        return Ok(false);
    };
    let bind: SocketAddr = raw
        .to_str()
        .context("INDEX_METRICS_BIND must be text")?
        .parse()
        .context("invalid INDEX_METRICS_BIND")?;
    anyhow::ensure!(
        bind.ip().is_loopback(),
        "INDEX_METRICS_BIND must use a loopback address"
    );
    comms::metrics::install_prometheus_exporter(bind)?;
    tracing::info!(%bind, "index /metrics listening");
    Ok(true)
}

struct Target {
    source: &'static str,
    network: String,
}

impl Target {
    fn set(&self, name: &'static str, value: f64) {
        metrics::gauge!(name, "source" => self.source, "network" => self.network.clone())
            .set(value);
    }
}

struct CursorTarget {
    target: Target,
    url: Option<String>,
    publication_expected: bool,
    chain_id: u64,
    lag_limit: u64,
}

struct MirrorTarget {
    target: Target,
    url: String,
    max_age: Duration,
}

struct Monitor {
    land_registry: String,
    estate_registry: String,
    cursors: Vec<CursorTarget>,
    mirrors: Vec<MirrorTarget>,
}

pub(super) fn spawn(
    jobs: &Jobs,
    market: &Arc<MarketplaceState>,
    land: &land_authz::Settings,
    rentals: Option<bool>,
) {
    let monitor = Arc::new(Monitor::configured(market, land, rentals));
    let (pool, http) = (market.pool.clone(), market.http.clone());
    jobs.spawn_periodic(
        "index-metrics",
        PeriodicCfg::every(Duration::from_secs(60)),
        move |_| {
            let (monitor, pool, http) = (monitor.clone(), pool.clone(), http.clone());
            async move { monitor.sample(&pool, &http).await }
        },
    );
}

impl Monitor {
    fn configured(
        market: &Arc<MarketplaceState>,
        land: &land_authz::Settings,
        rentals: Option<bool>,
    ) -> Self {
        let mut cursors = Vec::new();
        for net in ChainNet::ALL {
            for (index, url) in chain_jobs::expected_cursors(market, net, rentals) {
                let settings = &market.settings.market.indexes;
                cursors.push(CursorTarget {
                    target: Target {
                        source: index.indexer(),
                        network: net.db_name().into(),
                    },
                    url,
                    publication_expected: settings.stop_block(net).is_none()
                        && settings
                            .start_override(net)
                            .is_none_or(|start| start <= index.start_block()),
                    chain_id: index.chain_id() as u64,
                    lag_limit: net.confirmations() + net.publish_lag(),
                });
            }
        }
        if land.index_enabled {
            cursors.push(CursorTarget {
                target: Target {
                    source: "land_authz",
                    network: land.network.clone(),
                },
                url: land.rpc_url.clone(),
                publication_expected: land.stop_block.is_none(),
                chain_id: if land.network == crate::chain_index::registries::NETWORK_SEPOLIA {
                    11_155_111
                } else {
                    1
                },
                lag_limit: land
                    .confirmations
                    .saturating_add(land_authz::indexer::HEAD_LAG_BLOCKS),
            });
        }
        let settings = &market.settings;
        let mut mirrors = Vec::new();
        if settings.market.mirrors_enabled {
            for net in settings.collection_networks() {
                if let Some(url) = net.url(&market.index) {
                    for mirror in [mirrors::Mirror::Collections, mirrors::Mirror::Items] {
                        mirrors.push(MirrorTarget {
                            target: Target {
                                source: mirror.name(),
                                network: net.db_name().into(),
                            },
                            url: url.into(),
                            max_age: mirrors::stale_after(settings, mirror),
                        });
                    }
                }
            }
        }
        if let Some(url) = &settings.trades_sync_upstream_url {
            mirrors.push(MirrorTarget {
                target: Target {
                    source: "trades",
                    network: "offchain".into(),
                },
                url: url.clone(),
                max_age: trades::max_age(settings.trades_sync_interval),
            });
        }
        Self {
            cursors,
            mirrors,
            land_registry: land.land_registry.clone(),
            estate_registry: land.estate_registry.clone(),
        }
    }
}

// The schema bounds both reads by the number of source/network pairs.
#[derive(sqlx::FromRow)]
struct Cursor {
    source: String,
    network: String,
    last_block: i64,
    chain_id: Option<i64>,
    folded_block: Option<i64>,
    folded_at: Option<NaiveDateTime>,
    quarantined: bool,
}

#[derive(sqlx::FromRow)]
struct Mirror {
    source: String,
    network: String,
    url: String,
    completed: Option<NaiveDateTime>,
    pages: i64,
}

impl Monitor {
    async fn sample(&self, pool: &PgPool, http: &http::Outbound) -> anyhow::Result<()> {
        // Projections may share a network label while using separate RPCs or chain IDs.
        let urls: BTreeSet<_> = self
            .cursors
            .iter()
            .filter_map(|c| c.url.as_deref())
            .collect();
        let heads = join_all(urls.into_iter().map(|url| async move {
            let source = RpcSource::new(Rpc::at(http, url));
            let probe = async {
                let chain = source.chain_id().await.ok()?.value;
                Some((chain, source.head().await.ok()?.value))
            };
            (
                url,
                tokio::time::timeout(Duration::from_secs(10), probe)
                    .await
                    .ok()
                    .flatten(),
            )
        }))
        .await
        .into_iter()
        .collect::<HashMap<_, _>>();

        let cursors = sqlx::query_as::<_, Cursor>(
            "SELECT indexer AS source, network, last_block, folded_block, folded_at, chain_id,
                    quarantined_at IS NOT NULL AS quarantined FROM shop.chain_cursor
             UNION ALL SELECT 'land_authz', network, last_block, folded_block, folded_at,
                    NULL::bigint, false FROM land_authz.index_cursor WHERE land_registry = $1 AND estate_registry = $2",
        )
        .bind(&self.land_registry).bind(&self.estate_registry)
        .fetch_all(pool)
        .await?;
        let mirrors = sqlx::query_as::<_, Mirror>(
            "SELECT mirror AS source, network, source_url AS url, last_complete_pass_at AS completed,
                    pages_fetched AS pages FROM shop.mirror_state
             UNION ALL SELECT 'trades', 'offchain', upstream_url, completed_at, pages_fetched
                    FROM shop.trades_mirror WHERE id = 1"
        ).fetch_all(pool).await?;
        let now = Utc::now().naive_utc();
        for c in &self.cursors {
            let row = cursors
                .iter()
                .find(|r| r.source == c.target.source && r.network == c.target.network);
            let head = c
                .url
                .as_deref()
                .and_then(|url| heads[url])
                .filter(|(id, _)| *id == c.chain_id)
                .map(|(_, h)| h);
            c.report(row, head, now);
        }
        for m in &self.mirrors {
            let row = mirrors.iter().find(|r| {
                r.source == m.target.source && r.network == m.target.network && r.url == m.url
            });
            m.report(row, now);
        }
        metrics::gauge!("catalyst_index_sample_timestamp_seconds")
            .set(now.and_utc().timestamp() as f64);
        Ok(())
    }
}

fn age(now: NaiveDateTime, at: Option<NaiveDateTime>) -> f64 {
    at.map_or(f64::NAN, |at| (now - at).num_seconds().max(0) as f64)
}

impl CursorTarget {
    fn report(&self, row: Option<&Cursor>, head: Option<u64>, now: NaiveDateTime) {
        let t = &self.target;
        let lag = head
            .zip(row)
            .filter(|(_, r)| {
                self.target.source == "land_authz" || r.chain_id == Some(self.chain_id as i64)
            })
            .map(|(h, r)| (h as i128 - i128::from(r.last_block)).max(0) as f64);
        t.set("catalyst_index_source_observed", f64::from(lag.is_some()));
        t.set("catalyst_index_lag_limit_blocks", self.lag_limit as f64);
        t.set(
            "catalyst_index_publication_expected",
            f64::from(self.publication_expected),
        );
        t.set("catalyst_index_lag_blocks", lag.unwrap_or(f64::NAN));
        t.set(
            "catalyst_index_last_block",
            row.map_or(f64::NAN, |r| r.last_block as f64),
        );
        t.set(
            "catalyst_index_folded_block",
            row.and_then(|r| r.folded_block)
                .map_or(f64::NAN, |b| b as f64),
        );
        t.set(
            "catalyst_index_fold_age_seconds",
            age(now, row.and_then(|r| r.folded_at)),
        );
        t.set(
            "catalyst_index_quarantined",
            row.map_or(f64::NAN, |r| f64::from(r.quarantined)),
        );
    }
}

impl MirrorTarget {
    fn report(&self, row: Option<&Mirror>, now: NaiveDateTime) {
        let t = &self.target;
        t.set("catalyst_index_source_observed", f64::from(row.is_some()));
        t.set(
            "catalyst_index_mirror_age_limit_seconds",
            self.max_age.as_secs_f64(),
        );
        t.set(
            "catalyst_index_mirror_age_seconds",
            age(now, row.and_then(|r| r.completed)),
        );
        t.set(
            "catalyst_index_mirror_pages",
            row.map_or(f64::NAN, |r| r.pages as f64),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Json, Router, routing::post};
    use serde_json::{Value, json};
    use std::sync::atomic::{AtomicU64, Ordering};

    fn gauge(text: &str, name: &str, source: &str, network: &str) -> f64 {
        let labels = [
            format!("source=\"{source}\""),
            format!("network=\"{network}\""),
        ];
        let prefix = format!("catalyst_index_{name}{{");
        let line = text
            .lines()
            .find(|l| l.starts_with(&prefix) && labels.iter().all(|k| l.contains(k)))
            .unwrap_or_else(|| panic!("missing {name} for {source}/{network}"));
        line.split_whitespace().last().unwrap().parse().unwrap()
    }

    #[tokio::test]
    async fn samples_durable_progress_while_workers_are_stopped() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        sqlx::query("INSERT INTO shop.chain_cursor
            (indexer, network, start_block, last_block, folded_block, folded_at, full_history, chain_id, quarantined_at, quarantine_reason)
            VALUES ('nft','ethereum',0,88,88,timezone('UTC',now()) - interval '3 hours',true,1,NULL,NULL),
                   ('rentals','ethereum',0,20,NULL,NULL,true,1,timezone('UTC',now()),'unproven')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO land_authz.index_cursor (network, land_registry, estate_registry, last_block)
            VALUES ('ethereum-mainnet','land','estate',35)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO shop.mirror_state (mirror,network,source_url,last_complete_pass_at,pages_fetched)
            VALUES ('items','ethereum','fixture',timezone('UTC',now()) - interval '1 hour',7)").execute(&pool).await.unwrap();
        let height = Arc::new(AtomicU64::new(100));
        let tip = height.clone();
        let rpc = Router::new().route(
            "/",
            post(move |Json(req): Json<Value>| {
                let block = tip.load(Ordering::Relaxed);
                async move {
                    let result = match req["method"].as_str().unwrap() {
                        "eth_chainId" => "0x1".into(),
                        "eth_blockNumber" => format!("0x{block:x}"),
                        other => panic!("unexpected method {other}"),
                    };
                    Json(json!({"jsonrpc":"2.0","id":req["id"],"result":result}))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, rpc).await.unwrap() });
        let http = http::Outbound::for_tests();
        let mut settings = crate::shop::market::Settings::for_tests();
        settings.eth_rpc_url = Some(url.clone());
        settings.eth_collections_subgraph_url = Some("fixture".into());
        settings.market.mirrors_enabled = true;
        settings.market.indexes.nft_enabled = true;
        settings.market.indexes.sales_enabled = true;
        let market = Arc::new(MarketplaceState::new(pool.clone(), http.clone(), settings));
        let mut land = land_authz::Settings::for_tests();
        land.index_enabled = true;
        land.land_registry = "land".into();
        land.estate_registry = "estate".into();
        let without_land_rpc = Monitor::configured(&market, &land, Some(false));
        land.rpc_url = Some(url);
        let monitor = Monitor::configured(&market, &land, Some(true));
        let recorder = comms::metrics::prometheus_builder()
            .unwrap()
            .build_recorder();
        let handle = recorder.handle();
        let _recording = metrics::set_default_local_recorder(&recorder);
        without_land_rpc.sample(&pool, &http).await.unwrap();
        let incomplete = handle.render();
        assert_eq!(
            gauge(
                &incomplete,
                "source_observed",
                "land_authz",
                "ethereum-mainnet"
            ),
            0.0
        );
        assert_eq!(
            gauge(&incomplete, "source_observed", "nft", "ethereum"),
            1.0
        );
        assert!(!incomplete.contains("source=\"rentals\""));
        assert!(!incomplete.contains("source=\"committee\""));
        monitor.sample(&pool, &http).await.unwrap();
        let text = handle.render();
        assert_eq!(gauge(&text, "lag_blocks", "nft", "ethereum"), 12.0);
        assert_eq!(gauge(&text, "quarantined", "nft", "ethereum"), 0.0);
        assert_eq!(gauge(&text, "folded_block", "nft", "ethereum"), 88.0);
        assert!(gauge(&text, "fold_age_seconds", "nft", "ethereum") >= 10800.0);
        assert_eq!(
            gauge(&text, "lag_blocks", "land_authz", "ethereum-mainnet"),
            65.0
        );
        assert_eq!(gauge(&text, "quarantined", "rentals", "ethereum"), 1.0);
        assert!(gauge(&text, "folded_block", "rentals", "ethereum").is_nan());
        assert_eq!(gauge(&text, "source_observed", "sales", "matic"), 0.0);
        assert!(gauge(&text, "mirror_age_seconds", "items", "ethereum") >= 3600.0);
        assert_eq!(gauge(&text, "mirror_pages", "items", "ethereum"), 7.0);
        sqlx::query(
            "UPDATE shop.chain_cursor SET folded_at = timezone('UTC',now()) WHERE indexer = 'nft'",
        )
        .execute(&pool)
        .await
        .unwrap();
        monitor.sample(&pool, &http).await.unwrap();
        let healthy = handle.render();
        assert_eq!(gauge(&healthy, "source_observed", "nft", "ethereum"), 1.0);
        assert!(gauge(&healthy, "fold_age_seconds", "nft", "ethereum") < 60.0);
        sqlx::query("UPDATE land_authz.index_cursor SET estate_registry = 'other', last_block=99, folded_block=99, folded_at=timezone('UTC',now())")
            .execute(&pool).await.unwrap();
        monitor.sample(&pool, &http).await.unwrap();
        let wrong_land = handle.render();
        assert_eq!(
            gauge(
                &wrong_land,
                "source_observed",
                "land_authz",
                "ethereum-mainnet"
            ),
            0.0
        );
        assert!(
            gauge(
                &wrong_land,
                "folded_block",
                "land_authz",
                "ethereum-mainnet"
            )
            .is_nan()
        );
        height.store(200, Ordering::Relaxed);
        monitor.sample(&pool, &http).await.unwrap();
        let stalled = handle.render();
        assert_eq!(gauge(&stalled, "last_block", "nft", "ethereum"), 88.0);
        assert_eq!(gauge(&stalled, "lag_blocks", "nft", "ethereum"), 112.0);

        sqlx::query("UPDATE shop.chain_cursor SET chain_id = 11155111 WHERE indexer = 'nft'")
            .execute(&pool)
            .await
            .unwrap();
        monitor.sample(&pool, &http).await.unwrap();
        assert!(gauge(&handle.render(), "lag_blocks", "nft", "ethereum").is_nan());
        sqlx::query("DELETE FROM shop.mirror_state")
            .execute(&pool)
            .await
            .unwrap();
        server.abort();
        let _ = server.await;
        monitor.sample(&pool, &http).await.unwrap();
        let missing = handle.render();
        assert_eq!(gauge(&missing, "source_observed", "items", "ethereum"), 0.0);
        assert!(gauge(&missing, "mirror_age_seconds", "items", "ethereum").is_nan());
        assert_eq!(gauge(&missing, "source_observed", "nft", "ethereum"), 0.0);
        assert!(gauge(&missing, "lag_blocks", "nft", "ethereum").is_nan());
        assert_eq!(gauge(&missing, "quarantined", "rentals", "ethereum"), 1.0);
    }
}
