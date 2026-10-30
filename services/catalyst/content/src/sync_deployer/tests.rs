use super::*;
use crate::sync_test_support::{signed_chain, signed_chain_behind_contract, test_pool, unique};
use crate::sync_types::AuthLinkType;

const ENTITY_BODY: &[u8] = br#"{"type":"scene","pointers":["0,0"],"timestamp":1,"content":[]}"#;

fn lazy_deployer() -> LiveSyncDeployer {
    LiveSyncDeployer {
        pool: sqlx::PgPool::connect_lazy("postgres://nobody@127.0.0.1:1/none").unwrap(),
        batch: Arc::new(Mutex::new(Vec::new())),
        flush_sem: Arc::new(Semaphore::new(1)),
        in_flight: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        idle_notify: Arc::new(tokio::sync::Notify::new()),
        lost: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        trusted_expirations: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        backlog: Arc::new(Semaphore::new(DEFAULT_FLUSH_BACKLOG_MAX)),
        flush_backlog_max: DEFAULT_FLUSH_BACKLOG_MAX,
        inflight_bytes: Arc::new(ByteBudget::new(DEFAULT_INFLIGHT_BYTES_MAX)),
    }
}

#[test]
fn a_flush_failure_record_carries_the_stamped_retry_state() {
    let entity = parse_entity_for_deploy(ENTITY_BODY, "bafytest", &Vec::new()).unwrap();
    let before = chrono::Utc::now().timestamp_millis();

    let fresh = flush_failure_record(&entity, Vec::new(), String::new(), (0, 0));
    assert_eq!(fresh.retry_count, 0);
    assert!(fresh.next_retry_at >= before);

    let record = flush_failure_record(&entity, Vec::new(), "boom".to_string(), (4, 1_800_000));
    assert_eq!(record.retry_count, 4);
    assert!(record.next_retry_at >= before + 1_800_000);
    assert!(record.next_retry_at <= chrono::Utc::now().timestamp_millis() + 1_800_000);
    assert_eq!(record.entity_id, "bafytest");
    assert_eq!(record.error_description, "boom");
}

#[test]
fn a_repeated_entity_in_one_batch_is_recorded_with_its_strongest_retry_state() {
    let weak = parse_entity_for_deploy(ENTITY_BODY, "bafydup", &Vec::new()).unwrap();
    let mut strong = parse_entity_for_deploy(ENTITY_BODY, "bafydup", &Vec::new()).unwrap();
    strong.retry_count = 4;
    strong.backoff_ms = 1_800_000;
    let entities = vec![weak, strong];

    let mut strongest = strongest_retry_state(&entities);
    assert_eq!(strongest.len(), 1);

    let before = chrono::Utc::now().timestamp_millis();
    let retry = strongest
        .remove(entities[0].entity_id.as_str())
        .expect("the first sighting takes the merged state");
    let record = flush_failure_record(&entities[0], Vec::new(), "boom".to_string(), retry);
    assert_eq!(record.retry_count, 4);
    assert!(record.next_retry_at >= before + 1_800_000);
    assert!(strongest.remove(entities[1].entity_id.as_str()).is_none());
}

#[tokio::test]
async fn a_buffered_entity_carries_the_retry_state_of_the_attempt_that_queued_it() {
    let deployer = lazy_deployer();
    let entity = ENTITY_BODY;

    deployer
        .deploy_entity(
            entity,
            "bafysync",
            &signed_chain("bafysync").await,
            DeploymentContext::Synced,
            None,
            None,
        )
        .await
        .unwrap();
    deployer
        .deploy_entity(
            entity,
            "bafyretry",
            &signed_chain("bafyretry").await,
            DeploymentContext::SyncedFix,
            None,
            Some((4, 1_800_000)),
        )
        .await
        .unwrap();

    let buffered = deployer.batch.lock().await;
    assert_eq!(buffered[0].retry_count, 0);
    assert_eq!(buffered[0].backoff_ms, 0);
    assert_eq!(buffered[1].retry_count, 4);
    assert_eq!(buffered[1].backoff_ms, 1_800_000);
}

#[tokio::test]
async fn an_entity_whose_auth_chain_does_not_verify_is_rejected_before_it_is_buffered() {
    let deployer = lazy_deployer();

    let err = deployer
        .deploy_entity(
            ENTITY_BODY,
            "bafyforged",
            &signed_chain("bafyother").await,
            DeploymentContext::Synced,
            None,
            None,
        )
        .await
        .expect_err("a chain signed for another entity is not this entity's");
    assert!(matches!(err, SyncError::DeploymentRejected(_)), "{err}");
    assert!(err.to_string().contains("bafyforged"), "{err}");

    let err = deployer
        .deploy_entity(
            ENTITY_BODY,
            "bafyempty",
            &Vec::new(),
            DeploymentContext::Synced,
            None,
            None,
        )
        .await
        .expect_err("an empty chain is malformed");
    assert!(matches!(err, SyncError::DeploymentRejected(_)), "{err}");

    let mut tampered = signed_chain("bafytampered").await;
    tampered[0].payload = "0x0000000000000000000000000000000000000001".to_string();
    let err = deployer
        .deploy_entity(
            ENTITY_BODY,
            "bafytampered",
            &tampered,
            DeploymentContext::Synced,
            None,
            None,
        )
        .await
        .expect_err("a signature that recovers to another signer is rejected");
    assert!(err.to_string().contains("Invalid signer address"), "{err}");

    let mut expired = signed_chain_behind_contract("bafyexpired", "1970-01-01T00:00:00.000Z").await;
    expired[1].signature = Some("0x00".into());
    let err = deployer
        .deploy_entity(
            ENTITY_BODY,
            "bafyexpired",
            &expired,
            DeploymentContext::Synced,
            None,
            None,
        )
        .await
        .expect_err("an ephemeral key that had expired when the entity was signed never signed it");
    assert!(matches!(err, SyncError::DeploymentRejected(_)), "{err}");
    assert!(err.to_string().contains("expired"), "{err}");

    let mut unsigned = signed_chain_behind_contract("bafynosig", "2099-01-01T00:00:00.000Z").await;
    unsigned[1].signature = None;
    let err = deployer
        .deploy_entity(
            ENTITY_BODY,
            "bafynosig",
            &unsigned,
            DeploymentContext::Synced,
            None,
            None,
        )
        .await
        .expect_err("a contract link still needs a signature");
    assert!(matches!(err, SyncError::DeploymentRejected(_)), "{err}");

    assert!(
        deployer.batch.lock().await.is_empty(),
        "nothing rejected is buffered"
    );
}

/// The contract's answer is the peer's: a synced chain with an ECDSA_EIP_1654 link is
/// accepted without asking any RPC, while the ECDSA links after it are still recovered.
#[tokio::test]
async fn a_contract_wallet_chain_is_trusted_from_the_peer_and_its_ecdsa_links_still_verify() {
    let deployer = lazy_deployer();

    let chain = signed_chain_behind_contract("bafycontract", "2099-01-01T00:00:00.000Z").await;
    assert_eq!(chain[1].link_type, AuthLinkType::EcdsaEip1654Ephemeral);
    deployer
        .deploy_entity(
            ENTITY_BODY,
            "bafycontract",
            &chain,
            DeploymentContext::Synced,
            None,
            None,
        )
        .await
        .expect("the peer validated the contract signature when it deployed the entity");

    let direct = vec![
        AuthLink {
            link_type: AuthLinkType::SIGNER,
            payload: "0x0000000000000000000000000000000000000002".into(),
            signature: None,
        },
        AuthLink {
            link_type: AuthLinkType::EcdsaEip1654SignedEntity,
            payload: "bafydirect".into(),
            signature: Some("0x00".into()),
        },
    ];
    deployer
        .deploy_entity(
            ENTITY_BODY,
            "bafydirect",
            &direct,
            DeploymentContext::Synced,
            None,
            None,
        )
        .await
        .expect("a contract that signed the entity itself");

    let mut forged =
        signed_chain_behind_contract("bafyforgedeph", "2099-01-01T00:00:00.000Z").await;
    forged[2] = signed_chain("bafyforgedeph").await[1].clone();
    forged[2].payload = "bafyforgedeph".into();
    let err = deployer
        .deploy_entity(
            ENTITY_BODY,
            "bafyforgedeph",
            &forged,
            DeploymentContext::Synced,
            None,
            None,
        )
        .await
        .expect_err("the entity signature must come from the ephemeral key the contract named");
    assert!(matches!(err, SyncError::DeploymentRejected(_)), "{err}");

    let buffered = deployer.batch.lock().await;
    let ids: Vec<&str> = buffered.iter().map(|e| e.entity_id.as_str()).collect();
    assert_eq!(ids, ["bafycontract", "bafydirect"]);
}

/// The 2020 web client wrote `Date.toString()` expirations; the peer read them with
/// `new Date` and has served those entities since. They are dates here too (still
/// checked against the entity timestamp), and an expiration nothing parses is the
/// peer's verdict: buffered and counted, never a retry row the worker evicts.
#[tokio::test]
async fn a_synced_expiration_the_peer_accepted_is_not_a_rejection() {
    let deployer = lazy_deployer();

    let js = signed_chain_behind_contract(
        "bafyjsdate",
        "Thu Feb 13 2020 15:31:10 GMT-0300 (Argentina Standard Time)",
    )
    .await;
    deployer
        .deploy_entity(
            ENTITY_BODY,
            "bafyjsdate",
            &js,
            DeploymentContext::Synced,
            None,
            None,
        )
        .await
        .expect("a Date.toString() expiration parses");
    assert_eq!(deployer.trusted_expiration_count(), 0);

    let epoch = signed_chain_behind_contract(
        "bafyjsexpired",
        "Thu Jan 01 1970 00:00:00 GMT+0000 (Coordinated Universal Time)",
    )
    .await;
    let err = deployer
        .deploy_entity(
            ENTITY_BODY,
            "bafyjsexpired",
            &epoch,
            DeploymentContext::Synced,
            None,
            None,
        )
        .await
        .expect_err(
            "a parsed Date.toString() expiration is still checked against the entity timestamp",
        );
    assert!(err.to_string().contains("expired"), "{err}");

    let unknown = signed_chain_behind_contract("bafyunknown", "next Tuesday").await;
    deployer
        .deploy_entity(
            ENTITY_BODY,
            "bafyunknown",
            &unknown,
            DeploymentContext::Synced,
            None,
            None,
        )
        .await
        .expect("the peer accepted this payload when it deployed the entity");
    assert_eq!(deployer.trusted_expiration_count(), 1);

    let buffered = deployer.batch.lock().await;
    let ids: Vec<&str> = buffered.iter().map(|e| e.entity_id.as_str()).collect();
    assert_eq!(ids, ["bafyjsdate", "bafyunknown"]);
}

use crate::sync_types::AuthLink;

#[test]
fn a_failure_reason_survives_the_column_round_trip() {
    for reason in [FailureReason::DeploymentError, FailureReason::NoEntity] {
        let label = reason_label(&reason);
        assert_eq!(reason_from_label(label), reason);
    }
    assert_eq!(reason_label(&FailureReason::NoEntity), "No entity");
    assert_eq!(
        reason_label(&FailureReason::DeploymentError),
        "Deployment error"
    );
}

async fn cleanup(pool: &PgPool, prefix: &str) {
    let like = format!("{prefix}%");
    for sql in [
        "DELETE FROM content.content_files WHERE deployment IN (SELECT id FROM content.deployments WHERE entity_id LIKE $1)",
        "DELETE FROM content.active_pointers WHERE pointer LIKE $1",
        "DELETE FROM content.deployments WHERE entity_id LIKE $1",
        "DELETE FROM content.failed_deployments WHERE entity_id LIKE $1",
    ] {
        sqlx::query(sql).bind(&like).execute(pool).await.unwrap();
    }
}

/// Two batches: the older entity lands first, then a newer one over a shared pointer
/// supersedes it (deleter_deployment, active pointer moved, orphaned pointer cleared), and a
/// re-sighting of the older one is a no-op.
/// The batch ticker is a named job of the shared lifecycle, not a
/// detached loop (review finding plan-docs-5): it is reported, it is
/// scoped to the sync job that owns the deployer (it ends when that job
/// is cancelled) and it stops on shutdown.
#[tokio::test]
async fn the_batch_ticker_is_a_job_of_the_shared_lifecycle() {
    let Some(pool) = test_pool(1).await else {
        return;
    };
    let jobs = std::sync::Arc::new(crate::jobs::Jobs::new());
    let deployer = LiveSyncDeployer::new(pool, 1);
    let parent = jobs.spawn("sync", {
        let jobs = jobs.clone();
        move |ctx| async move {
            deployer.spawn_flush_ticker(&jobs, &ctx);
            ctx.cancel.cancelled().await;
            Ok(())
        }
    });
    let mut names: Vec<String> = Vec::new();
    assert!(
        crate::shared::test_wait::until(async || {
            names = jobs.report().into_iter().map(|j| j.name).collect();
            names.sort();
            names == ["sync", "sync-batch-flush"]
        })
        .await,
        "ticker not reported: {names:?}"
    );
    parent.cancel();
    assert!(
        crate::shared::test_wait::until(async || {
            jobs.report()
                .into_iter()
                .all(|j| j.state != crate::jobs::JobState::Running)
        })
        .await,
        "the ticker outlived its parent"
    );
    let aborted = jobs.shutdown(std::time::Duration::from_secs(5)).await;
    assert!(
        aborted.is_empty(),
        "the ticker ends on cancellation, not by abort: {aborted:?}"
    );
}

#[tokio::test]
async fn flushed_batches_write_rows_and_resolve_pointer_succession() {
    let Some(pool) = test_pool(3).await else {
        return;
    };
    let prefix = unique();
    let deployer = LiveSyncDeployer::new(pool.clone(), 2);
    let p = |n: &str| format!("{prefix}:{n}");
    let (older, newer) = (format!("{prefix}-older"), format!("{prefix}-newer"));

    let older_body = serde_json::json!({
        "version": "v3", "type": "scene", "pointers": [p("0,0").to_uppercase(), p("1,0")],
        "timestamp": 1_000, "content": [{"file": "a.txt", "hash": "hA"}],
        "metadata": {"name": "older"}
    });
    let newer_body = serde_json::json!({
        "version": "v3", "type": "scene", "pointers": [p("1,0"), p("2,0")],
        "timestamp": 2_000, "content": [], "metadata": null
    });
    deployer
        .deploy_entity(
            &serde_json::to_vec(&older_body).unwrap(),
            &older,
            &signed_chain(&older).await,
            DeploymentContext::Synced,
            None,
            None,
        )
        .await
        .unwrap();
    deployer.flush().await.unwrap();
    deployer
        .deploy_entity(
            &serde_json::to_vec(&newer_body).unwrap(),
            &newer,
            &signed_chain(&newer).await,
            DeploymentContext::Synced,
            None,
            None,
        )
        .await
        .unwrap();
    deployer
        .deploy_entity(
            &serde_json::to_vec(&older_body).unwrap(),
            &older,
            &signed_chain(&older).await,
            DeploymentContext::Synced,
            None,
            None,
        )
        .await
        .unwrap();
    deployer.flush().await.unwrap();
    assert_eq!(deployer.lost_count(), 0);

    type Row = (String, Vec<String>, Option<i32>, Option<Value>, String);
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT entity_id, entity_pointers, deleter_deployment, entity_metadata, deployer_address
         FROM content.deployments WHERE entity_id = ANY($1) ORDER BY id",
    )
    .bind(vec![older.clone(), newer.clone()])
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].0, older);
    assert_eq!(
        rows[0].1,
        vec![p("0,0"), p("1,0")],
        "pointers are lowercased"
    );
    assert!(rows[0].2.is_some(), "the older entity is superseded");
    assert_eq!(rows[0].3, Some(serde_json::json!({"v": {"name": "older"}})));
    assert_eq!(rows[0].4, signed_chain(&older).await[0].payload);
    assert_eq!((rows[1].0.as_str(), rows[1].2), (newer.as_str(), None));

    assert!(is_entity_deployed(&pool, &older, 1_000).await.unwrap());
    assert!(!is_entity_deployed(&pool, &older, 1_001).await.unwrap());
    assert!(
        !is_entity_deployed(&pool, &format!("{prefix}-nope"), 0)
            .await
            .unwrap()
    );
    let nope = format!("{prefix}-nope");
    let batched = deployed_entity_ids(&pool, &[(&older, 1_000), (&newer, 2_001), (&nope, 0)])
        .await
        .unwrap();
    assert_eq!(batched, HashSet::from([older.clone()]));

    let active: Vec<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT pointer, entity_id, entity_type FROM content.active_pointers WHERE pointer LIKE $1 ORDER BY pointer",
    )
    .bind(format!("{prefix}%"))
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        active,
        vec![
            (p("1,0"), newer.clone(), Some("scene".into())),
            (p("2,0"), newer.clone(), Some("scene".into())),
        ]
    );
    let files: Vec<(String, String)> = sqlx::query_as(
        "SELECT key, content_hash FROM content.content_files
         WHERE deployment = (SELECT id FROM content.deployments WHERE entity_id = $1)",
    )
    .bind(&older)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(files, vec![("a.txt".into(), "hA".into())]);

    cleanup(&pool, &prefix).await;
    pool.close().await;
}

async fn buffer_scene(deployer: &LiveSyncDeployer, id: &str, timestamp: i64, pointer: &str) {
    buffer_scene_on(deployer, id, timestamp, &[pointer]).await;
}

async fn buffer_scene_on(deployer: &LiveSyncDeployer, id: &str, timestamp: i64, pointers: &[&str]) {
    let body = serde_json::json!({
        "version": "v3", "type": "scene", "pointers": pointers,
        "timestamp": timestamp, "content": [], "metadata": null
    });
    deployer
        .deploy_entity(
            &serde_json::to_vec(&body).unwrap(),
            id,
            &signed_chain(id).await,
            DeploymentContext::Synced,
            None,
            None,
        )
        .await
        .unwrap();
}

async fn active_on(pool: &PgPool, pointer: &str) -> Option<String> {
    sqlx::query_scalar("SELECT entity_id FROM content.active_pointers WHERE pointer = $1")
        .bind(pointer)
        .fetch_optional(pool)
        .await
        .unwrap()
}

async fn deleter_of(pool: &PgPool, id: &str) -> Option<String> {
    sqlx::query_scalar(
        "SELECT d.entity_id FROM content.deployments o
         JOIN content.deployments d ON d.id = o.deleter_deployment WHERE o.entity_id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

/// Upstream breaks an equal-timestamp tie by lower(entity_id), and so must every step of a
/// flush. "-Z" beats "-a" there while their bytes order them the other way: delivered in
/// one batch, the pair left the pointer with no active entity at all.
#[tokio::test]
async fn an_equal_timestamp_pair_settles_on_the_lowercase_winner_in_any_batching() {
    let Some(pool) = test_pool(3).await else {
        return;
    };
    let deployer = LiveSyncDeployer::new(pool.clone(), 2);
    let prefix = unique();
    let batchings: [&[&[&str]]; 4] = [
        &[&["Z", "a"]],
        &[&["a", "Z"]],
        &[&["Z"], &["a"]],
        &[&["a"], &["Z"]],
    ];
    for (n, batches) in batchings.into_iter().enumerate() {
        let pointer = format!("{prefix}:{n}");
        let id = |s: &str| format!("{prefix}-{n}-{s}");
        for batch in batches {
            for s in *batch {
                buffer_scene(&deployer, &id(s), 1_000, &pointer).await;
            }
            deployer.flush().await.unwrap();
        }
        assert_eq!(
            active_on(&pool, &pointer).await,
            Some(id("Z")),
            "{batches:?}"
        );
        assert_eq!(
            deleter_of(&pool, &id("a")).await,
            Some(id("Z")),
            "{batches:?}"
        );
        assert_eq!(deleter_of(&pool, &id("Z")).await, None, "{batches:?}");
    }
    cleanup(&pool, &prefix).await;
    pool.close().await;
}

/// The deleter is the nearest successor whatever the arrival order: A < B < C on one
/// pointer arriving A, C, B left A deleted by C, which an unpublish of C then reads.
#[tokio::test]
async fn the_deleter_is_the_nearest_successor_in_any_arrival_order() {
    let Some(pool) = test_pool(3).await else {
        return;
    };
    let deployer = LiveSyncDeployer::new(pool.clone(), 2);
    let prefix = unique();
    let orders = [
        ["A", "B", "C"],
        ["A", "C", "B"],
        ["C", "B", "A"],
        ["B", "A", "C"],
    ];
    for (n, order) in orders.into_iter().enumerate() {
        let pointer = format!("{prefix}:{n}");
        let id = |s: &str| format!("{prefix}-{n}-{s}");
        for s in order {
            let timestamp = match s {
                "A" => 1_000,
                "B" => 2_000,
                _ => 3_000,
            };
            buffer_scene(&deployer, &id(s), timestamp, &pointer).await;
            deployer.flush().await.unwrap();
        }
        assert_eq!(
            deleter_of(&pool, &id("A")).await,
            Some(id("B")),
            "{order:?}"
        );
        assert_eq!(
            deleter_of(&pool, &id("B")).await,
            Some(id("C")),
            "{order:?}"
        );
        assert_eq!(deleter_of(&pool, &id("C")).await, None, "{order:?}");
        assert_eq!(active_on(&pool, &pointer).await, Some(id("C")), "{order:?}");
    }
    cleanup(&pool, &prefix).await;
    pool.close().await;
}

/// An unpublished local scene is nobody's successor: a peer scene older than it that
/// arrives after the unpublish takes the pointer instead of being stored as replaced.
#[tokio::test]
async fn a_peer_scene_arriving_after_an_unpublish_is_not_superseded_by_the_tombstone() {
    let Some(pool) = test_pool(3).await else {
        return;
    };
    let deployer = LiveSyncDeployer::new(pool.clone(), 2);
    let prefix = unique();
    let pointer = format!("{prefix}:0,0");
    let (local, peer) = (format!("{prefix}-local"), format!("{prefix}-peer"));
    buffer_scene(&deployer, &local, 2_000, &pointer).await;
    deployer.flush().await.unwrap();
    sqlx::query("INSERT INTO content.local_entities (entity_id, signer) VALUES ($1, '0xabc')")
        .bind(&local)
        .execute(&pool)
        .await
        .unwrap();
    let out = crate::land_publish::tombstone_and_repoint(&pool, &pointer)
        .await
        .unwrap();
    assert_eq!(out.repointed, vec![(pointer.clone(), None)]);

    buffer_scene(&deployer, &peer, 1_000, &pointer).await;
    deployer.flush().await.unwrap();
    assert_eq!(deleter_of(&pool, &peer).await, None);
    assert_eq!(active_on(&pool, &pointer).await, Some(peer.clone()));

    sqlx::query("DELETE FROM content.local_entities WHERE entity_id = $1")
        .bind(&local)
        .execute(&pool)
        .await
        .unwrap();
    cleanup(&pool, &prefix).await;
    pool.close().await;
}

/// A flush takes its pointers' locks before it reads, one at a time in byte order as the
/// write path's functions do. With an unpublish of the scene it would name as deleter
/// still open on the byte-first pointer, the flush waits there holding nothing (in the
/// database collation's order it would hold the other one), and once the unpublish
/// commits the peer scene takes both pointers instead of keeping the tombstone as deleter.
#[tokio::test]
async fn a_flush_waits_in_byte_order_for_an_open_unpublish_of_its_successor() {
    let Some(pool) = test_pool(4).await else {
        return;
    };
    let deployer = LiveSyncDeployer::new(pool.clone(), 2);
    let prefix = unique();
    let (first, second) = (format!("{prefix}:0,-2"), format!("{prefix}:0,10"));
    assert!(first < second, "byte order");
    let (local, peer) = (format!("{prefix}-local"), format!("{prefix}-peer"));
    buffer_scene(&deployer, &local, 2_000, &first).await;
    deployer.flush().await.unwrap();
    sqlx::query("INSERT INTO content.local_entities (entity_id, signer) VALUES ($1, '0xabc')")
        .bind(&local)
        .execute(&pool)
        .await
        .unwrap();

    let mut unpublish = pool.begin().await.unwrap();
    let unpublish_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *unpublish)
        .await
        .unwrap();
    let repointed: Vec<(Option<String>, Option<String>)> =
        sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT pointer, replacement FROM content.{}($1)",
            crate::land_publish::unpublish_function_name()
        )))
        .bind(&first)
        .fetch_all(&mut *unpublish)
        .await
        .unwrap();
    assert_eq!(repointed, vec![(Some(first.clone()), None)]);

    buffer_scene_on(&deployer, &peer, 1_000, &[&second, &first]).await;
    let mut flush = tokio::spawn({
        let deployer = deployer.clone();
        async move { deployer.flush().await }
    });
    let waiting = {
        let pool = pool.clone();
        async move {
            loop {
                let waiter: Option<i32> = sqlx::query_scalar(
                    "SELECT pid FROM pg_locks WHERE locktype = 'advisory' AND NOT granted
                       AND $1 = ANY(pg_blocking_pids(pid)) LIMIT 1",
                )
                .bind(unpublish_pid)
                .fetch_optional(&pool)
                .await
                .unwrap();
                if let Some(pid) = waiter {
                    return pid;
                }
                tokio::task::yield_now().await;
            }
        }
    };
    let raced = tokio::time::timeout(std::time::Duration::from_secs(60), async {
        tokio::select! {
            done = &mut flush => Err(done),
            pid = waiting => Ok(pid),
        }
    })
    .await
    .expect("hang guard: the flush neither finished nor waited on the unpublish");
    if let Ok(flush_pid) = raced {
        let held: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_locks
             WHERE pid = $1 AND locktype = 'advisory' AND granted",
        )
        .bind(flush_pid)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            held, 0,
            "the flush waits on the byte-first pointer holding nothing"
        );
    }
    unpublish.commit().await.unwrap();
    match raced {
        Ok(_) => flush.await.unwrap().unwrap(),
        Err(done) => done.unwrap().unwrap(),
    }

    assert_eq!(deleter_of(&pool, &peer).await, None);
    assert_eq!(active_on(&pool, &first).await, Some(peer.clone()));
    assert_eq!(active_on(&pool, &second).await, Some(peer.clone()));

    sqlx::query("DELETE FROM content.local_entities WHERE entity_id = $1")
        .bind(&local)
        .execute(&pool)
        .await
        .unwrap();
    cleanup(&pool, &prefix).await;
    pool.close().await;
}

#[tokio::test]
async fn failed_deployments_round_trip_and_never_rewind_retry_state() {
    let Some(pool) = test_pool(2).await else {
        return;
    };
    let prefix = unique();
    let store = LiveFailedDeploymentsStore::new(pool.clone());
    let id = format!("{prefix}-failed");
    let chain = signed_chain(&id).await;
    let record = |retry_count: u32, next_retry_at: i64, description: &str| FailedDeployment {
        entity_type: "scene".into(),
        entity_id: id.clone(),
        reason: FailureReason::DeploymentError,
        auth_chain: chain.clone(),
        error_description: description.into(),
        failure_timestamp: 1_700_000_000_000,
        snapshot_hash: Some("bafysnap".into()),
        retry_count,
        next_retry_at,
    };

    store
        .report_failure(record(3, 1_700_000_900_000, "third attempt"))
        .await
        .unwrap();
    store
        .report_failure(record(0, 0, "seen again in a snapshot"))
        .await
        .unwrap();
    let mine: Vec<FailedDeployment> = store
        .get_all_failed()
        .await
        .unwrap()
        .into_iter()
        .filter(|f| f.entity_id == id)
        .collect();
    assert_eq!(mine.len(), 1);
    assert_eq!(mine[0].error_description, "seen again in a snapshot");
    assert_eq!(
        (mine[0].retry_count, mine[0].next_retry_at),
        (3, 1_700_000_900_000)
    );
    assert_eq!(mine[0].failure_timestamp, 1_700_000_000_000);
    assert_eq!(mine[0].snapshot_hash.as_deref(), Some("bafysnap"));
    assert_eq!(mine[0].reason, FailureReason::DeploymentError);
    assert_eq!(mine[0].auth_chain, chain);

    assert_eq!(
        store
            .remove_exhausted(std::slice::from_ref(&id), 4)
            .await
            .unwrap(),
        Vec::<String>::new()
    );
    assert_eq!(
        store
            .remove_exhausted(std::slice::from_ref(&id), 3)
            .await
            .unwrap(),
        vec![id.clone()]
    );
    assert!(
        !store
            .get_all_failed()
            .await
            .unwrap()
            .iter()
            .any(|f| f.entity_id == id)
    );

    cleanup(&pool, &prefix).await;
    pool.close().await;
}
