//! Recoverable LiveKit room occupancy. Successful complete reads own membership
//! and source time; verified webhooks only invalidate admission authority.
//! Source bracketing describes an observation interval, not a transactional snapshot.

use crate::shared::{cors, nul_guard};
use crate::worlds_envelope::ApiError;
use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use chrono::SecondsFormat;
use parking_lot::Mutex;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;

mod recovery;
pub use recovery::RecoveryFailure;
#[cfg(test)]
mod recovery_tests;

const PUBLIC_TTL: Duration = Duration::from_secs(90);
const ADMISSION_TTL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy)]
pub struct CommsStats {
    pub rooms: i64,
    pub users: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("wallet has memberships in several distinct worlds")]
pub struct AmbiguousWorld;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub(crate) enum Scope {
    Lobby(String),
    Scenes(String),
    Room(String),
}
impl Scope {
    fn includes(&self, room: &RecoveredRoom) -> bool {
        match self {
            Self::Lobby(world) => room.is_world && room.realm == *world && room.scene.is_none(),
            Self::Scenes(world) => room.is_world && room.realm == *world && room.scene.is_some(),
            Self::Room(name) => room.name == *name,
        }
    }
}

#[derive(Clone)]
struct Observation {
    utc: chrono::DateTime<chrono::Utc>,
    mono: Instant,
}
impl Observation {
    fn now() -> Self {
        Self {
            utc: chrono::Utc::now(),
            mono: Instant::now(),
        }
    }
    fn fresh(&self, ttl: Duration) -> bool {
        self.mono.elapsed() < ttl
    }
}

#[derive(Clone)]
struct RecoveredRoom {
    name: String,
    sid: String,
    realm: String,
    is_world: bool,
    scene: Option<String>,
    participants: Vec<crate::shared::livekit::OccupancyParticipant>,
    observed: Observation,
    generation: u64,
}
impl RecoveredRoom {
    fn count(&self) -> i64 {
        self.participants
            .iter()
            .filter(|p| counts_as_player(&p.identity))
            .count() as i64
    }
}
#[derive(Clone, Default)]
struct Complete {
    rooms: Vec<RecoveredRoom>,
    observed: Option<Observation>,
    unknown_generation: u64,
}
struct AdmissionSample {
    count: i64,
    observed: Observation,
    unknown_generation: u64,
    generations: HashMap<String, u64>,
}
#[derive(Default)]
struct StateData {
    complete: Arc<Complete>,
    generations: HashMap<String, u64>,
    unknown_generation: u64,
    admission: HashMap<Scope, AdmissionSample>,
}

pub struct Presence {
    state: Mutex<StateData>,
    // Three background operations plus one reserved admission operation.
    reads: tokio::sync::Semaphore,
    background: tokio::sync::Semaphore,
    admission_flight: tokio::sync::Semaphore,
    waiters: tokio::sync::Semaphore,
    dirty: tokio::sync::Notify,
}
impl Default for Presence {
    fn default() -> Self {
        Self {
            state: Mutex::new(StateData::default()),
            reads: tokio::sync::Semaphore::new(4),
            background: tokio::sync::Semaphore::new(3),
            admission_flight: tokio::sync::Semaphore::new(1),
            waiters: tokio::sync::Semaphore::new(64),
            dirty: tokio::sync::Notify::new(),
        }
    }
}

pub(crate) fn counts_as_player(identity: &str) -> bool {
    identity != "authoritative-server"
}

impl Presence {
    pub fn new() -> Self {
        Self::default()
    }

    /// Bounded invalidation: unseen rooms bump one epoch rather than allocating
    /// attacker-controlled event keys. Known rooms retain independent generations.
    pub fn invalidate_room(&self, room: &str) {
        let mut state = self.state.lock();
        if let Some(generation) = state.generations.get_mut(room) {
            *generation = generation.wrapping_add(1);
        } else {
            state.unknown_generation = state.unknown_generation.wrapping_add(1);
        }
        drop(state);
        self.dirty.notify_one();
    }

    fn complete(&self) -> Arc<Complete> {
        self.state.lock().complete.clone()
    }
    pub fn last_updated(&self) -> String {
        self.complete()
            .observed
            .as_ref()
            .map(|o| o.utc)
            .unwrap_or(chrono::DateTime::UNIX_EPOCH)
            .to_rfc3339_opts(SecondsFormat::Millis, true)
    }
    fn visible(&self) -> Arc<Complete> {
        let complete = self.complete();
        if complete
            .observed
            .as_ref()
            .is_some_and(|o| o.fresh(PUBLIC_TTL))
        {
            complete
        } else {
            Arc::new(Complete::default())
        }
    }
    pub fn get_peer_world(&self, wallet: &str) -> Result<Option<String>, AmbiguousWorld> {
        if !counts_as_player(wallet) {
            return Ok(None);
        }
        // Preserve case-insensitive wallet lookup and all raw memberships.
        // The approved public policy retries rather than choosing a winner.
        let worlds: HashSet<_> = self
            .visible()
            .rooms
            .iter()
            .filter(|r| {
                r.participants.iter().any(|p| {
                    counts_as_player(&p.identity) && p.identity.eq_ignore_ascii_case(wallet)
                })
            })
            .filter(|r| r.is_world)
            .map(|r| r.realm.clone())
            .collect();
        if worlds.len() > 1 {
            return Err(AmbiguousWorld);
        }
        Ok(worlds.into_iter().next())
    }
    pub fn world_counts(&self) -> Vec<(String, i64)> {
        Self::counts_of(&self.complete())
    }
    fn counts_of(complete: &Complete) -> Vec<(String, i64)> {
        if !complete
            .observed
            .as_ref()
            .is_some_and(|o| o.fresh(PUBLIC_TTL))
        {
            return vec![];
        }
        let mut counts = HashMap::new();
        for room in &complete.rooms {
            if room.is_world && room.scene.is_none() && room.count() > 0 {
                *counts.entry(room.realm.clone()).or_insert(0) += room.count();
            }
        }
        let mut out: Vec<_> = counts.into_iter().collect();
        out.sort();
        out
    }
    pub fn comms_stats(&self) -> CommsStats {
        let view = self.visible();
        let mut identities = HashSet::new();
        for room in view.rooms.iter().filter(|r| r.is_world) {
            for p in &room.participants {
                if counts_as_player(&p.identity) {
                    identities.insert(p.identity.as_str());
                }
            }
        }
        CommsStats {
            rooms: view
                .rooms
                .iter()
                .filter(|r| r.is_world && r.count() > 0)
                .count() as i64,
            users: identities.len() as i64,
        }
    }
    /// Public lobby projection only. Admission must use the bounded async API.
    pub fn world_participant_count(&self, world: &str) -> i64 {
        self.world_counts()
            .into_iter()
            .find(|(w, _)| w == &world.to_lowercase())
            .map_or(0, |(_, n)| n)
    }
    fn admission_count(&self, scope: &Scope) -> Option<i64> {
        let state = self.state.lock();
        if let Some(sample) = state.admission.get(scope).filter(|sample| {
            sample.observed.fresh(ADMISSION_TTL)
                && sample.unknown_generation == state.unknown_generation
                && sample
                    .generations
                    .iter()
                    .all(|(name, generation)| state.generations.get(name) == Some(generation))
        }) {
            return Some(sample.count);
        }
        let complete = &state.complete;
        if complete.unknown_generation != state.unknown_generation
            || !complete
                .observed
                .as_ref()
                .is_some_and(|o| o.fresh(ADMISSION_TTL))
        {
            return None;
        }
        let selected: Vec<_> = complete
            .rooms
            .iter()
            .filter(|r| scope.includes(r))
            .collect();
        selected
            .iter()
            .all(|r| state.generations.get(&r.name) == Some(&r.generation))
            .then(|| selected.iter().map(|r| r.count()).sum())
    }

    // Compatibility fixtures for cross-crate route tests. Production webhooks
    // cannot call these helpers and never manufacture a successful source time.
    #[cfg(any(test, feature = "test-support"))]
    pub fn complete_empty_fixture(&self) {
        let mut state = self.state.lock();
        state.complete = Arc::new(Complete {
            rooms: vec![],
            observed: Some(Observation::now()),
            unknown_generation: state.unknown_generation,
        });
        state.generations.clear();
        state.admission.clear();
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn peer_joined(&self, identity: &str, world: &str, room: &str, scene: bool) {
        if !counts_as_player(identity) {
            return;
        }
        let mut state = self.state.lock();
        let view = Arc::make_mut(&mut state.complete);
        let observed = Observation::now();
        let index = match view.rooms.iter().position(|r| r.name == room) {
            Some(index) => index,
            None => {
                view.rooms.push(RecoveredRoom {
                    name: room.into(),
                    sid: format!("fixture:{room}"),
                    realm: world.to_lowercase(),
                    is_world: true,
                    scene: scene.then(|| "fixture-scene".into()),
                    participants: vec![],
                    observed: observed.clone(),
                    generation: 0,
                });
                view.rooms.len() - 1
            }
        };
        let members = &mut view.rooms[index].participants;
        if !members.iter().any(|p| p.identity == identity) {
            members.push(crate::shared::livekit::OccupancyParticipant {
                identity: identity.into(),
                sid: format!("fixture:{identity}"),
            });
        }
        view.observed = Some(observed);
        state.generations.entry(room.into()).or_insert(0);
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn peer_left(&self, identity: &str, room: &str, _: bool) {
        let mut state = self.state.lock();
        for r in &mut Arc::make_mut(&mut state.complete).rooms {
            if r.name == room {
                r.participants.retain(|p| p.identity != identity);
            }
        }
    }
}

/// The two routes behind the worlds routes' NUL guard and CORS. `serve`
/// mounts them at the root and under `/worlds-content-server` (where worlds'
/// core routers served them) and wraps each mount in the worlds writes gate
/// (`WORLDS_WRITES_ENABLED`), as those routers were. Worlds' 30 s
/// `request_timeout` layer is not carried over on purpose: both handlers are
/// synchronous reads of the immutable recovered view, so they cannot time out.
pub fn router(presence: Arc<Presence>) -> Router {
    Router::new()
        .route("/wallet/{wallet}/connected-world", get(connected_world))
        .route("/live-data", get(live_data))
        .layer(axum::middleware::from_fn(nul_guard::nul_guard_middleware))
        .layer(axum::middleware::from_fn(cors::cors_middleware))
        .with_state(presence)
}

/// GET /wallet/{wallet}/connected-world
pub async fn connected_world(
    State(presence): State<Arc<Presence>>,
    Path(wallet): Path<String>,
) -> Result<Json<Value>, ApiError> {
    match presence.get_peer_world(&wallet).map_err(|_| {
        ApiError::service_unavailable("Connected world could not be decided; retry shortly.")
    })? {
        Some(world) => Ok(Json(json!({ "wallet": wallet, "world": world }))),
        None => Err(ApiError::not_found(format!(
            "Wallet {wallet} is not connected to any world"
        ))),
    }
}

/// One world's instantaneous occupancy. `worldName` is camelCase on the wire
/// because this endpoint is wire-compatible with Decentraland's
/// worlds-content-server `/live-data`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct WorldOccupancy {
    pub world_name: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub users: i64,
}

/// Both fields are required and neither is defaulted: a fabricated
/// `totalUsers: 0` is indistinguishable from an empty world.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct LiveDataPayload {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub total_users: i64,
    pub per_world: Vec<WorldOccupancy>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct LiveDataResponse {
    pub data: LiveDataPayload,
    /// RFC3339 with millisecond precision, matching the upstream server.
    pub last_updated: String,
}

/// GET /live-data
pub async fn live_data(State(presence): State<Arc<Presence>>) -> Json<LiveDataResponse> {
    // Capture membership and source clock from the same immutable generation.
    let complete = presence.complete();
    let counts = Presence::counts_of(&complete);
    let last_updated = complete
        .observed
        .as_ref()
        .map(|o| o.utc)
        .unwrap_or(chrono::DateTime::UNIX_EPOCH)
        .to_rfc3339_opts(SecondsFormat::Millis, true);
    let total: i64 = counts.iter().map(|(_, c)| c).sum();
    let per_world = counts
        .into_iter()
        .map(|(world_name, users)| WorldOccupancy { world_name, users })
        .collect();
    Json(LiveDataResponse {
        data: LiveDataPayload {
            total_users: total,
            per_world,
        },
        last_updated,
    })
}
