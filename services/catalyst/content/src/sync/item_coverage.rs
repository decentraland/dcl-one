//! Item history coverage: whether this node's accepted content holds every
//! wearable and emote deployment its sync peers offer, read through
//! [`item_history_coverage`].
//!
//! The sync leader proves it with [`ItemHistoryReplay`]: recurring strict
//! passes over its peers' whole `/pointer-changes` history of the item types,
//! from the history floor, beside the incremental sync and independently of
//! its cursors, processed snapshots and sync state. A pass schedules only the
//! entities the node does not hold, then counts as debt every delta it could
//! not prove accepted. The pass in progress is `content.item_history_replay`;
//! the last completed one is `content.item_history_coverage`, stamped with
//! the lease session that held the `sync` lease. Coverage is an as-observed
//! fact about the configured peers, never a global absence proof.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use chrono::NaiveDateTime;
use sqlx::{PgConnection, PgPool, Row};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use super::orchestrator::SyncControl;
use super::pointer_changes::{fetch_page, is_usable_timestamp};
use super::{SyncDeployment, SyncError, Timestamp};
use crate::batch_deployer::BatchDeployer;
use crate::sync_deployer::deployed_entity_ids;

/// The item types a pass replays and the reader answers for.
pub const ITEM_TYPES: &[&str] = &["emote", "wearable"];
/// The replay's proof rules; a coverage row written under another is unknown.
pub const ALGORITHM: i32 = 1;
/// How long after its pass started a completed pass still proves coverage.
pub const STALE_AFTER: Duration = Duration::from_secs(30 * 60);
/// The fleet lease whose holder alone writes coverage.
pub const SYNC_LEASE: &str = "sync";

/// What [`item_history_coverage`] answers for one item type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coverage {
    /// Every delta the peers offered from history zero, read in a pass that
    /// started at `as_of` (UTC), is accepted here.
    Covered { as_of: NaiveDateTime },
    /// Nothing may be concluded from a missing entity.
    Unknown(Unknown),
}

/// Why coverage is unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unknown {
    /// No completed pass recorded: a fresh or legacy database.
    NoRecord,
    /// Written under other proof rules.
    Algorithm,
    /// The leader's `SYNC_ENTITY_TYPES` leaves the type out.
    ExcludedType,
    /// The leader syncs from a nonzero `SYNC_FROM_TIMESTAMP_MS`.
    HistoryFloor,
    /// The pass met deltas it could not prove accepted.
    Debt,
    /// The `sync` lease is not held by the session that wrote the pass:
    /// lost, taken by another process or by a binary that writes no coverage.
    OwnerMismatch,
    /// The pass started more than [`STALE_AFTER`] ago.
    Stale,
}

macro_rules! sync_lease_holder {
    ($namespace:literal) => {
        concat!(
            "SELECT l.pid, timezone('UTC', a.backend_start) AS backend_start
               FROM pg_locks l JOIN pg_stat_activity a ON a.pid = l.pid
              WHERE l.locktype = 'advisory' AND l.granted AND l.objsubid = 2
                AND l.database = (SELECT oid FROM pg_database WHERE datname = current_database())
                AND l.classid = hashtext(",
            $namespace,
            ")::oid AND l.objid = hashtext('sync')::oid"
        )
    };
}

struct CoverageRow {
    algorithm: i32,
    entity_types: Vec<String>,
    history_floor_ms: i64,
    debt: i64,
    pass_started_at: NaiveDateTime,
    lease_pid: i32,
    lease_backend_start: NaiveDateTime,
    holder_pid: Option<i32>,
    holder_backend_start: Option<NaiveDateTime>,
    age_secs: f64,
}

fn decide(row: Option<CoverageRow>, entity_type: &str) -> Coverage {
    let Some(row) = row else {
        return Coverage::Unknown(Unknown::NoRecord);
    };
    let unknown = if row.algorithm != ALGORITHM {
        Some(Unknown::Algorithm)
    } else if !row.entity_types.iter().any(|t| t == entity_type) {
        Some(Unknown::ExcludedType)
    } else if row.history_floor_ms != 0 {
        Some(Unknown::HistoryFloor)
    } else if row.debt != 0 {
        Some(Unknown::Debt)
    } else if row.holder_pid != Some(row.lease_pid)
        || row.holder_backend_start != Some(row.lease_backend_start)
    {
        Some(Unknown::OwnerMismatch)
    } else if !(row.age_secs >= 0.0 && row.age_secs <= STALE_AFTER.as_secs_f64()) {
        Some(Unknown::Stale)
    } else {
        None
    };
    match unknown {
        Some(why) => Coverage::Unknown(why),
        None => Coverage::Covered {
            as_of: row.pass_started_at,
        },
    }
}

/// Whether the node's accepted content covers the peers' whole history of
/// `entity_type` (`wearable` or `emote`): one statement, so a caller that
/// reads entities in the same REPEATABLE READ transaction reads them from
/// the snapshot this answer was taken in. Any other type is unknown.
pub async fn item_history_coverage(
    conn: &mut PgConnection,
    entity_type: &str,
) -> sqlx::Result<Coverage> {
    let row = sqlx::query(concat!(
        "SELECT c.algorithm, c.entity_types, c.history_floor_ms, c.debt, c.pass_started_at,
                c.lease_pid, c.lease_backend_start, h.pid AS holder_pid,
                h.backend_start AS holder_backend_start,
                EXTRACT(EPOCH FROM timezone('UTC', now()) - c.pass_started_at)::float8 AS age_secs
           FROM content.item_history_coverage c
           LEFT JOIN LATERAL (",
        sync_lease_holder!("c.lease_namespace"),
        " LIMIT 1) h ON true"
    ))
    .fetch_optional(&mut *conn)
    .await?
    .map(|r| -> sqlx::Result<CoverageRow> {
        Ok(CoverageRow {
            algorithm: r.try_get("algorithm")?,
            entity_types: r.try_get("entity_types")?,
            history_floor_ms: r.try_get("history_floor_ms")?,
            debt: r.try_get("debt")?,
            pass_started_at: r.try_get("pass_started_at")?,
            lease_pid: r.try_get("lease_pid")?,
            lease_backend_start: r.try_get("lease_backend_start")?,
            holder_pid: r.try_get("holder_pid")?,
            holder_backend_start: r.try_get("holder_backend_start")?,
            age_secs: r.try_get("age_secs")?,
        })
    })
    .transpose()?;
    Ok(decide(row, entity_type))
}

/// The lease session the sync leader holds `sync` on: its namespace and
/// backend pid. Every replay write is conditioned on it holding the lock.
#[derive(Debug, Clone)]
pub struct LeaseOwner {
    pub namespace: String,
    pub pid: i32,
}

/// The replay's timing: the pause between pages, between passes, while the
/// operator's switch is set and the failed-fetch backoff bounds.
#[derive(Debug, Clone, Copy)]
pub struct Pacing {
    pub page_pause: Duration,
    pub pass_interval: Duration,
    pub stopped_poll: Duration,
    pub retry_start: Duration,
    pub retry_max: Duration,
}

impl Default for Pacing {
    fn default() -> Self {
        Self {
            page_pause: Duration::from_millis(200),
            pass_interval: Duration::from_secs(600),
            stopped_poll: Duration::from_secs(10),
            retry_start: Duration::from_secs(5),
            retry_max: Duration::from_secs(300),
        }
    }
}

/// A completed pass, as recorded in `content.item_history_coverage`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassReport {
    pub started_at: NaiveDateTime,
    pub debt: i64,
    pub observed_frontier_ms: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum ReplayError {
    #[error("item history replay: the sync lease or the pass changed hands")]
    Lost,
    #[error("item history replay: {0}")]
    Database(#[from] sqlx::Error),
    #[error("item history replay: {0}")]
    Sync(#[from] SyncError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Scope {
    sources: Vec<String>,
    entity_types: Vec<String>,
    history_floor_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Checkpoint {
    started_at: NaiveDateTime,
    peer: String,
    from_ms: i64,
    last_id: Option<String>,
}

enum Next {
    At(String, i64, Option<String>),
    Done,
}

/// The sync leader's strict item history replay over `peers`.
pub struct ItemHistoryReplay {
    pub pool: PgPool,
    pub client: crate::http::CheckedRedirectClient,
    pub deployer: Arc<BatchDeployer>,
    pub peers: watch::Receiver<HashSet<String>>,
    /// The item types the leader syncs, a subset of [`ITEM_TYPES`].
    pub entity_types: Vec<String>,
    pub history_floor_ms: Timestamp,
    pub owner: LeaseOwner,
    pub control: SyncControl,
    pub pacing: Pacing,
}

impl ItemHistoryReplay {
    /// Passes back to back, [`Pacing::pass_interval`] apart, until `cancel`.
    pub async fn run(mut self, cancel: CancellationToken, report: impl Fn(Result<String, String>)) {
        loop {
            let wait = match self.pass(&cancel).await {
                Ok(Some(pass)) => {
                    report(Ok(format!(
                        "pass from {} complete, debt {}",
                        pass.started_at, pass.debt
                    )));
                    self.pacing.pass_interval
                }
                Ok(None) => self.pacing.retry_start,
                Err(e) => {
                    report(Err(e.to_string()));
                    self.pacing.retry_start
                }
            };
            if sleep_or_cancel(&cancel, wait).await {
                return;
            }
        }
    }

    fn scope(&mut self) -> Scope {
        let mut sources: Vec<String> = self.peers.borrow_and_update().iter().cloned().collect();
        sources.sort();
        let mut entity_types: Vec<String> = ITEM_TYPES
            .iter()
            .filter(|t| self.entity_types.iter().any(|e| e == *t))
            .map(|t| t.to_string())
            .collect();
        entity_types.sort();
        Scope {
            sources,
            entity_types,
            history_floor_ms: self.history_floor_ms,
        }
    }

    /// Resumes the recorded pass of this scope or starts a new one, then
    /// walks it to its end: `None` when cancelled, when the peer set changed
    /// or when there is nothing to replay.
    pub async fn pass(
        &mut self,
        cancel: &CancellationToken,
    ) -> Result<Option<PassReport>, ReplayError> {
        let scope = self.scope();
        if scope.sources.is_empty() || scope.entity_types.is_empty() {
            return Ok(None);
        }
        let mut at = self.begin(&scope).await?;
        loop {
            if self.control.wait_while_paused(cancel).await
                || !self.wait_while_stopped(cancel).await
            {
                return Ok(None);
            }
            if self.peers.has_changed().unwrap_or(false) {
                return Ok(None);
            }
            let Some((items, more)) = self.fetch(&at, &scope, cancel).await else {
                return Ok(None);
            };
            let keyset = more.then(|| keyset(&items)).flatten();
            let (mut debt, frontier) = self.accept(items, &scope, &at.peer).await?;
            let next = match keyset {
                Some((from, last)) if (from, Some(&last)) != (at.from_ms, at.last_id.as_ref()) => {
                    Next::At(at.peer.clone(), from, Some(last))
                }
                stuck => {
                    if more || stuck.is_some() {
                        debt += 1;
                    }
                    match scope.sources.iter().position(|p| *p == at.peer) {
                        Some(i) if i + 1 < scope.sources.len() => {
                            Next::At(scope.sources[i + 1].clone(), scope.history_floor_ms, None)
                        }
                        _ => Next::Done,
                    }
                }
            };
            match next {
                Next::Done => {
                    return self.complete(&scope, &at, debt, frontier).await.map(Some);
                }
                Next::At(peer, from_ms, last_id) => {
                    let next = Checkpoint {
                        started_at: at.started_at,
                        peer,
                        from_ms,
                        last_id,
                    };
                    self.save(&at, &next, debt, frontier).await?;
                    at = next;
                }
            }
            if sleep_or_cancel(cancel, self.pacing.page_pause).await {
                return Ok(None);
            }
        }
    }

    async fn wait_while_stopped(&self, cancel: &CancellationToken) -> bool {
        loop {
            let stopped: Option<bool> =
                match sqlx::query_scalar("SELECT stopped FROM content.item_history_replay")
                    .fetch_optional(&self.pool)
                    .await
                {
                    Ok(stopped) => stopped,
                    Err(e) => {
                        warn!(error = %e, "item history replay: reading the stop switch failed");
                        Some(true)
                    }
                };
            if !stopped.unwrap_or(false) {
                return true;
            }
            if sleep_or_cancel(cancel, self.pacing.stopped_poll).await {
                return false;
            }
        }
    }

    async fn begin(&self, scope: &Scope) -> Result<Checkpoint, ReplayError> {
        let recorded = sqlx::query(
            "SELECT algorithm, source_set, entity_types, history_floor_ms, pass_started_at, peer,
                    from_ms, last_id
               FROM content.item_history_replay",
        )
        .fetch_optional(&self.pool)
        .await?;
        let previous_start: Option<NaiveDateTime> = match &recorded {
            Some(r) => r.try_get("pass_started_at")?,
            None => None,
        };
        if let Some(r) = &recorded {
            let same = r.try_get::<Option<i32>, _>("algorithm")? == Some(ALGORITHM)
                && r.try_get::<Option<Vec<String>>, _>("source_set")?.as_ref()
                    == Some(&scope.sources)
                && r.try_get::<Option<Vec<String>>, _>("entity_types")?
                    .as_ref()
                    == Some(&scope.entity_types)
                && r.try_get::<Option<i64>, _>("history_floor_ms")? == Some(scope.history_floor_ms);
            let peer: Option<String> = r.try_get("peer")?;
            let from_ms: Option<i64> = r.try_get("from_ms")?;
            if let (true, Some(started_at), Some(peer), Some(from_ms)) =
                (same, previous_start, peer, from_ms)
                && scope.sources.contains(&peer)
            {
                return Ok(Checkpoint {
                    started_at,
                    peer,
                    from_ms,
                    last_id: r.try_get("last_id")?,
                });
            }
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query(concat!(
            "DELETE FROM content.item_history_coverage
              WHERE (algorithm, source_set, entity_types, history_floor_ms)
                    IS DISTINCT FROM ($1, $2, $3, $4)
                AND EXISTS (",
            sync_lease_holder!("$5"),
            " AND l.pid = $6)"
        ))
        .bind(ALGORITHM)
        .bind(&scope.sources)
        .bind(&scope.entity_types)
        .bind(scope.history_floor_ms)
        .bind(&self.owner.namespace)
        .bind(self.owner.pid)
        .execute(&mut *tx)
        .await?;
        let started_at: Option<NaiveDateTime> = sqlx::query_scalar(concat!(
            "INSERT INTO content.item_history_replay
                    (id, algorithm, source_set, entity_types, history_floor_ms, pass_started_at,
                     peer, from_ms, last_id, debt, observed_frontier_ms, updated_at)
             SELECT true, $1, $2, $3, $4, timezone('UTC', now()), $2[1], $4, NULL, 0, 0,
                    timezone('UTC', now())
               FROM (",
            sync_lease_holder!("$5"),
            ") h WHERE h.pid = $6
             ON CONFLICT (id) DO UPDATE SET
                    algorithm = EXCLUDED.algorithm, source_set = EXCLUDED.source_set,
                    entity_types = EXCLUDED.entity_types,
                    history_floor_ms = EXCLUDED.history_floor_ms,
                    pass_started_at = EXCLUDED.pass_started_at, peer = EXCLUDED.peer,
                    from_ms = EXCLUDED.from_ms, last_id = NULL, debt = 0,
                    observed_frontier_ms = 0, updated_at = EXCLUDED.updated_at
             RETURNING pass_started_at"
        ))
        .bind(ALGORITHM)
        .bind(&scope.sources)
        .bind(&scope.entity_types)
        .bind(scope.history_floor_ms)
        .bind(&self.owner.namespace)
        .bind(self.owner.pid)
        .fetch_optional(&mut *tx)
        .await?;
        let started_at = started_at.ok_or(ReplayError::Lost)?;
        tx.commit().await?;
        Ok(Checkpoint {
            started_at,
            peer: scope.sources[0].clone(),
            from_ms: scope.history_floor_ms,
            last_id: None,
        })
    }

    async fn save(
        &self,
        at: &Checkpoint,
        next: &Checkpoint,
        debt: i64,
        frontier: i64,
    ) -> Result<(), ReplayError> {
        let saved = sqlx::query(concat!(
            "UPDATE content.item_history_replay
                SET peer = $1, from_ms = $2, last_id = $3, debt = debt + $4,
                    observed_frontier_ms = GREATEST(observed_frontier_ms, $5),
                    updated_at = timezone('UTC', now())
              WHERE pass_started_at = $6 AND peer = $7 AND from_ms = $8
                AND last_id IS NOT DISTINCT FROM $9
                AND EXISTS (",
            sync_lease_holder!("$10"),
            " AND l.pid = $11)"
        ))
        .bind(&next.peer)
        .bind(next.from_ms)
        .bind(&next.last_id)
        .bind(debt)
        .bind(frontier)
        .bind(at.started_at)
        .bind(&at.peer)
        .bind(at.from_ms)
        .bind(&at.last_id)
        .bind(&self.owner.namespace)
        .bind(self.owner.pid)
        .execute(&self.pool)
        .await?;
        if saved.rows_affected() == 1 {
            Ok(())
        } else {
            Err(ReplayError::Lost)
        }
    }

    async fn complete(
        &self,
        scope: &Scope,
        at: &Checkpoint,
        debt: i64,
        frontier: i64,
    ) -> Result<PassReport, ReplayError> {
        let mut tx = self.pool.begin().await?;
        let pass = sqlx::query(
            "SELECT debt + $1 AS debt, GREATEST(observed_frontier_ms, $2) AS frontier
               FROM content.item_history_replay
              WHERE pass_started_at = $3 AND peer = $4 AND from_ms = $5
                AND last_id IS NOT DISTINCT FROM $6
             FOR UPDATE",
        )
        .bind(debt)
        .bind(frontier)
        .bind(at.started_at)
        .bind(&at.peer)
        .bind(at.from_ms)
        .bind(&at.last_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ReplayError::Lost)?;
        let report = PassReport {
            started_at: at.started_at,
            debt: pass.try_get("debt")?,
            observed_frontier_ms: pass.try_get("frontier")?,
        };
        let written = sqlx::query(concat!(
            "INSERT INTO content.item_history_coverage AS c
                    (id, algorithm, source_set, entity_types, history_floor_ms, pass_started_at,
                     pass_completed_at, observed_frontier_ms, debt, lease_namespace, lease_pid,
                     lease_backend_start)
             SELECT true, $1, $2, $3, $4, $5, timezone('UTC', now()), $6, $7, $8, h.pid,
                    h.backend_start
               FROM (",
            sync_lease_holder!("$8"),
            ") h WHERE h.pid = $9 AND h.backend_start IS NOT NULL
             ON CONFLICT (id) DO UPDATE SET
                    algorithm = EXCLUDED.algorithm, source_set = EXCLUDED.source_set,
                    entity_types = EXCLUDED.entity_types,
                    history_floor_ms = EXCLUDED.history_floor_ms,
                    pass_started_at = EXCLUDED.pass_started_at,
                    pass_completed_at = EXCLUDED.pass_completed_at,
                    observed_frontier_ms = EXCLUDED.observed_frontier_ms, debt = EXCLUDED.debt,
                    lease_namespace = EXCLUDED.lease_namespace, lease_pid = EXCLUDED.lease_pid,
                    lease_backend_start = EXCLUDED.lease_backend_start"
        ))
        .bind(ALGORITHM)
        .bind(&scope.sources)
        .bind(&scope.entity_types)
        .bind(scope.history_floor_ms)
        .bind(at.started_at)
        .bind(report.observed_frontier_ms)
        .bind(report.debt)
        .bind(&self.owner.namespace)
        .bind(self.owner.pid)
        .execute(&mut *tx)
        .await?;
        if written.rows_affected() != 1 {
            return Err(ReplayError::Lost);
        }
        sqlx::query(
            "UPDATE content.item_history_replay
                SET pass_started_at = NULL, peer = NULL, from_ms = NULL, last_id = NULL, debt = 0,
                    observed_frontier_ms = 0, updated_at = timezone('UTC', now())",
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(report)
    }

    async fn fetch(
        &self,
        at: &Checkpoint,
        scope: &Scope,
        cancel: &CancellationToken,
    ) -> Option<(Vec<serde_json::Value>, bool)> {
        let url = page_url(at, &scope.entity_types)?;
        let mut wait = self.pacing.retry_start;
        loop {
            match fetch_page(&self.client, &url).await {
                Ok(page) => return Some(page),
                Err(e) => {
                    warn!(peer = %at.peer, error = %e, "item history replay: page fetch failed")
                }
            }
            if sleep_or_cancel(cancel, wait).await {
                return None;
            }
            wait = (wait * 2).min(self.pacing.retry_max);
        }
    }

    /// Schedules the page's item deltas the node does not hold and returns
    /// the page's debt (deltas it cannot prove accepted) and newest
    /// `localTimestamp`.
    async fn accept(
        &self,
        items: Vec<serde_json::Value>,
        scope: &Scope,
        peer: &str,
    ) -> Result<(i64, i64), ReplayError> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let mut debt = 0;
        let mut frontier = 0;
        let mut wanted: Vec<SyncDeployment> = Vec::new();
        for item in items {
            let Ok(deployment) = serde_json::from_value::<SyncDeployment>(item) else {
                debt += 1;
                continue;
            };
            if !scope.entity_types.contains(&deployment.entity_type) {
                continue;
            }
            if !is_usable_timestamp(deployment.entity_timestamp, now_ms)
                || deployment
                    .local_timestamp
                    .is_some_and(|ts| !is_usable_timestamp(ts, now_ms))
            {
                debt += 1;
                continue;
            }
            frontier = frontier.max(deployment.local_timestamp.unwrap_or(0));
            wanted.push(deployment);
        }
        let missing = self.unheld(wanted).await?;
        if missing.is_empty() {
            return Ok((debt, frontier));
        }
        let servers: Vec<String> = std::iter::once(peer.to_string())
            .chain(scope.sources.iter().filter(|s| *s != peer).cloned())
            .collect();
        for deployment in missing.clone() {
            self.deployer
                .schedule_undeployed(deployment, &servers, None)
                .await?;
        }
        self.deployer.on_idle().await?;
        let unproven: HashSet<String> = self
            .unheld(missing)
            .await?
            .into_iter()
            .map(|d| d.entity_id)
            .collect();
        Ok((debt + unproven.len() as i64, frontier))
    }

    async fn unheld(
        &self,
        deployments: Vec<SyncDeployment>,
    ) -> Result<Vec<SyncDeployment>, ReplayError> {
        let probe: Vec<(&str, Timestamp)> = deployments
            .iter()
            .map(|d| (d.entity_id.as_str(), d.entity_timestamp))
            .collect();
        let held = deployed_entity_ids(&self.pool, &probe).await?;
        Ok(deployments
            .into_iter()
            .filter(|d| !held.contains(&d.entity_id))
            .collect())
    }
}

fn page_url(at: &Checkpoint, entity_types: &[String]) -> Option<String> {
    let mut url = url::Url::parse(&format!("{}/pointer-changes", at.peer)).ok()?;
    {
        let mut query = url.query_pairs_mut();
        for t in entity_types {
            query.append_pair("entityType", t);
        }
        query
            .append_pair("sortingOrder", "ASC")
            .append_pair("sortingField", "local_timestamp")
            .append_pair("from", &at.from_ms.to_string());
        if let Some(last) = &at.last_id {
            query.append_pair("lastId", last);
        }
    }
    Some(url.to_string())
}

fn keyset(page: &[serde_json::Value]) -> Option<(i64, String)> {
    page.iter().rev().find_map(|delta| {
        Some((
            delta.get("localTimestamp")?.as_i64()?,
            delta.get("entityId")?.as_str()?.to_string(),
        ))
    })
}

async fn sleep_or_cancel(cancel: &CancellationToken, wait: Duration) -> bool {
    tokio::select! {
        _ = cancel.cancelled() => true,
        _ = tokio::time::sleep(wait) => cancel.is_cancelled(),
    }
}

#[cfg(test)]
mod tests;
