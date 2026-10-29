//! Bounded worker lifecycle: named background tasks spawned with a
//! [`CancellationToken`], each reporting its progress and last error, all
//! cancelled at shutdown and awaited in reverse start order under one
//! deadline. The sync orchestrator is the first job; the mirrors are
//! [`Jobs::spawn_periodic`] loops on top of it.
//!
//! Several `catalyst serve` processes may share one database. A job is either
//! local (it keeps this process's own memory current: a projection reload, a
//! cache sweep, a buffer flush) and runs in every process, or fleet-wide (it
//! changes the shared database, the storage tree or an upstream) and must run
//! in one process at a time. Fleet-wide jobs go through [`Jobs::spawn_leased`]
//! and [`Jobs::spawn_periodic_leased`]: the job runs while this process holds
//! a Postgres session advisory lock named after it ([`Leases`]), stands by
//! polling for the lock while another process holds it, and is cancelled and
//! sent back to standby when the lease connection is lost. A process built
//! with [`Jobs::with_fleet_jobs`] false stands every fleet-wide job down (an
//! API replica); one built with [`Jobs::new`] and no leases (tests, a node
//! that never runs a second process) runs them unconditionally.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use sqlx::PgPool;
use sqlx::postgres::PgConnection;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    Running,
    /// A fleet-wide job waiting for its lease: another process holds it.
    Standby,
    /// A fleet-wide job in a process that runs no fleet jobs.
    StoodDown,
    Finished,
    Failed,
}

#[derive(Debug, Clone)]
pub struct JobReport {
    pub name: String,
    pub state: JobState,
    pub started_at: Instant,
    pub progress: Option<String>,
    pub last_error: Option<String>,
    /// Passes completed, whatever their outcome: a periodic job counts one
    /// per run of its task, a hand-rolled loop through [`JobContext::pass_done`].
    pub passes: u64,
}

#[derive(Default)]
struct Trace {
    progress: Option<String>,
    last_error: Option<String>,
    state: Option<JobState>,
    passes: u64,
}

/// Handed to the job body: its cancellation token and the trace it reports into.
#[derive(Clone)]
pub struct JobContext {
    pub cancel: CancellationToken,
    trace: Arc<Mutex<Trace>>,
    name: Arc<str>,
}

impl JobContext {
    /// One line of where the job is; replaces the previous one.
    pub fn progress(&self, msg: impl Into<String>) {
        self.trace.lock().progress = Some(msg.into());
    }

    /// The most recent error the job survived (a stream reconnect, a failed cycle).
    pub fn error(&self, msg: impl Into<String>) {
        self.trace.lock().last_error = Some(msg.into());
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    /// One pass of the job's loop is over, whatever its outcome. The first
    /// logs `first pass done`, the marker a test waits on before it judges
    /// what the job did.
    pub fn pass_done(&self) {
        let passes = {
            let mut t = self.trace.lock();
            t.passes += 1;
            t.passes
        };
        if passes == 1 {
            tracing::info!(job = %self.name, "first pass done");
        }
    }

    fn set_state(&self, state: Option<JobState>) {
        self.trace.lock().state = state;
    }
}

struct Job {
    name: String,
    started_at: Instant,
    cancel: CancellationToken,
    trace: Arc<Mutex<Trace>>,
    handle: Option<JoinHandle<()>>,
}

/// The registry. One per process; `shutdown` is the only way a job ends
/// early, so every job body must watch its token.
pub struct Jobs {
    root: CancellationToken,
    jobs: Mutex<Vec<Job>>,
    leases: Option<Arc<Leases>>,
    fleet_jobs: bool,
    first_pass_now: bool,
}

impl Default for Jobs {
    fn default() -> Self {
        Self::new()
    }
}

impl Jobs {
    /// No leases: fleet-wide jobs run unconditionally, as in a single process.
    pub fn new() -> Self {
        Self {
            root: CancellationToken::new(),
            jobs: Mutex::new(Vec::new()),
            leases: None,
            fleet_jobs: true,
            first_pass_now: false,
        }
    }

    /// Fleet-wide jobs run only while this process holds their lease in
    /// `pool`'s database. Spawns the lease keeper as the job `job-leases`,
    /// so it must be called on the runtime.
    pub fn with_leases(pool: PgPool, settings: LeaseSettings) -> Self {
        let leases = Arc::new(Leases::new(pool, settings));
        let jobs = Self {
            root: CancellationToken::new(),
            jobs: Mutex::new(Vec::new()),
            leases: Some(leases.clone()),
            fleet_jobs: true,
            first_pass_now: false,
        };
        let keepalive = leases.settings.keepalive;
        jobs.spawn("job-leases", move |ctx| async move {
            loop {
                if !sleep_or_cancel(&ctx.cancel, keepalive).await {
                    return Ok(());
                }
                leases.keepalive_once().await;
                ctx.progress(format!("{} lease(s) held", leases.held_count()));
            }
        });
        jobs
    }

    /// `false`: every fleet-wide job stands down in this process (an API
    /// replica). Per-process jobs are unaffected.
    pub fn with_fleet_jobs(mut self, enabled: bool) -> Self {
        self.fleet_jobs = enabled;
        self
    }

    /// `true`: every periodic job runs its first pass as soon as it is
    /// spawned (or its lease is taken), without jitter or a first interval,
    /// so a test sees each job's first pass within seconds of startup.
    pub fn with_first_pass_now(mut self, enabled: bool) -> Self {
        self.first_pass_now = enabled;
        self
    }

    fn periodic_cfg(&self, cfg: PeriodicCfg) -> PeriodicCfg {
        if self.first_pass_now {
            PeriodicCfg {
                run_on_start: true,
                jitter: Duration::ZERO,
                ..cfg
            }
        } else {
            cfg
        }
    }

    pub fn leases(&self) -> Option<&Arc<Leases>> {
        self.leases.as_ref()
    }

    /// Spawns `body` on the runtime as job `name`. An `Err` return or a panic
    /// marks the job failed with the message; `Ok` marks it finished. A
    /// finished or failed entry of the same name is replaced.
    pub fn spawn<F, Fut>(&self, name: impl Into<String>, body: F) -> CancellationToken
    where
        F: FnOnce(JobContext) -> Fut,
        Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        self.spawn_with_token(name, self.root.child_token(), body)
    }

    /// Like [`Jobs::spawn`], under `parent`: the job ends when the parent
    /// job's token is cancelled (a leased job's helpers end with its lease).
    pub fn spawn_scoped<F, Fut>(
        &self,
        parent: &JobContext,
        name: impl Into<String>,
        body: F,
    ) -> CancellationToken
    where
        F: FnOnce(JobContext) -> Fut,
        Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        self.spawn_with_token(name, parent.cancel.child_token(), body)
    }

    fn spawn_with_token<F, Fut>(
        &self,
        name: impl Into<String>,
        cancel: CancellationToken,
        body: F,
    ) -> CancellationToken
    where
        F: FnOnce(JobContext) -> Fut,
        Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        let name = name.into();
        let trace = Arc::new(Mutex::new(Trace::default()));
        let ctx = JobContext {
            cancel: cancel.clone(),
            trace: trace.clone(),
            name: Arc::from(name.as_str()),
        };
        let fut = body(ctx);
        let task_trace = trace.clone();
        let task_name = name.clone();
        let handle = tokio::spawn(async move {
            let outcome = std::panic::AssertUnwindSafe(fut);
            use futures::FutureExt;
            let result = outcome.catch_unwind().await;
            let mut t = task_trace.lock();
            match result {
                Ok(Ok(())) => {
                    tracing::info!(job = %task_name, "job finished");
                    t.state = Some(JobState::Finished);
                }
                Ok(Err(e)) => {
                    tracing::error!(job = %task_name, error = %e, "job failed");
                    t.last_error = Some(e.to_string());
                    t.state = Some(JobState::Failed);
                }
                Err(panic) => {
                    let msg = panic_message(panic.as_ref());
                    tracing::error!(job = %task_name, panic = %msg, "job panicked");
                    t.last_error = Some(format!("panic: {msg}"));
                    t.state = Some(JobState::Failed);
                }
            }
        });
        tracing::info!(job = %name, "job started");
        let mut jobs = self.jobs.lock();
        jobs.retain(|job| {
            job.name != name || job.handle.as_ref().is_some_and(|h| !h.is_finished())
        });
        jobs.push(Job {
            name,
            started_at: Instant::now(),
            cancel: cancel.clone(),
            trace,
            handle: Some(handle),
        });
        cancel
    }

    /// A fleet-wide job: `body` runs while this process holds the lease
    /// named `name` and is called again after every standby. It is cancelled
    /// (through its context's token) when the lease is lost, and its helpers
    /// spawned with [`Jobs::spawn_scoped`] end with it. An `Err` releases the
    /// lease and retries after a backoff, so a fleet-wide job never stays
    /// dead while a process could run it; `Ok` finishes the job. Without
    /// leases the body runs once, as [`Jobs::spawn`] would run it.
    pub fn spawn_leased<F, Fut>(&self, name: impl Into<String>, mut body: F) -> CancellationToken
    where
        F: FnMut(JobContext) -> Fut + Send + 'static,
        Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        let name = name.into();
        let lease_name = name.clone();
        let leases = self.leases.clone();
        let fleet_jobs = self.fleet_jobs;
        self.spawn(name, move |ctx| async move {
            if !fleet_jobs {
                ctx.set_state(Some(JobState::StoodDown));
                ctx.progress("stood down: this process runs no fleet jobs");
                ctx.cancel.cancelled().await;
                return Ok(());
            }
            let Some(leases) = leases else {
                return body(ctx).await;
            };
            let mut failures: u32 = 0;
            loop {
                let Some(guard) = wait_for_lease(&leases, &lease_name, &ctx).await else {
                    return Ok(());
                };
                ctx.set_state(None);
                ctx.progress("lease acquired");
                tracing::info!(job = %lease_name, "lease acquired");
                let run = ctx.cancel.child_token();
                let run_ctx = JobContext {
                    cancel: run.clone(),
                    trace: ctx.trace.clone(),
                    name: ctx.name.clone(),
                };
                let fut = body(run_ctx);
                tokio::pin!(fut);
                let lost = guard.lost.clone();
                let outcome = tokio::select! {
                    result = &mut fut => Some(result),
                    _ = lost.cancelled() => None,
                };
                run.cancel();
                match outcome {
                    Some(Ok(())) => {
                        guard.release().await;
                        return Ok(());
                    }
                    Some(Err(err)) if ctx.cancel.is_cancelled() => {
                        guard.release().await;
                        return Err(err);
                    }
                    Some(Err(err)) => {
                        failures += 1;
                        let wait = leases
                            .settings
                            .poll
                            .saturating_mul(1u32 << failures.min(6))
                            .min(Duration::from_secs(600));
                        tracing::error!(job = %lease_name, error = %format!("{err:#}"), wait_ms = wait.as_millis() as u64, "leased job failed; releasing the lease and retrying");
                        ctx.error(format!("{err:#}"));
                        ctx.progress(format!("failed; retrying in {} s", wait.as_secs()));
                        guard.release().await;
                        if !sleep_or_cancel(&ctx.cancel, wait).await {
                            return Ok(());
                        }
                    }
                    None => {
                        tracing::warn!(job = %lease_name, "lease lost; stopping and standing by");
                        ctx.error("lease lost: the lease connection dropped");
                        ctx.progress("lease lost; stopping");
                        let _ = fut.await;
                        drop(guard);
                    }
                }
            }
        })
    }

    /// A periodic pass as one job: `task` runs once per [`PeriodicCfg::interval`]
    /// (sleep-after-work, so a slow upstream is never hit back to back), the
    /// first pass right away or after one interval, optionally jittered so a
    /// restarted fleet does not hit the same upstream in lockstep. A failed
    /// or panicking pass is reported through the job trace and, with
    /// [`PeriodicCfg::with_error_backoff`], doubles the wait up to the cap
    /// until a pass succeeds; the loop itself only ends at shutdown.
    pub fn spawn_periodic<F, Fut>(
        &self,
        name: impl Into<String>,
        cfg: PeriodicCfg,
        task: F,
    ) -> CancellationToken
    where
        F: FnMut(JobContext) -> Fut + Send + 'static,
        Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        let name = name.into();
        let log_name = name.clone();
        let cfg = self.periodic_cfg(cfg);
        let task = Arc::new(tokio::sync::Mutex::new(task));
        self.spawn(name, move |ctx| run_periodic(ctx, log_name, cfg, task))
    }

    /// [`Jobs::spawn_periodic`] under `parent` (see [`Jobs::spawn_scoped`]).
    pub fn spawn_periodic_scoped<F, Fut>(
        &self,
        parent: &JobContext,
        name: impl Into<String>,
        cfg: PeriodicCfg,
        task: F,
    ) -> CancellationToken
    where
        F: FnMut(JobContext) -> Fut + Send + 'static,
        Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        let name = name.into();
        let log_name = name.clone();
        let cfg = self.periodic_cfg(cfg);
        let task = Arc::new(tokio::sync::Mutex::new(task));
        self.spawn_with_token(name, parent.cancel.child_token(), move |ctx| {
            run_periodic(ctx, log_name, cfg, task)
        })
    }

    /// A fleet-wide periodic pass: [`Jobs::spawn_periodic`] running only while
    /// this process holds the lease named `name` (see [`Jobs::spawn_leased`]).
    /// The first-pass timing of `cfg` applies each time the lease is taken.
    pub fn spawn_periodic_leased<F, Fut>(
        &self,
        name: impl Into<String>,
        cfg: PeriodicCfg,
        task: F,
    ) -> CancellationToken
    where
        F: FnMut(JobContext) -> Fut + Send + 'static,
        Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        let name = name.into();
        let log_name = name.clone();
        let cfg = self.periodic_cfg(cfg);
        let task = Arc::new(tokio::sync::Mutex::new(task));
        self.spawn_leased(name, move |ctx| {
            run_periodic(ctx, log_name.clone(), cfg.clone(), task.clone())
        })
    }

    pub fn report(&self) -> Vec<JobReport> {
        self.jobs
            .lock()
            .iter()
            .map(|job| {
                let t = job.trace.lock();
                JobReport {
                    name: job.name.clone(),
                    state: t.state.unwrap_or(JobState::Running),
                    started_at: job.started_at,
                    progress: t.progress.clone(),
                    last_error: t.last_error.clone(),
                    passes: t.passes,
                }
            })
            .collect()
    }

    /// A token cancelled when `shutdown` starts, for work that is not a job of its own.
    pub fn root_token(&self) -> CancellationToken {
        self.root.clone()
    }

    /// Cancels every job, then waits for them in reverse start order: the
    /// remainder of `timeout` is shared, and a job still running when it runs
    /// out is aborted. Returns the names that had to be aborted. The lease
    /// connection closes with the process and Postgres releases its locks.
    pub async fn shutdown(&self, timeout: Duration) -> Vec<String> {
        self.root.cancel();
        let deadline = tokio::time::Instant::now() + timeout;
        let mut jobs: Vec<Job> = std::mem::take(&mut *self.jobs.lock());
        let mut aborted = Vec::new();
        while let Some(mut job) = jobs.pop() {
            job.cancel.cancel();
            let Some(handle) = job.handle.take() else {
                continue;
            };
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            let abort = handle.abort_handle();
            match tokio::time::timeout(remaining, handle).await {
                Ok(_) => tracing::info!(job = %job.name, "job stopped"),
                Err(_) => {
                    tracing::warn!(job = %job.name, "job did not stop in time; aborting");
                    abort.abort();
                    aborted.push(job.name.clone());
                }
            }
        }
        if let Some(leases) = &self.leases {
            leases.close().await;
        }
        aborted
    }
}

async fn run_periodic<F, Fut>(
    ctx: JobContext,
    log_name: String,
    cfg: PeriodicCfg,
    task: Arc<tokio::sync::Mutex<F>>,
) -> anyhow::Result<()>
where
    F: FnMut(JobContext) -> Fut + Send + 'static,
    Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
{
    let first_delay = if cfg.run_on_start {
        first_fire_jitter(&log_name, cfg.jitter)
    } else {
        cfg.interval + first_fire_jitter(&log_name, cfg.jitter)
    };
    tracing::info!(
        job = %log_name,
        first_pass_in_ms = first_delay.as_millis() as u64,
        "periodic job armed"
    );
    if !sleep_or_cancel(&ctx.cancel, first_delay).await {
        return Ok(());
    }
    tracing::info!(
        job = %log_name,
        interval_ms = cfg.interval.as_millis() as u64,
        "periodic job started"
    );
    let mut pass: u64 = 0;
    let mut failures: u32 = 0;
    loop {
        pass += 1;
        let started = tokio::time::Instant::now();
        use futures::FutureExt;
        let fut = (task.lock().await)(ctx.clone());
        let outcome = std::panic::AssertUnwindSafe(fut).catch_unwind().await;
        let elapsed_ms = started.elapsed().as_millis() as u64;
        let wait = match outcome {
            Ok(Ok(())) => {
                failures = 0;
                tracing::debug!(job = %log_name, pass, elapsed_ms, "pass ok");
                ctx.progress(format!("pass {pass} ok in {elapsed_ms} ms"));
                cfg.interval
            }
            Ok(Err(err)) => {
                failures += 1;
                let wait = cfg.wait_after_failure(failures);
                tracing::warn!(job = %log_name, pass, elapsed_ms, err = %format!("{err:#}"), wait_ms = wait.as_millis() as u64, "pass failed");
                ctx.error(format!("pass {pass}: {err:#}"));
                ctx.progress(format!("pass {pass} failed; next in {} s", wait.as_secs()));
                wait
            }
            Err(panic) => {
                failures += 1;
                let wait = cfg.wait_after_failure(failures);
                let msg = panic_message(panic.as_ref());
                tracing::error!(job = %log_name, pass, elapsed_ms, panic = %msg, "pass panicked");
                ctx.error(format!("pass {pass}: panic: {msg}"));
                ctx.progress(format!(
                    "pass {pass} panicked; next in {} s",
                    wait.as_secs()
                ));
                wait
            }
        };
        ctx.pass_done();
        if !sleep_or_cancel(&ctx.cancel, wait).await {
            return Ok(());
        }
    }
}

/// Polls for the lease until it is taken (`Some`) or the job is cancelled (`None`).
async fn wait_for_lease(leases: &Arc<Leases>, name: &str, ctx: &JobContext) -> Option<LeaseGuard> {
    let mut announced = false;
    loop {
        match leases.try_acquire(name).await {
            Ok(Some(guard)) => return Some(guard),
            Ok(None) => {
                if !announced {
                    tracing::info!(job = %name, "standby: another process holds the lease");
                    announced = true;
                }
                ctx.set_state(Some(JobState::Standby));
                ctx.progress("standby: another process holds the lease");
            }
            Err(err) => {
                tracing::warn!(job = %name, error = %err, "standby: the lease connection is unavailable");
                ctx.set_state(Some(JobState::Standby));
                ctx.error(format!("lease: {err}"));
                ctx.progress("standby: the lease connection is unavailable");
            }
        }
        let wait = leases.settings.poll + first_fire_jitter(name, leases.settings.poll / 4);
        if !sleep_or_cancel(&ctx.cancel, wait).await {
            return None;
        }
    }
}

/// Timing of the lease connection ([`Jobs::with_leases`]).
#[derive(Clone, Debug)]
pub struct LeaseSettings {
    /// Prefix of every lock name: two fleets on one cluster must not share it.
    pub namespace: String,
    /// How often a standby job asks for its lease again (plus up to a quarter of jitter).
    pub poll: Duration,
    /// How often the lease connection is checked. A check that fails or takes
    /// longer than this loses every lease this process holds.
    pub keepalive: Duration,
}

impl Default for LeaseSettings {
    fn default() -> Self {
        Self {
            namespace: "catalyst".to_string(),
            poll: Duration::from_secs(15),
            keepalive: Duration::from_secs(5),
        }
    }
}

/// This process's fleet-wide leases: one dedicated Postgres session holding
/// a `pg_try_advisory_lock(hashtext(namespace), hashtext(name))` per job it
/// runs. The session is opened on first use, detached from the pool (one
/// connection beside the pool's), and its TCP keepalives are short, so a
/// backend whose client died releases the locks within seconds. Losing the
/// session (a failed or slow keepalive) cancels every holder's `lost` token
/// and forgets the leases; the next acquire reconnects.
pub struct Leases {
    pool: PgPool,
    settings: LeaseSettings,
    conn: tokio::sync::Mutex<Option<PgConnection>>,
    held: Mutex<HashMap<String, CancellationToken>>,
    pending_release: Mutex<Vec<String>>,
}

/// A held lease: dropped without [`LeaseGuard::release`] (a panicking job),
/// the keeper unlocks it on its next pass.
pub struct LeaseGuard {
    leases: Arc<Leases>,
    name: String,
    lost: CancellationToken,
    released: bool,
}

impl LeaseGuard {
    /// Cancelled when the lease connection was lost while this lease was held.
    pub fn lost(&self) -> &CancellationToken {
        &self.lost
    }

    pub async fn release(mut self) {
        self.released = true;
        self.leases.release(&self.name).await;
    }
}

impl Drop for LeaseGuard {
    fn drop(&mut self) {
        if !self.released && !self.lost.is_cancelled() {
            self.leases.pending_release.lock().push(self.name.clone());
        }
    }
}

impl Leases {
    pub fn new(pool: PgPool, settings: LeaseSettings) -> Self {
        Self {
            pool,
            settings,
            conn: tokio::sync::Mutex::new(None),
            held: Mutex::new(HashMap::new()),
            pending_release: Mutex::new(Vec::new()),
        }
    }

    pub fn settings(&self) -> &LeaseSettings {
        &self.settings
    }

    pub fn held_count(&self) -> usize {
        self.held.lock().len()
    }

    /// The names this process holds, sorted.
    pub fn held(&self) -> Vec<String> {
        let mut names: Vec<String> = self.held.lock().keys().cloned().collect();
        names.sort();
        names
    }

    async fn connect(&self) -> sqlx::Result<PgConnection> {
        let mut conn = self.pool.acquire().await?.detach();
        for statement in [
            "SET tcp_keepalives_idle = 5",
            "SET tcp_keepalives_interval = 2",
            "SET tcp_keepalives_count = 3",
        ] {
            sqlx::query(statement).execute(&mut conn).await?;
        }
        Ok(conn)
    }

    /// Takes the lease `name` if no session holds it: `Ok(None)` while
    /// another process (or another job of this one) holds it.
    pub async fn try_acquire(self: &Arc<Self>, name: &str) -> sqlx::Result<Option<LeaseGuard>> {
        if self.held.lock().contains_key(name) {
            return Ok(None);
        }
        let mut slot = self.conn.lock().await;
        if slot.is_none() {
            *slot = Some(self.connect().await?);
        }
        let conn = slot.as_mut().expect("connected above");
        let taken: Result<bool, sqlx::Error> =
            sqlx::query_scalar("SELECT pg_try_advisory_lock(hashtext($1), hashtext($2))")
                .bind(&self.settings.namespace)
                .bind(name)
                .fetch_one(&mut *conn)
                .await;
        match taken {
            Ok(true) => {
                let lost = CancellationToken::new();
                self.held.lock().insert(name.to_string(), lost.clone());
                Ok(Some(LeaseGuard {
                    leases: self.clone(),
                    name: name.to_string(),
                    lost,
                    released: false,
                }))
            }
            Ok(false) => Ok(None),
            Err(err) => {
                *slot = None;
                self.lose_all("the lease connection failed while acquiring");
                Err(err)
            }
        }
    }

    async fn release(&self, name: &str) {
        let mut slot = self.conn.lock().await;
        self.held.lock().remove(name);
        if let Some(conn) = slot.as_mut() {
            let unlocked = sqlx::query("SELECT pg_advisory_unlock(hashtext($1), hashtext($2))")
                .bind(&self.settings.namespace)
                .bind(name)
                .execute(&mut *conn)
                .await;
            if let Err(err) = unlocked {
                tracing::warn!(lease = %name, error = %err, "releasing the lease failed; dropping the lease connection");
                *slot = None;
                self.lose_all("the lease connection failed while releasing");
            }
        }
    }

    fn lose_all(&self, why: &str) {
        let held: Vec<(String, CancellationToken)> = self.held.lock().drain().collect();
        if !held.is_empty() {
            tracing::warn!(leases = ?held.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), "{why}: every lease of this process is lost");
        }
        for (_, lost) in held {
            lost.cancel();
        }
        self.pending_release.lock().clear();
    }

    /// One keeper pass: unlocks leases whose guards were dropped, then proves
    /// the session alive with a bounded `SELECT 1`.
    pub async fn keepalive_once(&self) {
        let mut slot = self.conn.lock().await;
        let Some(conn) = slot.as_mut() else {
            if self.held_count() > 0 {
                self.lose_all("no lease connection");
            }
            return;
        };
        let pending: Vec<String> = std::mem::take(&mut *self.pending_release.lock());
        for name in pending {
            self.held.lock().remove(&name);
            let unlocked = sqlx::query("SELECT pg_advisory_unlock(hashtext($1), hashtext($2))")
                .bind(&self.settings.namespace)
                .bind(&name)
                .execute(&mut *conn)
                .await;
            if let Err(err) = unlocked {
                tracing::warn!(lease = %name, error = %err, "releasing a dropped lease failed");
                *slot = None;
                self.lose_all("the lease connection failed while releasing");
                return;
            }
        }
        let alive = tokio::time::timeout(
            self.settings.keepalive,
            sqlx::query("SELECT 1").execute(&mut *conn),
        )
        .await;
        match alive {
            Ok(Ok(_)) => {}
            Ok(Err(err)) => {
                tracing::warn!(error = %err, "lease keepalive failed");
                *slot = None;
                self.lose_all("the lease connection failed its keepalive");
            }
            Err(_) => {
                tracing::warn!(
                    timeout_ms = self.settings.keepalive.as_millis() as u64,
                    "lease keepalive timed out"
                );
                *slot = None;
                self.lose_all("the lease connection timed out");
            }
        }
    }

    /// The Postgres backend pid of the lease session, if open (tests end it
    /// with `pg_terminate_backend`).
    pub async fn backend_pid(&self) -> Option<i32> {
        let mut slot = self.conn.lock().await;
        let conn = slot.as_mut()?;
        sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *conn)
            .await
            .ok()
    }

    async fn close(&self) {
        let conn = self.conn.lock().await.take();
        if let Some(conn) = conn {
            use sqlx::Connection;
            let _ = conn.close().await;
        }
        self.held.lock().clear();
    }
}

/// Timing of one [`Jobs::spawn_periodic`] loop.
#[derive(Clone, Debug)]
pub struct PeriodicCfg {
    /// Pause between the end of one pass and the start of the next.
    pub interval: Duration,
    /// Upper bound on a random delay added before the first pass.
    pub jitter: Duration,
    /// First pass immediately (after any jitter) instead of after one interval.
    pub run_on_start: bool,
    /// After a failed pass wait `interval`, then double per consecutive
    /// failure up to this cap; `None` keeps the plain interval.
    pub error_backoff_max: Option<Duration>,
}

impl PeriodicCfg {
    pub fn every(interval: Duration) -> Self {
        Self {
            interval,
            jitter: Duration::ZERO,
            run_on_start: true,
            error_backoff_max: None,
        }
    }

    pub fn with_jitter(mut self, jitter: Duration) -> Self {
        self.jitter = jitter;
        self
    }

    /// Hold the first pass back by one interval: for loops whose work is
    /// pointless until something else has happened.
    pub fn after_first_interval(mut self) -> Self {
        self.run_on_start = false;
        self
    }

    pub fn with_error_backoff(mut self, max: Duration) -> Self {
        self.error_backoff_max = Some(max);
        self
    }

    fn wait_after_failure(&self, consecutive_failures: u32) -> Duration {
        match self.error_backoff_max {
            None => self.interval,
            Some(max) => {
                let factor = 1u32 << consecutive_failures.saturating_sub(1).min(16);
                self.interval
                    .saturating_mul(factor)
                    .min(max)
                    .max(self.interval.min(max))
            }
        }
    }
}

/// False when the token fired first.
async fn sleep_or_cancel(cancel: &CancellationToken, wait: Duration) -> bool {
    if cancel.is_cancelled() {
        return false;
    }
    if wait.is_zero() {
        return true;
    }
    tokio::select! {
        _ = cancel.cancelled() => false,
        _ = tokio::time::sleep(wait) => true,
    }
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> &str {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        s
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.as_str()
    } else {
        "non-string panic payload"
    }
}

fn first_fire_jitter(name: &str, bound: Duration) -> Duration {
    let bound_nanos = bound.as_nanos().min(u64::MAX as u128) as u64;
    if bound_nanos == 0 {
        return Duration::ZERO;
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in name.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    let entropy = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    h ^= entropy.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    h = h.wrapping_mul(0x0000_0100_0000_01b3);
    h ^= h >> 33;
    Duration::from_nanos(h % bound_nanos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

    #[tokio::test(start_paused = true)]
    async fn jobs_report_progress_and_stop_under_one_deadline() {
        let jobs = Jobs::new();
        let order = Arc::new(Mutex::new(Vec::new()));

        for name in ["first", "second"] {
            let order = order.clone();
            jobs.spawn(name, move |ctx| async move {
                ctx.progress(format!("{name} running"));
                ctx.cancel.cancelled().await;
                order.lock().push(name);
                Ok(())
            });
        }
        let stuck_finished = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stuck_runs = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        jobs.spawn("stuck", {
            let (finished, runs) = (stuck_finished.clone(), stuck_runs.clone());
            move |_ctx| async move {
                loop {
                    runs.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    finished.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            }
        });
        jobs.spawn("broken", |_ctx| async move { anyhow::bail!("boom") });
        tokio::task::yield_now().await;

        let report = jobs.report();
        assert_eq!(report[0].progress.as_deref(), Some("first running"));
        assert_eq!(report[0].state, JobState::Running);
        let broken = report.iter().find(|r| r.name == "broken").unwrap();
        assert_eq!(broken.state, JobState::Failed);
        assert_eq!(broken.last_error.as_deref(), Some("boom"));

        let aborted = jobs.shutdown(Duration::from_millis(200)).await;
        assert_eq!(aborted, vec!["stuck".to_string()]);
        // Every job is cancelled at once; only the waits run in reverse start order.
        let mut stopped = order.lock().clone();
        stopped.sort();
        assert_eq!(stopped, vec!["first", "second"]);

        // Aborted means stopped, not detached: the straggler makes no further progress
        // over five of its virtual loop periods.
        let runs_at_shutdown = stuck_runs.load(std::sync::atomic::Ordering::SeqCst);
        stuck_finished.store(false, std::sync::atomic::Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(
            stuck_runs.load(std::sync::atomic::Ordering::SeqCst),
            runs_at_shutdown
        );
        assert!(!stuck_finished.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn a_panicking_job_is_reported_not_propagated() {
        let jobs = Jobs::new();
        jobs.spawn("panics", |_ctx| async move {
            panic!("kaboom");
        });
        tokio::task::yield_now().await;
        let report = jobs.report();
        assert_eq!(report[0].state, JobState::Failed);
        assert_eq!(report[0].last_error.as_deref(), Some("panic: kaboom"));
        assert!(jobs.shutdown(Duration::from_secs(1)).await.is_empty());
    }

    type Stamps = UnboundedReceiver<tokio::time::Instant>;

    fn stamping(jobs: &Jobs, cfg: PeriodicCfg, work: Duration) -> Stamps {
        let (tx, rx) = unbounded_channel();
        jobs.spawn_periodic("stamp", cfg, move |_ctx| {
            let tx = tx.clone();
            async move {
                let _ = tx.send(tokio::time::Instant::now());
                tokio::time::sleep(work).await;
                Ok(())
            }
        });
        rx
    }

    async fn gaps(rx: &mut Stamps, n: usize) -> Vec<Duration> {
        let mut stamps = Vec::new();
        for _ in 0..n {
            stamps.push(rx.recv().await.expect("the loop must keep firing"));
        }
        stamps.windows(2).map(|w| w[1] - w[0]).collect()
    }

    #[tokio::test(start_paused = true)]
    async fn periodic_sleeps_the_interval_after_each_pass() {
        let jobs = Jobs::new();
        let start = tokio::time::Instant::now();
        let mut rx = stamping(
            &jobs,
            PeriodicCfg::every(Duration::from_secs(10)),
            Duration::from_secs(3),
        );
        assert_eq!(
            rx.recv().await.unwrap() - start,
            Duration::ZERO,
            "first pass is immediate"
        );
        let g = gaps(&mut rx, 3).await;
        assert!(g.iter().all(|g| *g == Duration::from_secs(13)), "{g:?}");
        let report = jobs.report();
        assert_eq!(report[0].state, JobState::Running);
        assert!(
            report[0]
                .progress
                .as_deref()
                .unwrap()
                .contains("ok in 3000 ms"),
            "{report:?}"
        );
        // The pass body does not watch the token; shutdown waits for it to finish.
        assert!(jobs.shutdown(Duration::from_secs(5)).await.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn periodic_after_first_interval_and_jitter_delay_the_first_pass() {
        let jobs = Jobs::new();
        let start = tokio::time::Instant::now();
        let mut rx = stamping(
            &jobs,
            PeriodicCfg::every(Duration::from_secs(60)).after_first_interval(),
            Duration::ZERO,
        );
        assert_eq!(rx.recv().await.unwrap() - start, Duration::from_secs(60));

        let start = tokio::time::Instant::now();
        let mut rx = stamping(
            &jobs,
            PeriodicCfg::every(Duration::from_secs(60)).with_jitter(Duration::from_secs(30)),
            Duration::ZERO,
        );
        let first = rx.recv().await.unwrap() - start;
        assert!(
            first < Duration::from_secs(30),
            "jitter stays under its bound: {first:?}"
        );
        assert!(jobs.shutdown(Duration::from_secs(1)).await.is_empty());
    }

    async fn passes_reach(jobs: &Jobs, name: &str, n: u64) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
        while jobs
            .report()
            .iter()
            .find(|j| j.name == name)
            .map_or(0, |j| j.passes)
            < n
        {
            assert!(
                tokio::time::Instant::now() < deadline,
                "{name} did not reach {n} pass(es) within a virtual second: {:?}",
                jobs.report()
            );
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }

    /// `with_first_pass_now`: neither the first interval nor the jitter holds
    /// back a periodic job (plain, scoped or leased), and every pass, failed
    /// or not, counts in the report.
    #[tokio::test(start_paused = true)]
    async fn first_pass_now_runs_every_periodic_job_at_once_and_counts_its_passes() {
        let jobs = Jobs::new().with_first_pass_now(true);
        let slow = || {
            PeriodicCfg::every(Duration::from_secs(3600))
                .after_first_interval()
                .with_jitter(Duration::from_secs(600))
        };
        let start = tokio::time::Instant::now();
        let mut rx = stamping(&jobs, slow(), Duration::ZERO);
        assert_eq!(rx.recv().await.unwrap() - start, Duration::ZERO);
        jobs.spawn_periodic("fails", slow(), |_ctx| async {
            anyhow::bail!("upstream down")
        });
        jobs.spawn_periodic_leased("leased", slow(), |_ctx| async { Ok(()) });
        let parent_ctx = JobContext {
            cancel: jobs.root_token().child_token(),
            trace: Arc::default(),
            name: Arc::from("parent"),
        };
        jobs.spawn_periodic_scoped(&parent_ctx, "scoped", slow(), |_ctx| async { Ok(()) });
        for name in ["stamp", "fails", "leased", "scoped"] {
            passes_reach(&jobs, name, 1).await;
        }
        let report = jobs.report();
        let fails = report.iter().find(|j| j.name == "fails").unwrap();
        assert_eq!(fails.passes, 1);
        assert_eq!(fails.last_error.as_deref(), Some("pass 1: upstream down"));
        assert!(jobs.shutdown(Duration::from_secs(1)).await.is_empty());

        // Without it the same schedule waits out the first interval.
        let jobs = Jobs::new();
        jobs.spawn_periodic("held", slow(), |_ctx| async { Ok(()) });
        tokio::time::sleep(Duration::from_secs(3599)).await;
        assert_eq!(jobs.report()[0].passes, 0);
        assert!(jobs.shutdown(Duration::from_secs(1)).await.is_empty());
    }

    #[tokio::test]
    async fn a_hand_rolled_loop_counts_its_passes() {
        let jobs = Jobs::new();
        let (tx, mut rx) = unbounded_channel();
        jobs.spawn("loop", move |ctx| async move {
            ctx.pass_done();
            ctx.pass_done();
            let _ = tx.send(());
            ctx.cancel.cancelled().await;
            Ok(())
        });
        rx.recv().await.unwrap();
        assert_eq!(jobs.report()[0].passes, 2);
        assert!(jobs.shutdown(Duration::from_secs(1)).await.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn periodic_failures_back_off_doubling_to_the_cap_and_reset_on_success() {
        let jobs = Jobs::new();
        let (tx, mut rx) = unbounded_channel();
        let passes = Arc::new(AtomicUsize::new(0));
        jobs.spawn_periodic(
            "flaky",
            PeriodicCfg::every(Duration::from_secs(10)).with_error_backoff(Duration::from_secs(35)),
            move |_ctx| {
                let tx = tx.clone();
                let passes = passes.clone();
                async move {
                    let n = passes.fetch_add(1, Ordering::SeqCst);
                    let _ = tx.send(tokio::time::Instant::now());
                    match n {
                        0..=3 => anyhow::bail!("upstream down"),
                        4 => Ok(()),
                        5 => panic!("pass explodes"),
                        _ => Ok(()),
                    }
                }
            },
        );
        let g = gaps(&mut rx, 8).await;
        let secs: Vec<u64> = g.iter().map(|d| d.as_secs()).collect();
        // 4 failures: 10, 20, 35 (capped); then success resets: 10; a panic: 10; ok: 10
        assert_eq!(secs, vec![10, 20, 35, 35, 10, 10, 10]);
        let report = jobs.report();
        assert_eq!(
            report[0].state,
            JobState::Running,
            "the loop survives failures and panics"
        );
        assert_eq!(
            report[0].last_error.as_deref(),
            Some("pass 6: panic: pass explodes")
        );
        assert!(jobs.shutdown(Duration::from_secs(1)).await.is_empty());
    }

    #[test]
    fn periodic_wait_ladder() {
        let cfg = PeriodicCfg::every(Duration::from_secs(10));
        assert_eq!(cfg.wait_after_failure(3), Duration::from_secs(10));
        let cfg = cfg.with_error_backoff(Duration::from_secs(50));
        assert_eq!(cfg.wait_after_failure(1), Duration::from_secs(10));
        assert_eq!(cfg.wait_after_failure(2), Duration::from_secs(20));
        assert_eq!(cfg.wait_after_failure(3), Duration::from_secs(40));
        assert_eq!(cfg.wait_after_failure(4), Duration::from_secs(50));
        assert_eq!(cfg.wait_after_failure(40), Duration::from_secs(50));
        let tiny_cap =
            PeriodicCfg::every(Duration::from_secs(10)).with_error_backoff(Duration::from_secs(2));
        assert_eq!(tiny_cap.wait_after_failure(1), Duration::from_secs(2));
    }

    #[tokio::test(start_paused = true)]
    async fn periodic_shutdown_interrupts_the_sleep_and_a_cancelled_start_runs_nothing() {
        let jobs = Jobs::new();
        let mut rx = stamping(
            &jobs,
            PeriodicCfg::every(Duration::from_secs(3600)),
            Duration::ZERO,
        );
        rx.recv().await.unwrap();
        assert!(
            jobs.shutdown(Duration::from_millis(100)).await.is_empty(),
            "the sleep must yield"
        );
        assert_eq!(jobs.report().len(), 0);

        let jobs = Jobs::new();
        jobs.root_token().cancel();
        let mut rx = stamping(
            &jobs,
            PeriodicCfg::every(Duration::from_secs(1)).with_jitter(Duration::from_secs(30)),
            Duration::ZERO,
        );
        assert!(jobs.shutdown(Duration::from_millis(100)).await.is_empty());
        assert!(rx.try_recv().is_err(), "no pass may have run");
    }

    #[test]
    fn jitter_is_bounded_and_zero_bound_is_immediate() {
        assert_eq!(first_fire_jitter("x", Duration::ZERO), Duration::ZERO);
        for name in ["a", "places-mirror", "events-mirror"] {
            assert!(first_fire_jitter(name, Duration::from_secs(10)) < Duration::from_secs(10));
        }
    }
}

/// The lease protocol against a real cluster (`CATALYST_TEST_DATABASE_URL`;
/// skipped without it): two registries on one namespace, a stopping holder,
/// a killed lease session, a process that stands its fleet jobs down.
#[cfg(test)]
mod lease_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn settings(namespace: &str) -> LeaseSettings {
        LeaseSettings {
            namespace: format!("test-{namespace}-{}", uuid::Uuid::new_v4().simple()),
            poll: Duration::from_millis(100),
            keepalive: Duration::from_millis(100),
        }
    }

    async fn wait_until(what: &str, mut cond: impl FnMut() -> bool) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while !cond() {
            assert!(
                tokio::time::Instant::now() < deadline,
                "timed out waiting for {what}"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// Clears the job's progress line, so the next one it writes is a new report.
    fn clear_progress(jobs: &Jobs, name: &str) {
        for job in jobs.jobs.lock().iter().filter(|j| j.name == name) {
            job.trace.lock().progress = None;
        }
    }

    fn progress_of(jobs: &Jobs, name: &str) -> Option<String> {
        jobs.report()
            .into_iter()
            .find(|j| j.name == name)
            .and_then(|j| j.progress)
    }

    fn state_of(jobs: &Jobs, name: &str) -> Option<JobState> {
        jobs.report()
            .into_iter()
            .find(|j| j.name == name)
            .map(|j| j.state)
    }

    /// A leased body that counts its runs and holds the lease until cancelled.
    fn holder(runs: Arc<AtomicUsize>) -> impl FnMut(JobContext) -> BoxFuture + Send + 'static {
        move |ctx: JobContext| {
            runs.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                ctx.cancel.cancelled().await;
                Ok(())
            })
        }
    }

    type BoxFuture = std::pin::Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>>;

    #[tokio::test]
    async fn one_process_runs_a_fleet_job_and_the_other_stands_by_until_it_stops() {
        let Some(pool) = catalyst_db::scratch::pool(6).await else {
            return;
        };
        let settings = settings("failover");
        let a = Jobs::with_leases(pool.clone(), settings.clone());
        let b = Jobs::with_leases(pool.clone(), settings.clone());
        let (runs_a, runs_b) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
        a.spawn_leased("tick", holder(runs_a.clone()));
        wait_until("a holds the lease", || runs_a.load(Ordering::SeqCst) == 1).await;
        b.spawn_leased("tick", holder(runs_b.clone()));
        wait_until("b reports standby", || {
            state_of(&b, "tick") == Some(JobState::Standby)
        })
        .await;
        for _ in 0..3 {
            clear_progress(&b, "tick");
            wait_until("b polls the lease again and stays on standby", || {
                progress_of(&b, "tick").as_deref()
                    == Some("standby: another process holds the lease")
            })
            .await;
        }
        assert_eq!(
            runs_b.load(Ordering::SeqCst),
            0,
            "b ran while a held the lease"
        );
        assert_eq!(a.leases().unwrap().held(), vec!["tick".to_string()]);
        assert_eq!(b.leases().unwrap().held_count(), 0);

        let aborted = a.shutdown(Duration::from_secs(5)).await;
        assert!(aborted.is_empty(), "{aborted:?}");
        wait_until("b takes over", || runs_b.load(Ordering::SeqCst) == 1).await;
        wait_until("b reports running", || {
            state_of(&b, "tick") == Some(JobState::Running)
        })
        .await;
        assert_eq!(b.leases().unwrap().held(), vec!["tick".to_string()]);
        let aborted = b.shutdown(Duration::from_secs(5)).await;
        assert!(aborted.is_empty(), "{aborted:?}");
    }

    #[tokio::test]
    async fn a_killed_lease_session_cancels_the_holder_which_then_takes_the_lease_again() {
        let Some(pool) = catalyst_db::scratch::pool(6).await else {
            return;
        };
        let settings = settings("killed");
        let jobs = Jobs::with_leases(pool.clone(), settings.clone());
        let runs = Arc::new(AtomicUsize::new(0));
        let cancelled = Arc::new(AtomicUsize::new(0));
        jobs.spawn_leased("tick", {
            let (runs, cancelled) = (runs.clone(), cancelled.clone());
            move |ctx: JobContext| {
                runs.fetch_add(1, Ordering::SeqCst);
                let cancelled = cancelled.clone();
                async move {
                    ctx.cancel.cancelled().await;
                    cancelled.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            }
        });
        wait_until("first run", || runs.load(Ordering::SeqCst) == 1).await;
        let pid = jobs
            .leases()
            .unwrap()
            .backend_pid()
            .await
            .expect("the lease session is connected");
        let killed: bool = sqlx::query_scalar("SELECT pg_terminate_backend($1)")
            .bind(pid)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(killed);
        wait_until("the body is cancelled", || {
            cancelled.load(Ordering::SeqCst) == 1
        })
        .await;
        wait_until("the lease is taken again", || {
            runs.load(Ordering::SeqCst) == 2
        })
        .await;
        let again = jobs.leases().unwrap().backend_pid().await.unwrap();
        assert_ne!(again, pid, "a fresh session carries the lease");
        assert_eq!(jobs.leases().unwrap().held(), vec!["tick".to_string()]);
        let aborted = jobs.shutdown(Duration::from_secs(5)).await;
        assert!(aborted.is_empty(), "{aborted:?}");
        assert_eq!(cancelled.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_process_without_fleet_jobs_stands_leased_jobs_down_and_leaves_the_lock_free() {
        let Some(pool) = catalyst_db::scratch::pool(6).await else {
            return;
        };
        let settings = settings("stood-down");
        let replica = Jobs::with_leases(pool.clone(), settings.clone()).with_fleet_jobs(false);
        let runs = Arc::new(AtomicUsize::new(0));
        replica.spawn_leased("tick", holder(runs.clone()));
        wait_until("stood down", || {
            state_of(&replica, "tick") == Some(JobState::StoodDown)
        })
        .await;
        // A local (non-fleet) job on the same registry still runs.
        let local = Arc::new(AtomicUsize::new(0));
        replica.spawn("local", {
            let local = local.clone();
            move |ctx| async move {
                local.fetch_add(1, Ordering::SeqCst);
                ctx.cancel.cancelled().await;
                Ok(())
            }
        });
        wait_until("local runs", || local.load(Ordering::SeqCst) == 1).await;

        let leader = Jobs::with_leases(pool.clone(), settings.clone());
        let leader_runs = Arc::new(AtomicUsize::new(0));
        leader.spawn_leased("tick", holder(leader_runs.clone()));
        wait_until("the leader takes the lease", || {
            leader_runs.load(Ordering::SeqCst) == 1
        })
        .await;
        assert_eq!(runs.load(Ordering::SeqCst), 0);
        assert_eq!(replica.leases().unwrap().held_count(), 0);
        for jobs in [replica, leader] {
            let aborted = jobs.shutdown(Duration::from_secs(5)).await;
            assert!(aborted.is_empty(), "{aborted:?}");
        }
    }

    #[tokio::test]
    async fn a_failing_leased_body_releases_the_lease_and_retries_after_a_backoff() {
        let Some(pool) = catalyst_db::scratch::pool(6).await else {
            return;
        };
        let settings = settings("retry");
        let jobs = Jobs::with_leases(pool.clone(), settings.clone());
        let runs = Arc::new(AtomicUsize::new(0));
        jobs.spawn_leased("flaky", {
            let runs = runs.clone();
            move |ctx: JobContext| {
                let n = runs.fetch_add(1, Ordering::SeqCst);
                async move {
                    if n == 0 {
                        anyhow::bail!("first pass fails");
                    }
                    ctx.cancel.cancelled().await;
                    Ok(())
                }
            }
        });
        wait_until("second run", || runs.load(Ordering::SeqCst) == 2).await;
        let report = jobs
            .report()
            .into_iter()
            .find(|j| j.name == "flaky")
            .unwrap();
        assert_eq!(report.state, JobState::Running);
        assert!(
            report
                .last_error
                .as_deref()
                .unwrap_or("")
                .contains("first pass fails"),
            "{report:?}"
        );
        let aborted = jobs.shutdown(Duration::from_secs(5)).await;
        assert!(aborted.is_empty(), "{aborted:?}");
    }

    #[tokio::test]
    async fn spawning_a_finished_name_again_replaces_the_finished_entry() {
        let jobs = Jobs::new();
        jobs.spawn("once", |_ctx| async { Ok(()) });
        wait_until("finished", || {
            state_of(&jobs, "once") == Some(JobState::Finished)
        })
        .await;
        jobs.spawn("once", |ctx| async move {
            ctx.cancel.cancelled().await;
            Ok(())
        });
        let names: Vec<String> = jobs.report().into_iter().map(|j| j.name).collect();
        assert_eq!(names, vec!["once".to_string()]);
        assert_eq!(state_of(&jobs, "once"), Some(JobState::Running));
        jobs.shutdown(Duration::from_secs(2)).await;
    }
}
