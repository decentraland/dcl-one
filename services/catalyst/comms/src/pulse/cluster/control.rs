use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::protocol::bus::control_position::{ControlPosition, LegacyRoomIntent};
use crate::protocol::pulse::decentraland::common::Vector3;

pub(crate) const SUPPORTED_TRANSPORT_PEERS: usize = 4095 + 4096;
pub const DEFAULT_CONTROL_POSITION_CAPACITY: usize = SUPPORTED_TRANSPORT_PEERS * 2;
pub const CONTROL_POSITION_TTL: Duration = Duration::from_secs(10);
pub const CONTROL_TOMBSTONE_TTL: Duration = Duration::from_secs(300);
const CONTROL_SWEEP_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Debug, PartialEq)]
pub struct ControlMember {
    pub slot: u32,
    pub wallet: String,
    pub session: String,
    pub epoch: u64,
    pub origin: i32,
    pub realm: String,
    pub position: Vector3,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ControlSnapshot {
    pub members: Vec<ControlMember>,
    pub owners: HashMap<String, ControlOwner>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlOwner {
    pub session: String,
    pub closed: bool,
}

struct Head {
    session: String,
    epoch: u64,
    sequence: u64,
    realm: String,
    position: Option<Vector3>,
    closed: bool,
    observed: Instant,
}

struct Entry {
    slot: u32,
    heads: [Option<Head>; 2],
}

pub(crate) struct ControlPositions {
    audience: Option<String>,
    live_ttl: Duration,
    tombstone_ttl: Duration,
    entries: HashMap<String, Entry>,
    free_slots: Vec<u32>,
    last_sweep: Instant,
}

impl Default for ControlPositions {
    fn default() -> Self {
        Self::new(
            DEFAULT_CONTROL_POSITION_CAPACITY,
            CONTROL_POSITION_TTL,
            CONTROL_TOMBSTONE_TTL,
        )
    }
}

impl ControlPositions {
    pub(crate) fn new(capacity: usize, live_ttl: Duration, tombstone_ttl: Duration) -> Self {
        let capacity = capacity.min(u32::MAX as usize);
        Self {
            audience: None,
            live_ttl,
            tombstone_ttl,
            entries: HashMap::new(),
            free_slots: (0..capacity as u32).rev().collect(),
            last_sweep: Instant::now(),
        }
    }

    pub(crate) fn enable(&mut self, audience: &str) -> Result<(), &'static str> {
        if audience.is_empty() || audience.len() > 256 {
            return Err("control position audience must be between 1 and 256 bytes");
        }
        match self.audience.as_deref() {
            Some(current) if current != audience => {
                Err("control position audience cannot change after enable")
            }
            Some(_) => Ok(()),
            None => {
                self.audience = Some(audience.to_string());
                Ok(())
            }
        }
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.audience.is_some()
    }
    pub(crate) fn ingest(&mut self, update: ControlPosition) -> bool {
        self.ingest_at(update, Instant::now())
    }
    pub(crate) fn ingest_intent(&mut self, update: LegacyRoomIntent) -> bool {
        if update.encode().is_none() {
            return false;
        }
        self.ingest_version_at(update.position_update(), 1, Instant::now())
    }
    fn ingest_at(&mut self, update: ControlPosition, now: Instant) -> bool {
        self.ingest_version_at(update, 0, now)
    }
    fn ingest_version_at(&mut self, update: ControlPosition, version: usize, now: Instant) -> bool {
        if self.audience.as_deref() != Some(update.audience.as_str()) || update.encode().is_none() {
            return false;
        }
        let wallet = update.wallet.to_ascii_lowercase();
        if !self.entries.contains_key(&wallet) {
            if now.saturating_duration_since(self.last_sweep) >= CONTROL_SWEEP_INTERVAL {
                self.expire_at(now);
            }
            if self.free_slots.is_empty() {
                self.expire_at(now);
            }
            let Some(slot) = self.free_slots.pop() else {
                return false;
            };
            self.entries.insert(
                wallet.clone(),
                Entry {
                    slot,
                    heads: [None, None],
                },
            );
        }
        let head = &mut self.entries.get_mut(&wallet).unwrap().heads[version];
        if let Some(current) = head {
            expire_head(current, now, self.live_ttl);
            if (update.epoch, update.sequence) <= (current.epoch, current.sequence)
                || (update.epoch == current.epoch
                    && (current.closed || !current.session.eq_ignore_ascii_case(&update.session)))
            {
                return false;
            }
        }
        *head = Some(Head {
            session: update.session.to_ascii_lowercase(),
            epoch: update.epoch,
            sequence: update.sequence,
            realm: update.realm,
            position: update.position.map(vector),
            closed: update.position.is_none(),
            observed: now,
        });
        true
    }

    pub(crate) fn snapshot(&mut self) -> ControlSnapshot {
        self.snapshot_at(Instant::now())
    }

    // No production common allocator attestation exists yet. Both heads are
    // retained, but neither can supply mixed authority by audience text alone.
    fn winner(entry: &Entry) -> Option<(usize, &Head)> {
        match (&entry.heads[0], &entry.heads[1]) {
            (Some(head), None) => Some((0, head)),
            (None, Some(head)) => Some((1, head)),
            _ => None,
        }
    }

    pub(crate) fn allows_selected(
        &self,
        wallet: &str,
        session: &str,
        realm: &str,
        epoch: u64,
        origin: i32,
    ) -> bool {
        let Some((version, head)) = self.entries.get(wallet).and_then(Self::winner) else {
            return false;
        };
        origin == version as i32 + 2
            && head.epoch == epoch
            && head.realm == realm
            && head.session == session
            && !head.closed
            && head.position.is_some()
            && head.observed.elapsed() < self.live_ttl
    }

    #[cfg(test)]
    fn allows_assignment(
        &self,
        wallet: &str,
        session: &str,
        realm: &str,
        epoch: u64,
        native: bool,
    ) -> bool {
        !native && self.allows_selected(wallet, session, realm, epoch, 2)
    }

    fn snapshot_at(&mut self, now: Instant) -> ControlSnapshot {
        self.expire_at(now);
        let mut snapshot = ControlSnapshot::default();
        for (wallet, entry) in &self.entries {
            let Some((version, head)) = Self::winner(entry) else {
                continue;
            };
            snapshot.owners.insert(
                wallet.clone(),
                ControlOwner {
                    session: head.session.clone(),
                    closed: head.closed,
                },
            );
            if let Some(position) = head.position {
                snapshot.members.push(ControlMember {
                    slot: entry.slot,
                    wallet: wallet.clone(),
                    session: head.session.clone(),
                    epoch: head.epoch,
                    origin: version as i32 + 2,
                    realm: head.realm.clone(),
                    position,
                });
            }
        }
        snapshot.members.sort_unstable_by_key(|m| m.slot);
        snapshot
    }

    fn expire_at(&mut self, now: Instant) {
        let mut removed = Vec::new();
        self.entries.retain(|_, entry| {
            for head in &mut entry.heads {
                if let Some(value) = head {
                    expire_head(value, now, self.live_ttl);
                    if value.position.is_none()
                        && now.saturating_duration_since(value.observed) >= self.tombstone_ttl
                    {
                        *head = None;
                    }
                }
            }
            let retain = entry.heads.iter().any(Option::is_some);
            if !retain {
                removed.push(entry.slot);
            }
            retain
        });
        self.free_slots.extend(removed);
        self.last_sweep = now;
    }
}

fn expire_head(head: &mut Head, now: Instant, live_ttl: Duration) {
    if head.position.is_some() && now.saturating_duration_since(head.observed) >= live_ttl {
        head.position = None;
        head.closed = false;
        head.observed = now;
    }
}
fn vector(position: [f32; 3]) -> Vector3 {
    Vector3 {
        x: position[0],
        y: position[1],
        z: position[2],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_aba_preserves_cleanup_across_terminal_and_expired_control_heads() {
        use crate::pulse::cluster::feed::{CurrentAssignment, CurrentAssignments};
        use crate::pulse::snapshot::IdentityBoard;
        use std::sync::{Arc, Mutex};
        for origin in [2, 3] {
            for same_session in [false, true] {
                for terminal in [false, true] {
                    for before_native in [false, true] {
                        let now = Instant::now();
                        let controls = Mutex::new(ControlPositions::default());
                        controls.lock().unwrap().enable("realm-a").unwrap();
                        let current = Arc::new(CurrentAssignments::default());
                        let mut identity = IdentityBoard::new(1);
                        identity.attach_assignments(current.clone());
                        let version = (origin - 2) as usize;
                        let mut input = update(1, 8, 1, Some([0.0; 3]));
                        let assignment = |input: &ControlPosition| CurrentAssignment {
                            wallet: input.wallet.clone(),
                            session: input.session.clone(),
                            cluster_id: "room".into(),
                            realm: input.realm.clone(),
                            owner_epoch: input.epoch,
                            selected_origin: origin,
                            owner_peer: 0,
                            registration: None,
                        };
                        assert!(controls.lock().unwrap().ingest_version_at(
                            input.clone(),
                            version,
                            now
                        ));
                        current.replace_checked(
                            1,
                            Duration::from_secs(30),
                            vec![assignment(&input)],
                            Some(&controls),
                        );
                        input.epoch = 9;
                        input.session = format!("0x{:040x}", 99);
                        assert!(controls.lock().unwrap().ingest_version_at(
                            input.clone(),
                            version,
                            now
                        ));
                        current.replace_checked(
                            2,
                            Duration::from_secs(30),
                            vec![assignment(&input)],
                            Some(&controls),
                        );
                        let before = current
                            .lookup_checked(&input.wallet, &input.session, Some(&controls))
                            .unwrap();
                        assert_eq!(before.pending_takeovers.len(), 1);
                        let invalidate_control = || {
                            let mut controls = controls.lock().unwrap();
                            if terminal {
                                let mut withdrawal = input.clone();
                                withdrawal.sequence = 2;
                                withdrawal.position = None;
                                withdrawal.realm.clear();
                                assert!(controls.ingest_version_at(withdrawal, version, now));
                            } else {
                                controls.snapshot_at(now + CONTROL_POSITION_TTL);
                            }
                        };
                        if before_native {
                            invalidate_control();
                        }
                        identity.set_with_session(
                            0,
                            input.wallet.clone(),
                            if same_session {
                                input.session.clone()
                            } else {
                                format!("0x{:040x}", 100)
                            },
                        );
                        if !before_native {
                            invalidate_control();
                        }
                        identity.remove(0);
                        // No pass or lookup occurs between native arrival and departure.
                        let retained = current
                            .lookup_checked(&input.wallet, &input.session, Some(&controls))
                            .unwrap();
                        assert_eq!(retained.assignment_state, 2);
                        assert_eq!(retained.pending_takeovers, before.pending_takeovers);
                        assert_eq!(retained.token_not_before, before.token_not_before);
                        let completion =
                            crate::protocol::pulse::decentraland::pulse::TakeoverCompleted {
                                takeover_id: retained.pending_takeovers[0].takeover_id.clone(),
                                revoke_before: 100,
                            };
                        assert!(
                            current
                                .complete_checked(&input.wallet, &completion, Some(&controls))
                                .is_none()
                        );
                        assert!(
                            current
                                .lookup_checked(&input.wallet, &input.session, Some(&controls))
                                .is_none()
                        );
                        input.sequence = 3;
                        if terminal {
                            assert!(!controls.lock().unwrap().ingest_version_at(
                                input.clone(),
                                version,
                                now + CONTROL_POSITION_TTL
                            ));
                            input.epoch += 1;
                        }
                        assert!(controls.lock().unwrap().ingest_version_at(
                            input.clone(),
                            version,
                            now + CONTROL_POSITION_TTL
                        ));
                        assert!(
                            current
                                .lookup_checked(&input.wallet, &input.session, Some(&controls))
                                .is_none(),
                            "new live control alone cannot restore the invalidated assignment"
                        );
                        current.replace_checked(
                            3,
                            Duration::from_secs(30),
                            vec![assignment(&input)],
                            Some(&controls),
                        );
                        let restored = current
                            .lookup_checked(&input.wallet, &input.session, Some(&controls))
                            .unwrap();
                        assert_eq!(restored.assignment_state, 1);
                        assert!(restored.assignment_version > before.assignment_version);
                        assert_eq!(restored.token_not_before, Some(100));
                    }
                }
            }
        }
    }

    #[test]
    fn versions_share_wallet_capacity_and_unproved_mixed_heads_never_select() {
        let now = Instant::now();
        let mut positions = ControlPositions::new(1, CONTROL_POSITION_TTL, CONTROL_TOMBSTONE_TTL);
        positions.enable("realm-a").unwrap();
        let first = update(1, 7, 1, Some([0.0; 3]));
        assert!(positions.ingest_at(first.clone(), now));
        let mut intent = first.clone();
        intent.epoch = 9;
        assert!(positions.ingest_version_at(intent.clone(), 1, now));
        assert!(positions.snapshot_at(now).members.is_empty());
        assert!(!positions.ingest_at(update(2, 10, 1, Some([0.0; 3])), now));
        intent.position = None;
        intent.realm.clear();
        intent.sequence = 2;
        assert!(positions.ingest_version_at(intent, 1, now));
        let mut older = first;
        older.sequence = 2;
        assert!(positions.ingest_at(older, now));
        assert!(
            positions.snapshot_at(now).members.is_empty(),
            "withdrawal never reveals older class"
        );
        assert_eq!(positions.entries.len(), 1);
    }

    #[test]
    fn duplicate_does_not_refresh_live_expiry_for_either_version() {
        let now = Instant::now();
        for version in 0..2 {
            let mut positions = ControlPositions::default();
            positions.enable("realm-a").unwrap();
            let first = update(1, 7, 1, Some([0.0; 3]));
            assert!(positions.ingest_version_at(first.clone(), version, now));
            assert!(!positions.ingest_version_at(first, version, now + Duration::from_secs(9)));
            assert!(
                positions
                    .snapshot_at(now + CONTROL_POSITION_TTL)
                    .members
                    .is_empty()
            );
        }
    }

    #[test]
    fn explicit_withdrawal_is_terminal_but_live_expiry_can_resume() {
        let mut positions = ControlPositions::default();
        positions.enable("realm-a").unwrap();
        let now = Instant::now();
        let first = update(1, 7, 1, Some([1.0, 2.0, 3.0]));
        assert!(positions.ingest_at(first.clone(), now));
        let mut close = first.clone();
        close.sequence = 2;
        close.position = None;
        close.realm.clear();
        assert!(positions.ingest_at(close, now));
        let mut resume = first.clone();
        resume.sequence = 3;
        assert!(
            !positions.ingest_at(resume, now),
            "explicit close cannot resurrect"
        );

        let mut positions = ControlPositions::default();
        positions.enable("realm-a").unwrap();
        assert!(positions.ingest_at(first.clone(), now));
        assert!(
            positions
                .snapshot_at(now + CONTROL_POSITION_TTL)
                .members
                .is_empty()
        );
        let mut resume = first;
        resume.sequence = 2;
        assert!(positions.ingest_at(resume, now + CONTROL_POSITION_TTL));
        assert_eq!(
            positions
                .snapshot_at(now + CONTROL_POSITION_TTL)
                .members
                .len(),
            1
        );
    }

    #[test]
    fn same_epoch_session_is_fixed() {
        let mut positions = ControlPositions::default();
        positions.enable("realm-a").unwrap();
        let first = update(1, 7, 1, Some([1.0, 2.0, 3.0]));
        assert!(positions.ingest(first.clone()));
        let mut changed = first;
        changed.sequence = 2;
        changed.session = format!("0x{:040x}", 999);
        assert!(!positions.ingest(changed));
    }

    #[test]
    fn live_lookup_guard_rejects_epoch_replacement_release_and_expiry_before_next_pass() {
        let mut positions = ControlPositions::default();
        positions.enable("realm-a").unwrap();
        let mut first = update(1, 7, 10, Some([1.0, 2.0, 3.0]));
        assert!(positions.ingest(first.clone()));
        assert!(positions.allows_assignment(&first.wallet, &first.session, "main", 7, false));
        first.epoch = 8;
        assert!(positions.ingest(first.clone()));
        assert!(!positions.allows_assignment(&first.wallet, &first.session, "main", 7, false));
        assert!(positions.allows_assignment(&first.wallet, &first.session, "main", 8, false));
        positions.entries.get_mut(&first.wallet).unwrap().heads[0]
            .as_mut()
            .unwrap()
            .observed = Instant::now() - CONTROL_POSITION_TTL;
        assert!(!positions.allows_assignment(&first.wallet, &first.session, "main", 8, false));
        first.epoch = 9;
        first.position = None;
        first.realm.clear();
        assert!(positions.ingest(first.clone()));
        assert!(!positions.allows_assignment(&first.wallet, &first.session, "main", 9, false));
        assert!(!positions.allows_assignment(&first.wallet, &first.session, "main", 9, true));
    }

    fn update(
        wallet: u8,
        epoch: u64,
        sequence: u64,
        position: Option<[f32; 3]>,
    ) -> ControlPosition {
        ControlPosition {
            audience: "realm-a".into(),
            wallet: format!("0x{wallet:040x}"),
            session: format!("0x{:040x}", wallet + 100),
            epoch,
            sequence,
            realm: if position.is_some() {
                "main".into()
            } else {
                String::new()
            },
            position,
        }
    }

    #[test]
    fn ordering_audience_release_and_expiry_are_fail_closed() {
        let now = Instant::now();
        let mut positions =
            ControlPositions::new(2, Duration::from_secs(10), Duration::from_secs(30));
        positions.enable("realm-a").unwrap();
        let mut first = update(1, 7, 10, Some([1.0, 2.0, 3.0]));
        assert!(positions.ingest_at(first.clone(), now));
        assert!(!positions.ingest_at(first.clone(), now));
        first.sequence = 9;
        assert!(!positions.ingest_at(first.clone(), now));
        first.epoch = 6;
        first.sequence = 100;
        assert!(!positions.ingest_at(first, now));

        let mut wrong = update(2, 1, 1, Some([0.0; 3]));
        wrong.audience = "realm-b".into();
        assert!(!positions.ingest_at(wrong, now));

        assert_eq!(positions.snapshot_at(now).members.len(), 1);
        assert!(positions.ingest_at(update(1, 7, 11, None), now));
        let released = positions.snapshot_at(now);
        assert!(released.members.is_empty());
        assert_eq!(released.owners.len(), 1);
        assert!(released.owners.values().next().unwrap().closed);
        assert!(!positions.ingest_at(update(1, 7, 10, Some([4.0, 5.0, 6.0])), now));
        assert!(
            positions
                .snapshot_at(now + Duration::from_secs(31))
                .owners
                .is_empty()
        );
    }

    #[test]
    fn capacity_retains_live_members_and_tombstone_high_water() {
        let now = Instant::now();
        let mut positions =
            ControlPositions::new(2, Duration::from_secs(10), Duration::from_secs(30));
        positions.enable("realm-a").unwrap();
        assert!(positions.ingest_at(update(1, 1, 1, Some([0.0; 3])), now));
        assert!(positions.ingest_at(update(2, 1, 2, Some([0.0; 3])), now));
        assert!(!positions.ingest_at(update(3, 1, 3, Some([0.0; 3])), now));
        assert_eq!(positions.snapshot_at(now).members.len(), 2);

        assert!(positions.ingest_at(update(1, 1, 4, None), now));
        assert!(!positions.ingest_at(update(3, 1, 5, Some([0.0; 3])), now));
        assert!(!positions.ingest_at(update(1, 1, 3, Some([0.0; 3])), now));
        let snapshot = positions.snapshot_at(now);
        assert_eq!(snapshot.members.len(), 1);
        assert!(snapshot.owners.contains_key(&format!("0x{:040x}", 1)));

        positions.snapshot_at(now + Duration::from_secs(31));
        assert!(positions.ingest_at(
            update(3, 1, 5, Some([0.0; 3])),
            now + Duration::from_secs(31)
        ));
    }

    #[test]
    fn ingress_sweeps_are_throttled_but_capacity_reclaims_expired_tombstones() {
        let now = Instant::now();
        let mut positions =
            ControlPositions::new(1, Duration::from_secs(1), Duration::from_secs(2));
        positions.enable("realm-a").unwrap();
        positions.last_sweep = now;
        assert!(positions.ingest_at(update(1, 1, 1, Some([0.0; 3])), now));
        assert!(positions.ingest_at(
            update(1, 1, 2, Some([1.0; 3])),
            now + Duration::from_millis(100)
        ));
        assert_eq!(positions.last_sweep, now);

        assert!(positions.ingest_at(update(1, 1, 3, None), now));
        assert!(positions.ingest_at(
            update(2, 1, 1, Some([0.0; 3])),
            now + Duration::from_secs(3)
        ));
    }

    #[test]
    fn default_capacity_covers_all_supported_transport_peers_and_equal_churn() {
        assert_eq!(SUPPORTED_TRANSPORT_PEERS, 8191);
        assert_eq!(
            DEFAULT_CONTROL_POSITION_CAPACITY,
            SUPPORTED_TRANSPORT_PEERS * 2
        );
    }

    #[test]
    fn a_missed_live_heartbeat_becomes_a_tombstone_before_it_is_forgotten() {
        let now = Instant::now();
        let mut positions =
            ControlPositions::new(1, Duration::from_secs(10), Duration::from_secs(30));
        positions.enable("realm-a").unwrap();
        assert!(positions.ingest_at(update(1, 1, 1, Some([0.0; 3])), now));
        let expired_live = positions.snapshot_at(now + Duration::from_secs(10));
        assert!(expired_live.members.is_empty());
        assert_eq!(expired_live.owners.len(), 1);
        assert!(!expired_live.owners.values().next().unwrap().closed);
        let forgotten = positions.snapshot_at(now + Duration::from_secs(40));
        assert!(forgotten.owners.is_empty());
    }
}
