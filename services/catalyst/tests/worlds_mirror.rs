//! The worlds mirror against an in-process upstream (`/index`,
//! `/world/{name}/about`, `/contents/{hash}`) on a scratch database and a
//! temp blob store: a complete pass with its marker, the read routes over the
//! mirrored rows, an unchanged pass making no /about calls, a local publish
//! refused (never overwritten, also when it lands mid-download), a world
//! unlisted upstream retired, an interrupted pass resumed from its persisted
//! cursor, and the loop/hash guards. Needs CATALYST_TEST_DATABASE_URL;
//! skipped without it.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use parking_lot::Mutex;
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;

use catalyst::auth::hash::hash_bytes_v1;
use catalyst::jobs::{JobContext, Jobs};
use catalyst::storage::Storage;
use catalyst::worlds::mirror::{MirrorSettings, run_pass};
use catalyst::worlds::{Settings, WorldsState, ZERO_ADDRESS};

#[path = "support/db.rs"]
mod db;

// ── the fake upstream ──────────────────────────────────────────────────────

struct Scene {
    id: String,
    entity: Value,
}

#[derive(Default)]
struct Upstream {
    /// name -> scenes; a world with none is not in the index.
    worlds: BTreeMap<String, Vec<Scene>>,
    blobs: HashMap<String, Vec<u8>>,
    about_calls: BTreeMap<String, usize>,
    /// Scene base URL override per world (the loop-guard check).
    base_override: HashMap<String, String>,
    /// While set, /about for this world cancels `hold_token` and then waits on `hold`.
    hold_world: Option<String>,
    hold_token: Option<CancellationToken>,
}

type Shared = Arc<Mutex<Upstream>>;

struct Fake {
    state: Shared,
    base: String,
    hold: Arc<Notify>,
}

fn blob(state: &mut Upstream, bytes: &[u8]) -> String {
    let hash = hash_bytes_v1(bytes);
    state.blobs.insert(hash.clone(), bytes.to_vec());
    hash
}

/// A scene entity whose files are real blobs on the upstream; returns the entity id.
fn add_scene(
    state: &mut Upstream,
    world: &str,
    title: &str,
    base: &str,
    files: &[(&str, &[u8])],
) -> String {
    let content: Vec<Value> = files
        .iter()
        .map(|(file, bytes)| json!({"file": file, "hash": blob(state, bytes)}))
        .collect();
    let entity = json!({
        "version": "v3",
        "type": "scene",
        "pointers": [base],
        "timestamp": 1_700_000_000_000i64 + state.blobs.len() as i64,
        "content": content,
        "metadata": {
            "display": {"title": title, "navmapThumbnail": files.first().map(|f| f.0)},
            "scene": {"base": base, "parcels": [base]},
            "worldConfiguration": {"name": world, "skyboxConfig": {"fixedTime": 36000}},
            "runtimeVersion": "7"
        }
    });
    let bytes = serde_json::to_vec(&entity).unwrap();
    let id = blob(state, &bytes);
    state
        .worlds
        .entry(world.to_string())
        .or_default()
        .push(Scene {
            id: id.clone(),
            entity,
        });
    id
}

/// title, base parcel, (file, bytes) pairs.
type SceneSpec<'a> = (&'a str, &'a str, &'a [(&'a str, &'a [u8])]);

fn set_scenes(state: &mut Upstream, world: &str, scenes: &[SceneSpec]) -> Vec<String> {
    state.worlds.remove(world);
    scenes
        .iter()
        .map(|(title, base, files)| add_scene(state, world, title, base, files))
        .collect()
}

async fn fake_index(State(s): State<(Shared, String)>) -> Json<Value> {
    let st = s.0.lock();
    let data: Vec<Value> = st
        .worlds
        .iter()
        .filter(|(_, scenes)| !scenes.is_empty())
        .map(|(name, scenes)| {
            json!({"name": name, "scenes": scenes.iter().map(|sc| json!({
                "id": sc.id, "title": sc.entity["metadata"]["display"]["title"],
                "pointers": sc.entity["pointers"], "timestamp": sc.entity["timestamp"]
            })).collect::<Vec<_>>()})
        })
        .collect();
    Json(json!({"data": data, "lastUpdated": "2026-09-24T00:00:00.000Z"}))
}

async fn fake_about(
    State((s, base)): State<(Shared, String)>,
    Path(name): Path<String>,
    hold: axum::Extension<Arc<Notify>>,
) -> axum::response::Response {
    let (urns, held) = {
        let mut st = s.lock();
        *st.about_calls.entry(name.clone()).or_default() += 1;
        let Some(scenes) = st.worlds.get(&name).filter(|s| !s.is_empty()) else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let content_base = st
            .base_override
            .get(&name)
            .cloned()
            .unwrap_or_else(|| format!("{base}/contents/"));
        let urns: Vec<String> = scenes
            .iter()
            .map(|sc| format!("urn:decentraland:entity:{}?=&baseUrl={content_base}", sc.id))
            .collect();
        let held = st.hold_world.as_deref() == Some(name.as_str());
        if held && let Some(token) = st.hold_token.take() {
            token.cancel();
        }
        (urns, held)
    };
    if held {
        hold.0.notified().await;
    }
    Json(json!({
        "healthy": true, "acceptingUsers": true, "spawnCoordinates": "0,0",
        "configurations": {"scenesUrn": urns, "skybox": {"fixedHour": 43200}},
        "comms": {"adapter": "fixed-adapter:signed-login:https://x/comms"}
    }))
    .into_response()
}

async fn fake_contents(
    State((s, _)): State<(Shared, String)>,
    Path(hash): Path<String>,
) -> axum::response::Response {
    match s.lock().blobs.get(&hash) {
        Some(bytes) => (StatusCode::OK, bytes.clone()).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn start_fake() -> Fake {
    let state: Shared = Arc::new(Mutex::new(Upstream::default()));
    let hold = Arc::new(Notify::new());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let app = Router::new()
        .route("/index", get(fake_index))
        .route("/world/{name}/about", get(fake_about))
        .route("/contents/{hash}", get(fake_contents))
        .layer(axum::Extension(hold.clone()))
        .with_state((state.clone(), base.clone()));
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Fake { state, base, hold }
}

struct Node {
    pool: PgPool,
    storage: Arc<Storage>,
    client: catalyst::http::Outbound,
    settings: MirrorSettings,
    public_url: String,
    jobs: Jobs,
    tmp: std::path::PathBuf,
}

impl Node {
    async fn new(pool: PgPool, source: &str, tag: &str) -> Self {
        let tmp =
            std::env::temp_dir().join(format!("catalyst-worlds-{tag}-{}", std::process::id()));
        let _ = tokio::fs::remove_dir_all(&tmp).await;
        let storage = Arc::new(Storage::from_config(&tmp).await.unwrap());
        let public_url = "http://127.0.0.1:5210".to_string();
        let own = vec![public_url.clone()];
        let settings = MirrorSettings::for_tests(source)
            .with_own_urls(&own)
            .with_concurrency(1);
        Self {
            pool,
            storage,
            client: catalyst::http::Outbound::for_tests(),
            settings,
            public_url,
            jobs: Jobs::new(),
            tmp,
        }
    }

    /// A job context whose token the test can cancel.
    async fn ctx(&self) -> JobContext {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.jobs.spawn("ctx", move |ctx| async move {
            let _ = tx.send(ctx.clone());
            ctx.cancel.cancelled().await;
            Ok(())
        });
        rx.await.unwrap()
    }

    async fn pass(&self, ctx: &JobContext) -> catalyst::worlds::mirror::PassSummary {
        run_pass(&self.client, &self.pool, &self.storage, &self.settings, ctx)
            .await
            .expect("mirror pass")
    }

    fn router(&self) -> Router {
        let state = Arc::new(WorldsState::new(
            self.pool.clone(),
            self.storage.clone(),
            Settings {
                comms_online: true,
                ..Settings::for_tests(&self.public_url)
            },
            Some(self.settings.clone()),
            self.client.clone(),
        ));
        catalyst::worlds::router(state)
    }

    async fn get(&self, path: &str) -> (StatusCode, Value) {
        let resp = self
            .router()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, body)
    }

    async fn scene_rows(&self, world: &str) -> Vec<(String, String, Option<String>)> {
        sqlx::query_as(
            "SELECT entity_id, deployer, mirror_source FROM worlds.world_scenes WHERE lower(world_name) = lower($1) ORDER BY entity_id",
        )
        .bind(world)
        .fetch_all(&self.pool)
        .await
        .unwrap()
    }

    async fn pass_row(
        &self,
        mirror: &str,
    ) -> (Option<String>, Option<chrono::NaiveDateTime>, i64, Value) {
        sqlx::query_as(
            "SELECT cursor, pass_completed_at, pass_number, stats FROM worlds.mirror_passes WHERE mirror = $1",
        )
        .bind(mirror)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.tmp);
    }
}

fn about_calls(fake: &Fake, world: &str) -> usize {
    fake.state
        .lock()
        .about_calls
        .get(world)
        .copied()
        .unwrap_or(0)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mirror_passes_are_complete_resumable_and_never_touch_local_publishes() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let fake = start_fake().await;
    let node = Node::new(scratch.pool.clone(), &fake.base, "story").await;
    let png: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];
    let big = vec![7u8; 300 * 1024];
    let (a_id, b_ids, c_id) = {
        let mut st = fake.state.lock();
        let a = set_scenes(
            &mut st,
            "a.dcl.eth",
            &[("A", "0,0", &[("thumb.png", png), ("game.js", b"a()")])],
        );
        let b = set_scenes(
            &mut st,
            "b.dcl.eth",
            &[
                ("B1", "0,0", &[("thumb.png", png), ("big.bin", &big)]),
                ("B2", "1,0", &[("t2.png", png)]),
            ],
        );
        let c = set_scenes(&mut st, "c-ens", &[("C", "5,5", &[("c.txt", b"c")])]);
        (a[0].clone(), b, c[0].clone())
    };

    // 1. A complete first pass: catalogue, scenes, files, markers.
    let ctx = node.ctx().await;
    let summary = node.pass(&ctx).await;
    assert_eq!(
        (
            summary.catalogue.listed,
            summary.catalogue.unlisted,
            summary.catalogue.complete
        ),
        (3, 0, true)
    );
    assert_eq!(
        (
            summary.scenes.processed,
            summary.scenes.synced,
            summary.scenes.scenes
        ),
        (3, 3, 4)
    );
    assert!(summary.scenes.complete && !summary.scenes.interrupted);
    assert_eq!(
        summary.scenes.new_blobs,
        4 + 4,
        "4 entities and 4 distinct files; the shared PNG bytes are fetched once"
    );
    let hashes: Vec<String> = fake.state.lock().blobs.keys().cloned().collect();
    for hash in &hashes {
        assert!(
            node.storage.exist(hash).await.unwrap(),
            "{hash} must be in the blob store"
        );
    }
    let stored = node
        .storage
        .retrieve(&hash_bytes_v1(&big))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        stored.len(),
        big.len(),
        "a chunked (dag-pb) file verifies and stores whole"
    );
    let (cursor, completed, number, stats) = node.pass_row("scenes").await;
    assert!(
        cursor.is_none() && completed.is_some(),
        "a complete pass clears the cursor and stamps completion"
    );
    assert_eq!(number, 1);
    assert_eq!(stats["synced"], 3);
    let (_, completed, number, _) = node.pass_row("catalogue").await;
    assert!(completed.is_some() && number == 1);
    let rows = node.scene_rows("b.dcl.eth").await;
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .all(|(_, d, m)| d == ZERO_ADDRESS && m.as_deref() == Some(fake.base.as_str()))
    );

    // 2. The read routes over the mirrored rows.
    let (status, about) = node.get("/world/B.DCL.ETH/about").await;
    assert_eq!(status, StatusCode::OK, "{about}");
    let urns = about["configurations"]["scenesUrn"].as_array().unwrap();
    assert_eq!(urns.len(), 2);
    for urn in urns {
        let urn = urn.as_str().unwrap();
        assert!(
            urn.ends_with("?=&baseUrl=http://127.0.0.1:5210/contents/"),
            "{urn}"
        );
        assert!(b_ids.iter().any(|id| urn.contains(id)), "{urn}");
    }
    assert_eq!(
        about["configurations"]["realmName"], "b.dcl.eth",
        "a mirrored world keeps its ENS name"
    );
    assert_eq!(
        about["configurations"]["skybox"]["fixedHour"], 43200.0,
        "the upstream about's fixedHour"
    );
    assert_eq!(about["spawnCoordinates"], "0,0");
    assert_eq!(
        about["comms"]["adapter"],
        "fixed-adapter:signed-login:http://127.0.0.1:5210/worlds/b.dcl.eth/comms",
        "this node's comms mint, not the upstream's adapter"
    );
    let (status, scenes) = node.get("/world/b.dcl.eth/scenes").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(scenes["total"], 2);
    assert_eq!(scenes["scenes"][0]["deployer"], ZERO_ADDRESS);
    assert_eq!(scenes["scenes"][0]["status"], "DEPLOYED");
    let (status, index) = node.get("/index").await;
    assert_eq!(status, StatusCode::OK);
    let names: Vec<&str> = index["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["a.dcl.eth", "b.dcl.eth", "c-ens"]);
    assert_eq!(index["data"][0]["scenes"][0]["id"], a_id);
    assert!(
        index["data"][0]["scenes"][0]["thumbnail"]
            .as_str()
            .unwrap()
            .starts_with("http://127.0.0.1:5210/contents/"),
        "{index}"
    );
    let (status, st) = node.get("/worlds/status").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(st["content"]["worldsCount"], json!({"ens": 1, "dcl": 2}));
    assert_eq!(st["mirror"]["enabled"], true);
    assert_eq!(st["mirror"]["source"], fake.base);
    assert_eq!(st["mirror"]["scenes"]["inProgress"], false);
    assert!(
        st["mirror"]["scenes"]["lastCompletedAt"].is_string(),
        "{st}"
    );
    assert!(st["mirror"]["scenes"]["ageSeconds"].as_i64().unwrap() >= 0);
    assert_eq!(st["mirror"]["scenes"]["stats"]["synced"], 3);
    assert_eq!(
        st["mirror"]["worlds"],
        json!({"listed": 3, "mirrored": 3, "pending": 0, "failed": 0, "unlisted": 0})
    );
    let (status, _) = node.get("/world/nothing.dcl.eth/about").await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // 3. An unchanged catalogue costs no /about call.
    let calls_before: BTreeMap<String, usize> = fake.state.lock().about_calls.clone();
    let summary = node.pass(&ctx).await;
    assert_eq!(
        (
            summary.scenes.unchanged,
            summary.scenes.synced,
            summary.scenes.new_blobs
        ),
        (3, 0, 0)
    );
    assert_eq!(
        fake.state.lock().about_calls,
        calls_before,
        "no world was re-fetched"
    );
    assert_eq!(node.pass_row("scenes").await.2, 2);

    // 4. A local publish is refused, never overwritten, and stays served; the
    //    mirrored scene written before it is retired, since the mirror never
    //    refreshes the world again.
    sqlx::query(
        r#"INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size, mirror_source)
           VALUES ('a.dcl.eth', 'bafkreilocal', '[]', '{"pointers":["9,9"],"metadata":{"scene":{"base":"9,9"}}}', '0xabc', '{9,9}', 1, NULL)"#,
    )
    .execute(&node.pool)
    .await
    .unwrap();
    {
        let mut st = fake.state.lock();
        set_scenes(
            &mut st,
            "a.dcl.eth",
            &[("A v2", "0,0", &[("new.js", b"a2()")])],
        );
    }
    let summary = node.pass(&ctx).await;
    assert_eq!((summary.scenes.refused, summary.scenes.unchanged), (1, 2));
    let rows = node.scene_rows("a.dcl.eth").await;
    assert_eq!(
        rows,
        vec![("bafkreilocal".to_string(), "0xabc".to_string(), None)],
        "only the local scene; the leftover mirrored {a_id} is retired"
    );
    let mirror_source: Option<String> =
        sqlx::query_scalar("SELECT mirror_source FROM worlds.worlds WHERE name = 'a.dcl.eth'")
            .fetch_one(&node.pool)
            .await
            .unwrap();
    assert_eq!(mirror_source, None, "the world row says local");
    let (status, about) = node.get("/world/a.dcl.eth/about").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        about["configurations"]["realmName"], "a",
        "a locally published world loses the ENS suffix"
    );
    let (_, st) = node.get("/worlds/status").await;
    assert_eq!(
        st["mirror"]["worlds"]["failed"], 1,
        "the refusal is visible as the world's last error"
    );

    // 5. A world the catalogue no longer lists is retired; its blobs stay.
    fake.state.lock().worlds.remove("c-ens");
    let summary = node.pass(&ctx).await;
    assert_eq!(
        (summary.catalogue.listed, summary.catalogue.unlisted),
        (2, 1)
    );
    assert_eq!(summary.scenes.retired_scenes, 1);
    assert!(node.scene_rows("c-ens").await.is_empty());
    let world_rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM worlds.worlds WHERE name = 'c-ens'")
            .fetch_one(&node.pool)
            .await
            .unwrap();
    assert_eq!(world_rows, 0);
    assert!(
        node.storage.exist(&c_id).await.unwrap(),
        "blobs are content-addressed and not reaped here"
    );
    let (status, _) = node.get("/world/c-ens/about").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, index) = node.get("/index").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        index["data"].as_array().unwrap().len(),
        2,
        "the index memo is short; a fresh page after the sweep"
    );

    // 6. An interrupted pass resumes from its cursor: d is not re-fetched, e and f are.
    {
        let mut st = fake.state.lock();
        set_scenes(&mut st, "d.dcl.eth", &[("D", "0,0", &[("d.txt", b"d")])]);
        set_scenes(&mut st, "e.dcl.eth", &[("E", "0,0", &[("e.txt", b"e")])]);
        set_scenes(&mut st, "f.dcl.eth", &[("F", "0,0", &[("f.txt", b"f")])]);
        st.hold_world = Some("e.dcl.eth".to_string());
    }
    let interrupted_ctx = node.ctx().await;
    fake.state.lock().hold_token = Some(interrupted_ctx.cancel.clone());
    let summary = node.pass(&interrupted_ctx).await;
    assert!(summary.scenes.interrupted && !summary.scenes.complete);
    let (cursor, _, number, _) = node.pass_row("scenes").await;
    assert_eq!(
        cursor.as_deref(),
        Some("d.dcl.eth"),
        "the cursor is the last world completed"
    );
    assert_eq!(number, 4, "an interrupted pass does not count");
    let (_, st) = node.get("/worlds/status").await;
    assert_eq!(st["mirror"]["scenes"]["inProgress"], true);
    assert_eq!(st["mirror"]["scenes"]["cursor"], "d.dcl.eth");
    assert_eq!(
        st["mirror"]["worlds"]["pending"], 2,
        "e and f are listed but not mirrored yet"
    );
    fake.state.lock().hold_world = None;
    fake.hold.notify_waiters();
    assert_eq!(about_calls(&fake, "d.dcl.eth"), 1);
    assert_eq!(about_calls(&fake, "e.dcl.eth"), 1);
    assert_eq!(about_calls(&fake, "f.dcl.eth"), 0);

    let resumed_ctx = node.ctx().await;
    let summary = node.pass(&resumed_ctx).await;
    assert_eq!(summary.scenes.resumed_from.as_deref(), Some("d.dcl.eth"));
    assert!(summary.scenes.complete);
    assert_eq!(
        summary.scenes.processed, 5,
        "a, b, d before the interruption, then e and f"
    );
    assert_eq!(
        about_calls(&fake, "d.dcl.eth"),
        1,
        "nothing before the cursor is fetched again"
    );
    assert_eq!(about_calls(&fake, "e.dcl.eth"), 2);
    assert_eq!(about_calls(&fake, "f.dcl.eth"), 1);
    assert_eq!(node.scene_rows("f.dcl.eth").await.len(), 1);
    let (cursor, _, number, _) = node.pass_row("scenes").await;
    assert!(cursor.is_none() && number == 5);
    let listed: i64 =
        sqlx::query_scalar("SELECT count(*) FROM worlds.remote_worlds WHERE unlisted_at IS NULL")
            .fetch_one(&node.pool)
            .await
            .unwrap();
    assert_eq!(listed, 5);

    // 7. The guards: a scene base URL that names this node is a loop, and
    //    bytes that do not hash to their CID are never stored.
    {
        let mut st = fake.state.lock();
        set_scenes(&mut st, "g.dcl.eth", &[("G", "0,0", &[("g.txt", b"g")])]);
        st.base_override.insert(
            "g.dcl.eth".to_string(),
            format!("{}/contents/", node.public_url),
        );
        let ids = set_scenes(
            &mut st,
            "h.dcl.eth",
            &[("H", "0,0", &[("h.txt", b"honest")])],
        );
        let h_file = hash_bytes_v1(b"honest");
        st.blobs.insert(h_file, b"tampered".to_vec());
        let _ = ids;
    }
    let summary = node.pass(&node.ctx().await).await;
    assert_eq!(summary.scenes.failed, 2, "{summary:?}");
    let errors: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT world_name, last_error FROM worlds.remote_worlds WHERE world_name IN ('g.dcl.eth','h.dcl.eth') ORDER BY 1")
            .fetch_all(&node.pool)
            .await
            .unwrap();
    assert!(
        errors[0].1.as_deref().unwrap().contains("will not fetch"),
        "{errors:?}"
    );
    assert!(
        errors[1].1.as_deref().unwrap().contains("does not hash"),
        "{errors:?}"
    );
    assert!(node.scene_rows("g.dcl.eth").await.is_empty());
    assert!(node.scene_rows("h.dcl.eth").await.is_empty());
    assert!(!node.storage.exist(&hash_bytes_v1(b"honest")).await.unwrap());

    node.jobs.shutdown(std::time::Duration::from_secs(1)).await;
    drop(node);
    scratch.teardown().await;
}

/// A local publish that lands while the mirror is downloading a world wins:
/// the mirror's write re-checks under the world row's lock, so it neither
/// marks the world mirrored again, nor lifts its block, nor writes its scenes
/// beside the local one.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_local_publish_landing_mid_download_is_never_overwritten() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let fake = start_fake().await;
    let node = Node::new(scratch.pool.clone(), &fake.base, "race").await;
    let world = "r.dcl.eth";
    let v1 = set_scenes(
        &mut fake.state.lock(),
        world,
        &[("R", "0,0", &[("r.js", b"r()")])],
    );
    let ctx = node.ctx().await;
    assert_eq!(node.pass(&ctx).await.scenes.synced, 1);

    let about_reached = CancellationToken::new();
    {
        let mut st = fake.state.lock();
        set_scenes(&mut st, world, &[("R v2", "0,0", &[("r.js", b"r2()")])]);
        st.hold_world = Some(world.to_string());
        st.hold_token = Some(about_reached.clone());
    }
    let publish = async {
        about_reached.cancelled().await;
        // What a local publish leaves (core::store::deploy_scene), then an
        // operator's block, all while the mirror waits on /about.
        sqlx::query(
            "UPDATE worlds.worlds SET mirror_source = NULL, blocked_since = timezone('UTC', now()) WHERE name = $1",
        )
        .bind(world)
        .execute(&node.pool)
        .await
        .unwrap();
        sqlx::query("DELETE FROM worlds.world_scenes WHERE world_name = $1 AND entity_id = $2")
            .bind(world)
            .bind(&v1[0])
            .execute(&node.pool)
            .await
            .unwrap();
        sqlx::query(
            r#"INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size, mirror_source)
               VALUES ($1, 'bafkreilocalr', '[]', '{"pointers":["0,0"],"metadata":{"scene":{"base":"0,0"}}}', '0xabc', '{0,0}', 1, NULL)"#,
        )
        .bind(world)
        .execute(&node.pool)
        .await
        .unwrap();
        fake.state.lock().hold_world = None;
        fake.hold.notify_one();
    };
    let (summary, ()) = tokio::join!(node.pass(&ctx), publish);
    assert_eq!(
        (summary.scenes.refused, summary.scenes.synced),
        (1, 0),
        "{summary:?}"
    );
    assert_eq!(
        node.scene_rows(world).await,
        vec![("bafkreilocalr".to_string(), "0xabc".to_string(), None)]
    );
    let row: (Option<String>, Option<chrono::NaiveDateTime>) =
        sqlx::query_as("SELECT mirror_source, blocked_since FROM worlds.worlds WHERE name = $1")
            .bind(world)
            .fetch_one(&node.pool)
            .await
            .unwrap();
    assert_eq!(row.0, None, "the world stays local");
    assert!(row.1.is_some(), "the block stands");

    node.jobs.shutdown(std::time::Duration::from_secs(1)).await;
    drop(node);
    scratch.teardown().await;
}

/// An operator disable of a mirrored world survives the scheduled pass that
/// refreshes its scenes: the mirror never writes `blocked_since`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_mirror_pass_keeps_an_operator_disable() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let fake = start_fake().await;
    let node = Node::new(scratch.pool.clone(), &fake.base, "disable").await;
    let world = "d.dcl.eth";
    set_scenes(
        &mut fake.state.lock(),
        world,
        &[("D", "0,0", &[("d.js", b"d()")])],
    );
    let ctx = node.ctx().await;
    assert_eq!(node.pass(&ctx).await.scenes.synced, 1);

    sqlx::query("UPDATE worlds.worlds SET blocked_since = timezone('UTC', now()) WHERE name = $1")
        .bind(world)
        .execute(&node.pool)
        .await
        .unwrap();
    let v2 = set_scenes(
        &mut fake.state.lock(),
        world,
        &[("D v2", "0,0", &[("d.js", b"d2()")])],
    );
    assert_eq!(node.pass(&ctx).await.scenes.synced, 1);

    let rows = node.scene_rows(world).await;
    assert_eq!(
        rows.iter().map(|r| r.0.clone()).collect::<Vec<_>>(),
        v2,
        "the pass refreshed the scenes"
    );
    let blocked: Option<chrono::NaiveDateTime> =
        sqlx::query_scalar("SELECT blocked_since FROM worlds.worlds WHERE name = $1")
            .bind(world)
            .fetch_one(&node.pool)
            .await
            .unwrap();
    assert!(blocked.is_some(), "the operator disable stands");
    let (status, _) = node.get(&format!("/world/{world}/about")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    node.jobs.shutdown(std::time::Duration::from_secs(1)).await;
    drop(node);
    scratch.teardown().await;
}

/// `(identity, parcels, active)` of a world's storage identities, oldest first.
async fn identities(pool: &PgPool, world: &str) -> Vec<(uuid::Uuid, Vec<String>, bool)> {
    sqlx::query_as(
        "SELECT identity, parcels, active FROM explore.world_scene_place WHERE world_name = $1 ORDER BY created_at, identity",
    )
    .bind(world)
    .fetch_all(pool)
    .await
    .unwrap()
}

/// Newest-first about ordering must not give the predecessor's scope to
/// a later deployment on a parcel that its real replacement has vacated.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mirror_assigns_storage_identities_in_entity_timestamp_order() {
    let Some(scratch) = db::scratch_database().await else {
        return;
    };
    let fake = start_fake().await;
    let node = Node::new(scratch.pool.clone(), &fake.base, "identity-order").await;
    let world = "ordered.dcl.eth";
    let scene = |st: &mut Upstream, timestamp: i64, base: &str, parcels: &[&str]| {
        let entity = json!({
            "version": "v3", "type": "scene", "timestamp": timestamp,
            "pointers": parcels, "content": [],
            "metadata": {"scene": {"base": base, "parcels": parcels},
                         "worldConfiguration": {"name": world}}
        });
        let id = blob(st, &serde_json::to_vec(&entity).unwrap());
        Scene { id, entity }
    };
    {
        let mut st = fake.state.lock();
        let original = scene(&mut st, 1000, "0,0", &["0,0", "1,0"]);
        st.worlds.insert(world.into(), vec![original]);
    }
    let ctx = node.ctx().await;
    node.pass(&ctx).await;
    let original = identities(&node.pool, world).await[0].0;
    {
        let mut st = fake.state.lock();
        let replacement = scene(&mut st, 2000, "1,0", &["1,0"]);
        let later = scene(&mut st, 3000, "0,0", &["0,0"]);
        st.worlds.insert(world.into(), vec![later, replacement]);
    }
    node.pass(&ctx).await;
    let rows = identities(&node.pool, world).await;
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|(_, _, active)| *active));
    assert_eq!(
        rows.iter().find(|(_, p, _)| p == &["1,0"]).unwrap().0,
        original
    );
    assert_ne!(
        rows.iter().find(|(_, p, _)| p == &["0,0"]).unwrap().0,
        original
    );
    node.pass(&ctx).await;
    assert_eq!(identities(&node.pool, world).await, rows);
    node.jobs.shutdown(std::time::Duration::from_secs(1)).await;
    drop(node);
    scratch.teardown().await;
}

/// The mirror keeps one storage identity per mirrored scene by Places'
/// overlap rule: a republished scene over the same parcel keeps it, a scene
/// no longer listed retires its own, and an unlisted world retires them all.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_mirror_pass_keeps_a_storage_identity_per_scene() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let fake = start_fake().await;
    let node = Node::new(scratch.pool.clone(), &fake.base, "identity").await;
    {
        let mut st = fake.state.lock();
        set_scenes(&mut st, "keep.dcl.eth", &[("K", "0,0", &[("k.txt", b"k")])]);
        set_scenes(
            &mut st,
            "m.dcl.eth",
            &[
                ("M1", "0,0", &[("m1.txt", b"m1")]),
                ("M2", "1,0", &[("m2.txt", b"m2")]),
            ],
        );
    }
    let ctx = node.ctx().await;
    node.pass(&ctx).await;
    let first = identities(&node.pool, "m.dcl.eth").await;
    assert_eq!(first.len(), 2);
    assert!(first.iter().all(|(_, _, active)| *active));
    assert_ne!(first[0].0, first[1].0, "each mirrored scene has its own");
    let m1 = first
        .iter()
        .find(|(_, parcels, _)| parcels == &["0,0"])
        .unwrap()
        .0;

    // M1 republished over 0,0 (a new entity), M2 dropped upstream.
    set_scenes(
        &mut fake.state.lock(),
        "m.dcl.eth",
        &[("M1b", "0,0", &[("m1b.txt", b"m1b")])],
    );
    node.pass(&ctx).await;
    let second: Vec<(uuid::Uuid, bool)> = identities(&node.pool, "m.dcl.eth")
        .await
        .into_iter()
        .map(|(id, _, active)| (id, active))
        .collect();
    assert_eq!(second.len(), 2, "no identity minted for the republish");
    assert!(second.contains(&(m1, true)), "the republish kept M1's");
    assert_eq!(
        second.iter().filter(|(_, active)| *active).count(),
        1,
        "the dropped scene's identity retired"
    );

    // Unlisted: every identity of the world retires.
    fake.state.lock().worlds.remove("m.dcl.eth");
    node.pass(&ctx).await;
    assert!(node.scene_rows("m.dcl.eth").await.is_empty());
    assert!(
        identities(&node.pool, "m.dcl.eth")
            .await
            .iter()
            .all(|(_, _, active)| !active)
    );
    assert_eq!(identities(&node.pool, "keep.dcl.eth").await.len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_catalogue_that_would_unlist_most_of_the_mirror_is_refused() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let fake = start_fake().await;
    let mut node = Node::new(scratch.pool.clone(), &fake.base, "fuse").await;
    node.settings = node.settings.clone().with_concurrency(8);
    {
        let mut st = fake.state.lock();
        for i in 0..60 {
            let name = format!("w{i:03}.dcl.eth");
            let body = format!("scene {i}");
            set_scenes(
                &mut st,
                &name,
                &[("W", "0,0", &[("w.txt", body.as_bytes())])],
            );
        }
    }
    let ctx = node.ctx().await;
    let summary = node.pass(&ctx).await;
    assert_eq!((summary.catalogue.listed, summary.scenes.synced), (60, 60));

    let keep: Vec<String> = fake.state.lock().worlds.keys().take(1).cloned().collect();
    fake.state.lock().worlds.retain(|k, _| keep.contains(k));
    let summary = node.pass(&ctx).await;
    assert_eq!(summary.catalogue.listed, 1);
    assert_eq!(summary.catalogue.unlisted, 0, "59 of 60 exceeds the fuse");
    assert!(!summary.catalogue.complete);
    let (_, completed, number, _) = node.pass_row("catalogue").await;
    assert_eq!(number, 1, "a refused pass is not a complete pass");
    assert!(completed.is_some(), "the previous completion stands");
    let still_listed: i64 =
        sqlx::query_scalar("SELECT count(*) FROM worlds.remote_worlds WHERE unlisted_at IS NULL")
            .fetch_one(&node.pool)
            .await
            .unwrap();
    assert_eq!(still_listed, 60);
    assert_eq!(summary.scenes.retired_scenes, 0);
    let (_, st) = node.get("/worlds/status").await;
    assert!(
        st["mirror"]["catalogue"]["lastError"]
            .as_str()
            .unwrap()
            .contains("refused to unlist 59"),
        "{st}"
    );

    // A small removal still goes through.
    {
        let mut st = fake.state.lock();
        for i in 0..60 {
            let name = format!("w{i:03}.dcl.eth");
            if i >= 3 && !st.worlds.contains_key(&name) {
                let body = format!("scene {i}");
                set_scenes(
                    &mut st,
                    &name,
                    &[("W", "0,0", &[("w.txt", body.as_bytes())])],
                );
            }
        }
    }
    let summary = node.pass(&ctx).await;
    assert_eq!(
        (
            summary.catalogue.listed,
            summary.catalogue.unlisted,
            summary.catalogue.complete
        ),
        (58, 2, true)
    );
    assert_eq!(summary.scenes.retired_scenes, 2);

    node.jobs.shutdown(std::time::Duration::from_secs(1)).await;
    drop(node);
    scratch.teardown().await;
}
