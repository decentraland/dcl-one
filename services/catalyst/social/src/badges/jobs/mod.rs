//! The badge processor: upstream's event consumer as leased periodic jobs
//! over the tables that already hold its inputs, each feeding
//! [`super::evaluators`] through [`super::apply_progress`].
//!
//! - `badges-client-events`: `social.client_events` by `seq` (citizen,
//!   traveler, walkabout, moves master, social butterfly, vertical voyager);
//! - `badges-deployments`: `content.deployments` by `id` (the rarity badges,
//!   profile pro, land architect, the store half of open for business);
//! - `badges-sales`: the Polygon shop logs by chain position (emotionista,
//!   fashionista);
//! - `badges-recount`: the creators' items, the creators' collections (the
//!   other half of open for business) and the accepted referrals, recounted
//!   against the stored progress.
//!
//! Each source keeps one row of `social.badge_cursors`, locked and advanced
//! in the transaction that writes the progress of the rows it passed, so a
//! database error rolls both back and the next tick retries the same rows; a
//! row an evaluator cannot read (a malformed event) is logged and passed. A
//! cursor's first pass starts at the source's head (no history replay), and a
//! recount's first pass writes no notification. The id cursors read only
//! settled rows: a sequence value is drawn before its transaction commits, so
//! a row is passed only once every transaction older than its own has ended
//! (its `xmin` precedes the snapshot's `xmin`), and the scan stops at the
//! first row that has not.

mod client_events;
mod deployments;
mod recount;
mod sales;
#[cfg(test)]
mod tests;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use chrono::NaiveDateTime;
use sqlx::{PgConnection, PgPool};

use super::SharedState;
use super::progress::{Applied, ApplyOptions, Progress, apply_progress};
use crate::identity::Address;

pub use client_events::SOURCE as CLIENT_EVENTS_SOURCE;
pub use deployments::SOURCE as DEPLOYMENTS_SOURCE;

/// The processor's knobs (`BADGES_PROCESSOR_*`, `BADGES_EVENT_MAX_AGE_SECS`,
/// `BADGES_DEPLOYMENT_MAX_LAG_SECS`).
#[derive(Clone, Debug)]
pub struct ProcessorSettings {
    /// Run the jobs (on the primary; they are leased).
    pub enabled: bool,
    /// Rows (or recounted wallets) per transaction.
    pub batch_size: i64,
    /// Upstream's `EVENT_PROCESSING_MAX_AGE_IN_SECONDS`: an event older than
    /// this (its own time) is passed without effect. `None` keeps every one.
    pub max_event_age: Option<Duration>,
    /// A deployment the node stored this long after its entity timestamp is
    /// history a syncing node caught up on, not a new deployment: passed
    /// without effect. `None` keeps every one.
    pub deployment_max_lag: Option<Duration>,
}

impl Default for ProcessorSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            batch_size: 200,
            max_event_age: None,
            deployment_max_lag: Some(Duration::from_secs(86_400)),
        }
    }
}

/// What one pass needs: the pool, the notification asset base, the knobs.
#[derive(Clone)]
pub(crate) struct Pass {
    pub pool: PgPool,
    pub asset_base: String,
    pub settings: ProcessorSettings,
}

impl Pass {
    pub fn new(state: &SharedState) -> Self {
        Self {
            pool: state.badges.pool().clone(),
            asset_base: state.badges.asset_base_url().to_string(),
            settings: state.settings.processor.clone(),
        }
    }

    fn batch(&self) -> i64 {
        self.settings.batch_size.clamp(1, 10_000)
    }

    /// `true` when an event at `at` is past the age window.
    fn too_old(&self, at: NaiveDateTime, now: NaiveDateTime) -> bool {
        self.settings
            .max_event_age
            .is_some_and(|max| chrono::Duration::from_std(max).is_ok_and(|max| at < now - max))
    }
}

/// Batches one tick drains at most per source before yielding.
const MAX_BATCHES_PER_TICK: usize = 50;

/// The transaction horizon: every transaction whose id precedes it had ended
/// when it was read. Read as the transaction's first statement (before it
/// takes an id of its own); `settled_sql` compares a row's `xmin` to it.
pub(crate) async fn horizon(conn: &mut PgConnection) -> Result<String, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT (pg_snapshot_xmin(pg_current_snapshot())::text::numeric % 4294967296)::text",
    )
    .fetch_one(conn)
    .await
}

/// A row column: its inserting transaction precedes the horizon bound at `$p`
/// (wraparound-safe through `age`; a frozen row is always settled).
pub(crate) fn settled_sql(alias: &str, param: &str) -> String {
    format!("(age({alias}.xmin) > age({param}::xid))")
}

/// The cursor of `source`, locked for this transaction; `None` when the
/// source has none yet (the caller then records the source's head with
/// [`init_cursor`] and ends the pass, so the first pass replays no history).
pub(crate) async fn lock_cursor(
    conn: &mut PgConnection,
    source: &str,
) -> Result<Option<(i64, i32)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT position, log_index FROM social.badge_cursors WHERE source = $1 FOR UPDATE",
    )
    .bind(source)
    .fetch_optional(conn)
    .await
}

pub(crate) async fn init_cursor(
    conn: &mut PgConnection,
    source: &str,
    position: i64,
    log_index: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO social.badge_cursors (source, position, log_index) VALUES ($1, $2, $3) \
         ON CONFLICT (source) DO NOTHING",
    )
    .bind(source)
    .bind(position)
    .bind(log_index)
    .execute(conn)
    .await?;
    tracing::info!(
        source,
        position,
        "badge cursor initialized at the source's head (no history replay)"
    );
    Ok(())
}

pub(crate) async fn advance(
    conn: &mut PgConnection,
    source: &str,
    position: i64,
    log_index: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE social.badge_cursors SET position = $2, log_index = $3, \
         updated_at = timezone('UTC', now()) WHERE source = $1",
    )
    .bind(source)
    .bind(position)
    .bind(log_index)
    .execute(conn)
    .await?;
    Ok(())
}

/// The (address, badge) pairs of `addresses` already complete: an evaluator
/// leaves those untouched, so the batch skips their write path.
pub(crate) async fn completed(
    conn: &mut PgConnection,
    addresses: &[String],
) -> Result<HashSet<(String, String)>, sqlx::Error> {
    if addresses.is_empty() {
        return Ok(HashSet::new());
    }
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT address, badge_id FROM social.user_badge_progress \
         WHERE address = ANY($1) AND completed_at IS NOT NULL",
    )
    .bind(addresses)
    .fetch_all(conn)
    .await?;
    Ok(rows.into_iter().collect())
}

/// One evaluator write, with its notification (`notify`) unless suppressed.
pub(crate) async fn award<F>(
    conn: &mut PgConnection,
    address: &Address,
    badge_id: &str,
    notify: Option<&str>,
    update: F,
) -> Result<Applied, sqlx::Error>
where
    F: FnOnce(&mut Progress),
{
    let options = ApplyOptions {
        at: crate::shared::now_utc(),
        notify,
        granted_by: None,
    };
    let applied = apply_progress(conn, address, badge_id, &options, update).await?;
    if let Applied::Written(change) = &applied
        && (change.completed || !change.new_tiers.is_empty())
    {
        tracing::info!(
            address = address.as_str(),
            badge_id,
            tiers = ?change.new_tiers,
            completed = change.completed,
            "badge progress granted"
        );
    }
    Ok(applied)
}

/// Spawns the processor's jobs (leased: only the primary runs them).
pub fn spawn(state: &SharedState, jobs: &crate::jobs::Jobs) {
    if !state.settings.processor.enabled {
        tracing::info!("BADGES_PROCESSOR_ENABLED=false: no badge processor");
        return;
    }
    let pass = Arc::new(Pass::new(state));
    spawn_one(
        jobs,
        "badges-client-events",
        Duration::from_secs(15),
        pass.clone(),
        |p| Box::pin(async move { client_events::run(&p).await }),
    );
    spawn_one(
        jobs,
        "badges-deployments",
        Duration::from_secs(30),
        pass.clone(),
        |p| Box::pin(async move { deployments::run(&p).await }),
    );
    spawn_one(
        jobs,
        "badges-sales",
        Duration::from_secs(60),
        pass.clone(),
        |p| Box::pin(async move { sales::run(&p).await }),
    );
    spawn_one(
        jobs,
        "badges-recount",
        Duration::from_secs(300),
        pass,
        |p| Box::pin(async move { recount::run(&p).await }),
    );
    tracing::info!("badge processor up");
}

type PassFuture = std::pin::Pin<Box<dyn Future<Output = anyhow::Result<u64>> + Send>>;

fn spawn_one(
    jobs: &crate::jobs::Jobs,
    name: &'static str,
    every: Duration,
    pass: Arc<Pass>,
    run: fn(Arc<Pass>) -> PassFuture,
) {
    jobs.spawn_periodic_leased(
        name,
        crate::jobs::PeriodicCfg::every(every)
            .with_jitter(every / 4)
            .with_error_backoff(Duration::from_secs(600)),
        move |ctx| {
            let pass = pass.clone();
            async move {
                // A restore hold keeps every social write back, these too.
                if crate::shared::writes::refusal(true).is_some() {
                    return Ok(());
                }
                let written = run(pass).await?;
                if written > 0 {
                    ctx.progress(format!("{written} rows passed"));
                }
                Ok(())
            }
        },
    );
}
