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
    /// The client address the gateway reports.
    source: Option<&'static str>,
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
            source: None,
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
        .header("x-real-ip", b.source.unwrap_or(""))
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

fn gc_pins(f: &Fixture) -> catalyst_storage::gc::Pins<'_> {
    catalyst_storage::gc::Pins::new(&f.storage, &f.state.settings.global_scenes_urn)
}

/// A fixture whose deploy route runs under `limits`.
async fn fixture_with_limits(limits: PartialUploadLimits) -> Option<Fixture> {
    fixture_with_settings(|s| s.partial_uploads = limits).await
}

/// A fixture whose deploy route runs under the edited test settings.
async fn fixture_with_settings(edit: impl FnOnce(&mut Settings)) -> Option<Fixture> {
    let mut f = fixture().await?;
    let mut settings = Settings::for_tests(Some(ADMIN_TOKEN));
    edit(&mut settings);
    let state = Arc::new(
        CoreState::new(
            f.pool.clone(),
            f.storage.clone(),
            f.state.policy.clone(),
            settings,
            None,
        )
        .with_allowance(f.allowance.clone()),
    );
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

/// Held from aging an upload to checking its cleanup: a run reclaims every expired upload.
static CLEANUP: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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
    let _cleanup = CLEANUP.lock().await;
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
async fn one_cleanup_run_reclaims_several_uploads() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = owned(&f).await;
    let uploads = [upload(&world, 2), upload_of(&world, &["1,0"], 2, 64)];
    for u in &uploads {
        expect_batch(&f.app, u, Batch::of(&owner, true, &[0]), 202).await;
    }
    let _cleanup = CLEANUP.lock().await;
    for u in &uploads {
        age_upload(&f, u, std::time::Duration::from_secs(2 * 3600)).await;
    }
    let report = partial::delete_expired_uploads(&f.state).await;
    assert!(report.removed >= 2, "{report:?}");
    for u in &uploads {
        assert_eq!(pending_count(&f.pool, &u.entity_id).await, 0);
        assert!(!f.storage.exist(&u.files[0].0).await.unwrap());
        assert!(!f.storage.exist(&u.entity_id).await.unwrap());
    }
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
    let _cleanup = CLEANUP.lock().await;
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
    let _gc_runs = GC_RUNS.lock().await;
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
    let referenced = catalyst_storage::gc::referenced_among(&f.pool, gc_pins(&f), &keys)
        .await
        .unwrap();
    assert_eq!(referenced.len(), 3, "{referenced:?}");
    let published = catalyst_storage::gc::published_among(&f.pool, gc_pins(&f), &keys)
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
    let _gc_runs = GC_RUNS.lock().await;
    let _cleanup = CLEANUP.lock().await;
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

#[tokio::test]
async fn staging_is_held_to_the_owners_storage_allowance() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = owned(&f).await;
    let u = upload_of(&world, &["0,0"], 2, 64);
    // An allowance that cannot be read refuses the batch, as a publish.
    f.allowance.set(&owner.address, None);
    let body = expect_batch(&f.app, &u, Batch::of(&owner, true, &[0]), 503).await;
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("storage allowance"),
        "{body}"
    );
    f.allowance.set(&owner.address, Some(100));
    expect_batch(&f.app, &u, Batch::of(&owner, true, &[0]), 202).await;
    // The resume skips the permission, not the allowance.
    let body = expect_batch(&f.app, &u, Batch::of(&owner, false, &[1]), 400).await;
    assert_eq!(
        body["message"],
        "Deployment failed: The deployment is too big."
    );
    f.allowance.set(&owner.address, Some(128));
    expect_batch(&f.app, &u, Batch::of(&owner, false, &[1]), 200).await;

    // An owner without allowance left is refused at the first batch carrying content.
    let (world, owner) = owned(&f).await;
    f.allowance.set(&owner.address, Some(0));
    let v = upload_of(&world, &["0,0"], 1, 64);
    let body = expect_batch(&f.app, &v, Batch::of(&owner, true, &[0]), 400).await;
    assert_eq!(
        body["message"],
        "Deployment failed: The deployment is too big."
    );
    assert_eq!(pending_count(&f.pool, &v.entity_id).await, 0);
}

/// Moves each stored key's modification time `by` into the past.
async fn age_keys(f: &Fixture, keys: &[&String], by: std::time::Duration) {
    let past = std::time::SystemTime::now() - by;
    for key in keys {
        let path = f.storage.file_path(key).await.unwrap().unwrap();
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(past)
            .unwrap();
    }
}

async fn put(f: &Fixture, bytes: &[u8]) -> String {
    let key = hash_bytes_v1(bytes);
    f.storage
        .store(&key, bytes::Bytes::copy_from_slice(bytes))
        .await
        .unwrap();
    key
}

#[tokio::test]
async fn the_cleanup_unlinks_only_what_the_upload_stored() {
    let tag = Uuid::new_v4().simple().to_string();
    let global_file_bytes = format!("global file {tag}").into_bytes();
    let global_file = hash_bytes_v1(&global_file_bytes);
    let global_bytes =
        serde_json::to_vec(&json!({ "content": [{ "file": "g.js", "hash": global_file }] }))
            .unwrap();
    let urn = format!(
        "urn:decentraland:entity:{}?baseUrl=https://x/contents/",
        hash_bytes_v1(&global_bytes)
    );
    let Some(f) = fixture_with_settings(|s| s.global_scenes_urn = vec![urn]).await else {
        return;
    };
    put(&f, &global_file_bytes).await;
    let global = put(&f, &global_bytes).await;
    let pack_file = put(&f, format!("pack file {tag}").as_bytes()).await;
    let foreign = put(&f, format!("foreign file {tag}").as_bytes()).await;
    sqlx::query(
        "INSERT INTO builder.asset_catalog (id, packs, source, built_at) \
         VALUES (1, $1, 'test', timezone('UTC', now())) \
         ON CONFLICT (id) DO UPDATE SET packs = builder.asset_catalog.packs || EXCLUDED.packs",
    )
    .bind(json!([{ "assets": [{ "contents": { "a.glb": pack_file } }] }]))
    .execute(&f.pool)
    .await
    .unwrap();

    // A manifest naming stored files it never sends, one of its own and one left outstanding.
    let (world, owner) = owned(&f).await;
    let mut u = upload(&world, 2);
    let mut content: Vec<Value> = u
        .files
        .iter()
        .enumerate()
        .map(|(i, (hash, _))| json!({ "file": format!("f{i}.bin"), "hash": hash }))
        .collect();
    let named = [&global_file, &global, &pack_file, &foreign];
    for (i, hash) in named.iter().enumerate() {
        content.push(json!({ "file": format!("named{i}.bin"), "hash": hash }));
    }
    u.entity_bytes =
        serde_json::to_vec(&publish_entity(&world, &["0,0"], Value::Array(content))).unwrap();
    u.entity_id = hash_bytes_v1(&u.entity_bytes);
    let body = expect_batch(&f.app, &u, Batch::of(&owner, true, &[0]), 202).await;
    assert_eq!(body, json!({ "missing": hashes(&u, &[1]) }));

    let two_hours = std::time::Duration::from_secs(2 * 3600);
    let _cleanup = CLEANUP.lock().await;
    age_upload(&f, &u, two_hours).await;
    age_keys(&f, &named, two_hours).await;
    partial::delete_expired_uploads(&f.state).await;
    assert_eq!(pending_count(&f.pool, &u.entity_id).await, 0);
    assert!(!f.storage.exist(&u.files[0].0).await.unwrap());
    assert!(!f.storage.exist(&u.entity_id).await.unwrap());
    for key in named {
        assert!(f.storage.exist(key).await.unwrap(), "{key} stays");
    }
    let referenced = catalyst_storage::gc::referenced_among(
        &f.pool,
        gc_pins(&f),
        &[global_file.clone(), global.clone(), pack_file.clone()],
    )
    .await
    .unwrap();
    assert_eq!(referenced.len(), 3, "{referenced:?}");
}

#[tokio::test]
async fn the_cleanup_leaves_a_young_file_to_the_collector() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = owned(&f).await;
    let u = upload(&world, 2);
    expect_batch(&f.app, &u, Batch::of(&owner, true, &[0]), 202).await;
    let _cleanup = CLEANUP.lock().await;
    sqlx::query(
        "UPDATE worlds.pending_scenes SET created_at = created_at - interval '2 hours' \
         WHERE entity_id = $1",
    )
    .bind(&u.entity_id)
    .execute(&f.pool)
    .await
    .unwrap();
    partial::delete_expired_uploads(&f.state).await;
    assert_eq!(pending_count(&f.pool, &u.entity_id).await, 0);
    assert!(f.storage.exist(&u.files[0].0).await.unwrap());
}

#[tokio::test]
async fn a_stored_file_collected_before_completion_is_asked_for_again() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = owned(&f).await;
    let u = upload(&world, 2);
    put(&f, &u.files[0].1).await;
    let body = expect_batch(&f.app, &u, Batch::of(&owner, true, &[]), 202).await;
    assert_eq!(body, json!({ "missing": hashes(&u, &[1]) }));
    f.storage.delete_strict(&u.files[0].0).await.unwrap();

    let body = expect_batch(&f.app, &u, Batch::of(&owner, false, &[1]), 202).await;
    assert_eq!(body, json!({ "missing": hashes(&u, &[0]) }));
    let done = expect_batch(&f.app, &u, Batch::of(&owner, false, &[0]), 200).await;
    assert!(done["creationTimestamp"].is_i64(), "{done}");
}

#[tokio::test]
async fn a_personal_world_stages_only_once_published() {
    let personal = PersonalWorldsPolicy::new(
        1_000_000,
        50 * 1024 * 1024,
        crate::policy::MAX_SCENE_SIZE_BYTES,
    )
    .unwrap();
    let Some(f) = fixture_with(personal, None).await else {
        return;
    };
    let owner = Wallet::random();
    let world = WorldName::parse(&format!("{}.dcl.eth", owner.address.as_str())).unwrap();
    let u = upload(&world, 2);
    let body = expect_batch(&f.app, &u, Batch::of(&owner, true, &[0]), 400).await;
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .starts_with("Partial uploads to a personal world"),
        "{body}"
    );
    assert_eq!(pending_count(&f.pool, &u.entity_id).await, 0);
    // Its world row, as a first publish leaves it; no scene, so the realm count stays as other tests expect.
    sqlx::query("INSERT INTO worlds.worlds (name) VALUES ($1)")
        .bind(world.key())
        .execute(&f.pool)
        .await
        .unwrap();
    expect_batch(&f.app, &u, Batch::of(&owner, true, &[0]), 202).await;
}

#[tokio::test]
async fn uploads_from_one_source_share_its_staging_budget() {
    let Some(f) = fixture_with_limits(limits(|l| {
        l.max_pending_bytes_per_deployer = 4_000;
        l.max_pending_bytes_per_source = 4_000;
    }))
    .await
    else {
        return;
    };
    let ip = format!("198.51.100.{}", Uuid::new_v4().as_u128() % 250 + 1);
    let ip: &'static str = Box::leak(ip.into_boxed_str());
    let (world_a, a) = owned(&f).await;
    let (world_b, b) = owned(&f).await;
    let ua = upload_of(&world_a, &["0,0"], 2, 1_200);
    let ub = upload_of(&world_b, &["0,0"], 2, 1_200);
    let from = |signer, manifest, files: &[usize]| Batch {
        source: Some(ip),
        ..Batch::of(signer, manifest, files)
    };
    expect_batch(&f.app, &ua, from(&a, true, &[0]), 202).await;
    expect_batch(&f.app, &ub, from(&b, true, &[0]), 202).await;
    // Each wallet stays under its own budget; together they fill the source's.
    let (status, headers, body) = send(&f.app, &ub, from(&b, false, &[1])).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("for uploads from this source"),
        "{body}"
    );
    assert!(retry_after(&headers) >= 1);
}

#[tokio::test]
async fn a_partial_upload_of_a_mirrored_scene_publishes_it_here() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = owned(&f).await;
    let u = upload(&world, 1);
    sqlx::query(
        "INSERT INTO worlds.worlds (name, mirror_source) VALUES ($1, 'https://upstream.example')",
    )
    .bind(world.key())
    .execute(&f.pool)
    .await
    .unwrap();
    let entity: Value = serde_json::from_slice(&u.entity_bytes).unwrap();
    sqlx::query(
        "INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size, mirror_source) \
         VALUES ($1, $2, '[]'::json, $3, $4, ARRAY['0,0'], 1, 'https://upstream.example')",
    )
    .bind(world.key())
    .bind(&u.entity_id)
    .bind(&entity)
    .bind(crate::ZERO_ADDRESS)
    .execute(&f.pool)
    .await
    .unwrap();

    // Mirrored only, the entity is not answered as deployed: the upload stages and completes.
    let body = expect_batch(&f.app, &u, Batch::of(&owner, true, &[]), 202).await;
    assert_eq!(body, json!({ "missing": hashes(&u, &[0]) }));
    expect_batch(&f.app, &u, Batch::of(&owner, false, &[0]), 200).await;
    let mirror: Option<String> =
        sqlx::query_scalar("SELECT mirror_source FROM worlds.world_scenes WHERE entity_id = $1")
            .bind(&u.entity_id)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(mirror, None, "the scene is published here");
}

#[tokio::test]
async fn concurrent_batches_of_one_entity_publish_once() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = owned(&f).await;
    let u = upload(&world, 2);
    expect_batch(&f.app, &u, Batch::of(&owner, true, &[]), 202).await;
    let (a, b) = tokio::join!(
        send(&f.app, &u, Batch::of(&owner, false, &[0])),
        send(&f.app, &u, Batch::of(&owner, false, &[1])),
    );
    let mut statuses = [a.0.as_u16(), b.0.as_u16()];
    statuses.sort();
    assert_eq!(statuses, [200, 202], "{} {}", a.2, b.2);
    assert_eq!(
        deployed_entity_ids(&f.pool, &world).await,
        vec![u.entity_id.clone()]
    );
    assert_eq!(pending_count(&f.pool, &u.entity_id).await, 0);
}

#[tokio::test]
async fn a_refused_batch_keeps_an_upload_another_replica_created() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = owned(&f).await;
    let u = upload(&world, 1);
    // The account's per-minute rate is spent, so this batch is refused after its upsert.
    sqlx::query(
        "INSERT INTO worlds.partial_upload_rates (deployer, window_started, bytes) \
         VALUES ($1, timezone('UTC', now()), $2) \
         ON CONFLICT (deployer) DO UPDATE SET window_started = EXCLUDED.window_started, bytes = EXCLUDED.bytes",
    )
    .bind(owner.address.as_str())
    .bind(PartialUploadLimits::default().max_bytes_per_minute as i64)
    .execute(&f.pool)
    .await
    .unwrap();
    // Held so this batch reaches its upsert having seen no upload.
    let mut held = f.pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!(
            "worlds-partial-deployer:{}",
            owner.address.as_str()
        ))
        .execute(&mut *held)
        .await
        .unwrap();
    let holder = backend_pid(&mut held).await;
    let ((status, _, body), ()) = tokio::join!(
        send(&f.app, &u, Batch::of(&owner, true, &[])),
        async {
            blocked_by(&f.pool, holder, "pg_advisory_xact_lock(hashtextextended", 1).await;
            // Another replica creates the upload meanwhile.
            let entity: Value = serde_json::from_slice(&u.entity_bytes).unwrap();
            sqlx::query(
                "INSERT INTO worlds.pending_scenes (entity_id, world_name, parcels, entity, deployer, created_at) \
                 VALUES ($1, $2, ARRAY['0,0'], $3, $4, timezone('UTC', now()))",
            )
            .bind(&u.entity_id)
            .bind(world.key())
            .bind(&entity)
            .bind(owner.address.as_str())
            .execute(&f.pool)
            .await
            .unwrap();
            held.commit().await.unwrap();
        },
    );
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert_eq!(
        pending_count(&f.pool, &u.entity_id).await,
        1,
        "the other replica's upload stays"
    );
}

#[tokio::test]
async fn an_upload_expiring_during_its_completing_publish_is_refused_as_expired() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = owned(&f).await;
    let u = upload(&world, 2);
    put(&f, &u.files[0].1).await;
    sqlx::query("INSERT INTO worlds.worlds (name) VALUES ($1)")
        .bind(world.key())
        .execute(&f.pool)
        .await
        .unwrap();
    expect_batch(&f.app, &u, Batch::of(&owner, true, &[]), 202).await;

    // The completing batch waits on the world row inside its publish.
    let mut held = f.pool.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM worlds.worlds WHERE name = $1 FOR UPDATE")
        .bind(world.key())
        .execute(&mut *held)
        .await
        .unwrap();
    let holder = backend_pid(&mut held).await;
    let ((status, _, body), ()) =
        tokio::join!(send(&f.app, &u, Batch::of(&owner, false, &[1])), async {
            blocked_by(&f.pool, holder, "SELECT owner FROM worlds.worlds", 1).await;
            // Meanwhile the upload expires and a file it found stored is collected.
            sqlx::query(
                "UPDATE worlds.pending_scenes SET created_at = created_at - interval '2 hours' \
                 WHERE entity_id = $1",
            )
            .bind(&u.entity_id)
            .execute(&f.pool)
            .await
            .unwrap();
            f.storage.delete_strict(&u.files[0].0).await.unwrap();
            held.commit().await.unwrap();
        },);
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        body["message"],
        "This upload expired. Create a new entity with a fresh timestamp."
    );
    assert!(deployed_entity_ids(&f.pool, &world).await.is_empty());
}

#[tokio::test]
async fn a_completion_after_the_permission_is_revoked_is_refused() {
    let Some(f) = fixture().await else { return };
    let (world, _owner) = owned(&f).await;
    let grantee = Wallet::random();
    grant_parcels(&f.pool, &world, &grantee.address, &["0,0"]).await;
    let u = upload(&world, 1);
    expect_batch(&f.app, &u, Batch::of(&grantee, true, &[]), 202).await;
    sqlx::query("DELETE FROM worlds.world_permissions WHERE world_name = $1 AND address = $2")
        .bind(world.key())
        .bind(&grantee.address)
        .execute(&f.pool)
        .await
        .unwrap();
    let body = expect_batch(&f.app, &u, Batch::of(&grantee, false, &[0]), 400).await;
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("has no permission to publish"),
        "{body}"
    );
    assert!(deployed_entity_ids(&f.pool, &world).await.is_empty());
}

/// The backend id of the connection `conn`.
async fn backend_pid(conn: &mut sqlx::PgConnection) -> i32 {
    sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(conn)
        .await
        .unwrap()
}

/// Waits until `n` backends running a statement that contains `fragment` wait on `blocker`; their ids.
async fn blocked_by(pool: &PgPool, blocker: i32, fragment: &str, n: usize) -> Vec<i32> {
    waiting_on(pool, blocker, &[fragment], false, n).await
}

/// Waits until `n` backends running a statement with one of `fragments` wait on `blocker`,
/// directly or (`transitive`) behind other waiters; their ids.
async fn waiting_on(
    pool: &PgPool,
    blocker: i32,
    fragments: &[&str],
    transitive: bool,
    n: usize,
) -> Vec<i32> {
    for _ in 0..200 {
        let waiting: Vec<i32> = sqlx::query_scalar(
            "WITH RECURSIVE chain(pid, depth) AS ( \
               SELECT $1::int, 0 \
               UNION \
               SELECT a.pid, c.depth + 1 FROM pg_stat_activity a \
               JOIN chain c ON c.pid = ANY(pg_blocking_pids(a.pid)) \
               WHERE c.depth < 8 AND ($3 OR c.depth = 0)) \
             SELECT DISTINCT a.pid FROM pg_stat_activity a JOIN chain c ON c.pid = a.pid \
             WHERE c.depth > 0 AND EXISTS (SELECT 1 FROM unnest($2::text[]) f WHERE strpos(a.query, f) > 0)",
        )
        .bind(blocker)
        .bind(fragments)
        .bind(transitive)
        .fetch_all(pool)
        .await
        .unwrap();
        if waiting.len() >= n {
            return waiting;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let fragment = fragments.join(" | ");
    let waits: Vec<(i32, Vec<i32>, String)> = sqlx::query_as(
        "SELECT pid, pg_blocking_pids(pid), query FROM pg_stat_activity WHERE wait_event_type = 'Lock'",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    panic!("fewer than {n} statements with {fragment:?} waited on backend {blocker}: {waits:?}");
}

async fn insert_world(f: &Fixture, world: &WorldName) {
    sqlx::query("INSERT INTO worlds.worlds (name) VALUES ($1)")
        .bind(world.key())
        .execute(&f.pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn two_replicas_completing_one_entity_publish_it_once() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = owned(&f).await;
    let u = upload(&world, 1);
    insert_world(&f, &world).await;
    expect_batch(&f.app, &u, Batch::of(&owner, true, &[]), 202).await;

    // Both completing batches reach their publish and wait on the world row.
    let mut held = f.pool.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM worlds.worlds WHERE name = $1 FOR UPDATE")
        .bind(world.key())
        .execute(&mut *held)
        .await
        .unwrap();
    let holder = backend_pid(&mut held).await;
    let (a, b, ()) = tokio::join!(
        partial::REPLICA.scope("a", send(&f.app, &u, Batch::of(&owner, false, &[0]))),
        partial::REPLICA.scope("b", send(&f.app, &u, Batch::of(&owner, false, &[0]))),
        async {
            let first = blocked_by(&f.pool, holder, "SELECT owner FROM worlds.worlds", 1).await;
            // The second queues behind the first: on the row, or on an advisory lock (the world's
            // publish lock, the wallet's quota lock, or the store lock behind a collector).
            let row_or_lock = ["SELECT owner FROM worlds.worlds", "pg_advisory_xact_lock"];
            waiting_on(&f.pool, first[0], &row_or_lock, true, 1).await;
            held.commit().await.unwrap();
        },
    );
    assert_eq!((a.0.as_u16(), b.0.as_u16()), (200, 200), "{} {}", a.2, b.2);
    assert_eq!(
        deployed_entity_ids(&f.pool, &world).await,
        vec![u.entity_id.clone()]
    );
    assert_eq!(pending_count(&f.pool, &u.entity_id).await, 0);
}

#[tokio::test]
async fn a_resume_landing_after_another_replica_completed_replays_the_receipt() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = owned(&f).await;
    let u = upload(&world, 1);
    expect_batch(&f.app, &u, Batch::of(&owner, true, &[]), 202).await;

    let mut held = f.pool.begin().await.unwrap();
    let key = format!("worlds-partial-deployer:{}", owner.address.as_str());
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(key)
        .execute(&mut *held)
        .await
        .unwrap();
    let holder = backend_pid(&mut held).await;
    let racer = async {
        blocked_by(&f.pool, holder, "pg_advisory_xact_lock", 1).await;
        completed_elsewhere(&f.pool, &u, &owner, &world).await;
        held.commit().await.unwrap();
    };
    let ((status, _, body), ()) =
        tokio::join!(send(&f.app, &u, Batch::of(&owner, false, &[0])), racer);
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["creationTimestamp"], 7);
    assert_eq!(pending_count(&f.pool, &u.entity_id).await, 0);
}

#[tokio::test]
async fn a_grantee_resuming_after_the_owner_completed_never_gets_the_receipt() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = owned(&f).await;
    let u = upload(&world, 1);
    expect_batch(&f.app, &u, Batch::of(&owner, true, &[]), 202).await;
    let grantee = Wallet::random();
    grant_parcels(&f.pool, &world, &grantee.address, &["0,0"]).await;

    let mut held = f.pool.begin().await.unwrap();
    let key = format!("worlds-partial-deployer:{}", grantee.address.as_str());
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(key)
        .execute(&mut *held)
        .await
        .unwrap();
    let holder = backend_pid(&mut held).await;
    let racer = async {
        blocked_by(&f.pool, holder, "pg_advisory_xact_lock", 1).await;
        completed_elsewhere(&f.pool, &u, &owner, &world).await;
        held.commit().await.unwrap();
    };
    let ((status, _, body), ()) =
        tokio::join!(send(&f.app, &u, Batch::of(&grantee, true, &[0])), racer);
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        body["message"],
        "This upload expired. Create a new entity with a fresh timestamp."
    );
    assert!(body.get("creationTimestamp").is_none(), "{body}");
}

/// Another replica finished `u`: its pending row is gone and `owner` holds the receipt with creation timestamp 7.
async fn completed_elsewhere(pool: &PgPool, u: &Upload, owner: &Wallet, world: &WorldName) {
    sqlx::query("DELETE FROM worlds.pending_scenes WHERE entity_id = $1")
        .bind(&u.entity_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO worlds.completed_scene_uploads \
           (entity_id, deployer, world_name, parcels, creation_timestamp, completed_at) \
         VALUES ($1, $2, $3, '{}', 7, timezone('UTC', now()))",
    )
    .bind(&u.entity_id)
    .bind(owner.address.as_str())
    .bind(world.key())
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn an_upload_expiring_while_it_completes_is_reclaimed_by_a_concurrent_cleanup() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = owned(&f).await;
    let u = upload(&world, 2);
    insert_world(&f, &world).await;
    expect_batch(&f.app, &u, Batch::of(&owner, true, &[0]), 202).await;
    let _cleanup = CLEANUP.lock().await;

    // The completing batch holds the shared store lock while it waits on the world row.
    let mut held = f.pool.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM worlds.worlds WHERE name = $1 FOR UPDATE")
        .bind(world.key())
        .execute(&mut *held)
        .await
        .unwrap();
    let holder = backend_pid(&mut held).await;
    let racer = async {
        let publish = blocked_by(&f.pool, holder, "SELECT owner FROM worlds.worlds", 1).await;
        age_upload(&f, &u, std::time::Duration::from_secs(2 * 3600)).await;
        let (report, ()) = tokio::join!(partial::delete_expired_uploads(&f.state), async {
            blocked_by(&f.pool, publish[0], "pg_advisory_xact_lock($1)", 1).await;
            held.commit().await.unwrap();
        });
        report
    };
    let ((status, _, body), report) =
        tokio::join!(send(&f.app, &u, Batch::of(&owner, false, &[1])), racer);
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        body["message"],
        "This upload expired. Create a new entity with a fresh timestamp."
    );
    assert!(report.removed >= 1, "{report:?}");
    assert!(deployed_entity_ids(&f.pool, &world).await.is_empty());
    assert_eq!(pending_count(&f.pool, &u.entity_id).await, 0);
    for key in [&u.entity_id, &u.files[0].0, &u.files[1].0] {
        assert!(!f.storage.exist(key).await.unwrap(), "{key} is reclaimed");
    }
}

#[tokio::test]
async fn the_cleanup_keeps_a_file_the_catalogue_names_once_its_run_began() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = owned(&f).await;
    let u = upload(&world, 2);
    expect_batch(&f.app, &u, Batch::of(&owner, true, &[0]), 202).await;
    let _cleanup = CLEANUP.lock().await;
    age_upload(&f, &u, std::time::Duration::from_secs(2 * 3600)).await;

    // A publish holds the store lock while an import starts naming the upload's file.
    let mut held = f.pool.begin().await.unwrap();
    catalyst_storage::gc::lock_store_shared(&mut held)
        .await
        .unwrap();
    let holder = backend_pid(&mut held).await;
    let (report, ()) = tokio::join!(partial::delete_expired_uploads(&f.state), async {
        blocked_by(&f.pool, holder, "pg_advisory_xact_lock($1)", 1).await;
        sqlx::query(
            "INSERT INTO builder.asset_catalog (id, packs, source, built_at) \
             VALUES (1, $1, 'test', timezone('UTC', now())) \
             ON CONFLICT (id) DO UPDATE SET packs = builder.asset_catalog.packs || EXCLUDED.packs",
        )
        .bind(json!([{ "assets": [{ "contents": { "a.glb": u.files[0].0 } }] }]))
        .execute(&f.pool)
        .await
        .unwrap();
        held.commit().await.unwrap();
    });
    assert!(report.removed >= 1, "{report:?}");
    assert_eq!(pending_count(&f.pool, &u.entity_id).await, 0);
    assert!(
        f.storage.exist(&u.files[0].0).await.unwrap(),
        "the catalogue names it"
    );
    assert!(!f.storage.exist(&u.entity_id).await.unwrap());
}
