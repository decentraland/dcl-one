//! The outbound cluster feed: the publisher seam the tracker hands assignments to, and the
//! coalescing outbox a broker-backed implementation drains.
//!
//! Publisher methods are fire-and-forget hand-offs: they enqueue and return without waiting
//! on a broker, so a stalled or absent one never slows a tracker pass, and none returns an
//! error the caller has to handle.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::Duration;

use crate::protocol::bus::cluster::ClusterChange;
use crate::protocol::bus::topology::ISLANDS_SUBJECT;
use crate::protocol::pulse::decentraland::common::Position;
use crate::protocol::pulse::decentraland::kernel::comms::v3::{IslandData, IslandStatusMessage};
use crate::protocol::pulse::decentraland::pulse::PeerClusterChange;
use crate::protocol::pulse::decentraland::pulse::PeerClusterSnapshot;
use crate::pulse::cluster::control::ControlSnapshot;
use crate::pulse::cluster::{ClusterPass, ClusterSession};

#[derive(Debug, Clone)]
pub struct CurrentAssignment {
    pub wallet: String,
    pub session: String,
    pub cluster_id: String,
    pub realm: String,
    pub owner_epoch: u64,
    pub owner_peer: u32,
    pub registration: Option<std::sync::Arc<std::sync::atomic::AtomicU64>>,
}

impl PartialEq for CurrentAssignment {
    fn eq(&self, other: &Self) -> bool {
        (
            &self.wallet,
            &self.session,
            &self.cluster_id,
            &self.realm,
            self.owner_epoch,
            self.owner_peer,
        ) == (
            &other.wallet,
            &other.session,
            &other.cluster_id,
            &other.realm,
            other.owner_epoch,
            other.owner_peer,
        )
    }
}
impl Eq for CurrentAssignment {}

// Distinct native, WebTransport and control-position wallets may coexist.
// Reserve a second complete cohort for departures inside the retention window.
// This is finite backpressure, never permission to discard unfinished recovery.
const SIMULTANEOUS_WALLETS: usize =
    super::control::SUPPORTED_TRANSPORT_PEERS + super::control::DEFAULT_CONTROL_POSITION_CAPACITY;
const RECOVERY_CHURN_RESERVE: usize = SIMULTANEOUS_WALLETS;
const RECOVERY_WALLETS: usize = SIMULTANEOUS_WALLETS + RECOVERY_CHURN_RESERVE;
const RECOVERY_ROOMS: usize = 32;
const RETENTION_PASSES: u64 = 300;
const FLOOR_GRACE_SECONDS: u64 = 15;

struct RecoveryRecord {
    current: Option<(CurrentAssignment, u64)>,
    potential: Option<CurrentAssignment>,
    retained: Option<(CurrentAssignment, u64)>,
    pending: Vec<(
        crate::protocol::pulse::decentraland::pulse::PendingTakeover,
        u64,
    )>,
    floor: u64,
    last_pass: u64,
    blocked: bool,
}

struct AssignmentView {
    observed: tokio::time::Instant,
    valid_for: Duration,
    pass: u64,
    next_version: u64,
    hint_cursor: usize,
    records: HashMap<String, RecoveryRecord>,
}

/// Pulse's nonpersistent authority. Lossy hints never own recovery state.
pub struct CurrentAssignments {
    incarnation: String,
    view: Mutex<AssignmentView>,
}

impl Default for CurrentAssignments {
    fn default() -> Self {
        Self {
            incarnation: uuid::Uuid::new_v4().to_string(),
            view: Mutex::new(AssignmentView {
                observed: tokio::time::Instant::now(),
                valid_for: Duration::ZERO,
                pass: 0,
                next_version: 0,
                hint_cursor: 0,
                records: HashMap::new(),
            }),
        }
    }
}

fn unix_seconds() -> u64 {
    chrono::Utc::now().timestamp().max(0) as u64
}

impl CurrentAssignments {
    pub fn replace(
        &self,
        pass: u64,
        valid_for: Duration,
        assignments: Vec<CurrentAssignment>,
    ) -> Vec<PeerClusterSnapshot> {
        let mut view = self.view.lock().unwrap();
        view.observed = tokio::time::Instant::now();
        view.valid_for = valid_for;
        view.pass = pass;
        let mut desired: HashMap<_, _> = assignments
            .into_iter()
            .map(|mut a| {
                a.wallet.make_ascii_lowercase();
                a.session.make_ascii_lowercase();
                (a.wallet.clone(), a)
            })
            .collect();
        let now = unix_seconds();
        // Account for every departure before reclaiming or admitting any new
        // identity. Wallet sort order must not keep safely expired records in
        // front of a new wallet that sorts earlier.
        let mut departed = Vec::new();
        for (wallet, record) in &mut view.records {
            if !desired.contains_key(wallet)
                && let Some(previous) = record.current.take()
            {
                record.retained = Some(previous);
                departed.push(wallet.clone());
            }
        }
        view.records.retain(|_, r| {
            r.current.is_some()
                || r.blocked
                || !r.pending.is_empty()
                || pass.saturating_sub(r.last_pass) <= RETENTION_PASSES
                || now <= r.floor.saturating_add(FLOOR_GRACE_SECONDS)
        });
        let mut wallets: Vec<_> = view.records.keys().cloned().collect();
        for wallet in desired.keys() {
            if !view.records.contains_key(wallet) {
                wallets.push(wallet.clone());
            }
        }
        wallets.sort();
        let mut hints: Vec<_> = departed
            .into_iter()
            .filter_map(|wallet| self.snapshot(&mut view, &wallet))
            .collect();
        let mut refused = 0;
        for wallet in wallets {
            let assignment = desired.remove(&wallet);
            if !view.records.contains_key(&wallet) {
                if view.records.len() >= RECOVERY_WALLETS {
                    refused += 1;
                    continue;
                }
                view.records.insert(
                    wallet.clone(),
                    RecoveryRecord {
                        current: None,
                        potential: None,
                        retained: None,
                        pending: Vec::new(),
                        floor: 0,
                        last_pass: pass,
                        blocked: false,
                    },
                );
            }
            let unchanged =
                view.records[&wallet].current.as_ref().map(|(a, _)| a) == assignment.as_ref();
            if unchanged && (assignment.is_none() || !view.records[&wallet].blocked) {
                if assignment.is_some() {
                    view.records.get_mut(&wallet).unwrap().last_pass = pass;
                }
                continue;
            }
            let version = if assignment.is_some() {
                view.next_version.checked_add(1)
            } else {
                Some(view.next_version)
            };
            if assignment.is_some() && version.is_some() {
                view.next_version = version.unwrap();
            }
            let record = view.records.get_mut(&wallet).unwrap();
            if let Some(previous) = record.current.take() {
                record.retained = Some(previous);
            }
            record.blocked = false;
            if let Some(previous) = record.potential.as_ref().filter(|previous| {
                assignment
                    .as_ref()
                    .is_some_and(|a| !a.session.eq_ignore_ascii_case(&previous.session))
            }) {
                if record.pending.len() >= RECOVERY_ROOMS {
                    record.blocked = true;
                } else {
                    record.pending.push((
                        crate::protocol::pulse::decentraland::pulse::PendingTakeover {
                            takeover_id: uuid::Uuid::new_v4().to_string(),
                            displaced_session: previous.session.clone(),
                            displaced_cluster_id: previous.cluster_id.clone(),
                            displaced_realm: previous.realm.clone(),
                        },
                        record.floor,
                    ));
                    record.potential = None;
                }
            }
            if let Some(assignment) = assignment {
                record.last_pass = pass;
                if let Some(version) = version {
                    record.retained = Some((assignment.clone(), version));
                    record.current = Some((assignment, version));
                } else {
                    record.blocked = true;
                }
            }
            if let Some(snapshot) = self.snapshot(&mut view, &wallet) {
                hints.push(snapshot);
            }
        }
        crate::pulse::metrics::recovery_capacity(view.records.len(), refused);
        if refused > 0 {
            tracing::warn!(
                refused,
                retained = view.records.len(),
                capacity = RECOVERY_WALLETS,
                "Pulse recovery ledger exhausted; new wallet assignments withheld without evicting safety state"
            );
        }
        hints
    }

    fn snapshot(&self, view: &mut AssignmentView, wallet: &str) -> Option<PeerClusterSnapshot> {
        use prost::Message;
        let record = view.records.get_mut(wallet)?;
        if record.current.as_ref().is_some_and(|(a, _)| {
            a.registration.as_ref().is_some_and(|guard| {
                let live = guard.load(std::sync::atomic::Ordering::SeqCst);
                live != 0 && live != a.owner_epoch
            })
        }) {
            return None;
        }
        let active = view.observed.elapsed() < view.valid_for
            && record.current.as_ref().is_some_and(|(a, _)| {
                a.registration.as_ref().is_none_or(|guard| {
                    guard.load(std::sync::atomic::Ordering::SeqCst) == a.owner_epoch
                })
            });
        let (assignment, version) = if active {
            record.current.as_ref()?
        } else if !record.pending.is_empty() {
            record.retained.as_ref()?
        } else {
            return None;
        };
        let mut snapshot = PeerClusterSnapshot {
            wallet: assignment.wallet.clone(),
            session: assignment.session.clone(),
            cluster_id: assignment.cluster_id.clone(),
            realm: assignment.realm.clone(),
            pass: view.pass,
            source_incarnation: self.incarnation.clone(),
            assignment_version: *version,
            pending_takeovers: record.pending.iter().map(|(p, _)| p.clone()).collect(),
            token_not_before: (!record.blocked || !record.pending.is_empty())
                .then_some(record.floor),
        };
        if snapshot.encoded_len() > crate::protocol::bus::cluster::MAX_REPLY_BYTES {
            // No incomplete recovery plan can become ready.
            return None;
        }
        if active && snapshot.token_not_before.is_some() && snapshot.pending_takeovers.is_empty() {
            record.potential = Some(assignment.clone());
        }
        if record.blocked {
            snapshot.token_not_before = None;
        }
        Some(snapshot)
    }

    pub fn lookup(&self, wallet: &str, session: &str) -> Option<PeerClusterSnapshot> {
        let mut view = self.view.lock().unwrap();
        let snapshot = self.snapshot(&mut view, wallet)?;
        (snapshot.session == session).then_some(snapshot)
    }

    pub(crate) fn retained_hints(&self, budget: usize) -> Vec<PeerClusterSnapshot> {
        let mut view = self.view.lock().unwrap();
        let mut wallets: Vec<_> = view
            .records
            .iter()
            .filter(|(_, r)| !r.pending.is_empty())
            .map(|(w, _)| w.clone())
            .collect();
        wallets.sort();
        if wallets.is_empty() {
            view.hint_cursor = 0;
            return Vec::new();
        }
        let count = budget.min(wallets.len());
        let start = view.hint_cursor % wallets.len();
        view.hint_cursor = (start + count) % wallets.len();
        (0..count)
            .filter_map(|offset| {
                self.snapshot(&mut view, &wallets[(start + offset) % wallets.len()])
            })
            .collect()
    }

    pub(crate) fn registration_identity(&self, wallet: &str) -> Option<(u64, bool)> {
        let view = self.view.lock().unwrap();
        let (a, _) = view.records.get(wallet)?.current.as_ref()?;
        Some((a.owner_epoch, a.registration.is_some()))
    }

    pub fn complete(
        &self,
        wallet: &str,
        completion: &crate::protocol::pulse::decentraland::pulse::TakeoverCompleted,
    ) -> Option<PeerClusterSnapshot> {
        let mut view = self.view.lock().unwrap();
        let record = view.records.get_mut(wallet)?;
        let index = record
            .pending
            .iter()
            .position(|(p, _)| p.takeover_id == completion.takeover_id)?;
        if completion.revoke_before == 0
            || completion.revoke_before > crate::protocol::bus::cluster::MAX_CONSUMER_SECONDS
            || completion.revoke_before <= record.pending[index].1
        {
            return None;
        }
        record.pending.remove(index);
        record.floor = record.floor.max(completion.revoke_before);
        self.snapshot(&mut view, wallet)
    }
}

/// `IslandData.max_peers` belongs to archipelago's shape and Pulse caps cluster size nowhere, so
/// every island goes out carrying zero rather than a bound this server does not enforce.
const NO_PEER_CAP: u32 = 0;

pub trait ClusterFeedPublisher: Send + Sync {
    fn control_snapshot(&self) -> ControlSnapshot {
        ControlSnapshot::default()
    }

    /// A peer's published (post-debounce) cluster assignment changed. `session` names the session
    /// that owns it and, on a takeover, the one it displaced.
    fn publish_cluster_change(
        &self,
        wallet: &str,
        cluster_id: &str,
        realm: &str,
        session: &ClusterSession,
    );

    /// The full cluster topology for a completed pass. `active_peers` is the snapshot board's
    /// active-peer count, not the clustered count: it is what the `engine.discovery` heartbeat
    /// advertises as service load, and a peer with no realm or no live wallet binding is carried
    /// by this server while belonging to no cluster.
    fn publish_topology(&self, pass: &ClusterPass, active_peers: u32);

    /// Commits a completed pass before deriving broker hints from its complete recovery plans.
    fn replace_assignments(
        &self,
        pass: u64,
        valid_for: Duration,
        assignments: Vec<CurrentAssignment>,
    );
}

/// Stats-only mode: the tracker runs and reports metrics, nothing reaches a broker.
pub struct NoopClusterFeedPublisher;

impl ClusterFeedPublisher for NoopClusterFeedPublisher {
    fn publish_cluster_change(&self, _: &str, _: &str, _: &str, _: &ClusterSession) {}

    fn publish_topology(&self, _: &ClusterPass, _: u32) {}

    fn replace_assignments(&self, _: u64, _: Duration, _: Vec<CurrentAssignment>) {}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedChange {
    pub wallet: String,
    pub cluster_id: String,
    pub realm: String,
    pub session: ClusterSession,
}

/// In-process fake: records every hand-off with zero network I/O, so the tracker's publish
/// decisions can be asserted without a broker.
#[derive(Default)]
pub struct RecordingClusterFeed {
    changes: Mutex<Vec<RecordedChange>>,
    topologies: Mutex<Vec<Vec<(String, usize)>>>,
    active_peers: Mutex<Vec<u32>>,
    current: CurrentAssignments,
    control: Mutex<ControlSnapshot>,
}

impl RecordingClusterFeed {
    pub fn changes(&self) -> Vec<RecordedChange> {
        self.changes.lock().unwrap().clone()
    }

    pub fn topologies(&self) -> Vec<Vec<(String, usize)>> {
        self.topologies.lock().unwrap().clone()
    }

    pub fn active_peers(&self) -> Vec<u32> {
        self.active_peers.lock().unwrap().clone()
    }

    pub fn current(&self, wallet: &str, session: &str) -> Option<PeerClusterSnapshot> {
        self.current.lookup(wallet, session)
    }

    pub fn clear(&self) {
        self.changes.lock().unwrap().clear();
        self.topologies.lock().unwrap().clear();
        self.active_peers.lock().unwrap().clear();
    }

    pub fn set_control_snapshot(&self, snapshot: ControlSnapshot) {
        *self.control.lock().unwrap() = snapshot;
    }
}

impl ClusterFeedPublisher for RecordingClusterFeed {
    fn control_snapshot(&self) -> ControlSnapshot {
        self.control.lock().unwrap().clone()
    }
    fn replace_assignments(
        &self,
        pass: u64,
        valid_for: Duration,
        assignments: Vec<CurrentAssignment>,
    ) {
        let _ = self.current.replace(pass, valid_for, assignments);
    }

    fn publish_cluster_change(
        &self,
        wallet: &str,
        cluster_id: &str,
        realm: &str,
        session: &ClusterSession,
    ) {
        self.changes.lock().unwrap().push(RecordedChange {
            wallet: wallet.to_string(),
            cluster_id: cluster_id.to_string(),
            realm: realm.to_string(),
            session: session.clone(),
        });
    }

    fn publish_topology(&self, pass: &ClusterPass, active_peers: u32) {
        self.active_peers.lock().unwrap().push(active_peers);
        self.topologies.lock().unwrap().push(
            pass.clusters
                .iter()
                .map(|c| (c.id.clone(), c.count))
                .collect(),
        );
    }
}

/// What a `publish_cluster_change` hand-off puts on the bus. An absent displaced session or
/// cluster is the proto3 empty string.
pub fn cluster_change(
    wallet: &str,
    cluster_id: &str,
    realm: &str,
    session: &ClusterSession,
) -> ClusterChange {
    ClusterChange {
        wallet: wallet.to_string(),
        change: PeerClusterChange {
            cluster_id: cluster_id.to_string(),
            realm: realm.to_string(),
            session: session.session.clone(),
            displaced_session: session.displaced_session.clone().unwrap_or_default(),
            displaced_cluster_id: session.displaced_cluster_id.clone().unwrap_or_default(),
            // The committed ledger fills recovery and assignment identity below;
            // this projection supplies only the completed pass's topology hint.
            pending_takeovers: Vec::new(),
            token_not_before: None,
            assignment_version: 0,
            source_incarnation: String::new(),
        },
    }
}

/// Projects a pass onto archipelago's `IslandStatusMessage`, the shape archipelago-core published.
pub fn encode_topology(pass: &ClusterPass) -> Result<Vec<u8>, crate::protocol::bus::BusError> {
    let mut by_id: HashMap<&str, usize> = HashMap::new();
    let mut data: Vec<IslandData> = Vec::with_capacity(pass.clusters.len());
    for cluster in &pass.clusters {
        by_id.insert(cluster.id.as_str(), data.len());
        data.push(IslandData {
            id: cluster.id.clone(),
            peers: Vec::new(),
            max_peers: NO_PEER_CAP,
            center: Some(Position {
                x: cluster.centroid.x,
                y: cluster.centroid.y,
                z: cluster.centroid.z,
            }),
            radius: cluster.radius as f64,
        });
    }
    for peer in &pass.peers {
        if let Some(&index) = by_id.get(peer.cluster_id.as_str()) {
            data[index].peers.push(peer.wallet.clone());
        }
    }
    crate::protocol::bus::topology::encode_islands(&IslandStatusMessage { data })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueOutcome {
    /// Admitted as a new subject; nothing was lost.
    Admitted,
    /// Replaced an undelivered message for the same subject. Harmless -- the replacement carries
    /// strictly fresher state.
    Superseded,
    /// Admitted, and the longest-admitted pending subject was evicted to make room. The only path
    /// to genuine loss the outbox itself has.
    Dropped,
}

/// The outbox keeps the two feeds apart because they supersede differently. `engine.islands` is a
/// whole-world snapshot, so it lives in a single latest-wins slot and never competes with an
/// assignment. `peer.{addr}.cluster_change` supersedes per peer only, so changes are held one per
/// peer: a peer's newer assignment replaces its own older one, and one peer's event never
/// displaces another's -- which a shared FIFO with oldest-first eviction could not guarantee.
pub struct ClusterOutbox {
    capacity: usize,
    pending_change_by_subject: HashMap<String, Vec<u8>>,
    /// Admission order, not waiting order: a supersede leaves it untouched, so the evicted subject
    /// may be holding a message written a moment ago.
    change_order: VecDeque<String>,
    pending_topology: Option<(tokio::time::Instant, Vec<u8>)>,
}

impl ClusterOutbox {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            pending_change_by_subject: HashMap::new(),
            change_order: VecDeque::new(),
            pending_topology: None,
        }
    }

    pub fn queue_change(&mut self, subject: String, payload: Vec<u8>) -> QueueOutcome {
        if let Some(pending) = self.pending_change_by_subject.get_mut(&subject) {
            *pending = payload;
            return QueueOutcome::Superseded;
        }

        let mut dropped = false;
        while self.pending_change_by_subject.len() >= self.capacity {
            let Some(evicted) = self.change_order.pop_front() else {
                break;
            };
            if self.pending_change_by_subject.remove(&evicted).is_some() {
                dropped = true;
            }
        }

        self.pending_change_by_subject
            .insert(subject.clone(), payload);
        self.change_order.push_back(subject);

        if dropped {
            QueueOutcome::Dropped
        } else {
            QueueOutcome::Admitted
        }
    }

    /// Retained retry traffic cannot evict a fresh subject. Check and enqueue
    /// while the caller holds the same outbox lock used by fresh producers.
    /// Same-subject replacement still works even with no free slots.
    pub(crate) fn try_queue_retry(
        &mut self,
        subject: String,
        payload: Vec<u8>,
    ) -> Option<QueueOutcome> {
        if !self.pending_change_by_subject.contains_key(&subject)
            && self.pending_change_by_subject.len() >= self.capacity
        {
            return None;
        }
        Some(self.queue_change(subject, payload))
    }

    /// Latest wins: an undelivered snapshot is worthless once a newer one exists. Held apart from
    /// the changes, so it never counts against capacity.
    pub fn queue_topology(&mut self, payload: Vec<u8>) -> QueueOutcome {
        let superseded = self.pending_topology.is_some();
        self.pending_topology = Some((tokio::time::Instant::now(), payload));
        if superseded {
            QueueOutcome::Superseded
        } else {
            QueueOutcome::Admitted
        }
    }

    /// A pending topology snapshot ahead of any pending change, then changes in admission order.
    /// That priority is the whole of it and not an ordering between the two feeds: a snapshot
    /// admitted after a change preempts it, and one superseded before the drain gets here is never
    /// handed out at all.
    pub fn try_dequeue_next(&mut self) -> Option<(String, Vec<u8>)> {
        self.try_dequeue_next_timed()
            .map(|(subject, payload, _)| (subject, payload))
    }

    pub fn try_dequeue_next_timed(&mut self) -> Option<(String, Vec<u8>, tokio::time::Instant)> {
        if let Some((enqueued_at, topology)) = self.pending_topology.take()
            && enqueued_at.elapsed() < crate::topology::TOPOLOGY_RECEIPT_TTL
        {
            return Some((
                ISLANDS_SUBJECT.to_string(),
                topology,
                enqueued_at + crate::topology::TOPOLOGY_RECEIPT_TTL,
            ));
        }
        while let Some(subject) = self.change_order.pop_front() {
            if let Some(payload) = self.pending_change_by_subject.remove(&subject) {
                return Some((
                    subject,
                    payload,
                    tokio::time::Instant::now() + crate::topology::TOPOLOGY_RECEIPT_TTL,
                ));
            }
        }
        None
    }

    pub(crate) fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn pending_changes(&self) -> usize {
        self.pending_change_by_subject.len()
    }

    pub fn has_topology(&self) -> bool {
        self.pending_topology.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message as _;

    #[tokio::test(start_paused = true)]
    async fn pending_topology_expires_and_supersession_gets_its_own_deadline() {
        let mut queue = ClusterOutbox::new(4);
        queue.queue_topology(vec![1]);
        tokio::time::advance(crate::topology::TOPOLOGY_RECEIPT_TTL).await;
        assert!(queue.try_dequeue_next().is_none());
        queue.queue_topology(vec![2]);
        tokio::time::advance(Duration::from_secs(4)).await;
        queue.queue_topology(vec![3]);
        let (_, payload, deadline) = queue.try_dequeue_next_timed().unwrap();
        assert_eq!(payload, vec![3]);
        assert_eq!(
            deadline - tokio::time::Instant::now(),
            crate::topology::TOPOLOGY_RECEIPT_TTL
        );
    }

    fn payload(n: u8) -> Vec<u8> {
        vec![n]
    }

    #[test]
    fn a_fresher_change_for_one_peer_supersedes_its_own_older_one() {
        let mut outbox = ClusterOutbox::new(8);
        assert_eq!(
            outbox.queue_change("peer.a.cluster_change".into(), payload(1)),
            QueueOutcome::Admitted
        );
        assert_eq!(
            outbox.queue_change("peer.a.cluster_change".into(), payload(2)),
            QueueOutcome::Superseded
        );
        assert_eq!(outbox.pending_changes(), 1);
        assert_eq!(
            outbox.try_dequeue_next(),
            Some(("peer.a.cluster_change".to_string(), payload(2)))
        );
        assert_eq!(outbox.try_dequeue_next(), None);
    }

    #[test]
    fn retries_preserve_fresh_subjects_but_can_supersede_and_fill_free_slots() {
        let mut outbox = ClusterOutbox::new(1024);
        for n in 0..1024 {
            outbox.queue_change(format!("fresh-{n}"), vec![1]);
        }
        for n in 0..1024 {
            assert_eq!(outbox.try_queue_retry(format!("retry-{n}"), vec![2]), None);
        }
        assert_eq!(
            outbox.try_queue_retry("fresh-1023".into(), vec![3]),
            Some(QueueOutcome::Superseded)
        );
        for n in 0..1024 {
            assert_eq!(
                outbox.try_dequeue_next(),
                Some((
                    format!("fresh-{n}"),
                    if n == 1023 { vec![3] } else { vec![1] }
                ))
            );
        }
        assert_eq!(
            outbox.try_queue_retry("retry".into(), vec![2]),
            Some(QueueOutcome::Admitted)
        );
        assert_eq!(outbox.try_dequeue_next(), Some(("retry".into(), vec![2])));
        assert_eq!(
            ClusterOutbox::new(0).try_queue_retry("none".into(), vec![]),
            None
        );
    }

    #[test]
    fn retry_admission_serializes_with_a_fresh_producer() {
        let outbox = std::sync::Arc::new(Mutex::new(ClusterOutbox::new(2)));
        outbox.lock().unwrap().queue_change("first".into(), vec![1]);
        let producer = outbox.clone();
        std::thread::spawn(move || {
            producer
                .lock()
                .unwrap()
                .queue_change("fresh".into(), vec![2]);
        })
        .join()
        .unwrap();
        let mut queue = outbox.lock().unwrap();
        assert_eq!(queue.try_queue_retry("retained".into(), vec![3]), None);
        assert_eq!(queue.try_dequeue_next(), Some(("first".into(), vec![1])));
        assert_eq!(
            queue.try_queue_retry("retained".into(), vec![3]),
            Some(QueueOutcome::Admitted)
        );
        assert_eq!(queue.try_dequeue_next(), Some(("fresh".into(), vec![2])));
        assert_eq!(queue.try_dequeue_next(), Some(("retained".into(), vec![3])));
    }

    #[test]
    fn past_capacity_the_longest_admitted_subject_is_evicted() {
        let mut outbox = ClusterOutbox::new(2);
        outbox.queue_change("peer.a.cluster_change".into(), payload(1));
        outbox.queue_change("peer.b.cluster_change".into(), payload(2));
        outbox.queue_change("peer.a.cluster_change".into(), payload(3));
        assert_eq!(
            outbox.queue_change("peer.c.cluster_change".into(), payload(4)),
            QueueOutcome::Dropped
        );
        let drained: Vec<String> = std::iter::from_fn(|| outbox.try_dequeue_next())
            .map(|(s, _)| s)
            .collect();
        assert_eq!(
            drained,
            vec![
                "peer.b.cluster_change".to_string(),
                "peer.c.cluster_change".to_string()
            ]
        );
    }

    #[test]
    fn the_topology_slot_is_latest_wins_and_never_counts_against_capacity() {
        let mut outbox = ClusterOutbox::new(1);
        assert_eq!(outbox.queue_topology(payload(1)), QueueOutcome::Admitted);
        assert_eq!(outbox.queue_topology(payload(2)), QueueOutcome::Superseded);
        assert_eq!(
            outbox.queue_change("peer.a.cluster_change".into(), payload(9)),
            QueueOutcome::Admitted
        );
        assert!(outbox.has_topology());
        assert_eq!(outbox.pending_changes(), 1);
    }

    #[test]
    fn a_pending_topology_dequeues_ahead_of_a_change_admitted_before_it() {
        let mut outbox = ClusterOutbox::new(8);
        outbox.queue_change("peer.a.cluster_change".into(), payload(1));
        outbox.queue_topology(payload(2));
        assert_eq!(
            outbox.try_dequeue_next(),
            Some((ISLANDS_SUBJECT.to_string(), payload(2)))
        );
        assert_eq!(
            outbox.try_dequeue_next(),
            Some(("peer.a.cluster_change".to_string(), payload(1)))
        );
    }

    #[test]
    fn the_subject_lower_cases_the_wallet_once() {
        let session = ClusterSession::new("0xsession".to_string());
        assert_eq!(
            cluster_change("0xAbCd", "C1", "realm", &session).subject(),
            "peer.0xabcd.cluster_change"
        );
    }

    #[test]
    fn an_absent_displaced_session_encodes_as_the_proto3_empty_string() {
        let bytes = cluster_change(
            "0xwallet",
            "C1",
            "realm",
            &ClusterSession::new("0xsession".to_string()),
        )
        .encode();
        let decoded = PeerClusterChange::decode(bytes.as_slice()).unwrap();
        assert_eq!(decoded.cluster_id, "C1");
        assert_eq!(decoded.realm, "realm");
        assert_eq!(decoded.session, "0xsession");
        assert_eq!(decoded.displaced_session, "");
        assert_eq!(decoded.displaced_cluster_id, "");
    }

    #[test]
    fn a_takeover_names_the_displaced_session_and_cluster() {
        let session = ClusterSession {
            session: "0xnew".to_string(),
            displaced_session: Some("0xold".to_string()),
            displaced_cluster_id: Some("C7".to_string()),
        };
        let bytes = cluster_change("0xwallet", "C9", "realm", &session).encode();
        let decoded = PeerClusterChange::decode(bytes.as_slice()).unwrap();
        assert_eq!(decoded.session, "0xnew");
        assert_eq!(decoded.displaced_session, "0xold");
        assert_eq!(decoded.displaced_cluster_id, "C7");
    }

    /// The projection onto the islands snapshot, recorded before the subjects and decoders moved
    /// to `protocol::bus::topology`: one island with its centre, radius and two members.
    #[test]
    fn the_topology_projection_bytes_are_pinned() {
        use crate::protocol::pulse::decentraland::common::Vector3;
        use crate::pulse::cluster::{ClusterInfo, ClusterPeerInfo};

        let at = Vector3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        let mut pass = ClusterPass::default();
        pass.clusters.push(ClusterInfo {
            id: "island-C1".into(),
            realm: "r".into(),
            count: 2,
            centroid: at,
            radius: 8.0,
        });
        for wallet in ["0xa", "0xb"] {
            pass.peers.push(ClusterPeerInfo {
                peer: 0,
                wallet: wallet.into(),
                cluster_id: "island-C1".into(),
                realm: "r".into(),
                position: at,
                parcel: 0,
            });
        }
        assert_eq!(
            hex::encode(encode_topology(&pass).unwrap()),
            concat!(
                "0a2f0a0969736c616e642d433112033078611203307862220f0d0000803f15000000",
                "401d00004040290000000000002040"
            )
        );
    }
}

#[cfg(test)]
mod current_tests {
    use super::*;
    use crate::protocol::pulse::decentraland::pulse::TakeoverCompleted;

    #[tokio::test]
    async fn bounded_retry_batches_rotate_across_all_departed_pending_wallets() {
        let current = CurrentAssignments::default();
        let assignments = |session: &str| {
            (0..3)
                .map(|n| {
                    let mut a = assignment(session, 1);
                    a.wallet = format!("w{n}");
                    a
                })
                .collect()
        };
        current.replace(1, Duration::from_secs(2), assignments("a"));
        current.replace(2, Duration::from_secs(2), assignments("b"));
        current.replace(3, Duration::from_secs(2), vec![]);
        assert!(current.retained_hints(0).is_empty());
        let mut hinted = std::collections::HashSet::new();
        for _ in 0..3 {
            let batch = current.retained_hints(1);
            assert_eq!(batch.len(), 1);
            hinted.insert(batch[0].wallet.clone());
        }
        assert_eq!(
            hinted.len(),
            3,
            "a capacity-one outbox still reaches every retained wallet"
        );
    }

    fn cohort(start: usize, count: usize, session: &str) -> Vec<CurrentAssignment> {
        (start..start + count)
            .map(|n| {
                let mut a = assignment(session, 1);
                a.wallet = format!("w{n:06}");
                a
            })
            .collect()
    }

    #[tokio::test]
    async fn configured_transport_and_control_capacity_plus_one_churn_cohort_fit() {
        let current = CurrentAssignments::default();
        let simultaneous = crate::pulse::cluster::control::SUPPORTED_TRANSPORT_PEERS
            + crate::pulse::cluster::control::DEFAULT_CONTROL_POSITION_CAPACITY;
        assert_eq!(simultaneous, 4095 + 4096 + 16382);
        current.replace(1, Duration::from_secs(2), cohort(0, simultaneous, "a"));
        assert_eq!(current.view.lock().unwrap().records.len(), simultaneous);
        current.replace(
            2,
            Duration::from_secs(2),
            cohort(simultaneous, simultaneous, "b"),
        );
        assert_eq!(current.view.lock().unwrap().records.len(), simultaneous * 2);
        for n in [0, simultaneous - 1] {
            let wallet = format!("w{n:06}");
            let record = &current.view.lock().unwrap().records[&wallet];
            assert!(
                record.potential.is_some(),
                "churn cannot forget potentially admitted predecessors"
            );
        }
        // An old owner returning inside retention still requires its predecessor cleanup.
        let mut returning = cohort(0, 1, "c");
        returning.extend(cohort(simultaneous, simultaneous, "b"));
        current.replace(3, Duration::from_secs(2), returning);
        let returning = current.lookup("w000000", "c").unwrap();
        assert_eq!(returning.pending_takeovers.len(), 1);
        assert_eq!(returning.pending_takeovers[0].displaced_session, "a");
    }

    #[tokio::test]
    async fn safely_expired_departures_release_capacity_before_any_new_wallet_admission() {
        let current = CurrentAssignments::default();
        current.replace(1, Duration::from_secs(2), cohort(0, RECOVERY_WALLETS, "a"));
        let mut early = assignment("new", 1);
        early.wallet = "a-before-old-wallets".into();
        current.replace(RETENTION_PASSES + 2, Duration::from_secs(2), vec![early]);
        assert!(current.lookup("a-before-old-wallets", "new").is_some());
        assert_eq!(current.view.lock().unwrap().records.len(), 1);
    }

    #[tokio::test]
    async fn inactive_pending_blocked_and_unexpired_floors_survive_reclamation() {
        let current = CurrentAssignments::default();
        current.replace(1, Duration::from_secs(2), cohort(0, 4, "a"));
        let floor = unix_seconds() + FLOOR_GRACE_SECONDS;
        {
            let mut view = current.view.lock().unwrap();
            view.records.get_mut("w000000").unwrap().blocked = true;
            let pending = view.records.get_mut("w000001").unwrap();
            pending.pending.push((
                crate::protocol::pulse::decentraland::pulse::PendingTakeover {
                    takeover_id: "pending".into(),
                    displaced_session: "a".into(),
                    displaced_cluster_id: "room".into(),
                    displaced_realm: "realm".into(),
                },
                0,
            ));
            view.records.get_mut("w000002").unwrap().floor = floor;
        }
        current.replace(RETENTION_PASSES + 2, Duration::from_secs(2), vec![]);
        let view = current.view.lock().unwrap();
        assert_eq!(view.records.len(), 3);
        assert!(view.records["w000000"].blocked);
        assert_eq!(view.records["w000001"].pending.len(), 1);
        assert_eq!(view.records["w000002"].floor, floor);
    }

    #[tokio::test]
    async fn refusing_unconsumable_completion_preserves_pending_floor_and_version() {
        let current = CurrentAssignments::default();
        pass(&current, 1, "a");
        let before = pass(&current, 2, "b");
        assert!(
            current
                .complete(
                    "wallet",
                    &TakeoverCompleted {
                        takeover_id: before.pending_takeovers[0].takeover_id.clone(),
                        revoke_before: crate::protocol::bus::cluster::MAX_CONSUMER_SECONDS + 1,
                    }
                )
                .is_none()
        );
        assert_eq!(current.lookup("wallet", "b").unwrap(), before);
    }

    #[tokio::test]
    async fn room_capacity_blocks_without_truncation_and_matching_completion_releases_space() {
        let current = CurrentAssignments::default();
        pass(&current, 1, "a");
        {
            let mut view = current.view.lock().unwrap();
            let record = view.records.get_mut("wallet").unwrap();
            for n in 0..RECOVERY_ROOMS {
                record.pending.push((
                    crate::protocol::pulse::decentraland::pulse::PendingTakeover {
                        takeover_id: format!("old-{n}"),
                        displaced_session: "old".into(),
                        displaced_cluster_id: format!("room-{n}"),
                        displaced_realm: "realm".into(),
                    },
                    0,
                ));
            }
        }
        let blocked = pass(&current, 2, "b");
        assert_eq!(blocked.pending_takeovers.len(), 32);
        assert_eq!(blocked.token_not_before, None);
        current.complete(
            "wallet",
            &TakeoverCompleted {
                takeover_id: "old-0".into(),
                revoke_before: 100,
            },
        );
        let resumed = pass(&current, 3, "b");
        assert_eq!(resumed.pending_takeovers.len(), 32);
        assert_eq!(resumed.token_not_before, Some(100));
        assert!(
            resumed
                .pending_takeovers
                .iter()
                .any(|p| p.displaced_session == "a")
        );
        assert!(
            resumed
                .pending_takeovers
                .iter()
                .any(|p| p.takeover_id == "old-31")
        );
    }

    #[tokio::test]
    async fn pulse_accepts_consumer_maximum_future_cutoff_without_adding_clock_skew_policy() {
        let current = CurrentAssignments::default();
        pass(&current, 1, "a");
        let b = pass(&current, 2, "b");
        let ready = current
            .complete(
                "wallet",
                &TakeoverCompleted {
                    takeover_id: b.pending_takeovers[0].takeover_id.clone(),
                    revoke_before: crate::protocol::bus::cluster::MAX_CONSUMER_SECONDS,
                },
            )
            .unwrap();
        assert_eq!(
            ready.token_not_before,
            Some(crate::protocol::bus::cluster::MAX_CONSUMER_SECONDS)
        );
        current.replace(3, Duration::from_secs(2), vec![]);
        current.replace(1000, Duration::from_secs(2), vec![]);
        assert_eq!(
            current.view.lock().unwrap().records.len(),
            1,
            "confirmed floor clock grace also governs retirement"
        );
    }

    fn assignment(session: &str, epoch: u64) -> CurrentAssignment {
        CurrentAssignment {
            wallet: "wallet".into(),
            session: session.into(),
            realm: "realm".into(),
            cluster_id: "room".into(),
            owner_epoch: epoch,
            owner_peer: 0,
            registration: None,
        }
    }

    #[tokio::test]
    async fn same_slot_same_strings_registration_replacement_invalidates_before_next_pass() {
        let current = CurrentAssignments::default();
        let mut identity = crate::pulse::snapshot::IdentityBoard::new(1);
        identity.set_with_session(0, "wallet".into(), "a".into());
        let mut a = assignment("a", identity.registration_by_peer(0));
        a.registration = Some(identity.registration_guard(0));
        current.replace(1, Duration::from_secs(2), vec![a.clone()]);
        let before = current.lookup("wallet", "a").unwrap();
        identity.remove(0);
        assert!(current.lookup("wallet", "a").is_none());
        identity.set_with_session(0, "wallet".into(), "a".into());
        assert!(current.lookup("wallet", "a").is_none());
        a.owner_epoch = identity.registration_by_peer(0);
        current.replace(2, Duration::from_secs(2), vec![a]);
        assert!(
            current.lookup("wallet", "a").unwrap().assignment_version > before.assignment_version
        );
    }

    #[tokio::test]
    async fn departed_pending_identity_keeps_floor_and_releases_without_ever_becoming_ready() {
        let current = CurrentAssignments::default();
        pass(&current, 1, "a");
        let b = pass(&current, 2, "b");
        current.replace(1000, Duration::from_secs(2), vec![]);
        current.replace(2000, Duration::from_secs(2), vec![]);
        let inactive = current.lookup("wallet", "b").unwrap();
        assert_eq!(inactive.token_not_before, Some(0));
        assert_eq!(inactive.pending_takeovers, b.pending_takeovers);
        assert!(current.lookup("wallet", "a").is_none());
        assert_eq!(current.retained_hints(1).len(), 1);
        assert!(
            current
                .complete(
                    "wallet",
                    &TakeoverCompleted {
                        takeover_id: b.pending_takeovers[0].takeover_id.clone(),
                        revoke_before: 100
                    }
                )
                .is_none()
        );
        assert!(current.lookup("wallet", "b").is_none());
        assert!(current.retained_hints(1).is_empty());
        current.replace(3000, Duration::from_secs(2), vec![]);
        assert!(current.view.lock().unwrap().records.is_empty());
    }

    fn pass(current: &CurrentAssignments, n: u64, session: &str) -> PeerClusterSnapshot {
        current.replace(n, Duration::from_secs(2), vec![assignment(session, n)]);
        current.lookup("wallet", session).unwrap()
    }

    #[tokio::test]
    async fn ready_pass_exposure_retains_a_but_never_blocked_b_in_a_b_c() {
        let current = CurrentAssignments::default();
        current.replace(1, Duration::from_secs(2), vec![assignment("a", 1)]);
        // A is potentially admitted at completed-pass Snapshot, even without a lookup.
        let b = pass(&current, 2, "b");
        let c = pass(&current, 3, "c");
        assert_eq!(b.pending_takeovers, c.pending_takeovers);
        assert_eq!(c.pending_takeovers.len(), 1);
        assert_eq!(c.pending_takeovers[0].displaced_session, "a");
        let id = c.pending_takeovers[0].takeover_id.clone();
        let ready = current
            .complete(
                "wallet",
                &TakeoverCompleted {
                    takeover_id: id.clone(),
                    revoke_before: 100,
                },
            )
            .unwrap();
        assert!(ready.pending_takeovers.is_empty());
        assert_eq!(ready.token_not_before, Some(100));
        assert!(
            current
                .complete(
                    "wallet",
                    &TakeoverCompleted {
                        takeover_id: id,
                        revoke_before: 999
                    }
                )
                .is_none()
        );
        assert_eq!(
            current.lookup("wallet", "c").unwrap().token_not_before,
            Some(100)
        );
        let d = pass(&current, 4, "d");
        assert_eq!(d.pending_takeovers[0].displaced_session, "c");
    }

    #[tokio::test(start_paused = true)]
    async fn unfinished_work_survives_staleness_absence_and_retention() {
        let current = CurrentAssignments::default();
        pass(&current, 1, "a");
        let b = pass(&current, 2, "b");
        tokio::time::advance(Duration::from_secs(50)).await;
        assert_eq!(
            current.lookup("wallet", "b").unwrap().pending_takeovers,
            b.pending_takeovers
        );
        current.replace(1000, Duration::from_secs(2), vec![]);
        let c = pass(&current, 2000, "c");
        assert_eq!(b.pending_takeovers, c.pending_takeovers);
    }

    #[tokio::test]
    async fn refresh_and_same_session_moves_preserve_no_cleanup_but_invalidate_identity() {
        let current = CurrentAssignments::default();
        let a = assignment("a", 1);
        current.replace(1, Duration::from_secs(2), vec![a.clone()]);
        let first = current.lookup("wallet", "a").unwrap();
        current.replace(2, Duration::from_secs(2), vec![a.clone()]);
        assert_eq!(
            current.lookup("wallet", "a").unwrap().assignment_version,
            first.assignment_version
        );
        let mut moved = a.clone();
        moved.cluster_id = "next".into();
        current.replace(3, Duration::from_secs(2), vec![moved]);
        let second = current.lookup("wallet", "a").unwrap();
        assert!(second.assignment_version > first.assignment_version);
        assert!(second.pending_takeovers.is_empty());
        current.replace(4, Duration::from_secs(2), vec![a]);
        assert!(
            current.lookup("wallet", "a").unwrap().assignment_version > second.assignment_version
        );
        let mut registration = assignment("a", 2);
        registration.owner_peer = 8;
        current.replace(5, Duration::from_secs(2), vec![registration]);
        assert!(
            current.lookup("wallet", "a").unwrap().assignment_version > second.assignment_version
        );
    }

    #[tokio::test]
    async fn expiry_never_reuses_version_and_checked_wrap_never_admits() {
        let current = CurrentAssignments::default();
        let first = pass(&current, 1, "a");
        current.replace(2, Duration::from_secs(2), vec![]);
        current.replace(1000, Duration::from_secs(2), vec![]);
        let next = pass(&current, 1001, "a");
        assert!(next.assignment_version > first.assignment_version);
        current.view.lock().unwrap().next_version = u64::MAX;
        current.replace(1002, Duration::from_secs(2), vec![assignment("b", 3)]);
        assert!(current.lookup("wallet", "b").is_none());
        assert_eq!(current.view.lock().unwrap().next_version, u64::MAX);
    }

    #[tokio::test]
    async fn completions_validate_operation_prior_floor_and_do_not_advance_unknown_ids() {
        let current = CurrentAssignments::default();
        pass(&current, 1, "a");
        let b = pass(&current, 2, "b");
        let id = b.pending_takeovers[0].takeover_id.clone();
        for cutoff in [0, crate::protocol::bus::cluster::MAX_CONSUMER_SECONDS + 1] {
            assert!(
                current
                    .complete(
                        "wallet",
                        &TakeoverCompleted {
                            takeover_id: id.clone(),
                            revoke_before: cutoff
                        }
                    )
                    .is_none()
            );
        }
        // Inject a second independent retained operation to check out-of-order aggregate handling.
        {
            let mut view = current.view.lock().unwrap();
            let record = view.records.get_mut("wallet").unwrap();
            let mut other = record.pending[0].0.clone();
            other.takeover_id = "independent".into();
            record.pending.push((other, 0));
        }
        current
            .complete(
                "wallet",
                &TakeoverCompleted {
                    takeover_id: id,
                    revoke_before: 100,
                },
            )
            .unwrap();
        let ready = current
            .complete(
                "wallet",
                &TakeoverCompleted {
                    takeover_id: "independent".into(),
                    revoke_before: 90,
                },
            )
            .unwrap();
        assert_eq!(ready.token_not_before, Some(100));
        let c = pass(&current, 3, "c");
        assert!(
            current
                .complete(
                    "wallet",
                    &TakeoverCompleted {
                        takeover_id: c.pending_takeovers[0].takeover_id.clone(),
                        revoke_before: 100
                    }
                )
                .is_none()
        );
        assert!(
            current
                .complete(
                    "wallet",
                    &TakeoverCompleted {
                        takeover_id: "stale-incarnation".into(),
                        revoke_before: 999
                    }
                )
                .is_none()
        );
    }

    #[tokio::test]
    async fn saturation_never_truncates_or_evicts_unfinished_plans() {
        let current = CurrentAssignments::default();
        let mut assignments = Vec::new();
        for i in 0..RECOVERY_WALLETS {
            let mut a = assignment("a", 1);
            a.wallet = format!("w{i:06}");
            assignments.push(a);
        }
        current.replace(1, Duration::from_secs(2), assignments.clone());
        let mut extra = assignment("extra", 1);
        extra.wallet = "excess".into();
        assignments.push(extra);
        let recorded = crate::pulse::metrics::render_recorded(|| {
            current.replace(2, Duration::from_secs(2), assignments);
        });
        assert!(recorded.contains("pulse_recovery_refused_total 1"));
        assert!(recorded.contains(&format!("pulse_recovery_wallets {RECOVERY_WALLETS}")));
        assert!(current.lookup("excess", "extra").is_none());
        // Fill every retained record with pending work, depart, and age past the
        // ordinary inactive window. Exhaustion must still refuse without loss.
        current.replace(3, Duration::from_secs(2), cohort(0, RECOVERY_WALLETS, "b"));
        let mut refused = assignment("new", 1);
        refused.wallet = "excess".into();
        current.replace(1000, Duration::from_secs(2), vec![refused]);
        assert!(current.lookup("excess", "new").is_none());
        assert_eq!(current.view.lock().unwrap().records.len(), RECOVERY_WALLETS);
        assert!(
            current
                .view
                .lock()
                .unwrap()
                .records
                .values()
                .all(|r| r.pending.len() == 1)
        );
        let oversized = CurrentAssignments::default();
        let mut a = assignment("a", 1);
        a.realm = "r".repeat(65536);
        oversized.replace(1, Duration::from_secs(2), vec![a]);
        assert!(oversized.lookup("wallet", "a").is_none());
        assert!(
            oversized.view.lock().unwrap().records["wallet"]
                .potential
                .is_none()
        );
    }

    #[tokio::test]
    async fn ready_snapshot_carries_explicit_zero_floor_and_assignment_identity() {
        use prost::Message;
        let current = CurrentAssignments::default();
        current.replace(
            1,
            Duration::from_secs(2),
            vec![CurrentAssignment {
                wallet: "0xab".into(),
                session: "0xcd".into(),
                cluster_id: "C1".into(),
                realm: "realm".into(),
                owner_epoch: 0,
                owner_peer: 0,
                registration: None,
            }],
        );
        let bytes = current.lookup("0xab", "0xcd").unwrap().encode_to_vec();
        assert!(
            bytes.windows(2).any(|pair| pair == [0x40, 0]),
            "ready snapshot must encode present zero floor at tag 8"
        );
        assert!(
            bytes.windows(2).any(|pair| pair == [0x48, 1]),
            "first assignment has a positive version at tag 9"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_current_view_expires_at_its_deadline_and_is_replace_only() {
        let current = CurrentAssignments::default();
        let assignment = CurrentAssignment {
            wallet: "0xAB".into(),
            session: "0xCD".into(),
            cluster_id: "C1".into(),
            realm: "realm".into(),
            owner_epoch: 0,
            owner_peer: 0,
            registration: None,
        };
        current.replace(7, Duration::from_secs(2), vec![assignment.clone()]);
        assert_eq!(current.lookup("0xab", "0xcd").unwrap().pass, 7);
        assert!(current.lookup("0xab", "0xce").is_none());
        tokio::time::advance(Duration::from_millis(1999)).await;
        assert!(current.lookup("0xab", "0xcd").is_some());
        tokio::time::advance(Duration::from_millis(1)).await;
        assert!(current.lookup("0xab", "0xcd").is_none());
        current.replace(8, Duration::from_secs(2), vec![assignment]);
        assert_eq!(current.lookup("0xab", "0xcd").unwrap().pass, 8);
        current.replace(9, Duration::from_secs(2), vec![]);
        assert!(current.lookup("0xab", "0xcd").is_none());
    }
}
