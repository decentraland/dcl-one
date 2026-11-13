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
use catalyst::comms::gatekeeper::bus::{InProcessBus, RequestError};
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
}

#[async_trait]
impl ClusterGateway for RecordingGateway {
    fn safety_store(
        &self,
    ) -> Option<catalyst::comms::gatekeeper::admission::cluster::safety::SafetyStore> {
        self.pool
            .clone()
            .map(catalyst::comms::gatekeeper::admission::cluster::safety::SafetyStore::new)
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
        assert_eq!(wallet, WALLET);
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
            assert_eq!(subject, format!("peer.{WALLET}.cluster_lookup"));
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
        let safety = catalyst::comms::gatekeeper::admission::cluster::safety::SafetyStore::new(
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
            assert_eq!(subject, format!("peer.{WALLET}.cluster_lookup"));
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
