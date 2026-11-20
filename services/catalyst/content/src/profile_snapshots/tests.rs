use std::collections::HashMap;

use serde_json::json;

use super::*;
use crate::auth::{hash_bytes, hash_bytes_v1};
use crate::sync_test_support::{blob_peer, temp_storage, test_pool, unique};

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
    assert!(first.fetched >= 1 && first.unavailable >= 1, "{first:?}");
    assert!(storage.exist(&served_hash).await.unwrap());
    let expected = vec![
        ("body.png".to_string(), served_hash),
        ("face.png".to_string(), held_hash),
        ("face256.png".to_string(), listed_hash),
    ];
    assert_eq!(rows(&pool, active).await, expected);
    assert!(rows(&pool, superseded).await.is_empty());

    repair(&pool, &storage, &client, std::slice::from_ref(&url), 100)
        .await
        .unwrap();
    assert_eq!(rows(&pool, active).await, expected);
    server.abort();
    let _ = std::fs::remove_dir_all(tmp);
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
    let client = crate::sync::test_sync_client(reqwest::Client::new());

    repair(&pool, &storage, &client, std::slice::from_ref(&url), 10_000)
        .await
        .unwrap();
    assert!(
        storage.exist(&render_hash).await.unwrap(),
        "the render was fetched"
    );
    assert!(rows(&pool, profile).await.is_empty());
    server.abort();
    let _ = std::fs::remove_dir_all(tmp);
}
