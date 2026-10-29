//! The sync worker as `serve` attaches it: [`SyncSettings`] validates the
//! `SYNC_*` configuration once at startup, [`start`] builds the write path,
//! the orchestrator and the retry/prune loops on the ONE pool, ONE storage
//! root and ONE HTTP client, and registers them with the job registry. The
//! orchestrator owns the Bootstrapping -> Syncing transition from there.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use sqlx::PgPool;
use tokio::sync::RwLock;

use crate::batch_deployer::{BatchDeployer, BatchDeployerConfig};
use crate::bloom_filter::BloomFilter;
use crate::config::Config;
use crate::jobs::{JobContext, Jobs};
use crate::storage::Storage;
use crate::sync::orchestrator::{SyncControl, SyncDependencies, run_sync_job};
use crate::sync::retry_failed::run_prune_loop;
use crate::sync::snapshots::SnapshotScheduling;
use crate::sync::status_mirror;
use crate::sync::{
    LiveDeploymentRepository, LiveProcessedSnapshotStore, RetryFailedConfig,
    RetryFailedDeployments, SyncOrchestrator, SyncOrchestratorConfig, SyncState, SyncStatus,
    names_a_source, parse_entity_types, parse_sync_sources,
};
use crate::sync_deployer::{LiveFailedDeploymentsStore, LiveSyncDeployer, SyncMemoryLimits};

/// The memory limits `SyncSettings::from_config` validated, read back by [`start`]. They ride
/// beside `SyncSettings` rather than in it: the integration tests build the settings by
/// literal, and settings built by hand run with the defaults.
static MEMORY_LIMITS: parking_lot::Mutex<SyncMemoryLimits> =
    parking_lot::Mutex::new(SyncMemoryLimits::DEFAULT);

pub fn memory_limits() -> SyncMemoryLimits {
    *MEMORY_LIMITS.lock()
}

/// `SYNC_INFLIGHT_BYTES_MAX` and `SYNC_FLUSH_BACKLOG_MAX` as configured; `Err` on a zero, which
/// would stall every download or every producer for good.
pub fn memory_limits_from_config(config: &Config) -> anyhow::Result<SyncMemoryLimits> {
    if config.content.sync_inflight_bytes_max == 0 {
        anyhow::bail!("SYNC_INFLIGHT_BYTES_MAX must be at least 1 (0 would stall every download)");
    }
    if config.content.sync_flush_backlog_max == 0 {
        anyhow::bail!(
            "SYNC_FLUSH_BACKLOG_MAX must be at least 1 (0 would stall the worker forever)"
        );
    }
    Ok(SyncMemoryLimits {
        inflight_bytes_max: config.content.sync_inflight_bytes_max,
        flush_backlog_max: config.content.sync_flush_backlog_max,
    })
}

/// The peer-sync configuration after validation: peers parsed and checked
/// against this node's own URLs, the entity-type subset resolved. Sync needs
/// no RPC endpoint: contract-wallet links in synced chains are the peer's
/// verdict (`auth::PeerValidated`).
#[derive(Debug, Clone)]
pub struct SyncSettings {
    pub peers: Vec<String>,
    pub entity_types: Option<HashSet<String>>,
    pub from_timestamp_ms: i64,
    pub phased_sync: bool,
    pub content_download_concurrency: usize,
    pub connections_max_idle: usize,
    pub snapshot_concurrency: usize,
    pub snapshot_scheduling: SnapshotScheduling,
    pub flush_concurrency: usize,
    pub bloom_expected_elements: usize,
    pub profile_max_age_ms: i64,
    pub retry_failed_enabled: bool,
    pub max_retries: u32,
    pub retry_concurrency: usize,
    pub prune_ttl_days: i64,
}

impl SyncSettings {
    /// `None` when `SYNC_ENABLED` is off; `Err` on any setting that would make
    /// the worker unsafe to start (the old binary panicked on the same ones).
    pub fn from_config(config: &Config) -> anyhow::Result<Option<Self>> {
        if !config.node.sync_enabled {
            return Ok(None);
        }
        if config.node.serve_unsynced {
            anyhow::bail!(
                "SERVE_UNSYNCED and SYNC_ENABLED are exclusive: the sync worker reports \
                 readiness itself, and the override would forge it while bootstrapping"
            );
        }
        let own_urls: Vec<String> = [
            config.node.public_url.clone(),
            config.node.content_url.clone(),
            Some(format!("http://{}", config.node.bind_addr())),
        ]
        .into_iter()
        .flatten()
        .collect();
        let peers = parse_sync_sources(&config.content.sync_source, &own_urls)?;
        let entity_types = parse_entity_types(&config.content.sync_entity_types)?;
        for (name, value) in [
            (
                "CONCURRENT_SYNC_DOWNLOADS",
                config.content.concurrent_sync_downloads,
            ),
            (
                "SYNC_SNAPSHOT_CONCURRENCY",
                config.content.sync_snapshot_concurrency,
            ),
            (
                "SYNC_NONSCENE_CONCURRENCY",
                config.content.sync_nonscene_concurrency,
            ),
            (
                "SYNC_SCENE_CONCURRENCY",
                config.content.sync_scene_concurrency,
            ),
            (
                "SYNC_FLUSH_CONCURRENCY",
                config.content.sync_flush_concurrency,
            ),
            (
                "SYNC_RETRY_CONCURRENCY",
                config.content.sync_retry_concurrency,
            ),
        ] {
            if value == 0 {
                anyhow::bail!("{name} must be at least 1 (0 would stall the worker forever)");
            }
        }
        *MEMORY_LIMITS.lock() = memory_limits_from_config(config)?;
        Ok(Some(Self {
            peers,
            entity_types,
            from_timestamp_ms: config.content.sync_from_timestamp_ms,
            phased_sync: config.content.phased_sync,
            content_download_concurrency: config.content.concurrent_sync_downloads,
            connections_max_idle: config.content.connections_max_idle,
            snapshot_concurrency: config.content.sync_snapshot_concurrency,
            snapshot_scheduling: SnapshotScheduling {
                nonscene_concurrency: config.content.sync_nonscene_concurrency,
                scene_concurrency: config.content.sync_scene_concurrency,
            },
            flush_concurrency: config.content.sync_flush_concurrency,
            bloom_expected_elements: config.content.bloom_filter_expected_elements,
            profile_max_age_ms: match config.content.sync_profile_max_age_days {
                0 => i64::MAX,
                days => i64::from(days) * 86_400_000,
            },
            retry_failed_enabled: config.content.retry_failed_enabled,
            max_retries: config.content.max_failed_deployment_retries,
            retry_concurrency: config.content.sync_retry_concurrency,
            prune_ttl_days: config.content.retry_failed_prune_ttl_days,
        }))
    }
}

/// `SERVE_UNSYNCED` with no worker attached: the process claims `Syncing` so
/// the content read contracts replay against whatever the database holds.
/// While `SYNC_SOURCE` names a peer, that is an unsynced copy of the peer and
/// deploys keep the bootstrapping refusal, except for the entity types
/// `SERVE_UNSYNCED_DEPLOY_TYPES` names; with no source at all the node is its
/// own authority and takes them.
pub fn serve_unsynced(config: &Config, status: &SyncStatus) {
    let copy = names_a_source(&config.content.sync_source);
    let types: Vec<String> = config
        .node
        .serve_unsynced_deploy_types
        .split(',')
        .map(|t| t.trim().to_ascii_lowercase())
        .filter(|t| !t.is_empty())
        .collect();
    if copy {
        tracing::warn!(
            takes = ?types,
            "SERVE_UNSYNCED with SYNC_SOURCE naming a peer: deploys answer the bootstrapping 503 \
             but for the SERVE_UNSYNCED_DEPLOY_TYPES; an empty SYNC_SOURCE makes this node its own authority"
        );
        status.set_copy_deploy_types(types);
    }
    status.set_authority(true);
    status.set_unsynced_copy(copy);
    status.set_state(SyncState::Syncing);
}

/// What `start` hands back for the operator plane: the orchestrator's
/// pause/resume/force handle (`ops::admin`). Process state only.
pub struct SyncHandles {
    pub control: SyncControl,
}

/// Builds the sync pipeline and registers its jobs: `sync-batch-flush` (the
/// deployer's batch ticker), `sync` (the orchestrator), `retry-failed` when
/// enabled, and `failed-deployments-prune`. Returns once the jobs are spawned; the
/// orchestrator moves `status` out of Bootstrapping on its own.
pub async fn start(
    settings: &SyncSettings,
    pool: PgPool,
    storage: Arc<Storage>,
    status: Arc<SyncStatus>,
    client: reqwest::Client,
    jobs: &Arc<Jobs>,
) -> anyhow::Result<SyncHandles> {
    let control = SyncControl::default();
    let leader = Arc::new(Leader {
        settings: settings.clone(),
        pool,
        storage,
        status,
        client,
        control: control.clone(),
        jobs: jobs.clone(),
    });
    jobs.spawn_leased("sync", move |ctx| {
        let leader = leader.clone();
        async move { leader.run(ctx).await }
    });
    Ok(SyncHandles { control })
}

/// Everything the process holding the `sync` lease builds and runs: the
/// deployer and its flush ticker, the bloom filter over the deployed entity
/// ids, the orchestrator, the failed-deployments retry and prune loops and
/// the status publisher. Built afresh each time the lease is taken and ended
/// together when it is lost or the process stops: the helpers are scoped to
/// the leased job ([`Jobs::spawn_scoped`]). While it runs, this process is
/// the [`SyncStatus`] authority; every other process mirrors what the
/// publisher writes ([`status_mirror`]).
struct Leader {
    settings: SyncSettings,
    pool: PgPool,
    storage: Arc<Storage>,
    status: Arc<SyncStatus>,
    client: reqwest::Client,
    control: SyncControl,
    jobs: Arc<Jobs>,
}

impl Leader {
    async fn run(&self, ctx: JobContext) -> anyhow::Result<()> {
        let _authority = Authority::take(&self.status);
        // A takeover starts where a process restart would: nothing is followed
        // until the orchestrator has caught up with every peer again.
        self.status.set_state(SyncState::Bootstrapping);
        status_mirror::spawn_publisher(&self.jobs, &ctx, self.pool.clone(), self.status.clone());
        let settings = &self.settings;
        let limits = memory_limits();
        let deployer = Arc::new(LiveSyncDeployer::with_limits(
            self.pool.clone(),
            settings.flush_concurrency,
            limits,
        ));
        deployer.spawn_flush_ticker(&self.jobs, &ctx);
        tracing::info!(
            inflight_bytes_max = limits.inflight_bytes_max,
            flush_backlog_max = limits.flush_backlog_max,
            content_download_concurrency = settings.content_download_concurrency,
            "Sync memory limits"
        );
        let failed = Arc::new(LiveFailedDeploymentsStore::new(self.pool.clone()));
        let repo = Arc::new(LiveDeploymentRepository::new(
            self.pool.clone(),
            self.status.clone(),
        ));

        let mut bloom = BloomFilter::with_capacity(settings.bloom_expected_elements);
        let seeding = std::time::Instant::now();
        let count = repo
            .seed_deployed_entity_ids(&mut bloom)
            .await
            .context("loading deployed entity ids into the bloom filter")?;
        tracing::info!(
            count,
            elapsed_ms = seeding.elapsed().as_millis() as u64,
            pool_size = self.pool.size(),
            pool_idle = self.pool.num_idle(),
            "Bloom filter populated"
        );

        let batch = Arc::new(BatchDeployer::new(
            BatchDeployerConfig {
                content_download_concurrency: settings.content_download_concurrency,
                profile_max_age_ms: settings.profile_max_age_ms,
                ..Default::default()
            },
            self.client.clone(),
            self.storage.clone(),
            self.pool.clone(),
            deployer.clone(),
            failed.clone(),
            bloom,
        ));
        let orchestrator = Arc::new(SyncOrchestrator::new(
            SyncOrchestratorConfig {
                from_timestamp: settings.from_timestamp_ms,
                entity_types: settings.entity_types.clone(),
                phased_sync: settings.phased_sync,
                snapshot_concurrency: settings.snapshot_concurrency,
                snapshot_scheduling: settings.snapshot_scheduling,
                ..Default::default()
            },
            SyncDependencies {
                http_client: self.client.clone(),
                storage: self.storage.clone(),
                deployer: batch,
                processed_store: Arc::new(LiveProcessedSnapshotStore::new(self.pool.clone())),
                deployment_repo: repo,
                status: self.status.clone(),
                control: self.control.clone(),
            },
        ));
        if let Some(types) = &settings.entity_types {
            tracing::info!(?types, "Sync restricted to an entity-type allowlist");
        }
        if settings.from_timestamp_ms > 0 {
            tracing::warn!(
                from_timestamp_ms = settings.from_timestamp_ms,
                "SYNC_FROM_TIMESTAMP_MS set: bootstrap is bounded, older entities are never fetched"
            );
        }
        tracing::info!(peers = ?settings.peers, phased_sync = settings.phased_sync, "Sync orchestrator ready");
        let peers: HashSet<String> = settings.peers.iter().cloned().collect();
        if settings.retry_failed_enabled {
            let retry = RetryFailedDeployments::new(
                RetryFailedConfig {
                    max_retries: settings.max_retries,
                    retry_concurrency: settings.retry_concurrency,
                    ..Default::default()
                },
                self.client.clone(),
                self.storage.clone(),
                deployer,
                failed,
                Arc::new(RwLock::new(settings.peers.clone())),
            );
            self.jobs
                .spawn_scoped(&ctx, "retry-failed", move |ctx| async move {
                    let trace = ctx.clone();
                    retry
                        .run(ctx.cancel.clone(), move |outcome| match outcome {
                            Ok(stats) => trace.progress(format!("{stats:?}")),
                            Err(e) => trace.error(e.to_string()),
                        })
                        .await;
                    Ok(())
                });
        }
        let ttl_days = settings.prune_ttl_days;
        let pool = self.pool.clone();
        self.jobs
            .spawn_scoped(&ctx, "failed-deployments-prune", move |ctx| async move {
                let trace = ctx.clone();
                run_prune_loop(
                    pool,
                    ttl_days,
                    Duration::from_secs(300),
                    ctx.cancel.clone(),
                    move |outcome| match outcome {
                        Ok(pruned) => trace.progress(format!("pruned {pruned}")),
                        Err(e) => trace.error(e.to_string()),
                    },
                )
                .await;
                Ok(())
            });
        run_sync_job(orchestrator, peers, ctx).await
    }
}

/// Marks this process the sync authority for as long as it is held.
struct Authority(Arc<SyncStatus>);

impl Authority {
    fn take(status: &Arc<SyncStatus>) -> Self {
        status.set_authority(true);
        Self(status.clone())
    }
}

impl Drop for Authority {
    fn drop(&mut self) {
        self.0.set_authority(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(args: &[&str]) -> Config {
        let mut argv = vec!["x", "--database-url", "postgres:///catalyst"];
        argv.extend_from_slice(args);
        Config::parse_for_tests(&argv).unwrap()
    }

    #[test]
    fn disabled_sync_yields_no_settings_and_skips_every_check() {
        let settings = SyncSettings::from_config(&config(&["--sync-source", "garbage"])).unwrap();
        assert!(settings.is_none());
    }

    #[test]
    fn enabled_sync_needs_no_rpc_endpoint() {
        let settings = SyncSettings::from_config(&config(&[
            "--sync-enabled",
            "--sync-source",
            "https://peer.example/content",
        ]))
        .unwrap()
        .unwrap();
        assert_eq!(settings.peers, vec!["https://peer.example/content"]);
    }

    #[test]
    fn enabled_sync_refuses_a_self_referential_or_plaintext_peer() {
        let err = SyncSettings::from_config(&config(&[
            "--sync-enabled",
            "--sync-source",
            "http://127.0.0.1:5210/content",
        ]))
        .unwrap_err();
        assert!(err.to_string().contains("itself"), "{err}");

        let err = SyncSettings::from_config(&config(&[
            "--sync-enabled",
            "--sync-source",
            "http://peer.example/content",
        ]))
        .unwrap_err();
        assert!(err.to_string().contains("plaintext"), "{err}");
    }

    #[test]
    fn serve_unsynced_cannot_be_combined_with_the_worker() {
        let err = SyncSettings::from_config(&config(&[
            "--sync-enabled",
            "--serve-unsynced",
            "--sync-source",
            "https://peer.example/content",
        ]))
        .unwrap_err();
        assert!(err.to_string().contains("exclusive"), "{err}");
    }

    #[test]
    fn a_zero_concurrency_is_refused() {
        let err = SyncSettings::from_config(&config(&[
            "--sync-enabled",
            "--sync-source",
            "https://peer.example/content",
            "--sync-scene-concurrency",
            "0",
        ]))
        .unwrap_err();
        assert!(err.to_string().contains("SYNC_SCENE_CONCURRENCY"), "{err}");
    }

    #[test]
    fn memory_limits_follow_the_config_and_refuse_a_zero() {
        let limits = memory_limits_from_config(&config(&[
            "--sync-inflight-bytes-max",
            "123",
            "--sync-flush-backlog-max",
            "7",
        ]))
        .unwrap();
        assert_eq!(
            limits,
            SyncMemoryLimits {
                inflight_bytes_max: 123,
                flush_backlog_max: 7
            }
        );
        assert_eq!(
            memory_limits_from_config(&config(&[])).unwrap(),
            SyncMemoryLimits::DEFAULT
        );

        let err = SyncSettings::from_config(&config(&[
            "--sync-enabled",
            "--sync-source",
            "https://peer.example/content",
            "--sync-flush-backlog-max",
            "0",
        ]))
        .unwrap_err();
        assert!(err.to_string().contains("SYNC_FLUSH_BACKLOG_MAX"), "{err}");
        let err =
            memory_limits_from_config(&config(&["--sync-inflight-bytes-max", "0"])).unwrap_err();
        assert!(err.to_string().contains("SYNC_INFLIGHT_BYTES_MAX"), "{err}");
    }

    #[test]
    fn valid_settings_carry_the_parsed_peers_and_types() {
        let settings = SyncSettings::from_config(&config(&[
            "--sync-enabled",
            "--sync-source",
            "https://peer.example/content/, https://other.example/content",
            "--sync-entity-types",
            "store,outfits",
            "--concurrent-sync-downloads",
            "12",
            "--sync-from-timestamp-ms",
            "1788307200000",
        ]))
        .unwrap()
        .unwrap();
        assert_eq!(
            settings.peers,
            vec![
                "https://peer.example/content",
                "https://other.example/content"
            ]
        );
        let types = settings.entity_types.as_ref().unwrap();
        assert_eq!(types.len(), 2);
        assert!(types.contains("store") && types.contains("outfits"));
        assert_eq!(settings.content_download_concurrency, 12);
        assert_eq!(settings.from_timestamp_ms, 1_788_307_200_000);
        assert_eq!(settings.snapshot_scheduling.nonscene_concurrency, 64);
        assert_eq!(settings.connections_max_idle, 25);
    }
}
