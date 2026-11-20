use std::collections::HashMap;

use serde_json::json;

use super::*;
use crate::auth::{hash_bytes, hash_bytes_v1};
use crate::sync_test_support::{RemoveOnDrop, blob_peer, temp_storage, unique};

/// A database of the test's own: a repair pass reads every active profile, so on the shared
/// one its counts would include other tests' rows.
async fn test_pool(max_connections: u32) -> Option<PgPool> {
    crate::db::scratch::isolated_pool(max_connections).await
}

fn cid(name: &str) -> String {
    hash_bytes_v1(name.as_bytes())
}

#[test]
fn snapshot_files_keep_the_legacy_keys_and_skip_what_cannot_be_a_row() {
    let (listed, face, body, other_face) = (cid("listed"), cid("face"), cid("body"), cid("face2"));
    let avatars = json!([
        {"avatar": {"snapshots": {
            "face256": listed,
            "face": format!("https://peer.example/content/contents/{face}"),
            "body": body,
            "face128": "not-a-cid",
            // Only the render names are read, so a key stays short.
            ("x".repeat(3000)): cid("long name"),
            "extra": cid("extra"),
        }}},
        {"avatar": {"snapshots": {"face128": 7}}},
        // A second avatar's `face` would reuse `face.png`; the first one keeps the key.
        {"avatar": {"snapshots": {"face": other_face}}},
        {"avatar": {"snapshots": "a string is not snapshots"}},
    ]);
    let content = vec![("face256.png".to_string(), listed)];
    let mut files = snapshot_files(Some(&avatars), &content);
    files.sort();
    assert_eq!(
        files,
        vec![
            ("body.png".to_string(), body),
            ("face.png".to_string(), face)
        ]
    );
    // A key `content[]` holds under another hash is left to it.
    let taken = vec![("body.png".to_string(), cid("old body"))];
    assert!(
        !snapshot_files(
            Some(&json!([{"avatar": {"snapshots": {"body": cid("body")}}}])),
            &taken
        )
        .iter()
        .any(|(key, _)| key == "body.png")
    );
    assert!(snapshot_files(None, &[]).is_empty());

    // Older profiles name CIDv0 renders, bare or as a URL.
    let (v0_face, v0_body) = (hash_bytes(b"v0 face"), hash_bytes(b"v0 body"));
    assert!(v0_face.starts_with("Qm"));
    let avatars = json!([{"avatar": {"snapshots": {
        "face": format!("https://peer.example/content/contents/{v0_face}"),
        "body": v0_body,
    }}}]);
    let mut files = snapshot_files(Some(&avatars), &[]);
    files.sort();
    assert_eq!(
        files,
        vec![
            ("body.png".to_string(), v0_body),
            ("face.png".to_string(), v0_face)
        ]
    );
}

async fn insert_profile(pool: &PgPool, entity_id: &str, avatars: Value) -> i32 {
    sqlx::query_scalar(
        "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, \
         entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain) \
         VALUES ('0x0', 'v3', 'profile', $1, $2, now(), ARRAY[$1], now(), '[]') RETURNING id",
    )
    .bind(entity_id)
    .bind(json!({"v": {"avatars": avatars}}))
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn rows(pool: &PgPool, deployment: i32) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> =
        sqlx::query_as("SELECT key, content_hash FROM content.content_files WHERE deployment = $1")
            .bind(deployment)
            .fetch_all(pool)
            .await
            .unwrap();
    rows.sort();
    rows
}

/// Rows for the renders the store holds or a peer serves; none for one nobody has, for a
/// superseded profile or twice.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repair_records_the_stored_renders_and_is_idempotent() {
    let Some(pool) = test_pool(2).await else {
        return;
    };
    let tag = unique();
    let blob = |name: &str| format!("{name} {tag}").into_bytes();
    let (held, served, missing, listed) = (
        blob("held"),
        blob("served"),
        blob("missing"),
        blob("listed"),
    );
    let [held_hash, served_hash, missing_hash, listed_hash] =
        [&held, &served, &missing, &listed].map(|b| hash_bytes_v1(b));
    let (storage, tmp) = temp_storage("snapshot-repair").await;
    let _tmp = RemoveOnDrop(tmp);
    storage.store(&held_hash, held.into()).await.unwrap();
    let (url, server) = blob_peer(HashMap::from([(served_hash.clone(), served)])).await;
    let client = crate::sync::test_sync_client(reqwest::Client::new());

    let snapshots = json!([{"avatar": {"snapshots": {
        "face256": listed_hash, "face": held_hash, "body": served_hash, "face128": missing_hash,
    }}}]);
    let active = insert_profile(&pool, &format!("active-{tag}"), snapshots.clone()).await;
    sqlx::query(
        "INSERT INTO content.content_files (deployment, key, content_hash) \
         VALUES ($1, 'face256.png', $2)",
    )
    .bind(active)
    .bind(&listed_hash)
    .execute(&pool)
    .await
    .unwrap();
    let superseded = insert_profile(&pool, &format!("superseded-{tag}"), snapshots).await;
    sqlx::query("UPDATE content.deployments SET deleter_deployment = $1 WHERE id = $2")
        .bind(active)
        .bind(superseded)
        .execute(&pool)
        .await
        .unwrap();

    let first = repair(&pool, &storage, &client, std::slice::from_ref(&url), 1)
        .await
        .unwrap();
    assert_eq!(
        first,
        RepairReport {
            profiles: 1,
            rows: 2,
            fetched: 1,
            unavailable: 1
        }
    );
    assert!(storage.exist(&served_hash).await.unwrap());
    let expected = vec![
        ("body.png".to_string(), served_hash),
        ("face.png".to_string(), held_hash),
        ("face256.png".to_string(), listed_hash),
    ];
    assert_eq!(rows(&pool, active).await, expected);
    assert!(rows(&pool, superseded).await.is_empty());

    let second = repair(&pool, &storage, &client, std::slice::from_ref(&url), 100)
        .await
        .unwrap();
    assert_eq!(
        second,
        RepairReport {
            profiles: 1,
            rows: 0,
            fetched: 0,
            unavailable: 1
        }
    );
    assert_eq!(rows(&pool, active).await, expected);
    server.abort();
}

/// A profile superseded while its renders are fetched gets no row: the insert re-checks that
/// the deployment is still active.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repair_adds_no_row_to_a_profile_superseded_during_the_pass() {
    let Some(pool) = test_pool(2).await else {
        return;
    };
    let tag = unique();
    let render = format!("raced {tag}").into_bytes();
    let render_hash = hash_bytes_v1(&render);
    let profile = insert_profile(
        &pool,
        &format!("raced-{tag}"),
        json!([{"avatar": {"snapshots": {"body": render_hash}}}]),
    )
    .await;
    let newer = insert_profile(&pool, &format!("newer-{tag}"), json!([])).await;

    // The peer supersedes the profile before it answers: after the scan, before the insert.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (peer_pool, raced) = (pool.clone(), render_hash.clone());
    let app = axum::Router::new().route(
        "/contents/{hash}",
        axum::routing::get(
            move |axum::extract::Path(hash): axum::extract::Path<String>| {
                let (pool, render, raced) = (peer_pool.clone(), render.clone(), raced.clone());
                async move {
                    use axum::response::IntoResponse;
                    if hash != raced {
                        return axum::http::StatusCode::NOT_FOUND.into_response();
                    }
                    sqlx::query(
                        "UPDATE content.deployments SET deleter_deployment = $1 WHERE id = $2",
                    )
                    .bind(newer)
                    .bind(profile)
                    .execute(&pool)
                    .await
                    .unwrap();
                    render.into_response()
                }
            },
        ),
    );
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let (storage, tmp) = temp_storage("snapshot-repair-race").await;
    let _tmp = RemoveOnDrop(tmp);
    let client = crate::sync::test_sync_client(reqwest::Client::new());

    let report = repair(&pool, &storage, &client, std::slice::from_ref(&url), 10_000)
        .await
        .unwrap();
    assert_eq!(
        report,
        RepairReport {
            profiles: 2,
            rows: 0,
            fetched: 1,
            unavailable: 0
        }
    );
    assert!(
        storage.exist(&render_hash).await.unwrap(),
        "the render was fetched"
    );
    assert!(rows(&pool, profile).await.is_empty());
    server.abort();
}

/// A store that cannot keep a render a peer served ends the pass with an error, instead of
/// counting the render as unavailable.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_store_fault_ends_the_repair() {
    let Some(pool) = test_pool(2).await else {
        return;
    };
    let tag = unique();
    let render = format!("unstorable {tag}").into_bytes();
    let render_hash = hash_bytes_v1(&render);
    insert_profile(
        &pool,
        &format!("unstorable-{tag}"),
        json!([{"avatar": {"snapshots": {"body": render_hash}}}]),
    )
    .await;
    let (storage, tmp) = temp_storage("snapshot-repair-fault").await;
    let _tmp = RemoveOnDrop(tmp.clone());
    // A dangling link where the render's shard belongs: a read finds it absent, a write cannot
    // create the shard (even as root).
    let shard = storage
        .root()
        .join(crate::storage::hex_prefix(&render_hash));
    std::os::unix::fs::symlink(tmp.join("nowhere"), &shard).unwrap();
    let (url, server) = blob_peer(HashMap::from([(render_hash, render)])).await;
    let client = crate::sync::test_sync_client(reqwest::Client::new());

    let result = repair(&pool, &storage, &client, std::slice::from_ref(&url), 10_000).await;
    assert!(result.is_err(), "{result:?}");
    server.abort();
}

/// With no servers, a render the store lacks is counted unavailable and one it holds still gets
/// its row.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repair_without_servers_records_only_the_held_renders() {
    let Some(pool) = test_pool(2).await else {
        return;
    };
    let tag = unique();
    let held = format!("held {tag}").into_bytes();
    let [held_hash, absent_hash] =
        [held.clone(), format!("absent {tag}").into_bytes()].map(|b| hash_bytes_v1(&b));
    let profile = insert_profile(
        &pool,
        &format!("serverless-{tag}"),
        json!([{"avatar": {"snapshots": {"face": held_hash, "body": absent_hash}}}]),
    )
    .await;
    let (storage, tmp) = temp_storage("snapshot-repair-serverless").await;
    let _tmp = RemoveOnDrop(tmp);
    storage.store(&held_hash, held.into()).await.unwrap();
    let client = crate::sync::test_sync_client(reqwest::Client::new());

    let report = repair(&pool, &storage, &client, &[], 100).await.unwrap();
    assert_eq!(
        report,
        RepairReport {
            profiles: 1,
            rows: 1,
            fetched: 0,
            unavailable: 1
        }
    );
    assert_eq!(
        rows(&pool, profile).await,
        vec![("face.png".to_string(), held_hash)]
    );
}

/// A history prune that deletes a profile while the repair inserts its rows leaves no row
/// behind: the insert waits on the deployment's row lock and then finds it gone.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repair_leaves_no_row_on_a_deployment_deleted_concurrently() {
    let Some(pool) = test_pool(4).await else {
        return;
    };
    let tag = unique();
    let held = format!("pruned {tag}").into_bytes();
    let held_hash = hash_bytes_v1(&held);
    let profile = insert_profile(
        &pool,
        &format!("pruned-{tag}"),
        json!([{"avatar": {"snapshots": {"body": held_hash}}}]),
    )
    .await;
    let (storage, tmp) = temp_storage("snapshot-repair-prune").await;
    let _tmp = RemoveOnDrop(tmp);
    storage.store(&held_hash, held.into()).await.unwrap();

    // The prune's transaction: its rows deleted, not yet committed.
    let mut prune = pool.begin().await.unwrap();
    sqlx::query("DELETE FROM content.content_files WHERE deployment = $1")
        .bind(profile)
        .execute(&mut *prune)
        .await
        .unwrap();
    sqlx::query("DELETE FROM content.deployments WHERE id = $1")
        .bind(profile)
        .execute(&mut *prune)
        .await
        .unwrap();

    let client = crate::sync::test_sync_client(reqwest::Client::new());
    let pass = repair(&pool, &storage, &client, &[], 100);
    // Commits the prune once the pass waits on a lock (or after 10 s, when it never does).
    let release = async {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            let waiting: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM pg_stat_activity \
                 WHERE datname = current_database() AND wait_event_type = 'Lock'",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            if waiting > 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        prune.commit().await.unwrap();
    };
    let (report, ()) = tokio::join!(pass, release);
    let report = report.unwrap();
    assert_eq!(report.rows, 0, "{report:?}");
    let orphans: i64 =
        sqlx::query_scalar("SELECT count(*) FROM content.content_files WHERE deployment = $1")
            .bind(profile)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(orphans, 0);
}
