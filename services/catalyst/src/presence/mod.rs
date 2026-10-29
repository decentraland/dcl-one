//! Presence archive: the user-count history (one snapshot per collector
//! pass of archipelago peers, islands and hot scenes, per-scene occupancy and
//! per-world membership; `schemas/34_presence.sql`). Distinct from live
//! presence (comms' voice presence, the worlds presence map), which follows
//! the transport and is never persisted.
//!
//! Where each route reads: every route is
//! **local**, `presence.*`, filled by the `presence-collector` job
//! (`jobs::spawn_periodic`, [`Settings::snapshot_interval`], 300 s) over
//! module calls ([`sources::ModuleSources`]: archipelago's peer directory,
//! island feed and scene resolver, comms' `list_participants`, the worlds
//! presence map) where the old service polled `ARCHIPELAGO_URL`,
//! `COMMS_URL` and `WORLDS_SERVER_URL` over HTTP. A pass is one transaction
//! ([`collector::store_snapshot`]) and is local authority, so the job runs
//! only with `DATA_WRITES_ENABLED=true` (`crate::shared::writes::DataWrites`,
//! the social switch's twin) unless `PRESENCE_COLLECTOR_ENABLED` decides it
//! ([`Settings::switch`] names the deciding key); the old service had no HTTP write, so nothing
//! here needs the gate middleware. A read-only node records nothing and its
//! routes answer what the tables hold, `taken_at` saying how old; while the
//! archive is empty (no pass ever recorded here, or not yet restored) every
//! route answers [`unavailable`], the data plane's 503 envelope naming the
//! collector and its switch, never an empty list presented as a quiet realm.
//!
//! Routes (nested under `/presence`, the prefix the sites call): `/current`,
//! `/current/scenes`, `/current/worlds`, `/scenes/history?pointer=[&limit=]`,
//! `/worlds/history?world=[&limit=]`, the old crate's paths unchanged. Not
//! carried: the per-service `/health` (like every other old service's).
//! Retention is new and off by default ([`Settings::retention_days`],
//! `PRESENCE_RETENTION_DAYS`): the old archive grew without bound; set, the
//! pass deletes at most [`RETENTION_SWEEP_LIMIT`] expired snapshots.

pub mod collector;
pub mod model;
pub mod queries;
pub mod sources;

#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::jobs::{Jobs, PeriodicCfg};
use crate::shared::{cors, nul_guard};

pub const SCHEMA: &str = "presence";
pub const DEFAULT_SNAPSHOT_INTERVAL_SECS: u64 = 300;
pub const DEFAULT_GENESIS_REALM: &str = "main";
pub const HISTORY_DEFAULT_LIMIT: i64 = 200;
pub const HISTORY_MAX_LIMIT: i64 = 5000;
/// Rosters read at a time during a pass (the old `PARTICIPANT_FANOUT`).
pub const PARTICIPANT_FANOUT: usize = 8;
/// Expired snapshots one pass deletes at most: a day of passes at the default interval.
pub const RETENTION_SWEEP_LIMIT: i64 = 288;

/// What [`unavailable`] names: the job, and the switch that turns it on
/// (unless `PRESENCE_COLLECTOR_ENABLED` overrides it, [`COLLECTOR_SWITCH`]).
pub const SOURCE_NAME: &str = "presence-collector";
pub const SOURCE_CONFIGURE: &str = "DATA_WRITES_ENABLED";
pub const COLLECTOR_SWITCH: &str = "PRESENCE_COLLECTOR_ENABLED";

#[derive(Clone, Debug)]
pub struct Settings {
    /// Whether this node records passes.
    pub writes_enabled: bool,
    /// The key that decided `writes_enabled`, which every refusal names:
    /// `PRESENCE_COLLECTOR_ENABLED` when set, else `DATA_WRITES_ENABLED`.
    pub switch: &'static str,
    /// `PRESENCE_SNAPSHOT_INTERVAL_SECS` (300).
    pub snapshot_interval: Duration,
    /// `PRESENCE_GENESIS_REALM` (`main`): the realm the Genesis rosters are asked for.
    pub genesis_realm: String,
    /// `PRESENCE_RETENTION_DAYS`: unset keeps every snapshot, as the old archive did.
    pub retention_days: Option<u32>,
}

impl Settings {
    pub fn new(writes_enabled: bool) -> Self {
        Self {
            writes_enabled,
            switch: SOURCE_CONFIGURE,
            snapshot_interval: Duration::from_secs(DEFAULT_SNAPSHOT_INTERVAL_SECS),
            genesis_realm: DEFAULT_GENESIS_REALM.to_string(),
            retention_days: None,
        }
    }

    /// `PRESENCE_COLLECTOR_ENABLED` when set, else `DATA_WRITES_ENABLED`.
    pub fn from_switches(collector: Option<bool>, data_writes: bool) -> Self {
        match collector {
            Some(on) => Self {
                switch: COLLECTOR_SWITCH,
                ..Self::new(on)
            },
            None => Self::new(data_writes),
        }
    }

    /// The old knobs, with their old defaults for an unset or zero value.
    pub fn with_knobs(
        mut self,
        interval_secs: u64,
        genesis_realm: Option<&str>,
        retention_days: Option<u32>,
    ) -> Self {
        self.snapshot_interval = Duration::from_secs(if interval_secs == 0 {
            DEFAULT_SNAPSHOT_INTERVAL_SECS
        } else {
            interval_secs
        });
        if let Some(realm) = genesis_realm.map(str::trim).filter(|r| !r.is_empty()) {
            self.genesis_realm = realm.to_string();
        }
        self.retention_days = retention_days.filter(|d| *d > 0);
        self
    }
}

pub struct PresenceState {
    pub pool: PgPool,
    pub settings: Settings,
    pub sources: Arc<dyn sources::Sources>,
    pub(crate) queries: queries::Queries,
    /// The day `daily_stats` was last rolled up for in this process.
    pub(crate) last_aggregated: parking_lot::Mutex<Option<chrono::NaiveDate>>,
}

pub type SharedState = Arc<PresenceState>;

impl PresenceState {
    pub fn new(pool: PgPool, settings: Settings, sources: Arc<dyn sources::Sources>) -> Self {
        Self {
            queries: queries::Queries::new(pool.clone()),
            pool,
            settings,
            sources,
            last_aggregated: parking_lot::Mutex::new(None),
        }
    }

    /// Attaches the `presence-collector` job when this node records passes:
    /// one pass per interval, a 30 s first-fire jitter, failures doubling the
    /// wait up to an hour.
    pub fn spawn_jobs(self: &Arc<Self>, jobs: &Jobs) {
        if !self.settings.writes_enabled {
            tracing::info!(
                "{}=false: the presence collector is off; \
                 the presence routes answer the archive as it is (503 while empty)",
                self.settings.switch
            );
            return;
        }
        let state = self.clone();
        jobs.spawn_periodic_leased(
            "presence-collector",
            PeriodicCfg::every(self.settings.snapshot_interval)
                .with_jitter(Duration::from_secs(30))
                .with_error_backoff(Duration::from_secs(3600)),
            move |_ctx| {
                let state = state.clone();
                async move {
                    collector::run_pass(&state, chrono::Utc::now())
                        .await
                        .map(|_| ())
                }
            },
        );
    }
}

/// UTC as the tables hold it; serialized as the old `DateTime<Utc>` wire form.
mod utc_wire {
    use chrono::NaiveDateTime;

    pub(super) fn serialize<S: serde::Serializer>(
        t: &NaiveDateTime,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        s.serialize_str(
            &t.and_utc()
                .to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true),
        )
    }
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "presence/"))]
pub struct CurrentSnapshot {
    #[sqlx(rename = "id")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub snapshot_id: i64,
    #[serde(with = "utc_wire")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub taken_at: NaiveDateTime,
    pub peers_count: i32,
    pub islands_count: i32,
    pub hot_scenes_count: i32,
    pub scenes_polled: i32,
    pub scene_users_total: i32,
    pub worlds_polled: i32,
    pub active_worlds: i32,
    pub world_users_total: i32,
    pub worlds_live_total: Option<i32>,
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "presence/"))]
pub struct SceneOccupancyRow {
    #[serde(with = "utc_wire")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub taken_at: NaiveDateTime,
    pub pointer: String,
    pub scene_name: Option<String>,
    pub realm: String,
    pub count: i32,
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "presence/"))]
pub struct WorldHeadcountRow {
    #[serde(with = "utc_wire")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub taken_at: NaiveDateTime,
    pub world_name: String,
    pub count: i32,
    pub live_users: Option<i32>,
}

/// The old `clamp_limit`: absent or non-positive is the default, above the cap is the cap.
pub fn clamp_history_limit(limit: Option<i64>) -> i64 {
    match limit {
        Some(l) if l > 0 => l.min(HISTORY_MAX_LIMIT),
        _ => HISTORY_DEFAULT_LIMIT,
    }
}

/// The data plane's unavailable envelope (`marketplace::unavailable`'s
/// shape: `error`, `message`, `source`, `configure`, `Retry-After: 60`) for
/// an archive with no snapshot in it; `switch` is [`Settings::switch`].
pub fn unavailable(switch: &str) -> Response {
    let body = json!({
        "error": "Service Unavailable",
        "message": format!("no presence snapshot is recorded on this node: the presence collector runs only with {switch}=true, and a restored archive serves as soon as it holds one"),
        "source": SOURCE_NAME,
        "configure": switch,
    });
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::RETRY_AFTER, "60"),
        ],
        body.to_string(),
    )
        .into_response()
}

/// Why a read did not answer: the empty archive ([`unavailable`]) or a
/// failed statement (500, the old `{"error": "query failed"}`).
enum Refusal {
    /// The archive is empty; the switch that runs the collector.
    EmptyArchive(&'static str),
    Query(sqlx::Error),
}

impl From<sqlx::Error> for Refusal {
    fn from(e: sqlx::Error) -> Self {
        Self::Query(e)
    }
}

impl IntoResponse for Refusal {
    fn into_response(self) -> Response {
        match self {
            Self::EmptyArchive(switch) => unavailable(switch),
            Self::Query(e) => {
                tracing::error!(error = %e, "presence query failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": "query failed" })),
                )
                    .into_response()
            }
        }
    }
}

/// The latest bundle, or the refusal while the archive is empty.
async fn recorded_bundle(state: &PresenceState) -> Result<Arc<queries::CurrentBundle>, Refusal> {
    let bundle = state.queries.current_bundle().await?;
    if bundle.current.is_none() {
        return Err(Refusal::EmptyArchive(state.settings.switch));
    }
    Ok(bundle)
}

async fn current(State(state): State<SharedState>) -> Result<Json<Value>, Refusal> {
    let bundle = recorded_bundle(&state).await?;
    Ok(Json(json!({ "current": bundle.current })))
}

async fn current_scenes(State(state): State<SharedState>) -> Result<Json<Value>, Refusal> {
    let bundle = recorded_bundle(&state).await?;
    Ok(Json(json!({ "scenes": bundle.scenes })))
}

async fn current_worlds(State(state): State<SharedState>) -> Result<Json<Value>, Refusal> {
    let bundle = recorded_bundle(&state).await?;
    Ok(Json(json!({ "worlds": bundle.worlds })))
}

#[derive(Debug, Deserialize)]
pub struct SceneHistoryQuery {
    pub pointer: String,
    pub limit: Option<i64>,
}

async fn scene_history(
    State(state): State<SharedState>,
    Query(q): Query<SceneHistoryQuery>,
) -> Result<Json<Value>, Refusal> {
    recorded_bundle(&state).await?;
    let rows = state
        .queries
        .scene_history(&q.pointer, clamp_history_limit(q.limit))
        .await?;
    Ok(Json(json!({ "pointer": q.pointer, "history": rows })))
}

#[derive(Debug, Deserialize)]
pub struct WorldHistoryQuery {
    pub world: String,
    pub limit: Option<i64>,
}

async fn world_history(
    State(state): State<SharedState>,
    Query(q): Query<WorldHistoryQuery>,
) -> Result<Json<Value>, Refusal> {
    recorded_bundle(&state).await?;
    let rows = state
        .queries
        .world_history(&q.world, clamp_history_limit(q.limit))
        .await?;
    Ok(Json(json!({ "world": q.world, "history": rows })))
}

/// The five read routes at the old crate's paths, with the lambdas' CORS
/// and NUL guard; `main.rs` nests it under `/presence`.
pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/current", get(current))
        .route("/current/scenes", get(current_scenes))
        .route("/current/worlds", get(current_worlds))
        .route("/scenes/history", get(scene_history))
        .route("/worlds/history", get(world_history))
        .layer(axum::middleware::from_fn(nul_guard::nul_guard_middleware))
        .layer(axum::middleware::from_fn(cors::cors_middleware))
        .with_state(state)
}
