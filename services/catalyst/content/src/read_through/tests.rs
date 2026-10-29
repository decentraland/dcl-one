//! Read-through against an in-process upstream (`/content/entities/active`, `/content/audit`,
//! `/content/contents`) serving entities signed with the sync tests' key, over the scratch
//! database and a temp blob store. Needs CATALYST_TEST_DATABASE_URL, except the [`Fills`]
//! and configuration tests.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};
use tokio::sync::{Notify, watch};
use tower::ServiceExt;

use super::*;
use crate::auth::hash_bytes_v1;
use crate::sync_test_support::{signed_chain, temp_storage, test_pool, unique};
use crate::tests::support::require_test_db;
use crate::{ContentState, Settings, build_router};

/// A hang guard only: every wait below ends on an event. Also the readers' guard where a
/// test expects the fill's answer, so a slow run cannot turn it into the guard's miss.
const HANG_GUARD: Duration = Duration::from_secs(30);
/// Longer than any run, so no assertion depends on how long the run took.
const TEST_NEGATIVE_TTL: Duration = Duration::from_secs(24 * 3600);

#[derive(Default)]
struct Upstream {
    docs: Vec<Value>,
    chains: HashMap<String, Value>,
    blobs: HashMap<String, Vec<u8>>,
    active_calls: usize,
    audit_calls: usize,
    content_calls: usize,
}

#[derive(Clone)]
struct FakeState {
    upstream: Arc<parking_lot::Mutex<Upstream>>,
    entered: Arc<Notify>,
    /// `/entities/active` answers while this is open.
    gate: Arc<watch::Sender<bool>>,
}

struct Fake {
    state: FakeState,
    source: String,
}

async fn fake_active(State(f): State<FakeState>, Json(body): Json<Value>) -> Json<Value> {
    f.upstream.lock().active_calls += 1;
    f.entered.notify_waiters();
    let _ = f.gate.subscribe().wait_for(|open| *open).await;
    let wanted: Vec<String> = ["ids", "pointers"]
        .iter()
        .flat_map(|k| body[k].as_array().cloned().unwrap_or_default())
        .filter_map(|v| v.as_str().map(str::to_lowercase))
        .collect();
    let st = f.upstream.lock();
    let docs: Vec<Value> = st
        .docs
        .iter()
        .filter(|doc| {
            let id = doc["id"].as_str().unwrap_or_default().to_lowercase();
            wanted.contains(&id) || entity_pointers(doc).iter().any(|p| wanted.contains(p))
        })
        .cloned()
        .collect();
    Json(Value::Array(docs))
}

async fn fake_audit(
    State(f): State<FakeState>,
    Path((_, id)): Path<(String, String)>,
) -> axum::response::Response {
    let mut st = f.upstream.lock();
    st.audit_calls += 1;
    match st.chains.get(&id) {
        Some(chain) => Json(json!({"version": "v3", "authChain": chain})).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn fake_contents(
    State(f): State<FakeState>,
    Path(hash): Path<String>,
) -> axum::response::Response {
    let mut st = f.upstream.lock();
    st.content_calls += 1;
    match st.blobs.get(&hash) {
        Some(bytes) => (StatusCode::OK, bytes.clone()).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn start_fake() -> Fake {
    let state = FakeState {
        upstream: Default::default(),
        entered: Arc::new(Notify::new()),
        gate: Arc::new(watch::Sender::new(true)),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/content/entities/active", post(fake_active))
        .route("/content/audit/{type}/{id}", get(fake_audit))
        .route("/content/contents/{hash}", get(fake_contents))
        .with_state(state.clone());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Fake {
        state,
        source: format!("http://{addr}/content"),
    }
}

/// An entity with one content file, as a peer holds it.
struct Fixture {
    id: String,
    bytes: Vec<u8>,
    doc: Value,
    pointer: String,
    file_hash: String,
    file: Vec<u8>,
}

impl Fixture {
    fn new(entity_type: &str, pointer: &str, timestamp: i64) -> Self {
        Self::with_file(entity_type, pointer, timestamp, "model.glb")
    }

    fn with_file(entity_type: &str, pointer: &str, timestamp: i64, file_name: &str) -> Self {
        let file = format!("{file_name} of {pointer} {}", unique()).into_bytes();
        let file_hash = hash_bytes_v1(&file);
        let entity = json!({
            "version": "v3",
            "type": entity_type,
            "pointers": [pointer],
            "timestamp": timestamp,
            "content": [{"file": file_name, "hash": file_hash}],
            "metadata": {"name": pointer}
        });
        let bytes = serde_json::to_vec(&entity).unwrap();
        let id = hash_bytes_v1(&bytes);
        let mut doc = entity;
        doc["id"] = json!(id);
        Self {
            id,
            bytes,
            doc,
            pointer: pointer.to_string(),
            file_hash,
            file,
        }
    }
}

fn pointer() -> String {
    format!("urn:decentraland:off-chain:read-through:{}", unique())
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

impl Fake {
    fn publish(&self, fx: &Fixture, chain: &AuthChain) {
        let mut st = self.state.upstream.lock();
        st.docs.push(fx.doc.clone());
        st.chains
            .insert(fx.id.clone(), serde_json::to_value(chain).unwrap());
        st.blobs.insert(fx.id.clone(), fx.bytes.clone());
        st.blobs.insert(fx.file_hash.clone(), fx.file.clone());
    }

    fn active_calls(&self) -> usize {
        self.state.upstream.lock().active_calls
    }

    fn audit_calls(&self) -> usize {
        self.state.upstream.lock().audit_calls
    }

    fn content_calls(&self) -> usize {
        self.state.upstream.lock().content_calls
    }

    async fn until_active_calls(&self, n: usize) {
        tokio::time::timeout(HANG_GUARD, async {
            loop {
                let entered = self.state.entered.notified();
                if self.active_calls() >= n {
                    return;
                }
                entered.await;
            }
        })
        .await
        .expect("hang guard: upstream never asked");
    }
}

struct Node {
    state: Arc<ContentState>,
    read_through: Arc<ReadThrough>,
    storage: Arc<Storage>,
    pool: PgPool,
    tmp: std::path::PathBuf,
}

impl Node {
    /// `None` (skip) without CATALYST_TEST_DATABASE_URL. `wait` is the reader's guard.
    async fn new(tag: &str, fake: &Fake, wait: Duration, attach: bool) -> Option<Self> {
        Self::with_types(tag, fake, wait, attach, None).await
    }

    async fn with_types(
        tag: &str,
        fake: &Fake,
        wait: Duration,
        attach: bool,
        entity_types: Option<&[&str]>,
    ) -> Option<Self> {
        Self::with_sources(tag, fake, wait, attach, entity_types, Vec::new()).await
    }

    async fn with_sources(
        tag: &str,
        fake: &Fake,
        wait: Duration,
        attach: bool,
        entity_types: Option<&[&str]>,
        file_sources: Vec<String>,
    ) -> Option<Self> {
        let pool = test_pool(8).await?;
        let (storage, tmp) = temp_storage(tag).await;
        let state = Arc::new(ContentState::new(
            pool.clone(),
            Settings::for_tests(),
            storage.clone(),
        ));
        let read_through = Arc::new(ReadThrough::new(
            pool.clone(),
            storage.clone(),
            reqwest::Client::new(),
            vec![fake.source.clone()],
            entity_types.map(|types| types.iter().map(|t| t.to_string()).collect()),
            Fills::with_negative_ttl(4, wait, TEST_NEGATIVE_TTL),
        )
        .with_file_sources(file_sources));
        if attach {
            assert!(state.attach_read_through(read_through.clone()));
        }
        Some(Self {
            state,
            read_through,
            storage,
            pool,
            tmp,
        })
    }

    async fn active(&self, body: Value) -> Vec<String> {
        let resp = build_router(self.state.clone())
            .oneshot(
                Request::post("/content/entities/active")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let docs: Vec<Value> = serde_json::from_slice(&bytes).unwrap();
        docs.iter()
            .map(|d| d["id"].as_str().unwrap().to_string())
            .collect()
    }

    async fn get(&self, path: &str) -> (StatusCode, Vec<u8>) {
        let resp = build_router(self.state.clone())
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, bytes.to_vec())
    }

    async fn deployments(&self, id: &str) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM content.deployments WHERE entity_id = $1")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn holds(&self, hash: &str) -> bool {
        self.storage.exist(hash).await.unwrap()
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.tmp);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_miss_is_copied_verified_as_a_synced_deployment_then_served_locally() {
    let fake = start_fake().await;
    require_test_db!(Some(node) = Node::new("hit", &fake, HANG_GUARD, true).await);
    let by_pointer = Fixture::new("wearable", &pointer(), now_ms());
    let by_id = Fixture::new("wearable", &pointer(), now_ms());
    fake.publish(&by_pointer, &signed_chain(&by_pointer.id).await);
    fake.publish(&by_id, &signed_chain(&by_id.id).await);

    let ids = node
        .active(json!({"pointers": [by_pointer.pointer.to_uppercase()]}))
        .await;
    assert_eq!(ids, vec![by_pointer.id.clone()]);
    let ids = node.active(json!({"ids": [by_id.id]})).await;
    assert_eq!(ids, vec![by_id.id.clone()]);

    // Stored as sync stores it: the row, its pointer, the files; nothing marks it local.
    let (deployer, pointers, auth_chain): (String, Vec<String>, Value) = sqlx::query_as(
        "SELECT deployer_address, entity_pointers, auth_chain FROM content.deployments WHERE entity_id = $1",
    )
    .bind(&by_pointer.id)
    .fetch_one(&node.pool)
    .await
    .unwrap();
    let chain = signed_chain(&by_pointer.id).await;
    assert_eq!(deployer, chain[0].payload);
    assert_eq!(pointers, vec![by_pointer.pointer.clone()]);
    assert_eq!(auth_chain, serde_json::to_value(&chain).unwrap());
    let local: i64 =
        sqlx::query_scalar("SELECT count(*) FROM content.local_entities WHERE entity_id = $1")
            .bind(&by_pointer.id)
            .fetch_one(&node.pool)
            .await
            .unwrap();
    assert_eq!(local, 0);
    let active: String =
        sqlx::query_scalar("SELECT entity_id FROM content.active_pointers WHERE pointer = $1")
            .bind(&by_pointer.pointer)
            .fetch_one(&node.pool)
            .await
            .unwrap();
    assert_eq!(active, by_pointer.id);
    assert!(node.holds(&by_pointer.id).await && node.holds(&by_pointer.file_hash).await);

    // Served locally from now on: no further upstream call.
    let (calls, content_calls) = (fake.active_calls(), fake.content_calls());
    assert_eq!(calls, 2);
    let (status, body) = node
        .get(&format!(
            "/content/entities/wearable?pointer={}",
            urlencoding::encode(&by_pointer.pointer)
        ))
        .await;
    assert_eq!(status, StatusCode::OK);
    let docs: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(docs[0]["id"], by_pointer.id);
    assert_eq!(
        node.active(json!({"ids": [by_pointer.id, by_id.id]}))
            .await
            .len(),
        2
    );
    let (status, body) = node
        .get(&format!("/content/contents/{}", by_id.file_hash))
        .await;
    assert_eq!((status, body), (StatusCode::OK, by_id.file.clone()));
    assert_eq!(
        (fake.active_calls(), fake.content_calls()),
        (calls, content_calls)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_profile_past_the_sync_age_limit_is_reachable_through_the_profile_lookup() {
    let fake = start_fake().await;
    require_test_db!(Some(node) = Node::new("profile", &fake, HANG_GUARD, false).await);
    let address = format!("0x{}{}", unique(), &unique()[..8]);
    // 2020-01-01: older than any SYNC_PROFILE_MAX_AGE_DAYS a sync would have kept.
    let profile = Fixture::new("profile", &address, 1_577_836_800_000);
    fake.publish(&profile, &signed_chain(&profile.id).await);

    // The call `/lambdas/profiles` makes (the shop's profile reads).
    let docs = node
        .read_through
        .active_docs(false, std::slice::from_ref(&address))
        .await
        .unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].id, profile.id);
    assert_eq!(node.deployments(&profile.id).await, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_type_outside_sync_entity_types_is_not_copied() {
    let fake = start_fake().await;
    require_test_db!(
        Some(node) = Node::with_types(
            "types",
            &fake,
            HANG_GUARD,
            true,
            Some(&["scene", "profile"]),
        )
        .await
    );
    let wearable = Fixture::new("wearable", &pointer(), now_ms());
    fake.publish(&wearable, &signed_chain(&wearable.id).await);

    assert!(
        node.active(json!({"pointers": [wearable.pointer]}))
            .await
            .is_empty()
    );
    assert!(node.active(json!({"ids": [wearable.id]})).await.is_empty());
    assert_eq!(fake.active_calls(), 2);
    assert_eq!((fake.audit_calls(), fake.content_calls()), (0, 0));
    assert_eq!(node.deployments(&wearable.id).await, 0);
    assert!(!node.holds(&wearable.id).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_entity_file_that_does_not_hash_to_its_id_is_refused_and_nothing_is_stored() {
    let fake = start_fake().await;
    require_test_db!(Some(node) = Node::new("forged-hash", &fake, HANG_GUARD, true).await);
    let fx = Fixture::new("wearable", &pointer(), now_ms());
    fake.publish(&fx, &signed_chain(&fx.id).await);
    let mut forged = fx.doc.clone();
    forged["metadata"]["name"] = json!("forged");
    fake.state
        .upstream
        .lock()
        .blobs
        .insert(fx.id.clone(), serde_json::to_vec(&forged).unwrap());

    assert!(
        node.active(json!({"pointers": [fx.pointer]}))
            .await
            .is_empty()
    );
    assert_eq!(fake.active_calls(), 1);
    assert_eq!(node.deployments(&fx.id).await, 0);
    assert!(!node.holds(&fx.id).await);
    assert!(!node.holds(&fx.file_hash).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_entity_whose_signature_does_not_verify_is_refused_and_nothing_is_stored() {
    let fake = start_fake().await;
    require_test_db!(Some(node) = Node::new("forged-sig", &fake, HANG_GUARD, true).await);
    let other = Fixture::new("wearable", &pointer(), now_ms());
    let fx = Fixture::new("wearable", &pointer(), now_ms());
    // A real signature, over another entity.
    fake.publish(&fx, &signed_chain(&other.id).await);

    assert!(node.active(json!({"ids": [fx.id]})).await.is_empty());
    assert_eq!(fake.active_calls(), 1);
    assert_eq!(node.deployments(&fx.id).await, 0);
    assert!(!node.holds(&fx.id).await);
    assert!(!node.holds(&fx.file_hash).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_content_file_that_does_not_hash_to_its_name_is_refused() {
    let fake = start_fake().await;
    require_test_db!(Some(node) = Node::new("forged-file", &fake, HANG_GUARD, true).await);
    let fx = Fixture::new("wearable", &pointer(), now_ms());
    fake.publish(&fx, &signed_chain(&fx.id).await);
    fake.state
        .upstream
        .lock()
        .blobs
        .insert(fx.file_hash.clone(), b"not the model".to_vec());

    assert!(
        node.active(json!({"pointers": [fx.pointer]}))
            .await
            .is_empty()
    );
    assert_eq!(node.deployments(&fx.id).await, 0);
    assert!(!node.holds(&fx.file_hash).await);
    let (status, _) = node
        .get(&format!("/content/contents/{}", fx.file_hash))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(!node.holds(&fx.file_hash).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_content_miss_is_fetched_verified_and_then_served_locally() {
    let fake = start_fake().await;
    require_test_db!(Some(node) = Node::new("blob", &fake, HANG_GUARD, true).await);
    let face = format!("face {}", unique()).into_bytes();
    let hash = hash_bytes_v1(&face);
    fake.state
        .upstream
        .lock()
        .blobs
        .insert(hash.clone(), face.clone());

    let (status, body) = node.get(&format!("/content/contents/{hash}")).await;
    assert_eq!((status, body), (StatusCode::OK, face.clone()));
    assert!(node.holds(&hash).await);
    let (status, body) = node.get(&format!("/content/contents/{hash}")).await;
    assert_eq!((status, body), (StatusCode::OK, face));
    assert_eq!(fake.content_calls(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_file_the_peers_lack_is_read_from_a_file_source_which_is_never_asked_for_entities() {
    let peers = start_fake().await;
    let worlds = start_fake().await;
    require_test_db!(
        Some(node) = Node::with_sources(
            "file-source",
            &peers,
            HANG_GUARD,
            true,
            None,
            vec![worlds.source.clone()]
        )
        .await
    );
    let model = format!("world model {}", unique()).into_bytes();
    let hash = hash_bytes_v1(&model);
    worlds
        .state
        .upstream
        .lock()
        .blobs
        .insert(hash.clone(), model.clone());

    let (status, body) = node.get(&format!("/content/contents/{hash}")).await;
    assert_eq!((status, body), (StatusCode::OK, model));
    assert!(node.holds(&hash).await);
    assert_eq!((peers.content_calls(), worlds.content_calls()), (1, 1));

    // A file that does not hash to its name is refused from a file source as from a peer.
    let planted = hash_bytes_v1(format!("asked {}", unique()).as_bytes());
    worlds
        .state
        .upstream
        .lock()
        .blobs
        .insert(planted.clone(), b"something else".to_vec());
    let (status, _) = node.get(&format!("/content/contents/{planted}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(!node.holds(&planted).await);

    // Entities are asked of the peers alone.
    let fx = Fixture::new("wearable", &pointer(), now_ms());
    worlds.publish(&fx, &signed_chain(&fx.id).await);
    assert!(
        node.active(json!({"pointers": [fx.pointer]}))
            .await
            .is_empty()
    );
    assert_eq!((worlds.active_calls(), worlds.audit_calls()), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_profile_render_sync_skips_stays_a_miss_for_the_content_route() {
    let fake = start_fake().await;
    require_test_db!(Some(node) = Node::new("render", &fake, HANG_GUARD, true).await);
    let address = format!("0x{}{}", unique(), &unique()[..8]);
    let profile = Fixture::with_file("profile", &address, now_ms(), "face256.png");
    fake.publish(&profile, &signed_chain(&profile.id).await);
    let docs = node
        .read_through
        .active_docs(false, std::slice::from_ref(&address))
        .await
        .unwrap();
    assert_eq!(docs.len(), 1);
    assert!(
        !node.holds(&profile.file_hash).await,
        "sync skips the render"
    );

    let content_calls = fake.content_calls();
    let (status, _) = node
        .get(&format!("/content/contents/{}", profile.file_hash))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(fake.content_calls(), content_calls);
    assert!(!node.holds(&profile.file_hash).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_content_fill_waits_for_a_shared_download_slot() {
    let fake = start_fake().await;
    require_test_db!(Some(node) = Node::new("slots", &fake, HANG_GUARD, true).await);
    let node = Arc::new(node);
    let file = format!("file {}", unique()).into_bytes();
    let hash = hash_bytes_v1(&file);
    fake.state
        .upstream
        .lock()
        .blobs
        .insert(hash.clone(), file.clone());
    let downloads = node.read_through.downloads().clone();
    let mut taken = Vec::new();
    for _ in 0..MAX_CONCURRENT_DOWNLOADS {
        taken.push(downloads.slot().await.unwrap());
    }

    let read = {
        let (node, hash) = (node.clone(), hash.clone());
        tokio::spawn(async move { node.get(&format!("/content/contents/{hash}")).await })
    };
    tokio::time::timeout(HANG_GUARD, downloads.until_waited(1))
        .await
        .expect("hang guard: the fill never waited for a slot");
    assert_eq!(fake.content_calls(), 0);
    drop(taken);
    assert_eq!(read.await.unwrap(), (StatusCode::OK, file));
    assert_eq!(fake.content_calls(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_miss_the_sources_cannot_fill_is_not_asked_again_while_cached() {
    let fake = start_fake().await;
    require_test_db!(Some(node) = Node::new("negative", &fake, HANG_GUARD, true).await);
    let fx = Fixture::new("wearable", &pointer(), now_ms());

    assert!(
        node.active(json!({"pointers": [fx.pointer]}))
            .await
            .is_empty()
    );
    assert_eq!(fake.active_calls(), 1);
    // The upstream gets it now; the cached miss still answers without asking.
    fake.publish(&fx, &signed_chain(&fx.id).await);
    assert!(
        node.active(json!({"pointers": [fx.pointer]}))
            .await
            .is_empty()
    );
    assert_eq!(fake.active_calls(), 1);

    let missing = hash_bytes_v1(format!("absent {}", unique()).as_bytes());
    for _ in 0..2 {
        let (status, _) = node.get(&format!("/content/contents/{missing}")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
    assert_eq!(fake.content_calls(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn without_read_through_a_miss_answers_as_before_and_nothing_is_asked() {
    let fake = start_fake().await;
    require_test_db!(Some(node) = Node::new("island", &fake, HANG_GUARD, false).await);
    let fx = Fixture::new("wearable", &pointer(), now_ms());
    fake.publish(&fx, &signed_chain(&fx.id).await);

    assert!(
        node.active(json!({"pointers": [fx.pointer]}))
            .await
            .is_empty()
    );
    assert!(node.active(json!({"ids": [fx.id]})).await.is_empty());
    let (status, _) = node
        .get(&format!("/content/contents/{}", fx.file_hash))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!((fake.active_calls(), fake.content_calls()), (0, 0));
    assert_eq!(node.deployments(&fx.id).await, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_readers_of_one_miss_make_one_upstream_call_and_store_once() {
    let fake = start_fake().await;
    require_test_db!(Some(node) = Node::new("race", &fake, HANG_GUARD, true).await);
    let node = Arc::new(node);
    let fx = Fixture::new("wearable", &pointer(), now_ms());
    fake.publish(&fx, &signed_chain(&fx.id).await);
    fake.state.gate.send_replace(false);

    let read = |node: Arc<Node>, pointer: String| {
        tokio::spawn(async move { node.active(json!({"pointers": [pointer]})).await })
    };
    let first = read(node.clone(), fx.pointer.clone());
    fake.until_active_calls(1).await;
    let second = read(node.clone(), fx.pointer.clone());
    tokio::time::timeout(
        HANG_GUARD,
        node.read_through.fills().until(|c| c.joined >= 1),
    )
    .await
    .expect("hang guard: the second reader never joined");
    fake.state.gate.send_replace(true);

    assert_eq!(first.await.unwrap(), vec![fx.id.clone()]);
    assert_eq!(second.await.unwrap(), vec![fx.id.clone()]);
    assert_eq!(fake.active_calls(), 1);
    assert_eq!(node.read_through.fills().counts().started, 1);
    assert_eq!(node.deployments(&fx.id).await, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_reader_answers_the_miss_at_the_guard_and_the_fill_still_lands() {
    let fake = start_fake().await;
    require_test_db!(Some(node) = Node::new("guard", &fake, Duration::from_millis(50), true).await);
    let fx = Fixture::new("wearable", &pointer(), now_ms());
    fake.publish(&fx, &signed_chain(&fx.id).await);
    fake.state.gate.send_replace(false);

    // The upstream holds until after this answer, so the answer is the guard's.
    assert!(
        node.active(json!({"pointers": [fx.pointer]}))
            .await
            .is_empty()
    );
    fake.state.gate.send_replace(true);
    tokio::time::timeout(
        HANG_GUARD,
        node.read_through.fills().until(|c| c.finished >= 1),
    )
    .await
    .expect("hang guard: the fill never finished");

    assert_eq!(
        node.active(json!({"pointers": [fx.pointer]})).await,
        vec![fx.id.clone()]
    );
    assert_eq!(fake.active_calls(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_negative_entry_expires_after_its_ttl() {
    let fills = Fills::new(1, Duration::from_secs(1));
    let runs = Arc::new(AtomicU64::new(0));
    let fill = |runs: Arc<AtomicU64>| {
        move |_: Vec<String>| async move {
            runs.fetch_add(1, Ordering::SeqCst);
        }
    };

    assert!(fills.fill(vec!["k".into()], fill(runs.clone())).await);
    fills.until(|c| c.finished == 1).await;
    assert!(!fills.fill(vec!["k".into()], fill(runs.clone())).await);
    tokio::time::advance(NEGATIVE_TTL - Duration::from_secs(1)).await;
    assert!(!fills.fill(vec!["k".into()], fill(runs.clone())).await);
    tokio::time::advance(Duration::from_secs(2)).await;
    assert!(fills.fill(vec!["k".into()], fill(runs.clone())).await);
    fills.until(|c| c.finished == 2).await;
    assert_eq!(runs.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn fills_beyond_the_bound_answer_the_miss_without_starting() {
    let fills = Fills::new(1, Duration::from_secs(1));
    let (open, mut held) = watch::channel(false);
    let first = {
        let fills = fills.clone();
        tokio::spawn(async move {
            fills
                .fill(vec!["a".into()], move |_| async move {
                    let _ = held.wait_for(|open| *open).await;
                })
                .await
        })
    };
    fills.until(|c| c.started == 1).await;
    assert!(!fills.fill(vec!["b".into()], |_| async {}).await);
    assert_eq!(fills.counts().saturated, 1);
    open.send_replace(true);
    assert!(first.await.unwrap());
}

#[tokio::test]
async fn read_through_is_off_by_default_and_needs_a_source() {
    let (storage, tmp) = temp_storage("config").await;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://nobody@127.0.0.1:1/nowhere")
        .unwrap();
    let build = |args: &[&str]| {
        let mut argv = vec!["x", "--database-url", "postgres:///c"];
        argv.extend_from_slice(args);
        let config = crate::config::Config::parse_for_tests(&argv).unwrap();
        ReadThrough::from_config(
            &config,
            &["https://this.node.example".to_string()],
            pool.clone(),
            storage.clone(),
            reqwest::Client::new(),
        )
    };

    assert!(build(&[]).unwrap().is_none());
    assert!(
        build(&["--read-through-enabled", "true", "--sync-source", ""]).is_err(),
        "on without a source is refused"
    );
    assert!(
        build(&[
            "--read-through-enabled",
            "true",
            "--sync-source",
            "https://this.node.example/content"
        ])
        .is_err(),
        "a source naming this node is refused"
    );
    let on = build(&[
        "--read-through-enabled",
        "true",
        "--sync-source",
        "https://peer.example/content,http://127.0.0.1:5140",
    ])
    .unwrap()
    .expect("on");
    assert_eq!(
        on.upstream.sources,
        vec![
            "https://peer.example/content".to_string(),
            "http://127.0.0.1:5140".to_string()
        ]
    );
    assert!(on.upstream.file_sources.is_empty());
    let peer = ["--read-through-enabled", "true", "--sync-source", "https://peer.example/content"];
    let with_worlds = |worlds: &str| {
        let mut args = peer.to_vec();
        args.extend(["--worlds-content-mirror-source", worlds]);
        build(&args)
    };
    let on = with_worlds("https://worlds.example/").unwrap().expect("on");
    assert_eq!(on.upstream.file_sources, vec!["https://worlds.example".to_string()]);
    assert!(
        with_worlds("https://this.node.example").is_err(),
        "a worlds source naming this node is refused"
    );
    let _ = std::fs::remove_dir_all(tmp);
}
