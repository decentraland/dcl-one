//! A world's read-through against an in-process worlds content server (`/world/{name}/about`,
//! `/world/{name}/scenes`, `/contents/{hash}`) and marketplace subgraph (`POST /subgraph`, the
//! NAMEs each address holds) on a scratch database and a temp blob store: a world held with no
//! scene is mirrored on its first read through the mirror's single-world path, with the chain
//! and signer it was published with and only the files the scene host reads (the rest are left to
//! the content read-through), then served locally; a world the upstream lacks is asked
//! once; a file that does not hash to its name, a chain that does not sign its scene and a
//! signer who may not publish the world each store nothing; a malformed name or an ENS world is
//! never asked about; a local or blocked world is left alone, even one that turns so mid-fill;
//! without read-through a miss stays a miss. Needs CATALYST_TEST_DATABASE_URL; skipped without
//! it.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use alloy::signers::{Signer, local::PrivateKeySigner};
use axum::body::Body;
use axum::extract::{Path, Query, RawQuery, State};
use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use parking_lot::Mutex;
use serde_json::{Value, json};
use tower::ServiceExt;

use catalyst::auth::hash::hash_bytes_v1;
use catalyst::chain_index::{ChainIndex, IndexSettings};
use catalyst::content::deploy_remote_entity::ByteBudget;
use catalyst::content::read_through::{Downloads, Fills, MAX_CONCURRENT_DOWNLOADS};
use catalyst::storage::Storage;
use catalyst::worlds::mirror::MirrorSettings;
use catalyst::worlds::policy::{PersonalWorldsPolicy, Policy};
use catalyst::worlds::{Settings, WorldReadThrough, WorldsState};

#[path = "support/db.rs"]
mod db;

/// The readers' guard: longer than any fill here, so the answer is the fill's.
const WAIT: Duration = Duration::from_secs(30);
/// Longer than any run, so no assertion depends on how long the run took.
const NEGATIVE_TTL: Duration = Duration::from_secs(24 * 3600);
/// The NAME holder every world here is published by.
const OWNER_KEY: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
/// A signer who holds no NAME.
const INTRUDER_KEY: &str = "59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";

fn address(key: &str) -> String {
    format!("{:#x}", key.parse::<PrivateKeySigner>().unwrap().address())
}

/// A SIGNER + ECDSA_SIGNED_ENTITY chain naming `entity_id`, its signature by `key` over
/// `signed_id` (the entity id itself unless the chain is forged).
async fn chain(key: &str, entity_id: &str, signed_id: &str) -> Value {
    let key: PrivateKeySigner = key.parse().unwrap();
    let signature = key.sign_message(signed_id.as_bytes()).await.unwrap();
    json!([
        {"type": "SIGNER", "payload": format!("{:#x}", key.address())},
        {"type": "ECDSA_SIGNED_ENTITY", "payload": entity_id, "signature": signature.to_string()}
    ])
}

#[derive(Default)]
struct Upstream {
    /// world -> scene entity ids.
    worlds: HashMap<String, Vec<String>>,
    blobs: HashMap<String, Vec<u8>>,
    about_calls: HashMap<String, usize>,
    /// entity id -> the deployment auth chain `/scenes` lists.
    chains: HashMap<String, Value>,
    /// address -> the NAME labels the subgraph says it holds.
    names: HashMap<String, Vec<String>>,
    content_calls: HashMap<String, usize>,
    /// The query string of every `/scenes` request.
    scenes_queries: Vec<String>,
    /// Holds the next `/scenes` request until the test releases it.
    gate: Option<Arc<Gate>>,
    /// Every request the upstream answered, whatever its path.
    requests: usize,
    /// Requests to a path the fake does not route (the catalogue `/index` among them).
    unrouted: usize,
    /// world -> its `/permissions` answer; a world without one answers 500.
    permissions: HashMap<String, Value>,
    permissions_calls: usize,
}

type Shared = Arc<Mutex<Upstream>>;

#[derive(Default)]
struct Gate {
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}

async fn fake_about(
    State((s, base)): State<(Shared, String)>,
    Path(name): Path<String>,
) -> axum::response::Response {
    let mut st = s.lock();
    st.requests += 1;
    *st.about_calls.entry(name.clone()).or_default() += 1;
    let Some(scenes) = st.worlds.get(&name).filter(|s| !s.is_empty()) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let urns: Vec<String> = scenes
        .iter()
        .map(|id| format!("urn:decentraland:entity:{id}?=&baseUrl={base}/contents/"))
        .collect();
    Json(json!({
        "healthy": true, "acceptingUsers": true, "spawnCoordinates": "0,0",
        "configurations": {"scenesUrn": urns},
        "comms": {"adapter": "fixed-adapter:signed-login:https://x/comms"}
    }))
    .into_response()
}

#[derive(serde::Deserialize)]
struct Page {
    limit: Option<usize>,
    offset: Option<usize>,
    entity_id: Option<String>,
}

async fn fake_scenes(
    State((s, _)): State<(Shared, String)>,
    Path(name): Path<String>,
    Query(page): Query<Page>,
    RawQuery(raw): RawQuery,
) -> axum::response::Response {
    let gate = s.lock().gate.take();
    if let Some(gate) = gate {
        gate.entered.notify_one();
        gate.release.notified().await;
    }
    let mut st = s.lock();
    st.requests += 1;
    st.scenes_queries.push(raw.unwrap_or_default());
    let ids: Vec<String> = st
        .worlds
        .get(&name)
        .into_iter()
        .flatten()
        .filter(|id| page.entity_id.as_ref().is_none_or(|wanted| wanted == *id))
        .cloned()
        .collect();
    let scenes: Vec<Value> = ids
        .iter()
        .skip(page.offset.unwrap_or(0))
        .take(page.limit.unwrap_or(100))
        .map(|id| {
            json!({
                "worldName": name, "entityId": id,
                "deploymentAuthChain": st.chains.get(id).cloned().unwrap_or(json!([])),
                "size": LISTED_SIZE,
            })
        })
        .collect();
    Json(json!({"scenes": scenes, "total": ids.len()})).into_response()
}

async fn fake_subgraph(
    State((s, _)): State<(Shared, String)>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let mut st = s.lock();
    st.requests += 1;
    let owner = body["variables"]["owner"].as_str().unwrap_or_default();
    let first_page = body["variables"]["idFrom"]
        .as_str()
        .unwrap_or_default()
        .is_empty();
    let nfts: Vec<Value> = match st.names.get(owner) {
        Some(names) if first_page => names
            .iter()
            .map(|n| json!({"id": format!("name-{n}"), "name": n}))
            .collect(),
        _ => Vec::new(),
    };
    Json(json!({"data": {"nfts": nfts}}))
}

async fn fake_contents(
    State((s, _)): State<(Shared, String)>,
    Path(hash): Path<String>,
) -> axum::response::Response {
    let mut st = s.lock();
    st.requests += 1;
    *st.content_calls.entry(hash.clone()).or_default() += 1;
    match st.blobs.get(&hash) {
        Some(bytes) => (StatusCode::OK, bytes.clone()).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn fake_permissions(
    State((s, _)): State<(Shared, String)>,
    Path(name): Path<String>,
) -> axum::response::Response {
    let mut st = s.lock();
    st.requests += 1;
    st.permissions_calls += 1;
    match st.permissions.get(&name) {
        Some(body) => Json(body.clone()).into_response(),
        None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// The upstream's `/permissions` for a world: `owner` and the deployment allow-list.
fn permissions(owner: &str, deployers: &[&str]) -> Value {
    json!({
        "permissions": {
            "deployment": {"type": "allow-list", "wallets": deployers},
            "access": {"type": "unrestricted"}
        },
        "owner": owner
    })
}

async fn fake_other(State((s, _)): State<(Shared, String)>) -> StatusCode {
    let mut st = s.lock();
    st.requests += 1;
    st.unrouted += 1;
    StatusCode::NOT_FOUND
}

async fn start_fake() -> (Shared, String) {
    let state: Shared = Arc::new(Mutex::new(Upstream::default()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let app = Router::new()
        .route("/world/{name}/about", get(fake_about))
        .route("/world/{name}/scenes", get(fake_scenes))
        .route("/world/{name}/permissions", get(fake_permissions))
        .route("/subgraph", post(fake_subgraph))
        .route("/contents/{hash}", get(fake_contents))
        .fallback(fake_other)
        .with_state((state.clone(), base.clone()));
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (state, base)
}

/// The size the fake `/scenes` lists for every scene, as the upstream does (a string).
const LISTED_SIZE: &str = "4242";

/// The bytes of the asset every [`add_world`] scene carries beside its main file.
fn asset_of(world: &str) -> Vec<u8> {
    format!("model.glb of {world}").into_bytes()
}

/// One scene with a main file and one asset, signed by the holder of the world's NAME (the personal world's
/// own address); returns (entity id, file hash).
async fn add_world(fake: &Shared, world: &str) -> (String, String) {
    let owner = world
        .strip_suffix(".dcl.eth")
        .filter(|label| label.starts_with("0x"))
        .map_or(OWNER_KEY, |_| INTRUDER_KEY);
    let file = format!("main.js of {world}").into_bytes();
    let file_hash = hash_bytes_v1(&file);
    let asset = asset_of(world);
    let asset_hash = hash_bytes_v1(&asset);
    let entity = json!({
        "version": "v3",
        "type": "scene",
        "pointers": ["0,0"],
        "timestamp": 1_700_000_000_000i64,
        "content": [
            {"file": "main.js", "hash": file_hash},
            {"file": "model.glb", "hash": asset_hash}
        ],
        "metadata": {
            "main": "main.js",
            "display": {"title": world},
            "scene": {"base": "0,0", "parcels": ["0,0"]},
            "worldConfiguration": {"name": world}
        }
    });
    let bytes = serde_json::to_vec(&entity).unwrap();
    let id = hash_bytes_v1(&bytes);
    let signed = chain(owner, &id, &id).await;
    let mut st = fake.lock();
    st.blobs.insert(file_hash.clone(), file);
    st.blobs.insert(asset_hash, asset);
    st.blobs.insert(id.clone(), bytes);
    st.worlds.insert(world.to_string(), vec![id.clone()]);
    st.chains.insert(id.clone(), signed);
    if let Some(label) = world.strip_suffix(".dcl.eth") {
        st.names
            .entry(address(OWNER_KEY))
            .or_default()
            .push(label.to_string());
    }
    (id, file_hash)
}

struct Node {
    state: Arc<WorldsState>,
    storage: Arc<Storage>,
    tmp: std::path::PathBuf,
}

impl Node {
    async fn new(pool: sqlx::PgPool, source: &str, tag: &str, read_through: bool) -> Self {
        let downloads = Downloads::new(
            MAX_CONCURRENT_DOWNLOADS,
            Arc::new(ByteBudget::new(1024 * 1024 * 1024)),
        );
        Self::with_downloads(pool, source, tag, read_through.then_some(downloads)).await
    }

    /// A read-through node under `WORLDS_READ_THROUGH_TRUST_UPSTREAM_GRANTS`.
    async fn trusting_grants(pool: sqlx::PgPool, source: &str, tag: &str) -> Self {
        let downloads = Downloads::new(
            MAX_CONCURRENT_DOWNLOADS,
            Arc::new(ByteBudget::new(1024 * 1024 * 1024)),
        );
        let settings = MirrorSettings::for_tests(source)
            .read_through_only()
            .trusting_upstream_grants(true);
        Self::with_settings(pool, settings, source, tag, Some(downloads)).await
    }

    async fn with_downloads(
        pool: sqlx::PgPool,
        source: &str,
        tag: &str,
        downloads: Option<Downloads>,
    ) -> Self {
        // The read-through alone, as a node without WORLDS_CONTENT_MIRROR_ENABLED runs it.
        let settings = MirrorSettings::for_tests(source).read_through_only();
        Self::with_settings(pool, settings, source, tag, downloads).await
    }

    async fn with_settings(
        pool: sqlx::PgPool,
        settings: MirrorSettings,
        source: &str,
        tag: &str,
        downloads: Option<Downloads>,
    ) -> Self {
        let tmp = std::env::temp_dir().join(format!(
            "catalyst-worlds-rt-{tag}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let storage = Arc::new(Storage::from_config(&tmp).await.unwrap());
        let index = Arc::new(ChainIndex::new(
            catalyst::http::Outbound::for_tests(),
            IndexSettings {
                marketplace_subgraph_url: Some(format!("{source}/subgraph")),
                ..IndexSettings::for_tests()
            },
        ));
        index.allow_loopback_for_tests();
        let policy = Arc::new(Policy::new(
            pool.clone(),
            PersonalWorldsPolicy::disabled(),
            Some(index),
        ));
        let state = Arc::new(WorldsState::new(
            pool,
            storage.clone(),
            Settings::for_tests("http://127.0.0.1:5210"),
            Some(settings),
            catalyst::http::Outbound::for_tests(),
        ));
        if let Some(downloads) = downloads {
            assert!(state.attach_read_through(WorldReadThrough {
                fills: Fills::with_negative_ttl(4, WAIT, NEGATIVE_TTL),
                downloads,
                policy,
            }));
        }
        Self {
            state,
            storage,
            tmp,
        }
    }

    async fn get(&self, path: &str) -> (StatusCode, Value) {
        let resp = catalyst::worlds::router(self.state.clone())
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    /// (chain, deployer) of each scene row.
    async fn signatures(&self, world: &str) -> Vec<(Value, String)> {
        sqlx::query_as(
            "SELECT deployment_auth_chain::jsonb, deployer FROM worlds.world_scenes WHERE world_name = lower($1) ORDER BY entity_id",
        )
        .bind(world)
        .fetch_all(&self.state.pool)
        .await
        .unwrap()
    }

    async fn scene_rows(&self, world: &str) -> Vec<(String, Option<String>)> {
        sqlx::query_as(
            "SELECT entity_id, mirror_source FROM worlds.world_scenes WHERE world_name = lower($1) ORDER BY entity_id",
        )
        .bind(world)
        .fetch_all(&self.state.pool)
        .await
        .unwrap()
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.tmp);
    }
}

fn about_calls(fake: &Shared, world: &str) -> usize {
    fake.lock().about_calls.get(world).copied().unwrap_or(0)
}

fn content_calls(fake: &Shared, hash: &str) -> usize {
    fake.lock().content_calls.get(hash).copied().unwrap_or(0)
}

async fn world_rows(pool: &sqlx::PgPool) -> (i64, i64) {
    sqlx::query_as(
        "SELECT (SELECT count(*) FROM worlds.worlds), (SELECT count(*) FROM worlds.world_scenes)",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_world_not_held_is_mirrored_on_first_read_then_served_locally() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let (fake, base) = start_fake().await;
    let node = Node::new(scratch.pool.clone(), &base, "hit", true).await;
    let (id, file_hash) = add_world(&fake, "rt.dcl.eth").await;

    let (status, about) = node.get("/world/RT.dcl.eth/about").await;
    assert_eq!(status, StatusCode::OK, "{about}");
    let urns = about["configurations"]["scenesUrn"].to_string();
    assert!(urns.contains(&id), "{urns}");
    assert_eq!(
        node.scene_rows("rt.dcl.eth").await,
        vec![(id.clone(), Some(base.clone()))]
    );
    assert!(node.storage.exist(&id).await.unwrap());
    assert!(node.storage.exist(&file_hash).await.unwrap());
    // Only the scene host's files are stored; an asset is read through when first asked for.
    let asset_hash = hash_bytes_v1(&asset_of("rt.dcl.eth"));
    assert!(!node.storage.exist(&asset_hash).await.unwrap());
    assert_eq!(content_calls(&fake, &asset_hash), 0);
    let published = fake.lock().chains[&id].clone();
    assert_eq!(
        node.signatures("rt.dcl.eth").await,
        vec![(published.clone(), address(OWNER_KEY))]
    );

    let (status, scenes) = node.get("/world/rt.dcl.eth/scenes").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(scenes["total"], 1);
    assert_eq!(scenes["scenes"][0]["entityId"], id);
    assert_eq!(scenes["scenes"][0]["deploymentAuthChain"], published);
    assert_eq!(scenes["scenes"][0]["deployer"], json!(address(OWNER_KEY)));
    assert_eq!(scenes["scenes"][0]["size"], LISTED_SIZE);
    assert_eq!(about_calls(&fake, "rt.dcl.eth"), 1);

    // A personal world is its own address's to publish, NAME or none.
    let personal = format!("{}.dcl.eth", address(INTRUDER_KEY));
    add_world(&fake, &personal).await;
    let (status, about) = node.get(&format!("/world/{personal}/about")).await;
    assert_eq!(status, StatusCode::OK, "{about}");
    assert_eq!(node.signatures(&personal).await[0].1, address(INTRUDER_KEY));

    // A world the upstream does not have: asked once, then the cached miss answers.
    for _ in 0..2 {
        let (status, _) = node.get("/world/nope.dcl.eth/about").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, scenes) = node.get("/world/nope.dcl.eth/scenes").await;
        assert_eq!(
            (status, scenes["total"].clone()),
            (StatusCode::OK, json!(0))
        );
    }
    assert_eq!(about_calls(&fake, "nope.dcl.eth"), 1);

    drop(node);
    scratch.teardown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_read_through_alone_reads_only_the_world_asked_for_and_runs_no_pass() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let (fake, base) = start_fake().await;
    let node = Node::new(scratch.pool.clone(), &base, "alone", true).await;
    let (id, file_hash) = add_world(&fake, "basiccontroller.dcl.eth").await;
    add_world(&fake, "other.dcl.eth").await;

    let (status, about) = node.get("/world/basiccontroller.dcl.eth/about").await;
    assert_eq!(status, StatusCode::OK, "{about}");
    assert!(node.storage.exist(&id).await.unwrap());
    assert!(node.storage.exist(&file_hash).await.unwrap());
    assert_eq!(about_calls(&fake, "other.dcl.eth"), 0);
    assert!(node.scene_rows("other.dcl.eth").await.is_empty());
    assert_eq!(fake.lock().unrouted, 0, "no catalogue /index was asked");

    let (status, report) = node.get("/worlds/status").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(report["mirror"]["enabled"], false);
    assert_eq!(report["mirror"]["source"], json!(base));
    assert_eq!(report["mirror"]["intervalSeconds"], Value::Null);

    drop(node);
    scratch.teardown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_world_file_that_does_not_hash_to_its_name_stores_nothing() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let (fake, base) = start_fake().await;
    let node = Node::new(scratch.pool.clone(), &base, "forged", true).await;
    let (_, file_hash) = add_world(&fake, "forged.dcl.eth").await;
    fake.lock()
        .blobs
        .insert(file_hash.clone(), b"planted".to_vec());

    let (status, _) = node.get("/world/forged.dcl.eth/about").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(node.scene_rows("forged.dcl.eth").await.is_empty());
    assert!(!node.storage.exist(&file_hash).await.unwrap());

    drop(node);
    scratch.teardown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn without_read_through_a_world_not_held_stays_a_miss() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let (fake, base) = start_fake().await;
    let node = Node::new(scratch.pool.clone(), &base, "island", false).await;
    add_world(&fake, "island.dcl.eth").await;

    let (status, _) = node.get("/world/island.dcl.eth/about").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, scenes) = node.get("/world/island.dcl.eth/scenes").await;
    assert_eq!(
        (status, scenes["total"].clone()),
        (StatusCode::OK, json!(0))
    );
    assert_eq!(about_calls(&fake, "island.dcl.eth"), 0);
    assert!(node.scene_rows("island.dcl.eth").await.is_empty());

    drop(node);
    scratch.teardown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_name_no_worlds_server_can_hold_is_never_asked_about() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let (fake, base) = start_fake().await;
    let node = Node::new(scratch.pool.clone(), &base, "probe", true).await;
    add_world(&fake, "rt.dcl.eth").await;

    for probe in [
        "rt.dcl.eth%5Cabout%3F0",
        "rt.dcl.eth%5Cabout%3F1",
        "rt.dcl.eth%5Cabout%3F2",
        "rt.dcl.eth%3Fx",
        "rt.dcl.eth%23x",
        "rt..eth",
        "..",
        "my-world.dcl.eth",
    ] {
        for route in ["about", "scenes"] {
            let (status, body) = node.get(&format!("/world/{probe}/{route}")).await;
            match route {
                "about" => assert_eq!(status, StatusCode::NOT_FOUND, "{probe}: {body}"),
                _ => assert_eq!((status, body["total"].clone()), (StatusCode::OK, json!(0))),
            }
        }
    }
    assert_eq!(fake.lock().requests, 0);
    assert_eq!(world_rows(&scratch.pool).await, (0, 0));

    drop(node);
    scratch.teardown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_world_scene_whose_chain_does_not_sign_it_stores_nothing() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let (fake, base) = start_fake().await;
    let node = Node::new(scratch.pool.clone(), &base, "unsigned", true).await;
    let (id, file_hash) = add_world(&fake, "unsigned.dcl.eth").await;
    // The owner's signature, but over another entity: the chain names this scene and does not
    // sign it.
    let forged = chain(OWNER_KEY, &id, &file_hash).await;
    fake.lock().chains.insert(id.clone(), forged);

    let (status, _) = node.get("/world/unsigned.dcl.eth/about").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(node.scene_rows("unsigned.dcl.eth").await.is_empty());
    assert!(!node.storage.exist(&id).await.unwrap());
    assert!(!node.storage.exist(&file_hash).await.unwrap());
    assert_eq!(content_calls(&fake, &file_hash), 0);
    assert_eq!(world_rows(&scratch.pool).await, (0, 0));

    drop(node);
    scratch.teardown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_world_scene_signed_by_one_who_may_not_publish_the_world_stores_nothing() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let (fake, base) = start_fake().await;
    let node = Node::new(scratch.pool.clone(), &base, "intruder", true).await;
    let (id, file_hash) = add_world(&fake, "taken.dcl.eth").await;
    // A valid signature by an address that holds no NAME.
    let intruder = chain(INTRUDER_KEY, &id, &id).await;
    fake.lock().chains.insert(id.clone(), intruder);

    let (status, _) = node.get("/world/taken.dcl.eth/about").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, scenes) = node.get("/world/taken.dcl.eth/scenes").await;
    assert_eq!(
        (status, scenes["total"].clone()),
        (StatusCode::OK, json!(0))
    );
    assert!(node.scene_rows("taken.dcl.eth").await.is_empty());
    assert!(!node.storage.exist(&id).await.unwrap());
    // Refused before the entity is fetched.
    assert_eq!(content_calls(&fake, &id), 0);
    assert_eq!(content_calls(&fake, &file_hash), 0);
    assert_eq!(world_rows(&scratch.pool).await, (0, 0));
    // Upstream grants are not trusted unless asked to be.
    assert_eq!(fake.lock().permissions_calls, 0);

    drop(node);
    scratch.teardown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_grantee_signed_world_is_taken_on_the_upstream_grant_only_when_its_owner_holds_the_name()
{
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let (fake, base) = start_fake().await;
    let node = Node::trusting_grants(scratch.pool.clone(), &base, "grants").await;
    let (owner, grantee) = (address(OWNER_KEY), address(INTRUDER_KEY));
    let stranger = "0x00000000000000000000000000000000000000dd";
    // Each world's one scene signed by the grantee, who holds no NAME; OWNER holds every label.
    let grantee_world = |world: &'static str, perms: Option<Value>| {
        let fake = fake.clone();
        async move {
            let (id, file_hash) = add_world(&fake, world).await;
            let signed = chain(INTRUDER_KEY, &id, &id).await;
            let mut st = fake.lock();
            st.chains.insert(id.clone(), signed);
            if let Some(perms) = perms {
                st.permissions.insert(world.to_string(), perms);
            }
            (id, file_hash)
        }
    };

    // The owner the upstream names holds the NAME here and lists the signer: accepted.
    let (id, _) = grantee_world(
        "granted.dcl.eth",
        Some(permissions(&owner, &[stranger, &grantee.to_uppercase().replace("0X", "0x")])),
    )
    .await;
    let (status, about) = node.get("/world/granted.dcl.eth/about").await;
    assert_eq!(status, StatusCode::OK, "{about}");
    assert!(node.storage.exist(&id).await.unwrap());
    let published = fake.lock().chains[&id].clone();
    assert_eq!(
        node.signatures("granted.dcl.eth").await,
        vec![(published, grantee.clone())]
    );

    // The upstream names an owner this node's NAME authority does not.
    let (id, _) = grantee_world(
        "otherowner.dcl.eth",
        Some(permissions(stranger, &[&grantee])),
    )
    .await;
    // The signer is not on the list.
    let (id2, _) = grantee_world(
        "unlisted.dcl.eth",
        Some(permissions(&owner, &[stranger])),
    )
    .await;
    // The permissions cannot be read.
    let (id3, _) = grantee_world("unreadable.dcl.eth", None).await;
    for (world, id) in [
        ("otherowner.dcl.eth", id),
        ("unlisted.dcl.eth", id2),
        ("unreadable.dcl.eth", id3),
    ] {
        let (status, _) = node.get(&format!("/world/{world}/about")).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{world}");
        assert!(node.scene_rows(world).await.is_empty(), "{world}");
        assert!(!node.storage.exist(&id).await.unwrap(), "{world}");
        assert_eq!(content_calls(&fake, &id), 0, "{world}");
    }
    // Asked once per fill, and a refused world is not asked again while its miss is remembered.
    let calls = fake.lock().permissions_calls;
    assert_eq!(calls, 4);
    let (status, _) = node.get("/world/unreadable.dcl.eth/about").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(fake.lock().permissions_calls, calls);

    drop(node);
    scratch.teardown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_world_fill_downloads_under_the_bound_content_fills_share() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let (fake, base) = start_fake().await;
    // One file in flight across every fill; the test holds it.
    let downloads = Downloads::new(1, Arc::new(ByteBudget::new(1024 * 1024 * 1024)));
    let node = Arc::new(
        Node::with_downloads(
            scratch.pool.clone(),
            &base,
            "bound",
            Some(downloads.clone()),
        )
        .await,
    );
    let (id, file_hash) = add_world(&fake, "bound.dcl.eth").await;
    let held = downloads.slot().await.unwrap();

    let read = {
        let node = node.clone();
        tokio::spawn(async move { node.get("/world/bound.dcl.eth/about").await })
    };
    tokio::time::timeout(WAIT, downloads.until_waited(1))
        .await
        .expect("hang guard: the world fill never waited for a download slot");
    assert_eq!(content_calls(&fake, &id), 0);
    drop(held);
    let (status, about) = read.await.unwrap();
    assert_eq!(status, StatusCode::OK, "{about}");
    // Fetched once: the bytes checked are the bytes stored.
    assert_eq!(content_calls(&fake, &id), 1);
    assert!(node.storage.exist(&file_hash).await.unwrap());

    drop(node);
    scratch.teardown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_local_or_blocked_world_with_no_scene_is_never_filled() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let (fake, base) = start_fake().await;
    let node = Node::new(scratch.pool.clone(), &base, "kept", true).await;
    add_world(&fake, "local.dcl.eth").await;
    add_world(&fake, "blocked.dcl.eth").await;
    // A local world with settings and no scene yet, and a mirrored world an admin blocked.
    sqlx::query(
        "INSERT INTO worlds.worlds (name, owner, title) VALUES ('local.dcl.eth', $1, 'mine')",
    )
    .bind(address(OWNER_KEY))
    .execute(&scratch.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO worlds.worlds (name, mirror_source, blocked_since) \
         VALUES ('blocked.dcl.eth', $1, timezone('UTC', now()))",
    )
    .bind(&base)
    .execute(&scratch.pool)
    .await
    .unwrap();
    let before: Vec<(String, Option<String>, Option<String>, bool)> = sqlx::query_as(
        "SELECT name, mirror_source, title, blocked_since IS NOT NULL FROM worlds.worlds ORDER BY name",
    )
    .fetch_all(&scratch.pool)
    .await
    .unwrap();

    for world in ["local.dcl.eth", "blocked.dcl.eth"] {
        let (status, _) = node.get(&format!("/world/{world}/scenes")).await;
        assert_eq!(status, StatusCode::OK);
        node.get(&format!("/world/{world}/about")).await;
        assert!(node.scene_rows(world).await.is_empty());
    }
    assert_eq!(fake.lock().requests, 0);
    let after: Vec<(String, Option<String>, Option<String>, bool)> = sqlx::query_as(
        "SELECT name, mirror_source, title, blocked_since IS NOT NULL FROM worlds.worlds ORDER BY name",
    )
    .fetch_all(&scratch.pool)
    .await
    .unwrap();
    assert_eq!(after, before);

    drop(node);
    scratch.teardown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_world_scene_signed_through_a_contract_wallet_link_stores_nothing() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let (fake, base) = start_fake().await;
    let node = Node::new(scratch.pool.clone(), &base, "eip1654", true).await;
    let (id, file_hash) = add_world(&fake, "wallet.dcl.eth").await;
    // The NAME holder as signer and an EIP-1654 link no contract was asked about.
    fake.lock().chains.insert(
        id.clone(),
        json!([
            {"type": "SIGNER", "payload": address(OWNER_KEY)},
            {"type": "ECDSA_EIP_1654_SIGNED_ENTITY", "payload": id, "signature": "0x00"}
        ]),
    );

    let (status, _) = node.get("/world/wallet.dcl.eth/about").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(node.scene_rows("wallet.dcl.eth").await.is_empty());
    assert!(!node.storage.exist(&id).await.unwrap());
    assert!(!node.storage.exist(&file_hash).await.unwrap());
    assert_eq!(world_rows(&scratch.pool).await, (0, 0));

    drop(node);
    scratch.teardown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_ens_world_is_never_asked_about() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let (fake, base) = start_fake().await;
    let node = Node::new(scratch.pool.clone(), &base, "ens", true).await;
    add_world(&fake, "rtens.eth").await;

    for _ in 0..2 {
        let (status, _) = node.get("/world/rtens.eth/about").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, scenes) = node.get("/world/rtens.eth/scenes").await;
        assert_eq!(
            (status, scenes["total"].clone()),
            (StatusCode::OK, json!(0))
        );
    }
    assert_eq!(fake.lock().requests, 0);
    assert_eq!(world_rows(&scratch.pool).await, (0, 0));

    drop(node);
    scratch.teardown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_oversized_scene_chain_is_refused_before_anything_of_the_scene_is_fetched() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let (fake, base) = start_fake().await;
    let node = Node::new(scratch.pool.clone(), &base, "oversized", true).await;
    let (id, file_hash) = add_world(&fake, "heavy.dcl.eth").await;
    // The owner's valid chain, padded past any real one by a link the verifier never reaches.
    let mut padded = chain(OWNER_KEY, &id, &id).await;
    padded.as_array_mut().unwrap().push(json!({
        "type": "ECDSA_SIGNED_ENTITY", "payload": "x".repeat(64 * 1024), "signature": "0x00"
    }));
    fake.lock().chains.insert(id.clone(), padded);

    let (status, _) = node.get("/world/heavy.dcl.eth/about").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // One scene asked for by its id, and the entity never fetched.
    assert_eq!(
        fake.lock().scenes_queries,
        vec![format!("entity_id={id}&limit=1")]
    );
    assert_eq!(content_calls(&fake, &id), 0);
    assert_eq!(content_calls(&fake, &file_hash), 0);
    assert!(node.scene_rows("heavy.dcl.eth").await.is_empty());
    assert_eq!(world_rows(&scratch.pool).await, (0, 0));

    drop(node);
    scratch.teardown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_world_that_becomes_local_or_blocked_mid_fill_is_left_alone() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let (fake, base) = start_fake().await;
    let node = Arc::new(Node::new(scratch.pool.clone(), &base, "race", true).await);
    let local = format!(
        "INSERT INTO worlds.worlds (name, owner, title) VALUES ($1, '{}', 'mine')",
        address(OWNER_KEY)
    );
    let blocked = "INSERT INTO worlds.worlds (name, mirror_source, blocked_since) \
                   VALUES ($1, 'elsewhere', timezone('UTC', now()))"
        .to_string();

    // Both upstream before the first fill: the names reader memoizes what the owner holds.
    add_world(&fake, "race.dcl.eth").await;
    add_world(&fake, "raced.dcl.eth").await;

    for (world, row) in [("race.dcl.eth", local), ("raced.dcl.eth", blocked)] {
        let gate = Arc::new(Gate::default());
        fake.lock().gate = Some(gate.clone());
        let read = {
            let node = node.clone();
            tokio::spawn(async move { node.get(&format!("/world/{world}/about")).await })
        };
        // The fill has passed its up-front check and waits on the scene listing: the row lands
        // now.
        tokio::time::timeout(WAIT, gate.entered.notified())
            .await
            .expect("hang guard: the fill never asked for its scene");
        sqlx::query(sqlx::AssertSqlSafe(row))
            .bind(world)
            .execute(&scratch.pool)
            .await
            .unwrap();
        let snapshot = || async {
            sqlx::query_as::<_, (Option<String>, Option<String>, bool, Option<String>)>(
                "SELECT mirror_source, title, blocked_since IS NOT NULL, owner \
                   FROM worlds.worlds WHERE name = $1",
            )
            .bind(world)
            .fetch_one(&scratch.pool)
            .await
            .unwrap()
        };
        let before = snapshot().await;
        gate.release.notify_one();
        read.await.unwrap();

        assert_eq!(snapshot().await, before, "{world}");
        assert!(node.scene_rows(world).await.is_empty(), "{world}");
        // The fill got as far as storing the files: only the row guard stopped it.
        let id = fake.lock().worlds[world][0].clone();
        assert!(node.storage.exist(&id).await.unwrap(), "{world}");
    }

    drop(node);
    scratch.teardown().await;
}
