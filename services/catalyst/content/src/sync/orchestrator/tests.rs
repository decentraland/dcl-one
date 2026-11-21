use super::SnapshotPassVerdict::*;
use super::*;

fn snapshot(hash: &str, end: Timestamp) -> SnapshotMetadata {
    SnapshotMetadata {
        hash: hash.to_string(),
        time_range: TimeRange {
            init_timestamp: 0,
            end_timestamp: end,
        },
        number_of_entities: 1,
        replaced_snapshot_hashes: None,
        generation_timestamp: end,
    }
}

/// Returns the servers the fetch held back.
fn fetch(
    server: &str,
    snapshots: Vec<SnapshotMetadata>,
    discarded: usize,
    genesis: Timestamp,
    last_ts: &mut HashMap<String, Timestamp>,
    by_hash: &mut SnapshotsByHash,
) -> HashSet<String> {
    let mut held = HashSet::new();
    apply_snapshot_fetch(
        server,
        snapshots::SnapshotsFromServer {
            snapshots,
            discarded,
        },
        genesis,
        last_ts,
        by_hash,
        &mut held,
    );
    held
}

#[test]
fn discarded_snapshot_entries_hold_the_server_back() {
    let (mut last_ts, mut by_hash) = (HashMap::new(), HashMap::new());
    let snaps = || vec![snapshot("QmClean", 1_700_000_000_000)];
    let held = fetch(
        "https://peer.test",
        snaps(),
        2,
        0,
        &mut last_ts,
        &mut by_hash,
    );
    assert!(
        held.contains("https://peer.test"),
        "a partially-readable snapshot list must hold the server in bootstrap"
    );
    assert!(by_hash.contains_key("QmClean"));

    let held_clean = fetch("https://ok.test", snaps(), 0, 0, &mut last_ts, &mut by_hash);
    assert!(held_clean.is_empty());
    assert_eq!(last_ts["https://ok.test"], 1_700_000_000_000);
    assert_eq!(by_hash["QmClean"].0.len(), 2);
    assert_eq!(by_hash["QmClean"].1.len(), 2);
}

#[test]
fn empty_snapshot_list_falls_back_to_genesis() {
    let (mut last_ts, mut by_hash) = (HashMap::new(), HashMap::new());
    let held = fetch(
        "https://new.test",
        vec![],
        0,
        42,
        &mut last_ts,
        &mut by_hash,
    );
    assert_eq!(last_ts["https://new.test"], 42);
    assert!(held.is_empty());
}

#[test]
fn failed_pass_always_holds_regardless_of_history() {
    for bits in 0..16u8 {
        let b = |i: u8| bits & (1 << i) != 0;
        assert_eq!(snapshot_pass_verdict(true, b(0), b(1), b(2), b(3)), Failed);
    }
}

#[test]
fn non_marking_slice_remembers_clean_snapshots() {
    assert_eq!(
        snapshot_pass_verdict(false, false, false, false, false),
        CleanUnmarked
    );
    assert_eq!(
        snapshot_pass_verdict(false, false, false, true, false),
        CleanUnmarked
    );
}

#[test]
fn clean_unfiltered_pass_retires_even_previous_failures() {
    assert_eq!(snapshot_pass_verdict(false, true, true, true, true), Retire);
    assert_eq!(
        snapshot_pass_verdict(false, true, true, false, false),
        Retire
    );
}

#[test]
fn filtered_marking_pass_needs_the_complementary_slice_proven() {
    assert_eq!(
        snapshot_pass_verdict(false, true, false, false, true),
        Retire
    );
    assert_eq!(
        snapshot_pass_verdict(false, true, false, false, false),
        Unproven
    );
    assert_eq!(
        snapshot_pass_verdict(false, true, false, true, true),
        Unproven
    );
}

#[test]
fn allowlist_defines_what_a_complete_pass_is() {
    let set = |types: &[&str]| -> HashSet<String> { types.iter().map(|s| s.to_string()).collect() };
    assert!(pass_covers_all_synced_types(None, None));
    assert!(pass_covers_all_synced_types(None, Some(&set(&["scene"]))));
    assert!(!pass_covers_all_synced_types(Some(&set(&["scene"])), None));
    assert!(pass_covers_all_synced_types(
        Some(&set(&["scene"])),
        Some(&set(&["scene"]))
    ));
    assert!(pass_covers_all_synced_types(
        Some(&set(&["scene", "wearable"])),
        Some(&set(&["scene"]))
    ));
    assert!(!pass_covers_all_synced_types(
        Some(&set(&["profile"])),
        Some(&set(&["scene", "profile"]))
    ));
}

#[test]
fn a_server_resumes_from_its_own_cursor_even_when_it_lags_the_global_frontier() {
    assert_eq!(resume_point(Some(1_000)), 1_000);
    assert_eq!(resume_point(Some(9_500)), 9_500);
}

#[test]
fn a_server_with_no_cursor_row_starts_from_the_config_floor() {
    assert_eq!(
        resume_point(None),
        0,
        "never the global frontier: the server's own snapshots set its start"
    );
}

#[test]
fn deployment_report_incomplete_until_all_acknowledged() {
    let report = DeploymentReport::default();
    assert!(report.is_complete(), "an empty run is trivially complete");
    report.record_scheduled();
    report.record_scheduled();
    assert!(!report.is_complete());
    report.record_acknowledged();
    assert!(!report.is_complete());
    report.record_acknowledged();
    assert!(report.is_complete());
}

#[tokio::test]
async fn the_bootstrap_signal_settles_on_a_transition_that_already_happened() {
    let status = Arc::new(SyncStatus::default());
    let signal = BootstrapSignal {
        bootstrap_done: Arc::new(Notify::new()),
        status: status.clone(),
    };
    status.set_state(SyncState::PartiallySynced {
        ready_types: vec!["scene".into()],
    });
    signal.bootstrap_done.notify_waiters();
    assert!(
        futures::poll!(std::pin::pin!(signal.wait_settled())).is_pending(),
        "a partially synced node keeps the waiter waiting"
    );

    status.set_state(SyncState::Syncing);
    signal.bootstrap_done.notify_waiters();
    futures::FutureExt::now_or_never(signal.wait_settled())
        .expect("a transition that already happened must not be missed");
}

#[tokio::test]
async fn bootstrap_callbacks_require_success_and_keep_parent_cancellation_scope() {
    use crate::batch_deployer::BatchDeployerConfig;
    use crate::bloom_filter::BloomFilter;
    use crate::failed_deployments_store::LiveFailedDeploymentsStore;
    use crate::jobs::Jobs;
    use crate::sync_deployer::{LiveSyncDeployer, SyncValidation};
    use std::sync::atomic::AtomicUsize;

    // A partially synced node is still bootstrapping profiles: the callback (the
    // failed-deployments retry) waits for `Syncing`.
    let partial = SyncState::PartiallySynced {
        ready_types: vec!["scene".into()],
    };
    for (states, cancelled, expected) in [
        (vec![SyncState::Syncing], false, 1),
        (vec![partial, SyncState::Syncing], false, 1),
        (vec![SyncState::Failed], false, 0),
        (vec![SyncState::Syncing], true, 0),
    ] {
        let (storage, temp) = crate::sync_test_support::temp_storage("bootstrap-callback").await;
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://nobody@127.0.0.1:1/unused")
            .unwrap();
        let status = Arc::new(SyncStatus::default());
        let client = crate::sync::test_sync_client(reqwest::Client::new());
        let deployer = Arc::new(BatchDeployer::new(
            BatchDeployerConfig::default(),
            client.clone(),
            storage.clone(),
            pool.clone(),
            Arc::new(LiveSyncDeployer::new(
                pool.clone(),
                storage.clone(),
                1,
                SyncValidation::AuthChainOnly,
            )),
            Arc::new(LiveFailedDeploymentsStore::new(pool.clone())),
            BloomFilter::with_capacity(1),
        ));
        let orchestrator = Arc::new(SyncOrchestrator::new(
            SyncOrchestratorConfig::default(),
            SyncDependencies {
                http_client: client,
                storage,
                deployer,
                processed_store: Arc::new(LiveProcessedSnapshotStore::new(pool.clone())),
                deployment_repo: Arc::new(LiveDeploymentRepository::new(pool, status.clone())),
                status,
                control: SyncControl::default(),
            },
        ));
        let jobs = Arc::new(Jobs::new());
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let terminated = Arc::new(AtomicUsize::new(0));
        let ended = terminated.clone();
        let registry = jobs.clone();
        let sync = orchestrator.clone();
        let (entered, started) = tokio::sync::oneshot::channel();
        jobs.spawn("bootstrap-test", move |ctx| async move {
            let scope = ctx.clone();
            entered.send(ctx.cancel.clone()).unwrap();
            let (_peers_tx, peers) = tokio::sync::watch::channel(HashSet::new());
            let retry_peers = Arc::new(tokio::sync::RwLock::new(Vec::new()));
            run_sync_job(sync, peers, retry_peers, ctx, || {
                observed.fetch_add(1, SeqCst);
                registry.spawn_scoped(&scope, "after-bootstrap", |child| async move {
                    child.cancel.cancelled().await;
                    ended.fetch_add(1, SeqCst);
                    Ok(())
                });
            })
            .await
        });
        let cancellation = started.await.unwrap();
        assert!(
            crate::shared::test_wait::until(async || jobs
                .report()
                .iter()
                .any(|job| job.name == "bootstrap-test"
                    && job.progress.as_deref() == Some("bootstrapping")))
            .await
        );
        if cancelled {
            cancellation.cancel();
        }
        for state in states {
            let partial = matches!(state, SyncState::PartiallySynced { .. });
            orchestrator.inner.set_state(state);
            if partial {
                tokio::time::sleep(Duration::from_millis(100)).await;
                assert_eq!(calls.load(SeqCst), 0, "no callback while partially synced");
            }
        }
        if !cancelled {
            assert!(
                crate::shared::test_wait::until(async || jobs.report().iter().any(|job| job.name
                    == "bootstrap-test"
                    && job
                        .progress
                        .as_deref()
                        .is_some_and(|value| value.starts_with("bootstrapped;"))))
                .await
            );
        }
        assert_eq!(
            jobs.report()
                .iter()
                .filter(|job| job.name == "after-bootstrap")
                .count(),
            expected
        );
        jobs.shutdown(Duration::from_secs(5)).await;
        assert_eq!(calls.load(SeqCst), expected);
        assert_eq!(terminated.load(SeqCst), expected);
        assert!(orchestrator.inner.is_stopped());
        let _ = std::fs::remove_dir_all(temp);
    }
}

#[test]
fn stream_backoff_resets_only_after_a_settled_poll_and_a_full_cycle() {
    let cfg = SyncOrchestratorConfig::default();
    let base = cfg.syncing_reconnect_time_ms as f64;
    let short = Duration::from_millis(10);
    let healthy = Duration::from_millis(cfg.pointer_changes_wait_time_ms);
    let grown = base * 1.2;

    let mut delay = base;
    for _ in 0..100 {
        delay = cfg.next_syncing_backoff(delay, true, true, short);
    }
    assert_eq!(
        delay, 3_600_000.0,
        "short failures grow up to the one-hour cap"
    );
    assert_eq!(cfg.next_syncing_backoff(base, true, true, short), grown);

    assert_eq!(
        cfg.next_syncing_backoff(delay, true, true, healthy),
        base,
        "a failure after a settled poll and a full cycle reconnects at the base delay"
    );
    assert_eq!(
        cfg.next_syncing_backoff(base, true, true, healthy - Duration::from_millis(1)),
        grown,
        "a run just short of a full cycle still grows the delay"
    );
    assert_eq!(cfg.next_syncing_backoff(delay, false, false, short), base);
}

#[test]
fn a_stalled_stream_run_grows_the_backoff_however_long_it_lasted() {
    let cfg = SyncOrchestratorConfig::default();
    let base = cfg.syncing_reconnect_time_ms as f64;
    // A peer that served a page, then hung for an hour before erroring: no poll settled.
    assert_eq!(
        cfg.next_syncing_backoff(base, true, false, Duration::from_secs(3_600)),
        base * 1.2
    );
}

#[test]
fn stream_backoff_needs_the_longer_of_the_poll_wait_and_the_base_delay() {
    let cfg = SyncOrchestratorConfig {
        pointer_changes_wait_time_ms: 1_000,
        syncing_reconnect_time_ms: 5_000,
        ..SyncOrchestratorConfig::default()
    };
    let base = cfg.syncing_reconnect_time_ms as f64;
    assert_eq!(
        cfg.next_syncing_backoff(base, true, true, Duration::from_millis(4_999)),
        base * 1.2
    );
    assert_eq!(
        cfg.next_syncing_backoff(base * 1.2, true, true, Duration::from_millis(5_000)),
        base
    );
}

/// An in-process peer: `/snapshots` answers `snapshots`, `/pointer-changes` the deltas at or
/// after `from`, `/contents/{id}` the entity files. While `down` is set, every answer is a 500.
struct TestPeer {
    url: String,
    http: tokio::task::JoinHandle<()>,
    down: Arc<AtomicBool>,
}

impl Drop for TestPeer {
    fn drop(&mut self) {
        self.http.abort();
    }
}

/// One signed entity per `(kind, timestamp)`, each on its own fresh pointer; returns the peer
/// and the entity ids in order.
async fn spawn_test_peer(
    entities: &[(&str, Timestamp)],
    snapshots: serde_json::Value,
    down: bool,
) -> (TestPeer, Vec<String>) {
    use axum::extract::{Path, Query, State};
    use axum::response::IntoResponse;
    use axum::routing::get;
    use axum::{Json, Router};
    use serde_json::json;

    let mut files = HashMap::new();
    let mut deltas = Vec::new();
    let mut ids = Vec::new();
    for (kind, timestamp) in entities {
        let pointer = format!("{kind}-{}", crate::sync_test_support::unique());
        let bytes = serde_json::to_vec(&json!({
            "version": "v3", "type": kind, "timestamp": timestamp,
            "pointers": [pointer], "content": [], "metadata": {}
        }))
        .unwrap();
        let id = crate::auth::hash_bytes_v1(&bytes);
        let chain = crate::sync_test_support::signed_chain(&id).await;
        files.insert(id.clone(), bytes);
        deltas.push(json!({
            "entityId": id, "entityType": kind, "entityTimestamp": timestamp,
            "localTimestamp": timestamp, "authChain": serde_json::to_value(chain).unwrap(),
            "pointers": [pointer]
        }));
        ids.push(id);
    }
    let down = Arc::new(AtomicBool::new(down));
    let app = {
        let down = down.clone();
        Router::new()
            .route("/snapshots", get(move || async move { Json(snapshots) }))
            .route(
                "/pointer-changes",
                get(move |Query(query): Query<HashMap<String, String>>| {
                    let deltas = deltas.clone();
                    async move {
                        let from = query["from"].parse::<i64>().unwrap();
                        let page: Vec<_> = deltas
                            .into_iter()
                            .filter(|d| d["localTimestamp"].as_i64().unwrap() >= from)
                            .collect();
                        Json(json!({"deltas": page, "pagination": {"moreData": false}}))
                    }
                }),
            )
            .route(
                "/contents/{id}",
                get(
                    |State(files): State<Arc<HashMap<String, Vec<u8>>>>,
                     Path(id): Path<String>| async move {
                        files
                            .get(&id)
                            .cloned()
                            .ok_or(axum::http::StatusCode::NOT_FOUND)
                    },
                ),
            )
            .with_state(Arc::new(files))
            .layer(axum::middleware::from_fn(
                move |request: axum::extract::Request, next: axum::middleware::Next| {
                    let down = down.load(SeqCst);
                    async move {
                        if down {
                            axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
                        } else {
                            next.run(request).await
                        }
                    }
                },
            ))
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let http = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (TestPeer { url, http, down }, ids)
}

/// An orchestrator over the test database, wired as the sync leader wires it.
async fn test_orchestrator(
    pool: &sqlx::PgPool,
    config: SyncOrchestratorConfig,
) -> (
    Arc<SyncOrchestrator>,
    Arc<LiveDeploymentRepository>,
    std::path::PathBuf,
) {
    use crate::batch_deployer::BatchDeployerConfig;
    use crate::bloom_filter::BloomFilter;
    use crate::failed_deployments_store::LiveFailedDeploymentsStore;
    use crate::sync_deployer::{LiveSyncDeployer, SyncValidation};

    let (storage, temp) = crate::sync_test_support::temp_storage("orchestrator").await;
    let status = Arc::new(SyncStatus::default());
    let repo = Arc::new(LiveDeploymentRepository::new(pool.clone(), status.clone()));
    let client = crate::sync::test_sync_client(reqwest::Client::new());
    let deployer = Arc::new(BatchDeployer::new(
        BatchDeployerConfig::default(),
        client.clone(),
        storage.clone(),
        pool.clone(),
        Arc::new(LiveSyncDeployer::new(
            pool.clone(),
            4,
            SyncValidation::AuthChainOnly,
        )),
        Arc::new(LiveFailedDeploymentsStore::new(pool.clone())),
        BloomFilter::with_capacity(1_000),
    ));
    let orchestrator = Arc::new(SyncOrchestrator::new(
        config,
        SyncDependencies {
            http_client: client,
            storage,
            deployer,
            processed_store: Arc::new(LiveProcessedSnapshotStore::new(pool.clone())),
            deployment_repo: repo.clone(),
            status,
            control: SyncControl::default(),
        },
    ));
    (orchestrator, repo, temp)
}

/// Fast retries, so a held peer is retried within a test's patience.
fn fast_config() -> SyncOrchestratorConfig {
    SyncOrchestratorConfig {
        request_max_retries: 1,
        request_retry_wait_ms: 10,
        pointer_changes_wait_time_ms: 100,
        bootstrap_reconnect_time_ms: 50,
        bootstrap_max_reconnect_ms: 200,
        syncing_reconnect_time_ms: 50,
        ..Default::default()
    }
}

async fn deployed(pool: &sqlx::PgPool, ids: &[String]) -> usize {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM content.deployments WHERE entity_id = ANY($1)",
    )
    .bind(ids)
    .fetch_one(pool)
    .await
    .unwrap() as usize
}

async fn wait_until(what: &str, mut done: impl AsyncFnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(20), async {
        while !done().await {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {what}"));
}

/// A peer added to a node whose global frontier is already at `now` has no cursor row and an
/// empty snapshot list: it starts from its earliest pointer change, so the entities it holds
/// hours before the frontier (far outside the 20-minute shift) are requested and deployed.
#[tokio::test]
async fn a_new_peer_starts_from_its_own_history_not_the_global_frontier() {
    let Some(pool) = crate::sync_test_support::test_pool(8).await else {
        return;
    };
    let now = chrono::Utc::now().timestamp_millis();
    let oldest = now - 3 * 3_600_000;
    let (peer, ids) = spawn_test_peer(
        &[("profile", oldest), ("scene", now - 2 * 3_600_000)],
        serde_json::json!([]),
        false,
    )
    .await;
    let (orchestrator, repo, temp) = test_orchestrator(&pool, fast_config()).await;
    repo.advance_sync_frontier(now).await.unwrap();
    assert_eq!(repo.get_server_sync_cursor(&peer.url).await.unwrap(), None);

    let _handle = orchestrator
        .sync_with_servers([peer.url.clone()].into_iter().collect())
        .await
        .unwrap();
    wait_until("the peer's entities older than the frontier", async || {
        deployed(&pool, &ids).await == ids.len()
    })
    .await;
    wait_until("the steady state", async || {
        orchestrator.state() == SyncState::Syncing
    })
    .await;
    orchestrator.stop().await;
    let cursor = repo.get_server_sync_cursor(&peer.url).await.unwrap();
    let _ = std::fs::remove_dir_all(temp);
    assert!(
        cursor.is_some_and(|c| c < now - POINTER_CHANGES_SHIFT_MS),
        "the peer's own cursor follows its history, not the frontier: {cursor:?}"
    );
}

/// One unreachable peer does not hold the node: once the bootstrap pass over both peers ends,
/// the node is `Syncing` (phased: through `PartiallySynced`) while the dead peer, which never
/// got a cursor, is retried in the background. A restart over the same database bootstraps
/// again, and the dead peer, reachable by then, is caught up from its own history although
/// the frontier has moved past it.
#[tokio::test]
async fn an_unreachable_peer_does_not_hold_the_node_in_bootstrap() {
    let Some(pool) = crate::sync_test_support::test_pool(8).await else {
        return;
    };
    let now = chrono::Utc::now().timestamp_millis();
    let (good, good_ids) = spawn_test_peer(
        &[("profile", now - 3_600_000), ("scene", now - 60_000)],
        serde_json::json!([]),
        false,
    )
    .await;
    let (dead, dead_ids) = spawn_test_peer(
        &[("scene", now - 4 * 3_600_000)],
        serde_json::json!([]),
        true,
    )
    .await;
    let peers: HashSet<String> = [good.url.clone(), dead.url.clone()].into_iter().collect();

    let (first, repo, temp) = test_orchestrator(&pool, fast_config()).await;
    let _handle = first.sync_with_servers(peers.clone()).await.unwrap();
    wait_until("the steady state with a dead peer", async || {
        first.state() == SyncState::Syncing
    })
    .await;
    let good_deployed = deployed(&pool, &good_ids).await;
    let dead_deployed = deployed(&pool, &dead_ids).await;
    let dead_cursor = repo.get_server_sync_cursor(&dead.url).await.unwrap();
    let good_cursor = repo.get_server_sync_cursor(&good.url).await.unwrap();
    first.stop().await;
    let _ = std::fs::remove_dir_all(&temp);
    assert_eq!(
        good_deployed,
        good_ids.len(),
        "the reachable peer bootstrapped"
    );
    assert_eq!(dead_deployed, 0);
    assert_eq!(dead_cursor, None, "a held peer's cursor is never published");
    assert!(good_cursor.is_some());

    dead.down.store(false, SeqCst);
    let (second, repo, temp) = test_orchestrator(&pool, fast_config()).await;
    assert_eq!(
        second.state(),
        SyncState::Bootstrapping,
        "a restart bootstraps again"
    );
    let _handle = second.sync_with_servers(peers).await.unwrap();
    wait_until("the recovered peer's old entity", async || {
        deployed(&pool, &dead_ids).await == dead_ids.len()
    })
    .await;
    wait_until("the steady state after the restart", async || {
        second.state() == SyncState::Syncing
    })
    .await;
    let good_cursor_after = repo.get_server_sync_cursor(&good.url).await.unwrap();
    second.stop().await;
    let _ = std::fs::remove_dir_all(temp);
    assert!(
        good_cursor_after >= good_cursor,
        "the reachable peer's cursor survived the restart"
    );
}
