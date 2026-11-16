// from: catalyrst/crates/catalyrst-comms/tests/v4_assignment_fence/recovery.rs
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use catalyst::comms::gatekeeper::admission::cluster::ClusterConfig;
use catalyst::comms::gatekeeper::admission::cluster::fence::TokenFence;
use catalyst::comms::gatekeeper::admission::cluster::peer_state::ClusterPeerState;
use catalyst::comms::gatekeeper::admission::cluster::sockets::{IslandDelivery, IslandSockets};
use catalyst::comms::gatekeeper::admission::cluster::subscriber::{
    ClusterGateway, ClusterSubscriber, GatewayError, IslandOccupant,
};
use catalyst::comms::gatekeeper::bus::{InProcessBus, NatsBus, RequestError};
use catalyst::comms::gatekeeper::livekit::Removal;
use catalyst::comms::protocol::archipelago::archipelago::IslandChangedMessage;
use catalyst::comms::protocol::pulse::decentraland::pulse::PeerClusterSnapshot;
use prost::Message;
use tokio::sync::Notify;

use super::{AUDIENCE, Fixture, WALLET, row_state, seed_realm};

const SESSION: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const STALE_SESSION: &str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[derive(Default)]
struct RecordingGateway {
    pool: Option<sqlx::PgPool>,
    fences: Mutex<Vec<TokenFence>>,
    occupant: Mutex<Option<IslandOccupant>>,
    mint_pause: Mutex<Option<Arc<Notify>>>,
    mint_entered: Notify,
    multiple_wallets: bool,
}

#[async_trait]
impl ClusterGateway for RecordingGateway {
    fn safety_store(
        &self,
    ) -> Option<catalyst::comms::gatekeeper::admission::cluster::safety::SafetyStore> {
        self.pool.clone().map(
            catalyst::comms::gatekeeper::admission::cluster::safety::SafetyStore::unfenced_test,
        )
    }
    async fn is_denied(&self, _: &str) -> Result<bool, GatewayError> {
        Ok(false)
    }

    async fn mint_connection_string(
        &self,
        _admission: &catalyst::comms::gatekeeper::admission::cluster::fence::PulseAdmissionStamp,
        _: &str,
        _: Option<&str>,
        _: &str,
        _: u64,
        _: Option<u64>,
    ) -> Result<String, GatewayError> {
        panic!("an owned realm must never mint an unfenced token")
    }

    async fn mint_fenced_connection_string(
        &self,
        _admission: &catalyst::comms::gatekeeper::admission::cluster::fence::PulseAdmissionStamp,
        wallet: &str,
        room: &str,
        _: u64,
        fence: &TokenFence,
        _: Option<u64>,
    ) -> Result<String, GatewayError> {
        if !self.multiple_wallets {
            assert_eq!(wallet, WALLET);
        }
        self.fences.lock().unwrap().push(fence.clone());
        self.mint_entered.notify_one();
        let pause = self.mint_pause.lock().unwrap().clone();
        if let Some(pause) = pause {
            pause.notified().await;
        }
        Ok(format!("livekit:wss://example.invalid/{room}"))
    }

    async fn remove_participant(
        &self,
        _: &str,
        _: &str,
        _: Option<i64>,
    ) -> Result<Removal, GatewayError> {
        panic!("recovery must not blindly remove an existing participant")
    }

    async fn holds_participant(&self, _: &str, _: &str) -> Result<bool, GatewayError> {
        Ok(true)
    }

    async fn island_occupant(&self, _: &str, _: &str) -> Result<IslandOccupant, GatewayError> {
        Ok(self
            .occupant
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| IslandOccupant::Session(SESSION.into())))
    }
}

/// The connector's sockets: a fenced recovery commits to the authority row and must hand them
/// nothing.
#[derive(Default)]
struct RecordingSockets(Mutex<Vec<IslandChangedMessage>>);

impl IslandSockets for RecordingSockets {
    fn deliver_island(
        &self,
        _: &str,
        _: &str,
        assignment: &IslandChangedMessage,

        _pulse_admission: Option<
            &catalyst::comms::gatekeeper::admission::cluster::fence::PulseAdmissionStamp,
        >,
    ) -> IslandDelivery {
        self.0.lock().unwrap().push(assignment.clone());
        IslandDelivery::Delivered
    }
}

struct RecoveryHarness {
    subscriber: Arc<ClusterSubscriber>,
    bus: Arc<InProcessBus>,
    gateway: Arc<RecordingGateway>,
    source: Arc<Mutex<PeerClusterSnapshot>>,
    sockets: Arc<RecordingSockets>,
    fence: Arc<super::PgAssignmentFence>,
}

impl RecoveryHarness {
    async fn new(fixture: &Fixture, session: &str) -> Self {
        let source = Arc::new(Mutex::new(PeerClusterSnapshot {
            selected_origin: 1,
            selected_identity: Some(1),
            assignment_state: 1,
            wallet: WALLET.into(),
            session: session.into(),
            cluster_id: "current".into(),
            realm: "realm-current".into(),
            source_incarnation: "pulse-current".into(),
            pass: 4,
            pending_takeovers: Vec::new(),
            token_not_before: Some(0),
            assignment_version: 1,
        }));
        let replies = source.clone();
        let bus = Arc::new(InProcessBus::new());
        bus.respond_to_requests(Arc::new(move |subject, payload| {
            if subject != format!("peer.{WALLET}.cluster_lookup") {
                return Err(RequestError::NoResponders);
            }
            let snapshot = replies.lock().unwrap();
            if snapshot.session.as_bytes() != payload {
                return Err(RequestError::NoResponders);
            }
            Ok(snapshot.encode_to_vec())
        }));
        let gateway = Arc::new(RecordingGateway {
            pool: Some(fixture.pool.clone()),
            ..Default::default()
        });
        let fence = Arc::new(fixture.fence().await);
        let subscriber = ClusterSubscriber::new_with_assignment_fence(
            bus.clone(),
            gateway.clone(),
            Arc::new(ClusterPeerState::default()),
            ClusterConfig {
                nats_url: Some("nats://127.0.0.1:4222".into()),
                drain_timeout_ms: 0,
                ..ClusterConfig::default()
            },
            Some(fence.clone()),
        );
        let sockets = Arc::new(RecordingSockets::default());
        subscriber.attach_sockets(sockets.clone());
        subscriber.start();
        Self {
            subscriber,
            bus,
            gateway,
            source,
            sockets,
            fence,
        }
    }

    async fn claim(
        &self,
    ) -> Box<dyn catalyst::comms::gatekeeper::admission::cluster::fence::RealmAssignmentLease> {
        use catalyst::comms::gatekeeper::admission::cluster::fence::AssignmentFence;
        let selected = self.source.lock().unwrap().clone();
        self.fence
            .lock_selected(&selected)
            .await
            .expect("real selected coordinator writer")
    }

    fn connect(&self, session: &str) {
        self.subscriber.announce_connect(WALLET, session);
    }

    async fn settle(&self) {
        tokio::time::timeout(std::time::Duration::from_secs(5), self.subscriber.settle())
            .await
            .expect("recovery finishes within its test deadline");
        assert!(
            self.sockets.0.lock().unwrap().is_empty(),
            "no legacy assignment reaches a socket"
        );
        assert!(
            self.bus.published().is_empty(),
            "nothing crosses the broker"
        );
    }
}

#[tokio::test]
async fn a_stationary_reconnect_refills_the_claim_once_despite_an_old_participant() {
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    let harness = RecoveryHarness::new(&fixture, SESSION).await;
    let claimed = harness.claim().await.token_fence();

    harness.connect(SESSION);
    harness.settle().await;

    let (revision, fence, document) = row_state(&fixture.pool).await;
    assert_eq!(
        (revision, fence),
        (
            claimed.assignment_revision as i64,
            claimed.fencing_token as i64
        )
    );
    let document = document.expect("reconnect fills the claimed authority row");
    assert_eq!(document["islandId"], "island-current");
    assert_eq!(
        document["connectionString"],
        "livekit:wss://example.invalid/island-current"
    );
    assert!(document["fromIslandId"].is_null());
    let token = harness.gateway.fences.lock().unwrap()[0].clone();
    assert_eq!(token.audience, AUDIENCE);
    assert_eq!(token.lane, "realm");
    assert_eq!(token.owner_session, SESSION);
    assert_eq!(
        (
            token.owner_epoch,
            token.assignment_revision,
            token.fencing_token
        ),
        (
            claimed.owner_epoch,
            claimed.assignment_revision,
            claimed.fencing_token
        )
    );

    harness.connect(SESSION);
    harness.settle().await;
    assert_eq!(
        row_state(&fixture.pool).await,
        (revision, fence, Some(document))
    );
    assert_eq!(harness.gateway.fences.lock().unwrap().len(), 1);
    harness.subscriber.stop().await;
    drop(harness);
    fixture.finish().await;
}

#[tokio::test]
async fn a_stale_connect_cannot_refill_a_newer_owners_claim() {
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    seed_realm(&fixture.pool, WALLET, SESSION, 7, 3, 9, None).await;
    let harness = RecoveryHarness::new(&fixture, STALE_SESSION).await;

    harness.connect(STALE_SESSION);
    harness.settle().await;

    assert_eq!(row_state(&fixture.pool).await, (3, 9, None));
    assert!(harness.gateway.fences.lock().unwrap().is_empty());
    harness.subscriber.stop().await;
    drop(harness);
    fixture.finish().await;
}

#[tokio::test]
async fn same_room_control_handoffs_refill_same_and_different_session_claims() {
    let fixture = Fixture::new()
        .await
        .expect("required disposable PostgreSQL");
    let harness = RecoveryHarness::new(&fixture, SESSION).await;
    let mut previous = harness.claim().await.token_fence();
    for (identity, session) in [(2, SESSION), (3, STALE_SESSION)] {
        {
            let mut source = harness.source.lock().unwrap();
            source.session = session.into();
            source.selected_identity = Some(identity);
            source.assignment_version += 1;
        }
        let claimed = harness.claim().await.token_fence();
        assert!(claimed.owner_epoch > previous.owner_epoch);
        assert!(claimed.fencing_token > previous.fencing_token);
        harness.connect(session);
        harness.settle().await;
        let (revision, fence, document) = row_state(&fixture.pool).await;
        assert_eq!(
            (revision, fence),
            (
                claimed.assignment_revision as i64,
                claimed.fencing_token as i64
            )
        );
        assert_eq!(document.as_ref().unwrap()["islandId"], "island-current");
        let minted = harness
            .gateway
            .fences
            .lock()
            .unwrap()
            .last()
            .unwrap()
            .clone();
        assert_eq!(minted, claimed);
        assert_eq!(minted.owner_session, session);
        harness.connect(SESSION);
        harness.settle().await;
        assert_eq!(row_state(&fixture.pool).await, (revision, fence, document));
        previous = claimed;
    }
    assert_eq!(harness.gateway.fences.lock().unwrap().len(), 2);
    harness.subscriber.stop().await;
    drop(harness);
    fixture.finish().await;
}

#[tokio::test]
async fn a_move_during_token_mint_rolls_back_then_recovers_the_current_room() {
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    let harness = RecoveryHarness::new(&fixture, SESSION).await;
    let claimed = harness.claim().await.token_fence();
    let release = Arc::new(Notify::new());
    *harness.gateway.mint_pause.lock().unwrap() = Some(release.clone());

    harness.connect(SESSION);
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        harness.gateway.mint_entered.notified(),
    )
    .await
    .expect("mint reached after the optimistic authority read released SQL locks");
    let authority =
        catalyst::comms::archipelago::control_v4::AssignmentAuthority::pg_with_namespace(
            fixture.pool.clone(),
            AUDIENCE,
        )
        .await
        .unwrap();
    let newer_owner = catalyst::comms::archipelago::control_v4::V4Owner {
        address: WALLET.into(),
        session: SESSION.into(),
        epoch: authority.next_connection_epoch().await.unwrap(),
    };
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        authority.claim_lanes(
            &newer_owner,
            &[catalyst::comms::archipelago::control_v4::LaneKey::parse("realm").unwrap()],
        ),
    )
    .await
    .expect("a competing owner change must commit while the external mint is still paused")
    .unwrap();
    let displaced = row_state(&fixture.pool).await;
    assert_eq!(displaced.0, claimed.assignment_revision as i64);
    assert!(displaced.1 > claimed.fencing_token as i64);
    assert!(displaced.2.is_none());
    assert!(harness.sockets.0.lock().unwrap().is_empty());
    {
        let mut source = harness.source.lock().unwrap();
        source.cluster_id = "moved".into();
        source.pass += 1;
        source.assignment_version += 1;
    }
    release.notify_one();
    harness.settle().await;
    assert_eq!(
        row_state(&fixture.pool).await,
        displaced,
        "the stale private credential must not publish after the newer owner commits"
    );
    assert!(
        harness.sockets.0.lock().unwrap().is_empty(),
        "no legacy fallback may deliver the stale token"
    );

    *harness.gateway.mint_pause.lock().unwrap() = None;
    // The unrelated API writer is not retroactively owned. Its finite lease must lapse
    // before the selected coordinator can acquire the row again.
    sqlx::query("UPDATE social.archipelago_v4_assignments SET updated_at=clock_timestamp()-interval '91 seconds' WHERE deployment_audience=$1 AND owner_address=$2 AND lane_key='realm'")
        .bind(AUDIENCE).bind(WALLET).execute(&fixture.pool).await.unwrap();
    let recovered = harness.claim().await.token_fence();
    harness.connect(SESSION);
    harness.settle().await;
    let (revision, fence, document) = row_state(&fixture.pool).await;
    assert_eq!(
        (revision, fence),
        (
            recovered.assignment_revision as i64,
            recovered.fencing_token as i64
        )
    );
    assert_eq!(document.unwrap()["islandId"], "island-moved");
    assert_eq!(harness.gateway.fences.lock().unwrap().len(), 2);
    harness.subscriber.stop().await;
    drop(harness);
    fixture.finish().await;
}

#[tokio::test]
async fn same_room_recovery_renews_expired_credentials_without_periodic_churn() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    for (expiry, occupant, should_renew) in [
        (Some(now + 3600), IslandOccupant::Absent, false),
        (Some(1), IslandOccupant::Absent, true),
        (None, IslandOccupant::Absent, true),
        (Some(1), IslandOccupant::Session(SESSION.into()), false),
        (Some(1), IslandOccupant::Unattributed, false),
        (Some(1), IslandOccupant::Session(STALE_SESSION.into()), true),
    ] {
        let Some(fixture) = Fixture::new().await else {
            return;
        };
        let safety =
            catalyst::comms::gatekeeper::admission::cluster::safety::SafetyStore::unfenced_test(
                fixture.pool.clone(),
            )
            .snapshot(WALLET)
            .await
            .unwrap();
        assert!(safety.generation > 0);
        let mut document = serde_json::json!({
            "islandId": "island-current",
            "connectionString": "livekit:wss://example.invalid/previous",
            "pulseAdmission": { "selectedOrigin": "native", "selectedIdentity": 1, "wallet": WALLET, "session": SESSION, "clusterId": "current", "realm": "realm-current", "sourceIncarnation": "pulse-current", "assignmentVersion": 1, "tokenNotBefore": 0, "credentialNotBefore": 0, "safetyGeneration":safety.generation,"safetyTokenFloor":safety.token_floor }
        });
        if let Some(expiry) = expiry {
            document["tokenExpiresAtUnix"] = expiry.into();
        }
        let harness = RecoveryHarness::new(&fixture, SESSION).await;
        let lease = harness.claim().await;
        let initial = lease.token_fence();
        lease
            .commit(serde_json::from_value(document.clone()).unwrap())
            .await
            .unwrap();
        *harness.gateway.occupant.lock().unwrap() = Some(occupant.clone());

        harness.connect(SESSION);
        harness.settle().await;
        let (revision, fence, stored) = row_state(&fixture.pool).await;
        assert_eq!(fence, initial.fencing_token as i64);
        assert_eq!(
            revision,
            initial.assignment_revision as i64 + i64::from(should_renew),
            "{expiry:?} {occupant:?}"
        );
        assert_eq!(
            harness.gateway.fences.lock().unwrap().len(),
            usize::from(should_renew)
        );
        if should_renew {
            let stored = stored.expect("renewed assignment");
            assert!(stored["tokenExpiresAtUnix"].as_u64().unwrap() > now);
            assert_eq!(stored["fromIslandId"], "island-current");
            harness.connect(SESSION);
            harness.settle().await;
            assert_eq!(
                row_state(&fixture.pool).await,
                (revision, fence, Some(stored))
            );
            assert_eq!(
                harness.gateway.fences.lock().unwrap().len(),
                1,
                "fresh renewal does not churn at the next announcement"
            );
        } else {
            assert_eq!(stored, Some(document));
        }
        harness.subscriber.stop().await;
        drop(harness);
        fixture.finish().await;
    }
}

#[tokio::test]
async fn started_native_periodic_replay_retries_borrowed_and_foreign_rows_after_lapse() {
    for row in [
        "same-session-null",
        "same-session-assignment",
        "foreign-session",
    ] {
        periodic_native_retry_case(row).await;
    }
}

#[tokio::test]
async fn started_native_periodic_replay_survives_one_lookup_failure_without_another_hint() {
    periodic_native_retry_case("lookup-failure").await;
}

async fn periodic_native_retry_case(case: &str) {
    use catalyst::comms::gatekeeper::admission::cluster::{
        ClusterRuntime,
        fence::{AssignmentFence, PulseAdmissionStamp},
    };
    use catalyst::comms::gatekeeper::bus::{NatsBus, PublishOutcome};
    use catalyst::comms::protocol::bus::cluster::ClusterChange;
    use catalyst::comms::protocol::pulse::decentraland::pulse::PeerClusterChange;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    let fixture = Fixture::new()
        .await
        .expect("required disposable PostgreSQL");
    let harness = RecoveryHarness::new(&fixture, SESSION).await;
    let selected = harness.source.lock().unwrap().clone();
    let authority_available = Arc::new(AtomicBool::new(true));
    let initial_failures = Arc::new(AtomicUsize::new(usize::from(case == "lookup-failure")));
    let available = authority_available.clone();
    let failures = initial_failures.clone();
    let replies = harness.source.clone();
    harness
        .bus
        .respond_to_requests(Arc::new(move |subject, payload| {
            if subject != format!("peer.{WALLET}.cluster_lookup") {
                return Err(RequestError::NoResponders);
            }
            if !available.load(Ordering::SeqCst)
                || failures
                    .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                    .is_ok()
            {
                return Err(RequestError::TimedOut);
            }
            let current = replies.lock().unwrap();
            if current.session.as_bytes() != payload {
                return Err(RequestError::NoResponders);
            }
            Ok(current.encode_to_vec())
        }));
    if case != "lookup-failure" {
        // The external owner shares the real allocator domain; lapse must not need
        // artificial sequence catch-up or an ownership flag in the coordinator.
        let epoch: i64 =
            sqlx::query_scalar("SELECT nextval('social.archipelago_v4_connection_epoch')")
                .fetch_one(&fixture.pool)
                .await
                .unwrap();
        let previous = if case == "same-session-assignment" {
            let mut document = super::assignment(&fixture.pool, "island-current", None).await;
            let safety = document.pulse_admission.as_ref().unwrap().clone();
            let mut stamp = PulseAdmissionStamp::from_snapshot(&selected, 0).unwrap();
            stamp.safety_generation = safety.safety_generation;
            stamp.safety_token_floor = safety.safety_token_floor;
            document.pulse_admission = Some(stamp);
            Some(serde_json::to_value(document).unwrap())
        } else {
            None
        };
        seed_realm(
            &fixture.pool,
            WALLET,
            if case == "foreign-session" {
                STALE_SESSION
            } else {
                SESSION
            },
            epoch,
            1,
            1,
            previous,
        )
        .await;
    }
    let before = periodic_native_owner_row(&fixture).await;
    let change = ClusterChange {
        wallet: WALLET.into(),
        change: PeerClusterChange {
            session: selected.session.clone(),
            realm: selected.realm.clone(),
            cluster_id: selected.cluster_id.clone(),
            source_incarnation: selected.source_incarnation.clone(),
            assignment_version: selected.assignment_version,
            token_not_before: selected.token_not_before,
            ..Default::default()
        },
    };
    assert_eq!(
        harness
            .bus
            .publish(&change.subject(), change.encode())
            .await,
        PublishOutcome::Submitted
    );
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        harness.subscriber.settle(),
    )
    .await
    .unwrap();
    assert_eq!(initial_failures.load(Ordering::SeqCst), 0);
    assert!(
        harness.gateway.fences.lock().unwrap().is_empty(),
        "{case}: no token before selected acquisition"
    );
    assert_eq!(
        periodic_native_owner_row(&fixture).await,
        before,
        "{case}: foreign or borrowed authority remains untouched"
    );
    assert_eq!(
        harness.fence.selected_reconciliation_targets(),
        vec![(WALLET.into(), SESSION.into())],
        "{case}: retry survives the first failed/readonly attempt"
    );
    let jobs = catalyst::jobs::Jobs::new();
    let runtime = ClusterRuntime {
        subscriber: harness.subscriber.clone(),
        fence: Some(harness.fence.clone()),
    };
    runtime.start(&jobs);
    if case != "lookup-failure" {
        let asked = harness.bus.requested().len();
        tokio::time::timeout(std::time::Duration::from_secs(8),async{
            while harness.bus.requested().len()<=asked {tokio::time::sleep(std::time::Duration::from_millis(10)).await;}
            harness.subscriber.settle().await;
        }).await.expect("the started runtime actually replays its retained native target while the row is still fresh");
        assert_eq!(
            periodic_native_owner_row(&fixture).await,
            before,
            "{case}: periodic replay cannot renew/release a fresh external row"
        );
        assert!(harness.gateway.fences.lock().unwrap().is_empty());
        assert_eq!(
            harness.fence.selected_reconciliation_targets(),
            vec![(WALLET.into(), SESSION.into())]
        );
        sqlx::query("UPDATE social.archipelago_v4_assignments SET updated_at=timezone('UTC',clock_timestamp())-interval '91 seconds' WHERE deployment_audience=$1 AND owner_address=$2 AND lane_key='realm'")
            .bind(AUDIENCE).bind(WALLET).execute(&fixture.pool).await.unwrap();
    }
    tokio::time::timeout(std::time::Duration::from_secs(8),async{
        loop {
            let row=periodic_native_owner_row(&fixture).await;
            if row.as_ref().is_some_and(|row|row.0==SESSION && row.4.as_ref().is_some_and(|doc|doc["pulseAdmission"]["sourceIncarnation"]==selected.source_incarnation)) && !harness.gateway.fences.lock().unwrap().is_empty() {break;}
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        harness.subscriber.settle().await;
    }).await.expect("periodic replay automatically acquires and publishes for unchanged native selection, without /ws or another Pulse hint");
    assert_eq!(harness.gateway.fences.lock().unwrap().len(), 1);
    let acquired = periodic_native_owner_row(&fixture).await.unwrap();
    assert_eq!(acquired.0, SESSION);
    assert!(acquired.4.is_some());
    if let Some(previous) = &before {
        assert!(acquired.1 > previous.1);
        assert!(acquired.3 > previous.3);
    }
    assert!(harness.sockets.0.lock().unwrap().is_empty());
    assert_eq!(
        harness.bus.published(),
        vec![(change.subject(), change.encode())],
        "no new Pulse publication or manual reconnect drives recovery"
    );
    assert_eq!(
        harness.fence.selected_reconciliation_targets(),
        vec![(WALLET.into(), SESSION.into())],
        "one bounded target remains after acquisition"
    );
    if case == "lookup-failure" {
        // Silence is unknown even after the SQL lease lapses. It must neither
        // mint nor erase the only retry hint for this still-authenticated peer.
        authority_available.store(false, Ordering::SeqCst);
        sqlx::query("UPDATE social.archipelago_v4_assignments SET updated_at=timezone('UTC',clock_timestamp())-interval '91 seconds' WHERE deployment_audience=$1 AND owner_address=$2 AND lane_key='realm'")
            .bind(AUDIENCE).bind(WALLET).execute(&fixture.pool).await.unwrap();
        let lapsed = periodic_native_owner_row(&fixture).await;
        let asked = harness.bus.requested().len();
        harness.fence.reconcile_selected_now_for_test(WALLET).await;
        tokio::time::timeout(std::time::Duration::from_secs(8), async {
            while harness.bus.requested().len() <= asked {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            harness.subscriber.settle().await;
        })
        .await
        .expect("periodic lookup also runs while authority is unknown");
        assert_eq!(periodic_native_owner_row(&fixture).await, lapsed);
        assert_eq!(harness.gateway.fences.lock().unwrap().len(), 1);
        assert_eq!(
            harness.fence.selected_reconciliation_targets(),
            vec![(WALLET.into(), SESSION.into())]
        );
    }
    let final_row = periodic_native_owner_row(&fixture).await;
    let ended = Arc::downgrade(&harness.fence);
    jobs.shutdown(std::time::Duration::from_secs(2)).await;
    drop(runtime);
    drop(harness);
    assert!(
        ended.upgrade().is_none(),
        "retained retry state ends with the actual coordinator incarnation, not with a negative lookup"
    );
    assert_eq!(
        periodic_native_owner_row(&fixture).await,
        final_row,
        "lifecycle cleanup does not release a foreign or known SQL row"
    );
    fixture.finish().await;
}

type PeriodicNativeOwnerRow = (
    String,
    i64,
    i64,
    i64,
    Option<serde_json::Value>,
    chrono::NaiveDateTime,
);
async fn periodic_native_owner_row(fixture: &Fixture) -> Option<PeriodicNativeOwnerRow> {
    sqlx::query_as("SELECT owner_session,owner_epoch,assignment_revision,fencing_token,assignment_json,updated_at FROM social.archipelago_v4_assignments WHERE deployment_audience=$1 AND owner_address=$2 AND lane_key='realm'")
        .bind(AUDIENCE).bind(WALLET).fetch_optional(&fixture.pool).await.unwrap()
}

struct RetirementBus {
    inner: Arc<InProcessBus>,
    pause_negative: std::sync::atomic::AtomicBool,
    negative_entered: Notify,
    negative_release: Notify,
}
#[async_trait]
impl catalyst::comms::gatekeeper::bus::NatsBus for RetirementBus {
    fn is_enabled(&self) -> bool {
        self.inner.is_enabled()
    }
    fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }
    fn is_ready(&self) -> bool {
        self.inner.is_ready()
    }
    fn connect(&self) {
        self.inner.connect();
    }
    fn subscribe(
        &self,
        subject: &str,
        queue: Option<&str>,
        handler: catalyst::comms::gatekeeper::bus::NatsHandler,
    ) -> catalyst::comms::gatekeeper::bus::NatsSubscription {
        self.inner.subscribe(subject, queue, handler)
    }
    async fn publish(
        &self,
        subject: &str,
        bytes: Vec<u8>,
    ) -> catalyst::comms::gatekeeper::bus::PublishOutcome {
        self.inner.publish(subject, bytes).await
    }
    async fn request(&self, subject: &str, bytes: Vec<u8>) -> Result<Vec<u8>, RequestError> {
        let reply = self.inner.request(subject, bytes).await?;
        if subject.ends_with(".selection_retirement")
            && self
                .pause_negative
                .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            self.negative_entered.notify_one();
            self.negative_release.notified().await;
        }
        Ok(reply)
    }
}

struct SelectedSourceSockets(Arc<catalyst::comms::pulse::cluster::feed::CurrentAssignments>);
impl IslandSockets for SelectedSourceSockets {
    fn selected_transport_is_current(&self, snapshot: &PeerClusterSnapshot) -> bool {
        self.0.lookup(&snapshot.wallet, &snapshot.session).as_ref() == Some(snapshot)
    }
    fn deliver_island(
        &self,
        _: &str,
        _: &str,
        _: &IslandChangedMessage,
        _: Option<&catalyst::comms::gatekeeper::admission::cluster::fence::PulseAdmissionStamp>,
    ) -> IslandDelivery {
        IslandDelivery::Undelivered
    }
}

/// Real configured coordinator and complete Pulse authority; only NATS/media
/// transport is replaced. Admission positives use the started subscriber; race
/// fixtures invoke the same selected writer to control mutation ordering.
struct RetirementHarness {
    bus: Arc<RetirementBus>,
    source: Arc<catalyst::comms::pulse::cluster::feed::CurrentAssignments>,
    identity: catalyst::comms::pulse::snapshot::IdentityBoard,
    subscriber: Arc<ClusterSubscriber>,
    fence: Arc<super::PgAssignmentFence>,
    gateway: Arc<RecordingGateway>,
    runtime: catalyst::comms::gatekeeper::admission::cluster::ClusterRuntime,
    jobs: catalyst::jobs::Jobs,
    pass: u64,
    lookup_available: Arc<std::sync::atomic::AtomicBool>,
}
impl RetirementHarness {
    fn new(pool: sqlx::PgPool, limit: usize) -> Self {
        let harness = Self::new_unstarted(pool, limit);
        harness.runtime.start(&harness.jobs);
        harness
    }
    fn new_unstarted(pool: sqlx::PgPool, limit: usize) -> Self {
        use catalyst::comms::pulse::cluster::{
            ClusterOptions, ClusterTracker,
            feed::{ClusterFeedPublisher, RecordingClusterFeed},
        };
        let feed = Arc::new(RecordingClusterFeed::default());
        let source = feed.current_assignments().unwrap();
        let tracker = ClusterTracker::new(ClusterOptions::default(), 2, feed);
        let mut identity = catalyst::comms::pulse::snapshot::IdentityBoard::new(2);
        tracker.bind_identity(&mut identity);
        let inner = Arc::new(InProcessBus::new());
        let authority = source.clone();
        let lookup_available = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let available = lookup_available.clone();
        inner.respond_to_requests(Arc::new(move |subject, bytes| {
            use catalyst::comms::protocol::bus::cluster::{
                ClusterLookup, SelectionRetirementLookup,
            };
            if !available.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(RequestError::TimedOut);
            }
            if let Some(lookup) = SelectionRetirementLookup::decode(subject, bytes) {
                return authority
                    .lookup_retirement(&lookup)
                    .and_then(|reply| reply.encode())
                    .ok_or(RequestError::NoResponders);
            }
            let lookup =
                ClusterLookup::decode(subject, bytes).map_err(|_| RequestError::Invalid)?;
            authority
                .lookup(&lookup.wallet, &lookup.session)
                .map(|reply| reply.encode_to_vec())
                .ok_or(RequestError::NoResponders)
        }));
        let bus = Arc::new(RetirementBus {
            inner,
            pause_negative: false.into(),
            negative_entered: Notify::new(),
            negative_release: Notify::new(),
        });
        let config = ClusterConfig {
            nats_url: Some("nats://127.0.0.1:4222".into()),
            control_v4_audience: Some(AUDIENCE.into()),
            peer_state_max: limit,
            drain_timeout_ms: 0,
            ..Default::default()
        };
        let fence =
            catalyst::comms::gatekeeper::admission::cluster::fence_for(pool.clone(), &config)
                .unwrap()
                .unwrap();
        let gateway = Arc::new(RecordingGateway {
            pool: Some(pool),
            multiple_wallets: true,
            ..Default::default()
        });
        let subscriber = ClusterSubscriber::new_with_assignment_fence(
            bus.clone(),
            gateway.clone(),
            Arc::new(ClusterPeerState::new(
                config.peer_state_max,
                config.peer_state_ttl_ms,
            )),
            config,
            Some(fence.clone()),
        );
        subscriber.attach_sockets(Arc::new(SelectedSourceSockets(source.clone())));
        let runtime = catalyst::comms::gatekeeper::admission::cluster::ClusterRuntime {
            subscriber: subscriber.clone(),
            fence: Some(fence.clone()),
        };
        let jobs = catalyst::jobs::Jobs::new();
        Self {
            bus,
            source,
            identity,
            subscriber,
            fence,
            gateway,
            runtime,
            jobs,
            pass: 0,
            lookup_available,
        }
    }
    fn select(&mut self, wallet: &str, session: &str, native: bool) -> PeerClusterSnapshot {
        use catalyst::comms::pulse::cluster::feed::CurrentAssignment;
        self.pass += 1;
        let epoch = if native {
            self.identity
                .set_with_session(0, wallet.into(), session.into());
            self.identity.registration_by_peer(0)
        } else {
            self.pass
        };
        self.source.replace(
            self.pass,
            std::time::Duration::from_secs(120),
            vec![CurrentAssignment {
                wallet: wallet.into(),
                session: session.into(),
                realm: "realm-current".into(),
                cluster_id: format!("current-{}", self.pass),
                owner_epoch: epoch,
                selected_origin: if native { 1 } else { 3 },
                owner_peer: 0,
                registration: native.then(|| self.identity.registration_guard(0)),
            }],
        );
        self.source.lookup(wallet, session).unwrap()
    }
    async fn hint(&self, snapshot: &PeerClusterSnapshot) {
        use catalyst::comms::gatekeeper::bus::NatsBus;
        use catalyst::comms::protocol::bus::cluster::ClusterChange;
        let change = ClusterChange {
            wallet: snapshot.wallet.clone(),
            change: catalyst::comms::protocol::pulse::decentraland::pulse::PeerClusterChange {
                session: snapshot.session.clone(),
                source_incarnation: snapshot.source_incarnation.clone(),
                assignment_version: snapshot.assignment_version,
                cluster_id: snapshot.cluster_id.clone(),
                realm: snapshot.realm.clone(),
                token_not_before: snapshot.token_not_before,
                ..Default::default()
            },
        };
        self.bus.publish(&change.subject(), change.encode()).await;
    }
    async fn depart(&mut self, native: bool) {
        if native {
            self.identity.remove(0);
        }
        self.pass += 1;
        self.source
            .replace(self.pass, std::time::Duration::from_secs(120), vec![]);
        use catalyst::comms::gatekeeper::admission::cluster::fence::AssignmentFence;
        for (wallet, _) in self.fence.selected_reconciliation_targets() {
            self.fence.reconcile_selected_now_for_test(&wallet).await;
        }
    }
    async fn wait_retired(&self) {
        use catalyst::comms::gatekeeper::admission::cluster::fence::AssignmentFence;
        tokio::time::timeout(std::time::Duration::from_secs(8), async {
            while !self.fence.selected_reconciliation_targets().is_empty() {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            self.subscriber.settle().await;
        })
        .await
        .expect("started runtime periodically retires exact completed selections");
    }
    async fn shutdown(self) {
        self.jobs.shutdown(std::time::Duration::from_secs(2)).await;
        drop(self.runtime);
    }
}

#[tokio::test]
async fn started_selected_periodic_retirement_reclaims_capacity_across_native_and_legacy_churn() {
    use catalyst::comms::gatekeeper::admission::cluster::fence::AssignmentFence;
    let fixture = Fixture::new()
        .await
        .expect("required disposable PostgreSQL");
    let mut harness = RetirementHarness::new(fixture.pool.clone(), 2);
    for n in 1..=8 {
        let native = n % 2 == 1;
        let wallet = format!("0x{n:040x}");
        let selected = harness.select(&wallet, SESSION, native);
        harness.hint(&selected).await;
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            harness.subscriber.settle(),
        )
        .await
        .unwrap();
        let before: PeriodicNativeOwnerRow = sqlx::query_as("SELECT owner_session,owner_epoch,assignment_revision,fencing_token,assignment_json,updated_at FROM social.archipelago_v4_assignments WHERE deployment_audience=$1 AND owner_address=$2 AND lane_key='realm'")
            .bind(AUDIENCE).bind(&wallet).fetch_one(&fixture.pool).await.unwrap();
        assert_eq!(before.0, SESSION);
        assert_eq!(
            before.4.as_ref().unwrap()["pulseAdmission"]["selectedOrigin"].as_str(),
            Some(if native { "native" } else { "legacyIntentV2" })
        );
        assert_eq!(
            harness.fence.selected_reconciliation_targets(),
            vec![(wallet.clone(), SESSION.into())]
        );
        assert_eq!(harness.gateway.fences.lock().unwrap().len(), n as usize);
        let outage_requests = harness.bus.inner.requested().len();
        if n == 4 {
            harness
                .lookup_available
                .store(false, std::sync::atomic::Ordering::SeqCst);
        }
        harness.depart(native).await;
        if n == 4 {
            tokio::time::timeout(std::time::Duration::from_secs(8), async {
                while harness.bus.inner.requested().len() <= outage_requests {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
                harness.subscriber.settle().await;
            })
            .await
            .expect("the actual periodic job observes the simulated source outage");
            assert_eq!(
                harness.fence.selected_reconciliation_targets(),
                vec![(wallet.clone(), SESSION.into())],
                "unknown during an outage retains the legacy retry"
            );
            // Reclaim source recovery records during the outage. Capacity
            // recovery must not require an old departure record or another hint.
            harness.pass += 1000;
            harness
                .source
                .replace(harness.pass, std::time::Duration::from_secs(120), vec![]);
            harness
                .lookup_available
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
        harness.wait_retired().await;
        let after: PeriodicNativeOwnerRow = sqlx::query_as("SELECT owner_session,owner_epoch,assignment_revision,fencing_token,assignment_json,updated_at FROM social.archipelago_v4_assignments WHERE deployment_audience=$1 AND owner_address=$2 AND lane_key='realm'")
            .bind(AUDIENCE).bind(&wallet).fetch_one(&fixture.pool).await.unwrap();
        assert_eq!(
            after, before,
            "retirement never releases or mutates a fresh SQL owner"
        );
    }
    assert_eq!(
        harness.bus.inner.published().len(),
        8,
        "one admission hint per wallet; departure needs no new publication or socket reconnect"
    );
    harness.shutdown().await;
    fixture.finish().await;
}

#[tokio::test]
async fn started_selected_retirement_rejects_a_delayed_negative_after_native_reconnect_or_successor()
 {
    use catalyst::comms::gatekeeper::admission::cluster::fence::AssignmentFence;
    use std::sync::atomic::Ordering;
    for same_session in [true, false] {
        let fixture = Fixture::new()
            .await
            .expect("required disposable PostgreSQL");
        let mut harness = RetirementHarness::new(fixture.pool.clone(), 1);
        let first = harness.select(WALLET, SESSION, true);
        harness.hint(&first).await;
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            harness.subscriber.settle(),
        )
        .await
        .unwrap();
        assert_eq!(harness.gateway.fences.lock().unwrap().len(), 1);
        harness.bus.pause_negative.store(true, Ordering::SeqCst);
        harness.depart(true).await;
        tokio::time::timeout(
            std::time::Duration::from_secs(8),
            harness.bus.negative_entered.notified(),
        )
        .await
        .expect("periodic job captured a real authoritative absence before reconnect");
        let session = if same_session { SESSION } else { STALE_SESSION };
        let successor = harness.select(WALLET, session, true);
        harness.hint(&successor).await;
        if same_session {
            // A real newer selected writer can race the delayed broker response.
            // It is not used to drive retirement or the churn positive case.
            let lease = harness.fence.lock_selected(&successor).await.unwrap();
            assert!(
                lease.token_fence().owner_epoch
                    > harness.gateway.fences.lock().unwrap()[0].owner_epoch
            );
            drop(lease);
        }
        let before = periodic_native_owner_row(&fixture).await;
        harness.lookup_available.store(false, Ordering::SeqCst);
        harness.bus.negative_release.notify_one();
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            harness.subscriber.settle(),
        )
        .await
        .unwrap();
        let targets = harness.fence.selected_reconciliation_targets();
        assert!(
            targets.contains(&(WALLET.into(), session.into())),
            "stale absence cannot erase the newer desired tuple"
        );
        assert!(
            targets.len() <= 2,
            "only the desired and exact prior-owned sessions are retained"
        );
        if !same_session {
            assert!(
                targets.contains(&(WALLET.into(), SESSION.into())),
                "prior-owned cleanup work remains retained beside the successor"
            );
        }
        assert_eq!(
            periodic_native_owner_row(&fixture).await,
            before,
            "stale absence and unknown successor lookup cannot mutate either owner"
        );
        assert_eq!(harness.gateway.fences.lock().unwrap().len(), 1);
        harness.shutdown().await;
        fixture.finish().await;
    }
}

#[tokio::test]
async fn started_selected_retirement_retains_uncertain_attempt_credit_until_exact_sql_settlement() {
    use catalyst::comms::gatekeeper::admission::cluster::fence::{AssignmentFence, FenceError};
    let fixture = Fixture::new()
        .await
        .expect("required disposable PostgreSQL");
    let proxy = super::LostCommitProxy::start(&fixture.pool).await;
    let mut harness = RetirementHarness::new_unstarted(proxy.pool.clone(), 1);
    harness
        .lookup_available
        .store(false, std::sync::atomic::Ordering::SeqCst);
    let selected = harness.select(WALLET, SESSION, true);
    proxy.lose_next_commit();
    assert!(matches!(
        harness.fence.lock_selected(&selected).await,
        Err(FenceError::Unavailable)
    ));
    assert_eq!(harness.fence.unresolved_mutations(), 1);
    let credit = Arc::downgrade(&harness.fence.retained_work_credit(WALLET).unwrap());
    let mut held = fixture.pool.begin().await.unwrap();
    sqlx::query("SELECT wallet FROM social.island_admission_fences WHERE wallet=$1 FOR UPDATE")
        .bind(WALLET)
        .fetch_one(&mut *held)
        .await
        .unwrap();
    // Install the uncertainty observation and SQL blocker before periodic work can
    // reconcile the attempt. All retirement below still comes from the started runtime.
    harness.runtime.start(&harness.jobs);
    let before = periodic_native_owner_row(&fixture).await;
    let asked = harness.bus.inner.requested().len();
    harness.depart(true).await;
    harness
        .lookup_available
        .store(true, std::sync::atomic::Ordering::SeqCst);
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while !harness
            .bus
            .inner
            .requested()
            .iter()
            .skip(asked)
            .any(|(subject, _)| subject.ends_with(".selection_retirement"))
        {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        harness.subscriber.settle().await;
    })
    .await
    .expect("periodic retirement attempted, but blocked SQL cannot settle its uncertain mutation");
    assert_eq!(harness.fence.unresolved_mutations(), 1);
    assert!(
        credit.upgrade().is_some(),
        "unfinished attempt retains its original work credit"
    );
    assert_eq!(
        harness.fence.selected_reconciliation_targets(),
        vec![(WALLET.into(), SESSION.into())]
    );
    assert_eq!(periodic_native_owner_row(&fixture).await, before);
    held.rollback().await.unwrap();
    harness.wait_retired().await;
    assert_eq!(harness.fence.unresolved_mutations(), 0);
    assert!(
        credit.upgrade().is_none(),
        "exact conditional settlement returns the retained credit"
    );
    assert_eq!(
        periodic_native_owner_row(&fixture).await,
        before,
        "settlement and retirement never restore or release SQL facts"
    );
    assert!(harness.gateway.fences.lock().unwrap().is_empty());
    harness.shutdown().await;
    proxy.pool.close().await;
    drop(proxy);
    fixture.finish().await;
}

#[tokio::test]
async fn started_selected_retirement_waits_for_a_lease_and_preserves_a_real_foreign_successor() {
    use catalyst::comms::archipelago::control_v4::{AssignmentAuthority, LaneKey, V4Owner};
    use catalyst::comms::gatekeeper::admission::cluster::fence::AssignmentFence;
    let fixture = Fixture::new()
        .await
        .expect("required disposable PostgreSQL");
    let mut harness = RetirementHarness::new(fixture.pool.clone(), 1);
    let selected = harness.select(WALLET, SESSION, true);
    harness.hint(&selected).await;
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        harness.subscriber.settle(),
    )
    .await
    .unwrap();
    let outstanding = harness.fence.lock_selected(&selected).await.unwrap();
    let asked = harness.bus.inner.requested().len();
    harness.depart(true).await;
    tokio::time::timeout(std::time::Duration::from_secs(8), async {
        while !harness
            .bus
            .inner
            .requested()
            .iter()
            .skip(asked)
            .any(|(subject, _)| subject.ends_with(".selection_retirement"))
        {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        harness.subscriber.settle().await;
    })
    .await
    .unwrap();
    assert_eq!(
        harness.fence.selected_reconciliation_targets(),
        vec![(WALLET.into(), SESSION.into())],
        "an outstanding selected lease prevents slot removal"
    );
    let authority = AssignmentAuthority::pg_with_namespace(fixture.pool.clone(), AUDIENCE)
        .await
        .unwrap();
    let successor = V4Owner {
        address: WALLET.into(),
        session: STALE_SESSION.into(),
        epoch: authority.next_connection_epoch().await.unwrap(),
    };
    authority
        .claim_lanes(&successor, &[LaneKey::parse("realm").unwrap()])
        .await
        .unwrap();
    let foreign = periodic_native_owner_row(&fixture).await;
    drop(outstanding);
    harness.wait_retired().await;
    assert_eq!(
        periodic_native_owner_row(&fixture).await,
        foreign,
        "retirement cannot renew, release or restore over a real additive API successor"
    );
    assert_eq!(harness.gateway.fences.lock().unwrap().len(), 1);
    harness.shutdown().await;
    fixture.finish().await;
}
