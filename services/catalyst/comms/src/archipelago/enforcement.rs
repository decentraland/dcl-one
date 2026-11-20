use crate::archipelago::ban::BanChecker;
use crate::archipelago::config::ClusterConfig;
use crate::archipelago::registry::PeersRegistry;
use crate::gatekeeper::SharedState;
use crate::jobs::{JobContext, Jobs};

use dashmap::DashMap;
use futures::{FutureExt, stream::StreamExt};

use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::time::Duration;

const BAN_SWEEP_CONCURRENCY: usize = 20;

/// Wallet room assignments used only for moderation and participant removal.
/// A pre-heartbeat assignment survives until the last socket leaves.
pub struct Enforcement {
    assignments: DashMap<String, String>,
    cfg: ClusterConfig,
    /// The gatekeeper, whose LiveKit a kick removes the wallet's island participant through.
    gatekeeper: SharedState,
    ban_checker: Arc<BanChecker>,
    registry: Arc<PeersRegistry>,
}

impl Enforcement {
    pub fn new(
        cfg: ClusterConfig,
        gatekeeper: SharedState,
        ban_checker: Arc<BanChecker>,
        registry: Arc<PeersRegistry>,
    ) -> Arc<Self> {
        Arc::new(Self {
            assignments: DashMap::new(),
            cfg,
            gatekeeper,
            ban_checker,
            registry,
        })
    }

    /// Records what the feed told this wallet to join. Always recorded, even for a wallet whose
    /// first heartbeat has not arrived: the ban sweep needs the issued room for participant
    /// removal, and an unchanged assignment may never be repeated.
    pub fn set_island(&self, address: &str, island_id: &str) {
        self.assignments
            .insert(address.to_string(), island_id.to_string());
    }

    /// The live socket registry bounds assignments; disconnect and kick retire them.
    pub fn remove_peer(&self, address: &str) {
        self.assignments.remove(address);
    }

    /// Ends every session of a wallet on this replica and clears its retained room. The
    /// LiveKit participant is removed too, so a kick reaches the media plane and not only the
    /// signalling socket.
    pub fn kick_peer(&self, address: &str, reason: &str) {
        let room = self.island_of(address);
        tracing::info!(addr = %address, reason, "kicking peer");
        self.registry.kick(address);
        self.remove_peer(address);
        if let (Some(room), true) = (room, self.gatekeeper.connector_livekit_armed()) {
            let gatekeeper = Arc::clone(&self.gatekeeper);
            let addr = address.to_string();
            crate::shared::task::spawn(async move {
                if !gatekeeper.kick_island_participant(&room, &addr).await {
                    tracing::error!("island ban enforcement request was not persisted");
                }
            });
        }
    }

    pub async fn ban_sweep_once(&self) {
        let addrs = self.registry.wallets();
        if addrs.is_empty() {
            return;
        }
        let checker = Arc::clone(&self.ban_checker);
        let banned = futures::stream::iter(addrs)
            .map(|addr| {
                let checker = Arc::clone(&checker);
                async move { checker.is_banned(&addr).await.then_some(addr) }
            })
            .buffer_unordered(BAN_SWEEP_CONCURRENCY)
            .filter_map(|hit| async move { hit });
        futures::pin_mut!(banned);
        while let Some(addr) = banned.next().await {
            tracing::info!(addr = %addr, "ban sweep: evicting banned peer from comms");
            self.kick_peer(&addr, "banned");
        }
    }

    pub fn spawn_jobs(self: &Arc<Self>, jobs: &Jobs) {
        let this = Arc::clone(self);
        let period = Duration::from_millis(self.cfg.ban_sweep_interval_ms.max(1));
        let first_pass_now = jobs.first_pass_now();
        jobs.spawn("archipelago-ban-sweep", move |ctx| {
            run_ban_sweeps(ctx, period, first_pass_now, move || {
                let this = this.clone();
                async move {
                    this.ban_sweep_once().await;
                }
            })
        });
    }

    pub fn island_of(&self, address: &str) -> Option<String> {
        self.assignments.get(address).map(|v| v.value().clone())
    }
}

/// Consume overlapping ticks instead of moving the next tick to the end of a slow pass.
async fn run_ban_sweeps<F, Fut>(
    ctx: JobContext,
    period: Duration,
    first_pass_now: bool,
    mut sweep: F,
) -> anyhow::Result<()>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = ()>,
{
    let first = tokio::time::Instant::now()
        + if first_pass_now {
            Duration::ZERO
        } else {
            period
        };
    let mut ticks = tokio::time::interval_at(first, period);
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            biased;
            _ = ctx.cancel.cancelled() => return Ok(()),
            _ = ticks.tick() => {}
        }
        ctx.progress("checking connected-wallet bans");
        let pass = AssertUnwindSafe(sweep()).catch_unwind();
        tokio::pin!(pass);
        let mut stopping = false;
        let outcome = loop {
            tokio::select! {
                biased;
                // The active pass stays inside this job and its existing shutdown deadline.
                _ = ctx.cancel.cancelled(), if !stopping => stopping = true,
                _ = ticks.tick(), if !stopping => {
                    tracing::warn!("skipping ban sweep: the previous pass is still running");
                }
                outcome = &mut pass => break outcome,
            }
        };
        if outcome.is_err() {
            ctx.error("ban sweep panicked");
            tracing::error!("ban sweep panicked");
        }
        ctx.pass_done();
        if stopping {
            return Ok(());
        }
        ctx.progress("waiting for next ban sweep");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::{Semaphore, mpsc};
    use tokio::time::{Instant, advance};

    fn sweep_job(period_ms: u64) -> (Jobs, mpsc::UnboundedReceiver<Instant>, Arc<Semaphore>) {
        sweep_job_with_start(period_ms, false)
    }

    fn sweep_job_with_start(
        period_ms: u64,
        first_pass_now: bool,
    ) -> (Jobs, mpsc::UnboundedReceiver<Instant>, Arc<Semaphore>) {
        let jobs = Jobs::new().with_first_pass_now(first_pass_now);
        let (started, starts) = mpsc::unbounded_channel();
        let releases = Arc::new(Semaphore::new(0));
        let release = Arc::clone(&releases);
        let first_pass_now = jobs.first_pass_now();
        jobs.spawn("archipelago-ban-sweep", move |ctx| {
            run_ban_sweeps(
                ctx,
                Duration::from_millis(period_ms),
                first_pass_now,
                move || {
                    let started = started.clone();
                    let release = Arc::clone(&release);
                    async move {
                        started.send(Instant::now()).unwrap();
                        release.acquire().await.unwrap().forget();
                    }
                },
            )
        });
        (jobs, starts, releases)
    }

    async fn settle() {
        for _ in 0..4 {
            tokio::task::yield_now().await;
        }
    }

    #[tokio::test(start_paused = true)]
    async fn ban_sweep_first_pass_override_runs_immediately_without_resetting_cadence() {
        let before = Instant::now();
        let (jobs, mut starts, releases) = sweep_job_with_start(100, true);
        settle().await;
        assert_eq!(starts.try_recv().unwrap(), before);
        advance(Duration::from_millis(20)).await;
        releases.add_permits(1);
        settle().await;
        advance(Duration::from_millis(79)).await;
        settle().await;
        assert!(starts.try_recv().is_err());
        advance(Duration::from_millis(1)).await;
        settle().await;
        assert_eq!(
            starts.try_recv().unwrap().duration_since(before),
            Duration::from_millis(100)
        );
        releases.add_permits(1);
        assert!(jobs.shutdown(Duration::from_secs(1)).await.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn ban_sweep_waits_for_default_first_interval() {
        let before = Instant::now();
        let (jobs, mut starts, releases) =
            sweep_job(ClusterConfig::default().ban_sweep_interval_ms);
        settle().await;
        assert!(starts.try_recv().is_err());
        advance(Duration::from_millis(29_999)).await;
        settle().await;
        assert!(starts.try_recv().is_err());
        advance(Duration::from_millis(1)).await;
        settle().await;
        assert_eq!(
            starts.try_recv().unwrap().duration_since(before),
            Duration::from_secs(30)
        );
        releases.add_permits(1);
        assert!(jobs.shutdown(Duration::from_secs(1)).await.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn ban_sweep_subsecond_ticks_skip_overlap_without_resetting_cadence() {
        let before = Instant::now();
        let (jobs, mut starts, releases) = sweep_job(100);
        settle().await;
        advance(Duration::from_millis(99)).await;
        settle().await;
        assert!(starts.try_recv().is_err());
        advance(Duration::from_millis(1)).await;
        settle().await;
        assert_eq!(
            starts.try_recv().unwrap().duration_since(before),
            Duration::from_millis(100)
        );
        advance(Duration::from_millis(100)).await;
        settle().await;
        assert!(
            starts.try_recv().is_err(),
            "an unfinished sweep owns its concurrency budget"
        );
        advance(Duration::from_millis(50)).await;
        releases.add_permits(1);
        settle().await;
        assert!(
            starts.try_recv().is_err(),
            "the skipped tick must not catch up on completion"
        );
        advance(Duration::from_millis(49)).await;
        settle().await;
        assert!(starts.try_recv().is_err());
        advance(Duration::from_millis(1)).await;
        settle().await;
        assert_eq!(
            starts.try_recv().unwrap().duration_since(before),
            Duration::from_millis(300)
        );
        // Finishing quickly also leaves the timer anchored to its next scheduled tick.
        advance(Duration::from_millis(20)).await;
        releases.add_permits(1);
        settle().await;
        advance(Duration::from_millis(80)).await;
        settle().await;
        assert_eq!(
            starts.try_recv().unwrap().duration_since(before),
            Duration::from_millis(400)
        );
        releases.add_permits(1);
        assert!(jobs.shutdown(Duration::from_secs(1)).await.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn ban_sweep_missed_ticks_do_not_burst_or_accumulate_while_busy() {
        let before = Instant::now();
        let (jobs, mut starts, releases) = sweep_job(100);
        settle().await;
        advance(Duration::from_millis(550)).await;
        settle().await;
        assert_eq!(
            starts.try_recv().unwrap().duration_since(before),
            Duration::from_millis(550)
        );
        assert!(starts.try_recv().is_err());
        advance(Duration::from_millis(500)).await;
        settle().await;
        assert!(starts.try_recv().is_err());
        releases.add_permits(1);
        settle().await;
        assert!(starts.try_recv().is_err());
        advance(Duration::from_millis(49)).await;
        settle().await;
        assert!(starts.try_recv().is_err());
        advance(Duration::from_millis(1)).await;
        settle().await;
        assert_eq!(
            starts.try_recv().unwrap().duration_since(before),
            Duration::from_millis(1100)
        );
        releases.add_permits(1);
        assert!(jobs.shutdown(Duration::from_secs(1)).await.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn ban_sweep_cancel_before_first_tick_starts_no_pass() {
        let (jobs, mut starts, _releases) = sweep_job(100);
        settle().await;
        assert!(jobs.shutdown(Duration::from_secs(1)).await.is_empty());
        advance(Duration::from_millis(500)).await;
        assert!(starts.try_recv().is_err());
    }

    #[tokio::test(start_paused = true)]
    async fn ban_sweep_cancel_keeps_running_pass_owned_until_completion() {
        let (jobs, mut starts, releases) = sweep_job(100);
        settle().await;
        advance(Duration::from_millis(100)).await;
        settle().await;
        starts.try_recv().unwrap();
        jobs.root_token().cancel();
        advance(Duration::from_millis(500)).await;
        settle().await;
        assert!(starts.try_recv().is_err());
        assert_eq!(jobs.report()[0].state, crate::jobs::JobState::Running);
        releases.add_permits(1);
        assert!(jobs.shutdown(Duration::from_secs(1)).await.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn ban_sweep_stuck_pass_uses_existing_shutdown_deadline() {
        let (jobs, mut starts, _releases) = sweep_job(100);
        settle().await;
        advance(Duration::from_millis(100)).await;
        settle().await;
        starts.try_recv().unwrap();
        assert_eq!(
            jobs.shutdown(Duration::from_secs(1)).await,
            vec!["archipelago-ban-sweep"]
        );
        advance(Duration::from_millis(500)).await;
        settle().await;
        assert!(starts.try_recv().is_err());
    }
}
