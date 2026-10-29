//! Telemetry: the local Sentry and Segment sinks and their console (the old
//! `catalyrst-telemetry`, gateway prefix `/telemetry`) over the ONE pool,
//! client and job lifecycle; schema `telemetry` in `schemas/60_ops.sql`.
//! Everything writes `telemetry.events` through one bounded batch
//! writer ([`ingest::WriteBuffer`]: a 10,000-row queue coalesced into
//! 256-row `UNNEST` inserts per 200 ms, a refused chunk retried row by row,
//! a full queue dropping and counting) behind [`IngestControl`], the
//! admission that bounds the sink independently of anything the client
//! chooses: the operator's runtime toggle (`telemetry.ingest_settings`); a
//! per-project daily quota (`telemetry.project_quotas`, the exact project,
//! then a `*` row, then `TELEMETRY_DEFAULT_DAILY_LIMIT`, so a project no
//! operator ever named is capped too: the project is the Sentry URL segment
//! or the Segment write key, i.e. attacker-chosen); a process-wide rows
//! ceiling (`TELEMETRY_MAX_ROWS_PER_MINUTE`, a token bucket; sized under
//! the purge job's daily budget so a flood at the ceiling never outruns
//! retention); a process-wide bytes ceiling (`TELEMETRY_MAX_BYTES_PER_MINUTE`
//! over the decoded body); [`MAX_ITEMS_PER_REQUEST`] envelope items or batch
//! events per request; and [`MAX_DECODED_BYTES`] (2 MiB, the route body
//! cap) for an inflated gzip/deflate body. A refused event is dropped and
//! counted (`telemetry_ingest_dropped_total{reason}`); the sinks keep
//! answering the success shape, as a Sentry relay or Segment does.
//!
//! Switches. `TELEMETRY_INGEST_ENABLED` (default false) gates the two sinks
//! alone: an event is not user-authoritative state, so a node in catch-up
//! may accept telemetry before it accepts writes, and off they answer 503
//! with [`super::INGEST_DISABLED_MESSAGE`]. `OPS_WRITES_ENABLED` (default
//! false) gates every console mutation (`/dash/issue/state`, `/dash/{flag,
//! experiment,groups,group/target,area}`, `/dash/admin/*`) with the ops
//! envelope and `Retry-After`, the social gate's shape; `POST /login` (a
//! cookie mint) and `POST /dash/sql` (a `READ ONLY` transaction under a 15 s
//! `statement_timeout` on the telemetry reader's own login, [`sql`]) are
//! reads and stay open.
//!
//! Authorization, kept exactly: `CATALYRST_TELEMETRY_ADMIN_TOKEN` (or
//! `_FILE`; unset answers 403 "admin disabled", fail closed) as a bearer on
//! every write and on the JSON reads, or the 30-day `telemetry_admin` session
//! cookie the `/login` form mints for the console's GET reads and pages (an
//! HMAC under the token of the session's expiry, [`login::session_value`]:
//! the browser never holds the bearer; an HTML request without it is
//! redirected to `/login`); every compare is timing-safe; the
//! `/dash/admin/*` handlers name the [`admin::TelemetryAdmin`]
//! extractor, so the check is a demanded type, not a deletable line. Every
//! operator mutation writes its `ops.admin_audit` row in the same statement
//! (`admin::audited`), actor `token:CATALYRST_TELEMETRY_ADMIN_TOKEN` (the
//! verified credential; the console's `x-catalyrst-admin` label rides in
//! `detail.operator`), domain `telemetry`, target the key acted on.
//!
//! Retention: the `telemetry-purge` job ([`purge`]) runs daily and deletes
//! rows older than `TELEMETRY_RETENTION_DAYS` (90) and `client_error`
//! diagnostics older than 30 days (the old rule) in 5,000-row `ctid`
//! batches, each its own transaction, the cutoff bound from the pass
//! clock, at most `TELEMETRY_PURGE_MAX_BATCHES` per pass.
//!
//! Deviations from the old service, recorded: the current time is a bind
//! parameter in every window, purge and bulk predicate (`timestamp without
//! time zone` in UTC, DATABASE.md); the experiments readout statistics
//! (`ext.two_prop_z`, `two_prop_p`, `beta_post_*`, `p_beats`, functions the
//! old node had installed out of band by the v1 stack's `metabase/functions.sql`)
//! are computed in Rust from the per-variant counts ([`experiments::stats`],
//! the same formulas); `/dash/flags` reads the upstream flags document from
//! the local `explore_api.feature_flags` mirror (the old `FLAGS_URL` hop to
//! the explorer-api) unless `TELEMETRY_FLAGS_URL` names a public source,
//! fetched through `http::Outbound`; the "observed" flags rollup is bounded
//! to 30 days; the audit actor is the verified credential rather than the
//! client-supplied header; `admin_settings`, `project_quota`, `issue_merge`
//! and `admin_audit` are `ingest_settings`, `project_quotas`,
//! `issue_merges` and `ops.admin_audit`. Not carried: the per-service
//! `/healthz`, `TELEMETRY_PG_CONNECTION_STRING` (the ONE pool; the SQL
//! console alone has a pool of its own, a separate security role:
//! `TELEMETRY_SQL_DATABASE_URL`, [`sql`]), the `pg_trgm` search index
//! (dead under the OR'd predicate).
//!
//! Mount: `.nest("/telemetry", ops::telemetry::router(state))`, with
//! `TELEMETRY_BASE_PATH=/telemetry` so the pages, the login cookie and the
//! console's fetches carry the prefix (the old gateway stripped it).

pub mod admin;
pub mod contract;
pub mod dashboard;
pub mod experiments;
pub mod fonts;
pub mod groups;
pub mod ingest;
pub mod login;
pub mod purge;
pub mod segment;
pub mod sentry;
pub mod sql;
pub mod ssr;

#[cfg(test)]
pub(crate) mod tests;

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use anyhow::Context;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{Request, State as AxumState};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use flate2::read::{GzDecoder, ZlibDecoder};
use parking_lot::Mutex;
use serde_json::Value;
use sqlx::PgPool;

use crate::builder::writes::{Gate, Refusal, guard_with};
use crate::http::Outbound;
use crate::jobs::{Jobs, PeriodicCfg};
use crate::shared::cache::TtlMap;

pub const ADMIN_TOKEN_ENV: &str = "CATALYRST_TELEMETRY_ADMIN_TOKEN";
pub const DEFAULT_BASE_PATH: &str = "/telemetry";
pub const DEFAULT_RETENTION_DAYS: u32 = 90;
pub const CLIENT_ERROR_RETENTION_DAYS: u32 = 30;
pub const DEFAULT_PURGE_MAX_BATCHES: u32 = 100;
/// `TELEMETRY_DEFAULT_DAILY_LIMIT`: rows per UTC day for a project with no
/// `project_quotas` row (and no `*` row). 50,000 is five times the whole
/// old node's daily traffic; a project that needs more gets its own row.
pub const DEFAULT_DAILY_LIMIT: i64 = 50_000;
/// `TELEMETRY_MAX_ROWS_PER_MINUTE`: the process-wide admission ceiling.
/// 300/min = 432,000 rows a day, under the purge job's daily budget
/// (`TELEMETRY_PURGE_MAX_BATCHES` 100 x 5,000 = 500,000), so a flood held
/// at the ceiling for a whole day is still deleted by the next pass; the
/// old node averaged 13 rows/min.
pub const DEFAULT_MAX_ROWS_PER_MINUTE: u32 = 300;
/// `TELEMETRY_MAX_BYTES_PER_MINUTE`: decoded request bytes admitted per
/// minute (4 MiB: 300 rows of the 10-30 KB fat tail, 5.6 GiB a day at most).
pub const DEFAULT_MAX_BYTES_PER_MINUTE: u64 = 4 * 1024 * 1024;
/// Envelope items / batch events stored from one request; Segment's own
/// client batches 16, an explorer session start emits tens of items.
pub const MAX_ITEMS_PER_REQUEST: usize = 100;
/// The decoded body cap: the route body cap (axum's 2 MiB default), so a
/// gzip body cannot inflate past what a plain one may send.
pub const MAX_DECODED_BYTES: u64 = 2 * 1024 * 1024;
/// The `raw` kept of an envelope with no parseable item (the header line
/// is kept whole beside it).
pub const MAX_RAW_ENVELOPE_BYTES: usize = 4096;
pub const PURGE_JOB_NAME: &str = "telemetry-purge";
pub const WRITER_JOB_NAME: &str = "telemetry-ingest-writer";
pub const FLAGS_APP: &str = "explorer";

/// The raw values `main.rs` reads from `Config` (the old env names).
#[derive(Debug, Clone, Default)]
pub struct RawSettings {
    /// `TELEMETRY_INGEST_ENABLED` (false): the two sinks accept events.
    pub ingest_enabled: bool,
    /// `OPS_WRITES_ENABLED` (false): the console's mutations are accepted.
    pub writes_enabled: bool,
    /// `CATALYRST_TELEMETRY_ADMIN_TOKEN` (or `_FILE`): the console bearer; unset fails closed.
    pub admin_token: Option<String>,
    /// `TELEMETRY_CONTRACT_PATH`: telemetry-contract.json; unset/unreadable = accept everything.
    pub contract_path: Option<String>,
    /// `TELEMETRY_CONTRACT_PROJECTS`: comma-separated write keys the contract validates (unset = all).
    pub contract_projects: Option<String>,
    /// `TELEMETRY_FLAGS_URL`: a public flags document for `/dash/flags`; unset reads the local mirror.
    pub flags_url: Option<String>,
    /// `TELEMETRY_BASE_PATH` (`/telemetry`): the prefix the router is nested under.
    pub base_path: String,
    /// `TELEMETRY_RETENTION_DAYS` (90).
    pub retention_days: u32,
    /// `TELEMETRY_PURGE_MAX_BATCHES` (100): 5,000-row batches per purge pass.
    pub purge_max_batches: u32,
    /// `TELEMETRY_DEFAULT_DAILY_LIMIT` (50,000): the daily quota of a project without a row; 0 refuses unknown projects.
    pub default_daily_limit: i64,
    /// `TELEMETRY_MAX_ROWS_PER_MINUTE` (300): the process-wide rows ceiling.
    pub max_rows_per_minute: u32,
    /// `TELEMETRY_MAX_BYTES_PER_MINUTE` (4 MiB): the process-wide decoded-bytes ceiling.
    pub max_bytes_per_minute: u64,
    /// `TELEMETRY_SQL_DATABASE_URL` (or `_FILE`): the reader login `/dash/sql` runs on; unset answers 503.
    pub sql_database_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Settings {
    pub ingest_enabled: bool,
    pub writes_enabled: bool,
    pub admin_token: Option<String>,
    pub contract_path: Option<String>,
    pub contract_projects: Option<HashSet<String>>,
    /// Checked to be a public http(s) endpoint that is not this node.
    pub flags_url: Option<String>,
    /// Normalised: empty, or `/prefix` without a trailing slash.
    pub base_path: String,
    pub retention_days: u32,
    pub purge_max_batches: u32,
    pub bounds: Bounds,
    /// The telemetry reader's login ([`sql::ReaderDsn`], never printed).
    pub sql_reader: Option<sql::ReaderDsn>,
}

/// The admission ceilings of [`IngestControl`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    pub default_daily_limit: i64,
    pub max_rows_per_minute: u32,
    pub max_bytes_per_minute: u64,
}

impl Default for Bounds {
    fn default() -> Self {
        Self {
            default_daily_limit: DEFAULT_DAILY_LIMIT,
            max_rows_per_minute: DEFAULT_MAX_ROWS_PER_MINUTE,
            max_bytes_per_minute: DEFAULT_MAX_BYTES_PER_MINUTE,
        }
    }
}

impl Bounds {
    /// No ceiling but the daily quota: the old service's admission, for
    /// tests that count what one request stores.
    pub fn unlimited() -> Self {
        Self {
            default_daily_limit: i64::MAX,
            max_rows_per_minute: u32::MAX,
            max_bytes_per_minute: u64::MAX,
        }
    }
}

impl Settings {
    pub fn resolve(raw: RawSettings, self_hosts: &[String]) -> anyhow::Result<Self> {
        let flags_url = raw
            .flags_url
            .map(|u| u.trim().to_string())
            .filter(|u| !u.is_empty());
        if let Some(url) = &flags_url {
            crate::shop::market::check_upstream("TELEMETRY_FLAGS_URL", url, self_hosts)?;
        }
        let sql_reader = raw
            .sql_database_url
            .as_deref()
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .map(sql::ReaderDsn::parse)
            .transpose()?;
        Ok(Self {
            ingest_enabled: raw.ingest_enabled,
            writes_enabled: raw.writes_enabled,
            admin_token: raw
                .admin_token
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty()),
            contract_path: raw
                .contract_path
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty()),
            contract_projects: contract::projects_from(raw.contract_projects.as_deref()),
            flags_url,
            base_path: ssr::normalize_base(&raw.base_path),
            retention_days: raw.retention_days.max(1),
            purge_max_batches: raw.purge_max_batches.max(1),
            bounds: Bounds {
                default_daily_limit: raw.default_daily_limit.max(0),
                max_rows_per_minute: raw.max_rows_per_minute.max(1),
                max_bytes_per_minute: raw.max_bytes_per_minute.max(1),
            },
            sql_reader,
        })
    }

    pub fn for_tests() -> Self {
        Self {
            ingest_enabled: true,
            writes_enabled: true,
            admin_token: None,
            contract_path: None,
            contract_projects: None,
            flags_url: None,
            base_path: String::new(),
            retention_days: DEFAULT_RETENTION_DAYS,
            purge_max_batches: DEFAULT_PURGE_MAX_BATCHES,
            bounds: Bounds::unlimited(),
            sql_reader: None,
        }
    }
}

/// A token bucket over a `Mutex`: `capacity` tokens, refilled over a minute.
/// A capacity of `u32::MAX` or more is "no ceiling" ([`Bounds::unlimited`]).
struct Bucket {
    capacity: f64,
    refill_per_sec: f64,
    unlimited: bool,
    state: Mutex<(f64, std::time::Instant)>,
}

impl Bucket {
    fn per_minute(capacity: u64) -> Self {
        let unlimited = capacity >= u64::from(u32::MAX);
        let capacity = capacity as f64;
        Self {
            capacity,
            refill_per_sec: capacity / 60.0,
            unlimited,
            state: Mutex::new((capacity, std::time::Instant::now())),
        }
    }

    fn refill(&self) -> parking_lot::MutexGuard<'_, (f64, std::time::Instant)> {
        let mut state = self.state.lock();
        let now = std::time::Instant::now();
        let elapsed = now.saturating_duration_since(state.1).as_secs_f64();
        state.0 = (state.0 + elapsed * self.refill_per_sec).min(self.capacity);
        state.1 = now;
        state
    }

    /// Takes up to `wanted` tokens; returns how many it got (0 when empty).
    fn take_up_to(&self, wanted: u64) -> u64 {
        if self.unlimited {
            return wanted;
        }
        let mut state = self.refill();
        let granted = (state.0.floor() as u64).min(wanted);
        state.0 -= granted as f64;
        granted
    }

    /// All of `wanted` or nothing.
    fn take_all(&self, wanted: u64) -> bool {
        if self.unlimited {
            return true;
        }
        let mut state = self.refill();
        if state.0 >= wanted as f64 {
            state.0 -= wanted as f64;
            true
        } else {
            false
        }
    }
}

/// The admission: the runtime kill switch, the per-project daily quotas and
/// the process-wide ceilings. Process state seeded from
/// `telemetry.ingest_settings` and `telemetry.project_quotas` at boot and
/// updated by the operator routes; the day's counters reset on a UTC day
/// change and on restart (as the old service did). A project's limit is its
/// own row, else the `*` row, else `TELEMETRY_DEFAULT_DAILY_LIMIT`; the old
/// service's "no row means unlimited" is gone, since the project name is
/// the client's to choose.
pub struct IngestControl {
    pub enabled: AtomicBool,
    pub quotas: RwLock<HashMap<String, i64>>,
    pub bounds: Bounds,
    counter_day: RwLock<String>,
    counters: RwLock<HashMap<String, i64>>,
    rows: Bucket,
    bytes: Bucket,
    /// Rows refused by a quota or the rows ceiling, and requests refused by
    /// the bytes ceiling, since boot.
    pub refused_rows: std::sync::atomic::AtomicU64,
    pub refused_requests: std::sync::atomic::AtomicU64,
}

impl IngestControl {
    pub fn new(bounds: Bounds) -> Self {
        Self {
            enabled: AtomicBool::new(true),
            quotas: RwLock::new(HashMap::new()),
            bounds,
            counter_day: RwLock::new(String::new()),
            counters: RwLock::new(HashMap::new()),
            rows: Bucket::per_minute(u64::from(bounds.max_rows_per_minute)),
            bytes: Bucket::per_minute(bounds.max_bytes_per_minute),
            refused_rows: std::sync::atomic::AtomicU64::new(0),
            refused_requests: std::sync::atomic::AtomicU64::new(0),
        }
    }

    pub fn admit(&self, project: &str) -> bool {
        self.admit_n(project, 1) > 0
    }

    /// The daily limit of `project`: its row, the `*` row, the default.
    pub fn daily_limit(&self, project: &str) -> i64 {
        let q = self.quotas.read().unwrap();
        q.get(project)
            .or_else(|| q.get("*"))
            .copied()
            .unwrap_or(self.bounds.default_daily_limit)
    }

    /// How many of `count` events of `project` may be stored now: what the
    /// project's daily quota leaves, then what the process-wide rows
    /// ceiling leaves; the refused remainder is counted.
    pub fn admit_n(&self, project: &str, count: usize) -> usize {
        if count == 0 || !self.enabled.load(Ordering::Relaxed) {
            return 0;
        }
        let limit = self.daily_limit(project);
        let today = today_utc();
        {
            let day = self.counter_day.read().unwrap();
            if *day != today {
                drop(day);
                let mut day = self.counter_day.write().unwrap();
                if *day != today {
                    *day = today.clone();
                    self.counters.write().unwrap().clear();
                }
            }
        }
        let requested = i64::try_from(count).unwrap_or(i64::MAX);
        let by_quota = {
            let mut counters = self.counters.write().unwrap();
            let used = counters.entry(project.to_string()).or_insert(0);
            let remaining = limit.saturating_sub(*used).max(0);
            let granted = remaining.min(requested);
            *used += granted;
            granted as u64
        };
        let granted = self.rows.take_up_to(by_quota);
        let refused = count as u64 - granted;
        if refused > 0 {
            self.refused_rows.fetch_add(refused, Ordering::Relaxed);
            let reason = if by_quota < count as u64 {
                "quota"
            } else {
                "rows_ceiling"
            };
            metrics::counter!("telemetry_ingest_dropped_total", "reason" => reason)
                .increment(refused);
        }
        granted as usize
    }

    /// Whether a request of `decoded_bytes` fits the bytes ceiling; a
    /// refused request stores nothing and is counted.
    pub fn admit_bytes(&self, decoded_bytes: usize) -> bool {
        if !self.enabled.load(Ordering::Relaxed) {
            return false;
        }
        if self.bytes.take_all(decoded_bytes as u64) {
            return true;
        }
        self.refused_requests.fetch_add(1, Ordering::Relaxed);
        metrics::counter!("telemetry_ingest_dropped_total", "reason" => "bytes_ceiling")
            .increment(1);
        false
    }
}

/// Initialize every telemetry series before the first ingestion request.
pub(crate) fn register_metrics() {
    metrics::counter!("telemetry_rows_refused_total").increment(0);
    for reason in ["queue_full", "quota", "rows_ceiling", "bytes_ceiling"] {
        metrics::counter!("telemetry_ingest_dropped_total", "reason" => reason).increment(0);
    }
}

fn today_utc() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

/// One memoised value with single-flight refresh (the old `TtlCell`): a
/// `fresh` call reloads unconditionally and refreshes the slot for
/// everyone; `get_or_stale` serves the last good value when a reload fails.
pub struct Memo<V>
where
    V: Clone + Send + Sync + 'static,
{
    slot: TtlMap<u8, V>,
    last_good: Mutex<Option<V>>,
}

impl<V> Memo<V>
where
    V: Clone + Send + Sync + 'static,
{
    pub fn new(ttl: Duration) -> Self {
        Self {
            slot: TtlMap::new(ttl),
            last_good: Mutex::new(None),
        }
    }

    pub async fn get_or_refresh<F, Fut, E>(&self, fresh: bool, fetch: F) -> Result<V, E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<V, E>>,
    {
        if fresh {
            self.slot.clear();
        }
        let result = self.slot.get_or_fetch(0, fetch).await;
        if let Ok(value) = &result {
            *self.last_good.lock() = Some(value.clone());
        }
        result
    }

    pub async fn get_or_stale<F, Fut, E>(&self, fetch: F) -> Option<V>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<V, E>>,
    {
        match self.get_or_refresh(false, fetch).await {
            Ok(value) => Some(value),
            Err(_) => self.last_good.lock().clone(),
        }
    }
}

pub struct State {
    pub pool: PgPool,
    /// The telemetry reader's own pool, `POST /dash/sql` only ([`sql`]);
    /// `None` without `TELEMETRY_SQL_DATABASE_URL`.
    pub sql_pool: Option<PgPool>,
    pub http: Outbound,
    pub settings: Settings,
    pub ingest: IngestControl,
    pub writer: ingest::WriteBuffer,
    /// The drain loop, taken once by [`State::spawn_jobs`] (or a test).
    writer_task: Mutex<Option<ingest::Writer>>,
    /// `None` = validation disabled (fail-open): every event is stored unflagged.
    pub contract: Option<Arc<contract::Contract>>,
    pub experiments_cache: Memo<Value>,
    pub readout_cache: TtlMap<experiments::ReadoutKey, Vec<experiments::ReadoutRow>>,
    pub series_cache: TtlMap<(String, String), experiments::SeriesBundle>,
    pub flags_config: Memo<Value>,
    pub flags_observed: Memo<Vec<(Option<String>, i64)>>,
}

pub type SharedState = Arc<State>;

impl crate::shared::telemetry::Ingest for State {
    fn ingest_enabled(&self) -> bool {
        self.settings.ingest_enabled
    }

    fn ingest_disabled_response(&self) -> Response {
        ingest_disabled_response()
    }

    fn admit_bytes(&self, decoded_bytes: usize) -> bool {
        self.ingest.admit_bytes(decoded_bytes)
    }

    fn admit_n(&self, project: &str, count: usize) -> usize {
        self.ingest.admit_n(project, count)
    }

    fn push_all(&self, rows: Vec<ingest::Row>) -> usize {
        self.writer.push_all(rows)
    }
}

impl State {
    /// Loads the operator toggle and the quotas, starts the batch writer,
    /// opens the reader's pool lazily.
    pub async fn new(pool: PgPool, http: Outbound, settings: Settings) -> anyhow::Result<Self> {
        let ingest = IngestControl::new(settings.bounds);
        let enabled: Option<bool> =
            sqlx::query_scalar("SELECT enabled FROM telemetry.ingest_settings WHERE id = 1")
                .fetch_optional(&pool)
                .await
                .context("reading telemetry.ingest_settings")?;
        ingest
            .enabled
            .store(enabled.unwrap_or(true), Ordering::Relaxed);
        let quotas: Vec<(String, i64)> =
            sqlx::query_as("SELECT project, daily_limit FROM telemetry.project_quotas")
                .fetch_all(&pool)
                .await
                .context("reading telemetry.project_quotas")?;
        ingest.quotas.write().unwrap().extend(quotas);
        let contract = contract::load(settings.contract_path.as_deref());
        let (writer, writer_task) = ingest::WriteBuffer::new(pool.clone());
        let sql_pool = settings.sql_reader.as_ref().map(sql::ReaderDsn::pool);
        Ok(Self {
            sql_pool,
            writer,
            writer_task: Mutex::new(Some(writer_task)),
            pool,
            http,
            ingest,
            contract,
            experiments_cache: Memo::new(experiments::LIST_TTL),
            readout_cache: TtlMap::bounded(experiments::LIST_TTL, 256),
            series_cache: TtlMap::bounded(experiments::LIST_TTL, 256),
            flags_config: Memo::new(dashboard::flags::CONFIG_TTL),
            flags_observed: Memo::new(dashboard::flags::OBSERVED_TTL),
            settings,
        })
    }

    /// Attaches `telemetry-purge` (daily, a 10 min first-fire jitter, the
    /// first pass after one interval so a restarted fleet does not purge in
    /// lockstep at boot) and `telemetry-ingest-writer`, the batch writer's
    /// drain loop itself (its progress line is the running written/refused
    /// count; it drains the queue at shutdown and reports a failure if the
    /// loop ever ends early, after which every push is a counted drop).
    pub fn spawn_jobs(self: &Arc<Self>, jobs: &Jobs) {
        let state = self.clone();
        jobs.spawn_periodic_leased(
            PURGE_JOB_NAME,
            PeriodicCfg::every(Duration::from_secs(24 * 60 * 60))
                .with_jitter(Duration::from_secs(600))
                .after_first_interval()
                .with_error_backoff(Duration::from_secs(6 * 60 * 60)),
            move |ctx| {
                let state = state.clone();
                async move { purge::run_pass(&state, &ctx).await }
            },
        );
        let task = self.writer_task.lock().take();
        jobs.spawn(WRITER_JOB_NAME, move |ctx| async move {
            let Some(task) = task else {
                anyhow::bail!("the telemetry writer was already started");
            };
            task.run(ctx.cancel.clone(), Some(ctx)).await
        });
    }

    /// Runs the writer's drain loop detached from any job (tests without a
    /// `Jobs`); a second call is a no-op.
    pub fn start_writer_detached(&self) {
        if let Some(task) = self.writer_task.lock().take() {
            tokio::spawn(task.run(tokio_util::sync::CancellationToken::new(), None));
        }
    }

    pub fn sources_label(&self) -> String {
        format!(
            "ingest={} writes={} admin={} contract={} flags={} default_daily_limit={} rows_per_minute={} bytes_per_minute={} sql_reader={}",
            self.settings.ingest_enabled,
            self.settings.writes_enabled,
            self.settings.admin_token.is_some(),
            self.contract.as_ref().map(|c| c.events.len()).unwrap_or(0),
            self.settings
                .flags_url
                .as_deref()
                .unwrap_or("explore_api.feature_flags"),
            self.settings.bounds.default_daily_limit,
            self.settings.bounds.max_rows_per_minute,
            self.settings.bounds.max_bytes_per_minute,
            if self.sql_pool.is_some() {
                "configured"
            } else {
                "unset"
            },
        )
    }

    /// The admission and writer counters since boot (the `/dash/admin/ingest`
    /// read and the live checks).
    pub fn ingest_counters(&self) -> Value {
        serde_json::json!({
            "enabled": self.ingest.enabled.load(Ordering::Relaxed),
            "refused_rows": self.ingest.refused_rows.load(Ordering::Relaxed),
            "refused_requests": self.ingest.refused_requests.load(Ordering::Relaxed),
            "dropped_queue_full": self.writer.dropped.load(Ordering::Relaxed),
            "refused_by_database": self.writer.refused.load(Ordering::Relaxed),
            "written": self.writer.written.load(Ordering::Relaxed),
            "bounds": {
                "default_daily_limit": self.settings.bounds.default_daily_limit,
                "max_rows_per_minute": self.settings.bounds.max_rows_per_minute,
                "max_bytes_per_minute": self.settings.bounds.max_bytes_per_minute,
                "max_items_per_request": MAX_ITEMS_PER_REQUEST,
                "max_decoded_bytes": MAX_DECODED_BYTES,
            },
        })
    }
}

/// Logs at `error!` with a per-caller context label and returns a generic
/// message, so a query's internal detail never reaches the client.
pub(crate) fn db_err(context: &str, e: sqlx::Error) -> (StatusCode, String) {
    tracing::error!(error = %e, "{} db error", context);
    (StatusCode::INTERNAL_SERVER_ERROR, "database error".into())
}

/// Inflates a gzip/deflate request body, capped at [`MAX_DECODED_BYTES`] (a
/// bomb yields a truncated document that fails to parse, not memory, and
/// no row larger than a plain request could carry).
pub fn decode_body(headers: &HeaderMap, body: Bytes) -> Vec<u8> {
    let encoding = headers
        .get(header::CONTENT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    match encoding {
        "gzip" => {
            let mut out = Vec::new();
            let _ = GzDecoder::new(&body[..])
                .take(MAX_DECODED_BYTES)
                .read_to_end(&mut out);
            out
        }
        "deflate" => {
            let mut out = Vec::new();
            let _ = ZlibDecoder::new(&body[..])
                .take(MAX_DECODED_BYTES)
                .read_to_end(&mut out);
            out
        }
        _ => body.to_vec(),
    }
}

/// The `TELEMETRY_INGEST_ENABLED=false` answer of the sinks.
pub fn ingest_disabled_response() -> Response {
    let body = serde_json::json!({ "ok": false, "error": "Service Unavailable", "message": super::INGEST_DISABLED_MESSAGE });
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::RETRY_AFTER, "3600"),
        ],
        body.to_string(),
    )
        .into_response()
}

/// The `OPS_WRITES_ENABLED=false` answer of the console's mutations.
pub fn writes_disabled_response() -> Response {
    writes_refused_response(&Refusal::Off)
}

/// The console's 503 for `refusal`: the switch's own message while it is
/// off, the restore stamp that holds it and its age while it is on.
pub fn writes_refused_response(refusal: &Refusal) -> Response {
    let message = refusal.message("operator-plane", super::WRITES_DISABLED_MESSAGE);
    let body =
        serde_json::json!({ "ok": false, "error": "Service Unavailable", "message": message });
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::RETRY_AFTER, "3600"),
        ],
        body.to_string(),
    )
        .into_response()
}

/// The console paths whose `POST` is a read or a session mint, open while
/// `OPS_WRITES_ENABLED` is off.
pub const OPEN_WHILE_DISABLED: &[&str] = &["/login", "/dash/sql"];

async fn ingest_gate(AxumState(enabled): AxumState<bool>, req: Request, next: Next) -> Response {
    if !enabled {
        return ingest_disabled_response();
    }
    next.run(req).await
}

async fn require_admin(
    AxumState(state): AxumState<SharedState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    if let Err(rejection) = admin::authorize(&state, &headers) {
        return rejection.into_response();
    }
    next.run(request).await
}

async fn require_admin_read(
    AxumState(state): AxumState<SharedState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    if let Err(rejection) = admin::authorize_read(&state, &headers) {
        let wants_html = request.method() == axum::http::Method::GET
            && headers
                .get("accept")
                .and_then(|v| v.to_str().ok())
                .is_some_and(|a| a.contains("text/html"));
        if wants_html {
            return Redirect::to(&format!("{}/login", state.settings.base_path)).into_response();
        }
        return rejection.into_response();
    }
    next.run(request).await
}

fn gated_reads(state: SharedState) -> Router<SharedState> {
    Router::new()
        .route("/dash/events", get(dashboard::events))
        .route("/dash/metrics", get(dashboard::metrics))
        .route("/dash/health", get(dashboard::health))
        .route("/dash/funnel", get(dashboard::funnel))
        .route("/dash/breakdown", get(dashboard::breakdown))
        .route("/dash/event/{id}", get(dashboard::event_detail))
        .route("/dash/stats", get(dashboard::stats))
        .route("/dash/sql", post(dashboard::sql_query))
        .route("/dash/story/{id}", get(dashboard::story))
        .route("/dash/session/{id}", get(dashboard::session))
        .route_layer(middleware::from_fn_with_state(state, require_admin_read))
}

fn gated_ssr(state: SharedState) -> Router<SharedState> {
    Router::new()
        .route("/", get(ssr::page))
        .route("/events", get(ssr::page))
        .route("/issues/{fp}", get(ssr::page))
        .route("/metrics", get(ssr::page))
        .route("/metrics/stream", get(ssr::page))
        .route("/metrics/funnel", get(ssr::page))
        .route("/metrics/breakdown", get(ssr::page))
        .route("/health", get(ssr::page))
        .route("/sql", get(ssr::page))
        .route("/session/{id}", get(ssr::page))
        .route_layer(middleware::from_fn_with_state(state, require_admin_read))
}

fn gated_writes(state: SharedState) -> Router<SharedState> {
    Router::new()
        .route("/dash/issue/state", post(dashboard::set_issue_state))
        .route(
            "/dash/experiment",
            get(dashboard::experiments_get).post(dashboard::experiment_set),
        )
        .route("/dash/flag", post(dashboard::flag_set))
        .route("/dash/groups", get(groups::list).post(groups::set))
        .route("/dash/group/target", post(groups::set_target))
        .route("/dash/group/resolve", get(groups::resolve))
        .route("/dash/area", post(groups::set_area))
        .route_layer(middleware::from_fn_with_state(state, require_admin))
}

/// The two sinks: Sentry envelopes/store events and the Segment HTTP API,
/// behind `TELEMETRY_INGEST_ENABLED`.
fn sinks(state: SharedState) -> Router {
    Router::new()
        .route("/api/{project}/envelope/", post(sentry::envelope))
        .route("/api/{project}/envelope", post(sentry::envelope))
        .route("/api/{project}/store/", post(sentry::store))
        .route("/api/{project}/store", post(sentry::store))
        .route("/v1/batch", post(segment::batch))
        .route("/v1/import", post(segment::batch))
        .route("/v1/track", post(segment::single))
        .route("/v1/identify", post(segment::single))
        .route("/v1/page", post(segment::single))
        .route("/v1/screen", post(segment::single))
        .route("/v1/group", post(segment::single))
        .route("/v1/alias", post(segment::single))
        .route_layer(middleware::from_fn_with_state(
            state.settings.ingest_enabled,
            ingest_gate,
        ))
        .with_state(state)
}

/// The console: pages, fonts, login, the JSON reads and the operator
/// routes, its mutations behind `OPS_WRITES_ENABLED`.
fn console(state: SharedState) -> Router {
    let router = Router::new()
        .route("/flags", get(ssr::page))
        .route("/experiments", get(ssr::page))
        .route("/login", get(login::page).post(login::submit))
        .route("/fonts/{name}", get(fonts::serve))
        .route("/dash/flags", get(dashboard::flags))
        .merge(gated_ssr(state.clone()))
        .merge(gated_reads(state.clone()))
        .merge(gated_writes(state.clone()))
        .route("/dash/experiments", get(experiments::list))
        .route("/dash/experiments/readout", get(experiments::readout))
        .route("/dash/experiments/timeseries", get(experiments::timeseries))
        .route("/dash/experiments/rates", get(experiments::rates))
        .route("/dash/admin/purge", post(admin::purge))
        .route(
            "/dash/admin/ingest",
            get(admin::ingest_status).post(admin::ingest_toggle),
        )
        .route("/dash/admin/quota", post(admin::quota))
        .route("/dash/admin/bulk-delete", post(admin::bulk_delete))
        .route("/dash/admin/export", post(admin::export))
        .route("/dash/admin/audit", get(admin::audit_list))
        .route("/dash/admin/regroup", post(admin::regroup))
        .route("/dash/admin/release", post(admin::release))
        .with_state(state.clone());
    guard_with(
        router,
        Gate {
            enabled: state.settings.writes_enabled,
            open_while_disabled: OPEN_WHILE_DISABLED,
            refuse: writes_refused_response,
            blobs: false,
        },
    )
}

/// The old `api_router` under one nest: `.nest("/telemetry", router(state))`.
pub fn router(state: SharedState) -> Router {
    Router::new()
        .merge(sinks(state.clone()))
        .merge(console(state))
        .layer(middleware::from_fn(crate::shared::cors::cors_middleware))
}
