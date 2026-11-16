//! Pulse-owned legacy statistics; never a source of admission or native membership.
use crate::protocol::bus::statistics::{
    Change, Event, Observation, Peer, ProducerGrant, ProducerReply, ProducerRequest, Snapshot,
    Update,
};
use std::collections::{BTreeMap, HashMap, VecDeque};

/// Exact-case wallet membership with Map insertion order and no expiry. Producer
/// cursors suppress duplicate/out-of-order observations, not disconnected sessions:
/// every accepted socket disconnect removes its wallet, regardless of other sockets.
/// A handover keeps the outgoing writer's cursor until the next one, so its sockets'
/// heartbeats and disconnects still count while it drains, as upstream Stats takes
/// every connector's events.
#[derive(Default)]
pub struct Statistics {
    peers: BTreeMap<u64, Peer>,
    order: HashMap<String, u64>,
    next: u64,
    incarnation: uuid::Uuid,
    next_generation: u64,
    active: Option<(ProducerGrant, u64)>,
    retiring: Option<(ProducerGrant, u64)>,
    pending: Option<ProducerGrant>,
    islands: Vec<u8>,
    discovery: Vec<u8>,
    revision: u64,
    changes: VecDeque<Update>,
    checkpoint: Option<Update>,
    pending_bytes: usize,
    recovery: bool,
}
impl Statistics {
    pub fn with_incarnation(incarnation: uuid::Uuid) -> Self {
        Self {
            incarnation,
            ..Self::default()
        }
    }
    pub fn control(&mut self, request: ProducerRequest) -> ProducerReply {
        let rejected = ProducerReply::Rejected {
            incarnation: self.incarnation,
        };
        match request {
            ProducerRequest::Status => ProducerReply::Status {
                incarnation: self.incarnation,
            },
            ProducerRequest::Begin {
                incarnation,
                producer,
            } => {
                if incarnation != self.incarnation {
                    return rejected;
                }
                if let Some((grant, _)) = &self.active
                    && grant.producer == producer
                {
                    return ProducerReply::Active(grant.clone());
                }
                if let Some(grant) = &self.pending
                    && grant.producer == producer
                {
                    return ProducerReply::Grant(grant.clone());
                }
                let Some(generation) = self.next_generation.checked_add(1) else {
                    return rejected;
                };
                self.next_generation = generation;
                let grant = ProducerGrant {
                    incarnation,
                    producer,
                    generation,
                    challenge: uuid::Uuid::new_v4(),
                };
                self.pending = Some(grant.clone());
                ProducerReply::Grant(grant)
            }
            ProducerRequest::Confirm(grant) => {
                if grant.incarnation != self.incarnation {
                    return rejected;
                }
                if self
                    .active
                    .as_ref()
                    .is_some_and(|(current, _)| *current == grant)
                {
                    return ProducerReply::Active(grant);
                }
                if self.pending.as_ref() != Some(&grant)
                    || self
                        .active
                        .as_ref()
                        .is_some_and(|(current, _)| current.generation >= grant.generation)
                {
                    return rejected;
                }
                self.pending = None;
                // Defensive: `Begin` answers `Active` to the producer that is already
                // active, so a pending grant never shares the active producer today.
                self.retiring = self
                    .active
                    .take()
                    .filter(|(outgoing, _)| outgoing.producer != grant.producer);
                self.active = Some((grant.clone(), 0));
                ProducerReply::Active(grant)
            }
        }
    }
    /// `received_ms` is Pulse's wall clock at receipt, including backwards jumps.
    /// Sequence gaps are accepted: the internal core NATS link is at most once.
    pub fn observe(&mut self, observation: Observation, received_ms: i64) -> bool {
        let cursor = if observation.version == 2
            && observation.sequence > 0
            && observation.incarnation == self.incarnation
        {
            [&mut self.active, &mut self.retiring]
                .into_iter()
                .flatten()
                .find(|(grant, _)| {
                    grant.producer == observation.producer
                        && grant.generation == observation.generation
                })
                .map(|(_, cursor)| cursor)
                .filter(|cursor| observation.sequence > **cursor)
        } else {
            None
        };
        let Some(cursor) = cursor else {
            crate::pulse::metrics::statistics_observation_rejected();
            return false;
        };
        *cursor = observation.sequence;
        match observation.event {
            Event::Heartbeat { address, position } => {
                let index = *self.order.entry(address.clone()).or_insert_with(|| {
                    let index = self.next;
                    self.next = self
                        .next
                        .checked_add(1)
                        .expect("statistics insertion counter exhausted");
                    index
                });
                let peer = Peer {
                    address,
                    position,
                    time: received_ms,
                };
                self.peers.insert(index, peer.clone());
                self.changed(Change::Peer(peer));
            }
            Event::Disconnect { address } => {
                if let Some(index) = self.order.remove(&address) {
                    self.peers.remove(&index);
                }
                self.changed(Change::Remove(address));
            }
        }
        true
    }
    fn changed(&mut self, change: Change) {
        self.revision = self
            .revision
            .checked_add(1)
            .expect("statistics revision exhausted");
        if self.recovery {
            return;
        }
        let bytes = change_bytes(&change);
        if self.changes.len() >= 4096 || self.pending_bytes.saturating_add(bytes) > 8 * 1024 * 1024
        {
            self.require_recovery();
            tracing::warn!("statistics change backlog exceeded; complete recovery required");
            return;
        }
        self.pending_bytes += bytes;
        self.changes.push_back(Update::Change {
            revision: self.revision,
            change,
        });
    }
    pub fn require_recovery(&mut self) {
        self.changes.clear();
        self.checkpoint = None;
        self.pending_bytes = 0;
        self.recovery = true;
    }
    pub fn next_update(&mut self, full: bool) -> Option<Update> {
        if self.recovery {
            self.recovery = false;
            return Some(Update::Snapshot {
                revision: self.revision,
                state: self.snapshot(),
            });
        }
        if full && self.checkpoint.is_none() {
            self.checkpoint = Some(Update::Snapshot {
                revision: self.revision,
                state: self.snapshot(),
            });
        }
        // Capture once, drain its finite prefix, then publish that exact state.
        // Later arrivals cannot replace or postpone an outstanding checkpoint.
        if let Some(checkpoint) = &self.checkpoint
            && self
                .changes
                .front()
                .is_none_or(|next| next.revision() > checkpoint.revision())
        {
            return self.checkpoint.take();
        }
        let update = self.changes.pop_front();
        if let Some(Update::Change { change, .. }) = &update {
            self.pending_bytes -= change_bytes(change);
        }
        update
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            peers: self.peers.values().cloned().collect(),
            islands: self.islands.clone(),
            discovery: self.discovery.clone(),
        }
    }
    pub fn topology(&mut self, bytes: Vec<u8>) {
        if self.islands != bytes {
            self.islands = bytes.clone();
            self.changed(Change::Islands(bytes));
        }
    }
    pub fn discovery(&mut self, bytes: Vec<u8>) {
        if self.discovery != bytes {
            self.discovery = bytes.clone();
            self.changed(Change::Discovery(bytes));
        }
    }
}
fn change_bytes(change: &Change) -> usize {
    match change {
        Change::Peer(peer) => peer.address.len() + 64,
        Change::Remove(address) => address.len(),
        Change::Islands(bytes) | Change::Discovery(bytes) => bytes.len(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::bus::statistics::Event;
    #[test]
    fn draining_changes_releases_bytes_while_the_backlog_stays_nonempty() {
        let mut stats = Statistics::default();
        let size = 1024 * 1024;
        stats.topology(vec![1; size]);
        stats.discovery(vec![2; size]);
        for value in 3..24 {
            assert!(matches!(
                stats.next_update(false),
                Some(Update::Change { .. })
            ));
            assert_eq!(stats.pending_bytes, size);
            stats.topology(vec![value; size]);
            assert!(
                !stats.recovery,
                "drained traffic must not exhaust the backlog budget"
            );
            assert_eq!(stats.pending_bytes, 2 * size);
        }
        while stats.next_update(false).is_some() {}
        assert_eq!(stats.pending_bytes, 0);
    }

    #[test]
    fn overflow_and_failed_transfer_discard_pending_checkpoint_for_complete_recovery() {
        for overflow in [false, true] {
            let mut stats = registered();
            stats.observe(beat(1, "B"), 100);
            stats.observe(beat(2, "a"), 101);
            assert!(matches!(
                stats.next_update(true),
                Some(Update::Change { revision: 1, .. })
            ));
            assert!(stats.checkpoint.is_some());
            if overflow {
                for sequence in 3..=4100 {
                    stats.observe(beat(sequence, "B"), sequence as i64);
                }
            } else {
                stats.require_recovery();
                stats.observe(beat(3, "B"), -7);
            }
            assert!(stats.checkpoint.is_none());
            assert!(stats.changes.is_empty());
            let Some(Update::Snapshot { revision, state }) = stats.next_update(false) else {
                panic!("interrupted checkpoint must be replaced by complete recovery")
            };
            assert_eq!(revision, if overflow { 4100 } else { 3 });
            assert_eq!(state.peers[0].time, if overflow { 4100 } else { -7 });
            assert_eq!(state.peers[1].address, "a");
            assert!(stats.next_update(false).is_none());
        }
    }
    #[test]
    fn producer_capacity_never_forgets_cursors_and_backlog_recovers_completely() {
        let rendered = crate::pulse::metrics::render_recorded(|| {
            let mut stats = Statistics::default();
            assert!(!stats.observe(beat(1, "no-active"), 0));
            register(&mut stats, uuid::Uuid::nil());
            let accepted = beat(1, "retained");
            assert!(stats.observe(accepted.clone(), 1));
            assert!(!stats.observe(accepted, 2));
            let mut stale = beat(2, "refused");
            stale.generation += 1;
            assert!(!stats.observe(stale, 2));
            let mut stale = beat(2, "refused");
            stale.incarnation = uuid::Uuid::new_v4();
            assert!(!stats.observe(stale, 2));
            let mut stale = beat(2, "refused");
            stale.producer = uuid::Uuid::new_v4();
            assert!(!stats.observe(stale, 2));
            let mut stale = beat(2, "refused");
            stale.version = 1;
            assert!(!stats.observe(stale, 2));
            assert!(!stats.observe(beat(0, "refused"), 2));
            assert_eq!(stats.snapshot().peers.len(), 1);
        });
        assert_eq!(
            rendered
                .lines()
                .filter(|line| line.starts_with("pulse_statistics_observations_rejected_total"))
                .collect::<Vec<_>>(),
            ["pulse_statistics_observations_rejected_total 7"],
            "one counter, without producer or reason labels; accepted events do not increment it"
        );
        let mut stats = Statistics::default();
        let mut first = None;
        let mut current = None;
        for producer in 0..(crate::protocol::bus::statistics::MAX_PRODUCERS * 2 + 1) {
            let grant = register(&mut stats, uuid::Uuid::from_u128(producer as u128));
            first.get_or_insert_with(|| grant.clone());
            let mut event = beat(1, "shared");
            event.producer = grant.producer;
            event.generation = grant.generation;
            assert!(stats.observe(event, producer as i64));
            current = Some(grant);
        }
        let first = first.unwrap();
        let current = current.unwrap();
        assert_eq!(
            stats.control(ProducerRequest::Begin {
                incarnation: current.incarnation,
                producer: current.producer
            }),
            ProducerReply::Active(current.clone())
        );
        assert_eq!(
            stats.control(ProducerRequest::Confirm(current.clone())),
            ProducerReply::Active(current.clone())
        );
        let mut duplicate = beat(1, "duplicate");
        duplicate.producer = current.producer;
        duplicate.generation = current.generation;
        assert!(
            !stats.observe(duplicate, 0),
            "idempotent registration must preserve the active cursor"
        );
        assert!(matches!(
            stats.control(ProducerRequest::Confirm(first.clone())),
            ProducerReply::Rejected { .. }
        ));
        assert!(!stats.observe(beat(1, "shared"), 0));
        // A delayed registration creates only a candidate, never replacing the active cursor.
        let delayed = stats.control(ProducerRequest::Begin {
            incarnation: stats.incarnation,
            producer: first.producer,
        });
        let ProducerReply::Grant(delayed) = delayed else {
            panic!("delayed candidate")
        };
        let ProducerReply::Grant(newer) = stats.control(ProducerRequest::Begin {
            incarnation: current.incarnation,
            producer: uuid::Uuid::new_v4(),
        }) else {
            panic!("newer candidate")
        };
        assert!(
            matches!(
                stats.control(ProducerRequest::Confirm(delayed)),
                ProducerReply::Rejected { .. }
            ),
            "a superseded challenge cannot hand over the writer"
        );
        assert_eq!(stats.pending.as_ref(), Some(&newer));
        let mut next = beat(2, "continued");
        next.producer = current.producer;
        next.generation = current.generation;
        assert!(stats.observe(next, 77));
        assert!(stats.recovery);
        assert!(stats.changes.is_empty());
        let Update::Snapshot { revision, state } = stats.next_update(false).unwrap() else {
            panic!()
        };
        assert_eq!(
            revision,
            crate::protocol::bus::statistics::MAX_PRODUCERS as u64 * 2 + 2
        );
        assert_eq!(state.peers.len(), 2);
        assert_eq!(state.peers[1].time, 77);
        assert!(stats.next_update(false).is_none());
        assert_eq!(
            stats.control(ProducerRequest::Confirm(newer.clone())),
            ProducerReply::Active(newer.clone())
        );
        assert_eq!(
            stats.snapshot(),
            state,
            "writer handover cannot expire retained peers"
        );
        assert!(matches!(
            stats.control(ProducerRequest::Confirm(current.clone())),
            ProducerReply::Rejected { .. }
        ));
        let mut replayed = beat(2, "continued");
        replayed.producer = current.producer;
        replayed.generation = current.generation;
        assert!(
            !stats.observe(replayed, 998),
            "the outgoing writer keeps its cursor"
        );
        let mut draining = beat(999, "continued");
        draining.producer = current.producer;
        draining.generation = current.generation;
        assert!(
            stats.observe(draining, 999),
            "the outgoing writer's sockets still count while it drains"
        );
        register(&mut stats, uuid::Uuid::new_v4());
        let mut stale = beat(1000, "stale");
        stale.producer = current.producer;
        stale.generation = current.generation;
        assert!(
            !stats.observe(stale, 1000),
            "only the latest outgoing writer is kept"
        );
        let mut restarted = Statistics::with_incarnation(uuid::Uuid::new_v4());
        assert!(matches!(
            restarted.control(ProducerRequest::Confirm(current)),
            ProducerReply::Rejected { .. }
        ));
        assert!(matches!(
            restarted.control(ProducerRequest::Begin {
                incarnation: stats.incarnation,
                producer: first.producer
            }),
            ProducerReply::Rejected { .. }
        ));
        assert!(!restarted.observe(beat(99, "old-incarnation"), 100));
        let grant = register(&mut restarted, uuid::Uuid::new_v4());
        let mut fresh = beat(1, "new-incarnation");
        fresh.incarnation = grant.incarnation;
        fresh.generation = grant.generation;
        fresh.producer = grant.producer;
        assert!(restarted.observe(fresh, -100));
        restarted.next_generation = u64::MAX;
        assert!(
            matches!(
                restarted.control(ProducerRequest::Begin {
                    incarnation: grant.incarnation,
                    producer: uuid::Uuid::new_v4()
                }),
                ProducerReply::Rejected { .. }
            ),
            "generation overflow must fail closed rather than reuse a fence"
        );
    }
    #[test]
    fn a_handover_keeps_the_outgoing_connectors_heartbeats_and_disconnects() {
        let mut stats = registered();
        assert!(stats.observe(beat(1, "old-socket"), 1));
        let incoming = register(&mut stats, uuid::Uuid::from_u128(7));
        let mut moved = beat(1, "new-socket");
        moved.producer = incoming.producer;
        moved.generation = incoming.generation;
        assert!(stats.observe(moved, 2));
        // The outgoing connector drains: its socket still moves, then closes.
        assert!(stats.observe(beat(2, "old-socket"), 3));
        assert_eq!(stats.snapshot().peers[0].time, 3);
        assert!(stats.observe(
            observation(
                3,
                Event::Disconnect {
                    address: "old-socket".into()
                }
            ),
            4
        ));
        assert_eq!(
            stats
                .snapshot()
                .peers
                .iter()
                .map(|p| p.address.as_str())
                .collect::<Vec<_>>(),
            ["new-socket"],
            "a drained connector leaves no ghost"
        );
    }
    #[test]
    fn a_draining_disconnect_drops_a_reconnected_wallet_until_its_next_heartbeat() {
        let mut stats = registered();
        assert!(stats.observe(beat(1, "w"), 1));
        let incoming = register(&mut stats, uuid::Uuid::from_u128(7));
        let through_new = |sequence| {
            let mut beat = beat(sequence, "w");
            beat.producer = incoming.producer;
            beat.generation = incoming.generation;
            beat
        };
        assert!(stats.observe(through_new(1), 2));
        // The old socket closes after the wallet reconnected: as upstream, any
        // connector's disconnect removes the peer.
        let gone = Event::Disconnect {
            address: "w".into(),
        };
        assert!(stats.observe(observation(2, gone), 3));
        assert!(stats.snapshot().peers.is_empty());
        assert!(stats.observe(through_new(2), 4));
        assert_eq!(stats.snapshot().peers[0].address, "w");
    }
    #[test]
    fn only_the_most_recent_outgoing_connector_drains() {
        let mut stats = registered();
        assert!(stats.observe(beat(1, "a"), 1));
        register(&mut stats, uuid::Uuid::from_u128(7));
        register(&mut stats, uuid::Uuid::from_u128(8));
        let gone = Event::Disconnect {
            address: "a".into(),
        };
        assert!(!stats.observe(observation(2, gone), 2));
        assert_eq!(stats.snapshot().peers[0].address, "a");
    }
    fn register(stats: &mut Statistics, producer: uuid::Uuid) -> ProducerGrant {
        let ProducerReply::Grant(grant) = stats.control(ProducerRequest::Begin {
            incarnation: stats.incarnation,
            producer,
        }) else {
            panic!("candidate grant")
        };
        assert_eq!(
            stats.control(ProducerRequest::Confirm(grant.clone())),
            ProducerReply::Active(grant.clone())
        );
        grant
    }
    fn registered() -> Statistics {
        let mut stats = Statistics::default();
        register(&mut stats, uuid::Uuid::nil());
        stats
    }
    fn observation(sequence: u64, event: Event) -> Observation {
        Observation {
            version: 2,
            incarnation: uuid::Uuid::nil(),
            generation: 1,
            producer: uuid::Uuid::nil(),
            sequence,
            event,
        }
    }
    fn beat(sequence: u64, address: &str) -> Observation {
        observation(
            sequence,
            Event::Heartbeat {
                address: address.into(),
                position: [-16.01, 2.5, 16.0],
            },
        )
    }
    #[test]
    fn legacy_order_exact_ids_receipt_time_and_each_disconnect() {
        let mut stats = registered();
        assert!(stats.observe(beat(1, "B"), 100));
        assert!(stats.observe(beat(2, "a"), 101));
        assert!(stats.observe(beat(3, "B"), 90));
        let snapshot = stats.snapshot();
        assert_eq!(
            snapshot
                .peers
                .iter()
                .map(|p| p.address.as_str())
                .collect::<Vec<_>>(),
            ["B", "a"]
        );
        assert_eq!(
            snapshot.peers[0].time, 90,
            "receipt clocks can move backwards"
        );
        assert!(stats.observe(
            observation(
                4,
                Event::Disconnect {
                    address: "B".into()
                }
            ),
            102
        ));
        assert!(stats.observe(beat(5, "B"), 103));
        assert_eq!(
            stats
                .snapshot()
                .peers
                .iter()
                .map(|p| p.address.as_str())
                .collect::<Vec<_>>(),
            ["a", "B"]
        );
        assert!(!stats.observe(beat(3, "B"), 999));
        assert_eq!(stats.snapshot().peers[1].time, 103);
        assert!(
            stats.observe(beat(7, "A"), 1_000_000),
            "a lost observation does not block later core NATS traffic"
        );
        assert_eq!(stats.snapshot().peers.len(), 3, "no expiry and exact case");
    }
    #[test]
    fn topology_and_discovery_do_not_enrich_population() {
        let mut stats = Statistics::default();
        stats.topology(vec![1, 2, 3]);
        stats.discovery(vec![4, 5]);
        assert_eq!(
            stats.snapshot(),
            Snapshot {
                peers: vec![],
                islands: vec![1, 2, 3],
                discovery: vec![4, 5]
            }
        );
    }
    #[test]
    fn statistics_population_can_exceed_native_capacity_without_eviction() {
        let mut stats = registered();
        for sequence in 1..=30_000 {
            stats.observe(beat(sequence, &format!("Peer{sequence}")), sequence as i64);
        }
        let view = stats.snapshot();
        assert_eq!(view.peers.len(), 30_000);
        assert_eq!(view.peers.first().unwrap().address, "Peer1");
        assert_eq!(view.peers.last().unwrap().address, "Peer30000");
    }
}
