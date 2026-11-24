//! The market chain index jobs: one supervisor per network,
//! `chain-walk-ethereum` and `chain-walk-matic`. Each walks the projections
//! this process has enabled on its network, every one under its own lease
//! named `<index job>-<network>` (`nft-index-matic`), so processes with
//! different projections enabled share a network: a projection is walked by
//! whoever holds its lease.
//!
//! Each lease acquisition gets a fresh index (the nft walk's memory never
//! outlives it, and an attempt that panicked leaves its index behind), its
//! own [`Holdings`] and backoff, and at most one task in flight: an attempt
//! alone (a whole [`chain_walk::job_pass`]), its part in a shared walk
//! ([`chain_group::walk_together`]), or the close or walks-alone read that
//! follows its part. A lease request is a task of its own as well, run to
//! its end. A projection keeps its own cadence: the interval after an
//! attempt that ran, its own doubling backoff after one that failed, so one
//! projection failing, waiting on its endpoint or waiting for its lease
//! never holds another back; one that reached the target may join a due
//! companion's shared walk ahead of its own turn. As with a leased periodic
//! job, every lease is asked for at once and each acquisition's first attempt
//! waits the projection's first-pass delay (its jitter); a projection standing
//! by asks again after the lease poll plus its jitter. A lost lease ends its
//! acquisition, backoff included, as soon as the supervisor sees it. The
//! supervisor reports standby once a lease request was answered without its
//! lease and it holds none.
//!
//! An attempt ends as soon as its lease is lost or the process stops, even
//! inside a fold, and a shared walk ends for every member once one member's
//! lease is lost (each reads its cursor again with a fresh index): the
//! supervisor collects it before the slot lets its lease
//! go or takes it again, and the attempts are the supervisor's tasks, so
//! aborting the supervisor aborts them. Writes an attempt had already
//! committed stay; the owner generation fences any later one.
//!
//! A process that runs no fleet jobs (`FLEET_JOBS_ENABLED=false`) takes no
//! lease, reads no chain and claims nothing. Without leases (one process),
//! every projection is walked and claimed on the pool.
//!
//! With `CHAIN_LOGS_SOURCE=shadow`, each network also runs a shadow worker
//! (`chain-logs-shadow-<network>`): the windows its walks accept are compared
//! with a portal in the background ([`crate::chain_index::portal::shadow`]).
//! The walks read and commit RPC data only, as without it.

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
use super::chain_group::{self, Checkpoint, Member, MemberEnd};
use super::chain_walk::{self, Budget, ChainIndex, ChainNet, Holdings, IndexSettings, PassOutcome};
use crate::chain_index::portal::admit::{Admission, AdmittedPortal};
use crate::chain_index::portal::shadow::{self, Shadow, Shadowed};
use crate::chain_index::portal::{AddressHistory, PortalReader, Secret};
use crate::jobs::{JobContext, JobState, Jobs, LeaseGuard, Leases, PeriodicCfg};

/// How the walks use the portal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortalMode {
    /// `CHAIN_LOGS_SOURCE=shadow`: accepted windows are compared with it.
    Shadow,
    /// `CHAIN_LOGS_SOURCE=portal`: windows it admits are read from it
    /// ([`Admission`]), each admission and portal phase within `deadline`.
    Source { deadline: Duration },
}

/// `CHAIN_LOGS_SOURCE=shadow|portal`: the portal and how the walks use it.
/// `None` (`rpc`) reads everything from the RPC.
#[derive(Clone, Debug)]
pub struct PortalSettings {
    pub mode: PortalMode,
    pub origin: String,
    pub dataset_ethereum: String,
    pub dataset_matic: String,
    pub key: Option<Secret>,
    pub max_rps: f64,
    pub history: AddressHistory,
}

impl PortalSettings {
    /// The settings as the environment spells them: `None` for `rpc`; for
    /// `shadow` or `portal`, a portal origin and two known datasets, refused
    /// at startup otherwise.
    #[allow(
        clippy::too_many_arguments,
        reason = "one argument per environment variable it validates"
    )]
    pub fn parse(
        source: &str,
        origin: Option<&str>,
        dataset_ethereum: &str,
        dataset_matic: &str,
        key: Option<String>,
        max_rps: f64,
        assert_full_history: bool,
        deadline_secs: u64,
    ) -> anyhow::Result<Option<Self>> {
        let mode = match source {
            "rpc" => return Ok(None),
            "shadow" => PortalMode::Shadow,
            "portal" => {
                anyhow::ensure!(
                    (1..=600).contains(&deadline_secs),
                    "CHAIN_LOGS_PORTAL_DEADLINE_SECS must be between 1 and 600"
                );
                PortalMode::Source {
                    deadline: Duration::from_secs(deadline_secs),
                }
            }
            other => anyhow::bail!("CHAIN_LOGS_SOURCE={other}: rpc, shadow or portal"),
        };
        let Some(origin) = origin.map(str::trim).filter(|o| !o.is_empty()) else {
            anyhow::bail!("CHAIN_LOGS_SOURCE={source} needs CHAIN_LOGS_PORTAL_URL");
        };
        anyhow::ensure!(
            max_rps.is_finite() && max_rps > 0.0,
            "CHAIN_LOGS_PORTAL_MAX_RPS must be above 0"
        );
        let settings = Self {
            mode,
            origin: origin.to_string(),
            dataset_ethereum: dataset_ethereum.trim().to_string(),
            dataset_matic: dataset_matic.trim().to_string(),
            key: key.and_then(Secret::new),
            max_rps,
            history: if assert_full_history {
                AddressHistory::AssertedByOperator
            } else {
                AddressHistory::Unknown
            },
        };
        for net in ChainNet::ALL {
            let reader = settings.reader(net).map_err(anyhow::Error::msg)?;
            anyhow::ensure!(
                reader.chain() == chain_walk::mainnet_chain(net) as u64,
                "the {} portal dataset {:?} is not {} mainnet",
                net.db_name(),
                reader.dataset(),
                net.db_name()
            );
        }
        Ok(Some(settings))
    }

    /// The reader of `net`'s dataset.
    pub fn reader(&self, net: ChainNet) -> Result<PortalReader, String> {
        let dataset = match net {
            ChainNet::Ethereum => &self.dataset_ethereum,
            ChainNet::Matic => &self.dataset_matic,
        };
        PortalReader::new(&self.origin, dataset, self.key.clone(), self.max_rps)
    }
}

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
    /// How long a projection's own step in a shared walk (its gate, its
    /// opening, its filters, its commit) may take before it leaves the walk.
    pub member_deadline: Duration,
    /// What the shared walks cost, summed.
    pub stats: Arc<SharedStats>,
    /// Where accepted windows are compared (`CHAIN_LOGS_SOURCE=shadow`).
    pub shadow: Option<Arc<Shadow>>,
    /// The portal windows are read from where it admits them
    /// (`CHAIN_LOGS_SOURCE=portal`).
    pub portal: Option<Arc<Admission>>,
}

/// The shared walks of a supervisor, summed: how many ran, the requests they
/// attempted (counted as each is sent, whatever its outcome; the members'
/// closes are counted in their own outcomes), how many ended in a fallback
/// or were cut short, and the members that left a walk on their deadline or
/// a panic.
#[derive(Debug, Default)]
pub struct SharedStats {
    pub walks: std::sync::atomic::AtomicUsize,
    pub requests: std::sync::atomic::AtomicUsize,
    pub fallbacks: std::sync::atomic::AtomicUsize,
    pub interrupted: std::sync::atomic::AtomicUsize,
    pub cut_short: std::sync::atomic::AtomicUsize,
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
    /// A projection's attempt alone, or its close after a shared walk, where
    /// its cursor stood after it, and whether it still walks alone (a
    /// backfill owed).
    Attempted(Attempt, Option<Checkpoint>, bool),
    /// A shared walk, member by member.
    Grouped(Vec<(usize, MemberEnd)>),
    /// A shared walk cut short by a lost lease or the stop.
    GroupInterrupted,
    /// A task that panicked outside its own catch (a lease request, a shared
    /// walk's own steps) or was aborted.
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
    /// An attempt, a shared walk or a close is in flight.
    Running,
}

/// A task in flight: the slots it runs for and, for a shared walk, the count
/// of the requests it attempts.
struct Running {
    members: Vec<usize>,
    sent: Option<Arc<std::sync::atomic::AtomicUsize>>,
}

impl Running {
    fn one(slot: usize) -> Self {
        Self {
            members: vec![slot],
            sent: None,
        }
    }
}

struct Slot {
    acquisition: Option<Acquisition>,
    phase: Phase,
    /// The next attempt (this acquisition's cadence or backoff) or, with no
    /// acquisition, the next lease request.
    due: Instant,
    line: String,
    /// Its last lease request was answered without the lease: another
    /// process holds it, or the request failed.
    stood_by: bool,
    /// Where its cursor stood after its last attempt, when known: a shared
    /// walk gathers the projections standing at the same one.
    checkpoint: Option<Checkpoint>,
    /// Its last attempt walked to the target: it may join a shared walk
    /// before it is due.
    invitable: bool,
    /// Its next attempt walks alone (backfill lanes owed, a rebuild asked,
    /// filters a shared read does not take, a fallback). Cleared by an
    /// attempt alone on a whole budget.
    solo_once: bool,
    /// What it may still spend, after a shared walk it fell back from.
    carry: Option<Budget>,
    /// Its attempt in flight runs on a carried budget.
    carried: bool,
}

impl Slot {
    /// Its state was cut short: a fresh index, its checkpoint read again.
    fn suspect(&mut self, projection: &Projection) {
        if let Some(a) = self.acquisition.as_mut() {
            a.index = (projection.make)();
        }
        self.checkpoint = None;
        self.invitable = false;
    }
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
            stood_by: false,
            checkpoint: None,
            invitable: false,
            solo_once: false,
            carry: None,
            carried: false,
        })
        .collect();
    let mut tasks: JoinSet<Done> = JoinSet::new();
    let mut running: HashMap<tokio::task::Id, Running> = HashMap::new();
    // Stopping is terminal: nothing is dispatched once the token fires.
    while !ctx.cancel.is_cancelled() {
        for i in 0..slots.len() {
            if ctx.cancel.is_cancelled() {
                break;
            }
            let projection = &supervisor.projections[i];
            if slots[i].phase != Phase::Idle {
                continue;
            }
            let now = Instant::now();
            if slots[i]
                .acquisition
                .as_ref()
                .is_some_and(|a| a.lost.is_cancelled())
            {
                lost_lease(
                    projection,
                    &mut slots[i],
                    now,
                    standby_wait(&leases, &projection.lease),
                );
            }
            if now < slots[i].due {
                continue;
            }
            if slots[i].acquisition.is_none() {
                let slot = &mut slots[i];
                let Some(leases) = &leases else {
                    slot.acquisition = Some(Acquisition::new(projection, None));
                    slot.due = now + cfgs[i].first_delay(&projection.lease);
                    slot.line = "waiting for the first pass".into();
                    continue;
                };
                let (leases, name) = (leases.clone(), projection.lease.clone());
                let handle =
                    tasks.spawn(async move { Done::Requested(leases.try_acquire(&name).await) });
                running.insert(handle.id(), Running::one(i));
                slot.phase = Phase::Requesting;
                slot.line = "asking for the lease".into();
                continue;
            }
            let others = companions(&supervisor, &slots, i, now);
            if others.is_empty() {
                dispatch_attempt(
                    &ctx,
                    &supervisor,
                    i,
                    &mut slots[i],
                    &mut tasks,
                    &mut running,
                );
            } else {
                let mut members = vec![i];
                members.extend(others);
                dispatch_group(
                    &ctx,
                    &supervisor,
                    members,
                    &mut slots,
                    &mut tasks,
                    &mut running,
                );
            }
        }
        ctx.progress(progress_line(&supervisor, &slots));
        // Standing by once a request was answered without its lease and no
        // lease is held: not while the first requests are in flight, nor
        // right after a lease was lost.
        let holding = slots.iter().any(|s| s.acquisition.is_some());
        let stood_by = slots.iter().any(|s| s.stood_by);
        ctx.set_state((!holding && stood_by).then_some(JobState::Standby));
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
                settle(&ctx, &supervisor, &cfgs, &mut slots, &mut tasks, &mut running, done, &leases, false).await;
            }
            _ = any_cancelled(idle_leases) => {}
            _ = sleep_until(next) => {}
        }
    }
    // Every attempt watches the process's token, and a lease request runs to
    // its end: collect them all (a lease taken meanwhile is let go, no close
    // starts), then let the leases go.
    while let Some(done) = tasks.join_next_with_id().await {
        settle(
            &ctx,
            &supervisor,
            &cfgs,
            &mut slots,
            &mut tasks,
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

/// The projections that walk with slot `i`, due now: idle ones holding their
/// lease, out of backoff, reading the same endpoint and standing at the same
/// checkpoint, and either due as well or ahead of their turn after an attempt
/// that reached the target. None when `i` walks alone this time.
fn companions(supervisor: &Supervisor, slots: &[Slot], i: usize, now: Instant) -> Vec<usize> {
    let slot = &slots[i];
    if slot.solo_once || slot.carry.is_some() {
        return Vec::new();
    }
    let Some(at) = slot.checkpoint.as_ref() else {
        return Vec::new();
    };
    let url = &supervisor.projections[i].url;
    (0..slots.len())
        .filter(|&j| j != i)
        .filter(|&j| {
            let other = &slots[j];
            other.phase == Phase::Idle
                && (other.due <= now || other.invitable)
                && !other.solo_once
                && other.carry.is_none()
                && other
                    .acquisition
                    .as_ref()
                    .is_some_and(|a| !a.lost.is_cancelled() && a.failures == 0)
                && supervisor.projections[j].url == *url
                && other.checkpoint.as_ref() == Some(at)
        })
        .collect()
}

fn dispatch_attempt(
    ctx: &JobContext,
    supervisor: &Arc<Supervisor>,
    i: usize,
    slot: &mut Slot,
    tasks: &mut JoinSet<Done>,
    running: &mut HashMap<tokio::task::Id, Running>,
) {
    let acquisition = slot
        .acquisition
        .as_ref()
        .expect("an acquisition to attempt under");
    let carried = slot.carry.take();
    slot.carried = carried.is_some();
    let budget = carried.unwrap_or_else(|| Budget::fresh(&supervisor.settings));
    let task = attempt(
        supervisor.clone(),
        i,
        acquisition.index.clone(),
        acquisition.holdings.clone(),
        acquisition.guard.as_ref().map(LeaseGuard::control),
        budget,
        acquisition.lost.clone(),
        ctx.cancel.clone(),
        ctx.clone(),
    );
    let handle = tasks.spawn(task);
    running.insert(handle.id(), Running::one(i));
    slot.phase = Phase::Running;
}

/// A shared walk of `members` (the first is due) from its checkpoint, cut
/// short when the stop comes or any member's lease is lost.
fn dispatch_group(
    ctx: &JobContext,
    supervisor: &Arc<Supervisor>,
    members: Vec<usize>,
    slots: &mut [Slot],
    tasks: &mut JoinSet<Done>,
    running: &mut HashMap<tokio::task::Id, Running>,
) {
    let at = slots[members[0]]
        .checkpoint
        .clone()
        .expect("a group gathers at a checkpoint");
    let url = supervisor.projections[members[0]].url.clone();
    let mut lost = Vec::new();
    let walkers: Vec<Member> = members
        .iter()
        .map(|&j| {
            let a = slots[j]
                .acquisition
                .as_ref()
                .expect("a member holds its lease");
            lost.push(a.lost.clone());
            Member {
                slot: j,
                index: a.index.clone(),
                holdings: a.holdings.clone(),
                lease: a.guard.as_ref().map(LeaseGuard::control),
                gate: supervisor.projections[j].gate.clone(),
            }
        })
        .collect();
    let names: Vec<&str> = members
        .iter()
        .map(|&j| supervisor.projections[j].lease.as_str())
        .collect();
    let line = format!("walking together: {}", names.join(", "));
    let (sup, stop) = (supervisor.clone(), ctx.cancel.clone());
    // The walk's count lives with the supervisor, so it outlives the task
    // even when the task panics.
    let sent = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = sent.clone();
    let handle = tasks.spawn(async move {
        let walk = chain_group::walk_together(
            &sup.pool,
            &sup.http,
            &url,
            &sup.settings,
            walkers,
            &at,
            sup.member_deadline,
            &counted,
            sup.shadow.as_deref(),
            sup.portal.as_deref(),
        );
        tokio::select! {
            biased;
            _ = stop.cancelled() => Done::GroupInterrupted,
            _ = any_cancelled(lost) => Done::GroupInterrupted,
            ends = walk => Done::Grouped(ends),
        }
    });
    for &j in &members {
        slots[j].phase = Phase::Running;
        slots[j].line = line.clone();
    }
    running.insert(
        handle.id(),
        Running {
            members,
            sent: Some(sent),
        },
    );
}

/// The close of slot `i` after a shared walk: its fold and publish, in a
/// task of its own so a slow fold holds back only its projection.
#[allow(
    clippy::too_many_arguments,
    reason = "the supervisor's state, the slot and what its walk left"
)]
fn dispatch_close(
    ctx: &JobContext,
    supervisor: &Arc<Supervisor>,
    i: usize,
    slot: &mut Slot,
    out: PassOutcome,
    close_options: chain_walk::Close,
    tasks: &mut JoinSet<Done>,
    running: &mut HashMap<tokio::task::Id, Running>,
) {
    let acquisition = slot.acquisition.as_ref().expect("a member holds its lease");
    let (sup, index, holdings, lease, lost, stop, checkpoint) = (
        supervisor.clone(),
        acquisition.index.clone(),
        acquisition.holdings.clone(),
        acquisition.guard.as_ref().map(LeaseGuard::control),
        acquisition.lost.clone(),
        ctx.cancel.clone(),
        slot.checkpoint.clone(),
    );
    let handle = tasks.spawn(async move {
        let close = async {
            let projection = &sup.projections[i];
            let source = Shadowed::new(
                AdmittedPortal::new(
                    chain_walk::RpcSource::new(crate::chain_index::rpc::Rpc::at(
                        &sup.http,
                        &projection.url,
                    )),
                    sup.portal.as_deref(),
                ),
                sup.shadow.as_deref(),
            );
            let owner = holdings
                .owner(&sup.pool, lease.as_ref(), index.as_ref(), &sup.settings)
                .await?;
            let mut out = out;
            let closed = chain_walk::close(
                &sup.pool,
                &source,
                index.as_ref(),
                &sup.settings,
                &owner,
                close_options,
                &mut out,
            )
            .await;
            if let Err(e) = &closed
                && e.chain()
                    .any(|c| c.downcast_ref::<chain_walk::Fenced>().is_some())
            {
                holdings.forget(index.indexer(), index.net());
            }
            closed.map(|()| out)
        };
        let close = std::panic::AssertUnwindSafe(close).catch_unwind();
        let attempt = tokio::select! {
            biased;
            _ = lost.cancelled() => Attempt::Interrupted,
            _ = stop.cancelled() => Attempt::Interrupted,
            outcome = close => match outcome {
                Ok(result) => Attempt::Ran(result),
                Err(panic) => Attempt::Panicked(panic_message(panic.as_ref())),
            },
        };
        // The close's outcome stands; only then is the hint read, in this
        // projection's own task.
        let alone = match &attempt {
            Attempt::Ran(Ok(_)) => hint(&sup, index.as_ref(), &lost, &stop).await,
            _ => false,
        };
        Done::Attempted(attempt, checkpoint, alone)
    });
    running.insert(handle.id(), Running::one(i));
    slot.phase = Phase::Running;
    slot.line = "closing after a shared walk".into();
}

/// Whether `index` walks alone next, read in its slot's own task, raced
/// against its lease and the stop (either says no; the walk itself stands).
async fn hint(
    supervisor: &Supervisor,
    index: &dyn ChainIndex,
    lost: &CancellationToken,
    stop: &CancellationToken,
) -> bool {
    tokio::select! {
        biased;
        _ = lost.cancelled() => false,
        _ = stop.cancelled() => false,
        alone = chain_group::walks_alone(&supervisor.pool, index, supervisor.member_deadline) => alone,
    }
}

/// A member a shared walk paused at its budget: its walk stands as it is,
/// and its walks-alone hint is read in a task of its own, so a slow read
/// holds back no one else. The read ends at once when `stop` fires.
#[allow(
    clippy::too_many_arguments,
    reason = "the supervisor's state, the slot and what its walk left"
)]
fn dispatch_hint(
    stop: &CancellationToken,
    supervisor: &Arc<Supervisor>,
    i: usize,
    slot: &mut Slot,
    out: PassOutcome,
    checkpoint: Option<Checkpoint>,
    tasks: &mut JoinSet<Done>,
    running: &mut HashMap<tokio::task::Id, Running>,
) {
    let acquisition = slot.acquisition.as_ref().expect("a member holds its lease");
    let (sup, index, lost, stop) = (
        supervisor.clone(),
        acquisition.index.clone(),
        acquisition.lost.clone(),
        stop.clone(),
    );
    let handle = tasks.spawn(async move {
        let alone = hint(&sup, index.as_ref(), &lost, &stop).await;
        Done::Attempted(Attempt::Ran(Ok(out)), checkpoint, alone)
    });
    running.insert(handle.id(), Running::one(i));
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
    slot.stood_by = false;
    slot.checkpoint = None;
    slot.invitable = false;
    slot.solo_once = false;
    slot.carry = None;
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

/// Records a task that ended and sets its slots' next step. While
/// `stopping`, a lease just taken is let go at once and no close starts. It
/// runs none of the projection's code: a projection whose state was cut
/// short gets a fresh index.
#[allow(
    clippy::too_many_arguments,
    reason = "the supervisor's state, the task and the mode"
)]
async fn settle(
    ctx: &JobContext,
    supervisor: &Arc<Supervisor>,
    cfgs: &[PeriodicCfg],
    slots: &mut [Slot],
    tasks: &mut JoinSet<Done>,
    running: &mut HashMap<tokio::task::Id, Running>,
    done: Result<(tokio::task::Id, Done), tokio::task::JoinError>,
    leases: &Option<Arc<dyn TakeLease>>,
    stopping: bool,
) {
    let (Running { members, sent }, done) = match done {
        Ok((id, done)) => match running.remove(&id) {
            Some(task) => (task, done),
            None => return,
        },
        Err(err) => match running.remove(&err.id()) {
            Some(task) => (task, Done::Crashed(err.to_string())),
            None => return,
        },
    };
    let requests = sent
        .as_ref()
        .map_or(0, |s| s.load(std::sync::atomic::Ordering::SeqCst));
    let now = Instant::now();
    match done {
        Done::Grouped(ends) => {
            summarize(supervisor, &members, &ends, requests);
            for &j in &members {
                slots[j].phase = Phase::Idle;
            }
            for (j, end) in ends {
                settle_member(
                    ctx, supervisor, cfgs, slots, j, end, tasks, running, leases, stopping,
                );
            }
        }
        Done::GroupInterrupted => {
            let stats = &supervisor.stats;
            stats
                .walks
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            stats
                .requests
                .fetch_add(requests, std::sync::atomic::Ordering::SeqCst);
            stats
                .interrupted
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            tracing::warn!(members = ?names(supervisor, &members), requests, "marketplace chain index: a shared walk was cut short");
            for &j in &members {
                let projection = &supervisor.projections[j];
                let slot = &mut slots[j];
                slot.phase = Phase::Idle;
                slot.suspect(projection);
                slot.due = now;
                slot.line = "a shared walk was cut short".into();
                if slot
                    .acquisition
                    .as_ref()
                    .is_some_and(|a| a.lost.is_cancelled())
                {
                    lost_lease(
                        projection,
                        slot,
                        now,
                        standby_wait(leases, &projection.lease),
                    );
                }
            }
        }
        Done::Crashed(msg) if sent.is_some() => {
            let stats = &supervisor.stats;
            stats
                .walks
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            stats
                .requests
                .fetch_add(requests, std::sync::atomic::Ordering::SeqCst);
            stats
                .interrupted
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            tracing::error!(members = ?names(supervisor, &members), requests, panic = %msg, "marketplace chain index: a shared walk panicked");
            for &j in &members {
                slots[j].phase = Phase::Idle;
                settle_member(
                    ctx,
                    supervisor,
                    cfgs,
                    slots,
                    j,
                    MemberEnd::Failed {
                        error: anyhow::anyhow!("a shared walk panicked: {msg}"),
                        tainted: true,
                    },
                    tasks,
                    running,
                    leases,
                    stopping,
                );
            }
        }
        done => {
            let i = members[0];
            settle_one(
                ctx,
                supervisor,
                cfgs,
                &mut slots[i],
                i,
                done,
                leases,
                stopping,
            )
            .await;
        }
    }
}

fn names<'s>(supervisor: &'s Supervisor, members: &[usize]) -> Vec<&'s str> {
    members
        .iter()
        .map(|&j| supervisor.projections[j].lease.as_str())
        .collect()
}

/// One summary of a shared walk: its members, the requests it attempted and
/// how each member's part ended.
fn summarize(
    supervisor: &Supervisor,
    members: &[usize],
    ends: &[(usize, MemberEnd)],
    requests: usize,
) {
    use std::sync::atomic::Ordering::SeqCst;
    let stats = &supervisor.stats;
    stats.walks.fetch_add(1, SeqCst);
    stats.requests.fetch_add(requests, SeqCst);
    let fell_back = ends
        .iter()
        .any(|(_, end)| matches!(end, MemberEnd::Fallback { .. }));
    if fell_back {
        stats.fallbacks.fetch_add(1, SeqCst);
    }
    let outcomes: Vec<String> = ends
        .iter()
        .map(|(j, end)| {
            let what = match end {
                MemberEnd::Walked { close: Some(_), .. } => "reached the target",
                MemberEnd::Walked { close: None, .. } => "paused at its budget",
                MemberEnd::Waiting(_) => "waiting",
                MemberEnd::Ended(_) => "ended in its opening",
                MemberEnd::Failed { .. } => "failed",
                MemberEnd::Regroup { .. } => "regrouping",
                MemberEnd::Fallback { .. } => "falling back alone",
            };
            format!("{}: {what}", supervisor.projections[*j].lease)
        })
        .collect();
    tracing::info!(members = ?names(supervisor, members), requests, outcomes = ?outcomes, "marketplace chain index: a shared walk");
}

/// Records the end of slot `i`'s lease request, attempt or close.
#[allow(
    clippy::too_many_arguments,
    reason = "the supervisor's state, the task and the mode"
)]
async fn settle_one(
    ctx: &JobContext,
    supervisor: &Supervisor,
    cfgs: &[PeriodicCfg],
    slot: &mut Slot,
    i: usize,
    done: Done,
    leases: &Option<Arc<dyn TakeLease>>,
    stopping: bool,
) {
    let projection = &supervisor.projections[i];
    let cfg = &cfgs[i];
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
        slot.stood_by = true;
        return;
    }
    let (outcome, checkpoint, alone) = match done {
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
            slot.stood_by = false;
            return;
        }
        Done::Requested(Ok(None)) => {
            slot.due = now + standby;
            slot.line = "standby: another process holds the lease".into();
            slot.stood_by = true;
            return;
        }
        Done::Requested(Err(err)) => {
            ctx.error(format!("{}: lease: {err}", projection.lease));
            slot.due = now + standby;
            slot.line = "standby: the lease connection is unavailable".into();
            slot.stood_by = true;
            return;
        }
        Done::Attempted(outcome, checkpoint, alone) => (outcome, checkpoint, alone),
        Done::Crashed(msg) => (Attempt::Panicked(msg), None, false),
        Done::Grouped(_) | Done::GroupInterrupted => return,
    };
    after_alone(slot, alone);
    let Some(acquisition) = slot.acquisition.as_mut() else {
        return;
    };
    let number = acquisition.number;
    match outcome {
        Attempt::Ran(Ok(o)) => {
            ctx.pass_done();
            acquisition.failures = 0;
            slot.due = now + cfg.interval;
            slot.invitable = o.folded && !o.paused && o.quarantined.is_none();
            slot.checkpoint = checkpoint;
            slot.line = o.summary();
            tracing::info!(lease = %projection.lease, acquisition = number, summary = %slot.line, "marketplace chain index pass");
        }
        Attempt::Waiting(why) => {
            ctx.pass_done();
            acquisition.failures = 0;
            slot.due = now + cfg.interval;
            slot.invitable = false;
            slot.line = why;
        }
        Attempt::Ran(Err(err)) => {
            ctx.pass_done();
            acquisition.failures += 1;
            let wait = cfg.wait_after_failure(acquisition.failures);
            slot.due = now + wait;
            slot.checkpoint = None;
            slot.invitable = false;
            slot.line = format!("failed; next in {} s", wait.as_secs());
            tracing::warn!(lease = %projection.lease, acquisition = number, error = %format!("{err:#}"), wait_ms = wait.as_millis() as u64, "marketplace chain index pass failed");
            ctx.error(format!("{}: {err:#}", projection.lease));
        }
        Attempt::Panicked(msg) => {
            ctx.pass_done();
            acquisition.failures += 1;
            let wait = cfg.wait_after_failure(acquisition.failures);
            slot.due = now + wait;
            // A fresh index for the retry; the old one's state may be poisoned.
            slot.suspect(projection);
            slot.line = format!("panicked; next in {} s", wait.as_secs());
            tracing::error!(lease = %projection.lease, acquisition = number, panic = %msg, "marketplace chain index pass panicked");
            ctx.error(format!("{}: panic: {msg}", projection.lease));
        }
        Attempt::Interrupted => {
            slot.suspect(projection);
        }
    }
    if slot
        .acquisition
        .as_ref()
        .is_some_and(|a| a.lost.is_cancelled())
    {
        lost_lease(projection, slot, now, standby);
    }
}

/// Records how member `j` of a shared walk ended, starting its close when it
/// reached the target.
#[allow(
    clippy::too_many_arguments,
    reason = "the supervisor's state, the member and the mode"
)]
fn settle_member(
    ctx: &JobContext,
    supervisor: &Arc<Supervisor>,
    cfgs: &[PeriodicCfg],
    slots: &mut [Slot],
    j: usize,
    end: MemberEnd,
    tasks: &mut JoinSet<Done>,
    running: &mut HashMap<tokio::task::Id, Running>,
    leases: &Option<Arc<dyn TakeLease>>,
    stopping: bool,
) {
    let projection = &supervisor.projections[j];
    let cfg = &cfgs[j];
    let now = Instant::now();
    let slot = &mut slots[j];
    slot.phase = Phase::Idle;
    if slot.acquisition.is_none() {
        return;
    }
    match end {
        MemberEnd::Walked {
            out,
            checkpoint,
            close: Some(fold_since),
            ..
        } if !stopping => {
            slot.checkpoint = checkpoint;
            dispatch_close(ctx, supervisor, j, slot, out, fold_since, tasks, running);
            return;
        }
        MemberEnd::Walked {
            out,
            checkpoint,
            close: None,
        } if !stopping => {
            slot.checkpoint = checkpoint.clone();
            dispatch_hint(
                &ctx.cancel,
                supervisor,
                j,
                slot,
                out,
                checkpoint,
                tasks,
                running,
            );
            return;
        }
        MemberEnd::Walked {
            out, checkpoint, ..
        } => {
            ctx.pass_done();
            if let Some(a) = slot.acquisition.as_mut() {
                a.failures = 0;
            }
            slot.due = now + cfg.interval;
            slot.checkpoint = checkpoint;
            slot.invitable = false;
            slot.line = out.summary();
        }
        MemberEnd::Waiting(why) => {
            ctx.pass_done();
            if let Some(a) = slot.acquisition.as_mut() {
                a.failures = 0;
            }
            slot.due = now + cfg.interval;
            slot.invitable = false;
            slot.line = why;
        }
        MemberEnd::Ended(out) => {
            ctx.pass_done();
            if let Some(a) = slot.acquisition.as_mut() {
                a.failures = 0;
            }
            slot.due = now + cfg.interval;
            slot.checkpoint = None;
            slot.invitable = false;
            slot.line = out.summary();
        }
        MemberEnd::Failed { error, tainted } => {
            ctx.pass_done();
            let failures = slot.acquisition.as_mut().map_or(1, |a| {
                a.failures += 1;
                a.failures
            });
            let wait = cfg.wait_after_failure(failures);
            slot.due = now + wait;
            if tainted {
                supervisor
                    .stats
                    .cut_short
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                slot.suspect(projection);
            }
            slot.checkpoint = None;
            slot.invitable = false;
            slot.line = format!("failed in a shared walk; next in {} s", wait.as_secs());
            tracing::warn!(lease = %projection.lease, error = %format!("{error:#}"), tainted, "marketplace chain index: a shared walk's member failed");
            ctx.error(format!("{}: {error:#}", projection.lease));
        }
        MemberEnd::Regroup { checkpoint, solo } => regroup(slot, checkpoint, solo, now),
        MemberEnd::Fallback { budget } => fall_back(slot, budget, now),
    }
    if slot
        .acquisition
        .as_ref()
        .is_some_and(|a| a.lost.is_cancelled())
    {
        lost_lease(
            projection,
            slot,
            now,
            standby_wait(leases, &projection.lease),
        );
    }
}

/// After an attempt alone: the next one walks alone again only if this one
/// ran on a carried budget (the whole one is still owed) or the projection
/// still owes a backfill, whose lanes run alone without being gathered first.
fn after_alone(slot: &mut Slot, alone: bool) {
    if !std::mem::take(&mut slot.carried) {
        slot.solo_once = false;
    }
    slot.solo_once |= alone;
}

/// A member out of step with its walk: it keeps where its cursor really
/// stands (read again), and walks next, alone when `solo`.
fn regroup(slot: &mut Slot, checkpoint: Option<Checkpoint>, solo: bool, now: Instant) {
    slot.checkpoint = checkpoint;
    slot.solo_once |= solo;
    slot.invitable = false;
    slot.due = now;
    slot.line = if solo {
        "walking alone next".into()
    } else {
        "regrouping".into()
    };
}

/// A member whose shared read failed: it walks next, alone, with what it had
/// left; the walk after that is alone too, on a whole budget.
fn fall_back(slot: &mut Slot, budget: Budget, now: Instant) {
    slot.carry = Some(budget);
    slot.solo_once = true;
    slot.invitable = false;
    slot.due = now;
    slot.line = "a shared read failed; walking alone".into();
}

/// One attempt of projection `i` alone: its gate, then a whole job pass on
/// `budget`, then where its cursor stands. It ends at once when `lost` or
/// `stop` fires, wherever it is waiting.
#[allow(
    clippy::too_many_arguments,
    reason = "the acquisition's parts, the budget, the tokens and the job's trace"
)]
async fn attempt(
    supervisor: Arc<Supervisor>,
    i: usize,
    index: Arc<dyn ChainIndex>,
    holdings: Arc<Holdings>,
    lease: Option<crate::jobs::LeaseControl>,
    budget: Budget,
    lost: CancellationToken,
    stop: CancellationToken,
    ctx: JobContext,
) -> Done {
    let walk = async {
        let projection = &supervisor.projections[i];
        if let Some(gate) = &projection.gate {
            match gate().await {
                Ok(Some(why)) => return (Attempt::Waiting(why), None, false),
                Ok(None) => {}
                Err(err) => return (Attempt::Ran(Err(err)), None, false),
            }
        }
        let source = Shadowed::new(
            AdmittedPortal::new(
                chain_walk::RpcSource::new(crate::chain_index::rpc::Rpc::at(
                    &supervisor.http,
                    &projection.url,
                )),
                supervisor.portal.as_deref(),
            ),
            supervisor.shadow.as_deref(),
        );
        let progress = |m: String| ctx.progress(format!("{}: {m}", projection.lease));
        let outcome = chain_walk::job_pass_with(
            &supervisor.pool,
            &source,
            index.as_ref(),
            &supervisor.settings,
            &holdings,
            lease.as_ref(),
            budget,
            // The select below owns the interruption: it drops this pass at
            // once on a stop or a lost lease.
            None,
            &progress,
        )
        .await;
        let (checkpoint, alone) = match &outcome {
            Ok(_) => (
                chain_walk::cursor(&supervisor.pool, index.indexer(), index.net())
                    .await
                    .ok()
                    .flatten()
                    .and_then(|c| Checkpoint::of(&c)),
                chain_group::walks_alone(
                    &supervisor.pool,
                    index.as_ref(),
                    supervisor.member_deadline,
                )
                .await,
            ),
            Err(_) => (None, false),
        };
        (Attempt::Ran(outcome), checkpoint, alone)
    };
    let walk = std::panic::AssertUnwindSafe(walk).catch_unwind();
    let (attempt, checkpoint, alone) = tokio::select! {
        biased;
        _ = lost.cancelled() => (Attempt::Interrupted, None, false),
        _ = stop.cancelled() => (Attempt::Interrupted, None, false),
        outcome = walk => outcome.unwrap_or_else(|panic| (Attempt::Panicked(panic_message(panic.as_ref())), None, false)),
    };
    Done::Attempted(attempt, checkpoint, alone)
}

pub(crate) fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
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
/// switch (`None`: on for each configured Ethereum endpoint).
pub fn projections(
    state: &Arc<MarketplaceState>,
    net: ChainNet,
    rentals: Option<bool>,
) -> Vec<Projection> {
    let mut out = Vec::new();
    visit_projections(state, net, rentals, |job, url, make, gate, cfg| {
        if let Some(url) = url {
            out.push(Projection {
                lease: lease_name(job, net),
                url,
                cfg,
                make,
                gate,
            });
        }
    });
    out
}

/// Includes explicitly enabled projections even when their RPC is missing.
pub fn expected_cursors(
    state: &Arc<MarketplaceState>,
    net: ChainNet,
    rentals: Option<bool>,
) -> Vec<(Arc<dyn ChainIndex>, Option<String>)> {
    let mut out = Vec::new();
    visit_projections(state, net, rentals, |_, url, make, _, _| {
        out.push((make(), url))
    });
    out
}

fn visit_projections(
    state: &Arc<MarketplaceState>,
    net: ChainNet,
    rentals: Option<bool>,
    mut add: impl FnMut(&str, Option<String>, MakeIndex, Option<Gate>, PeriodicCfg),
) {
    let settings = &state.settings.market.indexes;
    let market_cfg = || {
        PeriodicCfg::every(settings.interval)
            .with_jitter(Duration::from_secs(1))
            .with_error_backoff(Duration::from_secs(30))
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
    if settings.committee_enabled && net == ChainNet::Matic {
        let chain_id = settings.chain_id(net);
        add(
            "committee-index",
            state.committee_rpc_url(net),
            Arc::new(move || Arc::new(super::committee_index::CommitteeIndex { chain_id })),
            None,
            market_cfg(),
        );
    }
    if settings.third_party_registry_enabled && net == ChainNet::Matic {
        add(
            "third-party-registry-index",
            state.polygon_mainnet_rpc_url(),
            Arc::new(|| Arc::new(super::third_party_registry::ThirdPartyRegistryIndex)),
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
    if net == ChainNet::Ethereum && rentals != Some(false) {
        use crate::rentals::index::RentalsIndex;
        let rentals_cfg = || {
            PeriodicCfg::every(settings.interval)
                .with_jitter(Duration::from_secs(20))
                .with_error_backoff(Duration::from_secs(1800))
        };
        let mainnet_url = state.venue_rpc_url(1);
        if rentals.unwrap_or(mainnet_url.is_some()) {
            add(
                "rentals-index",
                mainnet_url,
                Arc::new(|| Arc::new(RentalsIndex::mainnet())),
                None,
                rentals_cfg(),
            );
        }
        // A Sepolia endpoint opts the worker into the separate testnet history.
        if let Some(url) = state.sepolia_rentals_rpc_url() {
            add(
                "rentals-index-sepolia",
                Some(url),
                Arc::new(|| Arc::new(RentalsIndex::sepolia())),
                None,
                rentals_cfg(),
            );
        }
    }
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
        let portal = state.settings.market.portal.as_ref();
        let shadow = portal
            .filter(|p| p.mode == PortalMode::Shadow)
            .and_then(|p| spawn_shadow(jobs, p, net));
        let admission = portal.and_then(|p| match p.mode {
            PortalMode::Source { deadline } => admission(p, net, deadline),
            PortalMode::Shadow => None,
        });
        spawn(
            jobs,
            Supervisor {
                name: job_name(net),
                pool: state.pool.clone(),
                http: state.http.clone(),
                settings: state.settings.market.indexes.clone(),
                projections,
                member_deadline: Duration::from_secs(60),
                stats: Arc::default(),
                shadow,
                portal: admission,
            },
        );
    }
}

/// `net`'s portal as a source; `None` when its reader cannot be built
/// (startup already checked it).
fn admission(
    settings: &PortalSettings,
    net: ChainNet,
    deadline: Duration,
) -> Option<Arc<Admission>> {
    let reader = match settings.reader(net) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(network = net.db_name(), error = %e, "chain logs portal off for this network");
            return None;
        }
    };
    tracing::info!(
        network = net.db_name(),
        dataset = reader.dataset(),
        deadline_secs = deadline.as_secs(),
        "chain logs portal: windows its coverage admits are read from the portal"
    );
    Some(Arc::new(Admission::new(reader, net.db_name(), deadline)))
}

/// `net`'s shadow worker, as a job of its own that ends with the process;
/// `None` when its reader cannot be built (startup already checked it).
fn spawn_shadow(jobs: &Jobs, settings: &PortalSettings, net: ChainNet) -> Option<Arc<Shadow>> {
    let reader = match settings.reader(net) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(network = net.db_name(), error = %e, "chain logs shadow off for this network");
            return None;
        }
    };
    tracing::info!(
        network = net.db_name(),
        dataset = reader.dataset(),
        address_history = ?settings.history,
        "chain logs shadow: accepted windows are compared with a portal"
    );
    let (handle, worker) = shadow::channel(reader, net.db_name(), settings.history);
    jobs.spawn(
        format!("chain-logs-shadow-{}", net.db_name()),
        move |ctx| async move {
            worker.run(ctx.cancel.cancelled_owned()).await;
            Ok(())
        },
    );
    Some(handle)
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
            "committee index enabled but no Polygon JSON-RPC endpoint is configured (the TRADE_RPC_URLS entry of MARKETPLACE_INDEX_CHAIN_ID_MATIC, or RPC_ENDPOINT_POLYGON for 137 on mainnet): it stays off"
        );
    }
    if settings.third_party_registry_enabled && state.polygon_mainnet_rpc_url().is_none() {
        tracing::warn!(
            job = "third-party-registry-index",
            "third-party registry index enabled but no Polygon JSON-RPC endpoint is configured (TRADE_RPC_URLS 137=, or RPC_ENDPOINT_POLYGON on mainnet): it stays off"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_portal_settings_are_read_as_the_environment_spells_them() {
        let parse = |source: &str, origin: Option<&str>, deadline: u64| {
            PortalSettings::parse(
                source,
                origin,
                "ethereum-mainnet",
                "polygon-mainnet",
                None,
                1.0,
                false,
                deadline,
            )
        };
        let portal = Some("https://portal.example");
        assert!(
            parse("rpc", portal, 0).unwrap().is_none(),
            "rpc reads nothing"
        );
        assert_eq!(
            parse("portal", portal, 20).unwrap().unwrap().mode,
            PortalMode::Source {
                deadline: Duration::from_secs(20)
            }
        );
        for deadline in [0, 601] {
            assert!(parse("portal", portal, deadline).is_err(), "{deadline}");
        }
        // The deadline is only the portal mode's.
        assert_eq!(
            parse("shadow", portal, 0).unwrap().unwrap().mode,
            PortalMode::Shadow
        );
        assert!(parse("portal", None, 20).is_err(), "portal needs a URL");
        assert!(parse("lake", portal, 20).is_err());
    }

    fn checkpoint(block: i64) -> Checkpoint {
        Checkpoint {
            block,
            hash: format!("0x{block:064x}"),
            full_history: true,
        }
    }

    fn projection(lease: &str, url: &str) -> Projection {
        Projection {
            lease: lease.into(),
            url: url.into(),
            cfg: PeriodicCfg::every(Duration::from_secs(1)),
            make: Arc::new(|| {
                Arc::new(super::super::mana_transfers::ManaTransfersIndex {
                    net: ChainNet::Matic,
                }) as Arc<dyn ChainIndex>
            }),
            gate: None,
        }
    }

    fn slot(projection: &Projection, at: Option<Checkpoint>, due: Instant) -> Slot {
        Slot {
            acquisition: Some(Acquisition::new(projection, None)),
            phase: Phase::Idle,
            due,
            line: String::new(),
            stood_by: false,
            checkpoint: at,
            invitable: false,
            solo_once: false,
            carry: None,
            carried: false,
        }
    }

    fn supervisor(projections: Vec<Projection>) -> Supervisor {
        Supervisor {
            name: "chain-walk-matic".into(),
            pool: sqlx::postgres::PgPoolOptions::new()
                .connect_lazy("postgres://localhost/unused")
                .unwrap(),
            http: crate::http::Outbound::for_tests(),
            settings: IndexSettings::default(),
            projections,
            member_deadline: Duration::from_secs(60),
            stats: Arc::default(),
            shadow: None,
            portal: None,
        }
    }

    /// A member out of step keeps the checkpoint it was read at, not the one
    /// it was gathered at, so the next dispatch groups it by where it stands.
    #[tokio::test]
    async fn a_regroup_keeps_where_the_cursor_really_stands() {
        let p = projection("trade-events-index-matic", "http://rpc");
        let now = Instant::now();
        let mut s = slot(&p, Some(checkpoint(98)), now + Duration::from_secs(5));
        s.invitable = true;
        regroup(&mut s, Some(checkpoint(50)), false, now);
        assert_eq!(s.checkpoint, Some(checkpoint(50)));
        assert!(!s.invitable && !s.solo_once && s.due == now);
        regroup(&mut s, None, true, now);
        assert_eq!(s.checkpoint, None);
        assert!(s.solo_once);
    }

    /// After an attempt alone, a projection walks alone again while it owes a
    /// backfill or still has a whole budget owed after a carried one.
    #[tokio::test]
    async fn an_attempt_alone_decides_whether_the_next_walks_alone() {
        let p = projection("nft-index-matic", "http://rpc");
        let now = Instant::now();
        let mut s = slot(&p, None, now);
        s.solo_once = true;
        after_alone(&mut s, true);
        assert!(s.solo_once, "a backfill still owed");
        after_alone(&mut s, false);
        assert!(!s.solo_once, "the backfill done");
        s.solo_once = true;
        s.carried = true;
        after_alone(&mut s, false);
        assert!(
            s.solo_once && !s.carried,
            "a whole budget still owed after a carried one"
        );
        after_alone(&mut s, false);
        assert!(!s.solo_once);
    }

    /// An index whose walks-alone read answers `self.0`, and which is never
    /// walked.
    struct Answers(bool);

    #[async_trait::async_trait]
    impl ChainIndex for Answers {
        fn indexer(&self) -> &'static str {
            chain_walk::INDEXER_MANA
        }
        fn net(&self) -> ChainNet {
            ChainNet::Matic
        }
        fn start_block(&self) -> u64 {
            0
        }
        async fn groups(
            &self,
            _: &PgPool,
            _: u64,
            _: u64,
            _: (u64, i64),
        ) -> anyhow::Result<Vec<chain_walk::LogGroup>> {
            unreachable!("never walked")
        }
        async fn write(
            &self,
            _: &mut sqlx::PgConnection,
            _: &[crate::chain_index::rpc::RpcLog],
            _: &HashMap<i64, i64>,
        ) -> anyhow::Result<usize> {
            unreachable!("never walked")
        }
        async fn rewind(&self, _: &mut sqlx::PgConnection, _: i64) -> anyhow::Result<()> {
            unreachable!("never walked")
        }
        async fn fold(
            &self,
            _: &PgPool,
            _: Option<i64>,
            _: i64,
            _: &chain_walk::Owner,
        ) -> anyhow::Result<bool> {
            unreachable!("never walked")
        }
        async fn walks_alone(&self, _: &PgPool) -> anyhow::Result<bool> {
            Ok(self.0)
        }
    }

    /// A member a shared walk paused at its budget: the hint read in its
    /// own task carries the walk's outcome and checkpoint back as they were,
    /// with the index's answer, which decides whether it walks alone next.
    #[tokio::test]
    async fn a_paused_members_hint_decides_whether_it_walks_alone_next() {
        for answer in [true, false] {
            let mut p = projection("mana-transfers-index-matic", "http://rpc");
            p.make = Arc::new(move || Arc::new(Answers(answer)) as Arc<dyn ChainIndex>);
            let sup = Arc::new(supervisor(vec![p]));
            let mut s = slot(&sup.projections[0], None, Instant::now());
            let (mut tasks, mut running) = (JoinSet::new(), HashMap::new());
            let out = PassOutcome {
                paused: true,
                ..PassOutcome::default()
            };
            dispatch_hint(
                &CancellationToken::new(),
                &sup,
                0,
                &mut s,
                out.clone(),
                Some(checkpoint(70)),
                &mut tasks,
                &mut running,
            );
            assert!(s.phase == Phase::Running && running.len() == 1);
            let Done::Attempted(Attempt::Ran(Ok(o)), at, alone) =
                tasks.join_next().await.unwrap().unwrap()
            else {
                panic!("the hint ends as the paused walk's attempt");
            };
            assert_eq!((o, at, alone), (out, Some(checkpoint(70)), answer));
            after_alone(&mut s, alone);
            assert_eq!(s.solo_once, answer);
        }
    }

    /// Two projections paused by their budget at one checkpoint: the second
    /// is left out before its turn, and joins once it is due as well.
    #[tokio::test]
    async fn a_paused_projection_joins_a_shared_walk_once_due() {
        let sup = supervisor(vec![
            projection("mana-transfers-index-matic", "http://rpc"),
            projection("trade-events-index-matic", "http://rpc"),
        ]);
        let now = Instant::now();
        let mut slots = vec![
            slot(&sup.projections[0], Some(checkpoint(98)), now),
            slot(
                &sup.projections[1],
                Some(checkpoint(98)),
                now + Duration::from_secs(1),
            ),
        ];
        assert!(
            companions(&sup, &slots, 0, now).is_empty(),
            "not due yet, nor invitable"
        );
        slots[1].due = now;
        assert_eq!(companions(&sup, &slots, 0, now), vec![1]);
        // One in backoff, at another endpoint, or walking alone next stays out.
        slots[1].acquisition.as_mut().unwrap().failures = 1;
        assert!(companions(&sup, &slots, 0, now).is_empty());
        slots[1].acquisition.as_mut().unwrap().failures = 0;
        slots[1].solo_once = true;
        assert!(companions(&sup, &slots, 0, now).is_empty());
    }
}
