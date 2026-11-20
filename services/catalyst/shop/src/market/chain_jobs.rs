//! The market chain index jobs: one supervisor per network,
//! `chain-walk-ethereum` and `chain-walk-matic`. Each walks the projections
//! this process has enabled on its network, every one under its own lease
//! named `<index job>-<network>` (`nft-index-matic`), so processes with
//! different projections enabled share a network: a projection is walked by
//! whoever holds its lease.
//!
//! Each lease acquisition gets a fresh index (the nft walk's memory never
//! outlives it, and an attempt that panicked leaves its index behind), its
//! own [`Holdings`] and backoff, and at most one attempt in flight, a whole
//! [`chain_walk::job_pass`] in its own task. A lease request is a task of its
//! own as well, run to its end. A projection keeps its own cadence: the
//! interval after an attempt that ran, its own doubling backoff after one
//! that failed, so one projection failing, waiting on its endpoint or
//! waiting for its lease never holds another back. As with a leased periodic
//! job, every lease is asked for at once and each acquisition's first attempt
//! waits the projection's first-pass delay (its jitter); a projection standing
//! by asks again after the lease poll plus its jitter. A lost lease ends its
//! acquisition, backoff included, as soon as the supervisor sees it. The
//! supervisor reports standby while it holds no lease.
//!
//! An attempt ends as soon as its lease is lost or the process stops, even
//! inside a fold: the supervisor collects it before the slot lets its lease
//! go or takes it again, and the attempts are the supervisor's tasks, so
//! aborting the supervisor aborts them. Writes an attempt had already
//! committed stay; the owner generation fences any later one.
//!
//! A process that runs no fleet jobs (`FLEET_JOBS_ENABLED=false`) takes no
//! lease, reads no chain and claims nothing. Without leases (one process),
//! every projection is walked and claimed on the pool.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures::FutureExt;
use futures::future::BoxFuture;
use sqlx::PgPool;
use tokio::task::JoinSet;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use super::MarketplaceState;
use super::chain_walk::{self, ChainIndex, ChainNet, Holdings, IndexSettings, PassOutcome};
use crate::jobs::{JobContext, JobState, Jobs, LeaseGuard, Leases, PeriodicCfg};

/// Decides whether an attempt walks now: `Ok(None)` walks, `Ok(Some(why))`
/// waits for the next interval, an error fails the attempt.
pub type Gate = Arc<dyn Fn() -> BoxFuture<'static, anyhow::Result<Option<String>>> + Send + Sync>;

/// A fresh index, for a new lease acquisition.
pub type MakeIndex = Arc<dyn Fn() -> Arc<dyn ChainIndex> + Send + Sync>;

/// One projection a supervisor walks.
pub struct Projection {
    /// The lease it is walked under: `<index job>-<network>`.
    pub lease: String,
    /// The endpoint it reads.
    pub url: String,
    /// Its cadence: the interval, the first pass's jitter and the backoff cap.
    pub cfg: PeriodicCfg,
    pub make: MakeIndex,
    /// Checked at the start of every attempt.
    pub gate: Option<Gate>,
}

/// The projections of one network and what their attempts read with.
pub struct Supervisor {
    /// `chain-walk-<network>`.
    pub name: String,
    pub pool: PgPool,
    pub http: crate::http::Outbound,
    pub settings: IndexSettings,
    pub projections: Vec<Projection>,
}

/// The job name of `net`'s supervisor.
pub fn job_name(net: ChainNet) -> String {
    format!("chain-walk-{}", net.db_name())
}

/// The lease of a projection: its index job's name and the network.
pub fn lease_name(job: &str, net: ChainNet) -> String {
    format!("{job}-{}", net.db_name())
}

/// Where a supervisor takes its leases: the process's [`Leases`] in
/// production. A request runs to its end once started: one cut short could
/// leave the lock taken in PostgreSQL and unrecorded here.
#[async_trait::async_trait]
pub trait TakeLease: Send + Sync {
    async fn try_acquire(&self, name: &str) -> sqlx::Result<Option<LeaseGuard>>;
    /// How long a projection standing by for the lease `name` (another
    /// process holds it, or it was lost) waits before asking again.
    fn standby_wait(&self, name: &str) -> Duration;
}

struct ProcessLeases(Arc<Leases>);

#[async_trait::async_trait]
impl TakeLease for ProcessLeases {
    async fn try_acquire(&self, name: &str) -> sqlx::Result<Option<LeaseGuard>> {
        self.0.try_acquire(name).await
    }
    fn standby_wait(&self, name: &str) -> Duration {
        self.0.settings().standby_wait(name)
    }
}

/// The standby wait of `name`; a process without leases never stands by.
fn standby_wait(leases: &Option<Arc<dyn TakeLease>>, name: &str) -> Duration {
    leases
        .as_ref()
        .map_or(Duration::from_secs(15), |l| l.standby_wait(name))
}

/// Spawns the supervisor as a job of `jobs`, taking `jobs`' leases.
pub fn spawn(jobs: &Jobs, supervisor: Supervisor) {
    let leases = jobs
        .leases()
        .cloned()
        .map(|l| Arc::new(ProcessLeases(l)) as Arc<dyn TakeLease>);
    spawn_with(jobs, supervisor, leases);
}

/// [`spawn`] taking its leases from `leases` (`None`: a process without
/// leases, which walks every projection and claims on the pool).
pub fn spawn_with(jobs: &Jobs, supervisor: Supervisor, leases: Option<Arc<dyn TakeLease>>) {
    let fleet = jobs.fleet_jobs();
    let cfgs: Vec<PeriodicCfg> = supervisor
        .projections
        .iter()
        .map(|p| jobs.periodic_cfg(p.cfg.clone()))
        .collect();
    jobs.spawn(supervisor.name.clone(), move |ctx| {
        run(ctx, supervisor, cfgs, leases, fleet)
    });
}

/// What one attempt came to.
enum Attempt {
    Ran(anyhow::Result<PassOutcome>),
    Waiting(String),
    /// Its lease was lost or the process is stopping.
    Interrupted,
    Panicked(String),
}

/// A task of the supervisor that ended.
enum Done {
    /// A lease request.
    Requested(sqlx::Result<Option<LeaseGuard>>),
    Attempted(Attempt),
    /// A task that panicked outside an attempt's own catch (a lease request)
    /// or was aborted.
    Crashed(String),
}

/// One lease acquisition of a projection: its index, holdings and backoff
/// end with it.
struct Acquisition {
    /// `None` in a process without leases.
    guard: Option<LeaseGuard>,
    lost: CancellationToken,
    number: u64,
    index: Arc<dyn ChainIndex>,
    holdings: Arc<Holdings>,
    failures: u32,
}

impl Acquisition {
    fn new(projection: &Projection, guard: Option<LeaseGuard>) -> Self {
        let (lost, number) = match &guard {
            Some(g) => (g.lost().clone(), g.control().acquisition()),
            None => (CancellationToken::new(), 0),
        };
        Self {
            guard,
            lost,
            number,
            index: (projection.make)(),
            holdings: Arc::new(Holdings::default()),
            failures: 0,
        }
    }

    async fn release(self) {
        if let Some(guard) = self.guard {
            guard.release().await;
        }
    }
}

#[derive(PartialEq)]
enum Phase {
    Idle,
    /// A lease request is in flight.
    Requesting,
    /// An attempt is in flight.
    Running,
}

struct Slot {
    acquisition: Option<Acquisition>,
    phase: Phase,
    /// The next attempt (this acquisition's cadence or backoff) or, with no
    /// acquisition, the next lease request.
    due: Instant,
    line: String,
}

async fn run(
    ctx: JobContext,
    supervisor: Supervisor,
    cfgs: Vec<PeriodicCfg>,
    leases: Option<Arc<dyn TakeLease>>,
    fleet: bool,
) -> anyhow::Result<()> {
    if !fleet {
        ctx.set_state(Some(JobState::StoodDown));
        ctx.progress("stood down: this process runs no fleet jobs");
        ctx.cancel.cancelled().await;
        return Ok(());
    }
    let supervisor = Arc::new(supervisor);
    // Every lease is asked for at once; each acquisition's first attempt
    // waits the projection's first-pass delay, as a leased periodic job does.
    let start = Instant::now();
    let mut slots: Vec<Slot> = supervisor
        .projections
        .iter()
        .map(|_| Slot {
            acquisition: None,
            phase: Phase::Idle,
            due: start,
            line: "asking for the lease".into(),
        })
        .collect();
    let mut tasks: JoinSet<Done> = JoinSet::new();
    let mut running: HashMap<tokio::task::Id, usize> = HashMap::new();
    // Stopping is terminal: nothing is dispatched once the token fires.
    while !ctx.cancel.is_cancelled() {
        for (i, ((projection, cfg), slot)) in supervisor
            .projections
            .iter()
            .zip(&cfgs)
            .zip(&mut slots)
            .enumerate()
        {
            if ctx.cancel.is_cancelled() {
                break;
            }
            if slot.phase != Phase::Idle {
                continue;
            }
            let now = Instant::now();
            if slot
                .acquisition
                .as_ref()
                .is_some_and(|a| a.lost.is_cancelled())
            {
                lost_lease(
                    projection,
                    slot,
                    now,
                    standby_wait(&leases, &projection.lease),
                );
            }
            if now < slot.due {
                continue;
            }
            if slot.acquisition.is_none() {
                let Some(leases) = &leases else {
                    slot.acquisition = Some(Acquisition::new(projection, None));
                    slot.due = now + cfg.first_delay(&projection.lease);
                    slot.line = "waiting for the first pass".into();
                    continue;
                };
                let (leases, name) = (leases.clone(), projection.lease.clone());
                let handle =
                    tasks.spawn(async move { Done::Requested(leases.try_acquire(&name).await) });
                running.insert(handle.id(), i);
                slot.phase = Phase::Requesting;
                slot.line = "asking for the lease".into();
                continue;
            }
            dispatch_attempt(&ctx, &supervisor, i, slot, &mut tasks, &mut running);
        }
        ctx.progress(progress_line(&supervisor, &slots));
        // Standing by while another process holds every lease.
        let holding = slots.iter().any(|s| s.acquisition.is_some());
        ctx.set_state((!holding).then_some(JobState::Standby));
        let next = slots
            .iter()
            .filter(|s| s.phase == Phase::Idle)
            .map(|s| s.due)
            .min();
        let idle_leases: Vec<CancellationToken> = slots
            .iter()
            .filter(|s| s.phase == Phase::Idle)
            .filter_map(|s| s.acquisition.as_ref())
            .map(|a| a.lost.clone())
            .collect();
        tokio::select! {
            biased;
            _ = ctx.cancel.cancelled() => break,
            Some(done) = tasks.join_next_with_id(), if !tasks.is_empty() => {
                settle(&ctx, &supervisor, &cfgs, &mut slots, &mut running, done, &leases, false).await;
            }
            _ = any_cancelled(idle_leases) => {}
            _ = sleep_until(next) => {}
        }
    }
    // Every attempt watches the process's token, and a lease request runs to
    // its end: collect them all (a lease taken meanwhile is let go), then let
    // the leases go.
    while let Some(done) = tasks.join_next_with_id().await {
        settle(
            &ctx,
            &supervisor,
            &cfgs,
            &mut slots,
            &mut running,
            done,
            &leases,
            true,
        )
        .await;
    }
    for slot in &mut slots {
        if let Some(acquisition) = slot.acquisition.take() {
            acquisition.release().await;
        }
    }
    Ok(())
}

fn dispatch_attempt(
    ctx: &JobContext,
    supervisor: &Arc<Supervisor>,
    i: usize,
    slot: &mut Slot,
    tasks: &mut JoinSet<Done>,
    running: &mut HashMap<tokio::task::Id, usize>,
) {
    let acquisition = slot
        .acquisition
        .as_ref()
        .expect("an acquisition to attempt under");
    let task = attempt(
        supervisor.clone(),
        i,
        acquisition.index.clone(),
        acquisition.holdings.clone(),
        acquisition.guard.as_ref().map(LeaseGuard::control),
        acquisition.lost.clone(),
        ctx.cancel.clone(),
        ctx.clone(),
    );
    let handle = tasks.spawn(task.map(Done::Attempted));
    running.insert(handle.id(), i);
    slot.phase = Phase::Running;
}

/// Ends an acquisition whose lease was lost: the slot asks for the lease
/// again after `standby`, and the next acquisition starts with no backoff.
fn lost_lease(projection: &Projection, slot: &mut Slot, now: Instant, standby: Duration) {
    if let Some(a) = slot.acquisition.take() {
        tracing::warn!(lease = %projection.lease, acquisition = a.number, "marketplace chain index: lease lost; standing by");
    }
    slot.due = now + standby;
    slot.line = "lease lost; standing by".into();
}

/// Resolves when any of `tokens` fires: an idle projection's lease lost
/// wakes the supervisor, so its acquisition ends without waiting for its
/// backoff. Pending when there are none.
async fn any_cancelled(tokens: Vec<CancellationToken>) {
    if tokens.is_empty() {
        return std::future::pending().await;
    }
    futures::future::select_all(tokens.iter().map(|t| t.cancelled().boxed())).await;
}

async fn sleep_until(at: Option<Instant>) {
    match at {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}

fn progress_line(supervisor: &Supervisor, slots: &[Slot]) -> String {
    supervisor
        .projections
        .iter()
        .zip(slots)
        .map(|(p, s)| format!("{}: {}", p.lease, s.line))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Records a task that ended and sets the slot's next step. While
/// `stopping`, a lease just taken is let go at once. It runs none of the
/// projection's code: a panicked attempt's index is dropped, not cleaned.
#[allow(
    clippy::too_many_arguments,
    reason = "the supervisor's state, the task and the mode"
)]
async fn settle(
    ctx: &JobContext,
    supervisor: &Supervisor,
    cfgs: &[PeriodicCfg],
    slots: &mut [Slot],
    running: &mut HashMap<tokio::task::Id, usize>,
    done: Result<(tokio::task::Id, Done), tokio::task::JoinError>,
    leases: &Option<Arc<dyn TakeLease>>,
    stopping: bool,
) {
    let (i, done) = match done {
        Ok((id, done)) => match running.remove(&id) {
            Some(i) => (i, done),
            None => return,
        },
        Err(err) => match running.remove(&err.id()) {
            Some(i) => (i, Done::Crashed(err.to_string())),
            None => return,
        },
    };
    let projection = &supervisor.projections[i];
    let cfg = &cfgs[i];
    let slot = &mut slots[i];
    let now = Instant::now();
    let standby = standby_wait(leases, &projection.lease);
    // A crashed lease request fails like one, and is asked again after the
    // standby wait; a crashed attempt is one that panicked.
    let requesting = slot.phase == Phase::Requesting;
    slot.phase = Phase::Idle;
    if let (Done::Crashed(msg), true) = (&done, requesting) {
        ctx.error(format!("{}: lease request: {msg}", projection.lease));
        tracing::error!(lease = %projection.lease, error = %msg, "marketplace chain index: a lease request failed");
        slot.due = now + standby;
        slot.line = "standby: the lease request failed".into();
        return;
    }
    let outcome = match done {
        Done::Requested(Ok(Some(guard))) => {
            if stopping {
                guard.release().await;
                return;
            }
            let acquisition = Acquisition::new(projection, Some(guard));
            tracing::info!(lease = %projection.lease, acquisition = acquisition.number, "marketplace chain index: lease taken");
            slot.acquisition = Some(acquisition);
            slot.due = now + cfg.first_delay(&projection.lease);
            slot.line = "lease taken; waiting for the first pass".into();
            return;
        }
        Done::Requested(Ok(None)) => {
            slot.due = now + standby;
            slot.line = "standby: another process holds the lease".into();
            return;
        }
        Done::Requested(Err(err)) => {
            ctx.error(format!("{}: lease: {err}", projection.lease));
            slot.due = now + standby;
            slot.line = "standby: the lease connection is unavailable".into();
            return;
        }
        Done::Attempted(outcome) => outcome,
        Done::Crashed(msg) => Attempt::Panicked(msg),
    };
    let Some(acquisition) = slot.acquisition.as_mut() else {
        return;
    };
    let number = acquisition.number;
    match outcome {
        Attempt::Ran(Ok(o)) => {
            ctx.pass_done();
            acquisition.failures = 0;
            slot.due = now + cfg.interval;
            slot.line = o.summary();
            tracing::info!(lease = %projection.lease, acquisition = number, summary = %slot.line, "marketplace chain index pass");
        }
        Attempt::Waiting(why) => {
            ctx.pass_done();
            acquisition.failures = 0;
            slot.due = now + cfg.interval;
            slot.line = why;
        }
        Attempt::Ran(Err(err)) => {
            ctx.pass_done();
            acquisition.failures += 1;
            let wait = cfg.wait_after_failure(acquisition.failures);
            slot.due = now + wait;
            slot.line = format!("failed; next in {} s", wait.as_secs());
            tracing::warn!(lease = %projection.lease, acquisition = number, error = %format!("{err:#}"), wait_ms = wait.as_millis() as u64, "marketplace chain index pass failed");
            ctx.error(format!("{}: {err:#}", projection.lease));
        }
        Attempt::Panicked(msg) => {
            ctx.pass_done();
            // A fresh index for the retry; the old one's state may be poisoned.
            acquisition.index = (projection.make)();
            acquisition.failures += 1;
            let wait = cfg.wait_after_failure(acquisition.failures);
            slot.due = now + wait;
            slot.line = format!("panicked; next in {} s", wait.as_secs());
            tracing::error!(lease = %projection.lease, acquisition = number, panic = %msg, "marketplace chain index pass panicked");
            ctx.error(format!("{}: panic: {msg}", projection.lease));
        }
        Attempt::Interrupted => {}
    }
    if acquisition.lost.is_cancelled() {
        lost_lease(projection, slot, now, standby);
    }
}

/// One attempt of projection `i`: its gate, then a whole job pass. It ends
/// at once when `lost` or `stop` fires, wherever it is waiting.
#[allow(
    clippy::too_many_arguments,
    reason = "the acquisition's parts, the tokens and the job's trace"
)]
async fn attempt(
    supervisor: Arc<Supervisor>,
    i: usize,
    index: Arc<dyn ChainIndex>,
    holdings: Arc<Holdings>,
    lease: Option<crate::jobs::LeaseControl>,
    lost: CancellationToken,
    stop: CancellationToken,
    ctx: JobContext,
) -> Attempt {
    let walk = async {
        let projection = &supervisor.projections[i];
        if let Some(gate) = &projection.gate {
            match gate().await {
                Ok(Some(why)) => return Attempt::Waiting(why),
                Ok(None) => {}
                Err(err) => return Attempt::Ran(Err(err)),
            }
        }
        let source = chain_walk::RpcSource::new(crate::chain_index::rpc::Rpc::at(
            &supervisor.http,
            &projection.url,
        ));
        let progress = |m: String| ctx.progress(format!("{}: {m}", projection.lease));
        Attempt::Ran(
            chain_walk::job_pass(
                &supervisor.pool,
                &source,
                index.as_ref(),
                &supervisor.settings,
                &holdings,
                lease.as_ref(),
                // The select below owns the interruption: it drops this
                // pass at once on a stop or a lost lease.
                None,
                &progress,
            )
            .await,
        )
    };
    let walk = std::panic::AssertUnwindSafe(walk).catch_unwind();
    tokio::select! {
        biased;
        _ = lost.cancelled() => Attempt::Interrupted,
        _ = stop.cancelled() => Attempt::Interrupted,
        outcome = walk => outcome.unwrap_or_else(|panic| Attempt::Panicked(panic_message(panic.as_ref()))),
    }
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

/// The projections `state` enables on `net`, each with its endpoint, as the
/// switches and endpoints configure them; `rentals` is the rentals index's
/// switch (`None`: on wherever a chain-1 endpoint is configured).
pub fn projections(
    state: &Arc<MarketplaceState>,
    net: ChainNet,
    rentals: Option<bool>,
) -> Vec<Projection> {
    let settings = &state.settings.market.indexes;
    let market_cfg = || {
        PeriodicCfg::every(settings.interval)
            .with_jitter(Duration::from_secs(1))
            .with_error_backoff(Duration::from_secs(30))
    };
    let mut out = Vec::new();
    let mut add =
        |job: &str, url: Option<String>, make: MakeIndex, gate: Option<Gate>, cfg: PeriodicCfg| {
            if let Some(url) = url {
                out.push(Projection {
                    lease: lease_name(job, net),
                    url,
                    cfg,
                    make,
                    gate,
                });
            }
        };
    if settings.nft_enabled {
        let gate: Gate = {
            let state = state.clone();
            Arc::new(move || {
                let state = state.clone();
                async move {
                    Ok(
                        (!super::nft_index::mirror_complete(&state.pool, net).await).then(|| {
                            format!(
                                "nft {}: waiting for the collections mirror's first complete pass",
                                net.db_name()
                            )
                        }),
                    )
                }
                .boxed()
            })
        };
        add(
            "nft-index",
            state.chain_rpc_url(net),
            Arc::new(move || Arc::new(super::nft_index::NftIndex::new(net))),
            Some(gate),
            market_cfg(),
        );
    }
    if settings.sales_enabled && net == ChainNet::Matic {
        let url = state.chain_rpc_url(net);
        let gate: Gate = {
            let (state, url) = (state.clone(), url.clone().unwrap_or_default());
            let venues_checked = Arc::new(AtomicBool::new(false));
            Arc::new(move || {
                let (state, url, venues_checked) =
                    (state.clone(), url.clone(), venues_checked.clone());
                async move {
                    if !venues_checked.load(Ordering::SeqCst) {
                        let rpc = crate::chain_index::rpc::Rpc::at(&state.http, &url);
                        super::sales_index::check_venues(&rpc, &state.pool).await?;
                        venues_checked.store(true, Ordering::SeqCst);
                    }
                    Ok((!super::nft_index::mirror_complete(&state.pool, net).await).then(|| {
                        "sales matic: waiting for the collections mirror's first complete pass".to_string()
                    }))
                }
                .boxed()
            })
        };
        add(
            "sales-index",
            url,
            Arc::new(|| Arc::new(super::sales_index::SalesIndex)),
            Some(gate),
            market_cfg(),
        );
    }
    if settings.trade_events_enabled {
        let chain_id = settings.chain_id(net);
        add(
            "trade-events-index",
            state.trade_events_rpc_url(net),
            Arc::new(move || Arc::new(super::trade_events::TradeEventsIndex { net, chain_id })),
            None,
            market_cfg(),
        );
    }
    if settings.committee_enabled {
        add(
            "committee-index",
            state.committee_rpc_url(net),
            Arc::new(|| Arc::new(super::committee_index::CommitteeIndex)),
            None,
            market_cfg(),
        );
    }
    if settings.mana_transfers_enabled {
        add(
            "mana-transfers-index",
            state.chain_rpc_url(net),
            Arc::new(move || Arc::new(super::mana_transfers::ManaTransfersIndex { net })),
            None,
            market_cfg(),
        );
    }
    // The rentals venue is mainnet's: off mainnet ETH_RPC_URL serves another chain.
    if net == ChainNet::Ethereum && rentals != Some(false) {
        add(
            "rentals-index",
            state.venue_rpc_url(1),
            Arc::new(|| Arc::new(crate::rentals::index::RentalsIndex::mainnet())),
            None,
            PeriodicCfg::every(settings.interval)
                .with_jitter(Duration::from_secs(20))
                .with_error_backoff(Duration::from_secs(1800)),
        );
    }
    out
}

/// Spawns `chain-walk-ethereum` and `chain-walk-matic` for the projections
/// `state` enables, logging each switch that is on with no endpoint for it.
pub fn spawn_jobs(state: &Arc<MarketplaceState>, jobs: &Jobs, rentals: Option<bool>) {
    log_switches(state, rentals);
    for net in ChainNet::ALL {
        let projections = projections(state, net, rentals);
        if projections.is_empty() {
            continue;
        }
        tracing::info!(
            job = %job_name(net),
            leases = ?projections.iter().map(|p| p.lease.as_str()).collect::<Vec<_>>(),
            "marketplace chain index supervisor"
        );
        spawn(
            jobs,
            Supervisor {
                name: job_name(net),
                pool: state.pool.clone(),
                http: state.http.clone(),
                settings: state.settings.market.indexes.clone(),
                projections,
            },
        );
    }
}

/// Logs each index switch that is off, or on with no endpoint to read.
fn log_switches(state: &Arc<MarketplaceState>, rentals: Option<bool>) {
    let settings = &state.settings.market.indexes;
    let any_url =
        |f: &dyn Fn(ChainNet) -> Option<String>| ChainNet::ALL.into_iter().any(|n| f(n).is_some());
    if !settings.sales_enabled {
        tracing::info!(
            job = "sales-index",
            "marketplace chain index off: its routes answer the unavailable envelope"
        );
    } else if state.chain_rpc_url(ChainNet::Matic).is_none() {
        tracing::warn!(
            job = "sales-index",
            "marketplace chain index enabled but no Polygon JSON-RPC endpoint is configured (TRADE_RPC_URLS 137=): it stays off"
        );
    }
    for (enabled, job, has_url) in [
        (
            settings.nft_enabled,
            "nft-index",
            any_url(&|n| state.chain_rpc_url(n)),
        ),
        (
            settings.trade_events_enabled,
            "trade-events-index",
            any_url(&|n| state.trade_events_rpc_url(n)),
        ),
        (
            settings.mana_transfers_enabled,
            "mana-transfers-index",
            any_url(&|n| state.chain_rpc_url(n)),
        ),
    ] {
        if !enabled {
            tracing::info!(
                job,
                "marketplace chain index off: its routes answer the unavailable envelope"
            );
        } else if !has_url {
            tracing::warn!(
                job,
                "marketplace chain index enabled but no JSON-RPC endpoint is configured (TRADE_RPC_URLS 1= / 137=, or ETH_RPC_URL): it stays off"
            );
        }
    }
    if !settings.committee_enabled {
        tracing::info!(
            job = "committee-index",
            "committee index off: builder's committee routes admit only BUILDER_ADMIN_ADDRESSES"
        );
    } else if state.committee_rpc_url(ChainNet::Matic).is_none() {
        tracing::warn!(
            job = "committee-index",
            "committee index enabled but no Polygon JSON-RPC endpoint is configured (TRADE_RPC_URLS 137=, or RPC_ENDPOINT_POLYGON on mainnet): it stays off"
        );
    }
    if rentals == Some(false) {
        tracing::info!(
            "rentals index off (SHOP_RENTALS_INDEX_ENABLED=false): rental listings keep the status they were posted with"
        );
    } else if state.venue_rpc_url(1).is_none() {
        let why = "no chain-1 JSON-RPC endpoint is configured (TRADE_RPC_URLS 1=, or ETH_RPC_URL on mainnet)";
        if rentals == Some(true) {
            tracing::warn!("rentals index enabled but {why}: it stays off");
        } else {
            tracing::info!("rentals index off: {why}");
        }
    }
}
