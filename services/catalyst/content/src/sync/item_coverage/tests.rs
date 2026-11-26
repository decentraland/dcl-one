use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use parking_lot::Mutex;
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use super::*;
use crate::batch_deployer::{BatchDeployer, BatchDeployerConfig};
use crate::bloom_filter::BloomFilter;
use crate::db::scratch::{OwnedDatabase, Schema};
use crate::failed_deployments_store::LiveFailedDeploymentsStore;
use crate::jobs::{LeaseGuard, LeaseSettings, Leases};
use crate::sync_deployer::{LiveSyncDeployer, SyncValidation};
use crate::sync_test_support::{signed_chain, temp_storage, unique};

#[derive(Default)]
struct FakePeer {
    deltas: Mutex<Vec<Value>>,
    files: Mutex<HashMap<String, Vec<u8>>>,
    file_hits: Mutex<Vec<String>>,
    pages: Mutex<Vec<String>>,
    page_times: Mutex<Vec<(String, std::time::Instant)>>,
    failing: AtomicBool,
}

impl FakePeer {
    fn page(&self, query: &str) -> Option<Value> {
        self.pages.lock().push(query.to_string());
        if self.failing.load(Ordering::SeqCst) {
            return None;
        }
        let params: Vec<(String, String)> = url::form_urlencoded::parse(query.as_bytes())
            .into_owned()
            .collect();
        let types: Vec<&str> = params
            .iter()
            .filter(|(k, _)| k == "entityType")
            .map(|(_, v)| v.as_str())
            .collect();
        let get = |key: &str| {
            params
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone())
        };
        let from: i64 = get("from").map_or(0, |v| v.parse().unwrap());
        let last = get("lastId");
        let rows: Vec<Value> = self
            .deltas
            .lock()
            .iter()
            .filter(|d| types.is_empty() || types.contains(&d["entityType"].as_str().unwrap()))
            .filter(|d| {
                let ts = d["localTimestamp"].as_i64().unwrap();
                let id = d["entityId"].as_str().unwrap();
                match &last {
                    Some(last) => ts > from || (ts == from && id > last.as_str()),
                    None => ts >= from,
                }
            })
            .cloned()
            .collect();
        Some(json!({
            "deltas": rows.iter().take(2).collect::<Vec<_>>(),
            "pagination": {"moreData": rows.len() > 2}
        }))
    }

    async fn serve(self: &Arc<Self>) -> String {
        use axum::extract::{Path, RawQuery};
        use axum::http::StatusCode;
        use axum::response::IntoResponse;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let pages = self.clone();
        let files = self.clone();
        let app = axum::Router::new()
            .route(
                "/snapshots",
                axum::routing::get(|| async {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    axum::Json(json!([]))
                }),
            )
            .route(
                "/pointer-changes",
                axum::routing::get(move |RawQuery(q): RawQuery| async move {
                    let q = q.unwrap_or_default();
                    pages
                        .page_times
                        .lock()
                        .push((q.clone(), std::time::Instant::now()));
                    match pages.page(&q) {
                        Some(page) => axum::Json(page).into_response(),
                        None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
                    }
                }),
            )
            .route(
                "/contents/{hash}",
                axum::routing::get(move |Path(hash): Path<String>| async move {
                    files.file_hits.lock().push(hash.clone());
                    match files.files.lock().get(&hash) {
                        Some(body) => body.clone().into_response(),
                        None => StatusCode::NOT_FOUND.into_response(),
                    }
                }),
            );
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}")
    }

    /// Offers one entity of `kind` at `local_ts`; `None` keeps its file away.
    async fn offer(&self, kind: &str, n: u32, local_ts: i64, with_file: bool) -> String {
        let pointer = format!("urn:test:{}:{n}", unique());
        let body = serde_json::to_vec(&json!({
            "version": "v3", "type": kind, "pointers": [pointer], "timestamp": 1_000 + n,
            "content": [], "metadata": {"n": n}
        }))
        .unwrap();
        let id = crate::auth::hash_bytes_v1(&body);
        let auth_chain = serde_json::to_value(signed_chain(&id).await).unwrap();
        let mut deltas = self.deltas.lock();
        deltas.push(json!({
            "entityId": id, "entityType": kind, "pointers": [pointer], "authChain": auth_chain,
            "entityTimestamp": 1_000 + n, "localTimestamp": local_ts,
        }));
        deltas.sort_by_key(|d| {
            (
                d["localTimestamp"].as_i64().unwrap(),
                d["entityId"].as_str().unwrap().to_string(),
            )
        });
        if with_file {
            self.files.lock().insert(id.clone(), body);
        } else {
            self.files.lock().remove(&id);
        }
        id
    }
}

struct Fixture {
    db: OwnedDatabase,
    leases: Arc<Leases>,
    guard: Option<LeaseGuard>,
    owner: LeaseOwner,
    peer: Arc<FakePeer>,
    base: String,
    storage: Arc<crate::storage::Storage>,
    peers: watch::Sender<HashSet<String>>,
}

async fn fixture() -> Option<Fixture> {
    let db = OwnedDatabase::new("itemcov", Schema::Full, 8).await?;
    let settings = LeaseSettings {
        namespace: format!("itemcov-{}", &unique()[..12]),
        ..LeaseSettings::default()
    };
    let leases = Arc::new(Leases::new(db.pool.clone(), settings.clone()));
    let guard = leases.try_acquire(SYNC_LEASE).await.unwrap().unwrap();
    let pid = leases.backend_pid().await.unwrap();
    let peer = Arc::new(FakePeer::default());
    let base = peer.serve().await;
    let (storage, _) = temp_storage("itemcov").await;
    let (peers, _) = watch::channel(HashSet::from([base.clone()]));
    Some(Fixture {
        db,
        leases,
        guard: Some(guard),
        owner: LeaseOwner {
            namespace: settings.namespace,
            pid,
        },
        peer,
        base,
        storage,
        peers,
    })
}

impl Fixture {
    fn pool(&self) -> &PgPool {
        &self.db.pool
    }

    async fn replay(&self, entity_types: &[&str], history_floor_ms: i64) -> ItemHistoryReplay {
        let pool = self.pool().clone();
        let deployer = Arc::new(LiveSyncDeployer::new(
            pool.clone(),
            self.storage.clone(),
            2,
            SyncValidation::AuthChainOnly,
        ));
        let batch = Arc::new(BatchDeployer::new(
            BatchDeployerConfig::default(),
            crate::sync::test_sync_client(reqwest::Client::new()),
            self.storage.clone(),
            pool.clone(),
            deployer,
            Arc::new(LiveFailedDeploymentsStore::new(pool.clone())),
            BloomFilter::with_capacity(100),
        ));
        batch.set_peers(vec![self.base.clone()]).await;
        ItemHistoryReplay {
            client: crate::sync::test_sync_client(reqwest::Client::new()),
            deployer: batch,
            pool,
            peers: self.peers.subscribe(),
            entity_types: entity_types.iter().map(|t| t.to_string()).collect(),
            history_floor_ms,
            owner: self.owner.clone(),
            control: SyncControl::default(),
            pacing: Pacing {
                page_pause: Duration::ZERO,
                pass_interval: Duration::ZERO,
                stopped_poll: Duration::from_millis(50),
                retry_start: Duration::from_millis(50),
                retry_max: Duration::from_millis(100),
            },
        }
    }

    async fn coverage(&self, entity_type: &str) -> Coverage {
        let mut conn = self.pool().acquire().await.unwrap();
        item_history_coverage(&mut conn, entity_type).await.unwrap()
    }

    async fn sql(&self, statement: &'static str) {
        sqlx::query(statement).execute(self.pool()).await.unwrap();
    }

    async fn deployed(&self, id: &str) -> bool {
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM content.deployments WHERE entity_id = $1)")
            .bind(id)
            .fetch_one(self.pool())
            .await
            .unwrap()
    }

    async fn pass_started_at(&self) -> NaiveDateTime {
        sqlx::query_scalar("SELECT pass_started_at FROM content.item_history_coverage")
            .fetch_one(self.pool())
            .await
            .unwrap()
    }

    async fn finish(mut self) {
        if let Some(guard) = self.guard.take() {
            guard.release().await;
        }
        drop(self.leases);
        self.db.drop().await;
    }
}

async fn legacy_sync_state(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT t FROM (
            SELECT 'cursor ' || server_url || ' ' || cursor_ms || ' ' || updated_at AS t
              FROM content.server_sync_cursors
            UNION ALL SELECT 'snapshot ' || hash || ' ' || process_time FROM content.processed_snapshots
            UNION ALL SELECT 'property ' || key || ' ' || value FROM content.system_properties
         ) s ORDER BY t",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

/// A scene-only history with high cursors and processed snapshot marks hides
/// old items from the incremental sync. A strict pass recovers them from
/// zero, downloads only what the node lacks, leaves every cursor, mark and
/// property as it was, and records coverage the reader then answers.
#[tokio::test]
async fn a_leader_recovers_items_behind_its_cursors_and_a_reader_gets_covered() {
    let Some(f) = fixture().await else {
        return;
    };
    let wearable = f.peer.offer("wearable", 1, 1_000, true).await;
    let emote = f.peer.offer("emote", 2, 2_000, true).await;
    let held = f.peer.offer("wearable", 3, 3_000, true).await;
    let scene = f.peer.offer("scene", 4, 2_500, true).await;
    sqlx::query(
        "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id,
             entity_timestamp, entity_pointers, local_timestamp, auth_chain)
         VALUES ('0xabc', 'v3', 'wearable', $1, to_timestamp(1003 / 1000.0), ARRAY['held'],
                 timezone('UTC', now()), '[]')",
    )
    .bind(&held)
    .execute(f.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO content.server_sync_cursors (server_url, cursor_ms) VALUES ($1, 9000000000000)",
    )
    .bind(&f.base)
    .execute(f.pool())
    .await
    .unwrap();
    f.sql("INSERT INTO content.processed_snapshots (hash, process_time) VALUES ('bafylegacysnapshot', timezone('UTC', now()))").await;
    f.sql("INSERT INTO content.system_properties (key, value) VALUES ('sync_frontier', '9000000000000'), ('sync_state', 'Syncing')").await;
    let before = legacy_sync_state(f.pool()).await;
    assert_eq!(
        f.coverage("wearable").await,
        Coverage::Unknown(Unknown::NoRecord)
    );

    let mut replay = f.replay(&["wearable", "emote"], 0).await;
    let pass = replay
        .pass(&CancellationToken::new())
        .await
        .unwrap()
        .unwrap();
    assert_eq!((pass.debt, pass.observed_frontier_ms), (0, 3_000));
    for id in [&wearable, &emote, &held] {
        assert!(f.deployed(id).await, "{id}");
    }
    assert!(!f.deployed(&scene).await, "only item types are replayed");
    let pages = f.peer.pages.lock().clone();
    assert!(
        pages
            .iter()
            .all(|q| q.starts_with("entityType=emote&entityType=wearable&")),
        "every page asks the peer for item types only: {pages:?}"
    );
    let mut hits = f.peer.file_hits.lock().clone();
    hits.sort();
    let mut fetched = vec![wearable.clone(), emote.clone()];
    fetched.sort();
    assert_eq!(hits, fetched, "the held entity is never downloaded");
    assert_eq!(legacy_sync_state(f.pool()).await, before);
    let as_of = f.pass_started_at().await;
    assert_eq!(f.coverage("wearable").await, Coverage::Covered { as_of });
    assert_eq!(f.coverage("emote").await, Coverage::Covered { as_of });
    assert_eq!(
        f.coverage("scene").await,
        Coverage::Unknown(Unknown::ExcludedType)
    );

    let completed: NaiveDateTime =
        sqlx::query_scalar("SELECT pass_completed_at FROM content.item_history_coverage")
            .fetch_one(f.pool())
            .await
            .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    let again = replay
        .pass(&CancellationToken::new())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(again.debt, 0);
    assert!(
        again.started_at >= completed + chrono::Duration::milliseconds(300),
        "the next pass is dated from when it starts reading, not from the last completion"
    );
    assert_eq!(
        f.peer.file_hits.lock().len(),
        2,
        "a second pass downloads nothing"
    );
    f.finish().await;
}

/// The reader refuses every unproven case: no record, other proof rules, an
/// excluded type, a history floor, debt, a lease the writing session no
/// longer holds, and a pass that started too long ago.
#[tokio::test]
async fn a_reader_gets_unknown_for_every_unproven_pass() {
    let Some(mut f) = fixture().await else {
        return;
    };
    f.peer.offer("wearable", 1, 1_000, true).await;
    f.replay(&["wearable", "emote"], 0)
        .await
        .pass(&CancellationToken::new())
        .await
        .unwrap()
        .unwrap();
    let as_of = f.pass_started_at().await;
    assert_eq!(f.coverage("wearable").await, Coverage::Covered { as_of });
    let cases: [(&'static str, &'static str, Unknown); 6] = [
        (
            "UPDATE content.item_history_coverage SET algorithm = algorithm + 1",
            "UPDATE content.item_history_coverage SET algorithm = algorithm - 1",
            Unknown::Algorithm,
        ),
        (
            "UPDATE content.item_history_coverage SET entity_types = ARRAY['emote']",
            "UPDATE content.item_history_coverage SET entity_types = ARRAY['emote', 'wearable']",
            Unknown::ExcludedType,
        ),
        (
            "UPDATE content.item_history_coverage SET history_floor_ms = 1",
            "UPDATE content.item_history_coverage SET history_floor_ms = 0",
            Unknown::HistoryFloor,
        ),
        (
            "UPDATE content.item_history_coverage SET debt = 1",
            "UPDATE content.item_history_coverage SET debt = 0",
            Unknown::Debt,
        ),
        (
            "UPDATE content.item_history_coverage SET lease_backend_start = lease_backend_start - interval '1 second'",
            "UPDATE content.item_history_coverage SET lease_backend_start = lease_backend_start + interval '1 second'",
            Unknown::OwnerMismatch,
        ),
        (
            "UPDATE content.item_history_coverage SET pass_started_at = pass_started_at - interval '31 minutes'",
            "UPDATE content.item_history_coverage SET pass_started_at = pass_started_at + interval '31 minutes'",
            Unknown::Stale,
        ),
    ];
    for (breaks, restores, why) in cases {
        f.sql(breaks).await;
        assert_eq!(
            f.coverage("wearable").await,
            Coverage::Unknown(why),
            "{breaks}"
        );
        f.sql(restores).await;
        assert_eq!(
            f.coverage("wearable").await,
            Coverage::Covered { as_of },
            "{restores}"
        );
    }
    f.sql("UPDATE content.item_history_coverage SET lease_pid = lease_pid + 1")
        .await;
    assert_eq!(
        f.coverage("wearable").await,
        Coverage::Unknown(Unknown::OwnerMismatch)
    );
    f.sql("UPDATE content.item_history_coverage SET lease_pid = lease_pid - 1")
        .await;
    assert_eq!(f.coverage("wearable").await, Coverage::Covered { as_of });

    f.guard.take().unwrap().release().await;
    assert_eq!(
        f.coverage("wearable").await,
        Coverage::Unknown(Unknown::OwnerMismatch),
        "a lost or released lease leaves the pass unproven at once"
    );
    let other = Leases::new(f.pool().clone(), f.leases.settings().clone());
    let other = Arc::new(other);
    let taken = other.try_acquire(SYNC_LEASE).await.unwrap().unwrap();
    assert_eq!(
        f.coverage("wearable").await,
        Coverage::Unknown(Unknown::OwnerMismatch),
        "another session holding the lease (an older binary) proves nothing"
    );
    taken.release().await;
    f.sql("DELETE FROM content.item_history_coverage").await;
    assert_eq!(
        f.coverage("wearable").await,
        Coverage::Unknown(Unknown::NoRecord)
    );
    f.finish().await;
}

/// A writer whose session does not hold the `sync` lease writes neither
/// progress nor coverage.
#[tokio::test]
async fn a_process_without_the_sync_lease_records_nothing() {
    let Some(mut f) = fixture().await else {
        return;
    };
    f.peer.offer("wearable", 1, 1_000, true).await;
    let mut replay = f.replay(&["wearable"], 0).await;
    replay.owner.pid += 1;
    assert!(matches!(
        replay.pass(&CancellationToken::new()).await,
        Err(ReplayError::Lost)
    ));
    f.guard.take().unwrap().release().await;
    let mut replay = f.replay(&["wearable"], 0).await;
    assert!(matches!(
        replay.pass(&CancellationToken::new()).await,
        Err(ReplayError::Lost)
    ));
    let rows: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM content.item_history_replay)
              + (SELECT count(*) FROM content.item_history_coverage)",
    )
    .fetch_one(f.pool())
    .await
    .unwrap();
    assert_eq!(rows, 0);
    f.finish().await;
}

/// A delta the node cannot accept (its file is gone, a record that does not
/// parse) is debt: the pass completes unproven, and a later pass that
/// accepts everything proves coverage again.
#[tokio::test]
async fn an_item_the_node_cannot_accept_keeps_coverage_unknown_until_accepted() {
    let Some(f) = fixture().await else {
        return;
    };
    f.peer.offer("wearable", 1, 1_000, true).await;
    let missing = f.peer.offer("emote", 2, 2_000, false).await;
    let mut replay = f.replay(&["wearable", "emote"], 0).await;
    let pass = replay
        .pass(&CancellationToken::new())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pass.debt, 1);
    assert_eq!(
        f.coverage("wearable").await,
        Coverage::Unknown(Unknown::Debt)
    );
    let failed: i64 = sqlx::query_scalar("SELECT count(*) FROM content.failed_deployments")
        .fetch_one(f.pool())
        .await
        .unwrap();
    assert_eq!(failed, 1, "the failure is recorded, which proves nothing");

    f.peer
        .deltas
        .lock()
        .retain(|d| d["entityId"] != missing.as_str());
    f.peer.offer("emote", 2, 2_000, true).await;
    f.peer
        .deltas
        .lock()
        .push(json!({"entityId": "garbage", "entityType": "emote", "localTimestamp": 9_000}));
    let pass = replay
        .pass(&CancellationToken::new())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pass.debt, 1, "an unparseable record is debt");
    f.peer.deltas.lock().retain(|d| d["entityId"] != "garbage");
    let pass = replay
        .pass(&CancellationToken::new())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pass.debt, 0);
    let as_of = f.pass_started_at().await;
    assert_eq!(f.coverage("emote").await, Coverage::Covered { as_of });
    f.finish().await;
}

/// An interrupted pass resumes at its saved checkpoint under its original
/// start, and never starts from zero again.
#[tokio::test]
async fn a_restarted_leader_resumes_its_pass_where_it_stopped() {
    let Some(f) = fixture().await else {
        return;
    };
    for n in 0..5 {
        f.peer
            .offer("wearable", n, 1_000 + i64::from(n), true)
            .await;
    }
    let cancel = CancellationToken::new();
    let mut first = f.replay(&["wearable"], 0).await;
    first.pacing.retry_max = Duration::from_secs(60);
    let running = tokio::spawn({
        let cancel = cancel.clone();
        async move { first.pass(&cancel).await }
    });
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    loop {
        if !f.peer.pages.lock().is_empty() {
            f.peer.failing.store(true, Ordering::SeqCst);
        }
        let saved: Option<i64> =
            sqlx::query_scalar("SELECT from_ms FROM content.item_history_replay")
                .fetch_optional(f.pool())
                .await
                .unwrap()
                .flatten();
        if saved.is_some_and(|from| from > 0) && f.peer.pages.lock().len() >= 3 {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "no checkpoint saved");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    cancel.cancel();
    assert!(running.await.unwrap().unwrap().is_none());
    let started: NaiveDateTime =
        sqlx::query_scalar("SELECT pass_started_at FROM content.item_history_replay")
            .fetch_one(f.pool())
            .await
            .unwrap();

    f.peer.pages.lock().clear();
    f.peer.failing.store(false, Ordering::SeqCst);
    let pass = f
        .replay(&["wearable"], 0)
        .await
        .pass(&CancellationToken::new())
        .await
        .unwrap()
        .unwrap();
    assert_eq!((pass.started_at, pass.debt), (started, 0));
    let pages = f.peer.pages.lock().clone();
    assert!(
        pages.iter().all(|q| !q.contains("from=0")),
        "resumed from zero: {pages:?}"
    );
    f.finish().await;
}

/// A peer whose page still fails once the retries back off to their longest
/// wait ends its walk with debt: the pass completes, the reader gets
/// `Unknown(Debt)`, and the next pass reads the peer again.
#[tokio::test]
async fn an_unreachable_peer_ends_its_walk_with_debt_and_the_next_pass_reads_it_again() {
    let Some(f) = fixture().await else {
        return;
    };
    let wearable = f.peer.offer("wearable", 1, 1_000, true).await;
    f.peer.failing.store(true, Ordering::SeqCst);
    let mut replay = f.replay(&["wearable"], 0).await;
    let pass = tokio::time::timeout(
        Duration::from_secs(10),
        replay.pass(&CancellationToken::new()),
    )
    .await
    .expect("the pass ends while the peer is down")
    .unwrap()
    .unwrap();
    assert_eq!(pass.debt, 1);
    assert_eq!(
        f.coverage("wearable").await,
        Coverage::Unknown(Unknown::Debt)
    );
    f.peer.failing.store(false, Ordering::SeqCst);
    let pass = replay
        .pass(&CancellationToken::new())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pass.debt, 0);
    assert!(f.deployed(&wearable).await);
    assert_eq!(
        f.coverage("wearable").await,
        Coverage::Covered {
            as_of: pass.started_at
        }
    );
    f.finish().await;
}

/// The operator's switch holds the replay before its next page until cleared.
#[tokio::test]
async fn an_operator_stops_the_replay_and_nothing_more_is_fetched() {
    let Some(f) = fixture().await else {
        return;
    };
    f.peer.offer("wearable", 1, 1_000, true).await;
    f.sql("INSERT INTO content.item_history_replay (stopped) VALUES (true)")
        .await;
    let mut replay = f.replay(&["wearable"], 0).await;
    let running = tokio::spawn(async move { replay.pass(&CancellationToken::new()).await });
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(
        f.peer.pages.lock().is_empty(),
        "a stopped replay reads no page"
    );
    f.sql("UPDATE content.item_history_replay SET stopped = false")
        .await;
    let pass = tokio::time::timeout(Duration::from_secs(20), running)
        .await
        .expect("the replay resumed")
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(pass.debt, 0);
    f.finish().await;
}

/// A leader that syncs from a history floor, leaves a type out or follows
/// another peer set never reuses another scope's coverage.
#[tokio::test]
async fn a_leader_with_another_floor_types_or_peers_never_reuses_coverage() {
    let Some(f) = fixture().await else {
        return;
    };
    f.peer.offer("wearable", 1, 1_000, true).await;
    f.replay(&["wearable", "emote"], 0)
        .await
        .pass(&CancellationToken::new())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        f.coverage("emote").await,
        Coverage::Covered { .. }
    ));

    let mut floor = f.replay(&["wearable", "emote"], 500).await;
    floor
        .pass(&CancellationToken::new())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        f.coverage("wearable").await,
        Coverage::Unknown(Unknown::HistoryFloor)
    );

    let mut wearables = f.replay(&["wearable", "scene"], 0).await;
    wearables
        .pass(&CancellationToken::new())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        f.coverage("wearable").await,
        Coverage::Covered { .. }
    ));
    assert_eq!(
        f.coverage("emote").await,
        Coverage::Unknown(Unknown::ExcludedType)
    );

    let other = FakePeer::default();
    let other = Arc::new(other);
    let other_base = other.serve().await;
    other.failing.store(true, Ordering::SeqCst);
    f.peers
        .send_replace(HashSet::from([f.base.clone(), other_base]));
    let cancel = CancellationToken::new();
    let running = tokio::spawn({
        let cancel = cancel.clone();
        async move { wearables.pass(&cancel).await }
    });
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    while f.coverage("wearable").await != Coverage::Unknown(Unknown::NoRecord) {
        assert!(
            std::time::Instant::now() < deadline,
            "the old scope's coverage stayed"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    cancel.cancel();
    assert!(running.await.unwrap().unwrap().is_none());

    let mut none = f.replay(&["scene"], 0).await;
    assert!(
        none.pass(&CancellationToken::new())
            .await
            .unwrap()
            .is_none()
    );
    f.finish().await;
}

/// A leader that loses the `sync` lease in the middle of a pass, to a
/// successor session, saves no further checkpoint and completes no coverage:
/// at its first page (a checkpoint) and at its last (the completion).
#[tokio::test]
async fn a_leader_that_loses_the_lease_mid_pass_writes_nothing_more() {
    for served in [0, 1] {
        let Some(mut f) = fixture().await else {
            return;
        };
        for n in 0..3 {
            f.peer
                .offer("wearable", n, 1_000 + i64::from(n), true)
                .await;
        }
        f.peer.failing.store(served == 0, Ordering::SeqCst);
        let peer = f.peer.clone();
        let hold = tokio::spawn(async move {
            while peer.pages.lock().len() < served {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            peer.failing.store(true, Ordering::SeqCst);
        });
        let mut replay = f.replay(&["wearable"], 0).await;
        replay.pacing.page_pause = Duration::from_millis(200);
        let running = tokio::spawn(async move { replay.pass(&CancellationToken::new()).await });
        hold.await.unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while f.peer.pages.lock().len() <= served + 1 {
            assert!(
                std::time::Instant::now() < deadline,
                "the held page was never retried"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let pool = f.pool().clone();
        let checkpoint = || async {
            sqlx::query_as::<_, (Option<i64>, Option<String>)>(
                "SELECT from_ms, last_id FROM content.item_history_replay",
            )
            .fetch_one(&pool)
            .await
            .unwrap()
        };
        let before = checkpoint().await;
        f.guard.take().unwrap().release().await;
        let successor = Arc::new(Leases::new(f.pool().clone(), f.leases.settings().clone()));
        let taken = successor.try_acquire(SYNC_LEASE).await.unwrap().unwrap();
        f.peer.failing.store(false, Ordering::SeqCst);
        let outcome = tokio::time::timeout(Duration::from_secs(20), running)
            .await
            .expect("the pass ended")
            .unwrap();
        assert!(
            matches!(outcome, Err(ReplayError::Lost)),
            "after {served}: {outcome:?}"
        );
        assert_eq!(checkpoint().await, before, "after {served}");
        assert_eq!(
            f.coverage("wearable").await,
            Coverage::Unknown(Unknown::NoRecord)
        );
        let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM content.item_history_coverage")
            .fetch_one(f.pool())
            .await
            .unwrap();
        assert_eq!(
            rows, 0,
            "after {served}: a successor's lease stamps nothing for this pass"
        );
        taken.release().await;
        f.finish().await;
    }
}

/// A peer set that changes while a pass runs ends that pass before it can
/// record coverage for the set it started with.
#[tokio::test]
async fn a_peer_set_change_mid_pass_records_no_coverage_for_the_old_set() {
    let Some(f) = fixture().await else {
        return;
    };
    for n in 0..3 {
        f.peer
            .offer("wearable", n, 1_000 + i64::from(n), true)
            .await;
    }
    let mut replay = f.replay(&["wearable"], 0).await;
    replay.pacing.page_pause = Duration::from_millis(300);
    let running = tokio::spawn(async move { replay.pass(&CancellationToken::new()).await });
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    while f.peer.pages.lock().is_empty() {
        assert!(std::time::Instant::now() < deadline, "no page read");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    f.peers.send_replace(HashSet::from([
        f.base.clone(),
        "https://other.example".to_string(),
    ]));
    let outcome = tokio::time::timeout(Duration::from_secs(20), running)
        .await
        .expect("the pass ended")
        .unwrap()
        .unwrap();
    assert!(outcome.is_none(), "{outcome:?}");
    assert_eq!(
        f.coverage("wearable").await,
        Coverage::Unknown(Unknown::NoRecord)
    );
    f.finish().await;
}

/// Each checkpoint and the completion apply only over the checkpoint they
/// were computed from: a writer holding a stale one changes nothing.
#[tokio::test]
async fn a_late_write_from_a_stale_checkpoint_changes_nothing() {
    let Some(f) = fixture().await else {
        return;
    };
    let mut replay = f.replay(&["wearable"], 0).await;
    let scope = replay.scope();
    let at = replay.begin(&scope).await.unwrap();
    let next = Checkpoint {
        from_ms: 5,
        last_id: Some("bafynext".into()),
        ..at.clone()
    };
    let stale = Checkpoint {
        from_ms: 7,
        ..at.clone()
    };
    assert!(matches!(
        replay.save(&stale, &next, 0, 0).await,
        Err(ReplayError::Lost)
    ));
    assert!(matches!(
        replay.complete(&scope, &stale, 0, 0).await,
        Err(ReplayError::Lost)
    ));
    replay.save(&at, &next, 0, 0).await.unwrap();
    assert!(matches!(
        replay.save(&at, &next, 0, 0).await,
        Err(ReplayError::Lost)
    ));
    assert!(matches!(
        replay.complete(&scope, &at, 0, 0).await,
        Err(ReplayError::Lost)
    ));
    assert_eq!(
        f.coverage("wearable").await,
        Coverage::Unknown(Unknown::NoRecord)
    );
    replay.complete(&scope, &next, 0, 0).await.unwrap();
    assert!(matches!(
        f.coverage("wearable").await,
        Coverage::Covered { .. }
    ));
    f.finish().await;
}

/// A sync leader that holds the `sync` lease replays item history once its
/// bootstrap is done, and a reader then gets covered for both item types.
#[tokio::test]
async fn a_sync_leader_proves_item_coverage_after_its_bootstrap() {
    let Some(mut f) = fixture().await else {
        return;
    };
    f.peer.offer("wearable", 1, 1_000, true).await;
    f.peer.offer("emote", 2, 2_000, true).await;
    let jobs = start_leader(&mut f, Some(Duration::from_secs(1))).await;
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    let first = loop {
        if let Coverage::Covered { as_of } = f.coverage("wearable").await {
            break as_of;
        }
        assert!(std::time::Instant::now() < deadline, "no coverage recorded");
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    while !matches!(f.coverage("wearable").await, Coverage::Covered { as_of } if as_of > first) {
        assert!(
            std::time::Instant::now() < deadline,
            "no second pass one interval after the first"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(matches!(
        f.coverage("emote").await,
        Coverage::Covered { .. }
    ));
    let pages = f.peer.pages.lock().clone();
    let first_replay = pages
        .iter()
        .position(|q| q.contains("entityType="))
        .unwrap();
    let first_sync = pages
        .iter()
        .position(|q| !q.contains("entityType="))
        .unwrap();
    assert!(
        first_sync < first_replay,
        "the replay waits for the bootstrap: {pages:?}"
    );
    jobs.shutdown(Duration::from_secs(10)).await;
    f.finish().await;
}

/// An operator who sets the replay's interval to 0 (`SYNC_ITEM_REPLAY_INTERVAL_SECS`)
/// gets a sync leader that never walks the item history, and no coverage.
#[tokio::test]
async fn an_operator_who_turns_the_item_replay_off_gets_no_pass_and_no_coverage() {
    let Some(mut f) = fixture().await else {
        return;
    };
    f.peer.offer("wearable", 1, 1_000, true).await;
    let jobs = start_leader(&mut f, None).await;
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while !f
        .peer
        .pages
        .lock()
        .iter()
        .any(|q| !q.contains("entityType="))
    {
        assert!(std::time::Instant::now() < deadline, "the sync never ran");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    tokio::time::sleep(Duration::from_secs(2)).await;
    let pages = f.peer.pages.lock().clone();
    assert!(
        !pages.iter().any(|q| q.contains("entityType=")),
        "the replay read a page: {pages:?}"
    );
    assert_eq!(
        f.coverage("wearable").await,
        Coverage::Unknown(Unknown::NoRecord)
    );
    jobs.shutdown(Duration::from_secs(10)).await;
    f.finish().await;
}

/// A leader whose replay walks a two-page history reads its pages the page
/// pause apart and starts its next pass one interval after the last one
/// started, so the pass's own length doesn't delay the next proof.
#[tokio::test]
async fn a_leader_starts_its_next_item_pass_one_interval_after_the_last_started() {
    let Some(mut f) = fixture().await else {
        return;
    };
    for n in 0..3 {
        f.peer
            .offer("wearable", n, 1_000 + i64::from(n), true)
            .await;
    }
    let jobs = start_leader(&mut f, Some(Duration::from_secs(1))).await;
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    let passes = loop {
        let replay: Vec<(String, std::time::Instant)> = f
            .peer
            .page_times
            .lock()
            .iter()
            .filter(|(q, _)| q.contains("entityType="))
            .cloned()
            .collect();
        let starts: Vec<usize> = replay
            .iter()
            .enumerate()
            .filter(|(_, (q, _))| !q.contains("lastId="))
            .map(|(i, _)| i)
            .collect();
        if starts.len() >= 3 {
            break (replay, starts);
        }
        assert!(
            std::time::Instant::now() < deadline,
            "fewer than three passes"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    let (replay, starts) = passes;
    for pair in starts.windows(2) {
        let pass = &replay[pair[0]..pair[1]];
        assert!(
            pass.len() >= 2,
            "a pass of two pages read in full: {pass:?}"
        );
        for page in pass.windows(2) {
            assert!(
                page[1].1.duration_since(page[0].1) >= Duration::from_millis(350),
                "pages of a pass the page pause apart: {:?}",
                page[1].1.duration_since(page[0].1)
            );
        }
        let (_, started) = pass.first().unwrap();
        let (_, ended) = pass.last().unwrap();
        let (_, next) = &replay[pair[1]];
        assert!(
            next.duration_since(*started) >= Duration::from_millis(900),
            "the next pass started {:?} after the first page",
            next.duration_since(*started)
        );
        assert!(
            next.duration_since(*ended) < Duration::from_millis(800),
            "the next pass started {:?} after the last page",
            next.duration_since(*ended)
        );
    }
    jobs.shutdown(Duration::from_secs(10)).await;
    f.finish().await;
}

/// Starts a sync leader over the fixture's peer, with leases, its replay
/// `interval` apart (none: off).
async fn start_leader(f: &mut Fixture, interval: Option<Duration>) -> Arc<crate::jobs::Jobs> {
    use crate::sync::worker::{self, SyncSettings};
    f.guard.take().unwrap().release().await;
    let jobs = Arc::new(crate::jobs::Jobs::with_leases(
        f.pool().clone(),
        f.leases.settings().clone(),
    ));
    let status = Arc::new(crate::SyncStatus::default());
    worker::start(
        &SyncSettings {
            item_replay_interval: interval,
            item_replay_page_pause: Duration::from_millis(400),
            peers: vec![f.base.clone()],
            dao: None,
            entity_types: None,
            from_timestamp_ms: 0,
            phased_sync: true,
            content_download_concurrency: 4,
            connections_max_idle: 2,
            snapshot_concurrency: 1,
            snapshot_scheduling: crate::sync::snapshots::SnapshotScheduling::default(),
            snapshot_resync_initial_delay: Duration::from_secs(3_600),
            snapshot_resync_interval: Duration::from_secs(14 * 86_400),
            pointer_changes_wait_time_ms: 5_000,
            flush_concurrency: 1,
            bloom_expected_elements: 100,
            profile_max_age_ms: 31_536_000_000,
            retry_failed_enabled: false,
            max_retries: 3,
            retry_concurrency: 1,
            prune_ttl_days: 7,
            discard_overtaken: false,
        },
        f.pool().clone(),
        f.storage.clone(),
        status,
        crate::sync::test_sync_client(reqwest::Client::new()),
        SyncValidation::AuthChainOnly,
        &jobs,
    )
    .await
    .unwrap();
    jobs
}

/// An operator who pauses sync from the console holds the replay as well,
/// until sync resumes.
#[tokio::test]
async fn an_operator_pausing_sync_holds_the_replay() {
    let Some(f) = fixture().await else {
        return;
    };
    f.peer.offer("wearable", 1, 1_000, true).await;
    let mut replay = f.replay(&["wearable"], 0).await;
    let control = replay.control.clone();
    control.pause();
    let running = tokio::spawn(async move { replay.pass(&CancellationToken::new()).await });
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(
        f.peer.pages.lock().is_empty(),
        "a paused replay reads no page"
    );
    control.resume();
    let pass = tokio::time::timeout(Duration::from_secs(20), running)
        .await
        .expect("the replay resumed")
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(pass.debt, 0);
    f.finish().await;
}

#[tokio::test]
async fn a_reader_connected_as_the_leaders_role_sees_its_coverage_and_another_role_does_not() {
    let Some(db) = OwnedDatabase::new("itemcovrole", Schema::Full, 2).await else {
        return;
    };
    let suffix = &unique()[..10];
    let (leader, other) = (
        format!("itemcov_leader_{suffix}"),
        format!("itemcov_other_{suffix}"),
    );
    for role in [&leader, &other] {
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "CREATE ROLE {role} LOGIN PASSWORD 'example-role-pass' NOSUPERUSER; \
             GRANT USAGE ON SCHEMA content TO {role}; \
             GRANT SELECT ON content.item_history_coverage TO {role};"
        )))
        .execute(&db.pool)
        .await
        .unwrap();
    }
    let connect = |role: String| {
        let mut url = url::Url::parse(&db.url).unwrap();
        url.set_username(&role).unwrap();
        url.set_password(Some("example-role-pass")).unwrap();
        async move {
            <sqlx::PgConnection as sqlx::Connection>::connect(url.as_str())
                .await
                .unwrap()
        }
    };
    let mut holder = connect(leader.clone()).await;
    let held: bool =
        sqlx::query_scalar("SELECT pg_try_advisory_lock(hashtext('catalyst'), hashtext('sync'))")
            .fetch_one(&mut holder)
            .await
            .unwrap();
    assert!(held);
    let (pid, started): (i32, chrono::NaiveDateTime) = sqlx::query_as(
        "SELECT pg_backend_pid(), timezone('UTC', backend_start) FROM pg_stat_activity WHERE pid = pg_backend_pid()",
    )
    .fetch_one(&mut holder)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO content.item_history_coverage (algorithm, source_set, entity_types, history_floor_ms, \
           pass_started_at, pass_completed_at, observed_frontier_ms, debt, lease_namespace, lease_pid, lease_backend_start) \
         VALUES ($1, ARRAY['https://peer.example'], ARRAY['emote', 'wearable'], 0, timezone('UTC', now()), \
           timezone('UTC', now()), 0, 0, 'catalyst', $2, $3)",
    )
    .bind(ALGORITHM)
    .bind(pid)
    .bind(started)
    .execute(&db.pool)
    .await
    .unwrap();
    let mut same = connect(leader.clone()).await;
    let seen_by_leader = item_history_coverage(&mut same, "wearable").await.unwrap();
    let mut stranger = connect(other.clone()).await;
    let seen_by_other = item_history_coverage(&mut stranger, "wearable")
        .await
        .unwrap();
    for conn in [holder, same, stranger] {
        sqlx::Connection::close(conn).await.unwrap();
    }
    for role in [&leader, &other] {
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "DROP OWNED BY {role}; DROP ROLE {role};"
        )))
        .execute(&db.pool)
        .await
        .unwrap();
    }
    assert!(
        matches!(seen_by_leader, Coverage::Covered { .. }),
        "{seen_by_leader:?}"
    );
    assert_eq!(seen_by_other, Coverage::Unknown(Unknown::OwnerMismatch));
}
