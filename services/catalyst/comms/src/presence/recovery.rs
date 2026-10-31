//! Read-only recovery through the gatekeeper-owned adapter. No event replay.
use super::*;
use crate::gatekeeper::CommsState;
use crate::rooms::RoomClassification;
use crate::shared::livekit::{OccupancyRoom, RoomServiceClient, RoomServiceError};
use futures::{StreamExt, TryStreamExt};

const OPERATION: Duration = Duration::from_secs(2);
const SCAN: Duration = Duration::from_secs(20);
const MAX_ROOMS: usize = 4096;
const MAX_RECORDS: usize = 100_000;
const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum RecoveryFailure {
    #[error("occupancy authority unavailable")]
    Source,
    #[error("occupancy operation deadline exceeded")]
    Deadline,
    #[error("occupancy inventory changed during observation")]
    Changed,
    #[error("occupancy resource budget exceeded")]
    Budget,
    #[error("occupancy contains an unresolved room identity")]
    Unresolved,
    #[error("occupancy room disappeared during its roster read")]
    RoomDisappeared,
}

#[derive(Default)]
struct ScanBudget {
    rooms: usize,
    records: usize,
    bytes: usize,
}

impl ScanBudget {
    fn include(&mut self, records: usize, bytes: usize) -> Result<(), RecoveryFailure> {
        let rooms = self.rooms.checked_add(1).ok_or(RecoveryFailure::Budget)?;
        let records = self
            .records
            .checked_add(records)
            .ok_or(RecoveryFailure::Budget)?;
        let bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or(RecoveryFailure::Budget)?;
        if rooms > MAX_ROOMS || records > MAX_RECORDS || bytes > MAX_BYTES {
            return Err(RecoveryFailure::Budget);
        }
        self.rooms = rooms;
        self.records = records;
        self.bytes = bytes;
        Ok(())
    }
}

#[derive(Clone)]
struct Classified {
    source: OccupancyRoom,
    realm: String,
    is_world: bool,
    scene: Option<String>,
}
impl Classified {
    fn selected(&self, scope: Option<&Scope>) -> bool {
        match scope {
            None => true,
            Some(Scope::Lobby(world)) => {
                self.is_world && self.realm == *world && self.scene.is_none()
            }
            Some(Scope::Scenes(world)) => {
                self.is_world && self.realm == *world && self.scene.is_some()
            }
            Some(Scope::Room(name)) => self.source.name == *name,
        }
    }
}

impl Presence {
    async fn operation<T>(
        &self,
        background: bool,
        future: impl std::future::Future<Output = Result<T, RecoveryFailure>>,
    ) -> Result<T, RecoveryFailure> {
        tokio::time::timeout(OPERATION, async {
            let _background = if background {
                Some(
                    self.background
                        .acquire()
                        .await
                        .map_err(|_| RecoveryFailure::Source)?,
                )
            } else {
                None
            };
            let _permit = self
                .reads
                .acquire()
                .await
                .map_err(|_| RecoveryFailure::Source)?;
            future.await
        })
        .await
        .map_err(|_| RecoveryFailure::Deadline)?
    }

    async fn inventory(
        &self,
        source: &RoomServiceClient<'_>,
        scenes: &dyn crate::interfaces::SceneIdentity,
        background: bool,
        scope: Option<&Scope>,
    ) -> Result<Vec<Classified>, RecoveryFailure> {
        let rooms = self
            .operation(background, async {
                source
                    .occupancy_rooms()
                    .await
                    .map_err(|_| RecoveryFailure::Source)
            })
            .await?;
        let mut out = Vec::new();
        for source in rooms {
            let classification = self
                .operation(background, async {
                    crate::rooms::classify_room_name(scenes, &source.name)
                        .await
                        .map_err(|_| RecoveryFailure::Source)
                })
                .await?;
            match classification {
                RoomClassification::Canonical(room) => {
                    if room.room != source.name {
                        return Err(RecoveryFailure::Unresolved);
                    }
                    let realm = room
                        .realm
                        .world()
                        .map(|world| world.key())
                        .unwrap_or_else(|| room.realm.as_str());
                    if realm.len() > 512
                        || room
                            .scene_id
                            .as_ref()
                            .is_some_and(|scene| scene.as_str().len() > 512)
                    {
                        return Err(RecoveryFailure::Budget);
                    }
                    let classified = Classified {
                        source,
                        realm: realm.to_owned(),
                        is_world: room.realm.is_world(),
                        scene: room.scene_id.map(|scene| scene.as_str().to_owned()),
                    };
                    if classified.selected(scope) {
                        out.push(classified);
                    }
                }

                // A replaced scene's room still decodes to its world and scene,
                // so it is counted there instead of failing every world's read.
                RoomClassification::RelevantRefused {
                    room,
                    reason: crate::rooms::RoomRefusal::Unresolved,
                } if let Some((world, scene)) = crate::rooms::decode_world_scene_room(&room) => {
                    tracing::debug!(%room, "occupancy counts a room without a current deployment");
                    let classified = Classified {
                        source,
                        realm: world.key().to_owned(),
                        is_world: true,
                        scene: Some(scene.as_str().to_owned()),
                    };
                    if classified.selected(scope) {
                        out.push(classified);
                    }
                }
                // Other refused names make even otherwise valid inventory
                // incomplete. Never drop them via the lossy event projection.
                RoomClassification::RelevantRefused { room, reason } => {
                    tracing::warn!(%room, ?reason, "occupancy inventory holds an unresolvable room");
                    return Err(RecoveryFailure::Unresolved);
                }
                RoomClassification::Unrelated => {}
            }
        }
        out.sort_by(|a, b| a.source.cmp(&b.source));
        Ok(out)
    }

    async fn candidate(
        &self,
        state: &CommsState,
        scope: Option<&Scope>,
    ) -> Result<(Vec<RecoveredRoom>, Observation, u64, HashMap<String, u64>), RecoveryFailure> {
        let mut budget = ScanBudget::default();
        // One disappearance permits one complete re-enumeration/re-read, never
        // a fabricated empty roster. Caller deadlines encompass both attempts;
        // completed room/record/byte work is charged to the same scan budget.
        match self.candidate_attempt(state, scope, &mut budget).await {
            Err(RecoveryFailure::RoomDisappeared) => {
                self.candidate_attempt(state, scope, &mut budget).await
            }
            result => result,
        }
    }

    async fn candidate_attempt(
        &self,
        state: &CommsState,
        scope: Option<&Scope>,
        budget: &mut ScanBudget,
    ) -> Result<(Vec<RecoveredRoom>, Observation, u64, HashMap<String, u64>), RecoveryFailure> {
        let background = scope.is_none();
        let source = state.room_service();
        // Capture the unseen-room epoch BEFORE enumeration. A webhook for a
        // previously untracked room during the read cannot be lost at commit.
        let unknown_generation = self.state.lock().unknown_generation;
        let started = Observation::now();
        let before = self
            .inventory(&source, state.scenes.as_ref(), background, scope)
            .await?;
        let generations = {
            let current = self.state.lock();
            before
                .iter()
                .map(|r| {
                    (
                        r.source.name.clone(),
                        current
                            .generations
                            .get(&r.source.name)
                            .copied()
                            .unwrap_or(0),
                    )
                })
                .collect::<HashMap<_, _>>()
        };
        let tasks: Vec<_> = before
            .clone()
            .into_iter()
            .map(|room: Classified| {
                let source = &source;
                let generations = &generations;
                async move {
                    let observed = Observation::now();
                    let participants = self
                        .operation(background, async {
                            source
                                .occupancy_participants(&room.source.name)
                                .await
                                .map_err(|error| match error {
                                    RoomServiceError::NotFound => RecoveryFailure::RoomDisappeared,
                                    _ => RecoveryFailure::Source,
                                })
                        })
                        .await?;
                    Ok::<_, RecoveryFailure>(RecoveredRoom {
                        name: room.source.name.clone(),
                        sid: room.source.sid.clone(),
                        realm: room.realm.clone(),
                        is_world: room.is_world,
                        scene: room.scene.clone(),
                        participants,
                        observed,
                        generation: generations[&room.source.name],
                    })
                }
            })
            .collect();
        let mut reads =
            futures::stream::iter(tasks).buffer_unordered(if background { 3 } else { 1 });
        let mut rooms = Vec::new();
        let mut participant_sids = HashSet::new();
        while let Some(room) = reads.try_next().await? {
            if room
                .participants
                .iter()
                .any(|p| !participant_sids.insert(p.sid.clone()))
            {
                return Err(RecoveryFailure::Source);
            }
            let bytes = room.name.len()
                + room.sid.len()
                + room.realm.len()
                + room.scene.as_ref().map_or(0, String::len)
                + room
                    .participants
                    .iter()
                    .map(|p| p.identity.len() + p.sid.len())
                    .sum::<usize>();
            budget.include(room.participants.len(), bytes)?;
            rooms.push(room);
        }
        drop(reads);
        let after = self
            .inventory(&source, state.scenes.as_ref(), background, scope)
            .await?;
        if before.len() != after.len()
            || before.iter().zip(&after).any(|(a, b)| {
                a.source != b.source
                    || a.realm != b.realm
                    || a.is_world != b.is_world
                    || a.scene != b.scene
            })
        {
            return Err(RecoveryFailure::Changed);
        }
        // Include enumeration in the conservative observation interval. Empty
        // scans also carry a real start time, rather than a fabricated epoch.
        let observed = rooms
            .iter()
            .map(|r| &r.observed)
            .min_by_key(|o| o.mono)
            .cloned()
            .unwrap_or(started);
        Ok((rooms, observed, unknown_generation, generations))
    }

    fn clean(current: &StateData, unknown: u64, generations: &HashMap<String, u64>) -> bool {
        current.unknown_generation == unknown
            && generations.iter().all(|(name, generation)| {
                current.generations.get(name).copied().unwrap_or(0) == *generation
            })
    }

    /// One complete pass. On error, retain membership AND original source time.
    pub async fn recover_complete(&self, state: &CommsState) -> Result<(), RecoveryFailure> {
        let (rooms, observed, unknown, generations) =
            tokio::time::timeout(SCAN, self.candidate(state, None))
                .await
                .map_err(|_| RecoveryFailure::Deadline)??;
        let mut current = self.state.lock();
        if !Self::clean(&current, unknown, &generations) {
            return Err(RecoveryFailure::Changed);
        }
        if current
            .complete
            .observed
            .as_ref()
            .is_some_and(|old| old.mono > observed.mono)
        {
            return Err(RecoveryFailure::Changed);
        }
        current.generations = generations;
        current.admission.clear();
        current.complete = Arc::new(Complete {
            rooms,
            observed: Some(observed),
            unknown_generation: unknown,
        });
        Ok(())
    }

    pub async fn admission_occupancy(
        &self,
        state: &CommsState,
        world: &str,
        scenes: bool,
    ) -> Result<i64, RecoveryFailure> {
        if world.len() > 512 {
            return Err(RecoveryFailure::Budget);
        }
        let scope = if scenes {
            Scope::Scenes(world.to_lowercase())
        } else {
            Scope::Lobby(world.to_lowercase())
        };
        self.refresh_scope(state, scope).await
    }

    /// Internal per-room occupancy, without changing the wallet-only public
    /// scene-participants response or adding a hosting scheduler.
    pub async fn requested_scene_occupancy(
        &self,
        state: &CommsState,
        room: &str,
    ) -> Result<i64, RecoveryFailure> {
        if room.len() > 512 {
            return Err(RecoveryFailure::Budget);
        }
        self.refresh_scope(state, Scope::Room(room.to_owned()))
            .await
    }

    async fn refresh_scope(
        &self,
        state: &CommsState,
        scope: Scope,
    ) -> Result<i64, RecoveryFailure> {
        if !matches!(&scope, Scope::Room(_)) {
            if let Some(count) = self.admission_count(&scope) {
                return Ok(count);
            }
        }
        let _waiter = self
            .waiters
            .try_acquire()
            .map_err(|_| RecoveryFailure::Budget)?;
        tokio::time::timeout(OPERATION, async {
            let _flight = self
                .admission_flight
                .acquire()
                .await
                .map_err(|_| RecoveryFailure::Source)?;
            if let Scope::Room(room) = &scope {
                let classification = self
                    .operation(false, async {
                        crate::rooms::classify_room_name(state.scenes.as_ref(), room)
                            .await
                            .map_err(|_| RecoveryFailure::Source)
                    })
                    .await?;
                match classification {
                    RoomClassification::Canonical(resolved)
                        if resolved.room == *room && resolved.scene_id.is_some() => {}
                    _ => return Err(RecoveryFailure::Unresolved),
                }
            }
            if let Some(count) = self.admission_count(&scope) {
                return Ok(count);
            }
            let (rooms, observed, unknown, generations) =
                self.candidate(state, Some(&scope)).await?;
            if !observed.fresh(ADMISSION_TTL) {
                return Err(RecoveryFailure::Deadline);
            }
            let count = rooms.iter().map(RecoveredRoom::count).sum();
            let mut current = self.state.lock();
            if !Self::clean(&current, unknown, &generations) {
                return Err(RecoveryFailure::Changed);
            }
            let additional = generations
                .keys()
                .filter(|k| !current.generations.contains_key(*k))
                .count();
            if current.generations.len() + additional > MAX_ROOMS
                || (!current.admission.contains_key(&scope) && current.admission.len() >= MAX_ROOMS)
            {
                return Err(RecoveryFailure::Budget);
            }
            // Count aggregate cached scope references, not just the number of
            // cache keys: repeated world scopes cannot multiply an inventory.
            let references: usize = current
                .admission
                .iter()
                .filter(|(key, _)| *key != &scope)
                .map(|(_, sample)| sample.generations.len())
                .sum();
            if references + generations.len() > MAX_ROOMS {
                return Err(RecoveryFailure::Budget);
            }
            let membership_bytes: usize = current
                .complete
                .rooms
                .iter()
                .map(|r| {
                    r.name.len()
                        + r.sid.len()
                        + r.realm.len()
                        + r.scene.as_ref().map_or(0, String::len)
                        + r.participants
                            .iter()
                            .map(|p| p.identity.len() + p.sid.len())
                            .sum::<usize>()
                })
                .sum();
            let cache_bytes: usize = current
                .admission
                .iter()
                .filter(|(key, _)| *key != &scope)
                .map(|(key, sample)| {
                    let key_bytes = match key {
                        Scope::Lobby(s) | Scope::Scenes(s) | Scope::Room(s) => s.len(),
                    };
                    key_bytes + sample.generations.keys().map(String::len).sum::<usize>()
                })
                .sum();
            let scope_bytes = match &scope {
                Scope::Lobby(s) | Scope::Scenes(s) | Scope::Room(s) => s.len(),
            };
            if membership_bytes
                + cache_bytes
                + scope_bytes
                + generations.keys().map(String::len).sum::<usize>()
                > MAX_BYTES
            {
                return Err(RecoveryFailure::Budget);
            }
            current.generations.extend(generations.clone());
            current.admission.insert(
                scope,
                AdmissionSample {
                    count,
                    observed,
                    unknown_generation: unknown,
                    generations,
                },
            );
            Ok(count)
        })
        .await
        .map_err(|_| RecoveryFailure::Deadline)?
    }

    pub(crate) fn spawn_recovery(
        self: &Arc<Self>,
        state: Arc<CommsState>,
        jobs: &crate::jobs::Jobs,
    ) {
        let presence = self.clone();
        jobs.spawn("comms-occupancy-recovery", move |ctx| async move {
            loop {
                let result = tokio::select! {
                    _ = ctx.cancel.cancelled() => return Ok(()),
                    result = presence.recover_complete(&state) => result,
                };
                let delay = match result {
                    Ok(()) => { ctx.progress("complete LiveKit occupancy observation"); Duration::from_secs(30) },
                    Err(error) => { ctx.error(error.to_string()); Duration::from_millis(5000 + rand::random::<u64>() % 1001) },
                };
                ctx.pass_done();
                // Coalesced hints never cause a tight loop. A pending hint can
                // shorten a successful poll to the bounded retry cadence.
                tokio::select! {
                    _ = ctx.cancel.cancelled() => return Ok(()),
                    _ = tokio::time::sleep(delay) => {},
                    _ = presence.dirty.notified() => {
                        tokio::select! {
                            _ = ctx.cancel.cancelled() => return Ok(()),
                            _ = tokio::time::sleep(Duration::from_millis(5000 + rand::random::<u64>() % 1001)) => {},
                        }
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;

    #[test]
    fn exact_room_record_and_decoded_byte_limits_are_inclusive_and_excess_is_atomic() {
        let mut rooms = ScanBudget::default();
        for _ in 0..MAX_ROOMS {
            rooms.include(0, 2).unwrap();
        }
        assert_eq!(rooms.rooms, 4096);
        assert!(matches!(rooms.include(0, 2), Err(RecoveryFailure::Budget)));
        assert_eq!((rooms.rooms, rooms.records, rooms.bytes), (4096, 0, 8192));

        let mut records = ScanBudget::default();
        records.include(50_000, 100_000).unwrap();
        records.include(50_000, 100_000).unwrap();
        assert_eq!(records.records, 100_000);
        assert!(matches!(
            records.include(1, 2),
            Err(RecoveryFailure::Budget)
        ));
        assert_eq!(
            (records.rooms, records.records, records.bytes),
            (2, 100_000, 200_000)
        );

        let mut bytes = ScanBudget::default();
        bytes.include(8192, 8 * 1024 * 1024).unwrap();
        bytes.include(8192, 8 * 1024 * 1024).unwrap();
        assert_eq!(bytes.bytes, 16 * 1024 * 1024);
        assert!(matches!(bytes.include(0, 1), Err(RecoveryFailure::Budget)));
        assert_eq!(
            (bytes.rooms, bytes.records, bytes.bytes),
            (2, 16384, 16 * 1024 * 1024)
        );
        assert!(matches!(
            bytes.include(usize::MAX, usize::MAX),
            Err(RecoveryFailure::Budget)
        ));
        assert_eq!(bytes.bytes, 16 * 1024 * 1024);
    }
}
