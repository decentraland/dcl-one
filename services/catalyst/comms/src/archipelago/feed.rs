//! The connector's side of the island feed. Over `serve`'s one NATS link, the gatekeeper's
//! (`gatekeeper::bus`, core NATS, no JetStream or durable consumer acknowledgement), inbound
//! carries the topology and service-discovery snapshots the stats surface serves
//! ([`dispatch`]) and outbound its realm owners' control positions ([`Outbox`]).
//!
//! The island subscriber lives in the same process, so the rest is calls: it hands each minted
//! assignment to the sockets ([`IslandSockets`], [`route_island`]), and the connector announces
//! each session to it ([`CommsState::announce_session`](crate::gatekeeper::CommsState::announce_session)) on the
//! welcome, on a v4 realm claim and from the reconciliation cycle the feed job runs. The
//! announcement is what makes a reconnect that did not move answerable at all.
//!
//! A position with no ready link is dropped rather than queued: it describes a socket that
//! exists now, so a backlog delivered after a reconnect would speak for sessions that are already
//! gone. Submitted means bounded local SDK acceptance, without broker or subscriber confirmation.
//! Already SDK-accepted commands may be lost or delivered after reconnect; application-queue
//! owner/revision checks do not purge that bounded SDK buffer.

use crate::archipelago::reconciliation::Reconciliation;
use crate::archipelago::registry::{
    AssignmentSend, ControlPositionPermit, MAX_ASSIGNMENT_BYTES, PeerLink, PeersRegistry,
};
use crate::archipelago::state::{AppState, AppStateInner};
use crate::gatekeeper::admission::cluster::sockets::{IslandDelivery, IslandSockets};
use crate::gatekeeper::bus::{NatsBus, NatsHandler, NatsSubscription, PublishOutcome};
use crate::jobs::Jobs;
use crate::protocol::archipelago::archipelago::{IslandChangedMessage, ServiceDiscoveryMessage};
use crate::protocol::bus::BusError;
use crate::protocol::bus::control_position::{
    ControlPosition, SUBJECT as CONTROL_POSITION_SUBJECT,
};
use crate::protocol::bus::topology::{
    DISCOVERY_SUBJECT, ISLANDS_SUBJECT, decode_discovery, decode_islands,
};
use parking_lot::{Mutex, RwLock};
use prost::Message as _;
use serde::Serialize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;

pub const OUTBOUND_CAPACITY: usize = 1024;
const MAX_ANNOUNCEMENT_BYTES: usize = 1024;
pub(crate) const RECONCILE_INTERVAL: Duration = Duration::from_secs(5);
const RECONCILE_PACING: Duration = Duration::from_millis(10);
/// The link reports no changes, so the feed job samples its readiness this often: a link that
/// comes back restarts the reconciliation cycle within this delay.
const READINESS_POLL: Duration = Duration::from_millis(100);

pub const SUBSCRIPTIONS: [&str; 2] = [ISLANDS_SUBJECT, DISCOVERY_SUBJECT];

/// How long a service discovery heartbeat stands before the core reads as unhealthy. Absolute
/// delta on purpose: the timestamp is stamped on the publisher's host, so forward clock skew must
/// not read as fresh.
pub const DISCOVERY_FRESHNESS_MS: i64 = 90_000;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct IslandReport {
    pub id: String,
    pub peers: Vec<String>,
    pub max_peers: u32,
    pub center: [f32; 3],
    pub radius: f32,
}

/// Decodes a topology snapshot into the shape the stats endpoints serve. The publisher reports an
/// uncapped `max_peers` of 0 and its own id scheme: nothing here parses or compares either.
pub fn decode_islands_report(data: &[u8]) -> Result<Vec<IslandReport>, BusError> {
    Ok(decode_islands(data)?
        .data
        .into_iter()
        .filter_map(|island| {
            let center = island.center?;
            Some(IslandReport {
                id: island.id,
                peers: island.peers,
                max_peers: island.max_peers,
                center: [center.x, center.y, center.z],
                radius: island.radius as f32,
            })
        })
        .collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Routed {
    /// Accepted by the socket mailbox, not a WebSocket write or a client receipt.
    Delivered,
    /// Replaced a pending absolute assignment; arrival order is not a source revision.
    Coalesced,
    TooLarge,
    NoSessionSocket,
    NoSocket,
    Topology,
    Discovery,
    Undecodable,
    Ignored,
}

/// The latest topology and service-discovery snapshots the feed carried, and the counters that
/// tell a silent feed apart from one whose assignments reach no socket.
#[derive(Default)]
pub struct FeedCache {
    /// Shared complete-wallet view; fed by this existing subscription on every replica.
    pub topology: Arc<crate::topology::Topology>,
    islands: RwLock<Vec<IslandReport>>,
    discovery: RwLock<Option<ServiceDiscoveryMessage>>,
    delivered: AtomicU64,
    no_session_socket: AtomicU64,
    deduplicated: AtomicU64,
    undecodable: AtomicU64,
    coalesced: AtomicU64,
    assignment_oversized: AtomicU64,
    socket_write_timeout: AtomicU64,
    socket_assignments_written: AtomicU64,
}

impl FeedCache {
    pub fn islands(&self) -> Vec<IslandReport> {
        self.islands.read().clone()
    }

    pub fn island(&self, id: &str) -> Option<IslandReport> {
        self.islands
            .read()
            .iter()
            .find(|island| island.id == id)
            .cloned()
    }

    pub fn islands_count(&self) -> usize {
        self.islands.read().len()
    }

    pub fn on_islands(&self, islands: Vec<IslandReport>) {
        *self.islands.write() = islands;
    }

    pub fn on_discovery(&self, message: ServiceDiscoveryMessage) {
        *self.discovery.write() = Some(message);
    }

    pub fn is_core_healthy(&self, now_ms: i64) -> bool {
        self.discovery
            .read()
            .as_ref()
            .and_then(|m| m.status.as_ref())
            .is_some_and(|status| {
                (now_ms - status.current_time as i64).abs() < DISCOVERY_FRESHNESS_MS
            })
    }

    pub fn core_user_count(&self) -> u32 {
        self.discovery
            .read()
            .as_ref()
            .and_then(|m| m.status.as_ref())
            .map(|status| status.user_count)
            .unwrap_or(0)
    }

    pub fn delivered_count(&self) -> u64 {
        self.delivered.load(Ordering::Relaxed)
    }

    pub fn no_session_socket_count(&self) -> u64 {
        self.no_session_socket.load(Ordering::Relaxed)
    }

    /// Counted by the socket layer, which owns the per-socket window: the feed delivered the
    /// message, the socket decided it had already forwarded that room.
    pub fn on_deduplicated(&self) {
        self.deduplicated.fetch_add(1, Ordering::Relaxed);
    }

    pub fn deduplicated_count(&self) -> u64 {
        self.deduplicated.load(Ordering::Relaxed)
    }

    /// Topology and discovery snapshots that could not be decoded.
    pub fn undecodable_count(&self) -> u64 {
        self.undecodable.load(Ordering::Relaxed)
    }

    pub fn coalesced_count(&self) -> u64 {
        self.coalesced.load(Ordering::Relaxed)
    }

    pub fn assignment_oversized_count(&self) -> u64 {
        self.assignment_oversized.load(Ordering::Relaxed)
    }

    pub fn socket_write_timeout_count(&self) -> u64 {
        self.socket_write_timeout.load(Ordering::Relaxed)
    }

    pub(crate) fn on_socket_write_timeout(&self) {
        self.socket_write_timeout.fetch_add(1, Ordering::Relaxed);
    }

    pub fn socket_assignments_written_count(&self) -> u64 {
        self.socket_assignments_written.load(Ordering::Relaxed)
    }

    pub(crate) fn on_socket_assignment_written(&self) {
        self.socket_assignments_written
            .fetch_add(1, Ordering::Relaxed);
    }
}

/// Routes one feed snapshot. Never fails: the publisher is out of this process, so a malformed
/// payload costs its own message and nothing else.
pub fn dispatch(state: &AppState, subject: &str, data: &[u8]) -> Routed {
    match subject {
        ISLANDS_SUBJECT => match state
            .feed
            .topology
            .receive(data)
            .and_then(|_| decode_islands_report(data))
        {
            Ok(islands) => {
                state.feed.on_islands(islands);
                Routed::Topology
            }
            Err(e) => undecodable(state, subject, &e),
        },
        DISCOVERY_SUBJECT => match decode_discovery(data) {
            Ok(message) => {
                state.feed.on_discovery(message);
                Routed::Discovery
            }
            Err(e) => undecodable(state, subject, &e),
        },
        _ => Routed::Ignored,
    }
}

fn undecodable(state: &AppState, subject: &str, error: &BusError) -> Routed {
    state.feed.undecodable.fetch_add(1, Ordering::Relaxed);
    tracing::warn!(subject = %subject, error = %error, "cannot decode a feed message");
    Routed::Undecodable
}

/// The island subscriber's hand-off: every assignment it mints reaches the connector's sockets
/// through this call. Queued or coalesced into a socket's mailbox is a delivery; no socket of
/// that wallet (or session) here, a closed one or an oversized assignment is not.
impl IslandSockets for AppStateInner {
    fn deliver_island(
        &self,
        wallet: &str,
        session: &str,
        assignment: &IslandChangedMessage,

        pulse_admission: Option<&crate::gatekeeper::admission::cluster::fence::PulseAdmissionStamp>,
    ) -> IslandDelivery {
        match route_to_session(
            self,
            &wallet.to_ascii_lowercase(),
            session,
            assignment,
            pulse_admission.cloned(),
        ) {
            Routed::Delivered | Routed::Coalesced => IslandDelivery::Delivered,
            _ => IslandDelivery::Undelivered,
        }
    }
}

/// Hands an assignment only to the named session; an absent socket costs no encoding.
pub fn route_island(
    state: &AppStateInner,
    wallet: &str,
    session: &str,
    assignment: &IslandChangedMessage,
) -> Routed {
    let address = wallet.to_ascii_lowercase();
    route_to_session(state, &address, session, assignment, None)
}

fn route_to_session(
    state: &AppStateInner,
    address: &str,
    session: &str,
    assignment: &IslandChangedMessage,
    pulse_admission: Option<crate::gatekeeper::admission::cluster::fence::PulseAdmissionStamp>,
) -> Routed {
    let session = session.to_ascii_lowercase();
    let Some(link) = state.registry.peer_link(address, &session) else {
        // Only a wallet held under another session tells a stale session apart from a wallet
        // this process does not hold at all.
        if state.registry.has_peer(address) {
            state.feed.no_session_socket.fetch_add(1, Ordering::Relaxed);
            return Routed::NoSessionSocket;
        }
        return Routed::NoSocket;
    };
    deliver(state, address, &link, assignment, pulse_admission)
}

/// Encodes the assignment for the socket's mailbox, which holds the wire bytes the socket
/// forwards, bounded by [`MAX_ASSIGNMENT_BYTES`].
fn deliver(
    state: &AppStateInner,
    address: &str,
    link: &PeerLink,
    assignment: &IslandChangedMessage,
    pulse_admission: Option<crate::gatekeeper::admission::cluster::fence::PulseAdmissionStamp>,
) -> Routed {
    let data = assignment.encode_to_vec();
    if data.len() > MAX_ASSIGNMENT_BYTES {
        state
            .feed
            .assignment_oversized
            .fetch_add(1, Ordering::Relaxed);
        return Routed::TooLarge;
    }
    let outcome = match link.send_assignment(&data, pulse_admission) {
        AssignmentSend::Queued => Routed::Delivered,
        AssignmentSend::Coalesced => {
            state.feed.coalesced.fetch_add(1, Ordering::Relaxed);
            Routed::Coalesced
        }
        AssignmentSend::TooLarge => {
            state
                .feed
                .assignment_oversized
                .fetch_add(1, Ordering::Relaxed);
            return Routed::TooLarge;
        }
        AssignmentSend::Closed => return Routed::NoSocket,
    };
    state.peers.set_island(address, &assignment.island_id);
    state.feed.delivered.fetch_add(1, Ordering::Relaxed);
    outcome
}

pub struct ControlPositionAnnouncement {
    payload: Vec<u8>,
    permit: ControlPositionPermit,
}

impl ControlPositionAnnouncement {
    pub(crate) fn new(record: ControlPosition, permit: ControlPositionPermit) -> Option<Self> {
        let close = record.position.is_none();
        if !permit.matches(&record.wallet, &record.session, record.epoch, close) {
            return None;
        }
        Some(Self {
            payload: record.encode()?,
            permit,
        })
    }
}

/// The bounded queue the realm owners' control positions go through: the feed job publishes each
/// in turn, after checking that the socket it speaks for is still current.
pub struct Outbox {
    bus: Arc<dyn NatsBus>,
    tx: mpsc::Sender<ControlPositionAnnouncement>,
    rx: Mutex<Option<mpsc::Receiver<ControlPositionAnnouncement>>>,
    dropped: AtomicU64,
}

impl Outbox {
    pub(crate) fn new(bus: Arc<dyn NatsBus>) -> Self {
        let (tx, rx) = mpsc::channel(OUTBOUND_CAPACITY);
        Self {
            bus,
            tx,
            rx: Mutex::new(Some(rx)),
            dropped: AtomicU64::new(0),
        }
    }

    /// Accepts a position into the local bounded queue, not a broker/consumer acknowledgement.
    /// `false` means unavailable, overloaded or oversized; the next heartbeat carries a fresh one.
    pub fn publish_control_position(&self, announcement: ControlPositionAnnouncement) -> bool {
        if CONTROL_POSITION_SUBJECT.len() + announcement.payload.len() > MAX_ANNOUNCEMENT_BYTES {
            return self.drop_one();
        }
        if !self.is_linked() || self.tx.try_send(announcement).is_err() {
            return self.drop_one();
        }
        true
    }

    /// Whether the link is configured and ready, a broker round trip confirmed on its current
    /// connection: what a position needs to be queued, what the reconciliation cycle needs to
    /// announce, and what `/stats/health` reports as `feed_connected`.
    pub fn is_linked(&self) -> bool {
        self.bus.is_enabled() && self.bus.is_ready()
    }

    /// Local rejections for an unavailable/overloaded link, stale ownership or non-Submitted
    /// handoff outcomes, including delivery-unknown results. This does not count every message
    /// lost after SDK acceptance. Surfaced on `/stats/health` (`feed_publish_dropped`).
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Positions accepted and not yet taken by the feed job.
    #[cfg(any(test, feature = "test-support"))]
    pub fn queued(&self) -> usize {
        self.tx.max_capacity() - self.tx.capacity()
    }

    fn drop_one(&self) -> bool {
        self.dropped.fetch_add(1, Ordering::Relaxed);
        false
    }
}

/// Registers the feed's subscriptions on the gatekeeper's link, dials it (the island subscriber
/// dials it too; connecting is idempotent) and spawns `archipelago-feed`, the job that owns the
/// subscriptions, drains the outbox and runs the reconciliation cycle. Without a configured link
/// nothing is subscribed and no job runs, which is byte-identical to not wiring the feed at all.
pub(crate) fn start(state: &AppState, jobs: &Jobs) -> bool {
    let bus = Arc::clone(&state.gatekeeper.bus);
    if !bus.is_enabled() {
        return false;
    }
    let Some(rx) = state.outbox.rx.lock().take() else {
        return false;
    };
    let feed = Arc::downgrade(state);
    let handler: NatsHandler = Arc::new(move |subject: &str, payload: &[u8]| {
        if let Some(state) = feed.upgrade() {
            dispatch(&state, subject, payload);
        }
    });
    let subscriptions: Vec<NatsSubscription> = SUBSCRIPTIONS
        .iter()
        .map(|subject| bus.subscribe(subject, None, Arc::clone(&handler)))
        .collect();
    bus.connect();
    let state = Arc::clone(state);
    jobs.spawn("archipelago-feed", move |ctx| async move {
        tokio::select! {
            _ = run(&state, rx) => {}
            _ = ctx.cancel.cancelled() => {}
        }
        drop(subscriptions);
        Ok(())
    });
    true
}

/// Drains the outbox one publish at a time and runs the reconciliation cycle, until the job is
/// cancelled. A non-Submitted local handoff counts as dropped, including delivery-unknown
/// outcomes; Submitted is SDK acceptance and can be lost or delivered after reconnect.
/// The application never retries that historical record. The link reconnects on its own:
/// recovery rebuilds from the registry when it comes back instead of retrying a historical
/// announcement. The cycle announces only while the link is up, because the subscriber's lookup
/// of an announced session needs it: a sweep the link drops in the middle of ends there.
async fn run(state: &AppState, mut rx: mpsc::Receiver<ControlPositionAnnouncement>) {
    let bus = state.gatekeeper.bus.as_ref();
    let mut pending = Reconciliation::new(Instant::now());
    let mut linked = false;
    let mut next_announcement = Instant::now();
    let mut readiness = tokio::time::interval(READINESS_POLL);
    readiness.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut reconcile = tokio::time::interval(RECONCILE_INTERVAL);
    reconcile.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = readiness.tick() => {
                sample(state, &mut linked, &mut pending);
            }
            _ = reconcile.tick() => {
                if sample(state, &mut linked, &mut pending) {
                    pending.refill(&state.registry, Instant::now());
                }
            }
            announcement = rx.recv() => {
                let Some(announcement) = announcement else { return };
                if !control_position_announcement_is_current(&state.registry, &announcement)
                    || bus.publish(CONTROL_POSITION_SUBJECT, announcement.payload).await
                        != PublishOutcome::Submitted
                {
                    state.outbox.drop_one();
                }
            }
            // The link is sampled again before each announcement, not only when this branch was
            // armed: one that dropped since abandons the sweep here, and nothing is announced.
            _ = tokio::time::sleep_until(next_announcement), if !pending.is_empty() => {
                if sample(state, &mut linked, &mut pending)
                    && let Some(target) = pending.next(&state.registry)
                {
                    state.gatekeeper.announce_session(&target.address, &target.session);
                }
                next_announcement = Instant::now() + RECONCILE_PACING;
            }
        }
    }
}

/// Samples the link and returns whether it is up. A link that went down abandons the sweep in
/// progress; one that came back (or is up for the first time) restarts the cycle with a full sweep
/// and its follow-ups, because every lookup made meanwhile failed and the cluster feed is
/// edge-triggered.
fn sample(state: &AppState, linked: &mut bool, pending: &mut Reconciliation) -> bool {
    let ready = state.outbox.is_linked();
    if ready != *linked {
        let now = Instant::now();
        pending.reset(now);
        if ready {
            pending.refill(&state.registry, now);
        }
        *linked = ready;
    }
    ready
}

fn control_position_announcement_is_current(
    registry: &PeersRegistry,
    announcement: &ControlPositionAnnouncement,
) -> bool {
    announcement.permit.is_close()
        || registry.control_position_permit_is_current(&announcement.permit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archipelago::config::Settings;
    use crate::archipelago::registry::{PeerLink, SocketEvents};
    use crate::archipelago::test_support::{state_on, state_over, state_with_island_feed};
    use crate::gatekeeper::bus::{InProcessBus, RequestError};
    use crate::protocol::archipelago::Position;
    use crate::protocol::archipelago::archipelago::{
        IslandData, IslandStatusMessage, ServiceStatus,
    };
    use crate::protocol::bus::cluster::ClusterLookup;
    use crate::protocol::pulse::decentraland::pulse::PeerClusterSnapshot;
    use std::collections::{HashMap, HashSet};
    use std::sync::atomic::AtomicBool;
    use tower::ServiceExt;

    fn status_message(current_time: u64, user_count: u32) -> Vec<u8> {
        ServiceDiscoveryMessage {
            server_name: "pulse".into(),
            status: Some(ServiceStatus {
                current_time,
                commit_hash: Some("deadbeef".into()),
                user_count,
            }),
        }
        .encode_to_vec()
    }

    fn islands_message() -> Vec<u8> {
        IslandStatusMessage {
            data: vec![
                IslandData {
                    id: "island-C1".into(),
                    peers: vec!["0xa".into(), "0xb".into()],
                    max_peers: 0,
                    center: Some(Position {
                        x: 1.0,
                        y: 2.0,
                        z: 3.0,
                    }),
                    radius: 8.0,
                },
                IslandData {
                    id: "centerless".into(),
                    peers: vec![],
                    max_peers: 0,
                    center: None,
                    radius: 0.0,
                },
            ],
        }
        .encode_to_vec()
    }

    #[test]
    fn a_topology_snapshot_drops_an_island_without_a_center() {
        let report = decode_islands_report(&islands_message()).expect("decodes");
        assert_eq!(report.len(), 1);
        assert_eq!(report[0].id, "island-C1");
        assert_eq!(report[0].peers, vec!["0xa".to_string(), "0xb".to_string()]);
        assert_eq!(report[0].max_peers, 0);
        assert_eq!(report[0].center, [1.0, 2.0, 3.0]);
        assert_eq!(report[0].radius, 8.0);
    }

    #[test]
    fn the_core_is_healthy_only_while_its_heartbeat_is_fresh() {
        let cache = FeedCache::default();
        assert!(!cache.is_core_healthy(1_000_000));
        assert_eq!(cache.core_user_count(), 0);

        let message = ServiceDiscoveryMessage::decode(status_message(1_000_000, 42).as_slice())
            .expect("decodes");
        cache.on_discovery(message);
        assert!(cache.is_core_healthy(1_000_000));
        assert!(cache.is_core_healthy(1_000_000 + DISCOVERY_FRESHNESS_MS - 1));
        assert!(!cache.is_core_healthy(1_000_000 + DISCOVERY_FRESHNESS_MS));
        assert!(
            !cache.is_core_healthy(1_000_000 - DISCOVERY_FRESHNESS_MS),
            "a heartbeat from the future is not fresh either"
        );
        assert_eq!(cache.core_user_count(), 42);
    }

    /// The gatekeeper's link as an in-process bus that also records every subscription asked of
    /// it, with its queue group.
    #[derive(Default)]
    struct Recorded {
        bus: InProcessBus,
        subscribed: Mutex<Vec<(String, Option<String>)>>,
    }

    #[async_trait::async_trait]
    impl NatsBus for Recorded {
        fn is_enabled(&self) -> bool {
            self.bus.is_enabled()
        }

        fn is_connected(&self) -> bool {
            self.bus.is_connected()
        }

        fn is_ready(&self) -> bool {
            self.bus.is_ready()
        }

        fn connect(&self) {}

        fn subscribe(
            &self,
            subject: &str,
            queue: Option<&str>,
            handler: NatsHandler,
        ) -> NatsSubscription {
            self.subscribed
                .lock()
                .push((subject.to_owned(), queue.map(str::to_owned)));
            self.bus.subscribe(subject, queue, handler)
        }

        async fn publish(&self, subject: &str, payload: Vec<u8>) -> PublishOutcome {
            self.bus.publish(subject, payload).await
        }

        async fn request(&self, subject: &str, payload: Vec<u8>) -> Result<Vec<u8>, RequestError> {
            self.bus.request(subject, payload).await
        }
    }

    /// On the gatekeeper's link the connector takes the two topology snapshots, every replica a
    /// copy (no queue group), and nothing else: no island assignment and no connect cross it.
    #[tokio::test]
    async fn the_connector_subscribes_to_the_topology_snapshots_alone() {
        let bus = Arc::new(Recorded::default());
        let _state = state_over(Settings::for_tests(), bus.clone()).await;
        assert_eq!(
            *bus.subscribed.lock(),
            vec![
                (ISLANDS_SUBJECT.to_owned(), None),
                (DISCOVERY_SUBJECT.to_owned(), None)
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn social_replica_uses_existing_unqueued_subscription_independently_of_primary_mounts() {
        use crate::protocol::pulse::decentraland::kernel::comms::v3::{
            IslandData, IslandStatusMessage,
        };
        let bus = Arc::new(Recorded::default());
        let state = state_over(Settings::for_tests(), bus.clone()).await;
        let _mounts = crate::archipelago::mounts(state.clone(), false);
        let bytes = IslandStatusMessage {
            data: vec![IslandData {
                peers: vec![WALLET.into()],
                ..Default::default()
            }],
        }
        .encode_to_vec();
        bus.bus.inject(ISLANDS_SUBJECT, &bytes);
        assert!(
            state.feed.topology.snapshot().is_online(WALLET),
            "centerless membership is still social truth"
        );
        assert_eq!(
            bus.subscribed
                .lock()
                .iter()
                .filter(|(subject, _)| subject == ISLANDS_SUBJECT)
                .count(),
            1
        );
        tokio::time::advance(crate::topology::TOPOLOGY_RECEIPT_TTL).await;
        // Healthy discovery cannot renew stopped topology.
        bus.bus.inject(
            DISCOVERY_SUBJECT,
            &crate::protocol::bus::topology::encode_discovery("pulse", "pin", 1, 1),
        );
        assert!(!state.feed.topology.snapshot().is_online(WALLET));
    }

    const WALLET: &str = "0x000000000000000000000000000000000000000a";
    const SESSION: &str = "0x000000000000000000000000000000000000000b";

    fn control_record(epoch: u64, sequence: u64, position: Option<[f32; 3]>) -> ControlPosition {
        ControlPosition {
            audience: "realm-a".into(),
            wallet: WALLET.into(),
            session: SESSION.into(),
            epoch,
            sequence,
            realm: position.map(|_| "main".to_owned()).unwrap_or_default(),
            position,
        }
    }

    /// `WALLET`'s realm-owning v4 socket at epoch 7 on `registry`, and its live position permit.
    fn realm_owner(registry: &PeersRegistry) -> (PeerLink, SocketEvents, ControlPositionPermit) {
        let (link, events) = registry.on_v4_peer_connected_with_realm_epoch(
            WALLET,
            SESSION,
            "connection-7",
            true,
            7,
        );
        let permit = registry
            .control_position_permit(WALLET, SESSION, 7, false)
            .unwrap();
        (link, events, permit)
    }

    fn position(permit: &ControlPositionPermit, sequence: u64) -> ControlPositionAnnouncement {
        ControlPositionAnnouncement::new(
            control_record(7, sequence, Some([1.0, 2.0, 3.0])),
            permit.clone(),
        )
        .unwrap()
    }

    #[test]
    fn a_position_without_a_ready_link_is_dropped_and_counted() {
        let registry = PeersRegistry::new();
        let (_link, _events, permit) = realm_owner(&registry);
        let bus = Arc::new(InProcessBus::new());
        bus.set_connected(false);
        let outbox = Outbox::new(bus);
        assert!(!outbox.publish_control_position(position(&permit, 1)));
        assert_eq!(outbox.dropped(), 1);

        let unconfigured = Outbox::new(Arc::new(crate::gatekeeper::bus::DisabledBus));
        assert!(!unconfigured.publish_control_position(position(&permit, 2)));
        assert_eq!(unconfigured.dropped(), 1);
        assert_eq!(unconfigured.queued(), 0);
    }

    #[tokio::test]
    async fn an_unconfigured_link_subscribes_nothing_and_runs_no_job() {
        let state = state_over(
            Settings::for_tests(),
            Arc::new(crate::gatekeeper::bus::DisabledBus),
        )
        .await;
        assert!(state.outbox.rx.lock().is_some(), "no job took the outbox");
        assert!(!start(&state, &Jobs::new()));
    }

    #[test]
    fn control_positions_require_an_exact_live_permit_and_an_authorized_close_survives_removal() {
        let registry = PeersRegistry::new();
        let record = control_record(7, 1, Some([1.0, 2.0, 3.0]));
        let (link, _events) = registry.on_v4_peer_connected_with_realm_epoch(
            &record.wallet,
            &record.session,
            "connection-7",
            true,
            record.epoch,
        );
        let permit = registry
            .control_position_permit(&record.wallet, &record.session, record.epoch, false)
            .unwrap();
        let live = ControlPositionAnnouncement::new(record.clone(), permit).unwrap();
        assert!(control_position_announcement_is_current(&registry, &live));

        link.close();
        assert!(!control_position_announcement_is_current(&registry, &live));
        let (_newer, _events) = registry.on_v4_peer_connected_with_realm_epoch(
            &record.wallet,
            &record.session,
            "connection-8",
            true,
            8,
        );
        let close_record = control_record(7, 2, None);
        let close_permit = registry
            .control_position_permit(
                &close_record.wallet,
                &close_record.session,
                close_record.epoch,
                true,
            )
            .unwrap();
        let close = ControlPositionAnnouncement::new(close_record, close_permit).unwrap();
        assert!(!control_position_announcement_is_current(&registry, &live));
        registry.on_v4_peer_disconnected(&record.wallet, link.id);
        assert!(control_position_announcement_is_current(&registry, &close));
        assert!(ControlPosition::decode(&close.payload).is_some());
    }

    async fn linked() -> (AppState, Arc<InProcessBus>) {
        let bus = Arc::new(InProcessBus::new());
        let state = state_over(Settings::for_tests(), bus.clone()).await;
        (state, bus)
    }

    fn published_on(bus: &InProcessBus, subject: &str) -> Vec<Vec<u8>> {
        bus.published()
            .into_iter()
            .filter(|(published, _)| published == subject)
            .map(|(_, payload)| payload)
            .collect()
    }

    async fn settled(mut done: impl FnMut() -> bool) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while !done() {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .expect("the feed job did not settle");
    }

    async fn get_json(state: &AppState, path: &str) -> serde_json::Value {
        let response = crate::archipelago::mounts(state.clone(), true)
            .stats
            .oneshot(
                axum::http::Request::get(path)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn feed_snapshots_arrive_over_the_gatekeeper_link() {
        let (state, bus) = linked().await;
        let now = chrono::Utc::now().timestamp_millis();
        bus.inject(ISLANDS_SUBJECT, &islands_message());
        bus.inject(DISCOVERY_SUBJECT, &status_message(now as u64, 7));

        let islands = get_json(&state, "/islands").await;
        assert_eq!(islands["islands"].as_array().unwrap().len(), 1);
        assert_eq!(islands["islands"][0]["id"], "island-C1");
        assert!(state.feed.is_core_healthy(now));
        let core = get_json(&state, "/core-status").await;
        assert_eq!(core["healthy"], true);
        assert_eq!(core["userCount"], 7);
    }

    /// A link whose socket is up but whose subscriptions have not crossed a broker round trip:
    /// what the broker link reports between a reconnect and its confirming probe.
    struct Unconfirmed(InProcessBus);

    #[async_trait::async_trait]
    impl NatsBus for Unconfirmed {
        fn is_enabled(&self) -> bool {
            true
        }

        fn is_connected(&self) -> bool {
            true
        }

        fn is_ready(&self) -> bool {
            false
        }

        fn connect(&self) {}

        fn subscribe(
            &self,
            subject: &str,
            queue: Option<&str>,
            handler: NatsHandler,
        ) -> NatsSubscription {
            self.0.subscribe(subject, queue, handler)
        }

        async fn publish(&self, _subject: &str, _payload: Vec<u8>) -> PublishOutcome {
            PublishOutcome::NoConnection
        }

        async fn request(
            &self,
            _subject: &str,
            _payload: Vec<u8>,
        ) -> Result<Vec<u8>, crate::gatekeeper::bus::RequestError> {
            Err(crate::gatekeeper::bus::RequestError::NoConnection)
        }
    }

    /// `/stats/health`'s `feed_connected` is a confirmed round trip on the current link, as the
    /// connector's own link reported it: never for a deployment without `NATS_URL`, and not for a
    /// socket that is up but not yet confirmed.
    #[tokio::test]
    async fn feed_connected_reports_a_ready_link() {
        let connected = |state: AppState| async move {
            get_json(&state, "/stats/health").await["feed_connected"].clone()
        };
        let unconfigured = state_over(
            Settings::for_tests(),
            Arc::new(crate::gatekeeper::bus::DisabledBus),
        )
        .await;
        assert_eq!(connected(unconfigured).await, false, "no NATS_URL");

        let (state, bus) = linked().await;
        assert_eq!(connected(state.clone()).await, true);
        bus.set_connected(false);
        assert_eq!(connected(state).await, false);

        let unconfirmed = state_over(
            Settings::for_tests(),
            Arc::new(Unconfirmed(InProcessBus::new())),
        )
        .await;
        assert_eq!(
            connected(unconfirmed).await,
            false,
            "connected is not ready until the broker round trip"
        );
    }

    /// The link outlives the connector's job, not its owners: the running job owns the
    /// connector (and through it the gatekeeper and the link), and once it stops nothing else
    /// does. The live-broker recovery test ends on the same check.
    #[tokio::test]
    async fn the_link_is_released_once_the_feed_job_stops() {
        let bus: Arc<dyn NatsBus> = Arc::new(InProcessBus::new());
        let jobs = Jobs::new();
        let state = state_on(Settings::for_tests(), bus.clone(), &jobs).await;
        let (_link, _events, permit) = realm_owner(&state.registry);
        assert!(state.outbox.publish_control_position(position(&permit, 1)));
        tokio::time::sleep(Duration::from_millis(20)).await;

        let link = Arc::downgrade(&bus);
        let connector = Arc::downgrade(&state);
        drop(bus);
        drop(state);
        assert!(
            connector.upgrade().is_some() && link.upgrade().is_some(),
            "the running feed job owns the connector and its link"
        );
        jobs.shutdown(Duration::from_secs(5)).await;
        assert!(connector.upgrade().is_none(), "the stopped job let go");
        assert!(link.upgrade().is_none(), "the link outlived its owners");
    }

    /// The same with the island subscriber attached, as `serve` wires it. The subscriber holds
    /// its sockets (the connector), the connector the gatekeeper, and the gatekeeper the
    /// subscriber only through a `Weak`: no cycle, so once the subscriber's job and the feed job
    /// stop, the subscriber, the connector, the gatekeeper and the link are all released.
    #[tokio::test]
    async fn the_island_subscriber_is_released_with_the_connector_once_the_jobs_stop() {
        let bus: Arc<dyn NatsBus> = Arc::new(InProcessBus::new());
        let jobs = Jobs::new();
        let state = state_with_island_feed(Settings::for_tests(), bus.clone(), &jobs).await;
        let (_socket, _events, _) = state.registry.on_peer_connected(WALLET, SESSION);
        state.gatekeeper.announce_session(WALLET, SESSION);
        let (_owner, _owner_events, permit) = realm_owner(&state.registry);
        assert!(state.outbox.publish_control_position(position(&permit, 1)));
        tokio::time::sleep(Duration::from_millis(20)).await;

        let subscriber = state.gatekeeper.island_subscriber();
        let gatekeeper = Arc::downgrade(&state.gatekeeper);
        let connector = Arc::downgrade(&state);
        let link = Arc::downgrade(&bus);
        drop(bus);
        drop(state);
        assert!(
            subscriber.upgrade().is_some()
                && connector.upgrade().is_some()
                && gatekeeper.upgrade().is_some()
                && link.upgrade().is_some(),
            "the running jobs own the subscriber, the connector, the gatekeeper and the link"
        );
        jobs.shutdown(Duration::from_secs(5)).await;
        assert!(
            subscriber.upgrade().is_none(),
            "the stopped subscriber let go"
        );
        assert!(
            connector.upgrade().is_none(),
            "the subscriber let go of its sockets"
        );
        assert!(
            gatekeeper.upgrade().is_none(),
            "the connector let go of the gatekeeper"
        );
        assert!(link.upgrade().is_none(), "the link outlived its owners");
    }

    #[tokio::test]
    async fn a_stale_control_position_is_dropped_before_publish() {
        let (state, bus) = linked().await;
        let (link, _events, permit) = realm_owner(&state.registry);
        assert!(state.outbox.publish_control_position(position(&permit, 1)));
        link.close();

        settled(|| state.outbox.dropped() == 1).await;
        assert!(published_on(&bus, CONTROL_POSITION_SUBJECT).is_empty());
        assert_eq!(state.outbox.queued(), 0);
    }

    #[tokio::test]
    async fn the_outbox_drops_past_1024_and_counts_it() {
        let (state, bus) = linked().await;
        let (_link, _events, permit) = realm_owner(&state.registry);
        let accepted = (0..OUTBOUND_CAPACITY as u64 + 7)
            .filter(|sequence| {
                state
                    .outbox
                    .publish_control_position(position(&permit, sequence + 1))
            })
            .count();
        assert_eq!(accepted, OUTBOUND_CAPACITY, "the queue never grows");
        assert_eq!(state.outbox.queued(), OUTBOUND_CAPACITY);
        assert_eq!(state.outbox.dropped(), 7);

        settled(|| published_on(&bus, CONTROL_POSITION_SUBJECT).len() == OUTBOUND_CAPACITY).await;
        assert_eq!(state.outbox.queued(), 0);
        assert_eq!(state.outbox.dropped(), 7);
        let health = get_json(&state, "/stats/health").await;
        assert_eq!(health["feed_publish_dropped"], 7);

        bus.set_connected(false);
        assert!(
            !state
                .outbox
                .publish_control_position(position(&permit, 2_000)),
            "nothing is queued while the link is down"
        );
        assert_eq!(state.outbox.dropped(), 8);
    }

    /// Pulse's lookup responder on a bus: records the wallet of every lookup that reaches it and,
    /// once `answer` is set, answers each with the wallet's cluster (`c1` unless it was moved)
    /// for the session it asks about.
    struct PulseStandIn {
        asked: Arc<Mutex<Vec<String>>>,
        answering: Arc<AtomicBool>,
        moved: Arc<Mutex<HashMap<String, String>>>,
    }

    impl PulseStandIn {
        fn on(bus: &InProcessBus) -> Self {
            let asked = Arc::new(Mutex::new(Vec::new()));
            let answering = Arc::new(AtomicBool::new(false));
            let moved = Arc::new(Mutex::new(HashMap::new()));
            let (record, answer, clusters) = (
                Arc::clone(&asked),
                Arc::clone(&answering),
                Arc::clone(&moved),
            );
            bus.respond_to_requests(Arc::new(move |subject, payload| {
                let lookup =
                    ClusterLookup::decode(subject, payload).map_err(|_| RequestError::Invalid)?;
                record.lock().push(lookup.wallet.clone());
                if !answer.load(Ordering::SeqCst) {
                    return Err(RequestError::TimedOut);
                }
                let cluster_id = clusters
                    .lock()
                    .get(&lookup.wallet)
                    .cloned()
                    .unwrap_or_else(|| "c1".to_owned());
                Ok(PeerClusterSnapshot {
                    wallet: lookup.wallet,
                    session: lookup.session,
                    cluster_id,
                    realm: "main".into(),
                    pass: 1,
                    source_incarnation: "pulse-test".into(),
                    pending_takeovers: Vec::new(),
                    token_not_before: Some(0),
                    assignment_version: 1,
                }
                .encode_to_vec())
            }));
            Self {
                asked,
                answering,
                moved,
            }
        }

        fn answer(&self) {
            self.answering.store(true, Ordering::SeqCst);
        }

        /// Pulse places `wallet` in `cluster` from now on.
        fn move_to(&self, wallet: &str, cluster: &str) {
            self.moved
                .lock()
                .insert(wallet.to_owned(), cluster.to_owned());
        }

        fn asked(&self, wallet: &str) -> usize {
            self.asked
                .lock()
                .iter()
                .filter(|asked| *asked == wallet)
                .count()
        }
    }

    /// Review focus 1, through the announce call: the reconciliation cycle restarts when the
    /// shared link comes back, as it did on the connector's own link. A socket that holds no
    /// island gets one within two rounds (announced to the island subscriber, which asks Pulse
    /// and hands the minted island to the socket's mailbox), and an assigned one is announced at
    /// once instead of at its next 30 s sweep, even after an outage that fell between two rounds.
    #[tokio::test(start_paused = true)]
    async fn reconciliation_restarts_when_the_link_returns() {
        const FRESH: &str = "0x00000000000000000000000000000000000000f1";
        const ASSIGNED: &str = "0x00000000000000000000000000000000000000f2";
        let bus = Arc::new(InProcessBus::new());
        let pulse = PulseStandIn::on(&bus);
        let jobs = Jobs::new();
        let state = state_with_island_feed(Settings::for_tests(), bus.clone(), &jobs).await;
        let (_fresh, mut fresh_events, _) = state.registry.on_peer_connected(FRESH, SESSION);
        let (assigned, _assigned_events, _) = state.registry.on_peer_connected(ASSIGNED, SESSION);
        assigned.assignment_sent();

        // The first pass and its follow-ups (0, 5 and 10 s) announce both; the round at 15 s
        // only the unassigned one, the next full sweep being due at 40 s. Pulse does not answer
        // yet, so the unassigned socket holds no island.
        tokio::time::sleep(Duration::from_secs(11)).await;
        let (fresh_before, assigned_before) = (pulse.asked(FRESH), pulse.asked(ASSIGNED));
        assert!(fresh_before >= 1 && assigned_before >= 1);
        tokio::time::sleep(Duration::from_secs(5)).await;
        assert!(pulse.asked(FRESH) > fresh_before);
        assert_eq!(pulse.asked(ASSIGNED), assigned_before);

        // Down from 16 s to 19 s, between the rounds at 15 s and 20 s; Pulse answers from now on.
        bus.set_connected(false);
        pulse.answer();
        tokio::time::sleep(Duration::from_secs(3)).await;
        let (fresh_down, assigned_down) = (pulse.asked(FRESH), pulse.asked(ASSIGNED));
        assert_eq!(assigned_down, assigned_before);
        assert!(
            fresh_events.try_recv().is_err(),
            "no island reached the unassigned socket before the link came back"
        );

        bus.set_connected(true);
        tokio::time::sleep(2 * RECONCILE_INTERVAL).await;
        assert!(
            pulse.asked(FRESH) > fresh_down,
            "a socket without an island is re-announced within two rounds"
        );
        assert!(
            pulse.asked(ASSIGNED) > assigned_down,
            "the link's return restarts the cycle with a full sweep"
        );
        let Ok(crate::archipelago::registry::SocketEvent::IslandChanged(bytes, _)) =
            fresh_events.try_recv()
        else {
            panic!("the socket without an island got none within two rounds of the link's return");
        };
        let island = IslandChangedMessage::decode(bytes.as_slice()).unwrap();
        assert_eq!(island.island_id, "island-c1");
        assert_eq!(
            island.conn_str,
            "livekit:wss://sfu.invalid?access_token=token-for-island-c1"
        );
        assert!(bus.published().is_empty(), "an announce crosses no broker");
        jobs.shutdown(Duration::from_secs(5)).await;
    }

    /// The island in a socket's mailbox, if it holds one.
    fn island_in(events: &mut SocketEvents) -> Option<IslandChangedMessage> {
        match events.try_recv() {
            Ok(crate::archipelago::registry::SocketEvent::IslandChanged(bytes, _)) => {
                Some(IslandChangedMessage::decode(bytes.as_slice()).unwrap())
            }
            _ => None,
        }
    }

    /// A move Pulse makes while the link is down is lost with its cluster change, and a socket
    /// that already holds an island is not one the 5 s rounds retry. The link's return restarts
    /// the cycle with a full sweep, so the assigned socket is announced too and receives the
    /// island Pulse moved it to: the assignment itself, not only a lookup.
    #[tokio::test(start_paused = true)]
    async fn a_move_lost_while_the_link_was_down_reaches_an_assigned_socket_on_its_return() {
        const MOVED: &str = "0x00000000000000000000000000000000000000f3";
        let bus = Arc::new(InProcessBus::new());
        let pulse = PulseStandIn::on(&bus);
        pulse.answer();
        let jobs = Jobs::new();
        let state = state_with_island_feed(Settings::for_tests(), bus.clone(), &jobs).await;
        let (socket, mut events, _) = state.registry.on_peer_connected(MOVED, SESSION);

        // The first pass and its follow-ups (0, 5 and 10 s) hand the socket island-c1, which its
        // writer forwards: from then on the 5 s rounds leave it alone, the next full sweep being
        // due at 40 s.
        tokio::time::sleep(Duration::from_secs(11)).await;
        let first = island_in(&mut events).expect("the socket got its first island");
        assert_eq!(first.island_id, "island-c1");
        socket.assignment_sent();

        // Down from 16 s to 19 s; Pulse moves the wallet to c2 meanwhile, and the cluster change
        // it publishes for the move never reaches this process.
        tokio::time::sleep(Duration::from_secs(5)).await;
        bus.set_connected(false);
        pulse.move_to(MOVED, "c2");
        tokio::time::sleep(Duration::from_secs(3)).await;
        assert!(
            island_in(&mut events).is_none(),
            "nothing moved the socket while down"
        );

        // Within half a second: the readiness sample's sweep, not the 5 s round at 20 s.
        bus.set_connected(true);
        tokio::time::sleep(Duration::from_millis(500)).await;
        let moved = island_in(&mut events)
            .expect("the assigned socket got the island it moved to once the link returned");
        assert_eq!(moved.island_id, "island-c2");
        assert_eq!(
            moved.conn_str,
            "livekit:wss://sfu.invalid?access_token=token-for-island-c2"
        );
        assert_eq!(moved.from_island_id.as_deref(), Some("island-c1"));
        assert_eq!(state.peers.island_of(MOVED).as_deref(), Some("island-c2"));
        assert!(bus.published().is_empty(), "an announce crosses no broker");
        jobs.shutdown(Duration::from_secs(5)).await;
    }

    /// Nothing is announced while the link is down, even in the middle of a sweep: a link that
    /// drops while sessions wait under the 10 ms pacing abandons the sweep at the next
    /// announcement (each lookup would fail), and its return announces them all again. The bus
    /// records every lookup the subscriber attempts, answered or not, so an announcement made
    /// while the link is down would show.
    #[tokio::test(start_paused = true)]
    async fn nothing_is_announced_while_the_link_is_down() {
        const SESSIONS: usize = 50;
        let bus = Arc::new(InProcessBus::new());
        let jobs = Jobs::new();
        let state = state_with_island_feed(Settings::for_tests(), bus.clone(), &jobs).await;
        let wallets: HashSet<String> = (1..=SESSIONS)
            .map(|index| format!("0x{index:040x}"))
            .collect();
        let _links: Vec<_> = wallets
            .iter()
            .map(|wallet| state.registry.on_peer_connected(wallet, SESSION))
            .collect();
        let asked_since = |from: usize| -> Vec<String> {
            bus.requested()[from..]
                .iter()
                .filter_map(|(subject, payload)| ClusterLookup::decode(subject, payload).ok())
                .map(|lookup| lookup.wallet)
                .collect()
        };

        // Up from the start, the first sweep announces one session every 10 ms. Down at 115 ms,
        // between two announcements: the readiness poll (every 100 ms) next samples the link at
        // 200 ms, so only the check made before each announcement can stop the sweep sooner.
        tokio::time::sleep(Duration::from_millis(115)).await;
        bus.set_connected(false);
        let at_drop = bus.requested().len();
        assert!(
            (5..SESSIONS).contains(&at_drop),
            "the link drops mid-sweep: {at_drop} announced"
        );
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(
            asked_since(at_drop),
            Vec::<String>::new(),
            "announced while the link was down"
        );

        bus.set_connected(true);
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(
            asked_since(at_drop).into_iter().collect::<HashSet<_>>(),
            wallets,
            "the link's return announces every live session again"
        );
        jobs.shutdown(Duration::from_secs(5)).await;
    }

    /// An assignment goes only to its named session, even with a newer socket present.
    #[tokio::test]
    async fn the_island_sockets_route_only_to_the_named_session() {
        let (state, bus) = linked().await;
        let (_older, mut older_events, _) = state.registry.on_peer_connected(WALLET, SESSION);
        let newer_session = "0x000000000000000000000000000000000000000c";
        let (_newer, mut newer_events, _) = state.registry.on_peer_connected(WALLET, newer_session);
        let assignment = |island: &str| IslandChangedMessage {
            island_id: island.into(),
            conn_str: "livekit:wss://example.invalid?access_token=t".into(),
            ..Default::default()
        };

        assert_eq!(
            route_island(&state, WALLET, SESSION, &assignment("island-session")),
            Routed::Delivered
        );
        assert!(older_events.try_recv().is_ok());
        assert!(newer_events.try_recv().is_err());
        assert_eq!(state.feed.delivered_count(), 1);
        assert_eq!(
            state.peers.island_of(WALLET).as_deref(),
            Some("island-session")
        );

        // What the island subscriber counts as undelivered: no socket of that wallet, or none
        // under that session, in this process.
        let elsewhere = "0x000000000000000000000000000000000000000d";
        assert_eq!(
            state.deliver_island(elsewhere, SESSION, &assignment("island-x"), None),
            IslandDelivery::Undelivered
        );
        assert_eq!(
            state.deliver_island(WALLET, elsewhere, &assignment("island-x"), None),
            IslandDelivery::Undelivered
        );
        assert_eq!(state.feed.delivered_count(), 1);
        assert!(bus.published().is_empty());
    }
}
