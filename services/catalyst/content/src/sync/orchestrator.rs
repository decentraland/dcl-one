//! Sequences one node's sync against its peers. Every peer walks
//! snapshots -> pointer-changes catch-up -> steady-state stream; with
//! `phased_sync` the non-profile types go first so the node can serve them
//! (`PartiallySynced`) while profiles bootstrap. Progress is reported into
//! the shared [`SyncStatus`] and persisted as the global frontier plus one
//! cursor per peer, and a peer whose snapshots did not fully deploy is held
//! back and retried rather than advanced past the entities it still owes.
//! The node leaves `Bootstrapping` (`/about` 503) once a bootstrap pass over
//! every peer finished, whatever each peer's outcome: a peer that is
//! unreachable or did not fully deploy stays in bootstrap and is retried in
//! the background without holding the node, so one dead DAO peer cannot keep
//! a node unready. Deploys of a type are taken only once that type is ready
//! ([`SyncStatus::takes_deploys_of`]).

use futures::future::{BoxFuture, Shared};
use futures::{FutureExt, StreamExt};
use parking_lot::Mutex as SyncMutex;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering::Relaxed, Ordering::SeqCst};
use std::time::Duration;
use tokio::sync::{Mutex, Notify};
use tokio::task::{JoinError, JoinHandle};
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use super::metrics;
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
    pub snapshot_resync_initial_delay: Duration,
    pub snapshot_resync_interval: Duration,
    pub bootstrap_reconnect_time_ms: u64,
    pub bootstrap_reconnect_exponent: f64,
    pub bootstrap_max_reconnect_ms: u64,
    pub syncing_reconnect_time_ms: u64,
    pub syncing_reconnect_exponent: f64,
    pub syncing_max_reconnect_ms: u64,
    /// `PHASED_SYNC`: non-profile types first, then profiles.
    pub phased_sync: bool,
    /// `SYNC_ENTITY_TYPES`: `None` syncs every type. A snapshot receipt covers the types
    /// this pass synced, so a wider allowlist can replay the snapshot.
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
            pointer_changes_wait_time_ms: 5_000,
            snapshot_resync_initial_delay: Duration::from_secs(3_600),
            snapshot_resync_interval: Duration::from_secs(14 * 86_400),
            bootstrap_reconnect_time_ms: 5_000,
            bootstrap_reconnect_exponent: 1.5,
            bootstrap_max_reconnect_ms: 3_600_000,
            syncing_reconnect_time_ms: 1_000,
            syncing_reconnect_exponent: 1.2,
            syncing_max_reconnect_ms: 3_600_000,
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
    retry_at: Instant,
    retry_delay_ms: f64,
    cancel: CancellationToken,
    sync_task: Option<JoinHandle<()>>,
}

/// A peer captured for one bootstrap pass. Removal cancels this membership even if the same
/// URL is added again before the pass returns.
#[derive(Clone)]
struct Peer {
    url: String,
    cancel: CancellationToken,
}

type SnapshotsByHash = HashMap<String, (Vec<SnapshotMetadata>, HashSet<String>)>;

/// Everything the orchestrator drives, sharing ONE pool, ONE http client and
/// ONE storage tree (a downloaded snapshot file is a blob in it, like any
/// content file).
pub struct SyncDependencies {
    pub http_client: crate::http::CheckedRedirectClient,
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
    tasks: Mutex<SyncTasks>,
}

type BootstrapTask = Shared<BoxFuture<'static, Result<(), Arc<JoinError>>>>;

#[derive(Default)]
struct SyncTasks {
    bootstrap: Option<BootstrapTask>,
    retired_streams: Vec<JoinHandle<()>>,
    stopped: bool,
}

const POINTER_CHANGES_SHIFT_MS: Timestamp = 20 * 60_000;
const DEPLOYMENT_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);

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
                peers_changed: Default::default(),
                cancel: CancellationToken::new(),
                control: deps.control,
                failed_snapshot_hashes: Default::default(),
                unmarked_clean_snapshots: Default::default(),
            },
            tasks: Mutex::default(),
        }
    }

    pub async fn sync_with_servers(
        &self,
        peer_servers: HashSet<String>,
    ) -> Result<SyncHandle, SyncError> {
        let mut tasks = self.tasks.lock().await;
        self.inner.check_stopped()?;

        let initial_peers = {
            let mut servers = self.inner.servers.lock().await;
            for url in &peer_servers {
                if !servers.contains_key(url) {
                    info!(server = %url, "Adding new server to sync");
                    servers.insert(
                        url.clone(),
                        ServerState {
                            phase: ServerPhase::BootstrappingSnapshots,
                            last_snapshot_timestamp: self.inner.config.from_timestamp,
                            retry_at: Instant::now(),
                            retry_delay_ms: self.inner.bootstrap_backoff_start(),
                            cancel: self.inner.cancel.child_token(),
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
                state.cancel.cancel();
                if let Some(handle) = state.sync_task.take() {
                    tasks.retired_streams.push(handle);
                }
                false
            });
            report_phases(&servers);
            let mut urls: Vec<String> = servers.keys().cloned().collect();
            urls.sort();
            self.inner.deployer.set_peers(urls).await;
            if tasks.bootstrap.is_none() {
                servers
                    .iter()
                    .map(|(url, state)| Peer {
                        url: url.clone(),
                        cancel: state.cancel.clone(),
                    })
                    .collect()
            } else {
                Vec::new()
            }
        };

        tasks.retired_streams.retain(|task| !task.is_finished());
        let task = tasks.bootstrap.get_or_insert_with(|| {
            let orchestrator = self.inner.clone();
            let task = tokio::spawn(async move {
                if let Err(e) = orchestrator.run_bootstrap(initial_peers).await
                    && !matches!(e, SyncError::Stopped)
                {
                    error!(error = %e, "Bootstrap failed");
                }
            });
            async move { task.await.map_err(Arc::new) }.boxed().shared()
        });
        self.inner.peers_changed.notify_one();
        Ok(SyncHandle {
            bootstrap_done: self.inner.bootstrap_done.clone(),
            status: self.inner.status.clone(),
            task: task.clone(),
        })
    }

    pub async fn stop(&self) {
        let mut tasks = self.tasks.lock().await;
        if tasks.stopped {
            return;
        }
        info!("Stopping sync orchestrator");
        self.inner.cancel.cancel();

        if let Some(task) = &tasks.bootstrap
            && let Err(e) = task.clone().await
        {
            warn!(error = %e, "Bootstrap task exited before shutdown");
        }

        tasks.retired_streams.extend(
            self.inner
                .servers
                .lock()
                .await
                .values_mut()
                .filter_map(|state| state.sync_task.take()),
        );
        while let Some(task) = tasks.retired_streams.last_mut() {
            if let Err(e) = task.await {
                warn!(error = %e, "Sync stream exited before shutdown");
            }
            tasks.retired_streams.pop();
        }
        match tokio::time::timeout(DEPLOYMENT_DRAIN_TIMEOUT, self.inner.deployer.on_idle()).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => error!(error = %e, "Failed to drain sync deployments during shutdown"),
            Err(_) => {
                warn!(
                    in_flight = self.inner.deployer.in_flight(),
                    "Sync deployment drain timed out; next bootstrap replays unconfirmed work"
                );
                self.inner.deployer.cancel_pending();
            }
        }
        metrics::server_phases(0, 0, 0);
        tasks.stopped = true;
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

impl Drop for SyncOrchestrator {
    fn drop(&mut self) {
        self.inner.cancel.cancel();
        self.inner.deployer.cancel_pending();
    }
}

/// The sync job for [`crate::jobs::Jobs`]: starts sync against `peers` and holds until the job
/// is cancelled, then stops the orchestrator. A bootstrap task that dies (panics) marks the node
/// `Failed`: `/about` answers 503 and every deploy is refused, while `/status` stays 200 and
/// names the state. The bootstrap task remains active to catch up newly added peers and refresh
/// snapshots. `on_bootstrap` starts the failed-deployments retry when every type bootstrapped.
/// A `PartiallySynced` node is still deploying profile snapshots.
///
/// A changed `peers` set (the DAO list refresh) is handed to
/// [`SyncOrchestrator::sync_with_servers`] at any time, during the first bootstrap too: a new
/// peer bootstraps beside the others while the node keeps its state, a dropped peer's stream
/// stops, and `retry_peers` (where the
/// failed-deployments retry fetches from) follows.
pub async fn run_sync_job(
    orchestrator: Arc<SyncOrchestrator>,
    mut peers: tokio::sync::watch::Receiver<HashSet<String>>,
    retry_peers: Arc<tokio::sync::RwLock<Vec<String>>>,
    ctx: JobContext,
    on_bootstrap: impl FnOnce(),
) -> anyhow::Result<()> {
    let initial = peers.borrow_and_update().clone();
    info!(peers = ?initial, "Sync orchestrator starting...");
    ctx.progress("starting");
    let handle = match orchestrator.sync_with_servers(initial).await {
        Ok(handle) => handle,
        Err(e) => {
            error!(error = %e, "Sync orchestrator failed to start; sync state is now Failed and every deploy is refused");
            orchestrator.status().set_state(SyncState::Failed);
            return Err(anyhow::anyhow!("sync orchestrator failed to start: {e}"));
        }
    };
    info!("Sync started, waiting for bootstrap...");
    ctx.progress("bootstrapping");
    let status = orchestrator.status();
    let signal = handle.bootstrap_signal();
    let mut bootstrapped = false;
    let mut peers_open = true;
    let mut on_bootstrap = Some(on_bootstrap);
    loop {
        tokio::select! {
            biased;
            _ = ctx.cancel.cancelled() => break,
            _ = signal.wait_settled(), if !bootstrapped => {
                bootstrapped = true;
                if !ctx.cancel.is_cancelled() && status.state() == SyncState::Syncing
                    && let Some(on_bootstrap) = on_bootstrap.take()
                {
                    on_bootstrap();
                }
                info!("Bootstrap complete, sync is now in steady state");
                ctx.progress(format!("bootstrapped; {}", status.state_name()));
            }
            changed = peers.changed(), if peers_open => {
                if changed.is_err() {
                    peers_open = false;
                    continue;
                }
                let next = peers.borrow_and_update().clone();
                info!(peers = ?next, "Sync peers changed");
                if let Err(e) = orchestrator.sync_with_servers(next.clone()).await {
                    warn!(error = %e, "Sync peers changed after the orchestrator stopped");
                    ctx.error(format!("peer change after stop: {e}"));
                    break;
                }
                let mut sorted: Vec<String> = next.into_iter().collect();
                sorted.sort();
                *retry_peers.write().await = sorted;
                ctx.progress(format!("peers changed; {}", status.state_name()));
            }
            outcome = handle.join() => {
                if let Err(e) = outcome
                    && !e.is_cancelled() {
                        error!(error = %e, "Bootstrap task died; sync state is now Failed and every deploy is refused");
                        status.set_state(SyncState::Failed);
                        ctx.error(format!("bootstrap task died: {e}"));
                        orchestrator.stop().await;
                        return Err(anyhow::anyhow!("bootstrap task died: {e}"));
                    }
                break;
            }
        }
    }
    ctx.progress("stopping");
    orchestrator.stop().await;
    Ok(())
}

#[derive(Clone)]
struct SyncOrchestratorRefs {
    config: SyncOrchestratorConfig,
    http_client: crate::http::CheckedRedirectClient,
    storage: Arc<Storage>,
    deployer: Arc<BatchDeployer>,
    processed_store: Arc<LiveProcessedSnapshotStore>,
    deployment_repo: Arc<LiveDeploymentRepository>,
    status: Arc<SyncStatus>,
    servers: Arc<Mutex<HashMap<String, ServerState>>>,
    bootstrap_done: Arc<Notify>,
    peers_changed: Arc<Notify>,
    cancel: CancellationToken,
    control: SyncControl,
    failed_snapshot_hashes: Arc<SyncMutex<HashSet<String>>>,
    unmarked_clean_snapshots: Arc<SyncMutex<HashSet<String>>>,
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

impl SyncOrchestratorConfig {
    /// Next stream reconnect delay: a run that ended cleanly, or settled a poll and stayed up
    /// for a full poll cycle, resets it; any other failed run grows it. A settled poll already
    /// implies a run of at least the poll wait, so the elapsed check only matters when
    /// `syncing_reconnect_time_ms` is the larger value.
    fn next_syncing_backoff(
        &self,
        backoff_ms: f64,
        failed: bool,
        settled_poll: bool,
        run: Duration,
    ) -> f64 {
        let healthy_run = Duration::from_millis(
            self.pointer_changes_wait_time_ms
                .max(self.syncing_reconnect_time_ms),
        );
        if !failed || (settled_poll && run >= healthy_run) {
            self.syncing_reconnect_time_ms as f64
        } else {
            backoff(
                backoff_ms,
                self.syncing_reconnect_exponent,
                self.syncing_max_reconnect_ms,
            )
        }
    }
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

    async fn until_stopped<T>(&self, future: impl Future<Output = T>) -> Result<T, SyncError> {
        tokio::select! {
            biased;
            _ = self.cancel.cancelled() => Err(SyncError::Stopped),
            outcome = future => Ok(outcome),
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

    async fn select_servers<T>(
        &self,
        mut f: impl FnMut(&str, &ServerState) -> Option<T>,
    ) -> Vec<T> {
        self.servers
            .lock()
            .await
            .iter()
            .filter_map(|(url, s)| f(url, s))
            .collect()
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

    /// A pass must cover a current peer generation before opening the deployment gate.
    /// Unreachable peers that were attempted stay eligible for background bootstrap retries.
    async fn announce(&self, peers: &[Peer], state: SyncState, message: &str) -> bool {
        let servers = self.servers.lock().await;
        if !peers.iter().any(|peer| !peer.cancel.is_cancelled()) {
            return false;
        }
        let held: Vec<_> = servers
            .iter()
            .filter_map(|(url, state)| (state.phase != ServerPhase::Syncing).then_some(url))
            .collect();
        if held.is_empty() {
            info!("{message}");
        } else {
            warn!(
                servers = ?held,
                "{message}; peers that did not finish bootstrap are retried in the background"
            );
        }
        self.set_state(state);
        true
    }

    async fn run_bootstrap(&self, mut peers: Vec<Peer>) -> Result<(), SyncError> {
        let phased = self.config.phased_sync
            && self.config.entity_types.as_ref().is_none_or(|allow| {
                allow.contains("profile") && NON_PROFILE_TYPES.iter().any(|t| allow.contains(*t))
            });
        let mut backoff_ms = self.bootstrap_backoff_start();
        loop {
            self.check_stopped()?;
            if peers.iter().all(|peer| peer.cancel.is_cancelled()) {
                peers = self
                    .select_servers(|url, state| {
                        Some(Peer {
                            url: url.to_owned(),
                            cancel: state.cancel.clone(),
                        })
                    })
                    .await;
                if peers.is_empty() {
                    self.until_stopped(self.peers_changed.notified()).await?;
                    continue;
                }
            }

            let result = if phased {
                self.run_phased_bootstrap(&peers).await
            } else {
                self.run_full_bootstrap(&peers).await
            };
            match result {
                Ok(()) => {
                    if self
                        .announce(
                            &peers,
                            SyncState::Syncing,
                            "Bootstrap complete, entering steady-state sync",
                        )
                        .await
                    {
                        break;
                    }
                }
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

        let next_resync = Instant::now() + self.config.snapshot_resync_initial_delay;
        self.defer_bootstrap_retries(&peers).await;
        drop(peers);
        self.maintain_peers(next_resync).await
    }

    async fn maintain_peers(&self, mut next_resync: Instant) -> Result<(), SyncError> {
        loop {
            self.wait_while_paused().await?;
            let now = Instant::now();
            if now >= next_resync {
                match self.resync_snapshots().await {
                    Err(SyncError::Stopped) => return Err(SyncError::Stopped),
                    Err(e) => warn!(error = %e, "Periodic snapshot sync failed"),
                    Ok(()) => {}
                }
                next_resync = Instant::now() + self.config.snapshot_resync_interval;
                continue;
            }

            let mut deadline = next_resync;
            let due = self
                .select_servers(|url, state| {
                    if state.phase == ServerPhase::Syncing {
                        return None;
                    }
                    if state.retry_at > now {
                        deadline = deadline.min(state.retry_at);
                        return None;
                    }
                    Some(Peer {
                        url: url.to_owned(),
                        cancel: state.cancel.clone(),
                    })
                })
                .await;
            if !due.is_empty() {
                match self.catch_up_peers(&due).await {
                    Err(SyncError::Stopped) => return Err(SyncError::Stopped),
                    Err(e) => warn!(error = %e, "Peer bootstrap failed; retrying"),
                    Ok(()) => {}
                }
                self.defer_bootstrap_retries(&due).await;
                continue;
            }

            tokio::select! {
                _ = self.cancel.cancelled() => return Err(SyncError::Stopped),
                _ = self.peers_changed.notified() => {}
                _ = tokio::time::sleep_until(deadline) => {}
            }
        }
    }

    async fn defer_bootstrap_retries(&self, peers: &[Peer]) {
        let now = Instant::now();
        let mut servers = self.servers.lock().await;
        for peer in peers {
            if peer.cancel.is_cancelled() {
                continue;
            }
            if let Some(state) = servers.get_mut(&peer.url)
                && state.phase != ServerPhase::Syncing
            {
                state.retry_at = now + Duration::from_millis(state.retry_delay_ms as u64);
                state.retry_delay_ms = self.bootstrap_backoff(state.retry_delay_ms);
            }
        }
        if servers
            .values()
            .all(|state| state.phase == ServerPhase::Syncing)
        {
            self.failed_snapshot_hashes.lock().clear();
            self.unmarked_clean_snapshots.lock().clear();
        }
    }

    /// A newly added peer, or a peer held by either startup slice, needs a pass covering all
    /// enabled types to prove its snapshots complete.
    async fn catch_up_peers(&self, peers: &[Peer]) -> Result<(), SyncError> {
        self.resume_servers_from_cursors(peers).await?;
        let allow = self.config.entity_types.as_ref();
        self.bootstrap_from_snapshots(peers, ServerPhase::BootstrappingSnapshots, allow, true)
            .await?;
        self.bootstrap_from_pointer_changes(peers, allow, true)
            .await?;
        let caught_up = {
            let servers = self.servers.lock().await;
            peers.iter().any(|peer| {
                !peer.cancel.is_cancelled()
                    && servers
                        .get(&peer.url)
                        .is_some_and(|state| state.phase == ServerPhase::Syncing)
            })
        };
        if caught_up {
            self.save_frontier().await;
            self.resolve_deleters().await;
            self.start_steady_state_sync().await?;
        }
        Ok(())
    }

    async fn resync_snapshots(&self) -> Result<(), SyncError> {
        let peers = self
            .select_servers(|url, state| {
                (state.phase == ServerPhase::Syncing).then(|| Peer {
                    url: url.to_owned(),
                    cancel: state.cancel.clone(),
                })
            })
            .await;
        if peers.is_empty() {
            return Ok(());
        }
        info!(peers = peers.len(), "Starting periodic snapshot sync");
        self.bootstrap_from_snapshots(
            &peers,
            ServerPhase::Syncing,
            self.config.entity_types.as_ref(),
            true,
        )
        .await?;
        self.resolve_deleters().await;
        Ok(())
    }

    /// From ITS OWN persisted cursor (`resume_point` is the rule): a server whose cursor lags the
    /// global frontier resumes from the lag, because the frontier is max-over-servers and riding it
    /// would skip that server's not-yet-deployed entities. A server with no cursor row keeps the
    /// configured floor, so its own snapshots (or, with none, its earliest pointer change) set
    /// where it starts.
    async fn resume_servers_from_cursors(&self, peers: &[Peer]) -> Result<(), SyncError> {
        for peer in peers {
            let cursor = tokio::select! {
                biased;
                _ = peer.cancel.cancelled() => continue,
                cursor = self.deployment_repo.get_server_sync_cursor(&peer.url) => cursor?,
            };
            let resume = resume_point(cursor);
            let mut servers = self.servers.lock().await;
            if resume > 0
                && !peer.cancel.is_cancelled()
                && let Some(state) = servers.get_mut(&peer.url)
            {
                info!(server = %peer.url, resume, "Resuming server from its persisted cursor");
                state.last_snapshot_timestamp = state.last_snapshot_timestamp.max(resume);
            }
        }
        self.check_stopped()
    }

    async fn run_full_bootstrap(&self, peers: &[Peer]) -> Result<(), SyncError> {
        self.resume_servers_from_cursors(peers).await?;
        let allow = self.config.entity_types.as_ref();

        info!("Phase 1: Bootstrap from snapshots");
        self.bootstrap_from_snapshots(peers, ServerPhase::BootstrappingSnapshots, allow, true)
            .await?;

        info!("Phase 2: Bootstrap from pointer-changes");
        self.bootstrap_from_pointer_changes(peers, allow, true)
            .await?;
        self.save_frontier().await;

        info!("Resolving deleter_deployment for overwritten entities");
        self.resolve_deleters().await;
        self.check_stopped()?;

        self.start_steady_state_sync().await
    }

    async fn run_phased_bootstrap(&self, peers: &[Peer]) -> Result<(), SyncError> {
        self.resume_servers_from_cursors(peers).await?;

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
        self.bootstrap_from_snapshots(
            peers,
            ServerPhase::BootstrappingSnapshots,
            Some(&non_profile_filter),
            false,
        )
        .await?;

        info!("Phase 2: Pointer-changes catch-up for all enabled types");
        self.bootstrap_from_pointer_changes(peers, self.config.entity_types.as_ref(), false)
            .await?;
        self.save_frontier().await;
        self.check_stopped()?;

        let mut ready_types: Vec<String> = non_profile_filter.iter().cloned().collect();
        ready_types.sort();
        self.announce(
            peers,
            SyncState::PartiallySynced { ready_types },
            "Phase 3: Partially synced \u{2014} non-profile types ready, starting to serve queries",
        )
        .await;

        {
            let mut servers = self.servers.lock().await;
            for peer in peers {
                if !peer.cancel.is_cancelled()
                    && let Some(state) = servers.get_mut(&peer.url)
                {
                    state.phase = ServerPhase::BootstrappingSnapshots;
                }
            }
            report_phases(&servers);
        }

        info!("Phase 4: Bootstrap profiles from snapshots");
        self.bootstrap_from_snapshots(
            peers,
            ServerPhase::BootstrappingSnapshots,
            Some(&profile_filter),
            true,
        )
        .await?;

        info!("Phase 5: Final pointer-changes catch-up for all enabled types");
        self.bootstrap_from_pointer_changes(peers, self.config.entity_types.as_ref(), true)
            .await?;
        self.save_frontier().await;

        info!("Phase 6: Resolving deleter_deployment for overwritten entities");
        self.resolve_deleters().await;
        self.check_stopped()?;

        self.start_steady_state_sync().await
    }

    async fn resolve_deleters(&self) {
        if let Ok(Err(e)) = self
            .until_stopped(self.deployment_repo.resolve_deleter_deployments())
            .await
        {
            warn!(error = %e, "Failed to resolve deleter_deployment");
        }
    }

    async fn save_frontier(&self) {
        let _ = self
            .until_stopped(
                self.deployment_repo
                    .set_sync_heartbeat(chrono::Utc::now().timestamp_millis()),
            )
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
        peers: &[Peer],
        phase: ServerPhase,
        entity_type_filter: Option<&HashSet<String>>,
        mark_processed: bool,
    ) -> Result<(), SyncError> {
        let eligible: HashMap<&str, &CancellationToken> = {
            let servers = self.servers.lock().await;
            peers
                .iter()
                .filter(|peer| {
                    !peer.cancel.is_cancelled()
                        && servers
                            .get(&peer.url)
                            .is_some_and(|state| state.phase == phase)
                })
                .map(|peer| (peer.url.as_str(), &peer.cancel))
                .collect()
        };
        if eligible.is_empty() {
            return Ok(());
        }
        self.wait_while_paused().await?;
        let _beat = heartbeat_while_busy(self.deployment_repo.clone());

        info!(
            servers = ?eligible.keys(),
            has_type_filter = entity_type_filter.is_some(),
            "Fetching peer snapshots"
        );

        let mut snapshots_by_hash = SnapshotsByHash::new();
        let mut last_ts_by_server: HashMap<String, Timestamp> = HashMap::new();
        let mut held_from_fetch = HashSet::new();

        for (&server, cancel) in &eligible {
            let fetched = tokio::select! {
                biased;
                _ = cancel.cancelled() => continue,
                result = snapshots::fetch_snapshots(
                    &self.http_client,
                    server,
                    self.config.request_max_retries,
                ) => result,
            };
            match fetched {
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
        self.check_stopped()?;

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
            .until_stopped(
                self.processed_store
                    .filter_processed_in_chunks(all_hashes, self.config.entity_types.as_ref()),
            )
            .await??;

        let (deployable, newly_marked) = snapshots::decide_snapshot_pass(
            &candidates,
            &mut processed,
            self.config.from_timestamp,
        );

        self.until_stopped(
            self.processed_store
                .mark_processed_many(&newly_marked, self.config.entity_types.as_ref()),
        )
        .await??;

        let mut time_ranges_to_deploy: Vec<TimeRange> = Vec::new();
        let mut snapshots_to_process: Vec<(String, HashSet<String>)> = Vec::new();

        // A snapshot this node generated itself (a row of its own
        // content.snapshots, content::snapshot_gen) lists entities it already
        // holds active: skipped, neither downloaded nor marked processed, as
        // upstream's snapshotStorage.has.
        let own = self
            .until_stopped(self.processed_store.own_snapshot_hashes(&deployable))
            .await??;
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

        let snapshot_concurrency = self.config.snapshot_concurrency.max(1);

        let pass: Vec<(String, HashSet<String>, Arc<DeploymentReport>)> = snapshots_to_process
            .into_iter()
            .map(|(hash, servers)| {
                let report = DeploymentReport::new(metrics::SyncSource::snapshots());
                (hash, servers, Arc::new(report))
            })
            .collect();
        let failed_hashes = SyncMutex::new(HashSet::<String>::new());

        if !time_ranges_to_deploy.is_empty() {
            self.deployer
                .prepare_for_deployments_in(&time_ranges_to_deploy)
                .await?;
        }

        futures::stream::iter(pass.iter())
            .for_each_concurrent(snapshot_concurrency, |(hash, servers, report)| {
                let (failed_hashes, eligible) = (&failed_hashes, &eligible);
                async move {
                    let advertisers: Vec<_> = servers
                        .iter()
                        .filter_map(|url| {
                            let cancel = eligible[url.as_str()];
                            (!cancel.is_cancelled()).then_some((url.as_str(), cancel))
                        })
                        .collect();
                    if advertisers.is_empty() {
                        failed_hashes.lock().insert(hash.clone());
                        return;
                    }
                    if let Err(e) = snapshots::deploy_entities_from_snapshot(
                        &self.http_client,
                        self.storage.as_ref(),
                        self.deployer.as_ref(),
                        snapshots::SnapshotSource {
                            snapshot_hash: hash,
                            advertisers: &advertisers,
                            max_retries: self.config.request_max_retries,
                            retry_wait_ms: self.config.request_retry_wait_ms,
                        },
                        snapshots::SnapshotFilter {
                            genesis_timestamp: self.config.from_timestamp,
                            entity_type_filter,
                        },
                        self.config.snapshot_scheduling,
                        report,
                    )
                    .await
                    {
                        warn!(snapshot_hash = %hash, error = %e, "Snapshot deployment failed");
                        failed_hashes.lock().insert(hash.clone());
                    }
                }
            })
            .await;

        self.check_stopped()?;
        self.until_stopped(self.deployer.on_idle()).await??;
        self.check_stopped()?;

        let mut failed_hashes = failed_hashes.into_inner();
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
            let mut failed_global = self.failed_snapshot_hashes.lock();
            let mut clean_unmarked = self.unmarked_clean_snapshots.lock();
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
                        to_mark.push(hash.clone());
                    }
                    SnapshotPassVerdict::Unproven => held.extend(servers.iter().cloned()),
                }
            }
            (held, to_mark)
        };

        self.until_stopped(
            self.processed_store
                .mark_processed_many(&to_mark, self.config.entity_types.as_ref()),
        )
        .await??;
        metrics::snapshots_processed("saved", to_mark.len() as u64);
        if !to_mark.is_empty() {
            let mut failed_global = self.failed_snapshot_hashes.lock();
            let mut clean_unmarked = self.unmarked_clean_snapshots.lock();
            for hash in to_mark {
                failed_global.remove(&hash);
                clean_unmarked.remove(&hash);
            }
        }

        // The live pointer stream retains its own cursor during periodic snapshot sync.
        if phase == ServerPhase::Syncing {
            return Ok(());
        }

        let mut advanced = Vec::new();
        {
            let mut servers = self.servers.lock().await;
            for (url, ts) in &last_ts_by_server {
                if eligible[url.as_str()].is_cancelled() {
                    continue;
                }
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
            report_phases(&servers);
        }

        if mark_processed {
            for (url, ts) in advanced {
                tokio::select! {
                    biased;
                    _ = eligible[url.as_str()].cancelled() => {},
                    _ = self.deployment_repo.advance_server_sync_cursor(&url, ts) => {},
                }
            }
        }

        self.check_stopped()
    }

    async fn bootstrap_from_pointer_changes(
        &self,
        peers: &[Peer],
        entity_type_filter: Option<&HashSet<String>>,
        persist_cursor: bool,
    ) -> Result<(), SyncError> {
        let bootstrapping: Vec<_> = {
            let servers = self.servers.lock().await;
            peers
                .iter()
                .filter(|peer| !peer.cancel.is_cancelled())
                .filter_map(|peer| {
                    let state = servers.get(&peer.url)?;
                    (state.phase == ServerPhase::BootstrappingPointerChanges).then(|| {
                        let from =
                            (state.last_snapshot_timestamp - POINTER_CHANGES_SHIFT_MS).max(0);
                        (peer, from)
                    })
                })
                .collect()
        };
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

        futures::stream::iter(bootstrapping)
            .for_each_concurrent(None, |(peer, from_timestamp)| async move {
                let server = &peer.url;
                let options = PointerChangesOptions {
                    from_timestamp,
                    wait_time_ms: 0,
                    entity_type_filter,
                };
                let progress = AtomicI64::new(from_timestamp);
                let settled_polls = AtomicU64::new(0);
                let report = Arc::new(DeploymentReport::new(metrics::SyncSource::pointer_changes(
                    server,
                )));
                let pass = pointer_changes::deploy_entities_from_pointer_changes(
                    &self.http_client,
                    server,
                    &options,
                    ScheduleTarget {
                        deployer: self.deployer.as_ref(),
                        report: &report,
                    },
                    persist_cursor.then(|| self.deployment_repo.clone()),
                    &progress,
                    &settled_polls,
                    || peer.cancel.is_cancelled(),
                );
                let outcome = tokio::select! {
                    biased;
                    _ = peer.cancel.cancelled() => return,
                    outcome = pass => outcome,
                };
                let reached = progress.load(Relaxed);
                let (ts, failed) = match outcome {
                    Ok(greatest_ts) => (greatest_ts, false),
                    Err(e) => {
                        warn!(server = %server, error = %e, "Pointer-changes bootstrap failed");
                        (reached, true)
                    }
                };
                let mut servers = self.servers.lock().await;
                if !peer.cancel.is_cancelled()
                    && let Some(state) = servers.get_mut(server)
                {
                    // A finished pass delivered every row before its settled cursor, while the
                    // snapshot's end can sit above rows the pass read before they committed:
                    // the stream resumes from the pass.
                    state.last_snapshot_timestamp = if failed {
                        state.last_snapshot_timestamp.max(ts)
                    } else {
                        ts
                    };
                    if !failed {
                        state.phase = ServerPhase::Syncing;
                    }
                }
                report_phases(&servers);
            })
            .await;

        self.check_stopped()?;
        self.until_stopped(self.deployer.on_idle()).await??;
        self.check_stopped()
    }

    /// Spawns a stream for every `Syncing` peer without one. Spawning and recording the handle
    /// happen under one lock, with no await between, so stop can join every producer.
    async fn start_steady_state_sync(&self) -> Result<(), SyncError> {
        let mut servers = self.servers.lock().await;
        self.check_stopped()?;
        for (server, state) in servers.iter_mut() {
            if state.phase != ServerPhase::Syncing || state.sync_task.is_some() {
                continue;
            }
            let refs = self.clone();
            let server = server.clone();
            let cancel = state.cancel.clone();
            let mut from_timestamp = state.last_snapshot_timestamp;

            let handle = tokio::spawn(async move {
                let cfg = &refs.config;
                let mut backoff_ms = cfg.syncing_reconnect_time_ms as f64;
                loop {
                    if refs.control.wait_while_paused(&cancel).await {
                        return;
                    }
                    let options = PointerChangesOptions {
                        from_timestamp,
                        wait_time_ms: cfg.pointer_changes_wait_time_ms,
                        entity_type_filter: cfg.entity_types.as_ref(),
                    };
                    let progress = AtomicI64::new(from_timestamp);
                    let report = Arc::new(DeploymentReport::new(
                        metrics::SyncSource::pointer_changes(&server),
                    ));
                    let settled_polls = AtomicU64::new(0);
                    let run_started = tokio::time::Instant::now();
                    metrics::stream_connected(&server);
                    let stream = pointer_changes::deploy_entities_from_pointer_changes(
                        &refs.http_client,
                        &server,
                        &options,
                        ScheduleTarget {
                            deployer: refs.deployer.as_ref(),
                            report: &report,
                        },
                        Some(refs.deployment_repo.clone()),
                        &progress,
                        &settled_polls,
                        || cancel.is_cancelled() || refs.control.is_paused(),
                    );
                    let outcome = tokio::select! {
                        biased;
                        _ = cancel.cancelled() => return,
                        outcome = async {
                            let outcome = stream.await;
                            let confirmed = progress.load(Relaxed);
                            let greatest = outcome.as_ref().copied().unwrap_or(confirmed);
                            refs.advance_cursor(&server, &mut from_timestamp, greatest.max(confirmed)).await;
                            outcome
                        } => outcome,
                    };
                    let failed = match outcome {
                        Ok(_) => false,
                        Err(e) => {
                            error!(server = %server, error = %e, "Sync stream failed");
                            metrics::stream_failed(&server);
                            true
                        }
                    };
                    backoff_ms = cfg.next_syncing_backoff(
                        backoff_ms,
                        failed,
                        settled_polls.load(Relaxed) > 0,
                        run_started.elapsed(),
                    );
                    {
                        let mut servers = refs.servers.lock().await;
                        if !cancel.is_cancelled()
                            && let Some(state) = servers.get_mut(&server)
                        {
                            state.last_snapshot_timestamp =
                                state.last_snapshot_timestamp.max(from_timestamp);
                        }
                    }
                    tokio::select! {
                        _ = cancel.cancelled() => return,
                        _ = tokio::time::sleep(Duration::from_millis(backoff_ms as u64)) => {}
                        _ = refs.control.changed() => {}
                    }
                }
            });
            state.sync_task = Some(handle);
        }

        Ok(())
    }
}

pub struct SyncHandle {
    bootstrap_done: Arc<Notify>,
    status: Arc<SyncStatus>,
    task: BootstrapTask,
}

impl SyncHandle {
    /// Waits on the node's sync state without borrowing the handle; shared by every handle the
    /// orchestrator returns.
    pub fn bootstrap_signal(&self) -> BootstrapSignal {
        BootstrapSignal {
            bootstrap_done: self.bootstrap_done.clone(),
            status: self.status.clone(),
        }
    }

    /// Waits for the shared bootstrap and snapshot maintenance task to exit.
    pub async fn join(&self) -> Result<(), Arc<JoinError>> {
        self.task.clone().await
    }
}

/// Waits for the node's bootstrap; see [`SyncHandle::bootstrap_signal`].
#[derive(Clone)]
pub struct BootstrapSignal {
    bootstrap_done: Arc<Notify>,
    status: Arc<SyncStatus>,
}

impl BootstrapSignal {
    /// Resolves once every type bootstrapped (`Syncing`) or the sync failed, even when that
    /// happened before this was called: a `PartiallySynced` node is still bootstrapping its
    /// profiles.
    pub async fn wait_settled(&self) {
        self.wait_for(|state| matches!(state, SyncState::Syncing | SyncState::Failed))
            .await
    }

    async fn wait_for(&self, done: impl Fn(&SyncState) -> bool) {
        loop {
            let notified = self.bootstrap_done.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if done(&self.status.state()) {
                return;
            }
            notified.await;
        }
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

/// The peers in each phase, for the `dcl_bootstrapping_servers` and `dcl_syncing_servers` gauges.
fn report_phases(servers: &HashMap<String, ServerState>) {
    let count = |phase| servers.values().filter(|s| s.phase == phase).count();
    metrics::server_phases(
        count(ServerPhase::BootstrappingSnapshots),
        count(ServerPhase::BootstrappingPointerChanges),
        count(ServerPhase::Syncing),
    );
}

/// Its OWN persisted cursor, even when that lags the global frontier, and zero for a server with
/// no cursor row (never synced, or newly added), leaving the caller's in-memory floor
/// (config.from_timestamp) in charge. The global frontier is never a starting point: it is
/// max-over-servers, so a new peer's entities older than it would never be requested.
pub fn resume_point(own_cursor: Option<Timestamp>) -> Timestamp {
    own_cursor.unwrap_or(0)
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
