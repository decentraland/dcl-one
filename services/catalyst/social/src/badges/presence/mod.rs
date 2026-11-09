//! Presence badges (upstream badges #246): a badge configured in
//! `social.presence_badges` is earned by being in a scene (by parcel) or a
//! world inside a UTC window, on a `visit` or for a `stay` of at least
//! `required_seconds`.
//!
//! Input: the gatekeeper's verified LiveKit joins and leaves, which `serve`
//! fans out to [`sink::EventSink`]; it queues them without blocking the
//! webhook and its writer stores them in `social.presence_events`. The
//! leased `badges-presence` job applies the stored events in order, one
//! transaction per event that also deletes it ([`engine`]); the leased
//! `badges-presence-sweep` job closes visits whose leave was lost
//! ([`sweeper`]) against this process's LiveKit occupancy. Grants go through
//! [`crate::badges::apply_progress`] with `badge_granted` notifications, and
//! the progress document keeps upstream's `intervals`.
//!
//! Enabled by `BADGES_PRESENCE_ENABLED`.

pub mod engine;
pub mod intervals;
pub mod sink;
pub mod sweeper;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
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
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            check_after_ms: 900_000,
            max_age_ms: 86_400_000,
            applied_events_ttl_ms: 604_800_000,
            absent_expire_ms: 3_600_000,
            lookahead_ms: 6 * 3_600_000,
            sweep_batch: 100,
            prune_batch: 1000,
            event_batch: 500,
            poll_interval: Duration::from_secs(5),
            sweep_interval: Duration::from_secs(60),
        }
    }
}

/// An event that failed this many passes is dropped (logged and counted),
/// so one bad row cannot stop the flow.
const MAX_ATTEMPTS: u32 = 3;
/// Past this age in the queue a transient failure counts too, so an event
/// that always times out cannot hold the queue forever.
const TRANSIENT_GRACE_MS: i64 = 3_600_000;

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

/// Applies stored events until a pass comes back short.
pub async fn drain(
    pool: &PgPool,
    settings: &Settings,
    asset_base: &str,
    clock: Clock<'_>,
    failures: &mut HashMap<i64, u32>,
) -> anyhow::Result<()> {
    while apply_pending(pool, settings, asset_base, clock, failures).await? as i64
        >= settings.event_batch
    {}
    Ok(())
}

/// Applies up to `event_batch` stored events in `seq` order, each in its own
/// transaction that deletes it and with the clock read for it. `failures`
/// counts the passes an event failed (kept by the job); the first failure
/// ends the pass with the error. The age of the oldest queued event is
/// reported as `badges_presence_queue_oldest_seconds`.
pub async fn apply_pending(
    pool: &PgPool,
    settings: &Settings,
    asset_base: &str,
    clock: Clock<'_>,
    failures: &mut HashMap<i64, u32>,
) -> anyhow::Result<usize> {
    let queued: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT seq, (extract(epoch FROM timezone('UTC', now()) - received_at) * 1000)::bigint \
         FROM social.presence_events ORDER BY seq LIMIT $1",
    )
    .bind(settings.event_batch)
    .fetch_all(pool)
    .await?;
    let oldest = queued.iter().map(|(_, age)| *age).max().unwrap_or(0);
    metrics::gauge!("badges_presence_queue_oldest_seconds").set(oldest.max(0) as f64 / 1000.0);
    let mut applied = 0;
    for (seq, queued_ms) in queued {
        let flow = Flow {
            settings,
            now: clock(),
            asset_base,
        };
        match apply_one(pool, &flow, seq).await {
            Ok(outcome) => {
                failures.remove(&seq);
                applied += usize::from(outcome.is_some());
                for o in outcome.into_iter().flat_map(|o| o.outcomes) {
                    metrics::counter!("badges_presence_events_total", "outcome" => o).increment(1);
                }
            }
            Err(error) => {
                let aged = queued_ms > TRANSIENT_GRACE_MS;
                if !gives_up(failures, seq, &error, aged) {
                    return Err(error.into());
                }
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
    Ok(applied)
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

/// The two leased jobs; the writer runs on every process ([`EventSink::spawn_writer`]).
pub fn spawn_jobs(
    pool: PgPool,
    asset_base: String,
    occupancy: Arc<dyn Occupancy>,
    settings: Settings,
    jobs: &crate::jobs::Jobs,
) {
    let settings = Arc::new(settings);
    let asset_base = Arc::new(asset_base);
    let failures = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
    let (p, s, a) = (pool.clone(), settings.clone(), asset_base.clone());
    jobs.spawn_periodic_leased(
        "badges-presence",
        crate::jobs::PeriodicCfg::every(settings.poll_interval)
            .with_error_backoff(Duration::from_secs(60)),
        move |_ctx| {
            let (pool, settings, asset_base, failures) =
                (p.clone(), s.clone(), a.clone(), failures.clone());
            async move {
                let mut failures = failures.lock().await;
                drain(&pool, &settings, &asset_base, &now_ms, &mut failures).await
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
                };
                let summary = sweep(&pool, &flow, Some(occupancy.as_ref())).await?;
                if summary != SweepSummary::default() {
                    tracing::info!(?summary, "presence visits swept");
                }
                Ok(())
            }
        },
    );
}
