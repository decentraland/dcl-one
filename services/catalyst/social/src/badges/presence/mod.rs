//! Presence badges (upstream badges #246): a badge configured in
//! `social.presence_badges` is earned by being in a scene (by parcel) or a
//! world inside a UTC window, on a `visit` or for a `stay` of at least
//! `required_seconds`.
//!
//! Input: every verified LiveKit join and leave, which the gatekeeper hands
//! to [`sink::EventSink`]; it stores them in `social.presence_events` before
//! the webhook is acknowledged. The
//! leased `badges-presence` job applies the stored events in order, one
//! transaction per event that also deletes it ([`engine`]); the leased
//! `badges-presence-sweep` job closes visits whose leave was lost
//! ([`sweeper`]) against this process's LiveKit occupancy. Grants go through
//! [`crate::badges::apply_progress`] with `badge_granted` notifications, and
//! the progress document keeps upstream's `intervals`.
//!
//! Runs on every process with `SOCIAL_WRITES_ENABLED`.

pub mod engine;
pub mod intervals;
pub mod sink;
pub mod sweeper;

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;

pub use engine::{EventOutcome, Flow};
pub use sink::{EventSink, PresenceEvent};
pub use sweeper::{Occupancy, SweepSummary};

/// Upstream's bounds (`visit-bounds.ts`, the sweeper and the lookahead).
#[derive(Debug, Clone)]
pub struct Settings {
    /// A visit open this long is checked against the occupancy.
    pub check_after_ms: i64,
    /// A connection open longer is forgotten with no time.
    pub max_age_ms: i64,
    /// How long applied keys, pending leaves and absences are kept.
    pub applied_events_ttl_ms: i64,
    /// How long an unconfirmed visit waits for a late leave.
    pub absent_expire_ms: i64,
    /// A join this long before a window opens is kept.
    pub lookahead_ms: i64,
    pub sweep_batch: i64,
    pub prune_batch: i64,
    /// Events applied per pass.
    pub event_batch: i64,
    pub poll_interval: Duration,
    pub sweep_interval: Duration,
    /// A grant writes its `badge_granted` notification.
    pub grant_notifications: bool,
    /// The sweeper checks open visits against the occupancy.
    pub sweeper_enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        use crate::config::presence_badges as d;
        Self {
            check_after_ms: secs_to_ms(d::OPEN_VISIT_CHECK_AFTER_SECONDS),
            max_age_ms: secs_to_ms(d::OPEN_VISIT_MAX_AGE_SECONDS),
            applied_events_ttl_ms: secs_to_ms(d::APPLIED_EVENTS_TTL_SECONDS),
            absent_expire_ms: secs_to_ms(d::ABSENT_EXPIRE_AFTER_SECONDS),
            lookahead_ms: secs_to_ms(d::EARLY_JOIN_LOOKAHEAD_SECONDS),
            sweep_batch: i64::from(d::SWEEP_BATCH_SIZE),
            prune_batch: i64::from(d::PRUNE_BATCH_SIZE),
            event_batch: 500,
            poll_interval: Duration::from_secs(5),
            sweep_interval: Duration::from_secs(d::SWEEP_INTERVAL_SECONDS),
            grant_notifications: d::GRANT_NOTIFICATIONS,
            sweeper_enabled: d::SWEEPER_ENABLED,
        }
    }
}

/// Seconds as the milliseconds the settings hold.
pub fn secs_to_ms(seconds: u64) -> i64 {
    i64::try_from(seconds.saturating_mul(1000)).unwrap_or(i64::MAX)
}

/// An event that failed this many passes is dropped (logged and counted),
/// so one bad row cannot stop the flow.
const MAX_ATTEMPTS: u32 = 3;
/// Past this age in the queue a transient failure counts too, so an event
/// that always times out cannot hold the queue forever; an event waiting for
/// its scene to sync is applied as matching no parcel.
const TRANSIENT_GRACE_MS: i64 = 3_600_000;
/// A waiting event is tried again after 5 s, doubling up to 5 minutes.
const WAIT_FIRST_RETRY_MS: i64 = 5_000;
const WAIT_MAX_RETRY_MS: i64 = 300_000;
/// Past this many waiting events a new unsynced scene matches no parcel at
/// once, so made-up scene ids cannot grow the retry work without bound.
const MAX_WAITING_EVENTS: usize = 1_000;

/// What the presence job keeps between passes: the failures counted per
/// event, and the events waiting for their scene to sync.
#[derive(Default)]
pub struct Retries {
    pub failures: HashMap<i64, u32>,
    waits: HashMap<i64, Wait>,
}

/// An event waiting for its scene: its wallet, when to try it again, and
/// how many tries it waited.
struct Wait {
    wallet: String,
    next_ms: i64,
    tries: u32,
}

impl Wait {
    fn retry_after(&mut self, now: i64) {
        let delay = WAIT_FIRST_RETRY_MS
            .saturating_mul(1 << self.tries.min(16))
            .min(WAIT_MAX_RETRY_MS);
        self.next_ms = now.saturating_add(delay);
        self.tries += 1;
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct EventRow {
    event_id: String,
    kind: String,
    room_type: String,
    realm_name: String,
    scene_id: Option<String>,
    wallet: String,
    session_id: String,
    event_time_ms: i64,
}

impl From<EventRow> for PresenceEvent {
    fn from(r: EventRow) -> Self {
        Self {
            event_id: r.event_id,
            join: r.kind == "join",
            world: r.room_type == "world",
            realm_name: r.realm_name,
            scene_id: r.scene_id,
            wallet: r.wallet,
            session_id: r.session_id,
            event_time_ms: r.event_time_ms,
        }
    }
}

/// The process clock, epoch ms; read per event, so a long drain keeps time.
pub type Clock<'a> = &'a (dyn Fn() -> i64 + Sync);

/// Applies stored events until a pass comes back short. Events left
/// waiting for their scene stay queued; the drain reads past them, and
/// forgets waits whose event is gone.
pub async fn drain(
    pool: &PgPool,
    settings: &Settings,
    asset_base: &str,
    clock: Clock<'_>,
    retries: &mut Retries,
) -> anyhow::Result<()> {
    let mut pass = Pass::default();
    while apply_from(pool, settings, asset_base, clock, retries, &mut pass).await? as i64
        >= settings.event_batch
    {}
    retries.waits.retain(|seq, _| pass.seen.contains(seq));
    metrics::gauge!("badges_presence_waiting_events").set(retries.waits.len() as f64);
    Ok(())
}

/// Where a drain stands: the last `seq` read, the wallets one of whose
/// events waits for its scene (their later events wait too, keeping order)
/// and the waiting events still queued.
#[derive(Default)]
struct Pass {
    after: i64,
    waiting: HashSet<String>,
    seen: HashSet<i64>,
    /// Events applied this drain.
    applied: usize,
}

/// One pass over the head of the queue; see [`drain`].
pub async fn apply_pending(
    pool: &PgPool,
    settings: &Settings,
    asset_base: &str,
    clock: Clock<'_>,
    retries: &mut Retries,
) -> anyhow::Result<usize> {
    let mut pass = Pass::default();
    apply_from(pool, settings, asset_base, clock, retries, &mut pass).await?;
    Ok(pass.applied)
}

/// The age in ms of the oldest queued event whose wallet is not waiting for
/// a scene, over the whole queue (waiting wallets can fill a batch).
async fn oldest_not_waiting_ms(
    pool: &PgPool,
    retries: &Retries,
) -> Result<Option<i64>, sqlx::Error> {
    let waiting: Vec<String> = retries.waits.values().map(|w| w.wallet.clone()).collect();
    sqlx::query_scalar(
        "SELECT (extract(epoch FROM timezone('UTC', now()) - received_at) * 1000)::bigint \
         FROM social.presence_events WHERE wallet <> ALL($1) ORDER BY seq LIMIT 1",
    )
    .bind(&waiting)
    .fetch_optional(pool)
    .await
}

/// Applies up to `event_batch` stored events after `pass.after` in `seq`
/// order, each in its own transaction that deletes it and with the clock
/// read for it, and returns how many it read. `retries.failures` counts the
/// passes an event failed (kept by the job); the first failure ends the
/// pass with the error. An event a scene badge must wait for (its scene is
/// not synced yet) stays queued, as does every later event of its wallet
/// in the drain, and is tried again with backoff. The age of the oldest
/// queued event not waiting is reported as
/// `badges_presence_queue_oldest_seconds`.
async fn apply_from(
    pool: &PgPool,
    settings: &Settings,
    asset_base: &str,
    clock: Clock<'_>,
    retries: &mut Retries,
    pass: &mut Pass,
) -> anyhow::Result<usize> {
    let queued: Vec<(i64, i64, String)> = sqlx::query_as(
        "SELECT seq, (extract(epoch FROM timezone('UTC', now()) - received_at) * 1000)::bigint, wallet \
         FROM social.presence_events WHERE seq > $2 ORDER BY seq LIMIT $1",
    )
    .bind(settings.event_batch)
    .bind(pass.after)
    .fetch_all(pool)
    .await?;
    if pass.after == 0 {
        match oldest_not_waiting_ms(pool, retries).await {
            Ok(oldest) => metrics::gauge!("badges_presence_queue_oldest_seconds")
                .set(oldest.unwrap_or(0).max(0) as f64 / 1000.0),
            // keep the last reading; 0 would report an empty queue
            Err(error) => {
                metrics::counter!("badges_presence_queue_age_read_failures").increment(1);
                tracing::warn!(%error, "presence queue age read failed");
            }
        }
    }
    let read = queued.len();
    for (seq, queued_ms, wallet) in queued {
        pass.after = seq;
        if pass.waiting.contains(&wallet) {
            // keep a wait an earlier event of its wallet holds up
            if retries.waits.contains_key(&seq) {
                pass.seen.insert(seq);
            }
            continue;
        }
        let now = clock();
        if let Some(wait) = retries.waits.get(&seq) {
            pass.seen.insert(seq);
            if now < wait.next_ms {
                pass.waiting.insert(wallet);
                continue;
            }
        }
        let flow = Flow {
            settings,
            now,
            asset_base,
            wait_for_scenes: queued_ms <= TRANSIENT_GRACE_MS
                && (retries.waits.contains_key(&seq) || retries.waits.len() < MAX_WAITING_EVENTS),
        };
        match apply_one(pool, &flow, seq).await {
            Ok(outcome) => {
                retries.failures.remove(&seq);
                retries.waits.remove(&seq);
                pass.applied += usize::from(outcome.is_some());
                for o in outcome.into_iter().flat_map(|o| o.outcomes) {
                    metrics::counter!("badges_presence_events_total", "outcome" => o).increment(1);
                }
            }
            Err(error) if engine::is_scene_not_synced(&error) => {
                let wait = retries.waits.entry(seq).or_insert_with(|| {
                    tracing::debug!(seq, %error, "presence event waits for its scene");
                    metrics::counter!("badges_presence_events_total", "outcome" => "scene_not_synced")
                        .increment(1);
                    Wait {
                        wallet: wallet.clone(),
                        next_ms: now,
                        tries: 0,
                    }
                });
                wait.retry_after(now);
                pass.seen.insert(seq);
                pass.waiting.insert(wallet);
            }
            Err(error) => {
                let aged = queued_ms > TRANSIENT_GRACE_MS;
                if !gives_up(&mut retries.failures, seq, &error, aged) {
                    return Err(error.into());
                }
                retries.waits.remove(&seq);
                metrics::counter!("badges_presence_events_total", "outcome" => "failed")
                    .increment(1);
                tracing::error!(seq, %error, "presence event dropped after failing {MAX_ATTEMPTS} passes");
                sqlx::query("DELETE FROM social.presence_events WHERE seq = $1")
                    .bind(seq)
                    .execute(pool)
                    .await?;
            }
        }
    }
    Ok(read)
}

/// Whether the event is dropped after this failure. A deterministic error
/// counts toward [`MAX_ATTEMPTS`]; a transient one only once the event is
/// `aged` past [`TRANSIENT_GRACE_MS`], so an outage alone drops nothing.
fn gives_up(failures: &mut HashMap<i64, u32>, seq: i64, error: &sqlx::Error, aged: bool) -> bool {
    if is_transient(error) && !aged {
        return false;
    }
    let attempts = failures.entry(seq).or_default();
    *attempts += 1;
    if *attempts < MAX_ATTEMPTS {
        return false;
    }
    failures.remove(&seq);
    true
}

/// Errors a later pass can clear: the connection or pool, a timeout or
/// cancel, a serialization or deadlock conflict, a read-only failover.
fn is_transient(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Io(_)
        | sqlx::Error::Tls(_)
        | sqlx::Error::Protocol(_)
        | sqlx::Error::PoolTimedOut
        | sqlx::Error::PoolClosed
        | sqlx::Error::WorkerCrashed
        | sqlx::Error::BeginFailed => true,
        sqlx::Error::Database(e) => e.code().is_some_and(|code| {
            // 08 connection, 53 resources, 57 cancel/shutdown
            ["08", "53", "57"]
                .iter()
                .any(|class| code.starts_with(class))
                || matches!(code.as_ref(), "40001" | "40P01" | "55P03" | "25006")
        }),
        _ => false,
    }
}

async fn apply_one(pool: &PgPool, flow: &Flow<'_>, seq: i64) -> sqlx::Result<Option<EventOutcome>> {
    let mut tx = pool.begin().await?;
    engine::lock_flow(&mut tx).await?;
    let row: Option<EventRow> = sqlx::query_as(
        "DELETE FROM social.presence_events WHERE seq = $1 \
         RETURNING event_id, kind, room_type, realm_name, scene_id, wallet, session_id, event_time_ms",
    )
    .bind(seq)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let outcome = flow.apply(&mut tx, &row.into()).await?;
    tx.commit().await?;
    Ok(Some(outcome))
}

/// One sweeper pass in one transaction.
pub async fn sweep(
    pool: &PgPool,
    flow: &Flow<'_>,
    occupancy: Option<&dyn Occupancy>,
) -> sqlx::Result<SweepSummary> {
    let mut tx = pool.begin().await?;
    engine::lock_flow(&mut tx).await?;
    let summary = flow.sweep(&mut tx, occupancy).await?;
    tx.commit().await?;
    Ok(summary)
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// The two leased jobs; every process that receives webhooks stores events ([`EventSink`]).
pub fn spawn_jobs(
    pool: PgPool,
    asset_base: String,
    occupancy: Arc<dyn Occupancy>,
    settings: Settings,
    jobs: &crate::jobs::Jobs,
) {
    let settings = Arc::new(settings);
    let asset_base = Arc::new(asset_base);
    let retries = Arc::new(tokio::sync::Mutex::new(Retries::default()));
    let (p, s, a) = (pool.clone(), settings.clone(), asset_base.clone());
    jobs.spawn_periodic_leased(
        "badges-presence",
        crate::jobs::PeriodicCfg::every(settings.poll_interval)
            .with_error_backoff(Duration::from_secs(60)),
        move |_ctx| {
            let (pool, settings, asset_base, retries) =
                (p.clone(), s.clone(), a.clone(), retries.clone());
            async move {
                let mut retries = retries.lock().await;
                drain(&pool, &settings, &asset_base, &now_ms, &mut retries).await
            }
        },
    );
    jobs.spawn_periodic_leased(
        "badges-presence-sweep",
        crate::jobs::PeriodicCfg::every(settings.sweep_interval)
            .with_jitter(Duration::from_secs(10))
            .with_error_backoff(Duration::from_secs(600)),
        move |_ctx| {
            let (pool, settings, asset_base, occupancy) = (
                pool.clone(),
                settings.clone(),
                asset_base.clone(),
                occupancy.clone(),
            );
            async move {
                let flow = Flow {
                    settings: &settings,
                    now: now_ms(),
                    asset_base: &asset_base,
                    wait_for_scenes: false,
                };
                let occupancy = settings.sweeper_enabled.then_some(occupancy.as_ref());
                let summary = sweep(&pool, &flow, occupancy).await?;
                if summary != SweepSummary::default() {
                    tracing::info!(?summary, "presence visits swept");
                }
                Ok(())
            }
        },
    );
}
