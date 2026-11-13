// from: catalyrst/crates/catalyrst-comms/tests/v4_assignment_fence.rs
//! comms' realm-lane fence (`comms::gatekeeper::admission::cluster::fence`) against the
//! `social.archipelago_v4_assignments` rows `schema.sql` creates, on a scratch
//! database per test (`CATALYST_TEST_DATABASE_URL`; skipped without it): the
//! short optimistic reads, owner/session/lapse rules, the conditional commit,
//! the snapshot reads and the subscriber's fenced recovery.

use std::time::Duration;

use catalyst::comms::gatekeeper::admission::cluster::fence::{
    AssignmentDocument, AssignmentFence, FenceError, PgAssignmentFence, RealmAssignmentReader,
};
use serde_json::json;
use sqlx::{AssertSqlSafe, PgPool, Row};

#[path = "support/db.rs"]
mod db;
#[path = "support/gate.rs"]
mod gate;

use db::{ScratchDb, scratch_database};

#[path = "comms_v4_assignment_fence/recovery.rs"]
mod recovery;
#[path = "comms_v4_assignment_fence/snapshot.rs"]
mod snapshot;

const AUDIENCE: &str = "comms-v4-assignment-fence-test";
const WALLET: &str = "0x00000000000000000000000000000000000000f1";
const SESSION: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[tokio::test]
async fn selected_identity_domains_survive_production_assignment_json_postgres_roundtrip() {
    use catalyst::comms::gatekeeper::admission::cluster::fence::PulseAdmissionStamp;
    use catalyst::comms::protocol::pulse::decentraland::pulse::PeerClusterSnapshot;
    use prost::Message;
    let fixture = Fixture::new()
        .await
        .expect("required disposable PostgreSQL");
    seed_realm(&fixture.pool, WALLET, SESSION, 7, 1, 11, None).await;
    let vectors: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/pulse_selected_recovery.json")).unwrap();
    for vector in vectors["identity_boundary_vectors"].as_array().unwrap() {
        let bytes = hex::decode(vector["wire_hex"].as_str().unwrap()).unwrap();
        let mut snapshot = PeerClusterSnapshot::decode(bytes.as_slice()).unwrap();
        snapshot.wallet = WALLET.into();
        snapshot.session = SESSION.into();
        snapshot.cluster_id = "selected".into();
        let stamp = PulseAdmissionStamp::from_snapshot(&snapshot, 0);
        if !vector["expected_selection_evidence_valid"]
            .as_bool()
            .unwrap()
        {
            assert!(stamp.is_none());
            continue;
        }
        let mut document = assignment(&fixture.pool, "island-selected", None).await;
        let safety = document.pulse_admission.as_ref().unwrap().clone();
        let mut stamp = stamp.unwrap();
        stamp.safety_generation = safety.safety_generation;
        stamp.safety_token_floor = safety.safety_token_floor;
        document.pulse_admission = Some(stamp.clone());
        let fence = fixture.fence().await;
        fence
            .lock_realm(WALLET, SESSION)
            .await
            .unwrap()
            .commit(document)
            .await
            .unwrap();
        let restored = fence
            .current_realm_assignment(WALLET, SESSION)
            .await
            .unwrap()
            .pulse_admission
            .unwrap();
        assert_eq!(restored, stamp, "{}", vector["id"]);
        assert_eq!(
            restored.selected_identity.to_string(),
            vector["identity_decimal"].as_str().unwrap()
        );
        assert!(restored.matches_snapshot(&snapshot));
        let (_, _, json) = row_state(&fixture.pool).await;
        assert_eq!(
            json.unwrap()["pulseAdmission"]["selectedIdentity"].as_u64(),
            Some(restored.selected_identity)
        );
    }
    fixture.finish().await;
}

#[tokio::test]
async fn selected_realm_ownership_tracks_native_without_ws_and_borrows_external_rows_read_only() {
    use catalyst::comms::protocol::pulse::decentraland::pulse::PeerClusterSnapshot;
    let fixture = Fixture::new()
        .await
        .expect("required disposable PostgreSQL");
    let fence = fixture.fence().await;
    let mut selected = PeerClusterSnapshot {
        wallet: WALLET.into(),
        session: SESSION.into(),
        cluster_id: "selected".into(),
        realm: "main".into(),
        source_incarnation: "selected-lifecycle".into(),
        assignment_version: 1,
        pass: 1,
        selected_origin: 1,
        selected_identity: Some(1),
        assignment_state: 1,
        token_not_before: Some(0),
        pending_takeovers: vec![],
    };
    let lease = fence.lock_selected(&selected).await.unwrap();
    assert!(!lease.read_only());
    let first = lease.token_fence();
    let mut document = assignment(&fixture.pool, "island-selected", None).await;
    let safety = document.pulse_admission.as_ref().unwrap().clone();
    let mut stamp =
        catalyst::comms::gatekeeper::admission::cluster::fence::PulseAdmissionStamp::from_snapshot(
            &selected, 0,
        )
        .unwrap();
    stamp.safety_generation = safety.safety_generation;
    stamp.safety_token_floor = safety.safety_token_floor;
    document.pulse_admission = Some(stamp);
    lease.commit(document.clone()).await.unwrap();
    let before: (i64,i64,chrono::NaiveDateTime) = sqlx::query_as("SELECT owner_epoch, assignment_revision, updated_at FROM social.archipelago_v4_assignments WHERE deployment_audience=$1 AND owner_address=$2 AND lane_key='realm'")
        .bind(AUDIENCE).bind(WALLET).fetch_one(&fixture.pool).await.unwrap();
    // A restarted reader has no proof it owns this still-fresh row.
    let restarted = fixture.fence().await;
    let borrowed = restarted.lock_selected(&selected).await.unwrap();
    assert!(borrowed.read_only());
    assert_eq!(
        borrowed.commit(document).await.unwrap_err(),
        FenceError::Conflict
    );
    let after: (i64,i64,chrono::NaiveDateTime) = sqlx::query_as("SELECT owner_epoch, assignment_revision, updated_at FROM social.archipelago_v4_assignments WHERE deployment_audience=$1 AND owner_address=$2 AND lane_key='realm'")
        .bind(AUDIENCE).bind(WALLET).fetch_one(&fixture.pool).await.unwrap();
    assert_eq!(before, after, "borrowing cannot renew or mutate the owner");
    selected.session = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into();
    selected.selected_origin = 3;
    selected.selected_identity = Some(700);
    selected.assignment_version += 1;
    assert!(matches!(
        restarted.lock_selected(&selected).await,
        Err(FenceError::Conflict)
    ));
    let foreign_wallet = "0x00000000000000000000000000000000000000f2";
    seed_realm(&fixture.pool, foreign_wallet, SESSION, 1, 1, 1, None).await;
    let mut external = selected.clone();
    external.wallet = foreign_wallet.into();
    external.session = SESSION.into();
    let borrowed_null = restarted.lock_selected(&external).await.unwrap();
    assert!(borrowed_null.read_only());
    assert!(borrowed_null.previous_pulse_admission().is_none());
    drop(borrowed_null);
    let null_unchanged:bool=sqlx::query_scalar("SELECT assignment_json IS NULL FROM social.archipelago_v4_assignments WHERE deployment_audience=$1 AND owner_address=$2 AND lane_key='realm'")
        .bind(AUDIENCE).bind(foreign_wallet).fetch_one(&fixture.pool).await.unwrap();
    assert!(null_unchanged);
    let successor = fence.lock_selected(&selected).await.unwrap();
    let second = successor.token_fence();
    assert!(second.owner_epoch > first.owner_epoch);
    assert!(second.fencing_token > first.fencing_token);
    drop(successor);
    // Native priority changes authority through the known exact predecessor, without /ws.
    selected.session = SESSION.into();
    selected.selected_origin = 1;
    selected.selected_identity = Some(2);
    selected.assignment_version += 1;
    let native = fence.lock_selected(&selected).await.unwrap();
    assert!(native.token_fence().owner_epoch > second.owner_epoch);
    assert_eq!(native.token_fence().owner_session, SESSION);
    drop(native);
    // A fresh foreign owner cannot be replaced from a different coordinator.
    selected.session = "0xcccccccccccccccccccccccccccccccccccccccc".into();
    assert!(matches!(
        restarted.lock_selected(&selected).await,
        Err(FenceError::Conflict)
    ));
    sqlx::query("UPDATE social.archipelago_v4_assignments SET updated_at=clock_timestamp()-interval '91 seconds' WHERE deployment_audience=$1 AND owner_address=$2 AND lane_key='realm'")
        .bind(AUDIENCE).bind(WALLET).execute(&fixture.pool).await.unwrap();
    let recovered = restarted.lock_selected(&selected).await.unwrap();
    assert!(!recovered.read_only());
    assert_eq!(recovered.token_fence().owner_session, selected.session);
    drop(recovered);
    fixture.finish().await;
}

/// Drops the first armed COMMIT CommandComplete after PostgreSQL executed it.
/// All SQL and row locks are real; only the acknowledgment transport is faulted.
struct LostCommitProxy {
    pool: PgPool,
    armed: std::sync::Arc<std::sync::atomic::AtomicBool>,
    task: tokio::task::JoinHandle<()>,
}
impl LostCommitProxy {
    async fn start(pool: &PgPool) -> Self {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let options = pool.connect_options();
        let host = options.get_host().to_string();
        let port = options.get_port();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_port = listener.local_addr().unwrap().port();
        let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let fault = armed.clone();
        let task = tokio::spawn(async move {
            let mut connections = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept(), if connections.len()<4 => {
                        let (client,_) = accepted.unwrap(); let host=host.clone(); let fault=fault.clone();
                        connections.spawn(async move {
                            let server=tokio::net::TcpStream::connect((host.as_str(),port)).await.unwrap();
                            let (mut client_read,mut client_write)=client.into_split();
                            let (mut server_read,mut server_write)=server.into_split();
                            let upstream=tokio::io::copy(&mut client_read,&mut server_write);
                            let downstream=async {
                                loop {
                                    let mut header=[0u8;5]; if server_read.read_exact(&mut header).await.is_err(){return;}
                                    let size=u32::from_be_bytes(header[1..5].try_into().unwrap()) as usize;
                                    assert!((4..=256*1024).contains(&size)); let mut body=vec![0;size-4];
                                    if server_read.read_exact(&mut body).await.is_err(){return;}
                                    if header[0]==b'C' && body==b"COMMIT\0" && fault.swap(false,std::sync::atomic::Ordering::SeqCst){return;}
                                    if client_write.write_all(&header).await.is_err() || client_write.write_all(&body).await.is_err(){return;}
                                }
                            };
                            tokio::select! { _=upstream=>{}, _=downstream=>{} }
                        });
                    }
                    _=connections.join_next(), if !connections.is_empty()=>{}
                }
            }
        });
        let redirected = options
            .as_ref()
            .clone()
            .host("127.0.0.1")
            .port(proxy_port)
            .ssl_mode(sqlx::postgres::PgSslMode::Disable);
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_with(redirected)
            .await
            .unwrap();
        Self { pool, armed, task }
    }
    fn lose_next_commit(&self) {
        self.armed.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}
impl Drop for LostCommitProxy {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[tokio::test]
async fn lost_selected_claim_and_publication_acknowledgments_reconcile_without_restoring_a_predecessor()
 {
    use catalyst::comms::protocol::pulse::decentraland::pulse::PeerClusterSnapshot;
    let fixture = Fixture::new()
        .await
        .expect("required disposable PostgreSQL");
    let proxy = LostCommitProxy::start(&fixture.pool).await;
    let fence = PgAssignmentFence::new(proxy.pool.clone(), AUDIENCE.into()).unwrap();
    let mut selected = PeerClusterSnapshot {
        wallet: WALLET.into(),
        session: SESSION.into(),
        cluster_id: "selected".into(),
        realm: "main".into(),
        source_incarnation: "lost-ack".into(),
        assignment_version: 1,
        pass: 1,
        selected_origin: 1,
        selected_identity: Some(1),
        assignment_state: 1,
        token_not_before: Some(0),
        pending_takeovers: vec![],
    };
    proxy.lose_next_commit();
    assert!(matches!(
        fence.lock_selected(&selected).await,
        Err(FenceError::Unavailable)
    ));
    assert_eq!(fence.unresolved_mutations(), 1);
    let committed_epoch:i64=sqlx::query_scalar("SELECT owner_epoch FROM social.archipelago_v4_assignments WHERE deployment_audience=$1 AND owner_address=$2 AND lane_key='realm'").bind(AUDIENCE).bind(WALLET).fetch_one(&fixture.pool).await.unwrap();
    let lease = fence.lock_selected(&selected).await.unwrap();
    assert_eq!(
        lease.token_fence().owner_epoch,
        committed_epoch as u64,
        "reconciliation reuses the committed attempt rather than claiming a new epoch"
    );
    assert_eq!(fence.unresolved_mutations(), 0);
    let mut document = assignment(&fixture.pool, "island-selected", None).await;
    let safety = document.pulse_admission.as_ref().unwrap().clone();
    let mut stamp =
        catalyst::comms::gatekeeper::admission::cluster::fence::PulseAdmissionStamp::from_snapshot(
            &selected, 0,
        )
        .unwrap();
    stamp.safety_generation = safety.safety_generation;
    stamp.safety_token_floor = safety.safety_token_floor;
    document.pulse_admission = Some(stamp);
    proxy.lose_next_commit();
    assert_eq!(
        lease.commit(document.clone()).await.unwrap_err(),
        FenceError::Unavailable
    );
    assert_eq!(fence.unresolved_mutations(), 1);
    let stored:serde_json::Value=sqlx::query_scalar("SELECT assignment_json FROM social.archipelago_v4_assignments WHERE deployment_audience=$1 AND owner_address=$2 AND lane_key='realm'").bind(AUDIENCE).bind(WALLET).fetch_one(&fixture.pool).await.unwrap();
    assert_eq!(stored, serde_json::to_value(document).unwrap());
    selected.selected_identity = Some(2);
    selected.assignment_version += 1;
    let same_session = fence.lock_selected(&selected).await.unwrap();
    let same_epoch = same_session.token_fence().owner_epoch;
    assert!(same_epoch > committed_epoch as u64);
    assert_eq!(same_session.token_fence().owner_session, SESSION);
    drop(same_session);
    selected.session = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into();
    selected.selected_identity = Some(3);
    selected.assignment_version += 1;
    let replacement = fence.lock_selected(&selected).await.unwrap();
    assert!(replacement.token_fence().owner_epoch > same_epoch);
    assert!(replacement.token_fence().owner_epoch > committed_epoch as u64);
    assert_eq!(replacement.token_fence().owner_session, selected.session);
    drop(replacement);
    assert_eq!(fence.unresolved_mutations(), 0);
    let row:(String,Option<serde_json::Value>)=sqlx::query_as("SELECT owner_session,assignment_json FROM social.archipelago_v4_assignments WHERE deployment_audience=$1 AND owner_address=$2 AND lane_key='realm'").bind(AUDIENCE).bind(WALLET).fetch_one(&fixture.pool).await.unwrap();
    assert_eq!(row.0, selected.session);
    assert!(
        row.1.is_none(),
        "old committed credentials are not restored onto the successor"
    );
    proxy.pool.close().await;
    drop(proxy);
    fixture.finish().await;
}

struct Fixture {
    db: Option<ScratchDb>,
    pool: PgPool,
}

impl Fixture {
    async fn new() -> Option<Self> {
        gate::required_database_in_ci();
        let db = scratch_database().await?;
        let pool = db.pool.clone();
        Some(Self { db: Some(db), pool })
    }

    async fn finish(mut self) {
        if let Some(db) = self.db.take() {
            db.teardown().await;
        }
    }

    async fn fence(&self) -> PgAssignmentFence {
        PgAssignmentFence::new(self.pool.clone(), AUDIENCE.to_string()).expect("a valid audience")
    }
}

async fn seed_realm(
    pool: &PgPool,
    wallet: &str,
    session: &str,
    owner_epoch: i64,
    assignment_revision: i64,
    fencing_token: i64,
    assignment_json: Option<serde_json::Value>,
) {
    sqlx::query(
        "INSERT INTO social.archipelago_v4_assignments (
            deployment_audience, owner_address, lane_key, authority_incarnation,
            assignment_revision, owner_session, owner_epoch, fencing_token, assignment_json
        )
        VALUES ($1, $2, 'realm', 'authority-test-incarnation', $3, $4, $5, $6, $7)",
    )
    .bind(AUDIENCE)
    .bind(wallet)
    .bind(assignment_revision)
    .bind(session)
    .bind(owner_epoch)
    .bind(fencing_token)
    .bind(assignment_json)
    .execute(pool)
    .await
    .expect("seed realm owner row");
}

async fn assignment(
    pool: &PgPool,
    island_id: &str,
    from_island_id: Option<&str>,
) -> AssignmentDocument {
    let safety =
        catalyst::comms::gatekeeper::admission::cluster::safety::SafetyStore::new(pool.clone())
            .snapshot(WALLET)
            .await
            .unwrap();
    let mut stamp =
        catalyst::comms::gatekeeper::admission::cluster::fence::PulseAdmissionStamp::from_snapshot(
            &catalyst::comms::protocol::pulse::decentraland::pulse::PeerClusterSnapshot {
                selected_origin: 1,
                selected_identity: Some(1),
                assignment_state: 1,
                wallet: WALLET.into(),
                session: SESSION.into(),
                cluster_id: island_id
                    .strip_prefix("island-")
                    .unwrap_or(island_id)
                    .into(),
                realm: "realm".into(),
                source_incarnation: "fence-test".into(),
                assignment_version: 1,
                token_not_before: Some(0),
                ..Default::default()
            },
            0,
        )
        .unwrap();
    stamp.safety_generation = safety.generation;
    stamp.safety_token_floor = safety.token_floor;
    AssignmentDocument {
        island_id: island_id.to_string(),
        connection_string: format!("livekit:wss://example.invalid/{island_id}"),
        from_island_id: from_island_id.map(str::to_string),
        token_expires_at_unix: None,
        peers: Default::default(),
        pulse_admission: Some(stamp),
    }
}

async fn row_state(pool: &PgPool) -> (i64, i64, Option<serde_json::Value>) {
    let row = sqlx::query(
        "SELECT assignment_revision, fencing_token, assignment_json
         FROM social.archipelago_v4_assignments
         WHERE deployment_audience = $1 AND owner_address = $2 AND lane_key = 'realm'",
    )
    .bind(AUDIENCE)
    .bind(WALLET)
    .fetch_one(pool)
    .await
    .expect("load realm row");
    (
        row.try_get("assignment_revision").unwrap(),
        row.try_get("fencing_token").unwrap(),
        row.try_get("assignment_json").unwrap(),
    )
}

#[tokio::test]
async fn optimistic_leases_release_sql_locks_and_stale_snapshot_cannot_commit() {
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    seed_realm(&fixture.pool, WALLET, SESSION, 4, 1, 9, None).await;

    let first_fence = fixture.fence().await;
    let second_fence = fixture.fence().await;
    let first = first_fence
        .lock_realm(WALLET, SESSION)
        .await
        .expect("first lease");
    let first_token = first.token_fence();
    assert_eq!(first_token.assignment_revision, 2);
    assert_eq!(first_token.fencing_token, 9);

    let second = tokio::time::timeout(
        Duration::from_secs(2),
        second_fence.lock_realm(WALLET, SESSION),
    )
    .await
    .expect("snapshot read does not wait on a retained lease")
    .unwrap();
    assert_eq!(second.token_fence().assignment_revision, 2);
    first
        .commit(assignment(&fixture.pool, "island-after-first", None).await)
        .await
        .unwrap();
    assert_eq!(
        second
            .commit(assignment(&fixture.pool, "island-stale-snapshot", None).await)
            .await,
        Err(FenceError::Conflict)
    );
    let (revision, fence, document) = row_state(&fixture.pool).await;
    assert_eq!(revision, 2);
    assert_eq!(fence, 9);
    assert_eq!(
        document
            .as_ref()
            .and_then(|value| value["islandId"].as_str()),
        Some("island-after-first")
    );

    fixture.finish().await;
}

#[tokio::test]
async fn a_wallet_no_socket_claimed_is_unowned_and_is_told_apart_from_a_stale_session() {
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    seed_realm(&fixture.pool, WALLET, SESSION, 2, 3, 5, None).await;

    let fence = fixture.fence().await;
    let unclaimed = "0x00000000000000000000000000000000000000f2";
    let err = match fence.lock_realm(unclaimed, SESSION).await {
        Ok(_) => panic!("a wallet without a row has no lease to give"),
        Err(err) => err,
    };
    assert_eq!(err, FenceError::Unowned);
    assert_eq!(row_state(&fixture.pool).await, (3, 5, None));

    fixture.finish().await;
}

#[tokio::test]
async fn stale_session_gets_no_lease() {
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    seed_realm(&fixture.pool, WALLET, SESSION, 2, 3, 5, None).await;

    let fence = fixture.fence().await;
    let err = match fence.lock_realm(WALLET, "session-stale").await {
        Ok(_) => panic!("stale session must not receive a lease"),
        Err(err) => err,
    };
    assert_eq!(err, FenceError::Conflict);
    assert_eq!(row_state(&fixture.pool).await, (3, 5, None));

    fixture.finish().await;
}

async fn set_realm_renewal(pool: &PgPool, renewed_at: &str) {
    sqlx::query(AssertSqlSafe(format!(
        "UPDATE social.archipelago_v4_assignments SET updated_at = {renewed_at}
         WHERE deployment_audience = $1 AND owner_address = $2 AND lane_key = 'realm'"
    )))
    .bind(AUDIENCE)
    .bind(WALLET)
    .execute(pool)
    .await
    .expect("set realm renewal time");
}

#[tokio::test]
async fn lapsed_and_released_owners_cannot_remint_or_enable_legacy_fallback() {
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    seed_realm(&fixture.pool, WALLET, SESSION, 2, 3, 5, None).await;
    let fence = fixture.fence().await;

    for lapsed in ["now() - interval '10 minutes'", "'-infinity'"] {
        set_realm_renewal(&fixture.pool, lapsed).await;
        let before: serde_json::Value = sqlx::query_scalar(
            "SELECT to_jsonb(a) FROM social.archipelago_v4_assignments a
             WHERE deployment_audience = $1 AND owner_address = $2 AND lane_key = 'realm'",
        )
        .bind(AUDIENCE)
        .bind(WALLET)
        .fetch_one(&fixture.pool)
        .await
        .expect("snapshot the complete authority row");
        for session in [SESSION, "session-other"] {
            let err = match fence.lock_realm(WALLET, session).await {
                Ok(_) => panic!("a lapsed or released row must not grant any mutation lease"),
                Err(err) => err,
            };
            assert_eq!(err, FenceError::Conflict);
            assert_eq!(
                fence
                    .current_realm_assignment(WALLET, session)
                    .await
                    .unwrap_err(),
                FenceError::Unowned
            );
            let after: serde_json::Value = sqlx::query_scalar(
                "SELECT to_jsonb(a) FROM social.archipelago_v4_assignments a
                 WHERE deployment_audience = $1 AND owner_address = $2 AND lane_key = 'realm'",
            )
            .bind(AUDIENCE)
            .bind(WALLET)
            .fetch_one(&fixture.pool)
            .await
            .expect("read the unchanged authority row");
            assert_eq!(after, before);
        }
    }
    assert_eq!(row_state(&fixture.pool).await, (3, 5, None));

    set_realm_renewal(&fixture.pool, "now() - interval '20 seconds'").await;
    let err = match fence.lock_realm(WALLET, "session-other").await {
        Ok(_) => panic!("a renewing owner keeps the wallet"),
        Err(err) => err,
    };
    assert_eq!(err, FenceError::Conflict);

    fixture.finish().await;
}

#[tokio::test]
async fn an_owner_expiring_during_mint_cannot_commit_or_renew_the_row() {
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    seed_realm(&fixture.pool, WALLET, SESSION, 2, 3, 5, None).await;
    let fence = fixture.fence().await;
    let lease = fence.lock_realm(WALLET, SESSION).await.expect("live lease");

    set_realm_renewal(&fixture.pool, "clock_timestamp() - interval '91 seconds'").await;
    assert_eq!(
        lease
            .commit(assignment(&fixture.pool, "island-too-late", None).await)
            .await,
        Err(FenceError::Conflict)
    );
    assert_eq!(row_state(&fixture.pool).await, (3, 5, None));
    assert_eq!(
        fence.current_realm_assignment(WALLET, SESSION).await,
        Err(FenceError::Unowned)
    );
    fixture.finish().await;
}

#[tokio::test]
async fn an_owner_expiring_while_waiting_for_the_row_lock_gets_no_lease() {
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    seed_realm(&fixture.pool, WALLET, SESSION, 2, 3, 5, None).await;
    let mut blocker = fixture.pool.begin().await.expect("blocking transaction");
    sqlx::query("SELECT owner_session FROM social.archipelago_v4_assignments FOR UPDATE")
        .fetch_all(&mut *blocker)
        .await
        .expect("hold the authority row lock");
    let blocker_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let fence = fixture.fence().await;
    let waiter = tokio::spawn(async move { fence.lock_realm(WALLET, SESSION).await });
    assert!(catalyst::shared::test_wait::until(async || {
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE wait_event_type='Lock' AND $1=ANY(pg_blocking_pids(pid)))")
            .bind(blocker_pid).fetch_one(&fixture.pool).await.unwrap()
    }).await);
    sqlx::query("UPDATE social.archipelago_v4_assignments SET updated_at=clock_timestamp()-interval '91 seconds'")
        .execute(&mut *blocker).await.unwrap();
    assert!(!waiter.is_finished());
    blocker.commit().await.expect("release the row lock");
    match waiter.await.expect("waiting task") {
        Ok(_) => panic!("a row that expired while waiting must not grant a lease"),
        Err(error) => assert_eq!(error, FenceError::Conflict),
    }
    assert_eq!(row_state(&fixture.pool).await, (3, 5, None));
    fixture.finish().await;
}

#[tokio::test]
async fn conditional_commit_writes_shared_assignment_document_and_revision() {
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    seed_realm(
        &fixture.pool,
        WALLET,
        SESSION,
        7,
        5,
        11,
        Some(json!({
            "islandId": "island-old",
            "connectionString": "livekit:wss://example.invalid/island-old"
        })),
    )
    .await;

    let fence = fixture.fence().await;
    let lease = fence.lock_realm(WALLET, SESSION).await.expect("lease");
    assert_eq!(lease.previous_island().as_deref(), Some("island-old"));
    let token = lease.token_fence();
    assert_eq!(token.audience, AUDIENCE);
    assert_eq!(token.lane, "realm");
    assert_eq!(token.owner_session, SESSION);
    assert_eq!(token.owner_epoch, 7);
    assert_eq!(token.assignment_revision, 6);
    assert_eq!(token.fencing_token, 11);

    lease
        .commit(assignment(&fixture.pool, "island-new", Some("island-old")).await)
        .await
        .expect("commit shared assignment document");

    let (revision, fence, document) = row_state(&fixture.pool).await;
    let document = document.expect("assignment document");
    assert_eq!(revision, 6);
    assert_eq!(fence, 11);
    assert_eq!(document["islandId"], "island-new");
    assert_eq!(
        document["connectionString"],
        "livekit:wss://example.invalid/island-new"
    );
    assert_eq!(document["fromIslandId"], "island-old");
    assert!(
        document.get("peers").is_none(),
        "empty peer maps stay omitted in the shared document shape"
    );

    fixture.finish().await;
}

#[tokio::test]
async fn cancelled_lease_releases_lock_without_assignment() {
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    seed_realm(&fixture.pool, WALLET, SESSION, 3, 1, 2, None).await;

    let first_fence = fixture.fence().await;
    let second_fence = fixture.fence().await;
    let first = first_fence
        .lock_realm(WALLET, SESSION)
        .await
        .expect("first lease");
    let second = tokio::time::timeout(
        Duration::from_secs(2),
        second_fence.lock_realm(WALLET, SESSION),
    )
    .await
    .expect("no lock retained across work")
    .unwrap();
    drop(first);
    assert_eq!(second.token_fence().assignment_revision, 2);
    drop(second);
    assert_eq!(row_state(&fixture.pool).await, (1, 2, None));

    fixture.finish().await;
}
