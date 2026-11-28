//! Recoverable LiveKit room occupancy. Successful complete reads own membership
//! and source time; verified webhooks only invalidate admission authority.
//! Source bracketing describes an observation interval, not a transactional snapshot.

use crate::shared::routing::get;
use crate::shared::{cors, nul_guard};
use crate::worlds_envelope::ApiError;
use axum::extract::{Path, State};
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

pub(crate) const PUBLIC_TTL: Duration = Duration::from_secs(90);
const ADMISSION_TTL: Duration = Duration::from_secs(5);
const AUTHORITATIVE_SERVER_IDENTITY: &str = "authoritative-server";

#[derive(Debug, Clone, Copy)]
pub struct CommsStats {
    pub rooms: i64,
    pub users: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("world presence observation is unavailable or stale")]
pub struct PresenceUnavailable;

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

    /// Whether a room name could hold participants this scope counts: the
    /// exact room, or a world room or upstream-format room name that names the
    /// scope's world. Any other room, refused or canonical, cannot hold them,
    /// since every room this node admits to a world is canonical.
    fn may_hold(&self, room: &str) -> bool {
        let world = match self {
            Self::Room(name) => return room == name,
            Self::Lobby(world) | Self::Scenes(world) => world.as_str(),
        };
        if let Some((owner, _)) = crate::rooms::decode_world_scene_room(room) {
            return owner.key() == world;
        }
        let room = room.to_lowercase();
        [
            crate::rooms::LEGACY_WORLD_SCENE_ROOM_PREFIX,
            crate::rooms::WORLD_ROOM_PREFIX,
            "scene-",
        ]
        .iter()
        .any(|prefix| {
            room.strip_prefix(prefix).is_some_and(|rest| {
                rest.strip_prefix(world)
                    .is_some_and(|tail| tail.is_empty() || tail.starts_with('-'))
            })
        })
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
    legacy: bool,
    scene: Option<String>,
    participants: Vec<crate::shared::livekit::OccupancyParticipant>,
    observed: Observation,
    generation: u64,
}
impl RecoveredRoom {
    fn count(&self) -> i64 {
        crate::casts::len_i64(
            self.participants
                .iter()
                .filter(|p| counts_as_player(&p.identity))
                .count(),
        )
    }
}
#[derive(Clone, Default)]
struct Complete {
    rooms: Vec<RecoveredRoom>,
    /// Relevant rooms left out because this node refuses their names.
    refused: Vec<String>,
    observed: Option<Observation>,
    unknown_generation: u64,
}
impl Complete {
    fn uncertain_for(&self, scope: &Scope) -> bool {
        self.refused.iter().any(|room| scope.may_hold(room))
            || self
                .rooms
                .iter()
                .any(|room| room.legacy && scope.may_hold(&room.name))
    }
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
    // Social's online view: World room events and every read's World participants.
    topology: std::sync::OnceLock<Arc<crate::topology::Topology>>,
    // The World participants the background read in flight has listed so far.
    partial: Mutex<Option<PartialRead>>,
}

/// A read's start and the `(sid, identity)` World participants it has listed.
type PartialRead = (Instant, Vec<(String, String)>);
impl Default for Presence {
    fn default() -> Self {
        Self {
            state: Mutex::new(StateData::default()),
            reads: tokio::sync::Semaphore::new(4),
            background: tokio::sync::Semaphore::new(3),
            admission_flight: tokio::sync::Semaphore::new(1),
            waiters: tokio::sync::Semaphore::new(64),
            dirty: tokio::sync::Notify::new(),
            topology: std::sync::OnceLock::new(),
            partial: Mutex::new(None),
        }
    }
}

pub(crate) fn counts_as_player(identity: &str) -> bool {
    identity != AUTHORITATIVE_SERVER_IDENTITY
}

impl Presence {
    pub fn new() -> Self {
        Self::default()
    }

    /// Bounded invalidation: unseen rooms bump one epoch rather than allocating
    /// attacker-controlled event keys. Known rooms retain independent generations.
    /// A name that can never classify as relevant (island, voice chat) is never
    /// counted, so its events leave every occupancy read valid.
    pub fn invalidate_room(&self, room: &str) {
        let mut state = self.state.lock();
        if let Some(generation) = state.generations.get_mut(room) {
            *generation = generation.wrapping_add(1);
        } else if crate::rooms::may_classify_relevant(room) {
            state.unknown_generation = state.unknown_generation.wrapping_add(1);
        } else {
            return;
        }
        drop(state);
        self.dirty.notify_one();
    }

    /// Feeds World room events and every complete observation's World participants
    /// to `topology` (once).
    pub fn attach_topology(&self, topology: Arc<crate::topology::Topology>) {
        if self.topology.set(topology).is_ok() {
            let complete = self.complete();
            if let Some(observed) = &complete.observed {
                self.observe_world(&complete.rooms, observed.mono, true);
            }
        }
    }

    /// One verified World join or leave, from this replica's webhook or another's.
    /// Genesis presence comes from Pulse islands, so other rooms are ignored.
    pub fn apply_room_event(&self, event: &crate::protocol::bus::room_events::RoomEvent) {
        use crate::protocol::bus::room_events::{RoomEventKind, RoomType};
        let Some(topology) = self.topology.get() else {
            return;
        };
        let joined = match RoomEventKind::try_from(event.kind) {
            Ok(RoomEventKind::Joined) => true,
            Ok(RoomEventKind::Left) => false,
            _ => return,
        };
        if event.room_type == RoomType::World as i32 && counts_as_player(&event.identity) {
            topology.apply_world_event(&event.participant_sid, &event.identity, joined);
        }
    }

    /// A complete read of every World room that started at `started`.
    fn observe_world(&self, rooms: &[RecoveredRoom], started: Instant, complete: bool) {
        let Some(topology) = self.topology.get() else {
            return;
        };
        let participants = rooms
            .iter()
            .filter(|r| r.is_world && !r.legacy)
            .flat_map(|r| &r.participants)
            .filter(|p| counts_as_player(&p.identity))
            .map(|p| (p.sid.as_str(), p.identity.as_str()));
        topology.observe_world(started, participants, complete);
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
    fn visible(&self) -> Result<Arc<Complete>, PresenceUnavailable> {
        let complete = self.complete();
        if complete
            .observed
            .as_ref()
            .is_some_and(|o| o.fresh(PUBLIC_TTL))
        {
            Ok(complete)
        } else {
            Err(PresenceUnavailable)
        }
    }
    /// The world of the wallet's latest join. Equal join times, including
    /// missing timestamps, select the first world in name order.
    pub fn get_peer_world(&self, wallet: &str) -> Result<Option<String>, PresenceUnavailable> {
        let view = self.visible()?;
        if wallet.eq_ignore_ascii_case(AUTHORITATIVE_SERVER_IDENTITY) {
            return Ok(None);
        }
        Ok(view
            .rooms
            .iter()
            .filter(|room| room.is_world)
            .flat_map(|room| {
                room.participants
                    .iter()
                    .filter(|p| {
                        counts_as_player(&p.identity) && p.identity.eq_ignore_ascii_case(wallet)
                    })
                    .map(|p| (p.joined_at_ms, std::cmp::Reverse(room.realm.as_str())))
            })
            .max()
            .map(|(_, world)| world.0.to_owned()))
    }
    pub fn world_counts(&self) -> Vec<(String, i64)> {
        self.visible()
            .map_or_else(|_| Vec::new(), |view| Self::counts_of(&view))
    }
    fn counts_of(complete: &Complete) -> Vec<(String, i64)> {
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
        let Ok(view) = self.visible() else {
            return CommsStats { rooms: 0, users: 0 };
        };
        let rooms = view.rooms.iter().filter(|r| r.is_world && !r.legacy);
        let mut identities = HashSet::new();
        for room in rooms.clone() {
            for p in &room.participants {
                if counts_as_player(&p.identity) {
                    identities.insert(p.identity.as_str());
                }
            }
        }
        CommsStats {
            rooms: crate::casts::len_i64(rooms.filter(|r| r.count() > 0).count()),
            users: crate::casts::len_i64(identities.len()),
        }
    }
    /// Public lobby projection only. Admission must use the bounded async API.
    pub fn world_participant_count(&self, world: &str) -> i64 {
        self.world_counts()
            .into_iter()
            .find(|(w, _)| w == &world.to_lowercase())
            .map_or(0, |(_, n)| n)
    }
    /// The lowercased player identities of the last complete observation in
    /// a world's rooms (`scene` `None`: any of its rooms) or in one Genesis
    /// scene's rooms, with the earliest instant one of those rooms was read:
    /// a wallet that joined a room after that instant may be missing.
    /// `None` when that observation is stale or a room this node refused
    /// could hold them: no answer, never an empty one.
    pub fn room_occupants(
        &self,
        realm: &str,
        scene: Option<&str>,
    ) -> Option<(chrono::DateTime<chrono::Utc>, HashSet<String>)> {
        let view = self.visible().ok()?;
        let realm = realm.to_lowercase();
        if scene.is_none() && view.uncertain_for(&Scope::Lobby(realm.clone())) {
            return None;
        }
        let rooms: Vec<_> = view
            .rooms
            .iter()
            .filter(|r| match scene {
                None => r.is_world && r.realm == realm,
                Some(scene) => !r.is_world && r.realm == realm && r.scene.as_deref() == Some(scene),
            })
            .collect();
        let read_at = rooms
            .iter()
            .map(|r| r.observed.utc)
            .min()
            .or(view.observed.as_ref().map(|o| o.utc))?;
        let wallets = rooms
            .iter()
            .flat_map(|r| &r.participants)
            .filter(|p| counts_as_player(&p.identity))
            .map(|p| p.identity.to_lowercase())
            .collect();
        Some((read_at, wallets))
    }
    /// Positive evidence of a specific session from a fresh, complete
    /// LiveKit roster. Webhook invalidation makes an older roster unusable.
    pub fn session_observed_at(
        &self,
        room: &str,
        participant_sid: &str,
        wallet: &str,
    ) -> Option<chrono::DateTime<chrono::Utc>> {
        let state = self.state.lock();
        let roster = state.complete.rooms.iter().find(|r| r.name == room)?;
        (roster.observed.fresh(PUBLIC_TTL)
            && state.complete.unknown_generation == state.unknown_generation
            && state.generations.get(room) == Some(&roster.generation)
            && roster
                .participants
                .iter()
                .any(|p| p.sid == participant_sid && p.identity.eq_ignore_ascii_case(wallet)))
        .then_some(roster.observed.utc)
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
            || complete.uncertain_for(scope)
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
            refused: vec![],
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
                    legacy: false,
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
                joined_at_ms: 0,
            });
        }
        view.observed = Some(observed);
        state.generations.entry(room.into()).or_insert(0);
        let complete = state.complete.clone();
        drop(state);
        self.observe_world(&complete.rooms, Instant::now(), true);
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn peer_left(&self, identity: &str, room: &str, _: bool) {
        let mut state = self.state.lock();
        for r in &mut Arc::make_mut(&mut state.complete).rooms {
            if r.name == room {
                r.participants.retain(|p| p.identity != identity);
            }
        }
        let complete = state.complete.clone();
        drop(state);
        if complete.observed.is_some() {
            self.observe_world(&complete.rooms, Instant::now(), true);
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
    match presence.get_peer_world(&wallet).map_err(unavailable)? {
        Some(world) => Ok(Json(json!({ "wallet": wallet, "world": world }))),
        None => Err(ApiError::not_found(format!(
            "Wallet {wallet} is not connected to any world"
        ))),
    }
}

fn unavailable(_: PresenceUnavailable) -> ApiError {
    ApiError::service_unavailable("World presence is unavailable. Retry shortly.")
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
pub async fn live_data(
    State(presence): State<Arc<Presence>>,
) -> Result<Json<LiveDataResponse>, ApiError> {
    // Capture membership and source clock from the same immutable generation.
    let complete = presence.visible().map_err(unavailable)?;
    let counts = Presence::counts_of(&complete);
    let last_updated = complete
        .observed
        .as_ref()
        .ok_or_else(|| unavailable(PresenceUnavailable))?
        .utc
        .to_rfc3339_opts(SecondsFormat::Millis, true);
    let total: i64 = counts.iter().map(|(_, c)| c).sum();
    let per_world = counts
        .into_iter()
        .map(|(world_name, users)| WorldOccupancy { world_name, users })
        .collect();
    Ok(Json(LiveDataResponse {
        data: LiveDataPayload {
            total_users: total,
            per_world,
        },
        last_updated,
    }))
}

#[cfg(test)]
mod occupants_tests {
    use super::Presence;

    #[test]
    fn room_occupants_answer_a_world_from_the_complete_observation_only() {
        let presence = Presence::new();
        assert_eq!(
            presence.room_occupants("party.dcl.eth", None),
            None,
            "nothing observed yet"
        );
        presence.complete_empty_fixture();
        presence.peer_joined("0xAB", "party.dcl.eth", "world-party.dcl.eth", false);
        presence.peer_joined(
            "0xCD",
            "party.dcl.eth",
            "world-scene-room-party.dcl.eth-x",
            true,
        );
        presence.peer_joined(
            "authoritative-server",
            "party.dcl.eth",
            "world-party.dcl.eth",
            false,
        );
        presence.peer_joined("0xEF", "other.dcl.eth", "world-other.dcl.eth", false);
        let (_, wallets) = presence.room_occupants("Party.dcl.eth", None).unwrap();
        let mut wallets: Vec<_> = wallets.into_iter().collect();
        wallets.sort();
        assert_eq!(wallets, ["0xab", "0xcd"]);
        let (_, none) = presence
            .room_occupants("party.dcl.eth", Some("bafyscene"))
            .unwrap();
        assert!(none.is_empty(), "no Genesis scene room of that realm");
    }

    #[test]
    fn session_proof_requires_the_exact_participant_and_current_room_generation() {
        let presence = Presence::new();
        let room = "world-party.dcl.eth";
        presence.complete_empty_fixture();
        presence.peer_joined("0xAB", "party.dcl.eth", room, false);
        let (session, at) = {
            let mut state = presence.state.lock();
            let roster = &state.complete.rooms[0];
            let session = roster.participants[0].sid.clone();
            let at = roster.observed.utc;
            let generation = roster.generation;
            state.generations.insert(room.to_owned(), generation);
            (session, at)
        };
        assert_eq!(
            presence.session_observed_at(room, &session, "0xab"),
            Some(at)
        );
        assert_eq!(
            presence.session_observed_at(room, "wrong-session", "0xab"),
            None
        );
        assert_eq!(presence.session_observed_at(room, &session, "0xcd"), None);
        presence.invalidate_room(room);
        assert_eq!(presence.session_observed_at(room, &session, "0xab"), None);
    }

    #[test]
    fn room_occupants_are_observed_at_the_earliest_room_read() {
        let presence = Presence::new();
        presence.complete_empty_fixture();
        presence.peer_joined("0xAB", "party.dcl.eth", "world-party.dcl.eth", false);
        presence.peer_joined(
            "0xCD",
            "party.dcl.eth",
            "world-scene-room-party.dcl.eth-x",
            true,
        );
        let early = chrono::DateTime::from_timestamp_millis(1_800_000_000_000).unwrap();
        let late = early + chrono::Duration::seconds(30);
        {
            let mut state = presence.state.lock();
            let view = std::sync::Arc::make_mut(&mut state.complete);
            view.rooms[0].observed.utc = late;
            view.rooms[1].observed.utc = early;
        }
        let (at, _) = presence.room_occupants("party.dcl.eth", None).unwrap();
        assert_eq!(at, early);
    }
}
