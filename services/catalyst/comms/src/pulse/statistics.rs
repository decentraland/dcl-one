//! Pulse-owned legacy statistics; never a source of admission or native membership.
use crate::protocol::bus::statistics::{
    Change, Event, Observation, Peer, ProducerGrant, ProducerReply, ProducerRequest, Snapshot,
    Update,
};
use std::collections::{BTreeMap, HashMap, VecDeque};

const MAX_RETAINED_CONNECTOR_GENERATIONS: usize = 4096;

/// Exact-case wallet membership with Map insertion order and no expiry. Producer
/// cursors suppress duplicate/out-of-order observations, not disconnected sessions:
/// every accepted socket disconnect removes its wallet, regardless of other sockets.
/// Handovers retain outgoing cursors so every admitted connector can drain.
#[derive(Default)]
pub struct Statistics {
    peers: BTreeMap<u64, Peer>,
    order: HashMap<String, u64>,
    next: u64,
    incarnation: uuid::Uuid,
    next_generation: u64,
    active: Option<(ProducerGrant, u64)>,
    retiring: HashMap<u64, (ProducerGrant, u64)>,
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
        crate::pulse::metrics::register_statistics_capacity_series();
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
                if self.retiring.len() >= MAX_RETAINED_CONNECTOR_GENERATIONS {
                    crate::pulse::metrics::statistics_registration_capacity_refused();
                    return rejected;
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
                // Begin cannot grant another candidate at capacity; keep Confirm
                // defensive before any producer state changes.
                if self.retiring.len() >= MAX_RETAINED_CONNECTOR_GENERATIONS {
                    crate::pulse::metrics::statistics_registration_capacity_refused();
                    return rejected;
                }
                self.pending = None;
                if let Some(outgoing) = self.active.take() {
                    self.retiring.insert(outgoing.0.generation, outgoing);
                }
                crate::pulse::metrics::statistics_retained_connector_generations(
                    self.retiring.len(),
                );
                if self.retiring.len() == MAX_RETAINED_CONNECTOR_GENERATIONS {
                    tracing::warn!(
                        retained_generations = self.retiring.len(),
                        "statistics connector retention capacity reached; new statistics registrations are refused until Pulse restarts"
                    );
                }
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
            self.active
                .iter_mut()
                .chain(self.retiring.get_mut(&observation.generation))
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
    fn invalid_observations_are_counted_without_changing_population() {
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
    }
    #[test]
    fn capacity_metrics_report_retention_and_only_refused_registration_attempts() {
        let initial = crate::pulse::metrics::render_recorded(|| {
            Statistics::with_incarnation(uuid::Uuid::new_v4());
        });
        assert!(
            initial
                .lines()
                .any(|line| line == "pulse_statistics_retained_connector_generations 0")
        );
        assert!(
            initial
                .lines()
                .any(|line| line == "pulse_statistics_registration_capacity_refused_total 0")
        );
        let rendered = crate::pulse::metrics::render_recorded(|| {
            let mut stats = Statistics::with_incarnation(uuid::Uuid::new_v4());
            let mut active = None;
            for producer in 0..=MAX_RETAINED_CONNECTOR_GENERATIONS {
                active = Some(register(
                    &mut stats,
                    uuid::Uuid::from_u128(producer as u128),
                ));
            }
            let active = active.unwrap();
            assert_eq!(
                stats.control(ProducerRequest::Begin {
                    incarnation: active.incarnation,
                    producer: active.producer
                }),
                ProducerReply::Active(active.clone())
            );
            assert_eq!(
                stats.control(ProducerRequest::Confirm(active.clone())),
                ProducerReply::Active(active.clone())
            );
            assert!(matches!(
                stats.control(ProducerRequest::Begin {
                    incarnation: uuid::Uuid::new_v4(),
                    producer: uuid::Uuid::new_v4()
                }),
                ProducerReply::Rejected { .. }
            ));
            let retired = stats.retiring.values().next().unwrap().0.clone();
            let mut forged = active.clone();
            forged.challenge = uuid::Uuid::new_v4();
            for invalid in [retired, forged] {
                assert!(matches!(
                    stats.control(ProducerRequest::Confirm(invalid)),
                    ProducerReply::Rejected { .. }
                ));
            }
            for _ in 0..3 {
                assert!(matches!(
                    stats.control(ProducerRequest::Begin {
                        incarnation: active.incarnation,
                        producer: uuid::Uuid::new_v4()
                    }),
                    ProducerReply::Rejected { .. }
                ));
            }
        });
        let lines = rendered
            .lines()
            .filter(|line| line.starts_with("pulse_statistics_"))
            .collect::<Vec<_>>();
        assert!(lines.contains(&"pulse_statistics_retained_connector_generations 4096"));
        assert!(lines.contains(&"pulse_statistics_registration_capacity_refused_total 3"));
        assert_eq!(lines.len(), 2, "no per-producer labels or extra series");
        let restarted = crate::pulse::metrics::render_recorded(|| {
            let mut previous = Statistics::with_incarnation(uuid::Uuid::new_v4());
            register(&mut previous, uuid::Uuid::from_u128(1));
            register(&mut previous, uuid::Uuid::from_u128(2));
            // The same recorder must replace a nonzero gauge with zero.
            Statistics::with_incarnation(uuid::Uuid::new_v4());
        });
        assert!(
            restarted
                .lines()
                .any(|line| line == "pulse_statistics_retained_connector_generations 0")
        );
    }
    #[test]
    fn capacity_refuses_registration_without_forgetting_any_cursor() {
        let mut stats = Statistics::default();
        let mut grants = Vec::new();
        for producer in 0..=MAX_RETAINED_CONNECTOR_GENERATIONS {
            let grant = register(&mut stats, uuid::Uuid::from_u128(producer as u128));
            assert!(stats.observe(
                through(
                    &grant,
                    1,
                    Event::Heartbeat {
                        address: format!("peer{producer}"),
                        position: [1.0; 3]
                    }
                ),
                1
            ));
            grants.push(grant);
        }
        assert_eq!(stats.retiring.len(), MAX_RETAINED_CONNECTOR_GENERATIONS);
        let state = stats.snapshot();
        let active = stats.active.clone();
        let retained = stats.retiring.clone();
        let generation = stats.next_generation;
        let begin = ProducerRequest::Begin {
            incarnation: stats.incarnation,
            producer: uuid::Uuid::new_v4(),
        };
        assert!(matches!(
            stats.control(begin),
            ProducerReply::Rejected { .. }
        ));
        assert_eq!(stats.active, active);
        assert_eq!(stats.retiring, retained);
        assert_eq!(stats.next_generation, generation);
        assert!(stats.pending.is_none());
        assert_eq!(stats.snapshot(), state);
        let current = grants.last().unwrap();
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
        for grant in &grants {
            assert!(!stats.observe(
                through(
                    grant,
                    1,
                    Event::Disconnect {
                        address: "replay".into()
                    }
                ),
                999
            ));
            assert!(stats.observe(
                through(
                    grant,
                    3,
                    Event::Heartbeat {
                        address: format!("peer{}", grant.producer.as_u128()),
                        position: [3.0; 3]
                    }
                ),
                -7
            ));
        }
        assert_eq!(stats.snapshot().peers.len(), grants.len());
        assert!(stats.snapshot().peers.iter().all(|peer| peer.time == -7));
        let candidate = ProducerGrant {
            incarnation: stats.incarnation,
            producer: uuid::Uuid::new_v4(),
            generation: generation + 1,
            challenge: uuid::Uuid::new_v4(),
        };
        // Public control cannot create a pending candidate at full capacity.
        // Inject one only to exercise the defensive Confirm guard.
        stats.pending = Some(candidate.clone());
        assert!(matches!(
            stats.control(ProducerRequest::Begin {
                incarnation: stats.incarnation,
                producer: uuid::Uuid::new_v4()
            }),
            ProducerReply::Rejected { .. }
        ));
        assert_eq!(stats.pending, Some(candidate.clone()));
        assert!(matches!(
            stats.control(ProducerRequest::Confirm(candidate.clone())),
            ProducerReply::Rejected { .. }
        ));
        assert_eq!(stats.pending, Some(candidate));
        assert_eq!(stats.active, active.map(|(grant, _)| (grant, 3)));
        assert_eq!(stats.retiring.len(), MAX_RETAINED_CONNECTOR_GENERATIONS);
    }
    #[test]
    fn candidate_confirmation_and_restart_preserve_exact_fences() {
        let mut stats = registered();
        let first = stats.active.as_ref().unwrap().0.clone();
        assert!(stats.observe(beat(1, "a"), 1));
        let begin = |producer| ProducerRequest::Begin {
            incarnation: first.incarnation,
            producer,
        };
        let ProducerReply::Grant(delayed) = stats.control(begin(uuid::Uuid::from_u128(7))) else {
            panic!()
        };
        let ProducerReply::Grant(newer) = stats.control(begin(uuid::Uuid::from_u128(8))) else {
            panic!()
        };
        assert!(matches!(
            stats.control(ProducerRequest::Confirm(delayed)),
            ProducerReply::Rejected { .. }
        ));
        assert_eq!(stats.pending.as_ref(), Some(&newer));
        let mut forged = newer.clone();
        forged.challenge = uuid::Uuid::new_v4();
        assert!(matches!(
            stats.control(ProducerRequest::Confirm(forged)),
            ProducerReply::Rejected { .. }
        ));
        assert_eq!(stats.pending.as_ref(), Some(&newer));
        assert!(stats.observe(beat(2, "a"), 2));
        assert_eq!(
            stats.control(ProducerRequest::Confirm(newer.clone())),
            ProducerReply::Active(newer.clone())
        );
        assert_eq!(
            stats.control(ProducerRequest::Confirm(newer.clone())),
            ProducerReply::Active(newer)
        );
        assert!(!stats.observe(beat(2, "a"), 99));
        assert!(matches!(
            stats.control(ProducerRequest::Confirm(first.clone())),
            ProducerReply::Rejected { .. }
        ));
        let mut restarted = Statistics::with_incarnation(uuid::Uuid::new_v4());
        assert!(matches!(
            restarted.control(ProducerRequest::Confirm(first.clone())),
            ProducerReply::Rejected { .. }
        ));
        assert!(matches!(
            restarted.control(begin(first.producer)),
            ProducerReply::Rejected { .. }
        ));
        assert!(!restarted.observe(beat(3, "a"), 3));
        assert!(restarted.snapshot().peers.is_empty());
        let fresh = register(&mut restarted, uuid::Uuid::new_v4());
        assert!(restarted.observe(
            through(
                &fresh,
                1,
                Event::Heartbeat {
                    address: "fresh".into(),
                    position: [1.0; 3]
                }
            ),
            -100
        ));
        restarted.next_generation = u64::MAX;
        assert!(matches!(
            restarted.control(ProducerRequest::Begin {
                incarnation: fresh.incarnation,
                producer: uuid::Uuid::new_v4()
            }),
            ProducerReply::Rejected { .. }
        ));
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
        register(&mut stats, uuid::Uuid::from_u128(7));
        let incoming = register(&mut stats, uuid::Uuid::from_u128(8));
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
    fn repeated_handovers_keep_every_outgoing_connectors_events_and_cursors() {
        let mut stats = registered();
        let a = stats.active.as_ref().unwrap().0.clone();
        assert!(stats.observe(beat(1, "a"), 1));
        let b = register(&mut stats, uuid::Uuid::from_u128(7));
        assert!(stats.observe(
            through(
                &b,
                1,
                Event::Heartbeat {
                    address: "b".into(),
                    position: [2.0; 3]
                }
            ),
            2
        ));
        register(&mut stats, uuid::Uuid::from_u128(8));
        while stats.next_update(false).is_some() {}
        for (grant, address) in [(&a, "a"), (&b, "b")] {
            let heartbeat = through(
                grant,
                3,
                Event::Heartbeat {
                    address: address.into(),
                    position: [4.0; 3],
                },
            );
            assert!(stats.observe(heartbeat.clone(), -10));
            assert!(!stats.observe(heartbeat.clone(), 999));
            let mut forged = heartbeat;
            forged.sequence = 4;
            forged.producer = uuid::Uuid::new_v4();
            assert!(!stats.observe(forged, 999));
            let Some(Update::Change {
                change: Change::Peer(peer),
                ..
            }) = stats.next_update(false)
            else {
                panic!("heartbeat change")
            };
            assert_eq!(peer.time, -10);
            assert_eq!(peer.position, [4.0; 3]);
            let mut wrong_generation = through(
                grant,
                4,
                Event::Disconnect {
                    address: address.into(),
                },
            );
            wrong_generation.generation += 1;
            assert!(!stats.observe(wrong_generation, 999));
            assert!(stats.observe(
                through(
                    grant,
                    4,
                    Event::Disconnect {
                        address: address.into()
                    }
                ),
                11
            ));
            assert!(
                matches!(stats.next_update(false), Some(Update::Change { change: Change::Remove(removed), .. }) if removed == address)
            );
        }
        assert!(stats.snapshot().peers.is_empty());
    }
    #[test]
    fn backlog_recovery_retains_complete_state_across_handovers() {
        let mut stats = registered();
        for sequence in 1..=5000 {
            assert!(stats.observe(beat(sequence, "a"), sequence as i64));
        }
        assert!(stats.recovery);
        assert!(stats.changes.is_empty());
        let state = stats.snapshot();
        register(&mut stats, uuid::Uuid::from_u128(7));
        register(&mut stats, uuid::Uuid::from_u128(8));
        assert_eq!(stats.snapshot(), state);
        assert!(
            matches!(stats.next_update(false), Some(Update::Snapshot { revision: 5000, state: recovered }) if recovered == state)
        );
        assert!(stats.next_update(false).is_none());
        assert!(!stats.observe(beat(5000, "a"), 999));
        assert!(stats.observe(beat(5001, "a"), -1));
        assert_eq!(stats.snapshot().peers[0].time, -1);
    }
    fn through(grant: &ProducerGrant, sequence: u64, event: Event) -> Observation {
        Observation {
            version: 2,
            incarnation: grant.incarnation,
            generation: grant.generation,
            producer: grant.producer,
            sequence,
            event,
        }
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
