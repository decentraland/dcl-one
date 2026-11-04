//! Partial (multi-request) publishes against the real schema: worlds-content-server's
//! `docs/partial-deployment-contract.md` (202 `{missing}`, completion, replay, expiry,
//! staging budgets) and the blob collector keeping a pending upload's files.

use axum::http::HeaderMap;

use super::super::partial::{self, PartialUploadLimits};
use super::*;

/// A scene with `files` content files, each `size` bytes.
struct Upload {
    entity_bytes: Vec<u8>,
    entity_id: String,
    files: Vec<(String, Vec<u8>)>,
}

fn upload_of(world: &WorldName, parcels: &[&str], files: usize, size: usize) -> Upload {
    let tag = Uuid::new_v4().simple().to_string();
    let files: Vec<(String, Vec<u8>)> = (0..files)
        .map(|i| {
            let mut bytes = format!("{tag}-{i}-").into_bytes();
            bytes.resize(size.max(bytes.len()), b'x');
            (hash_bytes_v1(&bytes), bytes)
        })
        .collect();
    let content: Vec<Value> = files
        .iter()
        .enumerate()
        .map(|(i, (hash, _))| json!({ "file": format!("f{i}.bin"), "hash": hash }))
        .collect();
    let entity = publish_entity(world, parcels, Value::Array(content));
    let entity_bytes = serde_json::to_vec(&entity).unwrap();
    Upload {
        entity_id: hash_bytes_v1(&entity_bytes),
        entity_bytes,
        files,
    }
}

fn upload(world: &WorldName, files: usize) -> Upload {
    upload_of(world, &["0,0"], files, 64)
}

/// One `POST /entities` batch of `u`.
struct Batch<'a> {
    signer: &'a Wallet,
    /// Signs this id instead of the entity's.
    chain_over: Option<String>,
    manifest: bool,
    files: Vec<usize>,
    partial_field: bool,
    query: &'static str,
    /// Sends the first listed file twice.
    repeat: bool,
}

impl<'a> Batch<'a> {
    fn of(signer: &'a Wallet, manifest: bool, files: &[usize]) -> Self {
        Self {
            signer,
            chain_over: None,
            manifest,
            files: files.to_vec(),
            partial_field: true,
            query: "",
            repeat: false,
        }
    }
}

async fn send(app: &Router, u: &Upload, b: Batch<'_>) -> (StatusCode, HeaderMap, Value) {
    let over = b.chain_over.clone().unwrap_or_else(|| u.entity_id.clone());
    let chain = serde_json::to_string(&b.signer.chain(&over).await).unwrap();
    let mut parts: Vec<(&str, Option<&str>, &[u8])> = vec![
        ("entityId", None, u.entity_id.as_bytes()),
        ("authChain", None, chain.as_bytes()),
    ];
    if b.partial_field {
        parts.push(("partial", None, b"true"));
    }
    if b.manifest {
        parts.push((&u.entity_id, Some("entity.json"), &u.entity_bytes));
    }
    for i in &b.files {
        let (hash, bytes) = &u.files[*i];
        parts.push((hash, Some(hash), bytes));
    }
    if b.repeat {
        let (hash, bytes) = &u.files[b.files[0]];
        parts.push((hash, Some(hash), bytes));
    }
    let (body, content_type) = multipart_body(&parts);
    let request = Request::builder()
        .method("POST")
        .uri(format!("/entities{}", b.query))
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from(body))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let (status, headers) = (response.status(), response.headers().clone());
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, headers, serde_json::from_slice(&bytes).unwrap())
}

async fn expect_batch(app: &Router, u: &Upload, b: Batch<'_>, status: u16) -> Value {
    let (got, _, body) = send(app, u, b).await;
    assert_eq!(got.as_u16(), status, "{body}");
    body
}

fn hashes(u: &Upload, idx: &[usize]) -> Value {
    json!(
        idx.iter()
            .map(|i| u.files[*i].0.clone())
            .collect::<Vec<_>>()
    )
}

async fn pending_count(pool: &PgPool, entity_id: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM worlds.pending_scenes WHERE entity_id = $1")
        .bind(entity_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn world_rows(pool: &PgPool, world: &WorldName) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM worlds.worlds WHERE name = $1")
        .bind(world.key())
        .fetch_one(pool)
        .await
        .unwrap()
}

/// A fixture whose deploy route runs under `limits`.
async fn fixture_with_limits(limits: PartialUploadLimits) -> Option<Fixture> {
    let mut f = fixture().await?;
    let mut settings = Settings::for_tests(Some(ADMIN_TOKEN));
    settings.partial_uploads = limits;
    let state = Arc::new(CoreState::new(
        f.pool.clone(),
        f.storage.clone(),
        f.state.policy.clone(),
        settings,
        None,
    ));
    f.app = router(state.clone());
    f.state = state;
    Some(f)
}

async fn owned(f: &Fixture) -> (WorldName, Wallet) {
    let (world, owner) = (world_name(), Wallet::random());
    f.names.set(&owner.address, world.label());
    (world, owner)
}

#[tokio::test]
async fn batches_report_what_is_missing_and_the_last_one_publishes() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = owned(&f).await;
    let u = upload(&world, 3);

    let body = expect_batch(&f.app, &u, Batch::of(&owner, true, &[]), 202).await;
    assert_eq!(body, json!({ "missing": hashes(&u, &[0, 1, 2]) }));
    let body = expect_batch(&f.app, &u, Batch::of(&owner, true, &[0]), 202).await;
    assert_eq!(body, json!({ "missing": hashes(&u, &[1, 2]) }));
    // The original signer resumes without the manifest.
    let body = expect_batch(&f.app, &u, Batch::of(&owner, false, &[1]), 202).await;
    assert_eq!(body, json!({ "missing": hashes(&u, &[2]) }));
    assert_eq!(
        world_rows(&f.pool, &world).await,
        0,
        "a half-uploaded world has no world row"
    );

    let done = expect_batch(&f.app, &u, Batch::of(&owner, false, &[2]), 200).await;
    assert!(done["creationTimestamp"].is_i64(), "{done}");
    assert!(
        done["message"].as_str().unwrap().starts_with(&format!(
            "Your scene was deployed to World \"{}\"",
            world.display()
        )),
        "{done}"
    );
    assert_eq!(
        deployed_entity_ids(&f.pool, &world).await,
        vec![u.entity_id.clone()]
    );
    assert_eq!(pending_count(&f.pool, &u.entity_id).await, 0);
    for (hash, _) in &u.files {
        assert!(f.storage.exist(hash).await.unwrap());
    }

    // A lost completion answer is replayed; any signer's batch of a live entity gets it too.
    let retry = expect_batch(&f.app, &u, Batch::of(&owner, false, &[2]), 200).await;
    assert_eq!(retry, done);
    let stranger = Wallet::random();
    for manifest in [true, false] {
        let other = expect_batch(&f.app, &u, Batch::of(&stranger, manifest, &[0]), 200).await;
        assert_eq!(other["creationTimestamp"], done["creationTimestamp"]);
    }
}

#[tokio::test]
async fn a_first_batch_with_every_file_publishes_at_once() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = owned(&f).await;
    let u = upload(&world, 2);
    let body = expect_batch(&f.app, &u, Batch::of(&owner, true, &[0, 1]), 200).await;
    assert!(body["creationTimestamp"].is_i64(), "{body}");
    assert_eq!(
        deployed_entity_ids(&f.pool, &world).await,
        vec![u.entity_id.clone()]
    );
    let receipt: i64 = sqlx::query_scalar(
        "SELECT creation_timestamp FROM worlds.completed_scene_uploads WHERE entity_id = $1",
    )
    .bind(&u.entity_id)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(json!(receipt), body["creationTimestamp"]);
}

#[tokio::test]
async fn every_batch_is_authenticated_and_only_the_starter_resumes_without_the_manifest() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = owned(&f).await;
    let stranger = Wallet::random();
    let u = upload(&world, 2);
    expect_batch(&f.app, &u, Batch::of(&owner, true, &[]), 202).await;

    let body = expect_batch(&f.app, &u, Batch::of(&stranger, false, &[0]), 400).await;
    assert_eq!(
        body["message"],
        format!(
            "The first partial request for an entity must include the entity file \"{}\".",
            u.entity_id
        )
    );
    // With the manifest a stranger is checked for the permission like a publish.
    let body = expect_batch(&f.app, &u, Batch::of(&stranger, true, &[0]), 400).await;
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("has no permission to publish"),
        "{body}"
    );
    // A chain over another id does not authenticate a batch of this entity.
    let forged = Batch {
        chain_over: Some(hash_bytes_v1(b"another entity")),
        ..Batch::of(&owner, false, &[0])
    };
    let body = expect_batch(&f.app, &u, forged, 400).await;
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .starts_with("Deployment failed: The auth chain is invalid"),
        "{body}"
    );
    let staged: i64 =
        sqlx::query_scalar("SELECT count(*) FROM worlds.pending_scene_files WHERE entity_id = $1")
            .bind(&u.entity_id)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(staged, 1, "only the manifest is staged");

    // An authorized grantee with the manifest still cannot add to another account's upload.
    let grantee = Wallet::random();
    grant_parcels(&f.pool, &world, &grantee.address, &["0,0"]).await;
    let body = expect_batch(&f.app, &u, Batch::of(&grantee, true, &[0]), 400).await;
    assert_eq!(
        body["message"],
        "This upload was started by another account."
    );

    let body = expect_batch(&f.app, &u, Batch::of(&owner, false, &[0, 1]), 200).await;
    assert!(body["creationTimestamp"].is_i64(), "{body}");
}

#[tokio::test]
async fn malformed_batches_are_refused() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = owned(&f).await;
    let u = upload(&world, 2);
    let query_only = Batch {
        partial_field: false,
        query: "?partial=true",
        ..Batch::of(&owner, true, &[])
    };
    let body = expect_batch(&f.app, &u, query_only, 400).await;
    assert_eq!(
        body["message"],
        "The 'partial=true' query parameter requires the 'partial=true' form field"
    );
    let both = Batch {
        query: "?partial=true",
        ..Batch::of(&owner, true, &[])
    };
    expect_batch(&f.app, &u, both, 202).await;
    let repeated = Batch {
        repeat: true,
        ..Batch::of(&owner, false, &[0])
    };
    let body = expect_batch(&f.app, &u, repeated, 400).await;
    assert_eq!(
        body["message"],
        format!("Duplicate form field '{}'", u.files[0].0)
    );
    let body = expect_batch(&f.app, &u, Batch::of(&owner, true, &[]), 202).await;
    assert_eq!(
        body,
        json!({ "missing": hashes(&u, &[0, 1]) }),
        "the repeated file was not staged"
    );
}

#[tokio::test]
async fn a_single_request_publish_supersedes_the_pending_upload() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = owned(&f).await;
    let u = upload(&world, 1);
    expect_batch(&f.app, &u, Batch::of(&owner, true, &[]), 202).await;
    let regular = Batch {
        partial_field: false,
        ..Batch::of(&owner, true, &[0])
    };
    expect_batch(&f.app, &u, regular, 200).await;
    assert_eq!(pending_count(&f.pool, &u.entity_id).await, 0);
}

/// Moves `entity_id`'s upload and its stored files `by` into the past.
async fn age_upload(f: &Fixture, u: &Upload, by: std::time::Duration) {
    sqlx::query(
        "UPDATE worlds.pending_scenes SET created_at = created_at - make_interval(secs => $2) \
         WHERE entity_id = $1",
    )
    .bind(&u.entity_id)
    .bind(by.as_secs_f64())
    .execute(&f.pool)
    .await
    .unwrap();
    let past = std::time::SystemTime::now() - by;
    let keys = std::iter::once(&u.entity_id).chain(u.files.iter().map(|(h, _)| h));
    for key in keys {
        if let Some(path) = f.storage.file_path(key).await.unwrap() {
            std::fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(past)
                .unwrap();
        }
    }
}

#[tokio::test]
async fn an_expired_upload_is_refused_and_reclaimed_by_the_cleanup() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = owned(&f).await;
    let u = upload(&world, 2);
    expect_batch(&f.app, &u, Batch::of(&owner, true, &[0]), 202).await;
    assert!(f.storage.exist(&u.files[0].0).await.unwrap());
    age_upload(&f, &u, std::time::Duration::from_secs(2 * 3600)).await;

    // Without a live upload a resume must restart, and the expired one cannot be resumed.
    let body = expect_batch(&f.app, &u, Batch::of(&owner, false, &[1]), 400).await;
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .starts_with("The first partial request"),
        "{body}"
    );
    let body = expect_batch(&f.app, &u, Batch::of(&owner, true, &[1]), 400).await;
    assert_eq!(
        body["message"],
        "This upload expired. Create a new entity with a fresh timestamp."
    );

    let report = partial::delete_expired_uploads(&f.state).await;
    assert!(report.removed >= 1, "{report:?}");
    assert_eq!(pending_count(&f.pool, &u.entity_id).await, 0);
    assert!(!f.storage.exist(&u.files[0].0).await.unwrap());
    assert!(!f.storage.exist(&u.entity_id).await.unwrap());
    assert!(!f.storage.exist(&u.files[1].0).await.unwrap());
}

#[tokio::test]
async fn the_cleanup_keeps_files_something_else_names() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = owned(&f).await;
    let u = upload(&world, 2);
    expect_batch(&f.app, &u, Batch::of(&owner, true, &[0]), 202).await;
    // Another scene, published from the stored file alone, names the same file.
    let (other_world, other_owner) = owned(&f).await;
    let entity = publish_entity(
        &other_world,
        &["0,0"],
        json!([{ "file": "f0.bin", "hash": u.files[0].0 }]),
    );
    let entity_bytes = serde_json::to_vec(&entity).unwrap();
    let shared = Upload {
        entity_id: hash_bytes_v1(&entity_bytes),
        entity_bytes,
        files: u.files.clone(),
    };
    expect_batch(&f.app, &shared, Batch::of(&other_owner, true, &[]), 200).await;
    age_upload(&f, &u, std::time::Duration::from_secs(2 * 3600)).await;

    partial::delete_expired_uploads(&f.state).await;
    assert_eq!(pending_count(&f.pool, &u.entity_id).await, 0);
    assert!(
        f.storage.exist(&u.files[0].0).await.unwrap(),
        "a published scene names it"
    );
    assert!(!f.storage.exist(&u.entity_id).await.unwrap());
}

#[tokio::test]
async fn the_collector_keeps_a_pending_uploads_files() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = owned(&f).await;
    let u = upload(&world, 2);
    expect_batch(&f.app, &u, Batch::of(&owner, true, &[0]), 202).await;
    let orphan = hash_bytes_v1(b"an unreferenced blob");
    f.storage
        .store(&orphan, bytes::Bytes::from_static(b"an unreferenced blob"))
        .await
        .unwrap();

    let keys = vec![
        u.entity_id.clone(),
        u.files[0].0.clone(),
        u.files[1].0.clone(),
    ];
    let referenced = catalyst_storage::gc::referenced_among(&f.pool, &keys)
        .await
        .unwrap();
    assert_eq!(referenced.len(), 3, "{referenced:?}");
    let published = catalyst_storage::gc::published_among(&f.pool, &keys)
        .await
        .unwrap();
    assert!(published.is_empty(), "{published:?}");

    let report = catalyst_storage::gc::collect(
        &f.pool,
        &f.storage,
        &[],
        catalyst_storage::gc::GcOptions {
            dry_run: false,
            min_age: std::time::Duration::ZERO,
            allow_empty_reference_set: false,
        },
    )
    .await
    .unwrap();
    assert!(report.removed >= 1, "{report:?}");
    assert!(!f.storage.exist(&orphan).await.unwrap());
    assert!(f.storage.exist(&u.entity_id).await.unwrap());
    assert!(f.storage.exist(&u.files[0].0).await.unwrap());

    expect_batch(&f.app, &u, Batch::of(&owner, false, &[1]), 200).await;
}

#[tokio::test]
async fn gc_reports_the_expired_upload_cleanup() {
    let Some(f) = fixture().await else { return };
    let body = gc(&f.app, "/gc").await;
    assert!(
        ["success", "deferred", "error"]
            .contains(&body["expiredUploads"]["outcome"].as_str().unwrap()),
        "{body}"
    );
    assert!(body["expiredUploads"]["removed"].is_u64(), "{body}");
    let dry = gc(&f.app, "/gc?dry_run=true").await;
    assert_eq!(dry["expiredUploads"], Value::Null);
}

/// `/gc`'s answer; another test may hold the fleet-wide run lock for a moment.
async fn gc(app: &Router, path: &str) -> Value {
    for _ in 0..50 {
        let (status, body) = hit(app, Case::new("post", path).bearer(ADMIN_TOKEN)).await;
        if status != StatusCode::SERVICE_UNAVAILABLE {
            assert_eq!(status, StatusCode::OK, "{body}");
            return body;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("{path} stayed busy");
}

fn limits(edit: impl FnOnce(&mut PartialUploadLimits)) -> PartialUploadLimits {
    let mut limits = PartialUploadLimits::default();
    edit(&mut limits);
    limits
}

fn retry_after(headers: &HeaderMap) -> u64 {
    headers
        .get(header::RETRY_AFTER)
        .expect("a 429 carries Retry-After")
        .to_str()
        .unwrap()
        .parse()
        .unwrap()
}

#[tokio::test]
async fn an_account_holds_at_most_its_uploads() {
    let Some(f) = fixture_with_limits(limits(|l| l.max_pending_per_deployer = 1)).await else {
        return;
    };
    let (world, owner) = owned(&f).await;
    let (a, b) = (
        upload_of(&world, &["0,0"], 1, 8),
        upload_of(&world, &["1,0"], 1, 8),
    );
    expect_batch(&f.app, &a, Batch::of(&owner, true, &[]), 202).await;
    let (status, headers, body) = send(&f.app, &b, Batch::of(&owner, true, &[])).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert_eq!(body["error"], "Too Many Requests");
    assert!(
        body["message"].as_str().unwrap().starts_with(
            "Too many partial uploads in progress for this account: 1 of the 1 allowed."
        ),
        "{body}"
    );
    assert!(retry_after(&headers) >= 1);
    assert_eq!(pending_count(&f.pool, &b.entity_id).await, 0);
    // Completing the first upload frees the slot.
    expect_batch(&f.app, &a, Batch::of(&owner, false, &[0]), 200).await;
    expect_batch(&f.app, &b, Batch::of(&owner, true, &[]), 202).await;
}

#[tokio::test]
async fn one_account_leaves_a_world_slot_for_its_other_deployers() {
    let Some(f) = fixture_with_limits(limits(|l| l.max_pending_per_world = 2)).await else {
        return;
    };
    let (world, owner) = owned(&f).await;
    let (a, b) = (
        upload_of(&world, &["0,0"], 1, 8),
        upload_of(&world, &["1,0"], 1, 8),
    );
    expect_batch(&f.app, &a, Batch::of(&owner, true, &[]), 202).await;
    let (status, _, body) = send(&f.app, &b, Batch::of(&owner, true, &[])).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("for this account in this world: 1 of the 1 allowed"),
        "{body}"
    );
    let grantee = Wallet::random();
    grant_parcels(&f.pool, &world, &grantee.address, &["1,0"]).await;
    expect_batch(&f.app, &b, Batch::of(&grantee, true, &[]), 202).await;
}

#[tokio::test]
async fn batch_bytes_are_rate_limited_per_minute() {
    let Some(f) = fixture_with_limits(limits(|l| l.max_bytes_per_minute = 1_000)).await else {
        return;
    };
    let (world, owner) = owned(&f).await;
    let big = upload_of(&world, &["0,0"], 1, 2_000);
    let body = expect_batch(&f.app, &big, Batch::of(&owner, true, &[0]), 400).await;
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .starts_with("This batch is "),
        "{body}"
    );
    let (world, owner) = owned(&f).await;
    let u = upload_of(&world, &["0,0"], 2, 400);
    let (status, _, body) = send(&f.app, &u, Batch::of(&owner, true, &[0])).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    let (status, headers, body) = send(&f.app, &u, Batch::of(&owner, false, &[1])).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert!(
        (1..=60).contains(&retry_after(&headers)),
        "a fixed one-minute window"
    );
}

#[tokio::test]
async fn staging_budgets_refuse_what_can_never_fit_and_throttle_what_others_fill() {
    // The node budget is shared with every other test; the account budget is this wallet's.
    let Some(f) = fixture_with_limits(limits(|l| l.max_pending_bytes_per_deployer = 3_000)).await
    else {
        return;
    };
    let (world, owner) = owned(&f).await;
    let huge = upload_of(&world, &["0,0"], 2, 2_000);
    expect_batch(&f.app, &huge, Batch::of(&owner, true, &[0]), 202).await;
    let body = expect_batch(&f.app, &huge, Batch::of(&owner, false, &[1]), 400).await;
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("above the per-account partial upload limit"),
        "{body}"
    );
    let other = upload_of(&world, &["1,0"], 1, 1_500);
    let (status, headers, body) = send(&f.app, &other, Batch::of(&owner, true, &[0])).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("for this account, above its limit"),
        "{body}"
    );
    assert!(retry_after(&headers) >= 1);
    assert_eq!(pending_count(&f.pool, &other.entity_id).await, 0);
}

#[tokio::test]
async fn a_scene_over_the_world_cap_is_refused_while_staging() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = owned(&f).await;
    set_limit_override(&f.pool, &world, None, Some(1_000)).await;
    let u = upload_of(&world, &["0,0"], 2, 600);
    expect_batch(&f.app, &u, Batch::of(&owner, true, &[0]), 202).await;
    let body = expect_batch(&f.app, &u, Batch::of(&owner, false, &[1]), 400).await;
    assert_eq!(
        body["message"],
        "Deployment failed: The deployment is too big."
    );
}
