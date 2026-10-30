//! In-process contract tests: every route through the gateway router against
//! one mock upstream that stands in for all five sources. Database-backed
//! tests create a scratch database on the cluster `CATALYST_TEST_DATABASE_URL`
//! points at (schema.sql applied, dropped on teardown) and skip without it.

use std::collections::{BTreeSet, HashMap};
use std::future::IntoFuture;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Query, State as AxState};
use axum::http::{HeaderMap, Request, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use parking_lot::Mutex;
use serde_json::{Value, json};
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};
use tower::ServiceExt;

use super::*;
use crate::content::{ContentState, SyncState};
use crate::jobs::Jobs;
use crate::storage::Storage;

struct Mock {
    pois: Mutex<Vec<String>>,
    banned: Mutex<Vec<String>>,
    denylist: Mutex<Vec<Value>>,
    flags: Mutex<Value>,
    flags_etag: Mutex<String>,
    entities: Mutex<Vec<Value>>,
    profiles: Mutex<Vec<Value>>,
    tiles: Mutex<Value>,
    tiles_etag: Mutex<String>,
    tiles_last_updated: AtomicI64,
    hits: Mutex<HashMap<String, usize>>,
    bodies: Mutex<Vec<Value>>,
}

impl Mock {
    fn hit(&self, what: &str) {
        *self.hits.lock().entry(what.to_string()).or_default() += 1;
    }

    fn hits(&self, what: &str) -> usize {
        self.hits.lock().get(what).copied().unwrap_or(0)
    }
}

fn entity(id: &str, pointers: &[&str]) -> Value {
    json!({
        "id": id, "type": "scene", "timestamp": "1789578092206", "pointers": pointers, "content": [],
        "metadata": {}, "deployer": "0x462a", "status": "complete",
        "bundles": {"assets": {"mac": "complete", "windows": "complete", "webgl": "pending"}},
        "versions": {"assets": {"mac": {"version": "v49", "buildDate": "2026-09-16T17:14:28.886Z"}}}
    })
}

fn tile(x: i32, y: i32, kind: &str, estate: Option<&str>) -> Value {
    let mut t = json!({"id": format!("{x},{y}"), "x": x, "y": y, "type": kind, "top": false, "left": false, "topLeft": false, "updatedAt": 1});
    if let Some(e) = estate {
        t["estateId"] = json!(e);
        t["owner"] = json!("0xowner");
    }
    t
}

fn tiles_doc(stamp: i64) -> Value {
    let mut data = serde_json::Map::new();
    for t in [
        tile(0, 0, "plaza", Some("1164")),
        tile(1, 0, "road", None),
        tile(1, 1, "owned", Some("77")),
        tile(2, 1, "district", None),
    ] {
        data.insert(t["id"].as_str().unwrap().to_string(), t);
    }
    json!({"ok": true, "data": data, "stamp": stamp})
}

fn mock() -> Arc<Mock> {
    Arc::new(Mock {
        pois: Mutex::new(vec!["0,0".into(), "16,83".into(), "-148,131".into()]),
        banned: Mutex::new(vec!["badname".into()]),
        denylist: Mutex::new(vec![
            json!({"wallet": "0xAbC0000000000000000000000000000000000001"}),
            json!("0x0000000000000000000000000000000000000002"),
        ]),
        flags: Mutex::new(
            json!({"flags": {"explorer-alfa-minimum-requirements": true, "explorer-feature-a": false}, "variants": {"explorer-variant": {"name": "v", "enabled": true}}}),
        ),
        flags_etag: Mutex::new("\"flags-1\"".into()),
        entities: Mutex::new(vec![
            entity("bafyscene", &["0,0", "0,1"]),
            entity("bafyother", &["5,5"]),
        ]),
        profiles: Mutex::new(vec![
            json!({"timestamp": "1790276949479", "avatars": [{"name": "Ryumi", "ethAddress": "0xC9C29AB98E6BC42015985165A11153F564E9F8C2", "userId": "0xc9c29ab98e6bc42015985165a11153f564e9f8c2"}]}),
        ]),
        tiles: Mutex::new(tiles_doc(1000)),
        tiles_etag: Mutex::new("\"grid-1\"".into()),
        tiles_last_updated: AtomicI64::new(1000),
        hits: Mutex::new(HashMap::new()),
        bodies: Mutex::new(Vec::new()),
    })
}

async fn serve_mock(mock: Arc<Mock>) -> SocketAddr {
    async fn pois(AxState(m): AxState<Arc<Mock>>) -> Json<Value> {
        m.hit("pois");
        Json(json!({"data": *m.pois.lock()}))
    }
    async fn banned(AxState(m): AxState<Arc<Mock>>) -> Json<Value> {
        m.hit("banned-names");
        Json(json!({"data": *m.banned.lock()}))
    }
    async fn denylist(AxState(m): AxState<Arc<Mock>>) -> Json<Value> {
        m.hit("denylist");
        Json(json!({"users": *m.denylist.lock()}))
    }
    async fn flags(AxState(m): AxState<Arc<Mock>>, headers: HeaderMap) -> Response {
        m.hit("flags");
        if headers.get(header::REFERER).and_then(|v| v.to_str().ok())
            != Some("https://decentraland.org")
        {
            return (StatusCode::OK, Json(json!({"flags": {}, "variants": {}}))).into_response();
        }
        let etag = m.flags_etag.lock().clone();
        if headers
            .get(header::IF_NONE_MATCH)
            .and_then(|v| v.to_str().ok())
            == Some(etag.as_str())
        {
            m.hit("flags-304");
            return StatusCode::NOT_MODIFIED.into_response();
        }
        (
            StatusCode::OK,
            [(header::ETAG, etag)],
            Json(m.flags.lock().clone()),
        )
            .into_response()
    }
    async fn entities(
        AxState(m): AxState<Arc<Mock>>,
        Query(q): Query<HashMap<String, String>>,
        Json(body): Json<Value>,
    ) -> Json<Value> {
        m.hit("entities/active");
        m.bodies.lock().push(json!({"body": body, "query": q}));
        let wanted: Vec<String> = body["pointers"]
            .as_array()
            .or(body["ids"].as_array())
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_lowercase)
                    .collect()
            })
            .unwrap_or_default();
        let docs: Vec<Value> = m
            .entities
            .lock()
            .iter()
            .filter(|e| {
                wanted.contains(&e["id"].as_str().unwrap().to_string())
                    || e["pointers"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|p| wanted.contains(&p.as_str().unwrap().to_string()))
            })
            .cloned()
            .collect();
        Json(Value::Array(docs))
    }
    async fn profiles(AxState(m): AxState<Arc<Mock>>, Json(body): Json<Value>) -> Json<Value> {
        m.hit("profiles");
        m.bodies.lock().push(body.clone());
        let wanted: Vec<String> = body["ids"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_lowercase)
                    .collect()
            })
            .unwrap_or_default();
        let docs: Vec<Value> = m
            .profiles
            .lock()
            .iter()
            .filter(|p| wanted.contains(&p["avatars"][0]["userId"].as_str().unwrap().to_string()))
            .cloned()
            .collect();
        Json(Value::Array(docs))
    }
    /// Upstream's `getMetadata` projection: no avatars, the address as `pointer`.
    async fn profiles_metadata(
        AxState(m): AxState<Arc<Mock>>,
        Json(body): Json<Value>,
    ) -> Json<Value> {
        m.hit("profiles/metadata");
        let wanted: Vec<String> = body["ids"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_lowercase)
                    .collect()
            })
            .unwrap_or_default();
        let docs: Vec<Value> = m
            .profiles
            .lock()
            .iter()
            .filter(|p| wanted.contains(&p["avatars"][0]["userId"].as_str().unwrap().to_string()))
            .map(|p| json!({"pointer": p["avatars"][0]["userId"], "hasClaimedName": false, "name": p["avatars"][0]["name"], "thumbnailUrl": "https://profile-images.decentraland.org/entities/bafkreiprofile/face.png"}))
            .collect();
        Json(Value::Array(docs))
    }
    async fn tiles_info(AxState(m): AxState<Arc<Mock>>) -> Json<Value> {
        m.hit("tiles/info");
        Json(json!({"lastUpdatedAt": m.tiles_last_updated.load(Ordering::SeqCst)}))
    }
    async fn tiles(AxState(m): AxState<Arc<Mock>>, headers: HeaderMap) -> Response {
        m.hit("tiles");
        let etag = m.tiles_etag.lock().clone();
        if headers
            .get(header::IF_NONE_MATCH)
            .and_then(|v| v.to_str().ok())
            == Some(etag.as_str())
        {
            m.hit("tiles-304");
            return StatusCode::NOT_MODIFIED.into_response();
        }
        (
            StatusCode::OK,
            [(header::ETAG, etag)],
            Json(m.tiles.lock().clone()),
        )
            .into_response()
    }
    async fn ab_object(
        AxState(m): AxState<Arc<Mock>>,
        axum::extract::Path(path): axum::extract::Path<String>,
    ) -> Response {
        m.hit(&format!("ab-cdn:{path}"));
        if path.ends_with("missing_mac") {
            return StatusCode::NOT_FOUND.into_response();
        }
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/octet-stream")],
            format!("bundle:{path}"),
        )
            .into_response()
    }
    let app = Router::new()
        .route("/v49/{*path}", get(ab_object))
        .route("/LOD/{*path}", get(ab_object))
        .route("/pois", post(pois))
        .route("/banned-names", post(banned))
        .route("/denylist.json", get(denylist))
        .route("/explorer.json", get(flags))
        .route("/entities/active", post(entities))
        .route("/profiles", post(profiles))
        .route("/profiles/metadata", post(profiles_metadata))
        .route("/v2/tiles/info", get(tiles_info))
        .route("/v2/tiles", get(tiles))
        .with_state(mock);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(axum::serve(listener, app).into_future());
    addr
}

fn sources_at(addr: SocketAddr) -> Sources {
    let base = format!("http://{addr}");
    Sources {
        lists: base.clone(),
        denylist: format!("{base}/denylist.json"),
        feature_flags: format!("{base}/explorer.json"),
        registry: base.clone(),
        map: base.clone(),
        ab_cdn: base,
    }
}

struct Node {
    state: Shared,
    _tmp: TempDir,
}

struct TempDir(std::path::PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn content_state(pool: PgPool) -> (Arc<ContentState>, TempDir) {
    let tmp = std::env::temp_dir().join(format!(
        "catalyst-explore-api-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let storage = Arc::new(Storage::from_config(&tmp).await.unwrap());
    (
        Arc::new(ContentState::new(
            pool,
            crate::content::Settings::for_tests(),
            storage,
        )),
        TempDir(tmp),
    )
}

fn lazy_pool() -> PgPool {
    PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(300))
        .connect_lazy("postgres://nobody@127.0.0.1:1/nowhere")
        .unwrap()
}

/// A node with no database behind it: everything the routes answer from
/// memory (the realm doc, the registry reader, the embedded flags).
async fn node_without_db(settings: Settings) -> Node {
    let pool = lazy_pool();
    let (content, tmp) = content_state(pool.clone()).await;
    let state = Arc::new(State {
        settings,
        pool,
        http: crate::http::Outbound::for_tests(),
        content,
        registry: registry::Cache::new(),
        denylist: denylist::Snapshot::default(),
        flags: feature_flags::Snapshot::default(),
        map: map::MapState::new(),
        ab_cdn: ab_cdn::Cache::new(tmp.0.join("ab-cdn"), u64::MAX).unwrap(),
        satellite: satellite::SatelliteState::new(1 << 20, 64),
        world_manifest: world_manifest::WorldManifestState::default(),
        posters: posters::PosterState::default(),
        opensea: opensea::Runtime::default(),
        auth_api: auth_api::AuthApiState::default(),
        runtime_config: runtime_config::RuntimeConfigState::default(),
        onboarding: onboarding::OnboardingState::default(),
        eip1654: std::sync::OnceLock::new(),
    });
    Node { state, _tmp: tmp }
}

/// The state alone, its temporary storage leaked for the test's lifetime.
pub(crate) async fn node_without_db_state(settings: Settings) -> Shared {
    let node = node_without_db(settings).await;
    let state = node.state.clone();
    std::mem::forget(node);
    state
}

/// Through `State::new`: the mirrored rows are loaded at construction.
async fn node_with_db(settings: Settings, pool: PgPool) -> Node {
    let (content, tmp) = content_state(pool.clone()).await;
    let state = State::new(settings, pool, crate::http::Outbound::for_tests(), content)
        .await
        .unwrap();
    Node { state, _tmp: tmp }
}

struct ScratchDb {
    pool: PgPool,
    admin: PgPool,
    name: String,
}

async fn scratch_database() -> Option<ScratchDb> {
    let Ok(url) = std::env::var("CATALYST_TEST_DATABASE_URL") else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return None;
    };
    let base: PgConnectOptions = url
        .parse()
        .expect("CATALYST_TEST_DATABASE_URL must be a postgres DSN");
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(base.clone().database("postgres"))
        .await
        .unwrap();
    let name = format!("catalyst_test_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
        .execute(&admin)
        .await
        .unwrap();
    let pool = PgPoolOptions::new()
        .max_connections(6)
        .connect_with(base.database(&name).options([("TimeZone", "UTC")]))
        .await
        .unwrap();
    crate::db::apply_schema(&pool).await.unwrap();
    Some(ScratchDb { pool, admin, name })
}

impl ScratchDb {
    async fn teardown(self) {
        self.pool.close().await;
        let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE \"{}\" WITH (FORCE)",
            self.name
        )))
        .execute(&self.admin)
        .await;
    }
}

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut req = Request::builder().method(method).uri(path);
    let body = match body {
        Some(v) => {
            req = req.header("content-type", "application/json");
            Body::from(v.to_string())
        }
        None => Body::empty(),
    };
    let resp = app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, headers, bytes.to_vec())
}

async fn call_json(
    app: &Router,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let (status, _, bytes) = call(app, method, path, body).await;
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()))
    };
    (status, value)
}

/// A live job context (cancelled by `jobs.shutdown`) to run a pass under.
async fn job_ctx() -> (Jobs, crate::jobs::JobContext) {
    let jobs = Jobs::new();
    let (tx, rx) = tokio::sync::oneshot::channel();
    jobs.spawn("probe", move |ctx| async move {
        let _ = tx.send(ctx.clone());
        ctx.cancel.cancelled().await;
        Ok(())
    });
    let ctx = rx.await.unwrap();
    (jobs, ctx)
}

fn key_set(v: &Value) -> BTreeSet<String> {
    v.as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default()
}

#[test]
fn sources_must_be_public_and_not_this_node() {
    let mut settings = Settings::for_tests(Sources::defaults());
    settings.allow_private_sources = false;
    settings.public_url = "https://gateway.example.org".into();
    settings
        .validate()
        .expect("the decentraland.org defaults validate");

    let private = [
        "http://127.0.0.1:5210",
        "https://localhost/",
        "https://router/",
        "https://169.254.169.254/x",
        "ftp://example.com/",
    ];
    for url in private {
        let mut s = settings.clone();
        s.sources.lists = url.into();
        let err = s.validate().unwrap_err().to_string();
        assert!(err.contains("EXPLORER_LISTS_SOURCE_URL"), "{url}: {err}");
    }
    let mut s = settings.clone();
    s.sources.map = "https://gateway.example.org/api".into();
    let err = s.validate().unwrap_err().to_string();
    assert!(err.contains("own host gateway.example.org"), "{err}");
    assert!(s.guarded_url("https://api.decentraland.org").is_ok());

    // CONTENT_URL / LAMBDAS_URL hosts count as this node too.
    let mut s = settings.clone();
    s.own_hosts = vec!["content.example.org".into()];
    s.sources.denylist = "https://content.example.org/denylist.json".into();
    let err = s.validate().unwrap_err().to_string();
    assert!(
        err.contains("EXPLORER_DENYLIST_SOURCE_URL")
            && err.contains("own host content.example.org"),
        "{err}"
    );
}

#[test]
fn sources_from_env_strip_trailing_slashes() {
    let d = Sources::defaults();
    assert_eq!(d.all().len(), 6);
    assert!(d.all().iter().all(|(_, u)| !u.ends_with('/')));
}

/// The read-through: one upstream call per object, then the file; the
/// `assets/` family fetched as the CDN lays it out; a 404 remembered;
/// `HEAD` served too.
#[tokio::test]
async fn ab_cdn_reads_through_once_and_serves_the_cached_object_after() {
    let mock = mock();
    let addr = serve_mock(mock.clone()).await;
    let node = node_without_db(Settings::for_tests(sources_at(addr))).await;
    let app = router(node.state.clone());
    let path = "/ab-cdn/v49/bafkreidnionrlsubg63yjdeadaeyk3m5p6gofewq6kfqsju42kfx5cniwu/scene.json";
    let (status, headers, body) = call(&app, "GET", path, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["x-ab-cdn-source"], "upstream");
    assert_eq!(headers[header::CONTENT_TYPE], "application/json");
    assert_eq!(
        headers[header::CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );
    assert_eq!(
        String::from_utf8(body).unwrap(),
        "bundle:bafkreidnionrlsubg63yjdeadaeyk3m5p6gofewq6kfqsju42kfx5cniwu/scene.json"
    );
    let (status, headers, body) = call(&app, "GET", path, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["x-ab-cdn-source"], "store");
    assert_eq!(
        body.len(),
        "bundle:bafkreidnionrlsubg63yjdeadaeyk3m5p6gofewq6kfqsju42kfx5cniwu/scene.json".len()
    );
    assert_eq!(
        mock.hits("ab-cdn:bafkreidnionrlsubg63yjdeadaeyk3m5p6gofewq6kfqsju42kfx5cniwu/scene.json"),
        1
    );
    assert!(
        node.state
            .ab_cdn
            .store
            .path("v49/bafkreidnionrlsubg63yjdeadaeyk3m5p6gofewq6kfqsju42kfx5cniwu/scene.json")
            .unwrap()
            .exists()
    );

    let (status, headers, _) = call(&app, "GET", "/ab-cdn/v49/assets/bafy_mac", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers[header::CONTENT_TYPE],
        "application/wasm",
        "a platform-suffixed bundle, as the CDN answers"
    );
    assert_eq!(
        mock.hits("ab-cdn:assets/bafy_mac"),
        1,
        "the assets/ segment is the CDN's layout and stays in the upstream path"
    );

    let (status, _, _) = call(&app, "GET", "/ab-cdn/LOD/0/bafk_missing_mac", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = call(&app, "GET", "/ab-cdn/LOD/0/bafk_missing_mac", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        mock.hits("ab-cdn:0/bafk_missing_mac"),
        1,
        "an upstream 404 is remembered"
    );

    let (status, headers, body) = call(&app, "HEAD", "/ab-cdn/v49/assets/bafy_mac", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["x-ab-cdn-source"], "store");
    assert!(body.is_empty());
    assert_eq!(
        call(&app, "GET", "/ab-cdn/content/x", None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&app, "GET", "/ab-cdn/v49/../x", None).await.0,
        StatusCode::NOT_FOUND
    );
}

/// With no registry or asset-bundle CDN configured (an island, or a bare
/// node) neither reader sends a request: the registry knows nothing and
/// every object is this node's own. Blank used to fall back to
/// decentraland.org, and an empty source built a relative URL (502).
#[tokio::test]
async fn no_registry_or_cdn_source_reads_nothing_upstream() {
    let config = crate::config::Config::parse_for_tests(&[
        "x",
        "--database-url",
        "postgres:///catalyst",
        "--abgen-upstream-ab-cdn",
        " ",
    ])
    .unwrap();
    let from_config = Sources::from_config(&config);
    assert_eq!(
        (from_config.registry.as_str(), from_config.ab_cdn.as_str()),
        ("", "")
    );

    let mock = mock();
    let addr = serve_mock(mock.clone()).await;
    let sources = Sources {
        registry: String::new(),
        ab_cdn: String::new(),
        ..sources_at(addr)
    };
    let settings = Settings::for_tests(sources);
    settings.validate().unwrap();
    let node = node_without_db(settings).await;
    let app = router(node.state.clone());
    for path in ["/entities/active", "/entities/versions"] {
        let (status, body) = call_json(
            &app,
            "POST",
            &format!("/asset-bundle-registry{path}"),
            Some(json!({"pointers": ["0,0"]})),
        )
        .await;
        assert_eq!((status, body), (StatusCode::OK, json!([])), "{path}");
    }
    let (status, body) = call_json(
        &app,
        "POST",
        "/asset-bundle-registry/profiles",
        Some(json!({"ids": ["0xc9c29ab98e6bc42015985165a11153f564e9f8c2"]})),
    )
    .await;
    assert_eq!((status, body), (StatusCode::OK, json!([])));
    let (status, _, _) = call(&app, "GET", "/ab-cdn/v49/assets/bafy_mac", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(mock.hits.lock().is_empty(), "{:?}", mock.hits.lock());
}

/// The OpenSea resolver's remote legs follow the RPC relay upstreams: blank
/// ones (an island's) leave it no chain and no CryptoKitties to ask, where
/// it used to fall back to decentraland.org's RPC; set ones keep both.
#[test]
fn opensea_reads_no_chain_or_cryptokitties_without_an_rpc_upstream() {
    let parse = |extra: &[&str]| {
        let mut args = vec!["x", "--database-url", "postgres:///catalyst"];
        args.extend_from_slice(extra);
        let config = crate::config::Config::parse_for_tests(&args).unwrap();
        Settings::from_config(&config, Sources::defaults()).opensea
    };
    let island = parse(&["--rpc-upstream-mainnet", "", "--rpc-upstream-polygon", ""]);
    assert_eq!(
        (
            island.rpc_ethereum.as_str(),
            island.rpc_polygon.as_str(),
            island.cryptokitties_api.as_str()
        ),
        ("", "", "")
    );
    let mirror = parse(&[
        "--rpc-upstream-mainnet",
        "https://eth.example.org/",
        "--rpc-upstream-polygon",
        "https://polygon.example.org/",
        "--opensea-cryptokitties-api",
        "https://kitties.example.org/",
    ]);
    assert_eq!(
        (mirror.rpc_ethereum.as_str(), mirror.rpc_polygon.as_str()),
        ("https://eth.example.org/", "https://polygon.example.org/")
    );
    assert_eq!(mirror.cryptokitties_api, "https://kitties.example.org");
    // An RPC upstream alone names no CryptoKitties API: none is built in.
    let rpc_only = parse(&["--rpc-upstream-mainnet", "https://eth.example.org/"]);
    assert_eq!(rpc_only.cryptokitties_api, "");
}

#[tokio::test]
async fn realm_about_keeps_the_upstream_shape_and_reflects_the_sync_state() {
    let node = node_without_db(Settings::for_tests(Sources::defaults())).await;
    let app = router(node.state.clone());

    let (status, ours) = call_json(&app, "GET", "/realm-provider-ea/main/about", None).await;
    assert_eq!(status, StatusCode::OK);
    let theirs: Value = serde_json::from_str(include_str!("about-upstream.json")).unwrap();
    assert_eq!(key_set(&ours), key_set(&theirs));
    assert_eq!(key_set(&ours["content"]), key_set(&theirs["content"]));
    assert_eq!(key_set(&ours["comms"]), key_set(&theirs["comms"]));
    assert_eq!(key_set(&ours["bff"]), key_set(&theirs["bff"]));
    let mut cfg = key_set(&theirs["configurations"]);
    cfg.extend(["localSceneParcels".to_string(), "skybox".to_string()]);
    assert_eq!(key_set(&ours["configurations"]), cfg);
    assert_eq!(
        key_set(&ours["configurations"]["map"]),
        key_set(&theirs["configurations"]["map"])
    );
    assert_eq!(
        ours["configurations"]["map"]["sizes"],
        theirs["configurations"]["map"]["sizes"]
    );
    assert_eq!(ours["configurations"]["realmName"], "main");
    assert_eq!(ours["content"]["publicUrl"], "http://localhost/content/");
    assert_eq!(ours["content"]["synchronizationStatus"], "Bootstrapping");
    assert_eq!(
        ours["healthy"], false,
        "a bootstrapping node does not claim to accept users"
    );
    assert_eq!(ours["acceptingUsers"], false);

    node.state.content.sync.set_state(SyncState::Syncing);
    for path in ["/realm-provider-ea/main", "/realm-provider-ea/about"] {
        let (_, doc) = call_json(&app, "GET", path, None).await;
        assert_eq!(doc["healthy"], true, "{path}");
        assert_eq!(doc["content"]["synchronizationStatus"], "Syncing");
    }

    let (status, _, body) = call(
        &app,
        "GET",
        "/realm-provider-ea/main/preview-wearables",
        None,
    )
    .await;
    assert_eq!(
        (status, body.as_slice()),
        (StatusCode::NOT_FOUND, b"Not found".as_slice())
    );
    let (status, realms) = call_json(&app, "GET", "/realm-provider-ea/realms", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(realms[0]["serverName"], "main");
    let (_, st) = call_json(&app, "GET", "/realm-provider-ea/status", None).await;
    assert_eq!(
        key_set(&st),
        ["commitHash", "currentTime", "version"]
            .into_iter()
            .map(String::from)
            .collect()
    );
}

struct MutableRealmCore {
    status: Mutex<(bool, u64)>,
    probes: std::sync::atomic::AtomicUsize,
}

impl crate::content::about_comms::CommsCore for MutableRealmCore {
    fn core_status(&self) -> (bool, u64) {
        self.probes.fetch_add(1, Ordering::SeqCst);
        *self.status.lock()
    }
}

async fn realm_discovery_cases(with_core: bool) {
    use crate::content::about_comms::{AboutCommsState, Settings as CommsSettings};
    let node = node_without_db(Settings::for_tests(Sources::defaults())).await;
    node.state.content.sync.set_state(SyncState::Syncing);
    let app = router(node.state.clone());
    let attachment = Arc::new(AboutCommsState::new(CommsSettings {
        protocol: "v3".into(),
        fixed_adapter: "offline:offline".into(),
        version: None,
        commit_hash: None,
        v4: None,
        max_users: Some(1),
        offline_when_unreachable: false,
    }));
    assert!(node.state.content.attach_about_comms(attachment.clone()));
    let core = Arc::new(MutableRealmCore {
        status: Mutex::new((true, 42)),
        probes: Default::default(),
    });
    if with_core {
        assert!(attachment.attach_core(core.clone()));
    }
    let cases = if with_core {
        vec![
            (true, 42, true),
            (true, 0, true),
            (false, 42, true),
            (true, 42, false),
        ]
    } else {
        vec![(false, 0, true)]
    };
    for (core_healthy, count, syncing) in cases {
        *core.status.lock() = (core_healthy, count);
        node.state.content.sync.set_state(if syncing {
            SyncState::Syncing
        } else {
            SyncState::Bootstrapping
        });
        for path in [
            "/realm-provider-ea/main/about",
            "/realm-provider-ea/main",
            "/realm-provider-ea/about",
        ] {
            let before = core.probes.load(Ordering::SeqCst);
            let (status, doc) = call_json(&app, "GET", path, None).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(doc["comms"]["healthy"], core_healthy, "{path}");
            assert_eq!(doc["comms"]["usersCount"], count, "{path}");
            assert_eq!(doc["bff"]["userCount"], count, "{path}");
            assert_eq!(doc["content"]["healthy"], syncing, "{path}");
            assert_eq!(doc["healthy"], syncing && core_healthy, "{path}");
            assert_eq!(doc["acceptingUsers"], syncing && core_healthy, "{path}");
            assert_eq!(doc["comms"]["adapter"], node.state.settings.comms_adapter);
            assert_eq!(
                core.probes.load(Ordering::SeqCst) - before,
                usize::from(with_core)
            );
        }
        let before = core.probes.load(Ordering::SeqCst);
        let (status, realms) = call_json(&app, "GET", "/realm-provider-ea/realms", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(realms.as_array().unwrap().len(), usize::from(core_healthy));
        if core_healthy {
            assert_eq!(realms[0]["usersCount"], count);
        }
        assert_eq!(
            core.probes.load(Ordering::SeqCst) - before,
            usize::from(with_core)
        );
    }
}

#[tokio::test]
async fn realm_about_projects_attached_discovery_without_replacing_content_readiness() {
    realm_discovery_cases(true).await;
}

#[tokio::test]
async fn realm_about_attached_without_core_is_unhealthy() {
    realm_discovery_cases(false).await;
}

#[tokio::test]
async fn feature_flags_serve_the_embedded_full_set_before_the_first_pass() {
    let node = node_without_db(Settings::for_tests(Sources::defaults())).await;
    let app = router(node.state.clone());
    let (status, headers, body) = call(&app, "GET", "/feature-flags/explorer.json", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["cache-control"], "private,no-store,max-age=0");
    let doc: Value = serde_json::from_slice(&body).unwrap();
    feature_flags::validate(&doc).unwrap();
    assert_eq!(
        doc["flags"]["explorer-alfa-minimum-requirements"], true,
        "the referer-gated flag is in the served set"
    );
    let (status, flag) = call_json(
        &app,
        "GET",
        "/feature-flags/flags/explorer-alfa-minimum-requirements",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        flag["name"], "minimum_requirements",
        "a variant of that name answers before the plain flag"
    );
    let plain = doc["flags"]
        .as_object()
        .unwrap()
        .keys()
        .find(|k| doc["variants"].get(k).is_none())
        .unwrap()
        .clone();
    let (status, flag) =
        call_json(&app, "GET", &format!("/feature-flags/flags/{plain}"), None).await;
    assert_eq!((status, &flag), (StatusCode::OK, &doc["flags"][&plain]));
    let (status, missing) = call_json(&app, "GET", "/feature-flags/flags/no-such-flag", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        missing,
        json!({"error": "flag_not_found", "name": "no-such-flag"})
    );
}

#[tokio::test]
async fn registry_entities_are_cached_per_pointer_including_misses() {
    let mock = mock();
    let addr = serve_mock(mock.clone()).await;
    let node = node_without_db(Settings::for_tests(sources_at(addr))).await;
    let app = router(node.state.clone());
    let path = "/asset-bundle-registry/entities/active";

    let (status, docs) = call_json(
        &app,
        "POST",
        path,
        Some(json!({"pointers": ["0,0", "0,1"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        docs.as_array().unwrap().len(),
        1,
        "one entity for two of its pointers"
    );
    assert_eq!(docs[0]["id"], "bafyscene");
    assert_eq!(
        key_set(&docs[0]),
        [
            "bundles",
            "content",
            "deployer",
            "id",
            "metadata",
            "pointers",
            "status",
            "timestamp",
            "type",
            "versions"
        ]
        .into_iter()
        .map(String::from)
        .collect()
    );
    assert_eq!(mock.hits("entities/active"), 1);

    let (_, again) = call_json(
        &app,
        "POST",
        path,
        Some(json!({"pointers": ["0,1", "0,0"]})),
    )
    .await;
    assert_eq!(again, docs);
    assert_eq!(
        mock.hits("entities/active"),
        1,
        "both pointers answered from the cache"
    );

    let (_, mixed) = call_json(
        &app,
        "POST",
        path,
        Some(json!({"pointers": ["0,0", "9,9"]})),
    )
    .await;
    assert_eq!(mixed.as_array().unwrap().len(), 1);
    assert_eq!(
        mock.hits("entities/active"),
        2,
        "only the miss goes upstream"
    );
    let last = mock.bodies.lock().last().cloned().unwrap();
    assert_eq!(last["body"]["pointers"], json!(["9,9"]));

    let (_, none) = call_json(&app, "POST", path, Some(json!({"pointers": ["9,9"]}))).await;
    assert_eq!(none, json!([]));
    assert_eq!(
        mock.hits("entities/active"),
        2,
        "an absent pointer is cached as absent"
    );

    let (_, by_id) = call_json(&app, "POST", path, Some(json!({"ids": ["bafyother"]}))).await;
    assert_eq!(by_id[0]["pointers"], json!(["5,5"]));
    assert_eq!(mock.hits("entities/active"), 3);

    let (_, world) = call_json(
        &app,
        "POST",
        &format!("{path}?world_name=Foo.dcl.eth"),
        Some(json!({"pointers": ["0,0"]})),
    )
    .await;
    assert_eq!(world[0]["id"], "bafyscene");
    assert_eq!(
        mock.hits("entities/active"),
        4,
        "a world is its own pointer namespace"
    );
    assert_eq!(
        mock.bodies.lock().last().unwrap()["query"]["world_name"],
        "Foo.dcl.eth"
    );

    for bad in [
        json!({}),
        json!({"pointers": []}),
        json!({"pointers": ["0,0"], "ids": ["x"]}),
    ] {
        let (status, _) = call_json(&app, "POST", path, Some(bad)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    let too_many: Vec<String> = (0..1001).map(|i| format!("{i},0")).collect();
    let (status, _) = call_json(&app, "POST", path, Some(json!({"pointers": too_many}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn registry_profiles_are_matched_by_address_and_cached() {
    let mock = mock();
    let addr = serve_mock(mock.clone()).await;
    let node = node_without_db(Settings::for_tests(sources_at(addr))).await;
    let app = router(node.state.clone());
    let path = "/asset-bundle-registry/profiles";

    let (status, docs) = call_json(&app, "POST", path, Some(json!({"ids": ["0xC9C29AB98E6BC42015985165A11153F564E9F8C2", "0x0000000000000000000000000000000000000009"]}))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(docs.as_array().unwrap().len(), 1);
    assert_eq!(docs[0]["avatars"][0]["name"], "Ryumi");
    assert_eq!(
        docs[0]["timestamp"], "1790276949479",
        "the registry's string timestamp is served as is"
    );
    assert_eq!(mock.hits("profiles"), 1);

    let (_, again) = call_json(
        &app,
        "POST",
        path,
        Some(json!({"ids": ["0xc9c29ab98e6bc42015985165a11153f564e9f8c2"]})),
    )
    .await;
    assert_eq!(again, docs);
    let (_, unknown) = call_json(
        &app,
        "POST",
        path,
        Some(json!({"ids": ["0x0000000000000000000000000000000000000009"]})),
    )
    .await;
    assert_eq!(unknown, json!([]));
    assert_eq!(
        mock.hits("profiles"),
        1,
        "known and unknown addresses are both cached"
    );

    let (status, _) = call_json(&app, "POST", path, Some(json!({"ids": []}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, st) = call_json(&app, "GET", "/asset-bundle-registry/status", None).await;
    assert_eq!(status, StatusCode::OK);
    let (health_status, health) =
        call_json(&app, "GET", "/asset-bundle-registry/health", None).await;
    assert_eq!(health_status, StatusCode::OK);
    assert_eq!(health, json!({"status": "ok"}));
    assert_eq!(
        key_set(&st["data"]),
        ["commitHash", "currentTime", "version"]
            .into_iter()
            .map(String::from)
            .collect()
    );
}

/// `/profiles/metadata` answers upstream's projection (`pointer`, no
/// `avatars`): a document is matched to its address by `pointer`, so a known
/// profile is not dropped as address-less and the answer is not `[]`.
#[tokio::test]
async fn registry_profile_metadata_is_matched_by_pointer_and_cached() {
    let mock = mock();
    let addr = serve_mock(mock.clone()).await;
    let node = node_without_db(Settings::for_tests(sources_at(addr))).await;
    let app = router(node.state.clone());
    let path = "/asset-bundle-registry/profiles/metadata";

    let (status, docs) = call_json(&app, "POST", path, Some(json!({"ids": ["0xC9C29AB98E6BC42015985165A11153F564E9F8C2", "0x0000000000000000000000000000000000000009"]}))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        docs.as_array().unwrap().len(),
        1,
        "the known profile's metadata row is served"
    );
    assert_eq!(
        key_set(&docs[0]),
        ["hasClaimedName", "name", "pointer", "thumbnailUrl"]
            .into_iter()
            .map(String::from)
            .collect()
    );
    assert_eq!(
        docs[0]["pointer"],
        "0xc9c29ab98e6bc42015985165a11153f564e9f8c2"
    );
    assert_eq!(docs[0]["name"], "Ryumi");
    assert_eq!(mock.hits("profiles/metadata"), 1);

    let (_, again) = call_json(
        &app,
        "POST",
        path,
        Some(json!({"ids": ["0xc9c29ab98e6bc42015985165a11153f564e9f8c2"]})),
    )
    .await;
    assert_eq!(again, docs);
    let (_, unknown) = call_json(
        &app,
        "POST",
        path,
        Some(json!({"ids": ["0x0000000000000000000000000000000000000009"]})),
    )
    .await;
    assert_eq!(unknown, json!([]), "an address the registry has no row for");
    assert_eq!(
        mock.hits("profiles/metadata"),
        1,
        "known and unknown addresses are both cached"
    );
    assert_eq!(
        mock.hits("profiles"),
        0,
        "the metadata variant never asks for the full profile"
    );
}

/// The district documents of `api.decentraland.org` (`/v2/districts[/{id}]`,
/// `/v2/addresses/{address}/contributions`) are served from the embedded
/// upstream data, without a grid mirror pass.
#[tokio::test]
async fn map_district_documents_are_served_from_the_embedded_upstream_data() {
    let node = node_without_db(Settings::for_tests(sources_at(
        "127.0.0.1:1".parse().unwrap(),
    )))
    .await;
    let app = router(node.state.clone());

    let (status, all) = call_json(&app, "GET", "/api/v2/districts", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(all["ok"], true);
    assert_eq!(all["data"].as_array().unwrap().len(), 42);

    let (status, one) = call_json(
        &app,
        "GET",
        "/api/v2/districts/ea2c3a3e-bbd6-49d9-814c-3b64b37b057a",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(one["data"]["name"], "SciArt Lab");
    assert_eq!(one["data"]["totalParcels"], 208);
    assert_eq!(
        key_set(&one["data"]),
        ["description", "id", "name", "parcels", "totalParcels"]
            .into_iter()
            .map(String::from)
            .collect()
    );

    let (status, headers, body) =
        call(&app, "GET", "/api/v2/districts/no-such-district", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(headers["content-type"], "text/plain; charset=utf-8");
    assert_eq!(body, b"Not found");

    let (status, rows) = call_json(
        &app,
        "GET",
        "/api/v2/addresses/0x004398361d0fad60d7fcf5533d94398ee9fc8ddc/contributions",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        rows["data"][0]["districtId"],
        "219ac351-e6ce-4e17-8b84-eb008afddf69"
    );
    let (_, none) = call_json(
        &app,
        "GET",
        "/api/v2/addresses/0x8d6f63e382d73cf53858864f673f39e9ff915a1e/contributions",
        None,
    )
    .await;
    assert_eq!(none, json!({"ok": true, "data": []}));
}

#[tokio::test]
async fn registry_upstream_failure_is_a_502_not_a_short_answer() {
    let node = node_without_db(Settings::for_tests(sources_at(
        "127.0.0.1:1".parse().unwrap(),
    )))
    .await;
    let app = router(node.state.clone());
    let (status, body) = call_json(
        &app,
        "POST",
        "/asset-bundle-registry/entities/active",
        Some(json!({"pointers": ["0,0"]})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("asset-bundle registry upstream"),
        "{body}"
    );
}

#[tokio::test]
async fn lists_mirror_prunes_upstream_deletions_and_keeps_the_local_overlay() {
    let Some(db) = scratch_database().await else {
        return;
    };
    let mock = mock();
    let addr = serve_mock(mock.clone()).await;
    let node = node_with_db(Settings::for_tests(sources_at(addr)), db.pool.clone()).await;
    let app = router(node.state.clone());
    let (jobs, ctx) = job_ctx().await;

    let (status, empty) = call_json(&app, "POST", "/dcl-lists/pois", None).await;
    assert_eq!((status, empty), (StatusCode::OK, json!({"data": []})));

    lists::run_pass(&node.state, &ctx).await.unwrap();
    let (_, pois) = call_json(&app, "POST", "/dcl-lists/pois", None).await;
    assert_eq!(pois, json!({"data": ["-148,131", "0,0", "16,83"]}));
    let (_, banned) = call_json(&app, "POST", "/dcl-lists/banned-names", None).await;
    assert_eq!(banned, json!({"data": ["badname"]}));

    lists::set_local(&db.pool, "pois", "9,9", true)
        .await
        .unwrap();
    lists::set_local(&db.pool, "pois", "0,0", false)
        .await
        .unwrap();
    let (_, pois) = call_json(&app, "POST", "/dcl-lists/pois", None).await;
    assert_eq!(
        pois,
        json!({"data": ["-148,131", "16,83", "9,9"]}),
        "local add and local hide apply over the mirror"
    );

    *mock.pois.lock() = vec!["0,0".into(), "1,1".into(), "-148,131".into()];
    lists::run_pass(&node.state, &ctx).await.unwrap();
    let (_, pois) = call_json(&app, "POST", "/dcl-lists/pois", None).await;
    assert_eq!(
        pois,
        json!({"data": ["-148,131", "1,1", "9,9"]}),
        "16,83 pruned, 1,1 added, the overlay untouched"
    );
    let (local_rows,): (i64,) = sqlx::query_as("SELECT count(*) FROM explore_api.lists_local")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(local_rows, 2);
    lists::clear_local(&db.pool, "pois", "0,0").await.unwrap();
    let (_, pois) = call_json(&app, "POST", "/dcl-lists/pois", None).await;
    assert_eq!(pois["data"].as_array().unwrap().len(), 4);

    let (_, report) = call_json(&app, "GET", "/mirrors", None).await;
    let lists_row = report["mirrors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["mirror"] == "lists")
        .unwrap()
        .clone();
    assert_eq!(lists_row["passes"], 2);
    assert_eq!(lists_row["items"], 4);
    assert_eq!(lists_row["complete"], true);
    assert!(lists_row["lastError"].is_null());

    mock.pois.lock().clear();
    let err = lists::run_pass(&node.state, &ctx)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("empty list"), "{err}");
    let (_, pois) = call_json(&app, "POST", "/dcl-lists/pois", None).await;
    assert_eq!(
        pois["data"].as_array().unwrap().len(),
        4,
        "a refused pass leaves the mirror as it was"
    );
    let (_, report) = call_json(&app, "GET", "/mirrors", None).await;
    let lists_row = report["mirrors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["mirror"] == "lists")
        .unwrap()
        .clone();
    assert_eq!(
        lists_row["complete"], false,
        "started after the last completion"
    );
    assert_eq!(lists_row["passes"], 2);
    assert!(
        lists_row["lastError"]
            .as_str()
            .unwrap()
            .contains("empty list")
    );

    jobs.shutdown(Duration::from_secs(1)).await;
    db.teardown().await;
}

#[tokio::test]
async fn denylist_mirror_keeps_upstream_casing_and_the_overlay_wins() {
    let Some(db) = scratch_database().await else {
        return;
    };
    let mock = mock();
    let addr = serve_mock(mock.clone()).await;
    let node = node_with_db(Settings::for_tests(sources_at(addr)), db.pool.clone()).await;
    let app = router(node.state.clone());
    let (jobs, ctx) = job_ctx().await;

    let (_, empty) = call_json(&app, "GET", "/config/denylist.json", None).await;
    assert_eq!(empty, json!({"users": []}));
    denylist::run_pass(&node.state, &ctx).await.unwrap();
    let (status, doc) = call_json(&app, "GET", "/config/denylist.json", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        doc,
        json!({"users": [{"wallet": "0x0000000000000000000000000000000000000002"}, {"wallet": "0xAbC0000000000000000000000000000000000001"}]})
    );
    assert!(
        node.state
            .denylist
            .contains("0xabc0000000000000000000000000000000000001")
    );
    assert!(
        node.state
            .denylist
            .contains(" 0xABC0000000000000000000000000000000000001 ")
    );

    denylist::set_local(
        &db.pool,
        "0xAbC0000000000000000000000000000000000001",
        false,
    )
    .await
    .unwrap();
    denylist::set_local(&db.pool, "0xLocal", true)
        .await
        .unwrap();
    node.state.denylist.reload(&db.pool).await.unwrap();
    let (_, doc) = call_json(&app, "GET", "/config/denylist.json", None).await;
    assert_eq!(
        doc,
        json!({"users": [{"wallet": "0x0000000000000000000000000000000000000002"}, {"wallet": "0xlocal"}]})
    );
    assert!(
        !node
            .state
            .denylist
            .contains("0xabc0000000000000000000000000000000000001")
    );

    *mock.denylist.lock() = vec![json!({"wallet": "0xAbC0000000000000000000000000000000000001"})];
    denylist::run_pass(&node.state, &ctx).await.unwrap();
    let (_, doc) = call_json(&app, "GET", "/config/denylist.json", None).await;
    assert_eq!(
        doc,
        json!({"users": [{"wallet": "0xlocal"}]}),
        "the pruned wallet is gone, the hidden one stays hidden, the local one stays"
    );

    let restarted = node_with_db(Settings::for_tests(sources_at(addr)), db.pool.clone()).await;
    assert!(
        restarted.state.denylist.contains("0xlocal"),
        "a restart loads the mirrored + local set"
    );
    assert_eq!(
        mock.hits("denylist"),
        2,
        "loading at startup costs no upstream call"
    );

    jobs.shutdown(Duration::from_secs(1)).await;
    db.teardown().await;
}

#[tokio::test]
async fn feature_flags_mirror_sends_the_referer_applies_overrides_and_uses_the_etag() {
    let Some(db) = scratch_database().await else {
        return;
    };
    let mock = mock();
    let addr = serve_mock(mock.clone()).await;
    let node = node_with_db(Settings::for_tests(sources_at(addr)), db.pool.clone()).await;
    let app = router(node.state.clone());
    let (jobs, ctx) = job_ctx().await;

    feature_flags::run_pass(&node.state, &ctx).await.unwrap();
    let (_, doc) = call_json(&app, "GET", "/feature-flags/explorer.json", None).await;
    assert_eq!(
        doc["flags"],
        json!({"explorer-alfa-minimum-requirements": true, "explorer-feature-a": false}),
        "the referer-gated full set"
    );
    let (_, variant) = call_json(&app, "GET", "/feature-flags/flags/explorer-variant", None).await;
    assert_eq!(variant, json!({"name": "v", "enabled": true}));

    feature_flags::set_local_flag(&db.pool, "explorer", "explorer-feature-a", true)
        .await
        .unwrap();
    feature_flags::set_local_flag(&db.pool, "explorer", "explorer-local-only", true)
        .await
        .unwrap();
    node.state.flags.reload(&db.pool, "explorer").await.unwrap();
    let (_, doc) = call_json(&app, "GET", "/feature-flags/explorer.json", None).await;
    assert_eq!(doc["flags"]["explorer-feature-a"], true);
    assert_eq!(doc["flags"]["explorer-local-only"], true);
    let (stored,): (Value,) =
        sqlx::query_as("SELECT document FROM explore_api.feature_flags WHERE app = 'explorer'")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(
        stored["flags"]["explorer-feature-a"], false,
        "the mirror row is upstream's, untouched by the override"
    );

    feature_flags::run_pass(&node.state, &ctx).await.unwrap();
    assert_eq!(
        mock.hits("flags-304"),
        1,
        "the second pass sends the stored etag"
    );
    let (_, doc) = call_json(&app, "GET", "/feature-flags/explorer.json", None).await;
    assert_eq!(
        doc["flags"]["explorer-feature-a"], true,
        "overrides survive a 304 pass"
    );

    mock.flags.lock()["flags"]["explorer-feature-b"] = json!(true);
    *mock.flags_etag.lock() = "\"flags-2\"".into();
    feature_flags::run_pass(&node.state, &ctx).await.unwrap();
    let (_, doc) = call_json(&app, "GET", "/feature-flags/explorer.json", None).await;
    assert_eq!(doc["flags"]["explorer-feature-b"], true);
    let (_, report) = call_json(&app, "GET", "/mirrors", None).await;
    let row = report["mirrors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["mirror"] == "feature-flags")
        .unwrap()
        .clone();
    assert_eq!(row["cursor"], "\"flags-2\"");
    assert_eq!(row["passes"], 3);

    feature_flags::clear_local_flag(&db.pool, "explorer", "explorer-feature-a")
        .await
        .unwrap();
    let restarted = node_with_db(Settings::for_tests(sources_at(addr)), db.pool.clone()).await;
    let doc = restarted.state.flags.document();
    assert_eq!(doc["flags"]["explorer-feature-a"], false);
    assert_eq!(doc["flags"]["explorer-local-only"], true);

    jobs.shutdown(Duration::from_secs(1)).await;
    db.teardown().await;
}

#[tokio::test]
async fn map_grid_mirror_persists_the_grid_resumes_after_restart_and_gates_downloads() {
    let Some(db) = scratch_database().await else {
        return;
    };
    let mock = mock();
    let addr = serve_mock(mock.clone()).await;
    let node = node_with_db(Settings::for_tests(sources_at(addr)), db.pool.clone()).await;
    let app = router(node.state.clone());
    let (jobs, ctx) = job_ctx().await;

    let (status, _, _) = call(
        &app,
        "GET",
        "/api/v1/map.png?width=64&height=64&center=0,0&size=5",
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "no grid before the first pass"
    );
    let (status, _) = call_json(&app, "GET", "/api/v2/tiles/info", None).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);

    map::run_pass(&node.state, &ctx).await.unwrap();
    assert_eq!((mock.hits("tiles/info"), mock.hits("tiles")), (1, 1));
    let (status, info) = call_json(&app, "GET", "/api/v2/tiles/info", None).await;
    assert_eq!(
        (status, info),
        (StatusCode::OK, json!({"lastUpdatedAt": 1000}))
    );

    let (status, headers, png) = call(
        &app,
        "GET",
        "/api/v1/map.png?width=64&height=64&center=0,0&size=5",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["content-type"], "image/png");
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    assert!(
        headers["cache-control"]
            .to_str()
            .unwrap()
            .contains("max-age=120")
    );
    let etag = headers["etag"].to_str().unwrap().to_string();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v2/map.png?width=64&height=64&center=0,0&size=5")
                .header("if-none-match", &etag)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_MODIFIED);
    for path in [
        "/api/v1/minimap.png",
        "/api/v1/estatemap.png",
        "/api/v2/parcels/1/1/map.png",
        "/api/v1/estates/77/map.png",
    ] {
        let (status, headers, _) = call(&app, "GET", path, None).await;
        assert_eq!((path, status), (path, StatusCode::OK));
        assert_eq!(headers["content-type"], "image/png");
    }
    let (status, _, _) = call(&app, "GET", "/api/v1/estates/404/map.png", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, tiles) = call_json(
        &app,
        "GET",
        "/api/v2/tiles?x1=0&x2=1&y1=0&y2=1&include=x,y,type",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tiles["ok"], true);
    assert_eq!(
        key_set(&tiles["data"]),
        ["0,0", "1,0", "1,1"]
            .into_iter()
            .map(String::from)
            .collect()
    );
    assert_eq!(
        tiles["data"]["0,0"],
        json!({"x": 0, "y": 0, "type": "plaza"})
    );
    let (_, legacy) = call_json(&app, "GET", "/api/v1/tiles?x1=1&x2=1&y1=1&y2=1", None).await;
    assert_eq!(legacy["data"]["1,1"]["type"], 9);

    let (stored_stamp, count): (i64, i32) =
        sqlx::query_as("SELECT last_updated_at, tile_count FROM explore_api.map_grid WHERE id = 1")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!((stored_stamp, count), (1000, 4));

    let restarted = node_with_db(Settings::for_tests(sources_at(addr)), db.pool.clone()).await;
    assert!(
        restarted.state.map.is_ready(),
        "the persisted grid is loaded at startup"
    );
    assert_eq!(restarted.state.map.last_updated_at(), 1000);
    assert_eq!(mock.hits("tiles"), 1, "no download at startup");
    let app2 = router(restarted.state.clone());
    let (status, _, _) = call(
        &app2,
        "GET",
        "/api/v1/map.png?width=64&height=64&center=0,0&size=5",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    map::run_pass(&restarted.state, &ctx).await.unwrap();
    assert_eq!(
        mock.hits("tiles"),
        1,
        "an unchanged upstream stamp downloads nothing"
    );

    mock.tiles_last_updated.store(2000, Ordering::SeqCst);
    map::run_pass(&restarted.state, &ctx).await.unwrap();
    assert_eq!(
        mock.hits("tiles"),
        1,
        "a fresh grid is not re-downloaded inside the refresh floor"
    );
    assert_eq!(restarted.state.map.last_updated_at(), 1000);

    let mut eager = Settings::for_tests(sources_at(addr));
    eager.map_min_refresh = Duration::ZERO;
    let eager = node_with_db(eager, db.pool.clone()).await;
    map::run_pass(&eager.state, &ctx).await.unwrap();
    assert_eq!(
        (mock.hits("tiles"), mock.hits("tiles-304")),
        (2, 1),
        "past the floor the stored etag makes it a 304"
    );
    assert_eq!(
        eager.state.map.last_updated_at(),
        2000,
        "the held grid is restamped"
    );
    let (stored_stamp,): (i64,) =
        sqlx::query_as("SELECT last_updated_at FROM explore_api.map_grid WHERE id = 1")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(stored_stamp, 2000);

    *mock.tiles.lock() = {
        let mut doc = tiles_doc(3000);
        doc["data"]["9,9"] = tile(9, 9, "owned", None);
        doc
    };
    *mock.tiles_etag.lock() = "\"grid-2\"".into();
    mock.tiles_last_updated.store(3000, Ordering::SeqCst);
    map::run_pass(&eager.state, &ctx).await.unwrap();
    assert_eq!(mock.hits("tiles"), 3);
    let app3 = router(eager.state.clone());
    let (_, tiles) = call_json(&app3, "GET", "/api/v2/tiles?include=x", None).await;
    assert_eq!(tiles["data"].as_object().unwrap().len(), 5);
    let (_, report) = call_json(&app3, "GET", "/mirrors", None).await;
    let row = report["mirrors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["mirror"] == "map-grid")
        .unwrap()
        .clone();
    assert_eq!(row["cursor"], "3000");
    assert_eq!(row["items"], 5);
    assert_eq!(row["complete"], true);
    assert_eq!(report["mapGridLastUpdatedAt"], 3000);

    jobs.shutdown(Duration::from_secs(1)).await;
    db.teardown().await;
}

/// One bounded run of every mirror pass and reader against the real upstreams
/// (`cargo test --lib live_upstream -- --ignored --nocapture`): one 38 MB grid
/// download, three small documents, two registry calls. Prints the `/mirrors`
/// report; the freshness evidence in tests/README.md comes from here.
#[tokio::test]
#[ignore]
async fn live_upstream_passes_complete_and_the_boot_routes_answer() {
    let Some(db) = scratch_database().await else {
        return;
    };
    let mut settings = Settings::for_tests(Sources::defaults());
    settings.allow_private_sources = false;
    settings.public_url = "https://v2.example.org".into();
    let node = node_with_db(settings, db.pool.clone()).await;
    let app = router(node.state.clone());
    let (jobs, ctx) = job_ctx().await;

    lists::run_pass(&node.state, &ctx).await.unwrap();
    denylist::run_pass(&node.state, &ctx).await.unwrap();
    feature_flags::run_pass(&node.state, &ctx).await.unwrap();
    map::run_pass(&node.state, &ctx).await.unwrap();

    let (status, pois) = call_json(&app, "POST", "/dcl-lists/pois", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        pois["data"].as_array().unwrap().iter().any(|p| p == "0,0"),
        "Genesis Plaza is a POI"
    );
    let (_, deny) = call_json(&app, "GET", "/config/denylist.json", None).await;
    assert!(deny["users"].as_array().unwrap().len() > 100);
    let (_, flags) = call_json(&app, "GET", "/feature-flags/explorer.json", None).await;
    assert_eq!(
        flags["flags"]["explorer-alfa-minimum-requirements"], true,
        "the referer-gated flag reached the mirror"
    );
    let (status, headers, png) = call(
        &app,
        "GET",
        "/api/v1/map.png?width=64&height=64&center=0,0&size=5",
        None,
    )
    .await;
    assert_eq!(
        (status, headers["content-type"].to_str().unwrap()),
        (StatusCode::OK, "image/png")
    );
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let (_, tiles) = call_json(&app, "GET", "/api/v2/tiles?x1=0&x2=0&y1=0&y2=0", None).await;
    assert_eq!(tiles["data"]["0,0"]["name"], "Genesis Plaza");
    let (status, docs) = call_json(
        &app,
        "POST",
        "/asset-bundle-registry/entities/active",
        Some(json!({"pointers": ["0,0"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(docs[0]["type"], "scene");
    assert!(docs[0]["bundles"].is_object());
    let (status, profiles) = call_json(
        &app,
        "POST",
        "/asset-bundle-registry/profiles",
        Some(json!({"ids": ["0xc9c29ab98e6bc42015985165a11153f564e9f8c2"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(profiles[0]["avatars"].is_array());

    let (_, report) = call_json(&app, "GET", "/mirrors", None).await;
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    println!(
        "pois={} denylist={} flags={} variants={} tiles={}",
        pois["data"].as_array().unwrap().len(),
        deny["users"].as_array().unwrap().len(),
        flags["flags"].as_object().unwrap().len(),
        flags["variants"].as_object().unwrap().len(),
        node.state.map.snapshot().unwrap().tiles.len()
    );
    for m in report["mirrors"].as_array().unwrap() {
        assert_eq!(m["complete"], true, "{m}");
    }

    let restarted = node_with_db(Settings::for_tests(Sources::defaults()), db.pool.clone()).await;
    assert!(restarted.state.map.is_ready());
    assert!(restarted.state.denylist.list().users.len() > 100);
    jobs.shutdown(Duration::from_secs(1)).await;
    db.teardown().await;
}

#[tokio::test]
async fn spawn_jobs_registers_every_explorer_job_and_stops_with_the_registry() {
    // The satellite mirror is optional (no upstream, no job); this node configures it so the
    // registry lists every job the explorer API can run.
    let mut settings = Settings::for_tests(sources_at("127.0.0.1:1".parse().unwrap()));
    settings.satellite_upstream = Some("http://127.0.0.1:1".to_string());
    settings.world_manifest_upstream = "http://127.0.0.1:1/WorldManifest.json".to_string();
    let node = node_without_db(settings).await;
    let jobs = Jobs::new();
    node.state.spawn_jobs(&jobs);
    let names: BTreeSet<String> = jobs.report().into_iter().map(|r| r.name).collect();
    assert_eq!(
        names,
        [
            "ab-cdn-maintenance",
            "auth-api-identity-sweep",
            "explorer-denylist-mirror",
            "explorer-flags-mirror",
            "explorer-lists-mirror",
            "explorer-map-mirror",
            "explorer-satellite-mirror",
            "explorer-world-manifest-mirror",
            "opensea-cache-expiry",
        ]
        .into_iter()
        .map(String::from)
        .collect()
    );
    assert!(
        jobs.shutdown(Duration::from_secs(5)).await.is_empty(),
        "every mirror loop yields to shutdown"
    );
}

/// With the mirrors on but no source named (no posture, or an island), no
/// mirror job runs and nothing reaches an upstream: a blank source is none,
/// never a built-in decentraland.org endpoint.
#[tokio::test]
async fn blank_sources_spawn_no_mirror_and_read_no_builtin_upstream() {
    let config = crate::config::Config::parse_for_tests(&[
        "x",
        "--database-url",
        "postgres:///catalyst",
        "--explorer-mirrors-enabled",
        "true",
        "--events-mirror-upstream",
        "true",
    ])
    .unwrap();
    let sources = Sources::from_config(&config);
    assert!(
        sources.all().iter().all(|(_, url)| url.is_empty()),
        "{sources:?}"
    );
    let settings = Settings::from_config(&config, sources);
    assert_eq!(settings.world_manifest_upstream, "");
    assert_eq!(settings.poster_upstream, None);
    assert_eq!(settings.opensea.fallback, None);
    assert!(settings.opensea.ipfs_gateways.is_empty());
    assert_eq!(settings.opensea.cryptokitties_api, "");
    settings.validate().unwrap();

    let settings = Settings::for_tests(Sources::from_config(&config));
    let node = node_without_db(settings).await;
    let jobs = Jobs::new();
    node.state.spawn_jobs(&jobs);
    let names: BTreeSet<String> = jobs.report().into_iter().map(|r| r.name).collect();
    assert!(names.iter().all(|n| !n.ends_with("-mirror")), "{names:?}");
    jobs.shutdown(Duration::from_secs(5)).await;
}
