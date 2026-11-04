//! The partial-upload protocol through the content router, on the process's
//! scratch database (each test signs with a fresh wallet on parcels of its
//! own), and the pure quota arithmetic.

use std::sync::Arc;

use alloy::signers::{Signer as _, local::PrivateKeySigner};
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Request, StatusCode, header};
use bytes::Bytes;
use serde_json::{Value, json};
use tower::ServiceExt;

use super::*;
use crate::tests::support::require_test_db;
use crate::write_deployer::WriteDeployer;
use crate::{ContentState, Settings, SharedState, SyncState, build_router};

struct Node {
    state: SharedState,
    tmp: std::path::PathBuf,
}

impl Node {
    /// A node taking local writes over the scratch database, with the test
    /// deployer (the chain and content checks, no chain sources).
    async fn new(tag: &str, limits: Limits) -> Option<Self> {
        let pool = crate::db::scratch::pool(4).await?;
        let tmp = std::env::temp_dir().join(format!(
            "catalyst-partial-{tag}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let storage = Arc::new(Storage::from_config(&tmp).await.unwrap());
        let settings = Settings {
            partial_uploads: limits,
            ..Settings::for_tests()
        };
        let state = Arc::new(ContentState::new(pool.clone(), settings, storage.clone()));
        state.sync.set_state(SyncState::Syncing);
        let deployer = WriteDeployer::with_optional_eip1654(pool, storage, None)
            .with_denylist(state.denylist.clone());
        assert!(state.attach_writes(true, Arc::new(deployer)));
        Some(Self { state, tmp })
    }

    fn router(&self) -> Router {
        build_router(self.state.clone())
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.tmp);
    }
}

/// A scene on fresh parcels with `files` as its content, signed by `signer`.
struct Scene {
    id: String,
    bytes: Bytes,
    chain: Value,
    files: Vec<(String, Bytes)>,
    pointers: Vec<String>,
}

async fn make_scene(signer: &PrivateKeySigner, files: &[&[u8]], entity_type: &str) -> Scene {
    let x = 100 + (rand_u16() % 40) as i32;
    let y = -140 - (rand_u16() % 10) as i32;
    let pointers = vec![format!("{x},{y}")];
    let files: Vec<(String, Bytes)> = files
        .iter()
        .map(|f| (hash_bytes_v1(f), Bytes::copy_from_slice(f)))
        .collect();
    let entity = json!({
        "version": "v3",
        "type": entity_type,
        "pointers": pointers,
        "timestamp": now_ms(),
        "content": files.iter().enumerate()
            .map(|(i, (hash, _))| json!({"file": format!("f{i}.bin"), "hash": hash}))
            .collect::<Vec<_>>(),
        "metadata": {"scene": {"base": pointers[0], "parcels": pointers}},
    });
    let bytes = Bytes::from(serde_json::to_vec(&entity).unwrap());
    let id = hash_bytes_v1(&bytes);
    let chain = chain_for(signer, &id).await;
    Scene {
        id,
        bytes,
        chain,
        files,
        pointers,
    }
}

fn rand_u16() -> u16 {
    uuid::Uuid::new_v4().as_bytes()[0] as u16 * 256 + uuid::Uuid::new_v4().as_bytes()[1] as u16
}

async fn chain_for(signer: &PrivateKeySigner, entity_id: &str) -> Value {
    let signature = signer
        .sign_message(entity_id.as_bytes())
        .await
        .unwrap()
        .to_string();
    json!([
        {"type": "SIGNER", "payload": format!("{:#x}", signer.address()), "signature": ""},
        {"type": "ECDSA_SIGNED_ENTITY", "payload": entity_id, "signature": signature}
    ])
}

/// A batch as the SDK sends it: `entityId`, `authChain`, `partial` (unless
/// `form_partial` is off) and the files named by their hash.
fn batch_request(
    query: &str,
    entity_id: &str,
    chain: &Value,
    files: &[(String, Bytes)],
    form_partial: bool,
) -> Request<Body> {
    let boundary = "catalyst-partial-boundary";
    let mut body: Vec<u8> = Vec::new();
    let mut field = |name: &str, value: &[u8], file: bool| {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        let disposition = if file {
            format!(
                "Content-Disposition: form-data; name=\"{name}\"; filename=\"{name}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
            )
        } else {
            format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n")
        };
        body.extend_from_slice(disposition.as_bytes());
        body.extend_from_slice(value);
        body.extend_from_slice(b"\r\n");
    };
    field("entityId", entity_id.as_bytes(), false);
    field("authChain", chain.to_string().as_bytes(), false);
    if form_partial {
        field("partial", b"true", false);
    }
    for (name, bytes) in files {
        field(name, bytes, true);
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    Request::post(format!("/content/entities{query}"))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .header("x-real-ip", "203.0.113.7")
        .body(Body::from(body))
        .unwrap()
}

async fn call(router: &Router, req: Request<Body>) -> (StatusCode, HeaderMap, Value) {
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    (
        status,
        headers,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn send(
    router: &Router,
    scene: &Scene,
    chain: &Value,
    files: Vec<(String, Bytes)>,
) -> (StatusCode, HeaderMap, Value) {
    call(
        router,
        batch_request("?partial=true", &scene.id, chain, &files, true),
    )
    .await
}

fn entity_file(scene: &Scene) -> (String, Bytes) {
    (scene.id.clone(), scene.bytes.clone())
}

/// ADR-325 end to end: the first batch (entity file and one content file)
/// answers 202 with the rest missing; the batch that completes it publishes
/// the scene (200), which `/entities/active` then serves; the upload's rows
/// are gone, and any later batch is a replay with the same timestamp.
#[tokio::test]
async fn a_scene_uploaded_in_batches_is_published_by_its_last_batch() {
    require_test_db!(Some(node) = Node::new("roundtrip", Limits::default()).await);
    let router = node.router();
    let signer = PrivateKeySigner::random();
    let scene = make_scene(&signer, &[b"partial-a", b"partial-b"], "scene").await;
    let (a, b) = (scene.files[0].clone(), scene.files[1].clone());

    let (status, _, body) = send(&router, &scene, &scene.chain, vec![entity_file(&scene), a]).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    assert_eq!(body, json!({ "missing": [b.0.clone()] }));

    let (status, _, body) = send(&router, &scene, &scene.chain, vec![b]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let created = body["creationTimestamp"].as_i64().expect("a timestamp");

    let req = Request::post("/content/entities/active")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"pointers": scene.pointers}).to_string()))
        .unwrap();
    let (_, _, active) = call(&router, req).await;
    assert_eq!(active[0]["id"], scene.id);
    let rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM content.pending_deployments WHERE entity_id = $1")
            .bind(&scene.id)
            .fetch_one(&node.state.pool)
            .await
            .unwrap();
    assert_eq!(rows, 0, "a published upload leaves no pending row");

    let (status, _, body) = send(&router, &scene, &scene.chain, vec![]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["creationTimestamp"], created,
        "a replay answers the stored timestamp"
    );
}

/// Content already in the store is never listed as missing: a scene whose
/// files are all stored is published by its first batch.
#[tokio::test]
async fn stored_content_is_never_missing() {
    require_test_db!(Some(node) = Node::new("stored", Limits::default()).await);
    let router = node.router();
    let signer = PrivateKeySigner::random();
    let scene = make_scene(&signer, &[b"partial-stored"], "scene").await;
    node.state
        .storage
        .store(&scene.files[0].0, scene.files[0].1.clone())
        .await
        .unwrap();
    let (status, _, body) = send(&router, &scene, &scene.chain, vec![entity_file(&scene)]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

/// The refusals a batch gets before it is stored, with upstream's messages.
#[tokio::test]
async fn a_batch_is_refused_with_upstreams_messages() {
    require_test_db!(Some(node) = Node::new("refusals", Limits::default()).await);
    let router = node.router();
    let signer = PrivateKeySigner::random();
    let scene = make_scene(&signer, &[b"partial-r1", b"partial-r2"], "scene").await;

    // A file whose field name is not its hash.
    let wrong = ("bafkreiaaaa".to_string(), Bytes::from_static(b"x"));
    let (status, _, body) = send(
        &router,
        &scene,
        &scene.chain,
        vec![entity_file(&scene), wrong],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body["errors"][0],
        format!(
            "The uploaded file 'bafkreiaaaa' does not match its content hash (computed {}).",
            hash_bytes_v1(b"x")
        )
    );

    // No entity file and no upload yet.
    let (status, _, body) = send(&router, &scene, &scene.chain, vec![scene.files[0].clone()]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body["errors"][0],
        format!(
            "The first partial request for an entity, and any request by another signer, must include the entity file '{}'.",
            scene.id
        )
    );

    // Started by the signer, joined by another account.
    let (status, _, _) = send(&router, &scene, &scene.chain, vec![entity_file(&scene)]).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let stranger = PrivateKeySigner::random();
    let stranger_chain = chain_for(&stranger, &scene.id).await;
    let (status, _, body) = send(&router, &scene, &stranger_chain, vec![entity_file(&scene)]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body["errors"][0],
        "This upload was started by another account."
    );
    let (status, _, _) = send(
        &router,
        &scene,
        &stranger_chain,
        vec![scene.files[0].clone()],
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "another signer must send the entity file"
    );

    // A bad signature.
    let other = PrivateKeySigner::random();
    let mut forged = chain_for(&other, &scene.id).await;
    forged[0]["payload"] = json!(format!("{:#x}", signer.address()));
    let (status, _, body) = send(&router, &scene, &forged, vec![entity_file(&scene)]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body["errors"][0]
            .as_str()
            .unwrap()
            .starts_with("The signature is invalid. "),
        "{body}"
    );

    // Only scenes.
    let profile = make_scene(&signer, &[], "profile").await;
    let (status, _, body) = send(
        &router,
        &profile,
        &profile.chain,
        vec![entity_file(&profile)],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body["errors"][0],
        "Partial deployments are only supported for scenes, but the entity type is 'profile'."
    );
}

/// `?partial=true` declares a batch before its body; a form without the
/// `partial` field is then refused rather than deployed past the quota.
#[tokio::test]
async fn the_query_flag_needs_the_form_field() {
    require_test_db!(Some(node) = Node::new("query-flag", Limits::default()).await);
    let router = node.router();
    let signer = PrivateKeySigner::random();
    let scene = make_scene(&signer, &[], "scene").await;
    let (status, _, body) = call(
        &router,
        batch_request(
            "?partial=true",
            &scene.id,
            &scene.chain,
            &[entity_file(&scene)],
            false,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body,
        json!({"error": "The 'partial=true' query parameter requires the 'partial=true' form field"})
    );
}

/// The per-account upload cap answers 429 with Retry-After.
#[tokio::test]
async fn an_account_over_its_upload_cap_waits() {
    let limits = Limits {
        max_per_deployer: 1,
        ..Limits::default()
    };
    require_test_db!(Some(node) = Node::new("account-cap", limits).await);
    let router = node.router();
    let signer = PrivateKeySigner::random();
    let first = make_scene(&signer, &[b"cap-a"], "scene").await;
    let (status, _, _) = send(&router, &first, &first.chain, vec![entity_file(&first)]).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let second = make_scene(&signer, &[b"cap-b"], "scene").await;
    let (status, headers, body) =
        send(&router, &second, &second.chain, vec![entity_file(&second)]).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        body["errors"][0],
        "Too many partial uploads in progress for this account (max 1). Complete an upload or wait for expired uploads to be cleaned up."
    );
    assert!(headers.contains_key(header::RETRY_AFTER));
}

/// An expired upload refuses its next batch; the cleanup job deletes it and
/// unlinks its staged blobs.
#[tokio::test]
async fn an_expired_upload_is_refused_and_reclaimed() {
    require_test_db!(Some(node) = Node::new("expiry", Limits::default()).await);
    let router = node.router();
    let signer = PrivateKeySigner::random();
    let scene = make_scene(&signer, &[b"expiry-a", b"expiry-b"], "scene").await;
    let (status, _, _) = send(
        &router,
        &scene,
        &scene.chain,
        vec![entity_file(&scene), scene.files[0].clone()],
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    sqlx::query(
        "UPDATE content.pending_deployments SET created_at = created_at - interval '2 hours'
         WHERE entity_id = $1",
    )
    .bind(&scene.id)
    .execute(&node.state.pool)
    .await
    .unwrap();
    let (status, _, body) = send(
        &router,
        &scene,
        &scene.chain,
        vec![entity_file(&scene), scene.files[1].clone()],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body["errors"][0],
        "This upload expired. Create a new entity with a fresh timestamp."
    );

    cleanup_expired(&node.state.pool, &node.state.storage, &Limits::default())
        .await
        .unwrap();
    let rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM content.pending_deployments WHERE entity_id = $1")
            .bind(&scene.id)
            .fetch_one(&node.state.pool)
            .await
            .unwrap();
    assert_eq!(rows, 0);
    assert!(!node.state.storage.exist(&scene.files[0].0).await.unwrap());
    assert!(!node.state.storage.exist(&scene.id).await.unwrap());
}

#[test]
fn ipfs_v2_ids_are_base32_cid_v1() {
    assert!(is_ipfs_v2(&hash_bytes_v1(b"x")));
    assert!(!is_ipfs_v2(
        "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG"
    ));
    assert!(!is_ipfs_v2("bafkrei"));
}

#[test]
fn the_query_declares_a_batch_only_with_true() {
    assert!(is_declared(Some("partial=true")));
    assert!(is_declared(Some("a=1&partial=true")));
    assert!(!is_declared(Some("partial=1")));
    assert!(!is_declared(None));
}

/// Upstream `checkPointerQuotas`: a full parcel first, then the account's
/// share of one (all but one upload, at least 1).
#[test]
fn pointer_quotas_name_the_full_parcels() {
    let usage = |uploads, deployer_uploads| PointerUsage {
        pointer: "1,1".into(),
        uploads,
        deployer_uploads,
        oldest: 1_000,
        deployer_oldest: Some(2_000),
    };
    let (message, retry) = check_pointer_quotas(&[usage(4, 0)], 4, 1_000, 60_000).unwrap();
    assert_eq!(
        message,
        "Too many partial uploads in progress for parcels 1,1 (max 4 per parcel). Complete an upload or wait for one to expire."
    );
    assert_eq!(retry, 60);
    let (message, retry) = check_pointer_quotas(&[usage(3, 3)], 4, 1_000, 60_000).unwrap();
    assert_eq!(
        message,
        "Too many partial uploads in progress for this account on parcels 1,1 (max 3 per parcel). Complete an upload or wait for one to expire."
    );
    assert_eq!(retry, 61);
    assert!(check_pointer_quotas(&[usage(2, 2)], 4, 1_000, 60_000).is_none());
}

/// Upstream `secondsUntilCleanupFreesQuota`.
#[test]
fn quota_retry_waits_for_the_cleanup_after_the_oldest_upload_expires() {
    // Before any run: from now, the first interval past the expiry.
    assert_eq!(
        seconds_until_cleanup_frees_quota(0, Some(0), 3_600_000, None, 300_000),
        3_600
    );
    // Already expired: the next run.
    assert_eq!(
        seconds_until_cleanup_frees_quota(4_000_000, Some(0), 3_600_000, Some(3_900_000), 300_000),
        200
    );
    assert_eq!(
        seconds_until_cleanup_frees_quota(10, None, 3_600_000, None, 300_000),
        300
    );
}
