//! Sequences one node's sync against its peers. Every peer walks
//! snapshots -> pointer-changes catch-up -> steady-state stream; with
//! `phased_sync` the non-profile types go first so the node can serve them
//! (`PartiallySynced`) while profiles bootstrap. Progress is reported into
//! the shared [`SyncStatus`] and persisted as the global frontier plus one
//! cursor per peer, and a peer whose snapshots did not fully deploy is held
//! back and retried rather than advanced past the entities it still owes.
//! While any peer is held back the node stays `Bootstrapping` (`/about` 503):
//! a replica that owes a peer's snapshots is not ready, however far the
//! other peers got.

use futures::StreamExt;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering::Relaxed, Ordering::SeqCst};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::sync::{Mutex, Notify};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use super::pointer_changes::{self, PointerChangesOptions, ScheduleTarget};
use super::repository::{LiveDeploymentRepository, LiveProcessedSnapshotStore};
use super::snapshots::{self, SnapshotScheduling};
use super::{
    NON_PROFILE_TYPES, SnapshotMetadata, SyncError, SyncState, SyncStatus, TimeRange, Timestamp,
};
use crate::batch_deployer::{BatchDeployer, DeploymentReport};
use crate::jobs::JobContext;
use crate::storage::Storage;

/// Defaults are the values the old binary ran in production.
#[derive(Debug, Clone)]
pub struct SyncOrchestratorConfig {
    pub from_timestamp: Timestamp,
    pub request_max_retries: u32,
    pub request_retry_wait_ms: u64,
    pub pointer_changes_wait_time_ms: u64,
    pub bootstrap_reconnect_time_ms: u64,
    pub bootstrap_reconnect_exponent: f64,
    pub bootstrap_max_reconnect_ms: u64,
    pub syncing_reconnect_time_ms: u64,
    pub syncing_reconnect_exponent: f64,
    pub syncing_max_reconnect_ms: u64,
    /// `PHASED_SYNC`: non-profile types first, then profiles.
    pub phased_sync: bool,
    /// `SYNC_ENTITY_TYPES`: `None` syncs every type. Snapshots are marked processed for this
    /// subset only, so widening it later needs explicit replay/resync handling.
    pub entity_types: Option<HashSet<String>>,
    /// Snapshots deployed at once per pass (`SYNC_SNAPSHOT_CONCURRENCY`).
    pub snapshot_concurrency: usize,
    pub snapshot_scheduling: SnapshotScheduling,
}

impl Default for SyncOrchestratorConfig {
    fn default() -> Self {
        Self {
            from_timestamp: 0,
            request_max_retries: 10,
            request_retry_wait_ms: 5_000,
            pointer_changes_wait_time_ms: 10_000,
            bootstrap_reconnect_time_ms: 5_000,
            bootstrap_reconnect_exponent: 1.5,
            bootstrap_max_reconnect_ms: 3_600_000,
            syncing_reconnect_time_ms: 1_000,
            syncing_reconnect_exponent: 1.5,
            syncing_max_reconnect_ms: 86_400_000,
            phased_sync: true,
            entity_types: None,
            snapshot_concurrency: 4,
            snapshot_scheduling: SnapshotScheduling::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServerPhase {
    BootstrappingSnapshots,
    BootstrappingPointerChanges,
    Syncing,
}

struct ServerState {
    phase: ServerPhase,
    last_snapshot_timestamp: Timestamp,
    sync_task: Option<tokio::task::JoinHandle<()>>,
}

type SnapshotsByHash = HashMap<String, (Vec<SnapshotMetadata>, HashSet<String>)>;

/// Everything the orchestrator drives, sharing ONE pool, ONE http client and
/// ONE storage tree (a downloaded snapshot file is a blob in it, like any
/// content file).
pub struct SyncDependencies {
    pub http_client: reqwest::Client,
    pub storage: Arc<Storage>,
    pub deployer: Arc<BatchDeployer>,
    pub processed_store: Arc<LiveProcessedSnapshotStore>,
    pub deployment_repo: Arc<LiveDeploymentRepository>,
    pub status: Arc<SyncStatus>,
    /// The pause/resume handle the console holds; shared with every
    /// orchestrator this process builds over its lifetime, so a control taken
    /// before the sync lease was acquired still reaches the running one.
    pub control: SyncControl,
}

pub struct SyncOrchestrator {
    inner: SyncOrchestratorRefs,
    bootstrap_handle: Mutex<Option<tokio::task::AbortHandle>>,
}

const POINTER_CHANGES_SHIFT_MS: Timestamp = 20 * 60_000;

/// The operator's sync control (the old `SyncControlHandle`, re-added for
/// `/admin/api/content/sync/{pause,resume,force}`): process state beside the
/// [`CancellationToken`], never persisted, so a restart resumes syncing.
/// `pause` holds every *new* pass (a snapshot pass, a pointer-changes pass, a
/// steady-state round) at [`SyncControl::wait_while_paused`] and stops a
/// *running* steady-state stream at its next poll: the stream's `should_stop`
/// reads the flag too, returns its confirmed timestamp (the cursor lands
/// through `advance_cursor`) and the per-server loop parks at the waiter, so
/// a paused node makes no peer fetch past the one already in flight. A
/// snapshot download in flight finishes (its cursors land as usual). `resume`
/// releases the waiters and the loop restarts the stream from the persisted
/// cursor. `force` is `resume` plus a wake of the steady-state sleeps, so a
/// peer in error backoff is polled at once instead of at the end of its wait.
/// Stop wins over pause: a paused waiter returns when the token fires.
#[derive(Clone, Default)]
pub struct SyncControl {
    paused: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

impl SyncControl {
    pub fn pause(&self) {
        self.paused.store(true, SeqCst);
        self.notify.notify_waiters();
    }

    pub fn resume(&self) {
        self.paused.store(false, SeqCst);
        self.notify.notify_waiters();
    }

    pub fn force(&self) {
        self.resume();
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(SeqCst)
    }

    /// Blocks while paused; `true` when `cancel` fired first (the caller stops).
    pub async fn wait_while_paused(&self, cancel: &CancellationToken) -> bool {
        loop {
            if cancel.is_cancelled() {
                return true;
            }
            let notified = self.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if !self.is_paused() {
                return false;
            }
            tokio::select! {
                _ = cancel.cancelled() => return true,
                _ = &mut notified => {}
            }
        }
    }

    /// Resolves on the next `pause`/`resume`/`force`: the steady-state loop
    /// selects on it beside its sleep, so `force` cuts a backoff short.
    pub async fn changed(&self) {
        self.notify.notified().await;
    }
}

impl SyncOrchestrator {
    pub fn new(config: SyncOrchestratorConfig, deps: SyncDependencies) -> Self {
        SyncOrchestrator {
            inner: SyncOrchestratorRefs {
                config,
                http_client: deps.http_client,
                storage: deps.storage,
                deployer: deps.deployer,
                processed_store: deps.processed_store,
                deployment_repo: deps.deployment_repo,
                status: deps.status,
                servers: Default::default(),
                bootstrap_done: Default::default(),
                cancel: CancellationToken::new(),
                control: deps.control,
                failed_snapshot_hashes: Default::default(),
                unmarked_clean_snapshots: Default::default(),
            },
            bootstrap_handle: Mutex::new(None),
        }
    }

    pub async fn sync_with_servers(
        &self,
        peer_servers: HashSet<String>,
    ) -> Result<SyncHandle, SyncError> {
        self.inner.check_stopped()?;

        {
            let mut servers = self.inner.servers.lock().await;
            for url in &peer_servers {
                if !servers.contains_key(url) {
                    info!(server = %url, "Adding new server to sync");
                    servers.insert(
                        url.clone(),
                        ServerState {
                            phase: ServerPhase::BootstrappingSnapshots,
                            last_snapshot_timestamp: self.inner.config.from_timestamp,
                            sync_task: None,
                        },
                    );
                }
            }
            servers.retain(|url, state| {
                if peer_servers.contains(url) {
                    return true;
                }
                info!(server = %url, "Removing server from sync");
                if let Some(handle) = state.sync_task.take() {
                    handle.abort();
                }
                false
            });
        }

        if let Some(handle) = self.bootstrap_handle.lock().await.take() {
            info!("Aborting previous bootstrap task before starting new one");
            handle.abort();
        }

        let orchestrator = self.inner.clone();
        let handle = tokio::spawn(async move {
            if let Err(e) = orchestrator.run_bootstrap().await {
                error!(error = %e, "Bootstrap failed");
            }
        });
        *self.bootstrap_handle.lock().await = Some(handle.abort_handle());

        Ok(SyncHandle {
            bootstrap_done: self.inner.bootstrap_done.clone(),
            status: self.inner.status.clone(),
            task: handle,
        })
    }

    pub async fn stop(&self) {
        info!("Stopping sync orchestrator");
        self.inner.cancel.cancel();

        if let Some(handle) = self.bootstrap_handle.lock().await.take() {
            handle.abort();
            info!("Aborted bootstrap task");
        }

        for (url, state) in self.inner.servers.lock().await.iter_mut() {
            if let Some(handle) = state.sync_task.take() {
                handle.abort();
                info!(server = %url, "Aborted sync task");
            }
        }
    }

    pub fn state(&self) -> SyncState {
        self.inner.status.state()
    }

    pub fn status(&self) -> Arc<SyncStatus> {
        self.inner.status.clone()
    }

    /// The operator's pause/resume/force handle over this orchestrator.
    pub fn control(&self) -> SyncControl {
        self.inner.control.clone()
    }
}

/// The sync job for [`crate::jobs::Jobs`]: starts sync against `peers`, waits for the
/// bootstrap, then holds until the job is cancelled and stops the orchestrator. A bootstrap
/// task that dies (panics) marks the node `Failed`; that is what `/status` 503s on.
pub async fn run_sync_job(
    orchestrator: Arc<SyncOrchestrator>,
    peers: HashSet<String>,
    ctx: JobContext,
) -> anyhow::Result<()> {
    info!(peers = ?peers, "Sync orchestrator starting...");
    ctx.progress("starting");
    let handle = match orchestrator.sync_with_servers(peers).await {
        Ok(handle) => handle,
        Err(e) => {
            orchestrator.status().set_state(SyncState::Failed);
            return Err(anyhow::anyhow!("sync orchestrator failed to start: {e}"));
        }
    };
    info!("Sync started, waiting for bootstrap...");
    ctx.progress("bootstrapping");
    let status = orchestrator.status();
    tokio::select! {
        _ = ctx.cancel.cancelled() => {
            ctx.progress("stopping");
            orchestrator.stop().await;
            return Ok(());
        }
        _ = handle.wait_for_bootstrap() => {}
    }
    info!("Bootstrap complete, sync is now in steady state");
    ctx.progress(format!("bootstrapped; {}", status.state_name()));
    tokio::select! {
        _ = ctx.cancel.cancelled() => {}
        outcome = handle.join() => {
            if let Err(e) = outcome
                && !e.is_cancelled() {
                    status.set_state(SyncState::Failed);
                    ctx.error(format!("bootstrap task died: {e}"));
                    orchestrator.stop().await;
                    return Err(anyhow::anyhow!("bootstrap task died: {e}"));
                }
            ctx.cancel.cancelled().await;
        }
    }
    ctx.progress("stopping");
    orchestrator.stop().await;
    Ok(())
}

#[derive(Clone)]
struct SyncOrchestratorRefs {
    config: SyncOrchestratorConfig,
    http_client: reqwest::Client,
    storage: Arc<Storage>,
    deployer: Arc<BatchDeployer>,
    processed_store: Arc<LiveProcessedSnapshotStore>,
    deployment_repo: Arc<LiveDeploymentRepository>,
    status: Arc<SyncStatus>,
    servers: Arc<Mutex<HashMap<String, ServerState>>>,
    bootstrap_done: Arc<Notify>,
    cancel: CancellationToken,
    control: SyncControl,
    failed_snapshot_hashes: Arc<StdMutex<HashSet<String>>>,
    unmarked_clean_snapshots: Arc<StdMutex<HashSet<String>>>,
}

/// Beats the sync heartbeat every 10 s for as long as it lives. A bootstrap
/// pass deploys for hours between the frontier saves that otherwise carry the
/// heartbeat, and `/status` reports the worker down (`up: false`, or no
/// heartbeat at all on a fresh node) meanwhile, which a release controller
/// reads as a dead worker (the deployed host, 2026-09-26: the first activation waited
/// its whole readiness window on `up` while the node deployed snapshots).
/// Dropping the guard stops the beats.
struct HeartbeatGuard(tokio::task::JoinHandle<()>);

impl Drop for HeartbeatGuard {
    fn drop(&mut self) {
        self.0.abort();
    }
}

fn heartbeat_while_busy(repo: Arc<LiveDeploymentRepository>) -> HeartbeatGuard {
    HeartbeatGuard(tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(10));
        loop {
            tick.tick().await;
            let _ = repo
                .set_sync_heartbeat(chrono::Utc::now().timestamp_millis())
                .await;
        }
    }))
}

fn backoff(ms: f64, exponent: f64, max_ms: u64) -> f64 {
    (ms * exponent).min(max_ms as f64)
}

impl SyncOrchestratorRefs {
    fn is_stopped(&self) -> bool {
        self.cancel.is_cancelled()
    }

    fn check_stopped(&self) -> Result<(), SyncError> {
        if self.is_stopped() {
            Err(SyncError::Stopped)
        } else {
            Ok(())
        }
    }

    fn bootstrap_backoff_start(&self) -> f64 {
        self.config.bootstrap_reconnect_time_ms.max(1) as f64
    }

    fn bootstrap_backoff(&self, ms: f64) -> f64 {
        backoff(
            ms,
            self.config.bootstrap_reconnect_exponent,
            self.config.bootstrap_max_reconnect_ms,
        )
    }

    async fn select_servers<T>(&self, f: impl Fn(&str, &ServerState) -> Option<T>) -> Vec<T> {
        self.servers
            .lock()
            .await
            .iter()
            .filter_map(|(url, s)| f(url, s))
            .collect()
    }

    async fn server_urls(&self) -> Vec<String> {
        self.select_servers(|url, _| Some(url.to_string())).await
    }

    /// Holds a new pass while the operator has sync paused; `Stopped` when the
    /// orchestrator stopped meanwhile (stop wins over pause).
    async fn wait_while_paused(&self) -> Result<(), SyncError> {
        if self.control.wait_while_paused(&self.cancel).await {
            Err(SyncError::Stopped)
        } else {
            Ok(())
        }
    }

    /// True if the orchestrator stopped meanwhile.
    async fn sleep_or_stop(&self, ms: u64) -> bool {
        if self.is_stopped() {
            return true;
        }
        tokio::select! {
            _ = self.cancel.cancelled() => true,
            _ = tokio::time::sleep(Duration::from_millis(ms)) => self.is_stopped(),
        }
    }

    fn set_state(&self, state: SyncState) {
        self.status.set_state(state);
        self.bootstrap_done.notify_waiters();
    }

    async fn servers_not_syncing(&self) -> Vec<String> {
        self.select_servers(|url, s| (s.phase != ServerPhase::Syncing).then(|| url.to_string()))
            .await
    }

    /// Publishes `state` only when every peer reached steady state; otherwise the node stays
    /// where it is and the straggler retry publishes once the held peers catch up.
    async fn announce_if_every_server_synced(&self, state: SyncState, message: &str) {
        let held = self.servers_not_syncing().await;
        if held.is_empty() {
            info!("{message}");
            self.set_state(state);
        } else {
            info!(
                servers = ?held,
                "{message}, but held-back peers keep the node in bootstrap until their \
                 snapshots are fully deployed"
            );
        }
    }

    async fn run_bootstrap(&self) -> Result<(), SyncError> {
        let mut backoff_ms = self.bootstrap_backoff_start();
        loop {
            let phased = self.config.phased_sync
                && self.config.entity_types.as_ref().is_none_or(|allow| {
                    allow.contains("profile")
                        && NON_PROFILE_TYPES.iter().any(|t| allow.contains(*t))
                });
            let result = if phased {
                self.run_phased_bootstrap().await
            } else {
                self.run_full_bootstrap().await
            };
            match result {
                Ok(()) => break,
                Err(SyncError::Stopped) => return Err(SyncError::Stopped),
                Err(e) => {
                    warn!(error = %e, backoff_ms, "Bootstrap failed; retrying");
                    if self.sleep_or_stop(backoff_ms as u64).await {
                        return Err(SyncError::Stopped);
                    }
                    backoff_ms = self.bootstrap_backoff(backoff_ms);
                }
            }
        }

        self.retry_bootstrap_stragglers().await
    }

    /// Runs with only the node's global allowlist (no slice filter) on purpose: a straggler may have
    /// been held back by either phased slice, and only a pass covering every synced type proves its
    /// snapshots complete so they can be marked processed.
    async fn retry_bootstrap_stragglers(&self) -> Result<(), SyncError> {
        let mut backoff_ms = self.bootstrap_backoff_start();
        loop {
            let pending = self.servers_not_syncing().await;
            if pending.is_empty() {
                self.failed_snapshot_hashes.lock().unwrap().clear();
                self.unmarked_clean_snapshots.lock().unwrap().clear();
                if self.status.state() != SyncState::Syncing {
                    info!("Every peer reached steady state, entering steady-state sync");
                    self.set_state(SyncState::Syncing);
                }
                return Ok(());
            }
            info!(
                servers = ?pending,
                backoff_ms,
                "Servers held back from sync; retrying their bootstrap"
            );
            if self.sleep_or_stop(backoff_ms as u64).await {
                return Err(SyncError::Stopped);
            }

            let allow = self.config.entity_types.as_ref();
            match self.bootstrap_from_snapshots(allow, true).await {
                Err(SyncError::Stopped) => return Err(SyncError::Stopped),
                Err(e) => warn!(error = %e, "Snapshot bootstrap retry failed"),
                Ok(()) => {}
            }
            match self.bootstrap_from_pointer_changes(allow, true).await {
                Err(SyncError::Stopped) => return Err(SyncError::Stopped),
                Err(e) => warn!(error = %e, "Pointer-changes bootstrap retry failed"),
                Ok(()) => {}
            }

            let still_pending = self.servers_not_syncing().await.len();
            if still_pending < pending.len() {
                self.save_frontier().await;
                self.resolve_deleters().await;
                self.start_steady_state_sync().await?;
                backoff_ms = self.bootstrap_backoff_start();
            } else {
                backoff_ms = self.bootstrap_backoff(backoff_ms);
            }
        }
    }

    /// From ITS OWN persisted cursor (`resume_point` is the rule): a server whose cursor lags the
    /// global frontier resumes from the lag, because the frontier is max-over-servers and riding it
    /// would skip that server's not-yet-deployed entities until the re-snapshot pass.
    async fn resume_servers_from_cursors(&self) -> Result<(), SyncError> {
        let frontier = self.deployment_repo.get_sync_frontier().await?;
        let mut resume_by_url: HashMap<String, (Timestamp, bool)> = HashMap::new();
        for url in self.server_urls().await {
            let cursor = self.deployment_repo.get_server_sync_cursor(&url).await?;
            resume_by_url.insert(url, (resume_point(cursor, frontier), cursor.is_some()));
        }
        for (url, state) in self.servers.lock().await.iter_mut() {
            let Some(&(resume, own_cursor)) = resume_by_url.get(url) else {
                continue;
            };
            if resume > 0 {
                info!(
                    server = %url,
                    resume,
                    own_cursor,
                    "Resuming server from persisted cursor"
                );
                state.last_snapshot_timestamp = state.last_snapshot_timestamp.max(resume);
            }
        }
        Ok(())
    }

    async fn run_full_bootstrap(&self) -> Result<(), SyncError> {
        self.resume_servers_from_cursors().await?;
        let allow = self.config.entity_types.as_ref();

        info!("Phase 1: Bootstrap from snapshots");
        self.bootstrap_from_snapshots(allow, true).await?;

        info!("Phase 2: Bootstrap from pointer-changes");
        self.bootstrap_from_pointer_changes(allow, true).await?;
        self.save_frontier().await;

        info!("Resolving deleter_deployment for overwritten entities");
        self.resolve_deleters().await;

        self.announce_if_every_server_synced(
            SyncState::Syncing,
            "Bootstrap complete, entering steady-state sync",
        )
        .await;

        self.start_steady_state_sync().await
    }

    async fn run_phased_bootstrap(&self) -> Result<(), SyncError> {
        self.resume_servers_from_cursors().await?;

        let non_profile_filter: HashSet<String> = NON_PROFILE_TYPES
            .iter()
            .filter(|t| {
                self.config
                    .entity_types
                    .as_ref()
                    .is_none_or(|allow| allow.contains(**t))
            })
            .map(|s| s.to_string())
            .collect();
        let profile_filter: HashSet<String> = ["profile".to_string()].into_iter().collect();

        info!(types = ?non_profile_filter, "Phase 1: Bootstrap non-profile entities from snapshots");
        self.bootstrap_from_snapshots(Some(&non_profile_filter), false)
            .await?;

        info!("Phase 2: Pointer-changes catch-up for all enabled types");
        self.bootstrap_from_pointer_changes(self.config.entity_types.as_ref(), false)
            .await?;
        self.save_frontier().await;

        let mut ready_types: Vec<String> = non_profile_filter.iter().cloned().collect();
        ready_types.sort();
        self.announce_if_every_server_synced(
            SyncState::PartiallySynced { ready_types },
            "Phase 3: Partially synced \u{2014} non-profile types ready, starting to serve queries",
        )
        .await;

        for state in self.servers.lock().await.values_mut() {
            if state.sync_task.is_none() {
                state.phase = ServerPhase::BootstrappingSnapshots;
            }
        }

        info!("Phase 4: Bootstrap profiles from snapshots");
        self.bootstrap_from_snapshots(Some(&profile_filter), true)
            .await?;

        info!("Phase 5: Final pointer-changes catch-up for all enabled types");
        self.bootstrap_from_pointer_changes(self.config.entity_types.as_ref(), true)
            .await?;
        self.save_frontier().await;

        info!("Phase 6: Resolving deleter_deployment for overwritten entities");
        self.resolve_deleters().await;

        self.announce_if_every_server_synced(
            SyncState::Syncing,
            "Bootstrap complete, entering steady-state sync",
        )
        .await;

        self.start_steady_state_sync().await
    }

    async fn resolve_deleters(&self) {
        if let Err(e) = self.deployment_repo.resolve_deleter_deployments().await {
            warn!(error = %e, "Failed to resolve deleter_deployment");
        }
    }

    async fn save_frontier(&self) {
        let _ = self
            .deployment_repo
            .set_sync_heartbeat(chrono::Utc::now().timestamp_millis())
            .await;
    }

    /// Frontier first, then the per-server cursor, and only on forward movement.
    async fn advance_cursor(&self, server: &str, from: &mut Timestamp, to: Timestamp) {
        if to > *from {
            *from = to;
            let _ = self.deployment_repo.advance_sync_frontier(to).await;
            let _ = self
                .deployment_repo
                .advance_server_sync_cursor(server, to)
                .await;
        }
    }

    async fn bootstrap_from_snapshots(
        &self,
        entity_type_filter: Option<&HashSet<String>>,
        mark_processed: bool,
    ) -> Result<(), SyncError> {
        let bootstrapping = self
            .select_servers(|url, s| {
                (s.phase == ServerPhase::BootstrappingSnapshots).then(|| url.to_string())
            })
            .await;
        if bootstrapping.is_empty() {
            return Ok(());
        }
        self.wait_while_paused().await?;
        let _beat = heartbeat_while_busy(self.deployment_repo.clone());

        info!(
            servers = ?bootstrapping,
            has_type_filter = entity_type_filter.is_some(),
            "Bootstrapping from snapshots"
        );

        let mut snapshots_by_hash = SnapshotsByHash::new();
        let mut last_ts_by_server: HashMap<String, Timestamp> = HashMap::new();
        let mut held_from_fetch = HashSet::new();

        for server in &bootstrapping {
            match snapshots::fetch_snapshots(
                &self.http_client,
                server,
                self.config.request_max_retries,
            )
            .await
            {
                Ok(fetched) => apply_snapshot_fetch(
                    server,
                    fetched,
                    self.config.from_timestamp,
                    &mut last_ts_by_server,
                    &mut snapshots_by_hash,
                    &mut held_from_fetch,
                ),
                Err(e) => warn!(server = %server, error = %e, "Failed to fetch snapshots"),
            }
        }

        let mut candidates: Vec<(String, Timestamp, Vec<Vec<String>>)> = snapshots_by_hash
            .iter()
            .map(|(hash, (metas, _))| {
                let greatest_end = metas
                    .iter()
                    .map(|m| m.time_range.end_timestamp)
                    .max()
                    .unwrap_or(self.config.from_timestamp);
                let groups = metas
                    .iter()
                    .map(|m| m.replaced_snapshot_hashes.clone().unwrap_or_default())
                    .collect();
                (hash.clone(), greatest_end, groups)
            })
            .collect();
        candidates.sort_by(|a, b| a.0.cmp(&b.0));

        let all_hashes: Vec<String> = candidates
            .iter()
            .flat_map(|(hash, _, groups)| {
                std::iter::once(hash.clone()).chain(groups.iter().flatten().cloned())
            })
            .collect();
        let mut processed = self
            .processed_store
            .filter_processed_in_chunks(all_hashes)
            .await?;

        let (deployable, newly_marked) = snapshots::decide_snapshot_pass(
            &candidates,
            &mut processed,
            self.config.from_timestamp,
        );

        self.processed_store
            .mark_processed_many(&newly_marked)
            .await?;

        let mut time_ranges_to_deploy: Vec<TimeRange> = Vec::new();
        let mut snapshots_to_process: Vec<(String, HashSet<String>)> = Vec::new();

        // A snapshot this node generated itself (a row of its own
        // content.snapshots, content::snapshot_gen) lists entities it already
        // holds active: skipped, neither downloaded nor marked processed, as
        // upstream's snapshotStorage.has.
        let own = self
            .processed_store
            .own_snapshot_hashes(&deployable)
            .await?;
        if !own.is_empty() {
            info!(
                count = own.len(),
                "Skipping snapshots this node generated itself"
            );
        }
        for hash in deployable {
            if own.contains(&hash) {
                continue;
            }
            let (metas, servers) = &snapshots_by_hash[&hash];
            time_ranges_to_deploy.extend(metas.iter().map(|m| m.time_range));
            snapshots_to_process.push((hash, servers.clone()));
        }

        let mut need_download = Vec::new();
        for item in &snapshots_to_process {
            if !self.storage.exist(&item.0).await.unwrap_or(true) {
                need_download.push(item.clone());
            }
        }
        if !need_download.is_empty() {
            info!(
                count = need_download.len(),
                "Pre-downloading snapshot files in parallel"
            );
            snapshots::download_snapshot_files(
                &self.http_client,
                self.storage.clone(),
                &need_download,
                self.config.request_max_retries,
                self.config.request_retry_wait_ms,
            )
            .await;
        }

        let snapshot_concurrency = self.config.snapshot_concurrency.max(1);

        let pass: Vec<(String, HashSet<String>, Arc<DeploymentReport>)> = snapshots_to_process
            .into_iter()
            .map(|(hash, servers)| (hash, servers, Arc::new(DeploymentReport::default())))
            .collect();
        let failed_hashes = StdMutex::new(HashSet::<String>::new());

        if !time_ranges_to_deploy.is_empty() {
            self.deployer
                .prepare_for_deployments_in(&time_ranges_to_deploy)
                .await?;
        }

        futures::stream::iter(pass.iter())
            .for_each_concurrent(snapshot_concurrency, |(hash, servers, report)| {
                let failed_hashes = &failed_hashes;
                async move {
                    if self.is_stopped() {
                        return;
                    }
                    if let Err(e) = snapshots::deploy_entities_from_snapshot(
                        &self.http_client,
                        self.storage.as_ref(),
                        self.deployer.as_ref(),
                        snapshots::SnapshotSource {
                            snapshot_hash: hash,
                            servers,
                            max_retries: self.config.request_max_retries,
                            retry_wait_ms: self.config.request_retry_wait_ms,
                        },
                        snapshots::SnapshotFilter {
                            genesis_timestamp: self.config.from_timestamp,
                            entity_type_filter,
                        },
                        self.config.snapshot_scheduling,
                        report,
                        || self.is_stopped(),
                    )
                    .await
                    {
                        warn!(snapshot_hash = %hash, error = %e, "Snapshot deployment failed");
                        failed_hashes.lock().unwrap().insert(hash.clone());
                    }
                }
            })
            .await;

        self.check_stopped()?;
        self.deployer.on_idle().await?;
        self.check_stopped()?;

        let mut failed_hashes = failed_hashes.into_inner().unwrap();
        for (hash, _, report) in &pass {
            if !report.is_complete() {
                warn!(
                    snapshot_hash = %hash,
                    scheduled = report.scheduled(),
                    acknowledged = report.acknowledged(),
                    "Snapshot deployments not fully acknowledged after drain; treating the snapshot as failed"
                );
                failed_hashes.insert(hash.clone());
            }
            if report.lost() > 0 {
                warn!(
                    snapshot_hash = %hash,
                    lost = report.lost(),
                    "Deployer lost entities of this snapshot without a durable record; \
                     treating the snapshot as failed"
                );
                failed_hashes.insert(hash.clone());
            }
        }

        let covers_all =
            pass_covers_all_synced_types(entity_type_filter, self.config.entity_types.as_ref());
        let (held_servers, to_mark) = {
            let mut failed_global = self.failed_snapshot_hashes.lock().unwrap();
            let mut clean_unmarked = self.unmarked_clean_snapshots.lock().unwrap();
            let mut held = held_from_fetch;
            let mut to_mark = Vec::new();
            for (hash, servers, _) in &pass {
                match snapshot_pass_verdict(
                    failed_hashes.contains(hash),
                    mark_processed,
                    covers_all,
                    failed_global.contains(hash),
                    clean_unmarked.contains(hash),
                ) {
                    SnapshotPassVerdict::Failed => {
                        failed_global.insert(hash.clone());
                        clean_unmarked.remove(hash);
                        held.extend(servers.iter().cloned());
                    }
                    SnapshotPassVerdict::CleanUnmarked => {
                        clean_unmarked.insert(hash.clone());
                    }
                    SnapshotPassVerdict::Retire => {
                        failed_global.remove(hash);
                        clean_unmarked.remove(hash);
                        to_mark.push(hash.clone());
                    }
                    SnapshotPassVerdict::Unproven => held.extend(servers.iter().cloned()),
                }
            }
            (held, to_mark)
        };

        if let Err(e) = self.processed_store.mark_processed_many(&to_mark).await {
            warn!(snapshots = to_mark.len(), error = %e, "Failed to mark snapshots as processed");
        }

        let mut advanced = Vec::new();
        {
            let mut servers = self.servers.lock().await;
            for (url, ts) in &last_ts_by_server {
                if held_servers.contains(url) {
                    info!(
                        server = %url,
                        "Keeping the server in snapshot bootstrap: not all of its snapshots \
                         were fully deployed"
                    );
                    continue;
                }
                if let Some(state) = servers.get_mut(url) {
                    state.last_snapshot_timestamp = state.last_snapshot_timestamp.max(*ts);
                    state.phase = ServerPhase::BootstrappingPointerChanges;
                    advanced.push((url.clone(), state.last_snapshot_timestamp));
                }
            }
        }

        if mark_processed {
            for (url, ts) in advanced {
                let _ = self
                    .deployment_repo
                    .advance_server_sync_cursor(&url, ts)
                    .await;
            }
        }

        Ok(())
    }

    async fn bootstrap_from_pointer_changes(
        &self,
        entity_type_filter: Option<&HashSet<String>>,
        persist_cursor: bool,
    ) -> Result<(), SyncError> {
        let bootstrapping = self
            .select_servers(|url, s| {
                (s.phase == ServerPhase::BootstrappingPointerChanges).then(|| {
                    let from = (s.last_snapshot_timestamp - POINTER_CHANGES_SHIFT_MS).max(0);
                    (url.to_string(), from)
                })
            })
            .await;
        if bootstrapping.is_empty() {
            return Ok(());
        }
        self.wait_while_paused().await?;
        let _beat = heartbeat_while_busy(self.deployment_repo.clone());

        let now = chrono::Utc::now().timestamp_millis();
        let min_from = bootstrapping.iter().map(|(_, ts)| *ts).min().unwrap_or(0);
        self.deployer
            .prepare_for_deployments_in(&[TimeRange {
                init_timestamp: min_from,
                end_timestamp: now,
            }])
            .await?;

        let filter_owned = entity_type_filter.map(|f| Arc::new(f.clone()));

        let mut handles = Vec::new();
        for (server, from_timestamp) in bootstrapping {
            let refs = self.clone();
            let filter = filter_owned.clone();
            let all_servers = self.server_urls().await;

            handles.push(tokio::spawn(async move {
                let options = PointerChangesOptions {
                    from_timestamp,
                    wait_time_ms: 0,
                    entity_type_filter: filter.as_deref(),
                };
                let progress = AtomicI64::new(from_timestamp);
                let report = Arc::new(DeploymentReport::default());
                let outcome = pointer_changes::deploy_entities_from_pointer_changes(
                    &refs.http_client,
                    &server,
                    &options,
                    ScheduleTarget {
                        deployer: refs.deployer.as_ref(),
                        content_servers: &all_servers,
                        report: &report,
                    },
                    persist_cursor.then(|| refs.deployment_repo.clone()),
                    &progress,
                    || refs.is_stopped(),
                )
                .await;
                let reached = progress.load(Relaxed);
                let (ts, failed) = match outcome {
                    Ok(greatest_ts) => (greatest_ts, false),
                    Err(e) => {
                        warn!(server = %server, error = %e, "Pointer-changes bootstrap failed");
                        (reached, true)
                    }
                };
                if let Some(state) = refs.servers.lock().await.get_mut(&server) {
                    // A finished pass delivered every row before its settled cursor, while the
                    // snapshot's end can sit above rows the pass read before they committed:
                    // the stream resumes from the pass.
                    state.last_snapshot_timestamp = if failed {
                        state.last_snapshot_timestamp.max(ts)
                    } else {
                        ts
                    };
                    if !failed && !refs.is_stopped() {
                        state.phase = ServerPhase::Syncing;
                    }
                }
            }));
        }

        for handle in handles {
            let _ = handle.await;
        }

        self.deployer.on_idle().await?;
        Ok(())
    }

    async fn start_steady_state_sync(&self) -> Result<(), SyncError> {
        let syncing = self
            .select_servers(|url, s| {
                (s.phase == ServerPhase::Syncing && s.sync_task.is_none())
                    .then(|| (url.to_string(), s.last_snapshot_timestamp))
            })
            .await;

        for (server, mut from_timestamp) in syncing {
            let refs = self.clone();
            let all_servers = self.server_urls().await;
            let server_key = server.clone();

            let handle = tokio::spawn(async move {
                let cfg = &refs.config;
                let mut backoff_ms = cfg.syncing_reconnect_time_ms as f64;
                loop {
                    if refs.control.wait_while_paused(&refs.cancel).await {
                        return;
                    }
                    let options = PointerChangesOptions {
                        from_timestamp,
                        wait_time_ms: cfg.pointer_changes_wait_time_ms,
                        entity_type_filter: cfg.entity_types.as_ref(),
                    };
                    let progress = AtomicI64::new(from_timestamp);
                    let report = Arc::new(DeploymentReport::default());
                    let outcome = pointer_changes::deploy_entities_from_pointer_changes(
                        &refs.http_client,
                        &server,
                        &options,
                        ScheduleTarget {
                            deployer: refs.deployer.as_ref(),
                            content_servers: &all_servers,
                            report: &report,
                        },
                        Some(refs.deployment_repo.clone()),
                        &progress,
                        || refs.is_stopped() || refs.control.is_paused(),
                    )
                    .await;
                    refs.advance_cursor(&server, &mut from_timestamp, progress.load(Relaxed))
                        .await;
                    match outcome {
                        Ok(greatest_ts) => {
                            refs.advance_cursor(&server, &mut from_timestamp, greatest_ts)
                                .await;
                            backoff_ms = cfg.syncing_reconnect_time_ms as f64;
                        }
                        Err(e) => {
                            error!(server = %server, error = %e, "Sync stream failed");
                            backoff_ms = backoff(
                                backoff_ms,
                                cfg.syncing_reconnect_exponent,
                                cfg.syncing_max_reconnect_ms,
                            );
                        }
                    }
                    if let Some(state) = refs.servers.lock().await.get_mut(&server) {
                        state.last_snapshot_timestamp =
                            state.last_snapshot_timestamp.max(from_timestamp);
                    }
                    tokio::select! {
                        _ = refs.cancel.cancelled() => return,
                        _ = tokio::time::sleep(Duration::from_millis(backoff_ms as u64)) => {}
                        _ = refs.control.changed() => {}
                    }
                }
            });

            if let Some(state) = self.servers.lock().await.get_mut(&server_key) {
                state.sync_task = Some(handle);
            }
        }

        Ok(())
    }
}

pub struct SyncHandle {
    bootstrap_done: Arc<Notify>,
    status: Arc<SyncStatus>,
    task: tokio::task::JoinHandle<()>,
}

impl SyncHandle {
    /// Returns once the node left `Bootstrapping` (partially or fully synced), even when that
    /// happened before this was called.
    pub async fn wait_for_bootstrap(&self) {
        loop {
            let notified = self.bootstrap_done.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.status.state() != SyncState::Bootstrapping {
                return;
            }
            notified.await;
        }
    }

    /// The bootstrap task itself; it only ends on stop or on a panic.
    pub async fn join(self) -> Result<(), tokio::task::JoinError> {
        self.task.await
    }
}

/// The server's resume point is max(endTimestamp) over the entries that VALIDATED -- recorded
/// unconditionally, because whether it may be adopted is decided later: a server with discarded
/// entries goes into `held_servers` (each discarded entry stood for a time range nothing else
/// covers), and held servers never have their timestamp or phase advanced. A server with no
/// snapshots at all falls back to the genesis timestamp so it still advances to pointer-changes
/// bootstrap instead of being re-fetched forever; a FAILED fetch leaves no entry anywhere, holding
/// the server back.
///
/// Every advertiser's metadata for a hash is kept, not just the first server's, so the deployment
/// decision can weigh each advertiser's replaced-hash group and the greatest end-timestamp.
fn apply_snapshot_fetch(
    server: &str,
    fetched: snapshots::SnapshotsFromServer,
    genesis_timestamp: Timestamp,
    last_ts_by_server: &mut HashMap<String, Timestamp>,
    snapshots_by_hash: &mut SnapshotsByHash,
    held_servers: &mut HashSet<String>,
) {
    if fetched.discarded > 0 {
        warn!(
            server,
            discarded = fetched.discarded,
            "Snapshot list was not fully readable; keeping the server in snapshot bootstrap"
        );
        held_servers.insert(server.to_string());
    }
    let last_ts = fetched
        .snapshots
        .iter()
        .map(|s| s.time_range.end_timestamp)
        .max()
        .unwrap_or(genesis_timestamp);
    last_ts_by_server.insert(server.to_string(), last_ts);
    for snap in fetched.snapshots {
        let entry = snapshots_by_hash.entry(snap.hash.clone()).or_default();
        entry.0.push(snap);
        entry.1.insert(server.to_string());
    }
}

/// Its OWN persisted cursor when one exists -- even when that lags the global frontier, the
/// invariant this function carries -- the global frontier for a server with no cursor row (never
/// synced, or persisted by a build predating per-server cursors), and zero when neither exists,
/// leaving the caller's in-memory floor (config.from_timestamp) in charge.
pub fn resume_point(own_cursor: Option<Timestamp>, global_frontier: Timestamp) -> Timestamp {
    own_cursor.unwrap_or(global_frontier)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SnapshotPassVerdict {
    /// The pass could not prove the snapshot fully deployed (deploy error, unacknowledged
    /// entities, or lost-entity contamination): record it as failed and hold its advertising
    /// servers in snapshot bootstrap.
    Failed,
    /// Cleanly deployed for this pass's types, deliberately left unmarked (phased sync's
    /// non-marking slice): remember it so a later marking slice can prove the whole snapshot.
    CleanUnmarked,
    /// Provably complete: mark it processed and clear its bookkeeping.
    Retire,
    /// Clean for this slice, but the rest of the snapshot is unproven -- an earlier pass failed
    /// on it, or it never went through the non-marking slice. Marking it would silently skip
    /// the unproven types' entities forever, and advancing its servers would move their
    /// timestamps past those entities with nothing left to re-deliver them. Hold the servers
    /// back so the unfiltered straggler retry deploys the whole snapshot and retires it.
    Unproven,
}

/// `None` = all types. This is the condition under which a clean marking pass proves a snapshot
/// complete. Under an allowlist "complete" deliberately means the allowed types only: the excluded
/// types are never wanted, so waiting for them would leave every snapshot unretirable forever.
fn pass_covers_all_synced_types(
    pass_filter: Option<&HashSet<String>>,
    allowlist: Option<&HashSet<String>>,
) -> bool {
    match (pass_filter, allowlist) {
        (None, _) => true,
        (Some(f), Some(allow)) => allow.iter().all(|t| f.contains(t)),
        (Some(_), None) => false,
    }
}

/// Pure decision table for one snapshot at the end of a bootstrap pass.
fn snapshot_pass_verdict(
    failed_this_pass: bool,
    mark_processed: bool,
    covers_all_synced_types: bool,
    failed_earlier: bool,
    clean_unmarked_earlier: bool,
) -> SnapshotPassVerdict {
    if failed_this_pass {
        SnapshotPassVerdict::Failed
    } else if !mark_processed {
        SnapshotPassVerdict::CleanUnmarked
    } else if covers_all_synced_types || (!failed_earlier && clean_unmarked_earlier) {
        SnapshotPassVerdict::Retire
    } else {
        SnapshotPassVerdict::Unproven
    }
}

#[cfg(test)]
mod tests;
