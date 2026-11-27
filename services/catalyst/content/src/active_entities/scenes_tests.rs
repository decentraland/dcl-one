use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use tower::ServiceExt;

use super::mask_tests::{seed, unseed};
use crate::SyncState;
use crate::tests::support::{TestNode, require_test_db};

async fn get(node: &TestNode, uri: &str) -> (StatusCode, Option<String>, Value) {
    let (status, headers, body) = get_if_none_match(node, uri, None).await;
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap().to_string());
    (
        status,
        content_type,
        serde_json::from_slice(&body).unwrap_or(Value::Null),
    )
}

async fn get_if_none_match(
    node: &TestNode,
    uri: &str,
    if_none_match: Option<&str>,
) -> (StatusCode, header::HeaderMap, bytes::Bytes) {
    let mut request = Request::get(uri);
    if let Some(tag) = if_none_match {
        request = request.header(header::IF_NONE_MATCH, tag);
    }
    let response = node
        .router()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (parts, body) = response.into_parts();
    let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    (parts.status, parts.headers, body)
}

fn etag_of(headers: &header::HeaderMap) -> String {
    headers[header::ETAG].to_str().unwrap().to_string()
}

/// The entries `tag` seeded, out of a database other tests share.
fn seeded(body: &Value, tag: &str) -> Vec<Value> {
    let prefix = format!("test-{tag}-");
    body.as_array()
        .expect("an array")
        .iter()
        .filter(|e| e["id"].as_str().is_some_and(|id| id.starts_with(&prefix)))
        .cloned()
        .collect()
}

#[tokio::test]
async fn active_scenes_lists_every_active_scene_with_its_parcels_and_base() {
    require_test_db!(Some(node) = TestNode::with_db("activescenes").await);
    node.state.sync.set_state(SyncState::Syncing);
    let pool = node.state.pool.clone();
    let tag = "activescenes";
    unseed(&pool, tag).await;

    let based = seed(
        &pool,
        tag,
        1,
        "scene",
        &["7640,7640", "7640,7641", "7641,7640"],
        json!({ "scene": { "base": "7640,7640", "parcels": ["7640,7640", "7640,7641", "7641,7640"] } }),
        &["scene.json"],
    )
    .await;
    let baseless = seed(&pool, tag, 2, "scene", &["7645,7645"], json!({}), &[]).await;
    seed(&pool, tag, 3, "profile", &["0x7640"], json!({}), &[]).await;
    let replaced = seed(
        &pool,
        tag,
        4,
        "scene",
        &["7650,7650"],
        json!({ "scene": { "base": "7650,7650" } }),
        &[],
    )
    .await;
    sqlx::query("UPDATE content.deployments SET deleter_deployment = id WHERE entity_id = $1")
        .bind(&replaced.id)
        .execute(&pool)
        .await
        .unwrap();
    let denied = seed(&pool, tag, 5, "scene", &["7655,7655"], json!({}), &[]).await;
    node.state.denylist.add(&denied.id);

    for uri in [
        "/content/entities/active/scenes",
        "/entities/active/scenes",
        "/content/Entities/Active/Scenes",
    ] {
        let (status, content_type, body) = get(&node, uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert_eq!(content_type.as_deref(), Some("application/json"), "{uri}");
        assert_eq!(
            seeded(&body, tag),
            vec![
                json!({
                    "id": based.id,
                    "pointers": based.pointers,
                    "timestamp": 1_700_000_001_000i64,
                    "base": "7640,7640",
                }),
                json!({
                    "id": baseless.id,
                    "pointers": baseless.pointers,
                    "timestamp": 1_700_000_002_000i64,
                    "base": null,
                }),
            ],
            "{uri}"
        );
    }

    // The tag names the body: a request naming it is answered 304 with no body,
    // weak, in a list, or `*` alike, and one naming another tag gets the list.
    let uri = "/content/entities/active/scenes";
    let (status, headers, body) = get_if_none_match(&node, uri, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CACHE_CONTROL], "no-cache");
    let first = etag_of(&headers);
    assert_eq!(first.len(), 66, "{first}");
    assert!(first.starts_with('"') && first.ends_with('"'), "{first}");
    for if_none_match in [
        first.clone(),
        format!("W/{first}"),
        format!("\"other\", {first}"),
        "*".to_string(),
    ] {
        let (status, headers, body) = get_if_none_match(&node, uri, Some(&if_none_match)).await;
        assert_eq!(status, StatusCode::NOT_MODIFIED, "{if_none_match}");
        assert_eq!(etag_of(&headers), first, "{if_none_match}");
        assert_eq!(headers[header::CACHE_CONTROL], "no-cache");
        assert!(body.is_empty(), "{if_none_match}");
    }
    let (status, _, again) = get_if_none_match(&node, uri, Some("\"other\"")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again, body);

    // Memoized: a scene deployed after the first answer shows once the memo
    // is cleared, as a commit clears it, under a new tag.
    let later = seed(&pool, tag, 6, "scene", &["7660,7660"], json!({}), &[]).await;
    let (status, _, _) = get_if_none_match(&node, uri, Some(&first)).await;
    assert_eq!(status, StatusCode::NOT_MODIFIED);
    node.state.deployments_cache.clear();
    let (status, headers, body) = get_if_none_match(&node, uri, Some(&first)).await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(etag_of(&headers), first);
    let body: Value = serde_json::from_slice(&body).unwrap();
    let ids: Vec<Value> = seeded(&body, tag).iter().map(|e| e["id"].clone()).collect();
    assert_eq!(
        ids,
        vec![json!(based.id), json!(baseless.id), json!(later.id)]
    );

    unseed(&pool, tag).await;
}

#[tokio::test]
async fn active_scenes_is_unavailable_on_a_node_that_may_miss_scenes() {
    let bootstrapping = TestNode::without_db("activescenes-bootstrapping").await;

    let partial_sync = TestNode::without_db("activescenes-partial-sync").await;
    partial_sync
        .state
        .sync
        .set_state(SyncState::PartiallySynced {
            ready_types: vec!["wearable".to_string()],
        });

    let unsynced_copy = TestNode::without_db("activescenes-unsynced-copy").await;
    unsynced_copy.state.sync.set_state(SyncState::Syncing);
    unsynced_copy.state.sync.set_unsynced_copy(true);

    let mut settings = crate::Settings::for_tests();
    settings.syncs_scenes = false;
    let other_types_only =
        TestNode::without_db_settings("activescenes-other-types-only", settings).await;
    other_types_only.state.sync.set_state(SyncState::Syncing);

    for (case, node) in [
        ("bootstrapping", &bootstrapping),
        ("scenes not synced yet", &partial_sync),
        ("unsynced copy of a peer", &unsynced_copy),
        ("syncs other types only", &other_types_only),
    ] {
        let (status, headers, _) =
            get_if_none_match(node, "/content/entities/active/scenes", None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{case}");
        assert_eq!(headers[header::CACHE_CONTROL], "no-store", "{case}");
        assert!(headers.get(header::ETAG).is_none(), "{case}");
    }
}

#[tokio::test]
async fn active_scenes_answers_get_only() {
    let node = TestNode::without_db("activescenes405").await;
    let response = node
        .router()
        .oneshot(
            Request::post("/content/entities/active/scenes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(response.headers().get(header::ALLOW).unwrap(), "HEAD, GET");
}
