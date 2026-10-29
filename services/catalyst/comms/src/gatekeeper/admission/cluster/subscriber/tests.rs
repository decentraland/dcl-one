use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use super::*;
use crate::gatekeeper::bus::{DisabledBus, InProcessBus, PublishOutcome, RequestError};

const WALLET: &str = "0x1111111111111111111111111111111111111111";
const SESSION_A: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SESSION_B: &str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const SESSION_C: &str = "0xcccccccccccccccccccccccccccccccccccccccc";

#[test]
fn gateway_errors_discard_private_downstream_details() {
    const CANARY: &str = "cluster-gateway-private-material-canary";
    let error = GatewayError::new(CANARY);
    assert_eq!(error.to_string(), "cluster gateway operation failed");
    assert_eq!(format!("{error:?}"), "GatewayError");
    assert!(!error.to_string().contains(CANARY));
    assert!(!format!("{error:?}").contains(CANARY));
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MintCall {
    wallet: String,
    session: Option<String>,
    room: String,
    ttl_seconds: u64,
    not_before_unix: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RemoveCall {
    room: String,
    identity: String,
    revoke_before: Option<i64>,
}

#[derive(Default)]
struct FakeGateway {
    safety: Option<super::super::safety::SafetyStore>,
    remove_errors: Mutex<VecDeque<GatewayError>>,
    room_creations: AtomicUsize,
    access_pause: Mutex<Option<Arc<Notify>>>,
    access_entered: Notify,
    remove_pause: Mutex<Option<Arc<Notify>>>,
    remove_entered: Notify,
    denied: Mutex<HashSet<String>>,
    deny_error: AtomicBool,
    mint_error: AtomicBool,
    minted: Mutex<Vec<MintCall>>,
    removals: Mutex<Vec<RemoveCall>>,
    remove_outcomes: Mutex<VecDeque<Result<Removal, String>>>,
    holds: Mutex<HashMap<String, bool>>,
    held_sessions: Mutex<HashMap<String, String>>,
    holds_error: AtomicBool,
    holds_calls: AtomicUsize,
    mint_parked: AtomicBool,
    mint_pause: Mutex<Option<Arc<Notify>>>,
    mint_entered: Notify,
    fences: Mutex<Vec<TokenFence>>,
}

impl FakeGateway {
    fn minted(&self) -> Vec<MintCall> {
        self.minted.lock().unwrap().clone()
    }

    fn removals(&self) -> Vec<RemoveCall> {
        self.removals.lock().unwrap().clone()
    }

    fn fences(&self) -> Vec<TokenFence> {
        self.fences.lock().unwrap().clone()
    }

    fn queue_removals(&self, outcomes: impl IntoIterator<Item = Result<Removal, String>>) {
        *self.remove_outcomes.lock().unwrap() = outcomes.into_iter().collect();
    }
}

#[async_trait]
impl ClusterGateway for FakeGateway {
    fn safety_store(&self) -> Option<super::super::safety::SafetyStore> {
        self.safety.clone()
    }
    async fn ensure_recovery_room(&self, _: &str) -> Result<(), GatewayError> {
        self.room_creations.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    async fn is_denied(&self, wallet: &str) -> Result<bool, GatewayError> {
        self.access_entered.notify_one();
        let pause = self.access_pause.lock().unwrap().clone();
        if let Some(pause) = pause {
            pause.notified().await;
        }
        if self.deny_error.load(Ordering::Relaxed) {
            return Err(GatewayError::new("ban store unreachable"));
        }
        Ok(self.denied.lock().unwrap().contains(wallet))
    }

    async fn mint_connection_string(
        &self,
        _admission: &crate::gatekeeper::admission::cluster::fence::PulseAdmissionStamp,
        wallet: &str,
        session: Option<&str>,
        room: &str,
        ttl_seconds: u64,
        not_before_unix: Option<u64>,
    ) -> Result<String, GatewayError> {
        self.mint_entered.notify_one();
        let pause = self.mint_pause.lock().unwrap().clone();
        if let Some(pause) = pause {
            pause.notified().await;
        }
        if self.mint_parked.load(Ordering::Relaxed) {
            futures::future::pending::<()>().await;
        }
        if self.mint_error.load(Ordering::Relaxed) {
            return Err(GatewayError::new("livekit refused the mint"));
        }
        self.minted.lock().unwrap().push(MintCall {
            wallet: wallet.to_string(),
            session: session.map(str::to_string),
            room: room.to_string(),
            ttl_seconds,
            not_before_unix,
        });
        Ok(format!("livekit:wss://sfu?access_token=token-for-{room}"))
    }

    async fn remove_participant(
        &self,
        room: &str,
        identity: &str,
        revoke_tokens_minted_before: Option<i64>,
    ) -> Result<Removal, GatewayError> {
        self.removals.lock().unwrap().push(RemoveCall {
            room: room.to_string(),
            identity: identity.to_string(),
            revoke_before: revoke_tokens_minted_before,
        });
        self.remove_entered.notify_one();
        let pause = self.remove_pause.lock().unwrap().clone();
        if let Some(pause) = pause {
            pause.notified().await;
        }
        if let Some(error) = self.remove_errors.lock().unwrap().pop_front() {
            return Err(error);
        }
        match self.remove_outcomes.lock().unwrap().pop_front() {
            Some(Ok(removal)) => Ok(removal),
            Some(Err(error)) => Err(GatewayError::new(error)),
            None => Ok(Removal::Removed),
        }
    }

    async fn mint_fenced_connection_string(
        &self,
        _admission: &crate::gatekeeper::admission::cluster::fence::PulseAdmissionStamp,
        wallet: &str,
        room: &str,
        ttl_seconds: u64,
        fence: &TokenFence,
        not_before_unix: Option<u64>,
    ) -> Result<String, GatewayError> {
        self.fences.lock().unwrap().push(fence.clone());
        self.mint_connection_string(
            _admission,
            wallet,
            Some(&fence.owner_session),
            room,
            ttl_seconds,
            not_before_unix,
        )
        .await
    }

    async fn holds_participant(&self, room: &str, _identity: &str) -> Result<bool, GatewayError> {
        self.holds_calls.fetch_add(1, Ordering::Relaxed);
        if self.holds_error.load(Ordering::Relaxed) {
            return Err(GatewayError::new("livekit lookup failed"));
        }
        Ok(self
            .holds
            .lock()
            .unwrap()
            .get(room)
            .copied()
            .unwrap_or(false))
    }

    async fn island_occupant(
        &self,
        room: &str,
        wallet: &str,
    ) -> Result<IslandOccupant, GatewayError> {
        let held = self.holds_participant(room, wallet).await?;
        if let Some(session) = self.held_sessions.lock().unwrap().get(room) {
            return Ok(IslandOccupant::Session(session.clone()));
        }
        Ok(if held {
            IslandOccupant::Unattributed
        } else {
            IslandOccupant::Absent
        })
    }
}

/// One assignment the subscriber handed to the connector's sockets.
#[derive(Debug, Clone, PartialEq)]
struct Delivered {
    wallet: String,
    session: String,
    message: IslandChangedMessage,
}

/// The connector's sockets as the subscriber sees them: every assignment, in the order it came.
#[derive(Default)]
struct RecordingSockets(Mutex<Vec<Delivered>>);

impl IslandSockets for RecordingSockets {
    fn deliver_island(
        &self,
        wallet: &str,
        session: &str,
        assignment: &IslandChangedMessage,

        _pulse_admission: Option<
            &crate::gatekeeper::admission::cluster::fence::PulseAdmissionStamp,
        >,
    ) -> IslandDelivery {
        self.0.lock().unwrap().push(Delivered {
            wallet: wallet.to_owned(),
            session: session.to_owned(),
            message: assignment.clone(),
        });
        IslandDelivery::Delivered
    }
}

/// The connector's sockets in a process that holds none of the wallet's: every assignment is
/// handed over and comes back undelivered. Counts the hand-overs.
#[derive(Default)]
struct NoSocketHere(AtomicUsize);

impl IslandSockets for NoSocketHere {
    fn deliver_island(
        &self,
        _: &str,
        _: &str,
        _: &IslandChangedMessage,
        _pulse_admission: Option<
            &crate::gatekeeper::admission::cluster::fence::PulseAdmissionStamp,
        >,
    ) -> IslandDelivery {
        self.0.fetch_add(1, Ordering::SeqCst);
        IslandDelivery::Undelivered
    }
}

/// The value of an unlabelled series in a Prometheus exposition, if it was recorded.
fn metric_value(rendered: &str, name: &str) -> Option<f64> {
    rendered
        .lines()
        .filter(|line| !line.starts_with('#'))
        .find_map(|line| line.strip_prefix(name)?.strip_prefix(' ')?.parse().ok())
}

struct Harness {
    bus: Arc<InProcessBus>,
    gateway: Arc<FakeGateway>,
    subscriber: Arc<ClusterSubscriber>,
    source: Arc<Mutex<HashMap<String, PeerClusterSnapshot>>>,
    sockets: Arc<RecordingSockets>,
    lookups: Arc<Mutex<Vec<(String, Vec<u8>)>>>,
    _completion: NatsSubscription,
}

fn config() -> ClusterConfig {
    ClusterConfig {
        nats_url: Some("nats://127.0.0.1:4222".into()),
        takeover_retry_delay_ms: 0,
        self_hosted_token_quarantine: false,
        ..ClusterConfig::default()
    }
}

fn harness_with(config: ClusterConfig) -> Harness {
    harness_with_fence(config, None)
}

fn harness_with_fence(
    config: ClusterConfig,
    assignment_fence: Option<Arc<dyn AssignmentFence>>,
) -> Harness {
    harness_with_store(config, assignment_fence, None)
}

fn harness_with_store(
    config: ClusterConfig,
    assignment_fence: Option<Arc<dyn AssignmentFence>>,
    safety: Option<super::super::safety::SafetyStore>,
) -> Harness {
    let bus = Arc::new(InProcessBus::new());
    let gateway = Arc::new(FakeGateway {
        safety,
        ..Default::default()
    });
    let source = Arc::new(Mutex::new(HashMap::<String, PeerClusterSnapshot>::new()));
    let lookups = Arc::new(Mutex::new(Vec::new()));
    let replies = source.clone();
    let asked = lookups.clone();
    bus.respond_to_requests(Arc::new(move |subject, payload| {
        asked
            .lock()
            .unwrap()
            .push((subject.to_owned(), payload.to_vec()));
        let lookup = ClusterLookup::decode(subject, payload).map_err(|_| RequestError::Invalid)?;
        let current = replies.lock().unwrap();
        let snapshot = current
            .get(&lookup.wallet)
            .filter(|s| s.session.as_bytes() == payload)
            .ok_or(RequestError::TimedOut)?;
        Ok(snapshot.encode_to_vec())
    }));
    let completed = source.clone();
    let completion = bus.subscribe(
        crate::protocol::bus::cluster::Completion::SUBJECTS,
        None,
        Arc::new(move |subject, payload| {
            let Some(completion) =
                crate::protocol::bus::cluster::Completion::decode(subject, payload)
            else {
                return;
            };
            let mut source = completed.lock().unwrap();
            let Some(plan) = source.get_mut(&completion.wallet) else {
                return;
            };
            if let Some(index) = plan
                .pending_takeovers
                .iter()
                .position(|p| p.takeover_id == completion.completion.takeover_id)
            {
                plan.pending_takeovers.remove(index);
                plan.token_not_before = Some(
                    plan.token_not_before
                        .unwrap_or(0)
                        .max(completion.completion.revoke_before),
                );
            }
        }),
    );
    let subscriber = ClusterSubscriber::new_with_assignment_fence(
        bus.clone(),
        gateway.clone(),
        Arc::new(ClusterPeerState::default()),
        config,
        assignment_fence,
    );
    *subscriber.test_clock.lock().unwrap() =
        Some((now_unix_ms().max(0) as u64, tokio::time::Instant::now()));
    let sockets = Arc::new(RecordingSockets::default());
    subscriber.attach_sockets(sockets.clone());
    subscriber.start();
    Harness {
        bus,
        gateway,
        subscriber,
        source,
        sockets,
        lookups,
        _completion: completion,
    }
}

fn harness() -> Harness {
    harness_with(config())
}

/// A link over another that records what the subscriber asks of it: every subscription with its
/// queue group, and the subject of every request.
struct RecordingLink<L> {
    link: L,
    subscribed: Mutex<Vec<(String, Option<String>)>>,
    requested: Mutex<Vec<String>>,
}

impl<L> RecordingLink<L> {
    fn over(link: L) -> Arc<Self> {
        Arc::new(Self {
            link,
            subscribed: Mutex::default(),
            requested: Mutex::default(),
        })
    }

    fn subscribed(&self) -> Vec<(String, Option<String>)> {
        self.subscribed.lock().unwrap().clone()
    }

    fn requested(&self) -> Vec<String> {
        self.requested.lock().unwrap().clone()
    }
}

#[async_trait]
impl<L: NatsBus> NatsBus for RecordingLink<L> {
    fn is_enabled(&self) -> bool {
        self.link.is_enabled()
    }

    fn is_connected(&self) -> bool {
        self.link.is_connected()
    }

    fn is_ready(&self) -> bool {
        self.link.is_ready()
    }

    fn connect(&self) {
        self.link.connect();
    }

    fn subscribe(
        &self,
        subject: &str,
        queue: Option<&str>,
        handler: NatsHandler,
    ) -> NatsSubscription {
        self.subscribed
            .lock()
            .unwrap()
            .push((subject.to_owned(), queue.map(str::to_owned)));
        self.link.subscribe(subject, queue, handler)
    }

    async fn publish(&self, subject: &str, payload: Vec<u8>) -> PublishOutcome {
        self.link.publish(subject, payload).await
    }

    async fn request(&self, subject: &str, payload: Vec<u8>) -> Result<Vec<u8>, RequestError> {
        self.requested.lock().unwrap().push(subject.to_owned());
        self.link.request(subject, payload).await
    }
}

struct FakeAssignmentFence {
    owner_session: String,
    previous: Option<String>,
    previous_expires_at: Option<u64>,
    commits: Arc<Mutex<Vec<AssignmentDocument>>>,
    commit_error: bool,
}

struct FakeRealmLease {
    fence: TokenFence,
    previous: Option<String>,
    previous_expires_at: Option<u64>,
    commits: Arc<Mutex<Vec<AssignmentDocument>>>,
    commit_error: bool,
}

#[async_trait]
impl AssignmentFence for FakeAssignmentFence {
    async fn lock_realm(
        &self,
        _wallet: &str,
        session: &str,
    ) -> Result<
        Box<dyn crate::gatekeeper::admission::cluster::fence::RealmAssignmentLease>,
        crate::gatekeeper::admission::cluster::fence::FenceError,
    > {
        if session != self.owner_session {
            return Err(crate::gatekeeper::admission::cluster::fence::FenceError::Conflict);
        }
        Ok(Box::new(FakeRealmLease {
            fence: TokenFence {
                audience: "deployment-a".into(),
                lane: "realm".into(),
                owner_session: session.into(),
                owner_epoch: 7,
                assignment_revision: 12,
                fencing_token: 3,
            },
            previous: self.previous.clone(),
            previous_expires_at: self.previous_expires_at,
            commits: self.commits.clone(),
            commit_error: self.commit_error,
        }))
    }
}

#[async_trait]
impl crate::gatekeeper::admission::cluster::fence::RealmAssignmentLease for FakeRealmLease {
    fn previous_pulse_admission(&self) -> Option<PulseAdmissionStamp> {
        Some(PulseAdmissionStamp {
            wallet: WALLET.into(),
            session: self.fence.owner_session.clone(),
            cluster_id: self.previous.as_ref()?.strip_prefix("island-")?.into(),
            realm: "main".into(),
            source_incarnation: "test-source".into(),
            assignment_version: 1,
            token_not_before: 0,
            credential_not_before: 0,
            safety_generation: 0,
            safety_token_floor: 0,
        })
    }
    fn token_fence(&self) -> TokenFence {
        self.fence.clone()
    }

    fn previous_island(&self) -> Option<String> {
        self.previous.clone()
    }

    fn previous_token_expires_at_unix(&self) -> Option<u64> {
        self.previous_expires_at
    }

    async fn commit(
        self: Box<Self>,
        assignment: AssignmentDocument,
    ) -> Result<(), crate::gatekeeper::admission::cluster::fence::FenceError> {
        if self.commit_error {
            return Err(crate::gatekeeper::admission::cluster::fence::FenceError::Conflict);
        }
        self.commits.lock().unwrap().push(assignment);
        Ok(())
    }
}

#[tokio::test]
async fn fenced_realm_assignment_uses_live_source_and_commits_without_legacy_publish() {
    let commits = Arc::new(Mutex::new(Vec::new()));
    let authority = Arc::new(FakeAssignmentFence {
        owner_session: SESSION_A.into(),
        previous: Some("island-old".into()),
        previous_expires_at: None,
        commits: commits.clone(),
        commit_error: false,
    });
    let h = harness_with_fence(config(), Some(authority));
    let change = change("new", SESSION_A);
    h.feed(WALLET, &change).await;

    assert!(h.delivered().is_empty());
    assert_eq!(h.gateway.removals(), Vec::new());
    assert_eq!(
        h.gateway.fences(),
        vec![TokenFence {
            audience: "deployment-a".into(),
            lane: "realm".into(),
            owner_session: SESSION_A.into(),
            owner_epoch: 7,
            assignment_revision: 12,
            fencing_token: 3,
        }]
    );
    let committed = commits.lock().unwrap();
    assert_eq!(committed.len(), 1);
    assert_eq!(committed[0].island_id, "island-new");
    assert_eq!(committed[0].from_island_id.as_deref(), Some("island-old"));
    assert!(fenced_token_is_fresh(committed[0].token_expires_at_unix));
}

#[tokio::test]
async fn fenced_takeover_unit_model_settles_before_committing_replacement() {
    let commits = Arc::new(Mutex::new(Vec::new()));
    let authority = Arc::new(FakeAssignmentFence {
        owner_session: SESSION_B.into(),
        previous: Some("island-old".into()),
        previous_expires_at: None,
        commits: commits.clone(),
        commit_error: false,
    });
    let h = harness_with_fence(config(), Some(authority));
    let mut change = change("new", SESSION_B);
    change.displaced_session = SESSION_A.into();
    change.displaced_cluster_id = "old".into();
    h.feed(WALLET, &change).await;

    assert_eq!(h.gateway.removals().len(), 1);
    assert_eq!(h.gateway.fences().len(), 1);
    assert_eq!(commits.lock().unwrap().len(), 1);
    assert_eq!(commits.lock().unwrap()[0].island_id, "island-new");
    assert!(h.delivered().is_empty());
}

#[tokio::test]
async fn fenced_hints_resolve_current_state_and_owner_conflict_never_falls_back() {
    let commits = Arc::new(Mutex::new(Vec::new()));
    let authority = Arc::new(FakeAssignmentFence {
        owner_session: SESSION_A.into(),
        previous: Some("island-old".into()),
        previous_expires_at: None,
        commits: commits.clone(),
        commit_error: false,
    });
    let h = harness_with_fence(config(), Some(authority));
    h.snapshot(WALLET, "different", SESSION_A);
    h.subscriber
        .process_cluster_change(WALLET, &change("new", SESSION_A))
        .await;
    h.snapshot(WALLET, "new", SESSION_B);
    h.subscriber
        .process_cluster_change(WALLET, &change("new", SESSION_B))
        .await;

    assert_eq!(
        h.gateway.minted().len(),
        1,
        "the old hint may recover only current source authority"
    );
    assert_eq!(h.gateway.fences().len(), 1);
    assert_eq!(commits.lock().unwrap().len(), 1);
    assert_eq!(commits.lock().unwrap()[0].island_id, "island-different");
    assert!(h.delivered().is_empty());
}

#[tokio::test]
async fn stale_lease_commit_exposes_neither_assignment_nor_legacy_frame() {
    let commits = Arc::new(Mutex::new(Vec::new()));
    let authority = Arc::new(FakeAssignmentFence {
        owner_session: SESSION_A.into(),
        previous: Some("island-old".into()),
        previous_expires_at: None,
        commits: commits.clone(),
        commit_error: true,
    });
    let h = harness_with_fence(config(), Some(authority));
    h.feed(WALLET, &change("new", SESSION_A)).await;

    assert_eq!(h.gateway.minted().len(), 1);
    assert!(commits.lock().unwrap().is_empty());
    assert!(h.delivered().is_empty());
}

#[tokio::test]
async fn a_fenced_connect_refills_a_cleared_claim_without_a_legacy_publish() {
    let commits = Arc::new(Mutex::new(Vec::new()));
    let authority = Arc::new(FakeAssignmentFence {
        owner_session: SESSION_A.into(),
        previous: None,
        previous_expires_at: None,
        commits: commits.clone(),
        commit_error: false,
    });
    let h = harness_with_fence(config(), Some(authority));
    h.snapshot(WALLET, "current", SESSION_A);
    h.gateway
        .held_sessions
        .lock()
        .unwrap()
        .insert("island-current".into(), SESSION_A.into());

    h.connect(WALLET, SESSION_A).await;

    assert!(h.delivered().is_empty());
    assert_eq!(h.gateway.fences().len(), 1);
    let committed = commits.lock().unwrap();
    assert_eq!(committed.len(), 1);
    assert_eq!(committed[0].island_id, "island-current");
    assert_eq!(committed[0].from_island_id, None);
    assert!(fenced_token_is_fresh(committed[0].token_expires_at_unix));
}

#[tokio::test]
async fn a_fenced_connect_does_not_reissue_an_existing_current_assignment() {
    let commits = Arc::new(Mutex::new(Vec::new()));
    let authority = Arc::new(FakeAssignmentFence {
        owner_session: SESSION_A.into(),
        previous: Some("island-current".into()),
        previous_expires_at: None,
        commits: commits.clone(),
        commit_error: false,
    });
    let h = harness_with_fence(config(), Some(authority));
    h.snapshot(WALLET, "current", SESSION_A);
    h.gateway
        .held_sessions
        .lock()
        .unwrap()
        .insert("island-current".into(), SESSION_A.into());

    h.connect(WALLET, SESSION_A).await;

    assert!(h.gateway.minted().is_empty());
    assert!(h.gateway.fences().is_empty());
    assert!(commits.lock().unwrap().is_empty());
    assert!(h.delivered().is_empty());
}

#[tokio::test]
async fn a_fresh_same_room_token_suppresses_absent_or_different_session_recovery() {
    for occupant in [None, Some(SESSION_B)] {
        let commits = Arc::new(Mutex::new(Vec::new()));
        let authority = Arc::new(FakeAssignmentFence {
            owner_session: SESSION_A.into(),
            previous: Some("island-current".into()),
            previous_expires_at: Some(u64::MAX),
            commits: commits.clone(),
            commit_error: false,
        });
        let h = harness_with_fence(config(), Some(authority));
        h.snapshot(WALLET, "current", SESSION_A);
        if let Some(occupant) = occupant {
            h.gateway
                .held_sessions
                .lock()
                .unwrap()
                .insert("island-current".into(), occupant.into());
        }

        h.connect(WALLET, SESSION_A).await;

        assert!(h.gateway.minted().is_empty());
        assert!(commits.lock().unwrap().is_empty());
        assert!(h.delivered().is_empty());
    }
}

#[tokio::test]
async fn an_expired_or_unstamped_same_room_token_is_renewed_without_the_exact_participant() {
    for previous_expires_at in [Some(1), None] {
        for occupant in [None, Some(SESSION_B)] {
            let commits = Arc::new(Mutex::new(Vec::new()));
            let authority = Arc::new(FakeAssignmentFence {
                owner_session: SESSION_A.into(),
                previous: Some("island-current".into()),
                previous_expires_at,
                commits: commits.clone(),
                commit_error: false,
            });
            let h = harness_with_fence(config(), Some(authority));
            h.snapshot(WALLET, "current", SESSION_A);
            if let Some(occupant) = occupant {
                h.gateway
                    .held_sessions
                    .lock()
                    .unwrap()
                    .insert("island-current".into(), occupant.into());
            }

            h.connect(WALLET, SESSION_A).await;

            assert_eq!(h.gateway.fences().len(), 1, "{previous_expires_at:?}");
            let committed = commits.lock().unwrap();
            assert_eq!(committed.len(), 1, "{previous_expires_at:?}");
            assert!(fenced_token_is_fresh(committed[0].token_expires_at_unix));
            assert!(h.delivered().is_empty());
        }
    }
}

#[tokio::test]
async fn an_unknown_or_unreadable_occupant_blocks_even_expired_fenced_recovery() {
    for lookup_error in [false, true] {
        let commits = Arc::new(Mutex::new(Vec::new()));
        let authority = Arc::new(FakeAssignmentFence {
            owner_session: SESSION_A.into(),
            previous: Some("island-current".into()),
            previous_expires_at: Some(1),
            commits: commits.clone(),
            commit_error: false,
        });
        let h = harness_with_fence(config(), Some(authority));
        h.snapshot(WALLET, "current", SESSION_A);
        if lookup_error {
            h.gateway.holds_error.store(true, Ordering::Relaxed);
        } else {
            h.gateway
                .holds
                .lock()
                .unwrap()
                .insert("island-current".into(), true);
        }

        h.connect(WALLET, SESSION_A).await;

        assert!(h.gateway.minted().is_empty(), "{lookup_error}");
        assert!(commits.lock().unwrap().is_empty(), "{lookup_error}");
        assert!(h.delivered().is_empty(), "{lookup_error}");
    }
}

#[tokio::test]
async fn a_source_change_during_fenced_recovery_discards_the_private_token() {
    let commits = Arc::new(Mutex::new(Vec::new()));
    let authority = Arc::new(FakeAssignmentFence {
        owner_session: SESSION_A.into(),
        previous: None,
        previous_expires_at: None,
        commits: commits.clone(),
        commit_error: false,
    });
    let h = harness_with_fence(config(), Some(authority));
    h.snapshot(WALLET, "current", SESSION_A);
    let release = Arc::new(Notify::new());
    *h.gateway.mint_pause.lock().unwrap() = Some(release.clone());
    h.subscriber.announce_connect(WALLET, SESSION_A);
    h.gateway.mint_entered.notified().await;
    h.snapshot(WALLET, "moved", SESSION_A);
    release.notify_one();
    h.subscriber.settle().await;

    assert_eq!(h.gateway.minted().len(), 1);
    assert!(commits.lock().unwrap().is_empty());
    assert!(h.delivered().is_empty());
}

#[tokio::test]
async fn a_source_change_during_fenced_cluster_mint_cannot_be_committed() {
    let commits = Arc::new(Mutex::new(Vec::new()));
    let authority = Arc::new(FakeAssignmentFence {
        owner_session: SESSION_A.into(),
        previous: Some("island-old".into()),
        previous_expires_at: None,
        commits: commits.clone(),
        commit_error: false,
    });
    let h = harness_with_fence(config(), Some(authority));
    h.snapshot(WALLET, "current", SESSION_A);
    let release = Arc::new(Notify::new());
    *h.gateway.mint_pause.lock().unwrap() = Some(release.clone());
    h.bus.inject(
        &format!("peer.{WALLET}.cluster_change"),
        &change("current", SESSION_A).encode_to_vec(),
    );
    h.gateway.mint_entered.notified().await;
    h.snapshot(WALLET, "moved", SESSION_A);
    release.notify_one();
    h.subscriber.settle().await;

    assert_eq!(h.gateway.minted().len(), 1);
    assert!(commits.lock().unwrap().is_empty());
    assert!(h.delivered().is_empty());
}

/// An authority that holds no row for the wallet, which is every unmodified v3 client.
struct UnownedFence;

#[async_trait]
impl AssignmentFence for UnownedFence {
    async fn lock_realm(
        &self,
        _wallet: &str,
        _session: &str,
    ) -> Result<
        Box<dyn crate::gatekeeper::admission::cluster::fence::RealmAssignmentLease>,
        crate::gatekeeper::admission::cluster::fence::FenceError,
    > {
        Err(crate::gatekeeper::admission::cluster::fence::FenceError::Unowned)
    }
}

struct RejectedFence(crate::gatekeeper::admission::cluster::fence::FenceError);

#[async_trait]
impl AssignmentFence for RejectedFence {
    async fn lock_realm(
        &self,
        _wallet: &str,
        _session: &str,
    ) -> Result<
        Box<dyn crate::gatekeeper::admission::cluster::fence::RealmAssignmentLease>,
        crate::gatekeeper::admission::cluster::fence::FenceError,
    > {
        Err(self.0)
    }
}

#[tokio::test]
async fn a_wallet_no_v4_socket_claimed_is_assigned_on_the_legacy_path() {
    let h = harness_with_fence(config(), Some(Arc::new(UnownedFence)));
    h.feed(WALLET, &change("new", SESSION_A)).await;

    assert!(h.gateway.fences().is_empty());
    assert_eq!(h.gateway.minted().len(), 1);
    let published = h.delivered();
    assert_eq!(published.len(), 1);
    assert_eq!(delivered_to(&published[0]), (WALLET, SESSION_A));
    assert_eq!(published[0].message.island_id, "island-new");
}

#[tokio::test]
async fn an_unowned_connect_alone_uses_legacy_recovery() {
    let h = harness_with_fence(config(), Some(Arc::new(UnownedFence)));
    h.snapshot(WALLET, "current", SESSION_A);

    h.connect(WALLET, SESSION_A).await;

    assert!(h.gateway.fences().is_empty());
    assert_eq!(h.gateway.minted().len(), 1);
    assert_eq!(h.delivered()[0].message.island_id, "island-current");
}

#[tokio::test]
async fn rejected_fenced_recovery_never_falls_back_to_a_legacy_token() {
    for error in [
        crate::gatekeeper::admission::cluster::fence::FenceError::Conflict,
        crate::gatekeeper::admission::cluster::fence::FenceError::Unavailable,
    ] {
        let h = harness_with_fence(config(), Some(Arc::new(RejectedFence(error))));
        h.snapshot(WALLET, "current", SESSION_A);

        h.connect(WALLET, SESSION_A).await;

        assert!(h.gateway.minted().is_empty(), "{error:?}");
        assert!(h.gateway.fences().is_empty(), "{error:?}");
        assert!(h.delivered().is_empty(), "{error:?}");
    }
}

#[tokio::test]
async fn a_session_that_cannot_own_a_fenced_lane_never_asks_the_authority() {
    let commits = Arc::new(Mutex::new(Vec::new()));
    let authority = Arc::new(FakeAssignmentFence {
        owner_session: SESSION_A.into(),
        previous: Some("island-old".into()),
        previous_expires_at: None,
        commits: commits.clone(),
        commit_error: false,
    });
    let h = harness_with_fence(config(), Some(authority));
    h.feed(WALLET, &change("new", "")).await;

    assert!(h.gateway.fences().is_empty());
    assert!(commits.lock().unwrap().is_empty());
    assert!(h.delivered().is_empty());
    assert!(h.gateway.minted().is_empty());
}

#[tokio::test]
async fn subscriber_caps_programmatic_token_ttl_to_its_quarantine_budget() {
    let h = harness_with(ClusterConfig {
        island_token_ttl_seconds:
            crate::gatekeeper::admission::cluster::MAX_ISLAND_TOKEN_TTL_SECONDS + 1,
        ..config()
    });
    assert_eq!(
        h.subscriber.config.island_token_ttl_seconds,
        crate::gatekeeper::admission::cluster::MAX_ISLAND_TOKEN_TTL_SECONDS
    );
}

fn change(cluster_id: &str, session: &str) -> PeerClusterChange {
    PeerClusterChange {
        cluster_id: cluster_id.into(),
        realm: "main".into(),
        session: session.into(),
        displaced_session: String::new(),
        displaced_cluster_id: String::new(),
        pending_takeovers: Vec::new(),
        token_not_before: None,
        assignment_version: 0,
        source_incarnation: String::new(),
    }
}

impl Harness {
    fn snapshot(&self, wallet: &str, cluster: &str, session: &str) {
        let previous = self
            .source
            .lock()
            .unwrap()
            .get(&wallet.to_lowercase())
            .cloned();
        self.source.lock().unwrap().insert(
            wallet.to_lowercase(),
            PeerClusterSnapshot {
                wallet: wallet.to_lowercase(),
                session: session.to_lowercase(),
                cluster_id: cluster.into(),
                realm: "main".into(),
                pass: 1,
                source_incarnation: "test-source".into(),
                pending_takeovers: previous
                    .as_ref()
                    .map(|p| p.pending_takeovers.clone())
                    .unwrap_or_default(),
                token_not_before: previous
                    .as_ref()
                    .and_then(|p| p.token_not_before)
                    .or(Some(0)),
                assignment_version: previous
                    .as_ref()
                    .map(|p| p.assignment_version + 1)
                    .unwrap_or(1),
            },
        );
    }

    fn plan_takeover(&self, wallet: &str, change: &PeerClusterChange) {
        self.snapshot(wallet, &change.cluster_id, &change.session);
        if !change.displaced_session.is_empty()
            && self
                .source
                .lock()
                .unwrap()
                .get(wallet)
                .unwrap()
                .pending_takeovers
                .is_empty()
        {
            self.source
                .lock()
                .unwrap()
                .get_mut(wallet)
                .unwrap()
                .pending_takeovers = vec![
                crate::protocol::pulse::decentraland::pulse::PendingTakeover {
                    takeover_id: uuid::Uuid::new_v4().to_string(),
                    displaced_session: change.displaced_session.clone(),
                    displaced_cluster_id: change.displaced_cluster_id.clone(),
                    displaced_realm: change.realm.clone(),
                },
            ];
        }
    }

    async fn feed(&self, wallet: &str, change: &PeerClusterChange) {
        self.plan_takeover(wallet, change);
        self.bus.inject(
            &format!("peer.{wallet}.cluster_change"),
            &change.encode_to_vec(),
        );
        self.subscriber.settle().await;
    }

    async fn connect(&self, wallet: &str, session: &str) {
        self.subscriber.announce_connect(wallet, session);
        self.subscriber.settle().await;
    }

    fn delivered(&self) -> Vec<Delivered> {
        self.sockets.0.lock().unwrap().clone()
    }

    /// Every lookup the subscriber asked Pulse, as (subject, payload).
    fn lookups(&self) -> Vec<(String, Vec<u8>)> {
        self.lookups.lock().unwrap().clone()
    }
}

fn delivered_to(delivery: &Delivered) -> (&str, &str) {
    (delivery.wallet.as_str(), delivery.session.as_str())
}

/// This thread's `warn` and `error` lines, captured until the guard drops.
#[derive(Clone, Default)]
struct LogBuffer(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for LogBuffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A capture in force: this thread's subscriber, and a second live dispatcher beside it.
///
/// `tracing` caches a callsite's interest when the callsite is first hit. While one dispatcher
/// alone is live, a first hit on another test's thread asks only that thread's subscriber (none)
/// and caches "never", hiding the event from this capture. With two live, every first hit asks
/// all of them.
struct Capture {
    _default: tracing::subscriber::DefaultGuard,
    _second: tracing::Dispatch,
}

impl LogBuffer {
    fn capture() -> (Self, Capture) {
        let second = tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
        let buffer = Self::default();
        let writer = buffer.clone();
        let guard = tracing::subscriber::set_default(
            tracing_subscriber::fmt()
                .with_max_level(tracing::Level::WARN)
                .without_time()
                .with_ansi(false)
                .with_writer(move || writer.clone())
                .finish(),
        );
        (
            buffer,
            Capture {
                _default: guard,
                _second: second,
            },
        )
    }

    fn text(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }
}

/// The island subscriber hands the connector's socket the assignment it minted by call: the
/// legacy `IslandChangedMessage` addressed to the session, and nothing crosses the broker.
#[tokio::test]
async fn an_assignment_reaches_the_sockets_by_call() {
    let h = harness();
    let before = island_token_not_before(0, h.subscriber.admission_now_ms());
    h.feed(WALLET, &change("c1", SESSION_A)).await;

    let chosen = h.gateway.minted()[0].not_before_unix.unwrap();
    assert!(
        chosen >= before && chosen <= island_token_not_before(0, h.subscriber.admission_now_ms())
    );
    assert_eq!(
        h.gateway.minted(),
        vec![MintCall {
            wallet: WALLET.into(),
            session: Some(SESSION_A.into()),
            room: "island-c1".into(),
            ttl_seconds: 60,
            not_before_unix: Some(chosen),
        }]
    );

    let published = h.delivered();
    assert_eq!(published.len(), 1);
    assert_eq!(delivered_to(&published[0]), (WALLET, SESSION_A));
    assert_eq!(
        published[0].message,
        IslandChangedMessage {
            island_id: "island-c1".into(),
            conn_str: "livekit:wss://sfu?access_token=token-for-island-c1".into(),
            from_island_id: None,
            peers: HashMap::new(),
            assignment_context: None,
        }
    );
    assert!(
        h.bus.published().is_empty(),
        "nothing is published on engine.peer.* or anywhere else"
    );
    assert_eq!(published[0].message.island_id, "island-c1");
    assert_eq!(
        published[0].message.from_island_id, None,
        "a first assignment names no previous room rather than an empty one"
    );
    assert!(
        published[0].message.peers.is_empty(),
        "peers is empty by design: explorers read only conn_str"
    );
    assert!(published[0].message.conn_str.starts_with("livekit:"));
}

#[tokio::test]
async fn the_next_assignment_names_the_room_the_peer_came_from() {
    let h = harness();
    h.feed(WALLET, &change("c1", SESSION_A)).await;
    h.feed(WALLET, &change("c2", SESSION_A)).await;

    let published = h.delivered();
    assert_eq!(published[1].message.island_id, "island-c2");
    assert_eq!(
        published[1].message.from_island_id.as_deref(),
        Some("island-c1")
    );
}

/// Invalid internal assignments must not mint, evict or reach any socket.
#[tokio::test(flavor = "current_thread")]
async fn invalid_sessions_are_dropped_and_counted_without_side_effects() {
    let recorder = metrics_exporter_prometheus::PrometheusBuilder::new().build_recorder();
    let series = recorder.handle();
    let _recording = metrics::set_default_local_recorder(&recorder);
    let h = harness();
    for malformed in ["", "not-a-session", "0xABCDEF", "0x", "0x11.22"] {
        let mut event = change("c1", malformed);
        event.displaced_session = SESSION_B.into();
        event.displaced_cluster_id = "old".into();
        h.feed(WALLET, &event).await;
    }
    assert!(h.delivered().is_empty());
    assert!(h.gateway.minted().is_empty());
    assert!(h.gateway.removals().is_empty());
    assert!(h.subscriber.peers().assignment(WALLET).is_none());
    assert_eq!(
        metric_value(&series.render(), "comms_cluster_island_undelivered_total"),
        Some(5.0)
    );
}

/// The direct public entry point shares the feed's reject-before-side-effects rule.
#[tokio::test(flavor = "current_thread")]
async fn direct_processing_drops_invalid_sessions_before_mint_or_eviction() {
    let recorder = metrics_exporter_prometheus::PrometheusBuilder::new().build_recorder();
    let series = recorder.handle();
    let _recording = metrics::set_default_local_recorder(&recorder);
    let h = harness();
    for displaced in [false, true] {
        for malformed in ["", "not-a-session", "0xABCDEF", "0x", "0x11.22"] {
            let mut event = change("c1", malformed);
            if displaced {
                event.displaced_session = SESSION_B.into();
                event.displaced_cluster_id = "old".into();
            }
            h.subscriber.process_cluster_change(WALLET, &event).await;
        }
    }
    assert!(h.delivered().is_empty());
    assert!(h.gateway.minted().is_empty());
    assert!(h.gateway.removals().is_empty());
    assert!(h.subscriber.peers().assignment(WALLET).is_none());
    assert_eq!(
        metric_value(&series.render(), "comms_cluster_island_undelivered_total"),
        Some(10.0)
    );
}

#[tokio::test]
async fn an_upper_cased_session_is_lower_cased_before_it_is_used() {
    let h = harness();
    h.feed(WALLET, &change("c1", &SESSION_A.to_uppercase()))
        .await;
    assert_eq!(delivered_to(&h.delivered()[0]), (WALLET, SESSION_A));
}

#[tokio::test]
async fn the_wallet_is_taken_from_the_subject_and_lower_cased() {
    let h = harness();
    let mixed = "0xAAAAaaaaBBBBbbbbCCCCccccDDDDddddEEEEeeee";
    h.feed(mixed, &change("c1", SESSION_A)).await;
    assert_eq!(h.gateway.minted()[0].wallet, mixed.to_lowercase());
    assert_eq!(h.delivered()[0].wallet, mixed.to_lowercase());
}

#[tokio::test]
async fn a_banned_wallet_is_skipped_entirely() {
    let h = harness();
    h.gateway.denied.lock().unwrap().insert(WALLET.into());
    h.feed(WALLET, &change("c1", SESSION_A)).await;
    assert!(h.gateway.minted().is_empty());
    assert!(h.delivered().is_empty());
}

#[tokio::test]
async fn an_access_check_failure_lets_the_peer_through() {
    let h = harness();
    h.gateway.deny_error.store(true, Ordering::Relaxed);
    h.feed(WALLET, &change("c1", SESSION_A)).await;
    assert_eq!(
        h.gateway.minted().len(),
        1,
        "the gate fails open: a background feed has nobody to report the error to"
    );
}

#[tokio::test]
async fn an_empty_cluster_id_is_dropped_before_anything_is_minted() {
    let h = harness();
    h.feed(WALLET, &change("", SESSION_A)).await;
    assert!(h.gateway.minted().is_empty());
    assert!(h.delivered().is_empty());
}

/// One handler reads the feed, so a payload it cannot decode is one error line, naming the wallet
/// of its subject. The capture is this test's thread, on a current-thread runtime.
#[tokio::test(flavor = "current_thread")]
async fn a_malformed_change_is_logged_once() {
    let (logs, _guard) = LogBuffer::capture();
    let h = harness();
    h.bus.inject(
        &format!("peer.{WALLET}.cluster_change"),
        &[0xff, 0xff, 0xff],
    );
    let logged = logs.text();
    let lines: Vec<&str> = logged.lines().collect();
    assert_eq!(lines.len(), 1, "{logged}");
    assert!(lines[0].contains("ERROR"), "{logged}");
    assert!(lines[0].contains(&format!("wallet={WALLET}")), "{logged}");
}

#[tokio::test]
async fn an_undecodable_payload_costs_only_that_message() {
    let h = harness();
    h.bus.inject(
        &format!("peer.{WALLET}.cluster_change"),
        &[0xff, 0xff, 0xff],
    );
    h.subscriber.settle().await;
    h.feed(WALLET, &change("c1", SESSION_A)).await;
    assert_eq!(h.gateway.minted().len(), 1);
}

#[tokio::test]
async fn a_takeover_evicts_the_displaced_session_before_the_replacement_is_minted() {
    let h = harness();
    let mut takeover = change("c1", SESSION_B);
    takeover.displaced_session = SESSION_A.into();
    takeover.displaced_cluster_id = "c1".into();
    h.feed(WALLET, &takeover).await;

    let removals = h.gateway.removals();
    assert_eq!(removals.len(), 1);
    assert_eq!(removals[0].room, "island-c1");
    assert_eq!(
        removals[0].identity, WALLET,
        "LiveKit identities are wallets, so the wallet is what is removed"
    );

    let minted = h.gateway.minted();
    let boundary = minted[0].not_before_unix.expect("a revocation boundary");
    assert!(
        boundary >= removals[0].revoke_before.unwrap() as u64,
        "the revocation cutoff is a lower bound on the replacement nbf"
    );
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!(
        now >= boundary,
        "the consumer waits until the whole-second floor plus allowance; this adapter test does not establish Cloud revocation"
    );
}

#[tokio::test]
async fn a_displaced_session_that_already_left_is_not_a_failure() {
    let h = harness();
    h.gateway.queue_removals([Ok(Removal::Absent)]);
    let mut takeover = change("c2", SESSION_B);
    takeover.displaced_session = SESSION_A.into();
    takeover.displaced_cluster_id = "c1".into();
    h.feed(WALLET, &takeover).await;
    assert_eq!(
        h.gateway.removals().len(),
        1,
        "an absence ends the attempts"
    );
    assert_eq!(h.gateway.minted().len(), 1);
}

#[tokio::test]
async fn an_eviction_that_never_completes_does_not_mint_a_replacement() {
    let h = harness();
    h.gateway.queue_removals([
        Err("timeout".to_string()),
        Err("timeout".to_string()),
        Err("timeout".to_string()),
    ]);
    let mut takeover = change("c2", SESSION_B);
    takeover.displaced_session = SESSION_A.into();
    takeover.displaced_cluster_id = "c1".into();
    h.feed(WALLET, &takeover).await;
    assert_eq!(
        h.gateway.removals().len(),
        1,
        "unknown dispatched removal is never retried"
    );
    assert!(h.gateway.minted().is_empty());
}

#[tokio::test]
async fn a_takeover_that_names_no_displaced_cluster_removes_nothing() {
    let h = harness();
    let mut takeover = change("c2", SESSION_B);
    takeover.displaced_session = SESSION_A.into();
    h.feed(WALLET, &takeover).await;
    assert!(h.gateway.removals().is_empty());
    assert!(h.gateway.minted().is_empty());
}

#[tokio::test]
async fn a_stale_takeover_hint_recovers_only_the_current_authoritative_assignment() {
    let h = harness();
    let mut takeover = change("stale", SESSION_B);
    takeover.displaced_session = SESSION_A.into();
    takeover.displaced_cluster_id = "old".into();
    h.snapshot(WALLET, "current", SESSION_B);
    h.bus.inject(
        &format!("peer.{WALLET}.cluster_change"),
        &takeover.encode_to_vec(),
    );
    h.subscriber.settle().await;
    assert!(h.gateway.removals().is_empty());
    assert_eq!(h.gateway.minted().len(), 1);
    assert_eq!(h.delivered()[0].message.island_id, "island-current");
}

#[tokio::test(start_paused = true)]
async fn self_hosted_takeover_waits_out_old_tokens_removes_again_and_rechecks_source() {
    let mut cfg = config();
    cfg.self_hosted_token_quarantine = true;
    cfg.island_token_ttl_seconds = 1;
    let h = harness_with(cfg);
    let mut takeover = change("c1", SESSION_B);
    takeover.displaced_session = SESSION_A.into();
    takeover.displaced_cluster_id = "c1".into();
    h.plan_takeover(WALLET, &takeover);
    h.bus.inject(
        &format!("peer.{WALLET}.cluster_change"),
        &takeover.encode_to_vec(),
    );
    while h.gateway.removals().is_empty() {
        tokio::task::yield_now().await;
    }
    assert!(h.gateway.minted().is_empty());
    tokio::time::advance(Duration::from_secs(63)).await;
    h.subscriber.settle().await;
    assert_eq!(h.gateway.removals().len(), 2);
    assert_eq!(h.gateway.minted().len(), 1);
    assert_eq!(h.delivered().len(), 1);
}

/// On a paused clock the bound elapses only if the feed waits on a timer.
#[tokio::test(start_paused = true)]
async fn self_hosted_cross_room_takeover_does_not_wait_for_a_room_bound_token() {
    let mut cfg = config();
    cfg.self_hosted_token_quarantine = true;
    let h = harness_with(cfg);
    let mut takeover = change("c2", SESSION_B);
    takeover.displaced_session = SESSION_A.into();
    takeover.displaced_cluster_id = "c1".into();
    tokio::time::timeout(Duration::from_secs(10), h.feed(WALLET, &takeover))
        .await
        .expect("an old-room token cannot displace the owner in the new room");
    assert_eq!(h.gateway.removals().len(), 1);
    assert_eq!(h.gateway.minted().len(), 1);
    assert_eq!(h.delivered().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_source_change_during_self_hosted_quarantine_discards_the_candidate() {
    let mut cfg = config();
    cfg.self_hosted_token_quarantine = true;
    cfg.island_token_ttl_seconds = 1;
    let h = harness_with(cfg);
    let mut takeover = change("c1", SESSION_B);
    takeover.displaced_session = SESSION_A.into();
    takeover.displaced_cluster_id = "c1".into();
    h.plan_takeover(WALLET, &takeover);
    h.bus.inject(
        &format!("peer.{WALLET}.cluster_change"),
        &takeover.encode_to_vec(),
    );
    while h.gateway.removals().is_empty() {
        tokio::task::yield_now().await;
    }
    h.snapshot(WALLET, "newer", SESSION_B);
    tokio::time::advance(Duration::from_secs(63)).await;
    h.subscriber.settle().await;
    assert_eq!(h.gateway.removals().len(), 1);
    assert!(h.gateway.minted().is_empty());
    assert!(h.delivered().is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_newer_same_room_takeover_is_not_lost_behind_an_old_quarantine() {
    let mut cfg = config();
    cfg.self_hosted_token_quarantine = true;
    let h = harness_with(cfg);
    let mut first = change("c1", SESSION_B);
    first.displaced_session = SESSION_A.into();
    first.displaced_cluster_id = "c1".into();
    h.plan_takeover(WALLET, &first);
    h.bus.inject(
        &format!("peer.{WALLET}.cluster_change"),
        &first.encode_to_vec(),
    );
    while h.gateway.removals().is_empty() {
        tokio::task::yield_now().await;
    }

    let mut current = change("c1", SESSION_C);
    current.displaced_session = SESSION_B.into();
    current.displaced_cluster_id = "c1".into();
    h.snapshot(WALLET, "c1", SESSION_C);
    h.bus.inject(
        &format!("peer.{WALLET}.cluster_change"),
        &current.encode_to_vec(),
    );
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(122)).await;
    h.subscriber.settle().await;

    let assignments = h.delivered();
    assert_eq!(assignments.len(), 1);
    assert_eq!(assignments[0].session.as_str(), SESSION_C);
    assert_eq!(assignments[0].message.island_id, "island-c1");
}

#[tokio::test(start_paused = true)]
async fn an_accepted_but_stale_successor_cannot_cancel_the_current_quarantine() {
    let mut cfg = config();
    cfg.self_hosted_token_quarantine = true;
    cfg.island_token_ttl_seconds = 1;
    let h = harness_with(cfg);
    let mut current = change("c1", SESSION_B);
    current.displaced_session = SESSION_A.into();
    current.displaced_cluster_id = "c1".into();
    h.plan_takeover(WALLET, &current);
    h.bus.inject(
        &format!("peer.{WALLET}.cluster_change"),
        &current.encode_to_vec(),
    );
    while h.gateway.removals().is_empty() {
        tokio::task::yield_now().await;
    }

    let mut stale = change("c1", SESSION_C);
    stale.displaced_session = SESSION_B.into();
    stale.displaced_cluster_id = "c1".into();
    h.bus.inject(
        &format!("peer.{WALLET}.cluster_change"),
        &stale.encode_to_vec(),
    );
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(63)).await;
    h.subscriber.settle().await;

    let assignments = h.delivered();
    assert_eq!(assignments.len(), 1);
    assert_eq!(assignments[0].session.as_str(), SESSION_B);
    assert_eq!(h.gateway.removals().len(), 2);
}

/// A subscriber no connector attached its sockets to hands a minted assignment to nobody: it says
/// so once, at `warn`, counts it undelivered and records nothing, so an assignment no socket
/// received never becomes the next `from_island_id`.
#[tokio::test(flavor = "current_thread")]
async fn an_assignment_with_no_sockets_attached_reaches_nobody_and_records_nothing() {
    let (logs, _guard) = LogBuffer::capture();
    let recorder = metrics_exporter_prometheus::PrometheusBuilder::new().build_recorder();
    let series = recorder.handle();
    let _recording = metrics::set_default_local_recorder(&recorder);
    let h = harness();
    let bus = h.bus.clone();
    let gateway = h.gateway.clone();
    let subscriber = h.subscriber.clone();
    *subscriber.sockets.lock().unwrap() = None;
    let feed = |cluster: &str| {
        h.snapshot(WALLET, cluster, SESSION_A);
        bus.inject(
            &format!("peer.{WALLET}.cluster_change"),
            &change(cluster, SESSION_A).encode_to_vec(),
        );
    };
    for cluster in ["c1", "c2"] {
        feed(cluster);
        subscriber.settle().await;
    }
    assert_eq!(gateway.minted().len(), 2);
    assert!(subscriber.peers().assignment(WALLET).is_none());
    let rendered = series.render();
    assert_eq!(
        metric_value(&rendered, "comms_cluster_island_undelivered_total"),
        Some(2.0),
        "{rendered}"
    );
    assert_eq!(
        metric_value(&rendered, "comms_cluster_published_total"),
        None,
        "{rendered}"
    );
    let logged = logs.text();
    assert_eq!(
        logged.matches("WARN").count(),
        1,
        "one warning for the missing sockets, not one per assignment: {logged}"
    );

    let sockets = Arc::new(RecordingSockets::default());
    subscriber.attach_sockets(sockets.clone());
    feed("c3");
    subscriber.settle().await;
    let delivered = sockets.0.lock().unwrap().clone();
    assert_eq!(delivered.len(), 1);
    assert_eq!(delivered[0].message.island_id, "island-c3");
    assert_eq!(
        delivered[0].message.from_island_id, None,
        "an assignment no socket received must not become the next from_island_id"
    );
    assert_eq!(
        metric_value(&series.render(), "comms_cluster_published_total"),
        Some(1.0)
    );
    assert!(bus.published().is_empty());
}

/// An assignment the attached sockets do not take (this process holds no socket of that wallet
/// and session) is counted apart from a delivered one. `comms_cluster_published_total` counts
/// deliveries only and `comms_cluster_island_undelivered_total` the rest. It is still recorded,
/// as a submitted publish was, so the next assignment names it as the room the peer came from.
#[tokio::test(flavor = "current_thread")]
async fn an_assignment_no_socket_takes_is_counted_undelivered_and_still_recorded() {
    let recorder = metrics_exporter_prometheus::PrometheusBuilder::new().build_recorder();
    let series = recorder.handle();
    let _recording = metrics::set_default_local_recorder(&recorder);
    let h = harness();
    h.feed(WALLET, &change("c1", SESSION_A)).await;

    let elsewhere = Arc::new(NoSocketHere::default());
    h.subscriber.attach_sockets(elsewhere.clone());
    h.feed(WALLET, &change("c2", SESSION_A)).await;
    h.connect(WALLET, SESSION_A).await;
    assert_eq!(
        elsewhere.0.load(Ordering::SeqCst),
        2,
        "a move and a recovery"
    );
    let rendered = series.render();
    assert_eq!(
        metric_value(&rendered, "comms_cluster_published_total"),
        Some(1.0),
        "{rendered}"
    );
    assert_eq!(
        metric_value(&rendered, "comms_cluster_island_undelivered_total"),
        Some(2.0),
        "{rendered}"
    );
    assert_eq!(
        h.subscriber
            .peers()
            .assignment(WALLET)
            .map(|assignment| assignment.room)
            .as_deref(),
        Some("island-c2")
    );

    h.subscriber.attach_sockets(h.sockets.clone());
    h.feed(WALLET, &change("c3", SESSION_A)).await;
    let delivered = h.delivered();
    assert_eq!(delivered.len(), 2);
    assert_eq!(
        delivered[1].message.from_island_id.as_deref(),
        Some("island-c2")
    );
    assert_eq!(
        metric_value(&series.render(), "comms_cluster_published_total"),
        Some(2.0)
    );
}

#[tokio::test]
async fn a_mint_failure_delivers_nothing() {
    let h = harness();
    h.gateway.mint_error.store(true, Ordering::Relaxed);
    h.feed(WALLET, &change("c1", SESSION_A)).await;
    assert!(h.delivered().is_empty());
}

#[tokio::test]
async fn a_connect_for_a_wallet_with_no_known_cluster_re_announces_nothing() {
    let h = harness();
    h.connect(WALLET, SESSION_A).await;
    assert!(h.gateway.minted().is_empty());
    assert_eq!(h.gateway.holds_calls.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn a_connect_re_announces_the_last_island_to_the_connecting_session() {
    let h = harness();
    h.feed(WALLET, &change("c1", SESSION_A)).await;
    h.connect(WALLET, SESSION_A).await;

    let published = h.delivered();
    assert_eq!(published.len(), 2);
    assert_eq!(delivered_to(&published[1]), (WALLET, SESSION_A));
    assert_eq!(published[1].message.island_id, "island-c1");
    assert_eq!(
        published[1].message.from_island_id.as_deref(),
        Some("island-c1"),
        "the re-announcement names the room the peer was last told to join"
    );
}

/// The connector's announce is a call, not a subject: the subscriber asks Pulse for the session's
/// current cluster (one `peer.{wallet}.cluster_lookup`, the session as its payload) and hands the
/// island it mints to that session's socket.
#[tokio::test]
async fn announce_connect_asks_pulse_for_the_assignment() {
    let h = harness();
    h.snapshot(WALLET, "c7", SESSION_A);
    h.connect(&WALLET.to_uppercase().replacen("0X", "0x", 1), SESSION_A)
        .await;

    let lookups = h.lookups();
    assert_eq!(
        lookups.first(),
        Some(&(
            format!("peer.{WALLET}.cluster_lookup"),
            SESSION_A.as_bytes().to_vec()
        )),
        "a mixed-case wallet is asked about in lower case"
    );
    assert!(
        lookups.iter().all(|lookup| lookup == &lookups[0]),
        "every lookup of the recovery is about this wallet and session: {lookups:?}"
    );
    let delivered = h.delivered();
    assert_eq!(delivered.len(), 1);
    assert_eq!(delivered_to(&delivered[0]), (WALLET, SESSION_A));
    assert_eq!(delivered[0].message.island_id, "island-c7");
    assert!(h.bus.published().is_empty());
}

#[tokio::test]
async fn a_stopped_subscriber_ignores_announces() {
    let h = harness();
    h.snapshot(WALLET, "c7", SESSION_A);
    h.subscriber.stop().await;
    h.connect(WALLET, SESSION_A).await;
    assert!(h.lookups().is_empty());
    assert!(h.delivered().is_empty());

    let never_started = ClusterSubscriber::new(
        h.bus.clone(),
        h.gateway.clone(),
        Arc::new(ClusterPeerState::default()),
        config(),
    );
    never_started.attach_sockets(h.sockets.clone());
    never_started.announce_connect(WALLET, SESSION_A);
    never_started.settle().await;
    assert!(h.lookups().is_empty());
    assert!(h.delivered().is_empty());
}

#[tokio::test]
async fn a_connect_from_a_displaced_device_is_skipped() {
    let h = harness();
    h.feed(WALLET, &change("c1", SESSION_A)).await;
    h.connect(WALLET, SESSION_B).await;
    assert_eq!(
        h.delivered().len(),
        1,
        "handing the room back to a displaced device would put a stale identity beside the live one"
    );
    assert_eq!(h.gateway.holds_calls.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn a_connect_payload_that_is_not_a_session_key_cannot_query_authority() {
    let h = harness();
    h.feed(WALLET, &change("c1", SESSION_A)).await;
    h.connect(WALLET, "legacy-connector-payload").await;

    assert_eq!(h.delivered().len(), 1);
    assert_eq!(h.gateway.holds_calls.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn a_wallet_already_in_its_room_is_not_re_announced() {
    let h = harness();
    h.feed(WALLET, &change("c1", SESSION_A)).await;
    h.gateway
        .holds
        .lock()
        .unwrap()
        .insert("island-c1".into(), true);
    h.connect(WALLET, SESSION_A).await;
    assert_eq!(h.delivered().len(), 1);
}

#[tokio::test]
async fn a_participant_lookup_that_fails_suppresses_the_re_announcement() {
    let h = harness();
    h.feed(WALLET, &change("c1", SESSION_A)).await;
    h.gateway.holds_error.store(true, Ordering::Relaxed);
    h.connect(WALLET, SESSION_A).await;
    assert_eq!(
        h.delivered().len(),
        1,
        "this lookup fails closed: announcing a peer that is still in its room ends its session"
    );
}

#[tokio::test]
async fn a_snapshot_answers_a_reconnect_this_process_never_minted() {
    let h = harness();
    h.snapshot(WALLET, "c9", SESSION_A);
    h.connect(WALLET, SESSION_A).await;

    let published = h.delivered();
    assert_eq!(published.len(), 1);
    assert_eq!(published[0].message.island_id, "island-c9");
    assert_eq!(
        published[0].message.from_island_id, None,
        "this process minted nothing for the wallet, so it names no previous room"
    );
}

/// `serve` runs one island subscriber, so Pulse's feed arrives on one subscription, with no queue
/// group: every change reaches it once.
#[tokio::test]
async fn one_subscription_carries_the_cluster_feed() {
    let link = RecordingLink::over(InProcessBus::new());
    let subscriber = ClusterSubscriber::new(
        link.clone(),
        Arc::new(FakeGateway::default()),
        Arc::new(ClusterPeerState::default()),
        config(),
    );
    subscriber.start();
    assert_eq!(
        link.subscribed(),
        vec![("peer.*.cluster_change".to_owned(), None)]
    );
}

/// The handler queues one mint and one delivery without running network work inline.
#[tokio::test(flavor = "current_thread")]
async fn a_cluster_change_is_queued_then_minted_and_delivered_once() {
    let h = harness();
    h.snapshot(WALLET, "c1", SESSION_A);
    h.bus.inject(
        &format!("peer.{WALLET}.cluster_change"),
        &change("c1", SESSION_A).encode_to_vec(),
    );
    assert!(h.gateway.minted().is_empty(), "the mint is queued, not run");
    assert_eq!(h.subscriber.in_flight.load(Ordering::SeqCst), 1);
    h.subscriber.settle().await;
    assert_eq!(h.gateway.minted().len(), 1);
    assert_eq!(h.delivered().len(), 1);
    assert_eq!(h.delivered()[0].message.island_id, "island-c1");
}

#[tokio::test]
async fn a_lost_event_recovers_from_current_source_state() {
    let h = harness();
    h.snapshot(WALLET, "current", SESSION_A);
    h.connect(WALLET, SESSION_A).await;
    assert_eq!(h.delivered()[0].message.island_id, "island-current");
    assert!(
        h.gateway.removals().is_empty(),
        "lookup cannot replay takeover"
    );
}

#[tokio::test]
async fn an_unavailable_source_cannot_be_replaced_by_a_previous_assignment() {
    let h = harness();
    h.feed(WALLET, &change("old", SESSION_A)).await;
    h.source.lock().unwrap().clear();
    h.connect(WALLET, SESSION_A).await;
    assert_eq!(h.delivered().len(), 1);
    assert_eq!(h.gateway.holds_calls.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn snapshot_identity_and_source_fields_are_checked_before_minting() {
    for field in ["wallet", "session", "cluster", "source", "pass", "protobuf"] {
        let h = harness();
        h.snapshot(WALLET, "c1", SESSION_A);
        let mut snapshot = h.source.lock().unwrap().get(WALLET).unwrap().clone();
        match field {
            "wallet" => snapshot.wallet = SESSION_B.into(),
            "session" => snapshot.session = SESSION_B.into(),
            "cluster" => snapshot.cluster_id.clear(),
            "source" => snapshot.source_incarnation.clear(),
            "pass" => snapshot.pass = 0,
            _ => {}
        }
        h.bus.respond_to_requests(Arc::new(move |_, _| {
            Ok(if field == "protobuf" {
                vec![0xff]
            } else {
                snapshot.encode_to_vec()
            })
        }));
        h.connect(WALLET, SESSION_A).await;
        assert!(h.gateway.minted().is_empty(), "accepted invalid {field}");
    }
}

#[tokio::test]
async fn changes_at_the_source_during_mint_discard_the_token_without_a_local_event() {
    for field in [
        "owner", "cluster", "realm", "source", "rollback", "absent", "advance",
    ] {
        let h = harness();
        h.snapshot(WALLET, "c1", SESSION_A);
        h.source.lock().unwrap().get_mut(WALLET).unwrap().pass = 10;
        let release = Arc::new(Notify::new());
        *h.gateway.mint_pause.lock().unwrap() = Some(release.clone());
        h.subscriber.announce_connect(WALLET, SESSION_A);
        h.gateway.mint_entered.notified().await;
        {
            let mut source = h.source.lock().unwrap();
            let snapshot = source.get_mut(WALLET).unwrap();
            match field {
                "owner" => snapshot.session = SESSION_B.into(),
                "cluster" => snapshot.cluster_id = "c2".into(),
                "realm" => snapshot.realm = "other".into(),
                "source" => snapshot.source_incarnation = "restarted".into(),
                "rollback" => snapshot.pass = 9,
                "advance" => snapshot.pass = 11,
                "absent" => {
                    source.clear();
                }
                _ => unreachable!(),
            }
        }
        release.notify_one();
        h.subscriber.settle().await;
        assert_eq!(h.gateway.minted().len(), 1);
        assert_eq!(
            h.delivered().len(),
            usize::from(field == "advance"),
            "{field}"
        );
        assert!(h.gateway.removals().is_empty());
    }
}

#[tokio::test]
async fn another_sessions_announcement_cannot_cancel_the_current_owners_recovery() {
    let h = harness();
    h.snapshot(WALLET, "c1", SESSION_A);
    let release = Arc::new(Notify::new());
    *h.gateway.mint_pause.lock().unwrap() = Some(release.clone());
    h.subscriber.announce_connect(WALLET, SESSION_A);
    h.gateway.mint_entered.notified().await;
    h.subscriber.announce_connect(WALLET, SESSION_B);
    for session in [SESSION_A, SESSION_B].into_iter().cycle().take(100) {
        h.subscriber.announce_connect(WALLET, session);
    }
    assert_eq!(h.subscriber.in_flight.load(Ordering::SeqCst), 2);
    release.notify_one();
    h.subscriber.settle().await;
    assert_eq!(
        h.delivered().len(),
        1,
        "an unverified socket announcement cannot cancel recovery for Pulse's actual owner"
    );
    assert_eq!(h.gateway.minted()[0].session.as_deref(), Some(SESSION_A));
    assert_eq!(h.subscriber.queue.tracked_wallets(), 0);
}

#[tokio::test]
async fn a_source_owner_change_rejects_a_held_mint_before_the_new_session_recovers() {
    let h = harness();
    h.snapshot(WALLET, "c1", SESSION_A);
    let release = Arc::new(Notify::new());
    *h.gateway.mint_pause.lock().unwrap() = Some(release.clone());
    h.subscriber.announce_connect(WALLET, SESSION_A);
    h.gateway.mint_entered.notified().await;
    h.snapshot(WALLET, "c2", SESSION_B);
    h.subscriber.announce_connect(WALLET, SESSION_B);
    *h.gateway.mint_pause.lock().unwrap() = None;
    release.notify_one();
    h.subscriber.settle().await;
    assert_eq!(h.gateway.minted().len(), 2);
    let assignments = h.delivered();
    assert_eq!(assignments.len(), 1);
    assert_eq!(assignments[0].session.as_str(), SESSION_B);
    assert_eq!(assignments[0].message.island_id, "island-c2");
    assert_eq!(h.subscriber.queue.tracked_wallets(), 0);
}

/// A cluster change for the wallet, or a stop, cancels every recovery the wallet holds: neither
/// held recovery delivers, and a change delivers only its own assignment.
#[tokio::test]
async fn a_cluster_change_or_stop_cancels_all_held_recovery_attempts_for_a_wallet() {
    for event in ["change", "stop"] {
        let h = harness();
        h.snapshot(WALLET, "c1", SESSION_A);
        h.gateway.mint_parked.store(true, Ordering::Relaxed);
        h.subscriber.announce_connect(WALLET, SESSION_A);
        h.gateway.mint_entered.notified().await;
        h.subscriber.announce_connect(WALLET, SESSION_B);
        let expected: &[&str] = match event {
            "change" => {
                h.gateway.mint_parked.store(false, Ordering::Relaxed);
                h.snapshot(WALLET, "c2", SESSION_B);
                h.bus.inject(
                    &format!("peer.{WALLET}.cluster_change"),
                    &change("c2", SESSION_B).encode_to_vec(),
                );
                &["island-c2"]
            }
            "stop" => {
                h.subscriber.stop().await;
                &[]
            }
            _ => unreachable!(),
        };
        tokio::time::timeout(Duration::from_secs(1), h.subscriber.settle())
            .await
            .unwrap();
        let minted: Vec<String> = h.gateway.minted().into_iter().map(|m| m.room).collect();
        assert_eq!(minted, expected, "{event}");
        let delivered = h.delivered();
        assert_eq!(delivered.len(), expected.len(), "{event}");
        for (delivery, room) in delivered.iter().zip(expected) {
            assert_eq!(delivery.message.island_id, *room);
            assert_eq!(delivered_to(delivery), (WALLET, SESSION_B));
        }
        assert_eq!(h.subscriber.queue.tracked_wallets(), 0);
    }
}

#[tokio::test(start_paused = true)]
async fn repeated_connects_coalesce_and_a_stalled_mint_has_a_recovery_deadline() {
    let h = harness();
    h.snapshot(WALLET, "c1", SESSION_A);
    h.gateway.mint_parked.store(true, Ordering::Relaxed);
    h.subscriber.announce_connect(WALLET, SESSION_A);
    h.gateway.mint_entered.notified().await;
    for _ in 0..100 {
        h.subscriber.announce_connect(WALLET, SESSION_A);
    }
    assert_eq!(h.subscriber.in_flight.load(Ordering::SeqCst), 1);
    tokio::time::advance(RECOVERY_DEADLINE).await;
    h.subscriber.settle().await;
    assert!(h.delivered().is_empty());
    assert_eq!(h.subscriber.queue.tracked_wallets(), 0);
    h.gateway.mint_parked.store(false, Ordering::Relaxed);
    h.connect(WALLET, SESSION_A).await;
    assert_eq!(
        h.delivered().len(),
        1,
        "the expired attempt must release ownership"
    );
}

#[tokio::test]
async fn a_room_join_or_failed_participant_check_during_mint_suppresses_recovery() {
    for lookup_error in [false, true] {
        let h = harness();
        h.snapshot(WALLET, "c1", SESSION_A);
        let release = Arc::new(Notify::new());
        *h.gateway.mint_pause.lock().unwrap() = Some(release.clone());
        h.subscriber.announce_connect(WALLET, SESSION_A);
        h.gateway.mint_entered.notified().await;
        if lookup_error {
            h.gateway.holds_error.store(true, Ordering::Relaxed);
        } else {
            h.gateway
                .holds
                .lock()
                .unwrap()
                .insert("island-c1".into(), true);
        }
        release.notify_one();
        h.subscriber.settle().await;
        assert_eq!(h.gateway.minted().len(), 1);
        assert!(h.delivered().is_empty());
    }
}

#[tokio::test]
async fn an_attributed_displaced_occupant_allows_current_session_admission_without_removal() {
    let h = harness();
    h.snapshot(WALLET, "c1", SESSION_B);
    h.gateway
        .held_sessions
        .lock()
        .unwrap()
        .insert("island-c1".into(), SESSION_A.into());
    h.connect(WALLET, SESSION_B).await;
    assert_eq!(h.gateway.minted()[0].session.as_deref(), Some(SESSION_B));
    assert_eq!(h.delivered().len(), 1);
    assert!(h.gateway.removals().is_empty());
    h.gateway
        .held_sessions
        .lock()
        .unwrap()
        .insert("island-c1".into(), SESSION_B.into());
    h.connect(WALLET, SESSION_B).await;
    assert_eq!(h.gateway.minted().len(), 1);
    assert_eq!(h.delivered().len(), 1);
}

#[tokio::test]
async fn a_current_session_join_during_displaced_owner_recovery_discards_the_minted_token() {
    let h = harness();
    h.snapshot(WALLET, "c1", SESSION_B);
    h.gateway
        .held_sessions
        .lock()
        .unwrap()
        .insert("island-c1".into(), SESSION_A.into());
    let release = Arc::new(Notify::new());
    *h.gateway.mint_pause.lock().unwrap() = Some(release.clone());
    h.subscriber.announce_connect(WALLET, SESSION_B);
    h.gateway.mint_entered.notified().await;
    h.gateway
        .held_sessions
        .lock()
        .unwrap()
        .insert("island-c1".into(), SESSION_B.into());
    release.notify_one();
    h.subscriber.settle().await;
    assert_eq!(h.gateway.minted().len(), 1);
    assert!(h.delivered().is_empty());
    assert!(h.gateway.removals().is_empty());
}

/// Without `NATS_URL` the subscriber stays idle: `start` subscribes to nothing, it is not ready
/// (the disabled link reports ready, having nothing to wait for), and an announced session asks
/// Pulse nothing and mints nothing.
#[tokio::test]
async fn an_unconfigured_broker_leaves_the_subscriber_idle() {
    assert!(
        harness().subscriber.is_ready(),
        "a configured link is ready"
    );
    let link = RecordingLink::over(DisabledBus);
    let gateway = Arc::new(FakeGateway::default());
    let subscriber = ClusterSubscriber::new(
        link.clone(),
        gateway.clone(),
        Arc::new(ClusterPeerState::default()),
        config(),
    );
    subscriber.start();
    assert_eq!(link.subscribed(), Vec::new());
    assert!(!subscriber.is_ready());
    subscriber.announce_connect(WALLET, SESSION_A);
    subscriber.settle().await;
    assert_eq!(link.requested(), Vec::<String>::new());
    subscriber.stop().await;
    assert!(gateway.minted().is_empty());
}

#[tokio::test]
async fn a_stalled_wallet_cannot_accumulate_unbounded_cluster_work() {
    let h = harness();
    h.snapshot(WALLET, "held", SESSION_A);
    h.gateway.mint_parked.store(true, Ordering::Relaxed);
    h.bus.inject(
        &format!("peer.{WALLET}.cluster_change"),
        &change("held", SESSION_A).encode_to_vec(),
    );
    h.gateway.mint_entered.notified().await;
    for index in 0..5_000 {
        h.bus.inject(
            &format!("peer.{WALLET}.cluster_change"),
            &change(&format!("move-{index}"), SESSION_A).encode_to_vec(),
        );
    }
    assert!(
        h.subscriber.in_flight.load(Ordering::SeqCst) <= 16,
        "one stalled wallet retained the whole feed history"
    );
}

#[tokio::test(start_paused = true)]
async fn a_stalled_cluster_mint_expires_and_does_not_prevent_source_recovery() {
    let h = harness();
    h.snapshot(WALLET, "current", SESSION_A);
    h.gateway.mint_parked.store(true, Ordering::Relaxed);
    h.bus.inject(
        &format!("peer.{WALLET}.cluster_change"),
        &change("old", SESSION_A).encode_to_vec(),
    );
    h.gateway.mint_entered.notified().await;
    tokio::time::advance(wallet_queue::WORK_DEADLINE + Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    assert_eq!(
        h.subscriber.in_flight.load(Ordering::SeqCst),
        0,
        "cluster work held the wallet indefinitely"
    );
    assert_eq!(h.subscriber.queue.tracked_wallets(), 0);
    h.gateway.mint_parked.store(false, Ordering::Relaxed);
    h.connect(WALLET, SESSION_A).await;
    assert_eq!(h.delivered()[0].message.island_id, "island-current");
}

#[tokio::test(start_paused = true)]
async fn overload_releases_rejected_recovery_and_eventually_uses_the_current_source() {
    let h = harness();
    h.snapshot(WALLET, "old", SESSION_A);
    h.gateway.mint_parked.store(true, Ordering::Relaxed);
    h.bus.inject(
        &format!("peer.{WALLET}.cluster_change"),
        &change("old", SESSION_A).encode_to_vec(),
    );
    h.gateway.mint_entered.notified().await;
    for index in 0..100 {
        h.bus.inject(
            &format!("peer.{WALLET}.cluster_change"),
            &change(&format!("move-{index}"), SESSION_A).encode_to_vec(),
        );
    }
    h.snapshot(WALLET, "current", SESSION_B);
    h.subscriber.announce_connect(WALLET, SESSION_B);
    assert_eq!(
        h.subscriber.in_flight.load(Ordering::SeqCst),
        wallet_queue::MAX_WALLET_WORK
    );
    let ticket = h
        .subscriber
        .recovery
        .begin(WALLET, SESSION_B)
        .expect("a rejected queue entry must release the recovery coalescer");
    drop(ticket);
    tokio::time::advance(wallet_queue::WORK_DEADLINE).await;
    h.subscriber.settle().await;
    assert!(h.gateway.minted().is_empty());
    assert!(h.delivered().is_empty());
    h.gateway.mint_parked.store(false, Ordering::Relaxed);
    h.connect(WALLET, SESSION_B).await;
    let assignments = h.delivered();
    assert_eq!(assignments.len(), 1);
    assert_eq!(assignments[0].message.island_id, "island-current");
    assert_eq!(assignments[0].session.as_str(), SESSION_B);
    assert_eq!(h.subscriber.queue.outstanding_work(), 0);
}

#[tokio::test]
async fn invalid_or_oversized_feed_cannot_cancel_or_mutate_a_valid_recovery() {
    let h = harness();
    h.snapshot(WALLET, "current", SESSION_A);
    let release = Arc::new(Notify::new());
    *h.gateway.mint_pause.lock().unwrap() = Some(release.clone());
    h.subscriber.announce_connect(WALLET, SESSION_A);
    h.gateway.mint_entered.notified().await;
    let oversized = change(&"x".repeat(MAX_PAYLOAD_BYTES), SESSION_B).encode_to_vec();
    assert!(oversized.len() > MAX_PAYLOAD_BYTES);
    h.bus
        .inject(&format!("peer.{WALLET}.cluster_change"), &oversized);
    h.bus.inject(
        &format!("peer.{}.cluster_change", "x".repeat(MAX_SUBJECT_BYTES)),
        &change("bad", SESSION_B).encode_to_vec(),
    );
    h.bus.inject(
        "peer.invalid.cluster_change",
        &change("bad", SESSION_B).encode_to_vec(),
    );
    h.bus.inject(
        &format!("peer.{WALLET}.cluster_change"),
        &change("invalid-session", "not-a-session").encode_to_vec(),
    );
    assert_eq!(h.subscriber.in_flight.load(Ordering::SeqCst), 1);
    release.notify_one();
    h.subscriber.settle().await;
    assert_eq!(h.gateway.minted().len(), 1);
    assert_eq!(h.delivered()[0].message.island_id, "island-current");
}

#[tokio::test]
async fn all_concurrent_settle_waiters_observe_the_last_completion() {
    let h = harness();
    h.snapshot(WALLET, "current", SESSION_A);
    let release = Arc::new(Notify::new());
    *h.gateway.mint_pause.lock().unwrap() = Some(release.clone());
    h.bus.inject(
        &format!("peer.{WALLET}.cluster_change"),
        &change("current", SESSION_A).encode_to_vec(),
    );
    h.gateway.mint_entered.notified().await;
    let mut waiters = Vec::new();
    for _ in 0..8 {
        let subscriber = h.subscriber.clone();
        waiters.push(tokio::spawn(async move { subscriber.settle().await }));
    }
    tokio::task::yield_now().await;
    release.notify_one();
    tokio::time::timeout(Duration::from_secs(1), async {
        for waiter in waiters {
            waiter.await.unwrap();
        }
    })
    .await
    .expect("one completion must wake every drain waiter");
    assert_eq!(h.delivered().len(), 1);
}

#[tokio::test]
async fn stopping_unsubscribes_before_the_drain() {
    let h = harness();
    h.subscriber.stop().await;
    h.feed(WALLET, &change("c1", SESSION_A)).await;
    assert!(
        h.gateway.minted().is_empty(),
        "an event arriving after the stop is not minted: the wallet's next connect recovers it"
    );
}

/// A panic inside one item's work must not take the lane's drainer with it: the completion still
/// runs, so the in-flight count returns to zero and everything queued behind it still runs.
#[tokio::test]
async fn a_panicking_work_item_leaves_the_lane_draining() {
    let h = harness();
    let ran = Arc::new(AtomicBool::new(false));
    let flag = ran.clone();

    h.subscriber.spawn(WALLET.to_string(), |_| {
        Box::pin(async {
            panic!("a work item panicked");
        })
    });
    h.subscriber.spawn(WALLET.to_string(), move |_| {
        Box::pin(async move {
            flag.store(true, Ordering::SeqCst);
        })
    });
    tokio::time::timeout(Duration::from_secs(5), h.subscriber.settle())
        .await
        .expect("a panic must not strand the lane's in-flight count");

    assert!(
        ran.load(Ordering::SeqCst),
        "the item queued behind a panicking one still runs"
    );
    assert_eq!(h.subscriber.queue.tracked_wallets(), 0);
}

#[tokio::test]
async fn work_for_one_wallet_leaves_no_queue_entry_behind() {
    let h = harness();
    for cluster in ["c1", "c2", "c3"] {
        h.feed(WALLET, &change(cluster, SESSION_A)).await;
    }
    assert_eq!(h.subscriber.queue.tracked_wallets(), 0);
}

/// The lane is FIFO, and its position is taken on the delivery thread, so a burst for one wallet
/// publishes in arrival order and every `from_island_id` names the room the peer actually left.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_burst_of_old_hints_only_delivers_the_authoritative_current_assignment() {
    let h = harness();
    h.snapshot(WALLET, "c8", SESSION_A);
    for cluster in ["c1", "c2", "c3", "c4", "c5", "c6", "c7", "c8"] {
        h.bus.inject(
            &format!("peer.{WALLET}.cluster_change"),
            &change(cluster, SESSION_A).encode_to_vec(),
        );
    }
    h.subscriber.settle().await;
    let published = h.delivered();
    assert_eq!(published.len(), 8);
    assert!(
        published
            .iter()
            .all(|delivery| delivery.message.island_id == "island-c8")
    );
}

/// The drain is bounded, so one request with no timeout of its own cannot hold shutdown open until
/// the orchestrator kills the process.
#[tokio::test(start_paused = true)]
async fn stopping_gives_up_on_work_that_will_never_finish() {
    let h = harness_with(ClusterConfig {
        drain_timeout_ms: 5,
        ..config()
    });
    h.snapshot(WALLET, "c1", SESSION_A);
    h.gateway.mint_parked.store(true, Ordering::Relaxed);
    h.bus.inject(
        &format!("peer.{WALLET}.cluster_change"),
        &change("c1", SESSION_A).encode_to_vec(),
    );

    tokio::time::timeout(Duration::from_secs(5), h.subscriber.stop())
        .await
        .expect("a parked gateway call must not hold the stop open");
}

#[tokio::test(start_paused = true)]
async fn a_mint_released_after_stop_cannot_publish() {
    for drain_timeout_ms in [0, 5] {
        let h = harness_with(ClusterConfig {
            drain_timeout_ms,
            ..config()
        });
        h.snapshot(WALLET, "old", SESSION_A);
        let release = Arc::new(Notify::new());
        *h.gateway.mint_pause.lock().unwrap() = Some(release.clone());
        h.bus.inject(
            &format!("peer.{WALLET}.cluster_change"),
            &change("old", SESSION_A).encode_to_vec(),
        );
        h.gateway.mint_entered.notified().await;
        h.subscriber.stop().await;
        release.notify_one();
        h.subscriber.settle().await;
        assert!(
            h.delivered().is_empty(),
            "stopped mint published with drain={drain_timeout_ms}"
        );
        assert_eq!(h.subscriber.queue.outstanding_work(), 0);
    }
}

#[tokio::test(start_paused = true)]
async fn an_old_stop_cancels_only_its_incarnation_when_start_races_the_drain() {
    let h = harness_with(ClusterConfig {
        drain_timeout_ms: 50,
        ..config()
    });
    h.snapshot(WALLET, "old-active", SESSION_A);
    let old_release = Arc::new(Notify::new());
    *h.gateway.mint_pause.lock().unwrap() = Some(old_release.clone());
    for cluster in ["old-active", "old-queued"] {
        h.bus.inject(
            &format!("peer.{WALLET}.cluster_change"),
            &change(cluster, SESSION_A).encode_to_vec(),
        );
    }
    h.gateway.mint_entered.notified().await;
    let subscriber = h.subscriber.clone();
    let stopping = tokio::spawn(async move { subscriber.stop().await });
    tokio::task::yield_now().await;
    assert!(h.subscriber.lifecycle.lock().unwrap().owner.is_none());
    h.subscriber.start();
    let new_release = Arc::new(Notify::new());
    *h.gateway.mint_pause.lock().unwrap() = Some(new_release.clone());
    h.snapshot(WALLET, "new", SESSION_B);
    h.bus.inject(
        &format!("peer.{WALLET}.cluster_change"),
        &change("new", SESSION_B).encode_to_vec(),
    );
    tokio::time::advance(Duration::from_millis(50)).await;
    tokio::time::timeout(Duration::from_millis(1), stopping)
        .await
        .expect("old stop must not drain the new incarnation")
        .unwrap();
    h.gateway.mint_entered.notified().await;
    assert_eq!(h.subscriber.in_flight.load(Ordering::SeqCst), 1);
    assert!(h.delivered().is_empty());
    old_release.notify_one();
    new_release.notify_one();
    h.subscriber.settle().await;
    let assignments = h.delivered();
    assert_eq!(assignments.len(), 1);
    assert_eq!(assignments[0].message.island_id, "island-new");
    assert_eq!(assignments[0].session.as_str(), SESSION_B);
    assert_eq!(h.gateway.minted().len(), 1);
}

#[tokio::test]
async fn callbacks_cloned_before_unsubscribe_cannot_enter_a_restarted_subscriber() {
    let h = harness();
    let old_owner = h
        .subscriber
        .lifecycle
        .lock()
        .unwrap()
        .owner
        .as_ref()
        .unwrap()
        .clone();
    let stale = h.subscriber.guarded(old_owner, |me, owner, change| {
        me.handle_cluster_change(owner, change);
    });
    h.subscriber.stop().await;
    h.subscriber.start();
    h.snapshot(WALLET, "new", SESSION_B);
    let release = Arc::new(Notify::new());
    *h.gateway.mint_pause.lock().unwrap() = Some(release.clone());
    h.subscriber.announce_connect(WALLET, SESSION_B);
    h.gateway.mint_entered.notified().await;
    stale(
        &format!("peer.{WALLET}.cluster_change"),
        &change("old", SESSION_A).encode_to_vec(),
    );
    assert_eq!(h.subscriber.in_flight.load(Ordering::SeqCst), 1);
    release.notify_one();
    h.subscriber.settle().await;
    assert_eq!(h.gateway.minted().len(), 1);
    assert_eq!(h.delivered()[0].message.island_id, "island-new");
}

#[tokio::test]
async fn graceful_stop_allows_already_accepted_work_to_finish() {
    let h = harness();
    h.snapshot(WALLET, "current", SESSION_A);
    let release = Arc::new(Notify::new());
    *h.gateway.mint_pause.lock().unwrap() = Some(release.clone());
    h.bus.inject(
        &format!("peer.{WALLET}.cluster_change"),
        &change("current", SESSION_A).encode_to_vec(),
    );
    h.gateway.mint_entered.notified().await;
    let subscriber = h.subscriber.clone();
    let stopping = tokio::spawn(async move { subscriber.stop().await });
    tokio::task::yield_now().await;
    assert!(h.subscriber.lifecycle.lock().unwrap().owner.is_none());
    release.notify_one();
    stopping.await.unwrap();
    assert_eq!(h.delivered()[0].message.island_id, "island-current");
    assert_eq!(h.subscriber.in_flight.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_padded_connect_payload_is_not_a_session_key() {
    let h = harness();
    h.feed(WALLET, &change("c1", SESSION_A)).await;
    h.connect(WALLET, &format!(" {SESSION_B} ")).await;

    assert_eq!(h.delivered().len(), 1);
    assert_eq!(h.gateway.holds_calls.load(Ordering::Relaxed), 0);
}

#[test]
fn only_a_lower_cased_ephemeral_address_is_a_session_key() {
    assert!(is_session_key(SESSION_A));
    assert!(!is_session_key(&SESSION_A.to_uppercase()));
    assert!(!is_session_key("0x123"));
    assert!(!is_session_key(""));
    assert!(!is_session_key(
        "0xgggggggggggggggggggggggggggggggggggggggg"
    ));
}

#[test]
fn the_revocation_boundary_is_the_second_after_this_one() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let boundary = next_whole_second_unix();
    assert!(boundary == now + 1 || boundary == now + 2);
}

/// Pins the re-signing the takeover rests on: the payload survives untouched apart from `nbf`,
/// and the signature is still one the API secret verifies.
#[test]
fn moving_a_token_s_not_before_leaves_every_other_claim_alone() {
    let minted = crate::gatekeeper::livekit::AccessToken::new(
        "key",
        "secret",
        WALLET,
        crate::gatekeeper::livekit::join_grants("island-c1"),
    )
    .to_jwt()
    .unwrap();
    let moved =
        crate::gatekeeper::livekit::with_not_before(&minted, "secret", 1_700_000_000).unwrap();

    let decode = |jwt: &str| -> serde_json::Value {
        let payload = jwt.split('.').nth(1).unwrap();
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).unwrap()).unwrap()
    };
    let before = decode(&minted);
    let after = decode(&moved);
    assert_eq!(after["nbf"], 1_700_000_000u64);
    for claim in ["iss", "sub", "exp", "video"] {
        assert_eq!(
            before[claim], after[claim],
            "{claim} must survive untouched"
        );
    }

    let signed_again = crate::gatekeeper::livekit::sign_hs256(
        "secret",
        &URL_SAFE_NO_PAD
            .decode(moved.split('.').next().unwrap())
            .unwrap(),
        &URL_SAFE_NO_PAD
            .decode(moved.split('.').nth(1).unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        signed_again, moved,
        "the token must verify under the secret"
    );
}

#[tokio::test]
async fn poisoned_lane_registry_still_drains_new_work() {
    let h = harness();
    let queue = h.subscriber.queue.clone();
    queue.poison_for_test();
    let ran = Arc::new(AtomicBool::new(false));
    let flag = ran.clone();
    h.subscriber.spawn(WALLET.to_string(), move |_| {
        Box::pin(async move {
            flag.store(true, Ordering::SeqCst);
        })
    });
    tokio::time::timeout(Duration::from_secs(5), h.subscriber.settle())
        .await
        .unwrap();
    assert!(ran.load(Ordering::SeqCst));
    assert_eq!(queue.tracked_wallets(), 0);
}

#[test]
fn authoritative_floor_uses_whole_seconds_allowance_and_exclusive_sixty_second_limit() {
    assert_eq!(floor_wait_ms(105, 102_999), Some(3001));
    assert_eq!(floor_wait_ms(105, 106_000), Some(0));
    assert_eq!(floor_wait_ms(105, 46_000), None);
    assert_eq!(floor_wait_ms(105, 46_001), Some(59_999));
    assert_eq!(
        floor_wait_ms(crate::protocol::bus::cluster::MAX_CONSUMER_SECONDS + 1, 0),
        None
    );
}

#[tokio::test]
async fn changed_assignment_version_during_access_refuses_stale_work() {
    let h = harness();
    h.snapshot(WALLET, "current", SESSION_A);
    let pause = Arc::new(Notify::new());
    *h.gateway.access_pause.lock().unwrap() = Some(pause.clone());
    let subscriber = h.subscriber.clone();
    let work =
        tokio::spawn(async move { subscriber.process_peer_connect(WALLET, SESSION_A).await });
    h.gateway.access_entered.notified().await;
    h.source
        .lock()
        .unwrap()
        .get_mut(WALLET)
        .unwrap()
        .assignment_version += 1;
    pause.notify_one();
    work.await.unwrap();
    assert!(h.gateway.minted().is_empty());
    assert!(h.gateway.removals().is_empty());
    assert!(h.delivered().is_empty());
}

#[tokio::test(start_paused = true)]
async fn authoritative_floor_change_during_mint_discards_old_credential() {
    let h = harness();
    h.snapshot(WALLET, "current", SESSION_A);
    let pause = Arc::new(Notify::new());
    *h.gateway.mint_pause.lock().unwrap() = Some(pause.clone());
    let subscriber = h.subscriber.clone();
    let work =
        tokio::spawn(async move { subscriber.process_peer_connect(WALLET, SESSION_A).await });
    h.gateway.mint_entered.notified().await;
    h.source
        .lock()
        .unwrap()
        .get_mut(WALLET)
        .unwrap()
        .token_not_before = Some(now_unix_seconds() + 2);
    pause.notify_one();
    work.await.unwrap();
    assert_eq!(h.gateway.minted().len(), 1);
    assert!(h.delivered().is_empty());
}

fn retained_operation() -> crate::protocol::pulse::decentraland::pulse::PendingTakeover {
    crate::protocol::pulse::decentraland::pulse::PendingTakeover {
        takeover_id: "stable-operation".into(),
        displaced_session: SESSION_A.into(),
        displaced_cluster_id: "old".into(),
        displaced_realm: "main".into(),
    }
}

#[tokio::test(start_paused = true)]
async fn confirmed_remove_survives_changed_authority_and_does_not_repeat_rpc() {
    let h = harness();
    h.snapshot(WALLET, "current", SESSION_B);
    h.source
        .lock()
        .unwrap()
        .get_mut(WALLET)
        .unwrap()
        .pending_takeovers = vec![retained_operation()];
    let pause = Arc::new(Notify::new());
    *h.gateway.remove_pause.lock().unwrap() = Some(pause.clone());
    let subscriber = h.subscriber.clone();
    let work =
        tokio::spawn(async move { subscriber.process_peer_connect(WALLET, SESSION_B).await });
    h.gateway.remove_entered.notified().await;
    h.source
        .lock()
        .unwrap()
        .get_mut(WALLET)
        .unwrap()
        .assignment_version += 1;
    pause.notify_one();
    work.await.unwrap();
    assert!(h.gateway.minted().is_empty());
    assert!(h.delivered().is_empty());
    *h.gateway.remove_pause.lock().unwrap() = None;
    h.subscriber
        .reconcile_current(WALLET, SESSION_B, false, None)
        .await;
    assert_eq!(h.gateway.removals().len(), 1);
    assert_eq!(h.gateway.minted().len(), 1);
    assert_eq!(h.delivered().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn cancelled_dispatched_removal_stays_unknown_after_long_elapsed_time() {
    let h = harness();
    h.snapshot(WALLET, "current", SESSION_B);
    h.source
        .lock()
        .unwrap()
        .get_mut(WALLET)
        .unwrap()
        .pending_takeovers = vec![retained_operation()];
    let pause = Arc::new(Notify::new());
    *h.gateway.remove_pause.lock().unwrap() = Some(pause);
    let subscriber = h.subscriber.clone();
    let work =
        tokio::spawn(async move { subscriber.process_peer_connect(WALLET, SESSION_B).await });
    h.gateway.remove_entered.notified().await;
    work.abort();
    let _ = work.await;
    *h.gateway.remove_pause.lock().unwrap() = None;
    tokio::time::advance(Duration::from_secs(100 * 365 * 86400)).await;
    h.subscriber.process_peer_connect(WALLET, SESSION_B).await;
    assert_eq!(h.gateway.removals().len(), 1);
    assert!(h.gateway.minted().is_empty());
    assert!(h.delivered().is_empty());
}

#[tokio::test(start_paused = true)]
async fn definite_missing_room_creates_once_then_revalidates_and_removes_with_fresh_cutoff() {
    let h = harness();
    h.snapshot(WALLET, "current", SESSION_B);
    h.source
        .lock()
        .unwrap()
        .get_mut(WALLET)
        .unwrap()
        .pending_takeovers = vec![retained_operation()];
    h.gateway
        .remove_errors
        .lock()
        .unwrap()
        .push_back(GatewayError::RoomMissing);
    h.subscriber
        .reconcile_current(WALLET, SESSION_B, false, None)
        .await;
    let calls = h.gateway.removals();
    assert_eq!(calls.len(), 2);
    assert!(calls[1].revoke_before > calls[0].revoke_before);
    assert_eq!(h.gateway.room_creations.load(Ordering::SeqCst), 1);
    assert_eq!(h.gateway.minted().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn definite_rejections_are_bounded_and_do_not_become_unknown_receipts() {
    let h = harness();
    h.snapshot(WALLET, "current", SESSION_B);
    h.source
        .lock()
        .unwrap()
        .get_mut(WALLET)
        .unwrap()
        .pending_takeovers = vec![retained_operation()];
    h.gateway
        .remove_errors
        .lock()
        .unwrap()
        .extend([GatewayError::DefiniteRejection; 3]);
    h.subscriber
        .reconcile_current(WALLET, SESSION_B, false, None)
        .await;
    assert_eq!(h.gateway.removals().len(), 3);
    assert!(h.gateway.minted().is_empty());
    h.subscriber
        .reconcile_current(WALLET, SESSION_B, false, None)
        .await;
    assert_eq!(h.gateway.removals().len(), 4);
    assert_eq!(h.gateway.minted().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_backward_wall_clock_during_floor_wait_never_advertises_future_credentials() {
    let h = harness();
    h.snapshot(WALLET, "current", SESSION_A);
    let floor = h.subscriber.admission_now_ms() / 1000 + 2;
    h.source
        .lock()
        .unwrap()
        .get_mut(WALLET)
        .unwrap()
        .token_not_before = Some(floor);
    let subscriber = h.subscriber.clone();
    let work = tokio::spawn(async move {
        subscriber
            .reconcile_current(WALLET, SESSION_A, false, None)
            .await
    });
    while h.lookups().len() < 3 {
        tokio::task::yield_now().await;
    }
    {
        let mut clock = h.subscriber.test_clock.lock().unwrap();
        clock.as_mut().unwrap().0 -= 10_000;
    }
    tokio::time::advance(Duration::from_secs(3)).await;
    work.await.unwrap();
    assert!(h.gateway.minted().is_empty());
    assert!(h.delivered().is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_backward_wall_clock_during_mint_discards_the_signed_candidate() {
    let h = harness();
    h.snapshot(WALLET, "current", SESSION_A);
    let floor = 0;
    h.source
        .lock()
        .unwrap()
        .get_mut(WALLET)
        .unwrap()
        .token_not_before = Some(floor);
    let pause = Arc::new(Notify::new());
    *h.gateway.mint_pause.lock().unwrap() = Some(pause.clone());
    let subscriber = h.subscriber.clone();
    let work = tokio::spawn(async move {
        subscriber
            .reconcile_current(WALLET, SESSION_A, false, None)
            .await
    });
    h.gateway.mint_entered.notified().await;
    {
        let mut clock = h.subscriber.test_clock.lock().unwrap();
        clock.as_mut().unwrap().0 -= 5_000;
    }
    pause.notify_one();
    work.await.unwrap();
    assert_eq!(h.gateway.minted().len(), 1);
    assert!(h.delivered().is_empty());
}

#[tokio::test(start_paused = true)]
async fn positively_settled_late_removal_gets_a_fresh_cutoff_but_unknown_does_not() {
    let h = harness();
    h.snapshot(WALLET, "current", SESSION_B);
    h.source
        .lock()
        .unwrap()
        .get_mut(WALLET)
        .unwrap()
        .pending_takeovers = vec![retained_operation()];
    let pause = Arc::new(Notify::new());
    *h.gateway.remove_pause.lock().unwrap() = Some(pause.clone());
    let subscriber = h.subscriber.clone();
    let work = tokio::spawn(async move {
        subscriber
            .reconcile_current(WALLET, SESSION_B, true, None)
            .await
    });
    h.gateway.remove_entered.notified().await;
    tokio::time::advance(Duration::from_secs(8)).await;
    *h.gateway.remove_pause.lock().unwrap() = None;
    pause.notify_one();
    work.await.unwrap();
    let calls = h.gateway.removals();
    assert_eq!(calls.len(), 2);
    assert!(calls[1].revoke_before > calls[0].revoke_before);
    assert_eq!(h.delivered().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn unknown_cleanup_entering_during_final_lookup_closes_ready_and_stamp_delivery() {
    for stamp_check in [false, true] {
        let h = harness();
        h.snapshot(WALLET, "current", SESSION_A);
        let floor = if stamp_check {
            h.subscriber.admission_now_ms() / 1000 - 2
        } else {
            h.subscriber.admission_now_ms() / 1000 + 1
        };
        h.source
            .lock()
            .unwrap()
            .get_mut(WALLET)
            .unwrap()
            .token_not_before = Some(floor);
        let expected = h.source.lock().unwrap().get(WALLET).unwrap().clone();
        let replies = h.source.clone();
        let subscriber = Arc::downgrade(&h.subscriber);
        let calls = AtomicUsize::new(0);
        h.bus.respond_to_requests(Arc::new(move |_, _| {
            let n = calls.fetch_add(1, Ordering::SeqCst);
            if n == if stamp_check { 0 } else { 1 } {
                let subscriber = subscriber.upgrade().unwrap();
                subscriber
                    .receipts
                    .reserve(
                        "older-source",
                        WALLET,
                        &retained_operation(),
                        0,
                        subscriber.admission_now_ms() / 1000,
                        subscriber.config.island_token_ttl_seconds,
                    )
                    .unwrap();
            }
            Ok(replies.lock().unwrap().get(WALLET).unwrap().encode_to_vec())
        }));
        if stamp_check {
            let stamp = PulseAdmissionStamp::from_snapshot(&expected, floor).unwrap();
            assert!(!h.subscriber.verify_stamp(&stamp).await);
        } else {
            assert!(h.subscriber.ready_floor(&expected).await.is_none());
        }
        assert!(!h.subscriber.receipts.safe_to_mint(WALLET));
        assert!(h.gateway.minted().is_empty());
        assert!(h.delivered().is_empty());
    }
}

#[tokio::test(start_paused = true)]
async fn settled_cleanup_obeys_conservative_allowance_boundary() {
    let mut observed = Vec::new();
    for offset_ms in [-1_i64, 0, 1, 1500] {
        let h = harness();
        h.snapshot(WALLET, "current", SESSION_B);
        h.source
            .lock()
            .unwrap()
            .get_mut(WALLET)
            .unwrap()
            .pending_takeovers = vec![retained_operation()];
        let pause = Arc::new(Notify::new());
        *h.gateway.remove_pause.lock().unwrap() = Some(pause.clone());
        let subscriber = h.subscriber.clone();
        let work = tokio::spawn(async move {
            subscriber
                .reconcile_current(WALLET, SESSION_B, true, None)
                .await
        });
        h.gateway.remove_entered.notified().await;
        let original = h.gateway.removals()[0].revoke_before.unwrap() as u64;
        let target = (original * 1000 - 1000) as i64 + offset_ms;
        let remaining = target as u64 - h.subscriber.admission_now_ms();
        tokio::time::advance(Duration::from_millis(remaining)).await;
        *h.gateway.remove_pause.lock().unwrap() = None;
        pause.notify_one();
        work.await.unwrap();
        let removals = h.gateway.removals();
        let completions: Vec<_> = h
            .bus
            .published()
            .iter()
            .filter_map(|(subject, payload)| {
                crate::protocol::bus::cluster::Completion::decode(subject, payload)
            })
            .collect();
        let fresh = offset_ms >= 0;
        observed.push((
            offset_ms,
            removals.len(),
            completions[0].completion.revoke_before > original,
        ));
        if removals.len() > 1 {
            assert!(removals[1].revoke_before.unwrap() as u64 > original);
        }
        assert_eq!(h.delivered().len(), 1);
        if fresh && removals.len() > 1 {
            assert!(
                completions
                    .iter()
                    .all(|c| c.completion.revoke_before > original)
            );
            assert!(
                h.gateway
                    .minted()
                    .iter()
                    .all(|m| m.not_before_unix.unwrap() > original)
            );
        }
    }
    assert_eq!(
        observed,
        vec![(-1, 1, false), (0, 2, true), (1, 2, true), (1500, 2, true)]
    );
}

#[tokio::test(start_paused = true)]
async fn stamp_local_readiness_closes_clock_and_receipt_changes_after_authority_wait() {
    for unknown in [false, true] {
        let h = harness();
        h.snapshot(WALLET, "current", SESSION_A);
        let expected = h.source.lock().unwrap().get(WALLET).unwrap().clone();
        let nbf = island_token_not_before(0, h.subscriber.admission_now_ms());
        let stamp = PulseAdmissionStamp::from_snapshot(&expected, nbf).unwrap();
        assert!(h.subscriber.verify_stamp(&stamp).await);
        if unknown {
            h.subscriber
                .receipts
                .reserve(
                    "older-source",
                    WALLET,
                    &retained_operation(),
                    0,
                    h.subscriber.admission_now_ms() / 1000,
                    h.subscriber.config.island_token_ttl_seconds,
                )
                .unwrap();
        } else {
            h.subscriber.test_clock.lock().unwrap().as_mut().unwrap().0 -= 5000;
        }
        assert!(!h.subscriber.stamp_ready(&stamp).await);
        assert!(h.gateway.minted().is_empty());
    }
}

async fn wait_durable_sql_barrier(pool: &sqlx::PgPool, fragment: &str) {
    tokio::time::timeout(Duration::from_secs(1),async{
        loop {
            let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND query LIKE $1)").bind(format!("%{fragment}%")).fetch_one(pool).await.unwrap();
            if waiting {break;}tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await.expect("actual dispatch/settlement SQL lock wait");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn durable_soft_invalidation_preserves_exact_commit_rpc_and_positive_settlement() {
    assert!(
        std::env::var("CATALYST_TEST_DATABASE_URL").is_ok(),
        "real PostgreSQL required"
    );
    for phase in ["commit", "rpc", "settle"] {
        let (pool, admin, db_name) = liveness_isolated_pool().await;
        let wallet = format!("0x00000000{}", uuid::Uuid::new_v4().simple());
        let store = super::super::safety::SafetyStore::new(pool.clone());
        let (_broker, client, subscriber, gateway, source, sockets, responder, completions) =
            durable_nats_subscriber(store.clone(), &wallet).await;
        let release = Arc::new(Notify::new());
        *gateway.remove_pause.lock().unwrap() = Some(release.clone());
        let mut lock = None;
        if phase == "commit" {
            let mut tx = pool.begin().await.unwrap();
            sqlx::query("LOCK TABLE social.island_cleanup_dispatches IN SHARE MODE")
                .execute(&mut *tx)
                .await
                .unwrap();
            lock = Some(tx);
        }
        subscriber.announce_connect(&wallet, SESSION_B);
        if phase == "commit" {
            wait_durable_sql_barrier(&pool, "INSERT INTO social.island_cleanup_dispatches").await;
        } else {
            tokio::time::timeout(Duration::from_secs(2), gateway.remove_entered.notified())
                .await
                .unwrap();
            if phase == "settle" {
                let mut tx = pool.begin().await.unwrap();
                sqlx::query(
                    "SELECT wallet FROM social.island_admission_fences WHERE wallet=$1 FOR UPDATE",
                )
                .bind(&wallet)
                .execute(&mut *tx)
                .await
                .unwrap();
                lock = Some(tx);
                release.notify_one();
                wait_durable_sql_barrier(&pool, "SELECT generation,token_floor").await;
            }
        }
        // Keep the authoritative old operation valid: only the actual NATS
        // ticket invalidation may suppress stale completion/retry here.
        assert!(source.lock().unwrap().is_some());
        client
            .publish(
                format!("peer.{wallet}.cluster_change"),
                change("next", SESSION_C).encode_to_vec().into(),
            )
            .await
            .unwrap();
        client.flush().await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if subscriber.queue.outstanding_work() >= 2 {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("real NATS event invalidated ticket and queued same-wallet work");
        assert!(
            subscriber.queue.running_work() > 0,
            "critical work remains owned/accounted"
        );
        if let Some(tx) = lock {
            tx.rollback().await.unwrap();
        }
        if phase != "settle" {
            release.notify_one();
        }
        tokio::time::timeout(Duration::from_secs(3), subscriber.settle())
            .await
            .unwrap();
        let rows: Vec<(String, Option<chrono::NaiveDateTime>)> = sqlx::query_as(
            "SELECT state,settled_at FROM social.island_cleanup_dispatches WHERE wallet=$1",
        )
        .bind(&wallet)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(rows.len(), 1, "{phase}: one immutable attempt only");
        assert_eq!(
            rows[0].0, "confirmed",
            "{phase}: soft cancellation must preserve positive result"
        );
        assert!(rows[0].1.is_some());
        assert_eq!(gateway.removals().len(), 1);
        assert!(gateway.minted().is_empty());
        assert!(sockets.0.lock().unwrap().is_empty());
        assert_eq!(
            completions.load(Ordering::SeqCst),
            0,
            "stale work cannot acknowledge Pulse"
        );
        assert_eq!(subscriber.queue.outstanding_work(), 0);
        // Only after exact settlement may a fresh recovery complete the same
        // retained operation, without issuing another destructive RPC.
        subscriber.announce_connect(&wallet, SESSION_B);
        tokio::time::timeout(Duration::from_secs(12), subscriber.settle())
            .await
            .unwrap();
        if gateway.minted().is_empty() {
            assert_eq!(completions.load(Ordering::SeqCst), 1);
            subscriber.announce_connect(&wallet, SESSION_B);
            tokio::time::timeout(Duration::from_secs(12), subscriber.settle())
                .await
                .unwrap();
        }
        assert_eq!(gateway.removals().len(), 1);
        assert_eq!(gateway.minted().len(), 1);
        assert_eq!(sockets.0.lock().unwrap().len(), 1);
        assert!(
            source
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .pending_takeovers
                .is_empty()
        );
        assert!(
            sqlx::query_scalar::<_, bool>(
                "SELECT acknowledged FROM social.island_takeover_receipts WHERE wallet=$1"
            )
            .bind(&wallet)
            .fetch_one(&pool)
            .await
            .unwrap()
        );
        subscriber.stop().await;
        responder.abort();
        liveness_drop_pool(&pool, &admin, &db_name).await;
    }
}

async fn liveness_isolated_pool() -> (sqlx::PgPool, sqlx::PgPool, String) {
    let base = crate::db::scratch::pool(2).await.expect("real PostgreSQL");
    let admin = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect_with(base.connect_options().as_ref().clone().database("postgres"))
        .await
        .unwrap();
    let name = format!("dcl_liveness_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
        .execute(&admin)
        .await
        .unwrap();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect_with(base.connect_options().as_ref().clone().database(&name))
        .await
        .unwrap();
    crate::db::apply_schema(&pool).await.unwrap();
    (pool, admin, name)
}
async fn liveness_drop_pool(pool: &sqlx::PgPool, admin: &sqlx::PgPool, name: &str) {
    assert!(
        name.starts_with("dcl_liveness_")
            && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    );
    pool.close().await;
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE \"{name}\" WITH (FORCE)"
    )))
    .execute(admin)
    .await
    .unwrap();
    admin.close().await;
}

struct LivenessBroker {
    child: std::process::Child,
    reader: Option<std::thread::JoinHandle<()>>,
    url: String,
}
impl Drop for LivenessBroker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            reader.join().unwrap();
        }
    }
}
fn liveness_broker() -> LivenessBroker {
    use std::io::BufRead;
    let binary = std::env::var("PULSE_NATS_SERVER_BIN").expect("real pinned NATS binary required");
    let mut child = std::process::Command::new(binary)
        .args(["--addr", "127.0.0.1", "--port", "-1"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let stderr = child.stderr.take().unwrap();
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    let reader = std::thread::spawn(move || {
        for line in std::io::BufReader::new(stderr).lines() {
            let Ok(line) = line else {
                break;
            };
            if let Some((_, address)) = line.split_once("Listening for client connections on ") {
                if let Ok(address) = address.trim().parse::<std::net::SocketAddr>() {
                    let _ = send.try_send(address.port());
                }
            }
        }
    });
    let mut broker = LivenessBroker {
        child,
        reader: Some(reader),
        url: String::new(),
    };
    broker.url = format!(
        "nats://127.0.0.1:{}",
        receive
            .recv_timeout(Duration::from_secs(5))
            .expect("test NATS port")
    );
    broker
}

type DurableNatsSource = Arc<Mutex<Option<PeerClusterSnapshot>>>;
async fn durable_nats_subscriber(
    store: super::super::safety::SafetyStore,
    wallet: &str,
) -> (
    LivenessBroker,
    async_nats::Client,
    Arc<ClusterSubscriber>,
    Arc<FakeGateway>,
    DurableNatsSource,
    Arc<RecordingSockets>,
    tokio::task::JoinHandle<()>,
    Arc<AtomicUsize>,
) {
    use futures::StreamExt;
    let broker = liveness_broker();
    let client = async_nats::connect(&broker.url).await.unwrap();
    let source = Arc::new(Mutex::new(Some(PeerClusterSnapshot {
        wallet: wallet.into(),
        session: SESSION_B.into(),
        cluster_id: "current".into(),
        realm: "realm".into(),
        source_incarnation: "durable-nats-source".into(),
        assignment_version: 1,
        pass: 1,
        token_not_before: Some(0),
        pending_takeovers: vec![retained_operation()],
    })));
    let mut requests = client
        .subscribe(format!("peer.{wallet}.cluster_lookup"))
        .await
        .unwrap();
    let mut completed = client
        .subscribe(format!("peer.{wallet}.takeover_completed"))
        .await
        .unwrap();
    client.flush().await.unwrap();
    let responses = source.clone();
    let sender = client.clone();
    let completions = Arc::new(AtomicUsize::new(0));
    let observed = completions.clone();
    let responder = tokio::spawn(async move {
        loop {
            tokio::select! {
                    Some(request)=requests.next()=>{let snapshot=responses.lock().unwrap().clone();if let (Some(reply),Some(snapshot))=(request.reply,snapshot){sender.publish(reply,snapshot.encode_to_vec().into()).await.unwrap();}},
                    Some(message)=completed.next()=>{
                if let Ok(completion)=crate::protocol::pulse::decentraland::pulse::TakeoverCompleted::decode(message.payload.as_ref()) {
                    if let Some(snapshot)=responses.lock().unwrap().as_mut() {
                        snapshot.pending_takeovers.retain(|op|op.takeover_id!=completion.takeover_id);
                        snapshot.token_not_before=Some(snapshot.token_not_before.unwrap_or(0).max(completion.revoke_before));
                    }
                }
                observed.fetch_add(1,Ordering::SeqCst);
            },
                    else=>return,
                }
        }
    });
    let bus = Arc::new(crate::gatekeeper::bus::BrokerBus::new(
        Some(broker.url.clone()),
        "durable-liveness-test",
    ));
    let gateway = Arc::new(FakeGateway {
        safety: Some(store),
        ..Default::default()
    });
    let mut cfg = config();
    cfg.drain_timeout_ms = 50;
    let subscriber = ClusterSubscriber::new(
        bus.clone(),
        gateway.clone(),
        Arc::new(ClusterPeerState::default()),
        cfg,
    );
    let sockets = Arc::new(RecordingSockets::default());
    subscriber.attach_sockets(sockets.clone());
    subscriber.start();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if bus.is_ready() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    (
        broker,
        client,
        subscriber,
        gateway,
        source,
        sockets,
        responder,
        completions,
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn durable_soft_deadline_records_positive_result_but_hard_stop_retains_unknown() {
    assert!(
        std::env::var("CATALYST_TEST_DATABASE_URL").is_ok(),
        "real PostgreSQL required"
    );
    for hard in [false, true] {
        let (pool, admin, name) = liveness_isolated_pool().await;
        let store = super::super::safety::SafetyStore::new(pool.clone());
        let wallet = format!("0x00000000{}", uuid::Uuid::new_v4().simple());
        let (_broker, _client, subscriber, gateway, source, sockets, responder, completions) =
            durable_nats_subscriber(store.clone(), &wallet).await;
        let release = Arc::new(Notify::new());
        *gateway.remove_pause.lock().unwrap() = Some(release.clone());
        subscriber.announce_connect(&wallet, SESSION_B);
        tokio::time::timeout(Duration::from_secs(2), gateway.remove_entered.notified())
            .await
            .unwrap();
        let id:uuid::Uuid=sqlx::query_scalar("SELECT dispatch_id FROM social.island_cleanup_dispatches WHERE wallet=$1 AND state='dispatched'").bind(&wallet).fetch_one(&pool).await.unwrap();
        assert!(source.lock().unwrap().is_some());
        if hard {
            // Existing drain grace then owner cancellation; no extra lifetime.
            tokio::time::timeout(Duration::from_secs(3), subscriber.stop())
                .await
                .expect("bounded hard stop");
            assert_eq!(
                sqlx::query_scalar::<_, String>(
                    "SELECT state FROM social.island_cleanup_dispatches WHERE dispatch_id=$1"
                )
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap(),
                "dispatched"
            );
        } else {
            tokio::time::sleep(RECOVERY_DEADLINE + Duration::from_millis(100)).await;
            assert_eq!(
                subscriber.queue.running_work(),
                1,
                "soft deadline cannot release committed cleanup lane"
            );
            release.notify_one();
            tokio::time::timeout(Duration::from_secs(3), subscriber.settle())
                .await
                .unwrap();
            assert_eq!(sqlx::query_as::<_,(String,bool)>("SELECT state,settled_at IS NOT NULL FROM social.island_cleanup_dispatches WHERE dispatch_id=$1").bind(id).fetch_one(&pool).await.unwrap(),("settled_late".into(),true));
            subscriber.stop().await;
        }
        assert_eq!(gateway.removals().len(), 1);
        assert!(gateway.minted().is_empty());
        assert!(sockets.0.lock().unwrap().is_empty());
        assert_eq!(completions.load(Ordering::SeqCst), 0);
        assert_eq!(subscriber.queue.outstanding_work(), 0);
        assert!(store.snapshot(&wallet).await.is_err());
        responder.abort();
        liveness_drop_pool(&pool, &admin, &name).await;
    }
}

#[tokio::test]
async fn durable_hard_queue_expiry_keeps_unknown_and_releases_original_owned_lane() {
    let (pool, admin, name) = liveness_isolated_pool().await;
    let store = super::super::safety::SafetyStore::new(pool.clone());
    let wallet = format!("0x00000000{}", uuid::Uuid::new_v4().simple());
    let (_broker, _client, subscriber, gateway, _source, sockets, responder, completions) =
        durable_nats_subscriber(store.clone(), &wallet).await;
    let release = Arc::new(Notify::new());
    *gateway.remove_pause.lock().unwrap() = Some(release);
    subscriber.announce_connect(&wallet, SESSION_B);
    tokio::time::timeout(Duration::from_secs(2), gateway.remove_entered.notified())
        .await
        .unwrap();
    let id:uuid::Uuid=sqlx::query_scalar("SELECT dispatch_id FROM social.island_cleanup_dispatches WHERE wallet=$1 AND state='dispatched'").bind(&wallet).fetch_one(&pool).await.unwrap();
    // Virtualize ONLY Tokio's existing hard queue deadline after real PG/NATS
    // dispatch. PostgreSQL timestamps and the issuer lifetime are not changed.
    tokio::time::pause();
    tokio::time::advance(wallet_queue::WORK_DEADLINE + Duration::from_secs(1)).await;
    for _ in 0..8 {
        tokio::task::yield_now().await;
    }
    tokio::time::resume();
    tokio::time::timeout(Duration::from_secs(1), subscriber.settle())
        .await
        .unwrap();
    assert_eq!(subscriber.queue.outstanding_work(), 0);
    assert_eq!(subscriber.queue.tracked_wallets(), 0);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM social.island_cleanup_dispatches WHERE dispatch_id=$1"
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "dispatched"
    );
    assert_eq!(gateway.removals().len(), 1);
    assert!(gateway.minted().is_empty());
    assert!(sockets.0.lock().unwrap().is_empty());
    assert_eq!(completions.load(Ordering::SeqCst), 0);
    assert!(store.snapshot(&wallet).await.is_err());
    subscriber.stop().await;
    responder.abort();
    liveness_drop_pool(&pool, &admin, &name).await;
}
